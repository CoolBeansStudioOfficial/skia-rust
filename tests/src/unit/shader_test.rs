// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ShaderTest.cpp (chrome/m156)
//
// Not ported yet:
// * `ComposeShaderSingle`: `SkShaders::MakeFractalNoise` (perlin noise shader).
// * `ShaderTestNestedBlendsCpu`, `ShaderTestNestedBlendsGraphite`: `SkRuntimeEffect`.

#![cfg(test)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;

use crate::{Reporter, def_test, reporter_assert};

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
