// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/DrawBitmapRectTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{Canvas, SrcRectConstraint};
use skia_rust_core::color::Color;
use skia_rust_core::matrix::{Matrix, TypeMask};
use skia_rust_core::matrix_utils::treat_as_sprite;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::{SCALAR_NAN, scalar};
use skia_rust_core::size::ISize;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::{Reporter, def_tier_test, reporter_assert};

///////////////////////////////////////////////////////////////////////////////

// Port of: tests/DrawBitmapRectTest.cpp#L33-L50 (chrome/m156)
fn rand_matrix(mat: &mut Matrix, rand: &mut Random, mask: TypeMask) {
    mat.set_identity();
    if mask.contains(TypeMask::TRANSLATE) {
        mat.post_translate((rand.next_s_scalar1(), rand.next_s_scalar1()));
    }
    if mask.contains(TypeMask::SCALE) {
        mat.post_scale((rand.next_s_scalar1(), rand.next_s_scalar1()), None);
    }
    if mask.contains(TypeMask::AFFINE) {
        mat.post_rotate(rand.next_s_scalar1() * 360.0, None);
    }
    if mask.contains(TypeMask::PERSPECTIVE) {
        mat.set_persp_x(rand.next_s_scalar1());
        mat.set_persp_y(rand.next_s_scalar1());
    }
}

// Port of: tests/DrawBitmapRectTest.cpp#L52-L54 (chrome/m156)
#[allow(clippy::cast_possible_wrap)] // masked to 16 bits
fn rand_size(size: &mut ISize, rand: &mut Random) {
    size.width = (rand.next_u() & 0xFFFF) as i32;
    size.height = (rand.next_u() & 0xFFFF) as i32;
}

// Port of: tests/DrawBitmapRectTest.cpp#L56-L108 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors SkScalar arithmetic on ints
fn test_treat_as_sprite(reporter: &mut Reporter) {
    let mut mat = Matrix::new_identity();
    let mut size = ISize::default();
    let mut rand = Random::default();

    let sampling = SamplingOptions::default();

    // assert: translate-only no-aa can always be treated as sprite
    for _ in 0..1000 {
        rand_matrix(&mut mat, &mut rand, TypeMask::TRANSLATE);
        for _ in 0..1000 {
            rand_size(&mut size, &mut rand);
            reporter_assert!(reporter, treat_as_sprite(&mat, size, &sampling, false));
        }
    }

    // assert: rotate/perspect is never treated as sprite
    for _ in 0..1000 {
        rand_matrix(
            &mut mat,
            &mut rand,
            TypeMask::AFFINE | TypeMask::PERSPECTIVE,
        );
        for _ in 0..1000 {
            rand_size(&mut size, &mut rand);
            reporter_assert!(reporter, !treat_as_sprite(&mat, size, &sampling, false));
            reporter_assert!(reporter, !treat_as_sprite(&mat, size, &sampling, true));
        }
    }

    size = ISize::new(500, 600);

    let too_much_subpixel: scalar = 100.1;
    mat.set_translate((too_much_subpixel, 0.0));
    reporter_assert!(reporter, !treat_as_sprite(&mat, size, &sampling, true));
    mat.set_translate((0.0, too_much_subpixel));
    reporter_assert!(reporter, !treat_as_sprite(&mat, size, &sampling, true));

    let tiny_sub_pixel: scalar = 100.02;
    mat.set_translate((tiny_sub_pixel, 0.0));
    reporter_assert!(reporter, treat_as_sprite(&mat, size, &sampling, true));
    mat.set_translate((0.0, tiny_sub_pixel));
    reporter_assert!(reporter, treat_as_sprite(&mat, size, &sampling, true));

    let two_thirds: scalar = 1.0 * 2.0 / 3.0;
    let big_scale = (size.width as scalar + two_thirds) / size.width as scalar;
    mat.set_scale((big_scale, big_scale), None);
    reporter_assert!(reporter, !treat_as_sprite(&mat, size, &sampling, false));
    reporter_assert!(reporter, !treat_as_sprite(&mat, size, &sampling, true));

    let one_third: scalar = 1.0 / 3.0;
    let small_scale = (size.width as scalar + one_third) / size.width as scalar;
    mat.set_scale((small_scale, small_scale), None);
    reporter_assert!(reporter, treat_as_sprite(&mat, size, &sampling, false));
    reporter_assert!(reporter, !treat_as_sprite(&mat, size, &sampling, true));

    let one_fortyth: scalar = 1.0 / 40.0;
    let tiny_scale = (size.width as scalar + one_fortyth) / size.width as scalar;
    mat.set_scale((tiny_scale, tiny_scale), None);
    reporter_assert!(reporter, treat_as_sprite(&mat, size, &sampling, false));
    reporter_assert!(reporter, treat_as_sprite(&mat, size, &sampling, true));
}

// Port of: tests/DrawBitmapRectTest.cpp#L110-L158 (chrome/m156)
fn test_wacky_bitmapshader(reporter: &mut Reporter, width: i32, height: i32) {
    let mut dev = Bitmap::new();
    dev.alloc_n32_pixels((0x56F, 0x4f6), None);
    dev.erase_color(Color::TRANSPARENT); // necessary, so we know if we draw to it

    let mut matrix = Matrix::new_identity();

    {
        let c = Canvas::from_bitmap(&mut dev, None).expect("a canvas");
        matrix.set_all(
            -119.340_97,
            -43.436_558,
            93_489.945,
            43.436_558,
            -119.340_97,
            123.984_26,
            0.0,
            0.0,
            1.0,
        );
        c.concat(&matrix);

        let mut bm = Bitmap::new();
        if bm.try_alloc_n32_pixels((width, height), false) {
            bm.erase_color(Color::RED);
        } else {
            debug_assert!(false);
            return;
        }

        #[allow(clippy::excessive_precision)] // the C++ literals, digit for digit
        matrix.set_all(
            0.007_874_015_7,
            0.0,
            249.0,
            0.0,
            0.007_874_015_7,
            239.0,
            0.0,
            0.0,
            1.0,
        );
        let mut paint = Paint::default();
        paint.set_shader(bm.to_shader(
            (TileMode::Repeat, TileMode::Repeat),
            SamplingOptions::default(),
            &matrix,
        ));

        let r = Rect::from_xywh(681.0, 239.0, 695.0, 253.0);
        c.draw_rect(r, &paint);
    }

    for y in 0..dev.height() {
        for x in 0..dev.width() {
            if 0 == dev.get_addr32(x, y) {
                reporter_assert!(reporter, false);
                return;
            }
        }
    }
}

// Original bug was asserting that the matrix-proc had generated a (Y) value
// that was out of range. See the comment in the C++ for the whole story.
// Port of: tests/DrawBitmapRectTest.cpp#L160-L204 (chrome/m156)
fn test_giantrepeat_crbug118018(reporter: &mut Reporter) {
    struct Tests {
        width: i32,
        height: i32,
    }

    const G_TESTS: [Tests; 3] = [
        // crbug.com/40054915 (width exceeds 64K)... should draw safely now.
        Tests {
            width: 0x1b294,
            height: 0x7f,
        },
        // should draw, test max width
        Tests {
            width: 0xFFFF,
            height: 0x7f,
        },
        // should draw, test max height
        Tests {
            width: 0x7f,
            height: 0xFFFF,
        },
    ];

    for t in &G_TESTS {
        test_wacky_bitmapshader(reporter, t.width, t.height);
    }
}

///////////////////////////////////////////////////////////////////////////////

// Port of: tests/DrawBitmapRectTest.cpp#L208-L226 (chrome/m156)
fn test_nan_antihair() {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((20, 20), None);

    let canvas = Canvas::from_bitmap(&mut bm, None).expect("a canvas");

    let mut builder = PathBuilder::new();
    builder.move_to((0.0, 0.0));
    builder.line_to((10.0, SCALAR_NAN));

    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(Style::Stroke);

    // before our fix to SkScan_Antihair.cpp to check for integral NaN (0x800...)
    // this would trigger an assert/crash.
    //
    // see rev. 3558
    canvas.draw_path(&builder.detach(), &paint);
}

// Port of: tests/DrawBitmapRectTest.cpp#L228-L239 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // width and height are positive
fn check_for_all_zeros(bm: &Bitmap) -> bool {
    let count = bm.width() as usize * bm.bytes_per_pixel();
    let Some(pixmap) = bm.peek_pixels() else {
        return true;
    };
    let row_bytes = pixmap.row_bytes();
    let bytes = pixmap.addr().expect("pixels");
    for y in 0..bm.height() as usize {
        let ptr = &bytes[y * row_bytes..y * row_bytes + count];
        if ptr.iter().any(|&b| b != 0) {
            return false;
        }
    }
    true
}

const G_WIDTH: i32 = 256;
const G_HEIGHT: i32 = 256;

// Port of: tests/DrawBitmapRectTest.cpp#L244-L247 (chrome/m156)
fn create(bm: &mut Bitmap, color: Color) {
    bm.alloc_n32_pixels((G_WIDTH, G_HEIGHT), None);
    bm.erase_color(color);
}

// Port of: tests/DrawBitmapRectTest.cpp#L249-L269 (chrome/m156)
def_tier_test!(DrawBitmapRect, |reporter| {
    let mut src = Bitmap::new();
    let mut dst = Bitmap::new();

    create(&mut src, Color::new(0xFFFF_FFFF));
    create(&mut dst, Color::new(0));

    {
        let canvas = Canvas::from_bitmap(&mut dst, None).expect("a canvas");

        #[allow(clippy::cast_precision_loss)] // SkScalar of an int
        let src_r = Rect::new(G_WIDTH as scalar, 0.0, G_WIDTH as scalar + 16.0, 16.0);
        let dst_r = Rect::new(0.0, 0.0, 16.0, 16.0);

        canvas.draw_image_rect(
            src.as_image().expect("an image"),
            Some((&src_r, SrcRectConstraint::Strict)),
            dst_r,
            &Paint::default(),
        );
    }

    // ensure that we draw nothing if srcR does not intersect the bitmap
    reporter_assert!(reporter, check_for_all_zeros(&dst));

    test_nan_antihair();
    test_giantrepeat_crbug118018(reporter);

    test_treat_as_sprite(reporter);
});
