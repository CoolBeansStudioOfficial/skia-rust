// Copyright 2012 Google Inc.
// Copyright 2008 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkMathPriv.h, src/core/SkMathPriv.cpp

//! Private math helpers (`SkMathPriv.h`), exposed for Skia's own tests.

use std::ops::{Div, Rem};

use crate::math::U8CPU;

/// Returns the number of leading zero bits.
// Port of: src/core/SkMathPriv.h#L22-L24 (chrome/m156)
#[doc(alias = "SkCLZ")]
#[allow(clippy::cast_possible_wrap)] // result is at most 32
#[must_use]
pub const fn clz(x: u32) -> i32 {
    x.leading_zeros() as i32
}

/// Returns the number of trailing zero bits.
// Port of: src/core/SkMathPriv.h#L29-L31 (chrome/m156)
#[doc(alias = "SkCTZ")]
#[allow(clippy::cast_possible_wrap)] // result is at most 32
#[must_use]
pub const fn ctz(x: u32) -> i32 {
    x.trailing_zeros() as i32
}

/// Returns the number of set bits (the population count) in the provided `u32`.
// Port of: src/core/SkMathPriv.h#L36-L38 (chrome/m156)
#[doc(alias = "SkPopCount")]
#[allow(clippy::cast_possible_wrap)] // result is at most 32
#[must_use]
pub const fn pop_count(x: u32) -> i32 {
    x.count_ones() as i32
}

/// Returns the integer square root of `x`, with a bias of `count` bits.
///
/// www.worldserver.com/turk/computergraphics/FixedSqrt.pdf
// Port of: src/core/SkMathPriv.cpp#L18-L38 (chrome/m156)
#[doc(alias = "SkSqrtBits")]
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // mirrors the C++ uint32_t/int32_t casts
#[must_use]
pub fn sqrt_bits(x: i32, count: i32) -> i32 {
    debug_assert!(x >= 0 && count > 0 && (count as u32) <= 30);
    let mut count = count;

    let mut root: u32 = 0;
    let mut rem_hi: u32 = 0;
    let mut rem_lo: u32 = x as u32;

    loop {
        root <<= 1;

        rem_hi = (rem_hi << 2) | (rem_lo >> 30);
        rem_lo <<= 2;

        let test_div = (root << 1) + 1;
        if rem_hi >= test_div {
            rem_hi -= test_div;
            root += 1;
        }
        count -= 1;
        if count < 0 {
            break;
        }
    }

    root as i32
}

/// Returns the integer square root of `n`, treated as a `Fixed` (16.16).
// Port of: src/core/SkMathPriv.h#L47 (chrome/m156)
#[doc(alias = "SkSqrt32")]
#[must_use]
pub fn sqrt32(n: i32) -> i32 {
    sqrt_bits(n, 15)
}

/// Returns `(value < 0 ? 0 : value)` efficiently (i.e. no compares or branches).
// Port of: src/core/SkMathPriv.h#L52-L54 (chrome/m156)
#[doc(alias = "SkClampPos")]
#[must_use]
pub fn clamp_pos(value: i32) -> i32 {
    value & !(value >> 31)
}

/// Returns `(numer / denom, numer % denom)`.
// Port of: src/core/SkMathPriv.h#L59-L63 (chrome/m156)
#[doc(alias = "SkTDivMod")]
#[must_use]
pub fn t_div_mod<T>(numer: T, denom: T) -> (T, T)
where
    T: Copy + Div<Output = T> + Rem<Output = T>,
{
    (numer / denom, numer % denom)
}

/// Returns -1 if `n < 0`, else returns 0.
// Port of: src/core/SkMathPriv.h#L67 (chrome/m156)
#[doc(alias = "SkExtractSign")]
#[must_use]
pub fn extract_sign(n: i32) -> i32 {
    n >> 31
}

/// If `sign == -1`, returns `-n`, else `sign` must be 0, and returns `n`.
/// Typically used in conjunction with [`extract_sign`].
// Port of: src/core/SkMathPriv.h#L72-L75 (chrome/m156)
#[doc(alias = "SkApplySign")]
#[must_use]
pub fn apply_sign(n: i32, sign: i32) -> i32 {
    debug_assert!(sign == 0 || sign == -1);
    (n ^ sign).wrapping_sub(sign)
}

/// Returns `x` with the sign of `y`.
// Port of: src/core/SkMathPriv.h#L78-L80 (chrome/m156)
#[doc(alias = "SkCopySign32")]
#[must_use]
pub fn copy_sign32(x: i32, y: i32) -> i32 {
    apply_sign(x, extract_sign(x ^ y))
}

/// Given a positive value and a positive max, returns the value pinned against max.
/// Note: only works as long as `max - value` doesn't wrap around.
// Port of: src/core/SkMathPriv.h#L87-L92 (chrome/m156)
#[doc(alias = "SkClampUMax")]
#[must_use]
pub fn clamp_u_max(value: u32, max: u32) -> u32 {
    if value > max { max } else { value }
}

/// Negates an `i32` through `usize` (sign-extending first), avoiding UB on `i32::MIN`.
// Port of: src/core/SkMathPriv.h#L98-L108 (chrome/m156)
#[doc(alias = "sk_negate_to_size_t")]
#[allow(clippy::cast_sign_loss)] // mirrors static_cast<size_t>(int32_t)
#[must_use]
pub fn negate_to_size_t(value: i32) -> usize {
    (value as usize).wrapping_neg()
}

/// Returns `a*b/255`, truncating away any fractional bits. Only valid if both `a` and `b` are
/// `0..=255`.
// Port of: src/core/SkMathPriv.h#L114-L119 (chrome/m156)
#[doc(alias = "SkMulDiv255Trunc")]
#[must_use]
pub fn mul_div_255_trunc(a: U8CPU, b: U8CPU) -> U8CPU {
    debug_assert!(u8::try_from(a).is_ok());
    debug_assert!(u8::try_from(b).is_ok());
    let prod = a * b + 1;
    (prod + (prod >> 8)) >> 8
}

/// Returns `(a*b)/255`, taking the ceiling of any fractional bits. Only valid if both `a` and `b`
/// are `0..=255`. The expected result equals `(a * b + 254) / 255`.
// Port of: src/core/SkMathPriv.h#L124-L129 (chrome/m156)
#[doc(alias = "SkMulDiv255Ceiling")]
#[must_use]
pub fn mul_div_255_ceiling(a: U8CPU, b: U8CPU) -> U8CPU {
    debug_assert!(u8::try_from(a).is_ok());
    debug_assert!(u8::try_from(b).is_ok());
    let prod = a * b + 255;
    (prod + (prod >> 8)) >> 8
}

/// Just the rounding step in `SkMulDiv255Round`: `round(value / 255)`.
// Port of: src/core/SkMathPriv.h#L133-L136 (chrome/m156)
#[doc(alias = "SkDiv255Round")]
#[must_use]
pub fn div_255_round(prod: u32) -> u32 {
    let prod = prod + 128;
    (prod + (prod >> 8)) >> 8
}

/// Swaps byte order of a 4-byte value, e.g. `0xaarrggbb -> 0xbbggrraa`.
// Port of: src/core/SkMathPriv.h#L142-L146 (chrome/m156)
#[doc(alias = "SkBSwap32")]
#[must_use]
pub const fn bswap32(v: u32) -> u32 {
    v.swap_bytes()
}

/// Returns the log2 of the specified value, were that value to be rounded up to the next power
/// of 2. It is undefined to pass 0.
// Port of: src/core/SkMathPriv.h#L157-L160 (chrome/m156)
#[doc(alias = "SkNextLog2")]
#[must_use]
pub fn next_log2(value: u32) -> i32 {
    debug_assert!(value != 0);
    32 - clz(value.wrapping_sub(1))
}

/// Returns the log2 of the specified value, were that value to be rounded down to the previous
/// power of 2. It is undefined to pass 0.
// Port of: src/core/SkMathPriv.h#L171-L174 (chrome/m156)
#[doc(alias = "SkPrevLog2")]
#[must_use]
pub fn prev_log2(value: u32) -> i32 {
    debug_assert!(value != 0);
    32 - clz(value >> 1)
}

/// Returns the smallest power-of-2 that is `>=` the specified value. If value is already a power
/// of 2, then it is returned unchanged. It is undefined if value is `<= 0`.
// Port of: src/core/SkMathPriv.h#L181-L184 (chrome/m156)
#[doc(alias = "SkNextPow2")]
#[allow(clippy::cast_sign_loss)] // mirrors static_cast<uint32_t>; value is positive
#[must_use]
pub fn next_pow2(value: i32) -> i32 {
    debug_assert!(value > 0);
    1 << next_log2(value as u32)
}

/// Returns the largest power-of-2 that is `<=` the specified value. If value is already a power
/// of 2, then it is returned unchanged. It is undefined if value is `<= 0`.
// Port of: src/core/SkMathPriv.h#L191-L194 (chrome/m156)
#[doc(alias = "SkPrevPow2")]
#[allow(clippy::cast_sign_loss)] // mirrors static_cast<uint32_t>; value is positive
#[must_use]
pub fn prev_pow2(value: i32) -> i32 {
    debug_assert!(value > 0);
    1 << prev_log2(value as u32)
}

/// Returns the next power of 2 `>= n` or `n` if the next power of 2 can't be represented by
/// `usize`.
// Port of: src/core/SkMathPriv.h#L201-L219 (chrome/m156)
#[doc(alias = "SkNextSizePow2")]
#[must_use]
pub fn next_size_pow2(n: usize) -> usize {
    const NUM_SIZE_T_BITS: u32 = usize::BITS;
    const HIGH_BIT_SET: usize = 1usize << (NUM_SIZE_T_BITS - 1);

    let mut n = n;
    if n == 0 {
        return 1;
    } else if n >= HIGH_BIT_SET {
        return n;
    }

    n -= 1;
    let mut shift: u32 = 1;
    while shift < NUM_SIZE_T_BITS {
        n |= n >> shift;
        shift <<= 1;
    }
    n + 1
}

/// Conservative check: returns false for very large values that "could" fit.
///
/// Skia's `SkFitsInFixed<T>` is generic over `SkTAbs`; here `T` is any type that widens to `f64`
/// exactly (`f32`, `i32`, ...), which gives the same answers for those types.
// Port of: src/core/SkMathPriv.h#L221-L223 (chrome/m156)
#[doc(alias = "SkFitsInFixed")]
#[must_use]
pub fn fits_in_fixed<T: Into<f64>>(x: T) -> bool {
    x.into().abs() <= f64::from(32767.0f32)
}
