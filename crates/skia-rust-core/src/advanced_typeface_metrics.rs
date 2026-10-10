// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/core/SkAdvancedTypefaceMetrics.h (chrome/m156)

//! [`AdvancedTypefaceMetrics`]: what the PDF backend needs to know to embed a typeface.

use bitflags::bitflags;

use crate::rect::IRect;

bitflags! {
    /// `SkAdvancedTypefaceMetrics::StyleFlags`. These values match the values used in the PDF
    /// file format.
    // Port of: src/core/SkAdvancedTypefaceMetrics.h#L30-L38 (chrome/m156)
    #[doc(alias = "SkAdvancedTypefaceMetrics::StyleFlags")]
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct StyleFlags: u32 {
        /// `kFixedPitch_Style`.
        const FIXED_PITCH = 0x0000_0001;
        /// `kSerif_Style`.
        const SERIF = 0x0000_0002;
        /// `kScript_Style`.
        const SCRIPT = 0x0000_0008;
        /// `kItalic_Style`.
        const ITALIC = 0x0000_0040;
        /// `kAllCaps_Style`.
        const ALL_CAPS = 0x0001_0000;
        /// `kSmallCaps_Style`.
        const SMALL_CAPS = 0x0002_0000;
        /// `kForceBold_Style`.
        const FORCE_BOLD = 0x0004_0000;
    }
}

bitflags! {
    /// `SkAdvancedTypefaceMetrics::FontFlags`: global font flags.
    // Port of: src/core/SkAdvancedTypefaceMetrics.h#L52-L57 (chrome/m156)
    #[doc(alias = "SkAdvancedTypefaceMetrics::FontFlags")]
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct FontFlags: u8 {
        /// `kVariable_FontFlag`: may be true for Type1, CFF, or TrueType fonts.
        const VARIABLE = 1 << 0;
        /// `kNotEmbeddable_FontFlag`: may not be embedded.
        const NOT_EMBEDDABLE = 1 << 1;
        /// `kNotSubsettable_FontFlag`: may not be subset.
        const NOT_SUBSETTABLE = 1 << 2;
        /// `kAltDataFormat_FontFlag`: data compressed. Table access may still work.
        const ALT_DATA_FORMAT = 1 << 3;
    }
}

/// `SkAdvancedTypefaceMetrics::FontType`: the type of the underlying font program. It determines
/// which of the other fields are valid. For `Other` the per glyph information is never
/// populated.
// Port of: src/core/SkAdvancedTypefaceMetrics.h#L40-L46 (chrome/m156)
#[doc(alias = "SkAdvancedTypefaceMetrics::FontType")]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontType {
    /// `kType1_Font`.
    Type1,
    /// `kType1CID_Font`.
    Type1Cid,
    /// `kCFF_Font`.
    Cff,
    /// `kTrueType_Font`.
    TrueType,
    /// `kOther_Font`.
    #[default]
    Other,
}

/// `SkAdvancedTypefaceMetrics`: used by the PDF backend to correctly embed typefaces. Filled in
/// by [`Typeface::advanced_metrics`](crate::typeface::Typeface::advanced_metrics).
// Port of: src/core/SkAdvancedTypefaceMetrics.h#L21-L66 (chrome/m156)
#[doc(alias = "SkAdvancedTypefaceMetrics")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AdvancedTypefaceMetrics {
    /// `fPostScriptName`: the PostScript name of the font. See `FontName` and `BaseFont` in the
    /// PDF standard.
    pub post_script_name: String,
    /// `fStyle`: font style characteristics.
    pub style: StyleFlags,
    /// `fType`: the type of the underlying font program.
    pub font_type: FontType,
    /// `fFlags`: global font flags.
    pub flags: FontFlags,
    /// `fItalicAngle`: counterclockwise degrees from vertical of the dominant vertical stroke
    /// for an italic face.
    pub italic_angle: i16,
    /// `fAscent`: max height above baseline, not including accents (font units).
    pub ascent: i16,
    /// `fDescent`: max depth below baseline, negative (font units).
    pub descent: i16,
    /// `fStemV`: thickness of dominant vertical stem (font units).
    pub stem_v: i16,
    /// `fCapHeight`: height (from baseline) of top of flat capitals (font units).
    pub cap_height: i16,
    /// `fBBox`: the bounding box of all glyphs (font units).
    pub bbox: IRect,
}
