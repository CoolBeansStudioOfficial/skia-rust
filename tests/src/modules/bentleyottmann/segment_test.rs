// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/bentleyottmann/tests/SegmentTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_bentleyottmann::point::Point;
use skia_rust_bentleyottmann::segment::{
    Segment, compare_slopes, intersect, less_than_at, no_intersection_by_bounding_box,
    rounded_point_less_than_segment_in_x_lower, rounded_point_less_than_segment_in_x_upper,
};

use crate::{Reporter, def_test, reporter_assert};

/// `swap_ends`: the same segment with its endpoints in the other order.
fn swap_ends(s: &Segment) -> Segment {
    Segment::new(s.p1, s.p0)
}

def_test!(BO_SegmentBasic, |reporter| {
    {
        let s = Segment::new(Point::new(0, 0), Point::new(1, 1));
        reporter_assert!(reporter, s.upper() == s.p0);
        reporter_assert!(reporter, s.lower() == s.p1);
    }
    {
        let s = Segment::new(Point::new(1, 0), Point::new(0, 1));
        reporter_assert!(reporter, s.upper() == s.p0);
        reporter_assert!(reporter, s.lower() == s.p1);
    }
    {
        let s = Segment::new(Point::new(1, 1), Point::new(0, 0));
        reporter_assert!(reporter, s.upper() == s.p1);
        reporter_assert!(reporter, s.lower() == s.p0);
    }
    {
        let s = Segment::new(Point::new(0, 1), Point::new(1, 0));
        reporter_assert!(reporter, s.upper() == s.p1);
        reporter_assert!(reporter, s.lower() == s.p0);
    }
});

def_test!(BO_no_intersection_bounding_box, |reporter| {
    let interesting = [
        Segment::new(Point::smallest(), Point::smallest() + Point::new(10, 5)),
        Segment::new(Point::largest(), Point::largest() - Point::new(10, 5)),
        Segment::new(Point::new(-10, -5), Point::new(10, 5)),
    ];

    // Intersection
    for s0 in &interesting {
        let (l, t, r, b) = s0.bounds();
        // Points in the interior of interesting rectangles
        for p in [
            Point::new(l + 1, t + 1),
            Point::new(r - 1, t + 1),
            Point::new(r - 1, b - 1),
            Point::new(l + 1, b - 1),
        ] {
            let s1 = Segment::new(p, Point::new(0, 0));
            reporter_assert!(reporter, !no_intersection_by_bounding_box(s0, &s1));
            reporter_assert!(reporter, !no_intersection_by_bounding_box(&s1, s0));
            reporter_assert!(
                reporter,
                !no_intersection_by_bounding_box(&swap_ends(s0), &swap_ends(&s1))
            );
            reporter_assert!(
                reporter,
                !no_intersection_by_bounding_box(&swap_ends(s0), &swap_ends(&s1))
            );
        }
    }

    let small = Point::smallest().x;
    let big = Point::largest().x;

    // No Intersection
    for s0 in &interesting {
        let (l, t, r, b) = s0.bounds();
        let outside = [
            Segment::new(Point::new(r, t), Point::new(big, b)),
            Segment::new(Point::new(r, b), Point::new(big, big)),
            Segment::new(Point::new(l, b), Point::new(r, big)),
            Segment::new(Point::new(l, b), Point::new(small, big)),
            Segment::new(Point::new(l, t), Point::new(small, b)),
            Segment::new(Point::new(l, t), Point::new(small, small)),
            Segment::new(Point::new(l, t), Point::new(r, small)),
            Segment::new(Point::new(r, t), Point::new(small, small)),
        ];
        for s1 in &outside {
            reporter_assert!(reporter, no_intersection_by_bounding_box(s0, s1));
            reporter_assert!(reporter, no_intersection_by_bounding_box(s1, s0));
            reporter_assert!(
                reporter,
                no_intersection_by_bounding_box(&swap_ends(s0), &swap_ends(s1))
            );
            reporter_assert!(
                reporter,
                no_intersection_by_bounding_box(&swap_ends(s0), &swap_ends(s1))
            );
        }
    }
});

def_test!(BO_intersectBasic, |reporter| {
    let check_intersection =
        |reporter: &mut Reporter, s0: Segment, s1: Segment, expected: Point| {
            {
                let answer = intersect(&s0, &s1);
                reporter_assert!(reporter, answer.is_some());
                reporter_assert!(reporter, answer == Some(expected));
            }
            {
                let answer = intersect(&s1, &s0);
                reporter_assert!(reporter, answer.is_some());
                reporter_assert!(reporter, answer == Some(expected));
            }
            {
                let answer = intersect(&swap_ends(&s0), &swap_ends(&s1));
                reporter_assert!(reporter, answer.is_some());
                reporter_assert!(reporter, answer == Some(expected));
            }
            {
                let answer = intersect(&swap_ends(&s1), &swap_ends(&s0));
                reporter_assert!(reporter, answer.is_some());
                reporter_assert!(reporter, answer == Some(expected));
            }
        };

    {
        let s0 = Segment::new(Point::new(-1, 0), Point::new(1, 0));
        let s1 = Segment::new(Point::new(0, 1), Point::new(0, -1));
        check_intersection(reporter, s0, s1, Point::new(0, 0));
    }
    {
        let s0 = Segment::new(Point::new(-1, 0), Point::new(5, 0));
        let s1 = Segment::new(Point::new(0, 1), Point::new(0, -1));
        check_intersection(reporter, s0, s1, Point::new(0, 0));
    }
    {
        let s0 = Segment::new(Point::new(5, 0), Point::new(-1, 0));
        let s1 = Segment::new(Point::new(0, -1), Point::new(0, 1));
        check_intersection(reporter, s0, s1, Point::new(0, 0));
    }
    {
        let s0 = Segment::new(Point::new(-5, -5), Point::new(5, 5));
        let s1 = Segment::new(Point::new(-5, 5), Point::new(5, -5));
        check_intersection(reporter, s0, s1, Point::new(0, 0));
    }

    // Test very close segments (x0, 0) -> (x1, 1) & (x2, 0) -> (x3, 1)
    for x0 in -10_i32..=10 {
        for x1 in -10_i32..=10 {
            for x2 in -10_i32..=10 {
                for x3 in -10_i32..=10 {
                    let p0 = Point::new(x0, 0);
                    let p1 = Point::new(x1, 1);
                    let p2 = Point::new(x2, 0);
                    let p3 = Point::new(x3, 1);
                    let actual = intersect(&Segment::new(p0, p1), &Segment::new(p2, p3));
                    let expected = (x0 < x2 && x3 < x1) || (x2 < x0 && x1 < x3);
                    reporter_assert!(reporter, actual.is_some() == expected);
                    if let Some(p) = actual {
                        let y = i32::from((x2 - x0).abs() >= (x3 - x1).abs());
                        reporter_assert!(reporter, p.y == y);
                    }
                }
            }
        }
    }

    {
        let s0 = Segment::new(Point::new(0, -100), Point::new(0, -50));
        let s1 = Segment::new(Point::new(100, -100), Point::new(-100, 100)); // goes through (0,0)
        let mut answer = intersect(&s0, &s1);
        reporter_assert!(reporter, answer.is_none());
        answer = intersect(&s1, &s0);
        reporter_assert!(reporter, answer.is_none());
    }
    {
        let s0 = Segment::new(Point::new(0, 100), Point::new(0, 50));
        let s1 = Segment::new(Point::new(100, -100), Point::new(-100, 100)); // goes through (0,0)
        let mut answer = intersect(&s0, &s1);
        reporter_assert!(reporter, answer.is_none());
        answer = intersect(&s1, &s0);
        reporter_assert!(reporter, answer.is_none());
    }
});

def_test!(BO_lessAtBasic, |reporter| {
    {
        // Parallel lines
        let s0 = Segment::new(Point::new(-1, 1), Point::new(-1, -1));
        let s1 = Segment::new(Point::new(1, 1), Point::new(1, -1));
        reporter_assert!(reporter, less_than_at(&s0, &s1, -1));
        reporter_assert!(reporter, !less_than_at(&s1, &s0, -1));
        reporter_assert!(reporter, less_than_at(&s0, &s1, 0));
        reporter_assert!(reporter, !less_than_at(&s1, &s0, 0));
        reporter_assert!(reporter, less_than_at(&s0, &s1, 1));
        reporter_assert!(reporter, !less_than_at(&s1, &s0, 1));
    }
    {
        // Crossed lines
        let s0 = Segment::new(Point::new(-1, -1), Point::new(1, 1));
        let s1 = Segment::new(Point::new(1, -1), Point::new(-1, 1));
        reporter_assert!(reporter, less_than_at(&s0, &s1, -1));
        reporter_assert!(reporter, !less_than_at(&s1, &s0, -1));
        // When they are == neither is less.
        reporter_assert!(reporter, !less_than_at(&s0, &s1, 0));
        reporter_assert!(reporter, !less_than_at(&s1, &s0, 0));
        reporter_assert!(reporter, !less_than_at(&s0, &s1, 1));
        reporter_assert!(reporter, less_than_at(&s1, &s0, 1));
    }
    {
        // Near crossing
        let s0 = Segment::new(Point::new(0, -100), Point::new(0, 100));
        let s1 = Segment::new(Point::new(-3, 98), Point::new(3, 104));
        reporter_assert!(reporter, !less_than_at(&s0, &s1, 98));
        reporter_assert!(reporter, less_than_at(&s1, &s0, 98));
        reporter_assert!(reporter, !less_than_at(&s0, &s1, 99));
        reporter_assert!(reporter, less_than_at(&s1, &s0, 99));
        reporter_assert!(reporter, !less_than_at(&s0, &s1, 100));
        reporter_assert!(reporter, less_than_at(&s1, &s0, 100));
    }
});

def_test!(BO_compareSlopesBasic, |reporter| {
    {
        // Both horizontal
        let s0 = Segment::new(Point::new(-1, 1), Point::new(0, 1));
        let s1 = Segment::new(Point::new(-2, 1), Point::new(1, 1));
        reporter_assert!(reporter, compare_slopes(&s0, &s1) == 0);
        reporter_assert!(reporter, compare_slopes(&s1, &s0) == 0);
    }
    {
        // One horizontal
        let s0 = Segment::new(Point::new(-1, 1), Point::new(0, 0));
        let s1 = Segment::new(Point::new(-2, 1), Point::new(1, 1));
        reporter_assert!(reporter, compare_slopes(&s0, &s1) == -1);
        reporter_assert!(reporter, compare_slopes(&s1, &s0) == 1);
    }
    {
        // One vertical
        let s0 = Segment::new(Point::new(-1, 1), Point::new(-1, 0)); // Vertical
        let s1 = Segment::new(Point::new(-2, 1), Point::new(-1, 0));
        let s2 = Segment::new(Point::new(2, 1), Point::new(-1, 0));
        reporter_assert!(reporter, compare_slopes(&s0, &s1) == 1);
        reporter_assert!(reporter, compare_slopes(&s1, &s0) == -1);
        reporter_assert!(reporter, compare_slopes(&s0, &s2) == -1);
        reporter_assert!(reporter, compare_slopes(&s2, &s0) == 1);
    }
    {
        // Equal slope
        let s0 = Segment::new(Point::new(-2, 1), Point::new(0, 0));
        let s1 = Segment::new(Point::new(-4, 2), Point::new(0, 0));
        reporter_assert!(reporter, compare_slopes(&s0, &s1) == 0);
        reporter_assert!(reporter, compare_slopes(&s1, &s0) == 0);
    }
    {
        // Equal slope
        let s0 = Segment::new(Point::new(2, 1), Point::new(0, 0));
        let s1 = Segment::new(Point::new(4, 2), Point::new(0, 0));
        reporter_assert!(reporter, compare_slopes(&s0, &s1) == 0);
        reporter_assert!(reporter, compare_slopes(&s1, &s0) == 0);
    }
    {
        let s0 = Segment::new(Point::new(-2, 1), Point::new(0, 0));
        let s1 = Segment::new(Point::new(4, 2), Point::new(0, 0));
        reporter_assert!(reporter, compare_slopes(&s0, &s1) == -1);
        reporter_assert!(reporter, compare_slopes(&s1, &s0) == 1);
    }
    {
        let s0 = Segment::new(Point::new(-2, 1), Point::new(0, 0));
        let s1 = Segment::new(Point::new(-3, 1), Point::new(0, 0));
        reporter_assert!(reporter, compare_slopes(&s0, &s1) == 1);
        reporter_assert!(reporter, compare_slopes(&s1, &s0) == -1);
    }
});

def_test!(BO_rounded_point_less_than_segment_in_x_lower, |reporter| {
    {
        // Vertical segment
        let s = Segment::new(Point::new(3, -50), Point::new(3, 50));
        reporter_assert!(
            reporter,
            rounded_point_less_than_segment_in_x_lower(&s, Point::new(4, 7))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_lower(&s, Point::new(3, 7))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_lower(&s, Point::new(2, 7))
        );
    }
    {
        // Horizontal segment
        let s = Segment::new(Point::new(-10, 3), Point::new(10, 3));
        reporter_assert!(
            reporter,
            rounded_point_less_than_segment_in_x_lower(&s, Point::new(11, 3))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_lower(&s, Point::new(10, 3))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_lower(&s, Point::new(4, 3))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_lower(&s, Point::new(-10, 3))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_lower(&s, Point::new(-11, 3))
        );
    }
    {
        // Pass through {0, 0}
        let s = Segment::new(Point::new(-100, -100), Point::new(100, 100));
        reporter_assert!(
            reporter,
            rounded_point_less_than_segment_in_x_lower(&s, Point::new(1, 0))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_lower(&s, Point::new(0, 0))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_lower(&s, Point::new(-1, 0))
        );
    }
    {
        // Just left of {0, 0}, but still rounds to {0, 0}
        let s = Segment::new(Point::new(-100, -100), Point::new(199, 200));
        reporter_assert!(
            reporter,
            rounded_point_less_than_segment_in_x_lower(&s, Point::new(1, 0))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_lower(&s, Point::new(0, 0))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_lower(&s, Point::new(-1, 0))
        );
    }
    {
        // Just right of {0, 0}, but still rounds to {0, 0}
        let s = Segment::new(Point::new(-100, -100), Point::new(201, 200));
        reporter_assert!(
            reporter,
            rounded_point_less_than_segment_in_x_lower(&s, Point::new(1, 0))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_lower(&s, Point::new(0, 0))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_lower(&s, Point::new(-1, 0))
        );
    }
});

def_test!(BO_rounded_point_less_than_segment_in_x_upper, |reporter| {
    {
        // Vertical segment
        let s = Segment::new(Point::new(3, -50), Point::new(3, 50));
        reporter_assert!(
            reporter,
            rounded_point_less_than_segment_in_x_upper(&s, Point::new(4, 7))
        );
        reporter_assert!(
            reporter,
            rounded_point_less_than_segment_in_x_upper(&s, Point::new(3, 7))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_upper(&s, Point::new(2, 7))
        );
    }
    {
        // Horizontal segment
        let s = Segment::new(Point::new(-10, 3), Point::new(10, 3));
        reporter_assert!(
            reporter,
            rounded_point_less_than_segment_in_x_upper(&s, Point::new(11, 3))
        );
        reporter_assert!(
            reporter,
            rounded_point_less_than_segment_in_x_upper(&s, Point::new(10, 3))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_upper(&s, Point::new(4, 3))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_upper(&s, Point::new(-10, 3))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_upper(&s, Point::new(-11, 3))
        );
    }
    {
        // Pass through {0, 0}
        let s = Segment::new(Point::new(-100, -100), Point::new(100, 100));
        reporter_assert!(
            reporter,
            rounded_point_less_than_segment_in_x_upper(&s, Point::new(1, 0))
        );
        reporter_assert!(
            reporter,
            rounded_point_less_than_segment_in_x_upper(&s, Point::new(0, 0))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_upper(&s, Point::new(-1, 0))
        );
    }
    {
        // Just left of {0, 0}, but still rounds to {0, 0}
        let s = Segment::new(Point::new(-100, -100), Point::new(199, 200));
        reporter_assert!(
            reporter,
            rounded_point_less_than_segment_in_x_upper(&s, Point::new(1, 0))
        );
        reporter_assert!(
            reporter,
            rounded_point_less_than_segment_in_x_upper(&s, Point::new(0, 0))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_upper(&s, Point::new(-1, 0))
        );
    }
    {
        // Just right of {0, 0}, but still rounds to {0, 0}
        let s = Segment::new(Point::new(-100, -100), Point::new(201, 200));
        reporter_assert!(
            reporter,
            rounded_point_less_than_segment_in_x_upper(&s, Point::new(1, 0))
        );
        reporter_assert!(
            reporter,
            rounded_point_less_than_segment_in_x_upper(&s, Point::new(0, 0))
        );
        reporter_assert!(
            reporter,
            !rounded_point_less_than_segment_in_x_upper(&s, Point::new(-1, 0))
        );
    }
});
