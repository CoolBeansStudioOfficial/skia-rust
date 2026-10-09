// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_1156804.cpp (chrome/m156)

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
use skia_rust_core::color::Color;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::{blur_filter, crop_filter};

// Port of: gm/crbug_1156804.cpp#L9-L23 (chrome/m156)
fn draw_one(canvas: &Canvas, rect: Rect, save_border: f32, sigma: f32, c: Color) {
    let mut border_rect = rect;
    border_rect.outset((save_border, save_border));
    let mut p = Paint::default();
    p.set_color(c);
    p.set_image_filter(blur_filter::blur(
        sigma,
        sigma,
        TileMode::Decal,
        crop_filter::crop(&border_rect, TileMode::Clamp, None),
        Some({
            let mut r = border_rect;
            r.outset((3.0 * sigma, 3.0 * sigma));
            r
        }),
    ));
    p.set_anti_alias(true);
    canvas.draw_rect(rect, &p);
}

// Port of: gm/crbug_1156804.cpp#L25-L40 (chrome/m156)
crate::def_simple_gm!(crbug_1156804, canvas, 250, 250, {
    draw_one(
        canvas,
        Rect::from_xywh(64.0, 64.0, 25.0, 25.0),
        1.0,
        3.0,
        Color::GREEN,
    );
    draw_one(
        canvas,
        Rect::from_xywh(164.0, 64.0, 25.0, 25.0),
        30.0,
        3.0,
        Color::GREEN,
    );
    // This one would draw incorrectly because the large sigma causes downscaling of the source
    // and the one-pixel border would make the downscaled image not contain trans-black at the
    // edges. Combined with the clamp mode on the blur filter it would "harden" the edge.
    draw_one(
        canvas,
        Rect::from_xywh(64.0, 164.0, 25.0, 25.0),
        1.0,
        20.0,
        Color::RED,
    );
    draw_one(
        canvas,
        Rect::from_xywh(164.0, 164.0, 25.0, 25.0),
        30.0,
        20.0,
        Color::GREEN,
    );
});
