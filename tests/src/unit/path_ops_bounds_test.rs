// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsBoundsTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{def_test, reporter_assert};
use skia_rust_core::point::Point;
use skia_rust_pathops::cubic::DCubic;
use skia_rust_pathops::curve::DCurve;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::quad::DQuad;
use skia_rust_pathops::rect::Bounds;

/// A `SkPathOpsBounds` (`SkRect` layout: left, top, right, bottom) from its edges.
const fn rect(left: f32, top: f32, right: f32, bottom: f32) -> Bounds {
    Bounds {
        left,
        top,
        right,
        bottom,
    }
}

/// `ValidBounds(const SkPathOpsBounds& bounds)`: no edge is NaN.
// Port of: tests/PathOpsTestCommon.cpp#L264-L280 (chrome/m156)
fn valid_bounds(bounds: &Bounds) -> bool {
    !bounds.left.is_nan()
        && !bounds.top.is_nan()
        && !bounds.right.is_nan()
        && !bounds.bottom.is_nan()
}

/// `static const SkRect sectTests[][2]`.
// Port of: tests/PathOpsBoundsTest.cpp#L11-L19 (chrome/m156)
const SECT_TESTS: [[Bounds; 2]; 8] = [
    [rect(2.0, 0.0, 4.0, 1.0), rect(4.0, 0.0, 6.0, 1.0)],
    [rect(2.0, 0.0, 4.0, 1.0), rect(3.0, 0.0, 5.0, 1.0)],
    [rect(2.0, 0.0, 4.0, 1.0), rect(3.0, 0.0, 5.0, 0.0)],
    [rect(2.0, 0.0, 4.0, 1.0), rect(3.0, 1.0, 5.0, 2.0)],
    [rect(2.0, 1.0, 4.0, 2.0), rect(1.0, 0.0, 5.0, 3.0)],
    [rect(2.0, 1.0, 5.0, 3.0), rect(3.0, 1.0, 4.0, 2.0)],
    [rect(2.0, 0.0, 4.0, 1.0), rect(3.0, 0.0, 3.0, 0.0)], // intersecting an empty bounds is OK
    [rect(2.0, 0.0, 4.0, 1.0), rect(4.0, 1.0, 5.0, 2.0)], // touching just on a corner is OK
];

/// `static const SkRect noSectTests[][2]`.
// Port of: tests/PathOpsBoundsTest.cpp#L21-L26 (chrome/m156)
const NO_SECT_TESTS: [[Bounds; 2]; 2] = [
    [rect(2.0, 0.0, 4.0, 1.0), rect(5.0, 0.0, 6.0, 1.0)],
    [rect(2.0, 0.0, 4.0, 1.0), rect(3.0, 2.0, 5.0, 2.0)],
];

/// `const SkPoint curvePts[]`.
const CURVE_PTS: [Point; 4] = [
    Point::new(0.0, 0.0),
    Point::new(1.0, 2.0),
    Point::new(3.0, 4.0),
    Point::new(5.0, 6.0),
];

def_test!(PathOpsBounds, |reporter| {
    for pair in &SECT_TESTS {
        let bounds1 = pair[0];
        debug_assert!(valid_bounds(&bounds1));
        let bounds2 = pair[1];
        debug_assert!(valid_bounds(&bounds2));
        let touches = Bounds::intersects(&bounds1, &bounds2);
        reporter_assert!(reporter, touches);
    }
    for pair in &NO_SECT_TESTS {
        let bounds1 = pair[0];
        debug_assert!(valid_bounds(&bounds1));
        let bounds2 = pair[1];
        debug_assert!(valid_bounds(&bounds2));
        let touches = Bounds::intersects(&bounds1, &bounds2);
        reporter_assert!(reporter, !touches);
    }
    // bounds.setEmpty()
    let mut bounds = Bounds::default();
    bounds.add_edges(1.0, 2.0, 3.0, 4.0);
    // expected.setLTRB(0, 0, 3, 4)
    let mut expected = rect(0.0, 0.0, 3.0, 4.0);
    reporter_assert!(reporter, bounds == expected);
    bounds = Bounds::default();
    let ordinal = rect(1.0, 2.0, 3.0, 4.0);
    bounds.add_bounds(&ordinal);
    reporter_assert!(reporter, bounds == expected);
    bounds = Bounds::default();
    let bot_right = DPoint::new(3.0, 4.0);
    bounds.add_dpoint(bot_right);
    reporter_assert!(reporter, bounds == expected);
    // SkDCurve curve; curve.fQuad.set(curvePts); curve.setQuadBounds(curvePts, 1, 0, 1, &bounds);
    let mut quad = DQuad::default();
    quad.set([CURVE_PTS[0], CURVE_PTS[1], CURVE_PTS[2]]);
    let curve = DCurve::Quad(quad);
    curve.set_quad_bounds(
        [CURVE_PTS[0], CURVE_PTS[1], CURVE_PTS[2]],
        1.0,
        0.0,
        1.0,
        &mut bounds,
    );
    expected = rect(0.0, 0.0, 3.0, 4.0);
    reporter_assert!(reporter, bounds == expected);
    // curve.fCubic.set(curvePts); curve.setCubicBounds(curvePts, 1, 0, 1, &bounds);
    let mut cubic = DCubic::default();
    cubic.set(CURVE_PTS);
    let curve = DCurve::Cubic(cubic);
    curve.set_cubic_bounds(CURVE_PTS, 1.0, 0.0, 1.0, &mut bounds);
    expected = rect(0.0, 0.0, 5.0, 6.0);
    reporter_assert!(reporter, bounds == expected);
});
