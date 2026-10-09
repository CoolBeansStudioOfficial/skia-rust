// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/RTreeBench.cpp

//! Building and querying an `SkRTree` (non-rendering benches: `bench/RTreeBench.cpp`).

use skia_rust_core::r_tree::RTree;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::int_to_scalar;

use crate::def_bench;
use crate::prelude::*;

// Port of: bench/RTreeBench.cpp#L14-L19 (chrome/m156)
// confine rectangles to a smallish area, so queries generally hit something, and overlap occurs:
const GENERATE_EXTENTS: f32 = 1000.0;
const NUM_BUILD_RECTS: usize = 500;
const NUM_QUERY_RECTS: usize = 5000;
const GRID_WIDTH: i32 = 100;

/// `typedef SkRect (*MakeRectProc)(SkRandom&, int, int)`.
type MakeRectProc = fn(&mut Random, i32, i32) -> Rect;

/// `class RTreeBuildBench`: time how long it takes to build an R-Tree.
// Port of: bench/RTreeBench.cpp#L21-L50 (chrome/m156)
struct RTreeBuildBench {
    name: String,
    proc_: MakeRectProc,
}

impl RTreeBuildBench {
    fn new(name: &str, proc_: MakeRectProc) -> Self {
        // fName.printf("rtree_%s_build", name);
        Self {
            name: format!("rtree_{name}_build"),
            proc_,
        }
    }
}

impl Benchmark for RTreeBuildBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let mut rand = Random::default();
        let mut rects = vec![Rect::default(); NUM_BUILD_RECTS];
        for (i, rect) in rects.iter_mut().enumerate() {
            // The C++ loop index and count are ints.
            let index = i32::try_from(i).expect("loop index fits an int");
            let num = i32::try_from(NUM_BUILD_RECTS).expect("count fits an int");
            *rect = (self.proc_)(&mut rand, index, num);
        }

        for _ in 0..loops {
            let tree = RTree::new();
            tree.insert(&rects);
        }
    }
}

/// `class RTreeQueryBench`: time how long it takes to perform queries on an R-Tree.
// Port of: bench/RTreeBench.cpp#L52-L94 (chrome/m156)
struct RTreeQueryBench {
    tree: RTree,
    proc_: MakeRectProc,
    name: String,
}

impl RTreeQueryBench {
    fn new(name: &str, proc_: MakeRectProc) -> Self {
        // fName.printf("rtree_%s_query", name);
        Self {
            tree: RTree::new(),
            proc_,
            name: format!("rtree_{name}_query"),
        }
    }
}

impl Benchmark for RTreeQueryBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_delayed_setup(&mut self) {
        let mut rand = Random::default();
        let mut rects = vec![Rect::default(); NUM_QUERY_RECTS];
        for (i, rect) in rects.iter_mut().enumerate() {
            // The C++ loop index and count are ints.
            let index = i32::try_from(i).expect("loop index fits an int");
            let num = i32::try_from(NUM_QUERY_RECTS).expect("count fits an int");
            *rect = (self.proc_)(&mut rand, index, num);
        }
        self.tree.insert(&rects);
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let mut rand = Random::default();
        for _ in 0..loops {
            let mut hits: Vec<usize> = Vec::new();
            let mut query = Rect::default();
            query.left = rand.next_range_f(0.0, GENERATE_EXTENTS);
            query.top = rand.next_range_f(0.0, GENERATE_EXTENTS);
            query.right = query.left + 1.0 + rand.next_range_f(0.0, GENERATE_EXTENTS / 2.0);
            query.bottom = query.top + 1.0 + rand.next_range_f(0.0, GENERATE_EXTENTS / 2.0);
            self.tree.search(&query, &mut hits);
        }
    }
}

// Port of: bench/RTreeBench.cpp#L96-L101 (chrome/m156)
fn make_xy_ordered_rects(rand: &mut Random, index: i32, _num_rects: i32) -> Rect {
    let left = int_to_scalar(index % GRID_WIDTH);
    let top = int_to_scalar(index / GRID_WIDTH);
    let right = left + 1.0 + rand.next_range_f(0.0, GENERATE_EXTENTS / 3.0);
    let bottom = top + 1.0 + rand.next_range_f(0.0, GENERATE_EXTENTS / 3.0);
    Rect::from_ltrb(left, top, right, bottom)
}

// Port of: bench/RTreeBench.cpp#L102-L108 (chrome/m156)
fn make_yx_ordered_rects(rand: &mut Random, index: i32, _num_rects: i32) -> Rect {
    let left = int_to_scalar(index / GRID_WIDTH);
    let top = int_to_scalar(index % GRID_WIDTH);
    let right = left + 1.0 + rand.next_range_f(0.0, GENERATE_EXTENTS / 3.0);
    let bottom = top + 1.0 + rand.next_range_f(0.0, GENERATE_EXTENTS / 3.0);
    Rect::from_ltrb(left, top, right, bottom)
}

// Port of: bench/RTreeBench.cpp#L110-L116 (chrome/m156)
fn make_random_rects(rand: &mut Random, _index: i32, _num_rects: i32) -> Rect {
    let left = rand.next_range_f(0.0, GENERATE_EXTENTS);
    let top = rand.next_range_f(0.0, GENERATE_EXTENTS);
    let right = left + 1.0 + rand.next_range_f(0.0, GENERATE_EXTENTS / 5.0);
    let bottom = top + 1.0 + rand.next_range_f(0.0, GENERATE_EXTENTS / 5.0);
    Rect::from_ltrb(left, top, right, bottom)
}

// Port of: bench/RTreeBench.cpp#L118-L121 (chrome/m156)
fn make_concentric_rects(_rand: &mut Random, index: i32, _num_rects: i32) -> Rect {
    // SkRect::MakeWH(SkIntToScalar(index+1), SkIntToScalar(index+1))
    Rect::from_wh(int_to_scalar(index + 1), int_to_scalar(index + 1))
}

// Port of: bench/RTreeBench.cpp#L132-L140 (chrome/m156)
def_bench!(
    rtree_build_xy = "RTreeBuildBench(\"XY\", &make_XYordered_rects)",
    RTreeBuildBench::new("XY", make_xy_ordered_rects)
);
def_bench!(
    rtree_build_yx = "RTreeBuildBench(\"YX\", &make_YXordered_rects)",
    RTreeBuildBench::new("YX", make_yx_ordered_rects)
);
def_bench!(
    rtree_build_random = "RTreeBuildBench(\"random\", &make_random_rects)",
    RTreeBuildBench::new("random", make_random_rects)
);
def_bench!(
    rtree_build_concentric = "RTreeBuildBench(\"concentric\", &make_concentric_rects)",
    RTreeBuildBench::new("concentric", make_concentric_rects)
);
def_bench!(
    rtree_query_xy = "RTreeQueryBench(\"XY\", &make_XYordered_rects)",
    RTreeQueryBench::new("XY", make_xy_ordered_rects)
);
def_bench!(
    rtree_query_yx = "RTreeQueryBench(\"YX\", &make_YXordered_rects)",
    RTreeQueryBench::new("YX", make_yx_ordered_rects)
);
def_bench!(
    rtree_query_random = "RTreeQueryBench(\"random\", &make_random_rects)",
    RTreeQueryBench::new("random", make_random_rects)
);
def_bench!(
    rtree_query_concentric = "RTreeQueryBench(\"concentric\", &make_concentric_rects)",
    RTreeQueryBench::new("concentric", make_concentric_rects)
);
