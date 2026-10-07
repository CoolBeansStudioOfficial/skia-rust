// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ReadPixelsTest.cpp (chrome/m156)
//
// Not ported (no `SkImage` yet, Phase 3):
// * `ReadPixels`: `fill_src_canvas` draws the source with `SkCanvas::drawImage` of an
//   `SkBitmap::asImage()` image (`SkBlendMode::kSrc`); the readPixels half is covered by the
//   unit test `read_pixels_matches_the_surface` in `skia-rust-raster`'s `canvas_tests.rs`.
// * `ReadPixels_ValidConversion`: converts through `SkImages::RasterFromPixmap` and
//   `SkImage::readPixels`.

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_raster::surfaces;

use crate::{def_tier_test, reporter_assert};

// Port of: tests/ReadPixelsTest.cpp#L541-L555 (chrome/m156)
def_tier_test!(ReadPixels_InvalidRowBytes, |reporter| {
    let src_ii = ImageInfo::new((10, 10), ColorType::RGBA8888, AlphaType::Premul, None);
    let mut surf = surfaces::raster(&src_ii, None, None).expect("surface");
    #[allow(clippy::cast_possible_wrap)] // LAST_ENUM is small
    for ct in 0..=(ColorType::LAST_ENUM as i32) {
        let color_type = ColorType::from_i32(ct).expect("valid color type");
        let bpp = color_type.bytes_per_pixel();
        if bpp <= 1 {
            continue;
        }
        let dst_ii = src_ii.with_color_type(color_type);
        #[allow(clippy::cast_sign_loss)] // width and height are positive
        let (bad_row_bytes, height) = (
            (surf.width() + 1) as usize * bpp - 1,
            surf.height() as usize,
        );
        let mut storage = vec![0_u8; bad_row_bytes * height];
        reporter_assert!(
            reporter,
            !surf.read_pixels(&dst_ii, &mut storage, bad_row_bytes, (0, 0))
        );
    }
});
