// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/glyph_pos.cpp (chrome/m156)

//! This test tries to define the effect of using hairline strokes on text.
//! Provides non-hairline images for reference and consistency checks.
//! `glyph_pos_(h/n)_(s/f/b)`: test hairline/non-hairline stroke/fill/stroke+fill.

use crate::prelude::*;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::matrix::{Matrix, TypeMask};
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::scalar::{scalar, scalar_invert};
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/glyph_pos.cpp#L27 (chrome/m156)
const TEXT_HEIGHT: scalar = 14.0;
// Port of: gm/glyph_pos.cpp#L28 (chrome/m156)
const TEXT: &[u8] = b"Proportional Hamburgefons #% fi";

// Port of: gm/glyph_pos.cpp#L88-L148 (chrome/m156), drawTestCase
fn draw_test_case(canvas: &Canvas, text_scale: scalar, stroke_width: scalar, stroke_style: Style) {
    let mut paint = Paint::default();
    paint.set_color(Color::BLACK);
    paint.set_anti_alias(true);
    paint.set_stroke_width(stroke_width);
    paint.set_style(stroke_style);

    let font = Font::from_size(default_portable_typeface(), TEXT_HEIGHT * text_scale);

    // This demonstrates that we can not measure the text if
    // there's a device transform. The canvas total matrix will
    // end up being a device transform.
    let draw_ref = (canvas.local_to_device_as_3x3().get_type() & !TypeMask::TRANSLATE).is_empty();
    if draw_ref {
        let (advance, bounds) = font.measure_text(TEXT, TextEncoding::UTF8, Some(&paint));
        paint.set_stroke_width(0.0);
        paint.set_style(Style::Stroke);
        // Green box is the measured text bounds.
        paint.set_color(Color::GREEN);
        canvas.draw_rect(bounds, &paint);
        // Red line is the measured advance from the 0,0 of the text position.
        paint.set_color(Color::RED);
        canvas.draw_line((0.0, 0.0), (advance, 0.0), &paint);
    }

    // Black text is the testcase, eg. the text.
    paint.set_color(Color::BLACK);
    paint.set_stroke_width(stroke_width);
    paint.set_style(stroke_style);
    canvas.draw_simple_text(TEXT, TextEncoding::UTF8, (0.0, 0.0), &font, &paint);

    if draw_ref {
        let len = TEXT.len();
        let mut glyphs = vec![0; len];
        let count = font.text_to_glyphs(TEXT, TextEncoding::UTF8, &mut glyphs);
        glyphs.truncate(count);
        let mut widths = vec![0.0; count];
        font.get_widths_bounds(&glyphs, &mut widths, &mut [], Some(&paint));
        paint.set_stroke_width(0.0);
        paint.set_style(Style::Stroke);
        // Magenta lines are the positions for the characters.
        paint.set_color(Color::MAGENTA);
        let mut w: scalar = 0.0;
        for width in widths.iter().take(TEXT.len()) {
            canvas.draw_line((w, 0.0), (w, 5.0), &paint);
            w += width;
        }
    }
}

// Port of: gm/glyph_pos.cpp#L35-L86 (chrome/m156), draw_gm
fn draw_gm(canvas: &Canvas, stroke_width: scalar, stroke_style: Style) {
    // There's a black pixel at 40, 40 for reference.
    canvas.draw_point((40.0, 40.0), &Paint::default());

    // Two reference images.
    canvas.translate((50.0, 50.0));
    draw_test_case(canvas, 1.0, stroke_width, stroke_style);
    canvas.translate((0.0, 50.0));
    draw_test_case(canvas, 3.0, stroke_width, stroke_style);

    // Uniform scaling test.
    canvas.translate((0.0, 100.0));
    canvas.save();
    canvas.scale((3.0, 3.0));
    draw_test_case(canvas, 1.0, stroke_width, stroke_style);
    canvas.restore();

    // Non-uniform scaling test.
    canvas.translate((0.0, 100.0));
    canvas.save();
    canvas.scale((3.0, 6.0));
    draw_test_case(canvas, 1.0, stroke_width, stroke_style);
    canvas.restore();

    // Skew test.
    canvas.translate((0.0, 80.0));
    canvas.save();
    canvas.scale((3.0, 3.0));
    let mut skew = Matrix::default();
    skew.set_skew_x(8.0 / 25.0);
    skew.set_skew_y(2.0 / 25.0);
    canvas.concat(&skew);
    draw_test_case(canvas, 1.0, stroke_width, stroke_style);
    canvas.restore();

    // Perspective test.
    canvas.translate((0.0, 80.0));
    canvas.save();
    let mut perspective = Matrix::default();
    perspective.set_persp_x(-scalar_invert(340.0));
    perspective.set_skew_x(8.0 / 25.0);
    perspective.set_skew_y(2.0 / 25.0);
    canvas.concat(&perspective);
    draw_test_case(canvas, 1.0, stroke_width, stroke_style);
    canvas.restore();
}

// Port of: gm/glyph_pos.cpp#L150-L152 (chrome/m156)
crate::def_simple_gm!(glyph_pos_h_b, c, 800, 600, {
    draw_gm(c, 0.0, Style::StrokeAndFill);
});
// Port of: gm/glyph_pos.cpp#L153-L155 (chrome/m156)
crate::def_simple_gm!(glyph_pos_n_b, c, 800, 600, {
    draw_gm(c, 1.2, Style::StrokeAndFill);
});
// Port of: gm/glyph_pos.cpp#L156-L158 (chrome/m156)
crate::def_simple_gm!(glyph_pos_h_s, c, 800, 600, {
    draw_gm(c, 0.0, Style::Stroke);
});
// Port of: gm/glyph_pos.cpp#L159-L161 (chrome/m156)
crate::def_simple_gm!(glyph_pos_n_s, c, 800, 600, {
    draw_gm(c, 1.2, Style::Stroke);
});
// Port of: gm/glyph_pos.cpp#L162-L164 (chrome/m156)
crate::def_simple_gm!(glyph_pos_h_f, c, 800, 600, {
    draw_gm(c, 0.0, Style::Fill);
});
// Port of: gm/glyph_pos.cpp#L165-L167 (chrome/m156)
crate::def_simple_gm!(glyph_pos_n_f, c, 800, 600, {
    draw_gm(c, 1.2, Style::Fill);
});
