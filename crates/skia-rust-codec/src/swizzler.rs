// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkSwizzler.cpp#L31-L1259 (chrome/m156), src/codec/SkSwizzler.h (chrome/m156)
// Ported from: src/codec/SkSwizzler.cpp, src/codec/SkSwizzler.h
//
// Skia keeps a "fast" and a "slow" row proc per case. The fast ones call SkOpts kernels and are
// used only when there is no sampling. Their output is byte-identical to the slow proc they pair
// with (the premultiply rounding `(x + 127) / 255` equals `SkMulDiv255Round` for every product in
// range, checked exhaustively), so this port keeps one proc per case and selects the same bytes.
// The fast/slow choice in `onSetSampleX` therefore has no observable effect and is not ported.

//! Converts rows of encoded pixels into the destination colour type.

// SkSwizzler keeps widths and offsets as C++ `int`s and converts them to byte counts. Every value
// the casts see is a non-negative image dimension or subset coordinate, which the callers check,
// so the casts mirror the C++ arithmetic.
#![allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]

use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::math::mul_div_255_round;
use skia_rust_core::rect::IRect;

use crate::codec::{Options, ZeroInitialized};
use crate::codec_priv::{
    get_sampled_dimension, get_start_coord, pack_argb_as_bgra, pack_argb_as_rgba, pack_argb32,
    pack888_to_rgb16, pixel32_to_pixel16, premultiply_argb_as_bgra, premultiply_argb_as_rgba,
};
use crate::encoded_info::{Alpha, Color, EncodedInfo};
use crate::sampler::SamplerBase;

/// A row routine. Port of `SkSwizzler::RowProc`: writes `dst_width` pixels to `dst` from `src`,
/// reading `delta_src` bytes (or bits, for sub-byte sources) per pixel starting at `offset`. `bpp`
/// is the source bytes (or bits) per pixel, and `ctable` is the palette when there is one.
pub type RowProc = fn(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    bpp: usize,
    delta_src: usize,
    offset: usize,
    ctable: &[u32],
);

// Writes one native N32 pixel at pixel index `x` (`SkPMColor` is a native-endian u32).
#[inline]
fn put_u32(dst: &mut [u8], x: usize, value: u32) {
    dst[x * 4..x * 4 + 4].copy_from_slice(&value.to_ne_bytes());
}

// Writes one native 16-bit pixel at pixel index `x`.
#[inline]
fn put_u16(dst: &mut [u8], x: usize, value: u16) {
    dst[x * 2..x * 2 + 2].copy_from_slice(&value.to_ne_bytes());
}

// Copies `n` bytes per pixel, the common shape of `sample1`..`sample8` and `copy`.
// Port of: src/codec/SkSwizzler.cpp#L31-L89 (copy, sample1, sample2, sample4, sample6, sample8)
fn sample_bytes(
    dst: &mut [u8],
    src: &[u8],
    width: usize,
    delta_src: usize,
    offset: usize,
    n: usize,
) {
    for x in 0..width {
        let pos = offset + x * delta_src;
        dst[x * n..x * n + n].copy_from_slice(&src[pos..pos + n]);
    }
}

// Port of: src/codec/SkSwizzler.cpp#L40-L48 (sample1)
fn sample1(
    dst: &mut [u8],
    src: &[u8],
    width: usize,
    _bpp: usize,
    delta: usize,
    offset: usize,
    _c: &[u32],
) {
    sample_bytes(dst, src, width, delta, offset, 1);
}

// Port of: src/codec/SkSwizzler.cpp#L50-L58 (sample2)
fn sample2(
    dst: &mut [u8],
    src: &[u8],
    width: usize,
    _bpp: usize,
    delta: usize,
    offset: usize,
    _c: &[u32],
) {
    sample_bytes(dst, src, width, delta, offset, 2);
}

// Port of: src/codec/SkSwizzler.cpp#L60-L68 (sample4)
fn sample4(
    dst: &mut [u8],
    src: &[u8],
    width: usize,
    _bpp: usize,
    delta: usize,
    offset: usize,
    _c: &[u32],
) {
    sample_bytes(dst, src, width, delta, offset, 4);
}

// Port of: src/codec/SkSwizzler.cpp#L70-L79 (sample6: 16-bit RGB, no alpha)
fn sample6(
    dst: &mut [u8],
    src: &[u8],
    width: usize,
    _bpp: usize,
    delta: usize,
    offset: usize,
    _c: &[u32],
) {
    sample_bytes(dst, src, width, delta, offset, 6);
}

// Port of: src/codec/SkSwizzler.cpp#L81-L89 (sample8: 16-bit RGBA)
fn sample8(
    dst: &mut [u8],
    src: &[u8],
    width: usize,
    _bpp: usize,
    delta: usize,
    offset: usize,
    _c: &[u32],
) {
    sample_bytes(dst, src, width, delta, offset, 8);
}

// kBit
// These routines exclusively choose between white and black

const GRAYSCALE_BLACK: u8 = 0;
const GRAYSCALE_WHITE: u8 = 0xFF;

/// Reads the bit at `bit_index_base + x * delta_src` (0-based from the most significant bit of
/// each byte), as the kBit loops do.
#[inline]
fn bit_at(src: &[u8], bit_index_base: usize, delta_src: usize, x: usize) -> bool {
    let bit = bit_index_base + x * delta_src;
    let byte = src[bit / 8];
    (byte >> (7 - (bit % 8))) & 1 != 0
}

// Port of: src/codec/SkSwizzler.cpp#L99-L118 (swizzle_bit_to_grayscale)
fn swizzle_bit_to_grayscale(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for (x, out) in dst.iter_mut().take(dst_width).enumerate() {
        *out = if bit_at(src, offset, delta_src, x) {
            GRAYSCALE_WHITE
        } else {
            GRAYSCALE_BLACK
        };
    }
}

// Port of: src/codec/SkSwizzler.cpp#L124-L142 (swizzle_bit_to_n32; SK_ColorWHITE, SK_ColorBLACK)
fn swizzle_bit_to_n32(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    const SK_COLOR_WHITE: u32 = 0xFFFF_FFFF;
    const SK_COLOR_BLACK: u32 = 0xFF00_0000;
    for x in 0..dst_width {
        let c = if bit_at(src, offset, delta_src, x) {
            SK_COLOR_WHITE
        } else {
            SK_COLOR_BLACK
        };
        put_u32(dst, x, c);
    }
}

// Port of: src/codec/SkSwizzler.cpp#L144-L165 (swizzle_bit_to_565; RGB565_BLACK, RGB565_WHITE)
fn swizzle_bit_to_565(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    const RGB565_BLACK: u16 = 0;
    const RGB565_WHITE: u16 = 0xFFFF;
    for x in 0..dst_width {
        let c = if bit_at(src, offset, delta_src, x) {
            RGB565_WHITE
        } else {
            RGB565_BLACK
        };
        put_u16(dst, x, c);
    }
}

// Port of: src/codec/SkSwizzler.cpp#L170-L197 (swizzle_bit_to_f16; four half-floats per pixel)
fn swizzle_bit_to_f16(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    // SK_Half1 is the half-float encoding of 1.0.
    const SK_HALF1: u64 = 0x3C00;
    const K_WHITE: u64 = SK_HALF1 | (SK_HALF1 << 16) | (SK_HALF1 << 32) | (SK_HALF1 << 48);
    const K_BLACK: u64 = SK_HALF1 << 48;
    for x in 0..dst_width {
        let c = if bit_at(src, offset, delta_src, x) {
            K_WHITE
        } else {
            K_BLACK
        };
        dst[x * 8..x * 8 + 8].copy_from_slice(&c.to_ne_bytes());
    }
}

// kIndex1, kIndex2, kIndex4: the same walk as the kIndex routines, but reading `bpp` bits.

// Port of: src/codec/SkSwizzler.cpp#L201-L220 (swizzle_small_index_to_565)
fn swizzle_small_index_to_565(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    bpp: usize,
    delta_src: usize,
    offset: usize,
    ctable: &[u32],
) {
    let mut src_pos = offset / 8;
    let mut bit_index = offset % 8;
    let mut curr_byte = src[src_pos];
    let mask = (1u8 << bpp) - 1;
    let mut index = (curr_byte >> (8 - bpp - bit_index)) & mask;
    put_u16(dst, 0, pixel32_to_pixel16(ctable[usize::from(index)]));

    for x in 1..dst_width {
        let bit_offset = bit_index + delta_src;
        bit_index = bit_offset % 8;
        src_pos += bit_offset / 8;
        curr_byte = src[src_pos];
        index = (curr_byte >> (8 - bpp - bit_index)) & mask;
        put_u16(dst, x, pixel32_to_pixel16(ctable[usize::from(index)]));
    }
}

// Port of: src/codec/SkSwizzler.cpp#L222-L241 (swizzle_small_index_to_n32)
fn swizzle_small_index_to_n32(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    bpp: usize,
    delta_src: usize,
    offset: usize,
    ctable: &[u32],
) {
    let mut src_pos = offset / 8;
    let mut bit_index = offset % 8;
    let mut curr_byte = src[src_pos];
    let mask = (1u8 << bpp) - 1;
    let mut index = (curr_byte >> (8 - bpp - bit_index)) & mask;
    put_u32(dst, 0, ctable[usize::from(index)]);

    for x in 1..dst_width {
        let bit_offset = bit_index + delta_src;
        bit_index = bit_offset % 8;
        src_pos += bit_offset / 8;
        curr_byte = src[src_pos];
        index = (curr_byte >> (8 - bpp - bit_index)) & mask;
        put_u32(dst, x, ctable[usize::from(index)]);
    }
}

// kIndex

// Port of: src/codec/SkSwizzler.cpp#L245-L256 (swizzle_index_to_n32)
fn swizzle_index_to_n32(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    ctable: &[u32],
) {
    for x in 0..dst_width {
        put_u32(dst, x, ctable[usize::from(src[offset + x * delta_src])]);
    }
}

// Port of: src/codec/SkSwizzler.cpp#L258-L271 (swizzle_index_to_n32_skipZ: a zero entry leaves the
// destination untouched)
fn swizzle_index_to_n32_skip_z(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    ctable: &[u32],
) {
    for x in 0..dst_width {
        let c = ctable[usize::from(src[offset + x * delta_src])];
        if c != 0 {
            put_u32(dst, x, c);
        }
    }
}

// Port of: src/codec/SkSwizzler.cpp#L273-L282 (swizzle_index_to_565)
fn swizzle_index_to_565(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    ctable: &[u32],
) {
    for x in 0..dst_width {
        put_u16(
            dst,
            x,
            pixel32_to_pixel16(ctable[usize::from(src[offset + x * delta_src])]),
        );
    }
}

// kGray

// Port of: src/codec/SkSwizzler.cpp#L286-L296 (swizzle_gray_to_n32). The gray channels are
// symmetric, so the N32 channel order does not change the bytes.
fn swizzle_gray_to_n32(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..dst_width {
        let g = u32::from(src[offset + x * delta_src]);
        put_u32(dst, x, pack_argb32(0xFF, g, g, g));
    }
}

// Port of: src/codec/SkSwizzler.cpp#L311-L321 (swizzle_gray_to_565)
fn swizzle_gray_to_565(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..dst_width {
        let g = u32::from(src[offset + x * delta_src]);
        put_u16(dst, x, pack888_to_rgb16(g, g, g));
    }
}

// kGrayAlpha

// Port of: src/codec/SkSwizzler.cpp#L325-L335 (swizzle_grayalpha_to_n32_unpremul)
fn swizzle_grayalpha_to_n32_unpremul(
    dst: &mut [u8],
    src: &[u8],
    width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..width {
        let p = offset + x * delta_src;
        let (g, a) = (u32::from(src[p]), u32::from(src[p + 1]));
        put_u32(dst, x, pack_argb32(a, g, g, g));
    }
}

// Port of: src/codec/SkSwizzler.cpp#L350-L361 (swizzle_grayalpha_to_n32_premul)
fn swizzle_grayalpha_to_n32_premul(
    dst: &mut [u8],
    src: &[u8],
    width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..width {
        let p = offset + x * delta_src;
        let a = u32::from(src[p + 1]);
        let pmgray = mul_div_255_round(u32::from(src[p + 1]), u32::from(src[p]));
        put_u32(dst, x, pack_argb32(a, pmgray, pmgray, pmgray));
    }
}

// Port of: src/codec/SkSwizzler.cpp#L376-L384 (swizzle_grayalpha_to_a8)
fn swizzle_grayalpha_to_a8(
    dst: &mut [u8],
    src: &[u8],
    width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..width {
        // src[0] is gray, ignored
        dst[x] = src[offset + x * delta_src + 1];
    }
}

// kBGR

// Port of: src/codec/SkSwizzler.cpp#L388-L398 (swizzle_bgr_to_565)
fn swizzle_bgr_to_565(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..dst_width {
        let p = offset + x * delta_src;
        put_u16(
            dst,
            x,
            pack888_to_rgb16(
                u32::from(src[p + 2]),
                u32::from(src[p + 1]),
                u32::from(src[p]),
            ),
        );
    }
}

// kRGB

// Port of: src/codec/SkSwizzler.cpp#L402-L412 (swizzle_rgb_to_rgba)
fn swizzle_rgb_to_rgba(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..dst_width {
        let p = offset + x * delta_src;
        put_u32(
            dst,
            x,
            pack_argb_as_rgba(
                0xFF,
                u32::from(src[p]),
                u32::from(src[p + 1]),
                u32::from(src[p + 2]),
            ),
        );
    }
}

// Port of: src/codec/SkSwizzler.cpp#L414-L424 (swizzle_rgb_to_bgra)
fn swizzle_rgb_to_bgra(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..dst_width {
        let p = offset + x * delta_src;
        put_u32(
            dst,
            x,
            pack_argb_as_bgra(
                0xFF,
                u32::from(src[p]),
                u32::from(src[p + 1]),
                u32::from(src[p + 2]),
            ),
        );
    }
}

// Port of: src/codec/SkSwizzler.cpp#L448-L458 (swizzle_rgb_to_565)
fn swizzle_rgb_to_565(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..dst_width {
        let p = offset + x * delta_src;
        put_u16(
            dst,
            x,
            pack888_to_rgb16(
                u32::from(src[p]),
                u32::from(src[p + 1]),
                u32::from(src[p + 2]),
            ),
        );
    }
}

// kRGBA

// Port of: src/codec/SkSwizzler.cpp#L462-L472 (swizzle_rgba_to_rgba_premul)
fn swizzle_rgba_to_rgba_premul(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..dst_width {
        let p = offset + x * delta_src;
        put_u32(
            dst,
            x,
            premultiply_argb_as_rgba(
                u32::from(src[p + 3]),
                u32::from(src[p]),
                u32::from(src[p + 1]),
                u32::from(src[p + 2]),
            ),
        );
    }
}

// Port of: src/codec/SkSwizzler.cpp#L474-L484 (swizzle_rgba_to_bgra_premul)
fn swizzle_rgba_to_bgra_premul(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..dst_width {
        let p = offset + x * delta_src;
        put_u32(
            dst,
            x,
            premultiply_argb_as_bgra(
                u32::from(src[p + 3]),
                u32::from(src[p]),
                u32::from(src[p + 1]),
                u32::from(src[p + 2]),
            ),
        );
    }
}

// Port of: src/codec/SkSwizzler.cpp#L508-L519 (swizzle_rgba_to_bgra_unpremul)
fn swizzle_rgba_to_bgra_unpremul(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..dst_width {
        let p = offset + x * delta_src;
        put_u32(
            dst,
            x,
            pack_argb_as_bgra(
                u32::from(src[p + 3]),
                u32::from(src[p]),
                u32::from(src[p + 1]),
                u32::from(src[p + 2]),
            ),
        );
    }
}

// 16-bits per component kRGB and kRGBA. Only the high byte of each component is kept.

// Port of: src/codec/SkSwizzler.cpp#L534-L547 (swizzle_rgb16_to_rgba)
fn swizzle_rgb16_to_rgba(
    dst: &mut [u8],
    src: &[u8],
    width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..width {
        let p = offset + x * delta_src;
        let v = 0xFF00_0000
            | (u32::from(src[p + 4]) << 16)
            | (u32::from(src[p + 2]) << 8)
            | u32::from(src[p]);
        put_u32(dst, x, v);
    }
}

// Port of: src/codec/SkSwizzler.cpp#L549-L562 (swizzle_rgb16_to_bgra)
fn swizzle_rgb16_to_bgra(
    dst: &mut [u8],
    src: &[u8],
    width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..width {
        let p = offset + x * delta_src;
        let v = 0xFF00_0000
            | (u32::from(src[p]) << 16)
            | (u32::from(src[p + 2]) << 8)
            | u32::from(src[p + 4]);
        put_u32(dst, x, v);
    }
}

// Port of: src/codec/SkSwizzler.cpp#L564-L577 (swizzle_rgb16_to_565)
fn swizzle_rgb16_to_565(
    dst: &mut [u8],
    src: &[u8],
    width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..width {
        let p = offset + x * delta_src;
        put_u16(
            dst,
            x,
            pack888_to_rgb16(
                u32::from(src[p]),
                u32::from(src[p + 2]),
                u32::from(src[p + 4]),
            ),
        );
    }
}

// Port of: src/codec/SkSwizzler.cpp#L579-L592 (swizzle_rgba16_to_rgba_unpremul)
fn swizzle_rgba16_to_rgba_unpremul(
    dst: &mut [u8],
    src: &[u8],
    width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..width {
        let p = offset + x * delta_src;
        let v = (u32::from(src[p + 6]) << 24)
            | (u32::from(src[p + 4]) << 16)
            | (u32::from(src[p + 2]) << 8)
            | u32::from(src[p]);
        put_u32(dst, x, v);
    }
}

// Port of: src/codec/SkSwizzler.cpp#L594-L607 (swizzle_rgba16_to_rgba_premul)
fn swizzle_rgba16_to_rgba_premul(
    dst: &mut [u8],
    src: &[u8],
    width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..width {
        let p = offset + x * delta_src;
        put_u32(
            dst,
            x,
            premultiply_argb_as_rgba(
                u32::from(src[p + 6]),
                u32::from(src[p]),
                u32::from(src[p + 2]),
                u32::from(src[p + 4]),
            ),
        );
    }
}

// Port of: src/codec/SkSwizzler.cpp#L609-L622 (swizzle_rgba16_to_bgra_unpremul)
fn swizzle_rgba16_to_bgra_unpremul(
    dst: &mut [u8],
    src: &[u8],
    width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..width {
        let p = offset + x * delta_src;
        let v = (u32::from(src[p + 6]) << 24)
            | (u32::from(src[p]) << 16)
            | (u32::from(src[p + 2]) << 8)
            | u32::from(src[p + 4]);
        put_u32(dst, x, v);
    }
}

// Port of: src/codec/SkSwizzler.cpp#L624-L637 (swizzle_rgba16_to_bgra_premul)
fn swizzle_rgba16_to_bgra_premul(
    dst: &mut [u8],
    src: &[u8],
    width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..width {
        let p = offset + x * delta_src;
        put_u32(
            dst,
            x,
            premultiply_argb_as_bgra(
                u32::from(src[p + 6]),
                u32::from(src[p]),
                u32::from(src[p + 2]),
                u32::from(src[p + 4]),
            ),
        );
    }
}

// kInvertedCMYK

// Port of: src/codec/SkSwizzler.cpp#L684-L698 (swizzle_cmyk_to_rgba)
fn swizzle_cmyk_to_rgba(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..dst_width {
        let pos = offset + x * delta_src;
        let alpha = u32::from(src[pos + 3]);
        let red = mul_div_255_round(u32::from(src[pos]), alpha);
        let green = mul_div_255_round(u32::from(src[pos + 1]), alpha);
        let blue = mul_div_255_round(u32::from(src[pos + 2]), alpha);
        put_u32(dst, x, pack_argb_as_rgba(0xFF, red, green, blue));
    }
}

// Port of: src/codec/SkSwizzler.cpp#L700-L714 (swizzle_cmyk_to_bgra)
fn swizzle_cmyk_to_bgra(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..dst_width {
        let pos = offset + x * delta_src;
        let alpha = u32::from(src[pos + 3]);
        let red = mul_div_255_round(u32::from(src[pos]), alpha);
        let green = mul_div_255_round(u32::from(src[pos + 1]), alpha);
        let blue = mul_div_255_round(u32::from(src[pos + 2]), alpha);
        put_u32(dst, x, pack_argb_as_bgra(0xFF, red, green, blue));
    }
}

// Port of: src/codec/SkSwizzler.cpp#L738-L752 (swizzle_cmyk_to_565)
fn swizzle_cmyk_to_565(
    dst: &mut [u8],
    src: &[u8],
    dst_width: usize,
    _bpp: usize,
    delta_src: usize,
    offset: usize,
    _c: &[u32],
) {
    for x in 0..dst_width {
        let pos = offset + x * delta_src;
        let alpha = u32::from(src[pos + 3]);
        let red = mul_div_255_round(u32::from(src[pos]), alpha);
        let green = mul_div_255_round(u32::from(src[pos + 1]), alpha);
        let blue = mul_div_255_round(u32::from(src[pos + 2]), alpha);
        put_u16(dst, x, pack888_to_rgb16(red, green, blue));
    }
}

// Skipping leading zero pixels. A zero-initialised destination already holds the zero, so these
// wrappers advance past the leading transparent source pixels and let `proc` handle the rest.

/// Port of `SkSwizzler::SkipLeadingGrayAlphaZerosThen`: a gray-alpha pixel of two zero bytes is
/// skipped (its destination pixel is left untouched).
// Port of: src/codec/SkSwizzler.cpp#L754-L771 (chrome/m156)
// Eight arguments mirror the C++ template's argument list.
#[allow(clippy::too_many_arguments)]
fn skip_leading_gray_alpha_zeros_then(
    proc: RowProc,
    dst: &mut [u8],
    src: &[u8],
    mut width: usize,
    bpp: usize,
    delta_src: usize,
    offset: usize,
    ctable: &[u32],
) {
    let mut src_pos = offset;
    let mut dst_pos = 0usize;
    while width > 0 && src[src_pos] == 0 && src[src_pos + 1] == 0 {
        width -= 1;
        dst_pos += 4;
        src_pos += delta_src;
    }
    proc(
        &mut dst[dst_pos..],
        &src[src_pos..],
        width,
        bpp,
        delta_src,
        0,
        ctable,
    );
}

/// Port of `SkSwizzler::SkipLeading8888ZerosThen`: a four-byte zero source pixel is skipped.
// Port of: src/codec/SkSwizzler.cpp#L773-L790 (chrome/m156)
// Eight arguments mirror the C++ template's argument list.
#[allow(clippy::too_many_arguments)]
fn skip_leading_8888_zeros_then(
    proc: RowProc,
    dst: &mut [u8],
    src: &[u8],
    mut dst_width: usize,
    bpp: usize,
    delta_src: usize,
    offset: usize,
    ctable: &[u32],
) {
    let mut src_pos = offset;
    let mut dst_pos = 0usize;
    while dst_width > 0 && src[src_pos..src_pos + 4] == [0, 0, 0, 0] {
        dst_width -= 1;
        dst_pos += 4;
        // `src32 += deltaSrc/4` advances by whole words.
        src_pos += (delta_src / 4) * 4;
    }
    proc(
        &mut dst[dst_pos..],
        &src[src_pos..],
        dst_width,
        bpp,
        delta_src,
        0,
        ctable,
    );
}

// One concrete function per (proc, skip) pair, since `RowProc` is a plain function pointer.
macro_rules! skipping_proc {
    ($name:ident, $skip:ident, $inner:ident) => {
        fn $name(
            dst: &mut [u8],
            src: &[u8],
            width: usize,
            bpp: usize,
            delta_src: usize,
            offset: usize,
            ctable: &[u32],
        ) {
            $skip($inner, dst, src, width, bpp, delta_src, offset, ctable);
        }
    };
}

skipping_proc!(
    skip_zeros_grayalpha_unpremul,
    skip_leading_gray_alpha_zeros_then,
    swizzle_grayalpha_to_n32_unpremul
);
skipping_proc!(
    skip_zeros_grayalpha_premul,
    skip_leading_gray_alpha_zeros_then,
    swizzle_grayalpha_to_n32_premul
);
skipping_proc!(
    skip_zeros_rgba_to_rgba_premul,
    skip_leading_8888_zeros_then,
    swizzle_rgba_to_rgba_premul
);
skipping_proc!(skip_zeros_sample4, skip_leading_8888_zeros_then, sample4);
skipping_proc!(
    skip_zeros_rgba_to_bgra_premul,
    skip_leading_8888_zeros_then,
    swizzle_rgba_to_bgra_premul
);
skipping_proc!(
    skip_zeros_rgba_to_bgra_unpremul,
    skip_leading_8888_zeros_then,
    swizzle_rgba_to_bgra_unpremul
);

/// The swizzler for one decode. Port of `SkSwizzler`.
///
/// It is also a sampler (`SkSampler`): [`Swizzler::set_sample_x`] selects every `sampleX`th pixel,
/// and the sample-Y state lives in [`SamplerBase`].
#[derive(Debug)]
#[doc(alias = "SkSwizzler")]
pub struct Swizzler {
    sampler: SamplerBase,
    proc: RowProc,
    color_table: Vec<u32>,
    src_offset: usize,
    src_offset_units: usize,
    dst_offset: usize,
    dst_offset_bytes: usize,
    src_width: usize,
    dst_width: usize,
    swizzle_width: usize,
    allocated_width: usize,
    sample_x: usize,
    src_bpp: usize,
    dst_bpp: usize,
}

impl Swizzler {
    /// Port of `SkSwizzler::Make(encodedInfo, ctable, dstInfo, options, frame)`. Returns `None`
    /// for an unsupported combination, and for a palette without a colour table.
    // Port of: src/codec/SkSwizzler.cpp#L829-L1167 (chrome/m156)
    #[doc(alias = "SkSwizzler::Make")]
    #[must_use]
    // One arm per (encoded colour, destination colour) case, as in the C++ switch.
    #[allow(clippy::too_many_lines)]
    pub fn make(
        encoded: &EncodedInfo,
        ctable: Option<&[u32]>,
        dst_info: &ImageInfo,
        options: &Options,
        frame: Option<IRect>,
    ) -> Option<Self> {
        // Port of: src/codec/SkSwizzler.cpp#L833-L835
        if encoded.color() == Color::Palette && ctable.is_none() {
            return None;
        }
        let zero_init = options.zero_initialized;
        let dst_ct = dst_info.color_type();
        // Port of: src/codec/SkSwizzler.cpp#L837-L838
        let premultiply = encoded.alpha() != Alpha::Opaque
            && dst_info.alpha_type() == skia_rust_core::alpha_type::AlphaType::Premul;
        let bits = encoded.bits_per_component();
        let proc: RowProc = match encoded.color() {
            Color::Gray => match bits {
                1 => match dst_ct {
                    ColorType::RGBA8888 | ColorType::BGRA8888 => swizzle_bit_to_n32,
                    ColorType::RGB565 => swizzle_bit_to_565,
                    ColorType::Gray8 => swizzle_bit_to_grayscale,
                    ColorType::RGBAF16Norm => swizzle_bit_to_f16,
                    _ => return None,
                },
                8 => match dst_ct {
                    ColorType::RGBA8888 | ColorType::BGRA8888 => swizzle_gray_to_n32,
                    ColorType::Gray8 => sample1,
                    ColorType::RGB565 => swizzle_gray_to_565,
                    _ => return None,
                },
                _ => return None,
            },
            Color::XAlpha | Color::GrayAlpha => match dst_ct {
                ColorType::RGBA8888 | ColorType::BGRA8888 => {
                    if premultiply {
                        if zero_init == ZeroInitialized::Yes {
                            skip_zeros_grayalpha_premul
                        } else {
                            swizzle_grayalpha_to_n32_premul
                        }
                    } else if zero_init == ZeroInitialized::Yes {
                        skip_zeros_grayalpha_unpremul
                    } else {
                        swizzle_grayalpha_to_n32_unpremul
                    }
                }
                ColorType::Alpha8 => swizzle_grayalpha_to_a8,
                _ => return None,
            },
            Color::Palette => match bits {
                1 | 2 | 4 => match dst_ct {
                    ColorType::RGBA8888 | ColorType::BGRA8888 => swizzle_small_index_to_n32,
                    ColorType::RGB565 => swizzle_small_index_to_565,
                    _ => return None,
                },
                8 => match dst_ct {
                    ColorType::RGBA8888 | ColorType::BGRA8888 | ColorType::BGR101010xXR => {
                        if zero_init == ZeroInitialized::Yes {
                            swizzle_index_to_n32_skip_z
                        } else {
                            swizzle_index_to_n32
                        }
                    }
                    ColorType::RGB565 => swizzle_index_to_565,
                    _ => return None,
                },
                _ => return None,
            },
            // Treat 565 exactly like RGB (since it's still encoded as 8 bits per component).
            Color::Color565 | Color::RGB => match dst_ct {
                ColorType::RGBA8888 => {
                    if bits == 16 {
                        swizzle_rgb16_to_rgba
                    } else {
                        swizzle_rgb_to_rgba
                    }
                }
                ColorType::BGRA8888 => {
                    if bits == 16 {
                        swizzle_rgb16_to_bgra
                    } else {
                        swizzle_rgb_to_bgra
                    }
                }
                ColorType::RGB565 => {
                    if bits == 16 {
                        swizzle_rgb16_to_565
                    } else {
                        swizzle_rgb_to_565
                    }
                }
                _ => return None,
            },
            Color::RGBA => match dst_ct {
                ColorType::RGBA8888 => {
                    if bits == 16 {
                        if premultiply {
                            swizzle_rgba16_to_rgba_premul
                        } else {
                            swizzle_rgba16_to_rgba_unpremul
                        }
                    } else if premultiply {
                        if zero_init == ZeroInitialized::Yes {
                            skip_zeros_rgba_to_rgba_premul
                        } else {
                            swizzle_rgba_to_rgba_premul
                        }
                    } else if zero_init == ZeroInitialized::Yes {
                        skip_zeros_sample4
                    } else {
                        sample4
                    }
                }
                ColorType::BGRA8888 => {
                    if bits == 16 {
                        if premultiply {
                            swizzle_rgba16_to_bgra_premul
                        } else {
                            swizzle_rgba16_to_bgra_unpremul
                        }
                    } else if premultiply {
                        if zero_init == ZeroInitialized::Yes {
                            skip_zeros_rgba_to_bgra_premul
                        } else {
                            swizzle_rgba_to_bgra_premul
                        }
                    } else if zero_init == ZeroInitialized::Yes {
                        skip_zeros_rgba_to_bgra_unpremul
                    } else {
                        swizzle_rgba_to_bgra_unpremul
                    }
                }
                _ => return None,
            },
            // BGR and BGRX share a body: the fourth byte of BGRX is never read.
            Color::BGR | Color::BGRX => match dst_ct {
                ColorType::BGRA8888 => swizzle_rgb_to_rgba,
                ColorType::RGBA8888 => swizzle_rgb_to_bgra,
                ColorType::RGB565 => swizzle_bgr_to_565,
                _ => return None,
            },
            Color::BGRA => match dst_ct {
                ColorType::BGRA8888 => {
                    if premultiply {
                        if zero_init == ZeroInitialized::Yes {
                            skip_zeros_rgba_to_rgba_premul
                        } else {
                            swizzle_rgba_to_rgba_premul
                        }
                    } else if zero_init == ZeroInitialized::Yes {
                        skip_zeros_sample4
                    } else {
                        sample4
                    }
                }
                ColorType::RGBA8888 => {
                    if premultiply {
                        if zero_init == ZeroInitialized::Yes {
                            skip_zeros_rgba_to_bgra_premul
                        } else {
                            swizzle_rgba_to_bgra_premul
                        }
                    } else if zero_init == ZeroInitialized::Yes {
                        skip_zeros_rgba_to_bgra_unpremul
                    } else {
                        swizzle_rgba_to_bgra_unpremul
                    }
                }
                _ => return None,
            },
            Color::InvertedCMYK => match dst_ct {
                ColorType::RGBA8888 => swizzle_cmyk_to_rgba,
                ColorType::BGRA8888 => swizzle_cmyk_to_bgra,
                ColorType::RGB565 => swizzle_cmyk_to_565,
                _ => return None,
            },
            _ => return None,
        };

        // Store bpp in bytes if it is an even multiple, otherwise use bits
        let bits_per_pixel = usize::from(encoded.bits_per_pixel());
        let src_bpp = if bits_per_pixel % 8 == 0 {
            bits_per_pixel / 8
        } else {
            bits_per_pixel
        };
        let dst_bpp = dst_info.bytes_per_pixel();
        Some(Self::make_from_proc(
            dst_info, proc, ctable, src_bpp, dst_bpp, options, frame,
        ))
    }

    /// Port of `SkSwizzler::MakeSimple`: a copy-with-sampling swizzle for the 1-, 2-, 4-, 6- and
    /// 8-byte source pixels that need no conversion.
    // Port of: src/codec/SkSwizzler.cpp#L792-L827 (chrome/m156)
    #[doc(alias = "SkSwizzler::MakeSimple")]
    #[must_use]
    pub fn make_simple(
        src_bpp: usize,
        dst_info: &ImageInfo,
        options: &Options,
        frame: Option<IRect>,
    ) -> Option<Self> {
        let proc: RowProc = match src_bpp {
            1 => sample1,
            2 => sample2,
            4 => sample4,
            6 => sample6,
            8 => sample8,
            _ => return None,
        };
        Some(Self::make_from_proc(
            dst_info,
            proc,
            None,
            src_bpp,
            dst_info.bytes_per_pixel(),
            options,
            frame,
        ))
    }

    /// Port of `SkSwizzler::Make(dstInfo, fastProc, proc, ctable, srcBPP, dstBPP, options, frame)`
    /// (the fast proc is folded into `proc`; see the module note).
    // Port of: src/codec/SkSwizzler.cpp#L1169-L1190 (chrome/m156)
    #[must_use]
    fn make_from_proc(
        dst_info: &ImageInfo,
        proc: RowProc,
        ctable: Option<&[u32]>,
        src_bpp: usize,
        dst_bpp: usize,
        options: &Options,
        frame: Option<IRect>,
    ) -> Self {
        let mut src_offset = 0usize;
        let mut src_width = dst_info.width() as usize;
        let mut dst_offset = 0usize;
        let dst_width = src_width;
        if let Some(subset) = options.subset {
            src_offset = subset.left() as usize;
            src_width = subset.width() as usize;
        } else if let Some(frame) = frame {
            dst_offset = frame.left() as usize;
            src_width = frame.width() as usize;
        }
        Self::new(
            proc,
            ctable.unwrap_or(&[]),
            src_offset,
            src_width,
            dst_offset,
            dst_width,
            src_bpp,
            dst_bpp,
        )
    }

    // Port of: src/codec/SkSwizzler.cpp#L1192-L1209 (the SkSwizzler constructor)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ constructor's argument list
    fn new(
        proc: RowProc,
        ctable: &[u32],
        src_offset: usize,
        src_width: usize,
        dst_offset: usize,
        dst_width: usize,
        src_bpp: usize,
        dst_bpp: usize,
    ) -> Self {
        Self {
            sampler: SamplerBase::default(),
            proc,
            color_table: ctable.to_vec(),
            src_offset,
            src_offset_units: src_offset * src_bpp,
            dst_offset,
            dst_offset_bytes: dst_offset * dst_bpp,
            src_width,
            dst_width,
            swizzle_width: src_width,
            allocated_width: dst_width,
            sample_x: 1,
            src_bpp,
            dst_bpp,
        }
    }

    /// Port of `SkSampler::setSampleY`.
    pub fn set_sample_y(&mut self, sample_y: i32) {
        self.sampler.set_sample_y(sample_y);
    }

    /// Port of `SkSampler::sampleY`.
    #[must_use]
    pub fn sample_y(&self) -> i32 {
        self.sampler.sample_y()
    }

    /// Port of `SkSampler::rowNeeded`.
    #[must_use]
    pub fn row_needed(&self, row: i32) -> bool {
        self.sampler.row_needed(row)
    }

    /// Port of `SkSwizzler::fillWidth` (`fAllocatedWidth`).
    #[must_use]
    pub fn fill_width(&self) -> i32 {
        self.allocated_width as i32
    }

    /// Port of `SkSampler::setSampleX` (`SkSwizzler::onSetSampleX`). Returns the width after
    /// sampling.
    // Port of: src/codec/SkSwizzler.cpp#L1211-L1253 (chrome/m156)
    // The sample factor is an i32 in Skia; it is positive for every caller.
    #[allow(clippy::cast_sign_loss)]
    pub fn set_sample_x(&mut self, sample_x: i32) -> i32 {
        let sx = sample_x as usize;
        self.sample_x = sx;
        self.dst_offset_bytes = (self.dst_offset / sx) * self.dst_bpp;
        self.swizzle_width = get_sampled_dimension(self.src_width as i32, sample_x) as usize;
        self.allocated_width = get_sampled_dimension(self.dst_width as i32, sample_x) as usize;
        let mut frame_sample_x = sample_x;
        if self.src_width < self.dst_width {
            // Although SkSampledCodec adjusted sampleX so that it will never be larger than the
            // width of the image (or subset, if applicable), it doesn't account for the width of a
            // subset frame (i.e. gif). Compute a sampling rate based on the frame width to ensure
            // that fSrcOffsetUnits is sensible.
            frame_sample_x = (self.src_width / self.swizzle_width) as i32;
        }
        self.src_offset_units =
            (get_start_coord(frame_sample_x) as usize + self.src_offset) * self.src_bpp;
        if self.dst_offset_bytes > 0 {
            let dst_swizzle_bytes = self.swizzle_width * self.dst_bpp;
            let dst_allocated_bytes = self.allocated_width * self.dst_bpp;
            if self.dst_offset_bytes + dst_swizzle_bytes > dst_allocated_bytes {
                self.dst_offset_bytes = dst_allocated_bytes.wrapping_sub(dst_swizzle_bytes);
            }
        }
        self.allocated_width as i32
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
            self.sample_x * self.src_bpp,
            self.src_offset_units,
            &self.color_table,
        );
    }
}
