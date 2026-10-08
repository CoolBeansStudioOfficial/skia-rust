// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsTestCommon.h (chrome/m156)

#![cfg(test)]

/// `SkDPoint`, as used by the `PathOps` test data (the shared `skia-rust-pathops` type).
pub use skia_rust_pathops::point::DPoint;

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
