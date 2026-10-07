// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/strokefill.cpp (chrome/m156)

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
    clippy::unreadable_literal
)]

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;

// Port of: gm/strokefill.cpp#L20-L30 (chrome/m156)
fn bug339297_path() -> Path {
    PathBuilder::new()
        .move_to((-469515.0, -10354890.0))
        .cubic_to(
            (771919.62, -10411179.0),
            (2013360.1, -10243774.0),
            (3195542.8, -9860664.0),
        )
        .line_to((3195550.0, -9860655.0))
        .line_to((3195539.0, -9860652.0))
        .line_to((3195539.0, -9860652.0))
        .line_to((3195539.0, -9860652.0))
        .cubic_to(
            (2013358.1, -10243761.0),
            (771919.25, -10411166.0),
            (-469513.84, -10354877.0),
        )
        .line_to((-469515.0, -10354890.0))
        .close()
        .detach()
}

// Port of: gm/strokefill.cpp#L19-L47 (chrome/m156)
crate::def_simple_gm!(bug339297, canvas, 640, 480, {
    let path = bug339297_path();

    canvas.translate((258.0, 10365663.0));

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::BLACK);
    paint.set_style(Style::Fill);
    canvas.draw_path(&path, &paint);

    paint.set_color(Color::RED);
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(1.0);
    canvas.draw_path(&path, &paint);
});

// Port of: gm/strokefill.cpp#L49-L76 (chrome/m156)
crate::def_simple_gm!(bug339297_as_clip, canvas, 640, 480, {
    let path = bug339297_path();

    canvas.translate((258.0, 10365663.0));

    canvas.save();
    canvas.clip_path(&path, None, true);
    canvas.clear(Color::BLACK);
    canvas.restore();

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(Style::Fill);
    paint.set_color(Color::RED);
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(1.0);
    canvas.draw_path(&path, &paint);
});

// Port of: gm/strokefill.cpp#L78-L97 (chrome/m156)
crate::def_simple_gm!(bug6987, canvas, 200, 200, {
    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(0.0001);
    paint.set_anti_alias(true);

    let path = PathBuilder::new()
        .move_to((0.0005, 0.0004))
        .line_to((0.0008, 0.0010))
        .line_to((0.0002, 0.0010))
        .close()
        .detach();

    canvas.save();
    canvas.scale((50000.0, 50000.0));
    canvas.draw_path(&path, &paint);
    canvas.restore();
});
