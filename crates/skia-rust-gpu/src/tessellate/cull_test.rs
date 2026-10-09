// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/gpu/tessellate/CullTest.h (chrome/m156)

//! Determines whether the given local-space points will be contained in the cull bounds post
//! transform. For the versions that take more than one point, it returns whether any region of
//! their device-space bounding box will be in the cull bounds.
//!
//! NOTE: The view matrix is not a normal matrix. `M*p` maps to the float4 `[x, y, -x, -y]` in
//! device space. This is done to aid in quick bounds calculations. The matrix also does not have
//! a translation element. Instead the translation is unapplied to the cull bounds ahead of time.

use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_simd::vx::{Float4, all};

// Port of: src/gpu/tessellate/CullTest.h#L23-L108 (chrome/m156), class `CullTest`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[allow(clippy::struct_field_names)] // the C++ members are all named f*
pub struct CullTest {
    // [fMatX, fMatY] maps path coordinates to the float4 [x, y, -x, -y] in device space.
    f_mat_x: Float4,
    f_mat_y: Float4,
    // [l, t, -r, -b]
    f_cull_bounds: Float4,
}

impl CullTest {
    /// `CullTest(const SkRect& devCullBounds, const SkMatrix& m)`.
    // Port of: src/gpu/tessellate/CullTest.h#L30-L32 (chrome/m156)
    #[must_use]
    pub fn new(dev_cull_bounds: &Rect, m: &Matrix) -> Self {
        let mut cull_test = Self::default();
        cull_test.set(dev_cull_bounds, m);
        cull_test
    }

    /// `CullTest::set(const SkRect& devCullBounds, const SkMatrix& m)`.
    // Port of: src/gpu/tessellate/CullTest.h#L34-L47 (chrome/m156)
    pub fn set(&mut self, dev_cull_bounds: &Rect, m: &Matrix) {
        debug_assert!(!m.has_perspective());
        // getScaleX/getSkewY/getSkewX/getScaleY/getTranslate{X,Y}, as the matrix entries.
        let scale_x = m.rc(0, 0);
        let skew_y = m.rc(1, 0);
        let skew_x = m.rc(0, 1);
        let scale_y = m.rc(1, 1);
        let translate_x = m.rc(0, 2);
        let translate_y = m.rc(1, 2);
        // [fMatX, fMatY] maps path coordinates to the float4 [x, y, -x, -y] in device space.
        self.f_mat_x = Float4::new(scale_x, skew_y, -scale_x, -skew_y);
        self.f_mat_y = Float4::new(skew_x, scale_y, -skew_x, -scale_y);
        // Store the cull bounds as [l, t, -r, -b] for faster math. Also subtract the matrix
        // translate from the cull bounds ahead of time, rather than adding it to every point
        // every time we test.
        self.f_cull_bounds = Float4::new(
            dev_cull_bounds.left - translate_x,
            dev_cull_bounds.top - translate_y,
            translate_x - dev_cull_bounds.right,
            translate_y - dev_cull_bounds.bottom,
        );
    }

    /// Returns whether `M*p` will be in the viewport.
    // Port of: src/gpu/tessellate/CullTest.h#L49-L55 (chrome/m156)
    #[must_use]
    pub fn is_visible(&self, p: Point) -> bool {
        // devPt = [x, y, -x, -y] in device space.
        let dev_pt = self.f_mat_x * p.x + self.f_mat_y * p.y;
        // i.e., l < x && t < y && r > x && b > y.
        all(self.f_cull_bounds.lt_mask(dev_pt))
    }

    /// Returns whether any region of the bounding box of `M * p0..2` will be in the viewport.
    // Port of: src/gpu/tessellate/CullTest.h#L57-L75 (chrome/m156)
    #[must_use]
    pub fn are_visible3(&self, p: &[Point]) -> bool {
        // Transform p0..2 to device space.
        let mut val0 = self.f_mat_y * p[0].y;
        let mut val1 = self.f_mat_y * p[1].y;
        let mut val2 = self.f_mat_y * p[2].y;
        val0 = self.f_mat_x * p[0].x + val0;
        val1 = self.f_mat_x * p[1].x + val1;
        val2 = self.f_mat_x * p[2].x + val2;
        // At this point: valN = {xN, yN, -xN, -yN} in device space.

        // Find the device-space bounding box of p0..2.
        val0 = val0.max(val1);
        val0 = val0.max(val2);
        // At this point: val0 = [r, b, -l, -t] of the device-space bounding box of p0..2.

        // Does fCullBounds intersect the device-space bounding box of p0..2?
        // i.e., l0 < r1 && t0 < b1 && r0 > l1 && b0 > t1.
        all(self.f_cull_bounds.lt_mask(val0))
    }

    /// Returns whether any region of the bounding box of `M * p0..3` will be in the viewport.
    // Port of: src/gpu/tessellate/CullTest.h#L77-L97 (chrome/m156)
    #[must_use]
    pub fn are_visible4(&self, p: &[Point]) -> bool {
        // Transform p0..3 to device space.
        let mut val0 = self.f_mat_y * p[0].y;
        let mut val1 = self.f_mat_y * p[1].y;
        let mut val2 = self.f_mat_y * p[2].y;
        let mut val3 = self.f_mat_y * p[3].y;
        val0 = self.f_mat_x * p[0].x + val0;
        val1 = self.f_mat_x * p[1].x + val1;
        val2 = self.f_mat_x * p[2].x + val2;
        val3 = self.f_mat_x * p[3].x + val3;
        // At this point: valN = {xN, yN, -xN, -yN} in device space.

        // Find the device-space bounding box of p0..3.
        val0 = val0.max(val1);
        val2 = val2.max(val3);
        val0 = val0.max(val2);
        // At this point: val0 = [r, b, -l, -t] of the device-space bounding box of p0..3.

        // Does fCullBounds intersect the device-space bounding box of p0..3?
        // i.e., l0 < r1 && t0 < b1 && r0 > l1 && b0 > t1.
        all(self.f_cull_bounds.lt_mask(val0))
    }
}
