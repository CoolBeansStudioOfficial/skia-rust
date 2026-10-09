// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsDVectorTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::scalar::{Scalar, double_to_scalar, scalar};
use skia_rust_pathops::point::{DPoint, DVector};
use skia_rust_pathops::types::approximately_equal;

// Port of: tests/PathOpsDVectorTest.cpp#L20-L28 (chrome/m156)
const TESTS: [DPoint; 7] = [
    DPoint::new(0.0, 0.0),
    DPoint::new(1.0, 0.0),
    DPoint::new(0.0, 1.0),
    DPoint::new(2.0, 1.0),
    DPoint::new(1.0, 2.0),
    DPoint::new(1.0, 1.0),
    DPoint::new(2.0, 2.0),
];

def_test!(PathOpsDVector, |reporter| {
    for index in 0..TESTS.len() - 1 {
        let mut v1 = TESTS[index + 1] - TESTS[index];
        let mut v2 = TESTS[index] - TESTS[index + 1];
        v1 += v2;
        reporter_assert!(reporter, v1.x == 0.0 && v1.y == 0.0);
        v2 -= v2;
        reporter_assert!(reporter, v2.x == 0.0 && v2.y == 0.0);
        v1 = TESTS[index + 1] - TESTS[index];
        v1 /= 2.0;
        v1 *= 2.0;
        v1 -= TESTS[index + 1] - TESTS[index];
        reporter_assert!(reporter, v1.x == 0.0 && v1.y == 0.0);
        let sv = v1.as_sk_vector();
        reporter_assert!(reporter, sv.x == 0.0 && sv.y == 0.0);
        v1 = TESTS[index + 1] - TESTS[index];
        let len_sq = v1.length_squared();
        let v1_dot = v1.dot(v1);
        #[allow(clippy::float_cmp)] // exact comparison, as in the C++ test
        {
            reporter_assert!(reporter, len_sq == v1_dot);
        }
        reporter_assert!(reporter, approximately_equal(len_sq.sqrt(), v1.length()));
        let v1_cross = v1.cross(v1);
        #[allow(clippy::float_cmp)] // exact comparison, as in the C++ test
        {
            reporter_assert!(reporter, v1_cross == 0.0);
        }
    }
});

/// `SkScalarNearlyEqual(double, double)`: the C++ converts both to `SkScalar`.
fn assert_doubles_equal(reporter: &mut Reporter, left: f64, right: f64) {
    let l: scalar = double_to_scalar(left);
    let r: scalar = double_to_scalar(right);
    reporter_assert!(reporter, <scalar as Scalar>::nearly_equal(l, r, None));
}

def_test!(SkDVector_normalize, |reporter| {
    let mut first = DVector::new(1.2, 3.4);
    first.normalize();
    reporter_assert!(reporter, first.is_finite());
    assert_doubles_equal(reporter, first.x, 0.332_820);
    assert_doubles_equal(reporter, first.y, 0.942_990);

    let mut second = DVector::new(2.3, -4.5);
    second.normalize();
    reporter_assert!(reporter, second.is_finite());
    assert_doubles_equal(reporter, second.x, 0.455_111);
    assert_doubles_equal(reporter, second.y, -0.890_435);
});

def_test!(SkDVector_normalize_infinity_and_nan, |reporter| {
    let mut first = DVector::new(0.0, 0.0);
    first.normalize();
    reporter_assert!(reporter, !first.is_finite());
    reporter_assert!(reporter, first.x.is_nan());
    reporter_assert!(reporter, first.y.is_nan());

    let mut second = DVector::new(f64::MAX, f64::MAX);
    second.normalize();
    reporter_assert!(reporter, second.is_finite());
    #[allow(clippy::float_cmp)] // exact comparison, as in the C++ test
    {
        reporter_assert!(reporter, second.x == 0.0);
        reporter_assert!(reporter, second.y == 0.0);
    }
});
