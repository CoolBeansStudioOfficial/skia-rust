// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ShaderImageFilterTest.cpp (chrome/m156)
//
// Not ported: `SkRuntimeShaderImageFilter_GPU` (it needs a GPU context).

#![cfg(test)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{Canvas as CoreCanvas, SaveLayerRec};
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::runtime_effect::{RuntimeEffect, RuntimeShaderBuilder};
use skia_rust_core::shaders;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient_shader;
use skia_rust_effects::image_filters::{self, Dither, runtime_shader};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

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

// Port of: tests/ShaderImageFilterTest.cpp#L154-L211 (chrome/m156), test_runtime_shader
fn test_runtime_shader(reporter: &mut Reporter, surface: &mut Surface<'_>) {
    let effect = RuntimeEffect::make_for_shader(
        r"
        uniform shader child;
        vec4 main(vec2 coord) {
            return child.eval(coord) * 0.5;
        }
    ",
        None,
    )
    .expect("a runtime effect");
    let builder = RuntimeShaderBuilder::new(effect);

    // create a red image filter to feed as input into the SkImageFilters::RuntimeShader
    let input = image_filters::shader(Some(shaders::color(Color::RED)), Dither::No, None);

    // Create the different variations of SkImageFilters::RuntimeShader
    // All variations should produce the same pixel output
    let filters = [
        runtime_shader(&builder, 0.0, "", input.clone(), false),
        runtime_shader(&builder, 0.0, "child", input, false),
    ];

    for filter in filters {
        let canvas = surface.canvas();

        // clear to transparent
        let mut paint = Paint::default();
        paint.set_color(Color::TRANSPARENT);
        paint.set_blend_mode(BlendMode::Src);
        canvas.draw_paint(&paint);

        let mut filter_paint = Paint::default();
        // the green color will be ignored by the filter within the runtime shader
        filter_paint.set_color(Color::GREEN);
        filter_paint.set_image_filter(filter);
        canvas.save_layer(&SaveLayerRec::default().paint(&filter_paint));
        // the blue color will be ignored by the filter because the input to the image filter is not
        // null
        canvas.draw_color(Color4f::from(Color::BLUE), BlendMode::SrcOver);
        canvas.restore();

        // This is expected to read back the half transparent red pixel produced by the image filter.
        // skia-rust: the readback goes to a byte buffer of the surface's size (`SkBitmap` keeps its
        // pixels private), which is then read as a pixmap.
        let info = surface.image_info();
        let row_bytes = info.min_row_bytes();
        let mut pixels = vec![0_u8; info.compute_byte_size(row_bytes)];
        reporter_assert!(
            reporter,
            surface.read_pixels(&info, &mut pixels, row_bytes, (0, 0))
        );
        let pixmap = Pixmap::new(&info, &mut pixels, row_bytes).expect("a pixmap");
        let color = pixmap.get_color((0, 0));

        // check alpha with a small tolerance
        let alpha = color.a();
        reporter_assert!(
            reporter,
            (127..=129).contains(&alpha),
            "Expected: {} Actual: {}",
            128,
            alpha
        );

        // check each color channel
        let color = color.with_a(255);
        reporter_assert!(
            reporter,
            Color::RED == color,
            "Expected: {:?} Actual: {:?}",
            Color::RED,
            color
        );
    }
}

// Port of: tests/ShaderImageFilterTest.cpp#L213-L217 (chrome/m156), SkRuntimeShaderImageFilter_CPU
def_test!(SkRuntimeShaderImageFilter_CPU, |reporter| {
    let info = ImageInfo::new_n32_premul((1, 1), None);
    let mut surface = surfaces::raster(&info, None, None).expect("a surface");
    test_runtime_shader(reporter, &mut surface);
});

// Port of: tests/ShaderImageFilterTest.cpp#L149-L152 (chrome/m156)
def_test!(ShaderImageFilter, |reporter| {
    test_unscaled(reporter);
    test_scaled(reporter);
});
