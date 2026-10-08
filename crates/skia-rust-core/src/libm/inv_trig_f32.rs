// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `asinf`, `acosf` and `atanf` as the UCRT computes them (x64, FMA3 code path). `asinf` and
//! `acosf` are fdlibm's float algorithms with fused multiply-adds; `atanf` evaluates in double.

const ONE: f32 = 1.0;
const PI_F: f32 = f32::from_bits(0x4049_0fdb);
const PI_OVER_2_F: f32 = f32::from_bits(0x3fc9_0fdb);
const PI_OVER_2_D: f64 = f64::from_bits(0x3ff9_21fb_5444_2d18);
const PI_D: f64 = f64::from_bits(0x4009_21fb_5444_2d18);
const PI_OVER_2_LO_D: f64 = f64::from_bits(0x3c91_a626_3314_5c07);
const DEFAULT_NAN: f32 = f32::from_bits(0xffc0_0000);

/// `R(z) = z·P(z)/Q(z)`, the rational approximation shared by `asinf` and `acosf`.
fn asin_rational(z: f32) -> f32 {
    const P0: f32 = f32::from_bits(0x3e3c_94dc);
    const P1: f32 = f32::from_bits(0x3d67_8bdd);
    const P2: f32 = f32::from_bits(0xbc5b_3fe1);
    const P3: f32 = f32::from_bits(0x3b81_ce6b);
    const Q0: f32 = f32::from_bits(0x3f8d_6fa5);
    const Q1: f32 = f32::from_bits(0x3f56_1f0d);
    let mut p = (-P3).mul_add(z, P2);
    p = p.mul_add(z, -P1);
    p = p.mul_add(z, P0);
    let p = p * z;
    let q = (-Q1).mul_add(z, Q0);
    p / q
}

/// Splits `(1 - |x|)/2` into `(z, s = sqrt(z))`.
fn half_complement(ax: f32) -> (f32, f32) {
    let z = (ONE - ax) * 0.5;
    (z, z.sqrt())
}

/// `acosf(x)`.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `acosf`, FMA3 path
#[must_use]
#[allow(clippy::cast_possible_truncation)] // the (float) conversions of double results
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
#[allow(clippy::float_cmp)] // mirrors the exact ucomiss tests against ±1
pub fn acosf(x: f32) -> f32 {
    let bits = x.to_bits();
    let exp = (bits >> 23) & 0xff;
    let ux = bits & 0x7fff_ffff;
    if ux > 0x7f80_0000 {
        return f32::from_bits(bits | 0x0040_0000);
    }
    if exp < 0x65 {
        return PI_OVER_2_F;
    }
    if exp >= 0x7f {
        return if x == 1.0 {
            0.0
        } else if x == -1.0 {
            PI_F
        } else {
            DEFAULT_NAN
        };
    }
    let negative = bits & 0x8000_0000 != 0;
    let ax = x.abs();
    if exp < 0x7e {
        let r = asin_rational(ax * ax);
        let rx = f64::from(r * x);
        return (PI_OVER_2_D - (f64::from(x) - (PI_OVER_2_LO_D - rx))) as f32;
    }
    let (z, s) = half_complement(ax);
    let r = asin_rational(z);
    if negative {
        let w = f64::from(s * r) - PI_OVER_2_LO_D + f64::from(s);
        return (PI_D - (w + w)) as f32;
    }
    let df = f32::from_bits(s.to_bits() & 0xffff_0000);
    let c = (-df).mul_add(df, z) / (df + s);
    let s2 = s + s;
    let t = r.mul_add(s2, c + c);
    t + (df + df)
}

/// `asinf(x)`.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `asinf`, FMA3 path
#[must_use]
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
#[allow(clippy::float_cmp)] // mirrors the exact ucomiss tests against ±1
pub fn asinf(x: f32) -> f32 {
    const PIO2_LO: f32 = f32::from_bits(0x33a2_2168);
    const PIO4_HI: f32 = f32::from_bits(0x3f49_0fda);
    let bits = x.to_bits();
    let exp = (bits >> 23) & 0xff;
    let ux = bits & 0x7fff_ffff;
    if ux > 0x7f80_0000 {
        return f32::from_bits(bits | 0x0040_0000);
    }
    if exp < 0x71 {
        return x;
    }
    if exp >= 0x7f {
        return if x == 1.0 {
            PI_OVER_2_F
        } else if x == -1.0 {
            -PI_OVER_2_F
        } else {
            DEFAULT_NAN
        };
    }
    let ax = x.abs();
    let t = if exp < 0x7e {
        let r = asin_rational(ax * ax);
        r.mul_add(ax, ax)
    } else {
        let (z, s) = half_complement(ax);
        let r = asin_rational(z);
        let df = f32::from_bits(s.to_bits() & 0xffff_0000);
        let c = (-df).mul_add(df, z) / (df + s);
        let p = r.mul_add(s + s, -(PIO2_LO - (c + c)));
        PIO4_HI - (p - (PIO4_HI - (df + df)))
    };
    if bits & 0x8000_0000 != 0 { -t } else { t }
}

/// `atanf(x)`: fdlibm's argument reduction and a rational approximation, in double.
// Port of: UCRT ucrtbase.dll 10.0.26100 (x64) `atanf`, FMA3 path
#[must_use]
#[allow(clippy::cast_possible_truncation)] // the (float) conversion of the double result
#[allow(clippy::many_single_char_names)] // the math notation of the ported algorithm
pub fn atanf(x: f32) -> f32 {
    const P0: f64 = f64::from_bits(0x3fd2_fa53_1690_7834);
    const P1: f64 = f64::from_bits(0x3fc8_9e17_3a81_ee7f);
    const P2: f64 = f64::from_bits(0x3f73_476a_758d_a22a);
    const Q0: f64 = f64::from_bits(0x3fec_777c_a210_53f0);
    const Q1: f64 = f64::from_bits(0x3ff1_c587_93da_6ea4);
    const Q2: f64 = f64::from_bits(0x3fd3_27e3_df2c_f2aa);
    const ATAN_HALF: f64 = f64::from_bits(0x3fdd_ac67_0561_bb4f);
    const ATAN_ONE: f64 = f64::from_bits(0x3fe9_21fb_5444_2d18);
    const ATAN_THREE_HALVES: f64 = f64::from_bits(0x3fef_730b_d281_f69b);
    let xd = f64::from(x);
    let negative = xd.to_bits() >> 63 != 0;
    let ax = xd.abs();
    let a = ax.to_bits();
    let (hi, t) = if a < 0x3fdc_0000_0000_0000 {
        (0.0, ax)
    } else if a < 0x3fe6_0000_0000_0000 {
        (ATAN_HALF, ((ax + ax) - 1.0) / (ax + 2.0))
    } else if a < 0x3ff3_0000_0000_0000 {
        (ATAN_ONE, (ax - 1.0) / (ax + 1.0))
    } else if a < 0x4003_8000_0000_0000 {
        (ATAN_THREE_HALVES, (ax - 1.5) / 1.5f64.mul_add(ax, 1.0))
    } else {
        if a > 0x7ff0_0000_0000_0000 {
            return f32::from_bits(x.to_bits() | 0x0040_0000);
        }
        if ax > f64::from_bits(0x43d3_2000_0000_0000) {
            return if negative { -PI_OVER_2_F } else { PI_OVER_2_F };
        }
        (PI_OVER_2_D, -1.0 / ax)
    };
    let z = t * t;
    let num = P2.mul_add(z, P1).mul_add(z, P0) * (t * z);
    let den = Q2.mul_add(z, Q1).mul_add(z, Q0);
    let r = hi - (num / den - t);
    (if negative { -r } else { r }) as f32
}
