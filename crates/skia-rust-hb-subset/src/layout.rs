// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-subset-plan-layout.cc, src/hb-subset-table-layout.cc (harfbuzz 9cb1fee5)

//! Layout (`GDEF`, `GSUB`, `GPOS`, `BASE`, `MATH`) planning and subsetting. `GSUB`, `GPOS`,
//! `BASE` and `MATH` are not ported yet: every call reports `Unsupported` when the source font has
//! the table.

use std::collections::BTreeSet;

use crate::Res;
use crate::bytes::tag;
use crate::ot::View;
use crate::gsubgpos::{Kind, TablePlan};
use crate::plan::{Plan, unsupported};
use crate::serialize::Serializer;

/// The layout members of `hb_subset_plan_t` (hb-subset-plan-member-list.hh).
#[derive(Default)]
pub(crate) struct LayoutPlan {
    pub gsub: TablePlan,
    pub gpos: TablePlan,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // a reference to a byte-string literal
fn has(plan: &Plan<'_>, t: &[u8; 4]) -> bool {
    !plan.source.table(tag(t)).is_empty()
}

/// Port of `layout_populate_gids_to_retain` (hb-subset-plan-layout.cc#L338-L368).
pub(crate) fn populate_gids_to_retain(plan: &mut Plan<'_>) -> Res<()> {
    crate::gsubgpos::populate_gids_to_retain(plan)
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

/// Port of `_hb_subset_table<T>` (hb-subset-table.hh#L88-L150) for a table that serializes with
/// the object serializer: `None` is returned by `end_serialize` on an error.
pub(crate) fn run_table(
    plan: &mut Plan<'_>,
    t: u32,
    subset: impl FnOnce(&Plan<'_>, &mut Serializer, View<'_>) -> Res<bool>,
) -> Res<bool> {
    let data = plan.source.table(t);
    if data.is_empty() {
        return Err(crate::SubsetError::Failed);
    }
    let mut s = Serializer::new();
    s.start_serialize();
    let needed = subset(plan, &mut s, View::new(data))?;
    let out = s.end_serialize();
    if s.in_error() && !s.only_offset_overflow() {
        return Err(crate::SubsetError::Failed);
    }
    if !needed {
        return Ok(true);
    }
    match out {
        Some(bytes) => {
            plan.add_table(t, bytes);
            Ok(true)
        }
        None => unsupported("offset overflow (hb-repacker)"),
    }
}

/// Port of `_hb_subset_table_layout` (hb-subset-table-layout.cc#L35-L50): `None` when the tag is
/// not a layout table.
pub(crate) fn subset_table(plan: &mut Plan<'_>, t: u32) -> Option<Res<bool>> {
    match &t.to_be_bytes() {
        b"GDEF" => Some(run_table(plan, t, |plan, s, v| crate::gdef::subset(plan, s, v))),
        b"GSUB" => Some(run_table(plan, t, |plan, s, v| crate::gsubgpos::subset(plan, s, v, Kind::Gsub))),
        b"GPOS" => Some(run_table(plan, t, |plan, s, v| crate::gsubgpos::subset(plan, s, v, Kind::Gpos))),
        b"BASE" | b"MATH" => Some(unsupported("BASE/MATH")),
        _ => None,
    }
}
