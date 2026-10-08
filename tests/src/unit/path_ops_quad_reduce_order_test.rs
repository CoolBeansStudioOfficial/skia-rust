// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsQuadReduceOrderTest.cpp (chrome/m156)

#![cfg(test)]

use crate::unit::path_ops_quad_intersection_test_data::{QUADRATICLINES, QUADRATICMODEPSILONLINES};
use crate::unit::path_ops_test_common::QuadPts;
use crate::{Reporter, def_test};
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::quad::DQuad;
use skia_rust_pathops::reduce_order::ReduceOrder;

// Port of: tests/PathOpsQuadReduceOrderTest.cpp (chrome/m156)
#[allow(clippy::unreadable_literal)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
const TEST_SET: [QuadPts; 2] = [
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.000003),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 0.0),
        DPoint::new(2.0, 6.0),
        DPoint::new(3.0, 0.0),
    ]),
];

/// Port of `oneOffTest`.
// Port of: tests/PathOpsQuadReduceOrderTest.cpp#L26-L35 (chrome/m156)
#[allow(clippy::doc_markdown, clippy::manual_assert_eq)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
fn one_off_test(_reporter: &mut Reporter) {
    for q in &TEST_SET {
        let quad = DQuad::new(q.pts);
        let mut reducer = ReduceOrder::default();
        let result = reducer.reduce_quad(&quad);
        debug_assert!(result == 3);
    }
}

/// Port of `standardTestCases`. The C++ only reports orders that differ from the expected ones
/// (`SkDebugf`) and makes no assertions, so the loops run without checks.
// Port of: tests/PathOpsQuadReduceOrderTest.cpp#L37-L75 (chrome/m156)
fn standard_test_cases() {
    let mut reducer = ReduceOrder::default();
    for q in &QUADRATICLINES {
        let quad = DQuad::new(q.pts);
        let _order = reducer.reduce_quad(&quad);
    }
    for q in &QUADRATICMODEPSILONLINES {
        let quad = DQuad::new(q.pts);
        let _order = reducer.reduce_quad(&quad);
    }
}

def_test!(PathOpsReduceOrderQuad, |reporter| {
    one_off_test(reporter);
    standard_test_cases();
});
