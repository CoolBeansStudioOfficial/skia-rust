// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/OffsetSimplePolyTest.cpp (chrome/m156)

#![cfg(test)]
// The float literals are copied verbatim from the C++ test, digits and all.
#![allow(clippy::excessive_precision, clippy::unreadable_literal)]

use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::utils::poly_utils::{
    is_convex_polygon, is_simple_polygon, offset_simple_polygon,
};

use crate::{def_test, reporter_assert};

// Port of: tests/OffsetSimplePolyTest.cpp#L15-L204 (chrome/m156)
def_test!(OffsetSimplePoly, |reporter| {
    let rrect_poly: Vec<Point> = vec![
        ///////////////////////////////////////////////////////////////////////
        // Try convex tests first
        // round rect
        Point::new(-100.0, 55.0),
        Point::new(100.0, 55.0),
        Point::new(100.0 + 2.5, 50.0 + 4.330127),
        Point::new(100.0 + 3.535534, 50.0 + 3.535534),
        Point::new(100.0 + 4.330127, 50.0 + 2.5),
        Point::new(105.0, 50.0),
        Point::new(105.0, -50.0),
        Point::new(100.0 + 4.330127, -50.0 - 2.5),
        Point::new(100.0 + 3.535534, -50.0 - 3.535534),
        Point::new(100.0 + 2.5, -50.0 - 4.330127),
        Point::new(100.0, -55.0),
        Point::new(-100.0, -55.0),
        Point::new(-100.0 - 2.5, -50.0 - 4.330127),
        Point::new(-100.0 - 3.535534, -50.0 - 3.535534),
        Point::new(-100.0 - 4.330127, -50.0 - 2.5),
        Point::new(-105.0, -50.0),
        Point::new(-105.0, 50.0),
        Point::new(-100.0 - 4.330127, 50.0 + 2.5),
        Point::new(-100.0 - 3.535534, 50.0 + 3.535534),
        Point::new(-100.0 - 2.5, 50.0 + 4.330127),
    ];
    let mut bounds = Rect::from_xywh(0.0, 0.0, 0.0, 0.0);
    bounds.set_bounds_check(&rrect_poly);
    reporter_assert!(reporter, is_convex_polygon(&rrect_poly));

    // inset a little
    let mut offset_poly: Vec<Point> = Vec::new();
    let result = offset_simple_polygon(&rrect_poly, &bounds, 3.0, &mut offset_poly, None);
    reporter_assert!(reporter, result);
    reporter_assert!(reporter, is_convex_polygon(&offset_poly));

    // inset to rect
    let result = offset_simple_polygon(&rrect_poly, &bounds, 10.0, &mut offset_poly, None);
    reporter_assert!(reporter, result);
    reporter_assert!(reporter, is_convex_polygon(&offset_poly));
    reporter_assert!(reporter, offset_poly.len() == 4);
    if offset_poly.len() == 4 {
        reporter_assert!(reporter, offset_poly[0] == Point::new(-95.0, 45.0));
        reporter_assert!(reporter, offset_poly[1] == Point::new(95.0, 45.0));
        reporter_assert!(reporter, offset_poly[2] == Point::new(95.0, -45.0));
        reporter_assert!(reporter, offset_poly[3] == Point::new(-95.0, -45.0));
    }

    // just to full inset
    // fails, but outputs a line segment
    let result = offset_simple_polygon(&rrect_poly, &bounds, 55.0, &mut offset_poly, None);
    reporter_assert!(reporter, !result);
    reporter_assert!(reporter, !is_convex_polygon(&offset_poly));
    reporter_assert!(reporter, offset_poly.len() == 2);
    if offset_poly.len() == 2 {
        reporter_assert!(reporter, offset_poly[0] == Point::new(-50.0, 0.0));
        reporter_assert!(reporter, offset_poly[1] == Point::new(50.0, 0.0));
    }

    // past full inset
    let result = offset_simple_polygon(&rrect_poly, &bounds, 75.0, &mut offset_poly, None);
    reporter_assert!(reporter, !result);

    // troublesome case
    let clipped_rrect_poly: Vec<Point> = vec![
        Point::new(335.928101, 428.219055),
        Point::new(330.414459, 423.034912),
        Point::new(325.749084, 417.395508),
        Point::new(321.931946, 411.300842),
        Point::new(318.963074, 404.750977),
        Point::new(316.842468, 397.745850),
        Point::new(315.570068, 390.285522),
        Point::new(315.145966, 382.369965),
        Point::new(315.570068, 374.454346),
        Point::new(316.842468, 366.994019),
        Point::new(318.963074, 359.988892),
        Point::new(321.931946, 353.439056),
        Point::new(325.749084, 347.344421),
        Point::new(330.414459, 341.705017),
        Point::new(335.928101, 336.520813),
        Point::new(342.289948, 331.791901),
        Point::new(377.312134, 331.791901),
        Point::new(381.195313, 332.532593),
        Point::new(384.464935, 334.754700),
        Point::new(386.687042, 338.024292),
        Point::new(387.427765, 341.907532),
        Point::new(387.427765, 422.832367),
        Point::new(386.687042, 426.715576),
        Point::new(384.464935, 429.985168),
        Point::new(381.195313, 432.207275),
        Point::new(377.312134, 432.947998),
        Point::new(342.289948, 432.947998),
    ];
    bounds.set_bounds_check(&clipped_rrect_poly);
    reporter_assert!(reporter, is_convex_polygon(&clipped_rrect_poly));
    let result = offset_simple_polygon(
        &clipped_rrect_poly,
        &bounds,
        32.369_941_7,
        &mut offset_poly,
        None,
    );
    reporter_assert!(reporter, result);
    reporter_assert!(reporter, is_convex_polygon(&offset_poly));

    ////////////////////////////////////////////////////////////////////////////////
    // Concave tests
    let star_poly: Vec<Point> = vec![
        Point::new(0.0, -50.0),
        Point::new(14.43, -25.0),
        Point::new(43.30, -25.0),
        Point::new(28.86, 0.0),
        Point::new(43.30, 25.0),
        Point::new(14.43, 25.0),
        Point::new(0.0, 50.0),
        Point::new(-14.43, 25.0),
        Point::new(-43.30, 25.0),
        Point::new(-28.86, 0.0),
        Point::new(-43.30, -25.0),
        Point::new(-14.43, -25.0),
    ];
    bounds.set_bounds_check(&star_poly);
    reporter_assert!(reporter, is_simple_polygon(&star_poly));

    // try a variety of distances
    let result = offset_simple_polygon(&star_poly, &bounds, 0.1, &mut offset_poly, None);
    reporter_assert!(reporter, result);
    reporter_assert!(reporter, is_simple_polygon(&offset_poly));

    let result = offset_simple_polygon(&star_poly, &bounds, 5.665, &mut offset_poly, None);
    reporter_assert!(reporter, result);
    reporter_assert!(reporter, is_simple_polygon(&offset_poly));

    let result = offset_simple_polygon(&star_poly, &bounds, 28.0, &mut offset_poly, None);
    reporter_assert!(reporter, result);
    reporter_assert!(reporter, is_simple_polygon(&offset_poly));

    // down to a point
    let result = offset_simple_polygon(&star_poly, &bounds, 28.866, &mut offset_poly, None);
    reporter_assert!(reporter, !result);

    // and past
    let result = offset_simple_polygon(&star_poly, &bounds, 50.5, &mut offset_poly, None);
    reporter_assert!(reporter, !result);

    // and now out
    let result = offset_simple_polygon(&star_poly, &bounds, -0.1, &mut offset_poly, None);
    reporter_assert!(reporter, result);
    reporter_assert!(reporter, is_simple_polygon(&offset_poly));

    let result = offset_simple_polygon(&star_poly, &bounds, -5.6665, &mut offset_poly, None);
    reporter_assert!(reporter, result);
    reporter_assert!(reporter, is_simple_polygon(&offset_poly));

    let result = offset_simple_polygon(&star_poly, &bounds, -50.0, &mut offset_poly, None);
    reporter_assert!(reporter, result);
    reporter_assert!(reporter, is_simple_polygon(&offset_poly));

    let intersecting_poly: Vec<Point> = vec![
        Point::new(0.0, -50.0),
        Point::new(14.43, -25.0),
        Point::new(43.30, -25.0),
        Point::new(-28.86, 0.0),
        Point::new(43.30, 25.0),
        Point::new(14.43, 25.0),
        Point::new(0.0, 50.0),
        Point::new(-14.43, 25.0),
        Point::new(-43.30, 25.0),
        Point::new(28.86, 0.0),
        Point::new(-43.30, -25.0),
        Point::new(-14.43, -25.0),
    ];
    // SkOffsetSimplePolygon now assumes that the input is simple, so we'll just check for that
    let result = is_simple_polygon(&intersecting_poly);
    reporter_assert!(reporter, !result);
});
