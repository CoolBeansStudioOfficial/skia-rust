// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/TextBlobTest.cpp (chrome/m156)
//
// Not ported yet:
// - `TextBlob_getIntercepts` and `TextBlob_serialize`: they need `SkTextBlob::getIntercepts` and
//   the blob serialization (T15b).
// - `SkCanvas_drawTextBlob_b513820666`: it records and replays a picture with typeface procs
//   (T16) and draws through a drawable typeface (T17).

#![cfg(test)]
// The assertions compare floats exactly, as the C++ `REPORTER_ASSERT`s do, and cast small test
// counts to scalars. `test_builder` is one C++ function, so it stays one function.
#![allow(
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::cast_possible_truncation,
    clippy::needless_range_loop
)]

use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_core::text_blob::{GlyphPositioning, TextBlob, TextBlobBuilder};
use skia_rust_core::typeface::Typeface;
use skia_rust_tools::font_tool_utils::{
    create_portable_typeface, create_test_typeface, default_font,
};

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/TextBlobTest.cpp#L20-L30 (chrome/m156), the RunDef of TextBlobTester
#[derive(Clone, Copy)]
struct RunDef {
    count: usize,
    pos: GlyphPositioning,
    x: scalar,
    y: scalar,
}

// Port of: tests/TextBlobTest.cpp#L33-L51 (chrome/m156), TextBlobTester::AddRun
fn add_run(
    font: &Font,
    count: usize,
    pos: GlyphPositioning,
    offset: Point,
    builder: &mut TextBlobBuilder,
    bounds: Option<&Rect>,
) {
    match pos {
        GlyphPositioning::Default => {
            let glyphs = builder.alloc_run(font, count, offset.x, offset.y, bounds);
            for (i, glyph) in glyphs.iter_mut().enumerate() {
                *glyph = GlyphId::try_from(i).expect("test counts fit a glyph id");
            }
        }
        GlyphPositioning::Horizontal => {
            let (glyphs, xs) = builder.alloc_run_pos_h(font, count, offset.y, bounds);
            for (i, (glyph, x)) in glyphs.iter_mut().zip(xs.iter_mut()).enumerate() {
                *glyph = GlyphId::try_from(i).expect("test counts fit a glyph id");
                *x = i as scalar;
            }
        }
        GlyphPositioning::Full => {
            let (glyphs, points) = builder.alloc_run_pos(font, count, bounds);
            for (i, (glyph, point)) in glyphs.iter_mut().zip(points.iter_mut()).enumerate() {
                *glyph = GlyphId::try_from(i).expect("test counts fit a glyph id");
                *point = Point::new(i as scalar, -(i as scalar));
            }
        }
        GlyphPositioning::RSXform => unreachable!("the tests add no RSXform runs"),
    }
}

// Port of: tests/TextBlobTest.cpp#L53-L101 (chrome/m156), TextBlobTester::RunBuilderTest
fn run_builder_test(
    reporter: &mut Reporter,
    builder: &mut TextBlobBuilder,
    input: &[RunDef],
    output: &[RunDef],
) {
    let font = default_font();
    for run in input {
        add_run(
            &font,
            run.count,
            run.pos,
            Point::new(run.x, run.y),
            builder,
            None,
        );
    }
    let blob = builder.make();
    reporter_assert!(reporter, input.is_empty() == blob.is_none());
    let Some(blob) = blob else {
        return;
    };
    let mut it = blob.run_iter();
    for out in output {
        reporter_assert!(reporter, !it.done());
        reporter_assert!(reporter, out.pos == it.positioning());
        reporter_assert!(reporter, out.count == it.glyph_count());
        match out.pos {
            GlyphPositioning::Default => {
                reporter_assert!(reporter, out.x == it.offset().x);
                reporter_assert!(reporter, out.y == it.offset().y);
            }
            GlyphPositioning::Horizontal => {
                reporter_assert!(reporter, out.y == it.offset().y);
            }
            _ => {}
        }
        for k in 0..it.glyph_count() {
            reporter_assert!(
                reporter,
                GlyphId::try_from(k % 128).expect("fits") == it.glyphs()[k]
            );
            match it.positioning() {
                GlyphPositioning::Horizontal => {
                    reporter_assert!(reporter, (k % 128) as scalar == it.pos()[k]);
                }
                GlyphPositioning::Full => {
                    reporter_assert!(reporter, (k % 128) as scalar == it.points()[k].x);
                    reporter_assert!(reporter, -((k % 128) as scalar) == it.points()[k].y);
                }
                _ => {}
            }
        }
        it.next();
    }
    reporter_assert!(reporter, it.done());
}

// This unit test feeds an SkTextBlobBuilder various runs then checks to see if the result
// contains the provided data and merges runs when appropriate.
// Port of: tests/TextBlobTest.cpp#L103-L165 (chrome/m156), TextBlobTester::TestBuilder
fn test_builder(reporter: &mut Reporter) {
    let mut builder = TextBlobBuilder::new();

    // empty run set
    run_builder_test(reporter, &mut builder, &[], &[]);

    let set1 = [RunDef {
        count: 128,
        pos: GlyphPositioning::Default,
        x: 100.0,
        y: 100.0,
    }];
    run_builder_test(reporter, &mut builder, &set1, &set1);

    let set2 = [RunDef {
        count: 128,
        pos: GlyphPositioning::Horizontal,
        x: 100.0,
        y: 100.0,
    }];
    run_builder_test(reporter, &mut builder, &set2, &set2);

    let set3 = [RunDef {
        count: 128,
        pos: GlyphPositioning::Full,
        x: 100.0,
        y: 100.0,
    }];
    run_builder_test(reporter, &mut builder, &set3, &set3);

    let d = |pos, x, y| RunDef {
        count: 128,
        pos,
        x,
        y,
    };
    let set4 = [
        d(GlyphPositioning::Default, 100.0, 150.0),
        d(GlyphPositioning::Default, 100.0, 150.0),
        d(GlyphPositioning::Default, 100.0, 150.0),
    ];
    run_builder_test(reporter, &mut builder, &set4, &set4);

    let set5 = [
        d(GlyphPositioning::Horizontal, 100.0, 150.0),
        d(GlyphPositioning::Horizontal, 200.0, 150.0),
        d(GlyphPositioning::Horizontal, 300.0, 250.0),
    ];
    let merged_set5 = [
        RunDef {
            count: 256,
            pos: GlyphPositioning::Horizontal,
            x: 0.0,
            y: 150.0,
        },
        RunDef {
            count: 128,
            pos: GlyphPositioning::Horizontal,
            x: 0.0,
            y: 250.0,
        },
    ];
    run_builder_test(reporter, &mut builder, &set5, &merged_set5);

    let set6 = [
        d(GlyphPositioning::Full, 100.0, 100.0),
        d(GlyphPositioning::Full, 200.0, 200.0),
        d(GlyphPositioning::Full, 300.0, 300.0),
    ];
    let merged_set6 = [RunDef {
        count: 384,
        pos: GlyphPositioning::Full,
        x: 0.0,
        y: 0.0,
    }];
    run_builder_test(reporter, &mut builder, &set6, &merged_set6);

    let set7 = [
        d(GlyphPositioning::Default, 100.0, 150.0),
        d(GlyphPositioning::Default, 100.0, 150.0),
        d(GlyphPositioning::Horizontal, 100.0, 150.0),
        d(GlyphPositioning::Horizontal, 200.0, 150.0),
        d(GlyphPositioning::Full, 400.0, 350.0),
        d(GlyphPositioning::Full, 400.0, 350.0),
        d(GlyphPositioning::Default, 100.0, 450.0),
        d(GlyphPositioning::Default, 100.0, 450.0),
        d(GlyphPositioning::Horizontal, 100.0, 550.0),
        d(GlyphPositioning::Horizontal, 200.0, 650.0),
        d(GlyphPositioning::Full, 400.0, 750.0),
        d(GlyphPositioning::Full, 400.0, 850.0),
    ];
    let merged_set7 = [
        d(GlyphPositioning::Default, 100.0, 150.0),
        d(GlyphPositioning::Default, 100.0, 150.0),
        RunDef {
            count: 256,
            pos: GlyphPositioning::Horizontal,
            x: 0.0,
            y: 150.0,
        },
        RunDef {
            count: 256,
            pos: GlyphPositioning::Full,
            x: 0.0,
            y: 0.0,
        },
        d(GlyphPositioning::Default, 100.0, 450.0),
        d(GlyphPositioning::Default, 100.0, 450.0),
        RunDef {
            count: 128,
            pos: GlyphPositioning::Horizontal,
            x: 0.0,
            y: 550.0,
        },
        RunDef {
            count: 128,
            pos: GlyphPositioning::Horizontal,
            x: 0.0,
            y: 650.0,
        },
        RunDef {
            count: 256,
            pos: GlyphPositioning::Full,
            x: 0.0,
            y: 0.0,
        },
    ];
    run_builder_test(reporter, &mut builder, &set7, &merged_set7);
}

// Port of: tests/TextBlobTest.cpp#L167-L233 (chrome/m156), TextBlobTester::TestBounds
fn test_bounds(reporter: &mut Reporter) {
    let mut builder = TextBlobBuilder::new();
    let mut font = default_font();

    // Explicit bounds.
    reporter_assert!(reporter, builder.make().is_none());
    {
        let r1 = Rect::from_xywh(10.0, 10.0, 20.0, 20.0);
        builder.alloc_run(&font, 16, 0.0, 0.0, Some(&r1));
        let blob = builder.make().expect("a run was added");
        reporter_assert!(reporter, *blob.bounds() == r1);
    }
    {
        let r1 = Rect::from_xywh(10.0, 10.0, 20.0, 20.0);
        builder.alloc_run_pos_h(&font, 16, 0.0, Some(&r1));
        let blob = builder.make().expect("a run was added");
        reporter_assert!(reporter, *blob.bounds() == r1);
    }
    {
        let r1 = Rect::from_xywh(10.0, 10.0, 20.0, 20.0);
        builder.alloc_run_pos(&font, 16, Some(&r1));
        let blob = builder.make().expect("a run was added");
        reporter_assert!(reporter, *blob.bounds() == r1);
    }
    {
        let r1 = Rect::from_xywh(10.0, 10.0, 20.0, 20.0);
        let r2 = Rect::from_xywh(15.0, 20.0, 50.0, 50.0);
        let r3 = Rect::from_xywh(0.0, 5.0, 10.0, 5.0);
        builder.alloc_run(&font, 16, 0.0, 0.0, Some(&r1));
        builder.alloc_run_pos_h(&font, 16, 0.0, Some(&r2));
        builder.alloc_run_pos(&font, 16, Some(&r3));
        let blob = builder.make().expect("runs were added");
        reporter_assert!(
            reporter,
            *blob.bounds() == Rect::from_xywh(0.0, 5.0, 65.0, 65.0)
        );
    }
    reporter_assert!(reporter, builder.make().is_none());

    // Implicit bounds
    {
        // Exercise the empty bounds path, and ensure that RunRecord-aligned pos buffers don't
        // trigger asserts (http://crbug.com/542643).
        font.set_size(0.0);
        let txt = b"BOOO";
        let glyph_count = font.count_text(txt, TextEncoding::UTF8);
        let (glyphs, pos) = builder.alloc_run_pos(&font, glyph_count, None);
        font.text_to_glyphs(txt, TextEncoding::UTF8, glyphs);
        pos.fill(Point::default());
        let blob = builder.make().expect("a run was added");
        reporter_assert!(reporter, blob.bounds().is_empty());
    }
}

// Verify that text-related properties are captured in run paints.
// Port of: tests/TextBlobTest.cpp#L235-L287 (chrome/m156), TextBlobTester::TestPaintProps
fn test_paint_props(reporter: &mut Reporter) {
    let mut font = Font::default();
    // Kitchen sink font.
    font.set_size(42.0);
    font.set_scale_x(4.2);
    font.set_typeface(Some(create_portable_typeface(
        Some("Sans"),
        FontStyle::bold(),
    )));
    font.set_skew_x(0.42);
    font.set_hinting(skia_rust_core::font_types::FontHinting::Full);
    font.set_edging(Edging::SubpixelAntiAlias);
    font.set_embolden(true);
    font.set_linear_metrics(true);
    font.set_subpixel(true);
    font.set_embedded_bitmaps(true);
    font.set_force_auto_hinting(true);

    // Ensure we didn't pick default values by mistake.
    let default_font = default_font();
    reporter_assert!(reporter, default_font.size() != font.size());
    reporter_assert!(reporter, default_font.scale_x() != font.scale_x());
    reporter_assert!(reporter, *default_font.typeface() != *font.typeface());
    reporter_assert!(reporter, default_font.skew_x() != font.skew_x());
    reporter_assert!(reporter, default_font.hinting() != font.hinting());
    reporter_assert!(reporter, default_font.edging() != font.edging());
    reporter_assert!(reporter, default_font.is_embolden() != font.is_embolden());
    reporter_assert!(
        reporter,
        default_font.is_linear_metrics() != font.is_linear_metrics()
    );
    reporter_assert!(reporter, default_font.is_subpixel() != font.is_subpixel());
    reporter_assert!(
        reporter,
        default_font.is_embedded_bitmaps() != font.is_embedded_bitmaps()
    );
    reporter_assert!(
        reporter,
        default_font.is_force_auto_hinting() != font.is_force_auto_hinting()
    );

    let mut builder = TextBlobBuilder::new();
    add_run(
        &font,
        1,
        GlyphPositioning::Default,
        Point::new(0.0, 0.0),
        &mut builder,
        None,
    );
    add_run(
        &font,
        1,
        GlyphPositioning::Horizontal,
        Point::new(0.0, 0.0),
        &mut builder,
        None,
    );
    add_run(
        &font,
        1,
        GlyphPositioning::Full,
        Point::new(0.0, 0.0),
        &mut builder,
        None,
    );
    let blob = builder.make().expect("runs were added");
    let mut it = blob.run_iter();
    while !it.done() {
        reporter_assert!(reporter, it.font() == Some(&font));
        it.next();
    }
}

// Port of: tests/TextBlobTest.cpp#L335-L338 (chrome/m156), TextBlob_builder
def_test!(TextBlob_builder, |reporter| {
    test_builder(reporter);
    test_bounds(reporter);
});

// Port of: tests/TextBlobTest.cpp#L340-L342 (chrome/m156), TextBlob_paint
def_test!(TextBlob_paint, |reporter| {
    test_paint_props(reporter);
});

// Port of: tests/TextBlobTest.cpp#L344-L377 (chrome/m156), TextBlob_extended
def_test!(TextBlob_extended, |reporter| {
    let mut builder = TextBlobBuilder::new();
    let font = default_font();
    let text1 = b"Foo";
    let text2 = b"Bar";

    let glyph_count = font.count_text(text1, TextEncoding::UTF8);
    let mut glyphs = vec![0 as GlyphId; glyph_count];
    font.text_to_glyphs(text1, TextEncoding::UTF8, &mut glyphs);

    {
        let (run_glyphs, run_text, run_clusters) =
            builder.alloc_run_text(&font, glyph_count, (0.0, 0.0), text2.len(), None);
        run_glyphs.copy_from_slice(&glyphs);
        run_text.copy_from_slice(text2);
        for (i, cluster) in run_clusters.iter_mut().enumerate() {
            *cluster = u32::try_from(i).expect("fits").min(text2.len() as u32);
        }
    }
    let blob = builder.make();
    reporter_assert!(reporter, blob.is_some());
    let blob = blob.expect("checked above");

    let mut it = blob.run_iter();
    while !it.done() {
        reporter_assert!(reporter, it.glyph_count() == glyph_count);
        for i in 0..it.glyph_count() {
            reporter_assert!(reporter, it.glyphs()[i] == glyphs[i]);
        }
        reporter_assert!(reporter, GlyphPositioning::Default == it.positioning());
        reporter_assert!(reporter, Point::new(0.0, 0.0) == it.offset());
        reporter_assert!(reporter, it.text_size() > 0);
        reporter_assert!(reporter, !it.clusters().is_empty());
        for i in 0..it.glyph_count() {
            reporter_assert!(reporter, i as u32 == it.clusters()[i]);
        }
        reporter_assert!(reporter, text2 == it.text());
        it.next();
    }
});

// Port of: tests/TextBlobTest.cpp#L473-L485 (chrome/m156), TextBlob_MakeAsDrawText
def_test!(TextBlob_MakeAsDrawText, |reporter| {
    let text = "Hello";
    let blob = TextBlob::from_str(text, &default_font()).expect("the text has glyphs");

    let mut runs = 0;
    let mut it = blob.run_iter();
    while !it.done() {
        reporter_assert!(reporter, it.glyph_count() == text.len());
        reporter_assert!(reporter, it.positioning() == GlyphPositioning::Full);
        runs += 1;
        it.next();
    }
    reporter_assert!(reporter, runs == 1);
});

// Port of: tests/TextBlobTest.cpp#L379-L393 (chrome/m156), add_run (the iterator test's helper)
fn add_text_run(builder: &mut TextBlobBuilder, text: &str, x: scalar, y: scalar, tf: &Typeface) {
    let mut font = Font::from_size(tf.clone(), 16.0);
    font.set_edging(Edging::AntiAlias);
    font.set_subpixel(true);
    let glyph_count = font.count_text(text.as_bytes(), TextEncoding::UTF8);
    let glyphs = builder.alloc_run(&font, glyph_count, x, y, None);
    font.text_to_glyphs(text.as_bytes(), TextEncoding::UTF8, glyphs);
}

// Port of: tests/TextBlobTest.cpp#L487-L517 (chrome/m156), TextBlob_iter
def_test!(TextBlob_iter, |reporter| {
    let tf = create_test_typeface(None, FontStyle::bold_italic());

    let mut builder = TextBlobBuilder::new();
    add_text_run(&mut builder, "Hello", 10.0, 20.0, &tf);
    add_text_run(&mut builder, "World!", 10.0, 40.0, &tf);
    let blob = builder.make().expect("runs were added");

    // (typeface, glyph count)
    let expected = [(&tf, 5usize), (&tf, 6usize)];
    let mut iter = blob.iter();
    for (exp_typeface, exp_count) in expected {
        let run = iter.next().expect("a run for each expected run");
        reporter_assert!(reporter, run.typeface() == exp_typeface);
        reporter_assert!(reporter, run.glyph_count() == exp_count);
        for i in 0..run.glyph_count() {
            reporter_assert!(
                reporter,
                run.glyph_indices[i] != 0,
                "Glyph Index {i} is unexpectedly 0"
            );
        }
    }
    reporter_assert!(reporter, iter.next().is_none()); // we're done

    let mut iter2 = blob.iter();
    let run = iter2.next().expect("a first run");
    // Hello should have the same glyph repeated for the 'l'
    reporter_assert!(reporter, run.glyph_indices[2] == run.glyph_indices[3]);
});
