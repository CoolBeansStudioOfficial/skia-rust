// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ImageIsOpaqueTest.cpp (chrome/m156)
//
// Not ported: `Image_isAlphaOnly` (decodes `images/mandrill_128.png` and `images/color_wheel.jpg`
// and makes a lazy picture image, `DeferredFromPicture`). `ImageIsOpaqueTest_Gpu` is Ganesh-only
// (excluded).

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

use crate::{Reporter, def_tier_test, reporter_assert};

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
