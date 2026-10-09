// Copyright 2013 Google Inc. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the COPYING file. Port by The skia-rust Authors.

//! Port of the C kernels of libwebp `src/dsp/alpha_processing.c` used by the decoder: the
//! premultiplication of RGBA-8888 and RGBA-4444 rows (`WebPApplyAlphaMultiply`,
//! `WebPApplyAlphaMultiply4444`), and the ARGB row premultiplication of the lossless rescaler
//! (`WebPMultARGBRow_C`).

// Module-level clippy allows. Each one mirrors the C source of this module.
// clippy::cast_possible_truncation: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::many_single_char_names: pixel, offset and loop variables keep the single-letter names of the C source.
#![allow(clippy::cast_possible_truncation, clippy::many_single_char_names)]

/// Port of `dither_hi` (`alpha_processing.c`).
#[inline]
fn dither_hi(x: u8) -> u8 {
    (x & 0xf0) | (x >> 4)
}

/// Port of `dither_lo`.
#[inline]
fn dither_lo(x: u8) -> u8 {
    (x & 0x0f) | (x << 4)
}

/// Port of `multiply(x, m)`: `(x * m) >> 16`, truncated to 8 bits as the C `uint8_t` return.
#[inline]
fn multiply(x: u8, m: u32) -> u8 {
    ((u32::from(x) * m) >> 16) as u8
}

/// Port of `ApplyAlphaMultiply4444_C`: premultiplies `w` pixels in each of `h` rows of
/// RGBA-4444 (2 bytes per pixel, `stride` bytes between rows). `rg_byte_pos` selects which of the
/// two bytes holds the red and green channels (`WEBP_SWAP_16BIT_CSP` puts them second).
#[doc(alias = "WebPApplyAlphaMultiply4444")]
pub fn apply_alpha_multiply4444(
    rgba4444: &mut [u8],
    w: usize,
    h: usize,
    stride: usize,
    rg_byte_pos: usize,
) {
    let mut base = 0usize;
    for _ in 0..h {
        for i in 0..w {
            let rg = u32::from(rgba4444[base + 2 * i + rg_byte_pos]);
            let ba = u32::from(rgba4444[base + 2 * i + (rg_byte_pos ^ 1)]);
            let a = (ba & 0x0f) as u8;
            let mult = u32::from(a) * 0x1111; // 0x1111 ~= (1 << 16) / 15
            let r = multiply(dither_hi(rg as u8), mult);
            let g = multiply(dither_lo(rg as u8), mult);
            let b = multiply(dither_hi(ba as u8), mult);
            rgba4444[base + 2 * i + rg_byte_pos] = (r & 0xf0) | ((g >> 4) & 0x0f);
            rgba4444[base + 2 * i + (rg_byte_pos ^ 1)] = (b & 0xf0) | a;
        }
        base += stride;
    }
}

/// Port of `ApplyAlphaMultiply_16b_C` with `WEBP_SWAP_16BIT_CSP == 1`.
#[doc(alias = "WebPApplyAlphaMultiply4444")]
pub fn apply_alpha_multiply_16b(rgba4444: &mut [u8], w: usize, h: usize, stride: usize) {
    apply_alpha_multiply4444(rgba4444, w, h, stride, 1);
}

/// Port of `MFIX` and `HALF` (`alpha_processing.c`): 24-bit fixed-point premultiplication.
const MFIX: u32 = 24;
const HALF: u32 = (1u32 << MFIX) >> 1;
/// Port of `KINV_255`: `(1 << MFIX) / 255`.
const KINV_255: u32 = (1u32 << MFIX) / 255;

/// Port of `Mult` (`alpha_processing.c`). The product wraps as the C `uint32_t` arithmetic does.
#[inline]
fn mult(x: u8, mult: u32) -> u32 {
    (u32::from(x).wrapping_mul(mult).wrapping_add(HALF)) >> MFIX
}

/// Port of `GetScale` in its default configuration (`USE_TABLES_FOR_ALPHA_MULT == 0`).
#[inline]
fn get_scale(a: u32, inverse: bool) -> u32 {
    if inverse {
        (255u32 << MFIX) / a
    } else {
        a.wrapping_mul(KINV_255)
    }
}

/// Port of `WebPMultARGBRow_C`: premultiplies (or, with `inverse`, un-premultiplies) one row of
/// ARGB pixels in place.
#[doc(alias = "WebPMultARGBRow")]
pub fn mult_argb_row(ptr: &mut [u32], inverse: bool) {
    for p in ptr.iter_mut() {
        let argb = *p;
        if argb < 0xff00_0000 {
            // alpha < 255
            if argb <= 0x00ff_ffff {
                // alpha == 0
                *p = 0;
            } else {
                let alpha = (argb >> 24) & 0xff;
                let scale = get_scale(alpha, inverse);
                let mut out = argb & 0xff00_0000;
                out |= mult(argb as u8, scale);
                out |= mult((argb >> 8) as u8, scale) << 8;
                out |= mult((argb >> 16) as u8, scale) << 16;
                *p = out;
            }
        }
    }
}
