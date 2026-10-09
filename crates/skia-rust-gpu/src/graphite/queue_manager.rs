// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/QueueManager.h, src/gpu/graphite/QueueManager.cpp

//! The `QueueManager`: owns the command buffers, submits them to the GPU in order, and tracks
//! which submissions are still outstanding.
//!
//! The neutral half is [`QueueManager`]. The backend half ([`QueueManagerBackend`]) makes new
//! command buffers and submits the current one (`getNewCommandBuffer` and `onSubmitToGpu` in
//! Skia); the wgpu one is G11c. Stats queries and semaphores are not ported (they come with G11c
//! and `Surface`).

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use crate::gpu::gpu_types::Protected;
use crate::gpu::ref_cnted_callback::{CallbackProc, RefCntedCallback};
use crate::gpu::sk_log::skia_log_e;
use skia_rust_core::point::IVector;
use skia_rust_core::rect::IRect;

use crate::graphite::buffer::Buffer;
use crate::graphite::command_buffer::CommandBuffer;
use crate::graphite::context_priv::{ContextPriv, SharedResourceProvider};
use crate::graphite::gpu_work_submission::GpuWorkSubmission;
use crate::graphite::graphite_types::{
    InsertFinishInfo, InsertRecordingInfo, InsertStatus, SubmitInfo, SyncToCpu,
};
use crate::graphite::recording::Recording;
use crate::graphite::resource::ResourceRef;
use crate::graphite::task::{ReplayTargetData, Status, Task};
use crate::graphite::upload_buffer_manager::UploadBufferManager;

/// `SK_InvalidGenID`: a recorder that does not require ordered recordings.
const INVALID_GEN_ID: u32 = 0;

/// The backend half of the queue manager: the virtual functions of `QueueManager` that a backend
/// overrides.
// Port of: src/gpu/graphite/QueueManager.h (the private virtuals, chrome/m156)
pub trait QueueManagerBackend: Send {
    /// `getNewCommandBuffer()`: a fresh command buffer, or `None` if it cannot be created.
    #[doc(alias = "getNewCommandBuffer")]
    fn get_new_command_buffer(
        &mut self,
        resource_provider: &SharedResourceProvider,
        protected: Protected,
    ) -> Option<Box<dyn CommandBuffer>>;

    /// `onSubmitToGpu()`: submits `command_buffer` and returns the submission that tracks it.
    #[doc(alias = "onSubmitToGpu")]
    fn on_submit_to_gpu(
        &mut self,
        command_buffer: Box<dyn CommandBuffer>,
        submit_info: &SubmitInfo,
    ) -> Option<GpuWorkSubmission>;

    /// `tick()`: called while waiting for async maps to finish.
    fn tick(&self) {}
}

/// Manages all command buffers and makes sure they are submitted to the GPU in the correct order.
// Port of: src/gpu/graphite/QueueManager.h#L27-L108 (chrome/m156)
#[doc(alias = "skgpu::graphite::QueueManager")]
pub struct QueueManager {
    /// `fSharedContext->isProtected()`.
    shared_is_protected: Protected,
    /// `fSharedContext->caps()->allowCpuSync()`.
    allow_cpu_sync: bool,
    /// `fCurrentCommandBuffer`.
    current_command_buffer: Option<Box<dyn CommandBuffer>>,
    /// `fOutstandingSubmissions`, oldest first.
    outstanding_submissions: VecDeque<GpuWorkSubmission>,
    /// `fAvailableCommandBuffers`.
    available_command_buffers: Vec<Box<dyn CommandBuffer>>,
    /// `fAvailableProtectedCommandBuffers`.
    available_protected_command_buffers: Vec<Box<dyn CommandBuffer>>,
    /// `fLastAddedRecordingIDs`: the last unique ID added per recorder.
    last_added_recording_ids: HashMap<u32, u32>,
    /// The backend half.
    backend: Box<dyn QueueManagerBackend>,
}

impl std::fmt::Debug for QueueManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueueManager")
            .field("shared_is_protected", &self.shared_is_protected)
            .field("allow_cpu_sync", &self.allow_cpu_sync)
            .field("outstanding_submissions", &self.outstanding_submissions.len())
            .finish_non_exhaustive()
    }
}

/// A failed insert: the status, and the message to log.
type InsertFailure = (InsertStatus, String);

/// A failure with the given status and message.
fn failure(status: InsertStatus, message: &str) -> InsertFailure {
    (status, message.to_owned())
}

/// `SIMULATE_FAIL(status)`: fails with `status` if it is the simulated status.
fn simulated_failure(simulated: InsertStatus, status: InsertStatus) -> Result<(), InsertFailure> {
    if simulated == status {
        Err(failure(status, &format!("Simulating '{status:?}' failure")))
    } else {
        Ok(())
    }
}

/// `instantiatePromiseImages` in `addRecording`: the lazy proxies are instantiated before we make
/// any modification to the current command buffer, so a failure here leaves it untouched.
// Port of: src/gpu/graphite/QueueManager.cpp#L146-L166 (chrome/m156)
fn instantiate_lazy_proxies(
    recording: &mut Recording,
    resource_provider: &SharedResourceProvider,
    simulated: InsertStatus,
) -> Result<(), InsertFailure> {
    let mut provider = resource_provider
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if recording.priv_().has_non_volatile_lazy_proxies()
        && !recording
            .priv_()
            .instantiate_non_volatile_lazy_proxies(&mut provider)
    {
        return Err(failure(
            InsertStatus::PromiseImageInstantiationFailed,
            "Non-volatile PromiseImage instantiation has failed",
        ));
    }
    if recording.priv_().has_volatile_lazy_proxies()
        && !recording
            .priv_()
            .instantiate_volatile_lazy_proxies(&mut provider)
    {
        return Err(failure(
            InsertStatus::PromiseImageInstantiationFailed,
            "Volitile PromiseImage instantiation has failed",
        ));
    }
    drop(provider);
    simulated_failure(simulated, InsertStatus::PromiseImageInstantiationFailed)
}

/// `RETURN_FAIL_IF`: fails the insert, failing its finished procs and releasing the volatile
/// lazy proxies of the recording, and logs `message`.
fn fail_insert(
    recording: Option<&mut Recording>,
    callback: Option<&Arc<RefCntedCallback>>,
    status: InsertStatus,
    message: &str,
) -> InsertStatus {
    if let Some(callback) = callback {
        callback.set_failure_result();
    }
    if let Some(recording) = recording {
        recording.priv_().set_failure_result_for_finished_procs();
        recording.priv_().deinstantiate_volatile_lazy_proxies();
    }
    skia_log_e!("{message}");
    status
}

impl QueueManager {
    /// `QueueManager(sharedContext)`.
    // Port of: src/gpu/graphite/QueueManager.cpp#L30-L33 (chrome/m156)
    #[must_use]
    pub fn new(
        shared_is_protected: Protected,
        allow_cpu_sync: bool,
        backend: Box<dyn QueueManagerBackend>,
    ) -> Self {
        Self {
            shared_is_protected,
            allow_cpu_sync,
            current_command_buffer: None,
            outstanding_submissions: VecDeque::new(),
            available_command_buffers: Vec::new(),
            available_protected_command_buffers: Vec::new(),
            last_added_recording_ids: HashMap::new(),
            backend,
        }
    }

    /// `getAvailableCommandBufferList(isProtected)`.
    // Port of: src/gpu/graphite/QueueManager.cpp#L44-L48 (chrome/m156)
    fn available_command_buffer_list(
        &mut self,
        is_protected: Protected,
    ) -> &mut Vec<Box<dyn CommandBuffer>> {
        if is_protected == Protected::No {
            &mut self.available_command_buffers
        } else {
            &mut self.available_protected_command_buffers
        }
    }

    /// `setupCommandBuffer(resourceProvider, isProtected)`.
    // Port of: src/gpu/graphite/QueueManager.cpp#L50-L79 (chrome/m156)
    fn setup_command_buffer(
        &mut self,
        resource_provider: &SharedResourceProvider,
        is_protected: Protected,
    ) -> bool {
        if self.current_command_buffer.is_none() {
            if let Some(mut command_buffer) = self.available_command_buffer_list(is_protected).pop()
                && command_buffer.set_new_command_buffer_resources()
            {
                self.current_command_buffer = Some(command_buffer);
            }
        } else if let Some(current) = &self.current_command_buffer
            && current.is_protected() != is_protected
        {
            // If we're doing things where we are switching between using protected and
            // unprotected command buffers, it is our job to make sure previous work was
            // submitted.
            skia_log_e!(
                "Trying to use a CommandBuffer with protectedness that differs from our \
                 current active command buffer."
            );
            return false;
        }

        if self.current_command_buffer.is_none() {
            self.current_command_buffer =
                self.backend.get_new_command_buffer(resource_provider, is_protected);
        }
        self.current_command_buffer.is_some()
    }

    /// `addRecording(info, context)`: adds the recording's commands to the current command
    /// buffer, and takes its finished procs.
    // Port of: src/gpu/graphite/QueueManager.cpp#L81-L212 (chrome/m156)
    #[doc(alias = "addRecording")]
    #[must_use]
    pub fn add_recording(
        &mut self,
        mut info: InsertRecordingInfo,
        context: &mut dyn ContextPriv,
    ) -> InsertStatus {
        // Configure the callback before validation so that failures are propagated to the finish
        // procs that were registered on `info` as well.
        let callback = if let Some(proc) = info.finished_with_stats_proc.take() {
            Some(RefCntedCallback::make(CallbackProc::ResultWithStats(proc)))
        } else {
            info.finished_proc
                .take()
                .map(|proc| RefCntedCallback::make(CallbackProc::Result(proc)))
        };
        let simulated = info.simulated_status;

        let Some(mut recording) = info.recording.take() else {
            return fail_insert(
                None,
                callback.as_ref(),
                InsertStatus::InvalidRecording,
                "Cannot insert null Recording",
            );
        };
        match self.add_recording_commands(
            &mut recording,
            info.target_translation,
            info.target_clip,
            simulated,
            context,
        ) {
            Ok(()) => {
                if let (Some(callback), Some(command_buffer)) =
                    (callback, self.current_command_buffer.as_deref_mut())
                {
                    command_buffer.add_finished_proc(callback);
                }
                recording.priv_().deinstantiate_volatile_lazy_proxies();
                // If we got here, the simulated status should be Success or it means we missed
                // returning the simulated error earlier.
                debug_assert_eq!(simulated, InsertStatus::Success);
                InsertStatus::Success
            }
            Err((status, message)) => fail_insert(
                Some(&mut recording),
                callback.as_ref(),
                status,
                &message,
            ),
        }
    }

    /// The checks and the work of `addRecording` between the callback and the success return.
    /// On failure, returns the status and the message to log.
    // Port of: src/gpu/graphite/QueueManager.cpp#L101-L186 (chrome/m156)
    fn add_recording_commands(
        &mut self,
        recording: &mut Recording,
        target_translation: IVector,
        target_clip: IRect,
        simulated: InsertStatus,
        context: &mut dyn ContextPriv,
    ) -> Result<(), InsertFailure> {
        self.check_recording_order(recording, simulated)?;

        let resource_provider = context.resource_provider().clone();
        // Technically no commands have been added yet, but if this fails, things are in a bad state
        // so signal the unrecoverable status.
        if !self.setup_command_buffer(&resource_provider, self.shared_is_protected) {
            return Err(failure(
                InsertStatus::AddCommandsFailed,
                "CommandBuffer creation failed",
            ));
        }

        instantiate_lazy_proxies(recording, &resource_provider, simulated)?;

        let Some(command_buffer) = self.current_command_buffer.as_deref_mut() else {
            return Err(failure(
                InsertStatus::AddCommandsFailed,
                "CommandBuffer creation failed",
            ));
        };
        if recording
            .priv_()
            .add_commands(context, command_buffer, None, target_translation, target_clip)
        {
            simulated_failure(simulated, InsertStatus::AddCommandsFailed)?;
            simulated_failure(simulated, InsertStatus::AsyncShaderCompilesFailed)?;
            return Ok(());
        }

        // If the commands failed, iterate over all the used pipelines to see if their async
        // compilation was the reason for failure. Clients that manage pipeline disk caches may
        // want to handle the failure differently than when any other GPU command failed. We will
        // only report the 1st pipeline creation's failure message.
        let mut failure_msg = None;
        let valid_pipelines = recording.priv_().task_list().visit_pipelines(&mut |pipeline| {
            let Some(pipeline) = pipeline else {
                return true;
            };
            if let Some(failure) = pipeline.did_async_compilation_fail() {
                failure_msg = Some(failure);
                return false;
            }
            true
        });
        // We are already definitely going to fail, it's just a matter of which status to return.
        if valid_pipelines {
            return Err(failure(
                InsertStatus::AddCommandsFailed,
                "Adding Recording commands to the CommandBuffer has failed",
            ));
        }
        Err(failure(
            InsertStatus::AsyncShaderCompilesFailed,
            &format!(
                "Async pipeline compiles failed, unable to add Recording commands: {}",
                failure_msg.unwrap_or_default()
            ),
        ))
    }

    /// The order and deferred-target checks at the start of `addRecording`.
    // Port of: src/gpu/graphite/QueueManager.cpp#L101-L144 (chrome/m156)
    fn check_recording_order(
        &mut self,
        recording: &mut Recording,
        simulated: InsertStatus,
    ) -> Result<(), InsertFailure> {
        // Recordings from a Recorder that requires ordered recordings will have a valid recorder
        // ID. Recordings that don't have any required order are assigned SK_InvalidID.
        let recorder_id = recording.priv_().recorder_id();
        let unique_id = recording.priv_().unique_id();
        if recorder_id != INVALID_GEN_ID {
            if let Some(&last) = self.last_added_recording_ids.get(&recorder_id)
                && unique_id != last + 1
            {
                return Err(failure(
                    InsertStatus::OutOfOrderRecording,
                    "Recordings are expected to be replayed in order",
                ));
            }
            // Note the new Recording ID.
            self.last_added_recording_ids.insert(recorder_id, unique_id);
        }

        // `fTargetSurface` is not ported (`Surface` is G10d), so a deferred replay target can never
        // be set up here: this is the failure Skia reports when no surface is provided.
        if recording.priv_().deferred_target_proxy().is_some() {
            return Err(failure(
                InsertStatus::PromiseImageInstantiationFailed,
                "No surface provided to instantiate deferred replay target",
            ));
        }

        simulated_failure(simulated, InsertStatus::InvalidRecording)
    }

    /// `addTask(task, context, isProtected)`: adds a task's commands outside of a recording.
    // Port of: src/gpu/graphite/QueueManager.cpp#L214-L229 (chrome/m156)
    #[doc(alias = "addTask")]
    #[must_use]
    pub fn add_task(
        &mut self,
        task: &mut Task,
        context: &mut dyn ContextPriv,
        is_protected: Protected,
    ) -> bool {
        let resource_provider = context.resource_provider().clone();
        if !self.setup_command_buffer(&resource_provider, is_protected) {
            skia_log_e!("CommandBuffer creation failed");
            return false;
        }
        let Some(command_buffer) = self.current_command_buffer.as_deref_mut() else {
            skia_log_e!("CommandBuffer creation failed");
            return false;
        };
        if task.add_commands(context, command_buffer, &ReplayTargetData::default()) == Status::Fail
        {
            skia_log_e!("Adding Task commands to the CommandBuffer has failed");
            return false;
        }
        true
    }

    /// `addFinishInfo(info, resourceProvider, buffersToAsyncMap)`: a finished proc with no
    /// recording (`Context::insertFinishInfo`).
    // Port of: src/gpu/graphite/QueueManager.cpp#L231-L252 (chrome/m156)
    #[doc(alias = "addFinishInfo")]
    #[must_use]
    pub fn add_finish_info(
        &mut self,
        info: InsertFinishInfo,
        resource_provider: &SharedResourceProvider,
        buffers_to_async_map: &[ResourceRef<Buffer>],
    ) -> bool {
        let callback = info
            .finished_proc
            .map(|proc| RefCntedCallback::make(CallbackProc::Result(proc)));
        if !self.setup_command_buffer(resource_provider, self.shared_is_protected) {
            if let Some(callback) = callback {
                callback.set_failure_result();
            }
            skia_log_e!("CommandBuffer creation failed");
            return false;
        }
        if let Some(command_buffer) = self.current_command_buffer.as_deref_mut() {
            if let Some(callback) = callback {
                command_buffer.add_finished_proc(callback);
            }
            command_buffer.add_buffers_to_async_map_on_submit(buffers_to_async_map);
        }
        true
    }

    /// `submitToGpu(submitInfo)`: submits the current command buffer, if any.
    // Port of: src/gpu/graphite/QueueManager.cpp#L254-L290 (chrome/m156)
    #[doc(alias = "submitToGpu")]
    #[must_use]
    pub fn submit_to_gpu(&mut self, mut submit_info: SubmitInfo) -> bool {
        let callback = submit_info
            .finished_proc
            .take()
            .map(|proc| RefCntedCallback::make(CallbackProc::Result(proc)));

        let Some(mut command_buffer) = self.current_command_buffer.take() else {
            // If a finish proc was provided, attach it to the most recent outstanding submission,
            // or let it fire immediately if the GPU is idle (when callback goes out of scope).
            if let (Some(callback), Some(back)) = (callback, self.outstanding_submissions.back_mut()) {
                    back.add_finished_proc(callback);
                }
            // With no active command buffer the submit is a no-op and succeeds.
            return true;
        };

        if let Some(callback) = callback {
            command_buffer.add_finished_proc(callback);
        }
        let Some(submission) = self.backend.on_submit_to_gpu(command_buffer, &submit_info) else {
            return false;
        };
        self.outstanding_submissions.push_back(submission);
        true
    }

    /// `hasUnfinishedGpuWork()`.
    // Port of: src/gpu/graphite/QueueManager.cpp#L292-L292 (chrome/m156)
    #[doc(alias = "hasUnfinishedGpuWork")]
    #[must_use]
    pub fn has_unfinished_gpu_work(&self) -> bool {
        !self.outstanding_submissions.is_empty()
    }

    /// `hasPendingGPUWork()`: true if there is a current command buffer, even one without work
    /// (a recording can be inserted just to track its finished proc).
    // Port of: src/gpu/graphite/QueueManager.cpp#L294-L300 (chrome/m156)
    #[doc(alias = "hasPendingGPUWork")]
    #[must_use]
    pub fn has_pending_gpu_work(&self) -> bool {
        self.current_command_buffer.is_some()
    }

    /// `checkForFinishedWork(sync)`: retires the submissions the GPU has finished, in order. With
    /// [`SyncToCpu::Yes`] it first waits for the newest one.
    // Port of: src/gpu/graphite/QueueManager.cpp#L302-L329 (chrome/m156)
    #[doc(alias = "checkForFinishedWork")]
    pub fn check_for_finished_work(&mut self, sync: SyncToCpu) {
        if sync == SyncToCpu::Yes {
            debug_assert!(self.allow_cpu_sync);
            // Wait for the last submission to finish.
            let backend = &self.backend;
            let tick = || backend.tick();
            if let Some(back) = self.outstanding_submissions.back() {
                back.wait_until_finished(&tick);
            }
        }

        // The work submissions are in order from oldest to newest, so we start at the front to
        // check if they have finished. If so we pop it off and move onto the next. Repeat till we
        // find a submission that has not finished yet (all others afterwards are also guaranteed
        // to not have finished).
        while let Some(front) = self.outstanding_submissions.front() {
            if !front.is_finished() {
                break;
            }
            let Some(submission) = self.outstanding_submissions.pop_front() else {
                break;
            };
            if let Some(command_buffer) = submission.retire() {
                self.return_command_buffer(command_buffer);
            }
        }
        debug_assert!(sync == SyncToCpu::No || self.outstanding_submissions.is_empty());
    }

    /// `returnCommandBuffer(commandBuffer)`: pools a command buffer for reuse.
    // Port of: src/gpu/graphite/QueueManager.cpp#L331-L335 (chrome/m156)
    #[doc(alias = "returnCommandBuffer")]
    pub fn return_command_buffer(&mut self, command_buffer: Box<dyn CommandBuffer>) {
        let is_protected = command_buffer.is_protected();
        self.available_command_buffer_list(is_protected)
            .push(command_buffer);
    }

    /// `addUploadBufferManagerRefs(uploadManager, resourceProvider)`: hands the upload buffers
    /// to the current (non-protected) command buffer.
    // Port of: src/gpu/graphite/QueueManager.cpp#L337-L343 (chrome/m156)
    #[doc(alias = "addUploadBufferManagerRefs")]
    pub fn add_upload_buffer_manager_refs(
        &mut self,
        upload_manager: &mut UploadBufferManager,
        resource_provider: &SharedResourceProvider,
    ) {
        self.setup_command_buffer(resource_provider, Protected::No);
        if let Some(command_buffer) = self.current_command_buffer.as_deref_mut() {
            upload_manager.transfer_to_command_buffer(command_buffer);
        }
    }
}

impl Drop for QueueManager {
    // Port of: src/gpu/graphite/QueueManager.cpp#L36-L42 (chrome/m156)
    fn drop(&mut self) {
        if self.allow_cpu_sync {
            self.check_for_finished_work(SyncToCpu::Yes);
        } else {
            assert!(
                self.outstanding_submissions.is_empty(),
                "When ContextOptions::fNeverYieldToWebGPU is specified all GPU work must be \
                 finished before destroying Context."
            );
        }
    }
}
