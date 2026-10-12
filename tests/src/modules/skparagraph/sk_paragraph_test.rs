// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skparagraph/tests/SkParagraphTest.cpp (chrome/m156)

#![cfg(test)]
// Variable names mirror the C++ tests.
#![allow(non_snake_case)]
// The tests mirror the C++ literals and casts (sizes and counts that fit an `f32`).
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::float_cmp,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::unreadable_literal,
    clippy::approx_constant,
    clippy::excessive_precision
)]
// The tests are 1:1 translations of the C++: its idioms (`size() == 0`, `== false`, `== nullptr`,
// `return` in lambdas, `05`, `(a + b) / 2`, text that is not in NFC, pointer identity) are kept.
#![allow(
    clippy::len_zero,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::manual_assert_eq,
    clippy::bool_comparison,
    clippy::partialeq_to_none,
    clippy::needless_return,
    clippy::zero_prefixed_literal,
    clippy::needless_range_loop,
    clippy::manual_midpoint,
    clippy::needless_borrow,
    clippy::unicode_not_nfc,
    clippy::borrow_as_ptr,
    clippy::bool_to_int_with_if,
    clippy::if_not_else,
    clippy::field_reassign_with_default
)]

use std::sync::{Arc, Mutex};

use skia_rust_core::canvas::AutoCanvasRestore;
use skia_rust_core::color::{Color, colors};
use skia_rust_core::font::Font;
use skia_rust_core::font_style::{FontStyle, Slant, Weight, Width};
use skia_rust_core::paint::{Paint, Style as PaintStyle};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{
    SCALAR_INFINITY, SCALAR_MAX, SCALAR_NAN, SCALAR_NEARLY_ZERO, SCALAR_NEGATIVE_INFINITY, Scalar,
    scalar,
};
use skia_rust_core::utf::Unichar;
// `using namespace skia::textlayout;`
use skia_rust_paragraph::one_line_shaper::get_emoji_sequence_start;
use skia_rust_paragraph::utils::TestFontCollection;
#[allow(clippy::wildcard_imports)]
use skia_rust_paragraph::*;
use skia_rust_tools::font_tool_utils::test_font_mgr;
use skia_rust_tools::paragraph_fonts::ResourceFontCollection;
use skia_rust_unicode::unicode::{Unicode, convert_utf16_to_utf8};

use crate::{Reporter, def_paragraph_test, errorf, reporter_assert};

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L74-L76 (chrome/m156), the flags
// `paragraph_fonts` and `run_paragraph_tests_needing_system_fonts`: see `skia_rust_tools`.

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L78-L80 (chrome/m156)
const VERY_LONG_CANVAS_WIDTH: scalar = 1_000_000.0;
const TEST_CANVAS_WIDTH: scalar = 1000.0;
const TEST_CANVAS_HEIGHT: scalar = 600.0;

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L85-L91 (chrome/m156)
const EPSILON1000: scalar = 0.001;
const EPSILON100: scalar = 0.01;
const EPSILON50: scalar = 0.02;
const EPSILON20: scalar = 0.05;
const EPSILON10: scalar = 0.1;
const EPSILON5: scalar = 0.20;
const EPSILON2: scalar = 0.50;

/// `GetResourcePath(name)`: the path of a resource of Skia's `resources` directory.
fn get_resource_path(name: &str) -> String {
    let dir = skia_rust_tools::resources::resource_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("resources"));
    dir.join(name).to_string_lossy().into_owned()
}

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L93-L95 (chrome/m156)
fn equal(base: &[u8], a: TextRange, b: &str) -> bool {
    // `std::strncmp(b, base + a.start, a.width()) == 0`
    let width = a.width();
    let lhs = &base[a.start..a.start + width];
    let rhs = b.as_bytes();
    for i in 0..width {
        let l = lhs.get(i).copied().unwrap_or(0);
        let r = rhs.get(i).copied().unwrap_or(0);
        if l != r {
            return false;
        }
        if r == 0 {
            return true;
        }
    }
    true
}

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L97-L106 (chrome/m156)
fn mirror(text: &str) -> Vec<u16> {
    let mut result: Vec<u16> = Vec::new();
    result.push(0x202E);
    for ch in text.bytes().rev() {
        result.push(u16::from(ch));
    }
    //result += u"\u202C";
    result
}

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L108-L116 (chrome/m156)
fn straight(text: &str) -> Vec<u16> {
    let mut result: Vec<u16> = Vec::new();
    result.push(0x202D);
    for ch in text.bytes() {
        result.push(u16::from(ch));
    }
    result
}

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L205-L255 (chrome/m156)
struct TestCanvas {
    name: &'static str,
    surface: skia_rust_raster::surface::Surface<'static>,
}

impl TestCanvas {
    // Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L207-L211 (chrome/m156)
    fn new(test_name: &'static str) -> Self {
        let mut surface = skia_rust_raster::surfaces::raster_n32_premul((
            TEST_CANVAS_WIDTH as i32,
            TEST_CANVAS_HEIGHT as i32,
        ))
        .expect("a surface");
        surface.canvas().clear(Color::WHITE);
        Self {
            name: test_name,
            surface,
        }
    }

    // Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L223-L238 (chrome/m156)
    fn draw_rects(&mut self, color: Color, result: &[TextBox]) {
        self.draw_rects_fill(color, result, false);
    }

    fn draw_rects_fill(&mut self, color: Color, result: &[TextBox], fill: bool) {
        let mut paint = Paint::default();
        if !fill {
            paint.set_style(PaintStyle::Stroke);
            paint.set_anti_alias(true);
            paint.set_stroke_width(1.0);
        }
        paint.set_color(color);
        for r in result {
            self.surface.canvas().draw_rect(r.rect, &paint);
        }
    }

    // Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L240-L252 (chrome/m156)
    fn draw_line(&mut self, color: Color, rect: Rect, vertical: bool) {
        let mut paint = Paint::default();
        paint.set_style(PaintStyle::Stroke);
        paint.set_anti_alias(true);
        paint.set_stroke_width(1.0);
        paint.set_color(color);
        if vertical {
            self.surface.canvas().draw_line(
                (rect.left, rect.top),
                (rect.left, rect.bottom),
                &paint,
            );
        } else {
            self.surface
                .canvas()
                .draw_line((rect.left, rect.top), (rect.right, rect.top), &paint);
        }
    }

    // Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L254-L258 (chrome/m156)
    fn draw_lines(&mut self, color: Color, result: &[TextBox]) {
        for r in result {
            self.draw_line(color, r.rect, true);
        }
    }

    // Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L260 (chrome/m156)
    fn get(&mut self) -> &skia_rust_core::canvas::Canvas {
        self.surface.canvas()
    }
}

impl Drop for TestCanvas {
    // Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L213-L222 (chrome/m156)
    fn drop(&mut self) {
        if let Some(tmp_dir) = crate::tmp_dir::get_tmp_dir() {
            let path = tmp_dir.join(self.name);
            if let Some(pixmap) = self.surface.peek_pixels() {
                let encoded = skia_rust_codec::encode::png_encoder::encode_pixmap(
                    &pixmap.pixmap(),
                    &skia_rust_codec::encode::png_encoder::Options::default(),
                );
                match encoded {
                    Some(data) if std::fs::write(&path, data.as_bytes()).is_ok() => {}
                    _ => eprintln!("Cannot write a picture {}", self.name),
                }
            }
        }
    }
}

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L268-L271 (chrome/m156)
fn get_unicode() -> Arc<dyn Unicode> {
    // `SkShapers::BestAvailable()->getUnicode()`: the factory keeps the Unicode it was made
    // with, and `BestAvailable` makes it with `SkUnicodes::ICU::Make()`.
    skia_rust_unicode::unicode_icu::make().expect("a Unicode implementation")
}

// `SkScalarNearlyEqual` and `SkScalarNearlyZero` of SkScalar.h, with their default tolerance.
fn sk_scalar_nearly_equal_fn(
    x: impl Into<f64>,
    y: impl Into<f64>,
    tolerance: impl Into<f64>,
) -> bool {
    // The C++ calls convert their (often double) arguments to `SkScalar` implicitly.
    <scalar as Scalar>::nearly_equal(
        x.into() as scalar,
        y.into() as scalar,
        tolerance.into() as scalar,
    )
}

macro_rules! sk_scalar_nearly_equal {
    ($x:expr, $y:expr) => {
        sk_scalar_nearly_equal_fn($x, $y, SCALAR_NEARLY_ZERO)
    };
    ($x:expr, $y:expr, $tolerance:expr) => {
        sk_scalar_nearly_equal_fn($x, $y, $tolerance)
    };
}

/// `nearly_equal!(x, y, tolerance = SK_ScalarNearlyZero)` of TextStyle.h.
macro_rules! nearly_equal {
    ($x:expr, $y:expr) => {
        skia_rust_paragraph::text_style::nearly_equal($x, $y, SCALAR_NEARLY_ZERO)
    };
    ($x:expr, $y:expr, $tolerance:expr) => {
        skia_rust_paragraph::text_style::nearly_equal($x, $y, $tolerance)
    };
}

/// `std::min(a, b)`: `(b < a) ? b : a`.
fn std_min<T: PartialOrd>(a: T, b: T) -> T {
    if b < a { b } else { a }
}

/// `std::max(a, b)`: `(a < b) ? b : a`.
fn std_max<T: PartialOrd>(a: T, b: T) -> T {
    if a < b { b } else { a }
}

/// The code units of a `u"..."` literal.
fn u16s(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7998-L8004 (chrome/m156)
fn has_empty_typeface(f: &Font) -> bool {
    // `SkTypeface* face = f.getTypeface(); if (!face) return true;`: a `Font` always has a
    // typeface here.
    let face = f.typeface();
    face.count_glyphs() == 0 && face.get_bounds().is_empty()
}

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7445-L7478 (chrome/m156)
fn perform_get_rects_for_range_concurrently(reporter: &Mutex<&mut Reporter>) {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter.lock().unwrap(), font_collection);

    let text: Vec<u16> = vec![u16::from(b'x'); 42000];
    let paragraph_style = ParagraphStyle::new();
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(14.0);
    text_style.set_color(Color::BLACK);
    text_style.set_font_style(FontStyle::new(
        Weight::MEDIUM,
        Width::NORMAL,
        Slant::Upright,
    ));

    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder.push_style(&text_style);
    builder.add_text_utf16(&text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(f32::MAX);

    let height_style = RectHeightStyle::Max;
    let width_style = RectWidthStyle::Max;
    let paragraph = &paragraph;
    std::thread::scope(|scope| {
        let t1 = scope.spawn(move || {
            let result = paragraph.get_rects_for_range(0..2, height_style, width_style);
            reporter_assert!(reporter.lock().unwrap(), !result.is_empty());
        });
        let t2 = scope.spawn(move || {
            let result = paragraph.get_rects_for_range(5..10, height_style, width_style);
            reporter_assert!(reporter.lock().unwrap(), !result.is_empty());
        });
        t1.join().unwrap();
        t2.join().unwrap();
    });
}

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L298-L334 (chrome/m156)
def_paragraph_test!(SkParagraph_SimpleParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let text = "Hello World Text Dialog";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    reporter_assert!(reporter, paragraph.unresolved_glyphs() == Some(0));

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.runs().len() == 1);
    reporter_assert!(reporter, impl_.styles().len() == 1); // paragraph style does not count
    reporter_assert!(reporter, impl_.styles()[0].style.equals(&text_style));

    let mut index: usize = 0;
    for line in impl_.lines() {
        line.scan_styles(
            impl_,
            StyleType::Decorations,
            &mut |_text_range, style, _context| {
                reporter_assert!(reporter, index == 0);
                reporter_assert!(reporter, style.color() == Color::BLACK);
                index += 1;
            },
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L336-L371 (chrome/m156)
def_paragraph_test!(SkParagraph_Rounding_Off_LineBreaks, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let text = "AAAAAAAAAA";
    let len = text.len();

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_apply_rounding_hack(false);
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_color(Color::BLACK);

    let testFontSize: [scalar; 4] = [1.5_f64, 10.0 / 3.0, 10.0 / 6.0, 10.0].map(|v| v as scalar);
    for fontSize in testFontSize {
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        text_style.set_font_size(fontSize);
        builder.push_style(&text_style);
        builder.add_text(text);
        builder.pop();

        let mut paragraph = builder.build();
        paragraph.layout(SCALAR_INFINITY);
        // Slightly wider than the max intrinsic width.
        reporter_assert!(reporter, paragraph.unresolved_glyphs() == Some(0));
        paragraph.layout(paragraph.max_intrinsic_width());

        let impl_ = &paragraph;

        reporter_assert!(reporter, impl_.lines().len() == 1);
        let line = &impl_.lines()[0];

        let metrics = line.get_metrics(impl_);
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(metrics.width, fontSize * len as scalar, EPSILON2)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L373-L471 (chrome/m156)
def_paragraph_test!(SkParagraph_InlinePlaceholderParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    let mut canvas = TestCanvas::new("SkParagraph_InlinePlaceholderParagraph.png");
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = "012 34";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_max_lines(14);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);
    text_style.set_font_size(26.0);
    text_style.set_word_spacing(5.0);
    text_style.set_letter_spacing(1.0);
    text_style.set_decoration_type(TextDecoration::UNDERLINE);
    text_style.set_decoration_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);

    let placeholder1 = PlaceholderStyle::new(
        50.0,
        50.0,
        PlaceholderAlignment::Baseline,
        TextBaseline::Alphabetic,
        0.0,
    );
    builder.add_placeholder(&placeholder1);
    builder.add_text(text);
    builder.add_placeholder(&placeholder1);

    let placeholder2 = PlaceholderStyle::new(
        5.0,
        50.0,
        PlaceholderAlignment::Baseline,
        TextBaseline::Alphabetic,
        50.0,
    );
    builder.add_placeholder(&placeholder2);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder2);
    builder.add_text(text);
    builder.add_placeholder(&placeholder2);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_placeholder(&placeholder2);
    builder.add_placeholder(&placeholder2);
    builder.add_placeholder(&placeholder2);
    builder.add_placeholder(&placeholder2);
    builder.add_placeholder(&placeholder2);
    builder.add_placeholder(&placeholder1);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_placeholder(&placeholder2);
    builder.add_placeholder(&placeholder1);
    builder.add_text(text);
    builder.add_text(text);

    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let rect_height_style = RectHeightStyle::Tight;
    let rect_width_style = RectWidthStyle::Tight;

    let mut boxes = paragraph.get_rects_for_range(0..3, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::RED, &boxes);
    reporter_assert!(reporter, boxes.len() == 1);

    boxes = paragraph.get_rects_for_range(0..3, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::GREEN, &boxes);
    reporter_assert!(reporter, boxes.len() == 1);

    boxes = paragraph.get_rects_for_placeholders();
    canvas.draw_rects(Color::RED, &boxes);

    boxes = paragraph.get_rects_for_range(4..17, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::BLUE, &boxes);

    reporter_assert!(reporter, boxes.len() == 7);
    if boxes.len() >= 7 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.left, 90.921, EPSILON2)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.top, 50.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.right, 90.921 + 50.0, EPSILON2)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.bottom, 100.0, EPSILON100)
        );

        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[3].rect.left, 231.343, EPSILON2)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[3].rect.top, 50.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[3].rect.right, 231.343 + 50.0, EPSILON2)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[3].rect.bottom, 100.0, EPSILON100)
        );

        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[4].rect.left, 281.343, EPSILON2)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[4].rect.top, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[4].rect.right, 281.343 + 5.0, EPSILON2)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[4].rect.bottom, 50.0, EPSILON100)
        );

        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[6].rect.left, 336.343, EPSILON2)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[6].rect.top, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[6].rect.right, 336.343 + 5.0, EPSILON2)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[6].rect.bottom, 50.0, EPSILON100)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L473-L531 (chrome/m156)
def_paragraph_test!(SkParagraph_InlinePlaceholderBaselineParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    let mut canvas = TestCanvas::new("SkParagraph_InlinePlaceholderBaselineParagraph.png");
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = "012 34";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_max_lines(14);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);
    text_style.set_font_size(26.0);
    text_style.set_word_spacing(5.0);
    text_style.set_letter_spacing(1.0);
    text_style.set_decoration_type(TextDecoration::UNDERLINE);
    text_style.set_decoration_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);

    let placeholder = PlaceholderStyle::new(
        55.0,
        50.0,
        PlaceholderAlignment::Baseline,
        TextBaseline::Alphabetic,
        38.347,
    );
    builder.add_placeholder(&placeholder);
    builder.add_text(text);

    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let mut boxes = paragraph.get_rects_for_placeholders();
    canvas.draw_rects(Color::RED, &boxes);

    reporter_assert!(reporter, boxes.len() == 1);
    if boxes.len() >= 1 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 90.921, EPSILON2)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.top, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.right, 90.921 + 55.0, EPSILON2)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 50.0, EPSILON100)
        );
    }

    let rect_height_style = RectHeightStyle::Tight;
    let rect_width_style = RectWidthStyle::Tight;

    boxes = paragraph.get_rects_for_range(5..6, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::BLUE, &boxes);

    reporter_assert!(reporter, boxes.len() == 1);
    if boxes.len() >= 1 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 75.324, EPSILON2)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.top, 14.226, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.right, 90.921, EPSILON2)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 44.694, EPSILON100)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L533-L591 (chrome/m156)
def_paragraph_test!(
    SkParagraph_InlinePlaceholderAboveBaselineParagraph,
    |reporter| {
        let font_collection = ResourceFontCollection::new(false);
        let mut canvas = TestCanvas::new("SkParagraph_InlinePlaceholderAboveBaselineParagraph.png");
        skip_if_fonts_not_found!(reporter, font_collection);

        let text = "012 34";

        let mut paragraph_style = ParagraphStyle::new();
        paragraph_style.turn_hinting_off();
        paragraph_style.set_max_lines(14);
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

        let mut text_style = TextStyle::new();
        text_style.set_font_families(&["Roboto"]);
        text_style.set_color(Color::BLACK);
        text_style.set_font_size(26.0);
        text_style.set_word_spacing(5.0);
        text_style.set_letter_spacing(1.0);
        text_style.set_decoration_type(TextDecoration::UNDERLINE);
        text_style.set_decoration_color(Color::BLACK);
        builder.push_style(&text_style);
        builder.add_text(text);

        let placeholder = PlaceholderStyle::new(
            55.0,
            50.0,
            PlaceholderAlignment::AboveBaseline,
            TextBaseline::Alphabetic,
            903129.129308,
        );
        builder.add_placeholder(&placeholder);
        builder.add_text(text);

        builder.pop();

        let mut paragraph = builder.build();
        paragraph.layout(TEST_CANVAS_WIDTH);
        paragraph.paint(canvas.get(), (0.0, 0.0));

        let mut boxes = paragraph.get_rects_for_placeholders();
        canvas.draw_rects(Color::RED, &boxes);

        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 90.921, EPSILON2)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, -0.347, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 90.921 + 55.0, EPSILON2)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 49.652, EPSILON100)
            );
        }

        let rect_height_style = RectHeightStyle::Tight;
        let rect_width_style = RectWidthStyle::Tight;

        boxes = paragraph.get_rects_for_range(5..6, rect_height_style, rect_width_style);
        canvas.draw_rects(Color::BLUE, &boxes);

        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 75.324, EPSILON2)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 25.531, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 90.921, EPSILON2)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 56.0, EPSILON100)
            );
        }
    }
);

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L593-L651 (chrome/m156)
def_paragraph_test!(
    SkParagraph_InlinePlaceholderBelowBaselineParagraph,
    |reporter| {
        let font_collection = ResourceFontCollection::new(false);
        let mut canvas = TestCanvas::new("SkParagraph_InlinePlaceholderBelowBaselineParagraph.png");
        skip_if_fonts_not_found!(reporter, font_collection);

        let text = "012 34";

        let mut paragraph_style = ParagraphStyle::new();
        paragraph_style.turn_hinting_off();
        paragraph_style.set_max_lines(14);
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

        let mut text_style = TextStyle::new();
        text_style.set_font_families(&["Roboto"]);
        text_style.set_color(Color::BLACK);
        text_style.set_font_size(26.0);
        text_style.set_word_spacing(5.0);
        text_style.set_letter_spacing(1.0);
        text_style.set_decoration_type(TextDecoration::UNDERLINE);
        text_style.set_decoration_color(Color::BLACK);
        builder.push_style(&text_style);
        builder.add_text(text);

        let placeholder = PlaceholderStyle::new(
            55.0,
            50.0,
            PlaceholderAlignment::BelowBaseline,
            TextBaseline::Alphabetic,
            903129.129308,
        );
        builder.add_placeholder(&placeholder);
        builder.add_text(text);

        builder.pop();

        let mut paragraph = builder.build();
        paragraph.layout(TEST_CANVAS_WIDTH);
        paragraph.paint(canvas.get(), (0.0, 0.0));

        let mut boxes = paragraph.get_rects_for_placeholders();
        canvas.draw_rects(Color::RED, &boxes);

        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 90.921, EPSILON2)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 24.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 90.921 + 55.0, EPSILON2)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 74.0, EPSILON100)
            );
        }

        let rect_height_style = RectHeightStyle::Tight;
        let rect_width_style = RectWidthStyle::Tight;

        boxes = paragraph.get_rects_for_range(5..6, rect_height_style, rect_width_style);
        canvas.draw_rects(Color::BLUE, &boxes);

        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 75.324, EPSILON2)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, -0.121, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 90.921, EPSILON2)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 30.347, EPSILON100)
            );
        }
    }
);

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L653-L709 (chrome/m156)
def_paragraph_test!(SkParagraph_InlinePlaceholderBottomParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    let mut canvas = TestCanvas::new("SkParagraph_InlinePlaceholderBottomParagraph.png");
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = "012 34";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_max_lines(14);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);
    text_style.set_font_size(26.0);
    text_style.set_word_spacing(5.0);
    text_style.set_letter_spacing(1.0);
    text_style.set_decoration_type(TextDecoration::UNDERLINE);
    text_style.set_decoration_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);

    let placeholder = PlaceholderStyle::new(
        55.0,
        50.0,
        PlaceholderAlignment::Bottom,
        TextBaseline::Alphabetic,
        0.0,
    );
    builder.add_placeholder(&placeholder);
    builder.add_text(text);

    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let rect_height_style = RectHeightStyle::Tight;
    let rect_width_style = RectWidthStyle::Tight;

    let mut boxes = paragraph.get_rects_for_placeholders();
    canvas.draw_rects(Color::RED, &boxes);
    reporter_assert!(reporter, boxes.len() == 1);
    if boxes.len() >= 1 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 90.921, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.top, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.right, 90.921 + 55.0, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 50.0, EPSILON100)
        );
    }

    boxes = paragraph.get_rects_for_range(0..1, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::BLUE, &boxes);
    reporter_assert!(reporter, boxes.len() == 1);
    if boxes.len() >= 1 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 0.5, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.top, 19.531, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.right, 16.097, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 50.0, EPSILON100)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L711-L767 (chrome/m156)
def_paragraph_test!(SkParagraph_InlinePlaceholderTopParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    let mut canvas = TestCanvas::new("SkParagraph_InlinePlaceholderTopParagraph.png");
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = "012 34";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_max_lines(14);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);
    text_style.set_font_size(26.0);
    text_style.set_word_spacing(5.0);
    text_style.set_letter_spacing(1.0);
    text_style.set_decoration_type(TextDecoration::UNDERLINE);
    text_style.set_decoration_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);

    let placeholder = PlaceholderStyle::new(
        55.0,
        50.0,
        PlaceholderAlignment::Top,
        TextBaseline::Alphabetic,
        0.0,
    );
    builder.add_placeholder(&placeholder);
    builder.add_text(text);

    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let rect_height_style = RectHeightStyle::Tight;
    let rect_width_style = RectWidthStyle::Tight;

    let mut boxes = paragraph.get_rects_for_placeholders();
    canvas.draw_rects(Color::RED, &boxes);
    reporter_assert!(reporter, boxes.len() == 1);
    if boxes.len() >= 1 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 90.921, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.top, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.right, 90.921 + 55.0, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 50.0, EPSILON100)
        );
    }

    boxes = paragraph.get_rects_for_range(0..1, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::BLUE, &boxes);
    reporter_assert!(reporter, boxes.len() == 1);
    if boxes.len() >= 1 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 0.5, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.top, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.right, 16.097, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 30.468, EPSILON100)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L769-L825 (chrome/m156)
def_paragraph_test!(SkParagraph_InlinePlaceholderMiddleParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    let mut canvas = TestCanvas::new("SkParagraph_InlinePlaceholderMiddleParagraph.png");
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = "012 34";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_max_lines(14);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);
    text_style.set_font_size(26.0);
    text_style.set_word_spacing(5.0);
    text_style.set_letter_spacing(1.0);
    text_style.set_decoration_type(TextDecoration::UNDERLINE);
    text_style.set_decoration_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);

    let placeholder = PlaceholderStyle::new(
        55.0,
        50.0,
        PlaceholderAlignment::Middle,
        TextBaseline::Alphabetic,
        0.0,
    );
    builder.add_placeholder(&placeholder);
    builder.add_text(text);

    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let rect_height_style = RectHeightStyle::Tight;
    let rect_width_style = RectWidthStyle::Tight;

    let mut boxes = paragraph.get_rects_for_placeholders();
    canvas.draw_rects(Color::RED, &boxes);
    reporter_assert!(reporter, boxes.len() == 1);
    if boxes.len() >= 1 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 90.921, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.top, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.right, 90.921 + 55.0, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 50.0, EPSILON100)
        );
    }

    boxes = paragraph.get_rects_for_range(5..6, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::BLUE, &boxes);
    reporter_assert!(reporter, boxes.len() == 1);
    if boxes.len() >= 1 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 75.324, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.top, 9.765, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.right, 90.921, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 40.234, EPSILON100)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L827-L882 (chrome/m156)
def_paragraph_test!(
    SkParagraph_InlinePlaceholderIdeographicBaselineParagraph,
    |reporter| {
        let font_collection = ResourceFontCollection::new(false);
        let mut canvas =
            TestCanvas::new("SkParagraph_InlinePlaceholderIdeographicBaselineParagraph.png");
        skip_if_fonts_not_found!(reporter, font_collection);

        let text = "給能上目秘使";

        let mut paragraph_style = ParagraphStyle::new();
        paragraph_style.turn_hinting_off();
        paragraph_style.set_max_lines(14);
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

        let mut text_style = TextStyle::new();
        text_style.set_font_families(&["Source Han Serif CN"]);
        text_style.set_color(Color::BLACK);
        text_style.set_font_size(26.0);
        text_style.set_word_spacing(5.0);
        text_style.set_letter_spacing(1.0);
        text_style.set_decoration_type(TextDecoration::UNDERLINE);
        text_style.set_decoration_color(Color::BLACK);
        builder.push_style(&text_style);
        builder.add_text(text);
        let placeholder = PlaceholderStyle::new(
            55.0,
            50.0,
            PlaceholderAlignment::Baseline,
            TextBaseline::Ideographic,
            38.347,
        );
        builder.add_placeholder(&placeholder);
        builder.add_text(text);

        builder.pop();

        let mut paragraph = builder.build();
        paragraph.layout(TEST_CANVAS_WIDTH);
        paragraph.paint(canvas.get(), (0.0, 0.0));

        let rect_height_style = RectHeightStyle::Tight;
        let rect_width_style = RectWidthStyle::Tight;

        let mut boxes = paragraph.get_rects_for_placeholders();
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 162.5, EPSILON50)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 162.5 + 55.0, EPSILON50)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 50.0, EPSILON100)
            );
        }

        boxes = paragraph.get_rects_for_range(5..6, rect_height_style, rect_width_style);
        canvas.draw_rects(Color::BLUE, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 135.5, EPSILON50)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 4.703, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 162.5, EPSILON50)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 42.065, EPSILON100)
            );
        }
    }
);

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L884-L1021 (chrome/m156)
def_paragraph_test!(SkParagraph_InlinePlaceholderBreakParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    let mut canvas = TestCanvas::new("SkParagraph_InlinePlaceholderBreakParagraph.png");
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = "012 34";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_max_lines(14);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);
    text_style.set_font_size(26.0);
    text_style.set_word_spacing(5.0);
    text_style.set_letter_spacing(1.0);
    text_style.set_decoration_type(TextDecoration::UNDERLINE);
    text_style.set_decoration_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);

    let placeholder1 = PlaceholderStyle::new(
        50.0,
        50.0,
        PlaceholderAlignment::Baseline,
        TextBaseline::Alphabetic,
        50.0,
    );
    let placeholder2 = PlaceholderStyle::new(
        25.0,
        25.0,
        PlaceholderAlignment::Baseline,
        TextBaseline::Alphabetic,
        12.5,
    );

    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder2);
    builder.add_placeholder(&placeholder1);
    builder.add_text(text);

    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder2); // 4 + 1
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder2); // 6 + 1
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder2); // 7 + 1

    builder.add_placeholder(&placeholder1);
    builder.add_text(text);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder2);

    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);

    builder.add_placeholder(&placeholder2);
    builder.add_placeholder(&placeholder1);

    builder.add_text(text);

    builder.add_placeholder(&placeholder2);

    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);

    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH - 100.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let rect_height_style = RectHeightStyle::Tight;
    let rect_width_style = RectWidthStyle::Tight;

    let mut boxes = paragraph.get_rects_for_range(0..3, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::RED, &boxes);
    reporter_assert!(reporter, boxes.len() == 1);

    boxes = paragraph.get_rects_for_range(175..176, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::GREEN, &boxes);
    reporter_assert!(reporter, boxes.len() == 1);
    if boxes.len() >= 1 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 31.695, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.top, 218.531, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.right, 47.292, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 249.0, EPSILON100)
        );
    }

    boxes = paragraph.get_rects_for_placeholders();
    canvas.draw_rects(Color::RED, &boxes);

    boxes = paragraph.get_rects_for_range(4..45, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::BLUE, &boxes);
    reporter_assert!(reporter, boxes.len() == 30);
    if boxes.len() >= 30 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 59.726, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.top, 26.378, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.right, 90.921, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 56.847, EPSILON100)
        );

        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[11].rect.left, 606.343, EPSILON20)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[11].rect.top, 38.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[11].rect.right, 631.343, EPSILON20)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[11].rect.bottom, 63.0, EPSILON100)
        );

        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[17].rect.left, 0.5, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[17].rect.top, 63.5, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[17].rect.right, 50.5, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[17].rect.bottom, 113.5, EPSILON100)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L1023-L1153 (chrome/m156)
def_paragraph_test!(SkParagraph_InlinePlaceholderGetRectsParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    let mut canvas = TestCanvas::new("SkParagraph_InlinePlaceholderGetRectsParagraph.png");
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = "012 34";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_max_lines(14);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);
    text_style.set_font_size(26.0);
    text_style.set_word_spacing(5.0);
    text_style.set_letter_spacing(1.0);
    text_style.set_decoration_type(TextDecoration::UNDERLINE);
    text_style.set_decoration_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);

    let placeholder1 = PlaceholderStyle::new(
        50.0,
        50.0,
        PlaceholderAlignment::Baseline,
        TextBaseline::Alphabetic,
        50.0,
    );
    let placeholder2 = PlaceholderStyle::new(
        5.0,
        20.0,
        PlaceholderAlignment::Baseline,
        TextBaseline::Alphabetic,
        10.0,
    );

    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder2); // 8 + 1
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder2); // 5 + 1
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder1); // 8 + 0

    builder.add_text(text);

    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder2);
    builder.add_placeholder(&placeholder2); // 1 + 2
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder2);
    builder.add_placeholder(&placeholder2); // 1 + 2

    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text);
    builder.add_text(text); // 11

    builder.add_placeholder(&placeholder2);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder2);
    builder.add_placeholder(&placeholder1);
    builder.add_placeholder(&placeholder2);

    builder.add_text(text);

    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let rect_height_style = RectHeightStyle::Max;
    let rect_width_style = RectWidthStyle::Tight;

    let mut boxes = paragraph.get_rects_for_placeholders();
    canvas.draw_rects(Color::RED, &boxes);

    reporter_assert!(reporter, boxes.len() == 34);
    if boxes.len() >= 34 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 90.921, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.top, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.right, 140.921, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 50.0, EPSILON100)
        );

        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[16].rect.left, 800.921, EPSILON20)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[16].rect.top, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[16].rect.right, 850.921, EPSILON20)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[16].rect.bottom, 50.0, EPSILON100)
        );

        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[33].rect.left, 503.382, EPSILON10)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[33].rect.top, 160.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[33].rect.right, 508.382, EPSILON10)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[33].rect.bottom, 180.0, EPSILON100)
        );
    }

    boxes = paragraph.get_rects_for_range(30..50, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::BLUE, &boxes);

    reporter_assert!(reporter, boxes.len() == 8);
    if boxes.len() >= 8 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 216.097, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.top, 60.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.right, 290.921, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 120.0, EPSILON100)
        );

        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.left, 290.921, EPSILON20)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.top, 60.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.right, 340.921, EPSILON20)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.bottom, 120.0, EPSILON100)
        );

        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[2].rect.left, 340.921, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[2].rect.top, 60.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[2].rect.right, 345.921, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[2].rect.bottom, 120.0, EPSILON100)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L1155-L1191 (chrome/m156)
def_paragraph_test!(SkParagraph_SimpleRedParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let text = "I am RED";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::RED);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    reporter_assert!(reporter, paragraph.unresolved_glyphs() == Some(0));

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.runs().len() == 1);
    reporter_assert!(reporter, impl_.styles().len() == 1); // paragraph style does not count
    reporter_assert!(reporter, impl_.styles()[0].style.equals(&text_style));

    let mut index: usize = 0;
    for line in impl_.lines() {
        line.scan_styles(
            impl_,
            StyleType::Decorations,
            &mut |_text_range, style, _context| {
                reporter_assert!(reporter, index == 0);
                reporter_assert!(reporter, style.color() == Color::RED);
                index += 1;
            },
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L1194-L1315 (chrome/m156)
def_paragraph_test!(SkParagraph_RainbowParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    let mut canvas = TestCanvas::new("SkParagraph_RainbowParagraph.png");
    skip_if_fonts_not_found!(reporter, font_collection);
    let text1 = "Red Roboto";
    let text2 = "big Greeen Default";
    let text3 = "Defcolor Homemade Apple";
    let text4 = "Small Blue Roboto";
    let text41 = "Small Blue ";
    let text5 = concat!(
        "Continue Last Style With lots of words to check if it overlaps ",
        "properly or not",
    );
    let text42 = concat!(
        "Roboto",
        "Continue Last Style With lots of words to check if it overlaps ",
        "properly or not",
    );

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_text_align(TextAlign::Left);
    paragraph_style.set_max_lines(2);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style1 = TextStyle::new();
    text_style1.set_font_families(&["Roboto"]);

    text_style1.set_color(Color::RED);
    builder.push_style(&text_style1);
    builder.add_text(text1);

    let mut text_style2 = TextStyle::new();
    text_style2.set_font_families(&["Roboto"]);
    text_style2.set_font_size(50.0);
    text_style2.set_font_style(FontStyle::new(
        Weight::MEDIUM,
        Width::NORMAL,
        Slant::Upright,
    ));
    text_style2.set_letter_spacing(10.0);
    text_style2.set_decoration_color(Color::BLACK);
    text_style2.set_decoration_type(
        TextDecoration::UNDERLINE | TextDecoration::OVERLINE | TextDecoration::LINE_THROUGH,
    );
    text_style2.set_word_spacing(30.0);
    text_style2.set_color(Color::GREEN);
    builder.push_style(&text_style2);
    builder.add_text(text2);

    let mut text_style3 = TextStyle::new();
    text_style3.set_font_families(&["Homemade Apple"]);
    text_style3.set_color(Color::BLACK);
    builder.push_style(&text_style3);
    builder.add_text(text3);

    let mut text_style4 = TextStyle::new();
    text_style4.set_font_families(&["Roboto"]);
    text_style4.set_font_size(14.0);
    text_style4.set_decoration_color(Color::BLACK);
    text_style4.set_decoration_type(
        TextDecoration::UNDERLINE | TextDecoration::OVERLINE | TextDecoration::LINE_THROUGH,
    );
    text_style4.set_color(Color::BLUE);
    builder.push_style(&text_style4);
    builder.add_text(text4);

    builder.add_text(text5);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(1000.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    reporter_assert!(reporter, paragraph.unresolved_glyphs() == Some(0));

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.runs().len() == 4);
    reporter_assert!(reporter, impl_.styles().len() == 4);
    reporter_assert!(reporter, impl_.lines().len() == 2);

    let rects = paragraph.get_rects_for_range(
        0..impl_.text().len(),
        RectHeightStyle::Max,
        RectWidthStyle::Tight,
    );
    canvas.draw_rects(Color::MAGENTA, &rects);

    let mut index: usize = 0;
    impl_.lines()[0].scan_styles(
        impl_,
        StyleType::AllAttributes,
        &mut |text_range, style, _context| {
            match index {
                0 => {
                    reporter_assert!(reporter, style.equals(&text_style1));
                    reporter_assert!(reporter, equal(impl_.text(), text_range, text1));
                }
                1 => {
                    reporter_assert!(reporter, style.equals(&text_style2));
                    reporter_assert!(reporter, equal(impl_.text(), text_range, text2));
                }
                2 => {
                    reporter_assert!(reporter, style.equals(&text_style3));
                    reporter_assert!(reporter, equal(impl_.text(), text_range, text3));
                }
                3 => {
                    reporter_assert!(reporter, style.equals(&text_style4));
                    reporter_assert!(reporter, equal(impl_.text(), text_range, text41));
                }
                _ => {
                    reporter_assert!(reporter, false);
                }
            }
            index += 1;
        },
    );
    impl_.lines()[1].scan_styles(
        impl_,
        StyleType::AllAttributes,
        &mut |text_range, style, _context| {
            match index {
                4 => {
                    reporter_assert!(reporter, style.equals(&text_style4));
                    reporter_assert!(reporter, equal(impl_.text(), text_range, text42));
                }
                _ => {
                    reporter_assert!(reporter, false);
                }
            }
            index += 1;
        },
    );
    reporter_assert!(reporter, index == 5);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L1317-L1355 (chrome/m156)
def_paragraph_test!(SkParagraph_DefaultStyleParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_DefaultStyleParagraph.png");
    let text = "No TextStyle! Uh Oh!";

    let mut paragraph_style = ParagraphStyle::new();
    let mut defaultStyle = TextStyle::new();
    defaultStyle.set_font_families(&["Roboto"]);
    defaultStyle.set_color(Color::BLACK);
    paragraph_style.set_text_style(&defaultStyle);
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder.add_text(text);

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (10.0, 15.0));

    reporter_assert!(reporter, paragraph.unresolved_glyphs() == Some(0));

    let impl_ = &paragraph;

    reporter_assert!(reporter, impl_.runs().len() == 1);
    reporter_assert!(reporter, impl_.styles().len() == 1);
    reporter_assert!(reporter, impl_.lines().len() == 1);

    let mut index: usize = 0;
    impl_.lines()[0].scan_styles(
        impl_,
        StyleType::AllAttributes,
        &mut |text_range, style, _context| {
            reporter_assert!(reporter, style.equals(paragraph_style.text_style()));
            reporter_assert!(reporter, equal(impl_.text(), text_range, text));
            index += 1;
        },
    );
    reporter_assert!(reporter, index == 1);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L1357-L1401 (chrome/m156)
def_paragraph_test!(SkParagraph_BoldParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_BoldParagraph.png");
    let text = "This is Red max bold text!";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::RED);
    text_style.set_font_size(60.0);
    text_style.set_letter_spacing(0.0);
    text_style.set_font_style(FontStyle::new(Weight::BLACK, Width::NORMAL, Slant::Upright));
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(VERY_LONG_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (10.0, 60.0));

    reporter_assert!(reporter, paragraph.unresolved_glyphs() == Some(0));

    let impl_ = &paragraph;

    reporter_assert!(reporter, impl_.runs().len() == 1);
    reporter_assert!(reporter, impl_.styles().len() == 1);
    reporter_assert!(reporter, impl_.lines().len() == 1);

    let mut index: usize = 0;
    impl_.lines()[0].scan_styles(
        impl_,
        StyleType::AllAttributes,
        &mut |text_range, style, _context| {
            reporter_assert!(reporter, style.equals(&text_style));
            reporter_assert!(reporter, equal(impl_.text(), text_range, text));
            index += 1;
        },
    );
    reporter_assert!(reporter, index == 1);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L1403-L1457 (chrome/m156)
def_paragraph_test!(SkParagraph_HeightOverrideParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_HeightOverrideParagraph.png");
    let text = "01234満毎冠行来昼本可\nabcd\n満毎冠行来昼本可";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_max_lines(10);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    text_style.set_height(3.6345);
    text_style.set_height_override(true);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(550.0);

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.runs().len() == 5);
    reporter_assert!(reporter, impl_.styles().len() == 1); // paragraph style does not count
    reporter_assert!(reporter, impl_.styles()[0].style.equals(&text_style));

    paragraph.paint(canvas.get(), (0.0, 0.0));

    let mut paint = Paint::default();
    paint.set_style(PaintStyle::Stroke);
    paint.set_anti_alias(true);
    paint.set_stroke_width(1.0);

    // Tests for GetRectsForRange()
    let rect_height_style = RectHeightStyle::IncludeLineSpacingMiddle;
    let rect_width_style = RectWidthStyle::Tight;
    paint.set_color(Color::RED);
    let mut boxes = paragraph.get_rects_for_range(0..0, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::RED, &boxes);
    reporter_assert!(reporter, boxes.len() == 0_usize);

    boxes = paragraph.get_rects_for_range(0..40, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::BLUE, &boxes);
    reporter_assert!(reporter, boxes.len() == 3_usize);
    if boxes.len() >= 3 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.left, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.top, 92.805, EPSILON5)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.right, 43.843, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.bottom, 165.495, EPSILON5)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L1459-L1516 (chrome/m156)
def_paragraph_test!(SkParagraph_BasicHalfLeading, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = "01234満毎冠行来昼本可\nabcd\n満毎冠行来昼本可";
    let len = text.len();

    let mut canvas = TestCanvas::new("SkParagraph_BasicHalfLeading.png");

    let paragraph_style = ParagraphStyle::new();
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    text_style.set_letter_spacing(0.0);
    text_style.set_word_spacing(0.0);
    text_style.set_height_override(true);
    text_style.set_height(3.6345);
    text_style.set_half_leading(true);

    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    builder.push_style(&text_style);
    builder.add_text(text);

    let mut paragraph = builder.build();
    paragraph.layout(550.0);

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.styles().len() == 1); // paragraph style does not count
    reporter_assert!(reporter, impl_.styles()[0].style.equals(&text_style));

    paragraph.paint(canvas.get(), (0.0, 0.0));

    let rect_width_style = RectWidthStyle::Tight;
    let boxes = paragraph.get_rects_for_range(0..len, RectHeightStyle::Tight, rect_width_style);
    let lineBoxes = paragraph.get_rects_for_range(0..len, RectHeightStyle::Max, rect_width_style);

    canvas.draw_rects(Color::BLUE, &boxes);
    reporter_assert!(reporter, boxes.len() == 3_usize);
    reporter_assert!(reporter, lineBoxes.len() == boxes.len());
    if boxes.len() >= 3 {
        let line_spacing1 = boxes[1].rect.top - boxes[0].rect.bottom;
        let line_spacing2 = boxes[2].rect.top - boxes[1].rect.bottom;

        // Uniform line spacing.
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(line_spacing1, line_spacing2)
        );

        // line spacing is distributed evenly over and under the text.
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(
                lineBoxes[0].rect.bottom - boxes[0].rect.bottom,
                boxes[0].rect.top - lineBoxes[0].rect.top
            )
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(
                lineBoxes[1].rect.bottom - boxes[1].rect.bottom,
                boxes[1].rect.top - lineBoxes[1].rect.top
            )
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(
                lineBoxes[2].rect.bottom - boxes[2].rect.bottom,
                boxes[2].rect.top - lineBoxes[2].rect.top
            )
        );

        // Half leading does not move the text horizontally.
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.left, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.right, 43.843, EPSILON100)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L1518-L1606 (chrome/m156)
def_paragraph_test!(SkParagraph_NearZeroHeightMixedDistribution, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = "Cookies need love";
    let len = text.len();

    let mut canvas = TestCanvas::new("SkParagraph_ZeroHeightHalfLeading.png");

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_height_behavior(TextHeightBehavior::All);
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    text_style.set_letter_spacing(0.0);
    text_style.set_word_spacing(0.0);
    text_style.set_height_override(true);
    text_style.set_height(0.001);

    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    // First run, half leading.
    text_style.set_half_leading(true);
    builder.push_style(&text_style);
    builder.add_text(text);

    // Second run, no half leading.
    text_style.set_half_leading(false);
    builder.push_style(&text_style);
    builder.add_text(text);

    let mut paragraph = builder.build();
    paragraph.layout(550.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.runs().len() == 2);
    reporter_assert!(reporter, impl_.styles().len() == 2); // paragraph style does not count
    reporter_assert!(reporter, impl_.lines().len() == 1_usize);

    let rect_width_style = RectWidthStyle::Tight;

    let boxes = paragraph.get_rects_for_range(0..len, RectHeightStyle::Tight, rect_width_style);
    let lineBoxes = paragraph.get_rects_for_range(0..len, RectHeightStyle::Max, rect_width_style);

    canvas.draw_rects(Color::BLUE, &boxes);
    reporter_assert!(reporter, boxes.len() == 1_usize);
    reporter_assert!(reporter, lineBoxes.len() == boxes.len());

    // From font metrics.
    let metricsAscent = -18.5546875;
    let metricsDescent = 4.8828125;

    // As the height multiplier converges to 0 (but not 0 since 0 is used as a
    // magic value to indicate there's no height multiplier), the `Run`s top
    // edge and bottom edge will converge to a horizontal line:
    // - When half leading is used the vertical line is roughly the center of
    //   of the glyphs in the run ((fontMetrics.descent - fontMetrics.ascent) / 2)
    // - When half leading is disabled the line is the alphabetic baseline.

    // Expected values in baseline coordinate space:
    let run1_ascent = (metricsAscent + metricsDescent) / 2.0;
    let run1_descent = (metricsAscent + metricsDescent) / 2.0;
    let run2_ascent = 0.0;
    let run2_descent = 0.0;
    let line_top = std_min(run1_ascent, run2_ascent);
    let line_bottom = std_max(run1_descent, run2_descent);

    // Expected glyph height in linebox coordinate space:
    let glyphs_top = metricsAscent - line_top;
    let glyphs_bottom = metricsDescent - line_top;

    // kTight reports the glyphs' bounding box_ in the linebox's coordinate
    // space.
    let actual_glyphs_top = boxes[0].rect.top - lineBoxes[0].rect.top;
    let actual_glyphs_bottom = boxes[0].rect.bottom - lineBoxes[0].rect.top;

    // Use a relatively large epsilon since the heightMultiplier is not actually
    // 0.
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(glyphs_top, actual_glyphs_top, EPSILON20)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(glyphs_bottom, actual_glyphs_bottom, EPSILON20)
    );

    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(lineBoxes[0].rect.height(), line_bottom - line_top, EPSILON2)
    );
    reporter_assert!(reporter, lineBoxes[0].rect.height() > 1.0);

    // Half leading does not move the text horizontally.
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(boxes[0].rect.left, 0.0, EPSILON100)
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L1608-L1672 (chrome/m156)
def_paragraph_test!(SkParagraph_StrutHalfLeadingSimple, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = "A";
    let len = text.len();

    let mut canvas = TestCanvas::new("SkParagraph_StrutHalfLeading.png");

    let mut paragraph_style = ParagraphStyle::new();
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(100.0);
    text_style.set_color(Color::BLACK);
    text_style.set_letter_spacing(0.0);
    text_style.set_word_spacing(0.0);
    text_style.set_height_override(true);
    text_style.set_height(2.0);
    text_style.set_half_leading(true);

    let mut strut_style = StrutStyle::new();
    strut_style.set_font_families(&["Roboto"]);
    strut_style.set_font_size(100.0);
    strut_style.set_height_override(true);
    strut_style.set_height(2.0);
    strut_style.set_half_leading(true);
    strut_style.set_strut_enabled(true);
    strut_style.set_force_strut_height(true);

    paragraph_style.set_strut_style(strut_style.clone());
    paragraph_style.set_text_style(&text_style);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    builder.push_style(&text_style);
    builder.add_text(text);

    let mut paragraph = builder.build();
    paragraph.layout(550.0);

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.styles().len() == 1); // paragraph style does not count

    paragraph.paint(canvas.get(), (0.0, 0.0));

    let rect_width_style = RectWidthStyle::Tight;
    let boxes = paragraph.get_rects_for_range(0..len, RectHeightStyle::Tight, rect_width_style);
    let lineBoxes = paragraph.get_rects_for_range(0..len, RectHeightStyle::Max, rect_width_style);

    canvas.draw_rects(Color::BLUE, &boxes);
    reporter_assert!(reporter, lineBoxes.len() == boxes.len());
    if lineBoxes.len() >= 1 && boxes.len() >= 1 {
        // line spacing is distributed evenly over and under the text.
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(
                lineBoxes[0].rect.bottom - boxes[0].rect.bottom,
                boxes[0].rect.top - lineBoxes[0].rect.top
            )
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(lineBoxes[0].rect.top, 0.0)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(lineBoxes[0].rect.left, 0.0)
        );

        let lineMetrics = paragraph.get_line_metrics();
        let firstLine = &lineMetrics[0];
        reporter_assert!(reporter, sk_scalar_nearly_equal!(firstLine.height, 200.0));

        // Half leading does not move the text horizontally.
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 0.0, EPSILON100)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L1674-L1740 (chrome/m156)
def_paragraph_test!(SkParagraph_StrutHalfLeadingMultiline, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = "01234満毎冠行来昼本可\nabcd\n満毎冠行来昼本可";
    let len = text.len();

    let mut canvas = TestCanvas::new("SkParagraph_StrutHalfLeading.png");

    let mut paragraph_style = ParagraphStyle::new();
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    text_style.set_letter_spacing(0.0);
    text_style.set_word_spacing(0.0);
    text_style.set_height_override(true);
    text_style.set_height(3.0);
    text_style.set_half_leading(true);

    let mut strut_style = StrutStyle::new();
    strut_style.set_font_families(&["Roboto"]);
    strut_style.set_font_size(20.0);
    strut_style.set_height_override(true);
    strut_style.set_height(3.0);
    strut_style.set_half_leading(true);
    strut_style.set_strut_enabled(true);
    strut_style.set_force_strut_height(true);

    paragraph_style.set_strut_style(strut_style.clone());
    paragraph_style.set_text_style(&text_style);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    builder.push_style(&text_style);
    builder.add_text(text);

    let mut paragraph = builder.build();
    paragraph.layout(550.0);

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.styles().len() == 1); // paragraph style does not count

    paragraph.paint(canvas.get(), (0.0, 0.0));

    let rect_width_style = RectWidthStyle::Tight;
    let boxes = paragraph.get_rects_for_range(0..len, RectHeightStyle::Tight, rect_width_style);
    let lineBoxes = paragraph.get_rects_for_range(0..len, RectHeightStyle::Max, rect_width_style);

    canvas.draw_rects(Color::BLUE, &boxes);
    reporter_assert!(reporter, boxes.len() == 3_usize);
    reporter_assert!(reporter, lineBoxes.len() == boxes.len());
    if boxes.len() >= 3 {
        let line_spacing1 = boxes[1].rect.top - boxes[0].rect.bottom;
        let line_spacing2 = boxes[2].rect.top - boxes[1].rect.bottom;

        // Uniform line spacing.
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(line_spacing1, line_spacing2)
        );

        // line spacing is distributed evenly over and under the text.
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(
                lineBoxes[0].rect.bottom - boxes[0].rect.bottom,
                boxes[0].rect.top - lineBoxes[0].rect.top
            )
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(
                lineBoxes[1].rect.bottom - boxes[1].rect.bottom,
                boxes[1].rect.top - lineBoxes[1].rect.top
            )
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(
                lineBoxes[2].rect.bottom - boxes[2].rect.bottom,
                boxes[2].rect.top - lineBoxes[2].rect.top
            )
        );

        // Half leading does not move the text horizontally.
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.left, 0.0, EPSILON100)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L1742-L1804 (chrome/m156)
def_paragraph_test!(SkParagraph_TrimLeadingDistribution, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = "01234満毎冠行来昼本可\nabcd\n満毎冠行来昼本可";
    let len = text.len();

    let mut canvas = TestCanvas::new("SkParagraph_TrimHalfLeading.png");

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_height_behavior(TextHeightBehavior::DisableAll);
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    text_style.set_letter_spacing(0.0);
    text_style.set_word_spacing(0.0);
    text_style.set_height_override(true);
    text_style.set_height(3.6345);
    text_style.set_half_leading(true);

    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    builder.push_style(&text_style);
    builder.add_text(text);

    let mut paragraph = builder.build();
    paragraph.layout(550.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let rect_width_style = RectWidthStyle::Tight;

    let boxes = paragraph.get_rects_for_range(0..len, RectHeightStyle::Tight, rect_width_style);
    let lineBoxes = paragraph.get_rects_for_range(0..len, RectHeightStyle::Max, rect_width_style);

    canvas.draw_rects(Color::BLUE, &boxes);
    reporter_assert!(reporter, boxes.len() == 3_usize);
    reporter_assert!(reporter, lineBoxes.len() == boxes.len());
    if boxes.len() >= 3 {
        let line_spacing1 = boxes[1].rect.top - boxes[0].rect.bottom;
        let line_spacing2 = boxes[2].rect.top - boxes[1].rect.bottom;

        // Uniform line spacing. The delta is introduced by the height rounding.
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(line_spacing1, line_spacing2, 1.0)
        );

        // Trim the first line's top leading.
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(lineBoxes[0].rect.top, boxes[0].rect.top)
        );
        // Trim the last line's bottom leading.
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(lineBoxes[2].rect.bottom, boxes[2].rect.bottom)
        );

        let halfLeading = lineBoxes[0].rect.bottom - boxes[0].rect.bottom;
        // Large epsilon because of rounding.
        let epsilon = EPSILON10;
        // line spacing is distributed evenly over and under the text.
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(
                boxes[1].rect.top - lineBoxes[1].rect.top,
                halfLeading,
                epsilon
            )
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(lineBoxes[1].rect.bottom - boxes[1].rect.bottom, halfLeading)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(
                boxes[2].rect.top - lineBoxes[2].rect.top,
                halfLeading,
                epsilon
            )
        );

        // Half leading does not move the text horizontally.
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.left, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.right, 43.843, EPSILON100)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L1806-L1888 (chrome/m156)
def_paragraph_test!(SkParagraph_LeftAlignParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_LeftAlignParagraph.png");
    let text = concat!(
        "This is a very long sentence to test if the text will properly wrap ",
        "around and go to the next line. Sometimes, short sentence. Longer ",
        "sentences are okay too because they are nessecary. Very short. ",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod ",
        "tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim ",
        "veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea ",
        "commodo consequat. Duis aute irure dolor in reprehenderit in voluptate ",
        "velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint ",
        "occaecat cupidatat non proident, sunt in culpa qui officia deserunt ",
        "mollit anim id est laborum. ",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod ",
        "tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim ",
        "veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea ",
        "commodo consequat. Duis aute irure dolor in reprehenderit in voluptate ",
        "velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint ",
        "occaecat cupidatat non proident, sunt in culpa qui officia deserunt ",
        "mollit anim id est laborum.",
    );

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(14);
    paragraph_style.set_text_align(TextAlign::Left);
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(26.0);
    text_style.set_letter_spacing(1.0);
    text_style.set_word_spacing(5.0);
    text_style.set_color(Color::BLACK);
    text_style.set_height(1.0);
    text_style.set_decoration_type(TextDecoration::UNDERLINE);
    text_style.set_decoration_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH - 100.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;

    reporter_assert!(reporter, impl_.text().len() == text.len());
    reporter_assert!(reporter, impl_.runs().len() == 1);
    reporter_assert!(reporter, impl_.styles().len() == 1);
    reporter_assert!(reporter, impl_.styles()[0].style.equals(&text_style));
    reporter_assert!(
        reporter,
        impl_.lines().len() == paragraph_style.max_lines_raw()
    );

    let mut expected_y: f64 = 0.0;
    let epsilon: f64 = 0.01;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[0].baseline(), 24.121, epsilon)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[0].offset().y, expected_y, epsilon)
    );
    expected_y += 30.0;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[1].offset().y, expected_y, epsilon)
    );
    expected_y += 30.0;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[2].offset().y, expected_y, epsilon)
    );
    expected_y += 30.0;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[3].offset().y, expected_y, epsilon)
    );
    expected_y += 30.0 * 10.0;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[13].offset().y, expected_y, epsilon)
    );

    reporter_assert!(
        reporter,
        paragraph_style.text_align() == impl_.paragraph_style().text_align()
    );

    // Tests for GetGlyphPositionAtCoordinate()
    reporter_assert!(
        reporter,
        impl_.get_glyph_position_at_coordinate((0.0, 0.0)).position == 0
    );
    reporter_assert!(
        reporter,
        impl_.get_glyph_position_at_coordinate((1.0, 1.0)).position == 0
    );
    reporter_assert!(
        reporter,
        impl_.get_glyph_position_at_coordinate((1.0, 35.0)).position == 68
    );
    reporter_assert!(
        reporter,
        impl_.get_glyph_position_at_coordinate((1.0, 70.0)).position == 134
    );
    reporter_assert!(
        reporter,
        impl_
            .get_glyph_position_at_coordinate((2000.0, 35.0))
            .position
            == 134
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L1890-L1975 (chrome/m156)
def_paragraph_test!(SkParagraph_RightAlignParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_RightAlignParagraph.png");
    let text = concat!(
        "This is a very long sentence to test if the text will properly wrap ",
        "around and go to the next line. Sometimes, short sentence. Longer ",
        "sentences are okay too because they are nessecary. Very short. ",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod ",
        "tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim ",
        "veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea ",
        "commodo consequat. Duis aute irure dolor in reprehenderit in voluptate ",
        "velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint ",
        "occaecat cupidatat non proident, sunt in culpa qui officia deserunt ",
        "mollit anim id est laborum. ",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod ",
        "tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim ",
        "veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea ",
        "commodo consequat. Duis aute irure dolor in reprehenderit in voluptate ",
        "velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint ",
        "occaecat cupidatat non proident, sunt in culpa qui officia deserunt ",
        "mollit anim id est laborum.",
    );

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(14);
    paragraph_style.set_text_align(TextAlign::Right);
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(26.0);
    text_style.set_letter_spacing(1.0);
    text_style.set_word_spacing(5.0);
    text_style.set_color(Color::BLACK);
    text_style.set_height(1.0);
    text_style.set_decoration_type(TextDecoration::UNDERLINE);
    text_style.set_decoration_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH - 100.0);

    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;

    reporter_assert!(reporter, impl_.runs().len() == 1);
    reporter_assert!(reporter, impl_.styles().len() == 1);
    reporter_assert!(reporter, impl_.styles()[0].style.equals(&text_style));
    reporter_assert!(
        reporter,
        impl_.lines().len() == paragraph_style.max_lines_raw()
    );

    let mut expected_y: f64 = 0.0;
    let epsilon: f64 = 0.01;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[0].baseline(), 24.121, epsilon)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[0].offset().y, expected_y, epsilon)
    );
    expected_y += 30.0;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[1].offset().y, expected_y, epsilon)
    );
    expected_y += 30.0;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[2].offset().y, expected_y, epsilon)
    );
    expected_y += 30.0;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[3].offset().y, expected_y, epsilon)
    );
    expected_y += 30.0 * 10.0;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[13].offset().y, expected_y, epsilon)
    );

    let calculate = |line: &TextLine| -> scalar {
        return TEST_CANVAS_WIDTH - 100.0 - line.offset().x - line.width();
    };

    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(calculate(&impl_.lines()[0]), 0.0, epsilon)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(calculate(&impl_.lines()[1]), 0.0, epsilon)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(calculate(&impl_.lines()[2]), 0.0, epsilon)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(calculate(&impl_.lines()[3]), 0.0, epsilon)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(calculate(&impl_.lines()[13]), 0.0, epsilon)
    );

    reporter_assert!(
        reporter,
        paragraph_style.text_align() == impl_.paragraph_style().text_align()
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L1977-L2062 (chrome/m156)
def_paragraph_test!(SkParagraph_CenterAlignParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_CenterAlignParagraph.png");
    let text = concat!(
        "This is a very long sentence to test if the text will properly wrap ",
        "around and go to the next line. Sometimes, short sentence. Longer ",
        "sentences are okay too because they are nessecary. Very short. ",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod ",
        "tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim ",
        "veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea ",
        "commodo consequat. Duis aute irure dolor in reprehenderit in voluptate ",
        "velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint ",
        "occaecat cupidatat non proident, sunt in culpa qui officia deserunt ",
        "mollit anim id est laborum. ",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod ",
        "tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim ",
        "veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea ",
        "commodo consequat. Duis aute irure dolor in reprehenderit in voluptate ",
        "velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint ",
        "occaecat cupidatat non proident, sunt in culpa qui officia deserunt ",
        "mollit anim id est laborum.",
    );

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(14);
    paragraph_style.set_text_align(TextAlign::Center);
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(26.0);
    text_style.set_letter_spacing(1.0);
    text_style.set_word_spacing(5.0);
    text_style.set_color(Color::BLACK);
    text_style.set_height(1.0);
    text_style.set_decoration_type(TextDecoration::UNDERLINE);
    text_style.set_decoration_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH - 100.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;

    reporter_assert!(reporter, impl_.text().len() == text.len());
    reporter_assert!(reporter, impl_.runs().len() == 1);
    reporter_assert!(reporter, impl_.styles().len() == 1);
    reporter_assert!(reporter, impl_.styles()[0].style.equals(&text_style));
    reporter_assert!(
        reporter,
        impl_.lines().len() == paragraph_style.max_lines_raw()
    );

    let mut expected_y: f64 = 0.0;
    let epsilon: f64 = 0.01;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[0].baseline(), 24.121, epsilon)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[0].offset().y, expected_y, epsilon)
    );
    expected_y += 30.0;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[1].offset().y, expected_y, epsilon)
    );
    expected_y += 30.0;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[2].offset().y, expected_y, epsilon)
    );
    expected_y += 30.0;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[3].offset().y, expected_y, epsilon)
    );
    expected_y += 30.0 * 10.0;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[13].offset().y, expected_y, epsilon)
    );

    let calculate = |line: &TextLine| -> scalar {
        return TEST_CANVAS_WIDTH - 100.0 - (line.offset().x * 2.0 + line.width());
    };

    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(calculate(&impl_.lines()[0]), 0.0, epsilon)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(calculate(&impl_.lines()[1]), 0.0, epsilon)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(calculate(&impl_.lines()[2]), 0.0, epsilon)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(calculate(&impl_.lines()[3]), 0.0, epsilon)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(calculate(&impl_.lines()[13]), 0.0, epsilon)
    );

    reporter_assert!(
        reporter,
        paragraph_style.text_align() == impl_.paragraph_style().text_align()
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L2064-L2149 (chrome/m156)
def_paragraph_test!(SkParagraph_JustifyAlignParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_JustifyAlignParagraph.png");
    let text = concat!(
        "This is a very long sentence to test if the text will properly wrap ",
        "around and go to the next line. Sometimes, short sentence. Longer ",
        "sentences are okay too because they are nessecary. Very short. ",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod ",
        "tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim ",
        "veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea ",
        "commodo consequat. Duis aute irure dolor in reprehenderit in voluptate ",
        "velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint ",
        "occaecat cupidatat non proident, sunt in culpa qui officia deserunt ",
        "mollit anim id est laborum. ",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod ",
        "tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim ",
        "veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea ",
        "commodo consequat. Duis aute irure dolor in reprehenderit in voluptate ",
        "velit esse cillum dolore eu fugiat.",
    );

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(14);
    paragraph_style.set_text_align(TextAlign::Justify);
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(26.0);
    text_style.set_letter_spacing(0.0);
    text_style.set_word_spacing(5.0);
    text_style.set_color(Color::BLACK);
    text_style.set_height(1.0);
    text_style.set_decoration_type(TextDecoration::UNDERLINE);
    text_style.set_decoration_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH - 100.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let rect_height_style = RectHeightStyle::Max;
    let rect_width_style = RectWidthStyle::Tight;
    let boxes = paragraph.get_rects_for_range(0..100, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::RED, &boxes);

    let impl_ = &paragraph;

    reporter_assert!(reporter, impl_.text().len() == text.len());
    reporter_assert!(reporter, impl_.runs().len() == 1);
    reporter_assert!(reporter, impl_.styles().len() == 1);
    reporter_assert!(reporter, impl_.styles()[0].style.equals(&text_style));

    let mut expected_y: f64 = 0.0;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[0].baseline(), 24.121, EPSILON100)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[0].offset().y, expected_y, EPSILON100)
    );
    expected_y += 30.0;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[1].offset().y, expected_y, EPSILON100)
    );
    expected_y += 30.0;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[2].offset().y, expected_y, EPSILON100)
    );
    expected_y += 30.0;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[3].offset().y, expected_y, EPSILON100)
    );
    expected_y += 30.0 * 9.0;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(impl_.lines()[12].offset().y, expected_y, EPSILON100)
    );

    let calculate = |line: &TextLine| -> scalar {
        return line.offset().x;
    };

    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(calculate(&impl_.lines()[0]), 0.0, EPSILON100)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(calculate(&impl_.lines()[1]), 0.0, EPSILON100)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(calculate(&impl_.lines()[2]), 0.0, EPSILON100)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(calculate(&impl_.lines()[3]), 0.0, EPSILON100)
    );

    reporter_assert!(
        reporter,
        paragraph_style.text_align() == impl_.paragraph_style().text_align()
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L2152-L2227 (chrome/m156)
def_paragraph_test!(SkParagraph_JustifyRTL, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_JustifyRTL.png");
    let text = concat!(
        "אאא בּבּבּבּ אאאא בּבּ אאא בּבּבּ אאאאא בּבּבּבּ אאאא בּבּבּבּבּ ",
        "אאאאא בּבּבּבּבּ אאאבּבּבּבּבּבּאאאאא בּבּבּבּבּבּאאאאאבּבּבּבּבּבּ אאאאא בּבּבּבּבּ ",
        "אאאאא בּבּבּבּבּבּ אאאאא בּבּבּבּבּבּ אאאאא בּבּבּבּבּבּ אאאאא בּבּבּבּבּבּ ",
        "אאאאאבּבּבּבּבּבּאאאאאבּבּבּבּבּבּאאאאאבּבּבּבּבּבּ ",
        "אאאאא בּבּבּבּבּבּ",
    );

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(14);
    paragraph_style.set_text_align(TextAlign::Justify);
    paragraph_style.set_text_direction(TextDirection::RTL);
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(26.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH - 100.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;

    let calculate = |line: &TextLine| -> scalar {
        return TEST_CANVAS_WIDTH - 100.0 - line.width();
    };
    for line in impl_.lines() {
        if std::ptr::eq(line, impl_.lines().last().unwrap())
            || std::ptr::eq(line, &impl_.lines()[impl_.lines().len() - 2])
        {
            // Second-last line will be also right-aligned because it is only one cluster
            reporter_assert!(reporter, calculate(line) > EPSILON100);
            reporter_assert!(reporter, line.offset().x > EPSILON100);
        } else {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(calculate(line), 0.0, EPSILON100)
            );
        }
    }

    // Just make sure the the text is actually RTL
    for run in impl_.runs() {
        reporter_assert!(reporter, !run.left_to_right());
    }

    // Tests for GetRectsForRange()
    let rect_height_style = RectHeightStyle::Max;
    let rect_width_style = RectWidthStyle::Tight;
    let mut boxes = paragraph.get_rects_for_range(0..100, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::RED, &boxes);
    reporter_assert!(reporter, boxes.len() == 3);

    boxes = paragraph.get_rects_for_range(226..278, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::YELLOW, &boxes);
    reporter_assert!(reporter, boxes.len() == 1);

    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(boxes[0].rect.left, 16.0, EPSILON100)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(boxes[0].rect.top, 130.0, EPSILON100)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(boxes[0].rect.right, 900.0, EPSILON100)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(boxes[0].rect.bottom, 156.0, EPSILON100)
    );

    boxes = paragraph.get_rects_for_range(292..296, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::BLUE, &boxes);
    reporter_assert!(reporter, boxes.len() == 1);

    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(boxes[0].rect.left, 588.0, EPSILON100)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(boxes[0].rect.top, 156.0, EPSILON100)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(boxes[0].rect.right, 640.0, EPSILON100)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(boxes[0].rect.bottom, 182.0, EPSILON100)
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L2229-L2306 (chrome/m156)
def_paragraph_test!(SkParagraph_JustifyRTLNewLine, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_JustifyRTLNewLine.png");
    let text = concat!(
        "אאא בּבּבּבּ אאאא\nבּבּ אאא בּבּבּ אאאאא בּבּבּבּ אאאא בּבּבּבּבּ ",
        "אאאאא בּבּבּבּבּ אאאבּבּבּבּבּבּאאאאא בּבּבּבּבּבּאאאאאבּבּבּבּבּבּ אאאאא בּבּבּבּבּ ",
        "אאאאא בּבּבּבּבּבּ אאאאא בּבּבּבּבּבּ אאאאא בּבּבּבּבּבּ אאאאא בּבּבּבּבּבּ אאאאא בּבּבּבּבּבּ",
    );

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(14);
    paragraph_style.set_text_align(TextAlign::Justify);
    paragraph_style.set_text_direction(TextDirection::RTL);
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(26.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH - 100.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;

    let mut paint = Paint::default();
    paint.set_style(PaintStyle::Stroke);
    paint.set_anti_alias(true);
    paint.set_stroke_width(1.0);

    // Tests for GetRectsForRange()
    let rect_height_style = RectHeightStyle::Max;
    let rect_width_style = RectWidthStyle::Tight;
    paint.set_color(Color::RED);
    let mut boxes = paragraph.get_rects_for_range(0..30, rect_height_style, rect_width_style);
    for i in 0..boxes.len() {
        canvas.get().draw_rect(boxes[i].rect, &paint);
    }
    reporter_assert!(reporter, boxes.len() == 2_usize);
    if boxes.len() >= 2 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 562.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.top, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.right, 900.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 26.0, EPSILON100)
        );
    }

    paint.set_color(Color::BLUE);
    boxes = paragraph.get_rects_for_range(240..250, rect_height_style, rect_width_style);
    for i in 0..boxes.len() {
        canvas.get().draw_rect(boxes[i].rect, &paint);
    }
    reporter_assert!(reporter, boxes.len() == 1_usize);
    if boxes.len() >= 1 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 68.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.top, 130.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.right, 120.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 156.0, EPSILON100)
        );
    }

    // All lines should be justified to the width of the paragraph
    // except for #0 (new line) and #5 (the last one)
    for (num, line) in impl_.lines().iter().enumerate() {
        if num == 0 || num == 5 {
            reporter_assert!(reporter, line.width() < TEST_CANVAS_WIDTH - 100.0);
        } else {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(line.width(), TEST_CANVAS_WIDTH - 100.0, EPSILON100),
                "#{}: {} <= {}\n",
                num,
                line.width(),
                TEST_CANVAS_WIDTH - 100.0
            );
        }
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L2308-L2349 (chrome/m156)
def_paragraph_test!(SkParagraph_LeadingSpaceRTL, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_LeadingSpaceRTL.png");

    let text = " leading space";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(14);
    paragraph_style.set_text_align(TextAlign::Justify);
    paragraph_style.set_text_direction(TextDirection::RTL);
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(26.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH - 100.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let mut paint = Paint::default();
    paint.set_style(PaintStyle::Stroke);
    paint.set_anti_alias(true);
    paint.set_stroke_width(1.0);

    // Tests for GetRectsForRange()
    let rect_height_style = RectHeightStyle::Max;
    let rect_width_style = RectWidthStyle::Tight;
    paint.set_color(Color::RED);
    let boxes = paragraph.get_rects_for_range(0..100, rect_height_style, rect_width_style);
    for i in 0..boxes.len() {
        canvas.get().draw_rect(boxes[i].rect, &paint);
    }
    reporter_assert!(reporter, boxes.len() == 2_usize);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L2351-L2468 (chrome/m156)
def_paragraph_test!(SkParagraph_DecorationsParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_DecorationsParagraph.png");
    let text1 = "This text should be";
    let text2 = " decorated even when";
    let text3 = " wrapped around to";
    let text4 = " the next line.";
    let text5 = " Otherwise, bad things happen.";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(14);
    paragraph_style.set_text_align(TextAlign::Left);
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(26.0);
    text_style.set_letter_spacing(0.0);
    text_style.set_word_spacing(5.0);
    text_style.set_color(Color::BLACK);
    text_style.set_height(2.0);
    text_style.set_decoration_type(TextDecoration::UNDERLINE);
    text_style.set_decoration_color(Color::BLACK);
    text_style.set_decoration_type(
        TextDecoration::UNDERLINE | TextDecoration::OVERLINE | TextDecoration::LINE_THROUGH,
    );
    text_style.set_decoration_style(TextDecorationStyle::Solid);
    text_style.set_decoration_color(Color::BLACK);
    text_style.set_decoration_thickness_multiplier(2.0);
    builder.push_style(&text_style);
    builder.add_text(text1);

    text_style.set_decoration_style(TextDecorationStyle::Double);
    text_style.set_decoration_color(Color::BLUE);
    text_style.set_decoration_thickness_multiplier(1.0);
    builder.push_style(&text_style);
    builder.add_text(text2);

    text_style.set_decoration_style(TextDecorationStyle::Dotted);
    text_style.set_decoration_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text3);

    text_style.set_decoration_style(TextDecorationStyle::Dashed);
    text_style.set_decoration_color(Color::BLACK);
    text_style.set_decoration_thickness_multiplier(3.0);
    builder.push_style(&text_style);
    builder.add_text(text4);

    text_style.set_decoration_style(TextDecorationStyle::Wavy);
    text_style.set_decoration_color(Color::RED);
    text_style.set_decoration_thickness_multiplier(1.0);
    builder.push_style(&text_style);
    builder.add_text(text5);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH - 100.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;

    let mut index: usize = 0;
    for line in impl_.lines() {
        line.scan_styles(
            impl_,
            StyleType::Decorations,
            &mut |_text_range, style, _context| {
                let decoration = TextDecoration::UNDERLINE
                    | TextDecoration::OVERLINE
                    | TextDecoration::LINE_THROUGH;
                reporter_assert!(reporter, style.decoration_type() == decoration);
                match index {
                    0 => {
                        reporter_assert!(
                            reporter,
                            style.decoration_style() == TextDecorationStyle::Solid
                        );
                        reporter_assert!(reporter, style.decoration_color() == Color::BLACK);
                        reporter_assert!(reporter, style.decoration_thickness_multiplier() == 2.0);
                    }
                    1 => {
                        reporter_assert!(
                            reporter,
                            style.decoration_style() == TextDecorationStyle::Double
                        );
                        reporter_assert!(reporter, style.decoration_color() == Color::BLUE);
                        reporter_assert!(reporter, style.decoration_thickness_multiplier() == 1.0);
                    }
                    2 => {
                        reporter_assert!(
                            reporter,
                            style.decoration_style() == TextDecorationStyle::Dotted
                        );
                        reporter_assert!(reporter, style.decoration_color() == Color::BLACK);
                        reporter_assert!(reporter, style.decoration_thickness_multiplier() == 1.0);
                    }
                    3 => {}
                    4 => {
                        reporter_assert!(
                            reporter,
                            style.decoration_style() == TextDecorationStyle::Dashed
                        );
                        reporter_assert!(reporter, style.decoration_color() == Color::BLACK);
                        reporter_assert!(reporter, style.decoration_thickness_multiplier() == 3.0);
                    }
                    5 => {
                        reporter_assert!(
                            reporter,
                            style.decoration_style() == TextDecorationStyle::Wavy
                        );
                        reporter_assert!(reporter, style.decoration_color() == Color::RED);
                        reporter_assert!(reporter, style.decoration_thickness_multiplier() == 1.0);
                    }
                    _ => {
                        reporter_assert!(reporter, false);
                    }
                }
                index += 1;
            },
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L2472-L2534 (chrome/m156)
def_paragraph_test!(SkParagraph_ItalicsParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_ItalicsParagraph.png");
    let text1 = "No italic ";
    let text2 = "Yes Italic ";
    let text3 = "No Italic again.";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(10.0);
    text_style.set_color(Color::RED);
    builder.push_style(&text_style);
    builder.add_text(text1);

    text_style.set_font_style(FontStyle::new(Weight::NORMAL, Width::NORMAL, Slant::Italic));
    builder.push_style(&text_style);
    builder.add_text(text2);
    builder.pop();
    builder.add_text(text3);

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;

    reporter_assert!(reporter, impl_.runs().len() == 3);
    reporter_assert!(reporter, impl_.styles().len() == 3);
    reporter_assert!(reporter, impl_.lines().len() == 1);
    let line = &impl_.lines()[0];
    let mut index: usize = 0;
    line.scan_styles(
        impl_,
        StyleType::Foreground,
        &mut |_text_range, style, _context| {
            match index {
                0 => {
                    reporter_assert!(reporter, style.font_style().slant() == Slant::Upright);
                }
                1 => {
                    reporter_assert!(reporter, style.font_style().slant() == Slant::Italic);
                }
                2 => {
                    reporter_assert!(reporter, style.font_style().slant() == Slant::Upright);
                }
                _ => {
                    reporter_assert!(reporter, false);
                }
            }
            index += 1;
        },
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L2536-L2584 (chrome/m156)
def_paragraph_test!(SkParagraph_ChineseParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_ChineseParagraph.png");
    let text = concat!(
        "     左線読設重説切後碁給能上目秘使約。満毎冠行来昼本可必図将発確年。今属場育",
        "図情闘陰野高備込制詩西校客。審対江置講今固残必託地集済決維駆年策。立得庭",
        "際輝求佐抗蒼提夜合逃表。注統天言件自謙雅載報紙喪。作画稿愛器灯女書利変探",
        "訃第金線朝開化建。子戦年帝励害表月幕株漠新期刊人秘。図的海力生禁挙保天戦",
        "聞条年所在口。",
    );

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(14);
    paragraph_style.set_text_align(TextAlign::Justify);
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let decoration =
        TextDecoration::UNDERLINE | TextDecoration::OVERLINE | TextDecoration::LINE_THROUGH;

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Source Han Serif CN"]);
    text_style.set_font_size(35.0);
    text_style.set_color(Color::BLACK);
    text_style.set_letter_spacing(2.0);
    text_style.set_height(1.0);
    text_style.set_decoration_type(decoration);
    text_style.set_decoration_color(Color::BLACK);
    text_style.set_decoration_style(TextDecorationStyle::Solid);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH - 100.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    reporter_assert!(reporter, paragraph.unresolved_glyphs() == Some(0));

    let impl_ = &paragraph;

    reporter_assert!(reporter, impl_.runs().len() == 1);
    reporter_assert!(reporter, impl_.lines().len() == 7);
    reporter_assert!(reporter, impl_.styles().len() == 1);
    if impl_.styles().len() >= 1 {
        reporter_assert!(reporter, impl_.styles()[0].style.equals(&text_style));
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L2587-L2631 (chrome/m156)
def_paragraph_test!(SkParagraph_ArabicParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_ArabicParagraph.png");
    let text = concat!(
        "من أسر وإعلان الخاصّة وهولندا،, عل قائمة الضغوط بالمطالبة تلك. الصفحة ",
        "بمباركة التقليدية قام عن. تصفح",
    );

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(14);
    paragraph_style.set_text_align(TextAlign::Justify);
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let decoration =
        TextDecoration::UNDERLINE | TextDecoration::OVERLINE | TextDecoration::LINE_THROUGH;

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Katibeh"]);
    text_style.set_font_size(35.0);
    text_style.set_color(Color::BLACK);
    text_style.set_letter_spacing(2.0);
    text_style.set_decoration_type(decoration);
    text_style.set_decoration_color(Color::BLACK);
    text_style.set_decoration_style(TextDecorationStyle::Solid);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH - 100.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    reporter_assert!(reporter, paragraph.unresolved_glyphs() == Some(0));

    let impl_ = &paragraph;

    reporter_assert!(reporter, impl_.runs().len() == 1);
    reporter_assert!(reporter, impl_.lines().len() == 2);
    reporter_assert!(reporter, impl_.styles().len() == 1);
    if impl_.styles().len() >= 1 {
        reporter_assert!(reporter, impl_.styles()[0].style.equals(&text_style));
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L2634-L2680 (chrome/m156)
def_paragraph_test!(SkParagraph_ArabicRectsParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_ArabicRectsParagraph.png");
    let text = "بمباركة التقليدية قام عن. تصفح يد    ";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_max_lines(14);
    paragraph_style.set_text_align(TextAlign::Right);
    paragraph_style.set_text_direction(TextDirection::RTL);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Noto Naskh Arabic"]);
    text_style.set_font_size(26.0);
    text_style.set_word_spacing(5.0);
    text_style.set_color(Color::BLACK);
    text_style.set_decoration_type(TextDecoration::UNDERLINE);
    text_style.set_decoration_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH - 100.0);

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.runs().len() == 1);

    paragraph.paint(canvas.get(), (0.0, 0.0));

    let rect_height_style = RectHeightStyle::Max;
    let rect_width_style = RectWidthStyle::Tight;
    let boxes = paragraph.get_rects_for_range(0..100, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::RED, &boxes);

    reporter_assert!(reporter, boxes.len() == 1_usize);
    if boxes.len() >= 1 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 538.120, EPSILON100)
        ); // DIFF: 510.09375
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.top, -0.280, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.right, 900.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 44.0, EPSILON100)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L2686-L2733 (chrome/m156)
def_paragraph_test!(SkParagraph_ArabicRectsLTRLeftAlignParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_ArabicRectsLTRLeftAlignParagraph.png");
    let text = "Helloبمباركة التقليدية قام عن. تصفح يد ";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_max_lines(14);
    paragraph_style.set_text_align(TextAlign::Left);
    paragraph_style.set_text_direction(TextDirection::LTR);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Noto Naskh Arabic"]);
    text_style.set_font_size(26.0);
    text_style.set_word_spacing(5.0);
    text_style.set_color(Color::BLACK);
    text_style.set_decoration_type(TextDecoration::UNDERLINE);
    text_style.set_decoration_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH - 100.0);

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.runs().len() == 3);

    paragraph.paint(canvas.get(), (0.0, 0.0));

    let rect_height_style = RectHeightStyle::Max;
    let rect_width_style = RectWidthStyle::Tight;
    // There are 39 codepoints: [0:39); asking for [36:40) would give the same as for [36:39)
    let boxes = paragraph.get_rects_for_range(36..40, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::RED, &boxes);

    reporter_assert!(reporter, boxes.len() == 2_usize);
    if boxes.len() >= 2 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 65.65, EPSILON100)
        ); // DIFF: 89.40625
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.top, -0.27, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.right, 86.89, EPSILON100)
        ); // DIFF: 121.87891
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 44.0, EPSILON100)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L2736-L2783 (chrome/m156)
def_paragraph_test!(SkParagraph_ArabicRectsLTRRightAlignParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_ArabicRectsLTRRightAlignParagraph.png");
    let text = "Helloبمباركة التقليدية قام عن. تصفح يد ";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_max_lines(14);
    paragraph_style.set_text_align(TextAlign::Right);
    paragraph_style.set_text_direction(TextDirection::LTR);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Noto Naskh Arabic"]);
    text_style.set_font_size(26.0);
    text_style.set_word_spacing(5.0);
    text_style.set_color(Color::BLACK);
    text_style.set_decoration_type(TextDecoration::UNDERLINE);
    text_style.set_decoration_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH - 100.0);

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.runs().len() == 3);

    paragraph.paint(canvas.get(), (0.0, 0.0));

    let rect_height_style = RectHeightStyle::Max;
    let rect_width_style = RectWidthStyle::Tight;
    let boxes = paragraph.get_rects_for_range(36..40, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::RED, &boxes);

    reporter_assert!(reporter, boxes.len() == 2_usize); // DIFF
    if boxes.len() >= 2 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.left, 561.1, EPSILON100)
        ); // DIFF
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.top, -0.27, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.right, 582.34, EPSILON100)
        ); // DIFF
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 44.0, EPSILON100)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L2785-L2847 (chrome/m156)
def_paragraph_test!(
    SkParagraph_GetGlyphPositionAtCoordinateParagraph,
    |reporter| {
        let font_collection = ResourceFontCollection::new(false);
        skip_if_fonts_not_found!(reporter, font_collection);
        let mut canvas = TestCanvas::new("SkParagraph_GetGlyphPositionAtCoordinateParagraph.png");
        let text = concat!(
            "12345 67890 12345 67890 12345 67890 12345 67890 12345 67890 12345 ",
            "67890 12345",
        );

        let mut paragraphStyle = ParagraphStyle::new();
        paragraphStyle.set_text_align(TextAlign::Left);
        paragraphStyle.set_max_lines(10);
        paragraphStyle.turn_hinting_off();
        let mut textStyle = TextStyle::new();
        textStyle.set_font_families(&["Roboto"]);
        textStyle.set_font_style(FontStyle::new(
            Weight::NORMAL,
            Width::NORMAL,
            Slant::Upright,
        ));
        textStyle.set_font_size(50.0);
        textStyle.set_letter_spacing(1.0);
        textStyle.set_word_spacing(5.0);
        textStyle.set_height(1.0);
        textStyle.set_color(Color::BLACK);

        let mut builder =
            ParagraphBuilderImpl::new(&paragraphStyle, &font_collection, get_unicode());
        builder.push_style(&textStyle);
        builder.add_text(text);
        builder.pop();

        let mut paragraph = builder.build();
        paragraph.layout(550.0);
        paragraph.paint(canvas.get(), (0.0, 0.0));

        // Tests for getGlyphPositionAtCoordinate()
        // NOTE: resulting values can be a few off from their respective positions in
        // the original text because the final trailing whitespaces are sometimes not
        // drawn (namely, when using "justify" alignment) and therefore are not active
        // glyphs.
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((-10000.0, -10000.0))
                .position
                == 0
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((-1.0, -1.0))
                .position
                == 0
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((0.0, 0.0))
                .position
                == 0
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((3.0, 3.0))
                .position
                == 0
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((35.0, 1.0))
                .position
                == 1
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((300.0, 2.0))
                .position
                == 11
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((301.0, 2.2))
                .position
                == 11
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((302.0, 2.6))
                .position
                == 11
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((301.0, 2.1))
                .position
                == 11
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((100000.0, 20.0))
                .position
                == 18
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((450.0, 20.0))
                .position
                == 16
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((100000.0, 90.0))
                .position
                == 36
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((-100000.0, 90.0))
                .position
                == 18
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((20.0, -80.0))
                .position
                == 1
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((1.0, 90.0))
                .position
                == 18
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((1.0, 170.0))
                .position
                == 36
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((10000.0, 180.0))
                .position
                == 72
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((70.0, 180.0))
                .position
                == 56
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((1.0, 270.0))
                .position
                == 72
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((35.0, 90.0))
                .position
                == 19
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((10000.0, 10000.0))
                .position
                == 77
        );
        reporter_assert!(
            reporter,
            paragraph
                .get_glyph_position_at_coordinate((85.0, 10000.0))
                .position
                == 75
        );
    }
);

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L2849-L2953 (chrome/m156)
def_paragraph_test!(SkParagraph_GetRectsForRangeParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_GetRectsForRangeParagraph.png");
    let text = concat!(
        "12345,  \"67890\" 12345 67890 12345 67890 12345 67890 12345 67890 12345 ",
        "67890 12345",
    );

    let mut paragraphStyle = ParagraphStyle::new();
    paragraphStyle.set_text_align(TextAlign::Left);
    paragraphStyle.set_max_lines(10);
    paragraphStyle.turn_hinting_off();
    let mut textStyle = TextStyle::new();
    textStyle.set_font_families(&["Roboto"]);
    textStyle.set_font_size(50.0);
    textStyle.set_color(Color::BLACK);
    textStyle.set_font_style(FontStyle::new(
        Weight::MEDIUM,
        Width::NORMAL,
        Slant::Upright,
    ));

    let mut builder = ParagraphBuilderImpl::new(&paragraphStyle, &font_collection, get_unicode());
    builder.push_style(&textStyle);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(550.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let heightStyle = RectHeightStyle::Max;
    let widthStyle = RectWidthStyle::Tight;

    let mut paint = Paint::default();
    paint.set_style(PaintStyle::Stroke);
    paint.set_anti_alias(true);
    paint.set_stroke_width(1.0);

    {
        let result = paragraph.get_rects_for_range(0..0, heightStyle, widthStyle);
        reporter_assert!(reporter, result.is_empty());
    }
    {
        let result = paragraph.get_rects_for_range(0..1, heightStyle, widthStyle);
        canvas.draw_rects(Color::RED, &result);
        reporter_assert!(reporter, result.len() == 1);
        if result.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.left, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.top, 0.40625, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.right, 28.417, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.bottom, 59.0, EPSILON100)
            );
        }
    }
    {
        let result = paragraph.get_rects_for_range(2..8, heightStyle, widthStyle);
        canvas.draw_rects(Color::BLUE, &result);
        reporter_assert!(reporter, result.len() == 1);
        if result.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.left, 56.835, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.top, 0.40625, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.right, 177.97, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.bottom, 59.0, EPSILON100)
            );
        }
    }
    {
        let result = paragraph.get_rects_for_range(8..21, heightStyle, widthStyle);
        canvas.draw_rects(Color::GREEN, &result);
        reporter_assert!(reporter, result.len() == 1);
        if result.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.left, 177.97, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.top, 0.40625, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.right, 507.031, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.bottom, 59.0, EPSILON100)
            );
        }
    }
    {
        let result = paragraph.get_rects_for_range(30..100, heightStyle, widthStyle);
        canvas.draw_rects(Color::RED, &result);
        reporter_assert!(reporter, result.len() == 4);
        if result.len() >= 4 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.left, 211.375, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.top, 59.40625, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.right, 463.623, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.bottom, 118.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[3].rect.left, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[3].rect.top, 236.406, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[3].rect.right, 142.089, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[3].rect.bottom, 295.0, EPSILON100)
            );
        }
    }
    {
        let result = paragraph.get_rects_for_range(19..22, heightStyle, widthStyle);
        canvas.draw_rects(Color::BLUE, &result);
        reporter_assert!(reporter, result.len() == 1);
        if result.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.left, 450.1875, EPSILON20)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.top, 0.40625, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.right, 519.47266, EPSILON20)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.bottom, 59.0, EPSILON100)
            );
        }
    }
    {
        let result = paragraph.get_rects_for_range(21..21, heightStyle, widthStyle);
        reporter_assert!(reporter, result.is_empty());
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L2955-L3030 (chrome/m156)
def_paragraph_test!(SkParagraph_GetRectsForRangeTight, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_GetRectsForRangeTight.png");
    let text = concat!(
        "(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(",
        "　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(",
        "　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)",
    );
    /*
    (　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)
        S    S     S   S   S    S     S   S   S    S     S   S   S    S     S   S   S    S     S   S   S    S     S   S   S    S     S   S   S    S     S   S   S    S     S   S   S    S     S   S   S    S     S   S   S    S     S   S   S    S     S   S   S    S     S   S   S    S     S   S   S    S     S   S   S    S     S   S   S    S     S   S   S    S     S   S   S    S     S   S
     G  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GGG  G G  G  G  G  GG
     W                  W                  W                  W                  W                  W                  W                  W                  W                  W                  W                  W                  W                  W                  W                  W                  W                  W                  W                  W

     */
    let mut paragraphStyle = ParagraphStyle::new();
    paragraphStyle.set_text_align(TextAlign::Left);
    paragraphStyle.set_max_lines(10);
    paragraphStyle.turn_hinting_off();
    let mut textStyle = TextStyle::new();
    textStyle.set_font_families(&["Noto Sans CJK JP"]);
    textStyle.set_font_size(50.0);
    textStyle.set_color(Color::BLACK);
    textStyle.set_font_style(FontStyle::new(
        Weight::MEDIUM,
        Width::NORMAL,
        Slant::Upright,
    ));

    let mut builder = ParagraphBuilderImpl::new(&paragraphStyle, &font_collection, get_unicode());
    builder.push_style(&textStyle);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(550.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let heightStyle = RectHeightStyle::Tight;
    let widthStyle = RectWidthStyle::Tight;
    {
        let result = paragraph.get_rects_for_range(0..0, heightStyle, widthStyle);
        reporter_assert!(reporter, result.is_empty());
    }
    {
        let result = paragraph.get_rects_for_range(0..1, heightStyle, widthStyle);
        canvas.draw_rects(Color::RED, &result);
        reporter_assert!(reporter, result.len() == 1);
        if result.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.left, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.top, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.right, 16.898, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.bottom, 74.0, EPSILON100)
            );
        }
    }
    {
        let result = paragraph.get_rects_for_range(2..8, heightStyle, widthStyle);
        canvas.draw_rects(Color::BLUE, &result);
        reporter_assert!(reporter, result.len() == 1);
        if result.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.left, 66.899, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.top, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.right, 264.099, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.bottom, 74.0, EPSILON100)
            );
        }
    }
    {
        let result = paragraph.get_rects_for_range(8..21, heightStyle, widthStyle);
        canvas.draw_rects(Color::GREEN, &result);
        reporter_assert!(reporter, result.len() == 2);
        if result.len() >= 2 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.left, 264.099, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.top, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.right, 595.085, EPSILON50)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.bottom, 74.0, EPSILON100)
            );
        }
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L3033-L3161 (chrome/m156)
def_paragraph_test!(
    SkParagraph_GetRectsForRangeIncludeLineSpacingMiddle,
    |reporter| {
        let font_collection = ResourceFontCollection::new(false);
        skip_if_fonts_not_found!(reporter, font_collection);
        let mut canvas =
            TestCanvas::new("SkParagraph_GetRectsForRangeIncludeLineSpacingMiddle.png");
        let text = concat!(
            "(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(",
            "　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(",
            "　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)",
        );

        let mut paragraphStyle = ParagraphStyle::new();
        paragraphStyle.set_text_align(TextAlign::Left);
        paragraphStyle.set_max_lines(10);
        paragraphStyle.turn_hinting_off();
        let mut textStyle = TextStyle::new();
        textStyle.set_font_families(&["Roboto"]);
        textStyle.set_font_size(50.0);
        textStyle.set_height(1.6);
        textStyle.set_height_override(true);
        textStyle.set_color(Color::BLACK);
        textStyle.set_font_style(FontStyle::new(
            Weight::MEDIUM,
            Width::NORMAL,
            Slant::Upright,
        ));

        let mut builder =
            ParagraphBuilderImpl::new(&paragraphStyle, &font_collection, get_unicode());
        builder.push_style(&textStyle);
        builder.add_text(text);
        builder.pop();

        let mut paragraph = builder.build();
        paragraph.layout(550.0);
        paragraph.paint(canvas.get(), (0.0, 0.0));

        let heightStyle = RectHeightStyle::IncludeLineSpacingMiddle;
        let widthStyle = RectWidthStyle::Max;
        {
            let result = paragraph.get_rects_for_range(0..0, heightStyle, widthStyle);
            reporter_assert!(reporter, result.is_empty());
        }

        {
            let result = paragraph.get_rects_for_range(0..1, heightStyle, widthStyle);
            canvas.draw_rects(Color::RED, &result);
            reporter_assert!(reporter, result.len() == 1);
            if result.len() >= 1 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 0.0, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 16.946615, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 17.4296889, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 88.473305, EPSILON100)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(2..8, heightStyle, widthStyle);
            canvas.draw_rects(Color::BLUE, &result);
            reporter_assert!(reporter, result.len() == 1);
            if result.len() >= 1 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 67.429688, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 16.946615, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 190.00781, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 88.473305, EPSILON100)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(8..21, heightStyle, widthStyle);
            canvas.draw_rects(Color::GREEN, &result);
            reporter_assert!(reporter, result.len() == 1);
            if result.len() >= 1 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 190.00781, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 16.946615, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 508.0625, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 88.473305, EPSILON100)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(30..150, heightStyle, widthStyle);
            canvas.draw_rects(Color::RED, &result);
            reporter_assert!(reporter, result.len() == 8);
            if result.len() >= 8 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 190.00781, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 88.473305, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 525.687, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 168.47331, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.left, 525.687, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.top, 88.473305, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.right, 570.02344, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.bottom, 168.47331, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[2].rect.left, 0.0, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[2].rect.top, 168.47331, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[2].rect.right, 531.574, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[2].rect.bottom, 248.47331, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[3].rect.left, 531.574, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[3].rect.top, 168.47331, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[3].rect.right, 570.02344, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[3].rect.bottom, 248.47331, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[4].rect.left, 0.0, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[4].rect.top, 248.47331, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[4].rect.right, 570.02344, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[4].rect.bottom, 328.47333, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[5].rect.left, 0.0, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[5].rect.top, 328.47333, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[5].rect.right, 570.02344, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[5].rect.bottom, 408.4733, EPSILON100)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(19..22, heightStyle, widthStyle);
            canvas.draw_rects(Color::BLUE, &result);
            reporter_assert!(reporter, result.len() == 2); // DIFF
            if result.len() >= 2 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 463.72656, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 16.946615, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 530.23047, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 88.473305, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.left, 530.23047, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.top, 16.946615, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.right, 570.02344, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.bottom, 88.473305, EPSILON100)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(21..21, heightStyle, widthStyle);
            reporter_assert!(reporter, result.is_empty());
        }
    }
);

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L3164-L3292 (chrome/m156)
def_paragraph_test!(
    SkParagraph_GetRectsForRangeIncludeLineSpacingTop,
    |reporter| {
        let font_collection = ResourceFontCollection::new(false);
        skip_if_fonts_not_found!(reporter, font_collection);
        let mut canvas = TestCanvas::new("SkParagraph_GetRectsForRangeIncludeLineSpacingTop.png");
        let text = concat!(
            "(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(",
            "　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(",
            "　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)",
        );

        let mut paragraphStyle = ParagraphStyle::new();
        paragraphStyle.set_text_align(TextAlign::Left);
        paragraphStyle.set_max_lines(10);
        paragraphStyle.turn_hinting_off();
        let mut textStyle = TextStyle::new();
        textStyle.set_font_families(&["Roboto"]);
        textStyle.set_font_size(50.0);
        textStyle.set_height(1.6);
        textStyle.set_height_override(true);
        textStyle.set_color(Color::BLACK);
        textStyle.set_font_style(FontStyle::new(
            Weight::MEDIUM,
            Width::NORMAL,
            Slant::Upright,
        ));

        let mut builder =
            ParagraphBuilderImpl::new(&paragraphStyle, &font_collection, get_unicode());
        builder.push_style(&textStyle);
        builder.add_text(text);
        builder.pop();

        let mut paragraph = builder.build();
        paragraph.layout(550.0);
        paragraph.paint(canvas.get(), (0.0, 0.0));

        let heightStyle = RectHeightStyle::IncludeLineSpacingTop;
        let widthStyle = RectWidthStyle::Max;
        {
            let result = paragraph.get_rects_for_range(0..0, heightStyle, widthStyle);
            reporter_assert!(reporter, result.is_empty());
        }

        {
            let result = paragraph.get_rects_for_range(0..1, heightStyle, widthStyle);
            canvas.draw_rects(Color::RED, &result);
            reporter_assert!(reporter, result.len() == 1);
            if result.len() >= 1 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 0.0, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 16.946615, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 17.4296889, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 80.0, EPSILON100)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(2..8, heightStyle, widthStyle);
            canvas.draw_rects(Color::BLUE, &result);
            reporter_assert!(reporter, result.len() == 1);
            if result.len() >= 1 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 67.429688, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 16.946615, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 190.00781, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 80.0, EPSILON100)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(8..21, heightStyle, widthStyle);
            canvas.draw_rects(Color::GREEN, &result);
            reporter_assert!(reporter, result.len() == 1);
            if result.len() >= 1 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 190.00781, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 16.946615, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 508.0625, EPSILON50)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 80.0, EPSILON100)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(30..150, heightStyle, widthStyle);
            canvas.draw_rects(Color::MAGENTA, &result);
            reporter_assert!(reporter, result.len() == 8);
            if result.len() >= 8 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 190.00781, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 80.0, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 525.687, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 160.0, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.left, 525.687, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.top, 80.0, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.right, 570.02344, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.bottom, 160.0, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[2].rect.left, 0.0, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[2].rect.top, 160.0, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[2].rect.right, 531.574, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[2].rect.bottom, 240.0, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[3].rect.left, 531.574, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[3].rect.top, 160.0, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[3].rect.right, 570.02344, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[3].rect.bottom, 240.0, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[4].rect.left, 0.0, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[4].rect.top, 240.0, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[4].rect.right, 570.02344, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[4].rect.bottom, 320.0, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[5].rect.left, 0.0, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[5].rect.top, 320.0, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[5].rect.right, 570.02344, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[5].rect.bottom, 400.0, EPSILON100)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(19..22, heightStyle, widthStyle);
            canvas.draw_rects(Color::BLACK, &result);
            reporter_assert!(reporter, result.len() == 2); // DIFF
            if result.len() >= 2 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 463.72656, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 16.946615, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 530.23047, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 80.0, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.left, 530.23047, EPSILON50)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.top, 16.946615, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.right, 570.02344, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.bottom, 80.0, EPSILON100)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(21..21, heightStyle, widthStyle);
            reporter_assert!(reporter, result.is_empty());
        }
    }
);

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L3295-L3423 (chrome/m156)
def_paragraph_test!(
    SkParagraph_GetRectsForRangeIncludeLineSpacingBottom,
    |reporter| {
        let font_collection = ResourceFontCollection::new(false);
        skip_if_fonts_not_found!(reporter, font_collection);
        let mut canvas =
            TestCanvas::new("SkParagraph_GetRectsForRangeIncludeLineSpacingBottom.png");
        let text = concat!(
            "(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(",
            "　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(",
            "　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)(　´･‿･｀)",
        );

        let mut paragraphStyle = ParagraphStyle::new();
        paragraphStyle.set_text_align(TextAlign::Left);
        paragraphStyle.set_max_lines(10);
        paragraphStyle.turn_hinting_off();
        let mut textStyle = TextStyle::new();
        textStyle.set_font_families(&["Roboto"]);
        textStyle.set_font_size(50.0);
        textStyle.set_height(1.6);
        textStyle.set_height_override(true);
        textStyle.set_color(Color::BLACK);
        textStyle.set_font_style(FontStyle::new(
            Weight::MEDIUM,
            Width::NORMAL,
            Slant::Upright,
        ));

        let mut builder =
            ParagraphBuilderImpl::new(&paragraphStyle, &font_collection, get_unicode());
        builder.push_style(&textStyle);
        builder.add_text(text);
        builder.pop();

        let mut paragraph = builder.build();
        paragraph.layout(550.0);
        paragraph.paint(canvas.get(), (0.0, 0.0));

        let heightStyle = RectHeightStyle::IncludeLineSpacingBottom;
        let widthStyle = RectWidthStyle::Max;
        {
            let result = paragraph.get_rects_for_range(0..0, heightStyle, widthStyle);
            reporter_assert!(reporter, result.is_empty());
        }

        {
            let result = paragraph.get_rects_for_range(0..1, heightStyle, widthStyle);
            canvas.draw_rects(Color::RED, &result);
            reporter_assert!(reporter, result.len() == 1);
            if result.len() >= 1 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 0.0, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 16.946, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 17.429, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 96.946, EPSILON100)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(2..8, heightStyle, widthStyle);
            canvas.draw_rects(Color::BLUE, &result);
            reporter_assert!(reporter, result.len() == 1);
            if result.len() >= 1 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 67.4298, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 16.946, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 190.007, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 96.946, EPSILON100)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(8..21, heightStyle, widthStyle);
            canvas.draw_rects(Color::GREEN, &result);
            reporter_assert!(reporter, result.len() == 1);
            if result.len() >= 1 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 190.007, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 16.946, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 508.062, EPSILON50)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 96.946, EPSILON100)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(30..150, heightStyle, widthStyle);
            canvas.draw_rects(Color::MAGENTA, &result);
            reporter_assert!(reporter, result.len() == 8);
            if result.len() >= 8 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 190.007, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 96.946, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 525.687, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 176.946, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.left, 525.687, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.top, 96.946, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.right, 570.023, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.bottom, 176.946, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[2].rect.left, 0.0, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[2].rect.top, 176.946, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[2].rect.right, 531.574, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[2].rect.bottom, 256.946, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[3].rect.left, 531.574, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[3].rect.top, 176.946, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[3].rect.right, 570.023, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[3].rect.bottom, 256.946, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[4].rect.left, 0.0, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[4].rect.top, 256.946, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[4].rect.right, 570.023, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[4].rect.bottom, 336.946, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[5].rect.left, 0.0, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[5].rect.top, 336.946, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[5].rect.right, 570.023, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[5].rect.bottom, 416.946, EPSILON100)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(19..22, heightStyle, widthStyle);
            canvas.draw_rects(Color::BLACK, &result);
            reporter_assert!(reporter, result.len() == 2); // DIFF
            if result.len() >= 2 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 463.726, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 16.946, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 530.230, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 96.946, EPSILON100)
                );

                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.left, 530.230, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.top, 16.946, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.right, 570.023, EPSILON20)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[1].rect.bottom, 96.946, EPSILON100)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(21..21, heightStyle, widthStyle);
            reporter_assert!(reporter, result.is_empty());
        }
    }
);

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L3490-L3595 (chrome/m156)
def_paragraph_test!(SkParagraph_GetRectsForRangeCenterParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_GetRectsForRangeCenterParagraph.png");
    // Minikin uses a hard coded list of unicode characters that he treats as invisible - as spaces.
    // It's absolutely wrong - invisibility is a glyph attribute, not character/grapheme.
    // Any attempt to substitute one for another leads to errors
    // (for instance, some fonts can use these hard coded characters for something that is visible)
    let text = "01234  　 ";

    let mut paragraphStyle = ParagraphStyle::new();
    paragraphStyle.set_text_align(TextAlign::Center);
    paragraphStyle.set_max_lines(10);
    paragraphStyle.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraphStyle, &font_collection, get_unicode());

    let mut textStyle = TextStyle::new();
    textStyle.set_font_families(&["Roboto"]);
    textStyle.set_font_size(50.0);
    textStyle.set_height(1.0);
    textStyle.set_color(Color::BLACK);
    textStyle.set_font_style(FontStyle::new(
        Weight::MEDIUM,
        Width::NORMAL,
        Slant::Upright,
    ));

    builder.push_style(&textStyle);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(550.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    // Some of the formatting lazily done on paint
    let heightStyle = RectHeightStyle::Max;
    let widthStyle = RectWidthStyle::Tight;
    {
        let result = paragraph.get_rects_for_range(0..0, heightStyle, widthStyle);
        reporter_assert!(reporter, result.is_empty());
    }

    {
        let result = paragraph.get_rects_for_range(0..1, heightStyle, widthStyle);
        canvas.draw_rects(Color::RED, &result);
        reporter_assert!(reporter, result.len() == 1);
        if result.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.left, 203.955, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.top, 0.40625, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.right, 232.373, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.bottom, 59.0, EPSILON100)
            );
        }
    }

    {
        let result = paragraph.get_rects_for_range(2..4, heightStyle, widthStyle);
        canvas.draw_rects(Color::BLUE, &result);
        reporter_assert!(reporter, result.len() == 1);
        if result.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.left, 260.791, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.top, 0.40625, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.right, 317.626, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.bottom, 59.0, EPSILON100)
            );
        }
    }

    {
        let result = paragraph.get_rects_for_range(4..5, heightStyle, widthStyle);
        canvas.draw_rects(Color::GREEN, &result);
        reporter_assert!(reporter, result.len() == 1);
        if result.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.left, 317.626, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.top, 0.40625, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.right, 346.044, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.bottom, 59.0, EPSILON100)
            );
        }
    }

    {
        let result = paragraph.get_rects_for_range(4..6, heightStyle, widthStyle);
        canvas.draw_rects(Color::BLACK, &result);
        reporter_assert!(reporter, result.len() == 1); // DIFF
        if result.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.left, 317.626, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.top, 0.40625, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.right, 358.494, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.bottom, 59.0, EPSILON100)
            );
        }
    }

    {
        let result = paragraph.get_rects_for_range(5..6, heightStyle, widthStyle);
        canvas.draw_rects(Color::RED, &result);
        reporter_assert!(reporter, result.len() == 1);
        if result.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.left, 346.044, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.top, 0.40625, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.right, 358.494, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.bottom, 59.0, EPSILON100)
            );
        }
    }

    {
        let result = paragraph.get_rects_for_range(21..21, heightStyle, widthStyle);
        reporter_assert!(reporter, result.is_empty());
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L3598-L3661 (chrome/m156)
def_paragraph_test!(
    SkParagraph_GetRectsForRangeCenterParagraphNewlineCentered,
    |reporter| {
        let font_collection = ResourceFontCollection::new(false);
        skip_if_fonts_not_found!(reporter, font_collection);
        let mut canvas =
            TestCanvas::new("SkParagraph_GetRectsForRangeCenterParagraphNewlineCentered.png");
        let text = "01234\n";

        let mut paragraphStyle = ParagraphStyle::new();
        paragraphStyle.set_text_align(TextAlign::Center);
        paragraphStyle.set_max_lines(10);
        paragraphStyle.turn_hinting_off();
        let mut builder =
            ParagraphBuilderImpl::new(&paragraphStyle, &font_collection, get_unicode());

        let mut textStyle = TextStyle::new();
        textStyle.set_font_families(&["Roboto"]);
        textStyle.set_font_size(50.0);
        textStyle.set_height(1.0);
        textStyle.set_color(Color::BLACK);
        textStyle.set_font_style(FontStyle::new(
            Weight::MEDIUM,
            Width::NORMAL,
            Slant::Upright,
        ));

        builder.push_style(&textStyle);
        builder.add_text(text);
        builder.pop();

        let mut paragraph = builder.build();
        paragraph.layout(550.0);

        paragraph.paint(canvas.get(), (0.0, 0.0));

        let impl_ = &paragraph;
        reporter_assert!(reporter, impl_.lines().len() == 2);

        let heightStyle = RectHeightStyle::Max;
        let widthStyle = RectWidthStyle::Tight;
        {
            let result = paragraph.get_rects_for_range(0..0, heightStyle, widthStyle);
            reporter_assert!(reporter, result.is_empty());
        }

        {
            let result = paragraph.get_rects_for_range(0..1, heightStyle, widthStyle);
            canvas.draw_rects(Color::RED, &result);
            reporter_assert!(reporter, result.len() == 1);
            if result.len() >= 1 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 203.955, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 0.40625, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 232.373, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 59.0, EPSILON100)
                );
            }
        }

        {
            let result = paragraph.get_rects_for_range(6..7, heightStyle, widthStyle);
            canvas.draw_rects(Color::BLUE, &result);
            reporter_assert!(reporter, result.len() == 1);
            if result.len() >= 1 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 275.0, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 59.406, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 275.0, EPSILON100)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 118.0, EPSILON100)
                );
            }
        }
    }
);

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L3664-L3775 (chrome/m156)
def_paragraph_test!(
    SkParagraph_GetRectsForRangeCenterMultiLineParagraph,
    |reporter| {
        let font_collection = ResourceFontCollection::new(false);
        skip_if_fonts_not_found!(reporter, font_collection);
        let mut canvas =
            TestCanvas::new("SkParagraph_GetRectsForRangeCenterMultiLineParagraph.png");
        let text = "01234  　 \n0123　        ";

        let mut paragraphStyle = ParagraphStyle::new();
        paragraphStyle.set_text_align(TextAlign::Center);
        paragraphStyle.set_max_lines(10);
        paragraphStyle.turn_hinting_off();
        let mut builder =
            ParagraphBuilderImpl::new(&paragraphStyle, &font_collection, get_unicode());

        let mut textStyle = TextStyle::new();
        textStyle.set_font_families(&["Roboto"]);
        textStyle.set_font_size(50.0);
        textStyle.set_height(1.0);
        textStyle.set_color(Color::BLACK);
        textStyle.set_font_style(FontStyle::new(
            Weight::MEDIUM,
            Width::NORMAL,
            Slant::Upright,
        ));

        builder.push_style(&textStyle);
        builder.add_text(text);
        builder.pop();

        let mut paragraph = builder.build();
        paragraph.layout(550.0);

        paragraph.paint(canvas.get(), (0.0, 0.0));

        let impl_ = &paragraph;

        reporter_assert!(reporter, impl_.lines().len() == 2);

        let heightStyle = RectHeightStyle::Max;
        let widthStyle = RectWidthStyle::Tight;
        let epsilon: scalar = 0.01;
        {
            let result = paragraph.get_rects_for_range(0..0, heightStyle, widthStyle);
            reporter_assert!(reporter, result.is_empty());
        }
        {
            let result = paragraph.get_rects_for_range(0..1, heightStyle, widthStyle);
            canvas.draw_rects(Color::RED, &result);
            reporter_assert!(reporter, result.len() == 1);
            if result.len() >= 1 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 203.955, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 0.40625, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 232.373, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 59.0, epsilon)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(2..4, heightStyle, widthStyle);
            canvas.draw_rects(Color::BLUE, &result);
            reporter_assert!(reporter, result.len() == 1);
            if result.len() >= 1 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 260.791, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 0.40625, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 317.626, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 59.0, epsilon)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(4..6, heightStyle, widthStyle);
            canvas.draw_rects(Color::GREEN, &result);
            reporter_assert!(reporter, result.len() == 1);
            if result.len() >= 1 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 317.626, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 0.40625, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 358.494, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 59.0, epsilon)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(5..6, heightStyle, widthStyle);
            canvas.draw_rects(Color::YELLOW, &result);
            reporter_assert!(reporter, result.len() == 1);
            if result.len() >= 1 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 346.044, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 0.40625, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 358.494, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 59.0, epsilon)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(10..12, heightStyle, widthStyle);
            canvas.draw_rects(Color::CYAN, &result);
            reporter_assert!(reporter, result.len() == 1);
            if result.len() >= 1 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 218.164, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 59.40625, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 275.0, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 118.0, epsilon)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(14..18, heightStyle, widthStyle);
            canvas.draw_rects(Color::BLACK, &result);
            reporter_assert!(reporter, result.len() == 1);
            if result.len() >= 1 {
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.left, 331.835, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.top, 59.40625, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.right, 419.189, epsilon)
                );
                reporter_assert!(
                    reporter,
                    sk_scalar_nearly_equal!(result[0].rect.bottom, 118.0, epsilon)
                );
            }
        }
        {
            let result = paragraph.get_rects_for_range(21..21, heightStyle, widthStyle);
            reporter_assert!(reporter, result.is_empty());
        }
    }
);

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L3778-L3824 (chrome/m156)
def_paragraph_test!(SkParagraph_GetRectsForRangeStrut, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_GetRectsForRangeStrut.png");
    let text = "Chinese 字典";

    let mut strutStyle = StrutStyle::new();
    strutStyle.set_strut_enabled(true);
    strutStyle.set_font_families(&["Roboto"]);
    strutStyle.set_font_size(14.0);

    let mut paragraphStyle = ParagraphStyle::new();
    paragraphStyle.set_strut_style(strutStyle.clone());

    let mut textStyle = TextStyle::new();
    textStyle.set_font_families(&["Noto Sans CJK JP"]);
    textStyle.set_font_size(20.0);
    textStyle.set_color(Color::BLACK);

    let mut builder = ParagraphBuilderImpl::new(&paragraphStyle, &font_collection, get_unicode());
    builder.push_style(&textStyle);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(550.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    {
        let result =
            paragraph.get_rects_for_range(0..10, RectHeightStyle::Tight, RectWidthStyle::Max);
        canvas.draw_rects(Color::GREEN, &result);
        reporter_assert!(reporter, result.len() == 1);
    }

    {
        let result =
            paragraph.get_rects_for_range(0..10, RectHeightStyle::Strut, RectWidthStyle::Max);
        canvas.draw_rects(Color::RED, &result);
        reporter_assert!(reporter, result.len() == 1);
        if result.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.left, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.top, 10.611, EPSILON2)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.right, 118.605, EPSILON50)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.bottom, 27.017, EPSILON2)
            );
        }
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L3826-L3864 (chrome/m156)
def_paragraph_test!(SkParagraph_GetRectsForRangeStrutWithHeight, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let text = "A";

    let mut strutStyle = StrutStyle::new();
    strutStyle.set_strut_enabled(true);
    strutStyle.set_font_families(&["Roboto"]);
    strutStyle.set_font_size(14.0);
    strutStyle.set_height_override(true);
    strutStyle.set_height(2.0);
    strutStyle.set_leading(3.0);

    let mut paragraphStyle = ParagraphStyle::new();
    paragraphStyle.set_strut_style(strutStyle.clone());

    let mut textStyle = TextStyle::new();
    textStyle.set_font_families(&["Roboto"]);
    textStyle.set_font_size(10.0);
    textStyle.set_color(Color::BLACK);

    let mut builder = ParagraphBuilderImpl::new(&paragraphStyle, &font_collection, get_unicode());
    builder.push_style(&textStyle);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(550.0);

    let result = paragraph.get_rects_for_range(0..1, RectHeightStyle::Strut, RectWidthStyle::Max);
    reporter_assert!(reporter, result.len() == 1);
    if result.len() >= 1 {
        // Half of the strut leading: 3.0 * 14.0 / 2
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(result[0].rect.top, 21.0, EPSILON100)
        );
        // Strut height 2.0 * 14.0
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(result[0].rect.height(), 28.0, EPSILON100)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L3866-L3906 (chrome/m156)
def_paragraph_test!(
    SkParagraph_GetRectsForRangeStrutWithHeightAndHalfLeading,
    |reporter| {
        let font_collection = ResourceFontCollection::new(false);
        skip_if_fonts_not_found!(reporter, font_collection);
        let text = "A";

        let mut strutStyle = StrutStyle::new();
        strutStyle.set_strut_enabled(true);
        strutStyle.set_font_families(&["Roboto"]);
        strutStyle.set_font_size(14.0);
        strutStyle.set_height_override(true);
        strutStyle.set_height(2.0);
        strutStyle.set_leading(3.0);
        strutStyle.set_half_leading(true);

        let mut paragraphStyle = ParagraphStyle::new();
        paragraphStyle.set_strut_style(strutStyle.clone());

        let mut textStyle = TextStyle::new();
        textStyle.set_font_families(&["Roboto"]);
        textStyle.set_font_size(10.0);
        textStyle.set_color(Color::BLACK);

        let mut builder =
            ParagraphBuilderImpl::new(&paragraphStyle, &font_collection, get_unicode());
        builder.push_style(&textStyle);
        builder.add_text(text);
        builder.pop();

        let mut paragraph = builder.build();
        paragraph.layout(550.0);

        // Produces the same results as halfLeading = false.
        let result =
            paragraph.get_rects_for_range(0..1, RectHeightStyle::Strut, RectWidthStyle::Max);
        reporter_assert!(reporter, result.len() == 1);
        if result.len() >= 1 {
            // Half of the strut leading: 3.0 * 14.0 / 2
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.top, 21.0, EPSILON100)
            );
            // Strut height 2.0 * 14.0
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(result[0].rect.height(), 28.0, EPSILON100)
            );
        }
    }
);

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L3908-L3946 (chrome/m156)
def_paragraph_test!(SkParagraph_GetRectsForRangeStrutFallback, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_GetRectsForRangeStrutFallback.png");
    let text = "Chinese 字典";

    let mut strutStyle = StrutStyle::new();
    strutStyle.set_strut_enabled(false);

    let mut paragraphStyle = ParagraphStyle::new();
    paragraphStyle.set_strut_style(strutStyle.clone());

    let mut textStyle = TextStyle::new();
    textStyle.set_font_families(&["Noto Sans CJK JP"]);
    textStyle.set_font_size(20.0);
    textStyle.set_color(Color::BLACK);

    let mut builder = ParagraphBuilderImpl::new(&paragraphStyle, &font_collection, get_unicode());
    builder.push_style(&textStyle);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(550.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let result1 = paragraph.get_rects_for_range(0..10, RectHeightStyle::Tight, RectWidthStyle::Max);
    canvas.draw_rects(Color::GREEN, &result1);
    reporter_assert!(reporter, result1.len() == 1);

    let result2 = paragraph.get_rects_for_range(0..10, RectHeightStyle::Strut, RectWidthStyle::Max);
    canvas.draw_rects(Color::RED, &result2);
    reporter_assert!(reporter, result2.len() == 1);

    if result1.len() >= 1 && result2.len() >= 1 {
        reporter_assert!(reporter, result1[0].rect == result2[0].rect);
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L3949-L4022 (chrome/m156)
def_paragraph_test!(SkParagraph_GetWordBoundaryParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_GetWordBoundaryParagraph.png");
    let text = concat!(
        "12345  67890 12345 67890 12345 67890 12345 ",
        "67890 12345 67890 12345 67890 12345",
    );
    let mut paragraphStyle = ParagraphStyle::new();
    paragraphStyle.set_text_align(TextAlign::Left);
    paragraphStyle.set_max_lines(10);
    paragraphStyle.turn_hinting_off();
    let mut textStyle = TextStyle::new();
    textStyle.set_font_families(&["Roboto"]);
    textStyle.set_font_size(52.0);
    textStyle.set_letter_spacing(1.19039);
    textStyle.set_word_spacing(5.0);
    textStyle.set_height(1.5);
    textStyle.set_height_override(true);
    textStyle.set_color(Color::BLACK);

    let mut builder = ParagraphBuilderImpl::new(&paragraphStyle, &font_collection, get_unicode());
    builder.push_style(&textStyle);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(550.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    reporter_assert!(reporter, paragraph.get_word_boundary(0) == (0..5));
    reporter_assert!(reporter, paragraph.get_word_boundary(1) == (0..5));
    reporter_assert!(reporter, paragraph.get_word_boundary(2) == (0..5));
    reporter_assert!(reporter, paragraph.get_word_boundary(3) == (0..5));
    reporter_assert!(reporter, paragraph.get_word_boundary(4) == (0..5));
    let mut boxes =
        paragraph.get_rects_for_range(5..6, RectHeightStyle::Max, RectWidthStyle::Tight);
    canvas.draw_lines(Color::RED, &boxes);

    reporter_assert!(reporter, paragraph.get_word_boundary(5) == (5..7));
    boxes = paragraph.get_rects_for_range(6..7, RectHeightStyle::Max, RectWidthStyle::Tight);
    canvas.draw_lines(Color::RED, &boxes);

    reporter_assert!(reporter, paragraph.get_word_boundary(6) == (5..7));
    boxes = paragraph.get_rects_for_range(7..8, RectHeightStyle::Max, RectWidthStyle::Tight);
    canvas.draw_lines(Color::RED, &boxes);

    reporter_assert!(reporter, paragraph.get_word_boundary(7) == (7..12));
    reporter_assert!(reporter, paragraph.get_word_boundary(8) == (7..12));
    reporter_assert!(reporter, paragraph.get_word_boundary(9) == (7..12));
    reporter_assert!(reporter, paragraph.get_word_boundary(10) == (7..12));
    reporter_assert!(reporter, paragraph.get_word_boundary(11) == (7..12));
    reporter_assert!(reporter, paragraph.get_word_boundary(12) == (12..13));
    reporter_assert!(reporter, paragraph.get_word_boundary(13) == (13..18));
    reporter_assert!(reporter, paragraph.get_word_boundary(30) == (30..31));

    boxes = paragraph.get_rects_for_range(12..13, RectHeightStyle::Max, RectWidthStyle::Tight);
    canvas.draw_lines(Color::RED, &boxes);
    boxes = paragraph.get_rects_for_range(13..14, RectHeightStyle::Max, RectWidthStyle::Tight);
    canvas.draw_lines(Color::RED, &boxes);
    boxes = paragraph.get_rects_for_range(18..19, RectHeightStyle::Max, RectWidthStyle::Tight);
    canvas.draw_lines(Color::RED, &boxes);
    boxes = paragraph.get_rects_for_range(19..20, RectHeightStyle::Max, RectWidthStyle::Tight);
    canvas.draw_lines(Color::RED, &boxes);
    boxes = paragraph.get_rects_for_range(24..25, RectHeightStyle::Max, RectWidthStyle::Tight);
    canvas.draw_lines(Color::RED, &boxes);
    boxes = paragraph.get_rects_for_range(25..26, RectHeightStyle::Max, RectWidthStyle::Tight);
    canvas.draw_lines(Color::RED, &boxes);
    boxes = paragraph.get_rects_for_range(30..31, RectHeightStyle::Max, RectWidthStyle::Tight);
    canvas.draw_lines(Color::RED, &boxes);
    boxes = paragraph.get_rects_for_range(31..32, RectHeightStyle::Max, RectWidthStyle::Tight);
    canvas.draw_lines(Color::RED, &boxes);

    let outLen = (&paragraph).text().len();
    reporter_assert!(
        reporter,
        paragraph.get_word_boundary((outLen - 1) as u32) == (outLen - 5..outLen)
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L4025-L4105 (chrome/m156)
def_paragraph_test!(SkParagraph_SpacingParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_SpacingParagraph.png");
    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(10);
    paragraph_style.set_text_align(TextAlign::Left);
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(50.0);
    text_style.set_letter_spacing(20.0);
    text_style.set_word_spacing(0.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text("H");
    builder.pop();

    text_style.set_letter_spacing(10.0);
    text_style.set_word_spacing(0.0);
    builder.push_style(&text_style);
    builder.add_text("H");
    builder.pop();

    text_style.set_letter_spacing(20.0);
    text_style.set_word_spacing(0.0);
    builder.push_style(&text_style);
    builder.add_text("H");
    builder.pop();

    text_style.set_letter_spacing(0.0);
    text_style.set_word_spacing(0.0);
    builder.push_style(&text_style);
    builder.add_text("|");
    builder.pop();

    let hSpace = "H ";

    text_style.set_letter_spacing(0.0);
    text_style.set_word_spacing(20.0);
    builder.push_style(&text_style);
    builder.add_text(hSpace);
    builder.pop();

    text_style.set_letter_spacing(0.0);
    text_style.set_word_spacing(0.0);
    builder.push_style(&text_style);
    builder.add_text(hSpace);
    builder.pop();

    text_style.set_letter_spacing(0.0);
    text_style.set_letter_spacing(0.0);
    text_style.set_word_spacing(20.0);
    builder.push_style(&text_style);
    builder.add_text(hSpace);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(550.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.lines().len() == 1);
    let mut index: usize = 0;
    impl_.lines()[0].scan_styles(
        impl_,
        StyleType::LetterSpacing,
        &mut |_text_range, _style, _context| {
            index += 1;
        },
    );
    reporter_assert!(reporter, index == 4);
    index = 0;
    impl_.lines()[0].scan_styles(
        impl_,
        StyleType::WordSpacing,
        &mut |_text_range, _style, _context| {
            index += 1;
        },
    );
    reporter_assert!(reporter, index == 4);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L4108-L4151 (chrome/m156)
def_paragraph_test!(SkParagraph_LongWordParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_LongWordParagraph.png");
    let text = concat!(
        "A ",
        "veryverylongwordtoseewherethiswillwraporifitwillatallandifitdoesthenthat",
        "wouldbeagoodthingbecausethebreakingisworking.",
    );

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::RED);
    text_style.set_font_size(31.0);
    text_style.set_letter_spacing(0.0);
    text_style.set_word_spacing(0.0);
    text_style.set_color(Color::BLACK);
    text_style.set_height(1.0);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH / 2.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.text().len() == text.len());
    reporter_assert!(reporter, impl_.runs().len() == 1);
    reporter_assert!(reporter, impl_.styles().len() == 1);
    if impl_.styles().len() >= 1 {
        reporter_assert!(reporter, impl_.styles()[0].style.equals(&text_style));
    }
    reporter_assert!(reporter, impl_.lines().len() == 4);
    if impl_.lines().len() >= 4 {
        reporter_assert!(
            reporter,
            impl_.lines()[0].width() > TEST_CANVAS_WIDTH / 2.0 - 20.0
        );
        reporter_assert!(
            reporter,
            impl_.lines()[1].width() > TEST_CANVAS_WIDTH / 2.0 - 20.0
        );
        reporter_assert!(
            reporter,
            impl_.lines()[2].width() > TEST_CANVAS_WIDTH / 2.0 - 20.0
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L4154-L4199 (chrome/m156)
def_paragraph_test!(SkParagraph_KernScaleParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_KernScaleParagraph.png");

    let text1 = "AVAVAWAH A0 V0 VA To The Lo";
    let text2 = concat!(
        " Dialog Text List lots of words to see ",
        "if kerning works on a bigger set of characters AVAVAW",
    );
    let scale: f32 = 3.0;
    let paragraph_style = ParagraphStyle::new();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Droid Serif"]);
    text_style.set_font_size(100.0 / scale);
    text_style.set_color(Color::BLACK);

    builder.push_style(&text_style);
    builder.add_text(text1);
    builder.push_style(&text_style);
    builder.add_text("A");
    builder.push_style(&text_style);
    builder.add_text("V");
    text_style.set_font_size(14.0 / scale);
    builder.push_style(&text_style);
    builder.add_text(text2);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH / scale);
    canvas.get().scale((scale, scale));
    paragraph.paint(canvas.get(), (0.0, 0.0));
    canvas.get().scale((1.0, 1.0));

    let impl_ = &paragraph;

    // First and second lines must have the same width, the third one must be bigger
    reporter_assert!(reporter, impl_.lines().len() == 3);
    if impl_.lines().len() >= 3 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(impl_.lines()[0].width(), 285.858, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(impl_.lines()[1].width(), 329.709, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(impl_.lines()[2].width(), 120.619, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(impl_.lines()[0].height(), 39.00, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(impl_.lines()[1].height(), 39.00, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(impl_.lines()[2].height(), 05.00, EPSILON100)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L4202-L4242 (chrome/m156)
def_paragraph_test!(SkParagraph_NewlineParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_NewlineParagraph.png");
    let text = concat!(
        "line1\nline2 test1 test2 test3 test4 test5 test6 test7\nline3\n\nline4 ",
        "test1 test2 test3 test4",
    );

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::RED);
    text_style.set_font_size(60.0);
    text_style.set_color(Color::BLACK);
    text_style.set_height(1.0);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH - 300.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;
    // Minikin does not count empty lines but SkParagraph does
    reporter_assert!(reporter, impl_.lines().len() == 7);

    if impl_.lines().len() >= 7 {
        reporter_assert!(reporter, impl_.lines()[0].offset().y == 0.0);
        reporter_assert!(reporter, impl_.lines()[1].offset().y == 70.0);
        reporter_assert!(reporter, impl_.lines()[2].offset().y == 140.0);
        reporter_assert!(reporter, impl_.lines()[3].offset().y == 210.0);
        reporter_assert!(reporter, impl_.lines()[4].offset().y == 280.0); // Empty line
        reporter_assert!(reporter, impl_.lines()[5].offset().y == 350.0);
        reporter_assert!(reporter, impl_.lines()[6].offset().y == 420.0);
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L4245-L4292 (chrome/m156)
def_paragraph_test!(
    #[ignore = "see notes/SkParagraphTest-SkParagraph_EmojiParagraph.md"]
    SkParagraph_EmojiParagraph,
    |reporter| {
        let font_collection = ResourceFontCollection::new(false);
        skip_if_fonts_not_found!(reporter, font_collection);
        let mut canvas = TestCanvas::new("SkParagraph_EmojiParagraph.png");
        let text = "😀😃😄😁😆😅😂🤣☺😇🙂😍😡😟😢😻👽💩👍👎🙏👌👋👄👁👦👼👨‍🚀👨‍🚒🙋‍♂️👳👨‍👨‍👧‍👧      💼👡👠☂🐶🐰🐻🐼🐷🐒🐵🐔🐧🐦🐋🐟🐡🕸🐌🐴🐊🐄🐪🐘🌸🌏🔥🌟🌚🌝💦💧      ❄🍕🍔🍟🥝🍱🕶🎩🏈⚽🚴‍♀️🎻🎼🎹🚨🚎🚐⚓🛳🚀🚁🏪🏢🖱⏰📱💾💉📉🛏🔑🔓      📁🗓📊❤💯🚫🔻♠♣🕓❗🏳🏁🏳️‍🌈🇮🇹🇱🇷🇺🇸🇬🇧🇨🇳🇧🇴";

        let mut paragraph_style = ParagraphStyle::new();
        paragraph_style.turn_hinting_off();
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

        let mut text_style = TextStyle::new();
        text_style.set_font_families(&["Noto Color Emoji"]);
        text_style.set_font_size(50.0);
        text_style.set_decoration_type(TextDecoration::UNDERLINE);
        text_style.set_color(Color::BLACK);
        builder.push_style(&text_style);
        builder.add_text(text);
        builder.pop();

        let mut paragraph = builder.build();
        paragraph.layout(TEST_CANVAS_WIDTH);
        paragraph.paint(canvas.get(), (0.0, 0.0));

        reporter_assert!(reporter, paragraph.unresolved_glyphs() == Some(0));

        let impl_ = &paragraph;

        reporter_assert!(reporter, impl_.lines().len() == 8);

        for line in impl_.lines() {
            if !std::ptr::eq(line, &impl_.lines()[impl_.lines().len() - 1]) {
                // The actual value is 50_size / 109_ppemX * 136_advance = ~62.385319
                // FreeType reports advances in 24.6 fixed point, so each is 62.390625
                reporter_assert!(
                    reporter,
                    line.width() == 998.25 || (998.1 < line.width() && line.width() < 998.2),
                    "width: {}",
                    line.width()
                );
            } else {
                reporter_assert!(reporter, line.width() < 998.25, "width: {}", line.width());
            }
            reporter_assert!(reporter, line.height() == 59.0, "height: {}", line.height());
        }
    }
);

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L4295-L4353 (chrome/m156)
def_paragraph_test!(SkParagraph_EmojiMultiLineRectsParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_EmojiMultiLineRectsParagraph.png");
    let text = concat!(
        "👩‍👩‍👦👩‍👩‍👧‍👧🇺🇸👩‍👩‍👦👩‍👩‍👧‍👧i🇺🇸👩‍👩‍👦👩‍👩‍👧‍👧🇺🇸👩‍👩‍👦👩‍👩‍👧‍👧🇺🇸",
        "👩‍👩‍👦👩‍👩‍👧‍👧🇺🇸👩‍👩‍👦👩‍👩‍👧‍👧🇺🇸👩‍👩‍👦👩‍👩‍👧‍👧🇺🇸👩‍👩‍👦👩‍👩‍👧‍👧🇺🇸",
        "👩‍👩‍👦👩‍👩‍👧‍👧🇺🇸👩‍👩‍👦👩‍👩‍👧‍👧🇺🇸👩‍👩‍👦👩‍👩‍👧‍👧🇺🇸👩‍👩‍👦👩‍👩‍👧‍👧🇺🇸",
        "👩‍👩‍👦👩‍👩‍👧‍👧🇺🇸👩‍👩‍👦👩‍👩‍👧‍👧🇺🇸👩‍👩‍👦👩‍👩‍👧‍👧🇺🇸👩‍👩‍👦👩‍👩‍👧‍👧🇺🇸",
        "❄🍕🍔🍟🥝🍱🕶🎩🏈⚽🚴‍♀️🎻🎼🎹🚨🚎🚐⚓🛳🚀🚁🏪🏢🖱⏰📱💾💉📉🛏🔑🔓",
        "📁🗓📊❤💯🚫🔻♠♣🕓❗🏳🏁🏳️‍🌈🇮🇹🇱🇷🇺🇸🇬🇧🇨🇳🇧🇴",
    );

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Noto Color Emoji"]);
    text_style.set_font_size(50.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH - 300.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let rect_height_style = RectHeightStyle::Tight;
    let rect_width_style = RectWidthStyle::Tight;

    let mut result = paragraph.get_rects_for_range(0..0, rect_height_style, rect_width_style);
    reporter_assert!(reporter, result.len() == 0);

    result = paragraph.get_rects_for_range(0..119, rect_height_style, rect_width_style);
    reporter_assert!(reporter, result.len() == 2);
    canvas.draw_rects(Color::RED, &result);

    result = paragraph.get_rects_for_range(122..132, rect_height_style, rect_width_style);
    reporter_assert!(reporter, result.len() == 0);
    // We changed the selection algorithm and now the selection is empty
    canvas.draw_rects(Color::BLUE, &result);

    let mut pos = paragraph
        .get_glyph_position_at_coordinate((610.0, 100.0))
        .position;
    result = paragraph.get_rects_for_range(0..pos as usize, rect_height_style, rect_width_style);
    reporter_assert!(reporter, result.len() == 2);
    canvas.draw_rects(Color::GREEN, &result);

    pos = paragraph
        .get_glyph_position_at_coordinate((580.0, 100.0))
        .position;
    result = paragraph.get_rects_for_range(0..pos as usize, rect_height_style, rect_width_style);
    reporter_assert!(reporter, result.len() == 2);
    canvas.draw_rects(Color::GREEN, &result);

    pos = paragraph
        .get_glyph_position_at_coordinate((560.0, 100.0))
        .position;
    result = paragraph.get_rects_for_range(0..pos as usize, rect_height_style, rect_width_style);
    reporter_assert!(reporter, result.len() == 2);
    canvas.draw_rects(Color::GREEN, &result);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L4356-L4393 (chrome/m156)
def_paragraph_test!(SkParagraph_RepeatLayoutParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_RepeatLayoutParagraph.png");
    let text = concat!(
        "Sentence to layout at diff widths to get diff line counts. short words ",
        "short words short words short words short words short words short words ",
        "short words short words short words short words short words short words ",
        "end",
    );

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(31.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(300.0);

    let impl_ = &paragraph;
    // Some of the formatting lazily done on paint
    reporter_assert!(reporter, impl_.runs().len() == 1);
    reporter_assert!(reporter, impl_.styles().len() == 1);
    reporter_assert!(reporter, impl_.lines().len() == 12);

    paragraph.layout(600.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));
    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.runs().len() == 1);
    reporter_assert!(reporter, impl_.styles().len() == 1);
    reporter_assert!(reporter, impl_.lines().len() == 6);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L4396-L4434 (chrome/m156)
def_paragraph_test!(SkParagraph_Ellipsize, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_Ellipsize.png");
    let text = concat!(
        "This is a very long sentence to test if the text will properly wrap ",
        "around and go to the next line. Sometimes, short sentence. Longer ",
        "sentences are okay too because they are nessecary. Very short. ",
    );

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(1);
    let ellipsis: Vec<u16> = u16s("\u{2026}");
    paragraph_style.set_ellipsis_utf16(&ellipsis);
    let _e: Vec<u16> = paragraph_style.ellipsis_utf16().to_vec();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;

    // Check that the ellipsizer limited the text to one line and did not wrap to a second line.
    reporter_assert!(reporter, impl_.lines().len() == 1);
    if impl_.lines().len() >= 1 {
        let line = &impl_.lines()[0];
        reporter_assert!(reporter, line.ellipsis().is_some());
        reporter_assert!(reporter, impl_.runs().len() == 1);
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L4437-L4503 (chrome/m156)
def_paragraph_test!(SkParagraph_UnderlineShiftParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_UnderlineShiftParagraph.png");
    let text1 = "fluttser ";
    let text2 = "mdje";
    let text3 = "fluttser mdje";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_text_align(TextAlign::Left);
    paragraph_style.set_max_lines(2);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text1);
    text_style.set_decoration_type(TextDecoration::UNDERLINE);
    text_style.set_decoration_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text2);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;

    let mut builder1 = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    text_style.set_decoration_type(TextDecoration::NO_DECORATION);
    builder1.push_style(&text_style);
    builder1.add_text(text3);
    builder1.pop();

    let mut paragraph1 = builder1.build();
    paragraph1.layout(TEST_CANVAS_WIDTH);
    paragraph1.paint(canvas.get(), (0.0, 25.0));

    let impl1 = &paragraph1;

    reporter_assert!(reporter, impl_.lines().len() == 1);
    reporter_assert!(reporter, impl1.lines().len() == 1);

    let rect = paragraph
        .get_rects_for_range(0..12, RectHeightStyle::Max, RectWidthStyle::Tight)
        .first()
        .unwrap()
        .rect;
    let rect1 = paragraph1
        .get_rects_for_range(0..12, RectHeightStyle::Max, RectWidthStyle::Tight)
        .first()
        .unwrap()
        .rect;
    reporter_assert!(reporter, rect.left == rect1.left);
    reporter_assert!(reporter, rect.right == rect1.right);

    for i in 0..12 {
        // Not all ranges produce a rectangle ("fl" goes into one cluster so [0:1) is empty)
        let r1 =
            paragraph.get_rects_for_range(i..i + 1, RectHeightStyle::Max, RectWidthStyle::Tight);
        let r2 =
            paragraph1.get_rects_for_range(i..i + 1, RectHeightStyle::Max, RectWidthStyle::Tight);

        reporter_assert!(reporter, r1.len() == r2.len());
        if !r1.is_empty() && !r2.is_empty() {
            reporter_assert!(
                reporter,
                r1.first().unwrap().rect.left == r2.first().unwrap().rect.left
            );
            reporter_assert!(
                reporter,
                r1.first().unwrap().rect.right == r2.first().unwrap().rect.right
            );
        }
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L4506-L4541 (chrome/m156)
def_paragraph_test!(SkParagraph_SimpleShadow, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_SimpleShadow.png");
    let text = "Hello World Text Dialog";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);
    text_style.add_shadow(TextShadow::new(Color::BLACK, Point::new(2.0, 2.0), 1.0));
    builder.push_style(&text_style);
    builder.add_text(text);

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (10.0, 15.0));

    let impl_ = &paragraph;

    reporter_assert!(reporter, impl_.runs().len() == 1);
    reporter_assert!(reporter, impl_.styles().len() == 1);
    let mut index: usize = 0;
    for line in impl_.lines() {
        line.scan_styles(
            impl_,
            StyleType::Shadow,
            &mut |_text_range, style, _context| {
                reporter_assert!(reporter, index == 0 && style.equals(&text_style));
                index += 1;
            },
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L4544-L4611 (chrome/m156)
def_paragraph_test!(SkParagraph_ComplexShadow, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_ComplexShadow.png");
    let text = "Text Chunk ";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);
    text_style.add_shadow(TextShadow::new(Color::BLACK, Point::new(2.0, 2.0), 1.0));
    builder.push_style(&text_style);
    builder.add_text(text);

    text_style.add_shadow(TextShadow::new(Color::RED, Point::new(2.0, 2.0), 5.0));
    text_style.add_shadow(TextShadow::new(Color::GREEN, Point::new(10.0, -5.0), 3.0));
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    builder.add_text(text);

    text_style.add_shadow(TextShadow::new(Color::RED, Point::new(0.0, 1.0), 0.0));
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    builder.add_text(text);

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (10.0, 15.0));

    let impl_ = &paragraph;

    let mut index: usize = 0;
    for line in impl_.lines() {
        line.scan_styles(
            impl_,
            StyleType::Shadow,
            &mut |_text_range, style, _context| {
                index += 1;
                match index {
                    1 => {
                        reporter_assert!(reporter, style.shadow_number() == 1);
                    }
                    2 => {
                        reporter_assert!(reporter, style.shadow_number() == 3);
                    }
                    3 => {
                        reporter_assert!(reporter, style.shadow_number() == 1);
                    }
                    4 => {
                        reporter_assert!(reporter, style.shadow_number() == 4);
                        reporter_assert!(reporter, style.equals(&text_style));
                    }
                    5 => {
                        reporter_assert!(reporter, style.shadow_number() == 1);
                    }
                    _ => {
                        reporter_assert!(reporter, false);
                    }
                }
            },
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L4614-L4658 (chrome/m156)
def_paragraph_test!(SkParagraph_BaselineParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_BaselineParagraph.png");
    let text = concat!(
        "左線読設Byg後碁給能上目秘使約。満毎冠行来昼本可必図将発確年。今属場育",
        "図情闘陰野高備込制詩西校客。審対江置講今固残必託地集済決維駆年策。立得",
    );

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_max_lines(14);
    paragraph_style.set_text_align(TextAlign::Justify);
    paragraph_style.set_height(1.5);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Source Han Serif CN"]);
    text_style.set_color(Color::BLACK);
    text_style.set_font_size(55.0);
    text_style.set_letter_spacing(2.0);
    text_style.set_decoration_style(TextDecorationStyle::Solid);
    text_style.set_decoration_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH - 100.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let rect1 = Rect::from_xywh(
        0.0,
        paragraph.ideographic_baseline(),
        paragraph.max_width(),
        paragraph.ideographic_baseline(),
    );
    let rect2 = Rect::from_xywh(
        0.0,
        paragraph.alphabetic_baseline(),
        paragraph.max_width(),
        paragraph.alphabetic_baseline(),
    );
    canvas.draw_line(Color::RED, rect1, false);
    canvas.draw_line(Color::GREEN, rect2, false);

    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(paragraph.ideographic_baseline(), 79.035, EPSILON100)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(paragraph.alphabetic_baseline(), 63.305, EPSILON100)
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L4661-L4749 (chrome/m156)
def_paragraph_test!(SkParagraph_FontFallbackParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_FontFallbackParagraph.png");

    let text1 = "Roboto 字典 ";
    let text2 = "Homemade Apple 字典";
    let text3 = "Chinese 字典";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&[
        "Not a real font",
        "Also a fake font",
        "So fake it is obvious",
        "Next one should be a real font...",
        "Roboto",
        "another fake one in between",
        "Homemade Apple",
    ]);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text1);

    text_style.set_font_families(&[
        "Not a real font",
        "Also a fake font",
        "So fake it is obvious",
        "Homemade Apple",
        "Next one should be a real font...",
        "Roboto",
        "another fake one in between",
        "Noto Sans CJK JP",
        "Source Han Serif CN",
    ]);
    builder.push_style(&text_style);
    builder.add_text(text2);

    text_style.set_font_families(&[
        "Not a real font",
        "Also a fake font",
        "So fake it is obvious",
        "Homemade Apple",
        "Next one should be a real font...",
        "Roboto",
        "another fake one in between",
        "Source Han Serif CN",
        "Noto Sans CJK JP",
    ]);
    builder.push_style(&text_style);
    builder.add_text(text3);

    builder.pop();

    let mut paragraph = builder.build();
    reporter_assert!(reporter, paragraph.unresolved_glyphs() == None); // Not shaped yet
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (10.0, 15.0));

    let spaceRun: usize = 1;
    reporter_assert!(reporter, paragraph.unresolved_glyphs() == Some(2)); // From the text1 ("字典" - excluding the last space)

    let impl_ = &paragraph;

    reporter_assert!(reporter, impl_.runs().len() == 6 + spaceRun);
    if impl_.runs().len() == 6 + spaceRun {
        // Font resolution in Skia produces 6 runs because 2 parts of "Roboto 字典 " have different
        // script (Minikin merges the first 2 into one because of unresolved)
        // [Apple + Unresolved + ' '] 0, 1, 2
        // [Apple + Noto] 3, 4
        // [Apple + Han] 5, 6
        let mut robotoAdvance = impl_.runs()[0].advance().x + impl_.runs()[1].advance().x;
        robotoAdvance += impl_.runs()[2].advance().x;

        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(robotoAdvance, 64.199, EPSILON50)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(impl_.runs()[2 + spaceRun].advance().x, 139.125, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(impl_.runs()[3 + spaceRun].advance().x, 27.999, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(impl_.runs()[4 + spaceRun].advance().x, 62.248, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(impl_.runs()[5 + spaceRun].advance().x, 27.999, EPSILON100)
        );

        // When a different font is resolved, then the metrics are different.
        reporter_assert!(
            reporter,
            impl_.runs()[3 + spaceRun].correct_ascent()
                != impl_.runs()[5 + spaceRun].correct_ascent()
        );
        reporter_assert!(
            reporter,
            impl_.runs()[3 + spaceRun].correct_descent()
                != impl_.runs()[5 + spaceRun].correct_descent()
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L4752-L4866 (chrome/m156)
def_paragraph_test!(SkParagraph_StrutParagraph1, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_StrutParagraph1.png");
    // The chinese extra height should be absorbed by the strut.
    let text = "01234満毎冠p来É本可\nabcd\n満毎É行p昼本可";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(10);
    paragraph_style.set_text_align(TextAlign::Left);
    paragraph_style.turn_hinting_off();

    let mut strut_style = StrutStyle::new();
    strut_style.set_strut_enabled(true);
    strut_style.set_font_families(&["BlahFake", "Ahem"]);
    strut_style.set_font_size(50.0);
    strut_style.set_height(1.8);
    strut_style.set_height_override(true);
    strut_style.set_leading(0.1);
    paragraph_style.set_strut_style(strut_style.clone());

    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(50.0);
    text_style.set_font_style(FontStyle::new(
        Weight::MEDIUM,
        Width::NORMAL,
        Slant::Upright,
    ));
    text_style.set_color(Color::BLACK);
    text_style.set_height(0.5);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(550.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.lines().len() == 4);

    let rect_height_style = RectHeightStyle::Tight;
    let rect_height_max_style = RectHeightStyle::Max;
    let rect_width_style = RectWidthStyle::Tight;
    {
        let boxes = paragraph.get_rects_for_range(0..0, rect_height_style, rect_width_style);
        reporter_assert!(reporter, boxes.is_empty());
    }
    {
        let boxes = paragraph.get_rects_for_range(0..1, rect_height_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 34.5, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 50.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 84.5, EPSILON100)
            );
        }
    }
    {
        let boxes = paragraph.get_rects_for_range(0..1, rect_height_max_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 50.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 95.0, EPSILON100)
            );
        }
    }
    {
        let boxes = paragraph.get_rects_for_range(6..10, rect_height_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 300.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 34.5, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 500.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 84.5, EPSILON100)
            );
        }
    }
    {
        let boxes = paragraph.get_rects_for_range(6..10, rect_height_max_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 300.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 500.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 95.0, EPSILON100)
            );
        }
    }
    {
        let boxes = paragraph.get_rects_for_range(14..16, rect_height_max_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 190.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 100.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 285.0, EPSILON100)
            );
        }
    }
    {
        let boxes = paragraph.get_rects_for_range(20..25, rect_height_max_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 50.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 285.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 300.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 380.0, EPSILON100)
            );
        }
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L4869-L4985 (chrome/m156)
def_paragraph_test!(SkParagraph_StrutParagraph2, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_StrutParagraph2.png");
    // The chinese extra height should be absorbed by the strut.
    let text = "01234ABCDEFGH\nabcd\nABCDEFGH";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(10);
    paragraph_style.set_text_align(TextAlign::Left);
    paragraph_style.turn_hinting_off();

    let mut strut_style = StrutStyle::new();

    strut_style.set_strut_enabled(true);
    strut_style.set_font_families(&["Ahem"]);
    strut_style.set_font_size(50.0);
    strut_style.set_height(1.6);
    strut_style.set_height_override(true);
    paragraph_style.set_strut_style(strut_style.clone());

    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(50.0);
    text_style.set_font_style(FontStyle::new(
        Weight::MEDIUM,
        Width::NORMAL,
        Slant::Upright,
    ));
    text_style.set_color(Color::BLACK);
    text_style.set_height(1.0);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(550.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;
    // Font is not resolved and the first line does not fit
    reporter_assert!(reporter, impl_.lines().len() == 4);

    let rect_height_style = RectHeightStyle::Tight;
    let rect_height_max_style = RectHeightStyle::Max;
    let rect_width_style = RectWidthStyle::Tight;
    {
        let boxes = paragraph.get_rects_for_range(0..0, rect_height_style, rect_width_style);
        reporter_assert!(reporter, boxes.is_empty());
    }
    {
        let boxes = paragraph.get_rects_for_range(0..1, rect_height_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 24.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 50.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 74.0, EPSILON100)
            );
        }
    }
    {
        let boxes = paragraph.get_rects_for_range(0..1, rect_height_max_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 50.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 80.0, EPSILON100)
            );
        }
    }
    {
        let boxes = paragraph.get_rects_for_range(6..10, rect_height_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 300.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 24.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 500.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 74.0, EPSILON100)
            );
        }
    }
    {
        let boxes = paragraph.get_rects_for_range(6..10, rect_height_max_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 300.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 500.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 80.0, EPSILON100)
            );
        }
    }
    {
        let boxes = paragraph.get_rects_for_range(14..16, rect_height_max_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 160.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 100.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 240.0, EPSILON100)
            );
        }
    }
    {
        let boxes = paragraph.get_rects_for_range(20..25, rect_height_max_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 50.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 240.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 300.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 320.0, EPSILON100)
            );
        }
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L4988-L5105 (chrome/m156)
def_paragraph_test!(SkParagraph_StrutParagraph3, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_StrutParagraph3.png");

    // The chinese extra height should be absorbed by the strut.
    let text = "01234満毎p行来昼本可\nabcd\n満毎冠行来昼本可";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(10);
    paragraph_style.set_text_align(TextAlign::Left);
    paragraph_style.turn_hinting_off();

    let mut strut_style = StrutStyle::new();
    strut_style.set_strut_enabled(true);
    strut_style.set_font_families(&["Ahem"]);
    strut_style.set_font_size(50.0);
    strut_style.set_height(1.2);
    strut_style.set_height_override(true);
    paragraph_style.set_strut_style(strut_style.clone());

    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(50.0);
    text_style.set_font_style(FontStyle::new(
        Weight::MEDIUM,
        Width::NORMAL,
        Slant::Upright,
    ));
    text_style.set_color(Color::BLACK);
    text_style.set_height(1.0);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(550.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;
    // Font is not resolved and the first line does not fit
    reporter_assert!(reporter, impl_.lines().len() == 4);

    let rect_height_style = RectHeightStyle::Tight;
    let rect_height_max_style = RectHeightStyle::Max;
    let rect_width_style = RectWidthStyle::Tight;
    let epsilon: scalar = 0.001;
    {
        let boxes = paragraph.get_rects_for_range(0..0, rect_height_style, rect_width_style);
        reporter_assert!(reporter, boxes.is_empty());
    }
    {
        let boxes = paragraph.get_rects_for_range(0..1, rect_height_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 0.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 8.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 50.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 58.0, epsilon)
            );
        }
    }
    {
        let boxes = paragraph.get_rects_for_range(0..1, rect_height_max_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 0.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 0.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 50.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 60.0, epsilon)
            );
        }
    }
    {
        let boxes = paragraph.get_rects_for_range(6..10, rect_height_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 300.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 8.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 500.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 58.0, epsilon)
            );
        }
    }
    {
        let boxes = paragraph.get_rects_for_range(6..10, rect_height_max_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 300.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 0.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 500.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 60.0, epsilon)
            );
        }
    }
    {
        let boxes = paragraph.get_rects_for_range(14..16, rect_height_max_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 0.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 120.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 100.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 180.0, epsilon)
            );
        }
    }
    {
        let boxes = paragraph.get_rects_for_range(20..25, rect_height_max_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 50.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 180.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 300.0, epsilon)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 240.0, epsilon)
            );
        }
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5108-L5216 (chrome/m156)
def_paragraph_test!(SkParagraph_StrutForceParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_StrutForceParagraph.png");
    let text = "01234満毎冠行来昼本可\nabcd\n満毎冠行来昼本可";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(10);
    paragraph_style.set_text_align(TextAlign::Left);
    paragraph_style.turn_hinting_off();

    let mut strut_style = StrutStyle::new();
    strut_style.set_strut_enabled(true);
    strut_style.set_font_families(&["Ahem"]);
    strut_style.set_font_size(50.0);
    strut_style.set_height(1.5);
    strut_style.set_height_override(true);
    strut_style.set_leading(0.1);
    strut_style.set_force_strut_height(true);
    paragraph_style.set_strut_style(strut_style.clone());

    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(50.0);
    text_style.set_letter_spacing(0.0);
    text_style.set_color(Color::BLACK);
    text_style.set_height(1.0);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(550.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;
    // Font is not resolved and the first line does not fit
    reporter_assert!(reporter, impl_.lines().len() == 4);

    let rect_height_style = RectHeightStyle::Tight;
    let rect_height_max_style = RectHeightStyle::Max;
    let rect_width_style = RectWidthStyle::Tight;

    let boxes1 = paragraph.get_rects_for_range(0..0, rect_height_style, rect_width_style);
    reporter_assert!(reporter, boxes1.is_empty());

    let boxes2 = paragraph.get_rects_for_range(0..1, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::RED, &boxes2);
    reporter_assert!(reporter, boxes2.len() == 1);
    if boxes2.len() >= 1 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes2[0].rect.left, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes2[0].rect.top, 22.5, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes2[0].rect.right, 50.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes2[0].rect.bottom, 72.5, EPSILON100)
        );
    }

    let boxes3 = paragraph.get_rects_for_range(0..1, rect_height_max_style, rect_width_style);
    canvas.draw_rects(Color::RED, &boxes3);
    reporter_assert!(reporter, boxes3.len() == 1);
    if boxes3.len() >= 1 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes3[0].rect.left, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes3[0].rect.top, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes3[0].rect.right, 50.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes3[0].rect.bottom, 80.0, EPSILON100)
        );
    }

    let boxes4 = paragraph.get_rects_for_range(6..10, rect_height_style, rect_width_style);
    canvas.draw_rects(Color::RED, &boxes4);
    reporter_assert!(reporter, boxes4.len() == 1);
    if boxes4.len() >= 1 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes4[0].rect.left, 300.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes4[0].rect.top, 22.5, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes4[0].rect.right, 500.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes4[0].rect.bottom, 72.5, EPSILON100)
        );
    }

    let boxes5 = paragraph.get_rects_for_range(6..10, rect_height_max_style, rect_width_style);
    canvas.draw_rects(Color::RED, &boxes5);
    reporter_assert!(reporter, boxes5.len() == 1);
    if boxes5.len() >= 1 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes5[0].rect.left, 300.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes5[0].rect.top, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes5[0].rect.right, 500.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes5[0].rect.bottom, 80.0, EPSILON100)
        );
    }

    let boxes6 = paragraph.get_rects_for_range(14..16, rect_height_max_style, rect_width_style);
    canvas.draw_rects(Color::RED, &boxes6);
    reporter_assert!(reporter, boxes6.len() == 1);
    if boxes6.len() >= 1 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes6[0].rect.left, 0.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes6[0].rect.top, 160.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes6[0].rect.right, 100.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes6[0].rect.bottom, 240.0, EPSILON100)
        );
    }

    let boxes7 = paragraph.get_rects_for_range(20..25, rect_height_max_style, rect_width_style);
    canvas.draw_rects(Color::RED, &boxes7);
    reporter_assert!(reporter, boxes7.len() == 1);
    if boxes7.len() >= 1 {
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes7[0].rect.left, 50.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes7[0].rect.top, 240.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes7[0].rect.right, 300.0, EPSILON100)
        );
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes7[0].rect.bottom, 320.0, EPSILON100)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5219-L5284 (chrome/m156)
def_paragraph_test!(SkParagraph_StrutDefaultParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_StrutDefaultParagraph.png");

    let text = "01234満毎冠行来昼本可\nabcd\n満毎冠行来昼本可";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(10);
    paragraph_style.set_text_align(TextAlign::Left);
    paragraph_style.turn_hinting_off();

    let mut strut_style = StrutStyle::new();
    strut_style.set_strut_enabled(true);
    strut_style.set_font_families(&["Ahem"]);
    strut_style.set_font_size(50.0);
    strut_style.set_height(1.5);
    strut_style.set_leading(0.1);
    strut_style.set_force_strut_height(false);
    paragraph_style.set_strut_style(strut_style.clone());

    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(550.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let rect_height_style = RectHeightStyle::Tight;
    let rect_height_strut_style = RectHeightStyle::Strut;
    let rect_width_style = RectWidthStyle::Tight;
    {
        let boxes = paragraph.get_rects_for_range(0..0, rect_height_style, rect_width_style);
        reporter_assert!(reporter, boxes.is_empty());
    }
    {
        let boxes = paragraph.get_rects_for_range(0..1, rect_height_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 26.5, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 20.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 46.5, EPSILON100)
            );
        }
    }
    {
        let boxes = paragraph.get_rects_for_range(0..2, rect_height_strut_style, rect_width_style);
        canvas.draw_rects(Color::RED, &boxes);
        reporter_assert!(reporter, boxes.len() == 1);
        if boxes.len() >= 1 {
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.left, 0.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.top, 2.5, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.right, 40.0, EPSILON100)
            );
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(boxes[0].rect.bottom, 52.5, EPSILON100)
            );
        }
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5286-L5336 (chrome/m156)
def_paragraph_test!(SkParagraph_FontFeaturesParagraph, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_FontFeaturesParagraph.png");

    let text = "12ab\n";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_style(FontStyle::italic()); // Regular Roboto doesn't have font features
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);

    text_style.add_font_feature("tnum", 1);
    builder.push_style(&text_style);
    builder.add_text(text);

    text_style.reset_font_features();
    text_style.add_font_feature("tnum", 0);
    text_style.add_font_feature("pnum", 1);
    builder.push_style(&text_style);
    builder.add_text(text);

    builder.pop();
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);

    paragraph.paint(canvas.get(), (10.0, 15.0));

    let impl_ = &paragraph;
    reporter_assert!(reporter, paragraph.line_number() == 3_usize);
    if paragraph.line_number() >= 3 {
        let tnum_line = &impl_.lines()[0];
        let pnum_line = &impl_.lines()[1];

        reporter_assert!(reporter, tnum_line.clusters().width() == 4_usize);
        reporter_assert!(reporter, pnum_line.clusters().width() == 4_usize);
        // Tabular numbers should have equal widths.
        reporter_assert!(
            reporter,
            impl_.clusters()[0].width() == impl_.clusters()[1].width()
        );
        // Proportional numbers should have variable widths.
        reporter_assert!(
            reporter,
            impl_.clusters()[5].width() != impl_.clusters()[6].width()
        );
        // Alphabetic characters should be unaffected.
        reporter_assert!(
            reporter,
            impl_.clusters()[2].width() == impl_.clusters()[7].width()
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5339-L5368 (chrome/m156)
def_paragraph_test!(SkParagraph_WhitespacesInMultipleFonts, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let text = "English English 字典 字典 😀😃😄 😀😃😄";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto", "Noto Color Emoji", "Source Han Serif CN"]);
    text_style.set_font_size(60.0);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);

    reporter_assert!(reporter, paragraph.unresolved_glyphs() == Some(0));

    let impl_ = &paragraph;
    for i in 0..impl_.runs().len() - 1 {
        let first = impl_.runs()[i].text_range();
        let next = impl_.runs()[i + 1].text_range();
        reporter_assert!(reporter, first.end == next.start);
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5453-L5486 (chrome/m156)
def_paragraph_test!(SkParagraph_CacheText, |reporter| {
    let cache = ParagraphCache::new();
    cache.turn_on(true);
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);

    let mut test = |text: &str, count: i32, expectedToBeFound: bool| {
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        builder.push_style(&text_style);
        builder.add_text(text);
        builder.pop();
        let mut paragraph = builder.build();

        let impl_ = &mut paragraph;
        reporter_assert!(reporter, count == cache.count());
        let found = cache.find_paragraph(impl_);
        reporter_assert!(reporter, found == expectedToBeFound);
        let added = cache.update_paragraph(impl_);
        reporter_assert!(reporter, added != expectedToBeFound);
    };

    test("text1", 0, false);
    test("text1", 1, true);
    test("text2", 1, false);
    test("text2", 2, true);
    test("text3", 2, false);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5488-L5526 (chrome/m156)
def_paragraph_test!(SkParagraph_CacheFonts, |reporter| {
    let cache = ParagraphCache::new();
    cache.turn_on(true);
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();

    let mut text_style = TextStyle::new();
    text_style.set_color(Color::BLACK);

    let text = "text";

    let mut test = |text_style: &TextStyle, count: i32, expectedToBeFound: bool| {
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        builder.push_style(&text_style);
        builder.add_text(text);
        builder.pop();
        let mut paragraph = builder.build();
        let impl_ = &mut paragraph;

        reporter_assert!(reporter, count == cache.count());
        let found = cache.find_paragraph(impl_);
        reporter_assert!(reporter, found == expectedToBeFound);
        let added = cache.update_paragraph(impl_);
        reporter_assert!(reporter, added != expectedToBeFound);
    };

    text_style.set_font_families(&["Roboto"]);
    test(&text_style, 0, false);
    test(&text_style, 1, true);
    text_style.set_font_families(&["Homemade Apple"]);
    test(&text_style, 1, false);
    test(&text_style, 2, true);
    text_style.set_font_families(&["Noto Color Emoji"]);
    test(&text_style, 2, false);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5528-L5571 (chrome/m156)
def_paragraph_test!(SkParagraph_CacheFontRanges, |reporter| {
    let cache = ParagraphCache::new();
    cache.turn_on(true);
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);

    let mut test = |text1: &str,
                    text2: &str,
                    font1: &str,
                    font2: &str,
                    count: i32,
                    expectedToBeFound: bool| {
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        text_style.set_font_families(&[font1]);
        builder.push_style(&text_style);
        builder.add_text(text1);
        builder.pop();
        text_style.set_font_families(&[font2]);
        builder.push_style(&text_style);
        builder.add_text(text2);
        builder.pop();
        let mut paragraph = builder.build();
        let impl_ = &mut paragraph;

        reporter_assert!(reporter, count == cache.count());
        let found = cache.find_paragraph(impl_);
        reporter_assert!(reporter, found == expectedToBeFound);
        let added = cache.update_paragraph(impl_);
        reporter_assert!(reporter, added != expectedToBeFound);
    };

    test("text", "", "Roboto", "Homemade Apple", 0, false);
    test("t", "ext", "Roboto", "Homemade Apple", 1, false);
    test("te", "xt", "Roboto", "Homemade Apple", 2, false);
    test("tex", "t", "Roboto", "Homemade Apple", 3, false);
    test("text", "", "Roboto", "Homemade Apple", 4, true);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5573-L5611 (chrome/m156)
def_paragraph_test!(SkParagraph_CacheStyles, |reporter| {
    let cache = ParagraphCache::new();
    cache.turn_on(true);
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);

    let text = "text";

    let mut test = |text_style: &TextStyle, count: i32, expectedToBeFound: bool| {
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        builder.push_style(&text_style);
        builder.add_text(text);
        builder.pop();
        let mut paragraph = builder.build();
        let impl_ = &mut paragraph;

        reporter_assert!(reporter, count == cache.count());
        let found = cache.find_paragraph(impl_);
        reporter_assert!(reporter, found == expectedToBeFound);
        let added = cache.update_paragraph(impl_);
        reporter_assert!(reporter, added != expectedToBeFound);
    };

    test(&text_style, 0, false);
    test(&text_style, 1, true);
    text_style.set_letter_spacing(10.0);
    test(&text_style, 1, false);
    test(&text_style, 2, true);
    text_style.set_word_spacing(10.0);
    test(&text_style, 2, false);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5613-L5639 (chrome/m156)
def_paragraph_test!(SkParagraph_ParagraphWithLineBreak, |reporter| {
    let mut font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    font_collection.set_default_font_manager(test_font_mgr());
    font_collection.enable_font_fallback();

    let mut canvas = TestCanvas::new("SkParagraph_ParagraphWithLineBreak.png");

    let paragraph_style = ParagraphStyle::new();
    let mut text_style = TextStyle::new();
    text_style.set_font_size(16.0);
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder.add_text("abc\n\ndef");
    text_style.set_color(Color::BLACK);

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    // Select a position at the second (empty) line
    let pos = paragraph.get_glyph_position_at_coordinate((0.0, 21.0));
    reporter_assert!(
        reporter,
        pos.affinity == Affinity::Downstream && pos.position == 4
    );
    let rect = paragraph.get_rects_for_range(4..5, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, rect.len() == 1 && rect[0].rect.width() == 0.0);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5642-L5660 (chrome/m156)
def_paragraph_test!(SkParagraph_NullInMiddleOfText, |reporter| {
    let mut font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    font_collection.set_default_font_manager(test_font_mgr());

    // `SkString(const char[])` stops at the NUL of the literal.
    let text = "null terminator ->";

    let paragraph_style = ParagraphStyle::new();
    let mut text_style = TextStyle::new();
    text_style.set_color(Color::BLACK);
    text_style.set_font_size(16.0);
    text_style.set_font_families(&["Roboto"]);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder.add_text(text);

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    reporter_assert!(reporter, paragraph.height() > 0.0);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5663-L5680 (chrome/m156)
def_paragraph_test!(SkParagraph_PlaceholderOnly, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let paragraph_style = ParagraphStyle::new();
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    let mut paint = Paint::default();
    paint.set_color(Color::RED);
    text_style.set_background_color(&paint);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let placeholder = PlaceholderStyle::new(
        0.0,
        0.0,
        PlaceholderAlignment::Baseline,
        TextBaseline::Alphabetic,
        0.0,
    );
    builder.add_placeholder(&placeholder);

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    let result = paragraph.get_rects_for_placeholders();
    reporter_assert!(reporter, result.len() == 1);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5682-L5724 (chrome/m156)
def_paragraph_test!(SkParagraph_Fallbacks, |reporter| {
    let mut font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    font_collection.set_default_font_manager_and_family_name(test_font_mgr(), "Arial");
    font_collection.enable_font_fallback();
    let mut canvas = TestCanvas::new("SkParagraph_Fallbacks.png");

    let multiScript = "A1!aÀàĀāƁƀḂⱠꜲꬰəͲἀἏЀЖԠꙐꙮՁخ‎ࡔࠇܦআਉઐଘஇఘಧൺඣᭆᯔᮯ᳇ꠈᜅᩌꪈ༇ꥄꡙꫤ᧰៘꧁꧂ᜰᨏᯤᢆᣭᗗꗃⵞ𐒎߷ጩꬤ𖠺‡₩℻Ⅷ↹⋇⏳ⓖ╋▒◛⚧⑆שׁ🅕㊼龜ポ䷤🂡\n";

    let androidFonts = [
        "sans-serif",
        "sans-serif-condensed",
        "serif",
        "monospace",
        "serif-monospace",
        "casual",
        "cursive",
        "sans-serif-smallcaps",
    ];

    for font in androidFonts {
        let paragraph_style = ParagraphStyle::new();
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

        let mut text_style = TextStyle::new();
        text_style.set_color(Color::BLACK);
        text_style.set_locale("en_US");
        text_style.set_font_size(20.0);

        text_style.set_font_families(&[font]);
        builder.push_style(&text_style);
        builder.add_text(multiScript);

        builder.pop();

        let mut paragraph = builder.build();
        paragraph.layout(TEST_CANVAS_WIDTH);
        paragraph.paint(canvas.get(), (0.0, 0.0));
        canvas.get().translate((0.0, paragraph.height() + 10.0));
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5726-L5776 (chrome/m156)
def_paragraph_test!(SkParagraph_Bidi1, |reporter| {
    let mut font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    font_collection.set_default_font_manager(test_font_mgr());
    font_collection.enable_font_fallback();
    let mut canvas = TestCanvas::new("SkParagraph_Bidi1.png");

    let abc: Vec<u16> = u16s("\u{202D}abc");
    let DEF: Vec<u16> = u16s("\u{202E}DEF");
    let ghi: Vec<u16> = u16s("\u{202D}ghi");
    let JKL: Vec<u16> = u16s("\u{202E}JKL");
    let mno: Vec<u16> = u16s("\u{202D}mno");

    let _abcDEFghiJKLmno: Vec<u16> =
        u16s("\u{202D}abc\u{202E}DEF\u{202D}ghi\u{202E}JKL\u{202D}mno");

    let paragraph_style = ParagraphStyle::new();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["sans-serif"]);
    text_style.set_font_size(40.0);

    text_style.set_color(Color::CYAN);
    text_style.set_font_style(FontStyle::new(Weight::THIN, Width::NORMAL, Slant::Upright));
    builder.push_style(&text_style);
    builder.add_text_utf16(&abc);

    text_style.set_color(Color::GREEN);
    text_style.set_font_style(FontStyle::new(Weight::LIGHT, Width::NORMAL, Slant::Upright));
    builder.push_style(&text_style);
    builder.add_text_utf16(&DEF);

    text_style.set_color(Color::YELLOW);
    text_style.set_font_style(FontStyle::new(
        Weight::NORMAL,
        Width::NORMAL,
        Slant::Upright,
    ));
    builder.push_style(&text_style);
    builder.add_text_utf16(&ghi);

    text_style.set_color(Color::MAGENTA);
    text_style.set_font_style(FontStyle::new(
        Weight::MEDIUM,
        Width::NORMAL,
        Slant::Upright,
    ));
    builder.push_style(&text_style);
    builder.add_text_utf16(&JKL);

    text_style.set_color(Color::BLUE);
    text_style.set_font_style(FontStyle::new(Weight::BLACK, Width::NORMAL, Slant::Upright));
    builder.push_style(&text_style);
    builder.add_text_utf16(&mno);

    let mut paragraph = builder.build();
    paragraph.layout(400.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5778-L5817 (chrome/m156)
def_paragraph_test!(SkParagraph_Bidi2, |reporter| {
    let mut font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    font_collection.set_default_font_manager(test_font_mgr());
    font_collection.enable_font_fallback();
    let mut canvas = TestCanvas::new("SkParagraph_Bidi2.png");

    let abcD: Vec<u16> = u16s("\u{202D}abc\u{202E}D");
    let EFgh: Vec<u16> = u16s("EF\u{202D}gh");
    let iJKLmno: Vec<u16> = u16s("i\u{202E}JKL\u{202D}mno");

    let abcDEFghiJKLmno: Vec<u16> = u16s("\u{202D}abc\u{202E}DEF\u{202D}ghi\u{202E}JKL\u{202D}mno");

    let paragraph_style = ParagraphStyle::new();
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["sans-serif"]);
    text_style.set_font_size(40.0);
    text_style.set_color(Color::BLACK);
    {
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        builder.push_style(&text_style);
        builder.add_text_utf16(&abcD);
        builder.push_style(&text_style);
        builder.add_text_utf16(&EFgh);
        builder.push_style(&text_style);
        builder.add_text_utf16(&iJKLmno);
        let mut paragraph = builder.build();
        paragraph.layout(360.0);
        paragraph.paint(canvas.get(), (0.0, 0.0));
    }
    canvas.get().translate((0.0, 400.0));
    {
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        builder.push_style(&text_style);
        builder.add_text_utf16(&abcDEFghiJKLmno);
        let mut paragraph = builder.build();
        paragraph.layout(360.0);
        paragraph.paint(canvas.get(), (0.0, 0.0));
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5820-L5838 (chrome/m156)
def_paragraph_test!(SkParagraph_NewlineOnly, |reporter| {
    let mut font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    font_collection.set_default_font_manager(test_font_mgr());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_color(Color::BLACK);
    let mut strut_style = StrutStyle::new();
    strut_style.set_strut_enabled(false);
    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_strut_style(strut_style.clone());
    paragraph_style.set_text_style(&text_style);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder.add_text("\n");
    let mut paragraph = builder.build();
    paragraph.layout(1000.0);
    reporter_assert!(reporter, paragraph.height() == 28.0);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5840-L5903 (chrome/m156)
def_paragraph_test!(SkParagraph_FontResolutions, |reporter| {
    let mut canvas = TestCanvas::new("SkParagraph_FontResolutions.png");

    let mut font_collection = TestFontCollection::new(&get_resource_path("fonts"), false, true);
    skip_if_fonts_not_found!(reporter, font_collection);

    if !font_collection.add_font_from_file("abc/abc.ttf", "abc") {
        errorf!(reporter, "abc/abc.ttf not found");
        return;
    }
    if !font_collection.add_font_from_file("abc/abc+grave.ttf", "abc+grave") {
        errorf!(reporter, "abc/abc+grave.ttf not found");
        return;
    }
    if !font_collection.add_font_from_file("abc/abc+agrave.ttf", "abc+agrave") {
        errorf!(reporter, "abc/abc+agrave.ttf not found");
        return;
    }

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["abc"]);
    text_style.set_font_size(50.0);

    let paragraph_style = ParagraphStyle::new();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    text_style.set_font_families(&["abc", "abc+grave"]);
    text_style.set_color(Color::BLUE);
    builder.push_style(&text_style);
    builder.add_text_utf16(&u16s("a\u{0300}")); // FIXME-CONVERT
    text_style.set_color(Color::MAGENTA);
    builder.push_style(&text_style);
    builder.add_text_utf16(&u16s("à")); // FIXME-CONVERT

    text_style.set_font_families(&["abc", "abc+agrave"]);

    text_style.set_color(Color::RED);
    builder.push_style(&text_style);
    builder.add_text_utf16(&u16s("a\u{0300}")); // FIXME-CONVERT
    text_style.set_color(Color::GREEN);
    builder.push_style(&text_style);
    builder.add_text_utf16(&u16s("à")); // FIXME-CONVERT

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.runs().len() == 2);
    if impl_.runs().len() >= 2 {
        reporter_assert!(reporter, impl_.runs().first().unwrap().size() == 4);
        if impl_.runs().first().unwrap().size() >= 4 {
            reporter_assert!(
                reporter,
                impl_.runs().first().unwrap().glyphs()[0]
                    == impl_.runs().first().unwrap().glyphs()[2]
            );
            reporter_assert!(
                reporter,
                impl_.runs().first().unwrap().glyphs()[1]
                    == impl_.runs().first().unwrap().glyphs()[3]
            );
        }

        reporter_assert!(reporter, impl_.runs().last().unwrap().size() == 2);
        if impl_.runs().last().unwrap().size() >= 2 {
            reporter_assert!(
                reporter,
                impl_.runs().last().unwrap().glyphs()[0]
                    == impl_.runs().last().unwrap().glyphs()[1]
            );
        }
    }

    paragraph.paint(canvas.get(), (100.0, 100.0));
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5905-L5941 (chrome/m156)
def_paragraph_test!(SkParagraph_FontStyle, |reporter| {
    let mut canvas = TestCanvas::new("SkParagraph_FontStyle.png");

    let font_collection = TestFontCollection::new(&get_resource_path("fonts"), false, true);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);
    text_style.set_font_size(20.0);
    let fs = FontStyle::new(Weight::LIGHT, Width::NORMAL, Slant::Upright);
    text_style.set_font_style(fs);
    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_style(&text_style);
    let mut boldItalic = TextStyle::new();
    boldItalic.set_font_families(&["Roboto"]);
    boldItalic.set_color(Color::RED);
    let bi = FontStyle::new(Weight::BOLD, Width::NORMAL, Slant::Italic);
    boldItalic.set_font_style(bi);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder.add_text("Default text\n");
    builder.push_style(&boldItalic);
    builder.add_text("Bold and Italic\n");
    builder.pop();
    builder.add_text("back to normal");
    let mut paragraph = builder.build();
    paragraph.layout(250.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5943-L5967 (chrome/m156)
def_paragraph_test!(SkParagraph_Shaping, |reporter| {
    let mut canvas = TestCanvas::new("SkParagraph_Shaping.png");

    let font_collection = TestFontCollection::new(&get_resource_path("fonts"), true, true);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::GRAY);
    text_style.set_font_size(14.0);
    let b = FontStyle::new(Weight::NORMAL, Width::NORMAL, Slant::Upright);
    text_style.set_font_style(b);
    let paragraph_style = ParagraphStyle::new();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder.push_style(&text_style);
    builder.add_text("Eat0 apple0 pies0 | Eat1 apple1 pies1 | Eat2 apple2 pies2");
    let mut paragraph = builder.build();
    paragraph.layout(380.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L5969-L6028 (chrome/m156)
def_paragraph_test!(SkParagraph_Ellipsis, |reporter| {
    let mut font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    font_collection.set_default_font_manager(test_font_mgr());
    let mut canvas = TestCanvas::new("SkParagraph_Ellipsis.png");

    let text = concat!(
        "This\n",
        "is a wrapping test. It should wrap at manual newlines, and if softWrap is true, also at spaces.",
    );
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_color(Color::BLACK);
    text_style.set_font_size(10.0);

    // (Drawn before the closure which borrows the canvas; nothing else draws in between.)
    let mut paint = Paint::default();
    paint.set_color(Color::LIGHT_GRAY);
    canvas
        .get()
        .draw_rect(Rect::from_xywh(0.0, 0.0, 50.0, 500.0), &paint);

    let mut relayout = |lines: usize,
                        ellipsis: bool,
                        width: scalar,
                        height: scalar,
                        minWidth: scalar,
                        maxWidth: scalar,
                        bg: Color| {
        let mut paragraph_style = ParagraphStyle::new();
        let mut paint = Paint::default();
        paint.set_color(bg);
        text_style.set_foreground_color(&paint);
        paragraph_style.set_text_style(&text_style);
        paragraph_style.set_max_lines(lines);
        if ellipsis {
            paragraph_style.set_ellipsis_utf16(&u16s("\u{2026}")); // FIXME-CONVERT
        }
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        builder.add_text(text);
        let mut paragraph = builder.build();
        paragraph.layout(50.0);
        paragraph.paint(canvas.get(), (0.0, 0.0));
        canvas.get().translate((50.0, paragraph.height() + 10.0));
        let result = paragraph.get_rects_for_range(
            0..text.len(),
            RectHeightStyle::Tight,
            RectWidthStyle::Tight,
        );
        let mut background = Paint::default();
        background.set_color(Color::RED);
        background.set_style(PaintStyle::Stroke);
        background.set_anti_alias(true);
        background.set_stroke_width(1.0);
        canvas
            .get()
            .draw_rect(result.first().unwrap().rect, &background);

        debug_assert!(width == paragraph.max_width());
        debug_assert!(height == paragraph.height());
        debug_assert!(nearly_equal!(
            minWidth,
            paragraph.min_intrinsic_width(),
            EPSILON100
        ));
        debug_assert!(nearly_equal!(
            maxWidth,
            paragraph.max_intrinsic_width(),
            EPSILON100
        ));
    };

    relayout(1, false, 50.0, 10.0, 950.0, 950.0, Color::RED);
    relayout(3, false, 50.0, 30.0, 90.0, 950.0, Color::BLUE);
    relayout(usize::MAX, false, 50.0, 200.0, 90.0, 950.0, Color::GREEN); // FIXME-CONVERT

    relayout(1, true, 50.0, 10.0, 950.0, 950.0, Color::YELLOW);
    relayout(3, true, 50.0, 30.0, 90.0, 950.0, Color::MAGENTA);
    relayout(usize::MAX, true, 50.0, 20.0, 950.0, 950.0, Color::CYAN); // FIXME-CONVERT

    relayout(1, false, 50.0, 10.0, 950.0, 950.0, Color::RED);
    relayout(3, false, 50.0, 30.0, 90.0, 950.0, Color::BLUE);
    relayout(usize::MAX, false, 50.0, 200.0, 90.0, 950.0, Color::GREEN); // FIXME-CONVERT
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6030-L6059 (chrome/m156)
def_paragraph_test!(SkParagraph_MemoryLeak, |reporter| {
    let mut font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    font_collection.set_default_font_manager(test_font_mgr());

    let mut text = String::new();
    for _i in 0..10 {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(Color::BLACK);

        let mut textStyle = TextStyle::new();
        textStyle.set_foreground_color(&paint);
        textStyle.set_font_families(&["Roboto"]);

        let mut paragraphStyle = ParagraphStyle::new();
        paragraphStyle.set_text_style(&textStyle);

        let mut builder =
            ParagraphBuilderImpl::new(&paragraphStyle, &font_collection, get_unicode());
        text += "Text ";
        builder.add_text(text.as_str());

        let mut paragraph = builder.build();
        paragraph.layout(100.0);

        //used to add a delay so I can monitor memory usage
        //std::this_thread::sleep_for(std::chrono::milliseconds(1000));
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6061-L6095 (chrome/m156)
def_paragraph_test!(SkParagraph_FormattingInfinity, |reporter| {
    let mut font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    font_collection.set_default_font_manager(test_font_mgr());
    let mut canvas = TestCanvas::new("SkParagraph_FormattingInfinity.png");

    let text = "Some text\nAnother line";

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::BLACK);

    let mut textStyle = TextStyle::new();
    textStyle.set_foreground_color(&paint);
    textStyle.set_font_families(&["Roboto"]);
    let mut paragraphStyle = ParagraphStyle::new();
    paragraphStyle.set_text_style(&textStyle);

    let mut draw = |_prefix: &str, textAlign: TextAlign, textDirection: TextDirection| {
        paragraphStyle.set_text_align(textAlign);
        paragraphStyle.set_text_direction(textDirection);
        let mut builder =
            ParagraphBuilderImpl::new(&paragraphStyle, &font_collection, get_unicode());
        builder.add_text(text);
        let mut paragraph = builder.build();
        paragraph.layout(SCALAR_INFINITY);
        paragraph.paint(canvas.get(), (0.0, 0.0));
        canvas.get().translate((0.0, 100.0));
    };

    draw("left", TextAlign::Left, TextDirection::LTR);
    draw("right", TextAlign::Right, TextDirection::LTR);
    draw("center", TextAlign::Center, TextDirection::LTR);
    draw("justify LTR", TextAlign::Justify, TextDirection::LTR);
    draw("justify RTL", TextAlign::Justify, TextDirection::RTL);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6097-L6113 (chrome/m156)
def_paragraph_test!(SkParagraph_Infinity, |_reporter| {
    debug_assert!(nearly_equal!(1.0, SCALAR_INFINITY) == false);
    debug_assert!(nearly_equal!(1.0, SCALAR_NEGATIVE_INFINITY) == false);
    debug_assert!(nearly_equal!(1.0, SCALAR_NAN) == false);

    debug_assert!(nearly_equal!(SCALAR_INFINITY, SCALAR_INFINITY) == true);
    debug_assert!(nearly_equal!(SCALAR_INFINITY, SCALAR_NEGATIVE_INFINITY) == false);
    debug_assert!(nearly_equal!(SCALAR_INFINITY, SCALAR_NAN) == false);

    debug_assert!(nearly_equal!(SCALAR_NEGATIVE_INFINITY, SCALAR_INFINITY) == false);
    debug_assert!(nearly_equal!(SCALAR_NEGATIVE_INFINITY, SCALAR_NEGATIVE_INFINITY) == true);
    debug_assert!(nearly_equal!(SCALAR_NEGATIVE_INFINITY, SCALAR_NAN) == false);

    debug_assert!(nearly_equal!(SCALAR_NAN, SCALAR_INFINITY) == false);
    debug_assert!(nearly_equal!(SCALAR_NAN, SCALAR_NEGATIVE_INFINITY) == false);
    debug_assert!(nearly_equal!(SCALAR_NAN, SCALAR_NAN) == false);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6115-L6193 (chrome/m156)
def_paragraph_test!(SkParagraph_LineMetrics, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_LineMetrics.png");

    let text = "One line of text\n";
    let len = text.len();

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);

    text_style.set_font_size(8.0);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    text_style.set_font_size(12.0);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    text_style.set_font_size(18.0);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    text_style.set_font_size(30.0);
    builder.push_style(&text_style);
    builder.add_text_bytes(&text.as_bytes()[..len - 1]); // Skip the last \n
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);

    let metrics = paragraph.get_line_metrics();

    let impl_ = &paragraph;
    debug_assert!(metrics.len() == impl_.lines().len());
    for i in 0..metrics.len() {
        let line = &impl_.lines()[i];
        let baseline = metrics[i].baseline;
        let top = line.offset().y;
        let bottom = line.offset().y + line.height();
        debug_assert!(baseline > f64::from(top) && baseline <= f64::from(bottom));
    }

    paragraph.paint(canvas.get(), (0.0, 0.0));
    let rects =
        paragraph.get_rects_for_range(0..len * 4, RectHeightStyle::Max, RectWidthStyle::Tight);

    let mut red = Paint::default();
    red.set_color(Color::RED);
    red.set_style(PaintStyle::Stroke);
    red.set_anti_alias(true);
    red.set_stroke_width(1.0);

    for rect in rects {
        canvas.get().draw_rect(rect.rect, &red);
    }

    let mut green = Paint::default();
    green.set_color(Color::GREEN);
    green.set_style(PaintStyle::Stroke);
    green.set_anti_alias(true);
    green.set_stroke_width(1.0);
    for metric in metrics {
        let x0 = 0.0;
        let x1 = metric.width;
        let y = metric.baseline;
        canvas
            .get()
            .draw_line((x0, y as scalar), (x1 as scalar, y as scalar), &green);
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6228-L6271 (chrome/m156)
def_paragraph_test!(SkParagraph_LineMetricsTextAlign, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_LineMetricsTextAlign.png");

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);

    let mut layout = |text_align: TextAlign| -> (scalar, scalar) {
        paragraph_style.set_text_align(text_align);
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        builder.push_style(&text_style);
        builder.add_text("Some text that takes more than 200 px");
        let mut paragraph = builder.build();
        paragraph.layout(200.0);
        paragraph.paint(canvas.get(), (0.0, 0.0));
        canvas.get().translate((0.0, paragraph.height()));
        let metrics = paragraph.get_line_metrics();
        reporter_assert!(reporter, metrics.len() > 0);
        (metrics[0].left as scalar, metrics[0].width as scalar)
    };

    let mut left: [scalar; 4] = [0.0; 4];
    let mut width: [scalar; 4] = [0.0; 4];
    (left[0], width[0]) = layout(TextAlign::Left);
    (left[1], width[1]) = layout(TextAlign::Center);
    (left[2], width[2]) = layout(TextAlign::Right);
    (left[3], width[3]) = layout(TextAlign::Justify);

    // delta = line_width - text_width
    reporter_assert!(reporter, left[0] == 0.0); // Starts from 0
    reporter_assert!(reporter, left[1] > left[0]); // Starts from delta / 2
    reporter_assert!(reporter, left[2] > left[1]); // Starts from delta
    reporter_assert!(reporter, left[3] == left[0]); // Starts from 0
    reporter_assert!(reporter, width[1] == width[0]);
    reporter_assert!(reporter, width[2] == width[0]);
    reporter_assert!(reporter, width[3] > width[0]); // delta == 0
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6273-L6301 (chrome/m156)
def_paragraph_test!(SkParagraph_FontResolutionInRTL, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_FontResolutionInRTL.png");
    let text = " אאא בּבּבּבּ אאאא בּבּ אאא בּבּבּ אאאאא בּבּבּבּ אאאא בּבּבּבּבּ ";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(14);
    paragraph_style.set_text_align(TextAlign::Right);
    paragraph_style.set_text_direction(TextDirection::RTL);
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(26.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.runs().len() == (10 + 11));
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6303-L6335 (chrome/m156)
def_paragraph_test!(SkParagraph_FontResolutionInLTR, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_FontResolutionInLTR.png");
    let text = u16s("abc \u{01A2} \u{01A2} def");

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(14);
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(26.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text_utf16(&text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.runs().len() == 5);
    if impl_.runs().len() >= 5 {
        reporter_assert!(reporter, impl_.runs()[0].text_range().width() == 4); // "abc "
        reporter_assert!(reporter, impl_.runs()[1].text_range().width() == 2); // "{unresolved}"
        reporter_assert!(reporter, impl_.runs()[2].text_range().width() == 1); // " "
        reporter_assert!(reporter, impl_.runs()[3].text_range().width() == 2); // "{unresolved}"
        reporter_assert!(reporter, impl_.runs()[4].text_range().width() == 4); // " def"
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6337-L6356 (chrome/m156)
def_paragraph_test!(SkParagraph_Intrinsic, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let text = "a".repeat(3000);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Google Sans"]);
    text_style.set_font_size(12.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(&text);

    let mut paragraph = builder.build();
    paragraph.layout(300000.0);
    reporter_assert!(
        reporter,
        paragraph.min_intrinsic_width() <= paragraph.max_intrinsic_width()
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6359-L6420 (chrome/m156)
def_paragraph_test!(SkParagraph_NoCache1, |reporter| {
    let cache = ParagraphCache::new();
    cache.turn_on(true);

    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);
    // Long arabic text with english spaces
    let text = concat!(
        "من أسر وإعلان الخاصّة وهولندا،, عل قائمة الضغوط بالمطالبة تلك. الصفحة ",
        "من أسر وإعلان الخاصّة وهولندا،, عل قائمة الضغوط بالمطالبة تلك. الصفحة ",
        "من أسر وإعلان الخاصّة وهولندا،, عل قائمة الضغوط بالمطالبة تلك. الصفحة ",
        "من أسر وإعلان الخاصّة وهولندا،, عل قائمة الضغوط بالمطالبة تلك. الصفحة ",
        "من أسر وإعلان الخاصّة وهولندا،, عل قائمة الضغوط بالمطالبة تلك. الصفحة ",
        "عل بمباركة التقليدية قام عن. تصفح",
    );

    let mut str = String::new();

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_direction(TextDirection::LTR);
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(14.0);
    text_style.set_color(Color::BLACK);

    let mut test = |test: Option<&str>, text: &str, editing: bool| {
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        //SkDebugf("test %s:\n", test);
        builder.push_style(&text_style);
        builder.add_text(text);
        builder.pop();

        let cache = font_collection.paragraph_cache();
        let countBefore = cache.count();
        let mut paragraph = builder.build();
        paragraph.layout(TEST_CANVAS_WIDTH);
        let countAfter = cache.count();

        if test.is_none() {
            return;
        }

        reporter_assert!(reporter, (countBefore == countAfter) == editing);
    };

    str.push_str(text);
    test(Some("Long arabic text"), str.as_str(), false);

    str.push_str("عل");
    test(Some("+2 character at the end"), str.as_str(), true);

    str = text.to_owned();
    test(Some("-2 characters from the end"), str.as_str(), true);

    str.insert_str(0, "عل");
    test(Some("+2 character at the start"), str.as_str(), true);

    test(Some("-2 characters from the start"), text, true);

    // Make sure that different strings are not flagged as editing
    test(
        Some("different strings"),
        "0123456789 0123456789 0123456789 0123456789 0123456789",
        false,
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6422-L6452 (chrome/m156)
def_paragraph_test!(SkParagraph_HeightCalculations, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_HeightCalculations.png");

    let mut draw = |hb: TextHeightBehavior, text: &str, height: scalar| {
        let mut paragraph_style = ParagraphStyle::new();
        paragraph_style.set_text_height_behavior(hb);

        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        let mut text_style = TextStyle::new();
        text_style.set_font_families(&["Roboto"]);
        text_style.set_font_size(14.0);
        text_style.set_height(5.0);
        text_style.set_height_override(true);
        text_style.set_color(Color::BLACK);
        builder.push_style(&text_style);
        builder.add_text(text);

        let mut paragraph = builder.build();
        paragraph.layout(500.0);
        paragraph.paint(canvas.get(), (0.0, 0.0));
        canvas.get().translate((0.0, paragraph.height()));
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(paragraph.height(), height)
        );
    };

    draw(TextHeightBehavior::All, "Hello\nLine 2\nLine 3", 210.0);
    draw(
        TextHeightBehavior::DisableAll,
        "Hello\nLine 2\nLine 3",
        157.0,
    );
    draw(TextHeightBehavior::DisableFirstAscent, "Hello", 28.0);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6454-L6492 (chrome/m156)
def_paragraph_test!(SkParagraph_RTL_With_Styles, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_RTL_With_Styles.png");

    let mut whiteSpaces = Paint::default();
    whiteSpaces.set_color(Color::LIGHT_GRAY);

    let mut breakingSpace = Paint::default();
    breakingSpace.set_color(Color::YELLOW);

    let mut text = Paint::default();
    text.set_color(Color::WHITE);

    let arabic = "قففغغغغقففغغغغقففغغغ";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_align(TextAlign::Right);
    let mut text_style = TextStyle::new();
    text_style.set_color(Color::BLACK);
    text_style.set_font_families(&["Roboto"]);

    paragraph_style.set_text_direction(TextDirection::RTL);
    paragraph_style.set_text_align(TextAlign::Right);
    text_style.set_font_size(20.0);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    text_style.set_background_color(&whiteSpaces);
    builder.push_style(&text_style);
    builder.add_text("   ");
    text_style.set_background_color(&text);
    builder.push_style(&text_style);
    builder.add_text(arabic);

    let mut paragraph = builder.build();
    paragraph.layout(300.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6494-L6547 (chrome/m156)
def_paragraph_test!(SkParagraph_PositionInsideEmoji, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_PositionInsideEmoji.png");

    let text: Vec<u16> = u16s(
        "\u{1f469}\u{200D}\u{1f469}\u{200D}\u{1f467}\u{200D}\u{1f467}\u{1f469}\u{1f469}\u{1f467}\u{1f467}",
    );

    let paragraph_style = ParagraphStyle::new();
    let mut text_style = TextStyle::new();
    text_style.set_color(Color::BLACK);
    text_style.set_font_families(&["Noto Color Emoji"]);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder.push_style(&text_style);
    builder.add_text_utf16(&text);

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    // UTF8       UTF16
    // 4          [0:2)
    // 3 + 4      [2:5)
    // 3 + 4      [5:8)
    // 3 + 4      [8:11)
    // 4          [11:13)
    // 4          [13:15)
    // 4          [15:17)
    // 4          [17:19)

    let _family =
        paragraph.get_rects_for_range(0..11, RectHeightStyle::Tight, RectWidthStyle::Tight);
    let _face01 =
        paragraph.get_rects_for_range(11..13, RectHeightStyle::Tight, RectWidthStyle::Tight);
    let _face02 =
        paragraph.get_rects_for_range(13..15, RectHeightStyle::Tight, RectWidthStyle::Tight);
    let _face03 =
        paragraph.get_rects_for_range(15..17, RectHeightStyle::Tight, RectWidthStyle::Tight);
    let _face04 =
        paragraph.get_rects_for_range(17..19, RectHeightStyle::Tight, RectWidthStyle::Tight);

    let words: [usize; 6] = [11, 13, 15, 17, 19, 21];
    let mut j: usize = 0;
    for i in words {
        let rects =
            paragraph.get_rects_for_range(j..i, RectHeightStyle::Tight, RectWidthStyle::Tight);
        if rects.is_empty() {
            continue;
        }
        let X = rects[0].rect.center_x();
        let Y = rects[0].rect.center_y();
        let res1 = paragraph.get_glyph_position_at_coordinate((X - 5.0, Y));
        //SkDebugf("[%d:%d) @%f,%f: %d %s\n", j, i, X - 5, Y, res1.position, res1.affinity == Affinity::kDownstream ? "D" : "U");
        let res2 = paragraph.get_glyph_position_at_coordinate((X + 5.0, Y));
        //SkDebugf("[%d:%d) @%f,%f: %d %s\n\n", j, i, X + 5, Y, res2.position, res2.affinity == Affinity::kDownstream ? "D" : "U");
        reporter_assert!(
            reporter,
            i as i32 == res2.position && res1.position == j as i32
        );
        j = i;
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6549-L6576 (chrome/m156)
def_paragraph_test!(SkParagraph_SingleLineHeight1, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_SingleLineHeight1.png");

    let mut paint = |text: &str| {
        let mut paragraph_style = ParagraphStyle::new();
        paragraph_style.set_text_height_behavior(TextHeightBehavior::DisableAll);
        paragraph_style.set_max_lines(1);
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        let mut text_style = TextStyle::new();
        text_style.set_color(Color::BLACK);
        text_style.set_font_families(&["Ahem"]);
        text_style.set_font_size(14.0);
        text_style.set_height(2.0);
        text_style.set_height_override(true);
        builder.push_style(&text_style);
        builder.add_text(text);
        let mut paragraph = builder.build();
        paragraph.layout(80.0);
        paragraph.paint(canvas.get(), (0.0, 0.0));
        reporter_assert!(reporter, paragraph.height() == 14.0);
    };

    paint("Loooooooooooooooooooooooooooooooooooong text");
    paint("");
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6578-L6604 (chrome/m156)
def_paragraph_test!(SkParagraph_SingleLineHeight2, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_SingleLineHeight2.png");

    let mut paint = |text: &str| {
        let mut paragraph_style = ParagraphStyle::new();
        paragraph_style.set_max_lines(1);
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        let mut text_style = TextStyle::new();
        text_style.set_color(Color::BLACK);
        text_style.set_font_families(&["Ahem"]);
        text_style.set_font_size(14.0);
        text_style.set_height(2.0);
        text_style.set_height_override(true);
        builder.push_style(&text_style);
        builder.add_text(text);
        let mut paragraph = builder.build();
        paragraph.layout(80.0);
        paragraph.paint(canvas.get(), (0.0, 0.0));
        reporter_assert!(reporter, paragraph.height() == 28.0);
    };

    paint("Loooooooooooooooooooooooooooooooooooong text");
    paint("");
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6606-L6648 (chrome/m156)
def_paragraph_test!(SkParagraph_PlaceholderWidth, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_PlaceholderWidth.png");

    let text = "1 23 456 7890";

    let paragraph_style = ParagraphStyle::new();
    let mut text_style = TextStyle::new();
    text_style.set_color(Color::BLACK);
    text_style.set_font_size(50.0);
    text_style.set_font_families(&["Ahem"]);
    let placeholder = PlaceholderStyle::new(
        300.0,
        50.0,
        PlaceholderAlignment::Baseline,
        TextBaseline::Alphabetic,
        0.0,
    );

    let mut draw = |withPlaceholder: bool| {
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        builder.push_style(&text_style);
        builder.add_text(text);
        if withPlaceholder {
            let mut red = Paint::default();
            red.set_color(Color::RED);
            text_style.set_background_color(&red);
            builder.push_style(&text_style);
            builder.add_placeholder(&placeholder);
        }
        builder.add_text(text);

        let mut paragraph = builder.build();
        paragraph.layout(950.0);
        paragraph.paint(canvas.get(), (0.0, 0.0));
        canvas.get().translate((0.0, paragraph.height()));
        return paragraph.min_intrinsic_width();
    };

    let len1 = draw(true);
    let len2 = draw(false);

    // placeholder: 300 "78901": 250
    reporter_assert!(reporter, sk_scalar_nearly_equal!(len1, 300.0, EPSILON100));
    reporter_assert!(reporter, sk_scalar_nearly_equal!(len2, 250.0, EPSILON100));
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6650-L6680 (chrome/m156)
def_paragraph_test!(SkParagraph_GlyphPositionsInEmptyLines, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_GlyphPositionsInEmptyLines.png");
    let paragraph_style = ParagraphStyle::new();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text("A\n\n");
    builder.pop();
    let mut paragraph = builder.build();
    paragraph.layout(300.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let res1 = paragraph.get_glyph_position_at_coordinate((paragraph.min_intrinsic_width(), 1.0));
    reporter_assert!(
        reporter,
        res1.position == 1 && res1.affinity == Affinity::Upstream
    );

    let res2 = paragraph.get_glyph_position_at_coordinate((0.0, paragraph.height() * 0.5));
    reporter_assert!(
        reporter,
        res2.position == 2 && res2.affinity == Affinity::Downstream
    );

    let res3 = paragraph.get_glyph_position_at_coordinate((0.0, paragraph.height() - 1.0));
    reporter_assert!(
        reporter,
        res3.position == 3 && res3.affinity == Affinity::Downstream
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6682-L6720 (chrome/m156)
def_paragraph_test!(SkParagraph_RTLGlyphPositions, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_RTLGlyphPositions.png");
    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_direction(TextDirection::RTL);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text("אאאא");
    builder.pop();
    let mut paragraph = builder.build();
    paragraph.layout(500.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let checks: Vec<(scalar, PositionWithAffinity)> = vec![
        (
            550.0,
            PositionWithAffinity {
                position: 0,
                affinity: Affinity::Downstream,
            },
        ),
        (
            500.0,
            PositionWithAffinity {
                position: 0,
                affinity: Affinity::Downstream,
            },
        ),
        (
            494.0,
            PositionWithAffinity {
                position: 1,
                affinity: Affinity::Upstream,
            },
        ),
        (
            488.0,
            PositionWithAffinity {
                position: 1,
                affinity: Affinity::Downstream,
            },
        ),
        (
            485.0,
            PositionWithAffinity {
                position: 2,
                affinity: Affinity::Upstream,
            },
        ),
        (
            480.0,
            PositionWithAffinity {
                position: 2,
                affinity: Affinity::Downstream,
            },
        ),
        (
            475.0,
            PositionWithAffinity {
                position: 3,
                affinity: Affinity::Upstream,
            },
        ),
        (
            471.0,
            PositionWithAffinity {
                position: 3,
                affinity: Affinity::Downstream,
            },
        ),
        (
            467.0,
            PositionWithAffinity {
                position: 4,
                affinity: Affinity::Upstream,
            },
        ),
        (
            0.0,
            PositionWithAffinity {
                position: 4,
                affinity: Affinity::Upstream,
            },
        ),
    ];

    for check in checks {
        let pos = paragraph.get_glyph_position_at_coordinate((check.0, 0.0));
        reporter_assert!(reporter, pos.affinity == check.1.affinity);
        reporter_assert!(reporter, pos.position == check.1.position);
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6722-L6751 (chrome/m156)
def_paragraph_test!(SkParagraph_RTLGlyphPositionsInEmptyLines, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_RTLGlyphPositionsInEmptyLines.png");

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_direction(TextDirection::RTL);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    //builder.addText("בבבב\n\nאאאא");
    builder.add_text("בבבב\n\nאאאא");
    builder.pop();
    let mut paragraph = builder.build();
    paragraph.layout(500.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let height = paragraph.height();
    let res1 = paragraph.get_glyph_position_at_coordinate((0.0, 0.0));
    reporter_assert!(
        reporter,
        res1.position == 4 && res1.affinity == Affinity::Upstream
    );
    let res2 = paragraph.get_glyph_position_at_coordinate((0.0, height / 2.0));
    reporter_assert!(
        reporter,
        res2.position == 5 && res2.affinity == Affinity::Downstream
    );
    let res3 = paragraph.get_glyph_position_at_coordinate((0.0, height));
    reporter_assert!(
        reporter,
        res3.position == 10 && res3.affinity == Affinity::Upstream
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6753-L6792 (chrome/m156)
def_paragraph_test!(SkParagraph_LTRGlyphPositionsForTrailingSpaces, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_LTRGlyphPositionsForTrailingSpaces.png");

    let paragraph_style = ParagraphStyle::new();
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(10.0);
    text_style.set_color(Color::BLACK);

    let mut test = |text: &str| {
        let str = straight(text);
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        builder.push_style(&text_style);
        builder.add_text_utf16(&str);
        builder.pop();
        let gray = Paint::default();
        let mut paragraph = builder.build();
        paragraph.layout(100.0);
        canvas.get().translate((0.0, 20.0));
        canvas.get().draw_rect(
            Rect::from_xywh(
                0.0,
                0.0,
                paragraph.max_intrinsic_width(),
                paragraph.height(),
            ),
            &gray,
        );
        paragraph.paint(canvas.get(), (0.0, 0.0));
        canvas.get().translate((0.0, paragraph.height()));

        for i in 0..str.len() {
            let res = paragraph.get_glyph_position_at_coordinate(((i * 10) as scalar, 2.0));
            //SkDebugf(if "@%f[%d]: %d %s\n", i * 10.0f, i, res.position, res.affinity == Affinity::kDownstream { "D" } else { "U" });
            // There is a hidden codepoint at the beginning (to make it symmetric to RTL)
            reporter_assert!(
                reporter,
                res.position == (i) as i32 + (if i > 0 { 1 } else { 0 })
            );
            // The ending looks slightly different...
            reporter_assert!(
                reporter,
                res.affinity
                    == (if res.position == str.len() as i32 {
                        Affinity::Upstream
                    } else {
                        Affinity::Downstream
                    })
            );
        }
    };

    test("    ");
    test("hello               ");
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6794-L6849 (chrome/m156)
def_paragraph_test!(SkParagraph_RTLGlyphPositionsForTrailingSpaces, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_RTLGlyphPositionsForTrailingSpaces.png");

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_direction(TextDirection::RTL);
    paragraph_style.set_text_align(TextAlign::Right);
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(10.0);
    text_style.set_color(Color::BLACK);
    canvas.get().translate((200.0, 0.0));

    let mut test = |text: &str, whitespaces: i32| {
        let str = mirror(text);
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        builder.push_style(&text_style);
        builder.add_text_utf16(&str);
        builder.pop();
        let mut gray = Paint::default();
        let mut paragraph = builder.build();
        paragraph.layout(100.0);
        canvas.get().translate((0.0, 20.0));
        let res = paragraph.get_rects_for_range(
            0..str.len(),
            RectHeightStyle::Tight,
            RectWidthStyle::Tight,
        );
        let mut even: bool = true;
        for r in res {
            if even {
                gray.set_color(Color::GRAY);
            } else {
                gray.set_color(Color::LIGHT_GRAY);
            }
            even = !even;
            canvas.get().draw_rect(r.rect, &gray);
        }
        gray.set_color(Color::RED);
        canvas
            .get()
            .draw_rect(Rect::from_xywh(0.0, 0.0, 1.0, paragraph.height()), &gray);
        paragraph.paint(canvas.get(), (0.0, 0.0));
        canvas.get().translate((0.0, paragraph.height()));

        for i in 0..str.len() as i32 {
            // Additional 1.0f to make sure the offset is not too close to the
            // edge of glyphs.
            let pointX = ((whitespaces + i) * 10) as scalar + 1.0;
            let pos = paragraph.get_glyph_position_at_coordinate((pointX, 2.0));
            //SkDebugf(if "@%f[%d]: %d %s\n", pointX, i, pos.position, pos.affinity == Affinity::kDownstream { "D" } else { "U" });
            // At the beginning there is a control codepoint that makes the string RTL
            reporter_assert!(
                reporter,
                (pos.position + i)
                    == str.len() as i32
                        - (if pos.affinity == Affinity::Downstream {
                            1
                        } else {
                            0
                        })
            );
        }
    };

    test("    ", 6);
    test("               hello", -10);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6851-L6891 (chrome/m156)
def_paragraph_test!(
    SkParagraph_LTRLineMetricsDoesNotIncludeNewLine,
    |reporter| {
        let font_collection = ResourceFontCollection::new(false);
        skip_if_fonts_not_found!(reporter, font_collection);

        let mut canvas = TestCanvas::new("SkParagraph_LTRLineMetricsDoesNotIncludeNewLine.png");

        let mut paragraph_style = ParagraphStyle::new();
        paragraph_style.set_text_direction(TextDirection::RTL);
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        let mut text_style = TextStyle::new();
        text_style.set_font_families(&["Roboto"]);
        text_style.set_font_size(20.0);
        text_style.set_color(Color::BLACK);
        builder.push_style(&text_style);
        builder.add_text("one two\n\nthree four\nwith spaces     \n    \n______________________");
        builder.pop();
        let mut paragraph = builder.build();
        paragraph.layout(190.0);
        paragraph.paint(canvas.get(), (0.0, 0.0));

        let expected: Vec<(usize, usize, usize, usize)> = vec![
            (0, 7, 7, 8),     // one two\n
            (8, 8, 8, 9),     // \n
            (9, 19, 19, 20),  // three four\n
            (20, 31, 36, 37), // with spaces    \n
            (37, 37, 41, 42), //      { just spaces }\n
            (42, 63, 63, 63), // _____________________
            (63, 64, 64, 64), // _
        ];
        let metrics = paragraph.get_line_metrics();
        for metric in metrics {
            //SkDebugf("Line[%d:%d <= %d <=%d)\n", metric.start_index, metric.end_excluding_whitespaces, metric.end_index, metric.end_including_newline);
            let result = expected[metric.line_number];
            reporter_assert!(reporter, metric.start_index == result.0);
            reporter_assert!(reporter, metric.end_excluding_whitespaces == result.1);
            reporter_assert!(reporter, metric.end_index == result.2);
            reporter_assert!(reporter, metric.end_including_newline == result.3);
        }
    }
);

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6893-L6966 (chrome/m156)
def_paragraph_test!(
    SkParagraph_RTLLineMetricsDoesNotIncludeNewLine,
    |reporter| {
        let font_collection = ResourceFontCollection::new(false);
        skip_if_fonts_not_found!(reporter, font_collection);

        let mut canvas = TestCanvas::new("SkParagraph_RTLLineMetricsDoesNotIncludeNewLine.png");
        canvas.get().translate((100.0, 100.0));

        let mut paragraph_style = ParagraphStyle::new();
        paragraph_style.set_text_direction(TextDirection::RTL);
        paragraph_style.set_text_align(TextAlign::Right);
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        let mut text_style = TextStyle::new();
        text_style.set_font_families(&["Roboto"]);
        text_style.set_font_size(20.0);
        text_style.set_color(Color::BLACK);
        builder.push_style(&text_style);
        builder.add_text_utf16(&mirror(
            "______________________\none two\n\nthree four\nwith spaces     \n    ",
        ));
        builder.pop();
        let mut paragraph = builder.build();
        paragraph.layout(190.0);
        paragraph.paint(canvas.get(), (0.0, 0.0));
        //auto impl = &paragraph;

        let mut gray = Paint::default();
        gray.set_color(Color::GRAY);
        gray.set_style(PaintStyle::Stroke);
        gray.set_anti_alias(true);
        gray.set_stroke_width(1.0);
        canvas.get().draw_rect(
            Rect::from_xywh(0.0, 0.0, paragraph.max_width(), paragraph.height()),
            &gray,
        );

        let mut red = Paint::default();
        red.set_color(Color::RED);
        red.set_style(PaintStyle::Stroke);
        red.set_anti_alias(true);
        red.set_stroke_width(1.0);

        let mut blue = Paint::default();
        blue.set_color(Color::RED);
        blue.set_style(PaintStyle::Stroke);
        blue.set_anti_alias(true);
        blue.set_stroke_width(1.0);

        let boxes =
            paragraph.get_rects_for_range(0..100, RectHeightStyle::Tight, RectWidthStyle::Tight);
        let mut even: bool = false;
        for box_ in boxes {
            canvas
                .get()
                .draw_rect(box_.rect, if even { &red } else { &blue });
            even = !even;
        }

        // RTL codepoint u"\u202E" messes everything up
        // (adds one invisible codepoint to the first line
        // and shift all the indexes by 1 right)
        let expected: Vec<(i32, i32, i32, i32)> = vec![
            (0, 1, 5, 6), //      { just spaces; the end of the text considered as a new line in libtxt?!? }
            (6, 22, 22, 23), // with spaces    \n
            (23, 33, 33, 34), // three four\n
            (34, 34, 34, 35), // \n
            (35, 42, 42, 43), // one two\n
            (43, 64, 64, 64), // _____________________
            (64, 65, 65, 65), // _
        ];

        let metrics = paragraph.get_line_metrics();
        for metric in metrics {
            //SkDebugf("Line[%d:%d <= %d <=%d]\n", metric.start_index, metric.end_excluding_whitespaces, metric.end_index, metric.end_including_newline);
            let result = expected[metric.line_number];
            reporter_assert!(reporter, metric.start_index == (result.0) as usize);
            reporter_assert!(
                reporter,
                metric.end_excluding_whitespaces == (result.1) as usize
            );
            reporter_assert!(reporter, metric.end_index == (result.2) as usize);
            reporter_assert!(
                reporter,
                metric.end_including_newline == (result.3) as usize
            );
        }
    }
);

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L6968-L6998 (chrome/m156)
def_paragraph_test!(SkParagraph_PlaceholderPosition, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_PlaceholderPosition.png");
    canvas.get().translate((100.0, 100.0));

    let mut text_style = TextStyle::new();
    text_style.set_color(Color::BLACK);
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(10.0);
    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_style(&text_style);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder.push_style(&text_style);
    builder.add_text("abcd");

    let mut placeholder_style = PlaceholderStyle::default();
    placeholder_style.height = 10.0;
    placeholder_style.width = 10.0;
    placeholder_style.baseline = TextBaseline::Alphabetic;
    placeholder_style.alignment = PlaceholderAlignment::Bottom;
    builder.add_placeholder(&placeholder_style);

    let mut paragraph = builder.build();
    paragraph.layout(500.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let result = paragraph.get_glyph_position_at_coordinate((41.0, 0.0));
    reporter_assert!(
        reporter,
        result.position == 4 && result.affinity == Affinity::Downstream
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7000-L7039 (chrome/m156)
def_paragraph_test!(SkParagraph_LineEnd, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_LineEnd.png");
    canvas.get().translate((100.0, 100.0));

    let mut text_style = TextStyle::new();
    text_style.set_color(Color::BLACK);
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(10.0);
    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_style(&text_style);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder.push_style(&text_style);
    builder.add_text("Hello ");
    builder.add_text("hello   ");
    builder.add_text("hello\n");
    builder.add_text("hello   \n");
    builder.add_text("world");

    let mut paragraph = builder.build();
    paragraph.layout(60.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let lm = paragraph.get_line_metrics();
    /*
    for lm in lm {
        SkDebugf("%d %d %d\n", (int)lm.end_excluding_whitespaces, (int)lm.end_index, (int)lm.end_including_newline);  // FIXME-CONVERT
    }
    */
    reporter_assert!(reporter, lm.len() >= 4);
    if lm.len() >= 4 {
        reporter_assert!(
            reporter,
            lm[0].end_excluding_whitespaces == 05
                && lm[0].end_index == 06
                && lm[0].end_including_newline == 06
        );
        reporter_assert!(
            reporter,
            lm[1].end_excluding_whitespaces == 11
                && lm[1].end_index == 14
                && lm[1].end_including_newline == 14
        );
        reporter_assert!(
            reporter,
            lm[2].end_excluding_whitespaces == 19
                && lm[2].end_index == 19
                && lm[2].end_including_newline == 20
        );
        reporter_assert!(
            reporter,
            lm[3].end_excluding_whitespaces == 25
                && lm[3].end_index == 28
                && lm[3].end_including_newline == 29
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7041-L7070 (chrome/m156)
def_paragraph_test!(SkParagraph_Utf16Indexes, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_Utf16Indexes.png");
    canvas.get().translate((100.0, 100.0));

    let mut text_style = TextStyle::new();
    text_style.set_color(Color::BLACK);
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(10.0);
    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_style(&text_style);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder.push_style(&text_style);
    builder.add_text("áéíóú\nxxxx");
    let mut paragraph = builder.build();
    paragraph.layout(60.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));
    let lm = paragraph.get_line_metrics();
    //for lm in lm {
    //    SkDebugf("%d %d %d\n", (int)lm.end_excluding_whitespaces, (int)lm.end_index, (int)lm.end_including_newline);
    //}
    reporter_assert!(reporter, lm.len() == 2);
    if lm.len() >= 2 {
        reporter_assert!(
            reporter,
            lm[0].end_excluding_whitespaces == 05
                && lm[0].end_index == 05
                && lm[0].end_including_newline == 06
        );
        reporter_assert!(
            reporter,
            lm[1].end_excluding_whitespaces == 10
                && lm[1].end_index == 10
                && lm[1].end_including_newline == 10
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7072-L7127 (chrome/m156)
def_paragraph_test!(SkParagraph_RTLFollowedByLTR, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_RTLFollowedByLTR.png");
    canvas.get().translate((100.0, 100.0));

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(10.0);
    text_style.set_color(Color::BLACK);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_style(&text_style);
    paragraph_style.set_text_direction(TextDirection::LTR);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder.push_style(&text_style);
    builder.add_text_utf16(&u16s("\u{05D0}\u{05D0}\u{05D0}ABC")); // FIXME-CONVERT
    let mut paragraph = builder.build();
    paragraph.layout(100.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let boxes = paragraph.get_rects_for_range(
        0..paragraph.max_width() as usize,
        RectHeightStyle::Tight,
        RectWidthStyle::Tight,
    );
    reporter_assert!(reporter, boxes.len() == 2);
    reporter_assert!(
        reporter,
        boxes[0].direct == TextDirection::RTL && boxes[1].direct == TextDirection::LTR
    );
    reporter_assert!(reporter, sk_scalar_nearly_equal!(boxes[0].rect.left, 0.0));
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(boxes[0].rect.right, boxes[1].rect.left)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(boxes[1].rect.right, paragraph.max_intrinsic_width())
    );

    let checks: Vec<(scalar, PositionWithAffinity)> = vec![
        (
            -10.0,
            PositionWithAffinity {
                position: 3,
                affinity: Affinity::Upstream,
            },
        ),
        (
            0.0,
            PositionWithAffinity {
                position: 3,
                affinity: Affinity::Upstream,
            },
        ),
        (
            5.0,
            PositionWithAffinity {
                position: 2,
                affinity: Affinity::Downstream,
            },
        ),
        (
            10.0,
            PositionWithAffinity {
                position: 2,
                affinity: Affinity::Upstream,
            },
        ),
        (
            15.0,
            PositionWithAffinity {
                position: 1,
                affinity: Affinity::Downstream,
            },
        ),
        (
            20.0,
            PositionWithAffinity {
                position: 1,
                affinity: Affinity::Upstream,
            },
        ),
        (
            25.0,
            PositionWithAffinity {
                position: 0,
                affinity: Affinity::Downstream,
            },
        ),
        (
            30.0,
            PositionWithAffinity {
                position: 3,
                affinity: Affinity::Downstream,
            },
        ),
        (
            35.0,
            PositionWithAffinity {
                position: 4,
                affinity: Affinity::Upstream,
            },
        ),
        (
            40.0,
            PositionWithAffinity {
                position: 4,
                affinity: Affinity::Downstream,
            },
        ),
        (
            45.0,
            PositionWithAffinity {
                position: 5,
                affinity: Affinity::Upstream,
            },
        ),
        (
            50.0,
            PositionWithAffinity {
                position: 5,
                affinity: Affinity::Downstream,
            },
        ),
        (
            55.0,
            PositionWithAffinity {
                position: 6,
                affinity: Affinity::Upstream,
            },
        ),
        (
            60.0,
            PositionWithAffinity {
                position: 6,
                affinity: Affinity::Upstream,
            },
        ),
    ];

    for check in checks {
        let pos = paragraph.get_glyph_position_at_coordinate((check.0, 0.0));
        reporter_assert!(reporter, pos.affinity == check.1.affinity);
        reporter_assert!(reporter, pos.position == check.1.position);
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7129-L7177 (chrome/m156)
def_paragraph_test!(SkParagraph_StrutTopLine, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_StrutTopLine.png");

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(10.0);
    let mut black = Paint::default();
    black.set_color(Color::BLACK);
    text_style.set_foreground_color(&black);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_style(&text_style);
    paragraph_style.set_text_direction(TextDirection::LTR);
    let mut strut_style = StrutStyle::new();
    strut_style.set_strut_enabled(true);
    strut_style.set_font_families(&["Ahem"]);
    strut_style.set_font_size(16.0);
    strut_style.set_height(4.0);
    strut_style.set_height_override(true);
    strut_style.set_leading(-1.0);
    strut_style.set_force_strut_height(true);
    paragraph_style.set_strut_style(strut_style.clone());
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    builder.push_style(&text_style);
    builder.add_text_utf16(&u16s("Atwater Peel Sherbrooke Bonaventure\nhi\nwasssup!")); // FIXME-CONVERT

    let mut paragraph = builder.build();
    paragraph.layout(797.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));
    let boxes = paragraph.get_rects_for_range(
        0..60,
        RectHeightStyle::IncludeLineSpacingTop,
        RectWidthStyle::Max,
    );
    reporter_assert!(reporter, boxes.len() == 4);
    if boxes.len() >= 4 {
        reporter_assert!(reporter, sk_scalar_nearly_equal!(boxes[0].rect.top, 38.4));
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 64.0)
        );

        reporter_assert!(reporter, sk_scalar_nearly_equal!(boxes[1].rect.top, 64.0));
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.bottom, 128.0)
        );

        reporter_assert!(reporter, sk_scalar_nearly_equal!(boxes[2].rect.top, 64.0));
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[2].rect.bottom, 128.0)
        );

        reporter_assert!(reporter, sk_scalar_nearly_equal!(boxes[3].rect.top, 128.0));
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[3].rect.bottom, 192.0)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7179-L7225 (chrome/m156)
def_paragraph_test!(SkParagraph_DifferentFontsTopLine, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_DifferentFontsTopLine.png");

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(10.0);
    let mut black = Paint::default();
    black.set_color(Color::BLACK);
    text_style.set_foreground_color(&black);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_style(&text_style);
    paragraph_style.set_text_direction(TextDirection::LTR);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    text_style.set_font_size(30.0);
    builder.push_style(&text_style);
    builder.add_text_utf16(&u16s("Atwater Peel ")); // FIXME-CONVERT
    text_style.set_font_size(15.0);
    builder.push_style(&text_style);
    builder.add_text_utf16(&u16s("Sherbrooke Bonaventure ")); // FIXME-CONVERT
    text_style.set_font_size(10.0);
    builder.push_style(&text_style);
    builder.add_text_utf16(&u16s("hi wassup!")); // FIXME-CONVERT

    let mut paragraph = builder.build();
    paragraph.layout(797.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));
    let boxes = paragraph.get_rects_for_range(
        0..60,
        RectHeightStyle::IncludeLineSpacingTop,
        RectWidthStyle::Max,
    );
    reporter_assert!(reporter, boxes.len() == 4);
    if boxes.len() >= 4 {
        reporter_assert!(reporter, sk_scalar_nearly_equal!(boxes[0].rect.top, 00.0));
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[0].rect.bottom, 30.0)
        );

        reporter_assert!(reporter, sk_scalar_nearly_equal!(boxes[1].rect.top, 00.0));
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[1].rect.bottom, 30.0)
        );

        reporter_assert!(reporter, sk_scalar_nearly_equal!(boxes[2].rect.top, 00.0));
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[2].rect.bottom, 30.0)
        );

        reporter_assert!(reporter, sk_scalar_nearly_equal!(boxes[3].rect.top, 30.0));
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(boxes[3].rect.bottom, 40.0)
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7227-L7268 (chrome/m156)
def_paragraph_test!(SkParagraph_SimpleParagraphReset, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let text = "Hello World Text Dialog";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    for _iteration in 0..2 {
        builder.reset();
        reporter_assert!(
            reporter,
            builder.peek_style().equals(paragraph_style.text_style())
        );

        let mut text_style = TextStyle::new();
        text_style.set_font_families(&["Roboto"]);
        text_style.set_color(Color::BLACK);
        builder.push_style(&text_style);
        builder.add_text(text);
        builder.pop();

        let mut paragraph = builder.build();
        paragraph.layout(TEST_CANVAS_WIDTH);
        reporter_assert!(reporter, paragraph.unresolved_glyphs() == Some(0));

        let impl_ = &paragraph;
        reporter_assert!(reporter, impl_.runs().len() == 1);
        reporter_assert!(reporter, impl_.styles().len() == 1); // paragraph style does not count
        reporter_assert!(reporter, impl_.styles()[0].style.equals(&text_style));

        let mut index: usize = 0;
        for line in impl_.lines() {
            line.scan_styles(
                impl_,
                StyleType::Decorations,
                &mut |_text_range, style, _context| {
                    reporter_assert!(reporter, index == 0);
                    reporter_assert!(reporter, style.color() == Color::BLACK);
                    index += 1;
                },
            );
        }
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7270-L7312 (chrome/m156)
def_paragraph_test!(SkParagraph_EllipsisGetRectForRange, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_EllipsisGetRectForRange.png");
    let text = concat!(
        "This is a very long sentence to test if the text will properly wrap ",
        "around and go to the next line. Sometimes, short sentence. Longer ",
        "sentences are okay too because they are nessecary. Very short. ",
    );

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(1);
    let ellipsis: Vec<u16> = u16s("\u{2026}");
    paragraph_style.set_ellipsis_utf16(&ellipsis);
    let _e: Vec<u16> = paragraph_style.ellipsis_utf16().to_vec();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let impl_ = &paragraph;

    // Check that the ellipsizer limited the text to one line and did not wrap to a second line.
    reporter_assert!(reporter, impl_.lines().len() == 1);

    let boxes1 = impl_.get_rects_for_range(0..2, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, boxes1.len() == 1);

    let boxes2 = impl_.get_rects_for_range(0..3, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, boxes2.len() == 1);

    canvas.draw_rects(Color::RED, &boxes1);
    canvas.draw_rects(Color::RED, &boxes2);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7315-L7352 (chrome/m156)
def_paragraph_test!(SkParagraph_StrutAndTextBehavior, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let text = "";

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_font_size(16.0);
    text_style.set_color(Color::BLACK);
    let mut strut_style = StrutStyle::new();
    strut_style.set_strut_enabled(true);
    strut_style.set_force_strut_height(true);
    strut_style.set_height(1.5);
    strut_style.set_height_override(true);
    strut_style.set_font_families(&["Ahem"]);
    strut_style.set_font_size(16.0);
    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_strut_style(strut_style.clone());
    paragraph_style.set_text_style(&text_style);

    let mut layout = |tb: TextHeightBehavior| {
        paragraph_style.set_text_height_behavior(tb);
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        builder.push_style(&text_style);
        builder.add_text(text);
        let mut paragraph = builder.build();
        paragraph.layout(SCALAR_INFINITY);
        return paragraph.height();
    };

    let height1 = layout(TextHeightBehavior::DisableAll);
    let height2 = layout(TextHeightBehavior::All);

    // Regardless of TextHeightBehavior strut sets the line height
    reporter_assert!(reporter, sk_scalar_nearly_equal!(height1, 24.0));
    reporter_assert!(reporter, sk_scalar_nearly_equal!(height2, 24.0));
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7354-L7404 (chrome/m156)
def_paragraph_test!(SkParagraph_NonMonotonicGlyphsLTR, |reporter| {
    let mut font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    need_system_fonts!(font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_NonMonotonicGlyphsLTR.png");
    let text: Vec<u16> = u16s(
        "\u{0068}\u{0301}\u{0350}\u{0312}\u{0357}\u{030C}\u{0369}\u{0305}\u{036C}\u{0304}\u{0310}\u{033F}\u{0366}\u{0350} ",
    );
    // The rest of the C++ literal is commented out there.

    let mut text_style = TextStyle::new();
    text_style.set_font_size(14.0);
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_style(&text_style);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    builder.push_style(&text_style);
    builder.add_text_utf16(&text);
    let mut paragraph = builder.build();
    paragraph.layout(SCALAR_INFINITY);

    paragraph.layout(paragraph.min_intrinsic_width() + 1.0);
    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.runs().len() > 1); // It's not the simple case
    let mut hasNonMonotonicPlacement: bool = false;
    for run in impl_.runs() {
        for offset in run.offsets() {
            if offset.x < 0.0 {
                hasNonMonotonicPlacement = true;
            }
        }
        if hasNonMonotonicPlacement {
            break;
        }
    }
    reporter_assert!(reporter, hasNonMonotonicPlacement); // There are non-monotonic placement
    reporter_assert!(reporter, impl_.line_number() == 1); // But it's still one line
    paragraph.paint(canvas.get(), (0.0, 0.0));
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7406-L7443 (chrome/m156)
def_paragraph_test!(SkParagraph_NonMonotonicGlyphsRTL, |reporter| {
    let mut font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    need_system_fonts!(font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_NonMonotonicGlyphsRTL.png");
    let text = "ٱلْرَّحْمَـانُ";

    let mut text_style = TextStyle::new();
    text_style.set_font_size(14.0);
    text_style.set_color(Color::BLACK);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_style(&text_style);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    builder.push_style(&text_style);
    builder.add_text(text);
    let mut paragraph = builder.build();
    paragraph.layout(SCALAR_INFINITY);

    paragraph.layout(paragraph.min_intrinsic_width() + 1.0);
    let impl_ = &paragraph;
    let mut hasNonMonotonicPlacement: bool = false;
    for run in impl_.runs() {
        for offset in run.offsets() {
            if offset.x < 0.0 {
                hasNonMonotonicPlacement = true;
            }
        }
        if hasNonMonotonicPlacement {
            break;
        }
    }
    reporter_assert!(reporter, impl_.line_number() == 1); // But it's still one line
    paragraph.paint(canvas.get(), (0.0, 0.0));
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7480-L7489 (chrome/m156)
def_paragraph_test!(SkParagraph_GetRectsForRangeConcurrently, |reporter| {
    let threads_count = 100;
    let reporter = Mutex::new(reporter);
    std::thread::scope(|scope| {
        let threads: Vec<_> = (0..threads_count)
            .map(|_| scope.spawn(|| perform_get_rects_for_range_concurrently(&reporter)))
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
    });
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7491-L7525 (chrome/m156)
def_paragraph_test!(SkParagraph_TabSubstitution, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_TabSubstitution.png");

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_replace_tab_characters(true);

    let mut text_style = TextStyle::new();
    text_style.set_color(Color::BLACK);
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(100.0);

    let mut builder1 = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder1.push_style(&text_style);
    builder1.add_text("There is a tab>\t<right here");
    let mut paragraph1 = builder1.build();
    paragraph1.layout(TEST_CANVAS_WIDTH);
    paragraph1.paint(canvas.get(), (0.0, 0.0));

    paragraph_style.set_replace_tab_characters(false);
    let mut builder2 = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder2.push_style(&text_style);
    builder2.add_text("There is a tab>\t<right here");
    let mut paragraph2 = builder2.build();
    paragraph2.layout(TEST_CANVAS_WIDTH);
    paragraph2.paint(canvas.get(), (0.0, 0.0));

    // Second paragraph has an unresolved \t (glyph == 0)
    reporter_assert!(reporter, paragraph1.runs()[0].glyphs()[15] != 0);
    reporter_assert!(reporter, paragraph2.runs()[0].glyphs()[15] == 0);
    // Notice, that the cache didn't work for the second paragraph - as it should not
    reporter_assert!(reporter, 2 == font_collection.paragraph_cache().count());
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7527-L7547 (chrome/m156)
def_paragraph_test!(SkParagraph_lineMetricsWithEllipsis, |reporter| {
    let mut font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    font_collection.set_default_font_manager(test_font_mgr());
    font_collection.enable_font_fallback();

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_max_lines(1);
    let ellipsis: Vec<u16> = u16s("\u{2026}");
    paragraph_style.set_ellipsis_utf16(&ellipsis);

    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder.add_text("hello");

    let mut paragraph = builder.build();
    paragraph.layout(1.);

    let lm = paragraph.get_line_metrics();
    reporter_assert!(reporter, lm.len() == 1);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7549-L7573 (chrome/m156)
def_paragraph_test!(SkParagraph_lineMetricsAfterUpdate, |reporter| {
    let mut font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    font_collection.set_default_font_manager(test_font_mgr());
    font_collection.enable_font_fallback();

    let text: Vec<u16> = u16s("hello world");

    let paragraph_style = ParagraphStyle::new();

    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder.add_text_utf16(&text);

    let mut paragraph = builder.build();
    paragraph.layout(200.);

    let lm = paragraph.get_line_metrics();
    reporter_assert!(reporter, lm.len() == 1, "size: {}", lm.len());

    paragraph.update_font_size(0, text.len(), 42.0);
    paragraph.layout(200.);
    let lm = paragraph.get_line_metrics();
    reporter_assert!(reporter, lm.len() == 2, "size: {}", lm.len());
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7576-L7690 (chrome/m156)
def_paragraph_test!(
    #[ignore = "needs GoogleSans-Regular.ttf, which test-fonts-m156 does not contain (see docs/design/modules.md)"]
    SkParagraph_MultiStyle_Logo,
    |reporter| {
        let font_collection = ResourceFontCollection::new(true);
        skip_if_fonts_not_found!(reporter, font_collection);

        let mut canvas = TestCanvas::new("SkParagraph_MultiStyle_Logo.png");

        canvas.get().draw_color(Color::WHITE, None);
        let width: scalar = TEST_CANVAS_WIDTH;
        let height: scalar = TEST_CANVAS_HEIGHT / 2.0;

        let cv = canvas.get();
        let _acr = AutoCanvasRestore::guard(cv, true);
        cv.clip_rect(Rect::from_wh(width, height), None, None);

        let mut style = TextStyle::new();
        style.set_font_families(&["Google Sans"]);
        style.set_font_size(30.0);

        let mut style0 = style.clone();
        style0.set_decoration_type(TextDecoration::UNDERLINE);
        style0.set_decoration_color(Color::BLACK);

        let mut style1 = style.clone();
        style1.set_decoration_type(TextDecoration::OVERLINE);
        style1.set_decoration_color(Color::BLACK);

        let mut paraStyle = ParagraphStyle::new();
        paraStyle.set_text_style(&style);
        paraStyle.set_max_lines(usize::MAX);

        let logo1 = "google_";
        let logo2 = "logo";
        let logo3 = "go";
        let logo4 = "ogle_logo";
        let logo5 = "google_lo";
        let logo6 = "go";

        let mut builder = ParagraphBuilderImpl::new(&paraStyle, &font_collection, get_unicode());
        style0.set_decoration_style(TextDecorationStyle::Double);
        style0.set_foreground_color(&Paint::new(colors::BLACK, None));
        style0.set_background_color(&Paint::new(colors::LIGHT_GREY, None));
        builder.push_style(&style0);
        builder.add_text(logo1);
        style1.set_decoration_style(TextDecorationStyle::Wavy);
        style1.set_foreground_color(&Paint::new(colors::BLUE, None));
        style1.set_background_color(&Paint::new(colors::YELLOW, None));
        builder.push_style(&style1);
        builder.add_text(logo2);
        builder.add_text(" ");

        style0.set_decoration_style(TextDecorationStyle::Solid);
        style0.set_foreground_color(&Paint::new(colors::BLUE, None));
        style0.set_background_color(&Paint::new(colors::WHITE, None));
        builder.push_style(&style0);
        builder.add_text(logo3);
        style1.set_decoration_style(TextDecorationStyle::Dotted);
        style1.set_foreground_color(&Paint::new(colors::BLACK, None));
        style1.set_background_color(&Paint::new(colors::MAGENTA, None));
        builder.push_style(&style1);
        builder.add_text(logo4);
        builder.add_text(" ");

        style0.set_decoration_style(TextDecorationStyle::Dashed);
        style0.set_foreground_color(&Paint::new(colors::GREEN, None));
        style0.set_background_color(&Paint::new(colors::GREY, None));
        builder.push_style(&style0);
        builder.add_text(logo5);
        style1.set_decoration_style(TextDecorationStyle::Double);
        style1.set_foreground_color(&Paint::new(colors::BLUE, None));
        style1.set_background_color(&Paint::new(colors::CYAN, None));
        builder.push_style(&style1);
        builder.add_text(logo6);

        let mut paragraph = builder.build();
        paragraph.layout(width - 40.0);
        paragraph.paint(cv, (20.0, 20.0));

        let impl_ = &paragraph;
        reporter_assert!(reporter, impl_.lines().len() == 1);

        let mut index: usize = 0;
        let mut left: scalar = 0.0;
        impl_.lines()[0].scan_styles(
            impl_,
            StyleType::Decorations,
            &mut |_text_range, _style, context| {
                match index {
                    0 => {
                        reporter_assert!(reporter, context.pos == 0 && context.size == 1);
                        reporter_assert!(
                            reporter,
                            sk_scalar_nearly_equal!(context.clip.left, left)
                        );
                        reporter_assert!(
                            reporter,
                            sk_scalar_nearly_equal!(context.clip.right, 62.6944885)
                        );
                    }
                    1 => {
                        reporter_assert!(reporter, context.pos == 0 && context.size == 2);
                        reporter_assert!(
                            reporter,
                            sk_scalar_nearly_equal!(context.clip.left, left)
                        );
                        reporter_assert!(
                            reporter,
                            sk_scalar_nearly_equal!(context.clip.right, 105.479904)
                        );
                    }
                    2 => {
                        reporter_assert!(reporter, context.pos == 2 && context.size == 1);
                        reporter_assert!(
                            reporter,
                            sk_scalar_nearly_equal!(context.clip.left, left)
                        );
                        reporter_assert!(
                            reporter,
                            sk_scalar_nearly_equal!(context.clip.right, 123.3926165)
                        );
                    }
                    3 => {
                        reporter_assert!(reporter, context.pos == 2 && context.size == 2);
                        reporter_assert!(
                            reporter,
                            sk_scalar_nearly_equal!(context.clip.left, left)
                        );
                        reporter_assert!(
                            reporter,
                            sk_scalar_nearly_equal!(context.clip.right, 210.959808)
                        );
                    }
                    4 => {
                        reporter_assert!(reporter, context.pos == 4 && context.size == 1);
                        reporter_assert!(
                            reporter,
                            sk_scalar_nearly_equal!(context.clip.left, left)
                        );
                        reporter_assert!(
                            reporter,
                            sk_scalar_nearly_equal!(context.clip.right, 291.567017)
                        );
                    }
                    5 => {
                        reporter_assert!(reporter, context.pos == 4 && context.size == 1); // No space at the end
                        reporter_assert!(
                            reporter,
                            sk_scalar_nearly_equal!(context.clip.left, left)
                        );
                        reporter_assert!(
                            reporter,
                            sk_scalar_nearly_equal!(context.clip.right, 309.479736)
                        );
                    }
                    _ => {
                        reporter_assert!(reporter, false);
                    }
                }
                left = context.clip.right;
                index += 1;
            },
        );
    }
);

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7693-L7752 (chrome/m156)
def_paragraph_test!(SkParagraph_MultiStyle_FFI, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_MultiStyle_FFI.png");

    canvas.get().draw_color(Color::WHITE, None);

    let paragraph_style = ParagraphStyle::new();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    let mut text_style = TextStyle::new();
    text_style.set_color(Color::BLACK);
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(60.0);
    text_style.set_background_color(&Paint::new(colors::GREY, None));
    builder.push_style(&text_style);
    builder.add_text("f");
    text_style.set_background_color(&Paint::new(colors::YELLOW, None));
    builder.push_style(&text_style);
    builder.add_text("f");
    text_style.set_background_color(&Paint::new(colors::LIGHT_GREY, None));
    builder.push_style(&text_style);
    builder.add_text("i");
    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, 0.0));
    let width = paragraph.longest_line();
    let height = paragraph.height();

    let f1Pos = paragraph.get_glyph_position_at_coordinate((width / 6.0 - 5.0, height / 2.0));
    let f2Pos = paragraph.get_glyph_position_at_coordinate((width / 2.0 - 5.0, height / 2.0));
    let iPos = paragraph.get_glyph_position_at_coordinate((width * 5.0 / 6.0 - 5.0, height / 2.0));

    // Positions are aligned with graphemes (no pointing inside ffi grapheme)
    reporter_assert!(
        reporter,
        f1Pos.position == 0 && f1Pos.affinity == Affinity::Downstream
    );
    reporter_assert!(
        reporter,
        f2Pos.position == 1 && f2Pos.affinity == Affinity::Downstream
    );
    reporter_assert!(
        reporter,
        iPos.position == 2 && iPos.affinity == Affinity::Downstream
    );

    // Bounding boxes show the extact position (inside ffi grapheme)
    let f1 = paragraph.get_rects_for_range(0..1, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, f1.len() == 1);
    reporter_assert!(reporter, f1[0].direct == TextDirection::LTR);
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(f1[0].rect.left, 0.000000, EPSILON100)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(f1[0].rect.right, 17.070000, EPSILON100)
    );

    let f2 = paragraph.get_rects_for_range(1..2, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, f2.len() == 1);
    reporter_assert!(reporter, f2[0].direct == TextDirection::LTR);
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(f2[0].rect.left, 17.070000, EPSILON100)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(f2[0].rect.right, 34.139999, EPSILON100)
    );

    let fi = paragraph.get_rects_for_range(2..3, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, fi.len() == 1);
    reporter_assert!(reporter, fi[0].direct == TextDirection::LTR);
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(fi[0].rect.left, 34.139999, EPSILON100)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(fi[0].rect.right, 51.209999, EPSILON100)
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7755-L7806 (chrome/m156)
def_paragraph_test!(SkParagraph_MultiStyle_EmojiFamily, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_MultiStyle_EmojiFamily.png");

    canvas.get().draw_color(Color::WHITE, None);

    let paragraph_style = ParagraphStyle::new();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    let mut text_style = TextStyle::new();
    text_style.set_color(Color::BLACK);
    text_style.set_font_families(&["Noto Color Emoji"]);
    text_style.set_font_size(40.0);
    builder.push_style(&text_style);
    builder.add_text_utf16(&u16s(
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}",
    )); // FIXME-CONVERT
    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    let mut paint = Paint::default();
    paint.set_style(PaintStyle::Stroke);
    paint.set_anti_alias(true);
    paint.set_stroke_width(1.0);
    paint.set_color(Color::LIGHT_GRAY);
    canvas.get().draw_rect(
        Rect::from_xywh(0.0, 0.0, paragraph.longest_line(), paragraph.height()),
        &paint,
    );
    paragraph.paint(canvas.get(), (0.0, 0.0));
    let width = paragraph.longest_line();
    let height = paragraph.height();

    let pos00 = paragraph.get_glyph_position_at_coordinate((width / 4.0, height / 4.0));
    let pos10 = paragraph.get_glyph_position_at_coordinate((width * 3.0 / 4.0, height / 2.0));
    let pos01 = paragraph.get_glyph_position_at_coordinate((width / 4.0, height / 2.0));
    let pos11 = paragraph.get_glyph_position_at_coordinate((width * 3.0 / 4.0, height * 3.0 / 4.0));

    // Positioning is aligned with the closest grapheme edge (left or right)
    reporter_assert!(
        reporter,
        pos00.position == 0 && pos00.affinity == Affinity::Downstream
    );
    reporter_assert!(
        reporter,
        pos01.position == 0 && pos01.affinity == Affinity::Downstream
    );
    reporter_assert!(
        reporter,
        pos10.position == 11 && pos10.affinity == Affinity::Upstream
    );
    reporter_assert!(
        reporter,
        pos11.position == 11 && pos11.affinity == Affinity::Upstream
    );

    // Bounding boxes around a part of a grapheme are empty
    let f1 = paragraph.get_rects_for_range(0..2, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, f1.len() == 0);

    let f2 = paragraph.get_rects_for_range(4..6, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, f2.len() == 0);

    let f3 = paragraph.get_rects_for_range(8..10, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, f3.len() == 0);

    let f4 = paragraph.get_rects_for_range(8..10, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, f4.len() == 0);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7809-L7858 (chrome/m156)
def_paragraph_test!(SkParagraph_MultiStyle_Arabic, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_MultiStyle_Arabic.png");

    canvas.get().draw_color(Color::WHITE, None);

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Noto Naskh Arabic"]);
    text_style.set_font_size(50.0);
    text_style.set_color(Color::BLACK);
    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_style(&text_style);
    paragraph_style.set_text_direction(TextDirection::RTL);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    text_style.set_color(Color::BLUE);
    builder.push_style(&text_style);
    builder.add_text("ك");
    text_style.set_color(Color::RED);
    builder.push_style(&text_style);
    builder.add_text("ِّ");
    text_style.set_color(Color::BLUE);
    builder.push_style(&text_style);
    builder.add_text("ـ");
    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let width = paragraph.longest_line();
    let height = paragraph.height();

    // Positioning is align with the grapheme
    let f1Pos = paragraph
        .get_glyph_position_at_coordinate((TEST_CANVAS_WIDTH - width * 5.0 / 6.0, height / 2.0));
    let f2Pos =
        paragraph.get_glyph_position_at_coordinate((TEST_CANVAS_WIDTH - width / 3.0, height / 2.0));
    let iPos =
        paragraph.get_glyph_position_at_coordinate((TEST_CANVAS_WIDTH - width / 6.0, height / 2.0));
    reporter_assert!(
        reporter,
        f1Pos.position == 4 && f1Pos.affinity == Affinity::Upstream
    );
    reporter_assert!(
        reporter,
        f2Pos.position == 1 && f2Pos.affinity == Affinity::Upstream
    );
    reporter_assert!(
        reporter,
        iPos.position == 0 && iPos.affinity == Affinity::Downstream
    );

    // Bounding boxes around a part of a grapheme are empty
    let f1 = paragraph.get_rects_for_range(0..1, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, f1.len() == 0);

    let f2 = paragraph.get_rects_for_range(1..2, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, f2.len() == 0);

    let fi = paragraph.get_rects_for_range(2..3, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, fi.len() == 0);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7861-L7921 (chrome/m156)
def_paragraph_test!(SkParagraph_MultiStyle_Zalgo, |reporter| {
    let mut font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    need_system_fonts!(font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_MultiStyle_Zalgo.png");

    canvas.get().draw_color(Color::WHITE, None);

    //SkString text(">S͛ͭ̋͆̈̔̇͗̍͑̎ͪͮͧͣ̽ͫͣ́ͬ̀͌͑͂͗͒̍̔̄ͧ̏̉̌̊̊̿̀̌̃̄͐̓̓̚̚҉̵̡͜͟͝͠͏̸̵̡̧͜҉̷̡͇̜̘̻̺̘̟̝͙̬̘̩͇̭̼̥̖̤̦͎k͉̩̘͚̜̹̗̗͍̤̥̱͉̳͕͖̤̲̣͚̮̞̬̲͍͔̯̻̮̞̭͈̗̫͓̂ͨ̉ͪ̒͋͛̀̍͊ͧ̿̅͆̓̔̔ͬ̇̑̿ͩ͗ͮ̎͌̿̄ͅP̴̵̡̡̛̪͙̼̣̟̩̭̫̱͙̬͔͉͍̘̠͉̦̝̘̥̟̗͖̫̤͕̙̬̦͍̱̖̮̱͑͐̎̃̒͐͋̚͘͞a̶̶̵̵̵̶̶̡̧̢̢̺͔̣͖̭̺͍̤͚̱̜̰̥͕̬̥̲̞̥̘͇͚̺̰͚̪̺͔̤͍̓̿͆̎͋̓ͦ̈́ͦ̌́̄͗̌̓͌̕͜͜͟͢͝͡ŕ͎̝͕͉̻͎̤̭͚̗̳̖̙̘͚̫͖͓͚͉͔͈̟̰̟̬̗͓̟͚̱̕͡ͅͅͅa̸̶̢̛̛̽ͮͩ̅͒ͫ͗͂̎ͦ̈́̓̚͘͜͢͡҉̷̵̶̢̡̜̮̦̜̥̜̯̙͓͔̼̗̻͜͜ͅḡ̢̛͕̗͖̖̤̦̘͔ͨͨ̊͒ͩͭͤ̍̅̃ͪ̋̏̓̍̋͗̋ͨ̏̽̈́̔̀̋̉ͫ̅̂ͭͫ̏͒͋ͥ̚͜r̶̢̧̧̥̤̼̀̂̒ͪ͌̿͌̅͛ͨͪ͒̍ͥ̉ͤ̌̿̆́ͭ͆̃̒ͤ͛̊ͧ̽͘͝͠a̧̢̧̢͑͑̓͑ͮ̃͂̄͛́̈́͋̂͌̽̄͒̔́̇ͨͧͭ͐ͦ̋ͨ̍ͦ̍̋͆̔ͧ͑͋͌̈̓͛͛̚͢͜͜͏̴̢̧̛̳͍̹͚̰̹̻͔p̨̡͆ͦͣ͊̽̔͂̉ͣ̔ͣ̌̌̉̃̋̂͒ͫ̄̎̐͗̉̌̃̽̽́̀̚͘͜͟҉̱͉h̭̮̘̗͔̜̯͔͈̯̺͔̗̣̭͚̱̰̙̼̹͚̣̻̥̲̮͍̤͜͝<");
    let text16: Vec<u16> = u16s(
        ">S\u{035B}\u{036D}\u{030B}\u{0346}\u{0308}\u{0314}\u{0307}\u{0357}\u{030D}\u{0351}\u{030E}\u{036A}\u{036E}\u{0367}\u{0363}\u{033D}\u{036B}\u{0363}\u{0301}\u{036C}\u{0300}\u{034C}\u{0351}\u{0342}\u{0357}\u{0352}\u{030D}\u{0314}\u{0304}\u{0367}\u{030F}\u{031A}\u{0309}\u{030C}\u{030A}\u{030A}\u{033F}\u{0300}\u{030C}\u{0303}\u{0304}\u{0350}\u{0313}\u{031A}\u{0313}\u{0363}\u{0321}\u{035C}\u{035D}\u{035F}\u{0360}\u{0335}\u{034F}\u{0321}\u{0327}\u{0338}\u{035C}\u{0335}\u{0363}\u{0337}\u{0321}\u{0347}\u{031C}\u{0318}\u{033B}\u{033A}\u{0318}\u{031F}\u{031D}\u{0359}\u{032C}\u{0318}\u{0329}\u{0347}\u{032D}\u{033C}\u{0325}\u{0316}\u{0324}\u{0326}\u{034E}k\u{0302}\u{0368}\u{0309}\u{036A}\u{0312}\u{034B}\u{035B}\u{0300}\u{030D}\u{034A}\u{0367}\u{033F}\u{0305}\u{0346}\u{0313}\u{0314}\u{0314}\u{036C}\u{0307}\u{0311}\u{033F}\u{0369}\u{0357}\u{036E}\u{030E}\u{034C}\u{033F}\u{0304}\u{0349}\u{0329}\u{0318}\u{035A}\u{031C}\u{0339}\u{0317}\u{0317}\u{034D}\u{0324}\u{0325}\u{0331}\u{0349}\u{0333}\u{0355}\u{0345}\u{0356}\u{0324}\u{0332}\u{0323}\u{035A}\u{032E}\u{031E}\u{032C}\u{0332}\u{034D}\u{0354}\u{032F}\u{033B}\u{032E}\u{031E}\u{032D}\u{0348}\u{0317}\u{032B}\u{0353}P\u{031A}\u{0351}\u{0350}\u{030E}\u{0303}\u{0312}\u{0350}\u{034B}\u{0334}\u{031B}\u{035E}\u{0358}\u{0321}\u{0335}\u{0321}\u{032A}\u{0359}\u{033C}\u{0323}\u{031F}\u{0329}\u{032D}\u{032B}\u{0331}\u{0359}\u{032C}\u{0354}\u{0349}\u{034D}\u{0318}\u{0320}\u{0349}\u{0326}\u{031D}\u{0318}\u{0325}\u{031F}\u{0317}\u{0356}\u{032B}\u{0324}\u{0355}\u{0319}\u{032C}\u{0326}\u{034D}\u{0331}\u{0316}\u{032E}\u{0331}a\u{0313}\u{033F}\u{0346}\u{030E}\u{034B}\u{0313}\u{0366}\u{0344}\u{0366}\u{030C}\u{0301}\u{0304}\u{0357}\u{030C}\u{0313}\u{034C}\u{035C}\u{0336}\u{035C}\u{0321}\u{0336}\u{035D}\u{0315}\u{0335}\u{0335}\u{0335}\u{035F}\u{0336}\u{0336}\u{0327}\u{0322}\u{0361}\u{0362}\u{0322}\u{033A}\u{0354}\u{0323}\u{0356}\u{032D}\u{033A}\u{034D}\u{0324}\u{035A}\u{0331}\u{031C}\u{0330}\u{0325}\u{0355}\u{032C}\u{0325}\u{0332}\u{031E}\u{0325}\u{0318}\u{0347}\u{035A}\u{033A}\u{0330}\u{035A}\u{032A}\u{033A}\u{0354}\u{0324}\u{034D}r\u{0301}\u{0361}\u{0315}\u{034E}\u{031D}\u{0355}\u{0349}\u{033B}\u{034E}\u{0324}\u{0345}\u{0345}\u{032D}\u{035A}\u{0317}\u{0333}\u{0316}\u{0319}\u{0318}\u{035A}\u{0345}\u{032B}\u{0356}\u{0353}\u{035A}\u{0349}\u{0354}\u{0348}\u{031F}\u{0330}\u{031F}\u{032C}\u{0317}\u{0353}\u{031F}\u{035A}\u{0331}a\u{033D}\u{036E}\u{0369}\u{0305}\u{0352}\u{031A}\u{036B}\u{0357}\u{0342}\u{030E}\u{0366}\u{0344}\u{0343}\u{0338}\u{035C}\u{0361}\u{0322}\u{031B}\u{0358}\u{031B}\u{0362}\u{0336}\u{0363}\u{0337}\u{035C}\u{0322}\u{035C}\u{0321}\u{0335}\u{0336}\u{0345}\u{031C}\u{032E}\u{0326}\u{031C}\u{0325}\u{031C}\u{032F}\u{0319}\u{0353}\u{0354}\u{033C}\u{0317}\u{033B}g\u{0304}\u{0368}\u{0368}\u{030A}\u{0352}\u{0369}\u{036D}\u{0364}\u{030D}\u{0305}\u{0303}\u{036A}\u{030B}\u{030F}\u{0313}\u{030D}\u{031A}\u{030B}\u{0357}\u{030B}\u{0368}\u{030F}\u{033D}\u{0344}\u{0314}\u{0300}\u{030B}\u{0309}\u{036B}\u{0305}\u{0302}\u{036D}\u{036B}\u{030F}\u{0352}\u{034B}\u{0365}\u{0322}\u{031B}\u{035C}\u{0355}\u{0317}\u{0356}\u{0316}\u{0324}\u{0326}\u{0318}\u{0354}r\u{0300}\u{0302}\u{0312}\u{036A}\u{034C}\u{033F}\u{034C}\u{0305}\u{035B}\u{0368}\u{036A}\u{0352}\u{030D}\u{0365}\u{0309}\u{0364}\u{030C}\u{033F}\u{0306}\u{0301}\u{036D}\u{0346}\u{0303}\u{0312}\u{0364}\u{035B}\u{030A}\u{0367}\u{033D}\u{035D}\u{0360}\u{0322}\u{0358}\u{0327}\u{0327}\u{0336}\u{0325}\u{0324}\u{033C}a\u{0351}\u{0351}\u{0313}\u{0351}\u{036E}\u{0303}\u{0342}\u{0304}\u{035B}\u{0301}\u{0344}\u{034B}\u{0302}\u{034C}\u{033D}\u{0304}\u{0352}\u{0314}\u{0301}\u{0307}\u{0368}\u{0367}\u{036D}\u{0350}\u{0366}\u{031A}\u{030B}\u{0368}\u{030D}\u{0366}\u{030D}\u{030B}\u{0346}\u{0314}\u{0367}\u{0351}\u{034B}\u{034C}\u{0308}\u{0343}\u{035B}\u{035B}\u{0327}\u{0322}\u{0327}\u{0362}\u{035C}\u{035C}\u{0322}\u{034F}\u{0322}\u{031B}\u{0334}\u{0327}\u{0333}\u{034D}\u{0339}\u{035A}\u{0330}\u{0339}\u{033B}\u{0354}p\u{0346}\u{0366}\u{031A}\u{0363}\u{034A}\u{033D}\u{0314}\u{0342}\u{0309}\u{0363}\u{0314}\u{0363}\u{030C}\u{030C}\u{0309}\u{0303}\u{030B}\u{0302}\u{0352}\u{036B}\u{0304}\u{030E}\u{0310}\u{0357}\u{0309}\u{030C}\u{0303}\u{033D}\u{033D}\u{0328}\u{0341}\u{0358}\u{0340}\u{0321}\u{035C}\u{035F}\u{0363}\u{0331}\u{0349}h\u{035C}\u{035D}\u{032D}\u{032E}\u{0318}\u{0317}\u{0354}\u{031C}\u{032F}\u{0354}\u{0348}\u{032F}\u{033A}\u{0354}\u{0317}\u{0323}\u{032D}\u{035A}\u{0331}\u{0330}\u{0319}\u{033C}\u{0339}\u{035A}\u{0323}\u{033B}\u{0325}\u{0332}\u{032E}\u{034D}\u{0324}<",
    );
    let text = convert_utf16_to_utf8(&text16).into_bytes();
    let K = text.iter().position(|&c| c == b'k').unwrap();
    let P = text.iter().position(|&c| c == b'P').unwrap();
    let h = text.iter().position(|&c| c == b'h').unwrap();
    let paragraph_style = ParagraphStyle::new();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::RED);
    builder.push_style(&text_style);
    builder.add_text_bytes(&text[..K + 3]);
    text_style.set_color(Color::BLUE);
    text_style.set_background_color(&Paint::new(colors::YELLOW, None));
    builder.push_style(&text_style);
    builder.add_text_bytes(&text[K + 3..K + 3 + (P - K - 3 + 6)]);
    text_style.set_color(Color::GREEN);
    builder.push_style(&text_style);
    builder.add_text_bytes(&text[P + 6..P + 6 + (h - P - 6)]);
    text_style.set_color(Color::BLACK);
    text_style.set_background_color(&Paint::new(colors::LIGHT_GREY, None));
    builder.push_style(&text_style);
    builder.add_text_bytes(&text[h..h + (text.len() - h)]);
    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, paragraph.height() / 2.0));
    //auto width = paragraph->getLongestLine();
    let height = paragraph.height();

    let resSK = paragraph.get_rects_for_range(0..K, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, resSK.len() != 0);
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(resSK[0].rect.width(), 10.45, EPSILON100)
    );

    let resKP = paragraph.get_rects_for_range(K..P, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, resKP.len() != 0);
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(resKP[0].rect.width(), 11.22)
    );
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(resKP[0].rect.width(), 11.22, EPSILON100)
    );

    let resPh = paragraph.get_rects_for_range(P..h, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, resPh.len() != 0);
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(resPh[0].rect.width(), 67.26, EPSILON20)
    );

    let posK = paragraph
        .get_glyph_position_at_coordinate((resSK.last().unwrap().rect.right, height / 2.0));
    let posP = paragraph
        .get_glyph_position_at_coordinate((resKP.last().unwrap().rect.right, height / 2.0));
    let posH = paragraph
        .get_glyph_position_at_coordinate((resPh.last().unwrap().rect.right, height / 2.0));
    reporter_assert!(
        reporter,
        posK.position == 148 && posP.position == 264 && posH.position == 572
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7924-L7959 (chrome/m156)
def_paragraph_test!(SkParagraph_RtlEllipsis1, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_RtlEllipsis1.png");

    canvas.get().draw_color(Color::WHITE, None);

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Noto Naskh Arabic", "Roboto"]);
    text_style.set_font_size(100.0);
    text_style.set_color(Color::BLACK);
    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_style(&text_style);
    paragraph_style.set_text_direction(TextDirection::RTL);
    paragraph_style.set_ellipsis_utf16(&u16s("\u{2026}")); // FIXME-CONVERT
    paragraph_style.set_text_align(TextAlign::Start);
    paragraph_style.set_max_lines(1);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder.push_style(&text_style);
    builder.add_text_utf16(&u16s("1  2  3  4  5  6  7  8  9")); // FIXME-CONVERT
    let mut paragraph = builder.build();
    paragraph.layout(474.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));
    let impl_ = &paragraph;
    reporter_assert!(reporter, paragraph.line_number() == 1);
    let line = &impl_.lines()[0];
    let mut first: bool = true;
    line.iterate_through_visual_runs(
        impl_,
        true,
        &mut |run, _runOffsetInLine, _text_range, _runWidthInLine| {
            reporter_assert!(reporter, first == (run.is_ellipsis()));
            first = false;
            true
        },
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L7961-L7996 (chrome/m156)
def_paragraph_test!(SkParagraph_RtlEllipsis2, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);

    let mut canvas = TestCanvas::new("SkParagraph_RtlEllipsis2.png");

    canvas.get().draw_color(Color::WHITE, None);

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Noto Naskh Arabic", "Roboto"]);
    text_style.set_font_size(100.0);
    text_style.set_color(Color::BLACK);
    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_style(&text_style);
    paragraph_style.set_text_direction(TextDirection::RTL);
    paragraph_style.set_ellipsis_utf16(&u16s("\u{2026}")); // FIXME-CONVERT
    paragraph_style.set_text_align(TextAlign::Start);
    paragraph_style.set_max_lines(2);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder.push_style(&text_style);
    builder.add_text_utf16(&u16s("تظاهرات و تجمعات اعتراضی در سراسر کشور ۲۳ مهر")); // FIXME-CONVERT
    let mut paragraph = builder.build();
    paragraph.layout(474.0);
    paragraph.paint(canvas.get(), (0.0, 0.0));
    let impl_ = &paragraph;
    reporter_assert!(reporter, paragraph.line_number() == 2);
    let line = &impl_.lines()[1];
    let mut first: bool = true;
    line.iterate_through_visual_runs(
        impl_,
        true,
        &mut |run, _runOffsetInLine, _text_range, _runWidthInLine| {
            reporter_assert!(reporter, first == (run.is_ellipsis()));
            first = false;
            true
        },
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8006-L8097 (chrome/m156)
def_paragraph_test!(SkParagraph_TextEditingFunctionality, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_TextEditingFunctionality.png");
    let text = concat!(
        "This is a very long sentence to test if the text will properly wrap ",
        "around and go to the next line. Sometimes, short sentence. Longer ",
        "sentences are okay too because they are nessecary. Very short. ",
        "This is a very long sentence to test if the text will properly wrap ",
        "around and go to the next line. Sometimes, short sentence. Longer ",
        "sentences are okay too because they are nessecary. Very short. ",
    );

    let len = text.len();

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_ellipsis_utf16(&u16s("\u{2026}")); // FIXME-CONVERT
    paragraph_style.set_max_lines(3);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    paragraph.paint(canvas.get(), (0.0, 0.0));

    let mut lineNumber = paragraph.get_line_number_at(0);
    reporter_assert!(reporter, lineNumber == Some(0));
    lineNumber = paragraph.get_line_number_at(len / 2);
    reporter_assert!(reporter, lineNumber == Some(1));
    lineNumber = paragraph.get_line_number_at(len - 1);
    reporter_assert!(reporter, lineNumber == None);
    lineNumber = paragraph.get_line_number_at(len);
    reporter_assert!(reporter, lineNumber == None);
    lineNumber = paragraph.get_line_number_at(len + 10);
    reporter_assert!(reporter, lineNumber == None);

    let mut lineMetrics = paragraph.get_line_metrics_at(0);
    reporter_assert!(
        reporter,
        lineMetrics.as_ref().is_some_and(|m| m.line_number == 0)
    );
    lineMetrics = paragraph.get_line_metrics_at(1);
    reporter_assert!(
        reporter,
        lineMetrics.as_ref().is_some_and(|m| m.line_number == 1)
    );
    lineMetrics = paragraph.get_line_metrics_at(3);
    reporter_assert!(reporter, lineMetrics.is_none());
    lineMetrics = paragraph.get_line_metrics_at(10);
    reporter_assert!(reporter, lineMetrics.is_none());

    let metrics = paragraph.get_line_metrics();
    let mut actualText = paragraph.get_actual_text_range(0, false);
    reporter_assert!(
        reporter,
        actualText.end == metrics[0].end_excluding_whitespaces
    );
    actualText = paragraph.get_actual_text_range(1, false);
    reporter_assert!(
        reporter,
        actualText.end == metrics[1].end_excluding_whitespaces
    );
    actualText = paragraph.get_actual_text_range(2, false);
    reporter_assert!(
        reporter,
        actualText.end == metrics[2].end_excluding_whitespaces
    );

    let mut glyphInfo = paragraph.get_glyph_cluster_at(0);
    reporter_assert!(
        reporter,
        glyphInfo.as_ref().is_some_and(|g| g.text_range.start == 0)
    );
    glyphInfo = paragraph.get_glyph_cluster_at(len / 2);
    reporter_assert!(
        reporter,
        glyphInfo
            .as_ref()
            .is_some_and(|g| g.text_range.start == len / 2)
    );
    glyphInfo = paragraph.get_glyph_cluster_at(len);
    reporter_assert!(reporter, glyphInfo.is_none());

    glyphInfo = paragraph.get_closest_glyph_cluster_at((0.0, 10.0));
    reporter_assert!(
        reporter,
        glyphInfo
            .as_ref()
            .is_some_and(|g| g.text_range.start == 0 && g.text_range.end == 1)
    );
    glyphInfo = paragraph.get_closest_glyph_cluster_at((TEST_CANVAS_WIDTH / 2.0, 20.0));
    reporter_assert!(
        reporter,
        glyphInfo
            .as_ref()
            .is_some_and(|g| g.text_range.start == 61 && g.text_range.end == 62)
    );
    glyphInfo = paragraph.get_closest_glyph_cluster_at((TEST_CANVAS_WIDTH + 10.0, 30.0));

    reporter_assert!(
        reporter,
        glyphInfo
            .as_ref()
            .is_some_and(|g| g.text_range.start == 228 && g.text_range.end == 229)
    );

    let font = paragraph.get_font_at(10);
    reporter_assert!(reporter, !has_empty_typeface(&font));
    let mut fontFamily = font.typeface().family_name();
    reporter_assert!(reporter, fontFamily == "Roboto");

    let fonts = paragraph.get_fonts();
    reporter_assert!(reporter, fonts.len() == 1);
    reporter_assert!(
        reporter,
        fonts[0].text_range.start == 0 && fonts[0].text_range.end == len
    );
    reporter_assert!(reporter, !has_empty_typeface(&fonts[0].font));
    fontFamily = font.typeface().family_name();
    reporter_assert!(reporter, fontFamily == "Roboto");
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8099-L8137 (chrome/m156)
def_paragraph_test!(SkParagraph_getLineNumberAt_Ellipsis, |reporter| {
    let mut font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    font_collection.set_default_font_manager(test_font_mgr());
    let _canvas = TestCanvas::new("SkParagraph_Ellipsis.png");

    // The second line will be ellipsized. The 10th glyph ("0") will be replaced
    // by U+2026.
    let text = concat!("This\n", "1234567890ABCD",);

    let len = text.len();

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_ellipsis_utf16(&u16s("\u{2026}")); // FIXME-CONVERT
    paragraph_style.set_max_lines(2);

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Ahem"]);
    text_style.set_color(Color::BLACK);
    text_style.set_font_size(10.0);

    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();
    let mut paragraph = builder.build();

    // Roughly 10 characters wide.
    paragraph.layout(100.0);

    reporter_assert!(reporter, paragraph.get_line_number_at(0) == Some(0));
    reporter_assert!(reporter, paragraph.get_line_number_at(4) == Some(0));
    reporter_assert!(reporter, paragraph.get_line_number_at(5) == Some(1));
    reporter_assert!(reporter, paragraph.get_line_number_at(len) == None);
    reporter_assert!(reporter, paragraph.get_line_number_at(len - 1) == None);
    // "0" should be ellipsized away so the call return -1 instead of 1.
    reporter_assert!(reporter, paragraph.get_line_number_at(14) == None);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8139-L8183 (chrome/m156)
def_paragraph_test!(SkParagraph_API_USES_UTF16, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    // Each of these characters consists of 3 UTF-8 code points, or 1 UTF-16
    // code point.
    let text = "一丁丂七";

    let paragraph_style = ParagraphStyle::new();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);

    reporter_assert!(
        reporter,
        !has_empty_typeface(&paragraph.get_font_at_utf16_offset(0))
    );
    reporter_assert!(
        reporter,
        has_empty_typeface(&paragraph.get_font_at_utf16_offset(4))
    );

    reporter_assert!(
        reporter,
        paragraph.get_glyph_info_at_utf16_offset(0).is_some()
    );
    reporter_assert!(
        reporter,
        paragraph.get_glyph_info_at_utf16_offset(4).is_none()
    );
    // Verify the output also uses UTF-16
    let glyphInfo = paragraph.get_glyph_info_at_utf16_offset(3);
    reporter_assert!(reporter, glyphInfo.is_some());
    let glyphInfo = glyphInfo.unwrap_or_default();
    reporter_assert!(reporter, glyphInfo.grapheme_cluster_text_range.start == 3);
    reporter_assert!(reporter, glyphInfo.grapheme_cluster_text_range.end == 4);

    reporter_assert!(
        reporter,
        paragraph.get_line_number_at_utf16_offset(0) == Some(0)
    );
    reporter_assert!(
        reporter,
        paragraph.get_line_number_at_utf16_offset(4) == None
    );

    // The last logical character ("七") is considered hit.
    let glyphInfo = paragraph.get_closest_utf16_glyph_info_at((999.0, 999.0));
    reporter_assert!(reporter, glyphInfo.is_some());
    let glyphInfo = glyphInfo.unwrap_or_default();
    reporter_assert!(reporter, glyphInfo.grapheme_cluster_text_range.start == 3);
    reporter_assert!(reporter, glyphInfo.grapheme_cluster_text_range.end == 4);

    // The first logical character ("一") is considered hit.
    let glyphInfo = paragraph.get_closest_utf16_glyph_info_at((-999.0, 0.0));
    reporter_assert!(reporter, glyphInfo.is_some());
    let glyphInfo = glyphInfo.unwrap_or_default();
    reporter_assert!(reporter, glyphInfo.grapheme_cluster_text_range.start == 0);
    reporter_assert!(reporter, glyphInfo.grapheme_cluster_text_range.end == 1);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8185-L8207 (chrome/m156)
def_paragraph_test!(SkParagraph_Empty_Paragraph_Metrics, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let text = "";

    let paragraph_style = ParagraphStyle::new();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);

    reporter_assert!(reporter, has_empty_typeface(&paragraph.get_font_at(0)));
    reporter_assert!(reporter, paragraph.get_glyph_cluster_at(0).is_none());
    reporter_assert!(reporter, paragraph.get_line_number_at(0) == None);
    reporter_assert!(
        reporter,
        paragraph
            .get_closest_glyph_cluster_at((10.0, 5.0))
            .is_none()
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8209-L8232 (chrome/m156)
def_paragraph_test!(SkParagraph_GlyphCluster_Ligature, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let text = "fi";
    let len = text.len();

    let paragraph_style = ParagraphStyle::new();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);

    let glyphInfo = paragraph.get_glyph_cluster_at(0);
    reporter_assert!(reporter, glyphInfo.is_some());
    let glyphInfo = glyphInfo.unwrap_or_default();
    reporter_assert!(reporter, glyphInfo.text_range.start == 0);
    reporter_assert!(reporter, glyphInfo.text_range.end == len);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8234-L8295 (chrome/m156)
def_paragraph_test!(SkParagraph_GlyphInfo_LigatureDiacritics, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    // Ligature + diacritics
    // text = lam + hamza + alef + hamza
    // lam and alef form a laam-alif ligature and the 2 hamza are diacritical
    // marks the combines into the ligated base glyphs.
    let text = "لٔأ";
    let len = text.len();

    let paragraph_style = ParagraphStyle::new();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Katibeh"]);
    text_style.set_font_size(100.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);
    let mut canvas = TestCanvas::new("SkParagraph_laam_alif_diacritics.png");
    paragraph.paint(canvas.get(), (50.0, 50.0));

    let mut glyphInfo;

    let boxes =
        paragraph.get_rects_for_range(0..len, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, boxes.len() == 1);
    let fullRect: Rect = boxes[0].rect;

    let found = paragraph.get_glyph_info_at_utf16_offset(0);
    reporter_assert!(reporter, found.is_some());
    glyphInfo = found.unwrap_or_default();
    reporter_assert!(reporter, glyphInfo.grapheme_cluster_text_range.start == 0);
    reporter_assert!(reporter, glyphInfo.grapheme_cluster_text_range.end == 2);
    let rect0: Rect = glyphInfo.grapheme_layout_bounds;

    let found = paragraph.get_glyph_info_at_utf16_offset(1);
    reporter_assert!(reporter, found.is_some());
    glyphInfo = found.unwrap_or_default();
    reporter_assert!(reporter, glyphInfo.grapheme_cluster_text_range.start == 0);
    reporter_assert!(reporter, glyphInfo.grapheme_cluster_text_range.end == 2);
    let rect1: Rect = glyphInfo.grapheme_layout_bounds;
    reporter_assert!(reporter, rect0 == rect1);

    let found = paragraph.get_glyph_info_at_utf16_offset(2);
    reporter_assert!(reporter, found.is_some());
    glyphInfo = found.unwrap_or_default();
    reporter_assert!(reporter, glyphInfo.grapheme_cluster_text_range.start == 2);
    reporter_assert!(reporter, glyphInfo.grapheme_cluster_text_range.end == 4);
    let rect2: Rect = glyphInfo.grapheme_layout_bounds;
    // The latter half of the text forms a different grapheme.
    reporter_assert!(reporter, rect2 != rect1);

    let found = paragraph.get_glyph_info_at_utf16_offset(3);
    reporter_assert!(reporter, found.is_some());
    glyphInfo = found.unwrap_or_default();
    reporter_assert!(reporter, glyphInfo.grapheme_cluster_text_range.start == 2);
    reporter_assert!(reporter, glyphInfo.grapheme_cluster_text_range.end == 4);
    let rect3: Rect = glyphInfo.grapheme_layout_bounds;
    reporter_assert!(reporter, rect2 == rect3);

    // RTL text.
    reporter_assert!(reporter, fullRect.left == rect2.left);
    reporter_assert!(reporter, fullRect.right == rect0.right);

    // Currently it seems the 2 grapheme rects do not overlap each other.
    // REPORTER_ASSERT(reporter, rect2.right() < rect0.left());
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8297-L8329 (chrome/m156)
def_paragraph_test!(SkParagraph_SingleDummyPlaceholder, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let text = "Single dummy placeholder";

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH);

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.placeholders().len() == 1);

    let mut index: usize = 0;
    for line in impl_.lines() {
        line.scan_styles(
            impl_,
            StyleType::Decorations,
            &mut |_text_range, style, _context| {
                reporter_assert!(reporter, index == 0);
                reporter_assert!(reporter, style.color() == Color::BLACK);
                index += 1;
            },
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8331-L8359 (chrome/m156)
def_paragraph_test!(SkParagraph_EndWithLineSeparator, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = "A text ending with line separator.\u{2028}";

    let paragraph_style = ParagraphStyle::new();

    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    let mut textStyle = TextStyle::new();
    textStyle.set_font_families(&["Roboto"]);
    builder.push_style(&textStyle);
    builder.add_text(text);

    let mut paragraph = builder.build();
    paragraph.layout(SCALAR_MAX);

    let mut visitedCount: i32 = 0;
    paragraph.visit(|lineNumber, info| {
        visitedCount += 1;
        if lineNumber == 1 {
            // Visitor for second line created from line separator should only be called for 'end of line'.
            // 'end of line' is denoted by 'info' being nullptr.
            reporter_assert!(reporter, info.is_none());
        }
    });
    reporter_assert!(
        reporter,
        visitedCount == 3,
        "visitedCount: {}",
        visitedCount
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8361-L8397 (chrome/m156)
#[allow(non_snake_case)]
fn SkParagraph_EmojiFontResolution(_icu: Arc<dyn Unicode>, reporter: &mut Reporter) {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = "\u{267B}\u{FE0F}\u{1F3F4}\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}";
    let text1 = "\u{267B}\u{FE0F}";

    let paragraph_style = ParagraphStyle::new();
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&[""]);
    builder.push_style(&text_style);
    builder.add_text(text);
    let mut paragraph = builder.build();
    paragraph.layout(SCALAR_MAX);

    let impl_ = &paragraph;
    reporter_assert!(
        reporter,
        impl_.runs().len() == 1,
        "size: {}",
        impl_.runs().len()
    );

    let mut builder1 = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
    builder1.push_style(&text_style);
    builder1.add_text(text1);
    let mut paragraph1 = builder1.build();
    paragraph1.layout(SCALAR_MAX);

    let impl1 = &paragraph1;
    reporter_assert!(
        reporter,
        impl1.runs().len() == 1,
        "size: {}",
        impl1.runs().len()
    );
    if impl1.runs().len() == 1 {
        let ff = impl_.run(0).font().typeface().family_name();
        let ff1 = impl1.run(0).font().typeface().family_name();
        reporter_assert!(reporter, ff == ff1);
    }
}

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8411-L8473 (chrome/m156)
#[allow(non_snake_case)]
fn SkUnicode_Emoji(icu: &dyn Unicode, reporter: &mut Reporter) {
    let mut test = |text: &str, expected: Unichar| {
        // (The `if ((false))` debug printing of the C++ is not ported.)
        let mut begin: &[u8] = text.as_bytes();
        let emojiStart = get_emoji_sequence_start(icu, &mut begin);
        reporter_assert!(reporter, expected == emojiStart);
    };

    test("", -1);
    test("0", -1);
    test("2nd", -1);
    test("99", -1);
    test("0\u{FE0F}\u{20E3}", 48);
    test("0\u{FE0F}\u{20E3}12", 48);
    test("#", -1);
    test("#\u{FE0F}\u{20E3}", 35);
    test("#\u{FE0F}\u{20E3}#", 35);
    test("#\u{FE0F}\u{20E3}#\u{FE0F}\u{20E3}", 35);
    test("*", -1);
    test("*\u{FE0F}\u{20E3}", 42);
    test("*\u{FE0F}\u{20E3}abc", 42);
    test("*\u{FE0F}\u{20E3}\u{1F60A}", 42);
    test("\u{1F60A}", 128522);
    test("\u{1F60A}abc", 128522);
    test("\u{1F60A}*\u{FE0F}\u{20E3}", 128522);
    test(
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F466}\u{200D}\u{1F466}",
        128104,
    );
    test("\u{1F1F7}\u{1F1FA}", 127479); // Flag sequence
    test("0\u{FE0F}\u{20E3}", 48); // Keycap sequence
    test(
        "\u{1F3F4}\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}",
        127988,
    ); // Tag sequence
    test("\u{1F44B}\u{1F3FC}", 128075); // Modifier sequence
    test(
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}",
        128104,
    ); // ZWJ sequence
}

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8400-L8402 (chrome/m156)
def_paragraph_test!(SkParagraph_ICU_EmojiFontResolution, |reporter| {
    SkParagraph_EmojiFontResolution(get_unicode(), reporter);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8476-L8514 (chrome/m156)
def_paragraph_test!(SkParagraph_ArabicNoLetterSpacing, |reporter| {
    let font_collection = ResourceFontCollection::new(false);
    skip_if_fonts_not_found!(reporter, font_collection);
    let mut canvas = TestCanvas::new("SkParagraph_NoSpacesArabicParagraph.png");
    let arabic = "سلام";
    let english = "Hello";

    let paragraph_style = ParagraphStyle::new();
    let mut text_style = TextStyle::new();
    text_style.set_font_size(10.0);
    text_style.set_color(Color::BLACK);

    let mut layout = |familyName: &str, text: &str, letterSpacing: f64| -> f64 {
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
        text_style.set_letter_spacing(letterSpacing as scalar);
        text_style.set_font_families(&[familyName]);
        builder.push_style(&text_style);
        builder.add_text(text);
        builder.pop();

        let mut paragraph = builder.build();
        paragraph.layout(TEST_CANVAS_WIDTH);
        paragraph.paint(canvas.get(), (20.0, 0.0));
        canvas.get().translate((0.0, paragraph.height() + 20.0));
        f64::from(paragraph.longest_line())
    };

    {
        // Letter spacing does not affect the layout for Arabic
        let noLetterSpacing = layout("Katibeh", arabic, 0.0);
        let withLetterSpacing = layout("Katibeh", arabic, 100.0);
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(noLetterSpacing, withLetterSpacing, EPSILON1000)
        );
    }
    {
        // Letter spacing affects English text (number of letter * letter spacing)
        let noLetterSpacing = layout("Roboto", english, 0.0);
        let withLetterSpacing = layout("Roboto", english, 100.0);
        reporter_assert!(
            reporter,
            sk_scalar_nearly_equal!(
                withLetterSpacing - noLetterSpacing,
                100.0 * english.len() as f64,
                EPSILON1000
            )
        );
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8516-L8551 (chrome/m156)
def_paragraph_test!(
    SkParagraph_ArabicWithLetterSpacingWhitespacesAsChrome,
    |reporter| {
        let font_collection = ResourceFontCollection::new(false);
        skip_if_fonts_not_found!(reporter, font_collection);
        let mut canvas = TestCanvas::new("SkParagraph_SpacedArabicParagraph.png");
        let arabic = "سلام";
        let spaced = "س لا م";
        let paragraph_style = ParagraphStyle::new();
        let mut text_style = TextStyle::new();
        text_style.set_font_size(20.0);
        text_style.set_color(Color::BLACK);

        let mut layout = |familyName: &str, text: &str, letterSpacing: f64| -> f64 {
            let mut builder =
                ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
            text_style.set_letter_spacing(letterSpacing as scalar);
            text_style.set_font_families(&[familyName]);
            builder.push_style(&text_style);
            builder.add_text(text);
            builder.pop();

            let mut paragraph = builder.build();
            paragraph.layout(TEST_CANVAS_WIDTH);
            paragraph.paint(canvas.get(), (20.0, 0.0));
            canvas.get().translate((0.0, paragraph.height() + 20.0));
            f64::from(paragraph.longest_line())
        };
        {
            // Letter spacing does not show for Arabic without whitespaces
            let noLetterSpacing = layout("Katibeh", arabic, 0.0);
            let withLetterSpacing = layout("Katibeh", arabic, 10.0);
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(withLetterSpacing, noLetterSpacing, EPSILON100)
            );
        }
        {
            // Letter spacing shows for Arabic on whitespaces
            let noLetterSpacing = layout("Katibeh", spaced, 0.0);
            let withLetterSpacing = layout("Katibeh", spaced, 10.0);
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(withLetterSpacing, noLetterSpacing, EPSILON100)
            );
        }
    }
);

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8553-L8589 (chrome/m156)
def_paragraph_test!(
    SkParagraph_ArabicWithLetterSpacingWhitespacesAsCSS,
    |reporter| {
        let font_collection = ResourceFontCollection::new(false);
        skip_if_fonts_not_found!(reporter, font_collection);
        let mut canvas = TestCanvas::new("SkParagraph_SpacedArabicParagraph.png");
        let arabic = "سلام";
        let spaced = "س لا م";
        let mut paragraph_style = ParagraphStyle::new();
        paragraph_style.set_letter_spacing_by_css_spec(true);
        let mut text_style = TextStyle::new();
        text_style.set_font_size(20.0);
        text_style.set_color(Color::BLACK);

        let mut layout = |familyName: &str, text: &str, letterSpacing: f64| -> f64 {
            let mut builder =
                ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());
            text_style.set_letter_spacing(letterSpacing as scalar);
            text_style.set_font_families(&[familyName]);
            builder.push_style(&text_style);
            builder.add_text(text);
            builder.pop();

            let mut paragraph = builder.build();
            paragraph.layout(TEST_CANVAS_WIDTH);
            paragraph.paint(canvas.get(), (20.0, 0.0));
            canvas.get().translate((0.0, paragraph.height() + 20.0));
            f64::from(paragraph.longest_line())
        };
        {
            // Letter spacing does not show for Arabic without whitespaces
            let noLetterSpacing = layout("Katibeh", arabic, 0.0);
            let withLetterSpacing = layout("Katibeh", arabic, 10.0);
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(withLetterSpacing, noLetterSpacing, EPSILON100)
            );
        }
        {
            // Letter spacing shows for Arabic on whitespaces
            let noLetterSpacing = layout("Katibeh", spaced, 0.0);
            let withLetterSpacing = layout("Katibeh", spaced, 10.0);
            reporter_assert!(
                reporter,
                sk_scalar_nearly_equal!(withLetterSpacing, noLetterSpacing + 20.0, EPSILON100)
            );
        }
    }
);

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8595-L8637 (chrome/m156)
def_paragraph_test!(SkParagraph_SoftHyphenAtLineBreak, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);

    // "inter\u00ADnational" — soft hyphen between "inter" and "national"
    // With a narrow width, it should break at the soft hyphen and render a visible hyphen
    let text = concat!("inter\u{AD}", "national",);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_render_soft_hyphens(true);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    // Narrow width forces a break at the soft hyphen. We don't assert on the
    // exact line count because glyph advances vary across font versions; we
    // verify the feature works by checking the hyphen run's structural state.
    paragraph.layout(80.0);

    let impl_ = &paragraph;
    reporter_assert!(reporter, !impl_.lines().is_empty());

    // Structural check: the first line should have a hyphen run attached.
    // This is the feature's output regardless of how many total lines the
    // text breaks into, which depends on font metrics.
    let firstLine = &impl_.lines()[0];
    reporter_assert!(reporter, firstLine.hyphen().is_some());
    if firstLine.hyphen().is_none() {
        return;
    }
    // The hyphen run should have positive advance width
    reporter_assert!(reporter, firstLine.hyphen().unwrap().advance().x > 0.0);
    // The line's total width should include the hyphen
    reporter_assert!(
        reporter,
        firstLine.width() > firstLine.width_without_ellipsis()
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8639-L8670 (chrome/m156)
def_paragraph_test!(SkParagraph_SoftHyphenNoBreak, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);

    // Same text but with enough width to fit on one line — no hyphen should render
    let text = concat!("inter\u{AD}", "national",);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_render_soft_hyphens(true);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(TEST_CANVAS_WIDTH); // Wide enough for one line

    let impl_ = &paragraph;
    reporter_assert!(reporter, impl_.lines().len() == 1);

    // No hyphen should be rendered when soft hyphen is not at a line break
    let firstLine = &impl_.lines()[0];
    reporter_assert!(reporter, firstLine.hyphen().is_none());
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8672-L8709 (chrome/m156)
def_paragraph_test!(SkParagraph_MultipleSoftHyphens, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);

    // "su\u00ADper\u00ADcali\u00ADfrag" — multiple soft hyphens
    // Only the one at the actual line break should render
    let text = concat!("su\u{AD}", "per\u{AD}", "cali\u{AD}", "frag",);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_render_soft_hyphens(true);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    // Narrow width forces a break at one of the soft hyphens.
    paragraph.layout(80.0);

    let impl_ = &paragraph;
    reporter_assert!(reporter, !impl_.lines().is_empty());

    // The first line should have a hyphen (it breaks at a soft hyphen).
    let firstLine = &impl_.lines()[0];
    reporter_assert!(reporter, firstLine.hyphen().is_some());
    if firstLine.hyphen().is_none() {
        return;
    }
    reporter_assert!(reporter, firstLine.hyphen().unwrap().advance().x > 0.0);
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8711-L8747 (chrome/m156)
def_paragraph_test!(SkParagraph_SoftHyphenLineWidth, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = concat!("inter\u{AD}", "national",);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_render_soft_hyphens(true);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(80.0);

    let impl_ = &paragraph;
    reporter_assert!(reporter, !impl_.lines().is_empty());

    let firstLine = &impl_.lines()[0];
    reporter_assert!(reporter, firstLine.hyphen().is_some());
    if firstLine.hyphen().is_none() {
        return;
    }

    // Line width should equal text width + hyphen width
    let textWidth: scalar = firstLine.width_without_ellipsis();
    let hyphenWidth: scalar = firstLine.hyphen().unwrap().advance().x;
    reporter_assert!(
        reporter,
        sk_scalar_nearly_equal!(firstLine.width(), textWidth + hyphenWidth, EPSILON100)
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8749-L8786 (chrome/m156)
def_paragraph_test!(SkParagraph_SoftHyphenRTL, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);

    // Arabic text with soft hyphens: "كلمة\u00ADطويلة" (kalima-tawila = "long word")
    let text = concat!(
        "\u{643}\u{644}\u{645}\u{629}",
        "\u{AD}",
        "\u{637}\u{648}\u{64A}\u{644}\u{629}",
    );

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_render_soft_hyphens(true);
    paragraph_style.set_text_direction(TextDirection::RTL);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Noto Naskh Arabic"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    // Arabic glyphs are wider per character; narrow width forces a break.
    paragraph.layout(40.0);

    let impl_ = &paragraph;
    reporter_assert!(reporter, !impl_.lines().is_empty());

    let firstLine = &impl_.lines()[0];
    reporter_assert!(reporter, firstLine.hyphen().is_some());
    if firstLine.hyphen().is_none() {
        return;
    }
    reporter_assert!(reporter, firstLine.hyphen().unwrap().advance().x > 0.0);
    // Line width should include the hyphen
    reporter_assert!(
        reporter,
        firstLine.width() > firstLine.width_without_ellipsis()
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8790-L8824 (chrome/m156)
def_paragraph_test!(SkParagraph_SoftHyphenDefaultOff, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = concat!("inter\u{AD}", "national",);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    // Intentionally do NOT call setRenderSoftHyphens — default must be false.
    reporter_assert!(reporter, !paragraph_style.render_soft_hyphens());
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    paragraph.layout(80.0);

    let impl_ = &paragraph;
    reporter_assert!(reporter, !impl_.lines().is_empty());

    // No hyphen run should be created on any line when rendering is disabled.
    for line in impl_.lines() {
        // FIXME-CONVERT
        reporter_assert!(reporter, line.hyphen().is_none());
    }
    let firstLine = &impl_.lines()[0];
    reporter_assert!(
        reporter,
        firstLine.width() == firstLine.width_without_ellipsis()
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8829-L8885 (chrome/m156)
def_paragraph_test!(SkParagraph_SoftHyphenGetRectsForRange, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = concat!("inter\u{AD}", "national",);
    let textLen = text.len();

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.turn_hinting_off();
    paragraph_style.set_render_soft_hyphens(true);
    let mut builder = ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

    let mut text_style = TextStyle::new();
    text_style.set_font_families(&["Roboto"]);
    text_style.set_font_size(20.0);
    text_style.set_color(Color::BLACK);
    builder.push_style(&text_style);
    builder.add_text(text);
    builder.pop();

    let mut paragraph = builder.build();
    // Narrow width forces a break at the soft hyphen. Verify via structural
    // checks (hyphen run on first line) rather than exact line counts, since
    // those depend on font metrics that vary across platforms.
    paragraph.layout(80.0);

    let impl_ = &paragraph;
    reporter_assert!(reporter, !impl_.lines().is_empty());

    let firstLine = &impl_.lines()[0];
    reporter_assert!(reporter, firstLine.hyphen().is_some());
    if firstLine.hyphen().is_none() {
        return;
    }
    let textOnlyWidth: scalar = firstLine.width_without_ellipsis();
    let lineWidth: scalar = firstLine.width();
    reporter_assert!(reporter, lineWidth > textOnlyWidth); // hyphen contributes width

    // Request rects for the entire source text. None of the returned rects on the
    // first line should extend past the bare-text advance into the hyphen pixels.
    let boxes =
        paragraph.get_rects_for_range(0..textLen, RectHeightStyle::Tight, RectWidthStyle::Tight);
    reporter_assert!(reporter, !boxes.is_empty());

    let mut firstLineMaxRight: scalar = 0.0;
    let firstLineY: scalar = firstLine.offset().y + firstLine.height() / 2.0;
    for box_ in boxes {
        // FIXME-CONVERT
        if box_.rect.top <= firstLineY && box_.rect.bottom >= firstLineY {
            firstLineMaxRight = std_max(firstLineMaxRight, box_.rect.right); // FIXME-CONVERT
        }
    }
    // The right edge of any rect on the first line must not exceed the bare-text
    // advance (with a small epsilon). If the hyphen were producing a spurious rect,
    // this would equal firstLine.width(), not widthWithoutEllipsis().
    let firstLineLeft: scalar = firstLine.offset().x;
    reporter_assert!(
        reporter,
        firstLineMaxRight <= firstLineLeft + textOnlyWidth + EPSILON100
    );
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8890-L8931 (chrome/m156)
def_paragraph_test!(
    SkParagraph_SoftHyphenGlyphPositionAtCoordinate,
    |reporter| {
        let font_collection = ResourceFontCollection::new(true);
        skip_if_fonts_not_found!(reporter, font_collection);

        let text = concat!("inter\u{AD}", "national",);

        let mut paragraph_style = ParagraphStyle::new();
        paragraph_style.turn_hinting_off();
        paragraph_style.set_render_soft_hyphens(true);
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

        let mut text_style = TextStyle::new();
        text_style.set_font_families(&["Roboto"]);
        text_style.set_font_size(20.0);
        text_style.set_color(Color::BLACK);
        builder.push_style(&text_style);
        builder.add_text(text);
        builder.pop();

        let mut paragraph = builder.build();
        paragraph.layout(80.0);

        let impl_ = &paragraph;
        reporter_assert!(reporter, !impl_.lines().is_empty());
        let firstLine = &impl_.lines()[0];
        reporter_assert!(reporter, firstLine.hyphen().is_some());
        if firstLine.hyphen().is_none() {
            return;
        }

        let lineY: scalar = firstLine.offset().y + firstLine.height() / 2.0;
        let textRight: scalar = firstLine.offset().x + firstLine.width_without_ellipsis();
        let lineRight: scalar = firstLine.offset().x + firstLine.width();

        // Click inside the rendered hyphen pixel region (between text end and line end).
        let inHyphen =
            paragraph.get_glyph_position_at_coordinate(((textRight + lineRight) / 2.0, lineY));
        // The position should land at the end of the first line's source text -- i.e.
        // somewhere within the soft-hyphen cluster. It must NOT be greater than the
        // start of the second line's "national" text (position 7 = after "inter\xC2\xAD").
        reporter_assert!(reporter, inHyphen.position <= 7);
        reporter_assert!(reporter, inHyphen.position >= 5);
    }
);

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8935-L8970 (chrome/m156)
def_paragraph_test!(SkParagraph_SoftHyphenWordBoundary, |reporter| {
    let font_collection = ResourceFontCollection::new(true);
    skip_if_fonts_not_found!(reporter, font_collection);

    let text = concat!("inter\u{AD}", "national",);

    let buildParagraph = |renderHyphens: bool| {
        let mut paragraph_style = ParagraphStyle::new();
        paragraph_style.turn_hinting_off();
        paragraph_style.set_render_soft_hyphens(renderHyphens);
        let mut builder =
            ParagraphBuilderImpl::new(&paragraph_style, &font_collection, get_unicode());

        let mut text_style = TextStyle::new();
        text_style.set_font_families(&["Roboto"]);
        text_style.set_font_size(20.0);
        text_style.set_color(Color::BLACK);
        builder.push_style(&text_style);
        builder.add_text(text);
        builder.pop();
        let mut paragraph = builder.build();
        paragraph.layout(80.0);
        return paragraph;
    };

    let mut withHyphen = buildParagraph(true);
    let mut withoutHyphen = buildParagraph(false);

    // Word boundaries must be identical regardless of soft hyphen rendering.
    for i in 0..text.len() {
        let a = withHyphen.get_word_boundary(i as u32);
        let b = withoutHyphen.get_word_boundary(i as u32);
        reporter_assert!(reporter, a == b);
    }
});

// Port of: modules/skparagraph/tests/SkParagraphTest.cpp#L8973-L8975 (chrome/m156)
def_paragraph_test!(SkParagraph_ICU_EmojiRuns, |reporter| {
    SkUnicode_Emoji(&*get_unicode(), reporter);
});
