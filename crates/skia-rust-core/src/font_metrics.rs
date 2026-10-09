// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkFontMetrics.h

//! [`FontMetrics`]: the metrics of a font. The metric values are consistent with the Skia y-down
//! coordinate system.

use bitflags::bitflags;

use crate::scalar::scalar;

bitflags! {
    /// Indicates when certain metrics are valid; the underline or strikeout metrics may be valid
    /// and zero. Fonts with embedded bitmaps may not have valid underline or strikeout metrics.
    // Port of: include/core/SkFontMetrics.h#L39-L49 (chrome/m156)
    #[doc(alias = "SkFontMetrics::FontMetricsFlags")]
    #[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Flags: u32 {
        /// Set if `underline_thickness` is valid.
        #[doc(alias = "kUnderlineThicknessIsValid_Flag")]
        const UNDERLINE_THICKNESS_IS_VALID = 1 << 0;
        /// Set if `underline_position` is valid.
        #[doc(alias = "kUnderlinePositionIsValid_Flag")]
        const UNDERLINE_POSITION_IS_VALID = 1 << 1;
        /// Set if `strikeout_thickness` is valid.
        #[doc(alias = "kStrikeoutThicknessIsValid_Flag")]
        const STRIKEOUT_THICKNESS_IS_VALID = 1 << 2;
        /// Set if `strikeout_position` is valid.
        #[doc(alias = "kStrikeoutPositionIsValid_Flag")]
        const STRIKEOUT_POSITION_IS_VALID = 1 << 3;
        /// Set if `top`, `bottom`, `x_min`, `x_max` are invalid.
        #[doc(alias = "kBoundsInvalid_Flag")]
        const BOUNDS_INVALID = 1 << 4;
    }
}

/// The metrics of a font (`SkFontMetrics`). All fields are public, as in C++; use the accessors
/// to read the metrics that are only valid when their flag is set.
// Port of: include/core/SkFontMetrics.h#L18-L67 (chrome/m156)
#[doc(alias = "SkFontMetrics")]
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct FontMetrics {
    /// [`Flags`] indicating which metrics are valid.
    pub flags: Flags,
    /// Greatest extent above origin of any glyph bounding box, typically negative; deprecated
    /// with variable fonts.
    pub top: scalar,
    /// Distance to reserve above baseline, typically negative.
    pub ascent: scalar,
    /// Distance to reserve below baseline, typically positive.
    pub descent: scalar,
    /// Greatest extent below origin of any glyph bounding box, typically positive; deprecated
    /// with variable fonts.
    pub bottom: scalar,
    /// Distance to add between lines, typically positive or zero.
    pub leading: scalar,
    /// Average character width, zero if unknown.
    pub avg_char_width: scalar,
    /// Maximum character width, zero if unknown.
    pub max_char_width: scalar,
    /// Greatest extent to left of origin of any glyph bounding box, typically negative;
    /// deprecated with variable fonts.
    pub x_min: scalar,
    /// Greatest extent to right of origin of any glyph bounding box, typically positive;
    /// deprecated with variable fonts.
    pub x_max: scalar,
    /// Height of lower-case 'x', zero if unknown, typically negative.
    pub x_height: scalar,
    /// Height of an upper-case letter, zero if unknown, typically negative.
    pub cap_height: scalar,
    /// Underline thickness; valid only if [`Flags::UNDERLINE_THICKNESS_IS_VALID`] is set.
    pub underline_thickness: scalar,
    /// Distance from baseline to top of stroke, typically positive; valid only if
    /// [`Flags::UNDERLINE_POSITION_IS_VALID`] is set.
    pub underline_position: scalar,
    /// Strikeout thickness; valid only if [`Flags::STRIKEOUT_THICKNESS_IS_VALID`] is set.
    pub strikeout_thickness: scalar,
    /// Distance from baseline to bottom of stroke, typically negative; valid only if
    /// [`Flags::STRIKEOUT_POSITION_IS_VALID`] is set.
    pub strikeout_position: scalar,
}

impl FontMetrics {
    /// Returns `Some(thickness)` if the font metrics have a valid underline thickness, otherwise
    /// `None` (`SkFontMetrics::hasUnderlineThickness`).
    // Port of: include/core/SkFontMetrics.h#L76-L82 (chrome/m156)
    #[must_use]
    pub fn underline_thickness(&self) -> Option<scalar> {
        self.if_valid(
            Flags::UNDERLINE_THICKNESS_IS_VALID,
            self.underline_thickness,
        )
    }

    /// Returns `Some(position)` if the font metrics have a valid underline position, otherwise
    /// `None` (`SkFontMetrics::hasUnderlinePosition`).
    // Port of: include/core/SkFontMetrics.h#L91-L97 (chrome/m156)
    #[must_use]
    pub fn underline_position(&self) -> Option<scalar> {
        self.if_valid(Flags::UNDERLINE_POSITION_IS_VALID, self.underline_position)
    }

    /// Returns `Some(thickness)` if the font metrics have a valid strikeout thickness, otherwise
    /// `None` (`SkFontMetrics::hasStrikeoutThickness`).
    // Port of: include/core/SkFontMetrics.h#L106-L112 (chrome/m156)
    #[must_use]
    pub fn strikeout_thickness(&self) -> Option<scalar> {
        self.if_valid(
            Flags::STRIKEOUT_THICKNESS_IS_VALID,
            self.strikeout_thickness,
        )
    }

    /// Returns `Some(position)` if the font metrics have a valid strikeout position, otherwise
    /// `None` (`SkFontMetrics::hasStrikeoutPosition`).
    // Port of: include/core/SkFontMetrics.h#L121-L127 (chrome/m156)
    #[must_use]
    pub fn strikeout_position(&self) -> Option<scalar> {
        self.if_valid(Flags::STRIKEOUT_POSITION_IS_VALID, self.strikeout_position)
    }

    /// Returns true if the font metrics have a valid `top`, `bottom`, `x_min` and `x_max`
    /// (`SkFontMetrics::hasBounds`).
    // Port of: include/core/SkFontMetrics.h#L134-L137 (chrome/m156)
    #[must_use]
    pub fn has_bounds(&self) -> bool {
        !self.flags.contains(Flags::BOUNDS_INVALID)
    }

    fn if_valid(&self, flag: Flags, value: scalar) -> Option<scalar> {
        self.flags.contains(flag).then_some(value)
    }
}
