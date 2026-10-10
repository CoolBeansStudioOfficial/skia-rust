// Copyright © 2007,2008,2009,2010  Red Hat, Inc.
// Copyright © 2010,2012  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-ot-layout-gsubgpos.hh (closure contexts, GSUBGPOS), src/hb-ot-layout-common.hh
// (ScriptList, FeatureList, LookupList, Lookup), src/hb-ot-layout.cc (collect_features,
// lookups_substitute_closure), src/hb-subset-plan-layout.cc (harfbuzz 9cb1fee5)

//! The parts of `GSUB` and `GPOS` that the tables share: the closure contexts, feature
//! collection, the lists, lookups and the table subsetting. The lookup subtables are in
//! [`crate::gsub`], [`crate::gpos`] and [`crate::context`].

use std::collections::{BTreeSet, HashMap};

use crate::bytes::tag;
use crate::context;
use crate::gpos;
use crate::gsub;
use crate::ot::{Coverage, INVALID, View, set_add_range};
use crate::plan::{Plan, unsupported};
use crate::serialize::{Serializer, Whence};
use crate::{Res, SubsetError};

pub(crate) const HB_MAX_NESTING_LEVEL: u32 = 64;
const HB_MAX_SCRIPTS: u32 = 500;
const HB_MAX_LANGSYS: u32 = 2000;
const HB_MAX_LANGSYS_FEATURE_COUNT: u32 = 50000;
const HB_MAX_FEATURE_INDICES: u32 = 1500;
pub(crate) const HB_MAX_LOOKUP_VISIT_COUNT: u32 = 35000;
const HB_CLOSURE_MAX_STAGES: u32 = 12;

const TAG_DFLT: u32 = tag(b"DFLT");
const TAG_PREF: u32 = tag(b"pref");
const TAG_SIZE: u32 = tag(b"size");

/// Which of the two tables.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Gsub,
    Gpos,
}

impl Kind {
    pub(crate) fn tag(self) -> u32 {
        match self {
            Kind::Gsub => tag(b"GSUB"),
            Kind::Gpos => tag(b"GPOS"),
        }
    }
}

/// `GSUBGPOS` version 1.x with 16 bit offsets.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Gsubgpos<'a> {
    pub v: View<'a>,
}

impl<'a> Gsubgpos<'a> {
    /// `None` for an unsupported major version (version 2 uses 24 bit offsets).
    pub(crate) fn new(d: &'a [u8]) -> Option<Self> {
        let v = View::new(d);
        if v.u16(0) == 1 {
            Some(Gsubgpos { v })
        } else {
            None
        }
    }

    pub(crate) fn version(&self) -> u32 {
        self.v.u32(0)
    }

    pub(crate) fn script_list(&self) -> View<'a> {
        self.v.off16(4)
    }

    pub(crate) fn feature_list(&self) -> View<'a> {
        self.v.off16(6)
    }

    pub(crate) fn lookup_list(&self) -> View<'a> {
        self.v.off16(8)
    }

    pub(crate) fn get_lookup_count(&self) -> u32 {
        self.lookup_list().u16(0)
    }

    /// `get_lookup (i)`: the empty (`Null`) lookup past the end.
    pub(crate) fn get_lookup(&self, i: u32) -> View<'a> {
        let ll = self.lookup_list();
        if i >= ll.u16(0) {
            View::new(&[])
        } else {
            ll.off16(2 + 2 * i as usize)
        }
    }

    pub(crate) fn get_script_count(&self) -> u32 {
        self.script_list().u16(0)
    }

    pub(crate) fn get_script_tag(&self, i: u32) -> u32 {
        self.script_list().u32(2 + 6 * i as usize)
    }

    pub(crate) fn get_script(&self, i: u32) -> View<'a> {
        let sl = self.script_list();
        if i >= sl.u16(0) {
            View::new(&[])
        } else {
            sl.off16(2 + 6 * i as usize + 4)
        }
    }

    pub(crate) fn get_feature_count(&self) -> u32 {
        self.feature_list().u16(0)
    }

    /// `get_feature_tag (i)`; `HB_TAG_NONE` for `Index::NOT_FOUND_INDEX`.
    pub(crate) fn get_feature_tag(&self, i: u32) -> u32 {
        if i == INVALID {
            return 0;
        }
        let fl = self.feature_list();
        if i >= fl.u16(0) {
            0
        } else {
            fl.u32(2 + 6 * i as usize)
        }
    }

    pub(crate) fn get_feature(&self, i: u32) -> View<'a> {
        let fl = self.feature_list();
        if i >= fl.u16(0) {
            View::new(&[])
        } else {
            fl.off16(2 + 6 * i as usize + 4)
        }
    }

    /// `featureVars` of a version 1.1 table.
    pub(crate) fn has_feature_variations(&self) -> bool {
        self.version() >= 0x0001_0001 && self.v.u32(10) != 0
    }
}

/// `Lookup` accessors (hb-ot-layout-common.hh#L1257-L1340).
pub(crate) fn lookup_type(l: View<'_>) -> u32 {
    l.u16(0)
}

pub(crate) fn lookup_flag(l: View<'_>) -> u32 {
    l.u16(2)
}

pub(crate) fn lookup_subtable_count(l: View<'_>) -> u32 {
    l.u16(4)
}

pub(crate) fn lookup_subtable<'a>(l: View<'a>, i: u32) -> View<'a> {
    l.off16(6 + 2 * i as usize)
}

const USE_MARK_FILTERING_SET: u32 = 0x0010;
const IGNORE_MARKS: u32 = 0x0008;

// -------------------------------------------------------------------------------------------
// Subtable dispatch
// -------------------------------------------------------------------------------------------

/// The lookup type of the extension lookup of the table kind.
fn extension_type(kind: Kind) -> u32 {
    match kind {
        Kind::Gsub => 7,
        Kind::Gpos => 9,
    }
}

/// `ExtensionFormat1`: `(inner lookup type, inner subtable)` when the subtable is format 1.
pub(crate) fn extension_inner<'a>(sub: View<'a>) -> Option<(u32, View<'a>)> {
    if sub.u16(0) == 1 {
        Some((sub.u16(2), sub.off32(4)))
    } else {
        None
    }
}

/// `SubTable::intersects (glyphs, lookup_type)` through `hb_intersects_context_t`.
pub(crate) fn subtable_intersects(
    kind: Kind,
    lookup_type: u32,
    sub: View<'_>,
    glyphs: &BTreeSet<u32>,
) -> bool {
    if lookup_type == extension_type(kind) {
        return extension_inner(sub)
            .is_some_and(|(t, inner)| subtable_intersects(kind, t, inner, glyphs));
    }
    match kind {
        Kind::Gsub => gsub::intersects(lookup_type, sub, glyphs),
        Kind::Gpos => gpos::intersects(lookup_type, sub, glyphs),
    }
}

/// `Lookup::intersects`: `hb_intersects_context_t` stops at the first subtable that does.
pub(crate) fn lookup_intersects(kind: Kind, l: View<'_>, glyphs: &BTreeSet<u32>) -> bool {
    let t = lookup_type(l);
    (0..lookup_subtable_count(l))
        .any(|i| subtable_intersects(kind, t, lookup_subtable(l, i), glyphs))
}

/// `hb_have_non_1to1_context_t` over a subtable (GSUB only).
fn subtable_may_have_non_1to1(lookup_type: u32, sub: View<'_>) -> bool {
    if lookup_type == 7 {
        return extension_inner(sub).is_some_and(|(t, inner)| subtable_may_have_non_1to1(t, inner));
    }
    // Multiple, Ligature, Context and ChainContext may; Single, Alternate and ReverseChain do not.
    matches!(lookup_type, 2 | 4 | 5 | 6)
}

/// `SubstLookup::may_have_non_1to1`.
fn lookup_may_have_non_1to1(l: View<'_>) -> bool {
    let t = lookup_type(l);
    (0..lookup_subtable_count(l)).any(|i| subtable_may_have_non_1to1(t, lookup_subtable(l, i)))
}

// -------------------------------------------------------------------------------------------
// hb_closure_context_t (GSUB glyph closure)
// -------------------------------------------------------------------------------------------

/// Port of `hb_closure_context_t` (hb-ot-layout-gsubgpos.hh#L67-L200).
pub(crate) struct ClosureCtx<'a> {
    num_glyphs: u32,
    pub glyphs: &'a mut BTreeSet<u32>,
    pub output: BTreeSet<u32>,
    pub active_glyphs_stack: Vec<BTreeSet<u32>>,
    pub nesting_level_left: u32,
    done_lookups_glyph_count: HashMap<u32, u32>,
    done_lookups_glyph_set: HashMap<u32, BTreeSet<u32>>,
    lookup_count: u32,
    gsub: Gsubgpos<'a>,
}

impl<'a> ClosureCtx<'a> {
    pub(crate) fn new(num_glyphs: u32, glyphs: &'a mut BTreeSet<u32>, gsub: Gsubgpos<'a>) -> Self {
        ClosureCtx {
            num_glyphs,
            glyphs,
            output: BTreeSet::new(),
            active_glyphs_stack: Vec::new(),
            nesting_level_left: HB_MAX_NESTING_LEVEL,
            done_lookups_glyph_count: HashMap::new(),
            done_lookups_glyph_set: HashMap::new(),
            lookup_count: 0,
            gsub,
        }
    }

    /// `recurse (lookup_index, covered_seq_indicies, seq_index, end_index)` with
    /// `dispatch_closure_recurse_func` as the recurse function.
    pub(crate) fn recurse(
        &mut self,
        lookup_index: u32,
        covered_seq_indices: &mut BTreeSet<u32>,
        seq_index: u32,
        end_index: u32,
    ) {
        if self.nesting_level_left == 0 {
            return;
        }
        self.nesting_level_left -= 1;
        // `dispatch_closure_recurse_func`
        if self.should_visit_lookup(lookup_index) {
            // `closure_glyphs_recurse_func`
            let l = self.gsub.get_lookup(lookup_index);
            if lookup_may_have_non_1to1(l) {
                set_add_range(covered_seq_indices, seq_index, end_index);
            }
            self.lookup_dispatch(l);
        }
        self.nesting_level_left += 1;
    }

    /// `Lookup::dispatch<SubTable> (c)` for the closure context.
    fn lookup_dispatch(&mut self, l: View<'a>) {
        let t = lookup_type(l);
        for i in 0..lookup_subtable_count(l) {
            subtable_closure(self, t, lookup_subtable(l, i));
        }
    }

    pub(crate) fn reset_lookup_visit_count(&mut self) {
        self.lookup_count = 0;
    }

    pub(crate) fn lookup_limit_exceeded(&self) -> bool {
        self.lookup_count > HB_MAX_LOOKUP_VISIT_COUNT
    }

    fn should_visit_lookup(&mut self, lookup_index: u32) -> bool {
        let over = self.lookup_count > HB_MAX_LOOKUP_VISIT_COUNT;
        self.lookup_count += 1;
        if over {
            return false;
        }
        !self.is_lookup_done(lookup_index)
    }

    fn is_lookup_done(&mut self, lookup_index: u32) -> bool {
        let pop = self.glyphs.len() as u32;
        if self
            .done_lookups_glyph_count
            .get(&lookup_index)
            .copied()
            .unwrap_or(INVALID)
            != pop
        {
            self.done_lookups_glyph_count.insert(lookup_index, pop);
            self.done_lookups_glyph_set
                .entry(lookup_index)
                .or_default()
                .clear();
        }
        let parent = self.parent_active_glyphs().clone();
        let covered = self.done_lookups_glyph_set.entry(lookup_index).or_default();
        if parent.is_subset(covered) {
            return true;
        }
        covered.extend(parent);
        false
    }

    pub(crate) fn previous_parent_active_glyphs(&self) -> &BTreeSet<u32> {
        if self.active_glyphs_stack.len() <= 1 {
            return self.glyphs;
        }
        &self.active_glyphs_stack[self.active_glyphs_stack.len() - 2]
    }

    pub(crate) fn parent_active_glyphs(&self) -> &BTreeSet<u32> {
        self.active_glyphs_stack.last().unwrap_or(self.glyphs)
    }

    pub(crate) fn pop_cur_done_glyphs(&mut self) -> bool {
        self.active_glyphs_stack.pop().is_some()
    }

    fn flush(&mut self) {
        let num_glyphs = self.num_glyphs;
        let doomed: Vec<u32> = self.output.range(num_glyphs..).copied().collect();
        for g in doomed {
            self.output.remove(&g);
        }
        let out = std::mem::take(&mut self.output);
        self.glyphs.extend(out);
        self.active_glyphs_stack.pop();
        self.active_glyphs_stack.clear();
    }

    /// `SubstLookup::closure (c, this_index)`.
    pub(crate) fn lookup_closure(&mut self, this_index: u32) {
        if !self.should_visit_lookup(this_index) {
            return;
        }
        let l = self.gsub.get_lookup(this_index);
        self.lookup_dispatch(l);
        self.flush();
    }
}

/// `SubstLookupSubTable::dispatch (closure context)`.
fn subtable_closure(c: &mut ClosureCtx<'_>, lookup_type: u32, sub: View<'_>) {
    if lookup_type == 7 {
        if let Some((t, inner)) = extension_inner(sub) {
            subtable_closure(c, t, inner);
        }
        return;
    }
    gsub::closure(c, lookup_type, sub);
}

/// `hb_ot_layout_lookups_substitute_closure` (hb-ot-layout.cc#L1616-L1650).
fn lookups_substitute_closure(
    num_glyphs: u32,
    gsub: Gsubgpos<'_>,
    lookups: &BTreeSet<u32>,
    glyphs: &mut BTreeSet<u32>,
) {
    let mut c = ClosureCtx::new(num_glyphs, glyphs, gsub);
    let mut iteration_count = 0;
    loop {
        c.reset_lookup_visit_count();
        let glyphs_length = c.glyphs.len();
        for &lookup_index in lookups {
            c.lookup_closure(lookup_index);
        }
        let cont = iteration_count <= HB_CLOSURE_MAX_STAGES && glyphs_length != c.glyphs.len();
        iteration_count += 1;
        if !cont {
            break;
        }
    }
    c.flush();
}

// -------------------------------------------------------------------------------------------
// hb_closure_lookups_context_t
// -------------------------------------------------------------------------------------------

/// Port of `hb_closure_lookups_context_t` (hb-ot-layout-gsubgpos.hh#L203-L280).
pub(crate) struct ClosureLookupsCtx<'a> {
    pub kind: Kind,
    pub glyphs: &'a BTreeSet<u32>,
    pub nesting_level_left: u32,
    visited_lookups: BTreeSet<u32>,
    inactive_lookups: BTreeSet<u32>,
    lookup_count: u32,
    table: Gsubgpos<'a>,
}

impl<'a> ClosureLookupsCtx<'a> {
    fn lookup_limit_exceeded(&self) -> bool {
        self.lookup_count > HB_MAX_LOOKUP_VISIT_COUNT
    }

    pub(crate) fn lookup_limit_exceeded_pub(&self) -> bool {
        self.lookup_limit_exceeded()
    }

    fn is_lookup_visited(&mut self, lookup_index: u32) -> bool {
        let over = self.lookup_count > HB_MAX_LOOKUP_VISIT_COUNT;
        self.lookup_count += 1;
        if over {
            return true;
        }
        self.visited_lookups.contains(&lookup_index)
    }

    /// `recurse (lookup_index)`.
    pub(crate) fn recurse(&mut self, lookup_index: u32) {
        if self.nesting_level_left == 0 {
            return;
        }
        if self.lookup_limit_exceeded() || self.visited_lookups.contains(&lookup_index) {
            // Don't increment lookup count here, that will be done in the call to
            // closure_lookups() made by recurse_func.
            return;
        }
        self.nesting_level_left -= 1;
        self.lookup_closure_lookups(lookup_index);
        self.nesting_level_left += 1;
    }

    /// `Lookup::closure_lookups (c, this_index)`.
    fn lookup_closure_lookups(&mut self, this_index: u32) {
        if self.is_lookup_visited(this_index) {
            return;
        }
        self.visited_lookups.insert(this_index);
        let l = self.table.get_lookup(this_index);
        if !lookup_intersects(self.kind, l, self.glyphs) {
            self.inactive_lookups.insert(this_index);
            return;
        }
        let t = lookup_type(l);
        for i in 0..lookup_subtable_count(l) {
            subtable_closure_lookups(self, t, lookup_subtable(l, i));
        }
    }
}

/// `SubTable::dispatch (closure_lookups context)`.
fn subtable_closure_lookups(c: &mut ClosureLookupsCtx<'_>, lookup_type: u32, sub: View<'_>) {
    if lookup_type == extension_type(c.kind) {
        if let Some((t, inner)) = extension_inner(sub) {
            subtable_closure_lookups(c, t, inner);
        }
        return;
    }
    // Only the context lookups have nested lookups; the other subtables' `closure_lookups` do
    // nothing.
    let (ctx_type, chain_type) = match c.kind {
        Kind::Gsub => (5, 6),
        Kind::Gpos => (7, 8),
    };
    if lookup_type == ctx_type {
        context::context_closure_lookups(c, sub);
    } else if lookup_type == chain_type {
        context::chain_context_closure_lookups(c, sub);
    }
}

/// `GSUBGPOS::closure_lookups<TLookup>` (hb-ot-layout-gsubgpos.hh#L4867-L4883).
fn closure_lookups(
    kind: Kind,
    table: Gsubgpos<'_>,
    glyphs: &BTreeSet<u32>,
    lookup_indexes: &mut BTreeSet<u32>,
) {
    let mut c = ClosureLookupsCtx {
        kind,
        glyphs,
        nesting_level_left: HB_MAX_NESTING_LEVEL,
        visited_lookups: BTreeSet::new(),
        inactive_lookups: BTreeSet::new(),
        lookup_count: 0,
        table,
    };
    for &lookup_index in lookup_indexes.iter() {
        c.lookup_closure_lookups(lookup_index);
    }
    lookup_indexes.extend(c.visited_lookups.iter().copied());
    for l in &c.inactive_lookups {
        lookup_indexes.remove(l);
    }
}

// -------------------------------------------------------------------------------------------
// Feature collection (hb-ot-layout.cc)
// -------------------------------------------------------------------------------------------

/// Port of `hb_collect_features_context_t` (hb-ot-layout.cc#L1088-L1170).
struct CollectFeaturesCtx<'a> {
    g: Gsubgpos<'a>,
    feature_indices: BTreeSet<u32>,
    feature_indices_filter: BTreeSet<u32>,
    has_feature_filter: bool,
    visited_script: BTreeSet<usize>,
    visited_langsys: BTreeSet<usize>,
    script_count: u32,
    langsys_count: u32,
    feature_index_count: u32,
}

/// The offset of `inner` in `outer` (`(uintptr_t) &p - (uintptr_t) &g`).
fn offset_in(outer: &[u8], inner: &[u8]) -> usize {
    (inner.as_ptr() as usize).wrapping_sub(outer.as_ptr() as usize)
}

impl CollectFeaturesCtx<'_> {
    fn visited_script(&mut self, s: View<'_>) -> bool {
        // We might have Null() object here. Don't want to involve that in the memoize.
        if s.u16(0) == 0 && s.u16(2) == 0 {
            return true;
        }
        let over = self.script_count > HB_MAX_SCRIPTS;
        self.script_count += 1;
        if over {
            return true;
        }
        let delta = offset_in(self.g.v.d, s.d);
        !self.visited_script.insert(delta)
    }

    fn visited_langsys(&mut self, l: View<'_>) -> bool {
        // has_required_feature () is reqFeatureIndex != 0xFFFF; get_feature_count () is featureIndex.len
        if l.u16(2) == 0xFFFF && l.u16(4) == 0 {
            return true;
        }
        let over = self.langsys_count > HB_MAX_LANGSYS;
        self.langsys_count += 1;
        if over {
            return true;
        }
        let delta = offset_in(self.g.v.d, l.d);
        !self.visited_langsys.insert(delta)
    }

    fn visited_feature_indices(&mut self, count: u32) -> bool {
        self.feature_index_count += count;
        self.feature_index_count > HB_MAX_FEATURE_INDICES
    }

    /// `langsys_collect_features`.
    fn langsys_collect_features(&mut self, l: View<'_>) {
        if self.visited_langsys(l) {
            return;
        }
        let feature_count = l.u16(4);
        if !self.has_feature_filter {
            // All features.
            if l.u16(2) != 0xFFFF && !self.visited_feature_indices(1) {
                self.feature_indices.insert(l.u16(2));
            }
            if !self.visited_feature_indices(feature_count) {
                for i in 0..feature_count as usize {
                    self.feature_indices.insert(l.u16(6 + 2 * i));
                }
            }
        } else {
            if self.feature_indices_filter.is_empty() {
                return;
            }
            for i in 0..feature_count as usize {
                let feature_index = l.u16(6 + 2 * i);
                if !self.feature_indices_filter.contains(&feature_index) {
                    continue;
                }
                self.feature_indices.insert(feature_index);
                self.feature_indices_filter.remove(&feature_index);
            }
        }
    }

    /// `script_collect_features` with all languages.
    fn script_collect_features(&mut self, s: View<'_>) {
        if self.visited_script(s) {
            return;
        }
        // All languages.
        if s.u16(0) != 0 {
            let d = s.off16(0);
            self.langsys_collect_features(d);
        }
        for i in 0..s.u16(2) as usize {
            let l = s.off16(4 + 6 * i + 4);
            self.langsys_collect_features(l);
        }
    }
}

/// `hb_ot_layout_collect_features` with the scripts either all (`None`) or the listed tags, all
/// languages, and the features either all (`None`) or the listed tags.
fn collect_features(
    g: Gsubgpos<'_>,
    scripts: Option<&[u32]>,
    features: Option<&[u32]>,
) -> BTreeSet<u32> {
    let mut c = CollectFeaturesCtx {
        g,
        feature_indices: BTreeSet::new(),
        feature_indices_filter: BTreeSet::new(),
        has_feature_filter: false,
        visited_script: BTreeSet::new(),
        visited_langsys: BTreeSet::new(),
        script_count: 0,
        langsys_count: 0,
        feature_index_count: 0,
    };
    // `compute_feature_filter`
    if let Some(features) = features {
        c.has_feature_filter = true;
        let features_set: BTreeSet<u32> =
            features.iter().copied().take_while(|&t| t != 0).collect();
        for i in 0..g.get_feature_count() {
            if features_set.contains(&g.get_feature_tag(i)) {
                c.feature_indices_filter.insert(i);
            }
        }
    }
    match scripts {
        None => {
            // All scripts.
            for script_index in 0..g.get_script_count() {
                let s = g.get_script(script_index);
                c.script_collect_features(s);
            }
        }
        Some(scripts) => {
            for &script in scripts.iter().take_while(|&&t| t != 0) {
                // `find_script_index`: a binary search of the sorted script records.
                let sl = g.script_list();
                let found =
                    crate::bytes::bsearch(sl.u16(0) as usize, |i| script.cmp(&sl.u32(2 + 6 * i)));
                if let Some(i) = found {
                    let s = g.get_script(i as u32);
                    c.script_collect_features(s);
                }
            }
        }
    }
    c.feature_indices
}

// -------------------------------------------------------------------------------------------
// Plan (hb-subset-plan-layout.cc)
// -------------------------------------------------------------------------------------------

/// `_filter_tag_list` (hb-subset-plan-layout.cc#L53-L81): removes tags not in `filter` (`None` =
/// every tag) and duplicates, returns whether anything was removed.
fn filter_tag_list(tags: &mut Vec<u32>, filter: Option<&BTreeSet<u32>>) -> bool {
    let mut out = Vec::new();
    let mut removed = false;
    let mut visited = BTreeSet::new();
    for &t in tags.iter() {
        if t == 0 || visited.contains(&t) {
            continue;
        }
        if let Some(f) = filter {
            if !f.contains(&t) {
                removed = true;
                continue;
            }
        }
        visited.insert(t);
        out.push(t);
    }
    // The collect function needs a null element to signal end of the array.
    out.push(0);
    *tags = out;
    removed
}

/// The result of the planning of one table.
#[derive(Default)]
pub(crate) struct TablePlan {
    pub lookups: HashMap<u32, u32>,
    pub features: HashMap<u32, u32>,
    pub features_w_duplicates: HashMap<u32, u32>,
    pub langsys: HashMap<u32, BTreeSet<u32>>,
}

/// `remap_indexes`: the position of each element in the set.
fn remap_indexes(indexes: &BTreeSet<u32>) -> HashMap<u32, u32> {
    indexes
        .iter()
        .enumerate()
        .map(|(i, &v)| (v, i as u32))
        .collect()
}

/// `_closure_glyphs_lookups_features<T>` (hb-subset-plan-layout.cc#L233-L292). `glyphs` is the
/// `_glyphset_gsub` of the plan.
pub(crate) fn closure_glyphs_lookups_features(
    plan: &Plan<'_>,
    kind: Kind,
    gids_to_retain: &mut BTreeSet<u32>,
) -> Res<TablePlan> {
    let data = plan.source.table(kind.tag());
    let mut out = TablePlan::default();
    let Some(table) = Gsubgpos::new(data) else {
        // A missing table is the `Null` table: nothing to collect.
        if data.is_empty() || View::new(data).u16(0) == 0 {
            return Ok(out);
        }
        return unsupported("GSUB/GPOS version 2");
    };
    if table.has_feature_variations() {
        return unsupported("FeatureVariations");
    }
    let num_glyphs = plan.source.num_glyphs();

    // `_collect_layout_indices`
    let mut features: Vec<u32> = (0..table.get_feature_count())
        .map(|i| table.get_feature_tag(i))
        .collect();
    let retain_all_features = !filter_tag_list(&mut features, Some(&plan.layout_features));
    let mut scripts: Vec<u32> = (0..table.get_script_count())
        .map(|i| table.get_script_tag(i))
        .collect();
    let retain_all_scripts = !filter_tag_list(&mut scripts, None);

    let mut lookup_indices: BTreeSet<u32> = BTreeSet::new();
    let mut feature_indices = collect_features(
        table,
        if retain_all_scripts {
            None
        } else {
            Some(&scripts)
        },
        if retain_all_features {
            None
        } else {
            Some(&features)
        },
    );
    for &fi in &feature_indices {
        let f = table.get_feature(fi);
        for i in 0..f.u16(2) as usize {
            lookup_indices.insert(f.u16(4 + 2 * i));
        }
    }
    // (feature variations are rejected above, and no axes are pinned)

    if kind == Kind::Gsub {
        lookups_substitute_closure(num_glyphs, table, &lookup_indices, gids_to_retain);
    }
    closure_lookups(kind, table, gids_to_retain, &mut lookup_indices);
    out.lookups = remap_indexes(&lookup_indices);

    // prune features (`prune_features`)
    for i in feature_indices.clone() {
        let tag = table.get_feature_tag(i);
        if tag == TAG_PREF {
            // Note: Never ever drop feature 'pref', even if it's empty.
            continue;
        }
        let f = table.get_feature(i);
        if f.u16(0) != 0 && tag == TAG_SIZE {
            continue;
        }
        let intersects =
            (0..f.u16(2) as usize).any(|k| out.lookups.contains_key(&f.u16(4 + 2 * k)));
        if !intersects {
            feature_indices.remove(&i);
        }
    }
    let duplicate_feature_map = find_duplicate_features(table, &out.lookups, &feature_indices);

    let mut new_feature_indices = BTreeSet::new();
    prune_langsys(
        table,
        &duplicate_feature_map,
        &mut out.langsys,
        &mut new_feature_indices,
    );
    // `remap_feature_indices`
    let mut i = 0u32;
    for &fi in &new_feature_indices {
        let f_idx = duplicate_feature_map.get(&fi).copied().unwrap_or(INVALID);
        if let Some(&new_idx) = out.features.get(&f_idx) {
            out.features_w_duplicates.insert(fi, new_idx);
        } else {
            out.features.insert(fi, i);
            out.features_w_duplicates.insert(fi, i);
            i += 1;
        }
    }
    Ok(out)
}

/// `_GSUBGPOS_find_duplicate_features` (hb-subset-plan-layout.cc#L137-L214).
fn find_duplicate_features(
    g: Gsubgpos<'_>,
    lookup_indices: &HashMap<u32, u32>,
    feature_indices: &BTreeSet<u32>,
) -> HashMap<u32, u32> {
    let mut duplicate_feature_map: HashMap<u32, u32> = HashMap::new();
    if feature_indices.is_empty() {
        return duplicate_feature_map;
    }
    let mut unique_features: HashMap<u32, BTreeSet<u32>> = HashMap::new();
    for &i in feature_indices {
        let t = g.get_feature_tag(i);
        if t == INVALID {
            continue;
        }
        if !unique_features.contains_key(&t) {
            unique_features.entry(t).or_default().insert(i);
            duplicate_feature_map.insert(i, i);
            continue;
        }
        let mut found = false;
        let same_tag_features = unique_features.get(&t).cloned().unwrap_or_default();
        for &other_f_index in &same_tag_features {
            let f = g.get_feature(i);
            let other_f = g.get_feature(other_f_index);
            let a: Vec<u32> = (0..f.u16(2) as usize)
                .map(|k| f.u16(4 + 2 * k))
                .filter(|l| lookup_indices.contains_key(l))
                .collect();
            let b: Vec<u32> = (0..other_f.u16(2) as usize)
                .map(|k| other_f.u16(4 + 2 * k))
                .filter(|l| lookup_indices.contains_key(l))
                .collect();
            if a != b {
                continue;
            }
            found = true;
            duplicate_feature_map.insert(i, other_f_index);
            break;
        }
        if !found {
            unique_features.entry(t).or_default().insert(i);
            duplicate_feature_map.insert(i, i);
        }
    }
    duplicate_feature_map
}

/// `GSUBGPOS::prune_langsys` (hb-ot-layout-gsubgpos.hh#L4885-L4902) with `Script::prune_langsys`
/// and `LangSys::collect_features`/`compare` (hb-ot-layout-common.hh#L1005-L1147).
fn prune_langsys(
    g: Gsubgpos<'_>,
    duplicate_feature_map: &HashMap<u32, u32>,
    script_langsys_map: &mut HashMap<u32, BTreeSet<u32>>,
    new_feature_indexes: &mut BTreeSet<u32>,
) {
    let mut script_count = 0u32;
    let mut langsys_feature_count = 0u32;
    // `visitScript` / `visitLangsys`
    let mut visit_script = |script_count: &mut u32| -> bool {
        let r = *script_count < HB_MAX_SCRIPTS;
        *script_count += 1;
        r
    };
    let mut visit_langsys = |count: &mut u32, feature_count: u32| -> bool {
        *count += feature_count;
        *count < HB_MAX_LANGSYS_FEATURE_COUNT
    };

    let collect_features = |l: View<'_>, new_feature_indexes: &mut BTreeSet<u32>| {
        let has_required = l.u16(2) != 0xFFFF;
        let feature_count = l.u16(4);
        if !has_required && feature_count == 0 {
            return;
        }
        if has_required && duplicate_feature_map.contains_key(&l.u16(2)) {
            new_feature_indexes.insert(l.u16(2));
        }
        for i in 0..feature_count as usize {
            let f = l.u16(6 + 2 * i);
            if duplicate_feature_map.contains_key(&f) {
                new_feature_indexes.insert(f);
            }
        }
    };
    // `LangSys::compare`
    let compare = |a: View<'_>, b: View<'_>| -> bool {
        if a.u16(2) != b.u16(2) {
            return false;
        }
        let map = |l: View<'_>| -> Vec<u32> {
            (0..l.u16(4) as usize)
                .map(|i| l.u16(6 + 2 * i))
                .filter_map(|f| {
                    // `hb_filter (feature_index_map)` then `hb_map (feature_index_map)`
                    duplicate_feature_map.get(&f).copied()
                })
                .collect()
        };
        map(a) == map(b)
    };

    for script_index in 0..g.get_script_count() {
        // `layout_scripts->has (tag)`: all scripts.
        let s = g.get_script(script_index);
        // Script::prune_langsys
        let has_default = s.u16(0) != 0;
        let lang_count = s.u16(2) as usize;
        if !has_default && lang_count == 0 {
            continue;
        }
        if !visit_script(&mut script_count) {
            continue;
        }
        script_langsys_map.entry(script_index).or_default();
        if has_default {
            // only collect features from non-redundant langsys
            let d = s.off16(0);
            if visit_langsys(&mut langsys_feature_count, d.u16(4)) {
                collect_features(d, new_feature_indexes);
            }
            for li in 0..lang_count {
                let l = s.off16(4 + 6 * li + 4);
                if !visit_langsys(&mut langsys_feature_count, l.u16(4)) {
                    continue;
                }
                if compare(l, d) {
                    continue;
                }
                collect_features(l, new_feature_indexes);
                script_langsys_map
                    .entry(script_index)
                    .or_default()
                    .insert(li as u32);
            }
        } else {
            for li in 0..lang_count {
                let l = s.off16(4 + 6 * li + 4);
                if !visit_langsys(&mut langsys_feature_count, l.u16(4)) {
                    continue;
                }
                collect_features(l, new_feature_indexes);
                script_langsys_map
                    .entry(script_index)
                    .or_default()
                    .insert(li as u32);
            }
        }
    }
}

/// `layout_populate_gids_to_retain` (hb-subset-plan-layout.cc#L338-L368).
pub(crate) fn populate_gids_to_retain(plan: &mut Plan<'_>) -> Res<()> {
    let mut gids = std::mem::take(&mut plan.glyphset_gsub);
    if !plan.drop_tables.contains(&tag(b"GSUB")) {
        match closure_glyphs_lookups_features(plan, Kind::Gsub, &mut gids) {
            Ok(p) => plan.layout.gsub = p,
            Err(e) => {
                plan.glyphset_gsub = gids;
                return Err(e);
            }
        }
    }
    if !plan.drop_tables.contains(&tag(b"GPOS")) {
        match closure_glyphs_lookups_features(plan, Kind::Gpos, &mut gids) {
            Ok(p) => plan.layout.gpos = p,
            Err(e) => {
                plan.glyphset_gsub = gids;
                return Err(e);
            }
        }
    }
    plan.glyphset_gsub = gids;
    Ok(())
}

/// `GSUBGPOS::collect_name_ids` (hb-ot-layout-gsubgpos.hh#L4946-L4957) over the retained features.
pub(crate) fn collect_name_ids(plan: &mut Plan<'_>, kind: Kind) {
    let data = plan.source.table(kind.tag());
    let Some(g) = Gsubgpos::new(data) else { return };
    let features = match kind {
        Kind::Gsub => &plan.layout.gsub.features,
        Kind::Gpos => &plan.layout.gpos.features,
    };
    let mut ids = Vec::new();
    for i in 0..g.get_feature_count() {
        if !features.contains_key(&i) {
            continue;
        }
        let t = g.get_feature_tag(i);
        let f = g.get_feature(i);
        if f.u16(0) == 0 {
            continue;
        }
        let params = f.off16(0);
        // FeatureParams::collect_name_ids
        if t == TAG_SIZE {
            ids.push(params.u16(6));
        } else if t & 0xFFFF_0000 == tag(b"ss\0\0") {
            ids.push(params.u16(2));
        } else if t & 0xFFFF_0000 == tag(b"cv\0\0") {
            for k in [2usize, 4, 6] {
                if params.u16(k) != 0 {
                    ids.push(params.u16(k));
                }
            }
            let first = params.u16(10);
            let num = params.u16(8);
            if first != 0 && num != 0 && num < 0x7FFF {
                let last = first + num - 1;
                for id in first..=last {
                    ids.push(id);
                }
            }
        }
    }
    plan.name_ids.extend(ids);
}

// -------------------------------------------------------------------------------------------
// Subsetting (hb-subset-layout context)
// -------------------------------------------------------------------------------------------

/// Port of `hb_subset_layout_context_t` (hb-ot-layout-common.hh#L111-L193).
pub(crate) struct SubsetLayoutCtx<'p, 'a> {
    pub plan: &'p Plan<'a>,
    pub kind: Kind,
    script_count: u32,
    langsys_count: u32,
    feature_index_count: u32,
    lookup_index_count: u32,
    cur_script_index: u32,
}

impl SubsetLayoutCtx<'_, '_> {
    fn table_plan(&self) -> &TablePlan {
        match self.kind {
            Kind::Gsub => &self.plan.layout.gsub,
            Kind::Gpos => &self.plan.layout.gpos,
        }
    }

    fn visit_script(&mut self) -> bool {
        let r = self.script_count < HB_MAX_SCRIPTS;
        self.script_count += 1;
        r
    }

    fn visit_lang_sys(&mut self) -> bool {
        let r = self.langsys_count < HB_MAX_LANGSYS;
        self.langsys_count += 1;
        r
    }

    fn visit_feature_index(&mut self, count: u32) -> bool {
        self.feature_index_count += count;
        self.feature_index_count < HB_MAX_FEATURE_INDICES
    }

    fn visit_lookup_index(&mut self) -> bool {
        self.lookup_index_count += 1;
        self.lookup_index_count < HB_MAX_LOOKUP_VISIT_COUNT
    }
}

/// `GSUBGPOS::subset<TLookup>` through `GSUBGPOSVersion1_2::subset`
/// (hb-ot-layout-gsubgpos.hh#L4670-L4722). `Ok(true)` always when the table could be read.
pub(crate) fn subset(plan: &Plan<'_>, s: &mut Serializer, data: View<'_>, kind: Kind) -> Res<bool> {
    let Some(table) = Gsubgpos::new(data.d) else {
        // A major version other than 1: version 2 is not ported; others are not subset.
        if data.u16(0) == 2 {
            return unsupported("GSUB/GPOS version 2");
        }
        return Ok(false);
    };
    if table.has_feature_variations() {
        return unsupported("FeatureVariations");
    }
    let mut c = SubsetLayoutCtx {
        plan,
        kind,
        script_count: 0,
        langsys_count: 0,
        feature_index_count: 0,
        lookup_index_count: 0,
        cur_script_index: 0xFFFF,
    };
    let out = s.allocate(10); // extend_min
    // out->version = version (a 1.1 table without feature variations becomes 1.0 below)
    let version = if table.version() >= 0x0001_0001 {
        0x0001_0000
    } else {
        table.version()
    };
    s.set_u32(out, version);

    // lookupList
    if table.v.is_null16(8) {
        s.zero_field(out + 8, 2);
    } else {
        let ll = table.lookup_list();
        s.serialize_subset(out + 8, 2, true, |s| lookup_list_subset(&mut c, s, ll));
    }
    // featureList
    if table.v.is_null16(6) {
        s.zero_field(out + 6, 2);
    } else {
        let fl = table.feature_list();
        s.serialize_subset(out + 6, 2, true, |s| {
            feature_list_subset(&mut c, s, table, fl)
        });
    }
    // scriptList
    if table.v.is_null16(4) {
        s.zero_field(out + 4, 2);
    } else {
        let sl = table.script_list();
        s.serialize_subset(out + 4, 2, true, |s| {
            script_list_subset(&mut c, s, table, sl)
        });
    }
    Ok(true)
}

/// `LookupOffsetList::subset` (hb-ot-layout-common.hh#L1432-L1448).
fn lookup_list_subset(c: &mut SubsetLayoutCtx<'_, '_>, s: &mut Serializer, this: View<'_>) -> bool {
    let out = s.allocate(2); // extend_min
    let count = this.u16(0);
    for index in 0..count {
        if !c.table_plan().lookups.contains_key(&index) {
            continue;
        }
        let snap = s.snapshot();
        let o = s.array_append(out, 2);
        let off_field = 2 + 2 * index as usize;
        let ret = if this.is_null16(off_field) {
            s.zero_field(o, 2);
            false
        } else {
            let lookup = this.off16(off_field);
            let kind = c.kind;
            let plan = c.plan;
            s.serialize_subset(o, 2, true, |s| lookup_subset(plan, kind, s, lookup))
        };
        if !ret {
            s.array_pop(out);
            s.revert(snap);
        }
    }
    true
}

/// `Lookup::subset<TSubTable>` (hb-ot-layout-common.hh#L1330-L1397).
fn lookup_subset(plan: &Plan<'_>, kind: Kind, s: &mut Serializer, this: View<'_>) -> bool {
    let out = s.allocate(6); // extend_min
    let lookup_type_v = lookup_type(this);
    s.set_u16(out, lookup_type_v as u16);
    s.set_u16(out + 2, lookup_flag(this) as u16);

    for i in 0..lookup_subtable_count(this) {
        if !subtable_intersects(
            kind,
            lookup_type_v,
            lookup_subtable(this, i),
            &plan.glyphset_gsub,
        ) {
            continue;
        }
        // `subset_offset_array (c, out->get_subtables<TSubTable> (), this, lookup_type)`
        let snap = s.snapshot();
        let o = s.array_append(out + 4, 2);
        let off_field = 6 + 2 * i as usize;
        let ret = if this.is_null16(off_field) {
            s.zero_field(o, 2);
            false
        } else {
            let sub = this.off16(off_field);
            s.serialize_subset(o, 2, true, |s| {
                subtable_subset(plan, kind, s, lookup_type_v, sub)
            })
        };
        if !ret {
            s.array_pop(out + 4);
            s.revert(snap);
        }
    }

    if lookup_flag(this) & USE_MARK_FILTERING_SET != 0 {
        let mark_filtering_set = this.u16(6 + 2 * lookup_subtable_count(this) as usize);
        match plan.used_mark_sets_map.get(&mark_filtering_set) {
            None => {
                let mut new_flag = lookup_flag(this);
                new_flag &= !USE_MARK_FILTERING_SET;
                // https://github.com/harfbuzz/harfbuzz/issues/5499
                new_flag |= IGNORE_MARKS;
                s.set_u16(out + 2, new_flag as u16);
            }
            Some(&idx) => {
                // `c->serializer->extend (out)`: room for the mark filtering set.
                s.embed_u16(idx as u16);
            }
        }
    }
    // Always keep the lookup even if it's empty.
    true
}

/// `SubTable::dispatch (subset context, lookup_type)`.
fn subtable_subset(
    plan: &Plan<'_>,
    kind: Kind,
    s: &mut Serializer,
    lookup_type_v: u32,
    sub: View<'_>,
) -> bool {
    if lookup_type_v == extension_type(kind) {
        // `Extension::dispatch (hb_subset_context_t)`: `ExtensionFormat1::subset`
        if sub.u16(0) != 1 {
            return true;
        }
        let out = s.allocate(8);
        s.set_u16(out, 1);
        let inner_type = sub.u16(2);
        s.set_u16(out + 2, inner_type as u16);
        if sub.is_null32(4) {
            s.zero_field(out + 4, 4);
            return false;
        }
        let inner = sub.off32(4);
        let ret = {
            s.zero_field(out + 4, 4);
            s.push();
            let r = subtable_subset(plan, kind, s, inner_type, inner);
            let idx = s.pop_pack(true);
            s.add_link(out + 4, 4, idx, Whence::Head, 0);
            r
        };
        return ret;
    }
    match kind {
        Kind::Gsub => gsub::subset(plan, s, lookup_type_v, sub),
        Kind::Gpos => gpos::subset(plan, s, lookup_type_v, sub),
    }
}

/// `RecordListOfFeature::subset` (hb-ot-layout-common.hh#L944-L968) with `Record<Feature>::subset`
/// and `Feature::subset`.
fn feature_list_subset(
    c: &mut SubsetLayoutCtx<'_, '_>,
    s: &mut Serializer,
    table: Gsubgpos<'_>,
    this: View<'_>,
) -> bool {
    let out = s.allocate(2); // extend_min
    for index in 0..this.u16(0) {
        if !c.table_plan().features.contains_key(&index) {
            continue;
        }
        let snap = s.snapshot();
        // `Record::subset`: `embed (this)` then `offset.serialize_subset (..., c, &tag)`
        let rec = 2 + 6 * index as usize;
        let tag_v = this.u32(rec);
        let rec_pos = s.embed(&this.d[rec.min(this.d.len())..(rec + 6).min(this.d.len())]);
        let ret = if this.is_null16(rec + 4) {
            s.zero_field(rec_pos + 4, 2);
            false
        } else {
            let f = this.off16(rec + 4);
            s.serialize_subset(rec_pos + 4, 2, true, |s| {
                feature_subset(c, s, table, f, tag_v)
            })
        };
        if !ret {
            s.revert(snap);
        } else {
            let len = View::new(s.bytes()).u16(0);
            s.set_u16(out, (len + 1) as u16);
        }
    }
    true
}

/// `Feature::subset` (hb-ot-layout-common.hh#L795-L822).
fn feature_subset(
    c: &mut SubsetLayoutCtx<'_, '_>,
    s: &mut Serializer,
    _table: Gsubgpos<'_>,
    this: View<'_>,
    tag_v: u32,
) -> bool {
    let out = s.allocate(4); // extend_min
    // out->featureParams.serialize_subset (c, featureParams, this, tag)
    if this.is_null16(0) {
        s.zero_field(out, 2);
    } else {
        let params = this.off16(0);
        s.serialize_subset(out, 2, true, |s| feature_params_subset(s, params, tag_v));
    }
    let lookups = &c.table_plan().lookups;
    let it: Vec<u32> = (0..this.u16(2) as usize)
        .map(|i| this.u16(4 + 2 * i))
        .filter_map(|l| lookups.get(&l).copied())
        .collect();
    // `IndexArray::serialize`
    if !it.is_empty() {
        for v in it {
            if !c.visit_lookup_index() {
                break;
            }
            s.embed_u16(v as u16);
            let len = View::new(s.bytes()).u16(2);
            s.set_u16(out + 2, (len + 1) as u16);
        }
    }
    // The decision to keep or drop this feature is already made before we get here.
    true
}

/// `FeatureParams::subset (c, tag)` (hb-ot-layout-common.hh#L725-L735).
fn feature_params_subset(s: &mut Serializer, params: View<'_>, tag_v: u32) -> bool {
    let embed = |s: &mut Serializer, size: usize| {
        let mut b = params.d[..size.min(params.d.len())].to_vec();
        b.resize(size, 0);
        s.embed(&b);
        true
    };
    if tag_v == TAG_SIZE {
        return embed(s, 10);
    }
    if tag_v & 0xFFFF_0000 == tag(b"ss\0\0") {
        return embed(s, 4);
    }
    if tag_v & 0xFFFF_0000 == tag(b"cv\0\0") {
        // `FeatureParamsCharacterVariants::get_size`
        return embed(s, 14 + 3 * params.u16(12) as usize);
    }
    false
}

/// `RecordListOfScript::subset` (hb-ot-layout-common.hh#L1210-L1230) with `Record<Script>::subset`.
fn script_list_subset(
    c: &mut SubsetLayoutCtx<'_, '_>,
    s: &mut Serializer,
    _table: Gsubgpos<'_>,
    this: View<'_>,
) -> bool {
    let out = s.allocate(2);
    for index in 0..this.u16(0) {
        let snap = s.snapshot();
        c.cur_script_index = index;
        let rec = 2 + 6 * index as usize;
        let tag_v = this.u32(rec);
        let rec_pos = s.embed(&this.d[rec.min(this.d.len())..(rec + 6).min(this.d.len())]);
        let ret = if this.is_null16(rec + 4) {
            s.zero_field(rec_pos + 4, 2);
            false
        } else {
            let script = this.off16(rec + 4);
            s.serialize_subset(rec_pos + 4, 2, true, |s| script_subset(c, s, script, tag_v))
        };
        if !ret {
            s.revert(snap);
        } else {
            let len = View::new(s.bytes()).u16(0);
            s.set_u16(out, (len + 1) as u16);
        }
    }
    true
}

/// `Script::subset` (hb-ot-layout-common.hh#L1148-L1198).
fn script_subset(
    c: &mut SubsetLayoutCtx<'_, '_>,
    s: &mut Serializer,
    this: View<'_>,
    tag_v: u32,
) -> bool {
    if !c.visit_script() {
        return false;
    }
    // `c->plan->layout_scripts.has (*tag)`: every script is retained.
    let out = s.allocate(4); // extend_min

    let mut default_lang = false;
    if this.u16(0) != 0 {
        s.push();
        let ls = this.off16(0);
        let ret = langsys_subset(c, s, ls);
        if !ret && tag_v != TAG_DFLT {
            s.pop_discard();
            s.zero_field(out, 2);
        } else {
            let idx = s.pop_pack(true);
            s.add_link(out, 2, idx, Whence::Head, 0);
            default_lang = true;
        }
    }

    let active_langsys = c.table_plan().langsys.get(&c.cur_script_index).cloned();
    if let Some(active) = active_langsys {
        for li in 0..this.u16(2) {
            if !active.contains(&li) {
                continue;
            }
            if !c.visit_lang_sys() {
                continue;
            }
            // `subset_record_array (l, &(out->langSys), this)`
            let snap = s.snapshot();
            let rec = 4 + 6 * li as usize;
            let rec_pos = s.embed(&this.d[rec.min(this.d.len())..(rec + 6).min(this.d.len())]);
            let ret = if this.is_null16(rec + 4) {
                s.zero_field(rec_pos + 4, 2);
                false
            } else {
                let l = this.off16(rec + 4);
                s.serialize_subset(rec_pos + 4, 2, true, |s| langsys_subset(c, s, l))
            };
            if !ret {
                s.revert(snap);
            } else {
                let len = View::new(s.bytes()).u16(2);
                s.set_u16(out + 2, (len + 1) as u16);
            }
        }
    }

    View::new(s.bytes()).u16(2) != 0 || default_lang || c.kind == Kind::Gsub
}

/// `LangSys::subset` (hb-ot-layout-common.hh#L1033-L1066).
fn langsys_subset(c: &mut SubsetLayoutCtx<'_, '_>, s: &mut Serializer, this: View<'_>) -> bool {
    let out = s.allocate(6); // extend_min
    let w = &c.table_plan().features_w_duplicates;
    let req = this.u16(2);
    let v = w.get(&req).copied().unwrap_or(0xFFFF);
    s.set_u16(out + 2, v as u16);

    let feature_count = this.u16(4);
    if !c.visit_feature_index(feature_count) {
        return false;
    }
    let w = &c.table_plan().features_w_duplicates;
    let it: Vec<u32> = (0..feature_count as usize)
        .map(|i| this.u16(6 + 2 * i))
        .filter_map(|f| w.get(&f).copied())
        .collect();
    let ret = !it.is_empty();
    // `IndexArray::serialize`
    for v in it {
        if !c.visit_lookup_index() {
            break;
        }
        s.embed_u16(v as u16);
        let len = View::new(s.bytes()).u16(4);
        s.set_u16(out + 4, (len + 1) as u16);
    }
    ret
}

/// Silence the unused-import warning for items used only on some paths.
#[allow(dead_code)]
fn _coverage_marker(_: Coverage<'_>) {}

#[allow(dead_code)]
fn _error_marker() -> SubsetError {
    SubsetError::Failed
}
