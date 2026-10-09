// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/srcmode.cpp (chrome/m156)

// The int-to-scalar casts of small counts mirror the C++ arithmetic (exact in f32).
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::colors;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/srcmode.cpp#L8-L9 (chrome/m156), #define W / H
const W: f32 = 80.0;
const H: f32 = 60.0;

// Port of: gm/srcmode.cpp#L12-L14 (chrome/m156), identity_paintproc
fn identity_paintproc(paint: &mut Paint) {
    paint.set_shader(None);
}

// Port of: gm/srcmode.cpp#L16-L20 (chrome/m156), gradient_paintproc
fn gradient_paintproc(paint: &mut Paint) {
    let shader_colors = [colors::GREEN, colors::BLUE];
    let pts = [Point::new(0.0, 0.0), Point::new(W, H)];
    let grad = Gradient::new(
        Colors::new(&shader_colors, None, TileMode::Clamp, None),
        Interpolation::default(),
    );
    paint.set_shader(shaders::linear_gradient((pts[0], pts[1]), &grad, None));
}

type PaintProc = fn(&mut Paint);
type Proc = fn(&Canvas, &Paint, &Font);

// Port of: gm/srcmode.cpp#L26-L30 (chrome/m156), draw_hair
fn draw_hair(canvas: &Canvas, paint: &Paint, _font: &Font) {
    let mut p = paint.clone();
    p.set_stroke_width(0.0);
    canvas.draw_line((0.0, 0.0), (W, H), &p);
}

// Port of: gm/srcmode.cpp#L32-L36 (chrome/m156), draw_thick
fn draw_thick(canvas: &Canvas, paint: &Paint, _font: &Font) {
    let mut p = paint.clone();
    p.set_stroke_width(H / 5.0);
    canvas.draw_line((0.0, 0.0), (W, H), &p);
}

// Port of: gm/srcmode.cpp#L38-L40 (chrome/m156), draw_rect
fn draw_rect(canvas: &Canvas, paint: &Paint, _font: &Font) {
    canvas.draw_rect(Rect::from_wh(W, H), paint);
}

// Port of: gm/srcmode.cpp#L42-L44 (chrome/m156), draw_oval
fn draw_oval(canvas: &Canvas, paint: &Paint, _font: &Font) {
    canvas.draw_oval(Rect::from_wh(W, H), paint);
}

// Port of: gm/srcmode.cpp#L46-L48 (chrome/m156), draw_text
fn draw_text(canvas: &Canvas, paint: &Paint, font: &Font) {
    canvas.draw_str("Hamburge", (0.0, H * 2.0 / 3.0), font, paint);
}

// Port of: gm/srcmode.cpp (chrome/m156), class SrcModeGM
#[derive(Debug, Default)]
pub struct SrcModeGm;

impl SrcModeGm {
    // Port of: gm/srcmode.cpp (chrome/m156), SrcModeGM::drawContent
    fn draw_content(canvas: &Canvas) {
        canvas.translate((20.0, 20.0));

        let mut paint = Paint::default();
        let mut font = Font::from_size(default_portable_typeface(), H / 4.0);
        paint.set_color(Color::new(0x80F6_0000));

        let procs: [Proc; 5] = [draw_hair, draw_thick, draw_rect, draw_oval, draw_text];

        let modes = [BlendMode::SrcOver, BlendMode::Src, BlendMode::Clear];

        let paint_procs: [PaintProc; 2] = [identity_paintproc, gradient_paintproc];

        for aa in 0..=1 {
            paint.set_anti_alias(aa != 0);
            font.set_edging(if aa != 0 {
                Edging::AntiAlias
            } else {
                Edging::Alias
            });
            canvas.save();
            for paint_proc in paint_procs {
                paint_proc(&mut paint);
                for mode in modes {
                    paint.set_blend_mode(mode);
                    canvas.save();
                    for proc in procs {
                        proc(canvas, &paint, &font);
                        canvas.translate((0.0, H * 5.0 / 4.0));
                    }
                    canvas.restore();
                    canvas.translate((W * 5.0 / 4.0, 0.0));
                }
            }
            canvas.restore();
            canvas.translate((0.0, (H * 5.0 / 4.0) * procs.len() as f32));
        }
    }
}

impl GM for SrcModeGm {
    fn name(&self) -> String {
        "srcmode".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 760)
    }

    fn bg_color(&self) -> Color {
        Color::BLACK
    }

    // Port of: gm/srcmode.cpp (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        // `compat_surface`: a raster surface of the same info (the picture canvas case is not
        // reachable here, and a raster canvas's makeSurface is a raster surface too).
        let mut surf = surfaces::raster_n32_premul((640, 760)).expect("a raster surface");
        surf.canvas().draw_color(Color::WHITE, None);
        Self::draw_content(surf.canvas());
        surf.draw(canvas, (0.0, 0.0), SamplingOptions::default(), None);
    }
}

// Port of: gm/srcmode.cpp (chrome/m156), DEF_GM(return new SrcModeGM;)
crate::def_gm!(SrcModeGM, SrcModeGm);
