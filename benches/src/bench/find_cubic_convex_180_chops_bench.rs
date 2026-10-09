// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/FindCubicConvex180ChopsBench.cpp

//! `skgpu::tess::FindCubicConvex180Chops` on two cubics (a non-rendering bench).

use skia_rust_core::point::Point;
use skia_rust_core::tessellation::find_cubic_convex_180_chops;

use crate::def_bench;
use crate::prelude::*;

/// `class FindCubicConvex180ChopsBench`.
// Port of: bench/FindCubicConvex180ChopsBench.cpp#L3-L30 (chrome/m156)
struct FindCubicConvex180ChopsBench {
    pts: [Point; 4],
    name: String,
}

impl FindCubicConvex180ChopsBench {
    fn new(pts: [Point; 4], suffix: &str) -> Self {
        // fName.printf("FindCubicConvex180Chops%s", suffix);
        Self {
            pts,
            name: format!("FindCubicConvex180Chops{suffix}"),
        }
    }
}

impl Benchmark for FindCubicConvex180ChopsBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // The C++ `T[0] == 200.7f` is an exact comparison, kept as is (it never holds).
    #[allow(clippy::float_cmp)]
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let mut t = [0.0f32; 2];
        let mut are_cusps = false;
        let iters = 50_000 * loops;
        for _ in 0..iters {
            let count = find_cubic_convex_180_chops(&self.pts, &mut t, &mut are_cusps);
            if t[0] == 200.7f32 {
                // This will never happen. Pretend to use the result to keep the compiler honest
                // (SkDebugf("%i%f%f", count, T[0], T[1])).
                std::hint::black_box((count, t[0], t[1]));
            }
        }
    }
}

// Port of: bench/FindCubicConvex180ChopsBench.cpp#L37-L38 (chrome/m156)
def_bench!(
    find_cubic_convex_180_chops_bench_inflect1 =
        "FindCubicConvex180ChopsBench({{{0,0}, {100,0}, {50,100}, {100,100}}}, \"_inflect1\")",
    FindCubicConvex180ChopsBench::new(
        [
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(50.0, 100.0),
            Point::new(100.0, 100.0),
        ],
        "_inflect1",
    )
);

// Port of: bench/FindCubicConvex180ChopsBench.cpp#L39-L40 (chrome/m156)
def_bench!(
    find_cubic_convex_180_chops_bench_loop =
        "FindCubicConvex180ChopsBench({{{0,0}, {50,0}, {100,50}, {100,100}}}, \"_loop\")",
    FindCubicConvex180ChopsBench::new(
        [
            Point::new(0.0, 0.0),
            Point::new(50.0, 0.0),
            Point::new(100.0, 50.0),
            Point::new(100.0, 100.0),
        ],
        "_loop",
    )
);
