# gm/spritebitmap.cpp::SpriteBitmapGM

Status: `failing` (ported, registered, `#[ignore]`d). `8888` and `565` match on every tier; `f16`
mismatches on every tier.

## Root cause (confirmed)

Not an N32-image-onto-F16 problem. The unfiltered draws (rows 1 and 3) are bit-identical to the
goldens in `f16`; every differing pixel (29,480) lies in the blurred draws (rows 2 and 4, plus the
blur halo above them). Ours leaves those draws without the blur.

`RasterBlurEngine::find_algorithm` (`crates/skia-rust-raster/src/blur_engine.rs`) only has the A8 and
8888 algorithms and returns `None` for every other color type, so `FilterResult::Builder::blur`
returns an empty result. Skia's `RasterBlurEngine::findAlgorithm` (`src/core/SkBlurEngine.cpp#L1284-L1302`)
returns `RasterShaderBlurAlgorithm` (`SkShaderBlurAlgorithm`, `#L1536-L1740`) for those: a 1D/2D
Gaussian drawn as a runtime-effect shader (`SkKnownRuntimeEffects` `k2DBlurBase`/`kLinearBlur1DBase`)
into an `SkBitmapDevice` of the input's color type. An F16 layer takes that path.

## What blocks it

`SkShaderBlurAlgorithm` needs `SkRuntimeEffect`/`SkRuntimeShaderBuilder`, `SkKnownRuntimeEffects` and
the SkSL -> raster pipeline code generator. None of that is on this branch's base (it lives on
`origin/port/sksl-*`/`port/sksl-all`, unmerged). Re-test this GM once SkSL runtime effects are
landed and the shader blur algorithm is ported (`SkShaderBlurAlgorithm::{blur, evalBlur1D, evalBlur2D,
renderBlur}`, `Compute{1D,2D}Blur*`, `GetLinearBlur1DEffect`, `GetBlur2DEffect`) into
`RasterBlurEngine`.

Same cause as `gm/crbug_1174354.cpp::crbug_1174354`. (`gm/postercircle.cpp::PosterCircleGM()` looked
related but is a different bug: host libm `sinf`/`cosf`, fixed by `skia_rust_core::libm`.)
