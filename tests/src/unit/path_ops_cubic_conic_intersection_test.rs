// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsCubicConicIntersectionTest.cpp (chrome/m156)

#![cfg(test)]

use crate::unit::path_ops_test_common::{ConicPts, CubicPts, QuadPts, valid_conic, valid_cubic};
use crate::{Reporter, def_test, reporter_assert};
use skia_rust_pathops::conic::DConic;
use skia_rust_pathops::cubic::DCubic;
use skia_rust_pathops::intersections::Intersections;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::quad::DQuad;
use skia_rust_pathops::reduce_order::{Quadratics, ReduceOrder};

/// `struct cubicConic { CubicPts cubic; ConicPts conic; }`, as a `(cubic, conic)` pair.
// Port of: tests/PathOpsCubicConicIntersectionTest.cpp#L20-L23 (chrome/m156)
type CubicConic = (CubicPts, ConicPts);

/// `static constexpr auto cubicConicTests`.
// Port of: tests/PathOpsCubicConicIntersectionTest.cpp#L24-L57 (chrome/m156)
#[allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::approx_constant
)] // the table keeps the C++ literals verbatim
static CUBIC_CONIC_TESTS: &[CubicConic] = &[
    (
        CubicPts::new([
            DPoint::new(188.60000610351562, 2041.5999755859375),
            DPoint::new(188.60000610351562, 2065.39990234375),
            DPoint::new(208.0, 2084.800048828125),
            DPoint::new(231.80000305175781, 2084.800048828125),
        ]),
        ConicPts::new(
            QuadPts::new([
                DPoint::new(231.80000305175781, 2084.800048828125),
                DPoint::new(188.60000610351562, 2084.800048828125),
                DPoint::new(188.60000610351562, 2041.5999755859375),
            ]),
            0.707107008_f32,
        ),
    ),
    (
        CubicPts::new([
            DPoint::new(231.80000305175781, 2084.800048828125),
            DPoint::new(255.60000610351562, 2084.800048828125),
            DPoint::new(275.0, 2065.39990234375),
            DPoint::new(275.0, 2041.5999755859375),
        ]),
        ConicPts::new(
            QuadPts::new([
                DPoint::new(275.0, 2041.5999755859375),
                DPoint::new(275.0, 2084.800048828125),
                DPoint::new(231.80000305175781, 2084.800048828125),
            ]),
            0.707107008_f32,
        ),
    ),
];

/// `cubicConicTests_count`.
// Port of: tests/PathOpsCubicConicIntersectionTest.cpp#L59 (chrome/m156)
const CUBIC_CONIC_TESTS_COUNT: usize = CUBIC_CONIC_TESTS.len();

/// Port of `cubicConicIntersection`. Its `SkDebugf` lines are not ported.
// Port of: tests/PathOpsCubicConicIntersectionTest.cpp#L61-L93 (chrome/m156)
fn cubic_conic_intersection(reporter: &mut Reporter, index: usize) {
    let cu = &CUBIC_CONIC_TESTS[index].0;
    let cubic = DCubic::new(cu.pts);
    debug_assert!(valid_cubic(&cubic));
    let co = &CUBIC_CONIC_TESTS[index].1;
    let conic = DConic::new(DQuad::new(co.pts.pts), co.weight);
    debug_assert!(valid_conic(&conic));
    let mut reduce1 = ReduceOrder::default();
    let mut reduce2 = ReduceOrder::default();
    let order1 = reduce1.reduce_cubic(&cubic, Quadratics::No);
    let order2 = reduce2.reduce_quad(&conic.pts);
    if order1 != 4 {
        reporter_assert!(reporter, false);
    }
    if order2 != 3 {
        reporter_assert!(reporter, false);
    }
    let mut i = Intersections::default();
    let roots = i.intersect_cubic_conic(&cubic, &conic);
    for pt in 0..roots {
        let tt1 = i.t(0, pt);
        let xy1 = cubic.pt_at_t(tt1);
        let tt2 = i.t(1, pt);
        let xy2 = conic.pt_at_t(tt2);
        reporter_assert!(reporter, xy1.approximately_equal(xy2));
    }
    reporter.bump_test_count();
}

def_test!(PathOpsCubicConicIntersection, |reporter| {
    for index in 0..CUBIC_CONIC_TESTS_COUNT {
        cubic_conic_intersection(reporter, index);
        reporter.bump_test_count();
    }
});

def_test!(PathOpsCubicConicIntersectionOneOff, |reporter| {
    cubic_conic_intersection(reporter, 0);
});
