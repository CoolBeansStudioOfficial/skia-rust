// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/polygonoffset.cpp (chrome/m156)

// This GM is intended to exercise the offsetting of polygons. It is written against the
// `PolygonOffsetData` tables of the C++ source, which are ported below.

#![allow(
    clippy::cast_precision_loss,
    clippy::excessive_precision,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use crate::prelude::*;
use crate::tool_utils;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{degrees_to_radians, scalar, scalar_cos, scalar_sin};
use skia_rust_core::utils::poly_utils::{inset_convex_polygon, offset_simple_polygon};

const K_NUM_PATHS: usize = 20;
const K_MAX_PATH_HEIGHT: i32 = 100;
const K_MAX_OUTSET: i32 = 16;
const K_GM_WIDTH: i32 = 512;
const K_GM_HEIGHT: i32 = 512;

// Tables of `PolygonOffsetData` (gm/polygonoffset.cpp#L27-L558).
const G_POINTS0: &[Point] = &[
    Point::new(-1.5_f32, -50.0_f32),
    Point::new(1.5_f32, -50.0_f32),
    Point::new(1.5_f32, 50.0_f32),
    Point::new(-1.5_f32, 50.0_f32),
];
// narrow rect on an angle
const G_POINTS1: &[Point] = &[
    Point::new(-50.0_f32, -49.0_f32),
    Point::new(-49.0_f32, -50.0_f32),
    Point::new(50.0_f32, 49.0_f32),
    Point::new(49.0_f32, 50.0_f32),
];
// trap - narrow on top - wide on bottom
const G_POINTS2: &[Point] = &[
    Point::new(-10.0_f32, -50.0_f32),
    Point::new(10.0_f32, -50.0_f32),
    Point::new(50.0_f32, 50.0_f32),
    Point::new(-50.0_f32, 50.0_f32),
];
// wide skewed rect
const G_POINTS3: &[Point] = &[
    Point::new(-50.0_f32, -50.0_f32),
    Point::new(0.0_f32, -50.0_f32),
    Point::new(50.0_f32, 50.0_f32),
    Point::new(0.0_f32, 50.0_f32),
];
// thin rect with colinear-ish lines
const G_POINTS4: &[Point] = &[
    Point::new(-6.0_f32, -50.0_f32),
    Point::new(4.0_f32, -50.0_f32),
    Point::new(5.0_f32, -25.0_f32),
    Point::new(6.0_f32, 0.0_f32),
    Point::new(5.0_f32, 25.0_f32),
    Point::new(4.0_f32, 50.0_f32),
    Point::new(-4.0_f32, 50.0_f32),
];
// degenerate
const G_POINTS5: &[Point] = &[
    Point::new(-0.025_f32, -0.025_f32),
    Point::new(0.025_f32, -0.025_f32),
    Point::new(0.025_f32, 0.025_f32),
    Point::new(-0.025_f32, 0.025_f32),
];
// Quad with near coincident point
const G_POINTS6: &[Point] = &[
    Point::new(-20.0_f32, -13.0_f32),
    Point::new(-20.0_f32, -13.05_f32),
    Point::new(20.0_f32, -13.0_f32),
    Point::new(20.0_f32, 27.0_f32),
];
// thin rect with colinear lines
const G_POINTS7: &[Point] = &[
    Point::new(-10.0_f32, -50.0_f32),
    Point::new(10.0_f32, -50.0_f32),
    Point::new(10.0_f32, -20.0_f32),
    Point::new(10.0_f32, 0.0_f32),
    Point::new(10.0_f32, 35.0_f32),
    Point::new(10.0_f32, 50.0_f32),
    Point::new(-10.0_f32, 50.0_f32),
];
// capped teardrop
const G_POINTS8: &[Point] = &[
    Point::new(50.00_f32, 50.00_f32),
    Point::new(0.00_f32, 50.00_f32),
    Point::new(-15.45_f32, 47.55_f32),
    Point::new(-29.39_f32, 40.45_f32),
    Point::new(-40.45_f32, 29.39_f32),
    Point::new(-47.55_f32, 15.45_f32),
    Point::new(-50.00_f32, 0.00_f32),
    Point::new(-47.55_f32, -15.45_f32),
    Point::new(-40.45_f32, -29.39_f32),
    Point::new(-29.39_f32, -40.45_f32),
    Point::new(-15.45_f32, -47.55_f32),
    Point::new(0.00_f32, -50.00_f32),
    Point::new(50.00_f32, -50.00_f32),
];
// teardrop
const G_POINTS9: &[Point] = &[
    Point::new(4.39_f32, 40.45_f32),
    Point::new(-9.55_f32, 47.55_f32),
    Point::new(-25.00_f32, 50.00_f32),
    Point::new(-40.45_f32, 47.55_f32),
    Point::new(-54.39_f32, 40.45_f32),
    Point::new(-65.45_f32, 29.39_f32),
    Point::new(-72.55_f32, 15.45_f32),
    Point::new(-75.00_f32, 0.00_f32),
    Point::new(-72.55_f32, -15.45_f32),
    Point::new(-65.45_f32, -29.39_f32),
    Point::new(-54.39_f32, -40.45_f32),
    Point::new(-40.45_f32, -47.55_f32),
    Point::new(-25.0_f32, -50.0_f32),
    Point::new(-9.55_f32, -47.55_f32),
    Point::new(4.39_f32, -40.45_f32),
    Point::new(75.00_f32, 0.00_f32),
];
// clipped triangle
const G_POINTS10: &[Point] = &[
    Point::new(-10.0_f32, -50.0_f32),
    Point::new(10.0_f32, -50.0_f32),
    Point::new(50.0_f32, 31.0_f32),
    Point::new(40.0_f32, 50.0_f32),
    Point::new(-40.0_f32, 50.0_f32),
    Point::new(-50.0_f32, 31.0_f32),
];

// tab
const G_POINTS11: &[Point] = &[
    Point::new(-45.0, -25.0),
    Point::new(45.0, -25.0),
    Point::new(45.0, 25.0),
    Point::new(20.0, 25.0),
    Point::new(19.6157_f32, 25.0_f32 + 3.9018_f32),
    Point::new(18.4776_f32, 25.0_f32 + 7.6537_f32),
    Point::new(16.6294_f32, 25.0_f32 + 11.1114_f32),
    Point::new(14.1421_f32, 25.0_f32 + 14.1421_f32),
    Point::new(11.1114_f32, 25.0_f32 + 16.6294_f32),
    Point::new(7.6537_f32, 25.0_f32 + 18.4776_f32),
    Point::new(3.9018_f32, 25.0_f32 + 19.6157_f32),
    Point::new(0.0, 45.0_f32),
    Point::new(-3.9018_f32, 25.0_f32 + 19.6157_f32),
    Point::new(-7.6537_f32, 25.0_f32 + 18.4776_f32),
    Point::new(-11.1114_f32, 25.0_f32 + 16.6294_f32),
    Point::new(-14.1421_f32, 25.0_f32 + 14.1421_f32),
    Point::new(-16.6294_f32, 25.0_f32 + 11.1114_f32),
    Point::new(-18.4776_f32, 25.0_f32 + 7.6537_f32),
    Point::new(-19.6157_f32, 25.0_f32 + 3.9018_f32),
    Point::new(-20.0, 25.0),
    Point::new(-45.0, 25.0),
];

// star of david
const G_POINTS12: &[Point] = &[
    Point::new(0.0_f32, -50.0_f32),
    Point::new(14.43_f32, -25.0_f32),
    Point::new(43.30_f32, -25.0_f32),
    Point::new(28.86_f32, 0.0_f32),
    Point::new(43.30_f32, 25.0_f32),
    Point::new(14.43_f32, 25.0_f32),
    Point::new(0.0_f32, 50.0_f32),
    Point::new(-14.43_f32, 25.0_f32),
    Point::new(-43.30_f32, 25.0_f32),
    Point::new(-28.86_f32, 0.0_f32),
    Point::new(-43.30_f32, -25.0_f32),
    Point::new(-14.43_f32, -25.0_f32),
];

// notch
const K_BOTTOM: scalar = 25.0_f32;
const G_POINTS13: &[Point] = &[
    Point::new(-50.0, K_BOTTOM - 50.0_f32),
    Point::new(50.0, K_BOTTOM - 50.0_f32),
    Point::new(50.0, K_BOTTOM),
    Point::new(20.0, K_BOTTOM),
    Point::new(19.6157_f32, K_BOTTOM - 3.9018_f32),
    Point::new(18.4776_f32, K_BOTTOM - 7.6537_f32),
    Point::new(16.6294_f32, K_BOTTOM - 11.1114_f32),
    Point::new(14.1421_f32, K_BOTTOM - 14.1421_f32),
    Point::new(11.1114_f32, K_BOTTOM - 16.6294_f32),
    Point::new(7.6537_f32, K_BOTTOM - 18.4776_f32),
    Point::new(3.9018_f32, K_BOTTOM - 19.6157_f32),
    Point::new(0.0, K_BOTTOM - 20.0_f32),
    Point::new(-3.9018_f32, K_BOTTOM - 19.6157_f32),
    Point::new(-7.6537_f32, K_BOTTOM - 18.4776_f32),
    Point::new(-11.1114_f32, K_BOTTOM - 16.6294_f32),
    Point::new(-14.1421_f32, K_BOTTOM - 14.1421_f32),
    Point::new(-16.6294_f32, K_BOTTOM - 11.1114_f32),
    Point::new(-18.4776_f32, K_BOTTOM - 7.6537_f32),
    Point::new(-19.6157_f32, K_BOTTOM - 3.9018_f32),
    Point::new(-20.0, K_BOTTOM),
    Point::new(-50.0, K_BOTTOM),
];

// crown
const G_POINTS14: &[Point] = &[
    Point::new(-40.0, -39.0),
    Point::new(40.0, -39.0),
    Point::new(40.0, -20.0),
    Point::new(30.0, 40.0),
    Point::new(20.0, -20.0),
    Point::new(10.0, 40.0),
    Point::new(0.0, -20.0),
    Point::new(-10.0, 40.0),
    Point::new(-20.0, -20.0),
    Point::new(-30.0, 40.0),
    Point::new(-40.0, -20.0),
];

// dumbbell
const G_POINTS15: &[Point] = &[
    Point::new(-26.0, -3.0),
    Point::new(-24.0, -6.2_f32),
    Point::new(-22.5_f32, -8.0),
    Point::new(-20.0, -9.9_f32),
    Point::new(-17.5_f32, -10.3_f32),
    Point::new(-15.0, -10.9_f32),
    Point::new(-12.5_f32, -10.2_f32),
    Point::new(-10.0, -9.7_f32),
    Point::new(-7.5_f32, -8.1_f32),
    Point::new(-5.0, -7.7_f32),
    Point::new(-2.5_f32, -7.4_f32),
    Point::new(0.0, -7.7_f32),
    Point::new(3.0, -9.0),
    Point::new(6.5_f32, -11.5_f32),
    Point::new(10.6_f32, -14.0),
    Point::new(14.0, -15.2_f32),
    Point::new(17.0, -15.5_f32),
    Point::new(20.0, -15.2_f32),
    Point::new(23.4_f32, -14.0),
    Point::new(27.5_f32, -11.5_f32),
    Point::new(30.0, -8.0),
    Point::new(32.0, -4.0),
    Point::new(32.5_f32, 0.0),
    Point::new(32.0, 4.0),
    Point::new(30.0, 8.0),
    Point::new(27.5_f32, 11.5_f32),
    Point::new(23.4_f32, 14.0),
    Point::new(20.0, 15.2_f32),
    Point::new(17.0, 15.5_f32),
    Point::new(14.0, 15.2_f32),
    Point::new(10.6_f32, 14.0),
    Point::new(6.5_f32, 11.5_f32),
    Point::new(3.0, 9.0),
    Point::new(0.0, 7.7_f32),
    Point::new(-2.5_f32, 7.4_f32),
    Point::new(-5.0, 7.7_f32),
    Point::new(-7.5_f32, 8.1_f32),
    Point::new(-10.0, 9.7_f32),
    Point::new(-12.5_f32, 10.2_f32),
    Point::new(-15.0, 10.9_f32),
    Point::new(-17.5_f32, 10.3_f32),
    Point::new(-20.0, 9.9_f32),
    Point::new(-22.5_f32, 8.0),
    Point::new(-24.0, 6.2_f32),
    Point::new(-26.0, 3.0),
    Point::new(-26.5_f32, 0.0),
];

// truncated dumbbell
// (checks winding computation in OffsetSimplePolygon)
const G_POINTS16: &[Point] = &[
    Point::new(-15.0 + 3.0, -9.0),
    Point::new(-15.0 + 6.5_f32, -11.5_f32),
    Point::new(-15.0 + 10.6_f32, -14.0),
    Point::new(-15.0 + 14.0, -15.2_f32),
    Point::new(-15.0 + 17.0, -15.5_f32),
    Point::new(-15.0 + 20.0, -15.2_f32),
    Point::new(-15.0 + 23.4_f32, -14.0),
    Point::new(-15.0 + 27.5_f32, -11.5_f32),
    Point::new(-15.0 + 30.0, -8.0),
    Point::new(-15.0 + 32.0, -4.0),
    Point::new(-15.0 + 32.5_f32, 0.0),
    Point::new(-15.0 + 32.0, 4.0),
    Point::new(-15.0 + 30.0, 8.0),
    Point::new(-15.0 + 27.5_f32, 11.5_f32),
    Point::new(-15.0 + 23.4_f32, 14.0),
    Point::new(-15.0 + 20.0, 15.2_f32),
    Point::new(-15.0 + 17.0, 15.5_f32),
    Point::new(-15.0 + 14.0, 15.2_f32),
    Point::new(-15.0 + 10.6_f32, 14.0),
    Point::new(-15.0 + 6.5_f32, 11.5_f32),
    Point::new(-15.0 + 3.0, 9.0),
];

// square notch
// (to detect segment-segment intersection)
const G_POINTS17: &[Point] = &[
    Point::new(-50.0, K_BOTTOM - 50.0_f32),
    Point::new(50.0, K_BOTTOM - 50.0_f32),
    Point::new(50.0, K_BOTTOM),
    Point::new(20.0, K_BOTTOM),
    Point::new(20.0, K_BOTTOM - 20.0_f32),
    Point::new(-20.0, K_BOTTOM - 20.0_f32),
    Point::new(-20.0, K_BOTTOM),
    Point::new(-50.0, K_BOTTOM),
];

// box with Peano curve
const G_POINTS18: &[Point] = &[
    Point::new(0.0, 0.0),
    Point::new(0.0, -12.0),
    Point::new(-6.0, -12.0),
    Point::new(-6.0, 0.0),
    Point::new(-12.0, 0.0),
    Point::new(-12.0, -12.0),
    Point::new(-18.0, -12.0),
    Point::new(-18.0, 18.0),
    Point::new(-12.0, 18.0),
    Point::new(-12.0, 6.0),
    Point::new(-6.0, 6.0),
    Point::new(-6.0, 36.0),
    Point::new(-12.0, 36.0),
    Point::new(-12.0, 24.0),
    Point::new(-18.0, 24.0),
    Point::new(-18.0, 36.0),
    Point::new(-24.0, 36.0),
    Point::new(-24.0, 24.0),
    Point::new(-30.0, 24.0),
    Point::new(-30.0, 36.0),
    Point::new(-36.0, 36.0),
    Point::new(-36.0, 6.0),
    Point::new(-30.0, 6.0),
    Point::new(-30.0, 18.0),
    Point::new(-24.0, 18.0),
    Point::new(-24.0, -12.0),
    Point::new(-30.0, -12.0),
    Point::new(-30.0, 0.0),
    Point::new(-36.0, 0.0),
    Point::new(-36.0, -36.0),
    Point::new(36.0, -36.0),
    Point::new(36.0, 36.0),
    Point::new(12.0, 36.0),
    Point::new(12.0, 24.0),
    Point::new(6.0, 24.0),
    Point::new(6.0, 36.0),
    Point::new(0.0, 36.0),
    Point::new(0.0, 6.0),
    Point::new(6.0, 6.0),
    Point::new(6.0, 18.0),
    Point::new(12.0, 18.0),
    Point::new(12.0, -12.0),
    Point::new(6.0, -12.0),
    Point::new(6.0, 0.0),
];

const G_CONVEX_POINTS: &[&[Point]] = &[
    G_POINTS0, G_POINTS1, G_POINTS2, G_POINTS3, G_POINTS4, G_POINTS5, G_POINTS6, G_POINTS7,
    G_POINTS8, G_POINTS9, G_POINTS10,
];

const G_SIMPLE_POINTS: &[&[Point]] = &[
    G_POINTS0, G_POINTS1, G_POINTS2, G_POINTS4, G_POINTS5, G_POINTS7, G_POINTS8, G_POINTS11,
    G_POINTS12, G_POINTS13, G_POINTS14, G_POINTS15, G_POINTS16, G_POINTS17, G_POINTS18,
];

// Create a regular polygon with `n` points of width `w` and height `h`, wound in `dir`.
// Port of: gm/polygonoffset.cpp#L25-L41 (chrome/m156)
#[allow(clippy::many_single_char_names)] // mirrors the C++ names
fn create_ngon(n: usize, pts: &mut [Point], w: scalar, h: scalar, dir: PathDirection) {
    let mut angle_step: scalar = 360.0 / (n as scalar);
    let mut angle: scalar = 0.0;
    if n % 2 == 1 {
        angle = angle_step / 2.0;
    }
    if PathDirection::CCW == dir {
        angle = -angle;
        angle_step = -angle_step;
    }
    for pt in pts.iter_mut().take(n) {
        pt.x = -scalar_sin(degrees_to_radians(angle)) * w;
        pt.y = scalar_cos(degrees_to_radians(angle)) * h;
        angle += angle_step;
    }
}

// Port of: gm/polygonoffset.cpp#L178-L204 (chrome/m156)
fn get_convex_polygon(index: usize, dir: PathDirection) -> Vec<Point> {
    if index < G_CONVEX_POINTS.len() {
        // manually specified
        let points = G_CONVEX_POINTS[index];
        if PathDirection::CW == dir {
            points.to_vec()
        } else {
            points.iter().rev().copied().collect()
        }
    } else {
        // procedurally generated
        const NUM_PTS_ARRAY: [usize; 9] = [3, 4, 5, 5, 6, 8, 8, 20, 100];
        let mut width = (K_MAX_PATH_HEIGHT / 2) as scalar;
        let height = (K_MAX_PATH_HEIGHT / 2) as scalar;
        let array_index = index - G_CONVEX_POINTS.len();
        let num_pts = NUM_PTS_ARRAY[array_index];
        if array_index == 3 || array_index == 6 {
            // squashed pentagon and octagon
            width = (K_MAX_PATH_HEIGHT / 5) as scalar;
        }
        let mut data = vec![Point::default(); num_pts];
        create_ngon(num_pts, &mut data, width, height, dir);
        data
    }
}

// Port of: gm/polygonoffset.cpp#L206-L236 (chrome/m156)
fn get_simple_polygon(index: usize, dir: PathDirection) -> Vec<Point> {
    if index < G_SIMPLE_POINTS.len() {
        // manually specified
        let points = G_SIMPLE_POINTS[index];
        if PathDirection::CW == dir {
            points.to_vec()
        } else {
            points.iter().rev().copied().collect()
        }
    } else {
        // procedurally generated
        const NUM_PTS_ARRAY: [usize; 5] = [5, 7, 8, 20, 100];
        let array_index = (index - G_SIMPLE_POINTS.len()).min(NUM_PTS_ARRAY.len() - 1);
        let num_pts = NUM_PTS_ARRAY[array_index];
        // squash horizontally
        let width = (K_MAX_PATH_HEIGHT / 5) as scalar;
        let height = (K_MAX_PATH_HEIGHT / 2) as scalar;
        let mut data = vec![Point::default(); num_pts];
        create_ngon(num_pts, &mut data, width, height, dir);
        data
    }
}

// This GM tests the offsetting of convex (`convex_only`) or simple polygons.
// Port of: gm/polygonoffset.cpp#L238-L244 (chrome/m156)
struct PolygonOffsetGm {
    convex_only: bool,
}

impl PolygonOffsetGm {
    // Port of: gm/polygonoffset.cpp#L246-L249 (chrome/m156)
    fn new(convex_only: bool) -> Self {
        Self { convex_only }
    }

    // Draw a single polygon with insets and potentially outsets.
    // Port of: gm/polygonoffset.cpp#L370-L400 (chrome/m156)
    fn draw_polygon(&self, canvas: &Canvas, index: usize, position: &mut Point) {
        let center: Point;
        {
            let data = if self.convex_only {
                get_convex_polygon(index, PathDirection::CW)
            } else {
                get_simple_polygon(index, PathDirection::CW)
            };
            let mut bounds = Rect::bounds_or_empty(&data);
            if !self.convex_only {
                bounds.outset((K_MAX_OUTSET as scalar, K_MAX_OUTSET as scalar));
            }
            if position.x + bounds.width() > K_GM_WIDTH as scalar {
                position.x = 0.0;
                position.y += K_MAX_PATH_HEIGHT as scalar;
            }
            center = Point::new(position.x + (bounds.width() / 2.0), position.y);
            position.x += bounds.width();
        }

        let dirs = [PathDirection::CW, PathDirection::CCW];
        let insets: [scalar; 8] = [5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 35.0, 40.0];
        let offsets: [scalar; 11] = [
            2.0, 5.0, 9.0, 14.0, 20.0, 27.0, 35.0, 44.0, -2.0, -5.0, -9.0,
        ];
        let colors: [Color; 11] = [
            Color::new(0xFF90_1313),
            Color::new(0xFF8D_6214),
            Color::new(0xFF69_8B14),
            Color::new(0xFF1C_8914),
            Color::new(0xFF14_8755),
            Color::new(0xFF14_6C84),
            Color::new(0xFF14_2482),
            Color::new(0xFF4A_1480),
            Color::new(0xFF90_1313),
            Color::new(0xFF8D_6214),
            Color::new(0xFF69_8B14),
        ];

        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(1.0);

        let data = if self.convex_only {
            get_convex_polygon(index, dirs[index % 2])
        } else {
            get_simple_polygon(index, dirs[index % 2])
        };
        {
            let path = Path::polygon(&data, true, None, None);
            canvas.save();
            canvas.translate((center.x, center.y));
            canvas.draw_path(&path, &paint);
            canvas.restore();
        }

        let count = if self.convex_only {
            insets.len()
        } else {
            offsets.len()
        };
        // Like `SkTDArray<SkPoint> offsetPoly`, it is shared by the insets of this polygon.
        let mut offset_poly: Vec<Point> = Vec::new();
        for i in 0..count {
            let offset = if self.convex_only {
                insets[i]
            } else {
                offsets[i]
            };
            let result = if self.convex_only {
                inset_convex_polygon(&data, offset, &mut offset_poly)
            } else {
                let mut bounds = Rect::new_empty();
                let _ = bounds.set_bounds_check(&data);
                offset_simple_polygon(&data, &bounds, offset, &mut offset_poly, None)
            };
            if result {
                let path = Path::polygon(&offset_poly, true, None, None);
                paint.set_color(tool_utils::color_to_565(colors[i]));
                canvas.save();
                canvas.translate((center.x, center.y));
                canvas.draw_path(&path, &paint);
                canvas.restore();
            }
        }
    }
}

impl GM for PolygonOffsetGm {
    // Port of: gm/polygonoffset.cpp#L251-L258 (chrome/m156)
    fn name(&self) -> String {
        if self.convex_only {
            "convex-polygon-inset".to_string()
        } else {
            "simple-polygon-offset".to_string()
        }
    }

    // Port of: gm/polygonoffset.cpp#L260 (chrome/m156)
    fn size(&mut self) -> ISize {
        ISize::new(K_GM_WIDTH, K_GM_HEIGHT)
    }

    // Port of: gm/polygonoffset.cpp#L615-L616 (chrome/m156), `onDraw`
    fn on_draw(&mut self, canvas: &Canvas) {
        // the right edge of the last drawn path
        let mut offset = Point::new(0.0, (K_MAX_PATH_HEIGHT / 2) as scalar);
        if !self.convex_only {
            offset.y += K_MAX_OUTSET as scalar;
        }

        for i in 0..K_NUM_PATHS {
            self.draw_polygon(canvas, i, &mut offset);
        }
    }
}

// Port of: gm/polygonoffset.cpp#L614-L615 (chrome/m156)
crate::def_gm!(
    PolygonOffsetGM_true = "PolygonOffsetGM(true)",
    PolygonOffsetGm::new(true)
);
// Port of: gm/polygonoffset.cpp#L615 (chrome/m156)
crate::def_gm!(
    PolygonOffsetGM_false = "PolygonOffsetGM(false)",
    PolygonOffsetGm::new(false)
);
