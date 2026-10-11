# gm/runtimeimagefilter.cpp::rtif_distort

Status: `failing` (one diagnostic pass with diffs; not resolved).

## What was ported

- `SkImageFilters::RuntimeShader` (`SkRuntimeImageFilter.cpp`) as
  `crates/skia-rust-effects/src/image_filters/runtime_image_filter.rs`, including
  `onGetCTMCapability` (translate only), `onAffectsTransparentBlack`, the sample-radius
  outset, input and output layer bounds, and the fast bounds.
- The GM `rtif_distort` in `tests/gm/src/gm/runtimeimagefilter.rs` (SkSL distortion
  `coord.x += sin(coord.y / 3) * 4`, sample radius 4, 25 random text runs per layer).

## Result

`gm-verify --match rtif_distort`, every checkable tier, configs 565 and f16 mismatch (8888 on
5 tiers); `rtif_unsharp` (the two-child runtime filter with a decal blur) passes everywhere.

Diagnostic pass on `cpu-x64-scalar/565` (`target/gm-diffs/...`, our raw dump vs the golden raw
dump, RGB565):

- 1580 of 375000 pixels differ; the largest 8-bit channel delta is 66.
- All differing pixels are in rows 500 to 749, i.e. the bottom two layers: the skew cell
  (`Skew(-0.5, 0)`, x 0..250) and the perspective cell (`setPerspX/Y`, x 250..500).
- The translate, scale and 45-degree rotate cells (rows 0 to 499) match exactly, so the
  SkSL `sin` distortion, the random text and the child shader wiring are right where the
  matrix is affine-with-no-skew-and-no-perspective.

## Hypothesis (not verified)

The difference is in how the filter maps its sample radius and bounds for skew and perspective
CTMs, under `onGetCTMCapability() == kTranslate` with `evaluateInParameterSpace = true`
(`Mapping::param_to_layer_size` and the layer bounds of the child). Check
`SkImageFilter_Base`'s `getCTMCapability` handling and `Mapping::decomposeCTM` in
`src/core/SkImageFilterTypes.cpp` against `skia-rust`'s `image_filter_types.rs`. A second
candidate is `sin` precision (glibc libm vs the oracle's UCRT), but that would show up in the
translate cell too, and it does not, so it is less likely.

Next step: dump the bounds (`ctx.mapping()`, the layer bounds of each child, the
`explicit_output`) for the skew cell and compare with the C++ `SkImageFilterTypes` values.
