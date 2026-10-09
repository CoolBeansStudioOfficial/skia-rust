// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imageblur2.cpp (chrome/m156)

// GM ports mirror the C++ integer and scalar casts and loop indices.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::similar_names
)]

use crate::tool_utils::{color_to_565, int_to_scalar};
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::paint::Paint;
use skia_rust_core::random::Random;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::blur;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

const K_WIDTH: i32 = 500;
const K_HEIGHT: i32 = 500;

const K_BLUR_SIGMAS: [f32; 6] = [0.0, 0.3, 0.5, 2.0, 32.0, 80.0];
const K_TEST_STRINGS: [&str; 6] = [
    "The quick`~",
    "brown fox[]",
    "jumped over",
    "the lazy@#$",
    "dog.{}!%^&",
    "*()+=-\\'\"/",
];

// Port of: gm/imageblur2.cpp#L7-L40 (chrome/m156), imageblur2
crate::def_simple_gm!(imageblur2, canvas, K_WIDTH, K_HEIGHT, {
    let sigma_count = K_BLUR_SIGMAS.len() as i32;
    let test_string_count = K_TEST_STRINGS.len();
    // int division, as in the C++ `kWidth / sigmaCount`
    let dx = int_to_scalar(K_WIDTH / sigma_count);
    let dy = int_to_scalar(K_HEIGHT / sigma_count);
    let text_size: f32 = 12.0;
    let mut font = Font::from_size(default_portable_typeface(), text_size);
    font.set_edging(Edging::Alias);
    for x in 0..sigma_count {
        let sigma_x = K_BLUR_SIGMAS[x as usize];
        for y in 0..sigma_count {
            let sigma_y = K_BLUR_SIGMAS[y as usize];
            let mut paint = Paint::default();
            paint.set_image_filter(blur(sigma_x, sigma_y, TileMode::Decal, None, None));
            canvas.save_layer(&SaveLayerRec::default().paint(&paint));
            let mut rand = Random::default();
            let mut text_paint = Paint::default();
            text_paint.set_color(color_to_565(rand.next_bits(24) | 0xFF00_0000));
            for i in 0..test_string_count {
                canvas.draw_str(
                    K_TEST_STRINGS[i],
                    (
                        int_to_scalar(x) * dx,
                        int_to_scalar(y) * dy + text_size * int_to_scalar(i as i32) + text_size,
                    ),
                    &font,
                    &text_paint,
                );
            }
            canvas.restore();
        }
    }
});
