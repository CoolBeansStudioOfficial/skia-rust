// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/gammatext.cpp (chrome/m156)
//
// Test a set of clipping problems discovered while writing blitAntiRect, and test all the code
// paths through the clipping blitters. Each region should show as a blue center surrounded by a
// 2px green border, with no red.

use crate::prelude::*;
use skia_rust_core::color::colors;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::int_to_scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_tools::font_tool_utils::{create_portable_typeface, default_portable_font};

const HEIGHT: i32 = 480;

// Port of: gm/gammatext.cpp#L19-L22 (chrome/m156), make_heatGradient
fn make_heat_gradient(pts: &[Point; 2]) -> Option<Shader> {
    let bw = [colors::BLACK, colors::WHITE];
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&bw, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/gammatext.cpp#L51-L60 (chrome/m156), fg
const FG: [Color; 8] = [
    Color::new(0xFFFF_FFFF),
    Color::new(0xFFFF_FF00),
    Color::new(0xFFFF_00FF),
    Color::new(0xFF00_FFFF),
    Color::new(0xFFFF_0000),
    Color::new(0xFF00_FF00),
    Color::new(0xFF00_00FF),
    Color::new(0xFF00_0000),
];

// Port of: gm/gammatext.cpp#L30-L49 (chrome/m156), GammaTextGM
struct GammaTextGm;

impl GammaTextGm {
    // Port of: gm/gammatext.cpp#L35-L41 (chrome/m156), drawGrad
    fn draw_grad(canvas: &Canvas) {
        let pts = [Point::new(0.0, 0.0), Point::new(0.0, int_to_scalar(HEIGHT))];
        canvas.clear(Color::RED);
        let mut paint = Paint::default();
        paint.set_shader(make_heat_gradient(&pts));
        let r = Rect::from_ltrb(0.0, 0.0, 1024.0, int_to_scalar(HEIGHT));
        canvas.draw_rect(r, &paint);
    }
}

impl GM for GammaTextGm {
    // Port of: gm/gammatext.cpp#L32 (chrome/m156), getName
    fn name(&self) -> String {
        "gammatext".to_owned()
    }

    // Port of: gm/gammatext.cpp#L33 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(1024, HEIGHT)
    }

    // Port of: gm/gammatext.cpp#L42-L70 (chrome/m156), onDraw
    #[allow(clippy::cast_precision_loss)] // mirrors the SkIntToScalar(1024) / std::size(fg) division
    fn on_draw(&mut self, canvas: &Canvas) {
        Self::draw_grad(canvas);
        let text = "Hamburgefons";
        let mut paint = Paint::default();
        let mut font = default_portable_font();
        font.set_size(16.0);
        font.set_edging(Edging::SubpixelAntiAlias);
        let mut x = 10.0_f32;
        for &fg in &FG {
            paint.set_color(fg);
            let mut y = 40.0_f32;
            let stopy = int_to_scalar(HEIGHT);
            while y < stopy {
                canvas.draw_str(text, (x, y), &font, &paint);
                y += font.size() * 2.0;
            }
            x += 1024.0 / FG.len() as f32;
        }
    }
}

// Port of: gm/gammatext.cpp#L72 (chrome/m156), GammaTextGM registration
crate::def_gm!(GammaTextGM, GammaTextGm);

// Port of: gm/gammatext.cpp#L76-L81 (chrome/m156), make_gradient
fn make_gradient(c: Color) -> Option<Shader> {
    let pts = [Point::new(0.0, 0.0), Point::new(240.0, 0.0)];
    let colors = [
        Color4f::from(c),
        Color4f::from(Color::from_argb(0, c.r(), c.g(), c.b())),
    ];
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/gammatext.cpp#L83-L95 (chrome/m156), draw_pair
fn draw_pair(canvas: &Canvas, font: &Font, color: Color, shader: Option<&Shader>) {
    const TEXT: &str = "Now is the time for all good";
    let mut paint = Paint::default();
    paint.set_color(color);
    canvas.draw_str(TEXT, (10.0, 20.0), font, &paint);
    paint.set_shader(shaders::color(paint.color()));
    canvas.draw_str(TEXT, (10.0, 40.0), font, &paint);
    paint.set_shader(shader.cloned());
    canvas.draw_str(TEXT, (10.0, 60.0), font, &paint);
}

// Port of: gm/gammatext.cpp#L97-L125 (chrome/m156), GammaShaderTextGM
struct GammaShaderTextGm {
    shaders: [Option<Shader>; 3],
    colors: [Color; 3],
}

impl GammaShaderTextGm {
    // Port of: gm/gammatext.cpp#L104-L110 (chrome/m156), the constructor
    fn new() -> Self {
        Self {
            shaders: [None, None, None],
            colors: [Color::BLACK, Color::RED, Color::BLUE],
        }
    }
}

impl GM for GammaShaderTextGm {
    // Port of: gm/gammatext.cpp#L113 (chrome/m156), getName
    fn name(&self) -> String {
        "gammagradienttext".to_owned()
    }

    // Port of: gm/gammatext.cpp#L114 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(300, 300)
    }

    // Port of: gm/gammatext.cpp#L115-L119 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        for (shader, &color) in self.shaders.iter_mut().zip(self.colors.iter()) {
            *shader = make_gradient(color);
        }
    }

    // Port of: gm/gammatext.cpp#L120-L129 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        let tf = create_portable_typeface(Some("serif"), FontStyle::italic());
        let mut font = Font::from_size(tf, 18.0);
        font.set_edging(Edging::SubpixelAntiAlias);
        for i in 0..self.shaders.len() {
            draw_pair(canvas, &font, self.colors[i], self.shaders[i].as_ref());
            canvas.translate((0.0, 80.0));
        }
    }
}

// Port of: gm/gammatext.cpp#L131 (chrome/m156), GammaShaderTextGM registration
crate::def_gm!(GammaShaderTextGM, GammaShaderTextGm::new());

// Port of: gm/gammatext.cpp#L133-L150 (chrome/m156), gammatext_color_shader
crate::def_simple_gm_bg!(gammatext_color_shader, canvas, 300, 275, Color::GRAY, {
    let text = "ABCDEFG";
    let tf = create_portable_typeface(Some("serif"), FontStyle::default());
    let mut font = Font::from_size(tf, 18.0);
    font.set_edging(Edging::SubpixelAntiAlias);
    canvas.translate((10.0, 30.0));
    for i in (0_u8..=255).step_by(20) {
        let color = Color::from_argb(0xFF, i, i, i);
        let mut paint = Paint::default();
        paint.set_color(color);
        canvas.draw_str(text, (0.0, 0.0), &font, &paint);
        paint.set_shader(shaders::color(color));
        canvas.draw_str(text, (100.0, 0.0), &font, &paint);
        paint.set_shader(shaders::color_in_space(
            Color4f::from(color),
            ColorSpace::new_srgb(),
        ));
        canvas.draw_str(text, (200.0, 0.0), &font, &paint);
        canvas.translate((0.0, 20.0));
    }
});
