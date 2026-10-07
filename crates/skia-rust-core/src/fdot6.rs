// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkFDot6.h

//! 26.6 fixed point (`SkFDot6.h`), used by the scan converters.
//!
//! `SkFDot6` values have 6 fractional bits. The macros of the C++ header become functions.

use crate::fixed::{Fixed, fixed_div};
use crate::math::left_shift;
use crate::scalar::{scalar, scalar_to_double};

/// 26.6 fixed point.
// Port of: src/core/SkFDot6.h#L18 (chrome/m156)
#[doc(alias = "SkFDot6")]
pub type Fdot6 = i32;

/// `SK_FDot6One`.
// Port of: src/core/SkFDot6.h#L42 (chrome/m156)
#[doc(alias = "SK_FDot6One")]
pub const FDOT6_ONE: Fdot6 = 64;

/// `SK_FDot6Half`.
// Port of: src/core/SkFDot6.h#L43 (chrome/m156)
#[doc(alias = "SK_FDot6Half")]
pub const FDOT6_HALF: Fdot6 = 32;

/// Converts to 26.6 using the "magic number" approach (banker's rounding, i.e. round to nearest
/// even) of <http://stereopsis.com/sree/fpu2006.html>.
// Port of: src/core/SkFDot6.h#L25-L40 (chrome/m156)
#[doc(alias = "SkScalarRoundToFDot6")]
#[allow(clippy::cast_possible_truncation)] // mirrors reading the low word of the double's bits
#[allow(clippy::cast_possible_wrap)] // mirrors reading the low word as int32_t
#[allow(clippy::cast_precision_loss)] // (1LL << n) is exact in a double
#[must_use]
pub fn scalar_round_to_fdot6(x: scalar, shift: i32) -> Fdot6 {
    let fractional_bits = 6 + shift;
    let magic = (1_i64 << (52 - fractional_bits)) as f64 * 1.5;
    let tmp = scalar_to_double(x) + magic;
    // Little endian: fBits[0] is the low word.
    tmp.to_bits() as u32 as i32
}

/// `SkIntToFDot6`.
// Port of: src/core/SkFDot6.h#L45-L53 (chrome/m156)
#[doc(alias = "SkIntToFDot6")]
#[must_use]
pub fn int_to_fdot6(x: i32) -> Fdot6 {
    left_shift(x, 6)
}

/// `SkFDot6Floor`.
// Port of: src/core/SkFDot6.h#L55 (chrome/m156)
#[doc(alias = "SkFDot6Floor")]
#[must_use]
pub fn fdot6_floor(x: Fdot6) -> i32 {
    x >> 6
}

/// `SkFDot6Ceil`.
// Port of: src/core/SkFDot6.h#L56 (chrome/m156)
#[doc(alias = "SkFDot6Ceil")]
#[must_use]
pub fn fdot6_ceil(x: Fdot6) -> i32 {
    x.wrapping_add(63) >> 6
}

/// `SkFDot6Round`.
// Port of: src/core/SkFDot6.h#L57 (chrome/m156)
#[doc(alias = "SkFDot6Round")]
#[must_use]
pub fn fdot6_round(x: Fdot6) -> i32 {
    x.wrapping_add(FDOT6_HALF) >> 6
}

/// `SkFixedToFDot6`.
// Port of: src/core/SkFDot6.h#L59 (chrome/m156)
#[doc(alias = "SkFixedToFDot6")]
#[must_use]
pub fn fixed_to_fdot6(x: Fixed) -> Fdot6 {
    x >> 10
}

/// `SkFDot6ToFixed`.
// Port of: src/core/SkFDot6.h#L61-L65 (chrome/m156)
#[doc(alias = "SkFDot6ToFixed")]
#[must_use]
pub fn fdot6_to_fixed(x: Fdot6) -> Fixed {
    debug_assert_eq!(left_shift(x, 10) >> 10, x);
    left_shift(x, 10)
}

/// `SkFloatToFDot6`: `(SkFDot6)(x * 64)`.
///
/// skia-rust: the C++ cast is undefined for NaN and out-of-range values; the oracle (x86-64,
/// `cvttss2si`) yields `i32::MIN` for them, which this mirrors.
// Port of: src/core/SkFDot6.h#L67 (chrome/m156)
#[doc(alias = "SkFloatToFDot6")]
#[doc(alias = "SkScalarToFDot6")]
#[allow(clippy::cast_possible_truncation)] // mirrors the (SkFDot6) cast; the range is checked
#[allow(clippy::cast_precision_loss)] // the bounds are exactly representable
#[must_use]
pub fn float_to_fdot6(x: scalar) -> Fdot6 {
    let v = x * FDOT6_ONE as f32;
    if v >= 2_147_483_648.0_f32 || v < -2_147_483_648.0_f32 || v.is_nan() {
        i32::MIN
    } else {
        v as i32
    }
}

/// `SkFDot6ToFloat`.
// Port of: src/core/SkFDot6.h#L69 (chrome/m156)
#[doc(alias = "SkFDot6ToFloat")]
#[doc(alias = "SkFDot6ToScalar")]
#[allow(clippy::cast_precision_loss)] // mirrors the (float) cast
#[must_use]
pub fn fdot6_to_float(x: Fdot6) -> f32 {
    x as f32 * 0.015_625_f32
}

/// `SkFDot6Div`.
// Port of: src/core/SkFDot6.h#L72-L80 (chrome/m156)
#[doc(alias = "SkFDot6Div")]
#[must_use]
pub fn fdot6_div(a: Fdot6, b: Fdot6) -> Fixed {
    debug_assert!(b != 0);

    if i16::try_from(a).is_ok() {
        left_shift(a, 16) / b
    } else {
        fixed_div(a, b)
    }
}
