// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/transparency.cpp (chrome/m156)

// This GM verifies that a transparent bitmap drawn over a checkerboard pattern looks correct.

// The int-to-scalar casts of small counts mirror the C++ arithmetic (exact in f32).
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color4f;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_raster::surfaces;

// Port of: gm/transparency.cpp#L26-L51 (chrome/m156), make_transparency
fn make_transparency(canvas: &Canvas, width: scalar, height: scalar) {
    let pts = [Point::new(0.0, 0.0), Point::new(width, 0.0)];
    let k_colors = [
        Color::BLACK,
        Color::GRAY,
        Color::WHITE,
        Color::RED,
        Color::YELLOW,
        Color::GREEN,
        Color::CYAN,
        Color::BLUE,
        Color::MAGENTA,
    ];
    let row_height = height / k_colors.len() as scalar;
    for (i, color) in k_colors.iter().enumerate() {
        let shader_colors = [
            Color4f::new(0.0, 0.0, 0.0, 0.0),
            Color4f::from_color(*color),
        ];
        let grad = Gradient::new(
            Colors::new(&shader_colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        );
        let mut p = Paint::default();
        p.set_shader(shaders::linear_gradient((pts[0], pts[1]), &grad, None));
        canvas.draw_rect(
            Rect::from_xywh(0.0, i as scalar * row_height, width, row_height),
            &p,
        );
    }
}

// http://crrev.com/834303005
// Port of: gm/transparency.cpp#L54-L61 (chrome/m156), create_checkerboard_shader
fn create_checkerboard_shader(c1: Color, c2: Color, size: i32) -> Option<Shader> {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((2 * size, 2 * size), None);
    bm.erase_color(c1);
    bm.erase_area(IRect::from_ltrb(0, 0, size, size), c2);
    bm.erase_area(IRect::from_ltrb(size, size, 2 * size, 2 * size), c2);
    bm.to_shader(
        (TileMode::Repeat, TileMode::Repeat),
        SamplingOptions::default(),
        None,
    )
}

// http://crrev.com/834303005
// Port of: gm/transparency.cpp#L63-L68 (chrome/m156), checkerboard
fn checkerboard(canvas: &Canvas, c1: Color, c2: Color, size: i32) {
    let mut paint = Paint::default();
    paint.set_shader(create_checkerboard_shader(c1, c2, size));
    canvas.draw_paint(&paint);
}

// This GM verifies that a transparent bitmap drawn over a checkerboard pattern looks correct.
// Port of: gm/transparency.cpp#L70-L82 (chrome/m156), DEF_SIMPLE_GM(transparency_check)
crate::def_simple_gm!(transparency_check, canvas, 1792, 1080, {
    checkerboard(canvas, Color::new(0xFF99_9999), Color::new(0xFF66_6666), 8);
    {
        canvas.save();
        let surface = surfaces::raster_n32_premul((256, 9));
        let mut surface = surface.expect("a raster surface");
        make_transparency(surface.canvas(), 256.0, 9.0);
        canvas.scale((7.0, 120.0));
        surface.draw(canvas, (0.0, 0.0), SamplingOptions::default(), None);
        canvas.restore();
    }
});
