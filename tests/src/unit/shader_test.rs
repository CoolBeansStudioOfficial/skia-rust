// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ShaderTest.cpp (chrome/m156)
//
// The Ganesh variant of `ShaderTestNestedBlends` needs that backend and is not ported.

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Vector;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::runtime_effect::{ChildPtr, RuntimeEffect};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::perlin_noise_shader::shaders as perlin_shaders;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::surface_graphite::Surface as GraphiteSurface;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;

use crate::tools::test_surface::{GraphiteTestSurface, TestSurface};
use crate::{Reporter, def_graphite_adapter_test, def_test, errorf, reporter_assert};

// Port of: tests/ShaderTest.cpp#L55-L73 (chrome/m156)
fn check_isaimage(
    reporter: &mut Reporter,
    shader: &Shader,
    expected_w: i32,
    expected_h: i32,
    expected_x: TileMode,
    expected_y: TileMode,
    expected_m: &Matrix,
) {
    // (The C++ whacks the out-parameters first "so we don't get a false positive"; here they are
    // return values.)
    let is_a_image = shader.is_a_image();
    reporter_assert!(reporter, is_a_image.is_some());
    let Some((image, local_m, tile_modes)) = is_a_image else {
        return;
    };
    reporter_assert!(reporter, image.width() == expected_w);
    reporter_assert!(reporter, image.height() == expected_h);
    reporter_assert!(reporter, local_m == *expected_m);
    reporter_assert!(reporter, tile_modes.0 == expected_x);
    reporter_assert!(reporter, tile_modes.1 == expected_y);
}

// Port of: tests/ShaderTest.cpp#L75-L90 (chrome/m156)
def_test!(Shader_isAImage, |reporter| {
    const W: i32 = 100;
    const H: i32 = 100;
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((W, H), None);
    let local_m = Matrix::scale((2.0, 3.0));
    let tmx = TileMode::Repeat;
    let tmy = TileMode::Mirror;

    let shader0 = bm
        .to_shader((tmx, tmy), SamplingOptions::default(), &local_m)
        .expect("a shader");
    let shader1 = bm
        .as_image()
        .expect("an image")
        .to_shader((tmx, tmy), SamplingOptions::default(), &local_m)
        .expect("a shader");

    check_isaimage(reporter, &shader0, W, H, tmx, tmy, &local_m);
    check_isaimage(reporter, &shader1, W, H, tmx, tmy, &local_m);
});

// Port of: tests/ShaderTest.cpp#L93-L106 (chrome/m156)
def_test!(ComposeShaderSingle, |_reporter| {
    // Make sure things are ok with just a single leg.
    let mut src_bitmap = Bitmap::new();
    src_bitmap.alloc_n32_pixels((10, 10), None);
    src_bitmap.erase_color(Color::RED);
    let canvas = Canvas::from_bitmap(&mut src_bitmap, None).expect("canvas");
    let mut p = Paint::default();
    let noise = perlin_shaders::fractal_noise((1.0, 1.0), 2, 0.0, None).expect("a shader");
    p.set_shader(shaders::blend(BlendMode::Clear, shaders::empty(), noise));
    let mut rr = RRect::new();
    let rd = [Vector::new(0.0, 0.0); 4];
    rr.set_rect_radii(Rect::new(0.0, 0.0, 0.0, 0.0), &rd);
    canvas.draw_rrect(rr, &p);
});

/// Tests that nested blending will render as expected.
// Port of: tests/ShaderTest.cpp#L108-L176 (chrome/m156)
fn test_nested_blends(reporter: &mut Reporter, surface: &mut dyn TestSurface) {
    let red_effect = RuntimeEffect::make_for_shader(
        "half4 main(float2 coord) {
            return half4(1, 0, 0, 1);
        }",
        None,
    )
    .expect("the red effect compiles");

    let green_effect = RuntimeEffect::make_for_shader(
        "half4 main(float2 coord) {
            return half4(0, 1, 0, 1);
        }",
        None,
    )
    .expect("the green effect compiles");

    let blend_effect = RuntimeEffect::make_for_blender(
        "half4 main(half4 src, half4 dst) {
            return (src + dst) * 0.5;
        }",
        None,
    )
    .expect("the blend effect compiles");

    let nested_blend_effect = RuntimeEffect::make_for_blender(
        "uniform blender child_blender;
        half4 main(half4 src, half4 dst) {
            return (child_blender.eval(src, dst) + dst) * 0.5;
        }",
        None,
    )
    .expect("the nested blend effect compiles");

    let red_shader = red_effect
        .make_shader(Data::new_empty(), &[], None)
        .expect("a red shader");
    let green_shader = green_effect
        .make_shader(Data::new_empty(), &[], None)
        .expect("a green shader");
    let blender = blend_effect
        .make_blender(Data::new_empty(), &[])
        .expect("a blender");
    let children = [ChildPtr::Blender(blender.clone())];
    let nested_blender = nested_blend_effect
        .make_blender(Data::new_empty(), &children)
        .expect("a nested blender");

    let mut paint = Paint::default();
    paint.set_shader(shaders::blend_blender(
        &nested_blender,
        green_shader,
        red_shader,
    ));
    paint.set_blender(blender);

    // Do the drawing.
    surface.canvas().draw_paint(&paint);

    // Read pixels.
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(&surface.image_info(), None);
    if !surface.read_pixels(&mut bitmap) {
        errorf!(reporter, "readPixels failed");
        return;
    }
    let Some(pixmap) = bitmap.peek_pixels() else {
        return;
    };

    // Check the resulting blended color.
    // First, in the paint's shader, red and green are averaged in the child blender to get
    // (0.5, 0.5, 0, 1), which is then averaged with green in the parent blender to get
    // (0.25, 0.75, 0, 1). Then, in the paint's blender this is averaged with a transparent
    // background to get (0.125, 0.375, 0, 0.5) and then unpremuled to get (0.25, 0.75, 0, 0.5).
    let k_expected: [f32; 4] = [0.25, 0.75, 0.0, 0.5];
    let k_tolerance: [f32; 4] = [0.01, 0.01, 0.0, 0.01];
    let color = pixmap.get_color_4f((0, 0));
    let actual = [color.r, color.g, color.b, color.a];
    for i in 0..4 {
        if (actual[i] - k_expected[i]).abs() > k_tolerance[i] {
            errorf!(
                reporter,
                "Wrong color, expected ({:.2} {:.2} {:.2} {:.2}), actual ({:.2}, {:.2}, {:.2}, {:.2})",
                k_expected[0],
                k_expected[1],
                k_expected[2],
                k_expected[3],
                actual[0],
                actual[1],
                actual[2],
                actual[3]
            );
            break;
        }
    }
}

// Port of: tests/ShaderTest.cpp#L178-L184 (chrome/m156)
def_test!(ShaderTestNestedBlendsCpu, |reporter| {
    let info = ImageInfo::new((1, 1), ColorType::RGBA8888, AlphaType::Premul, None);
    let mut surface = surfaces::raster(&info, None, None).expect("a raster surface");
    test_nested_blends(reporter, &mut surface);
});

// Port of: tests/ShaderTest.cpp#L200-L212 (chrome/m156)
def_graphite_adapter_test!(ShaderTestNestedBlendsGraphite, |reporter, context| {
    let info = ImageInfo::new((1, 1), ColorType::RGBA8888, AlphaType::Premul, None);
    let recorder = context.make_recorder(None);
    let surface = GraphiteSurface::render_target(&recorder, &info, Mipmapped::No, None, "");
    reporter_assert!(reporter, surface.is_some());
    let Some(surface) = surface else {
        return;
    };
    test_nested_blends(
        reporter,
        &mut GraphiteTestSurface {
            context,
            surface: &surface,
        },
    );
});
