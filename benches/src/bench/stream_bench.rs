// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/StreamBench.cpp

//! `SkDynamicMemoryWStream` writes, either `write32` or `write` of byte arrays (a non-rendering
//! bench).

use skia_rust_core::stream::{DynamicMemoryWStream, WStream};

use crate::def_bench;
use crate::prelude::*;

/// `class StreamBench`.
// Port of: bench/StreamBench.cpp#L3-L44 (chrome/m156)
struct StreamBench {
    test_write4: bool,
    name: String,
}

impl StreamBench {
    fn new(test_write4: bool) -> Self {
        // fName.printf("wstream_%d", testWrite4);
        Self {
            test_write4,
            name: format!("wstream_{}", i32::from(test_write4)),
        }
    }
}

impl Benchmark for StreamBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let t3: [u8; 3] = [1, 2, 3];
        let t5: [u8; 5] = [1, 2, 3, 4, 5];
        for _ in 0..loops * 100 {
            let mut stream = DynamicMemoryWStream::new();
            for j in 0..10_000u32 {
                if self.test_write4 {
                    stream.write32(j);
                    stream.write32(j.wrapping_add(j));
                } else {
                    stream.write(&t3);
                    stream.write(&t5);
                }
            }
        }
    }
}

// Port of: bench/StreamBench.cpp#L49-L49 (chrome/m156)
def_bench!(
    stream_bench_false = "StreamBench(false)",
    StreamBench::new(false)
);
// Port of: bench/StreamBench.cpp#L50-L50 (chrome/m156)
def_bench!(
    stream_bench_true = "StreamBench(true)",
    StreamBench::new(true)
);
