// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skcms/skcms.cc (runtime CPU dispatch)

//! skcms's own CPU dispatch.
//!
//! skcms does not use `SkCpu`/`SkOpts`: it decodes `cpuid` itself and runs the transform program
//! with the `skcms_private::{baseline,hsw,skx}::run_program` kernels, all compiled from
//! `Transform_inl.h`. The kernels agree bit for bit except for the half-float conversions:
//! `F_from_Half`/`Half_from_F` are software (truncating, denormals flushed to zero) in the
//! baseline kernel on x86-64 and wasm, and F16C `vcvtph2ps`/`vcvtps2ph` (round to nearest even,
//! denormals kept) in the HSW and SKX kernels and, through `vcvt_f16_f32`, in the `AArch64` NEON
//! baseline. (`-ffp-contract=off` is passed for HSW/SKX in `BUILD.gn`, so FMA is not a
//! difference; every other op has the same lane-wise result.)
//!
//! The oracle's `SKIA_ORACLE_CPU_CAP` only caps `SkCpu`; skcms runs its own detection on the
//! oracle host (Zen 4: AVX-512), so **every x64 oracle tier ran the SKX kernel**, even
//! `cpu-x64-sse2` and `cpu-x64-scalar`. While a tier is forced (`skia_rust_simd::testing`), this
//! module therefore models that host on any machine. Otherwise it decodes the real host.

use std::sync::atomic::{AtomicBool, Ordering};

/// Which `run_program` kernel skcms picks.
// Port of: modules/skcms/skcms.cc#L2459 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CpuType {
    Baseline,
    Hsw,
    Skx,
}

// Port of: modules/skcms/skcms.cc#L44-L48 (chrome/m156)
static ALLOW_RUNTIME_CPU_DETECTION: AtomicBool = AtomicBool::new(true);

pub(crate) fn disable_runtime_cpu_detection() {
    ALLOW_RUNTIME_CPU_DETECTION.store(false, Ordering::Relaxed);
}

// Port of: modules/skcms/skcms.cc#L2461-L2518 (chrome/m156)
pub(crate) fn cpu_type() -> CpuType {
    if !ALLOW_RUNTIME_CPU_DETECTION.load(Ordering::Relaxed) {
        return CpuType::Baseline;
    }
    if skia_rust_simd::selection_is_forced() {
        // An oracle tier is being reproduced: the oracle host ran SKX for all of them.
        return CpuType::Skx;
    }
    host_cpu_type()
}

#[cfg(target_arch = "x86_64")]
fn host_cpu_type() -> CpuType {
    use std::sync::OnceLock;
    static TYPE: OnceLock<CpuType> = OnceLock::new();
    *TYPE.get_or_init(|| {
        // The cpuid bits skcms checks, as `std` names them.
        let hsw = is_x86_feature_detected!("sse4.2")
            && is_x86_feature_detected!("ssse3")
            && is_x86_feature_detected!("fma")
            && is_x86_feature_detected!("avx")
            && is_x86_feature_detected!("f16c")
            && is_x86_feature_detected!("avx2");
        let skx = hsw
            && is_x86_feature_detected!("avx512f")
            && is_x86_feature_detected!("avx512dq")
            && is_x86_feature_detected!("avx512cd")
            && is_x86_feature_detected!("avx512bw")
            && is_x86_feature_detected!("avx512vl");
        if skx {
            CpuType::Skx
        } else if hsw {
            CpuType::Hsw
        } else {
            CpuType::Baseline
        }
    })
}

#[cfg(not(target_arch = "x86_64"))]
fn host_cpu_type() -> CpuType {
    // `!defined(__x86_64__)`
    CpuType::Baseline
}

/// Whether the selected kernel converts half floats with F16C-style hardware instructions
/// (`USING_AVX_F16C`, `USING_AVX512F`, `USING_NEON_F16C`) instead of the software conversion.
pub(crate) fn hardware_half() -> bool {
    match cpu_type() {
        CpuType::Hsw | CpuType::Skx => true,
        CpuType::Baseline => cfg!(target_arch = "aarch64"),
    }
}
