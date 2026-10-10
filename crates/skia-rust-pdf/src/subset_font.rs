// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFSubsetFont.{h,cpp} (chrome/m156), the `SK_PDF_USE_HARFBUZZ_SUBSET` branch

//! The font subsetting seam of the PDF backend (`docs/design/modules.md` Q4).
//!
//! Skia subsets the fonts it embeds with `HarfBuzz`'s `hb-subset` when it is built with
//! `SK_PDF_USE_HARFBUZZ_SUBSET` (`BUILD.gn#L1262-L1279`). The port of the parts of `hb-subset`
//! that Skia uses is `skia-rust-hb-subset`; this module is `subset_harfbuzz` of
//! `SkPDFSubsetFont.cpp` over it.
//!
//! The seam takes the font data (the whole file, as `typeface.openStream` gives it), the glyph
//! usage and the collection index, like `SkPDFSubsetFont(typeface, glyphUsage)`.
//!
//! Not reproduced: the table-based face (`hb_face_create_for_tables` over `copyTableData`), which
//! Skia builds when the typeface has no memory stream or when the memory-stream face fails. It
//! needs the typeface's tables, not the bytes this seam receives. Its reachable case is a WOFF or
//! WOFF2 font (`kAltDataFormat_FontFlag`): `HarfBuzz` does not recognize those containers, so the
//! seam returns `None` for them and the caller embeds the original data.

use skia_rust_hb_subset::subset_font;

use crate::glyph_use::PdfGlyphUse;

/// `SkPDFSubsetFont`: the typeface's data subset to the glyphs used, with the glyph ids
/// unchanged; `None` if it cannot be subset (the caller then embeds the original data).
// Port of: src/pdf/SkPDFSubsetFont.cpp#L133-L183 (`subset_harfbuzz`), #L187-L189 (chrome/m156)
#[doc(alias = "SkPDFSubsetFont")]
#[must_use]
pub fn pdf_subset_font(
    font_data: &[u8],
    glyph_usage: &PdfGlyphUse,
    ttc_index: i32,
) -> Option<Vec<u8>> {
    // `stream_to_face` converts the `int` index to `unsigned`, so a negative index is out of
    // range there and no face is made.
    let ttc_index = u32::try_from(ttc_index).ok()?;
    // `glyphUsage.getSetValues([&glyphs](unsigned gid) { hb_set_add(glyphs, gid); })`. The ids
    // are `GlyphId` (16-bit) values, so the narrowing mirrors the C++ `unsigned`.
    let mut glyphs = Vec::new();
    glyph_usage.get_set_values(|gid| {
        #[allow(clippy::cast_possible_truncation)] // a 16-bit glyph id, as the C++ `unsigned`
        glyphs.push(gid as u32);
    });
    // `make_subset` (flags: `RETAIN_GIDS`, plus `NOTDEF_OUTLINE` when glyph 0 is in the set),
    // `hb_subset_or_fail`, `hb_face_reference_blob` and `to_data` are in `subset_font`.
    subset_font(font_data, glyphs, ttc_index)
}

/// `SkPDFCanSubsetTableBasedFonts`: `hb_version_atleast(4, 4, 0)`. The oracle's `HarfBuzz` is the
/// pinned 13.1.0 (`DEPS#L55`), so this is true.
// Port of: src/pdf/SkPDFSubsetFont.cpp#L191-L198 (chrome/m156)
#[doc(alias = "SkPDFCanSubsetTableBasedFonts")]
#[must_use]
pub fn pdf_can_subset_table_based_fonts() -> bool {
    true
}
