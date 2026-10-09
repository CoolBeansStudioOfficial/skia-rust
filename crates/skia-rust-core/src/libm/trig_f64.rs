// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `sin` and `cos` as the UCRT computes them (x64, FMA3 code path): fdlibm-style kernels with a
//! head/tail reduced argument.

use super::reduce::{large_f64, medium_f64};

const S1: f64 = f64::from_bits(0xbfc5_5555_5555_5555);
const S2: f64 = f64::from_bits(0x3f81_1111_1111_0bb3);
const S3: f64 = f64::from_bits(0xbf2a_01a0_19e8_3e5c);
const S4: f64 = f64::from_bits(0x3ec7_1de3_796c_de01);
const S5: f64 = f64::from_bits(0xbe5a_e600_b42f_dfa7);
const S6: f64 = f64::from_bits(0x3de5_e0b2_f9a4_3bb8);
const C1: f64 = f64::from_bits(0x3fa5_5555_5555_5555);
const C2: f64 = f64::from_bits(0xbf56_c16c_16c1_6967);
const C3: f64 = f64::from_bits(0x3efa_01a0_19f4_ec91);
const C4: f64 = f64::from_bits(0xbe92_7e4f_a17f_667b);
const C5: f64 = f64::from_bits(0x3e21_eeb6_9038_2eec);
const C6: f64 = f64::from_bits(0xbda9_07db_4725_8aa7);
const ONE_SIXTH: f64 = f64::from_bits(0x3fc5_5555_5555_5555);
const PI_OVER_4: u64 = 0x3fe9_21fb_5444_2d18;
const TWO_POW_M13: u64 = 0x3f20_0000_0000_0000;
const TWO_POW_M27: u64 = 0x3e40_0000_0000_0000;
const INFINITY: u64 = 0x7ff0_0000_0000_0000;
const LARGE: u64 = 0x4173_12d0_0000_0000; // 2e7
const SIGN: u64 = 1 << 63;

/// The UCRT's double NaN/infinity path: a NaN is returned quieted, an infinity becomes the default
/// NaN (`0xfff8000000000000`).
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64), double domain-error handler
fn nan_result_f64(x: f64) -> f64 {
    let bits = x.to_bits();
    if bits.trailing_zeros() >= 52 {
        f64::from_bits(0xfff8_0000_0000_0000)
    } else {
        f64::from_bits(bits | 0x0008_0000_0000_0000)
    }
}

/// `S2 + x²·(S3 + x²·(S4 + x²·(S5 + x²·S6)))`.
fn sin_inner(x2: f64) -> f64 {
    let mut p = S6.mul_add(x2, S5);
    p = p.mul_add(x2, S4);
    p = p.mul_add(x2, S3);
    p.mul_add(x2, S2)
}

/// `C1 + x²·(C2 + … + x²·C6)`.
fn cos_inner(x2: f64) -> f64 {
    let mut p = C6.mul_add(x2, C5);
    p = p.mul_add(x2, C4);
    p = p.mul_add(x2, C3);
    p = p.mul_add(x2, C2);
    p.mul_add(x2, C1)
}

/// The sine kernel on a reduced argument `r + rr`.
fn sin_kernel(r: f64, rr: f64) -> f64 {
    let x2 = r * r;
    let p = sin_inner(x2);
    let x3 = r * x2;
    let mut t = x3 * p;
    t = rr * 0.5 - t;
    t *= x2;
    t -= rr;
    t = (-x3).mul_add(S1, t);
    r - t
}

/// The cosine kernel on a reduced argument `r + rr`.
fn cos_kernel(r: f64, rr: f64) -> f64 {
    let x2 = r * r;
    let hz = x2 * 0.5;
    let w = 1.0 - hz;
    let mut t = (1.0 - w) - hz;
    t = (-r).mul_add(rr, t);
    let x4 = x2 * x2;
    let p = cos_inner(x2);
    p.mul_add(x4, t) + w
}

/// Reduces `|x|` (finite, at least π/4) by π/2.
fn reduce(abs_bits: u64) -> (f64, f64, u64) {
    if abs_bits >= LARGE {
        large_f64(abs_bits)
    } else {
        medium_f64(f64::from_bits(abs_bits))
    }
}

/// `sin(x)`.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `sin`, FMA3 path
#[must_use]
pub fn sin(x: f64) -> f64 {
    let bits = x.to_bits();
    let abs_bits = bits & !SIGN;
    if abs_bits < PI_OVER_4 {
        if abs_bits < TWO_POW_M13 {
            if abs_bits < TWO_POW_M27 {
                return x;
            }
            let x3 = x * x * x;
            return (-x3).mul_add(ONE_SIXTH, x);
        }
        let x2 = x * x;
        let p = sin_inner(x2).mul_add(x2, S1);
        let x3 = x * x2;
        return x3.mul_add(p, x);
    }
    if abs_bits >= INFINITY {
        return nan_result_f64(x);
    }
    let (r, rr, region) = reduce(abs_bits);
    let y = if region & 1 == 0 {
        sin_kernel(r, rr)
    } else {
        cos_kernel(r, rr)
    };
    let flip = (bits & SIGN) ^ if region & 2 == 0 { 0 } else { SIGN };
    f64::from_bits(y.to_bits() ^ flip)
}

/// `cos(x)`.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `cos`, FMA3 path
#[must_use]
pub fn cos(x: f64) -> f64 {
    let abs_bits = x.to_bits() & !SIGN;
    if abs_bits <= PI_OVER_4 {
        if abs_bits < TWO_POW_M13 {
            if abs_bits < TWO_POW_M27 {
                return 1.0;
            }
            return (-(x * 0.5)).mul_add(x, 1.0);
        }
        let x2 = x * x;
        let p = cos_inner(x2);
        return p.mul_add(x2, -0.5).mul_add(x2, 1.0);
    }
    if abs_bits >= INFINITY {
        return nan_result_f64(x);
    }
    let (r, rr, region) = reduce(abs_bits);
    let y = if region & 1 == 0 {
        cos_kernel(r, rr)
    } else {
        sin_kernel(r, rr)
    };
    let flip = if (region + 1) & 2 == 0 { 0 } else { SIGN };
    f64::from_bits(y.to_bits() ^ flip)
}
