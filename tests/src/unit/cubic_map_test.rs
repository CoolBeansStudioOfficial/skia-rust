// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CubicMapTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::cubic_map::CubicMap;
use skia_rust_core::cubics;
use skia_rust_core::geometry::CubicCoeff;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::{Scalar, scalar};
use skia_rust_simd::vx::Float2;

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/CubicMapTest.cpp#L20-L25 (chrome/m156)
#[allow(clippy::manual_assert_eq)] // mirrors the C++ test
fn accurate_t(a: f32, b: f32, c: f32, d: f32) -> f32 {
    let mut roots = [0.0f64; 3];
    let count = cubics::roots_valid_t(
        f64::from(a),
        f64::from(b),
        f64::from(c),
        f64::from(d),
        &mut roots,
    );
    debug_assert_eq!(count, 1);
    #[allow(clippy::cast_possible_truncation)] // (float)roots[0]
    {
        roots[0] as f32
    }
}

// Port of: tests/CubicMapTest.cpp#L27-L37 (chrome/m156)
fn accurate_solve(p1: Point, p2: Point, x: scalar) -> f32 {
    let array = [Point::new(0.0, 0.0), p1, p2, Point::new(1.0, 1.0)];
    let coeff = CubicCoeff::new(&array);

    let t = accurate_t(coeff.a[0], coeff.b[0], coeff.c[0], coeff.d[0] - x);
    debug_assert!((0.0..=1.0).contains(&t));
    let y = coeff.eval(Float2::splat(t))[1];
    debug_assert!((0.0..=1.0001f32).contains(&y));
    y
}

// Port of: tests/CubicMapTest.cpp#L39-L41 (chrome/m156)
fn nearly_le(a: scalar, b: scalar) -> bool {
    a <= b || (a - b).nearly_zero(None)
}

// Port of: tests/CubicMapTest.cpp#L43-L63 (chrome/m156)
#[allow(clippy::neg_cmp_op_on_partial_ord)] // inside reporter_assert!'s `!(cond)`
fn exercise_cubicmap(p1: Point, p2: Point, reporter: &mut Reporter) {
    const MAX_SOLVER_ERR: scalar = 0.008; // found by running w/ current impl

    let cmap = CubicMap::new(p1, p2);

    let mut prev_y: scalar = 0.0;
    let dx: scalar = 1.0 / 512.0;
    let mut x = dx;
    while x < 1.0 {
        let y = cmap.compute_y_from_x(x);
        // are we valid and (mostly) monotonic?
        if !nearly_le(prev_y, y) {
            let _ = cmap.compute_y_from_x(x);
            reporter_assert!(reporter, false);
        }
        prev_y = y;

        // are we close to the "correct" answer?
        let yy = accurate_solve(p1, p2, x);
        let diff = (yy - y).abs();
        reporter_assert!(reporter, diff < MAX_SOLVER_ERR);
        x += dx;
    }
}

// Port of: tests/CubicMapTest.cpp#L65-L77 (chrome/m156)
def_test!(CubicMap, |r| {
    let values: [scalar; 5] = [0.0, 1.0, 0.5, 0.000_000_1, 0.999_999];

    for x0 in values {
        for y0 in values {
            for x1 in values {
                for y1 in values {
                    exercise_cubicmap(Point::new(x0, y0), Point::new(x1, y1), r);
                }
            }
        }
    }
});
