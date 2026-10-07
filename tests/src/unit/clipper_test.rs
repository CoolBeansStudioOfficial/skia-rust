// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ClipperTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::edge_clipper::EdgeClipper;
use skia_rust_core::float_bits::float_to_bits;
use skia_rust_core::line_clipper::intersect_line;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_raster::raster_canvas::RasterCanvas;

// Port of: tests/ClipperTest.cpp#L24-L58 (chrome/m156)
fn test_hairclipping(reporter: &mut Reporter) {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((4, 4), None);
    bm.erase_color(Color::WHITE);

    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    {
        // The canvas owns the bitmap while it lives (docs/design/pixels.md), so it is scoped
        // before the pixels are read back.
        let canvas = Canvas::from_bitmap(&mut bm, None).expect("canvas");
        canvas.clip_rect(Rect::from_wh(4.0, 2.0), None, None);
        canvas.draw_line((1.5, 1.5), (3.5, 3.5), &paint);
    }

    /*
     *  We had a bug where we misinterpreted the bottom of the clip, and
     *  would draw another pixel (to the right in this case) on the same
     *  last scanline. i.e. we would draw to [2,1], even though this hairline
     *  should just draw to [1,1], [2,2], [3,3] modulo the clip.
     *
     *  The result of this entire draw should be that we only draw to [1,1]
     *
     *  Fixed in rev. 3366
     */
    for y in 0..4 {
        for x in 0..4 {
            let non_white = (1 == y) && (1 == x);
            let c = bm.get_addr32(x, y);
            if non_white {
                reporter_assert!(reporter, 0xFFFF_FFFF != c);
            } else {
                reporter_assert!(reporter, 0xFFFF_FFFF == c);
            }
        }
    }
}

// Port of: tests/ClipperTest.cpp#L60-L76 (chrome/m156)
fn test_edgeclipper() {
    let mut clipper = EdgeClipper::new(false);

    let pts = [
        Point::new(3.099_547_6e+010, 42.929_78),
        Point::new(-3.099_516_3e+010, 51.050_385),
        Point::new(-3.099_515_7e+010, 51.050_392),
        Point::new(-3.099_513_4e+010, 51.050_4),
    ];

    let clip = Rect::new(0.0, 0.0, 300.0, 200.0);

    // this should not assert, even though our choppers do a poor numerical
    // job when computing their t values.
    // http://code.google.com/p/skia/issues/detail?id=444
    let _ = clipper.clip_cubic(&pts, &clip);
}

// memcmp of two SkPoints.
fn same_bytes(a: &[Point], b: &[Point]) -> bool {
    a.iter().zip(b).all(|(a, b)| {
        float_to_bits(a.x) == float_to_bits(b.x) && float_to_bits(a.y) == float_to_bits(b.y)
    })
}

// Port of: tests/ClipperTest.cpp#L78-L157 (chrome/m156)
#[allow(clippy::too_many_lines, clippy::manual_midpoint)] // mirrors the long C++ test and its `(L + R) / 2.f`
fn test_intersectline(reporter: &mut Reporter) {
    const L: scalar = 0.0;
    const T: scalar = 0.0;
    const R: scalar = 100.0;
    const B: scalar = 100.0;
    const CX: scalar = (L + R) / 2.0;
    const CY: scalar = (T + B) / 2.0;
    let g_r = Rect::new(L, T, R, B);

    let p = Point::new;

    let g_empty = [
        // sides
        p(L, CY),
        p(L - 10.0, CY),
        p(R, CY),
        p(R + 10.0, CY),
        p(CX, T),
        p(CX, T - 10.0),
        p(CX, B),
        p(CX, B + 10.0),
        // corners
        p(L, T),
        p(L - 10.0, T - 10.0),
        p(L, B),
        p(L - 10.0, B + 10.0),
        p(R, T),
        p(R + 10.0, T - 10.0),
        p(R, B),
        p(R + 10.0, B + 10.0),
    ];
    for i in (0..g_empty.len()).step_by(2) {
        let dst = intersect_line(&[g_empty[i], g_empty[i + 1]], &g_r);
        let valid = dst.is_some();
        if let Some(dst) = dst {
            eprintln!(
                "----- [{}] {} {} -> {} {}",
                i / 2,
                dst[0].x,
                dst[0].y,
                dst[1].x,
                dst[1].y
            );
        }
        reporter_assert!(reporter, !valid);
    }

    let g_full = [
        // diagonals, chords
        p(L, T),
        p(R, B),
        p(L, B),
        p(R, T),
        p(CX, T),
        p(CX, B),
        p(L, CY),
        p(R, CY),
        p(CX, T),
        p(R, CY),
        p(CX, T),
        p(L, CY),
        p(L, CY),
        p(CX, B),
        p(R, CY),
        p(CX, B),
        // edges
        p(L, T),
        p(L, B),
        p(R, T),
        p(R, B),
        p(L, T),
        p(R, T),
        p(L, B),
        p(R, B),
    ];
    for i in (0..g_full.len()).step_by(2) {
        let dst = intersect_line(&[g_full[i], g_full[i + 1]], &g_r);
        let ok = dst.is_some_and(|dst| same_bytes(&g_full[i..i + 2], &dst));
        if !ok {
            let dst = dst.unwrap_or_default();
            eprintln!(
                "++++ [{}] {} {} -> {} {}",
                i / 2,
                dst[0].x,
                dst[0].y,
                dst[1].x,
                dst[1].y
            );
        }
        reporter_assert!(reporter, ok);
    }

    let g_partial = [
        p(L - 10.0, CY),
        p(CX, CY),
        p(L, CY),
        p(CX, CY),
        p(CX, T - 10.0),
        p(CX, CY),
        p(CX, T),
        p(CX, CY),
        p(R + 10.0, CY),
        p(CX, CY),
        p(R, CY),
        p(CX, CY),
        p(CX, B + 10.0),
        p(CX, CY),
        p(CX, B),
        p(CX, CY),
        // extended edges
        p(L, T - 10.0),
        p(L, B + 10.0),
        p(L, T),
        p(L, B),
        p(R, T - 10.0),
        p(R, B + 10.0),
        p(R, T),
        p(R, B),
        p(L - 10.0, T),
        p(R + 10.0, T),
        p(L, T),
        p(R, T),
        p(L - 10.0, B),
        p(R + 10.0, B),
        p(L, B),
        p(R, B),
    ];
    for i in (0..g_partial.len()).step_by(4) {
        let dst = intersect_line(&[g_partial[i], g_partial[i + 1]], &g_r);
        let ok = dst.is_some_and(|dst| same_bytes(&g_partial[i + 2..i + 4], &dst));
        if !ok {
            let dst = dst.unwrap_or_default();
            eprintln!(
                "++++ [{}] {} {} -> {} {}",
                i / 2,
                dst[0].x,
                dst[0].y,
                dst[1].x,
                dst[1].y
            );
        }
        reporter_assert!(reporter, ok);
    }
}

// Port of: tests/ClipperTest.cpp#L159-L163 (chrome/m156)
def_test!(Clipper, |reporter| {
    test_intersectline(reporter);
    test_edgeclipper();
    test_hairclipping(reporter);
});

// Port of: tests/ClipperTest.cpp#L165-L171 (chrome/m156)
def_test!(LineClipper_skbug_7981, |_r| {
    let src = [
        Point::new(-5.776_988e+17, -1.817_580_6e+23),
        Point::new(38127.0, 2.0),
    ];
    let clip = Rect::new(-32767.0, -32767.0, 32767.0, 32767.0);

    let _ = intersect_line(&src, &clip);
});
