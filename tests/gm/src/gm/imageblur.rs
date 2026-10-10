// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imageblur.cpp (chrome/m156)

// GM ports mirror the C++ integer and scalar casts and loop indices.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::similar_names
)]

use crate::prelude::*;
use crate::tool_utils::{color_to_565, int_to_scalar};
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::font::Font;
use skia_rust_core::paint::Paint;
use skia_rust_core::random::Random;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::blur;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

const WIDTH: i32 = 500;
const HEIGHT: i32 = 500;

// Port of: gm/imageblur.cpp#L13-L35 (chrome/m156), imageblurgm_draw
fn imageblurgm_draw(sigma_x: f32, sigma_y: f32, canvas: &Canvas) {
    let mut paint = Paint::default();
    paint.set_image_filter(blur(sigma_x, sigma_y, TileMode::Decal, None, None));
    canvas.save_layer(&SaveLayerRec::default().paint(&paint));
    let text = "The quick brown fox jumped over the lazy dog.";
    let mut rand = Random::default();
    let mut text_paint = Paint::default();
    let mut font = Font::from_size(default_portable_typeface(), 12.0);
    for _ in 0..25 {
        let x = rand.next_u_less_than(WIDTH as u32) as i32;
        let y = rand.next_u_less_than(HEIGHT as u32) as i32;
        text_paint.set_color(color_to_565(rand.next_bits(24) | 0xFF00_0000));
        font.set_size(rand.next_range_scalar(0.0, 300.0));
        canvas.draw_str(
            text,
            (int_to_scalar(x), int_to_scalar(y)),
            &font,
            &text_paint,
        );
    }
    canvas.restore();
}

// Port of: gm/imageblur.cpp#L37-L39 (chrome/m156), DEF_SIMPLE_GM_BG(imageblur, ...)
crate::def_simple_gm_bg!(imageblur, canvas, WIDTH, HEIGHT, Color::BLACK, {
    imageblurgm_draw(24.0, 0.0, canvas);
});

// Port of: gm/imageblur.cpp#L41-L43 (chrome/m156), DEF_SIMPLE_GM_BG(imageblur_large, ...)
crate::def_simple_gm_bg!(imageblur_large, canvas, WIDTH, HEIGHT, Color::BLACK, {
    imageblurgm_draw(80.0, 80.0, canvas);
});
