// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Every tier of the blit-row kernels against its formula, and the scalar twins.

#![allow(
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::too_many_arguments
)] // test code: Skia's names, byte truncation of random words, kernel signatures

use super::*;
use crate::testing::{Rng, test_selections};

fn random_pixels(rng: &mut Rng, n: usize) -> Vec<u32> {
    (0..n)
        .map(|_| match rng.next_u32() % 4 {
            // Premultiplied.
            0 => {
                let a = rng.next_u32() & 0xFF;
                let mut c = || (rng.next_u32() & 0xFF) * a / 255;
                let (r, g, b) = (c(), c(), c());
                (a << 24) | (r << 16) | (g << 8) | b
            }
            // Extreme alphas, to hit saturation.
            1 => {
                let a = if rng.next_u32() & 1 == 0 {
                    0
                } else {
                    0xFF00_0000
                };
                a | (rng.next_u32() & 0x00FF_FFFF)
            }
            // Garbage (channels above alpha).
            _ => rng.next_u32(),
        })
        .collect()
}

/// The x86 tiers (SIMD, model) and the scalar twin agree on every input, including
/// non-premultiplied garbage: the x86 formula is `SkPMSrcOver`.
#[test]
fn x86_formula_equals_scalar() {
    let mut rng = Rng(0x1234_5678_9ABC_DEF1);
    // Miri is ~1000x slower: a thinner sweep there.
    let (samples, step) = if cfg!(miri) {
        (2_000, 37)
    } else {
        (200_000, 1)
    };
    for _ in 0..samples {
        let (s, d) = (rng.next_u32(), rng.next_u32());
        assert_eq!(src_over_x86(s, d), pm_src_over(s, d), "{s:08x} {d:08x}");
    }
    // Every alpha against every dst value, saturating channels included.
    for sa in (0..=255u32).step_by(step) {
        for dv in (0..=255u32).step_by(step) {
            for sc in [0, 1, sa, 0x7F, 0xFE, 0xFF] {
                let s = (sa << 24) | (sc << 16) | (sc << 8) | sc;
                let d = (dv << 24) | (dv << 16) | (dv << 8) | dv;
                assert_eq!(src_over_x86(s, d), pm_src_over(s, d), "{s:08x} {d:08x}");
            }
        }
    }
}

/// The Neon formula is exactly rounded.
#[test]
fn neon_formula_is_exact_rounding() {
    for sa in (0..=255u32).step_by(if cfg!(miri) { 5 } else { 1 }) {
        for d in (0..=255u32).step_by(if cfg!(miri) { 17 } else { 1 }) {
            let expected = (sa + mul_div_255_round(255 - sa, d)).min(255);
            let px = src_over_neon(sa << 24, d << 24);
            assert_eq!(px >> 24, expected);
        }
    }
}

#[test]
fn every_tier_matches_its_formula() {
    let mut rng = Rng(0xDEAD_BEEF_0BAD_F00D);
    // Under Miri: a few lengths around the vector widths and tails (sweeps run natively).
    let lens: Vec<usize> = if cfg!(miri) {
        vec![0, 1, 3, 4, 5, 9, 17, 33, 129]
    } else {
        (0..70).chain([127, 128, 129, 1000]).collect()
    };
    for sel in test_selections() {
        for &len in &lens {
            let src = random_pixels(&mut rng, len);
            let dst0 = random_pixels(&mut rng, len);
            let expected: Vec<u32> = dst0
                .iter()
                .zip(&src)
                .map(|(d, s)| {
                    if sel.tier == Tier::Neon {
                        src_over_neon(*s, *d)
                    } else {
                        pm_src_over(*s, *d)
                    }
                })
                .collect();
            let mut dst = dst0.clone();
            blit_row_s32a_opaque_with(sel, &mut dst, &src, 0xFF);
            assert_eq!(dst, expected, "{sel}, len {len}");
        }
    }
}

#[test]
fn scalar_twin_is_pm_src_over() {
    let mut rng = Rng(5);
    let src = random_pixels(&mut rng, 37);
    let dst0 = random_pixels(&mut rng, 37);
    let mut dst = dst0.clone();
    blit_row_s32a_opaque_scalar(&mut dst, &src, 0xFF);
    for i in 0..37 {
        assert_eq!(dst[i], pm_src_over(src[i], dst0[i]));
    }
}

#[test]
fn leaves_the_rest_of_dst_alone() {
    let mut rng = Rng(7);
    for sel in test_selections() {
        let src = random_pixels(&mut rng, 40);
        let dst0 = random_pixels(&mut rng, 40);
        let mut dst = dst0.clone();
        blit_row_s32a_opaque_with(sel, &mut dst[..13], &src, 0xFF);
        assert_eq!(dst[13..], dst0[13..], "{sel}");
    }
}

#[test]
fn opaque_src_replaces_and_transparent_src_keeps() {
    for sel in test_selections() {
        let mut dst = [0x1234_5678u32; 9];
        blit_row_s32a_opaque_with(sel, &mut dst, &[0xFF11_2233; 9], 0xFF);
        assert_eq!(dst, [0xFF11_2233; 9], "{sel}");
        let mut dst = [0x1234_5678u32; 9];
        blit_row_s32a_opaque_with(sel, &mut dst, &[0; 9], 0xFF);
        // A fully transparent src adds nothing, but dst is scaled by 256/256 = unchanged for
        // the x86 formula and exactly for Neon.
        assert_eq!(dst, [0x1234_5678; 9], "{sel}");
    }
}

#[test]
fn color32_matches_scalar_twin() {
    let mut rng = Rng(99);
    for alpha in (1..=254u32).step_by(if cfg!(miri) { 41 } else { 1 }) {
        let color = (alpha << 24) | (rng.next_u32() & 0x00FF_FFFF);
        for len in [0, 1, 3, 4, 5, 8, 9, 31] {
            let dst0 = random_pixels(&mut rng, len);
            let (mut a, mut b) = (dst0.clone(), dst0);
            blit_row_color32(&mut a, color);
            blit_row_color32_scalar(&mut b, color);
            assert_eq!(a, b, "alpha {alpha}, len {len}");
        }
    }
}

#[test]
fn color32_known_values() {
    // 50% black over opaque white: 255 * (256 - 128) >> 8 = 127 per channel, alpha 255.
    let mut px = [0xFFFF_FFFFu32; 5];
    blit_row_color32(&mut px, 0x8000_0000);
    assert_eq!(px, [0xFF7F_7F7F; 5]);
}
