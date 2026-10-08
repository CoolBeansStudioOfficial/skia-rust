// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkMaskFilterBase.cpp

//! The drawing helpers of `SkMaskFilterBase` (friends of `skcpu::Draw`): [`filter_path`],
//! [`filter_rects`] and [`filter_rrect`] draw the mask a mask filter makes of a device-space
//! shape through a blitter, and [`DrawMaskRasterizer`] is the [`MaskRasterizer`] the nine-patch
//! filters draw their small masks with.
//!
//! skia-rust: these are free functions taking the filter handle; the `SkResourceCache*`
//! parameter is not ported (see `skia_rust_core::mask_filter`).

use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::draw_types::DrawCoverage;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask::{CreateMode, Mask, MaskBuilder, MaskFormat};
use skia_rust_core::mask_filter::{FilterReturn, MaskFilter, MaskRasterizer, NinePatch};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_priv;
use skia_rust_core::path_raw::PathRaw;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::{Contains, IRect, Rect};
use skia_rust_core::region::Cliperator;
use skia_rust_core::rrect::RRect;
use skia_rust_core::shader::Shader;
use skia_rust_core::stroke_rec::InitStyle;
use skia_rust_core::surface_props::SurfaceProps;

use crate::blitter::{Blitter, NullBlitter};
use crate::blitter_a8::a8_blitter_choose;
use crate::draw::{Draw, draw_to_mask};
use crate::raster_clip::{AAClipBlitterWrapper, RasterClip};

// Port of: src/core/SkMaskFilterBase.cpp#L56-L65 (chrome/m156)
fn extract_mask_subset<'a>(src: &Mask<'a>, mut bounds: IRect, new_x: i32, new_y: i32) -> Mask<'a> {
    debug_assert!(src.bounds.contains(&bounds));

    let dx = bounds.left - src.bounds.left;
    let dy = bounds.top - src.bounds.top;
    bounds.offset_to((new_x, new_y));
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    // the subset is inside the mask
    let offset = (i64::from(dy) * i64::from(src.row_bytes) + i64::from(dx)) as usize;
    Mask::new(&src.image[offset..], bounds, src.row_bytes, src.format)
}

// Port of: src/core/SkMaskFilterBase.cpp#L67-L73 (chrome/m156)
fn blit_clipped_mask(blitter: &mut dyn Blitter, mask: &Mask<'_>, bounds: &IRect, clip_r: &IRect) {
    if let Some(r) = IRect::intersect(bounds, clip_r) {
        blitter.blit_mask(mask, &r);
    }
}

// Port of: src/core/SkMaskFilterBase.cpp#L75-L80 (chrome/m156)
fn blit_clipped_rect(blitter: &mut dyn Blitter, rect: &IRect, clip_r: &IRect) {
    if let Some(r) = IRect::intersect(rect, clip_r) {
        blitter.blit_rect(r.left, r.top, r.width(), r.height());
    }
}

// Port of: src/core/SkMaskFilterBase.cpp#L82-L184 (chrome/m156)
#[allow(clippy::too_many_lines)] // mirrors the C++ function
fn draw_nine_clipped(
    mask: &Mask<'_>,
    outer_r: &IRect,
    center: IPoint,
    fill_center: bool,
    clip_r: &IRect,
    blitter: &mut dyn Blitter,
) {
    let cx = center.x;
    let cy = center.y;
    let mut bounds;

    // top-left
    bounds = mask.bounds;
    bounds.right = cx;
    bounds.bottom = cy;
    if bounds.width() > 0 && bounds.height() > 0 {
        let m = extract_mask_subset(mask, bounds, outer_r.left, outer_r.top);
        blit_clipped_mask(blitter, &m, &m.bounds, clip_r);
    }

    // top-right
    bounds = mask.bounds;
    bounds.left = cx + 1;
    bounds.bottom = cy;
    if bounds.width() > 0 && bounds.height() > 0 {
        let m = extract_mask_subset(mask, bounds, outer_r.right - bounds.width(), outer_r.top);
        blit_clipped_mask(blitter, &m, &m.bounds, clip_r);
    }

    // bottom-left
    bounds = mask.bounds;
    bounds.right = cx;
    bounds.top = cy + 1;
    if bounds.width() > 0 && bounds.height() > 0 {
        let m = extract_mask_subset(mask, bounds, outer_r.left, outer_r.bottom - bounds.height());
        blit_clipped_mask(blitter, &m, &m.bounds, clip_r);
    }

    // bottom-right
    bounds = mask.bounds;
    bounds.left = cx + 1;
    bounds.top = cy + 1;
    if bounds.width() > 0 && bounds.height() > 0 {
        let m = extract_mask_subset(
            mask,
            bounds,
            outer_r.right - bounds.width(),
            outer_r.bottom - bounds.height(),
        );
        blit_clipped_mask(blitter, &m, &m.bounds, clip_r);
    }

    let mut inner_r = IRect::default();
    inner_r.set_ltrb(
        outer_r.left + cx - mask.bounds.left,
        outer_r.top + cy - mask.bounds.top,
        outer_r.right + (cx + 1 - mask.bounds.right),
        outer_r.bottom + (cy + 1 - mask.bounds.bottom),
    );
    if fill_center {
        blit_clipped_rect(blitter, &inner_r, clip_r);
    }

    let inner_w = inner_r.width();
    // (innerW + 1) int16 runs and as many alphas
    let storage = usize::try_from(inner_w + 1).unwrap_or(0);
    let mut runs = vec![0i16; storage];
    let mut alpha = vec![0u8; storage];

    let mut r = IRect::default();
    // top
    r.set_ltrb(inner_r.left, outer_r.top, inner_r.right, inner_r.top);
    if let Some(rr) = IRect::intersect(&r, clip_r) {
        r = rr;
        let start_y = std::cmp::max(0, r.top - outer_r.top);
        let stop_y = start_y + r.height();
        let width = r.width();
        runs[0] = i16::try_from(width).expect("run fits");
        runs[usize::try_from(width).expect("positive")] = 0;
        for y in start_y..stop_y {
            alpha[0] = mask.get_addr8(cx, mask.bounds.top + y)[0];
            blitter.blit_anti_h(r.left, outer_r.top + y, &mut alpha, &mut runs);
        }
    }
    // bottom
    r.set_ltrb(inner_r.left, inner_r.bottom, inner_r.right, outer_r.bottom);
    if let Some(rr) = IRect::intersect(&r, clip_r) {
        r = rr;
        let start_y = outer_r.bottom - r.bottom;
        let stop_y = start_y + r.height();
        let width = r.width();
        runs[0] = i16::try_from(width).expect("run fits");
        runs[usize::try_from(width).expect("positive")] = 0;
        for y in start_y..stop_y {
            alpha[0] = mask.get_addr8(cx, mask.bounds.bottom - y - 1)[0];
            blitter.blit_anti_h(r.left, outer_r.bottom - y - 1, &mut alpha, &mut runs);
        }
    }
    // left
    r.set_ltrb(outer_r.left, inner_r.top, inner_r.left, inner_r.bottom);
    if let Some(rr) = IRect::intersect(&r, clip_r) {
        r = rr;
        let left_mask = Mask::new(
            mask.get_addr8(
                mask.bounds.left + r.left - outer_r.left,
                mask.bounds.top + cy,
            ),
            r,
            0, // so we repeat the scanline for our height
            MaskFormat::A8,
        );
        blitter.blit_mask(&left_mask, &r);
    }
    // right
    r.set_ltrb(inner_r.right, inner_r.top, outer_r.right, inner_r.bottom);
    if let Some(rr) = IRect::intersect(&r, clip_r) {
        r = rr;
        let right_mask = Mask::new(
            mask.get_addr8(
                mask.bounds.right - outer_r.right + r.left,
                mask.bounds.top + cy,
            ),
            r,
            0, // so we repeat the scanline for our height
            MaskFormat::A8,
        );
        blitter.blit_mask(&right_mask, &r);
    }
}

// Port of: src/core/SkMaskFilterBase.cpp#L186-L202 (chrome/m156)
fn draw_nine(
    mask: &Mask<'_>,
    outer_r: &IRect,
    center: IPoint,
    fill_center: bool,
    clip: &RasterClip,
    blitter: &mut dyn Blitter,
) {
    // if we get here, we need to (possibly) resolve the clip and blitter
    let mut wrapper = AAClipBlitterWrapper::new(clip, blitter);
    let (rgn, blitter) = wrapper.parts();

    for cr in Cliperator::new(rgn, outer_r) {
        draw_nine_clipped(mask, outer_r, center, fill_center, &cr, blitter);
    }
}

// Port of: src/core/SkMaskFilterBase.cpp#L204-L213 (chrome/m156)
fn count_nested_rects(raw: &PathRaw<'_>, rects: &mut [Rect; 2]) -> usize {
    if let Some((nested, _)) = path_priv::is_nested_fill_rects(raw) {
        *rects = nested;
        return 2;
    }
    if let Some(r) = raw.is_rect() {
        rects[0] = r;
        return 1;
    }
    0
}

/// Rasterizes the device-space round rect `dev_rrect` as a nine-patch (if the filter can) and
/// blits it. Returns false if the filter cannot, and nothing was drawn (`filterRRect`).
// Port of: src/core/SkMaskFilterBase.cpp#L215-L231 (chrome/m156)
#[doc(alias = "filterRRect")]
pub fn filter_rrect(
    filter: &MaskFilter,
    dev_rrect: &RRect,
    matrix: &Matrix,
    clip: &RasterClip,
    blitter: &mut dyn Blitter,
) -> bool {
    // Attempt to speed up drawing by creating a nine patch. If a nine patch
    // cannot be used, return false to allow our caller to recover and perform
    // the drawing another way.
    let Some(patch) = filter.as_base().filter_rrect_to_nine(
        dev_rrect,
        matrix,
        clip.bounds(),
        &DrawMaskRasterizer,
    ) else {
        return false;
    };
    draw_nine(
        &patch.mask.as_mask(),
        &patch.outer_rect,
        patch.center,
        true,
        clip,
        blitter,
    );
    true
}

/// Rasterizes the device-space rectangles (one, or two nested ones) as a nine-patch (if the
/// filter can) and blits it (`filterRects`).
///
/// # Panics
/// Never: a filter that returns `True` provides the patch.
// Port of: src/core/SkMaskFilterBase.cpp#L233-L260 (chrome/m156)
#[doc(alias = "filterRects")]
pub fn filter_rects(
    filter: &MaskFilter,
    dev_rects: &[Rect],
    matrix: &Matrix,
    clip: &RasterClip,
    blitter: &mut dyn Blitter,
) -> FilterReturn {
    let mut patch: Option<NinePatch> = None;

    let filter_return = filter.as_base().filter_rects_to_nine(
        dev_rects,
        matrix,
        clip.bounds(),
        &mut patch,
        &DrawMaskRasterizer,
    );
    match filter_return {
        FilterReturn::False => {
            debug_assert!(patch.is_none());
        }

        FilterReturn::True => {
            let patch = patch.expect("kTrue comes with a patch");
            draw_nine(
                &patch.mask.as_mask(),
                &patch.outer_rect,
                patch.center,
                1 == dev_rects.len(),
                clip,
                blitter,
            );
        }

        FilterReturn::Unimplemented => {
            debug_assert!(patch.is_none());
            // fall out
        }
    }
    filter_return
}

/// Rasterizes the device-space path `dev_raw` into an A8 mask, filters the mask, and blits it.
/// Returns false if the filter did not filter the mask, and nothing was drawn (`filterPath`).
// Port of: src/core/SkMaskFilterBase.cpp#L262-L324 (chrome/m156)
#[doc(alias = "filterPath")]
pub fn filter_path(
    filter: &MaskFilter,
    dev_raw: &PathRaw<'_>,
    matrix: &Matrix,
    clip: &RasterClip,
    blitter: &mut dyn Blitter,
    style: InitStyle,
) -> bool {
    let mut rects = [Rect::default(); 2];
    let mut rect_count = 0;
    if InitStyle::Fill == style {
        rect_count = count_nested_rects(dev_raw, &mut rects);
    }
    if rect_count > 0 {
        match filter_rects(filter, &rects[..rect_count], matrix, clip, blitter) {
            FilterReturn::False => return false,
            FilterReturn::True => return true,
            FilterReturn::Unimplemented => {}
        }
    }

    let mut src_m = MaskBuilder::default();
    let mut dst_m = MaskBuilder::default();

    if !draw_to_mask(
        dev_raw,
        clip.bounds(),
        filter,
        matrix,
        &mut src_m,
        CreateMode::ComputeBoundsAndRenderImage,
        style,
    ) {
        return false;
    }

    if !filter
        .as_base()
        .filter_mask(&mut dst_m, &src_m.as_mask(), matrix, None)
    {
        return false;
    }

    // if we get here, we need to (possibly) resolve the clip and blitter
    let mut wrapper = AAClipBlitterWrapper::new(clip, blitter);
    let (rgn, blitter) = wrapper.parts();

    let dst_mask = dst_m.as_mask();
    for cr in Cliperator::new(rgn, dst_m.bounds) {
        blitter.blit_mask(&dst_mask, &cr);
    }

    true
}

/// The A8 blitter chooser mask filters draw their small masks with (`SkA8Blitter_Choose` as a
/// [`crate::draw::BlitterChooser`]).
#[allow(clippy::too_many_arguments)] // Skia's BlitterChooser signature
fn choose_a8<'b>(
    dst: Pixmap<'b>,
    ctm: &Matrix,
    paint: &Paint,
    _alloc: &'b ArenaAlloc,
    draw_coverage: DrawCoverage,
    clip_shader: Option<&Shader>,
    _props: &SurfaceProps,
    dev_bounds: &Rect,
) -> Box<dyn Blitter + 'b> {
    match a8_blitter_choose(dst, ctm, paint, draw_coverage, clip_shader, dev_bounds) {
        Some(b) => b,
        None => Box::new(NullBlitter::default()),
    }
}

/// Draws the small masks of the nine-patch filters with [`Draw`] into A8 pixels
/// (`draw_into_mask` of `SkBlurMaskFilterImpl.cpp`).
#[derive(Copy, Clone, Debug, Default)]
pub struct DrawMaskRasterizer;

// Port of: src/core/SkBlurMaskFilterImpl.cpp#L172-L186 (chrome/m156)
fn draw_into_mask(mask: &mut MaskBuilder, proc: impl FnOnce(&mut Draw<'_>, &Paint)) {
    let dx = mask.bounds.left;
    let dy = mask.bounds.top;
    let width = mask.bounds.width();
    let height = mask.bounds.height();
    let rclip = RasterClip::from_rect(&IRect::from_wh(width, height));

    debug_assert_eq!(mask.format, MaskFormat::A8);
    let info = ImageInfo::new_a8((width, height));
    let row_bytes = mask.row_bytes as usize;
    let pm = Pixmap::new(&info, &mut mask.image, row_bytes).expect("a prepared A8 mask");

    #[allow(clippy::cast_precision_loss)] // mirrors SkIntToScalar
    let ctm = Matrix::translate((-(dx as f32), -(dy as f32)));

    let mut draw = Draw::new(pm, &ctm, &rclip);
    draw.blitter_chooser = choose_a8;

    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    proc(&mut draw, &paint);
}

impl MaskRasterizer for DrawMaskRasterizer {
    // Port of: src/core/SkBlurMaskFilterImpl.cpp#L188-L202 (chrome/m156)
    fn draw_rects(&self, mask: &mut MaskBuilder, rects: &[Rect]) {
        debug_assert!(rects.len() == 1 || rects.len() == 2);
        draw_into_mask(mask, |draw, paint| {
            if rects.len() == 1 {
                draw.draw_rect(&rects[0], paint);
            } else {
                // todo: do I need a fast way to do this?
                let path: Path = PathBuilder::new()
                    .add_rect(rects[0], None, None)
                    .add_rect(rects[1], None, None)
                    .set_fill_type(PathFillType::EvenOdd)
                    .detach();
                draw.draw_path(&path, paint, None);
            }
        });
    }

    // Port of: src/core/SkBlurMaskFilterImpl.cpp#L204-L208 (chrome/m156)
    fn draw_rrect(&self, mask: &mut MaskBuilder, rrect: &RRect) {
        draw_into_mask(mask, |draw, paint| {
            draw.draw_rrect(rrect, paint);
        });
    }
}
