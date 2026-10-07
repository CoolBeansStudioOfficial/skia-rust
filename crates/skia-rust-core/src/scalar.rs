// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkScalar.h

//! The `scalar` type and its helpers (`SkScalar.h`). Names follow `skia-safe`'s `scalar_.rs`.

use crate::floating_point::{
    FLOAT_INFINITY, FLOAT_NAN, FLOAT_NEGATIVE_INFINITY, FLOAT_PI, FLOAT_SQRT2, double_to_float,
    float_ceil2int, float_floor2int, float_midpoint, float_round, float_round2int,
    float_saturate2int, is_finite_array,
};

/// Skia's `SkScalar`: a float.
// Port of: include/core/SkScalar.h#L16 (chrome/m156)
#[doc(alias = "SkScalar")]
#[allow(non_camel_case_types)]
pub type scalar = f32;

// Port of: include/core/SkScalar.h#L18-L28 (chrome/m156)
/// `1.0`.
#[doc(alias = "SK_Scalar1")]
pub const SCALAR_1: scalar = 1.0;
/// `0.5`.
#[doc(alias = "SK_ScalarHalf")]
pub const SCALAR_HALF: scalar = 0.5;
/// `sqrt(2)`.
#[doc(alias = "SK_ScalarSqrt2")]
pub const SCALAR_SQRT2: scalar = FLOAT_SQRT2;
/// Pi.
#[doc(alias = "SK_ScalarPI")]
pub const SCALAR_PI: scalar = FLOAT_PI;
/// `tan(pi / 8)`.
#[doc(alias = "SK_ScalarTanPIOver8")]
#[allow(clippy::excessive_precision)] // Skia's literal, kept verbatim
pub const SCALAR_TAN_PI_OVER_8: scalar = 0.414_213_562_f32;
/// `sqrt(2) / 2`.
#[doc(alias = "SK_ScalarRoot2Over2")]
#[allow(clippy::excessive_precision, clippy::approx_constant)] // Skia's literal, kept verbatim
pub const SCALAR_ROOT_2_OVER_2: scalar = 0.707_106_781_f32;
/// Largest finite scalar.
#[doc(alias = "SK_ScalarMax")]
#[allow(clippy::excessive_precision)] // Skia's literal, kept verbatim
pub const SCALAR_MAX: scalar = 3.402_823_466e+38_f32;
/// `-SCALAR_MAX`.
#[doc(alias = "SK_ScalarMin")]
pub const SCALAR_MIN: scalar = -SCALAR_MAX;
/// Positive infinity.
#[doc(alias = "SK_ScalarInfinity")]
pub const SCALAR_INFINITY: scalar = FLOAT_INFINITY;
/// Negative infinity.
#[doc(alias = "SK_ScalarNegativeInfinity")]
pub const SCALAR_NEGATIVE_INFINITY: scalar = FLOAT_NEGATIVE_INFINITY;
/// NaN.
#[doc(alias = "SK_ScalarNaN")]
pub const SCALAR_NAN: scalar = FLOAT_NAN;

/// Port of `SK_ScalarNearlyZero`: `1 / (1 << 12)`.
// Port of: include/core/SkScalar.h#L98 (chrome/m156)
#[doc(alias = "SK_ScalarNearlyZero")]
#[allow(clippy::cast_precision_loss)] // 1 << 12 is exact
pub const SCALAR_NEARLY_ZERO: scalar = SCALAR_1 / ((1 << 12) as scalar);

/// Port of `SK_ScalarSinCosNearlyZero`: `1 / (1 << 16)`.
// Port of: include/core/SkScalar.h#L112 (chrome/m156)
#[doc(alias = "SK_ScalarSinCosNearlyZero")]
#[allow(clippy::cast_precision_loss)] // 1 << 16 is exact
pub const SCALAR_SIN_COS_NEARLY_ZERO: scalar = SCALAR_1 / ((1 << 16) as scalar);

/// Scalar constants and tolerant comparisons, as in `skia-safe`.
pub trait Scalar: Copy {
    /// `0.0`.
    const ZERO: Self;
    /// `SK_ScalarNearlyZero`.
    const NEARLY_ZERO: Self;
    /// `1.0`.
    const ONE: Self;
    /// `0.5`.
    const HALF: Self;

    /// `SkScalarNearlyEqual(x, y, tolerance)`; `None` uses `NEARLY_ZERO`.
    fn nearly_equal(x: scalar, y: scalar, tolerance: impl Into<Option<scalar>>) -> bool;
    /// `SkScalarNearlyZero(self, tolerance)`; `None` uses `NEARLY_ZERO`.
    fn nearly_zero(&self, tolerance: impl Into<Option<scalar>>) -> bool;
}

impl Scalar for scalar {
    const ZERO: Self = 0.0;
    const NEARLY_ZERO: Self = SCALAR_NEARLY_ZERO;
    const ONE: Self = SCALAR_1;
    const HALF: Self = SCALAR_HALF;

    // Port of: include/core/SkScalar.h#L106-L110 (chrome/m156)
    #[doc(alias = "SkScalarNearlyEqual")]
    fn nearly_equal(x: scalar, y: scalar, tolerance: impl Into<Option<scalar>>) -> bool {
        let tolerance = tolerance.into().unwrap_or(Self::NEARLY_ZERO);
        debug_assert!(tolerance >= 0.0);
        (x - y).abs() <= tolerance
    }

    // Port of: include/core/SkScalar.h#L100-L104 (chrome/m156)
    #[doc(alias = "SkScalarNearlyZero")]
    fn nearly_zero(&self, tolerance: impl Into<Option<scalar>>) -> bool {
        let tolerance = tolerance.into().unwrap_or(Self::NEARLY_ZERO);
        debug_assert!(tolerance >= 0.0);
        self.abs() <= tolerance
    }
}

/// Operations on slices of scalars.
pub trait Scalars {
    /// True if every element is finite (`SkIsFinite(array, count)`). An empty slice is finite.
    fn are_finite(&self) -> bool;
}

impl Scalars for [scalar] {
    // Port of: include/private/SkFloatingPoint.h#L57-L66 (chrome/m156)
    #[doc(alias = "SkIsFinite")]
    fn are_finite(&self) -> bool {
        self.is_empty() || is_finite_array(self)
    }
}

/// Floor as a scalar.
// Port of: include/core/SkScalar.h#L30 (chrome/m156)
#[doc(alias = "SkScalarFloorToScalar")]
#[must_use]
pub fn scalar_floor_to_scalar(x: scalar) -> scalar {
    x.floor()
}

/// Ceil as a scalar.
// Port of: include/core/SkScalar.h#L31 (chrome/m156)
#[doc(alias = "SkScalarCeilToScalar")]
#[must_use]
pub fn scalar_ceil_to_scalar(x: scalar) -> scalar {
    x.ceil()
}

/// Round as a scalar (`sk_float_round`).
// Port of: include/core/SkScalar.h#L32 (chrome/m156)
#[doc(alias = "SkScalarRoundToScalar")]
#[must_use]
pub fn scalar_round_to_scalar(x: scalar) -> scalar {
    float_round(x)
}

/// Truncate as a scalar.
// Port of: include/core/SkScalar.h#L33 (chrome/m156)
#[doc(alias = "SkScalarTruncToScalar")]
#[must_use]
pub fn scalar_trunc_to_scalar(x: scalar) -> scalar {
    x.trunc()
}

/// Saturating floor to int.
// Port of: include/core/SkScalar.h#L35 (chrome/m156)
#[doc(alias = "SkScalarFloorToInt")]
#[must_use]
pub fn scalar_floor_to_int(x: scalar) -> i32 {
    float_floor2int(x)
}

/// Saturating ceil to int.
// Port of: include/core/SkScalar.h#L36 (chrome/m156)
#[doc(alias = "SkScalarCeilToInt")]
#[must_use]
pub fn scalar_ceil_to_int(x: scalar) -> i32 {
    float_ceil2int(x)
}

/// Saturating round to int.
// Port of: include/core/SkScalar.h#L37 (chrome/m156)
#[doc(alias = "SkScalarRoundToInt")]
#[must_use]
pub fn scalar_round_to_int(x: scalar) -> i32 {
    float_round2int(x)
}

/// Absolute value.
// Port of: include/core/SkScalar.h#L39 (chrome/m156)
#[doc(alias = "SkScalarAbs")]
#[must_use]
pub fn scalar_abs(x: scalar) -> scalar {
    x.abs()
}

/// `copysign(x, y)`.
// Port of: include/core/SkScalar.h#L40 (chrome/m156)
#[doc(alias = "SkScalarCopySign")]
#[must_use]
pub fn scalar_copy_sign(x: scalar, y: scalar) -> scalar {
    x.copysign(y)
}

/// `fmod(x, y)`.
// Port of: include/core/SkScalar.h#L41 (chrome/m156)
#[doc(alias = "SkScalarMod")]
#[must_use]
pub fn scalar_mod(x: scalar, y: scalar) -> scalar {
    x % y
}

/// Square root.
// Port of: include/core/SkScalar.h#L42 (chrome/m156)
#[doc(alias = "SkScalarSqrt")]
#[must_use]
pub fn scalar_sqrt(x: scalar) -> scalar {
    x.sqrt()
}

/// `pow(b, e)`.
// Port of: include/core/SkScalar.h#L43 (chrome/m156)
#[doc(alias = "SkScalarPow")]
#[must_use]
pub fn scalar_pow(b: scalar, e: scalar) -> scalar {
    // skia-rust: libm
    b.powf(e)
}

/// `sin(radians)`.
// Port of: include/core/SkScalar.h#L45 (chrome/m156)
#[doc(alias = "SkScalarSin")]
#[must_use]
pub fn scalar_sin(radians: scalar) -> scalar {
    // skia-rust: libm
    radians.sin()
}

/// `cos(radians)`.
// Port of: include/core/SkScalar.h#L46 (chrome/m156)
#[doc(alias = "SkScalarCos")]
#[must_use]
pub fn scalar_cos(radians: scalar) -> scalar {
    // skia-rust: libm
    radians.cos()
}

/// `tan(radians)`.
// Port of: include/core/SkScalar.h#L47 (chrome/m156)
#[doc(alias = "SkScalarTan")]
#[must_use]
pub fn scalar_tan(radians: scalar) -> scalar {
    // skia-rust: libm
    radians.tan()
}

/// `asin(val)`.
// Port of: include/core/SkScalar.h#L48 (chrome/m156)
#[doc(alias = "SkScalarASin")]
#[must_use]
pub fn scalar_asin(val: scalar) -> scalar {
    // skia-rust: libm
    val.asin()
}

/// `acos(val)`.
// Port of: include/core/SkScalar.h#L49 (chrome/m156)
#[doc(alias = "SkScalarACos")]
#[must_use]
pub fn scalar_acos(val: scalar) -> scalar {
    // skia-rust: libm
    val.acos()
}

/// `atan2(y, x)`.
// Port of: include/core/SkScalar.h#L50 (chrome/m156)
#[doc(alias = "SkScalarATan2")]
#[must_use]
pub fn scalar_atan2(y: scalar, x: scalar) -> scalar {
    // skia-rust: libm
    y.atan2(x)
}

/// `exp(x)`.
// Port of: include/core/SkScalar.h#L51 (chrome/m156)
#[doc(alias = "SkScalarExp")]
#[must_use]
pub fn scalar_exp(x: scalar) -> scalar {
    // skia-rust: libm
    x.exp()
}

/// Natural log.
// Port of: include/core/SkScalar.h#L52 (chrome/m156)
#[doc(alias = "SkScalarLog")]
#[must_use]
pub fn scalar_log(x: scalar) -> scalar {
    // skia-rust: libm
    x.ln()
}

/// Base-2 log.
// Port of: include/core/SkScalar.h#L53 (chrome/m156)
#[doc(alias = "SkScalarLog2")]
#[must_use]
pub fn scalar_log2(x: scalar) -> scalar {
    // skia-rust: libm
    x.log2()
}

/// Converts an int to a scalar.
// Port of: include/core/SkScalar.h#L57 (chrome/m156)
#[doc(alias = "SkIntToScalar")]
#[allow(clippy::cast_precision_loss)] // mirrors static_cast<SkScalar>(int)
#[must_use]
pub fn int_to_scalar(x: i32) -> scalar {
    x as scalar
}

/// Converts an int to a float.
// Port of: include/core/SkScalar.h#L58 (chrome/m156)
#[doc(alias = "SkIntToFloat")]
#[allow(clippy::cast_precision_loss)] // mirrors static_cast<float>(int)
#[must_use]
pub fn int_to_float(x: i32) -> f32 {
    x as f32
}

/// Saturating truncation to int.
// Port of: include/core/SkScalar.h#L59 (chrome/m156)
#[doc(alias = "SkScalarTruncToInt")]
#[must_use]
pub fn scalar_trunc_to_int(x: scalar) -> i32 {
    float_saturate2int(x)
}

/// Scalar to double.
// Port of: include/core/SkScalar.h#L63 (chrome/m156)
#[doc(alias = "SkScalarToDouble")]
#[must_use]
pub fn scalar_to_double(x: scalar) -> f64 {
    f64::from(x)
}

/// Double to scalar (`sk_double_to_float`).
// Port of: include/core/SkScalar.h#L64 (chrome/m156)
#[doc(alias = "SkDoubleToScalar")]
#[must_use]
pub fn double_to_scalar(x: f64) -> scalar {
    double_to_float(x)
}

/// Returns the fractional part of the scalar.
// Port of: include/core/SkScalar.h#L67-L69 (chrome/m156)
#[doc(alias = "SkScalarFraction")]
#[must_use]
pub fn scalar_fraction(x: scalar) -> scalar {
    x - scalar_trunc_to_scalar(x)
}

/// `x * x`.
// Port of: include/core/SkScalar.h#L71 (chrome/m156)
#[doc(alias = "SkScalarSquare")]
#[must_use]
pub fn scalar_square(x: scalar) -> scalar {
    x * x
}

/// `1 / x`.
// Port of: include/core/SkScalar.h#L73 (chrome/m156)
#[doc(alias = "SkScalarInvert")]
#[must_use]
pub fn scalar_invert(x: scalar) -> scalar {
    SCALAR_1 / x
}

/// Midpoint of `a` and `b`.
// Port of: include/core/SkScalar.h#L74 (chrome/m156)
#[doc(alias = "SkScalarAve")]
#[must_use]
pub fn scalar_ave(a: scalar, b: scalar) -> scalar {
    float_midpoint(a, b)
}

/// Degrees to radians.
// Port of: include/core/SkScalar.h#L76 (chrome/m156)
#[doc(alias = "SkDegreesToRadians")]
#[must_use]
pub fn degrees_to_radians(degrees: scalar) -> scalar {
    degrees * (SCALAR_PI / 180.0)
}

/// Radians to degrees.
// Port of: include/core/SkScalar.h#L77 (chrome/m156)
#[doc(alias = "SkRadiansToDegrees")]
#[must_use]
pub fn radians_to_degrees(radians: scalar) -> scalar {
    radians * (180.0 / SCALAR_PI)
}

/// True if `x` is integral.
// Port of: include/core/SkScalar.h#L79-L81 (chrome/m156)
#[doc(alias = "SkScalarIsInt")]
#[allow(clippy::float_cmp)] // exact comparison, as in Skia
#[must_use]
pub fn scalar_is_int(x: scalar) -> bool {
    x == scalar_floor_to_scalar(x)
}

/// Returns -1, 0 or 1 depending on the sign of `x`.
// Port of: include/core/SkScalar.h#L89-L91 (chrome/m156)
#[doc(alias = "SkScalarSignAsInt")]
#[must_use]
pub fn scalar_sign_as_int(x: scalar) -> i32 {
    if x < 0.0 { -1 } else { i32::from(x > 0.0) }
}

/// Scalar result version of [`scalar_sign_as_int`].
// Port of: include/core/SkScalar.h#L94-L96 (chrome/m156)
#[doc(alias = "SkScalarSignAsScalar")]
#[must_use]
pub fn scalar_sign_as_scalar(x: scalar) -> scalar {
    if x < 0.0 {
        -SCALAR_1
    } else if x > 0.0 {
        SCALAR_1
    } else {
        0.0
    }
}

/// `sin(radians)`, snapped to 0 when nearly zero.
// Port of: include/core/SkScalar.h#L114-L117 (chrome/m156)
#[doc(alias = "SkScalarSinSnapToZero")]
#[must_use]
pub fn scalar_sin_snap_to_zero(radians: scalar) -> f32 {
    let v = scalar_sin(radians);
    if v.nearly_zero(SCALAR_SIN_COS_NEARLY_ZERO) {
        0.0
    } else {
        v
    }
}

/// `cos(radians)`, snapped to 0 when nearly zero.
// Port of: include/core/SkScalar.h#L119-L122 (chrome/m156)
#[doc(alias = "SkScalarCosSnapToZero")]
#[must_use]
pub fn scalar_cos_snap_to_zero(radians: scalar) -> f32 {
    let v = scalar_cos(radians);
    if v.nearly_zero(SCALAR_SIN_COS_NEARLY_ZERO) {
        0.0
    } else {
        v
    }
}

/// Linearly interpolates between `a` and `b` based on `t` (`0..=1`).
// Port of: include/core/SkScalar.h#L130-L133 (chrome/m156)
#[doc(alias = "SkScalarInterp")]
#[must_use]
pub fn scalar_interp(a: scalar, b: scalar, t: scalar) -> scalar {
    debug_assert!((0.0..=SCALAR_1).contains(&t));
    a + (b - a) * t
}

/// True if both slices (of equal length) hold equal scalars.
// Port of: include/core/SkScalar.h#L138-L145 (chrome/m156)
#[doc(alias = "SkScalarsEqual")]
#[allow(clippy::float_cmp)] // exact comparison, as in Skia
#[must_use]
pub fn scalars_equal(a: &[scalar], b: &[scalar]) -> bool {
    debug_assert_eq!(a.len(), b.len());
    a.iter().zip(b).all(|(x, y)| x == y)
}
