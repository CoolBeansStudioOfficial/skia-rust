// Copyright 2020 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ConvertPixelsTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::convert_pixels::convert_pixels_in_place;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::size::ISize;
use skia_rust_raster::surfaces;

use crate::{def_tier_test, reporter_assert};

// Port of: tests/ConvertPixelsTest.cpp#L13-L47 (chrome/m156)
def_tier_test!(ConvertPixels_in_place, |r| {
    const TEST_SIZE: ISize = ISize {
        width: 256,
        height: 256,
    };
    const G_TEST_CTS: [ColorType; 11] = [
        ColorType::Alpha8,
        ColorType::RGB565,
        ColorType::ARGB4444,
        ColorType::RGBA8888,
        ColorType::BGRA8888,
        ColorType::RGBA1010102,
        ColorType::BGRA1010102,
        ColorType::Gray8,
        ColorType::RGBAF16Norm,
        ColorType::RGBAF16,
        ColorType::RGBAF32,
    ];

    let mut surface =
        surfaces::raster(&ImageInfo::new_n32_premul(TEST_SIZE, None), None, None).expect("surface");
    surface.canvas().draw_color(Color::GREEN, None);
    let image = surface.image_snapshot().expect("a snapshot");

    for src_ct in G_TEST_CTS {
        let src_info = ImageInfo::new(TEST_SIZE, src_ct, AlphaType::Premul, None);
        // `SkAutoPixmapStorage pm; pm.alloc(srcInfo)`
        let mut storage = vec![0_u8; src_info.compute_min_byte_size()];
        {
            let mut pm =
                Pixmap::new(&src_info, &mut storage, src_info.min_row_bytes()).expect("a pixmap");
            reporter_assert!(r, image.read_pixels_to_pixmap(&mut pm, (0, 0)));
        }

        for dst_ct in G_TEST_CTS {
            let dst_info = ImageInfo::new(TEST_SIZE, dst_ct, AlphaType::Premul, None);
            // Expected to succeed iff bpp matches.
            let should_succeed = src_info.bytes_per_pixel() == dst_info.bytes_per_pixel();
            reporter_assert!(
                r,
                convert_pixels_in_place(
                    &dst_info,
                    dst_info.min_row_bytes(),
                    &src_info,
                    src_info.min_row_bytes(),
                    &mut storage,
                ) == should_succeed
            );
        }
    }
});
