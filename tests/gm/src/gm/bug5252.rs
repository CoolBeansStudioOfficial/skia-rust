// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bug5252.cpp (chrome/m156)

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

use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::rect::Rect;

// Port of: gm/bug5252.cpp#L15-L40 (chrome/m156)
crate::def_simple_gm!(bug5252, canvas, 500, 500, {
    canvas.translate((10.0, 20.0));

    canvas.clip_path(&Path::oval(Rect::from_wh(225.0, 200.0), None), None, None); // bug

    let mut pa = Paint::default();
    pa.set_style(Style::Stroke);
    pa.set_anti_alias(true);
    pa.set_stroke_width(1.0);
    for i in 0..15 {
        for j in 0..10 {
            let _acs = AutoCanvasRestore::guard(canvas, true);

            #[allow(clippy::cast_precision_loss)] // SkIntToScalar
            canvas.translate((i as f32 * 15.0, j as f32 * 20.0));
            canvas.draw_rect(Rect::from_xywh(5.0, 5.0, 10.0, 15.0), &pa);
            let path = PathBuilder::new()
                .move_to((6.0, 6.0))
                .cubic_to((14.0, 10.0), (13.0, 12.0), (10.0, 12.0))
                .cubic_to((7.0, 15.0), (8.0, 17.0), (14.0, 18.0))
                .detach();
            canvas.draw_path(&path, &pa);
        }
    }
});
