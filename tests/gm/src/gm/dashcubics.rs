// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/dashcubics.cpp (chrome/m156)

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
use skia_rust_core::paint::{Join, Paint};
use skia_rust_core::path::Path;
use skia_rust_core::utils::parse_path;
use skia_rust_effects::dash_path_effect;

// Inspired by http://code.google.com/p/chromium/issues/detail?id=112145
// Port of: gm/dashcubics.cpp#L28-L45 (chrome/m156)
fn flower(canvas: &Canvas, path: &Path, intervals: &[f32; 2], join: Join) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_stroke(true);
    paint.set_stroke_join(join);
    paint.set_stroke_width(42.0);
    canvas.draw_path(path, &paint);

    paint.set_color(Color::RED);
    paint.set_stroke_width(21.0);
    paint.set_path_effect(dash_path_effect::new(intervals, 0.0));
    canvas.draw_path(path, &paint);

    paint.set_color(Color::GREEN);
    paint.set_path_effect(None);
    paint.set_stroke_width(0.0);
    canvas.draw_path(path, &paint);
}

// Port of: gm/dashcubics.cpp#L47-L74 (chrome/m156)
crate::def_simple_gm!(dashcubics, canvas, 865, 750, {
    let d = "M 337,98 C 250,141 250,212 250,212 C 250,212 250,212 250,212\
             C 250,212 250,212 250,212 C 250,212 250,141 163,98 C 156,195 217,231 217,231\
             C 217,231 217,231 217,231 C 217,231 217,231 217,231 C 217,231 156,195 75,250\
             C 156,305 217,269 217,269 C 217,269 217,269 217,269 C 217,269 217,269 217,269\
             C 217,269 156,305 163,402 C 250,359 250,288 250,288 C 250,288 250,288 250,288\
             C 250,288 250,288 250,288 C 250,288 250,359 338,402 C 345,305 283,269 283,269\
             C 283,269 283,269 283,269 C 283,269 283,269 283,269 C 283,269 345,305 425,250\
             C 344,195 283,231 283,231 C 283,231 283,231 283,231 C 283,231 283,231 283,231\
             C 283,231 344,195 338,98";

    let path = parse_path::from_svg(d).unwrap_or_default();
    canvas.translate((-35.0, -55.0));
    for x in 0..2 {
        for y in 0..2 {
            canvas.save();
            #[allow(clippy::cast_precision_loss)] // x * 430.f
            canvas.translate((x as f32 * 430.0, y as f32 * 355.0));
            let intervals = [5.0 + (if x != 0 { 0.0 } else { 0.0001 + 0.0001 }), 10.0];
            flower(
                canvas,
                &path,
                &intervals,
                if y != 0 { Join::Miter } else { Join::Round },
            );
            canvas.restore();
        }
    }
});
