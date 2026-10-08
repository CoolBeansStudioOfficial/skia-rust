// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `acos`, `cbrt` and `cbrtf` as the UCRT computes them (x64): fdlibm's `acos` with fused multiply-adds, and
//! a rational first guess for `cbrt` refined by two Newton steps.

const PI_OVER_2: f64 = f64::from_bits(0x3ff9_21fb_5444_2d18);
const PI: f64 = f64::from_bits(0x4009_21fb_5444_2d18);
const PI_OVER_2_LO: f64 = f64::from_bits(0x3c91_a626_3314_5c07);
const SIGN: u64 = 1 << 63;

/// `acos(x)`.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `acos`, FMA3 path
#[must_use]
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
#[allow(clippy::float_cmp)] // mirrors the exact ucomisd tests against ±1
pub fn acos(x: f64) -> f64 {
    const P0: f64 = f64::from_bits(0x3fcd_1e41_8002_9834);
    const P1: f64 = f64::from_bits(0x3fdc_7b29_7e26_9eac);
    const P2: f64 = f64::from_bits(0x3fd1_a2be_c1b7_ef59);
    const P3: f64 = f64::from_bits(0x3fac_28d3_90c2_9690);
    const P4: f64 = f64::from_bits(0x3f51_e5f8_87a6_2135);
    const P5: f64 = f64::from_bits(0x3f09_5166_5d32_1061);
    const Q0: f64 = f64::from_bits(0x3ff5_d6b1_2001_f228);
    const Q1: f64 = f64::from_bits(0x400a_4646_f903_cdea);
    const Q2: f64 = f64::from_bits(0x4006_2021_571d_ccfc);
    const Q3: f64 = f64::from_bits(0x3fee_324a_b418_f78d);
    const Q4: f64 = f64::from_bits(0x3fbb_1a42_2982_ce76);
    let bits = x.to_bits();
    let exp = (bits >> 52) & 0x7ff;
    if bits & !SIGN > 0x7ff0_0000_0000_0000 {
        return f64::from_bits(bits | 0x0008_0000_0000_0000);
    }
    if exp < 0x3c7 {
        return PI_OVER_2;
    }
    if exp >= 0x3ff {
        return if x == 1.0 {
            0.0
        } else if x == -1.0 {
            PI
        } else {
            f64::from_bits(0xfff8_0000_0000_0000)
        };
    }
    let ax = x.abs();
    let (z, s) = if exp < 0x3fe {
        (ax * ax, 0.0)
    } else {
        let z = (1.0 - ax) * 0.5;
        (z, z.sqrt())
    };
    let mut p = P5.mul_add(z, P4);
    p = p.mul_add(z, -P3);
    p = p.mul_add(z, P2);
    p = p.mul_add(z, -P1);
    p = p.mul_add(z, P0);
    let p = p * z;
    let mut q = Q4.mul_add(z, -Q3);
    q = q.mul_add(z, Q2);
    q = q.mul_add(z, -Q1);
    q = q.mul_add(z, Q0);
    let r = p / q;
    if exp < 0x3fe {
        let t = (-x).mul_add(r, PI_OVER_2_LO);
        return PI_OVER_2 - (x - t);
    }
    if bits & SIGN != 0 {
        let w = r.mul_add(s, -PI_OVER_2_LO) + s;
        return PI - (w + w);
    }
    let df = f64::from_bits(s.to_bits() & 0xffff_ffff_0000_0000);
    let c = (-df).mul_add(df, z) / (df + s);
    let t = r.mul_add(s + s, c + c);
    t + (df + df)
}

/// `cbrt(x)`.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `cbrt`
#[must_use]
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)] // exponent arithmetic on bit fields
#[allow(clippy::manual_midpoint)] // the UCRT's `(a + b) * 0.5`
pub fn cbrt(x: f64) -> f64 {
    const A: f64 = f64::from_bits(0x3fcf_3482_be8b_c16a);
    const B: f64 = f64::from_bits(0x3fee_a882_6aa8_eb46);
    const C: f64 = f64::from_bits(0x3fb4_3419_e300_14f9);
    const D: f64 = f64::from_bits(0x3fd1_e54b_48d3_ae68);
    let bits = x.to_bits();
    let biased = (bits >> 52) & 0x7ff;
    let mantissa = bits & 0x000f_ffff_ffff_ffff;
    if biased == 0x7ff || (biased == 0 && mantissa == 0) {
        // Infinities, NaNs (not quieted) and zeros are returned as they are.
        return x;
    }
    // frexp: |x| = m·2^e with m in [0.5, 1), denormals normalized first.
    let (m_bits, mut e) = if biased == 0 {
        let shift = mantissa.leading_zeros() - 11;
        let normalized = (mantissa << shift) & 0x000f_ffff_ffff_ffff;
        (normalized, 1 - i64::from(shift) - 0x3fe)
    } else {
        (mantissa, biased as i64 - 0x3fe)
    };
    let mut k = 0i64;
    while e % 3 != 0 {
        e += 1;
        k -= 1;
    }
    let f = f64::from_bits(((0x3fe + k) as u64) << 52 | m_bits);
    let half = f * 0.5;
    let three_halves = f * 1.5;
    let y0 = ((f * A + B) * f + C) / (f + D);
    let y1 = (three_halves / (y0 * y0 + half / y0) + y0) * 0.5;
    let y2 = (three_halves / (y1 * y1 + half / y1) + y1) * 0.5;
    let y = if bits & SIGN != 0 { -y2 } else { y2 };
    f64::from_bits(y.to_bits().wrapping_add(((e / 3) as u64) << 52))
}

/// `cbrtf(x)`: the algorithm of `cbrt` in single precision, with one Newton step.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `cbrtf`
#[must_use]
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
#[allow(clippy::manual_midpoint)] // the UCRT's `(a + b) * 0.5`
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)] // exponent arithmetic on bit fields
pub fn cbrtf(x: f32) -> f32 {
    const A: f32 = f32::from_bits(0x3e79_a416);
    const B: f32 = f32::from_bits(0x3f75_4413);
    const C: f32 = f32::from_bits(0x3da1_a0cf);
    const D: f32 = f32::from_bits(0x3e8f_2a5a);
    let bits = x.to_bits();
    let biased = (bits >> 23) & 0xff;
    let mantissa = bits & 0x007f_ffff;
    if biased == 0xff || (biased == 0 && mantissa == 0) {
        // Infinities, NaNs (not quieted) and zeros are returned as they are.
        return x;
    }
    // frexp: |x| = m·2^e with m in [0.5, 1), denormals normalized first.
    let (m_bits, mut e) = if biased == 0 {
        let shift = mantissa.leading_zeros() - 8;
        let normalized = (mantissa << shift) & 0x007f_ffff;
        (normalized, 1 - shift as i32 - 0x7e)
    } else {
        (mantissa, biased as i32 - 0x7e)
    };
    let mut k = 0i32;
    while e % 3 != 0 {
        e += 1;
        k -= 1;
    }
    let f = f32::from_bits(((0x7e + k) as u32) << 23 | m_bits);
    let half = f * 0.5;
    let three_halves = f * 1.5;
    let y0 = ((f * A + B) * f + C) / (f + D);
    let y1 = (three_halves / (y0 * y0 + half / y0) + y0) * 0.5;
    let y = if bits & 0x8000_0000 != 0 { -y1 } else { y1 };
    f32::from_bits(y.to_bits().wrapping_add(((e / 3) as u32) << 23))
}
