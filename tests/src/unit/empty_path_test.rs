// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/EmptyPathTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::scalar::scalar;
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::{Reporter, def_test, errorf};

// Port of: tests/EmptyPathTest.cpp#L23 (chrome/m156)
const DIMENSION: i32 = 32;

// Port of: tests/EmptyPathTest.cpp#L25-L66 (chrome/m156)
fn draw_and_test(reporter: &mut Reporter, path: &Path, paint: &Paint, should_draw: bool) {
    let mut bm = Bitmap::default();
    bm.alloc_n32_pixels((DIMENSION, DIMENSION), None);
    // ensure no padding on each row
    assert_eq!(
        usize::try_from(DIMENSION * 4).expect("row bytes"),
        bm.row_bytes()
    );
    bm.erase_color(Color::TRANSPARENT);
    {
        let canvas = Canvas::from_bitmap(&mut bm, None).expect("canvas");
        let mut p = paint.clone();
        p.set_color(Color::WHITE);
        canvas.draw_path(path, &p);
    }

    let mut and_value: u32 = !0;
    let mut or_value: u32 = 0;
    // Row by row, as the C++ walks `getAddr32(0, 0)` across the bitmap (no row padding).
    for y in 0..DIMENSION {
        for x in 0..DIMENSION {
            let c = bm.get_addr32(x, y);
            and_value &= c;
            or_value |= c;
        }
    }

    // success means we drew everywhere or nowhere (depending on shouldDraw)
    let success = if should_draw {
        !0u32 == and_value
    } else {
        0 == or_value
    };
    if !success {
        let str = if should_draw {
            "Path expected to draw everywhere, but didn't. "
        } else {
            "Path expected to draw nowhere, but did. "
        };
        errorf!(
            reporter,
            "{} style[{:?}] cap[{:?}] join[{:?}] antialias[{}] filltype[{:?}] ptcount[{}]",
            str,
            paint.style(),
            paint.stroke_cap(),
            paint.stroke_join(),
            paint.is_anti_alias(),
            path.fill_type(),
            path.count_points()
        );
    }
}

// Port of: tests/EmptyPathTest.cpp#L68-L71 (chrome/m156)
#[derive(Clone, Copy, PartialEq, Eq)]
enum DrawCaps {
    DontDrawCaps,
    DrawCaps,
}

// Port of: tests/EmptyPathTest.cpp#L73-L112 (chrome/m156)
fn iter_paint(reporter: &mut Reporter, path: &Path, should_draw: bool, draw_caps: DrawCaps) {
    const G_CAPS: [Cap; 3] = [Cap::Butt, Cap::Round, Cap::Square];
    const G_JOINS: [Join; 3] = [Join::Miter, Join::Round, Join::Bevel];
    const G_STYLES: [Style; 3] = [Style::Fill, Style::Stroke, Style::StrokeAndFill];
    for &cap in &G_CAPS {
        for &join in &G_JOINS {
            for &style in &G_STYLES {
                if draw_caps == DrawCaps::DrawCaps && cap != Cap::Butt && style != Style::Fill {
                    continue;
                }
                let mut paint = Paint::default();
                paint.set_stroke_width(10.0);
                paint.set_stroke_cap(cap);
                paint.set_stroke_join(join);
                paint.set_style(style);
                paint.set_anti_alias(false);
                draw_and_test(reporter, path, &paint, should_draw);
                paint.set_anti_alias(true);
                draw_and_test(reporter, path, &paint, should_draw);
            }
        }
    }
}

// Port of: tests/EmptyPathTest.cpp#L114-L115 (chrome/m156)
// `CX` and `CY` are `SkIntToScalar(DIMENSION) / 2`.
const CX: scalar = 32.0 / 2.0;
const CY: scalar = 32.0 / 2.0;

// Port of: tests/EmptyPathTest.cpp#L117-L133 (chrome/m156)
fn make_empty(_bu: &mut PathBuilder) {}
fn make_m(bu: &mut PathBuilder) {
    bu.move_to((CX, CY));
}
fn make_mm(bu: &mut PathBuilder) {
    bu.move_to((CX, CY)).move_to((CX, CY));
}
fn make_mzm(bu: &mut PathBuilder) {
    bu.move_to((CX, CY)).close().move_to((CX, CY));
}
fn make_l(bu: &mut PathBuilder) {
    bu.move_to((CX, CY)).line_to((CX, CY));
}
fn make_q(bu: &mut PathBuilder) {
    bu.move_to((CX, CY)).quad_to((CX, CY), (CX, CY));
}
fn make_c(bu: &mut PathBuilder) {
    bu.move_to((CX, CY)).cubic_to((CX, CY), (CX, CY), (CX, CY));
}

// Port of: tests/EmptyPathTest.cpp#L135-L170 (chrome/m156)
// The C++ compares the procedure pointers to find which ones allow caps; the table carries that
// flag instead, with the same values.
fn test_emptydrawing(reporter: &mut Reporter) {
    const G_FILLS: [PathFillType; 4] = [
        PathFillType::Winding,
        PathFillType::EvenOdd,
        PathFillType::InverseWinding,
        PathFillType::InverseEvenOdd,
    ];
    type MakeProc = fn(&mut PathBuilder);
    // (procedure, whether zero-length segments allow caps)
    let g_make_proc: [(MakeProc, bool); 7] = [
        (make_empty, false),
        (make_m, false),
        (make_mm, false),
        (make_mzm, true),
        (make_l, true),
        (make_q, true),
        (make_c, true),
    ];

    for do_close in 0..2 {
        for &(make, allow_caps_base) in &g_make_proc {
            let mut builder = PathBuilder::new();
            make(&mut builder);
            if do_close != 0 {
                builder.close();
            }
            // zero length segments and close following moves draw round and square caps
            let allow_caps = allow_caps_base || do_close != 0;
            for &fill in &G_FILLS {
                builder.set_fill_type(fill);
                let should_draw = builder.is_inverse_fill_type();
                let draw_caps = if allow_caps {
                    DrawCaps::DrawCaps
                } else {
                    DrawCaps::DontDrawCaps
                };
                iter_paint(reporter, &builder.detach(), should_draw, draw_caps);
            }
        }
    }
}

// Port of: tests/EmptyPathTest.cpp#L172-L174 (chrome/m156)
def_test!(EmptyPath, |reporter| {
    test_emptydrawing(reporter);
});
