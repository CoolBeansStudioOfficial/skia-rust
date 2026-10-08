// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/testgradient.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::color::colors;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// Port of: gm/testgradient.cpp#L22-L69 (chrome/m156)
struct TestGradientGm;

impl GM for TestGradientGm {
    fn name(&self) -> String {
        "testgradient".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(800, 800)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        // Set up a gradient paint for a rect.
        // And non-gradient paint for other objects.
        canvas.draw_color(Color::WHITE, None);

        let mut paint = Paint::default();
        paint.set_style(Style::Fill);
        paint.set_anti_alias(true);
        paint.set_stroke_width(4.0);
        paint.set_color(Color::new(0xFFFE_938C));

        let mut rect = Rect::from_xywh(10.0, 10.0, 100.0, 160.0);

        let points = [Point::new(0.0, 0.0), Point::new(256.0, 256.0)];
        let colors = [colors::BLUE, colors::YELLOW];
        let mut new_paint = paint.clone();
        new_paint.set_shader(shaders::linear_gradient(
            (points[0], points[1]),
            &Gradient::new(
                Colors::new(&colors, None, TileMode::Clamp, None),
                Interpolation::default(),
            ),
            None,
        ));
        canvas.draw_rect(rect, &new_paint);

        let mut oval = RRect::default();
        oval.set_oval(rect);
        oval.offset((40.0, 80.0));
        paint.set_color(Color::new(0xFFE6_B89C));
        canvas.draw_rrect(oval, &paint);

        paint.set_color(Color::new(0xFF9C_AFB7));
        canvas.draw_circle((180.0, 50.0), 25.0, &paint);

        rect.offset((80.0, 50.0));
        paint.set_color(Color::new(0xFF42_81A4));
        paint.set_style(Style::Stroke);
        canvas.draw_round_rect(rect, 10.0, 10.0, &paint);
    }
}

// Register the GM
// Port of: gm/testgradient.cpp#L73 (chrome/m156)
crate::def_gm!(TestGradientGM, TestGradientGm);
