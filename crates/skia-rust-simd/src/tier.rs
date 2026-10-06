/// A CPU code-path tier, mirroring the tiers Skia selects between in `SkOpts`
/// and `SkRasterPipeline`. Each tier must reproduce the exact output of the
/// corresponding Skia tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Tier {
    /// Scalar reference implementation; the twin every SIMD kernel is tested against.
    Portable,
    /// x86-64 baseline.
    Sse2,
    /// x86-64 with SSE4.1.
    Sse41,
    /// x86-64 with AVX.
    Avx,
    /// x86-64 Haswell: AVX2, FMA, F16C (Skia's `SK_CPU_SSE_LEVEL_AVX2` / "hsw").
    Hsw,
    /// x86-64 Skylake-X: AVX-512 F/DQ/CD/BW/VL (Skia's "skx").
    Skx,
    /// `AArch64` NEON.
    Neon,
    /// wasm32 with the `simd128` proposal (Skia built with Emscripten, CanvasKit-style).
    WasmSimd128,
}
