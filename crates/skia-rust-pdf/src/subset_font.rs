// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFSubsetFont.{h,cpp} (chrome/m156), the `#else` branch

//! The font subsetting seam of the PDF backend (`docs/design/modules.md` Q4).
//!
//! Skia subsets the fonts it embeds with HarfBuzz's `hb-subset` when it is built with
//! `SK_PDF_USE_HARFBUZZ_SUBSET` (`BUILD.gn#L1262-L1279`). Whether to depend on a Rust subsetter
//! (`skera`), port the parts of `hb-subset` Skia uses, or embed whole fonts is an open question
//! (Q4), and no current test compares subset bytes.
//!
//! This module is the branch Skia takes when `SK_PDF_USE_HARFBUZZ_SUBSET` is off
//! (`SkPDFSubsetFont.cpp#L200-L210`): nothing can be subset, so [`pdf_subset_font`] returns
//! `None` and the document embeds the whole font (the caller's "if subsetting fails, fall back
//! to original font data"), and [`pdf_can_subset_table_based_fonts`] is false, so a font whose
//! data is not in a standard format is drawn as Type3.
//!
//! When Q4 is decided, this is the one place that changes.

use skia_rust_core::typeface::Typeface;

use crate::glyph_use::PdfGlyphUse;

/// `SkPDFSubsetFont`: the typeface's data subset to the glyphs used, with the glyph ids
/// unchanged; `None` if it cannot be subset. Always `None` until Q4 (subsetting) is decided.
// Port of: src/pdf/SkPDFSubsetFont.cpp#L200-L202 (chrome/m156)
#[doc(alias = "SkPDFSubsetFont")]
#[must_use]
pub fn pdf_subset_font(_typeface: &Typeface, _glyph_usage: &PdfGlyphUse) -> Option<Vec<u8>> {
    // Q4: subsetting. The `#else` branch of `SK_PDF_USE_HARFBUZZ_SUBSET`.
    None
}

/// `SkPDFCanSubsetTableBasedFonts`. Always false until Q4 (subsetting) is decided.
// Port of: src/pdf/SkPDFSubsetFont.cpp#L204-L206 (chrome/m156)
#[doc(alias = "SkPDFCanSubsetTableBasedFonts")]
#[must_use]
pub fn pdf_can_subset_table_based_fonts() -> bool {
    // Q4: subsetting. The `#else` branch of `SK_PDF_USE_HARFBUZZ_SUBSET`.
    false
}
