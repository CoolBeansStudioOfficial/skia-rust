// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Per-lane reference models of the A64 Advanced SIMD instructions the `Neon` lane primitives
//! use (design §2.8), shared by the `model_neon` instantiations.
//!
//! Each function computes one lane exactly as the Arm ARM's pseudocode defines it, under the
//! `FPCR` the tier runs with (Linux/macOS default: `DN = 0`, `FZ = 0`, `AH = 0`, round to
//! nearest even), including NaN results:
//!
//! - `FPProcessNaNs`: the first *signalling* NaN operand (in operand order) wins, else the first
//!   quiet NaN; the chosen NaN is returned quieted, sign and payload kept;
//! - invalid operations (`0 * inf`, `inf - inf`, `sqrt(-1)`, …) return the *default NaN*
//!   `0x7FC00000` (positive, unlike x86's `0xFFC00000`);
//! - `FNEG`/`FABS` only flip/clear the sign bit, also of NaNs (`AH = 0`);
//! - conversions to integers saturate, and NaN converts to 0.
//!
//! Like [`x86_model`](super::x86_model), these never let a NaN come out of Rust arithmetic (whose
//! NaN payloads are unspecified).

use crate::estimates::arm;
use crate::tier::Estimates;

/// `FPDefaultNaN()` (`0x7FC00000`).
pub const DEFAULT_NAN: f32 = f32::from_bits(0x7fc0_0000);

/// Whether `x` is a signalling NaN.
#[inline]
fn is_snan(x: f32) -> bool {
    x.is_nan() && x.to_bits() & 0x0040_0000 == 0
}

/// `FPProcessNaN`: `x` (a NaN) with its quiet bit set.
#[inline]
#[must_use]
pub fn quiet(x: f32) -> f32 {
    f32::from_bits(x.to_bits() | 0x0040_0000)
}

/// `FPProcessNaNs` over `ops` in operand order: the first signalling NaN quieted, else the first
/// quiet NaN, else `None`.
#[inline]
fn process_nans(ops: &[f32]) -> Option<f32> {
    ops.iter()
        .find(|&&x| is_snan(x))
        .or_else(|| ops.iter().find(|x| x.is_nan()))
        .map(|&x| quiet(x))
}

/// A two-operand arithmetic instruction with exact result `r` (only used when neither operand
/// is NaN; a NaN `r` means an invalid operation).
#[inline]
fn arith(a: f32, b: f32, r: impl FnOnce(f32, f32) -> f32) -> f32 {
    process_nans(&[a, b]).unwrap_or_else(|| {
        let r = r(a, b);
        if r.is_nan() { DEFAULT_NAN } else { r }
    })
}

/// `FADD a, b`.
#[inline]
#[must_use]
pub fn fadd(a: f32, b: f32) -> f32 {
    arith(a, b, |a, b| a + b)
}

/// `FSUB a, b`.
#[inline]
#[must_use]
pub fn fsub(a: f32, b: f32) -> f32 {
    arith(a, b, |a, b| a - b)
}

/// `FMUL a, b`.
#[inline]
#[must_use]
pub fn fmul(a: f32, b: f32) -> f32 {
    arith(a, b, |a, b| a * b)
}

/// `FNEG`: flips the sign bit (of NaNs too).
#[inline]
#[must_use]
pub fn fneg(x: f32) -> f32 {
    f32::from_bits(x.to_bits() ^ 0x8000_0000)
}

/// `FABS`: clears the sign bit (of NaNs too; no NaN processing).
#[inline]
#[must_use]
pub fn fabs(x: f32) -> f32 {
    f32::from_bits(x.to_bits() & 0x7fff_ffff)
}

/// `FPMulAdd(addend, op1, op2)`: `addend + op1*op2` with a single rounding (`FMLA`,
/// `vfmaq_f32(addend, op1, op2)`). NaN operands are processed in the order `addend, op1, op2`;
/// a quiet NaN addend with `0 * inf` gives the default NaN.
#[inline]
#[must_use]
pub fn fmla(addend: f32, op1: f32, op2: f32) -> f32 {
    let inf_times_zero = (op1.is_infinite() && op2 == 0.0) || (op1 == 0.0 && op2.is_infinite());
    if let Some(nan) = process_nans(&[addend, op1, op2]) {
        if addend.is_nan() && !is_snan(addend) && inf_times_zero {
            return DEFAULT_NAN;
        }
        return nan;
    }
    // Without NaN operands FPMulAdd is IEEE fusedMultiplyAdd (including the signs of exact
    // zeros under round to nearest), which `mul_add` computes on every host.
    let r = op1.mul_add(op2, addend);
    if r.is_nan() { DEFAULT_NAN } else { r }
}

/// `FMLS`: `FPMulAdd(addend, FPNeg(op1), op2)` (`vfmsq_f32(addend, op1, op2)`).
#[inline]
#[must_use]
pub fn fmls(addend: f32, op1: f32, op2: f32) -> f32 {
    fmla(addend, fneg(op1), op2)
}

/// `FPMin` (`FMIN`, `vminq_f32`): NaN operands propagate (`FPProcessNaNs`); `-0 < +0`.
#[inline]
#[must_use]
pub fn fmin(a: f32, b: f32) -> f32 {
    if let Some(nan) = process_nans(&[a, b]) {
        nan
    } else if a == 0.0 && b == 0.0 {
        // FPZero(sign1 OR sign2)
        f32::from_bits((a.to_bits() | b.to_bits()) & 0x8000_0000)
    } else if a < b {
        a
    } else {
        b
    }
}

/// `FPMax` (`FMAX`, `vmaxq_f32`): NaN operands propagate; `-0 < +0`.
#[inline]
#[must_use]
pub fn fmax(a: f32, b: f32) -> f32 {
    if let Some(nan) = process_nans(&[a, b]) {
        nan
    } else if a == 0.0 && b == 0.0 {
        // FPZero(sign1 AND sign2)
        f32::from_bits(a.to_bits() & b.to_bits() & 0x8000_0000)
    } else if a > b {
        a
    } else {
        b
    }
}

/// `FPSqrt` (`FSQRT`, `vsqrtq_f32`): exact; `sqrt(-0) = -0`; other negatives → default NaN.
#[inline]
#[must_use]
pub fn fsqrt(x: f32) -> f32 {
    if x.is_nan() {
        quiet(x)
    } else if x < 0.0 {
        DEFAULT_NAN
    } else {
        x.sqrt()
    }
}

/// `FRINTM` (`vrndmq_f32`): round toward minus infinity (`floor(-0) = -0`).
#[inline]
#[must_use]
pub fn frintm(x: f32) -> f32 {
    if x.is_nan() { quiet(x) } else { x.floor() }
}

/// `FRINTP` (`vrndpq_f32`): round toward plus infinity (`ceil(-0.5) = -0`).
#[inline]
#[must_use]
pub fn frintp(x: f32) -> f32 {
    if x.is_nan() { quiet(x) } else { x.ceil() }
}

/// `FPRecipStepFused(op1, op2)` (`FRECPS`, `vrecpsq_f32`): `2 - op1*op2` with a single rounding;
/// `0 * inf` gives `2.0`. `op1` is negated *before* NaN processing, so a NaN `op1` comes out
/// with its sign flipped.
#[inline]
#[must_use]
pub fn frecps(op1: f32, op2: f32) -> f32 {
    let op1 = fneg(op1);
    if let Some(nan) = process_nans(&[op1, op2]) {
        nan
    } else if (op1.is_infinite() && op2 == 0.0) || (op1 == 0.0 && op2.is_infinite()) {
        2.0
    } else if op1.is_infinite() || op2.is_infinite() {
        // FPInfinity(sign1 EOR sign2)
        f32::from_bits(0x7f80_0000 | ((op1.to_bits() ^ op2.to_bits()) & 0x8000_0000))
    } else {
        // 2 + op1*op2 rounded once; an exact zero is +0 (round to nearest), as `mul_add` gives.
        op1.mul_add(op2, 2.0)
    }
}

/// `FPRSqrtStepFused(op1, op2)` (`FRSQRTS`, `vrsqrtsq_f32`): `(3 - op1*op2) / 2` with a single
/// rounding; `0 * inf` gives `1.5`. `op1` is negated before NaN processing.
#[inline]
#[must_use]
pub fn frsqrts(op1: f32, op2: f32) -> f32 {
    let op1 = fneg(op1);
    if let Some(nan) = process_nans(&[op1, op2]) {
        nan
    } else if (op1.is_infinite() && op2 == 0.0) || (op1 == 0.0 && op2.is_infinite()) {
        1.5
    } else if op1.is_infinite() || op2.is_infinite() {
        f32::from_bits(0x7f80_0000 | ((op1.to_bits() ^ op2.to_bits()) & 0x8000_0000))
    } else if op1.abs() >= f32::from_bits(0x0100_0000) {
        // |op1| >= 2^-125, so op1/2 is exact and (3 + op1*op2)/2 = 1.5 + (op1/2)*op2 is computed
        // with one rounding (also where 3 + op1*op2 alone would overflow).
        (op1 * 0.5).mul_add(op2, 1.5)
    } else {
        // |op1*op2| < 8: 3 + op1*op2 rounds once to a value in (-5, 11) whose halving is exact.
        op1.mul_add(op2, 3.0) * 0.5
    }
}

/// `FCVTNS` (`vcvtnq_s32_f32`): round to nearest even, saturate, NaN → 0.
#[inline]
#[must_use]
pub fn fcvtns(x: f32) -> i32 {
    #[allow(clippy::cast_possible_truncation)] // `as` saturates and maps NaN to 0, like FCVTNS
    let r = x.round_ties_even() as i32;
    r
}

/// `FCVTNU` (`vcvtnq_u32_f32`): round to nearest even, saturate to `[0, u32::MAX]`, NaN → 0.
#[inline]
#[must_use]
pub fn fcvtnu(x: f32) -> u32 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // saturating, like FCVTNU
    let r = x.round_ties_even() as u32;
    r
}

/// `FCVTZS` (`vcvtq_s32_f32`, any float → `I32` vector cast): truncate, saturate, NaN → 0.
#[inline]
#[must_use]
pub fn fcvtzs(x: f32) -> i32 {
    #[allow(clippy::cast_possible_truncation)] // `as` saturates and maps NaN to 0, like FCVTZS
    let r = x as i32;
    r
}

/// `SCVTF` (`vcvtq_f32_s32`): round to nearest even.
#[inline]
#[must_use]
pub fn scvtf(i: i32) -> f32 {
    #[allow(clippy::cast_precision_loss)] // SCVTF rounds to nearest even, like `as`
    let f = i as f32;
    f
}

/// `FCVT` half → single (`vcvt_f32_f16`): exact; half denormals become normal floats; a NaN
/// is quieted with its payload moved to the top fraction bits.
#[must_use]
pub fn fcvt_f32_f16(h: u16) -> f32 {
    let sign = u32::from(h & 0x8000) << 16;
    let exp = u32::from(h >> 10) & 0x1f;
    let frac = u32::from(h & 0x03ff);
    let bits = match (exp, frac) {
        (0, 0) => sign,
        // A half denormal is frac * 2^-24: exact in f32 (a normal number), sign attached.
        (0, _) => {
            let two_m24 = f32::from_bits(0x3380_0000);
            #[allow(clippy::cast_precision_loss)] // frac < 1024: exact
            let value = frac as f32 * two_m24;
            sign | value.to_bits()
        }
        (0x1f, 0) => sign | 0x7f80_0000,
        // FPConvertNaN: sign : Ones(8) : '1' : frac<8:0> : Zeros(13).
        (0x1f, _) => sign | 0x7fc0_0000 | (frac << 13),
        _ => sign | ((exp + 112) << 23) | (frac << 13),
    };
    f32::from_bits(bits)
}

/// `FCVTN` single → half (`vcvt_f16_f32`): round to nearest even, half denormals produced,
/// overflow → `±inf`; a NaN is quieted and keeps the top 9 payload bits.
#[must_use]
pub fn fcvt_f16_f32(x: f32) -> u16 {
    let bits = x.to_bits();
    #[allow(clippy::cast_possible_truncation)] // bit 15 only
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = (bits >> 23) & 0xff;
    let frac = bits & 0x007f_ffff;
    let mag: u32 = if exp == 0xff {
        if frac == 0 {
            0x7c00
        } else {
            // FPConvertNaN: sign : Ones(5) : '1' : frac<21:13>.
            0x7e00 | (frac >> 13)
        }
    } else if exp < 113 {
        // Below 2^-14: a half denormal (or zero), in units of 2^-24. Scaling by 2^24 is exact,
        // and rounding 1024 up gives the smallest normal half's encoding.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // in 0..=1024
        let q = (f32::from_bits(bits & 0x7fff_ffff) * 16_777_216.0).round_ties_even() as u32;
        q
    } else {
        let mut m = ((exp - 112) << 10) | (frac >> 13);
        let rem = frac & 0x1fff;
        if rem > 0x1000 || (rem == 0x1000 && m & 1 == 1) {
            m += 1;
        }
        m.min(0x7c00)
    };
    #[allow(clippy::cast_possible_truncation)] // at most 0x7fff
    let mag = mag as u16;
    sign | mag
}

/// `URSRA` + `URSHR` by 8 (`vrshrq_n_u16(vrsraq_n_u16(v, v, 8), 8)`): the rounding shifts are
/// computed without overflow, the accumulate wraps at 16 bits.
#[inline]
#[must_use]
pub fn div255(v: u16) -> u16 {
    // (x + 128) >> 8 of a 16-bit x fits in 9 bits.
    #[allow(clippy::cast_possible_truncation)]
    let rshr8 = |x: u16| ((u32::from(x) + 128) >> 8) as u16;
    rshr8(v.wrapping_add(rshr8(v)))
}

/// `SQRDMULH` (`vqrdmulhq_s16`): `(2*a*b + 2^15) >> 16`, saturated (only `-32768 * -32768`
/// saturates, to `32767`).
#[inline]
#[must_use]
pub fn sqrdmulh(a: i16, b: i16) -> i16 {
    let p = (2 * i64::from(a) * i64::from(b) + (1 << 15)) >> 16;
    i16::try_from(p).unwrap_or(i16::MAX)
}

/// `FRECPE` over `N` lanes, from `estimates`.
///
/// # Panics
/// With [`Estimates::Host`] on a host without NEON, and with [`Estimates::AmdZen4`] (both
/// rejected by `Selection::check`).
#[must_use]
pub fn frecpe<const N: usize>(estimates: Estimates, xs: [f32; N]) -> [f32; N] {
    match estimates {
        Estimates::Host => xs.map(|x| {
            arm::host_frecpe(x).unwrap_or_else(|| {
                panic!("Model(Estimates::Host) needs the host's FRECPE (Selection::check)")
            })
        }),
        Estimates::Arm => xs.map(arm::frecpe),
        Estimates::AmdZen4 => {
            unreachable!("AmdZen4 estimates exist only for the x86 tiers (Selection::check)")
        }
    }
}

/// `FRSQRTE` over `N` lanes, from `estimates`.
///
/// # Panics
/// As [`frecpe`].
#[must_use]
pub fn frsqrte<const N: usize>(estimates: Estimates, xs: [f32; N]) -> [f32; N] {
    match estimates {
        Estimates::Host => xs.map(|x| {
            arm::host_frsqrte(x).unwrap_or_else(|| {
                panic!("Model(Estimates::Host) needs the host's FRSQRTE (Selection::check)")
            })
        }),
        Estimates::Arm => xs.map(arm::frsqrte),
        Estimates::AmdZen4 => {
            unreachable!("AmdZen4 estimates exist only for the x86 tiers (Selection::check)")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(x: f32) -> u32 {
        x.to_bits()
    }

    #[test]
    fn nan_rules() {
        let qa = f32::from_bits(0x7fc0_0001);
        let sb = f32::from_bits(0xff80_0002); // signalling, negative
        // A signalling NaN wins over an earlier quiet one; x86 would return `qa`.
        assert_eq!(fadd(qa, sb).to_bits(), 0xffc0_0002);
        assert_eq!(fadd(sb, qa).to_bits(), 0xffc0_0002);
        assert_eq!(fmul(qa, 1.0).to_bits(), 0x7fc0_0001);
        assert_eq!(fmul(0.0, f32::INFINITY).to_bits(), 0x7fc0_0000);
        assert_eq!(fsub(f32::INFINITY, f32::INFINITY).to_bits(), 0x7fc0_0000);
        assert_eq!(fsqrt(-1.0).to_bits(), 0x7fc0_0000);
        assert_eq!(fsqrt(-0.0).to_bits(), 0x8000_0000);
        assert_eq!(fmin(f32::NAN, 1.0).to_bits(), 0x7fc0_0000);
        assert_eq!(fmin(1.0, sb).to_bits(), 0xffc0_0002);
        assert_eq!(fmin(-0.0, 0.0).to_bits(), 0x8000_0000);
        assert_eq!(fmin(0.0, -0.0).to_bits(), 0x8000_0000);
        assert_eq!(fmax(-0.0, 0.0).to_bits(), 0);
        assert_eq!(fabs(f32::from_bits(0xff80_0001)).to_bits(), 0x7f80_0001);
        assert_eq!(fneg(f32::from_bits(0x7fc0_0000)).to_bits(), 0xffc0_0000);
    }

    #[test]
    fn fused() {
        // -1 + (1 + 2^-23)(1 - 2^-23) = -2^-46 exactly: only the fused form keeps it.
        let (a, b) = (1.0 + f32::EPSILON, 1.0 - f32::EPSILON);
        assert_eq!(fmla(-1.0, a, b).to_bits(), f(-f32::EPSILON * f32::EPSILON));
        assert_eq!(fmls(1.0, a, b).to_bits(), f(f32::EPSILON * f32::EPSILON));
        // NaN order: addend, op1, op2; signalling first.
        let (q1, q2, s3) = (
            f32::from_bits(0x7fc0_0001),
            f32::from_bits(0x7fc0_0002),
            f32::from_bits(0x7f80_0003),
        );
        assert_eq!(fmla(q1, q2, s3).to_bits(), 0x7fc0_0003);
        assert_eq!(fmla(q1, q2, 1.0).to_bits(), 0x7fc0_0001);
        assert_eq!(fmla(1.0, q2, q1).to_bits(), 0x7fc0_0002);
        // A quiet NaN addend with 0 * inf is invalid: the default NaN.
        assert_eq!(fmla(q1, 0.0, f32::INFINITY).to_bits(), 0x7fc0_0000);
        assert_eq!(fmla(1.0, 0.0, f32::INFINITY).to_bits(), 0x7fc0_0000);
        assert_eq!(
            fmla(f32::NEG_INFINITY, f32::INFINITY, 1.0).to_bits(),
            0x7fc0_0000
        );
        // FMLS negates op1, NaN or not.
        assert_eq!(fmls(1.0, q2, 1.0).to_bits(), 0xffc0_0002);
        assert_eq!(fmla(-0.0, 0.0, -1.0).to_bits(), 0x8000_0000);
        assert_eq!(fmla(0.0, 0.0, -1.0).to_bits(), 0);
    }

    #[test]
    fn steps() {
        assert_eq!(frecps(0.0, f32::INFINITY), 2.0);
        assert_eq!(frecps(2.0, 0.5), 1.0);
        assert_eq!(frecps(1.0, 2.0).to_bits(), 0);
        assert_eq!(frecps(1.0, f32::INFINITY), f32::NEG_INFINITY);
        // op1 is negated before NaN processing.
        assert_eq!(
            frecps(f32::from_bits(0x7f80_0001), 1.0).to_bits(),
            0xffc0_0001
        );
        assert_eq!(
            frecps(1.0, f32::from_bits(0x7f80_0001)).to_bits(),
            0x7fc0_0001
        );
        assert_eq!(frsqrts(0.0, f32::INFINITY), 1.5);
        assert_eq!(frsqrts(1.0, 1.0), 1.0);
        assert_eq!(frsqrts(3.0, 1.0).to_bits(), 0);
        assert_eq!(frsqrts(-1.0, f32::INFINITY), f32::INFINITY);
        // (3 + 2^127 * 3) / 2 is finite although 3 + 2^127 * 3 overflows.
        let big = f32::from_bits(0x7f00_0000); // 2^127
        assert_eq!(frsqrts(-3.0, big), 1.5 * big);
        // Tiny op1: the other branch.
        let tiny = f32::from_bits(0x0040_0000); // 2^-127
        assert_eq!(frsqrts(tiny, big), 1.0);
    }

    #[test]
    fn conversions() {
        assert_eq!(fcvtns(2.5), 2);
        assert_eq!(fcvtns(-2.5), -2);
        assert_eq!(fcvtns(3e9), i32::MAX);
        assert_eq!(fcvtns(-3e9), i32::MIN);
        assert_eq!(fcvtns(f32::NAN), 0);
        assert_eq!(fcvtnu(-0.7), 0);
        assert_eq!(fcvtnu(254.5), 254);
        assert_eq!(fcvtnu(5e9), u32::MAX);
        assert_eq!(fcvtnu(f32::NAN), 0);
        assert_eq!(fcvtzs(-1.9), -1);
        assert_eq!(fcvtzs(f32::NEG_INFINITY), i32::MIN);
        assert_eq!(scvtf(16_777_217), 16_777_216.0);
        assert_eq!(div255(255 * 255), 255);
        assert_eq!(div255(127), 0);
        assert_eq!(div255(128), 1);
        assert_eq!(div255(0xffff), 1); // 0xffff + 256 wraps to 255, (255 + 128) >> 8 = 1
        assert_eq!(sqrdmulh(i16::MIN, i16::MIN), i16::MAX);
        assert_eq!(sqrdmulh(0x4000, 0x4000), 0x2000);
        assert_eq!(sqrdmulh(-1, 1), 0);
    }

    #[test]
    fn half() {
        assert_eq!(fcvt_f32_f16(0x3c00), 1.0);
        assert_eq!(fcvt_f32_f16(0x0001).to_bits(), 0x3380_0000); // 2^-24
        assert_eq!(fcvt_f32_f16(0x83ff).to_bits(), 0xb87f_c000); // -1023 * 2^-24
        assert_eq!(fcvt_f32_f16(0x7c00), f32::INFINITY);
        assert_eq!(fcvt_f32_f16(0xfc01).to_bits(), 0xffc0_2000);
        assert_eq!(fcvt_f32_f16(0x7e00).to_bits(), 0x7fc0_0000);
        assert_eq!(fcvt_f16_f32(1.0), 0x3c00);
        assert_eq!(fcvt_f16_f32(-0.0), 0x8000);
        assert_eq!(fcvt_f16_f32(65504.0), 0x7bff);
        assert_eq!(fcvt_f16_f32(65519.996), 0x7bff);
        assert_eq!(fcvt_f16_f32(65520.0), 0x7c00); // ties to even: overflow
        assert_eq!(fcvt_f16_f32(f32::MAX), 0x7c00);
        assert_eq!(fcvt_f16_f32(f32::from_bits(0x3f80_1000)), 0x3c00); // tie, even stays
        assert_eq!(fcvt_f16_f32(f32::from_bits(0x3f80_3000)), 0x3c02); // tie, odd rounds up
        assert_eq!(fcvt_f16_f32(f32::from_bits(0x3f80_1001)), 0x3c01);
        assert_eq!(fcvt_f16_f32(5.960_464_5e-8), 0x0001);
        assert_eq!(fcvt_f16_f32(2.980_232_2e-8), 0x0000); // 2^-25: tie to even 0
        assert_eq!(fcvt_f16_f32(-4.470_348_4e-8), 0x8001); // 1.5 * 2^-25 rounds up
        // 2047 * 2^-25 = 1023.5 half denormal units: rounds (to even) into the normals.
        assert_eq!(fcvt_f16_f32(f32::from_bits(0x387f_e000)), 0x0400);
        assert_eq!(fcvt_f16_f32(f32::from_bits(0x7f80_0001)), 0x7e00);
        assert_eq!(fcvt_f16_f32(f32::from_bits(0xffc0_2000)), 0xfe01);
        for h in 0..=u16::MAX {
            let x = fcvt_f32_f16(h);
            let back = fcvt_f16_f32(x);
            let want = if x.is_nan() { h | 0x0200 } else { h };
            assert_eq!(back, want, "{h:#06x}");
        }
    }
}
