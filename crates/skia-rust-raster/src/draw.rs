// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkDraw.h, src/core/SkDraw.cpp

//! [`Draw`]: the workhorse of the software backend (`skcpu::Draw`). It orchestrates the drawing
//! of primitives into a CPU pixmap: it analyses the primitive, the paint and the device state,
//! chooses a blitter ([`auto_blitter_choose`]) and calls the scan converters.
//!
//! A [`Draw`] is a lightweight context: the destination pixels, the current matrix and the clip
//! (plus the optional surface properties and the blitter chooser).
//!
//! skia-rust:
//! * `skcpu::Draw`'s methods are `const` but draw into `fDst`'s pixels through blitters; the
//!   Rust methods take `&mut self` because the blitter holds the `&mut` pixels. `Draw draw(*this)`
//!   (a copy that changes `fCTM`) is [`Draw::reborrow`] plus a field assignment.
//! * Not ported yet (they need images, vertices, text or mask filters, ported in D7 and Phase
//!   3): `drawBitmap` is ported for the sprite case only (D6, layer restores), `drawSprite`,
//!   `drawBitmapAsMask`,
//!   `drawGlyphRunList`/`paintMasks` (text), `drawVertices`/`drawFixedVertices` and `drawAtlas`
//!   (`SkVertices`), and the mask filter branches of `drawDevPath`/`drawRRectNinePatch`
//!   (`SkMaskFilterBase::filterPath`/`filterRects`/`filterRRect`). Where a mask filter would have
//!   drawn, the geometry is drawn unfiltered. See the "As implemented in D5" design note.
//! * `BitmapDevicePainter`, the interface text and bitmaps are painted through, is not ported
//!   with them.

use std::borrow::Cow;

use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode_priv::supports_coverage_as_alpha;
use skia_rust_core::canvas::PointMode;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::device::Device;
use skia_rust_core::draw_procs::draw_treat_as_hairline;
use skia_rust_core::draw_types::DrawCoverage;
use skia_rust_core::floating_point::{float_round2int, float_saturate2int};
use skia_rust_core::glyph::Glyph;
use skia_rust_core::glyph_run::GlyphRunList;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_info_priv::color_type_is_alpha_only;
use skia_rust_core::m44::M44;
use skia_rust_core::mask::{CreateMode, Mask, MaskBuilder, MaskFormat};
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::matrix_utils::treat_as_sprite;
use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_data::PathData;
use skia_rust_core::path_effect::PointData;
use skia_rust_core::path_enums::ResolveConvexity;
use skia_rust_core::path_raw::PathRaw;
use skia_rust_core::path_utils::fill_path_with_paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::{IPoint, Point, Vector};
use skia_rust_core::rect::{IRect, Rect, RoundOut, rect_priv};
use skia_rust_core::region::Cliperator;
use skia_rust_core::rrect::{RRect, Type as RRectType};
use skia_rust_core::scalar::{SCALAR_HALF, SCALAR_SQRT2, Scalar, scalar};
use skia_rust_core::shader::Shader;
use skia_rust_core::stroke_rec::{InitStyle, StrokeRec};
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::{path_priv, scalar as scalar_mod};

use crate::auto_blitter_choose::auto_blitter_choose;
use crate::blitter::Blitter;
use crate::blitter_a8::choose_a8_blitter;
use crate::blitter_choose::choose;
use crate::glyph_run_painter::{BitmapDevicePainter, GlyphRunListPainter};
use crate::raster_clip::{AAClipBlitterWrapper, RasterClip};
use crate::scan::{
    fill_irect_clip, fill_path_clip, fill_rect_clip, fill_xrect_clip, xrect_set_rect,
};
use crate::scan_anti_path::anti_fill_path_clip;
use crate::scan_antihair::{
    anti_fill_rect_clip, anti_fill_x_rect_clip, anti_frame_rect_clip, anti_hair_line,
    anti_hair_rect,
};
use crate::scan_hairline::{
    anti_hair_path, anti_hair_round_path, anti_hair_square_path, frame_rect, hair_line, hair_path,
    hair_rect, hair_round_path, hair_square_path,
};
use crate::sprite_blitter::choose_sprite;

/// The function `Draw` calls to choose a blitter (`Draw::BlitterChooser`). The default is
/// [`choose`] (`SkBlitter::Choose`).
///
/// skia-rust: it returns the blitter by value (`Box<dyn Blitter>`) instead of allocating it in
/// `alloc`, which it takes to build the blitter's contexts in.
// Port of: src/core/SkDraw.h#L158-L165 (chrome/m156)
pub type BlitterChooser = for<'b> fn(
    dst: Pixmap<'b>,
    ctm: &Matrix,
    paint: &Paint,
    alloc: &'b ArenaAlloc,
    draw_coverage: DrawCoverage,
    clip_shader: Option<&Shader>,
    props: &SurfaceProps,
    dev_bounds: &Rect,
) -> Box<dyn Blitter + 'b>;

/// `SkBlitter::Choose` as a [`BlitterChooser`].
#[allow(clippy::too_many_arguments)] // Skia's BlitterChooser signature
fn choose_default<'b>(
    dst: Pixmap<'b>,
    ctm: &Matrix,
    paint: &Paint,
    alloc: &'b ArenaAlloc,
    draw_coverage: DrawCoverage,
    clip_shader: Option<&Shader>,
    props: &SurfaceProps,
    dev_bounds: &Rect,
) -> Box<dyn Blitter + 'b> {
    choose(
        dst,
        ctm,
        paint,
        alloc,
        draw_coverage,
        clip_shader,
        props,
        dev_bounds,
        false,
    )
}

/// How [`Draw::draw_rect`] draws a rectangle (`Draw::RectType`).
// Port of: src/core/SkDraw.h#L131-L136 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RectType {
    /// `kHair`.
    Hair,
    /// `kFill`.
    Fill,
    /// `kStroke`.
    Stroke,
    /// `kPath`.
    Path,
}

// Port of: src/core/SkDraw.cpp#L351-L363 (chrome/m156)
fn clipped_out(m: &Matrix, c: &RasterClip, width: i32, height: i32) -> bool {
    let r = Rect::from_iwh(width, height);
    let dst_r = m.map_rect(r).0;
    c.quick_reject(&dst_r.round_out())
}

// Port of: src/core/SkDraw.cpp#L365-L367 (chrome/m156)
fn clip_handles_sprite(clip: &RasterClip, x: i32, y: i32, pmap: &Pixmap<'_>) -> bool {
    clip.is_bw() || clip.quick_contains(&IRect::from_xywh(x, y, pmap.width(), pmap.height()))
}

/// The context of one drawing operation: destination pixels, matrix, clip and properties
/// (`skcpu::Draw`).
// Port of: src/core/SkDraw.h#L82-L222 (chrome/m156)
#[doc(alias = "skcpu::Draw")]
#[doc(alias = "SkDraw")]
pub struct Draw<'a> {
    /// The destination (`fDst`).
    pub dst: Pixmap<'a>,
    /// The blitter chooser (`fBlitterChooser`); `SkBlitter::Choose` by default.
    pub blitter_chooser: BlitterChooser,
    /// The matrix from local to device coordinates (`fCTM`).
    pub ctm: &'a Matrix,
    /// The clip, in device coordinates (`fRC`).
    pub rc: &'a RasterClip,
    /// The surface properties (`fProps`), optional.
    pub props: Option<&'a SurfaceProps>,
}

impl std::fmt::Debug for Draw<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Draw")
            .field("dst", &self.dst)
            .field("ctm", self.ctm)
            .field("rc", self.rc)
            .field("props", &self.props)
            .finish_non_exhaustive()
    }
}

// Port of: src/core/SkDraw.cpp#L145-L147 (chrome/m156)
/// A rectangle of `radius` around `center`.
fn make_square_rad(center: Point, radius: scalar) -> Rect {
    Rect::new(
        center.x - radius,
        center.y - radius,
        center.x + radius,
        center.y + radius,
    )
}

/// What a point-drawing procedure needs (`PtProcRec`).
// Port of: src/core/SkDraw.cpp#L90-L109 (chrome/m156)
struct PtProcRec<'p> {
    mode: PointMode,
    paint: &'p Paint,
    rc: &'p RasterClip,

    // computed values
    clip_bounds: Rect,
    radius: scalar,
}

/// `PtProcRec::Proc`; `clip` is `fClip`.
type PtProc = fn(&PtProcRec<'_>, &[Point], &skia_rust_core::region::Region, &mut dyn Blitter);

// Port of: src/core/SkDraw.cpp#L121-L146 (chrome/m156)
fn bw_pt_hair_proc(
    rec: &PtProcRec<'_>,
    dev_pts: &[Point],
    clip: &skia_rust_core::region::Region,
    blitter: &mut dyn Blitter,
) {
    let _ = rec;
    if clip.is_rect()
        && let Some(mut direct) = blitter.can_direct_blit()
    {
        let cr = clip.bounds();
        let v = direct.value;
        let bpp = direct.pm.info().bytes_per_pixel();
        for p in dev_pts {
            // SkScalarFloorToInt
            let x = scalar_mod::scalar_floor_to_int(p.x);
            let y = scalar_mod::scalar_floor_to_int(p.y);
            if rect_contains_point(cr, x, y) {
                // The value's low bits match the pixmap's bit depth (`DirectBlit::value`).
                #[allow(clippy::cast_possible_truncation)] // truncates to the pixel size, as C++
                match bpp {
                    1 => direct.pm.set_addr8(x, y, v as u8),
                    2 => direct.pm.set_addr16(x, y, v as u16),
                    4 => direct.pm.set_addr32(x, y, v as u32),
                    8 => direct.pm.set_addr64(x, y, v),
                    _ => debug_assert!(false),
                }
            }
        }
    } else {
        for p in dev_pts {
            let x = scalar_mod::scalar_floor_to_int(p.x);
            let y = scalar_mod::scalar_floor_to_int(p.y);
            if clip.contains_point(IPoint::new(x, y)) {
                blitter.blit_h(x, y, 1);
            }
        }
    }
}

/// `SkIRect::contains(x, y)`.
fn rect_contains_point(r: &IRect, x: i32, y: i32) -> bool {
    r.left <= x && x < r.right && r.top <= y && y < r.bottom
}

// Port of: src/core/SkDraw.cpp#L148-L154 (chrome/m156)
fn bw_line_hair_proc(
    rec: &PtProcRec<'_>,
    dev_pts: &[Point],
    _clip: &skia_rust_core::region::Region,
    blitter: &mut dyn Blitter,
) {
    let mut i = 0;
    while i + 1 < dev_pts.len() {
        hair_line(&dev_pts[i..i + 2], rec.rc, blitter);
        i += 2;
    }
}

// Port of: src/core/SkDraw.cpp#L156-L159 (chrome/m156)
fn bw_poly_hair_proc(
    rec: &PtProcRec<'_>,
    dev_pts: &[Point],
    _clip: &skia_rust_core::region::Region,
    blitter: &mut dyn Blitter,
) {
    hair_line(dev_pts, rec.rc, blitter);
}

// aa versions

// Port of: src/core/SkDraw.cpp#L163-L169 (chrome/m156)
fn aa_line_hair_proc(
    rec: &PtProcRec<'_>,
    dev_pts: &[Point],
    _clip: &skia_rust_core::region::Region,
    blitter: &mut dyn Blitter,
) {
    let mut i = 0;
    while i + 1 < dev_pts.len() {
        anti_hair_line(&dev_pts[i..i + 2], rec.rc, blitter);
        i += 2;
    }
}

// Port of: src/core/SkDraw.cpp#L171-L174 (chrome/m156)
fn aa_poly_hair_proc(
    rec: &PtProcRec<'_>,
    dev_pts: &[Point],
    _clip: &skia_rust_core::region::Region,
    blitter: &mut dyn Blitter,
) {
    anti_hair_line(dev_pts, rec.rc, blitter);
}

// square procs (strokeWidth > 0 but matrix is square-scale (sx == sy)

// Port of: src/core/SkDraw.cpp#L186-L193 (chrome/m156)
fn bw_square_proc(
    rec: &PtProcRec<'_>,
    dev_pts: &[Point],
    _clip: &skia_rust_core::region::Region,
    blitter: &mut dyn Blitter,
) {
    for p in dev_pts {
        let mut r = make_square_rad(*p, rec.radius);
        if r.intersect(rec.clip_bounds) {
            fill_xrect_clip(&xrect_set_rect(&r), rec.rc, blitter);
        }
    }
}

// Port of: src/core/SkDraw.cpp#L195-L202 (chrome/m156)
fn aa_square_proc(
    rec: &PtProcRec<'_>,
    dev_pts: &[Point],
    _clip: &skia_rust_core::region::Region,
    blitter: &mut dyn Blitter,
) {
    for p in dev_pts {
        let mut r = make_square_rad(*p, rec.radius);
        if r.intersect(rec.clip_bounds) {
            anti_fill_x_rect_clip(&xrect_set_rect(&r), rec.rc, blitter);
        }
    }
}

impl<'p> PtProcRec<'p> {
    /// If this returns `Some`, then `choose_proc` returns a valid proc.
    // Port of: src/core/SkDraw.cpp#L204-L241 (chrome/m156)
    fn init(
        mode: PointMode,
        paint: &'p Paint,
        matrix: &Matrix,
        rc: &'p RasterClip,
    ) -> Option<PtProcRec<'p>> {
        if paint.path_effect().is_some() || paint.mask_filter().is_some() {
            return None;
        }
        let width = paint.stroke_width();
        let mut radius: f32 = -1.0; // sentinel value, a "valid" value must be > 0

        #[allow(clippy::float_cmp)] // mirrors `0 == width`
        if 0.0 == width {
            radius = 0.5;
        } else if paint.stroke_cap() != Cap::Round
            && matrix.is_scale_translate()
            && PointMode::Points == mode
        {
            let sx = matrix.scale_x();
            let sy = matrix.scale_y();
            if (sx - sy).nearly_zero(None) {
                radius = (width * sx.abs()) / 2.0;
            }
        }
        if radius > 0.0 {
            let clip_bounds = Rect::from_irect(rc.bounds());
            // if we return true, the caller may assume that the constructed shapes can be
            // represented using SkFixed (after clipping), so we preflight that here.
            if !rect_priv::fits_in_fixed_rect(&clip_bounds) {
                return None;
            }
            return Some(PtProcRec {
                mode,
                paint,
                rc,
                clip_bounds,
                radius,
            });
        }
        None
    }

    // Port of: src/core/SkDraw.cpp#L243-L283 (chrome/m156)
    fn choose_proc(&self) -> PtProc {
        // for our arrays
        let mut proc: Option<PtProc> = None;

        if self.paint.is_anti_alias() {
            #[allow(clippy::float_cmp)] // mirrors `0 == getStrokeWidth()`
            if 0.0 == self.paint.stroke_width() {
                const AA_PROCS: [PtProc; 3] =
                    [aa_square_proc, aa_line_hair_proc, aa_poly_hair_proc];
                proc = Some(AA_PROCS[self.mode as usize]);
            } else if self.paint.stroke_cap() != Cap::Round {
                debug_assert_eq!(PointMode::Points, self.mode);
                proc = Some(aa_square_proc);
            }
        } else {
            // BW
            if self.radius <= 0.5 {
                // small radii and hairline
                const BW_PROCS: [PtProc; 3] =
                    [bw_pt_hair_proc, bw_line_hair_proc, bw_poly_hair_proc];
                proc = Some(BW_PROCS[self.mode as usize]);
            } else {
                proc = Some(bw_square_proc);
            }
        }
        proc.expect("init() returned true, so a proc exists")
    }
}

// each of these costs 8-bytes of stack space, so don't make it too large
// must be even for lines/polygon to work
const MAX_DEV_PTS: usize = 32;

// Port of: src/core/SkDraw.cpp#L669-L676 (chrome/m156)
fn compute_stroke_size(paint: &Paint, matrix: &Matrix) -> Point {
    debug_assert!(matrix.rect_stays_rect());
    debug_assert_ne!(Style::Fill, paint.style());

    let size = matrix.map_vector(Vector::new(paint.stroke_width(), paint.stroke_width()));
    Point::new(size.x.abs(), size.y.abs())
}

// Port of: src/core/SkDraw.cpp#L678-L689 (chrome/m156)
fn easy_rect_join(rect: &Rect, paint: &Paint, matrix: &Matrix, stroke_size: &mut Point) -> bool {
    if rect.is_empty() || Join::Miter != paint.stroke_join() || paint.stroke_miter() < SCALAR_SQRT2
    {
        return false;
    }

    *stroke_size = compute_stroke_size(paint, matrix);
    true
}

// Port of: src/core/SkDraw.cpp#L1017-L1019 (chrome/m156)
// (the lines of `modifyPaintForHairlines` precede it; see below)

/// Tricky idea: can we treat thin strokes as hairlines? If so, depending on how thin, we may
/// decide to modulate the paint's alpha to 'simulate' very think strokes, even though hairline is
/// always 1-pixel wide.
///
/// The motivation at the time was performance: hairlines draw faster than constructing the
/// inner/outer contours and filling that (as we do for normal stroking).
///
/// Questionable decision, since our hairline algorithm draws each segment of the path separately,
/// meaning a path that crosses itself can have blending artifacts. Note: this doesn't happen with
/// normal stroking, as the built inner/outer path never double-hits a pixel.
// Port of: src/core/SkDraw.cpp#L978-L1007 (chrome/m156)
fn modify_paint_for_hairlines(orig_paint: &Paint, matrix: &Matrix) -> Option<Paint> {
    if let Some(coverage) = draw_treat_as_hairline(orig_paint, matrix) {
        let bm = orig_paint.as_blend_mode();
        #[allow(clippy::float_cmp)] // mirrors `coverage == 1`
        if coverage == 1.0 {
            let mut paint = orig_paint.clone();
            paint.set_stroke_width(0.0);
            return Some(paint);
        } else if let Some(bm) = bm
            && supports_coverage_as_alpha(bm)
        {
            // this is the old technique, which we preserve for now so
            // we don't change previous results (testing)
            // the new way seems fine, its just (a tiny bit) different
            #[allow(clippy::cast_possible_truncation)] // mirrors `(int)(coverage * 256)`
            let scale = float_saturate2int(coverage * 256.0);
            // U8CPU: `getAlpha() * scale >> 8` fits a byte for 0 <= coverage <= 1
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            let new_alpha = ((i32::from(orig_paint.alpha()) * scale) >> 8) as u8;
            let mut paint = orig_paint.clone();
            paint.set_stroke_width(0.0);
            paint.set_alpha(new_alpha);
            return Some(paint);
        }
    }
    None
}

impl<'a> Draw<'a> {
    /// A draw into `dst` with `ctm` and the clip `rc`, choosing blitters with `SkBlitter::Choose`
    /// (the `Draw()` constructor plus the required fields).
    // Port of: src/core/SkDraw.cpp#L92 (chrome/m156)
    #[must_use]
    pub fn new(dst: Pixmap<'a>, ctm: &'a Matrix, rc: &'a RasterClip) -> Draw<'a> {
        Draw {
            dst,
            blitter_chooser: choose_default,
            ctm,
            rc,
            props: None,
        }
    }

    /// A copy of this draw (`Draw draw(*this)`) over the same pixels.
    #[must_use]
    pub fn reborrow(&mut self) -> Draw<'_> {
        Draw {
            dst: self.dst.reborrow_mut(),
            blitter_chooser: self.blitter_chooser,
            ctm: self.ctm,
            rc: self.rc,
            props: self.props,
        }
    }

    /// Draws `bitmap` mapped by the CTM and `prematrix` with `paint` (`drawBitmap`), with nearest
    /// sampling (`linear_filter` is `sampling.filter == kLinear`).
    ///
    /// skia-rust: only the sprite path is ported (the bitmap lands on integer device pixels,
    /// which is every layer restore without a transform). Anything else draws through an image
    /// shader (`make_paint_with_image_and_mips` and `SkImageShader`, Phase 3) and is a
    /// TODO(Phase 3): it draws nothing here.
    // Port of: src/core/SkDraw.cpp#L369-L443 (chrome/m156)
    #[doc(alias = "drawBitmap")]
    pub fn draw_bitmap(
        &mut self,
        bitmap: &Bitmap,
        prematrix: &Matrix,
        _dst_bounds: Option<&Rect>,
        linear_filter: bool,
        orig_paint: &Paint,
    ) {
        self.validate();

        // nothing to draw
        if self.rc.is_empty()
            || bitmap.width() == 0
            || bitmap.height() == 0
            || bitmap.color_type() == ColorType::Unknown
        {
            return;
        }

        let mut paint = Cow::Borrowed(orig_paint);
        if orig_paint.style() != Style::Fill {
            paint.to_mut().set_style(Style::Fill);
        }

        let matrix = Matrix::concat(self.ctm, prematrix);

        if clipped_out(&matrix, self.rc, bitmap.width(), bitmap.height()) {
            return;
        }

        if !color_type_is_alpha_only(bitmap.color_type())
            && treat_as_sprite(
                &matrix,
                bitmap.dimensions(),
                linear_filter,
                paint.is_anti_alias(),
            )
        {
            // It is safe to call lock pixels now, since we know the matrix is (more or less)
            // identity.
            let Some(pmap) = bitmap.peek_pixels() else {
                return;
            };
            let ix = float_round2int(matrix.translate_x());
            let iy = float_round2int(matrix.translate_y());
            if clip_handles_sprite(self.rc, ix, iy, &pmap) {
                let alloc = ArenaAlloc::new();
                let (w, h) = (pmap.width(), pmap.height());
                let blitter = choose_sprite(
                    self.dst.reborrow_mut(),
                    &paint,
                    pmap,
                    ix,
                    iy,
                    &alloc,
                    self.rc.clip_shader(),
                    false,
                );
                if let Some(mut blitter) = blitter {
                    fill_irect_clip(&IRect::from_xywh(ix, iy, w, h), self.rc, &mut *blitter);
                }
                // if !blitter, then we fall-through to the slower case
            }
        }

        // TODO(Phase 3): the slower case draws the bitmap's rect with an image shader.
    }

    /// Debug checks of the draw's state (`validate`).
    // Port of: src/core/SkDraw.cpp#L1086-L1097 (chrome/m156)
    pub fn validate(&self) {
        #[cfg(debug_assertions)]
        if self.dst.addr().is_some() {
            use skia_rust_core::rect::Contains;
            let cr = self.rc.bounds();
            let mut br = IRect::new_empty();
            br.set_wh(self.dst.width(), self.dst.height());
            debug_assert!(cr.is_empty() || br.contains(cr));
        }
    }

    /// Draws points, lines or a polyline with `paint`. If `device` is given the primitives that
    /// are not hairlines or squares are sent to it (`drawPoints`).
    // Port of: src/core/SkDraw.cpp#L289-L350 (chrome/m156)
    #[doc(alias = "drawPoints")]
    pub fn draw_points(
        &mut self,
        mode: PointMode,
        mut points: &[Point],
        paint: &Paint,
        device: Option<&mut dyn Device>,
    ) {
        // if we're in lines mode, force count to be even
        if PointMode::Lines == mode {
            points = &points[..points.len() & !1]; // force it to be even
        }

        self.validate();

        // nothing to draw
        if points.is_empty() || self.rc.is_empty() {
            return;
        }

        let rc = self.rc;
        let ctm = self.ctm;
        let rec = if device.is_none() {
            PtProcRec::init(mode, paint, ctm, rc)
        } else {
            None
        };
        if let Some(rec) = rec {
            // Can't easily get bounds of points so don't try.
            auto_blitter_choose(
                self,
                None,
                paint,
                &Rect::new_empty(),
                DrawCoverage::No,
                |blitter| {
                    let proc = rec.choose_proc();
                    // We use the wrapped blitter and BW region for an AA clip, as Skia's
                    // `chooseProc` sets `fClip` and swaps the blitter.
                    let mut wrapper;
                    let (clip, bltr): (&skia_rust_core::region::Region, &mut dyn Blitter) =
                        if rc.is_bw() {
                            (rc.bw_rgn(), blitter)
                        } else {
                            wrapper = AAClipBlitterWrapper::new(rc, blitter);
                            wrapper.parts()
                        };
                    let mut dev_pts = [Point::default(); MAX_DEV_PTS];
                    // we have to back up subsequent passes if we're in polygon mode
                    let backup = usize::from(PointMode::Polygon == mode);

                    let mut count = points.len();
                    let mut pts = points;
                    loop {
                        let n = count.min(MAX_DEV_PTS);
                        ctm.map_points(&mut dev_pts[..n], &pts[..n]);
                        if !dev_pts[..n].iter().all(|p| p.is_finite()) {
                            return;
                        }
                        proc(&rec, &dev_pts[..n], clip, &mut *bltr);
                        pts = &pts[n - backup..];
                        debug_assert!(count >= n);
                        count -= n;
                        if count > 0 {
                            count += backup;
                        }
                        if count == 0 {
                            break;
                        }
                    }
                },
            );
        } else {
            self.draw_device_points(mode, points, paint, device);
        }
    }

    /// Fills the clip with `paint` (`drawPaint`).
    // Port of: src/core/SkDraw.cpp#L631-L643 (chrome/m156)
    #[doc(alias = "drawPaint")]
    pub fn draw_paint(&mut self, paint: &Paint) {
        self.validate();

        if self.rc.is_empty() {
            return;
        }

        let mut dev_rect = IRect::new_empty();
        dev_rect.set_wh(self.dst.width(), self.dst.height());

        let rc = self.rc;
        auto_blitter_choose(
            self,
            None,
            paint,
            &Rect::from_irect(dev_rect),
            DrawCoverage::No,
            |blitter| fill_irect_clip(&dev_rect, rc, blitter),
        );
    }

    /// Classifies how to draw `rect` given the paint's style and stroke width and the matrix. If
    /// no special case is available, returns [`RectType::Path`]. Iff the result is
    /// [`RectType::Stroke`], `stroke_size` is set to the device width and height of the stroke
    /// (`ComputeRectType`).
    // Port of: src/core/SkDraw.cpp#L691-L710 (chrome/m156)
    #[doc(alias = "ComputeRectType")]
    #[must_use]
    pub fn compute_rect_type(
        rect: &Rect,
        paint: &Paint,
        matrix: &Matrix,
        stroke_size: &mut Point,
    ) -> RectType {
        let width = paint.stroke_width();
        #[allow(clippy::float_cmp)] // mirrors `0 == width`
        let zero_width = 0.0 == width;
        let mut style = paint.style();

        if Style::StrokeAndFill == style && zero_width {
            style = Style::Fill;
        }

        if paint.path_effect().is_some()
            || paint.mask_filter().is_some()
            || !matrix.rect_stays_rect()
            || Style::StrokeAndFill == style
        {
            return RectType::Path;
        }
        if Style::Fill == style {
            return RectType::Fill;
        }
        if zero_width {
            return RectType::Hair;
        }
        if easy_rect_join(rect, paint, matrix, stroke_size) {
            return RectType::Stroke;
        }
        RectType::Path
    }

    /// Draws a rectangle (`drawRect(rect, paint)`).
    // Port of: src/core/SkDraw.h#L103-L105 (chrome/m156)
    #[doc(alias = "drawRect")]
    pub fn draw_rect(&mut self, rect: &Rect, paint: &Paint) {
        self.draw_rect_with(rect, paint, None, None);
    }

    /// Draws a rectangle. If `paint_matrix` is given it is applied to the rect's paint (shader)
    /// only: the geometry is `post_paint_rect` instead of `pre_paint_rect`, mapped by the CTM
    /// alone (`drawRect(prePaintRect, paint, paintMatrix, postPaintRect)`).
    ///
    /// # Panics
    /// In debug builds if `paint_matrix` and `post_paint_rect` are not both given or both absent.
    // Port of: src/core/SkDraw.cpp#L712-L800 (chrome/m156)
    #[doc(alias = "drawRect")]
    pub fn draw_rect_with(
        &mut self,
        pre_paint_rect: &Rect,
        paint: &Paint,
        paint_matrix: Option<&Matrix>,
        post_paint_rect: Option<&Rect>,
    ) {
        self.validate();

        // nothing to draw
        if self.rc.is_empty() {
            return;
        }

        let mut matrix: Cow<'_, Matrix> = Cow::Borrowed(self.ctm);
        if let Some(pm) = paint_matrix {
            debug_assert!(post_paint_rect.is_some());
            matrix.to_mut().pre_concat(pm);
        } else {
            debug_assert!(post_paint_rect.is_none());
        }

        let mut stroke_size = Point::default();
        let rtype = Self::compute_rect_type(pre_paint_rect, paint, self.ctm, &mut stroke_size);

        if RectType::Path == rtype {
            draw_rect_as_path(self, pre_paint_rect, paint, &matrix);
            return;
        }

        let paint_rect = if paint_matrix.is_some() {
            post_paint_rect.expect("a post-paint rect with a paint matrix")
        } else {
            pre_paint_rect
        };
        // skip the paintMatrix when transforming the rect by the CTM
        let src_pts = [
            Point::new(paint_rect.left, paint_rect.top),
            Point::new(paint_rect.right, paint_rect.bottom),
        ];
        let mut dev_pts = [Point::default(); 2];
        self.ctm.map_points(&mut dev_pts, &src_pts);
        let mut dev_rect = Rect::new(dev_pts[0].x, dev_pts[0].y, dev_pts[1].x, dev_pts[1].y);
        dev_rect.sort();

        // look for the quick exit, before we build a blitter
        let mut bbox = dev_rect;
        if paint.style() != Style::Fill {
            // extra space for hairlines
            #[allow(clippy::float_cmp)] // mirrors `== 0`
            if paint.stroke_width() == 0.0 {
                bbox.outset((1.0, 1.0));
            } else {
                // For RectType::kStroke, strokeSize is already computed.
                let ssize = if RectType::Stroke == rtype {
                    stroke_size
                } else {
                    compute_stroke_size(paint, self.ctm)
                };
                bbox.outset((ssize.x / 2.0, ssize.y / 2.0));
            }
        }
        if path_priv::too_big_for_math(&bbox) {
            return;
        }

        if !rect_priv::fits_in_fixed_rect(&bbox) && rtype != RectType::Hair {
            draw_rect_as_path(self, pre_paint_rect, paint, &matrix);
            return;
        }

        let ir: IRect = bbox.round_out();
        if self.rc.quick_reject(&ir) {
            return;
        }

        let clip = self.rc;
        auto_blitter_choose(
            self,
            Some(&matrix),
            paint,
            &dev_rect,
            DrawCoverage::No,
            |blitter| {
                // we want to "fill" if we are kFill or kStrokeAndFill, since in the latter
                // case we are also hairline (if we've gotten to here), which devolves to
                // effectively just kFill
                match rtype {
                    RectType::Fill => {
                        if paint.is_anti_alias() {
                            anti_fill_rect_clip(&dev_rect, clip, blitter);
                        } else {
                            fill_rect_clip(&dev_rect, clip, blitter);
                        }
                    }
                    RectType::Stroke => {
                        if paint.is_anti_alias() {
                            anti_frame_rect_clip(&dev_rect, &stroke_size, clip, blitter);
                        } else {
                            frame_rect(&dev_rect, &stroke_size, clip, blitter);
                        }
                    }
                    RectType::Hair => {
                        if paint.is_anti_alias() {
                            anti_hair_rect(&dev_rect, clip, blitter);
                        } else {
                            hair_rect(&dev_rect, clip, blitter);
                        }
                    }
                    RectType::Path => unreachable!("bad rtype"),
                }
            },
        );
    }

    /// Draws an oval (`drawOval`).
    // Port of: src/core/SkDraw.cpp#L840-L848 (chrome/m156)
    #[doc(alias = "drawOval")]
    pub fn draw_oval(&mut self, oval: &Rect, paint: &Paint) {
        self.validate();

        if self.rc.is_empty() {
            return;
        }

        self.draw_path(&Path::oval(oval, None), paint, None);
    }

    /// Draws a round rect (`drawRRect`).
    // Port of: src/core/SkDraw.cpp#L850-L880 (chrome/m156)
    #[doc(alias = "drawRRect")]
    pub fn draw_rrect(&mut self, rrect: &RRect, paint: &Paint) {
        self.validate();

        if self.rc.is_empty() {
            return;
        }

        // TODO: Investigate optimizing these options. They are in the same
        // order as skcpu::Draw::drawPath, which handles each case. It may be
        // that there is no way to optimize for these using the SkRRect path.
        let go_to_draw_path = draw_treat_as_hairline(paint, self.ctm).is_some()
            || paint.path_effect().is_some()
            || paint.style() != Style::Fill;

        if !go_to_draw_path
            && paint.mask_filter().is_some()
            && self.draw_rrect_nine_patch(rrect, paint)
        {
            return;
        }

        // Now fall back to the default case of using a path.
        self.draw_path(&Path::rrect(rrect, None), paint, None);
    }

    /// Specialized draw for round rects that only draws if it is nine-patchable
    /// (`drawRRectNinePatch`).
    ///
    /// skia-rust: nine-patch blurs come from `SkMaskFilterBase::filterRects`/`filterRRect`,
    /// which belong to the mask filters of Phase 3, so nothing is drawn and this returns false
    /// (the caller then draws the path, as Skia does when the filter declines).
    // Port of: src/core/SkDraw.cpp#L882-L902 (chrome/m156)
    #[doc(alias = "drawRRectNinePatch")]
    pub fn draw_rrect_nine_patch(&mut self, rrect: &RRect, paint: &Paint) -> bool {
        debug_assert!(paint.mask_filter().is_some());

        if let Some(_rr) = rrect.transform(self.ctm) {
            // TODO(Phase 3): choose a blitter for `rrect.getBounds()` and call
            // `maskFilter->filterRects` (for `RRect::Type::Rect`) or `filterRRect`.
            let _ = RRectType::Rect;
        }
        false
    }

    /// Draws `src` with `paint` (`drawPath(path, paint, prePathMatrix)`). If `pre_path_matrix`
    /// is given it is applied before any stroking or other effects.
    // Port of: src/core/SkDraw.h#L113-L118 (chrome/m156)
    #[doc(alias = "drawPath")]
    pub fn draw_path(&mut self, path: &Path, paint: &Paint, pre_path_matrix: Option<&Matrix>) {
        self.draw_path_with(path, paint, pre_path_matrix, DrawCoverage::No, None);
    }

    /// Overwrites the target with the path's coverage (i.e. its mask). Will overwrite the entire
    /// device, so it need not be zero'd first. Only device A8 is supported right now
    /// (`drawPathCoverage`).
    // Port of: src/core/SkDraw.h#L125-L136 (chrome/m156)
    #[doc(alias = "drawPathCoverage")]
    pub fn draw_path_coverage(
        &mut self,
        src: &Path,
        paint: &Paint,
        custom_blitter: Option<&mut dyn Blitter>,
    ) {
        #[allow(clippy::float_cmp)] // mirrors `getStrokeWidth() == 0`
        let is_hairline = paint.style() == Style::Stroke && paint.stroke_width() == 0.0;
        self.draw_path_with(
            src,
            paint,
            None,
            if is_hairline {
                DrawCoverage::No
            } else {
                DrawCoverage::Yes
            },
            custom_blitter,
        );
    }

    /// Draws a path. `pre_path_matrix`, if any, applies before stroking and path effects;
    /// `custom_blitter` replaces the blitter the chooser would pick (the private
    /// `drawPath(path, paint, preMatrix, drawCoverage, customBlitter)`).
    // Port of: src/core/SkDraw.cpp#L1009-L1085 (chrome/m156)
    #[doc(alias = "drawPath")]
    pub fn draw_path_with(
        &mut self,
        orig_src_path: &Path,
        orig_paint: &Paint,
        pre_path_matrix: Option<&Matrix>,
        draw_coverage: DrawCoverage,
        custom_blitter: Option<&mut dyn Blitter>,
    ) {
        self.validate();

        // nothing to draw
        if self.rc.is_empty() {
            return;
        }

        let new_paint = modify_paint_for_hairlines(orig_paint, self.ctm);
        let paint = new_paint.as_ref().unwrap_or(orig_paint);

        let needs_fill_path = paint.path_effect().is_some() || paint.style() != Style::Fill;

        let mut builder = PathBuilder::new();
        let mut do_fill = true;
        let pdata: Option<std::sync::Arc<PathData>>;
        let pre_path_storage: Option<Path>;
        let raw: PathRaw<'_>;

        if needs_fill_path {
            let cull_rect = self.compute_conservative_local_clip_bounds();

            let mut path_ptr = orig_src_path;
            if let Some(pre) = pre_path_matrix {
                pre_path_storage = path_ptr.try_make_transform(pre);
                match pre_path_storage.as_ref() {
                    Some(p) => path_ptr = p,
                    None => return,
                }
            }
            do_fill = fill_path_with_paint(
                path_ptr,
                paint,
                &mut builder,
                cull_rect.as_ref(),
                self.ctm.clone(),
            );
            builder.transform(self.ctm);
            match path_priv::raw_builder(&builder, ResolveConvexity::Yes) {
                Some(r) => raw = r,
                None => return,
            }
        } else {
            let mut matrix = self.ctm.clone();
            if let Some(pre) = pre_path_matrix {
                matrix.pre_concat(pre);
            }

            if matrix.is_identity() {
                match path_priv::raw(orig_src_path, ResolveConvexity::Yes) {
                    Some(r) => raw = r,
                    None => return,
                }
            } else {
                let Some(r0) = path_priv::raw(orig_src_path, ResolveConvexity::No) else {
                    return;
                };
                pdata = PathData::make_transform_raw(&r0, &matrix);
                match pdata.as_ref() {
                    Some(pd) => {
                        raw = pd.raw(orig_src_path.fill_type(), ResolveConvexity::Yes);
                    }
                    None => return, // failed to create pdata
                }
            }
        }

        self.draw_dev_path(&raw, paint, draw_coverage, custom_blitter, do_fill);
    }

    // Port of: src/core/SkDraw.cpp#L904-L976 (chrome/m156)
    fn draw_dev_path(
        &mut self,
        raw: &PathRaw<'_>,
        paint: &Paint,
        draw_coverage: DrawCoverage,
        custom_blitter: Option<&mut dyn Blitter>,
        do_fill: bool,
    ) {
        if path_priv::too_big_for_math(&raw.bounds()) {
            return;
        }

        let rc = self.rc;
        let run = |blitter: &mut dyn Blitter| {
            if paint.mask_filter().is_some() {
                // TODO(Phase 3): `SkMaskFilterBase::filterPath(raw, ctm, rc, blitter, style)`
                // draws the filtered mask and returns true; no mask filter can draw yet, so this
                // falls through to the unfiltered geometry.
            }

            if do_fill {
                if paint.is_anti_alias() {
                    anti_fill_path_clip(raw, rc, blitter);
                } else {
                    fill_path_clip(raw, rc, blitter);
                }
            } else {
                // hairline
                let proc: fn(&PathRaw<'_>, &RasterClip, &mut dyn Blitter) = if paint.is_anti_alias()
                {
                    match paint.stroke_cap() {
                        Cap::Butt => anti_hair_path,
                        Cap::Square => anti_hair_square_path,
                        Cap::Round => anti_hair_round_path,
                    }
                } else {
                    match paint.stroke_cap() {
                        Cap::Butt => hair_path,
                        Cap::Square => hair_square_path,
                        Cap::Round => hair_round_path,
                    }
                };
                proc(raw, rc, blitter);
            }
        };
        match custom_blitter {
            Some(blitter) => run(blitter),
            None => auto_blitter_choose(self, None, paint, &raw.bounds(), draw_coverage, run),
        }
    }

    /// Returns the current clip bounds, in local coordinates, with slop to account for
    /// antialiasing or hairlines (i.e. device-bounds outset by 1, and then run through the
    /// inverse of the matrix). `None` if the matrix cannot be inverted or the clip is empty
    /// (`computeConservativeLocalClipBounds`).
    // Port of: src/core/SkDraw.cpp#L614-L629 (chrome/m156)
    #[doc(alias = "computeConservativeLocalClipBounds")]
    #[must_use]
    pub fn compute_conservative_local_clip_bounds(&self) -> Option<Rect> {
        if self.rc.is_empty() {
            return None;
        }

        let inverse = self.ctm.invert()?;
        let mut dev_bounds = *self.rc.bounds();
        // outset to have slop for antialasing and hairlines
        dev_bounds.outset((1, 1));
        let (local_bounds, _) = inverse.map_rect(Rect::from_irect(dev_bounds));
        Some(local_bounds)
    }

    /// Draws a mask (an A8 or BW coverage image in device coordinates) with the paint's color,
    /// shader and blend, filtered by the paint's mask filter if it filters this mask
    /// (`drawDevMask`). `paint_matrix` replaces the CTM for choosing the blitter.
    // Port of: src/core/SkDraw.cpp#L495-L525 (chrome/m156)
    #[doc(alias = "drawDevMask")]
    pub fn draw_dev_mask(
        &mut self,
        src_m: &Mask<'_>,
        paint: &Paint,
        paint_matrix: Option<&Matrix>,
    ) {
        if src_m.bounds.is_empty() {
            return;
        }

        let mut dst_m = MaskBuilder::default();
        let filtered = paint
            .mask_filter()
            .is_some_and(|mf| mf.as_base().filter_mask(&mut dst_m, src_m, self.ctm, None));
        let mask: Mask<'_> = if filtered { dst_m.as_mask() } else { *src_m };

        let rc = self.rc;
        // `dstM.bounds()` even when the mask was not filtered (and so is still empty).
        let dev_bounds = Rect::from_irect(dst_m.bounds);
        auto_blitter_choose(
            self,
            paint_matrix,
            paint,
            &dev_bounds,
            DrawCoverage::No,
            |blitter| {
                if rc.is_bw() {
                    blitter.blit_mask_region(&mask, rc.bw_rgn());
                } else {
                    let mut wrapper = AAClipBlitterWrapper::new(rc, blitter);
                    let (clip_rgn, b) = wrapper.parts();
                    b.blit_mask_region(&mask, clip_rgn);
                }
            },
        );
    }

    // Port of: src/core/SkDraw.cpp#L1217-L1375 (chrome/m156)
    /// Draws points through `draw_path`/`draw_rect` (or the `device`'s) when the fast procs of
    /// [`draw_points`](Self::draw_points) do not apply (`drawDevicePoints`).
    #[doc(alias = "drawDevicePoints")]
    #[allow(clippy::too_many_lines)] // mirrors the C++ switch
    pub fn draw_device_points(
        &mut self,
        mode: PointMode,
        mut points: &[Point],
        paint: &Paint,
        mut device: Option<&mut dyn Device>,
    ) {
        // if we're in lines mode, force count to be even
        if PointMode::Lines == mode {
            points = &points[..points.len() & !1]; // force it to be even
        }

        self.validate();

        // nothing to draw
        if points.is_empty() || self.rc.is_empty() {
            return;
        }

        // needed?
        if !points.iter().all(|p| p.is_finite()) {
            return;
        }

        let mut fall_through_to_polygon = false;
        match mode {
            PointMode::Points => {
                // temporarily mark the paint as filling.
                let mut new_paint = paint.clone();
                new_paint.set_style(Style::Fill);

                let width = new_paint.stroke_width();
                let radius = width / 2.0;

                if new_paint.stroke_cap() == Cap::Round {
                    if let Some(device) = device.as_deref_mut() {
                        for pt in points {
                            let r = Rect::new(
                                pt.x - radius,
                                pt.y - radius,
                                pt.x + radius,
                                pt.y + radius,
                            );
                            device.draw_oval(&r, &new_paint);
                        }
                    } else {
                        let path = Path::circle((0.0, 0.0), radius, None);
                        let mut pre_matrix = Matrix::new_identity();

                        for pt in points {
                            pre_matrix.set_translate((pt.x, pt.y));
                            self.draw_path(&path, &new_paint, Some(&pre_matrix));
                        }
                    }
                } else {
                    let mut r = Rect::new_empty();

                    for pt in points {
                        r.left = pt.x - radius;
                        r.top = pt.y - radius;
                        r.right = r.left + width;
                        r.bottom = r.top + width;
                        if let Some(device) = device.as_deref_mut() {
                            device.draw_rect(&r, &new_paint);
                        } else {
                            self.draw_rect(&r, &new_paint);
                        }
                    }
                }
            }
            PointMode::Lines => {
                if 2 == points.len()
                    && let Some(pe) = paint.path_effect()
                {
                    // most likely a dashed line - see if it is one of the ones
                    // we can accelerate
                    let stroke = StrokeRec::from_paint(paint, None, None);
                    let mut point_data = PointData::default();

                    let path = Path::line(points[0], points[1]);

                    let cull_rect = Rect::from_irect(*self.rc.bounds());

                    if pe.as_points(&mut point_data, &path, &stroke, self.ctm, Some(&cull_rect)) {
                        // 'asPoints' managed to find some fast path

                        let mut new_p = paint.clone();
                        new_p.set_path_effect(None);
                        new_p.set_style(Style::Fill);

                        if !point_data.first.is_empty() {
                            if let Some(device) = device.as_deref_mut() {
                                device.draw_path(&point_data.first, &new_p);
                            } else {
                                self.draw_path(&point_data.first, &new_p, None);
                            }
                        }

                        if !point_data.last.is_empty() {
                            if let Some(device) = device.as_deref_mut() {
                                device.draw_path(&point_data.last, &new_p);
                            } else {
                                self.draw_path(&point_data.last, &new_p, None);
                            }
                        }

                        #[allow(clippy::float_cmp)] // mirrors `fSize.fX == fSize.fY`
                        if point_data.size.x == point_data.size.y {
                            // The rest of the dashed line can just be drawn as points
                            debug_assert_eq!(point_data.size.x, new_p.stroke_width() / 2.0);

                            if point_data
                                .flags
                                .contains(skia_rust_core::path_effect::PointFlags::CIRCLES)
                            {
                                new_p.set_stroke_cap(Cap::Round);
                            } else {
                                new_p.set_stroke_cap(Cap::Butt);
                            }

                            if let Some(device) = device.as_deref_mut() {
                                device.draw_points(PointMode::Points, &point_data.points, &new_p);
                            } else {
                                self.draw_device_points(
                                    PointMode::Points,
                                    &point_data.points,
                                    &new_p,
                                    None,
                                );
                            }
                        } else {
                            // The rest of the dashed line must be drawn as rects
                            debug_assert!(
                                !point_data
                                    .flags
                                    .contains(skia_rust_core::path_effect::PointFlags::CIRCLES)
                            );

                            let mut r = Rect::new_empty();

                            for pt in &point_data.points {
                                r.set_ltrb(
                                    pt.x - point_data.size.x,
                                    pt.y - point_data.size.y,
                                    pt.x + point_data.size.x,
                                    pt.y + point_data.size.y,
                                );
                                if let Some(device) = device.as_deref_mut() {
                                    device.draw_rect(&r, &new_p);
                                } else {
                                    self.draw_rect(&r, &new_p);
                                }
                            }
                        }

                        return;
                    }
                }
                fall_through_to_polygon = true; // couldn't take fast path
            }
            PointMode::Polygon => {
                fall_through_to_polygon = true;
            }
        }

        if fall_through_to_polygon {
            let count = points.len() - 1;
            let mut p = paint.clone();
            p.set_style(Style::Stroke);
            let inc = if PointMode::Lines == mode { 2 } else { 1 };

            let mut i = 0;
            while i < count {
                let path = Path::line(points[i], points[i + 1]);
                if let Some(device) = device.as_deref_mut() {
                    device.draw_path(&path, &p);
                } else {
                    self.draw_path(&path, &p, None);
                }
                i += inc;
            }
        }
    }
}

// Port of: src/core/SkDraw.cpp#L705-L715 (chrome/m156)
fn draw_rect_as_path(orig: &mut Draw<'_>, pre_paint_rect: &Rect, paint: &Paint, ctm: &Matrix) {
    let mut draw = orig.reborrow();
    draw.ctm = ctm;
    draw.draw_path(&Path::rect(pre_paint_rect, None), paint, None);
}

// ---------------------------------------------------------------------------------------------

/// How much room a mask filter needs around the clip (`kMaxMargin` of `compute_mask_bounds`).
const MAX_MARGIN: i32 = 128;

/// Computes the bounds of the mask of a path with the filter (`compute_mask_bounds`).
// Port of: src/core/SkDraw.cpp#L1099-L1137 (chrome/m156)
fn compute_mask_bounds(
    dev_path_bounds: &Rect,
    clip_bounds: &IRect,
    filter: &MaskFilter,
    filter_matrix: &Matrix,
    bounds: &mut IRect,
) -> bool {
    //  init our bounds from the path
    *bounds = dev_path_bounds
        .with_outset((SCALAR_HALF, SCALAR_HALF))
        .round_out();

    let mut margin = IPoint::new(0, 0);
    let src_m = Mask::new(&[], *bounds, 0, MaskFormat::A8);
    let mut dst_m = MaskBuilder::default();
    if !filter
        .as_base()
        .filter_mask(&mut dst_m, &src_m, filter_matrix, Some(&mut margin))
    {
        return false;
    }

    // trim the bounds to reflect the clip (plus whatever slop the filter needs)
    // Ugh. Guard against gigantic margins from wacky filters. Without this
    // check we can request arbitrary amounts of slop beyond our visible
    // clip, and bring down the renderer (at least on finite RAM machines
    // like handsets, etc.). Need to balance this invented value between
    // quality of large filters like blurs, and the corresponding memory
    // requests.
    let outset = clip_bounds.with_outset((margin.x.min(MAX_MARGIN), margin.y.min(MAX_MARGIN)));
    match IRect::intersect(bounds, &outset) {
        Some(r) => {
            *bounds = r;
            true
        }
        None => false,
    }
}

/// Rasterizes the device-space path `raw` into the A8 mask (`draw_into_mask`).
// Port of: src/core/SkDraw.cpp#L1139-L1185 (chrome/m156)
fn draw_into_mask(mask: &mut MaskBuilder, raw: &PathRaw<'_>, style: InitStyle) {
    let bounds = mask.bounds;
    let row_bytes = mask.row_bytes as usize;
    let info = ImageInfo::new_a8((bounds.width(), bounds.height()));
    let Some(dst) = Pixmap::new(&info, &mut mask.image, row_bytes) else {
        return;
    };

    #[allow(clippy::cast_precision_loss)] // mirrors the implicit int -> float conversion
    let (dx, dy) = (-bounds.left as scalar, -bounds.top as scalar);
    let translate = Matrix::translate((dx, dy));

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let Some(mut blitter) = choose_a8_blitter(dst, &translate, &paint, DrawCoverage::No, None)
    else {
        return;
    };

    // transform a copy of the points, so we can apply the ctm/translate
    let mut dev_points = vec![Point::default(); raw.points.len()];
    translate.map_points(&mut dev_points, raw.points);
    let mut raw = *raw;
    raw.points = &dev_points;
    raw.bounds = raw.bounds.with_offset((dx, dy));
    if !raw.bounds.is_finite() {
        return;
    }

    let clip = RasterClip::from_rect(&IRect::from_wh(bounds.width(), bounds.height()));

    match style {
        InitStyle::Hairline => anti_hair_path(&raw, &clip, &mut *blitter),
        InitStyle::Fill => anti_fill_path_clip(&raw, &clip, &mut *blitter),
    }
}

impl Draw<'_> {
    /// `skcpu::Draw::drawGlyphRunList`: draws `list` with `painter`, unless the clip is empty.
    /// The canvas of C++ is not passed: glyph paths draw on this draw, see [`BitmapDevicePainter`].
    // Port of: src/core/SkDraw_text.cpp#L125-L134 (chrome/m156)
    #[doc(alias = "drawGlyphRunList")]
    pub fn draw_glyph_run_list(
        &mut self,
        painter: &GlyphRunListPainter,
        list: &GlyphRunList<'_>,
        paint: &Paint,
    ) {
        self.validate();
        if self.rc.is_empty() {
            return;
        }
        let ctm = self.ctm;
        painter.draw_for_bitmap_device(self, list, paint, ctm);
    }
}

/// `skcpu::Draw` as the glyph painter sees it: masks through `paintMasks` and paths through the
/// draw itself.
impl BitmapDevicePainter for Draw<'_> {
    // Port of: src/core/SkDraw_text.cpp#L53-L123 (chrome/m156), Draw::paintMasks
    fn paint_masks(&mut self, accepted: &[(&Glyph, Point)], paint: &Paint) {
        let rc = self.rc;
        let use_region = rc.is_bw() && !rc.is_rect();
        auto_blitter_choose(
            self,
            None,
            paint,
            &Rect::default(),
            DrawCoverage::No,
            |blitter| {
                let mut wrapper = AAClipBlitterWrapper::new(rc, blitter);
                if use_region {
                    for (glyph, pos) in accepted {
                        if !check_glyph_position(*pos) {
                            continue;
                        }
                        let mask = glyph.mask_at(*pos);
                        let mut clipper = Cliperator::new(rc.bw_rgn(), mask.bounds);
                        if clipper.is_done() {
                            continue;
                        }
                        // TODO(text-T20): color masks are drawn with `Draw::drawSprite`, whose
                        // fallback needs the image-shader paint. Until then they draw nothing.
                        if mask.format == MaskFormat::Argb32 {
                            continue;
                        }
                        loop {
                            wrapper.blitter().blit_mask(&mask, clipper.rect());
                            clipper.next();
                            if clipper.is_done() {
                                break;
                            }
                        }
                    }
                } else {
                    let clip_bounds = if rc.is_bw() {
                        *rc.bw_rgn().bounds()
                    } else {
                        *rc.aa_rgn().bounds()
                    };
                    for (glyph, pos) in accepted {
                        if !check_glyph_position(*pos) {
                            continue;
                        }
                        let mask = glyph.mask_at(*pos);
                        // this extra test is worth it, assuming that most of the time it succeeds
                        // since we can avoid writing to storage
                        let bounds = if clip_bounds.contains_no_empty_check(&mask.bounds) {
                            mask.bounds
                        } else {
                            match IRect::intersect(&mask.bounds, &clip_bounds) {
                                Some(bounds) => bounds,
                                None => continue,
                            }
                        };
                        // TODO(text-T20): color masks need `Draw::drawSprite` (see above).
                        if mask.format == MaskFormat::Argb32 {
                            continue;
                        }
                        wrapper.blitter().blit_mask(&mask, &bounds);
                    }
                }
            },
        );
    }

    // Port of: src/core/SkCanvas.cpp#L2866-L2874 (chrome/m156), concat then drawPath
    fn draw_glyph_path_concat(&mut self, path: &Path, matrix: &Matrix, paint: &Paint) {
        // canvas->concat(m): the canvas matrix is an SkM44, so CTM * m is composed in 4x4 (float
        // order of SkM44::setConcat), and the device takes its 3x3 part (`asM33`).
        let ctm = M44::concat(&M44::from(self.ctm.clone()), &M44::from(matrix.clone())).to_m33();
        let mut draw = self.reborrow();
        draw.ctm = &ctm;
        draw.draw_path(path, paint, None);
    }

    fn draw_glyph_path_device(&mut self, path: &Path, paint: &Paint) {
        self.draw_path(path, paint, None);
    }
}

/// `SkDraw_text.cpp`'s `check_glyph_position`: glyphs whose position would straddle the int range
/// are not drawn. Written so that NaN is rejected.
// Port of: src/core/SkDraw_text.cpp#L28-L36 (chrome/m156)
// The negated comparisons are the point: a NaN fails them, as in C++.
#[allow(
    clippy::neg_cmp_op_on_partial_ord,
    clippy::cast_precision_loss // (float)int, as in C++
)]
fn check_glyph_position(position: Point) -> bool {
    // Comparisons written a little weirdly so that NaN coordinates are treated safely.
    let gt = |a: f32, b: i32| !(a <= b as f32);
    let lt = |a: f32, b: i32| !(a >= b as f32);
    !(gt(
        position.x,
        i32::MAX - (i32::from(i16::MAX) + i32::from(u16::MAX)),
    ) || lt(position.x, i32::MIN - i32::from(i16::MIN))
        || gt(
            position.y,
            i32::MAX - (i32::from(i16::MAX) + i32::from(u16::MAX)),
        )
        || lt(position.y, i32::MIN - i32::from(i16::MIN)))
}

/// Creates a mask from a device-space path and the mask filter that will filter it, to size the
/// mask. The resulting mask is not filtered; that is done afterwards (`filterMask`)
/// (`DrawToMask`).
// Port of: src/core/SkDraw.cpp#L1187-L1215 (chrome/m156)
#[doc(alias = "DrawToMask")]
pub fn draw_to_mask(
    dev_raw: &PathRaw<'_>,
    clip_bounds: &IRect,
    filter: &MaskFilter,
    filter_matrix: &Matrix,
    dst: &mut MaskBuilder,
    mode: CreateMode,
    style: InitStyle,
) -> bool {
    if dev_raw.is_empty() {
        return false;
    }

    if CreateMode::JustRenderImage != mode {
        // By using infinite bounds for inverse fills, compute_mask_bounds is able to clip it to
        // 'clipBounds' outset by whatever extra margin the mask filter requires.
        const INVERSE_BOUNDS: Rect = Rect::new(
            skia_rust_core::scalar::SCALAR_NEGATIVE_INFINITY,
            skia_rust_core::scalar::SCALAR_NEGATIVE_INFINITY,
            skia_rust_core::scalar::SCALAR_INFINITY,
            skia_rust_core::scalar::SCALAR_INFINITY,
        );
        let path_bounds = if dev_raw.is_inverse_fill_type() {
            INVERSE_BOUNDS
        } else {
            dev_raw.bounds()
        };
        if !compute_mask_bounds(
            &path_bounds,
            clip_bounds,
            filter,
            filter_matrix,
            &mut dst.bounds,
        ) {
            return false;
        }
    }

    if CreateMode::ComputeBoundsAndRenderImage == mode {
        dst.format = MaskFormat::A8;
        #[allow(clippy::cast_sign_loss)] // the bounds are non-empty, so the width is positive
        {
            dst.row_bytes = dst.bounds.width() as u32;
        }
        let size = dst.compute_image_size();
        if 0 == size {
            // we're too big to allocate the mask, abort
            return false;
        }
        dst.image = MaskBuilder::alloc_image(size, skia_rust_core::mask::AllocType::ZeroInit);
    }

    if CreateMode::JustComputeBounds != mode {
        draw_into_mask(dst, dev_raw, style);
    }
    true
}
