// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/HighContrastFilterTest.cpp (chrome/m156)

//! `HighContrastFilterTest`: the high contrast filter's smoke test and its invalid inputs.

use skia_rust_core::color::{Color4f, colors};
use skia_rust_effects::high_contrast_filter::{
    HighContrastConfig, HighContrastFilter, InvertStyle,
};

use crate::{def_test, reporter_assert};

// Port of: tests/HighContrastFilterTest.cpp#L17-L29 (chrome/m156)
def_test!(HighContrastFilter_SmokeTest, |r| {
    let config = HighContrastConfig {
        invert_style: InvertStyle::INVERT_LIGHTNESS,
        ..HighContrastConfig::default()
    };
    let filter = HighContrastFilter::make(&config);
    reporter_assert!(
        r,
        filter
            .as_ref()
            .is_some_and(skia_rust_core::color_filter::ColorFilter::is_alpha_unchanged)
    );
    let Some(filter) = filter else {
        return;
    };

    let white_inverted = filter.filter_color4f(Color4f::new(1.0, 1.0, 1.0, 1.0), None, None);
    reporter_assert!(r, white_inverted == colors::BLACK);

    let black_inverted = filter.filter_color4f(Color4f::new(0.0, 0.0, 0.0, 1.0), None, None);
    reporter_assert!(r, black_inverted == colors::WHITE);
});

// Port of: tests/HighContrastFilterTest.cpp#L31-L61 (chrome/m156)
def_test!(HighContrastFilter_InvalidInputs, |r| {
    let mut config = HighContrastConfig::default();
    reporter_assert!(r, config.is_valid());

    // Valid invert style
    config.invert_style = InvertStyle::INVERT_BRIGHTNESS;
    reporter_assert!(r, config.is_valid());
    config.invert_style = InvertStyle::INVERT_LIGHTNESS;
    reporter_assert!(r, config.is_valid());
    let mut filter = HighContrastFilter::make(&config);
    reporter_assert!(r, filter.is_some());

    // Invalid invert style
    config.invert_style = InvertStyle::from_raw(999);
    reporter_assert!(r, !config.is_valid());
    filter = HighContrastFilter::make(&config);
    reporter_assert!(r, filter.is_none());

    // Valid contrast
    for contrast in [0.5_f32, 1.0, -1.0] {
        config.invert_style = InvertStyle::INVERT_BRIGHTNESS;
        config.contrast = contrast;
        reporter_assert!(r, config.is_valid());
        filter = HighContrastFilter::make(&config);
        reporter_assert!(r, filter.is_some());
    }

    // Invalid contrast
    config.contrast = 1.1;
    reporter_assert!(r, !config.is_valid());
    filter = HighContrastFilter::make(&config);
    reporter_assert!(r, filter.is_none());
});
