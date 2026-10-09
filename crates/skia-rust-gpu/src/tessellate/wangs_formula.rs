// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/gpu/tessellate/WangsFormula.h (chrome/m156)

//! Wang's formula: the minimum number of evenly spaced (in the parametric sense) line segments a
//! bezier curve must be chopped into so that every line stays within `1/precision` pixels of the
//! true curve.
//!
//! For a bezier of degree `n`:
//!
//! ```text
//!     maxLength = max([length(p[i+2] - 2p[i+1] + p[i]) for (0 <= i <= n-2)])
//!     numParametricSegments = sqrt(maxLength * precision * n*(n - 1)/8)
//! ```
//!
//! (Goldman, Ron. (2003). 5.6.3 Wang's Formula. "Pyramid Algorithms: A Dynamic Programming
//! Approach to Curves and Surfaces for Geometric Modeling". Morgan Kaufmann Publishers.)
//!
//! The lane math runs on `skvx` (`skia_rust_simd::vx`), lane for lane as in the C++. Skia's
//! `std::min`/`std::max` are reproduced by [`cpp_min`]/[`cpp_max`], which differ from
//! `f32::min`/`f32::max` on NaN.

use skia_rust_core::float_bits::float_to_bits;
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_simd::vx::{Float2, Float4, dot, join};

/// `std::max(a, b)`: `(a < b) ? b : a`.
#[inline]
fn cpp_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// `std::min(a, b)`: `(b < a) ? b : a`.
#[inline]
fn cpp_min(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

/// `sqrtf(sqrtf(x))`.
#[inline]
#[must_use]
pub fn root4(x: f32) -> f32 {
    x.sqrt().sqrt()
}

/// The `int` to `float` conversion that the C++ performs implicitly on small constants.
#[inline]
#[allow(clippy::cast_precision_loss)] // mirrors the implicit int -> float conversion in C++
fn int_to_float(x: i32) -> f32 {
    x as f32
}

/// The point as an `skvx::float2`: `sk_bit_cast<skvx::float2>(pt)`.
#[inline]
fn pt2(p: Point) -> Float2 {
    Float2::new(p.x, p.y)
}

/// `length_term<Degree>(precision)`: the factor the length term of Wang's formula is multiplied
/// by.
// Port of: src/gpu/tessellate/WangsFormula.h#L34-L36 (chrome/m156)
#[must_use]
pub fn length_term<const DEGREE: i32>(precision: f32) -> f32 {
    (int_to_float(DEGREE * (DEGREE - 1)) / 8.0) * precision
}

/// `length_term_p2<Degree>(precision)`: the factor applied to the squared length term.
// Port of: src/gpu/tessellate/WangsFormula.h#L37-L39 (chrome/m156)
#[must_use]
pub fn length_term_p2<const DEGREE: i32>(precision: f32) -> f32 {
    (int_to_float(DEGREE * DEGREE * ((DEGREE - 1) * (DEGREE - 1))) / 64.0) * (precision * precision)
}

/// `numeric_limits<float>::digits - 1`.
const DIGITS_AFTER_BINARY_POINT: u32 = 23;

/// Returns `nextlog2(x)`.
///
/// For finite positive `x > 1`, returns `ceil(log2(x))`; otherwise returns 0 (NaN and `-inf`
/// included), and 128 for `+inf`.
// Port of: src/gpu/tessellate/WangsFormula.h#L43-L66 (chrome/m156)
#[must_use]
pub fn nextlog2(x: f32) -> i32 {
    if x <= 1.0 {
        return 0;
    }

    let mut bits = float_to_bits(x);

    // The constant is a significand of all 1s. So, if the significand of x is all 0s (and
    // therefore an integer power of two) this will not increment the exponent, but if it is just
    // one ULP above the power of two the carry will ripple into the exponent incrementing the
    // exponent by 1.
    bits = bits.wrapping_add((1u32 << DIGITS_AFTER_BINARY_POINT) - 1u32);

    // Shift the exponent down, and adjust it by the exponent offset so that 2^0 is really 0
    // instead of 127. Strip off the sign bit.
    #[allow(clippy::cast_possible_wrap)] // the exponent field is 8 bits
    let exp = ((bits >> DIGITS_AFTER_BINARY_POINT) & 0b1111_1111) as i32 - 127;

    // Return 0 for x <= 1.
    if exp > 0 { exp } else { 0 }
}

/// Returns `nextlog2(sqrt(x))`.
// Port of: src/gpu/tessellate/WangsFormula.h#L70-L74 (chrome/m156)
#[must_use]
pub fn nextlog4(x: f32) -> i32 {
    (nextlog2(x) + 1) >> 1
}

/// Returns `nextlog2(sqrt(sqrt(x)))`.
// Port of: src/gpu/tessellate/WangsFormula.h#L76-L80 (chrome/m156)
#[must_use]
pub fn nextlog16(x: f32) -> i32 {
    (nextlog2(x) + 3) >> 2
}

/// Returns `nextlog2(pow(x, 1/6))`.
// Port of: src/gpu/tessellate/WangsFormula.h#L82-L86 (chrome/m156)
#[must_use]
pub fn nextlog64(x: f32) -> i32 {
    (nextlog2(x) + 5) / 6
}

/// Represents the upper-left 2x2 matrix of an affine transform for applying to vectors:
///
/// ```text
///     VectorXform(p1 - p0) == M * float3(p1, 1) - M * float3(p0, 1)
/// ```
// Port of: src/gpu/tessellate/WangsFormula.h#L90-L127 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VectorXform {
    // First and second columns of the 2x2 matrix.
    c0: Float2,
    c1: Float2,
}

impl Default for VectorXform {
    // `VectorXform() : fC0{1.0f, 0.f}, fC1{0.f, 1.f}`
    fn default() -> Self {
        Self {
            c0: Float2::new(1.0, 0.0),
            c1: Float2::new(0.0, 1.0),
        }
    }
}

impl From<&Matrix> for VectorXform {
    // `VectorXform::operator=(const SkMatrix&)`
    fn from(m: &Matrix) -> Self {
        debug_assert!(!m.has_perspective());
        Self {
            c0: Float2::new(m.rc(0, 0), m.rc(1, 0)),
            c1: Float2::new(m.rc(0, 1), m.rc(1, 1)),
        }
    }
}

impl From<&M44> for VectorXform {
    // `VectorXform::operator=(const SkM44&)`
    #[allow(clippy::float_cmp)] // SkASSERT compares the bottom row of the matrix exactly
    fn from(m: &M44) -> Self {
        debug_assert!(
            m.rc(3, 0) == 0.0 && m.rc(3, 1) == 0.0 && m.rc(3, 2) == 0.0 && m.rc(3, 3) == 1.0
        );
        Self {
            c0: Float2::new(m.rc(0, 0), m.rc(1, 0)),
            c1: Float2::new(m.rc(0, 1), m.rc(1, 1)),
        }
    }
}

impl VectorXform {
    /// `VectorXform::operator()(skvx::float2 vector)`.
    #[must_use]
    pub fn apply2(&self, vector: Float2) -> Float2 {
        self.c0 * vector.x() + self.c1 * vector.y()
    }

    /// `VectorXform::operator()(skvx::float4 vectors)`: transforms `(x, y)` and `(z, w)` as two
    /// vectors.
    #[must_use]
    pub fn apply4(&self, vectors: Float4) -> Float4 {
        join(
            self.c0 * vectors.x() + self.c1 * vectors.y(),
            self.c0 * vectors.z() + self.c1 * vectors.w(),
        )
    }
}

/// Returns Wang's formula, raised to the 4th power, specialized for a quadratic curve. The
/// control points are given as `skvx::float2`.
// Port of: src/gpu/tessellate/WangsFormula.h#L131-L140 (chrome/m156)
#[must_use]
pub fn quadratic_p4_vec(
    precision: f32,
    p0: Float2,
    p1: Float2,
    p2: Float2,
    vector_xform: &VectorXform,
) -> f32 {
    let mut v = -2.0_f32 * p1 + p0 + p2;
    v = vector_xform.apply2(v);
    let vv = v * v;
    (vv[0] + vv[1]) * length_term_p2::<2>(precision)
}

/// Returns Wang's formula, raised to the 4th power, specialized for a quadratic curve.
// Port of: src/gpu/tessellate/WangsFormula.h#L141-L149 (chrome/m156)
#[must_use]
pub fn quadratic_p4(precision: f32, pts: &[Point], vector_xform: &VectorXform) -> f32 {
    quadratic_p4_vec(
        precision,
        pt2(pts[0]),
        pt2(pts[1]),
        pt2(pts[2]),
        vector_xform,
    )
}

/// Returns Wang's formula specialized for a quadratic curve.
// Port of: src/gpu/tessellate/WangsFormula.h#L152-L157 (chrome/m156)
#[must_use]
pub fn quadratic(precision: f32, pts: &[Point], vector_xform: &VectorXform) -> f32 {
    root4(quadratic_p4(precision, pts, vector_xform))
}

/// Returns the log2 value of Wang's formula specialized for a quadratic curve, rounded up to the
/// next int.
// Port of: src/gpu/tessellate/WangsFormula.h#L161-L167 (chrome/m156)
#[must_use]
pub fn quadratic_log2(precision: f32, pts: &[Point], vector_xform: &VectorXform) -> i32 {
    // nextlog16(x) == ceil(log2(sqrt(sqrt(x))))
    nextlog16(quadratic_p4(precision, pts, vector_xform))
}

/// Returns Wang's formula, raised to the 4th power, specialized for a cubic curve. The control
/// points are given as `skvx::float2`.
// Port of: src/gpu/tessellate/WangsFormula.h#L170-L182 (chrome/m156)
#[must_use]
pub fn cubic_p4_vec(
    precision: f32,
    p0: Float2,
    p1: Float2,
    p2: Float2,
    p3: Float2,
    vector_xform: &VectorXform,
) -> f32 {
    let p01 = Float4::from_xy_zw(p0, p1);
    let p12 = Float4::from_xy_zw(p1, p2);
    let p23 = Float4::from_xy_zw(p2, p3);
    let mut v = -2.0_f32 * p12 + p01 + p23;
    v = vector_xform.apply4(v);
    let vv = v * v;
    cpp_max(vv[0] + vv[1], vv[2] + vv[3]) * length_term_p2::<3>(precision)
}

/// Returns Wang's formula, raised to the 4th power, specialized for a cubic curve.
// Port of: src/gpu/tessellate/WangsFormula.h#L184-L192 (chrome/m156)
#[must_use]
pub fn cubic_p4(precision: f32, pts: &[Point], vector_xform: &VectorXform) -> f32 {
    cubic_p4_vec(
        precision,
        pt2(pts[0]),
        pt2(pts[1]),
        pt2(pts[2]),
        pt2(pts[3]),
        vector_xform,
    )
}

/// Returns Wang's formula specialized for a cubic curve.
// Port of: src/gpu/tessellate/WangsFormula.h#L195-L200 (chrome/m156)
#[must_use]
pub fn cubic(precision: f32, pts: &[Point], vector_xform: &VectorXform) -> f32 {
    root4(cubic_p4(precision, pts, vector_xform))
}

/// Returns the log2 value of Wang's formula specialized for a cubic curve, rounded up to the next
/// int.
// Port of: src/gpu/tessellate/WangsFormula.h#L203-L208 (chrome/m156)
#[must_use]
pub fn cubic_log2(precision: f32, pts: &[Point], vector_xform: &VectorXform) -> i32 {
    // nextlog16(x) == ceil(log2(sqrt(sqrt(x))))
    nextlog16(cubic_p4(precision, pts, vector_xform))
}

/// Returns the maximum number of line segments a cubic with the given device-space bounding box
/// size would ever need to be divided into, raised to the 4th power. This is simply a special
/// case of the cubic formula where we maximize its value by placing control points on specific
/// corners of the bounding box.
// Port of: src/gpu/tessellate/WangsFormula.h#L213-L217 (chrome/m156)
#[must_use]
pub fn worst_case_cubic_p4(precision: f32, dev_width: f32, dev_height: f32) -> f32 {
    let kk = length_term_p2::<3>(precision);
    (4.0 * kk) * (dev_width * dev_width + dev_height * dev_height)
}

/// Returns the maximum number of line segments a cubic with the given device-space bounding box
/// size would ever need to be divided into.
// Port of: src/gpu/tessellate/WangsFormula.h#L221-L225 (chrome/m156)
#[must_use]
pub fn worst_case_cubic(precision: f32, dev_width: f32, dev_height: f32) -> f32 {
    root4(worst_case_cubic_p4(precision, dev_width, dev_height))
}

/// Returns the maximum log2 number of line segments a cubic with the given device-space bounding
/// box size would ever need to be divided into.
// Port of: src/gpu/tessellate/WangsFormula.h#L228-L233 (chrome/m156)
#[must_use]
pub fn worst_case_cubic_log2(precision: f32, dev_width: f32, dev_height: f32) -> i32 {
    // nextlog16(x) == ceil(log2(sqrt(sqrt(x))))
    nextlog16(worst_case_cubic_p4(precision, dev_width, dev_height))
}

/// Returns Wang's formula, raised to the second power, specialized for a conic curve. The
/// control points are given as `skvx::float2` and should be in projected space.
///
/// This is not actually due to Wang, but is an analogue from (Theorem 3, corollary 1):
///   J. Zheng, T. Sederberg. "Estimating Tessellation Parameter Intervals for Rational Curves and
///   Surfaces." ACM Transactions on Graphics 19(1). 2000.
// Port of: src/gpu/tessellate/WangsFormula.h#L240-L277 (chrome/m156)
#[must_use]
pub fn conic_p2_vec(
    precision: f32,
    p0: Float2,
    p1: Float2,
    p2: Float2,
    w: f32,
    vector_xform: &VectorXform,
) -> f32 {
    let mut p0 = vector_xform.apply2(p0);
    let mut p1 = vector_xform.apply2(p1);
    let mut p2 = vector_xform.apply2(p2);

    // Compute center of bounding box in projected space.
    let c = 0.5_f32 * (p0.min(p1).min(p2) + p0.max(p1).max(p2));

    // Translate by -C. This improves translation-invariance of the formula, see Sec. 3.3 of the
    // cited paper.
    p0 -= c;
    p1 -= c;
    p2 -= c;

    // Compute max length.
    let max_len = cpp_max(dot(p0, p0), cpp_max(dot(p1, p1), dot(p2, p2))).sqrt();

    // Compute forward differences.
    let dp = -2.0_f32 * w * p1 + p0 + p2;
    let dw = (-2.0_f32 * w + 2.0).abs();

    // Compute numerator and denominator for parametric step size of linearization. Here, the
    // epsilon referenced from the cited paper is 1/precision.
    let rp_minus_1 = cpp_max(0.0, max_len * precision - 1.0);
    let numer = dot(dp, dp).sqrt() * precision + rp_minus_1 * dw;
    let denom = 4.0 * cpp_min(w, 1.0);

    // Number of segments = sqrt(numer / denom). This assumes parametric interval of curve being
    // linearized is [t0,t1] = [0, 1]. If not, the number of segments is
    // (tmax - tmin) / sqrt(denom / numer).
    numer / denom
}

/// Returns Wang's formula, raised to the second power, specialized for a conic curve. Input
/// points should be in projected space.
// Port of: src/gpu/tessellate/WangsFormula.h#L278-L287 (chrome/m156)
#[must_use]
pub fn conic_p2(precision: f32, pts: &[Point], w: f32, vector_xform: &VectorXform) -> f32 {
    conic_p2_vec(
        precision,
        pt2(pts[0]),
        pt2(pts[1]),
        pt2(pts[2]),
        w,
        vector_xform,
    )
}

/// Returns the value of Wang's formula specialized for a conic curve.
// Port of: src/gpu/tessellate/WangsFormula.h#L290-L296 (chrome/m156)
#[must_use]
pub fn conic(tolerance: f32, pts: &[Point], w: f32, vector_xform: &VectorXform) -> f32 {
    conic_p2(tolerance, pts, w, vector_xform).sqrt()
}

/// Returns the log2 value of Wang's formula specialized for a conic curve, rounded up to the next
/// int.
// Port of: src/gpu/tessellate/WangsFormula.h#L299-L306 (chrome/m156)
#[must_use]
pub fn conic_log2(tolerance: f32, pts: &[Point], w: f32, vector_xform: &VectorXform) -> i32 {
    // nextlog4(x) == ceil(log2(sqrt(x)))
    nextlog4(conic_p2(tolerance, pts, w, vector_xform))
}
