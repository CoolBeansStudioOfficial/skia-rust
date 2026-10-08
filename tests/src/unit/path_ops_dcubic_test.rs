// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsDCubicTest.cpp (chrome/m156)

#![cfg(test)]

use crate::unit::path_ops_test_common::CubicPts;
use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::floating_point::{double_nearly_zero, doubles_nearly_equal_ulps_max_diff};
use skia_rust_pathops::cubic::DCubic;
use skia_rust_pathops::point::DPoint;

// Port of: tests/PathOpsDCubicTest.cpp (chrome/m156)
#[allow(clippy::excessive_precision, clippy::unreadable_literal)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
const HULL_TESTS: [CubicPts; 1] = [CubicPts::new([
    DPoint::new(2.6250000819563866, 2.3750000223517418),
    DPoint::new(2.833333432674408, 2.3333333432674408),
    DPoint::new(3.1111112236976624, 2.3333333134651184),
    DPoint::new(3.4074075222015381, 2.3333332538604736),
])];

def_test!(PathOpsCubicHull, |_reporter| {
    for c in &HULL_TESTS {
        let cubic = DCubic::new(c.pts);
        let mut order = [0u8; 4];
        let _ = cubic.convex_hull(&mut order);
    }
});

/// Port of `nearly_equal` of the test file.
// Port of: tests/PathOpsDCubicTest.cpp#L37-L42 (chrome/m156)
fn nearly_equal(expected: f64, actual: f64) -> bool {
    if double_nearly_zero(expected) {
        return double_nearly_zero(actual);
    }
    doubles_nearly_equal_ulps_max_diff(expected, actual, 64)
}

/// Port of `testConvertToPolynomial`.
// Port of: tests/PathOpsDCubicTest.cpp#L44-L63 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature of the test helper
fn test_convert_to_polynomial(
    reporter: &mut Reporter,
    name: &str,
    curve_inputs: &[DPoint],
    y_values: bool,
    expected_a: f64,
    expected_b: f64,
    expected_c: f64,
    expected_d: f64,
) {
    reporter.set_context(Some(name.to_string()));
    reporter_assert!(reporter, curve_inputs.len() == 4);
    let coords: [f64; 4] = if y_values {
        [
            curve_inputs[0].y,
            curve_inputs[1].y,
            curve_inputs[2].y,
            curve_inputs[3].y,
        ]
    } else {
        [
            curve_inputs[0].x,
            curve_inputs[1].x,
            curve_inputs[2].x,
            curve_inputs[3].x,
        ]
    };
    let (a, b, c, d) = DCubic::coefficients(coords);
    reporter_assert!(reporter, nearly_equal(expected_a, a));
    reporter_assert!(reporter, nearly_equal(expected_b, b));
    reporter_assert!(reporter, nearly_equal(expected_c, c));
    reporter_assert!(reporter, nearly_equal(expected_d, d));
    reporter.set_context(None);
}

def_test!(SkDCubicPolynomialCoefficients, |reporter| {
    let inputs = [
        DPoint::new(1.0, 2.0),
        DPoint::new(-3.0, 4.0),
        DPoint::new(5.0, -6.0),
        DPoint::new(7.0, 8.0),
    ];
    test_convert_to_polynomial(
        reporter,
        "Arbitrary control points X direction",
        &inputs,
        false, // =yValues
        -18.0,
        36.0,
        -12.0,
        1.0,
    );
    test_convert_to_polynomial(
        reporter,
        "Arbitrary control points Y direction",
        &inputs,
        true, // =yValues
        36.0,
        -36.0,
        6.0,
        2.0,
    );
});
