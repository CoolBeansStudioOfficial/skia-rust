// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/GrTextBlobTest.cpp (chrome/m156)
//
// Not ported: `BagOfBytesBasic` and `SubRunAllocator` test the arena allocators of the GPU text
// code (`excluded` in the manifest: Rust owns the sub run data), and the two Ganesh-only tests
// (`GrTextBlobScaleAnimation`, `GrTextBlobMoveAround`).

#![cfg(test)]

use skia_rust_core::font::Font;
use skia_rust_core::glyph_run::GlyphRunBuilder;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::scaler_context::ScalerContextBuildFlags;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::text_blob::TextBlobBuilder;
use skia_rust_gpu::text_gpu::sub_run_container::StrikeDeviceInfo;
use skia_rust_gpu::text_gpu::sub_run_control::SubRunControl;
use skia_rust_gpu::text_gpu::text_blob::TextBlobKey;
use skia_rust_tools::font_tool_utils::default_typeface;

use crate::{def_test, reporter_assert};

// Port of: tests/GrTextBlobTest.cpp#L356-L386 (chrome/m156)
def_test!(KeyEqualityOnPerspective, |r| {
    let mut builder = TextBlobBuilder::new();
    let font = Font::from_size(default_typeface(), 16.0);
    builder.alloc_run(&font, 1, 0.0, 0.0, None)[0] = 3;
    let blob = builder.make().expect("a text blob");
    let mut gr_builder = GlyphRunBuilder::new();
    let glyph_run_list = gr_builder.blob_to_glyph_run_list(&blob, Point::new(100.0, 100.0));
    let paint = Paint::default();

    // Build the strike device.
    let props = SurfaceProps::default();
    let control = SubRunControl::new(false, false, false, 1.0, 100.0, false, false);
    let strike_device = StrikeDeviceInfo {
        surface_props: props,
        scaler_context_flags: ScalerContextBuildFlags::BOOST_CONTRAST,
        sub_run_control: control,
    };
    let mut matrix1 = Matrix::default();
    matrix1.set_all(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 1.0);
    let mut matrix2 = Matrix::default();
    matrix2.set_all(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 2.0, 2.0, 1.0);
    let key1 = TextBlobKey::make(&glyph_run_list, &paint, &matrix1, &strike_device).1;
    let key2 = TextBlobKey::make(&glyph_run_list, &paint, &matrix1, &strike_device).1;
    let key3 = TextBlobKey::make(&glyph_run_list, &paint, &matrix2, &strike_device).1;
    reporter_assert!(r, key1 == key2);
    reporter_assert!(r, key1 == key3);
});
