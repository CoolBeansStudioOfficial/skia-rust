// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/gradtext.cpp (chrome/m156)

use crate::prelude::*;

use skia_rust_core::color::colors;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_tools::font_tool_utils::{default_portable_font, default_portable_typeface};

// test shader w/ transparency
// Port of: gm/gradtext.cpp#L10-L15 (chrome/m156), make_grad
fn make_grad(width: f32) -> Option<Shader> {
    let colors = [colors::RED, Color4f::new(0.0, 1.0, 0.0, 0.0), colors::BLUE];
    let pts = [Point::new(0.0, 0.0), Point::new(width, 0.0)];
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Mirror, None),
            Interpolation::default(),
        ),
        None,
    )
}

// test opaque shader
// Port of: gm/gradtext.cpp#L18-L23 (chrome/m156), make_grad2
fn make_grad2(width: f32) -> Option<Shader> {
    let colors = [colors::RED, colors::GREEN, colors::BLUE];
    let pts = [Point::new(0.0, 0.0), Point::new(width, 0.0)];
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Mirror, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/gradtext.cpp#L25-L30 (chrome/m156), make_chrome_solid
fn make_chrome_solid() -> Option<Shader> {
    let colors = [colors::GREEN, colors::GREEN];
    let pts = [Point::new(0.0, 0.0), Point::new(1.0, 0.0)];
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Replicate chrome layout test - clipped pathed gradient-shaded text
// Port of: gm/gradtext.cpp#L33-L47 (chrome/m156), ChromeGradTextGM1
struct ChromeGradTextGm1;

impl GM for ChromeGradTextGm1 {
    // Port of: gm/gradtext.cpp#L34 (chrome/m156), getName
    fn name(&self) -> String {
        "chrome_gradtext1".to_owned()
    }

    // Port of: gm/gradtext.cpp#L35 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(500, 480)
    }

    // Port of: gm/gradtext.cpp#L36-L46 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        let r = Rect::from_xywh(0.0, 0.0, 100.0, 100.0);
        canvas.clip_rect(r, None, None);
        paint.set_color(Color::RED);
        canvas.draw_rect(r, &paint);
        // Minimal repro doesn't require AA, LCD, or a nondefault typeface
        paint.set_shader(make_chrome_solid());
        let mut font = Font::from_size(default_portable_typeface(), 500.0);
        font.set_edging(Edging::Alias);
        canvas.draw_str("I", (0.0, 100.0), &font, &paint);
    }
}

// Replicate chrome layout test - switching between solid & gradient text
// Port of: gm/gradtext.cpp#L49-L66 (chrome/m156), ChromeGradTextGM2
struct ChromeGradTextGm2;

impl GM for ChromeGradTextGm2 {
    // Port of: gm/gradtext.cpp#L50 (chrome/m156), getName
    fn name(&self) -> String {
        "chrome_gradtext2".to_owned()
    }

    // Port of: gm/gradtext.cpp#L51 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(500, 480)
    }

    // Port of: gm/gradtext.cpp#L52-L65 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        let mut font = default_portable_font();
        font.set_edging(Edging::Alias);
        paint.set_style(Style::Fill);
        canvas.draw_str("Normal Fill Text", (0.0, 50.0), &font, &paint);
        paint.set_style(Style::Stroke);
        canvas.draw_str("Normal Stroke Text", (0.0, 100.0), &font, &paint);
        // Minimal repro doesn't require AA, LCD, or a nondefault typeface
        paint.set_shader(make_chrome_solid());
        paint.set_style(Style::Fill);
        canvas.draw_str("Gradient Fill Text", (0.0, 150.0), &font, &paint);
        paint.set_style(Style::Stroke);
        canvas.draw_str("Gradient Stroke Text", (0.0, 200.0), &font, &paint);
    }
}

// Port of: gm/gradtext.cpp#L68-L70 (chrome/m156), ChromeGradTextGM1 registration
crate::def_gm!(ChromeGradTextGM1, ChromeGradTextGm1);
// Port of: gm/gradtext.cpp#L68-L70 (chrome/m156), ChromeGradTextGM2 registration
crate::def_gm!(ChromeGradTextGM2, ChromeGradTextGm2);

// Port of: gm/gradtext.cpp#L72-L104 (chrome/m156), gradtext
crate::def_simple_gm!(gradtext, canvas, 500, 480, {
    const TEXT_SIZE: f32 = 26.0;
    let mut font = Font::from_size(default_portable_typeface(), TEXT_SIZE);
    canvas.draw_rect(Rect::from_ltrb(0.0, 0.0, 500.0, 240.0), &Paint::default());
    canvas.translate((20.0, TEXT_SIZE));
    let mut grad_paint = Paint::default();
    grad_paint.set_shader(make_grad(80.0));
    let mut opaque_paint = Paint::default();
    opaque_paint.set_shader(make_grad2(80.0));
    let paints = [grad_paint, opaque_paint];
    let edgings = [Edging::Alias, Edging::AntiAlias, Edging::SubpixelAntiAlias];
    for _ in 0..2 {
        for paint in &paints {
            for &edging in &edgings {
                font.set_edging(edging);
                canvas.draw_str(
                    "When in the course of human events",
                    (0.0, 0.0),
                    &font,
                    paint,
                );
                canvas.translate((0.0, TEXT_SIZE * 4.0 / 3.0));
            }
            canvas.translate((0.0, TEXT_SIZE * 2.0 / 3.0));
        }
    }
});
