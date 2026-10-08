// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkConvertPixels.{h,cpp}, src/core/SkRectMemcpy.h

//! `SkConvertPixels`: converts a rectangle of pixels between color types, alpha types and color
//! spaces.
//!
//! The fast paths are tried in Skia's order (`rect_memcpy`, `swizzle_or_premul`,
//! `convert_to_alpha8`); everything else runs a raster pipeline
//! (`load, <SkColorSpaceXformSteps stages>, store`) on the current CPU tier.
//!
//! skia-rust: the C++ converts in place when `srcPixels == dstPixels`. A `&mut [u8]` and a
//! `&[u8]` cannot alias, so that case is [`convert_pixels_in_place`]: it makes the checks of the
//! C++ for it and converts from a copy of the pixels.

use crate::arena_alloc::ArenaAlloc;
use crate::color_data::packed4444_to_a32;
use crate::color_space_xform_steps::ColorSpaceXformSteps;
use crate::color_type::ColorType;
use crate::half::half_to_float;
use crate::image_info::ImageInfo;
use crate::image_info_priv::image_info_valid_conversion;
use crate::raster_pipeline::{MemSlot, MemView, MemoryBindings, MemoryCtx, RasterPipeline};
use crate::t_pin::t_pin;
use skia_rust_simd::{Tier, swizzle};

/// The fast paths' signature (`dstInfo, dstPixels, dstRB, srcInfo, srcPixels, srcRB, steps`).
type FastPath =
    fn(&ImageInfo, &mut [u8], usize, &ImageInfo, &[u8], usize, &ColorSpaceXformSteps) -> bool;

fn get_u16(bytes: &[u8], off: usize) -> u16 {
    u16::from_ne_bytes([bytes[off], bytes[off + 1]])
}

fn get_u32(bytes: &[u8], off: usize) -> u32 {
    u32::from_ne_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
}

fn get_u64(bytes: &[u8], off: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&bytes[off..off + 8]);
    u64::from_ne_bytes(b)
}

fn get_f32(bytes: &[u8], off: usize) -> f32 {
    f32::from_bits(get_u32(bytes, off))
}

// `int` dimensions of an image already checked by `convert_pixels` (non-negative).
#[allow(clippy::cast_sign_loss)] // mirrors the C++ loops over non-negative ints
fn dims(info: &ImageInfo) -> (usize, usize) {
    (info.width().max(0) as usize, info.height().max(0) as usize)
}

/// `SkRectMemcpy(dst, dstRB, src, srcRB, trimRowBytes, rowCount)`.
// Port of: src/core/SkRectMemcpy.h#L16-L30 (chrome/m156)
fn rect_memcpy_rows(
    dst: &mut [u8],
    dst_rb: usize,
    src: &[u8],
    src_rb: usize,
    trim_row_bytes: usize,
    row_count: usize,
) {
    debug_assert!(trim_row_bytes <= dst_rb);
    debug_assert!(trim_row_bytes <= src_rb);
    if trim_row_bytes == dst_rb && trim_row_bytes == src_rb {
        let n = trim_row_bytes * row_count;
        dst[..n].copy_from_slice(&src[..n]);
        return;
    }

    for y in 0..row_count {
        let d = y * dst_rb;
        let s = y * src_rb;
        dst[d..d + trim_row_bytes].copy_from_slice(&src[s..s + trim_row_bytes]);
    }
}

// Port of: src/core/SkConvertPixels.cpp#L28-L45 (chrome/m156)
fn rect_memcpy(
    dst_info: &ImageInfo,
    dst_pixels: &mut [u8],
    dst_rb: usize,
    src_info: &ImageInfo,
    src_pixels: &[u8],
    src_rb: usize,
    steps: &ColorSpaceXformSteps,
) -> bool {
    // We can copy the pixels when no color type, alpha type, or color space changes.
    if dst_info.color_type() != src_info.color_type() {
        return false;
    }
    if dst_info.color_type() != ColorType::Alpha8 && steps.flags.mask() != 0b00000 {
        return false;
    }

    // (srcPixels != dstPixels: always true for a `&mut` and a `&` slice.)
    rect_memcpy_rows(
        dst_pixels,
        dst_rb,
        src_pixels,
        src_rb,
        dst_info.min_row_bytes(),
        dims(dst_info).1,
    );
    true
}

// Port of: src/core/SkConvertPixels.cpp#L47-L86 (chrome/m156)
fn swizzle_or_premul(
    dst_info: &ImageInfo,
    dst_pixels: &mut [u8],
    dst_rb: usize,
    src_info: &ImageInfo,
    src_pixels: &[u8],
    src_rb: usize,
    steps: &ColorSpaceXformSteps,
) -> bool {
    let is_8888 = |ct: ColorType| ct == ColorType::RGBA8888 || ct == ColorType::BGRA8888;
    // `#if !defined(SK_ARM_HAS_NEON) steps.fFlags.unpremul ||`: Skia's NEON builds unpremul
    // with the swizzler; the Neon tier stands for those builds (design §2.2).
    let arm_has_neon = skia_rust_simd::selection().tier == Tier::Neon;
    if !is_8888(dst_info.color_type())
        || !is_8888(src_info.color_type())
        || steps.flags.linearize
        || steps.flags.gamut_transform
        || (!arm_has_neon && steps.flags.unpremul)
        || steps.flags.encode
    {
        return false;
    }

    let swap_rb = dst_info.color_type() != src_info.color_type();

    let f: fn(&mut [u8], &[u8], usize) = if steps.flags.premul {
        if swap_rb {
            swizzle::rgba_to_bgra_premul
        } else {
            swizzle::rgba_to_rgba_premul
        }
    } else if steps.flags.unpremul {
        if swap_rb {
            swizzle::rgba_premul_to_bgra
        } else {
            swizzle::rgba_premul_to_rgba
        }
    } else {
        // If we're not swizzling, we ought to have used rect_memcpy().
        debug_assert!(swap_rb);
        swizzle::rgba_to_bgra
    };

    let (width, height) = dims(dst_info);
    for y in 0..height {
        f(
            &mut dst_pixels[y * dst_rb..],
            &src_pixels[y * src_rb..],
            width,
        );
    }
    true
}

// Port of: src/core/SkConvertPixels.cpp#L88-L248 (chrome/m156)
#[allow(clippy::too_many_lines)] // one arm per color type, as the C++ switch
#[allow(clippy::match_same_arms)] // one arm per color type, as the C++ switch
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
// mirrors the C++ narrowing conversions to uint8_t
fn convert_to_alpha8(
    dst_info: &ImageInfo,
    dst: &mut [u8],
    dst_rb: usize,
    src_info: &ImageInfo,
    src: &[u8],
    src_rb: usize,
    _steps: &ColorSpaceXformSteps,
) -> bool {
    if dst_info.color_type() != ColorType::Alpha8 {
        return false;
    }
    let (width, height) = dims(src_info);

    // Runs `f(src_row_offset, x)` for every pixel, storing the result at `dst[x]` of the row.
    let mut each = |f: &dyn Fn(usize, usize) -> u8| {
        for y in 0..height {
            let (d, s) = (y * dst_rb, y * src_rb);
            for x in 0..width {
                dst[d + x] = f(s, x);
            }
        }
    };

    match src_info.color_type() {
        ColorType::Unknown | ColorType::Alpha8 => {
            // Unknown should never happen.
            // Alpha8 should have been handled by rect_memcpy().
            debug_assert!(false);
            return false;
        }

        ColorType::A16UNorm => each(&|s, x| (get_u16(src, s + 2 * x) >> 8) as u8),

        ColorType::Gray8
        | ColorType::RGB565
        | ColorType::R8G8UNorm
        | ColorType::R16UNorm
        | ColorType::R16Float
        | ColorType::R16G16UNorm
        | ColorType::R16G16Float
        | ColorType::RGB888x
        | ColorType::RGB101010x
        | ColorType::BGR101010x
        | ColorType::BGR101010xXR
        | ColorType::RGBF16F16F16x
        | ColorType::R8UNorm => {
            for y in 0..height {
                dst[y * dst_rb..y * dst_rb + width].fill(0xFF);
            }
        }

        ColorType::ARGB4444 => {
            each(&|s, x| packed4444_to_a32(u32::from(get_u16(src, s + 2 * x))) as u8);
        }

        ColorType::BGRA8888 | ColorType::RGBA8888 | ColorType::SRGBA8888 => {
            each(&|s, x| (get_u32(src, s + 4 * x) >> 24) as u8);
        }

        ColorType::RGBA1010102 | ColorType::BGRA1010102 => {
            each(&|s, x| ((get_u32(src, s + 4 * x) >> 30) * 0x55) as u8);
        }

        ColorType::RGBAF16Norm | ColorType::RGBAF16 => {
            each(&|s, x| (255.0f32 * half_to_float((get_u64(src, s + 8 * x) >> 48) as u16)) as u8);
        }

        ColorType::RGBAF32 => each(&|s, x| (255.0f32 * get_f32(src, s + 16 * x + 12)) as u8),

        ColorType::A16Float => {
            each(&|s, x| (255.0f32 * half_to_float(get_u16(src, s + 2 * x))) as u8);
        }

        ColorType::BGRA10101010XR => {
            each(&|s, x| {
                const ZERO: i64 = 384;
                const RANGE: i64 = 510;
                const MAX_U8: i64 = 0xff;
                const MIN_U8: i64 = 0x00;
                const DIVISOR: i64 = RANGE / MAX_U8;
                let raw_alpha = (get_u64(src, s + 8 * x) >> 54).cast_signed();
                // f(384) = 0
                // f(894) = 255
                let alpha = t_pin((raw_alpha - ZERO) / DIVISOR, MIN_U8, MAX_U8);
                alpha as u8
            });
        }
        ColorType::RGBA10x6 | ColorType::R16G16B16A16UNorm => {
            each(&|s, x| ((get_u64(src, s + 8 * x) >> 48) >> 8) as u8);
        }
    }
    true
}

// Default: Use the pipeline.
// Port of: src/core/SkConvertPixels.cpp#L250-L262 (chrome/m156)
#[allow(clippy::too_many_arguments)] // the C++ signature
fn convert_with_pipeline(
    dst_info: &ImageInfo,
    dst_row: &mut [u8],
    dst_stride: i32,
    src_info: &ImageInfo,
    src_row: &[u8],
    src_stride: i32,
    steps: &ColorSpaceXformSteps,
) {
    let src = MemoryCtx::new(MemSlot(0));
    let dst = MemoryCtx::new(MemSlot(1));

    let alloc = ArenaAlloc::new();
    let mut pipeline = RasterPipeline::new();
    pipeline.append_load(src_info.color_type(), src);
    steps.apply_to_pipeline(&mut pipeline, &alloc);
    pipeline.append_store(dst_info.color_type(), dst);

    let (width, height) = dims(src_info);
    let mut mem = MemoryBindings::new()
        .with(
            MemSlot(0),
            MemView::read(src_row).with_stride(src_stride as isize),
        )
        .with(
            MemSlot(1),
            MemView::write(dst_row).with_stride(dst_stride as isize),
        );
    pipeline.run(0, 0, width, height, &mut mem);
}

/// Converts the `dst_info.dimensions()` pixels of `src_pixels` (described by `src_info`, rows
/// `src_rb` bytes apart) to `dst_info`'s color type, alpha type and color space, into
/// `dst_pixels` (rows `dst_rb` bytes apart).
///
/// Returns false if a row-byte count is not a multiple of its color type's pixel size.
///
/// skia-rust: also returns false if either color type is [`ColorType::Unknown`] (a division by
/// zero in C++, which asserts `SkImageInfoValidConversion`) or if either slice is too small for
/// its info and row bytes (the C++ trusts the caller).
// Port of: src/core/SkConvertPixels.cpp#L264-L292 (chrome/m156)
#[doc(alias = "SkConvertPixels")]
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
// mirrors (int)(srcRB / srcInfo.bytesPerPixel())
#[allow(clippy::cast_sign_loss)] // mirrors (size_t)srcStride
pub fn convert_pixels(
    dst_info: &ImageInfo,
    dst_pixels: &mut [u8],
    dst_rb: usize,
    src_info: &ImageInfo,
    src_pixels: &[u8],
    src_rb: usize,
) -> bool {
    debug_assert_eq!(dst_info.dimensions(), src_info.dimensions());
    debug_assert!(image_info_valid_conversion(dst_info, src_info));

    let (src_bpp, dst_bpp) = (src_info.bytes_per_pixel(), dst_info.bytes_per_pixel());
    if src_bpp == 0 || dst_bpp == 0 {
        return false;
    }
    let src_stride = (src_rb / src_bpp) as i32;
    let dst_stride = (dst_rb / dst_bpp) as i32;
    if (src_stride as usize).wrapping_mul(src_bpp) != src_rb
        || (dst_stride as usize).wrapping_mul(dst_bpp) != dst_rb
    {
        return false;
    }

    if src_pixels.len() < src_info.compute_byte_size(src_rb)
        || dst_pixels.len() < dst_info.compute_byte_size(dst_rb)
    {
        return false;
    }

    let steps = ColorSpaceXformSteps::new(
        src_info.color_space().as_ref(),
        src_info.alpha_type(),
        dst_info.color_space().as_ref(),
        dst_info.alpha_type(),
    );

    for f in [
        rect_memcpy as FastPath,
        swizzle_or_premul,
        convert_to_alpha8,
    ] {
        if f(
            dst_info, dst_pixels, dst_rb, src_info, src_pixels, src_rb, &steps,
        ) {
            return true;
        }
    }
    convert_with_pipeline(
        dst_info, dst_pixels, dst_stride, src_info, src_pixels, src_stride, &steps,
    );
    true
}

/// [`convert_pixels`] with `srcPixels == dstPixels`: converts `pixels` (rows `src_rb` bytes apart,
/// as `src_info`) to `dst_info` (rows `dst_rb` bytes apart) in place. Returns false if the pixel
/// widths differ ("In-place conversions are not supported for different pixel widths") or
/// [`convert_pixels`] would.
// Port of: src/core/SkConvertPixels.cpp#L264-L292 (chrome/m156)
#[doc(alias = "SkConvertPixels")]
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
// mirrors (int)(srcRB / srcInfo.bytesPerPixel())
#[allow(clippy::cast_sign_loss)] // mirrors (size_t)srcStride
pub fn convert_pixels_in_place(
    dst_info: &ImageInfo,
    dst_rb: usize,
    src_info: &ImageInfo,
    src_rb: usize,
    pixels: &mut [u8],
) -> bool {
    let (src_bpp, dst_bpp) = (src_info.bytes_per_pixel(), dst_info.bytes_per_pixel());
    if src_bpp == 0 || dst_bpp == 0 {
        return false;
    }
    let src_stride = (src_rb / src_bpp) as i32;
    let dst_stride = (dst_rb / dst_bpp) as i32;
    if (src_stride as usize).wrapping_mul(src_bpp) != src_rb
        || (dst_stride as usize).wrapping_mul(dst_bpp) != dst_rb
    {
        return false;
    }

    if src_bpp != dst_bpp {
        // In-place conversions are not supported for different pixel widths.
        return false;
    }

    let src = pixels.to_vec();
    convert_pixels(dst_info, pixels, dst_rb, src_info, &src, src_rb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alpha_type::AlphaType;

    fn info(ct: ColorType, at: AlphaType) -> ImageInfo {
        ImageInfo::new((3, 2), ct, at, None)
    }

    #[test]
    fn rejects_row_bytes_that_are_not_whole_pixels() {
        let src = info(ColorType::RGBA8888, AlphaType::Premul);
        let dst = info(ColorType::RGB565, AlphaType::Opaque);
        let s = [0u8; 64];
        let mut d = [0u8; 64];
        assert!(!convert_pixels(&dst, &mut d, 7, &src, &s, 12));
        assert!(!convert_pixels(&dst, &mut d, 6, &src, &s, 13));
        assert!(convert_pixels(&dst, &mut d, 6, &src, &s, 12));
        // skia-rust: too-small slices are rejected instead of overrun.
        assert!(!convert_pixels(&dst, &mut d[..11], 6, &src, &s, 12));
        assert!(!convert_pixels(&dst, &mut d, 6, &src, &s[..23], 12));
    }

    #[test]
    fn fast_paths() {
        let src_info = info(ColorType::RGBA8888, AlphaType::Premul);
        let mut src = [0u8; 2 * 16];
        for (i, b) in src.iter_mut().enumerate() {
            #[allow(clippy::cast_possible_truncation)] // i < 32
            {
                *b = i as u8 * 7;
            }
        }
        // rect_memcpy, with padded source rows.
        let mut dst = [0u8; 24];
        assert!(convert_pixels(&src_info, &mut dst, 12, &src_info, &src, 16));
        assert_eq!(dst[..12], src[..12]);
        assert_eq!(dst[12..], src[16..28]);
        // swizzle_or_premul: swap R and B.
        let bgra = info(ColorType::BGRA8888, AlphaType::Premul);
        assert!(convert_pixels(&bgra, &mut dst, 12, &src_info, &src, 16));
        assert_eq!(dst[..4], [src[2], src[1], src[0], src[3]]);
        // convert_to_alpha8.
        let a8 = info(ColorType::Alpha8, AlphaType::Premul);
        let mut alpha = [0u8; 8];
        assert!(convert_pixels(&a8, &mut alpha, 4, &src_info, &src, 16));
        assert_eq!(
            alpha,
            [src[3], src[7], src[11], 0, src[19], src[23], src[27], 0]
        );
    }

    #[test]
    fn pipeline_565() {
        let src_info = ImageInfo::new((1, 1), ColorType::RGBA8888, AlphaType::Opaque, None);
        let dst_info = ImageInfo::new((1, 1), ColorType::RGB565, AlphaType::Opaque, None);
        let src = [0xFF, 0x80, 0x00, 0xFF];
        let mut dst = [0u8; 2];
        assert!(convert_pixels(&dst_info, &mut dst, 2, &src_info, &src, 4));
        // r = 31, g = round(0x80/255 * 63) = 32, b = 0.
        assert_eq!(u16::from_ne_bytes(dst), (31 << 11) | (32 << 5));
    }
}
