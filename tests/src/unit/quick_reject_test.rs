// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/QuickRejectTest.cpp (chrome/m156)
//
// Not ported: `QuickReject_MatrixState` needs `SkImageFilters::DistantLitDiffuse` (lighting image
// filters, Phase 3).

#![cfg(test)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{Canvas, SaveLayerRec};
use skia_rust_core::color::Color;
use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::SCALAR_NAN;
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::{Reporter, def_tier_test, reporter_assert};

// The pixel at (x, y) of the canvas's pixels (`*dst.getAddr32(x, y)`: the C++ canvas shares the
// bitmap's pixels, the Rust one holds them until it is dropped).
fn canvas_pixel(canvas: &Canvas, x: i32, y: i32) -> u32 {
    canvas.peek_pixels().expect("pixels").pixmap().addr32(x, y)
}

// Port of: tests/QuickRejectTest.cpp#L26-L50 (chrome/m156)
fn test_draw_bitmap(reporter: &mut Reporter) {
    let mut src = Bitmap::new();
    src.alloc_n32_pixels((10, 10), None);
    src.erase_color(Color::WHITE);

    let mut dst = Bitmap::new();
    dst.alloc_n32_pixels((10, 10), None);
    dst.erase_color(Color::TRANSPARENT);

    let canvas = Canvas::from_bitmap(&mut dst, None).expect("a canvas");

    // we are initially transparent
    reporter_assert!(reporter, 0 == canvas_pixel(&canvas, 5, 5));

    // we see the bitmap drawn
    canvas.draw_image(src.as_image().expect("an image"), (0.0, 0.0), None);
    reporter_assert!(reporter, 0xFFFF_FFFF == canvas_pixel(&canvas, 5, 5));

    // reverify we are clear again
    canvas.clear(Color::TRANSPARENT);
    reporter_assert!(reporter, 0 == canvas_pixel(&canvas, 5, 5));

    // if the bitmap is clipped out, we don't draw it
    canvas.draw_image(src.as_image().expect("an image"), (-10.0, 0.0), None);
    reporter_assert!(reporter, 0 == canvas_pixel(&canvas, 5, 5));
}

// Port of: tests/QuickRejectTest.cpp#L52-L65 (chrome/m156)
fn test_layers(reporter: &mut Reporter) {
    let canvas = Canvas::new_no_pixels((100, 100), None).expect("a canvas");

    let mut r = Rect::from_wh(10.0, 10.0);
    reporter_assert!(reporter, !canvas.quick_reject_rect(r));

    r.offset((300.0, 300.0));
    reporter_assert!(reporter, canvas.quick_reject_rect(r));

    // Test that saveLayer updates quickReject
    let bounds = Rect::new(50.0, 50.0, 70.0, 70.0);
    canvas.save_layer(&SaveLayerRec::default().bounds(&bounds));
    reporter_assert!(
        reporter,
        canvas.quick_reject_rect(Rect::from_wh(10.0, 10.0))
    );
    reporter_assert!(
        reporter,
        !canvas.quick_reject_rect(Rect::from_wh(60.0, 60.0))
    );
}

// Port of: tests/QuickRejectTest.cpp#L67-L100 (chrome/m156)
fn test_quick_reject(reporter: &mut Reporter) {
    let canvas = Canvas::new_no_pixels((100, 100), None).expect("a canvas");
    let r0 = Rect::new(-50.0, -50.0, 50.0, 50.0);
    let r1 = Rect::new(-50.0, 110.0, 50.0, 120.0);
    let r2 = Rect::new(110.0, -50.0, 120.0, 50.0);
    let r3 = Rect::new(-120.0, -50.0, 120.0, 50.0);
    let r4 = Rect::new(-50.0, -120.0, 50.0, 120.0);
    let r5 = Rect::new(-120.0, -120.0, 120.0, 120.0);
    let r6 = Rect::new(-120.0, -120.0, -110.0, -110.0);
    let r7 = Rect::new(SCALAR_NAN, -50.0, 50.0, 50.0);
    let r8 = Rect::new(-50.0, SCALAR_NAN, 50.0, 50.0);
    let r9 = Rect::new(-50.0, -50.0, SCALAR_NAN, 50.0);
    let r10 = Rect::new(-50.0, -50.0, 50.0, SCALAR_NAN);
    reporter_assert!(reporter, !canvas.quick_reject_rect(r0));
    reporter_assert!(reporter, canvas.quick_reject_rect(r1));
    reporter_assert!(reporter, canvas.quick_reject_rect(r2));
    reporter_assert!(reporter, !canvas.quick_reject_rect(r3));
    reporter_assert!(reporter, !canvas.quick_reject_rect(r4));
    reporter_assert!(reporter, !canvas.quick_reject_rect(r5));
    reporter_assert!(reporter, canvas.quick_reject_rect(r6));
    reporter_assert!(reporter, canvas.quick_reject_rect(r7));
    reporter_assert!(reporter, canvas.quick_reject_rect(r8));
    reporter_assert!(reporter, canvas.quick_reject_rect(r9));
    reporter_assert!(reporter, canvas.quick_reject_rect(r10));

    let mut m = Matrix::scale((2.0, 2.0));
    m.set_translate_x(10.0);
    m.set_translate_y(10.0);
    canvas.set_matrix(&M44::from(&m));
    let r11 = Rect::new(5.0, 5.0, 100.0, 100.0);
    let r12 = Rect::new(5.0, 50.0, 100.0, 100.0);
    let r13 = Rect::new(50.0, 5.0, 100.0, 100.0);
    reporter_assert!(reporter, !canvas.quick_reject_rect(r11));
    reporter_assert!(reporter, canvas.quick_reject_rect(r12));
    reporter_assert!(reporter, canvas.quick_reject_rect(r13));
}

// Port of: tests/QuickRejectTest.cpp#L102-L106 (chrome/m156)
def_tier_test!(QuickReject, |reporter| {
    test_draw_bitmap(reporter);
    test_layers(reporter);
    test_quick_reject(reporter);
});
