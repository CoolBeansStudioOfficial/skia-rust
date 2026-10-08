// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `exp` and `log` as the UCRT computes them (x64, FMA3 code path): table-driven, with head/tail
//! table values.

use super::tables::{
    EXP_HEAD_J_OVER_64, EXP_TAIL_J_OVER_64, EXP2_J_OVER_64, LOG_INV_F, LOG_LEAD, LOG_TAIL,
};

const QUIET: u64 = 0x0008_0000_0000_0000;
const INF: u64 = 0x7ff0_0000_0000_0000;
const NEG_INF: u64 = 0xfff0_0000_0000_0000;

/// `exp(x)`.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `exp`, FMA3 path
#[must_use]
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // cvttsd2si and bit assembly
pub fn exp(x: f64) -> f64 {
    const OVERFLOW: f64 = f64::from_bits(0x4086_2e42_fefa_39ef);
    const UNDERFLOW: f64 = f64::from_bits(0xc087_4046_dfef_d9d0);
    const DENORM_MIN_LIMIT: f64 = f64::from_bits(0xc087_4910_d52d_3051);
    const SIXTY_FOUR_OVER_LN2: f64 = f64::from_bits(0x4057_1547_652b_82fe);
    const MINUS_LN2_OVER_64_HI: f64 = f64::from_bits(0xbf86_2e42_fefa_0000);
    const MINUS_LN2_OVER_64_LO: f64 = f64::from_bits(0xbd1c_f79a_bc9e_3b39);
    const C2: f64 = 0.5;
    const C3: f64 = f64::from_bits(0x3fc5_5555_5555_5555);
    const C4: f64 = f64::from_bits(0x3fa5_5555_5555_5555);
    const C5: f64 = f64::from_bits(0x3f81_1111_1111_1111);
    const C6: f64 = f64::from_bits(0x3f56_c16c_16c1_6c17);
    let bits = x.to_bits();
    let abs = bits & !(1 << 63);
    if abs >= INF {
        return match bits {
            INF => x,
            NEG_INF => 0.0,
            _ => f64::from_bits(bits | QUIET),
        };
    }
    if !(UNDERFLOW..=OVERFLOW).contains(&x) {
        if x > OVERFLOW {
            return f64::INFINITY;
        }
        return if x >= DENORM_MIN_LIMIT {
            f64::from_bits(1)
        } else {
            0.0
        };
    }
    if abs <= 0x3e50_0000_0000_0000 {
        return x + 1.0;
    }
    let t = x * SIXTY_FOUR_OVER_LN2;
    let n = t.trunc();
    let ni = t as i32;
    let rr = n.mul_add(MINUS_LN2_OVER_64_HI, x);
    let r = n * MINUS_LN2_OVER_64_LO + rr;
    let j = (ni & 63) as usize;
    let m = ni >> 6;
    let mut p = C6.mul_add(r, C5);
    p = p.mul_add(r, C4);
    p = p.mul_add(r, C3);
    p = p.mul_add(r, C2);
    let q = (r * r).mul_add(p, r);
    let v = q * f64::from_bits(EXP2_J_OVER_64[j])
        + f64::from_bits(EXP_TAIL_J_OVER_64[j])
        + f64::from_bits(EXP_HEAD_J_OVER_64[j]);
    if m > -1022 || (m == -1022 && v >= 1.0) {
        return f64::from_bits(v.to_bits().wrapping_add((i64::from(m) as u64) << 52));
    }
    v * f64::from_bits(1u64 << ((m + 1074) as u32 & 63))
}

/// `log(x)`.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `log`, FMA3 path
#[must_use]
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // exponent/index bit fields
pub fn log(x: f64) -> f64 {
    const LN2_LEAD: f64 = f64::from_bits(0x3fe6_2e42_e000_0000);
    const LN2_TAIL: f64 = f64::from_bits(0x3e6e_fa39_ef35_793c);
    const A2: f64 = 0.5;
    const A3: f64 = f64::from_bits(0x3fd5_5555_5555_5555);
    const A4: f64 = 0.25;
    const A5: f64 = f64::from_bits(0x3fc9_9999_9999_999a);
    const A6: f64 = f64::from_bits(0x3fc5_5555_5555_5555);
    const B0: f64 = f64::from_bits(0x3fb5_5555_5555_54e6);
    const B1: f64 = f64::from_bits(0x3f89_9999_99ba_c6d4);
    const B2: f64 = f64::from_bits(0x3f62_4923_07f1_519f);
    const B3: f64 = f64::from_bits(0x3f3c_8034_c85d_fff0);
    let bits = x.to_bits();
    if bits & INF == INF {
        return match bits {
            INF => x,
            NEG_INF => f64::from_bits(0xfff8_0000_0000_0000),
            _ => f64::from_bits(bits | QUIET),
        };
    }
    if x <= 0.0 {
        return if x == 0.0 {
            f64::NEG_INFINITY
        } else {
            f64::from_bits(0xfff8_0000_0000_0000)
        };
    }
    let f_minus_one = x - 1.0;
    let (y, e) = if (bits >> 52) == 0 {
        // Denormal: normalize through a subtraction; the value carried on is the bare mantissa.
        let v = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | 0x3ff0_0000_0000_0000) - 1.0;
        let e = ((v.to_bits() >> 52) as i32) - 0x7fd;
        (
            f64::from_bits(v.to_bits() & 0x000f_ffff_ffff_ffff),
            f64::from(e),
        )
    } else {
        (x, f64::from(((bits >> 52) as i32) - 0x3ff))
    };
    if f_minus_one.abs() < 0.0625 {
        let f = f_minus_one;
        let u = f / (2.0 + f);
        let uf = f * u;
        let v = u + u;
        let v2 = v * v;
        let lo = B1.mul_add(v2, B0);
        let hi = B3.mul_add(v2, B2);
        let v3 = v2 * v;
        let v7 = v3 * v3 * v;
        return f + (hi.mul_add(v7, lo * v3) - uf);
    }
    let yb = y.to_bits();
    let idx_bits = (yb & 0x000f_f000_0000_0000).wrapping_add((yb & 0x0000_0800_0000_0000) << 1);
    let i = (idx_bits >> 44) as usize;
    let f = f64::from_bits((yb & 0x000f_ffff_ffff_ffff) | 0x3fe0_0000_0000_0000);
    let big_f = f64::from_bits(idx_bits | 0x3fe0_0000_0000_0000);
    let r = (big_f - f) * f64::from_bits(LOG_INV_F[i]);
    let r2 = r * r;
    let p_hi = A6.mul_add(r, A5).mul_add(r, A4);
    let p_lo = A3.mul_add(r, A2);
    let r4 = r2 * r2;
    let q = p_hi.mul_add(r4, p_lo.mul_add(r2, r));
    let tail = f64::from_bits(LOG_TAIL[i]) + LN2_TAIL.mul_add(e, -q);
    let head = e.mul_add(LN2_LEAD, f64::from_bits(LOG_LEAD[i]));
    head + tail
}
