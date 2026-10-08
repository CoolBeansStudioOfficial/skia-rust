// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ImageBitmapTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color;
use skia_rust_core::rect::IRect;

use crate::{def_tier_test, reporter_assert};

// skbug.com/40036261
// Test that when we make an image from a subset of a bitmap, that it
// has a diff (ID, dimensions) from an image made from the entire
// bitmap or a different subset of the image.
// Port of: tests/ImageBitmapTest.cpp#L16-L33 (chrome/m156)
def_tier_test!(ImageBitmapIdentity, |reporter| {
    let mut bm = Bitmap::new();
    let mut a = Bitmap::new();
    let mut b = Bitmap::new();
    bm.alloc_n32_pixels((32, 64), None);
    bm.erase_color(Color::BLACK);
    bm.set_immutable();
    let _ = bm.extract_subset(&mut a, IRect::from_xywh(0, 0, 32, 32));
    let _ = bm.extract_subset(&mut b, IRect::from_xywh(0, 32, 32, 32));
    reporter_assert!(reporter, a.generation_id() == b.generation_id());
    let img = bm.as_image().expect("an image");
    let img_a = a.as_image().expect("an image");
    let img_b = b.as_image().expect("an image");
    reporter_assert!(reporter, img.unique_id() == bm.generation_id());
    reporter_assert!(reporter, img.unique_id() != img_a.unique_id());
    reporter_assert!(reporter, img.unique_id() != img_b.unique_id());
    reporter_assert!(reporter, img_a.unique_id() != img_b.unique_id());
    reporter_assert!(reporter, img_a.unique_id() != a.generation_id());
    reporter_assert!(reporter, img_b.unique_id() != b.generation_id());
});
