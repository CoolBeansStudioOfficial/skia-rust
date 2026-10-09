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

use crate::gpu::backing_fit::BackingFit;
use crate::gpu::gpu_types::{BackendApi, Budgeted, Mipmapped, Origin, Protected};
use crate::gpu::sk_log::{skia_log_e, skia_log_w};
use crate::graphite::async_read::{
    AsyncReadParams, AsyncReadResult, PixelTransferResult, SharedClientMappedBufferManager,
    lock_manager,
};
use crate::graphite::backend_texture::BackendTexture;
use crate::graphite::buffer::Buffer;
use crate::graphite::caps::Caps;
use crate::graphite::client_mapped_buffer_manager::{ClientMappedBufferManager, ContextId};
use crate::graphite::context_options::ContextOptions;
use crate::graphite::context_priv::{ContextPriv, SharedResourceProvider};
use crate::graphite::graphite_types::{
    InsertFinishInfo, InsertRecordingInfo, InsertStatus, SubmitInfo, SyncToCpu,
};
use crate::graphite::image_graphite::Image;
use crate::graphite::queue_manager::QueueManager;
use crate::graphite::recorder::{Recorder, RecorderOptions, RecorderSharedContext};
use crate::graphite::resource::ResourceRef;
use crate::graphite::resource_types::{AccessPattern, BufferType};
use crate::graphite::surface_graphite::Surface;
use crate::graphite::task::copy_task::CopyTextureToBufferTask;
use crate::graphite::task::synchronize_to_cpu_task::SynchronizeToCpuTask;
use crate::graphite::texture_format::texture_format_bytes_per_block;
use crate::graphite::texture_format_xfer_fn::TextureFormatXferFn;
use crate::graphite::texture_info::texture_info_priv;
use crate::graphite::texture_proxy_view::TextureProxyView;
use crate::graphite::texture_utils::{as_view, copy_as_draw};
use crate::graphite::wgpu::caps::WgpuCaps;
use crate::graphite::wgpu::queue_manager::WgpuQueueManagerBackend;
use crate::graphite::wgpu::shared_context::{WgpuBackendContext, WgpuSharedContext};
use skia_rust_core::color_space_xform_steps::ColorSpaceXformSteps;
use skia_rust_core::image::Image as CoreImage;
use skia_rust_core::image_info::{ColorInfo, ImageInfo};
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::{Contains, IRect};

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
    mapped_buffer_manager: SharedClientMappedBufferManager,
}

impl Drop for WgpuContext {
    /// `Context::~Context()` shuts the pipeline manager down: the compilation tasks it queued
    /// hold the shared context weakly, and are waited for here while it is certainly alive.
    // Port of: src/gpu/graphite/Context.cpp#L156-L172 (chrome/m156)
    fn drop(&mut self) {
        self.shared_context.base().pipeline_manager().shut_down();
    }
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
    let mut context = WgpuContext::new(shared_context, options);
    if !context.finish_initialization() {
        return None;
    }
    Some(context)
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
            Box::new(WgpuQueueManagerBackend::new(Arc::clone(&shared_context))),
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
            mapped_buffer_manager: Arc::new(Mutex::new(ClientMappedBufferManager::new(context_id))),
        }
    }

    /// `contextID()`.
    #[must_use]
    pub fn context_id(&self) -> ContextId {
        self.context_id
    }

    /// `finishInitialization()`: creates the dynamic samplers, makes the shared context's
    /// renderer provider (the one provider: recorders reach it through the shared context too)
    /// with its static buffers, and submits the copies that fill them. Returns `false` if any of
    /// them fails, in which case the context must not be used.
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

        // `RendererProvider`'s constructor fills the static buffer manager, and the shared
        // context's lazy provider finalizes it: the copy tasks and the transfer buffers they
        // read are collected there (`StaticBufferManager::finalize()`'s `QueueManager` calls).
        let tasks = shared.take_static_buffer_tasks();
        let upload_buffers = shared.take_static_upload_buffers();
        // If something went wrong filling out the static vertex buffers, any Renderer that
        // would use it will draw incorrectly, so it is better to fail the Context creation.
        if shared.static_buffers_failed() {
            return false;
        }
        if tasks.is_empty() {
            // No static buffers were needed, so there is nothing to submit.
            return true;
        }

        // `queueManager->addUploadBufferManagerRefs(...)`.
        self.queue_manager.add_resource_refs(
            upload_buffers
                .into_iter()
                .map(ResourceRef::into_any)
                .collect(),
            &self.resource_provider,
        );
        let mut context = ContextPrivView {
            shared_context: &shared,
            resource_provider: &self.resource_provider,
        };
        for task in &tasks {
            let mut task = task.lock();
            if !self
                .queue_manager
                .add_task(&mut task, &mut context, Protected::No)
            {
                return false;
            }
        }
        if !self.queue_manager.submit_to_gpu(SubmitInfo::default()) {
            skia_log_w!("Failed to submit initial command buffer for Context creation.\n");
            return false;
        }
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
        lock_manager(&self.mapped_buffer_manager).process();
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

/// Where `read_pixels` gets its result: `Some` once the callback ran, with the pixels if it read.
type ReadSlot = Arc<Mutex<Option<Option<(Vec<u8>, usize)>>>>;

/// `Context::readPixels` and its machinery, as far as it is reachable without images and
/// surfaces (G10d): reading a texture proxy view back.
impl WgpuContext {
    /// `makeInternalRecorder()`: a short-lived recorder that shares the context's resource
    /// provider and does not require ordered recordings.
    // Port of: src/gpu/graphite/Context.cpp#L239-L253 (chrome/m156)
    #[doc(alias = "makeInternalRecorder")]
    #[must_use]
    pub fn make_internal_recorder(&self) -> Recorder {
        let options = RecorderOptions {
            require_ordered_recordings: Some(false),
            ..RecorderOptions::default()
        };
        Recorder::new(
            self.shared_context.clone(),
            &options,
            Some(Arc::clone(&self.resource_provider)),
        )
    }

    /// `asyncReadPixels(recorder, params)`: reads back the region of `params.src`, calling
    /// `params.callback` with the pixels converted to `params.dst_image_info`, or with `None`
    /// if the read failed.
    ///
    /// A source that is not copyable, is bottom-left, or needs a transfer function is first drawn
    /// into a copyable texture (`CopyAsDraw`); when that draw is only optional and cannot be made,
    /// the conversion is done on the CPU by the transfer's converter.
    // Port of: src/gpu/graphite/Context.cpp#L413-L482 (chrome/m156)
    #[doc(alias = "asyncReadPixels")]
    pub fn async_read_pixels(&mut self, mut recorder: Option<Recorder>, params: AsyncReadParams) {
        debug_assert_eq!(
            params.src_rect.width(),
            params.dst_image_info.dimensions().width
        );
        debug_assert_eq!(
            params.src_rect.height(),
            params.dst_image_info.dimensions().height
        );
        // All paths to here are already validated.
        debug_assert!(params.validate());

        let caps = Arc::clone(self.shared_context.caps());
        let mut view = params.src.clone();
        let mut src_color_info = params.src_color_info.clone();
        let dst_color_info = params.dst_image_info.color_info().clone();
        let mut src_rect = params.src_rect;

        let make_xfer_fn = |view: &TextureProxyView, src_color_info: &ColorInfo| {
            let proxy = view.proxy()?;
            let format = texture_info_priv::view_format(proxy.texture_info());
            let cs_steps = ColorSpaceXformSteps::new(
                src_color_info.color_space_ref(),
                src_color_info.alpha_type(),
                dst_color_info.color_space_ref(),
                dst_color_info.alpha_type(),
            );
            TextureFormatXferFn::make_gpu_to_cpu(
                format,
                view.swizzle(),
                &cs_steps,
                dst_color_info.color_type(),
            )
        };
        let mut xfer_fn = make_xfer_fn(&view, &src_color_info);

        let has_view = view.proxy().is_some();
        let require_conversion = view
            .proxy()
            .is_none_or(|proxy| !Caps::is_copyable_src(&*caps, proxy.texture_info()));
        // Flip if the image is bottom left, and try the GPU conversion if the transfer function is
        // not identity.
        let try_gpu_conversion = has_view
            && (view.origin() == Origin::BottomLeft
                || xfer_fn.as_ref().is_some_and(|xfer| !xfer.is_identity()));

        if require_conversion || try_gpu_conversion {
            let recorder = recorder.get_or_insert_with(|| self.make_internal_recorder());
            let src_image = Image::new(view.clone(), &src_color_info).into_core();
            let converted = copy_as_draw(
                recorder,
                None,
                &src_image,
                src_rect,
                &dst_color_info,
                Budgeted::Yes,
                Mipmapped::No,
                BackingFit::Approx,
                "AsyncReadPixelsConversionTexture",
            );
            if let Some(converted) = converted {
                view = as_view(Some(&converted));
                src_color_info = converted.image_info().color_info().clone();
                src_rect = IRect::from_size(src_rect.size());

                // The GPU draw converted the pixels to the converted image's color info (target
                // color space and alpha type). The backing texture format may not natively match
                // the destination's channel ordering, so query a transfer function for the
                // remaining format and swizzle conversion.
                xfer_fn = make_xfer_fn(&view, &src_color_info);
            } else if require_conversion {
                skia_log_w!(
                    "AsyncRead failed because copy-as-drawing into a readable format failed"
                );
                return params.fail();
            }
            // else it couldn't be rendered so apply the GPU-optional conversions on the CPU
            // instead
        }

        let Some(xfer_fn) = xfer_fn else {
            return params.fail();
        };

        let new_params = AsyncReadParams {
            src: view,
            src_color_info,
            src_rect,
            dst_image_info: params.dst_image_info,
            callback: params.callback,
        };
        self.async_read_texture(recorder, new_params, &xfer_fn);
    }

    /// `asyncReadTexture(recorder, params, xferFn)`.
    // Port of: src/gpu/graphite/Context.cpp#L484-L520 (chrome/m156)
    #[doc(alias = "asyncReadTexture")]
    pub fn async_read_texture(
        &mut self,
        mut recorder: Option<Recorder>,
        params: AsyncReadParams,
        xfer_fn: &TextureFormatXferFn,
    ) {
        debug_assert_eq!(
            params.src_rect.width(),
            params.dst_image_info.dimensions().width
        );
        debug_assert_eq!(
            params.src_rect.height(),
            params.dst_image_info.dimensions().height
        );

        // We can get here directly from surface or testing-only read pixels, so re-validate
        if !params.validate() {
            return params.fail();
        }

        let transfer_result = self.transfer_pixels(
            recorder.as_mut(),
            &params.src,
            params.dst_image_info.color_info(),
            params.src_rect,
            xfer_fn,
        );

        if transfer_result.transfer_buffer.is_none() {
            // TODO: try to do a synchronous readPixels instead
            return params.fail();
        }

        self.finalize_async_read_pixels(recorder, vec![transfer_result], params.callback);
    }

    /// `finalizeAsyncReadPixels(recorder, transferResults, callback)`.
    // Port of: src/gpu/graphite/Context.cpp#L732-L810 (chrome/m156)
    #[doc(alias = "finalizeAsyncReadPixels")]
    pub fn finalize_async_read_pixels(
        &mut self,
        recorder: Option<Recorder>,
        transfer_results: Vec<PixelTransferResult>,
        callback: crate::graphite::async_read::ReadPixelsCallback,
    ) {
        // If the async readback work required a Recorder, insert the recording with all of the
        // accumulated work (which includes any copies). Otherwise, for pure copy readbacks,
        // transferPixels() already added the tasks directly to the QueueManager.
        if let Some(mut recorder) = recorder {
            let Some(mut recording) = recorder.snap() else {
                callback(None);
                return;
            };
            if self.insert_recording(InsertRecordingInfo::new(&mut recording))
                != InsertStatus::Success
            {
                callback(None);
                return;
            }
        }

        // Set up the finish context and add the transfer commands to the queue.
        let buffers_to_async_map: Vec<ResourceRef<Buffer>> =
            if self.shared_context.caps().buffer_maps_are_async() {
                transfer_results
                    .iter()
                    .filter_map(|result| result.transfer_buffer.clone())
                    .collect()
            } else {
                Vec::new()
            };
        let manager = Arc::clone(&self.mapped_buffer_manager);
        let info = InsertFinishInfo::new(Box::new(move |status| {
            use crate::gpu::gpu_types::CallbackResult;
            let (owner_id, sender) = {
                let manager = lock_manager(&manager);
                (manager.owner_id(), manager.sender())
            };
            let mut result =
                (status == CallbackResult::Success).then(|| AsyncReadResult::new(owner_id, sender));
            for r in &transfer_results {
                let Some(buffer) = &r.transfer_buffer else {
                    break;
                };
                if let Some(read) = &mut result
                    && !read.add_transfer_result(
                        r,
                        r.size,
                        r.row_bytes,
                        &mut lock_manager(&manager),
                    )
                {
                    result = None;
                }
                // If we didn't get this buffer into the mapped buffer manager then make sure it
                // gets unmapped if it has a pending or completed async map.
                if result.is_none() && buffer.is_unmappable() {
                    buffer.unmap();
                }
            }
            callback(result);
        }));

        // If addFinishInfo() fails, it invokes the finish callback automatically, which handles
        // all the required clean up for us, just log an error message. The buffers will never be
        // mapped and thus don't need an unmap.
        if !self
            .queue_manager
            .add_finish_info(info, &self.resource_provider, &buffers_to_async_map)
        {
            skia_log_e!("Failed to register finish callbacks for asyncReadPixels.");
        }
    }

    /// `transferPixels(recorder, srcView, dstColorInfo, srcRect, cpuXferFn)`: copies the region
    /// to a transfer buffer, with the work added to `recorder` or, without one, to the queue
    /// manager. The result has no transfer buffer if the transfer cannot be set up.
    // Port of: src/gpu/graphite/Context.cpp#L816-L910 (chrome/m156)
    #[doc(alias = "transferPixels")]
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    pub fn transfer_pixels(
        &mut self,
        recorder: Option<&mut Recorder>,
        src_view: &TextureProxyView,
        dst_color_info: &ColorInfo,
        src_rect: IRect,
        cpu_xfer_fn: &TextureFormatXferFn,
    ) -> PixelTransferResult {
        debug_assert!(IRect::from_size(src_view.dimensions()).contains(&src_rect));
        let none = PixelTransferResult::default();

        let caps = Arc::clone(self.shared_context.caps());
        let Some(proxy) = src_view.proxy() else {
            return none;
        };
        if !Caps::is_copyable_src(&*caps, proxy.texture_info()) {
            return none;
        }

        let tex_info = proxy.texture_info();
        let format = texture_info_priv::view_format(tex_info);

        let Ok(bpp) = usize::try_from(texture_format_bytes_per_block(format)) else {
            return none;
        };
        let Ok(width) = usize::try_from(src_rect.width()) else {
            return none;
        };
        let Ok(height) = usize::try_from(src_rect.height()) else {
            return none;
        };
        let Some(unaligned_row_bytes) = bpp.checked_mul(width) else {
            return none;
        };
        let row_bytes = Caps::get_aligned_texture_data_row_bytes(&*caps, unaligned_row_bytes, bpp);
        let Some(size) = row_bytes.checked_mul(height).and_then(|size| {
            size.checked_next_multiple_of(caps.required_transfer_buffer_alignment())
        }) else {
            return none;
        };
        if row_bytes == 0 || size == 0 {
            return none;
        }
        let buffer = self
            .resource_provider
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .find_or_create_non_shareable_buffer(
                size,
                BufferType::XferGpuToCpu,
                AccessPattern::HostVisible,
                "TransferToCpu",
            );
        let Some(buffer) = buffer else {
            return none;
        };

        let flip_y = src_view.origin() == crate::gpu::gpu_types::Origin::BottomLeft;
        let mut copy_rect = src_rect;
        if flip_y {
            let h = src_view.dimensions().height;
            copy_rect = IRect::from_ltrb(
                src_rect.left,
                h - src_rect.bottom,
                src_rect.right,
                h - src_rect.top,
            );
        }

        // Set up copy task. Since we always use a new buffer the offset can be 0 and we don't
        // need to worry about aligning it to the required transfer buffer alignment.
        let copy_task = CopyTextureToBufferTask::make(
            src_view.ref_proxy(),
            copy_rect,
            buffer.clone(),
            /* buffer_offset= */ 0,
            row_bytes,
        );
        let sync_task = SynchronizeToCpuTask::make(buffer.clone());
        let Some(copy_task) = copy_task else {
            return none;
        };
        match recorder {
            None => {
                // `addTasksDirectly`
                let is_protected = self.shared_context.is_protected();
                let mut context = ContextPrivView {
                    shared_context: &self.shared_context,
                    resource_provider: &self.resource_provider,
                };
                for task in [&copy_task, &sync_task] {
                    let mut task = task.lock();
                    if !self
                        .queue_manager
                        .add_task(&mut task, &mut context, is_protected)
                    {
                        return none;
                    }
                }
            }
            Some(recorder) => {
                // Add the tasks to the Recorder instead of the QueueManager if that's been
                // required for collecting tasks to prepare the copied textures.
                recorder.priv_().add(copy_task);
                recorder.priv_().add(sync_task);
            }
        }

        let mut result = PixelTransferResult {
            transfer_buffer: Some(buffer),
            size: src_rect.size(),
            row_bytes: 0,
            pixel_converter: None,
        };
        if cpu_xfer_fn.is_identity() && !flip_y {
            result.row_bytes = row_bytes;
        } else {
            let dst_info = ImageInfo::from_color_info(src_rect.size(), dst_color_info.clone());
            let dst_row_bytes = dst_info.min_row_bytes();
            result.row_bytes = dst_row_bytes;
            let cpu_xfer_fn = cpu_xfer_fn.clone();
            let dst_width = width;
            let dst_height = height;
            // TODO(b/553467540): do flipping in TextureFormatXferFn::run
            result.pixel_converter = Some(Arc::new(move |dst: &mut [u8], src: &[u8]| {
                if flip_y {
                    for y in 0..dst_height {
                        let src_row = &src[(dst_height - 1 - y) * row_bytes..];
                        let dst_row = &mut dst[y * dst_row_bytes..];
                        cpu_xfer_fn.run(dst_width, 1, src_row, row_bytes, dst_row, dst_row_bytes);
                    }
                } else {
                    cpu_xfer_fn.run(dst_width, dst_height, src, row_bytes, dst, dst_row_bytes);
                }
            }));
        }

        result
    }

    /// Reads back a region of `src` and waits for it: the testing helper the C++ tests write
    /// around `asyncReadPixels` (submit with `SyncToCpu::kYes`, then wait for the callback).
    /// Returns the pixels in `dst_image_info`'s format with `(pixels, row_bytes)`, or `None` if
    /// the read failed.
    ///
    /// This is the body of `ContextPriv::readPixels` up to the copy into the caller's pixmap:
    /// a texturable source goes through `asyncReadPixels` (so GPU conversions are attempted), a
    /// source that is only copyable is read directly with the conversion done on the CPU.
    // Port of: src/gpu/graphite/Context.cpp#L1046-L1115 (chrome/m156)
    pub fn read_pixels(
        &mut self,
        src: &TextureProxyView,
        src_color_info: &ColorInfo,
        src_rect: IRect,
        dst_image_info: &ImageInfo,
    ) -> Option<(Vec<u8>, usize)> {
        let slot: ReadSlot = Arc::new(Mutex::new(None));
        let signal = Arc::clone(&slot);
        let params = AsyncReadParams {
            src: src.clone(),
            src_color_info: src_color_info.clone(),
            src_rect,
            dst_image_info: dst_image_info.clone(),
            callback: Box::new(move |result| {
                let pixels = result.map(|result| (result.data(0).to_vec(), result.row_bytes(0)));
                *signal.lock().unwrap_or_else(PoisonError::into_inner) = Some(pixels);
            }),
        };

        let caps = Arc::clone(self.shared_context.caps());
        let tex_info = src.proxy()?.texture_info();
        if Caps::is_texturable(&*caps, tex_info, false) {
            // Since this is a synchronous testing-only API, callers should have flushed any
            // pending work that modifies this texture proxy already.
            if params.validate() {
                self.async_read_pixels(None, params);
            } else {
                params.fail();
            }
        } else if Caps::is_copyable_src(&*caps, tex_info) {
            let format = texture_info_priv::view_format(tex_info);
            let dst_color_info = dst_image_info.color_info();
            let cs_steps = ColorSpaceXformSteps::new(
                src_color_info.color_space_ref(),
                src_color_info.alpha_type(),
                dst_color_info.color_space_ref(),
                dst_color_info.alpha_type(),
            );
            let xfer_fn = TextureFormatXferFn::make_gpu_to_cpu(
                format,
                src.swizzle(),
                &cs_steps,
                dst_color_info.color_type(),
            )?;
            self.async_read_texture(None, params, &xfer_fn);
        } else {
            return None;
        }

        let _ = self.submit(SubmitInfo::new(SyncToCpu::Yes));
        // A failed read has called the callback already; a successful one calls it when the
        // submission is retired (waited for above).
        let mut tries = 0;
        loop {
            if let Some(pixels) = slot.lock().unwrap_or_else(PoisonError::into_inner).take() {
                return pixels;
            }
            self.check_for_finished_work(SyncToCpu::Yes);
            tries += 1;
            if tries > 1000 {
                return None;
            }
        }
    }

    /// `ContextPriv::readPixels(pm, srcView, srcImageInfo, srcX, srcY)`: reads the region of
    /// `src_view` at `(src_x, src_y)` the size of `dst` into `dst`.
    // Port of: src/gpu/graphite/Context.cpp#L1046-L1115 (chrome/m156)
    #[doc(alias = "readPixels")]
    pub fn read_pixels_into(
        &mut self,
        dst: &mut Pixmap<'_>,
        src_view: &TextureProxyView,
        src_image_info: &ImageInfo,
        src_x: i32,
        src_y: i32,
    ) -> bool {
        let rect = IRect::from_xywh(src_x, src_y, dst.width(), dst.height());
        let Some((pixels, row_bytes)) =
            self.read_pixels(src_view, src_image_info.color_info(), rect, dst.info())
        else {
            return false;
        };
        let min_row_bytes = dst.info().min_row_bytes();
        let dst_row_bytes = dst.row_bytes();
        let height = usize::try_from(dst.height()).unwrap_or(0);
        let Some(out) = dst.writable_addr() else {
            return false;
        };
        // SkRectMemcpy
        for y in 0..height {
            out[y * dst_row_bytes..y * dst_row_bytes + min_row_bytes]
                .copy_from_slice(&pixels[y * row_bytes..y * row_bytes + min_row_bytes]);
        }
        true
    }

    /// `Device::onReadPixels` for a surface made by one of this context's recorders: snaps the
    /// recorder, inserts the recording, and reads the surface's target back into `dst`
    /// (`Surface::readPixels` in the `GPU_TEST_UTILS` build).
    ///
    /// Skia's device finds the context through its recorder; here the context is the caller.
    // Port of: src/gpu/graphite/Device.cpp#L726-L749 (chrome/m156)
    #[doc(alias = "onReadPixels")]
    pub fn read_surface_pixels(
        &mut self,
        surface: &Surface,
        dst: &mut Pixmap<'_>,
        src_x: i32,
        src_y: i32,
    ) -> bool {
        let Some(mut recorder) = surface.recorder() else {
            return false;
        };
        // Add all previous commands generated to the command buffer. If the client snaps later
        // they'll only get post-read commands in their Recording, but since they're doing a
        // readPixels in the middle that shouldn't be unexpected.
        let Some(mut recording) = recorder.snap() else {
            return false;
        };
        if self.insert_recording(InsertRecordingInfo::new(&mut recording)) != InsertStatus::Success
        {
            return false;
        }
        self.read_pixels_into(dst, surface.target(), surface.image_info(), src_x, src_y)
    }

    /// `Image::readPixels` for a Graphite image (a testing helper): the pixels of the region at
    /// `(src_x, src_y)` the size of `dst`, read with `ContextPriv::readPixels`. Any pending work
    /// that draws to the image's texture must have been flushed and inserted.
    #[doc(alias = "readPixels")]
    pub fn read_image_pixels(
        &mut self,
        image: &CoreImage,
        dst: &mut Pixmap<'_>,
        src_x: i32,
        src_y: i32,
    ) -> bool {
        let view = as_view(Some(image));
        if view.proxy().is_none() {
            return false;
        }
        self.read_pixels_into(dst, &view, image.image_info(), src_x, src_y)
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
        assert_eq!(
            lock_manager(&first.mapped_buffer_manager).num_client_held_buffers(),
            0
        );
    }
}
