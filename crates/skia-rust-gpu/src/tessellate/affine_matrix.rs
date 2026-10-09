// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/gpu/tessellate/AffineMatrix.h (chrome/m156)

//! Applies an affine 2d transformation to points. Uses SIMD lanes, but takes care to map points
//! identically, regardless of which method is called.
//!
//! This struct stores redundant data, so it is best used only as a stack-allocated object at the
//! point of use.

use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_simd::vx::{Float2, Float4};

// Port of: src/gpu/tessellate/AffineMatrix.h#L20-L63 (chrome/m156), class `AffineMatrix`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[allow(clippy::struct_field_names)] // the C++ members are all named f*
pub struct AffineMatrix {
    f_scale: Float4,
    f_skew: Float4,
    f_trans: Float4,
}

impl From<&Matrix> for AffineMatrix {
    // Port of: `AffineMatrix::operator=(const SkMatrix& m)`.
    fn from(m: &Matrix) -> Self {
        debug_assert!(!m.has_perspective());
        // Duplicate the matrix in float4.lo and float4.hi so we can map two points at once.
        let scale = Float2::new(m.rc(0, 0), m.rc(1, 1)).xyxy();
        let skew = Float2::new(m.rc(0, 1), m.rc(1, 0)).xyxy();
        let trans = Float2::new(m.rc(0, 2), m.rc(1, 2)).xyxy();
        Self {
            f_scale: scale,
            f_skew: skew,
            f_trans: trans,
        }
    }
}

impl AffineMatrix {
    /// `map2Points(skvx::float4 p0p1)`: maps `(p0, p1)` packed as `(x0, y0, x1, y1)`.
    // Port of: src/gpu/tessellate/AffineMatrix.h#L37-L39 (chrome/m156)
    #[must_use]
    pub fn map2_points_vec(&self, p0p1: Float4) -> Float4 {
        self.f_scale * p0p1 + (self.f_skew * p0p1.yxwz() + self.f_trans)
    }

    /// `map2Points(const SkPoint pts[2])`.
    // Port of: src/gpu/tessellate/AffineMatrix.h#L41-L43 (chrome/m156)
    #[must_use]
    pub fn map2_points(&self, pts: &[Point]) -> Float4 {
        self.map2_points_vec(Float4::new(pts[0].x, pts[0].y, pts[1].x, pts[1].y))
    }

    /// `map2Points(SkPoint p0, SkPoint p1)`.
    // Port of: src/gpu/tessellate/AffineMatrix.h#L45-L49 (chrome/m156)
    #[must_use]
    pub fn map2_points_pair(&self, p0: Point, p1: Point) -> Float4 {
        self.map2_points_vec(Float4::new(p0.x, p0.y, p1.x, p1.y))
    }

    /// `mapPoint(skvx::float2 p)`.
    // Port of: src/gpu/tessellate/AffineMatrix.h#L51-L53 (chrome/m156)
    #[must_use]
    pub fn map_point_vec(&self, p: Float2) -> Float2 {
        self.f_scale.lo() * p + (self.f_skew.lo() * p.yx() + self.f_trans.lo())
    }

    /// `map1Point(const SkPoint pt[1])`.
    // Port of: src/gpu/tessellate/AffineMatrix.h#L55-L57 (chrome/m156)
    #[must_use]
    pub fn map1_point(&self, pt: &[Point]) -> Float2 {
        self.map_point_vec(Float2::new(pt[0].x, pt[0].y))
    }

    /// `mapPoint(SkPoint p)`.
    // Port of: src/gpu/tessellate/AffineMatrix.h#L59-L61 (chrome/m156)
    #[must_use]
    pub fn map_point(&self, p: Point) -> Point {
        let mapped = self.map_point_vec(Float2::new(p.x, p.y));
        Point::new(mapped[0], mapped[1])
    }
}
