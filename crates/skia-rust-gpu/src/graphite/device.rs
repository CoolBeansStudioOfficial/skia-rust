// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/Device.h, src/gpu/graphite/Device.cpp (L1-L2140 and L2343-L2631,
//                   the drawing core; paths, text and special images are G10c/G10d)

//! [`Device`]: the Graphite `SkDevice`. It turns the canvas's draw calls into draws recorded in
//! its [`DrawContext`], and flushes them into `Task`s of its recorder.
//!
//! # Structure
//!
//! `Device` is `Device { state: DeviceState, core: Rc<RefCell<DeviceCore>> }`
//! (`docs/design/gpu.md` §5.1). The canvas owns the `Device` through core's `Device` trait, whose
//! `state()` has to hand out a reference, so the `SkDevice` base state (the transforms) lives
//! outside the `RefCell`. Everything else is in [`DeviceCore`], which the recorder tracks weakly
//! ([`TrackedDevice`]) so that it can flush the device when it snaps. A device holds the recorder
//! through a `Weak<RecorderInner>`: once the recorder is gone the device's draws are no-ops
//! (`abandonRecorder()`).
//!
//! # What is not here yet
//!
//! - `ClipStack` (G10b) is the device's clip; the clip atlas it can hand draws (`ClipAtlasManager`)
//!   is G12a, so the device passes none and every clip element that is not analytic is a
//!   depth-only clip draw.
//! - Path rendering (`chooseRenderer()`'s atlas strategies, path atlases, G12a), text
//!   (`onDrawGlyphRunList`, `drawSlug`, G12b), `drawSpecial()`, `snapSpecial()`,
//!   `drawCoverageMask()`, `drawBlurredRRect()` and the image filtering backend (G10c), and
//!   `drawAsTiledImageRect()` (it needs `TiledTextureUtils::DrawAsTiledImageRect`) and the image
//!   links of `notifyInUse()` (`Image_Graphite` does not own the device: see `image_graphite`).
//!   `makeSurface()`, `makeImageCopy()` and the non-copyable `onWritePixels()` fallback are ported.
//! - Sparse strips (Q5, G17) and `GPU_TEST_UTILS` readPixels.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blender::Blender;
use skia_rust_core::canvas::{PointMode, SrcRectConstraint};
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::device::{CreateInfo, Device as CoreDevice, DeviceState};
use skia_rust_core::image::{Image, RequiredProperties};
use skia_rust_core::image_info::{ColorInfo, ImageInfo};
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::mesh::{self, Mesh, mesh_priv};
use skia_rust_core::paint::{Cap, Paint, Style as PaintStyle};
use skia_rust_core::path::Path;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{Contains, IRect, Rect as SkRect, rect_priv};
use skia_rust_core::region::Region;
use skia_rust_core::rrect::{RRect, rrect_priv};
use skia_rust_core::rsxform::RSXform;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::shader::Shader;
use skia_rust_core::size::ISize;
use skia_rust_core::stroke_rec::{InitStyle, StrokeRec, Style as StrokeStyleKind};
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::vertices::Vertices;
use skia_rust_raster::region_path::RegionExt;
use skia_rust_simd::vx::{self, Float2};

use crate::gpu::backing_fit::{BackingFit, get_approx_size};
use crate::gpu::gpu_types::{Budgeted, Mipmapped, Origin, Renderable};
use crate::gpu::sk_log::skia_log_w;
use crate::graphite::clip_stack::{
    ClipDrawHooks, ClipStack, ClipState, ElementList, PixelSnapping,
};
use crate::graphite::draw_context::DrawContext;
use crate::graphite::draw_list_base::MAX_RENDER_STEPS;
use crate::graphite::draw_list_types::{DrawParamsId, LayerId};
use crate::graphite::draw_order::{
    CompressedPaintersOrder, DisjointStencilIndex, DrawOrder, PaintersDepth,
};
use crate::graphite::draw_params::{Clip, StrokeStyle};
use crate::graphite::draw_types::DstUsage;
use crate::graphite::geom::bounds_manager::{BoundsManager, HybridBoundsManager};
use crate::graphite::geom::edge_aa_quad::{EdgeAAQuad, Flags as EdgeFlags};
use crate::graphite::geom::geometry::Geometry;
use crate::graphite::geom::intersection_tree::IntersectionTree;
use crate::graphite::geom::rect::Rect;
use crate::graphite::geom::shape::Shape;
use crate::graphite::geom::transform::{Transform, Type as TransformType};
use crate::graphite::graphite_types::{DepthStencilFlags, SampleCount};
use crate::graphite::image_factories::texture_from_image;
use crate::graphite::image_graphite::Image as GraphiteImage;
use crate::graphite::key_context::{KeyContext, KeyGenFlags};
use crate::graphite::paint_params::{PaintParams, ShadingParams, SimpleImage};
use crate::graphite::recorder::{Recorder, RecorderInner, RecorderPriv, TrackedDevice};
use crate::graphite::render_step::Coverage;
use crate::graphite::renderer::Renderer;
use crate::graphite::resource_types::{DstReadStrategy, LoadOp};
use crate::graphite::surface_graphite::Surface;
use crate::graphite::task::TaskRef;
use crate::graphite::task::upload_task::{MipLevel, UploadSource};
use crate::graphite::texture_proxy::TextureProxy;
use crate::graphite::texture_proxy_view::TextureProxyView;
use crate::graphite::unique_paint_params_id::UniquePaintParamsID;

// ASSERT_SINGLE_OWNER: a device is `!Send` (it holds `Rc`s), which is the single-owner contract.

/// The `SkCanvas::ImageSetEntry` of `drawEdgeAAImageSet()`, with edge flags as
/// [`EdgeAAQuad`] flags (`SkCanvas::QuadAAFlags`).
// Port of: include/core/SkCanvas.h (SkCanvas::ImageSetEntry) (chrome/m156)
#[doc(alias = "SkCanvas::ImageSetEntry")]
#[derive(Clone, Debug)]
pub struct ImageSetEntry {
    /// `fImage`.
    pub image: Image,
    /// `fSrcRect`.
    pub src_rect: SkRect,
    /// `fDstRect`.
    pub dst_rect: SkRect,
    /// `fMatrixIndex`: index into the pre-view matrices, or negative for none.
    pub matrix_index: i32,
    /// `fAlpha`.
    pub alpha: f32,
    /// `fAAFlags`.
    pub aa_flags: EdgeFlags,
    /// `fHasClip`: whether the entry has four points in the `dstClips` array.
    pub has_clip: bool,
}

// Port of: src/gpu/graphite/Device.cpp#L90-L93 (chrome/m156), `DefaultFillStyle()`
fn default_fill_style() -> StrokeRec {
    StrokeRec::new(InitStyle::Fill)
}

/// If the paint can be reduced to a solid flood-fill, compute the correct color to fill with.
// Port of: src/gpu/graphite/Device.cpp#L95-L111 (chrome/m156)
fn extract_paint_color(paint: &PaintParams, dst_color_info: &ColorInfo) -> Option<Color4f> {
    // kClear is converted to kSrc automatically; if we're here that means the final blend must
    // be src or src-over with an opaque effect.
    debug_assert!(
        paint.final_blender().is_some()
            || paint.final_blend_mode() == skia_rust_core::blend_mode::BlendMode::Src
            || paint.final_blend_mode() == skia_rust_core::blend_mode::BlendMode::SrcOver
    );
    // PaintParams has already consolidated constant shaders or images and applied color filters
    // to constant input colors. If the paint still has any of those fields, then we can't extract
    // it.
    if paint.shader().is_some() || paint.image_shader().is_some() || paint.color_filter().is_some()
    {
        return None;
    }

    // However, PaintParams stores the color in sRGB and we need to return this in the
    // destination color space.
    Some(crate::graphite::paint_params::color4f_prep_for_dst(
        *paint.color(),
        dst_color_info,
    ))
}

/// Returns a local rect that has been adjusted such that when it's rasterized with
/// `localToDevice` it will be pixel aligned. If this adjustment is not possible (due to
/// transform type or precision) then this returns the original local rect unmodified.
///
/// If `stroke_width` is `None`, it's assumed to be a filled rectangle. If it's `Some`, on input
/// it should hold the stroke width (or 0 for a hairline). After this returns, the stroke width
/// may have been adjusted so that outer and inner stroked edges are pixel aligned (in which case
/// the underlying rectangle geometry probably won't be pixel aligned).
///
/// A best effort is made to align the stroke edges when there's a non-uniform scale factor that
/// prevents exactly aligning both X and Y axes.
// Port of: src/gpu/graphite/Device.cpp#L113-L186 (chrome/m156)
#[must_use]
pub fn snap_rect_to_pixels(
    local_to_device: &Transform,
    rect: &Rect,
    stroke_width: Option<&mut f32>,
) -> Rect {
    if local_to_device.type_() > TransformType::RectStaysRect {
        return *rect;
    }

    let mut snapped_device_rect;
    match stroke_width {
        None => {
            // Just a fill, use round() to emulate non-AA rasterization (vs. roundOut() to get
            // the covering bounds). This matches how ClipStack treats clipRects with
            // PixelSnapping::kYes.
            snapped_device_rect = local_to_device.map_rect(rect).make_round();
        }
        Some(stroke_width) => {
            #[allow(clippy::float_cmp)] // exact comparison, as in the C++
            if *stroke_width == 0.0 {
                // Hairline case needs to be outset by 1/2 device pixels *before* rounding, and
                // then inset by 1/2px to get the base shape while leaving the stroke width as 0.
                snapped_device_rect = local_to_device.map_rect(rect);
                snapped_device_rect.outset(0.5).round().inset(0.5);
            } else {
                // For regular strokes, outset by the stroke radius *before* mapping to device
                // space, and then round.
                snapped_device_rect = local_to_device
                    .map_rect(&rect.make_outset(0.5 * *stroke_width))
                    .make_round();

                // devScales.x() holds scale factor affecting device-space X axis (so max of
                // |m00| or |m01|) and y() holds the device Y axis scale (max of |m10| or |m11|).
                let m = local_to_device.matrix();
                let dev_scales = vx::abs(Float2::new(m.rc(0, 0), m.rc(1, 0)))
                    .max(vx::abs(Float2::new(m.rc(0, 1), m.rc(1, 1))));
                let dev_stroke_width = vx::round(dev_scales * *stroke_width).max(1.0);

                // Prioritize the axis that has the largest device-space radius (any error from a
                // non-uniform scale factor will go into the inner edge of the opposite axis).
                // During animating scale factors, preserving the large axis leads to better
                // behavior.
                if dev_stroke_width.x() > dev_stroke_width.y() {
                    *stroke_width = dev_stroke_width.x() / dev_scales.x();
                } else {
                    *stroke_width = dev_stroke_width.y() / dev_scales.y();
                }

                snapped_device_rect.inset_vec(dev_scales * (0.5 * *stroke_width));
            }
        }
    }

    // Map back to local space so that it can be drawn with appropriate coord interpolation.
    let snapped_local_rect = local_to_device.inverse_map_rect(&snapped_device_rect);
    // If the transform has an extreme scale factor or large translation, it's possible for
    // floating point precision to round `snappedLocalRect` in such a way that re-transforming it
    // by the local-to-device matrix no longer matches the expected device bounds.
    if snapped_device_rect.nearly_equals(&local_to_device.map_rect(&snapped_local_rect), 0.0) {
        snapped_local_rect
    } else {
        // In this case we will just return the original geometry and the pixels will show
        // fractional coverage.
        *rect
    }
}

/// If possible, snaps `dst_rect` such that its device-space transformation lands on pixel
/// bounds, and then updates `src_rect` to match the original src-to-dst coordinate mapping.
// Port of: src/gpu/graphite/Device.cpp#L188-L202 (chrome/m156)
pub fn snap_src_and_dst_rect_to_pixels(
    local_to_device: &Transform,
    src_rect: &mut SkRect,
    dst_rect: &mut SkRect,
) {
    if local_to_device.type_() > TransformType::RectStaysRect {
        return;
    }

    // Assume snapping will succeed and always update 'src' to match; in the event snapping
    // returns the original dst rect, then the recalculated src rect is a no-op.
    let dst_to_src = Matrix::rect_to_rect_or_identity(*dst_rect, *src_rect, None);
    *dst_rect =
        snap_rect_to_pixels(local_to_device, &Rect::from_sk_rect(dst_rect), None).as_sk_rect();
    *src_rect = dst_to_src.map_rect(*dst_rect).0;
}

/// Returns the inner bounds of `geometry` that is known to have full coverage. This does not
/// worry about identifying draws that are equivalent pixel aligned and thus entirely full
/// coverage, as that should have been caught earlier and used a coverage-less renderer from the
/// beginning.
///
/// An empty `Rect` is returned if there is no available inner bounds, or if it's not worth
/// performing.
// Port of: src/gpu/graphite/Device.cpp#L204-L240 (chrome/m156)
#[allow(clippy::items_after_statements)] // mirrors the C++ local constexpr
fn get_inner_bounds(geometry: &Geometry, local_to_device: &Transform) -> Rect {
    let apply_aa_inset = |mut rect: Rect| {
        // If the aa inset is too large, rect becomes empty and the inner bounds draw is
        // automatically skipped
        let aa_inset = local_to_device.local_aa_radius(&rect);
        rect.inset(aa_inset);
        // Only add a second draw if it will have a reasonable number of covered pixels;
        // otherwise we are just adding draws to sort and pipelines to switch around.
        const INNER_FILL_AREA: f32 = 64.0 * 64.0;
        // Approximate the device-space area based on the minimum scale factor of the transform.
        let scale_factor = 1.0_f32 / aa_inset; // sk_ieee_float_divide
        if scale_factor * rect.area() >= INNER_FILL_AREA {
            rect
        } else {
            Rect::infinite_inverted()
        }
    };

    match geometry {
        Geometry::EdgeAAQuad(quad) => {
            if quad.is_rect() {
                return apply_aa_inset(quad.bounds());
            }
            // else currently we don't have a function to calculate the largest interior axis
            // aligned bounding box of a quadrilateral so skip the inner fill draw.
        }
        Geometry::Shape(shape) => {
            if shape.is_rect() {
                return apply_aa_inset(*shape.rect());
            } else if shape.is_rrect() {
                return apply_aa_inset(Rect::from_sk_rect(&rrect_priv::inner_bounds(
                    shape.rrect(),
                )));
            }
        }
        _ => {}
    }

    Rect::infinite_inverted()
}

// Port of: src/gpu/graphite/Device.cpp#L242-L244 (chrome/m156)
fn rect_to_pixelbounds(r: &Rect) -> IRect {
    r.make_round_out().as_sk_irect()
}

// Port of: src/gpu/graphite/Device.cpp#L246-L253 (chrome/m156)
fn is_pixel_aligned(r: &Rect, t: &Transform) -> bool {
    if t.type_() <= TransformType::RectStaysRect {
        let dev_rect = t.map_rect(r);
        return dev_rect.nearly_equals(&dev_rect.make_round(), Shape::DEFAULT_PIXEL_TOLERANCE);
    }

    false
}

// Port of: src/gpu/graphite/Device.cpp#L255-L279 (chrome/m156)
fn is_simple_shape(shape: &Shape, local_to_device: &Transform, ty: StrokeStyleKind) -> bool {
    if shape.is_flood_fill() {
        return true; // Always supported
    } else if !shape.inverted() && ty != StrokeStyleKind::StrokeAndFill {
        // A filled line renders nothing but that should be caught earlier, so the actual
        // branches in this function can be simplified.
        debug_assert!(!shape.is_line() || ty != StrokeStyleKind::Fill);

        if shape.is_rrect() && ty == StrokeStyleKind::Stroke {
            // Non-hairline stroked round rects require the corner radii to be circular to be
            // compatible with the shared Renderer.
            let tol =
                local_to_device.local_aa_radius(&shape.bounds()) * Shape::DEFAULT_PIXEL_TOLERANCE;
            return rrect_priv::all_corners_relatively_circular(shape.rrect(), tol);
        } else if shape.is_rrect() || shape.is_rect() || shape.is_line() {
            // There are no restrictions on filled or hairline [r]rects and lines.
            return true;
        } // Fallthrough
    }

    // Requires path rendering
    false
}

/// `IntersectionTreeSet` controls multiple `IntersectionTree`s to organize all add rectangles
/// into disjoint sets. For a given `CompressedPaintersOrder` and bounds, it returns the smallest
/// `DisjointStencilIndex` that guarantees the bounds are disjoint from all other draws that use
/// the same painters order and stencil index.
// Port of: src/gpu/graphite/Device.cpp#L326-L371 (chrome/m156)
#[doc(alias = "skgpu::graphite::Device::IntersectionTreeSet")]
#[derive(Debug, Default)]
pub struct IntersectionTreeSet {
    // Each compressed painters order defines a barrier around draws so each order's set of draws
    // are independent, even if they may intersect. Within each order, the list of trees holds the
    // IntersectionTrees representing each disjoint set.
    // TODO: This organization of trees is logically convenient but may need to be optimized
    // based on real world data (e.g. how sparse is the map, how long is each vector of
    // trees,...)
    trees: HashMap<u16, Vec<IntersectionTree>>,
}

impl IntersectionTreeSet {
    /// `add(drawOrder, rect)`.
    pub fn add(&mut self, draw_order: CompressedPaintersOrder, rect: Rect) -> DisjointStencilIndex {
        let trees = self.trees.entry(draw_order.bits()).or_default();
        let mut stencil = DisjointStencilIndex::first();
        for tree in trees.iter_mut() {
            if tree.add(rect) {
                return stencil;
            }
            stencil = stencil.next(); // advance to the next tree's index
        }

        // If here, no existing intersection tree can hold the rect so add a new one
        debug_assert!(stencil != DrawOrder::K_UNASSIGNED);
        let mut new_tree = IntersectionTree::new();
        let added = new_tree.add(rect);
        debug_assert!(added);
        trees.push(new_tree);
        stencil
    }

    /// `reset()`.
    pub fn reset(&mut self) {
        self.trees.clear();
    }
}

// These default tuning numbers for the HybridBoundsManager were chosen from looking at
// performance and accuracy curves produced by the BoundsManagerBench for random draw bounding
// boxes. This config will use brute force for the first 64 draw calls to the Device and then
// switch to a grid that is dynamically sized to produce cells that are 16x16, up to a grid
// that's 32x32 cells. This seemed like a sweet spot balancing accuracy for low-draw count
// surfaces and overhead for high-draw count and high-resolution surfaces. With the 32x32 grid
// limit, cell size will increase above 16px when the surface dimension goes above 512px.
// TODO: These could be exposed as context options or surface options, and we may want to have
// different strategies in place for a base device vs. a layer's device.
// Port of: src/gpu/graphite/Device.cpp#L437-L439 (chrome/m156)
const GRID_CELL_SIZE: i32 = 16;
const MAX_BRUTE_FORCE_N: i32 = 64;
const MAX_GRID_SIZE: i32 = 32;

/// The state of a [`Device`] that the recorder reaches (`Device` minus the `SkDevice` base).
#[doc(alias = "skgpu::graphite::Device")]
pub struct DeviceCore {
    recorder: Weak<RecorderInner>,
    dc: DrawContext,
    // Scratch devices hold on to their last snapped DrawTask so that they can be directly
    // referenced when the device image is drawn into some other surface.
    // NOTE: For now, this task is still added to the root task list when the Device is flushed,
    // but in the long-term, these scratch draw tasks will only be executed if they are
    // referenced by some other task chain that makes it to the root list.
    last_task: Option<TaskRef>,

    // `None` only while a call into the clip stack lends the core to it as its `ClipDrawHooks`.
    clip: Option<ClipStack>,

    // TODO (thomsmit): remove these when layering is added
    // Tracks accumulated intersections for ordering dependent use of the color and depth
    // attachment (i.e., depth-based clipping, and transparent blending)
    color_depth_bounds_manager: Rc<RefCell<HybridBoundsManager>>,
    // Tracks disjoint stencil indices for all recordered draws
    disjoint_stencil_set: IntersectionTreeSet,

    // The `SkDevice` state the device reads, synchronized from `Device::state` before each call.
    width: i32,
    height: i32,
    // Lazily updated Transform constructed from localToDevice()'s SkM44
    cached_local_to_device: Transform,
    local_to_device44: M44,
    local_to_device33: Matrix,
    global_to_device: M44,

    // The max depth value sent to the DrawContext, incremented so each draw has a unique value.
    current_depth: PaintersDepth,

    // Even when MSAA is supported, small paths may be sent to the atlas for higher quality and to
    // avoid triggering MSAA overhead on a render pass. However, the number of paths is capped
    // per Device flush.
    atlased_path_count: i32,
    // True if this Device has been drawn into another Device, in which case that other Device
    // depends on this Device's prior contents, so flushing this device with pending work must
    // also flush anything else that samples from it. If this is false, it's safe to skip
    // checking tracked devices for dependencies.
    must_flush_dependencies: bool,

    // Tracks the flushing state to ensure recursive flushing does not occur.
    #[cfg(debug_assertions)]
    is_flushing: bool,

    // When not 0, this Device is an unregistered scratch device that is intended to go out of
    // scope before the Recorder is snapped. Assuming controlling code is valid, that means the
    // Device's recorder's next recording ID should still be the the recording ID at the time the
    // Device was created. If not, it means the Device lived too long and may not be flushing
    // tasks in the expected order.
    scoped_recording_id: u32,

    // The tracked handle of this core, to deregister it.
    this: Weak<RefCell<DeviceCore>>,
}

impl std::fmt::Debug for DeviceCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceCore")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("dc", &self.dc)
            .finish_non_exhaustive()
    }
}

/// The Graphite device: the drawing target behind a canvas (or a layer of one).
// Port of: src/gpu/graphite/Device.h#L84-L282 (chrome/m156)
#[doc(alias = "skgpu::graphite::Device")]
pub struct Device {
    state: DeviceState,
    core: Rc<RefCell<DeviceCore>>,
    registered: bool,
}

impl std::fmt::Debug for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Device")
            .field("core", &self.core)
            .finish_non_exhaustive()
    }
}

impl Device {
    /// `Make(recorder, target, deviceSize, colorInfo, props, initialLoadOp, registerWithRecorder,
    /// allowUnpremul)`: if `register_with_recorder` is false, it is meant to be a short-lived
    /// Device that is managed by the caller within a limited scope (such that it is guaranteed to
    /// go out of scope before the Recorder can be snapped).
    // Port of: src/gpu/graphite/Device.cpp#L452-L520 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    #[must_use]
    pub fn make(
        recorder: &Recorder,
        target: Option<Arc<TextureProxy>>,
        device_size: ISize,
        color_info: &ColorInfo,
        props: &SurfaceProps,
        initial_load_op: LoadOp,
        register_with_recorder: bool,
        allow_unpremul: bool,
    ) -> Option<Device> {
        let target = target?;

        // DrawContext::Make ensures `target` can be rendered into, but if the path strategy might
        // require MSAA, then we need to make sure a multisampled attachment can also be created
        // later.
        // - This would also apply for compute renderers that have to write directly to
        //   `target`, but the current versions of compute render into separate compute-compatible
        //   textures instead.
        // (Only the tessellation strategy exists until G10c/G12a.)
        let priv_ = recorder.priv_();
        let caps = priv_.caps();
        if caps.get_compatible_msaa_sample_count(target.texture_info()) <= SampleCount::One {
            return None;
        }

        let mut dc = DrawContext::make(
            &**caps,
            Some(target),
            device_size,
            color_info,
            props,
            allow_unpremul,
        )?;
        match initial_load_op {
            LoadOp::Clear => dc.clear(Color4f::new(0.0, 0.0, 0.0, 0.0)),
            LoadOp::Discard => dc.discard(),
            LoadOp::Load => {} // kLoad is the default initial op for a DrawContext
        }

        let state = DeviceState::new(dc.image_info().clone(), *dc.surface_props());
        let core = Rc::new_cyclic(|this| {
            RefCell::new(DeviceCore::new(recorder.downgrade(), dc, this.clone()))
        });
        let device = Device {
            state,
            core,
            registered: register_with_recorder,
        };
        if register_with_recorder {
            // We don't register the device with the recorder until after the constructor has
            // returned.
            let tracked: Rc<RefCell<dyn TrackedDevice>> = device.core.clone();
            recorder.priv_().register_device(Rc::downgrade(&tracked));
        } else {
            // Since it's not registered, it should go out of scope before nextRecordingID()
            // changes from what is saved to fScopedRecordingID.
            device.core.borrow_mut().scoped_recording_id = priv_.next_recording_id();
        }
        Some(device)
    }

    /// The convenience factory that creates the underlying `TextureProxy` based on the
    /// configuration provided.
    // Port of: src/gpu/graphite/Device.cpp#L418-L450 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    #[must_use]
    pub fn make_with_info(
        recorder: Option<&Recorder>,
        image_info: &ImageInfo,
        budgeted: Budgeted,
        mipmapped: Mipmapped,
        backing_fit: BackingFit,
        props: &SurfaceProps,
        initial_load_op: LoadOp,
        label: &str,
        register_with_recorder: bool,
        allow_unpremul: bool,
    ) -> Option<Device> {
        debug_assert!(!(mipmapped == Mipmapped::Yes && backing_fit == BackingFit::Approx));
        let recorder = recorder?;

        let priv_ = recorder.priv_();
        let caps = priv_.caps();
        let backing_dimensions = if backing_fit == BackingFit::Approx {
            get_approx_size(image_info.dimensions())
        } else {
            image_info.dimensions()
        };
        let texture_info = caps.get_default_sampled_texture_info(
            image_info.color_type(),
            mipmapped,
            priv_.is_protected(),
            Renderable::Yes,
        );

        let target = {
            let mut resource_provider = priv_
                .resource_provider()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            TextureProxy::make(
                &**caps,
                &mut resource_provider,
                backing_dimensions,
                &texture_info,
                budgeted,
                label,
            )
        };
        Device::make(
            recorder,
            target,
            image_info.dimensions(),
            image_info.color_info(),
            props,
            initial_load_op,
            register_with_recorder,
            allow_unpremul,
        )
    }

    /// The core the recorder tracks.
    #[must_use]
    pub fn core(&self) -> &Rc<RefCell<DeviceCore>> {
        &self.core
    }

    // Synchronizes the SkDevice base state with the core and borrows the core.
    fn sync(&mut self) -> std::cell::RefMut<'_, DeviceCore> {
        let mut core = self.core.borrow_mut();
        if self.state.check_local_to_device_dirty() {
            core.set_local_to_device(self.state.local_to_device44(), self.state.local_to_device());
        }
        core.global_to_device = *self.state.global_to_device();
        core
    }

    /// `localToDeviceTransform()`.
    #[doc(alias = "localToDeviceTransform")]
    pub fn local_to_device_transform(&mut self) -> Transform {
        self.sync().cached_local_to_device
    }

    /// `flushPendingWork(nullptr)`: snaps all pending work from the `DrawContext` as a
    /// `RenderPassTask` and records it in the device's recorder.
    #[doc(alias = "flushPendingWork")]
    pub fn flush_pending_work(&mut self) {
        self.core.borrow_mut().flush_pending_work(None);
    }

    /// `target()`: may not be texturable, but includes the swizzle required when sampling or
    /// reading to CPU.
    #[must_use]
    pub fn target(&self) -> TextureProxyView {
        self.core.borrow().dc.target().clone()
    }

    /// `isTexturable()`.
    #[doc(alias = "isTexturable")]
    #[must_use]
    pub fn is_texturable(&self) -> bool {
        self.core.borrow().dc.is_texturable()
    }

    /// `isScratchDevice()`.
    #[doc(alias = "isScratchDevice")]
    #[must_use]
    pub fn is_scratch_device(&self) -> bool {
        self.core.borrow().is_scratch_device()
    }

    /// `lastDrawTask()`: only used for scratch devices.
    #[doc(alias = "lastDrawTask")]
    #[must_use]
    pub fn last_draw_task(&self) -> Option<TaskRef> {
        self.core.borrow().last_draw_task()
    }

    /// `hasPendingReads(texture)`: true if the device has pending reads to the given texture.
    #[doc(alias = "hasPendingReads")]
    #[must_use]
    pub fn has_pending_reads(&self, texture: &Arc<TextureProxy>) -> bool {
        TrackedDevice::has_pending_reads(&*self.core.borrow(), texture)
    }

    /// `resetStorageCache()`.
    #[doc(alias = "resetStorageCache")]
    pub fn reset_storage_cache(&mut self) {
        self.core
            .borrow_mut()
            .dc
            .storage_context()
            .borrow_mut()
            .reset_cache();
    }

    /// Flushes the pending draws into the device's `DrawTask` and returns the task (a
    /// `DrawContext::snapDrawTask()` after `internalFlush()`), without adding it to the recorder.
    /// For tests that inspect the `DrawPass` a device recorded.
    #[doc(alias = "snapDrawTask")]
    pub fn testing_only_snap_draw_task(&mut self) -> Option<TaskRef> {
        let mut core = self.sync();
        let recorder = core.recorder()?;
        core.internal_flush(&recorder);
        core.dc.snap_draw_task()
    }

    /// `testingOnly_pendingRenderSteps()`.
    #[doc(alias = "testingOnly_pendingRenderSteps")]
    #[must_use]
    pub fn testing_only_pending_render_steps(&self) -> usize {
        self.core.borrow().dc.pending_render_steps()
    }

    /// Reads the device's clip stack, for tests that check the element tree.
    pub fn testing_only_with_clip_stack<R>(&self, f: impl FnOnce(&ClipStack) -> R) -> R {
        f(self.core.borrow().clip())
    }

    /// Reads the device's `DrawContext`, for tests that check the pending draws.
    pub fn testing_only_with_draw_context<R>(&self, f: impl FnOnce(&DrawContext) -> R) -> R {
        f(&self.core.borrow().dc)
    }

    /// `drawEdgeAAQuad(rect, clip, aaFlags, color, mode)`.
    // Port of: src/gpu/graphite/Device.cpp#L1419-L1433 (chrome/m156)
    #[doc(alias = "drawEdgeAAQuad")]
    pub fn draw_edge_aa_quad(
        &mut self,
        rect: &SkRect,
        clip: Option<&[Point; 4]>,
        aa_flags: EdgeFlags,
        color: &Color4f,
        mode: skia_rust_core::blend_mode::BlendMode,
    ) {
        self.sync()
            .draw_edge_aa_quad(rect, clip, aa_flags, color, mode);
    }

    /// `drawEdgeAAImageSet(set, count, dstClips, preViewMatrices, sampling, paint, constraint)`.
    // Port of: src/gpu/graphite/Device.cpp#L1435-L1494 (chrome/m156)
    #[doc(alias = "drawEdgeAAImageSet")]
    pub fn draw_edge_aa_image_set(
        &mut self,
        set: &[ImageSetEntry],
        dst_clips: Option<&[Point]>,
        pre_view_matrices: Option<&[Matrix]>,
        sampling: &SamplingOptions,
        paint: &Paint,
        constraint: SrcRectConstraint,
    ) {
        self.sync().draw_edge_aa_image_set(
            set,
            dst_clips,
            pre_view_matrices,
            sampling,
            paint,
            constraint,
        );
    }
}

impl Drop for Device {
    // Port of: src/gpu/graphite/Device.cpp#L544-L557 (chrome/m156)
    fn drop(&mut self) {
        // In C++ the recorder owns a reference to every registered device, so a device the
        // client dropped is flushed by the recorder's next snap. Here the recorder holds devices
        // weakly, so a registered device is made immutable (flushing its pending work to the
        // recorder) as it goes. A scratch device that was not registered must go out of scope
        // before the recorder is snapped, and has no work to hand over.
        if self.registered
            && let Ok(mut core) = self.core.try_borrow_mut()
        {
            core.set_immutable();
        }
    }
}

impl DeviceCore {
    // Port of: src/gpu/graphite/Device.cpp#L441-L475 (chrome/m156)
    fn new(
        recorder: Weak<RecorderInner>,
        dc: DrawContext,
        this: Weak<RefCell<DeviceCore>>,
    ) -> Self {
        let width = dc.image_info().width();
        let height = dc.image_info().height();
        let dimensions = dc.image_info().dimensions();
        Self {
            recorder,
            dc,
            last_task: None,
            clip: Some(ClipStack::new(width, height)),
            color_depth_bounds_manager: Rc::new(RefCell::new(HybridBoundsManager::new(
                dimensions,
                GRID_CELL_SIZE,
                MAX_BRUTE_FORCE_N,
                MAX_GRID_SIZE,
            ))),
            disjoint_stencil_set: IntersectionTreeSet::default(),
            width,
            height,
            cached_local_to_device: Transform::new(M44::new_identity()),
            local_to_device44: M44::new_identity(),
            local_to_device33: Matrix::new_identity(),
            global_to_device: M44::new_identity(),
            current_depth: DrawOrder::K_CLEAR_DEPTH,
            atlased_path_count: 0,
            must_flush_dependencies: false,
            #[cfg(debug_assertions)]
            is_flushing: false,
            scoped_recording_id: 0,
            this,
        }
    }

    // Updates the cached local-to-device transform (`localToDeviceTransform()` when
    // `checkLocalToDeviceDirty()`).
    // Port of: src/gpu/graphite/Device.cpp#L658-L663 (chrome/m156)
    fn set_local_to_device(&mut self, local_to_device44: &M44, local_to_device33: &Matrix) {
        self.local_to_device44 = *local_to_device44;
        self.local_to_device33 = local_to_device33.clone();
        self.cached_local_to_device = Transform::new(*local_to_device44);
    }

    // The recorder, or `None` once it has been abandoned or dropped.
    pub(crate) fn recorder(&self) -> Option<Recorder> {
        self.recorder.upgrade().map(Recorder::from_inner)
    }

    /// `Device::makeSurface(ii, props)`: a render target of the recorder, with `props`.
    // Port of: src/gpu/graphite/Device.cpp#L695-L697 (chrome/m156)
    #[doc(alias = "makeSurface")]
    #[must_use]
    pub fn make_surface(&self, info: &ImageInfo, props: &SurfaceProps) -> Option<Surface> {
        let recorder = self.recorder()?;
        Surface::render_target(&recorder, info, Mipmapped::No, Some(props), "")
    }

    /// `Device::makeImageCopy(subset, budgeted, mipmapped, backingFit)`: the pending draws are
    /// flushed to the root task list, then `subset` is copied from the target.
    // Port of: src/gpu/graphite/Device.cpp#L698-L721 (chrome/m156)
    #[doc(alias = "makeImageCopy")]
    #[must_use]
    pub fn make_image_copy(
        &mut self,
        subset: IRect,
        budgeted: Budgeted,
        mipmapped: Mipmapped,
        backing_fit: BackingFit,
    ) -> Option<Image> {
        let recorder = self.recorder()?;
        // Although we have our own DrawContext here, we pass a nullptr to both flushPendingWork and
        // Image::Copy so that tasks end up on the root task list.
        self.flush_pending_work(None);
        let label = {
            let target_label = self.dc.target().proxy()?.label();
            if target_label.is_empty() {
                "CopyDeviceTexture".to_owned()
            } else {
                format!("{target_label}_DeviceCopy")
            }
        };
        GraphiteImage::copy(
            &recorder,
            None,
            self.dc.target(),
            self.dc.color_info(),
            subset,
            budgeted,
            mipmapped,
            backing_fit,
            &label,
        )
    }

    /// `Device::resetStorageCache()`'s body: the storage context drops its cached storage.
    // Port of: src/gpu/graphite/Device.cpp (`resetStorageCache`, chrome/m156)
    pub fn reset_storage_cache(&self) {
        self.dc.storage_context().borrow_mut().reset_cache();
    }

    fn clip(&self) -> &ClipStack {
        self.clip.as_ref().expect("the clip stack is not lent out")
    }

    // Calls `f` with the clip stack and the core as its hooks.
    fn with_clip<R>(&mut self, f: impl FnOnce(&mut ClipStack, &mut DeviceCore) -> R) -> R {
        let mut clip = self.clip.take().expect("the clip stack is not lent out");
        let result = f(&mut clip, self);
        self.clip = Some(clip);
        result
    }

    /// `abandonRecorder()`.
    // Port of: src/gpu/graphite/Device.h#L111-L113 (chrome/m156)
    pub fn abandon_recorder(&mut self) {
        self.recorder = Weak::new();
    }

    /// `isScratchDevice()`: scratch device status is inferred from whether or not the Device's
    /// target is instantiated. By default devices start out un-instantiated unless they are
    /// wrapping an existing backend texture (definitely not a scratch scenario), or Surface
    /// explicitly instantiates the target before returning to the client (not a scratch
    /// scenario).
    ///
    /// Scratch device targets are instantiated during the `prepareResources()` phase of
    /// `Recorder::snap()`. Truly scratch devices that have gone out of scope as intended will have
    /// already been destroyed at this point. Scratch devices that become longer-lived (linked to
    /// a client-owned object) automatically transition to non-scratch usage.
    // Port of: src/gpu/graphite/Device.cpp#L2559-L2571 (chrome/m156)
    #[must_use]
    #[allow(clippy::missing_panics_doc)] // the panics are SkASSERT-style invariants of the C++
    pub fn is_scratch_device(&self) -> bool {
        let proxy = self.dc.target().proxy().expect("a device has a target");
        !proxy.is_instantiated() && !proxy.is_lazy()
    }

    /// `lastDrawTask()`.
    // Port of: src/gpu/graphite/Device.cpp#L2343-L2346 (chrome/m156)
    #[must_use]
    pub fn last_draw_task(&self) -> Option<TaskRef> {
        debug_assert!(self.is_scratch_device());
        self.last_task.clone()
    }

    /// `setImmutable()`: flushes any pending work to the recorder and then deregisters and
    /// abandons the recorder.
    // Port of: src/gpu/graphite/Device.cpp#L575-L591 (chrome/m156)
    pub fn set_immutable(&mut self) {
        if let Some(recorder) = self.recorder() {
            // Push any pending work to the Recorder now. setImmutable() is only called by the
            // destructor of a client-owned Surface, or explicitly in layer/filtering workflows.
            // In both cases this is restricted to the Recorder's thread. This is in contrast to
            // ~Device(), which might be called from another thread if it was linked to an Image
            // used in multiple recorders.
            self.flush_pending_work(None);
            if let Some(this) = self.this.upgrade() {
                let tracked: Rc<RefCell<dyn TrackedDevice>> = this;
                recorder.priv_().deregister_device(&Rc::downgrade(&tracked));
            }
            // Abandoning the recorder ensures that there are no further operations that can be
            // recorded and is relied on by Image::notifyInUse() to detect when it can unlink
            // from a Device.
            self.abandon_recorder();
            // TODO (b/540923063): Remove once resetting storage cache can be placed directly
            // into flushPendingWork()
            self.dc.storage_context().borrow_mut().reset_cache();
        }
    }

    /// `notifyInUse(recorder, drawContext)`: called by an Image wrapping this Device to mark that
    /// the pending contents of this Device will be read by `recorder`, and specifically by
    /// `draw_context` (if any). Flushes any necessary work (depending on scratch state) and
    /// records task dependencies. Returns true if the caller does not need to track the Device on
    /// the Image anymore.
    ///
    /// `is_unique` is `this->unique()`: whether nothing else references the device.
    // Port of: src/gpu/graphite/Device.cpp#L593-L652 (chrome/m156)
    #[allow(clippy::missing_panics_doc)] // the panics are SkASSERT-style invariants of the C++
    pub fn notify_in_use(
        &mut self,
        recorder: &Recorder,
        mut draw_context: Option<&mut DrawContext>,
        is_unique: bool,
    ) -> bool {
        if self.is_scratch_device() {
            if let Some(last_task) = self.last_task.clone() {
                // Increment the pending read count for the device's target
                recorder
                    .priv_()
                    .add_pending_read(self.dc.target().proxy().expect("a device has a target"));
                if let Some(draw_context) = draw_context.as_deref_mut() {
                    // Add a reference to the device's drawTask to `drawContext` if that's
                    // provided.
                    draw_context.record_dependency(last_task);
                } else {
                    // If there's no `drawContext` this notify represents a copy, so for now
                    // append the task to the root task list since that is where the subsequent
                    // copy task will go as well.
                    recorder.priv_().add(last_task);
                }
            } else {
                // If there's no draw task yet, there are two possible scenarios:
                //
                // 1) the device is being drawn into a child scratch device (backdrop filter or
                //    init-from-prev layer), and the child will later on be drawn back into the
                //    device's `drawContext`. In this case `device` should already have performed
                //    an internal flush and have no pending work, and not yet be marked
                //    immutable. The correct action at this point in time is to do nothing: the
                //    final task order in the device's DrawTask will be pre-notified tasks into
                //    the device's target, then the child's DrawTask when it's drawn back into
                //    `device`, and then any post tasks that further modify the `device`'s
                //    target.
                // 2) the scratch device was flushed to a drawContext's local task list,
                //    resulting in no pending work but also no lastTask. The correct action is
                //    again to do nothing. In this case, it is also possible that the device was
                //    not registered with the recorder.
                debug_assert!(
                    self.recorder.upgrade().is_none()
                        || std::ptr::eq(self.recorder.as_ptr(), recorder_inner_ptr(recorder))
                );
            }

            // Scratch devices are often already marked immutable, but they are also the way in
            // which Image finds the last snapped DrawTask so we don't unlink scratch devices.
            // The scratch image view will be short-lived as well, or the device will transition
            // to a non-scratch device in a future Recording and then it will be unlinked then.
            // Thus, we always return false for scratch devices so they are not unlinked from
            // their images.
            false
        } else {
            // Automatic flushing of image views only happens when mixing reads and writes on the
            // originating Recorder. Draws of the view on another Recorder will always see the
            // texture content dependent on how Recordings are inserted.
            let same_recorder = self.recorder.upgrade().is_some()
                && std::ptr::eq(self.recorder.as_ptr(), recorder_inner_ptr(recorder));
            if same_recorder {
                // Non-scratch devices push their tasks to the root task list to maintain an order
                // consistent with the client-triggering actions. Because of this, there's no need
                // to add references to the `drawContext` that the device is being drawn into.
                self.flush_pending_work(None);

                if draw_context.is_some() {
                    // But if we are being drawn into another context, remember that there is an
                    // outstanding dependency on the current state of this device, in which case
                    // it's next flush must also flush those other devices before its new tasks
                    // are added.
                    self.must_flush_dependencies = true;
                }
            }
            // Return true (to unlink with the image) if the non-scratch surface is immutable
            // since this Device cannot record any more commands that will modify its texture.
            self.recorder.upgrade().is_none() || is_unique
        }
    }

    /// `flushPendingWork(drawContext)`: ensures clip elements are drawn that will clip previous
    /// draw calls, snaps all pending work from the `DrawContext` as a `RenderPassTask` and
    /// records it in the Device's recorder.
    ///
    /// The behavior of this function depends on whether a `draw_context` is provided:
    /// - If a drawContext is provided, then any flushed tasks will be added to that
    ///   drawContext's task list. Note, no lastTask will be recorded in this case.
    /// - Else, flushed tasks are added to the root task list, and if this device is a scratch
    ///   device, the last task will be recorded.
    // Port of: src/gpu/graphite/Device.cpp#L2348-L2403 (chrome/m156)
    #[allow(clippy::missing_panics_doc)] // the panics are SkASSERT-style invariants of the C++
    pub fn flush_pending_work(&mut self, draw_context: Option<&mut DrawContext>) {
        // If this is a scratch device being flushed, it should only be flushing into the expected
        // next recording from when the Device was first created.
        let Some(recorder) = self.recorder() else {
            return;
        };
        debug_assert!(
            self.scoped_recording_id == 0
                || self.scoped_recording_id == recorder.priv_().next_recording_id()
        );

        // Ideally we would just check if `drawTask` was non-null and then call
        // flushTrackedDevices() before we appended `drawTask` afterwards. Unfortunately,
        // internalFlush() is not 100% internal because it can record atlas uploads to the
        // DrawContext. If those uploads were moved to `drawTask` before flushTrackedDevices() is
        // called, any other Devices would incorrectly assume that the uploads would be executed
        // before their tasks, even though that's not the case here.
        if self.dc.modifies_target() && self.must_flush_dependencies {
            // If this is a client-owned Device that has also been used as an image in the same
            // Recorder we need flush all tracked devices that have pending reads from this
            // Device, because those need to be resolved *before* `drawTask` would be executed
            // and modify its texture state.
            self.must_flush_dependencies = false;
            let target = self.dc.target().ref_proxy().expect("a device has a target");
            recorder
                .priv_()
                .flush_tracked_devices_with_dependency(&target);
        }

        // While unbounded recursion is gone, bounded re-entrant flushing is still possible during
        // dependency resolution. We assert *after* the dependency flush to permit this valid
        // re-entry.
        #[cfg(debug_assertions)]
        {
            debug_assert!(!self.is_flushing);
            self.is_flushing = true;
        }

        self.internal_flush(&recorder);
        let draw_task = self.dc.snap_draw_task();
        if let Some(draw_context) = draw_context {
            if let Some(draw_task) = draw_task {
                draw_context.record_dependency(draw_task);
            }
        } else {
            if self.is_scratch_device() {
                // TODO(b/323887221): Once shared atlas resources are less brittle, scratch
                // devices won't flush to the recorder at all and will only store the snapped task
                // here.
                self.last_task.clone_from(&draw_task);
            } else {
                // Non-scratch devices do not need to point back to the last snapped task since
                // they are always added to the root task list.
                // TODO: It is currently possible for scratch devices to be flushed and
                // instantiated before their work is finished, meaning they will produce
                // additional tasks to be included in a follow-up Recording. However, in this
                // case they no longer appear scratch because the first Recording instantiated
                // the targets. When scratch devices are not actually registered with the
                // Recorder and are only included when they are drawn (e.g. restored), we should
                // be able to assert that `fLastTask` is null.
                self.last_task = None;
            }

            if let Some(draw_task) = draw_task {
                recorder.priv_().add(draw_task);
            }
        }

        #[cfg(debug_assertions)]
        {
            self.is_flushing = false;
        }
    }

    /// `internalFlush()`: flushes internal work, such as pending clip draws and atlas uploads,
    /// into the Device's `DrawTask`.
    // Port of: src/gpu/graphite/Device.cpp#L2409-L2430 (chrome/m156)
    fn internal_flush(&mut self, recorder: &Recorder) {
        // (The atlas provider would record its pending uploads that pending draws reference here
        // (G12a).)

        // Clip shapes are depth-only draws, but aren't recorded in the DrawContext until a flush
        // in order to determine the Z values for each element.
        self.with_clip(|clip, hooks| clip.record_deferred_clip_draws(hooks));

        // Flush all pending items to the internal task list and reset Device tracking state
        self.dc.flush(&recorder.priv_());

        self.color_depth_bounds_manager.borrow_mut().reset();
        self.disjoint_stencil_set.reset();
        self.current_depth = DrawOrder::K_CLEAR_DEPTH;
        self.atlased_path_count = 0;

        // (Any cleanup in the AtlasProvider (G12a).)
    }

    // Port of: src/gpu/graphite/Device.cpp#L2432-L2442 (chrome/m156)
    fn needs_flush_before_draw(
        &self,
        mut num_new_render_steps: usize,
        dst_read_strategy: DstReadStrategy,
    ) -> bool {
        // Must also account for the elements in the clip stack that might need to be recorded.
        num_new_render_steps +=
            self.clip().max_deferred_clip_draws() * crate::graphite::renderer::MAX_RENDER_STEPS;
        // Need flush if we don't have room to record into the current list.
        (MAX_RENDER_STEPS - self.dc.pending_render_steps()) < num_new_render_steps
            // Need flush if this draw needs to copy the dst surface for reading.
            || dst_read_strategy == DstReadStrategy::TextureCopy
    }

    // The `SkDevice::localToDevice()` that C++ reads off `this`.
    fn local_to_device_transform(&self) -> Transform {
        self.cached_local_to_device
    }

    ////////////////////////////////////////////////////////////////////////////
    // Clipping

    // Port of: src/gpu/graphite/Device.cpp#L804-L816 (chrome/m156)
    fn is_clip_anti_aliased(&self) -> bool {
        // All clips are AA'ed unless it's wide-open, empty, or a device-rect with integer
        // coordinates
        let ty = self.clip().clip_state();
        if ty == ClipState::WideOpen || ty == ClipState::Empty {
            false
        } else if ty == ClipState::DeviceRect {
            let rect = self
                .clip()
                .elements()
                .next()
                .expect("a device-rect clip has an element");
            debug_assert!(
                rect.shape.is_rect() && rect.local_to_device.type_() == TransformType::Identity
            );
            *rect.shape.rect() != rect.shape.rect().make_round_out()
        } else {
            true
        }
    }

    // Port of: src/gpu/graphite/Device.cpp#L841-L846 (chrome/m156)
    fn clip_rect(&mut self, rect: &SkRect, op: ClipOp, aa: bool) {
        debug_assert!(op == ClipOp::Intersect || op == ClipOp::Difference);
        let snapping = if aa {
            PixelSnapping::No
        } else {
            PixelSnapping::Yes
        };
        let transform = self.local_to_device_transform();
        let shape = Shape::from_rect(Rect::from_sk_rect(rect));
        self.with_clip(|clip, hooks| clip.clip_shape(hooks, &transform, &shape, op, snapping));
    }

    // Port of: src/gpu/graphite/Device.cpp#L847-L852 (chrome/m156)
    fn clip_rrect(&mut self, rrect: &RRect, op: ClipOp, aa: bool) {
        debug_assert!(op == ClipOp::Intersect || op == ClipOp::Difference);
        let snapping = if aa {
            PixelSnapping::No
        } else {
            PixelSnapping::Yes
        };
        let transform = self.local_to_device_transform();
        let shape = Shape::from_rrect(*rrect);
        self.with_clip(|clip, hooks| clip.clip_shape(hooks, &transform, &shape, op, snapping));
    }

    // Port of: src/gpu/graphite/Device.cpp#L853-L860 (chrome/m156)
    fn clip_path(&mut self, path: &Path, op: ClipOp) {
        debug_assert!(op == ClipOp::Intersect || op == ClipOp::Difference);
        // TODO: Ensure all path inspection is handled here or in SkCanvas, and that non-AA rects
        // as paths are routed appropriately.
        // TODO: Must also detect paths that are lines so the clip stack can be set to empty
        let transform = self.local_to_device_transform();
        let shape = Shape::from_path(path.clone());
        self.with_clip(|clip, hooks| {
            clip.clip_shape(hooks, &transform, &shape, op, PixelSnapping::No);
        });
    }

    // Port of: src/gpu/graphite/Device.cpp#L866-L881 (chrome/m156)
    fn clip_region(&mut self, global_rgn: &Region, op: ClipOp) {
        debug_assert!(op == ClipOp::Intersect || op == ClipOp::Difference);

        let global_to_device = Transform::new(self.global_to_device);

        if global_rgn.is_empty() {
            self.with_clip(|clip, hooks| {
                clip.clip_shape(
                    hooks,
                    &global_to_device,
                    &Shape::default(),
                    op,
                    PixelSnapping::No,
                );
            });
        } else if global_rgn.is_rect() {
            let shape = Shape::from_rect(Rect::from_sk_irect(global_rgn.bounds()));
            self.with_clip(|clip, hooks| {
                clip.clip_shape(hooks, &global_to_device, &shape, op, PixelSnapping::Yes);
            });
        } else {
            // TODO: Can we just iterate the region and do non-AA rects for each chunk?
            let mut builder = skia_rust_core::path_builder::PathBuilder::new();
            global_rgn.add_boundary_path(&mut builder);
            let shape = Shape::from_path(builder.detach());
            self.with_clip(|clip, hooks| {
                clip.clip_shape(hooks, &global_to_device, &shape, op, PixelSnapping::No);
            });
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Drawing

    // Port of: src/gpu/graphite/Device.cpp#L900-L912 (chrome/m156)
    fn draw_paint(&mut self, paint: &Paint) {
        let mut inverse_fill = Shape::default(); // defaults to empty
        inverse_fill.set_inverted(true);
        // An empty shape with an inverse fill completely floods the clip
        debug_assert!(inverse_fill.is_flood_fill());

        let transform = self.local_to_device_transform();
        self.draw_geometry(
            &transform,
            Geometry::Shape(inverse_fill),
            &PaintParams::new(paint, None, false, false),
            &default_fill_style(),
        );
    }

    // Port of: src/gpu/graphite/Device.cpp#L914-L935 (chrome/m156)
    fn draw_rect(&mut self, r: &SkRect, paint: &Paint) {
        let mut rect_to_draw = Rect::from_sk_rect(r);
        let mut style = StrokeRec::from_paint(paint, None, None);
        let transform = self.local_to_device_transform();
        if !paint.is_anti_alias() {
            // Graphite assumes everything is anti-aliased. In the case of axis-aligned non-aa
            // requested rectangles, we snap the local geometry to land on pixel boundaries to
            // emulate non-aa.
            if style.is_fill_style() {
                rect_to_draw = snap_rect_to_pixels(&transform, &rect_to_draw, None);
            } else {
                let stroke_and_fill = style.style() == StrokeStyleKind::StrokeAndFill;
                let mut stroke_width = style.width();
                rect_to_draw =
                    snap_rect_to_pixels(&transform, &rect_to_draw, Some(&mut stroke_width));
                style.set_stroke_style(stroke_width, stroke_and_fill);
            }
        }
        self.draw_geometry_with_path_effect(
            &transform,
            Geometry::Shape(Shape::from_rect(rect_to_draw)),
            &PaintParams::new(paint, None, false, false),
            style,
            paint.path_effect().as_ref(),
        );
    }

    // Port of: src/gpu/graphite/Device.cpp#L937-L950 (chrome/m156)
    fn draw_vertices(
        &mut self,
        vertices: &Vertices,
        blender: &Blender,
        paint: &Paint,
        skip_color_xform: bool,
    ) {
        // A null blender is normally equivalent to SrcOver; coerce it to non-null so that nullity
        // can be used by PaintParamsKeyBuilder to know when to add primitive blending blocks.
        // Use null for the primitive blender if `vertices` does not have per-vertex colors.
        let primitive_blender = vertices.has_colors().then_some(blender);
        let transform = self.local_to_device_transform();
        self.draw_geometry(
            &transform,
            Geometry::Vertices(vertices.clone()),
            &PaintParams::new(paint, primitive_blender, skip_color_xform, false),
            &default_fill_style(),
        );
    }

    // Port of: src/gpu/graphite/Device.cpp#L1007-L1070 (chrome/m156)
    fn draw_mesh(&mut self, mesh: &Mesh, blender: &Blender, paint: &Paint) {
        if !mesh.is_valid() {
            return;
        }
        let Some(spec) = mesh.spec() else {
            return;
        };

        // The caller could modify its CPU buffers after the draw, so the draw copies the data it
        // reads: the vertices from the vertex offset and the indices from the index offset.
        let vertex_size = mesh.vertex_count() * spec.stride();
        let vertex_offset = mesh.vertex_offset();
        let vertex_bytes = mesh
            .vertex_buffer()
            .expect("a valid mesh has a vertex buffer")
            .with_data(|data| data[vertex_offset..vertex_offset + vertex_size].to_vec());
        let vb = mesh::meshes::make_vertex_buffer(Some(&vertex_bytes), vertex_size);
        let uniforms = mesh.uniforms().cloned();

        let result = if let Some(ib) = mesh.index_buffer() {
            let index_size = mesh.index_count() * std::mem::size_of::<u16>();
            let index_offset = mesh.index_offset();
            let index_bytes =
                ib.with_data(|data| data[index_offset..index_offset + index_size].to_vec());
            let ib = mesh::meshes::make_index_buffer(Some(&index_bytes), index_size);
            Mesh::make_indexed(
                Some(spec.clone()),
                mesh.mode(),
                Some(vb),
                mesh.vertex_count(),
                0,
                Some(ib),
                mesh.index_count(),
                0,
                uniforms,
                mesh.children(),
                mesh.bounds(),
            )
        } else {
            Mesh::make(
                Some(spec.clone()),
                mesh.mode(),
                Some(vb),
                mesh.vertex_count(),
                0,
                uniforms,
                mesh.children(),
                mesh.bounds(),
            )
        };
        let draw_mesh = result.mesh;

        // A null blender is only used for the primitive color if the mesh has colors.
        let primitive_blender = mesh_priv::has_colors(spec).then_some(blender);
        let transform = self.local_to_device_transform();
        self.draw_geometry(
            &transform,
            Geometry::Mesh(draw_mesh),
            &PaintParams::new(paint, primitive_blender, false, false).make_with_mesh(mesh),
            &default_fill_style(),
        );
    }

    // Port of: src/gpu/graphite/Device.cpp#L1072-L1098 (chrome/m156)
    fn draw_image_lattice_patch(&mut self, patch_dst: &SkRect, color: Color, paint: &Paint) {
        // Use non-AA quads to match Ganesh and Raster backends behavior of drawImageLattice.
        let paint_params = PaintParams::from_paint_with_color(paint, Color4f::from(color));
        let transform = self.local_to_device_transform();
        self.draw_geometry(
            &transform,
            Geometry::EdgeAAQuad(EdgeAAQuad::from_sk_rect(patch_dst, EdgeFlags::NONE)),
            &paint_params,
            &default_fill_style(),
        );
    }

    // Port of: src/gpu/graphite/Device.cpp#L1100-L1132 (chrome/m156)
    fn draw_atlas(
        &mut self,
        xform: &[RSXform],
        tex: &[SkRect],
        colors: &[Color],
        blender: &Blender,
        paint: &Paint,
    ) {
        if xform.is_empty() {
            return;
        }

        let primitive_blender = blender;
        let base_params = PaintParams::new(paint, None, false, false);
        for (i, (xform, r)) in xform.iter().zip(tex).enumerate() {
            let mut mat = Matrix::new_identity();
            mat.set_rsxform(xform);
            // The rect defined by tex[i] is in the atlas's coordinate space; we perform the
            // translation so drawAtlas's rects are drawn at (0, 0, r.width(), r.height()).
            mat.pre_translate((-r.left, -r.top));

            // Ensure we supply a non-null blender if there is a color defined so the
            // SoliderColorShaderBlock is still created for the primitive color.
            let params = if colors.is_empty() {
                base_params.clone()
            } else {
                base_params
                    .make_with_primitive_color(Some(primitive_blender), Color4f::from(colors[i]))
            };
            let local_to_device =
                Transform::new(M44::concat(&self.local_to_device44, &M44::from(&mat)));
            self.draw_geometry(
                &local_to_device,
                Geometry::Shape(Shape::from_rect(Rect::from_sk_rect(r))),
                &params,
                &default_fill_style(),
            );
        }
    }

    // Port of: src/gpu/graphite/Device.cpp#L1134-L1148 (chrome/m156)
    fn draw_oval(&mut self, oval: &SkRect, paint: &Paint) {
        if let Some(path_effect) = paint.path_effect() {
            // Dashing requires that the oval path starts on the right side and travels clockwise.
            // This is the default for the SkPath::Oval constructor, as used by SkBitmapDevice.
            let transform = self.local_to_device_transform();
            self.draw_geometry_with_path_effect(
                &transform,
                Geometry::Shape(Shape::from_path(Path::oval(oval, None))),
                &PaintParams::new(paint, None, false, false),
                StrokeRec::from_paint(paint, None, None),
                Some(&path_effect),
            );
        } else {
            // TODO: This has wasted effort from the SkCanvas level since it instead converts
            // rrects that happen to be ovals into this, only for us to go right back to rrect.
            self.draw_rrect(&RRect::new_oval(oval), paint);
        }
    }

    // Port of: src/gpu/graphite/Device.cpp#L1150-L1169 (chrome/m156)
    fn draw_arc(&mut self, arc: &skia_rust_core::arc::Arc, paint: &Paint) {
        // For sweeps >= 360°, simple fills and simple strokes without the center point or square
        // caps are ovals. Culling these here simplifies the path processing in Shape.
        if paint.path_effect().is_none()
            && arc.sweep_angle.abs() >= 360.0
            && (paint.style() == PaintStyle::Fill
                || (paint.style() == PaintStyle::Stroke
                    // square caps can stick out from the shape so we can't do this with an rrect
                    // draw
                    && paint.stroke_cap() != Cap::Square
                    // wedge cases with strokes will draw lines to the center
                    && !arc.is_wedge()))
        {
            self.draw_rrect(&RRect::new_oval(arc.oval), paint);
        } else {
            let transform = self.local_to_device_transform();
            self.draw_geometry_with_path_effect(
                &transform,
                Geometry::Shape(Shape::from_arc(*arc)),
                &PaintParams::new(paint, None, false, false),
                StrokeRec::from_paint(paint, None, None),
                paint.path_effect().as_ref(),
            );
        }
    }

    // Port of: src/gpu/graphite/Device.cpp#L1171-L1202 (chrome/m156)
    fn draw_rrect(&mut self, rr: &RRect, paint: &Paint) {
        let mut rrect_to_draw = Shape::default();
        let mut style = StrokeRec::from_paint(paint, None, None);
        let transform = self.local_to_device_transform();

        if paint.is_anti_alias() {
            rrect_to_draw.set_rrect(*rr);
        } else {
            // Snap the horizontal and vertical edges of the rounded rectangle to pixel edges to
            // match the behavior of drawRect(rr.bounds()), to partially emulate non-AA rendering
            // while preserving the anti-aliasing of the curved corners.
            let snapped_bounds;
            if style.is_fill_style() {
                snapped_bounds =
                    snap_rect_to_pixels(&transform, &Rect::from_sk_rect(rr.rect()), None);
            } else {
                let stroke_and_fill = style.style() == StrokeStyleKind::StrokeAndFill;
                let mut stroke_width = style.width();
                snapped_bounds = snap_rect_to_pixels(
                    &transform,
                    &Rect::from_sk_rect(rr.rect()),
                    Some(&mut stroke_width),
                );
                style.set_stroke_style(stroke_width, stroke_and_fill);
            }

            let mut snapped_rrect = RRect::default();
            snapped_rrect.set_rect_radii(snapped_bounds.as_sk_rect(), rr.radii_ref());
            rrect_to_draw.set_rrect(snapped_rrect);
        }

        self.draw_geometry_with_path_effect(
            &transform,
            Geometry::Shape(rrect_to_draw),
            &PaintParams::new(paint, None, false, false),
            style,
            paint.path_effect().as_ref(),
        );
    }

    // Port of: src/gpu/graphite/Device.cpp#L1204-L1329 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    #[allow(clippy::similar_names)] // mirrors the C++ names strokeRect/strokeRRect
    #[allow(clippy::needless_range_loop)] // indexes the outer, inner and stroke corners in parallel as the C++ does
    fn draw_drrect(&mut self, outer: &RRect, inner: &RRect, paint: &Paint) {
        // If there's a path effect or inverse fill, fall back to path rendering
        if paint.path_effect().is_some() || paint.style() != PaintStyle::Fill {
            // `this->SkDevice::drawDRRect(outer, inner, paint)`
            let mut builder = skia_rust_core::path_builder::PathBuilder::new();
            builder.add_rrect(outer, None, None);
            builder.add_rrect(inner, None, None);
            builder.set_fill_type(skia_rust_core::path_types::PathFillType::EvenOdd);
            builder.set_is_volatile(true);
            let path = builder.detach();
            self.draw_path(&path, paint);
            return;
        }

        // This holds the positive insets from `outer` to `inner`
        let mut stroke_rect = Rect::from_sk_rect(outer.rect());
        let gap = Rect::from_sk_rect(inner.rect()).vals() - stroke_rect.vals();
        let transform = self.local_to_device_transform();
        let tolerance = Shape::DEFAULT_PIXEL_TOLERANCE * transform.local_aa_radius(&stroke_rect);
        let stroke_width = gap[0];
        let all_valid =
            (0..4).all(|i| gap[i] > tolerance && (gap[i] - stroke_width).abs() <= tolerance);
        if all_valid {
            // The shape is possibly expressible as a stroked [r]rect.
            let stroke_radius = 0.5 * stroke_width;
            stroke_rect.inset(stroke_radius);

            let mut stroke_paint = paint.clone();
            stroke_paint.set_stroke(true);
            stroke_paint.set_stroke_width(stroke_width);
            stroke_paint.set_stroke_cap(Cap::Butt);
            stroke_paint.set_stroke_miter(4.0); // large enough to not trigger bevels for 90 degree corners

            // Check the corners to see if they are equivalent to a stroked round rect. If `outer`
            // has rounded corners, they must be circular and be at least `strokeRadius`. An outer
            // corner can have a 0 radius if we assume a miter join style, but if an outer corner
            // is exactly the stroke radius then we have to use a round join style. The matching
            // corners of `inner` must also be rounded, and be exactly strokeWidth less, or they
            // must be 0 if the difference would be negative.
            let mut valid_corners = 0;
            let mut rect_corners = 0;
            let mut stroke_corners = [Point::default(); 4];
            let mut required_join: Option<skia_rust_core::paint::Join> = None;
            for i in 0..4 {
                let outer_corner_radii = outer.radii_ref()[i];
                let inner_corner_radii = inner.radii_ref()[i];

                let mut stroke_corner;
                if !rrect_priv::is_relatively_circular(
                    outer_corner_radii.x,
                    outer_corner_radii.y,
                    tolerance,
                ) {
                    // Not circular; a stroked ellipse is not just a larger ellipse
                    break;
                } else if outer_corner_radii.x.abs() <= tolerance {
                    // A rectangular outer corner requires miter joins
                    if required_join.is_some_and(|j| j != skia_rust_core::paint::Join::Miter) {
                        break;
                    }
                    required_join = Some(skia_rust_core::paint::Join::Miter);
                    stroke_corner = 0.0;
                    rect_corners += 1;
                } else {
                    stroke_corner = outer_corner_radii.x - stroke_radius;
                    if stroke_corner < -tolerance {
                        // Corner is rounded but less than the stroke radius, which isn't
                        // representable
                        break;
                    } else if stroke_corner <= tolerance {
                        // Corner is rounded to the stroke radius, which can only be represented
                        // as an underlying rect corner and round join
                        if required_join.is_some_and(|j| j != skia_rust_core::paint::Join::Round) {
                            break;
                        }
                        required_join = Some(skia_rust_core::paint::Join::Round);
                        stroke_corner = 0.0;
                        rect_corners += 1;
                    }
                }

                let expected_inner_radius = (stroke_corner - stroke_radius).max(0.0);
                if !rrect_priv::is_relatively_circular(
                    inner_corner_radii.x,
                    expected_inner_radius,
                    tolerance,
                ) || !rrect_priv::is_relatively_circular(
                    inner_corner_radii.y,
                    expected_inner_radius,
                    tolerance,
                ) {
                    // Inner corner doesn't match expectation
                    break;
                }

                stroke_corners[i] = Point::new(stroke_corner, stroke_corner);
                valid_corners += 1;
            }

            if valid_corners == 4 {
                stroke_paint
                    .set_stroke_join(required_join.unwrap_or(skia_rust_core::paint::Join::Round));
                if rect_corners == 4 {
                    self.draw_rect(&stroke_rect.as_sk_rect(), &stroke_paint);
                } else {
                    let mut stroke_rounded = RRect::default();
                    stroke_rounded.set_rect_radii(stroke_rect.as_sk_rect(), &stroke_corners);
                    self.draw_rrect(&stroke_rounded, &stroke_paint);
                }
                return;
            }
            // Otherwise fall through to draw+clip handling
        } else {
            // Either not approximately equal insets on all sides, or it would create a hairline
            // stroke which is something that should be requested via paint style.
            //
            // But we can try subtracting inner from outer if they are both rectangular to see if
            // it leaves a valid filled rect.
            let mut diff = SkRect::default();
            if outer.is_rect()
                && inner.is_rect()
                && rect_priv::subtract(outer.rect(), inner.rect(), &mut diff)
            {
                self.draw_rect(&diff, paint);
                return;
            }

            // Fall through to the draw+clip handling
        }

        // To avoid path rendering, treat DRRects as a drawRRect(outer) with a
        // clipRRect(inner, kDiff)
        self.with_clip(|clip, _hooks| clip.save());
        let shape = if inner.is_rect() {
            Shape::from_rect(Rect::from_sk_rect(inner.rect()))
        } else {
            Shape::from_rrect(*inner)
        };
        let snapping = if paint.is_anti_alias() {
            PixelSnapping::No
        } else {
            PixelSnapping::Yes
        };
        self.with_clip(|clip, hooks| {
            clip.clip_shape(hooks, &transform, &shape, ClipOp::Difference, snapping);
        });
        if outer.is_rect() {
            self.draw_rect(outer.rect(), paint);
        } else {
            self.draw_rrect(outer, paint);
        }
        self.with_clip(|clip, hooks| clip.restore(hooks));
    }

    // Port of: src/gpu/graphite/Device.cpp#L1331-L1383 (chrome/m156)
    fn draw_path(&mut self, path: &Path, paint: &Paint) {
        // Alternatively, we could move this analysis to SkCanvas. Also, we could consider
        // applying the path effect, being careful about starting point and direction.
        if paint.path_effect().is_none() && !path.is_inverse_fill_type() {
            if let Some(line_pts) = path.is_line() {
                // A line has zero area, so stroke and stroke-and-fill are equivalent
                if paint.style() != PaintStyle::Fill {
                    self.draw_points(PointMode::Lines, &[line_pts.0, line_pts.1], paint);
                } // and if it's fill, nothing is drawn
                return;
            }
            if let Some(oval) = path.is_oval() {
                self.draw_oval(&oval, paint);
                return;
            }
            if let Some(rrect) = path.is_rrect() {
                self.draw_rrect(&rrect, paint);
                return;
            }
            // For rects, if the path is not explicitly closed and the paint style is stroked then
            // it represents a rectangle with only 3 sides rasterized (and with any caps). If it's
            // filled or is closed+stroked, then the path renders identically to the rectangle.
            if let Some((rect, is_closed, _dir)) = path.is_rect()
                && (paint.style() == PaintStyle::Fill || is_closed)
            {
                self.draw_rect(&rect, paint);
                return;
            }
            // Detect filled nested rect contours
            if paint.style() == PaintStyle::Fill
                && let Some((rects, dirs)) =
                    skia_rust_core::path_priv::is_nested_fill_rects_path(path)
            {
                // For winding fills with contours going the same direction, there isn't any
                // cutout
                if path.fill_type() == skia_rust_core::path_types::PathFillType::Winding
                    && dirs[0] == dirs[1]
                {
                    self.draw_rect(&rects[0], paint);
                    return;
                }
                // The inner is cut out from the outer rect. Delegate to drawDRRect.
                self.draw_drrect(
                    &RRect::new_rect(rects[0]),
                    &RRect::new_rect(rects[1]),
                    paint,
                );
                return;
            }
        }

        // Full path rendering required
        let transform = self.local_to_device_transform();
        self.draw_geometry_with_path_effect(
            &transform,
            Geometry::Shape(Shape::from_path(path.clone())),
            &PaintParams::new(paint, None, false, false),
            StrokeRec::from_paint(paint, None, None),
            paint.path_effect().as_ref(),
        );
    }

    // Port of: src/gpu/graphite/Device.cpp#L1385-L1417 (chrome/m156)
    fn draw_points(&mut self, mode: PointMode, points: &[Point], paint: &Paint) {
        if points.is_empty() {
            return;
        }
        let mut count = points.len();

        let mut stroke = StrokeRec::from_paint(paint, Some(PaintStyle::Stroke), None);
        let mut next = 0;
        if mode == PointMode::Points {
            // Treat kPoints mode as stroking zero-length path segments, which produce caps so
            // that both hairlines and round vs. square geometry are handled entirely on the GPU.
            // TODO: SkCanvas should probably do the butt to square cap correction.
            if paint.stroke_cap() == Cap::Butt {
                stroke.set_stroke_params(Cap::Square, paint.stroke_join(), paint.stroke_miter());
            }
        } else {
            next = 1;
            count -= 1;
        }

        let paint_params = PaintParams::new(paint, None, false, false);
        let inc = if mode == PointMode::Lines { 2 } else { 1 };
        let transform = self.local_to_device_transform();
        let mut i = 0;
        while i < count {
            self.draw_geometry_with_path_effect(
                &transform,
                Geometry::Shape(Shape::new_line(
                    Float2::new(points[i].x, points[i].y),
                    Float2::new(points[i + next].x, points[i + next].y),
                )),
                &paint_params,
                stroke,
                paint.path_effect().as_ref(),
            );
            i += inc;
        }
    }

    // Port of: src/gpu/graphite/Device.cpp#L1419-L1433 (chrome/m156)
    fn draw_edge_aa_quad(
        &mut self,
        rect: &SkRect,
        clip: Option<&[Point; 4]>,
        aa_flags: EdgeFlags,
        color: &Color4f,
        mode: skia_rust_core::blend_mode::BlendMode,
    ) {
        // NOTE: We do not snap edge AA quads that are fully non-AA because we need their edges to
        // seam with quads that have mixed edge flags (so both need to match the GPU
        // rasterization, not our CPU rounding).
        let quad = match clip {
            Some(clip) => EdgeAAQuad::from_points(clip, aa_flags),
            None => EdgeAAQuad::from_sk_rect(rect, aa_flags),
        };
        let transform = self.local_to_device_transform();
        self.draw_geometry(
            &transform,
            Geometry::EdgeAAQuad(quad),
            &PaintParams::from_color(*color, mode),
            &default_fill_style(),
        );
    }

    // Port of: src/gpu/graphite/Device.cpp#L1435-L1494 (chrome/m156)
    fn draw_edge_aa_image_set(
        &mut self,
        set: &[ImageSetEntry],
        dst_clips: Option<&[Point]>,
        pre_view_matrices: Option<&[Matrix]>,
        sampling: &SamplingOptions,
        paint: &Paint,
        constraint: SrcRectConstraint,
    ) {
        debug_assert!(!set.is_empty());

        let local_to_device = self.local_to_device_transform();
        let mut dst_clip_index = 0;
        for entry in set {
            // If the entry is clipped by 'dstClips', that must be provided
            debug_assert!(!entry.has_clip || dst_clips.is_some());
            // Similarly, if it has an extra transform, those must be provided
            debug_assert!(entry.matrix_index < 0 || pre_view_matrices.is_some());

            // See SkImageShader::MakeForDrawRect, as this behavior is consistent but avoids
            // allocating SkShader objects or having to modify the SkPaint.
            // Adjust `dst` such that it only samples from the portion of fSrcRect that overlaps
            // with the image bounds. This "decal" effect is applied geometrically to what is
            // drawn so that actual texture tiling can be clamped to the src rect.
            let image_bounds = SkRect::from_irect(entry.image.bounds());
            let mut dst_to_draw = entry.dst_rect;
            let mut subset = entry.src_rect;
            let local_matrix = Matrix::rect_to_rect_or_identity(subset, dst_to_draw, None);
            if !image_bounds.contains(&subset) {
                if subset.intersect(image_bounds) {
                    // Update dst to match the smaller src
                    dst_to_draw = local_matrix.map_rect(subset).0;
                } else {
                    dst_to_draw.set_empty();
                }
            }
            if !dst_to_draw.is_empty() {
                let image_shader = SimpleImage {
                    image: entry.image.clone(),
                    local_matrix: Some(local_matrix),
                    subset: if constraint == SrcRectConstraint::Strict {
                        subset
                    } else {
                        image_bounds
                    },
                    sampling_options: *sampling,
                };

                // NOTE: See drawEdgeAAQuad for details, we do not snap non-AA quads.
                let quad = if entry.has_clip {
                    let clips = dst_clips.expect("a clipped entry has dst clips");
                    let points: [Point; 4] = [
                        clips[dst_clip_index],
                        clips[dst_clip_index + 1],
                        clips[dst_clip_index + 2],
                        clips[dst_clip_index + 3],
                    ];
                    EdgeAAQuad::from_points(&points, entry.aa_flags)
                } else {
                    EdgeAAQuad::from_sk_rect(&dst_to_draw, entry.aa_flags)
                };

                // TODO: Calling drawGeometry() for each entry re-evaluates the clip stack every
                // time, which is consistent with Ganesh's behavior. It also matches the behavior
                // if edge-AA images were submitted one at a time by SkiaRenderer (a nice client
                // simplification). However, we should explore the performance trade off with
                // doing one bulk evaluation for the whole set
                let xtra_xform = if entry.matrix_index < 0 {
                    None
                } else {
                    pre_view_matrices.map(|m| &m[usize::try_from(entry.matrix_index).unwrap_or(0)])
                };
                let entry_transform = match xtra_xform {
                    Some(m) => local_to_device.concat_m44(&M44::from(m)),
                    None => local_to_device,
                };
                self.draw_geometry(
                    &entry_transform,
                    Geometry::EdgeAAQuad(quad),
                    &PaintParams::from_paint_with_image(paint, image_shader, entry.alpha),
                    &default_fill_style(),
                );
            }
            dst_clip_index += 4 * usize::from(entry.has_clip);
        }
    }

    // Port of: src/gpu/graphite/Device.cpp#L1496-L1512 (chrome/m156)
    fn draw_image_rect(
        &mut self,
        image: &Image,
        src: Option<&SkRect>,
        dst: &SkRect,
        sampling: &SamplingOptions,
        paint: &Paint,
        constraint: SrcRectConstraint,
    ) {
        let mut single = ImageSetEntry {
            image: image.clone(),
            src_rect: src
                .copied()
                .unwrap_or_else(|| SkRect::from_irect(image.bounds())),
            dst_rect: *dst,
            matrix_index: -1,
            alpha: 1.0,
            aa_flags: EdgeFlags::ALL,
            has_clip: false,
        };
        // While this delegates to drawEdgeAAImageSet() for the image shading logic, semantically
        // a drawImageRect()'s non-AA behavior should match that of drawRect() so we snap dst (and
        // update src to match) if needed before hand.
        if !paint.is_anti_alias() {
            let transform = self.local_to_device_transform();
            snap_src_and_dst_rect_to_pixels(&transform, &mut single.src_rect, &mut single.dst_rect);
        }
        self.draw_edge_aa_image_set(
            std::slice::from_ref(&single),
            None,
            None,
            sampling,
            paint,
            constraint,
        );
    }

    // Port of: src/gpu/graphite/Device.cpp#L1610-L1651 (chrome/m156)
    fn draw_geometry_with_path_effect(
        &mut self,
        local_to_device: &Transform,
        mut geometry: Geometry,
        paint: &PaintParams,
        mut style: StrokeRec,
        path_effect: Option<&skia_rust_core::path_effect::PathEffect>,
    ) {
        // Path effects are applied on the CPU, which may modify the geometry to draw.
        // TODO(b/238757903): Handle dashing on the GPU when possible (e.g. straight lines)
        if let Some(path_effect) = path_effect
            && local_to_device.valid()
        {
            // Apply the path effect before anything else, which if we are applying here, means
            // that we are dealing with a Shape. drawVertices (and a SkVertices geometry) should
            // pass in kIgnorePathEffect per SkCanvas spec. Text geometry also should pass in
            // kIgnorePathEffect because the path effect is applied per glyph by the SkStrikeSpec
            // already.
            debug_assert!(geometry.is_shape());

            // TODO: If asADash() returns true and the base path matches the dashing fast path,
            // then that should be detected now as well. Maybe add dashPath to Device so canvas
            // can handle it
            let mut max_scale_factor = local_to_device.max_scale_factor();
            if local_to_device.type_() == TransformType::Perspective {
                let bounds = geometry.bounds();
                let tl = local_to_device
                    .scale_factors(Float2::new(bounds.left(), bounds.top()))
                    .1;
                let tr = local_to_device
                    .scale_factors(Float2::new(bounds.right(), bounds.top()))
                    .1;
                let br = local_to_device
                    .scale_factors(Float2::new(bounds.right(), bounds.bot()))
                    .1;
                let bl = local_to_device
                    .scale_factors(Float2::new(bounds.left(), bounds.bot()))
                    .1;
                max_scale_factor = (tl.max(tr)).max(bl.max(br));
            }

            style.set_res_scale(max_scale_factor);
            let mut builder = skia_rust_core::path_builder::PathBuilder::new();
            if path_effect.filter_path_inplace_with_matrix(
                &mut builder,
                &geometry.shape().as_path(),
                &mut style,
                None::<&SkRect>,
                &local_to_device.to_matrix(),
            ) {
                let mut dst = builder.detach();
                dst.set_is_volatile(true);
                geometry = Geometry::Shape(Shape::from_path(dst));
            } else {
                skia_log_w!("Path effect failed to apply, drawing original path.");
            }

            // Fallthrough, remaining code assumes the effect has been applied to `geometry` and
            // `style`
        }

        self.draw_geometry(local_to_device, geometry, paint, &style);
    }

    // The renderer for a geometry and style: the code of `chooseRenderer()` that does not
    // involve a path atlas or a `PathRendererStrategy` other than tessellation (G10c, G12a).
    // Port of: src/gpu/graphite/Device.cpp#L2142-L2302 (chrome/m156)
    fn choose_renderer<'r>(
        recorder: &'r RecorderPriv<'_>,
        local_to_device: &Transform,
        geometry: &Geometry,
        style: &StrokeRec,
        draw_bounds: &Rect,
    ) -> Option<&'r Renderer> {
        let renderers = recorder.renderer_provider();
        let ty = style.style();

        match geometry {
            Geometry::Vertices(vertices) => {
                return Some(renderers.vertices(vertices.has_colors(), vertices.has_tex_coords()));
            }
            Geometry::Mesh(_) => return Some(renderers.mesh()),
            Geometry::EdgeAAQuad(quad) => {
                debug_assert!(style.is_fill_style());
                // handled by specialized system, simplified from rects and round rects
                return if quad.is_rect()
                    && (quad.edge_flags() == EdgeFlags::NONE
                        || is_pixel_aligned(&quad.bounds(), local_to_device))
                {
                    // For non-AA rectangular quads, it can always use a coverage-less renderer;
                    // there's no need to check for pixel alignment to avoid popping if MSAA is
                    // turned on because quad tile edges will seam with each in either mode. We
                    // also switch to use the cover bounds when the quad is pixel aligned to be
                    // consistent with drawRect Renderer handling.
                    Some(renderers.non_aa_bounds_fill())
                } else {
                    Some(renderers.per_edge_aa_quad())
                };
            }
            Geometry::Shape(_) => {}
            // We must account for new Geometry types with specific Renderers
            Geometry::Empty => return None,
        }

        let Geometry::Shape(shape) = geometry else {
            return None;
        };
        if is_simple_shape(shape, local_to_device, ty) {
            debug_assert_ne!(ty, StrokeStyleKind::StrokeAndFill); // stroke+fill is *not* simple
            // For pixel-aligned rects, use the the non-AA bounds renderer to avoid triggering any
            // dst-read requirement due to src blending.
            let mut pixel_aligned_rect = false;
            if shape.is_rect() && style.is_fill_style() {
                pixel_aligned_rect = is_pixel_aligned(shape.rect(), local_to_device);
            }

            return if shape.is_empty() || pixel_aligned_rect {
                debug_assert!(!shape.is_empty() || shape.inverted());
                Some(renderers.non_aa_bounds_fill())
            } else {
                Some(renderers.analytic_rrect())
            };
        }

        if shape.is_arc()
            && shape.arc().sweep_angle.abs() < 360.0
            && local_to_device.type_() <= TransformType::Affine
            && rrect_priv::is_relatively_circular(
                shape.arc().oval.width(),
                shape.arc().oval.height(),
                Shape::DEFAULT_PIXEL_TOLERANCE * local_to_device.local_aa_radius(draw_bounds),
            )
        {
            // We aren't perspective, so the point passed to scaleFactors() doesn't matter
            let (min_scale, max_scale) = local_to_device.scale_factors(Float2::new(0.0, 0.0));
            if <f32 as skia_rust_core::scalar::Scalar>::nearly_equal(
                max_scale,
                min_scale,
                skia_rust_core::scalar::SCALAR_NEARLY_ZERO,
            ) {
                // Arc support depends on the style.
                match ty {
                    StrokeStyleKind::StrokeAndFill => {
                        // This produces a strange result that this op doesn't implement.
                    }
                    StrokeStyleKind::Fill => return Some(renderers.circular_arc()),
                    StrokeStyleKind::Stroke | StrokeStyleKind::Hairline => {
                        // Strokes that don't use the center point are supported with butt & round
                        // caps.
                        let is_wedge = shape.arc().is_wedge();
                        let is_square_cap = style.cap() == Cap::Square;
                        if !is_wedge && !is_square_cap {
                            return Some(renderers.circular_arc());
                        }
                    }
                }
            }
        }

        // (The PathRendererStrategy dispatch to the compute, small and raster path atlases, and
        // to sparse strips, comes with G12a and G17; the strategy is tessellation.)

        // If we got here, it requires tessellated path rendering or an MSAA technique applied to
        // a simple shape (so we interpret them as paths to reduce the number of pipelines we
        // need).
        Some(Self::choose_msaa_renderer(
            recorder,
            shape,
            style,
            draw_bounds,
        ))
    }

    // Ignoring specialized Shape renderers and the selected PathRendererStrategy, choose a
    // MSAA-requiring tessellation-based renderer for the shape and style.
    // Port of: src/gpu/graphite/Device.cpp#L2304-L2341 (chrome/m156)
    fn choose_msaa_renderer<'r>(
        recorder: &'r RecorderPriv<'_>,
        shape: &Shape,
        style: &StrokeRec,
        draw_bounds: &Rect,
    ) -> &'r Renderer {
        // TODO: All shapes that select a tessellating path renderer need to be "pre-chopped" if
        // they are large enough to exceed the fixed count tessellation limits. Fills are
        // pre-chopped to the viewport bounds, strokes and stroke-and-fills are pre-chopped to the
        // viewport bounds outset by the stroke radius (hence taking the whole style and not just
        // its type).
        let renderers = recorder.renderer_provider();
        let ty = style.style();

        if ty == StrokeStyleKind::Stroke || ty == StrokeStyleKind::Hairline {
            // Unlike in Ganesh, the HW stroke tessellator can work with arbitrary paints since
            // the depth test prevents double-blending when there is transparency, thus we can HW
            // stroke any path regardless of its paint.
            return renderers.tessellated_strokes(shape.inverted());
        }

        // 'type' could be kStrokeAndFill, but in that case chooseRenderer() is meant to return
        // the fill renderer since tessellatedStrokes() will always be used for the stroke pass.
        if shape.convex(false) && !shape.inverted() {
            // TODO: Ganesh doesn't have a curve+middle-out triangles option for convex paths, but
            // it would be pretty trivial to spin up.
            renderers.convex_tessellated_wedges()
        } else {
            let prefer_wedges =
                // TODO: Combine this heuristic with what is used in PathStencilCoverOp to choose
                // between wedges curves consistently in Graphite and Ganesh.
                (shape.is_path() && shape.path().count_verbs() < 50)
                    || draw_bounds.area() <= (256.0 * 256.0);

            if prefer_wedges {
                renderers.stencil_tessellated_wedges(shape.fill_type())
            } else {
                renderers.stencil_tessellated_curves_and_tris(shape.fill_type())
            }
        }
    }

    /// `drawGeometry(localToDevice, geometry, paint, style)`: records a draw with the given style
    /// and paint effects, applying any analytic clipping or depth-based clipping automatically
    /// based on the current clip stack state.
    ///
    /// All overridden `SkDevice::draw()` functions should bottom-out with calls to `drawGeometry()`.
    // Port of: src/gpu/graphite/Device.cpp#L1653-L2052 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    pub fn draw_geometry(
        &mut self,
        local_to_device: &Transform,
        mut geometry: Geometry,
        paint: &PaintParams,
        style: &StrokeRec,
    ) {
        let Some(recorder) = self.recorder() else {
            return;
        };
        let rp = recorder.priv_();

        if !local_to_device.valid() {
            // If the transform is not invertible or not finite then drawing isn't well defined.
            skia_log_w!("Skipping draw with non-invertible/non-finite transform.");
            return;
        }

        // TODO: The tessellating and atlas path renderers haven't implemented perspective yet,
        // so transform to device space so we draw something approximately correct (barring local
        // coord issues).
        if geometry.is_shape()
            && local_to_device.type_() == TransformType::Perspective
            && !is_simple_shape(geometry.shape(), local_to_device, style.style())
        {
            let mut device_path = geometry
                .shape()
                .as_path()
                .make_transform(&local_to_device.matrix().to_m33());
            device_path.set_is_volatile(true);
            // TODO(b/452415460): This fallback breaks perspective interpolation for local coords
            // and it causes strokes to render in device space.
            self.draw_geometry(
                &Transform::identity(),
                Geometry::Shape(Shape::from_path(device_path)),
                paint,
                style,
            );
            return;
        }

        // Ignores the inverted-ness of a shape with a hairline style, which follows the same
        // behavior as Ganesh and CPU rasterization.
        if let Geometry::Shape(shape) = &mut geometry
            && shape.inverted()
            && style.is_hairline_style()
        {
            shape.set_inverted(false);
        }

        let key_db = rp.pop_or_create_key_and_data_builder();
        debug_assert!(self.draw_builders_are_reset(&key_db));
        self.draw_geometry_with_builders(
            &recorder,
            local_to_device,
            geometry,
            paint,
            style,
            &key_db,
        );
        // The PipelineDataGatherer and builder must be reset before being returned to the pool.
        key_db.gatherer.borrow_mut().reset_for_draw();
        key_db.builder.borrow_mut().reset_for_draw();
        rp.push_key_and_data_builder(key_db);
    }

    // The gatherer and builder must be reset before being returned to the pool for reuse, so
    // they should be empty when we fetch them.
    #[allow(clippy::unused_self)] // mirrors the C++ member function
    fn draw_builders_are_reset(
        &self,
        key_db: &crate::graphite::recorder::KeyAndDataBuilder,
    ) -> bool {
        #[cfg(debug_assertions)]
        {
            key_db.gatherer.borrow().check_reset();
            key_db.builder.borrow().check_reset();
        }
        #[cfg(not(debug_assertions))]
        let _ = key_db;
        true
    }

    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    fn draw_geometry_with_builders(
        &mut self,
        recorder: &Recorder,
        local_to_device: &Transform,
        mut geometry: Geometry,
        paint: &PaintParams,
        style: &StrokeRec,
        key_db: &crate::graphite::recorder::KeyAndDataBuilder,
    ) {
        let rp = recorder.priv_();

        // Calculate the clipped bounds of the draw and determine the clip elements that affect
        // the draw without updating the clip stack.
        let mut clip_elements = ElementList::new();
        let mut clip = self.clip().visit_clip_stack_for_draw(
            local_to_device,
            &mut geometry,
            style,
            &mut clip_elements,
            // (The clip atlas is G12a: without it the remaining elements are depth-only draws.)
            None,
        );
        if clip.is_clipped_out() {
            // Clipped out, so don't record anything.
            return;
        }

        // We assume that we will receive a renderer, or a PathAtlas. If it's a PathAtlas, then we
        // assume that the renderer chosen in PathAtlas::addShape() will have single-channel
        // coverage, require AA bounds outsetting, and have a single renderStep. The clip's draw
        // bounds are passed in for heuristics, so it's fine if it doesn't include the AA
        // outsetting we add for some analytic coverage renderers.
        let Some(renderer) = Self::choose_renderer(
            &rp,
            local_to_device,
            &geometry,
            style,
            &clip.transformed_shape_bounds(),
        ) else {
            skia_log_w!("Skipping draw with no supported renderer or PathAtlas.");
            return;
        };

        // Update the pixel bounds of the draw to include any outsets done by the renderer (or
        // that must be included in the pixels required when using an atlas). This is important so
        // that all bounds overlap checks take into account pixels touched by rasterization, even
        // if the calculated coverage for a pixel is 0.
        //
        // TODO (thomsmit): Add handling specifically for EndCaps to align the clip to tile size.
        if renderer.outset_bounds_for_aa() {
            clip.outset_bounds_for_aa();
        }

        let format = self
            .dc
            .target()
            .proxy()
            .expect("a device has a target")
            .format();
        let clip_shader = self.clip().clip_shader_ref().cloned();
        let shading = ShadingParams::new(
            &**rp.caps(),
            paint,
            Some(clip.non_msaa_clip()),
            clip_shader.as_ref(),
            renderer.coverage(),
            format,
        );

        // Some shapes and styles combine multiple draws so the total render step count is split
        // between the main renderer and possibly a secondaryRenderer. As we can't be sure whether
        // a secondary renderer is required prior to getting the dstUsage from shading.toKey(), we
        // pessimistically assume it's required for needsFlushBeforeDraw().
        let mut num_new_render_steps = renderer.num_render_steps();
        let mut style_type = style.style();
        if style_type == StrokeStyleKind::StrokeAndFill {
            debug_assert!(geometry.is_shape());
            num_new_render_steps += rp
                .renderer_provider()
                .tessellated_strokes(/*inverse_fill=*/ false)
                .num_render_steps();
        } else if style_type == StrokeStyleKind::Fill && renderer.use_non_aa_inner_fill() {
            num_new_render_steps += rp
                .renderer_provider()
                .non_aa_bounds_fill()
                .num_render_steps();
        }

        // Decide if we have any reason to flush pending work. A flush may be necessary for two
        // reasons:
        //      1) A flush is required before updating the clip state or making any permanent
        //         changes to a path atlas, since otherwise clip operations and/or atlas entries
        //         for the current draw will be flushed.
        //      2) A flush is required before shading.toKey() is called so that child tasks
        //         required by this draw are associated with the DrawContext after any instead of
        //         being added as a child of the current draw. See "Layer" tests in
        //         NotifyInUseTest.cpp.
        let dst_read_strategy = if shading.dst_read_required() {
            self.dc.dst_read_strategy()
        } else {
            DstReadStrategy::NoneRequired
        };
        // TODO (thomsmit): Adjust this when the draw limit is removed.
        let needs_flush = self.needs_flush_before_draw(num_new_render_steps, dst_read_strategy);
        if needs_flush {
            // (Flushing the tracked devices instead, when a path atlas was chosen, is G12a.)
            self.flush_pending_work(None);
        }

        // Determine the paint ID and collect the paint uniforms now before anything has been
        // recorded. The paint may reference an SkPicture or a Graphite-backed dynamic SkImage
        // that can trigger a flush of the Recorder.
        let mut key_gen_flags = KeyGenFlags::DEFAULT;
        if renderer.use_non_aa_inner_fill() || renderer.coverage() == Coverage::None {
            key_gen_flags |= KeyGenFlags::PREFER_FIXED_SRC_BLEND;
        }
        let key_context = KeyContext::new_with_draw_context(
            recorder,
            &self.dc,
            &key_db.builder,
            &key_db.gatherer,
            local_to_device.matrix(),
            &clip.draw_bounds().as_sk_rect(),
            self.dc.color_info(),
            key_gen_flags,
            paint.color(),
        );
        let Some((paint_id, dst_usage)) = shading.to_key(&key_context) else {
            // Converting the SkPaint to a pipeline and set of uniform values + sampled textures
            // failed.
            skia_log_w!("Key context creation failed in Device::drawGeometry, draw dropped!");
            return;
        };

        // If we are unclipped, do not depend on the dst, and cover the target, then we can adjust
        // load ops of the renderpass to more optimally handle the draw (and avoid redundant
        // clears).
        // NOTE: We skip this for fully-lazy render targets because the load ops may impact a
        // larger area than the Device's theoretical bounds.
        let overwrites_all_pixels = dst_usage == DstUsage::NONE
            && geometry.is_shape()
            && geometry.shape().is_flood_fill()
            && !self
                .dc
                .target()
                .proxy()
                .is_some_and(|proxy| proxy.is_fully_lazy())
            && clip_elements.is_empty()
            && IRect::from_wh(self.width, self.height).contains(&clip.scissor())
            && clip
                .scissor()
                .contains(&IRect::from_wh(self.width, self.height));
        if overwrites_all_pixels {
            if let Some(color) = extract_paint_color(paint, self.dc.color_info()) {
                // Fullscreen clear, so nothing has to be rendered at all
                self.dc.clear(color);
                return;
            }
            // This paint does not depend on the destination and covers the entire surface, so
            // discard everything previously recorded and proceed with the draw. DstUsage::kNone
            // implies no blending will be enabled for the draw, so discards on floating-point
            // formats (that could inject NaNs into the dst) will be overwritten correctly.
            self.dc.discard();
            // But then continue to render the flood fill with shading
        }

        // (If an atlas path renderer was chosen the shape is inserted into the atlas here (G12a).)

        // Renderers and their component RenderSteps have flexibility in defining their
        // DepthStencilSettings. However, the clipping and ordering managed between Device and
        // ClipStack requires that only LESS or LEQUAL depth tests are used for draws recorded
        // through the client-facing, painters-order-oriented API. We assert here vs. in
        // Renderer's constructor to allow internal-oriented Renderers that are never selected
        // for a "regular" draw call to have more flexibility in their settings.
        #[cfg(debug_assertions)]
        for step in renderer.steps() {
            let dss = step.base().depth_stencil_settings();
            debug_assert!(
                (!step.performs_shading() || dss.depth_test_enabled)
                    && (!dss.depth_test_enabled
                        || dss.depth_compare_op == crate::graphite::draw_types::CompareOp::Less
                        || dss.depth_compare_op == crate::graphite::draw_types::CompareOp::LEqual)
            );
        }

        // Update the clip stack after issuing a flush (if it was needed). A draw will be recorded
        // after this point.
        let mut order = DrawOrder::new(self.current_depth.next());
        let current_depth = order.depth();
        let (clip_order, clip_layer) = {
            // The hooks (the core) don't touch the bounds manager while the clip stack updates.
            let bounds_manager = Rc::clone(&self.color_depth_bounds_manager);
            let bounds_manager = bounds_manager.borrow();
            self.with_clip(|clip_stack, hooks| {
                clip_stack.update_clip_state_for_draw(
                    hooks,
                    &clip,
                    &clip_elements,
                    &*bounds_manager,
                    current_depth,
                )
            })
        };

        // A draw's order always depends on the clips that must be drawn before it
        order.depends_on_painters_order(clip_order);
        let use_draw_list_layer = rp.caps().use_draw_list_layer();
        let avoid_depth_mode = rp.caps().avoid_depth_mode();
        if !use_draw_list_layer {
            // If a draw is not opaque, it must be drawn after the most recent draw it intersects
            // with in order to blend correctly. If there is no depth buffer, then always use
            // painters order.
            if dst_usage.contains(DstUsage::DEPENDS_ON_DST) || avoid_depth_mode {
                let prev_draw = self
                    .color_depth_bounds_manager
                    .borrow()
                    .get_most_recent_draw(clip.draw_bounds());
                order.depends_on_painters_order(prev_draw);
            }

            // Now that the base paint order and draw bounds are finalized, if the Renderer relies
            // on the stencil attachment, we compute a secondary sorting field to allow disjoint
            // draws to reorder the RenderSteps across draws instead of in sequence for each draw.
            if matches!(
                renderer.depth_stencil_flags(),
                DepthStencilFlags::Stencil | DepthStencilFlags::DepthStencil
            ) {
                debug_assert!(!avoid_depth_mode);
                let set_index = self
                    .disjoint_stencil_set
                    .add(order.paint_order(), clip.draw_bounds());
                order.depends_on_stencil(set_index);
            } else if !dst_usage.contains(DstUsage::DEPENDS_ON_DST)
                && style_type == StrokeStyleKind::Fill
                && ((geometry.is_edge_aa_quad() && geometry.edge_aa_quad().is_rect())
                    || (geometry.is_shape() && geometry.shape().is_rect()))
                && !avoid_depth_mode
            {
                // Sort this draw front to back since it will not blend against what came before
                // it. We could do this for all opaque/non-blending draws but that can hurt the
                // performance of the std::sort in DrawPass::Make if it has to effectively reverse
                // a large list. For now, limit it to filled rectangles (here and for the later
                // non-AA inner fill).
                order.reverse_depth_as_stencil();
            }
        }

        // The inner fill's opaque paint is found before the gatherer is borrowed below: the debug
        // validation in `optimize_for_opacity` borrows the gatherer of the key context again.
        let inner_fill_opaque_id = if style_type == StrokeStyleKind::Fill
            && dst_usage.contains(DstUsage::DST_ONLY_USED_BY_RENDERER)
            && renderer.use_non_aa_inner_fill()
            && !avoid_depth_mode
            && !get_inner_bounds(&geometry, local_to_device).is_empty_negative_or_nan()
        {
            Some(shading.optimize_for_opacity(&key_context, paint_id))
        } else {
            None
        };

        let gatherer = &mut *key_db.gatherer.borrow_mut();
        if style_type != StrokeStyleKind::Fill {
            debug_assert!(geometry.is_shape());
            // For inverse stroke-and-fill style, we perform a depth only draw of the stroke so
            // when we perform our draw for the fill, we don't write over the stroked part. This
            // ensures we keep the inverse fill outside of the entire shape including the stroke.
            let is_stroke_and_fill = style_type == StrokeStyleKind::StrokeAndFill;
            let depth_only_stroke = is_stroke_and_fill && geometry.shape().inverted();
            let stroke_renderer = if is_stroke_and_fill {
                rp.renderer_provider()
                    .tessellated_strokes(/*inverse_fill=*/ false)
            } else {
                renderer
            };
            let stroke_paint_id = if depth_only_stroke {
                UniquePaintParamsID::invalid()
            } else {
                paint_id
            };
            let stroke_order = order;
            if depth_only_stroke {
                order.depends_on_painters_order(stroke_order.paint_order());
            }

            let stroke = StrokeStyle::new(style.width(), style.miter(), style.join(), style.cap());
            self.dc.record_draw(
                stroke_renderer,
                local_to_device,
                &geometry,
                &clip,
                stroke_order,
                stroke_paint_id,
                dst_usage,
                gatherer,
                Some(&stroke),
                clip_layer,
            );
        } else if dst_usage.contains(DstUsage::DST_ONLY_USED_BY_RENDERER)
            && renderer.use_non_aa_inner_fill()
            && !avoid_depth_mode
        {
            // Possibly record an additional draw using the non-AA bounds renderer to fill the
            // interior with a renderer that can disable blending entirely.
            let inner_fill_bounds = get_inner_bounds(&geometry, local_to_device);
            if !inner_fill_bounds.is_empty_negative_or_nan() {
                let mut order_without_coverage = DrawOrder::new(order.depth());
                order_without_coverage.depends_on_painters_order(clip_order);
                // The regular draw has analytic coverage, so isn't being sorted front to back,
                // but we do want to sort the inner fill to maximize overdraw reduction
                order_without_coverage.reverse_depth_as_stencil();

                let opaque_id = inner_fill_opaque_id
                    .expect("the inner fill's opaque paint is found for the same draw");
                self.dc.record_draw(
                    rp.renderer_provider().non_aa_bounds_fill(),
                    local_to_device,
                    &Geometry::Shape(Shape::from_rect(inner_fill_bounds)),
                    &clip,
                    order_without_coverage,
                    opaque_id,
                    DstUsage::NONE,
                    gatherer,
                    None,
                    clip_layer,
                );
                // Force the coverage draw to come after the non-AA draw in order to benefit from
                // early depth testing.
                order.depends_on_painters_order(order_without_coverage.paint_order());
            }
        }

        if style_type == StrokeStyleKind::Fill || style_type == StrokeStyleKind::StrokeAndFill {
            self.dc.record_draw(
                renderer,
                local_to_device,
                &geometry,
                &clip,
                order,
                paint_id,
                dst_usage,
                gatherer,
                None,
                clip_layer,
            );
        }
        style_type = StrokeStyleKind::Fill;
        let _ = style_type;

        if !use_draw_list_layer {
            self.color_depth_bounds_manager
                .borrow_mut()
                .record_draw(clip.draw_bounds(), order.paint_order());
        }
        self.current_depth = order.depth();

        // TODO(b/238758897): When we enable layer elision that depends on draws not overlapping,
        // we can use the `getMostRecentDraw()` query to determine that, although that will mean
        // querying even if the draw does not depend on dst (so should be only be used when the
        // Device is an elision candidate).
    }

    /// `drawClipShape(...)`: like `drawGeometry()` but is Shape-only, depth-only, fill-only, and
    /// lets the `ClipStack` define the transform, clip, and `DrawOrder` (although `Device` still
    /// tracks stencil buffer usage).
    // Port of: src/gpu/graphite/Device.cpp#L2054-L2105 (chrome/m156)
    fn draw_clip_shape_impl(
        &mut self,
        local_to_device: &Transform,
        shape: &Shape,
        clip: &Clip,
        mut order: DrawOrder,
    ) {
        let Some(recorder) = self.recorder() else {
            return;
        };
        let rp = recorder.priv_();
        let key_db = rp.pop_or_create_key_and_data_builder();

        // A clip draw's state is almost fully defined by the ClipStack. The only thing we need to
        // account for is selecting a Renderer and tracking the stencil buffer usage.
        //
        // While kRasterAtlas attempts to route clip elements to an atlas, this can fail, in which
        // case the element may still be rendered into the depth buffer with tessellation (likely
        // w/o AA).
        let renderer = Self::choose_msaa_renderer(
            &rp,
            shape,
            &default_fill_style(),
            &clip.transformed_shape_bounds(),
        );
        if !rp.caps().use_draw_list_layer()
            && matches!(
                renderer.depth_stencil_flags(),
                DepthStencilFlags::Stencil | DepthStencilFlags::DepthStencil
            )
        {
            debug_assert!(!rp.caps().avoid_depth_mode());
            let set_index = self
                .disjoint_stencil_set
                .add(order.paint_order(), clip.draw_bounds());
            order.depends_on_stencil(set_index);
        }

        // This call represents one of the deferred clip shapes that's already pessimistically
        // counted in needsFlushBeforeDraw(), so the DrawContext should have room to add it.
        debug_assert!(
            self.dc.pending_render_steps() + renderer.num_render_steps() < MAX_RENDER_STEPS
        );

        // Anti-aliased clipping requires the renderer to use MSAA to modify the depth per sample,
        // so analytic coverage renderers cannot be used.
        debug_assert!(renderer.coverage() == Coverage::None && renderer.requires_msaa());

        // Clips draws are depth-only (invalid UniquePaintParamsID), and filled (null
        // StrokeStyle). The data gatherer must be reset so that the DrawList can use it for any
        // RenderStep data.
        if local_to_device.type_() == TransformType::Perspective {
            let device_path = shape
                .as_path()
                .make_transform(&local_to_device.matrix().to_m33());
            self.dc.record_draw(
                renderer,
                &Transform::identity(),
                &Geometry::Shape(Shape::from_path(device_path)),
                clip,
                order,
                UniquePaintParamsID::invalid(),
                DstUsage::NONE,
                &mut key_db.gatherer.borrow_mut(),
                None,
                None,
            );
        } else {
            self.dc.record_draw(
                renderer,
                local_to_device,
                &Geometry::Shape(shape.clone()),
                clip,
                order,
                UniquePaintParamsID::invalid(),
                DstUsage::NONE,
                &mut key_db.gatherer.borrow_mut(),
                None,
                None,
            );
        }
        // This ensures that draws recorded after this clip shape has been popped off the stack
        // will be unaffected by the Z value the clip shape wrote to the depth attachment.
        if order.depth() > self.current_depth {
            self.current_depth = order.depth();
        }

        key_db.gatherer.borrow_mut().reset_for_draw();
        key_db.builder.borrow_mut().reset_for_draw();
        rp.push_key_and_data_builder(key_db);
    }

    /// `drawClipShapeImmediate(...)`: records a draw and returns a backpointer to the
    /// `DrawParams` of the draw.
    // Port of: src/gpu/graphite/Device.cpp#L2107-L2130 (chrome/m156)
    fn draw_clip_shape_immediate_impl(
        &mut self,
        local_to_device: &Transform,
        shape: &Shape,
        clip: &Clip,
        order: DrawOrder,
    ) -> (Option<DrawParamsId>, Option<LayerId>) {
        let Some(recorder) = self.recorder() else {
            return (None, None);
        };
        let rp = recorder.priv_();
        let key_db = rp.pop_or_create_key_and_data_builder();
        let renderer = Self::choose_msaa_renderer(
            &rp,
            shape,
            &default_fill_style(),
            &clip.transformed_shape_bounds(),
        );

        let result = if local_to_device.type_() == TransformType::Perspective {
            let device_path = shape
                .as_path()
                .make_transform(&local_to_device.matrix().to_m33());
            self.dc.record_draw(
                renderer,
                &Transform::identity(),
                &Geometry::Shape(Shape::from_path(device_path)),
                clip,
                order,
                UniquePaintParamsID::invalid(),
                DstUsage::NONE,
                &mut key_db.gatherer.borrow_mut(),
                None,
                None,
            )
        } else {
            self.dc.record_draw(
                renderer,
                local_to_device,
                &Geometry::Shape(shape.clone()),
                clip,
                order,
                UniquePaintParamsID::invalid(),
                DstUsage::NONE,
                &mut key_db.gatherer.borrow_mut(),
                None,
                None,
            )
        };
        key_db.gatherer.borrow_mut().reset_for_draw();
        key_db.builder.borrow_mut().reset_for_draw();
        rp.push_key_and_data_builder(key_db);
        result
    }

    /// `updateNextDepthForClipping(depth)`.
    // Port of: src/gpu/graphite/Device.cpp#L2132-L2138 (chrome/m156)
    fn update_next_depth_for_clipping_impl(&mut self, depth: PaintersDepth) {
        // This ensures that draws recorded after this clip shape has been popped off the stack
        // will be unaffected by the Z value the clip shape wrote to the depth attachment.
        if depth > self.current_depth {
            self.current_depth = depth;
        }
    }

    // Port of: src/gpu/graphite/Device.cpp#L751-L801 (chrome/m156)
    fn on_write_pixels(&mut self, src: &Pixmap<'_>, x: i32, y: i32) -> bool {
        let Some(recorder) = self.recorder() else {
            return false;
        };
        let rp = recorder.priv_();
        // TODO: we may need to share this in a more central place to handle uploads
        // to backend textures

        let target = self.dc.target().ref_proxy().expect("a device has a target");
        if !rp.caps().is_copyable_dst(target.texture_info()) {
            // The target cannot be copied into: draw a texture made from `src` instead.
            let Some(raster) = skia_rust_core::images::raster_from_pixmap_copy(src) else {
                return false;
            };
            let Some(image) =
                texture_from_image(&recorder, &raster, RequiredProperties { mipmapped: false })
            else {
                return false;
            };
            let mut paint = Paint::default();
            paint.set_blend_mode(BlendMode::Src);
            // The destination is the pixel rect at `(x, y)`, as `SkRect::MakeXYWH` takes it.
            #[allow(clippy::cast_precision_loss)] // pixel coordinates are far below 2^24
            let dst_rect =
                SkRect::from_xywh(x as f32, y as f32, src.width() as f32, src.height() as f32);
            self.draw_image_rect(
                &image,
                None,
                &dst_rect,
                &SamplingOptions::from(FilterMode::Nearest),
                &paint,
                SrcRectConstraint::Fast,
            );
            return true;
        }

        debug_assert_eq!(self.dc.target().origin(), Origin::TopLeft);

        // Determine rect to copy
        let mut dst_rect = IRect::from_xywh(x, y, src.width(), src.height());
        if !target.is_fully_lazy() {
            match IRect::intersect(&dst_rect, &IRect::from_size(target.dimensions())) {
                Some(rect) => dst_rect = rect,
                None => return false,
            }
        }

        // Adjust the copy location for any change after intersection
        let level = MipLevel {
            pixels: src.addr_at((dst_rect.left - x, dst_rect.top - y)),
            row_bytes: src.row_bytes(),
        };

        // The writePixels() still respects painter's order, so flush everything to tasks before
        // this recording the upload for the pixel data.
        self.internal_flush(&recorder);
        // The new upload will be executed before any new draws are recorded and also ensures that
        // the next call to flushDeviceToRecorder() will produce a non-null DrawTask. If this
        // Device's target is mipmapped, mipmap generation tasks will be added automatically at
        // that point.
        let upload_source = UploadSource::make(
            &**rp.caps(),
            self.dc.target(),
            src.info().color_info(),
            self.dc.image_info().color_info(),
            std::slice::from_ref(&level),
            dst_rect,
        );
        self.dc.record_upload(&rp, &upload_source, None)
    }
}

// The address of the `RecorderInner` a `Recorder` handle refers to.
fn recorder_inner_ptr(recorder: &Recorder) -> *const RecorderInner {
    recorder.downgrade().as_ptr()
}

impl ClipDrawHooks for DeviceCore {
    fn draw_clip_shape(
        &mut self,
        local_to_device: &Transform,
        shape: &Shape,
        clip: &Clip,
        order: DrawOrder,
    ) {
        self.draw_clip_shape_impl(local_to_device, shape, clip, order);
    }

    fn draw_clip_shape_immediate(
        &mut self,
        local_to_device: &Transform,
        shape: &Shape,
        clip: &Clip,
        order: DrawOrder,
    ) -> (Option<DrawParamsId>, Option<LayerId>) {
        self.draw_clip_shape_immediate_impl(local_to_device, shape, clip, order)
    }

    fn update_next_depth_for_clipping(&mut self, depth: PaintersDepth) {
        self.update_next_depth_for_clipping_impl(depth);
    }

    fn use_draw_list_layer(&self) -> bool {
        self.recorder()
            .is_some_and(|recorder| recorder.priv_().caps().use_draw_list_layer())
    }

    fn update_clip_draw(
        &mut self,
        params: DrawParamsId,
        order: DrawOrder,
        draw_bounds: Rect,
        scissor: IRect,
    ) {
        self.dc
            .update_clip_draw(params, order, draw_bounds, scissor);
    }

    fn layer_order(&self, layer: LayerId) -> Option<CompressedPaintersOrder> {
        self.dc.layer_order(layer)
    }
}

impl TrackedDevice for DeviceCore {
    // Port of: src/gpu/graphite/Device.cpp#L654-L656 (chrome/m156)
    fn has_pending_reads(&self, dependency: &Arc<TextureProxy>) -> bool {
        self.recorder.upgrade().is_some() && self.dc.reads_texture(dependency)
    }

    fn flush_pending_work(&mut self) {
        DeviceCore::flush_pending_work(self, None);
    }

    fn has_recorder(&self) -> bool {
        self.recorder.upgrade().is_some()
    }

    fn abandon_recorder(&mut self) {
        DeviceCore::abandon_recorder(self);
    }

    fn reset_storage_cache(&mut self) {
        self.dc.storage_context().borrow_mut().reset_cache();
    }
}

impl CoreDevice for Device {
    fn state(&self) -> &DeviceState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut DeviceState {
        &mut self.state
    }

    // Port of: src/gpu/graphite/Device.cpp#L818-L821 (chrome/m156)
    fn dev_clip_bounds(&self) -> IRect {
        rect_to_pixelbounds(&self.core.borrow().clip().conservative_bounds())
    }

    fn push_clip_stack(&mut self) {
        self.sync().with_clip(|clip, _hooks| clip.save());
    }

    fn pop_clip_stack(&mut self) {
        self.sync().with_clip(|clip, hooks| clip.restore(hooks));
    }

    fn clip_rect(&mut self, rect: &SkRect, op: ClipOp, aa: bool) {
        self.sync().clip_rect(rect, op, aa);
    }

    fn clip_rrect(&mut self, rrect: &RRect, op: ClipOp, aa: bool) {
        self.sync().clip_rrect(rrect, op, aa);
    }

    fn clip_path(&mut self, path: &Path, op: ClipOp, _aa: bool) {
        self.sync().clip_path(path, op);
    }

    fn clip_region(&mut self, region: &Region, op: ClipOp) {
        self.sync().clip_region(region, op);
    }

    // Port of: src/gpu/graphite/Device.cpp#L861-L864 (chrome/m156)
    fn on_clip_shader(&mut self, shader: Shader) {
        self.sync()
            .with_clip(|clip, _hooks| clip.clip_shader(shader));
    }

    // Port of: src/gpu/graphite/Device.cpp#L883-L897 (chrome/m156)
    fn replace_clip(&mut self, _rect: &IRect) {
        // ReplaceClip() is currently not intended to be supported in Graphite since it's only
        // used for emulating legacy clip ops in Android Framework, and apps/devices that require
        // that should not use Graphite. However, if it needs to be supported, we could probably
        // implement it by:
        //  1. Flush all pending clip element depth draws.
        //  2. Draw a fullscreen rect to the depth attachment using a Z value greater than what's
        //     been used so far.
        //  3. Make sure all future "unclipped" draws use this Z value instead of 0 so they aren't
        //     sorted before the depth reset.
        //  4. Make sure all prior elements are inactive so they can't affect subsequent draws.
        //
        // For now, just ignore it.
    }

    fn is_clip_anti_aliased(&self) -> bool {
        self.core.borrow().is_clip_anti_aliased()
    }

    fn is_clip_empty(&self) -> bool {
        self.core.borrow().clip().clip_state() == ClipState::Empty
    }

    fn is_clip_rect(&self) -> bool {
        let state = self.core.borrow().clip().clip_state();
        state == ClipState::DeviceRect || state == ClipState::WideOpen
    }

    fn is_clip_wide_open(&self) -> bool {
        self.core.borrow().clip().clip_state() == ClipState::WideOpen
    }

    // Port of: src/gpu/graphite/Device.cpp#L823-L839 (chrome/m156)
    fn android_utils_clip_as_rgn(&self, region: &mut Region) {
        let core = self.core.borrow();
        let bounds = rect_to_pixelbounds(&core.clip().conservative_bounds());
        // Assume wide open and then perform intersection/difference operations reducing the
        // region
        region.set_rect(bounds);
        let device_bounds = Region::from_rect(bounds);
        for e in core.clip().elements() {
            let mut tmp = Region::new();
            if e.shape.is_rect() && e.local_to_device.type_() == TransformType::Identity {
                tmp.set_rect(rect_to_pixelbounds(e.shape.rect()));
            } else {
                let tmp_path = e
                    .shape
                    .as_path()
                    .make_transform(&e.local_to_device.to_matrix());
                tmp.set_path(&tmp_path, &device_bounds);
            }

            let op = match e.op {
                ClipOp::Difference => skia_rust_core::region::Op::Difference,
                ClipOp::Intersect => skia_rust_core::region::Op::Intersect,
            };
            region.op_region(&tmp, op);
        }
    }

    // Port of: src/gpu/graphite/Device.cpp#L575-L591 (chrome/m156)
    fn set_immutable(&mut self) {
        let mut core = self.sync();
        core.set_immutable();
    }

    // Port of: src/gpu/graphite/Device.cpp#L669-L693 (chrome/m156)
    fn create_device(
        &mut self,
        info: &CreateInfo,
        _layer_paint: Option<&Paint>,
    ) -> Option<Box<dyn CoreDevice>> {
        // TODO: Inspect the paint and create info to determine if there's anything that has to be
        // modified to support inline subpasses.
        let props = self
            .state
            .surface_props()
            .clone_with_pixel_geometry(info.pixel_geometry);

        // Skia's convention is to only clear a device if it is non-opaque.
        let initial_load_op = if info.info.is_opaque() {
            LoadOp::Discard
        } else {
            LoadOp::Clear
        };

        let (recorder, label) = {
            let core = self.core.borrow();
            let recorder = core.recorder()?;
            let mut label = core
                .dc
                .target()
                .proxy()
                .expect("a device has a target")
                .label()
                .to_owned();
            if label.is_empty() {
                "ChildDevice".clone_into(&mut label);
            } else {
                label += "_ChildDevice";
            }
            (recorder, label)
        };

        Device::make_with_info(
            Some(&recorder),
            &info.info,
            Budgeted::Yes,
            Mipmapped::No,
            BackingFit::Approx,
            &props,
            initial_load_op,
            &label,
            true,
            false,
        )
        .map(|device| Box::new(device) as Box<dyn CoreDevice>)
    }

    fn draw_paint(&mut self, paint: &Paint) {
        self.sync().draw_paint(paint);
    }

    fn draw_points(&mut self, mode: PointMode, points: &[Point], paint: &Paint) {
        self.sync().draw_points(mode, points, paint);
    }

    fn draw_rect(&mut self, r: &SkRect, paint: &Paint) {
        self.sync().draw_rect(r, paint);
    }

    fn draw_oval(&mut self, oval: &SkRect, paint: &Paint) {
        self.sync().draw_oval(oval, paint);
    }

    fn draw_rrect(&mut self, rr: &RRect, paint: &Paint) {
        self.sync().draw_rrect(rr, paint);
    }

    fn draw_path(&mut self, path: &Path, paint: &Paint) {
        self.sync().draw_path(path, paint);
    }

    fn draw_drrect(&mut self, outer: &RRect, inner: &RRect, paint: &Paint) {
        self.sync().draw_drrect(outer, inner, paint);
    }

    fn draw_arc(&mut self, arc: &skia_rust_core::arc::Arc, paint: &Paint) {
        self.sync().draw_arc(arc, paint);
    }

    // Port of: src/gpu/graphite/Device.h#L238-L240 (chrome/m156)
    fn on_draw_glyph_run_list(
        &mut self,
        _list: &skia_rust_core::glyph_run::GlyphRunList<'_>,
        _paint: &Paint,
    ) {
        // Text is drawn through the sub run container of `text_gpu` (G12b).
        skia_log_w!("Device::onDrawGlyphRunList needs text_gpu (G12b); the text is not drawn.");
    }

    fn draw_vertices(
        &mut self,
        vertices: &Vertices,
        blender: Blender,
        paint: &Paint,
        skip_color_xform: bool,
    ) {
        self.sync()
            .draw_vertices(vertices, &blender, paint, skip_color_xform);
    }

    fn draw_mesh(&mut self, mesh: &Mesh, blender: Blender, paint: &Paint) {
        self.sync().draw_mesh(mesh, &blender, paint);
    }

    // Port of: src/gpu/graphite/Device.cpp#L1100-L1132 (chrome/m156)
    fn draw_atlas(
        &mut self,
        xform: &[RSXform],
        tex: &[SkRect],
        colors: &[Color],
        blender: Blender,
        paint: &Paint,
    ) {
        self.sync().draw_atlas(xform, tex, colors, &blender, paint);
    }

    fn use_draw_coverage_mask_for_mask_filters(&self) -> bool {
        true
    }

    fn draw_image_rect(
        &mut self,
        image: &Image,
        src: Option<&SkRect>,
        dst: &SkRect,
        sampling: &SamplingOptions,
        paint: &Paint,
        constraint: SrcRectConstraint,
    ) {
        self.sync()
            .draw_image_rect(image, src, dst, sampling, paint, constraint);
    }

    // Port of: src/gpu/graphite/Device.cpp#L1072-L1098 (chrome/m156)
    fn draw_image_lattice(
        &mut self,
        image: &Image,
        lattice: &skia_rust_core::lattice_iter::Lattice<'_>,
        dst: &SkRect,
        filter: skia_rust_core::sampling_options::FilterMode,
        paint: &Paint,
    ) {
        let mut iter = skia_rust_core::lattice_iter::LatticeIter::new(lattice, dst);
        while let Some(patch) = iter.next_patch() {
            let src_r = SkRect::from_irect(patch.src);
            // Use non-AA quads to match Ganesh and Raster backends behavior of drawImageLattice.
            if let Some(color) = patch.fixed_color {
                self.sync()
                    .draw_image_lattice_patch(&patch.dst, color, paint);
            } else {
                let entry = ImageSetEntry {
                    image: image.clone(),
                    src_rect: src_r,
                    dst_rect: patch.dst,
                    matrix_index: -1,
                    alpha: 1.0,
                    aa_flags: EdgeFlags::NONE,
                    has_clip: false,
                };
                self.sync().draw_edge_aa_image_set(
                    std::slice::from_ref(&entry),
                    None,
                    None,
                    &SamplingOptions::from(filter),
                    paint,
                    SrcRectConstraint::Strict,
                );
            }
        }
    }

    // Port of: src/gpu/graphite/Device.cpp#L726-L749 (chrome/m156)
    fn on_read_pixels(&mut self, _dst: &mut Pixmap<'_>, _x: i32, _y: i32) -> bool {
        // We have no access to a context to do a read pixels here (`GPU_TEST_UTILS` reads go
        // through `Context::priv().readPixels()`, G9b/G11c).
        false
    }

    fn on_write_pixels(&mut self, src: &Pixmap<'_>, x: i32, y: i32) -> bool {
        self.sync().on_write_pixels(src, x, y)
    }
}
