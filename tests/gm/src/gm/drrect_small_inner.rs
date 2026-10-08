// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/drrect_small_inner.cpp (chrome/m156)

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
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

// Port of: gm/drrect_small_inner.cpp#L17-L46 (chrome/m156)
#[allow(clippy::float_cmp)] // exact constant comparisons as in C++
fn draw_drrect_small_inner(canvas: &Canvas) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    const OUTER_RADIUS: f32 = 35.0;
    let outer = RRect::new_oval(Rect::from_xywh(
        0.0,
        0.0,
        2.0 * OUTER_RADIUS,
        2.0 * OUTER_RADIUS,
    ));
    canvas.translate((10.0, 10.0));
    canvas.save();
    for offcenter in [false, true] {
        for oval in [false, true] {
            for inner_radius_x in [1.0_f32, 0.5, 0.1, 0.01] {
                let mut inner_radius_y = inner_radius_x;
                if oval {
                    inner_radius_y *= 0.95;
                }
                let mut tx = OUTER_RADIUS - inner_radius_x;
                let ty = OUTER_RADIUS - inner_radius_y;
                if offcenter {
                    tx += 1.0;
                }
                let inner = RRect::new_oval(Rect::from_xywh(
                    tx,
                    ty,
                    2.0 * inner_radius_x,
                    2.0 * inner_radius_y,
                ));
                canvas.draw_drrect(outer, inner, &paint);
                canvas.translate((0.0, 2.0 * OUTER_RADIUS + 5.0));
            }
        }
        canvas.restore();
        canvas.translate((2.0 * OUTER_RADIUS + 2.0, 0.0));
    }
    canvas.restore();
}

// Port of: gm/drrect_small_inner.cpp#L17-L46 (chrome/m156)
crate::def_simple_gm!(drrect_small_inner, canvas, 170, 610, {
    draw_drrect_small_inner(canvas);
});
