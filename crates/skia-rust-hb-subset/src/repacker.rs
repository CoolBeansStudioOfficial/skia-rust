// Copyright © 2022  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-repacker.hh (harfbuzz 9cb1fee5)

//! `hb-repacker`: resolves offset overflows by re-ordering, duplicating, isolating and (for
//! `GSUB`/`GPOS`) promoting and splitting the packed objects of a table.

use std::collections::BTreeSet;

use crate::bytes::tag;
use crate::graph::{Graph, OverflowRecord, PackedObject};
use crate::graph_gsubgpos::{Ctx, make_extension, split_subtables_if_needed};

const GPOS: u32 = tag(b"GPOS");
const GSUB: u32 = tag(b"GSUB");

/// `_presplit_subtables_if_needed` (hb-repacker.hh#L83-L102).
fn presplit_subtables_if_needed(c: &mut Ctx, g: &mut Graph) -> bool {
    let lookup_indices: Vec<u32> = c.lookups.iter().copied().collect();
    for lookup_index in lookup_indices {
        if !split_subtables_if_needed(c, g, lookup_index) {
            return false;
        }
    }
    true
}

struct LookupSize {
    lookup_index: u32,
    size: usize,
    num_subtables: u32,
}

/// `lookup_size_t::cmp`.
#[allow(clippy::cast_precision_loss, clippy::float_cmp)] // mirrors the C++ doubles; the comparison is exact there too
fn lookup_size_cmp(a: &LookupSize, b: &LookupSize) -> std::cmp::Ordering {
    let spb_a = f64::from(a.num_subtables) / a.size as f64;
    let spb_b = f64::from(b.num_subtables) / b.size as f64;
    let r: i32 = if spb_a == spb_b {
        // `return b->lookup_index - a->lookup_index;` (unsigned subtraction converted to int)
        b.lookup_index.wrapping_sub(a.lookup_index) as i32
    } else {
        let cmp = spb_b - spb_a;
        if cmp < 0.0 { -1 } else { i32::from(cmp > 0.0) }
    };
    r.cmp(&0)
}

/// `_promote_extensions_if_needed` (hb-repacker.hh#L108-L186).
fn promote_extensions_if_needed(c: &mut Ctx, g: &mut Graph) -> bool {
    if c.lookups.is_empty() {
        return true;
    }
    let mut total_lookup_table_sizes: u32 = 0;
    let mut lookup_sizes: Vec<LookupSize> = Vec::new();
    for &lookup_index in &c.lookups {
        total_lookup_table_sizes = total_lookup_table_sizes
            .wrapping_add(g.vertices[lookup_index as usize].table_size() as u32);
        let mut visited = BTreeSet::new();
        lookup_sizes.push(LookupSize {
            lookup_index,
            size: g.find_subgraph_size(lookup_index, &mut visited, u32::MAX),
            num_subtables: g.u16_at(lookup_index, 4),
        });
    }
    // `qsort`: the comparison is a total order (ties are broken by the lookup index).
    lookup_sizes.sort_by(lookup_size_cmp);

    let lookup_list_size = g.vertices[c.lookup_list_index as usize].table_size();
    let l2_l3_size = lookup_list_size + total_lookup_table_sizes as usize;
    let mut l3_l4_size = total_lookup_table_sizes as usize;
    let mut l4_plus_size = 0usize;

    for p in &lookup_sizes {
        let subtables_size = (p.num_subtables * 8) as usize;
        l3_l4_size += subtables_size;
        l4_plus_size += subtables_size;
    }

    let mut layers_full = false;
    for p in &lookup_sizes {
        if crate::graph_gsubgpos::is_extension_lookup(c, g, p.lookup_index) {
            continue;
        }
        if !layers_full {
            let lookup_size = g.vertices[p.lookup_index as usize].table_size();
            let mut visited = BTreeSet::new();
            let subtables_size = g
                .find_subgraph_size(p.lookup_index, &mut visited, 1)
                .wrapping_sub(lookup_size);
            let remaining_size = p
                .size
                .wrapping_sub(subtables_size)
                .wrapping_sub(lookup_size);

            l3_l4_size = l3_l4_size.wrapping_add(subtables_size);
            l3_l4_size = l3_l4_size.wrapping_sub((p.num_subtables * 8) as usize);
            l4_plus_size = l4_plus_size
                .wrapping_add(subtables_size)
                .wrapping_add(remaining_size);

            if l2_l3_size < (1 << 16) && l3_l4_size < (1 << 16) && l4_plus_size < (1 << 16) {
                continue;
            }
            layers_full = true;
        }
        if !make_extension(c, g, p.lookup_index) {
            return false;
        }
    }
    true
}

/// `_try_isolating_subgraphs` (hb-repacker.hh#L188-L248).
fn try_isolating_subgraphs(overflows: &[OverflowRecord], g: &mut Graph) -> bool {
    let mut space = 0u32;
    let mut roots_to_isolate: BTreeSet<u32> = BTreeSet::new();

    for r in overflows.iter().rev() {
        let (overflow_space, root) = g.space_for(r.parent);
        if overflow_space == 0 {
            continue;
        }
        if g.num_roots_for_space(overflow_space) <= 1 {
            continue;
        }
        if space == 0 {
            space = overflow_space;
        }
        if space == overflow_space {
            roots_to_isolate.insert(root);
        }
    }

    if roots_to_isolate.is_empty() {
        return false;
    }

    let maximum_to_move = (g.num_roots_for_space(space) / 2).max(1);
    if roots_to_isolate.len() as u32 > maximum_to_move {
        let mut extra = roots_to_isolate.len() as u32 - maximum_to_move;
        let ordering = g.ordering.clone();
        for id in ordering {
            if extra == 0 {
                break;
            }
            if roots_to_isolate.remove(&id) {
                extra -= 1;
            }
        }
    }

    g.isolate_subgraph(&mut roots_to_isolate);
    g.move_to_new_space(&roots_to_isolate);
    true
}

/// `_resolve_shared_overflow` (hb-repacker.hh#L250-L305).
fn resolve_shared_overflow(
    overflows: &[OverflowRecord],
    overflow_index: usize,
    g: &mut Graph,
) -> bool {
    let r = overflows[overflow_index];
    let mut parents: BTreeSet<u32> = BTreeSet::new();
    parents.insert(r.parent);
    for r2 in overflows[..overflow_index].iter().rev() {
        if r2.child == r.child {
            parents.insert(r2.parent);
        }
    }

    let mut result = g.duplicate_for_parents(&parents, r.child);
    if result == crate::graph::NONE && parents.len() > 2 {
        let min = *parents.iter().next().expect("non-empty");
        parents.remove(&min);
        result = g.duplicate_for_parents(&parents, r.child);
    }
    if result == crate::graph::NONE {
        return false;
    }
    if parents.len() > 1 {
        g.vertices[result as usize].give_max_priority();
    }
    true
}

/// `_process_overflows` (hb-repacker.hh#L307-L357).
fn process_overflows(
    overflows: &[OverflowRecord],
    priority_bumped_parents: &mut BTreeSet<u32>,
    g: &mut Graph,
) -> bool {
    let mut resolution_attempted = false;
    for i in (0..overflows.len()).rev() {
        let r = overflows[i];
        if g.vertices[r.child as usize].is_shared() && resolve_shared_overflow(overflows, i, g) {
            return true;
        }
        if g.vertices[r.child as usize].is_leaf()
            && !priority_bumped_parents.contains(&r.parent)
            && g.raise_childrens_priority(r.parent)
        {
            priority_bumped_parents.insert(r.parent);
            resolution_attempted = true;
        }
    }
    resolution_attempted
}

/// `hb_resolve_graph_overflows` (hb-repacker.hh#L359-L451).
fn resolve_graph_overflows(
    table_tag: u32,
    max_rounds: u32,
    always_recalculate_extensions: bool,
    g: &mut Graph,
) -> bool {
    g.sort_shortest_distance();
    if g.in_error() {
        return false;
    }

    let will_overflow = g.will_overflow(None);
    if !will_overflow {
        return true;
    }

    let is_gsub_or_gpos = table_tag == GPOS || table_tag == GSUB;
    let mut ext_context = Ctx::new(table_tag, g);
    if is_gsub_or_gpos && will_overflow {
        if always_recalculate_extensions {
            if !presplit_subtables_if_needed(&mut ext_context, g) {
                return false;
            }
            if !promote_extensions_if_needed(&mut ext_context, g) {
                return false;
            }
        }
        if g.assign_spaces() {
            g.sort_shortest_distance();
        } else {
            g.sort_shortest_distance_if_needed();
        }
    }

    let mut round = 0u32;
    let mut overflows: Vec<OverflowRecord> = Vec::new();
    while !g.in_error() && g.will_overflow(Some(&mut overflows)) && round < max_rounds {
        let mut priority_bumped_parents = BTreeSet::new();
        if !try_isolating_subgraphs(&overflows, g) {
            round += 1;
            if !process_overflows(&overflows, &mut priority_bumped_parents, g) {
                break;
            }
        }
        g.sort_shortest_distance();
    }

    if g.in_error() {
        return false;
    }

    if g.will_overflow(None) {
        if is_gsub_or_gpos && !always_recalculate_extensions {
            return resolve_graph_overflows(table_tag, max_rounds, true, g);
        }
        return false;
    }
    true
}

/// `hb_resolve_overflows` (hb-repacker.hh#L453-L482) with its default `max_rounds` of 32 and
/// `recalculate_extensions` of false.
pub(crate) fn resolve_overflows(
    packed: Vec<Option<PackedObject>>,
    table_tag: u32,
) -> Option<Vec<u8>> {
    let mut g = Graph::new(packed);
    if g.in_error() {
        return None;
    }
    if !g.is_fully_connected() {
        return None;
    }
    if g.in_error() {
        return None;
    }
    if !resolve_graph_overflows(table_tag, 32, false, &mut g) {
        return None;
    }
    g.serialize()
}
