// Copyright 2008 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkFloatBits.h

//! Helpers to see floats as bit patterns (`SkFloatBits.h`).

use crate::math::left_shift;

/// Converts a sign-bit int (a float interpreted as an int) into a 2s-complement int. This also
/// converts -0 (`0x80000000`) to 0, which allows floats to be compared with normal operators.
// Port of: src/core/SkFloatBits.h#L18-L24 (chrome/m156)
#[doc(alias = "SkSignBitTo2sCompliment")]
#[must_use]
pub fn sign_bit_to_2s_compliment(mut x: i32) -> i32 {
    if x < 0 {
        x &= 0x7FFF_FFFF;
        x = -x;
    }
    x
}

/// Converts a 2s-complement int to a sign-bit int; undoes [`sign_bit_to_2s_compliment`].
// Port of: src/core/SkFloatBits.h#L30-L40 (chrome/m156)
#[doc(alias = "Sk2sComplimentToSignBit")]
#[must_use]
pub fn compliment_2s_to_sign_bit(mut x: i32) -> i32 {
    let sign = x >> 31;
    // make x positive
    x = (x ^ sign).wrapping_sub(sign);
    // set the sign bit as needed
    x |= left_shift(sign, 31);
    x
}

/// Returns the float as its bit pattern.
// Port of: src/core/SkFloatBits.h#L43-L48 (chrome/m156)
#[doc(alias = "SkFloat2Bits")]
#[must_use]
pub fn float_to_bits(value: f32) -> u32 {
    value.to_bits()
}

/// Returns the bit pattern as a float.
// Port of: src/core/SkFloatBits.h#L51-L56 (chrome/m156)
#[doc(alias = "SkBits2Float")]
#[must_use]
pub fn bits_to_float(bits: u32) -> f32 {
    f32::from_bits(bits)
}

/// Returns the float as a 2s-complement int, only to be used to compare floats to each other or
/// against positive float-bit constants (like 0).
// Port of: src/core/SkFloatBits.h#L63-L65 (chrome/m156)
#[doc(alias = "SkFloatAs2sCompliment")]
#[allow(clippy::cast_possible_wrap)] // mirrors the (int32_t) cast
#[must_use]
pub fn float_as_2s_compliment(x: f32) -> i32 {
    sign_bit_to_2s_compliment(float_to_bits(x) as i32)
}

/// Returns the 2s-complement int as a float; undoes [`float_as_2s_compliment`].
// Port of: src/core/SkFloatBits.h#L71-L73 (chrome/m156)
#[doc(alias = "Sk2sComplimentAsFloat")]
#[allow(clippy::cast_sign_loss)] // mirrors the (uint32_t) cast
#[must_use]
pub fn compliment_2s_as_float(x: i32) -> f32 {
    bits_to_float(compliment_2s_to_sign_bit(x) as u32)
}
