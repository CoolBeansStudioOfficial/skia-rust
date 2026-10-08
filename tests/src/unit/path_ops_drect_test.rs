// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsDRectTest.cpp (chrome/m156)

#![cfg(test)]

use crate::unit::path_ops_test_common::{CubicPts, QuadPts};
use crate::{def_test, reporter_assert};
use skia_rust_pathops::cubic::DCubic;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::quad::DQuad;
use skia_rust_pathops::rect::DRect;

// Port of: tests/PathOpsDRectTest.cpp (chrome/m156)
const QUAD_TESTS: [QuadPts; 5] = [
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 1.0),
        DPoint::new(0.0, 2.0),
    ]),
    QuadPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 1.0),
    ]),
    QuadPts::new([
        DPoint::new(2.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
    ]),
    QuadPts::new([
        DPoint::new(4.0, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(4.0, 2.0),
    ]),
    QuadPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 1.0),
    ]),
];

const CUBIC_TESTS: [CubicPts; 3] = [
    CubicPts::new([
        DPoint::new(2.0, 0.0),
        DPoint::new(3.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(3.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(3.0, 0.0),
        DPoint::new(2.0, 1.0),
        DPoint::new(3.0, 2.0),
        DPoint::new(1.0, 1.0),
    ]),
];

/// Port of `setRawBounds(const SkDQuad&, SkDRect*)`.
fn set_raw_bounds_quad(quad: &DQuad, rect: &mut DRect) {
    rect.set(quad[0]);
    rect.add(quad[1]);
    rect.add(quad[2]);
}

/// Port of `setRawBounds(const SkDCubic&, SkDRect*)`.
fn set_raw_bounds_cubic(cubic: &DCubic, rect: &mut DRect) {
    rect.set(cubic[0]);
    rect.add(cubic[1]);
    rect.add(cubic[2]);
    rect.add(cubic[3]);
}

def_test!(PathOpsDRect, |reporter| {
    let mut rect = DRect::default();
    let mut rect2 = DRect::default();
    for q in &QUAD_TESTS {
        let quad = DQuad::new(q.pts);
        set_raw_bounds_quad(&quad, &mut rect);
        rect2.set_bounds_quad(&quad);
        reporter_assert!(reporter, rect.intersects(&rect2));
        let left_top = DPoint::new(rect2.left, rect2.top);
        reporter_assert!(reporter, rect.contains(left_top));
        let right_bottom = DPoint::new(rect2.right, rect2.bottom);
        reporter_assert!(reporter, rect.contains(right_bottom));
    }
    for c in &CUBIC_TESTS {
        let cubic = DCubic::new(c.pts);
        set_raw_bounds_cubic(&cubic, &mut rect);
        rect2.set_bounds_cubic(&cubic);
        reporter_assert!(reporter, rect.intersects(&rect2));
        let left_top = DPoint::new(rect2.left, rect2.top);
        reporter_assert!(reporter, rect.contains(left_top));
        let right_bottom = DPoint::new(rect2.right, rect2.bottom);
        reporter_assert!(reporter, rect.contains(right_bottom));
    }
});
