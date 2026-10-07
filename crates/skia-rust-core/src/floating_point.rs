// Copyright 2006 The Android Open Source Project
// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/private/SkFloatingPoint.h, src/core/SkFloatingPoint.cpp

//! Floating point helpers (`SkFloatingPoint.h`).
//!
//! Float-to-int conversions go through the `*_saturate2int` helpers, never a bare `as`
//! (see docs/PORTING.md section 5).

use std::ops::{Mul, Neg, Sub};

use crate::math::{MAX_S32, MAX_S64, MIN_S32};

// Port of: include/private/SkFloatingPoint.h#L19-L21 (chrome/m156)
/// `sqrt(2)` as a float.
#[doc(alias = "SK_FloatSqrt2")]
#[allow(clippy::excessive_precision, clippy::approx_constant)] // Skia's literal, kept verbatim
pub const FLOAT_SQRT2: f32 = 1.414_213_56_f32;
/// Pi as a float.
#[doc(alias = "SK_FloatPI")]
#[allow(clippy::excessive_precision, clippy::approx_constant)] // Skia's literal, kept verbatim
pub const FLOAT_PI: f32 = 3.141_592_65_f32;
/// Pi as a double.
#[doc(alias = "SK_DoublePI")]
#[allow(clippy::excessive_precision, clippy::approx_constant)] // Skia's literal, kept verbatim
pub const DOUBLE_PI: f64 = 3.141_592_653_589_793_238_462_643_383_279_502_88_f64;

/// Returns -1, 0 or 1 according to the sign of `x`.
// Port of: include/private/SkFloatingPoint.h#L23-L25 (chrome/m156)
#[doc(alias = "sk_float_sgn")]
#[must_use]
pub fn float_sgn(x: f32) -> i32 {
    i32::from(0.0f32 < x) - i32::from(x < 0.0f32)
}

/// Converts degrees to radians.
// Port of: include/private/SkFloatingPoint.h#L27-L29 (chrome/m156)
#[doc(alias = "sk_float_degrees_to_radians")]
#[must_use]
pub fn float_degrees_to_radians(degrees: f32) -> f32 {
    degrees * (FLOAT_PI / 180.0)
}

/// Converts radians to degrees.
// Port of: include/private/SkFloatingPoint.h#L31-L33 (chrome/m156)
#[doc(alias = "sk_float_radians_to_degrees")]
#[must_use]
pub fn float_radians_to_degrees(radians: f32) -> f32 {
    radians * (180.0 / FLOAT_PI)
}

/// `floor(x + 0.5)` in double precision.
// Port of: include/private/SkFloatingPoint.h#L126 (chrome/m156)
#[doc(alias = "sk_double_round")]
#[must_use]
pub fn double_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// Rounds a float by upcasting to double: `0.49999997` and `2^24` round correctly,
/// which `floorf(x + .5f)` would not.
// Port of: include/private/SkFloatingPoint.h#L38 (chrome/m156)
#[doc(alias = "sk_float_round")]
#[allow(clippy::cast_possible_truncation)] // mirrors the (float) cast of the double result
#[must_use]
pub fn float_round(x: f32) -> f32 {
    double_round(f64::from(x)) as f32
}

/// Floating point types accepted by [`is_nan`] and [`is_finite`].
pub trait FloatingPoint:
    Copy + PartialEq + Sub<Output = Self> + Mul<Output = Self> + Neg<Output = Self>
{
    /// Positive infinity.
    const INFINITY: Self;
    /// The largest finite value.
    const MAX: Self;
    /// Zero.
    const ZERO: Self;
    /// `T(123)`-style conversion from a small integer.
    fn from_u8(x: u8) -> Self;
}

impl FloatingPoint for f32 {
    const INFINITY: Self = f32::INFINITY;
    const MAX: Self = f32::MAX;
    const ZERO: Self = 0.0;
    fn from_u8(x: u8) -> Self {
        f32::from(x)
    }
}

impl FloatingPoint for f64 {
    const INFINITY: Self = f64::INFINITY;
    const MAX: Self = f64::MAX;
    const ZERO: Self = 0.0;
    fn from_u8(x: u8) -> Self {
        f64::from(x)
    }
}

/// Returns true if `x` is NaN.
// Port of: include/private/SkFloatingPoint.h#L40-L43 (chrome/m156)
#[doc(alias = "SkIsNaN")]
#[allow(clippy::eq_op, clippy::float_cmp)] // x != x is the NaN test
#[must_use]
pub fn is_nan<T: FloatingPoint>(x: T) -> bool {
    x != x
}

/// Returns true if `x` is finite.
///
/// Subtracting a value from itself will result in zero, except for NAN or +-Inf, which make NAN.
// Port of: include/private/SkFloatingPoint.h#L49-L55 (chrome/m156)
#[doc(alias = "SkIsFinite")]
#[allow(clippy::eq_op, clippy::float_cmp)] // prod == prod is the NaN test
#[must_use]
pub fn is_finite<T: FloatingPoint>(x: T) -> bool {
    let prod = x - x;
    // At this point, `prod` will either be NaN or 0.
    prod == prod
}

/// Returns true if `x` and every one of `values` are finite (the variadic `SkIsFinite`).
///
/// Multiplying a group of values against zero will result in zero for each product, except for
/// NAN or +-Inf, which will result in NAN and continue resulting in NAN for the rest.
// Port of: include/private/SkFloatingPoint.h#L49-L55 (chrome/m156)
#[doc(alias = "SkIsFinite")]
#[allow(clippy::eq_op, clippy::float_cmp)] // prod == prod is the NaN test
#[must_use]
pub fn is_finite_all<T: FloatingPoint>(x: T, values: &[T]) -> bool {
    let mut prod = x - x;
    for &v in values {
        prod = prod * v;
    }
    // At this point, `prod` will either be NaN or 0.
    prod == prod
}

/// Returns true if every element of `array` is finite. `array` must not be empty.
// Port of: include/private/SkFloatingPoint.h#L57-L66 (chrome/m156)
#[doc(alias = "SkIsFinite")]
#[allow(clippy::eq_op, clippy::float_cmp)] // prod == prod is the NaN test
#[must_use]
pub fn is_finite_array<T: FloatingPoint>(array: &[T]) -> bool {
    let x = array[0];
    let mut prod = x - x;
    for &v in &array[1..] {
        prod = prod * v;
    }
    // At this point, `prod` will either be NaN or 0.
    prod == prod
}

// Port of: include/private/SkFloatingPoint.h#L68-L73 (chrome/m156)
/// Largest `i32` that is exactly representable as a float.
#[doc(alias = "SK_MaxS32FitsInFloat")]
pub const MAX_S32_FITS_IN_FLOAT: i32 = 2_147_483_520;
/// `-MAX_S32_FITS_IN_FLOAT`.
#[doc(alias = "SK_MinS32FitsInFloat")]
pub const MIN_S32_FITS_IN_FLOAT: i32 = -MAX_S32_FITS_IN_FLOAT;
/// Largest `i64` that is exactly representable as a float: `0x7fffff8000000000`.
#[doc(alias = "SK_MaxS64FitsInFloat")]
pub const MAX_S64_FITS_IN_FLOAT: i64 = MAX_S64 >> (63 - 24) << (63 - 24);
/// `-MAX_S64_FITS_IN_FLOAT`.
#[doc(alias = "SK_MinS64FitsInFloat")]
pub const MIN_S64_FITS_IN_FLOAT: i64 = -MAX_S64_FITS_IN_FLOAT;

/// Returns the closest int for the given float. Returns `MAX_S32_FITS_IN_FLOAT` for NaN.
// Port of: include/private/SkFloatingPoint.h#L89-L94 (chrome/m156)
#[doc(alias = "sk_float_saturate2int")]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
// the bounds are exactly representable floats; x is within i32 range at the truncating cast
#[must_use]
pub fn float_saturate2int(x: f32) -> i32 {
    let mut x = x;
    x = if x < MAX_S32_FITS_IN_FLOAT as f32 {
        x
    } else {
        MAX_S32_FITS_IN_FLOAT as f32
    };
    x = if x > MIN_S32_FITS_IN_FLOAT as f32 {
        x
    } else {
        MIN_S32_FITS_IN_FLOAT as f32
    };
    x as i32
}

/// Returns the closest int for the given double. Returns `MAX_S32` for NaN.
// Port of: include/private/SkFloatingPoint.h#L99-L104 (chrome/m156)
#[doc(alias = "sk_double_saturate2int")]
#[allow(clippy::cast_possible_truncation)] // x is within i32 range at the truncating cast
#[must_use]
pub fn double_saturate2int(x: f64) -> i32 {
    let mut x = x;
    x = if x < f64::from(MAX_S32) {
        x
    } else {
        f64::from(MAX_S32)
    };
    x = if x > f64::from(MIN_S32) {
        x
    } else {
        f64::from(MIN_S32)
    };
    x as i32
}

/// Returns the closest i64 for the given float. Returns `MAX_S64_FITS_IN_FLOAT` for NaN.
// Port of: include/private/SkFloatingPoint.h#L109-L114 (chrome/m156)
#[doc(alias = "sk_float_saturate2int64")]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
// the bounds are exactly representable floats; x is within i64 range at the truncating cast
#[must_use]
pub fn float_saturate2int64(x: f32) -> i64 {
    let mut x = x;
    x = if x < MAX_S64_FITS_IN_FLOAT as f32 {
        x
    } else {
        MAX_S64_FITS_IN_FLOAT as f32
    };
    x = if x > MIN_S64_FITS_IN_FLOAT as f32 {
        x
    } else {
        MIN_S64_FITS_IN_FLOAT as f32
    };
    x as i64
}

/// `sk_float_saturate2int(floor(x))`.
// Port of: include/private/SkFloatingPoint.h#L118 (chrome/m156)
#[doc(alias = "sk_float_floor2int")]
#[must_use]
pub fn float_floor2int(x: f32) -> i32 {
    float_saturate2int(x.floor())
}

/// `sk_float_saturate2int(sk_float_round(x))`.
// Port of: include/private/SkFloatingPoint.h#L119 (chrome/m156)
#[doc(alias = "sk_float_round2int")]
#[must_use]
pub fn float_round2int(x: f32) -> i32 {
    float_saturate2int(float_round(x))
}

/// `sk_float_saturate2int(ceil(x))`.
// Port of: include/private/SkFloatingPoint.h#L120 (chrome/m156)
#[doc(alias = "sk_float_ceil2int")]
#[must_use]
pub fn float_ceil2int(x: f32) -> i32 {
    float_saturate2int(x.ceil())
}

/// `(int)floor(x)`. Skia's cast is UB out of range; this saturates (Rust `as`).
// Port of: include/private/SkFloatingPoint.h#L122 (chrome/m156)
#[doc(alias = "sk_float_floor2int_no_saturate")]
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ (int) cast, which is UB out of range
#[must_use]
pub fn float_floor2int_no_saturate(x: f32) -> i32 {
    x.floor() as i32
}

/// `(int)sk_float_round(x)`. Skia's cast is UB out of range; this saturates (Rust `as`).
// Port of: include/private/SkFloatingPoint.h#L123 (chrome/m156)
#[doc(alias = "sk_float_round2int_no_saturate")]
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ (int) cast, which is UB out of range
#[must_use]
pub fn float_round2int_no_saturate(x: f32) -> i32 {
    float_round(x) as i32
}

/// `(int)ceil(x)`. Skia's cast is UB out of range; this saturates (Rust `as`).
// Port of: include/private/SkFloatingPoint.h#L124 (chrome/m156)
#[doc(alias = "sk_float_ceil2int_no_saturate")]
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ (int) cast, which is UB out of range
#[must_use]
pub fn float_ceil2int_no_saturate(x: f32) -> i32 {
    x.ceil() as i32
}

/// `(int)floor(x)` for doubles. Skia's cast is UB out of range; this saturates (Rust `as`).
// Port of: include/private/SkFloatingPoint.h#L127 (chrome/m156)
#[doc(alias = "sk_double_floor2int")]
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ (int) cast, which is UB out of range
#[must_use]
pub fn double_floor2int(x: f64) -> i32 {
    x.floor() as i32
}

/// `(int)round(x)` for doubles (halfway cases away from zero, as `std::round`).
// Port of: include/private/SkFloatingPoint.h#L128 (chrome/m156)
#[doc(alias = "sk_double_round2int")]
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ (int) cast, which is UB out of range
#[must_use]
pub fn double_round2int(x: f64) -> i32 {
    x.round() as i32
}

/// `(int)ceil(x)` for doubles. Skia's cast is UB out of range; this saturates (Rust `as`).
// Port of: include/private/SkFloatingPoint.h#L129 (chrome/m156)
#[doc(alias = "sk_double_ceil2int")]
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ (int) cast, which is UB out of range
#[must_use]
pub fn double_ceil2int(x: f64) -> i32 {
    x.ceil() as i32
}

/// Casts double to float; too-large finite values give the largest float or infinity.
// Port of: include/private/SkFloatingPoint.h#L135-L137 (chrome/m156)
#[doc(alias = "sk_double_to_float")]
#[allow(clippy::cast_possible_truncation)] // mirrors static_cast<float>(double)
#[must_use]
pub fn double_to_float(x: f64) -> f32 {
    x as f32
}

// Port of: include/private/SkFloatingPoint.h#L139-L143 (chrome/m156)
/// Quiet NaN as a float.
#[doc(alias = "SK_FloatNaN")]
pub const FLOAT_NAN: f32 = f32::NAN;
/// Positive infinity as a float.
#[doc(alias = "SK_FloatInfinity")]
pub const FLOAT_INFINITY: f32 = f32::INFINITY;
/// Negative infinity as a float.
#[doc(alias = "SK_FloatNegativeInfinity")]
pub const FLOAT_NEGATIVE_INFINITY: f32 = -FLOAT_INFINITY;
/// Quiet NaN as a double.
#[doc(alias = "SK_DoubleNaN")]
pub const DOUBLE_NAN: f64 = f64::NAN;

/// Calculates the midpoint between `a` and `b`, in double math to avoid underflow and overflow.
// Port of: include/private/SkFloatingPoint.h#L146-L149 (chrome/m156)
#[doc(alias = "sk_float_midpoint")]
#[allow(clippy::cast_possible_truncation, clippy::manual_midpoint)] // mirrors static_cast<float>; Skia's exact formula
#[must_use]
pub fn float_midpoint(a: f32, b: f32) -> f32 {
    (0.5 * (f64::from(a) + f64::from(b))) as f32
}

/// `1 / sqrt(x)`.
// Port of: include/private/SkFloatingPoint.h#L151 (chrome/m156)
#[doc(alias = "sk_float_rsqrt")]
#[must_use]
pub fn float_rsqrt(x: f32) -> f32 {
    1.0f32 / x.sqrt()
}

/// IEEE float divide: well defined for non-finite values and zero denominators.
// Port of: include/private/SkFloatingPoint.h#L160-L162 (chrome/m156)
#[doc(alias = "sk_ieee_float_divide")]
#[must_use]
pub fn ieee_float_divide(numer: f32, denom: f32) -> f32 {
    numer / denom
}

/// IEEE double divide: well defined for non-finite values and zero denominators.
// Port of: include/private/SkFloatingPoint.h#L165-L167 (chrome/m156)
#[doc(alias = "sk_ieee_double_divide")]
#[must_use]
pub fn ieee_double_divide(numer: f64, denom: f64) -> f64 {
    numer / denom
}

/// Returns the positive magnitude of a double.
/// * normalized - given 1.bbb...bbb x 2^e return 2^e.
/// * subnormal - return 0.
/// * nan & infinity - return infinity
// Port of: src/core/SkFloatingPoint.cpp#L19-L28 (chrome/m156)
#[allow(clippy::unusual_byte_groupings)] // sign | exponent | mantissa, as in the C++ literal
fn magnitude(a: f64) -> f64 {
    const EXTRACT_MAGNITUDE: u64 =
        0b0_11111111111_0000000000000000000000000000000000000000000000000000;
    let mut bits = a.to_bits();
    bits &= EXTRACT_MAGNITUDE;
    f64::from_bits(bits)
}

/// Compares two doubles and returns true if they are within `max_ulps_diff` ULPs of each other.
/// * nan as a or b - returns false.
/// * infinity, infinity or -infinity, -infinity - returns true.
/// * infinity and any other number - returns false.
///
/// ULP is an initialism for Units in the Last Place.
// Port of: src/core/SkFloatingPoint.cpp#L30-L52 (chrome/m156)
#[doc(alias = "sk_doubles_nearly_equal_ulps")]
#[allow(clippy::items_after_statements)] // constants stay next to their explanatory comments, as in Skia
#[must_use]
pub fn doubles_nearly_equal_ulps_max_diff(a: f64, b: f64, max_ulps_diff: u8) -> bool {
    // std::max(a, b) is `(a < b) ? b : a`.
    let std_max = |x: f64, y: f64| if x < y { y } else { x };
    // The maximum magnitude to construct the ulp tolerance. The proper magnitude for
    // subnormal numbers is min_magnitude, which is 2^-1021, so if a and b are subnormal (having a
    // magnitude of 0) use min_magnitude. If a or b are infinity or nan, then max_magnitude will be
    // +infinity. This means the tolerance will also be infinity, but the expression b - a below
    // will either be NaN or infinity, so a tolerance of infinity doesn't matter.
    const MIN_MAGNITUDE: f64 = f64::MIN_POSITIVE;
    let max_magnitude = std_max(std_max(magnitude(a), MIN_MAGNITUDE), magnitude(b));

    // Given a magnitude, this is the factor that generates the ulp for that magnitude.
    // In numbers, 2 ^ (-precision + 1) = 2 ^ -52.
    const ULP_FACTOR: f64 = f64::EPSILON;

    // The tolerance in ULPs given the max_magnitude. Because the return statement must use <
    // for comparison instead of <= to correctly handle infinities, bump max_ulps_diff up to get
    // the full max_ulps_diff range.
    let tolerance = max_magnitude * (ULP_FACTOR * f64::from(i32::from(max_ulps_diff) + 1));

    // The expression a == b is mainly for handling infinities, but it also catches the exact
    // equals.
    #[allow(clippy::float_cmp)] // exact equality catches infinities, as in Skia
    let exactly_equal = a == b;
    exactly_equal || (b - a).abs() < tolerance
}

/// [`doubles_nearly_equal_ulps_max_diff`] with Skia's default of 16 ULPs.
// Port of: include/private/SkFloatingPoint.h#L181 (chrome/m156)
#[doc(alias = "sk_doubles_nearly_equal_ulps")]
#[must_use]
pub fn doubles_nearly_equal_ulps(a: f64, b: f64) -> bool {
    doubles_nearly_equal_ulps_max_diff(a, b, 16)
}

/// Returns true iff the provided number is within a small epsilon of 0.
// Port of: src/core/SkFloatingPoint.cpp#L54-L56 (chrome/m156)
#[doc(alias = "sk_double_nearly_zero")]
#[allow(clippy::float_cmp)] // exact zero test, as in Skia
#[must_use]
pub fn double_nearly_zero(a: f64) -> bool {
    a == 0.0 || a.abs() < f64::from(f32::EPSILON)
}
