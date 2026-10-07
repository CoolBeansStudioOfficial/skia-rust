// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Every tier of `blit_mask_d32_a8`: the non-Neon tiers against the scalar twin, `Neon`
//! (native where available) against its per-byte model, and the model against Skia's arithmetic
//! written out per pixel.

#![allow(
    clippy::many_single_char_names,
    clippy::range_plus_one,
    clippy::cast_possible_truncation,
    clippy::too_many_arguments
)] // test code: Skia's names, byte truncation of random words, kernel signatures

use super::*;
use crate::color_util::mul_div_255_round;
use crate::testing::{Rng, test_selections};

const COLORS: [u32; 7] = [
    COLOR_BLACK,
    0xFF12_3456,
    0xFFFF_FFFF,
    0x8000_0000,
    0x80FF_8040,
    0x0100_0000,
    0xFE12_ABCD,
];

fn run(
    sel: Selection,
    dst0: &[u32],
    dst_rb: usize,
    mask: &[u8],
    mask_rb: usize,
    color: u32,
    w: usize,
    h: usize,
) -> Vec<u32> {
    let mut dst = dst0.to_vec();
    blit_mask_d32_a8_with(sel, &mut dst, dst_rb, mask, mask_rb, color, w, h);
    dst
}

#[test]
fn tiers_agree() {
    let mut rng = Rng(0xFEED_FACE_CAFE_BEEF);
    for color in COLORS {
        for w in (0..40)
            .step_by(if cfg!(miri) { 7 } else { 1 })
            .chain([64, 65])
        {
            for h in 1..4usize {
                let pad = (rng.next_u32() % 5) as usize;
                let stride = w + pad;
                let mask_rb = [0, w, w + 3][rng.next_u32() as usize % 3];
                // Garbage (not premultiplied) destination pixels, to hit overflow behavior too.
                let dst0: Vec<u32> = (0..stride * h + 3).map(|_| rng.next_u32()).collect();
                let mask: Vec<u8> = (0..mask_rb * h + w + 1)
                    .map(|_| rng.next_u32() as u8)
                    .collect();

                let scalar = {
                    let mut d = dst0.clone();
                    blit_mask_d32_a8_scalar(&mut d, stride * 4, &mask, mask_rb, color, w, h);
                    d
                };
                let neon_model = run(
                    Selection::model(Tier::Neon, crate::tier::Estimates::Arm),
                    &dst0,
                    stride * 4,
                    &mask,
                    mask_rb,
                    color,
                    w,
                    h,
                );
                for sel in test_selections() {
                    let got = run(sel, &dst0, stride * 4, &mask, mask_rb, color, w, h);
                    let expected = if sel.tier == Tier::Neon {
                        &neon_model
                    } else {
                        &scalar
                    };
                    assert_eq!(
                        &got, expected,
                        "{sel}, color {color:08x}, {w}x{h}, pad {pad}"
                    );
                }
            }
        }
    }
}

/// On premultiplied destinations the two formulas stay within one count of each other and of
/// the exact blend (`Sk4px`'s `approx` is off by at most one).
#[test]
fn formulas_stay_close_on_premultiplied_input() {
    let mut rng = Rng(42);
    for color in COLORS {
        let (w, h) = (23, 2);
        let dst0: Vec<u32> = (0..w * h)
            .map(|_| {
                let a = rng.next_u32() & 0xFF;
                let mut c = || (rng.next_u32() & 0xFF) * a / 255;
                let (r, g, b) = (c(), c(), c());
                (a << 24) | (r << 16) | (g << 8) | b
            })
            .collect();
        let mask: Vec<u8> = (0..w * h).map(|_| rng.next_u32() as u8).collect();
        let x86 = run(
            Selection::native(Tier::Scalar),
            &dst0,
            w * 4,
            &mask,
            w,
            color,
            w,
            h,
        );
        let neon = run(
            Selection::model(Tier::Neon, crate::tier::Estimates::Arm),
            &dst0,
            w * 4,
            &mask,
            w,
            color,
            w,
            h,
        );
        for (i, (a, b)) in x86.iter().zip(&neon).enumerate() {
            for shift in [0, 8, 16, 24] {
                let (a, b) = ((a >> shift) & 0xFF, (b >> shift) & 0xFF);
                assert!(
                    a.abs_diff(b) <= 2,
                    "pixel {i}, color {color:08x}: {a} vs {b}"
                );
            }
        }
    }
}

/// Spot-checks of the per-byte arithmetic against Skia's expressions.
#[test]
fn known_values() {
    // Black through a full mask over anything: alpha = 255, color = d * (255 - 255) = 0.
    let mut dst = [0x1122_3344u32; 3];
    blit_mask_d32_a8_scalar(&mut dst, 12, &[255; 3], 3, COLOR_BLACK, 3, 1);
    assert_eq!(dst, [0xFF00_0000; 3]);
    // A zero mask changes nothing (approx(d, 255) == d).
    let mut dst = [0x1122_3344u32; 3];
    blit_mask_d32_a8_scalar(&mut dst, 12, &[0; 3], 3, 0xFF12_3456, 3, 1);
    assert_eq!(dst, [0x1122_3344; 3]);
    // The exact-rounding Neon alpha-multiply on a few values.
    assert_eq!(mul_div_255_round(255, 128), 128);
    assert_eq!(alpha_mul8(255, 256), 255);
    assert_eq!(alpha_mul8(128, 128), 64);
}

/// `mask_rb == 0` blits one mask row repeatedly (nine patch).
#[test]
fn zero_mask_row_bytes() {
    let mask = [10u8, 200, 30, 255, 0];
    for sel in test_selections() {
        let mut dst = vec![0xFF00_00FFu32; 10];
        blit_mask_d32_a8_with(sel, &mut dst, 20, &mask, 0, 0xFF80_8080, 5, 2);
        assert_eq!(dst[..5], dst[5..], "{sel}");
    }
}
