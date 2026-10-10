// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/stroke_rect_shader.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::{Join, Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

const K_PAD: f32 = 20.0;

// Port of: gm/stroke_rect_shader.cpp#L13-L44 (chrome/m156), stroke_rect_shader
crate::def_simple_gm!(stroke_rect_shader, canvas, 690, 300, {
    let k_rect = Rect::new(0.0, 0.0, 100.0, 100.0);
    let k_pts = [
        Point::new(k_rect.left(), k_rect.top()),
        Point::new(k_rect.right(), k_rect.bottom()),
    ];
    let k_colors = [Color4f::from(Color::RED), Color4f::from(Color::BLUE)];
    let shader = shaders::linear_gradient(
        (k_pts[0], k_pts[1]),
        &Gradient::new(
            Colors::new(&k_colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    );

    // Do a large initial translate so that local coords disagree with device coords significantly
    // for the first rect drawn.
    canvas.translate((k_rect.center_x(), k_rect.center_y()));
    for aa in [false, true] {
        let mut paint = Paint::default();
        paint.set_shader(shader.clone());
        paint.set_style(Style::Stroke);
        paint.set_anti_alias(aa);
        canvas.save();
        let k_stroke_width: f32 = 10.0;
        paint.set_stroke_width(k_stroke_width);
        paint.set_stroke_join(Join::Bevel);
        canvas.draw_rect(k_rect, &paint);
        canvas.translate((k_rect.width() + K_PAD, 0.0));
        paint.set_stroke_join(Join::Miter);
        canvas.draw_rect(k_rect, &paint);
        canvas.translate((k_rect.width() + K_PAD, 0.0));
        // This miter limit should effectively produce a bevel join.
        paint.set_stroke_miter(0.01);
        canvas.draw_rect(k_rect, &paint);
        canvas.translate((k_rect.width() + K_PAD, 0.0));
        paint.set_stroke_join(Join::Round);
        canvas.draw_rect(k_rect, &paint);
        canvas.translate((k_rect.width() + K_PAD, 0.0));
        paint.set_stroke_width(0.0);
        canvas.draw_rect(k_rect, &paint);
        canvas.restore();
        canvas.translate((0.0, k_rect.height() + K_PAD));
    }
});
