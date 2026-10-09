// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/SkHighContrastFilter.h, src/effects/SkHighContrastFilter.cpp

//! `SkHighContrastFilter`: grayscale, invert and contrast adjustments, for accessibility.
//!
//! skia-rust: built from the `HighContrast` known runtime effect (`SkSL`), run in the linear
//! working format.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_space::named_transfer_fn;
use skia_rust_core::data::Data;
use skia_rust_core::known_runtime_effects::{StableKey, get_known_runtime_effect};
use skia_rust_core::working_format_color_filter::with_working_format;

/// `SkHighContrastConfig::InvertStyle`: how the colors are inverted.
///
/// C++ has an `enum class` that any `int` can be cast to, and `isValid` checks for the values
/// out of range. That is a newtype here, since a Rust enum cannot hold those values.
// Port of: include/effects/SkHighContrastFilter.h#L15-L20 (chrome/m156)
#[doc(alias = "SkHighContrastConfig::InvertStyle")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct InvertStyle(i32);

impl InvertStyle {
    /// `kNoInvert`.
    pub const NO_INVERT: InvertStyle = InvertStyle(0);
    /// `kInvertBrightness`.
    pub const INVERT_BRIGHTNESS: InvertStyle = InvertStyle(1);
    /// `kInvertLightness`, the last valid style (`kLast`).
    pub const INVERT_LIGHTNESS: InvertStyle = InvertStyle(2);

    /// The style with the C++ enum value `value` (which may be out of range).
    #[must_use]
    pub const fn from_raw(value: i32) -> Self {
        InvertStyle(value)
    }

    /// The C++ enum value of the style.
    #[must_use]
    pub const fn raw(self) -> i32 {
        self.0
    }
}

/// `SkHighContrastConfig`: the settings of a high contrast filter.
// Port of: include/effects/SkHighContrastFilter.h#L22-L58 (chrome/m156)
#[doc(alias = "SkHighContrastConfig")]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct HighContrastConfig {
    /// If true, the color will be converted to grayscale (`fGrayscale`).
    pub grayscale: bool,
    /// Whether to invert brightness, lightness, or neither (`fInvertStyle`).
    pub invert_style: InvertStyle,
    /// After grayscale and inverting, the contrast can be adjusted linearly, from -1 to 1, where
    /// 0 is no adjustment (`fContrast`).
    pub contrast: f32,
}

impl Default for HighContrastConfig {
    // Port of: include/effects/SkHighContrastFilter.h#L27-L32 (chrome/m156)
    fn default() -> Self {
        HighContrastConfig {
            grayscale: false,
            invert_style: InvertStyle::NO_INVERT,
            contrast: 0.0,
        }
    }
}

impl HighContrastConfig {
    /// A config with the given settings (`SkHighContrastConfig(bool, InvertStyle, SkScalar)`).
    // Port of: include/effects/SkHighContrastFilter.h#L34-L38 (chrome/m156)
    #[must_use]
    pub fn new(grayscale: bool, invert_style: InvertStyle, contrast: f32) -> Self {
        HighContrastConfig {
            grayscale,
            invert_style,
            contrast,
        }
    }

    /// True if all of the fields are set within the valid range (`isValid`).
    // Port of: include/effects/SkHighContrastFilter.h#L44-L50 (chrome/m156)
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.invert_style.raw() >= InvertStyle::NO_INVERT.raw()
            && self.invert_style.raw() <= InvertStyle::INVERT_LIGHTNESS.raw()
            && self.contrast >= -1.0
            && self.contrast <= 1.0
    }
}

/// `SkHighContrastFilter`: the filter factory.
#[doc(alias = "SkHighContrastFilter")]
#[derive(Copy, Clone, Debug)]
pub struct HighContrastFilter;

impl HighContrastFilter {
    /// The filter for `config`, or `None` if the config is invalid (`Make`).
    // Port of: src/effects/SkHighContrastFilter.cpp#L22-L53 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(config: &HighContrastConfig) -> Option<ColorFilter> {
        if !config.is_valid() {
            return None;
        }

        // A contrast setting of exactly +1 would divide by zero (1+c)/(1-c), so pull in to +1-ε.
        // I'm not exactly sure why we've historically pinned -1 up to -1+ε, maybe just symmetry?
        // (`SkTPin(x, lo, hi)` is `max(lo, min(x, hi))`.)
        let c = (-1.0 + f32::EPSILON).max(config.contrast.min(1.0 - f32::EPSILON));
        let uniforms: [f32; 3] = [
            if config.grayscale { 1.0 } else { 0.0 },
            // 0.0 for none, 1.0 for brightness, 2.0 for lightness
            // (the style is 0, 1 or 2 here: `isValid` has passed)
            #[allow(clippy::cast_precision_loss)] // small integers are exact in f32
            (config.invert_style.raw() as f32),
            (1.0 + c) / (1.0 - c),
        ];
        let mut bytes = Vec::with_capacity(size_of_val(&uniforms));
        for value in uniforms {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }

        let high_contrast_effect = get_known_runtime_effect(StableKey::HighContrast)?;
        let filter = high_contrast_effect.make_color_filter(Data::new_copy(&bytes), &[]);
        let linear = named_transfer_fn::LINEAR;
        with_working_format(filter, Some(&linear), None, Some(&AlphaType::Unpremul))
    }
}
