// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/patch.cpp (chrome/m156)
//
// `patch_image` and `patch_image_persp` use the mandrill_128.png image shader; the other GMs use
// the gradient shader `make_shader()`.

// The int/usize-to-scalar casts of the small counts and offsets mirror the C++ arithmetic.
#![allow(clippy::cast_precision_loss)]

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::PointMode;
use skia_rust_core::color::Color4f;
use skia_rust_core::image::Image;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::utils::patch_utils;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

use crate::prelude::*;

// The order of the colors and points is clockwise starting at upper-left corner.
// Port of: gm/patch.cpp#L81-L92 (chrome/m156)
const G_CUBICS: [Point; patch_utils::NUM_CTRL_PTS] = [
    // top points
    Point::new(100.0, 100.0),
    Point::new(150.0, 50.0),
    Point::new(250.0, 150.0),
    Point::new(300.0, 100.0),
    // right points
    Point::new(250.0, 150.0),
    Point::new(350.0, 250.0),
    // bottom points
    Point::new(300.0, 300.0),
    Point::new(250.0, 250.0),
    Point::new(150.0, 350.0),
    Point::new(100.0, 300.0),
    // left points
    Point::new(50.0, 250.0),
    Point::new(150.0, 150.0),
];

// These two should look the same (one patch, one simple path)
// Port of: gm/patch.cpp#L186-L208 (chrome/m156)
crate::def_simple_gm!(patch_alpha_test, canvas, 550, 250, {
    canvas.translate((-75.0, -75.0));

    let colors = [Color::from(0x80FF_0000); patch_utils::NUM_CORNERS];
    let mut paint = Paint::default();
    canvas.draw_patch(&G_CUBICS, &colors, None, BlendMode::Dst, &paint);

    canvas.translate((300.0, 0.0));

    let path = PathBuilder::new()
        .move_to(G_CUBICS[0])
        .cubic_to(G_CUBICS[1], G_CUBICS[2], G_CUBICS[3])
        .cubic_to(G_CUBICS[4], G_CUBICS[5], G_CUBICS[6])
        .cubic_to(G_CUBICS[7], G_CUBICS[8], G_CUBICS[9])
        .cubic_to(G_CUBICS[10], G_CUBICS[11], G_CUBICS[0])
        .detach();
    paint.set_color(colors[0]);
    canvas.draw_path(&path, &paint);
});

// Port of: gm/patch.cpp#L24-L32 (chrome/m156), make_shader
fn make_shader() -> Option<Shader> {
    let grad_colors = [
        Color4f::new(1.0, 0.0, 0.0, 1.0),
        Color4f::new(0.0, 1.0, 1.0, 1.0),
        Color4f::new(0.0, 1.0, 0.0, 1.0),
        Color4f::new(1.0, 1.0, 1.0, 1.0),
        Color4f::new(1.0, 0.0, 1.0, 1.0),
        Color4f::new(0.0, 0.0, 1.0, 1.0),
        Color4f::new(1.0, 1.0, 0.0, 1.0),
    ];
    let pts = [
        Point::new(100.0 / 4.0, 0.0),
        Point::new(3.0 * 100.0 / 4.0, 100.0),
    ];
    let grad = Gradient::new(
        Colors::new(&grad_colors, None, TileMode::Mirror, None),
        Interpolation::default(),
    );
    shaders::linear_gradient((pts[0], pts[1]), &grad, None)
}

// Port of: gm/patch.cpp#L34-L76 (chrome/m156), draw_control_points
fn draw_control_points(canvas: &Canvas, cubics: &[Point; patch_utils::NUM_CTRL_PTS]) {
    // draw control points
    let mut paint = Paint::default();
    let bottom = patch_utils::get_bottom_cubic(cubics);
    let top = patch_utils::get_top_cubic(cubics);
    let left = patch_utils::get_left_cubic(cubics);
    let right = patch_utils::get_right_cubic(cubics);

    paint.set_color(Color::BLACK);
    paint.set_stroke_width(0.5);
    let corners = [bottom[0], bottom[3], top[0], top[3]];
    canvas.draw_points(PointMode::Lines, &bottom, &paint);
    canvas.draw_points(PointMode::Lines, &bottom[1..3], &paint);
    canvas.draw_points(PointMode::Lines, &top, &paint);
    canvas.draw_points(PointMode::Lines, &left, &paint);
    canvas.draw_points(PointMode::Lines, &right, &paint);

    canvas.draw_points(PointMode::Lines, &top[1..3], &paint);
    canvas.draw_points(PointMode::Lines, &left[1..3], &paint);
    canvas.draw_points(PointMode::Lines, &right[1..3], &paint);

    paint.set_stroke_width(2.0);

    paint.set_color(Color::RED);
    canvas.draw_points(PointMode::Points, &corners, &paint);

    paint.set_color(Color::BLUE);
    canvas.draw_points(PointMode::Points, &bottom[1..3], &paint);

    paint.set_color(Color::CYAN);
    canvas.draw_points(PointMode::Points, &top[1..3], &paint);

    paint.set_color(Color::YELLOW);
    canvas.draw_points(PointMode::Points, &left[1..3], &paint);

    paint.set_color(Color::GREEN);
    canvas.draw_points(PointMode::Points, &right[1..3], &paint);
}

// The texture coordinates of the corners (gTexCoords).
// Port of: gm/patch.cpp#L93-L96 (chrome/m156), gTexCoords
const G_TEX_COORDS: [Point; patch_utils::NUM_CORNERS] = [
    Point::new(0.0, 0.0),
    Point::new(100.0, 0.0),
    Point::new(100.0, 100.0),
    Point::new(0.0, 100.0),
];

// `dopatch` for the no-image path (the GM passes `nullptr` for the image).
// Port of: gm/patch.cpp#L98-L150 (chrome/m156), dopatch
fn dopatch(
    canvas: &Canvas,
    colors: &[Color; patch_utils::NUM_CORNERS],
    img: Option<&Image>,
    local_matrix: Option<&Matrix>,
) {
    let mut paint = Paint::default();
    paint.set_color(Color::GREEN);

    let modes = [BlendMode::Src, BlendMode::Dst, BlendMode::ColorDodge];

    let mut tex_storage = [Point::new(0.0, 0.0); patch_utils::NUM_CORNERS];
    let mut tex = G_TEX_COORDS;
    let shader = if let Some(img) = img {
        let w = img.width() as f32;
        let h = img.height() as f32;
        let shader = img.to_shader(None, SamplingOptions::default(), local_matrix);
        tex_storage[0] = Point::new(0.0, 0.0);
        tex_storage[1] = Point::new(w, 0.0);
        tex_storage[2] = Point::new(w, h);
        tex_storage[3] = Point::new(0.0, h);
        tex = tex_storage;
        shader
    } else {
        make_shader()
    };

    canvas.save();
    for (y, &mode) in modes.iter().enumerate() {
        for x in 0..4 {
            canvas.save();
            canvas.translate((x as f32 * 350.0, y as f32 * 350.0));
            match x {
                0 => {
                    canvas.draw_patch(&G_CUBICS, None::<&[Color; 4]>, None, mode, &paint);
                }
                1 => {
                    canvas.draw_patch(&G_CUBICS, colors, None, mode, &paint);
                }
                2 => {
                    paint.set_shader(shader.clone());
                    canvas.draw_patch(&G_CUBICS, None::<&[Color; 4]>, Some(&tex), mode, &paint);
                    paint.set_shader(None);
                }
                3 => {
                    paint.set_shader(shader.clone());
                    canvas.draw_patch(&G_CUBICS, colors, Some(&tex), mode, &paint);
                    paint.set_shader(None);
                }
                _ => {}
            }

            draw_control_points(canvas, &G_CUBICS);
            canvas.restore();
        }
    }
    canvas.restore();
}

// Port of: gm/patch.cpp#L158-L162 (chrome/m156), DEF_SIMPLE_GM(patch_primitive)
crate::def_simple_gm!(patch_primitive, canvas, 1500, 1100, {
    let colors = [Color::RED, Color::GREEN, Color::BLUE, Color::CYAN];
    dopatch(canvas, &colors, None, None);
});

// Port of: gm/patch.cpp#L164-L168 (chrome/m156), DEF_SIMPLE_GM(patch_image)
crate::def_simple_gm!(patch_image, canvas, 1500, 1100, {
    let colors = [Color::RED, Color::GREEN, Color::BLUE, Color::CYAN];
    let image = crate::tool_utils::get_resource_as_image("images/mandrill_128.png")
        .expect("images/mandrill_128.png");
    dopatch(canvas, &colors, Some(&image), None);
});

// Port of: gm/patch.cpp#L170-L178 (chrome/m156), DEF_SIMPLE_GM(patch_image_persp)
crate::def_simple_gm!(patch_image_persp, canvas, 1500, 1100, {
    let colors = [Color::RED, Color::GREEN, Color::BLUE, Color::CYAN];
    // force perspective: localM[6] = 0.00001f
    let mut local_m = Matrix::new_identity();
    local_m.set_all(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.000_01, 0.0, 1.0);
    let image = crate::tool_utils::get_resource_as_image("images/mandrill_128.png")
        .expect("images/mandrill_128.png");
    dopatch(canvas, &colors, Some(&image), Some(&local_m));
});

// Port of: gm/patch.cpp#L179-L185 (chrome/m156), DEF_SIMPLE_GM(patch_alpha)
crate::def_simple_gm!(patch_alpha, canvas, 1500, 1100, {
    let colors = [
        Color::RED,
        Color::new(0x0000_FF00),
        Color::BLUE,
        Color::new(0x00FF_00FF),
    ];
    dopatch(canvas, &colors, None, None);
});
