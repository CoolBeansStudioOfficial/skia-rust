// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/WangsFormulaTest.cpp (chrome/m156)

#![cfg(test)]
#![allow(clippy::cast_precision_loss)] // the C++ converts ints to floats implicitly
#![allow(clippy::cast_possible_truncation)] // the C++ converts floats to ints implicitly
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::float_cmp)] // the C++ compares scalars with ==
#![allow(clippy::many_single_char_names)] // the C++ names its points and parameters so
#![allow(clippy::similar_names)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::excessive_precision)]
#![allow(clippy::unusual_byte_groupings)] // the bit pattern is grouped like the C++ literal
#![allow(clippy::identity_op)]
// mirrors the C++ constant expression `(2 * 1) / 8.f`
// `0.5f * (a + b)` is kept as written: `midpoint` may round differently from the C++ arithmetic.
#![allow(clippy::manual_midpoint)]
#![allow(clippy::neg_cmp_op_on_partial_ord)] // REPORTER_ASSERT negates the condition
#![allow(clippy::items_after_statements)] // helper fns are declared next to the Skia test body

use crate::{def_test, reporter_assert};
use skia_rust_core::geometry::{chop_quad_at, eval_quad_at};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{SCALAR_NEARLY_ZERO, Scalar, scalar, scalar_ceil_to_int};
use skia_rust_gpu::tessellate::wangs_formula::{
    self, VectorXform, cubic, cubic_log2, quadratic, quadratic_log2,
};
use skia_rust_simd::vx::Double2;

use crate::Reporter;

// Port of: src/gpu/tessellate/Tessellation.h#L29 (chrome/m156), `constexpr static float kPrecision`.
const K_PRECISION: scalar = 4.0;

// Port of: tests/WangsFormulaTest.cpp#L24-L30 (chrome/m156)
const K_SERP: [Point; 4] = [
    Point::new(285.625, 499.687),
    Point::new(411.625, 808.188),
    Point::new(1064.62, 135.688),
    Point::new(1042.63, 585.187),
];

const K_LOOP: [Point; 4] = [
    Point::new(635.625, 614.687),
    Point::new(171.625, 236.188),
    Point::new(1064.62, 135.688),
    Point::new(516.625, 570.187),
];

const K_QUAD: [Point; 3] = [
    Point::new(460.625, 557.187),
    Point::new(707.121, 209.688),
    Point::new(779.628, 577.687),
];

/// `int` to `float`, as the C++ converts it implicitly.
fn int_to_float(x: i32) -> scalar {
    x as scalar
}

/// `std::ldexp(x, e)` for a float. Only the exponent range the tests use is supported (the
/// biased exponent must stay in `1..=254`), where scaling by a power of two is exact.
fn ldexp(x: scalar, e: i32) -> scalar {
    let biased = 127 + e;
    assert!(
        (1..=254).contains(&biased),
        "ldexp exponent out of range: {e}"
    );
    x * f32::from_bits((biased as u32) << 23)
}

/// `std::max(a, b)` for floats: `(a < b) ? b : a`.
fn std_max(a: scalar, b: scalar) -> scalar {
    if a < b { b } else { a }
}

/// `std::min(a, b)` for floats: `(b < a) ? b : a`.
fn std_min(a: scalar, b: scalar) -> scalar {
    if b < a { b } else { a }
}

/// `SkScalarNearlyEqual(x, y, tolerance)`.
fn nearly_equal(x: scalar, y: scalar, tolerance: scalar) -> bool {
    <scalar as Scalar>::nearly_equal(x, y, tolerance)
}

/// `SkScalarNearlyEqual(x, y)` with the default tolerance.
fn nearly_equal_default(x: scalar, y: scalar) -> bool {
    <scalar as Scalar>::nearly_equal(x, y, None)
}

/// `SkPoint::length()`.
fn length(p: Point) -> scalar {
    p.length()
}

// Port of: tests/WangsFormulaTest.cpp#L35-L38 (chrome/m156)
fn wangs_formula_quadratic_reference_impl(precision: scalar, p: &[Point]) -> scalar {
    let k = (int_to_float(2 * 1) / 8.0) * precision;
    (k * length(p[0] - p[1] * 2.0 + p[2])).sqrt()
}

// Port of: tests/WangsFormulaTest.cpp#L40-L44 (chrome/m156)
fn wangs_formula_cubic_reference_impl(precision: scalar, p: &[Point]) -> scalar {
    let k = (int_to_float(3 * 2) / 8.0) * precision;
    (k * std_max(
        length(p[0] - p[1] * 2.0 + p[2]),
        length(p[1] - p[2] * 2.0 + p[3]),
    ))
    .sqrt()
}

// Returns number of segments for linearized quadratic rational. This is an analogue to Wang's
// formula, taken from:
//
//   J. Zheng, T. Sederberg. "Estimating Tessellation Parameter Intervals for Rational Curves and
//   Surfaces." ACM Transactions on Graphics 19(1). 2000. See Thm 3, Corollary 1.
//
// Input points should be in projected space.
// Port of: tests/WangsFormulaTest.cpp#L51-L84 (chrome/m156)
fn wangs_formula_conic_reference_impl(precision: scalar, p: &[Point], w: scalar) -> scalar {
    // Compute center of bounding box in projected space.
    let mut min_x = p[0].x;
    let mut max_x = min_x;
    let mut min_y = p[0].y;
    let mut max_y = min_y;
    for pi in p.iter().take(3).skip(1) {
        min_x = std_min(min_x, pi.x);
        max_x = std_max(max_x, pi.x);
        min_y = std_min(min_y, pi.y);
        max_y = std_max(max_y, pi.y);
    }
    let c = Point::new(0.5 * (min_x + max_x), 0.5 * (min_y + max_y));

    // Translate control points and compute max length.
    let tp = [p[0] - c, p[1] - c, p[2] - c];
    let mut max_len: scalar = 0.0;
    for t in &tp {
        max_len = std_max(max_len, length(*t));
    }

    // Compute delta = parametric step size of linearization.
    let eps = 1.0 / precision;
    let r_minus_eps = std_max(0.0, max_len - eps);
    let min_w = std_min(w, 1.0);
    let numer = 4.0 * min_w * eps;
    let denom = length(tp[2] - tp[1] * 2.0 * w + tp[0]) + r_minus_eps * (1.0 - 2.0 * w + 1.0).abs();
    let delta = (numer / denom).sqrt();

    // Return corresponding num segments in the interval [tmin,tmax].
    let tmin: scalar = 0.0;
    let tmax: scalar = 1.0;
    (tmax - tmin) / delta
}

// Port of: tests/WangsFormulaTest.cpp#L86-L110 (chrome/m156)
fn for_random_matrices(rand: &mut Random, mut f: impl FnMut(&Matrix, &mut Random)) {
    let mut m = Matrix::default();
    m.set_identity();
    f(&m, rand);

    for i in -10..=30 {
        for j in -10..=30 {
            m.set_scale_x(ldexp(1.0 + rand.next_f(), i));
            m.set_skew_x(0.0);
            m.set_skew_y(0.0);
            m.set_scale_y(ldexp(1.0 + rand.next_f(), j));
            f(&m, rand);

            m.set_scale_x(ldexp(1.0 + rand.next_f(), i));
            m.set_skew_x(ldexp(1.0 + rand.next_f(), (j + i) / 2));
            m.set_skew_y(ldexp(1.0 + rand.next_f(), (j + i) / 2));
            m.set_scale_y(ldexp(1.0 + rand.next_f(), j));
            f(&m, rand);
        }
    }
}

// Port of: tests/WangsFormulaTest.cpp#L112-L127 (chrome/m156)
fn for_random_beziers(
    num_points: usize,
    rand: &mut Random,
    mut f: impl FnMut(&[Point], &mut Random),
    max_exponent: i32,
) {
    assert!(num_points <= 4);
    let mut pts = [Point::new(0.0, 0.0); 4];
    for i in -10..=max_exponent {
        for pt in pts.iter_mut().take(num_points) {
            *pt = Point::new(ldexp(1.0 + rand.next_f(), i), ldexp(1.0 + rand.next_f(), i));
        }
        f(&pts, rand);
    }
}

/// `for_random_beziers` with Skia's default `maxExponent = 30`.
fn for_random_beziers_default(
    num_points: usize,
    rand: &mut Random,
    f: impl FnMut(&[Point], &mut Random),
) {
    for_random_beziers(num_points, rand, f, 30);
}

/// Pass-through for the non-transform overloads of `wangs_formula::*`, which take the identity
/// `VectorXform` in Skia.
fn identity() -> VectorXform {
    VectorXform::default()
}

// Sets up a cubic such that the 'length' term in wang's formula == term.
//
//     f = sqrt(k * length(max(abs(p0 - p1*2 + p2), abs(p1 - p2*2 + p3))));
//
// Port of: tests/WangsFormulaTest.cpp#L144-L174 (chrome/m156), `setupCubicLengthTerm`.
fn setup_cubic_length_term(mut seed: i32, pts: &mut [Point; 4], term: scalar) {
    *pts = [Point::new(0.0, 0.0); 4];

    let mut term2d = if seed & 1 != 0 {
        Point::new(term, 0.0)
    } else {
        // `SkPoint::Make(.5f, std::sqrt(3)/2) * term`: the double sqrt narrows to float.
        Point::new(0.5, (3.0_f64.sqrt() / 2.0) as scalar) * term
    };
    seed >>= 1;

    if seed & 1 != 0 {
        term2d.x = -term2d.x;
    }
    seed >>= 1;

    if seed & 1 != 0 {
        std::mem::swap(&mut term2d.x, &mut term2d.y);
    }
    seed >>= 1;

    match seed % 4 {
        0 => {
            pts[0] = term2d;
            pts[3] = term2d * 0.75;
        }
        1 | 2 => {
            pts[1] = term2d * -0.5;
        }
        _ => {
            pts[3] = term2d;
            pts[0] = term2d * 0.75;
        }
    }
}

// Sets up a quadratic such that the 'length' term in wang's formula == term.
//
//     f = sqrt(k * length(p0 - p1*2 + p2));
//
// Port of: tests/WangsFormulaTest.cpp#L177-L202 (chrome/m156), `setupQuadraticLengthTerm`.
fn setup_quadratic_length_term(mut seed: i32, pts: &mut [Point; 4], term: scalar) {
    *pts = [Point::new(0.0, 0.0); 4];

    let mut term2d = if seed & 1 != 0 {
        Point::new(term, 0.0)
    } else {
        Point::new(0.5, (3.0_f64.sqrt() / 2.0) as scalar) * term
    };
    seed >>= 1;

    if seed & 1 != 0 {
        term2d.x = -term2d.x;
    }
    seed >>= 1;

    if seed & 1 != 0 {
        std::mem::swap(&mut term2d.x, &mut term2d.y);
    }
    seed >>= 1;

    match seed % 3 {
        0 => pts[0] = term2d,
        1 => pts[1] = term2d * -0.5,
        _ => pts[2] = term2d,
    }
}

// Port of: tests/WangsFormulaTest.cpp#L129-L288 (chrome/m156), the `check_cubic_log2` lambda.
fn check_cubic_log2(r: &mut Reporter, pts: &[Point]) {
    let id = identity();
    let f = std_max(1.0, wangs_formula_cubic_reference_impl(K_PRECISION, pts));
    let f_log2 = cubic_log2(K_PRECISION, pts, &id);
    reporter_assert!(r, scalar_ceil_to_int(f.log2()) == f_log2);
    let c = std_max(1.0, cubic(K_PRECISION, pts, &id));
    reporter_assert!(r, nearly_equal(c / f, 1.0, 1.0 / 128.0));
}

// Port of: tests/WangsFormulaTest.cpp#L129-L288 (chrome/m156), the `check_quadratic_log2` lambda.
fn check_quadratic_log2(r: &mut Reporter, pts: &[Point]) {
    let id = identity();
    let f = std_max(
        1.0,
        wangs_formula_quadratic_reference_impl(K_PRECISION, pts),
    );
    let f_log2 = quadratic_log2(K_PRECISION, pts, &id);
    reporter_assert!(r, scalar_ceil_to_int(f.log2()) == f_log2);
    let q = std_max(1.0, quadratic(K_PRECISION, pts, &id));
    reporter_assert!(r, nearly_equal(q / f, 1.0, 1.0 / 128.0));
}

// Ensure the optimized "*_log2" versions return the same value as ceil(std::log2(f)).
// Port of: tests/WangsFormulaTest.cpp#L129-L288 (chrome/m156)
def_test!(wangs_formula_log2, |r| {
    let id = identity();
    // Wang's formula and the cubic/quadratic routines use rsqrt-free sqrt here, so the
    // reference comparison is approximate: within ~1/2 tessellation segment is good enough.
    let k_tessellation_tolerance: scalar = 1.0 / 128.0;

    for level in 0..30 {
        let epsilon = ldexp(SCALAR_NEARLY_ZERO, level * 2);
        let mut pts = [Point::new(0.0, 0.0); 4];

        {
            // Test cubic boundaries.
            //     f = sqrt(k * length(max(abs(p0 - p1*2 + p2), abs(p1 - p2*2 + p3))));
            let k: scalar = int_to_float(3 * 2) / (8.0 * (1.0 / K_PRECISION));
            let x = ldexp(1.0, level * 2) / k;
            setup_cubic_length_term(level << 1, &mut pts, x - epsilon);
            let mut reference_value = wangs_formula_cubic_reference_impl(K_PRECISION, &pts);
            reporter_assert!(r, reference_value.log2().ceil() == int_to_float(level));
            let mut c = cubic(K_PRECISION, &pts, &id);
            reporter_assert!(
                r,
                nearly_equal(c / reference_value, 1.0, k_tessellation_tolerance)
            );
            reporter_assert!(r, cubic_log2(K_PRECISION, &pts, &id) == level);
            setup_cubic_length_term(level << 1, &mut pts, x + epsilon);
            reference_value = wangs_formula_cubic_reference_impl(K_PRECISION, &pts);
            reporter_assert!(r, reference_value.log2().ceil() == int_to_float(level + 1));
            c = cubic(K_PRECISION, &pts, &id);
            reporter_assert!(
                r,
                nearly_equal(c / reference_value, 1.0, k_tessellation_tolerance)
            );
            reporter_assert!(r, cubic_log2(K_PRECISION, &pts, &id) == level + 1);
        }

        {
            // Test quadratic boundaries.
            //     f = std::sqrt(k * Length(p0 - p1*2 + p2));
            let k: scalar = 2.0 / (8.0 * (1.0 / K_PRECISION));
            let x = ldexp(1.0, level * 2) / k;
            setup_quadratic_length_term(level << 1, &mut pts, x - epsilon);
            let mut reference_value = wangs_formula_quadratic_reference_impl(K_PRECISION, &pts);
            reporter_assert!(r, reference_value.log2().ceil() == int_to_float(level));
            let mut q = quadratic(K_PRECISION, &pts, &id);
            reporter_assert!(
                r,
                nearly_equal(q / reference_value, 1.0, k_tessellation_tolerance)
            );
            reporter_assert!(r, quadratic_log2(K_PRECISION, &pts, &id) == level);
            setup_quadratic_length_term(level << 1, &mut pts, x + epsilon);
            reference_value = wangs_formula_quadratic_reference_impl(K_PRECISION, &pts);
            reporter_assert!(r, reference_value.log2().ceil() == int_to_float(level + 1));
            q = quadratic(K_PRECISION, &pts, &id);
            reporter_assert!(
                r,
                nearly_equal(q / reference_value, 1.0, k_tessellation_tolerance)
            );
            reporter_assert!(r, quadratic_log2(K_PRECISION, &pts, &id) == level + 1);
        }
    }

    let mut rand = Random::default();

    for_random_matrices(&mut rand, |m, _rand| {
        let mut pts = [Point::new(0.0, 0.0); 4];
        m.map_points(&mut pts, &K_SERP);
        check_cubic_log2(r, &pts);

        m.map_points(&mut pts, &K_LOOP);
        check_cubic_log2(r, &pts);

        let mut quad = [Point::new(0.0, 0.0); 3];
        m.map_points(&mut quad, &K_QUAD);
        check_quadratic_log2(r, &quad);
    });

    for_random_beziers_default(4, &mut rand, |pts, _rand| {
        check_cubic_log2(r, pts);
    });

    for_random_beziers_default(3, &mut rand, |pts, _rand| {
        check_quadratic_log2(r, pts);
    });
});

// Ensure using transformations gives the same result as pre-transforming all points.
// Port of: tests/WangsFormulaTest.cpp#L290-L330 (chrome/m156)
def_test!(wangs_formula_vectorXforms, |r| {
    // Port of the `check_cubic_log2_with_transform` lambda.
    fn check_cubic_log2_with_transform(r: &mut Reporter, pts: &[Point], m: &Matrix) {
        let mut pts_xformed = [Point::new(0.0, 0.0); 4];
        m.map_points(&mut pts_xformed, &pts[..4]);
        let expected = cubic_log2(K_PRECISION, &pts_xformed, &identity());
        let actual = cubic_log2(K_PRECISION, pts, &VectorXform::from(m));
        reporter_assert!(r, actual == expected);
    }

    // Port of the `check_quadratic_log2_with_transform` lambda.
    fn check_quadratic_log2_with_transform(r: &mut Reporter, pts: &[Point], m: &Matrix) {
        let mut pts_xformed = [Point::new(0.0, 0.0); 3];
        m.map_points(&mut pts_xformed, &pts[..3]);
        let expected = quadratic_log2(K_PRECISION, &pts_xformed, &identity());
        let actual = quadratic_log2(K_PRECISION, pts, &VectorXform::from(m));
        reporter_assert!(r, actual == expected);
    }

    let mut rand = Random::default();

    for_random_matrices(&mut rand, |m, rand| {
        check_cubic_log2_with_transform(r, &K_SERP, m);
        check_cubic_log2_with_transform(r, &K_LOOP, m);
        check_quadratic_log2_with_transform(r, &K_QUAD, m);

        for_random_beziers_default(4, rand, |pts, _rand| {
            check_cubic_log2_with_transform(r, pts, m);
        });

        for_random_beziers_default(3, rand, |pts, _rand| {
            check_quadratic_log2_with_transform(r, pts, m);
        });
    });
});

// Port of: tests/WangsFormulaTest.cpp#L332-L361 (chrome/m156)
def_test!(wangs_formula_worst_case_cubic, |r| {
    let id = identity();
    {
        let worst_p = [
            Point::new(0.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
        ];
        reporter_assert!(
            r,
            wangs_formula::worst_case_cubic(K_PRECISION, 100.0, 100.0)
                == wangs_formula_cubic_reference_impl(K_PRECISION, &worst_p)
        );
        reporter_assert!(
            r,
            wangs_formula::worst_case_cubic_log2(K_PRECISION, 100.0, 100.0)
                == cubic_log2(K_PRECISION, &worst_p, &id)
        );
    }
    {
        let worst_p = [
            Point::new(100.0, 100.0),
            Point::new(100.0, 100.0),
            Point::new(200.0, 200.0),
            Point::new(100.0, 100.0),
        ];
        reporter_assert!(
            r,
            wangs_formula::worst_case_cubic(K_PRECISION, 100.0, 100.0)
                == wangs_formula_cubic_reference_impl(K_PRECISION, &worst_p)
        );
        reporter_assert!(
            r,
            wangs_formula::worst_case_cubic_log2(K_PRECISION, 100.0, 100.0)
                == cubic_log2(K_PRECISION, &worst_p, &id)
        );
    }

    fn check_worst_case_cubic(r: &mut Reporter, pts: &[Point]) {
        let mut bbox = Rect::default();
        bbox.set_bounds_no_check(&pts[..4]);
        let worst = wangs_formula::worst_case_cubic(K_PRECISION, bbox.width(), bbox.height());
        let worst_log2 =
            wangs_formula::worst_case_cubic_log2(K_PRECISION, bbox.width(), bbox.height());
        let actual = wangs_formula_cubic_reference_impl(K_PRECISION, pts);
        reporter_assert!(r, worst >= actual);
        reporter_assert!(r, std_max(1.0, worst).log2().ceil() as i32 == worst_log2);
    }

    let mut rand = Random::default();
    for _ in 0..100 {
        for_random_beziers_default(4, &mut rand, |pts, _rand| {
            check_worst_case_cubic(r, pts);
        });
    }

    // Make sure overflow saturates at infinity (not NaN).
    let inf = f32::INFINITY;
    reporter_assert!(
        r,
        wangs_formula::worst_case_cubic_p4(K_PRECISION, inf, inf) == inf
    );
    reporter_assert!(
        r,
        wangs_formula::worst_case_cubic(K_PRECISION, inf, inf) == inf
    );
});

// Ensure Wang's formula for quads produces max error within tolerance.
// Port of: tests/WangsFormulaTest.cpp#L363-L411 (chrome/m156)
def_test!(wangs_formula_quad_within_tol, |r| {
    // Wang's formula and the quad math starts to lose precision with very large coordinate
    // values, so limit the magnitude a bit to prevent test failures due to loss of precision.
    let max_exponent = 15;
    let mut rand = Random::default();
    for_random_beziers(
        3,
        &mut rand,
        |pts, _rand| {
            let nsegs = wangs_formula_quadratic_reference_impl(K_PRECISION, pts).ceil() as i32;

            let tdelta = 1.0 / int_to_float(nsegs);
            for j in 0..nsegs {
                let tmin = int_to_float(j) * tdelta;
                let tmax = int_to_float(j + 1) * tdelta;

                // Get section of quad in [tmin,tmax].
                let section_pts: [Point; 3];
                let mut tmp0 = [Point::new(0.0, 0.0); 5];
                let mut tmp1 = [Point::new(0.0, 0.0); 5];
                if tmin == 0.0 {
                    if tmax == 1.0 {
                        section_pts = [pts[0], pts[1], pts[2]];
                    } else {
                        chop_quad_at(&pts[..3], &mut tmp0, tmax);
                        section_pts = [tmp0[0], tmp0[1], tmp0[2]];
                    }
                } else {
                    chop_quad_at(&pts[..3], &mut tmp0, tmin);
                    if tmax == 1.0 {
                        section_pts = [tmp0[2], tmp0[3], tmp0[4]];
                    } else {
                        chop_quad_at(&tmp0[2..5], &mut tmp1, (tmax - tmin) / (1.0 - tmin));
                        section_pts = [tmp1[0], tmp1[1], tmp1[2]];
                    }
                }

                // For quads, max distance from baseline is always at t=0.5.
                let p = eval_quad_at(&section_pts, 0.5);

                // Get distance of p to baseline.
                let n = Point::new(
                    section_pts[2].y - section_pts[0].y,
                    section_pts[0].x - section_pts[2].x,
                );
                let d = ((p - section_pts[0]).dot(n)).abs() / length(n);

                // Check distance is within specified tolerance.
                reporter_assert!(r, d <= (1.0 / K_PRECISION) + SCALAR_NEARLY_ZERO);
            }
        },
        max_exponent,
    );
});

// Ensure the specialized version for rational quads reduces to regular Wang's formula when all
// weights are equal to one.
// Port of: tests/WangsFormulaTest.cpp#L413-L428 (chrome/m156)
def_test!(wangs_formula_rational_quad_reduces, |r| {
    let k_tessellation_tolerance: scalar = 1.0 / 128.0;

    let mut rand = Random::default();
    for _ in 0..100 {
        for_random_beziers_default(3, &mut rand, |pts, _rand| {
            let rational_nsegs = wangs_formula::conic(K_PRECISION, pts, 1.0, &identity());
            let integral_nsegs = wangs_formula_quadratic_reference_impl(K_PRECISION, pts);
            reporter_assert!(
                r,
                nearly_equal(rational_nsegs, integral_nsegs, k_tessellation_tolerance)
            );
        });
    }
});

// Ensure the rational quad version (used for conics) produces max error within tolerance.
// Port of: tests/WangsFormulaTest.cpp#L430-L489 (chrome/m156)
def_test!(wangs_formula_conic_within_tol, |r| {
    let max_exponent = 24;

    // Single-precision functions in SkConic/SkGeometry lose too much accuracy with
    // large-magnitude curves and large weights for this test to pass.
    let eval_conic = |pts: &[Point], w: scalar, t: scalar| -> Double2 {
        let eval = |a: Double2, b: Double2, c: Double2, t: scalar| -> Double2 {
            (a * f64::from(t) + b) * f64::from(t) + c
        };

        let p0 = Double2::new(f64::from(pts[0].x), f64::from(pts[0].y));
        let p1 = Double2::new(f64::from(pts[1].x), f64::from(pts[1].y));
        let p1w = p1 * f64::from(w);
        let p2 = Double2::new(f64::from(pts[2].x), f64::from(pts[2].y));
        let numer = eval(p2 - p1w * 2.0 + p0, (p1w - p0) * 2.0, p0, t);

        let denom_c = Double2::new(1.0, 1.0);
        // `2 * (w - 1)` is evaluated in float, then widened.
        let denom_b_v = f64::from(2.0_f32 * (w - 1.0));
        let denom_b = Double2::new(denom_b_v, denom_b_v);
        let denom_a_v = f64::from(-2.0_f32 * (w - 1.0));
        let denom_a = Double2::new(denom_a_v, denom_a_v);
        let denom = eval(denom_a, denom_b, denom_c, t);
        numer / denom
    };

    let dot = |a: Double2, b: Double2| -> f64 { a[0] * b[0] + a[1] * b[1] };

    let length = |p: Double2| -> f64 { (p[0] * p[0] + p[1] * p[1]).sqrt() };

    let mut rand = Random::default();
    for i in -10..=10 {
        let w = ldexp(1.0 + rand.next_f(), i);
        for_random_beziers(
            3,
            &mut rand,
            |pts, _rand| {
                let nsegs =
                    scalar_ceil_to_int(wangs_formula::conic(K_PRECISION, pts, w, &identity()));

                let tdelta = 1.0 / int_to_float(nsegs);
                for j in 0..nsegs {
                    let tmin = int_to_float(j) * tdelta;
                    let tmax = int_to_float(j + 1) * tdelta;
                    let tmid = 0.5 * (tmin + tmax);

                    let p0 = eval_conic(pts, w, tmin);
                    let p1 = eval_conic(pts, w, tmid);
                    let p2 = eval_conic(pts, w, tmax);

                    // Get distance of p1 to baseline (p0, p2).
                    let n = Double2::new(p2[1] - p0[1], p0[0] - p2[0]);
                    assert!(length(n) != 0.0);
                    let d = dot(p1 - p0, n).abs() / length(n);

                    // Check distance is within tolerance.
                    reporter_assert!(
                        r,
                        d <= (1.0 / f64::from(K_PRECISION)) + f64::from(SCALAR_NEARLY_ZERO)
                    );
                }
            },
            max_exponent,
        );
    }
});

// Ensure the vectorized conic version equals the reference implementation.
// Port of: tests/WangsFormulaTest.cpp#L491-L506 (chrome/m156)
def_test!(wangs_formula_conic_matches_reference, |r| {
    let mut rand = Random::default();
    for i in -10..=10 {
        let w = ldexp(1.0 + rand.next_f(), i);
        for_random_beziers(
            3,
            &mut rand,
            |pts, _rand| {
                let ref_nsegs = wangs_formula_conic_reference_impl(K_PRECISION, pts, w);
                let nsegs = wangs_formula::conic(K_PRECISION, pts, w, &identity());

                // Because the Gr version may implement the math differently for performance, allow
                // different slack in the comparison based on the rough scale of the answer.
                let cmp_thresh = ref_nsegs * (1.0 / 1_048_576.0);
                reporter_assert!(r, nearly_equal(ref_nsegs, nsegs, cmp_thresh));
            },
            30,
        );
    }
});

// Ensure using transformations gives the same result as pre-transforming all points.
// Port of: tests/WangsFormulaTest.cpp#L509-L535 (chrome/m156)
def_test!(wangs_formula_conic_vectorXforms, |r| {
    fn check_conic_with_transform(r: &mut Reporter, pts: &[Point], w: scalar, m: &Matrix) {
        let mut pts_xformed = [Point::new(0.0, 0.0); 3];
        m.map_points(&mut pts_xformed, &pts[..3]);
        let expected = wangs_formula::conic(K_PRECISION, &pts_xformed, w, &identity());
        let actual = wangs_formula::conic(K_PRECISION, pts, w, &VectorXform::from(m));
        reporter_assert!(r, nearly_equal_default(actual, expected));
    }

    let mut rand = Random::default();
    for i in -10..=10 {
        let w = ldexp(1.0 + rand.next_f(), i);
        for_random_beziers(
            3,
            &mut rand,
            |pts, rand| {
                check_conic_with_transform(r, pts, w, Matrix::i());
                let scale = Matrix::scale((
                    rand.next_range_f(-10.0, 10.0),
                    rand.next_range_f(-10.0, 10.0),
                ));
                check_conic_with_transform(r, pts, w, &scale);

                // Random 2x2 matrix.
                let mut m = Matrix::default();
                m.set_scale_x(rand.next_range_f(-10.0, 10.0));
                m.set_skew_x(rand.next_range_f(-10.0, 10.0));
                m.set_skew_y(rand.next_range_f(-10.0, 10.0));
                m.set_scale_y(rand.next_range_f(-10.0, 10.0));
                check_conic_with_transform(r, pts, w, &m);
            },
            30,
        );
    }
});

// Port of: tests/WangsFormulaTest.cpp#L537-L587 (chrome/m156)
def_test!(wangs_formula_nextlog2, |r| {
    reporter_assert!(
        r,
        0b0_00000000_111_1111111111_1111111111_u32 == (1_u32 << 23) - 1_u32
    );
    reporter_assert!(r, wangs_formula::nextlog2(f32::NEG_INFINITY) == 0);
    reporter_assert!(r, wangs_formula::nextlog2(-f32::MAX) == 0);
    reporter_assert!(r, wangs_formula::nextlog2(-1000.0) == 0);
    reporter_assert!(r, wangs_formula::nextlog2(-0.1) == 0);
    reporter_assert!(r, wangs_formula::nextlog2(-f32::MIN_POSITIVE) == 0);
    reporter_assert!(r, wangs_formula::nextlog2(-f32::from_bits(1)) == 0);
    reporter_assert!(r, wangs_formula::nextlog2(0.0) == 0);
    reporter_assert!(r, wangs_formula::nextlog2(f32::from_bits(1)) == 0);
    reporter_assert!(r, wangs_formula::nextlog2(f32::MIN_POSITIVE) == 0);
    reporter_assert!(r, wangs_formula::nextlog2(0.1) == 0);
    reporter_assert!(r, wangs_formula::nextlog2(1.0) == 0);
    reporter_assert!(r, wangs_formula::nextlog2(1.1) == 1);
    reporter_assert!(r, wangs_formula::nextlog2(2.0) == 1);
    reporter_assert!(r, wangs_formula::nextlog2(2.1) == 2);
    reporter_assert!(r, wangs_formula::nextlog2(3.0) == 2);
    reporter_assert!(r, wangs_formula::nextlog2(3.1) == 2);
    reporter_assert!(r, wangs_formula::nextlog2(4.0) == 2);
    reporter_assert!(r, wangs_formula::nextlog2(4.1) == 3);
    reporter_assert!(r, wangs_formula::nextlog2(5.0) == 3);
    reporter_assert!(r, wangs_formula::nextlog2(5.1) == 3);
    reporter_assert!(r, wangs_formula::nextlog2(6.0) == 3);
    reporter_assert!(r, wangs_formula::nextlog2(6.1) == 3);
    reporter_assert!(r, wangs_formula::nextlog2(7.0) == 3);
    reporter_assert!(r, wangs_formula::nextlog2(7.1) == 3);
    reporter_assert!(r, wangs_formula::nextlog2(8.0) == 3);
    reporter_assert!(r, wangs_formula::nextlog2(8.1) == 4);
    reporter_assert!(r, wangs_formula::nextlog2(9.0) == 4);
    reporter_assert!(r, wangs_formula::nextlog2(9.1) == 4);
    reporter_assert!(r, wangs_formula::nextlog2(f32::MAX) == 128);
    reporter_assert!(r, wangs_formula::nextlog2(f32::INFINITY) == 128);
    reporter_assert!(r, wangs_formula::nextlog2(f32::NAN) == 0);
    reporter_assert!(r, wangs_formula::nextlog2(-f32::NAN) == 0);

    for i in 0..100 {
        let pow2 = ldexp(1.0, i);
        let epsilon = ldexp(SCALAR_NEARLY_ZERO, i);
        reporter_assert!(r, wangs_formula::nextlog2(pow2) == i);
        reporter_assert!(r, wangs_formula::nextlog2(pow2 + epsilon) == i + 1);
        reporter_assert!(r, wangs_formula::nextlog2(pow2 - epsilon) == i);
    }

    reporter_assert!(r, wangs_formula::nextlog64(0.0) == 0);
    reporter_assert!(r, wangs_formula::nextlog64(1.0) == 0);
    reporter_assert!(r, wangs_formula::nextlog64(1.1) == 1);
    reporter_assert!(r, wangs_formula::nextlog64(64.0) == 1);
    reporter_assert!(r, wangs_formula::nextlog64(64.1) == 2);
    reporter_assert!(r, wangs_formula::nextlog64(4096.0) == 2);
    reporter_assert!(r, wangs_formula::nextlog64(4096.1) == 3);
});
