// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkHalf.h, src/core/SkHalf.cpp

//! 16-bit floating point storage type and conversions to and from single precision.

use skia_rust_simd::vx::{Vec as VxVec, from_half, to_half};

/// 16-bit floating point value: 1 bit sign, 5 bits exponent, 10 bits mantissa. Only used for
/// storage.
// Port of: src/core/SkHalf.h#L16 (chrome/m156)
#[doc(alias = "SkHalf")]
pub type Half = u16;

/// A NaN value, not all possible NaN values.
// Port of: src/core/SkHalf.h#L18 (chrome/m156)
#[doc(alias = "SK_HalfNaN")]
pub const HALF_NAN: u16 = 0x7c01;
// Port of: src/core/SkHalf.h#L19 (chrome/m156)
#[doc(alias = "SK_HalfInfinity")]
pub const HALF_INFINITY: u16 = 0x7c00;
/// 2^-14 (minimum positive normal value).
// Port of: src/core/SkHalf.h#L20 (chrome/m156)
#[doc(alias = "SK_HalfMin")]
pub const HALF_MIN: u16 = 0x0400;
/// 65504 (maximum positive normal value).
// Port of: src/core/SkHalf.h#L21 (chrome/m156)
#[doc(alias = "SK_HalfMax")]
pub const HALF_MAX: u16 = 0x7bff;
/// 2^-10.
// Port of: src/core/SkHalf.h#L22 (chrome/m156)
#[doc(alias = "SK_HalfEpsilon")]
pub const HALF_EPSILON: u16 = 0x1400;
/// 1.
// Port of: src/core/SkHalf.h#L23 (chrome/m156)
#[doc(alias = "SK_Half1")]
pub const HALF_1: u16 = 0x3C00;

/// Converts a half to single precision.
// Port of: src/core/SkHalf.cpp#L24-L26 (chrome/m156)
#[doc(alias = "SkHalfToFloat")]
#[must_use]
pub fn half_to_float(h: Half) -> f32 {
    from_half(VxVec::<1, u16>::splat(h))[0]
}

/// Converts a float to half precision; unlike `skvx::to_half`, a float NaN becomes a half NaN.
// Port of: src/core/SkHalf.cpp#L16-L22 (chrome/m156)
#[doc(alias = "SkFloatToHalf")]
#[must_use]
pub fn float_to_half(f: f32) -> Half {
    if f.is_nan() {
        HALF_NAN
    } else {
        to_half(VxVec::<1, f32>::splat(f))[0]
    }
}
