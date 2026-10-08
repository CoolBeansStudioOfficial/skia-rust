// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsConicIntersectionTest.cpp (chrome/m156)

#![cfg(test)]

// The `DEBUG_VISUALIZE_CONICS` image writers (`writePng`, `writeDPng`, `writeFrames`) are not
// ported: they only write files and are compiled out (`DEBUG_VISUALIZE_CONICS` is 0).

use crate::unit::path_ops_test_common::{ConicPts, QuadPts, valid_conic};
// `QuadPts` is named by the table literals below, through `ConicPts::new(QuadPts::new(..), ..)`.
use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::geometry::Conic;
use skia_rust_pathops::conic::DConic;
use skia_rust_pathops::intersections::Intersections;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::quad::DQuad;
use skia_rust_pathops::types::roughly_equal;

/// `static constexpr auto testSet`.
// Port of: tests/PathOpsConicIntersectionTest.cpp#L28-L35 (chrome/m156)
#[allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::approx_constant
)] // the table keeps the C++ literals verbatim
static TEST_SET: &[ConicPts] = &[
    ConicPts::new(
        QuadPts::new([
            DPoint::new(306.588013, -227.983994),
            DPoint::new(212.464996, -262.242004),
            DPoint::new(95.5512009, 58.9763985),
        ]),
        0.707107008_f32,
    ),
    ConicPts::new(
        QuadPts::new([
            DPoint::new(377.218994, -141.981003),
            DPoint::new(40.578701, -201.339996),
            DPoint::new(23.1854992, -102.697998),
        ]),
        0.707107008_f32,
    ),
    ConicPts::new(
        QuadPts::new([
            DPoint::new(5.1114602088928223, 628.77813720703125),
            DPoint::new(10.834027290344238, 988.964111328125),
            DPoint::new(163.40835571289062, 988.964111328125),
        ]),
        0.72944212_f32,
    ),
    ConicPts::new(
        QuadPts::new([
            DPoint::new(163.40835571289062, 988.964111328125),
            DPoint::new(5.0, 988.964111328125),
            DPoint::new(5.0, 614.7423095703125),
        ]),
        0.707106769_f32,
    ),
    ConicPts::new(
        QuadPts::new([
            DPoint::new(11.17222976684570312, -8.103978157043457031),
            DPoint::new(22.91432571411132812, -10.37866020202636719),
            DPoint::new(23.7764129638671875, -7.725424289703369141),
        ]),
        1.00862849_f32,
    ),
    ConicPts::new(
        QuadPts::new([
            DPoint::new(-1.545085430145263672, -4.755282402038574219),
            DPoint::new(22.23132705688476562, -12.48070907592773438),
            DPoint::new(23.7764129638671875, -7.725427150726318359),
        ]),
        0.707106769_f32,
    ),
    ConicPts::new(
        QuadPts::new([
            DPoint::new(-4.0, 1.0),
            DPoint::new(-4.0, 5.0),
            DPoint::new(0.0, 5.0),
        ]),
        0.707106769_f32,
    ),
    ConicPts::new(
        QuadPts::new([
            DPoint::new(-3.0, 4.0),
            DPoint::new(-3.0, 1.0),
            DPoint::new(0.0, 1.0),
        ]),
        0.707106769_f32,
    ),
    ConicPts::new(
        QuadPts::new([
            DPoint::new(0.0, 0.0),
            DPoint::new(0.0, 1.0),
            DPoint::new(1.0, 1.0),
        ]),
        0.5_f32,
    ),
    ConicPts::new(
        QuadPts::new([
            DPoint::new(1.0, 0.0),
            DPoint::new(0.0, 0.0),
            DPoint::new(0.0, 1.0),
        ]),
        0.5_f32,
    ),
];

/// `testSetCount`.
// Port of: tests/PathOpsConicIntersectionTest.cpp#L59 (chrome/m156)
const TEST_SET_COUNT: usize = TEST_SET.len();

/// Port of `chopCompare`: its checks are `SkASSERT`s.
// Port of: tests/PathOpsConicIntersectionTest.cpp#L61-L73 (chrome/m156)
fn chop_compare(chopped: &[Conic; 2], d_chopped: &[DConic; 2]) {
    debug_assert!(roughly_equal(
        f64::from(chopped[0].w),
        f64::from(d_chopped[0].weight)
    ));
    debug_assert!(roughly_equal(
        f64::from(chopped[1].w),
        f64::from(d_chopped[1].weight)
    ));
    for c_index in 0..2 {
        for p_index in 0..3 {
            let up = DPoint::from_sk_point(chopped[c_index].pts[p_index]);
            debug_assert!(d_chopped[c_index].pts.pts[p_index].approximately_equal(up));
        }
    }
}

/// Port of `chopBothWays`.
// Port of: tests/PathOpsConicIntersectionTest.cpp#L170-L188 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors conic.chopAt(SkDoubleToScalar(t), ...)
fn chop_both_ways(d_conic: &DConic, t: f64) {
    let mut conic = Conic::default();
    for index in 0..3 {
        conic.pts[index] = d_conic.pts.pts[index].as_sk_point();
    }
    conic.w = d_conic.weight;
    let mut chopped = [Conic::default(); 2];
    if !conic.chop_at(t as f32, &mut chopped) {
        return;
    }
    let d_chopped = [d_conic.sub_divide(0.0, t), d_conic.sub_divide(t, 1.0)];
    chop_compare(&chopped, &d_chopped);
}

/// Port of `oneOff(reporter, conic1, conic2, coin)`.
// Port of: tests/PathOpsConicIntersectionTest.cpp#L308-L340 (chrome/m156)
fn one_off(reporter: &mut Reporter, conic1: &ConicPts, conic2: &ConicPts, coin: bool) {
    let c1 = DConic::new(DQuad::new(conic1.pts.pts), conic1.weight);
    let c2 = DConic::new(DQuad::new(conic2.pts.pts), conic2.weight);
    chop_both_ways(&c1, 0.5);
    chop_both_ways(&c2, 0.5);
    debug_assert!(valid_conic(&c1));
    debug_assert!(valid_conic(&c2));
    let mut intersections = Intersections::default();
    intersections.intersect_conic_conic(&c1, &c2);
    reporter_assert!(reporter, !coin || intersections.used() == 2);
    for pt3 in 0..intersections.used() {
        let tt1 = intersections.t(0, pt3);
        let xy1 = c1.pt_at_t(tt1);
        let tt2 = intersections.t(1, pt3);
        let xy2 = c2.pt_at_t(tt2);
        let i_pt = intersections.pt(pt3);
        reporter_assert!(reporter, xy1.approximately_equal(i_pt));
        reporter_assert!(reporter, xy2.approximately_equal(i_pt));
        reporter_assert!(reporter, xy1.approximately_equal(xy2));
    }
    reporter.bump_test_count();
}

/// Port of `oneOff(reporter, outer, inner)`.
// Port of: tests/PathOpsConicIntersectionTest.cpp#L342-L345 (chrome/m156)
fn one_off_pair(reporter: &mut Reporter, outer: usize, inner: usize) {
    one_off(reporter, &TEST_SET[outer], &TEST_SET[inner], false);
}

/// Port of `oneOffTests`.
// Port of: tests/PathOpsConicIntersectionTest.cpp#L347-L354 (chrome/m156)
fn one_off_tests(reporter: &mut Reporter) {
    for outer in 0..TEST_SET_COUNT - 1 {
        for inner in outer + 1..TEST_SET_COUNT {
            one_off_pair(reporter, outer, inner);
        }
    }
}

def_test!(PathOpsConicIntersectionOneOff, |reporter| {
    one_off_pair(reporter, 0, 1);
});

def_test!(PathOpsConicIntersection, |reporter| {
    one_off_tests(reporter);
});
