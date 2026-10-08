// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ImageNewShaderTest.cpp (chrome/m156)
//
// Not ported: `ImageNewShader_GPU` (Ganesh) and `ImageRawShader` (decodes
// `images/mandrill_32.png`; image decoding is not ported).

#![cfg(test)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

use crate::{Reporter, def_tier_test, reporter_assert};

// Port of: tests/ImageNewShaderTest.cpp#L38-L41 (chrome/m156)
fn test_bitmap_equality(reporter: &mut Reporter, bm1: &Bitmap, bm2: &Bitmap) {
    reporter_assert!(reporter, bm1.compute_byte_size() == bm2.compute_byte_size());
    let pm1 = bm1.peek_pixels().expect("pixels");
    let pm2 = bm2.peek_pixels().expect("pixels");
    let size = bm1.compute_byte_size();
    reporter_assert!(
        reporter,
        pm1.addr().expect("addr")[..size] == pm2.addr().expect("addr")[..size]
    );
}

// Port of: tests/ImageNewShaderTest.cpp#L43-L58 (chrome/m156)
fn paint_source(source_surface: &mut Surface<'_>) {
    let height = source_surface.height();
    let source_canvas = source_surface.canvas();
    source_canvas.clear(Color::new(0xFFDE_DEDE));

    let mut paint_color = Paint::default();
    paint_color.set_color(Color::new(0xFFFF_0000));
    paint_color.set_style(Style::Fill);

    #[allow(clippy::cast_precision_loss)] // SkIntToScalar
    let rect = Rect::from_xywh(1.0, 0.0, 1.0, height as f32);

    source_canvas.draw_rect(rect, &paint_color);
}

// Port of: tests/ImageNewShaderTest.cpp#L60-L112 (chrome/m156)
fn run_shader_test(
    reporter: &mut Reporter,
    source_surface: &mut Surface<'_>,
    destination_surface: &mut Surface<'_>,
    info: &ImageInfo,
) {
    paint_source(source_surface);

    let source_image = source_surface.image_snapshot().expect("a snapshot");
    let source_shader = source_image.to_shader(
        (TileMode::Repeat, TileMode::Repeat),
        SamplingOptions::default(),
        None,
    );

    let mut paint = Paint::default();
    paint.set_shader(source_shader);

    destination_surface.canvas().clear(Color::TRANSPARENT);
    destination_surface.canvas().draw_paint(&paint);

    let mut bm_orig = Bitmap::new();
    bm_orig.alloc_n32_pixels((info.width(), info.height()), None);
    source_surface.read_pixels_to_bitmap(&mut bm_orig, (0, 0));

    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((info.width(), info.height()), None);
    destination_surface.read_pixels_to_bitmap(&mut bm, (0, 0));

    test_bitmap_equality(reporter, &bm_orig, &bm);

    // Test with a translated shader
    let mut matrix = Matrix::new_identity();
    matrix.set_translate((-1.0, 0.0));

    let source_shader_translated = source_image.to_shader(
        (TileMode::Repeat, TileMode::Repeat),
        SamplingOptions::default(),
        &matrix,
    );

    destination_surface.canvas().clear(Color::TRANSPARENT);

    let mut paint_translated = Paint::default();
    paint_translated.set_shader(source_shader_translated);

    destination_surface.canvas().draw_paint(&paint_translated);

    let mut bmt = Bitmap::new();
    bmt.alloc_n32_pixels((info.width(), info.height()), None);
    destination_surface.read_pixels_to_bitmap(&mut bmt, (0, 0));

    //  Test correctness
    {
        for y in 0..info.height() {
            reporter_assert!(reporter, Color::new(0xFFFF_0000) == bmt.get_color((0, y)));

            for x in 1..info.width() {
                reporter_assert!(reporter, Color::new(0xFFDE_DEDE) == bmt.get_color((x, y)));
            }
        }
    }
}

// Port of: tests/ImageNewShaderTest.cpp#L114-L122 (chrome/m156)
def_tier_test!(ImageNewShader, |reporter| {
    let info = ImageInfo::new_n32_premul((5, 5), None);

    let mut source_surface = surfaces::raster(&info, None, None).expect("surface");
    let mut destination_surface = surfaces::raster(&info, None, None).expect("surface");

    run_shader_test(
        reporter,
        &mut source_surface,
        &mut destination_surface,
        &info,
    );
});
