// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/bentleyottmann/tests/PointTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_bentleyottmann::point::Point;

use crate::{def_test, reporter_assert};

def_test!(BO_PointBasic, |reporter| {
    {
        let p0 = Point::new(0, 0);
        let p1 = Point::new(0, 0);
        reporter_assert!(reporter, p0 >= p1);
        reporter_assert!(reporter, p1 >= p0);
        reporter_assert!(reporter, p0 == p1);
    }
    {
        let p0 = Point::new(0, 0);
        let p1 = Point::new(1, 1);
        reporter_assert!(reporter, p0 < p1);
        reporter_assert!(reporter, p1 >= p0);
        reporter_assert!(reporter, p0 != p1);
    }
    {
        // Same y different x.
        let p0 = Point::new(0, 0);
        let p1 = Point::new(1, 0);
        reporter_assert!(reporter, p0 < p1);
        reporter_assert!(reporter, p1 >= p0);
        reporter_assert!(reporter, p0 != p1);
    }
    {
        reporter_assert!(
            reporter,
            !Point::difference_too_big(Point::smallest(), Point::new(0, 0))
        );
        reporter_assert!(
            reporter,
            !Point::difference_too_big(Point::largest(), Point::new(0, 0))
        );
        reporter_assert!(
            reporter,
            Point::difference_too_big(Point::new(0, 0), Point::smallest())
        );
        reporter_assert!(
            reporter,
            !Point::difference_too_big(Point::new(0, 0), Point::largest())
        );
        reporter_assert!(
            reporter,
            Point::difference_too_big(Point::smallest(), Point::largest())
        );
        reporter_assert!(
            reporter,
            Point::difference_too_big(Point::largest(), Point::smallest())
        );
        reporter_assert!(
            reporter,
            !Point::difference_too_big(Point::smallest() + Point::new(1, 1), Point::new(0, 0))
        );
        reporter_assert!(
            reporter,
            !Point::difference_too_big(Point::new(0, 0), Point::smallest() + Point::new(1, 1))
        );
    }
});
