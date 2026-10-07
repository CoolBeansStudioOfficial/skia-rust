// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathCoverageTest.cpp (chrome/m156)

#![cfg(test)]

// Duplicates lots of code from gpu/src/GrPathUtils.cpp
// It'd be nice not to do so, but that code's set up currently to only have
// a single implementation.

use crate::{Reporter, def_test, errorf};
use skia_rust_core::math_priv::{clz, next_pow2};
use skia_rust_core::point::Point;
use skia_rust_core::point::point_priv::distance_to_line_segment_between;
use skia_rust_core::safe32::abs32;
use skia_rust_core::scalar::{
    int_to_scalar, scalar, scalar_ceil_to_int, scalar_round_to_int, scalar_sqrt,
};

// Sk uses 6, Gr (implicitly) used 10, both apparently arbitrarily.
// Port of: tests/PathCoverageTest.cpp#L26-L28 (chrome/m156)
const MAX_COEFF_SHIFT: i32 = 6;
const MAX_POINTS_PER_CURVE: u32 = 1 << MAX_COEFF_SHIFT;

// max + 0.5 min has error [0.0, 0.12]
// max + 0.375 min has error [-.03, 0.07]
// 0.96043387 max + 0.397824735 min has error [-.06, +.05]
// For determining the maximum possible number of points to use in
// drawing a quadratic, we want to err on the high side.
// Port of: tests/PathCoverageTest.cpp#L30-L44 (chrome/m156)
fn cheap_distance(dx: scalar, dy: scalar) -> i32 {
    let mut idx = abs32(scalar_round_to_int(dx));
    let idy = abs32(scalar_round_to_int(dy));
    if idx > idy {
        idx += idy >> 1;
    } else {
        idx = idy + (idx >> 1);
    }
    idx
}

// Port of: tests/PathCoverageTest.cpp#L46-L49 (chrome/m156)
fn estimate_distance(points: &[Point]) -> i32 {
    cheap_distance(
        points[1].x * 2.0 - points[2].x - points[0].x,
        points[1].y * 2.0 - points[2].y - points[0].y,
    )
}

// Port of: tests/PathCoverageTest.cpp#L51-L53 (chrome/m156)
fn compute_distance(points: &[Point]) -> scalar {
    distance_to_line_segment_between(points[1], points[0], points[2])
}

// Port of: tests/PathCoverageTest.cpp#L55-L64 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // mirrors the implicit int -> uint32_t conversion
fn estimate_point_count(distance: i32) -> u32 {
    // Includes -2 bias because this estimator runs 4x high?
    let mut shift = 30 - clz(distance as u32);
    // Clamp to zero if above subtraction went negative.
    shift &= !(shift >> 31);
    if shift > MAX_COEFF_SHIFT {
        shift = MAX_COEFF_SHIFT;
    }
    1 << shift
}

// Port of: tests/PathCoverageTest.cpp#L66-L74 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // mirrors the std::min<uint32_t> conversion
fn compute_point_count(d: scalar, tol: scalar) -> u32 {
    if d < tol {
        1
    } else {
        let temp = scalar_ceil_to_int(scalar_sqrt(d / tol));
        (next_pow2(temp) as u32).min(MAX_POINTS_PER_CURVE)
    }
}

// Port of: tests/PathCoverageTest.cpp#L76-L79 (chrome/m156)
fn quadratic_point_count_ee(points: &[Point]) -> u32 {
    let distance = estimate_distance(points);
    estimate_point_count(distance)
}

// Port of: tests/PathCoverageTest.cpp#L81-L84 (chrome/m156)
fn quadratic_point_count_ec(points: &[Point], tol: scalar) -> u32 {
    let distance = estimate_distance(points);
    compute_point_count(int_to_scalar(distance), tol)
}

// Port of: tests/PathCoverageTest.cpp#L86-L89 (chrome/m156)
fn quadratic_point_count_ce(points: &[Point]) -> u32 {
    let distance = compute_distance(points);
    estimate_point_count(scalar_round_to_int(distance))
}

// Port of: tests/PathCoverageTest.cpp#L91-L94 (chrome/m156)
fn quadratic_point_count_cc(points: &[Point], tol: scalar) -> u32 {
    let distance = compute_distance(points);
    compute_point_count(distance, tol)
}

// Curve from samplecode/SampleSlides.cpp
// Port of: tests/PathCoverageTest.cpp#L96-L116 (chrome/m156)
const G_XY: [i32; 12] = [4, 0, 0, -4, 8, -4, 12, 0, 8, 4, 0, 4];

const G_SAWTOOTH: [i32; 18] = [
    0, 0, 10, 10, 20, 20, 30, 10, 40, 0, 50, -10, 60, -20, 70, -10, 80, 0,
];

const G_OVALISH: [i32; 10] = [0, 0, 5, 15, 20, 20, 35, 15, 40, 0];

const G_SHARP_SAWTOOTH: [i32; 10] = [0, 0, 1, 10, 2, 0, 3, -10, 4, 0];

// Curve crosses back over itself around 0,10
const G_RIBBON: [i32; 10] = [-4, 0, 4, 20, 0, 25, -4, 20, 4, 0];

// Port of: tests/PathCoverageTest.cpp#L118-L153 (chrome/m156)
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // counts are tiny
fn one_d_pe(array: &[i32], count: u32, reporter: &mut Reporter) -> bool {
    let mut path = [Point::default(); 3];
    path[1] = Point::new(int_to_scalar(array[0]), int_to_scalar(array[1]));
    path[2] = Point::new(int_to_scalar(array[2]), int_to_scalar(array[3]));
    let mut num_errors = 0;
    let mut i = 4u32;
    while i < count {
        path[0] = path[1];
        path[1] = path[2];
        path[2] = Point::new(
            int_to_scalar(array[i as usize]),
            int_to_scalar(array[i as usize + 1]),
        );
        let mut computed_count = quadratic_point_count_cc(&path, int_to_scalar(1));
        let mut estimated_count = quadratic_point_count_ee(&path);

        if false {
            // avoid bit rot, suppress warning
            computed_count = quadratic_point_count_ec(&path, int_to_scalar(1));
            estimated_count = quadratic_point_count_ce(&path);
        }
        // Allow estimated to be high by a factor of two, but no less than
        // the computed value.
        let is_accurate =
            (estimated_count >= computed_count) && (estimated_count <= 2 * computed_count);

        if !is_accurate {
            errorf!(
                reporter,
                "Curve from {:.2} {:.2} through {:.2} {:.2} to {:.2} {:.2} computes {}, estimates {}\n",
                path[0].x,
                path[0].y,
                path[1].x,
                path[1].y,
                path[2].x,
                path[2].y,
                computed_count,
                estimated_count
            );
            num_errors += 1;
        }
        i += 2;
    }

    num_errors == 0
}

// Port of: tests/PathCoverageTest.cpp#L157-L163 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // the arrays are tiny
fn test_quad_point_count(reporter: &mut Reporter) {
    one_d_pe(&G_XY, G_XY.len() as u32, reporter);
    one_d_pe(&G_SAWTOOTH, G_SAWTOOTH.len() as u32, reporter);
    one_d_pe(&G_OVALISH, G_OVALISH.len() as u32, reporter);
    one_d_pe(&G_SHARP_SAWTOOTH, G_SHARP_SAWTOOTH.len() as u32, reporter);
    one_d_pe(&G_RIBBON, G_RIBBON.len() as u32, reporter);
}

// Port of: tests/PathCoverageTest.cpp#L165-L168 (chrome/m156)
def_test!(PathCoverage, |reporter| {
    test_quad_point_count(reporter);
});
