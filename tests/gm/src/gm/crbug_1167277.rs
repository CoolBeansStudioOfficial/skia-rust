// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_1167277.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::QuadAAFlags;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

// Port of: gm/crbug_1167277.cpp#L17-L41 (chrome/m156)
fn draw_crbug_1167277(canvas: &Canvas) {
    canvas.translate((-1250.0, -900.0));
    // Matrix, clip, and quad values taken from Chrome repro scenario.
    let ctm = Matrix::new_all(
        f32::from_bits(0xBF8F_CFAE),
        f32::from_bits(0xBEAE_25EE),
        f32::from_bits(0x449C_A6DB),
        f32::from_bits(0x3C9D_C40F),
        f32::from_bits(0xBF95_0E35),
        f32::from_bits(0x4487_DA43),
        f32::from_bits(0xB8D4_D6BC),
        f32::from_bits(0xB92F_BB29),
        f32::from_bits(0x3F6F_605C),
    );
    let rect = Rect::new(
        f32::from_bits(0x0000_0000),
        f32::from_bits(0x0000_0000),
        f32::from_bits(0x4188_0000),
        f32::from_bits(0x4344_0000),
    );
    let clip = [
        Point::from((f32::from_bits(0x3EF4_34A2), f32::from_bits(0x4344_0004))),
        Point::from((f32::from_bits(0x0000_0000), f32::from_bits(0x4344_0009))),
        Point::from((f32::from_bits(0x38EF_605D), f32::from_bits(0x38EF_605D))),
        Point::from((f32::from_bits(0x3EF4_36E3), f32::from_bits(0x396F_5D30))),
    ];
    let mut color: u32 = 0xFF00_FF00; // SK_ColorGREEN
    for flags in 0..QuadAAFlags::ALL.bits() {
        let aa_flags = QuadAAFlags::from_bits_retain(flags);
        canvas.save();
        canvas.concat(&ctm);
        canvas.experimental_draw_edge_aa_quad(
            rect,
            Some(&clip),
            aa_flags,
            Color::from(color),
            BlendMode::SrcOver,
        );
        canvas.restore();
        canvas.translate((5.0, 0.0));
        let rgb = color & 0x00FF_FFFF;
        color = 0xFF00_0000 | (rgb << 4) | (rgb >> 20);
    }
}

// Port of: gm/crbug_1167277.cpp#L17-L41 (chrome/m156)
crate::def_simple_gm!(crbug_1167277, canvas, 230, 320, {
    draw_crbug_1167277(canvas);
});
