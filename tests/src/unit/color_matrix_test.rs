// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ColorMatrixTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::paint::Paint;
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/ColorMatrixTest.cpp#L18-L24 (chrome/m156)
fn assert_color_tolerance(reporter: &mut Reporter, expected: Color, actual: Color, tolerance: i32) {
    // The C++ subtracts unsigned channels and casts the wrapped result to int; the signed
    // difference is the same.
    let diff = |e: u8, a: u8| (i32::from(e) - i32::from(a)).abs();
    reporter_assert!(reporter, diff(expected.a(), actual.a()) <= tolerance);
    reporter_assert!(reporter, diff(expected.r(), actual.r()) <= tolerance);
    reporter_assert!(reporter, diff(expected.g(), actual.g()) <= tolerance);
    reporter_assert!(reporter, diff(expected.b(), actual.b()) <= tolerance);
}

// Port of: tests/ColorMatrixTest.cpp#L26-L29 (chrome/m156)
fn assert_color(reporter: &mut Reporter, expected: Color, actual: Color) {
    const TOLERANCE: i32 = 1;
    assert_color_tolerance(reporter, expected, actual, TOLERANCE);
}

/// Draws one point at the origin of `bm` with `paint`. The canvas owns the bitmap while it lives
/// (docs/design/pixels.md), so it is scoped to the draw; the C++ keeps one canvas and reads the
/// bitmap between draws, which is the same sequence.
fn draw_point(bm: &mut Bitmap, paint: &Paint) {
    let canvas = Canvas::from_bitmap(bm, None).expect("canvas");
    canvas.draw_point((0.0, 0.0), paint);
}

// Port of: tests/ColorMatrixTest.cpp#L35-L97 (chrome/m156)
// This test case is a mirror of the Android CTS tests for MatrixColorFilter
// found in the android.graphics.ColorMatrixColorFilterTest class.
fn test_color_matrix_cts(reporter: &mut Reporter) {
    let mut bitmap = Bitmap::new();
    bitmap.alloc_n32_pixels((1, 1), None);
    let mut paint = Paint::default();

    #[rustfmt::skip]
    let blue_to_cyan: [f32; 20] = [
        1.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 1.0, 0.0,
    ];
    paint.set_color_filter(color_filters::matrix_row_major(&blue_to_cyan, Clamp::Yes));
    paint.set_color(Color::BLUE);
    draw_point(&mut bitmap, &paint);
    assert_color(reporter, Color::CYAN, bitmap.get_color((0, 0)));

    paint.set_color(Color::GREEN);
    draw_point(&mut bitmap, &paint);
    assert_color(reporter, Color::GREEN, bitmap.get_color((0, 0)));

    paint.set_color(Color::RED);
    draw_point(&mut bitmap, &paint);
    assert_color(reporter, Color::RED, bitmap.get_color((0, 0)));

    // color components are clipped, not scaled
    paint.set_color(Color::MAGENTA);
    draw_point(&mut bitmap, &paint);
    assert_color(reporter, Color::WHITE, bitmap.get_color((0, 0)));

    #[rustfmt::skip]
    let mut transparent_red_add_blue: [f32; 20] = [
        1.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0, 64.0 / 255.0,
        -0.5, 0.0, 0.0, 1.0, 0.0,
    ];
    paint.set_color_filter(color_filters::matrix_row_major(
        &transparent_red_add_blue,
        Clamp::Yes,
    ));
    bitmap.erase_color(Color::TRANSPARENT);
    paint.set_color(Color::RED);
    draw_point(&mut bitmap, &paint);
    assert_color_tolerance(
        reporter,
        Color::from_argb(128, 255, 0, 64),
        bitmap.get_color((0, 0)),
        2,
    );

    paint.set_color(Color::CYAN);
    draw_point(&mut bitmap, &paint);
    // blue gets clipped
    assert_color(reporter, Color::CYAN, bitmap.get_color((0, 0)));

    // change array to filter out green
    #[allow(clippy::float_cmp)] // the C++ asserts this exact value
    let is_one = 1.0_f32 == transparent_red_add_blue[6];
    reporter_assert!(reporter, is_one);
    transparent_red_add_blue[6] = 0.0;

    // check that changing the array has no effect
    draw_point(&mut bitmap, &paint);
    assert_color(reporter, Color::CYAN, bitmap.get_color((0, 0)));

    // create a new filter with the changed matrix
    paint.set_color_filter(color_filters::matrix_row_major(
        &transparent_red_add_blue,
        Clamp::Yes,
    ));
    draw_point(&mut bitmap, &paint);
    assert_color(reporter, Color::BLUE, bitmap.get_color((0, 0)));
}

def_test!(ColorMatrix, |reporter| {
    test_color_matrix_cts(reporter);
});

// Port of: tests/ColorMatrixTest.cpp#L104-L122 (chrome/m156)
def_test!(ColorMatrix_clamp_while_unpremul, |r| {
    // This matrix does green += 255/255 and alpha += 32/255.  We want to test
    // that if we pass it opaque alpha and small red and blue values, red and
    // blue stay unchanged, not pumped up by that ~1.12 intermediate alpha.
    #[rustfmt::skip]
    let m: [f32; 20] = [
        1.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0, 1.0,
        0.0, 0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 1.0, 32.0 / 255.0,
    ];
    let filter = color_filters::matrix_row_major(&m, Clamp::Yes).expect("finite matrix");
    let src_color = Color4f::from_color(Color::new(0xff0a_0b0c));
    let filtered = filter.filter_color4f(src_color, None, None).to_color();
    reporter_assert!(r, filtered.a() == 0xff);
    reporter_assert!(r, filtered.r() == 0x0a);
    reporter_assert!(r, filtered.g() == 0xff);
    reporter_assert!(r, filtered.b() == 0x0c);
});
