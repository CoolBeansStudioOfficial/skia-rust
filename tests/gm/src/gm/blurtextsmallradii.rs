// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/blurtextsmallradii.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_tools::font_tool_utils::default_portable_font;

// GM to check the behavior from chrome bug:745290
// Port of: gm/blurtextsmallradii.cpp#L18-L34 (chrome/m156), blurSmallRadii
crate::def_simple_gm!(blurSmallRadii, canvas, 100, 150, {
    let sigmas = [0.25_f32, 0.5, 0.75, 1.0, 1.5, 2.5];
    let mut paint = Paint::default();
    let font = default_portable_font();

    for sigma in sigmas {
        paint.set_color(Color::RED);
        paint.set_anti_alias(true);
        paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, sigma, None));
        canvas.draw_str("guest", (20.0, 10.0), &font, &paint);

        paint.set_mask_filter(None);
        paint.set_color(Color::GREEN);
        canvas.draw_str("guest", (20.0, 10.0), &font, &paint);
        canvas.translate((0.0, 20.0));
    }
});
