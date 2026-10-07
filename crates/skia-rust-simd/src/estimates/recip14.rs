// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Software model of AVX-512's `vrcp14ps`/`vrsqrt14ps` (design §1.4, §6 R2).
//!
//! The algorithm is the one of Intel's "Reference Implementations for IA Approximation
//! Instructions VRCP14, VRSQRT14, VRCP28, VRSQRT28, and VEXP2" (`RECIP14.c`, functions
//! `RCP14S`/`RSQRT14S`): piecewise-linear interpolation on 64 (`rcp14`) or 2 × 32 (`rsqrt14`)
//! segments, evaluated exactly in `f64` and truncated to a 17-bit significand.
//!
//! The coefficients were **derived from the oracle host** (AMD Zen 4): for every segment, the
//! only integer pair that reproduces all of the host's outputs on that segment. They equal the
//! coefficient tables of Intel's reference code. The model as a whole (including special values,
//! denormal inputs and results) reproduces the host bit for bit on all 2³² inputs
//! (`exhaustive_amd_zen4_vs_host`).

/// `RCP14_Coeff`: `[2i]` is the slope `b`, `[2i + 1]` the value `a` at the centre of segment `i`
/// (`1 + 1/128 + i/64`), both in units of 2⁻¹⁸ (the slope pre-scaled by 2⁸).
#[rustfmt::skip]
pub const RCP14_COEFF: [u32; 128] = [
    1009, 260_119, 977, 256_148, 949, 252_296, 921, 248_558,
    893, 244_929, 869, 241_405, 843, 237_981, 821, 234_652,
    797, 231_416, 777, 228_266, 755, 225_202, 735, 222_220,
    717, 219_314, 699, 216_485, 681, 213_727, 663, 211_038,
    647, 208_417, 631, 205_859, 617, 203_364, 601, 200_929,
    587, 198_551, 573, 196_229, 561, 193_960, 547, 191_743,
    535, 189_576, 523, 187_458, 513, 185_387, 501, 183_360,
    491, 181_377, 479, 179_439, 469, 177_540, 459, 175_681,
    451, 173_860, 441, 172_077, 433, 170_330, 423, 168_618,
    415, 166_940, 407, 165_295, 399, 163_682, 391, 162_101,
    385, 160_550, 377, 159_027, 369, 157_535, 363, 156_069,
    357, 154_631, 349, 153_219, 343, 151_832, 337, 150_470,
    331, 149_133, 325, 147_819, 319, 146_528, 315, 145_260,
    309, 144_012, 303, 142_787, 299, 141_582, 293, 140_397,
    289, 139_232, 285, 138_085, 279, 136_959, 275, 135_853,
    271, 134_763, 267, 133_689, 263, 132_631, 259, 131_589,
];

/// `RSQRT14_Coeff`: for segment `i`, `[4i]`/`[4i + 1]` are the slope `d` and value `c` on
/// `[1,2)` (centre `1 + 1/64 + i/32`), `[4i + 2]`/`[4i + 3]` on `[2,4)` (centre
/// `2 + 1/32 + i/16`); values in units of 2⁻¹⁹.
#[rustfmt::skip]
pub const RSQRT14_COEFF: [u32; 128] = [
    1001, 520_261, 707, 367_881, 955, 512_437, 675, 362_349,
    915, 504_953, 647, 357_056, 877, 497_790, 619, 351_992,
    841, 490_922, 595, 347_136, 807, 484_331, 571, 342_475,
    775, 478_001, 549, 337_997, 747, 471_909, 527, 333_693,
    719, 466_046, 509, 329_545, 693, 460_397, 491, 325_551,
    669, 454_947, 473, 321_697, 647, 449_688, 457, 317_977,
    625, 444_606, 441, 314_385, 603, 439_694, 427, 310_910,
    585, 434_939, 413, 307_549, 567, 430_335, 401, 304_295,
    549, 425_875, 389, 301_139, 533, 421_551, 377, 298_079,
    517, 417_355, 365, 295_115, 501, 413_284, 355, 292_237,
    487, 409_329, 345, 289_439, 473, 405_487, 335, 286_722,
    461, 401_748, 325, 284_080, 449, 398_111, 317, 281_508,
    437, 394_571, 309, 279_006, 425, 391_127, 301, 276_569,
    415, 387_770, 293, 274_195, 403, 384_498, 285, 271_882,
    393, 381_307, 279, 269_625, 385, 378_194, 271, 267_425,
    375, 375_155, 265, 265_276, 367, 372_190, 259, 263_178,
];

const SIGN: u32 = 0x8000_0000;
const EXP_MASK: u32 = 0x7f80_0000;
const MANT_MASK: u32 = 0x007f_ffff;
const QNAN_BIT: u32 = 0x0040_0000;
const ONE: u32 = 0x3f80_0000;
const TWO: u32 = 0x4000_0000;
/// The x86 "real indefinite" `QNaN`, returned for negative `rsqrt14` inputs.
const DEFAULT_NAN: u32 = 0xffc0_0000;

/// Truncates a positive `f64` to 16 fraction bits (`FP64_CLEAR_LOW_36_BITS`) and divides it by
/// `2^k` by decrementing its exponent; the result is exact in `f32`.
fn truncate_scale(d: f64, k: u64) -> u32 {
    let u = (d.to_bits() & !((1u64 << 36) - 1)) - (k << 52);
    #[allow(clippy::cast_possible_truncation)] // exact: 17 significant bits, normal range
    let f = f64::from_bits(u) as f32;
    f.to_bits()
}

/// `rcp14` of `1.m` for `m` in `1..2²³` (the interpolation; result in `(0.5, 1)`).
#[allow(clippy::many_single_char_names)] // the reference code's names (a, b, x, y, ...)
fn rcp14_mantissa(m: u32) -> u32 {
    let i = (m >> 17) as usize; // floor((x - 1) * 64)
    let b = RCP14_COEFF[2 * i];
    let a = RCP14_COEFF[2 * i + 1];
    #[allow(clippy::cast_precision_loss)] // i < 64
    let y = 1.0 + 1.0 / 128.0 + i as f32 / 64.0; // exact
    let x = f32::from_bits((ONE | m) & !0x7f); // 17-bit significand
    let xmy = x - y; // exact
    let d = f64::from(a) - 256.0 * f64::from(b) * f64::from(xmy); // exact
    truncate_scale(d, 18)
}

/// `rsqrt14` of `1.m` (`odd == false`, `m` in `1..2²³`) or of `2·1.m` (`odd == true`).
#[allow(clippy::many_single_char_names)] // the reference code's names (c, d, x, y, ...)
fn rsqrt14_mantissa(m: u32, odd: bool) -> u32 {
    let i = (m >> 18) as usize; // floor((x - 1) * 32) or floor((x - 2) * 16)
    let (d, c, x, y, scale) = if odd {
        #[allow(clippy::cast_precision_loss)] // i < 32
        let y = 2.0 + 1.0 / 32.0 + i as f32 / 16.0;
        (
            RSQRT14_COEFF[4 * i + 2],
            RSQRT14_COEFF[4 * i + 3],
            f32::from_bits((TWO | m) & !0xff),
            y,
            128.0,
        )
    } else {
        #[allow(clippy::cast_precision_loss)] // i < 32
        let y = 1.0 + 1.0 / 64.0 + i as f32 / 32.0;
        (
            RSQRT14_COEFF[4 * i],
            RSQRT14_COEFF[4 * i + 1],
            f32::from_bits((ONE | m) & !0xff),
            y,
            256.0,
        )
    };
    let xmy = x - y; // exact
    let r = f64::from(c) - scale * f64::from(d) * f64::from(xmy); // exact
    truncate_scale(r, 19)
}

/// Splits a finite nonzero magnitude into `(E, m)` with `|x| = 1.m · 2ᴱ` (denormals normalized).
fn normalize(abs: u32) -> (i32, u32) {
    let exp = abs >> 23;
    if exp == 0 {
        let shift = abs.leading_zeros() - 8; // 1..=23
        #[allow(clippy::cast_possible_wrap)] // shift <= 23
        let e = -126 - shift as i32;
        (e, (abs << shift) & MANT_MASK)
    } else {
        #[allow(clippy::cast_possible_wrap)] // 8-bit exponent field
        let e = exp as i32 - 127;
        (e, abs & MANT_MASK)
    }
}

/// `vrcp14ps`/`vrcp14ss` on one lane.
#[must_use]
pub fn rcp14(x: f32) -> f32 {
    let bits = x.to_bits();
    let sign = bits & SIGN;
    let abs = bits & !SIGN;
    if abs >= EXP_MASK {
        // NaN: quieted; ±∞ → ±0.
        return f32::from_bits(if abs > EXP_MASK {
            bits | QNAN_BIT
        } else {
            sign
        });
    }
    if abs == 0 {
        return f32::from_bits(sign | EXP_MASK);
    }
    let (e, m) = normalize(abs);
    let r = if m == 0 { ONE } else { rcp14_mantissa(m) };
    #[allow(clippy::cast_possible_wrap)] // 8-bit exponent field
    let re = (r >> 23) as i32 - e;
    let out = if re >= 0xff {
        sign | EXP_MASK
    } else if re >= 1 {
        #[allow(clippy::cast_sign_loss)] // 1..=254
        let re = re as u32;
        sign | re << 23 | (r & MANT_MASK)
    } else {
        // Denormal result: the significand shifted right (exact: its low 7 bits are zero).
        #[allow(clippy::cast_sign_loss)] // re <= 0
        let shift = (1 - re) as u32;
        sign | ((r & MANT_MASK) | 0x0080_0000) >> shift
    };
    f32::from_bits(out)
}

/// `vrsqrt14ps`/`vrsqrt14ss` on one lane.
#[must_use]
#[allow(clippy::many_single_char_names)] // sign/exponent/mantissa names as in `rcp14`
pub fn rsqrt14(x: f32) -> f32 {
    let bits = x.to_bits();
    let sign = bits & SIGN;
    let abs = bits & !SIGN;
    let out = if abs > EXP_MASK {
        bits | QNAN_BIT
    } else if abs == 0 {
        sign | EXP_MASK
    } else if sign != 0 {
        DEFAULT_NAN
    } else if abs == EXP_MASK {
        0
    } else {
        let (e, m) = normalize(abs);
        let odd = e & 1 != 0;
        let r = if m == 0 && !odd {
            ONE
        } else {
            rsqrt14_mantissa(m, odd)
        };
        let n = (e - i32::from(odd)) / 2;
        #[allow(clippy::cast_sign_loss)] // two's-complement exponent arithmetic
        let delta = (n << 23) as u32;
        r.wrapping_sub(delta)
    };
    f32::from_bits(out)
}
