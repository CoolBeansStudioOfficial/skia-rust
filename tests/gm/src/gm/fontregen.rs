// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/fontregen.cpp (chrome/m156)
//
// GM to stress TextBlob regeneration and the glyph cache. The GPU-only steps (the context options
// and `flushAndSubmit`) have no raster counterpart and are not ported.

use crate::prelude::*;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::paint::Paint;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_tools::font_tool_utils::{create_portable_typeface, default_portable_font};

// Port of: gm/fontregen.cpp#L52-L59 (chrome/m156), make_blob
fn make_blob(text: &str, font: &Font) -> Option<TextBlob> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut glyphs = vec![GlyphId::default(); len];
    font.text_to_glyphs(bytes, TextEncoding::UTF8, &mut glyphs);
    let mut pos = vec![0.0_f32; len];
    font.get_x_pos(&glyphs, &mut pos, 0.0);
    TextBlob::from_pos_text_h(bytes, TextEncoding::UTF8, &pos, 0.0, font)
}

// Port of: gm/fontregen.cpp#L61-L110 (chrome/m156), FontRegenGM
struct FontRegenGm {
    blobs: [Option<TextBlob>; 3],
}

impl FontRegenGm {
    // Port of: gm/fontregen.cpp#L61 (chrome/m156), the constructor
    fn new() -> Self {
        Self {
            blobs: [None, None, None],
        }
    }
}

impl GM for FontRegenGm {
    // Port of: gm/fontregen.cpp#L80 (chrome/m156), getName
    fn name(&self) -> String {
        "fontregen".to_owned()
    }

    // Port of: gm/fontregen.cpp#L81 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(512, 512)
    }

    // Port of: gm/fontregen.cpp#L82 (chrome/m156), the background colour (SK_ColorLTGRAY)
    fn bg_color(&self) -> Color {
        Color::new(0xFFCC_CCCC)
    }

    // Port of: gm/fontregen.cpp#L84-L97 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let tf = create_portable_typeface(Some("sans-serif"), FontStyle::normal());
        let texts = ["abcdefghijklmnopqrstuvwxyz", "ABCDEFGHI", "NOPQRSTUV"];
        let mut font = Font::from_typeface(Some(tf));
        font.set_edging(Edging::AntiAlias);
        font.set_subpixel(false);
        font.set_size(80.0);
        self.blobs[0] = make_blob(texts[0], &font);
        font.set_size(162.0);
        self.blobs[1] = make_blob(texts[1], &font);
        self.blobs[2] = make_blob(texts[2], &font);
    }

    // Port of: gm/fontregen.cpp#L98-L121 (chrome/m156), onDraw
    fn on_draw_with_error(&mut self, canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        let mut paint = Paint::default();
        paint.set_color(Color::BLACK);
        let blob0 = self.blobs[0]
            .as_ref()
            .expect("onOnceBeforeDraw made the blob");
        let blob1 = self.blobs[1]
            .as_ref()
            .expect("onOnceBeforeDraw made the blob");
        canvas.draw_text_blob(blob0, (10.0, 80.0), &paint);
        canvas.draw_text_blob(blob1, (10.0, 225.0), &paint);
        // The GPU flush (`flushAndSubmit`) is not needed on a raster sink.
        paint.set_color(Color::new(0xFF01_0101));
        canvas.draw_text_blob(blob0, (10.0, 305.0), &paint);
        let blob2 = self.blobs[2]
            .as_ref()
            .expect("onOnceBeforeDraw made the blob");
        canvas.draw_text_blob(blob2, (10.0, 465.0), &paint);
        DrawResult::Ok
    }
}

// Port of: gm/fontregen.cpp#L122 (chrome/m156), FontRegenGM registration
crate::def_gm!(FontRegenGM_paren = "FontRegenGM()", FontRegenGm::new());

// Port of: gm/fontregen.cpp#L123-L144 (chrome/m156), BadAppleGM
struct BadAppleGm {
    blobs: [Option<TextBlob>; 2],
}

impl BadAppleGm {
    // Port of: gm/fontregen.cpp#L124 (chrome/m156), the constructor
    fn new() -> Self {
        Self {
            blobs: [None, None],
        }
    }
}

impl GM for BadAppleGm {
    // Port of: gm/fontregen.cpp#L126 (chrome/m156), getName
    fn name(&self) -> String {
        "badapple".to_owned()
    }

    // Port of: gm/fontregen.cpp#L127 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(512, 512)
    }

    // Port of: gm/fontregen.cpp#L129-L146 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        let mut font = default_portable_font();
        font.set_edging(Edging::SubpixelAntiAlias);
        font.set_subpixel(true);
        font.set_size(256.0);
        self.blobs[0] = make_blob("Meet", &font);
        self.blobs[1] = make_blob("iPad Pro", &font);
    }

    // Port of: gm/fontregen.cpp#L148-L154 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_color(Color::new(0xFF11_1111));
        let blob0 = self.blobs[0]
            .as_ref()
            .expect("onOnceBeforeDraw made the blob");
        let blob1 = self.blobs[1]
            .as_ref()
            .expect("onOnceBeforeDraw made the blob");
        canvas.draw_text_blob(blob0, (10.0, 260.0), &paint);
        canvas.draw_text_blob(blob1, (10.0, 500.0), &paint);
    }
}

// Port of: gm/fontregen.cpp#L155 (chrome/m156), BadAppleGM registration
crate::def_gm!(BadAppleGM_paren = "BadAppleGM()", BadAppleGm::new());
