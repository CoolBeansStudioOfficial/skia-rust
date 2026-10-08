// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/b_119394958.cpp (chrome/m156)

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
use skia_rust_core::rect::Rect;

// The root cause of this bug was that a stroked arc with round caps was batched with a filled
// circle. The circle op code would choose a GeometryProcessor configuration that expected round
// cap centers as vertex attributes. However, the tessellation code for the filled circle would
// not put in zero-width round cap centers and then didn't advance the pointer into which
// vertex data was being written by the expected vertex stride.
// Port of: gm/b_119394958.cpp#L14-L31 (chrome/m156)
crate::def_simple_gm!(b_119394958, canvas, 100, 100, {
    let mut paint = Paint::default();
    paint.set_color(Color::BLUE);
    paint.set_anti_alias(true);
    canvas.draw_circle((50.0, 50.0), 45.0, &paint);
    paint.set_color(Color::GREEN);
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(5.0);
    canvas.draw_circle((50.0, 50.0), 35.0, &paint);
    paint.set_color(Color::RED);
    paint.set_stroke_cap(Cap::Round);
    canvas.draw_arc(
        Rect::from_ltrb(30.0, 30.0, 70.0, 70.0),
        0.0,
        110.0,
        false,
        &paint,
    );
});
