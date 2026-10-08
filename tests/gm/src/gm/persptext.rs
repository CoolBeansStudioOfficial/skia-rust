// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/persptext.cpp (chrome/m156)
//
// `TEST_PERSP_CHECK` is a debugging switch in the C++ and is off, so it is not ported.

// The loop counter and step count are small, and C++ converts them to float the same way.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use skia_rust_core::font::Font;
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::scalar::scalar;
use skia_rust_tools::font_tool_utils::create_portable_typeface;

// Port of: gm/persptext.cpp#L19-L22 (chrome/m156), PerspMode
#[derive(Clone, Copy)]
enum PerspMode {
    X,
    Y,
    XY,
}

const STEPS: i32 = 8;

// Port of: gm/persptext.cpp#L24-L116 (chrome/m156), PerspTextGM
struct PerspTextGm {
    minimal: bool,
}

impl GM for PerspTextGm {
    // Port of: gm/persptext.cpp#L26-L30 (chrome/m156), getName
    fn name(&self) -> String {
        if self.minimal {
            "persptext_minimal".to_owned()
        } else {
            "persptext".to_owned()
        }
    }

    // Port of: gm/persptext.cpp#L31 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(1024, 768)
    }

    // Port of: gm/persptext.cpp#L25 (chrome/m156), the constructor's setBGColor
    fn bg_color(&self) -> Color {
        Color::WHITE
    }

    // Port of: gm/persptext.cpp#L34-L101 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.clear(Color::WHITE);
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        let mut font = Font::from_typeface(Some(create_portable_typeface(
            Some("serif"),
            FontStyle::default(),
        )));
        font.set_subpixel(true);
        font.set_size(32.0);
        font.set_baseline_snap(false);
        let text = "Hamburgefons";
        let text_width = font
            .measure_text(text.as_bytes(), TextEncoding::UTF8, None)
            .0;
        let text_height = font.metrics().0;
        let mut x: scalar = 10.0;
        let mut y: scalar = text_height + 5.0;
        let minimal_factor: f32 = if self.minimal { 32.0 } else { 1.0 };
        for pm in [PerspMode::X, PerspMode::Y, PerspMode::XY] {
            for i in 0..STEPS {
                canvas.save();
                let fi = i as f32;
                let steps = STEPS as f32;
                let mut persp = Matrix::default();
                match pm {
                    PerspMode::X => {
                        if self.minimal {
                            persp.set_persp_x(fi * 0.0005 / steps / minimal_factor);
                        } else {
                            persp.set_persp_x(fi * 0.00025 / steps);
                        }
                    }
                    PerspMode::Y => {
                        persp.set_persp_y(fi * 0.0025 / steps / minimal_factor);
                    }
                    PerspMode::XY => {
                        persp.set_persp_x(fi * -0.00025 / steps / minimal_factor);
                        persp.set_persp_y(fi * -0.00125 / steps / minimal_factor);
                    }
                }
                persp = Matrix::concat(&persp, &Matrix::translate((-x, -y)));
                persp = Matrix::concat(&Matrix::translate((x, y)), &persp);
                canvas.concat(&persp);
                paint.set_color(Color::BLACK);
                canvas.draw_str(text, (x, y), &font, &paint);
                y += text_height + 5.0;
                canvas.restore();
            }
            x += text_width + 10.0;
            y = text_height + 5.0;
        }
    }
}

// Port of: gm/persptext.cpp#L118 (chrome/m156), DEF_GM(return new PerspTextGM(true);)
crate::def_gm!(PerspTextGM_minimal, PerspTextGm { minimal: true });

// Port of: gm/persptext.cpp#L119 (chrome/m156), DEF_GM(return new PerspTextGM(false);)
crate::def_gm!(PerspTextGM_full, PerspTextGm { minimal: false });
