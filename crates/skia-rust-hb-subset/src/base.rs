// Copyright © 2016  Elie Roux <elie.roux@telecom-bretagne.eu>
// Copyright © 2018  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-ot-layout-base-table.hh (harfbuzz 9cb1fee5)

//! The `BASE` table subsetter. A `BASE` table with an `ItemVariationStore` (version 1.1 and a
//! variation store offset) is not ported; Skia does not subset variable fonts.

use crate::Res;
use crate::bytes::u16_at;
use crate::ot::{INVALID, View, offset_subset, serialize_copy_device};
use crate::plan::{Plan, unsupported};
use crate::serialize::{ERROR_INT_OVERFLOW, Serializer};

fn bytes_of(v: View<'_>, off: usize, len: usize) -> Vec<u8> {
    (0..len)
        .map(|k| v.d.get(off + k).copied().unwrap_or(0))
        .collect()
}

/// `BASE::subset` (hb-ot-layout-base-table.hh#L598-L618).
pub(crate) fn subset(plan: &Plan<'_>, s: &mut Serializer, base: View<'_>) -> Res<bool> {
    let out = s.allocate(8);
    s.set_u32(out, base.u32(0));
    if base.u32(0) >= 0x0001_0001 && base.u32(8) != 0 {
        return unsupported("BASE ItemVariationStore");
    }
    if !base.is_null16(4) && !offset_subset(s, out + 4, base, 4, |s, a| axis_subset(plan, s, a)) {
        return Ok(false);
    }
    if !base.is_null16(6) && !offset_subset(s, out + 6, base, 6, |s, a| axis_subset(plan, s, a)) {
        return Ok(false);
    }
    Ok(true)
}

/// `Axis::subset`.
fn axis_subset(plan: &Plan<'_>, s: &mut Serializer, axis: View<'_>) -> bool {
    let out = s.embed(&bytes_of(axis, 0, 4));
    // `baseTagList.serialize_copy (...)`: `SortedArray16Of<Tag>::copy`
    s.zero_field(out, 2);
    if !axis.is_null16(0) {
        let list = axis.off16(0);
        s.push();
        let n = list.u16(0) as usize;
        s.embed_u16(n as u16);
        s.embed(&bytes_of(list, 2, 4 * n));
        let idx = s.pop_pack(true);
        s.add_link(out, 2, idx, crate::serialize::Whence::Head, 0);
    }
    offset_subset(s, out + 2, axis, 2, |s, l| script_list_subset(plan, s, l))
}

/// `BaseScriptList::subset`.
fn script_list_subset(plan: &Plan<'_>, s: &mut Serializer, list: View<'_>) -> bool {
    let out = s.allocate(2);
    let mut len = 0u32;
    for i in 0..list.u16(0) as usize {
        let rec = 2 + 6 * i;
        // `layout_scripts` has every script.
        let pos = s.embed(&bytes_of(list, rec, 6));
        if !offset_subset(s, pos + 4, list, rec + 4, |s, b| {
            base_script_subset(plan, s, b)
        }) {
            return false;
        }
        len += 1;
    }
    check_len(s, out, len)
}

fn check_len(s: &mut Serializer, pos: usize, len: u32) -> bool {
    if !s.check_fits(u64::from(len), 16, ERROR_INT_OVERFLOW) {
        return false;
    }
    s.set_u16(pos, len as u16);
    true
}

/// `BaseScript::subset`.
fn base_script_subset(plan: &Plan<'_>, s: &mut Serializer, bs: View<'_>) -> bool {
    let out = s.allocate(6);
    if !bs.is_null16(0) && !offset_subset(s, out, bs, 0, |s, v| base_values_subset(plan, s, v)) {
        return false;
    }
    if !bs.is_null16(2) && !offset_subset(s, out + 2, bs, 2, |s, m| min_max_subset(plan, s, m)) {
        return false;
    }
    let n = bs.u16(4) as usize;
    for i in 0..n {
        // `BaseLangSysRecord::subset`
        let rec = 6 + 6 * i;
        let pos = s.embed(&bytes_of(bs, rec, 6));
        if !offset_subset(s, pos + 4, bs, rec + 4, |s, m| min_max_subset(plan, s, m)) {
            return false;
        }
    }
    check_len(s, out + 4, n as u32)
}

/// `BaseValues::subset`.
fn base_values_subset(plan: &Plan<'_>, s: &mut Serializer, bv: View<'_>) -> bool {
    let out = s.allocate(4);
    s.set_u16(out, bv.u16(0) as u16);
    for i in 0..bv.u16(2) as usize {
        let snap = s.snapshot();
        let o = s.array_append(out + 2, 2);
        let ret = offset_subset(s, o, bv, 4 + 2 * i, |s, c| coord_subset(plan, s, c));
        if !ret {
            s.array_pop(out + 2);
            s.revert(snap);
            return false;
        }
    }
    u16_at(s.bytes(), out + 2) != 0
}

/// `MinMax::subset`.
fn min_max_subset(plan: &Plan<'_>, s: &mut Serializer, mm: View<'_>) -> bool {
    let out = s.allocate(6);
    if !offset_subset(s, out, mm, 0, |s, c| coord_subset(plan, s, c))
        || !offset_subset(s, out + 2, mm, 2, |s, c| coord_subset(plan, s, c))
    {
        return false;
    }
    let mut len = 0u32;
    for i in 0..mm.u16(4) as usize {
        let rec = 6 + 8 * i;
        if !plan.layout_features.contains(&mm.u32(rec)) {
            continue;
        }
        // `FeatMinMaxRecord::subset`
        let pos = s.embed(&bytes_of(mm, rec, 8));
        if !offset_subset(s, pos + 4, mm, rec + 4, |s, c| coord_subset(plan, s, c)) {
            return false;
        }
        if !offset_subset(s, pos + 6, mm, rec + 6, |s, c| coord_subset(plan, s, c)) {
            return false;
        }
        len += 1;
    }
    check_len(s, out + 4, len)
}

/// `BaseCoord::subset`.
fn coord_subset(plan: &Plan<'_>, s: &mut Serializer, c: View<'_>) -> bool {
    match c.u16(0) {
        1 => {
            s.embed(&bytes_of(c, 0, 4));
            true
        }
        2 => {
            let out = s.embed(&bytes_of(c, 0, 8));
            let g = plan.glyph_map.get(&c.u16(4)).copied().unwrap_or(INVALID);
            if !s.check_fits(u64::from(g), 16, ERROR_INT_OVERFLOW) {
                return false;
            }
            s.set_u16(out + 4, g as u16);
            true
        }
        3 => {
            // `pinned_at_default` holds: the variation index map is not consulted.
            let out = s.embed(&bytes_of(c, 0, 6));
            serialize_copy_device(s, out + 4, c, 4)
        }
        _ => true,
    }
}
