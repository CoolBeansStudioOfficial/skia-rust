// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `powf` as the UCRT computes it (x64, FMA3 code path): `exp(y·log(x))` in double precision, with
//! table-driven `log` and `exp`. For `x` within 1/16 of 1 the logarithm uses a series in
//! `f/(2+f)`, and that branch finishes with the non-FMA `exp` code (as the UCRT does).

use super::tables::{EXP2_J_OVER_64, LOG_INV_F, POWF_LOG_2F};

const LN2: f64 = f64::from_bits(0x3fe6_2e42_fefa_39ef);
const ONE_THIRD: f64 = f64::from_bits(0x3fd5_5555_5555_5555);
const ONE_SIXTH: f64 = f64::from_bits(0x3fc5_5555_5555_5555);
const SIXTY_FOUR_OVER_LN2: f64 = f64::from_bits(0x4057_1547_652b_82fe);
const LN2_OVER_SIXTY_FOUR: f64 = f64::from_bits(0x3f86_2e42_fefa_39ef);
const OVERFLOW: f64 = f64::from_bits(0x4056_2e43_0000_0000);
const UNDERFLOW: f64 = f64::from_bits(0xc059_d1da_0000_0000);
const SIGN: u32 = 0x8000_0000;
const INF_BITS: u32 = 0x7f80_0000;
const ONE_BITS: u32 = 0x3f80_0000;

/// Whether `y` is an integer per `roundss(y) == y`, and if so whether it is odd (`cvtss2si`).
#[allow(clippy::cast_possible_truncation)] // |y| < 2^24 here, so cvtss2si is exact
fn integer_parity(y: f32) -> Option<bool> {
    let rounded = y.round_ties_even();
    #[allow(clippy::float_cmp)] // mirrors ucomiss
    if rounded != y {
        return None;
    }
    Some((y as i32) & 1 == 1)
}

/// The sign bit of a negative base raised to `y` (`|y| < 2^24` checked by the caller): set when `y`
/// is an odd integer.
fn odd_sign(y: f32) -> u32 {
    if (y.to_bits() & 0x7f80_0000) > 0x4b00_0000 {
        return 0;
    }
    match integer_parity(y) {
        Some(true) => SIGN,
        _ => 0,
    }
}

/// `log(x)` for a positive float `x` as a double, table-driven (FMA).
#[allow(clippy::cast_possible_truncation)] // index extraction from the mantissa
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
fn log_table(xd: f64) -> f64 {
    let bits = xd.to_bits();
    let mantissa = bits & 0x000f_ffff_ffff_ffff;
    let idx = (mantissa >> 44) + ((mantissa >> 43) & 1);
    let big_f = f64::from_bits((idx | 0x3fe00) << 44);
    let f = f64::from_bits(mantissa | 0x3fe0_0000_0000_0000);
    #[allow(clippy::cast_possible_wrap)] // an 11-bit exponent field
    let e = ((bits & 0x7ff0_0000_0000_0000) >> 52) as i64 - 0x3ff;
    let i = idx as usize;
    let r = (big_f - f) * f64::from_bits(LOG_INV_F[i]);
    let p = ONE_THIRD.mul_add(r, 0.5).mul_add(r, 1.0);
    let q = r * p;
    #[allow(clippy::cast_precision_loss)] // mirrors cvtdq2pd of a small exponent
    let ed = e as f64;
    ed * LN2 + f64::from_bits(POWF_LOG_2F[i]) - q
}

/// `log(x)` for `x` within 1/16 of 1: `f - u·f + series(2u)`, `u = f/(2+f)`.
fn log_near_one(xd: f64) -> f64 {
    const C0: f64 = f64::from_bits(0x3f89_9999_99ba_c6d4);
    const C1: f64 = f64::from_bits(0x3f3c_8034_c85d_fff0);
    const D0: f64 = f64::from_bits(0x3fb5_5555_5555_54e6);
    const D1: f64 = f64::from_bits(0x3f62_4923_07f1_519f);
    let f = xd - 1.0;
    let u = f / (f + 2.0);
    let uf = u * f;
    let v = u + u;
    let v2 = v * v;
    let lo = v2 * C0 + D0;
    let hi = v2 * C1 + D1;
    let v3 = v * v2;
    let v7 = v2 * v2 * v3;
    let s = v7 * hi + v3 * lo;
    f + (s - uf)
}

/// Assembles `2^(n/64)·(1 + p)` as the UCRT does: table value, then an integer add to the exponent.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // bit assembly
fn scale(n: i32, mantissa: f64) -> f64 {
    let m = i64::from(n >> 6);
    f64::from_bits(mantissa.to_bits().wrapping_add((m as u64) << 52))
}

/// `exp(w)` with the FMA kernel.
#[allow(clippy::cast_possible_truncation)] // mirrors cvtpd2dq
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
fn exp_fma(w: f64) -> f64 {
    let n = (w * SIXTY_FOUR_OVER_LN2).round_ties_even() as i32;
    let r = (-f64::from(n)).mul_add(LN2_OVER_SIXTY_FOUR, w);
    let p = r * ONE_SIXTH.mul_add(r, 0.5).mul_add(r, 1.0);
    #[allow(clippy::cast_sign_loss)] // n & 63 is in 0..64
    let t = f64::from_bits(EXP2_J_OVER_64[(n & 63) as usize]);
    scale(n, p.mul_add(t, t))
}

/// `exp(w)` with the non-FMA kernel.
#[allow(clippy::cast_possible_truncation)] // mirrors cvtpd2dq
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
fn exp_plain(w: f64) -> f64 {
    let n = (w * SIXTY_FOUR_OVER_LN2).round_ties_even() as i32;
    let r = w - f64::from(n) * LN2_OVER_SIXTY_FOUR;
    let p = r * r * (ONE_SIXTH * r + 0.5) + r;
    #[allow(clippy::cast_sign_loss)] // n & 63 is in 0..64
    let t = f64::from_bits(EXP2_J_OVER_64[(n & 63) as usize]);
    scale(n, p * t + t)
}

/// `exp(y·log)` rounded to float, with overflow and underflow, then the sign.
#[allow(clippy::cast_possible_truncation)] // the (float) conversion
fn finish(yd: f64, log: f64, sign: u32, fused: bool) -> f32 {
    let w = yd * log;
    if w > OVERFLOW {
        return f32::from_bits(INF_BITS | sign);
    }
    if w <= UNDERFLOW {
        return f32::from_bits(sign);
    }
    let v = if fused { exp_fma(w) } else { exp_plain(w) };
    f32::from_bits((v as f32).to_bits() | sign)
}

/// `powf(x, y)`.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `powf`, FMA3 path
#[must_use]
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
pub fn powf(x: f32, y: f32) -> f32 {
    let xb = x.to_bits();
    let yb = y.to_bits();
    let ax = xb & !SIGN;
    let ay = yb & !SIGN;

    if ay >= INF_BITS {
        return pow_y_special(x, y);
    }
    if ay <= ONE_BITS {
        if ay == 0 {
            // pow(x, ±0) = 1, except that a signaling NaN `x` is returned quieted.
            if ax > INF_BITS && ax < 0x7fc0_0000 {
                return f32::from_bits(ax | 0x0040_0000);
            }
            return 1.0;
        }
        if yb == ONE_BITS {
            return if ax > INF_BITS {
                f32::from_bits(xb | 0x0040_0000)
            } else {
                x
            };
        }
    }
    if ax >= INF_BITS {
        return pow_x_special(x, y);
    }

    let xd = f64::from(x);
    let yd = f64::from(y);
    #[allow(clippy::cast_possible_wrap)] // mirrors the signed compare on the bits
    let x_signed = xb as i32;
    if x_signed >= 0x3f88_0000 {
        return finish(yd, log_table(xd), 0, true);
    }
    let mut sign = 0;
    if x_signed <= 0 {
        if ax == 0 {
            return pow_zero(xb, y);
        }
        if (ay & 0x7f80_0000) <= 0x4b00_0000 {
            match integer_parity(y) {
                None => return f32::from_bits(0xffc0_0000),
                Some(true) => sign = SIGN,
                Some(false) => {}
            }
        }
    }
    if (xd - 1.0).abs() >= 0.0625 {
        return finish(yd, log_table(xd), sign, true);
    }
    finish(yd, log_near_one(xd), sign, false)
}

/// `powf(±0, y)` for `|y| > 1` finite.
fn pow_zero(xb: u32, y: f32) -> f32 {
    let magnitude = if y < 0.0 { INF_BITS } else { 0 };
    let sign = if odd_sign(y) == 0 { 0 } else { xb & SIGN };
    f32::from_bits(sign | magnitude)
}

/// `powf(x, y)` for `y` infinite or NaN.
fn pow_y_special(x: f32, y: f32) -> f32 {
    let xb = x.to_bits();
    let ax = xb & !SIGN;
    let yb = y.to_bits();
    if (yb & !SIGN) > INF_BITS {
        if ax > INF_BITS {
            return if xb == 0xffc0_0000 {
                f32::from_bits(yb | 0x0040_0000)
            } else {
                f32::from_bits(xb | 0x0040_0000)
            };
        }
        if xb == ONE_BITS {
            return 1.0;
        }
        return f32::from_bits(yb | 0x0040_0000);
    }
    if ax > INF_BITS {
        return f32::from_bits(xb | 0x0040_0000);
    }
    if ax == ONE_BITS {
        return 1.0;
    }
    let below_one = ax < ONE_BITS;
    if (yb & SIGN == 0) == below_one {
        0.0
    } else {
        f32::INFINITY
    }
}

/// `powf(x, y)` for `x` infinite or NaN and `y` finite with `|y| > 1` or `0 < |y| < 1`.
fn pow_x_special(x: f32, y: f32) -> f32 {
    let xb = x.to_bits();
    match xb {
        0x7f80_0000 => {
            if y < 0.0 {
                0.0
            } else {
                f32::INFINITY
            }
        }
        0xff80_0000 => {
            let magnitude = if y < 0.0 { 0 } else { INF_BITS };
            f32::from_bits(odd_sign(y) | magnitude)
        }
        _ => f32::from_bits(xb | 0x0040_0000),
    }
}
