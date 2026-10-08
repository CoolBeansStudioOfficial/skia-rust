// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsQuadLineIntersectionTest.cpp (chrome/m156)

#![cfg(test)]

use crate::unit::path_ops_test_common::QuadPts;
use crate::{Reporter, def_test, reporter_assert};
use skia_rust_pathops::intersections::Intersections;
use skia_rust_pathops::line::DLine;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::quad::DQuad;
use skia_rust_pathops::reduce_order::ReduceOrder;

/// Port of `struct lineQuad`.
struct LineQuad {
    quad: QuadPts,
    line: DLine,
    result: usize,
    expected: [DPoint; 2],
}

/// Port of `struct oneLineQuad`.
struct OneLineQuad {
    quad: QuadPts,
    line: DLine,
}

/// A `SkDLine` from its end point coordinates.
const fn seg(x0: f64, y0: f64, x1: f64, y1: f64) -> DLine {
    DLine::new([DPoint::new(x0, y0), DPoint::new(x1, y1)])
}

// Port of: tests/PathOpsQuadLineIntersectionTest.cpp (chrome/m156)
const LINE_QUAD_TESTS: [LineQuad; 5] = [
    LineQuad {
        quad: QuadPts::new([
            DPoint::new(1.0, 1.0),
            DPoint::new(2.0, 1.0),
            DPoint::new(0.0, 2.0),
        ]),
        line: seg(0.0, 0.0, 1.0, 1.0),
        result: 1,
        expected: [DPoint::new(1.0, 1.0), DPoint::new(0.0, 0.0)],
    },
    LineQuad {
        quad: QuadPts::new([
            DPoint::new(0.0, 0.0),
            DPoint::new(1.0, 1.0),
            DPoint::new(3.0, 1.0),
        ]),
        line: seg(0.0, 0.0, 3.0, 1.0),
        result: 2,
        expected: [DPoint::new(0.0, 0.0), DPoint::new(3.0, 1.0)],
    },
    LineQuad {
        quad: QuadPts::new([
            DPoint::new(2.0, 0.0),
            DPoint::new(1.0, 1.0),
            DPoint::new(2.0, 2.0),
        ]),
        line: seg(0.0, 0.0, 0.0, 2.0),
        result: 0,
        expected: [DPoint::new(0.0, 0.0), DPoint::new(0.0, 0.0)],
    },
    LineQuad {
        quad: QuadPts::new([
            DPoint::new(4.0, 0.0),
            DPoint::new(0.0, 1.0),
            DPoint::new(4.0, 2.0),
        ]),
        line: seg(3.0, 1.0, 4.0, 1.0),
        result: 0,
        expected: [DPoint::new(0.0, 0.0), DPoint::new(0.0, 0.0)],
    },
    LineQuad {
        quad: QuadPts::new([
            DPoint::new(0.0, 0.0),
            DPoint::new(0.0, 1.0),
            DPoint::new(1.0, 1.0),
        ]),
        line: seg(0.0, 1.0, 1.0, 0.0),
        result: 1,
        expected: [DPoint::new(0.25, 0.75), DPoint::new(0.0, 0.0)],
    },
];

/// Port of `doIntersect`.
// Port of: tests/PathOpsQuadLineIntersectionTest.cpp#L38-L65 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparisons of the C++ test are kept as-is
fn do_intersect(
    intersections: &mut Intersections,
    quad: &DQuad,
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
        intersections.vertical_quad(quad, top, bottom, line[0].x, *flipped)
    } else if line[0].y == line[1].y {
        let mut left = line[0].x;
        let mut right = line[1].x;
        *flipped = left > right;
        if *flipped {
            std::mem::swap(&mut left, &mut right);
        }
        intersections.horizontal_quad(quad, left, right, line[0].y, *flipped)
    } else {
        intersections.intersect_quad_line(quad, line);
        intersections.used()
    }
}

// Port of: tests/PathOpsQuadLineIntersectionTest.cpp (chrome/m156)
#[allow(clippy::excessive_precision, clippy::unreadable_literal)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
const ONE_OFFS: [OneLineQuad; 6] = [
    OneLineQuad {
        quad: QuadPts::new([
            DPoint::new(97.9337616, 100.0),
            DPoint::new(88.0, 112.94265),
            DPoint::new(88.0, 130.0),
        ]),
        line: seg(88.919838, 120.0, 107.058823, 120.0),
    },
    OneLineQuad {
        quad: QuadPts::new([
            DPoint::new(447.96701049804687, 894.4381103515625),
            DPoint::new(448.007080078125, 894.4239501953125),
            DPoint::new(448.0140380859375, 894.4215087890625),
        ]),
        line: seg(
            490.43548583984375,
            879.40740966796875,
            405.59262084960937,
            909.435546875,
        ),
    },
    OneLineQuad {
        quad: QuadPts::new([
            DPoint::new(142.589081, 102.283646),
            DPoint::new(149.821579, 100.0),
            DPoint::new(158.0, 100.0),
        ]),
        line: seg(90.0, 230.0, 160.0, 60.0),
    },
    OneLineQuad {
        quad: QuadPts::new([
            DPoint::new(1101.0, 10.0),
            DPoint::new(1101.0, 8.3431453704833984),
            DPoint::new(1099.828857421875, 7.1711997985839844),
        ]),
        line: seg(
            1099.828857421875,
            7.1711711883544922,
            1099.121337890625,
            7.8786783218383789,
        ),
    },
    OneLineQuad {
        quad: QuadPts::new([
            DPoint::new(973.0, 507.0),
            DPoint::new(973.0, 508.24264526367187),
            DPoint::new(972.12158203125, 509.12161254882812),
        ]),
        line: seg(930.0, 467.0, 973.0, 510.0),
    },
    OneLineQuad {
        quad: QuadPts::new([
            DPoint::new(369.848602, 145.680267),
            DPoint::new(382.360413, 121.298294),
            DPoint::new(406.207703, 121.298294),
        ]),
        line: seg(406.207703, 121.298294, 348.781738, 123.864815),
    },
];

/// Port of `testOneOffs`.
// Port of: tests/PathOpsQuadLineIntersectionTest.cpp#L95-L117 (chrome/m156)
fn test_one_offs(reporter: &mut Reporter) {
    let mut flipped = false;
    for one_off in &ONE_OFFS {
        let quad = DQuad::new(one_off.quad.pts);
        let line = &one_off.line;
        let mut intersections = Intersections::default();
        let result = do_intersect(&mut intersections, &quad, line, &mut flipped);
        for inner in 0..result {
            let quad_t = intersections.t(0, inner);
            let quad_xy = quad.pt_at_t(quad_t);
            let line_t = intersections.t(1, inner);
            let line_xy = line.pt_at_t(line_t);
            reporter_assert!(reporter, quad_xy.approximately_equal(line_xy));
        }
    }
}

def_test!(PathOpsQuadLineIntersectionOneOff, |reporter| {
    test_one_offs(reporter);
});

def_test!(PathOpsQuadLineIntersection, |reporter| {
    for test in &LINE_QUAD_TESTS {
        let quad = DQuad::new(test.quad.pts);
        let line = &test.line;
        let mut reducer1 = ReduceOrder::default();
        let mut reducer2 = ReduceOrder::default();
        let order1 = reducer1.reduce_quad(&quad);
        let order2 = reducer2.reduce_line(line);
        reporter_assert!(reporter, order1 >= 3);
        reporter_assert!(reporter, order2 >= 2);
        let mut intersections = Intersections::default();
        let mut flipped = false;
        let result = do_intersect(&mut intersections, &quad, line, &mut flipped);
        reporter_assert!(reporter, result == test.result);
        if intersections.used() == 0 {
            continue;
        }
        for pt in 0..result {
            let tt1 = intersections.t(0, pt);
            reporter_assert!(reporter, (0.0..=1.0).contains(&tt1));
            let t1 = quad.pt_at_t(tt1);
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
