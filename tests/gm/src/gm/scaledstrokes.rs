// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/scaledstrokes.cpp (chrome/m156)

#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::excessive_precision,
    clippy::float_cmp,
    clippy::inconsistent_digit_grouping,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::rect::Rect;

// Port of: gm/scaledstrokes.cpp#L26-L35 (chrome/m156)
fn draw_path(size: f32, canvas: &Canvas, paint: &Paint) {
    let c = 0.551_915_024_494_f32 * size;
    let path = PathBuilder::new()
        .move_to((0.0, size))
        .cubic_to((c, size), (size, c), (size, 0.0))
        .cubic_to((size, -c), (c, -size), (0.0, -size))
        .cubic_to((-c, -size), (-size, -c), (-size, 0.0))
        .cubic_to((-size, c), (-c, size), (0.0, size))
        .detach();
    canvas.draw_path(&path, paint);
}

// Port of: gm/scaledstrokes.cpp#L17-L80 (chrome/m156)
struct ScaledStrokesGm;

impl GM for ScaledStrokesGm {
    fn name(&self) -> String {
        "scaledstrokes".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 320)
    }

    #[allow(clippy::cast_precision_loss)] // int loop indices, as in C++
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_style(Style::Stroke);
        canvas.translate((5.0, 5.0));
        let size: f32 = 60.0;
        for i in 0..2 {
            paint.set_anti_alias(i == 1);
            for j in 0..4 {
                let scale = 4.0 - j as f32;
                paint.set_stroke_width(4.0 / scale);

                canvas.save();
                canvas.translate((size / 2.0, size / 2.0));
                canvas.scale((scale, scale));
                draw_path(size / 2.0 / scale, canvas, &paint);
                canvas.restore();

                canvas.save();
                canvas.translate((size / 2.0, 80.0 + size / 2.0));
                canvas.scale((scale, scale));
                canvas.draw_circle((0.0, 0.0), size / 2.0 / scale, &paint);
                canvas.restore();

                canvas.save();
                canvas.translate((0.0, 160.0));
                canvas.scale((scale, scale));
                canvas.draw_rect(
                    Rect::from_xywh(0.0, 0.0, size / scale, size / scale),
                    &paint,
                );
                canvas.restore();

                canvas.save();
                canvas.translate((0.0, 240.0));
                canvas.scale((scale, scale));
                canvas.draw_line((0.0, 0.0), (size / scale, size / scale), &paint);
                canvas.restore();

                canvas.translate((80.0, 0.0));
            }
        }
    }
}

crate::def_gm!(ScaledStrokesGM, ScaledStrokesGm);
