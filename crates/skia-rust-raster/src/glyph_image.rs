// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkScalerContext.cpp (GenerateImageFromPath and its helpers,
// #L316-L697)

//! Glyph masks made from paths: draw the glyph path into an A8 pixmap with the CPU draw, then
//! pack the result into the glyph's format (BW, A8, LCD16). [`GLYPH_PATH_RASTERIZER`] is the
//! implementation of core's [`GlyphPathRasterizer`] seam.

// Glyph extents are small, so the C++ int/float/u8 casts are exact in range.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_lossless
)]

use skia_rust_core::arena_alloc::ArenaAlloc;
use skia_rust_core::color_data::pack888_to_rgb16;
use skia_rust_core::draw_types::DrawCoverage;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask::{MaskBuilder, MaskFormat};
use skia_rust_core::mask_gamma::MaskPreBlend;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::scalar::scalar;
use skia_rust_core::scaler_context::GlyphPathRasterizer;
use skia_rust_core::shader::Shader;
use skia_rust_core::stroke_rec::StrokeRec;
use skia_rust_core::surface_props::SurfaceProps;

use crate::blitter::Blitter;
use crate::blitter_a8::a8_blitter_choose;
use crate::draw::Draw;
use crate::raster_clip::RasterClip;

/// `SkScalerContext::GenerateImageFromPath`'s samples per LCD pixel (`samplesPerPixel`, the
/// `SK_USE_LCD_TEXT_3X_FILTER`-off value).
// Port of: src/core/SkScalerContext.cpp#L624-L626 (chrome/m156)
const SAMPLES_PER_PIXEL: i32 = 4;

/// The FIR coefficients of the LCD filter, one row per subpixel (red, green, blue), each with
/// `SAMPLES_PER_PIXEL * 3` taps.
// Port of: src/core/SkScalerContext.cpp#L380-L389 (chrome/m156)
const LCD_COEFFICIENTS: [[i32; 12]; 3] = [
    // The red subpixel is centered inside the first sample (at 1/6 pixel), and is shifted.
    [
        0x03, 0x0b, 0x1c, 0x33, 0x40, 0x39, 0x24, 0x10, 0x05, 0x01, 0x00, 0x00,
    ],
    // The green subpixel is centered between two samples (at 1/2 pixel), so is symmetric.
    [
        0x00, 0x02, 0x08, 0x16, 0x2b, 0x3d, 0x3d, 0x2b, 0x16, 0x08, 0x02, 0x00,
    ],
    // The blue subpixel is centered inside the last sample (at 5/6 pixel), and is shifted.
    [
        0x00, 0x00, 0x01, 0x05, 0x10, 0x24, 0x39, 0x40, 0x33, 0x1c, 0x0b, 0x03,
    ],
];

/// The rasterizer of glyph images from paths. A unit struct: it has no state.
// Port of: src/core/SkScalerContext.cpp#L586-L697 (chrome/m156)
#[derive(Debug)]
pub struct RasterGlyphPathRasterizer;

/// The [`GlyphPathRasterizer`] every raster-backed scaler context uses.
pub static GLYPH_PATH_RASTERIZER: RasterGlyphPathRasterizer = RasterGlyphPathRasterizer;

impl GlyphPathRasterizer for RasterGlyphPathRasterizer {
    /// `SkScalerContext::GenerateImageFromPath`.
    // Port of: src/core/SkScalerContext.cpp#L586-L697 (chrome/m156)
    #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)] // mirrors the C++ signature
    fn generate_image_from_path(
        &self,
        dst: &mut MaskBuilder,
        path: &Path,
        pre_blend: &MaskPreBlend,
        do_bgr: bool,
        vertical_lcd: bool,
        a8_from_lcd: bool,
        hairline: bool,
    ) {
        generate_image_from_path(
            dst,
            path,
            pre_blend,
            do_bgr,
            vertical_lcd,
            a8_from_lcd,
            hairline,
        );
    }
}

/// `SkScalerContext::GenerateImageFromPath`.
// Port of: src/core/SkScalerContext.cpp#L586-L697 (chrome/m156)
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)] // mirrors the C++ signature
#[allow(clippy::too_many_lines)] // mirrors the C++ function, one block per format
fn generate_image_from_path(
    dst_mask: &mut MaskBuilder,
    path: &Path,
    mask_pre_blend: &MaskPreBlend,
    do_bgr: bool,
    vertical_lcd: bool,
    a8_from_lcd: bool,
    hairline: bool,
) {
    debug_assert!(matches!(
        dst_mask.format,
        MaskFormat::BW | MaskFormat::A8 | MaskFormat::Lcd16
    ));
    let mut paint = Paint::default();
    let mut stroke_path: Option<Path> = None;

    let src_w = dst_mask.bounds.width();
    let src_h = dst_mask.bounds.height();
    let mut dst_w = src_w;
    let mut dst_h = src_h;
    let mut matrix = Matrix::translate((
        -(dst_mask.bounds.left as scalar),
        -(dst_mask.bounds.top as scalar),
    ));

    paint.set_stroke(hairline);
    paint.set_anti_alias(dst_mask.format != MaskFormat::BW);

    let from_lcd =
        dst_mask.format == MaskFormat::Lcd16 || (dst_mask.format == MaskFormat::A8 && a8_from_lcd);
    let intermediate_dst = from_lcd || dst_mask.format == MaskFormat::BW;

    if from_lcd {
        let samples_per_pixel = SAMPLES_PER_PIXEL;
        if vertical_lcd {
            dst_w = samples_per_pixel * dst_h - 2 * samples_per_pixel;
            dst_h = src_w;
            matrix = Matrix::new_all(
                0.0,
                samples_per_pixel as scalar,
                -((dst_mask.bounds.top + 1) as scalar) * samples_per_pixel as scalar,
                1.0,
                0.0,
                -(dst_mask.bounds.left as scalar),
                0.0,
                0.0,
                1.0,
            );
        } else {
            dst_w = samples_per_pixel * dst_w - 2 * samples_per_pixel;
            matrix = Matrix::new_all(
                samples_per_pixel as scalar,
                0.0,
                -((dst_mask.bounds.left + 1) as scalar) * samples_per_pixel as scalar,
                0.0,
                1.0,
                -(dst_mask.bounds.top as scalar),
                0.0,
                0.0,
                1.0,
            );
        }
        // LCD hairline doesn't line up with the pixels, so do it the expensive way.
        let mut rec = StrokeRec::new_fill();
        if hairline {
            rec.set_stroke_style(1.0, false);
            rec.set_stroke_params(Cap::Butt, Join::Round, 0.0);
        }
        if rec.need_to_apply() {
            let mut builder = PathBuilder::new();
            if rec.apply_to_path(&mut builder, path) {
                stroke_path = Some(builder.detach());
                paint.set_style(Style::Fill);
            }
        }
    }
    let path_to_use: &Path = stroke_path.as_ref().unwrap_or(path);

    let mut clip = RasterClip::new();
    let _ = clip.set_rect(&IRect::from_wh(dst_w, dst_h));
    let info = ImageInfo::new_a8((dst_w, dst_h));
    let a8_row_bytes = usize::try_from(dst_w.max(0)).unwrap_or(0);
    let a8_rows = usize::try_from(dst_h.max(0)).unwrap_or(0);

    // sk_bzero(dstMask.image(), dstMask.computeImageSize()): the mask starts clear.
    dst_mask.image.iter_mut().for_each(|b| *b = 0);

    if intermediate_dst {
        // The A8 pixels live in a buffer of their own, then are packed into the mask.
        let mut intermediate = vec![0u8; a8_row_bytes * a8_rows];
        {
            // `dst.tryAlloc(info)` fails for an invalid size, which the LCD formula gives for a
            // narrow mask (a negative width): the mask stays empty and nothing is drawn.
            let Some(pixmap) = Pixmap::new(&info, intermediate.as_mut_slice(), a8_row_bytes) else {
                return;
            };
            draw_glyph_path(pixmap, &matrix, &clip, path_to_use, &paint);
        }
        match dst_mask.format {
            MaskFormat::BW => pack_a8_to_a1(dst_mask, &intermediate, a8_row_bytes),
            MaskFormat::A8 => {
                // from_lcd is set here: the LCD filter, to A8.
                pack_4x_h_to_mask(
                    &intermediate,
                    a8_row_bytes,
                    dst_w,
                    dst_h,
                    dst_mask,
                    mask_pre_blend,
                    do_bgr,
                    vertical_lcd,
                );
            }
            MaskFormat::Lcd16 => {
                pack_4x_h_to_mask(
                    &intermediate,
                    a8_row_bytes,
                    dst_w,
                    dst_h,
                    dst_mask,
                    mask_pre_blend,
                    do_bgr,
                    vertical_lcd,
                );
            }
            _ => unreachable!("glyph masks from paths are BW, A8 or LCD16"),
        }
    } else {
        // An A8 mask drawn in place, then the gamma table applied to it.
        let mask_row_bytes = dst_mask.row_bytes as usize;
        let pixmap = Pixmap::new(&info, dst_mask.image.as_mut_slice(), mask_row_bytes)
            .expect("the mask image is a valid A8 pixmap");
        draw_glyph_path(pixmap, &matrix, &clip, path_to_use, &paint);
        if let (true, Some(lut)) = (mask_pre_blend.is_applicable(), mask_pre_blend.g()) {
            apply_lut_to_a8_mask(dst_mask, lut);
        }
    }
}

/// The CPU draw of a glyph path into an A8 pixmap (`skcpu::Draw` with `SkA8Blitter_Choose`).
// Port of: src/core/SkScalerContext.cpp#L655-L670 (chrome/m156)
fn draw_glyph_path(
    pixmap: Pixmap<'_>,
    matrix: &Matrix,
    clip: &RasterClip,
    path: &Path,
    paint: &Paint,
) {
    let mut draw = Draw {
        dst: pixmap,
        blitter_chooser: choose_a8,
        ctm: matrix,
        rc: clip,
        props: None,
    };
    // We can save a copy if we had to use the local strokePath.
    draw.draw_path(path, paint, None);
}

/// The blitter chooser of the glyph draw: `SkA8Blitter_Choose`, which ignores the arena, the
/// surface properties and the device bounds. Public so that the GPU path atlases can rasterize
/// their A8 masks with the same blitter.
///
/// # Panics
/// Never, for a valid `dst`: `SkA8Blitter_Choose` always returns a blitter.
// Port of: src/core/SkBlitter_A8.cpp#L317-L324 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors SkBlitter's BlitterChooser signature
pub fn choose_a8<'b>(
    dst: Pixmap<'b>,
    ctm: &Matrix,
    paint: &Paint,
    _alloc: &'b ArenaAlloc,
    draw_coverage: DrawCoverage,
    clip_shader: Option<&Shader>,
    _props: &SurfaceProps,
    dev_bounds: &Rect,
) -> Box<dyn Blitter + 'b> {
    a8_blitter_choose(dst, ctm, paint, draw_coverage, clip_shader, dev_bounds)
        .expect("SkA8Blitter_Choose always returns a blitter")
}

/// `applyLUTToA8Mask`: maps every pixel of an A8 mask through the table.
// Port of: src/core/SkScalerContext.cpp#L316-L326 (chrome/m156)
fn apply_lut_to_a8_mask(mask: &mut MaskBuilder, lut: &[u8]) {
    let row_bytes = mask.row_bytes as usize;
    let width = mask.bounds.width().max(0) as usize;
    let height = mask.bounds.height().max(0) as usize;
    for y in 0..height {
        let row = &mut mask.image[y * row_bytes..y * row_bytes + width];
        for pixel in row.iter_mut() {
            *pixel = lut[usize::from(*pixel)];
        }
    }
}

/// `pack4xHToMask`: the LCD FIR filter over the 4x-wide A8 source, packed as LCD16 (or, for an
/// A8 destination, the average of the three subpixels).
// Port of: src/core/SkScalerContext.cpp#L328-L448 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
#[allow(clippy::too_many_lines)] // mirrors the C++ function
#[allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)] // C++ int/u8 mixing, values are in range
fn pack_4x_h_to_mask(
    src: &[u8],
    src_row_bytes: usize,
    src_w: i32,
    src_h: i32,
    dst: &mut MaskBuilder,
    pre_blend: &MaskPreBlend,
    do_bgr: bool,
    do_vert: bool,
) {
    let to_a8 = dst.format == MaskFormat::A8;
    debug_assert!(dst.format == MaskFormat::Lcd16 || to_a8);
    // doVert in this function means swap x and y when writing to dst.
    let sample_width = src_w;
    let height = src_h;
    let dst_rb = dst.row_bytes as usize;
    let dst_bpp: usize = if to_a8 { 1 } else { 2 };

    for y in 0..height as usize {
        // The start and step of this row's output pixels.
        let (dst_start, dst_delta) = if do_vert {
            (y * dst_bpp, dst_rb)
        } else {
            (y * dst_rb, dst_bpp)
        };
        let src_row = &src[y * src_row_bytes..];

        // TODO in C++: this FIR is straightforward but slow.
        let mut out_index = 0usize;
        let mut sample_x = -SAMPLES_PER_PIXEL;
        while sample_x < sample_width + SAMPLES_PER_PIXEL {
            let mut fir = [0i32; 3];
            let mut coeff_index = (0.max(sample_x - 4) - (sample_x - 4)) as usize;
            let mut sample_index = 0.max(sample_x - 4);
            while sample_index < (sample_x + 8).min(sample_width) {
                let sample_value = i32::from(src_row[sample_index as usize]);
                for (subpxl, fir_value) in fir.iter_mut().enumerate() {
                    *fir_value += LCD_COEFFICIENTS[subpxl][coeff_index] * sample_value;
                }
                sample_index += 1;
                coeff_index += 1;
            }
            for fir_value in &mut fir {
                *fir_value /= 0x100;
                *fir_value = (*fir_value).min(255);
            }
            let (mut r, mut g, mut b) = if do_bgr {
                (fir[2] as u8, fir[1] as u8, fir[0] as u8)
            } else {
                (fir[0] as u8, fir[1] as u8, fir[2] as u8)
            };

            let index = dst_start + out_index * dst_delta;
            if to_a8 {
                let mut a = (u32::from(r) + u32::from(g) + u32::from(b)) / 3;
                if let (true, Some(lut)) = (pre_blend.is_applicable(), pre_blend.g()) {
                    a = u32::from(lut[a as usize]);
                }
                // The average of three bytes is at most 255.
                dst.image[index] = u8::try_from(a).unwrap_or(u8::MAX);
            } else {
                if let (true, Some(lut_r), Some(lut_g), Some(lut_b)) = (
                    pre_blend.is_applicable(),
                    pre_blend.r(),
                    pre_blend.g(),
                    pre_blend.b(),
                ) {
                    r = lut_r[usize::from(r)];
                    g = lut_g[usize::from(g)];
                    b = lut_b[usize::from(b)];
                }
                let packed = pack888_to_rgb16(u32::from(r), u32::from(g), u32::from(b)) as u16;
                dst.image[index..index + 2].copy_from_slice(&packed.to_ne_bytes());
            }
            out_index += 1;
            sample_x += SAMPLES_PER_PIXEL;
        }
    }
}

/// `convert_8_to_1`: the high bit of an alpha byte.
// Port of: src/core/SkScalerContext.cpp#L526-L529 (chrome/m156)
fn convert_8_to_1(byte: u8) -> u32 {
    u32::from(byte >> 7)
}

/// `pack_8_to_1`: eight alpha bytes to one bit each, first byte most significant.
// Port of: src/core/SkScalerContext.cpp#L531-L538 (chrome/m156)
fn pack_8_to_1(alpha: &[u8]) -> u8 {
    let mut bits = 0u32;
    for &a in alpha.iter().take(8) {
        bits <<= 1;
        bits |= convert_8_to_1(a);
    }
    u8::try_from(bits).unwrap_or(u8::MAX)
}

/// `packA8ToA1`: packs an A8 mask into a BW mask, eight pixels per byte.
// Port of: src/core/SkScalerContext.cpp#L540-L570 (chrome/m156)
fn pack_a8_to_a1(dst_mask: &mut MaskBuilder, src: &[u8], src_row_bytes: usize) {
    let height = dst_mask.bounds.height().max(0) as usize;
    let width = dst_mask.bounds.width().max(0) as usize;
    let octs = width >> 3;
    let left_over_bits = width & 7;
    let dst_row_bytes = dst_mask.row_bytes as usize;
    debug_assert!(src_row_bytes >= width);
    for y in 0..height {
        let src_row = &src[y * src_row_bytes..];
        let dst_row = &mut dst_mask.image[y * dst_row_bytes..];
        let mut s = 0usize;
        let mut d = 0usize;
        for _ in 0..octs {
            dst_row[d] = pack_8_to_1(&src_row[s..s + 8]);
            s += 8;
            d += 1;
        }
        if left_over_bits > 0 {
            let mut bits = 0u32;
            let mut shift: i32 = 7;
            for _ in 0..left_over_bits {
                bits |= convert_8_to_1(src_row[s]) << shift;
                s += 1;
                shift -= 1;
            }
            dst_row[d] = bits as u8;
        }
    }
}
