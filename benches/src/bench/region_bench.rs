// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/RegionBench.cpp

//! `SkRegion` operations, containment, intersection and `setRects` (`bench/RegionBench.cpp`).

use skia_rust_core::point::IPoint;
use skia_rust_core::random::Random;
use skia_rust_core::rect::IRect;
use skia_rust_core::region::{Op, Region};

use crate::def_bench;
use crate::prelude::*;

/// `static bool union_proc(SkRegion& a, SkRegion& b)`: a `Proc` runs one operation per loop.
// Port of: bench/RegionBench.cpp#L10-L14 (chrome/m156)
type Proc = fn(&mut Region, &mut Region) -> bool;

// Port of: bench/RegionBench.cpp#L10-L14 (chrome/m156)
fn union_proc(a: &mut Region, b: &mut Region) -> bool {
    let mut result = Region::new();
    result.op_region_region(a, b, Op::Union)
}

// Port of: bench/RegionBench.cpp#L16-L20 (chrome/m156)
fn sect_proc(a: &mut Region, b: &mut Region) -> bool {
    let mut result = Region::new();
    result.op_region_region(a, b, Op::Intersect)
}

// Port of: bench/RegionBench.cpp#L22-L26 (chrome/m156)
fn diff_proc(a: &mut Region, b: &mut Region) -> bool {
    let mut result = Region::new();
    result.op_region_region(a, b, Op::Difference)
}

// Port of: bench/RegionBench.cpp#L28-L32 (chrome/m156)
fn diffrect_proc(a: &mut Region, b: &mut Region) -> bool {
    let mut result = Region::new();
    let bounds = *b.bounds();
    result.op_region_rect(a, bounds, Op::Difference)
}

// Port of: bench/RegionBench.cpp#L34-L38 (chrome/m156)
fn diffrectbig_proc(a: &mut Region, _b: &mut Region) -> bool {
    let mut result = Region::new();
    let bounds = *a.bounds();
    result.op_region_rect(a, bounds, Op::Difference)
}

// Port of: bench/RegionBench.cpp#L40-L49 (chrome/m156)
fn containsrect_proc(a: &mut Region, b: &mut Region) -> bool {
    let mut r = *a.bounds();
    r.inset(IPoint::new(r.width() / 4, r.height() / 4));
    let _ = a.contains_rect(r); // (void)a.contains(r)

    r = *b.bounds();
    r.inset(IPoint::new(r.width() / 4, r.height() / 4));
    b.contains_rect(r)
}

// Port of: bench/RegionBench.cpp#L51-L53 (chrome/m156)
fn sectsrgn_proc(a: &mut Region, b: &mut Region) -> bool {
    a.intersects_region(b)
}

// Port of: bench/RegionBench.cpp#L55-L60 (chrome/m156)
fn sectsrect_proc(a: &mut Region, _b: &mut Region) -> bool {
    let mut r = *a.bounds();
    r.inset(IPoint::new(r.width() / 4, r.height() / 4));
    a.intersects_rect(r)
}

// Port of: bench/RegionBench.cpp#L62-L74 (chrome/m156)
fn containsxy_proc(a: &mut Region, _b: &mut Region) -> bool {
    let r = *a.bounds();
    let dx = r.width() / 8;
    let dy = r.height() / 8;
    let mut y = r.top;
    while y < r.bottom {
        let mut x = r.left;
        while x < r.right {
            let _ = a.contains_point(IPoint::new(x, y)); // (void)a.contains(x, y)
            x += dx;
        }
        y += dy;
    }
    true
}

/// `class RegionBench`.
// Port of: bench/RegionBench.cpp#L76-L122 (chrome/m156)
struct RegionBench {
    a: Region,
    b: Region,
    proc_: Proc,
    name: String,
}

const REGION_W: i32 = 1024;
const REGION_H: i32 = 768;

impl RegionBench {
    fn randrect(rand: &mut Random) -> IRect {
        let x = (rand.next_u() % REGION_W as u32).cast_signed();
        let y = (rand.next_u() % REGION_H as u32).cast_signed();
        let w = (rand.next_u() % REGION_W as u32).cast_signed();
        let h = (rand.next_u() % REGION_H as u32).cast_signed();
        IRect::from_xywh(x, y, w >> 1, h >> 1)
    }

    fn new(count: i32, proc_: Proc, name: &str) -> Self {
        let mut rand = Random::default();
        let mut a = Region::new();
        let mut b = Region::new();
        for _ in 0..count {
            a.op_rect(Self::randrect(&mut rand), Op::XOR);
            b.op_rect(Self::randrect(&mut rand), Op::XOR);
        }
        Self {
            a,
            b,
            proc_,
            name: format!("region_{name}_{count}"),
        }
    }
}

impl Benchmark for RegionBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let proc_ = self.proc_;
        for _ in 0..loops {
            proc_(&mut self.a, &mut self.b);
        }
    }
}

/// `class RegionSetRectsBench`.
// Port of: bench/RegionBench.cpp#L124-L172 (chrome/m156)
struct RegionSetRectsBench {
    name: String,
    rects: Vec<IRect>,
}

impl RegionSetRectsBench {
    fn new(count: i32, sorted: bool) -> Self {
        let name = format!(
            "region_setRects_{count}{}",
            if sorted { "_sorted" } else { "" }
        );
        let mut rects = Vec::new();
        if sorted {
            // A grid of non-overlapping rectangles, naturally ordered by Y then X.
            let mut side: i32 = 1;
            while side * side < count {
                side += 1;
            }
            for i in 0..count {
                let x = i % side;
                let y = i / side;
                rects.push(IRect::from_xywh(x * 20, y * 20, 10, 10));
            }
        } else {
            // Random rectangles, likely overlapping and in arbitrary order.
            let mut rand = Random::default();
            for _ in 0..count {
                let x = (rand.next_u() % 1024).cast_signed();
                let y = (rand.next_u() % 768).cast_signed();
                let w = (rand.next_u() % 1024).cast_signed();
                let h = (rand.next_u() % 768).cast_signed();
                rects.push(IRect::from_xywh(x, y, w >> 1, h >> 1));
            }
        }
        Self { name, rects }
    }
}

impl Benchmark for RegionSetRectsBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            let mut rgn = Region::new();
            rgn.set_rects(&self.rects);
        }
    }
}

/// `class RegionCheckerboardBench`.
// Port of: bench/RegionBench.cpp#L185-L212 (chrome/m156)
struct RegionCheckerboardBench;

impl Benchmark for RegionCheckerboardBench {
    fn name(&self) -> String {
        "region_checkerboard".to_owned()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            let mut rgn = Region::new();
            // Simulate a 300x200 grid of tiles where every other tile is missing.
            // This creates 30,000 non-merging rectangles with high coordinate values
            // similar to b/511952524.
            for y in 0..300 {
                for x in 0..200 {
                    if (x + y) % 2 == 0 {
                        rgn.op_rect(IRect::from_xywh(x * 256, y * 256, 256, 256), Op::Union);
                    }
                }
            }
        }
    }
}

const SMALL: i32 = 16;

// Port of: bench/RegionBench.cpp#L174-L182 (chrome/m156)
def_bench!(
    region_bench_union = "RegionBench(SMALL, union_proc, \"union\")",
    RegionBench::new(SMALL, union_proc, "union")
);
def_bench!(
    region_bench_intersect = "RegionBench(SMALL, sect_proc, \"intersect\")",
    RegionBench::new(SMALL, sect_proc, "intersect")
);
def_bench!(
    region_bench_difference = "RegionBench(SMALL, diff_proc, \"difference\")",
    RegionBench::new(SMALL, diff_proc, "difference")
);
def_bench!(
    region_bench_differencerect = "RegionBench(SMALL, diffrect_proc, \"differencerect\")",
    RegionBench::new(SMALL, diffrect_proc, "differencerect")
);
def_bench!(
    region_bench_differencerectbig = "RegionBench(SMALL, diffrectbig_proc, \"differencerectbig\")",
    RegionBench::new(SMALL, diffrectbig_proc, "differencerectbig")
);
def_bench!(
    region_bench_containsrect = "RegionBench(SMALL, containsrect_proc, \"containsrect\")",
    RegionBench::new(SMALL, containsrect_proc, "containsrect")
);
def_bench!(
    region_bench_intersectsrgn = "RegionBench(SMALL, sectsrgn_proc, \"intersectsrgn\")",
    RegionBench::new(SMALL, sectsrgn_proc, "intersectsrgn")
);
def_bench!(
    region_bench_intersectsrect = "RegionBench(SMALL, sectsrect_proc, \"intersectsrect\")",
    RegionBench::new(SMALL, sectsrect_proc, "intersectsrect")
);
def_bench!(
    region_bench_containsxy = "RegionBench(SMALL, containsxy_proc, \"containsxy\")",
    RegionBench::new(SMALL, containsxy_proc, "containsxy")
);

// Port of: bench/RegionBench.cpp#L184-L191 (chrome/m156)
def_bench!(
    region_set_rects_50 = "RegionSetRectsBench(50, false)",
    RegionSetRectsBench::new(50, false)
);
def_bench!(
    region_set_rects_500 = "RegionSetRectsBench(500, false)",
    RegionSetRectsBench::new(500, false)
);
def_bench!(
    region_set_rects_2500 = "RegionSetRectsBench(2500, false)",
    RegionSetRectsBench::new(2500, false)
);
def_bench!(
    region_set_rects_10000 = "RegionSetRectsBench(10000, false)",
    RegionSetRectsBench::new(10000, false)
);
def_bench!(
    region_set_rects_50_sorted = "RegionSetRectsBench(50, true)",
    RegionSetRectsBench::new(50, true)
);
def_bench!(
    region_set_rects_500_sorted = "RegionSetRectsBench(500, true)",
    RegionSetRectsBench::new(500, true)
);
def_bench!(
    region_set_rects_2500_sorted = "RegionSetRectsBench(2500, true)",
    RegionSetRectsBench::new(2500, true)
);
def_bench!(
    region_set_rects_10000_sorted = "RegionSetRectsBench(10000, true)",
    RegionSetRectsBench::new(10000, true)
);

// Port of: bench/RegionBench.cpp#L212-L212 (chrome/m156)
def_bench!(
    region_checkerboard_bench = "RegionCheckerboardBench()",
    RegionCheckerboardBench
);
