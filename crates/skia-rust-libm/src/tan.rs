//! `tanf`: a rational approximation `x*P(x^2)/Q(x^2)` evaluated in double precision, with the
//! same reduction modulo pi/2 as `sinf`/`cosf` and `-1/tan(r)` for odd quadrants.
//!
//! The coefficients are not from a published source: they were fitted to the platform's
//! observed float results (docs/design/libm-cleanroom.md, section "tanf"): the platform's
//! error against the true tangent is a smooth oscillating function, a linear program over the
//! rounding intervals of 3.9e4 near-midpoint inputs found the rational shape (numerator degree 4,
//! denominator degree 2 in x^2) and one coefficient set that reproduces every one of the
//! 29,954,012 floats in [1/16, pi/4] bit for bit.

use crate::trig::{DOUBLE_N_LIMIT, non_finite, reduce};

const PIO4: f64 = core::f64::consts::FRAC_PI_4;

// Numerator (degree 4) and denominator (degree 2) coefficients in x^2, lowest order first.
const A: [f64; 5] = [
    0.999_999_999_999_987_9,
    -0.110_323_571_386_404_89,
    0.001_000_642_058_545_306,
    -1.738_525_356_594_785e-6,
    -6.634_005_686_130_02e-8,
];
const B: [f64; 3] = [1.0, -0.443_656_904_561_106_35, 0.015_552_936_430_052_375];

/// `(x*P(x^2), Q(x^2))`, whose quotient is the tangent for `|x| <= pi/4`.
fn parts(x: f64) -> (f64, f64) {
    let u = x * x;
    let p = (((A[4] * u + A[3]) * u + A[2]) * u + A[1]) * u + A[0];
    let q = (B[2] * u + B[1]) * u + B[0];
    (x * p, q)
}

/// Tangent of `x` as the Windows UCRT computes it.
#[must_use]
pub fn tanf(x: f32) -> f32 {
    if !x.is_finite() {
        return non_finite(x);
    }
    let xd = f64::from(x);
    if xd.abs() <= PIO4 {
        let (num, den) = parts(xd);
        return (num / den) as f32;
    }
    let (n, r) = reduce(xd, DOUBLE_N_LIMIT, false);
    let (num, den) = parts(r);
    (if n & 1 == 0 {
        num / den
    } else {
        -1.0 / (num / den)
    }) as f32
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
