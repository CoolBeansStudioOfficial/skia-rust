// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkBezierCurves.h, src/core/SkBezierCurves.cpp

//! `SkBezierCubic` and `SkBezierQuad`: double-precision utilities for Bézier curves.
//!
//! A cubic Bézier curve is stored as an array of 8 doubles, where the even indices are the X
//! coordinates, and the odd indices are the Y coordinates.

use crate::cubics;
use crate::floating_point::double_to_float;
use crate::point::Point;
use crate::quads;

// Port of: src/core/SkBezierCurves.cpp#L19-L21 (chrome/m156)
fn interpolate(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Utilities for dealing with cubic Bézier curves (`SkBezierCubic`).
///
/// These have a start XY point, an end XY point, and two control XY points in between. They take
/// a parameter `t` which is between 0 and 1 (inclusive) which is used to interpolate between the
/// start and end points, via a route dictated by the control points, and return a new XY point.
#[doc(alias = "SkBezierCubic")]
#[derive(Copy, Clone, Debug)]
pub struct BezierCubic;

impl BezierCubic {
    /// Evaluates the cubic Bézier curve for a given `t`. It returns an X and Y coordinate
    /// following the formula
    /// `X(t) = X_0*(1-t)^3 + 3*X_1*t(1-t)^2 + 3*X_2*t^2(1-t) + X_3*t^3` (and likewise for Y).
    ///
    /// `t` is typically in the range `[0, 1]`, but this function will not assert that, as
    /// Bézier curves are well-defined for any real number input.
    // Port of: src/core/SkBezierCurves.cpp#L23-L55 (chrome/m156)
    #[doc(alias = "EvalAt")]
    #[must_use]
    #[allow(clippy::float_cmp)] // exact fast paths for t == 0 and t == 1, as in Skia
    #[allow(clippy::many_single_char_names)] // names follow the C++
    pub fn eval_at(curve: &[f64; 8], t: f64) -> [f64; 2] {
        let in_x = |n: usize| curve[2 * n];
        let in_y = |n: usize| curve[2 * n + 1];

        // Two semi-common fast paths
        if t == 0.0 {
            return [in_x(0), in_y(0)];
        }
        if t == 1.0 {
            return [in_x(3), in_y(3)];
        }
        // X(t) = X_0*(1-t)^3 + 3*X_1*t(1-t)^2 + 3*X_2*t^2(1-t) + X_3*t^3
        // Y(t) = Y_0*(1-t)^3 + 3*Y_1*t(1-t)^2 + 3*Y_2*t^2(1-t) + Y_3*t^3
        let one_minus_t = 1.0 - t;
        let one_minus_t_squared = one_minus_t * one_minus_t;
        let a = one_minus_t_squared * one_minus_t;
        let b = 3.0 * one_minus_t_squared * t;
        let t_squared = t * t;
        let c = 3.0 * one_minus_t * t_squared;
        let d = t_squared * t;

        [
            a * in_x(0) + b * in_x(1) + c * in_x(2) + d * in_x(3),
            a * in_y(0) + b * in_y(1) + c * in_y(2) + d * in_y(3),
        ]
    }

    /// Splits the provided Bézier curve at the location `t`, resulting in two Bézier curves that
    /// share a point (the end point from curve 1 and the start point from curve 2 are the same).
    ///
    /// `t` must be in the interval `[0, 1]`.
    ///
    /// The returned array is filled such that indices 0-7 are the first curve (representing the
    /// interval `[0, t]`), and indices 6-13 are the second curve (representing `[t, 1]`).
    // Port of: src/core/SkBezierCurves.cpp#L57-L97 (chrome/m156)
    #[doc(alias = "Subdivide")]
    pub fn subdivide(curve: &[f64; 8], t: f64, two_curves: &mut [f64; 14]) {
        debug_assert!((0.0..=1.0).contains(&t));
        // We split the curve "in" into two curves "alpha" and "beta"
        let in_x = |n: usize| curve[2 * n];
        let in_y = |n: usize| curve[2 * n + 1];

        // alpha_X(n) = two_curves[2n], alpha_Y(n) = two_curves[2n + 1],
        // beta_X(n) = two_curves[2n + 6], beta_Y(n) = two_curves[2n + 7]
        two_curves[0] = in_x(0);
        two_curves[1] = in_y(0);

        two_curves[12] = in_x(3);
        two_curves[13] = in_y(3);

        let x01 = interpolate(in_x(0), in_x(1), t);
        let y01 = interpolate(in_y(0), in_y(1), t);
        let x12 = interpolate(in_x(1), in_x(2), t);
        let y12 = interpolate(in_y(1), in_y(2), t);
        let x23 = interpolate(in_x(2), in_x(3), t);
        let y23 = interpolate(in_y(2), in_y(3), t);

        two_curves[2] = x01;
        two_curves[3] = y01;

        two_curves[10] = x23;
        two_curves[11] = y23;

        two_curves[4] = interpolate(x01, x12, t);
        two_curves[5] = interpolate(y01, y12, t);

        two_curves[8] = interpolate(x12, x23, t);
        two_curves[9] = interpolate(y12, y23, t);

        // alpha_X(3) = beta_X(0)
        two_curves[6] = interpolate(two_curves[4], two_curves[8], t);
        two_curves[7] = interpolate(two_curves[5], two_curves[9], t);
    }

    /// Converts the provided Bézier curve into the equivalent cubic `f(t) = A*t^3 + B*t^2 + C*t + D`
    /// where `f(t)` will represent Y coordinates over time if `y_values` is true and the X
    /// coordinates if `y_values` is false.
    ///
    /// In effect, this turns the control points into an actual line, representing the x or y
    /// values.
    // Port of: src/core/SkBezierCurves.cpp#L99-L115 (chrome/m156)
    #[doc(alias = "ConvertToPolynomial")]
    #[must_use]
    pub fn convert_to_polynomial(curve: &[f64; 8], y_values: bool) -> [f64; 4] {
        let offset = usize::from(y_values);
        let p = |n: usize| curve[offset + 2 * n];
        // A cubic Bézier curve is interpolated as follows:
        //  c(t) = (1 - t)^3 P_0 + 3t(1 - t)^2 P_1 + 3t^2 (1 - t) P_2 + t^3 P_3
        //       = (-P_0 + 3P_1 + -3P_2 + P_3) t^3 + (3P_0 - 6P_1 + 3P_2) t^2 +
        //         (-3P_0 + 3P_1) t + P_0
        // Where P_N is the Nth point.
        [
            -p(0) + 3.0 * p(1) - 3.0 * p(2) + p(3),
            3.0 * p(0) - 6.0 * p(1) + 3.0 * p(2),
            -3.0 * p(0) + 3.0 * p(1),
            p(0),
        ]
    }

    /// Finds the X values of the curve (given by the first four of `control_points`) where its Y
    /// equals `y_intercept`. The results are stored in `intersection_storage` and returned as a
    /// slice of it.
    // Port of: src/core/SkBezierCurves.cpp#L153-L168 (chrome/m156)
    #[doc(alias = "IntersectWithHorizontalLine")]
    pub fn intersect_with_horizontal_line<'a>(
        control_points: &[Point],
        y_intercept: f32,
        intersection_storage: &'a mut [f32; 3],
    ) -> &'a [f32] {
        debug_assert!(control_points.len() >= 4);
        let p0 = DPoint::from(control_points[0]);
        let p1 = DPoint::from(control_points[1]);
        let p2 = DPoint::from(control_points[2]);
        let p3 = DPoint::from(control_points[3]);

        let a = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
        let b = 3.0 * p0 - 6.0 * p1 + 3.0 * p2;
        let c = -3.0 * p0 + 3.0 * p1;
        let d = p0;

        Self::intersect(
            a.x,
            b.x,
            c.x,
            d.x,
            a.y,
            b.y,
            c.y,
            d.y,
            y_intercept,
            intersection_storage,
        )
    }

    /// Finds the values of the X polynomial (`AX..DX`) at the `t` where the Y polynomial
    /// (`AY..DY`) equals `to_intersect`, for `t` on `[0, 1]`.
    // Port of: src/core/SkBezierCurves.cpp#L170-L187 (chrome/m156)
    #[doc(alias = "Intersect")]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    pub fn intersect(
        ax: f64,
        bx: f64,
        cx: f64,
        dx: f64,
        ay: f64,
        by: f64,
        cy: f64,
        dy: f64,
        to_intersect: f32,
        intersections_storage: &mut [f32; 3],
    ) -> &[f32] {
        let mut roots = [0.0f64; 3];
        let count = cubics::roots_real(ay, by, cy, dy - f64::from(to_intersect), &mut roots);

        let mut intersection_count = 0usize;
        for &t in &roots[..count] {
            let pinned_t = pin_t_range(t);
            if (0.0..=1.0).contains(&pinned_t) {
                intersections_storage[intersection_count] =
                    double_to_float(cubics::eval_at(ax, bx, cx, dx, pinned_t));
                intersection_count += 1;
            }
        }

        &intersections_storage[..intersection_count]
    }
}

/// Utilities for dealing with quadratic Bézier curves (`SkBezierQuad`).
#[doc(alias = "SkBezierQuad")]
#[derive(Copy, Clone, Debug)]
pub struct BezierQuad;

impl BezierQuad {
    /// Finds the X values of the quad (given by the first three of `control_points`) where its Y
    /// equals `y_intercept`.
    // Port of: src/core/SkBezierCurves.cpp#L189-L206 (chrome/m156)
    #[doc(alias = "IntersectWithHorizontalLine")]
    pub fn intersect_with_horizontal_line<'a>(
        control_points: &[Point],
        y_intercept: f32,
        intersection_storage: &'a mut [f32; 2],
    ) -> &'a [f32] {
        debug_assert!(control_points.len() >= 3);
        let p0 = DPoint::from(control_points[0]);
        let p1 = DPoint::from(control_points[1]);
        let p2 = DPoint::from(control_points[2]);

        // Calculate A, B, C using doubles to reduce round-off error.
        let a = p0 - 2.0 * p1 + p2;
        // Remember we are generating the polynomial in the form A*t^2 -2*B*t + C, so the factor
        // of 2 is not needed and the term is negated. This term for a Bézier curve is usually
        // 2(p1-p0).
        let b = p0 - p1;
        let c = p0;

        Self::intersect(
            a.x,
            b.x,
            c.x,
            a.y,
            b.y,
            c.y,
            f64::from(y_intercept),
            intersection_storage,
        )
    }

    /// Given `AY*t^2 -2*BY*t + CY = 0` and `AX*t^2 - 2*BX*t + CX = 0`, find the `t` where
    /// `AY*t^2 - 2*BY*t + CY - y = 0`, then return `AX*t^2 + - 2*BX*t + CX` where `t` is on
    /// `[0, 1]`.
    ///
    /// `y_intercept` is the height of the line which intersects the quadratic.
    // Port of: src/core/SkBezierCurves.cpp#L208-L227 (chrome/m156)
    #[doc(alias = "Intersect")]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    pub fn intersect(
        ax: f64,
        bx: f64,
        cx: f64,
        ay: f64,
        by: f64,
        cy: f64,
        y_intercept: f64,
        intersection_storage: &mut [f32; 2],
    ) -> &[f32] {
        let quads::RootResult {
            root0: r0,
            root1: r1,
            ..
        } = quads::roots(ay, by, cy - y_intercept);

        let mut intersection_count = 0usize;
        // Round the roots to the nearest float to generate the values t. Valid t's are on the
        // domain [0, 1].
        let t0 = pin_t_range(r0);
        if (0.0..=1.0).contains(&t0) {
            intersection_storage[intersection_count] =
                double_to_float(quads::eval_at(ax, -2.0 * bx, cx, t0));
            intersection_count += 1;
        }

        let t1 = pin_t_range(r1);
        #[allow(clippy::float_cmp)] // exact comparison, as in Skia
        if (0.0..=1.0).contains(&t1) && t1 != t0 {
            intersection_storage[intersection_count] =
                double_to_float(quads::eval_at(ax, -2.0 * bx, cx, t1));
            intersection_count += 1;
        }

        &intersection_storage[..intersection_count]
    }
}

// Port of: src/core/SkBezierCurves.cpp#L117-L139 (chrome/m156)
#[derive(Copy, Clone)]
struct DPoint {
    x: f64,
    y: f64,
}

impl From<Point> for DPoint {
    fn from(p: Point) -> Self {
        Self {
            x: f64::from(p.x),
            y: f64::from(p.y),
        }
    }
}

impl core::ops::Neg for DPoint {
    type Output = DPoint;
    fn neg(self) -> DPoint {
        DPoint {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl core::ops::Add for DPoint {
    type Output = DPoint;
    fn add(self, b: DPoint) -> DPoint {
        DPoint {
            x: self.x + b.x,
            y: self.y + b.y,
        }
    }
}

impl core::ops::Sub for DPoint {
    type Output = DPoint;
    fn sub(self, b: DPoint) -> DPoint {
        DPoint {
            x: self.x - b.x,
            y: self.y - b.y,
        }
    }
}

impl core::ops::Mul<DPoint> for f64 {
    type Output = DPoint;
    fn mul(self, a: DPoint) -> DPoint {
        DPoint {
            x: self * a.x,
            y: self * a.y,
        }
    }
}

// Pin to 0 or 1 if within half a float ulp of 0 or 1.
// Port of: src/core/SkBezierCurves.cpp#L140-L150 (chrome/m156)
#[allow(clippy::float_cmp)] // exact float comparisons, as in Skia
fn pin_t_range(t: f64) -> f64 {
    // The ULPs around 0 are tiny compared to the ULPs around 1. Shift to 1 to use the same
    // size ULPs.
    if double_to_float(t + 1.0) == 1.0f32 {
        0.0
    } else if double_to_float(t) == 1.0f32 {
        1.0
    } else {
        t
    }
}
