// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/GlyphRunRSXformBench.cpp

//! `GlyphRunRSXformCachedBench`: one cached blob of glyphs placed by random `RSXform`s, drawn
//! again and again (rendering).

use skia_rust_core::font::Font;
use skia_rust_core::font_types::GlyphId;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::random::Random;
use skia_rust_core::rsxform::RSXform;
use skia_rust_core::scalar::{SCALAR_PI, scalar, scalar_cos, scalar_sin};
use skia_rust_core::text_blob::TextBlob;
use skia_rust_tools::font_tool_utils::default_font;

use crate::def_bench;
use crate::prelude::*;

/// `makeBlob()`: creates an RSX form blob from randomized transforms.
// Port of: bench/GlyphRunRSXformBench.cpp#L20-L49 (chrome/m156)
fn make_blob() -> TextBlob {
    // font = ToolUtils::DefaultFont();
    let mut font: Font = default_font();
    font.set_subpixel(true);
    font.set_size(20.0);

    let text = "Keep your sentences short, but not overly so.".as_bytes();
    let glyph_count = font.count_text(text, TextEncoding::UTF8);
    let mut glyphs: Vec<GlyphId> = vec![0; glyph_count];
    font.text_to_glyphs(text, TextEncoding::UTF8, &mut glyphs);

    let mut x_forms = vec![RSXform::default(); glyph_count];
    // SkRandom rand;
    let mut rand = Random::default();
    let mut x: scalar = 0.0;
    for x_form in &mut x_forms {
        // SkScalar s = rand.nextF() * 0.5f + 0.5f;
        let s = rand.next_f() * 0.5 + 0.5;
        // SkScalar a = rand.nextF() * SK_ScalarPI * 0.25f;
        let a = rand.next_f() * SCALAR_PI * 0.25;
        // xForms[i] = SkRSXform::Make(s * SkScalarCos(a), s * SkScalarSin(a), x, rand.nextF() * 20);
        *x_form = RSXform::new(
            s * scalar_cos(a),
            s * scalar_sin(a),
            (x, rand.next_f() * 20.0),
        );
        x += 20.0;
    }

    // SkTextBlob::MakeFromRSXform(glyphs.data(), glyphs.size() * sizeof(SkGlyphID), ...)
    let glyph_bytes: Vec<u8> = glyphs.iter().flat_map(|g| g.to_ne_bytes()).collect();
    TextBlob::from_rsxform(&glyph_bytes, TextEncoding::GlyphId, &x_forms, &font)
        .expect("one transform per glyph")
}

/// `class GlyphRunRSXformCachedBench`.
// Port of: bench/GlyphRunRSXformBench.cpp#L51-L69 (chrome/m156)
struct GlyphRunRSXformCachedBench {
    blob: Option<TextBlob>,
}

impl Benchmark for GlyphRunRSXformCachedBench {
    // Port of: bench/GlyphRunRSXformBench.cpp#L53 (chrome/m156)
    fn name(&self) -> String {
        "GlyphRunRSXform_cached".to_owned()
    }

    // Port of: bench/GlyphRunRSXformBench.cpp#L55 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        self.blob = Some(make_blob());
    }

    // Port of: bench/GlyphRunRSXformBench.cpp#L57-L64 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("GlyphRunRSXform_cached is a rendering bench");
        let paint = Paint::default();
        let blob = self.blob.as_ref().expect("on_delayed_setup makes the blob");
        for _ in 0..loops {
            canvas.draw_text_blob(blob, (0.0, 0.0), &paint);
        }
    }
}

// Port of: bench/GlyphRunRSXformBench.cpp#L71 (chrome/m156)
def_bench!(
    glyph_run_rsxform_cached = "GlyphRunRSXformCachedBench()",
    GlyphRunRSXformCachedBench { blob: None }
);
