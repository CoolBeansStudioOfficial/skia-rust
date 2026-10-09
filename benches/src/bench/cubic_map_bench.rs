// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/CubicMapBench.cpp

//! `SkCubicMap::computeYFromX` over a sweep of `x` (`bench/CubicMapBench.cpp`).

use skia_rust_core::cubic_map::CubicMap;
use skia_rust_core::point::Point;

use crate::def_bench;
use crate::prelude::*;

/// `class CubicMapBench`.
// Port of: bench/CubicMapBench.cpp#L10-L41 (chrome/m156)
struct CubicMapBench {
    cmap: CubicMap,
    name: String,
}

impl CubicMapBench {
    fn new(p1: Point, p2: Point) -> Self {
        // fName.printf("cubicmap_%g_%g_%g_%g", ...). The names only use 0 and 1, which `%g` and
        // Rust's `Display` both print as `0` and `1`.
        Self {
            cmap: CubicMap::new(p1, p2),
            name: format!("cubicmap_{}_{}_{}_{}", p1.x, p1.y, p2.x, p2.y),
        }
    }
}

impl Benchmark for CubicMapBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..100 {
            for _ in 0..loops {
                let mut x: f32 = 0.0;
                while x <= 1.0 {
                    let _ = self.cmap.compute_y_from_x(x);
                    x += 1.0_f32 / 512.0;
                }
            }
        }
    }
}

// Port of: bench/CubicMapBench.cpp#L43-L53 (chrome/m156)
def_bench!(
    cubic_map_bench_1_0_0_0 = "CubicMapBench({1, 0}, {0,0})",
    CubicMapBench::new(Point::new(1.0, 0.0), Point::new(0.0, 0.0))
);
def_bench!(
    cubic_map_bench_1_0_0_1 = "CubicMapBench({1, 0}, {0,1})",
    CubicMapBench::new(Point::new(1.0, 0.0), Point::new(0.0, 1.0))
);
def_bench!(
    cubic_map_bench_1_0_1_0 = "CubicMapBench({1, 0}, {1,0})",
    CubicMapBench::new(Point::new(1.0, 0.0), Point::new(1.0, 0.0))
);
def_bench!(
    cubic_map_bench_1_0_1_1 = "CubicMapBench({1, 0}, {1,1})",
    CubicMapBench::new(Point::new(1.0, 0.0), Point::new(1.0, 1.0))
);
def_bench!(
    cubic_map_bench_0_1_0_0 = "CubicMapBench({0, 1}, {0,0})",
    CubicMapBench::new(Point::new(0.0, 1.0), Point::new(0.0, 0.0))
);
def_bench!(
    cubic_map_bench_0_1_0_1 = "CubicMapBench({0, 1}, {0,1})",
    CubicMapBench::new(Point::new(0.0, 1.0), Point::new(0.0, 1.0))
);
def_bench!(
    cubic_map_bench_0_1_1_0 = "CubicMapBench({0, 1}, {1,0})",
    CubicMapBench::new(Point::new(0.0, 1.0), Point::new(1.0, 0.0))
);
def_bench!(
    cubic_map_bench_0_1_1_1 = "CubicMapBench({0, 1}, {1,1})",
    CubicMapBench::new(Point::new(0.0, 1.0), Point::new(1.0, 1.0))
);
def_bench!(
    cubic_map_bench_0_0_1_1 = "CubicMapBench({0, 0}, {1,1})",
    CubicMapBench::new(Point::new(0.0, 0.0), Point::new(1.0, 1.0))
);
def_bench!(
    cubic_map_bench_1_1_0_0 = "CubicMapBench({1, 1}, {0,0})",
    CubicMapBench::new(Point::new(1.0, 1.0), Point::new(0.0, 0.0))
);
