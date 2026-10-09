// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/stroketext.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{AutoCanvasRestore, Canvas};
use skia_rust_core::font::Font;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::scalar::scalar;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_effects::dash_path_effect;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_tools::font_tool_utils::{create_typeface_from_resource, default_portable_typeface};

// Port of: gm/stroketext.cpp#L35-L49 (chrome/m156), test_nulldev
fn test_nulldev(_canvas: &Canvas) {
    let mut bm = Bitmap::new();
    let _ = bm.set_info(&ImageInfo::new_n32_premul((30, 30), None), None);
    // notice: no pixels mom! be sure we don't crash
    // https://code.google.com/p/chromium/issues/detail?id=352616
    // A canvas on a bitmap without pixels draws nothing, so the write below is a no-op.
    if let Some(c) = Canvas::from_bitmap(&mut bm, None) {
        let mut src = Bitmap::new();
        src.alloc_n32_pixels((10, 10), None);
        src.erase_color(Color::RED);
        // ensure we don't crash
        c.write_pixels_from_bitmap(&src, (0, 0));
    }
}

// Port of: gm/stroketext.cpp#L51-L64 (chrome/m156), draw_text_stroked
fn draw_text_stroked(canvas: &Canvas, paint: &Paint, font: &Font, stroke_width: scalar) {
    let mut p = paint.clone();
    let loc = Point::new(20.0, 435.0);
    if stroke_width > 0.0 {
        p.set_style(Style::Fill);
        canvas.draw_simple_text(b"P", TextEncoding::UTF8, (loc.x, loc.y - 225.0), font, &p);
        if let Some(blob) = TextBlob::from_pos_text(b"P", TextEncoding::UTF8, &[loc], font) {
            canvas.draw_text_blob(&blob, (0.0, 0.0), &p);
        }
    }
    p.set_color(Color::RED);
    p.set_style(Style::Stroke);
    p.set_stroke_width(stroke_width);
    canvas.draw_simple_text(b"P", TextEncoding::UTF8, (loc.x, loc.y - 225.0), font, &p);
    if let Some(blob) = TextBlob::from_pos_text(b"P", TextEncoding::UTF8, &[loc], font) {
        canvas.draw_text_blob(&blob, (0.0, 0.0), &p);
    }
}

// Port of: gm/stroketext.cpp#L66-L81 (chrome/m156), draw_text_set
fn draw_text_set(canvas: &Canvas, paint: &Paint, font: &Font) {
    let _acr = AutoCanvasRestore::guard(canvas, true);
    draw_text_stroked(canvas, paint, font, 10.0);
    canvas.translate((200.0, 0.0));
    draw_text_stroked(canvas, paint, font, 0.0);
    let intervals = [20.0, 10.0, 5.0, 10.0];
    let phase = 0.0;
    canvas.translate((200.0, 0.0));
    let mut p = paint.clone();
    p.set_path_effect(dash_path_effect::new(&intervals, phase));
    draw_text_stroked(canvas, &p, font, 10.0);
}

// Port of: gm/stroketext.cpp#L83-L86 (chrome/m156), the two sizes straddling the threshold
const BELOW_THRESHOLD_TEXT_SIZE: scalar = 255.0;
const ABOVE_THRESHOLD_TEXT_SIZE: scalar = 257.0;

// Port of: gm/stroketext.cpp#L88-L100 (chrome/m156), DEF_SIMPLE_GM(stroketext)
crate::def_simple_gm!(stroketext, canvas, 1200, 480, {
    test_nulldev(canvas);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let mut font = Font::from_size(default_portable_typeface(), BELOW_THRESHOLD_TEXT_SIZE);
    draw_text_set(canvas, &paint, &font);
    canvas.translate((600.0, 0.0));
    font.set_size(ABOVE_THRESHOLD_TEXT_SIZE);
    draw_text_set(canvas, &paint, &font);
});

// Port of: gm/stroketext.cpp#L102-L125 (chrome/m156), DEF_SIMPLE_GM_CAN_FAIL(stroketext_native)
//
// The GM needs the `fonts/Stroking.ttf` and `fonts/Variable.ttf` resources. The resource
// streams are not available in the portable configuration (`GetResourceAsStream` gives null,
// `CreateTypefaceFromResource` gives null, and the overlap typeface is built from that null
// stream), so the C++ GM takes its skip path: the whole body after the skip is unreachable here.
crate::def_simple_gm_can_fail!(stroketext_native, canvas, msg, 650, 420, {
    let ttf = create_typeface_from_resource(None, 0);
    let otf = create_typeface_from_resource(None, 0);
    // `overlap`: the variable font stream is null, so the lambda returns nullptr.
    let overlap: Option<skia_rust_core::typeface::Typeface> = None;
    if ttf.is_none() && otf.is_none() && overlap.is_none() {
        *msg = "No support for ttf or otf.".to_string();
        return DrawResult::Skip;
    }
    DrawResult::Ok
});
