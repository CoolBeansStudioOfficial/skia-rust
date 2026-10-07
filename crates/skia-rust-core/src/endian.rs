// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkEndian.h

//! Byte-order swaps (the swap helpers of `SkEndian.h`).

/// Reverses the 2 bytes of a 16-bit value, e.g. `0x1122 -> 0x2211`.
// Port of: src/core/SkEndian.h#L33-L35 (chrome/m156)
#[doc(alias = "SkEndianSwap16")]
#[must_use]
pub const fn endian_swap16(value: u16) -> u16 {
    (value >> 8) | ((value & 0xFF) << 8)
}

/// Compile-time version of [`endian_swap16`].
// Port of: src/core/SkEndian.h#L37-L39 (chrome/m156)
#[doc(alias = "SkTEndianSwap16")]
#[derive(Debug)]
pub struct TEndianSwap16<const N: u16>;
impl<const N: u16> TEndianSwap16<N> {
    /// `SkTEndianSwap16<N>::value`.
    pub const VALUE: u16 = endian_swap16(N);
}

/// Reverses the bytes of every value in `array`.
// Port of: src/core/SkEndian.h#L44-L51 (chrome/m156)
#[doc(alias = "SkEndianSwap16s")]
pub fn endian_swap16s(array: &mut [u16]) {
    for v in array {
        *v = endian_swap16(*v);
    }
}

/// Reverses the 4 bytes of a 32-bit value, e.g. `0x12345678 -> 0x78563412`.
// Port of: src/core/SkEndian.h#L56-L61 (chrome/m156)
#[doc(alias = "SkEndianSwap32")]
#[must_use]
pub const fn endian_swap32(value: u32) -> u32 {
    ((value & 0xFF) << 24) | ((value & 0xFF00) << 8) | ((value & 0x00FF_0000) >> 8) | (value >> 24)
}

/// Compile-time version of [`endian_swap32`].
// Port of: src/core/SkEndian.h#L63-L68 (chrome/m156)
#[doc(alias = "SkTEndianSwap32")]
#[derive(Debug)]
pub struct TEndianSwap32<const N: u32>;
impl<const N: u32> TEndianSwap32<N> {
    /// `SkTEndianSwap32<N>::value`.
    pub const VALUE: u32 = endian_swap32(N);
}

/// Reverses the bytes of every value in `array`.
// Port of: src/core/SkEndian.h#L73-L80 (chrome/m156)
#[doc(alias = "SkEndianSwap32s")]
pub fn endian_swap32s(array: &mut [u32]) {
    for v in array {
        *v = endian_swap32(*v);
    }
}

/// Reverses the 8 bytes of a 64-bit value, e.g. `0x1122334455667788 -> 0x8877665544332211`.
// Port of: src/core/SkEndian.h#L85-L93 (chrome/m156)
#[doc(alias = "SkEndianSwap64")]
#[must_use]
pub const fn endian_swap64(value: u64) -> u64 {
    ((value & 0x0000_0000_0000_00FF) << (8 * 7))
        | ((value & 0x0000_0000_0000_FF00) << (8 * 5))
        | ((value & 0x0000_0000_00FF_0000) << (8 * 3))
        | ((value & 0x0000_0000_FF00_0000) << 8)
        | ((value & 0x0000_00FF_0000_0000) >> 8)
        | ((value & 0x0000_FF00_0000_0000) >> (8 * 3))
        | ((value & 0x00FF_0000_0000_0000) >> (8 * 5))
        | ((value & 0xFF00_0000_0000_0000) >> (8 * 7))
}

/// Compile-time version of [`endian_swap64`].
// Port of: src/core/SkEndian.h#L95-L104 (chrome/m156)
#[doc(alias = "SkTEndianSwap64")]
#[derive(Debug)]
pub struct TEndianSwap64<const N: u64>;
impl<const N: u64> TEndianSwap64<N> {
    /// `SkTEndianSwap64<N>::value`.
    pub const VALUE: u64 = endian_swap64(N);
}

/// Reverses the bytes of every value in `array`.
// Port of: src/core/SkEndian.h#L109-L116 (chrome/m156)
#[doc(alias = "SkEndianSwap64s")]
pub fn endian_swap64s(array: &mut [u64]) {
    for v in array {
        *v = endian_swap64(*v);
    }
}
