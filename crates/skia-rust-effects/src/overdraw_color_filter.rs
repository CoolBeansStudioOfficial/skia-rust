// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/SkOverdrawColorFilter.h, src/effects/colorfilters/SkOverdrawColorFilter.cpp

//! `SkOverdrawColorFilter`: maps the alpha of a color (the overdraw count) to one of six colors.
//!
//! skia-rust: built from the `Overdraw` known runtime effect (`SkSL`).

use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::data::Data;
use skia_rust_core::known_runtime_effects::{StableKey, get_known_runtime_effect};

/// `SkOverdrawColorFilter::kNumColors`.
// Port of: include/effects/SkOverdrawColorFilter.h#L28 (chrome/m156)
pub const NUM_COLORS: usize = 6;

/// `SkOverdrawColorFilter::MakeWithSkColors`: the filter that maps overdraw count `i` to
/// `colors[i]`.
// Port of: src/effects/colorfilters/SkOverdrawColorFilter.cpp#L27-L37 (chrome/m156)
#[doc(alias = "SkOverdrawColorFilter::MakeWithSkColors")]
#[must_use]
pub fn make_with_sk_colors(colors: &[Color; NUM_COLORS]) -> Option<ColorFilter> {
    let overdraw_effect = get_known_runtime_effect(StableKey::Overdraw)?;

    // The premultiplied colors, as floats, one after the other.
    let mut data = Vec::with_capacity(NUM_COLORS * 4 * size_of::<f32>());
    for color in colors {
        for channel in Color4f::from_color(*color).premul().as_array() {
            data.extend_from_slice(&channel.to_ne_bytes());
        }
    }
    overdraw_effect.make_color_filter(Data::new_copy(&data), &[])
}
