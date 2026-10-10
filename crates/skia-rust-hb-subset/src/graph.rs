// Copyright © 2022  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/graph/graph.hh, src/graph/serialize.hh, src/hb-priority-queue.hh (harfbuzz 9cb1fee5)

//! The object graph of the repacker (`graph::graph_t`): the packed objects of a serializer as
//! vertices with links, parent bookkeeping, the shortest-distance ordering, 32-bit space
//! assignment, subgraph duplication and isolation, and the final serialization.
//!
//! Object bytes live in buffers shared by reference like `HarfBuzz`'s `head`/`tail` pointers: a
//! duplicate points to the same bytes as its original, and an in-place edit is visible to both.

use std::collections::{BTreeSet, HashSet};

use crate::hbmap::HbMap;
use crate::serialize::{Link, Whence};

pub(crate) const NONE: u32 = u32::MAX;

/// `graph_t::vertex_t::obj`: a link of an object (`object_t::link_t`) with 32-bit indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GLink {
    pub width: u8,
    pub is_signed: bool,
    pub whence: Whence,
    pub bias: u32,
    pub position: u32,
    pub objidx: u32,
}

impl GLink {
    /// A link with `Head` whence, no bias and unsigned, like the ones the graph code creates.
    pub(crate) fn new(width: u8, objidx: u32, position: u32) -> GLink {
        GLink {
            width,
            is_signed: false,
            whence: Whence::Head,
            bias: 0,
            position,
            objidx,
        }
    }
}

/// `hb_priority_queue_t<int64_t>`: a binary min-heap with `HarfBuzz`'s sifting.
#[derive(Default)]
pub(crate) struct PriorityQueue {
    heap: Vec<(i64, u32)>,
}

impl PriorityQueue {
    pub(crate) fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    pub(crate) fn insert(&mut self, priority: i64, value: u32) {
        self.heap.push((priority, value));
        self.bubble_up(self.heap.len() - 1);
    }

    pub(crate) fn pop_minimum(&mut self) -> (i64, u32) {
        let result = self.heap[0];
        let last = self.heap[self.heap.len() - 1];
        self.heap[0] = last;
        self.heap.pop();
        if !self.heap.is_empty() {
            self.bubble_down(0);
        }
        result
    }

    fn bubble_down(&mut self, mut index: usize) {
        loop {
            let left = 2 * index + 1;
            let right = 2 * index + 2;
            if left >= self.heap.len() {
                return;
            }
            let has_right = right < self.heap.len();
            if self.heap[index].0 <= self.heap[left].0
                && (!has_right || self.heap[index].0 <= self.heap[right].0)
            {
                return;
            }
            let child = if !has_right || self.heap[left].0 < self.heap[right].0 {
                left
            } else {
                right
            };
            self.heap.swap(index, child);
            index = child;
        }
    }

    fn bubble_up(&mut self, mut index: usize) {
        loop {
            if index == 0 {
                return;
            }
            let parent = (index - 1) / 2;
            if self.heap[parent].0 <= self.heap[index].0 {
                return;
            }
            self.heap.swap(index, parent);
            index = parent;
        }
    }
}

/// An object's bytes: `head`/`tail` as a buffer, an offset and a length.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Obj {
    pub buf: usize,
    pub head: usize,
    pub len: usize,
}

/// `graph_t::vertex_t`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Vertex {
    pub obj: Obj,
    pub real_links: Vec<GLink>,
    pub virtual_links: Vec<GLink>,
    pub distance: i64,
    pub space: u32,
    pub start: u32,
    pub end: u32,
    pub priority: u32,
    incoming_edges: u32,
    single_parent: u32,
    has_incoming_virtual_edges: bool,
    parents: HbMap,
}

impl Vertex {
    fn blank() -> Vertex {
        Vertex {
            single_parent: NONE,
            ..Vertex::default()
        }
    }

    pub(crate) fn table_size(&self) -> usize {
        self.obj.len
    }

    pub(crate) fn is_shared(&self) -> bool {
        self.parents.len() > 1
    }

    pub(crate) fn incoming_edges(&self) -> u32 {
        self.incoming_edges
    }

    pub(crate) fn has_incoming_virtual_edges(&self) -> bool {
        self.has_incoming_virtual_edges
    }

    pub(crate) fn incoming_edges_from_parent(&self, parent_index: u32) -> u32 {
        if self.single_parent != NONE {
            return u32::from(self.single_parent == parent_index);
        }
        self.parents.has(parent_index).unwrap_or(0)
    }

    pub(crate) fn is_leaf(&self) -> bool {
        self.real_links.is_empty() && self.virtual_links.is_empty()
    }

    fn reset_parents(&mut self) {
        self.incoming_edges = 0;
        self.has_incoming_virtual_edges = false;
        self.single_parent = NONE;
        self.parents.reset();
    }

    /// `add_parent`.
    pub(crate) fn add_parent(&mut self, parent_index: u32, is_virtual: bool) {
        self.has_incoming_virtual_edges |= is_virtual;
        if self.incoming_edges == 0 {
            self.single_parent = parent_index;
            self.incoming_edges = 1;
            return;
        } else if self.single_parent != NONE {
            self.parents.set(self.single_parent, 1);
            self.single_parent = NONE;
        }
        if let Some(v) = self.parents.get_mut(parent_index) {
            *v += 1;
            self.incoming_edges += 1;
        } else {
            self.parents.set(parent_index, 1);
            self.incoming_edges += 1;
        }
    }

    /// `remove_parent`.
    pub(crate) fn remove_parent(&mut self, parent_index: u32) {
        if parent_index == self.single_parent {
            self.single_parent = NONE;
            self.incoming_edges -= 1;
            return;
        }
        if let Some(v) = self.parents.has(parent_index) {
            self.incoming_edges -= 1;
            if v > 1 {
                if let Some(m) = self.parents.get_mut(parent_index) {
                    *m -= 1;
                }
            } else {
                self.parents.del(parent_index);
            }
            if self.incoming_edges == 1 {
                self.single_parent = self.parents.keys().next().unwrap_or(NONE);
                self.parents.reset();
            }
        }
    }

    /// `remap_parent`.
    pub(crate) fn remap_parent(&mut self, old_index: u32, new_index: u32) {
        if self.single_parent != NONE {
            if self.single_parent == old_index {
                self.single_parent = new_index;
            }
            return;
        }
        if let Some(v) = self.parents.has(old_index) {
            self.parents.set(new_index, v);
            self.parents.del(old_index);
            if self.incoming_edges == 1 {
                self.single_parent = self.parents.keys().next().unwrap_or(NONE);
                self.parents.reset();
            }
        }
    }

    /// `parents_iter`: the single parent, or the keys of the parents map in table order.
    pub(crate) fn parents_iter(&self) -> Vec<u32> {
        let mut out = Vec::new();
        if self.single_parent != NONE {
            out.push(self.single_parent);
        }
        out.extend(self.parents.keys());
        out
    }

    /// `remove_real_link (child_index, offset)`: `remove_unordered` of the link at the position.
    pub(crate) fn remove_real_link(&mut self, child_index: u32, position: u32) {
        for i in 0..self.real_links.len() {
            let link = self.real_links[i];
            if link.objidx != child_index || link.position != position {
                continue;
            }
            self.real_links.swap_remove(i);
            return;
        }
    }

    pub(crate) fn raise_priority(&mut self) -> bool {
        if self.has_max_priority() {
            return false;
        }
        self.priority += 1;
        true
    }

    pub(crate) fn give_max_priority(&mut self) -> bool {
        let mut result = false;
        while !self.has_max_priority() {
            result = true;
            self.priority += 1;
        }
        result
    }

    pub(crate) fn has_max_priority(&self) -> bool {
        self.priority >= 3
    }

    fn distance_modifier(&self) -> i64 {
        if self.priority == 0 {
            return 0;
        }
        let table_size = self.obj.len as i64;
        if self.priority == 1 {
            return -table_size / 2;
        }
        -table_size
    }

    /// `modified_distance`.
    fn modified_distance(&self, order: u32) -> i64 {
        let mut modified = (self.distance + self.distance_modifier()).clamp(0, 0x7FF_FFFF_FFFF);
        if self.has_max_priority() {
            modified = 0;
        }
        (modified << 18) | i64::from(0x3_FFFF & order)
    }

    /// `all_links ()`: the real links and then the virtual ones.
    pub(crate) fn all_links(&self) -> Vec<GLink> {
        let mut v = self.real_links.clone();
        v.extend_from_slice(&self.virtual_links);
        v
    }

    /// `position_to_index_map`.
    pub(crate) fn position_to_index_map(&self) -> HbMap {
        let mut result = HbMap::new();
        for l in &self.real_links {
            result.set(l.position, l.objidx);
        }
        result
    }
}

/// `overflow_record_t`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OverflowRecord {
    pub parent: u32,
    pub child: u32,
}

/// A set that can be inverted, for `hb_set_t::invert` in `assign_spaces`.
struct InvSet {
    set: BTreeSet<u32>,
    inverted: bool,
}

impl InvSet {
    fn has(&self, x: u32) -> bool {
        self.inverted ^ self.set.contains(&x)
    }

    fn add(&mut self, x: u32) {
        if self.inverted {
            self.set.remove(&x);
        } else {
            self.set.insert(x);
        }
    }
}

/// A packed object of the serializer: `(bytes, real links, virtual links)`.
pub(crate) type PackedObject = (Vec<u8>, Vec<Link>, Vec<Link>);

/// `graph_t`.
#[allow(clippy::struct_excessive_bools)] // the flags of `graph_t`
pub(crate) struct Graph {
    pub vertices: Vec<Vertex>,
    pub ordering: Vec<u32>,
    ordering_scratch: Vec<u32>,
    parents_invalid: bool,
    pub distance_invalid: bool,
    pub positions_invalid: bool,
    pub successful: bool,
    num_roots_for_space: Vec<u32>,
    pub bufs: Vec<Vec<u8>>,
}

impl Graph {
    /// The constructor from the packed objects of a serializer (`graph_t (const T& objects)`):
    /// `(bytes, real links, virtual links)`, the first being the nil object.
    pub(crate) fn new(objects: Vec<Option<PackedObject>>) -> Graph {
        let mut g = Graph {
            vertices: Vec::new(),
            ordering: Vec::new(),
            ordering_scratch: Vec::new(),
            parents_invalid: true,
            distance_invalid: true,
            positions_invalid: true,
            successful: true,
            num_roots_for_space: vec![1],
            // buffer 0 stands for the empty `nullptr` object
            bufs: vec![Vec::new()],
        };
        let count = objects.len();
        let removed_nil = matches!(objects.first(), Some(None));
        let n = if removed_nil { count - 1 } else { count };
        g.ordering = vec![0; n];
        let mut order = n;
        for (i, obj) in objects.into_iter().enumerate() {
            let Some((bytes, real, virt)) = obj else {
                continue;
            };
            let mut v = Vertex::blank();
            let buf = g.bufs.len();
            v.obj = Obj {
                buf,
                head: 0,
                len: bytes.len(),
            };
            g.bufs.push(bytes);
            let adjust = |l: &Link| GLink {
                width: l.width,
                is_signed: l.is_signed,
                whence: l.whence,
                bias: l.bias,
                position: l.position,
                objidx: (l.objidx as u32).wrapping_sub(u32::from(removed_nil)),
            };
            v.real_links = real.iter().map(adjust).collect();
            v.virtual_links = virt.iter().map(adjust).collect();
            if !link_positions_valid(&v, count, removed_nil, &real) {
                g.successful = false;
            }
            let obj_idx = if removed_nil { i - 1 } else { i };
            order -= 1;
            g.ordering[order] = obj_idx as u32;
            g.vertices.push(v);
        }
        g
    }

    pub(crate) fn in_error(&self) -> bool {
        !self.successful
    }

    pub(crate) fn root_idx(&self) -> u32 {
        self.ordering[0]
    }

    pub(crate) fn check_success(&mut self, success: bool) -> bool {
        if !self.successful {
            return false;
        }
        if !success {
            self.successful = false;
        }
        success
    }

    // ---- bytes ----

    pub(crate) fn bytes(&self, v: u32) -> &[u8] {
        let o = self.vertices[v as usize].obj;
        &self.bufs[o.buf][o.head..o.head + o.len]
    }

    pub(crate) fn bytes_mut(&mut self, v: u32) -> &mut [u8] {
        let o = self.vertices[v as usize].obj;
        &mut self.bufs[o.buf][o.head..o.head + o.len]
    }

    pub(crate) fn u16_at(&self, v: u32, pos: usize) -> u32 {
        crate::bytes::u16_at(self.bytes(v), pos).into()
    }

    pub(crate) fn set_u16_at(&mut self, v: u32, pos: usize, value: u32) {
        let b = self.bytes_mut(v);
        if let Some(s) = b.get_mut(pos..pos + 2) {
            s.copy_from_slice(&(value as u16).to_be_bytes());
        }
    }

    /// `add_buffer` + the pointer into it: a new buffer, as an `Obj`.
    pub(crate) fn add_buffer(&mut self, bytes: Vec<u8>) -> Obj {
        let buf = self.bufs.len();
        let len = bytes.len();
        self.bufs.push(bytes);
        Obj { buf, head: 0, len }
    }

    // ---- parents, distances, positions ----

    pub(crate) fn update_parents(&mut self) {
        if !self.parents_invalid {
            return;
        }
        for v in &mut self.vertices {
            v.reset_parents();
        }
        for p in 0..self.vertices.len() {
            let real = self.vertices[p].real_links.clone();
            let virt = self.vertices[p].virtual_links.clone();
            for l in real {
                self.vertices[l.objidx as usize].add_parent(p as u32, false);
            }
            for l in virt {
                self.vertices[l.objidx as usize].add_parent(p as u32, true);
            }
        }
        self.parents_invalid = false;
    }

    pub(crate) fn update_positions(&mut self) {
        if !self.positions_invalid {
            return;
        }
        let mut current_pos = 0u32;
        for &i in &self.ordering {
            let v = &mut self.vertices[i as usize];
            v.start = current_pos;
            current_pos += v.obj.len as u32;
            v.end = current_pos;
        }
        self.positions_invalid = false;
    }

    fn update_distances(&mut self) {
        if !self.distance_invalid {
            return;
        }
        let count = self.vertices.len();
        for v in &mut self.vertices {
            v.distance = i64::MAX;
        }
        let root = self.root_idx();
        self.vertices[root as usize].distance = 0;
        let mut queue = PriorityQueue::default();
        queue.insert(0, root);
        let mut visited = vec![false; count];
        while !queue.is_empty() {
            let next_idx = queue.pop_minimum().1 as usize;
            if visited[next_idx] {
                continue;
            }
            let next_distance = self.vertices[next_idx].distance;
            visited[next_idx] = true;
            for link in self.vertices[next_idx].all_links() {
                if visited[link.objidx as usize] {
                    continue;
                }
                let child_v = &mut self.vertices[link.objidx as usize];
                let link_width = if link.width != 0 {
                    u32::from(link.width)
                } else {
                    4
                };
                let child_weight = child_v.obj.len as i64
                    + (1i64 << (link_width * 8)) * i64::from(child_v.space + 1);
                let child_distance = next_distance + child_weight;
                if child_distance < child_v.distance {
                    child_v.distance = child_distance;
                    queue.insert(child_distance, link.objidx);
                }
            }
        }
        self.distance_invalid = false;
    }

    /// `sort_shortest_distance_if_needed`.
    pub(crate) fn sort_shortest_distance_if_needed(&mut self) {
        if !self.positions_invalid {
            return;
        }
        self.sort_shortest_distance();
    }

    /// `sort_shortest_distance`.
    pub(crate) fn sort_shortest_distance(&mut self) {
        self.positions_invalid = true;
        if self.vertices.len() <= 1 {
            return;
        }
        self.update_distances();
        let n = self.vertices.len();
        let mut queue = PriorityQueue::default();
        let mut new_ordering = std::mem::take(&mut self.ordering_scratch);
        new_ordering.resize(n, 0);
        let mut removed_edges = vec![0u32; n];
        self.update_parents();
        let root = self.root_idx();
        queue.insert(self.vertices[root as usize].modified_distance(0), root);
        let mut order = 1u32;
        let mut pos = 0usize;
        while !queue.is_empty() {
            let next_id = queue.pop_minimum().1;
            if !self.check_success(pos < new_ordering.len()) {
                self.ordering_scratch = new_ordering;
                return;
            }
            new_ordering[pos] = next_id;
            pos += 1;
            for link in self.vertices[next_id as usize].all_links() {
                removed_edges[link.objidx as usize] += 1;
                let v = &self.vertices[link.objidx as usize];
                if v.incoming_edges
                    .wrapping_sub(removed_edges[link.objidx as usize])
                    == 0
                {
                    queue.insert(v.modified_distance(order), link.objidx);
                    order += 1;
                }
            }
        }
        std::mem::swap(&mut self.ordering, &mut new_ordering);
        self.ordering_scratch = new_ordering;
        self.check_success(pos == n);
    }

    // ---- subgraphs ----

    /// `find_subgraph (node_idx, hb_map_t&)`: counts the incoming links of each node.
    fn find_subgraph_map(&self, node_idx: u32, subgraph: &mut HbMap) {
        for link in self.vertices[node_idx as usize].all_links() {
            if let Some(v) = subgraph.get_mut(link.objidx) {
                *v += 1;
                continue;
            }
            subgraph.set(link.objidx, 1);
            self.find_subgraph_map(link.objidx, subgraph);
        }
    }

    /// `find_subgraph (node_idx, hb_set_t&)`.
    pub(crate) fn find_subgraph(&self, node_idx: u32, subgraph: &mut BTreeSet<u32>) {
        if !subgraph.insert(node_idx) {
            return;
        }
        for link in self.vertices[node_idx as usize].all_links() {
            self.find_subgraph(link.objidx, subgraph);
        }
    }

    /// `find_subgraph_size`.
    pub(crate) fn find_subgraph_size(
        &self,
        node_idx: u32,
        subgraph: &mut BTreeSet<u32>,
        max_depth: u32,
    ) -> usize {
        if !subgraph.insert(node_idx) {
            return 0;
        }
        let o = &self.vertices[node_idx as usize];
        let mut size = o.obj.len;
        if max_depth == 0 {
            return size;
        }
        for link in o.all_links() {
            size += self.find_subgraph_size(link.objidx, subgraph, max_depth.wrapping_sub(1));
        }
        size
    }

    /// `find_32bit_roots`.
    fn find_32bit_roots(&self, node_idx: u32, found: &mut BTreeSet<u32>) {
        for link in self.vertices[node_idx as usize].all_links() {
            if !link.is_signed && link.width == 4 {
                found.insert(link.objidx);
                continue;
            }
            self.find_32bit_roots(link.objidx, found);
        }
    }

    fn find_space_roots(&self, visited: &mut BTreeSet<u32>, roots: &mut BTreeSet<u32>) {
        let root_index = self.root_idx();
        for &i in &self.ordering {
            if visited.contains(&i) {
                continue;
            }
            for l in &self.vertices[i as usize].real_links {
                if l.is_signed || l.width < 3 {
                    continue;
                }
                if i == root_index && l.width == 3 {
                    continue;
                }
                if l.width == 3 {
                    let mut sub_roots = BTreeSet::new();
                    self.find_32bit_roots(l.objidx, &mut sub_roots);
                    if !sub_roots.is_empty() {
                        for &sub_root_idx in &sub_roots {
                            roots.insert(sub_root_idx);
                            self.find_subgraph(sub_root_idx, visited);
                        }
                        continue;
                    }
                }
                roots.insert(l.objidx);
                self.find_subgraph(l.objidx, visited);
            }
        }
    }

    fn find_connected_nodes(
        &self,
        start_idx: u32,
        targets: &mut BTreeSet<u32>,
        visited: &mut InvSet,
        connected: &mut BTreeSet<u32>,
    ) {
        if visited.has(start_idx) {
            return;
        }
        visited.add(start_idx);
        if targets.remove(&start_idx) {
            connected.insert(start_idx);
        }
        let v = &self.vertices[start_idx as usize];
        for l in v.all_links() {
            self.find_connected_nodes(l.objidx, targets, visited, connected);
        }
        for p in v.parents_iter() {
            self.find_connected_nodes(p, targets, visited, connected);
        }
    }

    /// `assign_spaces`.
    pub(crate) fn assign_spaces(&mut self) -> bool {
        self.update_parents();
        let mut visited_set = BTreeSet::new();
        let mut roots = BTreeSet::new();
        self.find_space_roots(&mut visited_set, &mut roots);
        let mut visited = InvSet {
            set: visited_set,
            inverted: true,
        };
        if roots.is_empty() {
            return false;
        }
        while let Some(&next) = roots.iter().next() {
            let mut connected_roots = BTreeSet::new();
            self.find_connected_nodes(next, &mut roots, &mut visited, &mut connected_roots);
            self.isolate_subgraph(&mut connected_roots);
            let next_space = self.next_space();
            self.num_roots_for_space.push(0);
            for &root in &connected_roots {
                self.vertices[root as usize].space = next_space;
                self.num_roots_for_space[next_space as usize] += 1;
                self.distance_invalid = true;
                self.positions_invalid = true;
            }
        }
        true
    }

    fn wide_parents(&self, node_idx: u32, parents: &mut BTreeSet<u32>) -> u32 {
        let mut count = 0;
        for p in self.vertices[node_idx as usize].parents_iter() {
            for l in &self.vertices[p as usize].real_links {
                if l.objidx == node_idx && (l.width == 3 || l.width == 4) && !l.is_signed {
                    count += 1;
                    parents.insert(p);
                }
            }
        }
        count
    }

    fn reassign_link(
        &mut self,
        parent_idx: u32,
        link_index: usize,
        is_virtual: bool,
        new_idx: u32,
    ) {
        let old_idx = {
            let v = &mut self.vertices[parent_idx as usize];
            let link = if is_virtual {
                &mut v.virtual_links[link_index]
            } else {
                &mut v.real_links[link_index]
            };
            let old = link.objidx;
            link.objidx = new_idx;
            old
        };
        self.vertices[old_idx as usize].remove_parent(parent_idx);
        self.vertices[new_idx as usize].add_parent(parent_idx, is_virtual);
    }

    fn remap_obj_indices(&mut self, id_map: &HbMap, subgraph: &[u32], only_wide: bool) {
        if id_map.is_empty() {
            return;
        }
        for &i in subgraph {
            let num_real = self.vertices[i as usize].real_links.len();
            let total = num_real + self.vertices[i as usize].virtual_links.len();
            for count in 0..total {
                let (is_virtual, idx) = if count < num_real {
                    (false, count)
                } else {
                    (true, count - num_real)
                };
                let link = if is_virtual {
                    self.vertices[i as usize].virtual_links[idx]
                } else {
                    self.vertices[i as usize].real_links[idx]
                };
                let Some(v) = id_map.has(link.objidx) else {
                    continue;
                };
                if only_wide && (link.is_signed || (link.width != 4 && link.width != 3)) {
                    continue;
                }
                self.reassign_link(i, idx, is_virtual, v);
            }
        }
    }

    /// `isolate_subgraph`.
    pub(crate) fn isolate_subgraph(&mut self, roots: &mut BTreeSet<u32>) -> bool {
        self.update_parents();
        let mut subgraph = HbMap::new();
        let mut parents = BTreeSet::new();
        for &root_idx in roots.iter() {
            let n = self.wide_parents(root_idx, &mut parents);
            subgraph.set(root_idx, n);
            self.find_subgraph_map(root_idx, &mut subgraph);
        }
        let mut index_map = HbMap::new();
        let mut made_changes = false;
        let entries: Vec<(u32, u32)> = subgraph.iter().collect();
        for (node, subgraph_incoming_edges) in entries {
            if subgraph_incoming_edges < self.vertices[node as usize].incoming_edges() {
                made_changes = true;
                self.duplicate_subgraph(node, &mut index_map);
            }
        }
        if self.in_error() {
            return false;
        }
        if !made_changes {
            return false;
        }
        let new_subgraph: Vec<u32> = subgraph
            .keys()
            .map(|k| index_map.has(k).unwrap_or(k))
            .collect();
        self.remap_obj_indices(&index_map, &new_subgraph, false);
        let parents_vec: Vec<u32> = parents.iter().copied().collect();
        self.remap_obj_indices(&index_map, &parents_vec, true);
        for next in roots.clone() {
            if let Some(v) = index_map.has(next) {
                roots.remove(&next);
                roots.insert(v);
            }
        }
        true
    }

    fn duplicate_subgraph(&mut self, node_idx: u32, index_map: &mut HbMap) {
        if index_map.contains(node_idx) {
            return;
        }
        let clone_idx = self.duplicate(node_idx);
        if !self.check_success(clone_idx != NONE) {
            return;
        }
        index_map.set(node_idx, clone_idx);
        for l in self.vertices[node_idx as usize].all_links() {
            self.duplicate_subgraph(l.objidx, index_map);
        }
    }

    /// `duplicate (node_idx)`.
    pub(crate) fn duplicate(&mut self, node_idx: u32) -> u32 {
        self.positions_invalid = true;
        self.distance_invalid = true;
        let clone_idx = self.vertices.len() as u32;
        let child = self.vertices[node_idx as usize].clone();
        let mut clone = Vertex::blank();
        self.ordering.push(clone_idx);
        clone.obj = child.obj;
        clone.distance = child.distance;
        clone.space = child.space;
        clone.reset_parents();
        for l in &child.real_links {
            clone.real_links.push(*l);
        }
        for l in &child.virtual_links {
            clone.virtual_links.push(*l);
        }
        self.vertices.push(clone);
        for l in &child.real_links {
            self.vertices[l.objidx as usize].add_parent(clone_idx, false);
        }
        for l in &child.virtual_links {
            self.vertices[l.objidx as usize].add_parent(clone_idx, true);
        }
        clone_idx
    }

    /// `duplicate (parent_idx, child_idx)`.
    pub(crate) fn duplicate_for_parent(&mut self, parent_idx: u32, child_idx: u32) -> u32 {
        self.update_parents();
        let child = &self.vertices[child_idx as usize];
        let links_to_child = child.incoming_edges_from_parent(parent_idx);
        if child.incoming_edges() <= links_to_child || child.has_incoming_virtual_edges() {
            return NONE;
        }
        let clone_idx = self.duplicate(child_idx);
        if clone_idx == NONE {
            return NONE;
        }
        let mut parent_idx = parent_idx;
        if parent_idx == clone_idx {
            parent_idx += 1;
        }
        self.reassign_links_to(parent_idx, child_idx, clone_idx);
        clone_idx
    }

    /// For each link of the parent to `child_idx`: `reassign_link (l, parent_idx, clone_idx, count > num_real)`.
    fn reassign_links_to(&mut self, parent_idx: u32, child_idx: u32, clone_idx: u32) {
        let num_real = self.vertices[parent_idx as usize].real_links.len();
        let total = num_real + self.vertices[parent_idx as usize].virtual_links.len();
        for count in 0..total {
            let (is_virtual, idx) = if count < num_real {
                (false, count)
            } else {
                (true, count - num_real)
            };
            let objidx = if is_virtual {
                self.vertices[parent_idx as usize].virtual_links[idx].objidx
            } else {
                self.vertices[parent_idx as usize].real_links[idx].objidx
            };
            if objidx != child_idx {
                continue;
            }
            self.reassign_link(parent_idx, idx, is_virtual, clone_idx);
        }
    }

    /// `duplicate (const hb_set_t* parents, child_idx)`.
    pub(crate) fn duplicate_for_parents(&mut self, parents: &BTreeSet<u32>, child_idx: u32) -> u32 {
        if parents.is_empty() {
            return NONE;
        }
        self.update_parents();
        let child = &self.vertices[child_idx as usize];
        let mut links_to_child = 0;
        for &parent_idx in parents {
            links_to_child += child.incoming_edges_from_parent(parent_idx);
        }
        if child.incoming_edges() <= links_to_child || child.has_incoming_virtual_edges() {
            return NONE;
        }
        let clone_idx = self.duplicate(child_idx);
        if clone_idx == NONE {
            return NONE;
        }
        for &parent_idx in parents {
            let mut parent_idx = parent_idx;
            if parent_idx == clone_idx {
                parent_idx += 1;
            }
            self.reassign_links_to(parent_idx, child_idx, clone_idx);
        }
        clone_idx
    }

    /// `new_node (head, tail)` for a buffer.
    pub(crate) fn new_node(&mut self, obj: Obj) -> u32 {
        self.positions_invalid = true;
        self.distance_invalid = true;
        let clone_idx = self.vertices.len() as u32;
        let mut v = Vertex::blank();
        v.obj = obj;
        self.vertices.push(v);
        self.ordering.push(clone_idx);
        clone_idx
    }

    /// `remap_child`.
    pub(crate) fn remap_child(&mut self, parent_idx: u32, old_child_idx: u32) -> u32 {
        let new_child_idx = self.duplicate(old_child_idx);
        if new_child_idx == NONE {
            return NONE;
        }
        for i in 0..self.vertices[parent_idx as usize].real_links.len() {
            if self.vertices[parent_idx as usize].real_links[i].objidx != old_child_idx {
                continue;
            }
            self.reassign_link(parent_idx, i, false, new_child_idx);
        }
        for i in 0..self.vertices[parent_idx as usize].virtual_links.len() {
            if self.vertices[parent_idx as usize].virtual_links[i].objidx != old_child_idx {
                continue;
            }
            self.reassign_link(parent_idx, i, true, new_child_idx);
        }
        new_child_idx
    }

    /// `raise_childrens_priority`.
    pub(crate) fn raise_childrens_priority(&mut self, parent_idx: u32) -> bool {
        let mut made_change = false;
        for l in self.vertices[parent_idx as usize].all_links() {
            made_change |= self.vertices[l.objidx as usize].raise_priority();
        }
        made_change
    }

    /// `is_fully_connected`.
    pub(crate) fn is_fully_connected(&mut self) -> bool {
        self.update_parents();
        if self.vertices[self.root_idx() as usize].incoming_edges() != 0 {
            return false;
        }
        for i in 0..self.root_idx() {
            if self.vertices[i as usize].incoming_edges() == 0 {
                return false;
            }
        }
        true
    }

    pub(crate) fn num_roots_for_space(&self, space: u32) -> u32 {
        self.num_roots_for_space[space as usize]
    }

    pub(crate) fn next_space(&self) -> u32 {
        self.num_roots_for_space.len() as u32
    }

    /// `move_to_new_space`.
    pub(crate) fn move_to_new_space(&mut self, indices: &BTreeSet<u32>) {
        self.num_roots_for_space.push(0);
        let new_space = self.num_roots_for_space.len() as u32 - 1;
        for &index in indices {
            let old_space = self.vertices[index as usize].space;
            self.num_roots_for_space[old_space as usize] -= 1;
            self.num_roots_for_space[new_space as usize] += 1;
            self.vertices[index as usize].space = new_space;
            self.distance_invalid = true;
            self.positions_invalid = true;
        }
    }

    /// `space_for`: the space of the node and the root of its space.
    pub(crate) fn space_for(&self, mut index: u32) -> (u32, u32) {
        loop {
            let node = &self.vertices[index as usize];
            if node.space != 0 {
                return (node.space, index);
            }
            if node.incoming_edges() == 0 {
                return (0, index);
            }
            index = node.parents_iter()[0];
        }
    }

    /// `index_for_offset`: the child linked at `position` of the node, `NONE` when none is.
    pub(crate) fn index_for_offset(&self, node_idx: u32, position: usize) -> u32 {
        let node = &self.vertices[node_idx as usize];
        if position >= node.obj.len {
            return NONE;
        }
        for link in &node.real_links {
            if position as u32 != link.position {
                continue;
            }
            return link.objidx;
        }
        NONE
    }

    /// `mutable_index_for_offset`.
    pub(crate) fn mutable_index_for_offset(&mut self, node_idx: u32, position: usize) -> u32 {
        let child_idx = self.index_for_offset(node_idx, position);
        if child_idx == NONE {
            return NONE;
        }
        for p in self.vertices[child_idx as usize].parents_iter() {
            if p != node_idx {
                return self.duplicate_for_parent(node_idx, child_idx);
            }
        }
        child_idx
    }

    /// `add_link (offset, parent_id, child_id)` for a 16-bit offset at `position`.
    pub(crate) fn add_link(&mut self, position: u32, parent_id: u32, child_id: u32) {
        self.vertices[parent_id as usize]
            .real_links
            .push(GLink::new(2, child_id, position));
        self.vertices[child_id as usize].add_parent(parent_id, false);
    }

    /// `move_child`: the 16-bit offset at `old_position` of the old parent now sits at
    /// `new_position` of the new one.
    pub(crate) fn move_child(
        &mut self,
        old_parent_idx: u32,
        old_position: usize,
        new_parent_idx: u32,
        new_position: usize,
    ) -> u32 {
        self.distance_invalid = true;
        self.positions_invalid = true;
        let child_id = self.index_for_offset(old_parent_idx, old_position);
        self.vertices[new_parent_idx as usize]
            .real_links
            .push(GLink::new(2, child_id, new_position as u32));
        self.vertices[child_id as usize].add_parent(new_parent_idx, false);
        self.vertices[old_parent_idx as usize].remove_real_link(child_id, old_position as u32);
        self.vertices[child_id as usize].remove_parent(old_parent_idx);
        child_id
    }

    /// `move_children`.
    pub(crate) fn move_children(
        &mut self,
        old_parent_idx: u32,
        old_pos_start: u32,
        old_pos_end: u32,
        new_parent_idx: u32,
        new_pos_start: u32,
    ) {
        self.distance_invalid = true;
        self.positions_invalid = true;
        let mut old_links = Vec::new();
        let links = self.vertices[old_parent_idx as usize].real_links.clone();
        for l in links {
            if l.position < old_pos_start || l.position >= old_pos_end {
                old_links.push(l);
                continue;
            }
            let array_pos = l.position - old_pos_start;
            let child_id = l.objidx;
            self.vertices[new_parent_idx as usize]
                .real_links
                .push(GLink::new(2, child_id, new_pos_start + array_pos));
            self.vertices[child_id as usize].add_parent(new_parent_idx, false);
            self.vertices[child_id as usize].remove_parent(old_parent_idx);
        }
        self.vertices[old_parent_idx as usize].real_links = old_links;
    }

    /// `total_size_in_bytes`.
    fn total_size_in_bytes(&self) -> usize {
        self.vertices.iter().map(|v| v.obj.len).sum()
    }

    // ---- overflows and serialization ----

    /// `will_overflow (graph, &overflows)`: with `overflows` set, collects the records and returns
    /// whether there are any.
    pub(crate) fn will_overflow(
        &mut self,
        mut overflows: Option<&mut Vec<OverflowRecord>>,
    ) -> bool {
        if let Some(o) = overflows.as_deref_mut() {
            o.clear();
        }
        self.update_positions();
        let mut record_set: HashSet<u32> = HashSet::new();
        for &parent_idx in &self.ordering {
            for link in &self.vertices[parent_idx as usize].real_links {
                let offset = self.compute_offset(parent_idx, link);
                if is_valid_offset(offset, link) {
                    continue;
                }
                let Some(o) = overflows.as_deref_mut() else {
                    return true;
                };
                // The record set is keyed by the hash of the record (the key is a pointer to a
                // reused local, so only the hash tells the records apart).
                let hash = (parent_idx.wrapping_mul(2_654_435_761).wrapping_mul(31))
                    .wrapping_add(link.objidx.wrapping_mul(2_654_435_761))
                    & 0x3FFF_FFFF;
                if !record_set.insert(hash) {
                    continue;
                }
                o.push(OverflowRecord {
                    parent: parent_idx,
                    child: link.objidx,
                });
            }
        }
        match overflows {
            None => false,
            Some(o) => !o.is_empty(),
        }
    }

    fn compute_offset(&self, parent_idx: u32, link: &GLink) -> i64 {
        let parent = &self.vertices[parent_idx as usize];
        let child = &self.vertices[link.objidx as usize];
        let mut offset = match link.whence {
            Whence::Head => i64::from(child.start) - i64::from(parent.start),
            Whence::Tail => i64::from(child.start) - i64::from(parent.end),
            Whence::Absolute => i64::from(child.start),
        };
        offset -= i64::from(link.bias);
        offset
    }

    /// `graph::serialize`: the bytes in the order of the graph, or `None` on an error.
    pub(crate) fn serialize(&mut self) -> Option<Vec<u8>> {
        let size = self.total_size_in_bytes();
        if size == 0 {
            return Some(Vec::new());
        }
        self.update_positions();
        let mut out = Vec::with_capacity(size);
        for &i in &self.ordering {
            out.extend_from_slice(self.bytes(i));
        }
        for &i in &self.ordering {
            let v = &self.vertices[i as usize];
            for link in &v.real_links {
                if link.width == 0 {
                    continue;
                }
                let offset = self.compute_offset(i, link);
                if !is_valid_offset(offset, link) {
                    return None;
                }
                let p = v.start as usize + link.position as usize;
                let w = usize::from(link.width);
                let bytes = (offset as u64).to_be_bytes();
                out[p..p + w].copy_from_slice(&bytes[8 - w..]);
            }
        }
        Some(out)
    }
}

/// `is_valid_offset`.
fn is_valid_offset(offset: i64, link: &GLink) -> bool {
    if link.width == 0 {
        return link.is_signed || offset >= 0;
    }
    if link.is_signed {
        if link.width == 4 {
            (-(1i64 << 31)..(1i64 << 31)).contains(&offset)
        } else {
            (-(1 << 15)..(1 << 15)).contains(&offset)
        }
    } else if link.width == 4 {
        (0..(1i64 << 32)).contains(&offset)
    } else if link.width == 3 {
        (0..(1 << 24)).contains(&offset)
    } else {
        (0..(1 << 16)).contains(&offset)
    }
}

/// `vertex_t::link_positions_valid`.
fn link_positions_valid(
    v: &Vertex,
    num_objects: usize,
    removed_nil: bool,
    original: &[Link],
) -> bool {
    let mut assigned: BTreeSet<u32> = BTreeSet::new();
    for l in original {
        if l.objidx >= num_objects || (removed_nil && l.objidx == 0) {
            return false;
        }
        let start = l.position;
        if l.width < 2 || l.width > 4 {
            return false;
        }
        let end = start + u32::from(l.width) - 1;
        if end as usize >= v.table_size() {
            return false;
        }
        if assigned.range(start..=end).next().is_some() {
            return false;
        }
        assigned.extend(start..=end);
    }
    true
}
