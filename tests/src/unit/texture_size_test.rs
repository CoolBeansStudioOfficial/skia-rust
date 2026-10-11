// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/TextureSizeTest.cpp (chrome/m156)
//
// Only the Graphite test (`ImageSizeTest_Graphite`) is ported: the Ganesh one tests a backend that
// the port does not have.

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::{Image, RequiredProperties};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::image_factories::texture_from_image;
use skia_rust_gpu::graphite::recorder::Recorder;

use crate::{def_graphite_test_for_all_contexts, reporter_assert};

// Port of: tests/TextureSizeTest.cpp#L35-L63 (chrome/m156), the anonymous `run_test`
fn run_test(
    reporter: &mut crate::Reporter,
    test_mipmaps: bool,
    convert2gpu: impl Fn(&Image, Mipmapped) -> Option<Image>,
) {
    for mm in [Mipmapped::Yes, Mipmapped::No] {
        if !test_mipmaps && mm == Mipmapped::Yes {
            continue;
        }
        for ct in [ColorType::RGBA8888, ColorType::Alpha8] {
            let ii = ImageInfo::new((9, 9), ct, AlphaType::Premul, None);
            let mut src = Bitmap::new();
            src.alloc_pixels_info(&ii, None);
            src.erase_color(Color::WHITE);
            let raster = src.as_image().expect("a raster image of the bitmap");
            let gpu = convert2gpu(&raster, mm).expect("a texture image");
            let bytes_per_pixel = ct.bytes_per_pixel();
            let width = usize::try_from(gpu.width()).expect("a positive width");
            let height = usize::try_from(gpu.height()).expect("a positive height");
            let mut expected_size = bytes_per_pixel * width * height;
            if mm == Mipmapped::Yes {
                expected_size += expected_size / 3;
            }
            let actual_size = gpu.texture_size();
            reporter_assert!(
                reporter,
                actual_size == expected_size,
                "Expected: {}  Actual: {}",
                expected_size,
                actual_size
            );
        }
    }
}

// Port of: tests/TextureSizeTest.cpp#L93-L103 (chrome/m156)
def_graphite_test_for_all_contexts!(ImageSizeTest_Graphite, |reporter, context| {
    let recorder: Recorder = context.make_recorder(None);
    run_test(reporter, /* test_mipmaps= */ true, |src, mm| {
        texture_from_image(
            &recorder,
            src,
            RequiredProperties {
                mipmapped: mm == Mipmapped::Yes,
            },
        )
    });
});
