// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/TextBlobBench.cpp

//! The text blob benches: one run of "Keep your sentences short, but not overly so." in a
//! portable serif. `TextBlobCachedBench` and `TextBlobFirstTimeBench` draw the blob (rendering),
//! `TextBlobMakeBench` only builds it (non-rendering).

use skia_rust_core::font::Font;
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::GlyphId;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::scalar::scalar;
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_tools::font_tool_utils::create_portable_typeface;

use crate::def_bench;
use crate::prelude::*;

/// `class SkTextBlobBench`: the state the three benches share.
// Port of: bench/TextBlobBench.cpp#L25-L58 (chrome/m156)
struct SkTextBlobBench {
    builder: TextBlobBuilder,
    font: Font,
    glyphs: Vec<GlyphId>,
    x_pos: Vec<scalar>,
}

impl SkTextBlobBench {
    // Port of: bench/TextBlobBench.cpp#L28-L30 (chrome/m156)
    fn new() -> Self {
        Self {
            builder: TextBlobBuilder::new(),
            font: Font::default(),
            glyphs: Vec::new(),
            x_pos: Vec::new(),
        }
    }

    // Port of: bench/TextBlobBench.cpp#L32-L45 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        // fFont.setTypeface(ToolUtils::CreatePortableTypeface("serif", SkFontStyle()));
        self.font.set_typeface(Some(create_portable_typeface(
            Some("serif"),
            FontStyle::default(),
        )));
        self.font.set_subpixel(true);

        // This text seems representative in both length and letter frequency.
        let text = "Keep your sentences short, but not overly so.".as_bytes();

        self.glyphs = vec![0; self.font.count_text(text, TextEncoding::UTF8)];
        self.x_pos = vec![0.0; self.glyphs.len()];

        self.font
            .text_to_glyphs(text, TextEncoding::UTF8, &mut self.glyphs);
        self.font.get_x_pos(&self.glyphs, &mut self.x_pos, 0.0);
    }

    // Port of: bench/TextBlobBench.cpp#L47-L53 (chrome/m156)
    fn make_blob(&mut self) -> TextBlob {
        let (glyphs, pos) = self
            .builder
            .alloc_run_pos_h(&self.font, self.glyphs.len(), 10.0, None);
        glyphs.copy_from_slice(&self.glyphs);
        pos.copy_from_slice(&self.x_pos);
        self.builder
            .make()
            .expect("a run was allocated, so the blob is not empty")
    }
}

/// `class TextBlobCachedBench : public SkTextBlobBench`.
// Port of: bench/TextBlobBench.cpp#L60-L75 (chrome/m156)
struct TextBlobCachedBench(SkTextBlobBench);

impl Benchmark for TextBlobCachedBench {
    fn name(&self) -> String {
        "TextBlobCachedBench".to_owned()
    }

    fn on_delayed_setup(&mut self) {
        self.0.on_delayed_setup();
    }

    // Port of: bench/TextBlobBench.cpp#L62-L74 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("TextBlobCachedBench is a rendering bench");
        let paint = Paint::default();

        let blob = self.0.make_blob();
        let big_loops = loops * 100;
        for _ in 0..big_loops {
            // To ensure maximum caching, we just redraw the blob at the same place everytime
            canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        }
    }
}

/// `class TextBlobFirstTimeBench : public SkTextBlobBench`.
// Port of: bench/TextBlobBench.cpp#L78-L91 (chrome/m156)
struct TextBlobFirstTimeBench(SkTextBlobBench);

impl Benchmark for TextBlobFirstTimeBench {
    fn name(&self) -> String {
        "TextBlobFirstTimeBench".to_owned()
    }

    fn on_delayed_setup(&mut self) {
        self.0.on_delayed_setup();
    }

    // Port of: bench/TextBlobBench.cpp#L84-L90 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("TextBlobFirstTimeBench is a rendering bench");
        let paint = Paint::default();

        let big_loops = loops * 100;
        for _ in 0..big_loops {
            let blob = self.0.make_blob();
            canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
        }
    }
}

/// `class TextBlobMakeBench : public SkTextBlobBench`.
// Port of: bench/TextBlobBench.cpp#L94-L110 (chrome/m156)
struct TextBlobMakeBench(SkTextBlobBench);

impl Benchmark for TextBlobMakeBench {
    fn name(&self) -> String {
        "TextBlobMakeBench".to_owned()
    }

    // Port of: bench/TextBlobBench.cpp#L100-L102 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn on_delayed_setup(&mut self) {
        self.0.on_delayed_setup();
    }

    // Port of: bench/TextBlobBench.cpp#L104-L109 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            for _ in 0..1000 {
                // this->makeBlob();
                let _ = self.0.make_blob();
            }
        }
    }
}

// Port of: bench/TextBlobBench.cpp#L76 (chrome/m156)
def_bench!(
    text_blob_cached = "TextBlobCachedBench()",
    TextBlobCachedBench(SkTextBlobBench::new())
);
// Port of: bench/TextBlobBench.cpp#L92 (chrome/m156)
def_bench!(
    text_blob_first_time = "TextBlobFirstTimeBench()",
    TextBlobFirstTimeBench(SkTextBlobBench::new())
);
// Port of: bench/TextBlobBench.cpp#L111 (chrome/m156)
def_bench!(
    text_blob_make = "TextBlobMakeBench()",
    TextBlobMakeBench(SkTextBlobBench::new())
);
