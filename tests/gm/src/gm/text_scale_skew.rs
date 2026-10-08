// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/text_scale_skew.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::utils::text_utils::{Align, draw_string};
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/text_scale_skew.cpp#L15-L36 (chrome/m156), draw_the_text
fn draw_the_text(canvas: &Canvas) {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    let mut font = default_portable_font();
    font.set_size(18.0);
    let mut y: f32 = 10.0;
    for scale in [0.5_f32, 0.71, 1.0, 1.41, 2.0] {
        font.set_scale_x(scale);
        y += font.metrics().0;
        let mut x: f32 = 50.0;
        for skew in [-0.5_f32, 0.0, 0.5] {
            font.set_skew_x(skew);
            draw_string(canvas, "Skia", x, y, &font, &p, Align::Center);
            x += 78.0;
        }
    }
}

// skbug.com/40038559
// Port of: gm/text_scale_skew.cpp#L38-L40 (chrome/m156)
crate::def_simple_gm!(text_scale_skew, canvas, 256, 128, {
    draw_the_text(canvas);
});

// make sure we apply matrices in the correct order inside scalercontext
// Port of: gm/text_scale_skew.cpp#L43-L47 (chrome/m156)
crate::def_simple_gm!(text_scale_skew_rotate, canvas, 256, 128, {
    canvas.rotate(30.0, Some(Point::new(128.0, 64.0)));

    draw_the_text(canvas);
});
