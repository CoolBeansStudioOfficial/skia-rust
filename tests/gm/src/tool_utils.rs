// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/ToolUtils.cpp

//! The parts of `ToolUtils` that GMs use.

use skia_rust_core::color::{pre_multiply_color, Color};
use skia_rust_core::color_data::{pixel16_to_color, pixel32_to_pixel16};

/// `ToolUtils::color_to_565`: rounds `color` to what a 565 surface would store.
// Port of: tools/ToolUtils.cpp#L142-L151 (chrome/m156)
#[must_use]
pub fn color_to_565(color: impl Into<Color>) -> Color {
    let color = color.into();
    // Not a good idea to use this function for greyscale colors...
    // it will add an obvious purple or green tint.
    debug_assert!(color.r() != color.g() || color.r() != color.b() || color.g() != color.b());

    let pm_color = pre_multiply_color(color);
    let color16 = pixel32_to_pixel16(pm_color);
    pixel16_to_color(color16)
}
