// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_1174354.cpp (chrome/m156)

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
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color::Color4f;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_effects::image_filters::blur_filter;

// Port of: gm/crbug_1174354.cpp#L9-L30 (chrome/m156)
fn draw_bg_blur(canvas: &Canvas, rect: IRect, sigma: f32) {
    let mut outset_rect = Rect::from_irect(rect);
    outset_rect.outset((10.0, 10.0));
    canvas.save_layer(&SaveLayerRec::default().bounds(&outset_rect));
    let colors = [
        Color4f::new(1.0, 0.0, 0.0, 1.0),
        Color4f::new(0.0, 0.0, 1.0, 1.0),
        Color4f::new(0.0, 1.0, 0.0, 1.0),
    ];
    // `(rect.left() + rect.right() )/2.f`: the sum is an int, as in the C++.
    let cx = (rect.left() + rect.right()) as f32 / 2.0;
    let cy = (rect.top() + rect.bottom()) as f32 / 2.0;
    let g = shaders::sweep_gradient(
        Point::new(cx, cy),
        (0.0, 45.0),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Mirror, None),
            Interpolation::default(),
        ),
        None,
    );
    let mut paint = Paint::default();
    paint.set_shader(g);
    canvas.draw_rect(Rect::from_irect(rect), &paint);
    let blur = blur_filter::blur(
        sigma,
        sigma,
        TileMode::Clamp,
        None,
        Some(Rect::from_irect(rect)),
    );
    let rec = SaveLayerRec::default()
        .bounds(&outset_rect)
        .backdrop(blur.as_ref().expect("a blur filter"));
    canvas.save_layer(&rec);
    canvas.restore();
    canvas.restore();
}

// Port of: gm/crbug_1174354.cpp#L32-L38 (chrome/m156)
crate::def_simple_gm!(
    #[ignore = "see notes/gm_crbug_1174354_cpp_crbug_1174354.md"]
    crbug_1174354,
    canvas,
    70,
    250,
    {
        draw_bg_blur(canvas, IRect::from_xywh(10, 10, 50, 50), 5.0);
        draw_bg_blur(canvas, IRect::from_xywh(10, 70, 50, 50), 15.0);
        draw_bg_blur(canvas, IRect::from_xywh(10, 130, 50, 50), 30.0);
        draw_bg_blur(canvas, IRect::from_xywh(10, 190, 50, 50), 70.0);
    }
);
