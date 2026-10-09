// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/skbug_5321.cpp (chrome/m156)

use skia_rust_core::font::Edging;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::scalar::scalar;
use skia_rust_core::text_blob::TextBlobBuilder;
use skia_rust_tools::font_tool_utils::default_portable_font;

// https://bugs.skia.org/5321
// two strings should draw the same.  PDF did not.
// Port of: gm/skbug_5321.cpp#L14-L36 (chrome/m156), DEF_SIMPLE_GM(skbug_5321, canvas, 128, 128)
crate::def_simple_gm!(skbug_5321, canvas, 128, 128, {
    let mut font = default_portable_font();
    font.set_edging(Edging::Alias);
    font.set_size(30.0);
    // utf8(u"x̀y")
    let text: &[u8] = "x\u{300}y".as_bytes();
    let mut x: scalar = 20.0;
    let mut y: scalar = 45.0;
    canvas.draw_simple_text(text, TextEncoding::UTF8, (x, y), &font, &Paint::default());
    y += font.metrics().0;
    let glyph_count = font.count_text(text, TextEncoding::UTF8);
    let mut builder = TextBlobBuilder::new();
    let (glyphs, pos) = builder.alloc_run_pos_h(&font, glyph_count, y, None);
    font.text_to_glyphs(text, TextEncoding::UTF8, glyphs);
    font.get_widths(glyphs, pos);
    for w in pos.iter_mut() {
        let width = *w;
        *w = x;
        x += width;
    }
    if let Some(blob) = builder.make() {
        canvas.draw_text_blob(&blob, (0.0, 0.0), &Paint::default());
    }
});
