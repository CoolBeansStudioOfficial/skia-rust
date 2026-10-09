# gm/crbug_1174354.cpp::crbug_1174354

Ported 1:1 in `tests/gm/src/gm/crbug_1174354.rs`. Registered and `#[ignore]`d. `8888` and `565`
match on every tier; `f16` mismatches on every tier.

## Root cause (confirmed)

The differing pixels are exactly the four 50x50 blur boxes (4 x 2,500 px); everything outside them
matches. Inside, ours shows the unblurred sweep gradient (e.g. `(0, 1.0, 0.0004)`) where the golden
has the blurred mix (e.g. `(0.0001, 0.825, 0.175)`): the backdrop `Blur` image filter produced no
output on the F16 layer.

`RasterBlurEngine::find_algorithm` returns `None` for any color type other than A8/8888, so
`FilterResult::Builder::blur` returns an empty result (see
`crates/skia-rust-raster/src/blur_engine.rs`). Skia uses `RasterShaderBlurAlgorithm`
(`SkShaderBlurAlgorithm`, `src/core/SkBlurEngine.cpp#L1274-L1282`, `#L1536-L1740`), a runtime-effect
shader blur, for those color types. That needs SkSL runtime effects, which are not landed (they live on
`origin/port/sksl-*`).

Blocked until SkSL runtime effects land and `SkShaderBlurAlgorithm` is ported; then drop the
`#[ignore]` and re-run `gm-verify`. Same cause as `gm/spritebitmap.cpp::SpriteBitmapGM`.
