// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/QuickRejectBench.cpp

//! `SkCanvas::quickReject` over a million rectangles, and `SkCanvas::concat` (a rendering bench:
//! both need a canvas, `bench/QuickRejectBench.cpp`).

use skia_rust_core::m44::M44;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;

use crate::def_bench;
use crate::prelude::*;

const N: usize = 1_000_000;

/// `class QuickRejectBench`.
// Port of: bench/QuickRejectBench.cpp#L10-L38 (chrome/m156)
struct QuickRejectBench {
    floats: Vec<f32>,
    ints: Vec<i32>,
}

impl Benchmark for QuickRejectBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend != Backend::NonRendering
    }

    fn name(&self) -> String {
        "quick_reject".to_owned()
    }

    fn on_delayed_setup(&mut self) {
        let mut rand = Random::default();
        for f in &mut self.floats {
            *f = 300.0 * (rand.next_s_scalar1() + 0.5);
        }
    }

    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("QuickRejectBench is a rendering bench");
        for _ in 0..loops {
            for i in 0..N - 4 {
                // *(SkRect*)(fFloats + i): four floats read as a rectangle.
                let r = Rect::new(
                    self.floats[i],
                    self.floats[i + 1],
                    self.floats[i + 2],
                    self.floats[i + 3],
                );
                if canvas.quick_reject_rect(r) {
                    self.ints[i] = 11;
                } else {
                    self.ints[i] = 24;
                }
            }
        }
    }
}

// Port of: bench/QuickRejectBench.cpp#L38-L38 (chrome/m156)
def_bench!(
    quick_reject_bench = "QuickRejectBench",
    QuickRejectBench {
        floats: vec![0.0; N],
        ints: vec![0; N],
    }
);

/// `class ConcatBench`.
// Port of: bench/QuickRejectBench.cpp#L40-L60 (chrome/m156)
struct ConcatBench {
    matrix: Matrix,
}

impl Benchmark for ConcatBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend != Backend::NonRendering
    }

    fn name(&self) -> String {
        "concat".to_owned()
    }

    fn on_delayed_setup(&mut self) {
        // SkRandom r; is unused in the C++.
        self.matrix.set_scale((5.0, 5.0), None);
        self.matrix.set_translate_x(10.0);
        self.matrix.set_translate_y(10.0);
    }

    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("ConcatBench is a rendering bench");
        for _ in 0..loops {
            // canvas->setMatrix(SkMatrix::Scale(3, 3)): SkCanvas converts the SkMatrix to SkM44.
            canvas.set_matrix(&M44::from(&Matrix::scale((3.0, 3.0))));
            canvas.concat(&self.matrix);
        }
    }
}

// Port of: bench/QuickRejectBench.cpp#L62-L62 (chrome/m156)
def_bench!(
    concat_bench = "ConcatBench",
    ConcatBench {
        matrix: Matrix::default(),
    }
);
