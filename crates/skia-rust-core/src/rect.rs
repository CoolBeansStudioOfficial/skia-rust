// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkRect.h, src/core/SkRect.cpp, src/core/SkRectPriv.h

//! Integer and scalar rectangles (`SkRect.h`).

use crate::floating_point::{
    float_ceil2int, float_floor2int, float_midpoint, float_round2int, is_finite_all,
};
use crate::point::{IPoint, IVector, Point, Vector};
use crate::safe32::{can_overflow_sub, pin_to_s32, sat_add, sat_sub};
use crate::scalar::{SCALAR_NAN, scalar};
use crate::size::{ISize, Size};

/// Overload-style containment test, as in `skia-safe`'s `Contains`.
///
/// Implemented for the `contains(...)` overloads of [`IRect`] and [`Rect`].
pub trait Contains<T> {
    /// Returns true if `other` is contained (see the implementing type for the exact rule).
    fn contains(&self, other: T) -> bool;
}

// `std::min(a, b)` is `(b < a) ? b : a`; `std::max(a, b)` is `(a < b) ? b : a`. These differ from
// `f32::min`/`f32::max` when an operand is NaN.
fn std_min(a: scalar, b: scalar) -> scalar {
    if b < a { b } else { a }
}

fn std_max(a: scalar, b: scalar) -> scalar {
    if a < b { b } else { a }
}

/// A rectangle with 32-bit integer edges.
// Port of: include/core/SkRect.h#L37-L592 (chrome/m156)
#[doc(alias = "SkIRect")]
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct IRect {
    /// The x coordinate of the rectangle's left edge.
    pub left: i32,
    /// The y coordinate of the rectangle's top edge.
    pub top: i32,
    /// The x coordinate of the rectangle's right edge.
    pub right: i32,
    /// The y coordinate of the rectangle's bottom edge.
    pub bottom: i32,
}

impl AsRef<IRect> for IRect {
    fn as_ref(&self) -> &IRect {
        self
    }
}

impl IRect {
    /// Constructs from edges (`SkIRect` aggregate initialisation).
    #[must_use]
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// Returns the empty rectangle `(0, 0, 0, 0)`.
    #[doc(alias = "MakeEmpty")]
    #[must_use]
    pub const fn new_empty() -> Self {
        Self::new(0, 0, 0, 0)
    }

    /// Returns `(0, 0, w, h)`.
    #[doc(alias = "MakeWH")]
    #[must_use]
    pub const fn from_wh(w: i32, h: i32) -> Self {
        Self::new(0, 0, w, h)
    }

    /// Returns `(0, 0, size.width, size.height)`.
    #[doc(alias = "MakeSize")]
    #[must_use]
    pub fn from_size(size: impl Into<ISize>) -> Self {
        let size = size.into();
        Self::new(0, 0, size.width, size.height)
    }

    /// Returns the rectangle at `pt` with `size`.
    #[doc(alias = "MakePtSize")]
    #[must_use]
    pub fn from_pt_size(pt: impl Into<IPoint>, size: impl Into<ISize>) -> Self {
        let pt = pt.into();
        let size = size.into();
        Self::from_xywh(pt.x, pt.y, size.width, size.height)
    }

    /// Returns `(l, t, r, b)`.
    #[doc(alias = "MakeLTRB")]
    #[must_use]
    pub const fn from_ltrb(l: i32, t: i32, r: i32, b: i32) -> Self {
        Self::new(l, t, r, b)
    }

    /// Returns `(x, y, x + w, y + h)`, saturating the sums.
    // Port of: include/core/SkRect.h#L109-L111 (chrome/m156)
    #[doc(alias = "MakeXYWH")]
    #[must_use]
    pub const fn from_xywh(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            left: x,
            top: y,
            right: sat_add(x, w),
            bottom: sat_add(y, h),
        }
    }

    /// Returns the left edge.
    #[must_use]
    pub const fn left(&self) -> i32 {
        self.left
    }

    /// Returns the top edge.
    #[must_use]
    pub const fn top(&self) -> i32 {
        self.top
    }

    /// Returns the right edge.
    #[must_use]
    pub const fn right(&self) -> i32 {
        self.right
    }

    /// Returns the bottom edge.
    #[must_use]
    pub const fn bottom(&self) -> i32 {
        self.bottom
    }

    /// Returns the left edge.
    #[must_use]
    pub const fn x(&self) -> i32 {
        self.left
    }

    /// Returns the top edge.
    #[must_use]
    pub const fn y(&self) -> i32 {
        self.top
    }

    /// Returns the top-left corner.
    #[doc(alias = "topLeft")]
    #[must_use]
    pub const fn top_left(&self) -> IPoint {
        IPoint::new(self.left, self.top)
    }

    /// Returns `right - left`, wrapping on overflow.
    // Port of: include/core/SkRect.h#L163 (chrome/m156)
    #[must_use]
    pub const fn width(&self) -> i32 {
        can_overflow_sub(self.right, self.left)
    }

    /// Returns `bottom - top`, wrapping on overflow.
    // Port of: include/core/SkRect.h#L164 (chrome/m156)
    #[must_use]
    pub const fn height(&self) -> i32 {
        can_overflow_sub(self.bottom, self.top)
    }

    /// Returns the size of the rectangle.
    #[must_use]
    pub const fn size(&self) -> ISize {
        ISize::new(self.width(), self.height())
    }

    /// Returns `right - left` as an `i64`.
    #[doc(alias = "width64")]
    #[must_use]
    pub const fn width_64(&self) -> i64 {
        self.right as i64 - self.left as i64
    }

    /// Returns `bottom - top` as an `i64`.
    #[doc(alias = "height64")]
    #[must_use]
    pub const fn height_64(&self) -> i64 {
        self.bottom as i64 - self.top as i64
    }

    /// Returns true if the rectangle is empty, computed in 64 bits.
    #[doc(alias = "isEmpty64")]
    #[must_use]
    pub const fn is_empty_64(&self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }

    /// Returns true if the width or height is zero or negative, or overflows an `i32`.
    // Port of: include/core/SkRect.h#L207-L214 (chrome/m156)
    #[doc(alias = "isEmpty")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let w = self.width_64();
        let h = self.height_64();
        if w <= 0 || h <= 0 {
            return true;
        }
        // Return true if either exceeds int32_t
        i32::try_from(w | h).is_err()
    }

    /// Sets all edges to zero.
    #[doc(alias = "setEmpty")]
    pub fn set_empty(&mut self) {
        *self = Self::new_empty();
    }

    /// Sets the edges.
    #[doc(alias = "setLTRB")]
    pub fn set_ltrb(&mut self, left: i32, top: i32, right: i32, bottom: i32) {
        *self = Self::new(left, top, right, bottom);
    }

    /// Sets to `(x, y, x + w, y + h)`, saturating the sums.
    #[doc(alias = "setXYWH")]
    pub fn set_xywh(&mut self, x: i32, y: i32, w: i32, h: i32) {
        *self = Self::from_xywh(x, y, w, h);
    }

    /// Sets to `(0, 0, width, height)`.
    #[doc(alias = "setWH")]
    pub fn set_wh(&mut self, width: i32, height: i32) {
        self.left = 0;
        self.top = 0;
        self.right = width;
        self.bottom = height;
    }

    /// Sets to `(0, 0, size.width, size.height)`.
    #[doc(alias = "setSize")]
    pub fn set_size(&mut self, size: impl Into<ISize>) {
        let size = size.into();
        self.left = 0;
        self.top = 0;
        self.right = size.width;
        self.bottom = size.height;
    }

    /// Returns the rectangle moved by `delta`.
    // Port of: include/core/SkRect.h#L305-L310 (chrome/m156)
    #[doc(alias = "makeOffset")]
    #[must_use]
    pub fn with_offset(&self, delta: impl Into<IVector>) -> Self {
        let delta = delta.into();
        let (dx, dy) = (delta.x, delta.y);
        Self::new(
            sat_add(self.left, dx),
            sat_add(self.top, dy),
            sat_add(self.right, dx),
            sat_add(self.bottom, dy),
        )
    }

    /// Returns the rectangle with edges moved inwards by `delta`.
    // Port of: include/core/SkRect.h#L337-L342 (chrome/m156)
    #[doc(alias = "makeInset")]
    #[must_use]
    pub fn with_inset(&self, delta: impl Into<IVector>) -> Self {
        let delta = delta.into();
        let (dx, dy) = (delta.x, delta.y);
        Self::new(
            sat_add(self.left, dx),
            sat_add(self.top, dy),
            sat_sub(self.right, dx),
            sat_sub(self.bottom, dy),
        )
    }

    /// Returns the rectangle with edges moved outwards by `delta`.
    // Port of: include/core/SkRect.h#L355-L360 (chrome/m156)
    #[doc(alias = "makeOutset")]
    #[must_use]
    pub fn with_outset(&self, delta: impl Into<IVector>) -> Self {
        let delta = delta.into();
        let (dx, dy) = (delta.x, delta.y);
        Self::new(
            sat_sub(self.left, dx),
            sat_sub(self.top, dy),
            sat_add(self.right, dx),
            sat_add(self.bottom, dy),
        )
    }

    /// Offsets the rectangle by `delta`.
    // Port of: include/core/SkRect.h#L372-L394 (chrome/m156)
    pub fn offset(&mut self, delta: impl Into<IPoint>) {
        *self = self.with_offset(delta.into());
    }

    /// Offsets so that the top-left is at `new_p`, keeping the size (saturating).
    // Port of: include/core/SkRect.h#L399-L404 (chrome/m156)
    #[doc(alias = "offsetTo")]
    pub fn offset_to(&mut self, new_p: impl Into<IPoint>) {
        *self = self.with_offset_to(new_p);
    }

    /// Returns the rectangle offset so that its top-left is at `new_p`.
    #[doc(alias = "offsetTo")]
    #[must_use]
    pub fn with_offset_to(&self, new_p: impl Into<IPoint>) -> Self {
        let new_p = new_p.into();
        let (new_x, new_y) = (new_p.x, new_p.y);
        Self::new(
            new_x,
            new_y,
            pin_to_s32(i64::from(self.right) + i64::from(new_x) - i64::from(self.left)),
            pin_to_s32(i64::from(self.bottom) + i64::from(new_y) - i64::from(self.top)),
        )
    }

    /// Insets the rectangle by `delta`.
    // Port of: include/core/SkRect.h#L416-L421 (chrome/m156)
    pub fn inset(&mut self, delta: impl Into<IVector>) {
        *self = self.with_inset(delta);
    }

    /// Outsets the rectangle by `delta`.
    // Port of: include/core/SkRect.h#L433 (chrome/m156)
    pub fn outset(&mut self, delta: impl Into<IVector>) {
        let delta = delta.into();
        // outset(dx, dy) is inset(-dx, -dy)
        self.inset(IVector::new(delta.x.wrapping_neg(), delta.y.wrapping_neg()));
    }

    /// Returns the rectangle with each edge moved by the matching delta (saturating).
    #[doc(alias = "adjust")]
    #[must_use]
    pub fn with_adjustment(&self, d_l: i32, d_t: i32, d_r: i32, d_b: i32) -> Self {
        Self::new(
            sat_add(self.left, d_l),
            sat_add(self.top, d_t),
            sat_add(self.right, d_r),
            sat_add(self.bottom, d_b),
        )
    }

    /// Moves each edge by the matching delta (saturating).
    // Port of: include/core/SkRect.h#L440-L446 (chrome/m156)
    pub fn adjust(&mut self, d_l: i32, d_t: i32, d_r: i32, d_b: i32) {
        *self = self.with_adjustment(d_l, d_t, d_r, d_b);
    }

    /// Like `contains(r)` but without checking for empties; both must be non-empty.
    // Port of: include/core/SkRect.h#L504-L509 (chrome/m156)
    #[doc(alias = "containsNoEmptyCheck")]
    #[must_use]
    pub fn contains_no_empty_check(&self, r: &Self) -> bool {
        debug_assert!(self.left < self.right && self.top < self.bottom);
        debug_assert!(r.left < r.right && r.top < r.bottom);
        self.left <= r.left && self.top <= r.top && self.right >= r.right && self.bottom >= r.bottom
    }

    /// Returns the intersection of `a` and `b`, or `None` if it is empty.
    // Port of: src/core/SkRect.cpp#L17-L29 (chrome/m156)
    #[must_use]
    pub fn intersect(a: &Self, b: &Self) -> Option<Self> {
        let tmp = Self::new(
            a.left.max(b.left),
            a.top.max(b.top),
            a.right.min(b.right),
            a.bottom.min(b.bottom),
        );
        if tmp.is_empty() {
            return None;
        }
        Some(tmp)
    }

    /// Returns true if `a` and `b` intersect.
    // Port of: include/core/SkRect.h#L540-L542 (chrome/m156)
    #[doc(alias = "Intersects")]
    #[must_use]
    pub fn intersects(a: &Self, b: &Self) -> bool {
        Self::intersect(a, b).is_some()
    }

    /// Returns the union of `a` and `b`.
    ///
    /// Has no effect if `b` is empty. Otherwise, if `a` is empty, returns `b`.
    // Port of: src/core/SkRect.cpp#L31-L48 (chrome/m156)
    #[must_use]
    pub fn join(a: &Self, b: &Self) -> Self {
        let mut this = *a;
        let r = b;
        // do nothing if the params are empty
        if r.left >= r.right || r.top >= r.bottom {
            return this;
        }

        // if we are empty, just assign
        if this.left >= this.right || this.top >= this.bottom {
            this = *r;
        } else {
            if r.left < this.left {
                this.left = r.left;
            }
            if r.top < this.top {
                this.top = r.top;
            }
            if r.right > this.right {
                this.right = r.right;
            }
            if r.bottom > this.bottom {
                this.bottom = r.bottom;
            }
        }
        this
    }

    /// Swaps edges so that `left <= right` and `top <= bottom`.
    // Port of: include/core/SkRect.h#L558-L567 (chrome/m156)
    pub fn sort(&mut self) {
        if self.left > self.right {
            std::mem::swap(&mut self.left, &mut self.right);
        }
        if self.top > self.bottom {
            std::mem::swap(&mut self.top, &mut self.bottom);
        }
    }

    /// Returns the rectangle with edges sorted.
    // Port of: include/core/SkRect.h#L574-L577 (chrome/m156)
    #[doc(alias = "makeSorted")]
    #[must_use]
    pub fn sorted(&self) -> Self {
        Self::new(
            self.left.min(self.right),
            self.top.min(self.bottom),
            self.left.max(self.right),
            self.top.max(self.bottom),
        )
    }

    /// Returns the edges as `[left, top, right, bottom]` (`SkIRect::asInt32s`).
    ///
    /// Skia returns a pointer to the storage; safe Rust returns a copy.
    #[doc(alias = "asInt32s")]
    #[must_use]
    pub const fn as_i32s(&self) -> [i32; 4] {
        [self.left, self.top, self.right, self.bottom]
    }
}

impl Contains<IPoint> for IRect {
    /// True if `x >= left && x < right && y >= top && y < bottom`.
    // Port of: include/core/SkRect.h#L468-L470 (chrome/m156)
    fn contains(&self, other: IPoint) -> bool {
        let (x, y) = (other.x, other.y);
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

impl Contains<&IRect> for IRect {
    /// True if `r` is non-empty, this is non-empty and `r` lies within this.
    // Port of: include/core/SkRect.h#L480-L485 (chrome/m156)
    fn contains(&self, r: &IRect) -> bool {
        !r.is_empty()
            && !self.is_empty()
            && self.left <= r.left
            && self.top <= r.top
            && self.right >= r.right
            && self.bottom >= r.bottom
    }
}

impl Contains<IRect> for IRect {
    fn contains(&self, other: IRect) -> bool {
        self.contains(&other)
    }
}

impl Contains<&Rect> for IRect {
    /// True if `r` is non-empty, this is non-empty and `r` lies within this.
    // Port of: include/core/SkRect.h#L1411-L1417 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors the implicit int -> float conversion
    fn contains(&self, r: &Rect) -> bool {
        !r.is_empty()
            && !self.is_empty()
            && (self.left as scalar) <= r.left
            && (self.top as scalar) <= r.top
            && (self.right as scalar) >= r.right
            && (self.bottom as scalar) >= r.bottom
    }
}

impl Contains<Rect> for IRect {
    fn contains(&self, other: Rect) -> bool {
        self.contains(&other)
    }
}

impl From<(IPoint, ISize)> for IRect {
    fn from((point, size): (IPoint, ISize)) -> Self {
        Self::from_pt_size(point, size)
    }
}

/// A rectangle with scalar edges.
// Port of: include/core/SkRect.h#L594-L1404 (chrome/m156)
#[doc(alias = "SkRect")]
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Rect {
    /// The x coordinate of the rectangle's left edge.
    pub left: scalar,
    /// The y coordinate of the rectangle's top edge.
    pub top: scalar,
    /// The x coordinate of the rectangle's right edge.
    pub right: scalar,
    /// The y coordinate of the rectangle's bottom edge.
    pub bottom: scalar,
}

impl AsRef<Rect> for Rect {
    fn as_ref(&self) -> &Rect {
        self
    }
}

impl Rect {
    /// Constructs from edges (`SkRect` aggregate initialisation).
    #[must_use]
    pub const fn new(left: scalar, top: scalar, right: scalar, bottom: scalar) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// Returns the empty rectangle `(0, 0, 0, 0)`.
    #[doc(alias = "MakeEmpty")]
    #[must_use]
    pub const fn new_empty() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }

    /// Returns `(0, 0, w, h)`.
    #[doc(alias = "MakeWH")]
    #[must_use]
    pub const fn from_wh(w: scalar, h: scalar) -> Self {
        Self::new(0.0, 0.0, w, h)
    }

    /// Returns `(0, 0, w, h)` from integers.
    // Port of: include/core/SkRect.h#L633-L635 (chrome/m156)
    #[doc(alias = "MakeIWH")]
    #[allow(clippy::cast_precision_loss)] // mirrors static_cast<float>(int)
    #[must_use]
    pub fn from_iwh(w: i32, h: i32) -> Self {
        Self::new(0.0, 0.0, w as scalar, h as scalar)
    }

    /// Returns `(0, 0, size.width, size.height)`.
    #[doc(alias = "MakeSize")]
    #[must_use]
    pub fn from_size(size: impl Into<Size>) -> Self {
        let size = size.into();
        Self::new(0.0, 0.0, size.width, size.height)
    }

    /// Returns `(l, t, r, b)`.
    #[doc(alias = "MakeLTRB")]
    #[must_use]
    pub const fn from_ltrb(l: scalar, t: scalar, r: scalar, b: scalar) -> Self {
        Self::new(l, t, r, b)
    }

    /// Returns `(x, y, x + w, y + h)`.
    // Port of: include/core/SkRect.h#L671-L673 (chrome/m156)
    #[doc(alias = "MakeXYWH")]
    #[must_use]
    pub const fn from_xywh(x: scalar, y: scalar, w: scalar, h: scalar) -> Self {
        Self::new(x, y, x + w, y + h)
    }

    /// Returns the rectangle at `p` with size `sz`.
    #[doc(alias = "MakeXYWH")]
    #[must_use]
    pub fn from_point_and_size(p: impl Into<Point>, sz: impl Into<Size>) -> Self {
        let (p, sz) = (p.into(), sz.into());
        Self::from_xywh(p.x, p.y, sz.width, sz.height)
    }

    /// Returns `(0, 0, size.width, size.height)` from an integer size (`SkRect::Make(SkISize)`).
    #[doc(alias = "Make")]
    #[must_use]
    pub fn from_isize(isize: impl Into<ISize>) -> Self {
        let isize = isize.into();
        Self::from_iwh(isize.width, isize.height)
    }

    /// Returns the integer rectangle as a scalar rectangle (`SkRect::Make(SkIRect)`).
    // Port of: include/core/SkRect.h#L691-L696 (chrome/m156)
    #[doc(alias = "Make")]
    #[allow(clippy::cast_precision_loss)] // mirrors static_cast<float>(int)
    #[must_use]
    pub fn from_irect(irect: impl AsRef<IRect>) -> Self {
        let irect = irect.as_ref();
        Self::new(
            irect.left as scalar,
            irect.top as scalar,
            irect.right as scalar,
            irect.bottom as scalar,
        )
    }

    /// Returns true if `left >= right` or `top >= bottom`, or any edge is NaN.
    // Port of: include/core/SkRect.h#L705-L709 (chrome/m156)
    #[doc(alias = "isEmpty")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        // We write it as the NOT of a non-empty rect, so we will return true if any values
        // are NaN.
        !(self.left < self.right && self.top < self.bottom)
    }

    /// Returns true if `left <= right` and `top <= bottom`.
    #[doc(alias = "isSorted")]
    #[must_use]
    pub fn is_sorted(&self) -> bool {
        self.left <= self.right && self.top <= self.bottom
    }

    /// Returns true if all edges are finite.
    // Port of: include/core/SkRect.h#L723-L725 (chrome/m156)
    #[doc(alias = "isFinite")]
    #[must_use]
    pub fn is_finite(&self) -> bool {
        is_finite_all(self.left, &[self.top, self.right, self.bottom])
    }

    /// Returns the left edge.
    #[must_use]
    pub const fn x(&self) -> scalar {
        self.left
    }

    /// Returns the top edge.
    #[must_use]
    pub const fn y(&self) -> scalar {
        self.top
    }

    /// Returns the left edge.
    #[must_use]
    pub const fn left(&self) -> scalar {
        self.left
    }

    /// Returns the top edge.
    #[must_use]
    pub const fn top(&self) -> scalar {
        self.top
    }

    /// Returns the right edge.
    #[must_use]
    pub const fn right(&self) -> scalar {
        self.right
    }

    /// Returns the bottom edge.
    #[must_use]
    pub const fn bottom(&self) -> scalar {
        self.bottom
    }

    /// Returns `(width, height)`.
    #[must_use]
    pub fn size(&self) -> Size {
        Size::new(self.width(), self.height())
    }

    /// Returns `right - left`; may overflow to infinity.
    // Port of: include/core/SkRect.h#L765 (chrome/m156)
    #[must_use]
    pub const fn width(&self) -> scalar {
        self.right - self.left
    }

    /// Returns `bottom - top`; may overflow to infinity.
    // Port of: include/core/SkRect.h#L772 (chrome/m156)
    #[must_use]
    pub const fn height(&self) -> scalar {
        self.bottom - self.top
    }

    /// Returns the average of `left` and `right`, computed without overflowing.
    // Port of: include/core/SkRect.h#L788-L790 (chrome/m156)
    #[doc(alias = "centerX")]
    #[must_use]
    pub fn center_x(&self) -> scalar {
        float_midpoint(self.left, self.right)
    }

    /// Returns the average of `top` and `bottom`, computed without overflowing.
    // Port of: include/core/SkRect.h#L797-L799 (chrome/m156)
    #[doc(alias = "centerY")]
    #[must_use]
    pub fn center_y(&self) -> scalar {
        float_midpoint(self.top, self.bottom)
    }

    /// Returns the center point.
    #[must_use]
    pub fn center(&self) -> Point {
        Point::new(self.center_x(), self.center_y())
    }

    /// Returns the top-left corner.
    #[doc(alias = "TL")]
    #[must_use]
    pub const fn tl(&self) -> Point {
        Point::new(self.left, self.top)
    }

    /// Returns the top-right corner.
    #[doc(alias = "TR")]
    #[must_use]
    pub const fn tr(&self) -> Point {
        Point::new(self.right, self.top)
    }

    /// Returns the bottom-left corner.
    #[doc(alias = "BL")]
    #[must_use]
    pub const fn bl(&self) -> Point {
        Point::new(self.left, self.bottom)
    }

    /// Returns the bottom-right corner.
    #[doc(alias = "BR")]
    #[must_use]
    pub const fn br(&self) -> Point {
        Point::new(self.right, self.bottom)
    }

    /// Sets all edges to zero.
    #[doc(alias = "setEmpty")]
    pub fn set_empty(&mut self) {
        *self = Self::new_empty();
    }

    /// Sets to the integer rectangle `irect` (`SkRect::set(const SkIRect&)`).
    #[doc(alias = "set")]
    pub fn set_irect(&mut self, irect: impl AsRef<IRect>) {
        *self = Self::from_irect(irect);
    }

    /// Sets the edges.
    #[doc(alias = "setLTRB")]
    pub fn set_ltrb(&mut self, left: scalar, top: scalar, right: scalar, bottom: scalar) {
        *self = Self::new(left, top, right, bottom);
    }

    /// Returns the bounds of `points`, or `None` if any point is not finite. An empty slice
    /// yields the empty rectangle.
    // Port of: src/core/SkRect.cpp#L50-L119 (chrome/m156)
    #[doc(alias = "Bounds")]
    #[must_use]
    pub fn bounds(pts: &[Point]) -> Option<Rect> {
        let first = pts.first()?;
        // Skia has a 64-bit and a 32-bit (skvx) variant that "compute the same numerics"; this
        // is the 64-bit one.
        let (mut l, mut t, mut r, mut b) = (first.x, first.y, first.x, first.y);
        let mut nx: scalar = 0.0;
        let mut ny: scalar = 0.0;
        for p in pts {
            // fminf / fmaxf
            l = p.x.min(l);
            t = p.y.min(t);
            r = p.x.max(r);
            b = p.y.max(b);
            // we do this to look for infinities or nans
            nx *= p.x;
            ny *= p.y;
        }
        // if this is true, all our values were finite
        if nx == 0.0 && ny == 0.0 {
            return Some(Rect::new(l, t, r, b));
        }

        // If we got here, we were not empty, and at least one of the span values was either
        // an Infinity or NaN -- so we return failure (no finite bounds)
        None
    }

    /// Returns the bounds of `pts`, or the empty rectangle if any point is not finite.
    // Port of: include/core/SkRect.h#L918-L925 (chrome/m156)
    #[doc(alias = "BoundsOrEmpty")]
    #[must_use]
    pub fn bounds_or_empty(pts: &[Point]) -> Rect {
        Self::bounds(pts).unwrap_or_else(Self::new_empty)
    }

    /// Sets to the bounds of `points`; sets to empty if any point is not finite.
    #[doc(alias = "setBounds")]
    pub fn set_bounds(&mut self, points: &[Point]) {
        let _ = self.set_bounds_check(points);
    }

    /// Sets to the bounds of `points` and returns true if all points were finite; otherwise
    /// sets to empty and returns false.
    // Port of: src/core/SkRect.cpp#L121-L129 (chrome/m156)
    #[doc(alias = "setBoundsCheck")]
    pub fn set_bounds_check(&mut self, points: &[Point]) -> bool {
        if let Some(bounds) = Self::bounds(points) {
            *self = bounds;
            true
        } else {
            *self = Self::new_empty();
            false
        }
    }

    /// Sets to the bounds of `points`; sets all edges to NaN if any point is not finite.
    // Port of: src/core/SkRect.cpp#L131-L137 (chrome/m156)
    #[doc(alias = "setBoundsNoCheck")]
    pub fn set_bounds_no_check(&mut self, points: &[Point]) {
        if let Some(bounds) = Self::bounds(points) {
            *self = bounds;
        } else {
            self.set_ltrb(SCALAR_NAN, SCALAR_NAN, SCALAR_NAN, SCALAR_NAN);
        }
    }

    /// Sets to the bounds of the two points (`SkRect::set(const SkPoint&, const SkPoint&)`).
    // Port of: include/core/SkRect.h#L960-L965 (chrome/m156)
    #[doc(alias = "set")]
    pub fn set_bounds2(&mut self, p0: impl Into<Point>, p1: impl Into<Point>) {
        let (p0, p1) = (p0.into(), p1.into());
        self.left = std_min(p0.x, p1.x);
        self.right = std_max(p0.x, p1.x);
        self.top = std_min(p0.y, p1.y);
        self.bottom = std_max(p0.y, p1.y);
    }

    /// Returns the bounds of `points` if all are finite.
    #[must_use]
    pub fn from_bounds(points: &[Point]) -> Option<Self> {
        Self::bounds(points)
    }

    /// Sets to `(x, y, x + width, y + height)`.
    #[doc(alias = "setXYWH")]
    pub fn set_xywh(&mut self, x: scalar, y: scalar, width: scalar, height: scalar) {
        *self = Self::from_xywh(x, y, width, height);
    }

    /// Sets to `(0, 0, width, height)`.
    #[doc(alias = "setWH")]
    pub fn set_wh(&mut self, w: scalar, h: scalar) {
        *self = Self::from_wh(w, h);
    }

    /// Sets to `(0, 0, width, height)` from integers.
    #[doc(alias = "setIWH")]
    pub fn set_iwh(&mut self, width: i32, height: i32) {
        *self = Self::from_iwh(width, height);
    }

    /// Returns the rectangle moved by `d`.
    // Port of: include/core/SkRect.h#L1009-L1011 (chrome/m156)
    #[doc(alias = "makeOffset")]
    #[must_use]
    pub fn with_offset(&self, d: impl Into<Vector>) -> Self {
        let d = d.into();
        Self::new(
            self.left + d.x,
            self.top + d.y,
            self.right + d.x,
            self.bottom + d.y,
        )
    }

    /// Returns the rectangle with edges moved inwards by `d`.
    // Port of: include/core/SkRect.h#L1031-L1033 (chrome/m156)
    #[doc(alias = "makeInset")]
    #[must_use]
    pub fn with_inset(&self, d: impl Into<Vector>) -> Self {
        let d = d.into();
        Self::new(
            self.left + d.x,
            self.top + d.y,
            self.right - d.x,
            self.bottom - d.y,
        )
    }

    /// Returns the rectangle with edges moved outwards by `d`.
    // Port of: include/core/SkRect.h#L1046-L1048 (chrome/m156)
    #[doc(alias = "makeOutset")]
    #[must_use]
    pub fn with_outset(&self, d: impl Into<Vector>) -> Self {
        let d = d.into();
        Self::new(
            self.left - d.x,
            self.top - d.y,
            self.right + d.x,
            self.bottom + d.y,
        )
    }

    /// Offsets the rectangle by `d`.
    // Port of: include/core/SkRect.h#L1060-L1082 (chrome/m156)
    pub fn offset(&mut self, d: impl Into<Vector>) {
        let d = d.into();
        self.left += d.x;
        self.top += d.y;
        self.right += d.x;
        self.bottom += d.y;
    }

    /// Offsets so that the top-left is at `new_p`, keeping the size.
    // Port of: include/core/SkRect.h#L1087-L1092 (chrome/m156)
    #[doc(alias = "offsetTo")]
    pub fn offset_to(&mut self, new_p: impl Into<Point>) {
        let new_p = new_p.into();
        self.right += new_p.x - self.left;
        self.bottom += new_p.y - self.top;
        self.left = new_p.x;
        self.top = new_p.y;
    }

    /// Returns the rectangle offset so that its top-left is at `new_p`.
    #[doc(alias = "offsetTo")]
    #[must_use]
    pub fn with_offset_to(&self, new_p: impl Into<Point>) -> Self {
        let mut r = *self;
        r.offset_to(new_p);
        r
    }

    /// Insets the rectangle by `d`.
    // Port of: include/core/SkRect.h#L1104-L1109 (chrome/m156)
    pub fn inset(&mut self, d: impl Into<Vector>) {
        let d = d.into();
        self.left += d.x;
        self.top += d.y;
        self.right -= d.x;
        self.bottom -= d.y;
    }

    /// Outsets the rectangle by `d`.
    // Port of: include/core/SkRect.h#L1121 (chrome/m156)
    pub fn outset(&mut self, d: impl Into<Vector>) {
        let d = d.into();
        self.inset((-d.x, -d.y));
    }

    /// Sets this to the intersection with `r` and returns true if they intersect; otherwise
    /// returns false and leaves this unchanged.
    // Port of: src/core/SkRect.cpp#L139-L151 (chrome/m156)
    pub fn intersect(&mut self, r: impl AsRef<Rect>) -> bool {
        let r = r.as_ref();
        let a = *r;
        let b = *self;
        self.intersect2(a, b)
    }

    /// Sets this to the intersection of `a` and `b` and returns true if they intersect;
    /// otherwise returns false and leaves this unchanged.
    // Port of: src/core/SkRect.cpp#L153-L156 (chrome/m156)
    #[doc(alias = "intersect")]
    #[must_use]
    pub fn intersect2(&mut self, a: impl AsRef<Rect>, b: impl AsRef<Rect>) -> bool {
        let (a, b) = (a.as_ref(), b.as_ref());
        let left = std_max(a.left, b.left);
        let right = std_min(a.right, b.right);
        let top = std_max(a.top, b.top);
        let bottom = std_min(a.bottom, b.bottom);
        // do the !(opposite) check so we return false if either arg is NaN
        if !(left < right && top < bottom) {
            return false;
        }
        self.set_ltrb(left, top, right, bottom);
        true
    }

    // Port of: include/core/SkRect.h#L1148-L1157 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors Skia's private helper
    fn intersects_(
        al: scalar,
        at: scalar,
        ar: scalar,
        ab: scalar,
        bl: scalar,
        bt: scalar,
        br: scalar,
        bb: scalar,
    ) -> bool {
        let l = std_max(al, bl);
        let r = std_min(ar, br);
        let t = std_max(at, bt);
        let b = std_min(ab, bb);
        l < r && t < b
    }

    /// Returns true if this intersects `r`.
    // Port of: include/core/SkRect.h#L1165-L1168 (chrome/m156)
    #[must_use]
    pub fn intersects(&self, r: impl AsRef<Rect>) -> bool {
        let r = r.as_ref();
        Self::intersects_(
            self.left,
            self.top,
            self.right,
            self.bottom,
            r.left,
            r.top,
            r.right,
            r.bottom,
        )
    }

    /// Returns true if `a` intersects `b`.
    // Port of: include/core/SkRect.h#L1177-L1180 (chrome/m156)
    #[doc(alias = "Intersects")]
    #[must_use]
    pub fn intersects2(a: impl AsRef<Rect>, b: impl AsRef<Rect>) -> bool {
        a.as_ref().intersects(b)
    }

    /// Sets this to the union of itself and `r`. No effect if `r` is empty; if this is empty,
    /// sets this to `r`.
    // Port of: src/core/SkRect.cpp#L159-L172 (chrome/m156)
    pub fn join(&mut self, r: impl AsRef<Rect>) {
        let r = r.as_ref();
        if r.is_empty() {
            return;
        }

        if self.is_empty() {
            *self = *r;
        } else {
            self.left = std_min(self.left, r.left);
            self.top = std_min(self.top, r.top);
            self.right = std_max(self.right, r.right);
            self.bottom = std_max(self.bottom, r.bottom);
        }
    }

    /// Returns the union of `a` and `b` (see [`Self::join`]).
    #[must_use]
    pub fn join2(a: impl AsRef<Rect>, b: impl AsRef<Rect>) -> Rect {
        let mut result = *a.as_ref();
        result.join(b);
        result
    }

    /// Like [`Self::join`] but `r` must be non-empty.
    // Port of: include/core/SkRect.h#L1202-L1210 (chrome/m156)
    #[doc(alias = "joinNonEmptyArg")]
    pub fn join_non_empty_arg(&mut self, r: impl AsRef<Rect>) {
        let r = r.as_ref();
        debug_assert!(!r.is_empty());
        // if we are empty, just assign
        if self.left >= self.right || self.top >= self.bottom {
            *self = *r;
        } else {
            self.join_possibly_empty_rect(r);
        }
    }

    /// Like [`Self::join`] but treats both rectangles as plain bounds, empty or not.
    // Port of: include/core/SkRect.h#L1218-L1223 (chrome/m156)
    #[doc(alias = "joinPossiblyEmptyRect")]
    pub fn join_possibly_empty_rect(&mut self, r: impl AsRef<Rect>) {
        let r = r.as_ref();
        self.left = std_min(self.left, r.left);
        self.top = std_min(self.top, r.top);
        self.right = std_max(self.right, r.right);
        self.bottom = std_max(self.bottom, r.bottom);
    }

    /// Returns the rectangle rounded to the nearest integers.
    // Port of: include/core/SkRect.h#L1272-L1282 (chrome/m156)
    #[must_use]
    pub fn round(&self) -> IRect {
        IRect::new(
            float_round2int(self.left),
            float_round2int(self.top),
            float_round2int(self.right),
            float_round2int(self.bottom),
        )
    }

    /// Returns the largest integer rectangle contained in this.
    // Port of: include/core/SkRect.h#L1310-L1315 (chrome/m156)
    #[doc(alias = "roundIn")]
    #[must_use]
    pub fn round_in(&self) -> IRect {
        IRect::new(
            float_ceil2int(self.left),
            float_ceil2int(self.top),
            float_floor2int(self.right),
            float_floor2int(self.bottom),
        )
    }

    /// Swaps edges so that `left <= right` and `top <= bottom`.
    // Port of: include/core/SkRect.h#L1357-L1366 (chrome/m156)
    pub fn sort(&mut self) {
        if self.left > self.right {
            std::mem::swap(&mut self.left, &mut self.right);
        }
        if self.top > self.bottom {
            std::mem::swap(&mut self.top, &mut self.bottom);
        }
    }

    /// Returns the rectangle with edges sorted.
    // Port of: include/core/SkRect.h#L1374-L1377 (chrome/m156)
    #[doc(alias = "makeSorted")]
    #[must_use]
    pub fn sorted(&self) -> Rect {
        Rect::new(
            std_min(self.left, self.right),
            std_min(self.top, self.bottom),
            std_max(self.left, self.right),
            std_max(self.top, self.bottom),
        )
    }

    /// Returns the edges as `[left, top, right, bottom]` (`SkRect::asScalars`).
    ///
    /// Skia returns a pointer to the storage; safe Rust returns a copy.
    #[doc(alias = "asScalars")]
    #[must_use]
    pub const fn as_scalars(&self) -> [scalar; 4] {
        [self.left, self.top, self.right, self.bottom]
    }
}

impl Contains<&Point> for Rect {
    /// True if `x >= left && x < right && y >= top && y < bottom`.
    // Port of: include/core/SkRect.h#L1232-L1234 (chrome/m156)
    fn contains(&self, p: &Point) -> bool {
        p.x >= self.left && p.x < self.right && p.y >= self.top && p.y < self.bottom
    }
}

impl Contains<Point> for Rect {
    fn contains(&self, p: Point) -> bool {
        self.contains(&p)
    }
}

impl Contains<&Rect> for Rect {
    /// True if `r` is non-empty, this is non-empty and `r` lies within this.
    // Port of: include/core/SkRect.h#L1244-L1249 (chrome/m156)
    fn contains(&self, r: &Rect) -> bool {
        !r.is_empty()
            && !self.is_empty()
            && self.left <= r.left
            && self.top <= r.top
            && self.right >= r.right
            && self.bottom >= r.bottom
    }
}

impl Contains<Rect> for Rect {
    fn contains(&self, r: Rect) -> bool {
        self.contains(&r)
    }
}

impl Contains<&IRect> for Rect {
    /// True if `r` is non-empty, this is non-empty and `r` lies within this.
    // Port of: include/core/SkRect.h#L1259-L1264 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors the implicit int -> float conversion
    fn contains(&self, r: &IRect) -> bool {
        !r.is_empty()
            && !self.is_empty()
            && self.left <= r.left as scalar
            && self.top <= r.top as scalar
            && self.right >= r.right as scalar
            && self.bottom >= r.bottom as scalar
    }
}

impl Contains<IRect> for Rect {
    fn contains(&self, r: IRect) -> bool {
        self.contains(&r)
    }
}

/// Rounds a [`Rect`] outwards to a rectangle of type `R` (`SkRect::roundOut` overloads).
pub trait RoundOut<R> {
    /// Returns the smallest `R` containing this rectangle.
    fn round_out(&self) -> R;
}

impl RoundOut<IRect> for Rect {
    // Port of: include/core/SkRect.h#L1285-L1290 (chrome/m156)
    fn round_out(&self) -> IRect {
        IRect::new(
            float_floor2int(self.left),
            float_floor2int(self.top),
            float_ceil2int(self.right),
            float_ceil2int(self.bottom),
        )
    }
}

impl RoundOut<Rect> for Rect {
    // Port of: include/core/SkRect.h#L1298-L1301 (chrome/m156)
    fn round_out(&self) -> Rect {
        Rect::new(
            self.left.floor(),
            self.top.floor(),
            self.right.ceil(),
            self.bottom.ceil(),
        )
    }
}

impl From<(Point, Size)> for Rect {
    fn from((point, size): (Point, Size)) -> Self {
        Self::from_point_and_size(point, size)
    }
}

impl From<ISize> for Rect {
    fn from(isize: ISize) -> Self {
        Self::from_isize(isize)
    }
}

impl From<IRect> for Rect {
    fn from(irect: IRect) -> Self {
        Self::from_irect(irect)
    }
}

/// Private helpers of `SkRect` (`SkRectPriv.h`), used by Skia's own code and tests.
#[doc(hidden)]
pub mod rect_priv {
    use super::{Contains, IRect, Rect, std_max, std_min};
    use crate::floating_point::float_midpoint;
    use crate::math::{MAX_S32, MIN_S32};
    use crate::math_priv::fits_in_fixed;
    use crate::point::Point;
    use crate::scalar::{SCALAR_MAX, SCALAR_MIN, scalar};
    use crate::t_pin::t_pin;

    /// Returns an irect that is very large, and can be safely round-tripped with [`Rect`] and
    /// still be considered non-empty (width/height > 0) even if we round-out the [`Rect`].
    // Port of: src/core/SkRectPriv.h#L21-L27 (chrome/m156)
    #[doc(alias = "MakeILarge")]
    #[must_use]
    pub fn make_i_large() -> IRect {
        // SK_MaxS32 >> 1 seemed better, but it did not survive round-trip with SkRect and
        // rounding. Also, 1 << 29 can be perfectly represented in float, while SK_MaxS32 >> 1
        // cannot.
        let large: i32 = 1 << 29;
        IRect::new(-large, -large, large, large)
    }

    /// Returns an inverted irect spanning the whole `i32` range.
    // Port of: src/core/SkRectPriv.h#L29-L31 (chrome/m156)
    #[doc(alias = "MakeILargestInverted")]
    #[must_use]
    pub fn make_i_largest_inverted() -> IRect {
        IRect::new(MAX_S32, MAX_S32, MIN_S32, MIN_S32)
    }

    /// [`make_i_large`] as a [`Rect`].
    // Port of: src/core/SkRectPriv.h#L33-L37 (chrome/m156)
    #[doc(alias = "MakeLargeS32")]
    #[must_use]
    pub fn make_large_s32() -> Rect {
        let mut r = Rect::new_empty();
        r.set_irect(make_i_large());
        r
    }

    /// Returns the largest finite rectangle.
    // Port of: src/core/SkRectPriv.h#L39-L41 (chrome/m156)
    #[doc(alias = "MakeLargest")]
    #[must_use]
    pub fn make_largest() -> Rect {
        Rect::new(SCALAR_MIN, SCALAR_MIN, SCALAR_MAX, SCALAR_MAX)
    }

    /// Returns the inverted largest rectangle.
    // Port of: src/core/SkRectPriv.h#L43-L45 (chrome/m156)
    #[doc(alias = "MakeLargestInverted")]
    #[must_use]
    pub const fn make_largest_inverted() -> Rect {
        Rect::new(SCALAR_MAX, SCALAR_MAX, SCALAR_MIN, SCALAR_MIN)
    }

    /// Grows `r` to include `pt`.
    // Port of: src/core/SkRectPriv.h#L47-L52 (chrome/m156)
    #[doc(alias = "GrowToInclude")]
    pub fn grow_to_include(r: &mut Rect, pt: Point) {
        r.left = std_min(pt.x, r.left);
        r.right = std_max(pt.x, r.right);
        r.top = std_min(pt.y, r.top);
        r.bottom = std_max(pt.y, r.bottom);
    }

    /// Conservative check if `r` can be expressed in fixed-point. Returns false for very large
    /// values that might have fit.
    // Port of: src/core/SkRectPriv.h#L54-L58 (chrome/m156)
    #[doc(alias = "FitsInFixed")]
    #[must_use]
    pub fn fits_in_fixed_rect(r: &Rect) -> bool {
        fits_in_fixed(r.left)
            && fits_in_fixed(r.top)
            && fits_in_fixed(r.right)
            && fits_in_fixed(r.bottom)
    }

    /// Returns `r.width() / 2` but divides first to avoid `width()` overflowing.
    // Port of: src/core/SkRectPriv.h#L60-L62 (chrome/m156)
    #[doc(alias = "HalfWidth")]
    #[must_use]
    pub fn half_width(r: &Rect) -> scalar {
        float_midpoint(-r.left, r.right)
    }

    /// Returns `r.height() / 2` but divides first to avoid `height()` overflowing.
    // Port of: src/core/SkRectPriv.h#L64-L66 (chrome/m156)
    #[doc(alias = "HalfHeight")]
    #[must_use]
    pub fn half_height(r: &Rect) -> scalar {
        float_midpoint(-r.top, r.bottom)
    }

    // Port of: src/core/SkRect.cpp#L213-L287 (chrome/m156)
    // The C++ `subtract<R>` template, instantiated for each rectangle type: `$sub` computes
    // `x - y` (in the coordinate type) and `$to_float` the implicit conversion to float.
    macro_rules! impl_subtract {
        ($name:ident, $R:ty, $intersects:expr, $sub:expr, $to_float:expr, $width:expr, $height:expr) => {
            #[allow(clippy::float_cmp)] // mirrors exact comparisons of the C++
            fn $name(a: &$R, b: &$R, out: &mut $R) -> bool {
                if a.is_empty() || b.is_empty() || !$intersects(a, b) {
                    // Either already empty, or subtracting the empty rect, or there's no
                    // intersection, so in all cases the answer is A.
                    *out = *a;
                    return true;
                }

                // 4 rectangles to consider. If the edge in A is contained in B, the resulting
                // difference can be represented exactly as a rectangle. Otherwise the
                // difference is the largest subrectangle that is disjoint from B:
                // 1. Left part of A:   (A.left,  A.top,    B.left,  A.bottom)
                // 2. Right part of A:  (B.right, A.top,    A.right, A.bottom)
                // 3. Top part of A:    (A.left,  A.top,    A.right, B.top)
                // 4. Bottom part of A: (A.left,  B.bottom, A.right, A.bottom)
                //
                // Depending on how B intersects A, there will be 1 to 4 positive areas:
                //  - 4 occur when A contains B
                //  - 3 occur when B intersects a single edge
                //  - 2 occur when B intersects at a corner, or spans two opposing edges
                //  - 1 occurs when B spans two opposing edges and contains a 3rd, resulting in
                //    an exact rect
                //  - 0 occurs when B contains A, resulting in the empty rect
                //
                // Compute the relative areas of the 4 rects described above. Since each
                // subrectangle shares either the width or height of A, we only have to divide
                // by the other dimension, which avoids overflow on int32 types, and even if the
                // float relative areas overflow to infinity, the comparisons work out correctly
                // and (one of) the infinitely large subrects will be chosen.
                let a_height: f32 = $to_float($height(a));
                let a_width: f32 = $to_float($width(a));

                let mut left_area: f32 = 0.0;
                let mut right_area: f32 = 0.0;
                let mut top_area: f32 = 0.0;
                let mut bottom_area: f32 = 0.0;
                let mut positive_count = 0;
                if b.left > a.left {
                    left_area = $to_float($sub(b.left, a.left)) / a_width;
                    positive_count += 1;
                }
                if a.right > b.right {
                    right_area = $to_float($sub(a.right, b.right)) / a_width;
                    positive_count += 1;
                }
                if b.top > a.top {
                    top_area = $to_float($sub(b.top, a.top)) / a_height;
                    positive_count += 1;
                }
                if a.bottom > b.bottom {
                    bottom_area = $to_float($sub(a.bottom, b.bottom)) / a_height;
                    positive_count += 1;
                }

                if positive_count == 0 {
                    debug_assert!(b.contains(a));
                    *out = <$R>::new_empty();
                    return true;
                }

                *out = *a;
                if left_area > right_area && left_area > top_area && left_area > bottom_area {
                    // Left chunk of A, so the new right edge is B's left edge
                    out.right = b.left;
                } else if right_area > top_area && right_area > bottom_area {
                    // Right chunk of A, so the new left edge is B's right edge
                    out.left = b.right;
                } else if top_area > bottom_area {
                    // Top chunk of A, so the new bottom edge is B's top edge
                    out.bottom = b.top;
                } else {
                    // Bottom chunk of A, so the new top edge is B's bottom edge
                    debug_assert!(bottom_area > 0.0);
                    out.top = b.bottom;
                }

                // If we have 1 valid area, the disjoint shape is representable as a rectangle.
                debug_assert!(!$intersects(out, b));
                positive_count == 1
            }
        };
    }

    #[allow(clippy::cast_precision_loss)] // mirrors the implicit int -> float conversion
    fn int_to_float(x: i32) -> f32 {
        x as f32
    }

    impl_subtract!(
        subtract_rect_impl,
        Rect,
        |a: &Rect, b: &Rect| Rect::intersects2(a, b),
        |x: f32, y: f32| x - y,
        |x: f32| x,
        |r: &Rect| r.width(),
        |r: &Rect| r.height()
    );
    impl_subtract!(
        subtract_irect_impl,
        IRect,
        |a: &IRect, b: &IRect| IRect::intersects(a, b),
        |x: i32, y: i32| x.wrapping_sub(y),
        int_to_float,
        |r: &IRect| r.width(),
        |r: &IRect| r.height()
    );

    /// Evaluates `a - b`. If the difference shape cannot be represented as a rectangle then
    /// false is returned and `out` is set to the largest rectangle contained in said shape. If
    /// true is returned then `a - b` is representable as a rectangle, which is stored in `out`
    /// (`SkRectPriv::Subtract(SkRect, SkRect, SkRect*)`).
    // Port of: src/core/SkRect.cpp#L289-L291 (chrome/m156)
    #[doc(alias = "Subtract")]
    pub fn subtract(a: &Rect, b: &Rect, out: &mut Rect) -> bool {
        subtract_rect_impl(a, b, out)
    }

    /// [`subtract`] for [`IRect`] (`SkRectPriv::Subtract(SkIRect, SkIRect, SkIRect*)`).
    // Port of: src/core/SkRect.cpp#L293-L295 (chrome/m156)
    #[doc(alias = "Subtract")]
    pub fn subtract_irect(a: &IRect, b: &IRect, out: &mut IRect) -> bool {
        subtract_irect_impl(a, b, out)
    }

    /// Evaluates `a - b` and returns the largest rectangle contained in that shape. The
    /// returned rectangle will not intersect `b` (`SkRectPriv::Subtract(SkRect, SkRect)`).
    // Port of: src/core/SkRectPriv.h#L79-L83 (chrome/m156)
    #[doc(alias = "Subtract")]
    #[must_use]
    pub fn subtract_diff(a: &Rect, b: &Rect) -> Rect {
        let mut diff = Rect::new_empty();
        subtract(a, b, &mut diff);
        diff
    }

    /// [`subtract_diff`] for [`IRect`].
    // Port of: src/core/SkRectPriv.h#L84-L88 (chrome/m156)
    #[doc(alias = "Subtract")]
    #[must_use]
    pub fn subtract_irect_diff(a: &IRect, b: &IRect) -> IRect {
        let mut diff = IRect::new_empty();
        subtract_irect(a, b, &mut diff);
        diff
    }

    /// Assuming `src` does not intersect `dst`, returns the edge or corner of `src` that is
    /// closest to `dst`, e.g. the pixels that would be sampled from `src` when clamp-tiled into
    /// `dst`.
    ///
    /// The returned rectangle will not be empty if `src` is not empty and `dst` is not empty. At
    /// least one of its width or height will be equal to 1 (possibly both if a corner is
    /// closest). Returns `src` intersected with `dst` if they do actually intersect.
    // Port of: src/core/SkRect.cpp#L360-L393 (chrome/m156)
    #[doc(alias = "ClosestDisjointEdge")]
    #[must_use]
    pub fn closest_disjoint_edge(src: &IRect, dst: &IRect) -> IRect {
        if src.is_empty() || dst.is_empty() {
            return IRect::new_empty();
        }

        let mut l = src.left;
        let mut r = src.right;
        if r <= dst.left {
            // Select right column of pixels in crop
            l = r - 1;
        } else if l >= dst.right {
            // Left column of 'crop'
            r = l + 1;
        } else {
            // Regular intersection along X axis.
            l = t_pin(l, dst.left, dst.right);
            r = t_pin(r, dst.left, dst.right);
        }

        let mut t = src.top;
        let mut b = src.bottom;
        if b <= dst.top {
            // Select bottom row of pixels in crop
            t = b - 1;
        } else if t >= dst.bottom {
            // Top row of 'crop'
            b = t + 1;
        } else {
            t = t_pin(t, dst.top, dst.bottom);
            b = t_pin(b, dst.top, dst.bottom);
        }

        IRect::new(l, t, r, b)
    }
}
