// Copyright © 2007,2008,2009,2010  Red Hat, Inc.
// Copyright © 2010,2012  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-ot-layout-gsubgpos.hh (Rule, RuleSet, ContextFormat1_4, ContextFormat2_5,
// ContextFormat3, ChainRule, ChainRuleSet, ChainContextFormat1_4, ChainContextFormat2_5,
// ChainContextFormat3) (harfbuzz 9cb1fee5)

//! The `Context` and `ChainContext` subtables of `GSUB` and `GPOS` (lookup types 5 and 6, and 7
//! and 8): intersection, glyph closure, lookup closure and subsetting. Only formats 1 to 3
//! (16 bit offsets) are ported.

use std::collections::{BTreeSet, HashMap};

use crate::gsubgpos::{ClosureCtx, ClosureLookupsCtx, Kind};
use crate::ot::{
    ClassDef, ClassDefPlan, ClassDefSubsetArgs, Coverage, INVALID, View, classdef_subset, coverage_serialize,
    coverage_subset,
};
use crate::plan::Plan;
use crate::serialize::{ERROR_INT_OVERFLOW, Serializer, Whence};

/// `ContextFormat` (hb-ot-layout-gsubgpos.hh#L1710).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ContextFormat {
    Simple,
    ClassBased,
    CoverageBased,
}

/// The `data` pointer of the closure lookup context functions.
#[derive(Clone, Copy)]
enum IData<'a> {
    Glyph,
    Class(ClassDef<'a>),
    Coverage(View<'a>),
}

/// `this + offset` for a `Coverage` offset stored as an array value.
fn coverage_at(base: View<'_>, value: u32) -> Coverage<'_> {
    if value == 0 { Coverage(View::new(&[])) } else { Coverage(base.sub(value as usize)) }
}

/// `intersects_glyph`, `intersects_class` and `intersects_coverage`
/// (hb-ot-layout-gsubgpos.hh#L1167-L1192).
fn intersects_value(glyphs: &BTreeSet<u32>, value: u32, d: IData<'_>) -> bool {
    match d {
        IData::Glyph => glyphs.contains(&value),
        IData::Class(cd) => cd.intersects_class(glyphs, value),
        IData::Coverage(base) => coverage_at(base, value).intersects(glyphs),
    }
}

/// `array_is_subset_of` (hb-ot-layout-gsubgpos.hh#L1230-L1242).
fn array_is_subset_of(glyphs: &BTreeSet<u32>, values: &[u32], d: IData<'_>) -> bool {
    values.iter().all(|&v| intersects_value(glyphs, v, d))
}

/// `intersected_glyph`, `intersected_class_glyphs` and `intersected_coverage_glyphs`
/// (hb-ot-layout-gsubgpos.hh#L1193-L1228).
fn intersected_glyphs(
    glyphs: &BTreeSet<u32>,
    d: IData<'_>,
    value: u32,
    out: &mut BTreeSet<u32>,
    cache: &mut HashMap<u32, BTreeSet<u32>>,
) {
    match d {
        // The simple context passes the input array as `data`; handled by the callers.
        IData::Glyph => {}
        IData::Class(cd) => {
            if let Some(cached) = cache.get(&value) {
                out.extend(cached.iter().copied());
                return;
            }
            let mut v = BTreeSet::new();
            cd.intersected_class_glyphs(glyphs, value, &mut v);
            out.extend(v.iter().copied());
            cache.insert(value, v);
        }
        IData::Coverage(base) => {
            out.extend(coverage_at(base, value).intersect_set(glyphs));
        }
    }
}

/// `ContextClosureLookupContext` / `ChainContextClosureLookupContext`
/// (hb-ot-layout-gsubgpos.hh#L1970-L1976, L3076-L3083).
struct ClosureLookupContext<'a> {
    format: ContextFormat,
    /// `intersects_data[0..3]`: backtrack, input and lookahead (the context uses `[1]`).
    data: [IData<'a>; 3],
    /// `intersected_glyphs_cache`
    intersected_cache: HashMap<u32, BTreeSet<u32>>,
}

/// `context_closure_recurse_lookups` (hb-ot-layout-gsubgpos.hh#L1743-L1813).
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn context_closure_recurse_lookups(
    c: &mut ClosureCtx<'_>,
    input_count: u32,
    input: &[u32],
    lookup_records: &[(u32, u32)],
    value: u32,
    lc: &mut ClosureLookupContext<'_>,
) {
    let mut covered_seq_indices: BTreeSet<u32> = BTreeSet::new();
    let mut pos_glyphs: BTreeSet<u32> = BTreeSet::new();
    for &(seq_index, lookup_list_index) in lookup_records {
        if seq_index >= input_count {
            continue;
        }
        let mut has_pos_glyphs = false;
        if !covered_seq_indices.contains(&seq_index) {
            has_pos_glyphs = true;
            pos_glyphs.clear();
            if seq_index == 0 {
                match lc.format {
                    ContextFormat::Simple => {
                        pos_glyphs.insert(value);
                    }
                    ContextFormat::ClassBased => {
                        let parent = c.parent_active_glyphs().clone();
                        intersected_glyphs(&parent, lc.data[1], value, &mut pos_glyphs, &mut lc.intersected_cache);
                    }
                    ContextFormat::CoverageBased => {
                        pos_glyphs = c.parent_active_glyphs().clone();
                    }
                }
            } else if lc.format == ContextFormat::Simple {
                // `intersected_glyph`: `data` is the input array and the value an index into it.
                pos_glyphs.insert(input.get(seq_index as usize - 1).copied().unwrap_or(0));
            } else {
                let input_value = input.get(seq_index as usize - 1).copied().unwrap_or(0);
                let glyphs = c.glyphs.clone();
                intersected_glyphs(&glyphs, lc.data[1], input_value, &mut pos_glyphs, &mut lc.intersected_cache);
            }
        }
        covered_seq_indices.insert(seq_index);
        let cur = if has_pos_glyphs { std::mem::take(&mut pos_glyphs) } else { c.glyphs.clone() };
        c.active_glyphs_stack.push(cur);

        let mut end_index = input_count;
        if lc.format == ContextFormat::CoverageBased {
            end_index += 1;
        }
        c.recurse(lookup_list_index, &mut covered_seq_indices, seq_index, end_index);
        c.pop_cur_done_glyphs();
    }
}

/// `chain_context_closure_lookup` / `context_closure_lookup`
/// (hb-ot-layout-gsubgpos.hh#L2005-L2024, L3125-L3150): the input array excludes the first glyph.
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn closure_lookup(
    c: &mut ClosureCtx<'_>,
    backtrack: &[u32],
    input_count: u32,
    input: &[u32],
    lookahead: &[u32],
    lookup_records: &[(u32, u32)],
    value: u32,
    lc: &mut ClosureLookupContext<'_>,
    chain: bool,
) {
    let glyphs = c.glyphs.clone();
    let intersects = if chain {
        array_is_subset_of(&glyphs, backtrack, lc.data[0])
            && array_is_subset_of(&glyphs, input, lc.data[1])
            && array_is_subset_of(&glyphs, lookahead, lc.data[2])
    } else {
        array_is_subset_of(&glyphs, input, lc.data[1])
    };
    if intersects {
        context_closure_recurse_lookups(c, input_count, input, lookup_records, value, lc);
    }
}

// -------------------------------------------------------------------------------------------
// Rule / RuleSet (Context)
// -------------------------------------------------------------------------------------------

struct Rule {
    input_count: u32,
    lookup_count: u32,
    /// `inputZ.as_array (inputCount ? inputCount - 1 : 0)`
    input: Vec<u32>,
    lookups: Vec<(u32, u32)>,
}

fn parse_rule(r: View<'_>) -> Rule {
    let input_count = r.u16(0);
    let lookup_count = r.u16(2);
    let n = input_count.saturating_sub(1) as usize;
    let input: Vec<u32> = (0..n).map(|i| r.u16(4 + 2 * i)).collect();
    let base = 4 + 2 * n;
    let lookups = (0..lookup_count as usize).map(|i| (r.u16(base + 4 * i), r.u16(base + 4 * i + 2))).collect();
    Rule { input_count, lookup_count, input, lookups }
}

fn rule_set_rules<'a>(rs: View<'a>) -> Vec<View<'a>> {
    (0..rs.u16(0) as usize).map(|i| rs.off16(2 + 2 * i)).collect()
}

// -------------------------------------------------------------------------------------------
// Context
// -------------------------------------------------------------------------------------------

/// `Context::dispatch (hb_intersects_context_t)`.
pub(crate) fn context_intersects(sub: View<'_>, glyphs: &BTreeSet<u32>) -> bool {
    match sub.u16(0) {
        1 => {
            // ContextFormat1_4::intersects
            let cov = Coverage(sub.off16(2));
            let len = sub.u16(4) as usize;
            cov.iter().into_iter().take(len).enumerate().any(|(i, g)| {
                glyphs.contains(&g) && {
                    let rs = sub.off16(6 + 2 * i);
                    rule_set_rules(rs).into_iter().any(|r| {
                        let rule = parse_rule(r);
                        array_is_subset_of(glyphs, &rule.input, IData::Glyph)
                    })
                }
            })
        }
        2 => {
            // ContextFormat2_5::intersects
            let cov = Coverage(sub.off16(2));
            if !cov.intersects(glyphs) {
                return false;
            }
            let class_def = ClassDef(sub.off16(4));
            let retained_coverage_glyphs = cov.intersect_set(glyphs);
            let mut coverage_glyph_classes = BTreeSet::new();
            class_def.intersected_classes(&retained_coverage_glyphs, &mut coverage_glyph_classes);
            let n = sub.u16(6) as usize;
            (0..n).any(|i| {
                class_def.intersects_class(glyphs, i as u32)
                    && coverage_glyph_classes.contains(&(i as u32))
                    && rule_set_rules(sub.off16(8 + 2 * i)).into_iter().any(|r| {
                        let rule = parse_rule(r);
                        array_is_subset_of(glyphs, &rule.input, IData::Class(class_def))
                    })
            })
        }
        3 => {
            // ContextFormat3::intersects
            let glyph_count = sub.u16(2) as usize;
            if !Coverage(sub.off16(6)).intersects(glyphs) {
                return false;
            }
            let input: Vec<u32> = (1..glyph_count).map(|i| sub.u16(6 + 2 * i)).collect();
            array_is_subset_of(glyphs, &input, IData::Coverage(sub))
        }
        _ => false,
    }
}

/// `Context::dispatch (hb_closure_context_t)`.
pub(crate) fn context_closure(c: &mut ClosureCtx<'_>, sub: View<'_>) {
    match sub.u16(0) {
        1 => {
            // ContextFormat1_4::closure
            let cov = Coverage(sub.off16(2));
            c.active_glyphs_stack.push(BTreeSet::new());
            let prev = c.previous_parent_active_glyphs().clone();
            let cur = cov.intersect_set(&prev);
            *c.active_glyphs_stack.last_mut().expect("pushed") = cur;
            let mut lc = ClosureLookupContext {
                format: ContextFormat::Simple,
                data: [IData::Glyph; 3],
                intersected_cache: HashMap::new(),
            };
            let len = sub.u16(4) as usize;
            for (i, g) in cov.iter().into_iter().take(len).enumerate() {
                if !c.previous_parent_active_glyphs().contains(&g) {
                    continue;
                }
                let rs = sub.off16(6 + 2 * i);
                if c.lookup_limit_exceeded() {
                    continue;
                }
                for r in rule_set_rules(rs) {
                    if c.lookup_limit_exceeded() {
                        continue;
                    }
                    let rule = parse_rule(r);
                    closure_lookup(c, &[], rule.input_count, &rule.input, &[], &rule.lookups, g, &mut lc, false);
                }
            }
            c.pop_cur_done_glyphs();
        }
        2 => {
            // ContextFormat2_5::closure
            let cov = Coverage(sub.off16(2));
            if !cov.intersects(c.glyphs) {
                return;
            }
            c.active_glyphs_stack.push(BTreeSet::new());
            let prev = c.previous_parent_active_glyphs().clone();
            let cur = cov.intersect_set(&prev);
            *c.active_glyphs_stack.last_mut().expect("pushed") = cur;
            let class_def = ClassDef(sub.off16(4));
            let mut lc = ClosureLookupContext {
                format: ContextFormat::ClassBased,
                data: [IData::Class(class_def); 3],
                intersected_cache: HashMap::new(),
            };
            let n = sub.u16(6) as usize;
            for i in 0..n {
                let parent = c.parent_active_glyphs().clone();
                if !class_def.intersects_class(&parent, i as u32) {
                    continue;
                }
                let rs = sub.off16(8 + 2 * i);
                if c.lookup_limit_exceeded() {
                    continue;
                }
                for r in rule_set_rules(rs) {
                    if c.lookup_limit_exceeded() {
                        continue;
                    }
                    let rule = parse_rule(r);
                    closure_lookup(c, &[], rule.input_count, &rule.input, &[], &rule.lookups, i as u32, &mut lc, false);
                }
            }
            c.pop_cur_done_glyphs();
        }
        3 => {
            // ContextFormat3::closure
            let glyph_count = sub.u16(2);
            let lookup_count = sub.u16(4) as usize;
            let cov0 = Coverage(sub.off16(6));
            if !cov0.intersects(c.glyphs) {
                return;
            }
            c.active_glyphs_stack.push(BTreeSet::new());
            let prev = c.previous_parent_active_glyphs().clone();
            let cur = cov0.intersect_set(&prev);
            *c.active_glyphs_stack.last_mut().expect("pushed") = cur;
            let base = 6 + 2 * glyph_count as usize;
            let lookups: Vec<(u32, u32)> =
                (0..lookup_count).map(|i| (sub.u16(base + 4 * i), sub.u16(base + 4 * i + 2))).collect();
            let input: Vec<u32> = (1..glyph_count as usize).map(|i| sub.u16(6 + 2 * i)).collect();
            let mut lc = ClosureLookupContext {
                format: ContextFormat::CoverageBased,
                data: [IData::Coverage(sub); 3],
                intersected_cache: HashMap::new(),
            };
            closure_lookup(c, &[], glyph_count, &input, &[], &lookups, 0, &mut lc, false);
            c.pop_cur_done_glyphs();
        }
        _ => {}
    }
}

/// `Context::dispatch (hb_closure_lookups_context_t)`.
pub(crate) fn context_closure_lookups(c: &mut ClosureLookupsCtx<'_>, sub: View<'_>) {
    match sub.u16(0) {
        1 => {
            // ContextFormat1_4::closure_lookups
            let cov = Coverage(sub.off16(2));
            let len = sub.u16(4) as usize;
            for (i, g) in cov.iter().into_iter().take(len).enumerate() {
                if !c.glyphs.contains(&g) {
                    continue;
                }
                let rs = sub.off16(6 + 2 * i);
                if c.lookup_limit_exceeded_pub() {
                    continue;
                }
                for r in rule_set_rules(rs) {
                    if c.lookup_limit_exceeded_pub() {
                        continue;
                    }
                    let rule = parse_rule(r);
                    if !array_is_subset_of(c.glyphs, &rule.input, IData::Glyph) {
                        continue;
                    }
                    for &(_, l) in &rule.lookups {
                        c.recurse(l);
                    }
                }
            }
        }
        2 => {
            // ContextFormat2_5::closure_lookups
            let cov = Coverage(sub.off16(2));
            if !cov.intersects(c.glyphs) {
                return;
            }
            let class_def = ClassDef(sub.off16(4));
            let n = sub.u16(6) as usize;
            for i in 0..n {
                if !class_def.intersects_class(c.glyphs, i as u32) {
                    continue;
                }
                if c.lookup_limit_exceeded_pub() {
                    continue;
                }
                for r in rule_set_rules(sub.off16(8 + 2 * i)) {
                    if c.lookup_limit_exceeded_pub() {
                        continue;
                    }
                    let rule = parse_rule(r);
                    if !array_is_subset_of(c.glyphs, &rule.input, IData::Class(class_def)) {
                        continue;
                    }
                    for &(_, l) in &rule.lookups {
                        c.recurse(l);
                    }
                }
            }
        }
        3 => {
            // ContextFormat3::closure_lookups
            if !context_intersects(sub, c.glyphs) {
                return;
            }
            let glyph_count = sub.u16(2) as usize;
            let lookup_count = sub.u16(4) as usize;
            let base = 6 + 2 * glyph_count;
            for i in 0..lookup_count {
                c.recurse(sub.u16(base + 4 * i + 2));
            }
        }
        _ => {}
    }
}

/// `serialize_lookuprecord_array` (hb-ot-layout-gsubgpos.hh#L1723-L1741): the retained records
/// with the lookup index remapped. Returns the count.
fn serialize_lookuprecord_array(s: &mut Serializer, records: &[(u32, u32)], lookup_map: &HashMap<u32, u32>) -> u32 {
    let mut count = 0;
    for &(seq, lookup) in records {
        let Some(&new) = lookup_map.get(&lookup) else { continue };
        s.embed_u16(seq as u16);
        s.embed_u16(new as u16);
        count += 1;
    }
    count
}

fn lookup_map_of<'p>(plan: &'p Plan<'_>, kind: Kind) -> &'p HashMap<u32, u32> {
    match kind {
        Kind::Gsub => &plan.layout.gsub.lookups,
        Kind::Gpos => &plan.layout.gpos.lookups,
    }
}

/// `Rule::subset` / `Rule::serialize` (hb-ot-layout-gsubgpos.hh#L2188-L2240).
fn rule_subset(
    s: &mut Serializer,
    r: View<'_>,
    lookup_map: &HashMap<u32, u32>,
    mapping: &HashMap<u32, u32>,
) -> bool {
    let rule = parse_rule(r);
    if rule.input_count == 0 {
        return false;
    }
    if !rule.input.iter().all(|v| mapping.contains_key(v)) {
        return false;
    }
    let out = s.allocate(4);
    s.set_u16(out, rule.input_count as u16);
    for org in &rule.input {
        s.embed_u16(mapping.get(org).copied().unwrap_or(INVALID) as u16);
    }
    let count = serialize_lookuprecord_array(s, &rule.lookups, lookup_map);
    if count > 0xFFFF {
        s.err(ERROR_INT_OVERFLOW);
    }
    s.set_u16(out + 2, count as u16);
    true
}

/// `RuleSet::subset` (hb-ot-layout-gsubgpos.hh#L2392-L2430).
fn rule_set_subset(
    s: &mut Serializer,
    rs: View<'_>,
    lookup_map: &HashMap<u32, u32>,
    mapping: &HashMap<u32, u32>,
) -> bool {
    let snap = s.snapshot();
    let out = s.allocate(2);
    for i in 0..rs.u16(0) as usize {
        if rs.is_null16(2 + 2 * i) {
            continue;
        }
        let o_snap = s.snapshot();
        let o = s.array_append(out, 2);
        let r = rs.off16(2 + 2 * i);
        if !s.serialize_subset(o, 2, true, |s| rule_subset(s, r, lookup_map, mapping)) {
            s.array_pop(out);
            s.revert(o_snap);
        }
    }
    let ret = View::new(s.bytes()).u16(0) != 0;
    if !ret {
        s.revert(snap);
    }
    ret
}

/// `Context::dispatch (hb_subset_context_t)`.
pub(crate) fn context_subset(plan: &Plan<'_>, s: &mut Serializer, kind: Kind, sub: View<'_>) -> bool {
    let lookup_map = lookup_map_of(plan, kind);
    let cdp = ClassDefPlan {
        glyph_map_gsub: &plan.glyph_map_gsub,
        glyphset_gsub: &plan.glyphset_gsub,
        num_source_glyphs: plan.source.num_glyphs(),
    };
    match sub.u16(0) {
        1 => {
            // ContextFormat1_4::subset
            let out = s.allocate(6);
            s.set_u16(out, 1);
            let cov = Coverage(sub.off16(2));
            let len = sub.u16(4) as usize;
            let mut new_coverage: Vec<u32> = Vec::new();
            for (i, g) in cov.iter().into_iter().take(len).enumerate() {
                if !plan.glyphset_gsub.contains(&g) {
                    continue;
                }
                // `subset_offset_array (c, out->ruleSet, this, lookup_map)`
                let snap = s.snapshot();
                let o = s.array_append(out + 4, 2);
                let ret = if sub.is_null16(6 + 2 * i) {
                    s.zero_field(o, 2);
                    false
                } else {
                    let rs = sub.off16(6 + 2 * i);
                    s.serialize_subset(o, 2, true, |s| rule_set_subset(s, rs, lookup_map, &plan.glyph_map))
                };
                if !ret {
                    s.array_pop(out + 4);
                    s.revert(snap);
                } else {
                    new_coverage.push(plan.glyph_map.get(&g).copied().unwrap_or(INVALID));
                }
            }
            s.serialize_serialize(out + 2, 2, |s| coverage_serialize(s, &new_coverage));
            !new_coverage.is_empty()
        }
        2 => {
            // ContextFormat2_5::subset
            let out = s.allocate(8);
            s.set_u16(out, 2);
            if sub.is_null16(2) {
                s.zero_field(out + 2, 2);
                return false;
            }
            let cov = Coverage(sub.off16(2));
            if !s.serialize_subset(out + 2, 2, true, |s| {
                coverage_subset(s, cov, plan.source.num_glyphs(), &plan.glyph_map_gsub)
            }) {
                return false;
            }
            let mut klass_map: HashMap<u32, u32> = HashMap::new();
            if sub.is_null16(4) {
                s.zero_field(out + 4, 2);
            } else {
                let cd = ClassDef(sub.off16(4));
                s.serialize_subset(out + 4, 2, true, |s| {
                    classdef_subset(
                        s,
                        cd,
                        &cdp,
                        ClassDefSubsetArgs { klass_map: Some(&mut klass_map), ..Default::default() },
                    )
                });
            }
            let retained_coverage_glyphs = cov.intersect_set(&plan.glyphset_gsub);
            let mut coverage_glyph_classes = BTreeSet::new();
            ClassDef(sub.off16(4)).intersected_classes(&retained_coverage_glyphs, &mut coverage_glyph_classes);

            let n = sub.u16(6) as usize;
            let mut non_zero_index: i64 = -1;
            let mut index: i64 = 0;
            let mut snapshot = s.snapshot();
            for i in 0..n {
                if !klass_map.contains_key(&(i as u32)) {
                    continue;
                }
                let o = s.array_append(out + 6, 2);
                let ok = coverage_glyph_classes.contains(&(i as u32)) && {
                    if sub.is_null16(8 + 2 * i) {
                        s.zero_field(o, 2);
                        false
                    } else {
                        let rs = sub.off16(8 + 2 * i);
                        s.serialize_subset(o, 2, true, |s| rule_set_subset(s, rs, lookup_map, &klass_map))
                    }
                };
                if ok {
                    non_zero_index = index;
                    snapshot = s.snapshot();
                }
                index += 1;
            }
            if non_zero_index == -1 {
                return false;
            }
            // prune empty trailing ruleSets
            index -= 1;
            while index > non_zero_index {
                s.array_pop(out + 6);
                index -= 1;
            }
            s.revert(snapshot);
            View::new(s.bytes()).u16(out + 6) != 0
        }
        3 => {
            // ContextFormat3::subset
            let glyph_count = sub.u16(2);
            let lookup_count = sub.u16(4) as usize;
            let out = s.allocate(6);
            s.set_u16(out, 3);
            s.set_u16(out + 2, glyph_count as u16);
            for i in 0..glyph_count as usize {
                let o = s.allocate(2);
                if sub.is_null16(6 + 2 * i) {
                    s.zero_field(o, 2);
                    return false;
                }
                let cov = Coverage(sub.off16(6 + 2 * i));
                if !s.serialize_subset(o, 2, true, |s| {
                    coverage_subset(s, cov, plan.source.num_glyphs(), &plan.glyph_map_gsub)
                }) {
                    return false;
                }
            }
            let base = 6 + 2 * glyph_count as usize;
            let records: Vec<(u32, u32)> =
                (0..lookup_count).map(|i| (sub.u16(base + 4 * i), sub.u16(base + 4 * i + 2))).collect();
            let count = serialize_lookuprecord_array(s, &records, lookup_map);
            if count > 0xFFFF {
                s.err(ERROR_INT_OVERFLOW);
            }
            s.set_u16(out + 4, count as u16);
            true
        }
        _ => true,
    }
}

// -------------------------------------------------------------------------------------------
// ChainRule / ChainRuleSet (ChainContext)
// -------------------------------------------------------------------------------------------

struct ChainRule {
    backtrack: Vec<u32>,
    input_len_p1: u32,
    input: Vec<u32>,
    lookahead: Vec<u32>,
    lookups: Vec<(u32, u32)>,
}

fn parse_chain_rule(r: View<'_>) -> ChainRule {
    let bt_len = r.u16(0) as usize;
    let backtrack: Vec<u32> = (0..bt_len).map(|i| r.u16(2 + 2 * i)).collect();
    let input_pos = 2 + 2 * bt_len;
    let input_len_p1 = r.u16(input_pos);
    let n_in = input_len_p1.saturating_sub(1) as usize;
    let input: Vec<u32> = (0..n_in).map(|i| r.u16(input_pos + 2 + 2 * i)).collect();
    let la_pos = input_pos + 2 + 2 * n_in;
    let la_len = r.u16(la_pos) as usize;
    let lookahead: Vec<u32> = (0..la_len).map(|i| r.u16(la_pos + 2 + 2 * i)).collect();
    let lk_pos = la_pos + 2 + 2 * la_len;
    let lk_len = r.u16(lk_pos) as usize;
    let lookups = (0..lk_len).map(|i| (r.u16(lk_pos + 2 + 4 * i), r.u16(lk_pos + 2 + 4 * i + 2))).collect();
    ChainRule { backtrack, input_len_p1, input, lookahead, lookups }
}

fn chain_intersects(glyphs: &BTreeSet<u32>, rule: &ChainRule, data: &[IData<'_>; 3]) -> bool {
    array_is_subset_of(glyphs, &rule.backtrack, data[0])
        && array_is_subset_of(glyphs, &rule.input, data[1])
        && array_is_subset_of(glyphs, &rule.lookahead, data[2])
}

/// `ChainContext::dispatch (hb_intersects_context_t)`.
pub(crate) fn chain_context_intersects(sub: View<'_>, glyphs: &BTreeSet<u32>) -> bool {
    match sub.u16(0) {
        1 => {
            let cov = Coverage(sub.off16(2));
            let len = sub.u16(4) as usize;
            let data = [IData::Glyph; 3];
            cov.iter().into_iter().take(len).enumerate().any(|(i, g)| {
                glyphs.contains(&g)
                    && rule_set_rules(sub.off16(6 + 2 * i))
                        .into_iter()
                        .any(|r| chain_intersects(glyphs, &parse_chain_rule(r), &data))
            })
        }
        2 => {
            let cov = Coverage(sub.off16(2));
            if !cov.intersects(glyphs) {
                return false;
            }
            let backtrack_cd = ClassDef(sub.off16(4));
            let input_cd = ClassDef(sub.off16(6));
            let lookahead_cd = ClassDef(sub.off16(8));
            let data = [IData::Class(backtrack_cd), IData::Class(input_cd), IData::Class(lookahead_cd)];
            let retained_coverage_glyphs = cov.intersect_set(glyphs);
            let mut coverage_glyph_classes = BTreeSet::new();
            input_cd.intersected_classes(&retained_coverage_glyphs, &mut coverage_glyph_classes);
            let n = sub.u16(10) as usize;
            (0..n).any(|i| {
                input_cd.intersects_class(glyphs, i as u32)
                    && coverage_glyph_classes.contains(&(i as u32))
                    && rule_set_rules(sub.off16(12 + 2 * i))
                        .into_iter()
                        .any(|r| chain_intersects(glyphs, &parse_chain_rule(r), &data))
            })
        }
        3 => {
            let (backtrack, input, lookahead, _) = parse_chain3(sub);
            let Some(&first) = input.first() else { return false };
            if !coverage_at(sub, first).intersects(glyphs) {
                return false;
            }
            let data = [IData::Coverage(sub); 3];
            array_is_subset_of(glyphs, &backtrack, data[0])
                && array_is_subset_of(glyphs, &input[1..], data[1])
                && array_is_subset_of(glyphs, &lookahead, data[2])
        }
        _ => false,
    }
}

/// The arrays of a `ChainContextFormat3`: backtrack, input (all glyphs), lookahead, lookup records.
type Chain3 = (Vec<u32>, Vec<u32>, Vec<u32>, Vec<(u32, u32)>);

fn parse_chain3(sub: View<'_>) -> Chain3 {
    let bt_len = sub.u16(2) as usize;
    let backtrack: Vec<u32> = (0..bt_len).map(|i| sub.u16(4 + 2 * i)).collect();
    let in_pos = 4 + 2 * bt_len;
    let in_len = sub.u16(in_pos) as usize;
    let input: Vec<u32> = (0..in_len).map(|i| sub.u16(in_pos + 2 + 2 * i)).collect();
    let la_pos = in_pos + 2 + 2 * in_len;
    let la_len = sub.u16(la_pos) as usize;
    let lookahead: Vec<u32> = (0..la_len).map(|i| sub.u16(la_pos + 2 + 2 * i)).collect();
    let lk_pos = la_pos + 2 + 2 * la_len;
    let lk_len = sub.u16(lk_pos) as usize;
    let lookups = (0..lk_len).map(|i| (sub.u16(lk_pos + 2 + 4 * i), sub.u16(lk_pos + 2 + 4 * i + 2))).collect();
    (backtrack, input, lookahead, lookups)
}

/// `ChainContext::dispatch (hb_closure_context_t)`.
pub(crate) fn chain_context_closure(c: &mut ClosureCtx<'_>, sub: View<'_>) {
    match sub.u16(0) {
        1 => {
            let cov = Coverage(sub.off16(2));
            c.active_glyphs_stack.push(BTreeSet::new());
            let prev = c.previous_parent_active_glyphs().clone();
            let cur = cov.intersect_set(&prev);
            *c.active_glyphs_stack.last_mut().expect("pushed") = cur;
            let mut lc = ClosureLookupContext {
                format: ContextFormat::Simple,
                data: [IData::Glyph; 3],
                intersected_cache: HashMap::new(),
            };
            let len = sub.u16(4) as usize;
            for (i, g) in cov.iter().into_iter().take(len).enumerate() {
                if !c.previous_parent_active_glyphs().contains(&g) {
                    continue;
                }
                if c.lookup_limit_exceeded() {
                    continue;
                }
                for r in rule_set_rules(sub.off16(6 + 2 * i)) {
                    if c.lookup_limit_exceeded() {
                        continue;
                    }
                    let rule = parse_chain_rule(r);
                    closure_lookup(
                        c,
                        &rule.backtrack,
                        rule.input_len_p1,
                        &rule.input,
                        &rule.lookahead,
                        &rule.lookups,
                        g,
                        &mut lc,
                        true,
                    );
                }
            }
            c.pop_cur_done_glyphs();
        }
        2 => {
            let cov = Coverage(sub.off16(2));
            if !cov.intersects(c.glyphs) {
                return;
            }
            c.active_glyphs_stack.push(BTreeSet::new());
            let prev = c.previous_parent_active_glyphs().clone();
            let cur = cov.intersect_set(&prev);
            *c.active_glyphs_stack.last_mut().expect("pushed") = cur;
            let backtrack_cd = ClassDef(sub.off16(4));
            let input_cd = ClassDef(sub.off16(6));
            let lookahead_cd = ClassDef(sub.off16(8));
            let mut lc = ClosureLookupContext {
                format: ContextFormat::ClassBased,
                data: [IData::Class(backtrack_cd), IData::Class(input_cd), IData::Class(lookahead_cd)],
                intersected_cache: HashMap::new(),
            };
            let n = sub.u16(10) as usize;
            for i in 0..n {
                let parent = c.parent_active_glyphs().clone();
                if !input_cd.intersects_class(&parent, i as u32) {
                    continue;
                }
                if c.lookup_limit_exceeded() {
                    continue;
                }
                for r in rule_set_rules(sub.off16(12 + 2 * i)) {
                    if c.lookup_limit_exceeded() {
                        continue;
                    }
                    let rule = parse_chain_rule(r);
                    closure_lookup(
                        c,
                        &rule.backtrack,
                        rule.input_len_p1,
                        &rule.input,
                        &rule.lookahead,
                        &rule.lookups,
                        i as u32,
                        &mut lc,
                        true,
                    );
                }
            }
            c.pop_cur_done_glyphs();
        }
        3 => {
            let (backtrack, input, lookahead, lookups) = parse_chain3(sub);
            let Some(&first) = input.first() else { return };
            if !coverage_at(sub, first).intersects(c.glyphs) {
                return;
            }
            c.active_glyphs_stack.push(BTreeSet::new());
            let prev = c.previous_parent_active_glyphs().clone();
            let cur = coverage_at(sub, first).intersect_set(&prev);
            *c.active_glyphs_stack.last_mut().expect("pushed") = cur;
            let mut lc = ClosureLookupContext {
                format: ContextFormat::CoverageBased,
                data: [IData::Coverage(sub); 3],
                intersected_cache: HashMap::new(),
            };
            closure_lookup(
                c,
                &backtrack,
                input.len() as u32,
                &input[1..],
                &lookahead,
                &lookups,
                0,
                &mut lc,
                true,
            );
            c.pop_cur_done_glyphs();
        }
        _ => {}
    }
}

/// `ChainContext::dispatch (hb_closure_lookups_context_t)`.
pub(crate) fn chain_context_closure_lookups(c: &mut ClosureLookupsCtx<'_>, sub: View<'_>) {
    match sub.u16(0) {
        1 => {
            let cov = Coverage(sub.off16(2));
            let len = sub.u16(4) as usize;
            let data = [IData::Glyph; 3];
            for (i, g) in cov.iter().into_iter().take(len).enumerate() {
                if !c.glyphs.contains(&g) {
                    continue;
                }
                if c.lookup_limit_exceeded_pub() {
                    continue;
                }
                for r in rule_set_rules(sub.off16(6 + 2 * i)) {
                    if c.lookup_limit_exceeded_pub() {
                        continue;
                    }
                    let rule = parse_chain_rule(r);
                    if !chain_intersects(c.glyphs, &rule, &data) {
                        continue;
                    }
                    for &(_, l) in &rule.lookups {
                        c.recurse(l);
                    }
                }
            }
        }
        2 => {
            let cov = Coverage(sub.off16(2));
            if !cov.intersects(c.glyphs) {
                return;
            }
            let backtrack_cd = ClassDef(sub.off16(4));
            let input_cd = ClassDef(sub.off16(6));
            let lookahead_cd = ClassDef(sub.off16(8));
            let data = [IData::Class(backtrack_cd), IData::Class(input_cd), IData::Class(lookahead_cd)];
            let n = sub.u16(10) as usize;
            for i in 0..n {
                if !input_cd.intersects_class(c.glyphs, i as u32) {
                    continue;
                }
                if c.lookup_limit_exceeded_pub() {
                    continue;
                }
                for r in rule_set_rules(sub.off16(12 + 2 * i)) {
                    if c.lookup_limit_exceeded_pub() {
                        continue;
                    }
                    let rule = parse_chain_rule(r);
                    if !chain_intersects(c.glyphs, &rule, &data) {
                        continue;
                    }
                    for &(_, l) in &rule.lookups {
                        c.recurse(l);
                    }
                }
            }
        }
        3 => {
            if !chain_context_intersects(sub, c.glyphs) {
                return;
            }
            let (_, _, _, lookups) = parse_chain3(sub);
            for &(_, l) in &lookups {
                c.recurse(l);
            }
        }
        _ => {}
    }
}

/// `ChainRule::serialize` (hb-ot-layout-gsubgpos.hh#L3310-L3347).
fn chain_rule_serialize(
    s: &mut Serializer,
    rule: &ChainRule,
    lookup_map: &HashMap<u32, u32>,
    backtrack_map: &HashMap<u32, u32>,
    input_map: Option<&HashMap<u32, u32>>,
    lookahead_map: Option<&HashMap<u32, u32>>,
) {
    let mut mapping = backtrack_map;
    let get = |m: &HashMap<u32, u32>, v: u32| m.get(&v).copied().unwrap_or(INVALID);
    s.embed_u16(rule.backtrack.len() as u16);
    for &v in &rule.backtrack {
        s.embed_u16(get(mapping, v) as u16);
    }
    if let Some(m) = input_map {
        mapping = m;
    }
    s.embed_u16(rule.input_len_p1 as u16);
    for &v in &rule.input {
        s.embed_u16(get(mapping, v) as u16);
    }
    if let Some(m) = lookahead_map {
        mapping = m;
    }
    s.embed_u16(rule.lookahead.len() as u16);
    for &v in &rule.lookahead {
        s.embed_u16(get(mapping, v) as u16);
    }
    let lookup_count = s.embed_u16(rule.lookups.len() as u16);
    let count = serialize_lookuprecord_array(s, &rule.lookups, lookup_map);
    if count > 0xFFFF {
        s.err(ERROR_INT_OVERFLOW);
    }
    s.set_u16(lookup_count, count as u16);
}

/// The maps `ChainRule::subset` takes: none for format 1.
#[derive(Clone, Copy)]
struct ChainMaps<'m> {
    backtrack: Option<&'m HashMap<u32, u32>>,
    input: Option<&'m HashMap<u32, u32>>,
    lookahead: Option<&'m HashMap<u32, u32>>,
}

/// `ChainRule::subset` (hb-ot-layout-gsubgpos.hh#L3349-L3400).
fn chain_rule_subset(
    plan: &Plan<'_>,
    s: &mut Serializer,
    r: View<'_>,
    lookup_map: &HashMap<u32, u32>,
    maps: ChainMaps<'_>,
) -> bool {
    let rule = parse_chain_rule(r);
    match maps.backtrack {
        None => {
            let glyphset = &plan.glyphset_gsub;
            if !rule.backtrack.iter().all(|g| glyphset.contains(g))
                || !rule.input.iter().all(|g| glyphset.contains(g))
                || !rule.lookahead.iter().all(|g| glyphset.contains(g))
            {
                return false;
            }
            chain_rule_serialize(s, &rule, lookup_map, &plan.glyph_map, None, None);
        }
        Some(bm) => {
            let (Some(im), Some(lm)) = (maps.input, maps.lookahead) else { return false };
            if !rule.backtrack.iter().all(|g| bm.contains_key(g))
                || !rule.input.iter().all(|g| im.contains_key(g))
                || !rule.lookahead.iter().all(|g| lm.contains_key(g))
            {
                return false;
            }
            chain_rule_serialize(s, &rule, lookup_map, bm, Some(im), Some(lm));
        }
    }
    true
}

/// `ChainRuleSet::subset` (hb-ot-layout-gsubgpos.hh#L3640-L3685).
fn chain_rule_set_subset(
    plan: &Plan<'_>,
    s: &mut Serializer,
    rs: View<'_>,
    lookup_map: &HashMap<u32, u32>,
    maps: ChainMaps<'_>,
) -> bool {
    let snap = s.snapshot();
    let out = s.allocate(2);
    for i in 0..rs.u16(0) as usize {
        if rs.is_null16(2 + 2 * i) {
            continue;
        }
        let o_snap = s.snapshot();
        let o = s.array_append(out, 2);
        let r = rs.off16(2 + 2 * i);
        if !s.serialize_subset(o, 2, true, |s| chain_rule_subset(plan, s, r, lookup_map, maps)) {
            s.array_pop(out);
            s.revert(o_snap);
        }
    }
    let ret = View::new(s.bytes()).u16(0) != 0;
    if !ret {
        s.revert(snap);
    }
    ret
}

/// `ChainContext::dispatch (hb_subset_context_t)`.
pub(crate) fn chain_context_subset(plan: &Plan<'_>, s: &mut Serializer, kind: Kind, sub: View<'_>) -> bool {
    let lookup_map = lookup_map_of(plan, kind);
    let cdp = ClassDefPlan {
        glyph_map_gsub: &plan.glyph_map_gsub,
        glyphset_gsub: &plan.glyphset_gsub,
        num_source_glyphs: plan.source.num_glyphs(),
    };
    match sub.u16(0) {
        1 => {
            // ChainContextFormat1_4::subset
            let out = s.allocate(6);
            s.set_u16(out, 1);
            let cov = Coverage(sub.off16(2));
            let len = sub.u16(4) as usize;
            let mut new_coverage: Vec<u32> = Vec::new();
            for (i, g) in cov.iter().into_iter().take(len).enumerate() {
                if !plan.glyphset_gsub.contains(&g) {
                    continue;
                }
                let snap = s.snapshot();
                let o = s.array_append(out + 4, 2);
                let ret = if sub.is_null16(6 + 2 * i) {
                    s.zero_field(o, 2);
                    false
                } else {
                    let rs = sub.off16(6 + 2 * i);
                    s.serialize_subset(o, 2, true, |s| {
                        chain_rule_set_subset(
                            plan,
                            s,
                            rs,
                            lookup_map,
                            ChainMaps { backtrack: None, input: None, lookahead: None },
                        )
                    })
                };
                if !ret {
                    s.array_pop(out + 4);
                    s.revert(snap);
                } else {
                    new_coverage.push(plan.glyph_map.get(&g).copied().unwrap_or(INVALID));
                }
            }
            s.serialize_serialize(out + 2, 2, |s| coverage_serialize(s, &new_coverage));
            !new_coverage.is_empty()
        }
        2 => {
            // ChainContextFormat2_5::subset
            let out = s.allocate(12);
            s.set_u16(out, 2);
            let cov = Coverage(sub.off16(2));
            if sub.is_null16(2) {
                s.zero_field(out + 2, 2);
            } else {
                s.serialize_subset(out + 2, 2, true, |s| {
                    coverage_subset(s, cov, plan.source.num_glyphs(), &plan.glyph_map_gsub)
                });
            }
            let mut backtrack_klass_map: HashMap<u32, u32> = HashMap::new();
            let mut input_klass_map: HashMap<u32, u32> = HashMap::new();
            let mut lookahead_klass_map: HashMap<u32, u32> = HashMap::new();
            for (pos, map) in
                [(4usize, &mut backtrack_klass_map), (6, &mut input_klass_map), (8, &mut lookahead_klass_map)]
            {
                if sub.is_null16(pos) {
                    s.zero_field(out + pos, 2);
                    continue;
                }
                let cd = ClassDef(sub.off16(pos));
                s.serialize_subset(out + pos, 2, true, |s| {
                    classdef_subset(s, cd, &cdp, ClassDefSubsetArgs { klass_map: Some(map), ..Default::default() })
                });
            }
            let retained_coverage_glyphs = cov.intersect_set(&plan.glyphset_gsub);
            let mut coverage_glyph_classes = BTreeSet::new();
            ClassDef(sub.off16(6)).intersected_classes(&retained_coverage_glyphs, &mut coverage_glyph_classes);

            let n = sub.u16(10) as usize;
            let mut non_zero_index: i64 = -1;
            let mut index: i64 = 0;
            let mut last_non_zero = s.snapshot();
            let maps = ChainMaps {
                backtrack: Some(&backtrack_klass_map),
                input: Some(&input_klass_map),
                lookahead: Some(&lookahead_klass_map),
            };
            for i in 0..n {
                if !input_klass_map.contains_key(&(i as u32)) {
                    continue;
                }
                let o = s.array_append(out + 10, 2);
                let ok = coverage_glyph_classes.contains(&(i as u32)) && {
                    if sub.is_null16(12 + 2 * i) {
                        s.zero_field(o, 2);
                        false
                    } else {
                        let rs = sub.off16(12 + 2 * i);
                        s.serialize_subset(o, 2, true, |s| chain_rule_set_subset(plan, s, rs, lookup_map, maps))
                    }
                };
                if ok {
                    last_non_zero = s.snapshot();
                    non_zero_index = index;
                }
                index += 1;
            }
            if non_zero_index == -1 {
                return false;
            }
            // prune empty trailing ruleSets
            if index > non_zero_index {
                s.revert(last_non_zero);
                s.set_u16(out + 10, (non_zero_index + 1) as u16);
            }
            View::new(s.bytes()).u16(out + 10) != 0
        }
        3 => {
            // ChainContextFormat3::subset
            let (backtrack, input, lookahead, lookups) = parse_chain3(sub);
            s.embed_u16(3);
            let covs = |s: &mut Serializer, offs: &[u32]| -> bool {
                let len_pos = s.allocate(2);
                let _ = len_pos;
                for &off in offs {
                    // `out->serialize_append`: the array starts at `len_pos`; the offsets follow it.
                    let len = View::new(s.bytes()).u16(len_pos);
                    s.set_u16(len_pos, (len + 1) as u16);
                    let o = s.allocate(2);
                    if off == 0 {
                        s.zero_field(o, 2);
                        return false;
                    }
                    let cov = Coverage(sub.sub(off as usize));
                    if !s.serialize_subset(o, 2, true, |s| {
                        coverage_subset(s, cov, plan.source.num_glyphs(), &plan.glyph_map_gsub)
                    }) {
                        return false;
                    }
                }
                true
            };
            if !covs(s, &backtrack) || !covs(s, &input) || !covs(s, &lookahead) {
                return false;
            }
            let lookup_count = s.embed_u16(lookups.len() as u16);
            let count = serialize_lookuprecord_array(s, &lookups, lookup_map);
            if count > 0xFFFF {
                s.err(ERROR_INT_OVERFLOW);
            }
            s.set_u16(lookup_count, count as u16);
            true
        }
        _ => true,
    }
}

#[allow(dead_code)]
fn _whence_marker() -> Whence {
    Whence::Head
}
