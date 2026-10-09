// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imageresizetiled.cpp (chrome/m156)

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

use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::font::Font;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::utils::text_utils::{Align, draw_string};
use skia_rust_effects::image_filters::matrix_transform_filter;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/imageresizetiled.cpp#L9-L11 (chrome/m156)
const WIDTH: i32 = 640;
// Port of: gm/imageresizetiled.cpp#L9-L11 (chrome/m156)
const HEIGHT: i32 = 480;
// Port of: gm/imageresizetiled.cpp#L11 (chrome/m156)
const RESIZE_FACTOR: f32 = 2.0;

// Port of: gm/imageresizetiled.cpp#L12-L44 (chrome/m156)
crate::def_simple_gm!(imageresizetiled, canvas, WIDTH, HEIGHT, {
    let mut paint = Paint::default();
    let mut matrix = Matrix::new_identity();
    matrix.set_scale((RESIZE_FACTOR, RESIZE_FACTOR), None);
    paint.set_image_filter(matrix_transform_filter::matrix_transform(
        &matrix,
        SamplingOptions::default(),
        None,
    ));

    let font = Font::from_size(default_portable_typeface(), 100.0);
    let tile_size: f32 = 100.0;
    let mut y: f32 = 0.0;
    while y < HEIGHT as f32 {
        let mut x: f32 = 0.0;
        while x < WIDTH as f32 {
            canvas.save();
            canvas.clip_rect(Rect::from_xywh(x, y, tile_size, tile_size), None, None);
            canvas.scale((1.0 / RESIZE_FACTOR, 1.0 / RESIZE_FACTOR));
            canvas.save_layer(&SaveLayerRec::default().paint(&paint));
            let strs = ["The quick", "brown fox", "jumped over", "the lazy dog."];
            let mut pos_y: f32 = 0.0;
            for s in strs {
                pos_y += 100.0;
                draw_string(canvas, s, 0.0, pos_y, &font, &Paint::default(), Align::Left);
            }
            canvas.restore();
            canvas.restore();
            x += tile_size;
        }
        y += tile_size;
    }
});
