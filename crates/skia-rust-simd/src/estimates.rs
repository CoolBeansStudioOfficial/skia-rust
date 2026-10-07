// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The host's reciprocal and reciprocal-square-root *estimate* instructions.
//!
//! `rcpps`/`rsqrtps` (and AVX-512's `vrcp14ps`/`vrsqrt14ps`) are only specified to a relative
//! error bound; Intel and AMD implement different tables, so every Skia output that reaches
//! `rcp_fast`, `rcp_precise` or `rsqrt` is vendor-specific (`docs/design/raster-pipeline.md`
//! §1.4, §4.6). This module runs the host's instructions on slices, fingerprints them so hosts can
//! be compared with the oracle host, and extracts the 4096-entry tables that model `rcpps` and
//! `rsqrtps` (used by `cargo xtask cpu-probe`).
//!
//! Software models (bit-exact on every `f32` input, verified exhaustively on the oracle host):
//! - [`amd_zen4`]: `rcp`, `rsqrt`, `rcp14`, `rsqrt14` of the oracle host, the functions model
//!   tiers use for [`Estimates::AmdZen4`](crate::Estimates::AmdZen4).
//! - [`tables`]: the committed `rcpps`/`rsqrtps` tables ([`tables::AMD_ZEN4`]) and the
//!   table-driven `rcp_approx`/`rsqrt_approx` that extend them to every input.
//! - [`recip14`]: `vrcp14ps`/`vrsqrt14ps` (Intel's reference algorithm, which Zen 4 follows).
//!
//! # Fingerprints
//! A fingerprint is FNV-1a over 32-bit words: starting from `0xcbf29ce484222325`, for each output
//! `h = (h ^ bits as u64) * 0x100000001b3` (wrapping), over the inputs `f32::from_bits(base | m)`
//! for every mantissa `m` in `0..2²³`, in increasing order, where `base` is `0x3f80_0000` for
//! `[1,2)` and `0x4000_0000` for `[2,4)`. (Because the estimates' low mantissa bits are zero,
//! the low bits of the hash are data-independent; that is expected.)

use crate::tier::Tier;

pub mod amd_zen4;
pub mod recip14;
pub mod tables;

/// One estimate instruction, applied lane-wise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EstimateOp {
    /// `rcpps` (SSE): 12-bit reciprocal estimate. Used by `Sse2`, `Sse41` and `Ml3`
    /// (`vrcpps` is the same per-lane function).
    Rcpps,
    /// `rsqrtps` (SSE): 12-bit reciprocal square root estimate.
    Rsqrtps,
    /// `vrcp14ps` (AVX-512F/VL): 14-bit reciprocal estimate. Used by `Ml4`.
    Rcp14,
    /// `vrsqrt14ps` (AVX-512F/VL): 14-bit reciprocal square root estimate.
    Rsqrt14,
}

impl EstimateOp {
    /// Every op.
    pub const ALL: [EstimateOp; 4] = [
        EstimateOp::Rcpps,
        EstimateOp::Rsqrtps,
        EstimateOp::Rcp14,
        EstimateOp::Rsqrt14,
    ];

    /// The instruction's name as used in fingerprints (`rcpps`, `rsqrtps`, `rcp14`, `rsqrt14`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            EstimateOp::Rcpps => "rcpps",
            EstimateOp::Rsqrtps => "rsqrtps",
            EstimateOp::Rcp14 => "rcp14",
            EstimateOp::Rsqrt14 => "rsqrt14",
        }
    }

    /// Whether this host can execute the instruction.
    #[must_use]
    pub fn is_available(self) -> bool {
        match self {
            EstimateOp::Rcpps | EstimateOp::Rsqrtps => x86::sse_token().is_some(),
            EstimateOp::Rcp14 | EstimateOp::Rsqrt14 => x86::avx512_token().is_some(),
        }
    }
}

/// The host lacks the requested estimate instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EstimateUnavailable(pub EstimateOp);

impl std::fmt::Display for EstimateUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "this host cannot execute {}", self.0.name())
    }
}

impl std::error::Error for EstimateUnavailable {}

/// Applies `op` to every element of `xs`, writing `out[i] = op(xs[i])`.
///
/// # Errors
/// [`EstimateUnavailable`] if the host lacks the instruction ([`EstimateOp::is_available`]).
///
/// # Panics
/// If `xs` and `out` differ in length.
pub fn host_estimates(
    op: EstimateOp,
    xs: &[f32],
    out: &mut [f32],
) -> Result<(), EstimateUnavailable> {
    assert_eq!(xs.len(), out.len(), "input and output lengths differ");
    x86::run(op, xs, out).ok_or(EstimateUnavailable(op))
}

/// Which binade a fingerprint covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Binade {
    /// `[1, 2)`: input bits `0x3f80_0000 | m`.
    One,
    /// `[2, 4)`: input bits `0x4000_0000 | m` (`rsqrt` depends on exponent parity).
    Two,
}

impl Binade {
    /// The input bits for mantissa 0.
    #[must_use]
    pub const fn base_bits(self) -> u32 {
        match self {
            Binade::One => 0x3f80_0000,
            Binade::Two => 0x4000_0000,
        }
    }
}

/// The FNV-1a 64 offset basis.
pub const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/// The FNV-1a 64 prime.
pub const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a over 32-bit words (see the module docs).
#[must_use]
pub fn fnv1a_words(words: impl IntoIterator<Item = u32>) -> u64 {
    words.into_iter().fold(FNV_OFFSET, |h, w| {
        (h ^ u64::from(w)).wrapping_mul(FNV_PRIME)
    })
}

/// Runs `op` over all 2²³ mantissas of `binade` in increasing order, calling `f(m, out_bits)`.
fn sweep(
    op: EstimateOp,
    binade: Binade,
    mut f: impl FnMut(u32, u32),
) -> Result<(), EstimateUnavailable> {
    const CHUNK: u32 = 1 << 14;
    let mut xs = vec![0.0f32; CHUNK as usize];
    let mut out = vec![0.0f32; CHUNK as usize];
    for start in (0..1u32 << 23).step_by(CHUNK as usize) {
        for (m, x) in (start..).zip(xs.iter_mut()) {
            *x = f32::from_bits(binade.base_bits() | m);
        }
        host_estimates(op, &xs, &mut out)?;
        for (m, y) in (start..).zip(&out) {
            f(m, y.to_bits());
        }
    }
    Ok(())
}

/// The host's fingerprint of `op` over `binade` (2²³ evaluations).
///
/// # Errors
/// [`EstimateUnavailable`] if the host lacks the instruction.
pub fn host_fingerprint(op: EstimateOp, binade: Binade) -> Result<u64, EstimateUnavailable> {
    let mut h = FNV_OFFSET;
    sweep(op, binade, |_, bits| {
        h = (h ^ u64::from(bits)).wrapping_mul(FNV_PRIME);
    })?;
    Ok(h)
}

/// A host's estimate fingerprints (design §1.4). `None` where the host lacks the instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fingerprints {
    pub rcpps_1_2: Option<u64>,
    pub rsqrtps_1_2: Option<u64>,
    pub rsqrtps_2_4: Option<u64>,
    pub rcp14_1_2: Option<u64>,
    pub rsqrt14_1_2: Option<u64>,
}

/// The oracle host's fingerprints (AMD Ryzen 7 7800X3D, Zen 4), measured for the design (§1.4).
/// The goldens embed these estimates.
pub const AMD_ZEN4: Fingerprints = Fingerprints {
    rcpps_1_2: Some(0xf7ba_415b_37cf_2325),
    rsqrtps_1_2: Some(0xdf1c_5fd7_5f2c_2325),
    rsqrtps_2_4: Some(0xa918_bea8_42f8_2325),
    rcp14_1_2: Some(0x80dd_67e9_2b89_c525),
    rsqrt14_1_2: Some(0x9c83_0943_fb7e_9e25),
};

impl Fingerprints {
    /// Measures this host (5 × 2²³ evaluations; well under a second in release builds).
    #[must_use]
    pub fn host() -> Fingerprints {
        Fingerprints {
            rcpps_1_2: host_fingerprint(EstimateOp::Rcpps, Binade::One).ok(),
            rsqrtps_1_2: host_fingerprint(EstimateOp::Rsqrtps, Binade::One).ok(),
            rsqrtps_2_4: host_fingerprint(EstimateOp::Rsqrtps, Binade::Two).ok(),
            rcp14_1_2: host_fingerprint(EstimateOp::Rcp14, Binade::One).ok(),
            rsqrt14_1_2: host_fingerprint(EstimateOp::Rsqrt14, Binade::One).ok(),
        }
    }

    /// The fields as `(name, value)` pairs, in the order the design lists them.
    #[must_use]
    pub fn entries(&self) -> [(&'static str, Option<u64>); 5] {
        [
            ("rcpps[1,2)", self.rcpps_1_2),
            ("rsqrtps[1,2)", self.rsqrtps_1_2),
            ("rsqrtps[2,4)", self.rsqrtps_2_4),
            ("rcp14[1,2)", self.rcp14_1_2),
            ("rsqrt14[1,2)", self.rsqrt14_1_2),
        ]
    }

    /// Whether `self` (a host) produces `reference`'s estimates for everything `tier` uses:
    /// `rcpps`/`rsqrtps` for `Sse2`/`Sse41`/`Ml3`, `rcp14`/`rsqrt14` for `Ml4`. `Scalar` uses no
    /// estimates and `Neon`'s are architectural, so both always match. A missing fingerprint
    /// never matches.
    #[must_use]
    pub fn matches_for(&self, reference: &Fingerprints, tier: Tier) -> bool {
        let same = |a: Option<u64>, b: Option<u64>| a.is_some() && a == b;
        match tier {
            Tier::Sse2 | Tier::Sse41 | Tier::Ml3 => {
                same(self.rcpps_1_2, reference.rcpps_1_2)
                    && same(self.rsqrtps_1_2, reference.rsqrtps_1_2)
                    && same(self.rsqrtps_2_4, reference.rsqrtps_2_4)
            }
            Tier::Ml4 => {
                same(self.rcp14_1_2, reference.rcp14_1_2)
                    && same(self.rsqrt14_1_2, reference.rsqrt14_1_2)
            }
            Tier::Scalar | Tier::Neon => true,
        }
    }
}

/// Number of entries per binade in an `rcpps`/`rsqrtps` table (the top 12 mantissa bits).
pub const TABLE_LEN: usize = 4096;

/// The host's `rcpps`/`rsqrtps` as tables indexed by the top 12 mantissa bits (design §2.8).
///
/// `rcpps[i]` is the output bits for input bits `0x3f80_0000 | i << 11`; `rsqrtps[i]` likewise
/// for `[1,2)` and `rsqrtps[4096 + i]` for `[2,4)` (`0x4000_0000 | i << 11`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EstimateTables {
    pub rcpps: Vec<u32>,
    pub rsqrtps: Vec<u32>,
}

/// One binade of a table: the outputs for the 4096 inputs whose low 11 mantissa bits are zero.
fn table_binade(op: EstimateOp, binade: Binade) -> Result<Vec<u32>, EstimateUnavailable> {
    let xs: Vec<f32> = (0..1u32 << 12)
        .map(|i| f32::from_bits(binade.base_bits() | (i << 11)))
        .collect();
    let mut out = vec![0.0f32; TABLE_LEN];
    host_estimates(op, &xs, &mut out)?;
    Ok(out.into_iter().map(f32::to_bits).collect())
}

impl EstimateTables {
    /// Reads the host's tables.
    ///
    /// # Errors
    /// [`EstimateUnavailable`] if the host lacks `rcpps`/`rsqrtps` (non-x86).
    pub fn host() -> Result<EstimateTables, EstimateUnavailable> {
        let rcpps = table_binade(EstimateOp::Rcpps, Binade::One)?;
        let mut rsqrtps = table_binade(EstimateOp::Rsqrtps, Binade::One)?;
        rsqrtps.extend(table_binade(EstimateOp::Rsqrtps, Binade::Two)?);
        Ok(EstimateTables { rcpps, rsqrtps })
    }

    /// Counts the host outputs over all 2²³ mantissas of `[1,2)` (`rcpps`) and of `[1,2)` and
    /// `[2,4)` (`rsqrtps`) that the tables do *not* reproduce. `(0, 0)` means the tables model
    /// the host exactly in those binades.
    ///
    /// # Errors
    /// [`EstimateUnavailable`] if the host lacks `rcpps`/`rsqrtps`.
    pub fn mismatches_vs_host(&self) -> Result<(u32, u32), EstimateUnavailable> {
        let mut rcp_bad = 0;
        sweep(EstimateOp::Rcpps, Binade::One, |m, bits| {
            rcp_bad += u32::from(self.rcpps[(m >> 11) as usize] != bits);
        })?;
        let mut rsqrt_bad = 0;
        for (binade, offset) in [(Binade::One, 0), (Binade::Two, TABLE_LEN)] {
            sweep(EstimateOp::Rsqrtps, binade, |m, bits| {
                rsqrt_bad += u32::from(self.rsqrtps[offset + (m >> 11) as usize] != bits);
            })?;
        }
        Ok((rcp_bad, rsqrt_bad))
    }

    /// The tables as little-endian `u32` bytes (`rcpps`, `rsqrtps`), the format
    /// `cargo xtask cpu-probe --dump-tables` writes.
    #[must_use]
    pub fn to_le_bytes(&self) -> (Vec<u8>, Vec<u8>) {
        let le = |v: &[u32]| v.iter().flat_map(|w| w.to_le_bytes()).collect();
        (le(&self.rcpps), le(&self.rsqrtps))
    }
}

#[cfg(all(target_arch = "x86_64", not(miri)))]
mod x86 {
    use core::arch::x86_64::{
        __m128, _mm_cvtss_f32, _mm_rcp_ps, _mm_rcp14_ps, _mm_rsqrt_ps, _mm_rsqrt14_ps, _mm_setr_ps,
        _mm_shuffle_ps,
    };

    use super::EstimateOp;
    use crate::cpu::{Ml4Token, Sse2Token};

    pub(super) fn sse_token() -> Option<Sse2Token> {
        Sse2Token::get()
    }

    pub(super) fn avx512_token() -> Option<Ml4Token> {
        Ml4Token::get()
    }

    /// The four lanes of `v`, using value-only intrinsics (no memory access).
    #[target_feature(enable = "sse2")]
    fn lanes(v: __m128) -> [f32; 4] {
        [
            _mm_cvtss_f32(v),
            _mm_cvtss_f32(_mm_shuffle_ps::<0b01_01_01_01>(v, v)),
            _mm_cvtss_f32(_mm_shuffle_ps::<0b10_10_10_10>(v, v)),
            _mm_cvtss_f32(_mm_shuffle_ps::<0b11_11_11_11>(v, v)),
        ]
    }

    /// Applies a 4-lane op to `xs` in chunks of 4, padding the last chunk with 1.0.
    #[target_feature(enable = "sse2")]
    fn map4(xs: &[f32], out: &mut [f32], op: impl Fn(__m128) -> __m128) {
        for (x, o) in xs.chunks(4).zip(out.chunks_mut(4)) {
            let mut a = [1.0f32; 4];
            a[..x.len()].copy_from_slice(x);
            let y = lanes(op(_mm_setr_ps(a[0], a[1], a[2], a[3])));
            o.copy_from_slice(&y[..o.len()]);
        }
    }

    #[target_feature(enable = "sse2")]
    fn rcpps(xs: &[f32], out: &mut [f32]) {
        map4(xs, out, |v| _mm_rcp_ps(v));
    }

    #[target_feature(enable = "sse2")]
    fn rsqrtps(xs: &[f32], out: &mut [f32]) {
        map4(xs, out, |v| _mm_rsqrt_ps(v));
    }

    #[target_feature(
        enable = "sse2,ssse3,sse4.1,sse4.2,avx,avx2,bmi1,bmi2,f16c,fma,avx512f,avx512dq,avx512cd,avx512bw,avx512vl"
    )]
    fn rcp14(xs: &[f32], out: &mut [f32]) {
        map4(xs, out, |v| _mm_rcp14_ps(v));
    }

    #[target_feature(
        enable = "sse2,ssse3,sse4.1,sse4.2,avx,avx2,bmi1,bmi2,f16c,fma,avx512f,avx512dq,avx512cd,avx512bw,avx512vl"
    )]
    fn rsqrt14(xs: &[f32], out: &mut [f32]) {
        map4(xs, out, |v| _mm_rsqrt14_ps(v));
    }

    pub(super) fn run(op: EstimateOp, xs: &[f32], out: &mut [f32]) -> Option<()> {
        match op {
            EstimateOp::Rcpps => {
                let _tok: Sse2Token = sse_token()?;
                // SAFETY: `rcpps` enables only `sse2`; `_tok` exists only if `sse2` was detected.
                unsafe { rcpps(xs, out) };
            }
            EstimateOp::Rsqrtps => {
                let _tok: Sse2Token = sse_token()?;
                // SAFETY: `rsqrtps` enables only `sse2`; `_tok` exists only if `sse2` was
                // detected.
                unsafe { rsqrtps(xs, out) };
            }
            EstimateOp::Rcp14 => {
                let _tok: Ml4Token = avx512_token()?;
                // SAFETY: `rcp14` enables exactly `Ml4Token::FEATURES`; `_tok` exists only if
                // every one of them was detected at run time.
                unsafe { rcp14(xs, out) };
            }
            EstimateOp::Rsqrt14 => {
                let _tok: Ml4Token = avx512_token()?;
                // SAFETY: `rsqrt14` enables exactly `Ml4Token::FEATURES`; `_tok` exists only if
                // every one of them was detected at run time.
                unsafe { rsqrt14(xs, out) };
            }
        }
        Some(())
    }
}

/// Non-x86-64 hosts (and Miri, whose emulated estimates are not any CPU's) have no x86
/// estimate instructions.
#[cfg(not(all(target_arch = "x86_64", not(miri))))]
mod x86 {
    use super::EstimateOp;

    pub(super) fn sse_token() -> Option<()> {
        None
    }

    pub(super) fn avx512_token() -> Option<()> {
        None
    }

    pub(super) fn run(_: EstimateOp, _: &[f32], _: &mut [f32]) -> Option<()> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_reference() {
        assert_eq!(fnv1a_words([]), FNV_OFFSET);
        // One step by hand.
        assert_eq!(
            fnv1a_words([0x3f80_0000]),
            (FNV_OFFSET ^ 0x3f80_0000).wrapping_mul(FNV_PRIME)
        );
    }

    #[test]
    fn matches_for() {
        let none = Fingerprints {
            rcpps_1_2: None,
            rsqrtps_1_2: None,
            rsqrtps_2_4: None,
            rcp14_1_2: None,
            rsqrt14_1_2: None,
        };
        assert!(AMD_ZEN4.matches_for(&AMD_ZEN4, Tier::Ml4));
        assert!(AMD_ZEN4.matches_for(&AMD_ZEN4, Tier::Sse2));
        assert!(!none.matches_for(&AMD_ZEN4, Tier::Sse41));
        assert!(!none.matches_for(&none, Tier::Sse41));
        assert!(none.matches_for(&AMD_ZEN4, Tier::Neon));
        let sse_only = Fingerprints {
            rcp14_1_2: None,
            rsqrt14_1_2: None,
            ..AMD_ZEN4
        };
        assert!(sse_only.matches_for(&AMD_ZEN4, Tier::Ml3));
        assert!(!sse_only.matches_for(&AMD_ZEN4, Tier::Ml4));
    }

    #[test]
    fn availability() {
        for op in EstimateOp::ALL {
            let mut out = [0.0f32; 3];
            let r = host_estimates(op, &[1.0, 2.0, 3.0], &mut out);
            assert_eq!(r.is_ok(), op.is_available(), "{op:?}");
        }
        assert_eq!(
            EstimateOp::Rcpps.is_available(),
            cfg!(all(target_arch = "x86_64", not(miri)))
        );
        if cfg!(all(target_arch = "x86_64", not(miri))) {
            assert_eq!(EstimateOp::Rcp14.is_available(), Tier::Ml4.is_native());
        }
    }

    /// Checks the documented error bounds and power-of-two behaviour on every available op,
    /// including a tail that does not fill a 4-lane chunk.
    #[test]
    fn host_estimates_are_estimates() {
        let mut ran = false;
        for op in EstimateOp::ALL {
            if !op.is_available() {
                eprintln!("skipping {}: not available on this host", op.name());
                continue;
            }
            ran = true;
            let xs: Vec<f32> = (0..1031u16).map(|i| 1.0 + f32::from(i) / 1031.0).collect();
            let mut out = vec![0.0f32; xs.len()];
            host_estimates(op, &xs, &mut out).unwrap();
            let (bound, sqrt) = match op {
                EstimateOp::Rcpps => (1.5 * 2f64.powi(-12), false),
                EstimateOp::Rsqrtps => (1.5 * 2f64.powi(-12), true),
                EstimateOp::Rcp14 => (2f64.powi(-14), false),
                EstimateOp::Rsqrt14 => (2f64.powi(-14), true),
            };
            for (&x, &y) in xs.iter().zip(&out) {
                let exact = if sqrt {
                    1.0 / f64::from(x).sqrt()
                } else {
                    1.0 / f64::from(x)
                };
                let rel = ((f64::from(y) - exact) / exact).abs();
                assert!(rel <= bound, "{} {x} -> {y} (rel err {rel})", op.name());
            }
            // Lane-wise: each element's result does not depend on its neighbours or position.
            let mut single = [0.0f32];
            for i in [0, 1, 2, 3, 4, 1029, 1030] {
                host_estimates(op, &xs[i..=i], &mut single).unwrap();
                assert_eq!(
                    single[0].to_bits(),
                    out[i].to_bits(),
                    "{} lane {i}",
                    op.name()
                );
            }
        }
        if !ran {
            eprintln!("skipping host_estimates_are_estimates: no x86 estimate instructions");
        }
    }

    #[test]
    fn host_tables_index_top_12_bits() {
        let Ok(t) = EstimateTables::host() else {
            eprintln!("skipping host_tables_index_top_12_bits: host has no rcpps/rsqrtps");
            return;
        };
        assert_eq!(t.rcpps.len(), TABLE_LEN);
        assert_eq!(t.rsqrtps.len(), 2 * TABLE_LEN);
        let mut y = [0.0f32];
        for i in [0u32, 1, 2047, 4095] {
            host_estimates(
                EstimateOp::Rcpps,
                &[f32::from_bits(0x3f80_0000 | i << 11)],
                &mut y,
            )
            .unwrap();
            assert_eq!(t.rcpps[i as usize], y[0].to_bits());
            host_estimates(
                EstimateOp::Rsqrtps,
                &[f32::from_bits(0x4000_0000 | i << 11)],
                &mut y,
            )
            .unwrap();
            assert_eq!(t.rsqrtps[TABLE_LEN + i as usize], y[0].to_bits());
        }
        let (rcp, rsqrt) = t.to_le_bytes();
        assert_eq!(rcp.len(), 4 * TABLE_LEN);
        assert_eq!(rsqrt.len(), 8 * TABLE_LEN);
        assert_eq!(rcp[..4], t.rcpps[0].to_le_bytes());
    }

    /// The fingerprint equals FNV-1a over the explicitly computed outputs, so `cpu-probe`
    /// fingerprints mean what the module docs say.
    #[test]
    fn fingerprint_definition() {
        if !EstimateOp::Rcpps.is_available() {
            eprintln!("skipping fingerprint_definition: host has no rcpps");
            return;
        }
        let xs: Vec<f32> = (0..1u32 << 23)
            .map(|m| f32::from_bits(0x3f80_0000 | m))
            .collect();
        let mut out = vec![0.0f32; xs.len()];
        host_estimates(EstimateOp::Rcpps, &xs, &mut out).unwrap();
        assert_eq!(
            host_fingerprint(EstimateOp::Rcpps, Binade::One).unwrap(),
            fnv1a_words(out.iter().map(|y| y.to_bits()))
        );
        let host = Fingerprints::host();
        if host.matches_for(&AMD_ZEN4, Tier::Sse2) {
            eprintln!("host rcpps/rsqrtps match the oracle host (AMD Zen 4)");
        } else {
            eprintln!("host rcpps/rsqrtps differ from the oracle host: {host:x?}");
        }
    }
}
