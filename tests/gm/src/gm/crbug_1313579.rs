// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_1313579.cpp (chrome/m156)

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
use skia_rust_core::color::Color;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::blur_filter;

// Port of: gm/crbug_1313579.cpp#L9-L26 (chrome/m156)
crate::def_simple_gm!(crbug_1313579, canvas, 110, 110, {
    const K_BG_RECT: IRect = IRect::from_ltrb(0, 0, 100, 100);
    let backdrop_filter = blur_filter::blur(
        50.0,
        50.0,
        TileMode::Clamp,
        None,
        Some(Rect::from_irect(K_BG_RECT)),
    );
    canvas.clear(Color::GREEN);
    let m = Matrix::new_all(
        0.999999, 0.0, 4.99999, 0.0, 0.999999, 4.99999, 0.0, 0.0, 1.0,
    );
    canvas.concat(&m);
    canvas.clip_irect(K_BG_RECT, None);
    canvas.clear(Color::WHITE);
    canvas.save_layer(
        &SaveLayerRec::default().backdrop(backdrop_filter.as_ref().expect("a blur filter")),
    );
    canvas.restore();
});
