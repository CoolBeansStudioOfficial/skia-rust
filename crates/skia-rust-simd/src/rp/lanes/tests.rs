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

use super::test_support::{Budget, Kind, LowpPrim, Prim, inputs, mad_nan_ambiguous};
use crate::estimates::EstimateOp;
use crate::testing::force_tier;
use crate::tier::{Backend, Estimates, Selection, Tier};

/// The x86 tiers implemented so far.
const X86: [Tier; 2] = [Tier::Sse2, Tier::Sse41];

/// Runs `harness_highp` of the lane module that executes `sel`.
fn run_highp(sel: Selection, op: Prim, a: &[u32], b: &[u32], c: &[u32]) -> Vec<u32> {
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
        (Tier::Sse2, Backend::Model(Estimates::Host)) => {
            super::model_sse2::host::harness_highp(op, a, b, c)
        }
        (Tier::Sse41, Backend::Model(Estimates::Host)) => {
            super::model_sse41::host::harness_highp(op, a, b, c)
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
        (Tier::Sse2, Backend::Model(Estimates::Host)) => {
            super::model_sse2::host::lowp::harness_lowp(op, a, b, c)
        }
        (Tier::Sse41, Backend::Model(Estimates::Host)) => {
            super::model_sse41::host::lowp::harness_lowp(op, a, b, c)
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

/// Asserts `native == model` lane by lane. `mad_outer_commutes` is `Some` for `mad`/`nmad`,
/// whose NaN payload is unspecified when two NaNs meet (`mad_nan_ambiguous`).
fn assert_same(
    what: &str,
    mad_outer_commutes: Option<bool>,
    [a, b, c]: &[Vec<u32>; 3],
    native: &[u32],
    model: &[u32],
) {
    assert_eq!(native.len(), model.len());
    let nan = |x: u32| f32::from_bits(x).is_nan();
    let bad: Vec<usize> = (0..native.len())
        .filter(|&i| native[i] != model[i])
        .filter(|&i| {
            !mad_outer_commutes.is_some_and(|outer| {
                nan(native[i]) && nan(model[i]) && mad_nan_ambiguous(a[i], b[i], c[i], outer)
            })
        })
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

/// The native x86 tiers this host runs, with a note for the ones it cannot.
fn native_x86() -> Vec<Tier> {
    X86.into_iter()
        .filter(|t| {
            let ok = t.is_native() && EstimateOp::Rcpps.is_available();
            if !ok {
                eprintln!("skipping native {t}: not supported by this host");
            }
            ok
        })
        .collect()
}

#[test]
#[cfg(not(miri))] // intrinsics: Miri emulates rcpps/rsqrtps with random error
fn native_matches_model_highp() {
    let budget = Budget::current();
    for tier in native_x86() {
        // One thread per primitive (`force_tier` is per thread).
        std::thread::scope(|s| {
            for (seed, op) in (1u64..).zip(Prim::ALL) {
                s.spawn(move || {
                    let ins = inputs(op.kind(), seed, budget);
                    let native = forced_highp(Selection::native(tier), op, &ins);
                    let model = forced_highp(Selection::model(tier, Estimates::Host), op, &ins);
                    let mad = match op {
                        Prim::Mad => Some(true),
                        Prim::Nmad => Some(false),
                        _ => None,
                    };
                    assert_same(&format!("{tier} {op:?}"), mad, &ins, &native, &model);
                });
            }
        });
    }
}

#[test]
#[cfg(not(miri))] // intrinsics: Miri emulates rcpps/rsqrtps with random error
fn native_matches_model_lowp() {
    let budget = Budget::current();
    for tier in native_x86() {
        // One thread per primitive (`force_tier` is per thread).
        std::thread::scope(|s| {
            for (seed, op) in (100u64..).zip(LowpPrim::ALL) {
                s.spawn(move || {
                    let ins = inputs(op.kind(), seed, budget);
                    let native = forced_lowp(Selection::native(tier), op, &ins);
                    let model = forced_lowp(Selection::model(tier, Estimates::Host), op, &ins);
                    let mad = match op {
                        LowpPrim::Mad => Some(true),
                        LowpPrim::Nmad => Some(false),
                        _ => None,
                    };
                    assert_same(&format!("{tier} lowp {op:?}"), mad, &ins, &native, &model);
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
    for tier in native_x86() {
        std::thread::scope(|s| {
            for t in 0..threads {
                let (chunks, unary, lowp_unary) = (&chunks, &unary, &lowp_unary);
                s.spawn(move || {
                    for &start in chunks.iter().skip(t).step_by(threads) {
                        let a: Vec<u32> = (start..=start + (CHUNK - 1)).collect();
                        let ins = [a, vec![0; CHUNK as usize], vec![0; CHUNK as usize]];
                        for &op in unary {
                            let native = forced_highp(Selection::native(tier), op, &ins);
                            let sel = Selection::model(tier, Estimates::Host);
                            let model = forced_highp(sel, op, &ins);
                            assert_same(&format!("{tier} {op:?}"), None, &ins, &native, &model);
                        }
                        for &op in lowp_unary {
                            let native = forced_lowp(Selection::native(tier), op, &ins);
                            let sel = Selection::model(tier, Estimates::Host);
                            let model = forced_lowp(sel, op, &ins);
                            let what = format!("{tier} lowp {op:?}");
                            assert_same(&what, None, &ins, &native, &model);
                        }
                    }
                });
            }
        });
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

/// Every selection a known-answer test can run here: Scalar, the x86 models (whose primitives
/// without estimates run on any host, so they are called directly) and the native x86 tiers.
/// `estimates` says whether the primitive needs the host's `rcpps`/`rsqrtps`.
fn kat_selections(estimates: bool) -> Vec<Selection> {
    let mut sels = vec![Selection::native(Tier::Scalar)];
    for t in X86 {
        if !estimates || EstimateOp::Rcpps.is_available() {
            sels.push(Selection::model(t, Estimates::Host));
        }
        if cfg!(not(miri)) && t.is_native() && EstimateOp::Rcpps.is_available() {
            sels.push(Selection::native(t));
        }
    }
    sels
}

/// Checks `op` on inputs `ins` (splatted over 16 lanes) against `[scalar, sse2, sse41]`.
#[track_caller]
fn kat(op: Prim, ins: [u32; 3], expect: [E; 3]) {
    for sel in kat_selections(op.uses_estimates()) {
        let want = match sel.tier {
            Tier::Scalar => expect[0],
            Tier::Sse2 => expect[1],
            _ => expect[2],
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
    // x86 if_then_else is bitwise (`opts#L942-L948`); Scalar is `c ? t : e` (`opts#L156`).
    kat(
        IfThenElseF,
        [0x0000_ffff, 0x3f80_1234, f(2.0)],
        sx(E::Bits(0x3f80_1234), 0x4000_1234),
    );
    kat(IfThenElseF, [0, f(1.0), f(2.0)], all3(f(2.0)));
    // any/all: `_mm_movemask_ps` (`opts#L1062-L1063`) reads only the sign bits.
    kat(Any, [1, 0, 0], sx(E::Bits(1), 0));
    kat(Any, [0x7fff_ffff, 0, 0], sx(E::Bits(1), 0));
    kat(All, [0x8000_0000, 0, 0], all3(1));
    kat(Any, [0, 0, 0], all3(0));
    // cond_to_mask (`opts#L2277-L2286`): Scalar turns 0/1 into 0/~0; SIMD is the identity.
    kat(CondToMask, [1, 0, 0], sx(E::Bits(u32::MAX), 1));
    kat(CondToMask, [u32::MAX, 0, 0], all3(u32::MAX));
}

#[test]
fn known_answers_half() {
    use Prim::{FromHalf, ToHalf};
    let e = E::Bits;
    // from_half (`opts#L1651-L1672`, software): denormal halfs flush to +0 (sign lost), and
    // the exponent is rebiased without special-casing inf/NaN.
    kat(FromHalf, [0x0001, 0, 0], all3(0));
    kat(FromHalf, [0x83ff, 0, 0], all3(0));
    kat(FromHalf, [0x3c00, 0, 0], all3(f(1.0)));
    kat(FromHalf, [0x8400, 0, 0], all3(f(-6.103_515_6e-5)));
    kat(FromHalf, [0x7c00, 0, 0], all3(f(65536.0)));
    // to_half (`opts#L1674-L1695`, software): truncates, flushes, and ends in the tier's pack.
    kat(ToHalf, [f(1.0), 0, 0], all3(0x3c00));
    kat(ToHalf, [f(-1.0), 0, 0], all3(0xbc00));
    kat(ToHalf, [0x3f80_1800, 0, 0], all3(0x3c00)); // 1 + 2^-11 + 2^-12: truncated, not RNE
    kat(ToHalf, [f(-9.536_743e-7), 0, 0], all3(0)); // -2^-20: flushed, sign lost
    kat(ToHalf, [f(65536.0), 0, 0], all3(0x7c00));
    // (s>>16) + (em>>13) - (112<<10) exceeds 16 bits for |f| >= 2^32: truncated by Scalar and
    // Sse2's pack, saturated to 0xffff by Sse4.1's _mm_packus_epi32.
    kat(
        ToHalf,
        [f(f32::MAX), 0, 0],
        [e(0x3bff), e(0x3bff), e(0xffff)],
    );
    kat(
        ToHalf,
        [f(f32::INFINITY), 0, 0],
        [e(0x3c00), e(0x3c00), e(0xffff)],
    );
    kat(
        ToHalf,
        [f(f32::NEG_INFINITY), 0, 0],
        [e(0xbc00), e(0xbc00), e(0xffff)],
    );
    kat(ToHalf, [NAN, 0, 0], [e(0x3e00), e(0x3e00), e(0xffff)]);
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
    kat(DivU32, [7, 0, 0], sx(e(0), 0x8000_0000));
    kat(DivU32, [u32::MAX, 2, 0], sx(e(0x7fff_ffff), 0x3fff_ffff));
    kat(DivU32, [u32::MAX, u32::MAX, 0], all3(1));
    kat(DivU32, [10, 3, 0], all3(3));
}

#[test]
fn known_answers_arithmetic() {
    use Prim::{Mad, Nmad, RcpFast, RcpPrecise, Rsqrt, Sqrt};
    kat(Mad, [f(2.0), f(3.0), f(1.0)], all3(f(7.0)));
    kat(Nmad, [f(2.0), f(3.0), f(1.0)], all3(f(-5.0)));
    // inf + -inf is invalid: x86 returns the QNaN indefinite.
    kat(
        Mad,
        [f(1e30), f(1e30), f(f32::NEG_INFINITY)],
        sx(E::Nan, INDEFINITE),
    );
    kat(Sqrt, [f(-1.0), 0, 0], sx(E::Nan, INDEFINITE));
    kat(Sqrt, [f(-0.0), 0, 0], all3(f(-0.0)));
    kat(Sqrt, [f(6.25), 0, 0], all3(f(2.5)));
    // Estimates on special inputs are architectural (SDM: rcpps(±0) = ±inf, rcpps(±inf) = ±0,
    // rsqrtps(negative) = indefinite). Sse2's rcp_fast is rcp_precise (`opts#L1737-L1741`):
    // e*(2 - v*e) with v*e = 0*inf → indefinite. Sse4.1's is the raw estimate (`#L1742-L1745`).
    let e = E::Bits;
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

/// Checks a lowp primitive on `ins` (splatted over 16 lanes) against `[sse2, sse41]`.
#[track_caller]
fn kat_lowp(op: LowpPrim, ins: [u32; 3], expect: [E; 2]) {
    for sel in kat_selections(op.uses_estimates()) {
        if sel.tier == Tier::Scalar {
            continue; // no lowp pipeline
        }
        let want = if sel.tier == Tier::Sse2 {
            expect[0]
        } else {
            expect[1]
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
        Div255, Div255Accurate, Floor, MaxF, MaxIntrF, MinF, MinIntrF, ScaledMult, Trunc,
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
}

/// Skia defines several lowp primitives as the highp primitive on each half (`rcp_precise`,
/// `sqrt_`, `trunc_`, `max_intr`/`min_intr` on F), so lane by lane they must agree with highp.
/// Runs on every selection available, including the models under Miri.
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
    std::thread::scope(|s| {
        for (seed, (lop, hop)) in (200u64..).zip(pairs) {
            s.spawn(move || {
                let ins = inputs(lop.kind(), seed, budget);
                for sel in kat_selections(lop.uses_estimates()) {
                    if sel.tier == Tier::Scalar {
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
                    assert_same(&format!("{sel} {lop:?} vs {hop:?}"), None, &ins, &lo, &hi);
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
