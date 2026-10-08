// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bigrect.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;

// Port of: gm/bigrect.cpp#L18-L77 (chrome/m156)
fn draw_big_rect(canvas: &Canvas, big: f32, rect_paint: &Paint) {
    // Looks like this:
    // +--+-+----+-+----+
    // |  | |    | |    |
    // |--+-+----+-+----+
    // |--+-+----+-+----+
    // |  | |    | |    |
    // |  | |    +-+    |
    // +--+-+--+     +--+
    // +--+-+--+     +--+
    // |  | |    +-+    |
    // |  | |    | |    |
    // +--+-+----+-+----+

    canvas.clip_rect(Rect::new(0.0, 0.0, 35.0, 35.0), None, None);

    // Align to pixel boundaries.
    canvas.translate((0.5, 0.5));

    let horiz = Rect::from_ltrb(-big, 5.0, big, 10.0);
    canvas.draw_rect(horiz, rect_paint);

    let vert = Rect::from_ltrb(5.0, -big, 10.0, big);
    canvas.draw_rect(vert, rect_paint);

    let from_left = Rect::from_ltrb(-big, 20.0, 17.0, 25.0);
    canvas.draw_rect(from_left, rect_paint);

    let from_top = Rect::from_ltrb(20.0, -big, 25.0, 17.0);
    canvas.draw_rect(from_top, rect_paint);

    let from_right = Rect::from_ltrb(28.0, 20.0, big, 25.0);
    canvas.draw_rect(from_right, rect_paint);

    let from_bottom = Rect::from_ltrb(20.0, 28.0, 25.0, big);
    canvas.draw_rect(from_bottom, rect_paint);

    let left_border = Rect::from_ltrb(-2.0, -1.0, 0.0, 35.0);
    canvas.draw_rect(left_border, rect_paint);

    let top_border = Rect::from_ltrb(-1.0, -2.0, 35.0, 0.0);
    canvas.draw_rect(top_border, rect_paint);

    let right_border = Rect::from_ltrb(34.0, -1.0, 36.0, 35.0);
    canvas.draw_rect(right_border, rect_paint);

    let bottom_border = Rect::from_ltrb(-1.0, 34.0, 35.0, 36.0);
    canvas.draw_rect(bottom_border, rect_paint);

    let mut out_of_bounds_paint = Paint::default();
    out_of_bounds_paint.set_color(Color::RED);
    out_of_bounds_paint.set_style(Style::Stroke);
    out_of_bounds_paint.set_stroke_width(0.0);

    let out_of_bounds = Rect::from_ltrb(-1.0, -1.0, 35.0, 35.0);
    canvas.draw_rect(out_of_bounds, &out_of_bounds_paint);
}

// Port of: gm/bigrect.cpp#L79-L111 (chrome/m156)
crate::def_simple_gm!(bigrect, canvas, 325, 125, {
    // Test with sizes:
    //   - reasonable size (for comparison),
    //   - outside the range of int32, and
    //   - outside the range of SkFixed.
    let sizes: [f32; 3] = [100.0, 5e10f32, 1e6f32];

    for i in 0..8 {
        for (j, size) in sizes.iter().enumerate() {
            canvas.save();
            #[allow(clippy::cast_precision_loss)] // small loop indices
            canvas.translate(((i * 40 + 5) as f32, (j * 40 + 5) as f32));

            let mut paint = Paint::default();
            paint.set_color(Color::BLUE);
            // These are the three parameters that affect the behavior of skcpu::Draw::drawRect.
            if i & 1 != 0 {
                paint.set_style(Style::Fill);
            } else {
                paint.set_style(Style::Stroke);
            }
            if i & 2 != 0 {
                paint.set_stroke_width(1.0);
            } else {
                paint.set_stroke_width(0.0);
            }
            if i & 4 != 0 {
                paint.set_anti_alias(true);
            } else {
                paint.set_anti_alias(false);
            }

            draw_big_rect(canvas, *size, &paint);
            canvas.restore();
        }
    }
});
