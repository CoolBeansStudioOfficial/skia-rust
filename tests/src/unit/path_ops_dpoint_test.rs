// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsDPointTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{def_test, reporter_assert};
use skia_rust_core::point::Point;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::types::approximately_equal;

// Port of: tests/PathOpsDPointTest.cpp#L18-L26 (chrome/m156)
const TESTS: [DPoint; 7] = [
    DPoint::new(0.0, 0.0),
    DPoint::new(1.0, 0.0),
    DPoint::new(0.0, 1.0),
    DPoint::new(2.0, 1.0),
    DPoint::new(1.0, 2.0),
    DPoint::new(1.0, 1.0),
    DPoint::new(2.0, 2.0),
];

def_test!(PathOpsDPoint, |reporter| {
    for pt in TESTS {
        let mut p = pt;
        reporter_assert!(reporter, p == pt);
        #[allow(clippy::eq_op)] // mirrors the C++ self-comparison
        {
            reporter_assert!(reporter, !(pt != pt));
        }
        let v = p - pt;
        p += v;
        reporter_assert!(reporter, p == pt);
        p -= v;
        reporter_assert!(reporter, p == pt);
        reporter_assert!(reporter, p.approximately_equal(pt));
        let s_pt: Point = pt.as_sk_point();
        p.set(s_pt);
        reporter_assert!(reporter, p == pt);
        reporter_assert!(reporter, p.approximately_equal_sk(s_pt));
        reporter_assert!(reporter, p.roughly_equal(pt));
        p.x = 0.0;
        p.y = 0.0;
        reporter_assert!(reporter, p.x == 0.0 && p.y == 0.0);
        reporter_assert!(reporter, p.approximately_zero());
        #[allow(clippy::float_cmp)] // exact comparison, as in the C++ test
        {
            reporter_assert!(
                reporter,
                pt.distance_squared(p) == pt.x * pt.x + pt.y * pt.y
            );
        }
        reporter_assert!(
            reporter,
            approximately_equal(pt.distance(p), (pt.x * pt.x + pt.y * pt.y).sqrt())
        );
    }
});
