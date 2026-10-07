// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/thinstrokedrects.cpp (chrome/m156)

// GM ports mirror the C++ source line by line: literals, short names, local constants, int/float
// conversions, index loops and long bodies are kept as they are there.
#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::trivially_copy_pass_by_ref,
    clippy::write_with_newline,
    clippy::excessive_precision,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;

// Draw rects with various stroke widths at 1/8 pixel increments
// Port of: gm/thinstrokedrects.cpp#L22-L77 (chrome/m156)
struct ThinStrokedRectsGm;

impl GM for ThinStrokedRectsGm {
    fn name(&self) -> String {
        "thinstrokedrects".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(240, 320)
    }

    fn bg_color(&self) -> Color {
        Color::new(0xFF000000)
    }

    #[allow(clippy::cast_precision_loss)] // i * 0.125f
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_color(Color::WHITE);
        paint.set_style(Style::Stroke);
        paint.set_anti_alias(true);

        const RECT: Rect = Rect::new(0.0, 0.0, 10.0, 10.0);
        const RECT2: Rect = Rect::new(0.0, 0.0, 20.0, 20.0);

        const STROKE_WIDTHS: [f32; 7] = [4.0, 2.0, 1.0, 0.5, 0.25, 0.125, 0.0];

        canvas.translate((5.0, 5.0));
        for i in 0..8 {
            canvas.save();
            canvas.translate((i as f32 * 0.125, i as f32 * 30.0));
            for stroke_width in STROKE_WIDTHS {
                paint.set_stroke_width(stroke_width);
                canvas.draw_rect(RECT, &paint);
                canvas.translate((15.0, 0.0));
            }
            canvas.restore();
        }

        // Draw a second time in red with a scale
        paint.set_color(Color::RED);
        canvas.translate((0.0, 15.0));
        for i in 0..8 {
            canvas.save();
            canvas.translate((i as f32 * 0.125, i as f32 * 30.0));
            canvas.scale((0.5, 0.5));
            for stroke_width in STROKE_WIDTHS {
                paint.set_stroke_width(2.0 * stroke_width);
                canvas.draw_rect(RECT2, &paint);
                canvas.translate((30.0, 0.0));
            }
            canvas.restore();
        }
    }
}

// Port of: gm/thinstrokedrects.cpp#L79 (chrome/m156)
crate::def_gm!(ThinStrokedRectsGM, ThinStrokedRectsGm);
