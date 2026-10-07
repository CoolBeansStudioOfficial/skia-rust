// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Arm's `FRECPE`/`FRSQRTE` estimates: what model tiers use for
//! [`Estimates::Arm`](crate::Estimates::Arm) (design §1.4, §2.8).
//!
//! Unlike `rcpps`/`rsqrtps`, these are **architecturally defined**: the Arm Architecture Reference
//! Manual for A-profile (Arm ARM) specifies the result bit for bit with the shared pseudocode
//! functions `FPRecipEstimate`/`RecipEstimate` and `FPRSqrtEstimate`/`RecipSqrtEstimate`. This
//! module is a table-free port of that pseudocode for single precision, so it is exact by
//! construction on every Arm core (Apple, Neoverse, Cortex), and runs on any host and under
//! Miri. On `aarch64` hosts the tests also compare it with the hardware.
//!
//! The model assumes what the `Neon` tier runs under: `FPCR` at its Linux/macOS default
//! (`DN = 0`: NaN operands propagate quieted with their payload; `FZ = 0`: denormals are not
//! flushed; `AH = 0`, so no `FEAT_RPRES` increased precision; round to nearest even).

/// `RecipEstimate(a)` (Arm ARM shared pseudocode, `!increasedprecision` path): for a fixed-point
/// input `a / 512` in `[0.5, 1)` (`256 <= a < 512`), the estimate `r / 256` of its reciprocal,
/// in `[1, 2)` (`256 <= r < 512`), with 8 correct fraction bits.
///
/// # Panics
/// If `a` is outside `256..512`.
#[must_use]
pub fn recip_estimate(a: u32) -> u32 {
    assert!(
        (256..512).contains(&a),
        "RecipEstimate input {a} out of range"
    );
    let a = a * 2 + 1; // Round to nearest
    let b = (1 << 19) / a;
    let r = b.div_ceil(2); // (b+1) DIV 2: round to nearest
    debug_assert!((256..512).contains(&r));
    r
}

/// `RecipSqrtEstimate(a)` (Arm ARM shared pseudocode, `!increasedprecision` path): for a
/// fixed-point input `a / 512` in `[0.25, 1)` (`128 <= a < 512`), the estimate `r / 256` of its
/// reciprocal square root, in `[1, 2)` (`256 <= r < 512`).
///
/// # Panics
/// If `a` is outside `128..512`.
#[must_use]
pub fn recip_sqrt_estimate(a: u32) -> u32 {
    assert!(
        (128..512).contains(&a),
        "RecipSqrtEstimate input {a} out of range"
    );
    let a = if a < 256 {
        // 0.25 .. 0.5
        a * 2 + 1 // a in units of 1/512 rounded to nearest
    } else {
        // 0.5 .. 1.0
        let a = (a >> 1) << 1; // Discard bottom bit
        (a + 1) * 2 // a in units of 1/256 rounded to nearest
    };
    let mut b: u64 = 512;
    while u64::from(a) * (b + 1) * (b + 1) < 1 << 28 {
        b += 1;
    }
    // b = largest b such that b < 2^14 / sqrt(a)
    #[allow(clippy::cast_possible_truncation)] // b < 2^10 (asserted range below)
    let r = (b as u32).div_ceil(2); // (b+1) DIV 2: round to nearest
    debug_assert!((256..512).contains(&r));
    r
}

/// The quiet NaN `FPProcessNaN` returns for a NaN operand with `FPCR.DN = 0`: the operand with
/// its quiet bit set (sign and payload kept).
#[inline]
fn process_nan(bits: u32) -> f32 {
    f32::from_bits(bits | 0x0040_0000)
}

/// `FPDefaultNaN()`: `0x7FC00000`.
const DEFAULT_NAN: u32 = 0x7fc0_0000;

/// `FPRecipEstimate` for single precision (`FRECPE`, `vrecpeq_f32`), with the default `FPCR`.
///
/// NaN → the NaN quieted; `±inf` → `±0`; `±0` → `±inf`; `|x| < 2⁻¹²⁸` (small denormals) →
/// `±inf` (overflow, round to nearest); otherwise an 8-bit estimate of `1/x`, denormal for
/// `|x| >= 2¹²⁶`.
#[must_use]
pub fn frecpe(x: f32) -> f32 {
    let bits = x.to_bits();
    let sign = bits & 0x8000_0000;
    let exp_field = (bits >> 23) & 0xff;
    let frac_field = bits & 0x007f_ffff;
    if x.is_nan() {
        return process_nan(bits);
    }
    if x.is_infinite() {
        return f32::from_bits(sign); // FPZero(sign)
    }
    if exp_field == 0 && frac_field == 0 {
        return f32::from_bits(sign | 0x7f80_0000); // FPInfinity(sign)
    }
    if exp_field == 0 && frac_field < 0x0020_0000 {
        // Abs(value) < 2.0^-128: overflow_to_inf under FPRounding_TIEEVEN.
        return f32::from_bits(sign | 0x7f80_0000);
    }
    // Scale to a fixed point value in the range 0.5 <= x < 1.0 in steps of 1/512, and calculate
    // the result exponent. `fraction` is the pseudocode's bits(52) fraction.
    let mut fraction: u64 = u64::from(frac_field) << 29;
    #[allow(clippy::cast_possible_wrap)] // an 8-bit field
    let mut exp = exp_field as i32;
    if exp == 0 {
        if fraction >> 51 & 1 == 0 {
            exp = -1;
            fraction = (fraction << 2) & ((1 << 52) - 1); // fraction<49:0>:'00'
        } else {
            fraction = (fraction << 1) & ((1 << 52) - 1); // fraction<50:0>:'0'
        }
    }
    #[allow(clippy::cast_possible_truncation)] // 9 bits
    let scaled = (0x100 | (fraction >> 44)) as u32; // UInt('1':fraction<51:44>)
    let mut result_exp = 253 - exp; // In range 253-254 = -1 to 253+1 = 254
    let estimate = recip_estimate(scaled);
    // Estimate is in the range 256 .. 511 representing a fixed-point result in the range
    // [1.0 .. 2.0]. Convert to a scaled floating point result with copied sign bit, high-order
    // bits from estimate, and exponent calculated above.
    let mut fraction: u64 = u64::from(estimate & 0xff) << 44;
    if result_exp == 0 {
        fraction = (1 << 51) | (fraction >> 1); // '1':fraction<51:1>
    } else if result_exp == -1 {
        fraction = (1 << 50) | (fraction >> 2); // '01':fraction<51:2>
        result_exp = 0;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // 0 <= result_exp <= 254
    let result = sign | ((result_exp as u32) << 23) | (fraction >> 29) as u32;
    f32::from_bits(result)
}

/// `FPRSqrtEstimate` for single precision (`FRSQRTE`, `vrsqrteq_f32`), with the default `FPCR`.
///
/// NaN → the NaN quieted; `±0` → `±inf`; any other negative input (including `-inf` and negative
/// denormals) → the default NaN `0x7FC00000`; `+inf` → `+0`; otherwise an 8-bit estimate of
/// `1/sqrt(x)` (denormal inputs are normalized).
#[must_use]
pub fn frsqrte(x: f32) -> f32 {
    let bits = x.to_bits();
    let exp_field = (bits >> 23) & 0xff;
    let frac_field = bits & 0x007f_ffff;
    if x.is_nan() {
        return process_nan(bits);
    }
    if exp_field == 0 && frac_field == 0 {
        return f32::from_bits((bits & 0x8000_0000) | 0x7f80_0000); // FPInfinity(sign)
    }
    if bits & 0x8000_0000 != 0 {
        return f32::from_bits(DEFAULT_NAN);
    }
    if x.is_infinite() {
        return 0.0; // FPZero('0')
    }
    // Scale to a fixed-point value in the range 0.25 <= x < 1.0 in steps of 512, with the
    // evenness or oddness of the exponent unchanged, and calculate result exponent.
    let mut fraction: u64 = u64::from(frac_field) << 29;
    #[allow(clippy::cast_possible_wrap)] // an 8-bit field
    let mut exp = exp_field as i32;
    if exp == 0 {
        while fraction >> 51 & 1 == 0 {
            fraction = (fraction << 1) & ((1 << 52) - 1);
            exp -= 1;
        }
        fraction = (fraction << 1) & ((1 << 52) - 1);
    }
    #[allow(clippy::cast_possible_truncation)] // at most 9 bits
    let scaled = if exp & 1 == 0 {
        (0x100 | (fraction >> 44)) as u32 // UInt('1':fraction<51:44>)
    } else {
        (0x80 | (fraction >> 45)) as u32 // UInt('01':fraction<51:45>)
    };
    // (380 - exp) DIV 2 (exp <= 254, so the dividend is positive).
    let result_exp = (380 - exp) / 2;
    let estimate = recip_sqrt_estimate(scaled);
    // Estimate is in the range 256 .. 511 representing a fixed-point result in the range
    // [1.0 .. 2.0]: '0' : result_exp<7:0> : estimate<7:0> : Zeros(15).
    #[allow(clippy::cast_sign_loss)] // 63 <= result_exp <= 253 (exp >= -22 for denormals)
    let result = ((result_exp as u32 & 0xff) << 23) | ((estimate & 0xff) << 15);
    f32::from_bits(result)
}

/// `FRECPE` executed by this host (one lane of `vrecpeq_f32`), or `None` on hosts without NEON
/// (and under Miri, whose emulation of the instruction is not any CPU's).
#[must_use]
pub fn host_frecpe(x: f32) -> Option<f32> {
    host::frecpe(x)
}

/// `FRSQRTE` executed by this host (one lane of `vrsqrteq_f32`), or `None` on hosts without NEON
/// (and under Miri).
#[must_use]
pub fn host_frsqrte(x: f32) -> Option<f32> {
    host::frsqrte(x)
}

/// Whether [`host_frecpe`]/[`host_frsqrte`] run on this host.
#[must_use]
pub fn host_available() -> bool {
    host::frecpe(1.0).is_some()
}

#[cfg(all(target_arch = "aarch64", not(miri)))]
mod host {
    use core::arch::aarch64::{vdupq_n_f32, vgetq_lane_f32, vrecpeq_f32, vrsqrteq_f32};

    use crate::cpu::NeonToken;

    #[target_feature(enable = "neon")]
    fn frecpe_neon(x: f32) -> f32 {
        vgetq_lane_f32::<0>(vrecpeq_f32(vdupq_n_f32(x)))
    }

    #[target_feature(enable = "neon")]
    fn frsqrte_neon(x: f32) -> f32 {
        vgetq_lane_f32::<0>(vrsqrteq_f32(vdupq_n_f32(x)))
    }

    pub(super) fn frecpe(x: f32) -> Option<f32> {
        let _tok = NeonToken::get()?;
        // SAFETY: `frecpe_neon` enables only `neon` (`NeonToken::FEATURES`); `_tok` exists only
        // because NEON was detected at run time.
        Some(unsafe { frecpe_neon(x) })
    }

    pub(super) fn frsqrte(x: f32) -> Option<f32> {
        let _tok = NeonToken::get()?;
        // SAFETY: `frsqrte_neon` enables only `neon` (`NeonToken::FEATURES`); `_tok` exists only
        // because NEON was detected at run time.
        Some(unsafe { frsqrte_neon(x) })
    }
}

/// Hosts without NEON (and Miri) cannot execute `FRECPE`/`FRSQRTE`.
#[cfg(not(all(target_arch = "aarch64", not(miri))))]
mod host {
    pub(super) fn frecpe(_: f32) -> Option<f32> {
        None
    }

    pub(super) fn frsqrte(_: f32) -> Option<f32> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rcp(x: u32) -> u32 {
        frecpe(f32::from_bits(x)).to_bits()
    }

    fn rsq(x: u32) -> u32 {
        frsqrte(f32::from_bits(x)).to_bits()
    }

    /// The integer cores at their range ends and a few hand-computed points
    /// (`RecipEstimate(256)`: `a = 513`, `b = 2^19 DIV 513 = 1021`, `r = 1022 DIV 2 = 511`).
    #[test]
    fn integer_estimates() {
        assert_eq!(recip_estimate(256), 511);
        assert_eq!(recip_estimate(511), 256);
        assert_eq!(recip_estimate(384), 341); // 1/0.75 = 1.333 -> 341/256 = 1.332
        // RecipSqrtEstimate(128): a = 257, largest b with 257*(b+1)^2 >= 2^28 first at b = 1022.
        assert_eq!(recip_sqrt_estimate(128), 511);
        // RecipSqrtEstimate(256): a = 514, b = 722, r = 361 (1/sqrt(0.5) = 1.414 -> 1.410).
        assert_eq!(recip_sqrt_estimate(256), 361);
        assert_eq!(recip_sqrt_estimate(257), 361); // the bottom bit is discarded above 0.5
        assert_eq!(recip_sqrt_estimate(511), 256);
        // Every output is in range and the functions are non-increasing.
        for a in 257..512 {
            assert!(recip_estimate(a) <= recip_estimate(a - 1));
        }
        for a in 129..512 {
            assert!(recip_sqrt_estimate(a) <= recip_sqrt_estimate(a - 1));
        }
    }

    /// Known answers from the Arm ARM's definitions: `FRECPE(1.0) = 0.998046875` (`0x3F7F8000`,
    /// the classic `vrecpe` value), sign and exponent scaling, the special cases of
    /// `FPRecipEstimate`, and both denormal-result exponents.
    #[test]
    fn frecpe_known_answers() {
        assert_eq!(rcp(0x3f80_0000), 0x3f7f_8000); // 1.0
        assert_eq!(rcp(0xbf80_0000), 0xbf7f_8000); // -1.0
        assert_eq!(rcp(0x4000_0000), 0x3eff_8000); // 2.0
        assert_eq!(rcp(0x3f00_0000), 0x3fff_8000); // 0.5
        assert_eq!(rcp(0x4040_0000), 0x3eaa_8000); // 3.0: scaled 384 -> 341 = 0x155
        assert_eq!(rcp(0x3fff_ffff), 0x3f00_0000); // 2 - ulp: scaled 511 -> 256
        assert_eq!(rcp(0), 0x7f80_0000); // +0 -> +inf
        assert_eq!(rcp(0x8000_0000), 0xff80_0000); // -0 -> -inf
        assert_eq!(rcp(0x7f80_0000), 0); // +inf -> +0
        assert_eq!(rcp(0xff80_0000), 0x8000_0000); // -inf -> -0
        assert_eq!(rcp(0x7f80_0001), 0x7fc0_0001); // sNaN quieted, payload kept
        assert_eq!(rcp(0xffc0_1234), 0xffc0_1234); // qNaN unchanged
        assert_eq!(rcp(0x0000_0001), 0x7f80_0000); // |x| < 2^-128: overflow to inf
        assert_eq!(rcp(0x801f_ffff), 0xff80_0000);
        // 2^-128 (a denormal with fraction<51> = 0): exp = -1, result_exp = 254.
        assert_eq!(rcp(0x0020_0000), 0x7f7f_8000);
        // Largest denormal (fraction<51> = 1): exp stays 0, result_exp = 253.
        assert_eq!(rcp(0x007f_ffff), 0x7e80_0000);
        // 2^126: result_exp = 0, the result is the denormal '1':estimate.
        assert_eq!(rcp(0x7e80_0000), 0x007f_c000);
        // FLT_MAX: result_exp = -1, the result is the denormal '01':estimate (2^-128).
        assert_eq!(rcp(0x7f7f_ffff), 0x0020_0000);
        assert_eq!(rcp(0xff7f_ffff), 0x8020_0000);
    }

    /// Known answers for `FPRSqrtEstimate`: `FRSQRTE(1.0) = 0.998046875`, the exponent-parity
    /// split between `[1,2)` and `[2,4)`, the special cases and a normalized denormal.
    #[test]
    fn frsqrte_known_answers() {
        assert_eq!(rsq(0x3f80_0000), 0x3f7f_8000); // 1.0
        assert_eq!(rsq(0x4080_0000), 0x3eff_8000); // 4.0
        assert_eq!(rsq(0x4000_0000), 0x3f34_8000); // 2.0: 361 = 0x169 -> 0.705078125
        assert_eq!(rsq(0x3e80_0000), 0x3fff_8000); // 0.25
        assert_eq!(rsq(0), 0x7f80_0000); // +0 -> +inf
        assert_eq!(rsq(0x8000_0000), 0xff80_0000); // -0 -> -inf
        assert_eq!(rsq(0x7f80_0000), 0); // +inf -> +0
        assert_eq!(rsq(0xff80_0000), DEFAULT_NAN); // -inf -> default NaN
        assert_eq!(rsq(0xbf80_0000), DEFAULT_NAN); // -1.0 -> default NaN
        assert_eq!(rsq(0x8000_0001), DEFAULT_NAN); // negative denormal -> default NaN
        assert_eq!(rsq(0x7f80_0001), 0x7fc0_0001); // sNaN quieted
        assert_eq!(rsq(0xff80_0001), 0xffc0_0001); // negative sNaN quieted, sign kept
        // The smallest denormal 2^-149: normalized to exp = -22, rsqrt = 2^74.5.
        assert_eq!(rsq(0x0000_0001), 0x64b4_8000);
        // FLT_MAX: exp 254 (even): scaled 511 -> 256, result_exp 63.
        assert_eq!(rsq(0x7f7f_ffff), 0x1f80_0000);
    }

    /// Both estimates have relative error below 2^-8 (8-bit estimates) in every binade,
    /// including denormal inputs; for normal inputs they depend only on the exponent and the top
    /// 8 fraction bits.
    #[test]
    fn estimates_are_8_bit() {
        let step = if cfg!(miri) { 0x0040_0007 } else { 997 };
        for bits in (0x0020_0000u32..0x7f80_0000).step_by(step) {
            let x = f32::from_bits(bits);
            let r = f64::from(frecpe(x));
            let exact = 1.0 / f64::from(x);
            assert!(
                ((r - exact) / exact).abs() < 1.0 / 256.0,
                "frecpe({bits:#x})"
            );
            if bits >= 0x0080_0000 {
                // Normal inputs: only the top 8 fraction bits (and the exponent) matter.
                assert_eq!(rcp(bits), rcp(bits & !0x7fff), "frecpe({bits:#x}) low bits");
                assert_eq!(
                    rsq(bits),
                    rsq(bits & !0x7fff),
                    "frsqrte({bits:#x}) low bits"
                );
            }
            let s = f64::from(frsqrte(x));
            let exact = 1.0 / f64::from(x).sqrt();
            assert!(
                ((s - exact) / exact).abs() < 1.0 / 256.0,
                "frsqrte({bits:#x})"
            );
        }
    }

    /// The model against this host's `FRECPE`/`FRSQRTE` on every 4093rd bit pattern (all signs
    /// and exponents) plus specials. Runs on `aarch64` hosts; skipped elsewhere.
    #[test]
    fn sampled_arm_vs_host() {
        if !host_available() {
            eprintln!("skipping sampled_arm_vs_host: no NEON on this host");
            return;
        }
        let mut inputs: Vec<u32> = (0..=u32::MAX).step_by(4093).collect();
        inputs.extend([
            0,
            1,
            0x001f_ffff,
            0x0020_0000,
            0x007f_ffff,
            0x0080_0000,
            0x3f80_0000,
            0x7e80_0000,
            0x7f7f_ffff,
            0x7f80_0000,
            0x7f80_0001,
            0x7fc0_0000,
        ]);
        inputs.extend(inputs.clone().iter().map(|b| b | 0x8000_0000));
        let mut bad = Vec::new();
        for &b in &inputs {
            let x = f32::from_bits(b);
            let (hr, hs) = (host_frecpe(x).unwrap(), host_frsqrte(x).unwrap());
            if hr.to_bits() != rcp(b) || hs.to_bits() != rsq(b) {
                bad.push((b, hr.to_bits(), rcp(b), hs.to_bits(), rsq(b)));
            }
        }
        assert!(
            bad.is_empty(),
            "{} mismatches (input, host frecpe, model, host frsqrte, model): {:08x?}",
            bad.len(),
            &bad[..bad.len().min(16)]
        );
    }

    /// Every `f32` input through the host and the model. Needs an `aarch64` host.
    #[test]
    #[ignore = "exhaustive over 2^32 inputs per op; needs an aarch64 host. Run: \
                cargo test -p skia-rust-simd --release -- --ignored exhaustive_arm_vs_host"]
    fn exhaustive_arm_vs_host() {
        assert!(host_available(), "needs an aarch64 host with NEON");
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get() as u64);
        let per = (1u64 << 32).div_ceil(threads);
        let bad: u64 = std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    s.spawn(move || {
                        let mut bad = 0u64;
                        for b in t * per..((t + 1) * per).min(1 << 32) {
                            #[allow(clippy::cast_possible_truncation)] // < 2^32
                            let b = b as u32;
                            let x = f32::from_bits(b);
                            let ok_r = host_frecpe(x).unwrap().to_bits() == rcp(b);
                            let ok_s = host_frsqrte(x).unwrap().to_bits() == rsq(b);
                            if !(ok_r && ok_s) {
                                if bad < 16 {
                                    eprintln!("mismatch at {b:#010x}");
                                }
                                bad += 1;
                            }
                        }
                        bad
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).sum()
        });
        assert_eq!(bad, 0, "model differs from the host");
    }
}
