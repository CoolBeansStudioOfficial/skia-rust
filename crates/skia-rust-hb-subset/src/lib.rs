// Copyright © 2018  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-subset.cc, src/hb-subset-input.cc, src/hb-subset-plan.cc,
// src/pdf/SkPDFSubsetFont.cpp (harfbuzz 9cb1fee5, skia chrome/m156)

//! The parts of `HarfBuzz`'s `hb-subset` that Skia's PDF backend uses, ported so that the subset
//! font bytes equal `HarfBuzz`'s.
//!
//! Skia calls `hb_subset_or_fail` with a glyph set, `HB_SUBSET_FLAGS_RETAIN_GIDS` and, when glyph
//! 0 is in use, `HB_SUBSET_FLAGS_NOTDEF_OUTLINE`; every other input set keeps its default
//! (`src/pdf/SkPDFSubsetFont.cpp#L132-L172`). [`subset_font`] is that call. The code paths that
//! only other inputs reach (requested unicodes, instancing with axis locations, hint dropping,
//! custom glyph maps, name overrides) are not ported.
//!
//! The crate is `unsafe`-free and depends on nothing but `std`. `oracle/subset-diff` builds
//! `HarfBuzz` at the pin and records the outputs for a corpus; `tests/subset_diff.rs` replays it.

// The ported arithmetic mirrors the C++ integer casts between the widths of the font structures.
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::similar_names)]
// The ported code keeps the single-letter names of the C++ (`d` for data, `s` for the serializer).
#![allow(clippy::many_single_char_names)]

mod bytes;
mod cff;
mod cmap;
mod color;
mod gdef;
mod glyf;
mod gpos;
mod gsub;
mod gsubgpos;
mod context;
mod layout;
mod os2_ranges;
mod ot;
mod plan;
mod serialize;
mod sfnt;
mod tables;

use std::collections::BTreeSet;

use bytes::tag;
use plan::Plan;
use sfnt::{Face, face_offsets};

/// `HB_SUBSET_FLAGS_RETAIN_GIDS`.
pub(crate) const FLAG_RETAIN_GIDS: u32 = 0x0000_0002;
/// `HB_SUBSET_FLAGS_NOTDEF_OUTLINE`.
pub(crate) const FLAG_NOTDEF_OUTLINE: u32 = 0x0000_0040;
/// `HB_SUBSET_FLAGS_NO_PRUNE_UNICODE_RANGES`.
pub(crate) const FLAG_NO_PRUNE_UNICODE_RANGES: u32 = 0x0000_0400;

/// Why a subset did not produce a font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsetError {
    /// `HarfBuzz` returns `nullptr` as well: an unusable face, a table that does not sanitize, a
    /// table that cannot be subset.
    Failed,
    /// The font reaches a part of `hb-subset` that is not ported yet (the string says which);
    /// `HarfBuzz` would produce a font.
    Unsupported(&'static str),
}

pub(crate) type Res<T> = Result<T, SubsetError>;

/// Port of `subset_harfbuzz` for a face made from the font data
/// (`src/pdf/SkPDFSubsetFont.cpp#L141-L211`): `font_data` is the typeface's stream (the whole
/// file, for a collection), `glyph_ids` the glyphs in use (`SkPDFGlyphUse::getSetValues`), and
/// `ttc_index` the face index. As in Skia, glyph 0 in `glyph_ids` selects
/// `HB_SUBSET_FLAGS_NOTDEF_OUTLINE`.
///
/// # Errors
/// [`SubsetError::Failed`] where `HarfBuzz` returns no face, [`SubsetError::Unsupported`] for a
/// font that needs a part of `hb-subset` that is not ported.
pub fn try_subset_font(
    font_data: &[u8],
    glyph_ids: impl IntoIterator<Item = u32>,
    ttc_index: u32,
) -> Result<Vec<u8>, SubsetError> {
    // `stream_to_face`: the format is minimally recognized first, then the glyph count is checked.
    let num_faces = face_offsets(font_data).len();
    if num_faces == 0 || ttc_index as usize >= num_faces {
        return Err(SubsetError::Failed);
    }
    let face = Face::create(font_data, ttc_index).ok_or(SubsetError::Failed)?;
    if face.num_glyphs() == 0 {
        return Err(SubsetError::Failed);
    }

    let glyphs: BTreeSet<u32> = glyph_ids.into_iter().collect();
    // `make_subset`
    let mut flags = FLAG_RETAIN_GIDS;
    if glyphs.contains(&0) {
        flags |= FLAG_NOTDEF_OUTLINE;
    }
    let blob = subset_or_fail(&face, flags, glyphs)?;
    // `to_data`: an empty blob is no data.
    if blob.is_empty() {
        Err(SubsetError::Failed)
    } else {
        Ok(blob)
    }
}

/// Port of `SkPDFSubsetFont` (`src/pdf/SkPDFSubsetFont.cpp#L215-L217`): the subset font bytes, or
/// `None` where Skia gets a null `SkData`. A font that needs a part of `hb-subset` that is not
/// ported yet is `None` too; use [`try_subset_font`] to tell the two apart.
#[doc(alias = "SkPDFSubsetFont")]
#[must_use]
pub fn subset_font(
    font_data: &[u8],
    glyph_ids: impl IntoIterator<Item = u32>,
    ttc_index: u32,
) -> Option<Vec<u8>> {
    try_subset_font(font_data, glyph_ids, ttc_index).ok()
}

/// Port of `hb_subset_or_fail` (hb-subset.cc#L283-L304) and `hb_subset_plan_execute_or_fail`
/// (hb-subset.cc#L318-L398).
fn subset_or_fail(face: &Face<'_>, flags: u32, glyphs: BTreeSet<u32>) -> Res<Vec<u8>> {
    if face.num_glyphs() == 0 {
        return Err(SubsetError::Failed);
    }
    let mut plan = Plan::new(face, flags, glyphs)?;

    // The tags of the source face, as a set (`pending_subset_tags`).
    let pending: BTreeSet<u32> = face
        .table_tags()
        .filter(|&t| !should_drop_table(&plan, t))
        .collect();
    for t in pending {
        subset_table(&mut plan, t)?;
    }
    Ok(plan.build())
}

/// Port of `_should_drop_table` (hb-subset.cc#L148-L196) for a plan without axis locations or
/// hinting flags.
fn should_drop_table(plan: &Plan<'_>, t: u32) -> bool {
    plan.drop_tables.contains(&t)
}

/// Port of `_subset_table` (hb-subset.cc#L212-L275).
fn subset_table(plan: &mut Plan<'_>, t: u32) -> Res<()> {
    if plan.no_subset_tables.contains(&t) {
        let bytes = plan.source.table(t).to_vec();
        plan.add_table(t, bytes);
        return Ok(());
    }

    if let Some(r) = layout::subset_table(plan, t) {
        return r.map(|_| ());
    }
    match &t.to_be_bytes() {
        b"glyf" => glyf::subset(plan).map(|_| ()),
        b"hdmx" => tables::subset_hdmx(plan).map(|_| ()),
        b"name" => tables::subset_name(plan).map(|_| ()),
        b"hmtx" => tables::subset_mtx(plan, true).map(|_| ()),
        b"vmtx" => tables::subset_mtx(plan, false).map(|_| ()),
        b"maxp" => tables::subset_maxp(plan).map(|_| ()),
        b"cmap" => {
            let cmap = cmap::Cmap::new(plan.source.table(t)).ok_or(SubsetError::Failed)?;
            cmap::subset(plan, &cmap).map(|_| ())
        }
        b"OS/2" => tables::subset_os2(plan).map(|_| ()),
        b"post" => tables::subset_post(plan).map(|_| ()),
        b"head" => {
            if plan.source.table_tags().any(|x| x == tag(b"glyf")) {
                // skip head, handled by glyf
                return Ok(());
            }
            tables::subset_head(plan).map(|_| ())
        }
        // No user axes locations: these are copied.
        b"STAT" | b"cvt " | b"fvar" | b"avar" | b"cvar" | b"MVAR" => {
            let bytes = plan.source.table(t).to_vec();
            plan.add_table(t, bytes);
            Ok(())
        }
        b"HVAR" | b"VVAR" | b"gvar" => Err(SubsetError::Unsupported("HVAR/VVAR/gvar")),
        b"CFF " | b"CFF2" | b"VORG" => Err(SubsetError::Unsupported("CFF/CFF2/VORG")),
        b"sbix" | b"COLR" | b"CPAL" | b"CBLC" => Err(SubsetError::Unsupported("colour tables")),
        // `hhea`, `vhea` and `loca` are skipped (handled by `hmtx`, `vmtx` and `glyf`), `CBDT` is
        // skipped (handled by `CBLC`), and every other table is dropped
        // (`HB_SUBSET_FLAGS_PASSTHROUGH_UNRECOGNIZED` is not set).
        _ => Ok(()),
    }
}
