// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsTestCommon.h#L22-L35 (chrome/m156), the point-set structs only

#![cfg(test)]

/// `SkDPoint`: a double-precision point (from `src/pathops/SkPathOpsPoint.h`).
#[derive(Copy, Clone, Debug)]
pub struct DPoint {
    pub x: f64,
    pub y: f64,
}

impl DPoint {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

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
