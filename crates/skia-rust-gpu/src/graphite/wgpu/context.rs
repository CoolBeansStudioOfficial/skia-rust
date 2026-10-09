// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnGraphiteUtils.cpp (ContextFactory::MakeDawn),
//                   include/gpu/graphite/Context.h (the part that exists before G9b)

//! `ContextFactory::MakeDawn` on wgpu: [`make_context`].
//!
//! [`WgpuContext`] is Skia's `Context` on wgpu. It owns the shared context, the context's resource
//! provider (`Context::fResourceProvider`) and the queue manager, and it makes recorders and
//! inserts and submits recordings. Not yet here: the global cache and the pipeline manager (G9b
//! steps 6 and 7; `finishInitialization` needs the global cache), `readPixels` and the async
//! rescale-and-read (they need images and surfaces, G10d, and the readback copy, G11c).

use std::sync::{Arc, Mutex, PoisonError};

use crate::gpu::gpu_types::{BackendApi, Protected};
use crate::gpu::sk_log::skia_log_e;
use crate::graphite::backend_texture::BackendTexture;
use crate::graphite::caps::Caps;
use crate::graphite::context_options::ContextOptions;
use crate::graphite::context_priv::{ContextPriv, SharedResourceProvider};
use crate::graphite::graphite_types::{InsertRecordingInfo, InsertStatus, SubmitInfo, SyncToCpu};
use crate::graphite::queue_manager::QueueManager;
use crate::graphite::recorder::{Recorder, RecorderOptions, RecorderSharedContext};
use crate::graphite::wgpu::caps::WgpuCaps;
use crate::graphite::wgpu::queue_manager::WgpuQueueManagerBackend;
use crate::graphite::wgpu::shared_context::{WgpuBackendContext, WgpuSharedContext};

/// The wgpu `Context`: the shared context, the resource provider and the queue manager.
// Port of: include/gpu/graphite/Context.h#L45-L74 (chrome/m156)
#[doc(alias = "Context")]
#[derive(Debug)]
pub struct WgpuContext {
    shared_context: Arc<WgpuSharedContext>,
    resource_provider: SharedResourceProvider,
    /// `fQueueManager`.
    queue_manager: QueueManager,
    options: ContextOptions,
}

/// The part of a [`WgpuContext`] that the queue manager reads (`Context*` in Skia): the caps and
/// the context's resource provider. Borrowed from the context's other fields, so the queue manager
/// can be used at the same time.
struct ContextPrivView<'a> {
    shared_context: &'a WgpuSharedContext,
    resource_provider: &'a SharedResourceProvider,
}

impl ContextPriv for ContextPrivView<'_> {
    fn caps(&self) -> &dyn Caps {
        &**self.shared_context.caps()
    }

    fn resource_provider(&self) -> &SharedResourceProvider {
        self.resource_provider
    }
}

/// `SK_InvalidGenID`: the recorder id of the context's own resource provider.
const INVALID_GEN_ID: u32 = 0;

/// Creates a Graphite context on a wgpu device (`ContextFactory::MakeDawn`), or `None` if the
/// shared objects cannot be created.
// Port of: src/gpu/graphite/dawn/DawnGraphiteUtils.cpp#L27-L46 (chrome/m156)
#[doc(alias = "MakeDawn")]
#[must_use]
pub fn make_context(
    backend_context: &WgpuBackendContext,
    options: &ContextOptions,
) -> Option<WgpuContext> {
    let shared_context = WgpuSharedContext::make(backend_context, options)?;
    Some(WgpuContext::new(shared_context, options))
}

impl WgpuContext {
    /// `Context(sharedContext, queueManager, options)`.
    // Port of: src/gpu/graphite/Context.cpp#L69-L80 (chrome/m156)
    #[must_use]
    pub fn new(shared_context: Arc<WgpuSharedContext>, options: &ContextOptions) -> Self {
        let resource_provider = Arc::new(Mutex::new(
            shared_context.make_resource_provider(INVALID_GEN_ID, options.gpu_budget_in_bytes),
        ));
        // `fQueueManager` is made by the backend's `ContextFactory`, which passes the queue
        // manager to the context. The wgpu queue manager needs the device and the queue.
        let queue_manager = QueueManager::new(
            shared_context.is_protected(),
            shared_context.caps().allow_cpu_sync(),
            Box::new(WgpuQueueManagerBackend::new(
                shared_context.device().clone(),
                shared_context.queue().clone(),
            )),
        );
        // Port of: src/gpu/graphite/Context.cpp#L145-L147 (chrome/m156)
        shared_context.base().global_cache().set_pipeline_callback(
            options.pipeline_caching_callback.clone(),
            options.pipeline_callback.clone(),
        );
        Self {
            shared_context,
            resource_provider,
            queue_manager,
            options: options.clone(),
        }
    }

    /// `insertRecording(info)`: adds a recording's commands to the current command buffer.
    // Port of: src/gpu/graphite/Context.cpp#L255-L267 (chrome/m156)
    #[doc(alias = "insertRecording")]
    #[must_use]
    pub fn insert_recording(&mut self, info: InsertRecordingInfo<'_>) -> InsertStatus {
        let mut view = ContextPrivView {
            shared_context: &self.shared_context,
            resource_provider: &self.resource_provider,
        };
        self.queue_manager.add_recording(info, &mut view)
    }

    /// `submit(submitInfo)`: submits the current command buffer, then checks for finished work.
    /// With [`SyncToCpu::Yes`] on a context that allows CPU sync, waits for the submission.
    // Port of: src/gpu/graphite/Context.cpp#L269-L280 (chrome/m156)
    #[must_use]
    pub fn submit(&mut self, mut submit_info: SubmitInfo) -> bool {
        if submit_info.sync == SyncToCpu::Yes && !self.shared_context.caps().allow_cpu_sync() {
            skia_log_e!(
                "SyncToCpu::kYes not supported with ContextOptions::fNeverYieldToWebGPU. The \
                 parameter is ignored and no synchronization will occur."
            );
            submit_info.sync = SyncToCpu::No;
        }
        let sync = submit_info.sync;
        let success = self.queue_manager.submit_to_gpu(submit_info);
        self.check_for_finished_work(sync);
        success
    }

    /// `hasUnfinishedGpuWork()`.
    // Port of: src/gpu/graphite/Context.cpp#L282-L282 (chrome/m156)
    #[must_use]
    pub fn has_unfinished_gpu_work(&self) -> bool {
        self.queue_manager.has_unfinished_gpu_work()
    }

    /// `hasPendingGPUWork()`.
    // Port of: src/gpu/graphite/Context.cpp#L284-L284 (chrome/m156)
    #[must_use]
    pub fn has_pending_gpu_work(&self) -> bool {
        self.queue_manager.has_pending_gpu_work()
    }

    /// `checkForFinishedWork(syncToCpu)`: retires finished submissions and returns the cached
    /// resources that have been released.
    // Port of: src/gpu/graphite/Context.cpp#L912-L922 (chrome/m156)
    pub fn check_for_finished_work(&mut self, sync: SyncToCpu) {
        self.queue_manager.check_for_finished_work(sync);
        // Process the return queue periodically to make sure it doesn't get too big.
        self.resource_provider
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .force_process_returned_resources();
        self.shared_context
            .base()
            .force_process_returned_resources();
    }

    /// `checkAsyncWorkCompletion()`: polls the device and retires finished work, without waiting.
    // Port of: src/gpu/graphite/Context.cpp#L924-L926 (chrome/m156)
    #[doc(alias = "checkAsyncWorkCompletion")]
    pub fn check_async_work_completion(&mut self) {
        self.check_for_finished_work(SyncToCpu::No);
    }

    /// `backend()`.
    #[must_use]
    pub fn backend(&self) -> BackendApi {
        BackendApi::Dawn
    }

    /// `priv().caps()` as the concrete wgpu caps.
    #[doc(alias = "caps")]
    #[must_use]
    pub fn wgpu_caps(&self) -> &WgpuCaps {
        self.shared_context.caps()
    }

    /// The options the context was created with.
    #[must_use]
    pub fn options(&self) -> &ContextOptions {
        &self.options
    }

    /// The shared context.
    #[must_use]
    pub fn shared_context(&self) -> &Arc<WgpuSharedContext> {
        &self.shared_context
    }

    /// `maxBudgetedBytes()`: the budget of the context's resource cache.
    // Port of: src/gpu/graphite/Context.cpp#L974-L978 (chrome/m156)
    #[doc(alias = "maxBudgetedBytes")]
    #[must_use]
    pub fn max_budgeted_bytes(&self) -> usize {
        self.resource_provider
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_resource_cache_limit()
    }

    /// `setMaxBudgetedBytes()`.
    // Port of: src/gpu/graphite/Context.cpp#L980-L983 (chrome/m156)
    #[doc(alias = "setMaxBudgetedBytes")]
    pub fn set_max_budgeted_bytes(&self, bytes: usize) {
        self.resource_provider
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_resource_cache_limit(bytes);
    }

    /// `supportsProtectedContent()`.
    // Port of: src/gpu/graphite/Context.cpp#L1001-L1003 (chrome/m156)
    #[doc(alias = "supportsProtectedContent")]
    #[must_use]
    pub fn supports_protected_content(&self) -> bool {
        self.shared_context.is_protected() == Protected::Yes
    }

    /// `deleteBackendTexture()`: deleting is safe from the context or any recorder.
    // Port of: src/gpu/graphite/Context.cpp#L928-L935 (chrome/m156)
    #[doc(alias = "deleteBackendTexture")]
    pub fn delete_backend_texture(&self, texture: &BackendTexture) {
        if !texture.is_valid() || texture.backend() != self.backend() {
            return;
        }
        self.resource_provider
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .delete_backend_texture(texture);
    }

    /// `makeRecorder()`.
    // Port of: src/gpu/graphite/Context.cpp#L137-L151 (chrome/m156)
    #[doc(alias = "makeRecorder")]
    #[must_use]
    pub fn make_recorder(&self, options: Option<&RecorderOptions>) -> Recorder {
        let default_options = RecorderOptions::default();
        Recorder::new(
            self.shared_context.clone(),
            options.unwrap_or(&default_options),
            None,
        )
    }
}

impl ContextPriv for WgpuContext {
    fn caps(&self) -> &dyn Caps {
        &**self.shared_context.caps()
    }

    fn resource_provider(&self) -> &SharedResourceProvider {
        &self.resource_provider
    }
}
