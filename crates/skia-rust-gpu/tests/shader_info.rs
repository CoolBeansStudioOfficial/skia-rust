// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Adapted from Skia: src/gpu/graphite/ShaderInfo.cpp

//! [`ShaderInfo`] for whole pipelines: labels, the blend state and the shape of the `SkSL`.
//!
//! Each expectation is derived from the control flow of `ShaderInfo.cpp` for that pipeline, not
//! read back from the port: which branch of `generateFragmentSkSL` the blend mode and coverage
//! take, and how `Make` builds the labels.

mod support;

use std::sync::Arc;

use skia_rust_core::color_type::ColorType;
use skia_rust_gpu::gpu::blend::{BlendCoeff, BlendEquation};
use skia_rust_gpu::graphite::context_options::ContextOptions;
use skia_rust_gpu::graphite::render_pass_desc::RenderPassDesc;
use skia_rust_gpu::graphite::render_step::RenderStep;
use skia_rust_gpu::graphite::resource_types::Layout;
use skia_rust_gpu::graphite::shader_code_dictionary::ShaderCodeDictionary;
use skia_rust_gpu::graphite::texture_format::{TextureFormat, write_swizzle_for_color_type};
use skia_rust_gpu::graphite::unique_paint_params_id::UniquePaintParamsID;
use skia_rust_gpu::graphite::wgpu::pipeline_shaders::make_pipeline_shaders;
use skia_rust_gpu::graphite::wgpu::{CapsProfile, WgpuCaps};
use support::wgsl_corpus::{Recorded, all_steps, corpus_paints, pipelines, renderer_provider};

fn step(name: &str, caps: &WgpuCaps) -> Arc<dyn RenderStep> {
    all_steps(&renderer_provider(caps))
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no step {name}"))
        .1
}

#[test]
fn a_solid_paint_with_analytic_coverage_uses_the_hardware_blend_formula() {
    // SrcOver with coverage does not need a dual-source blend formula, so the coverage is
    // multiplied into the color (`kModulate`) and the hardware blends One / ISA... for Plus, whose
    // formula is One / One.
    let profile = CapsProfile::dawn_d3d12();
    let caps = WgpuCaps::new(&profile, &ContextOptions::default());
    let steps = vec![(
        "analytic_rrect[0]".to_owned(),
        step("analytic_rrect[0]", &caps),
    )];
    let paints: Vec<_> = corpus_paints(true)
        .into_iter()
        .filter(|(n, _)| n == "solid-Plus" || n == "solid-SrcOver")
        .collect();

    let result = pipelines(&profile, &paints, &steps, &[false]);

    assert_eq!(result.len(), 2);
    for pipeline in result {
        let shaders = pipeline.shaders.expect("the shaders compile");
        let info = &shaders.shader_info;
        assert!(
            info.vertex_sksl()
                .contains("void main() {float2 stepLocalCoords = float2(0);")
        );
        assert!(
            info.fragment_sksl()
                .contains("half4 outputCoverage = half4(1);")
        );
        assert!(
            info.fragment_sksl()
                .contains("sk_FragColor = outColor_1 * outputCoverage;")
        );
        assert_eq!(info.vs_label(), "AnalyticRRectRenderStep");
        assert_eq!(info.blend_info().equation, BlendEquation::Add);
        assert!(info.blend_info().writes_color);
        if pipeline.name.contains("Plus") {
            assert_eq!(info.blend_info().src_blend, BlendCoeff::One);
            assert_eq!(info.blend_info().dst_blend, BlendCoeff::One);
            assert_eq!(
                info.fs_label(),
                "AnalyticRRectRenderStep + SolidColor Plus "
            );
        }
    }
}

#[test]
fn an_advanced_blend_reads_the_dst_and_blends_in_the_shader() {
    // The hardware has no advanced blends, so the fixed Multiply blend reads the dst copy, the
    // hardware blend is Src (One / Zero), the coverage blends with the dst in the shader and a
    // zero coverage is discarded. The fragment label lists the dst read strategy.
    let profile = CapsProfile::dawn_d3d12();
    let caps = WgpuCaps::new(&profile, &ContextOptions::default());
    let steps = vec![(
        "analytic_rrect[0]".to_owned(),
        step("analytic_rrect[0]", &caps),
    )];
    let paints: Vec<_> = corpus_paints(true)
        .into_iter()
        .filter(|(n, _)| n == "solid-Multiply")
        .collect();

    let result = pipelines(&profile, &paints, &steps, &[false]);

    let shaders = result[0].shaders.as_ref().expect("the shaders compile");
    let info = &shaders.shader_info;
    assert_eq!(info.blend_info().src_blend, BlendCoeff::One);
    assert_eq!(info.blend_info().dst_blend, BlendCoeff::Zero);
    let fs = info.fragment_sksl();
    assert!(fs.contains("sampler2D dstSampler;"));
    assert!(fs.contains(
        "dstColor = sample(dstSampler,dstReadBounds.zw*(sk_FragCoord.xy - dstReadBounds.xy));"
    ));
    assert!(fs.contains("half4 outColor_3 = blend_multiply(outColor_1, dstColor);"));
    assert!(fs.contains("if (all(lessThanEqual(outputCoverage.rgb, half3(0)))) {discard;}"));
    assert!(fs.contains(
        "sk_FragColor = outColor_3 * outputCoverage + dstColor * (1.0 - outputCoverage);"
    ));
    assert_eq!(
        info.fs_label(),
        "AnalyticRRectRenderStep + SolidColor Multiply (rgba, TextureCopy)"
    );
    // One texture and sampler: the dst copy.
    assert_eq!(info.num_fragment_textures_and_samplers(), 2);
    assert_eq!(shaders.sampler_descs.len(), 1);
}

#[test]
fn a_draw_without_a_paint_has_only_a_vertex_shader() {
    let profile = CapsProfile::dawn_d3d12();
    let caps = WgpuCaps::new(&profile, &ContextOptions::default());
    let dict = ShaderCodeDictionary::new(Layout::Std140, &[]);
    let step = step("non_aa_bounds_fill[0]", &caps);
    let mut rp_desc = RenderPassDesc::default();
    rp_desc.color_attachment.format = TextureFormat::RGBA8;

    let shaders = make_pipeline_shaders(
        &caps,
        &dict,
        None,
        &rp_desc,
        step.as_ref(),
        UniquePaintParamsID::invalid(),
        &Recorded::default(),
    )
    .expect("the vertex shader compiles");

    assert!(shaders.fragment_wgsl.is_none());
    assert_eq!(shaders.shader_info.fragment_sksl(), "");
    assert!(!shaders.shader_info.blend_info().writes_color);
    assert_eq!(shaders.shader_info.vs_label(), step.name());
    // The shader portion of the label is "(empty)" for depth-only draws.
    assert!(
        shaders
            .shader_info
            .pipeline_label()
            .ends_with(&format!("{} + (empty)", step.name()))
    );
}

#[test]
fn the_write_swizzle_is_applied_to_the_final_color() {
    // An A8 target writes `rrra`-style swizzles in the shader, and the fragment label lists it.
    let profile = CapsProfile::dawn_d3d12();
    let caps = WgpuCaps::new(&profile, &ContextOptions::default());
    let steps = vec![(
        "non_aa_bounds_fill[0]".to_owned(),
        step("non_aa_bounds_fill[0]", &caps),
    )];
    let paints: Vec<_> = corpus_paints(true)
        .into_iter()
        .filter(|(n, _)| n == "solid-Src")
        .collect();

    let result = pipelines(&profile, &paints, &steps, &[true]);

    let shaders = result[0].shaders.as_ref().expect("the shaders compile");
    let info = &shaders.shader_info;
    let swizzle = write_swizzle_for_color_type(ColorType::Alpha8, TextureFormat::R8)
        .expect("a swizzle")
        .as_string();
    assert_ne!(swizzle, "rgba");
    assert_eq!(
        info.fs_label(),
        format!("CoverBoundsRenderStep[NonAAFill] + SolidColor Src ({swizzle})")
    );
    assert!(
        info.fragment_sksl()
            .contains(&format!("outColor_1 = outColor_1.{swizzle};"))
    );
}
