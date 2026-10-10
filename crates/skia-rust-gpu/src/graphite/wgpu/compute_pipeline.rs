// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnComputePipeline.h, DawnComputePipeline.cpp

//! [`WgpuComputePipeline`]: `DawnComputePipeline` on wgpu.
//!
//! Like the graphics pipeline, the creation runs in error scopes, so a bind group layout, pipeline
//! layout or pipeline that wgpu rejects is a `None` rather than an error of the device.

use std::any::Any;
use std::sync::Arc;

use skia_rust_sksl::ir::ProgramInterface;
use skia_rust_sksl::program_settings::{ProgramKind, ProgramSettings};

use crate::gpu::gpu_types::BackendApi;
use crate::gpu::sk_log::skia_log_e;
use crate::gpu::sksl_to_backend::sksl_to_wgsl;
use crate::graphite::compute::compute_step::{ComputeStep, NativeShaderFormat, ResourceType};
use crate::graphite::compute_pipeline::ComputePipeline;
use crate::graphite::compute_pipeline_desc::ComputePipelineDesc;
use crate::graphite::context_utils::build_compute_sksl;
use crate::graphite::wgpu::async_wait::create_checked;
use crate::graphite::wgpu::graphite_utils::compile_wgsl_shader_module;
use crate::graphite::wgpu::shared_context::WgpuSharedContext;
use crate::graphite::wgpu::texture_info::WgpuTextureInfoData;

/// The compiled shader module of a compute step and its entry point (`ShaderInfo` in the
/// anonymous namespace of `DawnComputePipeline.cpp`).
struct ShaderModuleAndEntryPoint {
    module: wgpu::ShaderModule,
    entry_point: String,
}

/// `compile_shader_module()`: the step's native WGSL, or its `SkSL` compiled to WGSL.
// Port of: src/gpu/graphite/dawn/DawnComputePipeline.cpp#L31-L75 (chrome/m156)
fn compile_shader_module(
    shared_context: &WgpuSharedContext,
    pipeline_desc: &ComputePipelineDesc,
) -> Option<ShaderModuleAndEntryPoint> {
    let caps = shared_context.caps();
    let step = pipeline_desc.compute_step();
    let error_handler = caps.shader_error_handler();

    if step.supports_native_shader() {
        let native_shader = step.native_shader_source(NativeShaderFormat::Wgsl);
        let module = compile_wgsl_shader_module(
            shared_context,
            step.name(),
            native_shader.source,
            error_handler,
        )?;
        Some(ShaderModuleAndEntryPoint {
            module,
            entry_point: native_shader.entry_point,
        })
    } else {
        let mut interface = ProgramInterface::default();
        let settings = ProgramSettings {
            force_high_precision: !caps.supports_half_precision(),
            ..ProgramSettings::default()
        };

        let sksl = build_compute_sksl(&**caps, step, BackendApi::Dawn);
        let wgsl = sksl_to_wgsl(
            caps.shader_caps(),
            &sksl,
            ProgramKind::Compute,
            settings,
            Some(&mut interface),
            error_handler,
        )?;
        let module = compile_wgsl_shader_module(shared_context, step.name(), &wgsl, error_handler)?;
        Some(ShaderModuleAndEntryPoint {
            module,
            entry_point: "main".to_owned(),
        })
    }
}

/// The bind group layout entries of a compute step: all resources go to a single bind group, with
/// the binding index assigned in increasing order. A sampled texture takes two (a sampler, then
/// the texture).
// Port of: src/gpu/graphite/dawn/DawnComputePipeline.cpp#L86-L152 (chrome/m156)
fn bind_group_layout_entries(
    shared_context: &WgpuSharedContext,
    step: &dyn ComputeStep,
) -> Option<Vec<wgpu::BindGroupLayoutEntry>> {
    // ComputeStep resources are listed in the order that they must be declared in the shader.
    // This order is then used for the index assignment using an "indexed by order" policy that
    // has backend-specific semantics. The semantics on Dawn is to assign the index number in
    // increasing order.
    //
    // For compute pipelines, all resources get assigned to a single bind group at index 0
    // (ignoring bind group indices assigned in by DawnCaps's ResourceBindingRequirements, which
    // are for non-compute shaders).
    let buffer = |ty| wgpu::BindingType::Buffer {
        ty,
        has_dynamic_offset: false,
        min_binding_size: None,
    };
    let float_texture = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: true },
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    };

    let resources = step.resources();
    // Sampled textures count as 2 resources (1 texture and 1 sampler). All other types count as
    // 1.
    let resource_count = resources.len()
        + resources
            .iter()
            .filter(|r| r.ty == ResourceType::SampledTexture)
            .count();
    let mut entries = Vec::with_capacity(resource_count);
    for (declaration_index, r) in resources.iter().enumerate() {
        let binding = u32::try_from(entries.len()).expect("a few resources");
        let mut entry = wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: float_texture,
            count: None,
        };
        match r.ty {
            ResourceType::UniformBuffer => {
                entry.ty = buffer(wgpu::BufferBindingType::Uniform);
            }
            ResourceType::StorageBuffer | ResourceType::IndirectBuffer => {
                entry.ty = buffer(wgpu::BufferBindingType::Storage { read_only: false });
            }
            ResourceType::ReadOnlyStorageBuffer => {
                entry.ty = buffer(wgpu::BufferBindingType::Storage { read_only: true });
            }
            ResourceType::ReadOnlyTexture => {
                entry.ty = float_texture;
            }
            ResourceType::WriteOnlyStorageTexture => {
                let (_, color_type) = step.calculate_texture_parameters(declaration_index, r);
                let texture_info = shared_context
                    .caps()
                    .get_default_storage_texture_info(color_type);
                let format = texture_info
                    .get::<WgpuTextureInfoData>()
                    .and_then(WgpuTextureInfoData::get_view_format)?;
                entry.ty = wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format,
                    view_dimension: wgpu::TextureViewDimension::D2,
                };
            }
            ResourceType::SampledTexture => {
                entry.ty = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);

                // Add an additional entry for the texture.
                entries.push(entry);
                entry = wgpu::BindGroupLayoutEntry {
                    binding: binding + 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: float_texture,
                    count: None,
                };
            }
        }
        entries.push(entry);
    }
    Some(entries)
}

/// `DawnComputePipeline`: a wgpu compute pipeline and its bind group layout.
// Port of: src/gpu/graphite/dawn/DawnComputePipeline.h#L20-L38 (chrome/m156)
#[doc(alias = "DawnComputePipeline")]
#[derive(Debug)]
pub struct WgpuComputePipeline {
    /// `fPipeline`.
    pipeline: wgpu::ComputePipeline,
    /// `fGroupLayout`.
    group_layout: wgpu::BindGroupLayout,
    /// The bind group layout entries the layout was made from.
    entries: Vec<wgpu::BindGroupLayoutEntry>,
}

impl WgpuComputePipeline {
    /// `DawnComputePipeline::Make()`: compiles the step's shader and creates the pipeline, or
    /// returns `None` if any step fails.
    // Port of: src/gpu/graphite/dawn/DawnComputePipeline.cpp#L77-L196 (chrome/m156)
    #[must_use]
    pub fn make(
        shared_context: &WgpuSharedContext,
        pipeline_desc: &ComputePipelineDesc,
    ) -> Option<Arc<Self>> {
        let ShaderModuleAndEntryPoint {
            module: shader_module,
            entry_point: entry_point_name,
        } = compile_shader_module(shared_context, pipeline_desc)?;

        let step = pipeline_desc.compute_step();
        let Some(entries) = bind_group_layout_entries(shared_context, step) else {
            skia_log_e!("No storage texture format for compute step {}", step.name());
            return None;
        };

        let caps = shared_context.caps();
        let device = shared_context.device();

        let created = create_checked(device, caps.allow_scoped_error_checks(), || {
            // All resources of a ComputeStep currently get assigned to a single bind group at
            // index 0.
            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &entries,
                });

            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: caps.set_backend_labels().then_some(step.name()),
                bind_group_layouts: &[Some(&bind_group_layout)],
                immediate_size: 0,
            });

            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                // Always set the label for pipelines, dawn may need it for tracing.
                label: Some(step.name()),
                layout: Some(&layout),
                module: &shader_module,
                entry_point: Some(&entry_point_name),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });
            (pipeline, bind_group_layout)
        });
        let (pipeline, group_layout) = created?;

        Some(Arc::new(Self {
            pipeline,
            group_layout,
            entries,
        }))
    }

    /// `dawnComputePipeline()`.
    #[doc(alias = "dawnComputePipeline")]
    #[must_use]
    pub fn compute_pipeline(&self) -> &wgpu::ComputePipeline {
        &self.pipeline
    }

    /// `dawnGroupLayout()`.
    #[doc(alias = "dawnGroupLayout")]
    #[must_use]
    pub fn group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.group_layout
    }

    /// The entries of the group layout, in binding order.
    #[must_use]
    pub fn group_layout_entries(&self) -> &[wgpu::BindGroupLayoutEntry] {
        &self.entries
    }
}

impl ComputePipeline for WgpuComputePipeline {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
