// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/getpostextpath.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::scalar::scalar;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_tools::font_tool_utils::{default_portable_font, get_text_path};

// Port of: gm/getpostextpath.cpp#L13-L20 (chrome/m156), strokePath
fn stroke_path(canvas: &Canvas, path: &Path) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::RED);
    paint.set_style(skia_rust_core::paint::Style::Stroke);
    canvas.draw_path(path, &paint);
}

// Port of: gm/getpostextpath.cpp#L22-L51 (chrome/m156), DEF_SIMPLE_GM(getpostextpath)
crate::def_simple_gm!(getpostextpath, canvas, 480, 780, {
    // explicitly add spaces, to test a prev. bug
    let text = b"Ham bur ge fons";
    let mut font = default_portable_font();
    font.set_size(48.0);

    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    canvas.translate((10.0, 64.0));
    canvas.draw_simple_text(text, TextEncoding::UTF8, (0.0, 0.0), &font, &paint);
    let path = get_text_path(&font, text, TextEncoding::UTF8, None);
    stroke_path(canvas, &path);

    let count = font.count_text(text, TextEncoding::UTF8);
    let mut glyphs = vec![0; count];
    font.text_to_glyphs(text, TextEncoding::UTF8, &mut glyphs);
    let mut pos = vec![Point::default(); count];
    let mut widths = vec![0.0; count];
    font.get_widths(&glyphs, &mut widths);

    let mut rand = Random::default();
    let mut x: scalar = 20.0;
    let y: scalar = 100.0;
    for i in 0..count {
        pos[i] = Point::new(x, y + rand.next_s_scalar1() * 24.0);
        x += widths[i];
    }

    canvas.translate((0.0, 64.0));
    if let Some(blob) = TextBlob::from_pos_text(text, TextEncoding::UTF8, &pos, &font) {
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
    }
    let path = get_text_path(&font, text, TextEncoding::UTF8, Some(&pos));
    stroke_path(canvas, &path);
});
