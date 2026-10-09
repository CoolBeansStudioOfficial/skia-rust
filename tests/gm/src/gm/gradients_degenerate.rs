// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/gradients_degenerate.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::font::Font;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/gradients_degenerate.cpp#L8-L9 (chrome/m156), COLORS and POS
const COLORS: [Color4f; 5] = [
    Color4f::new(1.0, 0.0, 0.0, 1.0), // kRed
    Color4f::new(1.0, 1.0, 1.0, 1.0), // kWhite
    Color4f::new(0.0, 0.0, 1.0, 1.0), // kBlue
    Color4f::new(0.0, 0.0, 0.0, 1.0), // kBlack
    Color4f::new(0.0, 1.0, 0.0, 1.0), // kGreen
];
const POS: [f32; 5] = [0.0, 0.0, 0.5, 1.0, 1.0];

// Port of: gm/gradients_degenerate.cpp#L10-L16 (chrome/m156), TILE_MODES and TILE_NAMES
const TILE_MODES: [TileMode; 4] = [
    TileMode::Decal,
    TileMode::Repeat,
    TileMode::Mirror,
    TileMode::Clamp,
];
const TILE_NAMES: [&str; 4] = ["decal", "repeat", "mirror", "clamp"];
const TILE_SIZE: i32 = 100;
const TILE_GAP: i32 = 10;
const CENTER: (f32, f32) = (50.0, 50.0);

// Port of: gm/gradients_degenerate.cpp#L23-L25 (chrome/m156), grad
fn grad(tm: TileMode) -> Gradient<'static> {
    Gradient::new(
        Colors::new(&COLORS, Some(&POS), tm, None),
        Interpolation::default(),
    )
}

// Port of: gm/gradients_degenerate.cpp#L36-L58 (chrome/m156), draw_tile_header
fn draw_tile_header(canvas: &Canvas) {
    canvas.save();
    let font = Font::from_size(default_portable_typeface(), 12.0);
    for name in TILE_NAMES {
        canvas.draw_str(name, (0.0, 0.0), &font, &Paint::default());
        canvas.translate((int_to_scalar(TILE_SIZE + TILE_GAP), 0.0));
    }
    canvas.restore();
    // Now adjust to start at rows below the header
    canvas.translate((0.0, int_to_scalar(2 * TILE_GAP)));
}

// Port of: gm/gradients_degenerate.cpp#L60-L84 (chrome/m156), draw_row
fn draw_row(canvas: &Canvas, desc: &str, factory: fn(TileMode) -> Option<Shader>) {
    canvas.save();
    let mut text = Paint::default();
    text.set_anti_alias(true);
    let font = Font::from_size(default_portable_typeface(), 12.0);
    canvas.translate((0.0, int_to_scalar(TILE_GAP)));
    canvas.draw_str(desc, (0.0, 0.0), &font, &text);
    canvas.translate((0.0, int_to_scalar(TILE_GAP)));
    let mut paint = Paint::default();
    paint.set_color(Color::BLACK);
    paint.set_style(Style::StrokeAndFill);
    paint.set_stroke_width(2.0);
    for tm in TILE_MODES {
        paint.set_shader(factory(tm));
        canvas.draw_rect(
            Rect::new(0.0, 0.0, int_to_scalar(TILE_SIZE), int_to_scalar(TILE_SIZE)),
            &paint,
        );
        canvas.translate((int_to_scalar(TILE_SIZE + TILE_GAP), 0.0));
    }
    canvas.restore();
    // Now adjust to start the next row below this one (1 gap for text and 2 gap for margin)
    canvas.translate((0.0, int_to_scalar(3 * TILE_GAP + TILE_SIZE)));
}

// Port of: gm/gradients_degenerate.cpp#L86-L90 (chrome/m156), make_linear
fn make_linear(mode: TileMode) -> Option<Shader> {
    // Same position
    let pts = [
        Point::new(CENTER.0, CENTER.1),
        Point::new(CENTER.0, CENTER.1),
    ];
    let g = grad(mode);
    gradient_shaders::linear_gradient((pts[0], pts[1]), &g, None)
}

// Port of: gm/gradients_degenerate.cpp#L92-L95 (chrome/m156), make_radial
fn make_radial(mode: TileMode) -> Option<Shader> {
    // Radius = 0
    gradient_shaders::radial_gradient((CENTER, 0.0), &grad(mode), None)
}

// Port of: gm/gradients_degenerate.cpp#L97-L101 (chrome/m156), make_sweep
fn make_sweep(mode: TileMode) -> Option<Shader> {
    // Start and end angles at 45
    const SWEEP_ANG: f32 = 45.0;
    gradient_shaders::sweep_gradient(CENTER, (SWEEP_ANG, SWEEP_ANG), &grad(mode), None)
}

// Port of: gm/gradients_degenerate.cpp#L103-L106 (chrome/m156), make_sweep_zero_ang
fn make_sweep_zero_ang(mode: TileMode) -> Option<Shader> {
    // Start and end angles at 0
    gradient_shaders::sweep_gradient(CENTER, (0.0, 0.0), &grad(mode), None)
}

// Port of: gm/gradients_degenerate.cpp#L108-L112 (chrome/m156), make_2pt_conic
fn make_2pt_conic(mode: TileMode) -> Option<Shader> {
    // Start and end radius = TILE_SIZE, same position
    let half = int_to_scalar(TILE_SIZE / 2);
    gradient_shaders::two_point_conical_gradient((CENTER, half), (CENTER, half), &grad(mode), None)
}

// Port of: gm/gradients_degenerate.cpp#L114-L117 (chrome/m156), make_2pt_conic_zero_rad
fn make_2pt_conic_zero_rad(mode: TileMode) -> Option<Shader> {
    // Start and end radius = 0, same position
    gradient_shaders::two_point_conical_gradient((CENTER, 0.0), (CENTER, 0.0), &grad(mode), None)
}

// Port of: gm/gradients_degenerate.cpp#L119-L146 (chrome/m156), DegenerateGradientGM
struct DegenerateGradientGm;

impl GM for DegenerateGradientGm {
    fn name(&self) -> String {
        "degenerate_gradients".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(800, 800)
    }

    // Port of: gm/gradients_degenerate.cpp#L125-L138 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((int_to_scalar(3 * TILE_GAP), int_to_scalar(3 * TILE_GAP)));
        draw_tile_header(canvas);
        draw_row(canvas, "linear: empty, blue, blue, green", make_linear);
        draw_row(canvas, "radial:  empty, blue, blue, green", make_radial);
        draw_row(
            canvas,
            "sweep-0: empty, blue, blue, green",
            make_sweep_zero_ang,
        );
        draw_row(
            canvas,
            "sweep-45: empty, blue, blue, red 45 degree sector then green",
            make_sweep,
        );
        draw_row(
            canvas,
            "2pt-conic-0: empty, blue, blue, green",
            make_2pt_conic_zero_rad,
        );
        draw_row(
            canvas,
            "2pt-conic-1: empty, blue, blue, full red circle on green",
            make_2pt_conic,
        );
    }
}

// Port of: gm/gradients_degenerate.cpp#L148 (chrome/m156)
crate::def_gm!(DegenerateGradientGM, DegenerateGradientGm);
