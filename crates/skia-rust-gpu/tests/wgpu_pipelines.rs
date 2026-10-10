// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The wgpu back end II (G11b) on wgpu's `noop` backend: `DawnGraphicsPipeline`,
//! `DawnComputePipeline`, `DawnErrorChecker`, `DawnCompileWGSLShaderModule`,
//! `DawnCaps::makeGraphicsPipelineKey` and `PipelineManager` (with `PipelineCreationTask` and
//! `GraphicsPipelineHandle`), with no GPU. The noop backend validates the WGSL with naga and the
//! pipeline against its layouts, so a pipeline that is created has a consistent shader, layout and
//! state; it renders nothing (`docs/design/gpu.md` §7).
//!
//! The pipelines come from a slice of the G6 WGSL corpus (`support::wgsl_corpus`): paints keyed
//! into the shared context's own shader dictionary, drawn with every render step of the renderer
//! provider.
#![cfg(not(target_arch = "wasm32"))]

mod support;

use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::executor::{Executor, ThreadPool, WorkOrder};
use skia_rust_core::image_info::ColorInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::size::ISize;
use skia_rust_gpu::gpu::blend::{BlendCoeff, BlendEquation};
use skia_rust_gpu::gpu::gpu_types::BackendApi;
use skia_rust_gpu::gpu::shader_error_handler::{ShaderErrorHandler, build_shader_error_message};
use skia_rust_gpu::graphite::caps::Caps;
use skia_rust_gpu::graphite::compute::compute_step::{
    ComputeStep, ComputeStepBase, DataFlow, NativeShaderFormat, NativeShaderSource, ResourceDesc,
    ResourcePolicy, ResourceType, WorkgroupSize,
};
use skia_rust_gpu::graphite::compute_pipeline_desc::ComputePipelineDesc;
use skia_rust_gpu::graphite::context_options::{Callback, ContextOptions};
use skia_rust_gpu::graphite::context_utils::build_compute_sksl;
use skia_rust_gpu::graphite::draw_types::{CompareOp, PrimitiveType};
use skia_rust_gpu::graphite::graphics_pipeline::{
    GraphicsPipeline, GraphicsPipelineBase, PipelineCreationFlags,
};
use skia_rust_gpu::graphite::graphics_pipeline_desc::GraphicsPipelineDesc;
use skia_rust_gpu::graphite::graphics_pipeline_desc::GraphicsPipelineHandle;
use skia_rust_gpu::graphite::graphite_types::SampleCount;
use skia_rust_gpu::graphite::key_context::KeyContext;
use skia_rust_gpu::graphite::paint_params::{PaintParams, ShadingParams};
use skia_rust_gpu::graphite::paint_params_key::PaintParamsKeyBuilder;
use skia_rust_gpu::graphite::pipeline_data::PipelineDataGatherer;
use skia_rust_gpu::graphite::pipeline_manager::PipelineCreationContext;
use skia_rust_gpu::graphite::recorder::RecorderSharedContext;
use skia_rust_gpu::graphite::render_pass_desc::{AttachmentDesc, RenderPassDesc};
use skia_rust_gpu::graphite::render_step::{RenderStep, RenderStepID};
use skia_rust_gpu::graphite::resource_types::{DstReadStrategy, Layout, LoadOp, StoreOp};
use skia_rust_gpu::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use skia_rust_gpu::graphite::shader_code_dictionary::ShaderCodeDictionary;
use skia_rust_gpu::graphite::shared_context::SharedContext;
use skia_rust_gpu::graphite::texture_format::TextureFormat;
use skia_rust_gpu::graphite::unique_paint_params_id::UniquePaintParamsID;
use skia_rust_gpu::graphite::wgpu::compute_pipeline::WgpuComputePipeline;
use skia_rust_gpu::graphite::wgpu::error_checker::{ErrorChecker, ErrorType};
use skia_rust_gpu::graphite::wgpu::graphics_pipeline::{
    APPEND_DATA_BUFFER_INDEX, STATIC_DATA_BUFFER_INDEX, WgpuGraphicsPipeline,
    blend_coeff_to_wgpu_blend, blend_coeff_to_wgpu_blend_for_alpha,
    blend_equation_to_wgpu_blend_op,
};
use skia_rust_gpu::graphite::wgpu::graphite_utils::compile_wgsl_shader_module;
use skia_rust_gpu::graphite::wgpu::{
    CapsProfile, DeviceFeatures, WgpuCaps, WgpuSharedContext, make_context,
    noop_backend_context_with_features,
};
use support::wgsl_corpus::{all_steps, corpus_paints, render_pass_desc};

/// The Dawn Vulkan profile without `ShaderF16`: the shaders are f32, as on the D3D12 tier.
///
/// wgpu 30 does not accept the f16 shaders Graphite makes when `ShaderF16` is on: they write
/// `half4` fragment colors, and wgpu's pipeline validation rejects an `f16` output for an
/// `Rgba8Unorm` target ("Output format Float32x4 is incompatible with the shader Float16x4"),
/// where Dawn accepts it. So on a real device with `SHADER_F16` the pipelines would not be
/// created; see the G11b report.
fn vulkan() -> CapsProfile {
    let mut profile = CapsProfile::dawn_vulkan();
    profile.features -= DeviceFeatures::SHADER_F16;
    profile
}

/// A shader error handler that records the failures.
#[derive(Default)]
struct Recorded(Mutex<Vec<String>>);

impl ShaderErrorHandler for Recorded {
    fn compile_error(&self, shader: &str, errors: &str, _was_cached: bool) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(build_shader_error_message(shader, errors));
    }
}

/// An executor that queues the work until a test runs it: the lowest work list first, in
/// submission order, as `SkThreadPool` with two work lists does.
#[derive(Default)]
struct ManualExecutor {
    queue: Mutex<Vec<(i32, skia_rust_core::executor::Work)>>,
}

impl ManualExecutor {
    fn run_next(&self) -> bool {
        let next = {
            let mut queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(index) = queue
                .iter()
                .enumerate()
                .min_by_key(|(i, (list, _))| (*list, *i))
                .map(|(i, _)| i)
            else {
                return false;
            };
            queue.remove(index)
        };
        (next.1)();
        true
    }

    fn pending_work_lists(&self) -> Vec<i32> {
        self.queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|(list, _)| *list)
            .collect()
    }
}

impl Executor for ManualExecutor {
    fn add_to_work_list(&self, work: skia_rust_core::executor::Work, work_list: i32) {
        self.queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((work_list, work));
    }

    // `TaskGroup::wait` borrows the executor's thread until the work is done.
    fn borrow(&self) {
        self.run_next();
    }
}

/// A shared context on a noop device that has the features `profile` needs, with the renderer
/// provider set.
fn shared_context(profile: &CapsProfile, options: &ContextOptions) -> Arc<WgpuSharedContext> {
    let mut features = wgpu::Features::IMMEDIATES;
    if profile.features.contains(DeviceFeatures::SHADER_F16) {
        features |= wgpu::Features::SHADER_F16;
    }
    if profile
        .features
        .contains(DeviceFeatures::DUAL_SOURCE_BLENDING)
    {
        features |= wgpu::Features::DUAL_SOURCE_BLENDING;
    }
    let limits = wgpu::Limits {
        max_immediate_size: 64,
        ..wgpu::Limits::default()
    };
    let backend_context = noop_backend_context_with_features(features, limits).unwrap();
    WgpuSharedContext::make_with_profile(&backend_context, profile, options).unwrap()
}

fn options_with(handler: &Arc<Recorded>, executor: Option<Arc<dyn Executor>>) -> ContextOptions {
    let handler: Arc<dyn ShaderErrorHandler> = handler.clone();
    ContextOptions {
        shader_error_handler: Some(Callback(handler)),
        executor: executor.map(Callback),
        ..ContextOptions::default()
    }
}

fn srgb_info() -> ColorInfo {
    ColorInfo::new(
        ColorType::RGBA8888,
        AlphaType::Premul,
        Some(ColorSpace::new_srgb()),
    )
}

/// What `Recorder::snap` + `DrawPass` do to make a pipeline description for a paint and a step:
/// keys the paint into the shared context's dictionary.
fn paint_id(
    shared: &WgpuSharedContext,
    paint: &Paint,
    step: &dyn RenderStep,
    rp_desc: &mut RenderPassDesc,
) -> (UniquePaintParamsID, Arc<RuntimeEffectDictionary>) {
    let caps = shared.caps();
    let dict = shared.shader_code_dictionary();
    let layout = caps.resource_binding_requirements().uniform_buffer_layout;
    let builder = RefCell::new(PaintParamsKeyBuilder::new(dict));
    let gatherer = RefCell::new(PipelineDataGatherer::new(layout));
    let rte_dict = Arc::new(RuntimeEffectDictionary::new());
    let info = srgb_info();
    let key_caps: Arc<dyn Caps> = caps.clone();
    let context = KeyContext::new(key_caps, &builder, &gatherer, dict, rte_dict.clone(), &info);
    let params = PaintParams::new(paint, None, false, false);
    let shading = ShadingParams::new(
        &**caps,
        &params,
        None,
        None,
        step.coverage(),
        rp_desc.color_attachment.format,
    );
    let (id, dst_usage) = shading.to_key(&context).unwrap_or((
        UniquePaintParamsID::invalid(),
        skia_rust_gpu::graphite::draw_types::DstUsage::NONE,
    ));
    rp_desc.dst_read_strategy =
        if dst_usage.contains(skia_rust_gpu::graphite::draw_types::DstUsage::DST_READ_REQUIRED) {
            caps.get_dst_read_strategy()
        } else {
            DstReadStrategy::NoneRequired
        };
    (id, rte_dict)
}

/// The paints of the slice: solid colors in the blend modes that pick different blend states
/// (hardware blending with and without dst coefficients, shader blending), a gradient and a
/// runtime shader.
fn slice_paints(storage_buffers: bool) -> Vec<(String, Paint)> {
    const NAMES: [&str; 6] = [
        "solid-Src",
        "solid-SrcOver",
        "solid-Multiply",
        "solid-opaque-Src",
        "gradient-linear5",
        "runtime-shader",
    ];
    corpus_paints(storage_buffers)
        .into_iter()
        .filter(|(name, _)| NAMES.contains(&name.as_str()))
        .collect()
}

/// The 4x MSAA render pass of the Dawn path: a multisampled color attachment with its resolve
/// attachment and a depth/stencil attachment.
fn msaa_render_pass_desc(caps: &WgpuCaps) -> RenderPassDesc {
    let mut desc = render_pass_desc(caps, false);
    desc.sample_count = SampleCount::Four;
    desc.color_attachment = AttachmentDesc {
        format: TextureFormat::RGBA8,
        load_op: LoadOp::Clear,
        store_op: StoreOp::Discard,
        sample_count: SampleCount::Four,
    };
    desc.color_resolve_attachment = AttachmentDesc {
        format: TextureFormat::RGBA8,
        load_op: LoadOp::Clear,
        store_op: StoreOp::Store,
        sample_count: SampleCount::One,
    };
    desc.depth_stencil_attachment = AttachmentDesc {
        format: TextureFormat::D24_S8,
        load_op: LoadOp::Clear,
        store_op: StoreOp::Discard,
        sample_count: SampleCount::Four,
    };
    desc
}

fn pipeline_of(
    handle: &GraphicsPipelineHandle,
    shared: &WgpuSharedContext,
) -> Arc<dyn GraphicsPipeline> {
    shared
        .resolve_pipeline_handle(handle)
        .expect("the pipeline compiled")
}

fn wgpu_pipeline(pipeline: &Arc<dyn GraphicsPipeline>) -> &WgpuGraphicsPipeline {
    pipeline
        .as_any()
        .downcast_ref::<WgpuGraphicsPipeline>()
        .expect("a wgpu pipeline")
}

fn step_named(name: &str, shared: &WgpuSharedContext) -> Arc<dyn RenderStep> {
    let provider = RecorderSharedContext::renderer_provider(shared);
    all_steps(provider)
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no step {name}"))
        .1
}

// ---------------------------------------------------------------------------------------------
// makeGraphicsPipelineKey
// ---------------------------------------------------------------------------------------------

#[test]
fn the_graphics_pipeline_key_is_four_words_in_one_domain() {
    let profile = vulkan();
    let caps = WgpuCaps::new(&profile, &ContextOptions::default());
    let mut rp_desc = render_pass_desc(&caps, false);
    rp_desc.write_swizzle = skia_rust_gpu::gpu::swizzle::Swizzle::bgra();
    let desc = GraphicsPipelineDesc::new(RenderStepID::CircularArc, UniquePaintParamsID::new(7));

    let key = caps.make_graphics_pipeline_key(&desc, &rp_desc);

    assert!(key.is_valid());
    assert_eq!(key.tag(), Some("DawnGraphicsPipeline"));
    assert_eq!(
        key.data(),
        &[
            RenderStepID::CircularArc as u32,
            7,
            caps.get_render_pass_desc_key_for_pipeline(&rp_desc, false),
            u32::from(rp_desc.write_swizzle.as_key()),
        ]
    );
    // Equal descriptions make equal keys, and each input changes the key.
    assert_eq!(key, caps.make_graphics_pipeline_key(&desc, &rp_desc));
    let other_step =
        GraphicsPipelineDesc::new(RenderStepID::AnalyticRRect, UniquePaintParamsID::new(7));
    let other_paint =
        GraphicsPipelineDesc::new(RenderStepID::CircularArc, UniquePaintParamsID::new(8));
    assert_ne!(key, caps.make_graphics_pipeline_key(&other_step, &rp_desc));
    assert_ne!(key, caps.make_graphics_pipeline_key(&other_paint, &rp_desc));
    let mut other_rp = rp_desc.clone();
    other_rp.write_swizzle = skia_rust_gpu::gpu::swizzle::Swizzle::rgba();
    assert_ne!(key, caps.make_graphics_pipeline_key(&desc, &other_rp));
    assert_ne!(
        key,
        caps.make_graphics_pipeline_key(&desc, &msaa_render_pass_desc(&caps))
    );
}

// ---------------------------------------------------------------------------------------------
// The blend mapping
// ---------------------------------------------------------------------------------------------

#[test]
fn blend_coefficients_map_as_in_dawn_graphics_pipeline() {
    use wgpu::BlendFactor as F;
    let with_dsb = WgpuCaps::new(&vulkan(), &ContextOptions::default());
    assert!(with_dsb.shader_caps().dual_source_blending_support);
    let mut no_dsb_profile = vulkan();
    no_dsb_profile.features -= DeviceFeatures::DUAL_SOURCE_BLENDING;
    let without_dsb = WgpuCaps::new(&no_dsb_profile, &ContextOptions::default());
    assert!(!without_dsb.shader_caps().dual_source_blending_support);

    let color = [
        (BlendCoeff::Zero, F::Zero),
        (BlendCoeff::One, F::One),
        (BlendCoeff::SC, F::Src),
        (BlendCoeff::ISC, F::OneMinusSrc),
        (BlendCoeff::DC, F::Dst),
        (BlendCoeff::IDC, F::OneMinusDst),
        (BlendCoeff::SA, F::SrcAlpha),
        (BlendCoeff::ISA, F::OneMinusSrcAlpha),
        (BlendCoeff::DA, F::DstAlpha),
        (BlendCoeff::IDA, F::OneMinusDstAlpha),
        (BlendCoeff::ConstC, F::Constant),
        (BlendCoeff::IConstC, F::OneMinusConstant),
        (BlendCoeff::Illegal, F::Zero),
    ];
    for (coeff, expected) in color {
        assert_eq!(blend_coeff_to_wgpu_blend(&with_dsb, coeff), expected);
        assert_eq!(blend_coeff_to_wgpu_blend(&without_dsb, coeff), expected);
    }
    // The second source needs dual-source blending, and is zero without it.
    for (coeff, expected) in [
        (BlendCoeff::S2C, F::Src1),
        (BlendCoeff::IS2C, F::OneMinusSrc1),
        (BlendCoeff::S2A, F::Src1Alpha),
        (BlendCoeff::IS2A, F::OneMinusSrc1Alpha),
    ] {
        assert_eq!(blend_coeff_to_wgpu_blend(&with_dsb, coeff), expected);
        assert_eq!(blend_coeff_to_wgpu_blend(&without_dsb, coeff), F::Zero);
    }

    // The alpha channel uses the alpha version of the source and dest color coefficients.
    for (coeff, expected) in [
        (BlendCoeff::SC, F::SrcAlpha),
        (BlendCoeff::ISC, F::OneMinusSrcAlpha),
        (BlendCoeff::DC, F::DstAlpha),
        (BlendCoeff::IDC, F::OneMinusDstAlpha),
        (BlendCoeff::One, F::One),
        (BlendCoeff::ISA, F::OneMinusSrcAlpha),
        (BlendCoeff::S2C, F::Src1),
    ] {
        assert_eq!(
            blend_coeff_to_wgpu_blend_for_alpha(&with_dsb, coeff),
            expected
        );
    }

    assert_eq!(
        blend_equation_to_wgpu_blend_op(BlendEquation::Add),
        wgpu::BlendOperation::Add
    );
    assert_eq!(
        blend_equation_to_wgpu_blend_op(BlendEquation::Subtract),
        wgpu::BlendOperation::Subtract
    );
    assert_eq!(
        blend_equation_to_wgpu_blend_op(BlendEquation::ReverseSubtract),
        wgpu::BlendOperation::ReverseSubtract
    );
}

// ---------------------------------------------------------------------------------------------
// Graphics pipelines from the corpus slice
// ---------------------------------------------------------------------------------------------

/// Creates the pipelines of the slice on `profile`, checking each against the mapping of
/// `DawnGraphicsPipeline::Make`. Returns how many were created.
// One long list of assertions, one per field of the pipeline state.
#[allow(clippy::too_many_lines)]
fn check_corpus_slice(profile: &CapsProfile, msaa: bool) -> usize {
    let handler = Arc::new(Recorded::default());
    let shared = shared_context(profile, &options_with(&handler, None));
    let caps = shared.caps().clone();
    let provider = RecorderSharedContext::renderer_provider(&*shared);
    let steps = all_steps(provider);
    let paints = slice_paints(caps.storage_buffer_support());
    let mut created = 0;
    let mut failed = Vec::new();

    for (paint_name, paint) in &paints {
        for (step_name, step) in &steps {
            let mut rp_desc = if msaa {
                msaa_render_pass_desc(&caps)
            } else {
                render_pass_desc(&caps, false)
            };
            let (id, rte_dict) = paint_id(&shared, paint, &**step, &mut rp_desc);
            let desc = GraphicsPipelineDesc::new(step.render_step_id(), id);

            let handle = shared.create_pipeline_handle(
                Some(rte_dict),
                &desc,
                &rp_desc,
                PipelineCreationFlags::NONE,
            );
            let Some(pipeline) = shared.resolve_pipeline_handle(&handle) else {
                failed.push(format!("{}/{paint_name}/{step_name}", profile.name));
                continue;
            };
            created += 1;
            let wgpu_pipeline = wgpu_pipeline(&pipeline);
            let state = wgpu_pipeline.state();

            // Always labeled, with the label of the ShaderInfo.
            assert_eq!(pipeline.label(), state.label);
            assert!(
                pipeline.label().contains(step.name()),
                "{}",
                pipeline.label()
            );

            // The color target: the render pass's format; no color writes without a fragment
            // shader; a blend state exactly when the blend is not Add(One, Zero).
            assert_eq!(state.color_target.format, wgpu::TextureFormat::Rgba8Unorm);
            assert_eq!(
                state.color_target.write_mask,
                if state.has_fragment_shader {
                    wgpu::ColorWrites::ALL
                } else {
                    wgpu::ColorWrites::empty()
                }
            );

            // The vertex buffers: the static buffer at slot 0 from location 0, the append buffer
            // at slot 1 from the location after the static attributes.
            let static_attrs = step.static_attributes();
            let static_buffer = &state.vertex_buffers[STATIC_DATA_BUFFER_INDEX];
            assert_eq!(static_buffer.is_some(), !static_attrs.is_empty());
            if let Some(buffer) = static_buffer {
                assert_eq!(
                    buffer.array_stride,
                    static_attrs
                        .iter()
                        .map(|a| a.size_align4() as u64)
                        .sum::<u64>()
                );
                assert_eq!(buffer.step_mode, wgpu::VertexStepMode::Vertex);
                let locations: Vec<u32> = buffer
                    .attributes
                    .iter()
                    .map(|a| a.shader_location)
                    .collect();
                assert_eq!(
                    locations,
                    (0..u32::try_from(static_attrs.len()).unwrap()).collect::<Vec<_>>()
                );
                let mut offset = 0;
                for (attr, wgpu_attr) in static_attrs.iter().zip(&buffer.attributes) {
                    assert_eq!(wgpu_attr.offset, offset);
                    offset += attr.size_align4() as u64;
                }
            }
            if let Some(buffer) = &state.vertex_buffers[APPEND_DATA_BUFFER_INDEX] {
                assert_eq!(
                    buffer.step_mode,
                    if step.appends_vertices() {
                        wgpu::VertexStepMode::Vertex
                    } else {
                        wgpu::VertexStepMode::Instance
                    }
                );
                assert_eq!(
                    buffer.attributes[0].shader_location as usize,
                    static_attrs.len()
                );
                assert_eq!(
                    buffer.array_stride,
                    buffer.attributes.last().map_or(0, |last| last.offset)
                        + last_format_size(buffer.attributes.last())
                );
            }

            // The primitive state.
            let (topology, strip) = match step.base().primitive_type() {
                PrimitiveType::Triangles => (wgpu::PrimitiveTopology::TriangleList, None),
                PrimitiveType::TriangleStrip => (
                    wgpu::PrimitiveTopology::TriangleStrip,
                    Some(wgpu::IndexFormat::Uint16),
                ),
                PrimitiveType::Points => (wgpu::PrimitiveTopology::PointList, None),
            };
            assert_eq!(state.primitive.topology, topology);
            assert_eq!(state.primitive.strip_index_format, strip);
            assert_eq!(state.primitive.front_face, wgpu::FrontFace::Ccw);
            assert_eq!(state.primitive.cull_mode, None);
            assert_eq!(wgpu_pipeline.primitive_type(), step.base().primitive_type());

            // The multisample state.
            assert_eq!(state.multisample.count, rp_desc.sample_count as u32);
            assert_eq!(state.multisample.mask, 0xFFFF_FFFF);
            assert!(!state.multisample.alpha_to_coverage_enabled);

            // The depth/stencil state.
            let settings = step.base().depth_stencil_settings();
            assert_eq!(
                wgpu_pipeline.stencil_reference_value(),
                settings.stencil_reference_value
            );
            if msaa {
                let ds = state
                    .depth_stencil
                    .as_ref()
                    .expect("a depth/stencil attachment");
                assert_eq!(ds.format, wgpu::TextureFormat::Depth24PlusStencil8);
                assert_eq!(
                    ds.depth_write_enabled,
                    Some(settings.depth_test_enabled && settings.depth_write_enabled)
                );
                assert_eq!(
                    ds.depth_compare,
                    Some(match settings.depth_compare_op {
                        CompareOp::Always => wgpu::CompareFunction::Always,
                        CompareOp::Less => wgpu::CompareFunction::Less,
                        CompareOp::LEqual => wgpu::CompareFunction::LessEqual,
                        CompareOp::Greater => wgpu::CompareFunction::Greater,
                        other => panic!("no step uses {other:?}"),
                    })
                );
                assert_eq!(ds.stencil.is_enabled(), settings.stencil_test_enabled);
            } else {
                assert!(state.depth_stencil.is_none());
            }

            // The layouts: the uniform buffers group always; the textures group iff the
            // fragment shader samples.
            let layouts = wgpu_pipeline.group_layouts();
            assert!(layouts[0].is_some());
            assert_eq!(
                layouts[1].is_some(),
                state.has_fragment_shader && pipeline.num_frag_textures_and_samplers() > 0
            );
            // The profile's push constants carry the intrinsic uniforms.
            assert_eq!(state.immediate_size, 32);
        }
    }
    assert!(
        handler.0.lock().unwrap().is_empty() || !failed.is_empty(),
        "shader errors without a failed pipeline"
    );
    // naga cannot compile the shaders that pass a pointer to a storage buffer array as a
    // function argument (`docs/design/gpu.md` §6.3 W4, `KNOWN_NAGA_LIMITATION`); every other
    // pipeline must be created.
    for message in handler.0.lock().unwrap().iter() {
        assert!(
            message.contains("pointer") || message.contains("InvalidArgumentPointerSpace"),
            "unexpected shader error: {message}"
        );
    }
    assert!(
        failed.len() * 10 <= created,
        "{} of {} pipelines failed: {failed:?}",
        failed.len(),
        created + failed.len()
    );
    created
}

/// The byte size of the vertex format of the last attribute of a buffer.
fn last_format_size(attribute: Option<&wgpu::VertexAttribute>) -> u64 {
    attribute.map_or(0, |a| a.format.size())
}

#[test]
fn the_vulkan_profile_slice_creates_pipelines() {
    // F16 shaders, dual-source blending, no storage buffers.
    let created = check_corpus_slice(&vulkan(), false);
    assert!(created > 100, "{created}");
}

#[test]
fn the_vulkan_profile_slice_creates_msaa_pipelines() {
    let created = check_corpus_slice(&vulkan(), true);
    assert!(created > 100, "{created}");
}

#[test]
fn the_d3d12_profile_slice_creates_pipelines() {
    // F32 shaders, storage buffers.
    let created = check_corpus_slice(&CapsProfile::dawn_d3d12(), false);
    assert!(created > 100, "{created}");
}

#[test]
fn the_wgpu_restricted_profile_slice_creates_pipelines() {
    let created = check_corpus_slice(&vulkan().wgpu_restricted(), false);
    assert!(created > 100, "{created}");
}

#[test]
fn the_blend_state_follows_the_blend_info() {
    use wgpu::BlendFactor as F;
    let handler = Arc::new(Recorded::default());
    let shared = shared_context(&vulkan(), &options_with(&handler, None));
    let caps = shared.caps().clone();
    let step = step_named("non_aa_bounds_fill[0]", &shared);
    let paints = corpus_paints(false);
    let state_of = |paint_name: &str| {
        let paint = &paints.iter().find(|(n, _)| n == paint_name).unwrap().1;
        let mut rp_desc = render_pass_desc(&caps, false);
        let (id, dict) = paint_id(&shared, paint, &*step, &mut rp_desc);
        let desc = GraphicsPipelineDesc::new(step.render_step_id(), id);
        let handle =
            shared.create_pipeline_handle(Some(dict), &desc, &rp_desc, PipelineCreationFlags::NONE);
        let pipeline = pipeline_of(&handle, &shared);
        wgpu_pipeline(&pipeline).state().clone()
    };

    // Src of an opaque color: Add(One, Zero) is not blended.
    assert_eq!(state_of("solid-opaque-Src").color_target.blend, None);
    // SrcOver of a translucent color: One, OneMinusSrcAlpha for color and alpha.
    let src_over = state_of("solid-SrcOver").color_target.blend.unwrap();
    assert_eq!(src_over.color.operation, wgpu::BlendOperation::Add);
    assert_eq!(src_over.color.src_factor, F::One);
    assert_eq!(src_over.color.dst_factor, F::OneMinusSrcAlpha);
    assert_eq!(src_over.alpha.operation, wgpu::BlendOperation::Add);
    assert_eq!(src_over.alpha.src_factor, F::One);
    assert_eq!(src_over.alpha.dst_factor, F::OneMinusSrcAlpha);
    // Src of a translucent color is also not blended.
    assert_eq!(state_of("solid-Src").color_target.blend, None);
}

#[test]
fn the_cover_step_pipeline_has_the_depth_pass_of_its_step() {
    let handler = Arc::new(Recorded::default());
    let shared = shared_context(&vulkan(), &options_with(&handler, None));
    let caps = shared.caps().clone();
    let paints = corpus_paints(false);
    let paint = &paints.iter().find(|(n, _)| n == "solid-SrcOver").unwrap().1;

    let make = |step_name: &str| {
        let step = step_named(step_name, &shared);
        let mut rp_desc = msaa_render_pass_desc(&caps);
        let (id, dict) = paint_id(&shared, paint, &*step, &mut rp_desc);
        let desc = GraphicsPipelineDesc::new(step.render_step_id(), id);
        let handle =
            shared.create_pipeline_handle(Some(dict), &desc, &rp_desc, PipelineCreationFlags::NONE);
        let pipeline = pipeline_of(&handle, &shared);
        wgpu_pipeline(&pipeline).state().clone()
    };

    // `kDirectDepthLessPass`: depth test and write, Less, no stencil.
    let direct = make("non_aa_bounds_fill[0]");
    let ds = direct.depth_stencil.unwrap();
    assert_eq!(ds.depth_write_enabled, Some(true));
    assert_eq!(ds.depth_compare, Some(wgpu::CompareFunction::Less));
    assert!(!ds.stencil.is_enabled());
    assert_eq!(ds.stencil.read_mask, 0xFFFF_FFFF);
    assert_eq!(ds.stencil.write_mask, 0xFFFF_FFFF);
    assert_eq!(
        direct.primitive.topology,
        wgpu::PrimitiveTopology::TriangleStrip
    );
    assert_eq!(
        direct.primitive.strip_index_format,
        Some(wgpu::IndexFormat::Uint16)
    );

    // `kWindingStencilPass`: stencil increment (front) and decrement (back) with wrap, depth
    // test (Less) without a depth write; triangles.
    let winding = make("stencil_wedges_Winding[0]");
    let ds = winding.depth_stencil.unwrap();
    assert_eq!(ds.depth_write_enabled, Some(false));
    assert_eq!(ds.depth_compare, Some(wgpu::CompareFunction::Less));
    assert_eq!(
        ds.stencil.front.pass_op,
        wgpu::StencilOperation::IncrementWrap
    );
    assert_eq!(
        ds.stencil.back.pass_op,
        wgpu::StencilOperation::DecrementWrap
    );
    assert_eq!(ds.stencil.front.compare, wgpu::CompareFunction::Always);
    assert_eq!(ds.stencil.front.fail_op, wgpu::StencilOperation::Keep);
    assert_eq!(ds.stencil.front.depth_fail_op, wgpu::StencilOperation::Keep);
    assert_eq!(
        (ds.stencil.read_mask, ds.stencil.write_mask),
        (0xFFFF_FFFF, 0xFFFF_FFFF)
    );
    assert_eq!(
        winding.primitive.topology,
        wgpu::PrimitiveTopology::TriangleList
    );
    assert_eq!(winding.primitive.strip_index_format, None);

    // `kEvenOddStencilPass`: Invert with a write mask of 1.
    let even_odd = make("stencil_wedges_EvenOdd[0]");
    let ds = even_odd.depth_stencil.unwrap();
    assert_eq!(ds.stencil.front.pass_op, wgpu::StencilOperation::Invert);
    assert_eq!(
        (ds.stencil.read_mask, ds.stencil.write_mask),
        (0xFFFF_FFFF, 1)
    );

    // `kRegularCoverPass`: the stencil test passes non-zero, clearing it.
    let cover = make("stencil_wedges_Winding[1]");
    let ds = cover.depth_stencil.unwrap();
    assert_eq!(ds.depth_write_enabled, Some(true));
    assert_eq!(ds.stencil.front.compare, wgpu::CompareFunction::NotEqual);
    assert_eq!(ds.stencil.front.fail_op, wgpu::StencilOperation::Keep);
    assert_eq!(ds.stencil.front.depth_fail_op, wgpu::StencilOperation::Zero);
    assert_eq!(ds.stencil.front.pass_op, wgpu::StencilOperation::Zero);
}

#[test]
fn the_circular_arc_pipeline_has_a_static_and_an_instanced_buffer() {
    let handler = Arc::new(Recorded::default());
    let shared = shared_context(&vulkan(), &options_with(&handler, None));
    let caps = shared.caps().clone();
    let step = step_named("circular_arc[0]", &shared);
    let paints = corpus_paints(false);
    let paint = &paints.iter().find(|(n, _)| n == "solid-SrcOver").unwrap().1;
    let mut rp_desc = render_pass_desc(&caps, false);
    let (id, dict) = paint_id(&shared, paint, &*step, &mut rp_desc);
    let desc = GraphicsPipelineDesc::new(RenderStepID::CircularArc, id);

    let handle =
        shared.create_pipeline_handle(Some(dict), &desc, &rp_desc, PipelineCreationFlags::NONE);
    let pipeline = pipeline_of(&handle, &shared);
    let state = wgpu_pipeline(&pipeline).state();

    // One `float3 position` per vertex.
    let static_buffer = state.vertex_buffers[STATIC_DATA_BUFFER_INDEX]
        .as_ref()
        .unwrap();
    assert_eq!(static_buffer.array_stride, 12);
    assert_eq!(static_buffer.step_mode, wgpu::VertexStepMode::Vertex);
    assert_eq!(
        static_buffer.attributes,
        vec![wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x3,
            offset: 0,
            shader_location: 0,
        }]
    );
    // The instance data: 12 attributes starting at location 1, 128 bytes.
    let append = state.vertex_buffers[APPEND_DATA_BUFFER_INDEX]
        .as_ref()
        .unwrap();
    assert_eq!(append.step_mode, wgpu::VertexStepMode::Instance);
    assert_eq!(append.array_stride, 128);
    assert_eq!(append.attributes.len(), 12);
    assert_eq!(append.attributes[0].shader_location, 1);
    assert_eq!(append.attributes[0].format, wgpu::VertexFormat::Float32x4);
    assert_eq!(append.attributes[1].offset, 16);
    assert_eq!(append.attributes[8].format, wgpu::VertexFormat::Uint32);
}

// ---------------------------------------------------------------------------------------------
// PipelineManager
// ---------------------------------------------------------------------------------------------

/// A description of a pipeline of the slice: a solid color drawn with the step.
fn solid_desc(
    shared: &WgpuSharedContext,
    step_name: &str,
) -> (
    GraphicsPipelineDesc,
    RenderPassDesc,
    Arc<RuntimeEffectDictionary>,
) {
    let step = step_named(step_name, shared);
    let paints = corpus_paints(false);
    let paint = &paints.iter().find(|(n, _)| n == "solid-SrcOver").unwrap().1;
    let mut rp_desc = render_pass_desc(shared.caps(), false);
    let (id, dict) = paint_id(shared, paint, &*step, &mut rp_desc);
    (
        GraphicsPipelineDesc::new(step.render_step_id(), id),
        rp_desc,
        dict,
    )
}

fn same_pipeline(a: &Arc<dyn GraphicsPipeline>, b: &Arc<dyn GraphicsPipeline>) -> bool {
    std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b))
}

#[test]
fn without_an_executor_the_manager_compiles_in_line() {
    let handler = Arc::new(Recorded::default());
    let shared = shared_context(&vulkan(), &options_with(&handler, None));
    let manager = shared.base().pipeline_manager();
    let (desc, rp_desc, dict) = solid_desc(&shared, "analytic_rrect[0]");

    let handle = shared.create_pipeline_handle(
        Some(dict.clone()),
        &desc,
        &rp_desc,
        PipelineCreationFlags::NONE,
    );

    // The task ran before `createHandle` returned.
    let pipeline = handle.pipeline_or_null().expect("compiled in-line");
    assert_eq!(manager.get_stats().num_tasks_created, 1);
    assert_eq!(manager.get_stats().num_preemptively_found_tasks, 0);
    assert_eq!(manager.num_active_tasks(), 0);
    assert_eq!(shared.base().global_cache().num_graphics_pipelines(), 1);
    assert!(pipeline.did_async_compilation_fail().is_none());
    assert!(!pipeline.from_precompile());
    // The pipeline records the key it was made for.
    let key = shared.caps().make_graphics_pipeline_key(&desc, &rp_desc);
    assert_eq!(pipeline.base().unique_key_hash(), key.hash());
    let info = pipeline.base().pipeline_info();
    assert_eq!(info.dst_read_strategy, rp_desc.dst_read_strategy);
    assert!(info.sksl_vertex_shader.contains("devPosition"));
    assert!(info.native_vertex_shader.contains("fn main"));
    assert!(info.native_fragment_shader.contains("fn main"));

    // A second request finds the pipeline in the global cache: no task.
    let again =
        shared.create_pipeline_handle(Some(dict), &desc, &rp_desc, PipelineCreationFlags::NONE);
    assert_eq!(manager.get_stats().num_tasks_created, 1);
    assert!(same_pipeline(&pipeline, &pipeline_of(&again, &shared)));
    assert!(same_pipeline(&pipeline, &pipeline_of(&handle, &shared)));
}

#[test]
fn the_recorder_side_factory_creates_and_resolves_handles() {
    // The `PipelineHandleFactory` a `DrawPass` gets from `RecorderSharedContext::pipeline_manager`.
    let handler = Arc::new(Recorded::default());
    let shared = shared_context(&vulkan(), &options_with(&handler, None));
    let factory = RecorderSharedContext::pipeline_manager(&*shared).expect("a factory");
    let (desc, rp_desc, dict) = solid_desc(&shared, "analytic_rrect[0]");

    let handle = factory.create_handle(Some(&dict), &desc, &rp_desc, PipelineCreationFlags::NONE);

    let pipeline = factory.resolve_handle(&handle).expect("compiled");
    assert!(same_pipeline(
        &pipeline,
        &handle.pipeline_or_null().unwrap()
    ));
    assert_eq!(
        shared
            .base()
            .pipeline_manager()
            .get_stats()
            .num_tasks_created,
        1
    );
}

#[test]
fn a_failed_compilation_resolves_to_no_pipeline() {
    let handler = Arc::new(Recorded::default());
    let shared = shared_context(&vulkan(), &options_with(&handler, None));
    let manager = shared.base().pipeline_manager();
    let (_, rp_desc, dict) = solid_desc(&shared, "analytic_rrect[0]");
    // The renderer provider has no sparse strips step (`EndCap`, G17), so no pipeline can be made.
    let desc = GraphicsPipelineDesc::new(RenderStepID::EndCap, UniquePaintParamsID::new(1));

    let handle =
        shared.create_pipeline_handle(Some(dict), &desc, &rp_desc, PipelineCreationFlags::NONE);

    assert!(handle.pipeline_or_null().is_none());
    assert!(shared.resolve_pipeline_handle(&handle).is_none());
    assert_eq!(manager.num_active_tasks(), 0);
    assert_eq!(shared.base().global_cache().num_graphics_pipelines(), 0);
}

#[test]
fn a_half_precision_fragment_output_is_a_creation_failure_on_wgpu() {
    // The Dawn Vulkan profile has `ShaderF16`, so the fragment shader writes a `half4`. Dawn takes
    // it for an `Rgba8Unorm` target; wgpu 30's pipeline validation does not. The failure is a
    // `None` from the creation (the error scopes catch it), not a panic. If wgpu learns to accept
    // f16 outputs this test starts failing, and `vulkan()` can go.
    let handler = Arc::new(Recorded::default());
    let profile = CapsProfile::dawn_vulkan();
    assert!(profile.features.contains(DeviceFeatures::SHADER_F16));
    let shared = shared_context(&profile, &options_with(&handler, None));
    let (desc, rp_desc, dict) = solid_desc(&shared, "analytic_rrect[0]");

    let handle =
        shared.create_pipeline_handle(Some(dict), &desc, &rp_desc, PipelineCreationFlags::NONE);

    assert!(shared.resolve_pipeline_handle(&handle).is_none());
    assert_eq!(shared.base().global_cache().num_graphics_pipelines(), 0);
    // The shaders themselves compiled: this is not a shader error.
    assert!(handler.0.lock().unwrap().is_empty());
}

#[test]
fn a_thread_pool_compiles_the_tasks_and_handles_resolve() {
    let handler = Arc::new(Recorded::default());
    let pool: Arc<dyn Executor> = Arc::new(ThreadPool::new(WorkOrder::Fifo, 2, 2, false));
    let shared = shared_context(&vulkan(), &options_with(&handler, Some(pool)));
    let manager = shared.base().pipeline_manager();
    let names = [
        "analytic_rrect[0]",
        "circular_arc[0]",
        "per_edge_aa_quad[0]",
        "non_aa_bounds_fill[0]",
        "vertices_c1_t0[0]",
        "stencil_wedges_Winding[0]",
    ];

    let queued: Vec<_> = names
        .iter()
        .map(|name| {
            let (desc, rp_desc, dict) = solid_desc(&shared, name);
            shared.create_pipeline_handle(Some(dict), &desc, &rp_desc, PipelineCreationFlags::NONE)
        })
        .collect();
    // Resolving waits for the compilation, or preempts it.
    let pipelines: Vec<_> = queued
        .iter()
        .map(|handle| pipeline_of(handle, &shared))
        .collect();
    manager.wait_test_only();

    assert_eq!(
        manager.get_stats().num_tasks_created,
        i32::try_from(names.len()).unwrap()
    );
    assert_eq!(manager.num_active_tasks(), 0);
    assert_eq!(
        shared.base().global_cache().num_graphics_pipelines(),
        names.len()
    );
    for (handle, pipeline) in queued.iter().zip(&pipelines) {
        assert!(same_pipeline(&handle.pipeline_or_null().unwrap(), pipeline));
    }
    for pipeline in &pipelines {
        assert_ne!(pipeline.label(), "");
    }

    // Shutting down waits for the work and stops threading: later handles compile in-line.
    manager.shut_down();
    let (desc, rp_desc, dict) = solid_desc(&shared, "circular_arc[0]");
    let mut other_rp = rp_desc.clone();
    other_rp.write_swizzle = skia_rust_gpu::gpu::swizzle::Swizzle::bgra();
    let handle =
        shared.create_pipeline_handle(Some(dict), &desc, &other_rp, PipelineCreationFlags::NONE);
    assert!(handle.pipeline_or_null().is_some());
}

#[test]
fn precompiles_are_low_priority_and_a_normal_request_promotes_the_task() {
    let handler = Arc::new(Recorded::default());
    let executor = Arc::new(ManualExecutor::default());
    let shared = shared_context(&vulkan(), &options_with(&handler, Some(executor.clone())));
    let manager = shared.base().pipeline_manager();
    let (desc, rp_desc, dict) = solid_desc(&shared, "analytic_rrect[0]");
    let (other_desc, other_rp, other_dict) = solid_desc(&shared, "circular_arc[0]");

    // A precompile goes to the low priority work list.
    let precompile = shared.create_pipeline_handle(
        Some(dict.clone()),
        &desc,
        &rp_desc,
        PipelineCreationFlags::FOR_PRECOMPILATION,
    );
    assert_eq!(executor.pending_work_lists(), vec![1]);
    assert!(precompile.pipeline_or_null().is_none());

    // A normal request for the same pipeline finds the task and re-adds it to the high priority
    // list; asking again does not add it a third time.
    let normal = shared.create_pipeline_handle(
        Some(dict.clone()),
        &desc,
        &rp_desc,
        PipelineCreationFlags::NONE,
    );
    let _ = shared.create_pipeline_handle(Some(dict), &desc, &rp_desc, PipelineCreationFlags::NONE);
    assert_eq!(executor.pending_work_lists(), vec![1, 0]);
    assert_eq!(manager.get_stats().num_tasks_created, 1);
    assert_eq!(manager.get_stats().num_preemptively_found_tasks, 2);
    assert_eq!(manager.num_active_tasks(), 1);

    // A different pipeline, kicked off normally.
    let other = shared.create_pipeline_handle(
        Some(other_dict),
        &other_desc,
        &other_rp,
        PipelineCreationFlags::NONE,
    );
    assert_eq!(executor.pending_work_lists(), vec![1, 0, 0]);

    // The high priority work runs first: the promoted task compiles as a normal compile.
    assert!(executor.run_next());
    let pipeline = normal
        .pipeline_or_null()
        .expect("the first work list ran first");
    assert!(!pipeline.from_precompile());
    assert!(same_pipeline(
        &pipeline,
        &precompile.pipeline_or_null().unwrap()
    ));

    // The duplicate in the low priority list finds the task started, and does nothing.
    assert!(executor.run_next());
    assert!(executor.run_next());
    assert!(!executor.run_next());
    assert_eq!(shared.base().global_cache().num_graphics_pipelines(), 2);
    assert_eq!(manager.num_active_tasks(), 0);
    assert!(other.pipeline_or_null().is_some());
}

#[test]
fn resolving_a_handle_preempts_the_queued_task() {
    let handler = Arc::new(Recorded::default());
    let executor = Arc::new(ManualExecutor::default());
    let shared = shared_context(&vulkan(), &options_with(&handler, Some(executor.clone())));
    let (desc, rp_desc, dict) = solid_desc(&shared, "analytic_rrect[0]");

    let handle =
        shared.create_pipeline_handle(Some(dict), &desc, &rp_desc, PipelineCreationFlags::NONE);
    assert!(handle.pipeline_or_null().is_none());

    // `resolveHandle` compiles in the calling thread instead of waiting for the executor.
    let pipeline = pipeline_of(&handle, &shared);
    assert!(handle.pipeline_or_null().is_some());

    // The queued closure finds the task started, and does not compile again.
    let creations_before = shared
        .base()
        .global_cache()
        .stats()
        .graphics_cache_additions;
    assert!(executor.run_next());
    assert_eq!(
        shared
            .base()
            .global_cache()
            .stats()
            .graphics_cache_additions,
        creations_before
    );
    assert_eq!(shared.base().global_cache().num_graphics_pipelines(), 1);
    assert!(same_pipeline(
        &pipeline,
        &handle.pipeline_or_null().unwrap()
    ));
}

#[test]
fn dropping_the_context_waits_for_the_queued_tasks() {
    let handler = Arc::new(Recorded::default());
    let executor = Arc::new(ManualExecutor::default());
    let options = options_with(&handler, Some(executor.clone()));
    let context = make_context(&noop_with_immediates(&vulkan()), &options).expect("a context");
    let shared = Arc::clone(context.shared_context());
    let (desc, rp_desc, dict) = solid_desc(&shared, "circular_arc[0]");
    let handle =
        shared.create_pipeline_handle(Some(dict), &desc, &rp_desc, PipelineCreationFlags::NONE);
    assert!(handle.pipeline_or_null().is_none());

    // `Context::~Context()` shuts the manager down.
    drop(context);

    assert!(handle.pipeline_or_null().is_some());
    assert_eq!(shared.base().global_cache().num_graphics_pipelines(), 1);
}

/// A noop device for the default profile of `make_context`, which reads the device's own
/// features.
fn noop_with_immediates(
    profile: &CapsProfile,
) -> skia_rust_gpu::graphite::wgpu::WgpuBackendContext {
    let mut features = wgpu::Features::IMMEDIATES;
    if profile.features.contains(DeviceFeatures::SHADER_F16) {
        features |= wgpu::Features::SHADER_F16;
    }
    noop_backend_context_with_features(
        features | wgpu::Features::DUAL_SOURCE_BLENDING,
        wgpu::Limits {
            max_immediate_size: 64,
            ..wgpu::Limits::default()
        },
    )
    .unwrap()
}

/// A pipeline that is only a record of its creation.
#[derive(Debug)]
struct FakePipeline {
    base: GraphicsPipelineBase,
}

impl GraphicsPipeline for FakePipeline {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn base(&self) -> &GraphicsPipelineBase {
        &self.base
    }
}

/// A `PipelineCreationContext` that is not a `WgpuSharedContext`, so it can be dropped (a wgpu
/// shared context owns a resource provider that owns it, and is never freed): the manager's
/// protocol with a context that creates fake pipelines.
struct FakeContext {
    base: SharedContext,
    creations: AtomicU32,
}

impl FakeContext {
    fn new(executor: Option<Arc<dyn Executor>>) -> Arc<Self> {
        let caps = Arc::new(WgpuCaps::new(&vulkan(), &ContextOptions::default()));
        Arc::new(Self {
            base: SharedContext::new(
                caps,
                BackendApi::Mock,
                ShaderCodeDictionary::new(Layout::Std140, &[]),
                executor,
            ),
            creations: AtomicU32::new(0),
        })
    }
}

impl PipelineCreationContext for FakeContext {
    fn shared_context(&self) -> &SharedContext {
        &self.base
    }

    fn renderer_provider(&self) -> &skia_rust_gpu::graphite::renderer_provider::RendererProvider {
        unimplemented!(
            "the fake context has no renderer provider; the tests here do not precompile"
        )
    }

    fn find_or_create_graphics_pipeline(
        &self,
        _runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
        pipeline_key: &skia_rust_gpu::gpu::resource_key::UniqueKey,
        pipeline_desc: &GraphicsPipelineDesc,
        render_pass_desc: &RenderPassDesc,
        flags: PipelineCreationFlags,
    ) -> Option<Arc<dyn GraphicsPipeline>> {
        self.base.find_or_create_graphics_pipeline(
            pipeline_key,
            pipeline_desc,
            render_pass_desc,
            flags,
            |compilation_id| {
                self.creations.fetch_add(1, Ordering::Relaxed);
                Some(Arc::new(FakePipeline {
                    base: GraphicsPipelineBase::new(
                        "fake",
                        pipeline_key.hash(),
                        compilation_id,
                        flags.contains(PipelineCreationFlags::FOR_PRECOMPILATION),
                    ),
                }) as Arc<dyn GraphicsPipeline>)
            },
        )
    }
}

fn fake_handle(
    context: &Arc<FakeContext>,
    step: RenderStepID,
    flags: PipelineCreationFlags,
) -> GraphicsPipelineHandle {
    let rp_desc = render_pass_desc(&WgpuCaps::new(&vulkan(), &ContextOptions::default()), false);
    let desc = GraphicsPipelineDesc::new(step, UniquePaintParamsID::new(1));
    let creation_context: Arc<dyn PipelineCreationContext> = context.clone();
    context
        .base
        .pipeline_manager()
        .create_handle(&creation_context, None, &desc, &rp_desc, flags)
}

#[test]
fn a_task_whose_shared_context_is_gone_completes_without_a_pipeline() {
    let executor = Arc::new(ManualExecutor::default());
    let context = FakeContext::new(Some(executor.clone()));
    let handle = fake_handle(
        &context,
        RenderStepID::CircularArc,
        PipelineCreationFlags::NONE,
    );
    assert!(handle.pipeline_or_null().is_none());
    assert_eq!(executor.pending_work_lists(), vec![0]);

    // The task holds the shared context weakly: dropping the last strong reference without
    // shutting the manager down drops the manager, which waits for the task. The task finds no
    // shared context, and completes with no pipeline.
    drop(context);

    assert_eq!(executor.pending_work_lists(), Vec::<i32>::new());
    assert!(handle.pipeline_or_null().is_none());
}

#[test]
fn the_manager_compiles_each_pipeline_once_whoever_asks() {
    let executor = Arc::new(ManualExecutor::default());
    let context = FakeContext::new(Some(executor.clone()));
    let manager = context.base.pipeline_manager();

    let a = fake_handle(
        &context,
        RenderStepID::CircularArc,
        PipelineCreationFlags::NONE,
    );
    let b = fake_handle(
        &context,
        RenderStepID::CircularArc,
        PipelineCreationFlags::NONE,
    );
    let c = fake_handle(
        &context,
        RenderStepID::AnalyticRRect,
        PipelineCreationFlags::NONE,
    );
    // Two tasks, one of them found a second time, and one work list entry for each.
    assert_eq!(manager.get_stats().num_tasks_created, 2);
    assert_eq!(manager.get_stats().num_preemptively_found_tasks, 1);
    assert_eq!(executor.pending_work_lists(), vec![0, 0]);

    // `resolveHandle` on one preempts its entry, which then finds the task started.
    let pipeline = manager.resolve_handle(&a).unwrap();
    assert!(same_pipeline(&pipeline, &b.pipeline_or_null().unwrap()));
    assert!(c.pipeline_or_null().is_none());
    manager.wait_test_only();
    assert!(c.pipeline_or_null().is_some());
    assert_eq!(context.creations.load(Ordering::Relaxed), 2);
    assert_eq!(manager.num_active_tasks(), 0);

    // The pipeline is in the cache: a new request for it makes no task.
    let again = fake_handle(
        &context,
        RenderStepID::CircularArc,
        PipelineCreationFlags::NONE,
    );
    assert_eq!(manager.get_stats().num_tasks_created, 2);
    assert!(same_pipeline(
        &pipeline,
        &manager.resolve_handle(&again).unwrap()
    ));
}

// ---------------------------------------------------------------------------------------------
// The error checker and the shader module helper
// ---------------------------------------------------------------------------------------------

#[test]
fn the_error_checker_reports_what_the_scopes_caught() {
    let handler = Arc::new(Recorded::default());
    let shared = shared_context(&vulkan(), &options_with(&handler, None));
    let device = shared.device();

    let mut clean = ErrorChecker::new(device);
    let _ok = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    assert_eq!(clean.pop_error_scopes(), ErrorType::NO_ERROR);
    // The scopes are popped once.
    assert_eq!(clean.pop_error_scopes(), ErrorType::NO_ERROR);

    let mut failing = ErrorChecker::new(device);
    // MAP_READ may only be combined with COPY_DST: a validation error.
    let _bad = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::VERTEX,
        mapped_at_creation: false,
    });
    assert_eq!(failing.pop_error_scopes(), ErrorType::VALIDATION);
    assert_eq!(failing.pop_error_scopes(), ErrorType::NO_ERROR);

    // Dropping a checker pops what is left (and, in debug builds, asserts there was no error).
    drop(ErrorChecker::new(device));
    assert_eq!(ErrorType::NO_ERROR.bits(), 0);
    assert_eq!(ErrorType::VALIDATION.bits(), 1);
    assert_eq!(ErrorType::OUT_OF_MEMORY.bits(), 2);
    assert_eq!(ErrorType::INTERNAL.bits(), 4);
}

#[test]
fn a_wgsl_module_that_does_not_compile_is_reported_to_the_error_handler() {
    let handler = Arc::new(Recorded::default());
    let shared = shared_context(&vulkan(), &options_with(&handler, None));
    let caps = shared.caps().clone();

    let good = "@fragment fn main() {}\n";
    assert!(
        compile_wgsl_shader_module(&shared, "good", good, caps.shader_error_handler()).is_some()
    );
    assert!(handler.0.lock().unwrap().is_empty());

    let bad = "@fragment\nfn main() { let x: f32 = vec2<f32>(1.0); }\n";
    assert!(compile_wgsl_shader_module(&shared, "bad", bad, caps.shader_error_handler()).is_none());
    let errors = handler.0.lock().unwrap().clone();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].contains("Shader compilation error"),
        "{}",
        errors[0]
    );
    assert!(errors[0].contains("line 2:"), "{}", errors[0]);
    assert!(
        errors[0].contains("fn main() { let x: f32"),
        "{}",
        errors[0]
    );
}

// ---------------------------------------------------------------------------------------------
// Compute pipelines
// ---------------------------------------------------------------------------------------------

#[derive(Debug)]
struct NativeStep {
    base: ComputeStepBase,
    wgsl: &'static str,
}

impl ComputeStep for NativeStep {
    fn base(&self) -> &ComputeStepBase {
        &self.base
    }

    fn native_shader_source(&self, format: NativeShaderFormat) -> NativeShaderSource<'_> {
        assert_eq!(format, NativeShaderFormat::Wgsl);
        NativeShaderSource {
            source: self.wgsl,
            entry_point: "cs_main".to_owned(),
        }
    }

    fn calculate_texture_parameters(
        &self,
        _resource_index: usize,
        _resource: &ResourceDesc,
    ) -> (ISize, ColorType) {
        (ISize::new(16, 16), ColorType::RGBA8888)
    }
}

const SCALE_WGSL: &str = "\
struct Params { scale: f32 }
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> data: array<f32>;
@group(0) @binding(2) var<storage, read> weights: array<f32>;
@compute @workgroup_size(8, 1, 1)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    data[id.x] = data[id.x] * params.scale * weights[id.x];
}
";

const TEXTURE_WGSL: &str = "\
@group(0) @binding(0) var samp: sampler;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var out_tex: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var in_tex: texture_2d<f32>;
@compute @workgroup_size(4, 4, 1)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let c = textureSampleLevel(tex, samp, vec2<f32>(0.5, 0.5), 0.0);
    let d = textureLoad(in_tex, vec2<i32>(id.xy), 0);
    textureStore(out_tex, vec2<i32>(id.xy), c + d);
}
";

fn native_step(name: &str, wgsl: &'static str, resources: &[ResourceDesc]) -> Arc<dyn ComputeStep> {
    Arc::new(NativeStep {
        base: ComputeStepBase::new(name, WorkgroupSize::new(8, 1, 1), resources, &[], true),
        wgsl,
    })
}

#[test]
fn a_native_compute_step_makes_a_pipeline_with_one_bind_group() {
    let handler = Arc::new(Recorded::default());
    let shared = shared_context(&vulkan(), &options_with(&handler, None));
    let step = native_step(
        "Scale",
        SCALE_WGSL,
        &[
            ResourceDesc::new(
                ResourceType::UniformBuffer,
                DataFlow::Private,
                ResourcePolicy::Mapped,
            ),
            ResourceDesc::with_slot(
                ResourceType::StorageBuffer,
                DataFlow::Shared,
                ResourcePolicy::Clear,
                0,
            ),
            ResourceDesc::new(
                ResourceType::ReadOnlyStorageBuffer,
                DataFlow::Private,
                ResourcePolicy::Mapped,
            ),
        ],
    );
    let desc = ComputePipelineDesc::new(step);

    let pipeline = shared
        .create_compute_pipeline(&desc)
        .expect("a compute pipeline");

    let entries = pipeline.group_layout_entries();
    assert_eq!(entries.len(), 3);
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(entry.binding as usize, i);
        assert_eq!(entry.visibility, wgpu::ShaderStages::COMPUTE);
    }
    let buffer_type = |entry: &wgpu::BindGroupLayoutEntry| match entry.ty {
        wgpu::BindingType::Buffer { ty, .. } => ty,
        other => panic!("not a buffer: {other:?}"),
    };
    assert_eq!(buffer_type(&entries[0]), wgpu::BufferBindingType::Uniform);
    assert_eq!(
        buffer_type(&entries[1]),
        wgpu::BufferBindingType::Storage { read_only: false }
    );
    assert_eq!(
        buffer_type(&entries[2]),
        wgpu::BufferBindingType::Storage { read_only: true }
    );
}

#[test]
fn textures_and_samplers_take_the_bindings_the_dawn_layout_gives_them() {
    let handler = Arc::new(Recorded::default());
    let shared = shared_context(&vulkan(), &options_with(&handler, None));
    let step = native_step(
        "Textures",
        TEXTURE_WGSL,
        &[
            ResourceDesc::new(
                ResourceType::SampledTexture,
                DataFlow::Private,
                ResourcePolicy::None,
            ),
            ResourceDesc::new(
                ResourceType::WriteOnlyStorageTexture,
                DataFlow::Private,
                ResourcePolicy::None,
            ),
            ResourceDesc::new(
                ResourceType::ReadOnlyTexture,
                DataFlow::Private,
                ResourcePolicy::None,
            ),
        ],
    );
    let desc = ComputePipelineDesc::new(step);

    let pipeline = shared
        .create_compute_pipeline(&desc)
        .expect("a compute pipeline");

    // A sampled texture is a sampler and a texture; the rest take one binding each.
    let entries = pipeline.group_layout_entries();
    assert_eq!(entries.len(), 4);
    assert_eq!(
        entries[0].ty,
        wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
    );
    assert!(matches!(
        entries[1].ty,
        wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        }
    ));
    assert_eq!(
        entries[2].ty,
        wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: wgpu::TextureFormat::Rgba8Unorm,
            view_dimension: wgpu::TextureViewDimension::D2,
        }
    );
    assert!(matches!(entries[3].ty, wgpu::BindingType::Texture { .. }));
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(entry.binding as usize, i);
    }
}

#[test]
fn compute_pipelines_are_cached_by_the_step_id() {
    let handler = Arc::new(Recorded::default());
    let shared = shared_context(&vulkan(), &options_with(&handler, None));
    let resources = [
        ResourceDesc::new(
            ResourceType::UniformBuffer,
            DataFlow::Private,
            ResourcePolicy::Mapped,
        ),
        ResourceDesc::with_slot(
            ResourceType::StorageBuffer,
            DataFlow::Shared,
            ResourcePolicy::Clear,
            0,
        ),
        ResourceDesc::new(
            ResourceType::ReadOnlyStorageBuffer,
            DataFlow::Private,
            ResourcePolicy::Mapped,
        ),
    ];
    let desc = ComputePipelineDesc::new(native_step("A", SCALE_WGSL, &resources));
    let other = ComputePipelineDesc::new(native_step("B", SCALE_WGSL, &resources));

    // One word: the step's unique id.
    let key = shared.caps().make_compute_pipeline_key(&desc);
    assert_eq!(key.data(), &[desc.unique_id()]);
    assert_eq!(key.tag(), Some("ComputePipeline"));
    assert_ne!(key, shared.caps().make_compute_pipeline_key(&other));

    let first = shared.find_or_create_compute_pipeline(&desc).unwrap();
    let second = shared.find_or_create_compute_pipeline(&desc).unwrap();
    let third = shared.find_or_create_compute_pipeline(&other).unwrap();
    assert!(std::ptr::addr_eq(Arc::as_ptr(&first), Arc::as_ptr(&second)));
    assert!(!std::ptr::addr_eq(Arc::as_ptr(&first), Arc::as_ptr(&third)));
    assert!(
        first
            .as_any()
            .downcast_ref::<WgpuComputePipeline>()
            .is_some()
    );
}

/// A step with `SkSL` source: the program is built with `BuildComputeSkSL`.
#[derive(Debug)]
struct SkslStep {
    base: ComputeStepBase,
}

impl ComputeStep for SkslStep {
    fn base(&self) -> &ComputeStepBase {
        &self.base
    }

    fn compute_sksl(&self) -> String {
        "void main() { uint i = sk_GlobalInvocationID.x; out_data[i] = in_data[i] * 2.0; }\n"
            .to_owned()
    }
}

#[test]
fn a_sksl_compute_step_declares_its_resources_in_order() {
    let handler = Arc::new(Recorded::default());
    let shared = shared_context(&vulkan(), &options_with(&handler, None));
    let step = SkslStep {
        base: ComputeStepBase::new(
            "Double",
            WorkgroupSize::new(16, 2, 1),
            &[
                ResourceDesc::with_sksl(
                    ResourceType::ReadOnlyStorageBuffer,
                    DataFlow::Private,
                    ResourcePolicy::Mapped,
                    "inBuf { float in_data[]; }",
                ),
                ResourceDesc::with_sksl(
                    ResourceType::StorageBuffer,
                    DataFlow::Private,
                    ResourcePolicy::None,
                    "outBuf { float out_data[]; }",
                ),
            ],
            &[],
            false,
        ),
    };

    let sksl = build_compute_sksl(
        &**shared.caps(),
        &step,
        skia_rust_gpu::gpu::gpu_types::BackendApi::Dawn,
    );

    assert_eq!(
        sksl,
        "layout(local_size_x=16, local_size_y=2, local_size_z=1) in;\n\
         layout(binding=0) readonly buffer inBuf { float in_data[]; };\n\
         layout(binding=1) buffer outBuf { float out_data[]; };\n\
         void main() { uint i = sk_GlobalInvocationID.x; out_data[i] = in_data[i] * 2.0; }\n"
    );

    // The SkSL is compiled to WGSL and the pipeline made from it.
    let pipeline = shared.create_compute_pipeline(&ComputePipelineDesc::new(Arc::new(step)));
    assert!(
        pipeline.is_some(),
        "{:?}",
        handler.0.lock().unwrap().clone()
    );
    assert_eq!(pipeline.unwrap().group_layout_entries().len(), 2);
}
