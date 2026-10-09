// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/ChecksumBench.cpp

//! `ComputeChecksumBench` with the wyhash checksum (`SkChecksum::Hash32`) over 64 KiB of random
//! bytes, in blocks of 4 to 1024 bytes (non-rendering). The MD5 variant needs `SkMD5`, which is
//! not ported, so it is not registered.

use skia_rust_core::checksum::hash32;
use skia_rust_core::random::Random;

use crate::def_bench_set;
use crate::prelude::*;

/// `ComputeChecksumBench::kBufferSize`.
// Port of: bench/ChecksumBench.cpp#L22-L27 (chrome/m156)
const BUFFER_SIZE: usize = 64 * 1024;

/// `class ComputeChecksumBench` with `kWyhash_ChecksumType`.
// Port of: bench/ChecksumBench.cpp#L22-L70 (chrome/m156)
struct ComputeChecksumBench {
    block_size: usize,
    /// `fName`.
    name: String,
    /// `fData`: filled in `onPreDraw` and released in `onPostDraw`.
    data: Vec<u8>,
}

impl ComputeChecksumBench {
    // Port of: bench/ChecksumBench.cpp#L29-L38 (chrome/m156)
    fn new(block_size: usize) -> Self {
        // SkASSERT(blockSize <= kBufferSize);
        debug_assert!(block_size <= BUFFER_SIZE);
        Self {
            // fName = "compute_wyhash"; fName.appendf("_%d", blockSize);
            name: format!("compute_wyhash_{block_size}"),
            block_size,
            data: Vec::new(),
        }
    }
}

impl Benchmark for ComputeChecksumBench {
    // Port of: bench/ChecksumBench.cpp#L40 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/ChecksumBench.cpp#L43-L49 (chrome/m156)
    fn on_pre_draw(&mut self, _canvas: Option<&Canvas>) {
        // fData.reset(new uint8_t[kBufferSize]);
        self.data = Vec::with_capacity(BUFFER_SIZE);
        // SkRandom rand;
        let mut rand = Random::default();
        for _ in 0..BUFFER_SIZE {
            // fData[i] = rand.nextBits(8);
            self.data
                .push(u8::try_from(rand.next_bits(8)).expect("8 bits"));
        }
    }

    // Port of: bench/ChecksumBench.cpp#L50-L53 (chrome/m156)
    fn on_post_draw(&mut self, _canvas: Option<&Canvas>) {
        // fData.reset();
        self.data = Vec::new();
    }

    // Port of: bench/ChecksumBench.cpp#L54-L72 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        // volatile uint32_t result = 0; (its stores are kept with black_box)
        // const size_t blockCount = kBufferSize / fBlockSize;
        let block_count = BUFFER_SIZE / self.block_size;
        // const uint8_t* bufEnd = fData.get() + (blockCount * fBlockSize);
        let buf_end = block_count * self.block_size;
        for _ in 0..loops {
            // for (const uint8_t* buf = fData.get(); buf < bufEnd; buf += fBlockSize)
            for start in (0..buf_end).step_by(self.block_size) {
                // result = SkChecksum::Hash32(buf, fBlockSize);
                let block = &self.data[start..start + self.block_size];
                std::hint::black_box(hash32(block, 0));
            }
        }
    }
}

/// `DEF_CHECKSUM_BENCH(kWyhash_ChecksumType)`: eight benchmarks, one per block size.
// Port of: bench/ChecksumBench.cpp#L74-L81 (chrome/m156)
fn wyhash_benches() -> Vec<Box<dyn Benchmark>> {
    [4, 8, 15, 16, 31, 32, 96, 1024]
        .into_iter()
        .map(|block_size| Box::new(ComputeChecksumBench::new(block_size)) as Box<dyn Benchmark>)
        .collect()
}

// Port of: bench/ChecksumBench.cpp#L81 (chrome/m156)
def_bench_set!(checksum_wyhash = "kWyhash_ChecksumType", wyhash_benches());
