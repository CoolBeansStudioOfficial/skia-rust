// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsTestCommon.h (chrome/m156)

#![cfg(test)]

use skia_rust_pathops::conic::DConic;
use skia_rust_pathops::cubic::DCubic;
/// `SkDPoint`, as used by the `PathOps` test data (the shared `skia-rust-pathops` type).
pub use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::quad::DQuad;

/// `QuadPts`.
#[derive(Copy, Clone, Debug)]
pub struct QuadPts {
    pub pts: [DPoint; 3],
}

impl QuadPts {
    #[must_use]
    pub const fn new(pts: [DPoint; 3]) -> Self {
        Self { pts }
    }
}

/// `CubicPts`.
#[derive(Copy, Clone, Debug)]
pub struct CubicPts {
    pub pts: [DPoint; 4],
}

impl CubicPts {
    #[must_use]
    pub const fn new(pts: [DPoint; 4]) -> Self {
        Self { pts }
    }
}

/// `ConicPts`: three control points and a weight.
#[derive(Copy, Clone, Debug)]
pub struct ConicPts {
    pub pts: QuadPts,
    pub weight: f32,
}

impl ConicPts {
    #[must_use]
    pub const fn new(pts: QuadPts, weight: f32) -> Self {
        Self { pts, weight }
    }
}

/// `ValidPoint(const SkDPoint& pt)`: neither coordinate is NaN.
// Port of: tests/PathOpsTestCommon.cpp#L307-L312 (chrome/m156)
#[must_use]
pub fn valid_point(pt: DPoint) -> bool {
    !pt.x.is_nan() && !pt.y.is_nan()
}

/// `ValidCubic(const SkDCubic& cubic)`: no control point is NaN.
// Port of: tests/PathOpsTestCommon.cpp#L289-L296 (chrome/m156)
#[must_use]
pub fn valid_cubic(cubic: &DCubic) -> bool {
    cubic.pts.iter().all(|pt| valid_point(*pt))
}

/// `ValidConic(const SkDConic& conic)`: no point and no weight is NaN.
// Port of: tests/PathOpsTestCommon.cpp#L277-L287 (chrome/m156)
#[must_use]
pub fn valid_conic(conic: &DConic) -> bool {
    conic.pts.pts.iter().all(|pt| valid_point(*pt)) && !conic.weight.is_nan()
}

/// `ValidQuad(const SkDQuad& quad)`.
// Port of: tests/PathOpsTestCommon.cpp#L326-L334 (chrome/m156)
#[must_use]
pub fn valid_quad(quad: &DQuad) -> bool {
    quad.pts.iter().all(|pt| valid_point(*pt))
}
