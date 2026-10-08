// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_1113794.cpp (chrome/m156)

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
use skia_rust_core::matrix::{Matrix, ScaleToFit};
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_effects::dash_path_effect;

// Port of: gm/crbug_1113794.cpp#L15-L32 (chrome/m156)
crate::def_simple_gm!(crbug_1113794, canvas, 600, 200, {
    let path = Path::line(Point::new(50.0, 80.0), Point::new(50.0, 20.0));

    let mut paint = Paint::default();
    paint.set_color(Color::BLACK);
    paint.set_anti_alias(true);
    paint.set_stroke_width(0.25);
    paint.set_style(Style::Stroke);

    let dash = [10.0_f32, 10.0];
    paint.set_path_effect(dash_path_effect::new(&dash, 0.0));

    let view_box = Matrix::rect_to_rect_or_identity(
        Rect::from_wh(100.0, 100.0),
        Rect::from_wh(600.0, 200.0),
        ScaleToFit::Fill,
    );
    canvas.concat(&view_box);

    canvas.draw_path(&path, &paint);
});
