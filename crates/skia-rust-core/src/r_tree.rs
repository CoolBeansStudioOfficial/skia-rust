// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRTree.h, src/core/SkRTree.cpp

//! `SkRTree`: an R-tree, a balanced n-ary tree containing a hierarchy of bounding rectangles.
//!
//! It only supports bulk-loading, i.e. creation from a batch of bounding rectangles. This performs
//! a bottom-up bulk load using the STR (sort-tile-recursive) algorithm.
//!
//! For more details see: Beckmann, N.; Kriegel, H. P.; Schneider, R.; Seeger, B. (1990). "The
//! R*-tree: an efficient and robust access method for points and rectangles".

use std::sync::Mutex;

use crate::bbh_factory::BBoxHierarchy;
use crate::rect::Rect;

/// A node's child: the index of an op (in a level 0 node) or of a subtree node (`SkRTree::Branch`,
/// whose `fSubtree`/`fOpIndex` union is the one `child` field).
// Port of: src/core/SkRTree.h#L53-L59 (chrome/m156)
#[derive(Copy, Clone, Debug)]
struct Branch {
    child: usize,
    bounds: Rect,
}

/// A tree node (`SkRTree::Node`).
// Port of: src/core/SkRTree.h#L61-L65 (chrome/m156)
#[derive(Copy, Clone, Debug)]
struct Node {
    num_children: u16,
    level: u16,
    children: [Branch; RTree::MAX_CHILDREN],
}

#[derive(Debug)]
struct RTreeState {
    /// The count of data elements (rather than total nodes in the tree).
    count: usize,
    root: Branch,
    nodes: Vec<Node>,
}

/// An R-tree (`SkRTree`).
// Port of: src/core/SkRTree.h#L30-L70 (chrome/m156)
#[doc(alias = "SkRTree")]
#[derive(Debug)]
pub struct RTree {
    state: Mutex<RTreeState>,
}

impl Default for RTree {
    fn default() -> Self {
        Self::new()
    }
}

impl RTree {
    /// These values were empirically determined to produce reasonable performance in most
    /// cases.
    #[doc(alias = "kMinChildren")]
    pub const MIN_CHILDREN: usize = 6;
    /// See [`RTree::MIN_CHILDREN`].
    #[doc(alias = "kMaxChildren")]
    pub const MAX_CHILDREN: usize = 11;

    /// An empty tree.
    // Port of: src/core/SkRTree.cpp#L12-L12 (chrome/m156)
    #[must_use]
    pub fn new() -> RTree {
        RTree {
            state: Mutex::new(RTreeState {
                count: 0,
                root: Branch {
                    child: 0,
                    bounds: Rect::new_empty(),
                },
                nodes: Vec::new(),
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, RTreeState> {
        // A poisoned lock only means another thread panicked; the tree is never left half-built
        // in a way a reader could misuse (insert builds into locals first).
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Returns the depth of the tree structure (`getDepth`).
    #[doc(alias = "getDepth")]
    #[must_use]
    pub fn get_depth(&self) -> usize {
        let state = self.lock();
        if state.count > 0 {
            usize::from(state.nodes[state.root.child].level) + 1
        } else {
            0
        }
    }

    /// Insertion count (not overall node count, which may be greater) (`getCount`).
    #[doc(alias = "getCount")]
    #[must_use]
    pub fn get_count(&self) -> usize {
        self.lock().count
    }
}

impl RTreeState {
    // Port of: src/core/SkRTree.cpp#L36-L44 (chrome/m156)
    fn allocate_node_at_level(&mut self, level: u16) -> usize {
        self.nodes.push(Node {
            num_children: 0,
            level,
            children: [Branch {
                child: 0,
                bounds: Rect::new_empty(),
            }; RTree::MAX_CHILDREN],
        });
        self.nodes.len() - 1
    }

    // This function parallels bulkLoad, but just counts how many nodes bulkLoad would allocate.
    // Port of: src/core/SkRTree.cpp#L47-L82 (chrome/m156)
    fn count_nodes(branches: usize) -> usize {
        if branches == 1 {
            return 1;
        }
        let mut remainder = branches % RTree::MAX_CHILDREN;
        if remainder > 0 {
            if remainder >= RTree::MIN_CHILDREN {
                remainder = 0;
            } else {
                remainder = RTree::MIN_CHILDREN - remainder;
            }
        }
        let mut current_branch = 0;
        let mut nodes = 0;
        while current_branch < branches {
            let mut increment_by = RTree::MAX_CHILDREN;
            if remainder != 0 {
                if remainder <= RTree::MAX_CHILDREN - RTree::MIN_CHILDREN {
                    increment_by -= remainder;
                    remainder = 0;
                } else {
                    increment_by = RTree::MIN_CHILDREN;
                    remainder -= RTree::MAX_CHILDREN - RTree::MIN_CHILDREN;
                }
            }
            nodes += 1;
            current_branch += 1;
            let mut k = 1;
            while k < increment_by && current_branch < branches {
                current_branch += 1;
                k += 1;
            }
        }
        nodes + Self::count_nodes(nodes)
    }

    // Consumes the input array.
    // Port of: src/core/SkRTree.cpp#L84-L132 (chrome/m156)
    fn bulk_load(&mut self, branches: &mut Vec<Branch>, level: u16) -> Branch {
        if branches.len() == 1 {
            // Only one branch.  It will be the root.
            return branches[0];
        }

        // We might sort our branches here, but we expect Blink gives us a reasonable x,y order.
        // Skipping a call to sort (in Y) here resulted in a 17% win for recording with negligible
        // difference in playback speed.
        let mut remainder = branches.len() % RTree::MAX_CHILDREN;
        let mut new_branches = 0;

        if remainder > 0 {
            // If the remainder isn't enough to fill a node, we'll add fewer nodes to other
            // branches.
            if remainder >= RTree::MIN_CHILDREN {
                remainder = 0;
            } else {
                remainder = RTree::MIN_CHILDREN - remainder;
            }
        }

        let mut current_branch = 0;
        while current_branch < branches.len() {
            let mut increment_by = RTree::MAX_CHILDREN;
            if remainder != 0 {
                // if need be, omit some nodes to make up for remainder
                if remainder <= RTree::MAX_CHILDREN - RTree::MIN_CHILDREN {
                    increment_by -= remainder;
                    remainder = 0;
                } else {
                    increment_by = RTree::MIN_CHILDREN;
                    remainder -= RTree::MAX_CHILDREN - RTree::MIN_CHILDREN;
                }
            }
            let n = self.allocate_node_at_level(level);
            self.nodes[n].num_children = 1;
            self.nodes[n].children[0] = branches[current_branch];
            let mut b = Branch {
                child: n,
                bounds: branches[current_branch].bounds,
            };
            current_branch += 1;
            let mut k = 1;
            while k < increment_by && current_branch < branches.len() {
                b.bounds.join(branches[current_branch].bounds);
                self.nodes[n].children[k] = branches[current_branch];
                self.nodes[n].num_children += 1;
                current_branch += 1;
                k += 1;
            }
            branches[new_branches] = b;
            new_branches += 1;
        }
        branches.truncate(new_branches);
        self.bulk_load(branches, level + 1)
    }

    // Port of: src/core/SkRTree.cpp#L142-L152 (chrome/m156)
    fn search_node(&self, node: usize, query: &Rect, results: &mut Vec<usize>) {
        let node = &self.nodes[node];
        for i in 0..usize::from(node.num_children) {
            if Rect::intersects2(node.children[i].bounds, query) {
                if 0 == node.level {
                    results.push(node.children[i].child);
                } else {
                    self.search_node(node.children[i].child, query, results);
                }
            }
        }
    }
}

impl BBoxHierarchy for RTree {
    // Port of: src/core/SkRTree.cpp#L14-L34 (chrome/m156)
    fn insert(&self, bounds_array: &[Rect]) {
        let mut state = self.lock();
        debug_assert_eq!(0, state.count);

        let mut branches: Vec<Branch> = Vec::with_capacity(bounds_array.len());

        for (i, bounds) in bounds_array.iter().enumerate() {
            if bounds.is_empty() {
                continue;
            }

            branches.push(Branch {
                child: i,
                bounds: *bounds,
            });
        }

        state.count = branches.len();
        if state.count != 0 {
            if 1 == state.count {
                state.nodes.reserve_exact(1);
                let n = state.allocate_node_at_level(0);
                state.nodes[n].num_children = 1;
                state.nodes[n].children[0] = branches[0];
                state.root = Branch {
                    child: n,
                    bounds: branches[0].bounds,
                };
            } else {
                let needed = RTreeState::count_nodes(state.count);
                state.nodes.reserve_exact(needed);
                let root = state.bulk_load(&mut branches, 0);
                state.root = root;
            }
        }
    }

    // Port of: src/core/SkRTree.cpp#L134-L140 (chrome/m156)
    fn search(&self, query: &Rect, results: &mut Vec<usize>) {
        let state = self.lock();
        if state.count > 0 && Rect::intersects2(state.root.bounds, query) {
            state.search_node(state.root.child, query, results);
        }
    }

    // Port of: src/core/SkRTree.cpp#L164-L170 (chrome/m156)
    fn bytes_used(&self) -> usize {
        let state = self.lock();
        let mut byte_count = std::mem::size_of::<RTree>();

        byte_count += state.nodes.capacity() * std::mem::size_of::<Node>();

        byte_count
    }
}
