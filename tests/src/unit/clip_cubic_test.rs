// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ClipCubicTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{def_test, reporter_assert};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::cubic_clipper::CubicClipper;
use skia_rust_core::float_bits::bits_to_float;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::scalar::scalar_abs;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;

// Currently the supersampler blitter uses int16_t for its index into an array
// the width of the clip. Test that we don't crash/assert if we try to draw
// with a device/clip that is larger.
// Port of: tests/ClipCubicTest.cpp#L26-L38 (chrome/m156)
fn test_giant_clip() {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((64919, 1), None);
    let canvas = Canvas::from_bitmap(&mut bm, None).expect("canvas");
    canvas.clear(Color::TRANSPARENT);

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    canvas.draw_path(
        &Path::polygon(
            &[
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(33.0, 1.0),
            ],
            false,
            None,
            None,
        ),
        &paint,
    );
}

// Port of: tests/ClipCubicTest.cpp#L40-L48 (chrome/m156)
fn print_curve(name: &str, crv: &[Point; 4]) {
    eprintln!(
        "{}: {:.10}, {:.10}, {:.10}, {:.10}, {:.10}, {:.10}, {:.10}, {:.10}",
        name, crv[0].x, crv[0].y, crv[1].x, crv[1].y, crv[2].x, crv[2].y, crv[3].x, crv[3].y
    );
}

// Port of: tests/ClipCubicTest.cpp#L51-L64 (chrome/m156)
fn curves_are_equal(c0: &[Point; 4], c1: &[Point; 4], tol: f32) -> bool {
    for i in 0..4 {
        if scalar_abs(c0[i].x - c1[i].x) > tol || scalar_abs(c0[i].y - c1[i].y) > tol {
            print_curve("c0", c0);
            print_curve("c1", c1);
            return false;
        }
    }
    true
}

// Port of: tests/ClipCubicTest.cpp#L67-L77 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn set_curve(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    x3: f32,
    y3: f32,
    crv: &mut [Point; 4],
) -> [Point; 4] {
    crv[0].x = x0;
    crv[0].y = y0;
    crv[1].x = x1;
    crv[1].y = y1;
    crv[2].x = x2;
    crv[2].y = y2;
    crv[3].x = x3;
    crv[3].y = y3;
    *crv
}

// Port of: tests/ClipCubicTest.cpp#L80-L173 (chrome/m156)
def_test!(
    #[allow(
        clippy::excessive_precision,
        clippy::similar_names,
        clippy::too_many_lines
    )] // literals are copied from the C++; mirrors the long C++ test
    ClipCubic,
    |reporter| {
        let crv = [
            Point::new(0.0, 0.0),
            Point::new(2.0, 3.0),
            Point::new(1.0, 10.0),
            Point::new(4.0, 12.0),
        ];

        let mut clipper = CubicClipper::new();
        let mut clipped = [Point::default(); 4];
        let mut shouldbe = [Point::default(); 4];
        let mut clip_rect = IRect::default();
        let tol = 1e-4_f32;

        // Test no clip, with plenty of room.
        clip_rect.set_ltrb(-2, -2, 6, 14);
        clipper.set_clip(&clip_rect);
        let mut success = clipper.clip_cubic(&crv, &mut clipped);
        reporter_assert!(reporter, success);
        let sb = set_curve(0.0, 0.0, 2.0, 3.0, 1.0, 10.0, 4.0, 12.0, &mut shouldbe);
        reporter_assert!(reporter, curves_are_equal(&clipped, &sb, tol));

        // Test no clip, touching first point.
        clip_rect.set_ltrb(-2, 0, 6, 14);
        clipper.set_clip(&clip_rect);
        success = clipper.clip_cubic(&crv, &mut clipped);
        reporter_assert!(reporter, success);
        let sb = set_curve(0.0, 0.0, 2.0, 3.0, 1.0, 10.0, 4.0, 12.0, &mut shouldbe);
        reporter_assert!(reporter, curves_are_equal(&clipped, &sb, tol));

        // Test no clip, touching last point.
        clip_rect.set_ltrb(-2, -2, 6, 12);
        clipper.set_clip(&clip_rect);
        success = clipper.clip_cubic(&crv, &mut clipped);
        reporter_assert!(reporter, success);
        let sb = set_curve(0.0, 0.0, 2.0, 3.0, 1.0, 10.0, 4.0, 12.0, &mut shouldbe);
        reporter_assert!(reporter, curves_are_equal(&clipped, &sb, tol));

        // Test all clip.
        clip_rect.set_ltrb(-2, 14, 6, 20);
        clipper.set_clip(&clip_rect);
        success = clipper.clip_cubic(&crv, &mut clipped);
        reporter_assert!(reporter, !success);

        // Test clip at 1.
        clip_rect.set_ltrb(-2, 1, 6, 14);
        clipper.set_clip(&clip_rect);
        success = clipper.clip_cubic(&crv, &mut clipped);
        reporter_assert!(reporter, success);
        let sb = set_curve(
            0.512_612_521_6,
            1.0,
            1.841_195_941,
            4.337_081_432,
            1.297_019_958,
            10.198_013_31,
            4.0,
            12.0,
            &mut shouldbe,
        );
        reporter_assert!(reporter, curves_are_equal(&clipped, &sb, tol));

        // Test clip at 2.
        clip_rect.set_ltrb(-2, 2, 6, 14);
        clipper.set_clip(&clip_rect);
        success = clipper.clip_cubic(&crv, &mut clipped);
        reporter_assert!(reporter, success);
        let sb = set_curve(
            0.841_235_220_4,
            2.0,
            1.767_683_744,
            5.400_758_266,
            1.550_529_48,
            10.367_019_65,
            4.0,
            12.0,
            &mut shouldbe,
        );
        reporter_assert!(reporter, curves_are_equal(&clipped, &sb, tol));

        // Test clip at 11.
        clip_rect.set_ltrb(-2, -2, 6, 11);
        clipper.set_clip(&clip_rect);
        success = clipper.clip_cubic(&crv, &mut clipped);
        reporter_assert!(reporter, success);
        let sb = set_curve(
            0.0,
            0.0,
            1.742_904_663,
            2.614_356_995,
            1.207_521_796,
            8.266_430_855,
            3.026_495_695,
            11.0,
            &mut shouldbe,
        );
        reporter_assert!(reporter, curves_are_equal(&clipped, &sb, tol));

        // Test clip at 10.
        clip_rect.set_ltrb(-2, -2, 6, 10);
        clipper.set_clip(&clip_rect);
        success = clipper.clip_cubic(&crv, &mut clipped);
        reporter_assert!(reporter, success);
        let sb = set_curve(
            0.0,
            0.0,
            1.551_193_237,
            2.326_789_856,
            1.297_736_168,
            7.059_780_121,
            2.505_550_385,
            10.0,
            &mut shouldbe,
        );
        reporter_assert!(reporter, curves_are_equal(&clipped, &sb, tol));

        test_giant_clip();
    }
);

// Port of: tests/ClipCubicTest.cpp#L175-L211 (chrome/m156)
def_test!(test_fuzz_crbug_698714, |_reporter| {
    let mut surface = surfaces::raster_n32_premul((500, 500)).expect("surface");
    let canvas = surface.canvas();
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let mut builder = PathBuilder::new();
    builder.move_to((bits_to_float(0x0000_0000), bits_to_float(0x0000_0000))); // 0,0
    builder.line_to((bits_to_float(0x4343_4343), bits_to_float(0x4343_0143))); //195.263f, 195.005f
    builder.line_to((bits_to_float(0x4343_4343), bits_to_float(0x4343_4343))); //195.263f, 195.263f
    builder.line_to((bits_to_float(0xb543_4343), bits_to_float(0x4343_00be))); //-7.2741e-07f, 195.003f
    // 195.263f, 195.263f, -1.16387e-05f, 3.58641e-38f, 3.85088e-29f,1.86082e-39f
    builder.cubic_to(
        (bits_to_float(0x4343_4343), bits_to_float(0x4343_4341)),
        (bits_to_float(0xb743_43bd), bits_to_float(0x0143_4343)),
        (bits_to_float(0x1043_4343), bits_to_float(0x0014_4332)),
    );
    // 4.11823e-38f, 195.263f, 195.263f, 195.263f, -7.2741e-07f, 195.263f
    builder.cubic_to(
        (bits_to_float(0x0160_37c0), bits_to_float(0x4343_4343)),
        (bits_to_float(0x4343_4343), bits_to_float(0x4343_4343)),
        (bits_to_float(0xb543_4343), bits_to_float(0x4343_4343)),
    );
    // 195.263f, 195.263f, -1.16387e-05f, 3.58641e-38f, 195.263f, -2
    builder.cubic_to(
        (bits_to_float(0x4343_4344), bits_to_float(0x4343_4341)),
        (bits_to_float(0xb743_43bd), bits_to_float(0x0143_4343)),
        (bits_to_float(0x4343_4343), bits_to_float(0xc000_0014)),
    );
    // -5.87228e+06f, 3.7773e-07f, 3.60231e-13f, -6.64511e+06f,2.77692e-15f, 2.48803e-15f
    builder.cubic_to(
        (bits_to_float(0xcab3_3535), bits_to_float(0x34ca_caca)),
        (bits_to_float(0x2aca_caca), bits_to_float(0xcaca_cae3)),
        (bits_to_float(0x2748_1927), bits_to_float(0x2733_4805)),
    );
    builder.line_to((bits_to_float(0xb543_4343), bits_to_float(0x4343_4343))); //-7.2741e-07f, 195.263f
    // 195.263f, 195.263f, -1.16387e-05f, 195.212f, 195.263f, -2
    builder.cubic_to(
        (bits_to_float(0x4343_4343), bits_to_float(0x4343_4341)),
        (bits_to_float(0xb743_43b9), bits_to_float(0x4343_3643)),
        (bits_to_float(0x4343_4343), bits_to_float(0xc000_0014)),
    );
    builder.line_to((bits_to_float(0xc700_4343), bits_to_float(0x2748_0527))); //-32835.3f, 2.77584e-15f
    builder.line_to((bits_to_float(0x0000_0000), bits_to_float(0x0000_0000))); // 0,0
    builder.close();
    canvas.clip_rect(Rect::from_ltrb(0.0, 0.0, 65.0, 202.0), None, None);
    canvas.draw_path(&builder.detach(), &paint);
});

// Port of: tests/ClipCubicTest.cpp#L213-L231 (chrome/m156)
def_test!(cubic_scan_error_crbug_844457_and_845489, |_reporter| {
    let mut surface = surfaces::raster_n32_premul((100, 100)).expect("surface");
    let canvas = surface.canvas();
    let paint = Paint::default();

    let mut builder = PathBuilder::new();
    builder.move_to((-30.0 / 64.0, -31.0 / 64.0));
    builder.cubic_to(
        (-31.0 / 64.0, -31.0 / 64.0),
        (-31.0 / 64.0, -31.0 / 64.0),
        (-31.0 / 64.0, 100.0),
    );
    builder.line_to((100.0, 100.0));
    canvas.draw_path(&builder.detach(), &paint);

    // May need to define SK_RASTERIZE_EVEN_ROUNDING to trigger the need for this test
    builder.move_to((-30.0 / 64.0_f32, -31.0 / 64.0_f32 + 1.0 / 256.0_f32));
    builder.cubic_to(
        (
            -31.0 / 64.0_f32 + 1.0 / 256.0_f32,
            -31.0 / 64.0_f32 + 1.0 / 256.0_f32,
        ),
        (
            -31.0 / 64.0_f32 + 1.0 / 256.0_f32,
            -31.0 / 64.0_f32 + 1.0 / 256.0_f32,
        ),
        (-31.0 / 64.0_f32 + 1.0 / 256.0_f32, 100.0),
    );
    builder.line_to((100.0, 100.0));
    canvas.draw_path(&builder.detach(), &paint);
});
