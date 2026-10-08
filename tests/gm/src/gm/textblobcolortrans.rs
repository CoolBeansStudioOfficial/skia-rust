// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/textblobcolortrans.cpp (chrome/m156)

// The GM mirrors C++ arithmetic: the glyph and position counters are small and are converted to
// scalars exactly as C++ does, and the scalar-to-int floors are SkScalarFloorToInt.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use crate::prelude::*;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_tools::font_tool_utils::{add_to_text_blob, default_portable_typeface};

const WIDTH: i32 = 675;
const HEIGHT: i32 = 1600;

// Port of: gm/textblobcolortrans.cpp#L15-L58 (chrome/m156), TextBlobColorTrans
struct TextBlobColorTransGm {
    blob: Option<TextBlob>,
}

impl GM for TextBlobColorTransGm {
    // Port of: gm/textblobcolortrans.cpp#L17-L34 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let mut builder = TextBlobBuilder::new();
        let mut font = Font::from_size(default_portable_typeface(), 256.0);
        font.set_edging(Edging::Alias);
        let mut text = "AB";
        let (_, bounds) = font.measure_text(text.as_bytes(), TextEncoding::UTF8, None);
        let y_offset = bounds.height();
        add_to_text_blob(&mut builder, text, &font, 0.0, y_offset - 30.0);
        font.set_size(28.0);
        text = "The quick brown fox jumps over the lazy dog.";
        let _ = font.measure_text(text.as_bytes(), TextEncoding::UTF8, None);
        add_to_text_blob(&mut builder, text, &font, 0.0, y_offset - 8.0);
        self.blob = builder.make();
    }

    // Port of: gm/textblobcolortrans.cpp#L36 (chrome/m156), getName
    fn name(&self) -> String {
        "textblobcolortrans".to_owned()
    }

    // Port of: gm/textblobcolortrans.cpp#L37 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    // Port of: gm/textblobcolortrans.cpp#L38-L56 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let Some(blob) = self.blob.clone() else {
            return;
        };
        canvas.draw_color(Color::GRAY, None);
        let mut paint = Paint::default();
        canvas.translate((10.0, 40.0));
        let bounds = *blob.bounds();
        let colors = [
            Color::from_argb(0xFF, 0x00, 0xFF, 0xFF), // SK_ColorCYAN
            Color::from_argb(0xFF, 0xCC, 0xCC, 0xCC), // SK_ColorLTGRAY
            Color::from_argb(0xFF, 0xFF, 0xFF, 0x00), // SK_ColorYELLOW
            Color::WHITE,
        ];
        let mut color_index = 0;
        let step = bounds.height().floor() as i32;
        let mut y = 0;
        while y + step < HEIGHT {
            paint.set_color(colors[color_index % colors.len()]);
            color_index += 1;
            canvas.save();
            canvas.translate((0.0, y as f32));
            canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
            canvas.restore();
            y += step;
        }
    }
}

// Port of: gm/textblobcolortrans.cpp#L60 (chrome/m156), DEF_GM
crate::def_gm!(TextBlobColorTrans, TextBlobColorTransGm { blob: None });
