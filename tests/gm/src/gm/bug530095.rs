// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bug530095.cpp (chrome/m156)

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
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_effects::dash_path_effect;

// Port of: gm/bug530095.cpp#L18-L51 (chrome/m156)
crate::def_simple_gm!(bug530095, canvas, 900, 1200, {
    let path1 = Path::circle((200.0, 200.0), 124.0, None);
    let path2 = Path::circle((2.0, 2.0), 1.24, None);

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(26.0);
    let intervals = [700.0, 700.0];
    paint.set_path_effect(dash_path_effect::new(&intervals, -40.0));
    canvas.draw_path(&path1, &paint);

    paint.set_stroke_width(0.26);
    let sm_intervals = [7.0, 7.0];
    paint.set_path_effect(dash_path_effect::new(&sm_intervals, -0.40));
    canvas.save();
    canvas.scale((100.0, 100.0));
    canvas.translate((4.0, 0.0));
    canvas.draw_path(&path2, &paint);
    canvas.restore();

    paint.set_stroke_width(26.0);
    paint.set_path_effect(dash_path_effect::new(&intervals, 0.0));
    canvas.save();
    canvas.translate((0.0, 400.0));
    canvas.draw_path(&path1, &paint);
    canvas.restore();

    paint.set_stroke_width(0.26);
    paint.set_path_effect(dash_path_effect::new(&sm_intervals, 0.0));
    canvas.scale((100.0, 100.0));
    canvas.translate((4.0, 4.0));
    canvas.draw_path(&path2, &paint);
});

// Port of: gm/bug530095.cpp#L53-L63 (chrome/m156)
crate::def_simple_gm!(bug591993, canvas, 40, 140, {
    let mut p = Paint::default();
    p.set_color(Color::RED);
    p.set_anti_alias(true);
    p.set_style(Style::Stroke);
    p.set_stroke_cap(Cap::Round);
    p.set_stroke_width(10.0);
    let intervals = [100.0, 100.0];
    p.set_path_effect(dash_path_effect::new(&intervals, 100.0));
    canvas.draw_line((20.0, 20.0), (120.0, 20.0), &p);
});
