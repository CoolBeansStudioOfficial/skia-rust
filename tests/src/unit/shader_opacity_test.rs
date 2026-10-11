// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ShaderOpacityTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shaders;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient_shader;

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/ShaderOpacityTest.cpp#L5-L38 (chrome/m156)
fn test_bitmap(reporter: &mut Reporter) {
    let info = ImageInfo::new_n32_premul((2, 2), None);
    let mut bmp = Bitmap::new();
    let _ = bmp.set_info(&info, None);
    // test 1: bitmap without pixel data
    let shader = bmp.to_shader(None, SamplingOptions::default(), None);
    reporter_assert!(reporter, shader.is_some());
    let shader = shader.expect("a bitmap shader");
    reporter_assert!(reporter, !shader.is_opaque());
    // From this point on, we have pixels
    bmp.alloc_pixels();
    // test 2: not opaque by default
    let shader = bmp.to_shader(None, SamplingOptions::default(), None);
    reporter_assert!(reporter, shader.is_some());
    let shader = shader.expect("a bitmap shader");
    reporter_assert!(reporter, !shader.is_opaque());
    // test 3: explicitly opaque
    bmp.set_alpha_type(AlphaType::Opaque);
    let shader = bmp.to_shader(None, SamplingOptions::default(), None);
    reporter_assert!(reporter, shader.is_some());
    let shader = shader.expect("a bitmap shader");
    reporter_assert!(reporter, shader.is_opaque());
    // test 4: explicitly not opaque
    bmp.set_alpha_type(AlphaType::Premul);
    let shader = bmp.to_shader(None, SamplingOptions::default(), None);
    reporter_assert!(reporter, shader.is_some());
    let shader = shader.expect("a bitmap shader");
    reporter_assert!(reporter, !shader.is_opaque());
}

// Port of: tests/ShaderOpacityTest.cpp#L40-L75 (chrome/m156)
fn test_gradient(reporter: &mut Reporter) {
    let pts = [Point::new(0.0, 0.0), Point::new(1.0, 0.0)];
    let pos = [0.0_f32, 1.0];
    let mut colors = [Color4f::default(); 2];

    // Builds the linear gradient of the current `colors`, as `SkShaders::LinearGradient(pts, gr)`.
    let make = |colors: &[Color4f; 2]| {
        gradient_shader::linear(
            (pts[0], pts[1]),
            &colors[..],
            Some(&pos[..]),
            TileMode::Clamp,
            None,
            None,
        )
    };

    // test 1: all opaque
    colors[0] = Color4f::from(Color::from_argb(0xFF, 0, 0, 0));
    colors[1] = Color4f::from(Color::from_argb(0xFF, 0, 0, 0));
    let grad = make(&colors);
    reporter_assert!(reporter, grad.is_some());
    reporter_assert!(reporter, grad.is_some_and(|g| g.is_opaque()));

    // test 2: all 0 alpha
    colors[0] = Color4f::from(Color::from_argb(0, 0, 0, 0));
    colors[1] = Color4f::from(Color::from_argb(0, 0, 0, 0));
    let grad = make(&colors);
    reporter_assert!(reporter, grad.is_some());
    reporter_assert!(reporter, grad.is_some_and(|g| !g.is_opaque()));

    // test 3: one opaque, one transparent
    colors[0] = Color4f::from(Color::from_argb(0xFF, 0, 0, 0));
    colors[1] = Color4f::from(Color::from_argb(0x40, 0, 0, 0));
    let grad = make(&colors);
    reporter_assert!(reporter, grad.is_some());
    reporter_assert!(reporter, grad.is_some_and(|g| !g.is_opaque()));

    // test 4: test 3, swapped
    colors[0] = Color4f::from(Color::from_argb(0x40, 0, 0, 0));
    colors[1] = Color4f::from(Color::from_argb(0xFF, 0, 0, 0));
    let grad = make(&colors);
    reporter_assert!(reporter, grad.is_some());
    reporter_assert!(reporter, grad.is_some_and(|g| !g.is_opaque()));
}

// Port of: tests/ShaderOpacityTest.cpp#L77-L85 (chrome/m156)
fn test_color(reporter: &mut Reporter) {
    let color_shader1 = shaders::color(Color::from_argb(0, 0, 0, 0));
    reporter_assert!(reporter, !color_shader1.is_opaque());
    let color_shader2 = shaders::color(Color::from_argb(0xFF, 0, 0, 0));
    reporter_assert!(reporter, color_shader2.is_opaque());
    let color_shader3 = shaders::color(Color::from_argb(0x7F, 0, 0, 0));
    reporter_assert!(reporter, !color_shader3.is_opaque());
}

// Port of: tests/ShaderOpacityTest.cpp#L87-L109 (chrome/m156)
fn test_matrix(reporter: &mut Reporter) {
    let color_shader1 = shaders::color(Color::from_argb(0, 0, 0, 0));
    reporter_assert!(reporter, !color_shader1.is_opaque());
    let color_shader2 = shaders::color(Color::from_argb(0xFF, 0, 0, 0));
    reporter_assert!(reporter, color_shader2.is_opaque());

    let k_matrix = Matrix::scale((1.0, 1.0));
    let lm1 = color_shader1.with_local_matrix(&k_matrix);
    reporter_assert!(reporter, !lm1.is_opaque());
    let lm2 = color_shader2.with_local_matrix(&k_matrix);
    reporter_assert!(reporter, lm2.is_opaque());

    let ctm1 = color_shader1.make_with_ctm(&k_matrix);
    reporter_assert!(reporter, !ctm1.is_opaque());
    let ctm2 = color_shader2.make_with_ctm(&k_matrix);
    reporter_assert!(reporter, ctm2.is_opaque());
}

// Port of: tests/ShaderOpacityTest.cpp#L111-L117 (chrome/m156)
def_test!(ShaderOpacity, |reporter| {
    test_gradient(reporter);
    test_color(reporter);
    test_bitmap(reporter);
    test_matrix(reporter);
});
