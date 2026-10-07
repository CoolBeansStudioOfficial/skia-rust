// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ICCTest.cpp (chrome/m156)

use skia_rust_core::color_space::{ColorSpace, named_gamut, named_transfer_fn};
use skia_rust_skcms as skcms;

use crate::resources::get_resource_as_data;
use crate::{def_test, reporter_assert};

// Port of: tests/ICCTest.cpp#L22-L32 (chrome/m156)
def_test!(AdobeRGB, |r| {
    if let Some(profile) = get_resource_as_data("icc_profiles/AdobeRGB1998.icc") {
        let parsed = skcms::parse(&profile);
        reporter_assert!(r, parsed.is_some());
        let Some(parsed) = parsed else {
            return;
        };
        reporter_assert!(r, !parsed.has_cicp);

        let got = ColorSpace::make(&parsed);
        let want = ColorSpace::new_rgb(&named_transfer_fn::DOT22, &named_gamut::ADOBE_RGB);
        reporter_assert!(r, ColorSpace::equals(got.as_ref(), want.as_ref()));
    }
});
