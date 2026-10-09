// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/SkLumaColorFilter.h, src/effects/colorfilters/SkLumaColorFilter.cpp

//! `SkLumaColorFilter`: converts a color to its luminance, as an alpha mask.
//!
//! skia-rust: built from the `Luma` known runtime effect (`SkSL`).

use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::data::Data;
use skia_rust_core::known_runtime_effects::{StableKey, get_known_runtime_effect};

/// `SkLumaColorFilter::Make`: the luminance filter (`None` only if the effect cannot be made).
// Port of: src/effects/colorfilters/SkLumaColorFilter.cpp#L17-L22 (chrome/m156)
#[doc(alias = "SkLumaColorFilter::Make")]
#[must_use]
pub fn make() -> Option<ColorFilter> {
    let luma_effect = get_known_runtime_effect(StableKey::Luma)?;
    luma_effect.make_color_filter(Data::new_empty(), &[])
}
