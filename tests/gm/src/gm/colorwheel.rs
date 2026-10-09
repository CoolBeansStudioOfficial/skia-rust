// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/colorwheel.cpp (chrome/m156)
#![allow(clippy::cast_precision_loss)] // mirrors the C++ int-to-scalar conversions of small sizes (exact in f32)

use crate::tool_utils::{draw_checkerboard, get_resource_as_data, get_resource_as_image};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::canvas::{Canvas, SrcRectConstraint};
use skia_rust_core::color::Color;
use skia_rust_core::data::Data;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::{FontStyle, Slant, Weight, Width};
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_tools::font_tool_utils::create_portable_typeface;

// Port of: gm/colorwheel.cpp#L22-L30 (chrome/m156)
fn draw_image(canvas: &Canvas, resource: &str, x: i32, y: i32) {
    if let Some(image) = get_resource_as_image(resource) {
        canvas.draw_image(&image, (x as f32, y as f32), None);
    } else {
        eprintln!("\nCould not decode file '{resource}'. Did you forget to set the resourcePath?");
    }
}

// This GM tests whether the image decoders properly decode each color channel.
// Port of: gm/colorwheel.cpp#L32-L50 (chrome/m156)
crate::def_simple_gm!(colorwheel, canvas, 384, 256, {
    // ToolUtils::draw_checkerboard(canvas) with its default colours and size.
    draw_checkerboard(
        canvas,
        Color::from_argb(0xFF, 0x99, 0x99, 0x99),
        Color::from_argb(0xFF, 0x66, 0x66, 0x66),
        8,
    );
    draw_image(canvas, "images/color_wheel.png", 0, 0); // top left
    draw_image(canvas, "images/color_wheel.gif", 128, 0); // top middle
    draw_image(canvas, "images/color_wheel.webp", 0, 128); // bottom left
    draw_image(canvas, "images/color_wheel.jpg", 128, 128); // bottom middle
    // SK_CODEC_DECODES_AVIF is not defined for the oracle build: no AVIF draw here.
});

// Port of: gm/colorwheel.cpp#L52-L70 (chrome/m156)
crate::def_simple_gm!(colorwheelnative, canvas, 128, 28, {
    let typeface = create_portable_typeface(
        Some("sans-serif"),
        FontStyle::new(Weight::BOLD, Width::NORMAL, Slant::Upright),
    );
    let mut font = Font::new(typeface, 18.0, 1.0, 0.0);
    font.set_edging(Edging::Alias);

    canvas.clear(Color::LIGHT_GRAY);
    let draw = |text: &str, x: f32, color: Color| {
        let mut paint = Paint::default();
        paint.set_color(color);
        canvas.draw_str(text, (x, 20.0), &font, &paint);
    };
    draw("R", 8.0, Color::RED);
    draw("G", 24.0, Color::GREEN);
    draw("B", 40.0, Color::BLUE);
    draw("C", 56.0, Color::CYAN);
    draw("M", 72.0, Color::MAGENTA);
    draw("Y", 88.0, Color::YELLOW);
    draw("K", 104.0, Color::BLACK);
});

// This GM tests decoding images with non-default (overridden) alpha types.
// Port of: gm/colorwheel.cpp#L72-L100 (chrome/m156)
crate::def_simple_gm!(colorwheel_alphatypes, canvas, 256, 128, {
    canvas.clear(Color::WHITE);

    let img_data = get_resource_as_data("images/color_wheel.png").expect("color_wheel.png");
    let decode = |alpha_type| {
        skia_rust_codec::images::deferred_from_encoded_data(
            Some(Data::new_from_vec(img_data.clone())),
            Some(alpha_type),
        )
    };
    // ToolUtils::MakeTextureImage returns the image unchanged without a GPU context.
    let pm_img = decode(AlphaType::Premul).expect("premul image");
    let upm_img = decode(AlphaType::Unpremul).expect("unpremul image");

    let linear = SamplingOptions::from(FilterMode::Linear);

    // We draw a tiny (8x8) section of the image that falls right on the edge of transparency,
    // and blow it up so we can really see the impact of filtering in premul or unpremul.
    let src_rect = Rect::from_xywh(12.0, 102.0, 8.0, 8.0);
    let dst_rect = Rect::from_ltrb(0.0, 0.0, 128.0, 128.0);

    // First, we draw the normal (premul-then-filter) image, which looks good.
    canvas.draw_image_rect_with_sampling_options(
        &pm_img,
        Some((&src_rect, SrcRectConstraint::Fast)),
        dst_rect,
        linear,
        &Paint::default(),
    );
    // Next, we draw the unpremul (filter-then-premul) image, which looks bad.
    canvas.draw_image_rect_with_sampling_options(
        &upm_img,
        Some((&src_rect, SrcRectConstraint::Fast)),
        dst_rect.with_offset((128.0, 0.0)),
        linear,
        &Paint::default(),
    );
});
