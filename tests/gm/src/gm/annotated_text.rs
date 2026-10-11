// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/annotated_text.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::data::Data;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/annotated_text.cpp#L19-L31 (chrome/m156), draw_url_annotated_text_with_box
fn draw_url_annotated_text_with_box(
    canvas: &Canvas,
    text: &str,
    x: f32,
    y: f32,
    font: &Font,
    url: &std::ffi::CStr,
) {
    let (_, bounds) = font.measure_text(text.as_bytes(), TextEncoding::UTF8, None);
    let bounds = bounds.with_offset((x, y));
    let url_data = Data::new_with_cstring(Some(url));
    canvas.draw_url_annotation(bounds, &url_data);
    let mut shade = Paint::default();
    shade.set_color(Color::new(0x8034_6180));
    canvas.draw_rect(bounds, &shade);
    canvas.draw_simple_text(
        text.as_bytes(),
        TextEncoding::UTF8,
        (x, y),
        font,
        &Paint::default(),
    );
}

// Port of: gm/annotated_text.cpp#L38-L52 (chrome/m156), annotated_text
crate::def_simple_gm!(annotated_text, canvas, 512, 512, {
    canvas.save();
    canvas.clear(Color::WHITE);
    canvas.clip_rect(Rect::from_xywh(64.0, 64.0, 256.0, 256.0), None, None);
    canvas.clear(Color::from(0xFFEE_EEEE));

    let mut font = default_portable_font();
    font.set_edging(Edging::Alias);
    font.set_size(40.0);
    let text = "Click this link!";
    let url = c"https://www.google.com/";
    draw_url_annotated_text_with_box(canvas, text, 200.0, 80.0, &font, url);
    canvas.save_layer(&SaveLayerRec::default());
    canvas.rotate(90.0, None);
    draw_url_annotated_text_with_box(canvas, text, 150.0, -55.0, &font, url);
    canvas.restore();
    canvas.restore();
});
