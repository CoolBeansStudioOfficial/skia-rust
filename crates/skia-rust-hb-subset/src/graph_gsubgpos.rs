// Copyright © 2022  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/graph/{gsubgpos-graph,gsubgpos-context,coverage-graph,classdef-graph,
// split-helpers,pairpos-graph,markbasepos-graph,ligature-graph}.hh, src/graph/gsubgpos-context.cc
// (harfbuzz 9cb1fee5)

//! The `GSUB`/`GPOS` specializations of the repacker: the lookups and their subtables as graph
//! vertices, promotion of lookups to extensions, and the splitting of the subtables that do not
//! fit in 64 KiB (pair positioning, mark to base positioning and ligature substitution).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::bytes::tag;
use crate::graph::{GLink, Graph, NONE};
use crate::hbmap::HbMap;
use crate::ot::{ClassDef, Coverage, View, classdef_serialize, coverage_serialize};
use crate::serialize::Serializer;

const GPOS: u32 = tag(b"GPOS");
const GSUB: u32 = tag(b"GSUB");

/// `gsubgpos_graph_context_t`.
pub(crate) struct Ctx {
    pub table_tag: u32,
    pub lookup_list_index: u32,
    /// The keys of `lookups` (the values are the lookup tables, read from the vertices).
    pub lookups: BTreeSet<u32>,
    subtable_to_extension: HbMap,
    split_subtables: HashMap<u32, Vec<u32>>,
}

impl Ctx {
    /// The constructor (gsubgpos-context.cc#L5-L20).
    pub(crate) fn new(table_tag: u32, g: &Graph) -> Ctx {
        let mut c = Ctx {
            table_tag,
            lookup_list_index: 0,
            lookups: BTreeSet::new(),
            subtable_to_extension: HbMap::new(),
            split_subtables: HashMap::new(),
        };
        if table_tag != GPOS && table_tag != GSUB {
            return c;
        }
        // `GSTAR::graph_to_gstar`
        let root = g.root_idx();
        let len = g.bytes(root).len();
        let version_ok = len >= 10
            && len
                >= 10
                    + if g.u16_at(root, 0) == 1 && g.u16_at(root, 2) >= 1 {
                        4
                    } else {
                        0
                    };
        if !version_ok {
            return c;
        }
        // `find_lookups` (major version 1)
        if g.u16_at(root, 0) != 1 {
            return c;
        }
        let lookup_list_idx = g.index_for_offset(root, 8);
        c.lookup_list_index = lookup_list_idx;
        if lookup_list_idx == NONE {
            return c;
        }
        let list_len = g.bytes(lookup_list_idx).len();
        let count = g.u16_at(lookup_list_idx, 0) as usize;
        if list_len < 2 || list_len < 2 * count {
            return c;
        }
        for i in 0..count {
            let lookup_idx = g.index_for_offset(lookup_list_idx, 2 + 2 * i);
            if lookup_idx == NONE {
                continue;
            }
            if !lookup_sanitize(g, lookup_idx) {
                continue;
            }
            c.lookups.insert(lookup_idx);
        }
        c
    }
}

/// `create_node (size)`.
fn create_node(g: &mut Graph, size: usize) -> u32 {
    let obj = g.add_buffer(vec![0; size]);
    g.new_node(obj)
}

// -------------------------------------------------------------------------------------------
// Lookups
// -------------------------------------------------------------------------------------------

fn extension_type(table_tag: u32) -> u32 {
    match table_tag {
        GPOS => 9,
        GSUB => 7,
        _ => 0,
    }
}

/// `Lookup::sanitize`.
fn lookup_sanitize(g: &Graph, idx: u32) -> bool {
    let len = g.bytes(idx).len();
    if len < 6 {
        return false;
    }
    let mark = if g.u16_at(idx, 2) & 0x0010 != 0 { 2 } else { 0 };
    len >= 6 + 2 * g.u16_at(idx, 4) as usize + mark
}

pub(crate) fn is_extension_lookup(c: &Ctx, g: &Graph, idx: u32) -> bool {
    is_extension(c, g, idx)
}

fn is_extension(c: &Ctx, g: &Graph, idx: u32) -> bool {
    g.u16_at(idx, 0) == extension_type(c.table_tag)
}

fn number_of_subtables(g: &Graph, idx: u32) -> u32 {
    g.u16_at(idx, 4)
}

fn is_supported_gsub_type(c: &Ctx, ty: u32) -> bool {
    c.table_tag == GSUB && ty == 4
}

fn is_supported_gpos_type(c: &Ctx, ty: u32) -> bool {
    c.table_tag == GPOS && (ty == 2 || ty == 4)
}

/// `Lookup::make_extension` (gsubgpos-graph.hh#L108-L134).
pub(crate) fn make_extension(c: &mut Ctx, g: &mut Graph, this_index: u32) -> bool {
    let ext_type = extension_type(c.table_tag);
    if ext_type == 0 || is_extension(c, g, this_index) {
        return true;
    }
    for i in 0..number_of_subtables(g, this_index) as usize {
        let subtable_index = g.index_for_offset(this_index, 6 + 2 * i);
        if !make_subtable_extension(c, g, this_index, subtable_index) {
            return false;
        }
    }
    g.set_u16_at(this_index, 0, ext_type);
    true
}

/// `Lookup::create_extension_subtable`.
fn create_extension_subtable(g: &mut Graph, subtable_index: u32, ty: u32) -> u32 {
    let ext_index = create_node(g, 8);
    if ext_index == NONE {
        return NONE;
    }
    // `extension->reset (type)`
    g.set_u16_at(ext_index, 0, 1);
    g.set_u16_at(ext_index, 2, ty);
    g.vertices[ext_index as usize]
        .real_links
        .push(GLink::new(4, subtable_index, 4));
    ext_index
}

/// `Lookup::make_subtable_extension`.
fn make_subtable_extension(
    c: &mut Ctx,
    g: &mut Graph,
    lookup_index: u32,
    subtable_index: u32,
) -> bool {
    let ty = g.u16_at(lookup_index, 0);
    let existing = c.subtable_to_extension.has(subtable_index);
    let ext_index = if let Some(e) = existing {
        e
    } else {
        let e = create_extension_subtable(g, subtable_index, ty);
        c.subtable_to_extension.set(subtable_index, e);
        e
    };
    if ext_index == NONE {
        return false;
    }
    for i in 0..g.vertices[lookup_index as usize].real_links.len() {
        if g.vertices[lookup_index as usize].real_links[i].objidx == subtable_index {
            g.vertices[lookup_index as usize].real_links[i].objidx = ext_index;
            if existing.is_some() {
                g.vertices[subtable_index as usize].remove_parent(lookup_index);
            }
        }
    }
    g.vertices[ext_index as usize].add_parent(lookup_index, false);
    if existing.is_none() {
        g.vertices[subtable_index as usize].remap_parent(lookup_index, ext_index);
    }
    true
}

/// `Lookup::split_subtables_if_needed` (gsubgpos-graph.hh#L136-L225).
pub(crate) fn split_subtables_if_needed(c: &mut Ctx, g: &mut Graph, this_index: u32) -> bool {
    let mut ty = g.u16_at(this_index, 0);
    let is_ext = is_extension(c, g, this_index);
    if c.table_tag != GPOS && c.table_tag != GSUB {
        return true;
    }
    if !is_ext && !is_supported_gpos_type(c, ty) && !is_supported_gsub_type(c, ty) {
        return true;
    }
    let mut all_new_subtables: Vec<(u32, Vec<u32>)> = Vec::new();
    for i in 0..number_of_subtables(g, this_index) {
        let mut subtable_index = g.index_for_offset(this_index, 6 + 2 * i as usize);
        if is_ext {
            let ext_subtable_index = subtable_index;
            // `ExtensionFormat1::sanitize`: the vertex holds the 8 bytes.
            if ext_subtable_index == NONE || g.bytes(ext_subtable_index).len() < 8 {
                continue;
            }
            subtable_index = g.index_for_offset(ext_subtable_index, 4);
            ty = g.u16_at(ext_subtable_index, 2);
            if !is_supported_gpos_type(c, ty) && !is_supported_gsub_type(c, ty) {
                continue;
            }
        }
        if let Some(split_result) = c.split_subtables.get(&subtable_index) {
            if split_result.is_empty() {
                continue;
            }
            all_new_subtables.push((i, split_result.clone()));
        } else {
            let new_sub_tables = if subtable_index == NONE {
                Some(Vec::new())
            } else if c.table_tag == GPOS {
                match ty {
                    2 => pair_pos_split(c, g, subtable_index),
                    4 => mark_base_pos_split(c, g, subtable_index),
                    _ => Some(Vec::new()),
                }
            } else if c.table_tag == GSUB {
                match ty {
                    4 => ligature_subst_split(c, g, subtable_index),
                    _ => Some(Vec::new()),
                }
            } else {
                Some(Vec::new())
            };
            let Some(new_sub_tables) = new_sub_tables else {
                return false;
            };
            c.split_subtables
                .insert(subtable_index, new_sub_tables.clone());
            if !new_sub_tables.is_empty() {
                all_new_subtables.push((i, new_sub_tables));
            }
        }
    }
    if !all_new_subtables.is_empty() {
        return add_sub_tables(c, g, this_index, ty, &all_new_subtables);
    }
    true
}

/// `Lookup::add_sub_tables`.
fn add_sub_tables(
    c: &mut Ctx,
    g: &mut Graph,
    this_index: u32,
    ty: u32,
    subtable_ids: &[(u32, Vec<u32>)],
) -> bool {
    let is_ext = is_extension(c, g, this_index);
    fix_existing_subtable_links(g, this_index, subtable_ids);
    let new_subtable_count: u32 = subtable_ids.iter().map(|p| p.1.len() as u32).sum();
    let old = g.bytes(this_index).to_vec();
    let new_size = old.len() + new_subtable_count as usize * 2;
    let mut buffer = vec![0u8; new_size];
    buffer[..old.len()].copy_from_slice(&old);
    if g.u16_at(this_index, 2) & 0x0010 != 0 {
        buffer[new_size - 2..].copy_from_slice(&old[old.len() - 2..]);
    }
    let old_sub_len = g.u16_at(this_index, 4);
    g.vertices[this_index as usize].obj = g.add_buffer(buffer);
    g.set_u16_at(this_index, 4, old_sub_len + new_subtable_count);
    let mut shift = 0u32;
    for (first, ids) in subtable_ids {
        let offset_index_start = first + shift + 1;
        shift += ids.len() as u32;
        for (offset_index, &subtable_id) in (offset_index_start..).zip(ids.iter()) {
            let mut subtable_id = subtable_id;
            if is_ext {
                let ext_id = create_extension_subtable(g, subtable_id, ty);
                g.vertices[subtable_id as usize].add_parent(ext_id, false);
                subtable_id = ext_id;
            }
            let position = 6 + 2 * offset_index;
            g.vertices[this_index as usize]
                .real_links
                .push(GLink::new(2, subtable_id, position));
            g.vertices[subtable_id as usize].add_parent(this_index, false);
        }
    }
    g.vertices[this_index as usize].real_links.sort_by(link_cmp);
    c.lookups.insert(this_index);
    true
}

/// `link_t::cmp`.
fn link_cmp(a: &GLink, b: &GLink) -> std::cmp::Ordering {
    (a.position as i32)
        .wrapping_sub(b.position as i32)
        .cmp(&0)
        .then((a.objidx as i32).wrapping_sub(b.objidx as i32).cmp(&0))
}

/// `Lookup::fix_existing_subtable_links`.
fn fix_existing_subtable_links(g: &mut Graph, this_index: u32, subtable_ids: &[(u32, Vec<u32>)]) {
    let mut shift = 0u32;
    for (first, ids) in subtable_ids {
        let insert_index = first + shift;
        let pos_offset = ids.len() as u32 * 2;
        let insert_offset = 6 + insert_index * 2;
        shift += ids.len() as u32;
        let v = &mut g.vertices[this_index as usize];
        for l in v.real_links.iter_mut().chain(v.virtual_links.iter_mut()) {
            if l.position > insert_offset {
                l.position += pos_offset;
            }
        }
    }
}

// -------------------------------------------------------------------------------------------
// Coverage and ClassDef helpers
// -------------------------------------------------------------------------------------------

/// `graph::Coverage::sanitize`.
fn coverage_sanitize(g: &Graph, idx: u32) -> bool {
    let len = g.bytes(idx).len();
    if len < 2 {
        return false;
    }
    match g.u16_at(idx, 0) {
        1 => len >= 4 && len >= 4 + 2 * g.u16_at(idx, 2) as usize,
        2 => len >= 4 && len >= 4 + 6 * g.u16_at(idx, 2) as usize,
        _ => false,
    }
}

/// `graph::ClassDef::sanitize`.
fn class_def_sanitize(g: &Graph, idx: u32) -> bool {
    let len = g.bytes(idx).len();
    if len < 2 {
        return false;
    }
    match g.u16_at(idx, 0) {
        1 => len >= 6 && len >= 6 + 2 * g.u16_at(idx, 4) as usize,
        2 => len >= 4 && len >= 4 + 6 * g.u16_at(idx, 2) as usize,
        _ => false,
    }
}

fn coverage_glyphs(g: &Graph, idx: u32) -> Vec<u32> {
    Coverage(View::new(g.bytes(idx))).iter()
}

/// `Coverage::make_coverage`: serializes the glyphs into a buffer of `max_size` bytes (the
/// serializer runs out of room above it) and makes it the bytes of `dest_obj`.
fn make_coverage(g: &mut Graph, glyphs: &[u32], dest_obj: u32, max_size: usize) -> bool {
    let mut s = Serializer::new();
    s.start_serialize();
    coverage_serialize(&mut s, glyphs);
    if s.peak() > max_size {
        return false;
    }
    let Some(bytes) = s.end_serialize() else {
        return false;
    };
    g.vertices[dest_obj as usize].obj = g.add_buffer(bytes);
    true
}

/// `Coverage::add_coverage`.
fn add_coverage(
    g: &mut Graph,
    parent_id: u32,
    link_position: u32,
    glyphs: &[u32],
    max_size: usize,
) -> bool {
    let coverage_prime_id = g.new_node(crate::graph::Obj::default());
    if !make_coverage(g, glyphs, coverage_prime_id, max_size) {
        return false;
    }
    g.vertices[parent_id as usize]
        .real_links
        .push(GLink::new(2, coverage_prime_id, link_position));
    g.vertices[coverage_prime_id as usize].add_parent(parent_id, false);
    true
}

/// `Coverage::clone_coverage`.
fn clone_coverage(
    g: &mut Graph,
    coverage_id: u32,
    new_parent_id: u32,
    link_position: u32,
    start: u32,
    end: u32,
) -> bool {
    let coverage_size = g.bytes(coverage_id).len();
    if !coverage_sanitize(g, coverage_id) {
        return false;
    }
    let new_coverage: Vec<u32> = coverage_glyphs(g, coverage_id)
        .into_iter()
        .zip(0u32..)
        .filter(|&(_, i)| i >= start && i < end)
        .map(|p| p.0)
        .collect();
    add_coverage(
        g,
        new_parent_id,
        link_position,
        &new_coverage,
        coverage_size,
    )
}

/// `Coverage::filter_coverage`.
fn filter_coverage(g: &mut Graph, existing_coverage: u32, start: u32, end: u32) -> bool {
    let coverage_size = g.bytes(existing_coverage).len();
    if !coverage_sanitize(g, existing_coverage) {
        return false;
    }
    let new_coverage: Vec<u32> = coverage_glyphs(g, existing_coverage)
        .into_iter()
        .zip(0u32..)
        .filter(|&(_, i)| i >= start && i < end)
        .map(|p| p.0)
        .collect();
    make_coverage(g, &new_coverage, existing_coverage, coverage_size * 2 + 100)
}

/// `ClassDef::make_class_def`.
fn make_class_def(
    g: &mut Graph,
    glyph_and_class: &[(u32, u32)],
    dest_obj: u32,
    max_size: usize,
) -> bool {
    let mut s = Serializer::new();
    s.start_serialize();
    classdef_serialize(&mut s, glyph_and_class);
    if s.peak() > max_size {
        return false;
    }
    let Some(bytes) = s.end_serialize() else {
        return false;
    };
    g.vertices[dest_obj as usize].obj = g.add_buffer(bytes);
    true
}

/// `ClassDef::add_class_def`.
fn add_class_def(
    g: &mut Graph,
    parent_id: u32,
    link_position: u32,
    glyph_and_class: &[(u32, u32)],
    max_size: usize,
) -> bool {
    let class_def_prime_id = g.new_node(crate::graph::Obj::default());
    if !make_class_def(g, glyph_and_class, class_def_prime_id, max_size) {
        return false;
    }
    g.vertices[parent_id as usize].real_links.push(GLink::new(
        2,
        class_def_prime_id,
        link_position,
    ));
    g.vertices[class_def_prime_id as usize].add_parent(parent_id, false);
    true
}

/// `graph.as_mutable_table<T> (parent, &offset)` for a table with the sanitizer: the index of
/// the (possibly duplicated) child.
fn as_mutable_index(
    g: &mut Graph,
    parent: u32,
    position: usize,
    sanitize: impl Fn(&Graph, u32) -> bool,
) -> Option<u32> {
    let index = g.mutable_index_for_offset(parent, position);
    if index == NONE || index as usize >= g.vertices.len() || !sanitize(g, index) {
        return None;
    }
    Some(index)
}

/// `graph.as_table<T> (parent, &offset)`.
fn as_index(
    g: &Graph,
    parent: u32,
    position: usize,
    sanitize: impl Fn(&Graph, u32) -> bool,
) -> Option<u32> {
    let index = g.index_for_offset(parent, position);
    if index == NONE || index as usize >= g.vertices.len() || !sanitize(g, index) {
        return None;
    }
    Some(index)
}

/// `actuate_subtable_split` for a split context.
trait SplitContext {
    fn original_count(&self, g: &Graph) -> u32;
    fn clone_range(&mut self, c: &mut Ctx, g: &mut Graph, start: u32, end: u32) -> u32;
    fn shrink(&mut self, c: &mut Ctx, g: &mut Graph, count: u32) -> bool;
}

fn actuate_subtable_split(
    split_context: &mut impl SplitContext,
    c: &mut Ctx,
    g: &mut Graph,
    split_points: &[u32],
) -> Option<Vec<u32>> {
    let mut new_objects = Vec::new();
    if split_points.is_empty() {
        return Some(new_objects);
    }
    for i in 0..split_points.len() {
        let start = split_points[i];
        let end = if i < split_points.len() - 1 {
            split_points[i + 1]
        } else {
            split_context.original_count(g)
        };
        let id = split_context.clone_range(c, g, start, end);
        if id == NONE {
            return None;
        }
        new_objects.push(id);
    }
    if !split_context.shrink(c, g, split_points[0]) {
        return None;
    }
    Some(new_objects)
}

// -------------------------------------------------------------------------------------------
// PairPos
// -------------------------------------------------------------------------------------------

/// `Lookup::split_subtable<PairPos>`.
fn pair_pos_split(c: &mut Ctx, g: &mut Graph, idx: u32) -> Option<Vec<u32>> {
    let len = g.bytes(idx).len();
    if len < 2 {
        return Some(Vec::new());
    }
    match g.u16_at(idx, 0) {
        1 => {
            // `PairPosFormat1::sanitize`
            if len < 10 || len < 10 + 2 * g.u16_at(idx, 8) as usize {
                return Some(Vec::new());
            }
            pair_pos1_split(c, g, idx)
        }
        2 => {
            // `PairPosFormat2::sanitize`
            if len < 16 {
                return Some(Vec::new());
            }
            let record_size = pair2_class1_record_size(g, idx);
            if len < 16 + g.u16_at(idx, 12) as usize * record_size {
                return Some(Vec::new());
            }
            pair_pos2_split(c, g, idx)
        }
        _ => Some(Vec::new()),
    }
}

fn pair_pos1_split(c: &mut Ctx, g: &mut Graph, this_index: u32) -> Option<Vec<u32>> {
    let mut visited = BTreeSet::new();
    let coverage_id = g.index_for_offset(this_index, 2);
    let coverage_size = g.vertices[coverage_id as usize].table_size() as u32;
    let base_size = 10u32;
    let mut partial_coverage_size = 4u32;
    let mut accumulated = base_size;
    let mut split_points = Vec::new();
    for i in 0..g.u16_at(this_index, 8) {
        let pair_set_index = g.index_for_offset(this_index, 10 + 2 * i as usize);
        let accumulated_delta =
            g.find_subgraph_size(pair_set_index, &mut visited, u32::MAX) as u32 + 2;
        partial_coverage_size += 2;
        accumulated += accumulated_delta;
        let total = accumulated + partial_coverage_size.min(coverage_size);
        if total >= (1 << 16) {
            split_points.push(i);
            accumulated = base_size + accumulated_delta;
            partial_coverage_size = 6;
            visited.clear();
        }
    }
    let mut sc = PairPos1Split { this_index };
    actuate_subtable_split(&mut sc, c, g, &split_points)
}

struct PairPos1Split {
    this_index: u32,
}

impl SplitContext for PairPos1Split {
    fn original_count(&self, g: &Graph) -> u32 {
        g.u16_at(self.this_index, 8)
    }

    fn clone_range(&mut self, _c: &mut Ctx, g: &mut Graph, start: u32, end: u32) -> u32 {
        let this_index = self.this_index;
        let num_pair_sets = end - start;
        let prime_size = 10 + num_pair_sets as usize * 2;
        let pair_pos_prime_id = create_node(g, prime_size);
        if pair_pos_prime_id == NONE {
            return NONE;
        }
        let (format, vf0, vf1) = (
            g.u16_at(this_index, 0),
            g.u16_at(this_index, 4),
            g.u16_at(this_index, 6),
        );
        g.set_u16_at(pair_pos_prime_id, 0, format);
        g.set_u16_at(pair_pos_prime_id, 4, vf0);
        g.set_u16_at(pair_pos_prime_id, 6, vf1);
        g.set_u16_at(pair_pos_prime_id, 8, num_pair_sets);
        for i in start..end {
            g.move_child(
                this_index,
                10 + 2 * i as usize,
                pair_pos_prime_id,
                10 + 2 * (i - start) as usize,
            );
        }
        let coverage_id = g.index_for_offset(this_index, 2);
        if !clone_coverage(g, coverage_id, pair_pos_prime_id, 2, start, end) {
            return NONE;
        }
        pair_pos_prime_id
    }

    fn shrink(&mut self, _c: &mut Ctx, g: &mut Graph, count: u32) -> bool {
        let this_index = self.this_index;
        let old_count = g.u16_at(this_index, 8);
        if count >= old_count {
            return true;
        }
        g.set_u16_at(this_index, 8, count);
        g.vertices[this_index as usize].obj.len -= (old_count - count) as usize * 2;
        let Some(coverage) = as_mutable_index(g, this_index, 2, coverage_sanitize) else {
            return false;
        };
        let coverage_size = g.bytes(coverage).len();
        let new_coverage: Vec<u32> = coverage_glyphs(g, coverage)
            .into_iter()
            .zip(0u32..)
            .filter(|&(_, i)| i < count)
            .map(|p| p.0)
            .collect();
        make_coverage(g, &new_coverage, coverage, coverage_size)
    }
}

/// `ValueFormat::get_size`.
fn value_format_size(vf: u32) -> usize {
    (vf & 0xFF).count_ones() as usize * 2
}

/// `ValueFormat::get_len`.
fn value_format_len(vf: u32) -> u32 {
    (vf & 0xFF).count_ones()
}

/// `ValueFormat::get_device_table_indices`.
fn device_table_indices(vf: u32) -> Vec<u32> {
    let mut i = 0;
    let mut result = Vec::new();
    for flag in [0x1u32, 0x2, 0x4, 0x8] {
        if vf & flag != 0 {
            i += 1;
        }
    }
    for flag in [0x10u32, 0x20, 0x40, 0x80] {
        if vf & flag != 0 {
            result.push(i);
            i += 1;
        }
    }
    result
}

/// `PairPosFormat2::get_class1_record_size`.
fn pair2_class1_record_size(g: &Graph, idx: u32) -> usize {
    g.u16_at(idx, 14) as usize
        * (value_format_size(g.u16_at(idx, 4)) + value_format_size(g.u16_at(idx, 6)))
}

/// `class_def_size_estimator_t`.
struct ClassDefSizeEstimator {
    num_ranges_per_class: HashMap<u32, u32>,
    glyphs_per_class: HashMap<u32, BTreeSet<u32>>,
    included_classes: BTreeSet<u32>,
    included_glyphs: BTreeSet<u32>,
    class_def_1_size: u32,
    class_def_2_size: u32,
}

impl ClassDefSizeEstimator {
    const CLASS_DEF_FORMAT1_BASE_SIZE: u32 = 6;
    const CLASS_DEF_FORMAT2_BASE_SIZE: u32 = 4;
    const COVERAGE_BASE_SIZE: u32 = 4;
    const BYTES_PER_RANGE: u32 = 6;
    const BYTES_PER_GLYPH: u32 = 2;

    fn new(glyph_and_class: &[(u32, u32)]) -> ClassDefSizeEstimator {
        let mut e = ClassDefSizeEstimator {
            num_ranges_per_class: HashMap::new(),
            glyphs_per_class: HashMap::new(),
            included_classes: BTreeSet::new(),
            included_glyphs: BTreeSet::new(),
            class_def_1_size: 0,
            class_def_2_size: 0,
        };
        e.reset();
        for &(gid, klass) in glyph_and_class {
            e.glyphs_per_class.entry(klass).or_default().insert(gid);
        }
        for (&klass, glyphs) in &e.glyphs_per_class {
            if klass == 0 {
                continue;
            }
            e.num_ranges_per_class.insert(klass, num_ranges(glyphs));
        }
        e
    }

    fn reset(&mut self) {
        self.class_def_1_size = Self::CLASS_DEF_FORMAT1_BASE_SIZE;
        self.class_def_2_size = Self::CLASS_DEF_FORMAT2_BASE_SIZE;
        self.included_glyphs.clear();
        self.included_classes.clear();
    }

    fn coverage_size(&self) -> u32 {
        let format1_size =
            Self::COVERAGE_BASE_SIZE + Self::BYTES_PER_GLYPH * self.included_glyphs.len() as u32;
        let format2_size =
            Self::COVERAGE_BASE_SIZE + Self::BYTES_PER_RANGE * num_ranges(&self.included_glyphs);
        format1_size.min(format2_size)
    }

    fn add_class_def_size(&mut self, klass: u32) -> u32 {
        if !self.included_classes.contains(&klass) {
            if let Some(glyphs) = self.glyphs_per_class.get(&klass) {
                self.included_glyphs.extend(glyphs.iter().copied());
            }
            self.class_def_1_size = Self::CLASS_DEF_FORMAT1_BASE_SIZE;
            if let (Some(&min), Some(&max)) =
                (self.included_glyphs.first(), self.included_glyphs.last())
            {
                self.class_def_1_size += Self::BYTES_PER_GLYPH * (max - min + 1);
            }
            self.class_def_2_size +=
                Self::BYTES_PER_RANGE * self.num_ranges_per_class.get(&klass).copied().unwrap_or(0);
            self.included_classes.insert(klass);
        }
        self.class_def_1_size.min(self.class_def_2_size)
    }
}

/// The number of ranges of consecutive glyphs of the set (`next_range` loop).
fn num_ranges(glyphs: &BTreeSet<u32>) -> u32 {
    let mut count = 0;
    let mut prev: Option<u32> = None;
    for &g in glyphs {
        if prev.is_none_or(|p| p + 1 != g) {
            count += 1;
        }
        prev = Some(g);
    }
    count
}

struct PairPos2Split {
    this_index: u32,
    class1_record_size: u32,
    value_record_len: u32,
    value1_record_len: u32,
    max_coverage_size: u32,
    max_class_def_size: u32,
    device_tables: HbMap,
    format1_device_table_indices: Vec<u32>,
    format2_device_table_indices: Vec<u32>,
}

fn pair_pos2_split(c: &mut Ctx, g: &mut Graph, this_index: u32) -> Option<Vec<u32>> {
    let base_size = 16u32;
    let class_def_2_id = g.index_for_offset(this_index, 10);
    let class_def_2_size = g.vertices[class_def_2_id as usize].table_size() as u32;
    // `get_coverage`, `get_class_def_1`: the `Null` objects when they do not sanitize.
    let coverage_id = g.index_for_offset(this_index, 2);
    let coverage_glyphs_v: Vec<u32> = if coverage_id != NONE && coverage_sanitize(g, coverage_id) {
        coverage_glyphs(g, coverage_id)
    } else {
        Vec::new()
    };
    let class_def_1_id = g.index_for_offset(this_index, 8);
    let class_def_1_ok = class_def_1_id != NONE && class_def_sanitize(g, class_def_1_id);
    let get_class_1 = |g: &Graph, gid: u32| -> u32 {
        if class_def_1_ok {
            ClassDef(View::new(g.bytes(class_def_1_id))).get_class(gid)
        } else {
            0
        }
    };
    let gid_and_class: Vec<(u32, u32)> = coverage_glyphs_v
        .iter()
        .map(|&gid| (gid, get_class_1(g, gid)))
        .collect();
    let mut estimator = ClassDefSizeEstimator::new(&gid_and_class);
    let class1_count = g.u16_at(this_index, 12);
    let class2_count = g.u16_at(this_index, 14);
    let class1_record_size = pair2_class1_record_size(g, this_index) as u32;
    let vf1 = g.u16_at(this_index, 4);
    let vf2 = g.u16_at(this_index, 6);
    let value_1_len = value_format_len(vf1);
    let value_2_len = value_format_len(vf2);
    let total_value_len = value_1_len + value_2_len;
    let mut accumulated = base_size;
    let mut max_coverage_size = 4u32;
    let mut max_class_def_1_size = 4u32;
    let mut split_points = Vec::new();
    let device_tables = g.vertices[this_index as usize].position_to_index_map();
    let format1_device_table_indices = device_table_indices(vf1);
    let format2_device_table_indices = device_table_indices(vf2);
    let has_device_tables =
        !format1_device_table_indices.is_empty() || !format2_device_table_indices.is_empty();
    let mut visited = BTreeSet::new();
    for i in 0..class1_count {
        let mut accumulated_delta = class1_record_size;
        let class_def_1_size = estimator.add_class_def_size(i);
        let coverage_size = estimator.coverage_size();
        max_coverage_size = max_coverage_size.max(coverage_size);
        max_class_def_1_size = max_class_def_1_size.max(class_def_1_size);
        if has_device_tables {
            for j in 0..class2_count {
                let value1_index = total_value_len * (class2_count * i + j);
                let value2_index = value1_index + value_1_len;
                accumulated_delta += size_of_value_record_children(
                    g,
                    &device_tables,
                    &format1_device_table_indices,
                    value1_index,
                    &mut visited,
                );
                accumulated_delta += size_of_value_record_children(
                    g,
                    &device_tables,
                    &format2_device_table_indices,
                    value2_index,
                    &mut visited,
                );
            }
        }
        accumulated += accumulated_delta;
        let total = accumulated + coverage_size + class_def_1_size + class_def_2_size
            - coverage_size.max(class_def_1_size).max(class_def_2_size);
        if total >= (1 << 16) {
            split_points.push(i);
            accumulated = base_size + accumulated_delta;
            estimator.reset();
            // The sizes HarfBuzz assigns here are overwritten at the top of the next iteration.
            estimator.add_class_def_size(i);
            visited.clear();
        }
    }
    let mut sc = PairPos2Split {
        this_index,
        class1_record_size,
        value_record_len: total_value_len,
        value1_record_len: value_1_len,
        max_coverage_size,
        max_class_def_size: max_class_def_1_size,
        device_tables,
        format1_device_table_indices,
        format2_device_table_indices,
    };
    actuate_subtable_split(&mut sc, c, g, &split_points)
}

fn size_of_value_record_children(
    g: &Graph,
    device_tables: &HbMap,
    device_table_indices: &[u32],
    value_record_index: u32,
    visited: &mut BTreeSet<u32>,
) -> u32 {
    let mut size = 0;
    for &i in device_table_indices {
        let record_position = 16 + 2 * (value_record_index + i);
        let Some(obj_idx) = device_tables.has(record_position) else {
            continue;
        };
        size += g.find_subgraph_size(obj_idx, visited, u32::MAX) as u32;
    }
    size
}

impl SplitContext for PairPos2Split {
    fn original_count(&self, g: &Graph) -> u32 {
        g.u16_at(self.this_index, 12)
    }

    fn clone_range(&mut self, _c: &mut Ctx, g: &mut Graph, start: u32, end: u32) -> u32 {
        let this_index = self.this_index;
        let num_records = end - start;
        let prime_size = 16 + num_records as usize * self.class1_record_size as usize;
        let pair_pos_prime_id = create_node(g, prime_size);
        if pair_pos_prime_id == NONE {
            return NONE;
        }
        for (pos, v) in [
            (0usize, g.u16_at(this_index, 0)),
            (4, g.u16_at(this_index, 4)),
            (6, g.u16_at(this_index, 6)),
            (12, num_records),
            (14, g.u16_at(this_index, 14)),
        ] {
            g.set_u16_at(pair_pos_prime_id, pos, v);
        }
        self.clone_class1_records(g, pair_pos_prime_id, start, end);
        let coverage_id = g.index_for_offset(this_index, 2);
        let class_def_1_id = g.index_for_offset(this_index, 8);
        if coverage_id == NONE
            || !coverage_sanitize(g, coverage_id)
            || class_def_1_id == NONE
            || !class_def_sanitize(g, class_def_1_id)
        {
            return NONE;
        }
        let class_def_1 = ClassDef(View::new(g.bytes(class_def_1_id)));
        let klass_map: Vec<(u32, u32)> = coverage_glyphs(g, coverage_id)
            .into_iter()
            .map(|gid| (gid, class_def_1.get_class(gid)))
            .filter(|&(_, klass)| klass >= start && klass < end)
            .map(|(gid, klass)| (gid, klass - start))
            .collect();
        let cov: Vec<u32> = klass_map.iter().map(|p| p.0).collect();
        if !add_coverage(
            g,
            pair_pos_prime_id,
            2,
            &cov,
            self.max_coverage_size as usize,
        ) {
            return NONE;
        }
        if !add_class_def(
            g,
            pair_pos_prime_id,
            8,
            &klass_map,
            self.max_class_def_size as usize,
        ) {
            return NONE;
        }
        let class_def_2_id = g.index_for_offset(this_index, 10);
        g.vertices[pair_pos_prime_id as usize]
            .real_links
            .push(GLink::new(2, class_def_2_id, 10));
        g.vertices[class_def_2_id as usize].add_parent(pair_pos_prime_id, false);
        g.duplicate_for_parent(pair_pos_prime_id, class_def_2_id);
        pair_pos_prime_id
    }

    fn shrink(&mut self, _c: &mut Ctx, g: &mut Graph, count: u32) -> bool {
        let this_index = self.this_index;
        let old_count = g.u16_at(this_index, 12);
        if count >= old_count {
            return true;
        }
        g.set_u16_at(this_index, 12, count);
        g.vertices[this_index as usize].obj.len -=
            (old_count - count) as usize * self.class1_record_size as usize;
        let Some(coverage) = as_mutable_index(g, this_index, 2, coverage_sanitize) else {
            return false;
        };
        let Some(class_def_1) = as_mutable_index(g, this_index, 8, class_def_sanitize) else {
            return false;
        };
        let klass_map: Vec<(u32, u32)> = {
            let cd = ClassDef(View::new(g.bytes(class_def_1)));
            coverage_glyphs(g, coverage)
                .into_iter()
                .map(|gid| (gid, cd.get_class(gid)))
                .filter(|&(_, klass)| klass < count)
                .collect()
        };
        let new_coverage: Vec<u32> = klass_map.iter().map(|p| p.0).collect();
        if !make_coverage(g, &new_coverage, coverage, 4 + new_coverage.len() * 2) {
            return false;
        }
        let max_size = g.bytes(class_def_1).len();
        make_class_def(g, &klass_map, class_def_1, max_size)
    }
}

impl PairPos2Split {
    /// `PairPosFormat2::clone_class1_records`.
    fn clone_class1_records(&self, g: &mut Graph, pair_pos_prime_id: u32, start: u32, end: u32) {
        let crs = self.class1_record_size as usize;
        let num_records = (end - start) as usize;
        let src: Vec<u8> = g
            .bytes(self.this_index)
            .iter()
            .skip(16 + start as usize * crs)
            .take(num_records * crs)
            .copied()
            .collect();
        {
            let dst = g.bytes_mut(pair_pos_prime_id);
            dst[16..16 + src.len()].copy_from_slice(&src);
        }
        if self.format1_device_table_indices.is_empty()
            && self.format2_device_table_indices.is_empty()
        {
            return;
        }
        let class2_count = g.u16_at(self.this_index, 14);
        for i in start..end {
            for j in 0..class2_count {
                let value1_index = self.value_record_len * (class2_count * i + j);
                let value2_index = value1_index + self.value1_record_len;
                let new_value1_index = self.value_record_len * (class2_count * (i - start) + j);
                let new_value2_index = new_value1_index + self.value1_record_len;
                self.transfer_device_tables(
                    g,
                    pair_pos_prime_id,
                    &self.format1_device_table_indices,
                    value1_index,
                    new_value1_index,
                );
                self.transfer_device_tables(
                    g,
                    pair_pos_prime_id,
                    &self.format2_device_table_indices,
                    value2_index,
                    new_value2_index,
                );
            }
        }
    }

    fn transfer_device_tables(
        &self,
        g: &mut Graph,
        pair_pos_prime_id: u32,
        indices: &[u32],
        old_value_record_index: u32,
        new_value_record_index: u32,
    ) {
        for &i in indices {
            let record_position = 16 + 2 * (old_value_record_index + i);
            if !self.device_tables.contains(record_position) {
                continue;
            }
            g.move_child(
                self.this_index,
                record_position as usize,
                pair_pos_prime_id,
                16 + 2 * (new_value_record_index + i) as usize,
            );
        }
    }
}

// -------------------------------------------------------------------------------------------
// MarkBasePos
// -------------------------------------------------------------------------------------------

/// `AnchorMatrix::sanitize (vertex, class_count)`.
fn anchor_matrix_ok(g: &Graph, idx: u32, class_count: u32) -> bool {
    let len = g.bytes(idx).len();
    len >= 2 && len >= 2 + 2 * class_count as usize * g.u16_at(idx, 0) as usize
}

/// `MarkArray::sanitize`.
fn mark_array_ok(g: &Graph, idx: u32) -> bool {
    let len = g.bytes(idx).len();
    len >= 2 && len >= 2 + 4 * g.u16_at(idx, 0) as usize
}

#[derive(Default, Clone)]
struct ClassInfo {
    marks: BTreeSet<u32>,
    child_indices: Vec<u32>,
}

struct MarkBase1Split {
    this_index: u32,
    class_to_info: Vec<ClassInfo>,
    mark_array_links: HbMap,
}

impl MarkBase1Split {
    /// `split_context_t::marks_for`.
    fn marks_for(&self, start: u32, end: u32) -> BTreeSet<u32> {
        let mut marks = BTreeSet::new();
        for klass in start..end {
            if let Some(info) = self.class_to_info.get(klass as usize) {
                marks.extend(info.marks.iter().copied());
            }
        }
        marks
    }
}

/// `Lookup::split_subtable<MarkBasePos>`.
fn mark_base_pos_split(c: &mut Ctx, g: &mut Graph, idx: u32) -> Option<Vec<u32>> {
    let len = g.bytes(idx).len();
    if len < 2 || g.u16_at(idx, 0) != 1 || len < 12 {
        return Some(Vec::new());
    }
    mark_base1_split_subtables(c, g, idx)
}

fn mark_base1_get_class_info(g: &Graph, this_index: u32) -> Vec<ClassInfo> {
    let class_count = g.u16_at(this_index, 6) as usize;
    if class_count == 0 {
        return Vec::new();
    }
    let mut class_to_info = vec![ClassInfo::default(); class_count];
    let Some(mark_array) = as_index(g, this_index, 8, mark_array_ok) else {
        return Vec::new();
    };
    let mark_count = g.u16_at(mark_array, 0);
    let record_class = |mark: u32| -> u32 {
        if mark < mark_count {
            g.u16_at(mark_array, 2 + 4 * mark as usize)
        } else {
            0
        }
    };
    for mark in 0..mark_count {
        let klass = record_class(mark);
        if klass as usize >= class_count {
            continue;
        }
        class_to_info[klass as usize].marks.insert(mark);
    }
    for link in &g.vertices[mark_array as usize].real_links {
        let mark = (link.position.wrapping_sub(2)) / 4;
        let klass = record_class(mark);
        if klass as usize >= class_count {
            continue;
        }
        class_to_info[klass as usize]
            .child_indices
            .push(link.objidx);
    }
    let base_array_id = g.index_for_offset(this_index, 10);
    for link in &g.vertices[base_array_id as usize].real_links {
        let index = (link.position.wrapping_sub(2)) / 2;
        let klass = index % class_count as u32;
        class_to_info[klass as usize]
            .child_indices
            .push(link.objidx);
    }
    class_to_info
}

fn mark_base1_split_subtables(c: &mut Ctx, g: &mut Graph, this_index: u32) -> Option<Vec<u32>> {
    let mut visited = BTreeSet::new();
    let base_coverage_id = g.index_for_offset(this_index, 4);
    let base_size = 12 + 2 + 2 + g.vertices[base_coverage_id as usize].table_size() as u32;
    let class_to_info = mark_base1_get_class_info(g, this_index);
    let class_count = g.u16_at(this_index, 6);
    let Some(base_array) = as_index(g, this_index, 10, |g, i| {
        anchor_matrix_ok(g, i, class_count)
    }) else {
        return Some(Vec::new());
    };
    let base_count = g.u16_at(base_array, 0);
    let mut partial_coverage_size = 4u32;
    let mut accumulated = base_size;
    let mut split_points = Vec::new();
    let empty = ClassInfo::default();
    for klass in 0..class_count {
        let info = class_to_info.get(klass as usize).unwrap_or(&empty);
        partial_coverage_size += 2 * info.marks.len() as u32;
        let mut accumulated_delta = 4 * info.marks.len() as u32 + 2 * base_count;
        for &objidx in &info.child_indices {
            accumulated_delta += g.find_subgraph_size(objidx, &mut visited, u32::MAX) as u32;
        }
        accumulated += accumulated_delta;
        let total = accumulated + partial_coverage_size;
        if total >= (1 << 16) {
            split_points.push(klass);
            accumulated = base_size + accumulated_delta;
            partial_coverage_size = 4 + 2 * info.marks.len() as u32;
            visited.clear();
        }
    }
    let mark_array_id = g.index_for_offset(this_index, 8);
    let mut sc = MarkBase1Split {
        this_index,
        class_to_info,
        mark_array_links: g.vertices[mark_array_id as usize].position_to_index_map(),
    };
    actuate_subtable_split(&mut sc, c, g, &split_points)
}

impl SplitContext for MarkBase1Split {
    fn original_count(&self, g: &Graph) -> u32 {
        g.u16_at(self.this_index, 6)
    }

    fn clone_range(&mut self, _c: &mut Ctx, g: &mut Graph, start: u32, end: u32) -> u32 {
        let this_index = self.this_index;
        let prime_id = create_node(g, 12);
        if prime_id == NONE {
            return NONE;
        }
        let format = g.u16_at(this_index, 0);
        g.set_u16_at(prime_id, 0, format);
        let new_class_count = end - start;
        g.set_u16_at(prime_id, 6, new_class_count);
        let base_coverage_id = g.index_for_offset(this_index, 4);
        g.add_link(4, prime_id, base_coverage_id);
        g.duplicate_for_parent(prime_id, base_coverage_id);
        let Some(mark_coverage) = as_index(g, this_index, 2, coverage_sanitize) else {
            return 0;
        };
        let marks = self.marks_for(start, end);
        let new_coverage: Vec<u32> = coverage_glyphs(g, mark_coverage)
            .into_iter()
            .zip(0u32..)
            .filter(|(_, i)| marks.contains(i))
            .map(|p| p.0)
            .collect();
        if !add_coverage(g, prime_id, 2, &new_coverage, marks.len() * 2 + 4) {
            return NONE;
        }
        let Some(mark_array) = as_index(g, this_index, 8, mark_array_ok) else {
            return NONE;
        };
        let new_mark_array = mark_array_clone(g, mark_array, &self.mark_array_links, &marks, start);
        g.add_link(8, prime_id, new_mark_array);
        let class_count = g.u16_at(this_index, 6);
        let Some(base_array) = as_index(g, this_index, 10, |g, i| {
            anchor_matrix_ok(g, i, class_count)
        }) else {
            return NONE;
        };
        let new_base_array = anchor_matrix_clone(g, base_array, start, end, class_count);
        g.add_link(10, prime_id, new_base_array);
        prime_id
    }

    fn shrink(&mut self, c: &mut Ctx, g: &mut Graph, count: u32) -> bool {
        let this_index = self.this_index;
        let old_count = g.u16_at(this_index, 6);
        if count >= old_count {
            return true;
        }
        g.set_u16_at(this_index, 6, count);
        let Some(mark_coverage) = as_mutable_index(g, this_index, 2, coverage_sanitize) else {
            return false;
        };
        let marks = self.marks_for(0, count);
        let new_coverage: Vec<u32> = coverage_glyphs(g, mark_coverage)
            .into_iter()
            .zip(0u32..)
            .filter(|(_, i)| marks.contains(i))
            .map(|p| p.0)
            .collect();
        if !make_coverage(g, &new_coverage, mark_coverage, 4 + 2 * marks.len()) {
            return false;
        }
        let Some(base_array) =
            as_mutable_index(g, this_index, 10, |g, i| anchor_matrix_ok(g, i, old_count))
        else {
            return false;
        };
        if !anchor_matrix_shrink(g, base_array, old_count, count) {
            return false;
        }
        let Some(mark_array) = as_mutable_index(g, this_index, 8, mark_array_ok) else {
            return false;
        };
        let _ = c;
        mark_array_shrink(g, &self.mark_array_links, mark_array, count)
    }
}

/// `AnchorMatrix::shrink`.
fn anchor_matrix_shrink(
    g: &mut Graph,
    this_index: u32,
    old_class_count: u32,
    new_class_count: u32,
) -> bool {
    if new_class_count >= old_class_count {
        return false;
    }
    let base_count = g.u16_at(this_index, 0) as usize;
    g.vertices[this_index as usize].obj.len = 2 + 2 * base_count * new_class_count as usize;
    for link in &mut g.vertices[this_index as usize].real_links {
        let index = (link.position.wrapping_sub(2)) / 2;
        let base = index / old_class_count;
        let klass = index % old_class_count;
        if klass >= new_class_count {
            return false;
        }
        let new_index = base * new_class_count + klass;
        link.position = 2 + 2 * new_index;
    }
    true
}

/// `AnchorMatrix::clone`.
fn anchor_matrix_clone(
    g: &mut Graph,
    this_index: u32,
    start: u32,
    end: u32,
    class_count: u32,
) -> u32 {
    let base_count = g.u16_at(this_index, 0);
    let new_class_count = end - start;
    let size = 2 + 2 * new_class_count as usize * base_count as usize;
    let prime_id = create_node(g, size);
    if prime_id == NONE {
        return NONE;
    }
    g.set_u16_at(prime_id, 0, base_count);
    let mut num_links = g.vertices[this_index as usize].real_links.len() as i64;
    let mut i: i64 = 0;
    while i < num_links {
        let link = g.vertices[this_index as usize].real_links[i as usize];
        let old_index = link.position.wrapping_sub(2) / 2;
        let klass = old_index % class_count;
        if klass < start || klass >= end {
            i += 1;
            continue;
        }
        let base = old_index / class_count;
        let new_klass = klass - start;
        let new_index = base * new_class_count + new_klass;
        let child_idx = link.objidx;
        g.add_link(2 + 2 * new_index, prime_id, child_idx);
        g.vertices[child_idx as usize].remove_parent(this_index);
        g.vertices[this_index as usize]
            .real_links
            .swap_remove(i as usize);
        num_links -= 1;
        // `i--` followed by the loop's `i++`
    }
    prime_id
}

/// `MarkArray::shrink`.
fn mark_array_shrink(
    g: &mut Graph,
    mark_array_links: &HbMap,
    this_index: u32,
    new_class_count: u32,
) -> bool {
    let links = std::mem::take(&mut g.vertices[this_index as usize].real_links);
    for link in &links {
        g.vertices[link.objidx as usize].remove_parent(this_index);
    }
    let len = g.u16_at(this_index, 0);
    let mut new_index = 0u32;
    for i in 0..len {
        let klass = g.u16_at(this_index, 2 + 4 * i as usize);
        if klass >= new_class_count {
            continue;
        }
        g.set_u16_at(this_index, 2 + 4 * new_index as usize, klass);
        let position = 2 + 4 * i + 2;
        let Some(objidx) = mark_array_links.has(position) else {
            new_index += 1;
            continue;
        };
        g.add_link(2 + 4 * new_index + 2, this_index, objidx);
        new_index += 1;
    }
    g.set_u16_at(this_index, 0, new_index);
    g.vertices[this_index as usize].obj.len = 2 + 4 * new_index as usize;
    true
}

/// `MarkArray::clone`.
fn mark_array_clone(
    g: &mut Graph,
    this_index: u32,
    pos_to_index: &HbMap,
    marks: &BTreeSet<u32>,
    start_class: u32,
) -> u32 {
    let size = 2 + 4 * marks.len();
    let obj = g.add_buffer(vec![0; size]);
    let prime_id = g.new_node(obj);
    g.set_u16_at(prime_id, 0, marks.len() as u32);
    for (i, &mark) in marks.iter().enumerate() {
        let klass = g
            .u16_at(this_index, 2 + 4 * mark as usize)
            .wrapping_sub(start_class);
        g.set_u16_at(prime_id, 2 + 4 * i, klass);
        let offset_pos = 2 + 4 * mark + 2;
        if pos_to_index.contains(offset_pos) {
            g.move_child(this_index, offset_pos as usize, prime_id, 2 + 4 * i + 2);
        }
    }
    prime_id
}

// -------------------------------------------------------------------------------------------
// LigatureSubst
// -------------------------------------------------------------------------------------------

/// `graph::LigatureSet::sanitize`.
fn ligature_set_ok(g: &Graph, idx: u32) -> bool {
    let len = g.bytes(idx).len();
    len >= 2 && len >= 2 + 2 * g.u16_at(idx, 0) as usize
}

/// `Lookup::split_subtable<LigatureSubst>`.
fn ligature_subst_split(c: &mut Ctx, g: &mut Graph, idx: u32) -> Option<Vec<u32>> {
    let len = g.bytes(idx).len();
    if len < 2 || g.u16_at(idx, 0) != 1 {
        return Some(Vec::new());
    }
    // `LigatureSubstFormat1::sanitize`
    if len < 6 || len < 6 + 2 * g.u16_at(idx, 4) as usize {
        return Some(Vec::new());
    }
    let split_points = ligature_compute_split_points(g, idx);
    let mut sc = LigaSplit {
        this_index: idx,
        original_count: ligature_total_number(g, idx),
        liga_counts: ligature_liga_counts(g, idx),
    };
    actuate_subtable_split(&mut sc, c, g, &split_points)
}

fn ligature_total_number(g: &Graph, this_index: u32) -> u32 {
    let mut total = 0;
    for i in 0..g.u16_at(this_index, 4) as usize {
        let Some(liga_set) = as_index(g, this_index, 6 + 2 * i, ligature_set_ok) else {
            return 0;
        };
        total += g.u16_at(liga_set, 0);
    }
    total
}

fn ligature_liga_counts(g: &Graph, this_index: u32) -> Vec<u32> {
    (0..g.u16_at(this_index, 4) as usize)
        .map(
            |i| match as_index(g, this_index, 6 + 2 * i, ligature_set_ok) {
                Some(liga_set) => g.u16_at(liga_set, 0),
                None => 0,
            },
        )
        .collect()
}

/// `ligature_index_to_object_id`.
fn ligature_index_to_object_id(g: &Graph, liga_set: u32) -> Vec<u32> {
    let len = g.u16_at(liga_set, 0) as usize;
    let mut map = vec![NONE; len];
    for l in &g.vertices[liga_set as usize].real_links {
        if l.position < 2 {
            continue;
        }
        let array_index = ((l.position - 2) / 2) as usize;
        if array_index < map.len() {
            map[array_index] = l.objidx;
        }
    }
    map
}

fn ligature_compute_split_points(g: &Graph, this_index: u32) -> Vec<u32> {
    let base_size = 6u32;
    let mut accumulated = base_size;
    let mut ligature_index = 0u32;
    let mut split_points = Vec::new();
    for i in 0..g.u16_at(this_index, 4) as usize {
        accumulated += 2;
        accumulated += 2;
        let Some(liga_set) = as_index(g, this_index, 6 + 2 * i, ligature_set_ok) else {
            return Vec::new();
        };
        let index_to_id = ligature_index_to_object_id(g, liga_set);
        for &liga_id in index_to_id.iter().take(g.u16_at(liga_set, 0) as usize) {
            if liga_id == NONE {
                continue;
            }
            let liga_size = g.vertices[liga_id as usize].table_size() as u32;
            accumulated += 2;
            accumulated += liga_size;
            if accumulated >= (1 << 16) {
                split_points.push(ligature_index);
                accumulated = base_size + 4 + 2 + liga_size;
            }
            ligature_index += 1;
        }
    }
    split_points
}

struct LigaSplit {
    this_index: u32,
    original_count: u32,
    liga_counts: Vec<u32>,
}

/// `clear_virtual_links`.
fn clear_virtual_links(g: &mut Graph, node_index: u32) {
    let links = std::mem::take(&mut g.vertices[node_index as usize].virtual_links);
    for l in links {
        g.vertices[l.objidx as usize].remove_parent(node_index);
    }
}

/// `add_virtual_link`.
fn add_virtual_link(g: &mut Graph, from: u32, to: u32) {
    g.vertices[to as usize].add_parent(from, true);
    g.vertices[from as usize].virtual_links.push(GLink {
        width: 0,
        is_signed: false,
        whence: crate::serialize::Whence::Head,
        bias: 0,
        position: 0,
        objidx: to,
    });
}

/// `current_liga_set_bounds`.
fn current_liga_set_bounds(g: &Graph, liga_set_index: u32) -> (u32, u32) {
    let mut min_index = u32::MAX;
    let mut max_index = 0u32;
    for l in &g.vertices[liga_set_index as usize].real_links {
        if l.position < 2 {
            continue;
        }
        let liga_index = (l.position - 2) / 2;
        min_index = min_index.min(liga_index);
        max_index = max_index.max(liga_index);
    }
    (min_index, max_index.wrapping_add(1))
}

/// `compact_liga_set`.
fn compact_liga_set(g: &mut Graph, liga_set: u32) {
    let ligature_len = g.u16_at(liga_set, 0);
    let real = g.vertices[liga_set as usize].real_links.len() as u32;
    if ligature_len <= real {
        return;
    }
    let to_remove = ligature_len - real;
    let mut new_position = 2;
    g.vertices[liga_set as usize].real_links.sort_by(link_cmp);
    for l in &mut g.vertices[liga_set as usize].real_links {
        l.position = new_position;
        new_position += 2;
    }
    g.set_u16_at(liga_set, 0, real);
    g.vertices[liga_set as usize].obj.len -= to_remove as usize * 2;
}

impl SplitContext for LigaSplit {
    fn original_count(&self, _g: &Graph) -> u32 {
        self.original_count
    }

    fn clone_range(&mut self, _c: &mut Ctx, g: &mut Graph, start: u32, end: u32) -> u32 {
        let this_index = self.this_index;
        let set_len = g.u16_at(this_index, 4);
        let prime_size = 6 + 2 * set_len as usize;
        let liga_subst_prime_id = create_node(g, prime_size);
        if liga_subst_prime_id == NONE {
            return NONE;
        }
        let format = g.u16_at(this_index, 0);
        g.set_u16_at(liga_subst_prime_id, 0, format);
        g.set_u16_at(liga_subst_prime_id, 4, set_len);
        let coverage_id = g.index_for_offset(this_index, 2);
        let coverage_prime_id = g.duplicate(coverage_id);
        g.vertices[liga_subst_prime_id as usize]
            .real_links
            .push(GLink::new(2, coverage_prime_id, 2));
        g.vertices[coverage_prime_id as usize].add_parent(liga_subst_prime_id, false);

        let mut count = 0u32;
        let mut liga_set_count = 0u32;
        let mut liga_set_start = u32::MAX;
        let mut liga_set_end = 0u32;
        for (i, &num_ligas) in self.liga_counts.clone().iter().enumerate() {
            let mut current_start = count;
            let mut current_end = count + num_ligas;
            if current_start >= end || start >= current_end {
                count += num_ligas;
                continue;
            }
            let liga_set_index = g.index_for_offset(this_index, 6 + 2 * i);
            let Some(liga_set) = as_index(g, this_index, 6 + 2 * i, ligature_set_ok) else {
                return NONE;
            };
            let liga_bounds = current_liga_set_bounds(g, liga_set_index);
            current_start = count.wrapping_add(liga_bounds.0).max(current_start);
            current_end = count.wrapping_add(liga_bounds.1).min(current_end);
            let liga_set_prime_id;
            if current_start >= start && current_end <= end {
                liga_set_end = i as u32;
                if (i as u32) < liga_set_start {
                    liga_set_start = i as u32;
                }
                liga_set_prime_id = g.move_child(
                    this_index,
                    6 + 2 * i,
                    liga_subst_prime_id,
                    6 + 2 * liga_set_count as usize,
                );
                liga_set_count += 1;
                compact_liga_set(g, liga_set);
            } else {
                let start_index = start.max(current_start) - count;
                let end_index = end.min(current_end) - count;
                let liga_count = end_index - start_index;
                // `new_liga_set`
                let prime_size = 2 + liga_count as usize * 2;
                let prime = create_node(g, prime_size);
                if prime == NONE {
                    return NONE;
                }
                g.set_u16_at(prime, 0, liga_count);
                liga_set_prime_id = prime;
                g.move_children(
                    liga_set_index,
                    2 + start_index * 2,
                    2 + end_index * 2,
                    liga_set_prime_id,
                    2,
                );
                liga_set_end = i as u32;
                if (i as u32) < liga_set_start {
                    liga_set_start = i as u32;
                }
                g.add_link(
                    6 + 2 * liga_set_count,
                    liga_subst_prime_id,
                    liga_set_prime_id,
                );
                liga_set_count += 1;
            }
            clear_virtual_links(g, liga_set_prime_id);
            add_virtual_link(g, liga_set_prime_id, coverage_prime_id);
            for l in g.vertices[liga_set_prime_id as usize].real_links.clone() {
                clear_virtual_links(g, l.objidx);
                add_virtual_link(g, l.objidx, coverage_prime_id);
            }
            count += num_ligas;
        }
        let drop = (set_len - liga_set_count) as usize * 2;
        g.vertices[liga_subst_prime_id as usize].obj.len -= drop;
        g.set_u16_at(liga_subst_prime_id, 4, liga_set_count);
        if !filter_coverage(g, coverage_prime_id, liga_set_start, liga_set_end + 1) {
            return NONE;
        }
        liga_subst_prime_id
    }

    fn shrink(&mut self, c: &mut Ctx, g: &mut Graph, count: u32) -> bool {
        let _ = c;
        let this_index = self.this_index;
        let old_count = self.original_count;
        if count >= old_count {
            return true;
        }
        let mut count = count;
        let mut retained_indices: BTreeSet<u32> = BTreeSet::new();
        let mut new_liga_set_count = 0u32;
        for (i, &num_ligas) in self.liga_counts.clone().iter().enumerate() {
            let Some(liga_set) = as_index(g, this_index, 6 + 2 * i, ligature_set_ok) else {
                return false;
            };
            clear_virtual_links(g, liga_set);
            retained_indices.insert(liga_set);
            let index_to_id = ligature_index_to_object_id(g, liga_set);
            for &liga_index in index_to_id.iter().take(g.u16_at(liga_set, 0) as usize) {
                if liga_index != NONE {
                    clear_virtual_links(g, liga_index);
                    retained_indices.insert(liga_index);
                }
            }
            if num_ligas >= count {
                let num_ligas_to_remove = num_ligas - count;
                new_liga_set_count = i as u32 + 1;
                g.vertices[liga_set as usize].obj.len -= num_ligas_to_remove as usize * 2;
                g.set_u16_at(liga_set, 0, count);
                break;
            }
            count -= num_ligas;
        }
        let set_len = g.u16_at(this_index, 4);
        g.vertices[this_index as usize].obj.len -= (set_len - new_liga_set_count) as usize * 2;
        g.set_u16_at(this_index, 4, new_liga_set_count);
        let mut coverage_idx = g.index_for_offset(this_index, 2);
        if coverage_idx == NONE {
            return false;
        }
        let coverage_size = g.bytes(coverage_idx).len();
        let coverage_obj = g.vertices[coverage_idx as usize].obj;
        let coverage_glyphs_v = Coverage(View::new(
            &g.bufs[coverage_obj.buf][coverage_obj.head..coverage_obj.head + coverage_obj.len],
        ))
        .iter();
        if g.vertices[coverage_idx as usize].is_shared() {
            coverage_idx = g.remap_child(this_index, coverage_idx);
            if coverage_idx == NONE {
                return false;
            }
        }
        for &i in &retained_indices {
            add_virtual_link(g, i, coverage_idx);
        }
        let new_coverage: Vec<u32> = coverage_glyphs_v
            .into_iter()
            .zip(0u32..)
            .filter(|&(_, i)| i < new_liga_set_count)
            .map(|p| p.0)
            .collect();
        make_coverage(g, &new_coverage, coverage_idx, coverage_size)
    }
}

/// Keeps the `BTreeMap` import used by the maps above in one place.
#[allow(dead_code)]
type Unused = BTreeMap<u32, u32>;
