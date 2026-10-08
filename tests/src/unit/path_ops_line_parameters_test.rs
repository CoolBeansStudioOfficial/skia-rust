// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsLineParametetersTest.cpp (chrome/m156)

#![cfg(test)]

use crate::unit::path_ops_test_common::CubicPts;
use crate::{def_test, reporter_assert};
use skia_rust_pathops::cubic::DCubic;
use skia_rust_pathops::line_parameters::LineParameters;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::types::almost_equal_ulps;

/// `FLT_EPSILON * 2`, as the C++ literal in the test data.
const FLT_EPSILON_X2: f64 = f32::EPSILON as f64 * 2.0;

// Port of: tests/PathOpsLineParametetersTest.cpp#L21-L32 (chrome/m156)
const TESTS: [CubicPts; 10] = [
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(0.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(3.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(5.0, 0.0),
        DPoint::new(-2.0, 4.0),
        DPoint::new(3.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 2.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(2.0, 0.0),
        DPoint::new(3.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.2),
        DPoint::new(1.0, 0.0),
        DPoint::new(2.0, 0.0),
        DPoint::new(3.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.02),
        DPoint::new(1.0, 0.0),
        DPoint::new(2.0, 0.0),
        DPoint::new(3.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.002),
        DPoint::new(1.0, 0.0),
        DPoint::new(2.0, 0.0),
        DPoint::new(3.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0002),
        DPoint::new(1.0, 0.0),
        DPoint::new(2.0, 0.0),
        DPoint::new(3.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.00002),
        DPoint::new(1.0, 0.0),
        DPoint::new(2.0, 0.0),
        DPoint::new(3.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, FLT_EPSILON_X2),
        DPoint::new(1.0, 0.0),
        DPoint::new(2.0, 0.0),
        DPoint::new(3.0, 0.0),
    ]),
];

// Port of: tests/PathOpsLineParametetersTest.cpp#L34-L45 (chrome/m156)
#[allow(clippy::excessive_precision, clippy::unreadable_literal)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
const ANSWERS: [[f64; 2]; 10] = [
    [1.0, 2.0],
    [1.0, 2.0],
    [4.0, 4.0],
    [1.1094003924, 0.5547001962],
    [0.133038021, 0.06651901052],
    [0.0133330370, 0.006666518523],
    [0.001333333037, 0.0006666665185],
    [0.000133333333, 6.666666652e-05],
    [1.333333333e-05, 6.666666667e-06],
    [1.5894571940104115e-07, 7.9472859700520577e-08],
];

def_test!(PathOpsLineParameters, |reporter| {
    for (index, c) in TESTS.iter().enumerate() {
        let mut line_parameters = LineParameters::default();
        let cubic = DCubic::new(c.pts);
        line_parameters.cubic_end_points(&cubic, 0, 3);
        let denormalized_distance = [
            line_parameters.control_pt_distance_cubic(&cubic, 1),
            line_parameters.control_pt_distance_cubic(&cubic, 2),
        ];
        let normal_squared = line_parameters.normal_squared();
        for inner in 0..2 {
            let mut dist_sq = denormalized_distance[inner];
            dist_sq *= dist_sq;
            let mut answers_sq = ANSWERS[index][inner];
            answers_sq *= answers_sq;
            // The C++ only reports mismatches here (SkDebugf), without a REPORTER_ASSERT.
            let _ = almost_equal_ulps(dist_sq, normal_squared * answers_sq);
        }
        line_parameters.normalize();
        let normalized_distance = [
            line_parameters.control_pt_distance_cubic(&cubic, 1),
            line_parameters.control_pt_distance_cubic(&cubic, 2),
        ];
        for inner in 0..2 {
            reporter_assert!(
                reporter,
                almost_equal_ulps(normalized_distance[inner].abs(), ANSWERS[index][inner])
            );
        }
    }
});
