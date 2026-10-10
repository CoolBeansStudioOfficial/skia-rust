// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/blurimagevmask.cpp (chrome/m156)

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
use crate::tool_utils::get_resource_as_image;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::font::Font;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::blur;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/blurimagevmask.cpp#L11-L40 (chrome/m156), blurimagevmask
crate::def_simple_gm!(blurimagevmask, canvas, 700, 1200, {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::BLACK);
    let font = Font::from_size(default_portable_typeface(), 25.0);
    let sigmas: [f64; 5] = [3.0, 8.0, 16.0, 24.0, 32.0];
    canvas.draw_str("mask blur", (285.0, 50.0), &font, &paint);
    canvas.draw_str("image blur", (285.0 + 250.0, 50.0), &font, &paint);
    let mut r = Rect::new(35.0, 100.0, 135.0, 200.0);
    for sigma in sigmas {
        canvas.draw_rect(r, &paint);
        // snprintf(out, "Sigma: %g", sigma)
        let out = format!("Sigma: {sigma}");
        canvas.draw_str(&out, (r.left(), r.bottom() + 35.0), &font, &paint);
        r.offset((250.0, 0.0));
        let sigma_f = sigma as f32;
        paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, sigma_f, None));
        canvas.draw_rect(r, &paint);
        paint.set_mask_filter(None);
        let mut image_blur_paint = Paint::default();
        r.offset((250.0, 0.0));
        image_blur_paint.set_image_filter(blur(sigma_f, sigma_f, TileMode::Decal, None, None));
        canvas.save_layer(&SaveLayerRec::default().paint(&image_blur_paint));
        canvas.draw_rect(r, &paint);
        canvas.restore();
        r.offset((-500.0, 200.0));
    }
});

// Port of: gm/blurimagevmask.cpp#L42-L62 (chrome/m156), blur_image
crate::def_simple_gm_can_fail!(blur_image, canvas, error_msg, 500, 500, {
    let Some(image) = get_resource_as_image("images/mandrill_128.png") else {
        error_msg.clear();
        error_msg
            .push_str("Could not load mandrill_128.png. Did you forget to set the resourcePath?");
        return DrawResult::Fail;
    };
    let mut paint = Paint::default();
    paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, 4.0, None));
    // both of these should draw with the blur, but (formerly) we had a bug where the unscaled
    // version (taking the spriteblitter code path) ignore the maskfilter.
    canvas.draw_image_with_sampling_options(
        &image,
        (10.0, 10.0),
        SamplingOptions::default(),
        Some(&paint),
    );
    canvas.scale((1.01, 1.01));
    canvas.draw_image_with_sampling_options(
        &image,
        (10.0 + image.width() as f32 + 10.0, 10.0),
        SamplingOptions::default(),
        Some(&paint),
    );
    DrawResult::Ok
});
