// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `expf`, `logf` and `log2f` as the UCRT computes them (x64, FMA3 code path).

use super::tables::{EXP2_J_OVER_64, LOG2F_INVC_LOGC, LOGF_INV, LOGF_LEAD, LOGF_TAIL};
use super::trig_f32::nan_result_f32;

/// `expf(x)`: `2^(n/64)` from a table times a cubic, evaluated in double.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `expf`, FMA3 path
#[must_use]
#[allow(clippy::cast_possible_truncation)] // mirrors cvtpd2dq and the final (float) conversion
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
pub fn expf(x: f32) -> f32 {
    const SIXTY_FOUR_OVER_LN2: f64 = f64::from_bits(0x4057_1547_652b_82fe);
    const LN2_OVER_SIXTY_FOUR: f64 = f64::from_bits(0x3f86_2e42_fefa_39ef);
    const ONE_SIXTH: f64 = f64::from_bits(0x3fc5_5555_5555_5555);
    let ux = x.to_bits() & 0x7fff_ffff;
    let xd = f64::from(x);
    let t = xd * SIXTY_FOUR_OVER_LN2;
    if ux >= 0x42b0_0000 {
        if ux >= 0x7f80_0000 {
            return match x.to_bits() {
                0x7f80_0000 => x,
                0xff80_0000 => 0.0,
                _ => nan_result_f32(x),
            };
        }
        if t >= 8192.0 {
            return f32::INFINITY;
        }
        if t < -9600.0 {
            return 0.0;
        }
    }
    // `cvtpd2dq` rounds to nearest even (the default MXCSR mode).
    let n = t.round_ties_even() as i32;
    let r = (-f64::from(n)).mul_add(LN2_OVER_SIXTY_FOUR, xd);
    let j = n & 63;
    let m = (n - j) >> 6;
    let r2 = r * r;
    let c = ONE_SIXTH.mul_add(r, 0.5);
    let p = r2.mul_add(c, r);
    let scale = f64::from_bits(u64::from((m + 0x3ff).cast_unsigned()) << 52);
    #[allow(clippy::cast_sign_loss)] // j is in 0..64
    let tj = f64::from_bits(EXP2_J_OVER_64[j as usize]);
    (p.mul_add(tj, tj) * scale) as f32
}

/// `logf(x)`: single-precision table-driven logarithm with FMA.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `logf`, FMA3 path
#[must_use]
#[allow(clippy::cast_precision_loss)] // mirrors cvtdq2ps of the exponent
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
pub fn logf(x: f32) -> f32 {
    const LN2_LEAD: f32 = f32::from_bits(0x3f31_7000);
    const LN2_TAIL: f32 = f32::from_bits(0x3805_fdf4);
    const ONE_THIRD: f32 = f32::from_bits(0x3eaa_aaab);
    const ONE_TWELFTH: f32 = f32::from_bits(0x3daa_aaab);
    const ONE_EIGHTIETH: f32 = f32::from_bits(0x3c4c_cccd);
    let bits = x.to_bits();
    let ux = bits & 0x7fff_ffff;
    if ux >= 0x7f80_0000 {
        return match bits {
            0x7f80_0000 => x,
            0xff80_0000 => f32::from_bits(0xffc0_0000),
            _ => nan_result_f32(x),
        };
    }
    if x <= 0.0 {
        return if x == 0.0 {
            f32::NEG_INFINITY
        } else {
            f32::from_bits(0xffc0_0000)
        };
    }
    let mut m = ux & 0x007f_ffff;
    let biased = ux >> 23;
    let (x, e) = if biased == 0 {
        // Denormal: normalize through a float subtraction; the "x" carried on is the bare mantissa.
        let v = f32::from_bits(m | 0x3f80_0000) - 1.0;
        let vb = v.to_bits();
        #[allow(clippy::cast_possible_wrap)] // an 8-bit exponent field
        let e = (vb >> 23) as i32 - 0xfd;
        m = vb & 0x007f_ffff;
        (f32::from_bits(m), e as f32)
    } else {
        #[allow(clippy::cast_possible_wrap)] // an 8-bit exponent field
        let e = biased as i32 - 0x7f;
        (x, e as f32)
    };
    if (x - 1.0).abs() < 0.0625 {
        let f = x - 1.0;
        let u = f / (2.0 + f);
        let hfsq = f * u;
        let u2 = u + u;
        let v = u2 * u2;
        let v3 = u2 * v;
        let p = v.mul_add(ONE_EIGHTIETH, ONE_TWELFTH);
        return f + p.mul_add(v3, -hfsq);
    }
    let f = f32::from_bits(m | 0x3f00_0000);
    let idx = (m >> 16) + ((m >> 15) & 1);
    let big_f = f32::from_bits((idx << 16) | 0x3f00_0000);
    let i = idx as usize;
    let r = (big_f - f) * f32::from_bits(LOGF_INV[i]);
    let c = ONE_THIRD.mul_add(r, 0.5);
    let r2 = r * r;
    let q = c.mul_add(r2, r);
    let tail = LN2_TAIL.mul_add(e, -q) + f32::from_bits(LOGF_TAIL[i]);
    let head = LN2_LEAD.mul_add(e, f32::from_bits(LOGF_LEAD[i]));
    head + tail
}

/// `log2f(x)`: 16-entry table and a cubic in double (the algorithm of ARM's optimized-routines).
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `log2f`, FMA3 path
#[must_use]
#[allow(clippy::cast_possible_truncation)] // the final (float) conversion
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
pub fn log2f(x: f32) -> f32 {
    const OFF: u32 = 0x3f33_0000;
    const INV_LN2: f64 = f64::from_bits(0x3ff7_1547_5f35_c8b8);
    const A0: f64 = f64::from_bits(0x3fde_cabf_4968_32e0);
    const A1: f64 = f64::from_bits(0x3fe7_1547_9ffa_e3de);
    const A2: f64 = f64::from_bits(0x3fd7_12b6_f70a_7e4d);
    let mut ix = x.to_bits();
    if ix == 0x3f80_0000 {
        return 0.0;
    }
    if ix.wrapping_sub(0x0080_0000) >= 0x7f00_0000 {
        if ix << 1 == 0 {
            return f32::NEG_INFINITY;
        }
        if ix == 0x7f80_0000 {
            return x;
        }
        if ix & 0x8000_0000 != 0 || (ix << 1) >= 0xff00_0000 {
            // (x - x) / (x - x): the default NaN, or the quieted input NaN.
            return if x.is_nan() {
                nan_result_f32(x)
            } else {
                f32::from_bits(0xffc0_0000)
            };
        }
        // Denormal: scale by 2^23.
        ix = (x * f32::from_bits(0x4b00_0000))
            .to_bits()
            .wrapping_sub(23 << 23);
    }
    let tmp = ix.wrapping_sub(OFF);
    let i = ((tmp >> 19) & 15) as usize;
    let top = tmp & 0xff80_0000;
    let iz = ix.wrapping_sub(top);
    #[allow(clippy::cast_possible_wrap)] // mirrors the signed shift of the C code
    let k = (tmp as i32) >> 23;
    let invc = f64::from_bits(LOG2F_INVC_LOGC[2 * i]);
    let logc = f64::from_bits(LOG2F_INVC_LOGC[2 * i + 1]);
    let z = f64::from(f32::from_bits(iz));
    let r = z * invc - 1.0;
    let y0 = f64::from(k) + logc;
    let y = y0 + r * INV_LN2;
    let r2 = r * r;
    let p = (r * A0 - A1) - r2 * A2;
    (y + p * r2) as f32
}
