// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/FSRectBench.cpp

//! Full-screen rectangles drawn with solid colors (a raster drawing bench).

use skia_rust_core::color::Color;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;

use crate::def_bench;
use crate::prelude::*;

// Port of: bench/FSRectBench.cpp#L52-L54 (chrome/m156)
const W: i32 = 640;
const H: i32 = 480;
const N: usize = 300;

const MIN_OFFSET: f32 = 0.0;
const MAX_OFFSET: f32 = 100.0;
const OFFSET_RANGE: f32 = MAX_OFFSET - MIN_OFFSET;

/// `class FSRectBench`.
// Port of: bench/FSRectBench.cpp#L19-L61 (chrome/m156)
struct FSRectBench {
    rects: [Rect; N],
    colors: [Color; N],
    init: bool,
}

impl FSRectBench {
    fn new() -> Self {
        Self {
            rects: [Rect::default(); N],
            colors: [Color::default(); N],
            init: false,
        }
    }
}

impl Benchmark for FSRectBench {
    fn name(&self) -> String {
        "fullscreen_rects".to_owned()
    }

    fn on_delayed_setup(&mut self) {
        if !self.init {
            let mut rand = Random::default();
            #[allow(clippy::cast_precision_loss)] // mirrors the implicit int -> float in C++
            let (w, h) = (W as f32, H as f32);
            for i in 0..N {
                self.rects[i].left = -MIN_OFFSET - rand.next_u_scalar1() * OFFSET_RANGE;
                self.rects[i].top = -MIN_OFFSET - rand.next_u_scalar1() * OFFSET_RANGE;
                self.rects[i].right = w + MIN_OFFSET + rand.next_u_scalar1() * OFFSET_RANGE;
                self.rects[i].bottom = h + MIN_OFFSET + rand.next_u_scalar1() * OFFSET_RANGE;
                self.colors[i] = Color::new(rand.next_u() | 0xFF00_0000);
            }
            self.init = true;
        }
    }

    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("FSRectBench is a rendering bench");
        let mut paint = Paint::default();
        for i in 0..loops {
            #[allow(clippy::cast_sign_loss)] // `i % N` of a non-negative loop index
            let index = i as usize % N;
            paint.set_color(self.colors[index]);
            canvas.draw_rect(self.rects[index], &paint);
        }
    }
}

// Port of: bench/FSRectBench.cpp#L63-L63 (chrome/m156)
def_bench!(fs_rect_bench = "FSRectBench()", FSRectBench::new());
