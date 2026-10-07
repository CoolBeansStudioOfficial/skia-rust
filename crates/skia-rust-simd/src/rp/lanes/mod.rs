// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/opts/SkRasterPipeline_opts.h

//! Lane types and per-tier primitives of the raster pipeline (design §1.3–§1.6, §2.4, §2.8).
//!
//! Skia compiles `SkRasterPipeline_opts.h` once per CPU path; each path defines the lane types
//! (`F`, `I32`, …) and a short list of primitives whose results differ between paths (`min`,
//! `mad`, `floor_`, `rcp_*`, `round`, `pack`, …). Everything else in a stage is lane-wise
//! IEEE/integer arithmetic that is identical on every tier. This module holds one Rust module per
//! tier with **the same names**, so stage code is written once and stamped into each tier
//! (design §2.5): the stage source names `F`, `mad`, `rcp_fast`, … unqualified and each tier
//! module brings its own into scope.
//!
//! | Module | Tier | Lane types | Functions are |
//! |---|---|---|---|
//! | [`scalar`] | `Scalar` (`SKRP_CPU_SCALAR`) | [`S<T>`], 1 lane, no lowp | plain `#[inline]` (it is its own model) |
//! | `sse2` (x86-64) | `Sse2` (`SKRP_CPU_SSE2`) | `Vec<4, _>`, lowp `Vec<8, _>` | `#[target_feature(enable = "sse2")]` |
//! | `sse41` (x86-64) | `Sse41` (`SKRP_CPU_SSE41`/`AVX`) | same | `#[target_feature(enable = "sse2,ssse3,sse4.1")]` |
//! | `ml3` (x86-64) | `Ml3` (`SKRP_CPU_AVX2`) | `Vec<8, _>`, lowp `Vec<16, _>` | `#[target_feature(enable = "sse2,ssse3,sse4.1,sse4.2,avx,avx2,bmi1,bmi2,f16c,fma")]` |
//! | `ml4` (x86-64) | `Ml4` (`SKRP_CPU_ML4`) | `Vec<16, _>`, lowp `Vec<16, _>` | `Ml3`'s + `"avx512f,avx512dq,avx512cd,avx512bw,avx512vl"` |
//! | `neon` (aarch64) | `Neon` (`SKRP_CPU_NEON`, arm64) | `Vec<4, _>`, lowp `Vec<8, _>` | `#[target_feature(enable = "neon")]` |
//! | `model_{sse2,sse41,ml3,ml4}::{host, amd_zen4}` (feature `models`, tests) | the models (§2.8) | same as the tier | plain; per-lane x86 semantics, `rcpps`/`rsqrtps` (`rcp14`/`rsqrt14` for Ml4) from [`Estimates::Host`](crate::Estimates::Host) or [`Estimates::AmdZen4`](crate::Estimates::AmdZen4) |
//! | `model_neon::{host, arm}` (feature `models`, tests) | the `Neon` model | same as the tier | plain; per-lane A64 semantics ([`neon_model`]), `FRECPE`/`FRSQRTE` from [`Estimates::Host`](crate::Estimates::Host) or [`Estimates::Arm`](crate::Estimates::Arm) |
//!
//! # What every tier module exports (the stage author's API)
//!
//! ```text
//! N                                   highp stride (1 on Scalar, 4 on Sse2/Sse41/Neon, 8 on Ml3,
//!                                     16 on Ml4)
//! LOWP_N                              lowp stride (8 on Sse*/Neon, 16 on Ml3/Ml4; not on Scalar:
//!                                     no lowp)
//! F I32 U32 U16 U8 U64                highp lane types (Vec<N, _>, or S<_> on Scalar)
//!
//! min_f(a, b)  max_f(a, b)            float min/max with the tier's NaN and ±0 rules (keep Skia's
//!                                     operand order: x86 returns the *second* operand on NaN/tie;
//!                                     Neon's FMIN/FMAX propagate NaNs, -0 < +0)
//! mad(f, m, a) = a + f*m              nmad(f, m, a) = a - f*m   (unfused on Scalar/Sse*, a single
//!                                     FMA on Ml3/Ml4/Neon)
//! abs_f(v)                            x86: v & (0 - v) (NaN keeps its sign); Scalar: fabsf;
//!                                     Neon: FABS
//! floor_(v)  ceil_(v)                 Sse2: cvtt emulation; Sse41+: roundps; Scalar: floorf/ceilf;
//!                                     Neon: FRINTM/FRINTP
//! sqrt_(v)                            exact
//! rcp_approx  rsqrt_approx            don't call directly (as in Skia); use rcp_fast / rsqrt
//!                                     (Neon: FRECPE/FRSQRTE + one fused Newton–Raphson step)
//! rcp_precise(v)                      x86: one Newton–Raphson step on rcpps (Ml3/Ml4: fused, Ml4:
//!                                     on rcp14); Scalar: 1/v; Neon: two FRECPS steps on FRECPE
//! rcp_fast(v)  rsqrt(v)               Sse2/Scalar: precise forms; Sse41+/Neon: the approx forms
//! iround(v) -> I32  round(v) -> U32   x86: cvtps2dq (ties-to-even, NaN/overflow → 0x80000000);
//!                                     Scalar: (int)(v + 0.5f); Neon: FCVTNS/FCVTNU (ties to
//!                                     even, saturating, NaN → 0; `round` is *unsigned*)
//! trunc_(v) -> U32  to_i32(v) -> I32  every F → int vector cast (x86: cvttps2dq; Neon: FCVTZS)
//! cast_f(U32) -> F                    Skia's `cast(U32)`: SIMD converts as *signed* I32
//! pack_u32(U32) -> U16                Scalar/Sse2/Neon: truncation; Sse41+: unsigned saturation
//! pack_u16(U16) -> U8                 Scalar/Neon: truncation; x86: packuswb (signed-saturating)
//! if_then_else_f(c, t, e)  if_then_else_i(c, t, e)   (c: I32; Sse*/Neon bitwise, Ml3/Ml4 the
//!                                     sign bit only, Scalar c != 0)
//! any(c)  all(c)                      Sse*: lane sign bits only; Ml3: any/every bit of the whole
//!                                     register; Ml4/Neon: some/every lane nonzero; Scalar: c != 0
//! cond_to_mask(c)                     identity except Scalar (0/1 → 0/-1)
//! from_half(U16) -> F  to_half(F) -> U16   Scalar/Sse*: software (flush denormals, truncate);
//!                                     Ml3/Ml4: F16C, Neon: FCVT (IEEE, round to nearest even)
//! div_i32(d, s)  div_u32(d, s)        SkSL integer `/` (x86: via f64, unsigned operands clamped to
//!                                     INT_MAX except on Ml4, which is exact; Scalar/Neon: guarded
//!                                     generic div_fn)
//! min_i max_i min_u max_u abs_i       integer min/max/abs (identical on every tier)
//!
//! lowp::{N, U8, U16, I16, I32, U32, I64, U64, F}   lowp lane types (LOWP_N lanes)
//! lowp::div255  lowp::div255_accurate (x86: (v+255)/256 and the exact two-step form;
//!                                     Neon: both are the exact URSRA/URSHR form)
//! lowp::{min_f, max_f, min_i, max_i, min_u16, max_u16}   compare-select (portable)
//! lowp::{min_intr_f, max_intr_f}      x86: minps/maxps per register (differs from min_f on
//!                                     NaN/±0); Neon: compare-select (= min_f/max_f)
//! lowp::{min_intr_i, max_intr_i, min_intr_u16, max_intr_u16}
//! lowp::{if_then_else_f, if_then_else_i, if_then_else_u16, if_then_else_u32}  bitwise
//! lowp::{mad, nmad, trunc_, to_i32, rcp_precise, sqrt_, floor_, scaled_mult}   (lowp mad/nmad are
//!                                     unfused on every tier, Ml3/Ml4/Neon included; Neon's
//!                                     scaled_mult saturates -32768 * -32768 to 32767)
//! ```
//!
//! Comparisons are the lane types' own methods (`eq_mask`, `lt_mask`, …): all-ones masks on
//! `Vec` (unsigned lanes give unsigned masks, use `bit_cast` to get `I32`), **0/1 on [`S`]**.
//! Skia's `sk_bit_cast` is `bit_cast`, `cast<D>` / `expand` between integer types and int → float
//! is `cast::<D>()` (exact on every tier); never use `cast` for float → int (use `to_i32` or
//! `trunc_`). Scalar overloads (`min(F, float)`) become `min_f(a, F::splat(b))`.
//!
//! # Safety
//! The native modules use `unsafe` only for the array ↔ register conversions in `x86` and
//! `aarch64` (design §3.2). Calling a native primitive from code without the tier's target
//! features needs `unsafe` and a token (`crate::cpu::Sse2Token`, `Sse41Token`, `Ml3Token`,
//! `Ml4Token`, `NeonToken`); stage code stamped into the tier's `#[target_feature]` functions
//! calls them safely.

/// Stamps Skia's software `from_half`/`to_half` (the path taken by every tier without F16C or
/// NEON half conversions: Scalar, Sse2, Sse41) into the invoking tier module, using that module's
/// `F`, `U16`, `U32`, `I32`, `if_then_else_f`, `if_then_else_i`, `pack_u32` and `si!`.
///
/// These are stage-level functions in Skia (defined once after the per-CPU blocks), so stamping
/// them is exactly what the C++ does; `to_half` inherits the tier's `pack` (truncating on
/// Scalar/Sse2, saturating on Sse41).
macro_rules! soft_half {
    () => {
        si! {
            // Port of: src/opts/SkRasterPipeline_opts.h#L1651-L1672 (chrome/m156) (software path)
            /// `from_half`: half bits to float, flushing denormal halfs (and zero) to `+0.0`.
            /// Infinities and NaNs are *not* special-cased (half `inf` becomes `65536.0`).
            pub fn from_half(h: U16) -> F {
                // Remember, a half is 1-5-10 (sign-exponent-mantissa) with 15 exponent bias.
                let sem: U32 = h.cast(); // expand(h)
                let s = sem & 0x8000;
                let em = sem ^ s;

                // Convert to 1-8-23 float with 127 bias, flushing denorm halfs (including zero)
                // to zero.
                let em_i: I32 = em.bit_cast();
                let denorm = em_i.lt_mask(0x0400); // I32 comparison is often quicker, and always safe here.
                let norm: F = ((s << 16) + (em << 13) + ((127 - 15) << 23)).bit_cast();
                if_then_else_f(denorm, F::splat(0.0), norm)
            }

            // Port of: src/opts/SkRasterPipeline_opts.h#L1674-L1695 (chrome/m156) (software path)
            /// `to_half`: float to half bits, flushing values that would be half denormals
            /// (including zero, of either sign) to `0` and **truncating** the mantissa. The
            /// final `pack` is the tier's.
            pub fn to_half(f: F) -> U16 {
                // Remember, a float is 1-8-23 (sign-exponent-mantissa) with 127 exponent bias.
                let sem: U32 = f.bit_cast();
                let s = sem & 0x8000_0000;
                let em = sem ^ s;

                // Convert to 1-5-10 half with 15 bias, flushing denorm halfs (including zero) to
                // zero.
                let em_i: I32 = em.bit_cast();
                let denorm = em_i.lt_mask(0x3880_0000); // I32 comparison is often quicker, and always safe here.
                let h: I32 = ((s >> 16) + (em >> 13) - ((127 - 15) << 10)).bit_cast();
                pack_u32(if_then_else_i(denorm, I32::splat(0), h).bit_cast())
            }
        }
    };
}

/// Test-only: stamps `harness_highp` (one call per primitive, on raw lane bits) into the
/// invoking tier module, so native tiers and their models are driven by identical code.
#[cfg(test)]
macro_rules! lane_harness {
    () => {
        si! {
            /// Applies `op` to consecutive `N`-lane chunks of `a`, `b`, `c` (raw 32-bit lane
            /// patterns: floats by bits, `U16` inputs from the low 16 bits) and returns the
            /// result lanes as bits (`U16`/`U8` results zero-extended; `any`/`all` as 0/1 in
            /// every lane of the chunk).
            pub(crate) fn harness_highp(
                op: $crate::rp::lanes::test_support::Prim,
                a: &[u32],
                b: &[u32],
                c: &[u32],
            ) -> ::std::vec::Vec<u32> {
                use $crate::rp::lanes::test_support::Prim;
                let mut out = ::std::vec![0u32; a.len()];
                for (i, o) in out.chunks_exact_mut(N).enumerate() {
                    let r = i * N..(i + 1) * N;
                    let (ua, ub, uc) = (
                        U32::load(&a[r.clone()]),
                        U32::load(&b[r.clone()]),
                        U32::load(&c[r]),
                    );
                    let (fa, fb, fc): (F, F, F) = (ua.bit_cast(), ub.bit_cast(), uc.bit_cast());
                    let (ia, ib, ic): (I32, I32, I32) =
                        (ua.bit_cast(), ub.bit_cast(), uc.bit_cast());
                    let ha: U16 = ua.cast();
                    let res: U32 = match op {
                        Prim::MinF => min_f(fa, fb).bit_cast(),
                        Prim::MaxF => max_f(fa, fb).bit_cast(),
                        Prim::Mad => mad(fa, fb, fc).bit_cast(),
                        Prim::Nmad => nmad(fa, fb, fc).bit_cast(),
                        Prim::AbsF => abs_f(fa).bit_cast(),
                        Prim::Floor => floor_(fa).bit_cast(),
                        Prim::Ceil => ceil_(fa).bit_cast(),
                        Prim::Sqrt => sqrt_(fa).bit_cast(),
                        Prim::RcpApprox => rcp_approx(fa).bit_cast(),
                        Prim::RsqrtApprox => rsqrt_approx(fa).bit_cast(),
                        Prim::RcpPrecise => rcp_precise(fa).bit_cast(),
                        Prim::RcpFast => rcp_fast(fa).bit_cast(),
                        Prim::Rsqrt => rsqrt(fa).bit_cast(),
                        Prim::Iround => iround(fa).bit_cast(),
                        Prim::Round => round(fa),
                        Prim::Trunc => trunc_(fa),
                        Prim::ToI32 => to_i32(fa).bit_cast(),
                        Prim::CastF => cast_f(ua).bit_cast(),
                        Prim::PackU32 => pack_u32(ua).cast(),
                        Prim::PackU16 => pack_u16(ha).cast(),
                        Prim::IfThenElseF => if_then_else_f(ia, fb, fc).bit_cast(),
                        Prim::IfThenElseI => if_then_else_i(ia, ib, ic).bit_cast(),
                        Prim::Any => U32::splat(u32::from(any(ia))),
                        Prim::All => U32::splat(u32::from(all(ia))),
                        Prim::CondToMask => cond_to_mask(ia).bit_cast(),
                        Prim::FromHalf => from_half(ha).bit_cast(),
                        Prim::ToHalf => to_half(fa).cast(),
                        Prim::DivI32 => div_i32(ia, ib).bit_cast(),
                        Prim::DivU32 => div_u32(ua, ub),
                        Prim::MinI => min_i(ia, ib).bit_cast(),
                        Prim::MaxI => max_i(ia, ib).bit_cast(),
                        Prim::MinU => min_u(ua, ub),
                        Prim::MaxU => max_u(ua, ub),
                        Prim::AbsI => abs_i(ia).bit_cast(),
                    };
                    res.store(o);
                }
                out
            }
        }
    };
}

/// Test-only: stamps `harness_lowp` into the invoking tier's `lowp` module.
#[cfg(test)]
macro_rules! lowp_harness {
    () => {
        si! {
            /// Like `harness_highp`, over `lowp::N`-lane chunks.
            pub(crate) fn harness_lowp(
                op: $crate::rp::lanes::test_support::LowpPrim,
                a: &[u32],
                b: &[u32],
                c: &[u32],
            ) -> ::std::vec::Vec<u32> {
                use $crate::rp::lanes::test_support::LowpPrim;
                let mut out = ::std::vec![0u32; a.len()];
                for (i, o) in out.chunks_exact_mut(N).enumerate() {
                    let r = i * N..(i + 1) * N;
                    let (ua, ub, uc) = (
                        U32::load(&a[r.clone()]),
                        U32::load(&b[r.clone()]),
                        U32::load(&c[r]),
                    );
                    let (fa, fb, fc): (F, F, F) = (ua.bit_cast(), ub.bit_cast(), uc.bit_cast());
                    let (ia, ib, ic): (I32, I32, I32) =
                        (ua.bit_cast(), ub.bit_cast(), uc.bit_cast());
                    let (ha, hb, hc): (U16, U16, U16) = (ua.cast(), ub.cast(), uc.cast());
                    let (sa, sb): (I16, I16) = (ua.cast(), ub.cast());
                    let res: U32 = match op {
                        LowpPrim::Div255 => div255(ha).cast(),
                        LowpPrim::Div255Accurate => div255_accurate(ha).cast(),
                        LowpPrim::MinF => min_f(fa, fb).bit_cast(),
                        LowpPrim::MaxF => max_f(fa, fb).bit_cast(),
                        LowpPrim::MinI => min_i(ia, ib).bit_cast(),
                        LowpPrim::MaxI => max_i(ia, ib).bit_cast(),
                        LowpPrim::MinU16 => min_u16(ha, hb).cast(),
                        LowpPrim::MaxU16 => max_u16(ha, hb).cast(),
                        LowpPrim::MinIntrF => min_intr_f(fa, fb).bit_cast(),
                        LowpPrim::MaxIntrF => max_intr_f(fa, fb).bit_cast(),
                        LowpPrim::MinIntrI => min_intr_i(ia, ib).bit_cast(),
                        LowpPrim::MaxIntrI => max_intr_i(ia, ib).bit_cast(),
                        LowpPrim::MinIntrU16 => min_intr_u16(ha, hb).cast(),
                        LowpPrim::MaxIntrU16 => max_intr_u16(ha, hb).cast(),
                        LowpPrim::IfThenElseF => if_then_else_f(ia, fb, fc).bit_cast(),
                        LowpPrim::IfThenElseI => if_then_else_i(ia, ib, ic).bit_cast(),
                        LowpPrim::IfThenElseU16 => {
                            if_then_else_u16(ha.bit_cast(), hb, hc).cast()
                        }
                        LowpPrim::IfThenElseU32 => if_then_else_u32(ia, ub, uc),
                        LowpPrim::Mad => mad(fa, fb, fc).bit_cast(),
                        LowpPrim::Nmad => nmad(fa, fb, fc).bit_cast(),
                        LowpPrim::Trunc => trunc_(fa),
                        LowpPrim::ToI32 => to_i32(fa).bit_cast(),
                        LowpPrim::RcpPrecise => rcp_precise(fa).bit_cast(),
                        LowpPrim::Sqrt => sqrt_(fa).bit_cast(),
                        LowpPrim::Floor => floor_(fa).bit_cast(),
                        LowpPrim::ScaledMult => {
                            let p: U16 = scaled_mult(sa, sb).bit_cast();
                            p.cast()
                        }
                    };
                    res.store(o);
                }
                out
            }
        }
    };
}

// The modules come after the macros: `macro_rules!` is textually scoped.
// `#[inline(always)]` on these lane operators is required by the design (§2.4): they must inline
// into the tiers' `#[target_feature]` stage functions.
#[allow(clippy::inline_always)]
pub mod s;
pub use s::S;

#[allow(clippy::inline_always)]
pub mod portable;
pub mod scalar;

#[cfg(target_arch = "x86_64")]
pub mod ml3;
#[cfg(target_arch = "x86_64")]
pub mod ml4;
#[cfg(target_arch = "x86_64")]
pub mod sse2;
#[cfg(target_arch = "x86_64")]
pub mod sse41;
#[cfg(target_arch = "x86_64")]
mod x86;

#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "aarch64")]
pub mod neon;

#[cfg(any(test, feature = "models"))]
pub mod model_ml3;
#[cfg(any(test, feature = "models"))]
pub mod model_ml4;
#[cfg(any(test, feature = "models"))]
pub mod model_sse2;
#[cfg(any(test, feature = "models"))]
pub mod model_sse41;
#[cfg(any(test, feature = "models"))]
pub mod x86_model;

#[cfg(any(test, feature = "models"))]
pub mod model_neon;
#[cfg(any(test, feature = "models"))]
pub mod neon_model;

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
