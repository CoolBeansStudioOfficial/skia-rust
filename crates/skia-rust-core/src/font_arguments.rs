// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkFontArguments.h

//! [`FontArguments`]: per-instance arguments for creating a typeface (`SkFontArguments.h`).

/// Arguments passed to `SkTypeface::makeClone` and the font-parameter APIs (`SkFontArguments`).
///
/// The variation position and palette overrides are borrowed, as they are in C++ (a raw pointer
/// plus count there, a slice here).
// Port of: include/core/SkFontArguments.h#L19-L114 (chrome/m156)
#[doc(alias = "SkFontArguments")]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontArguments<'vp, 'p> {
    collection_index: usize,
    variation_design_position: VariationPosition<'vp>,
    palette: Palette<'p>,
    synthetic_bold: Option<bool>,
    synthetic_oblique: Option<bool>,
}

/// Represents a position in the variation design space. Any axis not specified uses the default
/// value. Any specified axis not actually present in the font is ignored.
// Port of: include/core/SkFontArguments.h#L20-L32 (chrome/m156)
#[doc(alias = "SkFontArguments::VariationPosition")]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct VariationPosition<'a> {
    /// The axis/value pairs.
    pub coordinates: &'a [variation_position::Coordinate],
}

/// A single axis/value pair in a [`VariationPosition`].
pub mod variation_position {
    use crate::font_types::FourByteTag;
    use crate::font_types::set_four_byte_tag;

    /// One axis of a variation font and the design-space value for it.
    // Port of: include/core/SkFontArguments.h#L21-L29 (chrome/m156)
    #[doc(alias = "SkFontArguments::VariationPosition::Coordinate")]
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    pub struct Coordinate {
        /// The axis tag, such as [`Coordinate::WGHT`].
        pub axis: FourByteTag,
        /// The value on that axis.
        pub value: f32,
    }

    impl Coordinate {
        /// `wght`: weight.
        #[doc(alias = "wght")]
        pub const WGHT: FourByteTag = set_four_byte_tag(b'w', b'g', b'h', b't');
        /// `wdth`: width.
        #[doc(alias = "wdth")]
        pub const WDTH: FourByteTag = set_four_byte_tag(b'w', b'd', b't', b'h');
        /// `slnt`: slant.
        #[doc(alias = "slnt")]
        pub const SLNT: FourByteTag = set_four_byte_tag(b's', b'l', b'n', b't');
        /// `ital`: italic.
        #[doc(alias = "ital")]
        pub const ITAL: FourByteTag = set_four_byte_tag(b'i', b't', b'a', b'l');
        /// `opsz`: optical size.
        #[doc(alias = "opsz")]
        pub const OPSZ: FourByteTag = set_four_byte_tag(b'o', b'p', b's', b'z');
    }
}

/// Palette selection for color fonts (`SkFontArguments::Palette`).
// Port of: include/core/SkFontArguments.h#L41-L49 (chrome/m156)
#[doc(alias = "SkFontArguments::Palette")]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Palette<'a> {
    /// The index of the palette to use.
    pub index: i32,
    /// Entries of the palette to replace.
    pub overrides: &'a [palette::Override],
}

/// Palette entry overrides for [`Palette`].
pub mod palette {
    use crate::color::Color;

    /// Replaces one palette entry with a color.
    // Port of: include/core/SkFontArguments.h#L42-L45 (chrome/m156)
    #[doc(alias = "SkFontArguments::Palette::Override")]
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
    pub struct Override {
        /// The palette entry index.
        pub index: u16,
        /// The replacement color.
        pub color: Color,
    }
}

impl<'vp, 'p> FontArguments<'vp, 'p> {
    /// `SkFontArguments()`: collection index 0, no variation position, default palette, and
    /// synthetic bold and oblique unset.
    // Port of: include/core/SkFontArguments.h#L51-L54 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `SkFontArguments::setCollectionIndex`.
    // Port of: include/core/SkFontArguments.h#L61-L64 (chrome/m156)
    pub fn set_collection_index(&mut self, collection_index: usize) -> &mut Self {
        self.collection_index = collection_index;
        self
    }

    /// `SkFontArguments::setVariationDesignPosition`.
    // Port of: include/core/SkFontArguments.h#L73-L77 (chrome/m156)
    pub fn set_variation_design_position(&mut self, position: VariationPosition<'vp>) -> &mut Self {
        self.variation_design_position = position;
        self
    }

    /// `SkFontArguments::setPalette`.
    // Port of: include/core/SkFontArguments.h#L87-L91 (chrome/m156)
    pub fn set_palette(&mut self, palette: Palette<'p>) -> &mut Self {
        self.palette = palette;
        self
    }

    /// `SkFontArguments::setSyntheticBold`.
    // Port of: include/core/SkFontArguments.h#L96-L99 (chrome/m156)
    pub fn set_synthetic_bold(&mut self, bold: Option<bool>) -> &mut Self {
        self.synthetic_bold = bold;
        self
    }

    /// `SkFontArguments::setSyntheticOblique`.
    // Port of: include/core/SkFontArguments.h#L102-L105 (chrome/m156)
    pub fn set_synthetic_oblique(&mut self, oblique: Option<bool>) -> &mut Self {
        self.synthetic_oblique = oblique;
        self
    }

    /// `SkFontArguments::getCollectionIndex`.
    // Port of: include/core/SkFontArguments.h#L79-L81 (chrome/m156)
    #[must_use]
    pub fn collection_index(&self) -> usize {
        self.collection_index
    }

    /// `SkFontArguments::getVariationDesignPosition`.
    // Port of: include/core/SkFontArguments.h#L83-L85 (chrome/m156)
    #[must_use]
    pub fn variation_design_position(&self) -> VariationPosition<'vp> {
        self.variation_design_position
    }

    /// `SkFontArguments::getPalette`.
    // Port of: include/core/SkFontArguments.h#L94 (chrome/m156)
    #[must_use]
    pub fn palette(&self) -> Palette<'p> {
        self.palette
    }

    /// `SkFontArguments::getSyntheticBold`.
    // Port of: include/core/SkFontArguments.h#L100 (chrome/m156)
    #[must_use]
    pub fn synthetic_bold(&self) -> Option<bool> {
        self.synthetic_bold
    }

    /// `SkFontArguments::getSyntheticOblique`.
    // Port of: include/core/SkFontArguments.h#L106 (chrome/m156)
    #[must_use]
    pub fn synthetic_oblique(&self) -> Option<bool> {
        self.synthetic_oblique
    }
}
