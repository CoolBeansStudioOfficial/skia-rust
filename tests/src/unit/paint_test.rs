// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PaintTest.cpp (chrome/m156)
//
// Not ported yet (each needs a type that is not ported):
// - `Paint_copy`: `SkMaskFilter::MakeBlur` (mask filters, Phase 3).
// - `Paint_flattening`, `Paint_MoreFlattening`: `SkBinaryWriteBuffer` / `SkReadBuffer`
//   (`SkPaintPriv::Flatten`, `SkReadBuffer::readPaint`).
// - `Paint_nothingToDraw`: `SkColorMatrix` and `SkColorFilters::Matrix` (color filters,
//   Phase 3).

#![cfg(test)]

use crate::{def_font_test, def_test, reporter_assert};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::font_types::FontHinting;
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::paint::{Join, Paint, Style};
use skia_rust_core::paint_priv;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_utils::fill_path_with_paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Contains;
use skia_rust_core::scalar::{SCALAR_1, int_to_scalar, scalar};
use skia_rust_tools::font_tool_utils::default_portable_font;

// found and fixed for webkit: mishandling when we hit recursion limit on
// mostly degenerate cubic flatness test
// Port of: tests/PaintTest.cpp#L69-L101 (chrome/m156)
def_test!(
    #[allow(clippy::excessive_precision, clippy::unreadable_literal)] // the C++ literals, verbatim
    Paint_regression_cubic,
    |reporter| {
        let mut builder = PathBuilder::new();
        let mut paint = Paint::default();

        builder.move_to((460.2881309415525, 303.250847066498));
        builder.cubic_to(
            (463.36378422175284, 302.1169735073363),
            (456.32239330810046, 304.720354932878),
            (453.15255460013304, 305.788586869862),
        );
        let path = builder.detach();

        let fill_r = *path.bounds();

        paint.set_style(Style::Stroke);
        paint.set_stroke_width(int_to_scalar(2));
        let _ = fill_path_with_paint(&path, &paint, &mut builder, None, None);
        let stroke_r = builder.compute_bounds();

        let mut max_r = fill_r;
        // std::max(SK_Scalar1, paint.getStrokeMiter())
        let miter = if SCALAR_1 < paint.stroke_miter() {
            paint.stroke_miter()
        } else {
            SCALAR_1
        };
        let inset = if paint.stroke_join() == Join::Miter {
            paint.stroke_width() * miter
        } else {
            paint.stroke_width()
        };
        max_r.inset((-inset, -inset));

        // test that our stroke didn't explode
        reporter_assert!(reporter, max_r.contains(&stroke_r));
    }
);

// Port of: tests/PaintTest.cpp#L246-L253 (chrome/m156)
def_test!(Paint_dither, |reporter| {
    let mut p = Paint::default();
    p.set_dither(true);

    let should_dither = paint_priv::should_dither(&p, ColorType::BGRA8888);

    reporter_assert!(reporter, !should_dither);
});

// Port of: tests/PaintTest.cpp#L149-L160 (chrome/m156)
def_font_test!(Paint_regression_measureText, |reporter| {
    let mut font = default_portable_font();
    font.set_size(12.0);

    // C++ resets an out-parameter rect to NaN first and checks that measureText overwrites it;
    // `measure_text` returns its bounds, so only the empty-text assertion is left.
    let (_width, r) = font.measure_text(b"", TextEncoding::UTF8, None);
    reporter_assert!(reporter, r.is_empty());
});

// Port of: tests/PaintTest.cpp#L208-L244 (chrome/m156)
def_font_test!(Font_getpos, |reporter| {
    let mut font = default_portable_font();
    let text = b"Hamburgefons!@#!#23425,./;'[]";
    let count = font.count_text(text, TextEncoding::UTF8);
    let mut glyphs: Vec<GlyphId> = vec![0; count];
    let _ = font.text_to_glyphs(text, TextEncoding::UTF8, &mut glyphs);

    let mut widths: Vec<scalar> = vec![0.0; count];
    let mut xpos: Vec<scalar> = vec![0.0; count];
    let mut pos: Vec<Point> = vec![Point::new(0.0, 0.0); count];

    for subpix in [false, true] {
        font.set_subpixel(subpix);
        for hint in [
            FontHinting::None,
            FontHinting::Slight,
            FontHinting::Normal,
            FontHinting::Full,
        ] {
            font.set_hinting(hint);
            for size in [1.0_f32, 12.0, 100.0] {
                font.set_size(size);

                font.get_widths_bounds(&glyphs, &mut widths, &mut [], None);
                font.get_x_pos(&glyphs, &mut xpos, 10.0);
                font.get_pos(&glyphs, &mut pos, Point::new(10.0, 20.0));

                let nearly_eq = |a: scalar, b: scalar| (a - b).abs() < 0.000_001_f32;

                let mut x: scalar = 10.0;
                for i in 0..count {
                    reporter_assert!(reporter, nearly_eq(x, xpos[i]));
                    reporter_assert!(reporter, nearly_eq(x, pos[i].x));
                    reporter_assert!(reporter, nearly_eq(20.0, pos[i].y));
                    x += widths[i];
                }
            }
        }
    }
});
