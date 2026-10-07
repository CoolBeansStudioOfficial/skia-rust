// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkCpu.h, src/core/SkCpu.cpp, include/private/SkFeatures.h

//! CPU feature detection, ported from `SkCpu`.
//!
//! [`CpuFeatures::read`] is `read_cpu_features()`: it decodes `cpuid` exactly like Skia, including
//! Skia's own rules that differ from `std`'s detection (AVX-512 is only reported on AMD or on Intel
//! parts with VBMI2, and only when the OS saves the ZMM state). [`X64Level`] is
//! `SK_CPU_X64_LEVEL`, the compile-time baseline, and [`CpuFeatures::supports`] is
//! `SkCpu::Supports`, which ORs the baseline's features back in.
//!
//! [`CpuCap`] reproduces the oracle's `SKIA_ORACLE_CPU_CAP` (`oracle/patches/skia-oracle.patch`),
//! which hides runtime features above a level so one build can produce several runtime tiers.
//!
//! The feature *tokens* ([`Sse2Token`], [`Sse41Token`], [`Ml3Token`], [`Ml4Token`],
//! [`NeonToken`]) are a different question: whether this process may execute a tier's
//! `#[target_feature]` code. They are built from `std`'s detection of every feature in the
//! tier's `#[target_feature(enable = …)]` list and are the safety proof for calling tier entry
//! points (`docs/design/raster-pipeline.md` §2.3, §3.1).

use bitflags::bitflags;

bitflags! {
    /// Runtime CPU features as Skia's `SkCpu` reports them (`SkX64::*`).
    ///
    /// The bit values are Skia's, so a value can be compared with a C++ `SkCpu` dump.
    // Port of: src/core/SkCpu.h#L15-L44 (chrome/m156)
    #[doc(alias = "SkX64")]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
    pub struct CpuFeatures: u32 {
        const SSE1 = 1 << 0;
        const SSE2 = 1 << 1;
        const SSE3 = 1 << 2;
        const SSSE3 = 1 << 3;
        const SSE41 = 1 << 4;
        const SSE42 = 1 << 5;
        const AVX = 1 << 6;
        const F16C = 1 << 7;
        const FMA = 1 << 8;
        const AVX2 = 1 << 9;
        const BMI1 = 1 << 10;
        const BMI2 = 1 << 11;
        const AVX512F = 1 << 12;
        const AVX512DQ = 1 << 13;
        const AVX512IFMA = 1 << 14;
        const AVX512PF = 1 << 15;
        const AVX512ER = 1 << 16;
        const AVX512CD = 1 << 17;
        const AVX512BW = 1 << 18;
        const AVX512VL = 1 << 19;
        const ERMS = 1 << 20;

        /// `-march=x86-64-v3`: what Skia's `ml3` kernels need.
        const ML3 = Self::AVX2.bits() | Self::BMI1.bits() | Self::BMI2.bits()
            | Self::F16C.bits() | Self::FMA.bits();
        /// `-march=x86-64-v4`: what Skia's `ml4` kernels need.
        const ML4 = Self::AVX512F.bits() | Self::AVX512DQ.bits() | Self::AVX512CD.bits()
            | Self::AVX512BW.bits() | Self::AVX512VL.bits();
    }
}

/// `SK_CPU_X64_LEVEL`: the x86-64 baseline the code was compiled for.
///
/// Discriminants are Skia's `SK_CPU_X64_LEVEL_*` values.
// Port of: include/private/SkFeatures.h#L86-L94 (chrome/m156)
#[doc(alias = "SK_CPU_X64_LEVEL")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum X64Level {
    Sse1 = 10,
    Sse2 = 20,
    Sse3 = 30,
    Ssse3 = 31,
    Sse41 = 41,
    Sse42 = 42,
    Avx = 51,
    Avx2 = 52,
    Ml4 = 60,
}

impl X64Level {
    /// The level this crate is compiled for, from `cfg!(target_feature = …)` exactly as
    /// `SkFeatures.h` derives it from the compiler's `__AVX512F__`, `__AVX2__`, … macros.
    /// `None` on non-x86 targets.
    // Port of: include/private/SkFeatures.h#L108-L129 (chrome/m156)
    #[must_use]
    pub const fn compiled() -> Option<X64Level> {
        if !cfg!(any(target_arch = "x86", target_arch = "x86_64")) {
            return None;
        }
        Some(
            if cfg!(all(
                target_feature = "avx512f",
                target_feature = "avx512dq",
                target_feature = "avx512cd",
                target_feature = "avx512bw",
                target_feature = "avx512vl"
            )) {
                X64Level::Ml4
            } else if cfg!(target_feature = "avx2") {
                X64Level::Avx2
            } else if cfg!(target_feature = "avx") {
                X64Level::Avx
            } else if cfg!(target_feature = "sse4.2") {
                X64Level::Sse42
            } else if cfg!(target_feature = "sse4.1") {
                X64Level::Sse41
            } else if cfg!(target_feature = "ssse3") {
                X64Level::Ssse3
            } else if cfg!(target_feature = "sse3") {
                X64Level::Sse3
            } else if cfg!(target_feature = "sse2") {
                X64Level::Sse2
            } else {
                X64Level::Sse1
            },
        )
    }

    /// The features `SkCpu::Supports` ORs in because the baseline guarantees them.
    // Port of: src/core/SkCpu.h#L63-L91 (chrome/m156)
    #[must_use]
    pub const fn implied_features(self) -> CpuFeatures {
        let level = self as u32;
        let mut f = 0;
        if level >= X64Level::Sse1 as u32 {
            f |= CpuFeatures::SSE1.bits();
        }
        if level >= X64Level::Sse2 as u32 {
            f |= CpuFeatures::SSE2.bits();
        }
        if level >= X64Level::Sse3 as u32 {
            f |= CpuFeatures::SSE3.bits();
        }
        if level >= X64Level::Ssse3 as u32 {
            f |= CpuFeatures::SSSE3.bits();
        }
        if level >= X64Level::Sse41 as u32 {
            f |= CpuFeatures::SSE41.bits();
        }
        if level >= X64Level::Sse42 as u32 {
            f |= CpuFeatures::SSE42.bits();
        }
        if level >= X64Level::Avx as u32 {
            f |= CpuFeatures::AVX.bits();
        }
        // F16C goes here if we add SK_CPU_X64_LEVEL_F16C
        if level >= X64Level::Avx2 as u32 {
            f |= CpuFeatures::AVX2.bits();
        }
        if level >= X64Level::Ml4 as u32 {
            f |= CpuFeatures::ML4.bits();
        }
        // FMA doesn't fit neatly into this total ordering (Skia's comment).
        CpuFeatures::from_bits_retain(f)
    }
}

/// The oracle's `SKIA_ORACLE_CPU_CAP`: hides runtime CPU features above a level.
///
/// `Baseline` leaves only what the compile-time level guarantees (because
/// [`CpuFeatures::supports`] ORs those back in), exactly like the oracle patch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CpuCap {
    /// `baseline`: no runtime features.
    Baseline,
    /// `ssse3`: SSE1 through SSSE3 only.
    Ssse3,
    /// `ml3`: everything except AVX-512.
    Ml3,
    /// `ml4`: no cap.
    Ml4,
}

impl CpuCap {
    /// Every cap, in the oracle's increasing order (`LEVELS` in `xtask/src/oracle.rs`).
    pub const ALL: [CpuCap; 4] = [CpuCap::Baseline, CpuCap::Ssse3, CpuCap::Ml3, CpuCap::Ml4];

    /// The `SKIA_ORACLE_CPU_CAP` spelling.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            CpuCap::Baseline => "baseline",
            CpuCap::Ssse3 => "ssse3",
            CpuCap::Ml3 => "ml3",
            CpuCap::Ml4 => "ml4",
        }
    }

    /// Parses a `SKIA_ORACLE_CPU_CAP` value.
    #[must_use]
    pub fn from_name(name: &str) -> Option<CpuCap> {
        CpuCap::ALL.into_iter().find(|c| c.name() == name)
    }

    /// Applies the cap to runtime-detected features (`skia_oracle_cap_features`).
    // Port of: oracle/patches/skia-oracle.patch, skia_oracle_cap_features (SkCpu.cpp)
    #[must_use]
    pub fn apply(self, features: CpuFeatures) -> CpuFeatures {
        let above_ml3 = CpuFeatures::AVX512F
            | CpuFeatures::AVX512DQ
            | CpuFeatures::AVX512IFMA
            | CpuFeatures::AVX512PF
            | CpuFeatures::AVX512ER
            | CpuFeatures::AVX512CD
            | CpuFeatures::AVX512BW
            | CpuFeatures::AVX512VL;
        let up_to_ssse3 =
            CpuFeatures::SSE1 | CpuFeatures::SSE2 | CpuFeatures::SSE3 | CpuFeatures::SSSE3;
        match self {
            CpuCap::Baseline => CpuFeatures::empty(),
            CpuCap::Ssse3 => features & up_to_ssse3,
            CpuCap::Ml3 => features & !above_ml3,
            CpuCap::Ml4 => features,
        }
    }
}

impl CpuFeatures {
    /// `SkCpu::Supports(mask)` for code compiled at `baseline`: the runtime features plus the
    /// ones the baseline guarantees, tested as a whole mask.
    // Port of: src/core/SkCpu.h#L58-L114 (chrome/m156)
    #[must_use]
    pub fn supports(self, baseline: X64Level, mask: CpuFeatures) -> bool {
        let features = self | baseline.implied_features();
        // SK_CPU_LIMIT_{AVX,SSE41,SSE2} are not defined in GN builds.
        features.contains(mask)
    }

    /// `read_cpu_features()`: this CPU's features, decoded from `cpuid` exactly like Skia.
    ///
    /// Empty on non-x86 targets (Skia's `#else` branch) and under Miri, which cannot execute
    /// `cpuid`.
    #[must_use]
    pub fn read() -> CpuFeatures {
        read_cpu_features()
    }
}

/// The CPU vendor string from `cpuid` leaf 0 (`"AuthenticAMD"`, `"GenuineIntel"`, …).
///
/// `None` on non-x86 targets and under Miri.
#[must_use]
pub fn cpu_vendor() -> Option<String> {
    x86::vendor().map(|v| String::from_utf8_lossy(&v).into_owned())
}

/// The CPU brand string from `cpuid` leaves `0x8000_0002..=0x8000_0004`, trimmed.
///
/// `None` on non-x86 targets, under Miri, or if the CPU does not report one.
#[must_use]
pub fn cpu_brand() -> Option<String> {
    x86::brand()
}

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), not(miri)))]
fn read_cpu_features() -> CpuFeatures {
    x86::read_cpu_features()
}

#[cfg(not(all(any(target_arch = "x86", target_arch = "x86_64"), not(miri))))]
// Port of: src/core/SkCpu.cpp#L224-L228 (chrome/m156)
fn read_cpu_features() -> CpuFeatures {
    CpuFeatures::empty()
}

#[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), not(miri)))]
mod x86 {
    #[cfg(target_arch = "x86")]
    use core::arch::x86::{__cpuid, __cpuid_count, _xgetbv};
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::{__cpuid, __cpuid_count, _xgetbv};

    use super::CpuFeatures;

    // Port of: src/core/SkCpu.cpp#L39-L44 (chrome/m156)
    const VENDOR_LEAF: u32 = 0;
    const FMSF_LEAF: u32 = 1;
    const FLAGS_LEAF: u32 = 7;
    const FLAGS_SUBLEAF: u32 = 0;

    // Port of: src/core/SkCpu.cpp#L90-L124 (chrome/m156)
    // cpuid(1) -> EDX
    const K_SSE: u32 = 1 << 25;
    const K_SSE2: u32 = 1 << 26;
    // cpuid(1) -> ECX
    const K_SSE3: u32 = 1 << 0;
    const K_SSSE3: u32 = 1 << 9;
    const K_SSE41: u32 = 1 << 19;
    const K_SSE42: u32 = 1 << 20;
    const K_FMA: u32 = 1 << 12;
    const K_AVX: u32 = 1 << 28;
    const K_F16C: u32 = 1 << 29;
    const K_XSAVE: u32 = 1 << 26;
    const K_OSXSAVE: u32 = 1 << 27;
    // cpuid(7,0) -> EBX
    const K_BMI1: u32 = 1 << 3;
    const K_AVX2: u32 = 1 << 5;
    const K_BMI2: u32 = 1 << 8;
    const K_ERMS: u32 = 1 << 9;
    const K_AVX512F: u32 = 1 << 16;
    const K_AVX512DQ: u32 = 1 << 17;
    const K_AVX512IFMA: u32 = 1 << 21;
    const K_AVX512PF: u32 = 1 << 26;
    const K_AVX512ER: u32 = 1 << 27;
    const K_AVX512CD: u32 = 1 << 28;
    const K_AVX512BW: u32 = 1 << 30;
    const K_AVX512VL: u32 = 1 << 31;
    // cpuid(7,0) -> ECX
    const K_AVX512VBMI2: u32 = 1 << 6;
    // xgetbv(0) -> XCR0
    const K_XCR0_XMM_YMM_STATE: u64 = 0b0000_0110;
    const K_XCR0_ZMM_STATE: u64 = 0b1110_0000;

    /// `xgetbv(0)`. Only called after `cpuid(1).ECX` reported both XSAVE and OSXSAVE.
    fn xgetbv() -> u64 {
        // SAFETY: `_xgetbv` requires the `xsave` feature and an OS that has enabled it
        // (CR4.OSXSAVE). The only caller checks CPUID.01H:ECX.XSAVE[26] and OSXSAVE[27] first,
        // as Skia does (SkCpu.cpp#L164-L166); OSXSAVE=1 means XGETBV is executable.
        unsafe { _xgetbv(0) }
    }

    #[allow(clippy::unnecessary_wraps)] // same signature as the non-x86 twin, which returns None
    pub(super) fn vendor() -> Option<[u8; 12]> {
        let r = __cpuid(VENDOR_LEAF);
        let mut v = [0u8; 12];
        // The vendor string is in EBX, EDX, ECX (in that order).
        v[0..4].copy_from_slice(&r.ebx.to_le_bytes());
        v[4..8].copy_from_slice(&r.edx.to_le_bytes());
        v[8..12].copy_from_slice(&r.ecx.to_le_bytes());
        Some(v)
    }

    pub(super) fn brand() -> Option<String> {
        if __cpuid(0x8000_0000).eax < 0x8000_0004 {
            return None;
        }
        let mut bytes = Vec::with_capacity(48);
        for leaf in 0x8000_0002..=0x8000_0004 {
            let r = __cpuid(leaf);
            for reg in [r.eax, r.ebx, r.ecx, r.edx] {
                bytes.extend_from_slice(&reg.to_le_bytes());
            }
        }
        let s = String::from_utf8_lossy(&bytes);
        Some(
            s.trim_matches(|c: char| c == '\0' || c.is_whitespace())
                .to_owned(),
        )
    }

    // Port of: src/core/SkCpu.cpp#L126-L130 (chrome/m156)
    const fn ascii_le(s: [u8; 4]) -> u32 {
        u32::from_le_bytes(s)
    }

    // Port of: src/core/SkCpu.cpp#L132-L213 (chrome/m156)
    pub(super) fn read_cpu_features() -> CpuFeatures {
        let mut features = CpuFeatures::empty();

        let r = __cpuid(VENDOR_LEAF);
        // The vendor string in EBX, EDX, ECX (yes, in that order).
        let is_amd = r.ebx == ascii_le(*b"Auth")
            && r.edx == ascii_le(*b"enti")
            && r.ecx == ascii_le(*b"cAMD");

        let r = __cpuid(FMSF_LEAF);
        let (ecx, edx) = (r.ecx, r.edx);
        if edx & K_SSE != 0 {
            features |= CpuFeatures::SSE1;
        }
        if edx & K_SSE2 != 0 {
            features |= CpuFeatures::SSE2;
        }
        if ecx & K_SSE3 != 0 {
            features |= CpuFeatures::SSE3;
        }
        if ecx & K_SSSE3 != 0 {
            features |= CpuFeatures::SSSE3;
        }
        if ecx & K_SSE41 != 0 {
            features |= CpuFeatures::SSE41;
        }
        if ecx & K_SSE42 != 0 {
            features |= CpuFeatures::SSE42;
        }

        // Skia checks XSAVE (hardware) and OSXSAVE (OS support) before XGETBV.
        if (ecx & (K_XSAVE | K_OSXSAVE)) == (K_XSAVE | K_OSXSAVE) {
            let xcr = xgetbv();
            if (xcr & K_XCR0_XMM_YMM_STATE) == K_XCR0_XMM_YMM_STATE {
                if ecx & K_AVX != 0 {
                    features |= CpuFeatures::AVX;
                }
                if ecx & K_F16C != 0 {
                    features |= CpuFeatures::F16C;
                }
                if ecx & K_FMA != 0 {
                    features |= CpuFeatures::FMA;
                }

                let r = __cpuid_count(FLAGS_LEAF, FLAGS_SUBLEAF);
                let (ebx, ecx7) = (r.ebx, r.ecx);
                if ebx & K_AVX2 != 0 {
                    features |= CpuFeatures::AVX2;
                }
                if ebx & K_BMI1 != 0 {
                    features |= CpuFeatures::BMI1;
                }
                if ebx & K_BMI2 != 0 {
                    features |= CpuFeatures::BMI2;
                }
                if ebx & K_ERMS != 0 {
                    features |= CpuFeatures::ERMS;
                }

                if (xcr & K_XCR0_ZMM_STATE) == K_XCR0_ZMM_STATE {
                    // Skia only enables AVX-512 on AMD, or on Intel from Ice Lake on (VBMI2).
                    let is_newer_intel = ecx7 & K_AVX512VBMI2 != 0;
                    if is_amd || is_newer_intel {
                        for (bit, f) in [
                            (K_AVX512F, CpuFeatures::AVX512F),
                            (K_AVX512DQ, CpuFeatures::AVX512DQ),
                            (K_AVX512IFMA, CpuFeatures::AVX512IFMA),
                            (K_AVX512PF, CpuFeatures::AVX512PF),
                            (K_AVX512ER, CpuFeatures::AVX512ER),
                            (K_AVX512CD, CpuFeatures::AVX512CD),
                            (K_AVX512BW, CpuFeatures::AVX512BW),
                            (K_AVX512VL, CpuFeatures::AVX512VL),
                        ] {
                            if ebx & bit != 0 {
                                features |= f;
                            }
                        }
                    }
                }
            }
        }
        features
    }
}

#[cfg(not(all(any(target_arch = "x86", target_arch = "x86_64"), not(miri))))]
mod x86 {
    pub(super) fn vendor() -> Option<[u8; 12]> {
        None
    }
    pub(super) fn brand() -> Option<String> {
        None
    }
}

macro_rules! x86_token {
    ($(#[$m:meta])* $name:ident, $features:literal, [$($f:tt),*]) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $name(());

        impl $name {
            /// The `#[target_feature(enable = …)]` list this token vouches for.
            pub const FEATURES: &'static str = $features;

            /// Returns the token if every feature in [`Self::FEATURES`] was detected at run time.
            #[must_use]
            pub fn get() -> Option<Self> {
                #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
                {
                    static DETECTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
                    if *DETECTED.get_or_init(|| true $(&& std::arch::is_x86_feature_detected!($f))*) {
                        return Some($name(()));
                    }
                }
                None
            }
        }
    };
}

x86_token!(
    /// Proof that the host can run code compiled with Skia's `Sse2` features.
    ///
    /// SSE2 is part of the x86-64 baseline, so this always exists on x86-64.
    Sse2Token,
    "sse2",
    ["sse2"]
);
x86_token!(
    /// Proof that the host can run `#[target_feature(enable = "sse2,ssse3,sse4.1")]` code
    /// (the `Sse41` tier).
    Sse41Token,
    "sse2,ssse3,sse4.1",
    ["sse2", "ssse3", "sse4.1"]
);
x86_token!(
    /// Proof that the host can run the `Ml3` tier's features (`-march=x86-64-v3` as Skia builds
    /// `ml3`).
    Ml3Token,
    "sse2,ssse3,sse4.1,sse4.2,avx,avx2,bmi1,bmi2,f16c,fma",
    ["sse2", "ssse3", "sse4.1", "sse4.2", "avx", "avx2", "bmi1", "bmi2", "f16c", "fma"]
);
x86_token!(
    /// Proof that the host can run the `Ml4` tier's features (`-march=x86-64-v4` as Skia builds
    /// `ml4`).
    Ml4Token,
    "sse2,ssse3,sse4.1,sse4.2,avx,avx2,bmi1,bmi2,f16c,fma,avx512f,avx512dq,avx512cd,avx512bw,avx512vl",
    [
        "sse2", "ssse3", "sse4.1", "sse4.2", "avx", "avx2", "bmi1", "bmi2", "f16c", "fma",
        "avx512f", "avx512dq", "avx512cd", "avx512bw", "avx512vl"
    ]
);

/// Proof that the host can run `#[target_feature(enable = "neon")]` code (the `Neon` tier).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NeonToken(());

impl NeonToken {
    /// The `#[target_feature(enable = …)]` list this token vouches for.
    pub const FEATURES: &'static str = "neon";

    /// Returns the token on `AArch64` hosts with NEON (every `AArch64` target Rust supports).
    #[must_use]
    pub fn get() -> Option<Self> {
        #[cfg(target_arch = "aarch64")]
        {
            if std::arch::is_aarch64_feature_detected!("neon") {
                return Some(NeonToken(()));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_matches_oracle_patch() {
        let all = CpuFeatures::all();
        assert_eq!(CpuCap::Baseline.apply(all), CpuFeatures::empty());
        assert_eq!(
            CpuCap::Ssse3.apply(all),
            CpuFeatures::SSE1 | CpuFeatures::SSE2 | CpuFeatures::SSE3 | CpuFeatures::SSSE3
        );
        let ml3 = CpuCap::Ml3.apply(all);
        assert!(ml3.contains(CpuFeatures::ML3 | CpuFeatures::ERMS | CpuFeatures::SSE42));
        assert!(!ml3.intersects(CpuFeatures::ML4 | CpuFeatures::AVX512IFMA));
        assert_eq!(CpuCap::Ml4.apply(all), all);
        for c in CpuCap::ALL {
            assert_eq!(CpuCap::from_name(c.name()), Some(c));
        }
        assert_eq!(CpuCap::from_name("avx2"), None);
    }

    #[test]
    fn supports_ors_in_baseline() {
        let none = CpuFeatures::empty();
        assert!(none.supports(X64Level::Sse2, CpuFeatures::SSE1 | CpuFeatures::SSE2));
        assert!(!none.supports(X64Level::Sse2, CpuFeatures::SSE3));
        assert!(none.supports(X64Level::Avx, CpuFeatures::SSE42 | CpuFeatures::AVX));
        // The AVX2 baseline does not imply FMA/F16C/BMI (Skia's comment about FMA), so ML3
        // needs runtime bits even there.
        assert!(!none.supports(X64Level::Avx2, CpuFeatures::ML3));
        assert!(none.supports(X64Level::Ml4, CpuFeatures::ML4));
    }

    #[test]
    fn compiled_level_matches_cfg() {
        let level = X64Level::compiled();
        if cfg!(target_arch = "x86_64") {
            // x86-64 guarantees SSE2.
            assert!(level >= Some(X64Level::Sse2), "{level:?}");
            assert_eq!(
                level == Some(X64Level::Ml4),
                cfg!(target_feature = "avx512f")
            );
        } else if cfg!(target_arch = "x86") {
            assert!(level.is_some());
        } else {
            assert_eq!(level, None);
        }
    }

    #[test]
    fn read_is_consistent_with_std_detection() {
        let f = CpuFeatures::read();
        #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), not(miri)))]
        {
            use std::arch::is_x86_feature_detected as d;
            // Skia's decoding and std's agree on every bit both report the same way. (AVX-512
            // differs by design: Skia ignores it on pre-Ice-Lake Intel.)
            assert_eq!(f.contains(CpuFeatures::SSE2), d!("sse2"));
            assert_eq!(f.contains(CpuFeatures::SSE3), d!("sse3"));
            assert_eq!(f.contains(CpuFeatures::SSSE3), d!("ssse3"));
            assert_eq!(f.contains(CpuFeatures::SSE41), d!("sse4.1"));
            assert_eq!(f.contains(CpuFeatures::SSE42), d!("sse4.2"));
            assert_eq!(f.contains(CpuFeatures::AVX), d!("avx"));
            assert_eq!(f.contains(CpuFeatures::AVX2), d!("avx2"));
            assert_eq!(f.contains(CpuFeatures::FMA), d!("fma"));
            assert_eq!(f.contains(CpuFeatures::F16C), d!("f16c"));
            if f.contains(CpuFeatures::AVX512F) {
                assert!(d!("avx512f"));
            }
            assert!(cpu_vendor().is_some());
        }
        #[cfg(not(all(any(target_arch = "x86", target_arch = "x86_64"), not(miri))))]
        {
            assert_eq!(f, CpuFeatures::empty());
            assert_eq!(cpu_vendor(), None);
        }
    }

    #[test]
    fn tokens_follow_std_detection() {
        #[cfg(target_arch = "x86_64")]
        {
            assert!(Sse2Token::get().is_some());
            // Every higher token implies the lower ones.
            if Ml4Token::get().is_some() {
                assert!(Ml3Token::get().is_some());
            }
            if Ml3Token::get().is_some() {
                assert!(Sse41Token::get().is_some());
            }
            assert!(NeonToken::get().is_none());
        }
        #[cfg(target_arch = "aarch64")]
        {
            assert!(NeonToken::get().is_some());
            assert!(Sse2Token::get().is_none());
        }
    }
}
