// Copyright © 2007,2008,2009  Red Hat, Inc.
// Copyright © 2010,2011,2012  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/OT/Layout/GDEF/GDEF.hh (harfbuzz 9cb1fee5)

//! `GDEF` subsetting, version 1.0 to 1.3 with 16 bit offsets. A `GDEF` with an
//! `ItemVariationStore` is not ported.

use std::collections::BTreeSet;

use crate::ot::{
    ClassDef, ClassDefPlan, ClassDefSubsetArgs, Coverage, INVALID, View, classdef_subset,
    coverage_serialize, coverage_subset,
};
use crate::plan::{Plan, unsupported};
use crate::serialize::Serializer;
use crate::{Res, SubsetError};

/// The `layout_variation_idx_delta_map` of the plan is empty unless the `GDEF` has a variation
/// store, which this port rejects; variation devices are therefore dropped (`VariationDevice::copy`).
pub(crate) fn gdef_has_var_store(gdef: View<'_>) -> bool {
    match gdef.u16(0) {
        1 => gdef.u32(0) >= 0x0001_0003 && gdef.u32(14) != 0,
        2 => gdef.u32(14 + 4) != 0,
        _ => false,
    }
}

/// `GDEF::has_mark_glyph_sets` for version 1.
fn markglyphsets_off(gdef: View<'_>) -> Option<View<'_>> {
    if gdef.u16(0) == 1 && gdef.u32(0) >= 0x0001_0002 && gdef.u16(12) != 0 {
        Some(gdef.off16(12))
    } else {
        None
    }
}

/// `remap_used_mark_sets` (hb-subset-plan-layout.cc#L36-L51): the indexes of the mark glyph sets
/// that intersect the glyph set, remapped to be contiguous.
pub(crate) fn remap_used_mark_sets(
    gdef: View<'_>,
    glyphset_gsub: &BTreeSet<u32>,
) -> std::collections::HashMap<u32, u32> {
    let mut map = std::collections::HashMap::new();
    let Some(sets) = markglyphsets_off(gdef) else {
        return map;
    };
    // `MarkGlyphSets::collect_used_mark_sets`, format 1 only.
    if sets.u16(0) != 1 {
        return map;
    }
    let mut used: BTreeSet<u32> = BTreeSet::new();
    for i in 0..sets.u16(2) as usize {
        let cov = Coverage(sets.off32(4 + 4 * i));
        if cov.intersects(glyphset_gsub) {
            used.insert(i as u32);
        }
    }
    // `remap_indexes`
    for (i, v) in used.iter().enumerate() {
        map.insert(*v, i as u32);
    }
    map
}

/// `GDEF::subset` (GDEF.hh#L938-L946) with `GDEFVersion1_2::subset` (L732-L805).
pub(crate) fn subset(plan: &Plan<'_>, s: &mut Serializer, gdef: View<'_>) -> Res<bool> {
    match gdef.u16(0) {
        1 => {}
        2 => return unsupported("GDEF with 24 bit offsets"),
        _ => return Ok(false),
    }
    let version = gdef.u32(0);
    let out = s.allocate(12); // extend_min

    // Push var store first (if it's needed) so that it's last in the serialization order.
    let snapshot_version0 = s.snapshot();
    let mut out_mgs_pos = 0usize;
    if version >= 0x0001_0002 {
        out_mgs_pos = s.embed(&gdef.d[12..14.min(gdef.d.len())]);
        if gdef.d.len() < 14 {
            return Err(SubsetError::Failed);
        }
    }
    let snapshot_version2 = s.snapshot();
    let mut subset_varstore = false;
    if version >= 0x0001_0003 {
        if gdef.d.len() < 18 {
            return Err(SubsetError::Failed);
        }
        s.embed(&gdef.d[14..18]);
        if gdef.u32(14) != 0 {
            return unsupported("GDEF ItemVariationStore");
        }
        // `serialize_subset` with a null offset: `*this = 0`, false.
        let var_store_pos = out + 14;
        s.zero_field(var_store_pos, 4);
        subset_varstore = false;
    }

    // out->version = version
    s.set_u32(out, version);

    if !subset_varstore && version >= 0x0001_0002 {
        s.revert(snapshot_version2);
    }

    let cdp = ClassDefPlan {
        glyph_map_gsub: &plan.glyph_map_gsub,
        glyphset_gsub: &plan.glyphset_gsub,
        num_source_glyphs: plan.source.num_glyphs(),
    };

    let mut subset_markglyphsetsdef = false;
    if version >= 0x0001_0002 {
        subset_markglyphsetsdef = if gdef.is_null16(12) {
            s.zero_field(out_mgs_pos, 2);
            false
        } else {
            let sets = gdef.off16(12);
            s.serialize_subset(out_mgs_pos, 2, true, |s| {
                mark_glyph_sets_subset(plan, s, sets)
            })
        };
    }

    let out_version;
    if subset_varstore {
        out_version = 0x0001_0003;
    } else if subset_markglyphsetsdef {
        out_version = 0x0001_0002;
    } else {
        out_version = 0x0001_0000;
        s.revert(snapshot_version0);
    }
    s.set_u32(out, out_version);

    let subset_glyphclassdef = subset_classdef(s, out + 4, gdef.is_null16(4), gdef.off16(4), &cdp);
    let subset_attachlist = if gdef.is_null16(6) {
        s.zero_field(out + 6, 2);
        false
    } else {
        let v = gdef.off16(6);
        s.serialize_subset(out + 6, 2, true, |s| attach_list_subset(plan, s, v))
    };
    let subset_markattachclassdef =
        subset_classdef(s, out + 10, gdef.is_null16(10), gdef.off16(10), &cdp);
    let subset_ligcaretlist = if gdef.is_null16(8) {
        s.zero_field(out + 8, 2);
        false
    } else {
        let v = gdef.off16(8);
        s.serialize_subset(out + 8, 2, true, |s| lig_caret_list_subset(plan, s, v))
    };

    Ok(subset_glyphclassdef
        || subset_attachlist
        || subset_ligcaretlist
        || subset_markattachclassdef
        || (out_version >= 0x0001_0002 && subset_markglyphsetsdef)
        || (out_version >= 0x0001_0003 && subset_varstore))
}

/// `out->classDef.serialize_subset (c, classDef, this, nullptr, false, true)`.
fn subset_classdef(
    s: &mut Serializer,
    pos: usize,
    is_null: bool,
    cd: View<'_>,
    cdp: &ClassDefPlan<'_>,
) -> bool {
    if is_null {
        s.zero_field(pos, 2);
        return false;
    }
    s.serialize_subset(pos, 2, true, |s| {
        classdef_subset(
            s,
            ClassDef(cd),
            cdp,
            ClassDefSubsetArgs {
                klass_map: None,
                keep_empty_table: false,
                use_class_zero: true,
                glyph_filter: None,
            },
        )
    })
}

/// `AttachList::subset` (GDEF.hh#L87-L109).
fn attach_list_subset(plan: &Plan<'_>, s: &mut Serializer, this: View<'_>) -> bool {
    let out = s.allocate(4); // extend_min: coverage, attachPoint.len
    let cov = Coverage(this.off16(0));
    let count = this.u16(2) as usize;
    let mut new_coverage: Vec<u32> = Vec::new();
    for (index, g) in cov.iter().into_iter().take(count).enumerate() {
        if !plan.glyphset_gsub.contains(&g) {
            continue;
        }
        // `subset_offset_array (c, out->attachPoint, this)`
        let snap = s.snapshot();
        let o = s.array_append(out + 2, 2);
        let off_field = 4 + 2 * index;
        let ret = if this.is_null16(off_field) {
            s.zero_field(o, 2);
            false
        } else {
            let point = this.off16(off_field);
            s.serialize_subset(o, 2, true, |s| {
                // `AttachPoint::subset`: Array16Of<HBUINT16>::serialize
                let n = point.u16(0) as usize;
                s.embed_u16(n as u16);
                for i in 0..n {
                    s.embed_u16(point.u16(2 + 2 * i) as u16);
                }
                true
            })
        };
        if !ret {
            s.array_pop(out + 2);
            s.revert(snap);
        } else {
            new_coverage.push(plan.glyph_map.get(&g).copied().unwrap_or(INVALID));
        }
    }
    let ok = !new_coverage.is_empty();
    s.serialize_serialize(out, 2, |s| coverage_serialize(s, &new_coverage));
    ok
}

/// `LigCaretList::subset` (GDEF.hh#L368-L390).
fn lig_caret_list_subset(plan: &Plan<'_>, s: &mut Serializer, this: View<'_>) -> bool {
    let out = s.allocate(4);
    let cov = Coverage(this.off16(0));
    let count = this.u16(2) as usize;
    let mut new_coverage: Vec<u32> = Vec::new();
    for (index, g) in cov.iter().into_iter().take(count).enumerate() {
        if !plan.glyphset_gsub.contains(&g) {
            continue;
        }
        let snap = s.snapshot();
        let o = s.array_append(out + 2, 2);
        let off_field = 4 + 2 * index;
        let ret = if this.is_null16(off_field) {
            s.zero_field(o, 2);
            false
        } else {
            let lig_glyph = this.off16(off_field);
            s.serialize_subset(o, 2, true, |s| lig_glyph_subset(plan, s, lig_glyph))
        };
        if !ret {
            s.array_pop(out + 2);
            s.revert(snap);
        } else {
            new_coverage.push(plan.glyph_map.get(&g).copied().unwrap_or(INVALID));
        }
    }
    let ok = !new_coverage.is_empty();
    s.serialize_serialize(out, 2, |s| coverage_serialize(s, &new_coverage));
    ok
}

/// `LigGlyph::subset` (GDEF.hh#L305-L318).
fn lig_glyph_subset(_plan: &Plan<'_>, s: &mut Serializer, this: View<'_>) -> bool {
    let out = s.allocate(2);
    for i in 0..this.u16(0) as usize {
        let snap = s.snapshot();
        let o = s.array_append(out, 2);
        let off_field = 2 + 2 * i;
        let ret = if this.is_null16(off_field) {
            s.zero_field(o, 2);
            false
        } else {
            let caret = this.off16(off_field);
            s.serialize_subset(o, 2, true, |s| caret_value_subset(s, caret))
        };
        if !ret {
            s.array_pop(out);
            s.revert(snap);
        }
    }
    View::new(s.bytes()).u16(0) != 0
}

/// `CaretValue::subset` through `dispatch` (GDEF.hh#L113-L246).
fn caret_value_subset(s: &mut Serializer, this: View<'_>) -> bool {
    match this.u16(0) {
        1 | 2 => {
            // `c->serializer->embed (this)`
            s.embed(&this.d[..4.min(this.d.len())]);
            this.d.len() >= 4
        }
        3 => {
            // `CaretValueFormat3::subset`
            let out = s.length();
            s.embed(&this.d[..4.min(this.d.len())]); // caretValueFormat, coordinate
            // The variation index of a hinting device is `HB_OT_LAYOUT_NO_VARIATIONS_INDEX`
            // and the plan's map has no entry for it.
            let dev_pos = s.embed(&this.d[4.min(this.d.len())..6.min(this.d.len())]);
            let _ = out;
            if this.is_null16(4) {
                s.zero_field(dev_pos, 2);
                return false;
            }
            // `out->deviceTable.serialize_copy (c, deviceTable, this, to_bias (out), Head, map)`
            s.zero_field(dev_pos, 2);
            s.push();
            let ret = crate::ot::device_copy(s, this.off16(4));
            let idx = s.pop_pack(true);
            s.add_link(dev_pos, 2, idx, crate::serialize::Whence::Head, 0);
            ret
        }
        _ => true,
    }
}

/// `MarkGlyphSets::subset` through `MarkGlyphSetsFormat1::subset` (GDEF.hh#L452-L488).
fn mark_glyph_sets_subset(plan: &Plan<'_>, s: &mut Serializer, this: View<'_>) -> bool {
    if this.u16(0) != 1 {
        return false;
    }
    let out = s.allocate(4);
    s.set_u16(out, 1);
    let mut ret = true;
    for i in 0..this.u16(2) as usize {
        let snap = s.snapshot();
        let o = s.array_append(out + 2, 4);
        // The coverage array holds 32 bit offsets.
        s.push();
        let mut res = false;
        if !this.is_null32(4 + 4 * i) {
            let cov = Coverage(this.off32(4 + 4 * i));
            res = coverage_subset(s, cov, plan.source.num_glyphs(), &plan.glyph_map_gsub);
        }
        if !res {
            s.pop_discard();
            s.revert(snap);
            // `(out->coverage.len)--`
            let len = View::new(s.bytes()).u16(2);
            s.set_u16(out + 2, (len as u16).wrapping_sub(1));
            continue;
        }
        let idx = s.pop_pack(true);
        s.add_link(o, 4, idx, crate::serialize::Whence::Head, 0);
    }
    let _ = &mut ret;
    ret && View::new(s.bytes()).u16(2) != 0
}
