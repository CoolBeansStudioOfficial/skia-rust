// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/MathBench.cpp

//! Non-rendering math benchmarks (the first ones of `bench/MathBench.cpp`).

use skia_rust_core::color_priv::alpha_mul_q;
use skia_rust_core::fixed::{Fixed, float_to_fixed};
use skia_rust_core::floating_point::{float_floor2int, float_floor2int_no_saturate, float_rsqrt};
use skia_rust_core::point::Vector;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;

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

/// `class QMul32Bench : public MathBenchU32`. `MathBenchU32::performTest` reinterprets the float
/// buffers as `uint32_t`; `to_bits`/`from_bits` are that reinterpretation, element by element,
/// which compiles to register moves.
// Port of: bench/MathBench.cpp#L120-L135 (chrome/m156)
struct QMul32;

impl PerformTest for QMul32 {
    fn perform_test(&self, dst: &mut [f32; BUFFER], src: &[f32; BUFFER], count: usize) {
        for i in 0..count {
            // dst[i] = SkAlphaMulQ(src[i], (uint8_t)i);
            #[allow(clippy::cast_possible_truncation)] // (uint8_t)i
            let scale = i as u8;
            dst[i] = f32::from_bits(alpha_mul_q(src[i].to_bits(), u32::from(scale)));
        }
    }
}

/// `class QMul32Bench`.
// Port of: bench/MathBench.cpp#L120-L135 (chrome/m156)
fn new_qmul32_bench() -> MathBench<QMul32> {
    MathBench::new("qmul32", QMul32)
}

// ---------------------------------------------------------------------------------------------

/// `isFinite_int`: the exponent field is not all ones.
// Port of: bench/MathBench.cpp#L136-L140 (chrome/m156)
fn is_finite_int(x: f32) -> bool {
    let bits = x.to_bits(); // SkFloat2Bits: unsigned for the shifts
    let exponent = (bits << 1) >> 24;
    exponent != 0xFF
}

/// `isFinite_mulzero`.
// Port of: bench/MathBench.cpp#L142-L146 (chrome/m156)
#[allow(clippy::eq_op, clippy::float_cmp)] // `y == y` is the NaN test of the C++
fn is_finite_mulzero(x: f32) -> bool {
    let y = x * 0.0;
    y == y
}

// Port of: bench/MathBench.cpp#L148-L150 (chrome/m156)
fn isfinite_and_int(data: &[f32]) -> bool {
    is_finite_int(data[0])
        && is_finite_int(data[1])
        && is_finite_int(data[2])
        && is_finite_int(data[3])
}

// Port of: bench/MathBench.cpp#L152-L154 (chrome/m156)
fn isfinite_and_mulzero(data: &[f32]) -> bool {
    is_finite_mulzero(data[0])
        && is_finite_mulzero(data[1])
        && is_finite_mulzero(data[2])
        && is_finite_mulzero(data[3])
}

/// `mulzeroadd(data)`: left to right, as the macro expands.
// Port of: bench/MathBench.cpp#L156-L157 (chrome/m156)
fn mulzeroadd(data: &[f32]) -> f32 {
    data[0] * 0.0 + data[1] * 0.0 + data[2] * 0.0 + data[3] * 0.0
}

// Port of: bench/MathBench.cpp#L158-L160 (chrome/m156)
fn isfinite_plus_int(data: &[f32]) -> bool {
    is_finite_int(mulzeroadd(data))
}

// Port of: bench/MathBench.cpp#L161-L163 (chrome/m156)
#[allow(clippy::eq_op, clippy::float_cmp)] // `x == x` is the NaN test of the C++
fn isfinite_plus_mulzero(data: &[f32]) -> bool {
    let x = mulzeroadd(data);
    x == x
}

/// `typedef bool (*IsFiniteProc)(const float[])`.
type IsFiniteProc = fn(&[f32]) -> bool;

/// `gRec`: the four procs and their names.
// Port of: bench/MathBench.cpp#L166-L177 (chrome/m156)
const IS_FINITE_RECS: [(IsFiniteProc, &str); 4] = [
    (isfinite_and_int, "isfinite_and_int"),
    (isfinite_and_mulzero, "isfinite_and_mulzero"),
    (isfinite_plus_int, "isfinite_plus_int"),
    (isfinite_plus_mulzero, "isfinite_plus_mulzero"),
];

/// `SkRect::isFinite()` of the four floats at `data`, as `reinterpret_cast<const SkRect*>`.
// Port of: bench/MathBench.cpp#L179-L186 (chrome/m156)
fn is_finite_rect(data: &[f32]) -> bool {
    Rect::new(data[0], data[1], data[2], data[3]).is_finite()
}

const IS_FINITE_N: usize = 1000;

/// `class IsFiniteBench`. `index < 0` is the `SkRect::isFinite` variant (`fProc == nullptr`).
// Port of: bench/MathBench.cpp#L194-L255 (chrome/m156)
struct IsFiniteBench {
    data: [f32; IS_FINITE_N],
    proc_: Option<IsFiniteProc>,
    name: &'static str,
}

impl IsFiniteBench {
    fn new(index: i32) -> Self {
        let mut rand = Random::default();
        let mut data = [0.0; IS_FINITE_N];
        for d in &mut data {
            *d = rand.next_s_scalar1();
        }
        let (proc_, name) = if index < 0 {
            (None, "isfinite_rect")
        } else {
            let (proc_, name) = IS_FINITE_RECS[usize::try_from(index).expect("index >= 0")];
            (Some(proc_), name)
        };
        Self { data, proc_, name }
    }
}

impl Benchmark for IsFiniteBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.to_owned()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let proc_ = self.proc_;
        let data = &self.data;
        // do this so the compiler won't throw away the function call
        let mut counter: i32 = 0;

        if let Some(proc_) = proc_ {
            for _ in 0..loops {
                for i in 0..IS_FINITE_N - 4 {
                    counter = counter.wrapping_add(i32::from(proc_(&data[i..i + 4])));
                }
            }
        } else {
            for _ in 0..loops {
                for i in 0..IS_FINITE_N - 4 {
                    counter = counter.wrapping_add(i32::from(is_finite_rect(&data[i..i + 4])));
                }
            }
        }

        // SkPaint paint; if (paint.getAlpha() == 0) SkDebugf("%d\n", counter);
        let paint = Paint::default();
        if paint.alpha() == 0 {
            eprintln!("{counter}");
        }
    }
}

/// `class NormalizeBench`. `process` (a virtual no-op in C++) is `black_box`, the sink that keeps
/// the loops alive.
// Port of: bench/MathBench.cpp#L265-L305 (chrome/m156)
struct NormalizeBench {
    vec: [Vector; ARRAY],
}

impl NormalizeBench {
    fn new() -> Self {
        let mut rand = Random::default();
        let mut vec = [Vector::default(); ARRAY];
        for v in &mut vec {
            let x = rand.next_s_scalar1();
            let y = rand.next_s_scalar1();
            v.set(x, y);
        }
        Self { vec }
    }
}

impl Benchmark for NormalizeBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        "point_normalize".to_owned()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let mut accum: i32 = 0;

        for _ in 0..loops {
            for v in &mut self.vec {
                accum = accum.wrapping_add(i32::from(v.normalize()));
            }
            std::hint::black_box(accum); // this->process(accum)
        }
    }
}

/// `class FixedMathBench`.
// Port of: bench/MathBench.cpp#L312-L352 (chrome/m156)
struct FixedMathBench {
    data: [f32; IS_FINITE_N],
    result: [Fixed; IS_FINITE_N],
}

impl FixedMathBench {
    fn new() -> Self {
        let mut rand = Random::default();
        let mut data = [0.0; IS_FINITE_N];
        for d in &mut data {
            *d = rand.next_s_scalar1();
        }
        Self {
            data,
            result: [0; IS_FINITE_N],
        }
    }
}

impl Benchmark for FixedMathBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        "float_to_fixed".to_owned()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            for i in 0..IS_FINITE_N - 4 {
                self.result[i] = float_to_fixed(self.data[i]);
            }
        }

        // SkPaint paint; if (paint.getAlpha() == 0) SkDebugf("%d\n", fResult[0]);
        let paint = Paint::default();
        if paint.alpha() == 0 {
            eprintln!("{}", self.result[0]);
        }
    }
}

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

// Port of: bench/MathBench.cpp#L357-L370 (chrome/m156)
def_bench!(qmul32_bench = "QMul32Bench()", new_qmul32_bench());
def_bench!(
    is_finite_bench_neg_1 = "IsFiniteBench(-1)",
    IsFiniteBench::new(-1)
);
def_bench!(
    is_finite_bench_0 = "IsFiniteBench(0)",
    IsFiniteBench::new(0)
);
def_bench!(
    is_finite_bench_1 = "IsFiniteBench(1)",
    IsFiniteBench::new(1)
);
def_bench!(
    is_finite_bench_2 = "IsFiniteBench(2)",
    IsFiniteBench::new(2)
);
def_bench!(
    is_finite_bench_3 = "IsFiniteBench(3)",
    IsFiniteBench::new(3)
);
def_bench!(normalize_bench = "NormalizeBench()", NormalizeBench::new());
def_bench!(fixed_math_bench = "FixedMathBench()", FixedMathBench::new());
