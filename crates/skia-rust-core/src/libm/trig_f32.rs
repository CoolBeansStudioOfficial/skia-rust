// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `sinf`, `cosf` and `tanf` as the UCRT computes them (x64, FMA3 code path).
//!
//! All three evaluate in double precision from `(double)x`: a short Taylor polynomial on
//! `[-π/4, π/4]` after reduction by π/2, rounded to float at the end. The polynomials are short
//! enough that results are often one ulp away from the correctly rounded value; that is what the
//! oracle's Skia saw, so it is reproduced bit for bit.

use super::reduce::{large_f32, large_f64, medium_f32};

/// `-1/6`, `1/120`, `-1/5040`, `1/362880`.
const S1: f64 = f64::from_bits(0xbfc5_5555_5555_5555);
const S2: f64 = f64::from_bits(0x3f81_1111_1111_1111);
const S3: f64 = f64::from_bits(0xbf2a_01a0_1a01_a01a);
const S4: f64 = f64::from_bits(0x3ec7_1de3_a556_c734);
/// `1/24`, `-1/720`, `1/40320`, `-1/3628800`.
const C1: f64 = f64::from_bits(0x3fa5_5555_5555_5555);
const C2: f64 = f64::from_bits(0xbf56_c16c_16c1_6c16);
const C3: f64 = f64::from_bits(0x3efa_01a0_1a01_a019);
const C4: f64 = f64::from_bits(0xbe92_7e4f_b778_9f5c);
const ONE_SIXTH: f64 = f64::from_bits(0x3fc5_5555_5555_5555);
const SIGN: u64 = 1 << 63;

/// The UCRT's float NaN/infinity path: a NaN is returned quieted, an infinity becomes the default
/// NaN (`0xffc00000`).
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64), float domain-error handler
pub(super) fn nan_result_f32(x: f32) -> f32 {
    let bits = x.to_bits();
    if bits.trailing_zeros() >= 23 {
        f32::from_bits(0xffc0_0000)
    } else {
        f32::from_bits(bits | 0x0040_0000)
    }
}

/// `x + x³·(S1 + x²·(S2 + x²·(S3 + x²·S4)))`.
fn sin_poly(x: f64) -> f64 {
    let x2 = x * x;
    let mut p = x2.mul_add(S4, S3);
    p = p.mul_add(x2, S2);
    p = p.mul_add(x2, S1);
    let x3 = x * x2;
    p.mul_add(x3, x)
}

/// `x⁴·(C1 + x²·(C2 + x²·(C3 + x²·C4)))`, the tail of the cosine polynomial.
fn cos_tail(x2: f64) -> f64 {
    let mut p = x2.mul_add(C4, C3);
    p = p.mul_add(x2, C2);
    p.mul_add(x2, C1)
}

/// The cosine polynomial of `sinf`: `fma(x², -0.5, 1)` plus the tail.
fn cos_poly_sinf(x: f64) -> f64 {
    let x2 = x * x;
    let head = x2.mul_add(-0.5, 1.0);
    let p = cos_tail(x2);
    let x4 = x2 * x2;
    p.mul_add(x4, head)
}

/// The cosine polynomial of `cosf`: `1 - x²·0.5` (two roundings) plus the tail.
fn cos_poly_cosf(x: f64) -> f64 {
    let x2 = x * x;
    let head = 1.0 - x2 * 0.5;
    let p = cos_tail(x2);
    let x4 = x2 * x2;
    p.mul_add(x4, head)
}

/// `sinf(x)`.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `sinf`, FMA3 path
#[must_use]
#[allow(clippy::cast_possible_truncation)] // the final (float) conversion of the double result
pub fn sinf(x: f32) -> f32 {
    let ux = x.to_bits() & 0x7fff_ffff;
    if ux >= 0x7f80_0000 {
        return nan_result_f32(x);
    }
    let xd = f64::from(x);
    if ux <= 0x3f49_0fdb {
        if ux < 0x3c00_0000 {
            if ux < 0x3900_0000 {
                return x;
            }
            let x3 = xd * xd * xd;
            return (-x3).mul_add(ONE_SIXTH, xd) as f32;
        }
        return sin_poly(xd) as f32;
    }
    let ax = xd.abs();
    let (r, region) = if ux < 0x4b80_0456 {
        medium_f32(ax)
    } else {
        large_f32(ax.to_bits())
    };
    let y = if region & 1 == 0 {
        sin_poly(r)
    } else {
        cos_poly_sinf(r)
    };
    let mut flip = if region == 2 || region == 3 { 0 } else { SIGN };
    flip ^= !xd.to_bits() & SIGN;
    f64::from_bits(y.to_bits() ^ flip) as f32
}

/// `cosf(x)`.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `cosf`, FMA3 path
#[must_use]
#[allow(clippy::cast_possible_truncation)] // the final (float) conversion of the double result
pub fn cosf(x: f32) -> f32 {
    let ux = x.to_bits() & 0x7fff_ffff;
    if ux >= 0x7f80_0000 {
        return nan_result_f32(x);
    }
    let xd = f64::from(x);
    if ux <= 0x3f49_0fdb {
        if ux < 0x3c00_0000 {
            if ux < 0x3900_0000 {
                return 1.0;
            }
            return (-(xd * 0.5)).mul_add(xd, 1.0) as f32;
        }
        return cos_poly_cosf(xd) as f32;
    }
    let ax = xd.abs();
    let (r, region) = if ux < 0x4f49_0fdb {
        medium_f32(ax)
    } else {
        let (r, _, region) = large_f64(ax.to_bits());
        (r, region)
    };
    let y = if region & 1 == 1 {
        sin_poly(r)
    } else {
        cos_poly_cosf(r)
    };
    let flip = (((region + 1) >> 1) & 1) << 63;
    f64::from_bits(y.to_bits() ^ flip) as f32
}

/// `tanf(x)`.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `tanf`, FMA3 path
#[must_use]
#[allow(clippy::cast_possible_truncation)] // the final (float) conversion of the double result
pub fn tanf(x: f32) -> f32 {
    const ONE_THIRD: f64 = f64::from_bits(0x3fd5_5555_5555_5555);
    const MEDIUM_LIMIT: u64 = 0x41e9_21fb_4000_0000;
    let ux = x.to_bits() & 0x7fff_ffff;
    if ux >= 0x7f80_0000 {
        return nan_result_f32(x);
    }
    let xd = f64::from(x);
    if ux <= 0x3f49_0fdb {
        if ux < 0x3900_0000 {
            if ux < 0x3200_0000 {
                return x;
            }
            let x2 = xd * xd;
            let x3 = x2 * xd;
            return x3.mul_add(ONE_THIRD, xd) as f32;
        }
        return tan_poly(xd) as f32;
    }
    let ax = xd.abs();
    let (r, region) = if ax.to_bits() < MEDIUM_LIMIT {
        medium_f32(ax)
    } else {
        large_f32(ax.to_bits())
    };
    let mut y = tan_poly(r);
    if region & 1 == 1 {
        y = -1.0 / y;
    }
    f64::from_bits(y.to_bits() ^ (xd.to_bits() & SIGN)) as f32
}

/// `x + x³·P(x²)/Q(x²)`, the rational approximation of `tanf`.
fn tan_poly(x: f64) -> f64 {
    const P0: f64 = f64::from_bits(0x3fd8_a8b0_da56_cb17);
    const P1: f64 = f64::from_bits(0xbf91_9dba_6efd_6aad);
    const Q0: f64 = f64::from_bits(0x3ff2_7e84_a3e7_3a2e);
    const Q1: f64 = f64::from_bits(0xbfe0_7266_d7b3_511b);
    const Q2: f64 = f64::from_bits(0x3f92_e290_03c6_92d9);
    let x2 = x * x;
    let num = P1.mul_add(x2, P0);
    let den = Q2.mul_add(x2, Q1).mul_add(x2, Q0);
    let q = num / den;
    let x3 = x2 * x;
    x3.mul_add(q, x)
}
