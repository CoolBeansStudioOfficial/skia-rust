// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsTypes.h, src/pathops/SkPathOpsTypes.cpp

//! Epsilons, ULPS comparisons and small helpers shared by all of `PathOps`
//! (`SkPathOpsTypes.h`).

use skia_rust_core::float_bits::float_as_2s_compliment;
use skia_rust_core::floating_point::is_finite;
use skia_rust_core::path::Verb;
use skia_rust_core::scalar::double_to_scalar;

/// `FLT_EPSILON == 1.19209290E-07 == 1 / (2 ^ 23)`, as the exact `double` the C++ promotes it to.
const FLT_EPSILON: f64 = f32::EPSILON as f64;
/// `DBL_EPSILON == 2.22045e-16`.
const DBL_EPSILON: f64 = f64::EPSILON;

// Port of: src/pathops/SkPathOpsTypes.h#L305-L305 (chrome/m156)
pub const FLT_EPSILON_CUBED: f64 = FLT_EPSILON * FLT_EPSILON * FLT_EPSILON;
pub const FLT_EPSILON_HALF: f64 = FLT_EPSILON / 2.0;
pub const FLT_EPSILON_DOUBLE: f64 = FLT_EPSILON * 2.0;
pub const FLT_EPSILON_ORDERABLE_ERR: f64 = FLT_EPSILON * 16.0;
pub const FLT_EPSILON_SQUARED: f64 = FLT_EPSILON * FLT_EPSILON;
/// `sqrt(FLT_EPSILON)`, written as a constant in Skia (a 17 digit literal is exact).
#[allow(clippy::excessive_precision)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
pub const FLT_EPSILON_SQRT: f64 = 0.000_345_266_977_092_251_18;
pub const FLT_EPSILON_INVERSE: f64 = 1.0 / FLT_EPSILON;
pub const DBL_EPSILON_ERR: f64 = DBL_EPSILON * 4.0;
pub const DBL_EPSILON_SUBDIVIDE_ERR: f64 = DBL_EPSILON * 16.0;
pub const ROUGH_EPSILON: f64 = FLT_EPSILON * 64.0;
pub const MORE_ROUGH_EPSILON: f64 = FLT_EPSILON * 256.0;
pub const WAY_ROUGH_EPSILON: f64 = FLT_EPSILON * 2048.0;
pub const BUMP_EPSILON: f64 = FLT_EPSILON * 4096.0;
/// `SkScalar INVERSE_NUMBER_RANGE = FLT_EPSILON_ORDERABLE_ERR;`
#[allow(clippy::cast_possible_truncation)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
pub const INVERSE_NUMBER_RANGE: f32 = FLT_EPSILON_ORDERABLE_ERR as f32;

/// The `float` / `double` overload pair of the C++ `AlmostEqualUlps` family.
///
/// C++ overloads on `double` convert to `SkScalar` (`SkDoubleToScalar`) before comparing, so each
/// comparison takes this conversion for `f64` and the identity for `f32`.
pub trait UlpsScalar: Copy {
    /// The value as the `SkScalar` the C++ overload sees.
    fn to_sk_scalar(self) -> f32;
    /// `AlmostDequalUlps` for this type; the `double` overload is not a plain conversion.
    fn almost_dequal(a: Self, b: Self) -> bool;
}

impl UlpsScalar for f32 {
    fn to_sk_scalar(self) -> f32 {
        self
    }
    // Port of: src/pathops/SkPathOpsTypes.cpp (chrome/m156)
    fn almost_dequal(a: Self, b: Self) -> bool {
        d_equal_ulps(a, b, 16)
    }
}

impl UlpsScalar for f64 {
    fn to_sk_scalar(self) -> f32 {
        double_to_scalar(self)
    }
    // Port of: src/pathops/SkPathOpsTypes.cpp (chrome/m156)
    fn almost_dequal(a: Self, b: Self) -> bool {
        if a.abs() < f64::from(f32::MAX) && b.abs() < f64::from(f32::MAX) {
            return d_equal_ulps(double_to_scalar(a), double_to_scalar(b), 16);
        }
        (a - b).abs() / a.abs().max(b.abs()) < f64::from(f32::EPSILON) * 16.0
    }
}

// Port of: src/pathops/SkPathOpsTypes.cpp#L18-L21 (chrome/m156)
fn arguments_denormalized(a: f32, b: f32, epsilon: i32) -> bool {
    #[allow(clippy::cast_precision_loss)] // mirrors `FLT_EPSILON * epsilon` in float
    let denormalized_check = f32::EPSILON * epsilon as f32 / 2.0;
    a.abs() <= denormalized_check && b.abs() <= denormalized_check
}

// Port of: src/pathops/SkPathOpsTypes.cpp#L25-L33 (chrome/m156)
fn equal_ulps(a: f32, b: f32, epsilon: i32, depsilon: i32) -> bool {
    if arguments_denormalized(a, b, depsilon) {
        return true;
    }
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits < b_bits + epsilon && b_bits < a_bits + epsilon
}

// Port of: src/pathops/SkPathOpsTypes.cpp#L35-L40 (chrome/m156)
fn equal_ulps_no_normal_check(a: f32, b: f32, epsilon: i32) -> bool {
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits < b_bits + epsilon && b_bits < a_bits + epsilon
}

// Port of: src/pathops/SkPathOpsTypes.cpp#L42-L53 (chrome/m156)
fn equal_ulps_pin(a: f32, b: f32, epsilon: i32, depsilon: i32) -> bool {
    if !is_finite(a) || !is_finite(b) {
        return false;
    }
    if arguments_denormalized(a, b, depsilon) {
        return true;
    }
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits < b_bits + epsilon && b_bits < a_bits + epsilon
}

// Port of: src/pathops/SkPathOpsTypes.cpp#L55-L60 (chrome/m156)
fn d_equal_ulps(a: f32, b: f32, epsilon: i32) -> bool {
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits < b_bits + epsilon && b_bits < a_bits + epsilon
}

// Port of: src/pathops/SkPathOpsTypes.cpp#L62-L70 (chrome/m156)
fn not_equal_ulps(a: f32, b: f32, epsilon: i32) -> bool {
    if arguments_denormalized(a, b, epsilon) {
        return false;
    }
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits >= b_bits + epsilon || b_bits >= a_bits + epsilon
}

// Port of: src/pathops/SkPathOpsTypes.cpp#L72-L83 (chrome/m156)
fn not_equal_ulps_pin(a: f32, b: f32, epsilon: i32) -> bool {
    if !is_finite(a) || !is_finite(b) {
        return false;
    }
    if arguments_denormalized(a, b, epsilon) {
        return false;
    }
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits >= b_bits + epsilon || b_bits >= a_bits + epsilon
}

// Port of: src/pathops/SkPathOpsTypes.cpp#L85-L90 (chrome/m156)
fn d_not_equal_ulps(a: f32, b: f32, epsilon: i32) -> bool {
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits >= b_bits + epsilon || b_bits >= a_bits + epsilon
}

// Port of: src/pathops/SkPathOpsTypes.cpp#L92-L100 (chrome/m156)
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
fn less_ulps(a: f32, b: f32, epsilon: i32) -> bool {
    if arguments_denormalized(a, b, epsilon) {
        return a <= b - FLT_EPSILON as f32 * epsilon as f32;
    }
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits <= b_bits - epsilon
}

// Port of: src/pathops/SkPathOpsTypes.cpp#L102-L110 (chrome/m156)
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
fn less_or_equal_ulps(a: f32, b: f32, epsilon: i32) -> bool {
    if arguments_denormalized(a, b, epsilon) {
        return a < b + FLT_EPSILON as f32 * epsilon as f32;
    }
    let a_bits = float_as_2s_compliment(a);
    let b_bits = float_as_2s_compliment(b);
    a_bits < b_bits + epsilon
}

/// Port of `AlmostEqualUlps(float, float)` / `(double, double)`.
// Port of: src/pathops/SkPathOpsTypes.cpp#L138-L141 (chrome/m156)
#[doc(alias = "AlmostEqualUlps")]
#[must_use]
pub fn almost_equal_ulps<T: UlpsScalar>(a: T, b: T) -> bool {
    equal_ulps(a.to_sk_scalar(), b.to_sk_scalar(), 16, 16)
}

/// Port of `AlmostEqualUlpsNoNormalCheck`.
// Port of: src/pathops/SkPathOpsTypes.cpp#L143-L146 (chrome/m156)
#[doc(alias = "AlmostEqualUlpsNoNormalCheck")]
#[must_use]
pub fn almost_equal_ulps_no_normal_check<T: UlpsScalar>(a: T, b: T) -> bool {
    equal_ulps_no_normal_check(a.to_sk_scalar(), b.to_sk_scalar(), 16)
}

/// Port of `AlmostEqualUlps_Pin`.
// Port of: src/pathops/SkPathOpsTypes.cpp (chrome/m156)
#[doc(alias = "AlmostEqualUlps_Pin")]
#[must_use]
pub fn almost_equal_ulps_pin<T: UlpsScalar>(a: T, b: T) -> bool {
    equal_ulps_pin(a.to_sk_scalar(), b.to_sk_scalar(), 16, 16)
}

/// Port of `AlmostDequalUlps(float, float)` / `(double, double)`.
// Port of: src/pathops/SkPathOpsTypes.cpp#L123-L126 (chrome/m156)
#[doc(alias = "AlmostDequalUlps")]
#[must_use]
pub fn almost_dequal_ulps<T: UlpsScalar>(a: T, b: T) -> bool {
    T::almost_dequal(a, b)
}

/// Port of `NotAlmostEqualUlps`.
// Port of: src/pathops/SkPathOpsTypes.cpp#L153-L156 (chrome/m156)
#[doc(alias = "NotAlmostEqualUlps")]
#[must_use]
pub fn not_almost_equal_ulps<T: UlpsScalar>(a: T, b: T) -> bool {
    not_equal_ulps(a.to_sk_scalar(), b.to_sk_scalar(), 16)
}

/// Port of `NotAlmostEqualUlps_Pin`.
// Port of: src/pathops/SkPathOpsTypes.cpp (chrome/m156)
#[doc(alias = "NotAlmostEqualUlps_Pin")]
#[must_use]
pub fn not_almost_equal_ulps_pin<T: UlpsScalar>(a: T, b: T) -> bool {
    not_equal_ulps_pin(a.to_sk_scalar(), b.to_sk_scalar(), 16)
}

/// Port of `NotAlmostDequalUlps`.
// Port of: src/pathops/SkPathOpsTypes.cpp#L163-L166 (chrome/m156)
#[doc(alias = "NotAlmostDequalUlps")]
#[must_use]
pub fn not_almost_dequal_ulps<T: UlpsScalar>(a: T, b: T) -> bool {
    d_not_equal_ulps(a.to_sk_scalar(), b.to_sk_scalar(), 16)
}

/// Port of `AlmostBequalUlps`.
// Port of: src/pathops/SkPathOpsTypes.cpp#L113-L116 (chrome/m156)
#[doc(alias = "AlmostBequalUlps")]
#[must_use]
pub fn almost_bequal_ulps<T: UlpsScalar>(a: T, b: T) -> bool {
    equal_ulps(a.to_sk_scalar(), b.to_sk_scalar(), 2, 2)
}

/// Port of `AlmostPequalUlps`.
// Port of: src/pathops/SkPathOpsTypes.cpp#L118-L121 (chrome/m156)
#[doc(alias = "AlmostPequalUlps")]
#[must_use]
pub fn almost_pequal_ulps<T: UlpsScalar>(a: T, b: T) -> bool {
    equal_ulps(a.to_sk_scalar(), b.to_sk_scalar(), 8, 8)
}

/// Port of `RoughlyEqualUlps`.
// Port of: src/pathops/SkPathOpsTypes.cpp#L168-L172 (chrome/m156)
#[doc(alias = "RoughlyEqualUlps")]
#[must_use]
pub fn roughly_equal_ulps<T: UlpsScalar>(a: T, b: T) -> bool {
    equal_ulps(a.to_sk_scalar(), b.to_sk_scalar(), 256, 1024)
}

/// Port of `AlmostBetweenUlps`.
// Port of: src/pathops/SkPathOpsTypes.cpp#L174-L178 (chrome/m156)
#[doc(alias = "AlmostBetweenUlps")]
#[must_use]
pub fn almost_between_ulps<T: UlpsScalar>(a: T, b: T, c: T) -> bool {
    let (a, b, c) = (a.to_sk_scalar(), b.to_sk_scalar(), c.to_sk_scalar());
    if a <= c {
        less_or_equal_ulps(a, b, 2) && less_or_equal_ulps(b, c, 2)
    } else {
        less_or_equal_ulps(b, a, 2) && less_or_equal_ulps(c, b, 2)
    }
}

/// Port of `AlmostLessUlps`.
// Port of: src/pathops/SkPathOpsTypes.cpp#L180-L183 (chrome/m156)
#[doc(alias = "AlmostLessUlps")]
#[must_use]
pub fn almost_less_ulps<T: UlpsScalar>(a: T, b: T) -> bool {
    less_ulps(a.to_sk_scalar(), b.to_sk_scalar(), 16)
}

/// Port of `AlmostLessOrEqualUlps`.
// Port of: src/pathops/SkPathOpsTypes.cpp#L185-L188 (chrome/m156)
#[doc(alias = "AlmostLessOrEqualUlps")]
#[must_use]
pub fn almost_less_or_equal_ulps<T: UlpsScalar>(a: T, b: T) -> bool {
    less_or_equal_ulps(a.to_sk_scalar(), b.to_sk_scalar(), 16)
}

/// Port of `UlpsDistance`.
// Port of: src/pathops/SkPathOpsTypes.cpp#L190-L201 (chrome/m156)
#[doc(alias = "UlpsDistance")]
#[must_use]
#[allow(clippy::cast_possible_wrap, clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
pub fn ulps_distance<T: UlpsScalar>(a: T, b: T) -> i32 {
    let (a, b) = (a.to_sk_scalar(), b.to_sk_scalar());
    let float_int_a = a.to_bits() as i32;
    let float_int_b = b.to_bits() as i32;
    if (float_int_a < 0) != (float_int_b < 0) {
        return if a == b { 0 } else { i32::MAX };
    }
    float_int_a.wrapping_sub(float_int_b).wrapping_abs()
}

// The predicates below take `f64`. Skia's `float` overloads of the same predicates compare the
// same values after exact float-to-double promotion, so one `f64` version gives the same result.

/// `x == 0 || x == 1`.
#[must_use]
#[allow(clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
pub fn zero_or_one(x: f64) -> bool {
    x == 0.0 || x == 1.0
}

/// `fabs(x) < FLT_EPSILON`.
#[must_use]
pub fn approximately_zero(x: f64) -> bool {
    x.abs() < FLT_EPSILON
}

/// `fabs(x) < DBL_EPSILON_ERR`.
#[must_use]
pub fn precisely_zero(x: f64) -> bool {
    x.abs() < DBL_EPSILON_ERR
}

/// `fabs(x) < DBL_EPSILON_SUBDIVIDE_ERR`.
#[must_use]
pub fn precisely_subdivide_zero(x: f64) -> bool {
    x.abs() < DBL_EPSILON_SUBDIVIDE_ERR
}

/// `fabs(x) < FLT_EPSILON_HALF`.
#[must_use]
pub fn approximately_zero_half(x: f64) -> bool {
    x.abs() < FLT_EPSILON_HALF
}

/// `fabs(x) < FLT_EPSILON_DOUBLE`.
#[must_use]
pub fn approximately_zero_double(x: f64) -> bool {
    x.abs() < FLT_EPSILON_DOUBLE
}

/// `fabs(x) < FLT_EPSILON_ORDERABLE_ERR`.
#[must_use]
pub fn approximately_zero_orderable(x: f64) -> bool {
    x.abs() < FLT_EPSILON_ORDERABLE_ERR
}

/// `fabs(x) < FLT_EPSILON_SQUARED`.
#[must_use]
pub fn approximately_zero_squared(x: f64) -> bool {
    x.abs() < FLT_EPSILON_SQUARED
}

/// `fabs(x) < FLT_EPSILON_SQRT`.
#[must_use]
pub fn approximately_zero_sqrt(x: f64) -> bool {
    x.abs() < FLT_EPSILON_SQRT
}

/// `fabs(x) < ROUGH_EPSILON`.
#[must_use]
pub fn roughly_zero(x: f64) -> bool {
    x.abs() < ROUGH_EPSILON
}

/// `fabs(x) > FLT_EPSILON_INVERSE`.
#[must_use]
pub fn approximately_zero_inverse(x: f64) -> bool {
    x.abs() > FLT_EPSILON_INVERSE
}

/// `x == 0 || fabs(x) < fabs(y * FLT_EPSILON)`.
#[must_use]
pub fn approximately_zero_when_compared_to(x: f64, y: f64) -> bool {
    x == 0.0 || x.abs() < (y * FLT_EPSILON).abs()
}

/// `x == 0 || fabs(x) < fabs(y * DBL_EPSILON)`.
#[must_use]
pub fn precisely_zero_when_compared_to(x: f64, y: f64) -> bool {
    x == 0.0 || x.abs() < (y * DBL_EPSILON).abs()
}

/// `x == 0 || fabs(x) < fabs(y * ROUGH_EPSILON)`.
#[must_use]
pub fn roughly_zero_when_compared_to(x: f64, y: f64) -> bool {
    x == 0.0 || x.abs() < (y * ROUGH_EPSILON).abs()
}

/// Use this for comparing Ts in the range of 0 to 1: `approximately_zero(x - y)`.
#[must_use]
pub fn approximately_equal(x: f64, y: f64) -> bool {
    approximately_zero(x - y)
}

/// `precisely_zero(x - y)`.
#[must_use]
pub fn precisely_equal(x: f64, y: f64) -> bool {
    precisely_zero(x - y)
}

/// `precisely_subdivide_zero(x - y)`.
#[must_use]
pub fn precisely_subdivide_equal(x: f64, y: f64) -> bool {
    precisely_subdivide_zero(x - y)
}

/// `approximately_zero_half(x - y)`.
#[must_use]
pub fn approximately_equal_half(x: f64, y: f64) -> bool {
    approximately_zero_half(x - y)
}

/// `approximately_zero_double(x - y)`.
#[must_use]
pub fn approximately_equal_double(x: f64, y: f64) -> bool {
    approximately_zero_double(x - y)
}

/// `approximately_zero_orderable(x - y)`.
#[must_use]
pub fn approximately_equal_orderable(x: f64, y: f64) -> bool {
    approximately_zero_orderable(x - y)
}

/// `approximately_zero_squared(x - y)`.
#[must_use]
pub fn approximately_equal_squared(x: f64, y: f64) -> bool {
    approximately_equal(x, y)
}

/// `x - FLT_EPSILON >= y`.
#[must_use]
pub fn approximately_greater(x: f64, y: f64) -> bool {
    x - FLT_EPSILON >= y
}

/// `x - FLT_EPSILON_DOUBLE >= y`.
#[must_use]
pub fn approximately_greater_double(x: f64, y: f64) -> bool {
    x - FLT_EPSILON_DOUBLE >= y
}

/// `x - FLT_EPSILON_ORDERABLE_ERR >= y`.
#[must_use]
pub fn approximately_greater_orderable(x: f64, y: f64) -> bool {
    x - FLT_EPSILON_ORDERABLE_ERR >= y
}

/// `x + FLT_EPSILON > y`.
#[must_use]
pub fn approximately_greater_or_equal(x: f64, y: f64) -> bool {
    x + FLT_EPSILON > y
}

/// `x + FLT_EPSILON_DOUBLE > y`.
#[must_use]
pub fn approximately_greater_or_equal_double(x: f64, y: f64) -> bool {
    x + FLT_EPSILON_DOUBLE > y
}

/// `x + FLT_EPSILON_ORDERABLE_ERR > y`.
#[must_use]
pub fn approximately_greater_or_equal_orderable(x: f64, y: f64) -> bool {
    x + FLT_EPSILON_ORDERABLE_ERR > y
}

/// `x + FLT_EPSILON <= y`.
#[must_use]
pub fn approximately_lesser(x: f64, y: f64) -> bool {
    x + FLT_EPSILON <= y
}

/// `x + FLT_EPSILON_DOUBLE <= y`.
#[must_use]
pub fn approximately_lesser_double(x: f64, y: f64) -> bool {
    x + FLT_EPSILON_DOUBLE <= y
}

/// `x + FLT_EPSILON_ORDERABLE_ERR <= y`.
#[must_use]
pub fn approximately_lesser_orderable(x: f64, y: f64) -> bool {
    x + FLT_EPSILON_ORDERABLE_ERR <= y
}

/// `x - FLT_EPSILON < y`.
#[must_use]
pub fn approximately_lesser_or_equal(x: f64, y: f64) -> bool {
    x - FLT_EPSILON < y
}

/// `x - FLT_EPSILON_DOUBLE < y`.
#[must_use]
pub fn approximately_lesser_or_equal_double(x: f64, y: f64) -> bool {
    x - FLT_EPSILON_DOUBLE < y
}

/// `x - FLT_EPSILON_ORDERABLE_ERR < y`.
#[must_use]
pub fn approximately_lesser_or_equal_orderable(x: f64, y: f64) -> bool {
    x - FLT_EPSILON_ORDERABLE_ERR < y
}

/// `x > 1 - FLT_EPSILON`.
#[must_use]
pub fn approximately_greater_than_one(x: f64) -> bool {
    x > 1.0 - FLT_EPSILON
}

/// `x > 1 - DBL_EPSILON_ERR`.
#[must_use]
pub fn precisely_greater_than_one(x: f64) -> bool {
    x > 1.0 - DBL_EPSILON_ERR
}

/// `x < FLT_EPSILON`.
#[must_use]
pub fn approximately_less_than_zero(x: f64) -> bool {
    x < FLT_EPSILON
}

/// `x < DBL_EPSILON_ERR`.
#[must_use]
pub fn precisely_less_than_zero(x: f64) -> bool {
    x < DBL_EPSILON_ERR
}

/// `x < FLT_EPSILON`.
#[must_use]
pub fn approximately_negative(x: f64) -> bool {
    x < FLT_EPSILON
}

/// `x < FLT_EPSILON_ORDERABLE_ERR`.
#[must_use]
pub fn approximately_negative_orderable(x: f64) -> bool {
    x < FLT_EPSILON_ORDERABLE_ERR
}

/// `x < DBL_EPSILON_ERR`.
#[must_use]
pub fn precisely_negative(x: f64) -> bool {
    x < DBL_EPSILON_ERR
}

/// `x < 1 + FLT_EPSILON`.
#[must_use]
pub fn approximately_one_or_less(x: f64) -> bool {
    x < 1.0 + FLT_EPSILON
}

/// `x < 1 + FLT_EPSILON_DOUBLE`.
#[must_use]
pub fn approximately_one_or_less_double(x: f64) -> bool {
    x < 1.0 + FLT_EPSILON_DOUBLE
}

/// `x > -FLT_EPSILON`.
#[must_use]
pub fn approximately_positive(x: f64) -> bool {
    x > -FLT_EPSILON
}

/// `x > -(FLT_EPSILON_SQUARED)`.
#[must_use]
pub fn approximately_positive_squared(x: f64) -> bool {
    x > -FLT_EPSILON_SQUARED
}

/// `x > -FLT_EPSILON`.
#[must_use]
pub fn approximately_zero_or_more(x: f64) -> bool {
    x > -FLT_EPSILON
}

/// `x > -FLT_EPSILON_DOUBLE`.
#[must_use]
pub fn approximately_zero_or_more_double(x: f64) -> bool {
    x > -FLT_EPSILON_DOUBLE
}

/// `a <= c ? approximately_negative_orderable(a - b) && approximately_negative_orderable(b - c)
/// : approximately_negative_orderable(b - a) && approximately_negative_orderable(c - b)`.
#[must_use]
pub fn approximately_between_orderable(a: f64, b: f64, c: f64) -> bool {
    if a <= c {
        approximately_negative_orderable(a - b) && approximately_negative_orderable(b - c)
    } else {
        approximately_negative_orderable(b - a) && approximately_negative_orderable(c - b)
    }
}

/// `a <= c ? approximately_negative(a - b) && approximately_negative(b - c)
/// : approximately_negative(b - a) && approximately_negative(c - b)`.
#[must_use]
pub fn approximately_between(a: f64, b: f64, c: f64) -> bool {
    if a <= c {
        approximately_negative(a - b) && approximately_negative(b - c)
    } else {
        approximately_negative(b - a) && approximately_negative(c - b)
    }
}

/// `a <= c ? precisely_negative(a - b) && precisely_negative(b - c)
/// : precisely_negative(b - a) && precisely_negative(c - b)`.
#[must_use]
pub fn precisely_between(a: f64, b: f64, c: f64) -> bool {
    if a <= c {
        precisely_negative(a - b) && precisely_negative(b - c)
    } else {
        precisely_negative(b - a) && precisely_negative(c - b)
    }
}

/// Returns true if `(a <= b <= c) || (a >= b >= c)`.
// Port of: src/pathops/SkPathOpsTypes.h#L530-L534 (chrome/m156)
#[must_use]
pub fn between(a: f64, b: f64, c: f64) -> bool {
    debug_assert!(
        (((a <= b && b <= c) || (a >= b && b >= c)) == ((a - b) * (c - b) <= 0.0))
            || (precisely_zero(a) && precisely_zero(b) && precisely_zero(c))
    );
    (a - b) * (c - b) <= 0.0
}

/// `fabs(x - y) < ROUGH_EPSILON`.
#[must_use]
pub fn roughly_equal(x: f64, y: f64) -> bool {
    (x - y).abs() < ROUGH_EPSILON
}

/// `x < ROUGH_EPSILON`.
#[must_use]
pub fn roughly_negative(x: f64) -> bool {
    x < ROUGH_EPSILON
}

/// `a <= c ? roughly_negative(a - b) && roughly_negative(b - c)
/// : roughly_negative(b - a) && roughly_negative(c - b)`.
#[must_use]
pub fn roughly_between(a: f64, b: f64, c: f64) -> bool {
    if a <= c {
        roughly_negative(a - b) && roughly_negative(b - c)
    } else {
        roughly_negative(b - a) && roughly_negative(c - b)
    }
}

/// `fabs(x - y) < MORE_ROUGH_EPSILON`.
#[must_use]
pub fn more_roughly_equal(x: f64, y: f64) -> bool {
    (x - y).abs() < MORE_ROUGH_EPSILON
}

/// Port of `SkPathOpsPointsToVerb`: the verb for a count of points (0 move, 1 line, 2 quad, 3 cubic).
// Port of: src/pathops/SkPathOpsTypes.h (chrome/m156)
#[doc(alias = "SkPathOpsPointsToVerb")]
#[must_use]
pub fn points_to_verb(points: i32) -> Verb {
    let verb = (1 << points) >> 1;
    match verb {
        0 => Verb::Move,
        1 => Verb::Line,
        2 => Verb::Quad,
        3 => Verb::Cubic,
        _ => unreachable!("should not be here"),
    }
}

/// Port of `SkPathOpsVerbToPoints`.
// Port of: src/pathops/SkPathOpsTypes.h (chrome/m156)
#[doc(alias = "SkPathOpsVerbToPoints")]
#[must_use]
pub fn verb_to_points(verb: Verb) -> i32 {
    let v = verb as i32;
    v - ((v + 1) >> 2)
}

/// `start + (end - start) * t`, the `SkDInterp` of Skia.
// Port of: src/pathops/SkPathOpsTypes.h#L581-L583 (chrome/m156)
#[doc(alias = "SkDInterp")]
#[must_use]
pub fn d_interp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Returns -1 if negative, 0 if zero, 1 if positive.
// Port of: src/pathops/SkPathOpsTypes.h#L587-L589 (chrome/m156)
#[doc(alias = "SkDSign")]
#[must_use]
pub fn d_sign(x: f64) -> i32 {
    i32::from(x > 0.0) - i32::from(x < 0.0)
}

/// Returns 0 if negative, 1 if zero, 2 if positive.
// Port of: src/pathops/SkPathOpsTypes.h (chrome/m156)
#[doc(alias = "SKDSide")]
#[must_use]
pub fn d_side(x: f64) -> i32 {
    i32::from(x > 0.0) + i32::from(x >= 0.0)
}

/// Returns 1 if negative, 2 if zero, 4 if positive.
// Port of: src/pathops/SkPathOpsTypes.h#L599-L601 (chrome/m156)
#[doc(alias = "SkDSideBit")]
#[must_use]
pub fn d_side_bit(x: f64) -> i32 {
    1 << d_side(x)
}

/// `std::min(a, b)`: returns `b` only when `b < a`, so NaN handling matches the C++ template.
#[doc(alias = "std::min")]
#[must_use]
pub fn std_min<T: PartialOrd>(a: T, b: T) -> T {
    if b < a { b } else { a }
}

/// `std::max(a, b)`: returns `b` only when `a < b`, so NaN handling matches the C++ template.
#[doc(alias = "std::max")]
#[must_use]
pub fn std_max<T: PartialOrd>(a: T, b: T) -> T {
    if a < b { b } else { a }
}

/// Clamps `t` to `[0, 1]`, treating values within `DBL_EPSILON_ERR` of the ends as the ends.
// Port of: src/pathops/SkPathOpsTypes.h#L603-L605 (chrome/m156)
#[doc(alias = "SkPinT")]
#[must_use]
pub fn pin_t(t: f64) -> f64 {
    if precisely_less_than_zero(t) {
        0.0
    } else if precisely_greater_than_one(t) {
        1.0
    } else {
        t
    }
}
