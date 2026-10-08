// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsCubicLineIntersectionTest.cpp (chrome/m156)

#![cfg(test)]

use crate::unit::path_ops_test_common::CubicPts;
use crate::{Reporter, def_test, reporter_assert};
use skia_rust_pathops::cubic::DCubic;
use skia_rust_pathops::intersections::Intersections;
use skia_rust_pathops::line::DLine;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::reduce_order::{Quadratics, ReduceOrder};

/// Port of `struct lineCubic`.
struct LineCubic {
    cubic: CubicPts,
    line: DLine,
}

/// A `SkDLine` from its end point coordinates.
const fn seg(x0: f64, y0: f64, x1: f64, y1: f64) -> DLine {
    DLine::new([DPoint::new(x0, y0), DPoint::new(x1, y1)])
}

/// `CubicPts` from four coordinate pairs.
const fn cubic(p: [(f64, f64); 4]) -> CubicPts {
    CubicPts::new([
        DPoint::new(p[0].0, p[0].1),
        DPoint::new(p[1].0, p[1].1),
        DPoint::new(p[2].0, p[2].1),
        DPoint::new(p[3].0, p[3].1),
    ])
}

// Port of: tests/PathOpsCubicLineIntersectionTest.cpp (chrome/m156)
#[allow(clippy::unreadable_literal)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
const FAIL_LINE_CUBIC_TESTS: [LineCubic; 1] = [LineCubic {
    cubic: cubic([
        (37.5273438, -1.44140625),
        (37.8736992, -1.69921875),
        (38.1640625, -2.140625),
        (38.3984375, -2.765625),
    ]),
    line: seg(40.625, -5.7890625, 37.7109375, 1.3515625),
}];

// Port of: tests/PathOpsCubicLineIntersectionTest.cpp (chrome/m156)
// (the `#if 0` entry is not ported, as in the C++)
#[allow(clippy::excessive_precision, clippy::unreadable_literal)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
const LINE_CUBIC_TESTS: [LineCubic; 14] = [
    LineCubic {
        cubic: cubic([
            (0.0, 6.0),
            (1.0851458311080933, 4.3722810745239258),
            (1.5815209150314331, 3.038947582244873),
            (1.9683018922805786, 1.9999997615814209),
        ]),
        line: seg(3.0, 2.0, 1.0, 2.0),
    },
    LineCubic {
        cubic: cubic([
            (0.468027353, 4.0),
            (1.06734705, 1.33333337),
            (1.36700678, 0.0),
            (3.0, 0.0),
        ]),
        line: seg(2.0, 1.0, 0.0, 1.0),
    },
    LineCubic {
        cubic: cubic([
            (-634.60540771484375, -481.262939453125),
            (266.2696533203125, -752.70867919921875),
            (-751.8370361328125, -317.37921142578125),
            (-969.7427978515625, 824.7255859375),
        ]),
        line: seg(
            -287.9506133720805678,
            -557.1376476615772617,
            -285.9506133720805678,
            -557.1376476615772617,
        ),
    },
    LineCubic {
        cubic: cubic([
            (36.7184372, 0.888650894),
            (36.7184372, 0.888650894),
            (35.1233864, 0.554015458),
            (34.5114098, -0.115255356),
        ]),
        line: seg(35.4531212, 0.0, 31.9375, 0.0),
    },
    LineCubic {
        cubic: cubic([
            (421.0, 378.0),
            (421.0, 380.209137_f32 as f64),
            (418.761414_f32 as f64, 382.0),
            (416.0, 382.0),
        ]),
        line: seg(320.0, 378.0, 421.0, 378.000031_f32 as f64),
    },
    LineCubic {
        cubic: cubic([
            (416.0, 383.0),
            (418.761414_f32 as f64, 383.0),
            (421.0, 380.761414_f32 as f64),
            (421.0, 378.0),
        ]),
        line: seg(320.0, 378.0, 421.0, 378.000031_f32 as f64),
    },
    LineCubic {
        cubic: cubic([
            (154.0, 715.0),
            (151.238571, 715.0),
            (149.0, 712.761414),
            (149.0, 710.0),
        ]),
        line: seg(149.0, 675.0, 149.0, 710.001465),
    },
    LineCubic {
        cubic: cubic([(0.0, 1.0), (1.0, 6.0), (4.0, 1.0), (4.0, 3.0)]),
        line: seg(6.0, 1.0, 1.0, 4.0),
    },
    LineCubic {
        cubic: cubic([(0.0, 1.0), (2.0, 6.0), (4.0, 1.0), (5.0, 4.0)]),
        line: seg(6.0, 2.0, 1.0, 4.0),
    },
    LineCubic {
        cubic: cubic([(0.0, 4.0), (3.0, 4.0), (6.0, 2.0), (5.0, 2.0)]),
        line: seg(4.0, 3.0, 2.0, 6.0),
    },
    LineCubic {
        cubic: cubic([
            (1006.6951293945312, 291.0),
            (1023.263671875, 291.0),
            (1033.8402099609375, 304.43145751953125),
            (1030.318359375, 321.0),
        ]),
        line: seg(979.30487060546875, 561.0, 1036.695068359375, 291.0),
    },
    LineCubic {
        cubic: cubic([
            (259.30487060546875, 561.0),
            (242.73631286621094, 561.0),
            (232.15980529785156, 547.56854248046875),
            (235.68154907226562, 531.0),
        ]),
        line: seg(286.69512939453125, 291.0, 229.30485534667969, 561.0),
    },
    LineCubic {
        cubic: cubic([(1.0, 2.0), (2.0, 6.0), (2.0, 0.0), (1.0, 0.0)]),
        line: seg(1.0, 0.0, 1.0, 2.0),
    },
    LineCubic {
        cubic: cubic([(0.0, 0.0), (0.0, 1.0), (0.0, 1.0), (1.0, 1.0)]),
        line: seg(0.0, 1.0, 1.0, 0.0),
    },
];

/// Port of `doIntersect` (cubic).
// Port of: tests/PathOpsCubicLineIntersectionTest.cpp#L131-L157 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparisons of the C++ test are kept as-is
fn do_intersect(intersections: &mut Intersections, cubic: &DCubic, line: &DLine) -> usize {
    if line[0].x == line[1].x {
        let mut top = line[0].y;
        let mut bottom = line[1].y;
        let flipped = top > bottom;
        if flipped {
            std::mem::swap(&mut top, &mut bottom);
        }
        intersections.vertical_cubic(cubic, top, bottom, line[0].x, flipped)
    } else if line[0].y == line[1].y {
        let mut left = line[0].x;
        let mut right = line[1].x;
        let flipped = left > right;
        if flipped {
            std::mem::swap(&mut left, &mut right);
        }
        intersections.horizontal_cubic(cubic, left, right, line[0].y, flipped)
    } else {
        intersections.intersect_cubic_line(cubic, line);
        intersections.used()
    }
}

/// Port of `testFail`.
// Port of: tests/PathOpsCubicLineIntersectionTest.cpp#L37-L61 (chrome/m156)
fn test_fail(reporter: &mut Reporter, test: &LineCubic) {
    let cubic = DCubic::new(test.cubic.pts);
    let line = &test.line;
    let mut reduce1 = ReduceOrder::default();
    let mut reduce2 = ReduceOrder::default();
    let order1 = reduce1.reduce_cubic(&cubic, Quadratics::No);
    let order2 = reduce2.reduce_line(line);
    reporter_assert!(reporter, order1 >= 4);
    reporter_assert!(reporter, order2 >= 2);
    if order1 == 4 && order2 == 2 {
        let mut i = Intersections::default();
        let roots = i.intersect_cubic_line(&cubic, line);
        reporter_assert!(reporter, roots == 0);
    }
}

/// Port of `testOne`.
// Port of: tests/PathOpsCubicLineIntersectionTest.cpp#L159-L206 (chrome/m156)
fn test_one(reporter: &mut Reporter, test: &LineCubic) {
    let cubic = DCubic::new(test.cubic.pts);
    let line = &test.line;
    let mut reduce1 = ReduceOrder::default();
    let mut reduce2 = ReduceOrder::default();
    let order1 = reduce1.reduce_cubic(&cubic, Quadratics::No);
    let order2 = reduce2.reduce_line(line);
    reporter_assert!(reporter, order1 >= 4);
    reporter_assert!(reporter, order2 >= 2);
    if order1 == 4 && order2 == 2 {
        let mut i = Intersections::default();
        let roots = do_intersect(&mut i, &cubic, line);
        for pt in 0..roots {
            let tt1 = i.t(0, pt);
            let xy1 = cubic.pt_at_t(tt1);
            let tt2 = i.t(1, pt);
            let xy2 = line.pt_at_t(tt2);
            reporter_assert!(reporter, xy1.approximately_equal(xy2));
        }
    }
}

def_test!(PathOpsFailCubicLineIntersection, |reporter| {
    for test in &FAIL_LINE_CUBIC_TESTS {
        test_fail(reporter, test);
        reporter.bump_test_count();
    }
});

def_test!(PathOpsCubicLineIntersection, |reporter| {
    for test in &LINE_CUBIC_TESTS {
        test_one(reporter, test);
        reporter.bump_test_count();
    }
});

def_test!(PathOpsCubicLineIntersectionOneOff, |reporter| {
    let test = &LINE_CUBIC_TESTS[0];
    test_one(reporter, test);
    let cubic = DCubic::new(test.cubic.pts);
    let mut i = Intersections::default();
    i.intersect_cubic_line(&cubic, &test.line);
    debug_assert_eq!(i.used(), 1);
});
