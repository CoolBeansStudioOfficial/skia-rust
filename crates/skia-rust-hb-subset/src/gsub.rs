// Copyright © 2007,2008,2009  Red Hat, Inc.
// Copyright © 2010,2011,2012  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/OT/Layout/GSUB/{SingleSubst,SingleSubstFormat1,SingleSubstFormat2,MultipleSubst,
// MultipleSubstFormat1,Sequence,AlternateSubst,AlternateSubstFormat1,AlternateSet,LigatureSubst,
// LigatureSubstFormat1,LigatureSet,Ligature,ReverseChainSingleSubst,
// ReverseChainSingleSubstFormat1}.hh (harfbuzz 9cb1fee5)

//! The `GSUB` lookup subtables other than the context ones: single, multiple, alternate, ligature
//! and reverse chaining single substitution, formats with 16 bit glyph ids.

use std::collections::BTreeSet;

use crate::context;
use crate::gsubgpos::{ClosureCtx, Kind};
use crate::ot::{
    Coverage, INVALID, NOT_COVERED, View, coverage_serialize, coverage_subset, set_next,
};
use crate::plan::Plan;
use crate::serialize::{ERROR_INT_OVERFLOW, Serializer, Whence};

/// `SubstLookupSubTable::intersects`.
pub(crate) fn intersects(lookup_type: u32, sub: View<'_>, glyphs: &BTreeSet<u32>) -> bool {
    match lookup_type {
        1..=3 => {
            // Single (formats 1, 2), Multiple and Alternate (format 1): `coverage.intersects`.
            let ok_format = match lookup_type {
                1 => matches!(sub.u16(0), 1 | 2),
                _ => sub.u16(0) == 1,
            };
            ok_format && Coverage(sub.off16(2)).intersects(glyphs)
        }
        4 => {
            // LigatureSubstFormat1::intersects
            if sub.u16(0) != 1 {
                return false;
            }
            let cov = Coverage(sub.off16(2));
            let n = sub.u16(4) as usize;
            cov.iter().into_iter().take(n).enumerate().any(|(i, g)| {
                glyphs.contains(&g) && {
                    let ls = sub.off16(6 + 2 * i);
                    (0..ls.u16(0) as usize)
                        .any(|k| ligature_intersects(ls.off16(2 + 2 * k), glyphs))
                }
            })
        }
        5 => context::context_intersects(sub, glyphs),
        6 => context::chain_context_intersects(sub, glyphs),
        8 => {
            // ReverseChainSingleSubstFormat1::intersects
            if sub.u16(0) != 1 {
                return false;
            }
            if !Coverage(sub.off16(2)).intersects(glyphs) {
                return false;
            }
            let bt_len = sub.u16(4) as usize;
            for i in 0..bt_len {
                if !Coverage(sub.off16(6 + 2 * i)).intersects(glyphs) {
                    return false;
                }
            }
            let la_pos = 6 + 2 * bt_len;
            let la_len = sub.u16(la_pos) as usize;
            for i in 0..la_len {
                if !Coverage(sub.off16(la_pos + 2 + 2 * i)).intersects(glyphs) {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
}

/// `Ligature::intersects`: `hb_all (component, glyphs)`.
fn ligature_intersects(lig: View<'_>, glyphs: &BTreeSet<u32>) -> bool {
    let n = lig.u16(2).saturating_sub(1) as usize;
    (0..n).all(|i| glyphs.contains(&lig.u16(4 + 2 * i)))
}

/// `SubstLookupSubTable::dispatch (hb_closure_context_t)` for the subtables other than the
/// extension.
pub(crate) fn closure(c: &mut ClosureCtx<'_>, lookup_type: u32, sub: View<'_>) {
    match lookup_type {
        1 => single_closure(c, sub),
        2 | 3 => {
            // MultipleSubstFormat1_2::closure and AlternateSubstFormat1_2::closure: the glyphs of
            // every sequence/alternate set whose first glyph is active.
            if sub.u16(0) != 1 {
                return;
            }
            let cov = Coverage(sub.off16(2));
            let n = sub.u16(4) as usize;
            let active = c.parent_active_glyphs().clone();
            for (i, g) in cov.iter().into_iter().take(n).enumerate() {
                if !active.contains(&g) {
                    continue;
                }
                let set = sub.off16(6 + 2 * i);
                for k in 0..set.u16(0) as usize {
                    c.output.insert(set.u16(2 + 2 * k));
                }
            }
        }
        4 => {
            // LigatureSubstFormat1_2::closure
            if sub.u16(0) != 1 {
                return;
            }
            let cov = Coverage(sub.off16(2));
            let n = sub.u16(4) as usize;
            let active = c.parent_active_glyphs().clone();
            for (i, g) in cov.iter().into_iter().take(n).enumerate() {
                if !active.contains(&g) {
                    continue;
                }
                let ls = sub.off16(6 + 2 * i);
                for k in 0..ls.u16(0) as usize {
                    let lig = ls.off16(2 + 2 * k);
                    // Ligature::closure
                    if !ligature_intersects(lig, c.glyphs) {
                        continue;
                    }
                    c.output.insert(lig.u16(0));
                }
            }
        }
        5 => context::context_closure(c, sub),
        6 => context::chain_context_closure(c, sub),
        8 => {
            // ReverseChainSingleSubstFormat1::closure
            if sub.u16(0) != 1 || !intersects(8, sub, c.glyphs) {
                return;
            }
            let cov = Coverage(sub.off16(2));
            let bt_len = sub.u16(4) as usize;
            let la_pos = 6 + 2 * bt_len;
            let la_len = sub.u16(la_pos) as usize;
            let sub_pos = la_pos + 2 + 2 * la_len;
            let active = c.parent_active_glyphs().clone();
            let n = sub.u16(sub_pos) as usize;
            for (i, g) in cov.iter().into_iter().take(n).enumerate() {
                if active.contains(&g) {
                    c.output.insert(sub.u16(sub_pos + 2 + 2 * i));
                }
            }
        }
        _ => {}
    }
}

/// `SingleSubstFormat1_3::closure` and `SingleSubstFormat2_4::closure`.
fn single_closure(c: &mut ClosureCtx<'_>, sub: View<'_>) {
    match sub.u16(0) {
        1 => {
            let d = sub.u16(4);
            let mask = 0xFFFFu32;
            let cov = Coverage(sub.off16(2));
            let pop = cov.get_population();
            if pop >= mask {
                return;
            }
            let intersection = cov.intersect_set(c.parent_active_glyphs());
            let min_before = intersection.first().copied().unwrap_or(INVALID);
            let max_before = intersection.last().copied().unwrap_or(INVALID);
            let min_after = min_before.wrapping_add(d) & mask;
            let max_after = max_before.wrapping_add(d) & mask;
            if intersection.len() as u32 == max_before.wrapping_sub(min_before).wrapping_add(1)
                && ((min_before <= min_after && min_after <= max_before)
                    || (min_before <= max_after && max_after <= max_before))
            {
                return;
            }
            for g in intersection {
                c.output.insert(g.wrapping_add(d) & mask);
            }
        }
        2 => {
            let cov = Coverage(sub.off16(2));
            let len = sub.u16(4);
            let glyph_set = c.parent_active_glyphs().clone();
            if len > glyph_set.len() as u32 * 4 {
                for &g in &glyph_set {
                    let i = cov.get_coverage(g);
                    if i == NOT_COVERED || i >= len {
                        continue;
                    }
                    c.output.insert(sub.u16(6 + 2 * i as usize));
                }
                return;
            }
            for (i, g) in cov.iter().into_iter().take(len as usize).enumerate() {
                if glyph_set.contains(&g) {
                    c.output.insert(sub.u16(6 + 2 * i));
                }
            }
        }
        _ => {}
    }
}

/// `SingleSubst_serialize` / `SingleSubst::serialize` (SingleSubst.hh#L77-L124): `it` holds
/// `(old new-gid, substitute new-gid)` sorted by the first.
fn single_subst_serialize(s: &mut Serializer, it: &[(u32, u32)]) {
    let start = s.allocate(2);
    let mut format = 2u32;
    let mut delta = 0u32;
    if !it.is_empty() {
        format = 1;
        let mask = 0xFFFFu32;
        // BEYOND_64K: `if any substitute > 0xFFFF { format += 2; mask = 0xFFFFFF }`
        if it.iter().any(|p| p.1 > 0xFFFF) {
            s.err(ERROR_INT_OVERFLOW);
            return;
        }
        let get_delta = |p: &(u32, u32)| p.1.wrapping_sub(p.0) & mask;
        delta = get_delta(&it[0]);
        if !it[1..].iter().all(|p| get_delta(p) == delta) {
            format += 1;
        }
    }
    s.set_u16(start, format as u16);
    let glyphs: Vec<u32> = it.iter().map(|p| p.0).collect();
    if format == 1 {
        // SingleSubstFormat1_3::serialize
        let cov_pos = s.allocate(4);
        s.serialize_serialize(cov_pos, 2, |s| coverage_serialize(s, &glyphs));
        s.set_u16(cov_pos + 2, delta as u16);
    } else {
        // SingleSubstFormat2_4::serialize
        let cov_pos = s.allocate(2); // coverage
        s.embed_u16(it.len() as u16);
        for p in it {
            s.embed_u16(p.1 as u16);
        }
        s.serialize_serialize(cov_pos, 2, |s| coverage_serialize(s, &glyphs));
    }
}

/// `SubstLookupSubTable::dispatch (hb_subset_context_t)` for the subtables other than the
/// extension.
pub(crate) fn subset(plan: &Plan<'_>, s: &mut Serializer, lookup_type: u32, sub: View<'_>) -> bool {
    match lookup_type {
        1 => single_subset(plan, s, sub),
        2 | 3 => set_subset(plan, s, lookup_type, sub),
        4 => ligature_subset(plan, s, sub),
        5 => context::context_subset(plan, s, Kind::Gsub, sub),
        6 => context::chain_context_subset(plan, s, Kind::Gsub, sub),
        8 => reverse_chain_subset(plan, s, sub),
        _ => true,
    }
}

fn map_gid(plan: &Plan<'_>, g: u32) -> u32 {
    plan.glyph_map.get(&g).copied().unwrap_or(INVALID)
}

/// `SingleSubstFormat1_3::subset` and `SingleSubstFormat2_4::subset`.
fn single_subset(plan: &Plan<'_>, s: &mut Serializer, sub: View<'_>) -> bool {
    let glyphset = &plan.glyphset_gsub;
    let it: Vec<(u32, u32)> = match sub.u16(0) {
        1 => {
            let d = sub.u16(4);
            let mask = 0xFFFFu32;
            let cov = Coverage(sub.off16(2));
            cov.intersect_set(glyphset)
                .into_iter()
                .map(|g| (g, g.wrapping_add(d) & mask))
                .filter(|p| glyphset.contains(&p.1))
                .map(|p| (map_gid(plan, p.0), map_gid(plan, p.1)))
                .collect()
        }
        2 => {
            let cov = Coverage(sub.off16(2));
            let n = sub.u16(4) as usize;
            cov.iter()
                .into_iter()
                .take(n)
                .enumerate()
                .map(|(i, g)| (g, sub.u16(6 + 2 * i)))
                .filter(|p| glyphset.contains(&p.0) && glyphset.contains(&p.1))
                .map(|p| (map_gid(plan, p.0), map_gid(plan, p.1)))
                .collect()
        }
        _ => return true,
    };
    let ret = !it.is_empty();
    single_subst_serialize(s, &it);
    ret
}

/// `MultipleSubstFormat1_2::subset` (with `Sequence::subset`) and
/// `AlternateSubstFormat1_2::subset` (with `AlternateSet::subset`).
fn set_subset(plan: &Plan<'_>, s: &mut Serializer, lookup_type: u32, sub: View<'_>) -> bool {
    if sub.u16(0) != 1 {
        return true;
    }
    let out = s.allocate(6);
    s.set_u16(out, 1);
    let glyphset = &plan.glyphset_gsub;
    let cov = Coverage(sub.off16(2));
    let n = sub.u16(4) as usize;
    let mut new_coverage: Vec<u32> = Vec::new();
    for (i, g) in cov.iter().into_iter().take(n).enumerate() {
        if !glyphset.contains(&g) {
            continue;
        }
        let snap = s.snapshot();
        let o = s.array_append(out + 4, 2);
        let ret = if sub.is_null16(6 + 2 * i) {
            s.zero_field(o, 2);
            false
        } else {
            let set = sub.off16(6 + 2 * i);
            s.serialize_subset(o, 2, true, |s| {
                if lookup_type == 2 {
                    sequence_subset(plan, s, set)
                } else {
                    alternate_set_subset(plan, s, set)
                }
            })
        };
        if ret {
            new_coverage.push(map_gid(plan, g));
        } else {
            s.array_pop(out + 4);
            s.revert(snap);
        }
    }
    s.serialize_serialize(out + 2, 2, |s| coverage_serialize(s, &new_coverage));
    !new_coverage.is_empty()
}

/// `Sequence::subset` (Sequence.hh#L136-L153).
fn sequence_subset(plan: &Plan<'_>, s: &mut Serializer, seq: View<'_>) -> bool {
    let n = seq.u16(0) as usize;
    if !(0..n).all(|i| plan.glyphset_gsub.contains(&seq.u16(2 + 2 * i))) {
        return false;
    }
    s.embed_u16(n as u16);
    for i in 0..n {
        s.embed_u16(map_gid(plan, seq.u16(2 + 2 * i)) as u16);
    }
    true
}

/// `AlternateSet::subset` (AlternateSet.hh#L112-L128).
fn alternate_set_subset(plan: &Plan<'_>, s: &mut Serializer, set: View<'_>) -> bool {
    let items: Vec<u32> = (0..set.u16(0) as usize)
        .map(|i| set.u16(2 + 2 * i))
        .filter(|g| plan.glyphset_gsub.contains(g))
        .map(|g| map_gid(plan, g))
        .collect();
    s.embed_u16(items.len() as u16);
    for g in &items {
        s.embed_u16(*g as u16);
    }
    !items.is_empty()
}

/// `LigatureSubstFormat1_2::subset` (LigatureSubstFormat1.hh#L129-L172).
fn ligature_subset(plan: &Plan<'_>, s: &mut Serializer, sub: View<'_>) -> bool {
    if sub.u16(0) != 1 {
        return true;
    }
    let glyphset = &plan.glyphset_gsub;
    let out = s.allocate(6);
    s.set_u16(out, 1);

    let cov = Coverage(sub.off16(2));
    let n = sub.u16(4) as usize;
    // Due to a bug in some older versions of windows 7 the Coverage table must be packed after the
    // LigatureSet and Ligature tables, so serialize Coverage first which places it last in the
    // packed order.
    let mut new_coverage: BTreeSet<u32> = BTreeSet::new();
    for (i, g) in cov.iter().into_iter().take(n).enumerate() {
        if !glyphset.contains(&g) {
            continue;
        }
        let ls = sub.off16(6 + 2 * i);
        // LigatureSet::intersects_lig_glyph
        let any = (0..ls.u16(0) as usize).any(|k| {
            let lig = ls.off16(2 + 2 * k);
            glyphset.contains(&lig.u16(0)) && ligature_intersects(lig, glyphset)
        });
        if any {
            new_coverage.insert(g);
        }
    }
    let mapped: Vec<u32> = new_coverage.iter().map(|&g| map_gid(plan, g)).collect();
    s.push();
    if !coverage_serialize(s, &mapped) {
        s.pop_discard();
        return false;
    }
    let coverage_idx = s.pop_pack(true);
    s.add_link(out + 2, 2, coverage_idx, Whence::Head, 0);

    for (i, g) in cov.iter().into_iter().take(n).enumerate() {
        if !new_coverage.contains(&g) {
            continue;
        }
        let snap = s.snapshot();
        let o = s.array_append(out + 4, 2);
        let ret = if sub.is_null16(6 + 2 * i) {
            s.zero_field(o, 2);
            false
        } else {
            let ls = sub.off16(6 + 2 * i);
            s.serialize_subset(o, 2, true, |s| ligature_set_subset(plan, s, ls))
        };
        if !ret {
            s.array_pop(out + 4);
            s.revert(snap);
        }
    }
    !new_coverage.is_empty()
}

/// `LigatureSet::subset` (LigatureSet.hh#L174-L192).
fn ligature_set_subset(plan: &Plan<'_>, s: &mut Serializer, ls: View<'_>) -> bool {
    let out = s.allocate(2);
    for k in 0..ls.u16(0) as usize {
        let snap = s.snapshot();
        let o = s.array_append(out, 2);
        let ret = if ls.is_null16(2 + 2 * k) {
            s.zero_field(o, 2);
            false
        } else {
            let lig = ls.off16(2 + 2 * k);
            s.serialize_subset(o, 2, true, |s| ligature_subset_one(plan, s, lig))
        };
        if !ret {
            s.array_pop(out);
            s.revert(snap);
        }
    }
    View::new(s.bytes()).u16(0) != 0
}

/// `Ligature::subset` (Ligature.hh#L165-L186).
fn ligature_subset_one(plan: &Plan<'_>, s: &mut Serializer, lig: View<'_>) -> bool {
    let glyphset = &plan.glyphset_gsub;
    if !ligature_intersects(lig, glyphset) || !glyphset.contains(&lig.u16(0)) {
        return false;
    }
    let out = s.allocate(4);
    s.set_u16(out, map_gid(plan, lig.u16(0)) as u16);
    let n = lig.u16(2).saturating_sub(1) as usize;
    s.set_u16(out + 2, (n + 1) as u16);
    for i in 0..n {
        s.embed_u16(map_gid(plan, lig.u16(4 + 2 * i)) as u16);
    }
    true
}

/// `ReverseChainSingleSubstFormat1::subset` (ReverseChainSingleSubstFormat1.hh#L194-L240).
fn reverse_chain_subset(plan: &Plan<'_>, s: &mut Serializer, sub: View<'_>) -> bool {
    if sub.u16(0) != 1 {
        return true;
    }
    let glyphset = &plan.glyphset_gsub;
    let cov = Coverage(sub.off16(2));
    let bt_len = sub.u16(4) as usize;
    let la_pos = 6 + 2 * bt_len;
    let la_len = sub.u16(la_pos) as usize;
    let sub_pos = la_pos + 2 + 2 * la_len;
    let n = sub.u16(sub_pos) as usize;
    let it: Vec<(u32, u32)> = cov
        .iter()
        .into_iter()
        .take(n)
        .enumerate()
        .map(|(i, g)| (g, sub.u16(sub_pos + 2 + 2 * i)))
        .filter(|p| glyphset.contains(&p.0) && glyphset.contains(&p.1))
        .map(|p| (map_gid(plan, p.0), map_gid(plan, p.1)))
        .collect();
    if it.is_empty() {
        return false;
    }
    // `serialize`
    s.embed_u16(1); // format
    let cov_pos = s.embed_u16(sub.u16(2) as u16); // coverage (replaced below)
    let coverage_offsets = |s: &mut Serializer, count: usize, first: usize| -> bool {
        let len_pos = s.allocate(2);
        for i in 0..count {
            let len = View::new(s.bytes()).u16(len_pos);
            s.set_u16(len_pos, (len + 1) as u16);
            let o = s.allocate(2);
            if sub.is_null16(first + 2 * i) {
                s.zero_field(o, 2);
                return false;
            }
            let c = Coverage(sub.off16(first + 2 * i));
            if !s.serialize_subset(o, 2, true, |s| {
                coverage_subset(s, c, plan.source.num_glyphs(), &plan.glyph_map_gsub)
            }) {
                return false;
            }
        }
        true
    };
    if !coverage_offsets(s, bt_len, 6) {
        return false;
    }
    if !coverage_offsets(s, la_len, la_pos + 2) {
        return false;
    }
    s.embed_u16(it.len() as u16);
    for p in &it {
        s.embed_u16(p.1 as u16);
    }
    let glyphs: Vec<u32> = it.iter().map(|p| p.0).collect();
    s.serialize_serialize(cov_pos, 2, |s| coverage_serialize(s, &glyphs))
}

#[allow(dead_code)]
fn _unused(set: &BTreeSet<u32>) -> Option<u32> {
    set_next(set, INVALID)
}
