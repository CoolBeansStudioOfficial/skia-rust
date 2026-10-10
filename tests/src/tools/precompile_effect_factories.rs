// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tools/graphite/precompile/PrecompileEffectFactories.cpp, .h (chrome/m156)

//! Runtime effects, and the `Precompile` runtime-effect objects that describe them, that the
//! precompile tests share. Each effect is made once, on first use, and kept for the life of the
//! process (Skia keeps them in function-local statics).

use std::sync::OnceLock;

use skia_rust_core::blender::Blender;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::data::Data;
use skia_rust_core::runtime_effect::{ChildPtr, Options, RuntimeEffect};
use skia_rust_core::shader::Shader;
use skia_rust_gpu::graphite::precompile::blender::PrecompileBlender;
use skia_rust_gpu::graphite::precompile::color_filter::PrecompileColorFilter;
use skia_rust_gpu::graphite::precompile::runtime_effect::{
    PrecompileBaseHandle, PrecompileRuntimeEffects,
};
use skia_rust_gpu::graphite::precompile::shader::PrecompileShader;

// Port of: tools/graphite/precompile/PrecompileEffectFactories.cpp#L14-L28 (chrome/m156)
/// The annulus shader's SkSL.
#[must_use]
pub fn get_annulus_shader_code() -> &'static str {
    // draw a annulus centered at "center" w/ inner and outer radii in "radii"
    "uniform float2 center;\
     uniform float2 radii;\
     half4 main(float2 xy) {\
         float len = length(xy - center);\
         half value = len < radii.x ? 0.0 : (len > radii.y ? 0.0 : 1.0);\
         return half4(value);\
     }"
}

/// `SkRuntimeEffect::Options` with `fName` set to `name` (the other fields at their defaults).
fn named(name: &str) -> Options<'_> {
    let mut options = Options::default();
    options.name = name;
    options
}

/// The `float` uniforms as the bytes `SkData::MakeWithCopy` would copy.
fn float_uniforms(values: &[f32]) -> Data {
    let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_ne_bytes()).collect();
    Data::new_copy(&bytes)
}

// Port of: tools/graphite/precompile/PrecompileEffectFactories.cpp#L30-L43 (chrome/m156)
/// The annulus shader's effect. Skia makes it once, into a static.
#[must_use]
pub fn get_annulus_shader_effect() -> RuntimeEffect {
    static EFFECT: OnceLock<RuntimeEffect> = OnceLock::new();
    EFFECT
        .get_or_init(|| {
            RuntimeEffect::make_for_shader(get_annulus_shader_code(), Some(&named("AnnulusShader")))
                .expect("the annulus shader compiles")
        })
        .clone()
}

// Port of: tools/graphite/precompile/PrecompileEffectFactories.cpp#L45-L58 (chrome/m156)
/// The annulus shader, made with the fixed uniforms, and its precompile description.
#[must_use]
pub fn create_annulus_runtime_shader() -> (Shader, PrecompileShader) {
    let effect = get_annulus_shader_effect();
    let uniforms = float_uniforms(&[50.0, 50.0, 40.0, 50.0]);
    let s = effect
        .make_shader(uniforms, &[], None)
        .expect("the annulus shader makes a shader");
    let o = PrecompileRuntimeEffects::make_precompile_shader(effect, &[])
        .expect("the annulus shader makes a precompile shader");
    (s, o)
}

/// Makes a blender effect with a `fName` (`SkRuntimeEffect::MakeForBlender` into a static).
fn make_blender_effect(
    static_effect: &'static OnceLock<RuntimeEffect>,
    name: &str,
    sksl: &str,
) -> RuntimeEffect {
    static_effect
        .get_or_init(|| {
            RuntimeEffect::make_for_blender(sksl, Some(&named(name))).expect("the blender compiles")
        })
        .clone()
}

// Port of: tools/graphite/precompile/PrecompileEffectFactories.cpp#L60-L73 (chrome/m156)
#[must_use]
pub fn get_src_blender_effect() -> RuntimeEffect {
    static EFFECT: OnceLock<RuntimeEffect> = OnceLock::new();
    make_blender_effect(
        &EFFECT,
        "SrcBlender",
        "half4 main(half4 src, half4 dst) {return src;}",
    )
}

// Port of: tools/graphite/precompile/PrecompileEffectFactories.cpp#L75-L88 (chrome/m156)
/// The source blender's blender, and its precompile description.
#[must_use]
pub fn create_src_runtime_blender() -> (Blender, PrecompileBlender) {
    let effect = get_src_blender_effect();
    let b = effect
        .make_blender(Data::new_empty(), &[])
        .expect("the source blender makes a blender");
    let o = PrecompileRuntimeEffects::make_precompile_blender(effect, &[])
        .expect("the source blender makes a precompile blender");
    (b, o)
}

// Port of: tools/graphite/precompile/PrecompileEffectFactories.cpp#L90-L103 (chrome/m156)
#[must_use]
pub fn get_dst_blender_effect() -> RuntimeEffect {
    static EFFECT: OnceLock<RuntimeEffect> = OnceLock::new();
    make_blender_effect(
        &EFFECT,
        "DstBlender",
        "half4 main(half4 src, half4 dst) {return dst;}",
    )
}

// Port of: tools/graphite/precompile/PrecompileEffectFactories.cpp#L105-L118 (chrome/m156)
/// The destination blender's blender, and its precompile description.
#[must_use]
pub fn create_dst_runtime_blender() -> (Blender, PrecompileBlender) {
    let effect = get_dst_blender_effect();
    let b = effect
        .make_blender(Data::new_empty(), &[])
        .expect("the destination blender makes a blender");
    let o = PrecompileRuntimeEffects::make_precompile_blender(effect, &[])
        .expect("the destination blender makes a precompile blender");
    (b, o)
}

// Port of: tools/graphite/precompile/PrecompileEffectFactories.cpp#L120-L137 (chrome/m156)
#[must_use]
pub fn get_combo_blender_effect() -> RuntimeEffect {
    static EFFECT: OnceLock<RuntimeEffect> = OnceLock::new();
    make_blender_effect(
        &EFFECT,
        "ComboBlender",
        "uniform float blendFrac;\
         uniform blender a;\
         uniform blender b;\
         half4 main(half4 src, half4 dst) {\
             return (blendFrac * a.eval(src, dst)) + ((1 - blendFrac) * b.eval(src, dst));\
         }",
    )
}

// Port of: tools/graphite/precompile/PrecompileEffectFactories.cpp#L139-L157 (chrome/m156)
/// The combination blender, with the source and destination blenders as its children.
#[must_use]
pub fn create_combo_runtime_blender() -> (Blender, PrecompileBlender) {
    let effect = get_combo_blender_effect();
    let (src, src_o) = create_src_runtime_blender();
    let (dst, dst_o) = create_dst_runtime_blender();
    let children = [ChildPtr::Blender(src), ChildPtr::Blender(dst)];
    let uniforms = float_uniforms(&[1.0]);
    let b = effect
        .make_blender(uniforms, &children)
        .expect("the combination blender makes a blender");
    let o = PrecompileRuntimeEffects::make_precompile_blender(
        effect,
        &[
            vec![Some(PrecompileBaseHandle::Blender(src_o))],
            vec![Some(PrecompileBaseHandle::Blender(dst_o))],
        ],
    )
    .expect("the combination blender makes a precompile blender");
    (b, o)
}

// Port of: tools/graphite/precompile/PrecompileEffectFactories.cpp#L160-L173 (chrome/m156)
#[must_use]
pub fn get_double_color_filter_effect() -> RuntimeEffect {
    static EFFECT: OnceLock<RuntimeEffect> = OnceLock::new();
    EFFECT
        .get_or_init(|| {
            RuntimeEffect::make_for_color_filter(
                "half4 main(half4 c) {return 2*c;}",
                Some(&named("DoubleColorFilter")),
            )
            .expect("the double color filter compiles")
        })
        .clone()
}

// Port of: tools/graphite/precompile/PrecompileEffectFactories.cpp#L175-L184 (chrome/m156)
/// The double color filter, and its precompile description.
#[must_use]
pub fn create_double_runtime_color_filter() -> (ColorFilter, PrecompileColorFilter) {
    let effect = get_double_color_filter_effect();
    let cf = effect
        .make_color_filter(Data::new_empty(), &[])
        .expect("the double color filter makes a color filter");
    let o = PrecompileRuntimeEffects::make_precompile_color_filter(effect, &[])
        .expect("the double color filter makes a precompile color filter");
    (cf, o)
}

// Port of: tools/graphite/precompile/PrecompileEffectFactories.cpp#L186-L199 (chrome/m156)
#[must_use]
pub fn get_half_color_filter_effect() -> RuntimeEffect {
    static EFFECT: OnceLock<RuntimeEffect> = OnceLock::new();
    // We withhold this name to test out the default name case
    EFFECT
        .get_or_init(|| {
            RuntimeEffect::make_for_color_filter(
                "half4 main(half4 c) {return 0.5*c;}",
                Some(&Options::default()),
            )
            .expect("the half color filter compiles")
        })
        .clone()
}

// Port of: tools/graphite/precompile/PrecompileEffectFactories.cpp#L201-L213 (chrome/m156)
/// The half color filter, and its precompile description.
#[must_use]
pub fn create_half_runtime_color_filter() -> (ColorFilter, PrecompileColorFilter) {
    let effect = get_half_color_filter_effect();
    let cf = effect
        .make_color_filter(Data::new_empty(), &[])
        .expect("the half color filter makes a color filter");
    let o = PrecompileRuntimeEffects::make_precompile_color_filter(effect, &[])
        .expect("the half color filter makes a precompile color filter");
    (cf, o)
}

// Port of: tools/graphite/precompile/PrecompileEffectFactories.cpp#L215-L231 (chrome/m156)
#[must_use]
pub fn get_combo_color_filter_effect() -> RuntimeEffect {
    static EFFECT: OnceLock<RuntimeEffect> = OnceLock::new();
    EFFECT
        .get_or_init(|| {
            RuntimeEffect::make_for_color_filter(
                "uniform float blendFrac;\
                 uniform colorFilter a;\
                 uniform colorFilter b;\
                 half4 main(half4 c) {\
                     return (blendFrac * a.eval(c)) + ((1 - blendFrac) * b.eval(c));\
                 }",
                Some(&named("ComboColorFilter")),
            )
            .expect("the combination color filter compiles")
        })
        .clone()
}

// Port of: tools/graphite/precompile/PrecompileEffectFactories.cpp#L233-L250 (chrome/m156)
/// The combination color filter, with the double and half color filters as its children.
#[must_use]
pub fn create_combo_runtime_color_filter() -> (ColorFilter, PrecompileColorFilter) {
    let effect = get_combo_color_filter_effect();
    let (src, src_o) = create_double_runtime_color_filter();
    let (dst, dst_o) = create_half_runtime_color_filter();
    let children = [ChildPtr::ColorFilter(src), ChildPtr::ColorFilter(dst)];
    let uniforms = float_uniforms(&[0.5]);
    let cf = effect
        .make_color_filter(uniforms, &children)
        .expect("the combination color filter makes a color filter");
    let o = PrecompileRuntimeEffects::make_precompile_color_filter(
        effect,
        &[
            vec![Some(PrecompileBaseHandle::ColorFilter(src_o))],
            vec![Some(PrecompileBaseHandle::ColorFilter(dst_o))],
        ],
    )
    .expect("the combination color filter makes a precompile color filter");
    (cf, o)
}
