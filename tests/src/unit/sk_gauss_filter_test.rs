// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkGaussFilterTest.cpp (chrome/m156)

#![cfg(test)]
#![allow(clippy::float_cmp)] // the C++ compares the sums with ==
#![allow(clippy::neg_cmp_op_on_partial_ord)] // REPORTER_ASSERT(r, a < b) negates the condition

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::gauss_filter::{GAUSS_ARRAY_MAX, GaussFilter};

// one part in a million
// Port of: tests/SkGaussFilterTest.cpp#L19 (chrome/m156)
const EPSILON: f64 = 0.000_001;

// Port of: tests/SkGaussFilterTest.cpp#L21-L29 (chrome/m156)
fn careful_add(n: usize, gauss: &[f64]) -> f64 {
    // Sum smallest to largest to retain precision.
    let mut sum = 0.0;
    for i in (1..n).rev() {
        sum += 2.0 * gauss[i];
    }
    sum += gauss[0];
    sum
}

// Port of: tests/SkGaussFilterTest.cpp#L31-L63 (chrome/m156)
def_test!(SkGaussFilterCommon, |r| {
    type Test = (f64, Vec<f64>);

    let golden_check = |r: &mut Reporter, test: &Test| {
        let (sigma, golden) = test;
        let filter = GaussFilter::new(*sigma);
        let mut result = [0.0f64; GAUSS_ARRAY_MAX];
        let mut n = 0;
        for d in &filter {
            result[n] = *d;
            n += 1;
        }
        reporter_assert!(r, n == golden.len());
        let sum = careful_add(n, &result);
        reporter_assert!(r, sum == 1.0);
        for i in 0..golden.len() {
            reporter_assert!(r, (golden[i] - result[i]).abs() < EPSILON);
        }
    };

    // The following two sigmas account for about 85% of all sigmas used for masks.
    // Golden values generated using Mathematica.
    let tests: Vec<Test> = vec![
        // GaussianMatrix[{{Automatic}, {.788675}}]
        (0.788_675, vec![0.593_605, 0.176_225, 0.026_972_1]),
        // GaussianMatrix[{{4}, {1.07735}}, Method -> "Bessel"]
        (1.07735, vec![0.429_537, 0.214_955, 0.059_143, 0.011_133_7]),
    ];

    for test in &tests {
        golden_check(r, test);
    }
});

// Port of: tests/SkGaussFilterTest.cpp#L65-L84 (chrome/m156)
def_test!(SkGaussFilterSweep, |r| {
    // The double just before 2.0.
    let max_sigma = f64::from_bits(2.0f64.to_bits() - 1);
    let check = |r: &mut Reporter, sigma: f64| {
        let filter = GaussFilter::new(sigma);
        let mut result = [0.0f64; GAUSS_ARRAY_MAX];
        let mut n = 0;
        for d in &filter {
            result[n] = *d;
            n += 1;
        }
        reporter_assert!(r, n <= GAUSS_ARRAY_MAX);
        let sum = careful_add(n, &result);
        reporter_assert!(r, sum == 1.0);
    };

    let mut sigma = 0.0;
    while sigma < 2.0 {
        check(r, sigma);
        sigma += 0.1;
    }
    check(r, max_sigma);
});
