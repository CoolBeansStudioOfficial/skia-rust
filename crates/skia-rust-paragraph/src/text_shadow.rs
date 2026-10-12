// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/include/TextShadow.h,
// modules/skparagraph/src/TextShadow.cpp

//! `TextShadow`: a shadow of a text style.

use skia_rust_core::color::Color;
use skia_rust_core::point::Point;

// Port of: modules/skparagraph/include/TextShadow.h#L14-L28 (chrome/m156)
/// `TextShadow`: the color, offset and blur of a shadow.
#[derive(Debug, Copy, Clone)]
pub struct TextShadow {
    /// `fColor`.
    pub color: Color,
    /// `fOffset`.
    pub offset: Point,
    /// `fBlurSigma`.
    pub blur_sigma: f64,
}

impl Default for TextShadow {
    // Port of: modules/skparagraph/src/TextShadow.cpp#L11 (chrome/m156), with the member
    // initializers of TextShadow.h#L17-L19: black, a zero offset, no blur.
    fn default() -> Self {
        Self {
            color: Color::BLACK,
            offset: Point::new(0.0, 0.0),
            blur_sigma: 0.0,
        }
    }
}

impl PartialEq for TextShadow {
    // Port of: modules/skparagraph/src/TextShadow.cpp#L16-L22 (chrome/m156)
    fn eq(&self, other: &Self) -> bool {
        if self.color != other.color {
            return false;
        }
        if self.offset != other.offset {
            return false;
        }
        #[allow(clippy::float_cmp)] // mirrors `fBlurSigma != other.fBlurSigma`
        if self.blur_sigma != other.blur_sigma {
            return false;
        }
        true
    }
}

impl TextShadow {
    // Port of: modules/skparagraph/src/TextShadow.cpp#L13-L14 (chrome/m156)
    /// `TextShadow(SkColor color, SkPoint offset, double blurSigma)`.
    #[must_use]
    pub fn new(color: impl Into<Color>, offset: impl Into<Point>, blur_sigma: f64) -> Self {
        Self {
            color: color.into(),
            offset: offset.into(),
            blur_sigma,
        }
    }

    // Port of: modules/skparagraph/src/TextShadow.cpp#L26-L30 (chrome/m156)
    /// `hasShadow()`.
    #[must_use]
    #[doc(alias = "hasShadow")]
    pub fn has_shadow(&self) -> bool {
        if !self.offset.is_zero() {
            return true;
        }
        #[allow(clippy::float_cmp)] // mirrors `fBlurSigma != 0.0`
        if self.blur_sigma != 0.0 {
            return true;
        }
        false
    }
}
