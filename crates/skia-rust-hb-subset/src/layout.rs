// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-subset-plan-layout.cc (harfbuzz 9cb1fee5)

//! Layout (`GDEF`, `GSUB`, `GPOS`, `BASE`, `MATH`) planning and subsetting. Not ported yet:
//! every call reports `Unsupported` when the source font has the table.

use std::collections::BTreeSet;

use crate::Res;
use crate::bytes::tag;
use crate::plan::{Plan, unsupported};

#[allow(clippy::trivially_copy_pass_by_ref)] // a reference to a byte-string literal
fn has(plan: &Plan<'_>, t: &[u8; 4]) -> bool {
    !plan.source.table(tag(t)).is_empty()
}

/// Port of `layout_populate_gids_to_retain` (hb-subset-plan-layout.cc#L338-L368).
pub(crate) fn populate_gids_to_retain(plan: &mut Plan<'_>) -> Res<()> {
    if has(plan, b"GSUB") || has(plan, b"GPOS") {
        return unsupported("GSUB/GPOS glyph closure");
    }
    Ok(())
}

/// Port of `_math_closure` (hb-subset-plan.cc#L136-L146).
pub(crate) fn math_closure(plan: &mut Plan<'_>, _glyphs: &mut BTreeSet<u32>) -> Res<()> {
    if has(plan, b"MATH") {
        return unsupported("MATH");
    }
    Ok(())
}

/// Port of `_nameid_closure` (hb-subset-plan.cc#L432-L447).
pub(crate) fn nameid_closure(plan: &mut Plan<'_>) -> Res<()> {
    if has(plan, b"STAT") || has(plan, b"fvar") || has(plan, b"CPAL") {
        return unsupported("STAT/fvar/CPAL name id closure");
    }
    Ok(())
}

/// Port of `_hb_subset_table_layout` (hb-subset-table-layout.cc#L35-L50): `None` when the tag is
/// not a layout table.
pub(crate) fn subset_table(_plan: &mut Plan<'_>, t: u32) -> Option<Res<bool>> {
    match &t.to_be_bytes() {
        b"GDEF" | b"GSUB" | b"GPOS" | b"BASE" | b"MATH" => Some(unsupported("layout tables")),
        _ => None,
    }
}
