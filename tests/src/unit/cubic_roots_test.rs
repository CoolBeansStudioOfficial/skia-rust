// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CubicRootsTest.cpp (chrome/m156)
//
// Not ported yet (manifest stays `todo`): `CubicRootsReal_ActualCubics`, `CubicRootsReal_Quadratics`,
// `CubicRootsReal_Linear`, `CubicRootsReal_Constant`, `CubicRootsValidT` and
// `CubicRootsValidT_ClampToZeroAndOne` (they also check `SkDCubic` from SkPathOps).

#![cfg(test)]

use skia_rust_core::cubics;

use crate::{def_test, reporter_assert};

// Port of: tests/CubicRootsTest.cpp#L250-L296 (chrome/m156)
def_test!(CubicRootsReal_NonFiniteNumbers, |reporter| {
    // The Pathops implementation does not check for infinities nor nans in all cases.
    let mut roots = [0.0f64; 3];
    reporter_assert!(
        reporter,
        cubics::roots_real(f64::NAN, 1.0, 2.0, 3.0, &mut roots) == 0,
        "Nan A"
    );
    reporter_assert!(
        reporter,
        cubics::roots_real(1.0, f64::NAN, 2.0, 3.0, &mut roots) == 0,
        "Nan B"
    );
    reporter_assert!(
        reporter,
        cubics::roots_real(1.0, 2.0, f64::NAN, 3.0, &mut roots) == 0,
        "Nan C"
    );
    reporter_assert!(
        reporter,
        cubics::roots_real(1.0, 2.0, 3.0, f64::NAN, &mut roots) == 0,
        "Nan D"
    );

    {
        // oss-fuzz:55419 C and D are large
        let num_roots = cubics::roots_real(
            -2.0,
            0.0,
            f64::from_bits(0xd542_2020_2020_20ff), //-5.074559e+102
            f64::from_bits(0x600f_ff20_2020_ff20), // 5.362551e+154
            &mut roots,
        );
        reporter_assert!(
            reporter,
            num_roots == 0,
            "No finite roots expected, got {}",
            num_roots
        );
    }
    {
        // oss-fuzz:55829 A is zero and B is NAN
        let num_roots = cubics::roots_real(
            0.0,
            f64::from_bits(0xffff_ffff_ffff_2020), //-nan
            f64::from_bits(0x2020_2020_2020_20ff), // 6.013470e-154
            f64::from_bits(0xff20_2020_2020_2020), //-2.211661e+304
            &mut roots,
        );
        reporter_assert!(
            reporter,
            num_roots == 0,
            "No finite roots expected, got {}",
            num_roots
        );
    }
});
