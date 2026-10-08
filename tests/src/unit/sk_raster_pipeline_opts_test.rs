// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tests/SkRasterPipelineOptsTest.cpp

// Port of: tests/SkRasterPipelineOptsTest.cpp (chrome/m156)
//
// Mapping notes: the C++ test includes `SkRasterPipeline_opts.h` and calls its stage-level
// functions (`sin_`, `approx_pow2`, `any`, …) on `F`/`I32` lanes of the host's CPU tier. Those are
// private to the stamped tier modules here, so each test runs the pipeline stage that applies the
// function to a register (`sin_float`, `exp2_float`, …) on `skia_rust_simd::selection()`, the
// tier `SkOpts` would pick, with `F_(x)` as a register of `N` copies of `x`; `all(…)` is "every
// lane". `any`/`all` themselves are observed through `branch_if_any_lanes_active` and
// `branch_if_all_lanes_active`, whose result is `any(execution_mask())` and
// `all(execution_mask() | tailLanes)` (no tail lanes in a full chunk). `std::sin` and friends are
// the C library's float functions, `skia_rust_core::libm` (the oracle's UCRT).

use skia_rust_core::libm;
use skia_rust_simd::rp::contexts::{BinaryOpCtx, BranchCtx};
use skia_rust_simd::rp::{MemPtr, MemSlot, MemView, MemoryBindings, Program, Stage};

use crate::{def_test, reporter_assert};

const SK_SCALAR_PI: f32 = skia_rust_core::scalar::SCALAR_PI;
const REG: MemPtr = MemPtr::new(MemSlot(0), 0);

/// The highp stride of the host tier.
fn stride() -> usize {
    skia_rust_simd::selection().tier.highp_stride()
}

/// Runs `stages` over one full chunk with register `i` set to `F_(values[i])`, returning the
/// `N` lanes of each of the first `outputs` registers.
fn run_registers(program: &mut Program<'_>, values: &[f32], outputs: usize) -> Vec<Vec<f32>> {
    let n = stride();
    let mut bytes = vec![0u8; 4 * 4 * 16];
    for (i, v) in values.iter().enumerate() {
        for lane in 0..n {
            let at = 4 * (i * n + lane);
            bytes[at..at + 4].copy_from_slice(&v.to_ne_bytes());
        }
    }
    let mut mem = MemoryBindings::new().with(MemSlot(0), MemView::write(&mut bytes));
    program.run(0, 0, n, 1, &mut mem);
    drop(mem);
    (0..outputs)
        .map(|i| {
            (0..n)
                .map(|lane| {
                    let at = 4 * (i * n + lane);
                    f32::from_ne_bytes(bytes[at..at + 4].try_into().unwrap())
                })
                .collect()
        })
        .collect()
}

/// A program applying the 1-register stage `stage` to register 0.
fn unary(stage: Stage<'static>) -> Program<'static> {
    Program::new(&[stage], skia_rust_simd::selection(), true)
}

/// `stage(F_(x))`: its `N` lanes.
fn apply(program: &mut Program<'_>, x: f32) -> Vec<f32> {
    run_registers(program, &[x], 1).remove(0)
}

/// A program applying the two-register binary stage `stage` to register 0 (`dst`) and 1 (`src`).
fn binary(stage: fn(BinaryOpCtx) -> Stage<'static>) -> Program<'static> {
    let ctx = BinaryOpCtx {
        dst: 0,
        src: u32::try_from(4 * stride()).unwrap(),
    };
    Program::new(
        &[Stage::SetBasePointer(REG), stage(ctx)],
        skia_rust_simd::selection(),
        true,
    )
}

/// `all(delta < tolerance)` for `abs_(expected - result)`.
fn all_within(result: &[f32], expected: f32, tolerance: f32) -> bool {
    result.iter().all(|r| (expected - r).abs() < tolerance)
}

/// A program that takes its branch iff `any` (or `all`) of the `N` masks in `a` is set: it skips
/// `seed_shader`, which would otherwise set `b` to 1.
fn any_all_program(any: bool) -> Program<'static> {
    let ctx = BranchCtx { offset: 2 };
    let branch = if any {
        Stage::BranchIfAnyLanesActive(ctx)
    } else {
        Stage::BranchIfAllLanesActive(ctx)
    };
    Program::new(
        &[
            Stage::LoadSrc(REG),
            branch,
            Stage::SeedShader,
            Stage::StoreSrc(MemPtr::new(MemSlot(1), 0)),
        ],
        skia_rust_simd::selection(),
        true,
    )
}

/// Whether `program` (from `any_all_program`) took its branch with `masks` in `a`.
fn branch_taken(program: &mut Program<'_>, masks: &[i32]) -> bool {
    let n = stride();
    let mut regs = vec![0u8; 4 * 4 * 16];
    for (lane, m) in masks.iter().enumerate() {
        let at = 4 * (3 * n + lane);
        regs[at..at + 4].copy_from_slice(&m.to_ne_bytes());
    }
    let mut out = vec![0u8; 4 * 4 * 16];
    let mut mem = MemoryBindings::new()
        .with(MemSlot(0), MemView::read(&regs))
        .with(MemSlot(1), MemView::write(&mut out));
    program.run(0, 0, n, 1, &mut mem);
    drop(mem);
    // `b` is still 0 iff `seed_shader` was skipped.
    f32::from_ne_bytes(out[4 * 2 * n..4 * 2 * n + 4].try_into().unwrap()) == 0.0
}

// Port of: tests/SkRasterPipelineOptsTest.cpp#L33-L48 (chrome/m156)
/// Makes an array of masks that correspond to the bit pattern of `bits`.
fn make_masks(n: usize, mut bits: i32) -> Vec<i32> {
    let mut masks = vec![0; n];
    for mask in &mut masks {
        *mask = if bits & 1 != 0 { !0 } else { 0 };
        bits >>= 1;
    }
    assert_eq!(bits, 0);
    masks
}

// Port of: tests/SkRasterPipelineOptsTest.cpp#L50-L62 (chrome/m156)
def_test!(SkRasterPipelineOpts_Any, |r| {
    let n = stride();
    let mut program = any_all_program(true);

    for value in 0..(1 << n) {
        // Load masks corresponding to the bit-pattern of `value` into lanes of `i`.
        let masks = make_masks(n, value);

        // Verify that the raster pipeline any() matches expectations.
        reporter_assert!(
            r,
            branch_taken(&mut program, &masks) == masks.iter().any(|m| *m != 0)
        );
    }
});

// Port of: tests/SkRasterPipelineOptsTest.cpp#L64-L76 (chrome/m156)
def_test!(SkRasterPipelineOpts_All, |r| {
    let n = stride();
    let mut program = any_all_program(false);

    for value in 0..(1 << n) {
        // Load masks corresponding to the bit-pattern of `value` into lanes of `i`.
        let masks = make_masks(n, value);

        // Verify that the raster pipeline all() matches expectations.
        reporter_assert!(
            r,
            branch_taken(&mut program, &masks) == masks.iter().all(|m| *m != 0)
        );
    }
});

// Port of: tests/SkRasterPipelineOptsTest.cpp#L78-L88 (chrome/m156)
def_test!(SkRasterPipelineOpts_Sin, |r| {
    let pi = SK_SCALAR_PI;
    let k_tolerance = 0.000_875_f32;
    let mut program = unary(Stage::SinFloat(REG));
    let mut rad = -5.0 * pi;
    while rad <= 5.0 * pi {
        let result = apply(&mut program, rad);
        let expected = libm::sinf(rad);

        reporter_assert!(r, all_within(&result, expected, k_tolerance));
        rad += 0.1;
    }
});

// Port of: tests/SkRasterPipelineOptsTest.cpp#L90-L100 (chrome/m156)
def_test!(SkRasterPipelineOpts_Cos, |r| {
    let pi = SK_SCALAR_PI;
    let k_tolerance = 0.000_875_f32;
    let mut program = unary(Stage::CosFloat(REG));
    let mut rad = -5.0 * pi;
    while rad <= 5.0 * pi {
        let result = apply(&mut program, rad);
        let expected = libm::cosf(rad);

        reporter_assert!(r, all_within(&result, expected, k_tolerance));
        rad += 0.1;
    }
});

// Port of: tests/SkRasterPipelineOptsTest.cpp#L102-L119 (chrome/m156)
def_test!(SkRasterPipelineOpts_Tan, |r| {
    // Our tangent diverges more as we get near infinities (x near +- Pi/2),
    // so we bring in the domain a little.
    let pi = SK_SCALAR_PI;
    let k_epsilon = 0.16_f32;
    let k_tolerance = 0.001_75_f32;
    let mut program = unary(Stage::TanFloat(REG));

    // Test against various multiples of Pi, to check our periodicity
    for period in [0.0f32, -3.0 * pi, 3.0 * pi] {
        let mut rad = -pi / 2.0 + k_epsilon;
        while rad <= pi / 2.0 - k_epsilon {
            let result = apply(&mut program, rad + period);
            let expected = libm::tanf(rad);

            reporter_assert!(r, all_within(&result, expected, k_tolerance));
            rad += 0.01;
        }
    }
});

// Port of: tests/SkRasterPipelineOptsTest.cpp#L121-L130 (chrome/m156)
def_test!(SkRasterPipelineOpts_Asin, |r| {
    let k_tolerance = 0.001_75_f32;
    let mut program = unary(Stage::AsinFloat(REG));
    let mut x = -1.0f32;
    while x <= 1.0 {
        let result = apply(&mut program, x);
        let expected = libm::asinf(x);

        reporter_assert!(r, all_within(&result, expected, k_tolerance));
        x += 1.0 / 64.0;
    }
});

// Port of: tests/SkRasterPipelineOptsTest.cpp#L132-L141 (chrome/m156)
def_test!(SkRasterPipelineOpts_Acos, |r| {
    let k_tolerance = 0.001_75_f32;
    let mut program = unary(Stage::AcosFloat(REG));
    let mut x = -1.0f32;
    while x <= 1.0 {
        let result = apply(&mut program, x);
        let expected = libm::acosf(x);

        reporter_assert!(r, all_within(&result, expected, k_tolerance));
        x += 1.0 / 64.0;
    }
});

// Port of: tests/SkRasterPipelineOptsTest.cpp#L143-L152 (chrome/m156)
def_test!(SkRasterPipelineOpts_Atan, |r| {
    let k_tolerance = 0.001_75_f32;
    let mut program = unary(Stage::AtanFloat(REG));
    let mut x = -10.0f32;
    while x <= 10.0 {
        let result = apply(&mut program, x);
        let expected = libm::atanf(x);

        reporter_assert!(r, all_within(&result, expected, k_tolerance));
        x += 0.1;
    }
});

// Port of: tests/SkRasterPipelineOptsTest.cpp#L154-L165 (chrome/m156)
def_test!(SkRasterPipelineOpts_Atan2, |r| {
    let k_tolerance = 0.001_75_f32;
    let mut program = binary(Stage::Atan2NFloats);
    let mut y = -3.0f32;
    while y <= 3.0 {
        let mut x = -3.0f32;
        while x <= 3.0 {
            let result = run_registers(&mut program, &[y, x], 1).remove(0);
            let expected = libm::atan2f(y, x);

            reporter_assert!(r, all_within(&result, expected, k_tolerance));
            x += 0.1;
        }
        y += 0.1;
    }
});

// Port of: tests/SkRasterPipelineOptsTest.cpp#L167-L176 (chrome/m156)
def_test!(SkRasterPipelineOpts_Log2, |r| {
    let k_tolerance = 0.001_f32;
    let mut program = unary(Stage::Log2Float(REG));
    for value in [0.25f32, 0.5, 1.0, 2.0, 4.0, 8.0] {
        let result = apply(&mut program, value);
        let expected = libm::log2f(value);

        reporter_assert!(r, all_within(&result, expected, k_tolerance));
    }
});

// Port of: tests/SkRasterPipelineOptsTest.cpp#L178-L190 (chrome/m156)
def_test!(SkRasterPipelineOpts_Pow2, |r| {
    let k_tolerance = 0.001_f32;
    let mut program = unary(Stage::Exp2Float(REG));
    for value in [-80.0f32, -5.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 5.0] {
        let result = apply(&mut program, value);
        // `std::pow(2.0, value)` is a double, converted by `F_(float)`.
        #[allow(clippy::cast_possible_truncation)] // mirrors the implicit double -> float
        let expected = libm::pow(2.0, f64::from(value)) as f32;

        reporter_assert!(r, all_within(&result, expected, k_tolerance));
    }

    let result = apply(&mut program, 160.0);
    reporter_assert!(r, result.iter().all(|v| *v == f32::INFINITY));
});
