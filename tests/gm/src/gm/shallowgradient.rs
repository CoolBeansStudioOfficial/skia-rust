// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/shallowgradient.cpp (chrome/m156)

// mirrors the C++ `const SkColor[2]&` parameters and int -> float casts
#![allow(clippy::trivially_copy_pass_by_ref, clippy::cast_precision_loss)]

use crate::prelude::*;
use skia_rust_core::color::Color;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader;
use skia_rust_core::size::Size;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

type MakeShaderProc = fn(&Gradient<'_>, &Size) -> Option<Shader>;

// Port of: gm/shallowgradient.cpp#L26-L29 (chrome/m156)
fn shader_linear(grad: &Gradient<'_>, size: &Size) -> Option<Shader> {
    let pts = [Point::new(0.0, 0.0), Point::new(size.width, size.height)];
    shaders::linear_gradient((pts[0], pts[1]), grad, None)
}

// Port of: gm/shallowgradient.cpp#L31-L34 (chrome/m156)
fn shader_radial(grad: &Gradient<'_>, size: &Size) -> Option<Shader> {
    let center = Point::new(size.width / 2.0, size.height / 2.0);
    shaders::radial_gradient((center, size.width / 2.0), grad, None)
}

// Port of: gm/shallowgradient.cpp#L36-L40 (chrome/m156)
fn shader_conical(grad: &Gradient<'_>, size: &Size) -> Option<Shader> {
    let center = Point::new(size.width / 2.0, size.height / 2.0);
    shaders::two_point_conical_gradient(
        (center, size.width / 64.0),
        (center, size.width / 2.0),
        grad,
        None,
    )
}

// Port of: gm/shallowgradient.cpp#L42-L44 (chrome/m156)
fn shader_sweep(grad: &Gradient<'_>, size: &Size) -> Option<Shader> {
    shaders::sweep_gradient(
        Point::new(size.width / 2.0, size.height / 2.0),
        (0.0, 360.0),
        grad,
        None,
    )
}

// Port of: gm/shallowgradient.cpp#L46-L72 (chrome/m156)
struct ShallowGradientGm {
    proc: MakeShaderProc,
    name: &'static str,
    dither: bool,
}

impl ShallowGradientGm {
    fn new(proc: MakeShaderProc, name: &'static str, dither: bool) -> Self {
        Self { proc, name, dither }
    }
}

impl GM for ShallowGradientGm {
    fn name(&self) -> String {
        format!(
            "shallow_gradient_{}{}",
            self.name,
            if self.dither { "" } else { "_nodither" }
        )
    }

    fn size(&mut self) -> ISize {
        ISize::new(800, 800)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let colors = [
            Color4f::from_color(Color::new(0xFF55_5555)),
            Color4f::from_color(Color::new(0xFF44_4444)),
        ];
        let grad = Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        );

        let size = self.size();
        let r = Rect::new(0.0, 0.0, size.width as f32, size.height as f32);
        let size = Size::new(r.width(), r.height());

        let mut paint = Paint::default();
        paint.set_shader((self.proc)(&grad, &size));
        paint.set_dither(self.dither);
        canvas.draw_rect(r, &paint);
    }
}

// Port of: gm/shallowgradient.cpp#L76-L87 (chrome/m156)
crate::def_gm!(
    linear,
    ShallowGradientGm::new(shader_linear, "linear", true)
);
crate::def_gm!(
    radial,
    ShallowGradientGm::new(shader_radial, "radial", true)
);
crate::def_gm!(
    conical,
    ShallowGradientGm::new(shader_conical, "conical", true)
);
crate::def_gm!(sweep, ShallowGradientGm::new(shader_sweep, "sweep", true));

crate::def_gm!(
    linear_2 = "linear#2",
    ShallowGradientGm::new(shader_linear, "linear", false)
);
crate::def_gm!(
    radial_2 = "radial#2",
    ShallowGradientGm::new(shader_radial, "radial", false)
);
crate::def_gm!(
    conical_2 = "conical#2",
    ShallowGradientGm::new(shader_conical, "conical", false)
);
crate::def_gm!(
    sweep_2 = "sweep#2",
    ShallowGradientGm::new(shader_sweep, "sweep", false)
);
