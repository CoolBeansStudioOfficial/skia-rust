// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/drawlines_with_local_matrix.cpp (chrome/m156)

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
    clippy::unreadable_literal,
    clippy::unused_self
)]

use crate::prelude::*;
use skia_rust_core::canvas::PointMode;
use skia_rust_core::color::Color4f;
use skia_rust_core::paint::{Cap, Paint};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// Port of: gm/drawlines_with_local_matrix.cpp#L9-L49 (chrome/m156)
crate::def_simple_gm!(drawlines_with_local_matrix, canvas, 500, 500, {
    canvas.clip_rect(Rect::from_wh(500.0, 500.0), None, None);
    let mut grad = Paint::default();
    grad.set_anti_alias(true);
    grad.set_stroke_cap(Cap::Square);
    let pos: [f32; 6] = [0.0, 2.0 / 6.0, 3.0 / 6.0, 4.0 / 6.0, 5.0 / 6.0, 1.0];
    let indigo = Color4f::from_color(Color::new(0xFF4B0082));
    let violet = Color4f::from_color(Color::new(0xFFEE82EE));
    let colors: [Color4f; 6] = [
        Color4f::new(1.0, 0.0, 0.0, 1.0),
        Color4f::new(1.0, 1.0, 0.0, 1.0),
        Color4f::new(0.0, 1.0, 0.0, 1.0),
        Color4f::new(0.0, 0.0, 1.0, 1.0),
        indigo,
        violet,
    ];
    grad.set_shader(shaders::radial_gradient(
        (Point::new(250.0, 250.0), 280.0),
        &Gradient::new(
            Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    ));
    canvas.draw_paint(&grad);

    let mut white = Paint::default();
    white.set_anti_alias(true);
    white.set_stroke_cap(Cap::Square);
    white.set_color(Color::WHITE);
    let mut draw_line = |x0: f32, y0: f32, x1: f32, y1: f32, w: f32| {
        let p = [Point::new(x0, y0), Point::new(x1, y1)];
        white.set_stroke_width(w);
        canvas.draw_points(PointMode::Lines, &p, &white);
        grad.set_stroke_width(w - 4.0);
        canvas.draw_points(PointMode::Lines, &p, &grad);
    };
    draw_line(20.0, 20.0, 200.0, 120.0, 20.0);
    draw_line(20.0, 200.0, 20.0, 100.0, 20.0);
    draw_line(480.0, 20.0, 400.0, 400.0, 20.0);
    draw_line(50.0, 480.0, 260.0, 100.0, 20.0);
    draw_line(270.0, 20.0, 380.0, 210.0, 20.0);
    draw_line(280.0, 280.0, 400.0, 480.0, 20.0);
    draw_line(160.0, 375.0, 280.0, 375.0, 20.0);
    draw_line(220.0, 410.0, 220.0, 470.0, 20.0);
    draw_line(250.0, 250.0, 250.0, 250.0, 20.0);
});
