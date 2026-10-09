// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnGraphicsPipeline.h, DawnGraphicsPipeline.cpp

//! [`WgpuGraphicsPipeline`]: `DawnGraphicsPipeline` on wgpu.
//!
//! `DawnGraphicsPipeline::Make` is split in three. [`make_pipeline_shaders`] (in
//! [`pipeline_shaders`](super::pipeline_shaders)) builds the `ShaderInfo` and compiles the `SkSL` to
//! WGSL. [`make_graphics_pipeline_state`] is everything in `Make` that decides what the pipeline
//! *is* (the blend, depth/stencil, primitive, multisample and vertex states, and what the
//! bind-group layout needs): pure functions of the caps, the render step and the render pass, so
//! the mapping to wgpu can be checked without a device. [`WgpuGraphicsPipeline::make`] then
//! creates the shader modules (naga compiles the WGSL inside wgpu), the layouts and the pipeline.
//!
//! # Deviations from Dawn
//!
//! - **No asynchronous creation.** Dawn's `CreateRenderPipelineAsync` returns a future that the
//!   pipeline waits on in `dawnRenderPipeline()`. wgpu has no such call; `docs/design/gpu.md`
//!   §5.4 has the `PipelineCreationTask` run `create_render_pipeline` on the executor instead
//!   (`PipelineManager`). So `DawnCaps::useAsyncPipelineCreation()` does not change how the
//!   pipeline is created here, the pipeline always exists when `make` returns, and
//!   `didAsyncCompilationFail()` is always `None`. A failed creation returns `None`, as Dawn's
//!   synchronous path does ("fail ASAP ... so it affects the Recording snap").
//! - **No immutable samplers.** They exist only for YCbCr conversions, which wgpu does not have
//!   (`getImmutableSamplerInfo` is always the default), so a `SamplerDesc` that asks for one is
//!   an error.
//! - **No `ColorTargetStateExpandResolveTextureDawn`.** wgpu has no load-from-resolve load op
//!   (the caps never set `loadOpAffectsMSAAPipelines` on a real device; the key still has the bit
//!   for the oracle profiles).
//! - **Depth state.** Dawn leaves `depthWriteEnabled` undefined when the depth test is disabled
//!   and treats it as false; wgpu requires it for any format with a depth aspect, so it is
//!   `Some(false)` there, and `None` for stencil-only formats.
//! - Pipeline creation runs in error scopes: wgpu reports an invalid object through the device's
//!   uncaptured-error handler (a panic by default) where Dawn returns a null object.

use std::any::Any;
use std::sync::Arc;

use crate::gpu::blend::{BlendCoeff, BlendEquation, blend_should_disable};
use crate::gpu::resource_key::UniqueKey;
use crate::gpu::sk_log::skia_log_e;
use crate::graphite::draw_types::{
    CompareOp, DepthStencilSettings, Face, PrimitiveType, StencilOp, VertexAttribType,
};
use crate::graphite::graphics_pipeline::{
    GraphicsPipeline, GraphicsPipelineBase, PipelineCreationFlags, PipelineInfo,
};
use crate::graphite::graphics_pipeline_desc::GraphicsPipelineDesc;
use crate::graphite::recorder::RecorderSharedContext;
use crate::graphite::render_pass_desc::RenderPassDesc;
use crate::graphite::render_step::RenderStep;
use crate::graphite::resource_types::SamplerDesc;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::shader_info::ShaderInfo;
use crate::graphite::texture_format::{
    TextureFormat, texture_format_has_depth, texture_format_has_stencil,
    texture_format_is_depth_or_stencil,
};
use crate::graphite::wgpu::async_wait::create_checked;
use crate::graphite::wgpu::caps::WgpuCaps;
use crate::graphite::wgpu::graphite_utils::{
    compile_wgsl_shader_module, texture_format_to_wgpu_format,
};
use crate::graphite::wgpu::pipeline_shaders::make_pipeline_shaders;
use crate::graphite::wgpu::shared_context::WgpuSharedContext;

/// `kUniformBufferBindGroupIndex`.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.h#L51 (chrome/m156)
pub const UNIFORM_BUFFER_BIND_GROUP_INDEX: usize = 0;
/// `kTextureBindGroupIndex`.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.h#L52 (chrome/m156)
pub const TEXTURE_BIND_GROUP_INDEX: usize = 1;
/// `kBindGroupCount`.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.h#L53 (chrome/m156)
pub const BIND_GROUP_COUNT: usize = 2;

/// `kIntrinsicUniformBufferIndex`.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.h#L58 (chrome/m156)
pub const INTRINSIC_UNIFORM_BUFFER_INDEX: u32 = 0;
/// `kCombinedUniformIndex`.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.h#L59 (chrome/m156)
pub const COMBINED_UNIFORM_INDEX: u32 = 1;
/// `kStorageBufferIndex`.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.h#L60 (chrome/m156)
pub const STORAGE_BUFFER_INDEX: u32 = 2;
/// `kMaxNumUniformBuffers`.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.h#L61 (chrome/m156)
pub const MAX_NUM_UNIFORM_BUFFERS: u32 = 3;

/// `kIntrinsicUniformSize`: the size of the immediate data when the intrinsic constants are
/// passed as push constants.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.h#L63 (chrome/m156)
pub const INTRINSIC_UNIFORM_SIZE: u32 = 32;

/// `kStaticDataBufferIndex`.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.h#L65 (chrome/m156)
pub const STATIC_DATA_BUFFER_INDEX: usize = 0;
/// `kAppendDataBufferIndex`.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.h#L66 (chrome/m156)
pub const APPEND_DATA_BUFFER_INDEX: usize = 1;
/// `kNumVertexBuffers`.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.h#L67 (chrome/m156)
pub const NUM_VERTEX_BUFFERS: usize = 2;

/// `DawnGraphicsPipeline::BindGroupLayouts`: the layout of the uniform buffers group, and of the
/// texture group if the fragment shader samples.
pub type BindGroupLayouts = [Option<wgpu::BindGroupLayout>; BIND_GROUP_COUNT];

/// `attribute_type_to_dawn()`.
///
/// # Panics
/// For the attribute types WebGPU has no vertex format for (`kHalf`, `kByte`, `kUByte`,
/// `kUByte_norm`, `kUShort_norm`), which the C++ marks `SkUNREACHABLE`.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.cpp#L43-L98 (chrome/m156)
#[doc(alias = "attribute_type_to_dawn")]
#[must_use]
pub fn attribute_type_to_wgpu(ty: VertexAttribType) -> wgpu::VertexFormat {
    use wgpu::VertexFormat as F;
    match ty {
        VertexAttribType::Float => F::Float32,
        VertexAttribType::Float2 => F::Float32x2,
        VertexAttribType::Float3 => F::Float32x3,
        VertexAttribType::Float4 => F::Float32x4,
        VertexAttribType::Half2 => F::Float16x2,
        VertexAttribType::Half4 => F::Float16x4,
        VertexAttribType::Int2 => F::Sint32x2,
        VertexAttribType::Int3 => F::Sint32x3,
        VertexAttribType::Int4 => F::Sint32x4,
        VertexAttribType::UInt2 => F::Uint32x2,
        VertexAttribType::Byte2 => F::Sint8x2,
        VertexAttribType::Byte4 => F::Sint8x4,
        VertexAttribType::UByte2 => F::Uint8x2,
        VertexAttribType::UByte4 => F::Uint8x4,
        VertexAttribType::UByte4Norm => F::Unorm8x4,
        VertexAttribType::Short2 => F::Sint16x2,
        VertexAttribType::Short4 => F::Sint16x4,
        VertexAttribType::UShort2 => F::Uint16x2,
        VertexAttribType::UShort2Norm => F::Unorm16x2,
        VertexAttribType::Int => F::Sint32,
        VertexAttribType::UInt => F::Uint32,
        VertexAttribType::UShort4Norm => F::Unorm16x4,
        VertexAttribType::Half
        | VertexAttribType::Byte
        | VertexAttribType::UByte
        | VertexAttribType::UByteNorm
        | VertexAttribType::UShortNorm => {
            // Not supported.
            unreachable!("{ty:?} has no WebGPU vertex format")
        }
    }
}

/// `compare_op_to_dawn()`.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.cpp#L100-L121 (chrome/m156)
#[doc(alias = "compare_op_to_dawn")]
#[must_use]
pub fn compare_op_to_wgpu(op: CompareOp) -> wgpu::CompareFunction {
    match op {
        CompareOp::Always => wgpu::CompareFunction::Always,
        CompareOp::Never => wgpu::CompareFunction::Never,
        CompareOp::Greater => wgpu::CompareFunction::Greater,
        CompareOp::GEqual => wgpu::CompareFunction::GreaterEqual,
        CompareOp::Less => wgpu::CompareFunction::Less,
        CompareOp::LEqual => wgpu::CompareFunction::LessEqual,
        CompareOp::Equal => wgpu::CompareFunction::Equal,
        CompareOp::NotEqual => wgpu::CompareFunction::NotEqual,
    }
}

/// `stencil_op_to_dawn()`.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.cpp#L123-L144 (chrome/m156)
#[doc(alias = "stencil_op_to_dawn")]
#[must_use]
pub fn stencil_op_to_wgpu(op: StencilOp) -> wgpu::StencilOperation {
    match op {
        StencilOp::Keep => wgpu::StencilOperation::Keep,
        StencilOp::Zero => wgpu::StencilOperation::Zero,
        StencilOp::Replace => wgpu::StencilOperation::Replace,
        StencilOp::Invert => wgpu::StencilOperation::Invert,
        StencilOp::IncWrap => wgpu::StencilOperation::IncrementWrap,
        StencilOp::DecWrap => wgpu::StencilOperation::DecrementWrap,
        StencilOp::IncClamp => wgpu::StencilOperation::IncrementClamp,
        StencilOp::DecClamp => wgpu::StencilOperation::DecrementClamp,
    }
}

/// `stencil_face_to_dawn()`.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.cpp#L146-L153 (chrome/m156)
#[doc(alias = "stencil_face_to_dawn")]
#[must_use]
pub fn stencil_face_to_wgpu(face: &Face) -> wgpu::StencilFaceState {
    wgpu::StencilFaceState {
        compare: compare_op_to_wgpu(face.compare_op),
        fail_op: stencil_op_to_wgpu(face.stencil_fail_op),
        depth_fail_op: stencil_op_to_wgpu(face.depth_fail_op),
        pass_op: stencil_op_to_wgpu(face.depth_stencil_pass_op),
    }
}

/// `create_vertex_attributes()`: the attributes of one vertex buffer, given the CPU type and
/// `sizeAlign4()` of each, and the stride of the buffer they make up.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.cpp#L155-L172 (chrome/m156)
#[must_use]
pub fn create_vertex_attributes(
    attrs: impl IntoIterator<Item = (VertexAttribType, usize)>,
    shader_location_offset: u32,
) -> (u64, Vec<wgpu::VertexAttribute>) {
    let mut out = Vec::new();
    let mut vertex_attribute_offset = 0_u64;
    for (attribute_index, (cpu_type, size_align4)) in (0_u32..).zip(attrs) {
        out.push(wgpu::VertexAttribute {
            format: attribute_type_to_wgpu(cpu_type),
            offset: vertex_attribute_offset,
            shader_location: shader_location_offset + attribute_index,
        });
        vertex_attribute_offset += size_align4 as u64;
    }
    (vertex_attribute_offset, out)
}

/// `blend_coeff_to_dawn_blend()`. Without dual-source blending (and always in the browser, where
/// `__EMSCRIPTEN__` is defined) the second-source coefficients are `Zero`.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.cpp#L174-L226 (chrome/m156)
#[doc(alias = "blend_coeff_to_dawn_blend")]
#[must_use]
pub fn blend_coeff_to_wgpu_blend(caps: &WgpuCaps, coeff: BlendCoeff) -> wgpu::BlendFactor {
    let value_if_dsb_or_zero = |value: wgpu::BlendFactor| {
        if !cfg!(target_arch = "wasm32") && caps.shader_caps().dual_source_blending_support {
            value
        } else {
            wgpu::BlendFactor::Zero
        }
    };
    match coeff {
        BlendCoeff::Zero | BlendCoeff::Illegal => wgpu::BlendFactor::Zero,
        BlendCoeff::One => wgpu::BlendFactor::One,
        BlendCoeff::SC => wgpu::BlendFactor::Src,
        BlendCoeff::ISC => wgpu::BlendFactor::OneMinusSrc,
        BlendCoeff::DC => wgpu::BlendFactor::Dst,
        BlendCoeff::IDC => wgpu::BlendFactor::OneMinusDst,
        BlendCoeff::SA => wgpu::BlendFactor::SrcAlpha,
        BlendCoeff::ISA => wgpu::BlendFactor::OneMinusSrcAlpha,
        BlendCoeff::DA => wgpu::BlendFactor::DstAlpha,
        BlendCoeff::IDA => wgpu::BlendFactor::OneMinusDstAlpha,
        BlendCoeff::ConstC => wgpu::BlendFactor::Constant,
        BlendCoeff::IConstC => wgpu::BlendFactor::OneMinusConstant,
        BlendCoeff::S2C => value_if_dsb_or_zero(wgpu::BlendFactor::Src1),
        BlendCoeff::IS2C => value_if_dsb_or_zero(wgpu::BlendFactor::OneMinusSrc1),
        BlendCoeff::S2A => value_if_dsb_or_zero(wgpu::BlendFactor::Src1Alpha),
        BlendCoeff::IS2A => value_if_dsb_or_zero(wgpu::BlendFactor::OneMinusSrc1Alpha),
    }
}

/// `blend_coeff_to_dawn_blend_for_alpha()`.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.cpp#L228-L244 (chrome/m156)
#[doc(alias = "blend_coeff_to_dawn_blend_for_alpha")]
#[must_use]
pub fn blend_coeff_to_wgpu_blend_for_alpha(
    caps: &WgpuCaps,
    coeff: BlendCoeff,
) -> wgpu::BlendFactor {
    match coeff {
        // Force all srcColor used in alpha slot to alpha version.
        BlendCoeff::SC => wgpu::BlendFactor::SrcAlpha,
        BlendCoeff::ISC => wgpu::BlendFactor::OneMinusSrcAlpha,
        BlendCoeff::DC => wgpu::BlendFactor::DstAlpha,
        BlendCoeff::IDC => wgpu::BlendFactor::OneMinusDstAlpha,
        _ => blend_coeff_to_wgpu_blend(caps, coeff),
    }
}

/// `blend_equation_to_dawn_blend_op()`.
///
/// # Panics
/// For the advanced equations, which Graphite on wgpu never blends in hardware.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.cpp#L246-L262 (chrome/m156)
#[doc(alias = "blend_equation_to_dawn_blend_op")]
#[must_use]
pub fn blend_equation_to_wgpu_blend_op(equation: BlendEquation) -> wgpu::BlendOperation {
    match equation {
        BlendEquation::Add => wgpu::BlendOperation::Add,
        BlendEquation::Subtract => wgpu::BlendOperation::Subtract,
        BlendEquation::ReverseSubtract => wgpu::BlendOperation::ReverseSubtract,
        _ => unreachable!("{equation:?} is not a basic blend equation"),
    }
}

/// One vertex buffer of the pipeline (a `wgpu::VertexBufferLayout` that owns its attributes).
#[derive(Clone, Debug, PartialEq)]
pub struct VertexBufferState {
    /// `arrayStride`.
    pub array_stride: u64,
    /// `stepMode`.
    pub step_mode: wgpu::VertexStepMode,
    /// `attributes`.
    pub attributes: Vec<wgpu::VertexAttribute>,
}

/// What the pipeline is, in wgpu's terms: the part of `DawnGraphicsPipeline::Make` that is a pure
/// function of the caps, the render step, the `ShaderInfo` and the render pass.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.cpp#L430-L718 (chrome/m156)
#[derive(Clone, Debug, PartialEq)]
pub struct GraphicsPipelineState {
    /// `descriptor.label`: always set, since "dawn may need it for tracing".
    pub label: String,
    /// Whether the fragment `SkSL` is non-empty; if not the noop fragment shader is used.
    pub has_fragment_shader: bool,
    /// The single color target of the fragment state.
    pub color_target: wgpu::ColorTargetState,
    /// `descriptor.depthStencil`: `None` if the render pass has no depth/stencil attachment.
    pub depth_stencil: Option<wgpu::DepthStencilState>,
    /// The static data buffer (slot 0) and the append data buffer (slot 1) of the vertex state;
    /// `None` for a buffer with no attributes.
    pub vertex_buffers: [Option<VertexBufferState>; NUM_VERTEX_BUFFERS],
    /// `descriptor.primitive`.
    pub primitive: wgpu::PrimitiveState,
    /// `descriptor.multisample`.
    pub multisample: wgpu::MultisampleState,
    /// The shader stages that see the storage buffer binding of the uniform buffers group.
    pub storage_visibility: wgpu::ShaderStages,
    /// `layoutDesc.immediateSize`.
    pub immediate_size: u32,
    /// `step->primitiveType()`.
    pub primitive_type: PrimitiveType,
    /// `depthStencilSettings.fStencilReferenceValue`.
    pub stencil_reference_value: u32,
}

/// The state of `DawnGraphicsPipeline::Make` that follows from its inputs. Returns `None` if the
/// render pass's formats have no wgpu equivalent.
///
/// # Panics
/// If the blend equation is an advanced one (Graphite on wgpu never blends in hardware with
/// those), or if a step has more than `u32::MAX` static attributes.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.cpp#L430-L626 (chrome/m156)
#[must_use]
#[allow(clippy::too_many_lines)] // mirrors the state half of DawnGraphicsPipeline::Make
pub fn make_graphics_pipeline_state(
    caps: &WgpuCaps,
    step: &dyn RenderStep,
    shader_info: &ShaderInfo,
    render_pass_desc: &RenderPassDesc,
) -> Option<GraphicsPipelineState> {
    let has_fragment_sksl = !shader_info.fragment_sksl().is_empty();

    // Fragment state
    let blend_info = shader_info.blend_info();
    let equation = blend_info.equation;
    let src_coeff = blend_info.src_blend;
    let dst_coeff = blend_info.dst_blend;
    let blend_on = !blend_should_disable(equation, src_coeff, dst_coeff);

    let blend = blend_on.then(|| wgpu::BlendState {
        color: wgpu::BlendComponent {
            operation: blend_equation_to_wgpu_blend_op(equation),
            src_factor: blend_coeff_to_wgpu_blend(caps, src_coeff),
            dst_factor: blend_coeff_to_wgpu_blend(caps, dst_coeff),
        },
        alpha: wgpu::BlendComponent {
            operation: blend_equation_to_wgpu_blend_op(equation),
            src_factor: blend_coeff_to_wgpu_blend_for_alpha(caps, src_coeff),
            dst_factor: blend_coeff_to_wgpu_blend_for_alpha(caps, dst_coeff),
        },
    });

    let color_target = wgpu::ColorTargetState {
        format: texture_format_to_wgpu_format(render_pass_desc.color_attachment.format)?,
        blend,
        write_mask: if blend_info.writes_color && has_fragment_sksl {
            wgpu::ColorWrites::ALL
        } else {
            wgpu::ColorWrites::empty()
        },
    };

    // A render pass that loads the resolve texture (`loadMsaaFromResolve`) would, with Dawn, need
    // a `ColorTargetStateExpandResolveTextureDawn` chained to make the pipeline compatible. wgpu
    // has no such load op (see the module docs); the render pass is still described by its key.

    // Depth stencil state
    let depth_stencil_settings: &DepthStencilSettings = step.base().depth_stencil_settings();
    debug_assert!(
        depth_stencil_settings.depth_test_enabled
            || depth_stencil_settings.depth_compare_op == CompareOp::Always
    );
    let ds_format = render_pass_desc.depth_stencil_attachment.format;
    let depth_stencil = if ds_format == TextureFormat::Unsupported {
        None
    } else {
        debug_assert!(texture_format_is_depth_or_stencil(ds_format));
        let has_depth = texture_format_has_depth(ds_format);
        let mut depth_stencil = wgpu::DepthStencilState {
            format: texture_format_to_wgpu_format(ds_format)?,
            // See the module docs: Dawn's undefined `depthWriteEnabled` means false.
            depth_write_enabled: has_depth.then_some(
                depth_stencil_settings.depth_test_enabled
                    && depth_stencil_settings.depth_write_enabled,
            ),
            depth_compare: has_depth
                .then_some(compare_op_to_wgpu(depth_stencil_settings.depth_compare_op)),
            // The defaults of `wgpu::DepthStencilState` in Dawn (the WebGPU defaults).
            stencil: wgpu::StencilState {
                front: wgpu::StencilFaceState::IGNORE,
                back: wgpu::StencilFaceState::IGNORE,
                read_mask: 0xFFFF_FFFF,
                write_mask: 0xFFFF_FFFF,
            },
            bias: wgpu::DepthBiasState::default(),
        };

        // Dawn validation fails if the stencil state is non-default and the
        // format doesn't have the stencil aspect.
        if texture_format_has_stencil(ds_format) && depth_stencil_settings.stencil_test_enabled {
            depth_stencil.stencil = wgpu::StencilState {
                front: stencil_face_to_wgpu(&depth_stencil_settings.front_stencil),
                back: stencil_face_to_wgpu(&depth_stencil_settings.back_stencil),
                read_mask: depth_stencil_settings.front_stencil.read_mask,
                write_mask: depth_stencil_settings.front_stencil.write_mask,
            };
        }
        Some(depth_stencil)
    };

    // Vertex state
    // Static data buffer layout
    let (static_stride, static_attributes) = create_vertex_attributes(
        step.static_attributes()
            .iter()
            .map(|a| (a.cpu_type(), a.size_align4())),
        0,
    );
    let static_buffer = (static_stride != 0).then_some(VertexBufferState {
        array_stride: static_stride,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: static_attributes,
    });

    // Append data buffer layout
    // Note: the shaderLocationOffset in this function call needs to be the staticAttributeSize
    let (append_stride, append_attributes) = create_vertex_attributes(
        shader_info
            .append_attributes()
            .iter()
            .map(|a| (a.cpu_type(), a.size_align4())),
        u32::try_from(step.static_attributes().len()).expect("a few attributes"),
    );
    let append_buffer = (append_stride != 0).then(|| VertexBufferState {
        array_stride: append_stride,
        step_mode: if step.appends_vertices() {
            wgpu::VertexStepMode::Vertex
        } else {
            wgpu::VertexStepMode::Instance
        },
        attributes: append_attributes,
    });

    // Other state
    let (topology, strip_index_format) = match step.base().primitive_type() {
        PrimitiveType::Triangles => (wgpu::PrimitiveTopology::TriangleList, None),
        PrimitiveType::TriangleStrip => (
            wgpu::PrimitiveTopology::TriangleStrip,
            Some(wgpu::IndexFormat::Uint16),
        ),
        PrimitiveType::Points => (wgpu::PrimitiveTopology::PointList, None),
    };
    let primitive = wgpu::PrimitiveState {
        topology,
        strip_index_format,
        front_face: wgpu::FrontFace::Ccw,
        cull_mode: None,
        ..wgpu::PrimitiveState::default()
    };

    // Multisampled state
    let multisample = wgpu::MultisampleState {
        count: render_pass_desc.sample_count as u32,
        mask: 0xFFFF_FFFF,
        alpha_to_coverage_enabled: false,
    };

    let mut storage_visibility = wgpu::ShaderStages::NONE;
    if shader_info.vs_uses_storage() {
        storage_visibility |= wgpu::ShaderStages::VERTEX;
    }
    if shader_info.fs_uses_storage() {
        storage_visibility |= wgpu::ShaderStages::FRAGMENT;
    }

    let immediate_size = if !cfg!(target_arch = "wasm32")
        && caps
            .resource_binding_requirements()
            .use_push_constants_for_intrinsic_constants
    {
        INTRINSIC_UNIFORM_SIZE
    } else {
        0
    };

    Some(GraphicsPipelineState {
        label: shader_info.pipeline_label().to_owned(),
        has_fragment_shader: has_fragment_sksl,
        color_target,
        depth_stencil,
        vertex_buffers: [static_buffer, append_buffer],
        primitive,
        multisample,
        storage_visibility,
        immediate_size,
        primitive_type: step.base().primitive_type(),
        stencil_reference_value: depth_stencil_settings.stencil_reference_value,
    })
}

/// `DawnGraphicsPipeline`: a wgpu render pipeline and what the command buffer needs to bind
/// resources for it.
// Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.h#L47-L108 (chrome/m156)
#[doc(alias = "DawnGraphicsPipeline")]
pub struct WgpuGraphicsPipeline {
    base: GraphicsPipelineBase,
    /// The pipeline. Dawn's is created asynchronously and waited for in `dawnRenderPipeline()`;
    /// this one exists when the struct does.
    render_pipeline: wgpu::RenderPipeline,
    /// `fGroupLayouts`.
    group_layouts: BindGroupLayouts,
    /// What was passed to `CreateRenderPipeline` (besides the shader modules and layouts).
    state: GraphicsPipelineState,
}

impl std::fmt::Debug for WgpuGraphicsPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WgpuGraphicsPipeline")
            .field("label", &self.base.label())
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl WgpuGraphicsPipeline {
    /// `DawnGraphicsPipeline::Make()`: compiles the shaders and creates the pipeline, or returns
    /// `None` (after reporting to the shader error handler or logging) if any step fails.
    // Port of: src/gpu/graphite/dawn/DawnGraphicsPipeline.cpp#L328-L730 (chrome/m156)
    #[must_use]
    #[allow(clippy::too_many_lines)] // mirrors DawnGraphicsPipeline::Make
    pub fn make(
        shared_context: &WgpuSharedContext,
        runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
        pipeline_key: &UniqueKey,
        pipeline_desc: &GraphicsPipelineDesc,
        render_pass_desc: &RenderPassDesc,
        pipeline_creation_flags: PipelineCreationFlags,
        compilation_id: u32,
    ) -> Option<Arc<Self>> {
        let caps: &WgpuCaps = shared_context.caps();
        let device = shared_context.device();
        let error_handler = caps.shader_error_handler();

        let Some(step) = RecorderSharedContext::renderer_provider(shared_context)
            .lookup(pipeline_desc.render_step_id())
        else {
            skia_log_e!(
                "No render step {:?} to make a pipeline from",
                pipeline_desc.render_step_id()
            );
            return None;
        };

        // The SkSL, and the WGSL it compiles to.
        // Some steps just render depth buffer but not color buffer, so the fragment
        // shader is null.
        let shaders = make_pipeline_shaders(
            caps,
            shared_context.shader_code_dictionary(),
            runtime_dict.map(Arc::clone),
            render_pass_desc,
            &**step,
            pipeline_desc.paint_params_id(),
            error_handler,
        )?;
        let shader_info = &*shaders.shader_info;
        let num_textures_and_samplers = shader_info.num_fragment_textures_and_samplers();

        let has_fragment_sksl = shaders.fragment_wgsl.is_some();
        let fs_module = match &shaders.fragment_wgsl {
            Some(fs_wgsl) => Some(compile_wgsl_shader_module(
                shared_context,
                shader_info.fs_label(),
                fs_wgsl,
                error_handler,
            )?),
            None => None,
        };
        let vs_module = compile_wgsl_shader_module(
            shared_context,
            shader_info.vs_label(),
            &shaders.vertex_wgsl,
            error_handler,
        )?;

        let state = make_graphics_pipeline_state(caps, &**step, shader_info, render_pass_desc)?;

        // Immutable samplers exist only for YCbCr conversions, which wgpu does not have.
        if shaders.sampler_descs.iter().any(SamplerDesc::is_immutable) {
            skia_log_e!("Immutable samplers are not supported on wgpu");
            return None;
        }

        // Creating the layouts and the pipeline: wgpu raises an invalid object as an error of the
        // device, so they are made inside error scopes, and an error is a creation failure.
        let created = create_checked(device, caps.allow_scoped_error_checks(), || {
            // Determine the BindGroupLayouts that will be used to make up the pipeline layout.
            let mut group_layouts: BindGroupLayouts = [None, None];
            group_layouts[UNIFORM_BUFFER_BIND_GROUP_INDEX] = Some(
                shared_context
                    .get_uniform_buffers_bind_group_layout(state.storage_visibility)
                    .clone(),
            );

            let has_fragment_samplers = has_fragment_sksl && num_textures_and_samplers > 0;
            if has_fragment_samplers {
                // Check if we can optimize for the common case of a single texture + 1 dynamic
                // sampler
                group_layouts[TEXTURE_BIND_GROUP_INDEX] = Some(if num_textures_and_samplers == 2 {
                    shared_context
                        .get_single_texture_sampler_bind_group_layout()
                        .clone()
                } else {
                    let mut entries = Vec::new();
                    // Each sampler is followed by its texture in the layout.
                    for i in (0..u32::try_from(num_textures_and_samplers).unwrap_or(0)).step_by(2) {
                        entries.push(wgpu::BindGroupLayoutEntry {
                            binding: i,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        });
                        entries.push(wgpu::BindGroupLayoutEntry {
                            binding: i + 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        });
                    }
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: caps.set_backend_labels().then_some(shader_info.vs_label()),
                        entries: &entries,
                    })
                });
            }

            let layout_count = if has_fragment_samplers {
                BIND_GROUP_COUNT
            } else {
                BIND_GROUP_COUNT - 1
            };
            let layout_refs: Vec<Option<&wgpu::BindGroupLayout>> = group_layouts[..layout_count]
                .iter()
                .map(Option::as_ref)
                .collect();
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: caps.set_backend_labels().then_some(shader_info.fs_label()),
                bind_group_layouts: &layout_refs,
                immediate_size: state.immediate_size,
            });

            let vertex_buffers: Vec<Option<wgpu::VertexBufferLayout<'_>>> = state
                .vertex_buffers
                .iter()
                .map(|buffer| {
                    buffer.as_ref().map(|buffer| wgpu::VertexBufferLayout {
                        array_stride: buffer.array_stride,
                        step_mode: buffer.step_mode,
                        attributes: &buffer.attributes,
                    })
                })
                .collect();
            let targets = [Some(state.color_target.clone())];
            // Dawn doesn't allow having a color attachment but without fragment shader, so have
            // to use a noop fragment shader, if fragment shader is null.
            let fragment_module = fs_module
                .as_ref()
                .unwrap_or_else(|| shared_context.noop_fragment());
            let descriptor = wgpu::RenderPipelineDescriptor {
                // Always set the label for pipelines, dawn may need it for tracing.
                label: Some(&state.label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &vs_module,
                    entry_point: Some("main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &vertex_buffers,
                },
                primitive: state.primitive,
                depth_stencil: state.depth_stencil.clone(),
                multisample: state.multisample,
                fragment: Some(wgpu::FragmentState {
                    module: fragment_module,
                    entry_point: Some("main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &targets,
                }),
                multiview_mask: None,
                cache: None,
            };
            (group_layouts, device.create_render_pipeline(&descriptor))
        });
        // Fail ASAP for synchronous pipeline creation so it affects the Recording snap instead of
        // being detected later at insertRecording().
        let (group_layouts, render_pipeline) = created?;

        trace!(
            shared_context,
            crate::graphite::wgpu::trace::Record::new("create_pipeline")
                .s("label", shader_info.pipeline_label())
                .u("key", pipeline_key.hash())
                .u(
                    "vertex_wgsl_hash",
                    crate::graphite::wgpu::trace::hash_bytes(shaders.vertex_wgsl.as_bytes())
                )
                .u(
                    "fragment_wgsl_hash",
                    shaders.fragment_wgsl.as_ref().map_or(0, |fs| {
                        crate::graphite::wgpu::trace::hash_bytes(fs.as_bytes())
                    })
                )
        );
        #[cfg(feature = "trace")]
        {
            // The WGSL goes to the sink as blobs, as the oracle's `pipelines/` dump has it.
            shared_context.trace_with_blob(shaders.vertex_wgsl.as_bytes(), |hash| {
                crate::graphite::wgpu::trace::Record::new("wgsl")
                    .u("hash", hash)
                    .s("stage", "vertex")
            });
            if let Some(fs) = &shaders.fragment_wgsl {
                shared_context.trace_with_blob(fs.as_bytes(), |hash| {
                    crate::graphite::wgpu::trace::Record::new("wgsl")
                        .u("hash", hash)
                        .s("stage", "fragment")
                });
            }
        }

        let mut pipeline_info = PipelineInfo::from_shader_info(shader_info);
        pipeline_info
            .native_vertex_shader
            .clone_from(&shaders.vertex_wgsl);
        pipeline_info.native_fragment_shader = shaders.fragment_wgsl.clone().unwrap_or_default();

        Some(Arc::new(Self {
            base: GraphicsPipelineBase::with_info(
                shader_info.pipeline_label(),
                pipeline_info,
                pipeline_key.hash(),
                compilation_id,
                pipeline_creation_flags.contains(PipelineCreationFlags::FOR_PRECOMPILATION),
            ),
            render_pipeline,
            group_layouts,
            state,
        }))
    }

    /// `stencilReferenceValue()`.
    #[doc(alias = "stencilReferenceValue")]
    #[must_use]
    pub fn stencil_reference_value(&self) -> u32 {
        self.state.stencil_reference_value
    }

    /// `primitiveType()`.
    #[doc(alias = "primitiveType")]
    #[must_use]
    pub fn primitive_type(&self) -> PrimitiveType {
        self.state.primitive_type
    }

    /// `dawnRenderPipeline()`.
    #[doc(alias = "dawnRenderPipeline")]
    #[must_use]
    pub fn render_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.render_pipeline
    }

    /// `dawnGroupLayouts()`: the layout of the uniform buffers group, and of the texture group if
    /// the fragment shader samples (otherwise `None`).
    #[doc(alias = "dawnGroupLayouts")]
    #[must_use]
    pub fn group_layouts(&self) -> &BindGroupLayouts {
        &self.group_layouts
    }

    /// The pipeline's state as passed to wgpu.
    #[must_use]
    pub fn state(&self) -> &GraphicsPipelineState {
        &self.state
    }
}

impl GraphicsPipeline for WgpuGraphicsPipeline {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn base(&self) -> &GraphicsPipelineBase {
        &self.base
    }
}
