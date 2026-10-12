//! `atan2f`: `atan` of the double-precision quotient `|y/x|` with the usual quadrant fix-ups.
//!
//! Measured on the platform (docs/design/libm-cleanroom.md, section "atan2f"): the quotient is
//! formed in double precision (a float quotient mismatches thousands of inputs); the same
//! interval structure and core as `atanf` give the result, except that the correction is skipped
//! (`atan(q) = q`) when `q < 1e-4` or when the exponent of `y` is at least 14 below the exponent
//! of `x`.

use crate::atan::atan_pos;

const PI: f64 = core::f64::consts::PI;

/// Arc tangent of `y/x` (quadrant-correct) as the Windows UCRT computes it.
#[must_use]
pub fn atan2f(y: f32, x: f32) -> f32 {
    if x.is_nan() || y.is_nan() {
        // Which operand's payload wins is settled by the probe's special-value check.
        let n = if y.is_nan() { y } else { x };
        return f32::from_bits(n.to_bits() | 0x0040_0000);
    }
    let (yd, xd) = (f64::from(y), f64::from(x));
    let a = if y.is_infinite() && x.is_infinite() {
        core::f64::consts::FRAC_PI_4
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
    let both_inf = y.is_infinite() && x.is_infinite();
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
