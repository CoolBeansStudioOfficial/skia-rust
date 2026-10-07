// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkQuads.h, src/core/SkQuads.cpp

//! `SkQuads`: utilities for dealing with quadratic formulas with one variable:
//! `f(t) = A*t^2 + B*t + C`.

use crate::floating_point::{double_nearly_zero, doubles_nearly_equal_ulps};

/// The discriminant and the two roots of a quadratic (`SkQuads::RootResult`).
// Port of: src/core/SkQuads.h#L33-L37 (chrome/m156)
#[doc(alias = "SkQuads::RootResult")]
#[derive(Copy, Clone, Debug)]
pub struct RootResult {
    /// `discriminant`.
    pub discriminant: f64,
    /// `root0`.
    pub root0: f64,
    /// `root1`.
    pub root1: f64,
}

// Solve 0 = M * x + B. If M is 0, there are no solutions, unless B is also 0,
// in which case there are infinite solutions, so we just return 1 of them.
// Port of: src/core/SkQuads.cpp#L18-L31 (chrome/m156)
fn solve_linear(m: f64, b: f64, solution: &mut [f64; 2]) -> usize {
    if double_nearly_zero(m) {
        solution[0] = 0.0;
        if double_nearly_zero(b) {
            return 1;
        }
        return 0;
    }
    solution[0] = -b / m;
    if !solution[0].is_finite() {
        return 0;
    }
    1
}

// When B >> A, then the x^2 component doesn't contribute much to the output, so the second root
// will be very large, but have massive round off error. Because of the round off error, the
// second root will not evaluate to zero when substituted back into the quadratic equation. In
// the situation when B >> A, then just treat the quadratic as a linear equation.
// Port of: src/core/SkQuads.cpp#L33-L45 (chrome/m156)
#[allow(clippy::float_cmp)] // exact zero test, as in Skia
fn close_to_linear(a: f64, b: f64) -> bool {
    if a != 0.0 {
        // Return if B is much bigger than A.
        return (b / a).abs() >= 1.0e+16;
    }

    // Otherwise A is zero, and the quadratic is linear.
    true
}

/// Calculate a very accurate discriminant.
/// Given `A*t^2 -2*B*t + C = 0`, calculate `B^2 - AC` accurate to 2 bits.
/// Note the form of the quadratic is slightly different from the normal formulation.
///
/// The method used to calculate the discriminant is from
/// "On the Cost of Floating-Point Computation Without Extra-Precise Arithmetic" by W. Kahan.
// Port of: src/core/SkQuads.cpp#L47-L96 (chrome/m156)
#[doc(alias = "SkQuads::Discriminant")]
#[must_use]
pub fn discriminant(a: f64, b: f64, c: f64) -> f64 {
    let b2 = b * b;
    let ac = a * c;

    // Calculate the rough discriminate which may suffer from a loss in precision due to b2 and
    // ac being too close.
    let rough_discriminant = b2 - ac;

    // If 3 * |B2 - AC| >= AC + B2 holds, then the roughDiscriminant has 2-bits of rounding error
    // or less and can be used. (See the derivation in SkQuads.cpp.)
    if 3.0 * rough_discriminant.abs() >= b2 + ac {
        return rough_discriminant;
    }

    // Use the extra internal precision afforded by fma to calculate the rounding error for
    // b^2 and ac.
    let b2_rounding_error = b.mul_add(b, -b2);
    let ac_rounding_error = a.mul_add(c, -ac);

    // Add the total rounding error back into the discriminant guess.
    (b2 - ac) + (b2_rounding_error - ac_rounding_error)
}

/// Calculate the roots of a quadratic.
/// Given `A*t^2 -2*B*t + C = 0`, calculate the roots.
///
/// This does not try to detect a linear configuration of the equation, or detect if the two
/// roots are the same. It returns the discriminant and the two roots.
///
/// Note this uses a different form of the quadratic equation to reduce rounding error. Given
/// standard A, B, C, call this root finder with `roots(A, -0.5*B, C)` to find the roots of
/// `A*x^2 + B*x + C`.
///
/// If the roots are imaginary then NaN is returned. If the roots can't be represented as double
/// then inf is returned.
// Port of: src/core/SkQuads.cpp#L98-L129 (chrome/m156)
#[doc(alias = "SkQuads::Roots")]
#[must_use]
#[allow(clippy::float_cmp)] // exact comparisons, as in Skia
#[allow(clippy::many_single_char_names)] // names follow the C++
pub fn roots(a: f64, b: f64, c: f64) -> RootResult {
    let discriminant = discriminant(a, b, c);

    if a == 0.0 {
        let root = if b == 0.0 {
            if c == 0.0 { f64::INFINITY } else { f64::NAN }
        } else {
            // Solve -2*B*x + C == 0; x = c/(2*b).
            c / (2.0 * b)
        };
        return RootResult {
            discriminant,
            root0: root,
            root1: root,
        };
    }

    debug_assert!(a != 0.0);
    if discriminant == 0.0 {
        return RootResult {
            discriminant,
            root0: b / a,
            root1: b / a,
        };
    }

    if discriminant > 0.0 {
        let d = discriminant.sqrt();
        let r = if b > 0.0 { b + d } else { b - d };
        return RootResult {
            discriminant,
            root0: r / a,
            root1: c / r,
        };
    }

    // The discriminant is negative or is not finite.
    RootResult {
        discriminant,
        root0: f64::NAN,
        root1: f64::NAN,
    }
}

// Port of: src/core/SkQuads.cpp#L131-L133 (chrome/m156)
fn zero_if_tiny(x: f64) -> f64 {
    if double_nearly_zero(x) { 0.0 } else { x }
}

/// Puts up to 2 real solutions to the equation `A*t^2 + B*t + C = 0` in `solution` and returns
/// how many roots that was.
// Port of: src/core/SkQuads.cpp#L135-L162 (chrome/m156)
#[doc(alias = "SkQuads::RootsReal")]
#[must_use]
#[allow(clippy::float_cmp)] // exact comparisons, as in Skia
pub fn roots_real(a: f64, b: f64, c: f64, solution: &mut [f64; 2]) -> usize {
    if close_to_linear(a, b) {
        return solve_linear(b, c, solution);
    }

    debug_assert!(a != 0.0);
    let RootResult {
        discriminant,
        root0,
        root1,
    } = roots(a, -0.5 * b, c);

    // Handle invariants to mesh with existing code from here on.
    if !discriminant.is_finite() || discriminant < 0.0 {
        return 0;
    }

    let mut count = 0;
    let r0 = zero_if_tiny(root0);
    if r0.is_finite() {
        solution[count] = r0;
        count += 1;
    }
    let r1 = zero_if_tiny(root1);
    if r1.is_finite() {
        solution[count] = r1;
        count += 1;
    }

    if count == 2 && doubles_nearly_equal_ulps(solution[0], solution[1]) {
        count = 1;
    }

    count
}

/// Evaluates the quadratic function with the 3 provided coefficients and the provided variable.
// Port of: src/core/SkQuads.cpp#L164-L167 (chrome/m156)
#[doc(alias = "SkQuads::EvalAt")]
#[must_use]
pub fn eval_at(a: f64, b: f64, c: f64, t: f64) -> f64 {
    // Use fused-multiply-add to reduce the amount of round-off error between terms.
    a.mul_add(t, b).mul_add(t, c)
}
