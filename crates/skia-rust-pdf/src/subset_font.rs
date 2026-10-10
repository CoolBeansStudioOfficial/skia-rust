// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFSubsetFont.{h,cpp} (chrome/m156), the `#else` branch

//! The font subsetting seam of the PDF backend (`docs/design/modules.md` Q4).
//!
//! Skia subsets the fonts it embeds with `HarfBuzz`'s `hb-subset` when it is built with
//! `SK_PDF_USE_HARFBUZZ_SUBSET` (`BUILD.gn#L1262-L1279`).
//!
//! Q4 is decided: the parts of `hb-subset` that Skia uses are ported, byte-exact, in a crate of
//! their own. Until it lands, this module is the branch Skia takes when
//! `SK_PDF_USE_HARFBUZZ_SUBSET` is off (`SkPDFSubsetFont.cpp#L200-L210`): [`pdf_subset_font`]
//! returns `None`, so the caller embeds the font data as it is (Skia's "if subsetting fails,
//! fall back to original font data"), and [`pdf_can_subset_table_based_fonts`] is false, so a
//! font whose data is not in a standard format is drawn as Type3. The whole-font embedding is
//! not the final behaviour.
//!
//! The seam takes the font data, the glyph usage and the collection index and returns the
//! subset data, like `SkPDFSubsetFont`.

use crate::glyph_use::PdfGlyphUse;

/// `SkPDFSubsetFont`: the typeface's data subset to the glyphs used, with the glyph ids
/// unchanged; `None` if it cannot be subset. Always `None` until the hb-subset port lands (Q4).
// Port of: src/pdf/SkPDFSubsetFont.cpp#L200-L202 (chrome/m156)
#[doc(alias = "SkPDFSubsetFont")]
#[must_use]
pub fn pdf_subset_font(
    _font_data: &[u8],
    _glyph_usage: &PdfGlyphUse,
    _ttc_index: i32,
) -> Option<Vec<u8>> {
    // TODO(Q4 = hb-subset port): subset with the port of hb-subset.
    None
}

/// `SkPDFCanSubsetTableBasedFonts`. Always false until the hb-subset port lands (Q4).
// Port of: src/pdf/SkPDFSubsetFont.cpp#L204-L206 (chrome/m156)
#[doc(alias = "SkPDFCanSubsetTableBasedFonts")]
#[must_use]
pub fn pdf_can_subset_table_based_fonts() -> bool {
    // TODO(Q4 = hb-subset port): true once the subsetter handles table-based fonts.
    false
}
