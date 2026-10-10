// Copyright © 2007,2008,2009  Red Hat, Inc.
// Copyright © 2010,2012  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/OT/Layout/GPOS/{ValueFormat,SinglePos,SinglePosFormat1,SinglePosFormat2,PairPos,
// PairPosFormat1,PairPosFormat2,PairSet,PairValueRecord,Anchor,AnchorFormat1,AnchorFormat2,
// AnchorFormat3,AnchorMatrix,MarkRecord,MarkArray,CursivePosFormat1,MarkBasePosFormat1,
// MarkLigPosFormat1,LigatureArray,MarkMarkPosFormat1,PosLookupSubTable}.hh (harfbuzz 9cb1fee5)

//! The `GPOS` lookup subtables other than the extension: single, pair, cursive, mark to base, mark
//! to ligature and mark to mark positioning, with 16 bit offsets. Skia's subset input has no
//! instancing (`normalized_coords` is empty, `layout_variation_idx_delta_map` is empty) and does
//! not set `NO_HINTING`, so value formats are kept as they are and variation devices are dropped.

use std::collections::{BTreeSet, HashMap};

use crate::context;
use crate::gsubgpos::Kind;
use crate::ot::{
    ClassDef, ClassDefPlan, ClassDefSubsetArgs, Coverage, INVALID, View, classdef_subset,
    coverage_serialize, coverage_subset, device_copy, offset_subset, serialize_copy_device,
};
use crate::plan::Plan;
use crate::serialize::{Serializer, Whence};

const NO_VARIATIONS_INDEX: u32 = 0xFFFF_FFFF;

/// `PosLookupSubTable::intersects`.
pub(crate) fn intersects(lookup_type: u32, sub: View<'_>, glyphs: &BTreeSet<u32>) -> bool {
    let format = sub.u16(0);
    match lookup_type {
        1 => matches!(format, 1 | 2) && Coverage(sub.off16(2)).intersects(glyphs),
        2 => match format {
            1 => pair1_intersects(sub, glyphs),
            2 => {
                Coverage(sub.off16(2)).intersects(glyphs)
                    && ClassDef(sub.off16(10)).intersects(glyphs)
            }
            _ => false,
        },
        3 => format == 1 && Coverage(sub.off16(2)).intersects(glyphs),
        4..=6 => {
            format == 1
                && Coverage(sub.off16(2)).intersects(glyphs)
                && Coverage(sub.off16(4)).intersects(glyphs)
        }
        7 => context::context_intersects(sub, glyphs),
        8 => context::chain_context_intersects(sub, glyphs),
        _ => false,
    }
}

/// `PairPosFormat1_3::intersects`.
fn pair1_intersects(sub: View<'_>, glyphs: &BTreeSet<u32>) -> bool {
    let cov = Coverage(sub.off16(2));
    let n = sub.u16(8) as usize;
    let len1 = value_len(sub.u16(4));
    let len2 = value_len(sub.u16(6));
    let record_size = 2 + 2 * (len1 + len2);
    cov.iter().into_iter().take(n).enumerate().any(|(i, g)| {
        glyphs.contains(&g) && {
            // `PairSet::intersects`
            let ps = sub.off16(10 + 2 * i);
            (0..ps.u16(0) as usize).any(|k| glyphs.contains(&ps.u16(2 + k * record_size)))
        }
    })
}

/// `PosLookupSubTable::dispatch (hb_subset_context_t)` for the subtables other than the extension.
pub(crate) fn subset(plan: &Plan<'_>, s: &mut Serializer, lookup_type: u32, sub: View<'_>) -> bool {
    match lookup_type {
        1 => single_subset(plan, s, sub),
        2 => match sub.u16(0) {
            1 => pair1_subset(plan, s, sub),
            2 => pair2_subset(plan, s, sub),
            _ => true,
        },
        3 => {
            if sub.u16(0) == 1 {
                cursive_subset(plan, s, sub)
            } else {
                true
            }
        }
        4..=6 => {
            if sub.u16(0) != 1 {
                return true;
            }
            match lookup_type {
                4 => mark_base_subset(plan, s, sub),
                5 => mark_lig_subset(plan, s, sub),
                _ => mark_mark_subset(plan, s, sub),
            }
        }
        7 => context::context_subset(plan, s, Kind::Gpos, sub),
        8 => context::chain_context_subset(plan, s, Kind::Gpos, sub),
        _ => true,
    }
}

fn map_gid(map: &HashMap<u32, u32>, g: u32) -> u32 {
    map.get(&g).copied().unwrap_or(INVALID)
}

/// `bytes[off..off + len]` of the view, zero padded.
fn bytes_of(v: View<'_>, off: usize, len: usize) -> Vec<u8> {
    let mut out: Vec<u8> =
        v.d.get(off..)
            .unwrap_or(&[])
            .iter()
            .copied()
            .take(len)
            .collect();
    out.resize(len, 0);
    out
}

// -------------------------------------------------------------------------------------------
// ValueFormat
// -------------------------------------------------------------------------------------------

/// `ValueFormat::get_len`.
fn value_len(format: u32) -> usize {
    (format & 0xFF).count_ones() as usize
}

/// `ValueFormat::copy_values` (ValueFormat.hh#L123-L166) with an empty variation delta map. The
/// values are at `vo` of `vals`; the device offsets are relative to `base`.
fn copy_values(
    s: &mut Serializer,
    format: u32,
    new_format: u32,
    base: View<'_>,
    vals: View<'_>,
    vo: usize,
) {
    if format == 0 {
        return;
    }
    let mut vo = vo;
    for flag in [0x1u32, 0x2, 0x4, 0x8] {
        if format & flag != 0 {
            // `copy_value`
            if new_format & flag != 0 {
                s.embed_u16(vals.u16(vo) as u16);
            }
            vo += 2;
        }
    }
    if format & 0x00F0 == 0 {
        return;
    }
    for flag in [0x10u32, 0x20, 0x40, 0x80] {
        if format & flag != 0 {
            // `add_delta_to_value` does nothing with an empty map; `copy_device`:
            if new_format & flag != 0 {
                let src_off = vals.u16(vo);
                let pos = s.embed_u16(src_off as u16);
                if src_off != 0 {
                    s.set_u16(pos, 0);
                    s.push();
                    let ok = device_copy(s, base.sub(src_off as usize));
                    if ok {
                        let idx = s.pop_pack(true);
                        s.add_link(pos, 2, idx, Whence::Head, 0);
                    } else {
                        s.pop_discard();
                    }
                }
            }
            vo += 2;
        }
    }
}

// -------------------------------------------------------------------------------------------
// SinglePos
// -------------------------------------------------------------------------------------------

/// `SinglePosFormat1_2::subset` and `SinglePosFormat2::subset` with `SinglePos_serialize`.
fn single_subset(plan: &Plan<'_>, s: &mut Serializer, sub: View<'_>) -> bool {
    let format = sub.u16(0);
    let glyphset = &plan.glyphset_gsub;
    let value_format = sub.u16(4);
    let sub_length = value_len(value_format);
    // `(new glyph id, offset of the values)` of each glyph.
    let it: Vec<(u32, usize)> = match format {
        1 => Coverage(sub.off16(2))
            .intersect_set(glyphset)
            .into_iter()
            .map(|g| (map_gid(&plan.glyph_map, g), 6))
            .collect(),
        2 => {
            let count = sub.u16(6) as usize;
            Coverage(sub.off16(2))
                .iter()
                .into_iter()
                .take(count)
                .enumerate()
                .filter(|(_, g)| glyphset.contains(g))
                .map(|(i, g)| (map_gid(&plan.glyph_map, g), 8 + 2 * i * sub_length))
                .collect()
        }
        _ => return true,
    };
    let ret = !it.is_empty();

    // `SinglePos::serialize`
    let pos = s.allocate(2);
    let mut out_format = 2;
    if !it.is_empty() {
        // `get_format`
        out_format = 1;
        'outer: for &(_, vo) in &it {
            for k in 0..sub_length {
                if sub.u16(vo + 2 * k) != sub.u16(it[0].1 + 2 * k) {
                    out_format = 2;
                    break 'outer;
                }
            }
        }
    }
    s.set_u16(pos, out_format as u16);
    let glyphs: Vec<u32> = it.iter().map(|p| p.0).collect();
    if out_format == 1 {
        // `SinglePosFormat1::serialize`
        let out = pos;
        s.allocate(4);
        s.set_u16(out + 4, value_format as u16);
        if let Some(&(_, vo)) = it.first() {
            copy_values(s, value_format, value_format, sub, sub, vo);
        }
        s.serialize_serialize(out + 2, 2, |s| coverage_serialize(s, &glyphs));
    } else {
        // `SinglePosFormat2::serialize`
        let out = pos;
        s.allocate(6);
        s.set_u16(out + 4, value_format as u16);
        s.set_u16(out + 6, it.len() as u16);
        for &(_, vo) in &it {
            copy_values(s, value_format, value_format, sub, sub, vo);
        }
        s.serialize_serialize(out + 2, 2, |s| coverage_serialize(s, &glyphs));
    }
    ret
}

// -------------------------------------------------------------------------------------------
// PairPos
// -------------------------------------------------------------------------------------------

/// `PairPosFormat1_3::subset`.
fn pair1_subset(plan: &Plan<'_>, s: &mut Serializer, sub: View<'_>) -> bool {
    let glyphset = &plan.glyphset_gsub;
    let out = s.allocate(10);
    s.set_u16(out, sub.u16(0) as u16);
    let vf0 = sub.u16(4);
    let vf1 = sub.u16(6);
    s.set_u16(out + 4, vf0 as u16);
    s.set_u16(out + 6, vf1 as u16);
    let cov = Coverage(sub.off16(2));
    let n = sub.u16(8) as usize;
    let mut new_coverage: Vec<u32> = Vec::new();
    for (i, g) in cov.iter().into_iter().take(n).enumerate() {
        if !glyphset.contains(&g) {
            continue;
        }
        let snap = s.snapshot();
        let o = s.array_append(out + 8, 2);
        let ret = offset_subset(s, o, sub, 10 + 2 * i, |s, ps| {
            pair_set_subset(plan, s, ps, vf0, vf1)
        });
        if ret {
            new_coverage.push(map_gid(&plan.glyph_map, g));
        } else {
            s.array_pop(out + 8);
            s.revert(snap);
        }
    }
    s.serialize_serialize(out + 2, 2, |s| coverage_serialize(s, &new_coverage));
    !new_coverage.is_empty()
}

/// `PairSet::subset` (PairSet.hh#L148-L190) with `PairValueRecord::subset`.
fn pair_set_subset(plan: &Plan<'_>, s: &mut Serializer, ps: View<'_>, vf0: u32, vf1: u32) -> bool {
    let snap = s.snapshot();
    let out = s.allocate(2);
    let len1 = value_len(vf0);
    let len2 = value_len(vf1);
    let record_size = 2 + 2 * (len1 + len2);
    let count = ps.u16(0) as usize;
    let mut num = 0u32;
    for i in 0..count {
        let rec = 2 + i * record_size;
        let second = ps.u16(rec);
        if plan.glyphset_gsub.contains(&second) {
            // `PairValueRecord::subset`
            s.embed_u16(map_gid(&plan.glyph_map, second) as u16);
            copy_values(s, vf0, vf0, ps, ps, rec + 2);
            copy_values(s, vf1, vf1, ps, ps, rec + 2 + 2 * len1);
            num += 1;
        }
    }
    s.set_u16(out, num as u16);
    if num == 0 {
        s.revert(snap);
    }
    num != 0
}

/// `PairPosFormat2_4::subset`.
fn pair2_subset(plan: &Plan<'_>, s: &mut Serializer, sub: View<'_>) -> bool {
    let out = s.allocate(16);
    s.set_u16(out, sub.u16(0) as u16);
    let cdp = ClassDefPlan {
        glyph_map_gsub: &plan.glyph_map_gsub,
        glyphset_gsub: &plan.glyphset_gsub,
        num_source_glyphs: plan.source.num_glyphs(),
    };
    let mut klass1_map: HashMap<u32, u32> = HashMap::new();
    if sub.is_null16(8) {
        s.zero_field(out + 8, 2);
    } else {
        let cd = ClassDef(sub.off16(8));
        let filter = Coverage(sub.off16(2));
        s.serialize_subset(out + 8, 2, true, |s| {
            classdef_subset(
                s,
                cd,
                &cdp,
                ClassDefSubsetArgs {
                    klass_map: Some(&mut klass1_map),
                    keep_empty_table: true,
                    use_class_zero: true,
                    glyph_filter: Some(filter),
                },
            )
        });
    }
    s.set_u16(out + 12, klass1_map.len() as u16);
    let mut klass2_map: HashMap<u32, u32> = HashMap::new();
    if sub.is_null16(10) {
        s.zero_field(out + 10, 2);
    } else {
        let cd = ClassDef(sub.off16(10));
        s.serialize_subset(out + 10, 2, true, |s| {
            classdef_subset(
                s,
                cd,
                &cdp,
                ClassDefSubsetArgs {
                    klass_map: Some(&mut klass2_map),
                    keep_empty_table: true,
                    use_class_zero: false,
                    glyph_filter: None,
                },
            )
        });
    }
    s.set_u16(out + 14, klass2_map.len() as u16);

    let vf1 = sub.u16(4);
    let vf2 = sub.u16(6);
    s.set_u16(out + 4, vf1 as u16);
    s.set_u16(out + 6, vf2 as u16);
    let len1 = value_len(vf1) as u32;
    let len2 = value_len(vf2) as u32;
    let total_len = len1 + len2;
    let class1_count = sub.u16(12);
    let class2_count = sub.u16(14);
    let class2_idxs: Vec<u32> = (0..class2_count)
        .filter(|c| klass2_map.contains_key(c))
        .collect();
    for class1_idx in (0..class1_count).filter(|c| klass1_map.contains_key(c)) {
        for &class2_idx in &class2_idxs {
            let idx = class1_idx
                .wrapping_mul(class2_count)
                .wrapping_add(class2_idx)
                .wrapping_mul(total_len) as usize;
            copy_values(s, vf1, vf1, sub, sub, 16 + 2 * idx);
            copy_values(s, vf2, vf2, sub, sub, 16 + 2 * (idx + len1 as usize));
        }
    }
    let cov = Coverage(sub.off16(2));
    let ret = if sub.is_null16(2) {
        s.zero_field(out + 2, 2);
        false
    } else {
        s.serialize_subset(out + 2, 2, true, |s| {
            coverage_subset(s, cov, plan.source.num_glyphs(), &plan.glyph_map_gsub)
        })
    };
    !klass1_map.is_empty() && !klass2_map.is_empty() && ret
}

// -------------------------------------------------------------------------------------------
// Anchors and anchor matrices
// -------------------------------------------------------------------------------------------

/// `Anchor::subset` (Anchor.hh#L97-L112): `NO_HINTING` is not set, so format 2 is kept.
fn anchor_subset(s: &mut Serializer, anchor: View<'_>) -> bool {
    match anchor.u16(0) {
        1 => {
            // `AnchorFormat1::copy`: `out->format = 1`.
            s.embed(&bytes_of(anchor, 0, 6));
            true
        }
        2 => {
            s.embed(&bytes_of(anchor, 0, 8));
            true
        }
        3 => anchor3_subset(s, anchor),
        _ => false,
    }
}

/// `Device::get_variation_index`.
fn device_variation_index(dev: View<'_>) -> u32 {
    if dev.u16(4) == 0x8000 {
        dev.u32(0)
    } else {
        NO_VARIATIONS_INDEX
    }
}

/// `AnchorFormat3::subset` (AnchorFormat3.hh#L71-L141) with an empty variation index map.
fn anchor3_subset(s: &mut Serializer, anchor: View<'_>) -> bool {
    let start = s.embed(&bytes_of(anchor, 0, 6));
    let x_dev = anchor.u16(6);
    let y_dev = anchor.u16(8);
    let x_varidx = if x_dev != 0 {
        device_variation_index(anchor.sub(x_dev as usize))
    } else {
        NO_VARIATIONS_INDEX
    };
    if x_varidx != NO_VARIATIONS_INDEX {
        // `layout_variation_idx_delta_map` has no entry for it.
        return false;
    }
    let y_varidx = if y_dev != 0 {
        device_variation_index(anchor.sub(y_dev as usize))
    } else {
        NO_VARIATIONS_INDEX
    };
    if y_varidx != NO_VARIATIONS_INDEX {
        return false;
    }
    // `is_variation_device` is false for the devices left here.
    let no_downgrade = x_dev != 0 || y_dev != 0;
    if !no_downgrade {
        s.set_u16(start, 1);
        return true;
    }
    let xpos = s.embed_u16(x_dev as u16);
    let ypos = s.embed_u16(y_dev as u16);
    serialize_copy_device(s, xpos, anchor, 6);
    serialize_copy_device(s, ypos, anchor, 8);
    true
}

/// `AnchorMatrix::offset_is_null`.
fn matrix_offset_is_null(m: View<'_>, row: u32, col: u32, num_cols: u32) -> bool {
    if row >= m.u16(0) || col >= num_cols {
        return true;
    }
    m.u16(2 + 2 * (row.wrapping_mul(num_cols).wrapping_add(col)) as usize) == 0
}

/// `AnchorMatrix::subset` (AnchorMatrix.hh#L81-L101).
fn anchor_matrix_subset(s: &mut Serializer, m: View<'_>, num_rows: u32, indexes: &[u32]) -> bool {
    if indexes.is_empty() {
        return false;
    }
    let out = s.allocate(2);
    s.set_u16(out, num_rows as u16);
    for &i in indexes {
        let field = 2 + 2 * i as usize;
        let pos = s.embed_u16(m.u16(field) as u16);
        offset_subset(s, pos, m, field, anchor_subset);
    }
    true
}

/// `Markclass_closure_and_remap_indexes` (MarkArray.hh#L146-L170).
fn markclass_closure(
    mark_cov: &[u32],
    ma: View<'_>,
    glyphset: &BTreeSet<u32>,
) -> HashMap<u32, u32> {
    let n = ma.u16(0) as usize;
    let orig_classes: BTreeSet<u32> = mark_cov
        .iter()
        .zip(0..n)
        .filter(|(g, _)| glyphset.contains(g))
        .map(|(_, i)| ma.u16(2 + 4 * i))
        .collect();
    let mut klass_mapping = HashMap::new();
    let mut idx = 0;
    for klass in orig_classes {
        if klass_mapping.contains_key(&klass) {
            continue;
        }
        klass_mapping.insert(klass, idx);
        idx += 1;
    }
    klass_mapping
}

/// `MarkArray::subset` (MarkArray.hh#L106-L136) with `MarkRecord::subset`.
fn mark_array_subset(
    plan: &Plan<'_>,
    s: &mut Serializer,
    ma: View<'_>,
    coverage: &[u32],
    klass_mapping: &HashMap<u32, u32>,
) -> bool {
    let out = s.allocate(2);
    let n = ma.u16(0) as usize;
    let mut ret = false;
    let mut new_length = 0u32;
    for (&g, i) in coverage.iter().zip(0..n) {
        if !plan.glyphset_gsub.contains(&g) {
            continue;
        }
        let rec = 2 + 4 * i;
        let pos = s.embed(&bytes_of(ma, rec, 4));
        let klass = ma.u16(rec);
        s.set_u16(pos, map_gid(klass_mapping, klass) as u16);
        ret |= offset_subset(s, pos + 2, ma, rec + 2, anchor_subset);
        new_length += 1;
    }
    s.set_u16(out, new_length as u16);
    ret
}

// -------------------------------------------------------------------------------------------
// CursivePos
// -------------------------------------------------------------------------------------------

/// `CursivePosFormat1::subset`.
fn cursive_subset(plan: &Plan<'_>, s: &mut Serializer, sub: View<'_>) -> bool {
    let cov = Coverage(sub.off16(2));
    let n = sub.u16(4) as usize;
    let it: Vec<(u32, usize)> = cov
        .iter()
        .into_iter()
        .take(n)
        .enumerate()
        .filter(|(_, g)| plan.glyphset_gsub.contains(g))
        .map(|(i, g)| (map_gid(&plan.glyph_map, g), i))
        .collect();
    let ret = !it.is_empty();
    // `serialize`
    let out = s.allocate(6);
    s.set_u16(out, 1);
    s.set_u16(out + 4, it.len() as u16);
    for &(_, i) in &it {
        let rec = 6 + 4 * i;
        let pos = s.embed(&bytes_of(sub, rec, 4));
        let mut r = false;
        r |= offset_subset(s, pos, sub, rec, anchor_subset);
        r |= offset_subset(s, pos + 2, sub, rec + 2, anchor_subset);
        let _ = r;
    }
    let glyphs: Vec<u32> = it.iter().map(|p| p.0).collect();
    s.serialize_serialize(out + 2, 2, |s| coverage_serialize(s, &glyphs));
    ret
}

// -------------------------------------------------------------------------------------------
// MarkBasePos, MarkLigPos, MarkMarkPos
// -------------------------------------------------------------------------------------------

/// The marks of a mark coverage that are retained: `(glyph, record index)`.
fn retained_marks(plan: &Plan<'_>, mark_cov: &[u32], ma: View<'_>) -> Vec<u32> {
    let n = ma.u16(0) as usize;
    mark_cov
        .iter()
        .zip(0..n)
        .filter(|(g, _)| plan.glyphset_gsub.contains(g))
        .map(|(&g, _)| g)
        .collect()
}

/// The part of the three mark attachment subsetters up to and including the mark array.
/// `glyph_map` is the map of the new mark coverage. Returns `(klass_mapping, mark coverage glyphs)`
/// or `None` when the subtable is dropped.
fn mark_prefix(
    plan: &Plan<'_>,
    s: &mut Serializer,
    sub: View<'_>,
    out: usize,
    mark_cov_field: usize,
    mark_array_field: usize,
    glyph_map: &HashMap<u32, u32>,
) -> Option<HashMap<u32, u32>> {
    let mark_cov: Vec<u32> = Coverage(sub.off16(mark_cov_field)).iter();
    let ma = sub.off16(mark_array_field);
    let klass_mapping = markclass_closure(&mark_cov, ma, &plan.glyphset_gsub);
    if klass_mapping.is_empty() {
        return None;
    }
    s.set_u16(out + 6, klass_mapping.len() as u16);
    let new_coverage: Vec<u32> = retained_marks(plan, &mark_cov, ma)
        .into_iter()
        .map(|g| map_gid(glyph_map, g))
        .collect();
    if !s.serialize_serialize(out + mark_cov_field, 2, |s| {
        coverage_serialize(s, &new_coverage)
    }) {
        return None;
    }
    if !offset_subset(s, out + mark_array_field, sub, mark_array_field, |s, ma| {
        mark_array_subset(plan, s, ma, &mark_cov, &klass_mapping)
    }) {
        return None;
    }
    Some(klass_mapping)
}

/// `MarkBasePosFormat1_2::subset`.
fn mark_base_subset(plan: &Plan<'_>, s: &mut Serializer, sub: View<'_>) -> bool {
    mark_base_or_mark_subset(plan, s, sub, false)
}

/// `MarkMarkPosFormat1_2::subset`.
fn mark_mark_subset(plan: &Plan<'_>, s: &mut Serializer, sub: View<'_>) -> bool {
    mark_base_or_mark_subset(plan, s, sub, true)
}

/// The two subsetters differ only in the number of rows of the second array: the number of
/// retained base glyphs with a non-empty row for mark to base, and the number of retained second
/// marks for mark to mark.
fn mark_base_or_mark_subset(
    plan: &Plan<'_>,
    s: &mut Serializer,
    sub: View<'_>,
    is_mark_mark: bool,
) -> bool {
    let glyphset = &plan.glyphset_gsub;
    let out = s.allocate(12);
    s.set_u16(out, sub.u16(0) as u16);
    let Some(klass_mapping) = mark_prefix(plan, s, sub, out, 2, 8, &plan.glyph_map) else {
        return false;
    };
    let class_count = sub.u16(6);
    let array = sub.off16(10);
    let count = array.u16(0) as usize;
    let base_cov = Coverage(sub.off16(4)).iter();
    let iter: Vec<(u32, u32)> = base_cov
        .iter()
        .zip(0..count)
        .filter(|(g, _)| glyphset.contains(g))
        .map(|(&g, row)| (g, row as u32))
        .collect();
    let mut new_coverage: Vec<u32> = Vec::new();
    let mut indexes: Vec<u32> = Vec::new();
    for &(g, row) in &iter {
        let non_empty = (0..class_count)
            .filter(|c| klass_mapping.contains_key(c))
            .any(|col| !matrix_offset_is_null(array, row, col, class_count));
        if !non_empty {
            continue;
        }
        new_coverage.push(map_gid(&plan.glyph_map, g));
        indexes.extend(
            (0..class_count)
                .filter(|c| klass_mapping.contains_key(c))
                .map(|col| row.wrapping_mul(class_count).wrapping_add(col)),
        );
    }
    if new_coverage.is_empty() {
        return false;
    }
    if !s.serialize_serialize(out + 4, 2, |s| coverage_serialize(s, &new_coverage)) {
        return false;
    }
    let rows = if is_mark_mark {
        iter.len()
    } else {
        new_coverage.len()
    } as u32;
    offset_subset(s, out + 10, sub, 10, |s, m| {
        anchor_matrix_subset(s, m, rows, &indexes)
    })
}

/// `MarkLigPosFormat1_2::subset` with `LigatureArray::subset`.
fn mark_lig_subset(plan: &Plan<'_>, s: &mut Serializer, sub: View<'_>) -> bool {
    let out = s.allocate(12);
    s.set_u16(out, sub.u16(0) as u16);
    let Some(klass_mapping) = mark_prefix(plan, s, sub, out, 2, 8, &plan.glyph_map_gsub) else {
        return false;
    };
    let class_count = sub.u16(6);
    let lig_cov = Coverage(sub.off16(4)).iter();
    let mut new_lig_coverage: Vec<u32> = Vec::new();
    if !offset_subset(s, out + 10, sub, 10, |s, la| {
        lig_array_subset(
            plan,
            s,
            la,
            &lig_cov,
            class_count,
            &klass_mapping,
            &mut new_lig_coverage,
        )
    }) {
        return false;
    }
    s.serialize_serialize(out + 4, 2, |s| coverage_serialize(s, &new_lig_coverage))
}

/// `LigatureArray::subset` (LigatureArray.hh#L40-L88).
fn lig_array_subset(
    plan: &Plan<'_>,
    s: &mut Serializer,
    la: View<'_>,
    coverage: &[u32],
    class_count: u32,
    klass_mapping: &HashMap<u32, u32>,
    new_coverage: &mut Vec<u32>,
) -> bool {
    let out = s.allocate(2);
    let n = la.u16(0) as usize;
    let mut ret = false;
    for (&g, i) in coverage.iter().zip(0..n) {
        if !plan.glyph_map_gsub.contains_key(&g) {
            continue;
        }
        let src = la.off16(2 + 2 * i);
        let rows = src.u16(0);
        let total = rows.wrapping_mul(class_count);
        let indexes: Vec<u32> = (0..total)
            .filter(|idx| klass_mapping.contains_key(&(idx % class_count)))
            .collect();
        let non_empty = indexes.iter().any(|&idx| {
            !matrix_offset_is_null(src, idx / class_count, idx % class_count, class_count)
        });
        if !non_empty {
            continue;
        }
        let pos = s.array_append(out, 2);
        ret |= offset_subset(s, pos, la, 2 + 2 * i, |s, m| {
            anchor_matrix_subset(s, m, rows, &indexes)
        });
        new_coverage.push(map_gid(&plan.glyph_map_gsub, g));
    }
    ret
}
