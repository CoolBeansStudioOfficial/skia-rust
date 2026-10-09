// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/ImageShaderTest.cpp (chrome/m156)

#![cfg(test)]
// Mirrors the C++ test, which converts the float positions to pixel coordinates.
#![allow(clippy::cast_possible_truncation)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::RequiredProperties;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shaders::image_shader::ImageShader;
use skia_rust_core::size::ISize;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::image_factories::texture_from_image;
use skia_rust_gpu::graphite::surface_graphite::Surface;
use skia_rust_gpu::graphite::wgpu::WgpuContext;

use crate::{Reporter, def_graphite_adapter_test, reporter_assert};

const K_RECT_COLOR: Color4f = Color4f::new(1.0, 0.0, 0.0, 1.0);
const K_BG_COLOR: Color4f = Color4f::new(0.0, 0.0, 0.0, 0.0);

struct Expectation {
    pos: Point,
    color: Color4f,
}

// Port of: tests/graphite/ImageShaderTest.cpp#L26-L74 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn test_draw(
    reporter: &mut Reporter,
    context: &mut WgpuContext,
    canvas_size: ISize,
    image_size: ISize,
    src_rect: Rect,
    dst_rect: Rect,
    tile_mode: TileMode,
    sampling_options: SamplingOptions,
    expectations: &[Expectation],
) {
    let recorder = context.make_recorder(None);
    let surface = Surface::render_target(
        &recorder,
        &ImageInfo::new(canvas_size, ColorType::RGBA8888, AlphaType::Premul, None),
        Mipmapped::No,
        None,
        "",
    );
    reporter_assert!(reporter, surface.is_some());
    let Some(surface) = surface else {
        return;
    };
    let canvas = surface.canvas();

    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(
        &ImageInfo::new(image_size, ColorType::RGBA8888, AlphaType::Premul, None),
        0,
    );
    bitmap.erase_color_4f(K_RECT_COLOR);
    bitmap.set_immutable();
    let image = texture_from_image(
        &recorder,
        &bitmap.as_image().expect("an image"),
        RequiredProperties { mipmapped: false },
    );

    let mut p = Paint::default();
    let src_to_dst = Matrix::rect_to_rect_or_identity(src_rect, dst_rect, None);
    p.set_shader(ImageShader::make_subset(
        image,
        &src_rect,
        tile_mode,
        tile_mode,
        &sampling_options,
        Some(&src_to_dst),
        false,
    ));
    canvas.draw_rect(dst_rect, &p);

    let mut result = Bitmap::new();
    result.alloc_pixels_info(
        &ImageInfo::new(canvas_size, ColorType::RGBA8888, AlphaType::Premul, None),
        None,
    );
    let pm = result.peek_pixels_mut();
    reporter_assert!(reporter, pm.is_some());
    let Some(mut pm) = pm else {
        return;
    };

    let read_pixels_success = context.read_surface_pixels(&surface, &mut pm, 0, 0);
    reporter_assert!(reporter, read_pixels_success);

    for e in expectations {
        let a = e.color;
        let b = pm.get_color_4f((e.pos.x as i32, e.pos.y as i32));
        reporter_assert!(
            reporter,
            a == b,
            "At position {{{:.1}, {:.1}}}, expected {{{:.1}, {:.1}, {:.1}, {:.1}}}, found {{{:.1}, {:.1}, {:.1}, {:.1}}}",
            e.pos.x,
            e.pos.y,
            a.r,
            a.g,
            a.b,
            a.a,
            b.r,
            b.g,
            b.b,
            b.a
        );
    }
}

// Port of: tests/graphite/ImageShaderTest.cpp#L78-L107 (chrome/m156)
def_graphite_adapter_test!(ImageShaderTest, |reporter, context| {
    // Test that a subset bound covering less than half of a pixel causes that pixel not to be
    // drawn when using decal tiling and nearest-neighbor filtering. In this case we have a subset
    // that covers 3/4 the pixel column at y=1, all of the y=2 column, and 1/4 the y=3 column.
    test_draw(
        reporter,
        context,
        /* canvas_size= */ ISize::new(100, 100),
        /* image_size= */ ISize::new(4, 4),
        /* src_rect= */ Rect::new(1.25, 0.0, 3.25, 2.0),
        /* dst_rect= */ Rect::new(0.0, 0.0, 80.0, 80.0),
        TileMode::Decal,
        SamplingOptions::default(),
        // Pixel that should sample the image at y=1, since that's where the subset starts.
        &[
            Expectation {
                pos: Point::new(0.0, 40.0),
                color: K_RECT_COLOR,
            },
            // Pixel that would sample the image at y=3, but the subset bound at y=3.25 prevents
            // us from sampling the image.
            Expectation {
                pos: Point::new(75.0, 40.0),
                color: K_BG_COLOR,
            },
        ],
    );
});
