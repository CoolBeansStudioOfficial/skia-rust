// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/bentleyottmann/tests/BruteForceCrossingsTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_bentleyottmann::brute_force_crossings::brute_force_crossings;
use skia_rust_bentleyottmann::point::Point;
use skia_rust_bentleyottmann::segment::Segment;

use crate::{def_test, reporter_assert};

def_test!(BO_bruteForceCrossingsBasic, |reporter| {
    {
        let s0 = Segment::new(Point::new(-1, 0), Point::new(1, 0));
        let s1 = Segment::new(Point::new(0, 1), Point::new(0, -1));
        let segments = vec![s0, s1];
        let possible_crossings = brute_force_crossings(&segments);
        reporter_assert!(reporter, possible_crossings.is_some());
        if let Some(crossings) = &possible_crossings {
            reporter_assert!(reporter, crossings.len() == 1);
            let p = Point::new(0, 0);
            reporter_assert!(reporter, crossings[0].crossing == p);
            reporter_assert!(
                reporter,
                (crossings[0].s0 == s0 && crossings[0].s1 == s1)
                    || (crossings[0].s0 == s1 && crossings[0].s1 == s0)
            );
        }
    }
    {
        let p0 = Point::new(-50, -100);
        let p1 = Point::new(50, -100);
        let p2 = Point::new(0, 100);
        let s0 = Segment::new(p0, p1);
        let s1 = Segment::new(p1, p2);
        let s2 = Segment::new(p2, p0);
        let segments = vec![s0, s1, s2];
        let possible_crossings = brute_force_crossings(&segments);
        reporter_assert!(reporter, possible_crossings.is_some());
        if let Some(crossings) = &possible_crossings {
            reporter_assert!(reporter, crossings.is_empty());
        }
    }
    {
        let p0 = Point::new(-50, 100);
        let p1 = Point::new(50, 100);
        let p2 = Point::new(0, -100);
        let s0 = Segment::new(p0, p1);
        let s1 = Segment::new(p1, p2);
        let s2 = Segment::new(p2, p0);
        let segments = vec![s0, s1, s2];
        let possible_crossings = brute_force_crossings(&segments);
        reporter_assert!(reporter, possible_crossings.is_some());
        if let Some(crossings) = &possible_crossings {
            reporter_assert!(reporter, crossings.is_empty());
        }
    }
});
