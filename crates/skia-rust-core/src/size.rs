// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkSize.h

//! Integer and scalar sizes (`SkSize.h`).

use crate::scalar::{scalar, scalar_ceil_to_int, scalar_floor_to_int, scalar_round_to_int};
use std::ops::{Div, DivAssign, Mul, MulAssign};

/// A size with 32-bit integer width and height.
// Port of: include/core/SkSize.h#L16-L46 (chrome/m156)
#[doc(alias = "SkISize")]
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct ISize {
    /// Width.
    pub width: i32,
    /// Height.
    pub height: i32,
}

impl ISize {
    /// Constructs a size from `w` and `h` (`SkISize::Make`).
    #[doc(alias = "SkISize::Make")]
    #[must_use]
    pub const fn new(w: i32, h: i32) -> Self {
        Self {
            width: w,
            height: h,
        }
    }

    /// The `(0, 0)` size (`SkISize::MakeEmpty`).
    #[doc(alias = "MakeEmpty")]
    #[must_use]
    pub const fn new_empty() -> Self {
        Self::new(0, 0)
    }

    /// Sets width and height.
    pub fn set(&mut self, w: i32, h: i32) {
        *self = Self::new(w, h);
    }

    /// Returns true iff `width == 0 && height == 0`.
    #[doc(alias = "isZero")]
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.width == 0 && self.height == 0
    }

    /// Returns true if either width or height are `<= 0`.
    #[doc(alias = "isEmpty")]
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    /// Sets the width and height to `0`.
    #[doc(alias = "setEmpty")]
    pub fn set_empty(&mut self) {
        self.width = 0;
        self.height = 0;
    }

    /// Returns `width * height` as an `i64`.
    #[must_use]
    pub const fn area(self) -> i64 {
        // SkToS64 of an i32 is lossless.
        (self.width as i64) * (self.height as i64)
    }

    /// Returns true if the size equals `(w, h)`.
    #[must_use]
    pub fn equals(self, w: i32, h: i32) -> bool {
        self.width == w && self.height == h
    }
}

impl From<(i32, i32)> for ISize {
    fn from(source: (i32, i32)) -> Self {
        Self::new(source.0, source.1)
    }
}

/// A size with scalar width and height.
// Port of: include/core/SkSize.h#L52-L86 (chrome/m156)
#[doc(alias = "SkSize")]
#[derive(Copy, Clone, PartialEq, Default, Debug)]
pub struct Size {
    /// Width.
    pub width: scalar,
    /// Height.
    pub height: scalar,
}

impl Size {
    /// Constructs a size from `w` and `h` (`SkSize::Make`).
    #[doc(alias = "SkSize::Make")]
    #[must_use]
    pub const fn new(w: scalar, h: scalar) -> Self {
        Self {
            width: w,
            height: h,
        }
    }

    /// Constructs a size from an integer size (`SkSize::Make(const SkISize&)`).
    #[doc(alias = "SkSize::Make")]
    #[allow(clippy::cast_precision_loss)] // mirrors SkIntToScalar
    #[must_use]
    pub const fn from_isize(src: ISize) -> Self {
        Self::new(src.width as scalar, src.height as scalar)
    }

    /// The `(0, 0)` size (`SkSize::MakeEmpty`).
    #[doc(alias = "MakeEmpty")]
    #[must_use]
    pub const fn new_empty() -> Self {
        Self::new(0.0, 0.0)
    }

    /// Sets width and height.
    pub fn set(&mut self, w: scalar, h: scalar) {
        *self = Self::new(w, h);
    }

    /// Returns true iff `width == 0 && height == 0`.
    #[doc(alias = "isZero")]
    #[must_use]
    pub fn is_zero(self) -> bool {
        self.width == 0.0 && self.height == 0.0
    }

    /// Returns true if either width or height are `<= 0`.
    #[doc(alias = "isEmpty")]
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    /// Sets the width and height to `0`.
    #[doc(alias = "setEmpty")]
    pub fn set_empty(&mut self) {
        *self = Self::new_empty();
    }

    /// Returns true if the size equals `(w, h)`.
    #[allow(clippy::float_cmp)] // exact comparison, as in Skia
    #[must_use]
    pub fn equals(self, w: scalar, h: scalar) -> bool {
        self.width == w && self.height == h
    }

    /// Rounds each component to the nearest integer.
    #[doc(alias = "toRound")]
    #[must_use]
    pub fn to_round(self) -> ISize {
        ISize::new(
            scalar_round_to_int(self.width),
            scalar_round_to_int(self.height),
        )
    }

    /// Rounds each component up to an integer.
    #[doc(alias = "toCeil")]
    #[must_use]
    pub fn to_ceil(self) -> ISize {
        ISize::new(
            scalar_ceil_to_int(self.width),
            scalar_ceil_to_int(self.height),
        )
    }

    /// Rounds each component down to an integer.
    #[doc(alias = "toFloor")]
    #[must_use]
    pub fn to_floor(self) -> ISize {
        ISize::new(
            scalar_floor_to_int(self.width),
            scalar_floor_to_int(self.height),
        )
    }
}

impl From<(scalar, scalar)> for Size {
    fn from(source: (scalar, scalar)) -> Self {
        Self::new(source.0, source.1)
    }
}

impl From<ISize> for Size {
    fn from(size: ISize) -> Self {
        Self::from_isize(size)
    }
}

#[allow(clippy::cast_precision_loss)] // mirrors SkIntToScalar
impl From<(i32, i32)> for Size {
    fn from(source: (i32, i32)) -> Self {
        Self::new(source.0 as scalar, source.1 as scalar)
    }
}

// `SkSize.h` has no arithmetic operators; skia-safe adds these four.
impl Div<scalar> for Size {
    type Output = Self;
    fn div(self, rhs: scalar) -> Self {
        Self::new(self.width / rhs, self.height / rhs)
    }
}

impl DivAssign<scalar> for Size {
    fn div_assign(&mut self, rhs: scalar) {
        *self = *self / rhs;
    }
}

impl Mul<scalar> for Size {
    type Output = Self;
    fn mul(self, rhs: scalar) -> Self {
        Self::new(self.width * rhs, self.height * rhs)
    }
}

impl MulAssign<scalar> for Size {
    fn mul_assign(&mut self, rhs: scalar) {
        *self = *self * rhs;
    }
}
