// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnResourceProvider.h, DawnResourceProvider.cpp

//! `DawnResourceProvider` on wgpu: the backend half of the [`ResourceProvider`].
//!
//! The neutral half owns the resource cache and calls this half through
//! [`ResourceProviderBackend`]. The members `DawnResourceProvider` adds to the base class
//! (`findOrCreateDawnBuffer`, the blit encoders, the bind group helpers, the null buffer and
//! texture view) are methods of [`WgpuResourceProvider`], reached from the owner with
//! [`wgpu_backend`]; `findOrCreateDiscardableMSAALoadTexture`, which calls the base class's
//! `findOrCreateShareableTexture`, is a function of the owner.
//!
//! Not ported yet: the `IntrinsicConstantsManager` (it tracks the buffers it hands out on a
//! `DawnCommandBuffer`, G11c). `createComputePipeline` is [`WgpuSharedContext::create_compute_pipeline`].

use std::any::Any;
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;

use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;

use crate::gpu::gpu_types::{BackendApi, StdSteadyClockTimePoint};
use crate::graphite::backend_texture::BackendTexture;
use crate::graphite::buffer::{BindBufferInfo, Buffer};
use crate::graphite::graphite_resource_key::GraphiteResourceKey;
use crate::graphite::graphite_types::SampleCount;
use crate::graphite::render_pass_desc::RenderPassDesc;
use crate::graphite::resource::{AnyResource, Resource, ResourceRef};
use crate::graphite::resource_provider::{ResourceProvider, ResourceProviderBackend};
use crate::graphite::resource_types::{AccessPattern, BufferType, ResourceType, SamplerDesc};
use crate::graphite::sampler::Sampler;
use crate::graphite::texture::Texture;
use crate::graphite::texture_info::TextureInfo;
use crate::graphite::wgpu::async_wait::create_checked;
use crate::graphite::wgpu::backend_texture::backend_textures;
use crate::graphite::wgpu::buffer::{WgpuBuffer, as_wgpu_buffer};
use crate::graphite::wgpu::caps::{
    COMBINED_UNIFORM_INDEX, INTRINSIC_UNIFORM_BUFFER_INDEX, STORAGE_BUFFER_INDEX,
};
use crate::graphite::wgpu::graphite_utils::texture_format_to_wgpu_format;
use crate::graphite::wgpu::sampler::{WgpuSampler, as_wgpu_sampler};
use crate::graphite::wgpu::shared_context::WgpuSharedContext;
use crate::graphite::wgpu::texture::{WgpuTexture, as_wgpu_texture};
use crate::graphite::wgpu::texture_info::texture_infos;

// Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L30 (chrome/m156)
const BUFFER_BINDING_SIZE_ALIGNMENT: u64 = 16;

/// `DawnResourceProvider::kNumUniformEntries`.
pub const NUM_UNIFORM_ENTRIES: usize = 3;

/// The source of the blit-with-draw shader. `%u` is the source's sample count.
// Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L402-L442 (chrome/m156)
const BLIT_SHADER_SRC: &str = "\
struct VertexOutput {\
@builtin(position) position: vec4f,\
@location(1) @interpolate(flat, either) srcOffset: vec2i,\
};\
var<private> fullscreenTriPositions : array<vec2<f32>, 3> = array<vec2<f32>, 3>(\
vec2(-1.0, -1.0), vec2(-1.0, 3.0), vec2(3.0, -1.0));\
@vertex \
fn VS(@builtin(vertex_index) vertexIndex : u32,\
@builtin(instance_index) instanceIndex : u32)\
-> VertexOutput {\
var out: VertexOutput;\
out.position = vec4(fullscreenTriPositions[vertexIndex], 1.0, 1.0);\
var srcOffset = vec2u(\
bitcast<u32>(instanceIndex & 0xffff),\
bitcast<u32>(instanceIndex >> 16)\
);\
let hasSignBit = (srcOffset & vec2u(0x8000)) != vec2u(0u);\
srcOffset = select(srcOffset, srcOffset | vec2u(0xffff0000), hasSignBit);\
out.srcOffset = bitcast<vec2i>(srcOffset);\
return out;\
}\
fn getSamplingCoords(input: VertexOutput) -> vec2i {\
var coords : vec2<i32> = vec2<i32>(i32(input.position.x), i32(input.position.y));\
return coords - input.srcOffset;\
}\
@group(0) @binding(0) var colorMap: texture_2d<f32>;\
@fragment \
fn SampleFS(input: VertexOutput) -> @location(0) vec4<f32> {\
let coords = getSamplingCoords(input);\
return textureLoad(colorMap, coords, 0);\
}\
@group(0) @binding(1) var msColorMap: texture_multisampled_2d<f32>;\
@fragment\n\
fn SampleMSAAFS(input: VertexOutput) -> @location(0) vec4<f32> {\
let coords = getSamplingCoords(input);\
const sampleCount = %u;\
var sum = vec4f(0.0);\
for (var i: u32 = 0; i < sampleCount; i = i + 1) {\
sum += textureLoad(msColorMap, coords, i);\
}\
return sum * (1.0 / f32(sampleCount));\
}";

/// A pipeline that copies a texture region into the current render pass with a draw
/// (`DawnResourceProvider::BlitWithDrawEncoder`).
// Port of: src/gpu/graphite/dawn/DawnResourceProvider.h#L26-L44 (chrome/m156)
#[derive(Clone, Debug)]
pub struct BlitWithDrawEncoder {
    pipeline: Option<wgpu::RenderPipeline>,
    src_is_msaa: bool,
}

impl BlitWithDrawEncoder {
    /// `operator bool()`: whether the pipeline could be created.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.pipeline.is_some()
    }

    /// `EncodeBlit()`: draws the part of `src_texture_view` at `src_offset` into `dst_bounds` of
    /// the render pass.
    ///
    /// # Panics
    /// If the encoder is not valid.
    // Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L283-L336 (chrome/m156)
    #[doc(alias = "EncodeBlit")]
    #[allow(clippy::cast_sign_loss)] // dst bounds are non-negative render pass coordinates
    #[allow(clippy::cast_precision_loss)] // viewport coordinates are small integers
    pub fn encode_blit(
        &self,
        device: &wgpu::Device,
        render_encoder: &mut wgpu::RenderPass<'_>,
        src_texture_view: &wgpu::TextureView,
        src_offset: IPoint,
        dst_bounds: IRect,
    ) {
        let pipeline = self.pipeline.as_ref().expect("a valid blit encoder");
        render_encoder.set_pipeline(pipeline);

        // TODO(b/260368758): cache single texture's bind group creation.
        let entry = wgpu::BindGroupEntry {
            binding: u32::from(self.src_is_msaa),
            resource: wgpu::BindingResource::TextureView(src_texture_view),
        };
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[entry],
        });
        render_encoder.set_bind_group(0, &bind_group, &[]);

        render_encoder.set_scissor_rect(
            dst_bounds.left as u32,
            dst_bounds.top as u32,
            dst_bounds.width() as u32,
            dst_bounds.height() as u32,
        );
        render_encoder.set_viewport(
            dst_bounds.left as f32,
            dst_bounds.top as f32,
            dst_bounds.width() as f32,
            dst_bounds.height() as f32,
            0.0,
            1.0,
        );

        // In fragment shader, the sampling coords are calculated as:
        // - x = fragPosition.x - dstX + srcX = fragPosition.x - (dxtX - srcX)
        // - y = fragPosition.y - dstY + srcX = fragPosition.y - (dxtY - srcY)
        let delta_x = dst_bounds.left - src_offset.x;
        let delta_y = dst_bounds.top - src_offset.y;

        // Since texture's sizes are never larger than 16 bits, we can encode the offsets's (x, y)
        // in one single 32 bits instance index value. In future, once push constants are
        // implemented in Dawn, we should use them instead.
        debug_assert!(delta_x.abs() < i32::from(i16::MAX));
        debug_assert!(delta_y.abs() < i32::from(i16::MAX));
        let base_instance = (delta_x & 0xffff) | (delta_y << 16);

        // NOTE(b/457887457): need to cast baseInstance to uint32_t explicitly otherwise it would
        // cause TypeError in emscripten, because baseInstance value could be negative in signed
        // integer representation.
        let first_instance = u32::from_ne_bytes(base_instance.to_ne_bytes());
        render_encoder.draw(
            /* firstVertex.. vertexCount= */ 0..3,
            /* firstInstance.. instanceCount= */
            first_instance..first_instance.wrapping_add(1),
        );
    }
}

/// The wgpu half of the resource provider.
// Port of: src/gpu/graphite/dawn/DawnResourceProvider.h#L22-L100 (chrome/m156)
#[doc(alias = "DawnResourceProvider")]
#[derive(Debug)]
pub struct WgpuResourceProvider {
    shared_context: Arc<WgpuSharedContext>,
    blit_with_draw_pipelines: HashMap<u32, wgpu::RenderPipeline>,
    null_buffer: Option<wgpu::Buffer>,
    null_texture_view: Option<wgpu::TextureView>,
}

impl WgpuResourceProvider {
    /// `DawnResourceProvider(sharedContext, …)`.
    #[must_use]
    pub fn new(shared_context: Arc<WgpuSharedContext>) -> Self {
        Self {
            shared_context,
            blit_with_draw_pipelines: HashMap::new(),
            null_buffer: None,
            null_texture_view: None,
        }
    }

    /// `dawnSharedContext()`.
    #[doc(alias = "dawnSharedContext")]
    #[must_use]
    pub fn shared_context(&self) -> &Arc<WgpuSharedContext> {
        &self.shared_context
    }

    /// `findOrCreateBlitWithDrawEncoder()`.
    // Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L378-L478 (chrome/m156)
    #[doc(alias = "findOrCreateBlitWithDrawEncoder")]
    pub fn find_or_create_blit_with_draw_encoder(
        &mut self,
        render_pass_desc: &RenderPassDesc,
        src_sample_count: SampleCount,
    ) -> BlitWithDrawEncoder {
        // Currently Dawn only supports k1 and k4. So we can optimize the pipeline key by
        // specifying whether the source has MSAA or not.
        debug_assert!(matches!(
            src_sample_count,
            SampleCount::One | SampleCount::Four
        ));
        let src_is_msaa = src_sample_count > SampleCount::One;
        let pipeline_key = self
            .shared_context
            .caps()
            .get_render_pass_desc_key_for_pipeline(render_pass_desc, src_is_msaa);

        let mut pipeline = self.blit_with_draw_pipelines.get(&pipeline_key).cloned();
        if pipeline.is_none() {
            // Since texture's sizes are never larger than 16 bits, we can encode the offsets's
            // (x, y) in one single 32 bits instance index value.
            let source = BLIT_SHADER_SRC.replace("%u", &(src_sample_count as u32).to_string());
            let shader_module =
                self.shared_context
                    .device()
                    .create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: None,
                        source: wgpu::ShaderSource::Wgsl(source.into()),
                    });
            pipeline = self.create_blit_render_pipeline(
                /* label= */ "BlitWithDraw",
                &shader_module,
                "VS",
                if src_is_msaa {
                    "SampleMSAAFS"
                } else {
                    "SampleFS"
                },
                /* renderPassColorFormat= */
                texture_format_to_wgpu_format(render_pass_desc.color_attachment.format),
                /* renderPassDepthStencilFormat= */
                texture_format_to_wgpu_format(render_pass_desc.depth_stencil_attachment.format),
                render_pass_desc.color_attachment.sample_count,
            );
            if let Some(pipeline) = &pipeline {
                self.blit_with_draw_pipelines
                    .insert(pipeline_key, pipeline.clone());
            }
        }
        BlitWithDrawEncoder {
            pipeline,
            src_is_msaa,
        }
    }

    // Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L46-L112 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    fn create_blit_render_pipeline(
        &self,
        label: &str,
        shader_module: &wgpu::ShaderModule,
        vs_entry_point: &str,
        fs_entry_point: &str,
        render_pass_color_format: Option<wgpu::TextureFormat>,
        render_pass_depth_stencil_format: Option<wgpu::TextureFormat>,
        sample_count: SampleCount,
    ) -> Option<wgpu::RenderPipeline> {
        let color_target = wgpu::ColorTargetState {
            format: render_pass_color_format?,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        };
        let depth_stencil =
            render_pass_depth_stencil_format.map(|format| wgpu::DepthStencilState {
                format,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            });
        let targets = [Some(color_target)];
        let descriptor = wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: None,
            vertex: wgpu::VertexState {
                module: shader_module,
                entry_point: Some(vs_entry_point),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil,
            multisample: wgpu::MultisampleState {
                count: sample_count as u32,
                mask: 0xFFFF_FFFF,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(wgpu::FragmentState {
                module: shader_module,
                entry_point: Some(fs_entry_point),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &targets,
            }),
            multiview_mask: None,
            cache: None,
        };

        create_checked(
            self.shared_context.device(),
            self.shared_context.caps().allow_scoped_error_checks(),
            || {
                self.shared_context
                    .device()
                    .create_render_pipeline(&descriptor)
            },
        )
    }

    /// `findOrCreateDawnBuffer()`.
    // Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L600-L607 (chrome/m156)
    #[doc(alias = "findOrCreateDawnBuffer")]
    pub fn find_or_create_wgpu_buffer(
        provider: &mut ResourceProvider,
        size: usize,
        ty: BufferType,
        access_pattern: AccessPattern,
        label: &str,
    ) -> Option<ResourceRef<Buffer>> {
        provider.find_or_create_non_shareable_buffer(size, ty, access_pattern, label)
    }

    /// `getOrCreateNullBuffer()`: for bind group entries, using this to get a buffer rather than
    /// simply leaving the entry empty allows for assigning a label (when enabled in the caps)
    /// for more clear debugging.
    // Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L609-L624 (chrome/m156)
    #[doc(alias = "getOrCreateNullBuffer")]
    pub fn get_or_create_null_buffer(&mut self) -> &wgpu::Buffer {
        let shared_context = &self.shared_context;
        self.null_buffer.get_or_insert_with(|| {
            shared_context
                .device()
                .create_buffer(&wgpu::BufferDescriptor {
                    label: shared_context
                        .caps()
                        .set_backend_labels()
                        .then_some("UnusedBufferSlot"),
                    size: BUFFER_BINDING_SIZE_ALIGNMENT,
                    usage: wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::UNIFORM
                        | wgpu::BufferUsages::STORAGE,
                    mapped_at_creation: false,
                })
        })
    }

    /// `getOrCreateNullTextureView()`.
    // Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L626-L645 (chrome/m156)
    #[doc(alias = "getOrCreateNullTextureView")]
    pub fn get_or_create_null_texture_view(&mut self) -> &wgpu::TextureView {
        let shared_context = &self.shared_context;
        self.null_texture_view.get_or_insert_with(|| {
            let null_texture = shared_context
                .device()
                .create_texture(&wgpu::TextureDescriptor {
                    label: shared_context
                        .caps()
                        .set_backend_labels()
                        .then_some("UnusedTextureSlot"),
                    size: wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba32Float,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
            null_texture.create_view(&wgpu::TextureViewDescriptor::default())
        })
    }

    /// `createBindGroup()`.
    // Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L647-L656 (chrome/m156)
    #[doc(alias = "createBindGroup")]
    #[must_use]
    pub fn create_bind_group(
        &self,
        entries: &[wgpu::BindGroupEntry<'_>],
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        self.shared_context
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout,
                entries,
            })
    }

    /// `findOrCreateSingleUniformBindGroup()`: finds or creates a bind group containing the given
    /// buffer.
    ///
    /// # Panics
    /// If `buffer_info` has a buffer that is not a wgpu buffer.
    // Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L658-L701 (chrome/m156)
    #[doc(alias = "findOrCreateSingleUniformBindGroup")]
    pub fn find_or_create_single_uniform_bind_group(
        &mut self,
        buffer_info: &BindBufferInfo,
    ) -> Option<wgpu::BindGroup> {
        // We should only hit the single-uniform case if push constant usage is supported for
        // intrinsic constants.
        debug_assert!(
            self.shared_context
                .caps()
                .resource_binding_requirements()
                .use_push_constants_for_intrinsic_constants
        );

        let resource = buffer_info.buffer.as_ref()?;
        let buffer: &Buffer = resource;
        let wgpu_buffer = as_wgpu_buffer(buffer).expect("a wgpu buffer");
        let binding_size = buffer_info.size as usize;
        if let Some(cached_bind_group) =
            wgpu_buffer.get_cached_single_buffer_bind_group(binding_size)
        {
            return Some(cached_bind_group);
        }

        // We should be able to assume that if we only have one uniform that it is the combined
        // uniform buffer. Construct a list of bind group entries to represent the single
        // combined uniform buffer case.
        let null_buffer = self.get_or_create_null_buffer().clone();
        let combined_buffer = wgpu_buffer.wgpu_buffer()?;
        let null_entry = |binding| wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &null_buffer,
                offset: 0,
                size: None,
            }),
        };
        let entries: [wgpu::BindGroupEntry<'_>; NUM_UNIFORM_ENTRIES] = [
            null_entry(INTRINSIC_UNIFORM_BUFFER_INDEX),
            wgpu::BindGroupEntry {
                binding: COMBINED_UNIFORM_INDEX,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &combined_buffer,
                    offset: 0, // Use dynamic offsets; ignore bufferInfo.fOffset
                    size: NonZeroU64::new(
                        u64::from(buffer_info.size).next_multiple_of(BUFFER_BINDING_SIZE_ALIGNMENT),
                    ),
                }),
            },
            null_entry(STORAGE_BUFFER_INDEX),
        ];

        let bind_group = self.create_bind_group(
            &entries,
            self.shared_context
                .get_uniform_buffers_bind_group_layout(wgpu::ShaderStages::empty()),
        );
        wgpu_buffer.add_cached_single_buffer_bind_group(bind_group.clone(), binding_size);
        Some(bind_group)
    }

    /// `findOrCreateSingleTextureSamplerBindGroup()`: finds or creates a bind group containing
    /// the given sampler and texture.
    ///
    /// # Panics
    /// If `sampler` or `texture` is not a wgpu object.
    // Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L703-L728 (chrome/m156)
    #[doc(alias = "findOrCreateSingleTextureSamplerBindGroup")]
    #[must_use]
    pub fn find_or_create_single_texture_sampler_bind_group(
        &self,
        sampler: &Resource<Sampler>,
        texture: &Resource<Texture>,
    ) -> Option<wgpu::BindGroup> {
        let wgpu_texture: &WgpuTexture = as_wgpu_texture(texture).expect("a wgpu texture");
        let wgpu_sampler: &WgpuSampler = as_wgpu_sampler(sampler).expect("a wgpu sampler");

        // First check if we already have a cached bind group we can use.
        let sampler_id = sampler.base().unique_id();
        if let Some(cached_bind_group) =
            wgpu_texture.get_cached_single_texture_bind_group(sampler_id)
        {
            return Some(cached_bind_group);
        }

        // Otherwise, create one and store it on the Texture for potential future reuse.
        let wgpu_sampler = wgpu_sampler.wgpu_sampler()?;
        let sample_texture_view = wgpu_texture.sample_texture_view()?;
        let entries = [
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&wgpu_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&sample_texture_view),
            },
        ];
        let bind_group = self.create_bind_group(
            &entries,
            self.shared_context
                .get_single_texture_sampler_bind_group_layout(),
        );
        wgpu_texture.add_cached_single_texture_bind_group(bind_group.clone(), sampler_id);
        Some(bind_group)
    }
}

/// `findOrCreateDiscardableMSAALoadTexture()`: a single-sampled, sampleable texture to load an
/// MSAA texture's contents from.
// Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L524-L546 (chrome/m156)
#[doc(alias = "findOrCreateDiscardableMSAALoadTexture")]
pub fn find_or_create_discardable_msaa_load_texture(
    provider: &mut ResourceProvider,
    dimensions: ISize,
    msaa_info: &TextureInfo,
) -> Option<ResourceRef<Texture>> {
    debug_assert!(msaa_info.is_valid());

    // Derive the load texture's info from MSAA texture's info.
    let mut wgpu_msaa_load_texture_info = texture_infos::get_wgpu_texture_info(msaa_info)?;
    wgpu_msaa_load_texture_info.sample_count = SampleCount::One;
    wgpu_msaa_load_texture_info.usage |= wgpu::TextureUsages::TEXTURE_BINDING;
    // MSAA texture can be transient attachment (memoryless) but the load texture cannot be. This
    // is because the load texture will need to have its content retained between two passes
    // loading:
    // - first pass: the resolve texture is blitted to the load texture.
    // - 2nd pass: the actual render pass is started and the load texture is blitted to the MSAA
    // texture.
    wgpu_msaa_load_texture_info.usage &= !wgpu::TextureUsages::TRANSIENT_ATTACHMENT;

    provider.find_or_create_shareable_texture(
        dimensions,
        &texture_infos::make_wgpu(&wgpu_msaa_load_texture_info),
        "DiscardableLoadMSAATexture",
    )
}

/// The wgpu half of `provider`, if it has one.
#[must_use]
pub fn wgpu_backend(provider: &mut ResourceProvider) -> Option<&mut WgpuResourceProvider> {
    provider
        .backend()
        .as_any_mut()?
        .downcast_mut::<WgpuResourceProvider>()
}

impl ResourceProviderBackend for WgpuResourceProvider {
    fn max_texture_size(&self) -> i32 {
        self.shared_context.caps().max_texture_size()
    }

    fn build_key_for_texture(
        &self,
        dimensions: ISize,
        info: &TextureInfo,
        ty: ResourceType,
        key: &mut GraphiteResourceKey,
    ) {
        self.shared_context
            .caps()
            .build_key_for_texture(dimensions, info, ty, key);
    }

    // Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L587-L591 (chrome/m156)
    fn create_texture(
        &mut self,
        dimensions: ISize,
        info: &TextureInfo,
        label: &str,
    ) -> Option<ResourceRef<Texture>> {
        WgpuTexture::make(&self.shared_context, dimensions, info, label)
    }

    // Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L593-L598 (chrome/m156)
    fn create_buffer(
        &mut self,
        size: usize,
        ty: BufferType,
        access_pattern: AccessPattern,
        label: &str,
    ) -> Option<ResourceRef<Buffer>> {
        WgpuBuffer::make(&self.shared_context, size, ty, access_pattern, label)
    }

    // Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L550-L554 (chrome/m156)
    fn create_sampler(&mut self, sampler_desc: &SamplerDesc) -> Option<ResourceRef<Sampler>> {
        WgpuSampler::make(&self.shared_context, *sampler_desc)
    }

    // Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L497-L522 (chrome/m156)
    fn on_create_wrapped_texture(
        &mut self,
        texture: &BackendTexture,
        label: &str,
    ) -> Option<ResourceRef<Texture>> {
        let wgpu_texture = backend_textures::get_wgpu_texture(texture);
        let wgpu_texture_view = backend_textures::get_wgpu_texture_view(texture);
        debug_assert!(wgpu_texture.is_none() || wgpu_texture_view.is_none());
        if let Some(wgpu_texture) = wgpu_texture {
            WgpuTexture::make_wrapped(
                &self.shared_context,
                texture.dimensions(),
                &texture.info(),
                wgpu_texture,
                label,
            )
        } else {
            wgpu_texture_view.map(|view| {
                WgpuTexture::make_wrapped_view(texture.dimensions(), &texture.info(), view, label)
            })
        }
    }

    // Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L556-L567 (chrome/m156)
    fn on_create_backend_texture(
        &mut self,
        dimensions: ISize,
        info: &TextureInfo,
    ) -> BackendTexture {
        match WgpuTexture::make_wgpu_texture(&self.shared_context, dimensions, info, "") {
            Some(texture) => backend_textures::make_wgpu(&texture),
            None => BackendTexture::new(),
        }
    }

    // Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L569-L585 (chrome/m156)
    fn on_delete_backend_texture(&mut self, texture: &BackendTexture) {
        debug_assert!(texture.is_valid());
        debug_assert_eq!(texture.backend(), BackendApi::Dawn);
        // We need to explicitly call Destroy() here since since that is the recommended way to
        // delete a Dawn texture predictably versus just dropping a ref and relying on garbage
        // collection.
        //
        // Additionally this helps to work around an issue where Skia may have cached a BindGroup
        // that references the underlying texture. Skia currently doesn't destroy BindGroups when
        // its use of the texture goes away, thus a ref to the texture remains on the BindGroup
        // and memory is never cleared up unless we call Destroy() here.
        if let Some(wgpu_texture) = backend_textures::get_wgpu_texture(texture) {
            wgpu_texture.destroy();
        }
    }

    // Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L730-L734 (chrome/m156)
    fn on_free_gpu_resources(&mut self) {
        // The IntrinsicConstantsManager's buffers are freed here (G11c).
    }

    // Port of: src/gpu/graphite/dawn/DawnResourceProvider.cpp#L736-L741 (chrome/m156)
    fn on_purge_resources_not_used_since(
        &mut self,
        _purge_time: StdSteadyClockTimePoint,
        _quit_purging_time: Option<StdSteadyClockTimePoint>,
    ) {
        // The IntrinsicConstantsManager's buffers are purged here (G11c).
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}
