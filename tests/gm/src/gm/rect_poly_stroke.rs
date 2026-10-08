// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/rect_poly_stroke.cpp (chrome/m156)

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
use skia_rust_core::paint::{Join, Paint};
use skia_rust_core::path::Path;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

// Port of: gm/rect_poly_stroke.cpp#L14-L21 (chrome/m156)
fn draw_rect_as_rect_proc(canvas: &Canvas, rect: &Rect, paint: &Paint) {
    canvas.draw_rect(rect, paint);
}

// Port of: gm/rect_poly_stroke.cpp#L19-L21 (chrome/m156)
fn draw_rect_with_path_proc(canvas: &Canvas, rect: &Rect, paint: &Paint) {
    canvas.draw_path(&Path::rect(rect, None), paint);
}

type DrawRectProc = fn(&Canvas, &Rect, &Paint);

const GM_WIDTH: i32 = 1150;
const GM_HEIGHT: i32 = 920;

// Port of: gm/rect_poly_stroke.cpp#L25-L89 (chrome/m156)
#[allow(clippy::too_many_lines)]
fn draw_rect_poly_stroke(canvas: &Canvas) {
    struct DrawRec {
        proc_: DrawRectProc,
        color: Color,
    }
    let recs = [
        DrawRec {
            proc_: draw_rect_as_rect_proc,
            color: Color::BLACK,
        },
        DrawRec {
            proc_: draw_rect_with_path_proc,
            color: Color::new(0xFF00_0088),
        },
    ];

    const W: f32 = 100.0;
    const H: f32 = 80.0;
    let rects = [
        Rect::from_ltrb(0.0, 0.0, W, H),
        Rect::from_ltrb(0.0, 0.0, W, 0.0),
        Rect::from_ltrb(0.0, 0.0, 0.0, H),
        // we don't expect this to draw anything
        Rect::from_ltrb(0.0, 0.0, 0.0, 0.0),
    ];
    const SPACING: f32 = 150.0;

    let degrees = [0.0_f32, -30.0];

    const THICKNESS: f32 = 20.0;

    let joins = [Join::Miter, Join::Round, Join::Bevel];

    canvas.translate((30.0, 50.0));

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_stroke(true);
    paint.set_stroke_width(THICKNESS);
    for j in joins {
        paint.set_stroke_join(j);

        canvas.save();
        for r in &rects {
            for angle in degrees {
                canvas.save();
                for rec in &recs {
                    canvas.save();
                    canvas.rotate(angle, Some(Point::new(r.center_x(), r.center_y())));
                    {
                        paint.set_stroke_width(THICKNESS);
                        paint.set_color(rec.color);
                        (rec.proc_)(canvas, r, &paint);

                        paint.set_stroke_width(0.0);
                        paint.set_color(Color::GREEN);
                        (rec.proc_)(canvas, r, &paint);
                    }
                    canvas.restore();
                    canvas.translate((0.0, SPACING));
                }
                canvas.restore();
                canvas.translate((SPACING, 0.0));
            }
        }
        canvas.restore();
        canvas.translate((0.0, 2.0 * SPACING));
    }
}

// Port of: gm/rect_poly_stroke.cpp#L25-L89 (chrome/m156)
crate::def_simple_gm!(rect_poly_stroke, canvas, GM_WIDTH, GM_HEIGHT, {
    draw_rect_poly_stroke(canvas);
});
