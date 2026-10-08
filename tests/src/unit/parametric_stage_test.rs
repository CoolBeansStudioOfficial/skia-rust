// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ParametricStageTest.cpp (chrome/m156)
//
// Mapping notes: `SkRasterPipeline_<256> p` is a `RasterPipeline`; the `MemoryCtx`s name slots
// bound to the input and output buffers when the pipeline runs.

use skia_rust_core::libm;
use skia_rust_core::raster_pipeline::{
    MemSlot, MemView, MemoryBindings, MemoryCtx, RasterPipeline, Stage,
};
use skia_rust_simd::rp::contexts::TransferFunction;

use crate::{def_test, errorf};

// Port of: tests/ParametricStageTest.cpp#L16-L45 (chrome/m156)
// `i / 255.0f` is exact in C++ too.
#[allow(clippy::cast_precision_loss)]
fn check_error(r: &mut crate::Reporter, limit: f32, fn_: TransferFunction) {
    let mut input = [0.0f32; 256];
    let mut out = [0.0f32; 256];
    for i in 0..256 {
        input[i] = i as f32 / 255.0;
        out[i] = 0.0; // Not likely important.  Just being tidy.
    }

    let ip = MemoryCtx::new(MemSlot(0));
    let op = MemoryCtx::new(MemSlot(1));

    let in_bytes: Vec<u8> = input.iter().flat_map(|v| v.to_ne_bytes()).collect();
    let mut out_bytes = vec![0u8; 256 * 4];

    let mut p = RasterPipeline::new();
    p.append(Stage::LoadF32(ip));
    p.append_transfer_function(&fn_);
    p.append(Stage::StoreF32(op));

    p.run(
        0,
        0,
        256 / 4,
        1,
        &mut MemoryBindings::new()
            .with(MemSlot(0), MemView::read(&in_bytes))
            .with(MemSlot(1), MemView::write(&mut out_bytes)),
    );
    for (o, c) in out.iter_mut().zip(out_bytes.as_chunks::<4>().0) {
        *o = f32::from_ne_bytes(*c);
    }

    for i in 0..256 {
        let mut want = if input[i] <= fn_.d {
            fn_.c * input[i] + fn_.f
        } else {
            libm::powf(input[i] * fn_.a + fn_.b, fn_.g) + fn_.e
        };
        if i % 4 == 3 {
            // alpha should stay unchanged.
            want = input[i];
        }
        let err = (out[i] - want).abs();
        if err > limit {
            errorf!(
                r,
                "At {}, error was {} (got {}, want {})",
                i,
                err,
                out[i],
                want
            );
        }
    }
}

// Port of: tests/ParametricStageTest.cpp#L47-L52 (chrome/m156)
fn check_error_gamma(r: &mut crate::Reporter, limit: f32, gamma: f32) {
    let fn_ = TransferFunction {
        g: gamma,
        a: 1.0,
        ..TransferFunction::default()
    };
    check_error(r, limit, fn_);
}

// Port of: tests/ParametricStageTest.cpp#L54-L65 (chrome/m156)
def_test!(Parametric_sRGB, |r| {
    // Test our good buddy the sRGB transfer function in resplendent 7-parameter glory.
    check_error(
        r,
        1.0 / 510.0,
        TransferFunction {
            g: 2.4,
            a: 1.0 / 1.055,
            b: 0.055 / 1.055,
            c: 1.0 / 12.92,
            d: 0.04045,
            e: 0.0,
            f: 0.0,
        },
    );
});

// A nice little spread of simple gammas.
// Port of: tests/ParametricStageTest.cpp#L68-L68 (chrome/m156)
def_test!(Parametric_1dot0, |r| {
    check_error_gamma(r, 1.0 / 510.0, 1.0);
});

// Port of: tests/ParametricStageTest.cpp#L70-L70 (chrome/m156)
def_test!(Parametric_1dot2, |r| {
    check_error_gamma(r, 1.0 / 510.0, 1.2);
});
// Port of: tests/ParametricStageTest.cpp#L71-L71 (chrome/m156)
def_test!(Parametric_1dot4, |r| {
    check_error_gamma(r, 1.0 / 510.0, 1.4);
});
// Port of: tests/ParametricStageTest.cpp#L72-L72 (chrome/m156)
def_test!(Parametric_1dot8, |r| {
    check_error_gamma(r, 1.0 / 510.0, 1.8);
});
// Port of: tests/ParametricStageTest.cpp#L73-L73 (chrome/m156)
def_test!(Parametric_2dot0, |r| {
    check_error_gamma(r, 1.0 / 510.0, 2.0);
});
// Port of: tests/ParametricStageTest.cpp#L74-L74 (chrome/m156)
def_test!(Parametric_2dot2, |r| {
    check_error_gamma(r, 1.0 / 510.0, 2.2);
});
// Port of: tests/ParametricStageTest.cpp#L75-L75 (chrome/m156)
def_test!(Parametric_2dot4, |r| {
    check_error_gamma(r, 1.0 / 510.0, 2.4);
});

// Port of: tests/ParametricStageTest.cpp#L77-L77 (chrome/m156)
def_test!(Parametric_inv_1dot2, |r| {
    check_error_gamma(r, 1.0 / 510.0, 1.0 / 1.2);
});
// Port of: tests/ParametricStageTest.cpp#L78-L78 (chrome/m156)
def_test!(Parametric_inv_1dot4, |r| {
    check_error_gamma(r, 1.0 / 510.0, 1.0 / 1.4);
});
// Port of: tests/ParametricStageTest.cpp#L79-L79 (chrome/m156)
def_test!(Parametric_inv_1dot8, |r| {
    check_error_gamma(r, 1.0 / 510.0, 1.0 / 1.8);
});
// Port of: tests/ParametricStageTest.cpp#L80-L80 (chrome/m156)
def_test!(Parametric_inv_2dot0, |r| {
    check_error_gamma(r, 1.0 / 510.0, 1.0 / 2.0);
});
// Port of: tests/ParametricStageTest.cpp#L81-L81 (chrome/m156)
def_test!(Parametric_inv_2dot2, |r| {
    check_error_gamma(r, 1.0 / 510.0, 1.0 / 2.2);
});
// Port of: tests/ParametricStageTest.cpp#L82-L82 (chrome/m156)
def_test!(Parametric_inv_2dot4, |r| {
    check_error_gamma(r, 1.0 / 510.0, 1.0 / 2.4);
});
