// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bigmatrix.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::pre_multiply_color;
use skia_rust_core::color_data::{pixel16_to_color, pixel32_to_pixel16};
use skia_rust_core::color_priv::pack_argb32;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;

/// `ToolUtils::color_to_565`: rounds `color` to what a 565 surface would store.
// Port of: tools/ToolUtils.cpp#L142-L151 (chrome/m156)
fn color_to_565(color: Color) -> Color {
    // Not a good idea to use this function for greyscale colors...
    // it will add an obvious purple or green tint.
    debug_assert!(color.r() != color.g() || color.r() != color.b() || color.g() != color.b());

    let pm_color = pre_multiply_color(color);
    let color16 = pixel32_to_pixel16(pm_color);
    pixel16_to_color(color16)
}

// Port of: gm/bigmatrix.cpp#L23-L65 (chrome/m156)
crate::def_simple_gm_bg!(
    bigmatrix,
    canvas,
    50,
    50,
    color_to_565(Color::new(0xFF66_AA99)),
    {
        let mut m = Matrix::new_identity();
        m.reset();
        m.set_rotate(33.0, None);
        m.post_scale((3000.0, 3000.0), None);
        m.post_translate((6000.0, -5000.0));
        canvas.concat(&m);

        let mut paint = Paint::default();
        paint.set_color(Color::RED);
        paint.set_anti_alias(true);

        let m = m.invert().expect("the matrix is invertible");

        let small = 1.0 / 500.0;

        let mut pt = m.map_point((10.0, 10.0));
        canvas.draw_circle(pt, small, &paint);

        pt = m.map_point((30.0, 10.0));
        let mut rect = Rect::new(pt.x - small, pt.y - small, pt.x + small, pt.y + small);
        canvas.draw_rect(rect, &paint);

        let mut bmp = Bitmap::new();
        bmp.alloc_n32_pixels((2, 2), None);
        bmp.set_addr32(0, 0, pack_argb32(0xFF, 0xFF, 0x00, 0x00));
        bmp.set_addr32(1, 0, pack_argb32(0xFF, 0x00, 0xFF, 0x00));
        bmp.set_addr32(0, 1, pack_argb32(0x80, 0x00, 0x00, 0x00));
        bmp.set_addr32(1, 1, pack_argb32(0xFF, 0x00, 0x00, 0xFF));
        pt = m.map_point((30.0, 30.0));
        let mut s = Matrix::new_identity();
        s.reset();
        s.set_scale((1.0 / 1000.0, 1.0 / 1000.0), None);
        paint.set_shader(bmp.to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            SamplingOptions::from(FilterMode::Linear),
            &s,
        ));
        paint.set_anti_alias(false);
        rect.set_ltrb(pt.x - small, pt.y - small, pt.x + small, pt.y + small);
        canvas.draw_rect(rect, &paint);
    }
);
