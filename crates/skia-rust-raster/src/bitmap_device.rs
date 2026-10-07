// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkBitmapDevice.h, src/core/SkBitmapDevice.cpp

//! [`BitmapDevice`]: the raster device (`SkBitmapDevice`). It draws into a [`Bitmap`] with
//! [`Draw`], through a clip stack of [`RasterClip`]s ([`RasterClipStack`]), and tiles devices
//! larger than 8K ([`DrawTiler`]).
//!
//! skia-rust:
//! * The device owns its bitmap (`SkBitmapDevice` keeps a copy sharing the pixels, which the
//!   copy-on-write pixel refs of `docs/design/pixels.md` rule out); read it with
//!   [`BitmapDevice::bitmap`] or take it back with [`BitmapDevice::into_bitmap`].
//! * Not ported yet (they need images, vertices, meshes, text or special images, ported in
//!   D6/D7 and Phase 3): `drawImageRect`, `drawBitmap`, `drawVertices`, `drawMesh`, `drawAtlas`,
//!   `onDrawGlyphRunList` and its `GlyphRunListPainter`, `drawSpecial`, `drawCoverageMask`,
//!   `drawBlurredRRect`, `snapSpecial`, `makeSurface` and the `SkRasterHandleAllocator`
//!   (`fRasterHandle`), nor the `skcpu::Recorder`. [`Device::clip_shader`]'s `makeWithCTM` /
//!   `makeInvertAlpha` wrappers need shader machinery of Phase 3; [`Device::on_clip_shader`]
//!   takes the finished shader. See the "As implemented in D5" design note for what D6 wires up.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{PointMode, SrcRectConstraint};
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::device::{CreateInfo, Device, DeviceState};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::{IPoint, Point};
use skia_rust_core::rect::{IRect, Rect, RoundOut};
use skia_rust_core::region::Region;
use skia_rust_core::rrect::RRect;
use skia_rust_core::shader::Shader;
use skia_rust_core::special_image::SpecialImage;
use skia_rust_core::surface_props::SurfaceProps;

use crate::draw::Draw;
use crate::raster_clip::RasterClip;
use crate::raster_clip_stack::RasterClipStack;
use skia_rust_core::color_type::ColorType;

// 8K is 1 too big, since 8K << supersample == 32768 which is too big for SkFixed
const MAX_DIM: i32 = 8192 - 1;

/// The bounds of a draw's fast bounds, if the paint can compute them (`Bounder`).
// Port of: src/core/SkBitmapDevice.cpp#L47-L62 (chrome/m156)
struct Bounder {
    bounds: Option<Rect>,
}

impl Bounder {
    fn new(r: &Rect, paint: &Paint) -> Bounder {
        Bounder {
            bounds: paint.can_compute_fast_bounds().then(|| {
                if r.is_sorted() {
                    paint.compute_fast_bounds(r)
                } else {
                    // `computeFastBounds` only asserts (debug builds) that the rect is sorted;
                    // the release code is `doComputeFastBounds` without the fill shortcut.
                    paint.do_compute_fast_bounds(r, paint.style())
                }
            }),
        }
    }

    fn bounds(&self) -> Option<&Rect> {
        self.bounds.as_ref()
    }
}

/// What the next iteration of a [`DrawTiler`] draws into.
#[derive(Copy, Clone, Debug)]
enum Tile {
    /// The whole device, with the device's own matrix and clip.
    Whole,
    /// The sub-rectangle `bounds` of the device, with the tiler's translated matrix and clip.
    Sub(IRect),
}

/// Splits a draw into tiles of at most 8K x 8K when the device is larger, because the scan
/// converters cannot handle coordinates that large (`SkDrawTiler`).
///
/// skia-rust: Skia's tiler owns the `skcpu::Draw` it hands out; a Rust draw borrows the device's
/// pixels, so [`DrawTiler::next`] only advances the iteration, returns which [`Tile`] to draw
/// into, and exposes the tile matrix and clip; [`BitmapDevice`] builds the draw.
// Port of: src/core/SkBitmapDevice.cpp#L64-L184 (chrome/m156)
#[doc(alias = "SkDrawTiler")]
#[derive(Debug)]
pub struct DrawTiler {
    src_bounds: IRect,

    // fTileMatrix... are only used if fNeedTiling
    tile_matrix: Matrix,
    tile_rc: RasterClip,
    origin: IPoint,

    done: bool,
    needs_tiling: bool,
}

impl DrawTiler {
    /// Whether a device of this size needs tiling (`NeedsTiling`).
    #[must_use]
    pub fn needs_tiling_for(width: i32, height: i32) -> bool {
        width > MAX_DIM || height > MAX_DIM
    }

    /// A tiler for a draw that touches `bounds` (in local coordinates; `None` for everywhere)
    /// on a device with the matrix `local_to_device` and the clip `rc`.
    // Port of: src/core/SkBitmapDevice.cpp#L83-L131 (chrome/m156)
    #[must_use]
    pub fn new(local_to_device: &Matrix, rc: &RasterClip, bounds: Option<&Rect>) -> DrawTiler {
        let mut done = false;
        let mut src_bounds = IRect::new_empty();

        // do a quick check, so we don't even have to process "bounds" if there is no need
        let clip_r = *rc.bounds();
        let mut needs_tiling = clip_r.right > MAX_DIM || clip_r.bottom > MAX_DIM;
        if needs_tiling {
            if let Some(bounds) = bounds {
                // Make sure we round first, and then intersect. We can't rely on promoting the
                // clipR to floats (and then intersecting with devBounds) since promoting
                // int --> float can make the float larger than the int.
                // rounding(out) first runs the risk of clamping if the float is larger an intmax
                // but our roundOut() is saturating, which is fine for this use case
                //
                // e.g. the older version of this code did this:
                //    devBounds = mapRect(bounds);
                //    if (devBounds.intersect(SkRect::Make(clipR))) {
                //        fSrcBounds = devBounds.roundOut();
                // The problem being that the promotion of clipR to SkRect was unreliable
                //
                src_bounds = local_to_device.map_rect(bounds).0.round_out();
                if let Some(r) = IRect::intersect(&src_bounds, &clip_r) {
                    src_bounds = r;
                    // Check again, now that we have computed srcbounds.
                    needs_tiling = src_bounds.right > MAX_DIM || src_bounds.bottom > MAX_DIM;
                } else {
                    needs_tiling = false;
                    done = true;
                }
            } else {
                src_bounds = clip_r;
            }
        }

        let origin = if needs_tiling {
            // we'll step/increase it before using it
            IPoint::new(src_bounds.left - MAX_DIM, src_bounds.top)
        } else {
            IPoint::new(0, 0)
        };

        DrawTiler {
            src_bounds,
            tile_matrix: Matrix::new_identity(),
            tile_rc: RasterClip::new(),
            origin,
            done,
            needs_tiling,
        }
    }

    /// Whether the draw is tiled (`needsTiling`).
    #[doc(alias = "needsTiling")]
    #[must_use]
    pub fn needs_tiling(&self) -> bool {
        self.needs_tiling
    }

    /// The matrix of the current tile (valid after [`next`](Self::next) returned a
    /// [`Tile::Sub`]).
    fn tile_matrix(&self) -> &Matrix {
        &self.tile_matrix
    }

    /// The clip of the current tile.
    fn tile_rc(&self) -> &RasterClip {
        &self.tile_rc
    }

    /// Advances to the next tile (`next`), or `None` when every tile has been drawn.
    // Port of: src/core/SkBitmapDevice.cpp#L135-L154 (chrome/m156)
    fn next(
        &mut self,
        local_to_device: &Matrix,
        rc: &RasterClip,
        root_dimensions: (i32, i32),
    ) -> Option<Tile> {
        if self.done {
            return None;
        }
        if self.needs_tiling {
            loop {
                // might set the clip to empty and fDone to true
                self.step_and_setup_tile_draw(local_to_device, rc, root_dimensions);
                if self.done || !self.tile_rc.is_empty() {
                    break;
                }
            }
            // if we exit the loop and we're still empty, we're (past) done
            if self.tile_rc.is_empty() {
                debug_assert!(self.done);
                return None;
            }
            debug_assert!(!self.tile_rc.is_empty());
            let bounds = IRect::from_xywh(self.origin.x, self.origin.y, MAX_DIM, MAX_DIM);
            Some(Tile::Sub(bounds))
        } else {
            self.done = true; // only draw untiled once
            Some(Tile::Whole)
        }
    }

    // Port of: src/core/SkBitmapDevice.cpp#L156-L183 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors the int -> float conversion of `postTranslate`
    fn step_and_setup_tile_draw(
        &mut self,
        local_to_device: &Matrix,
        rc: &RasterClip,
        root_dimensions: (i32, i32),
    ) {
        debug_assert!(!self.done);
        debug_assert!(self.needs_tiling);

        // We do fRootPixmap.width() - kMaxDim instead of fOrigin.fX + kMaxDim to avoid overflow.
        if self.origin.x >= self.src_bounds.right - MAX_DIM {
            // too far
            self.origin.x = self.src_bounds.left;
            self.origin.y += MAX_DIM;
        } else {
            self.origin.x += MAX_DIM;
        }
        // fDone = next origin will be invalid.
        self.done = self.origin.x >= self.src_bounds.right - MAX_DIM
            && self.origin.y >= self.src_bounds.bottom - MAX_DIM;

        let bounds = IRect::from_xywh(self.origin.x, self.origin.y, MAX_DIM, MAX_DIM);
        debug_assert!(!bounds.is_empty());
        // The sub-pixmap is `bounds` clipped to the root's, whose size is what the tile clip is
        // limited to (`fDraw.fDst.dimensions()`).
        let sub = IRect::intersect(
            &bounds,
            &IRect::from_wh(root_dimensions.0, root_dimensions.1),
        )
        .expect("the tile intersects the pixmap (SkASSERT_RELEASE(success))");
        // now don't use bounds, since fDst has the clipped dimensions.

        self.tile_matrix = local_to_device.clone();
        self.tile_matrix
            .post_translate((-(self.origin.x as f32), -(self.origin.y as f32)));
        rc.translate(-self.origin.x, -self.origin.y, &mut self.tile_rc);
        self.tile_rc.op_irect(
            &IRect::from_wh(sub.width(), sub.height()),
            ClipOp::Intersect,
        );
    }
}

// Port of: src/core/SkBitmapDevice.cpp#L186-L205 (chrome/m156)
fn valid_for_bitmap_device(info: &ImageInfo, new_alpha_type: Option<&mut AlphaType>) -> bool {
    if info.width() < 0 || info.height() < 0 || ColorType::Unknown == info.color_type() {
        return false;
    }

    if let Some(new_alpha_type) = new_alpha_type {
        *new_alpha_type = if info.color_type().is_always_opaque() {
            AlphaType::Opaque
        } else {
            info.alpha_type()
        };
    }

    true
}

/// The raster device: draws into a [`Bitmap`] (`SkBitmapDevice`).
// Port of: src/core/SkBitmapDevice.h#L46-L141 (chrome/m156)
#[doc(alias = "SkBitmapDevice")]
#[derive(Debug)]
pub struct BitmapDevice {
    state: DeviceState,
    bitmap: Bitmap,
    rc_stack: RasterClipStack,
}

impl BitmapDevice {
    /// A device with `bitmap` as its backend and default surface properties. The bitmap may
    /// have no pixels, in which case any drawing to the device has no effect
    /// (`SkBitmapDevice(const SkBitmap&)`).
    // Port of: src/core/SkBitmapDevice.cpp#L213-L214 (chrome/m156)
    #[must_use]
    pub fn new(bitmap: Bitmap) -> BitmapDevice {
        Self::with_props(bitmap, SurfaceProps::default())
    }

    /// A device with `bitmap` as its backend and the given surface properties
    /// (`SkBitmapDevice(const SkBitmap&, const SkSurfaceProps&)`).
    ///
    /// skia-rust: the external raster handle is not ported.
    // Port of: src/core/SkBitmapDevice.cpp#L225-L237 (chrome/m156)
    #[must_use]
    pub fn with_props(bitmap: Bitmap, surface_props: SurfaceProps) -> BitmapDevice {
        debug_assert!(valid_for_bitmap_device(bitmap.info(), None));
        BitmapDevice {
            state: DeviceState::new(bitmap.info().clone(), surface_props),
            rc_stack: RasterClipStack::new(bitmap.width(), bitmap.height()),
            bitmap,
        }
    }

    /// Makes a device with newly allocated pixels, zeroed unless the info is opaque (Skia leaves
    /// those uninitialized; they are zero here too). `None` if the info is not valid for a
    /// bitmap device or the allocation fails (`Create`).
    ///
    /// skia-rust: `SkRasterHandleAllocator` is not ported.
    // Port of: src/core/SkBitmapDevice.cpp#L243-L276 (chrome/m156)
    #[must_use]
    pub fn create(orig_info: &ImageInfo, surface_props: SurfaceProps) -> Option<BitmapDevice> {
        let mut new_at = orig_info.alpha_type();
        if !valid_for_bitmap_device(orig_info, Some(&mut new_at)) {
            return None;
        }

        let info = orig_info.with_alpha_type(new_at);
        let mut bitmap = Bitmap::new();

        if ColorType::Unknown == info.color_type() {
            if !bitmap.set_info(&info, None) {
                return None;
            }
        } else if info.is_opaque() {
            // If this bitmap is opaque, we don't have any sensible default color,
            // so we just return uninitialized pixels.
            if !bitmap.try_alloc_pixels_info(&info, None) {
                return None;
            }
        } else {
            // This bitmap has transparency, so we'll zero the pixels (to transparent).
            // We use the flag as a faster alloc-then-eraseColor(SK_ColorTRANSPARENT).
            if !bitmap.try_alloc_pixels_flags(&info) {
                return None;
            }
        }

        Some(BitmapDevice::with_props(bitmap, surface_props))
    }

    /// The backing bitmap.
    #[must_use]
    pub fn bitmap(&self) -> &Bitmap {
        &self.bitmap
    }

    /// The backing bitmap, mutably.
    pub fn bitmap_mut(&mut self) -> &mut Bitmap {
        &mut self.bitmap
    }

    /// Takes the backing bitmap back.
    #[must_use]
    pub fn into_bitmap(self) -> Bitmap {
        self.bitmap
    }

    /// The device's current clip (`fRCStack.rc()`).
    #[must_use]
    pub fn raster_clip(&self) -> &RasterClip {
        self.rc_stack.rc()
    }

    /// Replaces the bitmap with one with the same size and pixels of another config or row
    /// bytes, without touching the clip (`replaceBitmapBackendForRasterSurface`).
    // Port of: src/core/SkBitmapDevice.cpp#L278-L282 (chrome/m156)
    #[doc(alias = "replaceBitmapBackendForRasterSurface")]
    pub fn replace_bitmap_backend_for_raster_surface(&mut self, bm: Bitmap) {
        debug_assert_eq!(bm.width(), self.bitmap.width());
        debug_assert_eq!(bm.height(), self.bitmap.height());
        self.bitmap = bm; // intent is to use bm's pixelRef (and rowbytes/config)
    }

    /// The pixels for drawing, with the generation ID bumped (`accessPixels`); an empty pixmap
    /// if the bitmap has none ("`NoDrawDevice` uses us (why?) so we have to catch this case w/ no
    /// pixels").
    fn draw_pixmap(bitmap: &mut Bitmap) -> Pixmap<'_> {
        // we need fDst to be set, and if we're actually drawing, to dirty the genID
        bitmap.notify_pixels_changed();
        if bitmap.color_type() == ColorType::Unknown {
            return Pixmap::default();
        }
        bitmap.peek_pixels_mut().unwrap_or_default()
    }

    /// Runs `code` for each tile of a draw that touches `bounds` (`LOOP_TILER`). Passing a
    /// bounds allows the tiler to only visit the dst-tiles that might intersect the drawing. If
    /// `None` is passed, the tiler has to visit everywhere. The bounds is expected to be in local
    /// coordinates, as the tiler itself will transform that into device coordinates.
    // Port of: src/core/SkBitmapDevice.cpp#L186-L195 (chrome/m156)
    fn loop_tiler(&mut self, bounds: Option<&Rect>, mut code: impl FnMut(&mut Draw<'_>)) {
        let BitmapDevice {
            state,
            bitmap,
            rc_stack,
        } = self;
        let rc = rc_stack.rc();
        let local_to_device = state.local_to_device();
        let props = *state.surface_props();
        let mut tiler = DrawTiler::new(local_to_device, rc, bounds);
        let mut root = Self::draw_pixmap(bitmap);
        let dimensions = (root.width(), root.height());

        while let Some(tile) = tiler.next(local_to_device, rc, dimensions) {
            match tile {
                Tile::Whole => {
                    let mut draw = Draw::new(root.reborrow_mut(), local_to_device, rc);
                    draw.props = Some(&props);
                    code(&mut draw);
                }
                Tile::Sub(tile_bounds) => {
                    let Some(dst) = root.extract_subset_mut(tile_bounds) else {
                        panic!("SkASSERT_RELEASE(success)");
                    };
                    let mut draw = Draw::new(dst, tiler.tile_matrix(), tiler.tile_rc());
                    draw.props = Some(&props);
                    code(&mut draw);
                }
            }
        }
    }

    /// Runs `code` with a draw over the whole device (`BDDraw`); no properties are set.
    // Port of: src/core/SkBitmapDevice.cpp#L197-L208 (chrome/m156)
    fn bd_draw(&mut self, code: impl FnOnce(&mut Draw<'_>)) {
        let BitmapDevice {
            state,
            bitmap,
            rc_stack,
        } = self;
        let dst = Self::draw_pixmap(bitmap);
        let mut draw = Draw::new(dst, state.local_to_device(), rc_stack.rc());
        code(&mut draw);
    }
}

impl Device for BitmapDevice {
    fn state(&self) -> &DeviceState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut DeviceState {
        &mut self.state
    }

    // Port of: src/core/SkBitmapDevice.cpp#L294-L300 (chrome/m156)
    fn on_access_pixels(&mut self) -> Option<Pixmap<'_>> {
        if self
            .bitmap
            .peek_pixels()
            .is_some_and(|pm| pm.color_type() != ColorType::Unknown)
        {
            self.bitmap.notify_pixels_changed();
            return self.bitmap.peek_pixels_mut();
        }
        None
    }

    // Port of: src/core/SkBitmapDevice.cpp#L302-L309 (chrome/m156)
    fn on_peek_pixels(&self) -> Option<Pixmap<'_>> {
        if self.bitmap.color_type() != ColorType::Unknown {
            return self.bitmap.peek_pixels();
        }
        None
    }

    // Port of: src/core/SkBitmapDevice.cpp#L311-L322 (chrome/m156)
    fn on_write_pixels(&mut self, pm: &Pixmap<'_>, x: i32, y: i32) -> bool {
        // since we don't stop creating un-pixeled devices yet, check for no pixels here
        if self.bitmap.peek_pixels().is_none() {
            return false;
        }

        if self.bitmap.write_pixels(pm, x, y) {
            self.bitmap.notify_pixels_changed();
            return true;
        }
        false
    }

    // Port of: src/core/SkBitmapDevice.cpp#L324-L326 (chrome/m156)
    fn on_read_pixels(&mut self, pm: &mut Pixmap<'_>, x: i32, y: i32) -> bool {
        self.bitmap.read_pixels_to_pixmap(pm, (x, y))
    }

    // Port of: src/core/SkBitmapDevice.cpp#L284-L297 (chrome/m156)
    fn create_device(
        &mut self,
        cinfo: &CreateInfo,
        layer_paint: Option<&Paint>,
    ) -> Option<Box<dyn Device>> {
        let surface_props = self
            .state
            .surface_props()
            .clone_with_pixel_geometry(cinfo.pixel_geometry);

        // Need to force L32 for now if we have an image filter.
        // If filters ever support other colortypes, e.g. F16, we can modify this check.
        let mut info = cinfo.info.clone();
        if layer_paint.is_some_and(|p| p.image_filter().is_some()) {
            // TODO: can we query the imagefilter, to see if it can handle floats (so we don't
            //       always use N32 when the layer itself was float)?
            info = info.with_color_type(ColorType::N32);
        }

        BitmapDevice::create(&info, surface_props).map(|d| Box::new(d) as Box<dyn Device>)
    }

    // Port of: src/core/SkBitmapDevice.h#L130 (chrome/m156)
    fn set_immutable(&mut self) {
        self.bitmap.set_immutable();
    }

    // Port of: src/core/SkBitmapDevice.cpp#L573-L593 (chrome/m156)
    fn draw_special(
        &mut self,
        src: &SpecialImage,
        local_to_device: &Matrix,
        paint: &Paint,
        _constraint: SrcRectConstraint,
    ) {
        debug_assert!(paint.image_filter().is_none());
        debug_assert!(paint.mask_filter().is_none());

        if let Some(result_bm) = src.as_bitmap() {
            let BitmapDevice {
                bitmap, rc_stack, ..
            } = self;
            // `accessPixels(&draw.fDst)`
            if bitmap.color_type() == ColorType::Unknown || bitmap.peek_pixels().is_none() {
                return; // no pixels to draw to so skip it
            }
            bitmap.notify_pixels_changed();
            let Some(dst) = bitmap.peek_pixels_mut() else {
                return;
            };
            let mut draw = Draw::new(dst, local_to_device, rc_stack.rc());
            draw.draw_bitmap(&result_bm, Matrix::i(), None, false, paint);
        }
    }

    // Port of: src/core/SkBitmapDevice.cpp#L641-L647 (chrome/m156)
    fn snap_special(&mut self, bounds: &IRect, force_copy: bool) -> Option<SpecialImage> {
        if force_copy {
            SpecialImage::copy_from_raster(bounds, &self.bitmap, self.state.surface_props())
        } else {
            SpecialImage::make_from_raster(bounds, &self.bitmap, self.state.surface_props())
        }
    }

    fn bitmap_mut(&mut self) -> Option<&mut Bitmap> {
        Some(&mut self.bitmap)
    }

    // Port of: src/core/SkBitmapDevice.cpp#L330-L332 (chrome/m156)
    fn draw_paint(&mut self, paint: &Paint) {
        self.bd_draw(|draw| draw.draw_paint(paint));
    }

    // Port of: src/core/SkBitmapDevice.cpp#L334-L337 (chrome/m156)
    fn draw_points(&mut self, mode: PointMode, pts: &[Point], paint: &Paint) {
        self.loop_tiler(None, |draw| draw.draw_points(mode, pts, paint, None));
    }

    // Port of: src/core/SkBitmapDevice.cpp#L339-L341 (chrome/m156)
    fn draw_rect(&mut self, r: &Rect, paint: &Paint) {
        let bounder = Bounder::new(r, paint);
        self.loop_tiler(bounder.bounds(), |draw| draw.draw_rect(r, paint));
    }

    // Port of: src/core/SkBitmapDevice.cpp#L343-L345 (chrome/m156)
    fn draw_oval(&mut self, oval: &Rect, paint: &Paint) {
        let bounder = Bounder::new(oval, paint);
        self.loop_tiler(bounder.bounds(), |draw| draw.draw_oval(oval, paint));
    }

    // Port of: src/core/SkBitmapDevice.cpp#L347-L349 (chrome/m156)
    fn draw_rrect(&mut self, rrect: &RRect, paint: &Paint) {
        let bounder = Bounder::new(rrect.bounds(), paint);
        self.loop_tiler(bounder.bounds(), |draw| draw.draw_rrect(rrect, paint));
    }

    // Port of: src/core/SkBitmapDevice.cpp#L351-L360 (chrome/m156)
    fn draw_path(&mut self, path: &Path, paint: &Paint) {
        let mut bounds = None;
        if DrawTiler::needs_tiling_for(self.state.width(), self.state.height())
            && !path.is_inverse_fill_type()
        {
            bounds = Some(*path.bounds());
        }
        let bounder = bounds.map(|b| Bounder::new(&b, paint));
        self.loop_tiler(bounder.as_ref().and_then(Bounder::bounds), |draw| {
            draw.draw_path(path, paint, None);
        });
    }

    // Port of: src/core/SkBitmapDevice.cpp#L582-L584 (chrome/m156)
    fn push_clip_stack(&mut self) {
        self.rc_stack.save();
    }

    // Port of: src/core/SkBitmapDevice.cpp#L586-L588 (chrome/m156)
    fn pop_clip_stack(&mut self) {
        self.rc_stack.restore();
    }

    // Port of: src/core/SkBitmapDevice.cpp#L590-L592 (chrome/m156)
    fn clip_rect(&mut self, rect: &Rect, op: ClipOp, aa: bool) {
        self.rc_stack
            .clip_rect(self.state.local_to_device(), rect, op, aa);
    }

    // Port of: src/core/SkBitmapDevice.cpp#L594-L596 (chrome/m156)
    fn clip_rrect(&mut self, rrect: &RRect, op: ClipOp, aa: bool) {
        self.rc_stack
            .clip_rrect(self.state.local_to_device(), rrect, op, aa);
    }

    // Port of: src/core/SkBitmapDevice.cpp#L598-L600 (chrome/m156)
    fn clip_path(&mut self, path: &Path, op: ClipOp, aa: bool) {
        self.rc_stack
            .clip_path(self.state.local_to_device(), path, op, aa);
    }

    // Port of: src/core/SkBitmapDevice.cpp#L602-L604 (chrome/m156)
    fn on_clip_shader(&mut self, sh: Shader) {
        self.rc_stack.clip_shader(sh);
    }

    // Port of: src/core/SkBitmapDevice.cpp#L606-L616 (chrome/m156)
    fn clip_region(&mut self, rgn: &Region, op: ClipOp) {
        let origin = self.state.origin();
        if (origin.x | origin.y) != 0 {
            // translate from "global/canvas" coordinates to relative to this device
            let mut tmp = Region::new();
            rgn.translate_to(-origin.x, -origin.y, &mut tmp);
            self.rc_stack.clip_region(&tmp, op);
        } else {
            self.rc_stack.clip_region(rgn, op);
        }
    }

    // Port of: src/core/SkBitmapDevice.cpp#L618-L622 (chrome/m156)
    fn replace_clip(&mut self, rect: &IRect) {
        // Transform from "global/canvas" coordinates to relative to this device
        let device_rect = skia_rust_core::matrix_priv::map_rect(
            self.state.global_to_device(),
            &Rect::from_irect(rect),
        );
        self.rc_stack.replace_clip(&device_rect.round());
    }

    // Port of: src/core/SkBitmapDevice.cpp#L624-L630 (chrome/m156)
    fn is_clip_wide_open(&self) -> bool {
        let rc = self.rc_stack.rc();
        // If we're AA, we can't be wide-open (we would represent that as BW)
        rc.is_bw()
            && rc.bw_rgn().is_rect()
            && *rc.bw_rgn().bounds() == IRect::new(0, 0, self.state.width(), self.state.height())
    }

    // Port of: src/core/SkBitmapDevice.cpp#L632-L634 (chrome/m156)
    fn is_clip_empty(&self) -> bool {
        self.rc_stack.rc().is_empty()
    }

    // Port of: src/core/SkBitmapDevice.cpp#L636-L639 (chrome/m156)
    fn is_clip_rect(&self) -> bool {
        let rc = self.rc_stack.rc();
        !rc.is_empty() && rc.is_rect() && rc.clip_shader().is_none()
    }

    // Port of: src/core/SkBitmapDevice.cpp#L641-L644 (chrome/m156)
    fn is_clip_anti_aliased(&self) -> bool {
        let rc = self.rc_stack.rc();
        !rc.is_empty() && rc.is_aa()
    }

    // Port of: src/core/SkBitmapDevice.cpp#L646-L653 (chrome/m156)
    fn android_utils_clip_as_rgn(&self, rgn: &mut Region) {
        let rc = self.rc_stack.rc();
        if rc.is_aa() {
            rgn.set_rect(*rc.bounds());
        } else {
            rgn.set(rc.bw_rgn());
        }
    }

    // Port of: src/core/SkBitmapDevice.cpp#L655-L657 (chrome/m156)
    fn dev_clip_bounds(&self) -> IRect {
        *self.rc_stack.rc().bounds()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skia_rust_core::color::Color;
    use skia_rust_core::paint::Style;

    fn n32_device(w: i32, h: i32) -> BitmapDevice {
        BitmapDevice::create(
            &ImageInfo::new_n32_premul((w, h), None),
            SurfaceProps::default(),
        )
        .expect("a device")
    }

    fn pixel(dev: &BitmapDevice, x: i32, y: i32) -> u32 {
        dev.bitmap().get_addr32(x, y)
    }

    #[test]
    fn rect_fill_and_clip() {
        let mut dev = n32_device(16, 8);
        assert!(dev.is_clip_wide_open());
        let mut paint = Paint::default();
        paint.set_color(Color::from(0xFF00_FF00));
        dev.push_clip_stack();
        dev.clip_rect(&Rect::new(2.0, 2.0, 6.0, 6.0), ClipOp::Intersect, false);
        assert!(dev.is_clip_rect() && !dev.is_clip_wide_open());
        dev.draw_rect(&Rect::new(0.0, 0.0, 16.0, 8.0), &paint);
        dev.pop_clip_stack();
        assert!(dev.is_clip_wide_open());
        let green = pixel(&dev, 3, 3);
        assert_ne!(green, 0);
        assert_eq!(pixel(&dev, 1, 3), 0);
        assert_eq!(pixel(&dev, 6, 3), 0);
        assert_eq!(pixel(&dev, 5, 5), green);
    }

    #[test]
    fn read_and_write_pixels() {
        let mut dev = n32_device(8, 8);
        let mut paint = Paint::default();
        paint.set_color(Color::from(0xFFFF_0000));
        dev.draw_paint(&paint);
        let info = ImageInfo::new_n32_premul((4, 4), None);
        let rb = info.min_row_bytes();
        let mut buf = vec![0u8; rb * 4];
        let mut dst = Pixmap::new(&info, &mut buf, rb).unwrap();
        assert!(dev.read_pixels(&mut dst, 2, 2));
        assert_eq!(dst.addr32(0, 0), pixel(&dev, 2, 2));
        assert!(!dev.read_pixels(&mut dst, 100, 100));
        assert!(dev.write_pixels(&dst, 4, 4));
    }

    #[test]
    fn stroked_rect_and_points_use_the_fast_paths() {
        let mut dev = n32_device(32, 32);
        let mut paint = Paint::default();
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(2.0);
        paint.set_color(Color::from(0xFF00_00FF));
        dev.draw_rect(&Rect::new(4.0, 4.0, 20.0, 20.0), &paint);
        assert_ne!(pixel(&dev, 4, 10), 0);
        assert_eq!(pixel(&dev, 10, 10), 0);
        paint.set_stroke_width(3.0);
        dev.draw_points(
            PointMode::Points,
            &[Point::new(25.0, 25.0), Point::new(28.5, 4.5)],
            &paint,
        );
        assert_ne!(pixel(&dev, 25, 25), 0);
        assert_ne!(pixel(&dev, 28, 4), 0);
    }

    #[test]
    fn tiler_splits_huge_clips() {
        let m = Matrix::new_identity();
        let rc = RasterClip::from_rect(&IRect::from_wh(9000, 10));
        let mut tiler = DrawTiler::new(&m, &rc, None);
        assert!(tiler.needs_tiling());
        let mut count = 0;
        while let Some(t) = tiler.next(&m, &rc, (9000, 10)) {
            assert!(matches!(t, Tile::Sub(_)));
            assert!(!tiler.tile_rc().is_empty());
            count += 1;
        }
        assert_eq!(count, 2);
        let rc_small = RasterClip::from_rect(&IRect::from_wh(100, 10));
        let mut tiler = DrawTiler::new(&m, &rc_small, None);
        assert!(!tiler.needs_tiling());
        assert!(matches!(
            tiler.next(&m, &rc_small, (100, 10)),
            Some(Tile::Whole)
        ));
        assert!(tiler.next(&m, &rc_small, (100, 10)).is_none());
    }
}
