// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/blurpositioning.cpp (chrome/m156)

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
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar_ceil_to_int;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::blur;

// Port of: gm/blurpositioning.cpp#L9-L30 (chrome/m156), check_small_sigma_offset
crate::def_simple_gm!(check_small_sigma_offset, canvas, 200, 1200, {
    for sigma in [0.0_f64, 0.1, 0.2, 0.3, 0.4, 0.6, 0.8, 1.0, 1.2] {
        let sigma = sigma as f32;
        // border calculation from SkBlurImageFilter
        let border = scalar_ceil_to_int(sigma * 3.0);
        let r = Rect::from_xywh(50.0, 50.0, 100.0, 50.0);
        let mut b = r;
        b.outset((border as f32 + 1.0, border as f32 + 1.0));
        b.inset((0.5, 0.5));
        let mut p = Paint::default();
        p.set_color(Color::RED);
        p.set_style(Style::Stroke);
        canvas.draw_rect(b, &p);
        let mut p = Paint::default();
        p.set_color(Color::BLACK);
        p.set_image_filter(blur(sigma, sigma, TileMode::Decal, None, None));
        canvas.draw_rect(r, &p);
        canvas.translate((0.0, 100.0));
    }
});
