// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/TextureFormat.h, src/gpu/graphite/TextureFormat.cpp

//! `TextureFormat`: every color, depth and stencil format a Graphite back end can use, and how
//! CPU color types map onto them.
//!
//! Names follow Skia's convention: components in little-endian order, a numeral `n` for an
//! unsigned normalized channel of `n` bits, `nF` for floats, with behavioural tags (sRGB,
//! extended range, compression) last. The table below gives the backend equivalents; a format is
//! supported when the back end's caps map it.
//!
//! | Format | `VK_FORMAT` | `wgpu::TextureFormat` | `MTLPixelFormat` |
//! |---|---|---|---|
//! | `R8` | `R8_UNORM` | `R8Unorm` | `R8Unorm` |
//! | `RGBA8` | `R8G8B8A8_UNORM` | `RGBA8Unorm` | `RGBA8Unorm` |
//! | `BGRA8` | `B8G8R8A8_UNORM` | `BGRA8Unorm` | `BGRA8Unorm` |
//! | `D24_S8` | `D24_UNORM_S8_UINT` | `Depth24PlusStencil8` | `Depth24Unorm_Stencil8` |
//!
//! (See `TextureFormat.h` for the full table.)

use bitflags::bitflags;
use skia_rust_core::color::ColorChannelFlag;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info_priv::{color_type_channel_flags, color_type_is_alpha_only};
use skia_rust_core::texture_compression_type::TextureCompressionType;

use crate::gpu::swizzle::Swizzle;

/// Every texture format Graphite back ends can support.
// Port of: src/gpu/graphite/TextureFormat.h#L60-L127 (chrome/m156)
#[doc(alias = "skgpu::graphite::TextureFormat")]
#[allow(non_camel_case_types)] // keeps Skia's names (`B5_G6_R5`, `RGB8_sRGB`, ...)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum TextureFormat {
    #[default]
    Unsupported,
    // 1 channel
    R8,
    R16,
    R16F,
    R32F,
    A8,
    // 2 channel
    RG8,
    RG16,
    RG16F,
    RG32F,
    // 3 channel
    RGB8,
    BGR8,
    B5_G6_R5,
    R5_G6_B5,
    RGB16,
    RGB16F,
    RGB32F,
    RGB8_sRGB,
    BGR10_XR,
    // 4 channel
    RGBA8,
    RGBA16,
    RGBA16F,
    RGBA32F,
    RGB10_A2,
    RGBA10x6,
    RGBA8_sRGB,
    BGRA8,
    BGR10_A2,
    BGRA8_sRGB,
    ABGR4,
    ARGB4,
    BGRA10x6_XR,
    // Compressed
    RGB8_ETC2,
    RGB8_ETC2_sRGB,
    RGB8_BC1,
    RGBA8_BC1,
    RGBA8_BC1_sRGB,
    // Multi-planar
    YUV8_P2_420,
    YUV8_P3_420,
    YUV10x6_P2_420,
    YUV8_P2_422,
    YUV8_P3_422,
    YUV10x6_P2_422,
    YUV8_P2_444,
    YUV8_P3_444,
    YUV10x6_P2_444,
    External,
    // Non-color
    S8,
    D16,
    D32F,
    D24_S8,
    D32F_S8,
}

use TextureFormat as TF;

impl TextureFormat {
    /// `kLast`.
    pub const LAST: Self = Self::D32F_S8;

    /// Every format, in enum order (`static_cast<TextureFormat>(i)` for `i < kTextureFormatCount`).
    pub const ALL: [Self; TEXTURE_FORMAT_COUNT] = [
        TF::Unsupported,
        TF::R8,
        TF::R16,
        TF::R16F,
        TF::R32F,
        TF::A8,
        TF::RG8,
        TF::RG16,
        TF::RG16F,
        TF::RG32F,
        TF::RGB8,
        TF::BGR8,
        TF::B5_G6_R5,
        TF::R5_G6_B5,
        TF::RGB16,
        TF::RGB16F,
        TF::RGB32F,
        TF::RGB8_sRGB,
        TF::BGR10_XR,
        TF::RGBA8,
        TF::RGBA16,
        TF::RGBA16F,
        TF::RGBA32F,
        TF::RGB10_A2,
        TF::RGBA10x6,
        TF::RGBA8_sRGB,
        TF::BGRA8,
        TF::BGR10_A2,
        TF::BGRA8_sRGB,
        TF::ABGR4,
        TF::ARGB4,
        TF::BGRA10x6_XR,
        TF::RGB8_ETC2,
        TF::RGB8_ETC2_sRGB,
        TF::RGB8_BC1,
        TF::RGBA8_BC1,
        TF::RGBA8_BC1_sRGB,
        TF::YUV8_P2_420,
        TF::YUV8_P3_420,
        TF::YUV10x6_P2_420,
        TF::YUV8_P2_422,
        TF::YUV8_P3_422,
        TF::YUV10x6_P2_422,
        TF::YUV8_P2_444,
        TF::YUV8_P3_444,
        TF::YUV10x6_P2_444,
        TF::External,
        TF::S8,
        TF::D16,
        TF::D32F,
        TF::D24_S8,
        TF::D32F_S8,
    ];
}

/// `kTextureFormatCount`.
// Port of: src/gpu/graphite/TextureFormat.h#L128 (chrome/m156)
pub const TEXTURE_FORMAT_COUNT: usize = TextureFormat::LAST as usize + 1;

/// `TextureFormatName`.
// Port of: src/gpu/graphite/TextureFormat.cpp#L18-L74 (chrome/m156)
#[doc(alias = "TextureFormatName")]
#[must_use]
pub const fn texture_format_name(format: TextureFormat) -> &'static str {
    match format {
        TF::Unsupported => "Unsupported",
        TF::R8 => "R8",
        TF::R16 => "R16",
        TF::R16F => "R16F",
        TF::R32F => "R32F",
        TF::A8 => "A8",
        TF::RG8 => "RG8",
        TF::RG16 => "RG16",
        TF::RG16F => "RG16F",
        TF::RG32F => "RG32F",
        TF::RGB8 => "RGB8",
        TF::BGR8 => "BGR8",
        TF::B5_G6_R5 => "B5_G6_R5",
        TF::R5_G6_B5 => "R5_G6_B5",
        TF::RGB16 => "RGB16",
        TF::RGB16F => "RGB16F",
        TF::RGB32F => "RGB32F",
        TF::RGB8_sRGB => "RGB8_sRGB",
        TF::BGR10_XR => "BGR10_XR",
        TF::RGBA8 => "RGBA8",
        // (sic) Skia's name for kRGBA16.
        TF::RGBA16 => "RBGA16",
        TF::RGBA16F => "RGBA16F",
        TF::RGBA32F => "RGBA32F",
        TF::RGB10_A2 => "RGB10_A2",
        TF::RGBA10x6 => "RGBA10x6",
        TF::RGBA8_sRGB => "RGBA8_sRGB",
        TF::BGRA8 => "BGRA8",
        TF::BGR10_A2 => "BGR10_A2",
        TF::BGRA8_sRGB => "BGRA8_sRGB",
        TF::ABGR4 => "ABGR4",
        TF::ARGB4 => "ARGB4",
        TF::BGRA10x6_XR => "BGRA10x6_XR",
        TF::RGB8_ETC2 => "RGB8_ETC2",
        TF::RGB8_ETC2_sRGB => "RGB8_ETC2_sRGB",
        TF::RGB8_BC1 => "RGB8_BC1",
        TF::RGBA8_BC1 => "RGBA8_BC1",
        TF::RGBA8_BC1_sRGB => "RGBA8_BC1_sRGB",
        TF::YUV8_P2_420 => "YUV8_P2_420",
        TF::YUV8_P3_420 => "YUV8_P3_420",
        TF::YUV10x6_P2_420 => "YUV10x6_P2_420",
        TF::YUV8_P2_422 => "YUV8_P2_422",
        TF::YUV8_P3_422 => "YUV8_P3_422",
        TF::YUV10x6_P2_422 => "YUV10x6_P2_422",
        TF::YUV8_P2_444 => "YUV8_P2_444",
        TF::YUV8_P3_444 => "YUV8_P3_444",
        TF::YUV10x6_P2_444 => "YUV10x6_P2_444",
        TF::External => "External",
        TF::S8 => "S8",
        TF::D16 => "D16",
        TF::D32F => "D32F",
        TF::D24_S8 => "D24_S8",
        TF::D32F_S8 => "D32F_S8",
    }
}

/// `TextureFormatCompressionType`.
// Port of: src/gpu/graphite/TextureFormat.cpp#L76-L85 (chrome/m156)
#[doc(alias = "TextureFormatCompressionType")]
#[must_use]
pub const fn texture_format_compression_type(format: TextureFormat) -> TextureCompressionType {
    match format {
        TF::RGB8_ETC2 | TF::RGB8_ETC2_sRGB => TextureCompressionType::ETC2_RGB8_UNORM,
        TF::RGB8_BC1 => TextureCompressionType::BC1_RGB8_UNORM,
        TF::RGBA8_BC1 | TF::RGBA8_BC1_sRGB => TextureCompressionType::BC1_RGBA8_UNORM,
        _ => TextureCompressionType::None,
    }
}

/// `CompressionTypeToTextureFormat`.
// Port of: src/gpu/graphite/TextureFormat.cpp#L87-L94 (chrome/m156)
#[doc(alias = "CompressionTypeToTextureFormat")]
#[must_use]
pub const fn compression_type_to_texture_format(ty: TextureCompressionType) -> TextureFormat {
    match ty {
        TextureCompressionType::BC1_RGB8_UNORM => TF::RGB8_BC1,
        TextureCompressionType::BC1_RGBA8_UNORM => TF::RGBA8_BC1,
        TextureCompressionType::ETC2_RGB8_UNORM => TF::RGB8_ETC2,
        TextureCompressionType::None => TF::Unsupported,
    }
}

/// `TextureFormatBytesPerBlock`: bytes per pixel, or per compressed block.
// Port of: src/gpu/graphite/TextureFormat.cpp#L96-L166 (chrome/m156)
#[doc(alias = "TextureFormatBytesPerBlock")]
#[must_use]
#[allow(clippy::match_same_arms)] // one arm per format, as in Skia's switch
pub const fn texture_format_bytes_per_block(format: TextureFormat) -> i32 {
    match format {
        TF::Unsupported => 0,
        TF::R8 => 1,
        TF::R16 => 2,
        TF::R16F => 2,
        TF::R32F => 4,
        TF::A8 => 1,
        TF::RG8 => 2,
        TF::RG16 => 4,
        TF::RG16F => 4,
        TF::RG32F => 8,
        TF::RGB8 => 3,
        TF::BGR8 => 3,
        TF::B5_G6_R5 => 2,
        TF::R5_G6_B5 => 2,
        TF::RGB16 => 6,
        TF::RGB16F => 6,
        TF::RGB32F => 12,
        TF::RGB8_sRGB => 3,
        TF::BGR10_XR => 4,
        TF::RGBA8 => 4,
        TF::RGBA16 => 8,
        TF::RGBA16F => 8,
        TF::RGBA32F => 16,
        TF::RGB10_A2 => 4,
        TF::RGBA10x6 => 8,
        TF::RGBA8_sRGB => 4,
        TF::BGRA8 => 4,
        TF::BGR10_A2 => 4,
        TF::BGRA8_sRGB => 4,
        TF::ABGR4 => 2,
        TF::ARGB4 => 2,
        TF::BGRA10x6_XR => 8,
        TF::S8 => 1,
        TF::D16 => 2,
        TF::D32F => 4,
        TF::D24_S8 => 4,
        TF::D32F_S8 => 5, // Assuming it's multiplanar

        // NOTE: For compressed formats, the block size refers to an actual compressed block of
        // multiple texels, whereas with other formats the block size represents a single pixel.
        TF::RGB8_ETC2 | TF::RGB8_ETC2_sRGB | TF::RGB8_BC1 | TF::RGBA8_BC1 | TF::RGBA8_BC1_sRGB => 8,
        // NOTE: We don't actually know the size of external formats, so this is an arbitrary
        // value. We will see external formats only in wrapped SkImages, so this won't impact
        // Skia's internal budgeting.
        TF::External => 4,
        // TODO(b/401016699): We are just over estimating this value to be used in gpu size
        // calculations even though the actually size is probably less.
        TF::YUV8_P2_420
        | TF::YUV8_P3_420
        | TF::YUV8_P2_422
        | TF::YUV8_P3_422
        | TF::YUV8_P2_444
        | TF::YUV8_P3_444 => 3,
        TF::YUV10x6_P2_420 | TF::YUV10x6_P2_422 | TF::YUV10x6_P2_444 => 6,
    }
}

const ALPHA: u32 = ColorChannelFlag::ALPHA.bits();
const RED: u32 = ColorChannelFlag::RED.bits();
const GRAY: u32 = ColorChannelFlag::GRAY.bits();

/// `TextureFormatChannelMask`: a mask of `SkColorChannelFlag` values.
// Port of: src/gpu/graphite/TextureFormat.cpp#L168-L229 (chrome/m156)
#[doc(alias = "TextureFormatChannelMask")]
#[must_use]
pub const fn texture_format_channel_mask(format: TextureFormat) -> u32 {
    match format {
        TF::A8 => ColorChannelFlag::ALPHA.bits(),

        TF::R8 | TF::R16 | TF::R16F | TF::R32F => ColorChannelFlag::RED.bits(),

        TF::RG8 | TF::RG16 | TF::RG16F | TF::RG32F => ColorChannelFlag::RG.bits(),

        TF::RGB8
        | TF::BGR8
        | TF::B5_G6_R5
        | TF::R5_G6_B5
        | TF::RGB16
        | TF::RGB16F
        | TF::RGB32F
        | TF::RGB8_sRGB
        | TF::BGR10_XR
        | TF::RGB8_ETC2
        | TF::RGB8_ETC2_sRGB
        | TF::RGB8_BC1
        | TF::YUV8_P2_420
        | TF::YUV8_P3_420
        | TF::YUV10x6_P2_420
        | TF::YUV8_P2_422
        | TF::YUV8_P3_422
        | TF::YUV10x6_P2_422
        | TF::YUV8_P2_444
        | TF::YUV8_P3_444
        | TF::YUV10x6_P2_444 => ColorChannelFlag::RGB.bits(),

        TF::RGBA8
        | TF::RGBA16
        | TF::RGBA16F
        | TF::RGBA32F
        | TF::RGB10_A2
        | TF::RGBA10x6
        | TF::RGBA8_sRGB
        | TF::BGRA8
        | TF::BGR10_A2
        | TF::BGRA8_sRGB
        | TF::ABGR4
        | TF::ARGB4
        | TF::BGRA10x6_XR
        | TF::RGBA8_BC1
        | TF::RGBA8_BC1_sRGB
        | TF::External => ColorChannelFlag::RGBA.bits(),

        TF::S8 | TF::D16 | TF::D32F | TF::D24_S8 | TF::D32F_S8 | TF::Unsupported => 0,
    }
}

/// `TextureFormatAutoClamps`: true if writes to a color attachment of this format automatically
/// clamp to [0,1].
// Port of: src/gpu/graphite/TextureFormat.cpp#L231-L238 (chrome/m156)
#[doc(alias = "TextureFormatAutoClamps")]
#[must_use]
pub const fn texture_format_auto_clamps(format: TextureFormat) -> bool {
    // Floating point formats, extended range formats, and non-normalized integer formats do not
    // auto-clamp. Everything behaves like an unsigned normalized number.
    !(texture_format_is_floating_point(format)
        || matches!(format, TF::BGR10_XR | TF::BGRA10x6_XR | TF::S8))
}

/// `TextureFormatIsFloatingPoint`.
// Port of: src/gpu/graphite/TextureFormat.cpp#L240-L299 (chrome/m156)
#[doc(alias = "TextureFormatIsFloatingPoint")]
#[must_use]
pub const fn texture_format_is_floating_point(format: TextureFormat) -> bool {
    match format {
        // Floating point formats
        TF::R16F
        | TF::R32F
        | TF::RG16F
        | TF::RG32F
        | TF::RGB16F
        | TF::RGB32F
        | TF::RGBA16F
        | TF::RGBA32F
        | TF::D32F
        | TF::D32F_S8 => true,

        // Everything else is unorm, unorm-srgb, fixed point, or integral
        TF::Unsupported
        | TF::R8
        | TF::R16
        | TF::A8
        | TF::RG8
        | TF::RG16
        | TF::RGB8
        | TF::BGR8
        | TF::B5_G6_R5
        | TF::R5_G6_B5
        | TF::RGB16
        | TF::RGB8_sRGB
        | TF::BGR10_XR
        | TF::RGBA8
        | TF::RGBA16
        | TF::RGB10_A2
        | TF::RGBA10x6
        | TF::RGBA8_sRGB
        | TF::BGRA8
        | TF::BGR10_A2
        | TF::BGRA8_sRGB
        | TF::ABGR4
        | TF::ARGB4
        | TF::BGRA10x6_XR
        | TF::RGB8_ETC2
        | TF::RGB8_ETC2_sRGB
        | TF::RGB8_BC1
        | TF::RGBA8_BC1
        | TF::RGBA8_BC1_sRGB
        | TF::YUV8_P2_420
        | TF::YUV8_P3_420
        | TF::YUV10x6_P2_420
        | TF::YUV8_P2_422
        | TF::YUV8_P3_422
        | TF::YUV10x6_P2_422
        | TF::YUV8_P2_444
        | TF::YUV8_P3_444
        | TF::YUV10x6_P2_444
        | TF::External
        | TF::S8
        | TF::D16
        | TF::D24_S8 => false,
    }
}

/// `TextureFormatIsDepthOrStencil`.
// Port of: src/gpu/graphite/TextureFormat.cpp#L301-L312 (chrome/m156)
#[doc(alias = "TextureFormatIsDepthOrStencil")]
#[must_use]
pub const fn texture_format_is_depth_or_stencil(format: TextureFormat) -> bool {
    matches!(
        format,
        TF::S8 | TF::D16 | TF::D32F | TF::D24_S8 | TF::D32F_S8
    )
}

/// `TextureFormatHasDepth`.
// Port of: src/gpu/graphite/TextureFormat.cpp#L314-L324 (chrome/m156)
#[doc(alias = "TextureFormatHasDepth")]
#[must_use]
pub const fn texture_format_has_depth(format: TextureFormat) -> bool {
    matches!(format, TF::D16 | TF::D32F | TF::D24_S8 | TF::D32F_S8)
}

/// `TextureFormatHasStencil`.
// Port of: src/gpu/graphite/TextureFormat.cpp#L326-L335 (chrome/m156)
#[doc(alias = "TextureFormatHasStencil")]
#[must_use]
pub const fn texture_format_has_stencil(format: TextureFormat) -> bool {
    matches!(format, TF::S8 | TF::D24_S8 | TF::D32F_S8)
}

/// `TextureFormatIsMultiplanar`.
// Port of: src/gpu/graphite/TextureFormat.cpp#L337-L352 (chrome/m156)
#[doc(alias = "TextureFormatIsMultiplanar")]
#[must_use]
pub const fn texture_format_is_multiplanar(format: TextureFormat) -> bool {
    matches!(
        format,
        TF::YUV8_P2_420
            | TF::YUV8_P3_420
            | TF::YUV10x6_P2_420
            | TF::YUV8_P2_422
            | TF::YUV8_P3_422
            | TF::YUV10x6_P2_422
            | TF::YUV8_P2_444
            | TF::YUV8_P3_444
            | TF::YUV10x6_P2_444
    )
}

/// `ReadSwizzleForColorType`: the swizzle to use when sampling or reading back a texture of
/// `format` as color type `ct`.
// Port of: src/gpu/graphite/TextureFormat.cpp#L357-L399 (chrome/m156)
#[doc(alias = "ReadSwizzleForColorType")]
#[must_use]
#[allow(clippy::if_not_else)] // keeps Skia's branch order
pub fn read_swizzle_for_color_type(ct: ColorType, format: TextureFormat) -> Swizzle {
    let color_channels = color_type_channel_flags(ct).bits();
    let format_channels = texture_format_channel_mask(format);

    // Read swizzles only have to handle a few semantics around the sampled values, as any sort of
    // channel ordering for RGB vs BGR is handled by hardware. All we have to handle is mapping to
    // "gray", red-vs-alpha, and forcing to opaque.
    if color_type_is_alpha_only(ct) {
        // If the format isn't just an alpha channel (e.g. TF::A8), we need to adjust
        if format_channels != ALPHA {
            // If the format has an alpha channel, mask every other channel to 0
            if format_channels & ALPHA != 0 {
                Swizzle::new("000a")
            } else {
                // Otherwise move the red channel to alpha
                debug_assert!(format_channels & RED != 0);
                Swizzle::new("000r")
            }
        } else {
            // otherwise leave as "rgba" and let hardware do the right thing
            Swizzle::rgba()
        }
    } else {
        // First map gray to rrra; if this is just gray and not gray+alpha, it will also be forced
        // to opaque below and become rrr1.
        let mut swizzle = if color_channels & GRAY != 0 {
            debug_assert!(format_channels & RED != 0);
            Swizzle::rrra()
        } else {
            Swizzle::rgba()
        };

        // Last, force the alpha to opaque if the color type masks it off but is present in the
        // texture format.
        if color_channels & ALPHA == 0 && format_channels & ALPHA != 0 {
            swizzle = Swizzle::concat(&swizzle, &Swizzle::rgb1());
        }

        swizzle
    }
}

/// `WriteSwizzleForColorType`: the swizzle to use when rendering color type `ct` into `format`,
/// or `None` if the color type cannot be rendered to it.
// Port of: src/gpu/graphite/TextureFormat.cpp#L401-L444 (chrome/m156)
#[doc(alias = "WriteSwizzleForColorType")]
#[must_use]
#[allow(clippy::if_not_else)] // keeps Skia's branch order
pub fn write_swizzle_for_color_type(ct: ColorType, format: TextureFormat) -> Option<Swizzle> {
    // D/S, compressed, external, and multiplanar formats aren't renderable with a color type.
    if format == TF::External
        || texture_format_is_depth_or_stencil(format)
        || texture_format_is_multiplanar(format)
        || !matches!(
            texture_format_compression_type(format),
            TextureCompressionType::None
        )
    {
        return None;
    }

    let color_channels = color_type_channel_flags(ct).bits();
    let format_channels = texture_format_channel_mask(format);

    // Write swizzles only have to handle red vs alpha; gray (luminance is beyond a swizzle) and
    // forced-opaque rendering are disallowed.
    if color_type_is_alpha_only(ct) {
        // If the format isn't just an alpha channel (e.g. TF::A8), we need to adjust
        if format_channels != ALPHA {
            // If the format has an alpha channel, mask every other channel to 0
            if format_channels & ALPHA != 0 {
                Some(Swizzle::new("000a"))
            } else {
                // Otherwise move the alpha channel to red
                debug_assert!(format_channels & RED != 0);
                Some(Swizzle::new("a000"))
            }
        } else {
            // otherwise leave as "rgba" and let hardware do the right thing
            Some(Swizzle::rgba())
        }
    } else {
        if (color_channels & format_channels) != format_channels || (color_channels & GRAY) != 0 {
            return None;
        }
        Some(Swizzle::rgba())
    }
}

/// `PreferredTextureFormats`: the formats for `ct`, most preferred first. These must still be
/// filtered by support in the back end's caps.
// Port of: src/gpu/graphite/TextureFormat.cpp#L446-L497 (chrome/m156)
#[doc(alias = "PreferredTextureFormats")]
#[must_use]
#[allow(clippy::match_same_arms)] // one arm per color type, as in Skia's switch
pub const fn preferred_texture_formats(ct: ColorType) -> &'static [TextureFormat] {
    // NOTE: Not all backends support all TextureFormats. For color types that have equivalent
    // texture formats differing only in RGB vs. BGR swizzle, we allow both format variations to
    // maximize color types that have some format. For alpha-only color types, we only match to
    // red-channel formats as they have the broadest support.
    match ct {
        ColorType::Unknown => &[],
        ColorType::Alpha8 => &[TF::R8],
        // NOTE: kRGB_565_SkColorType is misnamed and natively matches B5_G6_R5
        ColorType::RGB565 => &[TF::B5_G6_R5, TF::R5_G6_B5],
        // NOTE: kARGB_4444_SkColorType is misnamed and natively matches ABGR4
        ColorType::ARGB4444 => &[TF::ABGR4, TF::ARGB4],
        ColorType::RGBA8888 => &[TF::RGBA8, TF::BGRA8],
        ColorType::RGB888x => &[TF::RGB8, TF::RGBA8, TF::BGRA8],
        ColorType::BGRA8888 => &[TF::BGRA8, TF::RGBA8],
        ColorType::RGBA1010102 => &[TF::RGB10_A2, TF::BGR10_A2],
        ColorType::BGRA1010102 => &[TF::BGR10_A2, TF::RGB10_A2],
        ColorType::RGB101010x => &[TF::RGB10_A2, TF::BGR10_A2],
        ColorType::BGR101010x => &[TF::BGR10_A2, TF::RGB10_A2],
        ColorType::BGR101010xXR => &[TF::BGR10_XR],
        ColorType::BGRA10101010XR => &[TF::BGRA10x6_XR],
        ColorType::RGBA10x6 => &[TF::RGBA10x6],
        ColorType::Gray8 => &[TF::R8],
        ColorType::RGBAF16Norm => &[TF::RGBA16F],
        ColorType::RGBAF16 => &[TF::RGBA16F],
        ColorType::RGBF16F16F16x => &[TF::RGBA16F],
        ColorType::RGBAF32 => &[TF::RGBA32F],
        ColorType::R8G8UNorm => &[TF::RG8],
        ColorType::A16Float => &[TF::R16F],
        ColorType::R16Float => &[TF::R16F],
        ColorType::R16G16Float => &[TF::RG16F],
        ColorType::A16UNorm => &[TF::R16],
        ColorType::R16UNorm => &[TF::R16],
        ColorType::R16G16UNorm => &[TF::RG16],
        ColorType::R16G16B16A16UNorm => &[TF::RGBA16],
        ColorType::SRGBA8888 => &[TF::RGBA8_sRGB, TF::BGRA8_sRGB],
        ColorType::R8UNorm => &[TF::R8],
    }
}

bitflags! {
    /// Extra operations that must be applied to CPU data to make it exactly match the color type.
    ///
    /// The empty set is `kIdentity` (no extra conversion).
    // Port of: src/gpu/graphite/TextureFormat.h#L190-L196 (chrome/m156)
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct FormatXferOp: u8 {
        /// No color type matches the channel ordering, so swap RB on read or write.
        #[doc(alias = "kSwapRB")]
        const SWAP_RB = 0x1;
        /// No alpha channel in the format but the color type expects it, so drop or pad.
        #[doc(alias = "kDropAlpha")]
        const DROP_ALPHA = 0x2;
        /// No color type can represent the CPU data; transfers are disabled.
        #[doc(alias = "kDisabled")]
        const DISABLED = 0x4;
    }
}

impl FormatXferOp {
    /// `kIdentity`: no extra conversion needed.
    pub const IDENTITY: Self = Self::empty();
}

/// `TextureFormatColorTypeInfo`: the best color type for `format`, plus any data manipulation
/// needed to make the texture's data exactly match it. If the ops include
/// [`FormatXferOp::DISABLED`], reads and writes in terms of the color type are not allowed.
// Port of: src/gpu/graphite/TextureFormat.cpp#L499-L570 (chrome/m156)
#[doc(alias = "TextureFormatColorTypeInfo")]
#[must_use]
#[allow(clippy::match_same_arms)] // one arm per format, as in Skia's table
pub const fn texture_format_color_type_info(format: TextureFormat) -> (ColorType, FormatXferOp) {
    type X = FormatXferOp;
    let swap_and_drop = X::SWAP_RB.union(X::DROP_ALPHA);
    match format {
        //   TextureFormat  | SkColorType              | FormatXferOp(s)
        TF::Unsupported => (ColorType::Unknown, X::DISABLED),

        TF::R8 => (ColorType::R8UNorm, X::IDENTITY),
        TF::R16 => (ColorType::R16UNorm, X::IDENTITY),
        TF::R16F => (ColorType::R16Float, X::IDENTITY),
        TF::R32F => (ColorType::R16Float, X::DISABLED),
        TF::A8 => (ColorType::Alpha8, X::IDENTITY),
        TF::RG8 => (ColorType::R8G8UNorm, X::IDENTITY),
        TF::RG16 => (ColorType::R16G16UNorm, X::IDENTITY),
        TF::RG16F => (ColorType::R16G16Float, X::IDENTITY),
        TF::RG32F => (ColorType::R16G16Float, X::DISABLED),
        TF::RGB8 => (ColorType::RGB888x, X::DROP_ALPHA),
        TF::BGR8 => (ColorType::RGB888x, swap_and_drop),
        // NOTE: kRGB_565_SkColorType is misnamed and natively matches TextureFormat::B5_G6_R5
        TF::B5_G6_R5 => (ColorType::RGB565, X::IDENTITY),
        TF::R5_G6_B5 => (ColorType::RGB565, X::SWAP_RB),
        TF::RGB16 => (ColorType::R16G16B16A16UNorm, X::DROP_ALPHA),
        TF::RGB16F => (ColorType::RGBF16F16F16x, X::DROP_ALPHA),
        TF::RGB32F => (ColorType::RGBAF32, X::DROP_ALPHA),
        TF::RGB8_sRGB => (ColorType::SRGBA8888, X::DROP_ALPHA),
        TF::BGR10_XR => (ColorType::BGR101010xXR, X::IDENTITY),
        TF::RGBA8 => (ColorType::RGBA8888, X::IDENTITY),
        TF::RGBA16 => (ColorType::R16G16B16A16UNorm, X::IDENTITY),
        TF::RGBA16F => (ColorType::RGBAF16, X::IDENTITY),
        TF::RGBA32F => (ColorType::RGBAF32, X::IDENTITY),
        TF::RGB10_A2 => (ColorType::RGBA1010102, X::IDENTITY),
        TF::RGBA10x6 => (ColorType::RGBA10x6, X::IDENTITY),
        TF::RGBA8_sRGB => (ColorType::SRGBA8888, X::IDENTITY),
        TF::BGRA8 => (ColorType::BGRA8888, X::IDENTITY),
        TF::BGR10_A2 => (ColorType::BGRA1010102, X::IDENTITY),
        TF::BGRA8_sRGB => (ColorType::SRGBA8888, X::SWAP_RB),
        // NOTE: kARGB_4444_SkColorType is misnamed and natively matches TextureFormat::ABGR4
        TF::ABGR4 => (ColorType::ARGB4444, X::IDENTITY),
        TF::ARGB4 => (ColorType::ARGB4444, X::SWAP_RB),
        TF::BGRA10x6_XR => (ColorType::BGRA10101010XR, X::IDENTITY),

        // Compressed, multi-planar, and external formats can't exactly describe their data as
        // an SkColorType (although transfers with specialized data could be allowed).
        TF::RGB8_ETC2 => (ColorType::RGB888x, X::DISABLED),
        TF::RGB8_ETC2_sRGB => (ColorType::SRGBA8888, X::DISABLED),
        TF::RGB8_BC1 => (ColorType::RGB888x, X::DISABLED),
        TF::RGBA8_BC1 => (ColorType::RGBA8888, X::DISABLED),
        TF::RGBA8_BC1_sRGB => (ColorType::SRGBA8888, X::DISABLED),
        TF::YUV8_P2_420 => (ColorType::RGB888x, X::DISABLED),
        TF::YUV8_P3_420 => (ColorType::RGB888x, X::DISABLED),
        TF::YUV10x6_P2_420 => (ColorType::RGBA10x6, X::DISABLED),
        TF::YUV8_P2_422 => (ColorType::RGB888x, X::DISABLED),
        TF::YUV8_P3_422 => (ColorType::RGB888x, X::DISABLED),
        TF::YUV10x6_P2_422 => (ColorType::RGBA10x6, X::DISABLED),
        TF::YUV8_P2_444 => (ColorType::RGB888x, X::DISABLED),
        TF::YUV8_P3_444 => (ColorType::RGB888x, X::DISABLED),
        TF::YUV10x6_P2_444 => (ColorType::RGBA10x6, X::DISABLED),
        TF::External => (ColorType::RGBA8888, X::DISABLED),

        // Non color texture formats can't be used with SkColorType
        TF::S8 => (ColorType::Unknown, X::DISABLED),
        TF::D16 => (ColorType::Unknown, X::DISABLED),
        TF::D32F => (ColorType::Unknown, X::DISABLED),
        TF::D24_S8 => (ColorType::Unknown, X::DISABLED),
        TF::D32F_S8 => (ColorType::Unknown, X::DISABLED),
    }
}

/// `AreColorTypeAndFormatCompatible`: true if `target_color_type` has the same data type as the
/// format's base color type and any other channel differences are handled by hardware or by
/// [`read_swizzle_for_color_type`].
// Port of: src/gpu/graphite/TextureFormat.cpp#L572-L596 (chrome/m156)
#[doc(alias = "AreColorTypeAndFormatCompatible")]
#[must_use]
pub fn are_color_type_and_format_compatible(
    target_color_type: ColorType,
    format: TextureFormat,
) -> bool {
    // If the format maps to the color type, they are compatible
    let (base_color_type, _) = texture_format_color_type_info(format);
    if base_color_type != ColorType::Unknown && base_color_type == target_color_type {
        return true; // shortcut
    }

    // If the color type could map to the format, they are compatible
    if preferred_texture_formats(target_color_type).contains(&format) {
        return true;
    }

    // Also allow kRGB_888x if kRGBA_8888 is compatible since RGBx is just a swizzle. This is
    // almost always handled by the combination of base color type and preferred formats, but for
    // external and compressed formats those two functions aren't quite descriptive enough.
    if target_color_type == ColorType::RGB888x
        && are_color_type_and_format_compatible(ColorType::RGBA8888, format)
    {
        return true;
    }

    // Otherwise consider them incompatible
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_is_in_enum_order() {
        for (i, f) in TextureFormat::ALL.iter().enumerate() {
            assert_eq!(*f as usize, i);
        }
    }

    #[test]
    fn swizzles() {
        assert_eq!(
            read_swizzle_for_color_type(ColorType::Gray8, TF::R8).as_string(),
            "rrra"
        );
        assert_eq!(
            read_swizzle_for_color_type(ColorType::Alpha8, TF::R8).as_string(),
            "000r"
        );
        assert_eq!(
            read_swizzle_for_color_type(ColorType::RGB888x, TF::RGBA8).as_string(),
            "rgb1"
        );
        assert_eq!(
            write_swizzle_for_color_type(ColorType::Alpha8, TF::R8).map(|s| s.as_string()),
            Some("a000".to_owned())
        );
        assert!(write_swizzle_for_color_type(ColorType::RGB888x, TF::RGBA8).is_none());
        assert!(write_swizzle_for_color_type(ColorType::RGBA8888, TF::RGBA8_BC1).is_none());
    }

    #[test]
    fn compatibility() {
        assert!(are_color_type_and_format_compatible(
            ColorType::RGB888x,
            TF::External
        ));
        assert!(!are_color_type_and_format_compatible(
            ColorType::RGBA8888,
            TF::R8
        ));
        assert_eq!(
            texture_format_color_type_info(TF::BGR8).1,
            FormatXferOp::SWAP_RB | FormatXferOp::DROP_ALPHA
        );
    }
}
