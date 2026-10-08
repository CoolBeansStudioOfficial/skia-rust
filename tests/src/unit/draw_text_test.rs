// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/DrawTextTest.cpp (chrome/m156)
//
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::IRect;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::{default_font, default_typeface};

use crate::{def_test, reporter_assert};

const BG_COLOR: Color = Color::WHITE;

// Port of: tests/DrawTextTest.cpp#L19-L21 (chrome/m156), create
fn create(bound: &IRect) -> Bitmap {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((bound.width(), bound.height()), None);
    bm
}

// Port of: tests/DrawTextTest.cpp#L23-L53 (chrome/m156), compare
fn compare(reference: &Bitmap, iref: &IRect, test: &Bitmap, itest: &IRect) -> bool {
    let x_off = itest.left - iref.left;
    let y_off = itest.top - iref.top;

    for y in 0..test.height() {
        for x in 0..test.width() {
            let test_color = test.get_color((x, y));
            let ref_x = x + x_off;
            let ref_y = y + y_off;
            let ref_color = if ref_x >= 0
                && ref_x < reference.width()
                && ref_y >= 0
                && ref_y < reference.height()
            {
                reference.get_color((ref_x, ref_y))
            } else {
                BG_COLOR
            };
            if ref_color != test_color {
                return false;
            }
        }
    }
    true
}

// Test that a dash path effect on text makes the text disappear: the dash has an "on" interval of
// 1 and an "off" interval of 10000, so the stroked "A" never draws.
// Port of: tests/DrawTextTest.cpp#L71-L113 (chrome/m156), DrawText_dashout
def_test!(DrawText_dashout, |reporter| {
    let size = IRect::from_wh(64, 64);

    let mut draw_text_bitmap = create(&size);
    let draw_text_canvas = Canvas::from_bitmap(&mut draw_text_bitmap, None).expect("canvas");

    let mut draw_dashed_text_bitmap = create(&size);
    let draw_dashed_text_canvas =
        Canvas::from_bitmap(&mut draw_dashed_text_bitmap, None).expect("canvas");

    let mut empty_bitmap = create(&size);
    let empty_canvas = Canvas::from_bitmap(&mut empty_bitmap, None).expect("canvas");

    let point = (25.0_f32, 25.0_f32);
    let mut font = Font::from_size(default_typeface(), 20.0);
    font.set_edging(Edging::SubpixelAntiAlias);
    font.set_subpixel(true);

    let mut stroke_paint = Paint::default();
    stroke_paint.set_color(Color::GRAY);
    stroke_paint.set_style(Style::Stroke);

    // Draw a stroked "A" without a dash which will draw something.
    draw_text_canvas.draw_color(BG_COLOR, None);
    draw_text_canvas.draw_str("A", point, &font, &stroke_paint);

    // Draw an "A" but with a dash which will never draw anything.
    stroke_paint.set_stroke_width(2.0);
    let big_interval = 10000.0;
    let intervals = [1.0, big_interval];
    stroke_paint.set_path_effect(skia_rust_effects::dash_path_effect::new(&intervals, 2.0));

    draw_dashed_text_canvas.draw_color(BG_COLOR, None);
    draw_dashed_text_canvas.draw_str("A", point, &font, &stroke_paint);

    // Draw nothing.
    empty_canvas.draw_color(BG_COLOR, None);

    // The canvases own their bitmaps until they drop, so compare after they are gone.
    drop((draw_text_canvas, draw_dashed_text_canvas, empty_canvas));
    reporter_assert!(
        reporter,
        !compare(&draw_text_bitmap, &size, &empty_bitmap, &size)
    );
    reporter_assert!(
        reporter,
        compare(&draw_dashed_text_bitmap, &size, &empty_bitmap, &size)
    );
});

// Test drawing a text blob, then text whose bytes are not valid glyphs of the font.
// Port of: tests/DrawTextTest.cpp#L173-L187 (chrome/m156), DrawText_noglyphs
def_test!(DrawText_noglyphs, |_reporter| {
    let mut surface =
        surfaces::raster(&ImageInfo::new_n32_premul((100, 100), None), None, None).unwrap();
    let canvas = surface.canvas();
    let font = default_font();
    let text = "Hamburgfons";
    {
        // scoped to ensure blob is deleted.
        if let Some(blob) = TextBlob::from_str(text, &font) {
            canvas.draw_text_blob(&blob, (10.0, 10.0), &Paint::default());
        }
    }
    canvas.draw_simple_text(
        b"\x0d\xf3\xf2\xf2\xe9\x0d\x0d\x0d\x05\x0d\x0d\xe3\xe3\xe3\xe3\xe3\xe3\xe3\xe3\xe3",
        TextEncoding::UTF8,
        (10.0, 20.0),
        &font,
        &Paint::default(),
    );
});

// Test drawing text at some unusual coordinates.
// We measure success by not crashing or asserting.
// Port of: tests/DrawTextTest.cpp#L117-L132 (chrome/m156)
def_test!(DrawText_weirdCoordinates, |_reporter| {
    let mut surface =
        surfaces::raster(&ImageInfo::new_n32_premul((10, 10), None), None, None).unwrap();
    let canvas = surface.canvas();

    let font = default_font();
    let oddballs = [0.0_f32, f32::INFINITY, f32::NAN, 34_359_738_368.0];
    for x in oddballs {
        canvas.draw_str("a", (x, 0.0), &font, &Paint::default());
        canvas.draw_str("a", (-x, 0.0), &font, &Paint::default());
    }
    for y in oddballs {
        canvas.draw_str("a", (0.0, y), &font, &Paint::default());
        canvas.draw_str("a", (0.0, -y), &font, &Paint::default());
    }
});

// Test drawing text with some unusual matrices.
// We measure success by not crashing or asserting.
// Port of: tests/DrawTextTest.cpp#L136-L169 (chrome/m156)
def_test!(DrawText_weirdMatricies, |_reporter| {
    let mut surface =
        surfaces::raster(&ImageInfo::new_n32_premul((100, 100), None), None, None).unwrap();
    let canvas = surface.canvas();

    let mut font = default_font();
    font.set_edging(Edging::SubpixelAntiAlias);

    // (textSize, matrix) pairs.
    let test_cases: [(f32, [f32; 9]); 10] = [
        // 2x2 singular
        (10.0, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
        (10.0, [0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        (10.0, [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
        (10.0, [0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        (10.0, [0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        (10.0, [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
        (10.0, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
        (10.0, [1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
        (10.0, [1.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
        // See https://bugzilla.mozilla.org/show_bug.cgi?id=1305085 .
        (1.0, [10.0, 20.0, 0.0, 20.0, 40.0, 0.0, 0.0, 0.0, 1.0]),
    ];
    for (text_size, m) in test_cases {
        font.set_size(text_size);
        let mut mat = Matrix::default();
        mat.set_all(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8]);
        canvas.set_matrix(&M44::from(mat));
        canvas.draw_str("Hamburgefons", (10.0, 10.0), &font, &Paint::default());
    }
});
