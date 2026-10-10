// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ImageGeneratorTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::rect::Rect;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_raster::image_picture::{BitDepth, make_from_picture};

use crate::{def_test, reporter_assert};

// Port of: tests/ImageGeneratorTest.cpp#L97-L102 (chrome/m156), make_picture
fn make_picture() -> Picture {
    let mut recorder = PictureRecorder::new();
    recorder
        .begin_recording(Rect::from_wh(100.0, 100.0), false)
        .draw_color(Color::RED, None);
    recorder
        .finish_recording_as_picture(None)
        .expect("a picture")
}

// Port of: tests/ImageGeneratorTest.cpp#L104-L134 (chrome/m156), PictureImageGenerator
def_test!(PictureImageGenerator, |reporter| {
    let recs = [
        (ColorType::RGBA8888, AlphaType::Premul),
        (ColorType::BGRA8888, AlphaType::Premul),
        (ColorType::RGBAF16, AlphaType::Premul),
        (ColorType::RGBAF32, AlphaType::Premul),
        (ColorType::RGBA1010102, AlphaType::Premul),
        (ColorType::RGBA8888, AlphaType::Unpremul),
        (ColorType::BGRA8888, AlphaType::Unpremul),
        (ColorType::RGBAF16, AlphaType::Unpremul),
        (ColorType::RGBAF32, AlphaType::Unpremul),
        (ColorType::RGBA1010102, AlphaType::Unpremul),
    ];

    let colorspace = ColorSpace::new_srgb();
    let picture = make_picture();
    let mut generator = make_from_picture(
        (100, 100),
        picture,
        None,
        None,
        BitDepth::U8,
        Some(colorspace.clone()),
        SurfaceProps::default(),
    )
    .expect("a picture generator");

    // worst case for all requests
    let mut storage = vec![0u8; 100 * 100 * ColorType::RGBAF32.bytes_per_pixel()];

    for (color_type, alpha_type) in recs {
        let info = ImageInfo::new((100, 100), color_type, alpha_type, colorspace.clone());
        reporter_assert!(
            reporter,
            generator.get_pixels(&info, &mut storage, info.min_row_bytes())
        );
    }
});
