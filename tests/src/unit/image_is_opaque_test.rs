// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ImageIsOpaqueTest.cpp (chrome/m156)
//
// `ImageIsOpaqueTest_Gpu` is Ganesh-only (excluded).

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::Rect;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_raster::image_picture::{BitDepth, deferred_from_picture};
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

use crate::resources::get_resource_as_image;
use crate::{Reporter, def_test, def_tier_test, reporter_assert};

// Port of: tests/ImageIsOpaqueTest.cpp#L33-L37 (chrome/m156)
fn check_isopaque(reporter: &mut Reporter, surface: &mut Surface<'_>, expected_opaque: bool) {
    let image = surface.image_snapshot().expect("a snapshot");
    reporter_assert!(reporter, image.is_opaque() == expected_opaque);
}

// Port of: tests/ImageIsOpaqueTest.cpp#L39-L47 (chrome/m156)
def_tier_test!(ImageIsOpaqueTest, |reporter| {
    let info_transparent = ImageInfo::new_n32_premul((5, 5), None);
    let mut surface_transparent = surfaces::raster(&info_transparent, None, None).expect("surface");
    check_isopaque(reporter, &mut surface_transparent, false);

    let info_opaque = ImageInfo::new_n32((5, 5), AlphaType::Opaque, None);
    let mut surface_opaque = surfaces::raster(&info_opaque, None, None).expect("surface");
    check_isopaque(reporter, &mut surface_opaque, true);
});

// Port of: tests/ImageIsOpaqueTest.cpp#L62-L69 (chrome/m156), make_picture
fn make_picture() -> Picture {
    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(Rect::new(0.0, 0.0, 10.0, 10.0), false);
    canvas.draw_color(Color::RED, None);
    recorder.finish_recording_as_picture(None).expect("a picture")
}

// Port of: tests/ImageIsOpaqueTest.cpp#L71-L95 (chrome/m156), Image_isAlphaOnly
def_test!(Image_isAlphaOnly, |reporter| {
    let pm_colors: u32 = 0; // SkPMColor
    let pm_bytes = pm_colors.to_ne_bytes();
    let pmap = Pixmap::new_readonly(&ImageInfo::new_n32_premul((1, 1), None), &pm_bytes, 4)
        .expect("pixmap");

    let images: [Option<Image>; 4] = [
        images::raster_from_pixmap_copy(&pmap),
        get_resource_as_image("images/mandrill_128.png"),
        get_resource_as_image("images/color_wheel.jpg"),
        deferred_from_picture(
            make_picture(),
            (10, 10),
            None,
            None,
            BitDepth::U8,
            Some(ColorSpace::new_srgb()),
            SurfaceProps::default(),
        ),
    ];
    for image in images {
        let image = image.expect("image");
        reporter_assert!(reporter, image.is_alpha_only() == false);
    }

    let a8_info = ImageInfo::new((1, 1), ColorType::Alpha8, AlphaType::Premul, None);
    let a8 = images::raster_from_pixmap_copy(
        &Pixmap::new_readonly(&a8_info, &pm_bytes[..1], 1).expect("a8 pixmap"),
    )
    .expect("a8 image");
    reporter_assert!(reporter, a8.is_alpha_only() == true);
});
