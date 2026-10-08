// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsLine.h, src/pathops/SkPathOpsLine.cpp

//! Double-precision lines (`SkDLine`).

use std::ops::{Index, IndexMut};

use skia_rust_core::point::Point;

use crate::point::{DPoint, DVector};
use crate::types::{
    almost_bequal_ulps, almost_between_ulps, almost_equal_ulps_pin, between, pin_t,
    roughly_equal_ulps, std_max, std_min,
};

/// `SkDLine`: a line segment with double-precision end points.
// Port of: src/pathops/SkPathOpsLine.h (chrome/m156)
#[doc(alias = "SkDLine")]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct DLine {
    pub pts: [DPoint; 2],
}

/// Port of `interpolated_largest`.
// Port of: src/pathops/SkPathOpsLine.cpp#L41-L48 (chrome/m156)
fn interpolated_largest(p0: DPoint, p1: DPoint, t: f64) -> f64 {
    const SEGMENT_EXTENT_FLOOR_SCALE: f64 = 0.001;
    let largest0 = std_max(p0.x.abs(), p0.y.abs());
    let largest1 = std_max(p1.x.abs(), p1.y.abs());
    let seg_extent = std_max((p1.x - p0.x).abs(), (p1.y - p0.y).abs());
    let min_floor = seg_extent * SEGMENT_EXTENT_FLOOR_SCALE;
    std_max((1.0 - t) * largest0 + t * largest1, min_floor)
}

impl DLine {
    #[must_use]
    pub const fn new(pts: [DPoint; 2]) -> Self {
        Self { pts }
    }

    /// `const SkDLine& set(const SkPoint pts[2])`.
    pub fn set(&mut self, pts: [Point; 2]) -> &mut Self {
        self.pts[0] = DPoint::from_sk_point(pts[0]);
        self.pts[1] = DPoint::from_sk_point(pts[1]);
        self
    }

    /// `SkDPoint ptAtT(double t) const`.
    // Port of: src/pathops/SkPathOpsLine.cpp#L14-L24 (chrome/m156)
    #[doc(alias = "ptAtT")]
    #[must_use]
    #[allow(clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn pt_at_t(&self, t: f64) -> DPoint {
        if t == 0.0 {
            return self.pts[0];
        }
        if t == 1.0 {
            return self.pts[1];
        }
        let one_t = 1.0 - t;
        DPoint::new(
            one_t * self.pts[0].x + t * self.pts[1].x,
            one_t * self.pts[0].y + t * self.pts[1].y,
        )
    }

    /// `double exactPoint(const SkDPoint& xy) const`: 0 or 1 if `xy` is an end point, else -1.
    // Port of: src/pathops/SkPathOpsLine.cpp#L26-L34 (chrome/m156)
    #[must_use]
    pub fn exact_point(&self, xy: DPoint) -> f64 {
        if xy == self.pts[0] {
            // do cheapest test first
            return 0.0;
        }
        if xy == self.pts[1] {
            return 1.0;
        }
        -1.0
    }

    /// `static double ExactPointH(const SkDPoint& xy, double left, double right, double y)`.
    // Port of: src/pathops/SkPathOpsLine.cpp#L102-L112 (chrome/m156)
    #[doc(alias = "ExactPointH")]
    #[must_use]
    #[allow(clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn exact_point_h(xy: DPoint, left: f64, right: f64, y: f64) -> f64 {
        if xy.y == y {
            if xy.x == left {
                return 0.0;
            }
            if xy.x == right {
                return 1.0;
            }
        }
        -1.0
    }

    /// `static double ExactPointV(const SkDPoint& xy, double top, double bottom, double x)`.
    // Port of: src/pathops/SkPathOpsLine.cpp#L135-L145 (chrome/m156)
    #[doc(alias = "ExactPointV")]
    #[must_use]
    #[allow(clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn exact_point_v(xy: DPoint, top: f64, bottom: f64, x: f64) -> f64 {
        if xy.x == x {
            if xy.y == top {
                return 0.0;
            }
            if xy.y == bottom {
                return 1.0;
            }
        }
        -1.0
    }

    /// `double nearPoint(const SkDPoint& xy, bool* unequal) const`: the t of the nearest point on
    /// the segment if it is within ULPS tolerance, else -1. `unequal` (if given) receives whether
    /// the float-rounded largest coordinate differs from the float-rounded distance sum.
    // Port of: src/pathops/SkPathOpsLine.cpp#L50-L84 (chrome/m156)
    #[doc(alias = "nearPoint")]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::float_cmp,
        clippy::if_not_else,
        clippy::must_use_candidate
    )] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn near_point(&self, xy: DPoint, unequal: Option<&mut bool>) -> f64 {
        if self.pts[0].x == self.pts[1].x && !almost_bequal_ulps(xy.x, self.pts[0].x) {
            return -1.0;
        }
        if self.pts[0].y == self.pts[1].y && !almost_bequal_ulps(xy.y, self.pts[0].y) {
            return -1.0;
        }
        let len = self.pts[1] - self.pts[0]; // the x/y magnitudes of the line
        let denom = len.x * len.x + len.y * len.y; // see DLine intersectRay
        let ab0 = xy - self.pts[0];
        let numer = len.x * ab0.x + ab0.y * len.y;
        if !between(0.0, numer, denom) {
            return -1.0;
        }
        let mut t = if denom != 0.0 { numer / denom } else { 0.0 };
        let real_pt = self.pt_at_t(t);
        let dist = real_pt.distance(xy); // OPTIMIZATION: can we compare against distSq instead ?
        let largest = interpolated_largest(self.pts[0], self.pts[1], t);
        if !almost_equal_ulps_pin(largest, largest + dist) {
            // is the dist within ULPS tolerance?
            return -1.0;
        }
        if let Some(unequal) = unequal {
            *unequal = (largest as f32) != ((largest + dist) as f32);
        }
        // a looser pin breaks skpwww_lptemp_com_3
        t = pin_t(t);
        debug_assert!(between(0.0, t, 1.0));
        t
    }

    /// `bool nearRay(const SkDPoint& xy) const`.
    // Port of: src/pathops/SkPathOpsLine.cpp#L86-L100 (chrome/m156)
    #[doc(alias = "nearRay")]
    #[must_use]
    pub fn near_ray(&self, xy: DPoint) -> bool {
        let len = self.pts[1] - self.pts[0]; // the x/y magnitudes of the line
        let denom = len.x * len.x + len.y * len.y; // see DLine intersectRay
        let ab0 = xy - self.pts[0];
        let numer = len.x * ab0.x + ab0.y * len.y;
        let t = numer / denom;
        let real_pt = self.pt_at_t(t);
        let dist = real_pt.distance(xy); // OPTIMIZATION: can we compare against distSq instead ?
        let tiniest = std_min(
            std_min(std_min(self.pts[0].x, self.pts[0].y), self.pts[1].x),
            self.pts[1].y,
        );
        let mut largest = std_max(
            std_max(std_max(self.pts[0].x, self.pts[0].y), self.pts[1].x),
            self.pts[1].y,
        );
        largest = std_max(largest, -tiniest);
        roughly_equal_ulps(largest, largest + dist) // is the dist within ULPS tolerance?
    }

    /// `static double NearPointH(const SkDPoint& xy, double left, double right, double y)`.
    // Port of: src/pathops/SkPathOpsLine.cpp#L114-L133 (chrome/m156)
    #[doc(alias = "NearPointH")]
    #[must_use]
    pub fn near_point_h(xy: DPoint, left: f64, right: f64, y: f64) -> f64 {
        if !almost_bequal_ulps(xy.y, y) {
            return -1.0;
        }
        if !almost_between_ulps(left, xy.x, right) {
            return -1.0;
        }
        let mut t = (xy.x - left) / (right - left);
        t = pin_t(t);
        debug_assert!(between(0.0, t, 1.0));
        let real_pt_x = (1.0 - t) * left + t * right;
        let dist_u = DVector::new(xy.y - y, xy.x - real_pt_x);
        let dist_sq = dist_u.x * dist_u.x + dist_u.y * dist_u.y;
        let dist = dist_sq.sqrt(); // OPTIMIZATION: can we compare against distSq instead ?
        let largest = interpolated_largest(DPoint::new(left, y), DPoint::new(right, y), t);
        if !almost_equal_ulps_pin(largest, largest + dist) {
            // is the dist within ULPS tolerance?
            return -1.0;
        }
        t
    }

    /// `static double NearPointV(const SkDPoint& xy, double top, double bottom, double x)`.
    // Port of: src/pathops/SkPathOpsLine.cpp#L147-L166 (chrome/m156)
    #[doc(alias = "NearPointV")]
    #[must_use]
    pub fn near_point_v(xy: DPoint, top: f64, bottom: f64, x: f64) -> f64 {
        if !almost_bequal_ulps(xy.x, x) {
            return -1.0;
        }
        if !almost_between_ulps(top, xy.y, bottom) {
            return -1.0;
        }
        let mut t = (xy.y - top) / (bottom - top);
        t = pin_t(t);
        debug_assert!(between(0.0, t, 1.0));
        let real_pt_y = (1.0 - t) * top + t * bottom;
        let dist_u = DVector::new(xy.x - x, xy.y - real_pt_y);
        let dist_sq = dist_u.x * dist_u.x + dist_u.y * dist_u.y;
        let dist = dist_sq.sqrt(); // OPTIMIZATION: can we compare against distSq instead ?
        let largest = interpolated_largest(DPoint::new(x, top), DPoint::new(x, bottom), t);
        if !almost_equal_ulps_pin(largest, largest + dist) {
            // is the dist within ULPS tolerance?
            return -1.0;
        }
        t
    }
}

impl Index<usize> for DLine {
    type Output = DPoint;
    fn index(&self, n: usize) -> &DPoint {
        &self.pts[n]
    }
}

impl IndexMut<usize> for DLine {
    fn index_mut(&mut self, n: usize) -> &mut DPoint {
        &mut self.pts[n]
    }
}
