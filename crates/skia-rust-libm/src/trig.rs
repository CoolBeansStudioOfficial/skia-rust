//! `sinf` / `cosf`: truncated Taylor series evaluated in double precision after a reduction
//! modulo pi/2, rounded once to `f32`.
//!
//! Starting point: the Maclaurin series of sine and cosine (any calculus text). The measured
//! facts that pin it down (docs/design/libm-cleanroom.md, section "sinf/cosf"):
//! * `|x| <= pi/4`: sine is the degree-9 and cosine the degree-10 series, Horner in `f64`.
//! * `|x| < 2^-7`: sine is `x - x^3/6` and cosine is `1 - x^2/2` (shorter series).
//! * Larger `|x|`: reduce to `r` in `[-pi/4, pi/4]`, then the full series (no short forms).

use crate::reduce::reduce_pio2;

const PIO4: f64 = core::f64::consts::FRAC_PI_4;

// Maclaurin coefficients (-1)^k/(2k+1)! and (-1)^k/(2k)!, correctly rounded to f64.
const S1: f64 = -0.166_666_666_666_666_66;
const S2: f64 = 0.008_333_333_333_333_333;
const S3: f64 = -0.000_198_412_698_412_698_4;
const S4: f64 = 2.755_731_922_398_589_3e-6;
const C1: f64 = -0.5;
const C2: f64 = 0.041_666_666_666_666_664;
const C3: f64 = -0.001_388_888_888_888_889;
const C4: f64 = 2.480_158_730_158_73e-5;
const C5: f64 = -2.755_731_922_398_589e-7;

/// Below this magnitude the shorter series are used.
const SHORT: f64 = 1.0 / 128.0;

fn sin_series(x: f64) -> f64 {
    let z = x * x;
    ((((S4 * z + S3) * z + S2) * z + S1) * z + 1.0) * x
}

fn cos_series(x: f64) -> f64 {
    let z = x * x;
    (((((C5 * z + C4) * z + C3) * z + C2) * z + C1) * z) + 1.0
}

/// The result when `x` is not finite: `NaN`s propagate, infinities give the default NaN.
fn non_finite(x: f32) -> f32 {
    if x.is_nan() {
        x
    } else {
        f32::from_bits(0xFFC0_0000)
    }
}

/// Sine of `x` as the Windows UCRT computes it.
#[must_use]
#[allow(clippy::cast_possible_truncation)] // the double result is rounded to float once, as the CRT does
pub fn sinf(x: f32) -> f32 {
    if !x.is_finite() {
        return non_finite(x);
    }
    let xd = f64::from(x);
    let ax = xd.abs();
    if ax <= PIO4 {
        if ax < SHORT {
            return (xd - ((xd * xd) * xd) * (1.0 / 6.0)) as f32;
        }
        return sin_series(xd) as f32;
    }
    let (n, r) = reduce_pio2(xd);
    (match n {
        0 => sin_series(r),
        1 => cos_series(r),
        2 => -sin_series(r),
        _ => -cos_series(r),
    }) as f32
}

/// Cosine of `x` as the Windows UCRT computes it.
#[must_use]
#[allow(clippy::cast_possible_truncation)] // the double result is rounded to float once, as the CRT does
pub fn cosf(x: f32) -> f32 {
    if !x.is_finite() {
        return non_finite(x);
    }
    let xd = f64::from(x);
    let ax = xd.abs();
    if ax <= PIO4 {
        if ax < SHORT {
            return (1.0 - (0.5 * xd) * xd) as f32;
        }
        return cos_series(xd) as f32;
    }
    let (n, r) = reduce_pio2(xd);
    (match n {
        0 => cos_series(r),
        1 => -sin_series(r),
        2 => -cos_series(r),
        _ => sin_series(r),
    }) as f32
}

#[cfg(test)]
mod tests {
    use super::{cosf, sinf};

    /// Within a few ulps of the host libm over a sweep that exercises every reduction path.
    #[test]
    fn close_to_host() {
        let mut x = 1.0e-3_f32;
        while x < 3.0e38 {
            for s in [1.0f32, -1.0] {
                let v = x * s;
                let (a, b) = (sinf(v), v.sin());
                assert!(
                    (a - b).abs() <= 4.0 * f32::EPSILON * b.abs().max(1e-30),
                    "sinf({v}) = {a} vs {b}"
                );
                let (a, b) = (cosf(v), v.cos());
                assert!(
                    (a - b).abs() <= 4.0 * f32::EPSILON * b.abs().max(1e-30),
                    "cosf({v}) = {a} vs {b}"
                );
            }
            x *= 1.0371;
        }
    }

    #[test]
    fn basic_values() {
        assert_eq!(sinf(0.0).to_bits(), 0);
        assert_eq!(cosf(0.0), 1.0);
        assert!((sinf(1.0) - 0.841_471).abs() < 1e-6);
        assert!((cosf(1.0) - 0.540_302_3).abs() < 1e-6);
        assert!((sinf(100.0) - (-0.506_365_64)).abs() < 1e-6);
    }
}
