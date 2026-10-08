// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsCubicQuadIntersectionTest.cpp (chrome/m156)

#![cfg(test)]

use crate::unit::path_ops_test_common::{CubicPts, QuadPts, valid_cubic, valid_quad};
use crate::{Reporter, def_test, reporter_assert};
use skia_rust_pathops::cubic::DCubic;
use skia_rust_pathops::intersections::Intersections;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::quad::DQuad;

// The table literals below name `DPoint::new`, `CubicPts::new` and `QuadPts::new`.
use skia_rust_pathops::reduce_order::{Quadratics, ReduceOrder};

/// `struct quadCubic { CubicPts cubic; QuadPts quad; }`, as a `(cubic, quad)` pair.
// Port of: tests/PathOpsCubicQuadIntersectionTest.cpp#L19-L22 (chrome/m156)
type QuadCubic = (CubicPts, QuadPts);

/// `static constexpr auto quadCubicTests`.
// Port of: tests/PathOpsCubicQuadIntersectionTest.cpp#L23-L85 (chrome/m156)
#[allow(
    clippy::unreadable_literal,
    clippy::excessive_precision,
    clippy::approx_constant
)] // the table keeps the C++ literals verbatim
static QUAD_CUBIC_TESTS: &[QuadCubic] = &[
    (
        CubicPts::new([
            DPoint::new(945.08099365234375, 747.1619873046875),
            DPoint::new(982.5679931640625, 747.1619873046875),
            DPoint::new(1013.6290283203125, 719.656005859375),
            DPoint::new(1019.1910400390625, 683.72601318359375),
        ]),
        QuadPts::new([
            DPoint::new(945.0, 747.0),
            DPoint::new(976.0660400390625, 747.0),
            DPoint::new(998.03302001953125, 725.03302001953125),
        ]),
    ),
    (
        CubicPts::new([
            DPoint::new(778.0, 14089.0),
            DPoint::new(778.0, 14091.208984375),
            DPoint::new(776.20916748046875, 14093.0),
            DPoint::new(774.0, 14093.0),
        ]),
        QuadPts::new([
            DPoint::new(778.0, 14089.0),
            DPoint::new(777.99957275390625, 14090.65625),
            DPoint::new(776.82843017578125, 14091.828125),
        ]),
    ),
    (
        CubicPts::new([
            DPoint::new(1020.08099, 672.161987),
            DPoint::new(1020.08002, 630.73999),
            DPoint::new(986.502014, 597.161987),
            DPoint::new(945.080994, 597.161987),
        ]),
        QuadPts::new([
            DPoint::new(1020.0, 672.0),
            DPoint::new(1020.0, 640.93396),
            DPoint::new(998.03302, 618.96698),
        ]),
    ),
    (
        CubicPts::new([
            DPoint::new(778.0, 14089.0),
            DPoint::new(778.0, 14091.208984375),
            DPoint::new(776.20916748046875, 14093.0),
            DPoint::new(774.0, 14093.0),
        ]),
        QuadPts::new([
            DPoint::new(778.0, 14089.0),
            DPoint::new(777.99957275390625, 14090.65625),
            DPoint::new(776.82843017578125, 14091.828125),
        ]),
    ),
    (
        CubicPts::new([
            DPoint::new(1110.0, 817.0),
            DPoint::new(1110.55225_f32 as f64, 817.0),
            DPoint::new(1111.0, 817.447693_f32 as f64),
            DPoint::new(1111.0, 818.0),
        ]),
        QuadPts::new([
            DPoint::new(1110.70715_f32 as f64, 817.292908_f32 as f64),
            DPoint::new(1110.41406_f32 as f64, 817.000122_f32 as f64),
            DPoint::new(1110.0, 817.0),
        ]),
    ),
    (
        CubicPts::new([
            DPoint::new(1110.0, 817.0),
            DPoint::new(1110.55225_f32 as f64, 817.0),
            DPoint::new(1111.0, 817.447693_f32 as f64),
            DPoint::new(1111.0, 818.0),
        ]),
        QuadPts::new([
            DPoint::new(1111.0, 818.0),
            DPoint::new(1110.99988_f32 as f64, 817.585876_f32 as f64),
            DPoint::new(1110.70715_f32 as f64, 817.292908_f32 as f64),
        ]),
    ),
    (
        CubicPts::new([
            DPoint::new(55.0, 207.0),
            DPoint::new(52.238574981689453, 207.0),
            DPoint::new(50.0, 204.76142883300781),
            DPoint::new(50.0, 202.0),
        ]),
        QuadPts::new([
            DPoint::new(55.0, 207.0),
            DPoint::new(52.929431915283203, 206.99949645996094),
            DPoint::new(51.464466094970703, 205.53553771972656),
        ]),
    ),
    (
        CubicPts::new([
            DPoint::new(49.0, 47.0),
            DPoint::new(49.0, 74.614250183105469),
            DPoint::new(26.614250183105469, 97.0),
            DPoint::new(-1.0, 97.0),
        ]),
        QuadPts::new([
            DPoint::new(-8.659739592076221e-015, 96.991401672363281),
            DPoint::new(20.065492630004883, 96.645187377929688),
            DPoint::new(34.355339050292969, 82.355339050292969),
        ]),
    ),
    (
        CubicPts::new([
            DPoint::new(10.0, 234.0),
            DPoint::new(10.0, 229.58172607421875),
            DPoint::new(13.581720352172852, 226.0),
            DPoint::new(18.0, 226.0),
        ]),
        QuadPts::new([
            DPoint::new(18.0, 226.0),
            DPoint::new(14.686291694641113, 226.0),
            DPoint::new(12.342399597167969, 228.3424072265625),
        ]),
    ),
    (
        CubicPts::new([
            DPoint::new(10.0, 234.0),
            DPoint::new(10.0, 229.58172607421875),
            DPoint::new(13.581720352172852, 226.0),
            DPoint::new(18.0, 226.0),
        ]),
        QuadPts::new([
            DPoint::new(12.342399597167969, 228.3424072265625),
            DPoint::new(10.0, 230.68629455566406),
            DPoint::new(10.0, 234.0),
        ]),
    ),
];

/// `quadCubicTests_count`.
// Port of: tests/PathOpsCubicQuadIntersectionTest.cpp#L87 (chrome/m156)
const QUAD_CUBIC_TESTS_COUNT: usize = QUAD_CUBIC_TESTS.len();

/// Port of `cubicQuadIntersection`. Its `SkDebugf` lines are not ported.
// Port of: tests/PathOpsCubicQuadIntersectionTest.cpp#L89-L122 (chrome/m156)
fn cubic_quad_intersection(reporter: &mut Reporter, index: usize) {
    let c = &QUAD_CUBIC_TESTS[index].0;
    let cubic = DCubic::new(c.pts);
    debug_assert!(valid_cubic(&cubic));
    let q = &QUAD_CUBIC_TESTS[index].1;
    let quad = DQuad::new(q.pts);
    debug_assert!(valid_quad(&quad));
    let mut reduce1 = ReduceOrder::default();
    let mut reduce2 = ReduceOrder::default();
    let order1 = reduce1.reduce_cubic(&cubic, Quadratics::No);
    let order2 = reduce2.reduce_quad(&quad);
    if order1 != 4 {
        reporter_assert!(reporter, false);
    }
    if order2 != 3 {
        reporter_assert!(reporter, false);
    }
    let mut i = Intersections::default();
    let roots = i.intersect_cubic_quad(&cubic, &quad);
    for pt in 0..roots {
        let tt1 = i.t(0, pt);
        let xy1 = cubic.pt_at_t(tt1);
        let tt2 = i.t(1, pt);
        let xy2 = quad.pt_at_t(tt2);
        reporter_assert!(reporter, xy1.approximately_equal(xy2));
    }
    reporter.bump_test_count();
}

def_test!(PathOpsCubicQuadIntersection, |reporter| {
    for index in 0..QUAD_CUBIC_TESTS_COUNT {
        cubic_quad_intersection(reporter, index);
        reporter.bump_test_count();
    }
});

def_test!(PathOpsCubicQuadIntersectionOneOff, |reporter| {
    cubic_quad_intersection(reporter, 0);
});
