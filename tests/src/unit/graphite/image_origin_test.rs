// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/ImageOriginTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{Canvas, SrcRectConstraint};
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shaders::image_shader::ImageShader;
use skia_rust_core::size::ISize;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_gpu::gpu::gpu_types::{Mipmapped, Origin, Protected, Renderable};
use skia_rust_gpu::graphite::context_priv::ContextPriv;
use skia_rust_gpu::graphite::image_factories::wrap_texture_for_alpha_type;
use skia_rust_gpu::graphite::surface_graphite::Surface;
use skia_rust_gpu::graphite::wgpu::WgpuContext;

use crate::tools::managed_graphite_texture::ManagedGraphiteTexture;
use crate::{Reporter, def_graphite_adapter_test, reporter_assert};

const K_TOP_COLOR: Color = Color::RED;
const K_BOTTOM_COLOR: Color = Color::BLUE;
const K_HALF_SIZE: i32 = 4;
const K_IMAGE_SIZE: ISize = ISize::new(2 * K_HALF_SIZE, 2 * K_HALF_SIZE);

type DrawFn = fn(&Image, &Canvas, Rect, Rect);

// Port of: tests/graphite/ImageOriginTest.cpp#L26-L87 (chrome/m156)
fn test_draw(
    reporter: &mut Reporter,
    context: &mut WgpuContext,
    origin: Origin,
    src_rect: Rect,
    dst_rect: Rect,
    draw_image_fn: DrawFn,
) {
    let mut recorder = context.make_recorder(None);
    let is_protected = if ContextPriv::caps(context).protected_support() {
        Protected::Yes
    } else {
        Protected::No
    };
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(
        &ImageInfo::new(K_IMAGE_SIZE, ColorType::RGBA8888, AlphaType::Premul, None),
        None,
    );
    bitmap.erase_color(K_TOP_COLOR);
    bitmap.erase(
        K_BOTTOM_COLOR,
        IRect::from_ltrb(0, K_HALF_SIZE, K_IMAGE_SIZE.width, K_IMAGE_SIZE.height),
    );

    let managed_texture = ManagedGraphiteTexture::make_from_pixmap(
        context,
        &mut recorder,
        &bitmap.pixmap(),
        Mipmapped::No,
        Renderable::No,
        is_protected,
    );
    reporter_assert!(reporter, managed_texture.is_some());
    let Some(managed_texture) = managed_texture else {
        return;
    };

    let image = wrap_texture_for_alpha_type(
        &recorder,
        managed_texture.texture(),
        AlphaType::Premul,
        None,
        origin,
        None,
    );
    reporter_assert!(reporter, image.is_some());
    let Some(image) = image else {
        return;
    };

    let surface = Surface::render_target(
        &recorder,
        &ImageInfo::new(K_IMAGE_SIZE, ColorType::RGBA8888, AlphaType::Premul, None),
        Mipmapped::No,
        None,
        "",
    );
    reporter_assert!(reporter, surface.is_some());
    let Some(surface) = surface else {
        return;
    };
    let canvas = surface.canvas();
    draw_image_fn(&image, canvas, src_rect, dst_rect);

    let mut result = Bitmap::new();
    result.alloc_pixels_info(
        &ImageInfo::new(K_IMAGE_SIZE, ColorType::RGBA8888, AlphaType::Premul, None),
        None,
    );
    let pm = result.peek_pixels_mut();
    reporter_assert!(reporter, pm.is_some());
    let Some(mut pm) = pm else {
        return;
    };
    let read_pixels_success = context.read_surface_pixels(&surface, &mut pm, 0, 0);
    reporter_assert!(reporter, read_pixels_success);

    let result_top_color_on_top = origin == Origin::TopLeft;
    for y in 0..K_IMAGE_SIZE.height {
        for x in 0..K_IMAGE_SIZE.width {
            let color = pm.get_color_4f(IPoint::new(x, y));
            let expected_color = if (y < K_HALF_SIZE) == result_top_color_on_top {
                Color4f::from_color(K_TOP_COLOR)
            } else {
                Color4f::from_color(K_BOTTOM_COLOR)
            };
            reporter_assert!(
                reporter,
                color == expected_color,
                "At position {{{x}, {y}}}, expected {{{:.1}, {:.1}, {:.1}, {:.1}}}, found {{{:.1}, {:.1}, {:.1}, {:.1}}}",
                expected_color.r,
                expected_color.g,
                expected_color.b,
                expected_color.a,
                color.r,
                color.g,
                color.b,
                color.a
            );
        }
    }
}

// Port of: tests/graphite/ImageOriginTest.cpp#L100-L105 (chrome/m156), `kTestSrcRects`
fn test_src_rects() -> [Rect; 2] {
    [
        // entire thing
        Rect::from_wh(K_IMAGE_SIZE.width as f32, K_IMAGE_SIZE.height as f32),
        // half rect still splitting top and bottom colors
        Rect::from_xywh(2.0, 2.0, K_HALF_SIZE as f32, K_HALF_SIZE as f32),
    ]
}

// Port of: tests/graphite/ImageOriginTest.cpp#L107-L120 (chrome/m156)
fn test_draw_fn(reporter: &mut Reporter, context: &mut WgpuContext, draw_image_fn: DrawFn) {
    for origin in [Origin::TopLeft, Origin::BottomLeft] {
        for src_rect in test_src_rects() {
            test_draw(
                reporter,
                context,
                origin,
                src_rect,
                Rect::from_wh(K_IMAGE_SIZE.width as f32, K_IMAGE_SIZE.height as f32),
                draw_image_fn,
            );
        }
    }
}

// Port of: tests/graphite/ImageOriginTest.cpp#L122-L131 (chrome/m156)
fn draw_image(image: &Image, canvas: &Canvas, src_rect: Rect, dst_rect: Rect) {
    canvas.draw_image_rect_with_sampling_options(
        image,
        Some((&src_rect, SrcRectConstraint::Strict)),
        dst_rect,
        SamplingOptions::default(),
        &Paint::default(),
    );
}

// Port of: tests/graphite/ImageOriginTest.cpp#L133-L145 (chrome/m156)
fn draw_image_with_shader(image: &Image, canvas: &Canvas, src_rect: Rect, dst_rect: Rect) {
    let mut p = Paint::default();
    let src_to_dst = Matrix::rect_to_rect_or_identity(src_rect, dst_rect, None);
    p.set_shader(ImageShader::make_subset(
        Some(image.clone()),
        &src_rect,
        TileMode::Clamp,
        TileMode::Clamp,
        &SamplingOptions::default(),
        Some(&src_to_dst),
        false,
    ));
    canvas.draw_rect(dst_rect, &p);
}

// Port of: tests/graphite/ImageOriginTest.cpp#L171-L174 (chrome/m156)
def_graphite_adapter_test!(ImageOriginTest_drawImage_Graphite, |reporter, context| {
    test_draw_fn(reporter, context, draw_image);
});

// Port of: tests/graphite/ImageOriginTest.cpp#L176-L179 (chrome/m156)
def_graphite_adapter_test!(ImageOriginTest_imageShader_Graphite, |reporter, context| {
    test_draw_fn(reporter, context, draw_image_with_shader);
});
