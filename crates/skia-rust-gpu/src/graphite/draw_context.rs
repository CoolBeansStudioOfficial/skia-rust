// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/DrawContext.h, src/gpu/graphite/DrawContext.cpp

//! [`DrawContext`]: the pending draws and uploads of one render target, and the `DrawTask` they
//! are snapped into.
//!
//! Deviations from the C++:
//!
//! - `DrawContext` is owned by its `Device` (no `sk_sp`), and its functions take the recorder as a
//!   `RecorderPriv`, because a device holds the recorder weakly.
//! - The dst copy of a draw pass is `Image::Copy` in Skia (G10d). Here the copyable-source path
//!   is in `copy_target_for_dst_read()`; the copy-as-draw path, and `GenerateMipmaps()` for
//!   mipmapped targets, need `Image` and `Surface` (G10d) and drop the pass (copy-as-draw) or log
//!   (mipmaps) as their failure paths do.
//! - `getComputePathAtlas()` and `fComputePathAtlas` come with the atlases (G12a): there is no
//!   compute path atlas, which is what a platform without compute support gets.
//! - `fAdvancedBlendsRequireBarrier` is false: `blendEquationSupport()` is `kBasic` on the only
//!   backend, wgpu (`Caps::supportsHardwareAdvancedBlending()` is false).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use skia_rust_core::color::Color4f;
use skia_rust_core::image_info::{ColorInfo, ImageInfo};
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::{Contains, IRect};
use skia_rust_core::size::ISize;
use skia_rust_core::surface_props::SurfaceProps;

use crate::gpu::backing_fit::get_approx_size;
use crate::gpu::gpu_types::{Budgeted, Mipmapped};
use crate::gpu::sk_log::skia_log_w;
use crate::gpu::swizzle::Swizzle;
use crate::graphite::caps::Caps;
use crate::graphite::draw_list::DrawList;
use crate::graphite::draw_list_base::{DrawListBase, RecordDrawArgs, SnapArgs};
use crate::graphite::draw_list_layer::DrawListLayer;
use crate::graphite::draw_list_types::{DrawParamsId, LayerId};
use crate::graphite::draw_order::DrawOrder;
use crate::graphite::draw_params::{Clip, StrokeStyle};
use crate::graphite::draw_types::{BarrierType, DstUsage};
use crate::graphite::geom::geometry::Geometry;
use crate::graphite::geom::transform::Transform;
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::recorder::RecorderPriv;
use crate::graphite::render_pass_desc::RenderPassDesc;
use crate::graphite::renderer::Renderer;
use crate::graphite::resource_types::{DstReadStrategy, LoadOp, StoreOp};
use crate::graphite::storage_context::StorageContext;
use crate::graphite::task::copy_task::CopyTextureToTextureTask;
use crate::graphite::task::draw_task::DrawTask;
use crate::graphite::task::render_pass_task::{DrawPass, RenderPassTask};
use crate::graphite::task::upload_task::{
    ConditionalUploadContext, UploadList, UploadSource, UploadTask,
};
use crate::graphite::task::{Task, TaskRef};
use crate::graphite::texture_format::{
    are_color_type_and_format_compatible, read_swizzle_for_color_type, write_swizzle_for_color_type,
};
use crate::graphite::texture_info::texture_info_priv;
use crate::graphite::texture_proxy::TextureProxy;
use crate::graphite::texture_proxy_view::TextureProxyView;
use crate::graphite::unique_paint_params_id::UniquePaintParamsID;

/// The pending draws and uploads of a render target. `Device`s own one each.
// Port of: src/gpu/graphite/DrawContext.h#L45-L158 (chrome/m156)
#[doc(alias = "skgpu::graphite::DrawContext")]
#[derive(Debug)]
pub struct DrawContext {
    // May not be texturable, but preserves the read swizzle if it were to be read back to CPU or
    // copied to a texturable proxy.
    target: TextureProxyView,
    image_info: ImageInfo,
    surface_props: SurfaceProps,
    // Cache at creation so we don't require constant Caps access
    is_texturable: bool,

    // Does *not* reflect whether a dst read is needed by the DrawLists - simply specifies the
    // strategies to use should any encountered paint require it.
    dst_read_strategy: DstReadStrategy,
    supports_hardware_advanced_blend: bool,
    advanced_blends_require_barrier: bool,

    // The in-progress DrawTask that will be snapped and returned when some external requirement
    // must depend on the contents of this DrawContext's target. As higher-level Skia operations
    // are recorded, it can be necessary to flush pending draws and uploads into the task list.
    // This provides a place to reset scratch textures or buffers as their previous state will
    // have been consumed by the flushed tasks rendering to this DrawContext's target.
    current_draw_task: TaskRef,

    // Stores the most immediately recorded draws and uploads into the DrawContext's target.
    // These are collected outside of the DrawTask so that encoder switches can be minimized when
    // flushing.
    pending_draws: DrawListBase,
    pending_uploads: UploadList,

    // Shared with the `KeyContext`s of the draws recorded here, which write large gradients into
    // it.
    storage_context: Rc<RefCell<StorageContext>>,
}

// Locks `task` (a `DrawTask`) and calls `f` on it.
fn with_draw_task<R>(task: &TaskRef, f: impl FnOnce(&mut DrawTask) -> R) -> R {
    match &mut *task.lock() {
        Task::Draw(draw_task) => f(draw_task),
        _ => unreachable!("the current task of a DrawContext is a DrawTask"),
    }
}

impl DrawContext {
    /// `Make(caps, target, deviceSize, colorInfo, props, allowUnpremul)`: `allow_unpremul=true`
    /// should only be used if the target is only going to be rendered into with src-blending
    /// with calls to `drawPaint` or pixel-aligned `drawRect` calls to avoid anti-aliasing.
    ///
    /// `None` if the target cannot be rendered to with `color_info`.
    // Port of: src/gpu/graphite/DrawContext.cpp#L44-L72 (chrome/m156)
    #[must_use]
    pub fn make(
        caps: &dyn Caps,
        target: Option<Arc<TextureProxy>>,
        device_size: ISize,
        color_info: &ColorInfo,
        props: &SurfaceProps,
        allow_unpremul: bool,
    ) -> Option<DrawContext> {
        let target = target?;
        // We don't render to unknown or unpremul alphatypes unless allowUnpremul is explicitly
        // enabled.
        if color_info.alpha_type() == skia_rust_core::alpha_type::AlphaType::Unknown
            || (color_info.alpha_type() == skia_rust_core::alpha_type::AlphaType::Unpremul
                && !allow_unpremul)
        {
            return None;
        }
        if !caps.is_renderable(target.texture_info()) {
            return None;
        }
        if !are_color_type_and_format_compatible(color_info.color_type(), target.format()) {
            return None;
        }

        // Accept an approximate-fit texture, but make sure it's at least as large as the
        // device's logical size.
        debug_assert!(
            target.is_fully_lazy()
                || (target.dimensions().width >= device_size.width
                    && target.dimensions().height >= device_size.height)
        );
        let image_info = ImageInfo::from_color_info(device_size, color_info.clone());
        Some(DrawContext::new(caps, target, image_info, props))
    }

    // Port of: src/gpu/graphite/DrawContext.cpp#L74-L101 (chrome/m156)
    fn new(
        caps: &dyn Caps,
        target: Arc<TextureProxy>,
        image_info: ImageInfo,
        props: &SurfaceProps,
    ) -> Self {
        let swizzle = read_swizzle_for_color_type(image_info.color_type(), target.format());
        let target_view = TextureProxyView::new(Some(Arc::clone(&target)), swizzle);
        let is_texturable =
            caps.is_texturable(target.texture_info(), false) && !target.is_fully_lazy();
        let dst_read_strategy = caps.get_dst_read_strategy();
        let storage_buffer_support = caps.storage_buffer_support();
        let pending_draws = if caps.use_draw_list_layer() {
            DrawListBase::Layer(DrawListLayer::new(storage_buffer_support))
        } else {
            DrawListBase::List(DrawList::new())
        };
        let storage_context = Rc::new(RefCell::new(StorageContext::new(
            caps.resource_binding_requirements()
                .max_fallback_texture_size,
            storage_buffer_support,
        )));
        // Must determine a valid strategy to use should a dst texture read be required.
        debug_assert_ne!(dst_read_strategy, DstReadStrategy::NoneRequired);

        // TBD - Will probably want DrawLists (and its internal commands) to come from an arena
        // that the DC manages.
        Self {
            target: target_view,
            image_info,
            surface_props: *props,
            is_texturable,
            dst_read_strategy,
            supports_hardware_advanced_blend: caps.supports_hardware_advanced_blending(),
            advanced_blends_require_barrier: false,
            current_draw_task: DrawTask::new(target).into_ref(),
            pending_draws,
            pending_uploads: UploadList::new(),
            storage_context,
        }
    }

    /// `imageInfo()`.
    #[doc(alias = "imageInfo")]
    #[must_use]
    pub fn image_info(&self) -> &ImageInfo {
        &self.image_info
    }

    /// `colorInfo()`.
    #[doc(alias = "colorInfo")]
    #[must_use]
    pub fn color_info(&self) -> &ColorInfo {
        self.image_info.color_info()
    }

    /// `target()`.
    #[must_use]
    pub fn target(&self) -> &TextureProxyView {
        &self.target
    }

    /// `isTexturable()`.
    #[doc(alias = "isTexturable")]
    #[must_use]
    pub fn is_texturable(&self) -> bool {
        self.is_texturable
    }

    /// `surfaceProps()`.
    #[doc(alias = "surfaceProps")]
    #[must_use]
    pub fn surface_props(&self) -> &SurfaceProps {
        &self.surface_props
    }

    /// `pendingRenderSteps()`.
    #[doc(alias = "pendingRenderSteps")]
    #[must_use]
    pub fn pending_render_steps(&self) -> usize {
        self.pending_draws.render_step_count()
    }

    /// `modifiesTarget()`.
    #[doc(alias = "modifiesTarget")]
    #[must_use]
    pub fn modifies_target(&self) -> bool {
        self.pending_draws.modifies_target()
    }

    /// `readsTexture(texture)`.
    // Port of: src/gpu/graphite/DrawContext.cpp#L124-L139 (chrome/m156)
    #[doc(alias = "readsTexture")]
    #[must_use]
    pub fn reads_texture(&self, texture: &Arc<TextureProxy>) -> bool {
        if self.pending_draws.samples_texture(texture) {
            return true;
        }

        // visitProxies() before calling prepareResources() can revisit tasks in the general case
        // (e.g. processing everything in the root task list). In this case, the only tasks being
        // visited are pending tasks so their graph complexity should be minimal.
        let not_found = self.current_draw_task.lock().visit_proxies(
            &mut |other| {
                // Return true to continue visiting, i.e. when we haven't found `texture` yet.
                !Arc::ptr_eq(texture, other)
            },
            /*reads_only=*/ true,
        );

        !not_found // double negation means its found in a pending child task
    }

    /// `clear(clearColor)`.
    // Port of: src/gpu/graphite/DrawContext.cpp#L103-L106 (chrome/m156)
    pub fn clear(&mut self, clear_color: Color4f) {
        self.reset_for_clear_or_discard();
        self.pending_draws.reset(LoadOp::Clear, clear_color);
    }

    /// `discard()`.
    // Port of: src/gpu/graphite/DrawContext.cpp#L108-L111 (chrome/m156)
    pub fn discard(&mut self) {
        self.reset_for_clear_or_discard();
        self.pending_draws
            .reset(LoadOp::Discard, Color4f::new(0.0, 0.0, 0.0, 0.0));
    }

    // Port of: src/gpu/graphite/DrawContext.cpp#L113-L122 (chrome/m156)
    fn reset_for_clear_or_discard(&mut self) {
        // Non-loading operations on a fully lazy target can corrupt data beyond the
        // DrawContext's region so should be avoided.
        debug_assert!(
            !self
                .target
                .proxy()
                .is_some_and(|proxy| proxy.is_fully_lazy())
        );

        // NOTE: Eventually the current DrawTask should be reset, once there are no longer
        // implicit dependencies on atlas tasks between DrawContexts. When that's resolved, the
        // only tasks in the current DrawTask are those that directly impact the target, which
        // becomes irrelevant with the clear op overwriting it. For now, preserve the previous
        // tasks that might include atlas uploads that are not explicitly shared between
        // DrawContexts.
        // (The compute path atlas, if there were one, would be reset here.)
    }

    /// `recordDraw(renderer, localToDevice, geometry, clip, ordering, paintID, dstUsage, gatherer,
    /// stroke, lastInsertion)`: returns the draw's params and layer, which the clip stack
    /// remembers (both `None` for the sort-based draw list).
    // Port of: src/gpu/graphite/DrawContext.cpp#L141-L172 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    #[doc(alias = "recordDraw")]
    pub fn record_draw(
        &mut self,
        renderer: &Renderer,
        local_to_device: &Transform,
        geometry: &Geometry,
        clip: &Clip,
        ordering: DrawOrder,
        paint_id: UniquePaintParamsID,
        dst_usage: DstUsage,
        gatherer: &mut PipelineDataGatherer,
        stroke: Option<&StrokeStyle>,
        last_insertion: Option<LayerId>,
    ) -> (Option<DrawParamsId>, Option<LayerId>) {
        debug_assert!(
            skia_rust_core::rect::IRect::from_size(self.image_info.dimensions())
                .contains(&clip.scissor()),
            "Image {}x{}, scissor {:?}",
            self.image_info.width(),
            self.image_info.height(),
            clip.scissor()
        );

        // Determine whether a draw requies a barrier
        let mut barrier_before_draws = BarrierType::None;
        if self.dst_read_strategy == DstReadStrategy::ReadFromInput
            && dst_usage.contains(DstUsage::DST_READ_REQUIRED)
        {
            barrier_before_draws = BarrierType::ReadDstFromInput;
        }
        if dst_usage.contains(DstUsage::ADVANCED_BLEND)
            && self.supports_hardware_advanced_blend
            && self.advanced_blends_require_barrier
        {
            // A draw should only read from the dst OR use hardware for advanced blend modes.
            debug_assert!(!dst_usage.contains(DstUsage::DST_READ_REQUIRED));
            barrier_before_draws = BarrierType::AdvancedNoncoherentBlend;
        }

        self.pending_draws.record_draw(
            &RecordDrawArgs {
                renderer,
                local_to_device,
                geometry,
                clip,
                ordering,
                paint_id,
                dst_usage,
                barrier_before_draws,
                stroke,
            },
            gatherer,
            Some(&mut self.storage_context.borrow_mut()),
            last_insertion,
        )
    }

    /// `recordUpload(recorder, source, condContext)`.
    // Port of: src/gpu/graphite/DrawContext.cpp#L174-L181 (chrome/m156)
    #[doc(alias = "recordUpload")]
    pub fn record_upload(
        &mut self,
        recorder: &RecorderPriv<'_>,
        source: &UploadSource<'_>,
        cond_context: Option<Box<dyn ConditionalUploadContext>>,
    ) -> bool {
        // Since this upload is inline with the DrawContext's tasks, we do not attempt to upload
        // it directly via the host, we want to keep it as a repeatable task.
        self.pending_uploads.record_upload(
            &**recorder.caps(),
            &mut recorder.upload_buffer_manager().borrow_mut(),
            source,
            cond_context,
        )
    }

    /// `recordDependency(task)`: adds a `Task` that will be executed *before* any of the pending
    /// draws and uploads are executed as part of the next `flush()`.
    // Port of: src/gpu/graphite/DrawContext.cpp#L183-L189 (chrome/m156)
    #[doc(alias = "recordDependency")]
    pub fn record_dependency(&mut self, task: TaskRef) {
        // Adding `task` to the current DrawTask directly means that it will execute after any
        // previous dependent tasks and after any previous calls to flush(), but everything else
        // that's being collected on the DrawContext will execute after `task` once the next
        // flush() is performed.
        with_draw_task(&self.current_draw_task, |draw_task| {
            draw_task.add_task(task);
        });
    }

    /// `flush(recorder)`: moves all accumulated pending recorded operations (draws and uploads),
    /// and any other dependent tasks into the `DrawTask` currently being built.
    // Port of: src/gpu/graphite/DrawContext.cpp#L197-L322 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    #[allow(clippy::missing_panics_doc)] // the panics are SkASSERT-style invariants of the C++
    pub fn flush(&mut self, recorder: &RecorderPriv<'_>) {
        if self.pending_uploads.size() > 0 {
            if let Some(upload_task) = UploadTask::make(&mut self.pending_uploads) {
                self.record_dependency(upload_task);
            }
            // The UploadTask steals the collected upload instances, automatically resetting this
            // list
            debug_assert_eq!(self.pending_uploads.size(), 0);
        }

        // Generate compute dispatches that render into the atlas texture used by pending draws.
        // (The compute path atlas comes with G12a.)

        if !self.pending_draws.modifies_target() {
            // Nothing will be rasterized to the target that warrants a RenderPassTask, but we
            // preserve any added uploads or compute tasks since those could also affect the
            // target w/o rasterizing anything directly.
            return;
        }

        // Extract certain properties from DrawList relevant for DrawTask construction before
        // relinquishing the pending draw list to the DrawPass constructor.
        let mut dst_read_bounds = self.pending_draws.dst_read_bounds();
        dst_read_bounds.round_out();
        let dst_read_pixel_bounds = dst_read_bounds.as_sk_irect();
        let draws_require_msaa = self.pending_draws.draws_require_msaa();
        let ds_flags = self.pending_draws.depth_stencil_flags();
        // Determine the optimal dst read strategy for the drawpass given pending draw
        // characteristics
        let draw_pass_dst_read_strategy = if self.pending_draws.draws_read_dst() {
            self.dst_read_strategy
        } else {
            DstReadStrategy::NoneRequired
        };

        // Convert the pending draws and load/store ops into a DrawPass that will be executed
        // after the collected uploads and compute dispatches.
        // TODO: At this point, there's only ever one DrawPass in a RenderPassTask to a target.
        // When subpasses are implemented, they will either be collected alongside fPendingDraws
        // or added to the RenderPassTask separately.
        let target = self.target.ref_proxy().expect("a DrawContext has a target");
        let current_draw_task = Arc::clone(&self.current_draw_task);
        let pass = self.pending_draws.snap_draw_pass(
            Some(&mut self.storage_context.borrow_mut()),
            SnapArgs {
                recorder,
                record_dependency: &mut |task| {
                    with_draw_task(&current_draw_task, |draw_task| draw_task.add_task(task));
                },
                target: Arc::clone(&target),
                target_dimensions: self.image_info.dimensions(),
                dst_read_strategy: draw_pass_dst_read_strategy,
            },
        );
        debug_assert!(!self.pending_draws.modifies_target()); // Should be drained into `pass`.

        // else pass creation failed, DrawPass will have logged why. Don't discard the previously
        // accumulated tasks, however, since they may represent operations on an atlas that
        // other DrawContexts now implicitly depend on.
        let Some(pass) = pass else { return };
        debug_assert!(Arc::ptr_eq(&target, pass.target()));

        // If any paint used within the DrawPass reads from the dst texture (indicated by
        // nonempty dstReadPixelBounds) and the dstReadStrategy is kTextureCopy, then add a
        // CopyTask.
        let mut dst_copy: Option<Arc<TextureProxy>> = None;
        if !dst_read_pixel_bounds.is_empty()
            && draw_pass_dst_read_strategy == DstReadStrategy::TextureCopy
        {
            dst_copy = self.copy_target_for_dst_read(recorder, dst_read_pixel_bounds);
            if dst_copy.is_none() {
                skia_log_w!("DrawContext::flush Image::Copy failed, draw pass dropped!");
                return;
            }
        }

        let caps = recorder.caps();
        let (load_op, store_op) = pass.ops();
        let format = texture_info_priv::view_format(target.texture_info());
        let write_swizzle = write_swizzle_for_color_type(self.color_info().color_type(), format)
            .unwrap_or_else(|| {
                // Fall back to rgba in release builds
                skia_log_w!(
                    "No valid write swizzle for color type {:?} with format {:?}",
                    self.color_info().color_type(),
                    format
                );
                Swizzle::rgba()
            });
        let desc = RenderPassDesc::make(
            &**caps,
            target.texture_info(),
            load_op,
            store_op,
            ds_flags,
            pass.clear_color(),
            draws_require_msaa,
            write_swizzle,
            draw_pass_dst_read_strategy,
        );

        let passes: Vec<Box<dyn DrawPass>> = vec![Box::new(pass)];
        if let Some(render_pass_task) = RenderPassTask::make(
            passes,
            &desc,
            Some(Arc::clone(&target)),
            dst_copy,
            dst_read_pixel_bounds,
        ) {
            self.record_dependency(render_pass_task);
        }
        if self.target.mipmapped() == Mipmapped::Yes {
            // `GenerateMipmaps(recorder, this, target)` draws with a scratch `Surface` and
            // `Image` (G10d).
            skia_log_w!("DrawContext::flush GenerateMipmaps failed, draw pass dropped!");
        }
    }

    /// The copyable-source path of `Image::Copy()` for the dst copy of `flush()`: copies
    /// `subset` of the target into a new texture and records the copy before the render pass.
    // Port of: src/gpu/graphite/Image_Graphite.cpp#L86-L149 (chrome/m156)
    #[allow(clippy::trivially_copy_pass_by_ref)] // mirrors the C++ `Recorder*` parameter
    fn copy_target_for_dst_read(
        &mut self,
        recorder: &RecorderPriv<'_>,
        subset: IRect,
    ) -> Option<Arc<TextureProxy>> {
        let src_proxy = self.target.ref_proxy()?;
        debug_assert!(
            src_proxy.is_fully_lazy() || IRect::from_size(src_proxy.dimensions()).contains(&subset)
        );
        let caps = recorder.caps();
        if !caps.is_copyable_src(src_proxy.texture_info()) {
            // The texture is not blittable (and whether it is texturable, copy-as-draw is
            // `Image::CopyAsDraw()` of G10d).
            return None;
        }

        let texture_info =
            caps.get_texture_info_for_sampled_copy(src_proxy.texture_info(), Mipmapped::No);

        let dst = {
            let mut resource_provider = recorder
                .resource_provider()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            TextureProxy::make(
                &**caps,
                &mut resource_provider,
                get_approx_size(ISize::new(subset.width(), subset.height())),
                &texture_info,
                Budgeted::Yes,
                "DstCopy",
            )?
        };

        let copy_task = CopyTextureToTextureTask::make(
            Some(src_proxy),
            subset,
            Some(Arc::clone(&dst)),
            IPoint::new(0, 0),
            0,
        )?;
        self.record_dependency(copy_task);
        Some(dst)
    }

    /// `snapDrawTask()`: returns the current `DrawTask` to the caller, so all pending draws and
    /// uploads (if `flush()` was not immediately called prior to this) and subsequently recorded
    /// draws and uploads will go into a new `DrawTask`.
    // Port of: src/gpu/graphite/DrawContext.cpp#L324-L333 (chrome/m156)
    #[doc(alias = "snapDrawTask")]
    #[allow(clippy::missing_panics_doc)] // the panics are SkASSERT-style invariants of the C++
    pub fn snap_draw_task(&mut self) -> Option<TaskRef> {
        if !with_draw_task(&self.current_draw_task, |draw_task| draw_task.has_tasks()) {
            return None;
        }

        let target = self.target.ref_proxy().expect("a DrawContext has a target");
        let snapped_task = std::mem::replace(
            &mut self.current_draw_task,
            DrawTask::new(target).into_ref(),
        );
        Some(snapped_task)
    }

    /// `dstReadStrategy()`: the dst read strategy to use when/if a paint requires a dst read.
    #[doc(alias = "dstReadStrategy")]
    #[must_use]
    pub fn dst_read_strategy(&self) -> DstReadStrategy {
        self.dst_read_strategy
    }

    /// `storageContext()`.
    #[doc(alias = "storageContext")]
    #[must_use]
    pub fn storage_context(&self) -> &Rc<RefCell<StorageContext>> {
        &self.storage_context
    }

    /// The pending draws, for tests.
    #[must_use]
    pub fn pending_draws(&self) -> &DrawListBase {
        &self.pending_draws
    }
}

// Keeps `StoreOp` referenced: the pass ops always store (`RenderPassDesc::Make` asserts it).
const _: StoreOp = StoreOp::Store;
