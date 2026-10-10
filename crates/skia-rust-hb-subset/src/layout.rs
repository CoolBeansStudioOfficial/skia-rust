// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-subset-plan-layout.cc, src/hb-subset-table-layout.cc (harfbuzz 9cb1fee5)

//! Layout (`GDEF`, `GSUB`, `GPOS`, `BASE`, `MATH`) planning and subsetting. `GSUB`, `GPOS`,
//! `BASE` and `MATH` are not ported yet: every call reports `Unsupported` when the source font has
//! the table.

use std::collections::{BTreeSet, HashMap};

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
pub(crate) fn math_closure(plan: &mut Plan<'_>, glyphs: &mut BTreeSet<u32>) -> Res<()> {
    crate::math::closure_glyphs(View::new(plan.source.table(tag(b"MATH"))), glyphs);
    Ok(())
}

/// Port of `_nameid_closure` (hb-subset-plan.cc#L432-L447), without axis locations.
pub(crate) fn nameid_closure(plan: &mut Plan<'_>) -> Res<()> {
    if !plan.drop_tables.contains(&tag(b"STAT")) {
        let ids = stat_name_ids(plan.source.table(tag(b"STAT")));
        plan.name_ids.extend(ids);
    }
    // `!plan->all_axes_pinned`
    let ids = fvar_name_ids(plan.source.table(tag(b"fvar")));
    plan.name_ids.extend(ids);
    if !plan.drop_tables.contains(&tag(b"CPAL")) {
        let ids = cpal_name_ids(plan.source.table(tag(b"CPAL")), &plan.colr_palettes);
        plan.name_ids.extend(ids);
    }
    // `layout_nameid_closure`
    if !plan.drop_tables.contains(&tag(b"GPOS")) {
        crate::gsubgpos::collect_name_ids(plan, Kind::Gpos);
    }
    if !plan.drop_tables.contains(&tag(b"GSUB")) {
        crate::gsubgpos::collect_name_ids(plan, Kind::Gsub);
    }
    Ok(())
}

/// `STAT::collect_name_ids` (hb-ot-stat-table.hh#L518-L541) with no user axes location: every
/// axis value of a known format is kept.
fn stat_name_ids(data: &[u8]) -> Vec<u32> {
    let v = View::new(data);
    let mut out = Vec::new();
    if v.u32(0) == 0 {
        return out;
    }
    // `NNOffset32To`: an offset of 0 is the table itself.
    let axes = v.sub(v.u32(8) as usize);
    for i in 0..v.u16(6) as usize {
        out.push(axes.u16(8 * i + 4));
    }
    let offsets = v.sub(v.u32(14) as usize);
    for i in 0..v.u16(12) as usize {
        let off = offsets.u16(2 * i) as usize;
        if off == 0 {
            // `Null (AxisValue)`: format 0, not kept.
            continue;
        }
        let value = offsets.sub(off);
        if matches!(value.u16(0), 1..=4) {
            out.push(value.u16(6));
        }
    }
    out.push(v.u16(18));
    out
}

/// `fvar::collect_name_ids` (hb-ot-var-fvar-table.hh#L373-L402) with no user axes location.
fn fvar_name_ids(data: &[u8]) -> Vec<u32> {
    let v = View::new(data);
    let mut out = Vec::new();
    if v.u32(0) == 0 {
        return out;
    }
    let axis_count = v.u16(8) as usize;
    let first_axis = v.u16(4) as usize;
    for i in 0..axis_count {
        out.push(v.u16(first_axis + 20 * i + 18));
    }
    let instance_size = v.u16(14) as usize;
    let instances = first_axis + axis_count * 20;
    for i in 0..v.u16(12) as usize {
        let inst = instances + i * instance_size;
        out.push(v.u16(inst));
        if instance_size >= axis_count * 4 + 6 {
            let ps = v.u16(inst + 4 + axis_count * 4);
            if ps != 0xFFFF {
                out.push(ps);
            }
        }
    }
    out
}

/// `CPAL::collect_name_ids` (CPAL.hh#L221-L229).
fn cpal_name_ids(data: &[u8], colr_palettes: &HashMap<u32, u32>) -> Vec<u32> {
    let v = View::new(data);
    let mut out = Vec::new();
    if v.u16(0) != 1 {
        return out;
    }
    let color_count = v.u16(2) as usize;
    let palette_count = v.u16(4) as usize;
    let tail = 12 + 2 * palette_count;
    // `NNOffset32To`
    let palette_labels = v.u32(tail + 4) as usize;
    if palette_labels != 0 {
        let a = v.sub(palette_labels);
        for i in 0..palette_count {
            out.push(a.u16(2 * i));
        }
    }
    let color_labels = v.u32(tail + 8) as usize;
    if color_labels != 0 {
        let a = v.sub(color_labels);
        for i in 0..color_count {
            if colr_palettes.contains_key(&(i as u32)) {
                out.push(a.u16(2 * i));
            }
        }
    }
    out
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
    if !plan.account_table(t, data.len(), s.peak()) {
        return Err(crate::SubsetError::Failed);
    }
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
        b"MATH" => Some(run_table(plan, t, crate::math::subset)),
        b"BASE" => Some(unsupported("BASE")),
        _ => None,
    }
}
