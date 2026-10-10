// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/QuadRootsTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::floating_point::{
    double_nearly_zero, doubles_nearly_equal_ulps_max_diff, is_finite_all,
};
use skia_rust_core::quads;
use skia_rust_pathops::quad::DQuad;

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/QuadRootsTest.cpp#L25-L90 (chrome/m156)
// The C++ `testQuadRootsReal` helper, with its `ReporterContext`s as `set_context` calls. The
// C++ reads `expectedRoots[i]` for every root the solver reports; the Rust loop stops at the
// shorter of the two, so a count mismatch (already reported above) cannot index out of bounds.
fn test_quad_roots_real(
    reporter: &mut Reporter,
    name: &str,
    a: f64,
    b: f64,
    c: f64,
    expected_roots: &[f64],
) {
    reporter.set_context(Some(name.to_string()));
    // Validate test case
    reporter_assert!(
        reporter,
        expected_roots.len() <= 2,
        "Invalid test case, up to 2 roots allowed"
    );

    for (i, &x) in expected_roots.iter().enumerate() {
        // A*x^2 + B*x + C should equal 0
        let y = a * x * x + b * x + c;
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
        let mut roots = [0.0f64; 2];
        let root_count = DQuad::roots_real(a, b, c, &mut roots);
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
    {
        reporter.set_context(Some(format!("{name}: SkQuads Implementation")));
        let mut roots = [0.0f64; 2];
        let root_count = quads::roots_real(a, b, c, &mut roots);
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
    reporter.set_context(None);
}

// Port of: tests/QuadRootsTest.cpp#L92-L158 (chrome/m156)
def_test!(
    #[allow(clippy::excessive_precision, clippy::unreadable_literal)]
    // literals copied verbatim from the C++ test
    QuadRootsReal_ActualQuadratics,
    |reporter| {
        // All answers are given with 16 significant digits (max for a double) or as an integer
        // when the answer is exact.
        test_quad_roots_real(
            reporter,
            "two roots 3x^2 - 20x - 40",
            3.0,
            -20.0,
            -40.0,
            &[-1.610798991397109, 8.277465658063775],
        );

        // (2x - 4)(x + 17)
        test_quad_roots_real(
            reporter,
            "two roots 2x^2 + 30x - 68",
            2.0,
            30.0,
            -68.0,
            &[-17.0, 2.0],
        );

        test_quad_roots_real(
            reporter,
            "two roots x^2 - 5",
            1.0,
            0.0,
            -5.0,
            &[-2.236067977499790, 2.236067977499790],
        );

        test_quad_roots_real(reporter, "one root x^2 - 2x + 1", 1.0, -2.0, 1.0, &[1.0]);

        test_quad_roots_real(reporter, "no roots 5x^2 + 6x + 7", 5.0, 6.0, 7.0, &[]);

        test_quad_roots_real(reporter, "no roots 4x^2 + 1", 4.0, 0.0, 1.0, &[]);

        test_quad_roots_real(
            reporter,
            "one root is zero, another is big",
            14.0,
            -13.0,
            0.0,
            &[0.0, 0.9285714285714286],
        );

        // Values from a failing test case observed during testing.
        test_quad_roots_real(
            reporter,
            "one root is zero, another is small",
            0.2929016490705016,
            0.0000030451558069,
            0.0,
            &[-0.00001039651301576329, 0.0],
        );

        test_quad_roots_real(
            reporter,
            "b and c are zero, a is positive 4x^2",
            4.0,
            0.0,
            0.0,
            &[0.0],
        );

        test_quad_roots_real(
            reporter,
            "b and c are zero, a is negative -4x^2",
            -4.0,
            0.0,
            0.0,
            &[0.0],
        );

        // One solution is 0, the other is so close to zero it returns
        // true for sk_double_nearly_zero, so it is collapsed into one.
        test_quad_roots_real(
            reporter,
            "a and b are huge, c is zero",
            4.3719914983870202e+291,
            1.0269509510194551e+152,
            0.0,
            &[0.0],
        );

        // The roots are not in the range of doubles.
        // Rust has no hexadecimal float literals, so the C++ literals `0x1p-1055`,
        // `0x1.3000006p-1044` and `-0x1.c000008p+1009` are built exactly here:
        // 0x1p-1055 is the subnormal 2^19 * 2^-1074; 0x1.3000006p-1044 is 0x13000006 * 2^-1072,
        // i.e. the subnormal 0x13000006 * 4 * 2^-1074; -0x1.c000008p+1009 is 0x1c000008 * 2^981.
        test_quad_roots_real(
            reporter,
            "Very small A B, very large C",
            f64::from_bits(1 << 19),
            f64::from_bits(0x1300_0006 * 4),
            -(f64::from(0x1c00_0008_u32) * 2f64.powi(981)),
            &[],
        );
    }
);

// Port of: tests/QuadRootsTest.cpp#L160-L168 (chrome/m156)
def_test!(QuadRootsReal_Linear, |reporter| {
    test_quad_roots_real(reporter, "positive slope 5x + 6", 0.0, 5.0, 6.0, &[-1.2]);

    test_quad_roots_real(reporter, "negative slope -3x - 9", 0.0, -3.0, -9.0, &[-3.0]);
});

// Port of: tests/QuadRootsTest.cpp#L170-L178 (chrome/m156)
def_test!(QuadRootsReal_Constant, |reporter| {
    test_quad_roots_real(reporter, "No intersections y = -10", 0.0, 0.0, -10.0, &[]);

    test_quad_roots_real(reporter, "Infinite solutions y = 0", 0.0, 0.0, 0.0, &[0.0]);
});

// Port of: tests/QuadRootsTest.cpp#L180-L204 (chrome/m156)
def_test!(QuadRootsReal_NonFiniteNumbers, |reporter| {
    // The Pathops implementation does not check for infinities nor nans in all cases.
    let mut roots = [0.0f64; 2];
    reporter_assert!(
        reporter,
        quads::roots_real(f64::MAX, 0.0, f64::MAX, &mut roots) == 0,
        "Discriminant is negative infinity"
    );
    reporter_assert!(
        reporter,
        quads::roots_real(f64::MAX, f64::MAX, f64::MAX, &mut roots) == 0,
        "Double Overflow"
    );

    reporter_assert!(
        reporter,
        quads::roots_real(1.0, f64::NAN, -3.0, &mut roots) == 0,
        "Nan quadratic"
    );
    reporter_assert!(
        reporter,
        quads::roots_real(0.0, f64::NAN, 3.0, &mut roots) == 0,
        "Nan linear"
    );
    reporter_assert!(
        reporter,
        quads::roots_real(0.0, 0.0, f64::NAN, &mut roots) == 0,
        "Nan constant"
    );
});

// Test the discriminant using
// Use quadratics of the form F_n * x^2 - 2 * F_(n-1) * x + F_(n-2).
//   This has a discriminant of F_(n-1)^2 - F_n * F_(n-2) = 1 if n is even else -1.
// Port of: tests/QuadRootsTest.cpp#L209-L222 (chrome/m156)
def_test!(
    #[allow(clippy::float_cmp)] // exact float comparisons, as in the C++ test
    QuadDiscriminant_Fibonacci,
    |reporter| {
        //            n,  n-1, n-2
        let mut f: [i64; 3] = [1, 1, 0];
        // F_79 just fits in the 53 significant bits of a double.
        for i in 2..79 {
            f[0] = f[1] + f[2];

            let expected_discriminant = if i % 2 == 0 { 1 } else { -1 };
            #[allow(clippy::cast_precision_loss)] // F_79 fits in a double's 53 significant bits
            let discriminant = quads::discriminant(f[0] as f64, f[1] as f64, f[2] as f64);
            reporter_assert!(reporter, discriminant == f64::from(expected_discriminant));

            f[2] = f[1];
            f[1] = f[0];
        }
    }
);

// Port of: tests/QuadRootsTest.cpp#L224-L238 (chrome/m156)
def_test!(
    #[allow(clippy::float_cmp)] // exact float comparisons, as in the C++ test
    QuadRoots_Basic,
    |reporter| {
        {
            // (x - 1) (x - 1) normal quadratic form A = 1, B = 2, C =1.
            let quads::RootResult {
                discriminant,
                root0: r0,
                root1: r1,
            } = quads::roots(1.0, -0.5 * -2.0, 1.0);
            reporter_assert!(reporter, discriminant == 0.0);
            reporter_assert!(reporter, r0 == 1.0 && r1 == 1.0);
        }

        {
            // (x + 2) (x + 2) normal quadratic form A = 1, B = 4, C = 4.
            let quads::RootResult {
                discriminant,
                root0: r0,
                root1: r1,
            } = quads::roots(1.0, -0.5 * 4.0, 4.0);
            reporter_assert!(reporter, discriminant == 0.0);
            reporter_assert!(reporter, r0 == -2.0 && r1 == -2.0);
        }
    }
);

// Test the roots using
// Use quadratics of the form F_n * x^2 - 2 * F_(n-1) * x + F_(n-2).
// The roots are (F_(n–1) ± 1)/F_n if n is even otherwise there are no roots.
// Port of: tests/QuadRootsTest.cpp#L243-L273 (chrome/m156)
def_test!(
    #[allow(clippy::float_cmp)] // exact float comparisons, as in the C++ test
    QuadRoots_Fibonacci,
    |reporter| {
        //            n,  n-1, n-2
        let mut f: [i64; 3] = [1, 1, 0];
        // F_79 just fits in the 53 significant bits of a double.
        for i in 2..79 {
            f[0] = f[1] + f[2];

            let expected_discriminant = if i % 2 == 0 { 1 } else { -1 };
            #[allow(clippy::cast_precision_loss)] // F_79 fits in a double's 53 significant bits
            let quads::RootResult {
                discriminant,
                root0: r0,
                root1: r1,
            } = quads::roots(f[0] as f64, f[1] as f64, f[2] as f64);
            reporter_assert!(reporter, discriminant == f64::from(expected_discriminant));

            // There are only real roots when i is even.
            if i % 2 == 0 {
                #[allow(clippy::cast_precision_loss)] // F_79 fits in a double's 53 significant bits
                let expected_little = (f[1] as f64 - 1.0) / f[0] as f64;
                #[allow(clippy::cast_precision_loss)] // F_79 fits in a double's 53 significant bits
                let expected_big = (f[1] as f64 + 1.0) / f[0] as f64;
                if r0 <= r1 {
                    reporter_assert!(reporter, r0 == expected_little);
                    reporter_assert!(reporter, r1 == expected_big);
                } else {
                    reporter_assert!(reporter, r1 == expected_little);
                    reporter_assert!(reporter, r0 == expected_big);
                }
            } else {
                reporter_assert!(reporter, r0.is_nan());
                reporter_assert!(reporter, r1.is_nan());
            }

            f[2] = f[1];
            f[1] = f[0];
        }
    }
);

// These are test cases used in the paper "The Ins and Outs of Solving Quadratic Equations with
// Floating-Point Arithmetic" located at
// https://github.com/goualard-f/QuadraticEquation.jl/blob/main/test/tests.jl

// Port of: tests/QuadRootsTest.cpp#L279-L285 (chrome/m156)
struct TestCase {
    a: f64,
    b: f64,
    c: f64,
    answer_lo: f64,
    answer_hi: f64,
}

// Port of: tests/QuadRootsTest.cpp#L287-L376 (chrome/m156)
def_test!(
    #[allow(clippy::excessive_precision, clippy::unreadable_literal)] // literals copied verbatim from the C++ test
    #[allow(clippy::approx_constant)] // literals copied verbatim from the C++ test
    QuadRoots_Hard,
    |reporter| {
        let nan = f64::NAN;
        let infinity = f64::INFINITY;

        let special_equal = |actual: f64, test: f64| {
            if actual.is_nan() {
                return test.is_nan();
            }

            if actual.is_infinite() {
                return test.is_infinite();
            }

            // Comparison function from the paper "The Ins and Outs ...."
            let error_factor = f64::EPSILON.sqrt();
            // std::max(a, b) is `(a < b) ? b : a`.
            let (abs_test, abs_actual) = (test.abs(), actual.abs());
            let max = if abs_test < abs_actual {
                abs_actual
            } else {
                abs_test
            };
            (test - actual).abs() <= error_factor * max
        };

        let p2 = |a: f64| a.exp2();

        let cases = [
            // no real solutions
            TestCase {
                a: 2.0,
                b: 0.0,
                c: 3.0,
                answer_lo: nan,
                answer_hi: nan,
            },
            TestCase {
                a: 1.0,
                b: 1.0,
                c: 1.0,
                answer_lo: nan,
                answer_hi: nan,
            },
            TestCase {
                a: 2.0 * p2(600.0),
                b: 0.0,
                c: 2.0 * p2(600.0),
                answer_lo: nan,
                answer_hi: nan,
            },
            TestCase {
                a: -2.0 * p2(600.0),
                b: 0.0,
                c: -2.0 * p2(600.0),
                answer_lo: nan,
                answer_hi: nan,
            },
            // degenerate cases
            TestCase {
                a: 0.0,
                b: 0.0,
                c: 0.0,
                answer_lo: infinity,
                answer_hi: infinity,
            },
            TestCase {
                a: 0.0,
                b: 1.0,
                c: 0.0,
                answer_lo: 0.0,
                answer_hi: 0.0,
            },
            TestCase {
                a: 0.0,
                b: 1.0,
                c: 2.0,
                answer_lo: -2.0,
                answer_hi: -2.0,
            },
            TestCase {
                a: 0.0,
                b: 3.0,
                c: 4.0,
                answer_lo: -4.0 / 3.0,
                answer_hi: -4.0 / 3.0,
            },
            TestCase {
                a: 0.0,
                b: p2(600.0),
                c: -p2(600.0),
                answer_lo: 1.0,
                answer_hi: 1.0,
            },
            TestCase {
                a: 0.0,
                b: p2(600.0),
                c: p2(600.0),
                answer_lo: -1.0,
                answer_hi: -1.0,
            },
            TestCase {
                a: 0.0,
                b: p2(-600.0),
                c: p2(600.0),
                answer_lo: -infinity,
                answer_hi: -infinity,
            },
            TestCase {
                a: 0.0,
                b: p2(600.0),
                c: p2(-600.0),
                answer_lo: 0.0,
                answer_hi: 0.0,
            },
            TestCase {
                a: 0.0,
                b: 2.0,
                c: -1.0e-323,
                answer_lo: 5.0e-324,
                answer_hi: 5.0e-324,
            },
            TestCase {
                a: 3.0,
                b: 0.0,
                c: 0.0,
                answer_lo: 0.0,
                answer_hi: 0.0,
            },
            TestCase {
                a: p2(600.0),
                b: 0.0,
                c: 0.0,
                answer_lo: 0.0,
                answer_hi: 0.0,
            },
            TestCase {
                a: 2.0,
                b: 0.0,
                c: -3.0,
                answer_lo: -(3.0f64 / 2.0).sqrt(),
                answer_hi: (3.0f64 / 2.0).sqrt(),
            },
            // {p2(600), 0, -p2(600), -1, 1}, determinant is infinity
            TestCase {
                a: 3.0,
                b: 2.0,
                c: 0.0,
                answer_lo: -2.0 / 3.0,
                answer_hi: 0.0,
            },
            // {p2(600), p2(700), 0, -p2(100), 0},
            TestCase {
                a: p2(-600.0),
                b: p2(700.0),
                c: 0.0,
                answer_lo: -infinity,
                answer_hi: 0.0,
            },
            TestCase {
                a: p2(600.0),
                b: p2(-700.0),
                c: 0.0,
                answer_lo: 0.0,
                answer_hi: 0.0,
            },
            // two solutions tests
            TestCase {
                a: 1.0,
                b: -1.0,
                c: -1.0,
                answer_lo: -0.6180339887498948,
                answer_hi: 1.618033988749895,
            },
            TestCase {
                a: 1.0,
                b: 1.0 + p2(-52.0),
                c: 0.25 + p2(-53.0),
                answer_lo: (-1.0 - p2(-51.0)) / 2.0,
                answer_hi: -0.5,
            },
            TestCase {
                a: 1.0,
                b: p2(-511.0) + p2(-563.0),
                c: p2(-1024.0),
                answer_lo: -7.458340888372987e-155,
                answer_hi: -7.458340574027429e-155,
            },
            TestCase {
                a: 1.0,
                b: p2(27.0),
                c: 0.75,
                answer_lo: -134217728.0,
                answer_hi: -5.587935447692871e-09,
            },
            TestCase {
                a: 1.0,
                b: -1e9,
                c: 1.0,
                answer_lo: 1e-09,
                answer_hi: 1000000000.0,
            },
            // {1.3407807929942596e154, -1.3407807929942596e154, -1.3407807929942596e154, -0.6180339887498948, 1.618033988749895},
            TestCase {
                a: p2(600.0),
                b: 0.5,
                c: -p2(-600.0),
                answer_lo: -3.086568504549085e-181,
                answer_hi: 1.8816085719976428e-181,
            },
            // {p2(600), 0.5, -p2(600), -1.0, 1.0},
            // {8.0, p2(800),-p2(500), -8.335018041099818e+239, 4.909093465297727e-91},
            TestCase {
                a: 1.0,
                b: p2(26.0),
                c: -0.125,
                answer_lo: -67108864.0,
                answer_hi: 1.862645149230957e-09,
            },
            // {p2(-1073), -p2(-1073), -p2(-1073), -0.6180339887498948,1.618033988749895},
            TestCase {
                a: p2(600.0),
                b: -p2(-600.0),
                c: -p2(-600.0),
                answer_lo: -2.409919865102884e-181,
                answer_hi: 2.409919865102884e-181,
            },
            // Tests in Nivergelt paper
            TestCase {
                a: -158114166017.0,
                b: 316227766017.0,
                c: -158113600000.0,
                answer_lo: 0.99999642020057874,
                answer_hi: 1.0,
            },
            TestCase {
                a: -312499999999.0,
                b: 707106781186.0,
                c: -400000000000.0,
                answer_lo: 1.131369396027,
                answer_hi: 1.131372303775,
            },
            TestCase {
                a: -67.0,
                b: 134.0,
                c: -65.0,
                answer_lo: 0.82722631488372798,
                answer_hi: 1.17277368511627202,
            },
            TestCase {
                a: 0.247260273973,
                b: 0.994520547945,
                c: -0.138627953316,
                answer_lo: -4.157030027041105,
                answer_hi: 0.1348693622211607,
            },
            TestCase {
                a: 1.0,
                b: -2300000.0,
                c: 2.0e11,
                answer_lo: 90518.994979145,
                answer_hi: 2209481.005020854,
            },
            TestCase {
                a: 1.5 * p2(-1026.0),
                b: 0.0,
                c: -p2(1022.0),
                answer_lo: -1.4678102981723264e308,
                answer_hi: 1.4678102981723264e308,
            },
            // one solution tests
            TestCase {
                a: 1.5 * p2(-1026.0),
                b: 0.0,
                c: -p2(1022.0),
                answer_lo: -1.4678102981723264e308,
                answer_hi: 1.4678102981723264e308,
            },
        ];

        for test_case in &cases {
            let TestCase {
                a,
                b,
                c,
                answer_lo,
                answer_hi,
            } = *test_case;
            if is_finite_all(answer_lo, &[answer_hi]) {
                debug_assert!(answer_lo <= answer_hi);
            }
            let quads::RootResult {
                root0: r0,
                root1: r1,
                ..
            } = quads::roots(a, -0.5 * b, c);
            // std::min(a, b) is `(b < a) ? b : a`; std::max(a, b) is `(a < b) ? b : a`.
            let r_lo = if r1 < r0 { r1 } else { r0 };
            let r_hi = if r0 < r1 { r1 } else { r0 };
            reporter_assert!(reporter, special_equal(r_lo, answer_lo));
            reporter_assert!(reporter, special_equal(r_hi, answer_hi));
        }
    }
);
