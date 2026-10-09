// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_1174186.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::QuadAAFlags;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

// Port of: gm/crbug_1174186.cpp#L18-L43 (chrome/m156)
fn draw_crbug_1174186(canvas: &Canvas) {
    let m = Matrix::new_all(
        f32::from_bits(0x2448_0629),
        f32::from_bits(0xBF35_55C2),
        f32::from_bits(0x4377_D67B),
        f32::from_bits(0x23A6_1D51),
        f32::from_bits(0x3F34_B400),
        f32::from_bits(0x4453_F572),
        f32::from_bits(0x0000_0000),
        f32::from_bits(0x0000_0000),
        f32::from_bits(0x3F80_0000),
    );

    let pts = [
        Point::from((f32::from_bits(0x3F7F_FFF2), f32::from_bits(0x4348_3D60))),
        Point::from((f32::from_bits(0x0000_0000), f32::from_bits(0x4348_3D60))),
        Point::from((f32::from_bits(0x0000_0000), f32::from_bits(0x4311_A628))),
        Point::from((f32::from_bits(0x3F80_0000), f32::from_bits(0x4313_0F8C))),
    ];
    let mut color: u32 = 0xFF00_FF00; // SK_ColorGREEN
    canvas.translate((-500.0, 0.0));
    for _ in 0..10 {
        for flags in 0..QuadAAFlags::ALL.bits() {
            let aa_flags = QuadAAFlags::from_bits_retain(flags);
            canvas.save();
            canvas.concat(&m);
            canvas.experimental_draw_edge_aa_quad(
                Rect::from_wh(1000.0, 1000.0),
                Some(&pts),
                aa_flags,
                Color::from(color),
                BlendMode::SrcOver,
            );
            canvas.restore();
            canvas.translate((5.1, 0.0));
            let rgb = color & 0x00FF_FFFF;
            color = 0xFF00_0000 | (rgb << 4) | (rgb >> 20);
        }
    }
}

// Port of: gm/crbug_1174186.cpp#L18-L43 (chrome/m156)
crate::def_simple_gm!(crbug_1174186, canvas, 1200, 1200, {
    draw_crbug_1174186(canvas);
});
