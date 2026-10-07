// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkVx.h

//! The lane element types a [`Vec`](super::Vec) can hold, and their scalar semantics.

use core::fmt::Debug;

mod sealed {
    pub trait Sealed {}
}

/// A scalar type usable as a lane of a [`Vec`](super::Vec): `f32`, `f64`, `i8`, `u8`, `i16`,
/// `u16`, `i32`, `u32`, `i64` or `u64`. Sealed.
///
/// The `lane_*` methods are the scalar operations the C++ compiler would emit for the
/// Clang/GCC vector extensions: IEEE for floats, two's-complement wrapping for integers.
pub trait Lane: sealed::Sealed + Copy + Debug + PartialEq + PartialOrd + 'static {
    /// The result of a comparison, `skvx::M<T>`: `i32` for `f32`, `i64` for `f64`, and `Self`
    /// for integers (so unsigned lanes produce all-ones masks of the unsigned type).
    #[doc(alias = "M")]
    type Mask: IntLane;

    /// The zero value of the lane type.
    const ZERO: Self;

    /// `x + y` (wrapping for integers).
    #[must_use]
    fn lane_add(self, rhs: Self) -> Self;
    /// `x - y` (wrapping for integers).
    #[must_use]
    fn lane_sub(self, rhs: Self) -> Self;
    /// `x * y` (wrapping for integers).
    #[must_use]
    fn lane_mul(self, rhs: Self) -> Self;
    /// `x / y` (integer division by zero panics; it is undefined behaviour in C++).
    #[must_use]
    fn lane_div(self, rhs: Self) -> Self;
    /// `-x` (wrapping for integers, including unsigned ones).
    #[must_use]
    fn lane_neg(self) -> Self;

    /// `sk_bit_cast<M<T>>(x)`.
    fn to_mask_bits(self) -> Self::Mask;
    /// `sk_bit_cast<T>(m)`.
    #[must_use]
    fn from_mask_bits(bits: Self::Mask) -> Self;

    /// `x != 0`: the truth value `any()` and `all()` give a lane.
    fn is_nonzero(self) -> bool {
        self != Self::ZERO
    }

    /// One lane of `isfinite`: always true for integers, exponent bits not all ones for floats.
    fn is_finite_lane(self) -> bool;

    /// Byte `i` of the lane's native-endian representation (`i < size_of::<Self>()`).
    fn ne_byte(self, i: usize) -> u8;
    /// The lane whose native-endian byte `i` is `f(i)`.
    fn from_ne_byte_fn(f: impl FnMut(usize) -> u8) -> Self;
}

/// Integer lanes (and the mask types of float lanes): adds the bitwise operations.
pub trait IntLane: Lane<Mask = Self> {
    /// All bits set (`~0`).
    const ALL_ONES: Self;
    /// The largest value of the type (`std::numeric_limits<T>::max()`).
    const MAX: Self;

    /// `x & y`.
    #[must_use]
    fn lane_and(self, rhs: Self) -> Self;
    /// `x | y`.
    #[must_use]
    fn lane_or(self, rhs: Self) -> Self;
    /// `x ^ y`.
    #[must_use]
    fn lane_xor(self, rhs: Self) -> Self;
    /// `~x`.
    #[must_use]
    fn lane_not(self) -> Self;
    /// `x << k`; the shift amount is taken modulo the bit width (undefined in C++ when out of range).
    #[must_use]
    fn lane_shl(self, k: i32) -> Self;
    /// `x >> k`, arithmetic for signed and logical for unsigned lanes; see [`IntLane::lane_shl`].
    #[must_use]
    fn lane_shr(self, k: i32) -> Self;
}

/// Unsigned integer lanes (`std::is_unsigned_v<T>`).
pub trait UnsignedLane: IntLane {}

/// Lanes with a double-width unsigned counterpart, for [`mull`](super::mull): `u8` and `u16`.
pub trait MulWiden: UnsignedLane {
    /// `u16` for `u8`, `u32` for `u16`.
    type Wide: UnsignedLane + CastFrom<Self>;
}

impl MulWiden for u8 {
    type Wide = u16;
}
impl MulWiden for u16 {
    type Wide = u32;
}

/// Floating point lanes: `f32` and `f64`.
pub trait FloatLane: Lane {
    /// `floorf`.
    #[must_use]
    fn lane_floor(self) -> Self;
    /// `ceilf`.
    #[must_use]
    fn lane_ceil(self) -> Self;
    /// `truncf`.
    #[must_use]
    fn lane_trunc(self) -> Self;
    /// `roundf` (half away from zero).
    #[must_use]
    fn lane_round(self) -> Self;
    /// `sqrtf`.
    #[must_use]
    fn lane_sqrt(self) -> Self;
    /// `fabsf`.
    #[must_use]
    fn lane_abs(self) -> Self;
    /// `fmaf`: a single rounding.
    #[must_use]
    fn lane_fma(self, y: Self, z: Self) -> Self;
}

/// `(D)s`: the C cast of one lane type to another, as applied lane-wise by [`cast`](super::Vec::cast).
///
/// Float to integer conversions use Rust's saturating `as` (NaN becomes 0); the C++ cast is
/// undefined for out-of-range values.
pub trait CastFrom<S>: sealed::Sealed {
    /// Converts `s` as a C cast would.
    fn cast_from(s: S) -> Self;
}

macro_rules! impl_int_lane {
    ($($t:ty),* $(,)?) => {$(
        impl sealed::Sealed for $t {}
        impl Lane for $t {
            type Mask = $t;
            const ZERO: Self = 0;
            #[inline(always)]
            fn lane_add(self, rhs: Self) -> Self { self.wrapping_add(rhs) }
            #[inline(always)]
            fn lane_sub(self, rhs: Self) -> Self { self.wrapping_sub(rhs) }
            #[inline(always)]
            fn lane_mul(self, rhs: Self) -> Self { self.wrapping_mul(rhs) }
            #[inline(always)]
            fn lane_div(self, rhs: Self) -> Self { self.wrapping_div(rhs) }
            #[inline(always)]
            fn lane_neg(self) -> Self { self.wrapping_neg() }
            #[inline(always)]
            fn to_mask_bits(self) -> $t { self }
            #[inline(always)]
            fn from_mask_bits(bits: $t) -> Self { bits }
            #[inline(always)]
            fn is_finite_lane(self) -> bool { true }
            #[inline(always)]
            fn ne_byte(self, i: usize) -> u8 { self.to_ne_bytes()[i] }
            #[inline(always)]
            fn from_ne_byte_fn(f: impl FnMut(usize) -> u8) -> Self {
                Self::from_ne_bytes(core::array::from_fn(f))
            }
        }
        impl IntLane for $t {
            const ALL_ONES: Self = !0;
            const MAX: Self = <$t>::MAX;
            #[inline(always)]
            fn lane_and(self, rhs: Self) -> Self { self & rhs }
            #[inline(always)]
            fn lane_or(self, rhs: Self) -> Self { self | rhs }
            #[inline(always)]
            fn lane_xor(self, rhs: Self) -> Self { self ^ rhs }
            #[inline(always)]
            fn lane_not(self) -> Self { !self }
            #[inline(always)]
            fn lane_shl(self, k: i32) -> Self { self.wrapping_shl(k.cast_unsigned()) }
            #[inline(always)]
            fn lane_shr(self, k: i32) -> Self { self.wrapping_shr(k.cast_unsigned()) }
        }
    )*};
}
impl_int_lane!(i8, u8, i16, u16, i32, u32, i64, u64);

impl UnsignedLane for u8 {}
impl UnsignedLane for u16 {}
impl UnsignedLane for u32 {}
impl UnsignedLane for u64 {}

macro_rules! impl_float_lane {
    ($t:ty, $mask:ty, $exp:expr) => {
        impl sealed::Sealed for $t {}
        impl Lane for $t {
            type Mask = $mask;
            const ZERO: Self = 0.0;
            #[inline(always)]
            fn lane_add(self, rhs: Self) -> Self {
                self + rhs
            }
            #[inline(always)]
            fn lane_sub(self, rhs: Self) -> Self {
                self - rhs
            }
            #[inline(always)]
            fn lane_mul(self, rhs: Self) -> Self {
                self * rhs
            }
            #[inline(always)]
            fn lane_div(self, rhs: Self) -> Self {
                self / rhs
            }
            #[inline(always)]
            fn lane_neg(self) -> Self {
                -self
            }
            #[inline(always)]
            fn to_mask_bits(self) -> $mask {
                self.to_bits().cast_signed()
            }
            #[inline(always)]
            fn from_mask_bits(bits: $mask) -> Self {
                Self::from_bits(bits.cast_unsigned())
            }
            #[inline(always)]
            fn is_finite_lane(self) -> bool {
                let bits = self.to_bits().cast_signed();
                (bits & $exp) != $exp
            }
            #[inline(always)]
            fn ne_byte(self, i: usize) -> u8 {
                self.to_ne_bytes()[i]
            }
            #[inline(always)]
            fn from_ne_byte_fn(f: impl FnMut(usize) -> u8) -> Self {
                Self::from_ne_bytes(core::array::from_fn(f))
            }
        }
        impl FloatLane for $t {
            #[inline(always)]
            fn lane_floor(self) -> Self {
                self.floor()
            }
            #[inline(always)]
            fn lane_ceil(self) -> Self {
                self.ceil()
            }
            #[inline(always)]
            fn lane_trunc(self) -> Self {
                self.trunc()
            }
            #[inline(always)]
            fn lane_round(self) -> Self {
                self.round()
            }
            #[inline(always)]
            fn lane_sqrt(self) -> Self {
                self.sqrt()
            }
            #[inline(always)]
            fn lane_abs(self) -> Self {
                self.abs()
            }
            #[inline(always)]
            fn lane_fma(self, y: Self, z: Self) -> Self {
                self.mul_add(y, z)
            }
        }
    };
}
impl_float_lane!(f32, i32, 0x7f80_0000_i32);
impl_float_lane!(f64, i64, 0x7ff0_0000_0000_0000_i64);

macro_rules! impl_cast_from {
    ($d:ty; $($s:ty),*) => {$(
        impl CastFrom<$s> for $d {
            // Mirrors the C-style lane cast of SkVx's cast(); the identity and widening casts
            // are the diagonal and lossless entries of the same table.
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_possible_wrap,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss,
                clippy::cast_lossless,
                clippy::unnecessary_cast
            )]
            #[inline(always)]
            fn cast_from(s: $s) -> Self { s as $d }
        }
    )*};
}
macro_rules! impl_cast_all {
    ($($d:ty),*) => {$( impl_cast_from!($d; f32, f64, i8, u8, i16, u16, i32, u32, i64, u64); )*};
}
impl_cast_all!(f32, f64, i8, u8, i16, u16, i32, u32, i64, u64);
