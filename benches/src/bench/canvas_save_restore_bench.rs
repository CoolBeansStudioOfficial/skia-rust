// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/CanvasSaveRestoreBench.cpp

//! `SkCanvas::save`/`concat`/`restore` nested `depth` deep, then a `drawColor` (a raster bench on
//! a 1x1 canvas).

use skia_rust_core::color::Color;
use skia_rust_core::m44::{M44, V3};
use skia_rust_core::size::ISize;

use crate::def_bench;
use crate::prelude::*;

/// `class CanvasSaveRestoreBench`.
// Port of: bench/CanvasSaveRestoreBench.cpp#L6-L44 (chrome/m156)
struct CanvasSaveRestoreBench {
    depth: i32,
    name: String,
}

impl CanvasSaveRestoreBench {
    fn new(depth: i32) -> Self {
        // fName.printf("canvas_save_restore_%d", fDepth);
        Self {
            depth,
            name: format!("canvas_save_restore_{depth}"),
        }
    }
}

impl Benchmark for CanvasSaveRestoreBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::Raster
    }

    // Port of: bench/CanvasSaveRestoreBench.cpp#L19-L19 (chrome/m156)
    fn size(&mut self) -> ISize {
        ISize::new(1, 1)
    }

    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("CanvasSaveRestoreBench is a rendering bench");
        let m = M44::rotate(V3::new(0.0, 0.0, 1.0), 1.0);
        for _ in 0..loops {
            for _ in 0..self.depth {
                canvas.save();
                canvas.concat_44(&m);
            }
            canvas.draw_color(Color::new(0xFF00_FFFF), None);
            for _ in 0..self.depth {
                canvas.restore();
            }
        }
    }
}

// Performance remains roughly constant up to 32 (the number of preallocated save records).
// After that, the cost of additional malloc/free calls starts to be measurable.
// Port of: bench/CanvasSaveRestoreBench.cpp#L46-L46 (chrome/m156)
def_bench!(
    canvas_save_restore_bench_8 = "CanvasSaveRestoreBench(8)",
    CanvasSaveRestoreBench::new(8)
);
// Port of: bench/CanvasSaveRestoreBench.cpp#L47-L47 (chrome/m156)
def_bench!(
    canvas_save_restore_bench_32 = "CanvasSaveRestoreBench(32)",
    CanvasSaveRestoreBench::new(32)
);
// Port of: bench/CanvasSaveRestoreBench.cpp#L48-L48 (chrome/m156)
def_bench!(
    canvas_save_restore_bench_128 = "CanvasSaveRestoreBench(128)",
    CanvasSaveRestoreBench::new(128)
);
// Port of: bench/CanvasSaveRestoreBench.cpp#L49-L49 (chrome/m156)
def_bench!(
    canvas_save_restore_bench_512 = "CanvasSaveRestoreBench(512)",
    CanvasSaveRestoreBench::new(512)
);
