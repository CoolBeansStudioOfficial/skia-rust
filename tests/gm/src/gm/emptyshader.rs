// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/emptyshader.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};

// Port of: gm/emptyshader.cpp#L16-L18 (chrome/m156), empty
#[allow(clippy::unnecessary_wraps)] // the other makers return Option<Shader>, as in the C++ array
fn empty(_r: Rect) -> Option<Shader> {
    Some(shaders::empty())
}

// Port of: gm/emptyshader.cpp#L20-L27 (chrome/m156), degen_sweep
fn degen_sweep(r: Rect) -> Option<Shader> {
    // A too small angle between start and end falls back to an empty shader
    let start_angle: f32 = 0.0;
    let end_angle: f32 = start_angle.next_up();
    let colors = [Color4f::from(Color::RED), Color4f::from(Color::GREEN)];
    gradient_shaders::sweep_gradient(
        r.center(),
        (start_angle, end_angle),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Decal, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/emptyshader.cpp#L29-L35 (chrome/m156), degen_linear
fn degen_linear(r: Rect) -> Option<Shader> {
    // Having the two positions be the same causes a fallback to an empty shader
    let pts = [r.center(), r.center()];
    let colors = [Color4f::from(Color::RED), Color4f::from(Color::GREEN)];
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Decal, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/emptyshader.cpp#L37-L42 (chrome/m156), degen_radial
fn degen_radial(r: Rect) -> Option<Shader> {
    let colors = [Color4f::from(Color::RED), Color4f::from(Color::GREEN)];
    // Having a radius of 0.0 causes a fallback to an empty shader
    gradient_shaders::radial_gradient(
        (r.center(), 0.0),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Decal, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/emptyshader.cpp#L44-L51 (chrome/m156), degen_conical
fn degen_conical(r: Rect) -> Option<Shader> {
    let colors = [Color4f::from(Color::RED), Color4f::from(Color::GREEN)];
    // Having the start and end radii be the same causes a fallback to an empty shader
    gradient_shaders::two_point_conical_gradient(
        (r.center(), 0.0),
        (r.center(), 0.0),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Decal, None),
            Interpolation::default(),
        ),
        None,
    )
}

const K_PAD: i32 = 8;
const K_SIZE: i32 = 32;

// Port of: gm/emptyshader.cpp#L53-L90 (chrome/m156), EmptyShaderGM
struct EmptyShaderGm;

impl GM for EmptyShaderGm {
    fn name(&self) -> String {
        "emptyshader".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(128, 88)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFCC_CCCC)
    }

    // Port of: gm/emptyshader.cpp#L63-L84 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut stroke = Paint::default();
        stroke.set_style(Style::Stroke);
        let mut left = K_PAD;
        let mut top = K_PAD;
        let makers: [fn(Rect) -> Option<Shader>; 5] = [
            empty,
            degen_sweep,
            degen_linear,
            degen_radial,
            degen_conical,
        ];
        for f in makers {
            let r = Rect::new(
                int_to_scalar(left),
                int_to_scalar(top),
                int_to_scalar(left + K_SIZE),
                int_to_scalar(top + K_SIZE),
            );
            let mut p = Paint::default();
            p.set_color(Color::BLUE);
            p.set_shader(f(r));
            canvas.draw_rect(r, &p);
            canvas.draw_rect(r, &stroke);
            left += K_SIZE + K_PAD;
            if left >= 128 {
                left = K_PAD;
                top += K_SIZE + K_PAD;
            }
        }
    }
}

// Port of: gm/emptyshader.cpp#L92 (chrome/m156)
crate::def_gm!(EmptyShaderGM, EmptyShaderGm);
