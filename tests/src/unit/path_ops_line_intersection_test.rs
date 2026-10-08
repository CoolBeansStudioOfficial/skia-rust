// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsLineIntersectionTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_pathops::intersections::Intersections;
use skia_rust_pathops::line::DLine;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::types::{std_max, std_min};

/// A `SkDLine` from its end point coordinates.
const fn seg(x0: f64, y0: f64, x1: f64, y1: f64) -> DLine {
    DLine::new([DPoint::new(x0, y0), DPoint::new(x1, y1)])
}

// Port of: tests/PathOpsLineIntersectionTest.cpp#L20-L49 (chrome/m156)
// (the `#if 0` entries are not ported, as in the C++)
#[allow(clippy::excessive_precision, clippy::unreadable_literal)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
const TESTS: [[DLine; 2]; 15] = [
    [
        seg(
            0.00010360032320022583,
            1.0172703415155411,
            0.00014114845544099808,
            1.0200891587883234,
        ),
        seg(
            0.00010259449481964111,
            1.017270140349865,
            0.00018215179443359375,
            1.022890567779541,
        ),
    ],
    [seg(30.0, 20.0, 30.0, 50.0), seg(24.0, 30.0, 36.0, 30.0)],
    [seg(323.0, 193.0, -317.0, 193.0), seg(0.0, 994.0, 0.0, 0.0)],
    [
        seg(90.0, 230.0, 160.0, 60.0),
        seg(60.0, 120.0, 260.0, 120.0),
    ],
    [
        seg(90.0, 230.0, 160.0, 60.0),
        seg(181.176468, 120.0, 135.294128, 120.0),
    ],
    [
        seg(
            181.1764678955078125_f32 as f64,
            120.0,
            186.3661956787109375_f32 as f64,
            134.7042236328125_f32 as f64,
        ),
        seg(
            175.8309783935546875_f32 as f64,
            141.5211334228515625_f32 as f64,
            187.8782806396484375_f32 as f64,
            133.7258148193359375_f32 as f64,
        ),
    ],
    [seg(192.0, 4.0, 243.0, 4.0), seg(246.0, 4.0, 189.0, 4.0)],
    [seg(246.0, 4.0, 189.0, 4.0), seg(192.0, 4.0, 243.0, 4.0)],
    [seg(5.0, 0.0, 0.0, 5.0), seg(5.0, 4.0, 1.0, 4.0)],
    [seg(0.0, 0.0, 1.0, 0.0), seg(1.0, 0.0, 0.0, 0.0)],
    [seg(0.0, 0.0, 0.0, 0.0), seg(0.0, 0.0, 1.0, 0.0)],
    [seg(0.0, 1.0, 0.0, 1.0), seg(0.0, 0.0, 0.0, 2.0)],
    [seg(0.0, 0.0, 1.0, 0.0), seg(0.0, 0.0, 2.0, 0.0)],
    [seg(1.0, 1.0, 2.0, 2.0), seg(0.0, 0.0, 3.0, 3.0)],
    [
        seg(
            166.86950047022856,
            112.69654129527828,
            166.86948801592692,
            112.69655741235339,
        ),
        seg(
            166.86960700313026,
            112.6965477747386,
            166.86925794355412,
            112.69656471103423,
        ),
    ],
];

// Port of: tests/PathOpsLineIntersectionTest.cpp (chrome/m156)
const NO_INTERSECT: [[DLine; 2]; 6] = [
    [
        seg(
            (2.0_f32 - 1e-6_f32) as f64,
            2.0,
            (2.0_f32 - 1e-6_f32) as f64,
            4.0,
        ),
        seg(2.0, 1.0, 2.0, 3.0),
    ],
    [seg(0.0, 0.0, 1.0, 0.0), seg(3.0, 0.0, 2.0, 0.0)],
    [seg(0.0, 0.0, 0.0, 0.0), seg(1.0, 0.0, 2.0, 0.0)],
    [seg(0.0, 1.0, 0.0, 1.0), seg(0.0, 3.0, 0.0, 2.0)],
    [seg(0.0, 0.0, 1.0, 0.0), seg(2.0, 0.0, 3.0, 0.0)],
    [seg(1.0, 1.0, 2.0, 2.0), seg(4.0, 4.0, 3.0, 3.0)],
];

// Port of: tests/PathOpsLineIntersectionTest.cpp (chrome/m156)
#[allow(clippy::excessive_precision, clippy::unreadable_literal)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
const COINCIDENT_TESTS: [[DLine; 2]; 8] = [
    [
        seg(-1.48383003e-006, -83.0, 4.2268899e-014, -60.0),
        seg(9.5359502e-007, -60.0, 5.08227985e-015, -83.0),
    ],
    [
        seg(10105.0, 2510.0, 10123.0, 2509.98999_f32 as f64),
        seg(10105.0, 2509.98999_f32 as f64, 10123.0, 2510.0),
    ],
    [
        seg(0.0, 482.5, -4.4408921e-016, 682.5),
        seg(0.0, 683.0, 0.0, 482.0),
    ],
    [
        seg(1.77635684e-015, 312.0, -1.24344979e-014, 348.0),
        seg(0.0, 348.0, 0.0, 312.0),
    ],
    [
        seg(979.304871, 561.0, 1036.69507, 291.0),
        seg(985.681519, 531.0, 982.159790, 547.568542),
    ],
    [
        seg(232.159805, 547.568542, 235.681549, 531.0),
        seg(286.695129, 291.0, 229.304855, 561.0),
    ],
    [
        seg(
            186.3661956787109375_f32 as f64,
            134.7042236328125_f32 as f64,
            187.8782806396484375_f32 as f64,
            133.7258148193359375_f32 as f64,
        ),
        seg(
            175.8309783935546875_f32 as f64,
            141.5211334228515625_f32 as f64,
            187.8782806396484375_f32 as f64,
            133.7258148193359375_f32 as f64,
        ),
    ],
    [
        seg(235.681549, 531.0, 280.318420, 321.0),
        seg(286.695129, 291.0, 229.304855, 561.0),
    ],
];

/// Port of `check_results`.
// Port of: tests/PathOpsLineIntersectionTest.cpp#L94-L112 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparisons of the C++ test are kept as-is
fn check_results(
    reporter: &mut Reporter,
    line1: &DLine,
    line2: &DLine,
    ts: &Intersections,
    near_allowed: bool,
) {
    for i in 0..ts.used() {
        let result1 = line1.pt_at_t(ts.t(0, i));
        let mut result2 = line2.pt_at_t(ts.t(1, i));
        if near_allowed && result1.roughly_equal(result2) {
            continue;
        }
        if !result1.approximately_equal(result2) && !ts.nearly_same(i) {
            reporter_assert!(reporter, ts.used() != 1);
            result2 = line2.pt_at_t(ts.t(1, i ^ 1));
            reporter_assert!(reporter, result1.approximately_equal(result2));
            reporter_assert!(
                reporter,
                result1.approximately_equal_sk(ts.pt(i).as_sk_point())
            );
        }
    }
}

/// Port of `testOne`.
// Port of: tests/PathOpsLineIntersectionTest.cpp#L114-L156 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparisons of the C++ test are kept as-is
fn test_one(reporter: &mut Reporter, line1: &DLine, line2: &DLine, near_allowed: bool) {
    let mut i = Intersections::default();
    i.allow_near(near_allowed);
    let pts = i.intersect_line_line(line1, line2);
    reporter_assert!(reporter, pts != 0);
    reporter_assert!(reporter, pts == i.used());
    check_results(reporter, line1, line2, &i, near_allowed);
    if line1[0] == line1[1] || line2[0] == line2[1] {
        return;
    }
    if line1[0].y == line1[1].y {
        let left = std_min(line1[0].x, line1[1].x);
        let right = std_max(line1[0].x, line1[1].x);
        let mut ts = Intersections::default();
        ts.horizontal_line(line2, left, right, line1[0].y, line1[0].x != left);
        check_results(reporter, line2, line1, &ts, near_allowed);
    }
    if line2[0].y == line2[1].y {
        let left = std_min(line2[0].x, line2[1].x);
        let right = std_max(line2[0].x, line2[1].x);
        let mut ts = Intersections::default();
        ts.horizontal_line(line1, left, right, line2[0].y, line2[0].x != left);
        check_results(reporter, line1, line2, &ts, near_allowed);
    }
    if line1[0].x == line1[1].x {
        let top = std_min(line1[0].y, line1[1].y);
        let bottom = std_max(line1[0].y, line1[1].y);
        let mut ts = Intersections::default();
        ts.vertical_line(line2, top, bottom, line1[0].x, line1[0].y != top);
        check_results(reporter, line2, line1, &ts, near_allowed);
    }
    if line2[0].x == line2[1].x {
        let top = std_min(line2[0].y, line2[1].y);
        let bottom = std_max(line2[0].y, line2[1].y);
        let mut ts = Intersections::default();
        ts.vertical_line(line1, top, bottom, line2[0].x, line2[0].y != top);
        check_results(reporter, line1, line2, &ts, near_allowed);
    }
    reporter.bump_test_count();
}

/// Port of `testOneCoincident`.
// Port of: tests/PathOpsLineIntersectionTest.cpp#L158-L207 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparisons of the C++ test are kept as-is
fn test_one_coincident(reporter: &mut Reporter, line1: &DLine, line2: &DLine) {
    let mut i = Intersections::default();
    let pts = i.intersect_line_line(line1, line2);
    reporter_assert!(reporter, pts == 2);
    reporter_assert!(reporter, pts == i.used());
    check_results(reporter, line1, line2, &i, false);
    if line1[0] == line1[1] || line2[0] == line2[1] {
        return;
    }
    if line1[0].y == line1[1].y {
        let left = std_min(line1[0].x, line1[1].x);
        let right = std_max(line1[0].x, line1[1].x);
        let mut ts = Intersections::default();
        ts.horizontal_line(line2, left, right, line1[0].y, line1[0].x != left);
        reporter_assert!(reporter, pts == 2);
        reporter_assert!(reporter, pts == ts.used());
        check_results(reporter, line2, line1, &ts, false);
    }
    if line2[0].y == line2[1].y {
        let left = std_min(line2[0].x, line2[1].x);
        let right = std_max(line2[0].x, line2[1].x);
        let mut ts = Intersections::default();
        ts.horizontal_line(line1, left, right, line2[0].y, line2[0].x != left);
        reporter_assert!(reporter, pts == 2);
        reporter_assert!(reporter, pts == ts.used());
        check_results(reporter, line1, line2, &ts, false);
    }
    if line1[0].x == line1[1].x {
        let top = std_min(line1[0].y, line1[1].y);
        let bottom = std_max(line1[0].y, line1[1].y);
        let mut ts = Intersections::default();
        ts.vertical_line(line2, top, bottom, line1[0].x, line1[0].y != top);
        reporter_assert!(reporter, pts == 2);
        reporter_assert!(reporter, pts == ts.used());
        check_results(reporter, line2, line1, &ts, false);
    }
    if line2[0].x == line2[1].x {
        let top = std_min(line2[0].y, line2[1].y);
        let bottom = std_max(line2[0].y, line2[1].y);
        let mut ts = Intersections::default();
        ts.vertical_line(line1, top, bottom, line2[0].x, line2[0].y != top);
        reporter_assert!(reporter, pts == 2);
        reporter_assert!(reporter, pts == ts.used());
        check_results(reporter, line1, line2, &ts, false);
    }
    reporter.bump_test_count();
}

def_test!(PathOpsLineIntersection, |reporter| {
    for test in &COINCIDENT_TESTS {
        test_one_coincident(reporter, &test[0], &test[1]);
    }
    for test in &TESTS {
        test_one(reporter, &test[0], &test[1], true);
    }
    for test in &NO_INTERSECT {
        let mut ts = Intersections::default();
        let pts = ts.intersect_line_line(&test[0], &test[1]);
        reporter_assert!(reporter, pts == 0);
        reporter_assert!(reporter, pts == ts.used());
        reporter.bump_test_count();
    }
});

def_test!(PathOpsLineIntersectionOneOff, |reporter| {
    let index = 0;
    test_one(reporter, &TESTS[index][0], &TESTS[index][1], true);
});

def_test!(PathOpsLineIntersectionExactOneOff, |reporter| {
    let index = 0;
    test_one(reporter, &TESTS[index][0], &TESTS[index][1], false);
});

def_test!(PathOpsLineIntersectionOneCoincident, |reporter| {
    let index = 0;
    test_one_coincident(
        reporter,
        &COINCIDENT_TESTS[index][0],
        &COINCIDENT_TESTS[index][1],
    );
});
