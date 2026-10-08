// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsThreeWayTest.cpp (chrome/m156)

// The float literals below are Skia's test inputs, copied digit for digit.
#![allow(clippy::excessive_precision, clippy::unreadable_literal)]
#![cfg(test)]

use skia_rust_pathops::cubic::DCubic;
use skia_rust_pathops::intersections::Intersections;
use skia_rust_pathops::line::DLine;
use skia_rust_pathops::point::DPoint;

use crate::def_test;
use crate::unit::path_ops_test_common::CubicPts;

/// `struct Curve`: `ptCount` is 1 for a line (the first two points), 4 for a cubic.
// Port of: tests/PathOpsThreeWayTest.cpp#L13-L16 (chrome/m156)
struct Curve {
    pt_count: i32,
    curve: CubicPts,
}

/// `struct TestSet`.
// Port of: tests/PathOpsThreeWayTest.cpp#L35-L38 (chrome/m156)
struct TestSet {
    tests: &'static [Curve],
}

// Port of: tests/PathOpsThreeWayTest.cpp#L18-L24 (chrome/m156)
// extracted from skpClip2
const TEST_SET0: &[Curve] = &[
    Curve {
        pt_count: 4,
        curve: CubicPts::new([
            DPoint::new(134., 11414.),
            DPoint::new(131.990234, 11414.),
            DPoint::new(130.32666, 11415.4824),
            DPoint::new(130.042755, 11417.4131),
        ]),
    },
    Curve {
        pt_count: 4,
        curve: CubicPts::new([
            DPoint::new(130.042755, 11417.4131),
            DPoint::new(130.233124, 11418.3193),
            DPoint::new(131.037079, 11419.),
            DPoint::new(132., 11419.),
        ]),
    },
    Curve {
        pt_count: 4,
        curve: CubicPts::new([
            DPoint::new(132., 11419.),
            DPoint::new(130.895432, 11419.),
            DPoint::new(130., 11418.1045),
            DPoint::new(130., 11417.),
        ]),
    },
];

// Port of: tests/PathOpsThreeWayTest.cpp#L25-L30 (chrome/m156)
// extracted from cubicOp85i
const TEST_SET1: &[Curve] = &[
    Curve {
        pt_count: 4,
        curve: CubicPts::new([
            DPoint::new(3., 4.),
            DPoint::new(1., 5.),
            DPoint::new(4., 3.),
            DPoint::new(6., 4.),
        ]),
    },
    Curve {
        pt_count: 1,
        curve: CubicPts::new([
            DPoint::new(6., 4.),
            DPoint::new(3., 4.),
            DPoint::new(0., 0.),
            DPoint::new(0., 0.),
        ]),
    },
    Curve {
        pt_count: 4,
        curve: CubicPts::new([
            DPoint::new(3., 4.),
            DPoint::new(4., 6.),
            DPoint::new(4., 3.),
            DPoint::new(5., 1.),
        ]),
    },
    Curve {
        pt_count: 1,
        curve: CubicPts::new([
            DPoint::new(5., 1.),
            DPoint::new(3., 4.),
            DPoint::new(0., 0.),
            DPoint::new(0., 0.),
        ]),
    },
];

/// `testSets[]`.
// Port of: tests/PathOpsThreeWayTest.cpp#L40-L43 (chrome/m156)
const TEST_SETS: [TestSet; 2] = [TestSet { tests: TEST_SET0 }, TestSet { tests: TEST_SET1 }];

// Port of: tests/PathOpsThreeWayTest.cpp#L45-L77 (chrome/m156)
/// `testSetTest(reporter, index)`: checks the intersections of every pair of curves in the set.
/// The C++ has no assertions here, only the intersection calls (its dump is commented out).
fn test_set_test(index: usize) {
    let test_set = &TEST_SETS[index];
    let test_count = test_set.tests.len();
    assert!(test_count > 1);
    for outer in 0..test_count - 1 {
        let o_test = &test_set.tests[outer];
        for inner in outer + 1..test_count {
            let i_test = &test_set.tests[inner];
            let mut i = Intersections::default();
            let o_line = DLine::new([o_test.curve.pts[0], o_test.curve.pts[1]]);
            let i_line = DLine::new([i_test.curve.pts[0], i_test.curve.pts[1]]);
            let i_curve = DCubic::new(i_test.curve.pts);
            let o_curve = DCubic::new(o_test.curve.pts);
            match (o_test.pt_count, i_test.pt_count) {
                (1, 1) => {
                    i.intersect_line_line(&o_line, &i_line);
                }
                (1, 4) => {
                    i.intersect_cubic_line(&i_curve, &o_line);
                }
                (4, 1) => {
                    i.intersect_cubic_line(&o_curve, &i_line);
                }
                (4, 4) => {
                    i.intersect_cubic_cubic(&o_curve, &i_curve);
                }
                _ => unreachable!("SkASSERT(0): curves are lines or cubics"),
            }
        }
    }
}

// Port of: tests/PathOpsThreeWayTest.cpp#L79-L86 (chrome/m156)
def_test!(PathOpsThreeWay, |reporter| {
    for index in 0..TEST_SETS.len() {
        test_set_test(index);
        reporter.bump_test_count();
    }
});

// Port of: tests/PathOpsThreeWayTest.cpp#L88-L92 (chrome/m156)
def_test!(PathOpsThreeWayOneOff, |_reporter| {
    let index = 0;
    test_set_test(index);
});
