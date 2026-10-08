// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsCubicReduceOrderTest.cpp (chrome/m156)

#![cfg(test)]

// The C++ helpers `controls_inside`, `tiny` and `find_tight_bounds` and the test
// `PathOpsReduceOrderCubic`'s stroke-reduction loop are under `#if 0` ("disable test until
// stroke reduction is supported"), so they are not ported.

use crate::unit::path_ops_cubic_intersection_test_data::{
    LESSEPSILONLINES, LINES, MODEPSILONLINES, NEGEPSILONLINES, NOTLINES, NOTPOINTDEGENERATES,
    POINTDEGENERATES,
};
use crate::unit::path_ops_quad_intersection_test_data::{
    QUADRATICLINES, QUADRATICMODEPSILONLINES, QUADRATICPOINTS,
};
use crate::unit::path_ops_test_common::{valid_cubic, valid_quad};
use crate::{def_test, reporter_assert};
use skia_rust_pathops::cubic::DCubic;
use skia_rust_pathops::quad::DQuad;
use skia_rust_pathops::reduce_order::{Quadratics, ReduceOrder};

// Port of: tests/PathOpsCubicReduceOrderTest.cpp#L68-L269 (chrome/m156)
// The C++ `run` selector is `RunAll`, so every `first*Test` index is 0 and every loop runs in full.
def_test!(PathOpsReduceOrderCubic, |reporter| {
    let mut reducer = ReduceOrder::default();
    for c in &POINTDEGENERATES {
        let cubic = DCubic::new(c.pts);
        debug_assert!(valid_cubic(&cubic));
        let order = reducer.reduce_cubic(&cubic, Quadratics::Allow);
        reporter_assert!(reporter, order == 1);
    }
    for c in &NOTPOINTDEGENERATES {
        let cubic = DCubic::new(c.pts);
        debug_assert!(valid_cubic(&cubic));
        let order = reducer.reduce_cubic(&cubic, Quadratics::Allow);
        reporter_assert!(reporter, order != 1);
    }
    for c in &LINES {
        let cubic = DCubic::new(c.pts);
        debug_assert!(valid_cubic(&cubic));
        let order = reducer.reduce_cubic(&cubic, Quadratics::Allow);
        reporter_assert!(reporter, order == 2);
    }
    for c in &NOTLINES {
        let cubic = DCubic::new(c.pts);
        debug_assert!(valid_cubic(&cubic));
        let order = reducer.reduce_cubic(&cubic, Quadratics::Allow);
        reporter_assert!(reporter, order != 2);
    }
    for c in &MODEPSILONLINES {
        let cubic = DCubic::new(c.pts);
        debug_assert!(valid_cubic(&cubic));
        let order = reducer.reduce_cubic(&cubic, Quadratics::Allow);
        reporter_assert!(reporter, order != 2);
    }
    for c in &LESSEPSILONLINES {
        let cubic = DCubic::new(c.pts);
        debug_assert!(valid_cubic(&cubic));
        let order = reducer.reduce_cubic(&cubic, Quadratics::Allow);
        reporter_assert!(reporter, order == 2);
    }
    for c in &NEGEPSILONLINES {
        let cubic = DCubic::new(c.pts);
        debug_assert!(valid_cubic(&cubic));
        let order = reducer.reduce_cubic(&cubic, Quadratics::Allow);
        reporter_assert!(reporter, order == 2);
    }
    for q in &QUADRATICPOINTS {
        let quad = DQuad::new(q.pts);
        debug_assert!(valid_quad(&quad));
        let cubic = quad.debug_to_cubic();
        let order = reducer.reduce_cubic(&cubic, Quadratics::Allow);
        reporter_assert!(reporter, order == 1);
    }
    for q in &QUADRATICLINES {
        let quad = DQuad::new(q.pts);
        debug_assert!(valid_quad(&quad));
        let cubic = quad.debug_to_cubic();
        let order = reducer.reduce_cubic(&cubic, Quadratics::Allow);
        reporter_assert!(reporter, order == 2);
    }
    for q in &QUADRATICMODEPSILONLINES {
        let quad = DQuad::new(q.pts);
        debug_assert!(valid_quad(&quad));
        let cubic = quad.debug_to_cubic();
        let order = reducer.reduce_cubic(&cubic, Quadratics::Allow);
        reporter_assert!(reporter, order == 3);
    }
});
