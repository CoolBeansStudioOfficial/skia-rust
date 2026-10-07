// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ColorTest.cpp (chrome/m156)

use skia_rust_core::color::{
    Color, PMColor, pm_color_get_a, pm_color_get_b, pm_color_get_g, pm_color_get_r,
    pm_color_set_argb, pre_multiply_color,
};
use skia_rust_core::color_data::{fast_four_byte_interp, four_byte_interp};
use skia_rust_core::math_priv::mul_div_255_ceiling;
use skia_rust_core::random::Random;
use skia_rust_core::un_pre_multiply::pm_color_to_color;

use crate::{def_test, reporter_assert};

// Port of: tests/ColorTest.cpp#L22-L40 (chrome/m156)
def_test!(ColorPremul, |reporter| {
    for a in 0..=255u8 {
        for x in 0..=255u8 {
            let c0 = Color::from_argb(a, x, x, x);
            let p0 = pre_multiply_color(c0);

            let c1 = pm_color_to_color(p0);
            let p1 = pre_multiply_color(c1);

            // we can't promise that c0 == c1, since c0 -> p0 is a many to one
            // function, however, we can promise that p0 -> c1 -> p1 : p0 == p1
            reporter_assert!(reporter, p0 == p1);

            {
                let ax = mul_div_255_ceiling(u32::from(x), u32::from(a));
                reporter_assert!(reporter, ax <= u32::from(a));
            }
        }
    }
});

// Port of: tests/ColorTest.cpp#L42-L56 (chrome/m156)
def_test!(SkPMColor_SetAndRetrieveChannels, |reporter| {
    let pmc: PMColor = pm_color_set_argb(0xFE, 0xDC, 0xBA, 0x98);

    // SK_PMCOLOR_IS_RGBA
    reporter_assert!(reporter, pmc == 0xFE98_BADC);

    reporter_assert!(reporter, pm_color_get_a(pmc) == 0xFE);
    reporter_assert!(reporter, pm_color_get_r(pmc) == 0xDC);
    reporter_assert!(reporter, pm_color_get_g(pmc) == 0xBA);
    reporter_assert!(reporter, pm_color_get_b(pmc) == 0x98);
});

// This test fails: SkFourByteInterp does *not* preserve opaque destinations.
// SkAlpha255To256 implemented as (alpha + 1) is faster than
// (alpha + (alpha >> 7)), but inaccurate, and Skia intends to phase it out.
// Port of: tests/ColorTest.cpp#L58-L80 (chrome/m156)
def_test!(ColorInterp, |reporter| {
    let mut r = Random::default();

    let a0 = 0;
    let a255 = 255;
    for _ in 0..200 {
        let color_src = Color::new(r.next_u());
        let color_dst = Color::new(r.next_u());
        let src = pre_multiply_color(color_src);
        let dst = pre_multiply_color(color_dst);

        if false {
            reporter_assert!(reporter, four_byte_interp(src, dst, a0) == dst);
            reporter_assert!(reporter, four_byte_interp(src, dst, a255) == src);
        }
    }
});

// Port of: tests/ColorTest.cpp#L82-L91 (chrome/m156)
def_test!(ColorFastIterp, |reporter| {
    let mut r = Random::default();

    let a0 = 0;
    let a255 = 255;
    for _ in 0..200 {
        let color_src = Color::new(r.next_u());
        let color_dst = Color::new(r.next_u());
        let src = pre_multiply_color(color_src);
        let dst = pre_multiply_color(color_dst);

        reporter_assert!(reporter, fast_four_byte_interp(src, dst, a0) == dst);
        reporter_assert!(reporter, fast_four_byte_interp(src, dst, a255) == src);
    }
});
