// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkCubicMap.h, src/core/SkCubicMap.cpp

//! `SkCubicMap`: fast evaluation of a cubic ease-in / ease-out curve.

use skia_rust_simd::vx::Float2;

use crate::point::Point;
use crate::scalar::{Scalar, scalar, scalar_pow};
use crate::t_pin::t_pin;

// Port of: src/core/SkCubicMap.cpp#L18-L26 (chrome/m156)
// eval_poly(t, m, b, rest...) is eval_poly(t, fma(m, t, b), rest...); these are the arities used.
fn eval_poly2(t: f32, m: f32, b: f32) -> f32 {
    m.mul_add(t, b)
}

fn eval_poly3(t: f32, m: f32, b: f32, c: f32) -> f32 {
    eval_poly2(t, m.mul_add(t, b), c)
}

#[allow(clippy::many_single_char_names)] // names follow the C++
fn eval_poly4(t: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
    eval_poly3(t, a.mul_add(t, b), c, d)
}

const MAX_ITERS: i32 = 8;

// Port of: src/core/SkCubicMap.cpp#L28-L57 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
fn cubic_solver(a: f32, b: f32, c: f32, d: f32) -> f32 {
    let valid = |t: f32| (0.0..=1.0).contains(&t);

    let guess_nice_cubic_root = |_a: f32, _b: f32, _c: f32, d: f32| -d;
    let mut t = guess_nice_cubic_root(a, b, c, d);

    for _ in 0..MAX_ITERS {
        debug_assert!(valid(t));
        let f = eval_poly4(t, a, b, c, d); // f   = At^3 + Bt^2 + Ct + D
        if f.abs() <= 0.00005f32 {
            break;
        }
        let fp = eval_poly3(t, 3.0 * a, 2.0 * b, c); // f'  = 3At^2 + 2Bt + C
        let fpp = eval_poly2(t, 3.0 * a + 3.0 * a, 2.0 * b); // f'' = 6At + 2B

        let numer = 2.0 * fp * f;
        let denom = (2.0 * fp).mul_add(fp, -(f * fpp));

        t -= numer / denom;
    }

    debug_assert!(valid(t));
    t
}

// Port of: src/core/SkCubicMap.cpp#L59-L62 (chrome/m156)
fn nearly_zero(x: scalar) -> bool {
    debug_assert!(x >= 0.0);
    x <= 0.000_000_000_1f32
}

// Port of: src/core/SkCubicMap.cpp#L64-L66 (chrome/m156)
fn compute_t_from_x(a: f32, b: f32, c: f32, x: f32) -> f32 {
    cubic_solver(a, b, c, -x)
}

// Port of: src/core/SkCubicMap.cpp#L96-L98 (chrome/m156)
fn coeff_nearly_zero(delta: f32) -> bool {
    delta.abs() <= 0.000_000_1f32
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Type {
    /// `kLine_Type`: x == y.
    Line,
    /// `kCubeRoot_Type`: At^3 == x.
    CubeRoot,
    /// `kSolver_Type`: general monotonic cubic solver.
    Solver,
}

/// Fast evaluation of a cubic ease-in / ease-out curve. This is defined as a parametric cubic
/// curve inside the unit square.
///
/// `pt[0]` is implicitly `{ 0, 0 }`, `pt[3]` is implicitly `{ 1, 1 }`, `pts[1,2].X` are inside
/// the unit `[0..1]`.
// Port of: include/core/SkCubicMap.h#L22-L45 (chrome/m156)
#[doc(alias = "SkCubicMap")]
#[derive(Copy, Clone, Debug)]
pub struct CubicMap {
    coeff: [Point; 3],
    type_: Type,
}

impl CubicMap {
    /// Creates the map from the two control points.
    // Port of: src/core/SkCubicMap.cpp#L100-L117 (chrome/m156)
    #[must_use]
    pub fn new(p1: impl Into<Point>, p2: impl Into<Point>) -> Self {
        let mut p1 = p1.into();
        let mut p2 = p2.into();
        // Clamp X values only (we allow Ys outside [0..1]).
        // std::min(std::max(x, 0.0f), 1.0f): std::max(a, b) is `(a < b) ? b : a` and
        // std::min(a, b) is `(b < a) ? b : a`.
        let std_max = |a: f32, b: f32| if a < b { b } else { a };
        let std_min = |a: f32, b: f32| if b < a { b } else { a };
        p1.x = std_min(std_max(p1.x, 0.0), 1.0);
        p2.x = std_min(std_max(p2.x, 0.0), 1.0);

        let s1 = Float2::from_list(&[p1.x, p1.y]) * 3.0;
        let s2 = Float2::from_list(&[p2.x, p2.y]) * 3.0;

        let c0 = 1.0 + s1 - s2;
        let c1 = s2 - s1 - s1;
        let coeff = [
            Point::new(c0[0], c0[1]),
            Point::new(c1[0], c1[1]),
            Point::new(s1[0], s1[1]),
        ];

        let mut type_ = Type::Solver;
        if scalar::nearly_equal(p1.x, p1.y, None) && scalar::nearly_equal(p2.x, p2.y, None) {
            type_ = Type::Line;
        } else if coeff_nearly_zero(coeff[1].x) && coeff_nearly_zero(coeff[2].x) {
            type_ = Type::CubeRoot;
        }
        Self { coeff, type_ }
    }

    /// True if the map is the identity (`SkCubicMap::IsLinear`).
    // Port of: include/core/SkCubicMap.h#L30-L32 (chrome/m156)
    #[doc(alias = "IsLinear")]
    #[must_use]
    pub fn is_linear(p1: impl Into<Point>, p2: impl Into<Point>) -> bool {
        let p1 = p1.into();
        let p2 = p2.into();
        scalar::nearly_equal(p1.x, p1.y, None) && scalar::nearly_equal(p2.x, p2.y, None)
    }

    /// Evaluates the map's Y for the given X.
    // Port of: src/core/SkCubicMap.cpp#L68-L94 (chrome/m156)
    #[doc(alias = "computeYFromX")]
    #[must_use]
    #[allow(clippy::many_single_char_names)] // names follow the C++
    pub fn compute_y_from_x(&self, x: f32) -> f32 {
        let x = t_pin(x, 0.0f32, 1.0f32);

        if nearly_zero(x) || nearly_zero(1.0 - x) {
            return x;
        }
        if self.type_ == Type::Line {
            return x;
        }
        let t = if self.type_ == Type::CubeRoot {
            scalar_pow(x / self.coeff[0].x, 1.0f32 / 3.0)
        } else {
            compute_t_from_x(self.coeff[0].x, self.coeff[1].x, self.coeff[2].x, x)
        };
        let a = self.coeff[0].y;
        let b = self.coeff[1].y;
        let c = self.coeff[2].y;
        ((a * t + b) * t + c) * t
    }

    /// Evaluates the curve's point for the parameter `t`.
    // Port of: src/core/SkCubicMap.cpp#L119-L127 (chrome/m156)
    #[doc(alias = "computeFromT")]
    #[must_use]
    pub fn compute_from_t(&self, t: f32) -> Point {
        let a = Float2::from_list(&[self.coeff[0].x, self.coeff[0].y]);
        let b = Float2::from_list(&[self.coeff[1].x, self.coeff[1].y]);
        let c = Float2::from_list(&[self.coeff[2].x, self.coeff[2].y]);

        let result = ((a * t + b) * t + c) * t;
        Point::new(result[0], result[1])
    }
}
