// Copyright 2010 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PDFPrimitivesTest.cpp (chrome/m156), the tests that need no PDF device.
// `SkPDF_Primitives` (its `test_issue1083` draws a glyph through a PDF canvas) and the tests that
// draw are left to the PDF device (modules.md M25).

use skia_rust_core::random::Random;
use skia_rust_core::stream::DynamicMemoryWStream;
use skia_rust_core::utils::parse_path;
use skia_rust_pdf::float_to_decimal::{MAXIMUM_SK_FLOAT_TO_DECIMAL_LENGTH, float_to_decimal};
use skia_rust_pdf::utils::{EmptyArea, EmptyPath, EmptyVerb, color_to_decimal, emit_path};

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
