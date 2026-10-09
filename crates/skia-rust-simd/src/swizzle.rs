// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkSwizzler_opts.inc, src/core/SkSwizzler_opts.cpp

//! The `SkOpts` swizzles of `SkSwizzler_opts.inc`: the 8888 swizzles that `SkConvertPixels` uses
//! (`RGBA_to_BGRA`, `RGBA_to_rgbA`, `RGBA_to_bgrA`, `rgbA_to_RGBA`, `rgbA_to_BGRA`; task D8), and
//! the codec expansions (`RGB_to_RGB1`, `RGB_to_BGR1`, `gray_to_RGB1`, `grayA_to_RGBA`,
//! `grayA_to_rgbA`, `inverted_CMYK_to_RGB1`, `inverted_CMYK_to_BGR1`).
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
//! - Swapping R and B is a byte shuffle. Expanding RGB or gray to RGB1 inserts an opaque alpha
//!   byte; the SIMD kernels of Skia copy the same bytes.
//! - Premultiplying a gray/alpha pair and the CMYK to RGB conversion use the same `(x + 127) /
//!   255` (`scale` in the SSE/AVX2 code and `div255_round` in the NEON code), so they are
//!   portable kernels too.
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

// Port of: src/opts/SkSwizzler_opts.inc#L50-L52 (chrome/m156)
/// `reciprocal_alpha_times_255_portable`: `255 / a`, or 0 for `a == 0`.
#[allow(clippy::float_cmp)] // mirrors `a != 0 ? 255/a : 0`
#[must_use]
pub fn reciprocal_alpha_times_255_portable(a: f32) -> f32 {
    if a == 0.0 { 0.0f32 } else { 255.0f32 / a }
}

// Port of: src/opts/SkSwizzler_opts.inc#L54-L56 (chrome/m156)
/// `reciprocal_alpha_portable`: `1 / a`, or 0 for `a == 0`.
#[allow(clippy::float_cmp)] // mirrors `a != 0 ? 1/a : 0`
#[must_use]
pub fn reciprocal_alpha_portable(a: f32) -> f32 {
    if a == 0.0 { 0.0f32 } else { 1.0f32 / a }
}

// Port of: src/opts/SkSwizzler_opts.inc#L68-L86 (chrome/m156), the SSE version.
/// `reciprocal_alpha_times_255`. The SSE version divides (`255 / a`) and masks the quotient
/// with `a != 0`, which is the value of [`reciprocal_alpha_times_255_portable`] bit for bit;
/// the NEON and portable builds call the portable function, so this is that function too.
#[must_use]
pub fn reciprocal_alpha_times_255(a: f32) -> f32 {
    reciprocal_alpha_times_255_portable(a)
}

/// `reciprocal_alpha`: `1 / a`, or 0 for `a == 0`. As [`reciprocal_alpha_times_255`], the SSE
/// version is the same value as [`reciprocal_alpha_portable`].
#[must_use]
pub fn reciprocal_alpha(a: f32) -> f32 {
    reciprocal_alpha_portable(a)
}

// Port of: src/opts/SkSwizzler_opts.inc#L123-L134 (chrome/m156)
/// `pixel_round_as_RP`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
// mirrors the C++ float -> uint32_t conversions of values in [0, 255]
#[doc(alias = "pixel_round_as_RP")]
#[must_use]
pub fn pixel_round_as_rp(n: f32, tier: Tier) -> u32 {
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
#[doc(alias = "unpremul_simulating_RP")]
#[must_use]
pub fn unpremul_simulating_rp(reciprocal_a: f32, c: f32, tier: Tier) -> u32 {
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

// Port of: src/opts/SkSwizzler_opts.inc#L1805-L1816 (chrome/m156)
/// `SkOpts::RGB_to_RGB1`: expands `count` RGB triples (`src`, 3 bytes each) to opaque RGBA
/// pixels in `dst`.
#[doc(alias = "RGB_to_RGB1")]
pub fn rgb_to_rgb1(dst: &mut [u8], src: &[u8], count: usize) {
    for i in 0..count {
        let r = src[3 * i];
        let g = src[3 * i + 1];
        let b = src[3 * i + 2];
        store(
            dst,
            i,
            0xFF << 24 | u32::from(b) << 16 | u32::from(g) << 8 | u32::from(r),
        );
    }
}

// Port of: src/opts/SkSwizzler_opts.inc#L1817-L1828 (chrome/m156)
/// `SkOpts::RGB_to_BGR1`: as [`rgb_to_rgb1`], with R and B swapped.
#[doc(alias = "RGB_to_BGR1")]
pub fn rgb_to_bgr1(dst: &mut [u8], src: &[u8], count: usize) {
    for i in 0..count {
        let r = src[3 * i];
        let g = src[3 * i + 1];
        let b = src[3 * i + 2];
        store(
            dst,
            i,
            0xFF << 24 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b),
        );
    }
}

// Port of: src/opts/SkSwizzler_opts.inc#L1621-L1628 (chrome/m156)
/// `SkOpts::gray_to_RGB1`: expands `count` gray bytes to opaque RGBA pixels.
#[doc(alias = "gray_to_RGB1")]
pub fn gray_to_rgb1(dst: &mut [u8], src: &[u8], count: usize) {
    for (i, &s) in src[..count].iter().enumerate() {
        let g = u32::from(s);
        store(dst, i, 0xFF << 24 | g << 16 | g << 8 | g);
    }
}

// Port of: src/opts/SkSwizzler_opts.inc#L237-L247 (chrome/m156)
/// `SkOpts::grayA_to_RGBA`: expands `count` gray+alpha pairs (`src`, 2 bytes each) to RGBA
/// pixels, without premultiplying.
#[doc(alias = "grayA_to_RGBA")]
pub fn gray_a_to_rgba(dst: &mut [u8], src: &[u8], count: usize) {
    for i in 0..count {
        let g = u32::from(src[2 * i]);
        let a = u32::from(src[2 * i + 1]);
        store(dst, i, a << 24 | g << 16 | g << 8 | g);
    }
}

// Port of: src/opts/SkSwizzler_opts.inc#L249-L260 (chrome/m156)
/// `SkOpts::grayA_to_rgbA`: as [`gray_a_to_rgba`], premultiplying the gray by the alpha.
#[doc(alias = "grayA_to_rgbA")]
pub fn gray_a_to_rgba_premul(dst: &mut [u8], src: &[u8], count: usize) {
    for i in 0..count {
        let g = u32::from(src[2 * i]);
        let a = u32::from(src[2 * i + 1]);
        let g = premul_channel(g, a);
        store(dst, i, a << 24 | g << 16 | g << 8 | g);
    }
}

// Port of: src/opts/SkSwizzler_opts.inc#L262-L277 (chrome/m156)
/// `SkOpts::inverted_CMYK_to_RGB1`: converts `count` inverted CMYK pixels (`src`, the bytes of
/// `uint32_t`s with K, Y, M, C from the top byte down) to opaque RGBA pixels.
#[doc(alias = "inverted_CMYK_to_RGB1")]
pub fn inverted_cmyk_to_rgb1(dst: &mut [u8], src: &[u8], count: usize) {
    for i in 0..count {
        let p = load(src, i);
        let k = (p >> 24) & 0xFF;
        let y = (p >> 16) & 0xFF;
        let m = (p >> 8) & 0xFF;
        let c = p & 0xFF;
        let b = premul_channel(y, k);
        let g = premul_channel(m, k);
        let r = premul_channel(c, k);
        store(dst, i, 0xFF << 24 | b << 16 | g << 8 | r);
    }
}

// Port of: src/opts/SkSwizzler_opts.inc#L279-L293 (chrome/m156)
/// `SkOpts::inverted_CMYK_to_BGR1`: as [`inverted_cmyk_to_rgb1`], with R and B swapped.
#[doc(alias = "inverted_CMYK_to_BGR1")]
pub fn inverted_cmyk_to_bgr1(dst: &mut [u8], src: &[u8], count: usize) {
    for i in 0..count {
        let p = load(src, i);
        let k = (p >> 24) & 0xFF;
        let y = (p >> 16) & 0xFF;
        let m = (p >> 8) & 0xFF;
        let c = p & 0xFF;
        let b = premul_channel(y, k);
        let g = premul_channel(m, k);
        let r = premul_channel(c, k);
        store(dst, i, 0xFF << 24 | r << 16 | g << 8 | b);
    }
}

// Port of: src/core/SkSwizzle.cpp#L12-L14 (chrome/m156)
/// `SkOpts::RGBA_to_BGRA` (Skia's `SkSwapRB`) on `u32`
/// pixels: `dst.len()` pixels are swapped from `src`.
///
/// # Panics
/// If `src` is shorter than `dst`.
#[doc(alias = "RGBA_to_BGRA")]
pub fn rgba_to_bgra_words(dst: &mut [u32], src: &[u32]) {
    let src = &src[..dst.len()];
    for (d, &p) in dst.iter_mut().zip(src) {
        let a = (p >> 24) & 0xFF;
        let b = (p >> 16) & 0xFF;
        let g = (p >> 8) & 0xFF;
        let r = p & 0xFF;
        *d = a << 24 | r << 16 | g << 8 | b;
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

    /// Pixel counts that cover empty input, every tail length of the 4-, 8- and 16-wide loops
    /// of the SIMD kernels, and a long run. Fewer under Miri, which is slow.
    fn counts() -> Vec<usize> {
        if cfg!(miri) {
            (0..=17).chain([33]).collect()
        } else {
            (0..=40).chain([63, 64, 65, 255, 256, 1000]).collect()
        }
    }

    /// Deterministic pseudo-random bytes (a 32-bit LCG), so a failure reproduces.
    fn random_bytes(n: usize, seed: u32) -> Vec<u8> {
        let mut s = seed;
        (0..n)
            .map(|_| {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                s.to_be_bytes()[0]
            })
            .collect()
    }

    /// `(x + 128) * 257 >> 16`, the SSSE3/AVX2 `scale` (`_mm_mulhi_epu16(x * y + 128, 257)`),
    /// as a scalar twin of the SIMD arithmetic that the kernels must match.
    fn scale_simd(x: u32, y: u32) -> u32 {
        ((x * y + 128) * 257) >> 16
    }

    /// Runs `kernel` on `count` random source pixels of `src_stride` bytes and compares each
    /// output word with `reference`; bytes after the `count` pixels must be untouched.
    fn check_kernel(
        name: &str,
        src_stride: usize,
        kernel: fn(&mut [u8], &[u8], usize),
        reference: fn(&[u8]) -> u32,
    ) {
        for count in counts() {
            let src = random_bytes(src_stride * count, u32::try_from(count).unwrap() + 1);
            let mut dst = vec![0xEE; 4 * (count + 3)];
            kernel(&mut dst, &src, count);
            for i in 0..count {
                let expected = reference(&src[src_stride * i..src_stride * (i + 1)]);
                assert_eq!(load(&dst, i), expected, "{name}: count={count} pixel={i}");
            }
            assert!(
                dst[4 * count..].iter().all(|&b| b == 0xEE),
                "{name}: wrote past count={count}"
            );
        }
    }

    fn word(p: &[u8]) -> u32 {
        u32::from_ne_bytes([p[0], p[1], p[2], p[3]])
    }

    #[test]
    fn rgb_to_rgb1_matches_reference() {
        check_kernel("RGB_to_RGB1", 3, rgb_to_rgb1, |p| {
            0xFF << 24 | u32::from(p[2]) << 16 | u32::from(p[1]) << 8 | u32::from(p[0])
        });
    }

    #[test]
    fn rgb_to_bgr1_matches_reference() {
        check_kernel("RGB_to_BGR1", 3, rgb_to_bgr1, |p| {
            0xFF << 24 | u32::from(p[0]) << 16 | u32::from(p[1]) << 8 | u32::from(p[2])
        });
    }

    #[test]
    fn gray_to_rgb1_matches_reference() {
        check_kernel("gray_to_RGB1", 1, gray_to_rgb1, |p| {
            let g = u32::from(p[0]);
            0xFF << 24 | g << 16 | g << 8 | g
        });
    }

    #[test]
    fn gray_a_to_rgba_matches_reference() {
        check_kernel("grayA_to_RGBA", 2, gray_a_to_rgba, |p| {
            let g = u32::from(p[0]);
            u32::from(p[1]) << 24 | g << 16 | g << 8 | g
        });
    }

    /// Premultiplied gray uses the portable `(g * a + 127) / 255`; the SIMD twin must agree on
    /// every input.
    #[test]
    fn gray_a_to_rgba_premul_matches_reference() {
        check_kernel("grayA_to_rgbA", 2, gray_a_to_rgba_premul, |p| {
            let a = u32::from(p[1]);
            let g = scale_simd(u32::from(p[0]), a);
            a << 24 | g << 16 | g << 8 | g
        });
    }

    #[test]
    fn inverted_cmyk_to_rgb1_matches_reference() {
        check_kernel("inverted_CMYK_to_RGB1", 4, inverted_cmyk_to_rgb1, |p| {
            let p = word(p);
            let k = (p >> 24) & 0xFF;
            let (y, m, c) = ((p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF);
            let (b, g, r) = (scale_simd(y, k), scale_simd(m, k), scale_simd(c, k));
            0xFF << 24 | b << 16 | g << 8 | r
        });
    }

    #[test]
    fn inverted_cmyk_to_bgr1_matches_reference() {
        check_kernel("inverted_CMYK_to_BGR1", 4, inverted_cmyk_to_bgr1, |p| {
            let p = word(p);
            let k = (p >> 24) & 0xFF;
            let (y, m, c) = ((p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF);
            let (b, g, r) = (scale_simd(y, k), scale_simd(m, k), scale_simd(c, k));
            0xFF << 24 | r << 16 | g << 8 | b
        });
    }

    /// The word kernel behind `SkSwapRB` agrees with the byte kernel `RGBA_to_BGRA`.
    #[test]
    fn rgba_to_bgra_words_matches_byte_kernel() {
        for count in counts() {
            let src = random_bytes(4 * count, u32::try_from(count).unwrap() + 7);
            let words: Vec<u32> = src.as_chunks::<4>().0.iter().map(|p| word(p)).collect();
            let mut out = vec![0u32; count];
            rgba_to_bgra_words(&mut out, &words);
            let mut bytes = vec![0u8; 4 * count];
            rgba_to_bgra(&mut bytes, &src, count);
            for (i, &w) in out.iter().enumerate() {
                assert_eq!(w, word(&bytes[4 * i..]), "count={count} pixel={i}");
            }
        }
    }

    /// `pixel_round_as_RP`'s two rounding modes differ only at halves.
    #[test]
    fn pixel_round_as_rp_modes() {
        assert_eq!(pixel_round_as_rp(0.5, Tier::Scalar), 1);
        assert_eq!(pixel_round_as_rp(0.5, Tier::Sse2), 0);
        assert_eq!(pixel_round_as_rp(1.5, Tier::Sse2), 2);
        assert_eq!(pixel_round_as_rp(254.6, Tier::Sse2), 255);
    }
}
