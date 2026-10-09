// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/geom/IntersectionTree.h, src/gpu/graphite/geom/IntersectionTree.cpp

//! `skgpu::graphite::IntersectionTree`: a collection of non-overlapping rectangles.
//!
//! The C++ nodes live in an arena and are reached through virtual calls. Here they are a
//! `Box<Node>` enum: a binary space partition of `TreeNode`s over leaves that hold up to 64 rects
//! in lane-wise arrays. Every decision (split type, split coordinate, leaf layout, which lanes are
//! read) follows the C++.

// The `0.5f/kMaxRectsInList` and `int` to `float` conversions are the C++ arithmetic, exactly.
#![allow(clippy::cast_precision_loss)]

use skia_rust_core::t_pin::t_pin;
use skia_rust_simd::vx::Float4;

use crate::graphite::geom::rect::{ComplementRect, Rect};

// Port of: src/gpu/graphite/geom/IntersectionTree.h#L25-L28 (chrome/m156)
/// Whether a tree node splits along x (left/right) or y (top/bottom).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplitType {
    X,
    Y,
}

// Max number of rects to store in a leaf before splitting. With SSE/NEON optimizations, ~64 brute
// force rect comparisons seems to be the optimal number.
// Port of: src/gpu/graphite/geom/IntersectionTree.cpp#L66 (chrome/m156)
const MAX_RECTS_IN_LIST: usize = 64;

// BSP node. Space is partitioned by an either vertical or horizontal line. Note that if a rect
// straddles the partition line, it will need to go on both sides of the tree.
// Port of: src/gpu/graphite/geom/IntersectionTree.cpp#L21-L59 (chrome/m156)
#[derive(Debug)]
struct TreeNode {
    split_type: SplitType,
    split_coord: f32,
    lo: Box<Node>,
    hi: Box<Node>,
}

impl TreeNode {
    // Port of: src/gpu/graphite/geom/IntersectionTree.cpp#L49-L54 (chrome/m156)
    fn get_lo_val(&self, rect: &Rect) -> f32 {
        match self.split_type {
            SplitType::X => rect.left(),
            SplitType::Y => rect.top(),
        }
    }

    // Port of: src/gpu/graphite/geom/IntersectionTree.cpp#L52-L54 (chrome/m156)
    fn get_hi_val(&self, rect: &Rect) -> f32 {
        match self.split_type {
            SplitType::X => rect.right(),
            SplitType::Y => rect.bot(),
        }
    }

    // Port of: src/gpu/graphite/geom/IntersectionTree.cpp#L28-L36 (chrome/m156)
    fn intersects(&self, rect: Rect) -> bool {
        if self.get_lo_val(&rect) < self.split_coord && self.lo.intersects(rect) {
            return true;
        }
        if self.get_hi_val(&rect) > self.split_coord && self.hi.intersects(rect) {
            return true;
        }
        false
    }
}

// Leaf node. Rects are kept in a simple list and intersection testing is performed by brute force.
// Port of: src/gpu/graphite/geom/IntersectionTree.cpp#L62-L204 (chrome/m156)
#[derive(Debug)]
struct LeafNode {
    num_rects: usize,
    splittable_bounds: Float4, // [maxLeft, maxTop, -minRight, -minBot]
    rect_vals_sum: Float4,     // [sum(left), sum(top), -sum(right), -sum(bot)]
    lefts: [f32; MAX_RECTS_IN_LIST],
    tops: [f32; MAX_RECTS_IN_LIST],
    neg_rights: [f32; MAX_RECTS_IN_LIST],
    neg_bots: [f32; MAX_RECTS_IN_LIST],
}

impl LeafNode {
    // Port of: src/gpu/graphite/geom/IntersectionTree.cpp#L68-L78 (chrome/m156)
    fn new() -> Self {
        // Initialize our arrays with maximally negative rects. These have the advantage of always
        // failing intersection tests, thus allowing us to test for intersection beyond fNumRects
        // without failing.
        let mut leaf = Self {
            num_rects: 0,
            splittable_bounds: Float4::default(),
            rect_vals_sum: Float4::default(),
            lefts: [f32::INFINITY; MAX_RECTS_IN_LIST],
            tops: [f32::INFINITY; MAX_RECTS_IN_LIST],
            neg_rights: [f32::INFINITY; MAX_RECTS_IN_LIST],
            neg_bots: [f32::INFINITY; MAX_RECTS_IN_LIST],
        };
        leaf.pop_all();
        leaf
    }

    // Port of: src/gpu/graphite/geom/IntersectionTree.cpp#L80-L87 (chrome/m156)
    fn pop_all(&mut self) {
        self.num_rects = 0;
        self.splittable_bounds = Float4::splat(f32::NEG_INFINITY);
        self.rect_vals_sum = Float4::default();
        // Leave the rect arrays untouched. Since we know they are either already valid in the tree,
        // or else maximally negative, this allows the future list to check for intersection beyond
        // fNumRects without failing.
    }

    // Port of: src/gpu/graphite/geom/IntersectionTree.cpp#L89-L109 (chrome/m156)
    fn intersects(&self, rect: Rect) -> bool {
        // Test for intersection in sets of 4. Since all the data in our rect arrays is either
        // maximally negative, or valid from somewhere else in the tree, we can test beyond
        // fNumRects without failing.
        let comp = ComplementRect::new(rect).vals();
        for i in (0..self.num_rects).step_by(4) {
            for lane in i..i + 4 {
                if self.lefts[lane] < comp[0]
                    && self.tops[lane] < comp[1]
                    && self.neg_rights[lane] < comp[2]
                    && self.neg_bots[lane] < comp[3]
                {
                    return true;
                }
            }
        }
        false
    }

    // Port of: src/gpu/graphite/geom/IntersectionTree.cpp#L111-L118 (chrome/m156)
    fn add_non_intersecting(mut self: Box<Self>, rect: Rect) -> Node {
        if self.num_rects == MAX_RECTS_IN_LIST {
            // The new rect doesn't fit. Split our rect list first and then add.
            return self.split().add_non_intersecting(rect);
        }
        self.append_to_list(rect);
        Node::Leaf(self)
    }

    // Port of: src/gpu/graphite/geom/IntersectionTree.cpp#L121-L131 (chrome/m156)
    fn append_to_list(&mut self, rect: Rect) {
        debug_assert!(self.num_rects < MAX_RECTS_IN_LIST);
        let i = self.num_rects;
        self.num_rects += 1;
        // [maxLeft, maxTop, -minRight, -minBot]
        self.splittable_bounds = self.splittable_bounds.max(rect.vals());
        self.rect_vals_sum += rect.vals(); // [sum(left), sum(top), ...]
        let v = rect.vals();
        self.lefts[i] = v[0];
        self.tops[i] = v[1];
        self.neg_rights[i] = v[2];
        self.neg_bots[i] = v[3];
    }

    // Port of: src/gpu/graphite/geom/IntersectionTree.cpp#L133-L135 (chrome/m156)
    fn load_rect(&self, i: usize) -> Rect {
        Rect::from_vals(Float4::new(
            self.lefts[i],
            self.tops[i],
            self.neg_rights[i],
            self.neg_bots[i],
        ))
    }

    // Splits this node with a new LeafNode, then returns a TreeNode that reuses our "this" pointer
    // along with the new node.
    // Port of: src/gpu/graphite/geom/IntersectionTree.cpp#L139-L194 (chrome/m156)
    fn split(mut self: Box<Self>) -> Node {
        // This should only get called when our list is full.
        debug_assert_eq!(self.num_rects, MAX_RECTS_IN_LIST);

        // Since rects cannot overlap, there will always be a split that places at least one pairing
        // of rects on opposite sides. The region:
        //
        //     fSplittableBounds == [maxLeft, maxTop, -minRight, -minBot] == [r, b, -l, -t]
        //
        // Represents the region of splits that guarantee a strict subdivision of our rect list.
        let sb = self.splittable_bounds;
        let splittable_size = sb.xy() + sb.zw(); // == [r-l, b-t]
        let split_type = if splittable_size.x() > splittable_size.y() {
            SplitType::X
        } else {
            SplitType::Y
        };

        let split_coord;
        let (lo_vals, neg_hi_vals) = match split_type {
            SplitType::X => {
                // Split horizontally, at the geometric midpoint if it falls within the splittable
                // bounds.
                let sum = self.rect_vals_sum;
                let sc = (sum.x() - sum.z()) * (0.5_f32 / MAX_RECTS_IN_LIST as f32);
                split_coord = t_pin(sc, -sb.z(), sb.x());
                (self.lefts, self.neg_rights)
            }
            SplitType::Y => {
                // Split vertically, at the geometric midpoint if it falls within the splittable
                // bounds.
                let sum = self.rect_vals_sum;
                let sc = (sum.y() - sum.w()) * (0.5_f32 / MAX_RECTS_IN_LIST as f32);
                split_coord = t_pin(sc, -sb.w(), sb.y());
                (self.tops, self.neg_bots)
            }
        };

        // Split "this", leaving all rects below "splitCoord" in this, and placing all rects above
        // splitCoord in "hiNode". There may be some redundancy between lists, but we made sure to
        // select a split that would leave both lists strictly smaller than the original.
        let mut hi_node = LeafNode::new();
        let num_combined_rects = self.num_rects;
        let neg_split_coord = -split_coord;
        self.pop_all();
        for i in 0..num_combined_rects {
            let rect = self.load_rect(i);
            if lo_vals[i] < split_coord {
                self.append_to_list(rect);
            }
            if neg_hi_vals[i] < neg_split_coord {
                hi_node.append_to_list(rect);
            }
        }

        debug_assert!(0 < self.num_rects && self.num_rects < num_combined_rects);
        debug_assert!(0 < hi_node.num_rects && hi_node.num_rects < num_combined_rects);

        Node::Tree(TreeNode {
            split_type,
            split_coord,
            lo: Box::new(Node::Leaf(self)),
            hi: Box::new(Node::Leaf(Box::new(hi_node))),
        })
    }
}

/// A node of the tree: either a BSP split or a brute-force leaf.
#[derive(Debug)]
enum Node {
    Tree(TreeNode),
    // Boxed so that the enum stays as small as a `TreeNode` (the leaf holds 1 KiB of lanes).
    Leaf(Box<LeafNode>),
}

impl Node {
    // Port of: src/gpu/graphite/geom/IntersectionTree.cpp#L28-L36 (chrome/m156) (virtual dispatch)
    fn intersects(&self, rect: Rect) -> bool {
        match self {
            Node::Tree(t) => t.intersects(rect),
            Node::Leaf(l) => l.intersects(rect),
        }
    }

    // Port of: src/gpu/graphite/geom/IntersectionTree.cpp#L38-L46 (chrome/m156) (virtual dispatch)
    fn add_non_intersecting(self, rect: Rect) -> Node {
        match self {
            Node::Leaf(leaf) => leaf.add_non_intersecting(rect),
            Node::Tree(mut tree) => {
                if tree.get_lo_val(&rect) < tree.split_coord {
                    // Move the child out of its box, add to it, and box the result again.
                    let lo: Node = *tree.lo;
                    tree.lo = Box::new(lo.add_non_intersecting(rect));
                }
                if tree.get_hi_val(&rect) > tree.split_coord {
                    let hi: Node = *tree.hi;
                    tree.hi = Box::new(hi.add_non_intersecting(rect));
                }
                Node::Tree(tree)
            }
        }
    }
}

/// Maintains a collection of non-overlapping rectangles.
///
/// `add()` either adds the given rect to the collection, or returns false if it intersected with a
/// rect already in the collection.
#[doc(alias = "skgpu::graphite::IntersectionTree")]
#[derive(Debug)]
pub struct IntersectionTree {
    // Always `Some` outside of `add()`, which takes the root to rebuild it by value.
    root: Option<Box<Node>>,
}

impl IntersectionTree {
    // Port of: src/gpu/graphite/geom/IntersectionTree.cpp#L206-L211 (chrome/m156)
    /// Creates an empty tree.
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: Some(Box::new(Node::Leaf(Box::new(LeafNode::new())))),
        }
    }

    // Port of: src/gpu/graphite/geom/IntersectionTree.h#L32-L42 (chrome/m156)
    /// Adds `rect` if it does not intersect the rects already in the tree.
    pub fn add(&mut self, rect: Rect) -> bool {
        if rect.is_empty_negative_or_nan() {
            // Empty and undefined rects can simply pass without modifying the tree.
            return true;
        }
        // `root` is only `None` while it is taken here, so the fallback is never used.
        let root = self
            .root
            .take()
            .unwrap_or_else(|| Box::new(Node::Leaf(Box::new(LeafNode::new()))));
        if !root.intersects(rect) {
            self.root = Some(Box::new((*root).add_non_intersecting(rect)));
            return true;
        }
        self.root = Some(root);
        false
    }
}

impl Default for IntersectionTree {
    fn default() -> Self {
        Self::new()
    }
}
