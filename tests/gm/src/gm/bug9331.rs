// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bug9331.cpp (chrome/m156)

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
use skia_rust_core::rect::Rect;
use skia_rust_effects::dash_path_effect;

// Reproduces skbug.com/40040651, drawing differently in debug and release builds.
// Port of: gm/bug9331.cpp#L14-L40 (chrome/m156)
crate::def_simple_gm!(bug9331, canvas, 256, 256, {
    let clip = Rect::from_ltrb(0.0, 0.0, 200.0, 150.0);
    {
        let mut paint = Paint::default();
        paint.set_color(Color::new(0x44FF_0000));
        canvas.draw_rect(clip, &paint);
    }

    let draw = |color: Color, clip: Rect| {
        let intervals = [13.0_f32, 17.0];
        let phase = 9.0;

        let mut paint = Paint::default();
        paint.set_color(color);
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(10.0);
        paint.set_path_effect(dash_path_effect::new(&intervals, phase));

        canvas.save();
        canvas.clip_rect(clip, None, None);
        canvas.draw_rect(Rect::from_ltrb(50.0, 50.0, 150.0, 150.0), &paint);
        canvas.restore();
    };

    draw(Color::new(0xFF00_0000), clip);
    draw(Color::new(0xFF00_00FF), clip.with_offset((0.0, 150.0)));
});
