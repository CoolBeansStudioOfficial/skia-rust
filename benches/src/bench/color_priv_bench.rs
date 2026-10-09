// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/ColorPrivBench.cpp

//! The four `SkFourByteInterp*` variants over random premultiplied colors and every scale (a
//! non-rendering bench).

use skia_rust_core::color::{PMColor, pre_multiply_color};
use skia_rust_core::color_data::{
    fast_four_byte_interp, fast_four_byte_interp256, four_byte_interp, four_byte_interp256,
};
use skia_rust_core::random::Random;

use crate::def_bench;
use crate::prelude::*;

/// `static const int kInputs = 10;  // Arbitrary.`
const K_INPUTS: usize = 10;

/// `template <bool kFast, bool kScale> class FourByteInterpBench`.
// Port of: bench/ColorPrivBench.cpp#L5-L56 (chrome/m156)
struct FourByteInterpBench<const FAST: bool, const SCALE: bool> {
    srcs: [PMColor; K_INPUTS],
    dsts: [PMColor; K_INPUTS],
    /// `std::array<unsigned int, 257>`: space for [0, 256].
    scales: [u32; 257],
}

impl<const FAST: bool, const SCALE: bool> FourByteInterpBench<FAST, SCALE> {
    fn new() -> Self {
        Self {
            srcs: [0; K_INPUTS],
            dsts: [0; K_INPUTS],
            scales: [0; 257],
        }
    }
}

impl<const FAST: bool, const SCALE: bool> Benchmark for FourByteInterpBench<FAST, SCALE> {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        // fName.set("four_byte_interp"); append(kFast ? "_fast" : "_slow");
        // append(kScale ? "_255" : "_256");
        let speed = if FAST { "_fast" } else { "_slow" };
        let range = if SCALE { "_255" } else { "_256" };
        format!("four_byte_interp{speed}{range}")
    }

    fn on_delayed_setup(&mut self) {
        // A handful of random srcs and dsts.
        let mut rand = Random::default();
        for i in 0..K_INPUTS {
            self.srcs[i] = pre_multiply_color(rand.next_u());
            self.dsts[i] = pre_multiply_color(rand.next_u());
        }

        // We'll exhaustively test all scales instead of using random numbers.
        for (i, scale) in self.scales.iter_mut().enumerate() {
            *scale = u32::try_from(i).unwrap_or(u32::MAX);
        }
        // We'll just do 255 twice if we're limited to [0,255].
        if SCALE {
            self.scales[256] = 255;
        }
    }

    // `scale` is at most 256 here, so the `as i32` cast is exact.
    #[allow(clippy::cast_possible_wrap)]
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            for i in 0..K_INPUTS {
                for j in 0..=256usize {
                    // The C++ comment about volatile loads applies here too: the srcs and dsts are
                    // read inside the scale loop.
                    let src = self.srcs[i];
                    let dst = self.dsts[i];

                    let scale = self.scales[j];

                    // We xor results of FourByteInterp into junk to make sure the function runs.
                    let junk = if FAST && SCALE {
                        fast_four_byte_interp(src, dst, scale)
                    } else if FAST {
                        fast_four_byte_interp256(src, dst, scale)
                    } else if SCALE {
                        four_byte_interp(src, dst, scale)
                    } else {
                        four_byte_interp256(src, dst, scale as i32)
                    };
                    // The volatile store to `junk` of the C++: a sink the optimizer keeps.
                    std::hint::black_box(junk);
                }
            }
        }
    }
}

// Port of: bench/ColorPrivBench.cpp#L85-L85 (chrome/m156)
def_bench!(
    four_byte_interp_bench_true_true = "(new FourByteInterpBench<true COMMA true>)",
    FourByteInterpBench::<true, true>::new()
);
// Port of: bench/ColorPrivBench.cpp#L86-L86 (chrome/m156)
def_bench!(
    four_byte_interp_bench_true_false = "(new FourByteInterpBench<true COMMA false>)",
    FourByteInterpBench::<true, false>::new()
);
// Port of: bench/ColorPrivBench.cpp#L87-L87 (chrome/m156)
def_bench!(
    four_byte_interp_bench_false_true = "(new FourByteInterpBench<false COMMA true>)",
    FourByteInterpBench::<false, true>::new()
);
// Port of: bench/ColorPrivBench.cpp#L88-L88 (chrome/m156)
def_bench!(
    four_byte_interp_bench_false_false = "(new FourByteInterpBench<false COMMA false>)",
    FourByteInterpBench::<false, false>::new()
);
