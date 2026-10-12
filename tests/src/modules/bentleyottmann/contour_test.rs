// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/bentleyottmann/tests/ContourTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_bentleyottmann::contour::Contours;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::rect::IRect;

use crate::{def_test, reporter_assert};

/// `v * Contours::kScaleFactor`, the value the C++ compares the rounded points against.
fn scaled(v: i32) -> f64 {
    f64::from(v) * Contours::SCALE_FACTOR
}

/// `v * Contours::kScaleFactor` as the `int32_t` that `SkIRect::MakeLTRB` takes.
#[allow(clippy::cast_possible_truncation)] // the scaled test values are exact small integers
fn scaled_i32(v: i32) -> i32 {
    scaled(v) as i32
}

def_test!(
    #[allow(clippy::float_cmp)]
    CFC_Contours_Basic,
    |r| {
        {
            // No contours basic
            let p = Path::new();
            let contours = Contours::make(&p);
            reporter_assert!(r, contours.is_empty());
        }
        {
            // Path with just a close.
            let mut b = PathBuilder::new();
            b.close();
            let p = b.detach();
            let contours = Contours::make(&p);
            reporter_assert!(r, contours.is_empty());
        }
        {
            // Path with a bunch of moves.
            let mut b = PathBuilder::new();
            b.move_to((10, 10));
            b.move_to((20, 20));
            b.close();
            let p = b.detach();
            let contours = Contours::make(&p);
            reporter_assert!(r, contours.is_empty());
        }
        {
            // LineTo, but no close.
            let mut b = PathBuilder::new();
            b.move_to((10, 10));
            b.move_to((20, 20));
            b.line_to((30, 30));
            let p = b.detach();
            let contours = Contours::make(&p);
            reporter_assert!(r, contours.len() == 1);
            let c = contours.get(0);
            reporter_assert!(r, c.points.len() == 2);
            reporter_assert!(r, f64::from(c.points[0].x) == scaled(20));
            reporter_assert!(r, f64::from(c.points[1].x) == scaled(30));
            reporter_assert!(
                r,
                c.bounds
                    == IRect::from_ltrb(
                        scaled_i32(20),
                        scaled_i32(20),
                        scaled_i32(30),
                        scaled_i32(30)
                    )
            );
        }
        {
            // LineTo with close.
            let mut b = PathBuilder::new();
            b.move_to((10, 10));
            b.move_to((20, 20));
            b.line_to((30, 30));
            b.close();
            let p = b.detach();
            let contours = Contours::make(&p);
            reporter_assert!(r, contours.len() == 1);
            let c = contours.get(0);
            reporter_assert!(r, c.points.len() == 3);
            reporter_assert!(r, f64::from(c.points[0].x) == scaled(20));
            reporter_assert!(r, f64::from(c.points[1].x) == scaled(30));
            // Extra point added by close.
            reporter_assert!(r, f64::from(c.points[2].x) == scaled(20));
            reporter_assert!(
                r,
                c.bounds
                    == IRect::from_ltrb(
                        scaled_i32(20),
                        scaled_i32(20),
                        scaled_i32(30),
                        scaled_i32(30)
                    )
            );
        }
        {
            // LineTo with close and extra moves.
            let mut b = PathBuilder::new();
            b.move_to((10, 10));
            b.move_to((20, 20));
            b.line_to((30, 30));
            b.close();
            b.move_to((100, 100));
            let p = b.detach();
            let contours = Contours::make(&p);
            reporter_assert!(r, contours.len() == 1);
            let c = contours.get(0);
            reporter_assert!(r, c.points.len() == 3);
            reporter_assert!(r, f64::from(c.points[0].x) == scaled(20));
            reporter_assert!(r, f64::from(c.points[1].x) == scaled(30));
            // Extra point added by close.
            reporter_assert!(r, f64::from(c.points[2].x) == scaled(20));
            reporter_assert!(
                r,
                c.bounds
                    == IRect::from_ltrb(
                        scaled_i32(20),
                        scaled_i32(20),
                        scaled_i32(30),
                        scaled_i32(30)
                    )
            );
        }
    }
);
