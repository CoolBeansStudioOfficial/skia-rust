// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/skbug_257.cpp (chrome/m156)

// The float literals and int-to-scalar casts mirror the C++ arithmetic of the GM.
#![allow(clippy::cast_precision_loss, clippy::unreadable_literal)]
// Single-letter and similar names mirror the C++ GM (w, h, x, y; rect, rrect).
#![allow(clippy::many_single_char_names, clippy::similar_names)]

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::PointMode;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::rrect::RRect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::scalar;
use skia_rust_core::text_blob::TextBlobBuilder;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/skbug_257.cpp#L8-L24 (chrome/m156), rotated_checkerboard_shader
fn rotated_checkerboard_shader(paint: &mut Paint, c1: Color, c2: Color, size: i32) {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((2 * size, 2 * size), None);
    bm.erase_color(c1);
    bm.erase_area(IRect::from_ltrb(0, 0, size, size), c2);
    bm.erase_area(IRect::from_ltrb(size, size, 2 * size, 2 * size), c2);
    let mut matrix = Matrix::scale((0.75, 0.75));
    matrix.pre_rotate(30.0, None);
    paint.set_shader(bm.to_shader(
        (TileMode::Repeat, TileMode::Repeat),
        SamplingOptions::default(),
        &matrix,
    ));
}

// Port of: gm/skbug_257.cpp#L26-L36 (chrome/m156), exercise_draw_pos_text
fn exercise_draw_pos_text(
    canvas: &Canvas,
    text: &str,
    x: scalar,
    y: scalar,
    font: &Font,
    paint: &Paint,
) {
    let count = font.count_text(text.as_bytes(), TextEncoding::UTF8);
    let mut builder = TextBlobBuilder::new();
    let (glyphs, points) = builder.alloc_run_pos(font, count, None);
    font.text_to_glyphs(text.as_bytes(), TextEncoding::UTF8, glyphs);
    font.get_pos(glyphs, points, Point::new(x, y));
    let blob = builder.make().expect("a text blob");
    canvas.draw_text_blob(&blob, (0.0, 0.0), paint);
}

// Port of: gm/skbug_257.cpp#L38-L48 (chrome/m156), exercise_draw_pos_text_h
fn exercise_draw_pos_text_h(
    canvas: &Canvas,
    text: &str,
    x: scalar,
    y: scalar,
    font: &Font,
    paint: &Paint,
) {
    let count = font.count_text(text.as_bytes(), TextEncoding::UTF8);
    let mut builder = TextBlobBuilder::new();
    let (glyphs, pos): (&mut [GlyphId], &mut [scalar]) =
        builder.alloc_run_pos_h(font, count, 0.0, None);
    font.text_to_glyphs(text.as_bytes(), TextEncoding::UTF8, glyphs);
    font.get_x_pos(glyphs, pos, 0.0);
    let blob = builder.make().expect("a text blob");
    canvas.draw_text_blob(&blob, (x, y), paint);
}

// Port of: gm/skbug_257.cpp#L50-L63 (chrome/m156), test_text
fn test_text(canvas: &Canvas, size: scalar, color: Color, y_offset: scalar) {
    let mut font = Font::from_size(default_portable_typeface(), 24.0);
    font.set_edging(Edging::Alias);
    let mut type_paint = Paint::default();
    type_paint.set_color(color);
    let text = "HELLO WORLD";
    canvas.draw_simple_text(
        text,
        TextEncoding::UTF8,
        (32.0, size / 2.0 + y_offset),
        &font,
        &type_paint,
    );
    let line_spacing = font.metrics().0;
    exercise_draw_pos_text(
        canvas,
        text,
        32.0,
        size / 2.0 + y_offset + line_spacing,
        &font,
        &type_paint,
    );
    exercise_draw_pos_text_h(
        canvas,
        text,
        32.0,
        size / 2.0 + y_offset + 2.0 * line_spacing,
        &font,
        &type_paint,
    );
}

// If this GM works correctly, the cyan layer should be lined up with
// the objects below it.
// Port of: gm/skbug_257.cpp#L65-L127 (chrome/m156), DEF_SIMPLE_GM(skbug_257)
crate::def_simple_gm!(skbug_257, canvas, 512, 512, {
    let size: scalar = 256.0;
    canvas.save();
    let scale: scalar = 1.00168;
    canvas.scale((scale, scale));
    {
        let mut checker = Paint::default();
        rotated_checkerboard_shader(&mut checker, Color::WHITE, Color::BLACK, 16);
        checker.set_anti_alias(true);

        canvas.save();
        canvas.clear(Color::new(0xFFCE_CFCE));
        let translate: scalar = 225364.0;
        canvas.translate((0.0, -translate));

        // Test rects
        let rect = Rect::from_ltrb(8.0, 8.0 + translate, size - 8.0, size - 8.0 + translate);
        canvas.draw_rect(rect, &checker);

        // Test Paths
        canvas.translate((size, 0.0));
        let mut rrect = RRect::new();
        let radii: [Vector; 4] = [
            Vector::new(40.0, 40.0),
            Vector::new(40.0, 40.0),
            Vector::new(40.0, 40.0),
            Vector::new(40.0, 40.0),
        ];
        rrect.set_rect_radii(rect, &radii);
        canvas.draw_rrect(rrect, &checker);

        // Test Points
        canvas.translate((-size, size));
        let delta: scalar = 1.0 / 64.0;
        let points = [
            Point::new(size / 2.0, 8.0 + translate),
            Point::new(size / 2.0, 8.0 + translate + delta),
            Point::new(8.0, size / 2.0 + translate),
            Point::new(8.0, size / 2.0 + translate + delta),
            Point::new(size / 2.0, size - 8.0 + translate),
            Point::new(size / 2.0, size - 8.0 + translate + delta),
            Point::new(size - 8.0, size / 2.0 + translate),
            Point::new(size - 8.0, size / 2.0 + translate + delta),
        ];
        checker.set_style(Style::Stroke);
        checker.set_stroke_width(8.0);
        checker.set_stroke_cap(Cap::Round);
        canvas.draw_points(PointMode::Lines, &points, &checker);

        // Test Text
        canvas.translate((size, 0.0));
        test_text(canvas, size, Color::BLACK, translate);
        canvas.restore();
    }
    // reference points (without the huge translations).
    let mut stroke = Paint::default();
    stroke.set_style(Style::Stroke);
    stroke.set_stroke_width(5.0);
    stroke.set_color(Color::CYAN);
    canvas.draw_circle((size / 2.0, size / 2.0), size / 2.0 - 10.0, &stroke);
    canvas.draw_circle((3.0 * size / 2.0, size / 2.0), size / 2.0 - 10.0, &stroke);
    canvas.draw_circle((size / 2.0, 384.0), size / 2.0 - 10.0, &stroke);
    canvas.translate((size, size));
    test_text(canvas, size, Color::CYAN, 0.0);
    canvas.restore();
});
