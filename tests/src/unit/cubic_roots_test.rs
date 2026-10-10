// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CubicRootsTest.cpp (chrome/m156)

#![cfg(test)]
// The float literals are copied verbatim from the C++ tests, digits and all.
#![allow(clippy::unreadable_literal, clippy::excessive_precision)]

use skia_rust_core::cubics;
use skia_rust_core::floating_point::{
    double_nearly_zero, doubles_nearly_equal_ulps, doubles_nearly_equal_ulps_max_diff,
};
use skia_rust_pathops::cubic::DCubic;

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/CubicRootsTest.cpp#L58-L76 (chrome/m156), the sorted comparison that each
// implementation block of the C++ helpers repeats, factored out here.
// The C++ compares the roots that a solver reports, sorted, against the expected roots. The Rust
// loop stops at the shorter of the two lists, so a count mismatch (already reported by the caller)
// cannot index out of bounds.
fn check_sorted_roots(
    reporter: &mut Reporter,
    expected_roots: &[f64],
    roots: &mut [f64; 3],
    root_count: usize,
) {
    // We don't care which order the roots are returned from the algorithm.
    // For determinism, we will sort them (and ensure the provided solutions are also sorted).
    roots[..root_count].sort_by(f64::total_cmp);
    for i in 0..root_count.min(expected_roots.len()) {
        if double_nearly_zero(expected_roots[i]) {
            reporter_assert!(
                reporter,
                double_nearly_zero(roots[i]),
                "0 != {:.16} at index {}",
                roots[i],
                i
            );
        } else {
            reporter_assert!(
                reporter,
                doubles_nearly_equal_ulps_max_diff(expected_roots[i], roots[i], 64),
                "{:.16} != {:.16} at index {}",
                expected_roots[i],
                roots[i],
                i
            );
        }
    }
}

// Port of: tests/CubicRootsTest.cpp#L23-L94 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ helper's signature
#[allow(clippy::many_single_char_names)] // names follow the C++ helper
fn test_cubic_roots_real(
    reporter: &mut Reporter,
    name: &str,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    expected_roots: &[f64],
    skip_pathops: bool,
    skip_root_validation: bool,
) {
    reporter.set_context(Some(name.to_string()));
    // Validate test case
    reporter_assert!(
        reporter,
        expected_roots.len() <= 3,
        "Invalid test case, up to 3 roots allowed"
    );
    for (i, &x) in expected_roots.iter().enumerate() {
        // A*x^3 + B*x^2 + C*x + D should equal 0 (unless floating point error causes issues)
        let y = a * x * x * x + b * x * x + c * x + d;
        if !skip_root_validation {
            reporter_assert!(
                reporter,
                double_nearly_zero(y),
                "Invalid test case root {}. {:.16} != 0",
                i,
                y
            );
        }
        if i > 0 {
            let ascending = expected_roots[i - 1] <= expected_roots[i];
            reporter_assert!(
                reporter,
                ascending,
                "Invalid test case root {}. Roots should be sorted in ascending order",
                i
            );
        }
    }
    // The old pathops implementation sometimes gives incorrect solutions. We can opt
    // our tests out of checking that older implementation if that causes issues.
    if !skip_pathops {
        reporter.set_context(Some(format!("{name}: Pathops Implementation")));
        let mut roots = [0.0f64; 3];
        let root_count = DCubic::roots_real(a, b, c, d, &mut roots);
        reporter_assert!(
            reporter,
            expected_roots.len() == root_count,
            "Wrong number of roots returned {} != {}",
            expected_roots.len(),
            root_count
        );
        check_sorted_roots(reporter, expected_roots, &mut roots, root_count);
    }
    {
        reporter.set_context(Some(format!("{name}: SkCubics Analytic Implementation")));
        let mut roots = [0.0f64; 3];
        let root_count = cubics::roots_real(a, b, c, d, &mut roots);
        reporter_assert!(
            reporter,
            expected_roots.len() == root_count,
            "Wrong number of roots returned {} != {}",
            expected_roots.len(),
            root_count
        );
        check_sorted_roots(reporter, expected_roots, &mut roots, root_count);
    }
    reporter.set_context(None);
}

// Port of: tests/CubicRootsTest.cpp#L96-L200 (chrome/m156)
def_test!(
    #[allow(clippy::excessive_precision, clippy::unreadable_literal)]
    // literals copied verbatim from the C++ test
    CubicRootsReal_ActualCubics,
    |reporter| {
        // All answers are given with 16 significant digits (max for a double) or as an integer
        // when the answer is exact.
        test_cubic_roots_real(
            reporter,
            "one root 1x^3 + 2x^2 + 3x + 4",
            1.0,
            2.0,
            3.0,
            4.0,
            &[-1.650629191439388],
            false,
            false,
        );
        // (3x-5)(6x-10)(x+4) = 18x^3 + 12x^2 - 190x + 200
        test_cubic_roots_real(
            reporter,
            "touches y axis 18x^3 + 12x^2 - 190x + 200",
            18.0,
            12.0,
            -190.0,
            200.0,
            &[-4.0, 1.666666666666667],
            false,
            false,
        );
        test_cubic_roots_real(
            reporter,
            "three roots 10x^3 - 20x^2 - 30x + 40",
            10.0,
            -20.0,
            -30.0,
            40.0,
            &[-1.561552812808830, 1.0, 2.561552812808830],
            false,
            false,
        );
        test_cubic_roots_real(
            reporter,
            "three roots -10x^3 + 200x^2 + 300x - 400",
            -10.0,
            200.0,
            300.0,
            -400.0,
            &[-2.179884793243323, 0.8607083693981839, 21.31917642384514],
            false,
            false,
        );
        test_cubic_roots_real(
            reporter,
            "one root -x^3 + 0x^2 + 5x - 7",
            -1.0,
            0.0,
            5.0,
            -7.0,
            &[-2.747346540307211],
            false,
            false,
        );
        test_cubic_roots_real(
            reporter,
            "one root 2x^3 - 3x^2 + 0x + 3",
            2.0,
            -3.0,
            0.0,
            3.0,
            &[-0.806443932358772],
            false,
            false,
        );
        test_cubic_roots_real(
            reporter,
            "one root x^3 + 0x^2 + 0x - 9",
            1.0,
            0.0,
            0.0,
            -9.0,
            &[2.080083823051904],
            false,
            false,
        );
        test_cubic_roots_real(
            reporter,
            "three roots 2x^3 - 3x^2 - 4x + 0",
            2.0,
            -3.0,
            -4.0,
            0.0,
            &[-0.8507810593582122, 0.0, 2.350781059358212],
            false,
            false,
        );
        test_cubic_roots_real(
            reporter,
            "R^2 and Q^3 are near zero",
            -0.33790159225463867,
            -0.81997990608215332,
            -0.66327774524688721,
            -0.17884063720703125,
            &[-0.7995944894729731],
            false,
            false,
        );
        // The following three cases fallback to treating the cubic as a quadratic.
        // Otherwise, floating point error mangles the solutions near +- 1
        // This means we don't find all the roots, but usually we only care about roots
        // in the range [0, 1], so that is ok.
        test_cubic_roots_real(
            reporter,
            "oss-fuzz:55625 Two roots near zero, one big root",
            f64::from_bits(0xbf1a_8de5_8000_0000), // -0.00010129655
            f64::from_bits(0x4106_c0c6_8000_0000), // 186392.8125
            0.0,
            f64::from_bits(0xc104_c0ce_8000_0000), // -170009.8125
            // Wolfram Alpha puts the root at X = 0.955042 (~2e7 error). 1.84007e9 is the other
            // root, which we do not find.
            &[-0.9550418733785169, 0.9550418733785169],
            true, /* == skipPathops */
            true, /* == skipRootValidation */
        );
        test_cubic_roots_real(
            reporter,
            "oss-fuzz:55625 Two roots near zero, one big root, near linear",
            f64::from_bits(0x3c04_0404_0000_0000), // -1.3563156-19
            f64::from_bits(0x4106_c0c6_8000_0000), // 186392.8125
            0.0,
            f64::from_bits(0xc104_c0ce_8000_0000), // -170009.8125
            // 1.84007e9 is the other root, which we do not find.
            &[-0.9550418733785169, 0.9550418733785169],
            true, /* == skipPathops */
            false,
        );
        test_cubic_roots_real(
            reporter,
            "oss-fuzz:55680 A nearly zero, C is zero",
            f64::from_bits(0x3eb0_0000_0000_0000), // 9.5367431640625000e-07
            f64::from_bits(0x4092_78a5_6000_0000), // 1182.1614990234375
            0.0,
            f64::from_bits(0xc092_7061_6000_0000), // -1180.0950927734375
            // 1.239586176×10^9 is the other root, which we do not find.
            &[-0.9991256228290017, 0.9991256228290017],
            true,
            true, /* == skipRootValidation */
        );
    }
);

// Port of: tests/CubicRootsTest.cpp#L202-L228 (chrome/m156)
def_test!(CubicRootsReal_Quadratics, |reporter| {
    test_cubic_roots_real(
        reporter,
        "two roots -2x^2 + 3x + 4",
        0.0,
        -2.0,
        3.0,
        4.0,
        &[-0.8507810593582122, 2.350781059358212],
        false,
        false,
    );
    test_cubic_roots_real(
        reporter,
        "touches y axis -x^2 + 3x + 4",
        0.0,
        -2.0,
        3.0,
        4.0,
        &[-0.8507810593582122, 2.350781059358212],
        false,
        false,
    );
    test_cubic_roots_real(
        reporter,
        "no roots x^2 + 2x + 7",
        0.0,
        1.0,
        2.0,
        7.0,
        &[],
        false,
        false,
    );
    // similar to oss-fuzz:55680
    test_cubic_roots_real(
        reporter,
        "two roots one small one big (and ignored)",
        0.0,
        -0.01,
        200_000_000_000_000.0,
        -120_000_000_000_000.0,
        &[0.6],
        true, /* == skipPathops */
        false,
    );
});

// Port of: tests/CubicRootsTest.cpp#L230-L238 (chrome/m156)
def_test!(CubicRootsReal_Linear, |reporter| {
    test_cubic_roots_real(
        reporter,
        "positive slope 3x + 4",
        0.0,
        0.0,
        3.0,
        4.0,
        &[-1.333333333333333],
        false,
        false,
    );
    test_cubic_roots_real(
        reporter,
        "negative slope -2x - 8",
        0.0,
        0.0,
        -2.0,
        -8.0,
        &[-4.0],
        false,
        false,
    );
});

// Port of: tests/CubicRootsTest.cpp#L240-L248 (chrome/m156)
def_test!(CubicRootsReal_Constant, |reporter| {
    test_cubic_roots_real(
        reporter,
        "No intersections y = 4",
        0.0,
        0.0,
        0.0,
        4.0,
        &[],
        false,
        false,
    );
    test_cubic_roots_real(
        reporter,
        "Infinite solutions y = 0",
        0.0,
        0.0,
        0.0,
        0.0,
        &[0.0],
        false,
        false,
    );
});

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

// Port of: tests/CubicRootsTest.cpp#L291-L380 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++ helper
fn test_cubic_valid_t(
    reporter: &mut Reporter,
    name: &str,
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    expected_roots: &[f64],
) {
    reporter.set_context(Some(name.to_string()));
    // Validate test case
    reporter_assert!(
        reporter,
        expected_roots.len() <= 3,
        "Invalid test case, up to 3 roots allowed"
    );

    for (i, &x) in expected_roots.iter().enumerate() {
        reporter_assert!(
            reporter,
            (0.0..=1.0).contains(&x),
            "Invalid test case root {}. Roots must be in [0, 1]",
            i
        );

        // A*x^3 + B*x^2 + C*x + D should equal 0
        let y = a * x * x * x + b * x * x + c * x + d;
        reporter_assert!(
            reporter,
            double_nearly_zero(y),
            "Invalid test case root {}. {:.16} != 0",
            i,
            y
        );

        if i > 0 {
            let ascending = expected_roots[i - 1] <= expected_roots[i];
            reporter_assert!(
                reporter,
                ascending,
                "Invalid test case root {}. Roots should be sorted in ascending order",
                i
            );
        }
    }

    {
        reporter.set_context(Some(format!("{name}: Pathops Implementation")));
        let mut roots = [0.0f64; 3];
        let root_count = DCubic::roots_valid_t(a, b, c, d, &mut roots);
        reporter_assert!(
            reporter,
            expected_roots.len() == root_count,
            "Wrong number of roots returned {} != {}",
            expected_roots.len(),
            root_count
        );
        check_sorted_roots(reporter, expected_roots, &mut roots, root_count);
    }
    {
        reporter.set_context(Some(format!("{name}: SkCubics Analytic Implementation")));
        let mut roots = [0.0f64; 3];
        let root_count = cubics::roots_valid_t(a, b, c, d, &mut roots);
        reporter_assert!(
            reporter,
            expected_roots.len() == root_count,
            "Wrong number of roots returned {} != {}",
            expected_roots.len(),
            root_count
        );
        check_sorted_roots(reporter, expected_roots, &mut roots, root_count);
    }
    {
        reporter.set_context(Some(format!(
            "{name}: SkCubics Binary Search Implementation"
        )));
        let mut roots = [0.0f64; 3];
        let root_count = cubics::binary_search_roots_valid_t(a, b, c, d, &mut roots);
        reporter_assert!(
            reporter,
            expected_roots.len() == root_count,
            "Wrong number of roots returned {} != {}",
            expected_roots.len(),
            root_count
        );
        // We don't care which order the roots are returned from the algorithm.
        // For determinism, we will sort them (and ensure the provided solutions are also sorted).
        roots[..root_count].sort_by(f64::total_cmp);
        for i in 0..root_count.min(expected_roots.len()) {
            let delta = (roots[i] - expected_roots[i]).abs();
            let close = delta < 0.000001;
            reporter_assert!(
                reporter,
                // Binary search is not absolutely accurate all the time, but
                // it should be accurate enough reliably
                close,
                "{:.16} != {:.16} at index {}",
                expected_roots[i],
                roots[i],
                i
            );
        }
    }
    reporter.set_context(None);
}

// Port of: tests/CubicRootsTest.cpp#L381-L420 (chrome/m156)
def_test!(
    #[allow(clippy::excessive_precision, clippy::unreadable_literal)]
    // literals copied verbatim from the C++ test
    CubicRootsValidT,
    |reporter| {
        // All answers are given with 16 significant digits (max for a double) or as an integer
        // when the answer is exact.
        test_cubic_valid_t(
            reporter,
            "three roots 24x^3 - 46x^2 + 29x - 6",
            24.0,
            -46.0,
            29.0,
            -6.0,
            &[0.5, 0.6666666666666667, 0.75],
        );
        test_cubic_valid_t(
            reporter,
            "three roots total, two in range 54x^3 - 117x^2 + 45x + 0",
            54.0,
            -117.0,
            45.0,
            0.0,
            // 5/3 is the other root, but not in [0, 1]
            &[0.0, 0.5],
        );
        test_cubic_valid_t(
            reporter,
            "one root = 1 10x^3 - 20x^2 - 30x + 40",
            10.0,
            -20.0,
            -30.0,
            40.0,
            &[1.0],
        );
        test_cubic_valid_t(
            reporter,
            "one root = 0 2x^3 - 3x^2 - 4x + 0",
            2.0,
            -3.0,
            -4.0,
            0.0,
            &[0.0],
        );
        test_cubic_valid_t(
            reporter,
            "three roots total, two in range -2x^3 - 3x^2 + 4x + 0",
            -2.0,
            -3.0,
            4.0,
            0.0,
            // 0.8507810593582121716220544 from Wolfram Alpha
            &[0.0, 0.8507810593582122],
        );
        // x(x-1) = x^2 - x
        test_cubic_valid_t(
            reporter,
            "Two roots at exactly 0 and 1",
            0.0,
            1.0,
            -1.0,
            0.0,
            &[0.0, 1.0],
        );
        test_cubic_valid_t(
            reporter,
            "Single point has one root",
            0.0,
            0.0,
            0.0,
            0.0,
            &[0.0],
        );
    }
);

// Port of: tests/CubicRootsTest.cpp#L422-L452 (chrome/m156)
def_test!(CubicRootsValidT_ClampToZeroAndOne, |reporter| {
    {
        // (x + 0.00001)(x - 1.00005), but the answers will be 0 and 1
        let a = 0.0;
        let b = 1.0;
        let c = -1.00004;
        let d = -0.0000100005;
        let mut roots = [0.0f64; 3];
        let root_count = DCubic::roots_valid_t(a, b, c, d, &mut roots);
        reporter_assert!(reporter, root_count == 2);
        roots[..root_count].sort_by(f64::total_cmp);
        reporter_assert!(
            reporter,
            double_nearly_zero(roots[0]),
            "{:.16} != 0",
            roots[0]
        );
        reporter_assert!(
            reporter,
            doubles_nearly_equal_ulps(roots[1], 1.0),
            "{:.16} != 1",
            roots[1]
        );
    }
    {
        // Three very small roots, all of them are nearly equal zero
        // (1 - 10000000000x)(1 - 20000000000x)(1 - 30000000000x)
        // -6000000000000000000000000000000 x^3 + 1100000000000000000000 x^2 - 60000000000 x + 1
        let a = -6.0e30;
        let b = 1.1e21;
        let c = -6.0e10;
        let d = 1.0;
        let mut roots = [0.0f64; 3];
        let root_count = DCubic::roots_valid_t(a, b, c, d, &mut roots);
        reporter_assert!(reporter, root_count == 1);
        roots[..root_count].sort_by(f64::total_cmp);
        reporter_assert!(
            reporter,
            double_nearly_zero(roots[0]),
            "{:.16} != 0",
            roots[0]
        );
    }
});
