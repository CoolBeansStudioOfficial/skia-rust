// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ShaderImageFilterTest.cpp (chrome/m156)
//
// Not ported: `SkRuntimeShaderImageFilter_CPU` and `SkRuntimeShaderImageFilter_GPU`
// (`SkImageFilters::RuntimeShader` is not in the port yet).

#![cfg(test)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas as CoreCanvas;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient_shader;
use skia_rust_effects::image_filters::{self, Dither};
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::tools::tool_utils::equal_pixels;
use crate::{Reporter, def_test, errorf, reporter_assert};

/// The radial gradient both tests draw: blue, red, green over a radius-5 circle at (5, 5).
fn make_gradient() -> skia_rust_core::shader::Shader {
    let center = Point::new(5.0, 5.0);
    let colors = [
        Color4f::from(Color::BLUE),
        Color4f::from(Color::RED),
        Color4f::from(Color::GREEN),
    ];
    let pos = [0.0_f32, 0.5, 1.0];
    gradient_shader::radial(
        center,
        5.0,
        &colors[..],
        Some(&pos[..]),
        TileMode::Clamp,
        None,
        None,
    )
    .expect("a radial gradient")
}

// Port of: tests/ShaderImageFilterTest.cpp#L39-L85 (chrome/m156)
fn test_unscaled(reporter: &mut Reporter) {
    const K_WIDTH: i32 = 10;
    const K_HEIGHT: i32 = 10;
    let ir = IRect::from_wh(K_WIDTH, K_HEIGHT);

    let mut filter_result = Bitmap::new();
    filter_result.alloc_n32_pixels((K_WIDTH, K_HEIGHT), None);
    let mut paint_result = Bitmap::new();
    paint_result.alloc_n32_pixels((K_WIDTH, K_HEIGHT), None);

    let gradient = make_gradient();

    // Test using the image filter
    {
        let canvas_filter = CoreCanvas::from_bitmap(&mut filter_result, None).expect("a canvas");
        canvas_filter.clear(Color::TRANSPARENT);
        let mut paint = Paint::default();
        paint.set_image_filter(image_filters::shader(
            Some(gradient.clone()),
            Dither::No,
            Some(Rect::from_irect(ir)),
        ));
        canvas_filter.draw_rect(Rect::from_irect(ir), &paint);
    }

    // Test using the paint directly
    {
        let canvas_paint = CoreCanvas::from_bitmap(&mut paint_result, None).expect("a canvas");
        canvas_paint.clear(Color::TRANSPARENT);
        let mut paint = Paint::default();
        paint.set_shader(gradient);
        canvas_paint.draw_rect(Rect::from_irect(ir), &paint);
    }

    // Assert that both paths yielded the same result
    for y in 0..K_HEIGHT {
        for x in 0..K_WIDTH {
            reporter_assert!(
                reporter,
                filter_result.get_addr32(x, y) == paint_result.get_addr32(x, y)
            );
        }
    }
}

// Port of: tests/ShaderImageFilterTest.cpp#L87-L125 (chrome/m156)
fn test_scaled(reporter: &mut Reporter) {
    const K_WIDTH: i32 = 10;
    const K_HEIGHT: i32 = 10;
    let ir = IRect::from_wh(K_WIDTH, K_HEIGHT);

    let mut filter_result = Bitmap::new();
    filter_result.alloc_n32_pixels((K_WIDTH, K_HEIGHT), None);
    let mut paint_result = Bitmap::new();
    paint_result.alloc_n32_pixels((K_WIDTH, K_HEIGHT), None);

    let gradient = make_gradient();

    // Test using the image filter
    {
        let canvas_filter = CoreCanvas::from_bitmap(&mut filter_result, None).expect("a canvas");
        canvas_filter.clear(Color::TRANSPARENT);
        let mut paint = Paint::default();
        paint.set_image_filter(image_filters::shader(
            Some(gradient.clone()),
            Dither::No,
            Some(Rect::from_irect(ir)),
        ));
        canvas_filter.scale((2.0, 2.0));
        canvas_filter.draw_rect(Rect::from_irect(ir), &paint);
    }

    // Test using the paint directly
    {
        let canvas_paint = CoreCanvas::from_bitmap(&mut paint_result, None).expect("a canvas");
        canvas_paint.clear(Color::TRANSPARENT);
        let mut paint = Paint::default();
        paint.set_shader(gradient);
        canvas_paint.scale((2.0, 2.0));
        canvas_paint.draw_rect(Rect::from_irect(ir), &paint);
    }

    // Assert that both paths yielded the same result
    if !equal_pixels(&filter_result.pixmap(), &paint_result.pixmap()) {
        errorf!(reporter, "Image filter doesn't match paint reference\n");
    }
}

// Port of: tests/ShaderImageFilterTest.cpp#L149-L152 (chrome/m156)
def_test!(ShaderImageFilter, |reporter| {
    test_unscaled(reporter);
    test_scaled(reporter);
});
