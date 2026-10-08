// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/points.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::PointMode;
use skia_rust_core::paint::{Cap, Paint};
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;

const N: usize = 99;

// Port of: gm/points.cpp#L29-L82 (chrome/m156)
struct PointsGM;

impl PointsGM {
    // Port of: gm/points.cpp#L38-L47 (chrome/m156)
    fn fill_pts(pts: &mut [Point], rand: &mut Random) {
        for p in pts {
            // Compute these independently and store in variables, rather
            // than in the parameter-passing expression, to get consistent
            // evaluation order across compilers.
            let y = rand.next_u_scalar1() * 480.0;
            let x = rand.next_u_scalar1() * 640.0;
            *p = Point::new(x, y);
        }
    }
}

impl GM for PointsGM {
    fn name(&self) -> String {
        "points".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 490)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((1.0, 1.0));

        let mut rand = Random::default();
        let mut p0 = Paint::default();
        let mut p1 = Paint::default();
        let mut p2 = Paint::default();
        let mut p3 = Paint::default();

        p0.set_color(Color::RED);
        p1.set_color(Color::GREEN);
        p2.set_color(Color::BLUE);
        p3.set_color(Color::WHITE);

        p0.set_stroke_width(4.0);
        p2.set_stroke_cap(Cap::Round);
        p2.set_stroke_width(6.0);

        let mut pts = vec![Point::default(); N];
        Self::fill_pts(&mut pts, &mut rand);

        canvas.draw_points(PointMode::Polygon, &pts, &p0);
        canvas.draw_points(PointMode::Lines, &pts, &p1);
        canvas.draw_points(PointMode::Points, &pts, &p2);
        canvas.draw_points(PointMode::Points, &pts, &p3);
    }
}

// Port of: gm/points.cpp#L83 (chrome/m156)
crate::def_gm!(PointsGM_ = "PointsGM", PointsGM);
