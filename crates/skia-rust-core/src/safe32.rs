// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/private/SkSafe32.h

//! Saturating and overflow-tolerant 32-bit helpers (`SkSafe32.h`).

use crate::math::{MAX_S32, MIN_S32, NAN32};

/// Pins a 64-bit value to the `i32` range.
// Port of: include/private/SkSafe32.h#L16-L18 (chrome/m156)
#[doc(alias = "Sk64_pin_to_s32")]
#[allow(clippy::cast_possible_truncation)] // x is within the i32 range at the cast
#[must_use]
pub const fn pin_to_s32(x: i64) -> i32 {
    if x < MIN_S32 as i64 {
        MIN_S32
    } else if x > MAX_S32 as i64 {
        MAX_S32
    } else {
        x as i32
    }
}

/// Saturating add.
// Port of: include/private/SkSafe32.h#L20-L22 (chrome/m156)
#[doc(alias = "Sk32_sat_add")]
#[must_use]
pub const fn sat_add(a: i32, b: i32) -> i32 {
    pin_to_s32(a as i64 + b as i64)
}

/// Saturating subtract.
// Port of: include/private/SkSafe32.h#L24-L26 (chrome/m156)
#[doc(alias = "Sk32_sat_sub")]
#[must_use]
pub const fn sat_sub(a: i32, b: i32) -> i32 {
    pin_to_s32(a as i64 - b as i64)
}

/// Wrapping add.
// Port of: include/private/SkSafe32.h#L30-L32 (chrome/m156)
#[doc(alias = "Sk32_can_overflow_add")]
#[must_use]
pub const fn can_overflow_add(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}

/// Wrapping subtract.
// Port of: include/private/SkSafe32.h#L33-L35 (chrome/m156)
#[doc(alias = "Sk32_can_overflow_sub")]
#[must_use]
pub const fn can_overflow_sub(a: i32, b: i32) -> i32 {
    a.wrapping_sub(b)
}

/// A "safe" abs for 32-bit integers that asserts when undefined behavior would occur.
// Port of: include/private/SkSafe32.h#L41-L47 (chrome/m156)
#[doc(alias = "SkAbs32")]
#[must_use]
pub fn abs32(value: i32) -> i32 {
    debug_assert!(value != NAN32); // The most negative i32 can't be negated.
    if value < 0 { -value } else { value }
}
