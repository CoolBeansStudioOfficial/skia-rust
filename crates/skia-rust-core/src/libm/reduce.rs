// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Argument reduction by π/2, as the UCRT's trigonometric functions do it (see `docs/design/math.md`).
//!
//! Three reductions exist in the UCRT and each function picks its own thresholds:
//! - [`medium_f32`]: Cody–Waite with a 33-bit π/2 head, used by `sinf`/`cosf`/`tanf` for moderate
//!   arguments;
//! - [`medium_f64`]: three-part Cody–Waite with FMA, used by `sin`/`cos` below 2e7;
//! - [`payne_hanek_bits`]: an integer Payne–Hanek reduction against [`TWO_OVER_PI_BYTES`], finished
//!   either as a single double ([`large_f32`], inlined in `sinf`/`tanf`) or as a head/tail pair
//!   ([`large_f64`], the out-of-line routine `sin`/`cos`/`cosf` call).

use super::tables::TWO_OVER_PI_BYTES;

/// `2/π`, rounded to double.
pub(super) const TWO_OVER_PI: f64 = f64::from_bits(0x3fe4_5f30_6dc9_c883);
/// `π/2`, rounded to double.
pub(super) const PI_OVER_2: f64 = f64::from_bits(0x3ff9_21fb_5444_2d18);

/// Reads the little-endian `u64` at byte offset `at` of the 2/π table.
fn table_u64(at: u64) -> u64 {
    #[allow(clippy::cast_possible_truncation)] // the offset is at most 155 (see `payne_hanek_bits`)
    let at = at as usize;
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&TWO_OVER_PI_BYTES[at..at + 8]);
    u64::from_le_bytes(bytes)
}

/// Bit scan reverse (`bsr`): the index of the highest set bit. The instruction leaves its result
/// undefined for zero, which the reductions never pass in practice; 0 is returned then.
fn bsr(v: u64) -> u64 {
    if v == 0 { 0 } else { u64::from(v.ilog2()) }
}

/// Result of the integer part of the Payne–Hanek reduction.
pub(super) struct PayneHanek {
    /// The quadrant, `0..=3`.
    pub(super) region: u64,
    /// Bits of the reduced argument as a fraction of π/2 (sign, biased exponent and 52 mantissa bits).
    pub(super) head_bits: u64,
    /// The next 64 bits of the fraction, left-aligned below `head_bits`' mantissa.
    pub(super) r9: u64,
    /// The biased exponent of `head_bits`.
    pub(super) r11: u64,
    /// The sign bit of the reduced argument (`0` or `1 << 63`).
    pub(super) sign: u64,
}

/// The integer Payne–Hanek reduction shared by the UCRT's large-argument paths.
///
/// `abs_bits` are the bits of a positive double with an unbiased exponent of at least 23.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64), the shared integer reduction in `sinf`/`tanf`
// (inlined) and in the `sin`/`cos`/`cosf` large-argument helper
pub(super) fn payne_hanek_bits(abs_bits: u64) -> PayneHanek {
    let mut r11 = (abs_bits >> 52).wrapping_sub(0x3ff);
    let offset = 0x86u64.wrapping_sub(r11 >> 3);
    let t0 = table_u64(offset);
    let mantissa = ((abs_bits << 12) >> 12) | (1 << 52);
    let t1 = table_u64(offset + 8);
    let t2 = table_u64(offset + 16);

    let p = u128::from(t0) * u128::from(mantissa);
    #[allow(clippy::cast_possible_truncation)] // low/high halves of a 128-bit product
    let (mut r8, mut r10) = (p as u64, (p >> 64) as u64);
    let p = u128::from(t1) * u128::from(mantissa);
    #[allow(clippy::cast_possible_truncation)] // low/high halves of a 128-bit product
    let (lo, mut hi) = (p as u64, (p >> 64) as u64);
    r11 &= 7;
    let (sum, carry) = lo.overflowing_add(r10);
    hi = hi.wrapping_add(u64::from(carry));
    let mut r9 = sum;
    r10 = hi;
    r10 = r10.wrapping_add(t2.wrapping_mul(mantissa));

    // `shr rax, cl` leaves the last bit shifted out in CF; a set bit rounds the quadrant up and makes
    // the remainder negative (one's complement of the fraction).
    let cl = 0x36 - r11;
    let quadrant = r10 >> cl;
    let cf = (r10 >> (cl - 1)) & 1;
    let mut sign = 0u64;
    if cf == 1 {
        r10 = !r10;
        r9 = !r9;
        r8 = !r8;
        sign = 1 << 63;
    }
    let region = quadrant.wrapping_add(cf) & 3;

    let cl = r11 + 10;
    r10 = (r10 << cl) >> cl;
    r11 = cl.wrapping_sub(64);
    if r10 == 0 {
        r10 = r9;
        r9 = r8;
        r8 = 0;
        r11 = r11.wrapping_sub(64);
    }
    let top = bsr(r10);
    r11 = r11.wrapping_add(top);
    #[allow(clippy::cast_possible_wrap)] // `top` is at most 63
    let shift = top as i64 - 0x34;
    if shift < 0 {
        let n = shift.unsigned_abs();
        let rax = r9;
        r10 <<= n;
        r9 <<= n;
        let m = 64 - n;
        r10 |= rax >> m;
        r8 >>= m;
        r9 |= r8;
    } else if shift > 0 {
        let n = shift.unsigned_abs();
        r8 = r10;
        r10 >>= n;
        r9 >>= n;
        r8 <<= 64 - n;
        r9 |= r8;
    }
    r11 = r11.wrapping_add(0x3ff);
    r10 &= !(1 << 52);
    r10 |= sign;
    r10 |= r11 << 52;
    PayneHanek {
        region,
        head_bits: r10,
        r9,
        r11,
        sign,
    }
}

/// The large-argument reduction inlined in `sinf` and `tanf`: `(r, region)` with `r` a double.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `sinf`/`tanf`, FMA3 path, large-argument branch
pub(super) fn large_f32(abs_bits: u64) -> (f64, u64) {
    let ph = payne_hanek_bits(abs_bits);
    (f64::from_bits(ph.head_bits) * PI_OVER_2, ph.region)
}

/// The out-of-line large-argument reduction used by `sin`, `cos` and `cosf`: `(r, rr, region)`.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64), large-argument `rem_pio2` helper (FMA3)
pub(super) fn large_f64(abs_bits: u64) -> (f64, f64, u64) {
    const MASK_27: u64 = 0xffff_ffff_f800_0000;
    const PI_OVER_2_HEAD: f64 = f64::from_bits(0x3ff9_21fb_5000_0000);
    const PI_OVER_2_MID: f64 = f64::from_bits(0x3e51_10b4_6000_0000);
    const PI_OVER_2_TAIL: f64 = f64::from_bits(0x3c91_a626_3314_5c06);

    let ph = payne_hanek_bits(abs_bits);
    let head = f64::from_bits(ph.head_bits);
    let mut r9 = ph.r9;
    let top = bsr(r9);
    let rcx = 64 - top;
    r9 <<= rcx & 63;
    r9 >>= 12;
    let r11 = ph.r11.wrapping_sub(rcx + 0x34) << 52;
    r9 |= ph.sign;
    r9 |= r11;
    let tail = f64::from_bits(r9);

    let head_hi = f64::from_bits(head.to_bits() & MASK_27);
    let head_lo = head - head_hi;
    let r_hi = head * PI_OVER_2;
    let mut err = head_hi * PI_OVER_2_HEAD - r_hi;
    err = head_lo.mul_add(PI_OVER_2_HEAD, err);
    err = head_hi.mul_add(PI_OVER_2_MID, err);
    err = head_lo.mul_add(PI_OVER_2_MID, err);
    let t = head.mul_add(PI_OVER_2_TAIL, tail * PI_OVER_2);
    err += t;
    let r = r_hi + err;
    let rr = (r_hi - r) + err;
    (r, rr, ph.region)
}

/// The moderate-argument reduction of `sinf`, `cosf` and `tanf`: `(r, region)`.
///
/// `ax` is `|x|` as a double, below 2^31·π/2.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `sinf`/`cosf`/`tanf`, FMA3 path, medium branch
pub(super) fn medium_f32(ax: f64) -> (f64, u64) {
    const PI_OVER_2_1: f64 = f64::from_bits(0x3ff9_21fb_5440_0000);
    const PI_OVER_2_1_TAIL: f64 = f64::from_bits(0x3dd0_b461_1a62_6331);
    // `vcvttpd2dq` truncates; the argument is positive and below 2^31.
    #[allow(clippy::cast_possible_truncation)] // mirrors vcvttpd2dq
    let n = ax.mul_add(TWO_OVER_PI, 0.5) as i32;
    let region = u64::from(n.cast_unsigned() & 3);
    let nd = f64::from(n);
    let rhead = (-nd).mul_add(PI_OVER_2_1, ax);
    let rtail = nd * PI_OVER_2_1_TAIL;
    (rhead - rtail, region)
}

/// The moderate-argument reduction of `sin` and `cos`: `(r, rr, region)` for `|x|` below 2e7.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64), medium `rem_pio2` helper (FMA3)
pub(super) fn medium_f64(ax: f64) -> (f64, f64, u64) {
    const SHIFTER: f64 = f64::from_bits(0x4338_0000_0000_0000);
    const PI_OVER_2_2: f64 = f64::from_bits(0x3c91_a626_3314_5c00);
    const PI_OVER_2_3: f64 = f64::from_bits(0x397b_839a_2520_49c0);
    let n = ax.mul_add(TWO_OVER_PI, SHIFTER) - SHIFTER;
    #[allow(clippy::cast_possible_truncation)] // mirrors vcvttpd2dq on an integral value below 2^31
    let region = u64::from((n as i32).cast_unsigned() & 3);
    let rhead = (-n).mul_add(PI_OVER_2, ax);
    let prod = n * PI_OVER_2_2;
    let prod_err = n.mul_add(PI_OVER_2_2, -prod);
    let t = rhead - prod;
    let t_err = (rhead - t) - prod;
    let r = (-n).mul_add(PI_OVER_2_2, rhead);
    let rr = ((t - r) + t_err) - prod_err;
    let rr = (-n).mul_add(PI_OVER_2_3, rr);
    (r, rr, region)
}
