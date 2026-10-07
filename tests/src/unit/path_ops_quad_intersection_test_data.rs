// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsQuadIntersectionTestData.cpp (chrome/m156)

#![cfg(test)]

use super::path_ops_test_common::{DPoint, QuadPts};

// skia-rust: the C++ `FLT_EPSILON` and the `static const double` constants of the test data.
#[allow(clippy::cast_lossless)] // const context: `f64::from` is not const
const FLT_EPSILON: f64 = f32::EPSILON as f64;
const F: f64 = FLT_EPSILON * 32.0;
const H: f64 = FLT_EPSILON * 32.0;
const J: f64 = FLT_EPSILON * 32.0;
const K: f64 = FLT_EPSILON * 32.0;

pub static QUADRATICPOINTS: [QuadPts; 4] = [
    QuadPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    QuadPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 0.0),
    ]),
    QuadPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, 0.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0),
    ]),
];

pub static QUADRATICLINES: [QuadPts; 23] = [
    QuadPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 0.0),
        DPoint::new(2.0, 0.0),
        DPoint::new(3.0, 0.0),
    ]),
    QuadPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 1.0),
    ]),
    QuadPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    QuadPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 2.0),
        DPoint::new(0.0, 3.0),
    ]),
    QuadPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(3.0, 3.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(3.0, 3.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(4.0, 4.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(2.0, 2.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(2.0, 2.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
    ]),
    QuadPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
    ]),
    QuadPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
    ]),
    QuadPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(1.0, 1.0),
    ]),
    QuadPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(4.0, 4.0),
    ]),
    QuadPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(1.0, 1.0),
    ]),
    QuadPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
    ]),
];

pub static QUADRATICMODEPSILONLINES: [QuadPts; 23] = [
    QuadPts::new([
        DPoint::new(0.0, F),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    QuadPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, F),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, F),
        DPoint::new(0.0, 0.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, H),
        DPoint::new(2.0, 0.0),
        DPoint::new(3.0, 0.0),
    ]),
    QuadPts::new([
        DPoint::new(0.0, F),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
    ]),
    QuadPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(F, 0.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(F, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0 + J),
        DPoint::new(2.0, 2.0),
        DPoint::new(3.0, 3.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(3.0 + F, 3.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0 + F, 1.0),
        DPoint::new(2.0, 2.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0 + K),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0 + F),
        DPoint::new(3.0, 3.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0 + H, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0 + K),
        DPoint::new(3.0, 3.0),
        DPoint::new(4.0, 4.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0 + F, 3.0),
        DPoint::new(2.0, 2.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0 + F),
        DPoint::new(2.0, 2.0),
    ]),
    QuadPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0 + F, 3.0),
    ]),
    QuadPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0 + F),
    ]),
    QuadPts::new([
        DPoint::new(2.0 + F, 2.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
    ]),
    QuadPts::new([
        DPoint::new(2.0, 2.0 + F),
        DPoint::new(3.0, 3.0),
        DPoint::new(1.0, 1.0),
    ]),
    QuadPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(3.0 + F, 3.0),
        DPoint::new(4.0, 4.0),
    ]),
    QuadPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0 + F),
        DPoint::new(1.0, 1.0),
    ]),
    QuadPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0 + F, 3.0),
    ]),
];

pub static QUADRATICTESTS: [[QuadPts; 2]; 2] = [
    [
        QuadPts::new([
            DPoint::new(0.0, 0.0),
            DPoint::new(0.0, 1.0),
            DPoint::new(1.0, 1.0),
        ]),
        QuadPts::new([
            DPoint::new(0.0, 1.0),
            DPoint::new(0.0, 0.0),
            DPoint::new(1.0, 0.0),
        ]),
    ],
    [
        QuadPts::new([
            DPoint::new(1.0, 0.0),
            DPoint::new(2.0, 6.0),
            DPoint::new(3.0, 0.0),
        ]),
        QuadPts::new([
            DPoint::new(0.0, 1.0),
            DPoint::new(6.0, 2.0),
            DPoint::new(0.0, 3.0),
        ]),
    ],
];
