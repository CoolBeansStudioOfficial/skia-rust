// Copyright 2006 Google Inc.
// Copyright 2008 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkCanvas.h, src/core/SkCanvas.cpp

//! `SkCanvas`: the drawing interface (raster subset).
//!
//! The API mirrors `skia-safe`'s `Canvas`: every method takes `&self` (the state is behind a
//! `RefCell`, so a canvas is neither `Send` nor `Sync`, like skia-safe's) and drawing methods
//! return `&Self`. A canvas owns its devices: the root device (a [`Device`], normally the raster
//! `BitmapDevice` made by `skia_rust_raster`) and one per active `saveLayer`.
//!
//! skia-rust deviations (see the "As implemented in D6" design note):
//! * Constructors that make a raster device (`from_raster_direct`, `from_bitmap`, surfaces) live
//!   in `skia_rust_raster` (core cannot name `BitmapDevice`); [`Canvas::from_device`] is the
//!   generic one. A canvas that draws into caller pixels owns them for its lifetime
//!   ([`OwnedCanvas`] hands them back on drop), as `docs/design/pixels.md` decided.
//! * `peek_pixels` and `access_top_layer_pixels` return guards ([`PeekedPixels`],
//!   [`TopLayerPixels`]) because a `Pixmap` cannot outlive the `RefCell` borrow.
//! * Not ported (TODO Phase 3, each a no-op or a documented simplification here):
//!   text, drawables, shadows, meshes, annotations,
//!   edge-AA quads (`SkFont`, ... are not ported); image filters on
//!   paints and layers (`AutoLayerForImageFilter`, `internalDrawDeviceWithFilter`, backdrops;
//!   `skif` is Phase 3); `saveBehind`/`drawClippedToSaveBehind` (Android only); mask filter
//!   auto-layers (`useDrawCoverageMaskForMaskFilters` is false for the raster device); the
//!   blurred-rrect fast path; `SkRasterHandleAllocator`; `SkNWayCanvas`. The virtual
//!   `willSave`/`didConcat`/`onDrawRect`/... of a subclass are [`CanvasHooks`] (D7), which pictures
//!   record through.

use std::cell::{Cell, Ref, RefCell, RefMut};
use std::ops::Deref;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::alpha_type::AlphaType;
use crate::arc::Arc;
use crate::bitmap::Bitmap;
use crate::blend_mode::BlendMode;
use crate::blender::Blender;
use crate::canvas_priv::{AutoCanvasMatrixPaint, MAX_PICTURE_OPS_TO_UNROLL_INSTEAD_OF_REF};
use crate::clip_op::ClipOp;
use crate::color::{Color, Color4f};
use crate::color_space::ColorSpace;
use crate::color_type::ColorType;
use crate::device::{CreateInfo, Device, NoPixelsDevice, clip_shader};
use crate::floating_point::is_finite_all;
use crate::font::Font;
use crate::font_types::{GlyphId, TextEncoding};
use crate::glyph_run::{GlyphRun, GlyphRunBuilder, GlyphRunList};
use crate::image::Image;
use crate::image_filter::ImageFilter;
use crate::image_filter_types::{Mapping, MatrixCapability, ROUND_EPSILON, round_out};
use crate::image_info::ImageInfo;
use crate::lattice_iter::LatticeIter;
use crate::m44::M44;
use crate::matrix::Matrix;
use crate::matrix_priv::map_rect;
use crate::paint::{Paint, Style};
use crate::path::Path;
use crate::picture::Picture;
use crate::pixmap::Pixmap;
use crate::point::{IPoint, Point, Vector};
use crate::rect::{Contains, IRect, Rect, RoundOut};
use crate::region::Region;
use crate::rrect::RRect;
use crate::rsxform::RSXform;
use crate::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use crate::scalar::scalar;
use crate::shader::Shader;
use crate::size::ISize;
use crate::slug::Slug;
use crate::surface_props::{PixelGeometry, SurfaceProps};
use crate::text_blob::TextBlob;
use crate::tile_mode::TileMode;
use crate::utils::patch_utils;
use crate::vertices::{VertexMode, Vertices};

/// The lattice of [`Canvas::draw_image_lattice`] (`skia_safe::canvas::lattice`).
pub mod lattice {
    pub use crate::lattice_iter::{Lattice, RectType};
}

pub use lattice::Lattice;

/// How [`Device::draw_points`](crate::device::Device::draw_points) interprets its points
/// (`SkCanvas::PointMode`).
// Port of: include/core/SkCanvas.h#L1295-L1299 (chrome/m156)
#[doc(alias = "SkCanvas::PointMode")]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
#[repr(i32)]
pub enum PointMode {
    /// Draws each point separately (`kPoints_PointMode`).
    Points = 0,
    /// Draws each pair of points as a line segment (`kLines_PointMode`).
    Lines = 1,
    /// Draws the array of points as a polyline (`kPolygon_PointMode`).
    Polygon = 2,
}

/// Controls the behavior at the edge of the source rect when sampling filters
/// (`SkCanvas::SrcRectConstraint`).
// Port of: include/core/SkCanvas.h#L1456-L1459 (chrome/m156)
#[doc(alias = "SkCanvas::SrcRectConstraint")]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, Default)]
#[repr(i32)]
pub enum SrcRectConstraint {
    /// Sampling is guaranteed to stay inside the source rect (`kStrict_SrcRectConstraint`).
    Strict = 0,
    /// Sampling may bleed outside the source rect (`kFast_SrcRectConstraint`).
    #[default]
    Fast = 1,
}

bitflags::bitflags! {
    /// Flags for [`Canvas::save_layer`] (`SkCanvas::SaveLayerFlags`).
    // Port of: include/core/SkCanvas.h#L685-L704 (chrome/m156)
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
    pub struct SaveLayerFlags: u32 {
        /// `kPreserveLCDText_SaveLayerFlag`.
        const PRESERVE_LCD_TEXT = 1 << 1;
        /// `kInitWithPrevious_SaveLayerFlag`: initializes with the previous contents.
        const INIT_WITH_PREVIOUS = 1 << 2;
        /// `kF16ColorType`.
        const F16_COLOR_TYPE = 1 << 4;
    }
}

/// The state used to create a layer (`SkCanvas::SaveLayerRec`).
///
/// skia-rust: `fFilters` (the multi-filter span) is not here; image filters are Phase 3.
// Port of: include/core/SkCanvas.h#L706-L800 (chrome/m156)
#[doc(alias = "SkCanvas::SaveLayerRec")]
#[derive(Debug, Clone)]
pub struct SaveLayerRec<'a> {
    pub(crate) bounds: Option<&'a Rect>,
    pub(crate) paint: Option<&'a Paint>,
    pub(crate) backdrop: Option<&'a ImageFilter>,
    pub(crate) backdrop_tile_mode: TileMode,
    pub(crate) color_space: Option<&'a ColorSpace>,
    pub(crate) flags: SaveLayerFlags,
}

impl Default for SaveLayerRec<'_> {
    /// No bounds, no paint, no backdrop, no flags.
    fn default() -> Self {
        SaveLayerRec {
            bounds: None,
            paint: None,
            backdrop: None,
            backdrop_tile_mode: TileMode::Clamp,
            color_space: None,
            flags: SaveLayerFlags::empty(),
        }
    }
}

impl<'a> SaveLayerRec<'a> {
    /// Hints at layer size limit.
    #[must_use]
    pub fn bounds(mut self, bounds: &'a Rect) -> Self {
        self.bounds = Some(bounds);
        self
    }

    /// Modifies the layer when it is restored.
    #[must_use]
    pub fn paint(mut self, paint: &'a Paint) -> Self {
        self.paint = Some(paint);
        self
    }

    /// A backdrop filter (initializes the layer with the filtered previous contents). Image
    /// filters are not ported yet, so a backdrop only makes the layer start from the previous
    /// contents.
    #[must_use]
    pub fn backdrop(mut self, backdrop: &'a ImageFilter) -> Self {
        self.backdrop = Some(backdrop);
        self
    }

    /// The tile mode applied to the boundary of the prior layer's image.
    #[must_use]
    pub fn backdrop_tile_mode(mut self, backdrop_tile_mode: TileMode) -> Self {
        self.backdrop_tile_mode = backdrop_tile_mode;
        self
    }

    /// The color space the layer is drawn in.
    #[must_use]
    pub fn color_space(mut self, color_space: &'a ColorSpace) -> Self {
        self.color_space = Some(color_space);
        self
    }

    /// Preserves LCD text, creates with prior layer contents.
    #[must_use]
    pub fn flags(mut self, flags: SaveLayerFlags) -> Self {
        self.flags = flags;
        self
    }
}

/// What a canvas should do for a `saveLayer` (`SkCanvas::SaveLayerStrategy`).
// Port of: include/core/SkCanvas.h#L2213-L2217 (chrome/m156)
#[doc(alias = "SkCanvas::SaveLayerStrategy")]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, Default)]
pub enum SaveLayerStrategy {
    /// `kFullLayer_SaveLayerStrategy`: allocate a layer device.
    #[default]
    FullLayer,
    /// `kNoLayer_SaveLayerStrategy`: the layer has no pixels (a no-pixels device stands in).
    NoLayer,
}

/// The virtual notification and draw hooks of `SkCanvas` (`willSave`, `getSaveLayerStrategy`,
/// `didConcat44`, `onClipRect`, `onDrawRect`, ...), for the things Skia does with a subclass:
/// recording (`SkRecordCanvas`) and observing (test canvases that count saves).
///
/// skia-rust: Rust has no inheritance, so a canvas can carry one `CanvasHooks` object
/// ([`Canvas::set_hooks`]). The notifications (`will_*`, `did_*`, `on_clip_*`) are called where the
/// virtual is called in `SkCanvas`, after the base class work for the `on_clip_*` ones (the
/// `INHERITED` call). A draw hook returns `true` if it handled the draw, which stands for an
/// override that does not call the base class; `false` (the default) lets the canvas draw as the
/// base `SkCanvas` does.
#[doc(alias = "SkCanvas")]
pub trait CanvasHooks {
    /// `willSave`.
    fn will_save(&mut self) {}
    /// `getSaveLayerStrategy`.
    fn get_save_layer_strategy(&mut self, _rec: &SaveLayerRec<'_>) -> SaveLayerStrategy {
        SaveLayerStrategy::FullLayer
    }
    /// `willRestore`.
    fn will_restore(&mut self) {}
    /// `didRestore`, with the total matrix after the restore.
    fn did_restore(&mut self, _total_matrix: &Matrix) {}
    /// `didConcat44`.
    fn did_concat44(&mut self, _m: &M44) {}
    /// `didSetM44`.
    fn did_set_m44(&mut self, _m: &M44) {}
    /// `didScale`.
    fn did_scale(&mut self, _sx: scalar, _sy: scalar) {}
    /// `didTranslate`.
    fn did_translate(&mut self, _dx: scalar, _dy: scalar) {}
    /// `onClipRect` (`is_aa` is `kSoft_ClipEdgeStyle`).
    fn on_clip_rect(&mut self, _rect: &Rect, _op: ClipOp, _is_aa: bool) {}
    /// `onClipRRect`.
    fn on_clip_rrect(&mut self, _rrect: &RRect, _op: ClipOp, _is_aa: bool) {}
    /// `onClipPath`.
    fn on_clip_path(&mut self, _path: &Path, _op: ClipOp, _is_aa: bool) {}
    /// `onClipShader`.
    fn on_clip_shader(&mut self, _shader: &Shader, _op: ClipOp) {}
    /// `onClipRegion`.
    fn on_clip_region(&mut self, _device_rgn: &Region, _op: ClipOp) {}
    /// `onResetClip`.
    fn on_reset_clip(&mut self) {}
    /// `onDrawPaint`.
    fn on_draw_paint(&mut self, _paint: &Paint) -> bool {
        false
    }
    /// `onDrawPoints`.
    fn on_draw_points(&mut self, _mode: PointMode, _pts: &[Point], _paint: &Paint) -> bool {
        false
    }
    /// `onDrawRect`.
    fn on_draw_rect(&mut self, _rect: &Rect, _paint: &Paint) -> bool {
        false
    }
    /// `onDrawRegion`.
    fn on_draw_region(&mut self, _region: &Region, _paint: &Paint) -> bool {
        false
    }
    /// `onDrawOval`.
    fn on_draw_oval(&mut self, _oval: &Rect, _paint: &Paint) -> bool {
        false
    }
    /// `onDrawArc`.
    fn on_draw_arc(
        &mut self,
        _oval: &Rect,
        _start_angle: scalar,
        _sweep_angle: scalar,
        _use_center: bool,
        _paint: &Paint,
    ) -> bool {
        false
    }
    /// `onDrawRRect`.
    fn on_draw_rrect(&mut self, _rrect: &RRect, _paint: &Paint) -> bool {
        false
    }
    /// `onDrawDRRect`.
    fn on_draw_drrect(&mut self, _outer: &RRect, _inner: &RRect, _paint: &Paint) -> bool {
        false
    }
    /// `onDrawPath`.
    fn on_draw_path(&mut self, _path: &Path, _paint: &Paint) -> bool {
        false
    }
    /// `onDrawTextBlob`: `SkRecordCanvas` records the blob by reference instead of drawing it.
    // Port of: src/core/SkCanvas.h (onDrawTextBlob, chrome/m156), overridden by SkRecordCanvas
    fn on_draw_text_blob(
        &mut self,
        _blob: &TextBlob,
        _x: scalar,
        _y: scalar,
        _paint: &Paint,
    ) -> bool {
        false
    }
    /// `onDrawVerticesObject`.
    fn on_draw_vertices_object(
        &mut self,
        _vertices: &Vertices,
        _mode: BlendMode,
        _paint: &Paint,
    ) -> bool {
        false
    }
    /// `onDrawPatch`.
    fn on_draw_patch(
        &mut self,
        _cubics: &[Point; patch_utils::NUM_CTRL_PTS],
        _colors: Option<&[Color; patch_utils::NUM_CORNERS]>,
        _tex_coords: Option<&[Point; patch_utils::NUM_CORNERS]>,
        _mode: BlendMode,
        _paint: &Paint,
    ) -> bool {
        false
    }
    /// `onDrawAtlas2`, with the atlas as the shader `atlas->makeShader(sampling)` makes (see
    /// [`Canvas::draw_atlas`]); `None` when that shader is null.
    #[allow(clippy::too_many_arguments)] // mirrors onDrawAtlas2
    fn on_draw_atlas2(
        &mut self,
        _atlas_shader: Option<&Shader>,
        _xform: &[RSXform],
        _tex: &[Rect],
        _colors: &[Color],
        _mode: BlendMode,
        _cull: Option<&Rect>,
        _paint: Option<&Paint>,
    ) -> bool {
        false
    }
    /// `onDrawPicture`.
    fn on_draw_picture(
        &mut self,
        _picture: &Picture,
        _matrix: Option<&Matrix>,
        _paint: Option<&Paint>,
    ) -> bool {
        false
    }
    /// `onDrawImageRect2` (`SkCanvas::onDrawImage2` is unreachable in Skia and has no hook).
    fn on_draw_image_rect2(
        &mut self,
        _image: &Image,
        _src: &Rect,
        _dst: &Rect,
        _sampling: &SamplingOptions,
        _paint: Option<&Paint>,
        _constraint: SrcRectConstraint,
    ) -> bool {
        false
    }
    /// `onDrawImageLattice2`.
    fn on_draw_image_lattice2(
        &mut self,
        _image: &Image,
        _lattice: &Lattice<'_>,
        _dst: &Rect,
        _filter: FilterMode,
        _paint: Option<&Paint>,
    ) -> bool {
        false
    }
}

/// Whether the content of a surface is about to be discarded or kept
/// (`SkSurface::ContentChangeMode`).
// Port of: include/core/SkSurface.h#L216-L219 (chrome/m156)
#[doc(alias = "SkSurface::ContentChangeMode")]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum ContentChangeMode {
    /// `kDiscard_ContentChangeMode`: discards the surface's contents.
    Discard,
    /// `kRetain_ContentChangeMode`: preserves the surface's contents.
    Retain,
}

/// The part of `SkSurface_Base` a canvas talks to: the generation ID that changes whenever the
/// canvas is about to draw. Shared between a surface and its canvas.
///
/// skia-rust: the cached image snapshot (`fCachedImage`) is dropped by `aboutToDraw`. Copy on
/// write (`onCopyOnWrite`, `onRestoreBackingMutability`) is the pixel refs' job: a snapshot
/// shares the surface's pixel ref and the surface's next write detaches onto a copy
/// (`docs/design/pixels.md`).
// Port of: src/image/SkSurface_Base.cpp#L55-L91 (chrome/m156)
#[doc(alias = "SkSurface_Base")]
#[derive(Debug, Default)]
pub struct SurfaceBase {
    generation_id: Cell<u32>,
    cached_image: RefCell<Option<Image>>,
}

impl SurfaceBase {
    /// A base with a generation ID not yet assigned.
    #[must_use]
    pub fn new() -> SurfaceBase {
        SurfaceBase::default()
    }

    /// `SkSurface_Base::newGenerationID`.
    // Port of: src/image/SkSurface_Base.cpp#L93-L97 (chrome/m156)
    fn new_generation_id() -> u32 {
        static NEXT_ID: AtomicU32 = AtomicU32::new(1);
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    }

    /// The generation ID, changed by every draw that may change the pixels (`generationID`).
    // Port of: src/core/SkSurface.cpp#L64-L72 (chrome/m156)
    #[doc(alias = "generationID")]
    #[must_use]
    pub fn generation_id(&self) -> u32 {
        // We "lazily" assign the ID, since it is not needed until it is asked for.
        if self.generation_id.get() == 0 {
            self.generation_id.set(Self::new_generation_id());
        }
        self.generation_id.get()
    }

    /// Invalidates the generation ID (`dirtyGenerationID`).
    #[doc(alias = "dirtyGenerationID")]
    pub fn dirty_generation_id(&self) {
        self.generation_id.set(0);
    }

    /// Called before every draw (`aboutToDraw`); false if the draw must not happen.
    // Port of: src/image/SkSurface_Base.cpp#L65-L91 (chrome/m156)
    #[doc(alias = "aboutToDraw")]
    #[must_use]
    pub fn about_to_draw(&self, _mode: ContentChangeMode) -> bool {
        self.dirty_generation_id();
        // regardless of copy-on-write, we must drop our cached image now, so that the next
        // request will get our new contents.
        self.cached_image.borrow_mut().take();
        true
    }

    /// The cached image snapshot, if there is one (`fCachedImage`).
    #[must_use]
    pub fn cached_image(&self) -> Option<Image> {
        self.cached_image.borrow().clone()
    }

    /// Caches `image` as the snapshot of the surface until its next draw.
    pub fn set_cached_image(&self, image: Image) {
        *self.cached_image.borrow_mut() = Some(image);
    }

    /// `SkSurface::notifyContentWillChange`.
    // Port of: src/core/SkSurface.cpp#L74-L77 (chrome/m156)
    #[doc(alias = "notifyContentWillChange")]
    pub fn notify_content_will_change(&self, mode: ContentChangeMode) {
        let _ = self.about_to_draw(mode);
    }
}

/// A layer made by `saveLayer` (`SkCanvas::Layer`). Its device is in the canvas's device stack.
// Port of: src/core/SkCanvas.cpp#L178-L193 (chrome/m156)
#[derive(Debug)]
struct Layer {
    paint: Paint,
    is_coverage: bool,
    discard: bool,
    includes_padding: bool,
}

/// One entry of the matrix/clip stack (`SkCanvas::MCRec`).
// Port of: src/core/SkCanvas.cpp#L202-L232 (chrome/m156)
#[derive(Debug)]
struct MCRec {
    /// Index into the canvas's devices.
    device: usize,
    matrix: M44,
    deferred_save_count: i32,
    layer: Option<Layer>,
}

impl MCRec {
    fn from_prev(prev: &MCRec) -> MCRec {
        MCRec {
            device: prev.device,
            matrix: prev.matrix,
            deferred_save_count: 0,
            layer: None,
        }
    }
}

/// Where glyphs are drawn: at points, or with `RSXform`s (the `positions` of `drawGlyphs`).
#[doc(alias = "SkCanvas::drawGlyphs")]
#[derive(Clone, Copy, Debug)]
pub enum GlyphPositions<'a> {
    /// One point per glyph.
    Points(&'a [Point]),
    /// One rotation-scale transform per glyph.
    RSXforms(&'a [RSXform]),
}

impl<'a> From<&'a [Point]> for GlyphPositions<'a> {
    fn from(points: &'a [Point]) -> Self {
        GlyphPositions::Points(points)
    }
}

impl<'a> From<&'a [RSXform]> for GlyphPositions<'a> {
    fn from(xforms: &'a [RSXform]) -> Self {
        GlyphPositions::RSXforms(xforms)
    }
}

/// The predraw flags (`SkCanvas::PredrawFlags`).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
struct PredrawFlags(u32);

impl PredrawFlags {
    const NONE: PredrawFlags = PredrawFlags(0);
    const CHECK_FOR_OVERWRITE: PredrawFlags = PredrawFlags(1);

    fn has(self, other: PredrawFlags) -> bool {
        self.0 & other.0 != 0
    }
}

/// The state of a canvas (the data members of `SkCanvas`).
struct CanvasState {
    props: SurfaceProps,
    save_count: i32,
    mc_stack: Vec<MCRec>,
    /// `devices[0]` is the root device; each layer pushes another.
    devices: Vec<Box<dyn Device>>,
    /// `fQuickRejectBounds`.
    quick_reject_bounds: Rect,
    clip_restriction_rect: IRect,
    clip_restriction_save_count: i32,
    surface: Option<Rc<SurfaceBase>>,
    /// The subclass hooks (see [`CanvasHooks`]).
    hooks: Option<Box<dyn CanvasHooks>>,
}

impl std::fmt::Debug for CanvasState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Canvas")
            .field("save_count", &self.save_count)
            .field("quick_reject_bounds", &self.quick_reject_bounds)
            .field("devices", &self.devices.len())
            .finish_non_exhaustive()
    }
}

// Port of: src/core/SkCanvas.cpp#L671-L680 (chrome/m156)
fn image_filter_color_type(dst_info: &crate::image_info::ColorInfo) -> ColorType {
    if dst_info.bytes_per_pixel() <= 4
        && dst_info.color_type() != ColorType::RGBA8888
        && dst_info.color_type() != ColorType::BGRA8888
    {
        // "Upgrade" A8, G8, 565, 4444, 1010102, 101010x, and 888x to 8888
        ColorType::N32
    } else {
        dst_info.color_type()
    }
}

impl CanvasState {
    fn mc_rec(&self) -> &MCRec {
        self.mc_stack.last().expect("the MC stack is never empty")
    }

    fn mc_rec_mut(&mut self) -> &mut MCRec {
        self.mc_stack
            .last_mut()
            .expect("the MC stack is never empty")
    }

    fn top_device(&self) -> &(dyn Device + 'static) {
        &*self.devices[self.mc_rec().device]
    }

    fn top_device_mut(&mut self) -> &mut (dyn Device + 'static) {
        let idx = self.mc_rec().device;
        &mut *self.devices[idx]
    }

    fn root_device(&self) -> &(dyn Device + 'static) {
        &*self.devices[0]
    }

    fn root_device_mut(&mut self) -> &mut (dyn Device + 'static) {
        &mut *self.devices[0]
    }

    // Port of: src/core/SkCanvas.cpp#L304-L322 (chrome/m156)
    fn init(device: Box<dyn Device>, props: SurfaceProps) -> CanvasState {
        // The root device and the canvas should always have the same pixel geometry
        debug_assert_eq!(
            props.pixel_geometry(),
            device.state().surface_props().pixel_geometry()
        );
        let mut state = CanvasState {
            props,
            save_count: 1,
            mc_stack: vec![MCRec {
                device: 0,
                matrix: M44::new_identity(),
                deferred_save_count: 0,
                layer: None,
            }],
            devices: vec![device],
            quick_reject_bounds: Rect::new_empty(),
            clip_restriction_rect: IRect::new_empty(),
            clip_restriction_save_count: -1,
            surface: None,
            hooks: None,
        };
        state.quick_reject_bounds = state.compute_device_clip_bounds(true);
        state
    }

    // Port of: src/core/SkCanvas.cpp#L145-L154 (chrome/m156)
    fn predraw_notify_overwrite(&mut self, will_overwrites_entire_surface: bool) -> bool {
        if let Some(surface) = &self.surface {
            return surface.about_to_draw(if will_overwrites_entire_surface {
                ContentChangeMode::Discard
            } else {
                ContentChangeMode::Retain
            });
        }
        true
    }

    // Port of: src/core/SkCanvas.cpp#L156-L174 (chrome/m156)
    fn predraw_notify_with(
        &mut self,
        _rect: Option<&Rect>,
        _paint: Option<&Paint>,
        _flags: PredrawFlags,
    ) -> bool {
        // Skia only evaluates `wouldOverwriteEntireSurface` when an image snapshot of the surface
        // is outstanding (it picks `kDiscard` vs `kRetain` for the copy-on-write). There is no
        // `SkImage` here, so there is never one and the mode does not matter.
        if let Some(surface) = &self.surface {
            return surface.about_to_draw(ContentChangeMode::Retain);
        }
        true
    }

    fn predraw_notify(&mut self) -> bool {
        self.predraw_notify_overwrite(false)
    }

    /// `aboutToDraw`: notifies the surface and returns whether to draw. (The
    /// `AutoLayerForImageFilter` that would wrap the paint in a layer for an image filter is a
    /// TODO(Phase 3); the mask filter layer is skipped because the raster device does not use
    /// `drawCoverageMask`.)
    // Port of: src/core/SkCanvas.cpp#L258-L277 (chrome/m156)
    fn about_to_draw(
        &mut self,
        paint: &Paint,
        raw_bounds: Option<&Rect>,
        flags: PredrawFlags,
    ) -> bool {
        if flags.has(PredrawFlags::CHECK_FOR_OVERWRITE) {
            if !self.predraw_notify_with(raw_bounds, Some(paint), flags) {
                return false;
            }
        } else if !self.predraw_notify() {
            return false;
        }
        true
    }

    // Port of: src/core/SkCanvas.cpp#L369-L371 (chrome/m156)
    fn base_layer_size(&self) -> ISize {
        self.root_device().state().image_info().dimensions()
    }

    // Port of: src/core/SkCanvas.cpp#L378-L421 (chrome/m156)
    fn read_pixels(&mut self, pm: &mut Pixmap<'_>, x: i32, y: i32) -> bool {
        pm.addr().is_some() && self.root_device_mut().read_pixels(pm, x, y)
    }

    // Port of: src/core/SkCanvas.cpp#L399-L421 (chrome/m156)
    fn write_pixels(&mut self, src: &Pixmap<'_>, x: i32, y: i32) -> bool {
        let (dev_w, dev_h, dev_dims) = {
            let device = self.root_device();
            (
                device.state().width(),
                device.state().height(),
                device.state().image_info().dimensions(),
            )
        };

        // This check gives us an early out and prevents generation ID churn on the surface.
        // This is purely optional: it is a subset of the checks performed by SkWritePixelsRec.
        let Some(src_rect) = IRect::intersect(
            &IRect::from_xywh(x, y, src.info().width(), src.info().height()),
            &IRect::new(0, 0, dev_w, dev_h),
        ) else {
            return false;
        };

        // Tell our owning surface to bump its generation ID.
        let complete_overwrite = src_rect.size() == dev_dims;
        if !self.predraw_notify_overwrite(complete_overwrite) {
            return false;
        }

        // This can still fail, most notably in the case of a invalid color type or alpha type
        // conversion.
        self.root_device_mut().write_pixels(src, x, y)
    }

    // Port of: src/core/SkCanvas.cpp#L425-L429 (chrome/m156)
    fn check_for_deferred_save(&mut self) {
        if self.mc_rec().deferred_save_count > 0 {
            self.do_save();
        }
    }

    // Port of: src/core/SkCanvas.cpp#L447-L451 (chrome/m156)
    fn save(&mut self) -> i32 {
        self.save_count += 1;
        self.mc_rec_mut().deferred_save_count += 1;
        self.save_count - 1 // return our prev value
    }

    // Port of: src/core/SkCanvas.cpp#L453-L459 (chrome/m156)
    fn do_save(&mut self) {
        if let Some(hooks) = self.hooks.as_mut() {
            hooks.will_save();
        }
        debug_assert!(self.mc_rec().deferred_save_count > 0);
        self.mc_rec_mut().deferred_save_count -= 1;
        self.internal_save();
    }

    // Port of: src/core/SkCanvas.cpp#L461-L476 (chrome/m156)
    fn restore(&mut self) {
        if self.mc_rec().deferred_save_count > 0 {
            debug_assert!(self.save_count > 1);
            self.save_count -= 1;
            self.mc_rec_mut().deferred_save_count -= 1;
        } else {
            // check for underflow
            if self.mc_stack.len() > 1 {
                if let Some(hooks) = self.hooks.as_mut() {
                    hooks.will_restore();
                }
                debug_assert!(self.save_count > 1);
                self.save_count -= 1;
                self.internal_restore();
                let total_matrix = self.total_matrix();
                if let Some(hooks) = self.hooks.as_mut() {
                    hooks.did_restore(&total_matrix);
                }
            }
        }
    }

    // Port of: src/core/SkCanvas.cpp#L478-L488 (chrome/m156)
    fn restore_to_count(&mut self, count: i32) {
        // safety check
        let count = count.max(1);

        let n = self.save_count - count;
        for _ in 0..n {
            self.restore();
        }
    }

    // Port of: src/core/SkCanvas.cpp#L490-L494 (chrome/m156)
    fn internal_save(&mut self) {
        let rec = MCRec::from_prev(self.mc_rec());
        self.mc_stack.push(rec);

        self.top_device_mut().push_clip_stack();
    }

    // Port of: src/core/SkCanvas.cpp#L496-L512 (chrome/m156)
    fn save_layer(&mut self, rec: &SaveLayerRec<'_>) -> i32 {
        if rec.paint.is_some_and(|p| self.nothing_to_draw(p)) {
            // no need for the layer (or any of the draws until the matching restore()
            self.save();
            self.clip_rect(&Rect::new_empty(), ClipOp::Intersect, false);
        } else {
            let strategy = self
                .hooks
                .as_mut()
                .map_or(SaveLayerStrategy::FullLayer, |h| {
                    h.get_save_layer_strategy(rec)
                });
            self.save_count += 1;
            self.internal_save_layer(rec, false, strategy);
        }
        self.save_count - 1
    }

    // Port of: src/core/SkCanvas.cpp#L878-L1090 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors internalSaveLayer
    fn internal_save_layer(
        &mut self,
        rec: &SaveLayerRec<'_>,
        coverage_only: bool,
        strategy: SaveLayerStrategy,
    ) {
        // Do this before we create the layer. We don't call the public save() since that would
        // invoke a possibly overridden virtual.
        self.internal_save();

        if self.is_clip_empty() {
            // Early out if the layer wouldn't draw anything
            return;
        }

        // Build up the paint for restoring the layer, taking only the pieces of rec.fPaint that
        // are relevant. Filtering is automatically chosen in internalDrawDeviceWithFilter based
        // on the device's coordinate space.
        let mut restore_paint = rec.paint.cloned().unwrap_or_default();
        restore_paint.set_style(Style::Fill); // a layer is filled out "infinitely"
        restore_paint.set_path_effect(None); // path effects are ignored for saved layers
        restore_paint.set_mask_filter(None); // mask filters are ignored for saved layers
        restore_paint.set_image_filter(None); // the image filter is held separately
        // Smooth non-axis-aligned layer edges; this automatically downgrades to non-AA for
        // aligned layer restores. This is done to match legacy behavior where the post-applied
        // MatrixTransform bilerp also smoothed cropped edges. See skbug.com/40042614
        restore_paint.set_anti_alias(true);

        // TODO(Phase 3, image filters): `paintFilter`/`rec.fFilters` are not supported, so
        // `filters` is always empty.
        let filters_empty = true;
        let cf = restore_paint.color_filter();
        let blender = restore_paint.blender();

        // When this is false, restoring the layer filled with unmodified prior contents should be
        // identical to the prior contents, so we can restrict the layer even more than just the
        // clip bounds.
        let mut filters_prior_device = rec.backdrop.is_some();
        // A regular filter applied to a layer initialized with prior contents is somewhat
        // analogous to a backdrop filter so they are treated the same.
        filters_prior_device |= rec.flags.contains(SaveLayerFlags::INIT_WITH_PREVIOUS)
            && (!filters_empty
                || cf.is_some()
                || blender.is_some()
                || restore_paint.alpha_f() < 1.0);
        // If the restorePaint has a transparency-affecting colorfilter or blender, the output is
        // unbounded during restore(). `internalDrawDeviceWithFilter` automatically applies these
        // effects. When there's no image filter, SkDevice::drawDevice is used, which does not
        // apply effects beyond the layer's image so we mark `trivialRestore` as false too.
        let draw_device_must_fill_clip = filters_empty
            && (cf
                .as_ref()
                .is_some_and(|cf| cf.as_base().affects_transparent_black())
                || blender
                    .as_ref()
                    .is_some_and(|b| b.as_base().affects_transparent_black()));
        let trivial_restore = !filters_prior_device && !draw_device_must_fill_clip;

        // Size the new layer relative to the prior device, which may already be aligned for
        // filters.
        let prior_idx = self.mc_rec().device;
        let prior_local_to_device44 = *self.devices[prior_idx].state().local_to_device44();
        let output_bounds = self.devices[prior_idx].dev_clip_bounds();

        let mut content_bounds = None;
        // Set the bounds hint if provided and there's no further effects on prior device content
        if let Some(b) = rec.bounds
            && trivial_restore
        {
            content_bounds = Some(*b);
        }

        let mapping_and_bounds = get_layer_mapping_and_bounds(
            &prior_local_to_device44,
            &output_bounds,
            content_bounds.as_ref(),
        );

        let Some((new_layer_mapping, layer_bounds)) = mapping_and_bounds else {
            // The filtered content would not draw anything, or the new device space has an
            // invalid coordinate system, in which case we mark the current top device as empty
            // so that nothing draws until the canvas is restored past this saveLayer.
            self.abort_layer();
            return;
        };

        let padded_layer = false;
        if layer_bounds.is_empty() {
            // The image filter graph does not require any input, so we don't need to actually
            // render a new layer for the source image. (Only reachable with filters.)
            self.abort_layer();
            return;
        }
        // TODO(b/329700315): padding is only added with filters.

        let new_device: Option<Box<dyn Device>> = if strategy == SaveLayerStrategy::FullLayer {
            debug_assert!(!layer_bounds.is_empty());

            let prior_info = self.devices[prior_idx].state().image_info().clone();
            let layer_color_type = if coverage_only {
                ColorType::Alpha8
            } else if rec.flags.contains(SaveLayerFlags::F16_COLOR_TYPE) {
                ColorType::RGBAF16
            } else {
                image_filter_color_type(prior_info.color_info())
            };
            let info = ImageInfo::new(
                ISize::new(layer_bounds.width(), layer_bounds.height()),
                layer_color_type,
                AlphaType::Premul,
                rec.color_space
                    .cloned()
                    .or_else(|| prior_info.color_space()),
            );

            let geo = if rec.flags.contains(SaveLayerFlags::PRESERVE_LCD_TEXT) {
                self.props.pixel_geometry()
            } else {
                PixelGeometry::Unknown
            };
            let create_info = CreateInfo::new(info, geo);
            // Use the original paint as a hint so that it includes the image filter
            self.devices[prior_idx].create_device(&create_info, rec.paint)
        } else {
            None
        };

        let mut init_backdrop =
            rec.flags.contains(SaveLayerFlags::INIT_WITH_PREVIOUS) || rec.backdrop.is_some();
        let mut new_device = if let Some(d) = new_device {
            d
        } else {
            // Either we weren't meant to allocate a full layer, or the full layer creation
            // failed. Using an explicit NoPixelsDevice lets us reflect what the layer state would
            // have been on success (or kFull_LayerStrategy) while squashing draw calls that
            // target something that doesn't exist.
            init_backdrop = false;
            let cs = self.root_device().state().image_info().color_space();
            Box::new(NoPixelsDevice::new_with_color_space(
                &IRect::from_wh(layer_bounds.width(), layer_bounds.height()),
                self.props,
                cs,
            ))
        };

        // Clip while the device coordinate space is the identity so it's easy to define the rect
        // that excludes the added padding pixels. This ensures they remain cleared to
        // transparent black.
        if padded_layer {
            let inset = new_device.dev_clip_bounds();
            let inset = IRect::new(
                inset.left + 1,
                inset.top + 1,
                inset.right - 1,
                inset.bottom - 1,
            );
            new_device.clip_rect(&Rect::from_irect(inset), ClipOp::Intersect, false);
        }

        // Configure device to match determined mapping for any image filters.
        // The setDeviceCoordinateSystem applies the prior device's global transform since
        // 'newLayerMapping' only defines the transforms between the two devices and it must be
        // updated to the global coordinate system.
        let prior_state = self.devices[prior_idx].state();
        let device_to_global = prior_state.device_to_global() * new_layer_mapping.layer_to_device();
        let global_to_device =
            new_layer_mapping.device_to_layer_matrix() * prior_state.global_to_device();
        new_device.state_mut().set_device_coordinate_system(
            &device_to_global,
            &global_to_device,
            new_layer_mapping.layer_matrix(),
            layer_bounds.left,
            layer_bounds.top,
        );

        if init_backdrop {
            // TODO(Phase 3, image filters): backdrop filters; a plain `kInitWithPrevious` layer
            // is the prior device drawn into the new one with no filter (the devices differ by
            // an integer translation, so they are always compatible).
            debug_assert!(!coverage_only);
            let backdrop_paint = Paint::default();
            // Draw the prior device (src) into the new device (dst).
            let (before, _) = self.devices.split_at_mut(prior_idx + 1);
            let prior: &mut Box<dyn Device> = &mut before[prior_idx];
            new_device.draw_device(&mut **prior, &SamplingOptions::default(), &backdrop_paint);
        }

        // fMCRec->newLayer(...)
        self.devices.push(new_device);
        let new_idx = self.devices.len() - 1;
        let rec_mut = self.mc_rec_mut();
        rec_mut.layer = Some(Layer {
            paint: restore_paint,
            is_coverage: coverage_only,
            discard: false,
            includes_padding: padded_layer,
        });
        rec_mut.device = new_idx;
        self.quick_reject_bounds = self.compute_device_clip_bounds(true);
    }

    // The `abortLayer` lambda of internalSaveLayer.
    // Port of: src/core/SkCanvas.cpp#L950-L956 (chrome/m156)
    fn abort_layer(&mut self) {
        self.with_qr_update(|s| {
            s.top_device_mut()
                .clip_rect(&Rect::new_empty(), ClipOp::Intersect, false);
        });
    }

    // Port of: src/core/SkCanvas.cpp#L1141-L1211 (chrome/m156)
    fn internal_restore(&mut self) {
        debug_assert!(!self.mc_stack.is_empty());

        // now detach these from fMCRec so we can pop(). Gets freed after its drawn
        let mut rec = self.mc_stack.pop().expect("checked above");
        let layer = rec.layer.take();
        let mut layer_device = if layer.is_some() {
            self.devices.pop()
        } else {
            None
        };

        let matrix = self.mc_rec().matrix;
        self.top_device_mut().pop_clip_stack();
        self.top_device_mut().state_mut().set_global_ctm(&matrix);

        // Draw the layer's device contents into the now-current older device. We can't call
        // public draw functions since we don't want to record them.
        if let (Some(layer), Some(layer_device)) = (layer, layer_device.as_deref_mut())
            && !layer_device.is_no_pixels_device()
            && !layer.discard
        {
            layer_device.set_immutable();

            // Don't go through AutoLayerForImageFilter since device draws are so closely tied to
            // internalSaveLayer and internalRestore.
            if self.predraw_notify() {
                // NOTE: Layers with image filters are TODO(Phase 3); here the layer is always
                // drawn through `drawDevice`.
                debug_assert!(!layer.is_coverage && !layer.includes_padding);
                self.top_device_mut().draw_device(
                    layer_device,
                    &SamplingOptions::default(),
                    &layer.paint,
                );
            }
        }

        // Reset the clip restriction if the restore went past the save point that had added it.
        if self.save_count < self.clip_restriction_save_count {
            self.clip_restriction_rect.set_empty();
            self.clip_restriction_save_count = -1;
        }
        // Update the quick-reject bounds in case the restore changed the top device or the
        // removed save record had included modifications to the clip stack.
        self.quick_reject_bounds = self.compute_device_clip_bounds(true);
    }

    /// Runs `f` (which may change the clip) then recomputes the quick reject bounds
    /// (`AutoUpdateQRBounds`).
    // Port of: src/core/SkCanvas.cpp#L234-L254 (chrome/m156)
    fn with_qr_update(&mut self, f: impl FnOnce(&mut Self)) {
        f(self);
        self.quick_reject_bounds = self.compute_device_clip_bounds(true);
    }

    // Port of: src/core/SkCanvas.cpp#L1296-L1305 (chrome/m156)
    fn translate(&mut self, dx: scalar, dy: scalar) {
        if dx != 0.0 || dy != 0.0 {
            self.check_for_deferred_save();
            self.mc_rec_mut().matrix.pre_translate(dx, dy, None);
            let m = self.mc_rec().matrix;
            self.top_device_mut().state_mut().set_global_ctm(&m);

            if let Some(hooks) = self.hooks.as_mut() {
                hooks.did_translate(dx, dy);
            }
        }
    }

    // Port of: src/core/SkCanvas.cpp#L1307-L1316 (chrome/m156)
    fn scale(&mut self, sx: scalar, sy: scalar) {
        #[allow(clippy::float_cmp)] // mirrors `sx != 1 || sy != 1`
        if sx != 1.0 || sy != 1.0 {
            self.check_for_deferred_save();
            self.mc_rec_mut().matrix.pre_scale(sx, sy);
            let m = self.mc_rec().matrix;
            self.top_device_mut().state_mut().set_global_ctm(&m);

            if let Some(hooks) = self.hooks.as_mut() {
                hooks.did_scale(sx, sy);
            }
        }
    }

    // Port of: src/core/SkCanvas.cpp#L1336-L1349 (chrome/m156)
    fn concat(&mut self, matrix: &Matrix) {
        if matrix.is_identity() {
            return;
        }
        self.concat44(&M44::from(matrix));
    }

    fn concat44(&mut self, m: &M44) {
        self.check_for_deferred_save();

        self.mc_rec_mut().matrix.pre_concat(m);

        let m44 = *m;
        let m = self.mc_rec().matrix;
        self.top_device_mut().state_mut().set_global_ctm(&m);

        // notify subclasses
        if let Some(hooks) = self.hooks.as_mut() {
            hooks.did_concat44(&m44);
        }
    }

    // Port of: src/core/SkCanvas.cpp#L1367-L1371 (chrome/m156)
    fn set_matrix(&mut self, m: &M44) {
        self.check_for_deferred_save();
        self.mc_rec_mut().matrix = *m;
        let m44 = *m;
        let m = self.mc_rec().matrix;
        self.top_device_mut().state_mut().set_global_ctm(&m);
        if let Some(hooks) = self.hooks.as_mut() {
            hooks.did_set_m44(&m44);
        }
    }

    // Port of: src/core/SkCanvas.cpp#L1379-L1394 (chrome/m156)
    fn clip_rect(&mut self, rect: &Rect, op: ClipOp, do_aa: bool) {
        if !rect.is_finite() {
            return;
        }
        self.check_for_deferred_save();
        self.on_clip_rect(&rect.sorted(), op, do_aa);
    }

    fn on_clip_rect(&mut self, rect: &Rect, op: ClipOp, is_aa: bool) {
        debug_assert!(rect.is_sorted());
        self.with_qr_update(|s| s.top_device_mut().clip_rect(rect, op, is_aa));
        if let Some(hooks) = self.hooks.as_mut() {
            hooks.on_clip_rect(rect, op, is_aa);
        }
    }

    // Port of: src/core/SkCanvas.cpp#L1396-L1435 (chrome/m156)
    fn android_framework_set_device_clip_restriction(&mut self, rect: &IRect) {
        // See the long comment in SkCanvas.cpp: the restriction is remembered so that
        // `resetClip` respects it, and reset when the canvas is restored past this save count.
        debug_assert_eq!(self.mc_rec().device, 0); // shouldn't be in a nested layer
        // and shouldn't already have a restriction
        debug_assert!(
            self.clip_restriction_save_count < 0 && self.clip_restriction_rect.is_empty()
        );

        if self.clip_restriction_save_count < 0 && !rect.is_empty() {
            self.clip_restriction_rect = *rect;
            self.clip_restriction_save_count = self.save_count;

            // A non-empty clip restriction immediately applies an intersection op (ignoring the
            // ctm). so we have to resolve the save.
            self.check_for_deferred_save();
            self.with_qr_update(|s| {
                // Use clipRegion() since that operates in canvas-space, whereas clipRect() would
                // apply the device's current transform first.
                s.top_device_mut()
                    .clip_region(&Region::from_rect(rect), ClipOp::Intersect);
            });
        }
    }

    // Port of: src/core/SkCanvas.cpp#L1437-L1455 (chrome/m156)
    fn reset_clip(&mut self) {
        self.check_for_deferred_save();
        let mut device_restriction = self.top_device().state().image_info().bounds();
        if self.clip_restriction_save_count >= 0 && self.mc_rec().device == 0 {
            // Respect the device clip restriction when resetting the clip if we're on the base
            // device. If we're not on the base device, then the "reset" applies to the top
            // device's clip stack, and the clip restriction will be respected automatically
            // during a restore of the layer.
            if let Some(r) = IRect::intersect(&device_restriction, &self.clip_restriction_rect) {
                device_restriction = r;
            } else {
                device_restriction = IRect::new_empty();
            }
        }

        self.with_qr_update(|s| s.top_device_mut().replace_clip(&device_restriction));
        if let Some(hooks) = self.hooks.as_mut() {
            hooks.on_reset_clip();
        }
    }

    // Port of: src/core/SkCanvas.cpp#L1457-L1472 (chrome/m156)
    fn clip_rrect(&mut self, rrect: &RRect, op: ClipOp, do_aa: bool) {
        self.check_for_deferred_save();
        if rrect.is_rect() {
            self.on_clip_rect(rrect.bounds(), op, do_aa);
        } else {
            self.on_clip_rrect(rrect, op, do_aa);
        }
    }

    fn on_clip_rrect(&mut self, rrect: &RRect, op: ClipOp, is_aa: bool) {
        self.with_qr_update(|s| s.top_device_mut().clip_rrect(rrect, op, is_aa));
        if let Some(hooks) = self.hooks.as_mut() {
            hooks.on_clip_rrect(rrect, op, is_aa);
        }
    }

    // Port of: src/core/SkCanvas.cpp#L1474-L1504 (chrome/m156)
    fn clip_path(&mut self, path: &Path, op: ClipOp, do_aa: bool) {
        self.check_for_deferred_save();

        if !path.is_inverse_fill_type() && self.mc_rec().matrix.to_m33().rect_stays_rect() {
            if let Some((r, _, _)) = path.is_rect() {
                self.on_clip_rect(&r, op, do_aa);
                return;
            }
            if let Some(r) = path.is_oval() {
                let mut rrect = RRect::new();
                rrect.set_oval(r);
                self.on_clip_rrect(&rrect, op, do_aa);
                return;
            }
            if let Some(rrect) = path.is_rrect() {
                self.on_clip_rrect(&rrect, op, do_aa);
                return;
            }
        }

        self.on_clip_path(path, op, do_aa);
    }

    fn on_clip_path(&mut self, path: &Path, op: ClipOp, is_aa: bool) {
        self.with_qr_update(|s| s.top_device_mut().clip_path(path, op, is_aa));
        if let Some(hooks) = self.hooks.as_mut() {
            hooks.on_clip_path(path, op, is_aa);
        }
    }

    // Port of: src/core/SkCanvas.cpp#L1506-L1526 (chrome/m156)
    fn clip_shader(&mut self, sh: &Shader, op: ClipOp) {
        if sh.is_opaque() {
            if op == ClipOp::Intersect {
                // we don't occlude anything, so skip this call
            } else {
                debug_assert_eq!(op, ClipOp::Difference);
                // we occlude everything, so set the clip to empty
                self.clip_rect(&Rect::new_empty(), ClipOp::Intersect, false);
            }
        } else {
            self.check_for_deferred_save();
            self.with_qr_update(|s| clip_shader(s.top_device_mut(), sh, op));
            if let Some(hooks) = self.hooks.as_mut() {
                hooks.on_clip_shader(sh, op);
            }
        }
    }

    // Port of: src/core/SkCanvas.cpp#L1528-L1536 (chrome/m156)
    fn clip_region(&mut self, rgn: &Region, op: ClipOp) {
        self.check_for_deferred_save();
        self.with_qr_update(|s| s.top_device_mut().clip_region(rgn, op));
        if let Some(hooks) = self.hooks.as_mut() {
            hooks.on_clip_region(rgn, op);
        }
    }

    // Port of: src/core/SkCanvas.cpp#L1567-L1573 (chrome/m156)
    fn is_clip_empty(&self) -> bool {
        self.top_device().is_clip_empty()
    }

    fn is_clip_rect(&self) -> bool {
        self.top_device().is_clip_rect()
    }

    // Port of: src/core/SkCanvas.cpp#L1575-L1583 (chrome/m156)
    fn quick_reject(&self, src: &Rect) -> bool {
        let dev_rect = map_rect(&self.mc_rec().matrix, src);
        !dev_rect.is_finite() || !dev_rect.intersects(self.quick_reject_bounds)
    }

    fn quick_reject_path(&self, path: &Path) -> bool {
        path.is_empty() || self.quick_reject(path.bounds())
    }

    // Port of: src/core/SkCanvas.cpp#L1589-L1591 (chrome/m156)
    fn nothing_to_draw(&self, paint: &Paint) -> bool {
        !self
            .top_device()
            .state()
            .surface_props()
            .preserves_transparent_draws()
            && paint.nothing_to_draw()
    }

    // Port of: src/core/SkCanvas.cpp#L1593-L1605 (chrome/m156)
    fn internal_quick_reject(&self, bounds: &Rect, paint: &Paint, matrix: Option<&Matrix>) -> bool {
        if !bounds.is_finite() || self.nothing_to_draw(paint) {
            return true;
        }

        if paint.can_compute_fast_bounds() {
            let tmp = matrix.map_or(*bounds, |m| m.map_rect(bounds).0);
            return self.quick_reject(&paint.compute_fast_bounds(&tmp));
        }

        false
    }

    // Port of: src/core/SkCanvas.cpp#L1608-L1624 (chrome/m156)
    fn local_clip_bounds(&self) -> Rect {
        let ibounds = self.device_clip_bounds();
        if ibounds.is_empty() {
            return Rect::new_empty();
        }

        let Some(inverse) = self.mc_rec().matrix.to_m33().invert() else {
            // if we can't invert the CTM, we can't return local clip bounds
            return Rect::new_empty();
        };

        // adjust it outwards in case we are antialiasing
        let margin = 1;

        inverse
            .map_rect(Rect::from_irect(IRect::new(
                ibounds.left - margin,
                ibounds.top - margin,
                ibounds.right + margin,
                ibounds.bottom + margin,
            )))
            .0
    }

    // Port of: src/core/SkCanvas.cpp#L1626-L1628 (chrome/m156)
    fn device_clip_bounds(&self) -> IRect {
        self.compute_device_clip_bounds(false).round_out()
    }

    // Port of: src/core/SkCanvas.cpp#L1630-L1644 (chrome/m156)
    fn compute_device_clip_bounds(&self, outset_for_aa: bool) -> Rect {
        let dev = self.top_device();
        if dev.is_clip_empty() {
            Rect::new_empty()
        } else {
            let mut dev_clip_bounds = map_rect(
                dev.state().device_to_global(),
                &Rect::from_irect(dev.dev_clip_bounds()),
            );
            if outset_for_aa {
                // Expand bounds out by 1 in case we are anti-aliasing.  We store the bounds as
                // floats to enable a faster quick reject implementation.
                dev_clip_bounds.outset((1.0, 1.0));
            }
            dev_clip_bounds
        }
    }

    // Port of: src/core/SkCanvas.cpp#L1648-L1654 (chrome/m156)
    fn total_matrix(&self) -> Matrix {
        self.mc_rec().matrix.to_m33()
    }

    // Port of: src/core/SkCanvas.cpp#L1668-L1689 (chrome/m156)
    fn draw_drrect(&mut self, outer: &RRect, inner: &RRect, paint: &Paint) {
        if outer.is_empty() {
            return;
        }
        if inner.is_empty() {
            self.draw_rrect(outer, paint);
            return;
        }

        // We don't have this method (yet), but technically this is what we should be able to
        // return ...
        // if (!outer.contains(inner))) {
        //
        // For now at least check for containment of bounds
        if !outer.bounds().contains(inner.bounds()) {
            return;
        }

        self.on_draw_drrect(outer, inner, paint);
    }

    // Port of: src/core/SkCanvas.cpp#L1708-L1719 (chrome/m156)
    fn draw_region(&mut self, region: &Region, paint: &Paint) {
        if region.is_empty() {
            return;
        }

        if region.is_rect() {
            let r = Rect::from_irect(region.bounds());
            self.draw_rect(&r, paint);
            return;
        }

        self.on_draw_region(region, paint);
    }

    // Port of: src/core/SkCanvas.cpp#L1696-L1701 (chrome/m156)
    fn draw_rect(&mut self, r: &Rect, paint: &Paint) {
        // To avoid redundant logic in our culling code and various backends, we always sort
        // rects before passing them along.
        self.on_draw_rect(&r.sorted(), paint);
    }

    // Port of: src/core/SkCanvas.cpp#L1721-L1726 (chrome/m156)
    fn draw_oval(&mut self, r: &Rect, paint: &Paint) {
        self.on_draw_oval(&r.sorted(), paint);
    }

    // Port of: src/core/SkCanvas.cpp#L1733-L1738 (chrome/m156)
    fn draw_points(&mut self, mode: PointMode, pts: &[Point], paint: &Paint) {
        if !pts.is_empty() {
            self.on_draw_points(mode, pts, paint);
        }
    }

    // Port of: src/core/SkCanvas.cpp#L1922-L1924 (chrome/m156)
    fn on_draw_paint(&mut self, paint: &Paint) {
        if let Some(hooks) = self.hooks.as_mut()
            && hooks.on_draw_paint(paint)
        {
            return;
        }
        self.internal_draw_paint(paint);
    }

    // Port of: src/core/SkCanvas.cpp#L1922-L1937 (chrome/m156)
    fn internal_draw_paint(&mut self, paint: &Paint) {
        // drawPaint does not call internalQuickReject() because computing its geometry is not
        // free (see getLocalClipBounds(), and the two conditions below are sufficient.
        if self.nothing_to_draw(paint) || self.is_clip_empty() {
            return;
        }

        if self.about_to_draw(paint, None, PredrawFlags::CHECK_FOR_OVERWRITE) {
            self.top_device_mut().draw_paint(paint);
        }
    }

    // Port of: src/core/SkCanvas.cpp#L1939-L1977 (chrome/m156)
    fn on_draw_points(&mut self, mode: PointMode, pts: &[Point], paint: &Paint) {
        if let Some(hooks) = self.hooks.as_mut()
            && hooks.on_draw_points(mode, pts, paint)
        {
            return;
        }
        if pts.is_empty() || self.nothing_to_draw(paint) {
            return;
        }

        // Enforce paint style matches implicit behavior of drawPoints
        let mut stroke_paint = paint.clone();
        stroke_paint.set_style(Style::Stroke);

        let bounds_storage;
        let mut bounds_ptr = None;

        // Computing the bounds can actually slow us down (since we check inside). But if there
        // is a filter, then it is useful to limit the size of its offscreen, hence we only
        // compute it in those cases.
        if paint.image_filter().is_some() || paint.mask_filter().is_some() {
            let Some(bounds) = Rect::bounds(pts) else {
                return;
            };
            if self.internal_quick_reject(&bounds, &stroke_paint, None) {
                return;
            }
            bounds_storage = bounds;
            bounds_ptr = Some(&bounds_storage);
        }

        if self.about_to_draw(&stroke_paint, bounds_ptr, PredrawFlags::NONE) {
            self.top_device_mut().draw_points(mode, pts, &stroke_paint);
        }
    }

    /// `drawVertices` fills triangles and ignores mask filter and path effect, so canonicalize
    /// the paint before checking quick reject.
    // Port of: src/core/SkCanvas.cpp#L2236-L2243 (chrome/m156)
    fn clean_paint_for_draw_vertices(paint: &Paint) -> Paint {
        let mut paint = paint.clone();
        paint.set_style(Style::Fill);
        paint.set_mask_filter(None);
        paint.set_path_effect(None);
        paint
    }

    // Port of: src/core/SkCanvas.cpp#L1745-L1762 (chrome/m156)
    fn draw_vertices(&mut self, vertices: &Vertices, mode: BlendMode, paint: &Paint) {
        // We expect fans to be converted to triangles when building or deserializing SkVertices.
        debug_assert_ne!(vertices.mode(), VertexMode::TriangleFan);

        self.on_draw_vertices_object(vertices, mode, paint);
    }

    // Port of: src/core/SkCanvas.cpp#L2596-L2609 (chrome/m156)
    fn on_draw_vertices_object(&mut self, vertices: &Vertices, bmode: BlendMode, paint: &Paint) {
        if let Some(hooks) = self.hooks.as_mut()
            && hooks.on_draw_vertices_object(vertices, bmode, paint)
        {
            return;
        }
        let simple_paint = Self::clean_paint_for_draw_vertices(paint);

        let bounds = vertices.bounds();
        if self.internal_quick_reject(bounds, &simple_paint, None) {
            return;
        }

        if self.about_to_draw(&simple_paint, Some(bounds), PredrawFlags::NONE) {
            self.top_device_mut().draw_vertices(
                vertices,
                Blender::mode(bmode),
                &simple_paint,
                false,
            );
        }
    }

    // Port of: src/core/SkCanvas.cpp#L2630-L2653 (chrome/m156)
    fn on_draw_patch(
        &mut self,
        cubics: &[Point; patch_utils::NUM_CTRL_PTS],
        colors: Option<&[Color; patch_utils::NUM_CORNERS]>,
        tex_coords: Option<&[Point; patch_utils::NUM_CORNERS]>,
        bmode: BlendMode,
        paint: &Paint,
    ) {
        if let Some(hooks) = self.hooks.as_mut()
            && hooks.on_draw_patch(cubics, colors, tex_coords, bmode, paint)
        {
            return;
        }
        let Some(bounds) = Rect::bounds(cubics) else {
            return; // we don't draw if the bounds are not finite
        };

        // drawPatch has the same behavior restrictions as drawVertices
        let simple_paint = Self::clean_paint_for_draw_vertices(paint);

        // Since a patch is always within the convex hull of the control points, we discard it
        // when its bounding rectangle is completely outside the current clip.
        if self.internal_quick_reject(&bounds, &simple_paint, None) {
            return;
        }

        if self.about_to_draw(&simple_paint, Some(&bounds), PredrawFlags::NONE) {
            self.top_device_mut().draw_patch(
                cubics,
                colors,
                tex_coords,
                Blender::mode(bmode),
                &simple_paint,
            );
        }
    }

    // Port of: src/core/SkCanvas.cpp#L1835-L1850 and #L2687-L2708 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors drawAtlas/onDrawAtlas2
    fn draw_atlas_with_shader(
        &mut self,
        atlas_shader: Option<&Shader>,
        xform: &[RSXform],
        tex: &[Rect],
        colors: &[Color],
        bmode: BlendMode,
        cull: Option<&Rect>,
        paint: Option<&Paint>,
    ) {
        let mut count = xform.len().min(tex.len());
        if !colors.is_empty() {
            count = count.min(colors.len());
        }
        if count == 0 {
            return;
        }

        if let Some(hooks) = self.hooks.as_mut()
            && hooks.on_draw_atlas2(
                atlas_shader,
                &xform[..count],
                &tex[..count],
                if colors.is_empty() {
                    colors
                } else {
                    &colors[..count]
                },
                bmode,
                cull,
                paint,
            )
        {
            return;
        }

        // drawAtlas is a combination of drawVertices and drawImage...
        // (`clean_paint_for_drawImage`)
        let mut image_paint = Paint::default();
        if let Some(paint) = paint {
            image_paint = paint.clone();
            image_paint.set_style(Style::Fill);
            image_paint.set_path_effect(None);
        }
        let mut real_paint = Self::clean_paint_for_draw_vertices(&image_paint);
        real_paint.set_shader(atlas_shader.cloned());

        if let Some(cull) = cull
            && self.internal_quick_reject(cull, &real_paint, None)
        {
            return;
        }

        // drawAtlas should not have mask filters on its paint, so we don't need to worry about
        // converting its "drawImage" behavior into the paint to work with the auto-mask-filter
        // system.
        debug_assert!(real_paint.mask_filter().is_none());
        if self.about_to_draw(&real_paint, None, PredrawFlags::NONE) {
            let colors = if colors.is_empty() {
                colors
            } else {
                &colors[..count]
            };
            self.top_device_mut().draw_atlas(
                &xform[..count],
                &tex[..count],
                colors,
                Blender::mode(bmode),
                &real_paint,
            );
        }
    }

    // Port of: src/core/SkCanvas.cpp#L2036-L2056 (chrome/m156)
    fn on_draw_rect(&mut self, r: &Rect, paint: &Paint) {
        debug_assert!(r.is_sorted());
        if let Some(hooks) = self.hooks.as_mut()
            && hooks.on_draw_rect(r, paint)
        {
            return;
        }
        if self.internal_quick_reject(r, paint, None) {
            return;
        }

        // (canAttemptBlurredRRectDraw is always None for the raster device.)
        if self.about_to_draw(paint, Some(r), PredrawFlags::CHECK_FOR_OVERWRITE) {
            self.top_device_mut().draw_rect(r, paint);
        }
    }

    // Port of: src/core/SkCanvas.cpp#L2058-L2068 (chrome/m156)
    fn on_draw_region(&mut self, region: &Region, paint: &Paint) {
        if let Some(hooks) = self.hooks.as_mut()
            && hooks.on_draw_region(region, paint)
        {
            return;
        }
        let bounds = Rect::from_irect(region.bounds());
        if self.internal_quick_reject(&bounds, paint, None) {
            return;
        }

        if self.about_to_draw(paint, Some(&bounds), PredrawFlags::NONE) {
            self.top_device_mut().draw_region(region, paint);
        }
    }

    // Port of: src/core/SkCanvas.cpp#L2114-L2133 (chrome/m156)
    fn on_draw_oval(&mut self, oval: &Rect, paint: &Paint) {
        debug_assert!(oval.is_sorted());
        if let Some(hooks) = self.hooks.as_mut()
            && hooks.on_draw_oval(oval, paint)
        {
            return;
        }
        if self.internal_quick_reject(oval, paint, None) {
            return;
        }

        if self.about_to_draw(paint, Some(oval), PredrawFlags::NONE) {
            self.top_device_mut().draw_oval(oval, paint);
        }
    }

    // Port of: src/core/SkCanvas.cpp#L2135-L2159 (chrome/m156)
    fn on_draw_arc(
        &mut self,
        oval: &Rect,
        start_angle: scalar,
        sweep_angle: scalar,
        use_center: bool,
        paint: &Paint,
    ) {
        debug_assert!(oval.is_sorted());
        if let Some(hooks) = self.hooks.as_mut()
            && hooks.on_draw_arc(oval, start_angle, sweep_angle, use_center, paint)
        {
            return;
        }
        if self.internal_quick_reject(oval, paint, None) {
            return;
        }

        if self.about_to_draw(paint, Some(oval), PredrawFlags::NONE) {
            self.top_device_mut().draw_arc(
                &Arc::new(*oval, start_angle, sweep_angle, use_center),
                paint,
            );
        }
    }

    // Port of: src/core/SkCanvas.cpp#L2161-L2191 (chrome/m156)
    fn draw_rrect(&mut self, rrect: &RRect, paint: &Paint) {
        if let Some(hooks) = self.hooks.as_mut()
            && hooks.on_draw_rrect(rrect, paint)
        {
            return;
        }
        let bounds = rrect.bounds();

        // Delegating to simpler draw operations
        if rrect.is_rect() {
            // call the non-virtual version
            self.draw_rect(bounds, paint);
            return;
        } else if rrect.is_oval() {
            // call the non-virtual version
            self.draw_oval(bounds, paint);
            return;
        }

        if self.internal_quick_reject(bounds, paint, None) {
            return;
        }

        if self.about_to_draw(paint, Some(bounds), PredrawFlags::NONE) {
            self.top_device_mut().draw_rrect(rrect, paint);
        }
    }

    // Port of: src/core/SkCanvas.cpp#L2193-L2203 (chrome/m156)
    fn on_draw_drrect(&mut self, outer: &RRect, inner: &RRect, paint: &Paint) {
        if let Some(hooks) = self.hooks.as_mut()
            && hooks.on_draw_drrect(outer, inner, paint)
        {
            return;
        }
        let bounds = outer.bounds();
        if self.internal_quick_reject(bounds, paint, None) {
            return;
        }

        if self.about_to_draw(paint, Some(bounds), PredrawFlags::NONE) {
            self.top_device_mut().draw_drrect(outer, inner, paint);
        }
    }

    /// `onDrawTextBlob`: the hook may record the blob; otherwise its glyph runs are drawn.
    // Port of: src/core/SkCanvas.cpp#L2436-L2441 (chrome/m156), onDrawTextBlob
    fn draw_text_blob(&mut self, blob: &TextBlob, x: scalar, y: scalar, paint: &Paint) {
        if let Some(hooks) = self.hooks.as_mut()
            && hooks.on_draw_text_blob(blob, x, y, paint)
        {
            return;
        }
        let mut builder = GlyphRunBuilder::new();
        let list = builder.blob_to_glyph_run_list(blob, Point::new(x, y));
        self.draw_glyph_run_list(&list, paint);
    }

    // Port of: src/core/SkCanvas.cpp#L2443-L2455 (chrome/m156), onDrawGlyphRunList
    fn draw_glyph_run_list(&mut self, list: &GlyphRunList<'_>, paint: &Paint) {
        let bounds = list.source_bounds_with_origin();
        if self.internal_quick_reject(&bounds, paint, None) {
            return;
        }
        // Text attempts to apply any mask filter internally, so this draw does not need the
        // mask filter auto-layer (`kSkipMaskFilterAutoLayer`).
        if self.about_to_draw(paint, Some(&bounds), PredrawFlags::NONE) {
            crate::device::draw_glyph_run_list(self.top_device_mut(), list, paint);
        }
    }

    // Port of: src/core/SkCanvas.cpp#L2205-L2223 (chrome/m156)
    fn draw_path(&mut self, path: &Path, paint: &Paint) {
        if let Some(hooks) = self.hooks.as_mut()
            && hooks.on_draw_path(path, paint)
        {
            return;
        }
        if !path.is_finite() {
            return;
        }

        let path_bounds = *path.bounds();
        if !path.is_inverse_fill_type() && self.internal_quick_reject(&path_bounds, paint, None) {
            return;
        }
        if path.is_inverse_fill_type() && path_bounds.width() <= 0.0 && path_bounds.height() <= 0.0
        {
            self.internal_draw_paint(paint);
            return;
        }

        let b = if path.is_inverse_fill_type() {
            None
        } else {
            Some(&path_bounds)
        };
        if self.about_to_draw(paint, b, PredrawFlags::NONE) {
            self.top_device_mut().draw_path(path, paint);
        }
    }

    // Port of: src/core/SkCanvas.cpp#L2866-L2874 (chrome/m156)
    fn draw_arc(
        &mut self,
        oval: &Rect,
        start: scalar,
        sweep: scalar,
        use_center: bool,
        paint: &Paint,
    ) {
        if oval.is_empty() || sweep == 0.0 {
            return;
        }
        self.on_draw_arc(oval, start, sweep, use_center, paint);
    }

    // Port of: src/core/SkCanvas.cpp#L2379-L2390 (chrome/m156)
    fn draw_image(
        &mut self,
        image: &Image,
        x: scalar,
        y: scalar,
        sampling: &SamplingOptions,
        paint: Option<&Paint>,
    ) {
        #[allow(clippy::cast_precision_loss)] // mirrors SkIntToScalar
        let (w, h) = (image.width() as scalar, image.height() as scalar);
        self.draw_image_rect(
            image,
            &Rect::from_wh(w, h),
            &Rect::from_xywh(x, y, w, h),
            sampling,
            paint,
            SrcRectConstraint::Fast,
        );
    }

    // Port of: src/core/SkCanvas.cpp#L2392-L2400 (chrome/m156)
    fn draw_image_rect(
        &mut self,
        image: &Image,
        src: &Rect,
        dst: &Rect,
        sampling: &SamplingOptions,
        paint: Option<&Paint>,
        constraint: SrcRectConstraint,
    ) {
        if !fillable(dst) || !fillable(src) {
            return;
        }
        self.on_draw_image_rect2(image, src, dst, sampling, paint, constraint);
    }

    // Port of: src/core/SkCanvas.cpp#L2402-L2406 (chrome/m156)
    fn draw_image_rect_whole(
        &mut self,
        image: &Image,
        dst: &Rect,
        sampling: &SamplingOptions,
        paint: Option<&Paint>,
    ) {
        #[allow(clippy::cast_precision_loss)] // mirrors SkRect::MakeIWH
        let src = Rect::from_wh(image.width() as scalar, image.height() as scalar);
        self.draw_image_rect(image, &src, dst, sampling, paint, SrcRectConstraint::Fast);
    }

    // Port of: src/core/SkCanvas.cpp#L2265-L2363 (chrome/m156)
    //
    // TODO(Phase 3, image filters): a paint with an image filter draws the image through
    // `skif::FilterResult` in Skia; image filters are not ported, so the filter is ignored (as
    // for every other draw call, see `about_to_draw`).
    fn on_draw_image_rect2(
        &mut self,
        image: &Image,
        src: &Rect,
        dst: &Rect,
        sampling: &SamplingOptions,
        paint: Option<&Paint>,
        constraint: SrcRectConstraint,
    ) {
        if let Some(hooks) = self.hooks.as_mut()
            && hooks.on_draw_image_rect2(image, src, dst, sampling, paint, constraint)
        {
            return;
        }
        let real_paint = clean_paint_for_draw_image(paint);
        let real_sampling = clean_sampling_for_constraint(sampling, constraint);

        if self.internal_quick_reject(dst, &real_paint, None) {
            return;
        }

        // (`shouldDrawAsTiledImageRect` is false for the raster device, and so is
        // `useDrawCoverageMaskForMaskFilters`.)
        if self.about_to_draw(&real_paint, Some(dst), PredrawFlags::CHECK_FOR_OVERWRITE) {
            self.top_device_mut().draw_image_rect(
                image,
                Some(src),
                dst,
                &real_sampling,
                &real_paint,
                constraint,
            );
        }
    }

    // Port of: src/core/SkCanvas.cpp#L1794-L1809 (chrome/m156)
    fn draw_image_nine(
        &mut self,
        image: &Image,
        center: &IRect,
        dst: &Rect,
        filter: FilterMode,
        paint: Option<&Paint>,
    ) {
        let xdivs = [center.left, center.right];
        let ydivs = [center.top, center.bottom];

        let lat = Lattice {
            x_divs: &xdivs,
            y_divs: &ydivs,
            rect_types: None,
            bounds: None,
            colors: None,
        };
        self.draw_image_lattice(image, &lat, dst, filter, paint);
    }

    // Port of: src/core/SkCanvas.cpp#L1811-L1832 (chrome/m156)
    fn draw_image_lattice(
        &mut self,
        image: &Image,
        lattice: &Lattice<'_>,
        dst: &Rect,
        filter: FilterMode,
        paint: Option<&Paint>,
    ) {
        if dst.is_empty() {
            return;
        }

        let mut lattice_plus_bounds = lattice.clone();
        if lattice_plus_bounds.bounds.is_none() {
            lattice_plus_bounds.bounds = Some(IRect::from_wh(image.width(), image.height()));
        }

        let lattice_paint = clean_paint_for_lattice(paint);
        if LatticeIter::valid(image.width(), image.height(), &lattice_plus_bounds) {
            self.on_draw_image_lattice2(image, &lattice_plus_bounds, dst, filter, &lattice_paint);
        } else {
            #[allow(clippy::cast_precision_loss)] // mirrors SkRect::MakeIWH
            let src = Rect::from_wh(image.width() as scalar, image.height() as scalar);
            self.draw_image_rect(
                image,
                &src,
                dst,
                &SamplingOptions::from(filter),
                Some(&lattice_paint),
                SrcRectConstraint::Strict,
            );
        }
    }

    // Port of: src/core/SkCanvas.cpp#L2365-L2377 (chrome/m156)
    fn on_draw_image_lattice2(
        &mut self,
        image: &Image,
        lattice: &Lattice<'_>,
        dst: &Rect,
        filter: FilterMode,
        paint: &Paint,
    ) {
        if let Some(hooks) = self.hooks.as_mut()
            && hooks.on_draw_image_lattice2(image, lattice, dst, filter, Some(paint))
        {
            return;
        }
        let real_paint = clean_paint_for_draw_image(Some(paint));

        if self.internal_quick_reject(dst, &real_paint, None) {
            return;
        }

        if self.about_to_draw(&real_paint, Some(dst), PredrawFlags::NONE) {
            self.top_device_mut()
                .draw_image_lattice(image, lattice, dst, filter, &real_paint);
        }
    }
}

/// Returns true if the rect can be "filled": non-empty and finite (`fillable`).
// Port of: src/core/SkCanvas.cpp#L1778-L1782 (chrome/m156)
fn fillable(r: &Rect) -> bool {
    let w = r.width();
    let h = r.height();
    is_finite_all(w, &[h]) && w > 0.0 && h > 0.0
}

// Port of: src/core/SkCanvas.cpp#L1784-L1792 (chrome/m156)
fn clean_paint_for_lattice(paint: Option<&Paint>) -> Paint {
    let mut cleaned = Paint::default();
    if let Some(paint) = paint {
        cleaned = paint.clone();
        cleaned.set_mask_filter(None);
        cleaned.set_anti_alias(false);
    }
    cleaned
}

/// Clean-up the paint to match the drawing semantics for drawImage et al. (skbug.com/40039059).
// Port of: src/core/SkCanvas.cpp#L2226-L2234 (chrome/m156)
fn clean_paint_for_draw_image(paint: Option<&Paint>) -> Paint {
    let mut cleaned = Paint::default();
    if let Some(paint) = paint {
        cleaned = paint.clone();
        cleaned.set_style(Style::Fill);
        cleaned.set_path_effect(None);
    }
    cleaned
}

// Port of: src/core/SkCanvas.cpp#L2252-L2263 (chrome/m156)
fn clean_sampling_for_constraint(
    sampling: &SamplingOptions,
    constraint: SrcRectConstraint,
) -> SamplingOptions {
    if constraint == SrcRectConstraint::Strict {
        if sampling.mipmap != MipmapMode::None {
            return SamplingOptions::from(sampling.filter);
        }
        if sampling.is_aniso() {
            return SamplingOptions::from(FilterMode::Linear);
        }
    }
    *sampling
}

/// Computes the layer's mapping and bounds for a layer with no image filters
/// (`get_layer_mapping_and_bounds` with an empty filter span). `None` if the layer should be
/// skipped.
// Port of: src/core/SkCanvas.cpp#L569-L666 (chrome/m156)
const MIN_DIM_THRESHOLD: i32 = 2048;

fn get_layer_mapping_and_bounds(
    local_to_dst: &M44,
    target_output: &IRect,
    content_bounds: Option<&Rect>,
) -> Option<(Mapping, IRect)> {
    if !local_to_dst.is_finite() || local_to_dst.invert().is_none() {
        return None;
    }
    let dst_to_local = local_to_dst.invert()?;

    // compute_decomposition_center
    let center = {
        let rect = content_bounds.map_or_else(|| Rect::from_irect(target_output), |r| *r);
        let mut center = Point::new(rect.center_x(), rect.center_y());
        if content_bounds.is_none() {
            // Theoretically, the inverse transform could put center's homogeneous coord behind
            // W = 0, but that case is handled automatically in Mapping::decomposeCTM later.
            let mapped = dst_to_local.map(center.x, center.y, 0.0, 1.0);
            center = Point::new(
                crate::floating_point::ieee_float_divide(mapped.x, mapped.w),
                crate::floating_point::ieee_float_divide(mapped.y, mapped.w),
            );
        }
        center
    };

    // Determine initial mapping and a reasonable maximum dimension to prevent layer-to-device
    // transforms with perspective and skew from triggering excessive buffer allocations.
    let mut mapping = Mapping::new();
    let capability = MatrixCapability::Complex; // no filters
    if !mapping.decompose_ctm(local_to_dst, capability, center) {
        return None;
    }

    // Perspective and skew could exceed this since mapping.deviceToLayer(targetOutput) is
    // theoretically unbounded under those conditions. Under a 45 degree rotation, a layer needs
    // to be 2X larger per side of the prior device in order to fully cover it. We use the max of
    // that and 2048 for a reasonable upper limit (this allows small layers under extreme
    // transforms to use more relative resolution than a larger layer).
    let w64 = i64::from(target_output.right) - i64::from(target_output.left);
    let h64 = i64::from(target_output.bottom) - i64::from(target_output.top);
    let max_layer_dim = crate::safe32::pin_to_s32(2 * w64.max(h64)).max(MIN_DIM_THRESHOLD);

    let mut base_layer_bounds = mapping.device_to_layer(target_output);
    if let Some(content_bounds) = content_bounds {
        // For better or for worse, user bounds currently act as a hard clip on the layer's extent
        // (i.e., they implement the CSS filter-effects 'filter region' feature).
        let known_bounds = round_out(&mapping.param_to_layer_rect(content_bounds));
        if let Some(r) = IRect::intersect(&base_layer_bounds, &known_bounds) {
            base_layer_bounds = r;
        } else {
            base_layer_bounds = IRect::new_empty();
        }
    }

    // No filters:
    if base_layer_bounds.is_empty() {
        return None;
    }
    let mut layer_bounds = base_layer_bounds;

    if layer_bounds.width() > max_layer_dim || layer_bounds.height() > max_layer_dim {
        let new_layer_bounds = IRect::from_wh(
            layer_bounds.width().min(max_layer_dim),
            layer_bounds.height().min(max_layer_dim),
        );
        let adjust = M44::rect_to_rect(
            Rect::from_irect(layer_bounds),
            Rect::from_irect(new_layer_bounds),
        );
        if !mapping.adjust_layer_space(&adjust) {
            return None;
        }
        layer_bounds = new_layer_bounds;
    }

    let _ = ROUND_EPSILON;
    Some((mapping, layer_bounds))
}

/// A canvas (`SkCanvas`). See the module documentation.
#[doc(alias = "SkCanvas")]
#[derive(Debug)]
pub struct Canvas {
    state: RefCell<CanvasState>,
}

/// Guard over the pixels of a canvas's root device (`peekPixels`).
pub struct PeekedPixels<'a>(Ref<'a, dyn Device + 'static>);

impl PeekedPixels<'_> {
    /// The pixels, for reading.
    ///
    /// # Panics
    /// Never: the guard is only made for a device with pixels.
    #[must_use]
    pub fn pixmap(&self) -> Pixmap<'_> {
        self.0.peek_pixels().expect("checked on creation")
    }
}

impl std::fmt::Debug for PeekedPixels<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PeekedPixels").finish_non_exhaustive()
    }
}

/// The pixels of the top layer and where they are (`accessTopLayerPixels`).
pub struct TopLayerPixels<'a> {
    device: RefMut<'a, dyn Device + 'static>,
    /// The origin of the layer in the base layer's coordinates.
    pub origin: Option<IPoint>,
}

impl TopLayerPixels<'_> {
    /// The pixels, for writing.
    ///
    /// # Panics
    /// Never: the guard is only made for a device with pixels.
    pub fn pixmap(&mut self) -> Pixmap<'_> {
        self.device.access_pixels().expect("checked on creation")
    }
}

impl std::fmt::Debug for TopLayerPixels<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TopLayerPixels")
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}

impl Canvas {
    /// A canvas drawing into `device` (`SkCanvas(sk_sp<SkDevice>)`).
    // Port of: src/core/SkCanvas.cpp#L341-L345 (chrome/m156)
    #[must_use]
    pub fn from_device(device: Box<dyn Device>) -> Canvas {
        let props = *device.state().surface_props();
        Canvas {
            state: RefCell::new(CanvasState::init(device, props)),
        }
    }

    /// A canvas of `size` that draws nothing, with default props (`SkCanvas(width, height)`).
    /// `None` if a dimension is negative.
    // Port of: src/core/SkCanvas.cpp#L328-L333 (chrome/m156)
    #[must_use]
    pub fn new_no_pixels(size: impl Into<ISize>, props: Option<&SurfaceProps>) -> Option<Canvas> {
        let size = size.into();
        if size.width < 0 || size.height < 0 {
            return None;
        }
        let props = props.copied().unwrap_or_default();
        let device = NoPixelsDevice::new(
            &IRect::from_wh(size.width.max(0), size.height.max(0)),
            props,
        );
        Some(Canvas {
            state: RefCell::new(CanvasState::init(Box::new(device), props)),
        })
    }

    /// A canvas with an empty no-pixels device (`SkCanvas()`).
    #[must_use]
    pub fn new_empty() -> Canvas {
        let props = SurfaceProps::default();
        let device = NoPixelsDevice::new(&IRect::new_empty(), props);
        Canvas {
            state: RefCell::new(CanvasState::init(Box::new(device), props)),
        }
    }

    /// A canvas covering `bounds` (in global coordinates, so the origin can be anything) that
    /// draws nothing (`SkCanvas(const SkIRect&)`, as `SkNoDrawCanvas` uses it). An empty `bounds`
    /// is the empty rectangle at the origin.
    // Port of: src/core/SkCanvas.cpp#L335-L339 (chrome/m156)
    #[must_use]
    pub fn new_no_pixels_irect(bounds: &IRect, props: Option<&SurfaceProps>) -> Canvas {
        let props = props.copied().unwrap_or_default();
        let r = if bounds.is_empty() {
            IRect::new_empty()
        } else {
            *bounds
        };
        let device = NoPixelsDevice::new(&r, props);
        Canvas {
            state: RefCell::new(CanvasState::init(Box::new(device), props)),
        }
    }

    /// Resets a canvas made for recording a picture (one with a no-pixels root device) to a
    /// fresh state covering `bounds` (`resetForNextPicture`, which `SkNoDrawCanvas::resetCanvas`
    /// exposes).
    // Port of: src/core/SkCanvas.cpp#L287-L302 (chrome/m156)
    #[doc(alias = "resetForNextPicture")]
    pub fn reset_for_next_picture(&self, bounds: &IRect) {
        self.restore_to_count(1);

        let mut s = self.state.borrow_mut();
        // We're peering through a lot of structure here.  Only at this scope do we know that the
        // device is a no-pixels device.
        debug_assert!(s.devices[0].is_no_pixels_device());
        if !s.devices[0].reset_for_next_picture(bounds) {
            let props = *s.devices[0].state().surface_props();
            let color_space = s.devices[0].state().image_info().color_space();
            s.devices[0] = Box::new(NoPixelsDevice::new_with_color_space(
                bounds,
                props,
                color_space,
            ));
        }

        // fMCRec->reset(fRootDevice.get())
        s.mc_stack.truncate(1);
        let rec = s.mc_rec_mut();
        rec.device = 0;
        rec.matrix = M44::new_identity();
        rec.deferred_save_count = 0;
        rec.layer = None;
        s.quick_reject_bounds = s.compute_device_clip_bounds(true);
    }

    /// Installs (or removes) the hooks standing for a subclass of `SkCanvas`
    /// (see [`CanvasHooks`]).
    pub fn set_hooks(&self, hooks: Option<Box<dyn CanvasHooks>>) {
        self.state.borrow_mut().hooks = hooks;
    }

    /// Ties the canvas to the surface that owns it (`setSurfaceBase`).
    pub fn set_surface_base(&self, surface: Option<Rc<SurfaceBase>>) {
        self.state.borrow_mut().surface = surface;
    }

    /// Restores every save, discarding (not drawing) the layers still open, as `~SkCanvas` does.
    // Port of: src/core/SkCanvas.cpp#L347-L363 (chrome/m156)
    pub fn abandon_layers(&self) {
        let mut s = self.state.borrow_mut();
        // Mark all pending layers to be discarded during restore (rather than drawn)
        for rec in &mut s.mc_stack {
            if let Some(layer) = rec.layer.as_mut() {
                layer.discard = true;
            }
        }
        s.restore_to_count(1);
    }

    /// Takes the root device's bitmap out of the canvas, leaving it without pixels. Used by
    /// owners that wrapped caller pixels.
    #[must_use]
    pub fn take_root_bitmap(&self) -> Option<Bitmap> {
        let mut s = self.state.borrow_mut();
        s.devices[0].bitmap_mut().map(std::mem::take)
    }

    /// The root device's bitmap, if it is a raster device.
    pub fn with_root_bitmap<R>(&self, f: impl FnOnce(&mut Bitmap) -> R) -> Option<R> {
        let mut s = self.state.borrow_mut();
        s.devices[0].bitmap_mut().map(f)
    }

    /// Swaps the root device's bitmap for `bm` (`replaceBitmapBackendForRasterSurface`).
    pub fn replace_root_bitmap(&self, bm: Bitmap) {
        let mut s = self.state.borrow_mut();
        if let Some(b) = s.devices[0].bitmap_mut() {
            *b = bm;
        }
    }

    /// The image info of the canvas (`imageInfo`).
    #[doc(alias = "imageInfo")]
    #[must_use]
    pub fn image_info(&self) -> ImageInfo {
        self.state
            .borrow()
            .root_device()
            .state()
            .image_info()
            .clone()
    }

    /// The surface properties of the canvas (`getProps`).
    #[doc(alias = "getProps")]
    #[must_use]
    pub fn props(&self) -> Option<SurfaceProps> {
        Some(self.state.borrow().props)
    }

    /// The properties at the base of the layer stack (`getBaseProps`).
    #[doc(alias = "getBaseProps")]
    #[must_use]
    pub fn base_props(&self) -> SurfaceProps {
        self.state.borrow().props
    }

    /// The properties at the top of the layer stack (`getTopProps`).
    #[doc(alias = "getTopProps")]
    #[must_use]
    pub fn top_props(&self) -> SurfaceProps {
        *self.state.borrow().top_device().state().surface_props()
    }

    /// The size of the base layer (`getBaseLayerSize`).
    #[doc(alias = "getBaseLayerSize")]
    #[must_use]
    pub fn base_layer_size(&self) -> ISize {
        self.state.borrow().base_layer_size()
    }

    /// The pixels of the root device, if it has any (`peekPixels`).
    #[doc(alias = "peekPixels")]
    #[must_use]
    pub fn peek_pixels(&self) -> Option<PeekedPixels<'_>> {
        let r = Ref::map(self.state.borrow(), |s| s.root_device());
        r.peek_pixels()?;
        Some(PeekedPixels(r))
    }

    /// The pixels of the top layer and their origin (`accessTopLayerPixels`).
    #[doc(alias = "accessTopLayerPixels")]
    #[must_use]
    pub fn access_top_layer_pixels(&self) -> Option<TopLayerPixels<'_>> {
        let mut device = RefMut::map(self.state.borrow_mut(), |s| s.top_device_mut());
        device.access_pixels()?;
        // If the top level device isn't axis aligned, the pixels cannot be exposed with an
        // origin.
        if !device.state().is_pixel_aligned_to_global() {
            return None;
        }
        let origin = Some(device.state().origin());
        Some(TopLayerPixels { device, origin })
    }

    /// Copies pixels from the canvas's root device into `dst_pixels` (`readPixels`).
    #[doc(alias = "readPixels")]
    pub fn read_pixels(
        &self,
        dst_info: &ImageInfo,
        dst_pixels: &mut [u8],
        dst_row_bytes: usize,
        src: impl Into<IPoint>,
    ) -> bool {
        let Some(mut pm) = Pixmap::new(dst_info, dst_pixels, dst_row_bytes) else {
            return false;
        };
        let IPoint { x, y } = src.into();
        self.state.borrow_mut().read_pixels(&mut pm, x, y)
    }

    /// [`read_pixels`](Self::read_pixels) into a pixmap.
    #[doc(alias = "readPixels")]
    pub fn read_pixels_to_pixmap(&self, pixmap: &mut Pixmap<'_>, src: impl Into<IPoint>) -> bool {
        let IPoint { x, y } = src.into();
        self.state.borrow_mut().read_pixels(pixmap, x, y)
    }

    /// [`read_pixels`](Self::read_pixels) into a bitmap.
    #[doc(alias = "readPixels")]
    pub fn read_pixels_to_bitmap(&self, bitmap: &mut Bitmap, src: impl Into<IPoint>) -> bool {
        let IPoint { x, y } = src.into();
        match bitmap.peek_pixels_mut() {
            Some(mut pm) => self.state.borrow_mut().read_pixels(&mut pm, x, y),
            None => false,
        }
    }

    /// Copies pixels into the canvas's root device (`writePixels`).
    #[doc(alias = "writePixels")]
    pub fn write_pixels(
        &self,
        info: &ImageInfo,
        pixels: &[u8],
        row_bytes: usize,
        offset: impl Into<IPoint>,
    ) -> bool {
        let Some(src) = Pixmap::new_readonly(info, pixels, row_bytes) else {
            return false;
        };
        let IPoint { x, y } = offset.into();
        self.state.borrow_mut().write_pixels(&src, x, y)
    }

    /// [`write_pixels`](Self::write_pixels) from a bitmap.
    #[doc(alias = "writePixels")]
    pub fn write_pixels_from_bitmap(&self, bitmap: &Bitmap, offset: impl Into<IPoint>) -> bool {
        let IPoint { x, y } = offset.into();
        match bitmap.peek_pixels() {
            Some(pm) => self.state.borrow_mut().write_pixels(&pm, x, y),
            None => false,
        }
    }

    /// Saves the matrix and clip; returns the previous save count (`save`).
    pub fn save(&self) -> usize {
        usize::try_from(self.state.borrow_mut().save()).unwrap_or(0)
    }

    /// Saves the matrix and clip and allocates a layer; returns the previous save count
    /// (`saveLayer`).
    #[doc(alias = "saveLayer")]
    pub fn save_layer(&self, layer_rec: &SaveLayerRec<'_>) -> usize {
        usize::try_from(self.state.borrow_mut().save_layer(layer_rec)).unwrap_or(0)
    }

    /// `saveLayerAlphaf`.
    // Port of: src/core/SkCanvas.cpp#L1092-L1100 (chrome/m156)
    #[doc(alias = "saveLayerAlphaf")]
    pub fn save_layer_alpha_f(&self, bounds: impl Into<Option<Rect>>, alpha: f32) -> usize {
        let bounds = bounds.into();
        let mut rec = SaveLayerRec::default();
        if let Some(b) = bounds.as_ref() {
            rec = rec.bounds(b);
        }
        if alpha >= 1.0 {
            self.save_layer(&rec)
        } else {
            let mut tmp_paint = Paint::default();
            tmp_paint.set_alpha_f(alpha);
            self.save_layer(&rec.paint(&tmp_paint))
        }
    }

    /// `saveLayerAlpha` (an 8-bit alpha).
    #[doc(alias = "saveLayerAlpha")]
    pub fn save_layer_alpha(&self, bounds: impl Into<Option<Rect>>, alpha: u8) -> usize {
        self.save_layer_alpha_f(bounds, f32::from(alpha) * (1.0 / 255.0))
    }

    /// Removes changes to the matrix and clip since the last save (`restore`).
    pub fn restore(&self) -> &Self {
        self.state.borrow_mut().restore();
        self
    }

    /// The number of saved states, including the initial one (`getSaveCount`).
    #[doc(alias = "getSaveCount")]
    #[must_use]
    pub fn save_count(&self) -> usize {
        usize::try_from(self.state.borrow().save_count).unwrap_or(0)
    }

    /// Restores to the state when [`save`](Self::save) returned `save_count` (`restoreToCount`).
    #[doc(alias = "restoreToCount")]
    pub fn restore_to_count(&self, save_count: usize) -> &Self {
        let count = i32::try_from(save_count).unwrap_or(i32::MAX);
        self.state.borrow_mut().restore_to_count(count);
        self
    }

    /// Translates the matrix (`translate`).
    pub fn translate(&self, d: impl Into<Vector>) -> &Self {
        let d = d.into();
        self.state.borrow_mut().translate(d.x, d.y);
        self
    }

    /// Scales the matrix (`scale`).
    pub fn scale(&self, (sx, sy): (scalar, scalar)) -> &Self {
        self.state.borrow_mut().scale(sx, sy);
        self
    }

    /// Rotates the matrix by `degrees` around `p` (the origin if `None`) (`rotate`).
    pub fn rotate(&self, degrees: scalar, p: Option<Point>) -> &Self {
        let mut m = Matrix::new_identity();
        m.set_rotate(degrees, p);
        self.state.borrow_mut().concat(&m);
        self
    }

    /// Skews the matrix (`skew`).
    pub fn skew(&self, (sx, sy): (scalar, scalar)) -> &Self {
        let mut m = Matrix::new_identity();
        m.set_skew((sx, sy), None);
        self.state.borrow_mut().concat(&m);
        self
    }

    /// Replaces the matrix with the product of the matrix and `matrix` (`concat`).
    pub fn concat(&self, matrix: &Matrix) -> &Self {
        self.state.borrow_mut().concat(matrix);
        self
    }

    /// `concat(const SkM44&)`.
    #[doc(alias = "concat")]
    pub fn concat_44(&self, m: &M44) -> &Self {
        self.state.borrow_mut().concat44(m);
        self
    }

    /// Replaces the matrix (`setMatrix`).
    #[doc(alias = "setMatrix")]
    pub fn set_matrix(&self, matrix: &M44) -> &Self {
        self.state.borrow_mut().set_matrix(matrix);
        self
    }

    /// Sets the matrix to the identity (`resetMatrix`).
    #[doc(alias = "resetMatrix")]
    pub fn reset_matrix(&self) -> &Self {
        self.state.borrow_mut().set_matrix(&M44::new_identity());
        self
    }

    /// Replaces the clip with its intersection (or difference) with `rect` (`clipRect`).
    #[doc(alias = "clipRect")]
    pub fn clip_rect(
        &self,
        rect: impl AsRef<Rect>,
        op: impl Into<Option<ClipOp>>,
        do_anti_alias: impl Into<Option<bool>>,
    ) -> &Self {
        self.state.borrow_mut().clip_rect(
            rect.as_ref(),
            op.into().unwrap_or(ClipOp::Intersect),
            do_anti_alias.into().unwrap_or(false),
        );
        self
    }

    /// [`clip_rect`](Self::clip_rect) with an integer rectangle.
    pub fn clip_irect(&self, irect: impl AsRef<IRect>, op: impl Into<Option<ClipOp>>) -> &Self {
        self.clip_rect(Rect::from_irect(irect.as_ref()), op, false)
    }

    /// `clipRRect`.
    #[doc(alias = "clipRRect")]
    pub fn clip_rrect(
        &self,
        rrect: impl AsRef<RRect>,
        op: impl Into<Option<ClipOp>>,
        do_anti_alias: impl Into<Option<bool>>,
    ) -> &Self {
        self.state.borrow_mut().clip_rrect(
            rrect.as_ref(),
            op.into().unwrap_or(ClipOp::Intersect),
            do_anti_alias.into().unwrap_or(false),
        );
        self
    }

    /// `clipPath`.
    #[doc(alias = "clipPath")]
    pub fn clip_path(
        &self,
        path: &Path,
        op: impl Into<Option<ClipOp>>,
        do_anti_alias: impl Into<Option<bool>>,
    ) -> &Self {
        self.state.borrow_mut().clip_path(
            path,
            op.into().unwrap_or(ClipOp::Intersect),
            do_anti_alias.into().unwrap_or(false),
        );
        self
    }

    /// `clipShader`.
    #[doc(alias = "clipShader")]
    pub fn clip_shader(&self, shader: impl Into<Shader>, op: impl Into<Option<ClipOp>>) -> &Self {
        self.state
            .borrow_mut()
            .clip_shader(&shader.into(), op.into().unwrap_or(ClipOp::Intersect));
        self
    }

    /// `clipRegion`.
    #[doc(alias = "clipRegion")]
    pub fn clip_region(&self, device_rgn: &Region, op: impl Into<Option<ClipOp>>) -> &Self {
        self.state
            .borrow_mut()
            .clip_region(device_rgn, op.into().unwrap_or(ClipOp::Intersect));
        self
    }

    /// Resets the clip to the device bounds, respecting a clip restriction
    /// (`internal_private_resetClip`).
    #[doc(alias = "internal_private_resetClip")]
    pub fn reset_clip(&self) -> &Self {
        self.state.borrow_mut().reset_clip();
        self
    }

    /// Restricts all drawing to `rect` until the canvas is restored past this save count
    /// (`androidFramework_setDeviceClipRestriction`).
    #[doc(alias = "androidFramework_setDeviceClipRestriction")]
    pub fn android_framework_set_device_clip_restriction(&self, rect: &IRect) {
        self.state
            .borrow_mut()
            .android_framework_set_device_clip_restriction(rect);
    }

    /// The clip bounds in local coordinates, or `None` if empty (`getLocalClipBounds`).
    #[doc(alias = "getLocalClipBounds")]
    #[must_use]
    pub fn local_clip_bounds(&self) -> Option<Rect> {
        let r = self.state.borrow().local_clip_bounds();
        if r.is_empty() { None } else { Some(r) }
    }

    /// The clip bounds in device coordinates, or `None` if empty (`getDeviceClipBounds`).
    #[doc(alias = "getDeviceClipBounds")]
    #[must_use]
    pub fn device_clip_bounds(&self) -> Option<IRect> {
        let r = self.state.borrow().device_clip_bounds();
        if r.is_empty() { None } else { Some(r) }
    }

    /// True if the clip is empty (`isClipEmpty`).
    #[doc(alias = "isClipEmpty")]
    #[must_use]
    pub fn is_clip_empty(&self) -> bool {
        self.state.borrow().is_clip_empty()
    }

    /// True if the clip is a rectangle (`isClipRect`).
    #[doc(alias = "isClipRect")]
    #[must_use]
    pub fn is_clip_rect(&self) -> bool {
        self.state.borrow().is_clip_rect()
    }

    /// The total matrix as a 4x4 (`getLocalToDevice`).
    #[doc(alias = "getLocalToDevice")]
    #[must_use]
    pub fn local_to_device(&self) -> M44 {
        self.state.borrow().mc_rec().matrix
    }

    /// The total matrix as a 3x3 (`getLocalToDevice().asM33()`).
    #[must_use]
    pub fn local_to_device_as_3x3(&self) -> Matrix {
        self.state.borrow().total_matrix()
    }

    /// The total matrix (`getTotalMatrix`).
    #[doc(alias = "getTotalMatrix")]
    #[must_use]
    pub fn total_matrix(&self) -> Matrix {
        self.state.borrow().total_matrix()
    }

    /// True if drawing `rect` would draw nothing (`quickReject(const SkRect&)`).
    #[doc(alias = "quickReject")]
    #[must_use]
    pub fn quick_reject_rect(&self, rect: impl AsRef<Rect>) -> bool {
        self.state.borrow().quick_reject(rect.as_ref())
    }

    /// True if drawing `path` would draw nothing (`quickReject(const SkPath&)`).
    #[doc(alias = "quickReject")]
    #[must_use]
    pub fn quick_reject_path(&self, path: &Path) -> bool {
        self.state.borrow().quick_reject_path(path)
    }

    /// Fills the clip with `color` and `mode` (`drawColor`).
    #[doc(alias = "drawColor")]
    pub fn draw_color(
        &self,
        color: impl Into<Color4f>,
        mode: impl Into<Option<BlendMode>>,
    ) -> &Self {
        let mut paint = Paint::default();
        paint.set_color4f(color.into(), None);
        paint.set_blend_mode(mode.into().unwrap_or(BlendMode::SrcOver));
        self.draw_paint(&paint)
    }

    /// Fills the clip with `color`, replacing what is there (`clear`).
    // Port of: include/core/SkCanvas.h#L1263-L1265 (chrome/m156)
    pub fn clear(&self, color: impl Into<Color4f>) -> &Self {
        self.draw_color(color, BlendMode::Src)
    }

    /// Discards the contents of the surface, if any (`discard`).
    // Port of: src/core/SkCanvas.cpp#L1916-L1920 (chrome/m156)
    pub fn discard(&self) -> &Self {
        if let Some(surface) = &self.state.borrow().surface {
            let _ = surface.about_to_draw(ContentChangeMode::Discard);
        }
        self
    }

    /// Fills the clip with `paint` (`drawPaint`).
    #[doc(alias = "drawPaint")]
    pub fn draw_paint(&self, paint: &Paint) -> &Self {
        self.state.borrow_mut().on_draw_paint(paint);
        self
    }

    /// Draws points, lines or a polygon (`drawPoints`).
    #[doc(alias = "drawPoints")]
    pub fn draw_points(&self, mode: PointMode, pts: &[Point], paint: &Paint) -> &Self {
        self.state.borrow_mut().draw_points(mode, pts, paint);
        self
    }

    /// Draws a point (`drawPoint`).
    #[doc(alias = "drawPoint")]
    pub fn draw_point(&self, p: impl Into<Point>, paint: &Paint) -> &Self {
        self.draw_points(PointMode::Points, &[p.into()], paint)
    }

    /// Draws a line (`drawLine`).
    #[doc(alias = "drawLine")]
    pub fn draw_line(&self, p1: impl Into<Point>, p2: impl Into<Point>, paint: &Paint) -> &Self {
        self.draw_points(PointMode::Lines, &[p1.into(), p2.into()], paint)
    }

    /// Draws a rectangle (`drawRect`).
    #[doc(alias = "drawRect")]
    pub fn draw_rect(&self, rect: impl AsRef<Rect>, paint: &Paint) -> &Self {
        self.state.borrow_mut().draw_rect(rect.as_ref(), paint);
        self
    }

    /// Draws an integer rectangle (`drawIRect`).
    #[doc(alias = "drawIRect")]
    pub fn draw_irect(&self, rect: impl AsRef<IRect>, paint: &Paint) -> &Self {
        self.draw_rect(Rect::from_irect(rect.as_ref()), paint)
    }

    /// Draws a region (`drawRegion`).
    #[doc(alias = "drawRegion")]
    pub fn draw_region(&self, region: &Region, paint: &Paint) -> &Self {
        self.state.borrow_mut().draw_region(region, paint);
        self
    }

    /// Draws an oval (`drawOval`).
    #[doc(alias = "drawOval")]
    pub fn draw_oval(&self, oval: impl AsRef<Rect>, paint: &Paint) -> &Self {
        self.state.borrow_mut().draw_oval(oval.as_ref(), paint);
        self
    }

    /// Draws a round rectangle (`drawRRect`).
    #[doc(alias = "drawRRect")]
    pub fn draw_rrect(&self, rrect: impl AsRef<RRect>, paint: &Paint) -> &Self {
        self.state.borrow_mut().draw_rrect(rrect.as_ref(), paint);
        self
    }

    /// Draws the area between two round rectangles (`drawDRRect`).
    #[doc(alias = "drawDRRect")]
    pub fn draw_drrect(
        &self,
        outer: impl AsRef<RRect>,
        inner: impl AsRef<RRect>,
        paint: &Paint,
    ) -> &Self {
        self.state
            .borrow_mut()
            .draw_drrect(outer.as_ref(), inner.as_ref(), paint);
        self
    }

    /// Draws a circle (`drawCircle`).
    // Port of: src/core/SkCanvas.cpp#L2845-L2853 (chrome/m156)
    #[doc(alias = "drawCircle")]
    pub fn draw_circle(&self, center: impl Into<Point>, radius: scalar, paint: &Paint) -> &Self {
        let center = center.into();
        let radius = if radius < 0.0 { 0.0 } else { radius };
        let r = Rect::new(
            center.x - radius,
            center.y - radius,
            center.x + radius,
            center.y + radius,
        );
        self.draw_oval(r, paint)
    }

    /// Draws an arc (`drawArc`).
    #[doc(alias = "drawArc")]
    pub fn draw_arc(
        &self,
        oval: impl AsRef<Rect>,
        start_angle: scalar,
        sweep_angle: scalar,
        use_center: bool,
        paint: &Paint,
    ) -> &Self {
        self.state.borrow_mut().draw_arc(
            oval.as_ref(),
            start_angle,
            sweep_angle,
            use_center,
            paint,
        );
        self
    }

    /// Draws an [`Arc`] (`drawArc(const SkArc&, ...)`).
    pub fn draw_arc_2(&self, arc: &Arc, paint: &Paint) -> &Self {
        self.draw_arc(
            arc.oval,
            arc.start_angle,
            arc.sweep_angle,
            arc.is_wedge(),
            paint,
        )
    }

    /// Draws a round rectangle with radii `rx`, `ry` (`drawRoundRect`).
    // Port of: src/core/SkCanvas.cpp#L2855-L2864 (chrome/m156)
    #[doc(alias = "drawRoundRect")]
    pub fn draw_round_rect(
        &self,
        rect: impl AsRef<Rect>,
        rx: scalar,
        ry: scalar,
        paint: &Paint,
    ) -> &Self {
        if rx > 0.0 && ry > 0.0 {
            let rr = RRect::new_rect_xy(rect, rx, ry);
            self.draw_rrect(rr, paint)
        } else {
            self.draw_rect(rect, paint)
        }
    }

    /// Draws `vertices` (`drawVertices`). `mode` combines the vertex colors, if any, with the
    /// paint's shader if it has one, or the opaque paint color if not; it is ignored if there
    /// are no colors. The paint's style, mask filter and path effect are ignored.
    #[doc(alias = "drawVertices")]
    pub fn draw_vertices(&self, vertices: &Vertices, mode: BlendMode, paint: &Paint) -> &Self {
        self.state.borrow_mut().draw_vertices(vertices, mode, paint);
        self
    }

    /// Draws a Coons patch: the interpolation of four cubics with shared corners, associating a
    /// color, and optionally a texture [`Point`], with each corner (`drawPatch`).
    ///
    /// `cubics` starts at the top-left corner, in clockwise order, sharing every fourth point.
    /// `colors` are in top-left, top-right, bottom-right, bottom-left order. If the paint has a
    /// shader, `tex_coords` maps it as a texture to the corners in the same order (if `None`,
    /// the shader is mapped using the positions derived from `cubics`). `mode` is ignored if
    /// `colors` is `None`; otherwise it combines them with the shader or the opaque paint
    /// color.
    #[doc(alias = "drawPatch")]
    pub fn draw_patch<'a>(
        &self,
        cubics: &[Point; patch_utils::NUM_CTRL_PTS],
        colors: impl Into<Option<&'a [Color; patch_utils::NUM_CORNERS]>>,
        tex_coords: Option<&[Point; patch_utils::NUM_CORNERS]>,
        mode: BlendMode,
        paint: &Paint,
    ) -> &Self {
        self.state
            .borrow_mut()
            .on_draw_patch(cubics, colors.into(), tex_coords, mode, paint);
        self
    }

    /// Draws the `tex` rectangles of an atlas, each mapped by the matching `xform` and, if
    /// `colors` is not empty, blended with its color using `mode` (`drawAtlas`). `atlas_shader`
    /// is the atlas as a shader (Skia's `atlas->makeShader(sampling)`); [`Canvas::draw_atlas`]
    /// makes it from an [`Image`].
    ///
    /// skia-rust: this is an extra entry point beside `skia-safe`'s `draw_atlas`, for atlases
    /// given as a shader (the color-shader tests use it). `cull_rect` are the bounds of the
    /// transformed sprites (may be `None`), and `paint` supplies the color filter, alpha,
    /// blender and so on (may be `None`).
    #[doc(alias = "drawAtlas")]
    #[allow(clippy::too_many_arguments)] // mirrors drawAtlas
    pub fn draw_atlas_with_shader<'a>(
        &self,
        atlas_shader: &Shader,
        xform: &[RSXform],
        tex: &[Rect],
        colors: impl Into<Option<&'a [Color]>>,
        mode: BlendMode,
        cull_rect: impl Into<Option<Rect>>,
        paint: impl Into<Option<&'a Paint>>,
    ) {
        let colors = colors.into().unwrap_or(&[]);
        let cull_rect = cull_rect.into();
        self.state.borrow_mut().draw_atlas_with_shader(
            Some(atlas_shader),
            xform,
            tex,
            colors,
            mode,
            cull_rect.as_ref(),
            paint.into(),
        );
    }

    /// Draws the `tex` rectangles of the atlas `atlas`, each mapped by the matching `xform` and,
    /// if `colors` is not empty, blended with its color using `mode` (`drawAtlas`). The atlas is
    /// sampled as `atlas->makeShader(sampling)` makes it (clamped tiles, no local matrix).
    /// `cull_rect` are the bounds of the transformed sprites (may be `None`), and `paint`
    /// supplies the color filter, alpha, blender and so on (may be `None`).
    #[doc(alias = "drawAtlas")]
    #[allow(clippy::too_many_arguments)] // mirrors drawAtlas
    pub fn draw_atlas<'a>(
        &self,
        atlas: &Image,
        xform: &[RSXform],
        tex: &[Rect],
        colors: impl Into<Option<&'a [Color]>>,
        mode: BlendMode,
        sampling: impl Into<SamplingOptions>,
        cull_rect: impl Into<Option<Rect>>,
        paint: impl Into<Option<&'a Paint>>,
    ) {
        let colors = colors.into().unwrap_or(&[]);
        let cull_rect = cull_rect.into();
        // `SkCanvas::drawAtlas` builds the shader in `onDrawAtlas2`, after the sprite count check,
        // so an empty draw never makes one.
        let count = xform.len().min(tex.len());
        let count = if colors.is_empty() {
            count
        } else {
            count.min(colors.len())
        };
        if count == 0 {
            return;
        }
        let atlas_shader = atlas.to_shader((TileMode::Clamp, TileMode::Clamp), sampling, None);
        self.state.borrow_mut().draw_atlas_with_shader(
            atlas_shader.as_ref(),
            xform,
            tex,
            colors,
            mode,
            cull_rect.as_ref(),
            paint.into(),
        );
    }

    /// Draws the text `text` in `encoding`, starting at `origin`, with `font` and `paint`
    /// (`drawSimpleText`). Empty text draws nothing.
    // Port of: src/core/SkCanvas.cpp#L2501-L2514 (chrome/m156)
    #[doc(alias = "drawSimpleText")]
    pub fn draw_simple_text(
        &self,
        text: impl AsRef<[u8]>,
        encoding: TextEncoding,
        origin: impl Into<Point>,
        font: &Font,
        paint: &Paint,
    ) -> &Self {
        let text = text.as_ref();
        if !text.is_empty() {
            let mut builder = GlyphRunBuilder::new();
            let list = builder.text_to_glyph_run_list(font, paint, text, origin.into(), encoding);
            if !list.is_empty() {
                self.state.borrow_mut().draw_glyph_run_list(&list, paint);
            }
        }
        self
    }

    /// Draws the UTF-8 string `text`, starting at `origin`, with `font` and `paint` (`drawString`
    /// in skia-safe's `draw_str`). Typeface fallback is not done.
    // Port of: src/core/SkCanvas.cpp#L2501-L2514 (chrome/m156), the kUTF8 call of drawString
    #[doc(alias = "drawString")]
    #[doc(alias = "drawSimpleText")]
    pub fn draw_str(
        &self,
        text: impl AsRef<str>,
        origin: impl Into<Point>,
        font: &Font,
        paint: &Paint,
    ) -> &Self {
        self.draw_simple_text(
            text.as_ref().as_bytes(),
            TextEncoding::UTF8,
            origin,
            font,
            paint,
        )
    }

    /// Draws `glyphs` at `positions` (relative to `origin`), with the UTF-8 `utf8_text` and the
    /// `clusters` that map glyphs to it (`drawGlyphs`).
    ///
    /// # Panics
    ///
    /// If `positions` or `clusters` does not have one entry per glyph.
    // Port of: src/core/SkCanvas.cpp#L2516-L2533 (chrome/m156)
    #[doc(alias = "drawGlyphs")]
    #[allow(clippy::too_many_arguments)] // mirrors SkCanvas::drawGlyphs, which takes as many
    pub fn draw_glyphs_utf8(
        &self,
        glyphs: &[GlyphId],
        positions: &[Point],
        clusters: &[u32],
        utf8_text: impl AsRef<str>,
        origin: impl Into<Point>,
        font: &Font,
        paint: &Paint,
    ) {
        if glyphs.is_empty() {
            return;
        }
        assert_eq!(positions.len(), glyphs.len());
        assert_eq!(clusters.len(), glyphs.len());
        let run = GlyphRun::new(
            font.clone(),
            positions.to_vec(),
            glyphs.to_vec(),
            utf8_text.as_ref().as_bytes().to_vec(),
            clusters.to_vec(),
            Vec::new(),
        );
        self.draw_glyph_run(run, origin.into(), paint);
    }

    /// Draws `glyphs` at `positions`, either points or `RSXform`s, relative to `origin` (the two
    /// `drawGlyphs` and `drawGlyphsRSXform` overloads).
    ///
    /// # Panics
    ///
    /// If the positions do not have one entry per glyph.
    // Port of: src/core/SkCanvas.cpp#L2535-L2552 (chrome/m156), and #L2554-L2572 for RSXforms
    #[doc(alias = "drawGlyphs")]
    #[doc(alias = "drawGlyphsRSXform")]
    pub fn draw_glyphs_at<'a>(
        &self,
        glyphs: &[GlyphId],
        positions: impl Into<GlyphPositions<'a>>,
        origin: impl Into<Point>,
        font: &Font,
        paint: &Paint,
    ) {
        let count = glyphs.len();
        if count == 0 {
            return;
        }
        let run = match positions.into() {
            GlyphPositions::Points(points) => {
                assert_eq!(points.len(), count);
                GlyphRun::new(
                    font.clone(),
                    points.to_vec(),
                    glyphs.to_vec(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                )
            }
            GlyphPositions::RSXforms(xforms) => {
                assert_eq!(xforms.len(), count);
                let (positions, scaled_rotations) = GlyphRunBuilder::convert_rsxform(xforms);
                GlyphRun::new(
                    font.clone(),
                    positions,
                    glyphs.to_vec(),
                    Vec::new(),
                    Vec::new(),
                    scaled_rotations,
                )
            }
        };
        self.draw_glyph_run(run, origin.into(), paint);
    }

    /// Draws the text blob `blob` with its origin at `origin`, painted with `paint`
    /// (`drawTextBlob`).
    // Port of: src/core/SkCanvas.cpp#L2574-L2594 (chrome/m156), and onDrawTextBlob#L2436-L2441
    #[doc(alias = "drawTextBlob")]
    pub fn draw_text_blob(
        &self,
        blob: &TextBlob,
        origin: impl Into<Point>,
        paint: &Paint,
    ) -> &Self {
        let origin = origin.into();
        if !blob.bounds().with_offset(origin).is_finite() {
            return self;
        }
        // Overflow if more than 2^21 glyphs, stopping a buffer overflow later in the stack.
        // See chromium:1080481.
        let max_glyph_count: usize = 1 << 21;
        let total_glyph_count: usize = blob.iter().map(|run| run.glyph_count()).sum();
        if total_glyph_count > max_glyph_count {
            return self;
        }
        self.state
            .borrow_mut()
            .draw_text_blob(blob, origin.x, origin.y, paint);
        self
    }

    /// Draws a slug (`drawSlug`). A null slug draws nothing; a [`Slug`] is never made on the CPU.
    // Port of: src/core/SkCanvas.cpp#L2481-L2486 (chrome/m156)
    #[doc(alias = "drawSlug")]
    pub fn draw_slug(&self, slug: Option<&Slug>, _paint: &Paint) -> &Self {
        if let Some(slug) = slug {
            match *slug {}
        }
        self
    }

    /// Makes a one-run list of `run` at `origin` and draws it: the tail every `drawGlyphs` overload shares.
    // Port of: src/core/SkCanvas.cpp#L2516-L2552 (chrome/m156), the shared tail of drawGlyphs
    fn draw_glyph_run(&self, run: GlyphRun, origin: Point, paint: &Paint) {
        let builder = GlyphRunBuilder::new();
        let list = builder.make_glyph_run_list(run, paint, origin);
        self.state.borrow_mut().draw_glyph_run_list(&list, paint);
    }

    /// Draws a path (`drawPath`).
    #[doc(alias = "drawPath")]
    pub fn draw_path(&self, path: &Path, paint: &Paint) -> &Self {
        self.state.borrow_mut().draw_path(path, paint);
        self
    }

    /// Draws `picture`, optionally transformed by `matrix` and with `paint` applied to the
    /// result (`drawPicture`).
    // Port of: src/core/SkCanvas.cpp#L2887-L2898 (chrome/m156)
    #[doc(alias = "drawPicture")]
    pub fn draw_picture(
        &self,
        picture: impl AsRef<Picture>,
        matrix: Option<&Matrix>,
        paint: Option<&Paint>,
    ) -> &Self {
        let picture = picture.as_ref();

        let matrix = matrix.filter(|m| !m.is_identity());
        if picture.approximate_op_count() <= MAX_PICTURE_OPS_TO_UNROLL_INSTEAD_OF_REF {
            let _acmp = AutoCanvasMatrixPaint::new(self, matrix, paint, &picture.cull_rect());
            picture.playback(self);
        } else {
            self.on_draw_picture(picture, matrix, paint);
        }
        self
    }

    // Port of: src/core/SkCanvas.cpp#L2900-L2908 (chrome/m156)
    fn on_draw_picture(&self, picture: &Picture, matrix: Option<&Matrix>, paint: Option<&Paint>) {
        {
            let mut s = self.state.borrow_mut();
            if let Some(hooks) = s.hooks.as_mut()
                && hooks.on_draw_picture(picture, matrix, paint)
            {
                return;
            }
            let default_paint = Paint::default();
            if s.internal_quick_reject(
                &picture.cull_rect(),
                paint.unwrap_or(&default_paint),
                matrix,
            ) {
                return;
            }
        }

        let _acmp = AutoCanvasMatrixPaint::new(self, matrix, paint, &picture.cull_rect());
        picture.playback(self);
    }

    /// Draws `image` with its top-left corner at `left_top` using the current clip, matrix and
    /// `paint`, with nearest neighbor sampling (`drawImage`).
    #[doc(alias = "drawImage")]
    pub fn draw_image(
        &self,
        image: impl AsRef<Image>,
        left_top: impl Into<Point>,
        paint: Option<&Paint>,
    ) -> &Self {
        self.draw_image_with_sampling_options(image, left_top, SamplingOptions::default(), paint)
    }

    /// Draws `image` with its top-left corner at `left_top` with `sampling` (`drawImage`).
    #[doc(alias = "drawImage")]
    pub fn draw_image_with_sampling_options(
        &self,
        image: impl AsRef<Image>,
        left_top: impl Into<Point>,
        sampling: impl Into<SamplingOptions>,
        paint: Option<&Paint>,
    ) -> &Self {
        let left_top = left_top.into();
        self.state.borrow_mut().draw_image(
            image.as_ref(),
            left_top.x,
            left_top.y,
            &sampling.into(),
            paint,
        );
        self
    }

    /// Draws the `src` rect of `image` (all of it if `None`) into `dst` with nearest neighbor
    /// sampling (`drawImageRect`). The constraint applies only to `src`.
    #[doc(alias = "drawImageRect")]
    pub fn draw_image_rect(
        &self,
        image: impl AsRef<Image>,
        src: Option<(&Rect, SrcRectConstraint)>,
        dst: impl AsRef<Rect>,
        paint: &Paint,
    ) -> &Self {
        self.draw_image_rect_with_sampling_options(
            image,
            src,
            dst,
            SamplingOptions::default(),
            paint,
        )
    }

    /// Draws the `src` rect of `image` (all of it if `None`) into `dst` with `sampling`
    /// (`drawImageRect`).
    #[doc(alias = "drawImageRect")]
    pub fn draw_image_rect_with_sampling_options(
        &self,
        image: impl AsRef<Image>,
        src: Option<(&Rect, SrcRectConstraint)>,
        dst: impl AsRef<Rect>,
        sampling: impl Into<SamplingOptions>,
        paint: &Paint,
    ) -> &Self {
        let sampling = sampling.into();
        let mut state = self.state.borrow_mut();
        match src {
            Some((src, constraint)) => state.draw_image_rect(
                image.as_ref(),
                src,
                dst.as_ref(),
                &sampling,
                Some(paint),
                constraint,
            ),
            None => {
                state.draw_image_rect_whole(image.as_ref(), dst.as_ref(), &sampling, Some(paint));
            }
        }
        drop(state);
        self
    }

    /// Draws `image` stretched proportionally to fit into `dst`: the `center` rect of the image
    /// is stretched, the corners are not scaled, and the other eight patches are stretched in
    /// one direction (`drawImageNine`).
    #[doc(alias = "drawImageNine")]
    pub fn draw_image_nine(
        &self,
        image: impl AsRef<Image>,
        center: impl AsRef<IRect>,
        dst: impl AsRef<Rect>,
        filter_mode: FilterMode,
        paint: Option<&Paint>,
    ) -> &Self {
        self.state.borrow_mut().draw_image_nine(
            image.as_ref(),
            center.as_ref(),
            dst.as_ref(),
            filter_mode,
            paint,
        );
        self
    }

    /// Draws `image` stretched proportionally to fit into `dst`, divided by `lattice` into a
    /// rectangular grid (`drawImageLattice`).
    #[doc(alias = "drawImageLattice")]
    pub fn draw_image_lattice(
        &self,
        image: impl AsRef<Image>,
        lattice: &Lattice<'_>,
        dst: impl AsRef<Rect>,
        filter: FilterMode,
        paint: Option<&Paint>,
    ) -> &Self {
        self.state.borrow_mut().draw_image_lattice(
            image.as_ref(),
            lattice,
            dst.as_ref(),
            filter,
            paint,
        );
        self
    }

    /// `drawImageRect` with a nullable paint, as the record replays it (`SkRecords::Draw`).
    pub(crate) fn draw_image_rect_nullable_paint(
        &self,
        image: &Image,
        src: &Rect,
        dst: &Rect,
        sampling: &SamplingOptions,
        paint: Option<&Paint>,
        constraint: SrcRectConstraint,
    ) {
        self.state
            .borrow_mut()
            .draw_image_rect(image, src, dst, sampling, paint, constraint);
    }

    /// The device-level hook used by pictures and tests: runs `f` with the top device.
    pub fn with_top_device<R>(&self, f: impl FnOnce(&mut dyn Device) -> R) -> R {
        let mut s = self.state.borrow_mut();
        f(s.top_device_mut())
    }
}

/// A canvas that draws into pixels borrowed for `'lt` (`OwnedCanvas`): the pixels are handed back
/// when it is dropped.
pub struct OwnedCanvas<'lt> {
    canvas: Canvas,
    give_back: Option<Box<dyn FnOnce(Bitmap) + 'lt>>,
}

impl<'lt> OwnedCanvas<'lt> {
    /// An owned canvas; `give_back` receives the root device's bitmap on drop.
    #[must_use]
    pub fn new(canvas: Canvas, give_back: Option<Box<dyn FnOnce(Bitmap) + 'lt>>) -> Self {
        OwnedCanvas { canvas, give_back }
    }
}

impl Deref for OwnedCanvas<'_> {
    type Target = Canvas;
    fn deref(&self) -> &Canvas {
        &self.canvas
    }
}

impl std::fmt::Debug for OwnedCanvas<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedCanvas")
            .field("canvas", &self.canvas)
            .finish()
    }
}

impl Drop for OwnedCanvas<'_> {
    fn drop(&mut self) {
        self.canvas.abandon_layers();
        if let Some(give_back) = self.give_back.take()
            && let Some(bm) = self.canvas.take_root_bitmap()
        {
            give_back(bm);
        }
    }
}

/// Restores the canvas to its save count when dropped (`SkAutoCanvasRestore`).
#[doc(alias = "SkAutoCanvasRestore")]
#[derive(Debug)]
pub struct AutoRestoredCanvas<'a> {
    canvas: &'a Canvas,
    save_count: usize,
}

impl Deref for AutoRestoredCanvas<'_> {
    type Target = Canvas;
    fn deref(&self) -> &Canvas {
        self.canvas
    }
}

impl Drop for AutoRestoredCanvas<'_> {
    fn drop(&mut self) {
        self.canvas.restore_to_count(self.save_count);
    }
}

impl AutoRestoredCanvas<'_> {
    /// Restores now (`restore`).
    pub fn restore(self) {}
}

/// `SkAutoCanvasRestore`.
#[doc(alias = "SkAutoCanvasRestore")]
#[derive(Debug)]
pub enum AutoCanvasRestore {}

impl AutoCanvasRestore {
    /// Saves the canvas if `do_save` and restores it on drop.
    // Port of: include/core/SkCanvas.h#L2600-L2640 (chrome/m156)
    #[must_use]
    pub fn guard(canvas: &Canvas, do_save: bool) -> AutoRestoredCanvas<'_> {
        let save_count = canvas.save_count();
        if do_save {
            let _ = canvas.save();
        }
        AutoRestoredCanvas { canvas, save_count }
    }
}
