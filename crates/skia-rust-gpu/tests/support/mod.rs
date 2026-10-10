// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! A mock Graphite back end for the recorder, task and buffer manager tests.
//!
//! Skia's own tests for these classes run on a real `Context` (G9b/G11), so the tests in this
//! directory follow their scenarios and assertions on the seams the port left: a mock
//! `Caps`, resource provider back end, shared context, command buffer and context.
#![allow(dead_code)] // each test file uses a different part

pub mod wgsl_corpus;

use std::any::Any;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use skia_rust_core::color_type::ColorType;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;
use skia_rust_gpu::gpu::gpu_types::{BackendApi, GpuStats, Mipmapped, Protected};
use skia_rust_gpu::gpu::ref_cnted_callback::RefCntedCallback;
use skia_rust_gpu::gpu::resource_key::{UniqueKey, UniqueKeyBuilder};
use skia_rust_gpu::graphite::buffer::{Buffer, BufferBackend, MappedData};
use skia_rust_gpu::graphite::buffer_manager::StaticBufferManager;
use skia_rust_gpu::graphite::caps::{
    AttachmentSizePolicy, Caps, ResourceBindingRequirements, ShaderCaps, default_shader_caps,
};
use skia_rust_gpu::graphite::command_buffer::{BufferTextureCopyData, CommandBuffer};
use skia_rust_gpu::graphite::compute::dispatch_group::DispatchGroup;
use skia_rust_gpu::graphite::compute_pipeline::ComputePipeline;
use skia_rust_gpu::graphite::compute_pipeline_desc::ComputePipelineDesc;
use skia_rust_gpu::graphite::context_priv::{ContextPriv, SharedResourceProvider};
use skia_rust_gpu::graphite::graphics_pipeline_desc::GraphicsPipelineDesc;
use skia_rust_gpu::graphite::graphite_resource_key::{
    GraphiteResourceKey, GraphiteResourceKeyBuilder,
};
use skia_rust_gpu::graphite::graphite_types::{DepthStencilFlags, SampleCount};
use skia_rust_gpu::graphite::recorder::{Recorder, RecorderOptions, RecorderSharedContext};
use skia_rust_gpu::graphite::render_pass_desc::{AttachmentDesc, RenderPassDesc};
use skia_rust_gpu::graphite::renderer_provider::RendererProvider;
use skia_rust_gpu::graphite::resource::{AnyResourceRef, Resource, ResourceRef};
use skia_rust_gpu::graphite::resource_provider::{ResourceProvider, ResourceProviderBackend};
use skia_rust_gpu::graphite::resource_types::DstReadStrategy;
use skia_rust_gpu::graphite::resource_types::{
    AccessPattern, BufferType, Discardable, ImmutableSamplerInfo, Layout, Ownership, ResourceType,
};
use skia_rust_gpu::graphite::shader_code_dictionary::ShaderCodeDictionary;
use skia_rust_gpu::graphite::task::render_pass_task::DrawPass;
use skia_rust_gpu::graphite::texture::{Texture, TextureBackend};
use skia_rust_gpu::graphite::texture_format::TextureFormat;
use skia_rust_gpu::graphite::texture_info::{TextureInfo, TextureInfoData, texture_info_priv};

/// The texture info data of the mock back end.
#[derive(Debug, Clone, PartialEq)]
pub struct MockTextureInfo {
    pub format: TextureFormat,
}

impl TextureInfoData for MockTextureInfo {
    fn backend(&self) -> BackendApi {
        BackendApi::Mock
    }

    fn is_protected(&self) -> Protected {
        Protected::No
    }

    fn view_format(&self) -> TextureFormat {
        self.format
    }

    fn to_backend_string(&self) -> String {
        String::new()
    }

    fn is_compatible(&self, that: &TextureInfo, _require_exact: bool) -> bool {
        that.get::<Self>().is_some_and(|that| that == self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn texture_info(
    format: TextureFormat,
    sample_count: SampleCount,
    mipmapped: Mipmapped,
) -> TextureInfo {
    TextureInfo::make(MockTextureInfo { format }, sample_count, mipmapped)
}

pub fn rgba_info() -> TextureInfo {
    texture_info(TextureFormat::RGBA8, SampleCount::One, Mipmapped::No)
}

/// A texture that counts the frees and uploads on the host.
#[derive(Debug, Default)]
pub struct MockTexture {
    pub freed: Arc<AtomicUsize>,
}

impl TextureBackend for MockTexture {
    fn free_gpu_data(&self) {
        self.freed.fetch_add(1, Ordering::Relaxed);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A buffer whose commits (`unmap_with`) can be inspected.
#[derive(Debug, Default)]
pub struct MockBuffer {
    pub committed: Arc<Mutex<Option<Vec<u8>>>>,
    pub unmaps: Arc<AtomicUsize>,
    pub freed: Arc<AtomicUsize>,
}

impl BufferBackend for MockBuffer {
    fn free_gpu_data(&self) {
        self.freed.fetch_add(1, Ordering::Relaxed);
    }

    fn on_map(&self, size: usize) -> Option<MappedData> {
        Some(vec![0; size])
    }

    fn on_unmap(&self, written: Option<&[u8]>) {
        self.unmaps.fetch_add(1, Ordering::Relaxed);
        if let Some(written) = written {
            *self.committed.lock().unwrap() = Some(written.to_vec());
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The bytes committed to `buffer` by `unmap_with`, if any.
pub fn committed_bytes(buffer: &Buffer) -> Option<Vec<u8>> {
    buffer
        .backend()
        .as_any()
        .downcast_ref::<MockBuffer>()
        .expect("a mock buffer")
        .committed
        .lock()
        .unwrap()
        .clone()
}

/// The number of `unmap` calls the buffer's back end saw.
pub fn unmap_count(buffer: &Buffer) -> usize {
    buffer
        .backend()
        .as_any()
        .downcast_ref::<MockBuffer>()
        .expect("a mock buffer")
        .unmaps
        .load(Ordering::Relaxed)
}

/// The mock caps.
#[derive(Debug, Clone)]
pub struct MockCaps {
    pub draw_buffer_can_be_mapped: bool,
    pub require_ordered_recordings: bool,
    pub uniform_alignment: usize,
    pub storage_alignment: usize,
    pub transfer_alignment: usize,
    pub attachment_size_policy: AttachmentSizePolicy,
    pub storage_buffer_support: bool,
    /// What `toString(ImmutableSamplerInfo)` returns.
    pub immutable_sampler_string: String,
    /// `shaderCaps()`.
    pub shader_caps: ShaderCaps,
    /// `resourceBindingRequirements()`.
    pub resource_binding_requirements: ResourceBindingRequirements,
}

impl Default for MockCaps {
    fn default() -> Self {
        Self {
            draw_buffer_can_be_mapped: true,
            require_ordered_recordings: false,
            uniform_alignment: 256,
            storage_alignment: 16,
            transfer_alignment: 4,
            attachment_size_policy: AttachmentSizePolicy::Exact,
            storage_buffer_support: false,
            immutable_sampler_string: String::new(),
            shader_caps: default_shader_caps(),
            resource_binding_requirements: ResourceBindingRequirements::default(),
        }
    }
}

impl Caps for MockCaps {
    fn max_texture_size(&self) -> i32 {
        4096
    }

    fn get_default_storage_texture_info(&self, _color_type: ColorType) -> TextureInfo {
        todo!()
    }

    fn get_dst_read_strategy(&self) -> DstReadStrategy {
        DstReadStrategy::TextureCopy
    }

    fn supports_hardware_advanced_blending(&self) -> bool {
        false
    }

    fn dual_source_blending_support(&self) -> bool {
        false
    }

    fn require_ordered_recordings(&self) -> bool {
        self.require_ordered_recordings
    }

    fn draw_buffer_can_be_mapped(&self) -> bool {
        self.draw_buffer_can_be_mapped
    }

    fn buffer_maps_are_async(&self) -> bool {
        false
    }

    fn required_uniform_buffer_alignment(&self) -> usize {
        self.uniform_alignment
    }

    fn required_storage_buffer_alignment(&self) -> usize {
        self.storage_alignment
    }

    fn required_transfer_buffer_alignment(&self) -> usize {
        self.transfer_alignment
    }

    fn get_aligned_texture_data_row_bytes(
        &self,
        row_bytes: usize,
        bytes_per_block: usize,
    ) -> usize {
        // Rows aligned to the block size and 4 bytes.
        let alignment = bytes_per_block.max(4);
        row_bytes.div_ceil(alignment) * alignment
    }

    fn full_compressed_upload_size_must_align_to_block_dims(&self) -> bool {
        false
    }

    fn avoid_depth_mode(&self) -> bool {
        false
    }

    fn attachment_size_policy(&self) -> AttachmentSizePolicy {
        self.attachment_size_policy
    }

    fn get_depth_stencil_format(&self, _flags: DepthStencilFlags) -> TextureFormat {
        TextureFormat::D24_S8
    }

    fn get_default_attachment_texture_info(
        &self,
        desc: &AttachmentDesc,
        _is_protected: Protected,
        _discardable: Discardable,
    ) -> TextureInfo {
        texture_info(desc.format, desc.sample_count, Mipmapped::No)
    }

    fn get_default_sampled_texture_info(
        &self,
        color_type: skia_rust_core::color_type::ColorType,
        mipmapped: Mipmapped,
        _is_protected: Protected,
        _renderable: skia_rust_gpu::gpu::gpu_types::Renderable,
    ) -> TextureInfo {
        // The mock back end samples the formats of the color types the tests use.
        let format = match color_type {
            skia_rust_core::color_type::ColorType::Alpha8 => TextureFormat::A8,
            skia_rust_core::color_type::ColorType::RGBAF16 => TextureFormat::RGBA16F,
            _ => TextureFormat::RGBA8,
        };
        texture_info(format, SampleCount::One, mipmapped)
    }

    fn get_default_readable_texture_info(
        &self,
        format: TextureFormat,
        _is_protected: Protected,
    ) -> TextureInfo {
        texture_info(format, SampleCount::One, Mipmapped::No)
    }

    fn get_texture_info_for_sampled_copy(
        &self,
        info: &TextureInfo,
        mipmapped: Mipmapped,
    ) -> TextureInfo {
        texture_info(
            texture_info_priv::view_format(info),
            SampleCount::One,
            mipmapped,
        )
    }

    fn get_compatible_msaa_sample_count(&self, _info: &TextureInfo) -> SampleCount {
        SampleCount::Four
    }

    fn is_renderable_with_msrtss(&self, _info: &TextureInfo) -> bool {
        false
    }

    fn storage_buffer_support(&self) -> bool {
        self.storage_buffer_support
    }

    fn clamp_to_border_support(&self) -> bool {
        // The mock backend samples with clamp-to-border, so no decal substitution happens.
        true
    }

    fn immutable_sampler_info_to_string(&self, _info: &ImmutableSamplerInfo) -> String {
        self.immutable_sampler_string.clone()
    }

    fn shader_caps(&self) -> &ShaderCaps {
        &self.shader_caps
    }

    fn resource_binding_requirements(&self) -> &ResourceBindingRequirements {
        &self.resource_binding_requirements
    }

    // The remaining queries keep the base `Caps` defaults (`Caps.h`).
    fn max_varyings(&self) -> i32 {
        0
    }

    fn ndc_y_axis_points_down(&self) -> bool {
        false
    }

    fn protected_support(&self) -> bool {
        false
    }

    fn semaphore_support(&self) -> bool {
        false
    }

    fn allow_cpu_sync(&self) -> bool {
        true
    }

    fn storage_buffer_support_for_compute(&self) -> bool {
        false
    }

    fn compute_support(&self) -> bool {
        false
    }

    fn avoid_msaa(&self) -> bool {
        false
    }

    fn msaa_render_to_single_sampled_support(&self) -> bool {
        false
    }

    fn use_draw_list_layer(&self) -> bool {
        false
    }

    fn load_op_affects_msaa_pipelines(&self) -> bool {
        false
    }

    fn max_path_atlas_texture_size(&self) -> i32 {
        8192
    }

    fn allow_multiple_atlas_textures(&self) -> bool {
        true
    }

    fn support_bilerp_from_glyph_atlas(&self) -> bool {
        false
    }

    fn set_backend_labels(&self) -> bool {
        false
    }

    fn is_sample_count_supported(&self, _format: TextureFormat, _count: SampleCount) -> bool {
        true
    }

    fn is_texturable(&self, _info: &TextureInfo, _allow_msaa: bool) -> bool {
        true
    }

    fn is_readable(&self, _info: &TextureInfo, _allow_msaa: bool) -> bool {
        true
    }

    fn is_renderable(&self, _info: &TextureInfo) -> bool {
        true
    }

    fn is_copyable_src(&self, _info: &TextureInfo) -> bool {
        true
    }

    fn is_copyable_dst(&self, _info: &TextureInfo) -> bool {
        true
    }

    fn is_storage(&self, _info: &TextureInfo) -> bool {
        false
    }

    fn make_graphics_pipeline_key(
        &self,
        pipeline_desc: &GraphicsPipelineDesc,
        _render_pass_desc: &RenderPassDesc,
    ) -> UniqueKey {
        let mut key = UniqueKey::new();
        {
            let mut builder =
                UniqueKeyBuilder::new(&mut key, UniqueKey::generate_domain(), 2, Some("Mock"));
            builder[0] = pipeline_desc.render_step_id() as u32;
            builder[1] = pipeline_desc.paint_params_id().as_uint();
            builder.finish();
        }
        key
    }

    fn make_compute_pipeline_key(&self, pipeline_desc: &ComputePipelineDesc) -> UniqueKey {
        let mut key = UniqueKey::new();
        {
            let mut builder =
                UniqueKeyBuilder::new(&mut key, UniqueKey::generate_domain(), 1, Some("Mock"));
            builder[0] = pipeline_desc.unique_id();
            builder.finish();
        }
        key
    }
}

/// Counts what the mock resource provider back end created.
#[derive(Debug, Default, Clone)]
pub struct BackendCounts {
    pub textures: Arc<AtomicUsize>,
    pub buffers: Arc<AtomicUsize>,
}

pub struct MockResourceBackend {
    pub counts: BackendCounts,
}

impl ResourceProviderBackend for MockResourceBackend {
    fn find_or_create_compute_pipeline(
        &mut self,
        _pipeline_desc: &ComputePipelineDesc,
    ) -> Option<Arc<dyn ComputePipeline>> {
        None
    }
    fn max_texture_size(&self) -> i32 {
        4096
    }

    fn build_key_for_texture(
        &self,
        dimensions: ISize,
        info: &TextureInfo,
        ty: ResourceType,
        key: &mut GraphiteResourceKey,
    ) {
        let mut builder = GraphiteResourceKeyBuilder::new(key, ty, 4);
        builder[0] = dimensions.width.cast_unsigned();
        builder[1] = dimensions.height.cast_unsigned();
        builder[2] = texture_info_priv::view_format(info) as u32;
        builder[3] = info.sample_count() as u32;
    }

    fn create_texture(
        &mut self,
        dimensions: ISize,
        info: &TextureInfo,
        label: &str,
    ) -> Option<ResourceRef<Texture>> {
        self.counts.textures.fetch_add(1, Ordering::Relaxed);
        Some(Texture::make(
            dimensions,
            info,
            false,
            Ownership::Owned,
            label,
            Box::new(MockTexture::default()),
        ))
    }

    fn create_buffer(
        &mut self,
        size: usize,
        _ty: BufferType,
        _access_pattern: AccessPattern,
        label: &str,
    ) -> Option<ResourceRef<Buffer>> {
        self.counts.buffers.fetch_add(1, Ordering::Relaxed);
        Some(Buffer::make(
            size,
            Protected::No,
            label,
            false,
            false,
            Box::new(MockBuffer::default()),
        ))
    }
}

#[derive(Debug)]
pub struct MockSharedContext {
    pub caps: Arc<MockCaps>,
    pub counts: BackendCounts,
    pub shader_dictionary: ShaderCodeDictionary,
    pub renderer_provider: RendererProvider,
}

impl MockSharedContext {
    pub fn new(caps: MockCaps) -> Arc<Self> {
        let (resource_provider, _) = shared_provider();
        let mut buffer_manager = StaticBufferManager::new(resource_provider, &caps);
        let renderer_provider = RendererProvider::new(
            Layout::Std140,
            caps.shader_caps().infinity_support,
            &mut buffer_manager,
        );
        Arc::new(Self {
            caps: Arc::new(caps),
            counts: BackendCounts::default(),
            shader_dictionary: ShaderCodeDictionary::new(Layout::Std140, &[]),
            renderer_provider,
        })
    }
}

impl RecorderSharedContext for MockSharedContext {
    fn caps(&self) -> Arc<dyn Caps> {
        self.caps.clone()
    }

    fn backend(&self) -> BackendApi {
        BackendApi::Mock
    }

    fn is_protected(&self) -> Protected {
        Protected::No
    }

    fn shader_code_dictionary(&self) -> &ShaderCodeDictionary {
        &self.shader_dictionary
    }

    fn renderer_provider(&self) -> &RendererProvider {
        &self.renderer_provider
    }

    fn make_resource_provider(&self, recorder_id: u32, resource_budget: usize) -> ResourceProvider {
        ResourceProvider::new(
            Box::new(MockResourceBackend {
                counts: self.counts.clone(),
            }),
            recorder_id,
            resource_budget,
        )
    }
}

/// A recorder over a mock shared context with the given caps.
pub fn make_recorder(caps: MockCaps) -> (Recorder, Arc<MockSharedContext>) {
    make_recorder_with(caps, &RecorderOptions::default())
}

pub fn make_recorder_with(
    caps: MockCaps,
    options: &RecorderOptions,
) -> (Recorder, Arc<MockSharedContext>) {
    let shared_context = MockSharedContext::new(caps);
    let recorder = Recorder::new(shared_context.clone(), options, None);
    (recorder, shared_context)
}

/// A standalone resource provider over the mock back end.
pub fn provider() -> (ResourceProvider, BackendCounts) {
    let counts = BackendCounts::default();
    let provider = ResourceProvider::new(
        Box::new(MockResourceBackend {
            counts: counts.clone(),
        }),
        1,
        1 << 20,
    );
    (provider, counts)
}

pub fn shared_provider() -> (SharedResourceProvider, BackendCounts) {
    let (provider, counts) = provider();
    (Arc::new(Mutex::new(provider)), counts)
}

/// The calls a [`MockCommandBuffer`] recorded.
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    TrackResource,
    FinishedProc,
    ReplayClip {
        translation: IPoint,
        clip: IRect,
        bounds: IRect,
    },
    RenderPass {
        has_resolve: bool,
        has_depth_stencil: bool,
        has_dst_copy: bool,
        resolve_offset: IPoint,
        viewport: ISize,
        num_passes: usize,
    },
    ComputePass(usize),
    CopyBufferToBuffer {
        src_offset: usize,
        dst_offset: usize,
        size: usize,
    },
    CopyTextureToBuffer {
        src_rect: IRect,
        buffer_offset: usize,
        buffer_row_bytes: usize,
    },
    CopyBufferToTexture(Vec<BufferTextureCopyData>),
    CopyTextureToTexture {
        src_rect: IRect,
        dst_point: IPoint,
        dst_level: i32,
    },
    SynchronizeBufferToCpu,
    ClearBuffer {
        offset: usize,
        size: usize,
    },
}

/// A command buffer that records its calls.
#[derive(Default)]
pub struct MockCommandBuffer {
    pub calls: Vec<Call>,
    /// Calls fail (return false) when set.
    pub fail: bool,
    /// Whether `set_replay_translation_and_clip` accepts.
    pub reject_clip: bool,
    pub tracked: Vec<AnyResourceRef>,
    pub finished_procs: Vec<Arc<RefCntedCallback>>,
}

impl CommandBuffer for MockCommandBuffer {
    fn is_protected(&self) -> Protected {
        Protected::No
    }

    fn has_work(&self) -> bool {
        self.calls
            .iter()
            .any(|call| !matches!(call, Call::TrackResource | Call::FinishedProc))
    }

    fn set_new_command_buffer_resources(&mut self) -> bool {
        true
    }

    fn reset_command_buffer(&mut self) {
        self.tracked.clear();
        self.finished_procs.clear();
    }

    fn call_finished_procs(&mut self, success: bool) {
        for finished_proc in &self.finished_procs {
            if success {
                finished_proc.set_stats(&GpuStats::default());
            } else {
                finished_proc.set_failure_result();
            }
        }
        self.finished_procs.clear();
    }

    fn add_buffers_to_async_map_on_submit(&mut self, _buffers: &[ResourceRef<Buffer>]) {}

    fn buffers_to_async_map_on_submit(&self) -> &[ResourceRef<Buffer>] {
        &[]
    }

    fn track_resource(&mut self, resource: AnyResourceRef) {
        self.calls.push(Call::TrackResource);
        self.tracked.push(resource);
    }

    fn add_finished_proc(&mut self, finished_proc: Arc<RefCntedCallback>) {
        self.calls.push(Call::FinishedProc);
        self.finished_procs.push(finished_proc);
    }

    fn set_replay_translation_and_clip(
        &mut self,
        translation: IPoint,
        clip: IRect,
        render_target_bounds: IRect,
    ) -> bool {
        self.calls.push(Call::ReplayClip {
            translation,
            clip,
            bounds: render_target_bounds,
        });
        !self.reject_clip
    }

    fn add_render_pass(
        &mut self,
        _render_pass_desc: &RenderPassDesc,
        _color_texture: ResourceRef<Texture>,
        resolve_texture: Option<ResourceRef<Texture>>,
        depth_stencil_texture: Option<ResourceRef<Texture>>,
        dst_copy: Option<&Arc<Resource<Texture>>>,
        _dst_read_bounds: IRect,
        resolve_offset: IPoint,
        viewport_dims: ISize,
        draw_passes: &mut [Box<dyn DrawPass>],
    ) -> bool {
        self.calls.push(Call::RenderPass {
            has_resolve: resolve_texture.is_some(),
            has_depth_stencil: depth_stencil_texture.is_some(),
            has_dst_copy: dst_copy.is_some(),
            resolve_offset,
            viewport: viewport_dims,
            num_passes: draw_passes.len(),
        });
        !self.fail
    }

    fn add_compute_pass(&mut self, dispatches: &mut [Box<DispatchGroup>]) -> bool {
        self.calls.push(Call::ComputePass(dispatches.len()));
        !self.fail
    }

    fn copy_buffer_to_buffer(
        &mut self,
        _src_buffer: &Arc<Resource<Buffer>>,
        src_offset: usize,
        _dst_buffer: ResourceRef<Buffer>,
        dst_offset: usize,
        size: usize,
    ) -> bool {
        self.calls.push(Call::CopyBufferToBuffer {
            src_offset,
            dst_offset,
            size,
        });
        !self.fail
    }

    fn copy_texture_to_buffer(
        &mut self,
        _texture: ResourceRef<Texture>,
        src_rect: IRect,
        _buffer: ResourceRef<Buffer>,
        buffer_offset: usize,
        buffer_row_bytes: usize,
    ) -> bool {
        self.calls.push(Call::CopyTextureToBuffer {
            src_rect,
            buffer_offset,
            buffer_row_bytes,
        });
        !self.fail
    }

    fn copy_buffer_to_texture(
        &mut self,
        _buffer: &Arc<Resource<Buffer>>,
        _texture: ResourceRef<Texture>,
        copy_data: &[BufferTextureCopyData],
    ) -> bool {
        self.calls
            .push(Call::CopyBufferToTexture(copy_data.to_vec()));
        !self.fail
    }

    fn copy_texture_to_texture(
        &mut self,
        _src: ResourceRef<Texture>,
        src_rect: IRect,
        _dst: ResourceRef<Texture>,
        dst_point: IPoint,
        dst_level: i32,
    ) -> bool {
        self.calls.push(Call::CopyTextureToTexture {
            src_rect,
            dst_point,
            dst_level,
        });
        !self.fail
    }

    fn synchronize_buffer_to_cpu(&mut self, _buffer: ResourceRef<Buffer>) -> bool {
        self.calls.push(Call::SynchronizeBufferToCpu);
        !self.fail
    }

    fn clear_buffer(
        &mut self,
        _buffer: &Arc<Resource<Buffer>>,
        offset: usize,
        size: usize,
    ) -> bool {
        self.calls.push(Call::ClearBuffer { offset, size });
        !self.fail
    }
}

/// A context for tasks: the caps and one resource provider.
pub struct MockContext {
    pub caps: MockCaps,
    pub resource_provider: SharedResourceProvider,
}

impl MockContext {
    pub fn new(caps: MockCaps) -> (Self, BackendCounts) {
        let (resource_provider, counts) = shared_provider();
        (
            Self {
                caps,
                resource_provider,
            },
            counts,
        )
    }
}

impl ContextPriv for MockContext {
    fn caps(&self) -> &dyn Caps {
        &self.caps
    }

    fn resource_provider(&self) -> &SharedResourceProvider {
        &self.resource_provider
    }
}
