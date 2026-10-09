// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/MathBench.cpp

//! Non-rendering math benchmarks (the first ones of `bench/MathBench.cpp`).

use skia_rust_core::floating_point::{float_floor2int, float_floor2int_no_saturate, float_rsqrt};
use skia_rust_core::random::Random;

use crate::def_bench;
use crate::prelude::*;

// Port of: bench/MathBench.cpp#L21-L23 (chrome/m156)
const BUFFER: usize = 100;

/// What the `MathBench` subclasses override: `performTest`.
// Port of: bench/MathBench.cpp#L35-L37 (chrome/m156)
trait PerformTest {
    fn perform_test(&self, dst: &mut [f32; BUFFER], src: &[f32; BUFFER], count: usize);

    /// `mulLoopCount()`.
    fn mul_loop_count(&self) -> i32 {
        1
    }
}

/// `class MathBench`: the base class, generic over the `performTest` of its subclass.
// Port of: bench/MathBench.cpp#L19-L59 (chrome/m156)
struct MathBench<T> {
    name: String,
    src: [f32; BUFFER],
    dst: [f32; BUFFER],
    test: T,
}

impl<T: PerformTest> MathBench<T> {
    fn new(name: &str, test: T) -> Self {
        let mut rand = Random::default();
        let mut src = [0.0; BUFFER];
        for s in &mut src {
            *s = rand.next_s_scalar1();
        }
        Self {
            name: format!("math_{name}"),
            src,
            dst: [0.0; BUFFER],
            test,
        }
    }
}

impl<T: PerformTest> Benchmark for MathBench<T> {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let n = loops * self.test.mul_loop_count();
        for _ in 0..n {
            self.test.perform_test(&mut self.dst, &self.src, BUFFER);
        }
    }
}

// ---------------------------------------------------------------------------------------------

struct NoOp;

impl PerformTest for NoOp {
    fn perform_test(&self, dst: &mut [f32; BUFFER], src: &[f32; BUFFER], count: usize) {
        for i in 0..count {
            dst[i] = src[i] + 1.0;
        }
    }
}

/// `class NoOpMathBench`.
// Port of: bench/MathBench.cpp#L81-L92 (chrome/m156)
fn new_no_op_math_bench() -> MathBench<NoOp> {
    MathBench::new("noOp", NoOp)
}

struct SlowISqrt;

impl PerformTest for SlowISqrt {
    fn perform_test(&self, dst: &mut [f32; BUFFER], src: &[f32; BUFFER], count: usize) {
        for i in 0..count {
            dst[i] = 1.0f32 / src[i].sqrt();
        }
    }
}

/// `class SlowISqrtMathBench`.
// Port of: bench/MathBench.cpp#L94-L105 (chrome/m156)
fn new_slow_isqrt_math_bench() -> MathBench<SlowISqrt> {
    MathBench::new("slowIsqrt", SlowISqrt)
}

struct FastISqrt;

impl PerformTest for FastISqrt {
    fn perform_test(&self, dst: &mut [f32; BUFFER], src: &[f32; BUFFER], count: usize) {
        for i in 0..count {
            dst[i] = float_rsqrt(src[i]);
        }
    }
}

/// `class FastISqrtMathBench`.
// Port of: bench/MathBench.cpp#L107-L118 (chrome/m156)
fn new_fast_isqrt_math_bench() -> MathBench<FastISqrt> {
    MathBench::new("fastIsqrt", FastISqrt)
}

// ---------------------------------------------------------------------------------------------

// Port of: bench/MathBench.cpp#L377-L379 (chrome/m156)
const ARRAY: usize = 1000;

/// `class Floor2IntBench`.
// Port of: bench/MathBench.cpp#L376-L433 (chrome/m156)
struct Floor2IntBench {
    data: [f32; ARRAY],
    sat: bool,
    name: &'static str,
}

impl Floor2IntBench {
    fn new(sat: bool) -> Self {
        let mut rand = Random::default();
        let mut data = [0.0; ARRAY];
        for d in &mut data {
            *d = f32::from_bits(rand.next_u()); // SkBits2Float
        }
        Self {
            data,
            sat,
            name: if sat {
                "floor2int_sat"
            } else {
                "floor2int_undef"
            },
        }
    }

    // These exist to try to stop the compiler from detecting what we doing, and throwing
    // parts away (or knowing exactly how big the loop counts are). They are virtual in C++;
    // `black_box` plays that part here, in the same place.
    fn process(accum: u32) {
        std::hint::black_box(accum);
    }

    fn count() -> usize {
        std::hint::black_box(ARRAY)
    }
}

impl Benchmark for Floor2IntBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    #[allow(clippy::cast_sign_loss)] // mirrors `unsigned accum += int` (wraps)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        // used unsigned to avoid undefined behavior if/when the += might overflow
        let mut accum: u32 = 0;

        for _ in 0..loops {
            let n = Self::count();
            if self.sat {
                for i in 0..n {
                    accum = accum.wrapping_add(float_floor2int(self.data[i]) as u32);
                }
            } else {
                for i in 0..n {
                    accum = accum.wrapping_add(float_floor2int_no_saturate(self.data[i]) as u32);
                }
            }
            Self::process(accum);
        }
    }

    fn name(&self) -> String {
        self.name.to_owned()
    }
}

// Port of: bench/MathBench.cpp#L357-L359 (chrome/m156)
def_bench!(no_op_math_bench = "NoOpMathBench()", new_no_op_math_bench());
def_bench!(
    slow_isqrt_math_bench = "SlowISqrtMathBench()",
    new_slow_isqrt_math_bench()
);
def_bench!(
    fast_isqrt_math_bench = "FastISqrtMathBench()",
    new_fast_isqrt_math_bench()
);

// Port of: bench/MathBench.cpp#L434-L435 (chrome/m156)
def_bench!(
    floor2int_bench_false = "Floor2IntBench(false)",
    Floor2IntBench::new(false)
);
def_bench!(
    floor2int_bench_true = "Floor2IntBench(true)",
    Floor2IntBench::new(true)
);
