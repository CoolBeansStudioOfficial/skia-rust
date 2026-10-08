// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ImageFrom565Bitmap.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;

use crate::{def_test, reporter_assert};

// Port of: tests/ImageFrom565Bitmap.cpp#L18-L23 (chrome/m156)
def_test!(ImageFrom565Bitmap, |r| {
    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(
        &ImageInfo::new((5, 7), ColorType::RGB565, AlphaType::Opaque, None),
        None,
    );
    bm.erase_color(Color::BLACK);
    reporter_assert!(r, bm.as_image().is_some());
});
