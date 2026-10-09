// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnGraphiteUtils.cpp (ContextFactory::MakeDawn),
//                   include/gpu/graphite/Context.h (the part that exists before G9b)

//! `ContextFactory::MakeDawn` on wgpu: [`make_context`].
//!
//! [`WgpuContext`] is Skia's `Context` on wgpu. It owns the shared context, the context's resource
//! provider (`Context::fResourceProvider`) and the queue manager, and it makes recorders and
//! inserts and submits recordings, and it finishes its initialization (the dynamic samplers, the
//! static buffers and the renderer provider). Not yet here: the pipeline manager (G9b step 3),
//! `readPixels` and the async rescale-and-read (they need images and surfaces, G10d, and the
//! readback copy, G11c).

use std::sync::{Arc, Mutex, PoisonError};

use crate::gpu::gpu_types::{BackendApi, Protected};
use crate::gpu::sk_log::{skia_log_e, skia_log_w};
use crate::graphite::backend_texture::BackendTexture;
use crate::graphite::buffer::Buffer;
use crate::graphite::buffer_manager::{
    StaticBufferHost, StaticBufferManager, StaticFinishResult, StaticVertexCopyRanges,
};
use crate::graphite::caps::Caps;
use crate::graphite::client_mapped_buffer_manager::{ClientMappedBufferManager, ContextId};
use crate::graphite::context_options::ContextOptions;
use crate::graphite::context_priv::{ContextPriv, SharedResourceProvider};
use crate::graphite::global_cache::GlobalCache;
use crate::graphite::graphite_types::{InsertRecordingInfo, InsertStatus, SubmitInfo, SyncToCpu};
use crate::graphite::queue_manager::QueueManager;
use crate::graphite::recorder::{Recorder, RecorderOptions, RecorderSharedContext};
use crate::graphite::renderer_provider::RendererProvider;
use crate::graphite::resource::{Resource, ResourceRef};
use crate::graphite::task::TaskRef;
use crate::graphite::upload_buffer_manager::UploadBufferManager;
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
    /// `fContextID`.
    context_id: ContextId,
    /// `fMappedBufferManager`: buffers handed to clients while still mapped.
    mapped_buffer_manager: ClientMappedBufferManager,
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
    // `Context::finishInitialization()` is not called yet: its static-buffer copies need copy
    // recording on the wgpu command buffer (G11c), so calling it here would fail on every device.
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
        let context_id = ContextId::next();
        Self {
            shared_context,
            resource_provider,
            queue_manager,
            options: options.clone(),
            context_id,
            mapped_buffer_manager: ClientMappedBufferManager::new(context_id),
        }
    }

    /// `contextID()`.
    #[must_use]
    pub fn context_id(&self) -> ContextId {
        self.context_id
    }

    /// `finishInitialization()`: creates the dynamic samplers, the static buffers (submitting
    /// their copies when there are any) and the renderer provider. Returns `false` if any of them
    /// fails, in which case the context must not be used.
    // Port of: src/gpu/graphite/Context.cpp#L185-L210 (chrome/m156)
    #[doc(alias = "finishInitialization")]
    #[must_use]
    pub fn finish_initialization(&mut self) -> bool {
        let shared = Arc::clone(&self.shared_context);
        let base = shared.base();
        let caps: &Arc<WgpuCaps> = shared.caps();
        {
            let mut provider = self
                .resource_provider
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if !base
                .global_cache()
                .initialize_dynamic_samplers(&mut provider, &**caps)
            {
                return false;
            }
        }

        let mut buffer_manager = StaticBufferManager::new(self.resource_provider.clone(), &**caps);
        let renderers = RendererProvider::new(
            caps.resource_binding_requirements().uniform_buffer_layout,
            caps.shader_caps().infinity_support,
            &mut buffer_manager,
        );

        let result = {
            let mut host = FinishInitializationHost {
                queue_manager: &mut self.queue_manager,
                context: ContextPrivView {
                    shared_context: &shared,
                    resource_provider: &self.resource_provider,
                },
                global_cache: base.global_cache(),
            };
            buffer_manager.finalize(&mut host)
        };
        match result {
            // If something went wrong filling out the static vertex buffers, any Renderer that
            // would use it will draw incorrectly, so it is better to fail the Context creation.
            StaticFinishResult::Failure => return false,
            StaticFinishResult::Success => {
                if !self.queue_manager.submit_to_gpu(SubmitInfo::default()) {
                    skia_log_w!("Failed to submit initial command buffer for Context creation.\n");
                    return false;
                }
            }
            // No static buffers were needed, so there is nothing to submit.
            StaticFinishResult::NoWork => {}
        }
        base.set_renderer_provider(renderers);
        true
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
        self.mapped_buffer_manager.process();
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

/// What `StaticBufferManager::finalize()` needs from the `Context` (its resource provider and
/// `ContextPriv`), its `QueueManager` and its `GlobalCache`.
// Port of: src/gpu/graphite/Context.cpp#L193-L197 (the `Context*` and `QueueManager*` arguments
// of `StaticBufferManager::finalize`, chrome/m156)
struct FinishInitializationHost<'a> {
    queue_manager: &'a mut QueueManager,
    context: ContextPrivView<'a>,
    global_cache: &'a GlobalCache,
}

impl StaticBufferHost for FinishInitializationHost<'_> {
    // Port of: src/gpu/graphite/QueueManager.cpp#L384-L392 (called from BufferManager.cpp#L697)
    fn add_upload_buffer_manager_refs(&mut self, upload_manager: &mut UploadBufferManager) {
        self.queue_manager
            .add_upload_buffer_manager_refs(upload_manager, self.context.resource_provider);
    }

    // Port of: src/gpu/graphite/QueueManager.cpp#L247-L267 (`addTask`, with `Protected::kNo`)
    fn add_task(&mut self, task: &TaskRef, is_protected: Protected) -> bool {
        let mut task = task.lock();
        self.queue_manager
            .add_task(&mut task, &mut self.context, is_protected)
    }

    // Port of: src/gpu/graphite/GlobalCache.cpp#L519-L522 (`addStaticResource`)
    fn add_static_resource(&mut self, buffer: ResourceRef<Buffer>) {
        self.global_cache.add_static_resource(buffer.into_any());
    }

    // Port of: src/gpu/graphite/GlobalCache.cpp#L553-L558 (`testingOnly_SetStaticVertexInfo`)
    fn testing_only_set_static_vertex_info(
        &mut self,
        ranges: Vec<StaticVertexCopyRanges>,
        buffer: Option<Arc<Resource<Buffer>>>,
    ) {
        self.global_cache
            .testing_only_set_static_vertex_info(ranges, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphite::wgpu::noop_backend_context;

    #[test]
    fn contexts_get_distinct_valid_ids_and_finished_work_is_checked() {
        let options = ContextOptions::default();
        let mut first = make_context(&noop_backend_context(), &options).unwrap();
        let second = make_context(&noop_backend_context(), &options).unwrap();
        assert!(first.context_id().is_valid());
        assert_ne!(first.context_id(), second.context_id());

        first.check_for_finished_work(SyncToCpu::No);
        assert_eq!(first.mapped_buffer_manager.num_client_held_buffers(), 0);
    }
}
