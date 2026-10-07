// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/DrawPathTest.cpp (chrome/m156)

#![cfg(test)]
#![allow(clippy::excessive_precision)] // literals are copied verbatim from the C++

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_effect::PathEffect;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::path_utils::fill_path_with_paint_to_path;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::scalar::{scalar, scalar_round_to_int};
use skia_rust_core::stroke_rec::StrokeRec;
use skia_rust_effects::dash_path_effect::DashPathEffectExt;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;

// test that we can draw an aa-rect at coordinates > 32K (bigger than fixedpoint)
// Port of: tests/DrawPathTest.cpp#L33-L65 (chrome/m156)
fn test_big_aa_rect(reporter: &mut Reporter) {
    let mut output = Bitmap::new();
    // SkPMColor pixel[1]; output.installPixels(SkImageInfo::MakeN32Premul(1, 1), pixel, 4);
    let installed =
        output.install_pixels(&ImageInfo::new_n32_premul((1, 1), None), vec![0u8; 4], 4);
    assert!(installed, "installPixels failed");

    let mut surf =
        surfaces::raster(&ImageInfo::new_n32_premul((300, 33300), None), None, None).unwrap();

    let r = Rect::new(0.0, 33000.0, 300.0, 33300.0);
    let x = scalar_round_to_int(r.left());
    let y = scalar_round_to_int(r.top());

    // check that the pixel in question starts as transparent (by the surface)
    if surf.read_pixels_to_bitmap(&mut output, (x, y)) {
        reporter_assert!(reporter, 0 == output.get_addr32(0, 0));
    } else {
        reporter_assert!(reporter, false, "readPixels failed");
    }

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(Color::WHITE);

    surf.canvas().draw_rect(r, &paint);

    // Now check that it is BLACK
    if surf.read_pixels_to_bitmap(&mut output, (x, y)) {
        // don't know what swizzling PMColor did, but white should always
        // appear the same.
        reporter_assert!(reporter, 0xFFFF_FFFF == output.get_addr32(0, 0));
    } else {
        reporter_assert!(reporter, false, "readPixels failed");
    }
}

/*
 *  We check for finite paths (i.e. no coords that are Inf or NaN) before we try to
 *  rasterize... except we (before) didn't check after we applied the CTM
 *  (in SkDraw::drawPath). That could allow a (post-ctm) non-finite path down into
 *  the scan-converters, which is not supported (e.g. we assume we can compute intermediate
 *  values during clipping).
 *
 *  The fix was to check isFinite() after applying the CTM. With this, trying to draw
 *  this path will assert (e.g. in SkLineClipper).
 */
// Port of: tests/DrawPathTest.cpp#L67-L100 (chrome/m156)
fn test_big_hairpath(_reporter: &mut Reporter) {
    let mut surf =
        surfaces::raster(&ImageInfo::new_n32_premul((100, 100), None), None, None).unwrap();
    let canvas = surf.canvas();

    // big, but still (barely) finite
    let big: f32 = f32::MAX / 3.0;
    let pts = [
        Point::new(0.0, 0.0),
        Point::new(big, 20.0),
        Point::new(20.0, big),
    ];
    // big enough to turn 'big' into Inf
    canvas.scale((4.0, 4.0));

    // The makeToggleInverseFillType() is needed, as it allows us to skip-past the
    // quickReject() logic in SkCanvas. The presence of an invere-fill skips those
    // allowing the path to go down to SkDraw (where the CTM is applied).
    //
    // Other possible ways to skip that quickReject... imagefilter, some patheffects
    //
    let path = Path::polygon(&pts, true, None, None).with_toggle_inverse_fill_type();

    let mut paint = Paint::default();
    paint.set_stroke(true);
    paint.set_stroke_width(0.0);
    for aa in [false, true] {
        paint.set_anti_alias(aa);
        canvas.draw_path(&path, &paint);
    }
}

///////////////////////////////////////////////////////////////////////////////

// Port of: tests/DrawPathTest.cpp#L104-L107 (chrome/m156)
fn move_to_h(builder: &mut PathBuilder, raw: &[u32]) {
    let fptr: Vec<f32> = raw.iter().map(|&b| f32::from_bits(b)).collect();
    builder.move_to((fptr[0], fptr[1]));
}

// Port of: tests/DrawPathTest.cpp#L109-L112 (chrome/m156)
fn cubic_to_h(builder: &mut PathBuilder, raw: &[u32]) {
    let fptr: Vec<f32> = raw.iter().map(|&b| f32::from_bits(b)).collect();
    builder.cubic_to((fptr[0], fptr[1]), (fptr[2], fptr[3]), (fptr[4], fptr[5]));
}

// This used to assert, because we performed a cast (int)(pt[0].fX * scale) to
// arrive at an int (SkFDot6) rather than calling sk_float_round2int. The assert
// was that the initial line-segment produced by the cubic was not monotonically
// going down (i.e. the initial DY was negative). By rounding the floats, we get
// the more proper result.
//
// http://code.google.com/p/chromium/issues/detail?id=131181
//

// we're not calling this test anymore; is that for a reason?

// Port of: tests/DrawPathTest.cpp#L127-L153 (chrome/m156)
fn test_crbug131181() {
    /*
    fX = 18.8943768,
    fY = 129.121277
    }, {
    fX = 18.8937435,
    fY = 129.121689
    }, {
    fX = 18.8950119,
    fY = 129.120422
    }, {
    fX = 18.5030727,
    fY = 129.13121
    */
    let data: [u32; 8] = [
        0x4197_27af,
        0x4301_1f0c,
        0x4197_2663,
        0x4301_1f27,
        0x4197_28fc,
        0x4301_1ed4,
        0x4194_064b,
        0x4301_2197,
    ];

    let mut builder = PathBuilder::new();
    move_to_h(&mut builder, &data[0..]);
    cubic_to_h(&mut builder, &data[2..]);

    let mut surface =
        surfaces::raster(&ImageInfo::new_n32_premul((640, 480), None), None, None).unwrap();

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    surface.canvas().draw_path(&builder.detach(), &paint);
}

// This used to assert in debug builds (and crash writing bad memory in release)
// because we overflowed an intermediate value (B coefficient) setting up our
// stepper for the quadratic. Now we bias that value by 1/2 so we don't overflow
// Port of: tests/DrawPathTest.cpp#L155-L167 (chrome/m156)
fn test_crbug_140803() {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((2700, 30 * 1024), None);
    let canvas = Canvas::from_bitmap(&mut bm, None).expect("canvas");

    let path = PathBuilder::new()
        .move_to((2762.0, 20.0))
        .quad_to((11.0, 21702.0), (10.0, 21706.0))
        .detach();

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    canvas.draw_path(&path, &paint);
}

// Port of: tests/DrawPathTest.cpp#L169-L214 (chrome/m156)
fn test_crbug_1239558(reporter: &mut Reporter) {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((256, 256), None);

    {
        // The canvas owns the bitmap while it lives (docs/design/pixels.md), so it is scoped
        // before the pixels are read back.
        let canvas = Canvas::from_bitmap(&mut bm, None).expect("canvas");
        canvas.clear(Color::WHITE);

        // This creates a single cubic where the control points form an extremely skinny, vertical
        // triangle contained within the x=0 column of pixels. Since it is convex (ignoring the leading
        // moveTo's) it uses the convex aaa optimized edge walking algorithm after clipping the path to
        // the device bounds. However, due to fixed-point math while walking these edges, the edge
        // walking evaluates to coords that are very slightly less than 0 (i.e. 0.0012). Both the left
        // and right edges would be out of bounds, but the edge walking is optimized to only clamp the
        // left edge to the left bounds, and the right edge to the right bounds. After this clamping,
        // the left and right edges are no longer sorted. This then led to incorrect behavior in various
        // forms (described below).
        let mut builder = PathBuilder::new_with_fill_type(PathFillType::Winding);
        builder.move_to((7.00649e-45_f32, 2.0));
        builder.move_to((0.016_021_9_f32, 7.45063e-09_f32));
        builder.move_to((192.263_f32, 8.40779e-44_f32));
        builder.move_to((7.34684e-40_f32, 194.25));
        builder.move_to((2.3449e-38_f32, 6.01858e-36_f32));
        builder.move_to((7.34684e-40_f32, 194.25));
        builder.cubic_to(
            (5.07266e-39_f32, 56.0488_f32),
            (0.011_917_2_f32, 0.0),
            (7.34684e-40_f32, 194.25),
        );

        let mut paint = Paint::default();
        paint.set_color(Color::RED);
        paint.set_anti_alias(true);
        // On debug builds, the inverted left/right edges led to a negative coverage that triggered an
        // assert while converting to a uint8 alpha value. On release builds with UBSAN, it would
        // detect a negative left shift when computing the pixel address and crash. On regular release
        // builds it would write a saturate coverage value to pixels that wrapped around to the far edge
        canvas.draw_path(&builder.detach(), &paint);
    }

    // UBSAN and debug builds would fail inside the drawPath() call above, but detect the incorrect
    // memory access on release builds so that the test would fail. Given the path, it should only
    // touch pixels with x=0 but the incorrect addressing would wrap to the right edge.
    for y in 0..256 {
        if bm.get_color((255, y)) != Color::WHITE {
            reporter_assert!(reporter, false, "drawPath modified incorrect pixels");
            break;
        }
    }
}

// Need to exercise drawing an inverse-path whose bounds intersect the clip,
// but whose edges do not (since its a quad which draws only in the bottom half
// of its bounds).
// In the debug build, we used to assert in this case, until it was fixed.
//
// Port of: tests/DrawPathTest.cpp#L216-L255 (chrome/m156)
fn test_inversepathwithclip() {
    let mut builder = PathBuilder::new();
    builder.move_to((0.0, 20.0));
    builder.quad_to((10.0, 10.0), (20.0, 20.0));
    builder.toggle_inverse_fill_type();

    let mut path = builder.detach();

    let mut paint = Paint::default();

    let mut surface =
        surfaces::raster(&ImageInfo::new_n32_premul((640, 480), None), None, None).unwrap();
    let canvas = surface.canvas();
    canvas.save();
    canvas.clip_rect(Rect::from_wh(19.0, 11.0), None, None);

    paint.set_anti_alias(false);
    canvas.draw_path(&path, &paint);
    paint.set_anti_alias(true);
    canvas.draw_path(&path, &paint);

    canvas.restore();

    // Now do the test again, with the path flipped, so we only draw in the
    // top half of our bounds, and have the clip intersect our bounds at the
    // bottom.
    builder.move_to((0.0, 10.0));
    builder.quad_to((10.0, 20.0), (20.0, 10.0));
    path = builder.detach();

    canvas.clip_rect(Rect::from_xywh(0.0, 19.0, 19.0, 11.0), None, None);

    paint.set_anti_alias(false);
    canvas.draw_path(&path, &paint);
    paint.set_anti_alias(true);
    canvas.draw_path(&path, &paint);
}

// Port of: tests/DrawPathTest.cpp#L257-L270 (chrome/m156)
fn test_bug533() {
    /*
       http://code.google.com/p/skia/issues/detail?id=533
       This particular test/bug only applies to the float case, where the
       coordinates are very large.
    */
    let path = PathBuilder::new()
        .move_to((64.0, 3.0))
        .quad_to((-329_936.0, -100_000_000.0), (1153.0, 330_003.0))
        .detach();

    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    let mut surface =
        surfaces::raster(&ImageInfo::new_n32_premul((640, 480), None), None, None).unwrap();
    surface.canvas().draw_path(&path, &paint);
}

// Port of: tests/DrawPathTest.cpp#L272-L292 (chrome/m156)
fn test_crbug_140642() {
    /*
    *  We used to see this construct, and due to rounding as we accumulated
    *  our length, the loop where we apply the phase would run off the end of
    *  the array, since it relied on just -= each interval value, which did not
    *  behave as "expected". Now the code explicitly checks for walking off the
    *  end of that array.

    *  A different (better) fix might be to rewrite dashing to do all of its
    *  length/phase/measure math using double, but this may need to be
    *  coordinated with SkPathMeasure, to be consistent between the two.

    <path stroke="mintcream" stroke-dasharray="27734 35660 2157846850 247"
          stroke-dashoffset="-248.135982067">
    */

    let vals: [scalar; 4] = [27734.0, 35660.0, 2_157_846_850.0, 247.0];
    let _dont_assert = PathEffect::dash(&vals, -248.135_982_067);
}

// Port of: tests/DrawPathTest.cpp#L294-L302 (chrome/m156)
fn test_crbug_124652() {
    /*
       http://code.google.com/p/chromium/issues/detail?id=124652
       This particular test/bug only applies to the float case, where
       large values can "swamp" small ones.
    */
    let intervals: [scalar; 2] = [837_099_584.0, 33450.0];
    let _dont_assert = PathEffect::dash(&intervals, -10.0);
}

// Port of: tests/DrawPathTest.cpp#L304-L315 (chrome/m156)
fn test_bigcubic() {
    let path = PathBuilder::new()
        .move_to((64.0, 3.0))
        .cubic_to(
            (-329_936.0, -100_000_000.0),
            (-329_936.0, 100_000_000.0),
            (1153.0, 330_003.0),
        )
        .detach();

    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    let mut surface =
        surfaces::raster(&ImageInfo::new_n32_premul((640, 480), None), None, None).unwrap();
    surface.canvas().draw_path(&path, &paint);
}

// asserts if halfway case is not handled
// Port of: tests/DrawPathTest.cpp#L317-L346 (chrome/m156)
fn test_halfway() {
    let paint = Paint::default();
    let mut builder = PathBuilder::new();
    builder.move_to((16365.5, 1394.0));
    builder.line_to((16365.5, 1387.5));
    builder.quad_to((16365.5, 1385.43), (16367.0, 1383.96));
    builder.quad_to((16368.4, 1382.5), (16370.5, 1382.5));
    builder.line_to((16465.5, 1382.5));
    builder.quad_to((16467.6, 1382.5), (16469.0, 1383.96));
    builder.quad_to((16470.5, 1385.43), (16470.5, 1387.5));
    builder.line_to((16470.5, 1394.0));
    builder.quad_to((16470.5, 1396.07), (16469.0, 1397.54));
    builder.quad_to((16467.6, 1399.0), (16465.5, 1399.0));
    builder.line_to((16370.5, 1399.0));
    builder.quad_to((16368.4, 1399.0), (16367.0, 1397.54));
    builder.quad_to((16365.5, 1396.07), (16365.5, 1394.0));
    builder.close();
    let path = builder.detach();

    let mut p2 = path.make_offset((0.001, 0.001));

    let mut surface =
        surfaces::raster(&ImageInfo::new_n32_premul((640, 480), None), None, None).unwrap();
    let canvas = surface.canvas();
    canvas.translate((-16366.0, -1383.0));
    canvas.draw_path(&p2, &paint);

    p2 = p2.make_offset((-0.001, -0.001));
    canvas.draw_path(&p2, &paint);

    p2 = path.make_transform(Matrix::i());
    canvas.draw_path(&p2, &paint);
}

// we used to assert if the bounds of the device (clip) was larger than 32K
// even when the path itself was smaller. We just draw and hope in the debug
// version to not assert.
// Port of: tests/DrawPathTest.cpp#L348-L360 (chrome/m156)
fn test_giantaa() {
    const W: i32 = 400;
    const H: i32 = 400;
    let mut surface =
        surfaces::raster(&ImageInfo::new_n32_premul((33000, 10), None), None, None).unwrap();

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    #[allow(clippy::cast_precision_loss)] // small ints, exact in f32
    let path = Path::oval(
        Rect::from_xywh(-10.0, -10.0, (20 + W) as scalar, (20 + H) as scalar),
        None,
    );
    surface.canvas().draw_path(&path, &paint);
}

// Extremely large path_length/dash_length ratios may cause infinite looping
// in SkDashPathEffect::filterPath() due to single precision rounding.
// The test is quite expensive, but it should get much faster after the fix
// for http://crbug.com/165432 goes in.
// Port of: tests/DrawPathTest.cpp#L362-L378 (chrome/m156)
fn test_infinite_dash(reporter: &mut Reporter) {
    let path = Path::line((0.0, 0.0), (5_000_000.0, 0.0));

    let intervals: [scalar; 2] = [0.2, 0.2];
    let dash = PathEffect::dash(&intervals, 0.0);

    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_path_effect(dash);

    let _ = fill_path_with_paint_to_path(&path, &paint);
    // If we reach this, we passed.
    reporter_assert!(reporter, true);
}

// http://crbug.com/165432
// Limit extreme dash path effects to avoid exhausting the system memory.
// Port of: tests/DrawPathTest.cpp#L380-L396 (chrome/m156)
fn test_crbug_165432(reporter: &mut Reporter) {
    let path = Path::line((0.0, 0.0), (10_000_000.0, 0.0));

    let intervals: [scalar; 2] = [0.5, 0.5];
    let dash = PathEffect::dash(&intervals, 0.0).expect("dash");

    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_path_effect(Some(dash.clone()));

    let mut filtered_path = PathBuilder::new();
    let mut rec = StrokeRec::from_paint(&paint, None, None);
    reporter_assert!(
        reporter,
        !dash.filter_path_inplace(&mut filtered_path, &path, &mut rec, None)
    );
    reporter_assert!(reporter, filtered_path.is_empty());
}

// http://crbug.com/472147
// This is a simplified version from the bug. RRect radii not properly scaled.
// Port of: tests/DrawPathTest.cpp#L398-L411 (chrome/m156)
fn test_crbug_472147_simple(_reporter: &mut Reporter) {
    let mut surface =
        surfaces::raster(&ImageInfo::new_n32_premul((1000, 1000), None), None, None).unwrap();
    let canvas = surface.canvas();
    let p = Paint::default();
    let r = Rect::new(-246.0, 33.0, 848.0, 33_554_464.0);
    let radii: [Vector; 4] = [
        Vector::new(13.0, 8.0),
        Vector::new(170.0, 2.0),
        Vector::new(256.0, 33_554_430.0),
        Vector::new(120.0, 5.0),
    ];
    let mut rr = RRect::default();
    rr.set_rect_radii(r, &radii);
    canvas.draw_rrect(rr, &p);
}

// http://crbug.com/472147
// RRect radii not properly scaled.
// Port of: tests/DrawPathTest.cpp#L413-L430 (chrome/m156)
fn test_crbug_472147_actual(_reporter: &mut Reporter) {
    let mut surface =
        surfaces::raster(&ImageInfo::new_n32_premul((1000, 1000), None), None, None).unwrap();
    let canvas = surface.canvas();
    let p = Paint::default();
    let r = Rect::new(-246.0, 33.0, 848.0, 33_554_464.0);
    let radii: [Vector; 4] = [
        Vector::new(13.0, 8.0),
        Vector::new(170.0, 2.0),
        Vector::new(256.0, 33_554_430.0),
        Vector::new(120.0, 5.0),
    ];
    let mut rr = RRect::default();
    rr.set_rect_radii(r, &radii);
    canvas.clip_rrect(rr, None, None);

    let r2 = Rect::new(0.0, 33.0, 1102.0, 33_554_464.0);
    canvas.draw_rect(r2, &p);
}

// Port of: tests/DrawPathTest.cpp#L440-L458 (chrome/m156)
def_test!(DrawPath, |reporter| {
    test_giantaa();
    test_bug533();
    test_bigcubic();
    test_crbug_124652();
    test_crbug_140642();
    test_crbug_140803();
    test_inversepathwithclip();
    // why?
    if false {
        test_crbug131181();
    }
    test_infinite_dash(reporter);
    test_crbug_165432(reporter);
    test_crbug_472147_simple(reporter);
    test_crbug_472147_actual(reporter);
    test_crbug_1239558(reporter);
    test_big_aa_rect(reporter);
    test_halfway();
    test_big_hairpath(reporter);
});
