// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PaintTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{def_font_test, def_test, reporter_assert};
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::flattenable::FlattenableRegistry;
use skia_rust_core::font_types::FontHinting;
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::paint_priv;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_utils::fill_path_with_paint;
use skia_rust_core::point::Point;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::rect::Contains;
use skia_rust_core::scalar::{SCALAR_1, int_to_scalar, scalar};
use skia_rust_core::write_buffer::BinaryWriteBuffer;
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

// Port of: tests/PaintTest.cpp#L44-L67 (chrome/m156)
def_test!(Paint_copy, |reporter| {
    let mut paint = Paint::default();
    // set a few member variables
    paint.set_style(Style::StrokeAndFill);
    paint.set_stroke_width(int_to_scalar(2));
    // set a few pointers (`MakeBlur` with the default `respectCTM` of true)
    paint.set_mask_filter(MaskFilter::blur(
        BlurStyle::Normal,
        BlurMask::convert_radius_to_sigma(1.0),
        true,
    ));

    // copy the paint using the copy constructor and check they are the same
    let mut copied_paint = paint.clone();
    reporter_assert!(reporter, paint == copied_paint);

    // copy the paint using the equal operator and check they are the same
    copied_paint.clone_from(&paint);
    reporter_assert!(reporter, paint == copied_paint);

    // clean the paint and check they are back to their initial states
    let clean_paint = Paint::default();
    paint.reset();
    copied_paint.reset();
    reporter_assert!(reporter, clean_paint == paint);
    reporter_assert!(reporter, clean_paint == copied_paint);
});

// Port of: tests/PaintTest.cpp#L103-L146 (chrome/m156)
def_test!(Paint_flattening, |reporter| {
    let caps = [Cap::Butt, Cap::Round, Cap::Square];
    let joins = [Join::Miter, Join::Round, Join::Bevel];
    let styles = [Style::Fill, Style::Stroke, Style::StrokeAndFill];

    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    // we don't serialize hinting or encoding -- soon to be removed from paint

    for &cap in &caps {
        paint.set_stroke_cap(cap);
        for &join in &joins {
            paint.set_stroke_join(join);
            for &style in &styles {
                paint.set_style(style);

                let mut writer = BinaryWriteBuffer::new();
                writer.write_paint(&paint);

                let mut buf = vec![0u8; writer.bytes_written()];
                writer.write_to_memory(&mut buf);
                let mut reader = ReadBuffer::new(&buf);

                let paint2 = reader.read_paint(&FlattenableRegistry::EMPTY);
                reporter_assert!(reporter, paint2 == paint);
            }
        }
    }
});

// Port of: tests/PaintTest.cpp#L164-L182 (chrome/m156)
def_test!(Paint_MoreFlattening, |r| {
    let mut paint = Paint::default();
    paint.set_color(0x00AA_BBCCu32);
    paint.set_blend_mode(BlendMode::Modulate);

    let mut writer = BinaryWriteBuffer::new();
    writer.write_paint(&paint);

    let mut buf = vec![0u8; writer.bytes_written()];
    writer.write_to_memory(&mut buf);
    let mut reader = ReadBuffer::new(&buf);

    let other = reader.read_paint(&FlattenableRegistry::EMPTY);
    reporter_assert!(r, reader.offset() == writer.bytes_written());

    // No matter the encoding, these must always hold.
    reporter_assert!(r, other.color() == paint.color());
    reporter_assert!(r, other.as_blend_mode() == paint.as_blend_mode());
});

// Port of: tests/PaintTest.cpp#L184-L206 (chrome/m156)
def_test!(Paint_nothingToDraw, |r| {
    let mut paint = Paint::default();

    reporter_assert!(r, !paint.nothing_to_draw());
    paint.set_alpha(0);
    reporter_assert!(r, paint.nothing_to_draw());

    paint.set_alpha(0xFF);
    paint.set_blend_mode(BlendMode::Dst);
    reporter_assert!(r, paint.nothing_to_draw());

    paint.set_alpha(0);
    paint.set_blend_mode(BlendMode::SrcOver);

    let mut cm = ColorMatrix::default();
    cm.set_identity(); // does not change alpha
    paint.set_color_filter(color_filters::matrix(&cm, Clamp::Yes));
    reporter_assert!(r, paint.nothing_to_draw());

    cm.post_translate(0.0, 0.0, 0.0, 1.0 / 255.0); // wacks alpha
    paint.set_color_filter(color_filters::matrix(&cm, Clamp::Yes));
    reporter_assert!(r, !paint.nothing_to_draw());
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
