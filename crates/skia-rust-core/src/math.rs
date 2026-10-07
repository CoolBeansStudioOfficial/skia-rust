// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/private/SkMath.h, include/private/SkCPUTypes.h

//! Integer limits and small integer helpers (`SkMath.h`).

/// Port of `U8CPU`: an unsigned value known to fit in 8 bits, held in a CPU-friendly width.
// Port of: include/private/SkCPUTypes.h#L18 (chrome/m156)
pub type U8CPU = u32;

/// Port of `U16CPU`: an unsigned value known to fit in 16 bits, held in a CPU-friendly width.
// Port of: include/private/SkCPUTypes.h#L23 (chrome/m156)
pub type U16CPU = u32;

// Port of: include/private/SkMath.h#L18-L26 (chrome/m156)
/// Max signed 16 bit value.
#[doc(alias = "SK_MaxS16")]
pub const MAX_S16: i16 = i16::MAX;
/// `-MAX_S16`.
#[doc(alias = "SK_MinS16")]
pub const MIN_S16: i16 = -MAX_S16;
/// Max signed 32 bit value.
#[doc(alias = "SK_MaxS32")]
pub const MAX_S32: i32 = i32::MAX;
/// `-MAX_S32`.
#[doc(alias = "SK_MinS32")]
pub const MIN_S32: i32 = -MAX_S32;
/// `i32::MIN`, used as a "NaN" marker.
#[doc(alias = "SK_NaN32")]
pub const NAN32: i32 = i32::MIN;
/// Max signed 64 bit value.
#[doc(alias = "SK_MaxS64")]
pub const MAX_S64: i64 = i64::MAX;
/// `-MAX_S64`.
#[doc(alias = "SK_MinS64")]
pub const MIN_S64: i64 = -MAX_S64;

/// Multiplies two values already promoted to 64 bits.
// Port of: include/private/SkMath.h#L33-L35 (chrome/m156)
#[doc(alias = "sk_64_mul")]
#[must_use]
pub fn mul_64(a: i64, b: i64) -> i64 {
    a.wrapping_mul(b)
}

/// Shifts left through the unsigned type, so negative values do not trigger UB.
// Port of: include/private/SkMath.h#L37-L39 (chrome/m156)
#[doc(alias = "SkLeftShift")]
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // mirrors the (uint32_t)/(int32_t) casts
#[must_use]
pub const fn left_shift(value: i32, shift: i32) -> i32 {
    ((value as u32) << shift) as i32
}

/// 64-bit overload of [`left_shift`].
// Port of: include/private/SkMath.h#L41-L43 (chrome/m156)
#[doc(alias = "SkLeftShift")]
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // mirrors the (uint64_t)/(int64_t) casts
#[must_use]
pub const fn left_shift_64(value: i64, shift: i32) -> i64 {
    ((value as u64) << shift) as i64
}

/// Integer types accepted by [`is_pow2`].
pub trait PowerOfTwo: Copy {
    /// `(value & (value - 1)) == 0`, with the subtraction wrapping as in C++ unsigned math.
    fn is_pow2(self) -> bool;
}

macro_rules! impl_power_of_two {
    ($($t:ty),*) => {$(
        impl PowerOfTwo for $t {
            fn is_pow2(self) -> bool {
                (self & self.wrapping_sub(1)) == 0
            }
        }
    )*};
}
impl_power_of_two!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

/// Returns true if `value` is a power of 2. Does not explicitly check for `value <= 0`.
// Port of: include/private/SkMath.h#L51-L53 (chrome/m156)
#[doc(alias = "SkIsPow2")]
#[must_use]
pub fn is_pow2<T: PowerOfTwo>(value: T) -> bool {
    value.is_pow2()
}

/// Returns `a*b/((1 << shift) - 1)`, rounding any fractional bits.
/// Only valid if `a` and `b` are unsigned and `<= 32767` and `shift` is `> 0` and `<= 8`.
// Port of: include/private/SkMath.h#L61-L67 (chrome/m156)
#[doc(alias = "SkMul16ShiftRound")]
#[must_use]
pub fn mul_16_shift_round(a: U16CPU, b: U16CPU, shift: i32) -> u32 {
    debug_assert!(a <= 32767);
    debug_assert!(b <= 32767);
    debug_assert!(shift > 0 && shift <= 8);
    let prod = a * b + (1 << (shift - 1));
    (prod + (prod >> shift)) >> shift
}

/// Returns `a*b/255`, rounding any fractional bits. Only valid if `a` and `b` are `<= 32767`.
// Port of: include/private/SkMath.h#L73-L75 (chrome/m156)
#[doc(alias = "SkMulDiv255Round")]
#[must_use]
pub fn mul_div_255_round(a: U16CPU, b: U16CPU) -> U8CPU {
    mul_16_shift_round(a, b, 8)
}
