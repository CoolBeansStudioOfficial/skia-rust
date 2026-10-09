// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/localmatriximageshader.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::{get_resource_as_image, make_surface};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::{CubicResampler, SamplingOptions};
use skia_rust_core::shaders::ImageShader;
use skia_rust_core::tile_mode::TileMode;

// Port of: gm/localmatriximageshader.cpp#L13-L23 (chrome/m156), make_image
fn make_image(root_canvas: &Canvas, color: Color) -> Option<Image> {
    let info = ImageInfo::new((100, 100), ColorType::N32, AlphaType::Premul, None);
    let mut surface = make_surface(root_canvas, &info, None)?;
    {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(color);
        surface
            .canvas()
            .draw_irect(IRect::from_xywh(25, 25, 50, 50), &paint);
    }
    surface.image_snapshot()
}

// Port of: gm/localmatriximageshader.cpp#L25-L56 (chrome/m156), DEF_SIMPLE_GM(localmatriximageshader)
crate::def_simple_gm!(localmatriximageshader, canvas, 250, 250, {
    let red_image = make_image(canvas, Color::RED).expect("a red image");
    let translate = Matrix::translate((100.0, 0.0));
    let mut rotate = Matrix::new_identity();
    rotate.set_rotate(45.0, None);
    let red_image_shader = ImageShader::make(
        Some(red_image),
        TileMode::Clamp,
        TileMode::Clamp,
        &SamplingOptions::default(),
        Some(&rotate),
        false,
    )
    .expect("a red image shader");
    let red_local_matrix_shader = red_image_shader.with_local_matrix(&translate);

    // Rotate about the origin will happen first.
    let mut paint = Paint::default();
    paint.set_shader(red_local_matrix_shader.clone());
    canvas.draw_irect(IRect::from_wh(250, 250), &paint);

    let blue_image = make_image(canvas, Color::BLUE).expect("a blue image");
    let blue_image_shader = ImageShader::make(
        Some(blue_image),
        TileMode::Clamp,
        TileMode::Clamp,
        &SamplingOptions::default(),
        Some(&translate),
        false,
    )
    .expect("a blue image shader");
    let blue_local_matrix_shader = blue_image_shader.with_local_matrix(&rotate);

    // Translate will happen first.
    paint.set_shader(blue_local_matrix_shader.clone());
    canvas.draw_irect(IRect::from_wh(250, 250), &paint);

    canvas.translate((100.0, 0.0));

    // Use isAImage() and confirm that the shaders will draw exactly the same (to the right by 100).
    let (image, matrix, (mode_x, mode_y)) = red_local_matrix_shader
        .is_a_image()
        .expect("a red image shader");
    paint.set_shader(ImageShader::make(
        Some(image),
        mode_x,
        mode_y,
        &SamplingOptions::default(),
        Some(&matrix),
        false,
    ));
    canvas.draw_irect(IRect::from_wh(250, 250), &paint);

    let (image, matrix, (mode_x, mode_y)) = blue_local_matrix_shader
        .is_a_image()
        .expect("a blue image shader");
    paint.set_shader(ImageShader::make(
        Some(image),
        mode_x,
        mode_y,
        &SamplingOptions::default(),
        Some(&matrix),
        false,
    ));
    canvas.draw_irect(IRect::from_wh(250, 250), &paint);
});

// Port of: gm/localmatriximageshader.cpp#L69-L78 (chrome/m156), DEF_SIMPLE_GM(localmatriximageshader_filtering)
crate::def_simple_gm!(localmatriximageshader_filtering, canvas, 256, 256, {
    // Test that filtering decisions (eg bicubic for upscale) are made correctly when the scale
    // comes from a local matrix shader.
    let image = get_resource_as_image("images/mandrill_256.png").expect("mandrill_256.png");
    let mut m = Matrix::new_identity();
    m.set_scale((2.0, 2.0), None);
    let sampling = SamplingOptions::from(CubicResampler::mitchell());
    let shader = ImageShader::make(
        Some(image),
        TileMode::Clamp,
        TileMode::Clamp,
        &sampling,
        None,
        false,
    )
    .expect("an image shader")
    .with_local_matrix(&m);
    let mut p = Paint::default();
    p.set_shader(shader);
    canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 256.0, 256.0), &p);
});
