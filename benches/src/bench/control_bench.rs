// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/ControlBench.cpp

//! A constant-time baseline to control for thermal or other throttling (non-rendering).

use crate::def_bench;
use crate::prelude::*;

/// `struct ControlBench`.
// Port of: bench/ControlBench.cpp#L9-L21 (chrome/m156)
struct ControlBench;

impl Benchmark for ControlBench {
    fn name(&self) -> String {
        "control".to_owned()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        // Nothing terribly useful: force a memory read, a memory write, and some math.
        // The C++ `volatile uint32_t rand` is a sink: each store is kept, so the sink is the
        // black_box on the value written.
        let mut rand: u32 = 0;
        for _ in 0..1000 * loops {
            // uint32_t arithmetic wraps in C++.
            let val = rand.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            rand = std::hint::black_box(val);
        }
    }
}

// Port of: bench/ControlBench.cpp#L24-L24 (chrome/m156)
def_bench!(control_bench = "ControlBench", ControlBench);
