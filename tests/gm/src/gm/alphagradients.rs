// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/alphagradients.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::color::colors;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::interpolation::InPremul;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// Port of: gm/alphagradients.cpp#L21-L112 (chrome/m156)
struct AlphaGradientsGm;

impl AlphaGradientsGm {
    fn new() -> Self {
        AlphaGradientsGm
    }

    // Port of: gm/alphagradients.cpp#L31-L47 (chrome/m156)
    fn draw_grad(canvas: &Canvas, r: &Rect, c0: Color4f, c1: Color4f, do_pre_mul: bool) {
        let colors = [c0, c1];
        let pts = [
            Point::new(r.left(), r.top()),
            Point::new(r.right(), r.bottom()),
        ];
        let mut paint = Paint::default();
        let pm = if do_pre_mul {
            InPremul::Yes
        } else {
            InPremul::No
        };
        paint.set_shader(shaders::linear_gradient(
            (pts[0], pts[1]),
            &Gradient::new(
                Colors::new(&colors, None, TileMode::Clamp, None),
                Interpolation {
                    in_premul: pm,
                    ..Interpolation::default()
                },
            ),
            None,
        ));
        canvas.draw_rect(r, &paint);

        paint.set_shader(None);
        paint.set_style(Style::Stroke);
        canvas.draw_rect(r, &paint);
    }
}

impl GM for AlphaGradientsGm {
    fn name(&self) -> String {
        "alphagradients".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    // Port of: gm/alphagradients.cpp#L49-L91 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        struct ColorPair {
            color0: Color4f,
            color1: Color4f,
        }
        let pair = |color0, color1| ColorPair { color0, color1 };
        let g_rec = [
            pair(colors::WHITE, Color4f::new(0.0, 0.0, 0.0, 0.0)),
            pair(colors::WHITE, Color4f::new(1.0, 0.0, 0.0, 0.0)),
            pair(colors::WHITE, Color4f::new(1.0, 1.0, 0.0, 0.0)),
            pair(colors::WHITE, Color4f::new(1.0, 1.0, 1.0, 0.0)),
            pair(colors::RED, Color4f::new(0.0, 0.0, 0.0, 0.0)),
            pair(colors::RED, Color4f::new(1.0, 0.0, 0.0, 0.0)),
            pair(colors::RED, Color4f::new(1.0, 1.0, 0.0, 0.0)),
            pair(colors::RED, Color4f::new(1.0, 1.0, 1.0, 0.0)),
            pair(colors::BLUE, Color4f::new(0.0, 0.0, 0.0, 0.0)),
            pair(colors::BLUE, Color4f::new(1.0, 0.0, 0.0, 0.0)),
            pair(colors::BLUE, Color4f::new(1.0, 1.0, 0.0, 0.0)),
            pair(colors::BLUE, Color4f::new(1.0, 1.0, 1.0, 0.0)),
        ];

        let r = Rect::from_wh(300.0, 30.0);

        canvas.translate((10.0, 10.0));

        for do_pre_mul in 0..=1 {
            canvas.save();
            for rec in &g_rec {
                Self::draw_grad(canvas, &r, rec.color0, rec.color1, do_pre_mul != 0);
                canvas.translate((0.0, r.height() + 8.0));
            }
            canvas.restore();
            canvas.translate((r.width() + 10.0, 0.0));
        }
    }
}

// Port of: gm/alphagradients.cpp#L93 (chrome/m156)
crate::def_gm!(AlphaGradientsGM, AlphaGradientsGm::new());
