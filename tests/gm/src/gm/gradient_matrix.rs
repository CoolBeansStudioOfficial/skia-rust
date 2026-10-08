// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/gradient_matrix.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::color::colors;
use skia_rust_core::floating_point::float_midpoint;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

const G_COLORS: [Color4f; 2] = [colors::RED, colors::YELLOW];

// Port of: gm/gradient_matrix.cpp#L23-L25 (chrome/m156)
fn make_grad(tm: TileMode) -> Gradient<'static> {
    Gradient::new(
        Colors::new(&G_COLORS, None, tm, None),
        Interpolation::default(),
    )
}

// These annoying defines are necessary, because the only other alternative
// is to use SkIntToScalar(...) everywhere.
// Port of: gm/gradient_matrix.cpp#L27-L31 (chrome/m156)
const S_ZERO: f32 = 0.0;
const S_HALF: f32 = 0.5;
const S_ONE: f32 = 1.0;

// These arrays define the gradient stop points
// as x1, y1, x2, y2 per gradient to draw.
// Port of: gm/gradient_matrix.cpp#L33-L43 (chrome/m156)
const fn pt(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

const LINEAR_PTS: [[Point; 2]; 8] = [
    [pt(S_ZERO, S_ZERO), pt(S_ONE, S_ZERO)],
    [pt(S_ZERO, S_ZERO), pt(S_ZERO, S_ONE)],
    [pt(S_ONE, S_ZERO), pt(S_ZERO, S_ZERO)],
    [pt(S_ZERO, S_ONE), pt(S_ZERO, S_ZERO)],
    [pt(S_ZERO, S_ZERO), pt(S_ONE, S_ONE)],
    [pt(S_ONE, S_ONE), pt(S_ZERO, S_ZERO)],
    [pt(S_ONE, S_ZERO), pt(S_ZERO, S_ONE)],
    [pt(S_ZERO, S_ONE), pt(S_ONE, S_ZERO)],
];

// Port of: gm/gradient_matrix.cpp#L45-L55 (chrome/m156)
const RADIAL_PTS: [[Point; 2]; 8] = [
    [pt(S_ZERO, S_HALF), pt(S_ONE, S_HALF)],
    [pt(S_HALF, S_ZERO), pt(S_HALF, S_ONE)],
    [pt(S_ONE, S_HALF), pt(S_ZERO, S_HALF)],
    [pt(S_HALF, S_ONE), pt(S_HALF, S_ZERO)],
    [pt(S_ZERO, S_ZERO), pt(S_ONE, S_ONE)],
    [pt(S_ONE, S_ONE), pt(S_ZERO, S_ZERO)],
    [pt(S_ONE, S_ZERO), pt(S_ZERO, S_ONE)],
    [pt(S_ZERO, S_ONE), pt(S_ONE, S_ZERO)],
];

// These define the pixels allocated to each gradient image.
// Port of: gm/gradient_matrix.cpp#L57-L60 (chrome/m156)
const TESTGRID_X: f32 = 200.0;
const TESTGRID_Y: f32 = 200.0;

const IMAGES_X: usize = 4; // number of images per row

// Port of: gm/gradient_matrix.cpp#L62-L64 (chrome/m156)
fn make_linear_gradient(pts: &[Point; 2], local_matrix: &Matrix) -> Option<Shader> {
    shaders::linear_gradient(
        (pts[0], pts[1]),
        &make_grad(TileMode::Clamp),
        Some(local_matrix),
    )
}

// Port of: gm/gradient_matrix.cpp#L66-L72 (chrome/m156)
fn make_radial_gradient(pts: &[Point; 2], local_matrix: &Matrix) -> Option<Shader> {
    let center = Point::new(
        float_midpoint(pts[0].x, pts[1].x),
        float_midpoint(pts[0].y, pts[1].y),
    );
    let radius = (center - pts[0]).length();
    shaders::radial_gradient(
        (center, radius),
        &make_grad(TileMode::Clamp),
        Some(local_matrix),
    )
}

// Port of: gm/gradient_matrix.cpp#L74-L105 (chrome/m156)
fn draw_gradients(
    canvas: &Canvas,
    make_shader: fn(&[Point; 2], &Matrix) -> Option<Shader>,
    pts_array: &[[Point; 2]],
    num_images: usize,
) {
    // Use some nice prime numbers for the rectangle and matrix with
    // different scaling along the x and y axes (which is the bug this
    // test addresses, where incorrect order of operations mixed up the axes)
    let rect_grad = Rect::new(43.0, 61.0, 181.0, 167.0);
    let mut shader_mat = Matrix::new_identity();
    shader_mat.set_scale((rect_grad.width(), rect_grad.height()), None);
    shader_mat.post_translate((rect_grad.left(), rect_grad.top()));

    canvas.save();
    for (i, pts) in pts_array.iter().enumerate().take(num_images) {
        // Advance line downwards if necessary.
        if i % IMAGES_X == 0 && i != 0 {
            canvas.restore();
            canvas.translate((0.0, TESTGRID_Y));
            canvas.save();
        }

        let mut paint = Paint::default();
        paint.set_shader(make_shader(pts, &shader_mat));
        canvas.draw_rect(rect_grad, &paint);

        // Advance to next position.
        canvas.translate((TESTGRID_X, 0.0));
    }
    canvas.restore();
}

// Port of: gm/gradient_matrix.cpp#L107-L116 (chrome/m156)
crate::def_simple_gm_bg!(
    gradient_matrix,
    canvas,
    800,
    800,
    Color::new(0xFFDD_DDDD),
    {
        draw_gradients(canvas, make_linear_gradient, &LINEAR_PTS, LINEAR_PTS.len());

        canvas.translate((0.0, TESTGRID_Y));

        draw_gradients(canvas, make_radial_gradient, &RADIAL_PTS, RADIAL_PTS.len());
    }
);
