//! `atan2f`: `atan` of the double-precision quotient `|y/x|` with the usual quadrant fix-ups.
//!
//! Measured on the platform (docs/design/libm-cleanroom.md, section "atan2f"): the quotient is
//! formed in double precision (a float quotient mismatches thousands of inputs); the same
//! five-interval structure as `atanf` applies, except that the correction is skipped
//! (`atan(q) = q`) when `q < 1e-4` or when the exponent of `y` is at least 14 below the exponent
//! of `x`. The rational core is fitted separately per reduction group because no single core
//! reproduces all measurements (the joint fit is infeasible), so this function is **not**
//! verified bit-exact: see the provenance log for the measured mismatch rate.

use core::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

const ATAN_HALF: f64 = 0.463_647_609_000_806_1;
const ATAN_ONE_AND_HALF: f64 = 0.982_793_723_247_329;

type RatCore = ([f64; 4], [f64; 3]);

/// Rational cores `w*P(w^2)/Q(w^2)`, lowest order first: direct (`q < 7/16`), the three offset
/// intervals, and `q >= 39/16` (reciprocal).
const DIRECT: RatCore = (
    [
        1.0,
        0.913_422_567_253_340_9,
        0.119_600_812_985_323_73,
        -0.005_231_801_487_560_064,
    ],
    [1.0, 1.246_755_900_068_867, 0.335_186_196_367_187_1],
);
const OFFSET: RatCore = (
    [
        1.0,
        0.174_171_131_003_698_71,
        -0.155_072_622_333_945_94,
        0.020_678_305_164_974_653,
    ],
    [1.0, 0.507_504_460_186_180_2, -0.185_903_407_694_402_75],
);
const RECIPROCAL: RatCore = (
    [
        1.0,
        0.918_381_100_213_521_8,
        0.121_420_194_261_244_64,
        -0.005_398_835_297_105_257,
    ],
    [1.0, 1.251_714_433_557_256_8, 0.338_658_372_726_046_04],
);

fn core(c: &RatCore, w: f64) -> f64 {
    let u = w * w;
    let p = c.0.iter().rev().fold(0.0, |s, &k| s * u + k);
    let q = c.1.iter().rev().fold(0.0, |s, &k| s * u + k);
    w * p / q
}

/// `atan(t)` for `t >= 0`.
fn atan_pos(t: f64) -> f64 {
    if t < 0.4375 {
        core(&DIRECT, t)
    } else if t < 0.6875 {
        ATAN_HALF + core(&OFFSET, (t - 0.5) / (1.0 + 0.5 * t))
    } else if t < 1.1875 {
        FRAC_PI_4 + core(&OFFSET, (t - 1.0) / (t + 1.0))
    } else if t < 2.4375 {
        ATAN_ONE_AND_HALF + core(&OFFSET, (t - 1.5) / (1.0 + 1.5 * t))
    } else {
        FRAC_PI_2 - core(&RECIPROCAL, 1.0 / t)
    }
}

/// Arc tangent of `y/x` (quadrant-correct) as the Windows UCRT computes it.
#[must_use]
pub fn atan2f(y: f32, x: f32) -> f32 {
    if x.is_nan() || y.is_nan() {
        // The platform returns x (quieted) when both are NaN.
        let n = if x.is_nan() { x } else { y };
        return f32::from_bits(n.to_bits() | 0x0040_0000);
    }
    let (yd, xd) = (f64::from(y), f64::from(x));
    let both_inf = y.is_infinite() && x.is_infinite();
    let a = if both_inf {
        FRAC_PI_4
    } else {
        let q = (yd / xd).abs();
        let q = if q.is_nan() { 0.0 } else { q }; // 0/0
        let exp_diff = i32::from(((y.to_bits() >> 23) & 0xFF) as u16)
            - i32::from(((x.to_bits() >> 23) & 0xFF) as u16);
        if q < 1e-4 || exp_diff <= -14 {
            q
        } else {
            atan_pos(q)
        }
    };
    let v = if x.is_sign_negative() {
        if both_inf { 3.0 * a } else { PI - a }
    } else {
        a
    };
    (v as f32).copysign(y)
}

#[cfg(test)]
mod tests {
    use super::atan2f;

    #[test]
    fn quadrants_and_specials() {
        let pi = core::f32::consts::PI;
        assert_eq!(atan2f(0.0, 1.0).to_bits(), 0);
        assert_eq!(atan2f(-0.0, 1.0).to_bits(), 0x8000_0000);
        assert_eq!(atan2f(0.0, -1.0), pi);
        assert_eq!(atan2f(-0.0, -0.0), -pi);
        assert_eq!(atan2f(1.0, 0.0), core::f32::consts::FRAC_PI_2);
        assert_eq!(
            atan2f(f32::INFINITY, f32::INFINITY),
            core::f32::consts::FRAC_PI_4
        );
        assert!((atan2f(1.0, -1.0) - 3.0 * pi / 4.0).abs() < 1e-6);
        assert!(atan2f(f32::NAN, 1.0).is_nan());
    }
}
