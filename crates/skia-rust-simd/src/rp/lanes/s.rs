// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h (SKRP_CPU_SCALAR lane types)

//! [`S<T>`]: the one-lane type of the `Scalar` tier.

use core::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Div, DivAssign,
    Index, IndexMut, Mul, MulAssign, Neg, Not, Shl, ShlAssign, Shr, ShrAssign, Sub, SubAssign,
};

use crate::vx::{CastFrom, IntLane, Lane};

// Port of: src/opts/SkRasterPipeline_opts.h#L124-L131 (chrome/m156)
/// One lane of `T` with C scalar semantics: the `Scalar` tier's `F` (`float`), `I32`
/// (`int32_t`), `U32`, `U16`, `U8` and `U64`.
///
/// It has the same method names as [`vx::Vec`](crate::vx::Vec), so stage code stamped for every
/// tier (design §2.5) compiles against either, but **comparisons yield `0` or `1`** (a C `bool`
/// converted to an integer), not all-ones masks. Skia's `cond_to_mask` (the lane modules'
/// `cond_to_mask`) exists to turn those into masks where a stage needs one.
///
/// Arithmetic is IEEE for floats and wrapping for integers (Skia's integer lane arithmetic never
/// relies on signed overflow being undefined).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(transparent)]
pub struct S<T>(pub T);

impl<T: Lane> From<T> for S<T> {
    #[inline(always)]
    fn from(s: T) -> Self {
        Self(s)
    }
}

impl<T> Index<usize> for S<T> {
    type Output = T;
    /// `data[0]` (`select_lane` on the scalar path). Panics for any other index.
    #[inline(always)]
    fn index(&self, i: usize) -> &T {
        assert!(i == 0, "S has a single lane");
        &self.0
    }
}

impl<T> IndexMut<usize> for S<T> {
    #[inline(always)]
    fn index_mut(&mut self, i: usize) -> &mut T {
        assert!(i == 0, "S has a single lane");
        &mut self.0
    }
}

impl<T: Lane> S<T> {
    /// The value `s` (the counterpart of [`Vec::splat`](crate::vx::Vec::splat)).
    #[inline(always)]
    #[must_use]
    pub fn splat(s: T) -> Self {
        Self(s)
    }

    /// Reads `src[0]`.
    ///
    /// # Panics
    /// If `src` is empty.
    #[inline(always)]
    #[must_use]
    pub fn load(src: &[T]) -> Self {
        Self(src[0])
    }

    /// Writes the lane to `dst[0]`.
    ///
    /// # Panics
    /// If `dst` is empty.
    #[inline(always)]
    pub fn store(&self, dst: &mut [T]) {
        dst[0] = self.0;
    }

    /// Applies `f` to the lane.
    #[inline(always)]
    #[must_use]
    pub fn map<U: Lane>(self, f: impl FnOnce(T) -> U) -> S<U> {
        S(f(self.0))
    }

    /// `(D)x`: the C cast (float → integer uses Rust's saturating `as`, which is what the
    /// `Scalar` tier's real target, wasm32 with `nontrapping-fptoint`, does for C casts).
    #[inline(always)]
    #[must_use]
    pub fn cast<D: Lane + CastFrom<T>>(self) -> S<D> {
        S(D::cast_from(self.0))
    }

    /// `sk_bit_cast<U>(x)`: reinterprets the bytes (sizes must match, checked at compile time).
    #[inline(always)]
    #[must_use]
    #[doc(alias = "sk_bit_cast")]
    pub fn bit_cast<U: Lane>(self) -> S<U> {
        const {
            assert!(
                size_of::<T>() == size_of::<U>(),
                "bit_cast between lanes of different sizes"
            );
        }
        S(U::from_ne_byte_fn(|k| self.0.ne_byte(k)))
    }

    #[inline(always)]
    fn compare(self, rhs: impl Into<Self>, f: impl FnOnce(&T, &T) -> bool) -> S<T::Mask> {
        // A C comparison is a `bool`; converting it to an integer gives 0 or 1.
        let one = <T::Mask as IntLane>::ALL_ONES.lane_neg();
        S(if f(&self.0, &rhs.into().0) {
            one
        } else {
            <T::Mask as Lane>::ZERO
        })
    }

    /// `x == y` as `0`/`1`.
    #[inline(always)]
    #[must_use]
    pub fn eq_mask(self, rhs: impl Into<Self>) -> S<T::Mask> {
        self.compare(rhs, |a, b| a == b)
    }

    /// `x != y` as `0`/`1`.
    #[inline(always)]
    #[must_use]
    pub fn ne_mask(self, rhs: impl Into<Self>) -> S<T::Mask> {
        self.compare(rhs, |a, b| a != b)
    }

    /// `x < y` as `0`/`1`.
    #[inline(always)]
    #[must_use]
    pub fn lt_mask(self, rhs: impl Into<Self>) -> S<T::Mask> {
        self.compare(rhs, |a, b| a < b)
    }

    /// `x <= y` as `0`/`1`.
    #[inline(always)]
    #[must_use]
    pub fn le_mask(self, rhs: impl Into<Self>) -> S<T::Mask> {
        self.compare(rhs, |a, b| a <= b)
    }

    /// `x > y` as `0`/`1`.
    #[inline(always)]
    #[must_use]
    pub fn gt_mask(self, rhs: impl Into<Self>) -> S<T::Mask> {
        self.compare(rhs, |a, b| a > b)
    }

    /// `x >= y` as `0`/`1`.
    #[inline(always)]
    #[must_use]
    pub fn ge_mask(self, rhs: impl Into<Self>) -> S<T::Mask> {
        self.compare(rhs, |a, b| a >= b)
    }
}

macro_rules! impl_binop {
    ($tr:ident, $m:ident, $assign_tr:ident, $assign_m:ident, $lane:ident, $bound:ident) => {
        impl<T: $bound> $tr for S<T> {
            type Output = Self;
            #[inline(always)]
            fn $m(self, rhs: Self) -> Self {
                Self(self.0.$lane(rhs.0))
            }
        }
        impl<T: $bound> $tr<T> for S<T> {
            type Output = Self;
            #[inline(always)]
            fn $m(self, rhs: T) -> Self {
                Self(self.0.$lane(rhs))
            }
        }
        impl<T: $bound> $assign_tr for S<T> {
            #[inline(always)]
            fn $assign_m(&mut self, rhs: Self) {
                *self = $tr::$m(*self, rhs);
            }
        }
        impl<T: $bound> $assign_tr<T> for S<T> {
            #[inline(always)]
            fn $assign_m(&mut self, rhs: T) {
                *self = $tr::$m(*self, rhs);
            }
        }
    };
}
impl_binop!(Add, add, AddAssign, add_assign, lane_add, Lane);
impl_binop!(Sub, sub, SubAssign, sub_assign, lane_sub, Lane);
impl_binop!(Mul, mul, MulAssign, mul_assign, lane_mul, Lane);
impl_binop!(Div, div, DivAssign, div_assign, lane_div, Lane);
impl_binop!(
    BitXor,
    bitxor,
    BitXorAssign,
    bitxor_assign,
    lane_xor,
    IntLane
);
impl_binop!(
    BitAnd,
    bitand,
    BitAndAssign,
    bitand_assign,
    lane_and,
    IntLane
);
impl_binop!(BitOr, bitor, BitOrAssign, bitor_assign, lane_or, IntLane);

impl<T: Lane> Neg for S<T> {
    type Output = Self;
    #[inline(always)]
    fn neg(self) -> Self {
        Self(self.0.lane_neg())
    }
}

/// `~x`.
impl<T: IntLane> Not for S<T> {
    type Output = Self;
    #[inline(always)]
    fn not(self) -> Self {
        Self(self.0.lane_not())
    }
}

impl<T: IntLane> Shl<i32> for S<T> {
    type Output = Self;
    #[inline(always)]
    fn shl(self, k: i32) -> Self {
        Self(self.0.lane_shl(k))
    }
}

impl<T: IntLane> Shr<i32> for S<T> {
    type Output = Self;
    #[inline(always)]
    fn shr(self, k: i32) -> Self {
        Self(self.0.lane_shr(k))
    }
}

impl<T: IntLane> ShlAssign<i32> for S<T> {
    #[inline(always)]
    fn shl_assign(&mut self, k: i32) {
        *self = *self << k;
    }
}

impl<T: IntLane> ShrAssign<i32> for S<T> {
    #[inline(always)]
    fn shr_assign(&mut self, k: i32) {
        *self = *self >> k;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparisons_are_zero_or_one() {
        let x = S(1.5f32);
        assert_eq!(x.lt_mask(2.0), S(1i32));
        assert_eq!(x.gt_mask(2.0), S(0i32));
        assert_eq!(x.eq_mask(f32::NAN), S(0i32));
        assert_eq!(x.ne_mask(f32::NAN), S(1i32));
        assert_eq!(S(7u32).ge_mask(7), S(1u32));
        assert_eq!(S(-1i32).le_mask(0), S(1i32));
    }

    #[test]
    fn ops_and_casts() {
        assert_eq!(S(i32::MAX) + 1, S(i32::MIN));
        assert_eq!(S(3u32) - 4, S(u32::MAX));
        assert_eq!(!S(0u16), S(0xffff));
        assert_eq!(S(0x8000_0000u32) >> 31, S(1));
        assert_eq!(S(-8i32) >> 1, S(-4));
        assert_eq!(S(1.0f32).bit_cast::<u32>(), S(0x3f80_0000));
        assert_eq!(S(0xbf80_0000u32).bit_cast::<f32>(), S(-1.0));
        assert_eq!(S(-1.5f32).cast::<i32>(), S(-1));
        assert_eq!(S(-1.5f32).cast::<u32>(), S(0));
        assert_eq!(S(300u32).cast::<u8>(), S(44));
        let mut v = S(2.0f32);
        v *= 3.0;
        assert_eq!(v[0], 6.0);
        let mut out = [0.0f32; 2];
        S::load(&[4.0f32, 5.0]).store(&mut out);
        assert_eq!(out, [4.0, 0.0]);
    }
}
