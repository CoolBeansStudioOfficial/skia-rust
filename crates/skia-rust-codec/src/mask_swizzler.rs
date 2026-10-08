// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkMaskSwizzler.cpp#L20-L575 and src/codec/SkMaskSwizzler.h (chrome/m156)
// Ported from: src/codec/SkMaskSwizzler.cpp, src/codec/SkMaskSwizzler.h
//
// Skia has 21 row procedures (three source widths times seven destinations), one function each.
// Here they share one row routine that reads the source pixel for the width and packs the
// destination with the same codec_priv helpers the C++ calls, so every output byte is produced
// by the same formula. Sampling (`onSetSampleX`) is not ported: it is only reached through
// SkSampledCodec, which lands with the sampled-codec wave.

//! Converts BMP rows whose pixels are packed by bit masks (16, 24 or 32 bits per pixel).

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;

use crate::codec::Options;
use crate::codec_priv::{
    pack_argb_as_bgra, pack_argb_as_rgba, pack888_to_rgb16, premultiply_argb_as_bgra,
    premultiply_argb_as_rgba,
};
use crate::masks::Masks;

// The source pixel width: which `RowProc` family reads the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    // Port of: swizzle_mask16_* (uint16_t source pixels)
    Mask16,
    // Port of: swizzle_mask24_* (three bytes per pixel, little-endian)
    Mask24,
    // Port of: swizzle_mask32_* (uint32_t source pixels)
    Mask32,
}

// The destination layout and alpha treatment: which `RowProc` is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Output {
    // Port of: *_rgba_opaque (alpha forced to 0xFF)
    RgbaOpaque,
    // Port of: *_bgra_opaque
    BgraOpaque,
    // Port of: *_rgba_unpremul
    RgbaUnpremul,
    // Port of: *_bgra_unpremul
    BgraUnpremul,
    // Port of: *_rgba_premul
    RgbaPremul,
    // Port of: *_bgra_premul
    BgraPremul,
    // Port of: *_565
    Rgb565,
}

/// The swizzler for one BMP decode with bit masks. Port of `SkMaskSwizzler`.
#[derive(Debug, Clone, Copy)]
#[doc(alias = "SkMaskSwizzler")]
pub struct MaskSwizzler {
    masks: Masks,
    source: Source,
    output: Output,
    // Port of `fDstWidth`. Sampling is not ported, so this is the subset width.
    dst_width: i32,
    // Port of `fX0`: the first source pixel, which is `fSrcOffset` without sampling.
    x0: i32,
    // Port of `fSampleX`: always 1 until sampling is ported.
    sample_x: i32,
}

// Port of the source-width part of `SkMaskSwizzler::CreateMaskSwizzler`. `None` for a bit count
// the BMP header never produces (the C++ asserts first).
// Port of: src/codec/SkMaskSwizzler.cpp#L583-L600 (the switch on bitsPerPixel)
fn source_for(bits_per_pixel: u32) -> Option<Source> {
    match bits_per_pixel {
        16 => Some(Source::Mask16),
        24 => Some(Source::Mask24),
        32 => Some(Source::Mask32),
        _ => None,
    }
}

// Port of the destination part of `SkMaskSwizzler::CreateMaskSwizzler`. `None` where Skia leaves
// the row proc null (a combination the codec's conversionSupported check rules out).
// Port of: src/codec/SkMaskSwizzler.cpp#L600-L715 (the switch on colorType and alphaType)
fn output_for(dst: &ImageInfo, src_is_opaque: bool) -> Option<Output> {
    match dst.color_type() {
        ColorType::RGBA8888 => {
            if src_is_opaque {
                return Some(Output::RgbaOpaque);
            }
            match dst.alpha_type() {
                AlphaType::Unpremul => Some(Output::RgbaUnpremul),
                AlphaType::Premul => Some(Output::RgbaPremul),
                _ => None,
            }
        }
        ColorType::BGRA8888 => {
            if src_is_opaque {
                return Some(Output::BgraOpaque);
            }
            match dst.alpha_type() {
                AlphaType::Unpremul => Some(Output::BgraUnpremul),
                AlphaType::Premul => Some(Output::BgraPremul),
                _ => None,
            }
        }
        ColorType::RGB565 => Some(Output::Rgb565),
        _ => None,
    }
}

// Reads the source pixel at element index `index` (in units of the source width).
// Port of: the `uint16_t`/`uint32_t` loads and the 24-bit assembly in each row proc. Native
// byte order matches the C++ casts; the 24-bit form is little-endian by construction.
#[inline]
fn read_pixel(src: &[u8], source: Source, index: usize) -> u32 {
    match source {
        Source::Mask16 => {
            let o = index * 2;
            u32::from(u16::from_ne_bytes([src[o], src[o + 1]]))
        }
        Source::Mask24 => {
            let o = index * 3;
            u32::from(src[o]) | (u32::from(src[o + 1]) << 8) | (u32::from(src[o + 2]) << 16)
        }
        Source::Mask32 => {
            let o = index * 4;
            u32::from_ne_bytes([src[o], src[o + 1], src[o + 2], src[o + 3]])
        }
    }
}

impl MaskSwizzler {
    /// Port of `SkMaskSwizzler::CreateMaskSwizzler`. Returns `None` where Skia would create a
    /// swizzler with a null row proc.
    // Port of: src/codec/SkMaskSwizzler.cpp#L583-L720 (chrome/m156)
    #[doc(alias = "SkMaskSwizzler::CreateMaskSwizzler")]
    #[must_use]
    pub fn create(
        dst_info: &ImageInfo,
        src_is_opaque: bool,
        masks: Masks,
        bits_per_pixel: u32,
        options: &Options,
    ) -> Option<Self> {
        let source = source_for(bits_per_pixel)?;
        let output = output_for(dst_info, src_is_opaque)?;
        let (src_offset, src_width) = match options.subset {
            Some(subset) => (subset.left(), subset.width()),
            None => (0, dst_info.width()),
        };
        // Port of the SkMaskSwizzler constructor: fDstWidth(subsetWidth), fSampleX(1),
        // fX0(srcOffset).
        Some(Self {
            masks,
            source,
            output,
            dst_width: src_width,
            x0: src_offset,
            sample_x: 1,
        })
    }

    /// Port of `SkMaskSwizzler::swizzleWidth` (`fDstWidth`): the number of pixels a row produces.
    #[must_use]
    pub fn swizzle_width(&self) -> i32 {
        self.dst_width
    }

    /// Port of `SkMaskSwizzler::swizzle`: converts one source row into `dst`, which starts at the
    /// first destination pixel of the row.
    // Port of: src/codec/SkMaskSwizzler.cpp#L563-L575 (chrome/m156), and the row procs above it
    #[allow(clippy::cast_sign_loss)] // widths and offsets are non-negative (checked by the caller)
    pub fn swizzle(&self, dst: &mut [u8], src: &[u8]) {
        let width = self.dst_width as usize;
        let start_x = self.x0 as usize;
        let sample_x = self.sample_x as usize;
        for i in 0..width {
            let p = read_pixel(src, self.source, start_x + i * sample_x);
            let red = u32::from(self.masks.get_red(p));
            let green = u32::from(self.masks.get_green(p));
            let blue = u32::from(self.masks.get_blue(p));
            if self.output == Output::Rgb565 {
                let v = pack888_to_rgb16(red, green, blue);
                dst[i * 2..i * 2 + 2].copy_from_slice(&v.to_ne_bytes());
                continue;
            }
            let alpha = u32::from(self.masks.get_alpha(p));
            let v = match self.output {
                Output::RgbaOpaque => pack_argb_as_rgba(0xFF, red, green, blue),
                Output::BgraOpaque => pack_argb_as_bgra(0xFF, red, green, blue),
                Output::RgbaUnpremul => pack_argb_as_rgba(alpha, red, green, blue),
                Output::BgraUnpremul => pack_argb_as_bgra(alpha, red, green, blue),
                Output::RgbaPremul => premultiply_argb_as_rgba(alpha, red, green, blue),
                Output::BgraPremul => premultiply_argb_as_bgra(alpha, red, green, blue),
                Output::Rgb565 => 0,
            };
            dst[i * 4..i * 4 + 4].copy_from_slice(&v.to_ne_bytes());
        }
    }
}
