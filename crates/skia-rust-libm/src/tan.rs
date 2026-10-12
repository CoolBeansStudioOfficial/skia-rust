//! `tanf`: a rational approximation `x*P(x^2)/Q(x^2)` evaluated in double precision, with the
//! same reduction modulo pi/2 as `sinf`/`cosf` and `-1/tan(r)` for odd quadrants.
//!
//! The coefficients are not from a published source: they were fitted to the platform's
//! observed float results (docs/design/libm-cleanroom.md, section "tanf"): the platform's
//! error against the true tangent is a smooth oscillating function, a linear program over the
//! rounding intervals of near-midpoint inputs found the rational shape (numerator and
//! denominator of degree 2 in x^2, numerator constant term 1) and a coefficient set that reproduces every one of the
//! 29,954,012 floats in [1/16, pi/4] bit for bit.

use crate::trig::{DOUBLE_N_LIMIT, non_finite, reduce};

const PIO4: f64 = core::f64::consts::FRAC_PI_4;

// Numerator and denominator coefficients in x^2 (degree 2 each), lowest order first.
const A: [f64; 3] = [1.0, -0.111_316_113_748_408_3, 0.001_072_028_017_678_077_6];
const B: [f64; 3] = [1.0, -0.444_649_446_921_710_05, 0.015_955_169_808_454_586];

/// `(x*P(x^2), Q(x^2))`, whose quotient is the tangent for `|x| <= pi/4`.
fn parts(x: f64) -> (f64, f64) {
    let u = x * x;
    let p = A.iter().rev().fold(0.0, |s, &c| s * u + c);
    let q = B.iter().rev().fold(0.0, |s, &c| s * u + c);
    (x * p, q)
}

/// Reduced argument, parity of the quadrant and the double-precision result before the final
/// rounding to `f32`.
fn trace(x: f32) -> (f64, bool, f64) {
    let xd = f64::from(x);
    if xd.abs() <= PIO4 {
        let (num, den) = parts(xd);
        return (xd, false, num / den);
    }
    let (n, r) = reduce(xd, DOUBLE_N_LIMIT, false);
    let (num, den) = parts(r);
    if n & 1 == 0 {
        (r, false, num / den)
    } else {
        (r, true, -1.0 / (num / den))
    }
}

/// Tangent of `x` as the Windows UCRT computes it.
#[must_use]
pub fn tanf(x: f32) -> f32 {
    if !x.is_finite() {
        return non_finite(x);
    }
    trace(x).2 as f32
}

/// Internals of [`tanf`] for the black-box probe: `(reduced argument, odd quadrant, result before
/// rounding to f32)`. Only meaningful for finite `x`.
#[doc(hidden)]
#[must_use]
pub fn tanf_trace(x: f32) -> (f64, bool, f64) {
    trace(x)
}

#[cfg(test)]
mod tests {
    use super::tanf;

    #[test]
    fn close_to_host() {
        let mut x = 1.0e-3_f32;
        while x < 3.0e38 {
            for s in [1.0f32, -1.0] {
                let v = x * s;
                let (a, b) = (tanf(v), v.tan());
                assert!(
                    (a - b).abs() <= 8.0 * f32::EPSILON * b.abs().max(1e-30),
                    "tanf({v}) = {a} vs {b}"
                );
            }
            x *= 1.0371;
        }
        assert_eq!(tanf(0.0).to_bits(), 0);
        assert_eq!(tanf(-0.0).to_bits(), 0x8000_0000);
    }
}
