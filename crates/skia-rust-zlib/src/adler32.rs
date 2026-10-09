// Copyright 1995-2023 Mark Adler (zlib); port by The skia-rust Authors.
// Use of this source code is governed by the zlib licence in the LICENSE file.
// Port of: adler32.c#L1-L192 (chromium zlib@646b7f56), the portable path (the SIMD variants
// compute the same integer function).

//! The Adler-32 checksum of RFC 1950.

/// Port of `BASE`: the largest prime below 2^16.
const BASE: u32 = 65521;
/// Port of `NMAX`: the largest `n` such that 255n(n+1)/2 + (n+1)(BASE-1) fits in 32 bits.
const NMAX: usize = 5552;

/// The value zlib's `adler32(0L, Z_NULL, 0)` returns: the checksum of the empty input.
pub const ADLER32_INIT: u32 = 1;

/// Port of `adler32_z`: updates the checksum `adler` with `buf`.
///
/// A plain sum over the bytes with the two running sums reduced modulo `BASE` after every
/// `NMAX` bytes, which is exact because the sums cannot overflow in that window.
// Port of: adler32.c#L120-L160 (adler32_z, scalar path)
#[must_use]
pub fn adler32(adler: u32, buf: &[u8]) -> u32 {
    let mut sum1 = adler & 0xffff;
    let mut sum2 = (adler >> 16) & 0xffff;
    for chunk in buf.chunks(NMAX) {
        for &byte in chunk {
            sum1 += u32::from(byte);
            sum2 += sum1;
        }
        sum1 %= BASE;
        sum2 %= BASE;
    }
    (sum2 << 16) | sum1
}
