// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/textblobblockreordering.cpp (chrome/m156)

// The GM mirrors C++ arithmetic: the glyph and position counters are small and are converted to
// scalars exactly as C++ does, and the scalar-to-int floors are SkScalarFloorToInt.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use crate::prelude::*;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_tools::font_tool_utils::{add_to_text_blob, default_portable_typeface};

const WIDTH: i32 = 275;
const HEIGHT: i32 = 200;

// Port of: gm/textblobblockreordering.cpp#L15-L70 (chrome/m156), TextBlobBlockReordering
struct TextBlobBlockReorderingGm {
    blob: Option<TextBlob>,
}

impl GM for TextBlobBlockReorderingGm {
    // Port of: gm/textblobblockreordering.cpp#L17-L31 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let mut builder = TextBlobBuilder::new();
        // make textblob
        // Large text is used to trigger atlas eviction
        let mut font = Font::from_size(default_portable_typeface(), 56.0);
        font.set_edging(Edging::Alias);
        let text = "AB";
        let (_, bounds) = font.measure_text(text.as_bytes(), TextEncoding::UTF8, None);
        let y_offset = bounds.height();
        add_to_text_blob(&mut builder, text, &font, 0.0, y_offset - 30.0);
        // build
        self.blob = builder.make();
    }

    // Port of: gm/textblobblockreordering.cpp#L33 (chrome/m156), getName
    fn name(&self) -> String {
        "textblobblockreordering".to_owned()
    }

    // Port of: gm/textblobblockreordering.cpp#L34 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    // Port of: gm/textblobblockreordering.cpp#L40-L63 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let Some(blob) = self.blob.clone() else {
            return;
        };
        canvas.draw_color(Color::GRAY, None);
        let paint = Paint::default();
        canvas.translate((10.0, 40.0));
        let bounds = *blob.bounds();
        let y_delta = bounds.height().floor() as i32 + 20;
        let x_delta = bounds.width().floor() as i32;
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        canvas.translate((x_delta as f32, y_delta as f32));
        // Draw a rect where the text should be, and then twiddle the xfermode so we don't combine.
        let mut red_paint = Paint::default();
        red_paint.set_color(Color::RED);
        canvas.draw_rect(bounds, &red_paint);
        let mut src_in_paint = paint.clone();
        src_in_paint.set_blend_mode(BlendMode::SrcIn);
        canvas.draw_text_blob(&blob, (0.0, 0.0), &src_in_paint);
        canvas.translate((x_delta as f32, y_delta as f32));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
    }
}

// Port of: gm/textblobblockreordering.cpp#L66 (chrome/m156), DEF_GM
crate::def_gm!(
    TextBlobBlockReordering,
    TextBlobBlockReorderingGm { blob: None }
);
