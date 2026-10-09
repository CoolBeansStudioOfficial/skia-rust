// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `pow` and `exp2` as the UCRT computes them (x64, FMA3 code path): the algorithms of ARM's
//! optimized-routines (as in glibc), compiled with fused multiply-adds where the UCRT's build fused
//! them and with the operation order of its binary.

use super::tables::{EXP_TAB_N256, POW_LOG_TAB};

const SIGN: u64 = 1 << 63;
const ONE_BITS: u64 = 0x3ff0_0000_0000_0000;
const SIGN_BIAS: u64 = 0x8_0000;
const TWO_POW_M1022: f64 = f64::from_bits(0x0010_0000_0000_0000);
const TWO_POW_1009: f64 = f64::from_bits(0x7f00_0000_0000_0000);

fn top12(x: f64) -> u32 {
    #[allow(clippy::cast_possible_truncation)] // the top 12 bits
    let t = (x.to_bits() >> 52) as u32;
    t
}

/// `±inf` for overflow, `±0` for underflow (`__math_oflow` / `__math_uflow`).
fn overflow(sign: bool) -> f64 {
    if sign {
        f64::NEG_INFINITY
    } else {
        f64::INFINITY
    }
}
fn underflow(sign: bool) -> f64 {
    if sign { -0.0 } else { 0.0 }
}

/// `log(x)` as a head and tail (`log_inline` of ARM's pow).
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // bit fields
fn log_inline(ix: u64) -> (f64, f64) {
    const OFF: u64 = 0x3fe6_9555_0000_0000;
    const LN2_HI: f64 = f64::from_bits(0x3fe6_2e42_fefa_3800);
    const LN2_LO: f64 = f64::from_bits(0x3d2e_f357_93c7_6730);
    const A0: f64 = -0.5;
    const B1: f64 = f64::from_bits(0x3fe9_9999_9959_554e);
    const B2: f64 = f64::from_bits(0x3fe5_5555_5529_a47a);
    const B3: f64 = f64::from_bits(0x3ff0_002b_8b26_3fc3);
    const B4: f64 = f64::from_bits(0x3ff2_495b_9b48_45e9);
    const B5: f64 = f64::from_bits(0x3fe0_0000_0000_0006);
    const B6: f64 = f64::from_bits(0x3fe5_5555_5555_5560);
    let tmp = ix.wrapping_sub(OFF);
    let i = ((tmp >> 45) & 127) as usize;
    let k = (tmp as i64) >> 52;
    let iz = ix.wrapping_sub(tmp & 0xfff0_0000_0000_0000);
    let z = f64::from_bits(iz);
    let kd = f64::from(k as i32);
    let invc = f64::from_bits(POW_LOG_TAB[4 * i]);
    let logc = f64::from_bits(POW_LOG_TAB[4 * i + 2]);
    let logctail = f64::from_bits(POW_LOG_TAB[4 * i + 3]);
    let r = invc.mul_add(z, -1.0);
    let t1 = kd * LN2_HI + logc;
    let ar = r * A0;
    let ar2 = ar * r;
    let lo3 = ar.mul_add(r, -ar2);
    let lo1 = kd * LN2_LO + logctail;
    let t2 = t1 + r;
    let lo2 = (t1 - t2) + r;
    let lo123 = lo3 + (lo1 + lo2);
    let hi = ar2 + t2;
    let lo4 = (t2 - hi) + ar2;
    let lo = lo123 + lo4;
    let q1 = B1 - r * B2;
    let q2 = r * B3 - B4;
    let q3 = r * B5 - B6;
    let p = ((q1 + q2 * ar2) * ar2 + q3) * (ar2 * r);
    let lo = lo + p;
    let y = lo + hi;
    let tail = (hi - y) + lo;
    (y, tail)
}

/// `exp(x + xtail)` with the sign bias folded into the scale (`exp_inline` of ARM's pow).
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // bit fields
fn exp_inline(x: f64, xtail: f64, sign_bias: u64) -> f64 {
    const INV_LN2_N: f64 = f64::from_bits(0x4077_1547_652b_82fe);
    const SHIFT: f64 = f64::from_bits(0x4238_0000_0000_8000);
    const LN2_HI_N: f64 = f64::from_bits(0x3f66_2e42_fefc_0000);
    const LN2_LO_N: f64 = f64::from_bits(0x3d2c_610c_a86c_3899);
    const C2: f64 = f64::from_bits(0x3fdf_ffff_ffff_fdbd);
    const C3: f64 = f64::from_bits(0x3fc5_5555_5555_543c);
    const C4: f64 = f64::from_bits(0x3fa5_5555_cf16_e1ed);
    const C5: f64 = f64::from_bits(0x3f81_1111_67a4_b553);
    let mut abstop = top12(x) & 0x7ff;
    if abstop.wrapping_sub(0x3c9) >= 0x3f {
        if abstop.wrapping_sub(0x3c9) >= 0x8000_0000 {
            let one = x + 1.0;
            return if sign_bias == 0 { one } else { -one };
        }
        if abstop >= 0x409 {
            return if x.to_bits() >> 63 != 0 {
                underflow(sign_bias != 0)
            } else {
                overflow(sign_bias != 0)
            };
        }
        abstop = 0;
    }
    let kd = x * INV_LN2_N + SHIFT;
    let ki = kd.to_bits() >> 16;
    let kd = f64::from(ki as u32 as i32);
    let r = (x - kd * LN2_HI_N) + kd * LN2_LO_N;
    let r = r + xtail;
    let idx = 2 * (ki & 255) as usize;
    let top = (ki.wrapping_add(sign_bias)) << 44;
    let tail = f64::from_bits(EXP_TAB_N256[idx]);
    let sbits = EXP_TAB_N256[idx + 1].wrapping_add(top);
    let r2 = r * r;
    let a = (r * C3 + C2) * r2;
    let b = r * C5 + C4;
    let tmp = (a + (r + tail)) + b * (r2 * r2);
    if abstop == 0 {
        return special_case(tmp, sbits, ki);
    }
    let scale = f64::from_bits(sbits);
    tmp * scale + scale
}

/// The scaling of `exp_inline` near the overflow and underflow thresholds.
fn special_case(tmp: f64, sbits: u64, ki: u64) -> f64 {
    if ki & 0x8000_0000 == 0 {
        let scale = f64::from_bits(sbits.wrapping_sub(1009 << 52));
        return (scale * tmp + scale) * TWO_POW_1009;
    }
    let scale = f64::from_bits(sbits.wrapping_add(1022 << 52));
    let st = scale * tmp;
    let mut y = scale + st;
    if y.abs() < 1.0 {
        let one = if y < 0.0 { -1.0 } else { 1.0 };
        let lo = (scale - y) + st;
        let hi = one + y;
        let lo = lo + ((one - hi) + y);
        y = (lo + hi) - one;
        if y == 0.0 {
            y = f64::from_bits(sbits.wrapping_add(1022 << 52) & SIGN);
        }
    }
    y * TWO_POW_M1022
}

/// `1` for odd integers, `2` for even integers, `0` otherwise (`checkint`).
fn checkint(iy: u64) -> u32 {
    let e = (iy >> 52) & 0x7ff;
    if e < 0x3ff {
        return 0;
    }
    if e > 0x433 {
        return 2;
    }
    let m = 1u64 << (0x433 - e);
    if iy & (m - 1) != 0 {
        return 0;
    }
    if iy & m != 0 { 1 } else { 2 }
}

/// `pow(x, y)`.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `pow`, FMA3 path
#[must_use]
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
#[allow(clippy::float_cmp)] // mirrors exact comparisons of the C code
#[allow(clippy::similar_names)] // topx/topy as in the C code
pub fn pow(x: f64, y: f64) -> f64 {
    let mut ix = x.to_bits();
    let iy = y.to_bits();
    let mut topx = top12(x);
    let topy = top12(y);
    let mut sign_bias = 0u64;
    let fast = topx.wrapping_sub(1) < 0x7fe && (topy & 0x7ff).wrapping_sub(0x3be) < 0x80;
    if !fast {
        let zeroinfnan = |i: u64| (i << 1).wrapping_sub(1) >= 0xffdf_ffff_ffff_ffff;
        if zeroinfnan(iy) {
            if iy << 1 == 0 {
                return if ((ix ^ 0x0008_0000_0000_0000) << 1) > 0xfff0_0000_0000_0000 {
                    x + y
                } else {
                    1.0
                };
            }
            if ix == ONE_BITS {
                return if ((iy ^ 0x0008_0000_0000_0000) << 1) > 0xfff0_0000_0000_0000 {
                    x + y
                } else {
                    1.0
                };
            }
            if ix << 1 > 0xffe0_0000_0000_0000 || iy << 1 > 0xffe0_0000_0000_0000 {
                return x + y;
            }
            if ix << 1 == 0x7fe0_0000_0000_0000 {
                return 1.0;
            }
            if ((ix << 1) < 0x7fe0_0000_0000_0000) == (iy >> 63 == 0) {
                return 0.0;
            }
            return y * y;
        }
        if zeroinfnan(ix) {
            let mut x2 = x * x;
            let mut negative = false;
            if ix >> 63 != 0 && checkint(iy) == 1 {
                x2 = -x2;
                negative = true;
            }
            if ix << 1 == 0 {
                if iy >> 63 != 0 {
                    return overflow(negative);
                }
                return x2;
            }
            return if iy >> 63 != 0 { 1.0 / x2 } else { x2 };
        }
        if ix >> 63 != 0 {
            match checkint(iy) {
                0 => return f64::from_bits(0xfff8_0000_0000_0000),
                1 => sign_bias = SIGN_BIAS,
                _ => {}
            }
            ix &= !SIGN;
            topx &= 0x7ff;
        }
        if (topy & 0x7ff).wrapping_sub(0x3be) >= 0x80 {
            if ix == ONE_BITS {
                return 1.0;
            }
            if (topy & 0x7ff) < 0x3be {
                return if ix > ONE_BITS { y + 1.0 } else { 1.0 - y };
            }
            return if (ix > ONE_BITS) == (topy < 0x800) {
                overflow(false)
            } else {
                underflow(false)
            };
        }
        if topx == 0 {
            ix = (x * f64::from_bits(0x4330_0000_0000_0000)).to_bits() & !SIGN;
            ix = ix.wrapping_sub(52 << 52);
        }
    }
    if iy == ONE_BITS {
        return x;
    }
    if ix == ONE_BITS {
        return if sign_bias == 0 { 1.0 } else { -1.0 };
    }
    let (lhi, llo) = log_inline(ix);
    let ehi = y * lhi;
    let elo = y.mul_add(lhi, -ehi) + llo * y;
    exp_inline(ehi, elo, sign_bias)
}

/// `exp2(x)`.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `exp2`, FMA3 path
#[must_use]
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
#[allow(clippy::cast_possible_truncation)] // bit fields
pub fn exp2(x: f64) -> f64 {
    const SHIFT: f64 = f64::from_bits(0x42b8_0000_0000_0000);
    const C1: f64 = f64::from_bits(0x3fe6_2e42_fefa_39ef);
    const C2: f64 = f64::from_bits(0x3fce_bfbd_ff82_c424);
    const C3: f64 = f64::from_bits(0x3fac_6b08_d70c_f4b5);
    const C4: f64 = f64::from_bits(0x3f83_b2ab_d246_50cc);
    const C5: f64 = f64::from_bits(0x3f55_d7e0_9b4e_3a84);
    let bits = x.to_bits();
    let mut abstop = top12(x) & 0x7ff;
    if abstop.wrapping_sub(0x3c9) >= 0x3f {
        if abstop.wrapping_sub(0x3c9) >= 0x8000_0000 {
            return x + 1.0;
        }
        if abstop >= 0x409 {
            if bits == 0xfff0_0000_0000_0000 {
                return 0.0;
            }
            if abstop >= 0x7ff {
                return x + 1.0;
            }
            if bits >> 63 == 0 {
                return f64::INFINITY;
            }
            if bits >= 0xc090_cc00_0000_0000 {
                return 0.0;
            }
        }
        if bits << 1 > 0x811a_0000_0000_0000 {
            abstop = 0;
        }
    }
    let kd = x + SHIFT;
    let ki = kd.to_bits();
    let kd = kd - SHIFT;
    let r = x - kd;
    let idx = 2 * (ki & 255) as usize;
    let tail = f64::from_bits(EXP_TAB_N256[idx]);
    let sbits = EXP_TAB_N256[idx + 1].wrapping_add(ki << 44);
    let r2 = r * r;
    let a = (r * C3 + C2) * r2;
    let b = r * C5 + C4;
    let tmp = (a + (r * C1 + tail)) + b * (r2 * r2);
    if abstop == 0 {
        if ki & 0x8000_0000 == 0 {
            let scale = f64::from_bits(sbits.wrapping_sub(1 << 52));
            let y = scale * tmp + scale;
            return y + y;
        }
        let scale = f64::from_bits(sbits.wrapping_add(1022 << 52));
        let st = scale * tmp;
        let mut y = st + scale;
        if y < 1.0 {
            let lo = (scale - y) + st;
            let hi = y + 1.0;
            let lo = lo + ((1.0 - hi) + y);
            y = (lo + hi) - 1.0;
            if y == 0.0 {
                y = 0.0;
            }
        }
        return y * TWO_POW_M1022;
    }
    let scale = f64::from_bits(sbits);
    tmp * scale + scale
}
