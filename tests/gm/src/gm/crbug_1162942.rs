// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_1162942.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::QuadAAFlags;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

// Port of: gm/crbug_1162942.cpp#L24-L59 (chrome/m156)
fn draw_crbug_1162942(canvas: &Canvas) {
    // Matrix and quad values taken from Chrome repro scenario.
    let mut ctm = Matrix::new_all(
        f32::from_bits(0x3FCC_7F75),
        f32::from_bits(0x3D57_84FC),
        f32::from_bits(0x44C4_8C99),
        f32::from_bits(0x3F69_9F7F),
        f32::from_bits(0x3E0A_0D37),
        f32::from_bits(0x4390_8518),
        f32::from_bits(0x3AA1_7423),
        f32::from_bits(0x3A6C_CDC3),
        f32::from_bits(0x3F2E_FEEC),
    );
    ctm.post_translate((-1500.0, -325.0));

    let pts = [
        Point::from((f32::from_bits(0x3F39_778B), f32::from_bits(0x43FF_7FFC))),
        Point::from((f32::from_bits(0x0000_0000), f32::from_bits(0x43FF_7FFA))),
        Point::from((f32::from_bits(0xB83B_055E), f32::from_bits(0x4250_0003))),
        Point::from((f32::from_bits(0x3F39_776F), f32::from_bits(0x4250_000D))),
    ];
    let bounds = Rect::bounds_or_empty(&pts);

    canvas.clear(Color::WHITE);

    let flags = [
        QuadAAFlags::TOP | QuadAAFlags::LEFT,
        QuadAAFlags::BOTTOM | QuadAAFlags::RIGHT,
        QuadAAFlags::BOTTOM,
        QuadAAFlags::RIGHT,
        QuadAAFlags::RIGHT | QuadAAFlags::LEFT,
        QuadAAFlags::TOP | QuadAAFlags::BOTTOM,
    ];

    let mut color: u32 = 0xFF00_FF00; // SK_ColorGREEN
    for aa_flags in flags {
        canvas.save();
        canvas.concat(&ctm);
        canvas.experimental_draw_edge_aa_quad(
            bounds,
            Some(&pts),
            aa_flags,
            Color::from(color),
            BlendMode::SrcOver,
        );
        let rgb = color & 0x00FF_FFFF;
        color = 0xFF00_0000 | (rgb << 4) | (rgb >> 20);
        canvas.restore();
        canvas.translate((0.0, 25.0));
    }
}

// Port of: gm/crbug_1162942.cpp#L24-L59 (chrome/m156)
crate::def_simple_gm!(crbug_1162942, canvas, 620, 200, {
    draw_crbug_1162942(canvas);
});
