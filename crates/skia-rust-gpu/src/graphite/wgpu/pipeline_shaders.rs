// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnGraphicsPipeline.cpp (the shader half of `Make`)

//! The shaders of a graphics pipeline: [`ShaderInfo`] and its `SkSL` compiled to WGSL.
//!
//! This is the part of `DawnGraphicsPipeline::Make` that needs no device. It is split out because
//! it is what the byte-identical-WGSL criterion checks (`docs/design/gpu.md` §6.3): with a
//! [`WgpuCaps`] built from a [`CapsProfile`] it runs on any machine. `GraphicsPipeline` (G11b)
//! builds the wgpu shader modules and the pipeline from its result.

use std::sync::Arc;

use skia_rust_sksl::ir::ProgramInterface;
use skia_rust_sksl::program_settings::{ProgramKind, ProgramSettings};

use crate::gpu::shader_error_handler::ShaderErrorHandler;
use crate::gpu::sksl_to_backend::sksl_to_wgsl;
use crate::graphite::render_pass_desc::RenderPassDesc;
use crate::graphite::render_step::RenderStep;
use crate::graphite::resource_types::SamplerDesc;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::shader_code_dictionary::ShaderCodeDictionary;
use crate::graphite::shader_info::ShaderInfo;
use crate::graphite::unique_paint_params_id::UniquePaintParamsID;
use crate::graphite::wgpu::caps::WgpuCaps;

/// The `SkSL` program settings of every Graphite pipeline on Dawn.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.cpp#L343-L345 (chrome/m156)
#[must_use]
pub fn pipeline_program_settings(caps: &WgpuCaps) -> ProgramSettings {
    ProgramSettings {
        sharpen_textures: true,
        force_no_rt_flip: true,
        force_high_precision: !caps.supports_half_precision(),
        ..ProgramSettings::default()
    }
}

/// The shaders `DawnGraphicsPipeline::Make` hands to the device.
#[derive(Debug)]
pub struct PipelineShaders {
    /// The `SkSL`, labels and blend state.
    pub shader_info: Box<ShaderInfo>,
    /// The `SamplerDesc`s of the fragment shader's samplers.
    pub sampler_descs: Vec<SamplerDesc>,
    /// The fragment WGSL; `None` for a depth-only draw (no fragment `SkSL`).
    pub fragment_wgsl: Option<String>,
    /// The interface of the fragment program.
    pub fragment_interface: ProgramInterface,
    /// The vertex WGSL.
    pub vertex_wgsl: String,
    /// The interface of the vertex program.
    pub vertex_interface: ProgramInterface,
}

/// The shader half of `DawnGraphicsPipeline::Make`: builds the [`ShaderInfo`] of the pipeline and
/// compiles its `SkSL` to WGSL. Returns `None` after reporting to `error_handler` if a shader does
/// not compile.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.cpp#L330-L398 (chrome/m156)
#[must_use]
pub fn make_pipeline_shaders(
    caps: &WgpuCaps,
    dict: &ShaderCodeDictionary,
    runtime_dict: Option<Arc<RuntimeEffectDictionary>>,
    render_pass_desc: &RenderPassDesc,
    step: &dyn RenderStep,
    paint_id: UniquePaintParamsID,
    error_handler: &dyn ShaderErrorHandler,
) -> Option<PipelineShaders> {
    let settings = pipeline_program_settings(caps);

    let mut sampler_descs = Vec::new();
    let shader_info = ShaderInfo::make(
        caps,
        dict,
        runtime_dict,
        render_pass_desc,
        step,
        paint_id,
        Some(&mut sampler_descs),
    );

    // Some steps just render depth buffer but not color buffer, so the fragment shader is null.
    let fs_sksl = shader_info.fragment_sksl();
    let mut fragment_interface = ProgramInterface::default();
    let fragment_wgsl = if fs_sksl.is_empty() {
        None
    } else {
        Some(sksl_to_wgsl(
            caps.shader_caps(),
            fs_sksl,
            ProgramKind::GraphiteFragment,
            settings,
            Some(&mut fragment_interface),
            error_handler,
        )?)
    };

    let mut vertex_interface = ProgramInterface::default();
    let vertex_wgsl = sksl_to_wgsl(
        caps.shader_caps(),
        shader_info.vertex_sksl(),
        ProgramKind::GraphiteVertex,
        settings,
        Some(&mut vertex_interface),
        error_handler,
    )?;

    Some(PipelineShaders {
        shader_info,
        sampler_descs,
        fragment_wgsl,
        fragment_interface,
        vertex_wgsl,
        vertex_interface,
    })
}
