// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bug615686.cpp (chrome/m156)

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

use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_builder::PathBuilder;

// Port of: gm/bug615686.cpp#L13-L21 (chrome/m156)
crate::def_simple_gm!(bug615686, canvas, 250, 250, {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_style(Style::Stroke);
    p.set_stroke_width(20.0);
    let path = PathBuilder::new()
        .move_to((0.0, 0.0))
        .cubic_to((200.0, 200.0), (0.0, 200.0), (200.0, 0.0))
        .detach();
    canvas.draw_path(&path, &p);
});
