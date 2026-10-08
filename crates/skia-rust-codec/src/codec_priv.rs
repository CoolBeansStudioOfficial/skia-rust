// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkCodecPriv.h#L105-L213 (chrome/m156), SkCodec.cpp#L799-L837 (SelectXformFormat)
// Ported from: src/codec/SkCodecPriv.h (the helpers used so far; the rest arrive with the codecs
// that call them)

//! Small helpers shared by the codecs (`SkCodecPriv`).

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::math::mul_div_255_round;
use skia_rust_skcms::PixelFormat;

/// Port of `SK_PMCOLOR_IS_RGBA`: true when the native 32-bit colour type is RGBA.
pub(crate) const PMCOLOR_IS_RGBA: bool = matches!(ColorType::N32, ColorType::RGBA8888);

// Port of SkColorData.h's SK_{R,G,B,A}32_SHIFT for the native N32 layout.
const R32_SHIFT: u32 = if PMCOLOR_IS_RGBA { 0 } else { 16 };
const G32_SHIFT: u32 = 8;
const B32_SHIFT: u32 = if PMCOLOR_IS_RGBA { 16 } else { 0 };
const A32_SHIFT: u32 = 24;

/// Port of `SkPackARGB32`: packs into the native N32 layout.
// Port of: include/core/SkColorPriv.h (SkPackARGB32) via src/core/SkColorData.h (chrome/m156)
#[must_use]
pub fn pack_argb32(a: u32, r: u32, g: u32, b: u32) -> u32 {
    (a << A32_SHIFT) | (r << R32_SHIFT) | (g << G32_SHIFT) | (b << B32_SHIFT)
}

/// Port of `SkGetPackedR32`.
#[must_use]
pub fn get_packed_r32(c: u32) -> u32 {
    (c >> R32_SHIFT) & 0xFF
}

/// Port of `SkGetPackedG32`.
#[must_use]
pub fn get_packed_g32(c: u32) -> u32 {
    (c >> G32_SHIFT) & 0xFF
}

/// Port of `SkGetPackedB32`.
#[must_use]
pub fn get_packed_b32(c: u32) -> u32 {
    (c >> B32_SHIFT) & 0xFF
}

/// Port of `SkPackARGB_as_RGBA`: red in the low byte.
// Port of: include/core/SkColorPriv.h (SkPackARGB_as_RGBA) (chrome/m156)
#[must_use]
pub fn pack_argb_as_rgba(a: u32, r: u32, g: u32, b: u32) -> u32 {
    (a << 24) | (b << 16) | (g << 8) | r
}

/// Port of `SkPackARGB_as_BGRA`: blue in the low byte.
// Port of: include/core/SkColorPriv.h (SkPackARGB_as_BGRA) (chrome/m156)
#[must_use]
pub fn pack_argb_as_bgra(a: u32, r: u32, g: u32, b: u32) -> u32 {
    (a << 24) | (r << 16) | (g << 8) | b
}

/// Port of `SkPack888ToRGB16`.
// Port of: src/core/SkColorData.h#L330-L334 (chrome/m156)
#[must_use]
#[allow(clippy::cast_possible_truncation)] // each channel is at most 0x1F or 0x3F after the shift, so the sum fits in 16 bits
pub fn pack888_to_rgb16(r: u32, g: u32, b: u32) -> u16 {
    (((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)) as u16
}

/// Port of `SkPixel32ToPixel16`: a native N32 pixel to 5-6-5.
// Port of: src/core/SkColorData.h (SkPixel32ToPixel16) (chrome/m156)
#[must_use]
pub fn pixel32_to_pixel16(c: u32) -> u16 {
    pack888_to_rgb16(get_packed_r32(c), get_packed_g32(c), get_packed_b32(c))
}

/// Port of `SkCodecPriv::PremultiplyARGBasRGBA`.
// Port of: src/codec/SkCodecPriv.h#L294-L303 (chrome/m156)
#[must_use]
pub fn premultiply_argb_as_rgba(a: u32, mut r: u32, mut g: u32, mut b: u32) -> u32 {
    if a != 255 {
        r = mul_div_255_round(r, a);
        g = mul_div_255_round(g, a);
        b = mul_div_255_round(b, a);
    }
    pack_argb_as_rgba(a, r, g, b)
}

/// Port of `SkCodecPriv::PremultiplyARGBasBGRA`.
// Port of: src/codec/SkCodecPriv.h#L305-L313 (chrome/m156)
#[must_use]
pub fn premultiply_argb_as_bgra(a: u32, mut r: u32, mut g: u32, mut b: u32) -> u32 {
    if a != 255 {
        r = mul_div_255_round(r, a);
        g = mul_div_255_round(g, a);
        b = mul_div_255_round(b, a);
    }
    pack_argb_as_bgra(a, r, g, b)
}

/// Port of `SkCodecPriv::GetSampledDimension`.
// Port of: src/codec/SkCodecPriv.h#L127-L135 (chrome/m156)
#[must_use]
pub fn get_sampled_dimension(src_dimension: i32, sample_size: i32) -> i32 {
    if sample_size > src_dimension {
        return 1;
    }
    if sample_size == 0 {
        return 0;
    }
    src_dimension / sample_size
}

/// Port of `SkCodecPriv::GetStartCoord`.
// Port of: src/codec/SkCodecPriv.h#L143 (chrome/m156)
#[must_use]
pub fn get_start_coord(sample_factor: i32) -> i32 {
    sample_factor / 2
}

/// Port of `SkCodecPriv::ValidAlpha`. Skia also prints a performance warning for an opaque image
/// decoded as non-opaque; that diagnostic is not reproduced.
// Port of: src/codec/SkCodecPriv.h#L176-L191 (chrome/m156)
#[must_use]
pub fn valid_alpha(dst_alpha: AlphaType, src_is_opaque: bool) -> bool {
    if dst_alpha == AlphaType::Unknown {
        return false;
    }
    if src_is_opaque {
        return true;
    }
    dst_alpha != AlphaType::Opaque
}

/// Port of `SkCodecPriv::SelectXformFormat`: the skcms pixel format for a destination colour type.
// Port of: src/codec/SkCodec.cpp#L799-L838 (chrome/m156)
#[must_use]
pub fn select_xform_format(color_type: ColorType, for_color_table: bool) -> Option<PixelFormat> {
    match color_type {
        ColorType::RGBA8888 => Some(PixelFormat::Rgba8888),
        ColorType::BGRA8888 => Some(PixelFormat::Bgra8888),
        ColorType::RGB565 => {
            if for_color_table {
                // Mirrors `#if defined(SK_PMCOLOR_IS_RGBA)`: the native N32 order decides.
                if PMCOLOR_IS_RGBA {
                    Some(PixelFormat::Rgba8888)
                } else {
                    Some(PixelFormat::Bgra8888)
                }
            } else {
                Some(PixelFormat::Bgr565)
            }
        }
        ColorType::RGBAF16 => Some(PixelFormat::RgbaHhhh),
        ColorType::RGBA1010102 => Some(PixelFormat::Rgba1010102),
        ColorType::BGR101010xXR => Some(PixelFormat::Bgr101010xXr),
        ColorType::Gray8 => Some(PixelFormat::G8),
        _ => None,
    }
}
