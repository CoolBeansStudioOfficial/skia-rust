// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPoint3.h, src/core/SkPoint3.cpp

//! 3D points and vectors (`SkPoint3.h`).

use crate::floating_point::{is_finite, is_finite_all};
use crate::scalar::{SCALAR_NEARLY_ZERO, scalar};
use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

/// Alternative name for [`Point3`].
#[doc(alias = "SkVector3")]
pub type Vector3 = Point3;

/// Alternative name for [`Point3`].
#[doc(alias = "SkColor3f")]
pub type Color3f = Point3;

/// A point (or vector) with three scalar coordinates.
// Port of: include/core/SkPoint3.h#L15-L144 (chrome/m156)
#[doc(alias = "SkPoint3")]
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Point3 {
    /// x-axis value.
    pub x: scalar,
    /// y-axis value.
    pub y: scalar,
    /// z-axis value.
    pub z: scalar,
}

impl From<(scalar, scalar, scalar)> for Point3 {
    fn from((x, y, z): (scalar, scalar, scalar)) -> Self {
        Self::new(x, y, z)
    }
}

impl Neg for Point3 {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y, -self.z)
    }
}

impl Add for Point3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl AddAssign for Point3 {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
    }
}

impl Sub for Point3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl SubAssign for Point3 {
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
    }
}

impl Mul<Point3> for scalar {
    type Output = Point3;
    fn mul(self, p: Point3) -> Self::Output {
        Point3::new(self * p.x, self * p.y, self * p.z)
    }
}

// Port of: src/core/SkPoint3.cpp#L13-L16 (chrome/m156)
/// Returns the square of the Euclidean distance to `(x, y, z)`.
fn get_length_squared(x: f32, y: f32, z: f32) -> f32 {
    x * x + y * y + z * z
}

// Port of: src/core/SkPoint3.cpp#L18-L27 (chrome/m156)
/// Calculates the square of the Euclidean distance to `(x, y, z)`; returns whether the distance
/// is judged to be "nearly zero", and the squared length.
fn is_length_nearly_zero(x: f32, y: f32, z: f32) -> (bool, f32) {
    let length_squared = get_length_squared(x, y, z);
    (
        length_squared <= (SCALAR_NEARLY_ZERO * SCALAR_NEARLY_ZERO),
        length_squared,
    )
}

impl Point3 {
    /// Sets `x`, `y`, and `z` (`SkPoint3::Make`).
    #[doc(alias = "SkPoint3::Make")]
    #[must_use]
    pub const fn new(x: scalar, y: scalar, z: scalar) -> Self {
        Self { x, y, z }
    }

    /// Sets `x`, `y`, and `z`.
    pub fn set(&mut self, x: scalar, y: scalar, z: scalar) {
        *self = Self::new(x, y, z);
    }

    /// Returns the Euclidean distance from `(0, 0, 0)` to `(x, y, z)`.
    // Port of: src/core/SkPoint3.cpp#L29-L40 (chrome/m156)
    #[doc(alias = "SkPoint3::Length")]
    #[allow(clippy::cast_possible_truncation)] // mirrors the (float) cast of Skia's double sqrt
    #[must_use]
    pub fn length_xyz(x: scalar, y: scalar, z: scalar) -> scalar {
        let mag_sq = get_length_squared(x, y, z);
        if is_finite(mag_sq) {
            mag_sq.sqrt()
        } else {
            let xx = f64::from(x);
            let yy = f64::from(y);
            let zz = f64::from(z);
            (xx * xx + yy * yy + zz * zz).sqrt() as f32
        }
    }

    /// Returns the Euclidean distance from `(0, 0, 0)` to the point.
    #[must_use]
    pub fn length(&self) -> scalar {
        Self::length_xyz(self.x, self.y, self.z)
    }

    /// Sets the point (vector) to be unit-length in the same direction as it already points. If
    /// the point has a degenerate length (nearly 0) then sets it to `(0, 0, 0)` and returns
    /// false; otherwise returns true.
    // Port of: src/core/SkPoint3.cpp#L49-L79 (chrome/m156)
    #[allow(clippy::cast_possible_truncation)] // mirrors `fX *= scale` (double math, float store)
    pub fn normalize(&mut self) -> bool {
        let (nearly_zero, mag_sq) = is_length_nearly_zero(self.x, self.y, self.z);
        if nearly_zero {
            self.set(0.0, 0.0, 0.0);
            return false;
        }

        // sqrtf does not provide enough precision; since sqrt takes a double,
        // there's no additional penalty to storing invScale in a double
        let inv_scale: f64 = if is_finite(mag_sq) {
            f64::from(mag_sq)
        } else {
            // our magSq step overflowed to infinity, so use doubles instead.
            let xx = f64::from(self.x);
            let yy = f64::from(self.y);
            let zz = f64::from(self.z);
            xx * xx + yy * yy + zz * zz
        };

        // using a float instead of a double for scale loses too much precision
        let scale: f64 = 1.0 / inv_scale.sqrt();
        self.x = (f64::from(self.x) * scale) as f32;
        self.y = (f64::from(self.y) * scale) as f32;
        self.z = (f64::from(self.z) * scale) as f32;
        if !is_finite_all(self.x, &[self.y, self.z]) {
            self.set(0.0, 0.0, 0.0);
            return false;
        }
        true
    }

    /// Returns a new point that is unit-length in the same direction as this one, or `None` if
    /// the point has a degenerate length.
    #[must_use]
    pub fn normalized(&self) -> Option<Self> {
        let mut normalized = *self;
        normalized.normalize().then_some(normalized)
    }

    /// Returns a new point whose coordinates are scaled by `scale` (`SkPoint3::makeScale`).
    #[doc(alias = "makeScale")]
    #[must_use]
    pub fn scaled(&self, scale: scalar) -> Self {
        Self::new(scale * self.x, scale * self.y, scale * self.z)
    }

    /// Scales the point's coordinates by `value`.
    pub fn scale(&mut self, value: scalar) {
        self.x *= value;
        self.y *= value;
        self.z *= value;
    }

    /// Returns true if `x`, `y`, and `z` are measurable values.
    #[doc(alias = "isFinite")]
    #[must_use]
    pub fn is_finite(&self) -> bool {
        is_finite_all(self.x, &[self.y, self.z])
    }

    /// Returns the dot product of `a` and `b`, treating them as 3D vectors.
    #[doc(alias = "DotProduct")]
    #[must_use]
    pub fn dot_product(a: Self, b: Self) -> scalar {
        a.x * b.x + a.y * b.y + a.z * b.z
    }

    /// Returns the dot product of the point and `vec`, treating them as 3D vectors.
    #[must_use]
    pub fn dot(&self, vec: Self) -> scalar {
        Self::dot_product(*self, vec)
    }

    /// Returns the cross product of `a` and `b`, treating them as 3D vectors.
    #[doc(alias = "CrossProduct")]
    #[must_use]
    pub fn cross_product(a: Self, b: Self) -> Self {
        Self {
            x: a.y * b.z - a.z * b.y,
            y: a.z * b.x - a.x * b.z,
            z: a.x * b.y - a.y * b.x,
        }
    }

    /// Returns the cross product of the point and `vec`, treating them as 3D vectors.
    #[must_use]
    pub fn cross(&self, vec: Self) -> Self {
        Self::cross_product(*self, vec)
    }
}
