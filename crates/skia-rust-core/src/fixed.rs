// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/private/SkFixed.h

//! Types and functions for 16.16 fixed point (`SkFixed.h`).
//!
//! The ARM `VFPv3` `SkFloatToFixed_arm` override is not ported; the portable version is used.

use crate::floating_point::{float_saturate2int, float_saturate2int64};
use crate::math::{MAX_S32, MAX_S64, MIN_S32, left_shift_64};
use crate::scalar::scalar;
use crate::t_pin::t_pin;
use crate::to::to_s32;

/// 32 bit signed integer used to represent fractions values with 16 bits to the right of the
/// decimal point.
// Port of: include/private/SkFixed.h#L25 (chrome/m156)
#[doc(alias = "SkFixed")]
pub type Fixed = i32;

// Port of: include/private/SkFixed.h#L26-L34 (chrome/m156)
/// `1.0` in 16.16.
#[doc(alias = "SK_Fixed1")]
pub const FIXED_1: Fixed = 1 << 16;
/// `0.5` in 16.16.
#[doc(alias = "SK_FixedHalf")]
pub const FIXED_HALF: Fixed = 1 << 15;
/// `0.25` in 16.16.
#[doc(alias = "SK_FixedQuarter")]
pub const FIXED_QUARTER: Fixed = 1 << 14;
/// Largest fixed value.
#[doc(alias = "SK_FixedMax")]
pub const FIXED_MAX: Fixed = 0x7FFF_FFFF;
/// `-FIXED_MAX`.
#[doc(alias = "SK_FixedMin")]
pub const FIXED_MIN: Fixed = -FIXED_MAX;
/// Pi in 16.16.
#[doc(alias = "SK_FixedPI")]
pub const FIXED_PI: Fixed = 0x3243F;
/// `sqrt(2)` in 16.16.
#[doc(alias = "SK_FixedSqrt2")]
pub const FIXED_SQRT2: Fixed = 92682;
/// `tan(pi / 8)` in 16.16.
#[doc(alias = "SK_FixedTanPIOver8")]
pub const FIXED_TAN_PI_OVER_8: Fixed = 0x6A0A;
/// `sqrt(2) / 2` in 16.16.
#[doc(alias = "SK_FixedRoot2Over2")]
pub const FIXED_ROOT_2_OVER_2: Fixed = 0xB505;

/// Fixed to float. Exact.
// Port of: include/private/SkFixed.h#L41 (chrome/m156)
#[doc(alias = "SkFixedToFloat")]
#[allow(clippy::cast_precision_loss, clippy::excessive_precision)] // implicit int -> float conversion; Skia's literal kept verbatim
#[must_use]
pub fn fixed_to_float(x: Fixed) -> f32 {
    x as f32 * 1.525_878_906_25e-5_f32
}

/// Float to fixed, saturating.
// Port of: include/private/SkFixed.h#L42 (chrome/m156)
#[doc(alias = "SkFloatToFixed")]
#[allow(clippy::cast_precision_loss)] // mirrors the implicit int -> float conversion
#[must_use]
pub fn float_to_fixed(x: f32) -> Fixed {
    float_saturate2int(x * FIXED_1 as f32)
}

/// Float to fixed. In debug builds asserts that the result does not overflow;
/// otherwise the same as [`float_to_fixed`].
// Port of: include/private/SkFixed.h#L44-L52 (chrome/m156)
#[doc(alias = "SkFloatToFixed_Check")]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)] // mirrors the C++ casts
#[must_use]
pub fn float_to_fixed_check(x: f32) -> Fixed {
    if cfg!(debug_assertions) {
        let n64 = (x * FIXED_1 as f32) as i64;
        let n32 = n64 as Fixed;
        debug_assert_eq!(n64, i64::from(n32));
        n32
    } else {
        float_to_fixed(x)
    }
}

/// Fixed to double.
// Port of: include/private/SkFixed.h#L55 (chrome/m156)
#[doc(alias = "SkFixedToDouble")]
#[must_use]
#[allow(clippy::excessive_precision)] // Skia's literal, kept verbatim
pub fn fixed_to_double(x: Fixed) -> f64 {
    f64::from(x) * 1.525_878_906_25e-5_f64
}

/// Double to fixed. Skia's `(SkFixed)` cast is UB out of range; this saturates (Rust `as`).
// Port of: include/private/SkFixed.h#L56 (chrome/m156)
#[doc(alias = "SkDoubleToFixed")]
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ (SkFixed) cast
#[must_use]
pub fn double_to_fixed(x: f64) -> Fixed {
    (x * f64::from(FIXED_1)) as Fixed
}

/// Converts an integer to a fixed, asserting (in debug builds) that it does not overflow.
// Port of: include/private/SkFixed.h#L62-L77 (chrome/m156)
#[doc(alias = "SkIntToFixed")]
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // mirrors the (unsigned)/(SkFixed) casts
#[must_use]
pub fn int_to_fixed(n: i32) -> Fixed {
    debug_assert!((-32768..=32767).contains(&n));
    // Left shifting a negative value has undefined behavior in C, so we cast to unsigned before
    // shifting.
    ((n as u32) << 16) as Fixed
}

/// Rounds to the nearest integer.
// Port of: include/private/SkFixed.h#L76 (chrome/m156)
#[doc(alias = "SkFixedRoundToInt")]
#[must_use]
pub fn fixed_round_to_int(x: Fixed) -> i32 {
    x.wrapping_add(FIXED_HALF) >> 16
}

/// Rounds up to an integer.
// Port of: include/private/SkFixed.h#L77 (chrome/m156)
#[doc(alias = "SkFixedCeilToInt")]
#[must_use]
pub fn fixed_ceil_to_int(x: Fixed) -> i32 {
    x.wrapping_add(FIXED_1).wrapping_sub(1) >> 16
}

/// Rounds down to an integer.
// Port of: include/private/SkFixed.h#L78 (chrome/m156)
#[doc(alias = "SkFixedFloorToInt")]
#[must_use]
pub fn fixed_floor_to_int(x: Fixed) -> i32 {
    x >> 16
}

/// Rounds to the nearest whole fixed.
// Port of: include/private/SkFixed.h#L80-L82 (chrome/m156)
#[doc(alias = "SkFixedRoundToFixed")]
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // mirrors the (uint32_t)/(SkFixed) casts
#[must_use]
pub fn fixed_round_to_fixed(x: Fixed) -> Fixed {
    (x.wrapping_add(FIXED_HALF) as u32 & 0xFFFF_0000) as Fixed
}

/// Rounds up to a whole fixed.
// Port of: include/private/SkFixed.h#L83-L85 (chrome/m156)
#[doc(alias = "SkFixedCeilToFixed")]
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // mirrors the (uint32_t)/(SkFixed) casts
#[must_use]
pub fn fixed_ceil_to_fixed(x: Fixed) -> Fixed {
    (x.wrapping_add(FIXED_1).wrapping_sub(1) as u32 & 0xFFFF_0000) as Fixed
}

/// Rounds down to a whole fixed.
// Port of: include/private/SkFixed.h#L86-L88 (chrome/m156)
#[doc(alias = "SkFixedFloorToFixed")]
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // mirrors the (uint32_t)/(SkFixed) casts
#[must_use]
pub fn fixed_floor_to_fixed(x: Fixed) -> Fixed {
    (x as u32 & 0xFFFF_0000) as Fixed
}

/// `(a + b) >> 1`.
// Port of: include/private/SkFixed.h#L90 (chrome/m156)
#[doc(alias = "SkFixedAve")]
#[must_use]
pub fn fixed_ave(a: Fixed, b: Fixed) -> Fixed {
    a.wrapping_add(b) >> 1
}

/// Fixed divide. The divide may exceed 32 bits; the result is clamped to a signed 32 bit value.
// Port of: include/private/SkFixed.h#L93-L94 (chrome/m156)
#[doc(alias = "SkFixedDiv")]
#[must_use]
pub fn fixed_div(numer: i32, denom: i32) -> Fixed {
    to_s32(t_pin::<i64>(
        left_shift_64(i64::from(numer), 16) / i64::from(denom),
        i64::from(MIN_S32),
        i64::from(MAX_S32),
    ))
}

/// Fixed multiply.
// Port of: include/private/SkFixed.h#L96-L98 (chrome/m156)
#[doc(alias = "SkFixedMul")]
#[allow(clippy::cast_possible_truncation)] // mirrors the (SkFixed) cast of the 64-bit product
#[must_use]
pub fn fixed_mul(a: Fixed, b: Fixed) -> Fixed {
    (i64::from(a).wrapping_mul(i64::from(b)) >> 16) as Fixed
}

/// Fixed to scalar.
// Port of: include/private/SkFixed.h#L124 (chrome/m156)
#[doc(alias = "SkFixedToScalar")]
#[must_use]
pub fn fixed_to_scalar(x: Fixed) -> scalar {
    fixed_to_float(x)
}

/// Scalar to fixed.
// Port of: include/private/SkFixed.h#L125 (chrome/m156)
#[doc(alias = "SkScalarToFixed")]
#[must_use]
pub fn scalar_to_fixed(x: scalar) -> Fixed {
    float_to_fixed(x)
}

/// 32.32 fixed point.
// Port of: include/private/SkFixed.h#L129 (chrome/m156)
#[doc(alias = "SkFixed3232")]
pub type Fixed3232 = i64;

/// Largest 32.32 value.
#[doc(alias = "SkFixed3232Max")]
pub const FIXED_3232_MAX: Fixed3232 = MAX_S64;
/// `-FIXED_3232_MAX`.
#[doc(alias = "SkFixed3232Min")]
pub const FIXED_3232_MIN: Fixed3232 = -FIXED_3232_MAX;

/// Integer to 32.32.
// Port of: include/private/SkFixed.h#L134 (chrome/m156)
#[doc(alias = "SkIntToFixed3232")]
#[must_use]
pub fn int_to_fixed3232(x: i32) -> Fixed3232 {
    left_shift_64(Fixed3232::from(x), 32)
}

/// 32.32 to integer.
// Port of: include/private/SkFixed.h#L135 (chrome/m156)
#[doc(alias = "SkFixed3232ToInt")]
#[allow(clippy::cast_possible_truncation)] // mirrors the (int) cast
#[must_use]
pub fn fixed3232_to_int(x: Fixed3232) -> i32 {
    (x >> 32) as i32
}

/// 16.16 to 32.32.
// Port of: include/private/SkFixed.h#L136 (chrome/m156)
#[doc(alias = "SkFixedToFixed3232")]
#[must_use]
pub fn fixed_to_fixed3232(x: Fixed) -> Fixed3232 {
    left_shift_64(Fixed3232::from(x), 16)
}

/// 32.32 to 16.16.
// Port of: include/private/SkFixed.h#L137 (chrome/m156)
#[doc(alias = "SkFixed3232ToFixed")]
#[allow(clippy::cast_possible_truncation)] // mirrors the (SkFixed) cast
#[must_use]
pub fn fixed3232_to_fixed(x: Fixed3232) -> Fixed {
    (x >> 16) as Fixed
}

/// Float to 32.32, saturating.
// Port of: include/private/SkFixed.h#L138 (chrome/m156)
#[doc(alias = "SkFloatToFixed3232")]
#[must_use]
pub fn float_to_fixed3232(x: f32) -> Fixed3232 {
    float_saturate2int64(x * (65536.0f32 * 65536.0f32))
}

/// 32.32 to float.
// Port of: include/private/SkFixed.h#L139 (chrome/m156)
#[doc(alias = "SkFixed3232ToFloat")]
#[allow(clippy::cast_precision_loss)] // mirrors the implicit int64 -> float conversion
#[must_use]
pub fn fixed3232_to_float(x: Fixed3232) -> f32 {
    x as f32 * (1.0f32 / (65536.0f32 * 65536.0f32))
}

/// Scalar to 32.32.
// Port of: include/private/SkFixed.h#L141 (chrome/m156)
#[doc(alias = "SkScalarToFixed3232")]
#[must_use]
pub fn scalar_to_fixed3232(x: scalar) -> Fixed3232 {
    float_to_fixed3232(x)
}
