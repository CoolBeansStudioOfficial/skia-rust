// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkCubics.h, src/core/SkCubics.cpp

//! `SkCubics`: utilities for dealing with cubic formulas with one variable:
//! `f(t) = A*t^3 + B*t^2 + C*t + D`.

use crate::floating_point::{
    DOUBLE_PI, double_nearly_zero, doubles_nearly_equal_ulps, ieee_double_divide, is_finite,
    is_finite_all,
};
use crate::quads;
use crate::t_pin::t_pin;

// Port of: src/core/SkCubics.cpp#L19-L24 (chrome/m156)
fn nearly_equal(x: f64, y: f64) -> bool {
    if double_nearly_zero(x) {
        return double_nearly_zero(y);
    }
    doubles_nearly_equal_ulps(x, y)
}

// When the A coefficient of a cubic is close to 0, there can be floating point error
// that arises from computing a very large root. In those cases, we would rather be
// precise about the smaller 2 roots, so we have this arbitrary cutoff for when A is
// really small or small compared to B.
// Port of: src/core/SkCubics.cpp#L26-L35 (chrome/m156)
fn close_to_a_quadratic(a: f64, b: f64) -> bool {
    if double_nearly_zero(b) {
        return double_nearly_zero(a);
    }
    (a / b).abs() < 1.0e-7
}

/// `SkQuads::RootsReal` writing into the first two entries of a three entry solution array, as
/// the C++ passes `solution` (a `double[3]`) straight through.
fn quad_roots_real(a: f64, b: f64, c: f64, solution: &mut [f64; 3]) -> usize {
    let mut s2 = [solution[0], solution[1]];
    let count = quads::roots_real(a, b, c, &mut s2);
    solution[0] = s2[0];
    solution[1] = s2[1];
    count
}

/// Puts up to 3 real solutions to the equation `A*t^3 + B*t^2 + C*t + D = 0` in `solution` and
/// returns how many roots that was.
// Port of: src/core/SkCubics.cpp#L37-L124 (chrome/m156)
#[doc(alias = "SkCubics::RootsReal")]
#[must_use]
#[allow(clippy::many_single_char_names)] // names follow the C++
pub fn roots_real(a: f64, b: f64, c: f64, d: f64, solution: &mut [f64; 3]) -> usize {
    let mut a = a;
    if close_to_a_quadratic(a, b) {
        return quad_roots_real(b, c, d, solution);
    }
    if double_nearly_zero(d) {
        // 0 is one root
        let mut num = quad_roots_real(a, b, c, solution);
        for &root in &solution[..num] {
            if double_nearly_zero(root) {
                return num;
            }
        }
        solution[num] = 0.0;
        num += 1;
        return num;
    }
    if double_nearly_zero(a + b + c + d) {
        // 1 is one root
        let mut num = quad_roots_real(a, a + b, -d, solution);
        for &root in &solution[..num] {
            if doubles_nearly_equal_ulps(root, 1.0) {
                return num;
            }
        }
        solution[num] = 1.0;
        num += 1;
        return num;
    }
    let (a1, b1, c1);
    {
        // If A is zero (e.g. B was nan and thus close_to_a_quadratic was false), we will
        // temporarily have infinities rolling about, but will catch that when checking
        // R2MinusQ3.
        let inv_a = ieee_double_divide(1.0, a);
        a1 = b * inv_a;
        b1 = c * inv_a;
        c1 = d * inv_a;
    }
    let a2 = a1 * a1;
    let q = (a2 - b1 * 3.0) / 9.0;
    let r_ = (2.0 * a2 * a1 - 9.0 * a1 * b1 + 27.0 * c1) / 54.0;
    let r2 = r_ * r_;
    let q3 = q * q * q;
    let r2_minus_q3 = r2 - q3;
    // If one of R2 Q3 is infinite or nan, subtracting them will also be infinite/nan.
    // If both are infinite or nan, the subtraction will be nan.
    // In either case, we have no finite roots.
    if !is_finite(r2_minus_q3) {
        return 0;
    }
    let adiv3 = a1 / 3.0;
    let mut n = 0usize;
    if r2_minus_q3 < 0.0 {
        // we have 3 real roots
        // the divide/root can, due to finite precisions, be slightly outside of -1...1
        // skia-rust: libm (acos, cos, cbrt)
        let theta = t_pin(r_ / q3.sqrt(), -1.0, 1.0).acos();
        let neg2_root_q = -2.0 * q.sqrt();

        let mut r = neg2_root_q * (theta / 3.0).cos() - adiv3;
        solution[n] = r;
        n += 1;

        r = neg2_root_q * ((theta + 2.0 * DOUBLE_PI) / 3.0).cos() - adiv3;
        if !nearly_equal(solution[0], r) {
            solution[n] = r;
            n += 1;
        }
        r = neg2_root_q * ((theta - 2.0 * DOUBLE_PI) / 3.0).cos() - adiv3;
        if !nearly_equal(solution[0], r) && (n == 1 || !nearly_equal(solution[1], r)) {
            solution[n] = r;
            n += 1;
        }
    } else {
        // we have 1 real root
        let sqrt_r2_minus_q3 = r2_minus_q3.sqrt();
        a = r_.abs() + sqrt_r2_minus_q3;
        a = a.cbrt(); // cube root
        if r_ > 0.0 {
            a = -a;
        }
        if !double_nearly_zero(a) {
            a += q / a;
        }
        let mut r = a - adiv3;
        solution[n] = r;
        n += 1;
        if !double_nearly_zero(r2) && doubles_nearly_equal_ulps(r2, q3) {
            r = -a / 2.0 - adiv3;
            if !nearly_equal(solution[0], r) {
                solution[n] = r;
                n += 1;
            }
        }
    }
    n
}

/// Puts up to 3 real solutions to the equation `A*t^3 + B*t^2 + C*t + D = 0` in `solution`, with
/// the constraint that `t` is in the range `[0.0, 1.0]`, and returns how many roots that was.
// Port of: src/core/SkCubics.cpp#L126-L150 (chrome/m156)
#[doc(alias = "SkCubics::RootsValidT")]
#[must_use]
pub fn roots_valid_t(a: f64, b: f64, c: f64, d: f64, solution: &mut [f64; 3]) -> usize {
    let mut all_roots = [0.0f64; 3];
    let real_roots = roots_real(a, b, c, d, &mut all_roots);
    let mut found_roots = 0usize;
    for &t_value in &all_roots[..real_roots] {
        if t_value <= 1.00005 && (t_value >= 1.0 || doubles_nearly_equal_ulps(t_value, 1.0)) {
            // Make sure we do not already have 1 (or something very close) in the list of roots.
            if (found_roots < 1 || !doubles_nearly_equal_ulps(solution[0], 1.0))
                && (found_roots < 2 || !doubles_nearly_equal_ulps(solution[1], 1.0))
            {
                solution[found_roots] = 1.0;
                found_roots += 1;
            }
        } else if t_value >= -0.00005 && (t_value <= 0.0 || double_nearly_zero(t_value)) {
            // Make sure we do not already have 0 (or something very close) in the list of roots.
            if (found_roots < 1 || !double_nearly_zero(solution[0]))
                && (found_roots < 2 || !double_nearly_zero(solution[1]))
            {
                solution[found_roots] = 0.0;
                found_roots += 1;
            }
        } else if t_value > 0.0 && t_value < 1.0 {
            solution[found_roots] = t_value;
            found_roots += 1;
        }
    }
    found_roots
}

// Port of: src/core/SkCubics.cpp#L152-L156 (chrome/m156)
fn approximately_zero(x: f64) -> bool {
    // This cutoff for our binary search hopefully strikes a good balance between
    // performance and accuracy.
    x.abs() < 0.000_000_01
}

// Port of: src/core/SkCubics.cpp#L158-L173 (chrome/m156)
fn find_extrema_valid_t(a: f64, b: f64, c: f64, t: &mut [f64; 2]) -> usize {
    // To find the local min and max of a cubic, we take the derivative and
    // solve when that is equal to 0.
    // d/dt (A*t^3 + B*t^2 + C*t + D) = 3A*t^2 + 2B*t + C
    let mut roots = [0.0f64, 0.0];
    let num_roots = quads::roots_real(3.0 * a, 2.0 * b, c, &mut roots);
    let mut valid_roots = 0usize;
    for &t_value in &roots[..num_roots] {
        if (0.0..=1.0).contains(&t_value) {
            t[valid_roots] = t_value;
            valid_roots += 1;
        }
    }
    valid_roots
}

const MAX_ITERATIONS: i32 = 1000; // prevent infinite loop

// Port of: src/core/SkCubics.cpp#L175-L205 (chrome/m156)
#[allow(clippy::similar_names, clippy::manual_midpoint)] // mirrors the C++ `(a + b) / 2`; names follow the C++
fn binary_search(a: f64, b: f64, c: f64, d: f64, start: f64, stop: f64) -> f64 {
    let mut start = start;
    let mut stop = stop;
    debug_assert!(start <= stop);
    let left = eval_at(a, b, c, d, start);
    if approximately_zero(left) {
        return start;
    }
    let right = eval_at(a, b, c, d, stop);
    if !is_finite_all(left, &[right]) {
        return -1.0; // Not going to deal with one or more endpoints being non-finite.
    }
    if (left > 0.0 && right > 0.0) || (left < 0.0 && right < 0.0) {
        return -1.0; // We can only have a root if one is above 0 and the other is below 0.
    }

    for _ in 0..MAX_ITERATIONS {
        let step = (start + stop) / 2.0;
        let curr = eval_at(a, b, c, d, step);
        if approximately_zero(curr) {
            return step;
        }
        if (curr < 0.0 && left < 0.0) || (curr > 0.0 && left > 0.0) {
            // go right
            start = step;
        } else {
            // go left
            stop = step;
        }
    }
    -1.0
}

/// Puts up to 3 real solutions to the equation `A*t^3 + B*t^2 + C*t + D = 0` in `solution`, with
/// the constraint that `t` is in the range `[0.0, 1.0]`, and returns how many roots that was.
/// This is a slower method than [`roots_valid_t`], but more accurate in circumstances where
/// floating point error gets too big.
// Port of: src/core/SkCubics.cpp#L207-L240 (chrome/m156)
#[doc(alias = "SkCubics::BinarySearchRootsValidT")]
#[must_use]
pub fn binary_search_roots_valid_t(
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    solution: &mut [f64; 3],
) -> usize {
    if !is_finite_all(a, &[b, c, d]) {
        return 0;
    }
    let mut regions = [0.0f64, 0.0, 0.0, 1.0];
    // Find local minima and maxima
    let mut min_max = [0.0f64, 0.0];
    let extrema_count = find_extrema_valid_t(a, b, c, &mut min_max);
    let mut start_index = 2 - extrema_count;
    if extrema_count == 1 {
        regions[start_index + 1] = min_max[0];
    }
    if extrema_count == 2 {
        // While the roots will be in the range 0 to 1 inclusive, they might not be sorted.
        // std::min(a, b) is `(b < a) ? b : a`; std::max(a, b) is `(a < b) ? b : a`.
        let (m0, m1) = (min_max[0], min_max[1]);
        regions[start_index + 1] = if m1 < m0 { m1 } else { m0 };
        regions[start_index + 2] = if m0 < m1 { m1 } else { m0 };
    }
    // Starting at regions[startIndex] and going up through regions[3], we have
    // an ascending list of numbers in the range 0 to 1.0, between which are the possible
    // locations of a root.
    let mut found_roots = 0usize;
    while start_index < 3 {
        let root = binary_search(a, b, c, d, regions[start_index], regions[start_index + 1]);
        if root >= 0.0 {
            // Check for duplicates
            if (found_roots < 1 || !approximately_zero(solution[0] - root))
                && (found_roots < 2 || !approximately_zero(solution[1] - root))
            {
                solution[found_roots] = root;
                found_roots += 1;
            }
        }
        start_index += 1;
    }
    found_roots
}

/// Evaluates the cubic function with the 4 provided coefficients and the provided variable.
// Port of: src/core/SkCubics.h#L52-L54 (chrome/m156)
#[doc(alias = "SkCubics::EvalAt")]
#[must_use]
#[allow(clippy::many_single_char_names)] // names follow the C++
pub fn eval_at(a: f64, b: f64, c: f64, d: f64, t: f64) -> f64 {
    t.mul_add(t.mul_add(t.mul_add(a, b), c), d)
}

/// [`eval_at`] with the coefficients in an array (`SkCubics::EvalAt(double[4], double)`).
// Port of: src/core/SkCubics.h#L56-L58 (chrome/m156)
#[doc(alias = "SkCubics::EvalAt")]
#[must_use]
pub fn eval_at_coefficients(coefficients: &[f64; 4], t: f64) -> f64 {
    eval_at(
        coefficients[0],
        coefficients[1],
        coefficients[2],
        coefficients[3],
        t,
    )
}
