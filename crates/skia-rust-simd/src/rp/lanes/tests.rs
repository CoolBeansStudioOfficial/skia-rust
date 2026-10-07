// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Primitive tests (design §4.1, "Primitive exhaustive" layer):
//!
//! - every native tier against its model twin, bit for bit, under `force_tier`, on special
//!   values, sweeps over all 2^32 float bit patterns (strided; exhaustive with
//!   `SKIA_RUST_EXHAUSTIVE=1`), every 16-bit value, and random lanes;
//! - known answers per tier, each with the C++ it comes from (these also run on the models and
//!   the Scalar tier under Miri);
//! - structural checks (lowp primitives that Skia defines as two highp calls).

// Under Miri the native-vs-model tests are compiled out (intrinsics), leaving helpers unused.
#![cfg_attr(miri, allow(dead_code))]

use super::test_support::{
    Budget, Kind, LowpPrim, Prim, fma_nan_ambiguous, inputs, mad_nan_ambiguous,
};
use crate::estimates::{EstimateOp, arm};
use crate::testing::force_tier;
use crate::tier::{Backend, Estimates, Selection, Tier};

/// The x86 tiers.
const X86: [Tier; 4] = [Tier::Sse2, Tier::Sse41, Tier::Ml3, Tier::Ml4];

/// The estimate instruction an x86 tier's `rcp_approx` uses.
fn estimate_op(tier: Tier) -> EstimateOp {
    if tier == Tier::Ml4 {
        EstimateOp::Rcp14
    } else {
        EstimateOp::Rcpps
    }
}

/// Runs `harness_highp` of the lane module that executes `sel`.
pub(crate) fn run_highp(sel: Selection, op: Prim, a: &[u32], b: &[u32], c: &[u32]) -> Vec<u32> {
    match (sel.tier, sel.backend) {
        (Tier::Scalar, _) => super::scalar::harness_highp(op, a, b, c),
        #[cfg(target_arch = "x86_64")]
        (Tier::Sse2, Backend::Native) => {
            let _tok = crate::cpu::Sse2Token::get().expect("native Sse2 (Selection::check)");
            // SAFETY: `harness_highp` enables exactly `Sse2Token::FEATURES` ("sse2"); `_tok`
            // exists only because those features were detected at run time.
            unsafe { super::sse2::harness_highp(op, a, b, c) }
        }
        #[cfg(target_arch = "x86_64")]
        (Tier::Sse41, Backend::Native) => {
            let _tok = crate::cpu::Sse41Token::get().expect("native Sse41 (Selection::check)");
            // SAFETY: `harness_highp` enables exactly `Sse41Token::FEATURES`
            // ("sse2,ssse3,sse4.1"); `_tok` exists only because they were detected at run time.
            unsafe { super::sse41::harness_highp(op, a, b, c) }
        }
        #[cfg(target_arch = "x86_64")]
        (Tier::Ml3, Backend::Native) => {
            let _tok = crate::cpu::Ml3Token::get().expect("native Ml3 (Selection::check)");
            // SAFETY: `harness_highp` enables exactly `Ml3Token::FEATURES`; `_tok` exists only
            // because every one of them was detected at run time.
            unsafe { super::ml3::harness_highp(op, a, b, c) }
        }
        #[cfg(target_arch = "x86_64")]
        (Tier::Ml4, Backend::Native) => {
            let _tok = crate::cpu::Ml4Token::get().expect("native Ml4 (Selection::check)");
            // SAFETY: `harness_highp` enables exactly `Ml4Token::FEATURES`; `_tok` exists only
            // because every one of them was detected at run time.
            unsafe { super::ml4::harness_highp(op, a, b, c) }
        }
        (Tier::Sse2, Backend::Model(Estimates::Host)) => {
            super::model_sse2::host::harness_highp(op, a, b, c)
        }
        (Tier::Sse41, Backend::Model(Estimates::Host)) => {
            super::model_sse41::host::harness_highp(op, a, b, c)
        }
        (Tier::Sse2, Backend::Model(Estimates::AmdZen4)) => {
            super::model_sse2::amd_zen4::harness_highp(op, a, b, c)
        }
        (Tier::Sse41, Backend::Model(Estimates::AmdZen4)) => {
            super::model_sse41::amd_zen4::harness_highp(op, a, b, c)
        }
        (Tier::Ml3, Backend::Model(Estimates::Host)) => {
            super::model_ml3::host::harness_highp(op, a, b, c)
        }
        (Tier::Ml3, Backend::Model(Estimates::AmdZen4)) => {
            super::model_ml3::amd_zen4::harness_highp(op, a, b, c)
        }
        (Tier::Ml4, Backend::Model(Estimates::Host)) => {
            super::model_ml4::host::harness_highp(op, a, b, c)
        }
        (Tier::Ml4, Backend::Model(Estimates::AmdZen4)) => {
            super::model_ml4::amd_zen4::harness_highp(op, a, b, c)
        }
        #[cfg(target_arch = "aarch64")]
        (Tier::Neon, Backend::Native) => {
            let _tok = crate::cpu::NeonToken::get().expect("native Neon (Selection::check)");
            // SAFETY: `harness_highp` enables exactly `NeonToken::FEATURES` ("neon"); `_tok`
            // exists only because NEON was detected at run time.
            unsafe { super::neon::harness_highp(op, a, b, c) }
        }
        (Tier::Neon, Backend::Model(Estimates::Host)) => {
            super::model_neon::host::harness_highp(op, a, b, c)
        }
        (Tier::Neon, Backend::Model(Estimates::Arm)) => {
            super::model_neon::arm::harness_highp(op, a, b, c)
        }
        _ => panic!("no lane module for {sel} yet"),
    }
}

/// Runs `harness_lowp` of the lane module that executes `sel`.
fn run_lowp(sel: Selection, op: LowpPrim, a: &[u32], b: &[u32], c: &[u32]) -> Vec<u32> {
    match (sel.tier, sel.backend) {
        #[cfg(target_arch = "x86_64")]
        (Tier::Sse2, Backend::Native) => {
            let _tok = crate::cpu::Sse2Token::get().expect("native Sse2 (Selection::check)");
            // SAFETY: `harness_lowp` enables exactly `Sse2Token::FEATURES` ("sse2"); `_tok`
            // exists only because those features were detected at run time.
            unsafe { super::sse2::lowp::harness_lowp(op, a, b, c) }
        }
        #[cfg(target_arch = "x86_64")]
        (Tier::Sse41, Backend::Native) => {
            let _tok = crate::cpu::Sse41Token::get().expect("native Sse41 (Selection::check)");
            // SAFETY: `harness_lowp` enables exactly `Sse41Token::FEATURES`
            // ("sse2,ssse3,sse4.1"); `_tok` exists only because they were detected at run time.
            unsafe { super::sse41::lowp::harness_lowp(op, a, b, c) }
        }
        #[cfg(target_arch = "x86_64")]
        (Tier::Ml3, Backend::Native) => {
            let _tok = crate::cpu::Ml3Token::get().expect("native Ml3 (Selection::check)");
            // SAFETY: `harness_lowp` enables exactly `Ml3Token::FEATURES`; `_tok` exists only
            // because every one of them was detected at run time.
            unsafe { super::ml3::lowp::harness_lowp(op, a, b, c) }
        }
        #[cfg(target_arch = "x86_64")]
        (Tier::Ml4, Backend::Native) => {
            let _tok = crate::cpu::Ml4Token::get().expect("native Ml4 (Selection::check)");
            // SAFETY: `harness_lowp` enables exactly `Ml4Token::FEATURES`; `_tok` exists only
            // because every one of them was detected at run time.
            unsafe { super::ml4::lowp::harness_lowp(op, a, b, c) }
        }
        (Tier::Sse2, Backend::Model(Estimates::Host)) => {
            super::model_sse2::host::lowp::harness_lowp(op, a, b, c)
        }
        (Tier::Sse41, Backend::Model(Estimates::Host)) => {
            super::model_sse41::host::lowp::harness_lowp(op, a, b, c)
        }
        (Tier::Sse2, Backend::Model(Estimates::AmdZen4)) => {
            super::model_sse2::amd_zen4::lowp::harness_lowp(op, a, b, c)
        }
        (Tier::Sse41, Backend::Model(Estimates::AmdZen4)) => {
            super::model_sse41::amd_zen4::lowp::harness_lowp(op, a, b, c)
        }
        (Tier::Ml3, Backend::Model(Estimates::Host)) => {
            super::model_ml3::host::lowp::harness_lowp(op, a, b, c)
        }
        (Tier::Ml3, Backend::Model(Estimates::AmdZen4)) => {
            super::model_ml3::amd_zen4::lowp::harness_lowp(op, a, b, c)
        }
        (Tier::Ml4, Backend::Model(Estimates::Host)) => {
            super::model_ml4::host::lowp::harness_lowp(op, a, b, c)
        }
        (Tier::Ml4, Backend::Model(Estimates::AmdZen4)) => {
            super::model_ml4::amd_zen4::lowp::harness_lowp(op, a, b, c)
        }
        #[cfg(target_arch = "aarch64")]
        (Tier::Neon, Backend::Native) => {
            let _tok = crate::cpu::NeonToken::get().expect("native Neon (Selection::check)");
            // SAFETY: `harness_lowp` enables exactly `NeonToken::FEATURES` ("neon"); `_tok`
            // exists only because NEON was detected at run time.
            unsafe { super::neon::lowp::harness_lowp(op, a, b, c) }
        }
        (Tier::Neon, Backend::Model(Estimates::Host)) => {
            super::model_neon::host::lowp::harness_lowp(op, a, b, c)
        }
        (Tier::Neon, Backend::Model(Estimates::Arm)) => {
            super::model_neon::arm::lowp::harness_lowp(op, a, b, c)
        }
        _ => panic!("no lowp lane module for {sel}"),
    }
}

/// Forces `sel` on this thread and runs the highp primitive through the selected tier, the way
/// pipelines will pick their tier (`crate::selection()`).
fn forced_highp(sel: Selection, op: Prim, [a, b, c]: &[Vec<u32>; 3]) -> Vec<u32> {
    let _guard = force_tier(sel).unwrap_or_else(|e| panic!("{e}"));
    run_highp(crate::selection(), op, a, b, c)
}

/// [`forced_highp`] for lowp.
fn forced_lowp(sel: Selection, op: LowpPrim, [a, b, c]: &[Vec<u32>; 3]) -> Vec<u32> {
    let _guard = force_tier(sel).unwrap_or_else(|e| panic!("{e}"));
    run_lowp(crate::selection(), op, a, b, c)
}

/// Where a primitive's NaN result is unspecified, so that less than every bit is compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NanRule {
    /// Every lane is compared bit for bit.
    Exact,
    /// An unfused `mad` (`outer_commutes`) or `nmad`: two NaNs meeting in one commutative
    /// operation (`mad_nan_ambiguous`); only NaN-ness is compared there.
    Unfused { outer_commutes: bool },
    /// A fused `mad`: two NaN operands of one FMA (`fma_nan_ambiguous`); only NaN-ness is
    /// compared there.
    Fused,
    /// A fused `nmad` (`fnmadd(f, m, a)`), or Ml3/Ml4's `rcp_precise` (`fnmadd(v, e, 2) * e`):
    /// as [`NanRule::Fused`], and when the negated operand `f` (or `v`) is NaN the result's
    /// **sign** is not specified: the intrinsic is `fma(-f, m, a)`, and whether the compiler
    /// folds the negation into `vfnmadd` (which never negates a NaN, like Skia's release build
    /// and ours) or materializes it (a sign flip, as unoptimized builds do) depends on
    /// optimization. With `debug_assertions` everything but the sign bit is compared there;
    /// optimized test builds compare every bit.
    FusedNeg,
    /// Neon's fused `nmad` (`vfmsq_f32(a, f, m)`, i.e. `fma(-f, m, a)`): as [`NanRule::Fused`],
    /// and when `f` or `m` is NaN the result's **sign** is not specified: LLVM selects `FMLS`,
    /// which negates one multiplicand, and which one it puts first is a code-generation choice
    /// (the unoptimized test build negates `m`, which the model follows). Everything but the
    /// sign bit is compared there, in every build.
    NeonFusedNeg,
}

impl NanRule {
    /// The rule for highp `op` on `tier` (`mad`/`nmad`/`rcp_precise` are fused on Ml3/Ml4;
    /// `mad`/`nmad` on Neon).
    fn highp(tier: Tier, op: Prim) -> NanRule {
        let fused = matches!(tier, Tier::Ml3 | Tier::Ml4 | Tier::Neon);
        match op {
            Prim::Mad if fused => NanRule::Fused,
            // Neon's rcp_precise uses FRECPS, which negates inside the instruction: exact.
            Prim::Nmad if tier == Tier::Neon => NanRule::NeonFusedNeg,
            Prim::RcpPrecise if tier == Tier::Neon => NanRule::Exact,
            Prim::Nmad | Prim::RcpPrecise if fused => NanRule::FusedNeg,
            Prim::Mad => NanRule::Unfused {
                outer_commutes: true,
            },
            Prim::Nmad => NanRule::Unfused {
                outer_commutes: false,
            },
            _ => NanRule::Exact,
        }
    }

    /// The rule for lowp `op` on `tier` (lowp `mad`/`nmad` are unfused on every tier; lowp
    /// `rcp_precise` is highp's).
    fn lowp(tier: Tier, op: LowpPrim) -> NanRule {
        match op {
            LowpPrim::Mad => NanRule::Unfused {
                outer_commutes: true,
            },
            LowpPrim::Nmad => NanRule::Unfused {
                outer_commutes: false,
            },
            LowpPrim::RcpPrecise => NanRule::highp(tier, Prim::RcpPrecise),
            _ => NanRule::Exact,
        }
    }

    /// Whether `native` and `model` (which differ) are both acceptable results for the inputs
    /// `f, m, a` under this rule.
    fn accepts(self, [f, m, a]: [u32; 3], native: u32, model: u32) -> bool {
        let nan = |x: u32| f32::from_bits(x).is_nan();
        if !(nan(native) && nan(model)) {
            return false;
        }
        match self {
            NanRule::Exact => false,
            NanRule::Unfused { outer_commutes } => mad_nan_ambiguous(f, m, a, outer_commutes),
            NanRule::Fused => fma_nan_ambiguous(f, m, a),
            NanRule::FusedNeg => {
                fma_nan_ambiguous(f, m, a)
                    || (cfg!(debug_assertions) && nan(f) && (native ^ model) == 0x8000_0000)
            }
            NanRule::NeonFusedNeg => {
                fma_nan_ambiguous(f, m, a)
                    || ((nan(f) || nan(m)) && (native ^ model) == 0x8000_0000)
            }
        }
    }
}

/// Asserts `native == model` lane by lane, except where `rule` says the result is unspecified.
fn assert_same(
    what: &str,
    rule: NanRule,
    [a, b, c]: &[Vec<u32>; 3],
    native: &[u32],
    model: &[u32],
) {
    assert_eq!(native.len(), model.len());
    let bad: Vec<usize> = (0..native.len())
        .filter(|&i| native[i] != model[i])
        .filter(|&i| !rule.accepts([a[i], b[i], c[i]], native[i], model[i]))
        .collect();
    let detail = |&i: &usize| {
        format!(
            "lane {i}: in ({:#010x}, {:#010x}, {:#010x}) native {:#010x} model {:#010x}",
            a[i], b[i], c[i], native[i], model[i]
        )
    };
    assert!(
        bad.is_empty(),
        "{what}: {} of {} lanes differ; {}",
        bad.len(),
        native.len(),
        bad.iter()
            .take(8)
            .map(detail)
            .collect::<Vec<_>>()
            .join("; ")
    );
}

/// The native SIMD tiers this host runs (x86 or Neon), with a note for the ones it cannot.
fn native_tiers() -> Vec<Tier> {
    X86.into_iter()
        .chain([Tier::Neon])
        .filter(|&t| {
            let estimates = if t == Tier::Neon {
                arm::host_available()
            } else {
                estimate_op(t).is_available()
            };
            let ok = t.is_native() && estimates;
            if !ok {
                eprintln!("skipping native {t}: not supported by this host");
            }
            ok
        })
        .collect()
}

/// Whether this host's `rcpps`/`rsqrtps` are the oracle host's (fingerprints, design §4.6).
/// Measured once per test process.
fn host_is_amd_zen4(tier: Tier) -> bool {
    use crate::estimates::{AMD_ZEN4, Fingerprints};
    static HOST: std::sync::OnceLock<Fingerprints> = std::sync::OnceLock::new();
    HOST.get_or_init(Fingerprints::host)
        .matches_for(&AMD_ZEN4, tier)
}

/// The models a native tier is compared with: `Model(Host)` always, and `Model(AmdZen4)`, which
/// must be bit-identical too wherever the host's estimates are the oracle host's. On other hosts
/// `Model(AmdZen4)` is compared only on the primitives that use no estimates.
///
/// `Neon`'s model is compared with `Model(Host)` and `Model(Arm)` on every primitive: the Arm
/// estimates are architectural, so they must be the host's.
fn twin_models(tier: Tier, uses_estimates: bool) -> Vec<Selection> {
    let mut sels = vec![Selection::model(tier, Estimates::Host)];
    if tier == Tier::Neon {
        sels.push(Selection::model(tier, Estimates::Arm));
    } else if !uses_estimates || host_is_amd_zen4(tier) {
        sels.push(Selection::model(tier, Estimates::AmdZen4));
    }
    sels
}

#[test]
#[cfg(not(miri))] // intrinsics: Miri emulates rcpps/rsqrtps with random error
fn native_matches_model_highp() {
    let budget = Budget::current();
    for tier in native_tiers() {
        if !host_is_amd_zen4(tier) {
            eprintln!("{tier}: host estimates differ from AmdZen4; comparing those with Host only");
        }
        // One thread per primitive (`force_tier` is per thread).
        std::thread::scope(|s| {
            for (seed, op) in (1u64..).zip(Prim::ALL) {
                s.spawn(move || {
                    let ins = inputs(op.kind(), seed, budget);
                    let native = forced_highp(Selection::native(tier), op, &ins);
                    let rule = NanRule::highp(tier, op);
                    for sel in twin_models(tier, op.uses_estimates()) {
                        let model = forced_highp(sel, op, &ins);
                        assert_same(&format!("{sel} {op:?}"), rule, &ins, &native, &model);
                    }
                });
            }
        });
    }
}

#[test]
#[cfg(not(miri))] // intrinsics: Miri emulates rcpps/rsqrtps with random error
fn native_matches_model_lowp() {
    let budget = Budget::current();
    for tier in native_tiers() {
        // One thread per primitive (`force_tier` is per thread).
        std::thread::scope(|s| {
            for (seed, op) in (100u64..).zip(LowpPrim::ALL) {
                s.spawn(move || {
                    let ins = inputs(op.kind(), seed, budget);
                    let native = forced_lowp(Selection::native(tier), op, &ins);
                    let rule = NanRule::lowp(tier, op);
                    for sel in twin_models(tier, op.uses_estimates()) {
                        let model = forced_lowp(sel, op, &ins);
                        assert_same(&format!("{sel} lowp {op:?}"), rule, &ins, &native, &model);
                    }
                });
            }
        });
    }
}

/// Every unary float primitive on all 2^32 inputs, native vs model. Takes minutes in a release
/// build; runs only with `SKIA_RUST_EXHAUSTIVE=1` (the server's nightly layer, design §4.1).
#[test]
#[cfg(not(miri))]
fn exhaustive_unary_native_matches_model() {
    const CHUNK: u32 = 1 << 22;
    if std::env::var_os("SKIA_RUST_EXHAUSTIVE").is_none() {
        eprintln!("skipping exhaustive_unary_native_matches_model: set SKIA_RUST_EXHAUSTIVE=1");
        return;
    }
    let unary: Vec<Prim> = Prim::ALL
        .into_iter()
        .filter(|op| op.kind() == Kind::UnaryF)
        .collect();
    let lowp_unary: Vec<LowpPrim> = LowpPrim::ALL
        .into_iter()
        .filter(|op| op.kind() == Kind::UnaryF)
        .collect();
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let chunks: Vec<u32> = (0..=u32::MAX - (CHUNK - 1))
        .step_by(CHUNK as usize)
        .collect();
    for tier in native_tiers() {
        // Lanes compared (each native result against each twin model), for the report.
        let compared = std::sync::atomic::AtomicU64::new(0);
        std::thread::scope(|s| {
            for t in 0..threads {
                let (chunks, unary, lowp_unary) = (&chunks, &unary, &lowp_unary);
                let compared = &compared;
                s.spawn(move || {
                    let count = |n: usize| {
                        compared.fetch_add(n as u64, std::sync::atomic::Ordering::Relaxed);
                    };
                    for &start in chunks.iter().skip(t).step_by(threads) {
                        let a: Vec<u32> = (start..=start + (CHUNK - 1)).collect();
                        let ins = [a, vec![0; CHUNK as usize], vec![0; CHUNK as usize]];
                        for &op in unary {
                            let native = forced_highp(Selection::native(tier), op, &ins);
                            let rule = NanRule::highp(tier, op);
                            for sel in twin_models(tier, op.uses_estimates()) {
                                let model = forced_highp(sel, op, &ins);
                                let what = format!("{sel} {op:?}");
                                assert_same(&what, rule, &ins, &native, &model);
                                count(native.len());
                            }
                        }
                        for &op in lowp_unary {
                            let native = forced_lowp(Selection::native(tier), op, &ins);
                            let rule = NanRule::lowp(tier, op);
                            for sel in twin_models(tier, op.uses_estimates()) {
                                let model = forced_lowp(sel, op, &ins);
                                let what = format!("{sel} lowp {op:?}");
                                assert_same(&what, rule, &ins, &native, &model);
                                count(native.len());
                            }
                        }
                    }
                });
            }
        });
        eprintln!(
            "exhaustive {tier}: {} unary primitives ({} highp, {} lowp) on all 2^32 inputs, {} \
             lanes compared with the models, 0 mismatches",
            unary.len() + lowp_unary.len(),
            unary.len(),
            lowp_unary.len(),
            compared.into_inner()
        );
    }
}

/// An expected lane: exact bits, or any NaN (where the bits come from Rust arithmetic on a NaN,
/// which Rust leaves unspecified and Miri randomizes).
#[derive(Clone, Copy, Debug)]
enum E {
    Bits(u32),
    Nan,
}

impl E {
    fn matches(self, got: u32) -> bool {
        match self {
            E::Bits(b) => b == got,
            E::Nan => f32::from_bits(got).is_nan(),
        }
    }
}

fn f(x: f32) -> u32 {
    x.to_bits()
}

/// Every selection a known-answer test can run here: Scalar, the x86 models (`AmdZen4` runs on
/// any host and under Miri; `Host` without estimates too, so models are called directly) and the
/// native x86 tiers. `estimates` says whether the primitive needs the tier's estimates.
fn kat_selections(estimates: bool) -> Vec<Selection> {
    let mut sels = vec![Selection::native(Tier::Scalar)];
    for t in X86 {
        let host_estimates = estimate_op(t).is_available();
        sels.push(Selection::model(t, Estimates::AmdZen4));
        // Under Miri the Host-estimates model is the same code as the AmdZen4 one with another
        // estimate source: one representative tier (Sse2) keeps the `Host` wrappers covered.
        let host_model = !cfg!(miri) || t == Tier::Sse2;
        if host_model && (!estimates || host_estimates) {
            sels.push(Selection::model(t, Estimates::Host));
        }
        if cfg!(not(miri)) && t.is_native() && host_estimates {
            sels.push(Selection::native(t));
        }
    }
    sels
}

/// Every `Neon` selection a known-answer test can run here: `Model(Arm)` (any host, Miri),
/// `Model(Host)` (any host for estimate-free primitives, else `aarch64` only) and native `Neon`
/// on `aarch64`.
fn neon_selections(estimates: bool) -> Vec<Selection> {
    let mut sels = vec![Selection::model(Tier::Neon, Estimates::Arm)];
    if !cfg!(miri) && (!estimates || arm::host_available()) {
        sels.push(Selection::model(Tier::Neon, Estimates::Host));
    }
    if cfg!(not(miri)) && Tier::Neon.is_native() && arm::host_available() {
        sels.push(Selection::native(Tier::Neon));
    }
    sels
}

/// Checks `op` on inputs `ins` (splatted over 16 lanes) against `[scalar, sse2, sse41]`, where
/// Ml3 and Ml4 give Sse41's result.
#[track_caller]
fn kat(op: Prim, ins: [u32; 3], [s, sse2, sse41]: [E; 3]) {
    kat5(op, ins, [s, sse2, sse41, sse41, sse41]);
}

/// Checks `op` on inputs `ins` (splatted over 16 lanes) against
/// `[scalar, sse2, sse41, ml3, ml4]`.
#[track_caller]
fn kat5(op: Prim, ins: [u32; 3], expect: [E; 5]) {
    for sel in kat_selections(op.uses_estimates()) {
        let want = match sel.tier {
            Tier::Scalar => expect[0],
            Tier::Sse2 => expect[1],
            Tier::Sse41 => expect[2],
            Tier::Ml3 => expect[3],
            _ => expect[4],
        };
        let [a, b, c] = ins.map(|x| vec![x; 16]);
        let out = match sel.backend {
            Backend::Native => forced_highp(sel, op, &[a, b, c]),
            Backend::Model(_) => run_highp(sel, op, &a, &b, &c),
        };
        for (lane, &got) in out.iter().enumerate() {
            assert!(
                want.matches(got),
                "{sel} {op:?}({:#010x}, {:#010x}, {:#010x}) lane {lane}: got {got:#010x}, want {want:x?}",
                ins[0],
                ins[1],
                ins[2]
            );
        }
    }
}

/// The same result on every tier.
fn all3(x: u32) -> [E; 3] {
    [E::Bits(x); 3]
}

/// Scalar differs from the x86 tiers.
fn sx(scalar: E, x86: u32) -> [E; 3] {
    [scalar, E::Bits(x86), E::Bits(x86)]
}

const NAN: u32 = 0x7fc0_0000;
const INDEFINITE: u32 = 0xffc0_0000;

#[test]
fn known_answers_min_max_abs() {
    use Prim::{AbsF, AbsI, MaxF, MinF, MinI, MinU};
    // Scalar `min` is fminf (`opts#L133`); Sse2/Sse41 `min` is `_mm_min_ps(a,b)` (`opts#L950`),
    // which returns its second operand when either is NaN or when comparing ±0.
    kat(MinF, [NAN, f(1.0), 0], all3(f(1.0)));
    kat(MinF, [f(1.0), NAN, 0], sx(E::Bits(f(1.0)), NAN));
    kat(MinF, [f(-0.0), f(0.0), 0], sx(E::Bits(f(-0.0)), f(0.0)));
    kat(MinF, [f(0.0), f(-0.0), 0], all3(f(-0.0)));
    // `_mm_max_ps` (`opts#L951`) returns a signalling NaN second operand unchanged.
    kat(MaxF, [NAN, f(2.0), 0], all3(f(2.0)));
    kat(
        MaxF,
        [f(2.0), 0x7f80_0001, 0],
        sx(E::Bits(f(2.0)), 0x7f80_0001),
    );
    kat(MaxF, [f(0.0), f(-0.0), 0], sx(E::Bits(f(0.0)), f(-0.0)));
    // abs_(F): fabsf on Scalar; `_mm_and_ps(v, 0-v)` on x86 (`opts#L970`) keeps a NaN's sign.
    kat(
        AbsF,
        [0xffc0_0001, 0, 0],
        sx(E::Bits(0x7fc0_0001), 0xffc0_0001),
    );
    kat(AbsF, [f(-2.5), 0, 0], all3(f(2.5)));
    kat(AbsF, [f(-0.0), 0, 0], all3(0));
    kat(MinI, [u32::MAX, 1, 0], all3(u32::MAX));
    kat(MinU, [u32::MAX, 1, 0], all3(1));
    kat(AbsI, [0x8000_0000, 0, 0], all3(0x8000_0000));
}

#[test]
fn known_answers_floor_ceil() {
    use Prim::{Ceil, Floor};
    // SSE2 (`opts#L1069-L1070`): roundtrip = _mm_cvtepi32_ps(_mm_cvttps_epi32(v));
    // return roundtrip - if_then_else(roundtrip > v, 1, 0). cvtt(-0.0) = 0 gives +0.0.
    kat(
        Floor,
        [f(-0.0), 0, 0],
        [E::Bits(f(-0.0)), E::Bits(0), E::Bits(f(-0.0))],
    );
    // cvttps2dq(NaN) = 0x80000000, so Sse2 floors NaN to -2^31; roundps keeps the (quiet) NaN.
    kat(
        Floor,
        [NAN, 0, 0],
        [E::Nan, E::Bits(f(-2_147_483_648.0)), E::Bits(NAN)],
    );
    kat(
        Floor,
        [0x7f80_0001, 0, 0],
        [E::Nan, E::Bits(f(-2_147_483_648.0)), E::Bits(0x7fc0_0001)],
    );
    kat(
        Floor,
        [f(-3e9), 0, 0],
        [
            E::Bits(f(-3e9)),
            E::Bits(f(-2_147_483_648.0)),
            E::Bits(f(-3e9)),
        ],
    );
    kat(
        Floor,
        [f(3e9), 0, 0],
        [
            E::Bits(f(3e9)),
            E::Bits(f(-2_147_483_648.0)),
            E::Bits(f(3e9)),
        ],
    );
    kat(Floor, [f(-1.5), 0, 0], all3(f(-2.0)));
    kat(Floor, [f(2.75), 0, 0], all3(f(2.0)));
    // ceil_ (`opts#L1074-L1080`): roundtrip + if_then_else(roundtrip < v, 1, 0).
    kat(
        Ceil,
        [f(-0.5), 0, 0],
        [E::Bits(f(-0.0)), E::Bits(0), E::Bits(f(-0.0))],
    );
    kat(Ceil, [f(1.25), 0, 0], all3(f(2.0)));
}

#[test]
fn known_answers_conversions() {
    use Prim::{CastF, Iround, Round, ToI32, Trunc};
    // Scalar iround is (I32)(v + 0.5f) (`opts#L151`); x86 is _mm_cvtps_epi32 (`opts#L1042`),
    // ties to even, 0x80000000 for NaN and overflow. Scalar float→int casts saturate (wasm).
    kat(Iround, [f(2.5), 0, 0], sx(E::Bits(3), 2));
    kat(Iround, [f(-2.5), 0, 0], all3((-2i32).cast_unsigned()));
    kat(Iround, [f(0.5), 0, 0], sx(E::Bits(1), 0));
    kat(Iround, [f(-0.7), 0, 0], sx(E::Bits(0), u32::MAX));
    kat(Iround, [NAN, 0, 0], sx(E::Bits(0), 0x8000_0000));
    kat(
        Iround,
        [f(3e9), 0, 0],
        sx(E::Bits(0x7fff_ffff), 0x8000_0000),
    );
    // round (`opts#L152`, `opts#L1043`).
    kat(Round, [f(-1.0), 0, 0], sx(E::Bits(0), u32::MAX));
    kat(Round, [f(254.5), 0, 0], sx(E::Bits(255), 254));
    kat(Round, [f(255.5), 0, 0], all3(256));
    // trunc_ (`opts#L1588-L1600`): (U32)v on Scalar, (U32)convertvector<I32> elsewhere.
    kat(Trunc, [f(-1.5), 0, 0], sx(E::Bits(0), u32::MAX));
    kat(Trunc, [f(5e9), 0, 0], sx(E::Bits(u32::MAX), 0x8000_0000));
    kat(ToI32, [f(-1.5), 0, 0], all3(u32::MAX));
    kat(ToI32, [NAN, 0, 0], sx(E::Bits(0), 0x8000_0000));
    // cast(U32) (`opts#L1588-L1600`): (F)v on Scalar, convertvector((I32)v, F) elsewhere.
    kat(
        CastF,
        [0x8000_0000, 0, 0],
        sx(E::Bits(f(2_147_483_648.0)), f(-2_147_483_648.0)),
    );
    kat(
        CastF,
        [u32::MAX, 0, 0],
        sx(E::Bits(f(4_294_967_296.0)), f(-1.0)),
    );
    kat(CastF, [16_777_217, 0, 0], all3(f(16_777_216.0)));
}

#[test]
fn known_answers_pack() {
    use Prim::{PackU16, PackU32};
    // pack(U32) (`opts#L1045-L1053`): SSE2 sign-extends and packs (truncation); SSE4.1 is
    // _mm_packus_epi32 (saturation of the signed lanes to [0, 65535]); Scalar is (U16)v.
    let e = E::Bits;
    kat(
        PackU32,
        [0x0001_2345, 0, 0],
        [e(0x2345), e(0x2345), e(0xffff)],
    );
    kat(PackU32, [u32::MAX, 0, 0], [e(0xffff), e(0xffff), e(0)]);
    kat(PackU32, [0x8000, 0, 0], all3(0x8000));
    // pack(U16) (`opts#L1055-L1059`): _mm_packus_epi16 saturates the lanes as *signed* 16-bit.
    kat(PackU16, [0x1ff, 0, 0], all3(0xff));
    kat(PackU16, [0x100, 0, 0], sx(e(0), 0xff));
    kat(PackU16, [0x8000, 0, 0], all3(0));
    kat(PackU16, [0xff80, 0, 0], sx(e(0x80), 0));
    kat(PackU16, [0x7fff, 0, 0], all3(0xff));
}

#[test]
fn known_answers_masks() {
    use Prim::{All, Any, CondToMask, IfThenElseF};
    let e = E::Bits;
    // Sse2/Sse41 if_then_else is bitwise (`opts#L942-L948`); Scalar is `c ? t : e`
    // (`opts#L156`); Ml3's `_mm256_blendv_ps` (`opts#L731-L734`) and Ml4's sign-bit test +
    // `_mm512_mask_blend_ps` (`opts#L373-L382`) look only at the sign bit.
    kat5(
        IfThenElseF,
        [0x0000_ffff, 0x3f80_1234, f(2.0)],
        [
            e(0x3f80_1234),
            e(0x4000_1234),
            e(0x4000_1234),
            e(f(2.0)),
            e(f(2.0)),
        ],
    );
    kat5(
        IfThenElseF,
        [0x8000_0000, 0x3f80_1234, f(2.0)],
        [
            e(0x3f80_1234),
            e(0x4000_0000),
            e(0x4000_0000),
            e(0x3f80_1234),
            e(0x3f80_1234),
        ],
    );
    kat(IfThenElseF, [0, f(1.0), f(2.0)], all3(f(2.0)));
    // any/all: `_mm_movemask_ps` (`opts#L1062-L1063`) reads only the sign bits; Ml3's
    // `_mm256_testz_si256`/`_mm256_testc_si256` (`opts#L736-L738`) test every bit of the
    // register; Ml4's `_mm512_test_epi32_mask` (`opts#L383-L390`) tests each lane for nonzero.
    kat5(Any, [1, 0, 0], [e(1), e(0), e(0), e(1), e(1)]);
    kat5(Any, [0x7fff_ffff, 0, 0], [e(1), e(0), e(0), e(1), e(1)]);
    kat5(All, [0x8000_0000, 0, 0], [e(1), e(1), e(1), e(0), e(1)]);
    kat5(All, [1, 0, 0], [e(1), e(0), e(0), e(0), e(1)]);
    kat(All, [u32::MAX, 0, 0], all3(1));
    kat(Any, [0, 0, 0], all3(0));
    // cond_to_mask (`opts#L2277-L2286`): Scalar turns 0/1 into 0/~0; SIMD is the identity.
    kat(CondToMask, [1, 0, 0], sx(E::Bits(u32::MAX), 1));
    kat(CondToMask, [u32::MAX, 0, 0], all3(u32::MAX));
}

#[test]
fn known_answers_half() {
    use Prim::{FromHalf, ToHalf};
    let e = E::Bits;
    // Ml3/Ml4 (`opts#L1651-L1682`) use F16C's `vcvtph2ps`/`vcvtps2ph`: IEEE conversions (half
    // denormals, inf and NaN kept; round to nearest even with _MM_FROUND_CUR_DIRECTION).
    let soft_f16c = |soft: u32, f16c: u32| [e(soft), e(soft), e(soft), e(f16c), e(f16c)];
    // from_half (`opts#L1651-L1672`, software): denormal halfs flush to +0 (sign lost), and
    // the exponent is rebiased without special-casing inf/NaN.
    kat5(FromHalf, [0x0001, 0, 0], soft_f16c(0, 0x3380_0000)); // 2^-24
    kat5(FromHalf, [0x83ff, 0, 0], soft_f16c(0, 0xb87f_c000));
    kat(FromHalf, [0x3c00, 0, 0], all3(f(1.0)));
    kat(FromHalf, [0x8400, 0, 0], all3(f(-6.103_515_6e-5)));
    kat5(
        FromHalf,
        [0x7c00, 0, 0],
        soft_f16c(f(65536.0), f(f32::INFINITY)),
    );
    kat5(
        FromHalf,
        [0x7c01, 0, 0],
        soft_f16c(0x4780_2000, 0x7fc0_2000),
    ); // sNaN, quieted
    // to_half (`opts#L1674-L1695`, software): truncates, flushes, and ends in the tier's pack.
    kat(ToHalf, [f(1.0), 0, 0], all3(0x3c00));
    kat(ToHalf, [f(-1.0), 0, 0], all3(0xbc00));
    // 1 + 2^-11 + 2^-12: truncated by the software path, rounded up by F16C.
    kat5(ToHalf, [0x3f80_1800, 0, 0], soft_f16c(0x3c00, 0x3c01));
    // 1 + 2^-11: a tie, to even.
    kat(ToHalf, [0x3f80_1000, 0, 0], all3(0x3c00));
    // -2^-20: flushed (sign lost) by the software path, a half denormal with F16C.
    kat5(ToHalf, [f(-9.536_743e-7), 0, 0], soft_f16c(0, 0x8010));
    kat(ToHalf, [f(65536.0), 0, 0], all3(0x7c00));
    // 65519.99 truncates to 65504 (0x7bff) in software; F16C rounds it to 65504 too, while
    // 65520 is the first value F16C rounds to infinity.
    kat5(ToHalf, [f(65520.0), 0, 0], soft_f16c(0x7bff, 0x7c00));
    // (s>>16) + (em>>13) - (112<<10) exceeds 16 bits for |f| >= 2^32: truncated by Scalar and
    // Sse2's pack, saturated to 0xffff by Sse4.1's _mm_packus_epi32; F16C gives ±inf / qNaN.
    kat5(
        ToHalf,
        [f(f32::MAX), 0, 0],
        [e(0x3bff), e(0x3bff), e(0xffff), e(0x7c00), e(0x7c00)],
    );
    kat5(
        ToHalf,
        [f(f32::INFINITY), 0, 0],
        [e(0x3c00), e(0x3c00), e(0xffff), e(0x7c00), e(0x7c00)],
    );
    kat5(
        ToHalf,
        [f(f32::NEG_INFINITY), 0, 0],
        [e(0xbc00), e(0xbc00), e(0xffff), e(0xfc00), e(0xfc00)],
    );
    kat5(
        ToHalf,
        [NAN, 0, 0],
        [e(0x3e00), e(0x3e00), e(0xffff), e(0x7e00), e(0x7e00)],
    );
}

#[test]
fn known_answers_div() {
    use Prim::{DivI32, DivU32};
    let e = E::Bits;
    let i = |x: i32| x.cast_unsigned();
    // Scalar: the generic div_fn (`opts#L4950-L4970`): x/0 → x/-1, INT_MIN/-1 → INT_MIN/-2,
    // u/0 → u/0xffffffff. x86: through f64 and cvttpd2dq (`opts#L985-L1040`): x/0 and
    // INT_MIN/-1 give 0x80000000; uint operands are clamped to INT_MAX first.
    kat(DivI32, [7, 0, 0], sx(e(i(-7)), 0x8000_0000));
    kat(
        DivI32,
        [0x8000_0000, i(-1), 0],
        sx(e(0x4000_0000), 0x8000_0000),
    );
    kat(DivI32, [0x8000_0000, 0, 0], sx(e(0x4000_0000), 0x8000_0000));
    kat(DivI32, [i(-7), 2, 0], all3(i(-3)));
    kat(DivI32, [0, 0, 0], sx(e(0), 0x8000_0000));
    // Ml4 (`opts#L491-L540`) converts the uint operands exactly (`_mm256_cvtepu32_pd`) and
    // back with `_mm256_cvttpd_epu32`, whose indefinite (x/0) is 0xffffffff.
    let ml4 = |s: E, x86: u32, ml4: u32| [s, e(x86), e(x86), e(x86), e(ml4)];
    kat5(DivU32, [7, 0, 0], ml4(e(0), 0x8000_0000, u32::MAX));
    kat5(DivU32, [0, 0, 0], ml4(e(0), 0x8000_0000, u32::MAX));
    kat5(
        DivU32,
        [u32::MAX, 2, 0],
        ml4(e(0x7fff_ffff), 0x3fff_ffff, 0x7fff_ffff),
    );
    kat5(
        DivU32,
        [0x8000_0000, 3, 0],
        ml4(e(0x2aaa_aaaa), 0x2aaa_aaaa, 0x2aaa_aaaa),
    );
    kat5(DivU32, [u32::MAX, 0x8000_0000, 0], ml4(e(1), 1, 1));
    kat5(DivU32, [10, u32::MAX, 0], ml4(e(0), 0, 0));
    kat(DivU32, [u32::MAX, u32::MAX, 0], all3(1));
    kat(DivU32, [10, 3, 0], all3(3));
}

#[test]
fn known_answers_arithmetic() {
    use Prim::{Mad, Nmad, RcpFast, RcpPrecise, Rsqrt, Sqrt};
    kat(Mad, [f(2.0), f(3.0), f(1.0)], all3(f(7.0)));
    kat(Nmad, [f(2.0), f(3.0), f(1.0)], all3(f(-5.0)));
    let e = E::Bits;
    // Unfused, 1e30*1e30 overflows to inf and inf + -inf is invalid: x86 returns the QNaN
    // indefinite. Ml3/Ml4's `_mm256_fmadd_ps`/`_mm512_fmadd_ps` (`opts#L698`, `opts#L343`)
    // round once: 1e60 - inf = -inf.
    let ninf = f(f32::NEG_INFINITY);
    kat5(
        Mad,
        [f(1e30), f(1e30), ninf],
        [E::Nan, e(INDEFINITE), e(INDEFINITE), e(ninf), e(ninf)],
    );
    // (1 + 2^-23)(1 - 2^-23) - 1 = -2^-46: lost to the intermediate rounding when unfused.
    let eps = f32::EPSILON;
    kat5(
        Mad,
        [f(1.0 + eps), f(1.0 - eps), f(-1.0)],
        [e(0), e(0), e(0), e(f(-(eps * eps))), e(f(-(eps * eps)))],
    );
    kat5(
        Nmad,
        [f(1.0 + eps), f(1.0 - eps), f(1.0)],
        [e(0), e(0), e(0), e(f(eps * eps)), e(f(eps * eps))],
    );
    kat(Sqrt, [f(-1.0), 0, 0], sx(E::Nan, INDEFINITE));
    kat(Sqrt, [f(-0.0), 0, 0], all3(f(-0.0)));
    kat(Sqrt, [f(6.25), 0, 0], all3(f(2.5)));
    // Estimates on special inputs are architectural (SDM: rcpps(±0) = ±inf, rcpps(±inf) = ±0,
    // rsqrtps(negative) = indefinite). Sse2's rcp_fast is rcp_precise (`opts#L1737-L1741`):
    // e*(2 - v*e) with v*e = 0*inf → indefinite. Sse4.1's is the raw estimate (`#L1742-L1745`).
    // Ml4's rcp14/rsqrt14 (`opts#L355-L356`) give the same specials.
    kat(RcpPrecise, [0, 0, 0], sx(e(f(f32::INFINITY)), INDEFINITE));
    kat(
        RcpFast,
        [0, 0, 0],
        [e(f(f32::INFINITY)), e(INDEFINITE), e(f(f32::INFINITY))],
    );
    kat(
        RcpFast,
        [f(f32::INFINITY), 0, 0],
        [e(0), e(INDEFINITE), e(0)],
    );
    kat(
        Rsqrt,
        [0, 0, 0],
        [e(f(f32::INFINITY)), e(INDEFINITE), e(f(f32::INFINITY))],
    );
    kat(
        Rsqrt,
        [f(-1.0), 0, 0],
        [E::Nan, e(INDEFINITE), e(INDEFINITE)],
    );
}

/// Checks a lowp primitive on `ins` (splatted over 16 lanes) against `[sse2, sse41]`, where
/// Ml3 and Ml4 give Sse41's result.
#[track_caller]
fn kat_lowp(op: LowpPrim, ins: [u32; 3], [sse2, sse41]: [E; 2]) {
    kat_lowp4(op, ins, [sse2, sse41, sse41, sse41]);
}

/// Checks a lowp primitive on `ins` (splatted over 16 lanes) against
/// `[sse2, sse41, ml3, ml4]`.
#[track_caller]
fn kat_lowp4(op: LowpPrim, ins: [u32; 3], expect: [E; 4]) {
    for sel in kat_selections(op.uses_estimates()) {
        let want = match sel.tier {
            Tier::Scalar => continue, // no lowp pipeline
            Tier::Sse2 => expect[0],
            Tier::Sse41 => expect[1],
            Tier::Ml3 => expect[2],
            _ => expect[3],
        };
        let [a, b, c] = ins.map(|x| vec![x; 16]);
        let out = match sel.backend {
            Backend::Native => forced_lowp(sel, op, &[a, b, c]),
            Backend::Model(_) => run_lowp(sel, op, &a, &b, &c),
        };
        for (lane, &got) in out.iter().enumerate() {
            assert!(
                want.matches(got),
                "{sel} lowp {op:?}({:#x}, {:#x}, {:#x}) lane {lane}: got {got:#x}, want {want:x?}",
                ins[0],
                ins[1],
                ins[2]
            );
        }
    }
}

#[test]
fn known_answers_lowp() {
    use LowpPrim::{
        Div255, Div255Accurate, Floor, Mad, MaxF, MaxIntrF, MinF, MinIntrF, ScaledMult, Trunc,
    };
    let both = |x: u32| [E::Bits(x); 2];
    // div255 on x86 (`opts#L5701-L5714`) is (v+255)/256, wrapping in 16 bits; div255_accurate
    // (`opts#L5716-L5728`) is (v+128 + ((v+128)>>8))>>8.
    kat_lowp(Div255, [127, 0, 0], both(1));
    kat_lowp(Div255Accurate, [127, 0, 0], both(0));
    kat_lowp(Div255, [128, 0, 0], both(1));
    kat_lowp(Div255, [255 * 255, 0, 0], both(255));
    kat_lowp(Div255, [0xffff, 0, 0], both(0));
    kat_lowp(Div255Accurate, [0xffff, 0, 0], both(0));
    // lowp max is compare-select `if_then_else(x < y, y, x)` (`opts#L5771`); max_intr is
    // _mm_max_ps per half (`opts#L5847-L5859`): they differ on NaN and ±0.
    kat_lowp(MaxF, [NAN, f(1.0), 0], both(NAN));
    kat_lowp(MaxIntrF, [NAN, f(1.0), 0], both(f(1.0)));
    kat_lowp(MaxF, [f(-0.0), f(0.0), 0], both(f(-0.0)));
    kat_lowp(MaxIntrF, [f(-0.0), f(0.0), 0], both(f(0.0)));
    kat_lowp(MinF, [f(1.0), NAN, 0], both(NAN));
    kat_lowp(MinIntrF, [NAN, f(1.0), 0], both(f(1.0)));
    // floor_ (`opts#L6008-L6035`): cast round trip on Sse2, _mm_floor_ps on Sse4.1.
    kat_lowp(Floor, [f(-0.0), 0, 0], [E::Bits(0), E::Bits(f(-0.0))]);
    kat_lowp(Floor, [f(-1.25), 0, 0], both(f(-2.0)));
    // scaled_mult (`opts#L6042-L6063`): the generic formula and pmulhrsw both wrap.
    kat_lowp(ScaledMult, [0x8000, 0x8000, 0], both(0x8000));
    kat_lowp(ScaledMult, [0x4000, 0x4000, 0], both(0x2000));
    kat_lowp(ScaledMult, [0xffff, 1, 0], both(0));
    kat_lowp(Trunc, [f(-1.5), 0, 0], both(u32::MAX));
    // lowp mad is `a+f*m` (`opts#L5920-L5934`) on every tier, unfused even on Ml3/Ml4.
    kat_lowp(
        Mad,
        [f(1e30), f(1e30), f(f32::NEG_INFINITY)],
        both(INDEFINITE),
    );
    let eps = f32::EPSILON;
    kat_lowp(Mad, [f(1.0 + eps), f(1.0 - eps), f(-1.0)], both(0));
}

/// Skia defines several lowp primitives as the highp primitive on each half (`rcp_precise`,
/// `sqrt_`, `trunc_`, `max_intr`/`min_intr` on F), so lane by lane they must agree with highp.
/// Runs on every selection available, including the models under Miri.
///
/// On `Neon`, lowp `mad`/`nmad` (unfused) and `min_intr`/`max_intr` (compare-select) are *not*
/// the highp primitives (fused `FMLA`, `FMIN`/`FMAX`), so those pairs are x86-only.
#[test]
fn lowp_halves_match_highp() {
    let budget = Budget::current();
    let pairs = [
        (LowpPrim::MinIntrF, Prim::MinF),
        (LowpPrim::MaxIntrF, Prim::MaxF),
        (LowpPrim::Trunc, Prim::Trunc),
        (LowpPrim::ToI32, Prim::ToI32),
        (LowpPrim::Sqrt, Prim::Sqrt),
        (LowpPrim::Mad, Prim::Mad),
        (LowpPrim::Nmad, Prim::Nmad),
        (LowpPrim::Floor, Prim::Floor),
        (LowpPrim::RcpPrecise, Prim::RcpPrecise),
    ];
    // Under Miri: two representative pairs (the rest are model-vs-model sweeps run natively).
    let pairs = &pairs[..if cfg!(miri) { 2 } else { pairs.len() }];
    std::thread::scope(|s| {
        for (seed, (lop, hop)) in (200u64..).zip(pairs.iter().copied()) {
            s.spawn(move || {
                let ins = inputs(lop.kind(), seed, budget);
                let est = lop.uses_estimates();
                for sel in kat_selections(est).into_iter().chain(neon_selections(est)) {
                    let neon_differs = matches!(
                        lop,
                        LowpPrim::MinIntrF | LowpPrim::MaxIntrF | LowpPrim::Mad | LowpPrim::Nmad
                    );
                    if sel.tier == Tier::Scalar || (sel.tier == Tier::Neon && neon_differs) {
                        continue;
                    }
                    // Lowp `mad` is plain arithmetic (`opts#L5920-L5934`); highp `mad` is an
                    // FMA on Ml3/Ml4 (both checked in `known_answers_lowp`/`_arithmetic`).
                    let fused = matches!(sel.tier, Tier::Ml3 | Tier::Ml4);
                    if fused && matches!(lop, LowpPrim::Mad | LowpPrim::Nmad) {
                        continue;
                    }
                    let [a, b, c] = &ins;
                    let (lo, hi) = match sel.backend {
                        Backend::Native => {
                            (forced_lowp(sel, lop, &ins), forced_highp(sel, hop, &ins))
                        }
                        Backend::Model(_) => {
                            (run_lowp(sel, lop, a, b, c), run_highp(sel, hop, a, b, c))
                        }
                    };
                    let what = format!("{sel} {lop:?} vs {hop:?}");
                    assert_same(&what, NanRule::Exact, &ins, &lo, &hi);
                }
            });
        }
    });
}

/// The Scalar tier is its own model; check its stamped software half conversions against an
/// independent per-lane reading of the C++ over every half and a float sweep.
#[test]
fn scalar_soft_half_reference() {
    let budget = Budget::current();
    let halves: Vec<u32> = (0..=0xffff).step_by(budget.u16_step as usize).collect();
    let zeros = vec![0; halves.len()];
    let got = super::scalar::harness_highp(Prim::FromHalf, &halves, &zeros, &zeros);
    for (&h, &g) in halves.iter().zip(&got) {
        let (s, em) = (h & 0x8000, h & 0x7fff);
        let want = if em < 0x0400 {
            0
        } else {
            (s << 16) + (em << 13) + ((127 - 15) << 23)
        };
        assert_eq!(g, want, "from_half({h:#06x})");
    }
    let [fs, _, _] = inputs(Kind::UnaryF, 7, budget);
    let zeros = vec![0; fs.len()];
    let got = super::scalar::harness_highp(Prim::ToHalf, &fs, &zeros, &zeros);
    for (&x, &g) in fs.iter().zip(&got) {
        let (s, em) = (x & 0x8000_0000, x & 0x7fff_ffff);
        let want = if em < 0x3880_0000 {
            0
        } else {
            ((s >> 16) + (em >> 13)).wrapping_sub((127 - 15) << 10) & 0xffff
        };
        assert_eq!(g, want, "to_half({x:#010x})");
    }
}

/// Checks a `Neon` primitive on `ins` (splatted over 16 lanes) on every `Neon` selection
/// available (`Model(Arm)` everywhere, including Miri; native on `aarch64`).
#[track_caller]
fn kat_neon(op: Prim, ins: [u32; 3], want: E) {
    for sel in neon_selections(op.uses_estimates()) {
        let [a, b, c] = ins.map(|x| vec![x; 16]);
        let out = match sel.backend {
            Backend::Native => forced_highp(sel, op, &[a, b, c]),
            Backend::Model(_) => run_highp(sel, op, &a, &b, &c),
        };
        for (lane, &got) in out.iter().enumerate() {
            assert!(
                want.matches(got),
                "{sel} {op:?}({:#010x}, {:#010x}, {:#010x}) lane {lane}: got {got:#010x}, want {want:x?}",
                ins[0],
                ins[1],
                ins[2]
            );
        }
    }
}

/// Known answers for the `Neon` tier (`opts#L205-L244`, arm64 branches), from the A64
/// instructions' Arm ARM pseudocode. Where Neon differs from the x86 tiers, the x86 answer is
/// noted.
#[test]
#[allow(clippy::too_many_lines)] // one block per primitive group
fn known_answers_neon() {
    use Prim::{
        AbsF, All, Any, CastF, Ceil, CondToMask, DivI32, DivU32, Floor, FromHalf, IfThenElseF,
        Iround, Mad, MaxF, MinF, Nmad, PackU16, PackU32, RcpApprox, RcpFast, RcpPrecise, Round,
        Rsqrt, RsqrtApprox, Sqrt, ToHalf, ToI32, Trunc,
    };
    let b = E::Bits;
    let i = |x: i32| x.cast_unsigned();
    // min/max: vminq_f32/vmaxq_f32 (`opts#L215-L220`) are FMIN/FMAX: a NaN propagates (x86
    // returns the second operand), a signalling NaN wins over a quiet one, -0 < +0.
    kat_neon(MinF, [NAN, f(1.0), 0], b(NAN));
    kat_neon(MinF, [f(1.0), NAN, 0], b(NAN));
    kat_neon(MinF, [f(1.0), 0x7f80_0001, 0], b(0x7fc0_0001));
    kat_neon(MinF, [0x7fc0_1234, 0xff80_0001, 0], b(0xffc0_0001));
    kat_neon(MinF, [f(-0.0), f(0.0), 0], b(f(-0.0)));
    kat_neon(MinF, [f(0.0), f(-0.0), 0], b(f(-0.0)));
    kat_neon(MaxF, [f(-0.0), f(0.0), 0], b(f(0.0)));
    kat_neon(MaxF, [f(0.0), f(-0.0), 0], b(f(0.0)));
    kat_neon(MaxF, [f(2.0), f(3.0), 0], b(f(3.0)));
    // abs_: vabsq_f32 (`opts#L222`) clears the sign bit, also of (signalling) NaNs.
    kat_neon(AbsF, [0xffc0_0001, 0, 0], b(0x7fc0_0001));
    kat_neon(AbsF, [0xff80_0001, 0, 0], b(0x7f80_0001));
    kat_neon(AbsF, [f(-2.5), 0, 0], b(f(2.5)));
    // floor_/ceil_: vrndmq/vrndpq (`opts#L240-L241`), exact; NaNs quieted.
    kat_neon(Floor, [f(-0.0), 0, 0], b(f(-0.0)));
    kat_neon(Floor, [0x7f80_0001, 0, 0], b(0x7fc0_0001));
    kat_neon(Floor, [f(-3e9), 0, 0], b(f(-3e9)));
    kat_neon(Floor, [f(-1.5), 0, 0], b(f(-2.0)));
    kat_neon(Ceil, [f(-0.5), 0, 0], b(f(-0.0)));
    kat_neon(Ceil, [f(1.25), 0, 0], b(f(2.0)));
    // iround/round: vcvtnq_s32_f32/vcvtnq_u32_f32 (`opts#L243-L244`): ties to even, saturating,
    // NaN -> 0 (x86: 0x80000000); `round` converts to *unsigned*, so negatives give 0.
    kat_neon(Iround, [f(2.5), 0, 0], b(2));
    kat_neon(Iround, [f(-2.5), 0, 0], b(i(-2)));
    kat_neon(Iround, [f(-0.7), 0, 0], b(u32::MAX));
    kat_neon(Iround, [NAN, 0, 0], b(0));
    kat_neon(Iround, [f(3e9), 0, 0], b(0x7fff_ffff));
    kat_neon(Iround, [f(-3e9), 0, 0], b(0x8000_0000));
    kat_neon(Round, [f(-1.0), 0, 0], b(0));
    kat_neon(Round, [f(254.5), 0, 0], b(254));
    kat_neon(Round, [f(255.5), 0, 0], b(256));
    kat_neon(Round, [f(3e9), 0, 0], b(3_000_000_000));
    kat_neon(Round, [f(5e9), 0, 0], b(u32::MAX));
    kat_neon(Round, [NAN, 0, 0], b(0));
    // trunc_ / casts: __builtin_convertvector(v, I32) (`opts#L1595-L1599`) is FCVTZS.
    kat_neon(Trunc, [f(-1.5), 0, 0], b(u32::MAX));
    kat_neon(Trunc, [f(5e9), 0, 0], b(0x7fff_ffff));
    kat_neon(Trunc, [f(-5e9), 0, 0], b(0x8000_0000));
    kat_neon(ToI32, [NAN, 0, 0], b(0));
    kat_neon(ToI32, [f(-1.5), 0, 0], b(u32::MAX));
    kat_neon(CastF, [0x8000_0000, 0, 0], b(f(-2_147_483_648.0)));
    kat_neon(CastF, [u32::MAX, 0, 0], b(f(-1.0)));
    kat_neon(CastF, [16_777_217, 0, 0], b(f(16_777_216.0)));
    // pack: __builtin_convertvector (`opts#L228-L229`): truncation.
    kat_neon(PackU32, [0x0001_2345, 0, 0], b(0x2345));
    kat_neon(PackU32, [u32::MAX, 0, 0], b(0xffff));
    kat_neon(PackU16, [0x1ff, 0, 0], b(0xff));
    kat_neon(PackU16, [0x100, 0, 0], b(0));
    kat_neon(PackU16, [0xff80, 0, 0], b(0x80));
    // if_then_else: vbslq (`opts#L231-L232`), bitwise. any/all: vmaxvq/vminvq (`opts#L235-L236`):
    // any bit of a lane counts (x86 reads only sign bits).
    kat_neon(
        IfThenElseF,
        [0x0000_ffff, 0x3f80_1234, f(2.0)],
        b(0x4000_1234),
    );
    kat_neon(Any, [1, 0, 0], b(1));
    kat_neon(Any, [0x7fff_ffff, 0, 0], b(1));
    kat_neon(Any, [0, 0, 0], b(0));
    kat_neon(All, [1, 0, 0], b(1));
    kat_neon(All, [0x8000_0000, 0, 0], b(1));
    kat_neon(CondToMask, [1, 0, 0], b(1));
    // from_half/to_half: vcvt_f32_f16/vcvt_f16_f32 (`opts#L1652-L1677`): IEEE, half denormals
    // kept, round to nearest even, overflow to infinity, NaNs quieted.
    kat_neon(FromHalf, [0x0001, 0, 0], b(0x3380_0000));
    kat_neon(FromHalf, [0x83ff, 0, 0], b(0xb87f_c000));
    kat_neon(FromHalf, [0x3c00, 0, 0], b(f(1.0)));
    kat_neon(FromHalf, [0x7c00, 0, 0], b(f(f32::INFINITY)));
    kat_neon(FromHalf, [0x7c01, 0, 0], b(0x7fc0_2000));
    kat_neon(ToHalf, [f(1.0), 0, 0], b(0x3c00));
    kat_neon(ToHalf, [0x3f80_1800, 0, 0], b(0x3c01)); // 1 + 2^-11 + 2^-12 rounds up
    kat_neon(ToHalf, [0x3f80_1000, 0, 0], b(0x3c00)); // a tie: to even
    kat_neon(ToHalf, [f(-9.536_743e-7), 0, 0], b(0x8010)); // -2^-20: a half denormal
    kat_neon(ToHalf, [f(65520.0), 0, 0], b(0x7c00));
    kat_neon(ToHalf, [f(f32::MAX), 0, 0], b(0x7c00));
    kat_neon(ToHalf, [f(f32::NEG_INFINITY), 0, 0], b(0xfc00));
    kat_neon(ToHalf, [NAN, 0, 0], b(0x7e00));
    // div_fn: the generic version (`opts#L4950-L4970`), as on Scalar.
    kat_neon(DivI32, [7, 0, 0], b(i(-7)));
    kat_neon(DivI32, [0x8000_0000, i(-1), 0], b(0x4000_0000));
    kat_neon(DivI32, [0x8000_0000, 0, 0], b(0x4000_0000));
    kat_neon(DivI32, [i(-7), 2, 0], b(i(-3)));
    kat_neon(DivU32, [7, 0, 0], b(0));
    kat_neon(DivU32, [u32::MAX, 2, 0], b(0x7fff_ffff));
    // mad/nmad: vfmaq_f32/vfmsq_f32 (`opts#L238-L239`), fused: -1 + (1+2^-23)(1-2^-23) keeps
    // -2^-46 (unfused tiers give 0), and a finite product never overflows on its way to -inf.
    let (p, q) = (1.0 + f32::EPSILON, 1.0 - f32::EPSILON);
    kat_neon(Mad, [f(2.0), f(3.0), f(1.0)], b(f(7.0)));
    kat_neon(Nmad, [f(2.0), f(3.0), f(1.0)], b(f(-5.0)));
    kat_neon(Mad, [f(p), f(q), f(-1.0)], b(0xa880_0000));
    kat_neon(Nmad, [f(p), f(q), f(1.0)], b(0x2880_0000));
    kat_neon(
        Mad,
        [f(1e30), f(1e30), f(f32::NEG_INFINITY)],
        b(0xff80_0000),
    );
    // 0 * inf is invalid: the default NaN (positive; x86's indefinite is negative).
    kat_neon(Mad, [0, f(f32::INFINITY), f(1.0)], b(NAN));
    kat_neon(Sqrt, [f(-1.0), 0, 0], b(NAN));
    kat_neon(Sqrt, [f(-0.0), 0, 0], b(f(-0.0)));
    kat_neon(Sqrt, [f(6.25), 0, 0], b(f(2.5)));
    // Estimates (`opts#L223-L225`): FRECPE(1) = 1 - 2^-9; one FRECPS step gives
    // (2 - e) * e = 1 - 2^-18, a second one 1 - 2^-36, which rounds to 1.
    kat_neon(RcpApprox, [f(1.0), 0, 0], b(0x3f7f_ffc0));
    kat_neon(RcpFast, [f(1.0), 0, 0], b(0x3f7f_ffc0));
    kat_neon(RcpPrecise, [f(1.0), 0, 0], b(f(1.0)));
    // FRSQRTE(1) = 511/512; FRSQRTS(1, e*e) * e = 1 - 1535 * 2^-28, rounded to 1 - 96 * 2^-24.
    kat_neon(RsqrtApprox, [f(1.0), 0, 0], b(0x3f7f_ffa0));
    kat_neon(Rsqrt, [f(1.0), 0, 0], b(0x3f7f_ffa0));
    // FRECPE(±0) = ±inf, FRECPS(0, inf) = 2 (not NaN), 2 * inf = inf; FRECPE(inf) = 0.
    kat_neon(RcpFast, [0, 0, 0], b(f(f32::INFINITY)));
    kat_neon(RcpFast, [0x8000_0000, 0, 0], b(f(f32::NEG_INFINITY)));
    kat_neon(RcpFast, [f(f32::INFINITY), 0, 0], b(0));
    kat_neon(RcpPrecise, [0, 0, 0], b(f(f32::INFINITY)));
    kat_neon(Rsqrt, [0, 0, 0], b(f(f32::INFINITY)));
    kat_neon(Rsqrt, [f(-1.0), 0, 0], b(NAN));
    // FRECPS negates its first operand before NaN processing, and FMUL returns its first NaN:
    // rcp_approx(+qNaN) comes out with the sign flipped.
    kat_neon(RcpApprox, [NAN, 0, 0], b(0xffc0_0000));
}

/// Known answers for the `Neon` lowp primitives.
#[test]
fn known_answers_lowp_neon() {
    use LowpPrim::{
        Div255, Div255Accurate, Floor, Mad, MaxF, MaxIntrF, MinIntrF, ScaledMult, Trunc,
    };
    let check = |op: LowpPrim, ins: [u32; 3], want: u32| {
        for sel in neon_selections(op.uses_estimates()) {
            let [a, b, c] = ins.map(|x| vec![x; 16]);
            let out = match sel.backend {
                Backend::Native => forced_lowp(sel, op, &[a, b, c]),
                Backend::Model(_) => run_lowp(sel, op, &a, &b, &c),
            };
            for (lane, &got) in out.iter().enumerate() {
                assert_eq!(got, want, "{sel} lowp {op:?}({ins:#x?}) lane {lane}");
            }
        }
    };
    // div255 (`opts#L5706-L5709`): vrshrq_n_u16(vrsraq_n_u16(v, v, 8), 8), exact rounding of
    // v/255 for byte products (x86's (v+255)/256 gives 1 for 127); div255_accurate is the same.
    check(Div255, [127, 0, 0], 0);
    check(Div255, [128, 0, 0], 1);
    check(Div255, [254, 0, 0], 1);
    check(Div255, [255 * 255, 0, 0], 255);
    check(Div255, [0xffff, 0, 0], 1); // the accumulate wraps: 0xffff + 256 = 255
    check(Div255Accurate, [127, 0, 0], 0);
    check(Div255Accurate, [0x7f00, 0, 0], 127); // 32512 / 255 = 127.498
    // min_intr/max_intr are the compare-select min/max on Neon (`opts#L5881-L5889`).
    check(MaxF, [NAN, f(1.0), 0], NAN);
    check(MaxIntrF, [NAN, f(1.0), 0], NAN);
    check(MaxIntrF, [f(-0.0), f(0.0), 0], f(-0.0));
    check(MinIntrF, [f(1.0), NAN, 0], NAN);
    // scaled_mult: vqrdmulhq_s16 (`opts#L6049-L6050`) saturates -32768 * -32768 to 32767.
    check(ScaledMult, [0x8000, 0x8000, 0], 0x7fff);
    check(ScaledMult, [0x4000, 0x4000, 0], 0x2000);
    check(ScaledMult, [0xffff, 1, 0], 0);
    // floor_: vrndmq_f32 per half (`opts#L6009-L6012`).
    check(Floor, [f(-0.0), 0, 0], f(-0.0));
    check(Floor, [f(-1.25), 0, 0], f(-2.0));
    check(Trunc, [f(5e9), 0, 0], 0x7fff_ffff);
    // lowp mad is unfused on every tier (`opts#L5920`): the product rounds to 1 first.
    let (p, q) = (1.0 + f32::EPSILON, 1.0 - f32::EPSILON);
    check(Mad, [f(p), f(q), f(-1.0)], 0);
}
