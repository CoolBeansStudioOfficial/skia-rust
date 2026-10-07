// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RTreeTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::bbh_factory::BBoxHierarchy;
use skia_rust_core::r_tree::RTree;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;

const NUM_RECTS: usize = 200;
const NUM_ITERATIONS: usize = 100;
const NUM_QUERIES: usize = 50;

// Port of: tests/RTreeTest.cpp#L26-L37 (chrome/m156)
fn random_rect(rand: &mut Random) -> Rect {
    let mut rect = Rect::new(0.0, 0.0, 0.0, 0.0);
    while rect.is_empty() {
        rect.left = rand.next_range_f(0.0, 1000.0);
        rect.right = rand.next_range_f(0.0, 1000.0);
        rect.top = rand.next_range_f(0.0, 1000.0);
        rect.bottom = rand.next_range_f(0.0, 1000.0);
        rect.sort();
    }
    rect
}

// Port of: tests/RTreeTest.cpp#L39-L57 (chrome/m156)
fn verify_query(query: Rect, rects: &[Rect], found: &[usize]) -> bool {
    let mut expected: Vec<usize> = Vec::new();
    // manually intersect with every rectangle
    for (i, rect) in rects.iter().enumerate().take(NUM_RECTS) {
        if Rect::intersects2(query, rect) {
            expected.push(i);
        }
    }

    if expected.len() != found.len() {
        return false;
    }
    if expected.is_empty() {
        return true;
    }
    found == expected
}

// Port of: tests/RTreeTest.cpp#L59-L68 (chrome/m156)
fn run_queries(reporter: &mut Reporter, rand: &mut Random, rects: &[Rect], tree: &RTree) {
    for _ in 0..NUM_QUERIES {
        let mut hits: Vec<usize> = Vec::new();
        let query = random_rect(rand);
        tree.search(&query, &mut hits);
        reporter_assert!(reporter, verify_query(query, rects, &hits));
    }
}

// Port of: tests/RTreeTest.cpp#L70-L114 (chrome/m156)
def_test!(RTree, |reporter| {
    let mut expected_depth_min: i32 = -1;
    let mut tmp = i32::try_from(NUM_RECTS).unwrap();
    while tmp > 0 {
        // static_cast<int>(pow(double(kMaxChildren), double(expectedDepthMin + 1)))
        #[allow(clippy::cast_possible_truncation)] // mirrors the static_cast<int> of pow()
        let term = f64::from(i32::try_from(RTree::MAX_CHILDREN).unwrap())
            .powf(f64::from(expected_depth_min + 1)) as i32;
        tmp -= term;
        expected_depth_min += 1;
    }

    let mut expected_depth_max: i32 = -1;
    tmp = i32::try_from(NUM_RECTS).unwrap();
    while tmp > 0 {
        #[allow(clippy::cast_possible_truncation)] // mirrors the static_cast<int> of pow()
        let term = f64::from(i32::try_from(RTree::MIN_CHILDREN).unwrap())
            .powf(f64::from(expected_depth_max + 1)) as i32;
        tmp -= term;
        expected_depth_max += 1;
    }

    let mut rand = Random::default();
    let mut rects = vec![Rect::new_empty(); NUM_RECTS];
    for _ in 0..NUM_ITERATIONS {
        let rtree = RTree::new();
        reporter_assert!(reporter, 0 == rtree.get_count());

        for rect in &mut rects {
            *rect = random_rect(&mut rand);
        }

        rtree.insert(&rects);

        run_queries(reporter, &mut rand, &rects, &rtree);
        reporter_assert!(reporter, NUM_RECTS == rtree.get_count());
        let depth = i32::try_from(rtree.get_depth()).unwrap();
        reporter_assert!(
            reporter,
            expected_depth_min <= depth && expected_depth_max >= depth
        );
    }
});
