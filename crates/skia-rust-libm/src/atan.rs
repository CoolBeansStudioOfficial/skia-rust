//! `atanf`: the classic five-interval argument reduction of `atan` (the interval edges and the
//! reduction identities are textbook, as in fdlibm's `s_atan.c`) around a rational core
//! `w*P(w^2)/Q(w^2)` evaluated in double precision and rounded once to `f32`.
//!
//! The core's coefficients are not from a published source: they were fitted to the platform's
//! observed float results (docs/design/libm-cleanroom.md, section "atanf") with a linear program
//! over rounding intervals, after the measured error pattern showed this interval structure.

use core::f64::consts::{FRAC_PI_2, FRAC_PI_4};

const ATAN_HALF: f64 = 0.463_647_609_000_806_1;
const ATAN_ONE_AND_HALF: f64 = 0.982_793_723_247_329;

const A: [f64; 4] = [
    1.0,
    0.915_261_205_904_863_6,
    0.120_266_871_709_077_07,
    -0.005_291_239_218_447_625,
];
const B: [f64; 3] = [1.0, 1.248_594_539_087_201_3, 0.336_465_095_295_476_9];

/// Rational core coefficients (numerator, denominator in `w^2`, lowest order first).
pub(crate) struct Core {
    pub(crate) a: &'static [f64],
    pub(crate) b: &'static [f64],
}

/// The core used by `atanf`.
pub(crate) const ATANF_CORE: Core = Core { a: &A, b: &B };

impl Core {
    /// `atan(w)` for `0 <= w < 7/16`.
    fn eval(&self, w: f64) -> f64 {
        let u = w * w;
        let p = self.a.iter().rev().fold(0.0, |s, &c| s * u + c);
        let q = self.b.iter().rev().fold(0.0, |s, &c| s * u + c);
        w * p / q
    }

    /// `atan(t)` for `t >= 0`, in double precision (five-interval reduction).
    pub(crate) fn atan_pos(&self, t: f64) -> f64 {
        if t < 0.4375 {
            self.eval(t)
        } else if t < 0.6875 {
            ATAN_HALF + self.eval((t - 0.5) / (1.0 + 0.5 * t))
        } else if t < 1.1875 {
            FRAC_PI_4 + self.eval((t - 1.0) / (t + 1.0))
        } else if t < 2.4375 {
            ATAN_ONE_AND_HALF + self.eval((t - 1.5) / (1.0 + 1.5 * t))
        } else {
            FRAC_PI_2 - self.eval(1.0 / t)
        }
    }
}

/// Arc tangent of `x` as the Windows UCRT computes it.
#[must_use]
pub fn atanf(x: f32) -> f32 {
    if x.is_nan() {
        return f32::from_bits(x.to_bits() | 0x0040_0000);
    }
    (ATANF_CORE.atan_pos(f64::from(x.abs())) as f32).copysign(x)
}

#[cfg(test)]
mod tests {
    use super::atanf;

    #[test]
    // The host libm is only a loose (few ulp) sanity reference here.
    #[allow(clippy::disallowed_methods)]
    fn close_to_host() {
        let mut x = 1.0e-6_f32;
        while x < 3.0e38 {
            for s in [1.0f32, -1.0] {
                let v = x * s;
                let (a, b) = (atanf(v), v.atan());
                assert!(
                    (a - b).abs() <= 2.0 * f32::EPSILON * b.abs(),
                    "atanf({v}) = {a} vs {b}"
                );
            }
            x *= 1.0371;
        }
        assert_eq!(atanf(f32::INFINITY), core::f32::consts::FRAC_PI_2);
        assert_eq!(atanf(-0.0).to_bits(), 0x8000_0000);
    }
}
