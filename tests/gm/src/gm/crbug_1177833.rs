// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/crbug_1177833.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::QuadAAFlags;
use skia_rust_core::color::Color4f;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

// Port of: gm/crbug_1177833.cpp#L16-L98 (chrome/m156)
#[allow(clippy::too_many_lines)] // mirrors the three quads of the C++ GM
fn draw_crbug_1177833(canvas: &Canvas) {
    canvas.clear(Color::BLACK);
    canvas.translate((-700.0, -700.0));
    // This quad had two issues. The inset collapsed the inner 2D projected quad to a point but
    // didn't enable enough degrees of freedom to adjust the 4 3D points to project to that point.
    // Also, the outset produced a 2D projected point far away from the original quad but the
    // shader was not checking the geometric subset and so pixels far away from the projection of
    // the quad would have positive coverage.
    {
        canvas.save();
        canvas.concat(&Matrix::new_all(
            f32::from_bits(0xBF79_250E),
            f32::from_bits(0x3E9D_A860),
            f32::from_bits(0x4491_4C8A),
            f32::from_bits(0xBF98_2962),
            f32::from_bits(0xBF28_0002),
            f32::from_bits(0x44C3_116E),
            f32::from_bits(0xBA9B_FE62),
            f32::from_bits(0x39D1_0455),
            f32::from_bits(0x3FC9_B377),
        ));
        let rect = Rect::new(
            f32::from_bits(0x0000_0000),
            f32::from_bits(0x0000_0000),
            f32::from_bits(0x40A0_0000),
            f32::from_bits(0x4356_0000),
        );
        let clip = [
            Point::from((f32::from_bits(0x409F_FF57), f32::from_bits(0x40C8_6A18))),
            Point::from((f32::from_bits(0x409F_FF57), f32::from_bits(0x4314_DC8C))),
            Point::from((f32::from_bits(0x407F_6B0D), f32::from_bits(0x4315_7FFF))),
            Point::from((f32::from_bits(0x4040_859C), f32::from_bits(0x4314_0374))),
        ];
        let aa_flags = QuadAAFlags::from_bits_retain(0x0000_0002);
        let color = Color4f::new(
            f32::from_bits(0x3F6E_EEF0),
            f32::from_bits(0x3F6E_EEF0),
            f32::from_bits(0x3F6E_EEF0),
            f32::from_bits(0x3F80_0000),
        );
        // SkBlendMode 0x3 is kSrcOver.
        canvas.experimental_draw_edge_aa_quad(
            rect,
            Some(&clip),
            aa_flags,
            color,
            BlendMode::SrcOver,
        );
        canvas.restore();
    }
    // This quad also exposed the inset collapse to a point without enough degrees of freedom issue.
    canvas.save();
    canvas.translate((-300.0, 0.0));
    {
        canvas.save();
        canvas.concat(&Matrix::new_all(
            f32::from_bits(0x3F54_DD8A),
            f32::from_bits(0xBF90_96A4),
            f32::from_bits(0x447E_AE34),
            f32::from_bits(0x3F3F_6905),
            f32::from_bits(0xBE52_08BA),
            f32::from_bits(0x4418_118B),
            f32::from_bits(0x3AA1_34A1),
            f32::from_bits(0xB93E_F249),
            f32::from_bits(0x3F58_0BD4),
        ));
        let rect = Rect::new(
            f32::from_bits(0x0000_0000),
            f32::from_bits(0x0000_0000),
            f32::from_bits(0x40A0_0000),
            f32::from_bits(0x4356_0000),
        );
        let clip = [
            Point::from((f32::from_bits(0x40A0_000E), f32::from_bits(0x40C8_6B5A))),
            Point::from((f32::from_bits(0x40A0_001E), f32::from_bits(0x4314_DD5F))),
            Point::from((f32::from_bits(0x407F_76EB), f32::from_bits(0x4315_80C2))),
            Point::from((f32::from_bits(0x4040_92E7), f32::from_bits(0x4314_0445))),
        ];
        let aa_flags = QuadAAFlags::from_bits_retain(0x0000_0002);
        let color = Color4f::new(
            f32::from_bits(0x3F6E_EEF0),
            f32::from_bits(0x3F6E_EEF0),
            f32::from_bits(0x3F6E_EEF0),
            f32::from_bits(0x3F80_0000),
        );
        canvas.experimental_draw_edge_aa_quad(
            rect,
            Some(&clip),
            aa_flags,
            color,
            BlendMode::SrcOver,
        );
        canvas.restore();
    }
    canvas.restore();
    // This quad exposed a similar issue to the point issue above, but when collapsing to a
    // triangle. When a 2D quad edge collapsed from insetting we'd replace it with a point off of
    // its adjacent edges. We need to ensure the code that moves the 3D point that projects to
    // the 2D point has 2 degrees of freedom so it can find the correct 3D point.
    {
        canvas.save();
        canvas.concat(&Matrix::new_all(
            f32::from_bits(0x3F54_B255),
            f32::from_bits(0x3EB5_A94D),
            f32::from_bits(0x443D_7419),
            f32::from_bits(0x3F88_5D66),
            f32::from_bits(0x3F5A_6B9C),
            f32::from_bits(0x443C_7334),
            f32::from_bits(0x3AA9_5EA5),
            f32::from_bits(0xB8A1_391E),
            f32::from_bits(0x3F84_DDE5),
        ));
        let rect = Rect::new(
            f32::from_bits(0x0000_0000),
            f32::from_bits(0x0000_0000),
            f32::from_bits(0x40A0_0000),
            f32::from_bits(0x4310_0000),
        );
        let clip = [
            Point::from((f32::from_bits(0x405A_654C), f32::from_bits(0x42E8_C790))),
            Point::from((f32::from_bits(0x3728_C61B), f32::from_bits(0x42E7_DF31))),
            Point::from((f32::from_bits(0xB678_ECC5), f32::from_bits(0x412D_B4E0))),
            Point::from((f32::from_bits(0x4024_B2AD), f32::from_bits(0x413A_B3ED))),
        ];
        let aa_flags = QuadAAFlags::from_bits_retain(0x0000_0004);
        let color = Color4f::new(
            f32::from_bits(0x3F80_0000),
            f32::from_bits(0x3F80_0000),
            f32::from_bits(0x3F80_0000),
            f32::from_bits(0x3F80_0000),
        );
        canvas.experimental_draw_edge_aa_quad(
            rect,
            Some(&clip),
            aa_flags,
            color,
            BlendMode::SrcOver,
        );
        canvas.restore();
    }
}

// Port of: gm/crbug_1177833.cpp#L16-L98 (chrome/m156)
crate::def_simple_gm!(crbug_1177833, canvas, 400, 400, {
    draw_crbug_1177833(canvas);
});
