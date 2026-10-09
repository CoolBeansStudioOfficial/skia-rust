// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/lcdoverlap.cpp (chrome/m156)

// The float/int conversions mirror the C++ arithmetic of the GM.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::paint::Paint;
use skia_rust_core::scalar::scalar;
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_tools::font_tool_utils::{add_to_text_blob, default_portable_typeface};

// Port of: gm/lcdoverlap.cpp#L15-L16 (chrome/m156)
const WIDTH: i32 = 750;
const HEIGHT: i32 = 750;

// Port of: gm/lcdoverlap.cpp (chrome/m156), class LcdOverlapGM
#[derive(Debug, Default)]
pub struct LcdOverlapGm {
    blob: Option<TextBlob>,
}

impl LcdOverlapGm {
    // Port of: gm/lcdoverlap.cpp (chrome/m156), drawTestCase
    fn draw_test_case(
        &self,
        canvas: &Canvas,
        x: scalar,
        y: scalar,
        mode: BlendMode,
        mode2: BlendMode,
    ) {
        let colors = [
            Color::RED,
            Color::GREEN,
            Color::BLUE,
            Color::YELLOW,
            Color::CYAN,
            Color::MAGENTA,
        ];
        let blob = self
            .blob
            .as_ref()
            .expect("the text blob is built before drawing");

        for (i, color) in colors.iter().enumerate() {
            canvas.save();
            canvas.translate((x, y));
            canvas.rotate(360.0_f32 / colors.len() as f32 * i as f32, None);
            canvas.translate((
                -blob.bounds().width() / 2.0 - blob.bounds().left() + 0.5,
                0.0,
            ));

            let mut text_paint = Paint::default();
            text_paint.set_color(*color);
            text_paint.set_blend_mode(if i % 2 == 0 { mode } else { mode2 });
            canvas.draw_text_blob(blob, (0.0, 0.0), &text_paint);
            canvas.restore();
        }
    }
}

impl GM for LcdOverlapGm {
    fn name(&self) -> String {
        "lcdoverlap".to_owned()
    }

    // Port of: gm/lcdoverlap.cpp (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        // build text blob
        let mut builder = TextBlobBuilder::new();

        let mut font = Font::from_size(default_portable_typeface(), 32.0);
        let text = "able was I ere I saw elba";
        font.set_subpixel(true);
        font.set_edging(Edging::SubpixelAntiAlias);
        add_to_text_blob(&mut builder, text, &font, 0.0, 0.0);
        self.blob = builder.make();
    }

    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    // Port of: gm/lcdoverlap.cpp (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let offset_x = WIDTH as f32 / 4.0;
        let offset_y = HEIGHT as f32 / 4.0;
        self.draw_test_case(canvas, offset_x, offset_y, BlendMode::Src, BlendMode::Src);
        self.draw_test_case(
            canvas,
            3.0 * offset_x,
            offset_y,
            BlendMode::SrcOver,
            BlendMode::SrcOver,
        );
        self.draw_test_case(
            canvas,
            offset_x,
            3.0 * offset_y,
            BlendMode::HardLight,
            BlendMode::Luminosity,
        );
        self.draw_test_case(
            canvas,
            3.0 * offset_x,
            3.0 * offset_y,
            BlendMode::SrcOver,
            BlendMode::Src,
        );
    }
}

// Port of: gm/lcdoverlap.cpp (chrome/m156), DEF_GM(return new LcdOverlapGM;)
crate::def_gm!(LcdOverlapGM, LcdOverlapGm::default());
