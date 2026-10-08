// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkCodecPriv.h#L105-L213 (chrome/m156), SkCodec.cpp#L799-L837 (SelectXformFormat)
// Ported from: src/codec/SkCodecPriv.h (the helpers used so far; the rest arrive with the codecs
// that call them)

//! Small helpers shared by the codecs (`SkCodecPriv`).

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_skcms::PixelFormat;

/// Port of `SK_PMCOLOR_IS_RGBA`: true when the native 32-bit colour type is RGBA.
const PMCOLOR_IS_RGBA: bool = matches!(ColorType::N32, ColorType::RGBA8888);

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
        ColorType::RGBAF16Norm => Some(PixelFormat::RgbaHhhh),
        ColorType::RGBA1010102 => Some(PixelFormat::Rgba1010102),
        ColorType::BGR101010xXR => Some(PixelFormat::Bgr101010xXr),
        ColorType::Gray8 => Some(PixelFormat::G8),
        _ => None,
    }
}
