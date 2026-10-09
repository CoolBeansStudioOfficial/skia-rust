# Effects (Phase 3): shaders and friends

Where the pieces of `src/effects` and `src/shaders` live and how they differ from C++. The path
effects (dash, corner) are documented in `docs/design/raster-pipeline.md` ("As implemented in
C7"). This file grows with each effects task.

## As implemented: gradients (`skia-rust-effects`, `skia_rust_core::shaders`)

### Layout

| Skia | skia-rust |
|---|---|
| `include/effects/SkGradient.h` (`SkGradient`, `Colors`, `Interpolation`, `SkShaders::*Gradient`) | `skia_rust_effects::gradient` (`Gradient`, `Colors`, `Interpolation`, `interpolation::{InPremul, ColorSpace, HueMethod}`, `gradient::shaders::{linear_gradient, radial_gradient, two_point_conical_gradient, sweep_gradient}`), the API shape of `skia-safe`'s `gradient` module |
| the pre-`SkGradient` `SkGradientShader::Make*` (gone from m156) | `skia_rust_effects::gradient_shader::{linear, radial, two_point_conical, sweep, *_with_interpolation, Flags, GradientShaderColors}`: the free functions of `skia-safe`'s `gradient_shader`, written over the new API |
| `SkGradientBaseShader`, `SkColor4fXformer` | `gradient_base_shader::{GradientBaseShader, Color4fXformer, append_gradient_fill_stages, append_interpolated_to_dst_stages}` |
| `SkLinearGradient`, `SkRadialGradient`, `SkSweepGradient`, `SkConicalGradient` (+ `FocalData`) | `linear_gradient`, `radial_gradient`, `sweep_gradient`, `conical_gradient` |
| `SkShaderBase::GradientType/GradientInfo/asGradient`, `makeAsALocalMatrixShader` | `ShaderBase::as_gradient`, `shaders::{GradientType, GradientInfo}`, `ShaderBase::make_as_a_local_matrix_shader` (core) |
| `SkLocalMatrixShader`, `SkShader::makeWithLocalMatrix` | `shaders::LocalMatrixShader`, `Shader::with_local_matrix` (core) |

The shapes need `Shader`, which lives in `skia-rust-core`, so `skia-safe`'s deprecated
`Shader::linear_gradient(...)` associated functions cannot exist (an inherent impl cannot be in another
crate); use `gradient::shaders::*` or the `gradient_shader` free functions.

### Inheritance

`SkGradientBaseShader` has four subclasses that only add `appendGradientStages`. Here every
subclass holds a `GradientBaseShader` and implements `ShaderBase` itself;
`GradientBaseShader::append_stages(rec, m_rec, |alloc, p, post| ...)` takes the subclass's
`appendGradientStages` as a closure. `makeWithLocalMatrix` always wraps (even for the identity), as in
Skia, so the pipeline of a gradient built through the factories is the C++ pipeline (the local matrix is
applied by `MatrixRec`, not by an extra stage).

### Stages

All the gradient stages are ported in `skia_rust_simd::rp::tiers::{highp,lowp}::sampling`, the
two-point conical ones highp-only as in Skia: `evenly_spaced_2_stop_gradient`, `evenly_spaced_gradient`,
`gradient` (highp and lowp), `xy_to_radius` (both), `xy_to_unit_angle` (both), `negate_x`,
`xy_to_2pt_conical_{strip,focal_on_circle,well_behaved,smaller,greater}`,
`alter_2pt_conical_{compensate_focal,unswap}`, `mask_2pt_conical_{nan,degenerates}`, `apply_vector_mask`.
There are **no rp-diff cases** for them: the oracle host is gone, so they are checked by
`rp/tests_gradient.rs` (known answers on every selection: highp and lowp) and, together with the
shaders, by the GMs against Skia's goldens on every tier (the AVX2 `gradient_lookup` permute is the
gather, since the index is always below the stop count).

`GradientCtx` owns its factor, bias and `t` tables (`Vec<f32>`, at least `max(count + 1, 8)` entries)
instead of pointing into one arena block: that makes it `'static`, so it can live in `ArenaAlloc` like
the other contexts shaders allocate (`ArenaAlloc` cannot hold borrowing types). The tables' contents
are the C++ ones.

### Interpolation color spaces

`Color4fXformer::new` is `SkColor4fXformer` 1:1: convert to the intermediate space with
`convert_pixels` (RGBA F32, unpremul), then Lab/LCH/OKLab/OKLCH/HSL/HWB conversion, powerless-hue
stop splitting, hue method adjustment, premul. `std::cbrtf` and `atan2f` are `skia_rust_core::libm::{cbrtf, atan2f}` (UCRT-exact, `docs/design/math.md`).

### Not ported

Serialization (`flatten`, `unflatten`, the legacy flags), the GPU entry points
(`SkColor4fXformer`'s `forceExplicitPositions` parameter, `cachedBitmap`), `SkGradientShader`'s removed
legacy factories beyond the `skia-safe` wrappers, and the working-color-space shader.
