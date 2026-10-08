// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/BlurTest.cpp (chrome/m156)

#![cfg(test)]
#![allow(clippy::cast_precision_loss)] // the C++ converts ints to floats implicitly
#![allow(clippy::cast_possible_truncation)] // the C++ converts floats to ints implicitly
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)] // the C++ converts size_t to int implicitly
#![allow(clippy::neg_cmp_op_on_partial_ord)] // REPORTER_ASSERT(r, sigma > 0) negates the condition
#![allow(clippy::float_cmp)] // the C++ compares scalars with ==

use crate::{def_test, reporter_assert};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blur_mask::BlurMask;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;

use skia_rust_core::mask::{AllocType, MaskBuilder, MaskFormat};
use skia_rust_core::mask_filter::{BlurRec, MaskFilter};
use skia_rust_core::math_priv::clamp_pos;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_utils::fill_path_with_paint_to_path;
use skia_rust_core::point::IPoint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{Contains, IRect, Rect, RoundOut};
use skia_rust_core::scalar::{int_to_scalar, scalar, scalar_ceil_to_int, scalar_exp, scalar_sqrt};
use skia_rust_core::t_pin::t_pin;
use skia_rust_effects::emboss_mask_filter::{self, Light};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;

// Port of: tests/BlurTest.cpp#L74-L76 (chrome/m156)
const OUTSET: i32 = 100;
const BG_COLOR: Color = Color::WHITE;
const STROKE_WIDTH: i32 = 4;

// Port of: tests/BlurTest.cpp#L78-L80 (chrome/m156)
fn create(bound: &IRect) -> Bitmap {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((bound.width(), bound.height()), None);
    bm
}

// Port of: tests/BlurTest.cpp#L82-L84 (chrome/m156)
fn draw_bg(canvas: &Canvas) {
    canvas.draw_color(BG_COLOR, None);
}

// Port of: tests/BlurTest.cpp#L87-L91 (chrome/m156)
struct BlurTest {
    add_path: fn() -> Path,
    view_len: usize,
    views: [IRect; 9],
}

//Path Draw Procs
//Beware that paths themselves my draw differently depending on the clip.
// Port of: tests/BlurTest.cpp#L93-L95 (chrome/m156)
fn draw_50x50_rect() -> Path {
    Path::rect(Rect::new(0.0, 0.0, 50.0, 50.0), None)
}

//Tests
// Port of: tests/BlurTest.cpp#L97-L106 (chrome/m156)
fn tests() -> [BlurTest; 1] {
    let mut views = [IRect::new(0, 0, 0, 0); 9];
    // inner half of blur
    views[0] = IRect::new(0, 0, 50, 50);
    // blur, but no path.
    views[1] = IRect::new(50 + STROKE_WIDTH / 2, 50 + STROKE_WIDTH / 2, 100, 100);
    // just an edge
    views[2] = IRect::new(40, STROKE_WIDTH, 60, 50 - STROKE_WIDTH);
    [BlurTest {
        add_path: draw_50x50_rect,
        view_len: 3,
        views,
    }]
}

/** Assumes that the ref draw was completely inside ref canvas --
   implies that everything outside is "bgColor".
   Checks that all overlap is the same and that all non-overlap on the
   ref is "bgColor".
*/
// Port of: tests/BlurTest.cpp#L108-L133 (chrome/m156)
fn compare(reference: &Bitmap, iref: &IRect, test: &Bitmap, itest: &IRect) -> bool {
    let x_off = itest.left - iref.left;
    let y_off = itest.top - iref.top;

    for y in 0..test.height() {
        for x in 0..test.width() {
            let test_color = test.get_color((x, y));
            let ref_x = x + x_off;
            let ref_y = y + y_off;
            let ref_color = if ref_x >= 0
                && ref_x < reference.width()
                && ref_y >= 0
                && ref_y < reference.height()
            {
                reference.get_color((ref_x, ref_y))
            } else {
                BG_COLOR
            };
            if ref_color != test_color {
                return false;
            }
        }
    }
    true
}

// Port of: tests/BlurTest.cpp#L135-L182 (chrome/m156)
def_test!(BlurDrawing, |reporter| {
    let mut paint = Paint::default();
    paint.set_color(Color::GRAY);
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(int_to_scalar(STROKE_WIDTH));

    let sigma = BlurMask::convert_radius_to_sigma(int_to_scalar(5));
    for style in 0..=(BlurStyle::LAST_ENUM as i32) {
        let blur_style = BlurStyle::from_i32(style).unwrap();

        for respect_ctm in [false, true] {
            paint.set_mask_filter(MaskFilter::blur(blur_style, sigma, respect_ctm));

            for test in &tests() {
                let path = (test.add_path)();
                let stroked_path = fill_path_with_paint_to_path(&path, &paint).0;
                let ref_bound = stroked_path.bounds();
                let mut iref: IRect = ref_bound.round_out();
                iref.inset((-OUTSET, -OUTSET));
                let mut ref_bitmap = create(&iref);

                {
                    let ref_canvas = Canvas::from_bitmap(&mut ref_bitmap, None).expect("canvas");
                    ref_canvas.translate((int_to_scalar(-iref.left), int_to_scalar(-iref.top)));
                    draw_bg(&ref_canvas);
                    ref_canvas.draw_path(&path, &paint);
                }

                for view in 0..test.view_len {
                    let itest = test.views[view];
                    let mut test_bitmap = create(&itest);

                    {
                        let test_canvas =
                            Canvas::from_bitmap(&mut test_bitmap, None).expect("canvas");
                        test_canvas
                            .translate((int_to_scalar(-itest.left), int_to_scalar(-itest.top)));
                        draw_bg(&test_canvas);
                        test_canvas.draw_path(&path, &paint);
                    }

                    reporter_assert!(reporter, compare(&ref_bitmap, &iref, &test_bitmap, &itest));
                }
            }
        }
    }
});

///////////////////////////////////////////////////////////////////////////////

// Use SkBlurMask::BlurGroundTruth to blur a 'width' x 'height' solid
// white rect. Return the right half of the middle row in 'result'.
// Port of: tests/BlurTest.cpp#L186-L222 (chrome/m156)
fn ground_truth_2d(width: i32, height: i32, sigma: scalar, result: &mut [i32]) {
    let result_count = result.len();
    let mut src = MaskBuilder::default();
    let mut dst = MaskBuilder::default();

    src.bounds.set_wh(width, height);
    src.format = MaskFormat::A8;
    src.row_bytes = src.bounds.width() as u32;
    src.image = MaskBuilder::alloc_image(src.compute_total_image_size(), AllocType::Uninit);

    let total = src.compute_total_image_size();
    src.image[..total].fill(0xff);

    if !BlurMask::blur_ground_truth(sigma, &mut dst, &src.as_mask(), BlurStyle::Normal, None) {
        return;
    }

    let mid_x = dst.bounds.x() + dst.bounds.width() / 2;
    let mid_y = dst.bounds.y() + dst.bounds.height() / 2;
    let dst_mask = dst.as_mask();
    let bytes = dst_mask.get_addr8(mid_x, mid_y);
    let mut i = 0;
    while (i as i32) < dst.bounds.width() - (mid_x - dst.bounds.left) {
        if i < result_count {
            result[i] = i32::from(bytes[i]);
        }
        i += 1;
    }
    while i < result_count {
        result[i] = 0;
        i += 1;
    }
}

// Implement a step function that is 255 between min and max; 0 elsewhere.
// Port of: tests/BlurTest.cpp#L224-L230 (chrome/m156)
fn step(x: i32, min: scalar, max: scalar) -> i32 {
    if min < x as scalar && (x as scalar) < max {
        return 255;
    }
    0
}

// Implement a Gaussian function with 0 mean and std.dev. of 'sigma'.
// Port of: tests/BlurTest.cpp#L232-L237 (chrome/m156)
fn gaussian(x: i32, sigma: scalar) -> f32 {
    let k = 1.0 / (sigma * scalar_sqrt(2.0 * core::f32::consts::PI));
    let exponent = (-(x * x)) as f32 / (2.0 * sigma * sigma);
    k * scalar_exp(exponent)
}

// Perform a brute force convolution of a step function with a Gaussian.
// Return the right half in 'result'
// Port of: tests/BlurTest.cpp#L239-L254 (chrome/m156)
fn brute_force_1d(step_min: scalar, step_max: scalar, gaussian_sigma: scalar, result: &mut [i32]) {
    let gaussian_range = scalar_ceil_to_int(10.0 * gaussian_sigma);

    for (i, r) in result.iter_mut().enumerate() {
        let i = i as i32;
        let mut sum: scalar = 0.0;
        for j in -gaussian_range..gaussian_range {
            sum += gaussian(j, gaussian_sigma) * step(i - j, step_min, step_max) as f32;
        }

        *r = t_pin(clamp_pos((sum + 0.5) as i32), 0, 255);
    }
}

// Port of: tests/BlurTest.cpp#L256-L269 (chrome/m156)
fn blur_path(canvas: &Canvas, path: &Path, gaussian_sigma: scalar) {
    let mid_x = path.bounds().center_x();
    let mid_y = path.bounds().center_y();

    canvas.translate((-mid_x, -mid_y));

    let mut blur_paint = Paint::default();
    blur_paint.set_color(Color::WHITE);
    blur_paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, gaussian_sigma, None));

    canvas.draw_color(Color::BLACK, None);
    canvas.draw_path(path, &blur_paint);
}

// Readback the blurred draw results from the canvas
// Port of: tests/BlurTest.cpp#L271-L283 (chrome/m156)
fn readback(src: &Bitmap, result: &mut [i32]) {
    let result_count = result.len();
    let mut readback = Bitmap::new();
    readback.alloc_n32_pixels((result_count as i32, 30), None);
    let mut pm = readback.peek_pixels_mut().expect("pixels");
    let info = pm.info().clone();
    let row_bytes = pm.row_bytes();
    assert!(src.read_pixels(&info, pm.writable_addr().expect("pixels"), row_bytes, 0, 0));

    for (i, r) in result.iter_mut().enumerate() {
        *r = i32::from(pm.get_color((i as i32, 15)).r());
    }
}

// Draw a blurred version of the provided path.
// Return the right half of the middle row in 'result'.
// Port of: tests/BlurTest.cpp#L285-L296 (chrome/m156)
fn cpu_blur_path(path: &Path, gaussian_sigma: scalar, result: &mut [i32]) {
    let mut bitmap = Bitmap::new();
    bitmap.alloc_n32_pixels((result.len() as i32, 30), None);
    {
        let canvas = Canvas::from_bitmap(&mut bitmap, None).expect("canvas");

        blur_path(&canvas, path, gaussian_sigma);
    }
    readback(&bitmap, result);
}

// Port of: tests/BlurTest.cpp#L306-L316 (chrome/m156)
fn match_(first: &[i32], second: &[i32], count: usize, tol: i32) -> bool {
    for i in 0..count {
        let delta = first[i] - second[i];
        if delta > tol || delta < -tol {
            return false;
        }
    }

    true
}

// Test out the normal blur style with a wide range of sigmas
// Port of: tests/BlurTest.cpp#L318-L364 (chrome/m156)
def_test!(BlurSigmaRange, |reporter| {
    const SIZE: usize = 100;

    // The geometry is offset a smidge to trigger:
    // https://code.google.com/p/chromium/issues/detail?id=282418
    let rect_path = Path::rect(Rect::new(0.3, 0.3, 100.3, 100.3), None);

    let poly_pts = [
        Point::new(0.3, 0.3),
        Point::new(100.3, 0.3),
        Point::new(100.3, 100.3),
        Point::new(0.3, 100.3),
        Point::new(2.3, 50.3), // a little divet to throw off the rect special case
    ];
    let poly_path = Path::polygon(&poly_pts, true, None, None);

    let mut rect_special_case_result = [0i32; SIZE];
    let mut general_case_result = [0i32; SIZE];
    let mut ground_truth_result = [0i32; SIZE];
    let mut brute_force_1d_result = [0i32; SIZE];

    let mut sigma: scalar = 10.0;

    for _ in 0..4 {
        cpu_blur_path(&rect_path, sigma, &mut rect_special_case_result);
        cpu_blur_path(&poly_path, sigma, &mut general_case_result);

        ground_truth_2d(100, 100, sigma, &mut ground_truth_result);
        brute_force_1d(-50.0, 50.0, sigma, &mut brute_force_1d_result);

        reporter_assert!(
            reporter,
            match_(&rect_special_case_result, &brute_force_1d_result, SIZE, 5)
        );
        reporter_assert!(
            reporter,
            match_(&general_case_result, &brute_force_1d_result, SIZE, 15)
        );
        reporter_assert!(
            reporter,
            match_(&ground_truth_result, &brute_force_1d_result, SIZE, 1)
        );

        sigma /= 10.0;
    }
});

///////////////////////////////////////////////////////////////////////////////////////////

// Port of: tests/BlurTest.cpp#L366-L430 (chrome/m156)
def_test!(BlurAsABlur, |reporter| {
    let styles = [
        BlurStyle::Normal,
        BlurStyle::Solid,
        BlurStyle::Outer,
        BlurStyle::Inner,
    ];
    let sigmas: [scalar; 4] = [
        // values <= 0 should not success for a blur
        -1.0, 0.0, 0.5, 2.0,
    ];

    // Test asABlur for SkBlurMaskFilter
    //
    for &style in &styles {
        for &sigma in &sigmas {
            for respect_ctm in [false, true] {
                let mf = MaskFilter::blur(style, sigma, respect_ctm);
                match mf {
                    None => {
                        reporter_assert!(reporter, sigma <= 0.0);
                    }
                    Some(mf) => {
                        reporter_assert!(reporter, sigma > 0.0);
                        let rec: Option<BlurRec> = mf.as_base().as_a_blur();
                        let success = rec.is_some();
                        if respect_ctm {
                            reporter_assert!(reporter, success);
                            let rec = rec.unwrap();
                            reporter_assert!(reporter, rec.sigma == sigma);
                            reporter_assert!(reporter, rec.style == style);
                        } else {
                            reporter_assert!(reporter, !success);
                        }

                        let src = Rect::new(0.0, 0.0, 100.0, 100.0);
                        let dst = mf.as_base().compute_fast_bounds(&src);

                        // This is a very conservative test. With more knowledge, we could
                        // consider more stringent tests.
                        reporter_assert!(reporter, dst.contains(&src));
                    }
                }
            }
        }
    }

    // Test asABlur for SkEmbossMaskFilter -- should never succeed
    //
    {
        let light = Light {
            direction: [1.0, 1.0, 1.0],
            pad: 0,
            ambient: 127,
            specular: 127,
        };
        for &sigma in &sigmas {
            let mf = emboss_mask_filter::new(sigma, &light);
            if let Some(mf) = mf {
                let success = mf.as_base().as_a_blur().is_some();
                reporter_assert!(reporter, !success);
            }
        }
    }
});

///////////////////////////////////////////////////////////////////////////////////////////

// Port of: tests/BlurTest.cpp#L529-L556 (chrome/m156)
def_test!(BlurZeroSigma, |reporter| {
    let mut surf = surfaces::raster_n32_premul((20, 20)).expect("surface");
    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    let ir = IRect::new(5, 5, 15, 15);
    let r = Rect::from_irect(ir);

    let sigmas: [scalar; 2] = [0.0, skia_rust_core::float_bits::bits_to_float(1)];
    // if sigma is zero (or nearly so), we need to draw correctly (unblurred) and not crash
    // or assert.
    for sigma in sigmas {
        paint.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, sigma, None));
        surf.canvas().draw_rect(r, &paint);

        let pixels = surf.peek_pixels().expect("pixels");
        let pm = pixels.pixmap();
        // ToolUtils::PixelIter
        for y in 0..pm.height() {
            for x in 0..pm.width() {
                let p = pm.addr32(x, y);
                if ir.contains(IPoint::new(x, y)) {
                    // inside the rect we draw (opaque black)
                    reporter_assert!(
                        reporter,
                        p == skia_rust_core::color_priv::pack_argb32(0xFF, 0, 0, 0)
                    );
                } else {
                    // outside the rect we didn't draw at all, no blurred edges
                    reporter_assert!(reporter, p == 0);
                }
            }
        }
    }
});

///////////////////////////////////////////////////////////////////////////////////////////

// Port of: tests/BlurTest.cpp#L594-L601 (chrome/m156)
def_test!(zero_blur, |_reporter| {
    let mut alpha = Bitmap::new();
    let bitmap = Bitmap::new();

    let mut paint = Paint::default();
    paint.set_mask_filter(MaskFilter::blur(BlurStyle::Outer, 3.0, None));
    let _offset = bitmap.extract_alpha(&mut alpha, &paint);
});
