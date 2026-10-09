// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsConicQuadIntersectionTest.cpp (chrome/m156)

#![cfg(test)]

use crate::unit::path_ops_test_common::{ConicPts, QuadPts, valid_conic, valid_quad};
use crate::{Reporter, def_test, reporter_assert};
use skia_rust_pathops::conic::DConic;
use skia_rust_pathops::intersections::Intersections;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::quad::DQuad;
use skia_rust_pathops::reduce_order::ReduceOrder;

/// `struct conicQuad { ConicPts conic; QuadPts quad; }`, as a `(conic, quad)` pair.
// Port of: tests/PathOpsConicQuadIntersectionTest.cpp#L19-L22 (chrome/m156)
type ConicQuad = (ConicPts, QuadPts);

/// `static constexpr auto conicQuadTests`.
// Port of: tests/PathOpsConicQuadIntersectionTest.cpp#L23-L41 (chrome/m156)
#[allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::approx_constant
)] // the table keeps the C++ literals verbatim
static CONIC_QUAD_TESTS: &[ConicQuad] = &[
    (
        ConicPts::new(
            QuadPts::new([
                DPoint::new(0.00000000000000000, -1.8135968446731567),
                DPoint::new(0.00000000000000000, -1.0033817291259766),
                DPoint::new(-0.0073835160583257675, 0.00000000000000000),
            ]),
            2.26585215e+11_f32,
        ),
        QuadPts::new([
            DPoint::new(0.00000000000000000, -1.0000113248825073),
            DPoint::new(-2.4824290449032560e-05, -1.0000115633010864),
            DPoint::new(-0.0073835160583257675, 0.00000000000000000),
        ]),
    ),
    (
        ConicPts::new(
            QuadPts::new([
                DPoint::new(494.348663, 224.583771),
                DPoint::new(494.365143, 224.633194),
                DPoint::new(494.376404, 224.684067),
            ]),
            0.998645842_f32,
        ),
        QuadPts::new([
            DPoint::new(494.30481, 224.474213),
            DPoint::new(494.334961, 224.538284),
            DPoint::new(494.355774, 224.605927),
        ]),
    ),
    (
        ConicPts::new(
            QuadPts::new([
                DPoint::new(494.348663, 224.583771),
                DPoint::new(494.365143, 224.633194),
                DPoint::new(494.376404, 224.684067),
            ]),
            0.998645842_f32,
        ),
        QuadPts::new([
            DPoint::new(494.355774_f32 as f64, 224.605927_f32 as f64),
            DPoint::new(494.363708_f32 as f64, 224.631714_f32 as f64),
            DPoint::new(494.370148_f32 as f64, 224.657471_f32 as f64),
        ]),
    ),
];

/// `conicQuadTests_count`.
// Port of: tests/PathOpsConicQuadIntersectionTest.cpp#L43 (chrome/m156)
const CONIC_QUAD_TESTS_COUNT: usize = CONIC_QUAD_TESTS.len();

/// Port of `conicQuadIntersection`. Its `SkDebugf` lines are not ported.
// Port of: tests/PathOpsConicQuadIntersectionTest.cpp#L45-L77 (chrome/m156)
fn conic_quad_intersection(reporter: &mut Reporter, index: usize) {
    let c = &CONIC_QUAD_TESTS[index].0;
    let conic = DConic::new(DQuad::new(c.pts.pts), c.weight);
    debug_assert!(valid_conic(&conic));
    let q = &CONIC_QUAD_TESTS[index].1;
    let quad = DQuad::new(q.pts);
    debug_assert!(valid_quad(&quad));
    let mut reduce1 = ReduceOrder::default();
    let mut reduce2 = ReduceOrder::default();
    let order1 = reduce2.reduce_quad(&conic.pts);
    let order2 = reduce1.reduce_quad(&quad);
    if order2 != 3 {
        reporter_assert!(reporter, false);
    }
    if order1 != 3 {
        reporter_assert!(reporter, false);
    }
    let mut i = Intersections::default();
    let roots = i.intersect_conic_quad(&conic, &quad);
    for pt in 0..roots {
        let tt1 = i.t(0, pt);
        let xy1 = conic.pt_at_t(tt1);
        let tt2 = i.t(1, pt);
        let xy2 = quad.pt_at_t(tt2);
        reporter_assert!(reporter, xy1.approximately_equal(xy2));
    }
    reporter.bump_test_count();
}

def_test!(PathOpsConicQuadIntersection, |reporter| {
    for index in 0..CONIC_QUAD_TESTS_COUNT {
        conic_quad_intersection(reporter, index);
        reporter.bump_test_count();
    }
});

def_test!(PathOpsConicQuadIntersectionOneOff, |reporter| {
    conic_quad_intersection(reporter, 0);
});
