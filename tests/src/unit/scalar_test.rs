// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ScalarTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::float_bits::float_to_bits;
use skia_rust_core::floating_point::{is_finite, is_nan};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{SCALAR_INFINITY, SCALAR_NAN, scalar, scalar_round_to_int};

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/ScalarTest.cpp#L26-L47 (chrome/m156)
fn test_roundtoint(reporter: &mut Reporter) {
    let mut x: scalar = 0.499_999_97;
    let mut ix = scalar_round_to_int(x);
    // mirrors `(int) floorf(x + 0.5f)`; the value is in range
    #[allow(clippy::cast_possible_truncation)]
    let mut bad_ix = (x + 0.5_f32).floor() as i32;
    // We should get 0, since x < 0.5, but we wouldn't if SkScalarRoundToInt uses the commonly
    // recommended approach shown in 'badIx' due to float addition rounding up the low
    // bit after adding 0.5.
    reporter_assert!(reporter, 0 == ix);
    reporter_assert!(reporter, 1 == bad_ix);

    // Additionally, when the float value is between (2^23,2^24], it's precision is equal to
    // 1 integral value. Adding 0.5f rounds up automatically *before* the floor, so naive
    // rounding is also incorrect. Float values <= 2^23 and > 2^24 don't have this problem
    // because either the sum can be represented sufficiently for floor() to do the right thing,
    // or the sum will always round down to the integer multiple.
    x = 8_388_609.0;
    ix = scalar_round_to_int(x);
    #[allow(clippy::cast_possible_truncation)] // mirrors `(int) floorf(x + 0.5f)`; in range
    {
        bad_ix = (x + 0.5_f32).floor() as i32;
    }
    reporter_assert!(reporter, 8_388_609 == ix);
    reporter_assert!(reporter, 8_388_610 == bad_ix);
}

// Port of: tests/ScalarTest.cpp#L54-L101 (chrome/m156)
fn test_is_rect_finite(reporter: &mut Reporter) {
    struct GSets<'a> {
        f_pts: &'a [Point],
        f_is_finite: bool,
    }

    let g_f0 = [Point::new(0.0, 0.0), Point::new(1.0, 1.0)];
    let g_f1 = [
        Point::new(0.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(99.234, -42342.0),
    ];

    let g_bad0 = [
        Point::new(0.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(99.234, -42342.0),
        Point::new(SCALAR_NAN, 3.0),
        Point::new(2.0, 3.0),
    ];
    let g_bad1 = [
        Point::new(0.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(99.234, -42342.0),
        Point::new(3.0, SCALAR_NAN),
        Point::new(2.0, 3.0),
    ];
    let g_bad2 = [
        Point::new(0.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(99.234, -42342.0),
        Point::new(SCALAR_INFINITY, 3.0),
        Point::new(2.0, 3.0),
    ];
    let g_bad3 = [
        Point::new(0.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(99.234, -42342.0),
        Point::new(3.0, SCALAR_INFINITY),
        Point::new(2.0, 3.0),
    ];

    let g_sets = [
        GSets {
            f_pts: &g_f0,
            f_is_finite: true,
        },
        GSets {
            f_pts: &g_f1,
            f_is_finite: true,
        },
        GSets {
            f_pts: &g_bad0,
            f_is_finite: false,
        },
        GSets {
            f_pts: &g_bad1,
            f_is_finite: false,
        },
        GSets {
            f_pts: &g_bad2,
            f_is_finite: false,
        },
        GSets {
            f_pts: &g_bad3,
            f_is_finite: false,
        },
    ];

    for set in &g_sets {
        let r = Rect::bounds_or_empty(set.f_pts);
        let rect_is_finite = !r.is_empty();
        reporter_assert!(reporter, set.f_is_finite == rect_is_finite);
    }
}

// Port of: tests/ScalarTest.cpp#L103-L107 (chrome/m156)
fn is_finite_int(x: f32) -> bool {
    let bits: u32 = float_to_bits(x); // need unsigned for our shifts
    let exponent = (bits << 1) >> 24;
    exponent != 0xFF
}

// Port of: tests/ScalarTest.cpp#L109-L111 (chrome/m156)
fn is_finite_float(x: f32) -> bool {
    is_finite(x)
}

// Port of: tests/ScalarTest.cpp#L113-L116 (chrome/m156)
#[allow(clippy::eq_op, clippy::float_cmp)] // y == y is the NaN test
fn is_finite_mulzero(x: f32) -> bool {
    let y = x * 0.0;
    y == y
}

// return true if the float is finite
type IsFiniteProc1 = fn(f32) -> bool;

// Port of: tests/ScalarTest.cpp#L121-L123 (chrome/m156)
fn is_finite2_and(x: f32, y: f32, proc: IsFiniteProc1) -> bool {
    proc(x) && proc(y)
}

// Port of: tests/ScalarTest.cpp#L125-L127 (chrome/m156)
fn is_finite2_mulzeroadd(x: f32, y: f32, proc: IsFiniteProc1) -> bool {
    proc(x * 0.0 + y * 0.0)
}

// return true if both floats are finite
type IsFiniteProc2 = fn(f32, f32, IsFiniteProc1) -> bool;

// Port of: tests/ScalarTest.cpp#L132-L136 (chrome/m156)
#[derive(Copy, Clone, PartialEq, Eq)]
enum FloatClass {
    Finite,
    Infinite,
    NaN,
}

// Port of: tests/ScalarTest.cpp#L138-L143 (chrome/m156)
fn test_floatclass(reporter: &mut Reporter, value: f32, fc: FloatClass) {
    // our sk_float_is... function may return int instead of bool,
    // hence the double ! to turn it into a bool
    reporter_assert!(reporter, is_finite(value) == (fc == FloatClass::Finite));
    reporter_assert!(
        reporter,
        value.is_infinite() == (fc == FloatClass::Infinite)
    );
    reporter_assert!(reporter, is_nan(value) == (fc == FloatClass::NaN));
}

// Port of: tests/ScalarTest.cpp#L145-L215 (chrome/m156)
fn test_isfinite(reporter: &mut Reporter) {
    struct Rec {
        f_value: f32,
        f_is_finite: bool,
    }

    #[allow(clippy::excessive_precision)] // the C++ literal, kept verbatim
    let max: f32 = 3.402_823_466e+38;
    // we are intentionally causing an overflow here
    let inf: f32 = max * max;
    let nan: f32 = inf * 0.0;

    test_floatclass(reporter, 0.0, FloatClass::Finite);
    test_floatclass(reporter, max, FloatClass::Finite);
    test_floatclass(reporter, -max, FloatClass::Finite);
    test_floatclass(reporter, inf, FloatClass::Infinite);
    test_floatclass(reporter, -inf, FloatClass::Infinite);
    test_floatclass(reporter, nan, FloatClass::NaN);
    test_floatclass(reporter, -nan, FloatClass::NaN);

    let data = [
        Rec {
            f_value: 0.0,
            f_is_finite: true,
        },
        Rec {
            f_value: 1.0,
            f_is_finite: true,
        },
        Rec {
            f_value: -1.0,
            f_is_finite: true,
        },
        Rec {
            f_value: max * 0.75,
            f_is_finite: true,
        },
        Rec {
            f_value: max,
            f_is_finite: true,
        },
        Rec {
            f_value: -max * 0.75,
            f_is_finite: true,
        },
        Rec {
            f_value: -max,
            f_is_finite: true,
        },
        Rec {
            f_value: inf,
            f_is_finite: false,
        },
        Rec {
            f_value: -inf,
            f_is_finite: false,
        },
        Rec {
            f_value: nan,
            f_is_finite: false,
        },
    ];

    let g_proc1: [IsFiniteProc1; 3] = [is_finite_int, is_finite_float, is_finite_mulzero];
    let g_proc2: [IsFiniteProc2; 2] = [is_finite2_and, is_finite2_mulzeroadd];

    for rec in &data {
        for proc1 in g_proc1 {
            let finite = proc1(rec.f_value);
            reporter_assert!(reporter, rec.f_is_finite == finite);
        }
    }

    for rec0 in &data {
        for rec1 in &data {
            for proc1 in g_proc1 {
                for proc2 in g_proc2 {
                    let finite = proc2(rec0.f_value, rec1.f_value, proc1);
                    let finite2 = rec0.f_is_finite && rec1.f_is_finite;
                    reporter_assert!(reporter, finite2 == finite);
                }
            }
        }
    }

    test_is_rect_finite(reporter);
}

// Port of: tests/ScalarTest.cpp#L212-L215 (chrome/m156)
def_test!(Scalar, |reporter| {
    test_isfinite(reporter);
    test_roundtoint(reporter);
});
