// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkGeometry.h, src/core/SkGeometry.cpp

//! `SkGeometry`: evaluation, chopping, extrema, inflections and max curvature of quadratic,
//! cubic and conic (rational quadratic) Bézier curves.
//!
//! C++ pointer parameters become slices: `src` and `dst` are slices that must be at least as long
//! as the curve (or the result) needs. Functions that accept a null `dst` in C++ take
//! `Option<&mut [Point]>`. `src` and `dst` can't alias in Rust; callers that relied on aliasing
//! pass a copy of the source.
//!
//! Not ported yet (they need `SkMatrix` and `SkPathDirection`): `SkConic::TransformW` and
//! `SkConic::BuildUnitArc`.

use skia_rust_simd::vx::{self, Float2, Float4};

use crate::bezier_curves::BezierCubic;
use crate::cubics;
use crate::floating_point::{double_to_float, ieee_float_divide, is_finite, is_nan};
use crate::point::{Point, Vector, point_priv};
use crate::scalar::{
    SCALAR_1, SCALAR_PI, Scalar, double_to_scalar, scalar, scalar_abs, scalar_acos, scalar_cos,
    scalar_invert, scalar_pow, scalar_sqrt,
};
use crate::t_pin::t_pin;

mod conic;

pub use conic::{AutoConicToQuads, Conic, ConicCoeff, MAX_CONICS_FOR_ARC};

// Port of: src/core/SkGeometry.h#L22-L24 (chrome/m156)
/// `from_point`: loads a point into a two lane vector.
#[must_use]
pub fn from_point(point: Point) -> Float2 {
    Float2::from_list(&[point.x, point.y])
}

// Port of: src/core/SkGeometry.h#L26-L30 (chrome/m156)
/// `to_point`: stores a two lane vector into a point.
#[must_use]
pub fn to_point(x: Float2) -> Point {
    Point::new(x[0], x[1])
}

// Port of: src/core/SkGeometry.h#L32-L34 (chrome/m156)
/// `times_2`: `value + value`.
#[must_use]
pub fn times_2(value: Float2) -> Float2 {
    value + value
}

// Port of: src/core/SkGeometry.cpp#L35-L39 (chrome/m156)
pub(crate) fn to_vector(x: Float2) -> Vector {
    Vector::new(x[0], x[1])
}

// Port of: src/core/SkGeometry.cpp#L43-L50 (chrome/m156)
fn is_not_monotonic(a: scalar, b: scalar, c: scalar) -> bool {
    let ab = a - b;
    let mut bc = b - c;
    if ab < 0.0 {
        bc = -bc;
    }
    ab == 0.0 || bc < 0.0
}

// Port of: src/core/SkGeometry.cpp#L54-L76 (chrome/m156)
fn valid_unit_divide(numer: scalar, denom: scalar) -> Option<scalar> {
    let (mut numer, mut denom) = (numer, denom);
    if numer < 0.0 {
        numer = -numer;
        denom = -denom;
    }

    if denom == 0.0 || numer == 0.0 || numer >= denom {
        return None;
    }

    let r = numer / denom;
    if is_nan(r) {
        return None;
    }
    debug_assert!(
        (0.0..SCALAR_1).contains(&r),
        "numer {numer}, denom {denom}, r {r}"
    );
    if r == 0.0 {
        // catch underflow if numer <<<< denom
        return None;
    }
    Some(r)
}

/// Given a quadratic equation `Ax^2 + Bx + C = 0`, return 0, 1, 2 roots for the equation.
///
/// From Numerical Recipes in C: `Q = -1/2 (B + sign(B) sqrt[B*B - 4*A*C])`, `x1 = Q / A`,
/// `x2 = C / Q`.
// Port of: src/core/SkGeometry.cpp#L95-L127 (chrome/m156)
#[doc(alias = "SkFindUnitQuadRoots")]
#[must_use]
#[allow(clippy::many_single_char_names, clippy::float_cmp)] // exact float comparisons, as in Skia; names follow the C++
pub fn find_unit_quad_roots(a: scalar, b: scalar, c: scalar, roots: &mut [scalar; 2]) -> usize {
    if a == 0.0 {
        return match valid_unit_divide(-c, b) {
            Some(r) => {
                roots[0] = r;
                1
            }
            None => 0,
        };
    }

    let mut r = 0usize;

    // use doubles so we don't overflow temporarily trying to compute R
    let mut dr = f64::from(b) * f64::from(b) - 4.0 * f64::from(a) * f64::from(c);
    if dr < 0.0 {
        return 0;
    }
    dr = dr.sqrt();
    let big_r = double_to_scalar(dr);
    if !is_finite(big_r) {
        return 0;
    }

    let q = if b < 0.0 {
        -(b - big_r) / 2.0
    } else {
        -(b + big_r) / 2.0
    };
    if let Some(v) = valid_unit_divide(q, a) {
        roots[r] = v;
        r += 1;
    }
    if let Some(v) = valid_unit_divide(c, q) {
        roots[r] = v;
        r += 1;
    }
    if r == 2 {
        if roots[0] > roots[1] {
            roots.swap(0, 1);
        } else if roots[0] == roots[1] {
            // nearly-equal?
            r -= 1; // skip the double root
        }
    }
    r
}

// Port of: src/core/SkGeometry.h#L143-L146 (chrome/m156)
/// Use for: `eval(t) == A * t^2 + B * t + C`.
#[doc(alias = "SkQuadCoeff")]
#[derive(Copy, Clone, Debug)]
pub struct QuadCoeff {
    /// `fA`.
    pub a: Float2,
    /// `fB`.
    pub b: Float2,
    /// `fC`.
    pub c: Float2,
}

impl QuadCoeff {
    /// `SkQuadCoeff(A, B, C)`.
    #[must_use]
    pub fn new(a: Float2, b: Float2, c: Float2) -> Self {
        Self { a, b, c }
    }

    /// `SkQuadCoeff(const SkPoint src[3])`.
    // Port of: src/core/SkGeometry.h#L408-L414 (chrome/m156)
    #[must_use]
    pub fn from_points(src: &[Point]) -> Self {
        let c = from_point(src[0]);
        let p1 = from_point(src[1]);
        let p2 = from_point(src[2]);
        let b = times_2(p1 - c);
        let a = p2 - times_2(p1) + c;
        Self { a, b, c }
    }

    /// `eval(tt)`.
    // Port of: src/core/SkGeometry.h#L416-L418 (chrome/m156)
    #[must_use]
    pub fn eval(&self, tt: Float2) -> Float2 {
        (self.a * tt + self.b) * tt + self.c
    }
}

// Port of: src/core/SkGeometry.h#L455-L478 (chrome/m156)
/// Power basis coefficients of a cubic: `eval(t) == ((A * t + B) * t + C) * t + D`.
#[doc(alias = "SkCubicCoeff")]
#[derive(Copy, Clone, Debug)]
pub struct CubicCoeff {
    /// `fA`.
    pub a: Float2,
    /// `fB`.
    pub b: Float2,
    /// `fC`.
    pub c: Float2,
    /// `fD`.
    pub d: Float2,
}

impl CubicCoeff {
    /// `SkCubicCoeff(const SkPoint src[4])`.
    #[must_use]
    pub fn new(src: &[Point]) -> Self {
        let p0 = from_point(src[0]);
        let p1 = from_point(src[1]);
        let p2 = from_point(src[2]);
        let p3 = from_point(src[3]);
        let three = Float2::splat(3.0);
        Self {
            a: p3 + three * (p1 - p2) - p0,
            b: three * (p2 - times_2(p1) + p0),
            c: three * (p1 - p0),
            d: p0,
        }
    }

    /// `eval(t)`.
    #[must_use]
    pub fn eval(&self, t: Float2) -> Float2 {
        ((self.a * t + self.b) * t + self.c) * t + self.d
    }
}

/// Measures the angle between two vectors, in the range `[0, pi]`.
// Port of: src/core/SkGeometry.cpp#L197-L202 (chrome/m156)
#[doc(alias = "SkMeasureAngleBetweenVectors")]
#[must_use]
pub fn measure_angle_between_vectors(a: Vector, b: Vector) -> f32 {
    let mut cos_theta = ieee_float_divide(a.dot(b), (a.dot(a) * b.dot(b)).sqrt());
    // Pin cosTheta such that if it is NaN (e.g., if a or b was 0), then we return acos(1) = 0.
    // std::min(1, x) is `(x < 1) ? x : 1`; std::max(x, -1) is `(x < -1) ? -1 : x`.
    cos_theta = if cos_theta < 1.0 { cos_theta } else { 1.0 };
    cos_theta = if cos_theta < -1.0 { -1.0 } else { cos_theta };
    scalar_acos(cos_theta)
}

/// Returns a new, arbitrarily scaled vector that bisects the given vectors. The returned
/// bisector will always point toward the interior of the provided vectors.
// Port of: src/core/SkGeometry.cpp#L204-L229 (chrome/m156)
#[doc(alias = "SkFindBisector")]
#[must_use]
pub fn find_bisector(a: Vector, b: Vector) -> Vector {
    let mut v = [Vector::default(); 2];
    if a.dot(b) >= 0.0 {
        // a,b are within +/-90 degrees apart.
        v = [a, b];
    } else if a.cross(b) >= 0.0 {
        // a,b are >90 degrees apart. Find the bisector of their interior normals instead. (Above 90
        // degrees, the original vectors start cancelling each other out which eventually becomes
        // unstable.)
        v[0].set(-a.y, a.x);
        v[1].set(b.y, -b.x);
    } else {
        // a,b are <-90 degrees apart. Find the bisector of their interior normals instead. (Below
        // -90 degrees, the original vectors start cancelling each other out which eventually
        // becomes unstable.)
        v[0].set(a.y, -a.x);
        v[1].set(-b.y, b.x);
    }
    // Return "normalize(v[0]) + normalize(v[1])".
    let mut x0_x1 = Float2::from_list(&[v[0].x, v[1].x]);
    let mut y0_y1 = Float2::from_list(&[v[0].y, v[1].y]);
    let inv_lengths = 1.0f32 / vx::sqrt(x0_x1 * x0_x1 + y0_y1 * y0_y1);
    x0_x1 *= inv_lengths;
    y0_y1 *= inv_lengths;
    Point::new(x0_x1[0] + x0_x1[1], y0_y1[0] + y0_y1[1])
}

// ---------------------------------------------------------------------------------------------
// Quads
// ---------------------------------------------------------------------------------------------

/// Evaluates the quad `src` at `t` (`SkEvalQuadAt(src, t)`).
// Port of: src/core/SkGeometry.cpp#L144-L146 (chrome/m156)
#[doc(alias = "SkEvalQuadAt")]
#[must_use]
pub fn eval_quad_at(src: &[Point], t: scalar) -> Point {
    to_point(QuadCoeff::from_points(src).eval(Float2::splat(t)))
}

/// The tangent of the quad `src` at `t` (`SkEvalQuadTangentAt`).
// Port of: src/core/SkGeometry.cpp#L148-L167 (chrome/m156)
#[doc(alias = "SkEvalQuadTangentAt")]
#[must_use]
#[allow(clippy::float_cmp)] // exact float comparisons, as in Skia
pub fn eval_quad_tangent_at(src: &[Point], t: scalar) -> Vector {
    // The derivative equation is 2(b - a +(a - 2b +c)t). This returns a
    // zero tangent vector when t is 0 or 1, and the control point is equal
    // to the end point. In this case, use the quad end points to compute the tangent.
    if (t == 0.0 && src[0] == src[1]) || (t == 1.0 && src[1] == src[2]) {
        return src[2] - src[0];
    }
    debug_assert!((0.0..=SCALAR_1).contains(&t));

    let p0 = from_point(src[0]);
    let p1 = from_point(src[1]);
    let p2 = from_point(src[2]);

    let b = p1 - p0;
    let a = p2 - p1 - b;
    let tangent = a * t + b;

    to_vector(tangent + tangent)
}

/// Sets `pt` to the point on the src quadratic specified by `t`, and `tangent` to its tangent
/// (`SkEvalQuadAt(src, t, &pt, &tangent)`). `t` must be `0 <= t <= 1.0`.
// Port of: src/core/SkGeometry.cpp#L132-L142 (chrome/m156)
#[doc(alias = "SkEvalQuadAt")]
pub fn eval_quad_at_pos_tangent(
    src: &[Point],
    t: scalar,
    pt: Option<&mut Point>,
    tangent: Option<&mut Vector>,
) {
    debug_assert!((0.0..=SCALAR_1).contains(&t));

    if let Some(pt) = pt {
        *pt = eval_quad_at(src, t);
    }
    if let Some(tangent) = tangent {
        *tangent = eval_quad_tangent_at(src, t);
    }
}

// Port of: src/core/SkGeometry.cpp#L169-L173 (chrome/m156)
fn interp(v0: Float2, v1: Float2, t: Float2) -> Float2 {
    v0 + (v1 - v0) * t
}

/// Given a src quadratic bezier, chop it at the specified `t` value, where `0 < t < 1`, and
/// return the two new quadratics in `dst`: `dst[0..3]` and `dst[2..5]`.
// Port of: src/core/SkGeometry.cpp#L175-L191 (chrome/m156)
#[doc(alias = "SkChopQuadAt")]
pub fn chop_quad_at(src: &[Point], dst: &mut [Point], t: scalar) {
    debug_assert!(t > 0.0 && t < SCALAR_1);

    let p0 = from_point(src[0]);
    let p1 = from_point(src[1]);
    let p2 = from_point(src[2]);
    let tt = Float2::splat(t);

    let p01 = interp(p0, p1, tt);
    let p12 = interp(p1, p2, tt);

    dst[0] = to_point(p0);
    dst[1] = to_point(p01);
    dst[2] = to_point(interp(p01, p12, tt));
    dst[3] = to_point(p12);
    dst[4] = to_point(p2);
}

/// Given a src quadratic bezier, chop it at `t == 1/2`. The new quads are returned in
/// `dst[0..3]` and `dst[2..5]`.
// Port of: src/core/SkGeometry.cpp#L193-L195 (chrome/m156)
#[doc(alias = "SkChopQuadAtHalf")]
pub fn chop_quad_at_half(src: &[Point], dst: &mut [Point]) {
    chop_quad_at(src, dst, 0.5);
}

/// Measures the rotation of the given quadratic curve in radians.
///
/// Rotation is perhaps easiest described via a driving analogy: If you drive your car along the
/// curve from p0 to p2, then by the time you arrive at p2, how many radians will your car have
/// rotated? For a quadratic this is the same as the vector inside the tangents at the endpoints.
///
/// Quadratics can have rotations in the range `[0, pi]`.
// Port of: src/core/SkGeometry.h#L89-L91 (chrome/m156)
#[doc(alias = "SkMeasureQuadRotation")]
#[must_use]
pub fn measure_quad_rotation(pts: &[Point]) -> f32 {
    measure_angle_between_vectors(pts[1] - pts[0], pts[2] - pts[1])
}

/// Given a src quadratic bezier, returns the T value whose tangent angle is halfway between the
/// tangents at p0 and p3.
// Port of: src/core/SkGeometry.cpp#L231-L258 (chrome/m156)
#[doc(alias = "SkFindQuadMidTangent")]
#[must_use]
pub fn find_quad_mid_tangent(src: &[Point]) -> f32 {
    // Tangents point in the direction of increasing T, so tan0 and -tan1 both point toward the
    // midtangent. The bisector of tan0 and -tan1 is orthogonal to the midtangent:
    //
    //     n dot midtangent = 0
    //
    let tan0 = src[1] - src[0];
    let tan1 = src[2] - src[1];
    let bisector = find_bisector(tan0, -tan1);

    // The midtangent can be found where (F' dot bisector) = 0:
    //
    //   T = (tan0 dot bisector) / ((tan0 - tan1) dot bisector)
    let mut t = ieee_float_divide(tan0.dot(bisector), (tan0 - tan1).dot(bisector));
    if !(t > 0.0 && t < 1.0) {
        // Use "!(positive_logic)" so T=nan will take this branch.
        t = 0.5; // The quadratic was a line or near-line. Just chop at .5.
    }

    t
}

/// Given a src quadratic bezier, chop it at the tangent whose angle is halfway between the
/// tangents at p0 and p2. The new quads are returned in `dst[0..3]` and `dst[2..5]`.
// Port of: src/core/SkGeometry.h#L111-L113 (chrome/m156)
#[doc(alias = "SkChopQuadAtMidTangent")]
pub fn chop_quad_at_mid_tangent(src: &[Point], dst: &mut [Point]) {
    chop_quad_at(src, dst, find_quad_mid_tangent(src));
}

/// Given the 3 coefficients for a quadratic bezier (either X or Y values), look for extrema, and
/// return the number of t-values that are found that represent these extrema. If the quadratic
/// has no extrema between (0..1) exclusive, the function returns 0.
///
/// `Quad'(t) = At + B`, where `A = 2(a - 2b + c)`, `B = 2(b - a)`. Solve for t, only if it fits
/// between `0 < t < 1`.
// Port of: src/core/SkGeometry.cpp#L265-L270 (chrome/m156)
#[doc(alias = "SkFindQuadExtrema")]
#[must_use]
pub fn find_quad_extrema(a: scalar, b: scalar, c: scalar, t_value: &mut [scalar; 1]) -> usize {
    // At + B == 0
    // t = -B / A
    match valid_unit_divide(a - b, a - b - b + c) {
        Some(r) => {
            t_value[0] = r;
            1
        }
        None => 0,
    }
}

// Port of: src/core/SkGeometry.cpp#L285-L287 (chrome/m156)
fn flatten_double_quad_extrema_y(dst: &mut [Point]) {
    // coords[2] = coords[6] = coords[4] (starting at dst[0].fY)
    dst[3].y = dst[2].y;
    dst[1].y = dst[2].y;
}

fn flatten_double_quad_extrema_x(dst: &mut [Point]) {
    dst[3].x = dst[2].x;
    dst[1].x = dst[2].x;
}

/// Given 3 points on a quadratic bezier, chop it into 1, 2 beziers such that the resulting
/// beziers are monotonic in Y. This is called by the scan converter.
///
/// Returns 0 for 1 quad (`dst[0..3]` is the original quad) and 1 for two quads (`dst[0..3]` and
/// `dst[2..5]` are the two new quads). Either way the answer is stored in `dst`.
// Port of: src/core/SkGeometry.cpp#L279-L302 (chrome/m156)
#[doc(alias = "SkChopQuadAtYExtrema")]
pub fn chop_quad_at_y_extrema(src: &[Point], dst: &mut [Point]) -> usize {
    let a = src[0].y;
    let mut b = src[1].y;
    let c = src[2].y;

    if is_not_monotonic(a, b, c) {
        if let Some(t_value) = valid_unit_divide(a - b, a - b - b + c) {
            chop_quad_at(src, dst, t_value);
            flatten_double_quad_extrema_y(dst);
            return 1;
        }
        // if we get here, we need to force dst to be monotonic, even though
        // we couldn't compute a unit_divide value (probably underflow).
        b = if scalar_abs(a - b) < scalar_abs(b - c) {
            a
        } else {
            c
        };
    }
    dst[0].set(src[0].x, a);
    dst[1].set(src[1].x, b);
    dst[2].set(src[2].x, c);
    0
}

/// Like [`chop_quad_at_y_extrema`], for X.
// Port of: src/core/SkGeometry.cpp#L307-L330 (chrome/m156)
#[doc(alias = "SkChopQuadAtXExtrema")]
pub fn chop_quad_at_x_extrema(src: &[Point], dst: &mut [Point]) -> usize {
    let a = src[0].x;
    let mut b = src[1].x;
    let c = src[2].x;

    if is_not_monotonic(a, b, c) {
        if let Some(t_value) = valid_unit_divide(a - b, a - b - b + c) {
            chop_quad_at(src, dst, t_value);
            flatten_double_quad_extrema_x(dst);
            return 1;
        }
        // if we get here, we need to force dst to be monotonic, even though
        // we couldn't compute a unit_divide value (probably underflow).
        b = if scalar_abs(a - b) < scalar_abs(b - c) {
            a
        } else {
            c
        };
    }
    dst[0].set(a, src[0].y);
    dst[1].set(b, src[1].y);
    dst[2].set(c, src[2].y);
    0
}

/// Given 3 points on a quadratic bezier, if the point of maximum curvature exists on the
/// segment, returns the t value for this point along the curve. Otherwise it will return a
/// value of 0.
///
/// ```text
/// F(t)    = a (1 - t) ^ 2 + 2 b t (1 - t) + c t ^ 2
/// F'(t)   = 2 (b - a) + 2 (a - 2b + c) t
/// F''(t)  = 2 (a - 2b + c)
/// t = - (Ax Bx + Ay By) / (Bx ^ 2 + By ^ 2)
/// ```
// Port of: src/core/SkGeometry.cpp#L344-L365 (chrome/m156)
#[doc(alias = "SkFindQuadMaxCurvature")]
#[must_use]
pub fn find_quad_max_curvature(src: &[Point]) -> scalar {
    let ax = src[1].x - src[0].x;
    let ay = src[1].y - src[0].y;
    let bx = src[0].x - src[1].x - src[1].x + src[2].x;
    let by = src[0].y - src[1].y - src[1].y + src[2].y;

    let mut numer = -(ax * bx + ay * by);
    let mut denom = bx * bx + by * by;
    if denom < 0.0 {
        numer = -numer;
        denom = -denom;
    }
    if numer <= 0.0 {
        return 0.0;
    }
    if numer >= denom {
        // Also catches denom=0.
        return 1.0;
    }
    let t = numer / denom;
    debug_assert!((0.0..1.0).contains(&t) || is_nan(t));
    t
}

/// Given 3 points on a quadratic bezier, divide it into 2 quadratics if the point of maximum
/// curvature exists on the quad segment. Returns 1 if `dst[0..3]` is the original quad, 2 if
/// `dst[0..3]` and `dst[2..5]` are the two new quads.
// Port of: src/core/SkGeometry.cpp#L367-L376 (chrome/m156)
#[doc(alias = "SkChopQuadAtMaxCurvature")]
pub fn chop_quad_at_max_curvature(src: &[Point], dst: &mut [Point]) -> usize {
    let t = find_quad_max_curvature(src);
    if t > 0.0 && t < 1.0 {
        chop_quad_at(src, dst, t);
        2
    } else {
        dst[..3].copy_from_slice(&src[..3]);
        1
    }
}

/// Given 3 points on a quadratic bezier, use degree elevation to convert it into the cubic
/// fitting the same curve. The new cubic curve is returned in `dst[0..4]`.
// Port of: src/core/SkGeometry.cpp#L378-L388 (chrome/m156)
#[doc(alias = "SkConvertQuadToCubic")]
pub fn convert_quad_to_cubic(src: &[Point], dst: &mut [Point]) {
    let scale = Float2::splat(double_to_scalar(2.0 / 3.0));
    let s0 = from_point(src[0]);
    let s1 = from_point(src[1]);
    let s2 = from_point(src[2]);

    dst[0] = to_point(s0);
    dst[1] = to_point(s0 + (s1 - s0) * scale);
    dst[2] = to_point(s2 + (s1 - s2) * scale);
    dst[3] = to_point(s2);
}

// ---------------------------------------------------------------------------------------------
// Cubics
// ---------------------------------------------------------------------------------------------

// Port of: src/core/SkGeometry.cpp#L394-L405 (chrome/m156)
fn eval_cubic_derivative(src: &[Point], t: scalar) -> Vector {
    let p0 = from_point(src[0]);
    let p1 = from_point(src[1]);
    let p2 = from_point(src[2]);
    let p3 = from_point(src[3]);

    let coeff = QuadCoeff {
        a: p3 + 3.0f32 * (p1 - p2) - p0,
        b: times_2(p2 - times_2(p1) + p0),
        c: p1 - p0,
    };
    to_vector(coeff.eval(Float2::splat(t)))
}

// Port of: src/core/SkGeometry.cpp#L407-L416 (chrome/m156)
fn eval_cubic_2nd_derivative(src: &[Point], t: scalar) -> Vector {
    let p0 = from_point(src[0]);
    let p1 = from_point(src[1]);
    let p2 = from_point(src[2]);
    let p3 = from_point(src[3]);
    let a = p3 + 3.0f32 * (p1 - p2) - p0;
    let b = p2 - times_2(p1) + p0;

    to_vector(a * t + b)
}

/// Sets `loc` to the point on the src cubic specified by `t`, `tangent` to its tangent and
/// `curvature` to its second derivative. `t` must be `0 <= t <= 1.0`.
// Port of: src/core/SkGeometry.cpp#L418-L446 (chrome/m156)
#[doc(alias = "SkEvalCubicAt")]
#[allow(clippy::float_cmp)] // exact float comparisons, as in Skia
pub fn eval_cubic_at(
    src: &[Point],
    t: scalar,
    loc: Option<&mut Point>,
    tangent: Option<&mut Vector>,
    curvature: Option<&mut Vector>,
) {
    debug_assert!((0.0..=SCALAR_1).contains(&t));

    if let Some(loc) = loc {
        *loc = to_point(CubicCoeff::new(src).eval(Float2::splat(t)));
    }
    if let Some(tangent) = tangent {
        // The derivative equation returns a zero tangent vector when t is 0 or 1, and the
        // adjacent control point is equal to the end point. In this case, use the
        // next control point or the end points to compute the tangent.
        if (t == 0.0 && src[0] == src[1]) || (t == 1.0 && src[2] == src[3]) {
            if t == 0.0 {
                *tangent = src[2] - src[0];
            } else {
                *tangent = src[3] - src[1];
            }
            if tangent.x == 0.0 && tangent.y == 0.0 {
                *tangent = src[3] - src[0];
            }
        } else {
            *tangent = eval_cubic_derivative(src, t);
        }
    }
    if let Some(curvature) = curvature {
        *curvature = eval_cubic_2nd_derivative(src, t);
    }
}

/// Given the 4 coefficients for a cubic bezier (either X or Y values), look for extrema, and
/// return the number of t-values that are found that represent these extrema. If the cubic has
/// no extrema between (0..1) exclusive, the function returns 0.
///
/// `Cubic'(t) = At^2 + Bt + C`, where `A = 3(-a + 3(b - c) + d)`, `B = 6(a - 2b + c)`,
/// `C = 3(b - a)`. Solve for t, keeping only those that fit between `0 < t < 1`.
// Port of: src/core/SkGeometry.cpp#L454-L462 (chrome/m156)
#[doc(alias = "SkFindCubicExtrema")]
#[must_use]
pub fn find_cubic_extrema(
    a: scalar,
    b: scalar,
    c: scalar,
    d: scalar,
    t_values: &mut [scalar; 2],
) -> usize {
    // we divide A,B,C by 3 to simplify
    let big_a = d - a + 3.0 * (b - c);
    let big_b = 2.0 * (a - b - b + c);
    let big_c = b - a;

    find_unit_quad_roots(big_a, big_b, big_c, t_values)
}

// This does not return b when t==1, but it otherwise seems to get better precision than
// "a*(1 - t) + b*t" for things like chopping cubics on exact cusp points.
// The responsibility falls on the caller to check that t != 1 before calling.
// Port of: src/core/SkGeometry.cpp#L468-L471 (chrome/m156)
fn unchecked_mix<const N: usize>(
    a: vx::Vec<N, f32>,
    b: vx::Vec<N, f32>,
    t: vx::Vec<N, f32>,
) -> vx::Vec<N, f32> {
    (b - a) * t + a
}

/// Given a src cubic bezier, chop it at the specified `t` value, where `0 <= t <= 1`, and return
/// the two new cubics in `dst`: `dst[0..4]` and `dst[3..7]`.
// Port of: src/core/SkGeometry.cpp#L473-L502 (chrome/m156)
#[doc(alias = "SkChopCubicAt")]
#[allow(clippy::float_cmp)] // exact float comparisons, as in Skia
pub fn chop_cubic_at(src: &[Point], dst: &mut [Point], t: scalar) {
    debug_assert!((0.0..=1.0).contains(&t));

    if t == 1.0 {
        dst[..4].copy_from_slice(&src[..4]);
        dst[4] = src[3];
        dst[5] = src[3];
        dst[6] = src[3];
        return;
    }

    let p0 = from_point(src[0]);
    let p1 = from_point(src[1]);
    let p2 = from_point(src[2]);
    let p3 = from_point(src[3]);
    let big_t = Float2::splat(t);

    let ab = unchecked_mix(p0, p1, big_t);
    let bc = unchecked_mix(p1, p2, big_t);
    let cd = unchecked_mix(p2, p3, big_t);
    let abc = unchecked_mix(ab, bc, big_t);
    let bcd = unchecked_mix(bc, cd, big_t);
    let abcd = unchecked_mix(abc, bcd, big_t);

    dst[0] = to_point(p0);
    dst[1] = to_point(ab);
    dst[2] = to_point(abc);
    dst[3] = to_point(abcd);
    dst[4] = to_point(bcd);
    dst[5] = to_point(cd);
    dst[6] = to_point(p3);
}

/// Given a src cubic bezier, chop it at the specified `t0` and `t1` values, where
/// `0 <= t0 <= t1 <= 1`, and return the three new cubics in `dst`: `dst[0..4]`, `dst[3..7]` and
/// `dst[6..10]`.
// Port of: src/core/SkGeometry.cpp#L504-L539 (chrome/m156)
#[doc(alias = "SkChopCubicAt")]
#[allow(clippy::float_cmp)] // exact float comparisons, as in Skia
pub fn chop_cubic_at_t0_t1(src: &[Point], dst: &mut [Point], t0: f32, t1: f32) {
    debug_assert!(0.0 <= t0 && t0 <= t1 && t1 <= 1.0);

    if t1 == 1.0 {
        chop_cubic_at(src, dst, t0);
        dst[7] = src[3];
        dst[8] = src[3];
        dst[9] = src[3];
        return;
    }

    // Perform both chops in parallel using 4-lane SIMD.
    let dup = |p: Point| Float4::from_list(&[p.x, p.y, p.x, p.y]);
    let p00 = dup(src[0]);
    let p11 = dup(src[1]);
    let p22 = dup(src[2]);
    let p33 = dup(src[3]);
    let big_t = Float4::from_list(&[t0, t0, t1, t1]);

    let ab = unchecked_mix(p00, p11, big_t);
    let bc = unchecked_mix(p11, p22, big_t);
    let cd = unchecked_mix(p22, p33, big_t);
    let abc = unchecked_mix(ab, bc, big_t);
    let bcd = unchecked_mix(bc, cd, big_t);
    let abcd = unchecked_mix(abc, bcd, big_t);
    let middle = unchecked_mix(abc, bcd, vx::shuffle::<4, 4, f32>(big_t, [2, 3, 0, 1]));

    let lo = |v: Float4| Point::new(v[0], v[1]);
    let hi = |v: Float4| Point::new(v[2], v[3]);
    dst[0] = lo(p00);
    dst[1] = lo(ab);
    dst[2] = lo(abc);
    dst[3] = lo(abcd);
    dst[4] = lo(middle);
    dst[5] = hi(middle);
    dst[6] = hi(abcd);
    dst[7] = hi(bcd);
    dst[8] = hi(cd);
    dst[9] = hi(p33);
}

/// Given a src cubic bezier, chop it at the specified `t` values, where
/// `0 <= t0 <= t1 <= ... <= 1`, and return the new cubics in `dst`:
/// `dst[0..4], dst[3..7], ..., dst[3*t_count..3*(t_count+1)+1]`. `t_values.len()` is `tCount`.
// Port of: src/core/SkGeometry.cpp#L541-L573 (chrome/m156)
#[doc(alias = "SkChopCubicAt")]
pub fn chop_cubic_at_ts(src: &[Point], dst: Option<&mut [Point]>, t_values: &[scalar]) {
    debug_assert!(t_values.iter().all(|&t| (0.0..=1.0).contains(&t)));
    debug_assert!(t_values.windows(2).all(|w| w[0] <= w[1]));
    let t_count = t_values.len();

    if let Some(dst) = dst {
        if t_count == 0 {
            // nothing to chop
            dst[..4].copy_from_slice(&src[..4]);
        } else {
            let mut cur_src = [src[0], src[1], src[2], src[3]];
            let mut off = 0usize;
            let mut i = 0usize;
            while i + 1 < t_count {
                // Do two chops at once.
                let mut tt = Float2::load(&t_values[i..]);
                if i != 0 {
                    let last_t = t_values[i - 1];
                    tt = ((tt - last_t) / (1.0 - last_t))
                        .pin(Float2::splat(0.0), Float2::splat(1.0));
                }
                chop_cubic_at_t0_t1(&cur_src, &mut dst[off..], tt[0], tt[1]);
                // src = dst = dst + 6
                cur_src = [dst[off + 6], dst[off + 7], dst[off + 8], dst[off + 9]];
                off += 6;
                i += 2;
            }
            if i < t_count {
                // Chop the final cubic if there was an odd number of chops.
                debug_assert_eq!(i + 1, t_count);
                let mut t = t_values[i];
                if i != 0 {
                    let last_t = t_values[i - 1];
                    t = t_pin(ieee_float_divide(t - last_t, 1.0 - last_t), 0.0f32, 1.0f32);
                }
                chop_cubic_at(&cur_src, &mut dst[off..], t);
            }
        }
    }
}

/// Given a src cubic bezier, chop it at `t == 1/2`. The new cubics are returned in `dst[0..4]`
/// and `dst[3..7]`.
// Port of: src/core/SkGeometry.cpp#L575-L577 (chrome/m156)
#[doc(alias = "SkChopCubicAtHalf")]
pub fn chop_cubic_at_half(src: &[Point], dst: &mut [Point]) {
    chop_cubic_at(src, dst, 0.5);
}

/// Given a cubic curve with no inflection points, this method measures the rotation in radians.
///
/// Rotation is perhaps easiest described via a driving analogy: If you drive your car along the
/// curve from p0 to p3, then by the time you arrive at p3, how many radians will your car have
/// rotated? This is not quite the same as the vector inside the tangents at the endpoints, even
/// without inflection, because the curve might rotate around the outside of the tangents
/// (>= 180 degrees) or the inside (<= 180 degrees).
///
/// Cubics can have rotations in the range `[0, 2*pi]`.
///
/// NOTE: The caller must either call [`chop_cubic_at_inflections`] or otherwise prove that the
/// provided cubic has no inflection points prior to calling this method.
// Port of: src/core/SkGeometry.cpp#L579-L595 (chrome/m156)
#[doc(alias = "SkMeasureNonInflectCubicRotation")]
#[must_use]
pub fn measure_non_inflect_cubic_rotation(pts: &[Point]) -> f32 {
    let a = pts[1] - pts[0];
    let b = pts[2] - pts[1];
    let c = pts[3] - pts[2];
    if a.is_zero() {
        return measure_angle_between_vectors(b, c);
    }
    if b.is_zero() {
        return measure_angle_between_vectors(a, c);
    }
    if c.is_zero() {
        return measure_angle_between_vectors(a, b);
    }
    // Postulate: When no points are colocated and there are no inflection points in T=0..1, the
    // rotation is: 360 degrees, minus the angle [p0,p1,p2], minus the angle [p1,p2,p3].
    2.0 * SCALAR_PI - measure_angle_between_vectors(a, -b) - measure_angle_between_vectors(b, -c)
}

// Port of: src/core/SkGeometry.cpp#L597-L599 (chrome/m156)
fn fma(f: Float4, m: f32, a: Float4) -> Float4 {
    vx::fma(f, Float4::splat(m), a)
}

// Finds the root nearest 0.5. Returns 0.5 if the roots are undefined or outside 0..1.
// Port of: src/core/SkGeometry.cpp#L602-L614 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
fn solve_quadratic_equation_for_midtangent_discr(a: f32, b: f32, c: f32, discr: f32) -> f32 {
    // Quadratic formula from Numerical Recipes in C:
    let q = -0.5f32 * (b + discr.sqrt().copysign(b));
    // The roots are q/a and c/q. Pick the midtangent closer to T=.5.
    let half_qa = -0.5f32 * q * a;
    let mut t = if (q * q + half_qa).abs() < (a * c + half_qa).abs() {
        ieee_float_divide(q, a)
    } else {
        ieee_float_divide(c, q)
    };
    if !(t > 0.0 && t < 1.0) {
        // Use "!(positive_logic)" so T=NaN will take this branch.
        // Either the curve is a flat line with no rotation or FP precision failed us. Chop at .5.
        t = 0.5;
    }
    t
}

// Port of: src/core/SkGeometry.cpp#L616-L618 (chrome/m156)
pub(crate) fn solve_quadratic_equation_for_midtangent(a: f32, b: f32, c: f32) -> f32 {
    solve_quadratic_equation_for_midtangent_discr(a, b, c, b * b - 4.0 * a * c)
}

/// Given a src cubic bezier, returns the T value whose tangent angle is halfway between the
/// tangents at p0 and p3.
// Port of: src/core/SkGeometry.cpp#L620-L684 (chrome/m156)
#[doc(alias = "SkFindCubicMidTangent")]
#[must_use]
pub fn find_cubic_mid_tangent(src: &[Point]) -> f32 {
    // Tangents point in the direction of increasing T, so tan0 and -tan1 both point toward the
    // midtangent. The bisector of tan0 and -tan1 is orthogonal to the midtangent:
    //
    //     bisector dot midtangent == 0
    //
    let tan0 = if src[0] == src[1] {
        src[2] - src[0]
    } else {
        src[1] - src[0]
    };
    let tan1 = if src[2] == src[3] {
        src[3] - src[1]
    } else {
        src[3] - src[2]
    };
    let bisector = find_bisector(tan0, -tan1);

    // Find the T value at the midtangent. This is a simple quadratic equation:
    //
    //     midtangent dot bisector == 0, or using a tangent matrix C' in power basis form:
    //
    //                   |C'x  C'y|
    //     |T^2  T  1| * |.    .  | * |bisector.x| == 0
    //                   |.    .  |   |bisector.y|
    //
    // The coeffs for the quadratic equation we need to solve are therefore:  C' * bisector
    let k_m = [
        Float4::from_list(&[-1.0, 2.0, -1.0, 0.0]),
        Float4::from_list(&[3.0, -4.0, 1.0, 0.0]),
        Float4::from_list(&[-3.0, 2.0, 0.0, 0.0]),
    ];
    let c_x = fma(
        k_m[0],
        src[0].x,
        fma(
            k_m[1],
            src[1].x,
            fma(
                k_m[2],
                src[2].x,
                Float4::from_list(&[src[3].x, 0.0, 0.0, 0.0]),
            ),
        ),
    );
    let c_y = fma(
        k_m[0],
        src[0].y,
        fma(
            k_m[1],
            src[1].y,
            fma(
                k_m[2],
                src[2].y,
                Float4::from_list(&[src[3].y, 0.0, 0.0, 0.0]),
            ),
        ),
    );
    let mut coeffs = c_x * bisector.x + c_y * bisector.y;

    // Now solve the quadratic for T.
    let mut t = 0.0f32;
    let (mut a, mut b, c) = (coeffs[0], coeffs[1], coeffs[2]);
    let discr = b * b - 4.0 * a * c;
    if discr > 0.0 {
        // This will only be false if the curve is a line.
        solve_quadratic_equation_for_midtangent_discr(a, b, c, discr)
    } else {
        // This is a 0- or 360-degree flat line. It doesn't have single points of midtangent.
        // (tangent == midtangent at every point on the curve except the cusp points.)
        // Chop in between both cusps instead, if any. There can be up to two cusps on a flat line,
        // both where the tangent is perpendicular to the starting tangent:
        //
        //     tangent dot tan0 == 0
        //
        coeffs = c_x * tan0.x + c_y * tan0.y;
        a = coeffs[0];
        b = coeffs[1];
        if a != 0.0 {
            // We want the point in between both cusps. The midpoint of:
            //
            //     (-b +/- sqrt(b^2 - 4*a*c)) / (2*a)
            //
            // Is equal to:
            //
            //     -b / (2*a)
            t = -b / (2.0 * a);
        }
        if !(t > 0.0 && t < 1.0) {
            // Use "!(positive_logic)" so T=NaN will take this branch.
            // Either the curve is a flat line with no rotation or FP precision failed us. Chop at
            // .5.
            t = 0.5;
        }
        t
    }
}

/// Given a src cubic bezier, chop it at the tangent whose angle is halfway between the tangents
/// at p0 and p3. The new cubics are returned in `dst[0..4]` and `dst[3..7]`.
///
/// NOTE: 0- and 360-degree flat lines don't have single points of midtangent. (tangent ==
/// midtangent at every point on these curves except the cusp points.) If this is the case then we
/// simply chop at a point which guarantees neither side rotates more than 180 degrees.
// Port of: src/core/SkGeometry.h#L205-L207 (chrome/m156)
#[doc(alias = "SkChopCubicAtMidTangent")]
pub fn chop_cubic_at_mid_tangent(src: &[Point], dst: &mut [Point]) {
    chop_cubic_at(src, dst, find_cubic_mid_tangent(src));
}

// Port of: src/core/SkGeometry.cpp#L734-L736 (chrome/m156)
fn flatten_double_cubic_extrema_y(dst: &mut [Point]) {
    // coords[4] = coords[8] = coords[6] (starting at dst[0].fY)
    dst[4].y = dst[3].y;
    dst[2].y = dst[3].y;
}

fn flatten_double_cubic_extrema_x(dst: &mut [Point]) {
    dst[4].x = dst[3].x;
    dst[2].x = dst[3].x;
}

/// Given 4 points on a cubic bezier, chop it into 1, 2, 3 beziers such that the resulting
/// beziers are monotonic in Y. This is called by the scan converter. Returns the number of
/// chops: 0 means `dst[0..4]` is the original cubic, 1 means `dst[0..4]` and `dst[3..7]` are the
/// two new cubics, 2 means `dst[0..4]`, `dst[3..7]`, `dst[6..10]` are the three new cubics.
/// If `dst` is `None`, it is ignored and only the count is returned.
// Port of: src/core/SkGeometry.cpp#L698-L712 (chrome/m156)
#[doc(alias = "SkChopCubicAtYExtrema")]
#[must_use]
pub fn chop_cubic_at_y_extrema(src: &[Point], dst: Option<&mut [Point]>) -> usize {
    let mut t_values = [0.0f32; 2];
    let roots = find_cubic_extrema(src[0].y, src[1].y, src[2].y, src[3].y, &mut t_values);

    if let Some(dst) = dst {
        chop_cubic_at_ts(src, Some(&mut *dst), &t_values[..roots]);
        if roots > 0 {
            // we do some cleanup to ensure our Y extrema are flat
            flatten_double_cubic_extrema_y(dst);
            if roots == 2 {
                flatten_double_cubic_extrema_y(&mut dst[3..]);
            }
        }
    }
    roots
}

/// Like [`chop_cubic_at_y_extrema`], for X.
// Port of: src/core/SkGeometry.cpp#L714-L728 (chrome/m156)
#[doc(alias = "SkChopCubicAtXExtrema")]
#[must_use]
pub fn chop_cubic_at_x_extrema(src: &[Point], dst: Option<&mut [Point]>) -> usize {
    let mut t_values = [0.0f32; 2];
    let roots = find_cubic_extrema(src[0].x, src[1].x, src[2].x, src[3].x, &mut t_values);

    if let Some(dst) = dst {
        chop_cubic_at_ts(src, Some(&mut *dst), &t_values[..roots]);
        if roots > 0 {
            // we do some cleanup to ensure our Y extrema are flat
            flatten_double_cubic_extrema_x(dst);
            if roots == 2 {
                flatten_double_cubic_extrema_x(&mut dst[3..]);
            }
        }
    }
    roots
}

/// Given a cubic bezier, return 0, 1, or 2 t-values that represent the inflection points.
///
/// Inflection means that curvature is zero. Curvature is `[F' x F''] / [F'^3]`, so we solve
/// `F'x X F''y - F'y X F''y == 0`. With `A = b - a`, `B = c - 2b + a`, `C = d - 3c + 3b - a`:
/// `(BxCy - ByCx)t^2 + (AxCy - AyCx)t + AxBy - AyBx == 0`.
// Port of: src/core/SkGeometry.cpp#L741-L753 (chrome/m156)
#[doc(alias = "SkFindCubicInflections")]
#[must_use]
pub fn find_cubic_inflections(src: &[Point], t_values: &mut [scalar; 2]) -> usize {
    let ax = src[1].x - src[0].x;
    let ay = src[1].y - src[0].y;
    let bx = src[2].x - 2.0 * src[1].x + src[0].x;
    let by = src[2].y - 2.0 * src[1].y + src[0].y;
    let cx = src[3].x + 3.0 * (src[1].x - src[2].x) - src[0].x;
    let cy = src[3].y + 3.0 * (src[1].y - src[2].y) - src[0].y;

    find_unit_quad_roots(
        bx * cy - by * cx,
        ax * cy - ay * cx,
        ax * by - ay * bx,
        t_values,
    )
}

/// Returns 1 for no chop, 2 for having chopped the cubic at a single inflection point, 3 for
/// having chopped at 2 inflection points. `dst` will hold the resulting 1, 2, or 3 cubics.
// Port of: src/core/SkGeometry.cpp#L755-L767 (chrome/m156)
#[doc(alias = "SkChopCubicAtInflections")]
#[must_use]
pub fn chop_cubic_at_inflections(src: &[Point], dst: Option<&mut [Point]>) -> usize {
    let mut t_values = [0.0f32; 2];
    let count = find_cubic_inflections(src, &mut t_values);

    if let Some(dst) = dst {
        if count == 0 {
            dst[..4].copy_from_slice(&src[..4]);
        } else {
            chop_cubic_at_ts(src, Some(dst), &t_values[..count]);
        }
    }
    count + 1
}

// Assumes the third component of points is 1.
// Calcs p0 . (p1 x p2)
// Port of: src/core/SkGeometry.cpp#L771-L776 (chrome/m156)
fn calc_dot_cross_cubic(p0: Point, p1: Point, p2: Point) -> f64 {
    let x_comp = f64::from(p0.x) * (f64::from(p1.y) - f64::from(p2.y));
    let y_comp = f64::from(p0.y) * (f64::from(p2.x) - f64::from(p1.x));
    let w_comp = f64::from(p1.x) * f64::from(p2.y) - f64::from(p1.y) * f64::from(p2.x);
    x_comp + y_comp + w_comp
}

// Returns a positive power of 2 that, when multiplied by n, and excepting the two edge cases listed
// below, shifts the exponent of n to yield a magnitude somewhere inside [1..2).
// Returns 2^1023 if abs(n) < 2^-1022 (including 0).
// Returns NaN if n is Inf or NaN.
// Port of: src/core/SkGeometry.cpp#L782-L789 (chrome/m156)
fn previous_inverse_pow2(n: f64) -> f64 {
    let mut bits = n.to_bits();
    // exp=-exp (unsigned wraparound, as in C++)
    bits = (((1023u64 * 2) << 52) + ((1u64 << 52) - 1)).wrapping_sub(bits);
    bits &= 0x7ffu64 << 52; // mantissa=1.0, sign=0
    f64::from_bits(bits)
}

// Port of: src/core/SkGeometry.cpp#L791-L807 (chrome/m156)
fn write_cubic_inflection_roots(
    t0: f64,
    s0: f64,
    t1: f64,
    s1: f64,
    t: &mut [f64; 2],
    s: &mut [f64; 2],
) {
    t[0] = t0;
    s[0] = s0;

    // This copysign/abs business orients the implicit function so positive values are always on
    // the "left" side of the curve.
    t[1] = -(t1.copysign(t1 * s1));
    s[1] = -s1.abs();

    // Ensure t[0]/s[0] <= t[1]/s[1] (s[1] is negative from above).
    if s[1].copysign(s[0]) * t[0] > -s[0].abs() * t[1] {
        t.swap(0, 1);
        s.swap(0, 1);
    }
}

/// The classification of a cubic bezier curve (`SkCubicType`).
// Port of: src/core/SkGeometry.h#L261-L268 (chrome/m156)
#[doc(alias = "SkCubicType")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CubicType {
    /// `kSerpentine`.
    Serpentine,
    /// `kLoop`.
    Loop,
    /// `kLocalCusp`: cusp at a non-infinite parameter value with an inflection at t=infinity.
    LocalCusp,
    /// `kCuspAtInfinity`: cusp with a cusp at t=infinity and a local inflection.
    CuspAtInfinity,
    /// `kQuadratic`.
    Quadratic,
    /// `kLineOrPoint`.
    LineOrPoint,
}

impl CubicType {
    /// `SkCubicIsDegenerate`.
    // Port of: src/core/SkGeometry.h#L270-L284 (chrome/m156)
    #[doc(alias = "SkCubicIsDegenerate")]
    #[must_use]
    pub fn is_degenerate(self) -> bool {
        match self {
            CubicType::Serpentine
            | CubicType::Loop
            | CubicType::LocalCusp
            | CubicType::CuspAtInfinity => false,
            CubicType::Quadratic | CubicType::LineOrPoint => true,
        }
    }

    /// `SkCubicTypeName`.
    // Port of: src/core/SkGeometry.h#L286-L297 (chrome/m156)
    #[doc(alias = "SkCubicTypeName")]
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            CubicType::Serpentine => "kSerpentine",
            CubicType::Loop => "kLoop",
            CubicType::LocalCusp => "kLocalCusp",
            CubicType::CuspAtInfinity => "kCuspAtInfinity",
            CubicType::Quadratic => "kQuadratic",
            CubicType::LineOrPoint => "kLineOrPoint",
        }
    }
}

/// Returns the cubic classification.
///
/// If the input points contain infinities or NaN, the return values are undefined.
/// See [`classify_cubic_with`] for the optional outputs.
// Port of: src/core/SkGeometry.cpp#L809-L879 (chrome/m156)
#[doc(alias = "SkClassifyCubic")]
#[must_use]
pub fn classify_cubic(p: &[Point]) -> CubicType {
    classify_cubic_with(p, None, None, None)
}

/// Returns the cubic classification.
///
/// `t` and `s` are set to the two homogeneous parameter values at which points the lines L & M
/// intersect with K, sorted from smallest to largest and oriented so positive values of the
/// implicit are on the "left" side. For a serpentine curve they are the inflection points. For a
/// loop they are the double point. For a local cusp, they are both equal and denote the cusp
/// point. For a cusp at an infinite parameter value, one will be the local inflection point and
/// the other +inf (t,s = 1,0). If the curve is degenerate (i.e. quadratic or linear) they are
/// both set to a parameter value of +inf (t,s = 1,0). They are only written if both are `Some`.
///
/// `d` is filled with the cubic inflection function coefficients. See "Resolution Independent
/// Curve Rendering using Programmable Graphics Hardware", 4.2 Curve Categorization.
///
/// If the input points contain infinities or NaN, the return values are undefined.
// Port of: src/core/SkGeometry.cpp#L809-L879 (chrome/m156)
#[doc(alias = "SkClassifyCubic")]
#[must_use]
#[allow(clippy::many_single_char_names)] // names follow the C++
pub fn classify_cubic_with(
    p: &[Point],
    t: Option<&mut [f64; 2]>,
    s: Option<&mut [f64; 2]>,
    d: Option<&mut [f64; 4]>,
) -> CubicType {
    // Find the cubic's inflection function, I = [T^3  -3T^2  3T  -1] dot D. (D0 will always be 0
    // for integral cubics.)
    //
    // See "Resolution Independent Curve Rendering using Programmable Graphics Hardware",
    // 4.2 Curve Categorization:
    //
    // https://www.microsoft.com/en-us/research/wp-content/uploads/2005/01/p1000-loop.pdf
    let a1 = calc_dot_cross_cubic(p[0], p[3], p[2]);
    let a2 = calc_dot_cross_cubic(p[1], p[0], p[3]);
    let a3 = calc_dot_cross_cubic(p[2], p[1], p[0]);

    let mut d3 = 3.0 * a3;
    let mut d2 = d3 - a2;
    let mut d1 = d2 - a2 + a1;

    // Shift the exponents in D so the largest magnitude falls somewhere in 1..2. This protects us
    // from overflow down the road while solving for roots and KLM functionals.
    // std::max(a, b) is `(a < b) ? b : a`.
    let std_max = |x: f64, y: f64| if x < y { y } else { x };
    let d_max = std_max(std_max(d1.abs(), d2.abs()), d3.abs());
    let norm = previous_inverse_pow2(d_max);
    d1 *= norm;
    d2 *= norm;
    d3 *= norm;

    if let Some(d) = d {
        d[3] = d3;
        d[2] = d2;
        d[1] = d1;
        d[0] = 0.0;
    }

    let mut ts = match (t, s) {
        (Some(t), Some(s)) => Some((t, s)),
        _ => None,
    };

    // Now use the inflection function to classify the cubic.
    //
    // See "Resolution Independent Curve Rendering using Programmable Graphics Hardware",
    // 4.4 Integral Cubics:
    //
    // https://www.microsoft.com/en-us/research/wp-content/uploads/2005/01/p1000-loop.pdf
    if d1 != 0.0 {
        let discr = 3.0 * d2 * d2 - 4.0 * d1 * d3;
        if discr > 0.0 {
            // Serpentine.
            if let Some((t, s)) = ts.as_mut() {
                let q = 3.0 * d2 + (3.0 * discr).sqrt().copysign(d2);
                write_cubic_inflection_roots(q, 6.0 * d1, 2.0 * d3, q, t, s);
            }
            CubicType::Serpentine
        } else if discr < 0.0 {
            // Loop.
            if let Some((t, s)) = ts.as_mut() {
                let q = d2 + (-discr).sqrt().copysign(d2);
                write_cubic_inflection_roots(q, 2.0 * d1, 2.0 * (d2 * d2 - d3 * d1), d1 * q, t, s);
            }
            CubicType::Loop
        } else {
            // Cusp.
            if let Some((t, s)) = ts.as_mut() {
                write_cubic_inflection_roots(d2, 2.0 * d1, d2, 2.0 * d1, t, s);
            }
            CubicType::LocalCusp
        }
    } else if d2 != 0.0 {
        // Cusp at T=infinity.
        if let Some((t, s)) = ts.as_mut() {
            write_cubic_inflection_roots(d3, 3.0 * d2, 1.0, 0.0, t, s); // T1=infinity.
        }
        CubicType::CuspAtInfinity
    } else {
        // Degenerate.
        if let Some((t, s)) = ts.as_mut() {
            write_cubic_inflection_roots(1.0, 0.0, 1.0, 0.0, t, s); // T0=T1=infinity.
        }
        if d3 == 0.0 {
            CubicType::LineOrPoint
        } else {
            CubicType::Quadratic
        }
    }
}

// Port of: src/core/SkGeometry.cpp#L881-L890 (chrome/m156)
fn bubble_sort(array: &mut [scalar], count: usize) {
    for i in (1..count).rev() {
        for j in (1..=i).rev() {
            if array[j] < array[j - 1] {
                array.swap(j, j - 1);
            }
        }
    }
}

/// Given an array and count, remove all pair-wise duplicates from the array, keeping the
/// existing sorting, and return the new count.
// Port of: src/core/SkGeometry.cpp#L896-L908 (chrome/m156)
#[allow(clippy::float_cmp)] // exact float comparisons, as in Skia
fn collaps_duplicates(array: &mut [scalar], count: usize) -> usize {
    let mut count = count;
    let mut base = 0usize;
    let mut n = count;
    while n > 1 {
        if array[base] == array[base + 1] {
            for i in 1..n {
                array[base + i - 1] = array[base + i];
            }
            count -= 1;
        } else {
            base += 1;
        }
        n -= 1;
    }
    count
}

// Port of: src/core/SkGeometry.cpp#L950-L952 (chrome/m156)
fn scalar_cube_root(x: scalar) -> scalar {
    scalar_pow(x, 0.333_333_3_f32)
}

/*  Solve coeff(t) == 0, returning the number of roots that
    lie within 0 < t < 1.
    coeff[0]t^3 + coeff[1]t^2 + coeff[2]t + coeff[3]

    Eliminates repeated roots (so that all tValues are distinct, and are always
    in increasing order.
*/
// Port of: src/core/SkGeometry.cpp#L961-L1008 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
fn solve_cubic_poly(coeff: &[scalar; 4], t_values: &mut [scalar; 3]) -> usize {
    if Scalar::nearly_zero(&coeff[0], None) {
        // we're just a quadratic
        let mut roots = [0.0f32; 2];
        let count = find_unit_quad_roots(coeff[1], coeff[2], coeff[3], &mut roots);
        t_values[..2].copy_from_slice(&roots);
        return count;
    }

    debug_assert!(coeff[0] != 0.0);

    let inva = scalar_invert(coeff[0]);
    let a = coeff[1] * inva;
    let b = coeff[2] * inva;
    let c = coeff[3] * inva;
    let q = (a * a - b * 3.0) / 9.0;
    let r = (2.0 * a * a * a - 9.0 * a * b + 27.0 * c) / 54.0;

    let q3 = q * q * q;
    let r2_minus_q3 = r * r - q3;
    let adiv3 = a / 3.0;

    if r2_minus_q3 < 0.0 {
        // we have 3 real roots
        // the divide/root can, due to finite precisions, be slightly outside of -1...1
        let theta = scalar_acos(t_pin(r / scalar_sqrt(q3), -1.0f32, 1.0f32));
        let neg2_root_q = -2.0 * scalar_sqrt(q);

        t_values[0] = t_pin(
            neg2_root_q * scalar_cos(theta / 3.0) - adiv3,
            0.0f32,
            1.0f32,
        );
        t_values[1] = t_pin(
            neg2_root_q * scalar_cos((theta + 2.0 * SCALAR_PI) / 3.0) - adiv3,
            0.0f32,
            1.0f32,
        );
        t_values[2] = t_pin(
            neg2_root_q * scalar_cos((theta - 2.0 * SCALAR_PI) / 3.0) - adiv3,
            0.0f32,
            1.0f32,
        );

        // now sort the roots
        bubble_sort(t_values, 3);
        collaps_duplicates(t_values, 3)
    } else {
        // we have 1 real root
        let mut big_a = scalar_abs(r) + scalar_sqrt(r2_minus_q3);
        big_a = scalar_cube_root(big_a);
        if r > 0.0 {
            big_a = -big_a;
        }
        if big_a != 0.0 {
            big_a += q / big_a;
        }
        t_values[0] = t_pin(big_a - adiv3, 0.0f32, 1.0f32);
        1
    }
}

/*  Looking for F' dot F'' == 0

    A = b - a
    B = c - 2b + a
    C = d - 3c + 3b - a

    F' = 3Ct^2 + 6Bt + 3A
    F'' = 6Ct + 6B

    F' dot F'' -> CCt^3 + 3BCt^2 + (2BB + CA)t + AB
*/
// Port of: src/core/SkGeometry.cpp#L1021-L1030 (chrome/m156)
fn formulate_f1_dot_f2(src: [scalar; 4]) -> [scalar; 4] {
    let a = src[1] - src[0];
    let b = src[2] - 2.0 * src[1] + src[0];
    let c = src[3] + 3.0 * (src[1] - src[2]) - src[0];

    [c * c, 3.0 * b * c, 2.0 * b * b + c * a, a * b]
}

/// Finds the t values (up to 3) where the cubic has an extremum of curvature, i.e. where
/// `F' dot F'' == 0`.
// Port of: src/core/SkGeometry.cpp#L1043-L1058 (chrome/m156)
#[doc(alias = "SkFindCubicMaxCurvature")]
#[must_use]
pub fn find_cubic_max_curvature(src: &[Point], t_values: &mut [scalar; 3]) -> usize {
    let mut coeff_x = formulate_f1_dot_f2([src[0].x, src[1].x, src[2].x, src[3].x]);
    let coeff_y = formulate_f1_dot_f2([src[0].y, src[1].y, src[2].y, src[3].y]);

    for i in 0..4 {
        coeff_x[i] += coeff_y[i];
    }

    // now remove extrema where the curvature is zero (mins)
    // !!!! need a test for this !!!!
    solve_cubic_poly(&coeff_x, t_values)
}

/// Chops the cubic at its points of max curvature that lie inside `0..1`. Returns the number of
/// resulting cubics (1 to 4), stored in `dst` if it is `Some`. The t values used are stored in
/// `t_values` if it is `Some`.
// Port of: src/core/SkGeometry.cpp#L1060-L1087 (chrome/m156)
#[doc(alias = "SkChopCubicAtMaxCurvature")]
#[must_use]
pub fn chop_cubic_at_max_curvature(
    src: &[Point],
    dst: Option<&mut [Point]>,
    t_values: Option<&mut [scalar; 3]>,
) -> usize {
    let mut t_storage = [0.0f32; 3];
    let t_values = t_values.unwrap_or(&mut t_storage);

    let mut roots = [0.0f32; 3];
    let root_count = find_cubic_max_curvature(src, &mut roots);

    // Throw out values not inside 0..1.
    let mut count = 0usize;
    for &root in &roots[..root_count] {
        if 0.0 < root && root < 1.0 {
            t_values[count] = root;
            count += 1;
        }
    }

    if let Some(dst) = dst {
        if count == 0 {
            dst[..4].copy_from_slice(&src[..4]);
        } else {
            chop_cubic_at_ts(src, Some(dst), &t_values[..count]);
        }
    }
    count + 1
}

// Returns a constant proportional to the dimensions of the cubic.
// Constant found through experimentation -- maybe there's a better way....
// Port of: src/core/SkGeometry.cpp#L1091-L1094 (chrome/m156)
fn calc_cubic_precision(src: &[Point]) -> scalar {
    (point_priv::distance_to_sqd(src[1], src[0])
        + point_priv::distance_to_sqd(src[2], src[1])
        + point_priv::distance_to_sqd(src[3], src[2]))
        * 1e-8f32
}

// Returns true if both points src[testIndex], src[testIndex+1] are in the same half plane defined
// by the line segment src[lineIndex], src[lineIndex+1].
// Port of: src/core/SkGeometry.cpp#L1098-L1107 (chrome/m156)
fn on_same_side(src: &[Point], test_index: usize, line_index: usize) -> bool {
    let origin = src[line_index];
    let line = src[line_index + 1] - origin;
    let mut crosses = [0.0f32; 2];
    for (index, cross) in crosses.iter_mut().enumerate() {
        let test_line = src[test_index + index] - origin;
        *cross = line.cross(test_line);
    }
    crosses[0] * crosses[1] >= 0.0
}

/// Return location (in t) of cubic cusp, if there is one; returns -1 otherwise.
///
/// Note that classify cubic code does not reliably return all cusp'd cubics, so it is not called
/// here.
// Port of: src/core/SkGeometry.cpp#L1112-L1150 (chrome/m156)
#[doc(alias = "SkFindCubicCusp")]
#[must_use]
pub fn find_cubic_cusp(src: &[Point]) -> scalar {
    // When the adjacent control point matches the end point, it behaves as if
    // the cubic has a cusp: there's a point of max curvature where the derivative
    // goes to zero. Ideally, this would be where t is zero or one, but math
    // error makes not so. It is not uncommon to create cubics this way; skip them.
    if src[0] == src[1] {
        return -1.0;
    }
    if src[2] == src[3] {
        return -1.0;
    }
    // Cubics only have a cusp if the line segments formed by the control and end points cross.
    // Detect crossing if line ends are on opposite sides of plane formed by the other line.
    if on_same_side(src, 0, 2) || on_same_side(src, 2, 0) {
        return -1.0;
    }
    // Cubics may have multiple points of maximum curvature, although at most only
    // one is a cusp.
    let mut max_curvature = [0.0f32; 3];
    let roots = find_cubic_max_curvature(src, &mut max_curvature);
    for &test_t in &max_curvature[..roots] {
        if 0.0 >= test_t || test_t >= 1.0 {
            // no need to consider max curvature on the end
            continue;
        }
        // A cusp is at the max curvature, and also has a derivative close to zero.
        // Choose the 'close to zero' meaning by comparing the derivative length
        // with the overall cubic size.
        let d_pt = eval_cubic_derivative(src, test_t);
        let d_pt_magnitude = point_priv::length_sqd(d_pt);
        let precision = calc_cubic_precision(src);
        if d_pt_magnitude < precision {
            // All three max curvature t values may be close to the cusp;
            // return the first one.
            return test_t;
        }
    }
    -1.0
}

// Port of: src/core/SkGeometry.cpp#L1152-L1154 (chrome/m156)
fn close_enough_to_zero(x: f64) -> bool {
    x.abs() < 0.00001
}

// Port of: src/core/SkGeometry.cpp#L1156-L1186 (chrome/m156)
fn first_axis_intersection(
    coefficients: &[f64; 8],
    y_direction: bool,
    axis_intercept: f64,
) -> Option<f64> {
    let [a, b, c, mut d] = BezierCubic::convert_to_polynomial(coefficients, y_direction);
    d -= axis_intercept;
    let mut roots = [0.0f64; 3];
    let count = cubics::roots_valid_t(a, b, c, d, &mut roots);
    if count == 0 {
        return None;
    }
    // Verify that at least one of the roots is accurate.
    let accurate = |&root: &f64| close_enough_to_zero(cubics::eval_at(a, b, c, d, root));
    if let Some(&root) = roots[..count].iter().find(|r| accurate(r)) {
        return Some(root);
    }
    // None of the roots returned by our normal cubic solver were correct enough
    // (e.g. https://bugs.chromium.org/p/oss-fuzz/issues/detail?id=55732)
    // So we need to fallback to a more accurate solution.
    let count = cubics::binary_search_roots_valid_t(a, b, c, d, &mut roots);
    if count == 0 {
        return None;
    }
    roots[..count].iter().find(|r| accurate(r)).copied()
}

fn chop_mono_cubic_at(src: &[Point], value: scalar, y_direction: bool, dst: &mut [Point]) -> bool {
    let coefficients = [
        f64::from(src[0].x),
        f64::from(src[0].y),
        f64::from(src[1].x),
        f64::from(src[1].y),
        f64::from(src[2].x),
        f64::from(src[2].y),
        f64::from(src[3].x),
        f64::from(src[3].y),
    ];
    if let Some(solution) = first_axis_intersection(&coefficients, y_direction, f64::from(value)) {
        let mut cubic_pair = [0.0f64; 14];
        BezierCubic::subdivide(&coefficients, solution, &mut cubic_pair);
        for (i, pt) in dst[..7].iter_mut().enumerate() {
            pt.x = double_to_float(cubic_pair[i * 2]);
            pt.y = double_to_float(cubic_pair[i * 2 + 1]);
        }
        return true;
    }
    false
}

/// Given a monotonically increasing or decreasing cubic bezier `src`, chop it where the Y value
/// is the specified value. The returned cubics will be in `dst`, sharing the middle point. That
/// is, the first cubic is `dst[0..4]` and the second `dst[3..7]`.
///
/// If the cubic provided is *not* monotone, it will be chopped at the first time the curve has
/// the specified Y value.
///
/// If the cubic never reaches the specified value, the function returns false.
// Port of: src/core/SkGeometry.cpp#L1188-L1202 (chrome/m156)
#[doc(alias = "SkChopMonoCubicAtY")]
pub fn chop_mono_cubic_at_y(src: &[Point], y: scalar, dst: &mut [Point]) -> bool {
    chop_mono_cubic_at(src, y, true, dst)
}

/// Like [`chop_mono_cubic_at_y`], for X.
// Port of: src/core/SkGeometry.cpp#L1204-L1218 (chrome/m156)
#[doc(alias = "SkChopMonoCubicAtX")]
pub fn chop_mono_cubic_at_x(src: &[Point], x: scalar, dst: &mut [Point]) -> bool {
    chop_mono_cubic_at(src, x, false, dst)
}
