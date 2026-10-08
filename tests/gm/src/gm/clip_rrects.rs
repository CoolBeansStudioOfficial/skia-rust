// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/clip_rrects.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

// Tests clipRRect cases that Graphite should handle with an analytic shader.
// Port of: gm/clip_rrects.cpp#L19-L113 (chrome/m156)
crate::def_simple_gm!(clip_rrects_analytic, canvas, 360, 325, {
    let base_rect = Rect::new(0.0, 0.0, 32.0, 32.0);

    let make_rrect = |tl: bool, tr: bool, br: bool, bl: bool| {
        let corner_radius = 8.0;
        let r = |on: bool| {
            let v = if on { corner_radius } else { 0.0 };
            Vector::new(v, v)
        };
        let radii: [Vector; 4] = [r(tl), r(tr), r(br), r(bl)];
        RRect::new_rect_radii(base_rect, &radii)
    };

    // Test all combinations of circular corners
    let rrects = [
        make_rrect(false, false, false, false),
        make_rrect(true, false, false, false),
        make_rrect(false, true, false, false),
        make_rrect(false, false, true, false),
        make_rrect(false, false, false, true),
        make_rrect(true, true, false, false),
        make_rrect(true, false, true, false),
        make_rrect(true, false, false, true),
        make_rrect(false, true, true, false),
        make_rrect(false, true, false, true),
        make_rrect(false, false, true, true),
        make_rrect(true, true, true, false),
        make_rrect(true, true, false, true),
        make_rrect(true, false, true, true),
        make_rrect(false, true, true, true),
        make_rrect(true, true, true, true),
    ];

    let transforms: [(Point, Matrix); 4] = [
        // Identity (top-left grid)
        (Point::new(0.0, 0.0), Matrix::new_identity()),
        // Non-uniform scale (top-right grid)
        (Point::new(160.0, 0.0), Matrix::scale((1.2, 0.8))),
        // Orthgonal (bot-left grid)
        (
            Point::new(20.0, 160.0),
            Matrix::rotate_deg_pivot(
                10.0,
                Point::new(2.0 * base_rect.width(), 2.0 * base_rect.height()),
            ),
        ),
        // Fully affine (bot-right grid)
        (Point::new(180.0, 140.0), Matrix::skew((0.2, 0.1))),
    ];

    // Make this a complicated enough shape that Graphite doesn't just apply the clip geometrically.
    let mut b = PathBuilder::new();
    b.add_rect(base_rect, None, None);
    let mut inset_rect = base_rect;
    inset_rect.inset((10.0, 10.0));
    b.add_rect(inset_rect, None, None);
    b.set_fill_type(PathFillType::EvenOdd);
    let p = b.detach();

    let mut fill_paint = Paint::default();
    fill_paint.set_color(Color::BLACK);
    fill_paint.set_anti_alias(true);

    for (origin, xform) in &transforms {
        canvas.save();
        canvas.translate((origin.x, origin.y));

        let mut x: i32 = 0;
        let mut y: i32 = 0;
        for clip in &rrects {
            canvas.save();
            canvas.concat(xform);
            #[allow(clippy::cast_precision_loss)] // int -> float in C++ arithmetic
            canvas.translate((
                5.0 * (x + 1) as f32 + base_rect.width() * x as f32,
                5.0 * (y + 1) as f32 + base_rect.height() * y as f32,
            ));
            canvas.clip_rrect(clip, None, true);
            canvas.draw_path(&p, &fill_paint);
            canvas.restore();

            x += 1;
            if x >= 4 {
                x = 0;
                y += 1;
            }
        }

        canvas.restore();
    }
});
