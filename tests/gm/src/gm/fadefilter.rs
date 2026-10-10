// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/fadefilter.cpp (chrome/m156)

use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_effects::image_filters::color_filter;

// Port of: gm/fadefilter.cpp#L9-L19 (chrome/m156), fadefilter
crate::def_simple_gm!(fadefilter, canvas, 256, 256, {
    #[rustfmt::skip]
    let matrix = ColorMatrix::new(
        1.0, 0.0, 0.0, 0.0, 0.5,
        0.0, 1.0, 0.0, 0.0, 0.5,
        0.0, 0.0, 1.0, 0.0, 0.5,
        0.0, 0.0, 0.0, 1.0, 0.0,
    );
    let color_filter_value = color_filters::matrix(&matrix, Clamp::Yes);
    let mut layer_paint = Paint::default();
    layer_paint.set_image_filter(color_filter(color_filter_value, None, None));
    canvas.draw_rect(Rect::new(64.0, 64.0, 192.0, 192.0), &layer_paint);
});
