// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkOpts.cpp, src/opts/SkRasterPipeline_opts.h (tier choice)

//! CPU tiers: which of Skia's code paths runs, and how we execute it.
//!
//! See `docs/design/raster-pipeline.md` §2.2 (the tiers), §2.3 (dispatch) and §2.8 (models).

use std::fmt;
use std::sync::OnceLock;

use crate::cpu::{
    CpuCap, CpuFeatures, Ml3Token, Ml4Token, NeonToken, Sse2Token, Sse41Token, X64Level,
};

/// A CPU code path whose output must match one behaviour of real Skia bit for bit.
///
/// Each variant is one of Skia's `SKRP_CPU_*` code paths in `src/opts/SkRasterPipeline_opts.h`
/// (and the matching `SkOpts` kernels). Several oracle tiers collapse into one variant; see
/// `docs/design/raster-pipeline.md` §1 for the evidence and §2.2 for the mapping.
///
/// | Variant | Skia | Oracle tiers (m156) |
/// |---|---|---|
/// | [`Scalar`](Tier::Scalar) | `SKRP_CPU_SCALAR` | `wasm-simd128` (planned) |
/// | [`Sse2`](Tier::Sse2) | `SKRP_CPU_SSE2` | `cpu-x64-sse2`, `cpu-x64-ssse3`, `cpu-x64-sse2-rt-ssse3` |
/// | [`Sse41`](Tier::Sse41) | `SKRP_CPU_SSE41`, `SKRP_CPU_AVX` | `cpu-x64-sse41`, `cpu-x64-sse42`, `cpu-x64-avx` |
/// | [`Ml3`](Tier::Ml3) | `SKRP_CPU_AVX2` | every `*-rt-ml3` tier, `cpu-x64-v3` |
/// | [`Ml4`](Tier::Ml4) | `SKRP_CPU_ML4` | every `*-rt-ml4` tier, `cpu-x64-v4` |
/// | [`Neon`](Tier::Neon) | `SKRP_CPU_NEON` + `SK_CPU_ARM64` | `arm64-neon` (planned) |
///
/// The order is not a capability order across architectures; within x86-64 it is
/// `Sse2 < Sse41 < Ml3 < Ml4`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Tier {
    /// Skia's portable scalar path (`SKRP_CPU_SCALAR`): one lane in highp, no lowp pipeline.
    ///
    /// This is what Skia runs on wasm32: Emscripten defines neither an x86 level nor NEON, so
    /// `SkRasterPipeline_opts.h` falls through to `SKRP_CPU_SCALAR` whether or not `-msimd128` is
    /// passed. It is also the reference executed under Miri.
    Scalar,
    /// x86-64 SSE2 baseline (`SKRP_CPU_SSE2`): 4 highp lanes, 8 lowp lanes, precise `rcp_fast`,
    /// emulated `floor`/`ceil`, truncating `pack`, software half floats.
    #[doc(alias = "sse2")]
    Sse2,
    /// x86-64 SSE4.1 (`SKRP_CPU_SSE41`; an AVX baseline compiles the same code): like
    /// [`Sse2`](Tier::Sse2) but with `rcpps`/`rsqrtps` estimates for `rcp_fast`/`rsqrt`,
    /// `roundps` floor/ceil and saturating `packusdw`.
    #[doc(alias = "sse41")]
    #[doc(alias = "avx")]
    Sse41,
    /// x86-64-v3 (`SKRP_CPU_AVX2`, Skia's runtime `ml3` kernels or a v3 build): 8 highp lanes,
    /// 16 lowp lanes, fused `mad`, F16C half floats, `vrcpps`/`vrsqrtps` estimates.
    #[doc(alias = "ml3")]
    #[doc(alias = "avx2")]
    #[doc(alias = "hsw")]
    Ml3,
    /// x86-64-v4 (`SKRP_CPU_ML4`, Skia's runtime `ml4` kernels or a v4 build): 16 highp lanes,
    /// 16 lowp lanes, fused `mad`, `vrcp14ps`/`vrsqrt14ps` estimates.
    #[doc(alias = "ml4")]
    #[doc(alias = "avx512")]
    #[doc(alias = "skx")]
    Ml4,
    /// `AArch64` NEON (`SKRP_CPU_NEON` with `SK_CPU_ARM64`): 4 highp lanes, 8 lowp lanes, fused
    /// `mad`, `frecpe`/`frsqrte` estimates, exact lowp `div255`.
    #[doc(alias = "neon")]
    Neon,
}

impl Tier {
    /// Every tier.
    pub const ALL: [Tier; 6] = [
        Tier::Scalar,
        Tier::Sse2,
        Tier::Sse41,
        Tier::Ml3,
        Tier::Ml4,
        Tier::Neon,
    ];

    /// The tier's short lowercase name (`"scalar"`, `"sse2"`, `"sse41"`, `"ml3"`, `"ml4"`,
    /// `"neon"`), as used in logs and command lines.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Tier::Scalar => "scalar",
            Tier::Sse2 => "sse2",
            Tier::Sse41 => "sse41",
            Tier::Ml3 => "ml3",
            Tier::Ml4 => "ml4",
            Tier::Neon => "neon",
        }
    }

    /// Parses a name returned by [`Tier::name`].
    #[must_use]
    pub fn from_name(name: &str) -> Option<Tier> {
        Tier::ALL.into_iter().find(|t| t.name() == name)
    }

    /// What real Skia, built like this crate, runs on this host.
    ///
    /// The compile-time baseline picks the `SKRP_CPU_*` path (`cfg!(target_feature = "sse4.1")`
    /// or `"avx"` → [`Sse41`](Tier::Sse41), else [`Sse2`](Tier::Sse2); an AVX2 or AVX-512
    /// baseline → [`Ml3`](Tier::Ml3) / [`Ml4`](Tier::Ml4); `aarch64` → [`Neon`](Tier::Neon);
    /// anything else, including wasm32 → [`Scalar`](Tier::Scalar)). At run time it is upgraded
    /// to `Ml3`/`Ml4` exactly like `SkOpts::Init`, using Skia's own `cpuid` decoding (never to
    /// `Sse41`: Skia has no runtime SSE4.1 dispatch).
    ///
    /// In the pathological case where Skia's decoding reports a level that `std`'s detection
    /// does not confirm for every feature of the tier's `#[target_feature]` list, the result
    /// steps down (ml4 → ml3 → the compile-time path), so it is always
    /// [`is_native`](Tier::is_native).
    ///
    /// Computed once per process.
    #[must_use]
    pub fn detect() -> Tier {
        static DETECTED: OnceLock<Tier> = OnceLock::new();
        *DETECTED.get_or_init(|| Tier::detect_with_cap(CpuCap::Ml4))
    }

    /// [`Tier::detect`] with runtime features capped like the oracle's `SKIA_ORACLE_CPU_CAP`
    /// (`oracle/patches/skia-oracle.patch`). [`CpuCap::Ml4`] is no cap. Not cached.
    #[must_use]
    pub fn detect_with_cap(cap: CpuCap) -> Tier {
        let Some(baseline) = X64Level::compiled() else {
            return if cfg!(target_arch = "aarch64") && Tier::Neon.is_native() {
                Tier::Neon
            } else {
                Tier::Scalar
            };
        };
        let features = CpuFeatures::read();
        // Skia's choice, then (only if `std` disagrees with Skia's decoding) the same choice
        // with fewer runtime features, down to the compile-time path.
        [cap, cap.min(CpuCap::Ml3), CpuCap::Baseline]
            .into_iter()
            .map(|c| select_x64(baseline, c.apply(features)))
            .find(|t| t.is_native())
            .unwrap_or(Tier::Scalar)
    }

    /// Whether this host can execute the tier's real instructions ([`Backend::Native`]).
    ///
    /// [`Scalar`](Tier::Scalar) is native everywhere. The x86 tiers require every feature of
    /// the tier's `#[target_feature]` list (§2.4 of the design) to be detected at run time.
    #[must_use]
    pub fn is_native(self) -> bool {
        match self {
            Tier::Scalar => true,
            Tier::Sse2 => Sse2Token::get().is_some(),
            Tier::Sse41 => Sse41Token::get().is_some(),
            Tier::Ml3 => Ml3Token::get().is_some(),
            Tier::Ml4 => Ml4Token::get().is_some(),
            Tier::Neon => NeonToken::get().is_some(),
        }
    }

    /// Whether the tier is one of the x86-64 paths.
    #[must_use]
    pub const fn is_x86(self) -> bool {
        matches!(self, Tier::Sse2 | Tier::Sse41 | Tier::Ml3 | Tier::Ml4)
    }

    /// Highp lane count `N` (`sizeof(F)/sizeof(float)`).
    // Port of: src/opts/SkRasterPipeline_opts.h#L1748 (chrome/m156)
    #[must_use]
    pub const fn highp_stride(self) -> usize {
        match self {
            Tier::Scalar => 1,
            Tier::Sse2 | Tier::Sse41 | Tier::Neon => 4,
            Tier::Ml3 => 8,
            Tier::Ml4 => 16,
        }
    }

    /// Lowp lane count (`sizeof(U16)/2`); `None` on `Scalar`, which has no lowp pipeline.
    // Port of: src/opts/SkRasterPipeline_opts.h#L5465-L5499 (chrome/m156)
    #[must_use]
    pub const fn lowp_stride(self) -> Option<usize> {
        match self {
            Tier::Scalar => None,
            Tier::Sse2 | Tier::Sse41 | Tier::Neon => Some(8),
            Tier::Ml3 | Tier::Ml4 => Some(16),
        }
    }

    /// The oracle tiers whose goldens this tier must match (`cargo xtask oracle tiers`).
    ///
    /// All entries are expected to have identical goldens (`xtask oracle check-classes`). The
    /// first is the canonical one: what a default (SSE2-baseline) Skia runs on a CPU with the
    /// tier's features, or the tier's own build where no runtime dispatch reaches it. `Scalar`
    /// and `Neon` oracles are planned (design §4.5): `Scalar` lists the real wasm oracle first
    /// and the `x64-scalar` proxy second; a harness uses the first one that has goldens.
    #[must_use]
    pub fn oracle_tiers(self) -> &'static [&'static str] {
        match self {
            Tier::Scalar => &["wasm-simd128", "cpu-x64-scalar"],
            Tier::Sse2 => &["cpu-x64-sse2", "cpu-x64-sse2-rt-ssse3", "cpu-x64-ssse3"],
            Tier::Sse41 => &["cpu-x64-sse41", "cpu-x64-sse42", "cpu-x64-avx"],
            Tier::Ml3 => &[
                "cpu-x64-sse2-rt-ml3",
                "cpu-x64-ssse3-rt-ml3",
                "cpu-x64-sse41-rt-ml3",
                "cpu-x64-sse42-rt-ml3",
                "cpu-x64-avx-rt-ml3",
                "cpu-x64-v3",
            ],
            Tier::Ml4 => &[
                "cpu-x64-sse2-rt-ml4",
                "cpu-x64-ssse3-rt-ml4",
                "cpu-x64-sse41-rt-ml4",
                "cpu-x64-sse42-rt-ml4",
                "cpu-x64-avx-rt-ml4",
                "cpu-x64-v3-rt-ml4",
                "cpu-x64-v4",
            ],
            Tier::Neon => &["arm64-neon"],
        }
    }

    /// The RGBA-order counterparts of [`Tier::oracle_tiers`]: oracle builds with
    /// `SK_R32_SHIFT=0`, whose `8888` results are RGBA bytes (for hosts whose N32 is RGBA:
    /// macOS, Linux). Only the class representatives are built, so each list has one entry
    /// (`Scalar`'s is the `x64-scalar` proxy) and `Neon` has none yet. Each has the behaviour
    /// of its BGRA class, bytes swizzled (`xtask oracle check-classes` holds them as classes
    /// of their own).
    #[must_use]
    pub fn oracle_tiers_rgba(self) -> &'static [&'static str] {
        match self {
            Tier::Scalar => &["cpu-x64-scalar-rgba"],
            Tier::Sse2 => &["cpu-x64-sse2-rgba"],
            Tier::Sse41 => &["cpu-x64-sse41-rgba"],
            Tier::Ml3 => &["cpu-x64-sse2-rgba-rt-ml3"],
            Tier::Ml4 => &["cpu-x64-sse2-rgba-rt-ml4"],
            Tier::Neon => &[],
        }
    }

    /// The tier whose behaviour an oracle tier records (inverse of [`Tier::oracle_tiers`] and
    /// [`Tier::oracle_tiers_rgba`]).
    #[must_use]
    pub fn for_oracle_tier(name: &str) -> Option<Tier> {
        Tier::ALL
            .into_iter()
            .find(|t| t.oracle_tiers().contains(&name) || t.oracle_tiers_rgba().contains(&name))
    }

    /// The estimates the tier's model uses on this host when asked for [`Estimates::Host`]
    /// would come from hardware; this says whether such hardware is present.
    fn host_has_estimates(self) -> bool {
        match self {
            // No estimate instructions are used.
            Tier::Scalar => true,
            // rcpps/rsqrtps are SSE1, part of the x86-64 baseline.
            Tier::Sse2 | Tier::Sse41 | Tier::Ml3 => Sse2Token::get().is_some(),
            // vrcp14ps/vrsqrt14ps.
            Tier::Ml4 => Ml4Token::get().is_some(),
            // FRECPE/FRSQRTE (NEON; not under Miri, like the x86 estimates).
            Tier::Neon => crate::estimates::arm::host_available(),
        }
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// `SkOpts::Init` on x86: the compile-time `SKRP_CPU_*` path for `baseline`, upgraded to `ml3`
/// or `ml4` when `SkCpu::Supports` reports `ML3`/`ML4` and the baseline is below them.
///
/// `features` are the runtime features ([`CpuFeatures::read`], possibly capped).
// Port of: src/opts/SkRasterPipeline_opts.h#L76-L100 (chrome/m156) (compile-time choice)
// Port of: src/core/SkOpts.cpp#L51-L72 (chrome/m156) (runtime upgrade)
#[must_use]
pub fn select_x64(baseline: X64Level, features: CpuFeatures) -> Tier {
    let mut tier = if baseline >= X64Level::Ml4 {
        Tier::Ml4
    } else if baseline >= X64Level::Avx2 {
        Tier::Ml3
    } else if baseline >= X64Level::Sse41 {
        // SKRP_CPU_AVX and SKRP_CPU_SSE41 share one code block.
        Tier::Sse41
    } else if baseline >= X64Level::Sse2 {
        Tier::Sse2
    } else {
        // An SSE1-only x86 build: SkRasterPipeline_opts.h ends in SKRP_CPU_SCALAR.
        Tier::Scalar
    };
    if baseline < X64Level::Avx2 && features.supports(baseline, CpuFeatures::ML3) {
        tier = Tier::Ml3;
    }
    // GN defines SK_ENABLE_AVX512_OPTS unconditionally (BUILD.gn#L131-L132).
    if baseline < X64Level::Ml4 && features.supports(baseline, CpuFeatures::ML4) {
        tier = Tier::Ml4;
    }
    tier
}

/// How a tier is executed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Backend {
    /// The tier's real instructions (requires [`Tier::is_native`]).
    Native,
    /// The tier's model twin (design §2.8): per-lane reference primitives with the tier's exact
    /// semantics, no target features. Runs on any host and under Miri (except
    /// [`Estimates::Host`]).
    Model(Estimates),
}

/// Source of the `rcp`/`rsqrt` estimates in a model tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Estimates {
    /// This host's own estimate instructions (`rcpps`/`rsqrtps`, `vrcp14ps`/`vrsqrt14ps` or
    /// `frecpe`/`frsqrte`), executed one lane at a time. Requires the host to have them.
    Host,
    /// The oracle host's tables (AMD Ryzen 7 7800X3D, Zen 4; fingerprints in
    /// [`crate::estimates::AMD_ZEN4`]). x86 tiers only. The lane functions are
    /// [`crate::estimates::amd_zen4`]`::{rcp, rsqrt}` (Sse2/Sse41/Ml3) and
    /// `::{rcp14, rsqrt14}` (Ml4), exact on every input.
    AmdZen4,
    // IntelCore: once measured (design §6, R1/R2).
    /// The Arm ARM's architectural `RecipEstimate`/`RecipSqrtEstimate`. `Neon` only.
    Arm,
}

/// A tier and how to execute it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Selection {
    pub tier: Tier,
    pub backend: Backend,
}

impl Selection {
    /// `tier`, executed natively.
    #[must_use]
    pub const fn native(tier: Tier) -> Selection {
        Selection {
            tier,
            backend: Backend::Native,
        }
    }

    /// `tier`, executed by its model with `estimates`.
    #[must_use]
    pub const fn model(tier: Tier, estimates: Estimates) -> Selection {
        Selection {
            tier,
            backend: Backend::Model(estimates),
        }
    }

    /// Checks that this host can execute the selection.
    ///
    /// - `Native` needs [`Tier::is_native`].
    /// - `Model(AmdZen4)` is valid for the x86 tiers, `Model(Arm)` for `Neon`, `Model(Host)` for
    ///   any tier whose estimate instructions this host has. `Scalar` uses no estimates, so every
    ///   `Model` is valid for it (and equivalent to `Native`).
    ///
    /// # Errors
    /// [`Unsupported`] naming the reason.
    pub fn check(self) -> Result<Selection, Unsupported> {
        let reason = match (self.tier, self.backend) {
            (t, Backend::Native) if !t.is_native() => {
                "the host cannot execute this tier's instructions"
            }
            (Tier::Scalar, _) | (_, Backend::Native) => return Ok(self),
            (t, Backend::Model(Estimates::Host)) if !t.host_has_estimates() => {
                "the host lacks this tier's estimate instructions"
            }
            (t, Backend::Model(Estimates::AmdZen4)) if !t.is_x86() => {
                "AmdZen4 estimates exist only for x86 tiers"
            }
            (t, Backend::Model(Estimates::Arm)) if t != Tier::Neon => {
                "Arm estimates exist only for the Neon tier"
            }
            (_, Backend::Model(_)) => return Ok(self),
        };
        Err(Unsupported {
            selection: self,
            reason,
        })
    }
}

impl fmt::Display for Selection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.backend {
            Backend::Native => write!(f, "{} (native)", self.tier),
            Backend::Model(e) => write!(f, "{} (model, {e:?} estimates)", self.tier),
        }
    }
}

/// A [`Selection`] this host cannot execute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unsupported {
    pub selection: Selection,
    pub reason: &'static str,
}

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unsupported selection {}: {}",
            self.selection, self.reason
        )
    }
}

impl std::error::Error for Unsupported {}

/// The selection to use now: the current thread's forced selection (`testing::force_tier`,
/// test builds only), else [`Tier::detect`] executed natively.
///
/// Read where Skia reads its `SkOpts` tables: when a pipeline is compiled and when a blitter
/// picks its kernels.
#[must_use]
pub fn selection() -> Selection {
    #[cfg(any(test, feature = "testing"))]
    if let Some(sel) = crate::testing::forced() {
        return sel;
    }
    Selection::native(Tier::detect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Zen 4 as Skia sees it.
    fn zen4() -> CpuFeatures {
        CpuFeatures::all() - CpuFeatures::AVX512PF - CpuFeatures::AVX512ER
    }

    /// The compile-time level of each oracle build (`oracle/tiers.toml`).
    fn build_level(build: &str) -> X64Level {
        match build {
            "x64-sse2" => X64Level::Sse2,
            "x64-ssse3" => X64Level::Ssse3,
            "x64-sse41" => X64Level::Sse41,
            "x64-sse42" => X64Level::Sse42,
            "x64-avx" => X64Level::Avx,
            "x64-v3" => X64Level::Avx2,
            "x64-v4" => X64Level::Ml4,
            _ => panic!("unknown build {build}"),
        }
    }

    /// Splits `cpu-<build>[-rt-<cap>]` into the build's level and the runtime cap.
    fn parse_oracle_tier(name: &str) -> (X64Level, CpuCap) {
        let rest = name.strip_prefix("cpu-").unwrap();
        if let Some((build, cap)) = rest.split_once("-rt-") {
            (build_level(build), CpuCap::from_name(cap).unwrap())
        } else {
            let level = build_level(rest);
            // A build's own tier runs at the runtime level its baseline implies.
            let cap = match level {
                X64Level::Sse2 => CpuCap::Baseline,
                X64Level::Avx2 => CpuCap::Ml3,
                X64Level::Ml4 => CpuCap::Ml4,
                _ => CpuCap::Ssse3,
            };
            (level, cap)
        }
    }

    #[test]
    fn every_x64_oracle_tier_selects_its_tier() {
        let mut n = 0;
        for tier in Tier::ALL.into_iter().filter(|t| t.is_x86()) {
            for name in tier.oracle_tiers() {
                let (level, cap) = parse_oracle_tier(name);
                assert_eq!(select_x64(level, cap.apply(zen4())), tier, "{name}");
                assert_eq!(Tier::for_oracle_tier(name), Some(tier));
                n += 1;
            }
        }
        // The 19 x64 oracle tiers at m156.
        assert_eq!(n, 19);
    }

    #[test]
    fn rgba_oracle_tiers_select_their_tier() {
        let mut n = 0;
        for tier in Tier::ALL.into_iter().filter(|t| t.is_x86()) {
            for name in tier.oracle_tiers_rgba() {
                // `cpu-x64-sse2-rgba-rt-ml3` behaves as `cpu-x64-sse2-rt-ml3`.
                let bgra = name.replacen("-rgba", "", 1);
                let (level, cap) = parse_oracle_tier(&bgra);
                assert_eq!(select_x64(level, cap.apply(zen4())), tier, "{name}");
                assert_eq!(Tier::for_oracle_tier(name), Some(tier));
                n += 1;
            }
        }
        assert_eq!(n, 4);
        assert_eq!(
            Tier::for_oracle_tier("cpu-x64-scalar-rgba"),
            Some(Tier::Scalar)
        );
    }

    #[test]
    fn select_x64_mirrors_skopts_init() {
        let none = CpuFeatures::empty();
        assert_eq!(select_x64(X64Level::Sse2, none), Tier::Sse2);
        assert_eq!(select_x64(X64Level::Ssse3, none), Tier::Sse2);
        assert_eq!(select_x64(X64Level::Sse41, none), Tier::Sse41);
        assert_eq!(select_x64(X64Level::Avx, none), Tier::Sse41);
        assert_eq!(select_x64(X64Level::Avx2, none), Tier::Ml3);
        assert_eq!(select_x64(X64Level::Ml4, none), Tier::Ml4);
        assert_eq!(select_x64(X64Level::Sse1, none), Tier::Scalar);
        // SSE4.1 at run time does not switch an SSE2 build to Sse41.
        let sse42 = CpuFeatures::SSE1
            | CpuFeatures::SSE2
            | CpuFeatures::SSE3
            | CpuFeatures::SSSE3
            | CpuFeatures::SSE41
            | CpuFeatures::SSE42
            | CpuFeatures::AVX;
        assert_eq!(select_x64(X64Level::Sse2, sse42), Tier::Sse2);
        // ML3 needs all five bits.
        let almost_ml3 = sse42 | (CpuFeatures::ML3 - CpuFeatures::BMI2);
        assert_eq!(select_x64(X64Level::Sse2, almost_ml3), Tier::Sse2);
        assert_eq!(
            select_x64(X64Level::Sse2, almost_ml3 | CpuFeatures::BMI2),
            Tier::Ml3
        );
        // AVX-512 on a pre-Ice-Lake Intel is never reported by Skia's decoder, so a Skylake-X
        // looks like an ML3 machine; with ML4 bits present Ml4 wins.
        assert_eq!(select_x64(X64Level::Sse2, zen4()), Tier::Ml4);
        assert_eq!(select_x64(X64Level::Avx2, zen4()), Tier::Ml4);
    }

    #[test]
    fn strides() {
        let highp: Vec<usize> = Tier::ALL.iter().map(|t| t.highp_stride()).collect();
        assert_eq!(highp, [1, 4, 4, 8, 16, 4]);
        let lowp: Vec<Option<usize>> = Tier::ALL.iter().map(|t| t.lowp_stride()).collect();
        assert_eq!(lowp, [None, Some(8), Some(8), Some(16), Some(16), Some(8)]);
    }

    #[test]
    fn names_round_trip() {
        for t in Tier::ALL {
            assert_eq!(Tier::from_name(t.name()), Some(t));
            assert_eq!(t.to_string(), t.name());
        }
        assert_eq!(Tier::from_name("avx2"), None);
    }

    #[test]
    fn oracle_tiers_are_disjoint() {
        let mut seen = std::collections::HashSet::new();
        for t in Tier::ALL {
            assert_ne!(t.oracle_tiers(), &[] as &[&str]);
            for name in t.oracle_tiers().iter().chain(t.oracle_tiers_rgba()) {
                assert!(seen.insert(*name), "{name} listed twice");
            }
        }
    }

    #[test]
    fn detect_is_native_and_matches_skia_choice() {
        let t = Tier::detect();
        assert!(t.is_native(), "{t}");
        assert_eq!(Tier::detect_with_cap(CpuCap::Ml4), t);
        if cfg!(target_arch = "aarch64") {
            assert_eq!(t, Tier::Neon);
        } else if let Some(level) = X64Level::compiled() {
            let skia = select_x64(level, CpuFeatures::read());
            if skia.is_native() {
                assert_eq!(t, skia);
            } else {
                // Skia's decoding claims more than std confirms; we stepped down.
                assert!(t < skia, "{t} vs {skia}");
            }
            // The baseline cap gives the compile-time path (Sse2 for the default target).
            let base = Tier::detect_with_cap(CpuCap::Baseline);
            assert_eq!(base, select_x64(level, CpuFeatures::empty()));
            if level == X64Level::Sse2 {
                assert_eq!(base, Tier::Sse2);
                assert_eq!(Tier::detect_with_cap(CpuCap::Ssse3), Tier::Sse2);
            }
        } else {
            assert_eq!(t, Tier::Scalar);
        }
    }

    #[test]
    fn check_selections() {
        assert!(Selection::native(Tier::Scalar).check().is_ok());
        for e in [Estimates::Host, Estimates::AmdZen4, Estimates::Arm] {
            assert!(Selection::model(Tier::Scalar, e).check().is_ok());
        }
        for t in Tier::ALL {
            assert_eq!(Selection::native(t).check().is_ok(), t.is_native(), "{t}");
            assert_eq!(
                Selection::model(t, Estimates::AmdZen4).check().is_ok(),
                t.is_x86() || t == Tier::Scalar,
                "{t}"
            );
            assert_eq!(
                Selection::model(t, Estimates::Arm).check().is_ok(),
                matches!(t, Tier::Neon | Tier::Scalar),
                "{t}"
            );
        }
        let err = Selection::model(Tier::Neon, Estimates::AmdZen4)
            .check()
            .unwrap_err();
        assert!(err.to_string().contains("x86"), "{err}");
        #[cfg(target_arch = "x86_64")]
        {
            assert!(Selection::model(Tier::Ml3, Estimates::Host).check().is_ok());
            assert!(
                Selection::model(Tier::Neon, Estimates::Host)
                    .check()
                    .is_err()
            );
            assert_eq!(
                Selection::model(Tier::Ml4, Estimates::Host).check().is_ok(),
                Tier::Ml4.is_native()
            );
        }
    }

    #[test]
    fn default_selection_is_detect_native() {
        assert_eq!(selection(), Selection::native(Tier::detect()));
    }
}
