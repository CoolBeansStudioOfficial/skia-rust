// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/IntersectionTreeTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect as SkRect;
use skia_rust_gpu::graphite::geom::intersection_tree::IntersectionTree;
use skia_rust_gpu::graphite::geom::rect::Rect;

use crate::{def_test, reporter_assert};

// Port of: tests/graphite/IntersectionTreeTest.cpp#L14-L31 (chrome/m156)
struct SimpleIntersectionTree {
    rects: Vec<SkRect>,
}

impl SimpleIntersectionTree {
    fn new() -> Self {
        Self { rects: Vec::new() }
    }

    fn add(&mut self, rect: SkRect) -> bool {
        for r in &self.rects {
            if r.intersects(rect) {
                return false;
            }
        }
        self.rects.push(rect);
        true
    }
}

def_test!(skgpu_IntersectionTree, |r| {
    // Port of: tests/graphite/IntersectionTreeTest.cpp#L34-L74 (chrome/m156)
    let mut rand = Random::new(0);
    {
        let mut simple_tree = SimpleIntersectionTree::new();
        let mut tree = IntersectionTree::new();
        for _ in 0..1000 {
            let rect = Rect::xywh(
                rand.next_range_f(0.0, 500.0),
                rand.next_range_f(0.0, 500.0),
                rand.next_range_f(0.0, 70.0),
                rand.next_range_f(0.0, 70.0),
            );
            reporter_assert!(
                r,
                tree.add(rect)
                    == simple_tree.add(SkRect::new(
                        rect.left(),
                        rect.top(),
                        rect.right(),
                        rect.bot()
                    ))
            );
        }
    }
    {
        let mut simple_tree = SimpleIntersectionTree::new();
        let mut tree = IntersectionTree::new();
        for _ in 0..100 {
            let rect = Rect::xywh(
                rand.next_range_f(0.0, 500.0),
                rand.next_range_f(0.0, 500.0),
                rand.next_range_f(0.0, 200.0),
                rand.next_range_f(0.0, 200.0),
            );
            reporter_assert!(
                r,
                tree.add(rect)
                    == simple_tree.add(SkRect::new(
                        rect.left(),
                        rect.top(),
                        rect.right(),
                        rect.bot()
                    ))
            );
        }
    }
    {
        let mut tree = IntersectionTree::new();
        reporter_assert!(r, tree.add(Rect::infinite()));
        reporter_assert!(r, !tree.add(Rect::wh(1.0, 1.0)));
        reporter_assert!(r, !tree.add(Rect::wh(1.0, f32::INFINITY)));
        reporter_assert!(r, tree.add(Rect::wh(0.0, 0.0)));
        reporter_assert!(r, tree.add(Rect::wh(-1.0, 1.0)));
        reporter_assert!(r, tree.add(Rect::wh(1.0, f32::NAN)));
    }
});
