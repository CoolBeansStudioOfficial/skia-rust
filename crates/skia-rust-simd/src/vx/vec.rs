// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkVx.h

//! The [`Vec`] type itself: constructors, lane access, comparisons, operators.

use core::array;
use core::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Div, DivAssign,
    Index, IndexMut, Mul, MulAssign, Neg, Not, Shl, ShlAssign, Shr, ShrAssign, Sub, SubAssign,
};

use super::lane::{CastFrom, IntLane, Lane};

// Port of: src/core/SkVx.h#L78-L236 (chrome/m156)
/// `skvx::Vec<N, T>`: `N` lanes of `T`.
///
/// Unlike Skia's recursive `lo`/`hi` layout this is a flat array with the same memory layout
/// (`T vec[N]`). `N` should be a power of two, as in Skia.
///
/// # Comparison operators
/// Skia's `==`, `!=`, `<`, `<=`, `>`, `>=` return a *mask vector*. Rust's operators cannot, so
/// they are the methods [`eq_mask`](Self::eq_mask), [`ne_mask`](Self::ne_mask),
/// [`lt_mask`](Self::lt_mask), [`le_mask`](Self::le_mask), [`gt_mask`](Self::gt_mask) and
/// [`ge_mask`](Self::ge_mask); they accept a vector or a scalar right-hand side
/// (`all(v < 5)` is `all(v.lt_mask(5.0))`). The derived [`PartialEq`] is plain whole-vector
/// equality (lane-wise `==`, so NaN lanes make two vectors unequal).
///
/// # Differences in operators
/// `~x` is Rust's `!x` ([`Not`]); Skia's lane-wise logical `!x` is [`Vec::logical_not`].
/// Scalars on either side of `+ - * / ^ & |` splat as in Skia.
#[doc(alias = "skvx::Vec")]
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(transparent)]
pub struct Vec<const N: usize, T>(pub [T; N]);

impl<const N: usize, T: Lane> Default for Vec<N, T> {
    /// All lanes zero, as `Vec<N,T>{}`.
    fn default() -> Self {
        Self([T::ZERO; N])
    }
}

impl<const N: usize, T: Lane> From<T> for Vec<N, T> {
    /// `Vec(T s)`: splat a scalar into every lane.
    fn from(s: T) -> Self {
        Self([s; N])
    }
}

impl<const N: usize, T> Index<usize> for Vec<N, T> {
    type Output = T;
    /// `operator[]`.
    fn index(&self, i: usize) -> &T {
        &self.0[i]
    }
}

impl<const N: usize, T> IndexMut<usize> for Vec<N, T> {
    fn index_mut(&mut self, i: usize) -> &mut T {
        &mut self.0[i]
    }
}

impl<const N: usize, T: Lane> Vec<N, T> {
    /// `Vec(T s)`: every lane is `s`.
    #[must_use]
    pub fn splat(s: T) -> Self {
        Self([s; N])
    }

    /// `Vec{a, b, ...}`: the first lanes are `xs`, the rest are zero.
    ///
    /// # Panics
    /// If `xs` has more than `N` elements.
    #[must_use]
    pub fn from_list(xs: &[T]) -> Self {
        assert!(xs.len() <= N);
        let mut vals = [T::ZERO; N];
        vals[..xs.len()].copy_from_slice(xs);
        Self(vals)
    }

    /// `Vec::Load(ptr)`: reads the first `N` elements of `src`.
    ///
    /// # Panics
    /// If `src` has fewer than `N` elements.
    #[must_use]
    #[doc(alias = "Load")]
    pub fn load(src: &[T]) -> Self {
        Self(array::from_fn(|i| src[i]))
    }

    /// `store(ptr)`: writes the `N` lanes to the start of `dst`.
    ///
    /// # Panics
    /// If `dst` has fewer than `N` elements.
    pub fn store(&self, dst: &mut [T]) {
        dst[..N].copy_from_slice(&self.0);
    }

    /// Applies `f` to each lane: `map(fn, x)`.
    #[must_use]
    pub fn map<U: Lane>(self, mut f: impl FnMut(T) -> U) -> Vec<N, U> {
        Vec(array::from_fn(|i| f(self.0[i])))
    }

    /// `cast<D>(x)`: a C cast of every lane.
    #[must_use]
    // Port of: src/core/SkVx.h#L628-L640 (chrome/m156)
    pub fn cast<D: Lane + CastFrom<T>>(self) -> Vec<N, D> {
        Vec(array::from_fn(|i| D::cast_from(self.0[i])))
    }

    // Comparisons. Port of: src/core/SkVx.h#L313-L330 and L432-L451 (chrome/m156)

    fn compare(self, rhs: impl Into<Self>, f: impl Fn(&T, &T) -> bool) -> Vec<N, T::Mask> {
        let rhs = rhs.into();
        Vec(array::from_fn(|i| {
            if f(&self.0[i], &rhs.0[i]) {
                <T::Mask as IntLane>::ALL_ONES
            } else {
                <T::Mask as Lane>::ZERO
            }
        }))
    }

    /// `x == y`: all-ones lanes where equal, else zero.
    #[must_use]
    pub fn eq_mask(self, rhs: impl Into<Self>) -> Vec<N, T::Mask> {
        self.compare(rhs, |a, b| a == b)
    }

    /// `x != y`.
    #[must_use]
    pub fn ne_mask(self, rhs: impl Into<Self>) -> Vec<N, T::Mask> {
        self.compare(rhs, |a, b| a != b)
    }

    /// `x < y`.
    #[must_use]
    pub fn lt_mask(self, rhs: impl Into<Self>) -> Vec<N, T::Mask> {
        self.compare(rhs, |a, b| a < b)
    }

    /// `x <= y`.
    #[must_use]
    pub fn le_mask(self, rhs: impl Into<Self>) -> Vec<N, T::Mask> {
        self.compare(rhs, |a, b| a <= b)
    }

    /// `x > y`.
    #[must_use]
    pub fn gt_mask(self, rhs: impl Into<Self>) -> Vec<N, T::Mask> {
        self.compare(rhs, |a, b| a > b)
    }

    /// `x >= y`.
    #[must_use]
    pub fn ge_mask(self, rhs: impl Into<Self>) -> Vec<N, T::Mask> {
        self.compare(rhs, |a, b| a >= b)
    }

    /// `min(x, y)`, matching `std::min` lane-wise, which matters with NaN: `y < x ? y : x`
    /// (so a NaN `x` stays NaN and a NaN `y` yields `x`).
    #[must_use]
    // Port of: src/core/SkVx.h#L648-L654 (chrome/m156)
    pub fn min(self, y: impl Into<Self>) -> Self {
        let y = y.into();
        // naive_if_then_else(y < x, y, x)
        Self(array::from_fn(|i| {
            if y.0[i] < self.0[i] {
                y.0[i]
            } else {
                self.0[i]
            }
        }))
    }

    /// `max(x, y)`, matching `std::max` lane-wise: `x < y ? y : x`.
    #[must_use]
    // Port of: src/core/SkVx.h#L648-L654 (chrome/m156)
    pub fn max(self, y: impl Into<Self>) -> Self {
        let y = y.into();
        // naive_if_then_else(x < y, y, x)
        Self(array::from_fn(|i| {
            if self.0[i] < y.0[i] {
                y.0[i]
            } else {
                self.0[i]
            }
        }))
    }

    /// `pin(x, lo, hi)`, the logic of `SkTPin`: always within `lo..=hi`, and `lo` if `x` is NaN.
    #[must_use]
    // Port of: src/core/SkVx.h#L656-L660 (chrome/m156)
    pub fn pin(self, lo: impl Into<Self>, hi: impl Into<Self>) -> Self {
        lo.into().max(self.min(hi))
    }

    /// Lane-wise `!x`: all-ones where the lane is zero, else zero.
    #[must_use]
    // Port of: src/core/SkVx.h#L306 (chrome/m156)
    pub fn logical_not(self) -> Self {
        Self(array::from_fn(|i| {
            if self.0[i].is_nonzero() {
                T::ZERO
            } else {
                T::from_mask_bits(<T::Mask as IntLane>::ALL_ONES)
            }
        }))
    }
}

// Port of: src/core/SkVx.h#L123-L174 (chrome/m156)
impl<T: Lane> Vec<4, T> {
    /// `Vec(x, y, z, w)`.
    #[must_use]
    pub fn new(x: T, y: T, z: T, w: T) -> Self {
        Self([x, y, z, w])
    }

    /// `Vec(Vec<2,T> xy, T z, T w)`.
    #[must_use]
    pub fn from_xy_z_w(xy: Vec<2, T>, z: T, w: T) -> Self {
        Self([xy.0[0], xy.0[1], z, w])
    }

    /// `Vec(T x, T y, Vec<2,T> zw)`.
    #[must_use]
    pub fn from_x_y_zw(x: T, y: T, zw: Vec<2, T>) -> Self {
        Self([x, y, zw.0[0], zw.0[1]])
    }

    /// `Vec(Vec<2,T> xy, Vec<2,T> zw)`.
    #[must_use]
    pub fn from_xy_zw(xy: Vec<2, T>, zw: Vec<2, T>) -> Self {
        Self([xy.0[0], xy.0[1], zw.0[0], zw.0[1]])
    }

    /// `xy()` by value.
    #[must_use]
    pub fn xy(self) -> Vec<2, T> {
        Vec([self.0[0], self.0[1]])
    }

    /// `zw()` by value.
    #[must_use]
    pub fn zw(self) -> Vec<2, T> {
        Vec([self.0[2], self.0[3]])
    }

    /// `xy() = v`.
    pub fn set_xy(&mut self, v: Vec<2, T>) {
        self.0[0] = v.0[0];
        self.0[1] = v.0[1];
    }

    /// `zw() = v`.
    pub fn set_zw(&mut self, v: Vec<2, T>) {
        self.0[2] = v.0[0];
        self.0[3] = v.0[1];
    }

    /// The mutable `xy()` reference: the first two lanes.
    #[allow(clippy::missing_panics_doc)] // never panics: there are always 4 lanes
    pub fn xy_mut(&mut self) -> &mut [T; 2] {
        self.0.first_chunk_mut::<2>().expect("4 lanes")
    }

    /// The mutable `zw()` reference: the last two lanes.
    #[allow(clippy::missing_panics_doc)] // never panics: there are always 4 lanes
    pub fn zw_mut(&mut self) -> &mut [T; 2] {
        self.0.last_chunk_mut::<2>().expect("4 lanes")
    }

    /// `x()`.
    #[must_use]
    pub fn x(self) -> T {
        self.0[0]
    }
    /// `y()`.
    #[must_use]
    pub fn y(self) -> T {
        self.0[1]
    }
    /// `z()`.
    #[must_use]
    pub fn z(self) -> T {
        self.0[2]
    }
    /// `w()`.
    #[must_use]
    pub fn w(self) -> T {
        self.0[3]
    }
    /// Mutable `x()`.
    pub fn x_mut(&mut self) -> &mut T {
        &mut self.0[0]
    }
    /// Mutable `y()`.
    pub fn y_mut(&mut self) -> &mut T {
        &mut self.0[1]
    }
    /// Mutable `z()`.
    pub fn z_mut(&mut self) -> &mut T {
        &mut self.0[2]
    }
    /// Mutable `w()`.
    pub fn w_mut(&mut self) -> &mut T {
        &mut self.0[3]
    }

    /// `yxwz()`: `shuffle<1,0,3,2>`.
    #[must_use]
    pub fn yxwz(self) -> Self {
        Self([self.0[1], self.0[0], self.0[3], self.0[2]])
    }

    /// `zwxy()`: `shuffle<2,3,0,1>`.
    #[must_use]
    pub fn zwxy(self) -> Self {
        Self([self.0[2], self.0[3], self.0[0], self.0[1]])
    }
}

// Port of: src/core/SkVx.h#L176-L214 (chrome/m156)
impl<T: Lane> Vec<2, T> {
    /// `Vec(x, y)`.
    #[must_use]
    pub fn new(x: T, y: T) -> Self {
        Self([x, y])
    }
    /// `x()`.
    #[must_use]
    pub fn x(self) -> T {
        self.0[0]
    }
    /// `y()`.
    #[must_use]
    pub fn y(self) -> T {
        self.0[1]
    }
    /// Mutable `x()`.
    pub fn x_mut(&mut self) -> &mut T {
        &mut self.0[0]
    }
    /// Mutable `y()`.
    pub fn y_mut(&mut self) -> &mut T {
        &mut self.0[1]
    }
    /// `yx()`: `shuffle<1,0>`.
    #[must_use]
    pub fn yx(self) -> Self {
        Self([self.0[1], self.0[0]])
    }
    /// `xyxy()`.
    #[must_use]
    pub fn xyxy(self) -> Vec<4, T> {
        Vec([self.0[0], self.0[1], self.0[0], self.0[1]])
    }
}

// Port of: src/core/SkVx.h#L216-L236 (chrome/m156)
impl<T: Lane> Vec<1, T> {
    /// The `val` member.
    #[must_use]
    pub fn val(self) -> T {
        self.0[0]
    }
}

// Port of: src/core/SkVx.h#L244-L250 (chrome/m156)
/// Vectors that can be joined with another of the same size into one twice as large.
pub trait Join: Sized {
    /// `Vec<2N,T>`.
    type Output;
    /// Puts `self` in the low lanes and `hi` in the high lanes.
    fn join(self, hi: Self) -> Self::Output;
}

/// `join(lo, hi)`: two `Vec<N,T>` into one `Vec<2N,T>`. Implemented for `N` of 1, 2, 4, 8 and 16.
#[must_use]
// Port of: src/core/SkVx.h#L244-L250 (chrome/m156)
pub fn join<V: Join>(lo: V, hi: V) -> V::Output {
    lo.join(hi)
}

macro_rules! impl_halves {
    ($($h:literal => $n:literal),* $(,)?) => {$(
        impl<T: Lane> Join for Vec<$h, T> {
            type Output = Vec<$n, T>;
            fn join(self, hi: Self) -> Vec<$n, T> {
                Vec(array::from_fn(|i| if i < $h { self.0[i] } else { hi.0[i - $h] }))
            }
        }
        impl<T: Lane> Vec<$n, T> {
            /// The `lo` half (the first `N/2` lanes).
            #[must_use]
            pub fn lo(self) -> Vec<$h, T> {
                Vec(array::from_fn(|i| self.0[i]))
            }
            /// The `hi` half (the last `N/2` lanes).
            #[must_use]
            pub fn hi(self) -> Vec<$h, T> {
                Vec(array::from_fn(|i| self.0[i + $h]))
            }
        }
    )*};
}
impl_halves!(1 => 2, 2 => 4, 4 => 8, 8 => 16, 16 => 32);

// Operators. Port of: src/core/SkVx.h#L283-L470 (chrome/m156)

macro_rules! impl_binop {
    ($tr:ident, $m:ident, $assign_tr:ident, $assign_m:ident, $lane:ident, $bound:ident) => {
        impl<const N: usize, T: $bound> $tr for Vec<N, T> {
            type Output = Self;
            fn $m(self, rhs: Self) -> Self {
                Self(array::from_fn(|i| self.0[i].$lane(rhs.0[i])))
            }
        }
        impl<const N: usize, T: $bound> $tr<T> for Vec<N, T> {
            type Output = Self;
            fn $m(self, rhs: T) -> Self {
                Self(array::from_fn(|i| self.0[i].$lane(rhs)))
            }
        }
        impl<const N: usize, T: $bound> $assign_tr for Vec<N, T> {
            fn $assign_m(&mut self, rhs: Self) {
                *self = $tr::$m(*self, rhs);
            }
        }
        impl<const N: usize, T: $bound> $assign_tr<T> for Vec<N, T> {
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

impl<const N: usize, T: Lane> Neg for Vec<N, T> {
    type Output = Self;
    fn neg(self) -> Self {
        Self(array::from_fn(|i| self.0[i].lane_neg()))
    }
}

/// `~x`.
impl<const N: usize, T: IntLane> Not for Vec<N, T> {
    type Output = Self;
    fn not(self) -> Self {
        Self(array::from_fn(|i| self.0[i].lane_not()))
    }
}

impl<const N: usize, T: IntLane> Shl<i32> for Vec<N, T> {
    type Output = Self;
    fn shl(self, k: i32) -> Self {
        Self(array::from_fn(|i| self.0[i].lane_shl(k)))
    }
}

impl<const N: usize, T: IntLane> Shr<i32> for Vec<N, T> {
    type Output = Self;
    fn shr(self, k: i32) -> Self {
        Self(array::from_fn(|i| self.0[i].lane_shr(k)))
    }
}

impl<const N: usize, T: IntLane> ShlAssign<i32> for Vec<N, T> {
    fn shl_assign(&mut self, k: i32) {
        *self = *self << k;
    }
}

impl<const N: usize, T: IntLane> ShrAssign<i32> for Vec<N, T> {
    fn shr_assign(&mut self, k: i32) {
        *self = *self >> k;
    }
}

// `scalar op Vec` for each concrete lane type (a blanket impl would violate the orphan rules).
macro_rules! impl_scalar_lhs {
    ($t:ty: $($tr:ident $m:ident),*) => {$(
        impl<const N: usize> $tr<Vec<N, $t>> for $t {
            type Output = Vec<N, $t>;
            fn $m(self, rhs: Vec<N, $t>) -> Vec<N, $t> {
                $tr::$m(Vec::<N, $t>::splat(self), rhs)
            }
        }
    )*};
}
macro_rules! impl_scalar_lhs_float {
    ($($t:ty),*) => {$( impl_scalar_lhs!($t: Add add, Sub sub, Mul mul, Div div); )*};
}
macro_rules! impl_scalar_lhs_int {
    ($($t:ty),*) => {$(
        impl_scalar_lhs!($t: Add add, Sub sub, Mul mul, Div div, BitXor bitxor, BitAnd bitand, BitOr bitor);
    )*};
}
impl_scalar_lhs_float!(f32, f64);
impl_scalar_lhs_int!(i8, u8, i16, u16, i32, u32, i64, u64);
