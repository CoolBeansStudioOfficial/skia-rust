// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkGeometry.h, src/core/SkGeometry.cpp

//! Conics (rational quadratic Beziers): `SkConic`, `SkConicCoeff` and `SkAutoConicToQuads`.

use skia_rust_simd::vx::{self, Float2};

use super::{
    QuadCoeff, find_bisector, find_unit_quad_roots, from_point,
    solve_quadratic_equation_for_midtangent, times_2, to_point, to_vector,
};
use crate::floating_point::{ieee_double_divide, is_finite, is_nan};
use crate::matrix::Matrix;
use crate::path_types::PathDirection;
use crate::point::{Point, Vector, point_priv};
use crate::point3::Point3;
use crate::rect::Rect;
use crate::scalar::{
    SCALAR_1, SCALAR_HALF, SCALAR_NEARLY_ZERO, SCALAR_ROOT_2_OVER_2, scalar, scalar_abs,
    scalar_interp, scalar_invert, scalar_sqrt,
};

// Port of: src/core/SkGeometry.h#L425-L453 (chrome/m156)
/// Power basis coefficients of a conic: numerator and denominator quadratics.
#[doc(alias = "SkConicCoeff")]
#[derive(Copy, Clone, Debug)]
pub struct ConicCoeff {
    /// `fNumer`.
    pub numer: QuadCoeff,
    /// `fDenom`.
    pub denom: QuadCoeff,
}

impl ConicCoeff {
    /// `SkConicCoeff(const SkConic&)`.
    #[must_use]
    pub fn new(conic: &Conic) -> Self {
        let p0 = from_point(conic.pts[0]);
        let p1 = from_point(conic.pts[1]);
        let p2 = from_point(conic.pts[2]);
        let ww = Float2::splat(conic.w);

        let p1w = p1 * ww;
        let numer_c = p0;
        let numer_a = p2 - times_2(p1w) + p0;
        let numer_b = times_2(p1w - p0);

        let denom_c = Float2::splat(1.0);
        let denom_b = times_2(ww - denom_c);
        let denom_a = Float2::splat(0.0) - denom_b;
        Self {
            numer: QuadCoeff::new(numer_a, numer_b, numer_c),
            denom: QuadCoeff::new(denom_a, denom_b, denom_c),
        }
    }

    /// `eval(t)`.
    #[must_use]
    pub fn eval(&self, t: scalar) -> Float2 {
        let tt = Float2::splat(t);
        let numer = self.numer.eval(tt);
        let denom = self.denom.eval(tt);
        numer / denom
    }
}

// ---------------------------------------------------------------------------------------------
// Conics
// ---------------------------------------------------------------------------------------------

// F' = 2 (C t (1 + t (-1 + w)) - A (-1 + t) (t (-1 + w) - w) + B (1 - 2 t) w)
//
//  t^2 : (2 P0 - 2 P2 - 2 P0 w + 2 P2 w)
//  t^1 : (-2 P0 + 2 P2 + 4 P0 w - 4 P1 w)
//  t^0 : -2 P0 w + 2 P1 w
//
//  We disregard magnitude, so we can freely ignore the denominator of F', and
//  divide the numerator by 2
//
//    coeff[0] for t^2
//    coeff[1] for t^1
//    coeff[2] for t^0
//
// Port of: src/core/SkGeometry.cpp#L1251-L1260 (chrome/m156)
fn conic_deriv_coeff(src: [scalar; 3], w: scalar) -> [scalar; 3] {
    let p20 = src[2] - src[0];
    let p10 = src[1] - src[0];
    let w_p10 = w * p10;
    [w * p20 - p20, p20 - 2.0 * w_p10, w_p10]
}

// Port of: src/core/SkGeometry.cpp#L1262-L1275 (chrome/m156)
fn conic_find_extrema(src: [scalar; 3], w: scalar) -> Option<scalar> {
    let coeff = conic_deriv_coeff(src, w);

    let mut t_values = [0.0f32; 2];
    let roots = find_unit_quad_roots(coeff[0], coeff[1], coeff[2], &mut t_values);
    debug_assert!(roots == 0 || roots == 1);

    if roots == 1 {
        return Some(t_values[0]);
    }
    None
}

// We only interpolate one dimension at a time (the first, at +0, +3, +6).
// Port of: src/core/SkGeometry.cpp#L1278-L1284 (chrome/m156)
fn p3d_interp(src: [scalar; 3], t: scalar) -> [scalar; 3] {
    let ab = scalar_interp(src[0], src[1], t);
    let bc = scalar_interp(src[1], src[2], t);
    [ab, scalar_interp(ab, bc, t), bc]
}

// Port of: src/core/SkGeometry.cpp#L1286-L1290 (chrome/m156)
fn ratquad_map_to_3d(src: &[Point], w: scalar) -> [Point3; 3] {
    [
        Point3::new(src[0].x * 1.0, src[0].y * 1.0, 1.0),
        Point3::new(src[1].x * w, src[1].y * w, w),
        Point3::new(src[2].x * 1.0, src[2].y * 1.0, 1.0),
    ]
}

// Port of: src/core/SkGeometry.cpp#L1292-L1294 (chrome/m156)
fn project_down(src: Point3) -> Point {
    Point::new(src.x / src.z, src.y / src.z)
}

/// A conic section (a rational quadratic bezier): three points and a weight (`SkConic`).
// Port of: src/core/SkGeometry.h#L304-L408 (chrome/m156)
#[doc(alias = "SkConic")]
#[derive(Copy, Clone, Debug, Default)]
pub struct Conic {
    /// `fPts`.
    pub pts: [Point; 3],
    /// `fW`.
    pub w: scalar,
}

/// `SkConic::kMaxConicsForArc`.
pub const MAX_CONICS_FOR_ARC: usize = 5;

// Limit the number of suggested quads to approximate a conic
// Port of: src/core/SkGeometry.cpp#L1527 (chrome/m156)
const MAX_CONIC_TO_QUAD_POW2: i32 = 5;

// Port of: src/core/SkGeometry.cpp#L1488-L1490 (chrome/m156)
fn bad_conic_w(w: f32) -> bool {
    w < 0.0 || !is_finite(w)
}

// This was originally developed and tested for pathops: see SkOpTypes.h
// returns true if (a <= b <= c) || (a >= b >= c)
// Port of: src/core/SkGeometry.cpp#L1529-L1531 (chrome/m156)
fn between(a: scalar, b: scalar, c: scalar) -> bool {
    (a - b) * (c - b) <= 0.0
}

// Port of: src/core/SkGeometry.cpp#L1533-L1572 (chrome/m156)
fn subdivide(src: &Conic, pts: &mut [Point], pos: usize, level: i32) -> usize {
    debug_assert!(level >= 0);

    if level == 0 {
        pts[pos] = src.pts[1];
        pts[pos + 1] = src.pts[2];
        pos + 2
    } else {
        let mut dst = [Conic::default(); 2];
        src.chop(&mut dst);
        let start_y = src.pts[0].y;
        let end_y = src.pts[2].y;
        if between(start_y, src.pts[1].y, end_y) {
            // If the input is monotonic and the output is not, the scan converter hangs.
            // Ensure that the chopped conics maintain their y-order.
            let mid_y = dst[0].pts[2].y;
            if !between(start_y, mid_y, end_y) {
                // If the computed midpoint is outside the ends, move it to the closer one.
                let closer_y = if scalar_abs(mid_y - start_y) < scalar_abs(mid_y - end_y) {
                    start_y
                } else {
                    end_y
                };
                dst[0].pts[2].y = closer_y;
                dst[1].pts[0].y = closer_y;
            }
            if !between(start_y, dst[0].pts[1].y, dst[0].pts[2].y) {
                // If the 1st control is not between the start and end, put it at the start.
                // This also reduces the quad to a line.
                dst[0].pts[1].y = start_y;
            }
            if !between(dst[1].pts[0].y, dst[1].pts[1].y, end_y) {
                // If the 2nd control is not between the start and end, put it at the end.
                // This also reduces the quad to a line.
                dst[1].pts[1].y = end_y;
            }
            // Verify that all five points are in order.
            debug_assert!(between(start_y, dst[0].pts[1].y, dst[0].pts[2].y));
            debug_assert!(between(dst[0].pts[1].y, dst[0].pts[2].y, dst[1].pts[1].y));
            debug_assert!(between(dst[0].pts[2].y, dst[1].pts[1].y, end_y));
        }
        let level = level - 1;
        let pos = subdivide(&dst[0], pts, pos, level);
        subdivide(&dst[1], pts, pos, level)
    }
}

// Port of: src/core/SkGeometry.cpp#L1397-L1399 (chrome/m156)
fn subdivide_w_value(w: scalar) -> scalar {
    scalar_sqrt(SCALAR_HALF + w * SCALAR_HALF)
}

impl Conic {
    /// `SkConic(p0, p1, p2, w)`.
    // Port of: src/core/SkGeometry.h#L306-L308 (chrome/m156)
    #[must_use]
    pub fn new(p0: Point, p1: Point, p2: Point, w: scalar) -> Self {
        let mut conic = Self::default();
        conic.set(p0, p1, p2, w);
        conic
    }

    /// `SkConic(const SkPoint pts[3], SkScalar w)`.
    // Port of: src/core/SkGeometry.h#L310-L312 (chrome/m156)
    #[doc(alias = "set")]
    #[must_use]
    pub fn from_points(pts: &[Point], w: scalar) -> Self {
        let mut conic = Self::default();
        conic.set_points(pts, w);
        conic
    }

    /// `set(const SkPoint pts[3], SkScalar w)`.
    // Port of: src/core/SkGeometry.h#L320-L323 (chrome/m156)
    pub fn set_points(&mut self, pts: &[Point], w: scalar) {
        self.pts.copy_from_slice(&pts[..3]);
        self.set_w(w);
    }

    /// `set(p0, p1, p2, w)`.
    // Port of: src/core/SkGeometry.h#L325-L330 (chrome/m156)
    pub fn set(&mut self, p0: Point, p1: Point, p2: Point, w: scalar) {
        self.pts = [p0, p1, p2];
        self.set_w(w);
    }

    /// `setW`: guards against bad weights by forcing them to 1.
    // Port of: src/core/SkGeometry.h#L332-L339 (chrome/m156)
    #[doc(alias = "setW")]
    pub fn set_w(&mut self, w: scalar) {
        if is_finite(w) {
            debug_assert!(w > 0.0);
        }

        // Guard against bad weights by forcing them to 1.
        self.w = if w > 0.0 && is_finite(w) { w } else { 1.0 };
    }

    /// Given a t-value `[0...1]` return its position and/or tangent. NOTE the tangent value's
    /// length is arbitrary, and only its direction should be used.
    // Port of: src/core/SkGeometry.cpp#L1386-L1395 (chrome/m156)
    #[doc(alias = "evalAt")]
    pub fn eval_at_pos_tangent(
        &self,
        t: scalar,
        pos: Option<&mut Point>,
        tangent: Option<&mut Vector>,
    ) {
        debug_assert!((0.0..=SCALAR_1).contains(&t));

        if let Some(pos) = pos {
            *pos = self.eval_at(t);
        }
        if let Some(tangent) = tangent {
            *tangent = self.eval_tangent_at(t);
        }
    }

    /// Splits the conic at `t`. Returns false if infinity or NaN is generated.
    // Port of: src/core/SkGeometry.cpp#L1297-L1326 (chrome/m156)
    #[doc(alias = "chopAt")]
    #[must_use]
    pub fn chop_at(&self, t: scalar, dst: &mut [Conic; 2]) -> bool {
        let tmp = ratquad_map_to_3d(&self.pts, self.w);

        let xs = p3d_interp([tmp[0].x, tmp[1].x, tmp[2].x], t);
        let ys = p3d_interp([tmp[0].y, tmp[1].y, tmp[2].y], t);
        let zs = p3d_interp([tmp[0].z, tmp[1].z, tmp[2].z], t);
        let tmp2 = [
            Point3::new(xs[0], ys[0], zs[0]),
            Point3::new(xs[1], ys[1], zs[1]),
            Point3::new(xs[2], ys[2], zs[2]),
        ];

        dst[0].pts[0] = self.pts[0];
        dst[0].pts[1] = project_down(tmp2[0]);
        dst[0].pts[2] = project_down(tmp2[1]);
        dst[1].pts[0] = dst[0].pts[2];
        dst[1].pts[1] = project_down(tmp2[2]);
        dst[1].pts[2] = self.pts[2];

        // to put in "standard form", where w0 and w2 are both 1, we compute the
        // new w1 as sqrt(w1*w1/w0*w2)
        // or
        // w1 /= sqrt(w0*w2)
        //
        // However, in our case, we know that for dst[0]:
        //     w0 == 1, and for dst[1], w2 == 1
        //
        let root = scalar_sqrt(tmp2[1].z);
        dst[0].w = tmp2[0].z / root;
        dst[1].w = tmp2[2].z / root;
        // SkIsFinite(&dst[0].fPts[0].fX, 7 * 2): both conics are laid out contiguously.
        dst.iter()
            .all(|c| c.pts.iter().all(|p| is_finite(p.x) && is_finite(p.y)) && is_finite(c.w))
    }

    /// Chops the conic between `t1` and `t2` into `dst`.
    // Port of: src/core/SkGeometry.cpp#L1328-L1358 (chrome/m156)
    #[doc(alias = "chopAt")]
    #[allow(clippy::float_cmp, clippy::manual_midpoint)] // exact float comparisons, as in Skia; mirrors the C++ `(a + b) / 2`
    pub fn chop_at_interval(&self, t1: scalar, t2: scalar, dst: &mut Conic) {
        if 0.0 == t1 || 1.0 == t2 {
            if 0.0 == t1 && 1.0 == t2 {
                *dst = *self;
                return;
            }
            let mut pair = [Conic::default(); 2];
            if self.chop_at(if t1 == 0.0 { t2 } else { t1 }, &mut pair) {
                *dst = pair[usize::from(t1 != 0.0)];
                return;
            }
        }
        let coeff = ConicCoeff::new(self);
        let tt1 = Float2::splat(t1);
        let a_xy = coeff.numer.eval(tt1);
        let a_zz = coeff.denom.eval(tt1);
        let mid_tt = Float2::splat((t1 + t2) / 2.0);
        let d_xy = coeff.numer.eval(mid_tt);
        let d_zz = coeff.denom.eval(mid_tt);
        let tt2 = Float2::splat(t2);
        let c_xy = coeff.numer.eval(tt2);
        let c_zz = coeff.denom.eval(tt2);
        let b_xy = times_2(d_xy) - (a_xy + c_xy) * 0.5f32;
        let b_zz = times_2(d_zz) - (a_zz + c_zz) * 0.5f32;
        dst.pts[0] = to_point(a_xy / a_zz);
        dst.pts[1] = to_point(b_xy / b_zz);
        dst.pts[2] = to_point(c_xy / c_zz);
        let ww = b_zz / vx::sqrt(a_zz * c_zz);
        dst.w = ww[0];
    }

    /// Evaluates the conic's position at `t`.
    // Port of: src/core/SkGeometry.cpp#L1360-L1362 (chrome/m156)
    #[doc(alias = "evalAt")]
    #[must_use]
    pub fn eval_at(&self, t: scalar) -> Point {
        to_point(ConicCoeff::new(self).eval(t))
    }

    /// Evaluates the conic's tangent at `t`.
    // Port of: src/core/SkGeometry.cpp#L1364-L1384 (chrome/m156)
    #[doc(alias = "evalTangentAt")]
    #[must_use]
    #[allow(clippy::float_cmp)] // exact float comparisons, as in Skia
    pub fn eval_tangent_at(&self, t: scalar) -> Vector {
        // The derivative equation returns a zero tangent vector when t is 0 or 1,
        // and the control point is equal to the end point.
        // In this case, use the conic endpoints to compute the tangent.
        if (t == 0.0 && self.pts[0] == self.pts[1]) || (t == 1.0 && self.pts[1] == self.pts[2]) {
            return self.pts[2] - self.pts[0];
        }
        let p0 = from_point(self.pts[0]);
        let p1 = from_point(self.pts[1]);
        let p2 = from_point(self.pts[2]);
        let ww = Float2::splat(self.w);

        let p20 = p2 - p0;
        let p10 = p1 - p0;

        let c = ww * p10;
        let a = ww * p20 - p20;
        let b = p20 - c - c;

        to_vector(QuadCoeff::new(a, b, c).eval(Float2::splat(t)))
    }

    /// Splits the conic in half, into `dst`.
    // Port of: src/core/SkGeometry.cpp#L1430-L1462 (chrome/m156)
    pub fn chop(&self, dst: &mut [Conic; 2]) {
        // Observe that scale will always be smaller than 1 because fW > 0.
        let scale = scalar_invert(SCALAR_1 + self.w);

        // The subdivided control points below are the sums of the following three terms. Because
        // the terms are multiplied by something <1, and the resulting control points lie within
        // the control points of the original then the terms and the sums below will not overflow.
        // Note that fW * scale approaches 1 as fW becomes very large.
        let t0 = from_point(self.pts[0]) * scale;
        let t1 = from_point(self.pts[1]) * (self.w * scale);
        let t2 = from_point(self.pts[2]) * scale;

        // Calculate the subdivided control points
        let p1 = to_point(t0 + t1);
        let p3 = to_point(t1 + t2);

        // p2 = (t0 + 2*t1 + t2) / 2. Divide the terms by 2 before the sum to keep the sum for p2
        // from overflowing.
        let p2 = to_point(0.5f32 * t0 + t1 + 0.5f32 * t2);

        debug_assert!(p1.is_finite() && p2.is_finite() && p3.is_finite());

        dst[0].pts[0] = self.pts[0];
        dst[0].pts[1] = p1;
        dst[0].pts[2] = p2;
        dst[1].pts[0] = p2;
        dst[1].pts[1] = p3;
        dst[1].pts[2] = self.pts[2];

        // Update w.
        let w = subdivide_w_value(self.w);
        dst[0].w = w;
        dst[1].w = w;
    }

    // "High order approximation of conic sections by quadratic splines" by Michael Floater, 1993
    // Port of: src/core/SkGeometry.cpp#L1527-L1532 (chrome/m156)
    fn as_quad_error_setup(&self) -> (scalar, scalar) {
        let a = self.w - 1.0;
        let k = a / (4.0 * (2.0 + a));
        let x = k * (self.pts[0].x - 2.0 * self.pts[1].x + self.pts[2].x);
        let y = k * (self.pts[0].y - 2.0 * self.pts[1].y + self.pts[2].y);
        (x, y)
    }

    /// `computeAsQuadError`: the error of approximating this conic with a single quad.
    // Port of: src/core/SkGeometry.cpp#L1475-L1478 (chrome/m156)
    #[doc(alias = "computeAsQuadError")]
    #[must_use]
    pub fn compute_as_quad_error(&self) -> Vector {
        let (x, y) = self.as_quad_error_setup();
        Vector::new(x, y)
    }

    /// `asQuadTol`.
    // Port of: src/core/SkGeometry.cpp#L1480-L1483 (chrome/m156)
    #[doc(alias = "asQuadTol")]
    #[must_use]
    pub fn as_quad_tol(&self, tol: scalar) -> bool {
        let (x, y) = self.as_quad_error_setup();
        (x * x + y * y) <= tol * tol
    }

    /// Returns the power-of-2 number of quads needed to approximate this conic with a sequence
    /// of quads. Will be >= 0.
    // Port of: src/core/SkGeometry.cpp#L1492-L1525 (chrome/m156)
    #[doc(alias = "computeQuadPOW2")]
    #[must_use]
    pub fn compute_quad_pow2(&self, tol: scalar) -> i32 {
        if tol < 0.0 || !is_finite(tol) || !point_priv::are_finite(&self.pts) || bad_conic_w(self.w)
        {
            return 0;
        }

        let (x, y) = self.as_quad_error_setup();

        let mut error = scalar_sqrt(x * x + y * y);
        let mut pow2 = 0;
        while pow2 < MAX_CONIC_TO_QUAD_POW2 {
            if error <= tol {
                break;
            }
            error *= 0.25f32;
            pow2 += 1;
        }
        pow2
    }

    /// Chops this conic into N quads, stored contiguously in `pts`, where `N = 1 << pow2`. The
    /// amount of storage needed is `1 + 2 * N`. Returns the number of quads.
    // Port of: src/core/SkGeometry.cpp#L1574-L1609 (chrome/m156)
    #[doc(alias = "chopIntoQuadsPOW2")]
    #[must_use]
    pub fn chop_into_quads_pow2(&self, pts: &mut [Point], pow2: i32) -> usize {
        debug_assert!((0..=MAX_CONIC_TO_QUAD_POW2).contains(&pow2));
        let mut pow2 = pow2;

        if bad_conic_w(self.w) {
            pow2 = 0;
        }

        pts[0] = self.pts[0];
        let mut end_pts = None;
        if pow2 == MAX_CONIC_TO_QUAD_POW2 {
            // If an extreme weight generates many quads ...
            let mut dst = [Conic::default(); 2];
            self.chop(&mut dst);
            // check to see if the first chop generates a pair of lines
            if point_priv::equals_within_tolerance(dst[0].pts[1], dst[0].pts[2])
                && point_priv::equals_within_tolerance(dst[1].pts[0], dst[1].pts[1])
            {
                pts[1] = dst[0].pts[1];
                pts[2] = dst[0].pts[1];
                pts[3] = dst[0].pts[1]; // set ctrl == end to make lines
                pts[4] = dst[1].pts[2];
                pow2 = 1;
                end_pts = Some(5);
            }
        }
        let end_pts = end_pts.unwrap_or_else(|| subdivide(self, pts, 1, pow2));
        let quad_count = 1usize << pow2;
        let pt_count = 2 * quad_count + 1;
        debug_assert_eq!(end_pts, pt_count);
        if !point_priv::are_finite(&pts[..pt_count]) {
            // if we generated a non-finite, pin ourselves to the middle of the hull,
            // as our first and last are already on the first/last pts of the hull.
            for pt in &mut pts[1..pt_count - 1] {
                *pt = self.pts[1];
            }
        }
        quad_count
    }

    /// Returns the T value whose tangent angle is halfway between the tangents at p0 and p2.
    // Port of: src/core/SkGeometry.cpp#L1611-L1646 (chrome/m156)
    #[doc(alias = "findMidTangent")]
    #[must_use]
    pub fn find_mid_tangent(&self) -> f32 {
        // Tangents point in the direction of increasing T, so tan0 and -tan1 both point toward the
        // midtangent. The bisector of tan0 and -tan1 is orthogonal to the midtangent:
        //
        //     bisector dot midtangent = 0
        //
        let tan0 = self.pts[1] - self.pts[0];
        let tan1 = self.pts[2] - self.pts[1];
        let bisector = find_bisector(tan0, -tan1);

        // Start by finding the tangent function's power basis coefficients. These define a tangent
        // direction (scaled by some uniform value) as:
        //                                                |T^2|
        //     Tangent_Direction(T) = dx,dy = |A  B  C| * |T  |
        //                                    |.  .  .|   |1  |
        //
        // The derivative of a conic has a cumbersome order-4 denominator. However, this isn't
        // necessary if we are only interested in a vector in the same *direction* as a given
        // tangent line. Since the denominator scales dx and dy uniformly, we can throw it out
        // completely after evaluating the derivative with the standard quotient rule. This leaves
        // us with a simpler quadratic function that we use to find a tangent.
        let a = (self.pts[2] - self.pts[0]) * (self.w - 1.0);
        let b = (self.pts[2] - self.pts[0]) - (self.pts[1] - self.pts[0]) * (self.w * 2.0);
        let c = (self.pts[1] - self.pts[0]) * self.w;

        // Now solve for "bisector dot midtangent = 0":
        //
        //                            |T^2|
        //     bisector * |A  B  C| * |T  | = 0
        //                |.  .  .|   |1  |
        //
        let a = bisector.dot(a);
        let b = bisector.dot(b);
        let c = bisector.dot(c);
        solve_quadratic_equation_for_midtangent(a, b, c)
    }

    /// Finds the t of the X extremum, if there is one in `0 < t < 1`.
    // Port of: src/core/SkGeometry.cpp#L1648-L1650 (chrome/m156)
    #[doc(alias = "findXExtrema")]
    #[must_use]
    pub fn find_x_extrema(&self) -> Option<scalar> {
        conic_find_extrema([self.pts[0].x, self.pts[1].x, self.pts[2].x], self.w)
    }

    /// Finds the t of the Y extremum, if there is one in `0 < t < 1`.
    // Port of: src/core/SkGeometry.cpp#L1652-L1654 (chrome/m156)
    #[doc(alias = "findYExtrema")]
    #[must_use]
    pub fn find_y_extrema(&self) -> Option<scalar> {
        conic_find_extrema([self.pts[0].y, self.pts[1].y, self.pts[2].y], self.w)
    }

    /// Chops the conic at its X extremum.
    // Port of: src/core/SkGeometry.cpp#L1656-L1672 (chrome/m156)
    #[doc(alias = "chopAtXExtrema")]
    #[must_use]
    pub fn chop_at_x_extrema(&self, dst: &mut [Conic; 2]) -> bool {
        if let Some(t) = self.find_x_extrema() {
            if !self.chop_at(t, dst) {
                // if chop can't return finite values, don't chop
                return false;
            }
            // now clean-up the middle, since we know t was meant to be at
            // an X-extrema
            let value = dst[0].pts[2].x;
            dst[0].pts[1].x = value;
            dst[1].pts[0].x = value;
            dst[1].pts[1].x = value;
            return true;
        }
        false
    }

    /// Chops the conic at its Y extremum.
    // Port of: src/core/SkGeometry.cpp#L1674-L1690 (chrome/m156)
    #[doc(alias = "chopAtYExtrema")]
    #[must_use]
    pub fn chop_at_y_extrema(&self, dst: &mut [Conic; 2]) -> bool {
        if let Some(t) = self.find_y_extrema() {
            if !self.chop_at(t, dst) {
                // if chop can't return finite values, don't chop
                return false;
            }
            // now clean-up the middle, since we know t was meant to be at
            // an Y-extrema
            let value = dst[0].pts[2].y;
            dst[0].pts[1].y = value;
            dst[1].pts[0].y = value;
            dst[1].pts[1].y = value;
            return true;
        }
        false
    }

    /// The bounds of the conic curve itself.
    // Port of: src/core/SkGeometry.cpp#L1692-L1706 (chrome/m156)
    #[doc(alias = "computeTightBounds")]
    #[must_use]
    pub fn compute_tight_bounds(&self) -> Rect {
        let mut pts = [Point::default(); 4];
        pts[0] = self.pts[0];
        pts[1] = self.pts[2];
        let mut count = 2usize;

        if let Some(t) = self.find_x_extrema() {
            pts[count] = self.eval_at(t);
            count += 1;
        }
        if let Some(t) = self.find_y_extrema() {
            pts[count] = self.eval_at(t);
            count += 1;
        }
        Rect::bounds_or_empty(&pts[..count])
    }

    /// The bounds of the conic's control points.
    // Port of: src/core/SkGeometry.cpp#L1708-L1710 (chrome/m156)
    #[doc(alias = "computeFastBounds")]
    #[must_use]
    pub fn compute_fast_bounds(&self) -> Rect {
        Rect::bounds_or_empty(&self.pts)
    }

    /// The weight of the conic `pts`/`w` after it is transformed by `matrix` (unchanged unless
    /// the matrix has perspective).
    // Port of: src/core/SkGeometry.cpp#L1719-L1736 (chrome/m156)
    #[doc(alias = "TransformW")]
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // sk_double_to_float
    pub fn transform_w(pts: &[Point], w: scalar, matrix: &Matrix) -> scalar {
        if !matrix.has_perspective() {
            return w;
        }

        let src = ratquad_map_to_3d(pts, w);
        let mut dst = [Point3::default(); 3];

        matrix.map_homogeneous_points(&mut dst, &src);

        // w' = sqrt(w1*w1/w0*w2)
        // use doubles temporarily, to handle small numer/denom
        let w0 = f64::from(dst[0].z);
        let w1 = f64::from(dst[1].z);
        let w2 = f64::from(dst[2].z);
        ieee_double_divide(w1 * w1, w0 * w2).sqrt() as f32
    }

    /// Builds the conics (at most [`MAX_CONICS_FOR_ARC`]) for the unit-circle arc from `u_start`
    /// to `u_stop` in direction `dir`, mapped by `user_matrix`; returns how many were written to
    /// `dst` (0 if the vectors are effectively coincident).
    // Port of: src/core/SkGeometry.cpp#L1738-L1823 (chrome/m156)
    #[doc(alias = "BuildUnitArc")]
    #[allow(clippy::float_cmp)] // exact comparisons, as in C++
    #[allow(clippy::manual_range_contains)] // mirrors the C++ comparisons
    #[allow(clippy::manual_midpoint)] // mirrors the C++ `(a + b) / 2` arithmetic
    pub fn build_unit_arc(
        u_start: Vector,
        u_stop: Vector,
        dir: PathDirection,
        user_matrix: Option<&Matrix>,
        dst: &mut [Conic; MAX_CONICS_FOR_ARC],
    ) -> usize {
        // rotate by x,y so that uStart is (1.0)
        let x = Point::dot_product(u_start, u_stop);
        let mut y = Point::cross_product(u_start, u_stop);

        let abs_y = scalar_abs(y);

        // check for (effectively) coincident vectors
        // this can happen if our angle is nearly 0 or nearly 180 (y == 0)
        // ... we use the dot-prod to distinguish between 0 and 180 (x > 0)
        if abs_y <= SCALAR_NEARLY_ZERO
            && x > 0.0
            && ((y >= 0.0 && PathDirection::CW == dir) || (y <= 0.0 && PathDirection::CCW == dir))
        {
            return 0;
        }

        if dir == PathDirection::CCW {
            y = -y;
        }

        // We decide to use 1-conic per quadrant of a circle. What quadrant does [xy] lie in?
        //      0 == [0  .. 90)
        //      1 == [90 ..180)
        //      2 == [180..270)
        //      3 == [270..360)
        //
        let mut quadrant = 0;
        if 0.0 == y {
            quadrant = 2; // 180
            debug_assert!(scalar_abs(x + SCALAR_1) <= SCALAR_NEARLY_ZERO);
        } else if 0.0 == x {
            debug_assert!(abs_y - SCALAR_1 <= SCALAR_NEARLY_ZERO);
            quadrant = if y > 0.0 { 1 } else { 3 }; // 90 : 270
        } else {
            if y < 0.0 {
                quadrant += 2;
            }
            if (x < 0.0) != (y < 0.0) {
                quadrant += 1;
            }
        }

        let quadrant_pts = [
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
            Point::new(-1.0, 1.0),
            Point::new(-1.0, 0.0),
            Point::new(-1.0, -1.0),
            Point::new(0.0, -1.0),
            Point::new(1.0, -1.0),
        ];
        let quadrant_weight = SCALAR_ROOT_2_OVER_2;

        let mut conic_count = quadrant;
        for (i, conic) in dst.iter_mut().enumerate().take(conic_count) {
            conic.set_points(&quadrant_pts[i * 2..i * 2 + 3], quadrant_weight);
        }

        // Now compute any remaing (sub-90-degree) arc for the last conic
        let final_p = Point::new(x, y);
        let last_q = quadrant_pts[quadrant * 2]; // will already be a unit-vector
        let dot = Point::dot_product(last_q, final_p);
        if is_nan(dot) {
            return 0;
        }
        debug_assert!(0.0 <= dot && dot <= SCALAR_1 + SCALAR_NEARLY_ZERO);

        if dot < 1.0 {
            let mut off_curve = Vector::new(last_q.x + x, last_q.y + y);
            // compute the bisector vector, and then rescale to be the off-curve point.
            // we compute its length from cos(theta/2) = length / 1, using half-angle identity we
            // get length = sqrt(2 / (1 + cos(theta)). We already have cos() when to computed the
            // dot. This is nice, since our computed weight is cos(theta/2) as well!
            //
            let cos_theta_over_2 = scalar_sqrt((1.0 + dot) / 2.0);
            off_curve.set_length(scalar_invert(cos_theta_over_2));
            if !point_priv::equals_within_tolerance(last_q, off_curve) {
                dst[conic_count].set(last_q, off_curve, final_p, cos_theta_over_2);
                conic_count += 1;
            }
        }

        // now handle counter-clockwise and the initial unitStart rotation
        let mut matrix = Matrix::default();
        matrix.set_sin_cos((u_start.y, u_start.x), None);
        if dir == PathDirection::CCW {
            matrix.pre_scale((SCALAR_1, -SCALAR_1), None);
        }
        if let Some(user_matrix) = user_matrix {
            matrix.post_concat(user_matrix);
        }
        for conic in dst.iter_mut().take(conic_count) {
            matrix.map_points_inplace(&mut conic.pts);
        }
        conic_count
    }
}

/// Helper class to allocate storage for approximating a conic with N quads
/// (`SkAutoConicToQuads`).
// Port of: src/core/SkGeometry.h#L482-L541 (chrome/m156)
#[doc(alias = "SkAutoConicToQuads")]
#[derive(Clone, Debug, Default)]
pub struct AutoConicToQuads {
    storage: Vec<Point>,
    quad_count: usize,
}

impl AutoConicToQuads {
    /// `SkAutoConicToQuads()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Given a conic and a tolerance, return the array of points for the approximating quad(s).
    /// Call [`count_quads`](Self::count_quads) to know the number of quads represented in these
    /// points.
    ///
    /// The quads are allocated to share end-points. e.g. if there are 4 quads, there will be 9
    /// points allocated as follows: `quad[0] == pts[0..3]`, `quad[1] == pts[2..5]`,
    /// `quad[2] == pts[4..7]`, `quad[3] == pts[6..9]`.
    // Port of: src/core/SkGeometry.h#L497-L503 (chrome/m156)
    #[doc(alias = "computeQuads")]
    pub fn compute_quads(&mut self, conic: &Conic, tol: scalar) -> &[Point] {
        let pow2 = conic.compute_quad_pow2(tol);
        self.quad_count = 1 << pow2;
        self.storage = vec![Point::default(); 1 + 2 * self.quad_count];
        self.quad_count = conic.chop_into_quads_pow2(&mut self.storage, pow2);
        &self.storage
    }

    /// [`compute_quads`](Self::compute_quads) for the three points `pts` and a weight.
    // Port of: src/core/SkGeometry.h#L505-L526 (chrome/m156)
    #[doc(alias = "computeQuads")]
    #[allow(clippy::manual_midpoint)] // mirrors the C++ `(a + b) / 2`
    pub fn compute_quads_with_weight(
        &mut self,
        pts: &[Point],
        weight: scalar,
        tol: scalar,
    ) -> &[Point] {
        if weight <= 0.0 {
            // The underlying conic.chopIntoQuads... is based on an algorithm that
            // requires a positive, non-zero weight. So for w = 0 (or any invalid w < 0),
            // we just make this a quad that is a line between the two points.
            self.quad_count = 1;
            self.storage = vec![
                pts[0],
                Point::new(
                    (pts[0].x + pts[2].x) * 0.5f32,
                    (pts[0].y + pts[2].y) * 0.5f32,
                ),
                pts[2],
            ];
            return &self.storage;
        }
        let mut conic = Conic::default();
        conic.set_points(pts, weight);
        self.compute_quads(&conic, tol)
    }

    /// The number of quads computed by the last call to `compute_quads*`.
    // Port of: src/core/SkGeometry.h#L532 (chrome/m156)
    #[doc(alias = "countQuads")]
    #[must_use]
    pub fn count_quads(&self) -> usize {
        self.quad_count
    }
}
