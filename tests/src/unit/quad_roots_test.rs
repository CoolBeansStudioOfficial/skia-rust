// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/QuadRootsTest.cpp (chrome/m156)
//
// Not ported yet (manifest stays `todo`): `QuadRootsReal_ActualQuadratics`, `QuadRootsReal_Linear`
// and `QuadRootsReal_Constant` (they also check `SkDQuad::RootsReal` from SkPathOps).

#![cfg(test)]

use skia_rust_core::floating_point::is_finite_all;
use skia_rust_core::libm;
use skia_rust_core::quads;

use crate::{def_test, reporter_assert};

// Port of: tests/QuadRootsTest.cpp#L123-L147 (chrome/m156)
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
// Port of: tests/QuadRootsTest.cpp#L149-L165 (chrome/m156)
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

// Port of: tests/QuadRootsTest.cpp#L167-L182 (chrome/m156)
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
// Port of: tests/QuadRootsTest.cpp#L184-L220 (chrome/m156)
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

// Port of: tests/QuadRootsTest.cpp#L226-L232 (chrome/m156)
struct TestCase {
    a: f64,
    b: f64,
    c: f64,
    answer_lo: f64,
    answer_hi: f64,
}

// Port of: tests/QuadRootsTest.cpp#L234-L377 (chrome/m156)
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

        let p2 = libm::exp2;

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
