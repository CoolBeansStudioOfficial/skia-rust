// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The packing helpers of `SkBitmapProcState` that Skia's own tests call through the `sktests`
//! namespace: `pack_clamp`, `pack_repeat` and `pack_mirror` (`src/core/SkBitmapProcState.h`,
//! `src/core/SkBitmapProcState_matrixProcs.cpp`), and the unpacking helper from
//! `src/opts/SkBitmapProcState_opts.h`.
//!
//! Each coordinate is packed into 32 bits: 14 bits of the low integer coordinate, 4 bits of the
//! linear weight between the two integers, and 14 bits of the high integer coordinate.

use crate::fixed::{FIXED_1, Fixed};

// Port of: src/core/SkBitmapProcState_matrixProcs.cpp#L157-L161 (chrome/m156)
// used when both tilex and tiley are clamp
// Extract the high four fractional bits from fx, the lerp parameter when filtering.
#[allow(clippy::cast_sign_loss)] // the masked value is never negative, as in the C++ unsigned
fn extract_low_bits_clamp_clamp(fx: Fixed, _max: i32) -> u32 {
    // If we're already scaled up to by max like clamp/decal,
    // just grab the high four fractional bits.
    ((fx >> 12) & 0xf) as u32
}

// Port of: src/core/SkBitmapProcState_matrixProcs.cpp#L163-L168 (chrome/m156)
// used when one of tilex and tiley is not clamp
fn extract_low_bits_general(fx: Fixed, max: i32) -> u32 {
    // In repeat or mirror fx is in [0,1], so scale up by max first.
    extract_low_bits_clamp_clamp((fx & 0xffff).wrapping_mul(max + 1), max)
}

// Port of: src/core/SkBitmapProcState_matrixProcs.cpp#L270-L272 (chrome/m156)
// Helper to ensure that when we shift down, we do it w/o sign-extension
// so the caller doesn't have to manually mask off the top 16 bits.
fn sk_ushift16(x: u32) -> u32 {
    x >> 16
}

// Port of: src/core/SkBitmapProcState_matrixProcs.cpp#L274-L277 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // `max` is never negative, as in the C++ unsigned arithmetic
fn repeat(fx: Fixed, max: i32) -> u32 {
    sk_ushift16(((fx & 0xFFFF) as u32).wrapping_mul((max + 1) as u32))
}

// Port of: src/core/SkBitmapProcState_matrixProcs.cpp#L278-L285 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // the product is never negative, as in the C++ unsigned shift
fn mirror(fx: Fixed, max: i32) -> u32 {
    // s is 0xFFFFFFFF if we're on an odd interval, or 0 if an even interval
    // (SkLeftShift is a shift on the unsigned bits, then reinterpreted as signed).
    let s: Fixed = ((fx as u32) << 15).cast_signed() >> 31;

    // This should be exactly the same as repeat(fx ^ s, max) from here on.
    sk_ushift16((((fx ^ s) & 0xFFFF).wrapping_mul(max + 1)) as u32)
}

// Port of: src/core/SkBitmapProcState_matrixProcs.cpp#L287-L289 (chrome/m156)
fn clamp(fx: Fixed, max: i32) -> u32 {
    // SkTPin(fx >> 16, 0, max) is max(0, min(x, max)).
    let pinned = (fx >> 16).min(max).max(0);
    #[allow(clippy::cast_sign_loss)] // pinned is never negative
    {
        pinned as u32
    }
}

// Port of: src/core/SkBitmapProcState_matrixProcs.cpp#L186-L191 (chrome/m156)
// Takes a SkFixed number and packs it into a 32bit integer in the following schema:
// 14 bits to represent the low integer value (n)
// 4 bits to represent a linear distance between low and high (floored to nearest 1/16)
// 14 bits to represent the high integer value (n+1)
// If f is less than 0, then both integers will be 0. If f is greater than or equal to max, both
// integers will be that max value. In all cases, the middle 4 bits will represent the fractional
// part (to a resolution of 1/16). If the two integers are equal, doing any linear interpolation
// will result in the same integer, so the fractional part does not matter.
fn pack(
    f: Fixed,
    max: u32,
    one: Fixed,
    tile: fn(Fixed, i32) -> u32,
    extract_low_bits: fn(Fixed, i32) -> u32,
) -> u32 {
    let max = i32::try_from(max).expect("max fits in an int");
    let mut packed = tile(f, max); // low coordinate in high bits
    packed = (packed << 4) | extract_low_bits(f, max); // (lerp weight _is_ coord fractional part)
    // f + one wraps on overflow, as the C++ (SK_NO_SANITIZE("signed-integer-overflow")) does.
    packed = (packed << 14) | tile(f.wrapping_add(one), max); // high coordinate in low bits
    packed
}

// Port of: src/core/SkBitmapProcState_matrixProcs.cpp#L554-L557 (chrome/m156)
#[doc(alias = "sktests::pack_clamp")]
#[must_use]
pub fn pack_clamp(f: Fixed, max: u32) -> u32 {
    // Based on ClampX_ClampY_Procs[1] (filter_scale)
    pack(f, max, FIXED_1, clamp, extract_low_bits_clamp_clamp)
}

// Port of: src/core/SkBitmapProcState_matrixProcs.cpp#L559-L562 (chrome/m156)
// `SK_Fixed1 / width` is computed in size_t and then narrowed to SkFixed, as the C++ does.
#[doc(alias = "sktests::pack_repeat")]
#[must_use]
pub fn pack_repeat(f: Fixed, max: u32, width: usize) -> u32 {
    // Based on RepeatX_RepeatY_Procs[1] (filter_scale)
    let one = fixed_one_over_width(width);
    pack(f, max, one, repeat, extract_low_bits_general)
}

// Port of: src/core/SkBitmapProcState_matrixProcs.cpp#L564-L567 (chrome/m156)
// As above, but width is the width of the pretend bitmap.
#[doc(alias = "sktests::pack_mirror")]
#[must_use]
pub fn pack_mirror(f: Fixed, max: u32, width: usize) -> u32 {
    // Based on MirrorX_MirrorY_Procs[1] (filter_scale)
    let one = fixed_one_over_width(width);
    pack(f, max, one, mirror, extract_low_bits_general)
}

/// `SK_Fixed1 / width`, computed in `size_t` and narrowed to `SkFixed`.
fn fixed_one_over_width(width: usize) -> Fixed {
    let fixed_1 = usize::try_from(FIXED_1).expect("SK_Fixed1 is positive");
    // The C++ narrows the size_t quotient to SkFixed (an int32), keeping the low 32 bits.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    // mirrors the narrowing of the size_t quotient
    {
        (fixed_1 / width) as Fixed
    }
}

// Port of: src/opts/SkBitmapProcState_opts.h#L37-L43 (chrome/m156)
// This same basic packing scheme is used throughout the file. Unpacks the low coordinate, the
// high coordinate and the lerp weight of a packed value.
pub fn decode_packed_coordinates_and_weight(packed: u32, v0: &mut u32, v1: &mut u32, w: &mut u32) {
    *v0 = packed >> 18; // Integer coordinate x0 or y0.
    *v1 = packed & 0x3fff; // Integer coordinate x1 or y1.
    *w = (packed >> 14) & 0xf; // Lerp weight for v1; weight for v0 is 16-w.
}
