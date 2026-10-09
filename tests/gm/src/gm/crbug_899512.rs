// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_899512.cpp (chrome/m156)

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

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::color_filters;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;

// Port of: gm/crbug_899512.cpp#L10-L19 (chrome/m156)
crate::def_simple_gm!(crbug_899512, canvas, 520, 520, {
    let matrix = Matrix::new_all(-1.0, 0.0, 220.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
    canvas.concat(&matrix);
    let mut paint = Paint::default();
    paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 6.2735, false));
    paint.set_color_filter(color_filters::blend(
        Color4f::from_color(Color::BLACK),
        None,
        BlendMode::SrcIn,
    ));
    canvas.draw_rect(Rect::from_xywh(0.0, 10.0, 200.0, 200.0), &paint);
});
