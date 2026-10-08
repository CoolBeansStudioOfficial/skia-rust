// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsConicLineIntersectionTest.cpp (chrome/m156)

#![cfg(test)]

use crate::unit::path_ops_test_common::QuadPts;
use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::path::Verb;
use skia_rust_core::point::Point;
use skia_rust_pathops::conic::DConic;
use skia_rust_pathops::intersections::Intersections;
use skia_rust_pathops::line::DLine;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::quad::DQuad;
use skia_rust_pathops::reduce_order::ReduceOrder;

/// Port of `struct ConicPts`: the three control points and the weight.
#[derive(Copy, Clone, Debug)]
struct ConicPts {
    pts: QuadPts,
    weight: f32,
}

/// Port of `struct lineConic`.
struct LineConic {
    conic: ConicPts,
    line: DLine,
    result: usize,
    expected: [DPoint; 2],
}

/// Port of `struct oneLineConic`.
struct OneLineConic {
    conic: ConicPts,
    line: DLine,
}

/// A `SkDLine` from its end point coordinates.
const fn seg(x0: f64, y0: f64, x1: f64, y1: f64) -> DLine {
    DLine::new([DPoint::new(x0, y0), DPoint::new(x1, y1)])
}

// Port of: tests/PathOpsConicLineIntersectionTest.cpp (chrome/m156)
#[allow(clippy::excessive_precision, clippy::unreadable_literal)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
const LINE_CONIC_TESTS: [LineConic; 1] = [LineConic {
    conic: ConicPts {
        pts: QuadPts::new([
            DPoint::new(30.6499996, 25.6499996),
            DPoint::new(30.6499996, 20.6499996),
            DPoint::new(25.6499996, 20.6499996),
        ]),
        weight: 0.707107008,
    },
    line: seg(25.6499996, 20.6499996, 45.6500015, 20.6499996),
    result: 1,
    expected: [DPoint::new(25.6499996, 20.6499996), DPoint::new(0.0, 0.0)],
}];

/// Port of `doIntersect` (conic).
// Port of: tests/PathOpsConicLineIntersectionTest.cpp#L42-L69 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparisons of the C++ test are kept as-is
fn do_intersect(
    intersections: &mut Intersections,
    conic: &DConic,
    line: &DLine,
    flipped: &mut bool,
) -> usize {
    *flipped = false;
    if line[0].x == line[1].x {
        let mut top = line[0].y;
        let mut bottom = line[1].y;
        *flipped = top > bottom;
        if *flipped {
            std::mem::swap(&mut top, &mut bottom);
        }
        intersections.vertical_conic(conic, top, bottom, line[0].x, *flipped)
    } else if line[0].y == line[1].y {
        let mut left = line[0].x;
        let mut right = line[1].x;
        *flipped = left > right;
        if *flipped {
            std::mem::swap(&mut left, &mut right);
        }
        intersections.horizontal_conic(conic, left, right, line[0].y, *flipped)
    } else {
        intersections.intersect_conic_line(conic, line);
        intersections.used()
    }
}

/// `DConic conic; conic.debugSet(c.fPts.fPts, c.fWeight)`.
fn dconic_of(c: &ConicPts) -> DConic {
    DConic::new(DQuad::new(c.pts.pts), c.weight)
}

// Port of: tests/PathOpsConicLineIntersectionTest.cpp (chrome/m156)
#[allow(clippy::excessive_precision, clippy::unreadable_literal)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
const ONE_OFFS: [OneLineConic; 1] = [OneLineConic {
    conic: ConicPts {
        pts: QuadPts::new([
            DPoint::new(30.6499996, 25.6499996),
            DPoint::new(30.6499996, 20.6499996),
            DPoint::new(25.6499996, 20.6499996),
        ]),
        weight: 0.707107008,
    },
    line: seg(25.6499996, 20.6499996, 45.6500015, 20.6499996),
}];

/// Port of `testOneOffs`.
// Port of: tests/PathOpsConicLineIntersectionTest.cpp#L85-L107 (chrome/m156)
fn test_one_offs(reporter: &mut Reporter) {
    let mut flipped = false;
    for one_off in &ONE_OFFS {
        let conic = dconic_of(&one_off.conic);
        let line = &one_off.line;
        let mut intersections = Intersections::default();
        let result = do_intersect(&mut intersections, &conic, line, &mut flipped);
        for inner in 0..result {
            let conic_t = intersections.t(0, inner);
            let conic_xy = conic.pt_at_t(conic_t);
            let line_t = intersections.t(1, inner);
            let line_xy = line.pt_at_t(line_t);
            reporter_assert!(reporter, conic_xy.approximately_equal(line_xy));
        }
    }
}

def_test!(PathOpsConicLineIntersectionOneOff, |reporter| {
    test_one_offs(reporter);
});

def_test!(PathOpsConicLineIntersection, |reporter| {
    for test in &LINE_CONIC_TESTS {
        let conic = dconic_of(&test.conic);
        let line = &test.line;
        let pts: [Point; 3] = [
            conic.pts[0].as_sk_point(),
            conic.pts[1].as_sk_point(),
            conic.pts[2].as_sk_point(),
        ];
        let mut reduced = [Point::new(0.0, 0.0); 3];
        let reduced_verb = ReduceOrder::conic_verb(pts, test.conic.weight, &mut reduced);
        reporter_assert!(reporter, reduced_verb == Verb::Conic);
        let mut line_reducer = ReduceOrder::default();
        let line_order = line_reducer.reduce_line(line);
        reporter_assert!(reporter, line_order >= 2);
        let mut intersections = Intersections::default();
        let mut flipped = false;
        let result = do_intersect(&mut intersections, &conic, line, &mut flipped);
        reporter_assert!(reporter, result == test.result);
        if intersections.used() == 0 {
            continue;
        }
        for pt in 0..result {
            let tt1 = intersections.t(0, pt);
            reporter_assert!(reporter, (0.0..=1.0).contains(&tt1));
            let t1 = conic.pt_at_t(tt1);
            let tt2 = intersections.t(1, pt);
            reporter_assert!(reporter, (0.0..=1.0).contains(&tt2));
            let t2 = line.pt_at_t(tt2);
            reporter_assert!(reporter, t1.approximately_equal(t2));
            reporter_assert!(
                reporter,
                t1.approximately_equal(test.expected[0])
                    || (test.result != 1 && t1.approximately_equal(test.expected[1]))
            );
        }
    }
});
