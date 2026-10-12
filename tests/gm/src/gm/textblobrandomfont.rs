// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/textblobrandomfont.cpp (chrome/m156)
//
// The blob is built with `SkRandomTypeface`, which draws each glyph in a mask format picked from
// its glyph id. `onDraw` draws only with a GPU or Graphite context (`isGPU`), so the raster sink
// returns `kSkip` before anything is drawn, as the C++ does. The GPU body is not ported: it can
// never run here.

use crate::prelude::*;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_tools::font_tool_utils::{
    add_to_text_blob, create_portable_typeface, emoji_sample_default,
};
use skia_rust_tools::fonts::random_scaler_context::RandomTypeface;

const WIDTH: i32 = 2000;
const HEIGHT: i32 = 1600;

// This gm tests that textblobs can be translated and scaled with a font that returns random
// but deterministic masks
// Port of: gm/textblobrandomfont.cpp#L39-L180 (chrome/m156), class TextBlobRandomFont
struct TextBlobRandomFontGm {
    blob: Option<TextBlob>,
}

impl TextBlobRandomFontGm {
    // Port of: gm/textblobrandomfont.cpp#L43 (chrome/m156), TextBlobRandomFont()
    fn new() -> Self {
        Self { blob: None }
    }
}

impl GM for TextBlobRandomFontGm {
    // Port of: gm/textblobrandomfont.cpp#L102 (chrome/m156), getName
    fn name(&self) -> String {
        "textblobrandomfont".to_owned()
    }

    // Port of: gm/textblobrandomfont.cpp#L104 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(WIDTH, HEIGHT)
    }

    // Port of: gm/textblobrandomfont.cpp#L46-L100 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let mut builder = TextBlobBuilder::new();

        let text = "The quick brown fox jumps over the lazy dog.";

        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(Color::MAGENTA);

        // make textbloben
        let mut font = Font::default();
        font.set_size(32.0);
        font.set_edging(Edging::SubpixelAntiAlias);

        // Setup our random scaler context
        let typeface = create_portable_typeface(Some("sans-serif"), FontStyle::bold());
        font.set_typeface(Some(RandomTypeface::make(typeface, paint.clone(), false)));

        let mut y = 0.0;
        let (_, mut bounds) = font.measure_text(text.as_bytes(), TextEncoding::UTF8, None);
        y -= bounds.top;
        add_to_text_blob(&mut builder, text, &font, 0.0, y);
        y += bounds.bottom;

        // A8
        let bigtext1 = "The quick brown fox";
        let bigtext2 = "jumps over the lazy dog.";
        font.set_size(160.0);
        font.set_subpixel(false);
        font.set_edging(Edging::AntiAlias);
        bounds = font
            .measure_text(bigtext1.as_bytes(), TextEncoding::UTF8, None)
            .1;
        y -= bounds.top;
        add_to_text_blob(&mut builder, bigtext1, &font, 0.0, y);
        y += bounds.bottom;

        bounds = font
            .measure_text(bigtext2.as_bytes(), TextEncoding::UTF8, None)
            .1;
        y -= bounds.top;
        add_to_text_blob(&mut builder, bigtext2, &font, 0.0, y);
        y += bounds.bottom;

        // color emoji
        let sample = emoji_sample_default();
        if let Some(typeface) = sample.typeface {
            font.set_typeface(Some(RandomTypeface::make(typeface, paint.clone(), false)));
            bounds = font
                .measure_text(sample.sample_text.as_bytes(), TextEncoding::UTF8, None)
                .1;
            y -= bounds.top;
            add_to_text_blob(&mut builder, sample.sample_text, &font, 0.0, y);
            // The C++ then adds `bounds.fBottom` to `y`, which nothing reads after the last line.
        }

        // build
        self.blob = builder.make();
    }

    // Port of: gm/textblobrandomfont.cpp#L106-L120 (chrome/m156), onDraw: the GPU-only skip. The
    // raster sink has no GPU or Graphite context, so `isGPU` is false and nothing is drawn.
    fn on_draw_with_error(&mut self, _canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        error_msg.clear();
        error_msg.push_str(crate::ERROR_MSG_DRAW_SKIPPED_GPU_ONLY);
        DrawResult::Skip
    }
}

// Port of: gm/textblobrandomfont.cpp#L184 (chrome/m156), DEF_GM
crate::def_gm!(TextBlobRandomFont, TextBlobRandomFontGm::new());
