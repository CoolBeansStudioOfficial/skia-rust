// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkImageTest.cpp (chrome/m156)

#![cfg(test)]

use std::collections::BTreeMap;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::RequiredProperties;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::IRect;
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::{def_tier_test, reporter_assert};

const G_WIDTH: i32 = 20;
const G_HEIGHT: i32 = 20;

// Tests that SkNewImageFromBitmap obeys pixelref origin.
// Port of: tests/SkImageTest.cpp#L28-L63 (chrome/m156)
def_tier_test!(SkImageFromBitmap_extractSubset, |reporter| {
    let image;
    {
        let mut src_bitmap = Bitmap::new();
        src_bitmap.alloc_n32_pixels((G_WIDTH, G_HEIGHT), None);
        src_bitmap.erase_color(Color::RED);
        let r = IRect::from_xywh(5, 5, G_WIDTH - 5, G_WIDTH - 5);
        {
            let canvas = Canvas::from_bitmap(&mut src_bitmap, None).expect("a canvas");
            let mut p = Paint::default();
            p.set_color(Color::GREEN);
            canvas.draw_irect(r, &p);
        }
        let mut dst_bitmap = Bitmap::new();
        let _ = src_bitmap.extract_subset(&mut dst_bitmap, r);
        image = dst_bitmap.as_image().expect("an image");
    }

    let mut tgt = Bitmap::new();
    tgt.alloc_n32_pixels((G_WIDTH, G_HEIGHT), None);
    {
        let canvas = Canvas::from_bitmap(&mut tgt, None).expect("a canvas");
        canvas.clear(Color::TRANSPARENT);
        canvas.draw_image(&image, (0.0, 0.0), None);
    }

    let mut pixel = [0_u8; 4];
    let info = ImageInfo::new((1, 1), ColorType::BGRA8888, AlphaType::Unpremul, None);
    let _ = tgt.read_pixels(&info, &mut pixel, 4, 0, 0);
    reporter_assert!(
        reporter,
        u32::from_ne_bytes(pixel) == u32::from(Color::GREEN)
    );
    let _ = tgt.read_pixels(&info, &mut pixel, 4, G_WIDTH - 6, G_WIDTH - 6);
    reporter_assert!(
        reporter,
        u32::from_ne_bytes(pixel) == u32::from(Color::GREEN)
    );

    let _ = tgt.read_pixels(&info, &mut pixel, 4, G_WIDTH - 5, G_WIDTH - 5);
    reporter_assert!(
        reporter,
        u32::from_ne_bytes(pixel) == u32::from(Color::TRANSPARENT)
    );
});

// This makes sure we can use RequiredProperties as a key in a map, which some
// clients depend on.
// Port of: tests/SkImageTest.cpp#L65-L71 (chrome/m156)
def_tier_test!(SkImageRequiredPropertiesCanBeMapKey, |_r| {
    let mut test: BTreeMap<RequiredProperties, i32> = BTreeMap::new();
    let rp = RequiredProperties::default();
    test.insert(rp, 7);
});
