// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/colormatrix.cpp (chrome/m156)

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
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::image::Image;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_raster::surfaces;

// Port of: gm/colormatrix.cpp#L11-L13 (chrome/m156)
const WIDTH: i32 = 500;
// Port of: gm/colormatrix.cpp#L11-L13 (chrome/m156)
const HEIGHT: i32 = 160;

// Port of: gm/colormatrix.cpp#L14-L20 (chrome/m156)
fn set_color_matrix(paint: &mut Paint, matrix: &ColorMatrix) {
    paint.set_color_filter(color_filters::matrix(matrix, Clamp::Yes));
}

// Port of: gm/colormatrix.cpp#L21-L23 (chrome/m156)
fn set_array(paint: &mut Paint, array: &[f32; 20]) {
    paint.set_color_filter(color_filters::matrix_row_major(array, Clamp::Yes));
}

// Port of: gm/colormatrix.cpp#L28-L43 (chrome/m156)
fn create_solid_bitmap(width: i32, height: i32) -> Option<Image> {
    let mut surf = surfaces::raster_n32_premul((width, height))?;
    let canvas = surf.canvas();
    canvas.clear(Color4f::new(0.0, 0.0, 0.0, 0.0));
    for y in 0..height {
        for x in 0..width {
            let mut paint = Paint::default();
            paint.set_color(Color::from_argb(
                255,
                (x * 255 / width) as u8,
                (y * 255 / height) as u8,
                0,
            ));
            canvas.draw_rect(Rect::from_xywh(x as f32, y as f32, 1.0, 1.0), &paint);
        }
    }
    surf.image_snapshot()
}

// Port of: gm/colormatrix.cpp#L44-L56 (chrome/m156)
fn create_transparent_bitmap(width: i32, height: i32) -> Option<Image> {
    let mut surf = surfaces::raster_n32_premul((width, height))?;
    let canvas = surf.canvas();
    canvas.clear(Color4f::new(0.0, 0.0, 0.0, 0.0));
    let pts = [
        Point::new(0.0, 0.0),
        Point::new(width as f32, height as f32),
    ];
    let colors = [
        Color4f::new(0.0, 0.0, 0.0, 0.0),
        Color4f::new(1.0, 1.0, 1.0, 1.0),
    ];
    let mut paint = Paint::default();
    paint.set_shader(shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    ));
    canvas.draw_rect(Rect::from_wh(width as f32, height as f32), &paint);
    surf.image_snapshot()
}

// Port of: gm/colormatrix.cpp#L24-L100 (chrome/m156)
struct ColorMatrixGm {
    solid_img: Option<Image>,
    transparent_img: Option<Image>,
}

impl ColorMatrixGm {
    fn new() -> Self {
        Self {
            solid_img: None,
            transparent_img: None,
        }
    }
}

impl GM for ColorMatrixGm {
    fn name(&self) -> String {
        "colormatrix".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    fn bg_color(&self) -> Color {
        Color::new(0xFF808080)
    }

    // Port of: gm/colormatrix.cpp#L37-L40 (chrome/m156)
    fn on_once_before_draw(&mut self) {
        self.solid_img = create_solid_bitmap(64, 64);
        self.transparent_img = create_transparent_bitmap(64, 64);
    }

    // Port of: gm/colormatrix.cpp#L57-L92 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        let mut matrix = ColorMatrix::default();
        paint.set_blend_mode(BlendMode::Src);
        let bmps = [self.solid_img.as_ref(), self.transparent_img.as_ref()];
        for bmp in bmps.into_iter().flatten() {
            matrix.set_identity();
            set_color_matrix(&mut paint, &matrix);
            canvas.draw_image(bmp, (0.0, 0.0), Some(&paint));

            matrix.set_saturation(0.0);
            set_color_matrix(&mut paint, &matrix);
            canvas.draw_image(bmp, (80.0, 0.0), Some(&paint));

            matrix.set_saturation(0.5);
            set_color_matrix(&mut paint, &matrix);
            canvas.draw_image(bmp, (160.0, 0.0), Some(&paint));

            matrix.set_saturation(1.0);
            set_color_matrix(&mut paint, &matrix);
            canvas.draw_image(bmp, (240.0, 0.0), Some(&paint));

            matrix.set_saturation(2.0);
            set_color_matrix(&mut paint, &matrix);
            canvas.draw_image(bmp, (320.0, 0.0), Some(&paint));

            let data: [f32; 20] = [
                0.0, 0.0, 0.0, 0.0, 1.0, //
                0.0, 0.0, 0.0, 0.0, 1.0, //
                0.0, 0.0, 0.0, 0.0, 1.0, //
                1.0, 0.0, 0.0, 0.0, 0.0,
            ];
            set_array(&mut paint, &data);
            canvas.draw_image(bmp, (400.0, 0.0), Some(&paint));

            canvas.translate((0.0, 80.0));
        }
    }
}

crate::def_gm!(ColorMatrixGM, ColorMatrixGm::new());
