// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SwizzlerTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::swizzle::swap_rb;
use skia_rust_simd::Tier;
use skia_rust_simd::selection;
use skia_rust_simd::swizzle::{
    pixel_round_as_rp, reciprocal_alpha, reciprocal_alpha_portable, reciprocal_alpha_times_255,
    reciprocal_alpha_times_255_portable, rgba_to_bgra, rgba_to_bgra_premul, rgba_to_rgba_premul,
    unpremul_simulating_rp,
};

use crate::{Reporter, def_test, def_tier_test, reporter_assert};

/// Runs a one-pixel byte kernel (`SkOpts::...(&dst, &src, 1)`) and returns the output pixel.
fn one_pixel(kernel: fn(&mut [u8], &[u8], usize), src: u32) -> u32 {
    let mut dst = [0u8; 4];
    kernel(&mut dst, &src.to_ne_bytes(), 1);
    u32::from_ne_bytes(dst)
}

// Port of: tests/SwizzlerTest.cpp#L122-L152 (chrome/m156)
def_test!(SwizzleOpts, |r| {
    // forall c, c*255 == c, c*0 == 0
    for c in 0..=255u32 {
        let src = (255 << 24) | c;
        reporter_assert!(r, one_pixel(rgba_to_rgba_premul, src) == src);
        reporter_assert!(
            r,
            one_pixel(rgba_to_bgra_premul, src) == (255 << 24) | (c << 16)
        );

        let src = c;
        reporter_assert!(r, one_pixel(rgba_to_rgba_premul, src) == 0);
        reporter_assert!(r, one_pixel(rgba_to_bgra_premul, src) == 0);
    }

    // check a totally arbitrary color
    let src = 0xFACE_B004;
    reporter_assert!(r, one_pixel(rgba_to_rgba_premul, src) == 0xFACA_AD04);

    // swap red and blue
    reporter_assert!(r, one_pixel(rgba_to_bgra, src) == 0xFA04_B0CE);

    // all together now
    reporter_assert!(r, one_pixel(rgba_to_bgra_premul, src) == 0xFA04_ADCA);
});

// Port of: tests/SwizzlerTest.cpp#L154-L161 (chrome/m156)
def_test!(PublicSwizzleOpts, |r| {
    // check a totally arbitrary color
    let src = 0xFACE_B004;
    let mut dst = [0u32; 1];
    swap_rb(&mut dst, &[src]);
    reporter_assert!(r, dst[0] == 0xFA04_B0CE);
});

// Port of: tests/SwizzlerTest.cpp#L163-L181 (chrome/m156)
#[allow(clippy::float_cmp)] // the test checks bit-exact results, as Skia's REPORTER_ASSERT does
fn test_reciprocal_alpha(r: &mut Reporter, test255: fn(f32) -> f32, test1: fn(f32) -> f32) {
    reporter_assert!(r, test255(0.0) == 0.0);
    for i in 1..=255u8 {
        let rv = test255(f32::from(i));
        let e = 255.0f32 / f32::from(i);
        reporter_assert!(r, rv == e);
    }

    reporter_assert!(r, test1(0.0) == 0.0);
    for i in 1..=255u8 {
        let normalized = f32::from(i) / 255.0f32;
        let rv = test1(normalized);
        let e = 1.0f32 / normalized;
        reporter_assert!(r, rv == e);
    }
}

// Port of: tests/SwizzlerTest.cpp#L187-L191 (chrome/m156)
// The SSE and NEON reciprocals are the same IEEE divisions as the portable ones, so this test
// exercises the same functions as `ReciprocalAlphaPortable`.
def_test!(ReciprocalAlphaOptimized, |r| {
    test_reciprocal_alpha(r, reciprocal_alpha_times_255, reciprocal_alpha);
});

// Port of: tests/SwizzlerTest.cpp#L193-L197 (chrome/m156)
def_test!(ReciprocalAlphaPortable, |r| {
    test_reciprocal_alpha(
        r,
        reciprocal_alpha_times_255_portable,
        reciprocal_alpha_portable,
    );
});

// Port of: tests/SwizzlerTest.cpp#L199-L232 (chrome/m156), `calcExpected`.
/// The unpremultiplied byte the raster pipeline stores for `comp` at alpha `alpha`, as the
/// test simulates it; rounds with the rounding of `tier`.
fn calc_expected(alpha: f32, comp: f32, tier: Tier) -> u32 {
    if alpha == 0.0 {
        return 0;
    }
    let normalized = comp * (1.0f32 / 255.0f32);
    let normalized_a = alpha * (1.0f32 / 255.0f32);
    let inverse_alpha = 1.0f32 / normalized_a;
    let unpremul = normalized * inverse_alpha;
    let scaled_and_pinned = 255.0f32.min(unpremul * 255.0f32);
    pixel_round_as_rp(scaled_and_pinned, tier)
}

// Port of: tests/SwizzlerTest.cpp#L234-L247 (chrome/m156)
// The rounding of `unpremul_simulating_RP` depends on the tier, so the test runs on each tier.
def_tier_test!(UnpremulSimulatingRP, |r| {
    let tier = selection().tier;
    for a in 0..=255u8 {
        for c in 0..=255u8 {
            let (a, c) = (f32::from(a), f32::from(c));
            let expected = calc_expected(a, c, tier);
            let normalized_a = a * (1.0f32 / 255.0f32);
            let inv_a = reciprocal_alpha(normalized_a);
            let actual = unpremul_simulating_rp(inv_a, c, tier);
            reporter_assert!(
                r,
                actual == expected,
                "a: {a} c: {c} expected: {expected} actual: {actual}"
            );
        }
    }
});
