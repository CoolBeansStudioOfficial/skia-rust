// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkSwizzler_opts.inc, src/core/SkSwizzler_opts.cpp

//! The 8888 swizzles of `SkOpts` that `SkConvertPixels` uses (`RGBA_to_BGRA`, `RGBA_to_rgbA`,
//! `RGBA_to_bgrA`, `rgbA_to_RGBA`, `rgbA_to_BGRA`; task D8).
//!
//! Pixels are 32-bit words in native byte order, read from and written to byte slices (Skia's
//! `uint32_t*`): red in bits 0–7, alpha in bits 24–31 for an `RGBA` word.
//!
//! **Every tier computes the same bytes**, so these are the portable kernels only:
//!
//! - Premultiplication is `(c * a + 127) / 255` exactly in the portable code. The SSSE3/AVX2
//!   kernels compute `((c * a + 128) * 257) >> 16` and the NEON kernel
//!   `vraddhn(x, vrshr(x, 8))` = `(x + 128 + ((x + 128) >> 8)) >> 8` with `x = c * a`; both
//!   equal `(x + 127) / 255` for every `x <= 255 * 255` (checked exhaustively in the tests).
//! - Swapping R and B is a byte shuffle.
//! - Unpremultiplication (`kFastUnpremul == false`) is
//!   `round(min(255, (c * (1/255)) * (1 / (a * (1/255))) * 255))`, with `1/x` masked to 0 when
//!   `x == 0`, in every kernel (the NEON kernel computes the same products lane-wise and
//!   saturates instead of clamping, which agrees after rounding). Only the rounding depends on
//!   the build: `pixel_round_as_RP` rounds half to even on x86 (`_mm_cvtps_epi32`) and arm64
//!   (`vrndns_f32` / `vcvtnq_u32_f32`), and adds 0.5 and truncates in the portable build (the
//!   [`Tier::Scalar`] oracle, wasm).
//!
//! The kernels read the current [`selection`] for that rounding only.

#![allow(clippy::many_single_char_names)] // the C++ locals: p, a, b, g, r

use crate::tier::{Tier, selection};

#[inline]
fn load(src: &[u8], i: usize) -> u32 {
    u32::from_ne_bytes([src[4 * i], src[4 * i + 1], src[4 * i + 2], src[4 * i + 3]])
}

#[inline]
fn store(dst: &mut [u8], i: usize, v: u32) {
    dst[4 * i..4 * i + 4].copy_from_slice(&v.to_ne_bytes());
}

/// `(c * a + 127) / 255` on bytes.
#[inline]
fn premul_channel(c: u32, a: u32) -> u32 {
    (c * a + 127) / 255
}

// Port of: src/opts/SkSwizzler_opts.inc#L98-L112 (chrome/m156)
/// `SkOpts::RGBA_to_rgbA`: premultiplies `count` RGBA pixels.
#[doc(alias = "RGBA_to_rgbA")]
pub fn rgba_to_rgba_premul(dst: &mut [u8], src: &[u8], count: usize) {
    for i in 0..count {
        let p = load(src, i);
        let a = (p >> 24) & 0xFF;
        let b = premul_channel((p >> 16) & 0xFF, a);
        let g = premul_channel((p >> 8) & 0xFF, a);
        let r = premul_channel(p & 0xFF, a);
        store(dst, i, a << 24 | b << 16 | g << 8 | r);
    }
}

// Port of: src/opts/SkSwizzler_opts.inc#L208-L222 (chrome/m156)
/// `SkOpts::RGBA_to_bgrA`: premultiplies `count` RGBA pixels and swaps R and B.
#[doc(alias = "RGBA_to_bgrA")]
pub fn rgba_to_bgra_premul(dst: &mut [u8], src: &[u8], count: usize) {
    for i in 0..count {
        let p = load(src, i);
        let a = (p >> 24) & 0xFF;
        let b = premul_channel((p >> 16) & 0xFF, a);
        let g = premul_channel((p >> 8) & 0xFF, a);
        let r = premul_channel(p & 0xFF, a);
        store(dst, i, a << 24 | r << 16 | g << 8 | b);
    }
}

// Port of: src/opts/SkSwizzler_opts.inc#L224-L235 (chrome/m156)
/// `SkOpts::RGBA_to_BGRA`: swaps R and B of `count` pixels.
#[doc(alias = "RGBA_to_BGRA")]
pub fn rgba_to_bgra(dst: &mut [u8], src: &[u8], count: usize) {
    for i in 0..count {
        let p = load(src, i);
        let a = (p >> 24) & 0xFF;
        let b = (p >> 16) & 0xFF;
        let g = (p >> 8) & 0xFF;
        let r = p & 0xFF;
        store(dst, i, a << 24 | r << 16 | g << 8 | b);
    }
}

// Port of: src/opts/SkSwizzler_opts.inc#L54-L56 and #L65-L67, #L81-L86 (chrome/m156)
/// `reciprocal_alpha`: `1 / a`, or 0 for `a == 0` (the SSE version masks the quotient with
/// `a != 0`, which is the same value).
#[allow(clippy::float_cmp)] // mirrors `a != 0 ? 1/a : 0`
fn reciprocal_alpha(a: f32) -> f32 {
    if a == 0.0 { 0.0f32 } else { 1.0f32 / a }
}

// Port of: src/opts/SkSwizzler_opts.inc#L123-L134 (chrome/m156)
/// `pixel_round_as_RP`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
// mirrors the C++ float -> uint32_t conversions of values in [0, 255]
fn pixel_round_as_rp(n: f32, tier: Tier) -> u32 {
    if tier == Tier::Scalar {
        // return (uint32_t)(n + 0.5f);
        (n + 0.5f32) as u32
    } else {
        // _mm_cvtps_epi32 / vrndns_f32: round to nearest, ties to even.
        n.round_ties_even() as u32
    }
}

// Port of: src/opts/SkSwizzler_opts.inc#L153-L157 (chrome/m156)
/// `unpremul_simulating_RP`.
fn unpremul_simulating_rp(reciprocal_a: f32, c: f32, tier: Tier) -> u32 {
    let normalized_c = c * (1.0f32 / 255.0f32);
    let answer = 255.0f32.min(normalized_c * reciprocal_a * 255.0f32);
    pixel_round_as_rp(answer, tier)
}

// Port of: src/opts/SkSwizzler_opts.inc#L159-L180 (chrome/m156) (`kFastUnpremul == false`)
/// `rgbA_to_CCCA`.
#[allow(clippy::cast_precision_loss)] // bytes convert to float exactly
fn rgba_to_ccca(c00: u32, c08: u32, c16: u32, a: u32, tier: Tier) -> u32 {
    let normalized_a = a as f32 * (1.0f32 / 255.0f32);
    let reciprocal_a = reciprocal_alpha(normalized_a);
    let unpremul = |c: u32| unpremul_simulating_rp(reciprocal_a, c as f32, tier);
    a << 24 | unpremul(c16) << 16 | unpremul(c08) << 8 | unpremul(c00)
}

// Port of: src/opts/SkSwizzler_opts.inc#L182-L193 (chrome/m156)
/// `SkOpts::rgbA_to_RGBA`: unpremultiplies `count` RGBA pixels.
#[doc(alias = "rgbA_to_RGBA")]
pub fn rgba_premul_to_rgba(dst: &mut [u8], src: &[u8], count: usize) {
    let tier = selection().tier;
    for i in 0..count {
        let p = load(src, i);
        let (a, b, g, r) = (
            (p >> 24) & 0xFF,
            (p >> 16) & 0xFF,
            (p >> 8) & 0xFF,
            p & 0xFF,
        );
        store(dst, i, rgba_to_ccca(r, g, b, a, tier));
    }
}

// Port of: src/opts/SkSwizzler_opts.inc#L195-L206 (chrome/m156)
/// `SkOpts::rgbA_to_BGRA`: unpremultiplies `count` RGBA pixels and swaps R and B.
#[doc(alias = "rgbA_to_BGRA")]
pub fn rgba_premul_to_bgra(dst: &mut [u8], src: &[u8], count: usize) {
    let tier = selection().tier;
    for i in 0..count {
        let p = load(src, i);
        let (a, b, g, r) = (
            (p >> 24) & 0xFF,
            (p >> 16) & 0xFF,
            (p >> 8) & 0xFF,
            p & 0xFF,
        );
        store(dst, i, rgba_to_ccca(b, g, r, a, tier));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The SIMD kernels' rounded divisions equal the portable `(x + 127) / 255` for every
    /// product of two bytes.
    #[test]
    fn simd_div255_formulas_match_portable() {
        let step = if cfg!(miri) { 17 } else { 1 };
        for c in (0..=255u32).step_by(step) {
            for a in (0..=255u32).step_by(step) {
                let x = c * a;
                let portable = (x + 127) / 255;
                // SSSE3/AVX2 `scale`: _mm_mulhi_epu16(x + 128, 257).
                let x86 = ((x + 128) * 257) >> 16;
                // NEON `div255_round`: vraddhn_u16(x, vrshrq_n_u16(x, 8)).
                let neon = (x + ((x + 128) >> 8) + 128) >> 8;
                assert_eq!(portable, x86, "c={c} a={a}");
                assert_eq!(portable, neon, "c={c} a={a}");
            }
        }
    }

    #[test]
    fn swizzles() {
        let src = 0x8040_20ffu32.to_ne_bytes(); // a=0x80, b=0x40, g=0x20, r=0xff
        let mut dst = [0u8; 4];
        rgba_to_bgra(&mut dst, &src, 1);
        assert_eq!(u32::from_ne_bytes(dst), 0x80ff_2040);
        rgba_to_rgba_premul(&mut dst, &src, 1);
        assert_eq!(u32::from_ne_bytes(dst), 0x8020_1080);
        rgba_to_bgra_premul(&mut dst, &src, 1);
        assert_eq!(u32::from_ne_bytes(dst), 0x8080_1020);
        let pm = 0x8020_1080u32.to_ne_bytes();
        rgba_premul_to_rgba(&mut dst, &pm, 1);
        assert_eq!(u32::from_ne_bytes(dst), 0x8040_20ff);
        rgba_premul_to_bgra(&mut dst, &pm, 1);
        assert_eq!(u32::from_ne_bytes(dst), 0x80ff_2040);
        // a == 0: the reciprocal is masked to 0.
        rgba_premul_to_rgba(&mut dst, &0x0012_3456u32.to_ne_bytes(), 1);
        assert_eq!(u32::from_ne_bytes(dst), 0);
    }
}
