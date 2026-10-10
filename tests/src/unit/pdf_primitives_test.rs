// Copyright 2010 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PDFPrimitivesTest.cpp (chrome/m156), all of it but `SkPDF_Primitives` (its
// `test_issue1083` draws a glyph through a PDF canvas) and `SkPDF_FontCanEmbedTypeface`, which
// wait for the PDF fonts (modules.md M26).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{AutoCanvasRestore, SaveLayerRec};
use skia_rust_core::color::Color4f;
use skia_rust_core::font::Font;
use skia_rust_core::glyph_run::GlyphRun;
use skia_rust_core::image_filter::{ImageFilter, ImageFilterBase, ImageFilterCommon};
use skia_rust_core::image_filter_result::FilterResult;
use skia_rust_core::image_filter_types::{Context, Mapping};
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::shader::Shader;
use skia_rust_core::stream::{DynamicMemoryWStream, NullWStream};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::utils::parse_path;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_effects::image_filters;
use skia_rust_effects::perlin_noise_shader::shaders as perlin_shaders;
use skia_rust_pdf::clusterator::{Cluster, Clusterator};
use skia_rust_pdf::float_to_decimal::{MAXIMUM_SK_FLOAT_TO_DECIMAL_LENGTH, float_to_decimal};
use skia_rust_pdf::jpeg;
use skia_rust_pdf::metadata::Metadata;
use skia_rust_pdf::new_document;
use skia_rust_pdf::utils::{EmptyArea, EmptyPath, EmptyVerb, color_to_decimal, emit_path};
use skia_rust_tools::font_tool_utils::default_font;

use crate::{Reporter, def_test, errorf, reporter_assert};

/// The bytes of a C string: up to the first NUL.
fn c_str(buf: &[u8]) -> &[u8] {
    match buf.iter().position(|&b| b == 0) {
        Some(end) => &buf[..end],
        None => buf,
    }
}

// Port of: tests/PDFPrimitivesTest.cpp#L386-L404 (chrome/m156)
// test to see that all finite scalars round trip via scanf().
#[allow(clippy::float_cmp)] // the test compares exactly, as `roundTripFloat != inputFloat` does
fn check_pdf_scalar_serialization(reporter: &mut Reporter, input_float: f32) {
    let mut float_string = [0u8; MAXIMUM_SK_FLOAT_TO_DECIMAL_LENGTH];
    let len = float_to_decimal(input_float, &mut float_string);
    if len >= float_string.len() {
        errorf!(reporter, "string too long: {}", len);
        return;
    }
    if float_string[len] != 0 || c_str(&float_string).len() != len {
        errorf!(reporter, "terminator misplaced.");
        return; // The terminator is needed for sscanf().
    }
    let text = std::str::from_utf8(&float_string[..len]).unwrap_or("");
    let Ok(round_trip_float) = text.parse::<f32>() else {
        errorf!(reporter, "unscannable result: {}", text);
        return;
    };
    // `SkIsFinite(inputFloat) && roundTripFloat != inputFloat`
    if input_float.is_finite() && round_trip_float != input_float {
        errorf!(
            reporter,
            "roundTripFloat ({:.9e}) != inputFloat ({:.9e})",
            round_trip_float,
            input_float
        );
    }
}

// Port of: tests/PDFPrimitivesTest.cpp#L386-L404 (chrome/m156)
// Test SkPDFUtils::AppendScalar for accuracy.
def_test!(SkPDF_Primitives_Scalar, |reporter| {
    let mut random = Random::new(0x5EED);
    let mut iteration_count = 512;
    while iteration_count > 0 {
        iteration_count -= 1;
        // Any bit pattern, so NaNs and infinities are included too.
        let f = f32::from_bits(random.next_u());
        check_pdf_scalar_serialization(reporter, f);
    }
    let always_check = [
        0.0f32,
        -0.0,
        1.0,
        -1.0,
        std::f32::consts::PI,
        0.1,
        f32::MIN_POSITIVE,
        f32::MAX,
        -f32::MIN_POSITIVE,
        -f32::MAX,
        f32::MIN_POSITIVE / 16.0,
        -f32::MIN_POSITIVE / 16.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        -f32::MIN_POSITIVE / 8_388_608.0,
    ];
    for input_float in always_check {
        check_pdf_scalar_serialization(reporter, input_float);
    }
});

// Port of: tests/PDFPrimitivesTest.cpp#L407-L417 (chrome/m156)
// Test SkPDFUtils:: for accuracy.
def_test!(SkPDF_Primitives_Color, |reporter| {
    let mut buffer = [0u8; 5];
    for i in 0..=255u8 {
        let len = color_to_decimal(i, &mut buffer);
        let text = std::str::from_utf8(c_str(&buffer)).unwrap_or("");
        reporter_assert!(reporter, len == text.len());
        let parsed = text.parse::<f32>();
        reporter_assert!(reporter, parsed.is_ok());
        let f = parsed.unwrap_or(0.0);
        // `int roundTrip = (int)(0.5 + f * 255);`: `f * 255` is a float, the sum a double.
        #[allow(clippy::cast_possible_truncation)] // the C++ `(int)` cast of a value in range
        let round_trip = (0.5 + f64::from(f * 255.0)) as i32;
        reporter_assert!(reporter, round_trip == i32::from(i));
    }
});

// Port of: tests/PDFPrimitivesTest.cpp#L501-L580 (chrome/m156)
def_test!(SkPDF_EmitPath, |reporter| {
    // Selector bits of the test table.
    const DISCARD_EMPTY_PATH: u32 = 1 << 0;
    const DISCARD_EMPTY_VERB: u32 = 1 << 1;
    const DISCARD_EMPTY_AREA: u32 = 1 << 2;

    // (svg path, expected PDF, discard mask). The default mask discards nothing.
    let g_tests: &[(&str, &str, u32)] = &[
        // Empty path
        ("", "0 0 0 0 re\n", 0),
        ("", "", DISCARD_EMPTY_PATH),
        ("M10,10", "", 0), // TODO: this looks iffy, we prolly want an empty rect here too?
        ("M10,10", "", DISCARD_EMPTY_PATH),
        // Empty verb
        ("M10,10 L20,20 L20,20", "10 10 m\n20 20 l\n20 20 l\n", 0),
        (
            "M10,10 L20,20 L20,20",
            "10 10 m\n20 20 l\n",
            DISCARD_EMPTY_VERB,
        ),
        // Empty area
        ("M10,10 L10,10", "10 10 m\n10 10 l\n", 0),
        ("M10,10 L10,10", "", DISCARD_EMPTY_AREA),
        ("M10,10 L20,20", "10 10 m\n20 20 l\n", 0),
        ("M10,10 L20,20", "", DISCARD_EMPTY_AREA),
        ("M10,10 L20,20 L30,30", "10 10 m\n20 20 l\n30 30 l\n", 0),
        ("M10,10 L20,20 L30,30", "", DISCARD_EMPTY_AREA),
        ("M10,10 L20,20 L0,0", "10 10 m\n20 20 l\n0 0 l\n", 0),
        ("M10,10 L20,20 L0,0", "", DISCARD_EMPTY_AREA),
        (
            "M5,5   M10,10 L20,20 L20 30",
            "10 10 m\n20 20 l\n20 30 l\n",
            0,
        ),
        (
            "M5,5   M10,10 L20,20 L20 30",
            "10 10 m\n20 20 l\n20 30 l\n",
            DISCARD_EMPTY_AREA,
        ),
        (
            "M5,5   M10,10 L20,20 L20 30 Z",
            "10 10 m\n20 20 l\n20 30 l\n10 10 l\nh\n",
            0,
        ),
        (
            "M5,5   M10,10 L20,20 L20 30 Z",
            "10 10 m\n20 20 l\n20 30 l\n10 10 l\nh\n",
            DISCARD_EMPTY_AREA,
        ),
        (
            "M5,5   M0,10 L20,20   M10,10 L20,20 L20,0   M10,0 L20,20",
            "0 10 m\n20 20 l\n10 10 m\n20 20 l\n20 0 l\n10 0 m\n20 20 l\n",
            0,
        ),
        (
            "M5,5   M0,10 L20,20   M10,10 L20,20 L20,0   M10,0 L20,20",
            "10 10 m\n20 20 l\n20 0 l\n",
            DISCARD_EMPTY_AREA,
        ),
        (
            "M5,5   M0,10 L20,20 Z   M10,10 L20,20 L20,0 Z   M10,0 L20,20 Z",
            "0 10 m\n20 20 l\n0 10 l\nh\n10 10 m\n20 20 l\n20 0 l\n10 10 l\nh\n\
             10 0 m\n20 20 l\n10 0 l\nh\n",
            0,
        ),
        (
            "M5,5   M0,10 L20,20 Z   M10,10 L20,20 L20,0 Z   M10,0 L20,20 Z",
            "10 10 m\n20 20 l\n20 0 l\n10 10 l\nh\n",
            DISCARD_EMPTY_AREA,
        ),
    ];

    let mut str_ = DynamicMemoryWStream::new();
    for &(svg_path, expected_pdf, discard_mask) in g_tests {
        let src = parse_path::from_svg(svg_path).unwrap_or_default();
        let empty_path = if discard_mask & DISCARD_EMPTY_PATH != 0 {
            EmptyPath::Discard
        } else {
            EmptyPath::Preserve
        };
        let empty_verb = if discard_mask & DISCARD_EMPTY_VERB != 0 {
            EmptyVerb::Discard
        } else {
            EmptyVerb::Preserve
        };
        let empty_area = if discard_mask & DISCARD_EMPTY_AREA != 0 {
            EmptyArea::Discard
        } else {
            EmptyArea::Preserve
        };

        let did_emit = emit_path(&src, empty_path, empty_verb, empty_area, &mut str_, 0.25);
        let result = str_.detach_as_vector();

        reporter_assert!(
            reporter,
            result == expected_pdf.as_bytes(),
            "*** Unexpected PDF path for \"{}\":\n---\n{}---\nExpected:\n---\n{}---\n",
            svg_path,
            String::from_utf8_lossy(&result),
            expected_pdf
        );
        reporter_assert!(reporter, did_emit != result.is_empty());
    }
});

/// `TestImageFilter`: an image filter that records that it was asked to filter.
// Port of: tests/PDFPrimitivesTest.cpp#L276-L318 (chrome/m156)
#[derive(Debug)]
struct TestImageFilter {
    common: ImageFilterCommon,
    visited: Arc<AtomicBool>,
}

impl TestImageFilter {
    fn new(visited: Arc<AtomicBool>) -> Self {
        Self {
            common: ImageFilterCommon::new(Vec::new(), Some(false)),
            visited,
        }
    }
}

impl ImageFilterBase for TestImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    fn on_filter_image(&self, ctx: &Context<'_>) -> FilterResult {
        self.visited.store(true, Ordering::SeqCst);
        ctx.source().clone()
    }

    fn on_get_input_layer_bounds(
        &self,
        _mapping: &Mapping,
        desired_output: IRect,
        _content_bounds: Option<IRect>,
    ) -> IRect {
        desired_output
    }

    fn on_get_output_layer_bounds(
        &self,
        _mapping: &Mapping,
        content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        content_bounds
    }
}

// Port of: tests/PDFPrimitivesTest.cpp#L320-L339 (chrome/m156)
// Check that PDF rendering of image filters successfully falls back to
// CPU rasterization.
def_test!(SkPDF_ImageFilter, |reporter| {
    let mut stream = DynamicMemoryWStream::new();
    let visited = Arc::new(AtomicBool::new(false));
    {
        let mut doc = new_document(&mut stream, Some(&jpeg::metadata_with_callbacks()));
        let canvas = doc.begin_page(100.0, 100.0, None).expect("a canvas");

        let filter = ImageFilter::from_base(TestImageFilter::new(Arc::clone(&visited)));

        // Filter just created; should be unvisited.
        reporter_assert!(reporter, !visited.load(Ordering::SeqCst));
        let mut paint = Paint::default();
        paint.set_image_filter(filter);
        canvas.draw_rect(Rect::from_wh(100.0, 100.0), &paint);
        doc.close();
    }

    // Filter was used in rendering; should be visited.
    reporter_assert!(reporter, visited.load(Ordering::SeqCst));
});

/// `make_run`: a glyph run over the given arrays.
// Port of: tests/PDFPrimitivesTest.cpp#L452-L461 (chrome/m156)
fn make_run(
    glyphs: &[u16],
    pos: &[Point],
    font: &Font,
    clusters: &[u32],
    utf8_text: &[u8],
) -> GlyphRun {
    GlyphRun::new(
        font.clone(),
        pos.to_vec(),
        glyphs.to_vec(),
        utf8_text.to_vec(),
        clusters.to_vec(),
        Vec::new(),
    )
}

// Port of: tests/PDFPrimitivesTest.cpp#L463-L511 (chrome/m156)
def_test!(SkPDF_Clusterator, |reporter| {
    let font = default_font();
    {
        const LEN: usize = 11;
        let clusters: [u32; LEN] = [3, 2, 2, 1, 0, 4, 4, 7, 6, 6, 5];
        let glyphs: [u16; LEN] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        let pos = [Point::new(0.0, 0.0); LEN];
        let text = b"abcdefgh";
        let run = make_run(&glyphs, &pos, &font, &clusters, text);
        let mut clusterator = Clusterator::new(&run);
        let at = |i: usize| Some(&run.text()[i..=i]);
        let expectations = [
            Cluster {
                utf8_text: at(3),
                text_byte_length: 1,
                glyph_index: 0,
                glyph_count: 1,
            },
            Cluster {
                utf8_text: at(2),
                text_byte_length: 1,
                glyph_index: 1,
                glyph_count: 2,
            },
            Cluster {
                utf8_text: at(1),
                text_byte_length: 1,
                glyph_index: 3,
                glyph_count: 1,
            },
            Cluster {
                utf8_text: at(0),
                text_byte_length: 1,
                glyph_index: 4,
                glyph_count: 1,
            },
            Cluster {
                utf8_text: at(4),
                text_byte_length: 1,
                glyph_index: 5,
                glyph_count: 2,
            },
            Cluster {
                utf8_text: at(7),
                text_byte_length: 1,
                glyph_index: 7,
                glyph_count: 1,
            },
            Cluster {
                utf8_text: at(6),
                text_byte_length: 1,
                glyph_index: 8,
                glyph_count: 2,
            },
            Cluster {
                utf8_text: at(5),
                text_byte_length: 1,
                glyph_index: 10,
                glyph_count: 1,
            },
            Cluster {
                utf8_text: None,
                text_byte_length: 0,
                glyph_index: 0,
                glyph_count: 0,
            },
        ];
        for expectation in &expectations {
            reporter_assert!(reporter, clusterator.next() == *expectation);
        }
    }
    {
        const LEN: usize = 5;
        let clusters: [u32; LEN] = [0, 1, 4, 5, 6];
        let glyphs: [u16; LEN] = [43, 167, 79, 79, 82];
        let pos = [Point::new(0.0, 0.0); LEN];
        let text = b"Ha\xCC\x8Allo";
        let run = make_run(&glyphs, &pos, &font, &clusters, text);
        let mut clusterator = Clusterator::new(&run);
        let t = run.text();
        let expectations = [
            Cluster {
                utf8_text: Some(&t[0..1]),
                text_byte_length: 1,
                glyph_index: 0,
                glyph_count: 1,
            },
            Cluster {
                utf8_text: Some(&t[1..4]),
                text_byte_length: 3,
                glyph_index: 1,
                glyph_count: 1,
            },
            Cluster {
                utf8_text: Some(&t[4..5]),
                text_byte_length: 1,
                glyph_index: 2,
                glyph_count: 1,
            },
            Cluster {
                utf8_text: Some(&t[5..6]),
                text_byte_length: 1,
                glyph_index: 3,
                glyph_count: 1,
            },
            Cluster {
                utf8_text: Some(&t[6..7]),
                text_byte_length: 1,
                glyph_index: 4,
                glyph_count: 1,
            },
            Cluster {
                utf8_text: None,
                text_byte_length: 0,
                glyph_index: 0,
                glyph_count: 0,
            },
        ];
        for expectation in &expectations {
            reporter_assert!(reporter, clusterator.next() == *expectation);
        }
    }
});

// Port of: tests/PDFPrimitivesTest.cpp#L513-L535 (chrome/m156)
def_test!(fuzz875632f0, |_reporter| {
    let mut stream = NullWStream::new();
    let mut doc = new_document(&mut stream, Some(&jpeg::metadata_with_callbacks()));
    let canvas = doc.begin_page(128.0, 160.0, None).expect("a canvas");

    let _auto_canvas_restore = AutoCanvasRestore::guard(canvas, false);

    let mut layer_paint = Paint::new(Color4f::new(0.0, 0.0, 0.0, 0.0), None);
    layer_paint.set_image_filter(image_filters::dilate((536_870_912.0, 0.0), None, None));
    layer_paint.set_blend_mode(BlendMode::Clear);

    canvas.save_layer(&SaveLayerRec::default().paint(&layer_paint));
    canvas.save_layer(&SaveLayerRec::default());

    let mut paint = Paint::default();
    paint.set_blend_mode(BlendMode::Darken);
    paint.set_shader(perlin_shaders::fractal_noise((0.0, 0.0), 2, 0.0, None));
    paint.set_color4f(Color4f::new(0.0, 0.0, 0.0, 0.0), None);

    canvas.draw_path(&Path::new(), &paint);
});

// Port of: tests/PDFPrimitivesTest.cpp#L582-L594 (chrome/m156)
fn pdf_contains(data: &[u8], needle: &str) -> bool {
    let needle = needle.as_bytes();
    if needle.is_empty() || data.len() < needle.len() {
        return false;
    }
    data.windows(needle.len()).any(|window| window == needle)
}

// Port of: tests/PDFPrimitivesTest.cpp#L596-L600 (chrome/m156)
fn make_linear_gradient(colors: &[Color4f], tile_mode: TileMode) -> Shader {
    let pts = [Point::new(0.0, 0.0), Point::new(64.0, 64.0)];
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(colors, None, tile_mode, None),
            Interpolation::default(),
        ),
        None,
    )
    .expect("a gradient")
}

// Port of: tests/PDFPrimitivesTest.cpp#L602-L618 (chrome/m156)
fn render_gradient_pdf(
    colors: &[Color4f],
    rasterize_for_print: bool,
    tile_mode: TileMode,
) -> Vec<u8> {
    let mut metadata: Metadata = jpeg::metadata_with_callbacks();
    metadata.rasterize_alpha_gradients_for_printing = rasterize_for_print;
    let mut stream = DynamicMemoryWStream::new();
    {
        let mut doc = new_document(&mut stream, Some(&metadata));
        let canvas = doc.begin_page(64.0, 64.0, None).expect("a canvas");
        let mut paint = Paint::default();
        paint.set_shader(make_linear_gradient(colors, tile_mode));
        canvas.draw_rect(Rect::from_wh(64.0, 64.0), &paint);
        doc.end_page();
        doc.close();
    }
    stream.detach_as_vector()
}

// Port of: tests/PDFPrimitivesTest.cpp#L620-L644 (chrome/m156)
// The print workaround should avoid the alpha-gradient vector encoding that
// triggers downstream PDF-to-PostScript failures.
def_test!(SkPDF_RasterizeAlphaGradientForPrinting, |reporter| {
    let with_alpha = [
        Color4f::new(1.0, 0.0, 0.0, 0.5),
        Color4f::new(0.0, 0.0, 1.0, 1.0),
    ];

    let off = render_gradient_pdf(&with_alpha, false, TileMode::Clamp);
    let on = render_gradient_pdf(&with_alpha, true, TileMode::Clamp);

    reporter_assert!(reporter, pdf_contains(&off, "/S /Luminosity"));

    reporter_assert!(reporter, !pdf_contains(&on, "/S /Luminosity"));
    reporter_assert!(reporter, pdf_contains(&on, "/Subtype /Image"));

    let off = render_gradient_pdf(&with_alpha, false, TileMode::Decal);
    let on = render_gradient_pdf(&with_alpha, true, TileMode::Decal);

    reporter_assert!(reporter, pdf_contains(&off, "/S /Luminosity"));
    reporter_assert!(reporter, !pdf_contains(&on, "/S /Luminosity"));
    reporter_assert!(reporter, pdf_contains(&on, "/Subtype /Image"));
});

// Port of: tests/PDFPrimitivesTest.cpp#L646-L668 (chrome/m156)
// The workaround must touch alpha gradients only. An opaque gradient has no
// soft mask to begin with, so the flag must not change its output at all.
def_test!(SkPDF_RasterizeAlphaGradientForPrinting_OpaqueUnchanged, |reporter| {
    let opaque = [
        Color4f::new(1.0, 0.0, 0.0, 1.0),
        Color4f::new(0.0, 0.0, 1.0, 1.0),
    ];

    let off = render_gradient_pdf(&opaque, false, TileMode::Clamp);
    let on = render_gradient_pdf(&opaque, true, TileMode::Clamp);

    reporter_assert!(reporter, !pdf_contains(&on, "/Subtype /Image"));
    reporter_assert!(reporter, pdf_contains(&on, "/Shading"));
    reporter_assert!(reporter, off == on);

    let off = render_gradient_pdf(&opaque, false, TileMode::Decal);
    let on = render_gradient_pdf(&opaque, true, TileMode::Decal);

    reporter_assert!(reporter, !pdf_contains(&on, "/Subtype /Image"));
    reporter_assert!(reporter, pdf_contains(&on, "/Shading"));
    reporter_assert!(reporter, off == on);
});

// Port of: tests/PDFPrimitivesTest.cpp#L670-L685 (chrome/m156)
def_test!(SkPDF_GradientDegenerateStopsDoesNotCrash, |_reporter| {
    let colors = [
        Color4f::new(0.0, 0.0, 0.0, 1.0),
        Color4f::new(0.0, 0.0, 0.0, 1.0),
    ];
    let pos = [1.0f32, 1.0];
    let shader = gradient_shaders::sweep_gradient(
        (32.0, 32.0),
        (0.0, 360.0),
        &Gradient::new(
            Colors::new(&colors, Some(&pos), TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    )
    .expect("a gradient");

    let mut stream = DynamicMemoryWStream::new();
    let mut doc = new_document(&mut stream, Some(&jpeg::metadata_with_callbacks()));
    let canvas = doc.begin_page(64.0, 64.0, None).expect("a canvas");
    let mut paint = Paint::default();
    paint.set_shader(shader);
    canvas.draw_rect(Rect::from_wh(64.0, 64.0), &paint);
    doc.end_page();
    doc.close();
});
