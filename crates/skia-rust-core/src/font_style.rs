// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkFontStyle.h

//! [`FontStyle`]: the weight, width and slant of a font (`SkFontStyle.h`).

use std::ops::Deref;

/// The weight of a font (`SkFontStyle::Weight`). Wraps the integer value; the named constants
/// are the enum values from Skia.
// Port of: include/core/SkFontStyle.h#L18-L30 (chrome/m156)
#[doc(alias = "SkFontStyle::Weight")]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[repr(transparent)]
pub struct Weight(i32);

impl From<i32> for Weight {
    fn from(weight: i32) -> Self {
        Weight(weight)
    }
}

impl Deref for Weight {
    type Target = i32;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[allow(non_upper_case_globals)]
impl Weight {
    /// `kInvisible_Weight`.
    #[doc(alias = "kInvisible_Weight")]
    pub const INVISIBLE: Self = Self(0);
    /// `kThin_Weight`.
    #[doc(alias = "kThin_Weight")]
    pub const THIN: Self = Self(100);
    /// `kExtraLight_Weight`.
    #[doc(alias = "kExtraLight_Weight")]
    pub const EXTRA_LIGHT: Self = Self(200);
    /// `kLight_Weight`.
    #[doc(alias = "kLight_Weight")]
    pub const LIGHT: Self = Self(300);
    /// `kNormal_Weight`.
    #[doc(alias = "kNormal_Weight")]
    pub const NORMAL: Self = Self(400);
    /// `kMedium_Weight`.
    #[doc(alias = "kMedium_Weight")]
    pub const MEDIUM: Self = Self(500);
    /// `kSemiBold_Weight`.
    #[doc(alias = "kSemiBold_Weight")]
    pub const SEMI_BOLD: Self = Self(600);
    /// `kBold_Weight`.
    #[doc(alias = "kBold_Weight")]
    pub const BOLD: Self = Self(700);
    /// `kExtraBold_Weight`.
    #[doc(alias = "kExtraBold_Weight")]
    pub const EXTRA_BOLD: Self = Self(800);
    /// `kBlack_Weight`.
    #[doc(alias = "kBlack_Weight")]
    pub const BLACK: Self = Self(900);
    /// `kExtraBlack_Weight`.
    #[doc(alias = "kExtraBlack_Weight")]
    pub const EXTRA_BLACK: Self = Self(1000);
}

/// The width of a font (`SkFontStyle::Width`). Wraps the integer value; the named constants are
/// the enum values from Skia.
// Port of: include/core/SkFontStyle.h#L32-L43 (chrome/m156)
#[doc(alias = "SkFontStyle::Width")]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[repr(transparent)]
pub struct Width(i32);

impl From<i32> for Width {
    fn from(width: i32) -> Self {
        Width(width)
    }
}

impl Deref for Width {
    type Target = i32;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[allow(non_upper_case_globals)]
impl Width {
    /// `kUltraCondensed_Width`.
    #[doc(alias = "kUltraCondensed_Width")]
    pub const ULTRA_CONDENSED: Self = Self(1);
    /// `kExtraCondensed_Width`.
    #[doc(alias = "kExtraCondensed_Width")]
    pub const EXTRA_CONDENSED: Self = Self(2);
    /// `kCondensed_Width`.
    #[doc(alias = "kCondensed_Width")]
    pub const CONDENSED: Self = Self(3);
    /// `kSemiCondensed_Width`.
    #[doc(alias = "kSemiCondensed_Width")]
    pub const SEMI_CONDENSED: Self = Self(4);
    /// `kNormal_Width`.
    #[doc(alias = "kNormal_Width")]
    pub const NORMAL: Self = Self(5);
    /// `kSemiExpanded_Width`.
    #[doc(alias = "kSemiExpanded_Width")]
    pub const SEMI_EXPANDED: Self = Self(6);
    /// `kExpanded_Width`.
    #[doc(alias = "kExpanded_Width")]
    pub const EXPANDED: Self = Self(7);
    /// `kExtraExpanded_Width`.
    #[doc(alias = "kExtraExpanded_Width")]
    pub const EXTRA_EXPANDED: Self = Self(8);
    /// `kUltraExpanded_Width`.
    #[doc(alias = "kUltraExpanded_Width")]
    pub const ULTRA_EXPANDED: Self = Self(9);
}

/// The slant of a font (`SkFontStyle::Slant`).
// Port of: include/core/SkFontStyle.h#L44-L47 (chrome/m156)
#[doc(alias = "SkFontStyle::Slant")]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Default)]
#[repr(u8)]
pub enum Slant {
    /// `kUpright_Slant`.
    #[doc(alias = "kUpright_Slant")]
    #[default]
    Upright = 0,
    /// `kItalic_Slant`.
    #[doc(alias = "kItalic_Slant")]
    Italic = 1,
    /// `kOblique_Slant`.
    #[doc(alias = "kOblique_Slant")]
    Oblique = 2,
}

/// Describes the style of a font: weight, width and slant (`SkFontStyle`).
///
/// The three fields are packed into one `i32` exactly as in C++ (weight in bits 0..16, width in
/// bits 16..24, slant in bits 24..32), so equality is a single integer comparison.
// Port of: include/core/SkFontStyle.h#L16-L81 (chrome/m156)
#[doc(alias = "SkFontStyle")]
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct FontStyle {
    value: i32,
}

impl Default for FontStyle {
    /// `SkFontStyle()`: normal weight, normal width, upright.
    // Port of: include/core/SkFontStyle.h#L56 (chrome/m156)
    fn default() -> Self {
        Self::new(Weight::NORMAL, Width::NORMAL, Slant::Upright)
    }
}

impl std::fmt::Debug for FontStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontStyle")
            .field("weight", &self.weight())
            .field("width", &self.width())
            .field("slant", &self.slant())
            .finish()
    }
}

impl FontStyle {
    /// `SkFontStyle(int weight, int width, Slant slant)`. Each value is pinned to its range.
    // Port of: include/core/SkFontStyle.h#L50-L54 (chrome/m156)
    #[must_use]
    pub const fn new(weight: Weight, width: Width, slant: Slant) -> Self {
        let w = pin_i32(weight.0, Weight::INVISIBLE.0, Weight::EXTRA_BLACK.0);
        let wd = pin_i32(width.0, Width::ULTRA_CONDENSED.0, Width::ULTRA_EXPANDED.0);
        let s = pin_i32(slant as i32, Slant::Upright as i32, Slant::Oblique as i32);
        Self {
            value: w + (wd << 16) + (s << 24),
        }
    }

    /// `SkFontStyle::weight()`.
    // Port of: include/core/SkFontStyle.h#L62 (chrome/m156)
    #[must_use]
    pub const fn weight(self) -> Weight {
        Weight(self.value & 0xFFFF)
    }

    /// `SkFontStyle::width()`.
    // Port of: include/core/SkFontStyle.h#L63 (chrome/m156)
    #[must_use]
    pub const fn width(self) -> Width {
        Width((self.value >> 16) & 0xFF)
    }

    /// `SkFontStyle::slant()`.
    // Port of: include/core/SkFontStyle.h#L64 (chrome/m156)
    #[must_use]
    pub const fn slant(self) -> Slant {
        match (self.value >> 24) & 0xFF {
            0 => Slant::Upright,
            1 => Slant::Italic,
            _ => Slant::Oblique,
        }
    }

    /// `SkFontStyle::Normal()`.
    // Port of: include/core/SkFontStyle.h#L66-L68 (chrome/m156)
    #[doc(alias = "SkFontStyle::Normal")]
    #[must_use]
    pub const fn normal() -> Self {
        Self::new(Weight::NORMAL, Width::NORMAL, Slant::Upright)
    }

    /// `SkFontStyle::Bold()`.
    // Port of: include/core/SkFontStyle.h#L69-L71 (chrome/m156)
    #[doc(alias = "SkFontStyle::Bold")]
    #[must_use]
    pub const fn bold() -> Self {
        Self::new(Weight::BOLD, Width::NORMAL, Slant::Upright)
    }

    /// `SkFontStyle::Italic()`.
    // Port of: include/core/SkFontStyle.h#L72-L74 (chrome/m156)
    #[doc(alias = "SkFontStyle::Italic")]
    #[must_use]
    pub const fn italic() -> Self {
        Self::new(Weight::NORMAL, Width::NORMAL, Slant::Italic)
    }

    /// `SkFontStyle::BoldItalic()`.
    // Port of: include/core/SkFontStyle.h#L75-L77 (chrome/m156)
    #[doc(alias = "SkFontStyle::BoldItalic")]
    #[must_use]
    pub const fn bold_italic() -> Self {
        Self::new(Weight::BOLD, Width::NORMAL, Slant::Italic)
    }
}

/// `SkTPin<int>(x, lo, hi)`, usable in a `const fn` (`t_pin` is not const).
// Port of: include/private/SkTPin.h#L19-L21 (chrome/m156), specialised to int
const fn pin_i32(x: i32, lo: i32, hi: i32) -> i32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}
