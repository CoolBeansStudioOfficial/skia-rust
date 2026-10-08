// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkSwizzler.cpp#L91-L197 (the kBit routines), #L829-L867 (SkSwizzler::Make, the
// 1-bit gray cases), #L1169-L1260 (the SkSwizzler constructor, Make and swizzle), SkSwizzler.h
// (chrome/m156)
// Ported from: src/codec/SkSwizzler.cpp, src/codec/SkSwizzler.h

//! Converts rows of encoded pixels into the destination colour type.
//!
//! Only the 1-bit gray routines are ported so far (WBMP). The other swizzles (BMP palettes and
//! masks, PNG and JPEG channel layouts) come with the codecs that use them, and sampling
//! (`onSetSampleX`) comes with the scaled decodes. Every routine here writes the same bytes as its
//! C++ counterpart on any host, because the N32 values written are the same bytes in both orders
//! (black is `0xFF000000` and white is `0xFFFFFFFF`).

use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;

use crate::codec::Options;
use crate::encoded_info::{Color, EncodedInfo};

/// A row routine. Port of `SkSwizzler::RowProc`: writes `dst_width` pixels to `dst` from `src`,
/// reading `delta_src` bits per pixel starting at bit `offset` of `src`.
pub type RowProc =
    fn(dst: &mut [u8], src: &[u8], dst_width: usize, delta_src: usize, offset: usize);

// Port of: src/codec/SkSwizzler.cpp#L94-L121 (swizzle_bit_to_grayscale)
const GRAYSCALE_BLACK: u8 = 0;
const GRAYSCALE_WHITE: u8 = 0xFF;

/// Reads the bit at `pixel` (0-based from `offset`), as the kBit loops do.
#[inline]
fn bit_at(src: &[u8], bit_index_base: usize, delta_src: usize, x: usize) -> bool {
    let bit = bit_index_base + x * delta_src;
    let byte = src[bit / 8];
    (byte >> (7 - (bit % 8))) & 1 != 0
}

// Port of: src/codec/SkSwizzler.cpp#L99-L118 (swizzle_bit_to_grayscale; the loop's running bit
// index is the same as `offset + x * deltaSrc`, which the C++ accumulates)
fn swizzle_bit_to_grayscale(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    delta_src: usize,
    offset: usize,
) {
    for (x, out) in dst.iter_mut().take(dst_width).enumerate() {
        *out = if bit_at(src, offset, delta_src, x) {
            GRAYSCALE_WHITE
        } else {
            GRAYSCALE_BLACK
        };
    }
}

// Port of: src/codec/SkSwizzler.cpp#L124-L145 (swizzle_bit_to_n32; SK_ColorWHITE, SK_ColorBLACK)
fn swizzle_bit_to_n32(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    delta_src: usize,
    offset: usize,
) {
    const SK_COLOR_WHITE: u32 = 0xFFFF_FFFF;
    const SK_COLOR_BLACK: u32 = 0xFF00_0000;
    for (x, out) in dst
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .take(dst_width)
        .enumerate()
    {
        let c = if bit_at(src, offset, delta_src, x) {
            SK_COLOR_WHITE
        } else {
            SK_COLOR_BLACK
        };
        *out = c.to_ne_bytes();
    }
}

// Port of: src/codec/SkSwizzler.cpp#L144-L168 (swizzle_bit_to_565; RGB565_BLACK, RGB565_WHITE)
fn swizzle_bit_to_565(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    delta_src: usize,
    offset: usize,
) {
    const RGB565_BLACK: u16 = 0;
    const RGB565_WHITE: u16 = 0xFFFF;
    for (x, out) in dst
        .as_chunks_mut::<2>()
        .0
        .iter_mut()
        .take(dst_width)
        .enumerate()
    {
        let c = if bit_at(src, offset, delta_src, x) {
            RGB565_WHITE
        } else {
            RGB565_BLACK
        };
        *out = c.to_ne_bytes();
    }
}

// Port of: src/codec/SkSwizzler.cpp#L170-L197 (swizzle_bit_to_f16; four half-floats per pixel)
fn swizzle_bit_to_f16(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    delta_src: usize,
    offset: usize,
) {
    // SK_Half1 is the half-float encoding of 1.0.
    const SK_HALF1: u64 = 0x3C00;
    const K_WHITE: u64 = SK_HALF1 | (SK_HALF1 << 16) | (SK_HALF1 << 32) | (SK_HALF1 << 48);
    const K_BLACK: u64 = SK_HALF1 << 48;
    for (x, out) in dst
        .as_chunks_mut::<8>()
        .0
        .iter_mut()
        .take(dst_width)
        .enumerate()
    {
        let c = if bit_at(src, offset, delta_src, x) {
            K_WHITE
        } else {
            K_BLACK
        };
        *out = c.to_ne_bytes();
    }
}

/// The swizzler for one decode. Port of `SkSwizzler`, without sampling and the fast-path
/// variants (the 1-bit routines have none).
#[derive(Debug)]
#[doc(alias = "SkSwizzler")]
pub struct Swizzler {
    proc: RowProc,
    src_offset_units: usize,
    dst_offset_bytes: usize,
    swizzle_width: usize,
    src_bpp: usize,
}

impl Swizzler {
    /// Port of `SkSwizzler::Make(encodedInfo, ctable, dstInfo, options)` for the 1-bit gray
    /// sources. Returns `None` for any other combination, as Skia does for unsupported ones.
    // Port of: src/codec/SkSwizzler.cpp#L829-L867 (the kGray_Color, 1-bit cases) and
    // src/codec/SkSwizzler.cpp#L1169-L1190 (Make, with srcOffset/srcWidth from the subset)
    #[doc(alias = "SkSwizzler::Make")]
    #[must_use]
    // Image sizes and subset offsets are non-negative, so the casts to usize are exact.
    #[allow(clippy::cast_sign_loss)]
    pub fn make(encoded: &EncodedInfo, dst_info: &ImageInfo, options: &Options) -> Option<Self> {
        if encoded.color() != Color::Gray || encoded.bits_per_component() != 1 {
            return None;
        }
        let proc: RowProc = match dst_info.color_type() {
            ColorType::RGBA8888 | ColorType::BGRA8888 => swizzle_bit_to_n32,
            ColorType::RGB565 => swizzle_bit_to_565,
            ColorType::Gray8 => swizzle_bit_to_grayscale,
            ColorType::RGBAF16Norm => swizzle_bit_to_f16,
            _ => return None,
        };
        // Port of: src/codec/SkSwizzler.cpp#L1163-L1166
        let bits_per_pixel = encoded.bits_per_pixel();
        let src_bpp = if bits_per_pixel.is_multiple_of(8) {
            usize::from(bits_per_pixel) / 8
        } else {
            usize::from(bits_per_pixel)
        };
        // Port of: src/codec/SkSwizzler.cpp#L1172-L1189 (Make)
        let mut src_offset = 0usize;
        let mut src_width = dst_info.width() as usize;
        if let Some(subset) = options.subset {
            src_offset = subset.left() as usize;
            src_width = subset.width() as usize;
        }
        Some(Self {
            proc,
            // Port of: SkSwizzler's constructor (#L1192-L1209): fSrcOffsetUnits, fDstOffsetBytes
            src_offset_units: src_offset * src_bpp,
            dst_offset_bytes: 0,
            swizzle_width: src_width,
            src_bpp,
        })
    }

    /// Port of `SkSwizzler::swizzle(dst, src)`: converts one encoded row into `dst`, which starts
    /// at the first destination pixel of the row.
    // Port of: src/codec/SkSwizzler.cpp#L1255-L1259 (chrome/m156)
    pub fn swizzle(&self, dst: &mut [u8], src: &[u8]) {
        (self.proc)(
            &mut dst[self.dst_offset_bytes..],
            src,
            self.swizzle_width,
            self.src_bpp,
            self.src_offset_units,
        );
    }
}
