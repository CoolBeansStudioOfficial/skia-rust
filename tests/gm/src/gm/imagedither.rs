// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagedither.cpp (chrome/m156)

//! `image_dither`: image draws and gradients with the dither flag, and a runtime blender that
//! stretches the colors of the destination.

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blender::Blender;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::RuntimeEffect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_raster::surfaces;

// Port of: gm/imagedither.cpp#L22-L26 (chrome/m156)
fn stretch_colors_blender() -> Option<Blender> {
    RuntimeEffect::make_for_blender(
        "half4 main(half4 src, half4 dst) { return ((dst.rgb - 0.25) * 16).rgb1; }",
        None,
    )
    .expect("the blender compiles")
    .make_blender(Data::new_empty(), &[])
}

// Port of: gm/imagedither.cpp#L28-L80 (chrome/m156)
crate::def_simple_gm_can_fail!(image_dither, canvas, error_msg, 425, 110, {
    // (Skia skips recording mode here. The harness always draws to a surface, so the check is
    // not needed.)
    let _ = &error_msg;

    // First, we make a non-dithered image with a shallow radial gradient. This will be our source:
    let colors = [
        Color4f::from_color(Color::new(0xFF55_5555)),
        Color4f::from_color(Color::new(0xFF44_4444)),
    ];
    let gradient = gradient_shaders::linear_gradient(
        ((0.0, 0.0), (100.0, 100.0)),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    );
    let mut gradient_paint = Paint::default();
    gradient_paint.set_shader(gradient);
    let info = ImageInfo::new((100, 100), ColorType::RGBAF16, AlphaType::Premul, None);
    let mut surface = surfaces::raster(&info, None, None).expect("a surface");
    surface.canvas().draw_paint(&gradient_paint);
    let image = surface.image_snapshot().expect("a snapshot");

    // Now, we draw it three times:
    // 1) As-is (no dither), to ensure that our source image doesn't have any dithering included
    // 2) Using an image shader, with dithering enabled on the paint
    // 3) With drawImage, with dithering enabled on the paint
    //
    // We'd like #2 and #3 to both respect the dither flag, for consistency (b/320529640)
    canvas.translate((5.0, 5.0));
    canvas.draw_image(&image, (0.0, 0.0), None);
    canvas.translate((105.0, 0.0));

    let mut image_shader_paint = Paint::default();
    image_shader_paint.set_shader(image.to_shader(
        (TileMode::Clamp, TileMode::Clamp),
        SamplingOptions::default(),
        None,
    ));
    image_shader_paint.set_dither(true);
    canvas.draw_rect(Rect::new(0.0, 0.0, 100.0, 100.0), &image_shader_paint);
    canvas.translate((105.0, 0.0));

    let mut draw_image_paint = Paint::default();
    draw_image_paint.set_dither(true);
    canvas.draw_image_with_sampling_options(
        &image,
        (0.0, 0.0),
        SamplingOptions::default(),
        Some(&draw_image_paint),
    );
    canvas.translate((105.0, 0.0));

    // Also draw the actual gradient with the dither flag, to see how it should look:
    gradient_paint.set_dither(true);
    canvas.draw_rect(Rect::new(0.0, 0.0, 100.0, 100.0), &gradient_paint);

    let mut color_stretch_paint = Paint::default();
    color_stretch_paint.set_blender(stretch_colors_blender());
    canvas.draw_paint(&color_stretch_paint);

    DrawResult::Ok
});
