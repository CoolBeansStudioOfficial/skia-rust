// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CappedHairlinesTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::color::Color;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::IPoint;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

// Port of: tests/CappedHairlinesTest.cpp#L19-L26 (chrome/m156)
fn create_white_raster_surface() -> Option<Surface<'static>> {
    let info = ImageInfo::new_n32_premul((70, 70), None);
    let mut surface = surfaces::raster(&info, None, None);
    if let Some(surface) = &mut surface {
        surface.canvas().clear(Color::WHITE);
    }
    surface
}

// Port of: tests/CappedHairlinesTest.cpp#L28-L35 (chrome/m156)
fn create_aa_hairline_paint() -> Paint {
    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(0.0);
    paint.set_color(Color::BLACK);
    paint.set_anti_alias(true);
    paint
}

// Port of: tests/CappedHairlinesTest.cpp#L37-L40 (chrome/m156)
fn color_is_lteq(a: Color, b: Color) -> bool {
    a.r() <= b.r() && a.g() <= b.g() && a.b() <= b.b() && a.a() <= b.a()
}

// Port of: tests/CappedHairlinesTest.cpp#L42-L65 (chrome/m156)
fn assert_color_in_range(
    reporter: &mut Reporter,
    pixmap: &Pixmap<'_>,
    test_name: &str,
    pt: IPoint,
    expected_min: Color,
    expected_max: Color,
) {
    // skiatest::ReporterContext subtest(reporter, testName);
    reporter.set_context(Some(test_name.to_string()));

    let actual = pixmap.get_color(pt);

    reporter_assert!(
        reporter,
        color_is_lteq(expected_min, actual) && color_is_lteq(actual, expected_max),
        "{}: actual color ({:x}) is outside the range of expected colors (between ({:x}) \
         and ({:x})) for pixel at ({}, {})",
        test_name,
        u32::from(actual),
        u32::from(expected_min),
        u32::from(expected_max),
        pt.x,
        pt.y
    );

    reporter.set_context(None);
}

// Port of: tests/CappedHairlinesTest.cpp#L67-L119 (chrome/m156)
fn draw_aa_hairline_path_with_caps_no_offset(reporter: &mut Reporter, cap: Cap) {
    let mut surface = create_white_raster_surface().expect("surface");

    let mut paint = create_aa_hairline_paint();
    paint.set_stroke_cap(cap);

    let path = PathBuilder::new()
        .move_to((5.0, 5.0))
        .line_to((5.0, 10.0))
        .line_to((10.0, 10.0))
        .line_to((10.0, 5.0))
        .close()
        .detach();

    surface.canvas().draw_path(&path, &paint);

    let peek = surface.peek_pixels().expect("peekPixels");
    let pixmap = peek.pixmap();

    let assert_color = |reporter: &mut Reporter,
                        name: &str,
                        pt: IPoint,
                        expected_min: Color,
                        expected_max: Color| {
        assert_color_in_range(reporter, &pixmap, name, pt, expected_min, expected_max);
    };

    let assert_exact_color =
        |reporter: &mut Reporter, name: &str, pt: IPoint, exact_color: Color| {
            assert_color_in_range(reporter, &pixmap, name, pt, exact_color, exact_color);
        };

    // these asserts are to verify that the contour drawn is a square
    // with double pixel wide gray lines. The inside and outside of the square
    // should be white. This should be the expected behavior across all caps

    assert_exact_color(
        reporter,
        "left of west line",
        IPoint::new(3, 7),
        Color::WHITE,
    );
    assert_color(
        reporter,
        "on west line",
        IPoint::new(4, 7),
        Color::BLACK,
        Color::GRAY,
    );
    assert_color(
        reporter,
        "on west line",
        IPoint::new(5, 7),
        Color::BLACK,
        Color::GRAY,
    );
    assert_exact_color(
        reporter,
        "right of west line",
        IPoint::new(6, 7),
        Color::WHITE,
    );

    assert_exact_color(
        reporter,
        "above south line",
        IPoint::new(7, 8),
        Color::WHITE,
    );
    assert_color(
        reporter,
        "on south line",
        IPoint::new(7, 9),
        Color::BLACK,
        Color::GRAY,
    );
    assert_color(
        reporter,
        "on south line",
        IPoint::new(7, 10),
        Color::BLACK,
        Color::GRAY,
    );
    assert_exact_color(
        reporter,
        "below south line",
        IPoint::new(7, 11),
        Color::WHITE,
    );

    assert_exact_color(
        reporter,
        "left of east line",
        IPoint::new(8, 7),
        Color::WHITE,
    );
    assert_color(
        reporter,
        "on east line",
        IPoint::new(9, 7),
        Color::BLACK,
        Color::GRAY,
    );
    assert_color(
        reporter,
        "on east line",
        IPoint::new(10, 7),
        Color::BLACK,
        Color::GRAY,
    );
    assert_exact_color(
        reporter,
        "right of east line",
        IPoint::new(11, 7),
        Color::WHITE,
    );

    assert_exact_color(
        reporter,
        "above north line",
        IPoint::new(7, 3),
        Color::WHITE,
    );
    assert_color(
        reporter,
        "on north line",
        IPoint::new(7, 4),
        Color::BLACK,
        Color::GRAY,
    );
    assert_color(
        reporter,
        "on north line",
        IPoint::new(7, 5),
        Color::BLACK,
        Color::GRAY,
    );
    assert_exact_color(
        reporter,
        "below north line",
        IPoint::new(7, 6),
        Color::WHITE,
    );
}

// Port of: tests/CappedHairlinesTest.cpp#L121-L124 (chrome/m156)
def_test!(
    DrawAntialiasHairline_SquareCappedNoPixelOffsetClosedContour_DoublePixelWideOutput,
    |reporter| {
        draw_aa_hairline_path_with_caps_no_offset(reporter, Cap::Square);
    }
);

// Port of: tests/CappedHairlinesTest.cpp#L126-L129 (chrome/m156)
def_test!(
    DrawAntialiasHairline_RoundCappedNoPixelOffsetClosedContour_DoublePixelWideOutput,
    |reporter| {
        draw_aa_hairline_path_with_caps_no_offset(reporter, Cap::Round);
    }
);

// Port of: tests/CappedHairlinesTest.cpp#L131-L134 (chrome/m156)
def_test!(
    DrawAntialiasHairline_ButtCappedNoPixelOffsetClosedContour_DoublePixelWideOutput,
    |reporter| {
        draw_aa_hairline_path_with_caps_no_offset(reporter, Cap::Butt);
    }
);

// Port of: tests/CappedHairlinesTest.cpp#L136-L183 (chrome/m156)
fn draw_aa_hairline_path_with_caps_and_offset(reporter: &mut Reporter, cap: Cap) {
    let mut surface = create_white_raster_surface().expect("surface");

    let mut paint = create_aa_hairline_paint();
    paint.set_stroke_cap(cap);

    let path = PathBuilder::new()
        .move_to((5.0, 5.0))
        .line_to((5.0, 10.0))
        .line_to((10.0, 10.0))
        .line_to((10.0, 5.0))
        .close()
        .detach()
        .make_offset((0.5, 0.5));

    surface.canvas().draw_path(&path, &paint);

    let peek = surface.peek_pixels().expect("peekPixels");
    let pixmap = peek.pixmap();

    let assert_exact_color =
        |reporter: &mut Reporter, name: &str, pt: IPoint, exact_color: Color| {
            assert_color_in_range(reporter, &pixmap, name, pt, exact_color, exact_color);
        };

    // these asserts are to verify that the contour drawn is a square
    // with single pixel wide black lines. The inside and outside of the square
    // should be white. This should be the expected behavior across all caps

    assert_exact_color(
        reporter,
        "left of west line",
        IPoint::new(4, 7),
        Color::WHITE,
    );
    assert_exact_color(reporter, "on west line", IPoint::new(5, 7), Color::BLACK);
    assert_exact_color(
        reporter,
        "right of west line",
        IPoint::new(6, 7),
        Color::WHITE,
    );

    assert_exact_color(
        reporter,
        "above south line",
        IPoint::new(7, 9),
        Color::WHITE,
    );
    assert_exact_color(reporter, "on south line", IPoint::new(7, 10), Color::BLACK);
    assert_exact_color(
        reporter,
        "below south line",
        IPoint::new(7, 11),
        Color::WHITE,
    );

    assert_exact_color(
        reporter,
        "left of east line",
        IPoint::new(9, 7),
        Color::WHITE,
    );
    assert_exact_color(reporter, "on east line", IPoint::new(10, 7), Color::BLACK);
    assert_exact_color(
        reporter,
        "right of east line",
        IPoint::new(11, 7),
        Color::WHITE,
    );

    assert_exact_color(
        reporter,
        "above north line",
        IPoint::new(7, 4),
        Color::WHITE,
    );
    assert_exact_color(reporter, "on north line", IPoint::new(7, 5), Color::BLACK);
    assert_exact_color(
        reporter,
        "below north line",
        IPoint::new(7, 6),
        Color::WHITE,
    );
}

// Port of: tests/CappedHairlinesTest.cpp#L185-L188 (chrome/m156)
def_test!(
    DrawAntialiasHairline_SquareCappedHalfPixelOffsetClosedContour_SinglePixelWideOutput,
    |reporter| {
        draw_aa_hairline_path_with_caps_and_offset(reporter, Cap::Square);
    }
);

// Port of: tests/CappedHairlinesTest.cpp#L190-L193 (chrome/m156)
def_test!(
    DrawAntialiasHairline_RoundCappedHalfPixelOffsetClosedContour_SinglePixelWideOutput,
    |reporter| {
        draw_aa_hairline_path_with_caps_and_offset(reporter, Cap::Round);
    }
);

// Port of: tests/CappedHairlinesTest.cpp#L195-L198 (chrome/m156)
def_test!(
    DrawAntialiasHairline_ButtCappedHalfPixelOffsetClosedContour_SinglePixelWideOutput,
    |reporter| {
        draw_aa_hairline_path_with_caps_and_offset(reporter, Cap::Butt);
    }
);
