// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkImageInfoPriv.h

//! Private helpers for [`ColorType`], [`ColorInfo`] and [`ImageInfo`] (`SkImageInfoPriv.h`).

use crate::alpha_type::AlphaType;
use crate::color::ColorChannelFlag;
use crate::color_type::ColorType;
use crate::image_info::{ColorInfo, ImageInfo};

/// The channels stored by `ct`, as a [`ColorChannelFlag`] mask (`0` for [`ColorType::Unknown`]).
// Port of: src/core/SkImageInfoPriv.h#L16-L45 (chrome/m156)
#[doc(alias = "SkColorTypeChannelFlags")]
#[must_use]
#[allow(clippy::match_same_arms)] // one arm per color type, as the C++ switch
pub fn color_type_channel_flags(ct: ColorType) -> ColorChannelFlag {
    // skia-rust: ColorChannelFlag has no zero constant; `ALPHA & RED` is 0.
    match ct {
        ColorType::Unknown => ColorChannelFlag::ALPHA & ColorChannelFlag::RED,
        ColorType::Alpha8 => ColorChannelFlag::ALPHA,
        ColorType::RGB565 => ColorChannelFlag::RGB,
        ColorType::ARGB4444 => ColorChannelFlag::RGBA,
        ColorType::RGBA8888 => ColorChannelFlag::RGBA,
        ColorType::RGB888x => ColorChannelFlag::RGB,
        ColorType::BGRA8888 => ColorChannelFlag::RGBA,
        ColorType::RGBA1010102 => ColorChannelFlag::RGBA,
        ColorType::RGB101010x => ColorChannelFlag::RGB,
        ColorType::BGRA1010102 => ColorChannelFlag::RGBA,
        ColorType::BGR101010x => ColorChannelFlag::RGB,
        ColorType::BGR101010xXR => ColorChannelFlag::RGB,
        ColorType::BGRA10101010XR => ColorChannelFlag::RGBA,
        ColorType::RGBA10x6 => ColorChannelFlag::RGBA,
        ColorType::Gray8 => ColorChannelFlag::GRAY,
        ColorType::RGBAF16Norm => ColorChannelFlag::RGBA,
        ColorType::RGBAF16 => ColorChannelFlag::RGBA,
        ColorType::RGBF16F16F16x => ColorChannelFlag::RGB,
        ColorType::RGBAF32 => ColorChannelFlag::RGBA,
        ColorType::R8G8UNorm => ColorChannelFlag::RG,
        ColorType::A16UNorm => ColorChannelFlag::ALPHA,
        ColorType::R16UNorm => ColorChannelFlag::RED,
        ColorType::R16G16UNorm => ColorChannelFlag::RG,
        ColorType::A16Float => ColorChannelFlag::ALPHA,
        ColorType::R16Float => ColorChannelFlag::RED,
        ColorType::R16G16Float => ColorChannelFlag::RG,
        ColorType::R16G16B16A16UNorm => ColorChannelFlag::RGBA,
        ColorType::SRGBA8888 => ColorChannelFlag::RGBA,
        ColorType::R8UNorm => ColorChannelFlag::RED,
    }
}

/// The number of channels stored by `ct`.
// Port of: src/core/SkImageInfoPriv.h#L47-L61 (chrome/m156)
#[doc(alias = "SkColorTypeNumChannels")]
#[must_use]
pub fn color_type_num_channels(ct: ColorType) -> usize {
    let flags = color_type_channel_flags(ct);
    if flags == ColorChannelFlag::RED
        || flags == ColorChannelFlag::ALPHA
        || flags == ColorChannelFlag::GRAY
    {
        1
    } else if flags == ColorChannelFlag::GRAY_ALPHA || flags == ColorChannelFlag::RG {
        2
    } else if flags == ColorChannelFlag::RGB {
        3
    } else if flags == ColorChannelFlag::RGBA {
        4
    } else if flags.bits() == 0 {
        0
    } else {
        debug_assert!(false, "unexpected color channel flags");
        0
    }
}

/// True if `ct` stores only an alpha channel.
// Port of: src/core/SkImageInfoPriv.h#L63-L65 (chrome/m156)
#[doc(alias = "SkColorTypeIsAlphaOnly")]
#[must_use]
pub fn color_type_is_alpha_only(ct: ColorType) -> bool {
    color_type_channel_flags(ct) == ColorChannelFlag::ALPHA
}

/// True if `value` is a valid alpha type.
// Port of: src/core/SkImageInfoPriv.h#L67-L69 (chrome/m156)
#[doc(alias = "SkAlphaTypeIsValid")]
#[must_use]
pub fn alpha_type_is_valid(value: u32) -> bool {
    value <= AlphaType::LAST_ENUM as u32
}

/// The left shift that converts a pixel count to a byte count for `ct`.
// Port of: src/core/SkImageInfoPriv.h#L71-L102 (chrome/m156)
#[doc(alias = "SkColorTypeShiftPerPixel")]
#[must_use]
#[allow(clippy::match_same_arms)] // one arm per color type, as the C++ switch
pub fn color_type_shift_per_pixel(ct: ColorType) -> usize {
    match ct {
        ColorType::Unknown => 0,
        ColorType::Alpha8 => 0,
        ColorType::RGB565 => 1,
        ColorType::ARGB4444 => 1,
        ColorType::RGBA8888 => 2,
        ColorType::RGB888x => 2,
        ColorType::BGRA8888 => 2,
        ColorType::RGBA1010102 => 2,
        ColorType::RGB101010x => 2,
        ColorType::BGRA1010102 => 2,
        ColorType::BGR101010x => 2,
        ColorType::BGR101010xXR => 2,
        ColorType::BGRA10101010XR => 3,
        ColorType::RGBA10x6 => 3,
        ColorType::Gray8 => 0,
        ColorType::RGBAF16Norm => 3,
        ColorType::RGBAF16 => 3,
        ColorType::RGBF16F16F16x => 3,
        ColorType::RGBAF32 => 4,
        ColorType::R8G8UNorm => 1,
        ColorType::A16UNorm => 1,
        ColorType::R16UNorm => 1,
        ColorType::R16G16UNorm => 2,
        ColorType::A16Float => 1,
        ColorType::R16Float => 1,
        ColorType::R16G16Float => 2,
        ColorType::R16G16B16A16UNorm => 3,
        ColorType::SRGBA8888 => 2,
        ColorType::R8UNorm => 0,
    }
}

/// The minimum row bytes for `width` pixels of `ct`.
// Port of: src/core/SkImageInfoPriv.h#L104-L106 (chrome/m156)
#[doc(alias = "SkColorTypeMinRowBytes")]
#[must_use]
#[allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]
// mirrors (size_t)(width * SkColorTypeBytesPerPixel(ct)); callers pass width >= 0
pub fn color_type_min_row_bytes(ct: ColorType, width: i32) -> usize {
    (width.wrapping_mul(ct.bytes_per_pixel() as i32)) as usize
}

/// True if `value` is a valid [`ColorType`] value.
// Port of: src/core/SkImageInfoPriv.h#L108-L110 (chrome/m156)
#[doc(alias = "SkColorTypeIsValid")]
#[must_use]
pub fn color_type_is_valid(value: u32) -> bool {
    value <= ColorType::LAST_ENUM as u32
}

/// The byte offset of pixel (`x`, `y`) in a buffer of `ct` pixels with `row_bytes` per row.
// Port of: src/core/SkImageInfoPriv.h#L112-L120 (chrome/m156)
#[doc(alias = "SkColorTypeComputeOffset")]
#[must_use]
#[allow(clippy::cast_sign_loss)] // mirrors (size_t)y / (size_t)x; both are asserted non-negative
pub fn color_type_compute_offset(ct: ColorType, x: i32, y: i32, row_bytes: usize) -> usize {
    debug_assert!(x >= 0);
    debug_assert!(y >= 0);
    if ColorType::Unknown == ct {
        return 0;
    }
    (y as usize)
        .wrapping_mul(row_bytes)
        .wrapping_add((x as usize) << color_type_shift_per_pixel(ct))
}

/// True if the channels of `ct` are normalized to `[0, 1]`.
// Port of: src/core/SkImageInfoPriv.h#L122-L156 (chrome/m156)
#[doc(alias = "SkColorTypeIsNormalized")]
#[must_use]
#[allow(clippy::match_same_arms)] // one arm per color type, as the C++ switch
pub fn color_type_is_normalized(ct: ColorType) -> bool {
    match ct {
        ColorType::Unknown
        | ColorType::Alpha8
        | ColorType::RGB565
        | ColorType::ARGB4444
        | ColorType::RGBA8888
        | ColorType::RGB888x
        | ColorType::BGRA8888
        | ColorType::RGBA1010102
        | ColorType::RGB101010x
        | ColorType::BGRA1010102
        | ColorType::BGR101010x
        | ColorType::RGBA10x6
        | ColorType::Gray8
        | ColorType::RGBAF16Norm
        | ColorType::R8G8UNorm
        | ColorType::A16UNorm
        | ColorType::A16Float // subtle... alpha is always [0,1]
        | ColorType::R16UNorm
        | ColorType::R16G16UNorm
        | ColorType::R16G16B16A16UNorm
        | ColorType::SRGBA8888
        | ColorType::R8UNorm => true,

        ColorType::BGRA10101010XR
        | ColorType::BGR101010xXR
        | ColorType::RGBF16F16F16x
        | ColorType::RGBAF16
        | ColorType::RGBAF32
        | ColorType::R16Float
        | ColorType::R16G16Float => false,
    }
}

/// The maximum number of bits in any channel of `ct`.
// Port of: src/core/SkImageInfoPriv.h#L158-L204 (chrome/m156)
#[doc(alias = "SkColorTypeMaxBitsPerChannel")]
#[must_use]
#[allow(clippy::match_same_arms)] // one arm per color type, as the C++ switch
pub fn color_type_max_bits_per_channel(ct: ColorType) -> usize {
    match ct {
        ColorType::Unknown => 0,

        ColorType::ARGB4444 => 4,

        ColorType::RGB565 => 6,

        ColorType::Alpha8
        | ColorType::RGBA8888
        | ColorType::RGB888x
        | ColorType::BGRA8888
        | ColorType::Gray8
        | ColorType::R8G8UNorm
        | ColorType::SRGBA8888
        | ColorType::R8UNorm => 8,

        ColorType::RGBA1010102
        | ColorType::RGB101010x
        | ColorType::BGRA1010102
        | ColorType::BGR101010x
        | ColorType::BGR101010xXR
        | ColorType::BGRA10101010XR
        | ColorType::RGBA10x6 => 10,

        ColorType::RGBAF16Norm
        | ColorType::A16UNorm
        | ColorType::A16Float
        | ColorType::R16UNorm
        | ColorType::R16Float
        | ColorType::R16G16UNorm
        | ColorType::R16G16B16A16UNorm
        | ColorType::RGBAF16
        | ColorType::RGBF16F16F16x
        | ColorType::R16G16Float => 16,

        ColorType::RGBAF32 => 32,
    }
}

/// Returns true if `info` contains a valid color type and alpha type.
// Port of: src/core/SkImageInfoPriv.h#L206-L211 (chrome/m156)
#[doc(alias = "SkColorInfoIsValid")]
#[must_use]
pub fn color_info_is_valid(info: &ColorInfo) -> bool {
    info.color_type() != ColorType::Unknown && info.alpha_type() != AlphaType::Unknown
}

const MAX_DIMENSION: i32 = i32::MAX >> 2;

/// Returns true if `info` contains a valid combination of width, height and color info.
// Port of: src/core/SkImageInfoPriv.h#L213-L228 (chrome/m156)
#[doc(alias = "SkImageInfoIsValid")]
#[must_use]
pub fn image_info_is_valid(info: &ImageInfo) -> bool {
    if info.width() <= 0 || info.height() <= 0 {
        return false;
    }

    if info.width() > MAX_DIMENSION || info.height() > MAX_DIMENSION {
        return false;
    }

    color_info_is_valid(info.color_info())
}

/// Returns true if Skia has defined a pixel conversion from `src` to `dst`. Returns false
/// otherwise.
// Port of: src/core/SkImageInfoPriv.h#L230-L238 (chrome/m156)
#[doc(alias = "SkImageInfoValidConversion")]
#[must_use]
pub fn image_info_valid_conversion(dst: &ImageInfo, src: &ImageInfo) -> bool {
    image_info_is_valid(dst) && image_info_is_valid(src)
}
