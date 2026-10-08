// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsDLineTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{def_test, reporter_assert};
use skia_rust_pathops::line::DLine;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::types::approximately_equal;

// Port of: tests/PathOpsDLineTest.cpp#L18-L25 (chrome/m156)
const TESTS: [DLine; 6] = [
    DLine::new([DPoint::new(2.0, 1.0), DPoint::new(2.0, 1.0)]),
    DLine::new([DPoint::new(2.0, 1.0), DPoint::new(1.0, 1.0)]),
    DLine::new([DPoint::new(2.0, 1.0), DPoint::new(2.0, 2.0)]),
    DLine::new([DPoint::new(1.0, 1.0), DPoint::new(2.0, 2.0)]),
    DLine::new([DPoint::new(3.0, 0.0), DPoint::new(2.0, 1.0)]),
    DLine::new([DPoint::new(3.0, 2.0), DPoint::new(1.0, 1.0)]),
];

/// `(a + b) / 2` as the C++ test computes it.
#[allow(clippy::manual_midpoint)] // skia-rust: the C++ averages with (a + b) / 2, not midpoint
fn half_sum(a: f64, b: f64) -> f64 {
    (a + b) / 2.0
}

def_test!(PathOpsLineUtilities, |reporter| {
    for line in TESTS {
        let mut line2 = DLine::default();
        let pts = [line[0].as_sk_point(), line[1].as_sk_point()];
        line2.set(pts);
        reporter_assert!(reporter, line[0] == line2[0] && line[1] == line2[1]);
        let mid = line.pt_at_t(0.5);
        reporter_assert!(
            reporter,
            approximately_equal(half_sum(line[0].x, line[1].x), mid.x)
        );
        reporter_assert!(
            reporter,
            approximately_equal(half_sum(line[0].y, line[1].y), mid.y)
        );
    }
});
