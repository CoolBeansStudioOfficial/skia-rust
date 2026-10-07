// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Per-lane reference models of the x86 instructions the lane primitives use (design §2.8),
//! shared by the model tiers (`model_sse2`, `model_sse41`, and later `model_ml3`/`model_ml4`).
//!
//! Each function computes one lane exactly as the instruction does under the default MXCSR
//! (round to nearest even, no FTZ/DAZ), from the Intel SDM's definitions, including NaN results:
//!
//! - an SSE arithmetic instruction `op dst, src` returns the first NaN operand (`dst`, then
//!   `src`) quieted, and the *`QNaN` floating-point indefinite* `0xFFC00000` for invalid operations
//!   (`0 * inf`, `inf - inf`, `sqrt(-1)`, …);
//! - conversions to integers return the *integer indefinite* `0x80000000` for NaN and out-of-range
//!   inputs.
//!
//! Rust's own float arithmetic leaves NaN payloads unspecified (Miri randomizes them, and Arm
//! hosts pick NaNs differently), so these never let a NaN come out of Rust arithmetic.

use crate::estimates::{EstimateOp, host_estimates};
use crate::tier::Estimates;

/// The `QNaN` floating-point indefinite (`-nan`, bits `0xFFC00000`).
pub const INDEFINITE: f32 = f32::from_bits(0xFFC0_0000);

/// `x` with its quiet bit set (how x86 propagates a NaN operand).
#[inline]
#[must_use]
pub fn quiet(x: f32) -> f32 {
    f32::from_bits(x.to_bits() | 0x0040_0000)
}

/// An SSE binary arithmetic instruction `op a, b` with exact result `r` (only used when neither
/// operand is NaN).
#[inline]
fn arith(a: f32, b: f32, r: impl FnOnce(f32, f32) -> f32) -> f32 {
    if a.is_nan() {
        quiet(a)
    } else if b.is_nan() {
        quiet(b)
    } else {
        let r = r(a, b);
        if r.is_nan() { INDEFINITE } else { r }
    }
}

/// `addps a, b`.
#[inline]
#[must_use]
pub fn add(a: f32, b: f32) -> f32 {
    arith(a, b, |a, b| a + b)
}

/// `subps a, b`.
#[inline]
#[must_use]
pub fn sub(a: f32, b: f32) -> f32 {
    arith(a, b, |a, b| a - b)
}

/// `mulps a, b`.
#[inline]
#[must_use]
pub fn mul(a: f32, b: f32) -> f32 {
    arith(a, b, |a, b| a * b)
}

/// `divps a, b`.
#[inline]
#[must_use]
pub fn div(a: f32, b: f32) -> f32 {
    arith(a, b, |a, b| a / b)
}

/// `andps`.
#[inline]
#[must_use]
pub fn andps(a: f32, b: f32) -> f32 {
    f32::from_bits(a.to_bits() & b.to_bits())
}

/// `minps a, b`: `a < b ? a : b` (a NaN in either operand, or `±0` vs `∓0`, returns `b`).
#[inline]
#[must_use]
pub fn minps(a: f32, b: f32) -> f32 {
    if a < b { a } else { b }
}

/// `maxps a, b`: `a > b ? a : b` (a NaN in either operand, or `±0` vs `∓0`, returns `b`).
#[inline]
#[must_use]
pub fn maxps(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

/// `sqrtps`: exact; NaN → quieted NaN; negative (non-zero) → indefinite; `sqrt(-0) = -0`.
#[inline]
#[must_use]
pub fn sqrtps(x: f32) -> f32 {
    if x.is_nan() {
        quiet(x)
    } else if x < 0.0 {
        INDEFINITE
    } else {
        x.sqrt()
    }
}

/// `roundps` with `_MM_FROUND_FLOOR` (`_mm_floor_ps`).
#[inline]
#[must_use]
pub fn roundps_floor(x: f32) -> f32 {
    if x.is_nan() { quiet(x) } else { x.floor() }
}

/// `roundps` with `_MM_FROUND_CEIL` (`_mm_ceil_ps`).
#[inline]
#[must_use]
pub fn roundps_ceil(x: f32) -> f32 {
    if x.is_nan() { quiet(x) } else { x.ceil() }
}

/// An integral-valued float (or NaN) to `i32`, or the integer indefinite if out of range.
#[inline]
fn to_i32_or_indefinite(r: f64) -> i32 {
    if r.is_nan() || !(-2_147_483_648.0..2_147_483_648.0).contains(&r) {
        i32::MIN
    } else {
        #[allow(clippy::cast_possible_truncation)] // r is integral and in range (checked above)
        let i = r as i32;
        i
    }
}

/// `cvtps2dq`: round to nearest even (MXCSR default), NaN/out of range → `0x80000000`.
#[inline]
#[must_use]
pub fn cvtps2dq(x: f32) -> i32 {
    to_i32_or_indefinite(f64::from(x.round_ties_even()))
}

/// `cvttps2dq`: truncate, NaN/out of range → `0x80000000`.
#[inline]
#[must_use]
pub fn cvttps2dq(x: f32) -> i32 {
    to_i32_or_indefinite(f64::from(x.trunc()))
}

/// `cvttpd2dq` (one lane): truncate, NaN/out of range → `0x80000000`.
#[inline]
#[must_use]
pub fn cvttpd2dq(x: f64) -> i32 {
    to_i32_or_indefinite(x.trunc())
}

/// `cvtdq2ps`: round to nearest even.
#[inline]
#[must_use]
pub fn cvtdq2ps(i: i32) -> f32 {
    #[allow(clippy::cast_precision_loss)] // cvtdq2ps rounds to nearest even, like `as`
    let f = i as f32;
    f
}

/// `packssdw` (one lane): signed saturation of an `i32` to `i16`.
#[inline]
#[must_use]
pub fn packssdw(x: i32) -> i16 {
    #[allow(clippy::cast_possible_truncation)] // clamped to the i16 range first
    let r = x.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
    r
}

/// `packusdw` (one lane): unsigned saturation of an `i32` to `u16`.
#[inline]
#[must_use]
pub fn packusdw(x: i32) -> u16 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // clamped to u16 range
    let r = x.clamp(0, i32::from(u16::MAX)) as u16;
    r
}

/// `packuswb` (one lane): unsigned saturation of an `i16` to `u8`.
#[inline]
#[must_use]
pub fn packuswb(x: i16) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // clamped to u8 range
    let r = x.clamp(0, i16::from(u8::MAX)) as u8;
    r
}

/// `movmskps`: the lanes' sign bits, lane 0 in bit 0.
#[inline]
#[must_use]
pub fn movmskps<const N: usize>(c: [i32; N]) -> u32 {
    c.iter()
        .enumerate()
        .fold(0, |m, (i, &x)| m | (u32::from(x < 0) << i))
}

/// `pmulhrsw` (one lane), as the SDM defines it: `((a*b >> 14) + 1) >> 1`, low 16 bits.
#[inline]
#[must_use]
pub fn pmulhrsw(a: i16, b: i16) -> i16 {
    let t = ((i32::from(a) * i32::from(b)) >> 14) + 1;
    #[allow(clippy::cast_possible_truncation)] // the instruction keeps bits [16:1] of t
    let r = (t >> 1) as i16;
    r
}

/// The estimate instruction `op` over `xs`, from `estimates`.
fn estimate<const N: usize>(op: EstimateOp, estimates: Estimates, xs: [f32; N]) -> [f32; N] {
    match estimates {
        Estimates::Host => {
            let mut out = [0.0f32; N];
            host_estimates(op, &xs, &mut out).unwrap_or_else(|e| {
                panic!("Model(Estimates::Host) needs the host's estimates (Selection::check): {e}")
            });
            out
        }
        // TODO(A2d): the oracle host's `rcpps`/`rsqrtps` tables (4096/8192 entries + specials)
        // land with task A2d; then add a `model_*::amd_zen4` instantiation (design §2.8).
        Estimates::AmdZen4 => {
            panic!("Estimates::AmdZen4 tables are not available yet (task A2d)")
        }
        Estimates::Arm => {
            unreachable!("Arm estimates exist only for the Neon tier (Selection::check)")
        }
    }
}

/// `rcpps` over `N` lanes, from `estimates`.
#[must_use]
pub fn rcpps<const N: usize>(estimates: Estimates, xs: [f32; N]) -> [f32; N] {
    estimate(EstimateOp::Rcpps, estimates, xs)
}

/// `rsqrtps` over `N` lanes, from `estimates`.
#[must_use]
pub fn rsqrtps<const N: usize>(estimates: Estimates, xs: [f32; N]) -> [f32; N] {
    estimate(EstimateOp::Rsqrtps, estimates, xs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nan_rules() {
        let qa = f32::from_bits(0x7fc0_0001);
        let sb = f32::from_bits(0xff80_0002); // signalling, negative
        assert_eq!(add(qa, sb).to_bits(), 0x7fc0_0001);
        assert_eq!(add(sb, qa).to_bits(), 0xffc0_0002);
        assert_eq!(mul(0.0, f32::INFINITY).to_bits(), 0xffc0_0000);
        assert_eq!(sub(f32::INFINITY, f32::INFINITY).to_bits(), 0xffc0_0000);
        assert_eq!(sqrtps(-1.0).to_bits(), 0xffc0_0000);
        assert_eq!(sqrtps(-0.0).to_bits(), 0x8000_0000);
        assert_eq!(minps(f32::NAN, 1.0), 1.0);
        assert!(minps(1.0, f32::NAN).is_nan());
        assert_eq!(minps(-0.0, 0.0).to_bits(), 0);
        assert_eq!(maxps(0.0, -0.0).to_bits(), 0x8000_0000);
    }

    #[test]
    fn conversions() {
        assert_eq!(cvtps2dq(2.5), 2);
        assert_eq!(cvtps2dq(-2.5), -2);
        assert_eq!(cvtps2dq(3.5), 4);
        assert_eq!(cvtps2dq(2_147_483_520.0), 2_147_483_520);
        assert_eq!(cvtps2dq(2_147_483_648.0), i32::MIN);
        assert_eq!(cvtps2dq(-2_147_483_648.0), i32::MIN);
        assert_eq!(cvtps2dq(f32::NAN), i32::MIN);
        assert_eq!(cvttps2dq(-1.9), -1);
        assert_eq!(cvttps2dq(f32::NEG_INFINITY), i32::MIN);
        assert_eq!(cvttpd2dq(2_147_483_647.9), i32::MAX);
        assert_eq!(cvttpd2dq(2_147_483_648.0), i32::MIN);
        assert_eq!(cvttpd2dq(-2_147_483_648.9), i32::MIN);
        assert_eq!(packssdw(70_000), i16::MAX);
        assert_eq!(packssdw(-70_000), i16::MIN);
        assert_eq!(packusdw(-1), 0);
        assert_eq!(packusdw(70_000), u16::MAX);
        assert_eq!(packuswb(-1), 0);
        assert_eq!(packuswb(256), 255);
        assert_eq!(movmskps([-1, 0, i32::MIN, 1]), 0b0101);
        assert_eq!(pmulhrsw(i16::MIN, i16::MIN), i16::MIN);
        assert_eq!(pmulhrsw(16384, 16384), 8192);
        assert_eq!(pmulhrsw(-1, 1), 0);
    }
}
