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
