// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ParametricStageTest.cpp (chrome/m156)
//
// Mapping notes (skia-rust): the C++ pipeline is `load_f32, appendTransferFunction(fn),
// store_f32`, run over 64 pixels. `load_f32`/`store_f32` (task B2) and `appendTransferFunction`
// (task A4) are not ported yet, so each pixel is run on its own: `unbounded_uniform_color` sets
// the pixel's four floats, the stage that `appendTransferFunction` picks for a `sRGBish`
// function (`gamma_` for a pure gamma, `parametric` otherwise; the other three kinds are not
// used here) transforms them, and `store_src` writes them out (lane 0 is the pixel). The
// expected values and the error limit are the C++'s.

use skia_rust_core::raster_pipeline::{MemSlot, MemView, MemoryBindings, RasterPipeline, Stage};
use skia_rust_simd::rp::MemPtr;
use skia_rust_simd::rp::contexts::{TransferFunction, UniformColorCtx};

use crate::{def_test, errorf};

// Port of: tests/ParametricStageTest.cpp#L16-L45 (chrome/m156)
// `i / 255.0f` and `tf.a == 1`: exact in C++ too.
#[allow(clippy::cast_precision_loss, clippy::float_cmp)]
fn check_error(r: &mut crate::Reporter, limit: f32, fn_: TransferFunction) {
    let mut input = [0.0f32; 256];
    let mut out = [0.0f32; 256];
    for i in 0..256 {
        input[i] = i as f32 / 255.0;
        out[i] = 0.0; // Not likely important.  Just being tidy.
    }

    // `SkRasterPipeline::appendTransferFunction`, for the `sRGBish` kind.
    let tf_stage = if fn_.a == 1.0
        && fn_.b == 0.0
        && fn_.c == 0.0
        && fn_.d == 0.0
        && fn_.e == 0.0
        && fn_.f == 0.0
    {
        Stage::Gamma(fn_.g)
    } else {
        Stage::Parametric(&fn_)
    };

    // `p.run(0,0, 256/4,1)`, one pixel at a time (see the mapping notes).
    let n = skia_rust_simd::selection().tier.highp_stride();
    for px in 0..256 / 4 {
        let color = UniformColorCtx {
            r: input[4 * px],
            g: input[4 * px + 1],
            b: input[4 * px + 2],
            a: input[4 * px + 3],
            rgba: [0; 4],
        };
        let mut p = RasterPipeline::new();
        p.unchecked_append(Stage::UnboundedUniformColor(&color));
        p.unchecked_append(tf_stage);
        p.append(Stage::StoreSrc(MemPtr::new(MemSlot(0), 0)));

        let mut regs = vec![0u8; 4 * 4 * n];
        p.run(
            px,
            0,
            1,
            1,
            &mut MemoryBindings::new().with(MemSlot(0), MemView::write(&mut regs)),
        );
        for c in 0..4 {
            // Lane 0 of channel `c`.
            out[4 * px + c] =
                f32::from_ne_bytes(regs[4 * n * c..4 * n * c + 4].try_into().unwrap());
        }
    }

    for i in 0..256 {
        let mut want = if input[i] <= fn_.d {
            fn_.c * input[i] + fn_.f
        } else {
            (input[i] * fn_.a + fn_.b).powf(fn_.g) + fn_.e
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
