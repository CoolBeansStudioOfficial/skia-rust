// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/RotatedRectBench.cpp

//! Rendering rotated rectangles, optionally anti-aliased, with a changing, transparent, alternating
//! or shader paint, under a perspective matrix (a rendering bench).
//!
//! Each `DEF_FOR_AA_MODES(blend)` and `DEF_FOR_PERSP_MODES(aa)` macro in C++ is one registration
//! with several benchmarks, so each is a `def_bench_set!`.

// The int-to-scalar casts mirror the C++ `SkIntToScalar` and `int` arithmetic.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

use crate::def_bench_set;
use crate::prelude::*;

/// `enum ColorType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColorType {
    ConstantOpaque,
    ConstantTransparent,
    ChangingOpaque,
    ChangingTransparent,
    AlternatingOpaqueAndTransparent,
    ShaderOpaque,
}

/// `start_color(ColorType)`.
// Port of: bench/RotatedRectBench.cpp#L30-L43 (chrome/m156)
fn start_color(ct: ColorType) -> u32 {
    match ct {
        ColorType::ConstantOpaque
        | ColorType::ChangingOpaque
        | ColorType::AlternatingOpaqueAndTransparent => 0xFFA0_7040,
        ColorType::ConstantTransparent | ColorType::ChangingTransparent => 0x80A0_7040,
        ColorType::ShaderOpaque => 0xFFFF_FFFF, // SK_ColorWHITE
    }
}

/// `advance_color(SkColor old, ColorType ct, int step)`. The unsigned adds wrap, as in C++.
// Port of: bench/RotatedRectBench.cpp#L45-L62 (chrome/m156)
fn advance_color(old: u32, ct: ColorType, step: i32) -> u32 {
    let ct = if ct == ColorType::AlternatingOpaqueAndTransparent {
        if step & 0x1 != 0 {
            ColorType::ChangingOpaque
        } else {
            ColorType::ChangingTransparent
        }
    } else {
        ct
    };
    match ct {
        ColorType::ConstantOpaque | ColorType::ConstantTransparent | ColorType::ShaderOpaque => old,
        ColorType::ChangingOpaque => 0xFF00_0000 | old.wrapping_add(0x0001_0307),
        ColorType::ChangingTransparent => {
            (0x00FF_FFFF & old.wrapping_add(0x0001_0307)) | 0x8000_0000
        }
        ColorType::AlternatingOpaqueAndTransparent => unreachable!("Can't get here"),
    }
}

/// `class RotRectBench`.
// Port of: bench/RotatedRectBench.cpp#L72-L185 (chrome/m156)
struct RotRectBench {
    aa: bool,
    perspective: bool,
    color_type: ColorType,
    mode: BlendMode,
    name: String,
}

impl RotRectBench {
    // Port of: bench/RotatedRectBench.cpp#L74-L80 (chrome/m156)
    fn new(aa: bool, ct: ColorType, mode: BlendMode, perspective: bool) -> Self {
        let name = make_name(aa, ct, mode, perspective);
        Self {
            aa,
            perspective,
            color_type: ct,
            mode,
            name,
        }
    }
}

/// `RotRectBench::makeName()`.
// Port of: bench/RotatedRectBench.cpp#L145-L176 (chrome/m156)
fn make_name(aa: bool, ct: ColorType, mode: BlendMode, perspective: bool) -> String {
    let mut name = String::from("rotated_rects");
    name.push_str(if aa { "_aa" } else { "_bw" });
    if perspective {
        name.push_str("_persp");
    }
    name.push_str(match ct {
        ColorType::ConstantOpaque => "_same_opaque",
        ColorType::ConstantTransparent => "_same_transparent",
        ColorType::ChangingOpaque => "_changing_opaque",
        ColorType::ChangingTransparent => "_changing_transparent",
        ColorType::AlternatingOpaqueAndTransparent => "_alternating_transparent_and_opaque",
        ColorType::ShaderOpaque => "_shader_opaque",
    });
    // to_lower(SkBlendMode_Name(fMode)): ASCII lower-casing, as C's tolower in the C locale.
    format!("{name}_{}", mode.name().to_ascii_lowercase())
}

const RECT_W: scalar = 25.1;
const RECT_H: scalar = 25.9;

impl Benchmark for RotRectBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/RotatedRectBench.cpp#L85-L142 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("RotRectBench is a rendering bench");
        let mut paint = Paint::default();
        paint.set_anti_alias(self.aa);
        paint.set_blend_mode(self.mode);
        let mut color = start_color(self.color_type);

        let size = self.size();
        let w = size.width;
        let h = size.height;

        if self.color_type == ColorType::ShaderOpaque {
            // The only requirement for the shader is that it requires local coordinates.
            let pts = [Point::new(0.0, 0.0), Point::new(RECT_W, RECT_H)];
            let colors = [
                Color4f::from_color(Color::from(color)),
                Color4f::from_color(Color::BLUE),
            ];
            let gradient = Gradient::new(
                Colors::new(&colors, None, TileMode::Clamp, None),
                Interpolation::default(),
            );
            let shader: Option<Shader> =
                shaders::linear_gradient((pts[0], pts[1]), &gradient, None);
            paint.set_shader(shader);
        }

        let mut rotate = Matrix::new_identity();
        // This value was chosen so that we frequently hit the axis-aligned case.
        rotate.set_rotate(30.0, Point::new(RECT_W / 2.0, RECT_H / 2.0));
        let mut m = rotate.clone();

        let mut tx: scalar = 0.0;
        let mut ty: scalar = 0.0;

        if self.perspective {
            // Apply some fixed perspective to change how ops may draw the rects.
            let mut perspective = Matrix::new_identity();
            perspective.set_identity();
            perspective.set_persp_x(1e-4);
            perspective.set_persp_y(1e-3);
            perspective.set_skew_x(0.1);
            canvas.concat(&perspective);
        }

        for i in 0..loops {
            canvas.save();
            canvas.translate((tx, ty));
            canvas.concat(&m);
            paint.set_color(Color::from(color));
            color = advance_color(color, self.color_type, i);

            canvas.draw_rect(Rect::from_wh(RECT_W, RECT_H), &paint);
            canvas.restore();

            tx += RECT_W + 2.0;
            if tx > w as scalar {
                tx = 0.0;
                ty += RECT_H + 2.0;
                if ty > h as scalar {
                    ty = 0.0;
                }
            }

            m.post_concat(&rotate);
        }
    }
}

/// `DEF_FOR_COLOR_TYPES(aa, blend)`: the six color types, in C++ order.
fn for_color_types(aa: bool, blend: BlendMode) -> Vec<Box<dyn Benchmark>> {
    [
        ColorType::ConstantOpaque,
        ColorType::ConstantTransparent,
        ColorType::ChangingOpaque,
        ColorType::ChangingTransparent,
        ColorType::AlternatingOpaqueAndTransparent,
        ColorType::ShaderOpaque,
    ]
    .into_iter()
    .map(|ct| Box::new(RotRectBench::new(aa, ct, blend, false)) as Box<dyn Benchmark>)
    .collect()
}

/// `DEF_FOR_AA_MODES(blend)`: anti-aliased, then black and white.
fn for_aa_modes(blend: BlendMode) -> Vec<Box<dyn Benchmark>> {
    let mut all = for_color_types(true, blend);
    all.extend(for_color_types(false, blend));
    all
}

/// `DEF_FOR_PERSP_MODES(aa)`: a limited run of perspective benches.
fn for_persp_modes(aa: bool) -> Vec<Box<dyn Benchmark>> {
    vec![
        Box::new(RotRectBench::new(
            aa,
            ColorType::ConstantOpaque,
            BlendMode::SrcOver,
            true,
        )) as Box<dyn Benchmark>,
        Box::new(RotRectBench::new(
            aa,
            ColorType::ShaderOpaque,
            BlendMode::SrcOver,
            true,
        )),
    ]
}

// Choose kSrcOver because it always allows coverage and alpha to be conflated. kSrc only allows
// conflation when opaque, and kDarken because it isn't possilbe with standard GL blending.
// Port of: bench/RotatedRectBench.cpp#L200-L202 (chrome/m156)
def_bench_set!(
    rotated_rect_bench_src_over = "SkBlendMode::kSrcOver",
    for_aa_modes(BlendMode::SrcOver)
);
// Port of: bench/RotatedRectBench.cpp#L201-L201 (chrome/m156)
def_bench_set!(
    rotated_rect_bench_src = "SkBlendMode::kSrc",
    for_aa_modes(BlendMode::Src)
);
// Port of: bench/RotatedRectBench.cpp#L202-L202 (chrome/m156)
def_bench_set!(
    rotated_rect_bench_darken = "SkBlendMode::kDarken",
    for_aa_modes(BlendMode::Darken)
);
// Only do a limited run of perspective tests.
// Port of: bench/RotatedRectBench.cpp#L208-L208 (chrome/m156)
def_bench_set!(
    rotated_rect_bench_persp_true = "true",
    for_persp_modes(true)
);
// Port of: bench/RotatedRectBench.cpp#L209-L209 (chrome/m156)
def_bench_set!(
    rotated_rect_bench_persp_false = "false",
    for_persp_modes(false)
);
