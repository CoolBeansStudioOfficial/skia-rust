// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_1086705.cpp (chrome/m156)

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
use skia_rust_core::point::Point;
use skia_rust_core::scalar::{SCALAR_PI, scalar_cos, scalar_sin};

// See crbug.com/1086705. The convex linearizing path renderer would collapse too many of the
// very-near duplicate vertices and turn the path into a triangle. Since the stroke width is larger
// than the radius of the circle, there's the separate issue of what to do when stroke
// self-intersects
// Port of: gm/crbug_1086705.cpp#L17-L38 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // int to scalar, as in C++
fn draw_crbug_1086705(canvas: &Canvas) {
    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(5.0);
    paint.set_anti_alias(true);

    let mut circle_vertices = [Point::default(); 700];
    for (i, v) in circle_vertices.iter_mut().enumerate() {
        let angle_rads = 2.0 * SCALAR_PI * (i as f32) / 700.0;
        *v = Point::new(
            100.0 + 2.0 * scalar_cos(angle_rads),
            100.0 + 2.0 * scalar_sin(angle_rads),
        );
    }

    let mut circle = PathBuilder::new();
    circle.move_to(circle_vertices[0]);
    for v in &circle_vertices[1..] {
        circle.line_to(*v);
    }
    circle.close();

    canvas.draw_path(&circle.detach(), &paint);
}

// Port of: gm/crbug_1086705.cpp#L17-L38 (chrome/m156)
crate::def_simple_gm!(crbug_1086705, canvas, 200, 200, {
    draw_crbug_1086705(canvas);
});
