// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/InterpBench.cpp

//! Fixed-point and floating-point interpolation of a 128-entry tile table (`bench/InterpBench.cpp`).

use skia_rust_core::fixed::{Fixed, double_to_fixed, float_to_fixed};

use crate::def_bench;
use crate::prelude::*;

// Port of: bench/InterpBench.cpp#L13-L13 (chrome/m156)
const K_BUFFER: usize = 128;

/// `#define TILE(x, width) (((x) & 0xFFFF) * width >> 16)`.
// Port of: bench/InterpBench.cpp#L11-L11 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // the result is < width, which fits an int16_t
fn tile(x: i32, width: i32) -> i16 {
    (((x & 0xFFFF) * width) >> 16) as i16
}

/// `InterpBench::performTest(int16_t dst[], float x, float dx, int count)`.
trait InterpTest {
    fn perform_test(&self, dst: &mut [i16], x: f32, dx: f32, count: usize);
}

/// `class InterpBench`: the base class, generic over the `performTest` of its subclass.
// Port of: bench/InterpBench.cpp#L13-L42 (chrome/m156)
struct InterpBench<T> {
    name: String,
    dst: [i16; K_BUFFER],
    fx: f32,
    dx: f32,
    test: T,
}

impl<T: InterpTest> InterpBench<T> {
    fn new(name: &str, test: T) -> Self {
        Self {
            name: format!("interp_{name}"),
            dst: [0; K_BUFFER],
            fx: 3.3,
            dx: 0.1257,
            test,
        }
    }
}

impl<T: InterpTest> Benchmark for InterpBench<T> {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        // mulLoopCount() is 1 for every subclass here.
        for _ in 0..loops {
            self.test
                .perform_test(&mut self.dst, self.fx, self.dx, K_BUFFER);
        }
    }
}

// ---------------------------------------------------------------------------------------------

/// `class Fixed16D16Interp`.
// Port of: bench/InterpBench.cpp#L44-L58 (chrome/m156)
struct Fixed16D16Interp;

impl InterpTest for Fixed16D16Interp {
    fn perform_test(&self, dst: &mut [i16], fx: f32, dx: f32, count: usize) {
        let width = i32::try_from(count).expect("count fits an int");
        let mut curr: Fixed = float_to_fixed(fx);
        let step: Fixed = float_to_fixed(dx);
        for i in (0..count).step_by(4) {
            dst[i] = tile(curr, width);
            curr += step;
            dst[i + 1] = tile(curr, width);
            curr += step;
            dst[i + 2] = tile(curr, width);
            curr += step;
            dst[i + 3] = tile(curr, width);
            curr += step;
        }
    }
}

/// `class Fixed32D32Interp`.
// Port of: bench/InterpBench.cpp#L60-L84 (chrome/m156)
struct Fixed32D32Interp;

impl InterpTest for Fixed32D32Interp {
    #[allow(clippy::cast_possible_truncation)] // (SkFixed)(curr >> 16) truncates, as in C++
    fn perform_test(&self, dst: &mut [i16], fx: f32, dx: f32, count: usize) {
        let width = i32::try_from(count).expect("count fits an int");
        // (int64_t)(fx * 65536 * 655536): the float products, then the conversion.
        let mut curr = (fx * 65536.0 * 655_536.0) as i64;
        let step = (dx * 65536.0 * 655_536.0) as i64;
        for i in (0..count).step_by(4) {
            let mut tmp = (curr >> 16) as Fixed;
            dst[i] = tile(tmp, width);
            curr += step;

            tmp = (curr >> 16) as Fixed;
            dst[i + 1] = tile(tmp, width);
            curr += step;

            tmp = (curr >> 16) as Fixed;
            dst[i + 2] = tile(tmp, width);
            curr += step;

            tmp = (curr >> 16) as Fixed;
            dst[i + 3] = tile(tmp, width);
            curr += step;
        }
    }
}

/// `class Fixed16D48Interp`.
// Port of: bench/InterpBench.cpp#L86-L104 (chrome/m156)
struct Fixed16D48Interp;

impl InterpTest for Fixed16D48Interp {
    #[allow(clippy::cast_possible_truncation)] // (SkFixed) (curr >> 32) truncates, as in C++
    fn perform_test(&self, dst: &mut [i16], fx: f32, dx: f32, count: usize) {
        let width = i32::try_from(count).expect("count fits an int");
        let mut curr = (fx * 65536.0 * 655_536.0 * 65536.0) as i64;
        let step = (dx * 65536.0 * 655_536.0 * 65536.0) as i64;
        for i in (0..count).step_by(4) {
            let tmp = (curr >> 32) as Fixed;
            dst[i] = tile(tmp, width);
            curr += step;
            let tmp = (curr >> 32) as Fixed;
            dst[i + 1] = tile(tmp, width);
            curr += step;
            let tmp = (curr >> 32) as Fixed;
            dst[i + 2] = tile(tmp, width);
            curr += step;
            let tmp = (curr >> 32) as Fixed;
            dst[i + 3] = tile(tmp, width);
            curr += step;
        }
    }
}

/// `class FloatInterp`.
// Port of: bench/InterpBench.cpp#L106-L121 (chrome/m156)
struct FloatInterp;

impl InterpTest for FloatInterp {
    fn perform_test(&self, dst: &mut [i16], mut fx: f32, dx: f32, count: usize) {
        let width = i32::try_from(count).expect("count fits an int");
        for i in (0..count).step_by(4) {
            let tmp = float_to_fixed(fx);
            dst[i] = tile(tmp, width);
            fx += dx;
            let tmp = float_to_fixed(fx);
            dst[i + 1] = tile(tmp, width);
            fx += dx;
            let tmp = float_to_fixed(fx);
            dst[i + 2] = tile(tmp, width);
            fx += dx;
            let tmp = float_to_fixed(fx);
            dst[i + 3] = tile(tmp, width);
            fx += dx;
        }
    }
}

/// `class DoubleInterp`.
// Port of: bench/InterpBench.cpp#L123-L139 (chrome/m156)
struct DoubleInterp;

impl InterpTest for DoubleInterp {
    fn perform_test(&self, dst: &mut [i16], fx: f32, dx: f32, count: usize) {
        let width = i32::try_from(count).expect("count fits an int");
        let mut ffx = f64::from(fx);
        let ddx = f64::from(dx);
        for i in (0..count).step_by(4) {
            let tmp = double_to_fixed(ffx);
            dst[i] = tile(tmp, width);
            ffx += ddx;
            let tmp = double_to_fixed(ffx);
            dst[i + 1] = tile(tmp, width);
            ffx += ddx;
            let tmp = double_to_fixed(ffx);
            dst[i + 2] = tile(tmp, width);
            ffx += ddx;
            let tmp = double_to_fixed(ffx);
            dst[i + 3] = tile(tmp, width);
            ffx += ddx;
        }
    }
}

// Port of: bench/InterpBench.cpp#L143-L147 (chrome/m156)
def_bench!(
    fixed16_d16_interp = "Fixed16D16Interp()",
    InterpBench::new("16.16", Fixed16D16Interp)
);
def_bench!(
    fixed32_d32_interp = "Fixed32D32Interp()",
    InterpBench::new("32.32", Fixed32D32Interp)
);
def_bench!(
    fixed16_d48_interp = "Fixed16D48Interp()",
    InterpBench::new("16.48", Fixed16D48Interp)
);
def_bench!(
    float_interp = "FloatInterp()",
    InterpBench::new("float", FloatInterp)
);
def_bench!(
    double_interp = "DoubleInterp()",
    InterpBench::new("double", DoubleInterp)
);
