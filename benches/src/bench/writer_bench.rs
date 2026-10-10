// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/WriterBench.cpp

//! `SkWriter32::writeString` with every prefix of the alphabet (a non-rendering bench).

use skia_rust_core::write_buffer::Writer32;

use crate::def_bench;
use crate::prelude::*;

const G_STR: &str = "abcdefghimjklmnopqrstuvwxyz";

/// `class WriterBench`.
// Port of: bench/WriterBench.cpp#L7-L35 (chrome/m156)
struct WriterBench;

impl Benchmark for WriterBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        "writer".to_owned()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let mut writer = Writer32::new();
        for _ in 0..loops {
            // for (size_t j = 0; j <= gLen; j++) writer.writeString(gStr, j);
            for j in 0..=G_STR.len() {
                // writeString(str, len) writes the first `len` bytes of `str`.
                writer.write_string(Some(&G_STR[..j]));
            }
        }
    }
}

// Port of: bench/WriterBench.cpp#L40-L40 (chrome/m156)
def_bench!(writer_bench = "WriterBench()", WriterBench);
