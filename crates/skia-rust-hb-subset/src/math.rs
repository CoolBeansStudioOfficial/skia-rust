// Copyright © 2016  Igalia S.L.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-ot-math-table.hh (harfbuzz 9cb1fee5)

//! The `MATH` table: the glyph closure and the subsetter.

use std::collections::BTreeSet;

use crate::Res;
use crate::ot::{
    Coverage, INVALID, View, coverage_serialize, offset_subset, serialize_copy_device,
};
use crate::plan::Plan;
use crate::serialize::{ERROR_INT_OVERFLOW, Serializer};

/// `MATH::closure_glyphs` (hb-ot-math-table.hh#L1066-L1075) through `MathVariants::closure_glyphs`.
pub(crate) fn closure_glyphs(math: View<'_>, glyph_set: &mut BTreeSet<u32>) {
    if math.d.is_empty() || math.u32(0) == 0 || math.is_null16(8) {
        return;
    }
    let variants = math.off16(8);
    let vert_count = variants.u16(6) as usize;
    let horiz_count = variants.u16(8) as usize;
    let mut variant_glyphs: BTreeSet<u32> = BTreeSet::new();
    let mut part = |cov_field: usize, first: usize, count: usize| {
        if variants.is_null16(cov_field) {
            return;
        }
        for (i, g) in Coverage(variants.off16(cov_field))
            .iter()
            .into_iter()
            .take(count)
            .enumerate()
        {
            if !glyph_set.contains(&g) {
                continue;
            }
            // `MathGlyphConstruction::closure_glyphs`
            let construction = variants.off16(10 + 2 * (first + i));
            let assembly = construction.off16(0);
            for k in 0..assembly.u16(4) as usize {
                variant_glyphs.insert(assembly.u16(6 + 10 * k));
            }
            for k in 0..construction.u16(2) as usize {
                variant_glyphs.insert(construction.u16(4 + 4 * k));
            }
        }
    };
    part(2, 0, vert_count);
    part(4, vert_count, horiz_count);
    glyph_set.extend(variant_glyphs);
}

fn map_gid(plan: &Plan<'_>, g: u32) -> u32 {
    plan.glyph_map.get(&g).copied().unwrap_or(INVALID)
}

/// `MathValueRecord::copy (c, base)`: the record at `off` of `v`, whose device offset is relative
/// to `v`.
fn value_record_copy(s: &mut Serializer, v: View<'_>, off: usize) {
    let bytes: Vec<u8> = (0..4)
        .map(|k| v.d.get(off + k).copied().unwrap_or(0))
        .collect();
    let pos = s.embed(&bytes);
    serialize_copy_device(s, pos + 2, v, off + 2);
}

/// `MATH::subset` (hb-ot-math-table.hh#L1077-L1090).
#[allow(clippy::unnecessary_wraps)] // the callback shape of `run_table`
pub(crate) fn subset(plan: &Plan<'_>, s: &mut Serializer, math: View<'_>) -> Res<bool> {
    let out = s.embed(&math.d[..math.d.len().min(10)]);
    // `mathConstants.serialize_copy (c, mathConstants, this, 0, Head)`
    s.zero_field(out + 4, 2);
    if !math.is_null16(4) {
        let constants = math.off16(4);
        s.push();
        constants_copy(s, constants);
        let idx = s.pop_pack(true);
        s.add_link(out + 4, 2, idx, crate::serialize::Whence::Head, 0);
    }
    offset_subset(s, out + 6, math, 6, |s, gi| glyph_info_subset(plan, s, gi));
    offset_subset(s, out + 8, math, 8, |s, mv| variants_subset(plan, s, mv));
    Ok(true)
}

/// `MathConstants::copy` (hb-ot-math-table.hh#L74-L98).
fn constants_copy(s: &mut Serializer, c: View<'_>) {
    let head: Vec<u8> = (0..8).map(|k| c.d.get(k).copied().unwrap_or(0)).collect();
    s.embed(&head);
    for i in 0..51 {
        value_record_copy(s, c, 8 + 4 * i);
    }
    s.embed_u16(c.u16(8 + 4 * 51) as u16);
}

/// `MathItalicsCorrectionInfo::subset` and `MathTopAccentAttachment::subset`.
fn italics_or_accent_subset(plan: &Plan<'_>, s: &mut Serializer, v: View<'_>) -> bool {
    let out = s.allocate(4);
    // The coverage is at 0 and the records follow the length at 2.
    let new_coverage = record_array_subset_at(plan, s, out, v, 4, value_record_copy);
    s.serialize_serialize(out, 2, |s| coverage_serialize(s, &new_coverage));
    true
}

fn record_array_subset_at(
    plan: &Plan<'_>,
    s: &mut Serializer,
    out: usize,
    v: View<'_>,
    record_size: usize,
    copy: fn(&mut Serializer, View<'_>, usize),
) -> Vec<u32> {
    let cov = Coverage(v.off16(0));
    let n = v.u16(2) as usize;
    let mut new_coverage = Vec::new();
    for (i, g) in cov.iter().into_iter().take(n).enumerate() {
        if !plan.glyphset_mathed.contains(&g) {
            continue;
        }
        copy(s, v, 4 + record_size * i);
        let len = crate::bytes::u16_at(s.bytes(), out + 2);
        s.set_u16(out + 2, len.wrapping_add(1));
        new_coverage.push(map_gid(plan, g));
    }
    new_coverage
}

/// `MathGlyphInfo::subset` (hb-ot-math-table.hh#L420-L445).
fn glyph_info_subset(plan: &Plan<'_>, s: &mut Serializer, gi: View<'_>) -> bool {
    let out = s.embed(&gi.d[..gi.d.len().min(8)]);
    offset_subset(s, out, gi, 0, |s, v| italics_or_accent_subset(plan, s, v));
    offset_subset(s, out + 2, gi, 2, |s, v| {
        italics_or_accent_subset(plan, s, v)
    });
    let extended: Vec<u32> = Coverage(gi.off16(4))
        .iter()
        .into_iter()
        .take(plan.source.num_glyphs() as usize)
        .filter(|g| plan.glyphset_mathed.contains(g))
        .map(|g| map_gid(plan, g))
        .collect();
    if extended.is_empty() {
        s.zero_field(out + 4, 2);
    } else {
        s.serialize_serialize(out + 4, 2, |s| coverage_serialize(s, &extended));
    }
    offset_subset(s, out + 6, gi, 6, |s, v| kern_info_subset(plan, s, v));
    true
}

/// `MathKernInfo::subset` (hb-ot-math-table.hh#L371-L395) with `MathKernInfoRecord::copy`.
fn kern_info_subset(plan: &Plan<'_>, s: &mut Serializer, v: View<'_>) -> bool {
    let out = s.allocate(4);
    let new_coverage = record_array_subset_at(plan, s, out, v, 8, kern_info_record_copy);
    s.serialize_serialize(out, 2, |s| coverage_serialize(s, &new_coverage));
    true
}

fn kern_info_record_copy(s: &mut Serializer, v: View<'_>, off: usize) {
    let bytes: Vec<u8> = (0..8)
        .map(|k| v.d.get(off + k).copied().unwrap_or(0))
        .collect();
    let pos = s.embed(&bytes);
    for k in 0..4 {
        // `mathKern[i].serialize_copy (c, mathKern[i], base, 0, Head)`: `MathKern::copy`
        s.zero_field(pos + 2 * k, 2);
        if v.is_null16(off + 2 * k) {
            continue;
        }
        let kern = v.off16(off + 2 * k);
        s.push();
        s.embed_u16(kern.u16(0) as u16);
        for i in 0..=(2 * kern.u16(0) as usize) {
            value_record_copy(s, kern, 2 + 4 * i);
        }
        let idx = s.pop_pack(true);
        s.add_link(pos + 2 * k, 2, idx, crate::serialize::Whence::Head, 0);
    }
}

/// `MathVariants::subset` (hb-ot-math-table.hh#L986-L1020).
fn variants_subset(plan: &Plan<'_>, s: &mut Serializer, mv: View<'_>) -> bool {
    let glyphset = &plan.glyphset_mathed;
    let out = s.allocate(10);
    s.set_u16(out, mv.u16(0) as u16);
    let vert_count = mv.u16(6);
    let horiz_count = mv.u16(8);
    let mut indices: BTreeSet<u32> = BTreeSet::new();
    let mut collect = |field: usize, start: u32, end: u32| -> Vec<u32> {
        let mut new_coverage = Vec::new();
        if mv.is_null16(field) {
            return new_coverage;
        }
        for (i, g) in (start..end).zip(Coverage(mv.off16(field)).iter()) {
            if glyphset.contains(&g) {
                new_coverage.push(map_gid(plan, g));
                indices.insert(i);
            }
        }
        new_coverage
    };
    let new_vert = collect(2, 0, vert_count);
    let new_hori = collect(4, vert_count, vert_count + horiz_count);
    if !s.check_fits(new_vert.len() as u64, 16, ERROR_INT_OVERFLOW) {
        return false;
    }
    s.set_u16(out + 6, new_vert.len() as u16);
    if !s.check_fits(new_hori.len() as u64, 16, ERROR_INT_OVERFLOW) {
        return false;
    }
    s.set_u16(out + 8, new_hori.len() as u16);
    for &i in &indices {
        let field = 10 + 2 * i as usize;
        let pos = s.embed_u16(mv.u16(field) as u16);
        offset_subset(s, pos, mv, field, |s, c| construction_subset(plan, s, c));
    }
    if !new_vert.is_empty() {
        s.serialize_serialize(out + 2, 2, |s| coverage_serialize(s, &new_vert));
    }
    if !new_hori.is_empty() {
        s.serialize_serialize(out + 4, 2, |s| coverage_serialize(s, &new_hori));
    }
    true
}

/// `MathGlyphConstruction::subset` (hb-ot-math-table.hh#L880-L898).
fn construction_subset(plan: &Plan<'_>, s: &mut Serializer, c: View<'_>) -> bool {
    let out = s.allocate(4);
    offset_subset(s, out, c, 0, |s, a| assembly_subset(plan, s, a));
    let len = c.u16(2);
    s.set_u16(out + 2, len as u16);
    for i in 0..len as usize {
        // `MathGlyphVariantRecord::subset`
        let pos = s.embed(&[
            c.u8(4 + 4 * i) as u8,
            c.u8(5 + 4 * i) as u8,
            c.u8(6 + 4 * i) as u8,
            c.u8(7 + 4 * i) as u8,
        ]);
        let v = map_gid(plan, c.u16(4 + 4 * i));
        if !s.check_fits(u64::from(v), 16, ERROR_INT_OVERFLOW) {
            return false;
        }
        s.set_u16(pos, v as u16);
    }
    true
}

/// `MathGlyphAssembly::subset` (hb-ot-math-table.hh#L822-L832).
fn assembly_subset(plan: &Plan<'_>, s: &mut Serializer, a: View<'_>) -> bool {
    value_record_copy(s, a, 0);
    let n = a.u16(4) as usize;
    s.embed_u16(n as u16);
    for i in 0..n {
        let off = 6 + 10 * i;
        let bytes: Vec<u8> = (0..10)
            .map(|k| a.d.get(off + k).copied().unwrap_or(0))
            .collect();
        let pos = s.embed(&bytes);
        let v = map_gid(plan, a.u16(off));
        if !s.check_fits(u64::from(v), 16, ERROR_INT_OVERFLOW) {
            return false;
        }
        s.set_u16(pos, v as u16);
    }
    true
}
