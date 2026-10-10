// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/BezierCurveTest.cpp (chrome/m156)

#![cfg(test)]
// The float literals are copied verbatim from the C++ tests, digits and all.
#![allow(clippy::unreadable_literal, clippy::excessive_precision)]

use skia_rust_core::bezier_curves::{BezierCubic, BezierQuad};
use skia_rust_core::floating_point::{
    double_nearly_zero, double_to_float, doubles_nearly_equal_ulps_max_diff,
};
use skia_rust_core::quads;

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/BezierCurveTest.cpp#L23-L26 (chrome/m156)
// Grouping the test inputs into DoublePoints makes the test cases easier to read.
#[derive(Clone, Copy)]
struct DoublePoint {
    x: f64,
    y: f64,
}

// Port of: tests/BezierCurveTest.cpp#L28-L33 (chrome/m156)
fn nearly_equal(expected: f64, actual: f64) -> bool {
    if double_nearly_zero(expected) {
        return double_nearly_zero(actual);
    }
    doubles_nearly_equal_ulps_max_diff(expected, actual, 64)
}

// Flattens the `DoublePoint`s into the `double[8]` layout the C++ reads through
// `reinterpret_cast`: x0, y0, x1, y1, ...
fn flatten(curve_inputs: &[DoublePoint; 4]) -> [f64; 8] {
    let mut flat = [0.0; 8];
    for (i, p) in curve_inputs.iter().enumerate() {
        flat[2 * i] = p.x;
        flat[2 * i + 1] = p.y;
    }
    flat
}

// Port of: tests/BezierCurveTest.cpp#L35-L50 (chrome/m156)
fn test_cubic_eval_at_t(
    reporter: &mut Reporter,
    name: &str,
    curve_inputs: &[DoublePoint; 4],
    t: f64,
    expected_xy: DoublePoint,
) {
    reporter.set_context(Some(name.to_string()));
    reporter_assert!(
        reporter,
        curve_inputs.len() == 4,
        "Invalid test case. Should have 4 input points."
    );
    reporter_assert!(
        reporter,
        (0.0..=1.0).contains(&t),
        "Invalid test case. t {:.6} should be in [0, 1]",
        t
    );
    let [x, y] = BezierCubic::eval_at(&flatten(curve_inputs), t);
    reporter_assert!(
        reporter,
        nearly_equal(expected_xy.x, x),
        "X wrong {:.16} != {:.16}",
        expected_xy.x,
        x
    );
    reporter_assert!(
        reporter,
        nearly_equal(expected_xy.y, y),
        "Y wrong {:.16} != {:.16}",
        expected_xy.y,
        y
    );
    reporter.set_context(None);
}

// Port of: tests/BezierCurveTest.cpp#L52-L87 (chrome/m156)
def_test!(BezierCubicEvalAt, |reporter| {
    let dp = |x: f64, y: f64| DoublePoint { x, y };

    test_cubic_eval_at_t(
        reporter,
        "linear curve @0.1234",
        &[dp(0.0, 0.0), dp(0.0, 0.0), dp(10.0, 10.0), dp(10.0, 10.0)],
        0.1234,
        dp(0.4192451819200000, 0.4192451819200000),
    );
    test_cubic_eval_at_t(
        reporter,
        "linear curve @0.2345",
        &[dp(0.0, 0.0), dp(5.0, 5.0), dp(5.0, 5.0), dp(10.0, 10.0)],
        0.2345,
        dp(2.8215983862500000, 2.8215983862500000),
    );
    let arbitrary = [
        dp(-10.0, -20.0),
        dp(-7.0, 5.0),
        dp(14.0, -2.0),
        dp(3.0, 13.0),
    ];
    test_cubic_eval_at_t(
        reporter,
        "Arbitrary Cubic, t=0.0",
        &arbitrary,
        0.0,
        dp(-10.0, -20.0),
    );
    test_cubic_eval_at_t(
        reporter,
        "Arbitrary Cubic, t=0.3456",
        &arbitrary,
        0.3456,
        dp(-2.503786700800000, -3.31715344793600),
    );
    test_cubic_eval_at_t(
        reporter,
        "Arbitrary Cubic, t=0.5",
        &arbitrary,
        0.5,
        dp(1.75, 0.25),
    );
    test_cubic_eval_at_t(
        reporter,
        "Arbitrary Cubic, t=0.7891",
        &arbitrary,
        0.7891,
        dp(6.158763291450000, 5.938550084434000),
    );
    test_cubic_eval_at_t(
        reporter,
        "Arbitrary Cubic, t=1.0",
        &arbitrary,
        1.0,
        dp(3.0, 13.0),
    );
});

// Port of: tests/BezierCurveTest.cpp#L89-L105 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ helper's signature
fn test_cubic_convert_to_polynomial(
    reporter: &mut Reporter,
    name: &str,
    curve_inputs: &[DoublePoint; 4],
    y_values: bool,
    expected_a: f64,
    expected_b: f64,
    expected_c: f64,
    expected_d: f64,
) {
    reporter.set_context(Some(name.to_string()));
    reporter_assert!(
        reporter,
        curve_inputs.len() == 4,
        "Invalid test case. Need 4 points (start, control, control, end)"
    );
    reporter.set_context(Some(format!("{name}: SkBezierCurve Implementation")));
    let [a, b, c, d] = BezierCubic::convert_to_polynomial(&flatten(curve_inputs), y_values);
    reporter_assert!(
        reporter,
        nearly_equal(expected_a, a),
        "{:.6} != {:.6}",
        expected_a,
        a
    );
    reporter_assert!(
        reporter,
        nearly_equal(expected_b, b),
        "{:.6} != {:.6}",
        expected_b,
        b
    );
    reporter_assert!(
        reporter,
        nearly_equal(expected_c, c),
        "{:.6} != {:.6}",
        expected_c,
        c
    );
    reporter_assert!(
        reporter,
        nearly_equal(expected_d, d),
        "{:.6} != {:.6}",
        expected_d,
        d
    );
    reporter.set_context(None);
}

// Port of: tests/BezierCurveTest.cpp#L107-L117 (chrome/m156)
def_test!(BezierCubicToPolynomials, |reporter| {
    let dp = |x: f64, y: f64| DoublePoint { x, y };
    let points = [dp(1.0, 2.0), dp(-3.0, 4.0), dp(5.0, -6.0), dp(7.0, 8.0)];

    // See also tests/PathOpsDCubicTest.cpp->SkDCubicPolynomialCoefficients
    test_cubic_convert_to_polynomial(
        reporter,
        "Arbitrary control points X direction",
        &points,
        false, /*=yValues*/
        -18.0,
        36.0,
        -12.0,
        1.0,
    );
    test_cubic_convert_to_polynomial(
        reporter,
        "Arbitrary control points Y direction",
        &points,
        true, /*=yValues*/
        36.0,
        -36.0,
        6.0,
        2.0,
    );
});

// Port of: tests/BezierCurveTest.cpp#L121-L197 (chrome/m156)
// Since, Roots and EvalAt are separately unit tested, make sure that the parametric pramater t
// is correctly in range, and checked.
def_test!(QuadRoots_CheckTRange, |reporter| {
    // Pick interesting numbers around 0 and 1.
    let interesting_roots = [
        -1000.0, -10.0, -1.0, -0.1, -0.0001, 0.0, 0.0001, 0.1, 0.9, 0.9999, 1.0, 1.0001, 1.1, 10.0,
        1000.0,
    ];
    // Interesting scales to make the quadratic.
    let interesting_scales = [
        -1000.0, -10.0, -1.0, -0.1, -0.0001, 0.0001, 0.1, 1.0, 10.0, 1000.0,
    ];
    let outside_t_range = |r: f64| r < 0.0 || 1.0 < r;
    let inside_t_range = |r: f64| !outside_t_range(r);
    // The original test for AddValidTs (which quad intersect was based on) used 1 float ulp of
    // leeway for comparison. Tighten this up to half a float ulp.
    // When converted to float, a double will be rounded up to half a float ulp for a double
    // value between two float values.
    #[allow(clippy::float_cmp)] // exact comparison of rounded floats, as in the C++ test
    let equal_as_float = |a: f64, b: f64| double_to_float(a) == double_to_float(b);

    for &r1 in &interesting_roots {
        for &r0 in &interesting_roots {
            for &s in &interesting_scales {
                // Create a quadratic using the roots r0 and r1.
                // s(x-r0)(x-r1) = s(x^2 - r0*x - r1*x + r0*r1)
                let a = s;
                // Normally B = -(r0 + r1) but this needs the modified B' = (r0 + r1) / 2.
                let b = s * 0.5 * (r0 + r1);
                let c = s * r0 * r1;
                // The X coefficients are set to return t's generated by root intersection.
                // The offset is set to 0, because an arbitrary offset is essentially encoded in C.
                let mut storage = [0.0f32; 2];
                let intersections =
                    BezierQuad::intersect(0.0, -0.5, 0.0, a, b, c, 0.0, &mut storage);
                if intersections.is_empty() {
                    // Either imaginary or both roots are outside [0, 1].
                    reporter_assert!(
                        reporter,
                        quads::discriminant(a, b, c) < 0.0
                            || (outside_t_range(r0) && outside_t_range(r1))
                    );
                } else if intersections.len() == 1 {
                    // One of the roots is outside [0, 1]
                    reporter_assert!(reporter, inside_t_range(r0) || inside_t_range(r1));
                    let inside_root = if inside_t_range(r0) { r0 } else { r1 };
                    reporter_assert!(
                        reporter,
                        equal_as_float(inside_root, f64::from(intersections[0]))
                    );
                } else {
                    reporter_assert!(reporter, intersections.len() == 2);
                    reporter_assert!(reporter, inside_t_range(r0) && inside_t_range(r1));
                    // std::minmax(a, b): the smaller first, and `a` on a tie.
                    let (smaller, bigger) = if intersections[1] < intersections[0] {
                        (intersections[1], intersections[0])
                    } else {
                        (intersections[0], intersections[1])
                    };
                    let (smaller_root, bigger_root) = if r1 < r0 { (r1, r0) } else { (r0, r1) };
                    reporter_assert!(reporter, equal_as_float(f64::from(smaller), smaller_root));
                    reporter_assert!(reporter, equal_as_float(f64::from(bigger), bigger_root));
                }
            }
        }
    }
    // Check when A == 0.
    {
        let a = 0.0;
        // We need M = 4, so that will be a Kahan style B of -0.5 * M = -2.
        let b = -2.0;
        let c = -1.0;
        // Assume the offset is already encoded in C.
        let mut storage = [0.0f32; 2];
        let intersections = BezierQuad::intersect(0.0, -0.5, 0.0, a, b, c, 0.0, &mut storage);
        reporter_assert!(reporter, intersections.len() == 1);
        #[allow(clippy::float_cmp)] // exact expected root, as in the C++ test
        {
            reporter_assert!(reporter, intersections[0] == 0.25);
        }
    }
});

// Port of: tests/BezierCurveTest.cpp#L201-L264 (chrome/m156)
// Since, Roots and EvalAt are separately unit tested, make sure that the parametric pramater t
// is correctly in range, and checked.
def_test!(SkBezierCubic_CheckTRange, |reporter| {
    // Pick interesting numbers around 0 and 1.
    let interesting_roots = [-10.0, -5.0, -2.0, -1.0, 0.0, 0.5, 1.0, 2.0, 5.0, 10.0];
    // Interesting scales to make the quadratic.
    let interesting_scales = [
        -1000.0, -10.0, -1.0, -0.1, -0.0001, 0.0001, 0.1, 1.0, 10.0, 1000.0,
    ];
    let outside_t_range = |r: f64| r < 0.0 || 1.0 < r;
    let inside_t_range = |r: f64| !outside_t_range(r);
    let special_equal = |actual: f64, test: f64| {
        // At least a floats worth of digits are correct.
        let error_factor = f64::from(f32::EPSILON);
        (test - actual).abs() <= error_factor * test.abs().max(actual.abs())
    };

    for &r2 in &interesting_roots {
        for &r1 in &interesting_roots {
            for &r0 in &interesting_roots {
                for &s in &interesting_scales {
                    // Create a cubic using the roots r0, r1, and r2.
                    // s(x-r0)(x-r1)(x-r2) = s(x^3 - (r0+r1+r2)x^2 + (r0r1+r1r2+r0r2)x - r0r1r2)
                    let a = s;
                    let b = -s * (r0 + r1 + r2);
                    let c = s * (r0 * r1 + r1 * r2 + r0 * r2);
                    let d = -s * r0 * r1 * r2;
                    // Accumulate all the valid t's into a set: sorted and unique.
                    let mut in_range_roots: Vec<f64> = Vec::new();
                    for r in [r0, r1, r2] {
                        if inside_t_range(r) && !in_range_roots.contains(&r) {
                            in_range_roots.push(r);
                        }
                    }
                    // The X coefficients are set to return t's generated by root intersection.
                    // The offset is set to 0, because an arbitrary offset is essentially encoded
                    // in C.
                    let mut storage = [0.0f32; 3];
                    let intersections =
                        BezierCubic::intersect(0.0, 0.0, 1.0, 0.0, a, b, c, d, 0.0, &mut storage);
                    let mut correct = 0usize;
                    for &candidate in intersections {
                        for &answer in &in_range_roots {
                            if special_equal(f64::from(candidate), answer) {
                                correct += 1;
                                break;
                            }
                        }
                    }
                    reporter_assert!(reporter, correct == intersections.len());
                }
            }
        }
    }
});
