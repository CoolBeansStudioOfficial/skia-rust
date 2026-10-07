// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkColorType.h, include/core/SkImageInfo.h (the color type
// functions), src/core/SkImageInfo.cpp

//! [`ColorType`]: how pixel bits encode color.

use crate::alpha_type::AlphaType;
use crate::color::ColorChannelFlag;
use crate::color_priv::PMCOLOR_IS_BGRA;
use crate::image_info_priv::color_type_channel_flags;

/// Describes how pixel bits encode color. A pixel may be an alpha mask, a grayscale, RGB, or ARGB.
///
/// [`ColorType::N32`] selects the native 32-bit ARGB format for the current configuration. This
/// can lead to inconsistent results across platforms, so use with caution.
///
/// By default, Skia operates with the assumption of a little-Endian system. The names of each
/// variant implicitly define the channel ordering and size in memory. Due to historical reasons
/// the names do not follow 100% identical convention, but are typically labeled from least
/// significant to most significant. To help clarify when the actual data layout differs from the
/// default convention, every variant's comment includes a bit-labeled description of a pixel in
/// that color type on a LE system.
///
/// Unless specified otherwise, a channel's value is treated as an unsigned integer with a range of
/// of `[0, 2^N-1]` and this is mapped uniformly to a floating point value of `[0.0, 1.0]`. Some
/// color types instead store data directly in 32-bit floating point (assumed to be IEEE), or in
/// 16-bit "half" floating point values. A half float, or F16/float16, is interpreted as FP 1-5-10
/// or `Bits: [sign:15 exp:14..10 man:9..0]`.
// Port of: include/core/SkColorType.h#L32-L156 (chrome/m156)
#[doc(alias = "SkColorType")]
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[repr(i32)]
pub enum ColorType {
    /// Unknown or unrepresentable as a [`ColorType`].
    #[doc(alias = "kUnknown_SkColorType")]
    #[default]
    Unknown = 0,
    /// Single channel data (8-bit) interpreted as an alpha value. RGB are 0.
    /// Bits: `[A:7..0]`
    #[doc(alias = "kAlpha_8_SkColorType")]
    Alpha8 = 1,
    /// Three channel BGR data (5 bits red, 6 bits green, 5 bits blue) packed into a LE 16-bit word.
    /// Bits: `[R:15..11 G:10..5 B:4..0]`
    #[doc(alias = "kRGB_565_SkColorType")]
    RGB565 = 2,
    /// Four channel ABGR data (4 bits per channel) packed into a LE 16-bit word.
    /// Bits: `[R:15..12 G:11..8 B:7..4 A:3..0]`
    #[doc(alias = "kARGB_4444_SkColorType")]
    ARGB4444 = 3,
    /// Four channel RGBA data (8 bits per channel) packed into a LE 32-bit word.
    /// Bits: `[A:31..24 B:23..16 G:15..8 R:7..0]`
    #[doc(alias = "kRGBA_8888_SkColorType")]
    RGBA8888 = 4,
    /// Three channel RGB data (8 bits per channel) packed into a LE 32-bit word. The remaining
    /// bits are ignored and alpha is forced to opaque.
    /// Bits: `[x:31..24 B:23..16 G:15..8 R:7..0]`
    #[doc(alias = "kRGB_888x_SkColorType")]
    RGB888x = 5,
    /// Four channel BGRA data (8 bits per channel) packed into a LE 32-bit word. R and B are
    /// swapped relative to [`ColorType::RGBA8888`].
    /// Bits: `[A:31..24 R:23..16 G:15..8 B:7..0]`
    #[doc(alias = "kBGRA_8888_SkColorType")]
    BGRA8888 = 6,
    /// Four channel RGBA data (10 bits per color, 2 bits for alpha) packed into a LE 32-bit word.
    /// Bits: `[A:31..30 B:29..20 G:19..10 R:9..0]`
    #[doc(alias = "kRGBA_1010102_SkColorType")]
    RGBA1010102 = 7,
    /// Four channel BGRA data (10 bits per color, 2 bits for alpha) packed into a LE 32-bit word.
    /// R and B are swapped relative to [`ColorType::RGBA1010102`].
    /// Bits: `[A:31..30 R:29..20 G:19..10 B:9..0]`
    #[doc(alias = "kBGRA_1010102_SkColorType")]
    BGRA1010102 = 8,
    /// Three channel RGB data (10 bits per channel) packed into a LE 32-bit word. The remaining
    /// bits are ignored and alpha is forced to opaque.
    /// Bits: `[x:31..30 B:29..20 G:19..10 R:9..0]`
    #[doc(alias = "kRGB_101010x_SkColorType")]
    RGB101010x = 9,
    /// Three channel BGR data (10 bits per channel) packed into a LE 32-bit word. The remaining
    /// bits are ignored and alpha is forced to opaque. R and B are swapped relative to
    /// [`ColorType::RGB101010x`].
    /// Bits: `[x:31..30 R:29..20 G:19..10 B:9..0]`
    #[doc(alias = "kBGR_101010x_SkColorType")]
    BGR101010x = 10,
    /// Three channel BGR data (10 bits per channel) packed into a LE 32-bit word. The remaining
    /// bits are ignored and alpha is forced to opaque. Instead of normalizing `[0, 1023]` to
    /// `[0.0, 1.0]` the color channels map to an extended range of `[-0.752941, 1.25098]`,
    /// compatible with `MTLPixelFormatBGR10_XR`.
    /// Bits: `[x:31..30 R:29..20 G:19..10 B:9..0]`
    #[doc(alias = "kBGR_101010x_XR_SkColorType")]
    BGR101010xXR = 11,
    /// Four channel BGRA data (10 bits per channel) packed into a LE 64-bit word. Each channel is
    /// preceded by 6 bits of padding. Instead of normalizing `[0, 1023]` to `[0.0, 1.0]` the
    /// color and alpha channels map to an extended range of `[-0.752941, 1.25098]`, compatible
    /// with `MTLPixelFormatBGRA10_XR`.
    /// Bits: `[A:63..54 x:53..48 R:47..38 x:37..32 G:31..22 x:21..16 B:15..6 x:5..0]`
    #[doc(alias = "kBGRA_10101010_XR_SkColorType")]
    BGRA10101010XR = 12,
    /// Four channel RGBA data (10 bits per channel) packed into a LE 64-bit word. Each channel is
    /// preceded by 6 bits of padding.
    /// Bits: `[A:63..54 x:53..48 B:47..38 x:37..32 G:31..22 x:21..16 R:15..6 x:5..0]`
    #[doc(alias = "kRGBA_10x6_SkColorType")]
    RGBA10x6 = 13,
    /// Single channel data (8-bit) interpreted as a grayscale value (e.g. replicated to RGB).
    /// Bits: `[G:7..0]`
    #[doc(alias = "kGray_8_SkColorType")]
    Gray8 = 14,
    /// Four channel RGBA data (16-bit half-float per channel) packed into a LE 64-bit word. Values
    /// are assumed to be in `[0.0,1.0]` range, unlike [`ColorType::RGBAF16`].
    /// Bits: `[A:63..48 B:47..32 G:31..16 R:15..0]`
    #[doc(alias = "kRGBA_F16Norm_SkColorType")]
    RGBAF16Norm = 15,
    /// Four channel RGBA data (16-bit half-float per channel) packed into a LE 64-bit word. This
    /// has extended range compared to [`ColorType::RGBAF16Norm`].
    /// Bits: `[A:63..48 B:47..32 G:31..16 R:15..0]`
    #[doc(alias = "kRGBA_F16_SkColorType")]
    RGBAF16 = 16,
    /// Three channel RGB data (16-bit half-float per channel) packed into a LE 64-bit word. The
    /// last 16 bits are ignored and alpha is forced to opaque.
    /// Bits: `[x:63..48 B:47..32 G:31..16 R:15..0]`
    #[doc(alias = "kRGB_F16F16F16x_SkColorType")]
    RGBF16F16F16x = 17,
    /// Four channel RGBA data (32-bit float per channel) packed into a LE 128-bit word.
    /// Bits: `[A:127..96 B:95..64 G:63..32 R:31..0]`
    #[doc(alias = "kRGBA_F32_SkColorType")]
    RGBAF32 = 18,
    /// Two channel RG data (8 bits per channel). Blue is forced to 0, alpha is forced to opaque.
    /// Bits: `[G:15..8 R:7..0]`
    #[doc(alias = "kR8G8_unorm_SkColorType")]
    R8G8UNorm = 19,
    /// Single channel data (16-bit half-float) interpreted as alpha. RGB are 0.
    /// Bits: `[A:15..0]`
    #[doc(alias = "kA16_float_SkColorType")]
    A16Float = 20,
    /// Single channel data (16 bits half-float) interpreted as red. G and B are forced to 0, alpha
    /// is forced to opaque.
    /// Bits: `[R:15..0]`
    #[doc(alias = "kR16_float_SkColorType")]
    R16Float = 21,
    /// Two channel RG data (16-bit half-float per channel) packed into a LE 32-bit word. Blue is
    /// forced to 0, alpha is forced to opaque.
    /// Bits: `[G:31..16 R:15..0]`
    #[doc(alias = "kR16G16_float_SkColorType")]
    R16G16Float = 22,
    /// Single channel data (16 bits) interpreted as alpha. RGB are 0.
    /// Bits: `[A:15..0]`
    #[doc(alias = "kA16_unorm_SkColorType")]
    A16UNorm = 23,
    /// Single channel data (16 bits) interpreted as red. G and B are forced to 0, alpha is forced
    /// to opaque.
    /// Bits: `[R:15..0]`
    #[doc(alias = "kR16_unorm_SkColorType")]
    R16UNorm = 24,
    /// Two channel RG data (16 bits per channel) packed into a LE 32-bit word. B is forced to 0,
    /// alpha is forced to opaque.
    /// Bits: `[G:31..16 R:15..0]`
    #[doc(alias = "kR16G16_unorm_SkColorType")]
    R16G16UNorm = 25,
    /// Four channel RGBA data (16 bits per channel) packed into a LE 64-bit word.
    /// Bits: `[A:63..48 B:47..32 G:31..16 R:15..0]`
    #[doc(alias = "kR16G16B16A16_unorm_SkColorType")]
    R16G16B16A16UNorm = 26,
    /// Four channel RGBA data (8 bits per channel) packed into a LE 32-bit word. The RGB values
    /// are assumed to be encoded with the sRGB transfer function, which can be decoded
    /// automatically by GPU hardware with certain texture formats.
    /// Bits: `[A:31..24 B:23..16 G:15..8 R:7..0]`
    #[doc(alias = "kSRGBA_8888_SkColorType")]
    SRGBA8888 = 27,
    /// Single channel data (8 bits) interpreted as red. G and B are forced to 0, alpha is forced
    /// to opaque.
    /// Bits: `[R:7..0]`
    #[doc(alias = "kR8_unorm_SkColorType")]
    R8UNorm = 28,
}

/// All color types, indexed by their discriminant.
const ALL: [ColorType; ColorType::COUNT] = [
    ColorType::Unknown,
    ColorType::Alpha8,
    ColorType::RGB565,
    ColorType::ARGB4444,
    ColorType::RGBA8888,
    ColorType::RGB888x,
    ColorType::BGRA8888,
    ColorType::RGBA1010102,
    ColorType::BGRA1010102,
    ColorType::RGB101010x,
    ColorType::BGR101010x,
    ColorType::BGR101010xXR,
    ColorType::BGRA10101010XR,
    ColorType::RGBA10x6,
    ColorType::Gray8,
    ColorType::RGBAF16Norm,
    ColorType::RGBAF16,
    ColorType::RGBF16F16F16x,
    ColorType::RGBAF32,
    ColorType::R8G8UNorm,
    ColorType::A16Float,
    ColorType::R16Float,
    ColorType::R16G16Float,
    ColorType::A16UNorm,
    ColorType::R16UNorm,
    ColorType::R16G16UNorm,
    ColorType::R16G16B16A16UNorm,
    ColorType::SRGBA8888,
    ColorType::R8UNorm,
];

impl ColorType {
    /// The native 32-bit encoding: BGRA or RGBA, whichever is the platform's (see
    /// `color_priv::PMCOLOR_IS_BGRA`).
    #[doc(alias = "kN32_SkColorType")]
    pub const N32: Self = if PMCOLOR_IS_BGRA {
        Self::BGRA8888
    } else {
        Self::RGBA8888
    };

    /// The last valid value.
    #[doc(alias = "kLastEnum_SkColorType")]
    pub const LAST_ENUM: Self = Self::R8UNorm;

    /// The number of color types (`kSkColorTypeCnt`).
    #[doc(alias = "kSkColorTypeCnt")]
    pub const COUNT: usize = Self::LAST_ENUM as usize + 1;

    /// The native 32-bit encoding, same as [`ColorType::N32`].
    #[doc(alias = "kN32_SkColorType")]
    #[must_use]
    pub const fn n32() -> Self {
        Self::N32
    }

    /// Converts a C-style integer value to a color type, if it is a valid one
    /// (`SkColorTypeIsValid`).
    ///
    /// skia-rust: there is no C-style cast from an integer in Rust.
    #[doc(alias = "SkColorTypeIsValid")]
    #[must_use]
    pub fn from_i32(value: i32) -> Option<Self> {
        usize::try_from(value)
            .ok()
            .and_then(|i| ALL.get(i))
            .copied()
    }

    /// Returns the number of bytes required to store a pixel, including unused padding. Returns
    /// zero if `self` is [`ColorType::Unknown`].
    // Port of: src/core/SkImageInfo.cpp#L16-L49 (chrome/m156)
    #[doc(alias = "SkColorTypeBytesPerPixel")]
    #[must_use]
    #[allow(clippy::match_same_arms)] // one arm per color type, as the C++ switch
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Unknown => 0,
            Self::Alpha8 => 1,
            Self::RGB565 => 2,
            Self::ARGB4444 => 2,
            Self::RGBA8888 => 4,
            Self::BGRA8888 => 4,
            Self::RGB888x => 4,
            Self::RGBA1010102 => 4,
            Self::RGB101010x => 4,
            Self::BGRA1010102 => 4,
            Self::BGR101010x => 4,
            Self::BGR101010xXR => 4,
            Self::BGRA10101010XR => 8,
            Self::RGBA10x6 => 8,
            Self::Gray8 => 1,
            Self::RGBAF16Norm => 8,
            Self::RGBAF16 => 8,
            Self::RGBF16F16F16x => 8,
            Self::RGBAF32 => 16,
            Self::R8G8UNorm => 2,
            Self::A16UNorm => 2,
            Self::R16UNorm => 2,
            Self::R16G16UNorm => 4,
            Self::A16Float => 2,
            Self::R16Float => 2,
            Self::R16G16Float => 4,
            Self::R16G16B16A16UNorm => 8,
            Self::SRGBA8888 => 4,
            Self::R8UNorm => 1,
        }
    }

    /// Returns true if `self` always describes an opaque pixel (it does not reserve bits to
    /// encode alpha).
    // Port of: src/core/SkImageInfo.cpp#L51-L53 (chrome/m156)
    #[doc(alias = "SkColorTypeIsAlwaysOpaque")]
    #[must_use]
    pub fn is_always_opaque(self) -> bool {
        (color_type_channel_flags(self) & ColorChannelFlag::ALPHA).bits() == 0
    }

    /// Returns the canonical alpha type for `alpha_type` and this color type, if a valid
    /// [`AlphaType`] can be associated with it.
    ///
    /// Returns `None` only if `alpha_type` is [`AlphaType::Unknown`], the color type is not
    /// [`ColorType::Unknown`], and the color type is not always opaque.
    // Port of: src/core/SkImageInfo.cpp#L239-L288 (chrome/m156)
    #[doc(alias = "SkColorTypeValidateAlphaType")]
    #[must_use]
    #[allow(clippy::match_same_arms)] // one arm per color type, as the C++ switch
    pub fn validate_alpha_type(self, alpha_type: AlphaType) -> Option<AlphaType> {
        let mut alpha_type = alpha_type;
        match self {
            Self::Unknown => {
                alpha_type = AlphaType::Unknown;
            }
            Self::Alpha8
            | Self::A16UNorm
            | Self::A16Float
            | Self::ARGB4444
            | Self::RGBA8888
            | Self::SRGBA8888
            | Self::BGRA8888
            | Self::RGBA1010102
            | Self::BGRA1010102
            | Self::RGBA10x6
            | Self::RGBAF16Norm
            | Self::RGBAF16
            | Self::RGBAF32
            | Self::BGRA10101010XR
            | Self::R16G16B16A16UNorm => {
                // The first three fall through from the A8-like group in the C++:
                if matches!(self, Self::Alpha8 | Self::A16UNorm | Self::A16Float)
                    && alpha_type == AlphaType::Unpremul
                {
                    alpha_type = AlphaType::Premul;
                }
                if alpha_type == AlphaType::Unknown {
                    return None;
                }
            }
            Self::Gray8
            | Self::R8G8UNorm
            | Self::R16UNorm
            | Self::R16Float
            | Self::R16G16UNorm
            | Self::R16G16Float
            | Self::RGB565
            | Self::RGB888x
            | Self::RGB101010x
            | Self::BGR101010x
            | Self::BGR101010xXR
            | Self::RGBF16F16F16x
            | Self::R8UNorm => {
                alpha_type = AlphaType::Opaque;
            }
        }
        Some(alpha_type)
    }
}
