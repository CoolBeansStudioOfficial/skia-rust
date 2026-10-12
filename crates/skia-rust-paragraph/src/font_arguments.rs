// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/include/FontArguments.h,
// modules/skparagraph/src/FontArguments.cpp

//! `FontArguments` of the paragraph module: an owned copy of the `SkFontArguments` of a text
//! style.

use skia_rust_core::font_arguments::{
    FontArguments as CoreFontArguments, Palette, VariationPosition, palette,
    variation_position::Coordinate,
};
use skia_rust_core::typeface::Typeface;

// Port of: modules/skparagraph/include/FontArguments.h#L19-L46 (chrome/m156)
/// `FontArguments`: the collection index, variation coordinates and palette of a font, owned.
#[derive(Debug, Clone, PartialEq)]
pub struct FontArguments {
    collection_index: usize,
    coordinates: Vec<Coordinate>,
    palette_index: i32,
    palette_overrides: Vec<palette::Override>,
}

impl From<&CoreFontArguments<'_, '_>> for FontArguments {
    // Port of: modules/skparagraph/src/FontArguments.cpp#L40-L48 (chrome/m156)
    fn from(args: &CoreFontArguments<'_, '_>) -> Self {
        Self {
            collection_index: args.collection_index(),
            coordinates: args.variation_design_position().coordinates.to_vec(),
            palette_index: args.palette().index,
            palette_overrides: args.palette().overrides.to_vec(),
        }
    }
}

impl From<CoreFontArguments<'_, '_>> for FontArguments {
    fn from(args: CoreFontArguments<'_, '_>) -> Self {
        Self::from(&args)
    }
}

impl FontArguments {
    // Port of: modules/skparagraph/src/FontArguments.cpp#L58-L77 (chrome/m156)
    /// `CloneTypeface(typeface)`: `typeface->makeClone(args)` with these arguments.
    #[must_use]
    #[doc(alias = "CloneTypeface")]
    pub fn clone_typeface(&self, typeface: &Typeface) -> Typeface {
        let position = VariationPosition {
            coordinates: &self.coordinates,
        };
        let palette = Palette {
            index: self.palette_index,
            overrides: &self.palette_overrides,
        };
        let mut args = CoreFontArguments::new();
        args.set_collection_index(self.collection_index);
        args.set_variation_design_position(position);
        args.set_palette(palette);
        typeface.make_clone(&args)
    }
}
