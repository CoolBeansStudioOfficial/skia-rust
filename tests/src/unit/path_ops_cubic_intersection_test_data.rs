// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsCubicIntersectionTestData.cpp (chrome/m156)

#![cfg(test)]
// The data table keeps the C++ literals verbatim.
#![allow(clippy::unreadable_literal, clippy::excessive_precision)]

use super::path_ops_test_common::{CubicPts, DPoint};

// skia-rust: the C++ `FLT_EPSILON` and the `static const double` constants of the test data.
#[allow(clippy::cast_lossless)] // const context: `f64::from` is not const
const FLT_EPSILON: f64 = f32::EPSILON as f64;
// Port of: src/pathops/SkPathOpsTypes.h#L306 (chrome/m156)
const FLT_EPSILON_HALF: f64 = FLT_EPSILON / 2.0;
const D: f64 = FLT_EPSILON / 2.0;
const G: f64 = FLT_EPSILON / 3.0;
const N: f64 = -FLT_EPSILON / 2.0;
const M: f64 = -FLT_EPSILON / 3.0;
const E: f64 = FLT_EPSILON * 8.0;
const F: f64 = FLT_EPSILON * 8.0;

pub static POINTDEGENERATES: [CubicPts; 26] = [
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0 + FLT_EPSILON_HALF, 1.0),
        DPoint::new(1.0, 1.0 + FLT_EPSILON_HALF),
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0 + D, 1.0),
        DPoint::new(1.0 - D, 1.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, D),
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, D),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(D, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(D, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(2.0, 2.0 + D),
        DPoint::new(1.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, N),
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, N),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(N, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(N, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(N, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(D, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(D, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(N, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(2.0, 2.0 + N),
        DPoint::new(1.0, 1.0),
    ]),
];

pub static NOTPOINTDEGENERATES: [CubicPts; 2] = [
    CubicPts::new([
        DPoint::new(1.0 + FLT_EPSILON * 8.0, 1.0),
        DPoint::new(1.0, FLT_EPSILON * 8.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0 + FLT_EPSILON * 8.0, 1.0),
        DPoint::new(1.0 - FLT_EPSILON * 8.0, 1.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0),
    ]),
];

pub static TESTS: [[CubicPts; 2]; 13] = [
    [
        CubicPts::new([
            DPoint::new(0.0, 45.0),
            DPoint::new(6.0094158284751593, 51.610357411322688),
            DPoint::new(12.741093228940867, 55.981703949474607),
            DPoint::new(20.021417396476362, 58.652245509710262),
        ]),
        CubicPts::new([
            DPoint::new(2.2070737699246674, 52.703494107327209),
            DPoint::new(31.591482272629477, 23.811002295222025),
            DPoint::new(76.824588616426425, 44.049473790502674),
            DPoint::new(119.25488947221436, 55.599248272955073),
        ]),
    ],
    [
        CubicPts::new([
            DPoint::new(0.0, 45.0),
            DPoint::new(50.0, 100.0),
            DPoint::new(150.0, 0.0),
            DPoint::new(200.0, 55.0),
        ]),
        CubicPts::new([
            DPoint::new(0.0, 55.0),
            DPoint::new(50.0, 0.0),
            DPoint::new(150.0, 100.0),
            DPoint::new(200.0, 45.0),
        ]),
    ],
    [
        CubicPts::new([
            DPoint::new(0.0, 0.0),
            DPoint::new(0.0, 100.0),
            DPoint::new(200.0, 0.0),
            DPoint::new(200.0, 100.0),
        ]),
        CubicPts::new([
            DPoint::new(0.0, 100.0),
            DPoint::new(0.0, 0.0),
            DPoint::new(200.0, 100.0),
            DPoint::new(200.0, 0.0),
        ]),
    ],
    [
        CubicPts::new([
            DPoint::new(0.0, 0.0),
            DPoint::new(0.0, 100.0),
            DPoint::new(200.0, 100.0),
            DPoint::new(200.0, 0.0),
        ]),
        CubicPts::new([
            DPoint::new(0.0, 100.0),
            DPoint::new(0.0, 0.0),
            DPoint::new(200.0, 0.0),
            DPoint::new(200.0, 100.0),
        ]),
    ],
    [
        CubicPts::new([
            DPoint::new(150.0, 100.0),
            DPoint::new(150.0 + 0.1, 150.0),
            DPoint::new(150.0, 200.0),
            DPoint::new(150.0, 250.0),
        ]),
        CubicPts::new([
            DPoint::new(250.0, 150.0),
            DPoint::new(200.0, 150.0 + 0.1),
            DPoint::new(150.0, 150.0),
            DPoint::new(100.0, 150.0),
        ]),
    ],
    [
        CubicPts::new([
            DPoint::new(200.0, 100.0),
            DPoint::new(150.0, 100.0),
            DPoint::new(150.0, 150.0),
            DPoint::new(200.0, 150.0),
        ]),
        CubicPts::new([
            DPoint::new(250.0, 150.0),
            DPoint::new(250.0, 100.0),
            DPoint::new(100.0, 100.0),
            DPoint::new(100.0, 150.0),
        ]),
    ],
    [
        CubicPts::new([
            DPoint::new(1.0, 1.5),
            DPoint::new(15.5, 0.5),
            DPoint::new(-8.0, 3.5),
            DPoint::new(5.0, 1.5),
        ]),
        CubicPts::new([
            DPoint::new(4.0, 0.5),
            DPoint::new(5.0, 15.0),
            DPoint::new(2.0, -8.5),
            DPoint::new(4.0, 4.5),
        ]),
    ],
    [
        CubicPts::new([
            DPoint::new(664.00168, 0.0),
            DPoint::new(726.11545, 124.22757),
            DPoint::new(736.89069, 267.89743),
            DPoint::new(694.0017, 400.0002),
        ]),
        CubicPts::new([
            DPoint::new(850.66843, 115.55563),
            DPoint::new(728.515, 115.55563),
            DPoint::new(725.21347, 275.15309),
            DPoint::new(694.0017, 400.0002),
        ]),
    ],
    [
        CubicPts::new([
            DPoint::new(1.0, 1.0),
            DPoint::new(12.5, 6.5),
            DPoint::new(-4.0, 6.5),
            DPoint::new(7.5, 1.0),
        ]),
        CubicPts::new([
            DPoint::new(1.0, 6.5),
            DPoint::new(12.5, 1.0),
            DPoint::new(-4.0, 1.0),
            DPoint::new(0.5, 6.0),
        ]),
    ],
    [
        CubicPts::new([
            DPoint::new(315.748, 312.84),
            DPoint::new(312.644, 318.134),
            DPoint::new(305.836, 319.909),
            DPoint::new(300.542, 316.804),
        ]),
        CubicPts::new([
            DPoint::new(317.122, 309.05),
            DPoint::new(316.112, 315.102),
            DPoint::new(310.385, 319.19),
            DPoint::new(304.332, 318.179),
        ]),
    ],
    [
        CubicPts::new([
            DPoint::new(1046.604051, 172.937967),
            DPoint::new(1046.604051, 178.9763059),
            DPoint::new(1041.76745, 183.9279165),
            DPoint::new(1035.703842, 184.0432409),
        ]),
        CubicPts::new([
            DPoint::new(1046.452235, 174.7640504),
            DPoint::new(1045.544872, 180.1973817),
            DPoint::new(1040.837966, 184.0469882),
            DPoint::new(1035.505925, 184.0469882),
        ]),
    ],
    [
        CubicPts::new([
            DPoint::new(125.79356, 199.57382),
            DPoint::new(51.16556, 128.93575),
            DPoint::new(87.494, 16.67848),
            DPoint::new(167.29361, 16.67848),
        ]),
        CubicPts::new([
            DPoint::new(167.29361, 55.81876),
            DPoint::new(100.36128, 55.81876),
            DPoint::new(68.64099, 145.4755),
            DPoint::new(125.7942, 199.57309),
        ]),
    ],
    [
        CubicPts::new([
            DPoint::new(104.11546583642826, 370.21352558595504),
            DPoint::new(122.96968232592344, 404.54489231839295),
            DPoint::new(169.90881005384728, 425.00067000000007),
            DPoint::new(221.33045999999999, 425.00067000000001),
        ]),
        CubicPts::new([
            DPoint::new(116.32365976159625, 381.71048540582598),
            DPoint::new(103.86096590870899, 381.71048540581626),
            DPoint::new(91.394188003200725, 377.17917781762833),
            DPoint::new(82.622283093355179, 368.11683661930334),
        ]),
    ],
];

pub static LINES: [CubicPts; 30] = [
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 0.0),
        DPoint::new(2.0, 0.0),
        DPoint::new(3.0, 0.0),
        DPoint::new(4.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 2.0),
        DPoint::new(0.0, 3.0),
        DPoint::new(0.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(4.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(4.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(1.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(1.0, 1.0),
    ]),
];

pub static NOTLINES: [CubicPts; 6] = [
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
];

pub static MODEPSILONLINES: [CubicPts; 38] = [
    CubicPts::new([
        DPoint::new(0.0, E),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, E),
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, E),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, E),
    ]),
    CubicPts::new([
        DPoint::new(1.0, E),
        DPoint::new(2.0, 0.0),
        DPoint::new(3.0, 0.0),
        DPoint::new(4.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(E, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(E, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 1.0),
        DPoint::new(E, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(E, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(E, 1.0),
        DPoint::new(0.0, 2.0),
        DPoint::new(0.0, 3.0),
        DPoint::new(0.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(E, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(E, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(E, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(E, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, E),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, E),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(0.0, E),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, E),
        DPoint::new(0.0, 0.0),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, E),
        DPoint::new(2.0, 2.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(E, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0 + E),
        DPoint::new(3.0, 3.0),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0 + E, 2.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(2.0 + E, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0 + E, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(2.0, 2.0 + E),
        DPoint::new(1.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0 + E),
        DPoint::new(3.0, 3.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0 + E, 2.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(4.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0 + F + F),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0 + F + F),
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(4.0, 4.0 + E),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(3.0, 3.0 + E),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(2.0 + E, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0 + E, 3.0),
        DPoint::new(4.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0 + E, 1.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0 + E),
        DPoint::new(3.0, 3.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0 + E, 2.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(1.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0 + E, 4.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(1.0, 1.0 + E),
    ]),
];

pub static LESSEPSILONLINES: [CubicPts; 31] = [
    CubicPts::new([
        DPoint::new(0.0, D),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, D),
    ]),
    CubicPts::new([
        DPoint::new(1.0, D),
        DPoint::new(2.0, 0.0),
        DPoint::new(3.0, 0.0),
        DPoint::new(4.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(D, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(D, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(D, 1.0),
        DPoint::new(0.0, 2.0),
        DPoint::new(0.0, 3.0),
        DPoint::new(0.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(D, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(D, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, D),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, D),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0 + D),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, D),
        DPoint::new(0.0, 0.0),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, D),
        DPoint::new(2.0, 2.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(D, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0 + D),
        DPoint::new(3.0, 3.0),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0 + D, 2.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(2.0 + D, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0 + D, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0 + D),
        DPoint::new(3.0, 3.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0 + D / 2.0, 2.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(4.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0 + D),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0 + D),
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(4.0, 4.0 + D),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(3.0, 3.0 + D),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(2.0 + G, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0 + D, 3.0),
        DPoint::new(4.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0 + D, 1.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0 + D),
        DPoint::new(3.0, 3.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0 + G, 2.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(1.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0 + D, 4.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(1.0, 1.0 + D),
    ]),
];

pub static NEGEPSILONLINES: [CubicPts; 31] = [
    CubicPts::new([
        DPoint::new(0.0, N),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, N),
    ]),
    CubicPts::new([
        DPoint::new(1.0, N),
        DPoint::new(2.0, 0.0),
        DPoint::new(3.0, 0.0),
        DPoint::new(4.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(N, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(N, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(N, 1.0),
        DPoint::new(0.0, 2.0),
        DPoint::new(0.0, 3.0),
        DPoint::new(0.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(N, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(0.0, 0.0),
        DPoint::new(N, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, N),
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, N),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(0.0, 0.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0 + N),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, N),
        DPoint::new(0.0, 0.0),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(0.0, N),
        DPoint::new(2.0, 2.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(N, 0.0),
        DPoint::new(0.0, 0.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0 + N),
        DPoint::new(3.0, 3.0),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0 + N, 2.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(2.0 + N, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0 + N, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(1.0, 1.0 + N),
        DPoint::new(3.0, 3.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(2.0 + N / 2.0, 2.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(4.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0 + N),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0 + N),
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(4.0, 4.0 + N),
        DPoint::new(2.0, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(2.0, 2.0),
        DPoint::new(3.0, 3.0 + N),
    ]),
    CubicPts::new([
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(2.0 + M, 2.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0 + N, 3.0),
        DPoint::new(4.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(1.0 + N, 1.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0 + N),
        DPoint::new(3.0, 3.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(4.0, 4.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0 + M, 2.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(1.0, 1.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0 + N, 4.0),
        DPoint::new(1.0, 1.0),
        DPoint::new(3.0, 3.0),
    ]),
    CubicPts::new([
        DPoint::new(2.0, 2.0),
        DPoint::new(4.0, 4.0),
        DPoint::new(3.0, 3.0),
        DPoint::new(1.0, 1.0 + N),
    ]),
];
