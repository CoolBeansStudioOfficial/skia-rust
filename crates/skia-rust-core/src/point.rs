// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPoint.h, src/core/SkPoint.cpp

//! Integer and scalar points and vectors (`SkPoint.h`).

use crate::floating_point::{double_to_float, ieee_double_divide, is_finite, is_finite_all};
use crate::safe32::{sat_add, sat_sub};
use crate::scalar::scalar;
use crate::size::{ISize, Size};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// Alternative name for [`IPoint`]; the two can be used interchangeably.
#[doc(alias = "SkIVector")]
pub type IVector = IPoint;

/// A point with two 32-bit integer coordinates.
///
/// Like Skia's `SkIPoint`, adding and subtracting points and vectors saturates instead of
/// overflowing.
// Port of: include/core/SkPoint.h#L28-L151 (chrome/m156)
#[doc(alias = "SkIPoint")]
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct IPoint {
    /// x-axis value.
    pub x: i32,
    /// y-axis value.
    pub y: i32,
}

impl Neg for IPoint {
    type Output = Self;
    // Port of: include/core/SkPoint.h#L74-L76 (chrome/m156)
    fn neg(self) -> Self::Output {
        // C++ `-fX` overflows (UB) only for i32::MIN; wrap like the hardware negate.
        Self::new(self.x.wrapping_neg(), self.y.wrapping_neg())
    }
}

impl Add<IVector> for IPoint {
    type Output = Self;
    // Port of: include/core/SkPoint.h#L148-L150 (chrome/m156)
    fn add(self, rhs: IVector) -> Self {
        Self::new(sat_add(self.x, rhs.x), sat_add(self.y, rhs.y))
    }
}

impl AddAssign<IVector> for IPoint {
    // Port of: include/core/SkPoint.h#L82-L85 (chrome/m156)
    fn add_assign(&mut self, rhs: IVector) {
        self.x = sat_add(self.x, rhs.x);
        self.y = sat_add(self.y, rhs.y);
    }
}

impl Add<ISize> for IPoint {
    type Output = Self;
    fn add(self, rhs: ISize) -> Self {
        Self::new(sat_add(self.x, rhs.width), sat_add(self.y, rhs.height))
    }
}

impl AddAssign<ISize> for IPoint {
    fn add_assign(&mut self, rhs: ISize) {
        self.x = sat_add(self.x, rhs.width);
        self.y = sat_add(self.y, rhs.height);
    }
}

impl Sub for IPoint {
    type Output = Self;
    // Port of: include/core/SkPoint.h#L134-L136 (chrome/m156)
    fn sub(self, rhs: Self) -> Self {
        Self::new(sat_sub(self.x, rhs.x), sat_sub(self.y, rhs.y))
    }
}

impl SubAssign<IVector> for IPoint {
    // Port of: include/core/SkPoint.h#L91-L94 (chrome/m156)
    fn sub_assign(&mut self, rhs: IVector) {
        self.x = sat_sub(self.x, rhs.x);
        self.y = sat_sub(self.y, rhs.y);
    }
}

impl Sub<ISize> for IPoint {
    type Output = Self;
    fn sub(self, rhs: ISize) -> Self {
        Self::new(sat_sub(self.x, rhs.width), sat_sub(self.y, rhs.height))
    }
}

impl SubAssign<ISize> for IPoint {
    fn sub_assign(&mut self, rhs: ISize) {
        self.x = sat_sub(self.x, rhs.width);
        self.y = sat_sub(self.y, rhs.height);
    }
}

impl IPoint {
    /// Sets `x` and `y` (`SkIPoint::Make`).
    #[doc(alias = "SkIPoint::Make")]
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Returns true if `x` and `y` are both zero.
    // Port of: include/core/SkPoint.h#L58-L60 (chrome/m156)
    #[doc(alias = "isZero")]
    #[must_use]
    pub fn is_zero(self) -> bool {
        (self.x | self.y) == 0
    }

    /// Sets `x` and `y`.
    pub fn set(&mut self, x: i32, y: i32) {
        *self = Self::new(x, y);
    }

    /// Returns true if the point is equivalent to `(x, y)`.
    #[must_use]
    pub fn equals(self, x: i32, y: i32) -> bool {
        self.x == x && self.y == y
    }
}

impl From<(i32, i32)> for IPoint {
    fn from(source: (i32, i32)) -> Self {
        Self::new(source.0, source.1)
    }
}

/// Alternative name for [`Point`]; the two can be used interchangeably.
#[doc(alias = "SkVector")]
pub type Vector = Point;

/// A point with two 32-bit floating point coordinates.
// Port of: include/core/SkPoint.h#L163-L558 (chrome/m156)
#[doc(alias = "SkPoint")]
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Point {
    /// x-axis value.
    pub x: scalar,
    /// y-axis value.
    pub y: scalar,
}

impl Neg for Point {
    type Output = Self;
    // Port of: include/core/SkPoint.h#L366-L368 (chrome/m156)
    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y)
    }
}

impl Add<Vector> for Point {
    type Output = Self;
    // Port of: include/core/SkPoint.h#L469-L471 (chrome/m156)
    fn add(self, rhs: Vector) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign<Vector> for Point {
    // Port of: include/core/SkPoint.h#L374-L377 (chrome/m156)
    fn add_assign(&mut self, rhs: Vector) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Add<Size> for Point {
    type Output = Self;
    fn add(self, rhs: Size) -> Self {
        Self::new(self.x + rhs.width, self.y + rhs.height)
    }
}

impl AddAssign<Size> for Point {
    fn add_assign(&mut self, rhs: Size) {
        self.x += rhs.width;
        self.y += rhs.height;
    }
}

impl Sub for Point {
    type Output = Self;
    // Port of: include/core/SkPoint.h#L455-L457 (chrome/m156)
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl SubAssign<Vector> for Point {
    // Port of: include/core/SkPoint.h#L383-L386 (chrome/m156)
    fn sub_assign(&mut self, rhs: Vector) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl Sub<Size> for Point {
    type Output = Self;
    fn sub(self, rhs: Size) -> Self {
        Self::new(self.x - rhs.width, self.y - rhs.height)
    }
}

impl SubAssign<Size> for Point {
    fn sub_assign(&mut self, rhs: Size) {
        self.x -= rhs.width;
        self.y -= rhs.height;
    }
}

impl Mul<scalar> for Point {
    type Output = Self;
    // Port of: include/core/SkPoint.h#L394-L396 (chrome/m156)
    fn mul(self, rhs: scalar) -> Self {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl MulAssign<scalar> for Point {
    // Port of: include/core/SkPoint.h#L403-L407 (chrome/m156)
    fn mul_assign(&mut self, rhs: scalar) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

// `SkPoint.h` does not define a `/` operator; skia-safe adds it to complement `Mul`.
impl Div<scalar> for Point {
    type Output = Self;
    fn div(self, rhs: scalar) -> Self {
        Self::new(self.x / rhs, self.y / rhs)
    }
}

impl DivAssign<scalar> for Point {
    fn div_assign(&mut self, rhs: scalar) {
        self.x /= rhs;
        self.y /= rhs;
    }
}

// Port of: src/core/SkPoint.cpp#L33-L70 (chrome/m156)
/// Sets `pt` to `(x, y)` scaled to `length`, if possible; the shared worker of `normalize`,
/// `set_length` and friends.
///
/// We have to worry about 2 tricky conditions:
/// 1. underflow of mag2 (compared against nearlyzero^2)
/// 2. overflow of mag2 (compared w/ isfinite)
///
/// Skia (m156) always computes in doubles, which cannot overflow.
#[allow(clippy::cast_possible_truncation)] // mirrors `x *= dscale` (double math, float store)
fn set_point_length(
    pt: &mut Point,
    x: f32,
    y: f32,
    length: f32,
    orig_length: Option<&mut f32>,
) -> bool {
    // our mag2 step overflowed to infinity, so use doubles instead.
    // much slower, but needed when x or y are very large, other wise we
    // divide by inf. and return (0,0) vector.
    let xx = f64::from(x);
    let yy = f64::from(y);
    let dmag = (xx * xx + yy * yy).sqrt();
    let dscale = ieee_double_divide(f64::from(length), dmag);
    let x = (f64::from(x) * dscale) as f32;
    let y = (f64::from(y) * dscale) as f32;

    // check if we're not finite, or we're zero-length
    if !is_finite_all(x, &[y]) || (x == 0.0 && y == 0.0) {
        pt.set(0.0, 0.0);
        return false;
    }

    let mut mag = 0.0;
    if orig_length.is_some() {
        mag = double_to_float(dmag);
    }

    pt.set(x, y);
    if let Some(orig_length) = orig_length {
        *orig_length = mag;
    }
    true
}

impl Point {
    /// Sets `x` and `y`. Used both to set a point and a vector (`SkPoint::Make`).
    #[doc(alias = "SkPoint::Make")]
    #[must_use]
    pub const fn new(x: scalar, y: scalar) -> Self {
        Self { x, y }
    }

    /// Returns true if `x` and `y` are both zero.
    // Port of: include/core/SkPoint.h#L190-L192 (chrome/m156)
    #[doc(alias = "isZero")]
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.x == 0.0 && self.y == 0.0
    }

    /// Sets `x` and `y`.
    pub fn set(&mut self, x: scalar, y: scalar) {
        *self = Self::new(x, y);
    }

    /// Sets `x` and `y`, promoting integers to float values (`SkPoint::iset`).
    // Port of: include/core/SkPoint.h#L214-L232 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors static_cast<float>(int)
    pub fn iset(&mut self, p: impl Into<IPoint>) {
        let p = p.into();
        self.x = p.x as scalar;
        self.y = p.y as scalar;
    }

    /// Sets `x` to the absolute value of `p.x`, and `y` to the absolute value of `p.y`.
    // Port of: include/core/SkPoint.h#L236-L239 (chrome/m156)
    #[doc(alias = "setAbs")]
    pub fn set_abs(&mut self, p: impl Into<Point>) {
        let p = p.into();
        self.x = p.x.abs();
        self.y = p.y.abs();
    }

    /// Adds `offset` to each point in `points` (`SkPoint::Offset`).
    // Port of: include/core/SkPoint.h#L247-L263 (chrome/m156)
    #[doc(alias = "Offset")]
    pub fn offset_points(points: &mut [Point], offset: impl Into<Vector>) {
        let offset = offset.into();
        for p in points.iter_mut() {
            p.offset(offset);
        }
    }

    /// Adds `d` to the point (`SkPoint::offset`).
    // Port of: include/core/SkPoint.h#L269-L272 (chrome/m156)
    pub fn offset(&mut self, d: impl Into<Vector>) {
        let d = d.into();
        self.x += d.x;
        self.y += d.y;
    }

    /// Returns the Euclidean distance from origin, computed as `sqrt(x * x + y * y)`.
    // Port of: include/core/SkPoint.h#L282 (chrome/m156)
    #[must_use]
    pub fn length(self) -> scalar {
        Self::length_xy(self.x, self.y)
    }

    /// Returns the Euclidean distance from origin, computed as `sqrt(x * x + y * y)`.
    // Port of: include/core/SkPoint.h#L291 (chrome/m156)
    #[doc(alias = "distanceToOrigin")]
    #[must_use]
    pub fn distance_to_origin(self) -> scalar {
        self.length()
    }

    /// Scales so that [`Self::length()`] returns one, while preserving the ratio of `x` to `y`,
    /// if possible. If the prior length is nearly zero, sets the vector to `(0, 0)` and returns
    /// false; otherwise returns true.
    // Port of: src/core/SkPoint.cpp#L22-L24 (chrome/m156)
    pub fn normalize(&mut self) -> bool {
        self.set_length_xy(self.x, self.y, 1.0)
    }

    /// Sets the vector to `(x, y)` scaled so that [`Self::length()`] returns one, and so that
    /// the vector is proportional to `(x, y)`. If the `(x, y)` length is nearly zero, sets the
    /// vector to `(0, 0)` and returns false; otherwise returns true.
    // Port of: src/core/SkPoint.cpp#L26-L28 (chrome/m156)
    #[doc(alias = "setNormalize")]
    pub fn set_normalize(&mut self, x: scalar, y: scalar) -> bool {
        self.set_length_xy(x, y, 1.0)
    }

    /// Scales the vector so that [`Self::distance_to_origin()`] returns `length`, if possible.
    /// If the former length is nearly zero, sets the vector to `(0, 0)` and returns false;
    /// otherwise returns true.
    // Port of: src/core/SkPoint.cpp#L30-L32 (chrome/m156)
    #[doc(alias = "setLength")]
    pub fn set_length(&mut self, length: scalar) -> bool {
        self.set_length_xy(self.x, self.y, length)
    }

    /// Sets the vector to `(x, y)` scaled to `length`, if possible. If the former length is
    /// nearly zero, sets the vector to `(0, 0)` and returns false; otherwise returns true.
    // Port of: src/core/SkPoint.cpp#L90-L92 (chrome/m156)
    #[doc(alias = "setLength")]
    pub fn set_length_xy(&mut self, x: scalar, y: scalar, length: scalar) -> bool {
        set_point_length(self, x, y, length, None)
    }

    /// Returns the point times `scale` (`SkPoint::scale(float, SkPoint*)`).
    // Port of: src/core/SkPoint.cpp#L17-L20 (chrome/m156)
    #[doc(alias = "scale")]
    #[must_use]
    pub fn scaled(self, scale: scalar) -> Self {
        Self::new(self.x * scale, self.y * scale)
    }

    /// Scales the point in place by `scale`.
    // Port of: include/core/SkPoint.h#L350 (chrome/m156)
    pub fn scale(&mut self, scale: scalar) {
        *self = self.scaled(scale);
    }

    /// Changes the sign of `x` and `y`.
    // Port of: include/core/SkPoint.h#L357-L360 (chrome/m156)
    pub fn negate(&mut self) {
        self.x = -self.x;
        self.y = -self.y;
    }

    /// Returns true if both `x` and `y` are measurable values.
    // Port of: include/core/SkPoint.h#L412-L414 (chrome/m156)
    #[doc(alias = "isFinite")]
    #[must_use]
    pub fn is_finite(self) -> bool {
        is_finite_all(self.x, &[self.y])
    }

    /// Returns true if the point is equivalent to `(x, y)`.
    // Port of: include/core/SkPoint.h#L422-L424 (chrome/m156)
    #[allow(clippy::float_cmp)] // exact comparison, as in Skia
    #[must_use]
    pub fn equals(self, x: scalar, y: scalar) -> bool {
        self.x == x && self.y == y
    }

    /// Returns the Euclidean distance from origin, computed as `sqrt(x * x + y * y)`
    /// (`SkPoint::Length`).
    // Port of: src/core/SkPoint.cpp#L79-L88 (chrome/m156)
    #[doc(alias = "Length")]
    #[must_use]
    pub fn length_xy(x: scalar, y: scalar) -> scalar {
        let mag2 = x * x + y * y;
        if is_finite(mag2) {
            mag2.sqrt()
        } else {
            let xx = f64::from(x);
            let yy = f64::from(y);
            double_to_float((xx * xx + yy * yy).sqrt())
        }
    }

    /// Scales `v` so that [`Self::length()`] returns one, while preserving the ratio of `v.x` to
    /// `v.y`, if possible. If the original length is nearly zero, sets `v` to `(0, 0)` and
    /// returns zero; otherwise, returns the length of `v` before it is scaled
    /// (`SkPoint::Normalize`).
    ///
    /// The returned prior length may be infinity if it cannot be represented by a float.
    // Port of: src/core/SkPoint.cpp#L71-L77 (chrome/m156)
    #[doc(alias = "Normalize")]
    pub fn normalize_vector(v: &mut Vector) -> scalar {
        let mut mag = 0.0;
        let (x, y) = (v.x, v.y);
        if set_point_length(v, x, y, 1.0, Some(&mut mag)) {
            return mag;
        }
        0.0
    }

    /// Returns the Euclidean distance between `a` and `b`.
    // Port of: include/core/SkPoint.h#L508-L510 (chrome/m156)
    #[doc(alias = "Distance")]
    #[must_use]
    pub fn distance(a: Self, b: Self) -> scalar {
        Self::length_xy(a.x - b.x, a.y - b.y)
    }

    /// Returns the dot product of `a` and `b`.
    // Port of: include/core/SkPoint.h#L518-L520 (chrome/m156)
    #[doc(alias = "DotProduct")]
    #[must_use]
    pub fn dot_product(a: Self, b: Self) -> scalar {
        a.x * b.x + a.y * b.y
    }

    /// Returns the cross product of `a` and `b`: the z-axis component of the 3D cross product of
    /// the two vectors with z equal to zero.
    // Port of: include/core/SkPoint.h#L532-L534 (chrome/m156)
    #[doc(alias = "CrossProduct")]
    #[must_use]
    pub fn cross_product(a: Self, b: Self) -> scalar {
        a.x * b.y - a.y * b.x
    }

    /// Returns the cross product of the point and `vec`.
    // Port of: include/core/SkPoint.h#L545-L547 (chrome/m156)
    #[must_use]
    pub fn cross(self, vec: Vector) -> scalar {
        Self::cross_product(self, vec)
    }

    /// Returns the dot product of the point and `vec`.
    // Port of: include/core/SkPoint.h#L554-L556 (chrome/m156)
    #[must_use]
    pub fn dot(self, vec: Vector) -> scalar {
        Self::dot_product(self, vec)
    }
}

impl From<(scalar, scalar)> for Point {
    fn from(source: (scalar, scalar)) -> Self {
        Self::new(source.0, source.1)
    }
}

impl From<IPoint> for Point {
    #[allow(clippy::cast_precision_loss)] // mirrors static_cast<float>(int)
    fn from(source: IPoint) -> Self {
        Self::new(source.x as scalar, source.y as scalar)
    }
}

impl From<(i32, i32)> for Point {
    #[allow(clippy::cast_precision_loss)] // mirrors static_cast<float>(int)
    fn from(source: (i32, i32)) -> Self {
        Self::new(source.0 as scalar, source.1 as scalar)
    }
}

/// Private helpers of `SkPoint` (`SkPointPriv.h`), used by Skia's own code and tests.
#[doc(hidden)]
pub mod point_priv {
    use super::{IPoint, Point, Vector, set_point_length};
    use crate::floating_point::{float_sgn, ieee_float_divide, is_finite};
    use crate::scalar::{Scalar, scalar, scalar_sqrt};

    /// Which side of a line a point is on (`SkPointPriv::Side`).
    // Port of: src/core/SkPointPriv.h#L16-L20 (chrome/m156)
    #[doc(alias = "SkPointPriv::Side")]
    #[derive(Copy, Clone, PartialEq, Eq, Debug)]
    #[repr(i32)]
    pub enum Side {
        /// `kLeft_Side`.
        Left = -1,
        /// `kOn_Side`.
        On = 0,
        /// `kRight_Side`.
        Right = 1,
    }

    /// True if every point in `array` is finite (`SkPointPriv::AreFinite`).
    // Port of: src/core/SkPointPriv.h#L22-L24 (chrome/m156)
    #[doc(alias = "AreFinite")]
    #[must_use]
    #[allow(clippy::float_cmp, clippy::eq_op)] // prod == prod is the NaN test
    pub fn are_finite(array: &[Point]) -> bool {
        // SkIsFinite(&array[0].fX, count << 1): the scalars of the points, in order.
        // An empty slice is reported finite (C++ would read `array[0]` out of bounds).
        let Some(first) = array.first() else {
            return true;
        };
        let mut prod = first.x - first.x;
        prod *= first.y;
        for p in &array[1..] {
            prod *= p.x;
            prod *= p.y;
        }
        // At this point, `prod` will either be NaN or 0.
        prod == prod
    }

    /// Returns the point's scalars `[x, y]` (`SkPointPriv::AsScalars`).
    ///
    /// Skia returns a pointer to the point's storage; safe Rust returns a copy.
    // Port of: src/core/SkPointPriv.h#L26 (chrome/m156)
    #[doc(alias = "AsScalars")]
    #[must_use]
    pub fn as_scalars(pt: &Point) -> [scalar; 2] {
        [pt.x, pt.y]
    }

    /// True if `(dx, dy)` can be normalized (`SkPointPriv::CanNormalize`).
    // Port of: src/core/SkPointPriv.h#L28-L30 (chrome/m156)
    #[doc(alias = "CanNormalize")]
    #[must_use]
    pub fn can_normalize(dx: scalar, dy: scalar) -> bool {
        crate::floating_point::is_finite_all(dx, &[dy]) && (dx != 0.0 || dy != 0.0)
    }

    /// Squared distance from `pt` to the infinite line through `a` and `b`; optionally reports
    /// the side of the line `pt` is on (`SkPointPriv::DistanceToLineBetweenSqd`).
    // Port of: src/core/SkPoint.cpp#L101-L124 (chrome/m156)
    #[doc(alias = "DistanceToLineBetweenSqd")]
    #[must_use]
    pub fn distance_to_line_between_sqd(
        pt: Point,
        a: Point,
        b: Point,
        side: Option<&mut Side>,
    ) -> scalar {
        let u: Vector = b - a;
        let v: Vector = pt - a;

        let u_length_sqd = length_sqd(u);
        let det = u.cross(v);
        if let Some(side) = side {
            *side = match float_sgn(det) {
                -1 => Side::Left,
                0 => Side::On,
                _ => Side::Right,
            };
        }

        let mut temp = ieee_float_divide(det, u_length_sqd);
        temp *= det;
        // It's possible we have a degenerate line vector, or we're so far away it looks
        // degenerate. In this case, return squared distance to point A.
        if !is_finite(temp) {
            return length_sqd(v);
        }
        temp
    }

    /// Distance from `pt` to the infinite line through `a` and `b`
    /// (`SkPointPriv::DistanceToLineBetween`).
    // Port of: src/core/SkPointPriv.h#L37-L40 (chrome/m156)
    #[doc(alias = "DistanceToLineBetween")]
    #[must_use]
    pub fn distance_to_line_between(
        pt: Point,
        a: Point,
        b: Point,
        side: Option<&mut Side>,
    ) -> scalar {
        scalar_sqrt(distance_to_line_between_sqd(pt, a, b, side))
    }

    /// Squared distance from `pt` to the segment `a`-`b`
    /// (`SkPointPriv::DistanceToLineSegmentBetweenSqd`).
    // Port of: src/core/SkPoint.cpp#L126-L168 (chrome/m156)
    #[doc(alias = "DistanceToLineSegmentBetweenSqd")]
    #[must_use]
    pub fn distance_to_line_segment_between_sqd(pt: Point, a: Point, b: Point) -> scalar {
        // See comments to distanceToLineBetweenSqd. If the projection of c onto
        // u is between a and b then this returns the same result as that
        // function. Otherwise, it returns the distance to the closest of a and
        // b. Let the projection of v onto u be v'.  There are three cases:
        //    1. v' points opposite to u. c is not between a and b and is closer
        //       to a than b.
        //    2. v' points along u and has magnitude less than y. c is between
        //       a and b and the distance to the segment is the same as distance
        //       to the line ab.
        //    3. v' points along u and has greater magnitude than u. c is not
        //       between a and b and is closer to b than a.
        // v' = (u dot v) * u / |u|. So if (u dot v)/|u| is less than zero we're
        // in case 1. If (u dot v)/|u| is > |u| we are in case 3. Otherwise,
        // we're in case 2. We actually compare (u dot v) to 0 and |u|^2 to
        // avoid a sqrt to compute |u|.

        let u: Vector = b - a;
        let v: Vector = pt - a;

        let u_length_sqd = length_sqd(u);
        let u_dot_v = Point::dot_product(u, v);

        // closest point is point A
        if u_dot_v <= 0.0 {
            length_sqd(v)
        // closest point is point B
        } else if u_dot_v > u_length_sqd {
            distance_to_sqd(b, pt)
        // closest point is inside segment
        } else {
            let det = u.cross(v);
            let mut temp = ieee_float_divide(det, u_length_sqd);
            temp *= det;
            // It's possible we have a degenerate segment, or we're so far away it looks
            // degenerate. In this case, return squared distance to point A.
            if !is_finite(temp) {
                return length_sqd(v);
            }
            temp
        }
    }

    /// Distance from `pt` to the segment `a`-`b`
    /// (`SkPointPriv::DistanceToLineSegmentBetween`).
    // Port of: src/core/SkPointPriv.h#L47-L50 (chrome/m156)
    #[doc(alias = "DistanceToLineSegmentBetween")]
    #[must_use]
    pub fn distance_to_line_segment_between(pt: Point, a: Point, b: Point) -> scalar {
        scalar_sqrt(distance_to_line_segment_between_sqd(pt, a, b))
    }

    /// Squared distance between `pt` and `a` (`SkPointPriv::DistanceToSqd`).
    // Port of: src/core/SkPointPriv.h#L52-L56 (chrome/m156)
    #[doc(alias = "DistanceToSqd")]
    #[must_use]
    pub fn distance_to_sqd(pt: Point, a: Point) -> scalar {
        let dx = pt.x - a.x;
        let dy = pt.y - a.y;
        dx * dx + dy * dy
    }

    /// True if `p1` and `p2` are too close to tell apart
    /// (`SkPointPriv::EqualsWithinTolerance(p1, p2)`).
    // Port of: src/core/SkPointPriv.h#L58-L60 (chrome/m156)
    #[doc(alias = "EqualsWithinTolerance")]
    #[must_use]
    pub fn equals_within_tolerance(p1: Point, p2: Point) -> bool {
        !can_normalize(p1.x - p2.x, p1.y - p2.y)
    }

    /// True if `pt` and `p` are within `tol` of each other on both axes
    /// (`SkPointPriv::EqualsWithinTolerance(pt, p, tol)`).
    // Port of: src/core/SkPointPriv.h#L62-L65 (chrome/m156)
    #[doc(alias = "EqualsWithinTolerance")]
    #[must_use]
    pub fn equals_within_tolerance_tol(pt: Point, p: Point, tol: scalar) -> bool {
        (pt.x - p.x).nearly_zero(tol) && (pt.y - p.y).nearly_zero(tol)
    }

    /// Squared length of `pt` (`SkPointPriv::LengthSqd`).
    // Port of: src/core/SkPointPriv.h#L67-L69 (chrome/m156)
    #[doc(alias = "LengthSqd")]
    #[must_use]
    pub fn length_sqd(pt: Point) -> scalar {
        Point::dot_product(pt, pt)
    }

    /// Negates both coordinates of `pt` (`SkPointPriv::Negate`).
    // Port of: src/core/SkPointPriv.h#L71-L74 (chrome/m156)
    #[doc(alias = "Negate")]
    pub fn negate(pt: &mut IPoint) {
        *pt = -*pt;
    }

    /// Returns `src` rotated 90 degrees counter-clockwise (`SkPointPriv::RotateCCW`).
    // Port of: src/core/SkPointPriv.h#L76-L82 (chrome/m156)
    #[doc(alias = "RotateCCW")]
    #[must_use]
    pub fn rotate_ccw(src: Point) -> Point {
        Point::new(src.y, -src.x)
    }

    /// Rotates `pt` 90 degrees counter-clockwise in place (`SkPointPriv::RotateCCW(SkPoint*)`).
    // Port of: src/core/SkPointPriv.h#L83-L85 (chrome/m156)
    #[doc(alias = "RotateCCW")]
    pub fn rotate_ccw_in_place(pt: &mut Point) {
        *pt = rotate_ccw(*pt);
    }

    /// Returns `src` rotated 90 degrees clockwise (`SkPointPriv::RotateCW`).
    // Port of: src/core/SkPointPriv.h#L87-L93 (chrome/m156)
    #[doc(alias = "RotateCW")]
    #[must_use]
    pub fn rotate_cw(src: Point) -> Point {
        Point::new(-src.y, src.x)
    }

    /// Rotates `pt` 90 degrees clockwise in place (`SkPointPriv::RotateCW(SkPoint*)`).
    // Port of: src/core/SkPointPriv.h#L94-L96 (chrome/m156)
    #[doc(alias = "RotateCW")]
    pub fn rotate_cw_in_place(pt: &mut Point) {
        *pt = rotate_cw(*pt);
    }

    /// Like [`Point::set_length`], but Skia's "fast" variant (`SkPointPriv::SetLengthFast`). In
    /// m156 both share the same double-precision implementation.
    // Port of: src/core/SkPoint.cpp#L94-L96 (chrome/m156)
    #[doc(alias = "SetLengthFast")]
    pub fn set_length_fast(pt: &mut Point, length: scalar) -> bool {
        let (x, y) = (pt.x, pt.y);
        set_point_length(pt, x, y, length, None)
    }

    /// Returns a vector perpendicular to `vec` on the given `side` (`SkPointPriv::MakeOrthog`).
    ///
    /// `side` must be [`Side::Left`] or [`Side::Right`].
    // Port of: src/core/SkPointPriv.h#L98-L101 (chrome/m156)
    #[doc(alias = "MakeOrthog")]
    #[must_use]
    pub fn make_orthog(vec: Vector, side: Side) -> Point {
        debug_assert!(side == Side::Right || side == Side::Left);
        if side == Side::Right {
            Point::new(-vec.y, vec.x)
        } else {
            Point::new(vec.y, -vec.x)
        }
    }
}
