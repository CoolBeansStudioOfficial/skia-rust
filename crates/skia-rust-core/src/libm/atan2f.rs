// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `atan2f` as the UCRT computes it (x64, FMA3 code path): in double precision, with a 241-entry
//! table of `atan(k/256)` and a cubic correction.

use super::tables::ATAN_K_OVER_256;

const EXP_MASK: u64 = 0x7ff0_0000_0000_0000;
const SIGN: u64 = 1 << 63;
const PI_F: f32 = f32::from_bits(0x4049_0fdb);
const PI_OVER_2_F: f32 = f32::from_bits(0x3fc9_0fdb);
const PI_OVER_4_F: f32 = f32::from_bits(0x3f49_0fdb);
const THREE_PI_OVER_4_F: f32 = f32::from_bits(0x4016_cbe4);

/// Returns `v` or `-v` by the sign of `y`.
fn with_sign_of_y(v: f32, y_negative: bool) -> f32 {
    if y_negative { -v } else { v }
}

/// `atan2f(y, x)`.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `atan2f`, FMA3 path
#[must_use]
#[allow(clippy::cast_possible_truncation)] // the (float) conversions of double results
#[allow(clippy::many_single_char_names)] // mirrors the C signature atan2f(y, x)
pub fn atan2f(y: f32, x: f32) -> f32 {
    let xd = f64::from(x);
    let yd = f64::from(y);
    let x_bits = xd.to_bits();
    let y_bits = yd.to_bits();
    let ay = y_bits & !SIGN;
    let ax = x_bits & !SIGN;
    let y_negative = y_bits & SIGN != 0;
    let x_negative = x_bits & SIGN != 0;
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)] // 11-bit exponents
    let diff = ((y_bits >> 52) & 0x7ff) as i32 - ((x_bits >> 52) & 0x7ff) as i32;

    if ax > EXP_MASK {
        return f32::from_bits(x.to_bits() | 0x0040_0000);
    }
    if ay > EXP_MASK {
        return f32::from_bits(y.to_bits() | 0x0040_0000);
    }
    if ay == 0 {
        if x_negative {
            return with_sign_of_y(PI_F, y_negative);
        }
        return y;
    }
    if ax == 0 || diff > 26 {
        return with_sign_of_y(PI_OVER_2_F, y_negative);
    }
    if diff < -13 && !x_negative {
        if diff < -150 {
            return with_sign_of_y(0.0, y_negative);
        }
        // The UCRT scales by 2^100 around the division when the quotient is a float denormal; in
        // double precision the quotient is the same either way.
        return (yd / xd) as f32;
    }
    if diff < -26 {
        // `x` is negative here.
        return with_sign_of_y(PI_F, y_negative);
    }
    if ay == EXP_MASK && ax == EXP_MASK {
        let v = if x_negative {
            THREE_PI_OVER_4_F
        } else {
            PI_OVER_4_F
        };
        return with_sign_of_y(v, y_negative);
    }

    let mut large = xd.abs();
    let mut small = yd.abs();
    let swapped = small > large;
    if swapped {
        core::mem::swap(&mut large, &mut small);
    }
    let u = small / large;
    let mut r = if u > 0.0625 {
        const C: f64 = 256.0;
        const ONE_THIRD: f64 = f64::from_bits(0x3fd5_5555_5555_0877);
        #[allow(clippy::cast_sign_loss)] // mirrors vcvttsd2si on a value in 16.5..=256.5
        let k = C.mul_add(u, 0.5) as i32;
        let kd = f64::from(k);
        let den = small.mul_add(kd, large * C);
        let num = small.mul_add(C, -(kd * large));
        let t = num / den;
        let t2 = t * t;
        #[allow(clippy::cast_sign_loss)] // k is at least 16
        let base = f64::from_bits(ATAN_K_OVER_256[(k - 16) as usize]);
        let s = t + base;
        let t3 = t2 * t;
        (-t3).mul_add(ONE_THIRD, s)
    } else if u < f64::from_bits(0x3f1a_36e2_eb1c_432d) {
        u
    } else {
        const A3: f64 = f64::from_bits(0x3fd5_5555_5555_5538);
        const A5: f64 = f64::from_bits(0x3fc9_9999_9996_43a3);
        const A7: f64 = f64::from_bits(0x3fc2_4924_82bd_6be1);
        let u2 = u * u;
        let mut p = (-A7).mul_add(u2, A5);
        p = (-p).mul_add(u2, A3);
        let u3 = u2 * u;
        (-u3).mul_add(p, u)
    };
    if swapped {
        r = f64::from_bits(0x3ff9_21fb_5444_2d18) - r;
    }
    if x_negative {
        r = f64::from_bits(0x4009_21fb_5444_2d18) - r;
    }
    if y_negative {
        r = -r;
    }
    r as f32
}
