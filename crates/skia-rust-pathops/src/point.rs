// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsPoint.h

//! Double-precision points and vectors used by `PathOps` (`SkDPoint`, `SkDVector`).

use std::ops::{AddAssign, DivAssign, MulAssign, SubAssign};

use skia_rust_core::floating_point::is_finite;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::double_to_scalar;

use crate::types::{
    almost_dequal_ulps, almost_equal_ulps, almost_equal_ulps_no_normal_check, almost_pequal_ulps,
    approximately_equal, approximately_zero, roughly_equal, roughly_equal_ulps,
    roughly_zero_when_compared_to, std_max, std_min,
};

/// `AlmostEqualUlps(const SkPoint& pt1, const SkPoint& pt2)`.
// Port of: src/pathops/SkPathOpsPoint.h (chrome/m156)
#[doc(alias = "AlmostEqualUlps")]
#[must_use]
pub fn almost_equal_ulps_point(pt1: Point, pt2: Point) -> bool {
    almost_equal_ulps(pt1.x, pt2.x) && almost_equal_ulps(pt1.y, pt2.y)
}

/// `SkDVector`: a double-precision vector.
// Port of: src/pathops/SkPathOpsPoint.h (chrome/m156)
#[doc(alias = "SkDVector")]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct DVector {
    pub x: f64,
    pub y: f64,
}

impl DVector {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// `SkDVector::set(const SkVector&)`.
    pub fn set(&mut self, pt: Point) -> &mut Self {
        self.x = f64::from(pt.x);
        self.y = f64::from(pt.y);
        self
    }

    /// `SkVector asSkVector() const`.
    #[doc(alias = "asSkVector")]
    #[must_use]
    pub fn as_sk_vector(self) -> Point {
        Point::new(double_to_scalar(self.x), double_to_scalar(self.y))
    }

    /// `double cross(const SkDVector& a) const`.
    #[must_use]
    pub fn cross(self, a: Self) -> f64 {
        self.x * a.y - self.y * a.x
    }

    /// `double crossCheck(const SkDVector& a) const`.
    #[must_use]
    pub fn cross_check(self, a: Self) -> f64 {
        let xy = self.x * a.y;
        let yx = self.y * a.x;
        if almost_equal_ulps(xy, yx) {
            0.0
        } else {
            xy - yx
        }
    }

    /// `double crossNoNormalCheck(const SkDVector& a) const`.
    #[must_use]
    pub fn cross_no_normal_check(self, a: Self) -> f64 {
        let xy = self.x * a.y;
        let yx = self.y * a.x;
        if almost_equal_ulps_no_normal_check(xy, yx) {
            0.0
        } else {
            xy - yx
        }
    }

    /// `double dot(const SkDVector& a) const`.
    #[must_use]
    pub fn dot(self, a: Self) -> f64 {
        self.x * a.x + self.y * a.y
    }

    /// `double length() const`.
    #[must_use]
    pub fn length(self) -> f64 {
        self.length_squared().sqrt()
    }

    /// `double lengthSquared() const`.
    #[must_use]
    pub fn length_squared(self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    /// `SkDVector& normalize()`.
    pub fn normalize(&mut self) -> &mut Self {
        let inverse_length = 1.0 / self.length();
        self.x *= inverse_length;
        self.y *= inverse_length;
        self
    }

    /// `bool isFinite() const`.
    #[must_use]
    pub fn is_finite(self) -> bool {
        is_finite(self.x) && is_finite(self.y)
    }
}

impl AddAssign for DVector {
    fn add_assign(&mut self, v: Self) {
        self.x += v.x;
        self.y += v.y;
    }
}

impl SubAssign for DVector {
    fn sub_assign(&mut self, v: Self) {
        self.x -= v.x;
        self.y -= v.y;
    }
}

impl DivAssign<f64> for DVector {
    fn div_assign(&mut self, s: f64) {
        self.x /= s;
        self.y /= s;
    }
}

impl MulAssign<f64> for DVector {
    fn mul_assign(&mut self, s: f64) {
        self.x *= s;
        self.y *= s;
    }
}

/// `SkDPoint`: a double-precision point.
// Port of: src/pathops/SkPathOpsPoint.h (chrome/m156)
#[doc(alias = "SkDPoint")]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct DPoint {
    pub x: f64,
    pub y: f64,
}

impl DPoint {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// `void set(const SkPoint& pt)`.
    pub fn set(&mut self, pt: Point) {
        self.x = f64::from(pt.x);
        self.y = f64::from(pt.y);
    }

    /// Builds a point from an `SkPoint` (`operator=(const SkPoint&)`).
    #[must_use]
    pub fn from_sk_point(pt: Point) -> Self {
        Self::new(f64::from(pt.x), f64::from(pt.y))
    }

    /// `bool approximatelyDEqual(const SkDPoint& a) const`.
    #[doc(alias = "approximatelyDEqual")]
    #[must_use]
    pub fn approximately_d_equal(self, a: Self) -> bool {
        if approximately_equal(self.x, a.x) && approximately_equal(self.y, a.y) {
            return true;
        }
        if !roughly_equal_ulps(self.x, a.x) || !roughly_equal_ulps(self.y, a.y) {
            return false;
        }
        let dist = self.distance(a);
        let tiniest = std_min(std_min(std_min(self.x, a.x), self.y), a.y);
        let mut largest = std_max(std_max(std_max(self.x, a.x), self.y), a.y);
        largest = std_max(largest, -tiniest);
        almost_dequal_ulps(largest, largest + dist) // is the dist within ULPS tolerance?
    }

    /// `bool approximatelyDEqual(const SkPoint& a) const`.
    #[must_use]
    pub fn approximately_d_equal_sk(self, a: Point) -> bool {
        self.approximately_d_equal(Self::from_sk_point(a))
    }

    /// `bool approximatelyEqual(const SkDPoint& a) const`.
    #[doc(alias = "approximatelyEqual")]
    #[must_use]
    pub fn approximately_equal(self, a: Self) -> bool {
        if approximately_equal(self.x, a.x) && approximately_equal(self.y, a.y) {
            return true;
        }
        if !roughly_equal_ulps(self.x, a.x) || !roughly_equal_ulps(self.y, a.y) {
            return false;
        }
        let dist = self.distance(a);
        let tiniest = std_min(std_min(std_min(self.x, a.x), self.y), a.y);
        let mut largest = std_max(std_max(std_max(self.x, a.x), self.y), a.y);
        largest = std_max(largest, -tiniest);
        almost_pequal_ulps(largest, largest + dist) // is the dist within ULPS tolerance?
    }

    /// `bool approximatelyEqual(const SkPoint& a) const`.
    #[must_use]
    pub fn approximately_equal_sk(self, a: Point) -> bool {
        self.approximately_equal(Self::from_sk_point(a))
    }

    /// `static bool ApproximatelyEqual(const SkPoint& a, const SkPoint& b)`.
    #[doc(alias = "ApproximatelyEqual")]
    #[must_use]
    pub fn approximately_equal_points(a: Point, b: Point) -> bool {
        if approximately_equal(f64::from(a.x), f64::from(b.x))
            && approximately_equal(f64::from(a.y), f64::from(b.y))
        {
            return true;
        }
        if !roughly_equal_ulps(a.x, b.x) || !roughly_equal_ulps(a.y, b.y) {
            return false;
        }
        let da = Self::from_sk_point(a);
        let db = Self::from_sk_point(b);
        let dist = da.distance(db);
        let tiniest = std_min(std_min(std_min(a.x, b.x), a.y), b.y);
        let mut largest = std_max(std_max(std_max(a.x, b.x), a.y), b.y);
        largest = std_max(largest, -tiniest);
        // is dist within ULPS tolerance?
        almost_dequal_ulps(f64::from(largest), f64::from(largest) + dist)
    }

    /// `bool approximatelyZero() const`.
    #[must_use]
    pub fn approximately_zero(self) -> bool {
        approximately_zero(self.x) && approximately_zero(self.y)
    }

    /// `SkPoint asSkPoint() const`.
    #[doc(alias = "asSkPoint")]
    #[must_use]
    pub fn as_sk_point(self) -> Point {
        Point::new(double_to_scalar(self.x), double_to_scalar(self.y))
    }

    /// `double distance(const SkDPoint& a) const`.
    #[must_use]
    pub fn distance(self, a: Self) -> f64 {
        let temp = self - a;
        temp.length()
    }

    /// `double distanceSquared(const SkDPoint& a) const`.
    #[must_use]
    pub fn distance_squared(self, a: Self) -> f64 {
        let temp = self - a;
        temp.length_squared()
    }

    /// `static SkDPoint Mid(const SkDPoint& a, const SkDPoint& b)`.
    #[must_use]
    #[allow(clippy::manual_midpoint)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn mid(a: Self, b: Self) -> Self {
        Self::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
    }

    /// `bool roughlyEqual(const SkDPoint& a) const`.
    #[must_use]
    pub fn roughly_equal(self, a: Self) -> bool {
        if roughly_equal(self.x, a.x) && roughly_equal(self.y, a.y) {
            return true;
        }
        let dist = self.distance(a);
        let tiniest = std_min(std_min(std_min(self.x, a.x), self.y), a.y);
        let mut largest = std_max(std_max(std_max(self.x, a.x), self.y), a.y);
        largest = std_max(largest, -tiniest);
        roughly_equal_ulps(largest, largest + dist) // is the dist within ULPS tolerance?
    }

    /// `static bool RoughlyEqual(const SkPoint& a, const SkPoint& b)`.
    #[must_use]
    pub fn roughly_equal_points(a: Point, b: Point) -> bool {
        if !roughly_equal_ulps(a.x, b.x) && !roughly_equal_ulps(a.y, b.y) {
            return false;
        }
        let da = Self::from_sk_point(a);
        let db = Self::from_sk_point(b);
        let dist = da.distance(db);
        let tiniest = std_min(std_min(std_min(a.x, b.x), a.y), b.y);
        let mut largest = std_max(std_max(std_max(a.x, b.x), a.y), b.y);
        largest = std_max(largest, -tiniest);
        // is dist within ULPS tolerance?
        roughly_equal_ulps(f64::from(largest), f64::from(largest) + dist)
    }

    /// `static bool WayRoughlyEqual(const SkPoint& a, const SkPoint& b)`.
    #[must_use]
    pub fn way_roughly_equal(a: Point, b: Point) -> bool {
        let largest_number = std_max(a.x.abs(), std_max(a.y.abs(), std_max(b.x.abs(), b.y.abs())));
        let diffs_x = a.x - b.x;
        let diffs_y = a.y - b.y;
        let largest_diff = std_max(diffs_x.abs(), diffs_y.abs());
        roughly_zero_when_compared_to(f64::from(largest_diff), f64::from(largest_number))
    }
}

impl std::ops::Sub for DPoint {
    type Output = DVector;
    // Port of: src/pathops/SkPathOpsPoint.h (chrome/m156)
    fn sub(self, b: Self) -> DVector {
        DVector::new(self.x - b.x, self.y - b.y)
    }
}

impl std::ops::Add<DVector> for DPoint {
    type Output = DPoint;
    // Port of: src/pathops/SkPathOpsPoint.h (chrome/m156)
    fn add(mut self, v: DVector) -> DPoint {
        self += v;
        self
    }
}

impl std::ops::Sub<DVector> for DPoint {
    type Output = DPoint;
    // Port of: src/pathops/SkPathOpsPoint.h (chrome/m156)
    fn sub(mut self, v: DVector) -> DPoint {
        self -= v;
        self
    }
}

impl AddAssign<DVector> for DPoint {
    fn add_assign(&mut self, v: DVector) {
        self.x += v.x;
        self.y += v.y;
    }
}

impl SubAssign<DVector> for DPoint {
    fn sub_assign(&mut self, v: DVector) {
        self.x -= v.x;
        self.y -= v.y;
    }
}

impl From<Point> for DPoint {
    fn from(pt: Point) -> Self {
        Self::from_sk_point(pt)
    }
}
