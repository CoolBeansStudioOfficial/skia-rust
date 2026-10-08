// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/fontscaler.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::{FontHinting, TextEncoding};
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/fontscaler.cpp#L23-L78 (chrome/m156), FontScalerGM
struct FontScalerGm;

impl GM for FontScalerGm {
    // Port of: gm/fontscaler.cpp#L28-L30 (chrome/m156), the constructor's background
    fn bg_color(&self) -> Color {
        Color::WHITE
    }

    fn name(&self) -> String {
        "fontscaler".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1450, 750)
    }

    // Port of: gm/fontscaler.cpp#L32-L77 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut font: Font = default_portable_font();
        font.set_edging(skia_rust_core::font::Edging::SubpixelAntiAlias);
        // With freetype the default (normal hinting) can be really ugly.
        // Most distros now set slight (vertical hinting only) in any event.
        font.set_hinting(FontHinting::Slight);

        let text = "Hamburgefons ooo mmm";
        let text_bytes = text.as_bytes();
        for _ in 0..2 {
            // This used to do 6 iterations but it causes the N4 to crash in the MSAA4 config.
            for i in 0..5 {
                let x: scalar = 10.0;
                let mut y: scalar = 20.0;
                canvas.save();
                // `i` and `j` are small, so the scalar conversions are exact.
                #[allow(clippy::cast_precision_loss)]
                canvas.translate((50.0 + (i * 230) as scalar, 20.0));
                #[allow(clippy::cast_precision_loss)]
                canvas.rotate((i * 5) as scalar, Some(Point::new(x, y * 10.0)));
                {
                    let mut p = Paint::default();
                    p.set_anti_alias(true);
                    let r = Rect::from_ltrb(x - 3.0, 15.0, x - 1.0, 280.0);
                    canvas.draw_rect(r, &p);
                }
                for ps in 6..=22 {
                    // Sizes 6 to 22 are exact in a scalar.
                    #[allow(clippy::cast_precision_loss)]
                    font.set_size(ps as scalar);
                    canvas.draw_simple_text(
                        text_bytes,
                        TextEncoding::UTF8,
                        (x, y),
                        &font,
                        &Paint::default(),
                    );
                    y += font.metrics().0;
                }
                canvas.restore();
            }
            canvas.translate((0.0, 360.0));
            font.set_subpixel(true);
            font.set_linear_metrics(true);
            font.set_baseline_snap(false);
        }
    }
}

// Port of: gm/fontscaler.cpp#L82 (chrome/m156)
crate::def_gm!(FontScalerGM, FontScalerGm);
