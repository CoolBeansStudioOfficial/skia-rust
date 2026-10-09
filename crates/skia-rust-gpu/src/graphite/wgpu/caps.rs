// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnCaps.h, DawnCaps.cpp, and the backend-neutral
//                   parts of src/gpu/graphite/Caps.h, Caps.cpp that `DawnCaps` builds on

//! `WgpuCaps`: the capabilities of a wgpu device (`DawnCaps`).
//!
//! `DawnCaps` derives from the abstract `Caps`; `docs/design/gpu.md` §4.1 makes each such pair one
//! concrete struct, so the pieces of `Caps.cpp` that read the format table (`isTexturable`,
//! `getDefaultTextureInfo`, `getDepthStencilFormat`, …) are methods of [`WgpuCaps`] here, under
//! `// Port of: src/gpu/graphite/Caps.cpp`. The `Caps` trait of `caps.rs` is the narrow view of
//! it the recorder and the buffer managers take.
//!
//! # What the capabilities are computed from
//!
//! `DawnCaps` asks the `wgpu::Device` for its features (`HasFeature`) and limits. Many of the
//! features it reads are Dawn extensions that wgpu does not have (`DawnLoadResolveTexture`,
//! `RenderPassRenderArea`, `MSAARenderToSingleSampled`, `FramebufferFetch`, …). They are
//! [`DeviceFeatures`] flags that are never set when the caps are built from a real wgpu device
//! ([`CapsProfile::from_device`]), and set by the oracle profiles ([`CapsProfile::dawn_d3d12`],
//! [`CapsProfile::dawn_vulkan`]) that reproduce what Dawn reported when it rendered the goldens
//! (`docs/design/gpu.md` §1.3, §6.2). Tests can build any profile, which is how WGSL and pipeline
//! sets are checked without a GPU.
//!
//! # What is not ported yet
//!
//! `makeGraphicsPipelineKey`, `extractGraphicsDescs` and `makeComputePipelineKey` need
//! `GraphicsPipelineDesc`, `UniquePaintParamsID` and `ComputePipelineDesc` (G5/G7/G11b);
//! `getImmutableSamplerInfo` and `toString(ImmutableSamplerInfo)` are YCbCr-only and wgpu has no
//! YCbCr samplers. [`ShaderCaps`] is `SkSL::ShaderCaps`, re-exported with the other
//! backend-neutral half of `Caps` from [`crate::graphite::caps`].

use std::sync::Arc;

use bitflags::bitflags;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::size::ISize;
use skia_rust_core::texture_compression_type::TextureCompressionType;

use crate::gpu::gpu_types::{BackendApi, GpuStatsFlags, Mipmapped, Protected, Renderable};
pub use crate::graphite::caps::{ResourceBindingRequirements, ShaderCaps};
use crate::graphite::caps::{AttachmentSizePolicy, Caps, default_shader_caps};
use crate::graphite::context_options::ContextOptions;
use crate::graphite::graphite_resource_key::{GraphiteResourceKey, GraphiteResourceKeyBuilder};
use crate::graphite::graphite_types::{DepthStencilFlags, SampleCount};
use crate::graphite::render_pass_desc::{AttachmentDesc, RenderPassDesc};
use crate::graphite::resource_types::{
    Discardable, DstReadStrategy, Layout, LoadOp, NUM_SAMPLE_KEY_BITS, ResourceType, TextureUsage,
    Tiling, samples_to_key,
};
use crate::graphite::texture_format::{
    TEXTURE_FORMAT_COUNT, TextureFormat, compression_type_to_texture_format,
    preferred_texture_formats, texture_format_compression_type, texture_format_is_depth_or_stencil,
    write_swizzle_for_color_type,
};
use crate::graphite::texture_info::{TextureInfo, texture_info_priv};
use crate::graphite::wgpu::graphite_utils::{
    FormatFlag, texture_format_support, texture_format_to_wgpu_format,
};
use crate::graphite::wgpu::texture_info::{WgpuTextureInfo, WgpuTextureInfoData, texture_infos};

bitflags! {
    /// The device features `DawnCaps` reads (`wgpu::FeatureName` in Dawn), including the Dawn
    /// extensions that wgpu does not expose.
    // Port of: src/gpu/graphite/dawn/DawnCaps.cpp (the `HasFeature` calls) (chrome/m156)
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct DeviceFeatures: u32 {
        /// `ShaderF16` (`wgpu::Features::SHADER_F16`).
        const SHADER_F16 = 1 << 0;
        /// `DualSourceBlending` (`DUAL_SOURCE_BLENDING`).
        const DUAL_SOURCE_BLENDING = 1 << 1;
        /// `FramebufferFetch`: a Dawn extension.
        const FRAMEBUFFER_FETCH = 1 << 2;
        /// `BufferMapExtendedUsages` (`MAPPABLE_PRIMARY_BUFFERS`).
        const BUFFER_MAP_EXTENDED_USAGES = 1 << 3;
        /// `MSAARenderToSingleSampled`: a Dawn extension.
        const MSAA_RENDER_TO_SINGLE_SAMPLED = 1 << 4;
        /// `TransientAttachments` (`TextureUsages::TRANSIENT_ATTACHMENT` saves memory).
        const TRANSIENT_ATTACHMENTS = 1 << 5;
        /// `DawnLoadResolveTexture`: a Dawn extension (the `ExpandResolveTexture` load op).
        const DAWN_LOAD_RESOLVE_TEXTURE = 1 << 6;
        /// `DawnPartialLoadResolveTexture`: a Dawn extension.
        const DAWN_PARTIAL_LOAD_RESOLVE_TEXTURE = 1 << 7;
        /// `RenderPassRenderArea`: a Dawn extension.
        const RENDER_PASS_RENDER_AREA = 1 << 8;
        /// `DawnAllowUndefinedLoadStoreOp`: a Dawn extension (wgpu's `LoadOp::DontCare` is
        /// `unsafe`, so the wgpu back end never sets it).
        const DAWN_ALLOW_UNDEFINED_LOAD_STORE_OP = 1 << 9;
        /// `TimestampQuery`.
        const TIMESTAMP_QUERY = 1 << 10;
        /// Timestamps can be written between commands of an encoder
        /// (`TIMESTAMP_QUERY_INSIDE_ENCODERS`); Dawn-native writes them on command buffers.
        const TIMESTAMP_QUERY_INSIDE_ENCODERS = 1 << 11;
        /// `DawnTexelCopyBufferRowAlignment`: a Dawn extension; wgpu always needs 256.
        const DAWN_TEXEL_COPY_BUFFER_ROW_ALIGNMENT = 1 << 12;
        /// `CoreFeaturesAndLimits`: not a compatibility-mode device.
        const CORE_FEATURES_AND_LIMITS = 1 << 13;
        /// `TextureFormatsTier1`: a Dawn extension, wgpu has no equivalent yet.
        const TEXTURE_FORMATS_TIER1 = 1 << 14;
        /// `TextureFormatsTier2`: a Dawn extension, wgpu has no equivalent yet.
        const TEXTURE_FORMATS_TIER2 = 1 << 15;
        /// `Unorm16TextureFormats` (`TEXTURE_FORMAT_16BIT_NORM`).
        const UNORM16_TEXTURE_FORMATS = 1 << 16;
        /// `Float32Filterable`.
        const FLOAT32_FILTERABLE = 1 << 17;
        /// `Float32Blendable`.
        const FLOAT32_BLENDABLE = 1 << 18;
        /// `RG11B10UfloatRenderable`.
        const RG11B10UFLOAT_RENDERABLE = 1 << 19;
        /// `BGRA8UnormStorage`.
        const BGRA8UNORM_STORAGE = 1 << 20;
        /// `TextureCompressionBC`.
        const TEXTURE_COMPRESSION_BC = 1 << 21;
        /// `TextureCompressionETC2`.
        const TEXTURE_COMPRESSION_ETC2 = 1 << 22;
    }
}

impl DeviceFeatures {
    /// The features only Dawn has: what the `-wgpucaps` oracle profiles remove
    /// (`docs/design/gpu.md` §3.1, R3).
    pub const DAWN_ONLY: Self = Self::FRAMEBUFFER_FETCH
        .union(Self::MSAA_RENDER_TO_SINGLE_SAMPLED)
        .union(Self::DAWN_LOAD_RESOLVE_TEXTURE)
        .union(Self::DAWN_PARTIAL_LOAD_RESOLVE_TEXTURE)
        .union(Self::RENDER_PASS_RENDER_AREA)
        .union(Self::DAWN_ALLOW_UNDEFINED_LOAD_STORE_OP)
        .union(Self::DAWN_TEXEL_COPY_BUFFER_ROW_ALIGNMENT)
        .union(Self::TEXTURE_FORMATS_TIER1)
        .union(Self::TEXTURE_FORMATS_TIER2);
}

/// The limits `DawnCaps` reads (`wgpu::Limits` in Dawn).
// Port of: src/gpu/graphite/dawn/DawnCaps.cpp#L219-L257 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceLimits {
    /// `maxTextureDimension2D`.
    pub max_texture_dimension_2d: u32,
    /// `minUniformBufferOffsetAlignment`.
    pub min_uniform_buffer_offset_alignment: u32,
    /// `minStorageBufferOffsetAlignment`.
    pub min_storage_buffer_offset_alignment: u32,
    /// `maxInterStageShaderVariables`.
    pub max_inter_stage_shader_variables: u32,
    /// `maxImmediateSize`.
    pub max_immediate_size: u32,
    /// `maxStorageBuffersInVertexStage`.
    pub max_storage_buffers_in_vertex_stage: u32,
    /// `maxStorageBuffersInFragmentStage`.
    pub max_storage_buffers_in_fragment_stage: u32,
    /// `minTexelCopyBufferRowAlignment` (`DawnTexelCopyBufferRowAlignmentLimits`), if the device
    /// reports one.
    pub min_texel_copy_buffer_row_alignment: Option<u32>,
}

impl Default for DeviceLimits {
    /// The WebGPU default limits.
    fn default() -> Self {
        Self {
            max_texture_dimension_2d: 8192,
            min_uniform_buffer_offset_alignment: 256,
            min_storage_buffer_offset_alignment: 256,
            max_inter_stage_shader_variables: 16,
            max_immediate_size: 0,
            max_storage_buffers_in_vertex_stage: 8,
            max_storage_buffers_in_fragment_stage: 8,
            min_texel_copy_buffer_row_alignment: None,
        }
    }
}

/// What [`WgpuCaps`] is computed from: the device facts `DawnCaps` queries.
///
/// A profile built from a real device is [`CapsProfile::from_device`]; the Dawn oracle profiles
/// are [`CapsProfile::dawn_d3d12`] and [`CapsProfile::dawn_vulkan`].
// Port of: src/gpu/graphite/dawn/DawnCaps.cpp#L194-L219 (chrome/m156)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapsProfile {
    /// The profile's name (`dawn-d3d12-warp`, `dawn-vk-lavapipe`, …).
    pub name: String,
    /// `info.backendType`.
    pub backend: wgpu::Backend,
    /// `info.device`: the device name.
    pub device_name: String,
    /// The features the device has.
    pub features: DeviceFeatures,
    /// The device limits.
    pub limits: DeviceLimits,
    /// `DawnBackendContext::fTick != nullptr`: whether the context can wait for the GPU.
    pub has_tick: bool,
}

impl CapsProfile {
    /// The facts of a real wgpu device. `has_tick` is whether the device can be polled
    /// (everywhere except the browser).
    #[must_use]
    pub fn from_device(device: &wgpu::Device, has_tick: bool) -> Self {
        let info = device.adapter_info();
        let wgpu_features = device.features();
        let limits = device.limits();

        let mut features = DeviceFeatures::empty();
        let mut set = |present: bool, feature: DeviceFeatures| {
            if present {
                features |= feature;
            }
        };
        set(
            wgpu_features.contains(wgpu::Features::SHADER_F16),
            DeviceFeatures::SHADER_F16,
        );
        set(
            wgpu_features.contains(wgpu::Features::DUAL_SOURCE_BLENDING),
            DeviceFeatures::DUAL_SOURCE_BLENDING,
        );
        set(
            wgpu_features.contains(wgpu::Features::MAPPABLE_PRIMARY_BUFFERS),
            DeviceFeatures::BUFFER_MAP_EXTENDED_USAGES,
        );
        set(
            info.transient_saves_memory == Some(true),
            DeviceFeatures::TRANSIENT_ATTACHMENTS,
        );
        set(
            wgpu_features.contains(wgpu::Features::TIMESTAMP_QUERY),
            DeviceFeatures::TIMESTAMP_QUERY,
        );
        set(
            wgpu_features.contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS),
            DeviceFeatures::TIMESTAMP_QUERY_INSIDE_ENCODERS,
        );
        // OpenGL is wgpu's compatibility-level backend.
        set(
            info.backend != wgpu::Backend::Gl,
            DeviceFeatures::CORE_FEATURES_AND_LIMITS,
        );
        set(
            wgpu_features.contains(wgpu::Features::TEXTURE_FORMAT_16BIT_NORM),
            DeviceFeatures::UNORM16_TEXTURE_FORMATS,
        );
        set(
            wgpu_features.contains(wgpu::Features::FLOAT32_FILTERABLE),
            DeviceFeatures::FLOAT32_FILTERABLE,
        );
        set(
            wgpu_features.contains(wgpu::Features::FLOAT32_BLENDABLE),
            DeviceFeatures::FLOAT32_BLENDABLE,
        );
        set(
            wgpu_features.contains(wgpu::Features::RG11B10UFLOAT_RENDERABLE),
            DeviceFeatures::RG11B10UFLOAT_RENDERABLE,
        );
        set(
            wgpu_features.contains(wgpu::Features::BGRA8UNORM_STORAGE),
            DeviceFeatures::BGRA8UNORM_STORAGE,
        );
        set(
            wgpu_features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC),
            DeviceFeatures::TEXTURE_COMPRESSION_BC,
        );
        set(
            wgpu_features.contains(wgpu::Features::TEXTURE_COMPRESSION_ETC2),
            DeviceFeatures::TEXTURE_COMPRESSION_ETC2,
        );

        Self {
            name: format!("wgpu-{}", info.backend.to_str()),
            backend: info.backend,
            device_name: info.name,
            features,
            limits: DeviceLimits {
                max_texture_dimension_2d: limits.max_texture_dimension_2d,
                min_uniform_buffer_offset_alignment: limits.min_uniform_buffer_offset_alignment,
                min_storage_buffer_offset_alignment: limits.min_storage_buffer_offset_alignment,
                max_inter_stage_shader_variables: limits.max_inter_stage_shader_variables,
                max_immediate_size: limits.max_immediate_size,
                // wgpu limits storage buffers per shader stage, not per vertex/fragment stage.
                max_storage_buffers_in_vertex_stage: limits.max_storage_buffers_per_shader_stage,
                max_storage_buffers_in_fragment_stage: limits.max_storage_buffers_per_shader_stage,
                min_texel_copy_buffer_row_alignment: None,
            },
            has_tick,
        }
    }

    /// What Dawn reported on D3D12 when it rendered the `gpu-dawn-d3d12-*` goldens
    /// (`docs/design/gpu.md` §1.3): no `ShaderF16` (Skia builds Dawn without DXC), storage
    /// buffers, 64 bytes of immediates, dual-source blending, `DawnLoadResolveTexture` with its
    /// partial variant, `RenderPassRenderArea`, `DawnAllowUndefinedLoadStoreOp`,
    /// `TextureFormatsTier1` and `Unorm16TextureFormats`.
    ///
    /// The numeric limits other than `maxImmediateSize` are the WebGPU defaults, to be replaced
    /// by the oracle's `caps.json` (task G0b).
    #[must_use]
    pub fn dawn_d3d12() -> Self {
        Self {
            name: "dawn-d3d12".to_owned(),
            backend: wgpu::Backend::Dx12,
            device_name: String::new(),
            features: DeviceFeatures::DUAL_SOURCE_BLENDING
                | DeviceFeatures::DAWN_LOAD_RESOLVE_TEXTURE
                | DeviceFeatures::DAWN_PARTIAL_LOAD_RESOLVE_TEXTURE
                | DeviceFeatures::RENDER_PASS_RENDER_AREA
                | DeviceFeatures::DAWN_ALLOW_UNDEFINED_LOAD_STORE_OP
                | DeviceFeatures::TEXTURE_FORMATS_TIER1
                | DeviceFeatures::UNORM16_TEXTURE_FORMATS
                | DeviceFeatures::CORE_FEATURES_AND_LIMITS
                | DeviceFeatures::TIMESTAMP_QUERY,
            limits: DeviceLimits {
                max_immediate_size: 64,
                ..DeviceLimits::default()
            },
            has_tick: true,
        }
    }

    /// What Dawn reported on Vulkan when it rendered the `gpu-dawn-vk-*` goldens
    /// (`docs/design/gpu.md` §1.3): `ShaderF16` (NVIDIA), no storage buffers (Skia excludes Vulkan
    /// from them), 64 bytes of immediates, dual-source blending, `DawnLoadResolveTexture`,
    /// `Unorm16TextureFormats` and `TextureFormatsTier1`. The partial load resolve and render-area
    /// features are D3D12-only in that note.
    ///
    /// The numeric limits other than `maxImmediateSize` are the WebGPU defaults, to be replaced
    /// by the oracle's `caps.json` (task G0b).
    #[must_use]
    pub fn dawn_vulkan() -> Self {
        Self {
            name: "dawn-vk".to_owned(),
            backend: wgpu::Backend::Vulkan,
            device_name: String::new(),
            features: DeviceFeatures::SHADER_F16
                | DeviceFeatures::DUAL_SOURCE_BLENDING
                | DeviceFeatures::DAWN_LOAD_RESOLVE_TEXTURE
                | DeviceFeatures::DAWN_ALLOW_UNDEFINED_LOAD_STORE_OP
                | DeviceFeatures::TEXTURE_FORMATS_TIER1
                | DeviceFeatures::UNORM16_TEXTURE_FORMATS
                | DeviceFeatures::CORE_FEATURES_AND_LIMITS
                | DeviceFeatures::TIMESTAMP_QUERY,
            limits: DeviceLimits {
                max_immediate_size: 64,
                ..DeviceLimits::default()
            },
            has_tick: true,
        }
    }

    /// The same device with every Dawn-only feature removed: the `-wgpucaps` variant of an
    /// oracle profile, i.e. what wgpu can express (`docs/design/gpu.md` §3.1).
    #[must_use]
    pub fn wgpu_restricted(&self) -> Self {
        Self {
            name: format!("{}-wgpucaps", self.name),
            features: self.features - DeviceFeatures::DAWN_ONLY,
            ..self.clone()
        }
    }
}

bitflags! {
    /// A set of [`SampleCount`]s (`SkEnumBitMask<SampleCount>`; the counts are powers of two).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct SampleCounts: u8 {
        /// `SampleCount::k1`.
        const ONE = 1;
        /// `SampleCount::k2`.
        const TWO = 2;
        /// `SampleCount::k4`.
        const FOUR = 4;
        /// `SampleCount::k8`.
        const EIGHT = 8;
        /// `SampleCount::k16`.
        const SIXTEEN = 16;
    }
}

impl From<SampleCount> for SampleCounts {
    fn from(count: SampleCount) -> Self {
        Self::from_bits_retain(count as u8)
    }
}

/// `Caps::FormatSupport`: the usages and sample counts a format supports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FormatSupport {
    /// The supported usages.
    pub usage: TextureUsage,
    /// The supported sample counts.
    pub sample_counts: SampleCounts,
}

/// `wgpu::LoadOp::ExpandResolveTexture`, the Dawn load op that loads the resolve texture into the
/// MSAA attachment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolveTextureLoadOp {
    /// `ExpandResolveTexture`.
    ExpandResolveTexture,
}

/// The equivalent Dawn `LoadOp` of Graphite's `Discard`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscardLoadOp {
    /// `wgpu::LoadOp::Clear`.
    Clear,
    /// `wgpu::LoadOp::Undefined` (`DawnAllowUndefinedLoadStoreOp`).
    Undefined,
}

/// The equivalent Dawn `StoreOp` of Graphite's `Discard`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscardStoreOp {
    /// `wgpu::StoreOp::Discard`.
    Discard,
    /// `wgpu::StoreOp::Undefined` (`DawnAllowUndefinedLoadStoreOp`).
    Undefined,
}

// TextureFormat is backed by a uint8_t, so 8 bits are always sufficient (including using
// kUnsupported) to represent an unused attachment. To make room for the load-from-resolve bit, we
// reduce the uint8_t of fSampleCount to 3 bits with the SampleToKey function (x2 attachments)
// Port of: src/gpu/graphite/dawn/DawnCaps.cpp#L452-L475 (chrome/m156)
const FORMAT_BITS: u32 = 8; // x2 attachments (color & depthStencil formats)
const RESOLVE_BITS: u32 = 1;

const _: () = assert!(2 * (FORMAT_BITS + NUM_SAMPLE_KEY_BITS) + RESOLVE_BITS <= 32);
const _: () = assert!(TEXTURE_FORMAT_COUNT < 1 << FORMAT_BITS);

const DEPTH_STENCIL_NUM_SAMPLES_OFFSET: u32 = /* loadResolveOffset=0 + */ RESOLVE_BITS;
const DEPTH_STENCIL_FORMAT_OFFSET: u32 = DEPTH_STENCIL_NUM_SAMPLES_OFFSET + NUM_SAMPLE_KEY_BITS;
const COLOR_NUM_SAMPLES_OFFSET: u32 = DEPTH_STENCIL_FORMAT_OFFSET + FORMAT_BITS;
const COLOR_FORMAT_OFFSET: u32 = COLOR_NUM_SAMPLES_OFFSET + NUM_SAMPLE_KEY_BITS;
const ADDITIONAL_FLAG_OFFSET: u32 = COLOR_FORMAT_OFFSET + FORMAT_BITS;
const _: () = assert!(ADDITIONAL_FLAG_OFFSET <= 31);

// Port of: src/gpu/graphite/Caps.cpp#L76-L80 (chrome/m156)
const MAX_FALLBACK_TEXTURE_SIZE: i32 = 8192;
/// 4 floats (RGBA32F) per fallback texel.
const BYTES_PER_TEXEL: i32 = 16;

/// `DawnGraphicsPipeline::kUniformBufferBindGroupIndex`.
pub const UNIFORM_BUFFER_BIND_GROUP_INDEX: u32 = 0;
/// `DawnGraphicsPipeline::kTextureBindGroupIndex`.
pub const TEXTURE_BIND_GROUP_INDEX: u32 = 1;
/// `DawnGraphicsPipeline::kIntrinsicUniformBufferIndex`.
pub const INTRINSIC_UNIFORM_BUFFER_INDEX: u32 = 0;
/// `DawnGraphicsPipeline::kCombinedUniformIndex`.
pub const COMBINED_UNIFORM_INDEX: u32 = 1;
/// `DawnGraphicsPipeline::kStorageBufferIndex`.
pub const STORAGE_BUFFER_INDEX: u32 = 2;
/// `DawnGraphicsPipeline::kMaxNumUniformBuffers`.
pub const MAX_NUM_UNIFORM_BUFFERS: u32 = 3;
/// `DawnGraphicsPipeline::kIntrinsicUniformSize`.
pub const INTRINSIC_UNIFORM_SIZE: u32 = 32;

/// The capabilities of a wgpu device.
// Port of: src/gpu/graphite/dawn/DawnCaps.h#L22-L90, src/gpu/graphite/Caps.h (chrome/m156)
#[doc(alias = "DawnCaps")]
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)] // mirrors the C++ class
pub struct WgpuCaps {
    profile: CapsProfile,

    // The backend-neutral `Caps` state.
    format_support: Box<[[FormatSupport; TEXTURE_FORMAT_COUNT]; 2]>,
    max_texture_size: i32,
    required_uniform_buffer_alignment: usize,
    required_storage_buffer_alignment: usize,
    required_transfer_buffer_alignment: usize,
    texture_data_row_bytes_alignment: usize,
    max_varyings: i32,
    shader_caps: ShaderCaps,
    ndc_y_axis_points_down: bool,
    clamp_to_border_support: bool,
    protected_support: bool,
    semaphore_support: bool,
    allow_cpu_sync: bool,
    storage_buffer_support: bool,
    draw_buffer_can_be_mapped: bool,
    buffer_maps_are_async: bool,
    msaa_render_to_single_sampled_support: bool,
    avoid_msaa: bool,
    draw_list_layer: bool,
    avoid_depth_mode: bool,
    compute_support: bool,
    storage_buffer_support_for_compute: bool,
    full_compressed_upload_size_must_align_to_block_dims: bool,
    resource_binding_reqs: ResourceBindingRequirements,
    attachment_size_policy: AttachmentSizePolicy,
    supported_gpu_stats: GpuStatsFlags,
    max_internal_sample_count: SampleCount,
    glyph_cache_texture_maximum_bytes: usize,
    min_msaa_path_size: f32,
    min_distance_field_font_size: f32,
    glyphs_as_paths_font_size: f32,
    max_path_atlas_texture_size: i32,
    allow_multiple_atlas_textures: bool,
    support_bilerp_from_glyph_atlas: bool,
    require_ordered_recordings: bool,
    set_backend_labels: bool,

    // The `DawnCaps` state.
    supported_transient_attachment_usage: wgpu::TextureUsages,
    supported_resolve_texture_load_op: Option<ResolveTextureLoadOp>,
    supports_partial_load_resolve: bool,
    supports_render_pass_render_area: bool,
    discard_load_op: DiscardLoadOp,
    discard_store_op: DiscardStoreOp,
    emulate_load_store_resolve: bool,
    use_async_pipeline_creation: bool,
    allow_scoped_error_checks: bool,
    supports_command_buffer_timestamps: bool,
    supports_half_precision: bool,
}

impl WgpuCaps {
    /// `DawnCaps(backendContext, options)`: computes the capabilities of the device described by
    /// `profile`.
    // Port of: src/gpu/graphite/dawn/DawnCaps.cpp#L96-L103 (chrome/m156)
    #[must_use]
    pub fn new(profile: &CapsProfile, options: &ContextOptions) -> Self {
        let mut caps = Self {
            profile: profile.clone(),
            format_support: Box::new([[FormatSupport::default(); TEXTURE_FORMAT_COUNT]; 2]),
            max_texture_size: 0,
            required_uniform_buffer_alignment: 0,
            required_storage_buffer_alignment: 0,
            required_transfer_buffer_alignment: 0,
            texture_data_row_bytes_alignment: 1,
            max_varyings: 0,
            shader_caps: default_shader_caps(),
            ndc_y_axis_points_down: false,
            clamp_to_border_support: true,
            protected_support: false,
            semaphore_support: false,
            allow_cpu_sync: true,
            storage_buffer_support: false,
            draw_buffer_can_be_mapped: true,
            buffer_maps_are_async: false,
            msaa_render_to_single_sampled_support: false,
            avoid_msaa: false,
            draw_list_layer: false,
            avoid_depth_mode: false,
            compute_support: false,
            storage_buffer_support_for_compute: false,
            full_compressed_upload_size_must_align_to_block_dims: false,
            resource_binding_reqs: ResourceBindingRequirements::default(),
            attachment_size_policy: AttachmentSizePolicy::Exact,
            supported_gpu_stats: GpuStatsFlags::empty(),
            max_internal_sample_count: SampleCount::Four,
            glyph_cache_texture_maximum_bytes: 2048 * 1024 * 4,
            min_msaa_path_size: 0.0,
            min_distance_field_font_size: 18.0,
            glyphs_as_paths_font_size: 324.0,
            max_path_atlas_texture_size: 8192,
            allow_multiple_atlas_textures: true,
            support_bilerp_from_glyph_atlas: false,
            require_ordered_recordings: false,
            set_backend_labels: false,

            supported_transient_attachment_usage: wgpu::TextureUsages::empty(),
            supported_resolve_texture_load_op: None,
            supports_partial_load_resolve: false,
            supports_render_pass_render_area: false,
            discard_load_op: DiscardLoadOp::Clear,
            discard_store_op: DiscardStoreOp::Discard,
            emulate_load_store_resolve: false,
            use_async_pipeline_creation: true,
            allow_scoped_error_checks: true,
            supports_command_buffer_timestamps: false,
            supports_half_precision: false,
        };
        caps.init_caps(options);
        caps.init_shader_caps();
        caps.init_format_table();
        caps.finish_initialization(options);
        caps
    }

    // Port of: src/gpu/graphite/dawn/DawnCaps.cpp#L204-L441 (chrome/m156)
    fn init_caps(&mut self, options: &ContextOptions) {
        let _ = options;
        let features = self.profile.features;
        let limits = self.profile.limits;
        let backend = self.profile.backend;

        self.max_texture_size = i32::try_from(limits.max_texture_dimension_2d).unwrap_or(i32::MAX);

        self.required_transfer_buffer_alignment = 4;
        self.required_uniform_buffer_alignment =
            limits.min_uniform_buffer_offset_alignment as usize;
        self.required_storage_buffer_alignment =
            limits.min_storage_buffer_offset_alignment as usize;

        self.max_varyings =
            i32::try_from(limits.max_inter_stage_shader_variables).unwrap_or(i32::MAX);

        // Dawn requires 256 bytes per row alignment for buffer texture copies.
        self.texture_data_row_bytes_alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        // If the device supports the DawnTexelCopyBufferRowAlignment feature, the alignment can be
        // queried from its limits.
        if features.contains(DeviceFeatures::DAWN_TEXEL_COPY_BUFFER_ROW_ALIGNMENT)
            && let Some(alignment) = limits.min_texel_copy_buffer_row_alignment
        {
            self.texture_data_row_bytes_alignment = alignment as usize;
        }

        self.supports_half_precision = features.contains(DeviceFeatures::SHADER_F16);
        self.resource_binding_reqs.backend_api = BackendApi::Dawn;
        // The WGSL generator assumes tightly packed std430 layout for SSBOs which is also the
        // default for all types outside the uniform address space in WGSL (which is emulated to
        // 140 by SkSL's WGSL generation). If ShaderF16 is supported, we switch the layout to
        // upload half data.
        self.resource_binding_reqs.uniform_buffer_layout = if self.supports_half_precision {
            Layout::Std140F16
        } else {
            Layout::Std140
        };
        self.resource_binding_reqs.storage_buffer_layout = if self.supports_half_precision {
            Layout::Std430F16
        } else {
            Layout::Std430
        };
        self.resource_binding_reqs
            .separate_texture_and_sampler_binding = true;

        // We need 32 bytes push constant for 2 vectors worth of intrinsic data.
        self.resource_binding_reqs
            .use_push_constants_for_intrinsic_constants =
            limits.max_immediate_size >= INTRINSIC_UNIFORM_SIZE;

        self.resource_binding_reqs.uniforms_set_idx = UNIFORM_BUFFER_BIND_GROUP_INDEX.cast_signed();
        self.resource_binding_reqs.texture_sampler_set_idx = TEXTURE_BIND_GROUP_INDEX.cast_signed();

        self.resource_binding_reqs.intrinsic_buffer_binding =
            INTRINSIC_UNIFORM_BUFFER_INDEX.cast_signed();
        self.resource_binding_reqs.combined_uniform_buffer_binding =
            COMBINED_UNIFORM_INDEX.cast_signed();
        self.resource_binding_reqs.storage_buffer_binding = STORAGE_BUFFER_INDEX.cast_signed();

        // We need at least 4 SSBOs for intrinsic, render step, paint & gradient buffers.
        // TODO(b/418235681): Enable SSBOs after fixing performance regressions for Dawn/Vulkan.
        self.storage_buffer_support = backend != wgpu::Backend::Gl
            && backend != wgpu::Backend::Vulkan
            && limits.max_storage_buffers_in_vertex_stage >= 4
            && limits.max_storage_buffers_in_fragment_stage >= 4;

        self.draw_buffer_can_be_mapped = false;

        self.compute_support = true;

        // TODO: support clamp to border.
        self.clamp_to_border_support = false;

        // We use async map.
        self.buffer_maps_are_async = true;

        self.draw_buffer_can_be_mapped =
            features.contains(DeviceFeatures::BUFFER_MAP_EXTENDED_USAGES);

        self.msaa_render_to_single_sampled_support =
            features.contains(DeviceFeatures::MSAA_RENDER_TO_SINGLE_SAMPLED);

        if features.contains(DeviceFeatures::TRANSIENT_ATTACHMENTS) {
            self.supported_transient_attachment_usage = wgpu::TextureUsages::TRANSIENT_ATTACHMENT;
        }

        if features.contains(DeviceFeatures::DAWN_LOAD_RESOLVE_TEXTURE) {
            self.supported_resolve_texture_load_op =
                Some(ResolveTextureLoadOp::ExpandResolveTexture);
            self.supports_partial_load_resolve =
                features.contains(DeviceFeatures::DAWN_PARTIAL_LOAD_RESOLVE_TEXTURE);
            if self.supports_partial_load_resolve {
                // This extension allows the MSAA attachments to be smaller than the main target.
                self.attachment_size_policy = AttachmentSizePolicy::MsaaRenderArea;
            }
        }

        self.supports_render_pass_render_area =
            features.contains(DeviceFeatures::RENDER_PASS_RENDER_AREA);

        if features.contains(DeviceFeatures::DAWN_ALLOW_UNDEFINED_LOAD_STORE_OP) {
            self.discard_load_op = DiscardLoadOp::Undefined;
            self.discard_store_op = DiscardStoreOp::Undefined;
        }

        if !self.supports_partial_load_resolve
            && self.supported_transient_attachment_usage.is_empty()
        {
            // If the device doesn't support partial resolve nor transient attachments, we will
            // emulate load/resolve using separate render passes. This helps reuse MSAA textures
            // better to reduce memory usage. Since they are separate render passes, there is no
            // more requirement for the auxiliary attachments to match the main target's
            // dimensions.
            self.emulate_load_store_resolve = true;
            self.attachment_size_policy = AttachmentSizePolicy::MsaaRenderArea;

            // On hardware that doesn't support transient attachments or partial resolve, we
            // force-disable the ExpandResolveTexture loadOp. This is done because, under
            // emulation, ExpandResolveTexture isn't used, and fully disabling it prevents the
            // precompilation API from generating duplicate pipeline permutations (one for load
            // resolve texture, one for others).
            self.supported_resolve_texture_load_op = None;
        }

        if features.contains(DeviceFeatures::TIMESTAMP_QUERY) {
            // Native Dawn has an API for writing timestamps on command buffers; wgpu's
            // equivalent is writing timestamps inside encoders.
            // TODO(b/42240559): On Apple silicon, the timer queries don't have the correct
            // dependencies to measure all the encoders that the start/end commands encapsulate in
            // the commandbuffer.
            self.supports_command_buffer_timestamps = features
                .contains(DeviceFeatures::TIMESTAMP_QUERY_INSIDE_ENCODERS)
                && backend != wgpu::Backend::Metal;

            self.supported_gpu_stats |= GpuStatsFlags::ELAPSED_TIME;
        }

        if !self.profile.has_tick {
            self.allow_cpu_sync = false;
            // This seems paradoxical. However, if we use the async pipeline creation methods
            // (e.g Device::CreateRenderPipelineAsync) then we may have to synchronize before a
            // submit that uses the pipeline. If we use the methods that look synchronous (e.g
            // Device::CreateRenderPipeline) they actually operate asynchronously on WebGPU but the
            // browser becomes responsible for synchronizing when we call submit.
            self.use_async_pipeline_creation = false;

            // The implementation busy waits after popping.
            self.allow_scoped_error_checks = false;
        }

        self.full_compressed_upload_size_must_align_to_block_dims = true;
    }

    // Port of: src/gpu/graphite/dawn/DawnCaps.cpp#L457-L470 (chrome/m156)
    fn init_shader_caps(&mut self) {
        let features = self.profile.features;
        let shader_caps = &mut self.shader_caps;

        // WGSL does not actually support infinities regardless of hardware support. There are
        // discussions around enabling it using an extension in the future.
        shader_caps.infinity_support = false;

        if features.contains(DeviceFeatures::DUAL_SOURCE_BLENDING) {
            shader_caps.dual_source_blending_support = true;
        }
        if features.contains(DeviceFeatures::FRAMEBUFFER_FETCH) {
            shader_caps.fb_fetch_support = true;
        }
    }

    // Port of: src/gpu/graphite/dawn/DawnCaps.cpp#L108-L168 (chrome/m156)
    fn init_format_table(&mut self) {
        let features = self.profile.features;
        for (i, &tf) in TextureFormat::ALL.iter().enumerate() {
            let Some(format) = texture_format_to_wgpu_format(tf) else {
                continue;
            };
            let format_caps = texture_format_support(features, format);

            // The Dawn backend currently only supports optimal tiling
            if format_caps.is_empty() {
                debug_assert_eq!(
                    self.format_support[Tiling::Optimal as usize][i],
                    FormatSupport::default()
                );
                continue;
            }

            let support = &mut self.format_support[Tiling::Optimal as usize][i];

            // At this point, we can claim at least single sample and read is supported; all
            // valid formats in WebGPU support TextureBinding (nearest sampling / texel reads).
            // See https://gpuweb.github.io/gpuweb/#texture-format-caps
            support.usage |= TextureUsage::READ;
            support.sample_counts = SampleCounts::ONE;

            if format_caps.contains(FormatFlag::FILTER) {
                support.usage |= TextureUsage::SAMPLE;
            }

            if format_caps.contains(FormatFlag::RENDER) {
                support.usage |= TextureUsage::RENDER;

                let mut msaa_flag = FormatFlag::MSAA;
                if !texture_format_is_depth_or_stencil(tf) {
                    // We never resolve depth/stencil, but Graphite assumes we resolve to color
                    // formats
                    msaa_flag |= FormatFlag::RESOLVE;
                }
                if format_caps.contains(msaa_flag) {
                    // WebGPU only supports 1x and 4x MSAA
                    support.sample_counts |= SampleCounts::FOUR;
                    if self.msaa_render_to_single_sampled_support
                        && !texture_format_is_depth_or_stencil(tf)
                    {
                        // If WebGPU exposes the MSRTSS extension, assume that all color formats
                        // that support MSAA can support MSRTSS.
                        support.usage |= TextureUsage::MSRTSS;
                    }
                }
            }

            // For now, support kStorage usage if the format has both read-only and write-only
            // capabilities, but we don't require that it supports simultaneous read/write in a
            // binding.
            if format_caps.contains(FormatFlag::READ_ONLY)
                && format_caps.contains(FormatFlag::WRITE_ONLY)
            {
                support.usage |= TextureUsage::STORAGE;
            }

            if texture_format_compression_type(tf) != TextureCompressionType::None {
                // Compressed textures can be copied into, but disallow copying out
                support.usage |= TextureUsage::COPY_DST;
            } else if tf != TextureFormat::External {
                // Plain textures can be copied into and out of
                support.usage |= TextureUsage::COPY_SRC | TextureUsage::COPY_DST;
            } // else no copying for external texture formats
        }
    }

    // Port of: src/gpu/graphite/Caps.cpp#L46-L88 (chrome/m156)
    fn finish_initialization(&mut self, options: &ContextOptions) {
        self.max_internal_sample_count = options.internal_multisample_count;

        self.glyph_cache_texture_maximum_bytes = options.glyph_cache_texture_maximum_bytes;
        self.min_msaa_path_size = options.minimum_path_size_for_msaa;
        self.min_distance_field_font_size = options.min_distance_field_font_size;
        self.glyphs_as_paths_font_size = options.glyphs_as_paths_font_size;
        self.max_path_atlas_texture_size = options.max_path_atlas_texture_size;
        self.allow_multiple_atlas_textures = options.allow_multiple_atlas_textures;
        self.support_bilerp_from_glyph_atlas = options.support_bilerp_from_glyph_atlas;
        self.require_ordered_recordings = options.require_ordered_recordings;
        self.set_backend_labels = options.set_backend_labels;
        self.avoid_depth_mode = options.avoid_depth_mode;

        // Enable setting this flag from either the private or public context options.
        self.draw_list_layer |= options.use_draw_list_layer;

        self.resource_binding_reqs.max_fallback_texture_size =
            MAX_FALLBACK_TEXTURE_SIZE.min(self.max_texture_size).max(1);

        self.resource_binding_reqs.max_fallback_texture_bytes = self
            .resource_binding_reqs
            .max_fallback_texture_size
            .saturating_mul(self.resource_binding_reqs.max_fallback_texture_size)
            .saturating_mul(BYTES_PER_TEXEL);
    }

    /// The profile the caps were computed from.
    #[must_use]
    pub fn profile(&self) -> &CapsProfile {
        &self.profile
    }

    // ---- DawnCaps.h accessors ----------------------------------------------------------------

    /// `supportsHalfPrecision()`.
    #[doc(alias = "supportsHalfPrecision")]
    #[must_use]
    pub fn supports_half_precision(&self) -> bool {
        self.supports_half_precision
    }

    /// `useAsyncPipelineCreation()`.
    #[doc(alias = "useAsyncPipelineCreation")]
    #[must_use]
    pub fn use_async_pipeline_creation(&self) -> bool {
        self.use_async_pipeline_creation
    }

    /// `allowScopedErrorChecks()`.
    #[doc(alias = "allowScopedErrorChecks")]
    #[must_use]
    pub fn allow_scoped_error_checks(&self) -> bool {
        self.allow_scoped_error_checks
    }

    /// `resolveTextureLoadOp()`: if this has no value then loading the resolve texture via a
    /// `LoadOp` is not supported.
    #[doc(alias = "resolveTextureLoadOp")]
    #[must_use]
    pub fn resolve_texture_load_op(&self) -> Option<ResolveTextureLoadOp> {
        self.supported_resolve_texture_load_op
    }

    /// `supportsPartialLoadResolve()`.
    #[doc(alias = "supportsPartialLoadResolve")]
    #[must_use]
    pub fn supports_partial_load_resolve(&self) -> bool {
        self.supports_partial_load_resolve
    }

    /// `supportsRenderPassRenderArea()`.
    #[doc(alias = "supportsRenderPassRenderArea")]
    #[must_use]
    pub fn supports_render_pass_render_area(&self) -> bool {
        self.supports_render_pass_render_area
    }

    /// `discardLoadOp()`: the equivalent Dawn `LoadOp` to Discard.
    #[doc(alias = "discardLoadOp")]
    #[must_use]
    pub fn discard_load_op(&self) -> DiscardLoadOp {
        self.discard_load_op
    }

    /// `discardStoreOp()`: the equivalent Dawn `StoreOp` to Discard.
    #[doc(alias = "discardStoreOp")]
    #[must_use]
    pub fn discard_store_op(&self) -> DiscardStoreOp {
        self.discard_store_op
    }

    /// `loadOpAffectsMSAAPipelines()`.
    // Port of: src/gpu/graphite/dawn/DawnCaps.h#L56-L58 (chrome/m156)
    #[doc(alias = "loadOpAffectsMSAAPipelines")]
    #[must_use]
    pub fn load_op_affects_msaa_pipelines(&self) -> bool {
        self.supported_resolve_texture_load_op.is_some()
    }

    /// `supportsCommandBufferTimestamps()`.
    #[doc(alias = "supportsCommandBufferTimestamps")]
    #[must_use]
    pub fn supports_command_buffer_timestamps(&self) -> bool {
        self.supports_command_buffer_timestamps
    }

    /// `emulateLoadStoreResolve()`: whether we should emulate load/resolve with separate render
    /// passes.
    #[doc(alias = "emulateLoadStoreResolve")]
    #[must_use]
    pub fn emulate_load_store_resolve(&self) -> bool {
        self.emulate_load_store_resolve
    }

    /// The `TransientAttachment` usage when supported, otherwise empty
    /// (`fSupportedTransientAttachmentUsage`).
    #[must_use]
    pub fn supported_transient_attachment_usage(&self) -> wgpu::TextureUsages {
        self.supported_transient_attachment_usage
    }

    // ---- Caps.h accessors --------------------------------------------------------------------

    /// `maxTextureSize()`.
    #[doc(alias = "maxTextureSize")]
    #[must_use]
    pub fn max_texture_size(&self) -> i32 {
        self.max_texture_size
    }

    /// `requireOrderedRecordings()`.
    #[doc(alias = "requireOrderedRecordings")]
    #[must_use]
    pub fn require_ordered_recordings(&self) -> bool {
        self.require_ordered_recordings
    }

    /// `drawBufferCanBeMapped()`: whether a draw buffer can be mapped.
    #[doc(alias = "drawBufferCanBeMapped")]
    #[must_use]
    pub fn draw_buffer_can_be_mapped(&self) -> bool {
        self.draw_buffer_can_be_mapped
    }

    /// `bufferMapsAreAsync()`: whether `Buffer::asyncMap()` must be used to map buffers.
    #[doc(alias = "bufferMapsAreAsync")]
    #[must_use]
    pub fn buffer_maps_are_async(&self) -> bool {
        self.buffer_maps_are_async
    }

    /// `requiredUniformBufferAlignment()`.
    #[doc(alias = "requiredUniformBufferAlignment")]
    #[must_use]
    pub fn required_uniform_buffer_alignment(&self) -> usize {
        self.required_uniform_buffer_alignment
    }

    /// `requiredStorageBufferAlignment()`.
    #[doc(alias = "requiredStorageBufferAlignment")]
    #[must_use]
    pub fn required_storage_buffer_alignment(&self) -> usize {
        self.required_storage_buffer_alignment
    }

    /// `requiredTransferBufferAlignment()`.
    #[doc(alias = "requiredTransferBufferAlignment")]
    #[must_use]
    pub fn required_transfer_buffer_alignment(&self) -> usize {
        self.required_transfer_buffer_alignment
    }

    /// `fullCompressedUploadSizeMustAlignToBlockDims()`.
    #[doc(alias = "fullCompressedUploadSizeMustAlignToBlockDims")]
    #[must_use]
    pub fn full_compressed_upload_size_must_align_to_block_dims(&self) -> bool {
        self.full_compressed_upload_size_must_align_to_block_dims
    }

    /// `avoidDepthMode()`.
    #[doc(alias = "avoidDepthMode")]
    #[must_use]
    pub fn avoid_depth_mode(&self) -> bool {
        self.avoid_depth_mode
    }

    /// `attachmentSizePolicy()`.
    #[doc(alias = "attachmentSizePolicy")]
    #[must_use]
    pub fn attachment_size_policy(&self) -> AttachmentSizePolicy {
        self.attachment_size_policy
    }

    /// `shaderCaps()`.
    #[doc(alias = "shaderCaps")]
    #[must_use]
    pub fn shader_caps(&self) -> &ShaderCaps {
        &self.shader_caps
    }

    /// `avoidMSAA()`.
    // Port of: src/gpu/graphite/Caps.h#L151-L153 (chrome/m156)
    #[doc(alias = "avoidMSAA")]
    #[must_use]
    pub fn avoid_msaa(&self) -> bool {
        self.avoid_msaa
            || self.max_internal_sample_count == SampleCount::One
            || self.avoid_depth_mode
    }

    /// `msaaRenderToSingleSampledSupport()`.
    #[doc(alias = "msaaRenderToSingleSampledSupport")]
    #[must_use]
    pub fn msaa_render_to_single_sampled_support(&self) -> bool {
        self.msaa_render_to_single_sampled_support
    }

    /// `useDrawListLayer()`.
    #[doc(alias = "useDrawListLayer")]
    #[must_use]
    pub fn use_draw_list_layer(&self) -> bool {
        self.draw_list_layer
    }

    /// `resourceBindingRequirements()`.
    #[doc(alias = "resourceBindingRequirements")]
    #[must_use]
    pub fn resource_binding_requirements(&self) -> &ResourceBindingRequirements {
        &self.resource_binding_reqs
    }

    /// `maxVaryings()`.
    #[doc(alias = "maxVaryings")]
    #[must_use]
    pub fn max_varyings(&self) -> i32 {
        self.max_varyings
    }

    /// `ndcYAxisPointsDown()`.
    #[doc(alias = "ndcYAxisPointsDown")]
    #[must_use]
    pub fn ndc_y_axis_points_down(&self) -> bool {
        self.ndc_y_axis_points_down
    }

    /// `clampToBorderSupport()`.
    #[doc(alias = "clampToBorderSupport")]
    #[must_use]
    pub fn clamp_to_border_support(&self) -> bool {
        self.clamp_to_border_support
    }

    /// `protectedSupport()`.
    #[doc(alias = "protectedSupport")]
    #[must_use]
    pub fn protected_support(&self) -> bool {
        self.protected_support
    }

    /// `semaphoreSupport()`.
    #[doc(alias = "semaphoreSupport")]
    #[must_use]
    pub fn semaphore_support(&self) -> bool {
        self.semaphore_support
    }

    /// `allowCpuSync()`: if false then calling `Context::submit` with `SyncToCpu::kYes` is an
    /// error.
    #[doc(alias = "allowCpuSync")]
    #[must_use]
    pub fn allow_cpu_sync(&self) -> bool {
        self.allow_cpu_sync
    }

    /// `storageBufferSupport()`: whether storage buffers are supported and to be preferred over
    /// uniform buffers.
    // Port of: src/gpu/graphite/Caps.h#L321-L328 (chrome/m156)
    #[doc(alias = "storageBufferSupport")]
    #[must_use]
    pub fn storage_buffer_support(&self) -> bool {
        debug_assert!(
            !self.storage_buffer_support
                || matches!(
                    self.resource_binding_reqs.storage_buffer_layout,
                    Layout::Std430 | Layout::Std430F16 | Layout::Metal
                )
        );
        self.storage_buffer_support
    }

    /// `storageBufferSupportForCompute()`.
    #[doc(alias = "storageBufferSupportForCompute")]
    #[must_use]
    pub fn storage_buffer_support_for_compute(&self) -> bool {
        self.storage_buffer_support_for_compute
    }

    /// `computeSupport()`.
    #[doc(alias = "computeSupport")]
    #[must_use]
    pub fn compute_support(&self) -> bool {
        self.compute_support
    }

    /// `supportedGpuStats()`.
    #[doc(alias = "supportedGpuStats")]
    #[must_use]
    pub fn supported_gpu_stats(&self) -> GpuStatsFlags {
        self.supported_gpu_stats
    }

    /// `minPathSizeForMSAA()`.
    #[doc(alias = "minPathSizeForMSAA")]
    #[must_use]
    pub fn min_path_size_for_msaa(&self) -> f32 {
        self.min_msaa_path_size
    }

    /// `minDistanceFieldFontSize()`.
    #[doc(alias = "minDistanceFieldFontSize")]
    #[must_use]
    pub fn min_distance_field_font_size(&self) -> f32 {
        self.min_distance_field_font_size
    }

    /// `glyphsAsPathsFontSize()`.
    #[doc(alias = "glyphsAsPathsFontSize")]
    #[must_use]
    pub fn glyphs_as_paths_font_size(&self) -> f32 {
        self.glyphs_as_paths_font_size
    }

    /// `glyphCacheTextureMaximumBytes()`.
    #[doc(alias = "glyphCacheTextureMaximumBytes")]
    #[must_use]
    pub fn glyph_cache_texture_maximum_bytes(&self) -> usize {
        self.glyph_cache_texture_maximum_bytes
    }

    /// `maxPathAtlasTextureSize()`.
    #[doc(alias = "maxPathAtlasTextureSize")]
    #[must_use]
    pub fn max_path_atlas_texture_size(&self) -> i32 {
        self.max_path_atlas_texture_size
    }

    /// `allowMultipleAtlasTextures()`.
    #[doc(alias = "allowMultipleAtlasTextures")]
    #[must_use]
    pub fn allow_multiple_atlas_textures(&self) -> bool {
        self.allow_multiple_atlas_textures
    }

    /// `supportBilerpFromGlyphAtlas()`.
    #[doc(alias = "supportBilerpFromGlyphAtlas")]
    #[must_use]
    pub fn support_bilerp_from_glyph_atlas(&self) -> bool {
        self.support_bilerp_from_glyph_atlas
    }

    /// `setBackendLabels()`.
    #[doc(alias = "setBackendLabels")]
    #[must_use]
    pub fn set_backend_labels(&self) -> bool {
        self.set_backend_labels
    }

    /// `getDstReadStrategy()`: what method of dst read a draw should use for obtaining the dst
    /// color.
    // Port of: src/gpu/graphite/Caps.cpp#L400-L408 (chrome/m156)
    #[doc(alias = "getDstReadStrategy")]
    #[must_use]
    pub fn get_dst_read_strategy(&self) -> DstReadStrategy {
        // TODO(b/238757201; b/383769988): Dst reads are currently only supported by FB fetch and
        // texture copy.
        if self.shader_caps.fb_fetch_support {
            DstReadStrategy::FramebufferFetch
        } else {
            DstReadStrategy::TextureCopy
        }
    }

    // ---- Texture support queries (Caps.cpp) --------------------------------------------------

    fn get_texture_support(&self, format: TextureFormat, tiling: Tiling) -> FormatSupport {
        self.format_support[tiling as usize][format as usize]
    }

    /// The usages and sample counts `format` supports with optimal tiling.
    #[must_use]
    pub fn format_support(&self, format: TextureFormat) -> FormatSupport {
        self.get_texture_support(format, Tiling::Optimal)
    }

    /// `getTextureUsage()`: the usages a texture with `info` can be used for.
    // Port of: src/gpu/graphite/dawn/DawnCaps.cpp#L170-L200 (chrome/m156)
    fn get_texture_usage(&self, info: &TextureInfo) -> (TextureUsage, Tiling) {
        let Some(wgpu_info) = info.get::<WgpuTextureInfoData>() else {
            return (TextureUsage::empty(), Tiling::Optimal);
        };

        let mut usage = TextureUsage::empty();
        if is_valid_view(wgpu_info) {
            if wgpu_info
                .usage
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
            {
                usage |= TextureUsage::RENDER;
                // All color renderable formats can be used with MSRTSS when the extension is
                // available
                if self.msaa_render_to_single_sampled_support()
                    && !texture_format_is_depth_or_stencil(texture_info_priv::view_format(info))
                {
                    usage |= TextureUsage::MSRTSS;
                }
            }
            if wgpu_info
                .usage
                .contains(wgpu::TextureUsages::TEXTURE_BINDING)
            {
                usage |= TextureUsage::READ | TextureUsage::SAMPLE;
            }
            if wgpu_info.usage.contains(wgpu::TextureUsages::COPY_SRC) {
                usage |= TextureUsage::COPY_SRC;
            }
            if wgpu_info.usage.contains(wgpu::TextureUsages::COPY_DST) {
                usage |= TextureUsage::COPY_DST;
            }
            if wgpu_info
                .usage
                .contains(wgpu::TextureUsages::STORAGE_BINDING)
            {
                usage |= TextureUsage::STORAGE;
            }
            // NOTE: No support for TextureUsage::kHostCopy yet
        }

        (usage, Tiling::Optimal)
    }

    // Port of: src/gpu/graphite/Caps.cpp#L141-L173 (chrome/m156)
    #[allow(clippy::fn_params_excessive_bools)] // mirrors the C++ signature
    fn is_supported(
        &self,
        info: &TextureInfo,
        test: TextureUsage,
        allow_msaa: bool,
        allow_external: bool,
        allow_compressed: bool,
        allow_protected: bool,
    ) -> bool {
        let format = texture_info_priv::view_format(info);
        if format == TextureFormat::Unsupported {
            return false;
        }
        debug_assert!(info.is_valid());

        let (texture_usage, tiling) = self.get_texture_usage(info);
        let FormatSupport {
            usage: format_support,
            sample_counts: mut supported_sample_counts,
        } = self.get_texture_support(format, tiling);

        if !allow_msaa {
            // Remove everything but 1x if the operation requires non-MSAA
            supported_sample_counts &= SampleCounts::ONE;
        }

        // Intersect what the format and the texture can do to see if `test` is available, and
        // make sure that the texture's sample count is supported.
        if (format_support & texture_usage & test) == test
            && supported_sample_counts.intersects(info.sample_count().into())
        {
            // Basic rules that should be reflected in the supported operations bit masks
            debug_assert!(
                (allow_protected || info.is_protected() == Protected::No)
                    && (allow_msaa || info.sample_count() == SampleCount::One)
                    && (allow_compressed
                        || texture_format_compression_type(format) == TextureCompressionType::None)
                    && (allow_external || format != TextureFormat::External)
            );
            true
        } else {
            false
        }
    }

    /// `isTexturable()`.
    // Port of: src/gpu/graphite/Caps.cpp#L175-L182 (chrome/m156)
    #[doc(alias = "isTexturable")]
    #[must_use]
    pub fn is_texturable(&self, info: &TextureInfo, allow_msaa: bool) -> bool {
        self.is_supported(info, TextureUsage::SAMPLE, allow_msaa, true, true, true)
    }

    /// `isReadable()`.
    // Port of: src/gpu/graphite/Caps.cpp#L184-L191 (chrome/m156)
    #[doc(alias = "isReadable")]
    #[must_use]
    pub fn is_readable(&self, info: &TextureInfo, allow_msaa: bool) -> bool {
        self.is_supported(info, TextureUsage::READ, allow_msaa, true, true, true)
    }

    /// `isRenderable()`.
    // Port of: src/gpu/graphite/Caps.cpp#L193-L200 (chrome/m156)
    #[doc(alias = "isRenderable")]
    #[must_use]
    pub fn is_renderable(&self, info: &TextureInfo) -> bool {
        self.is_supported(info, TextureUsage::RENDER, true, true, false, true)
    }

    /// `isCopyableSrc()`.
    // Port of: src/gpu/graphite/Caps.cpp#L202-L209 (chrome/m156)
    #[doc(alias = "isCopyableSrc")]
    #[must_use]
    pub fn is_copyable_src(&self, info: &TextureInfo) -> bool {
        self.is_supported(info, TextureUsage::COPY_SRC, false, false, false, false)
    }

    /// `isCopyableDst()`.
    // Port of: src/gpu/graphite/Caps.cpp#L211-L218 (chrome/m156)
    #[doc(alias = "isCopyableDst")]
    #[must_use]
    pub fn is_copyable_dst(&self, info: &TextureInfo) -> bool {
        self.is_supported(info, TextureUsage::COPY_DST, false, false, true, true)
    }

    /// `isStorage()`.
    // Port of: src/gpu/graphite/Caps.cpp#L220-L227 (chrome/m156)
    #[doc(alias = "isStorage")]
    #[must_use]
    pub fn is_storage(&self, info: &TextureInfo) -> bool {
        self.is_supported(info, TextureUsage::STORAGE, false, false, false, false)
    }

    /// `isRenderableWithMSRTSS()`.
    // Port of: src/gpu/graphite/Caps.cpp#L229-L236 (chrome/m156)
    #[doc(alias = "isRenderableWithMSRTSS")]
    #[must_use]
    pub fn is_renderable_with_msrtss(&self, info: &TextureInfo) -> bool {
        self.is_supported(
            info,
            TextureUsage::MSRTSS | TextureUsage::RENDER,
            true,
            true,
            false,
            true,
        )
    }

    /// `isSampleCountSupported()`.
    // Port of: src/gpu/graphite/Caps.cpp#L90-L95 (chrome/m156)
    #[doc(alias = "isSampleCountSupported")]
    #[must_use]
    pub fn is_sample_count_supported(
        &self,
        format: TextureFormat,
        sample_count: SampleCount,
    ) -> bool {
        // Assume optimal tiling
        let support = self.get_texture_support(format, Tiling::Optimal);
        support.usage.contains(TextureUsage::RENDER)
            && support.sample_counts.intersects(sample_count.into())
    }

    /// `getDepthStencilFormat()`: the `TextureFormat` that satisfies `dss_flags`.
    // Port of: src/gpu/graphite/Caps.cpp#L97-L139 (chrome/m156)
    #[doc(alias = "getDepthStencilFormat")]
    #[must_use]
    pub fn get_depth_stencil_format(&self, dss_flags: DepthStencilFlags) -> TextureFormat {
        let can_use = |format: TextureFormat| {
            let support = self.get_texture_support(format, Tiling::Optimal);
            // Check that the format can be rendered into and that it supports single-sampled
            // rendering, and if we aren't avoiding MSAA, that it also has some additional sample
            // count.
            support.usage.contains(TextureUsage::RENDER)
                && support.sample_counts.contains(SampleCounts::ONE)
                && (self.avoid_msaa() || support.sample_counts != SampleCounts::ONE)
        };

        match dss_flags {
            DepthStencilFlags::Depth => {
                // Prefer D16, but fallback to D32F or lastly a combined DS format if needed
                if can_use(TextureFormat::D16) {
                    TextureFormat::D16
                } else if can_use(TextureFormat::D32F) {
                    TextureFormat::D32F
                } else {
                    self.get_depth_stencil_format(DepthStencilFlags::DepthStencil)
                }
            }
            DepthStencilFlags::Stencil => {
                // Prefer S8, but fallback to a combined DS format if needed
                if can_use(TextureFormat::S8) {
                    TextureFormat::S8
                } else {
                    self.get_depth_stencil_format(DepthStencilFlags::DepthStencil)
                }
            }
            DepthStencilFlags::DepthStencil => {
                // Prefer D24_S8 over D32F_S8 for memory savings if it is available
                if can_use(TextureFormat::D24_S8) {
                    TextureFormat::D24_S8
                } else {
                    TextureFormat::D32F_S8
                }
            }
            DepthStencilFlags::None => TextureFormat::Unsupported, // i.e. no attachment needed
        }
    }

    /// `getDepthAttachmentDimensions()`: wgpu has no multiplanar textures, so the color
    /// attachment's dimensions are the answer for every texture.
    // Port of: src/gpu/graphite/dawn/DawnCaps.cpp#L284-L328 (chrome/m156)
    #[doc(alias = "getDepthAttachmentDimensions")]
    #[must_use]
    pub fn get_depth_attachment_dimensions(
        &self,
        _info: &TextureInfo,
        color_attachment_dimensions: ISize,
    ) -> ISize {
        color_attachment_dimensions
    }

    /// `onGetDefaultTextureInfo()`.
    // Port of: src/gpu/graphite/dawn/DawnCaps.cpp#L223-L282 (chrome/m156)
    fn on_get_default_texture_info(
        &self,
        usage: TextureUsage,
        format: TextureFormat,
        sample_count: SampleCount,
        mipmapped: Mipmapped,
        discardable: Discardable,
    ) -> TextureInfo {
        let wgpu_format = texture_format_to_wgpu_format(format);
        debug_assert!(wgpu_format.is_some()); // should have been caught by Caps first

        let mut wgpu_usage = wgpu::TextureUsages::empty();

        if usage.intersects(TextureUsage::SAMPLE | TextureUsage::READ) {
            wgpu_usage |= wgpu::TextureUsages::TEXTURE_BINDING;
        }
        if usage.contains(TextureUsage::STORAGE) {
            wgpu_usage |= wgpu::TextureUsages::STORAGE_BINDING;
        }
        if usage.contains(TextureUsage::COPY_SRC) {
            wgpu_usage |= wgpu::TextureUsages::COPY_SRC;
        }
        if usage.contains(TextureUsage::COPY_DST) {
            wgpu_usage |= wgpu::TextureUsages::COPY_DST;
        }
        if usage.contains(TextureUsage::RENDER) {
            wgpu_usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
            // Use transient attachments if possible for discardable textures
            if !self.supported_transient_attachment_usage.is_empty()
                && discardable == Discardable::Yes
            {
                wgpu_usage |= self.supported_transient_attachment_usage;
            }
            if self.emulate_load_store_resolve && !texture_format_is_depth_or_stencil(format) {
                // When emulating the store, the color attachment is sampled into the resolve so
                // add this usage even if higher-up Graphite logic wasn't expecting to sample it.
                wgpu_usage |= wgpu::TextureUsages::TEXTURE_BINDING;
            }
        }
        // NOTE: kMSRTSS is ignored since it's implicitly available on any wgpu::Texture if the
        // extension is available. kHostCopy should not be requested from Caps since it's
        // unsupported.
        debug_assert!(!usage.contains(TextureUsage::HOST_COPY));

        texture_infos::make_wgpu(&WgpuTextureInfo {
            sample_count,
            mipmapped,
            format: wgpu_format,
            view_format: None,
            usage: wgpu_usage,
            aspect: wgpu::TextureAspect::All,
            slice: 0,
        })
    }

    // Port of: src/gpu/graphite/Caps.cpp#L238-L293 (chrome/m156)
    fn get_default_texture_info(
        &self,
        mut usage: TextureUsage,
        formats: &[TextureFormat],
        sample_count: SampleCount,
        mipmapped: Mipmapped,
        is_protected: Protected,
        discardable: Discardable,
    ) -> TextureInfo {
        // Assert we're only requesting Discardable::kYes when the requested usages make sense for
        // it.
        const DISCARDABLE_ALLOWED: TextureUsage = TextureUsage::RENDER.union(TextureUsage::MSRTSS);
        debug_assert!(discardable == Discardable::No || (usage & DISCARDABLE_ALLOWED) == usage);

        if is_protected == Protected::Yes && !self.protected_support() {
            return TextureInfo::new(); // Cannot handle protected content on this Context
        }

        for &format in formats {
            let support = self.get_texture_support(format, Tiling::Optimal);
            if (support.usage & usage) != usage
                || !support.sample_counts.intersects(sample_count.into())
            {
                continue; // unsupported, move on to the next possible format
            }

            if usage.contains(TextureUsage::RENDER)
                && support.usage.contains(TextureUsage::MSRTSS)
                && sample_count == SampleCount::One
            {
                // Proactivately prepare a single-sampled image for use with MSRTSS if it's
                // supported by the format and kRender is requested. This flag is expected to be
                // harmless (if not, it's a driver bug).
                usage |= TextureUsage::MSRTSS;
            }

            if usage.contains(TextureUsage::COPY_DST)
                && support.usage.contains(TextureUsage::HOST_COPY)
                && !usage.contains(TextureUsage::RENDER)
                && is_protected == Protected::No
            {
                // Proactively enable kHostCopy when supported by the format and kCopyDst is
                // requested, so long as it's not going to be protected or rendered into.
                usage |= TextureUsage::HOST_COPY;
            }
            return self.on_get_default_texture_info(
                usage,
                format,
                sample_count,
                mipmapped,
                discardable,
            );
        }

        // None of the possible formats were supported
        TextureInfo::new()
    }

    /// `getDefaultSampledTextureInfo()`.
    // Port of: src/gpu/graphite/Caps.cpp#L310-L338 (chrome/m156)
    #[doc(alias = "getDefaultSampledTextureInfo")]
    #[must_use]
    pub fn get_default_sampled_texture_info(
        &self,
        color_type: ColorType,
        mipmapped: Mipmapped,
        is_protected: Protected,
        renderable: Renderable,
    ) -> TextureInfo {
        let mut usage = DEFAULT_SAMPLED_USAGE;
        let mut formats: Vec<TextureFormat> = preferred_texture_formats(color_type).to_vec();
        if renderable == Renderable::Yes {
            usage |= TextureUsage::RENDER;
            // Any possible preferred format must also have a valid write swizzle for the
            // requested color type.
            formats.retain(|&f| write_swizzle_for_color_type(color_type, f).is_some());
        }

        self.get_default_texture_info(
            usage,
            &formats,
            SampleCount::One,
            mipmapped,
            is_protected,
            Discardable::No,
        )
    }

    /// `getDefaultReadableTextureInfo()`.
    // Port of: src/gpu/graphite/Caps.cpp#L340-L351 (chrome/m156)
    #[doc(alias = "getDefaultReadableTextureInfo")]
    #[must_use]
    pub fn get_default_readable_texture_info(
        &self,
        format: TextureFormat,
        is_protected: Protected,
    ) -> TextureInfo {
        self.get_default_texture_info(
            TextureUsage::READ | TextureUsage::COPY_SRC | TextureUsage::COPY_DST,
            &[format],
            SampleCount::One,
            Mipmapped::No,
            is_protected,
            Discardable::No,
        )
    }

    /// `getTextureInfoForSampledCopy()`.
    // Port of: src/gpu/graphite/Caps.cpp#L353-L363 (chrome/m156)
    #[doc(alias = "getTextureInfoForSampledCopy")]
    #[must_use]
    pub fn get_texture_info_for_sampled_copy(
        &self,
        info: &TextureInfo,
        mipmapped: Mipmapped,
    ) -> TextureInfo {
        let format = texture_info_priv::view_format(info);
        self.get_default_texture_info(
            DEFAULT_SAMPLED_USAGE,
            &[format],
            SampleCount::One,
            mipmapped,
            info.is_protected(),
            Discardable::No,
        )
    }

    /// `getTextureInfoForReadableCopy()`.
    // Port of: src/gpu/graphite/Caps.cpp#L365-L368 (chrome/m156)
    #[doc(alias = "getTextureInfoForReadableCopy")]
    #[must_use]
    pub fn get_texture_info_for_readable_copy(&self, info: &TextureInfo) -> TextureInfo {
        self.get_default_readable_texture_info(
            texture_info_priv::view_format(info),
            info.is_protected(),
        )
    }

    /// `getDefaultCompressedTextureInfo()`.
    // Port of: src/gpu/graphite/Caps.cpp#L370-L384 (chrome/m156)
    #[doc(alias = "getDefaultCompressedTextureInfo")]
    #[must_use]
    pub fn get_default_compressed_texture_info(
        &self,
        compression_type: TextureCompressionType,
        mipmapped: Mipmapped,
        is_protected: Protected,
    ) -> TextureInfo {
        // Remove CopySrc for compressed textures
        let format = compression_type_to_texture_format(compression_type);
        self.get_default_texture_info(
            DEFAULT_SAMPLED_USAGE - TextureUsage::COPY_SRC,
            &[format],
            SampleCount::One,
            mipmapped,
            is_protected,
            Discardable::No,
        )
    }

    /// `getDefaultStorageTextureInfo()`.
    // Port of: src/gpu/graphite/Caps.cpp#L386-L398 (chrome/m156)
    #[doc(alias = "getDefaultStorageTextureInfo")]
    #[must_use]
    pub fn get_default_storage_texture_info(&self, color_type: ColorType) -> TextureInfo {
        // Storage textures are currently always assumed to be sampleable from a shader and can be
        // copied out of (for unit tests).
        self.get_default_texture_info(
            TextureUsage::STORAGE | TextureUsage::SAMPLE | TextureUsage::COPY_SRC,
            preferred_texture_formats(color_type),
            SampleCount::One,
            Mipmapped::No,
            Protected::No,
            Discardable::No,
        )
    }

    /// `getDefaultReadableStorageTextureInfo()`.
    // Port of: src/gpu/graphite/Caps.cpp#L400-L412 (chrome/m156)
    #[doc(alias = "getDefaultReadableStorageTextureInfo")]
    #[must_use]
    pub fn get_default_readable_storage_texture_info(
        &self,
        format: TextureFormat,
        is_protected: Protected,
    ) -> TextureInfo {
        self.get_default_texture_info(
            TextureUsage::STORAGE
                | TextureUsage::READ
                | TextureUsage::COPY_SRC
                | TextureUsage::COPY_DST,
            &[format],
            SampleCount::One,
            Mipmapped::No,
            is_protected,
            Discardable::No,
        )
    }

    /// `getCompatibleMSAASampleCount()`.
    // Port of: src/gpu/graphite/Caps.cpp#L414-L436 (chrome/m156)
    #[doc(alias = "getCompatibleMSAASampleCount")]
    #[must_use]
    pub fn get_compatible_msaa_sample_count(&self, info: &TextureInfo) -> SampleCount {
        if info.sample_count() > SampleCount::One {
            // Use the inherent sample count since it's already MSAA
            return info.sample_count();
        } else if !self.avoid_msaa() {
            // The max internal sample count may be higher than what is universally supported for
            // every renderable TextureFormat, but unless avoidMSAA() was true, this should bottom
            // out at SampleCount::k4.
            let format = texture_info_priv::view_format(info);
            let mut s = self.max_internal_sample_count as u8;
            while s > 1 {
                let count = crate::graphite::graphite_types::to_sample_count(u32::from(s));
                if self.is_sample_count_supported(format, count) {
                    return count;
                }
                s >>= 1;
            }
        }

        // If we got here, MSAA has been disabled somehow (by ContextOption, driver workaround, or
        // no support for a particular TextureFormat).
        SampleCount::One
    }

    // ---- Sizes and alignments ----------------------------------------------------------------

    /// `getAlignedTextureDataRowBytes()`: the aligned rowBytes when transferring to or from a
    /// texture, or 0 on overflow.
    // Port of: src/gpu/graphite/Caps.h#L297-L306 (chrome/m156)
    #[doc(alias = "getAlignedTextureDataRowBytes")]
    #[must_use]
    pub fn get_aligned_texture_data_row_bytes(
        &self,
        row_bytes: usize,
        bytes_per_block: usize,
    ) -> usize {
        debug_assert!(bytes_per_block > 0);
        debug_assert!(self.texture_data_row_bytes_alignment > 0);
        let Some(alignment) = lcm(bytes_per_block, self.texture_data_row_bytes_alignment) else {
            return 0;
        };
        row_bytes.checked_next_multiple_of(alignment).unwrap_or(0)
    }

    /// `buildKeyForTexture()`.
    ///
    /// # Panics
    /// If `info` is not a wgpu texture info.
    // Port of: src/gpu/graphite/dawn/DawnCaps.cpp#L660-L714 (chrome/m156)
    #[doc(alias = "buildKeyForTexture")]
    pub fn build_key_for_texture(
        &self,
        dimensions: ISize,
        info: &TextureInfo,
        ty: ResourceType,
        key: &mut GraphiteResourceKey,
    ) {
        let wgpu_info = info
            .get::<WgpuTextureInfoData>()
            .expect("a wgpu texture info");

        debug_assert!(dimensions.width > 0 && dimensions.height > 0);

        debug_assert!(wgpu_info.get_view_format().is_some());
        // FIXME we can reduce this by packing format into samplesKey and then we're back down to
        // 5 ints we could go further if we said textures were likely to be under 65kx65kf...
        // The key identifies the view format; Graphite's format enum has a value for every format
        // a texture the caps allow can have.
        let format_key = texture_info_priv::view_format(info) as u32;

        let samples_key = samples_to_key(info.sample_count());
        // We don't have to key the number of mip levels because it is inherit in the combination
        // of isMipped and dimensions.
        let is_mipped = info.mipmapped() == Mipmapped::Yes;

        // Confirm all the below parts of the key can fit in a single uint32_t. The sum of the
        // shift amounts in the asserts must be less than or equal to 32.
        debug_assert!(samples_key < (1u32 << 3)); // sample key is first 3 bits
        debug_assert!(u32::from(is_mipped) < (1u32 << 1)); // isMapped is 4th bit
        debug_assert!(wgpu_info.usage.bits() < (1u32 << 28)); // usage is remaining 28 bits

        // We need two uint32_ts for dimensions, 1 for format, and 1 for the rest of the key;
        let num32_data_cnt: u16 = 2 + 1 + 1;
        let mut builder = GraphiteResourceKeyBuilder::new(key, ty, num32_data_cnt);

        #[allow(clippy::cast_sign_loss)] // dimensions are positive (asserted above)
        {
            builder[0] = dimensions.width as u32;
            builder[1] = dimensions.height as u32;
        }
        builder[2] = format_key;
        builder[3] = samples_key | (u32::from(is_mipped) << 3) | (wgpu_info.usage.bits() << 4);
    }

    /// `getRenderPassDescKeyForPipeline()`: computes the render pass desc's key as 32 bits. The
    /// key has room for additional flag which can optionally be provided.
    // Port of: src/gpu/graphite/dawn/DawnCaps.cpp#L477-L513 (chrome/m156)
    #[doc(alias = "getRenderPassDescKeyForPipeline")]
    #[must_use]
    pub fn get_render_pass_desc_key_for_pipeline(
        &self,
        render_pass_desc: &RenderPassDesc,
        additional_flag: bool,
    ) -> u32 {
        // The color attachment should be valid; the depth-stencil attachment may not be if it's
        // not being used. The full resolve attachment (if present) does not need to be included.
        let color = &render_pass_desc.color_attachment;
        let depth_stencil = &render_pass_desc.depth_stencil_attachment;
        debug_assert_ne!(color.format, TextureFormat::Unsupported);

        // Note: if Dawn supports ExpandResolveTexture load op and the render pass uses it to load
        // the resolve texture, a render pipeline will need to be created with
        // wgpu::ColorTargetStateExpandResolveTextureDawn chained struct in order to be
        // compatible. Hence a render pipeline created for a render pass using
        // ExpandResolveTexture load op will be different from the one created for a render pass
        // not using that load op. So we need to include a bit flag to differentiate the two kinds
        // of pipelines.
        let should_include_load_resolve_attachment_bit = self.load_op_affects_msaa_pipelines();
        let mut load_resolve_attachment_key = 0;
        if should_include_load_resolve_attachment_bit
            && render_pass_desc.color_resolve_attachment.format != TextureFormat::Unsupported
            && render_pass_desc.color_resolve_attachment.load_op == LoadOp::Load
        {
            load_resolve_attachment_key = 1;
        }

        debug_assert!(samples_to_key(color.sample_count) < (1 << NUM_SAMPLE_KEY_BITS));
        debug_assert!(samples_to_key(depth_stencil.sample_count) < (1 << NUM_SAMPLE_KEY_BITS));
        debug_assert!(load_resolve_attachment_key < (1 << RESOLVE_BITS));
        let additional_flag_key = u32::from(additional_flag);

        (additional_flag_key << ADDITIONAL_FLAG_OFFSET)
            | ((color.format as u32) << COLOR_FORMAT_OFFSET)
            | (samples_to_key(color.sample_count) << COLOR_NUM_SAMPLES_OFFSET)
            | ((depth_stencil.format as u32) << DEPTH_STENCIL_FORMAT_OFFSET)
            | (samples_to_key(depth_stencil.sample_count) << DEPTH_STENCIL_NUM_SAMPLES_OFFSET)
            | load_resolve_attachment_key
    }
}

/// `kDefaultSampledUsage`: Graphite by default requires copy-src and copy-dst for sampled
/// textures.
// Port of: src/gpu/graphite/Caps.cpp#L295-L297 (chrome/m156)
const DEFAULT_SAMPLED_USAGE: TextureUsage = TextureUsage::SAMPLE
    .union(TextureUsage::COPY_SRC)
    .union(TextureUsage::COPY_DST);

/// `is_valid_view()`: no multiplanar formats in pure WebGPU, so require that aspect == All and
/// view format and base format are the same. We allow the format to be Undefined if the view
/// format is set, which can arise with promise images.
// Port of: src/gpu/graphite/dawn/DawnCaps.cpp#L30-L82 (chrome/m156)
fn is_valid_view(info: &WgpuTextureInfoData) -> bool {
    info.aspect == wgpu::TextureAspect::All
        && info.get_view_format().is_some()
        && (info.format == info.get_view_format() || info.format.is_none())
}

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// `SkSafeMath::lcm`: `None` on overflow.
fn lcm(a: usize, b: usize) -> Option<usize> {
    (a / gcd(a, b)).checked_mul(b)
}

impl Caps for WgpuCaps {
    fn get_dst_read_strategy(&self) -> DstReadStrategy {
        WgpuCaps::get_dst_read_strategy(self)
    }

    fn supports_hardware_advanced_blending(&self) -> bool {
        // `fBlendEqSupport` keeps its `kBasic` default (`Caps.h`); the WebGPU backend never raises
        // it, so advanced blend modes always use shader blending.
        false
    }

    fn dual_source_blending_support(&self) -> bool {
        self.shader_caps.dual_source_blending_support
    }

    fn storage_buffer_support(&self) -> bool {
        WgpuCaps::storage_buffer_support(self)
    }

    fn clamp_to_border_support(&self) -> bool {
        WgpuCaps::clamp_to_border_support(self)
    }

    fn max_texture_size(&self) -> i32 {
        WgpuCaps::max_texture_size(self)
    }

    fn require_ordered_recordings(&self) -> bool {
        WgpuCaps::require_ordered_recordings(self)
    }

    fn draw_buffer_can_be_mapped(&self) -> bool {
        WgpuCaps::draw_buffer_can_be_mapped(self)
    }

    fn buffer_maps_are_async(&self) -> bool {
        WgpuCaps::buffer_maps_are_async(self)
    }

    fn required_uniform_buffer_alignment(&self) -> usize {
        WgpuCaps::required_uniform_buffer_alignment(self)
    }

    fn required_storage_buffer_alignment(&self) -> usize {
        WgpuCaps::required_storage_buffer_alignment(self)
    }

    fn required_transfer_buffer_alignment(&self) -> usize {
        WgpuCaps::required_transfer_buffer_alignment(self)
    }

    fn get_aligned_texture_data_row_bytes(
        &self,
        row_bytes: usize,
        bytes_per_block: usize,
    ) -> usize {
        WgpuCaps::get_aligned_texture_data_row_bytes(self, row_bytes, bytes_per_block)
    }

    fn full_compressed_upload_size_must_align_to_block_dims(&self) -> bool {
        WgpuCaps::full_compressed_upload_size_must_align_to_block_dims(self)
    }

    fn avoid_depth_mode(&self) -> bool {
        WgpuCaps::avoid_depth_mode(self)
    }

    fn attachment_size_policy(&self) -> AttachmentSizePolicy {
        WgpuCaps::attachment_size_policy(self)
    }

    fn get_depth_attachment_dimensions(
        &self,
        info: &TextureInfo,
        color_attachment_dimensions: ISize,
    ) -> ISize {
        WgpuCaps::get_depth_attachment_dimensions(self, info, color_attachment_dimensions)
    }

    fn get_depth_stencil_format(&self, flags: DepthStencilFlags) -> TextureFormat {
        WgpuCaps::get_depth_stencil_format(self, flags)
    }

    // Port of: src/gpu/graphite/Caps.cpp#L310-L338 (chrome/m156)
    fn get_default_sampled_texture_info(
        &self,
        color_type: ColorType,
        mipmapped: Mipmapped,
        is_protected: Protected,
        renderable: Renderable,
    ) -> TextureInfo {
        self.get_default_sampled_texture_info(color_type, mipmapped, is_protected, renderable)
    }

    // Port of: src/gpu/graphite/Caps.cpp#L295-L308 (chrome/m156)
    fn get_default_attachment_texture_info(
        &self,
        desc: &AttachmentDesc,
        is_protected: Protected,
        discardable: Discardable,
    ) -> TextureInfo {
        self.get_default_texture_info(
            TextureUsage::RENDER,
            &[desc.format],
            desc.sample_count,
            Mipmapped::No,
            is_protected,
            discardable,
        )
    }

    fn get_compatible_msaa_sample_count(&self, info: &TextureInfo) -> SampleCount {
        WgpuCaps::get_compatible_msaa_sample_count(self, info)
    }

    fn is_renderable_with_msrtss(&self, info: &TextureInfo) -> bool {
        WgpuCaps::is_renderable_with_msrtss(self, info)
    }

    fn shader_caps(&self) -> &ShaderCaps {
        WgpuCaps::shader_caps(self)
    }

    fn resource_binding_requirements(&self) -> &ResourceBindingRequirements {
        WgpuCaps::resource_binding_requirements(self)
    }

    fn max_varyings(&self) -> i32 {
        WgpuCaps::max_varyings(self)
    }

    fn ndc_y_axis_points_down(&self) -> bool {
        WgpuCaps::ndc_y_axis_points_down(self)
    }

    fn protected_support(&self) -> bool {
        WgpuCaps::protected_support(self)
    }

    fn semaphore_support(&self) -> bool {
        WgpuCaps::semaphore_support(self)
    }

    fn allow_cpu_sync(&self) -> bool {
        WgpuCaps::allow_cpu_sync(self)
    }

    fn storage_buffer_support_for_compute(&self) -> bool {
        WgpuCaps::storage_buffer_support_for_compute(self)
    }

    fn compute_support(&self) -> bool {
        WgpuCaps::compute_support(self)
    }

    fn avoid_msaa(&self) -> bool {
        WgpuCaps::avoid_msaa(self)
    }

    fn msaa_render_to_single_sampled_support(&self) -> bool {
        WgpuCaps::msaa_render_to_single_sampled_support(self)
    }

    fn use_draw_list_layer(&self) -> bool {
        WgpuCaps::use_draw_list_layer(self)
    }

    fn load_op_affects_msaa_pipelines(&self) -> bool {
        WgpuCaps::load_op_affects_msaa_pipelines(self)
    }

    fn max_path_atlas_texture_size(&self) -> i32 {
        WgpuCaps::max_path_atlas_texture_size(self)
    }

    fn allow_multiple_atlas_textures(&self) -> bool {
        WgpuCaps::allow_multiple_atlas_textures(self)
    }

    fn support_bilerp_from_glyph_atlas(&self) -> bool {
        WgpuCaps::support_bilerp_from_glyph_atlas(self)
    }

    fn set_backend_labels(&self) -> bool {
        WgpuCaps::set_backend_labels(self)
    }

    fn is_sample_count_supported(&self, format: TextureFormat, sample_count: SampleCount) -> bool {
        WgpuCaps::is_sample_count_supported(self, format, sample_count)
    }

    fn is_texturable(&self, info: &TextureInfo, allow_msaa: bool) -> bool {
        WgpuCaps::is_texturable(self, info, allow_msaa)
    }

    fn is_readable(&self, info: &TextureInfo, allow_msaa: bool) -> bool {
        WgpuCaps::is_readable(self, info, allow_msaa)
    }

    fn is_renderable(&self, info: &TextureInfo) -> bool {
        WgpuCaps::is_renderable(self, info)
    }

    fn is_copyable_src(&self, info: &TextureInfo) -> bool {
        WgpuCaps::is_copyable_src(self, info)
    }

    fn is_copyable_dst(&self, info: &TextureInfo) -> bool {
        WgpuCaps::is_copyable_dst(self, info)
    }

    fn is_storage(&self, info: &TextureInfo) -> bool {
        WgpuCaps::is_storage(self, info)
    }
}

/// A shareable handle to caps, as the shared context hands them out.
pub type SharedWgpuCaps = Arc<WgpuCaps>;

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(profile: &CapsProfile) -> WgpuCaps {
        WgpuCaps::new(profile, &ContextOptions::default())
    }

    fn rgba8_info(usage: wgpu::TextureUsages, samples: SampleCount) -> TextureInfo {
        texture_infos::make_wgpu(&WgpuTextureInfo::new(
            samples,
            Mipmapped::No,
            wgpu::TextureFormat::Rgba8Unorm,
            usage,
            wgpu::TextureAspect::All,
        ))
    }

    #[test]
    fn d3d12_profile_matches_the_note() {
        // docs/design/gpu.md §1.3: D3D12 has no ShaderF16, storage buffers, 64 bytes of
        // immediates, dual-source blending and the Dawn load-resolve extensions.
        let c = caps(&CapsProfile::dawn_d3d12());
        assert!(!c.supports_half_precision());
        assert_eq!(
            c.resource_binding_requirements().uniform_buffer_layout,
            Layout::Std140
        );
        assert_eq!(
            c.resource_binding_requirements().storage_buffer_layout,
            Layout::Std430
        );
        assert!(c.storage_buffer_support());
        assert!(
            c.resource_binding_requirements()
                .use_push_constants_for_intrinsic_constants
        );
        assert!(c.shader_caps().dual_source_blending_support);
        assert!(!c.shader_caps().fb_fetch_support);
        assert!(!c.shader_caps().infinity_support);
        assert_eq!(
            c.resolve_texture_load_op(),
            Some(ResolveTextureLoadOp::ExpandResolveTexture)
        );
        assert!(c.load_op_affects_msaa_pipelines());
        assert!(c.supports_partial_load_resolve());
        assert!(c.supports_render_pass_render_area());
        assert!(!c.emulate_load_store_resolve());
        assert_eq!(
            Caps::attachment_size_policy(&c),
            AttachmentSizePolicy::MsaaRenderArea
        );
        assert_eq!(c.discard_load_op(), DiscardLoadOp::Undefined);
        assert_eq!(c.discard_store_op(), DiscardStoreOp::Undefined);
        assert!(Caps::buffer_maps_are_async(&c));
        assert!(!Caps::draw_buffer_can_be_mapped(&c));
        assert!(c.use_async_pipeline_creation());
        assert!(c.allow_scoped_error_checks());
    }

    #[test]
    fn vulkan_profile_matches_the_note() {
        // Vulkan: ShaderF16 switches the buffer layouts, storage buffers are off.
        let c = caps(&CapsProfile::dawn_vulkan());
        assert!(c.supports_half_precision());
        assert_eq!(
            c.resource_binding_requirements().uniform_buffer_layout,
            Layout::Std140F16
        );
        assert_eq!(
            c.resource_binding_requirements().storage_buffer_layout,
            Layout::Std430F16
        );
        assert!(!c.storage_buffer_support());
        // No partial load resolve and no transient attachments: Graphite emulates load/store
        // resolve, and the ExpandResolveTexture load op is force-disabled.
        assert!(c.emulate_load_store_resolve());
        assert_eq!(c.resolve_texture_load_op(), None);
        assert!(!c.load_op_affects_msaa_pipelines());
        assert_eq!(
            Caps::attachment_size_policy(&c),
            AttachmentSizePolicy::MsaaRenderArea
        );
    }

    #[test]
    fn wgpu_restricted_removes_dawn_only_features() {
        let profile = CapsProfile::dawn_d3d12().wgpu_restricted();
        assert_eq!(profile.name, "dawn-d3d12-wgpucaps");
        assert!(!profile.features.intersects(DeviceFeatures::DAWN_ONLY));
        assert!(
            profile
                .features
                .contains(DeviceFeatures::DUAL_SOURCE_BLENDING)
        );
        let c = caps(&profile);
        assert!(c.emulate_load_store_resolve());
        assert_eq!(c.discard_load_op(), DiscardLoadOp::Clear);
        assert_eq!(c.discard_store_op(), DiscardStoreOp::Discard);
        // Without Dawn's tier-1 formats, 16-bit formats still come from Unorm16TextureFormats.
        assert!(
            c.format_support(TextureFormat::R16)
                .usage
                .contains(TextureUsage::RENDER)
        );
    }

    #[test]
    fn no_tick_disables_sync_features() {
        let mut profile = CapsProfile::dawn_d3d12();
        profile.has_tick = false;
        let c = caps(&profile);
        assert!(!c.allow_cpu_sync());
        assert!(!c.use_async_pipeline_creation());
        assert!(!c.allow_scoped_error_checks());
    }

    #[test]
    fn format_table_follows_the_flags() {
        let c = caps(&CapsProfile::dawn_d3d12());
        let rgba8 = c.format_support(TextureFormat::RGBA8);
        assert_eq!(
            rgba8.usage,
            TextureUsage::READ
                | TextureUsage::SAMPLE
                | TextureUsage::RENDER
                | TextureUsage::STORAGE
                | TextureUsage::COPY_SRC
                | TextureUsage::COPY_DST
        );
        assert_eq!(rgba8.sample_counts, SampleCounts::ONE | SampleCounts::FOUR);
        // Depth/stencil formats do not resolve, so their MSAA support is the MSAA flag alone.
        let d24s8 = c.format_support(TextureFormat::D24_S8);
        assert_eq!(d24s8.sample_counts, SampleCounts::ONE | SampleCounts::FOUR);
        assert!(!d24s8.usage.contains(TextureUsage::SAMPLE));
        // Compressed textures can be copied into but not out of.
        let bc1 = c.format_support(TextureFormat::RGBA8_BC1);
        assert_eq!(bc1, FormatSupport::default());
        // Unsupported formats have no support at all.
        assert_eq!(
            c.format_support(TextureFormat::A8),
            FormatSupport::default()
        );
    }

    #[test]
    fn texture_queries() {
        let c = caps(&CapsProfile::dawn_d3d12());
        let sampled = rgba8_info(
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            SampleCount::One,
        );
        assert!(c.is_texturable(&sampled, false));
        assert!(c.is_readable(&sampled, false));
        assert!(!c.is_renderable(&sampled));
        assert!(c.is_copyable_dst(&sampled));
        assert!(!c.is_copyable_src(&sampled));
        assert!(!c.is_storage(&sampled));

        let msaa = rgba8_info(wgpu::TextureUsages::RENDER_ATTACHMENT, SampleCount::Four);
        assert!(c.is_renderable(&msaa));
        assert!(!c.is_texturable(&msaa, false));
        let eight = rgba8_info(wgpu::TextureUsages::RENDER_ATTACHMENT, SampleCount::Eight);
        assert!(!c.is_renderable(&eight));
        assert!(!c.is_renderable_with_msrtss(&msaa));

        // A texture info of another backend, or an invalid one, supports nothing.
        assert!(!c.is_texturable(&TextureInfo::new(), true));
    }

    #[test]
    fn default_texture_infos() {
        let c = caps(&CapsProfile::dawn_d3d12());
        let sampled = c.get_default_sampled_texture_info(
            ColorType::RGBA8888,
            Mipmapped::Yes,
            Protected::No,
            Renderable::Yes,
        );
        let info = texture_infos::get_wgpu_texture_info(&sampled).unwrap();
        assert_eq!(info.format, Some(wgpu::TextureFormat::Rgba8Unorm));
        assert_eq!(info.mipmapped, Mipmapped::Yes);
        assert_eq!(
            info.usage,
            wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT
        );

        // Protected memory is unsupported.
        assert!(
            !c.get_default_sampled_texture_info(
                ColorType::RGBA8888,
                Mipmapped::No,
                Protected::Yes,
                Renderable::No
            )
            .is_valid()
        );

        // Compressed textures need the compression features.
        assert!(
            !c.get_default_compressed_texture_info(
                TextureCompressionType::ETC2_RGB8_UNORM,
                Mipmapped::No,
                Protected::No
            )
            .is_valid()
        );

        // Attachments.
        let attachment = AttachmentDesc {
            format: TextureFormat::RGBA8,
            sample_count: SampleCount::Four,
            ..AttachmentDesc::default()
        };
        let info = Caps::get_default_attachment_texture_info(
            &c,
            &attachment,
            Protected::No,
            Discardable::Yes,
        );
        assert!(c.is_renderable(&info));
        assert_eq!(info.sample_count(), SampleCount::Four);
    }

    #[test]
    fn transient_attachments_for_discardable_textures() {
        let mut profile = CapsProfile::dawn_vulkan();
        profile.features |= DeviceFeatures::TRANSIENT_ATTACHMENTS;
        let c = caps(&profile);
        let attachment = AttachmentDesc {
            format: TextureFormat::D24_S8,
            sample_count: SampleCount::Four,
            ..AttachmentDesc::default()
        };
        let info = Caps::get_default_attachment_texture_info(
            &c,
            &attachment,
            Protected::No,
            Discardable::Yes,
        );
        let wgpu_info = texture_infos::get_wgpu_texture_info(&info).unwrap();
        assert!(
            wgpu_info
                .usage
                .contains(wgpu::TextureUsages::TRANSIENT_ATTACHMENT)
        );
        // Transient attachments make load-store emulation unnecessary.
        assert!(!c.emulate_load_store_resolve());

        // Not discardable: no transient usage.
        let info = Caps::get_default_attachment_texture_info(
            &c,
            &attachment,
            Protected::No,
            Discardable::No,
        );
        assert!(
            !texture_infos::get_wgpu_texture_info(&info)
                .unwrap()
                .usage
                .contains(wgpu::TextureUsages::TRANSIENT_ATTACHMENT)
        );
    }

    #[test]
    fn emulated_load_store_resolve_makes_color_attachments_sampleable() {
        let c = caps(&CapsProfile::dawn_vulkan());
        assert!(c.emulate_load_store_resolve());
        let attachment = AttachmentDesc {
            format: TextureFormat::RGBA8,
            ..AttachmentDesc::default()
        };
        let info = Caps::get_default_attachment_texture_info(
            &c,
            &attachment,
            Protected::No,
            Discardable::No,
        );
        assert!(
            texture_infos::get_wgpu_texture_info(&info)
                .unwrap()
                .usage
                .contains(wgpu::TextureUsages::TEXTURE_BINDING)
        );
    }

    #[test]
    fn depth_stencil_formats() {
        let c = caps(&CapsProfile::dawn_d3d12());
        assert_eq!(
            Caps::get_depth_stencil_format(&c, DepthStencilFlags::Depth),
            TextureFormat::D16
        );
        assert_eq!(
            Caps::get_depth_stencil_format(&c, DepthStencilFlags::Stencil),
            TextureFormat::S8
        );
        assert_eq!(
            Caps::get_depth_stencil_format(&c, DepthStencilFlags::DepthStencil),
            TextureFormat::D24_S8
        );
        assert_eq!(
            Caps::get_depth_stencil_format(&c, DepthStencilFlags::None),
            TextureFormat::Unsupported
        );
    }

    #[test]
    fn msaa_sample_counts() {
        let c = caps(&CapsProfile::dawn_d3d12());
        let single = rgba8_info(wgpu::TextureUsages::RENDER_ATTACHMENT, SampleCount::One);
        assert_eq!(
            Caps::get_compatible_msaa_sample_count(&c, &single),
            SampleCount::Four
        );
        assert!(c.is_sample_count_supported(TextureFormat::RGBA8, SampleCount::Four));
        assert!(!c.is_sample_count_supported(TextureFormat::RGBA8, SampleCount::Eight));
        let msaa = rgba8_info(wgpu::TextureUsages::RENDER_ATTACHMENT, SampleCount::Four);
        assert_eq!(
            Caps::get_compatible_msaa_sample_count(&c, &msaa),
            SampleCount::Four
        );

        // Avoiding depth mode avoids MSAA.
        let options = ContextOptions {
            avoid_depth_mode: true,
            ..ContextOptions::default()
        };
        let c = WgpuCaps::new(&CapsProfile::dawn_d3d12(), &options);
        assert!(c.avoid_msaa());
        assert_eq!(
            Caps::get_compatible_msaa_sample_count(&c, &single),
            SampleCount::One
        );
    }

    #[test]
    fn alignments_and_limits() {
        let c = caps(&CapsProfile::dawn_d3d12());
        assert_eq!(Caps::max_texture_size(&c), 8192);
        assert_eq!(Caps::required_transfer_buffer_alignment(&c), 4);
        assert_eq!(Caps::required_uniform_buffer_alignment(&c), 256);
        assert_eq!(Caps::required_storage_buffer_alignment(&c), 256);
        // Buffer-to-texture copies need 256-byte rows (and a multiple of the block size).
        assert_eq!(c.get_aligned_texture_data_row_bytes(5, 4), 256);
        assert_eq!(c.get_aligned_texture_data_row_bytes(256, 4), 256);
        assert_eq!(c.get_aligned_texture_data_row_bytes(257, 4), 512);
        assert_eq!(c.get_aligned_texture_data_row_bytes(300, 12), 768);
        assert_eq!(c.get_aligned_texture_data_row_bytes(usize::MAX, 12), 0);
        assert!(Caps::full_compressed_upload_size_must_align_to_block_dims(
            &c
        ));
        let bindings = c.resource_binding_requirements();
        assert_eq!(bindings.backend_api, BackendApi::Dawn);
        assert_eq!(bindings.uniforms_set_idx, 0);
        assert_eq!(bindings.texture_sampler_set_idx, 1);
        assert_eq!(bindings.storage_buffer_binding, 2);
        assert_eq!(bindings.max_fallback_texture_size, 8192);
        assert_eq!(bindings.max_fallback_texture_bytes, 8192 * 8192 * 16);
        assert_eq!(c.get_dst_read_strategy(), DstReadStrategy::TextureCopy);
    }

    #[test]
    fn render_pass_desc_keys() {
        let c = caps(&CapsProfile::dawn_d3d12());
        let mut desc = RenderPassDesc {
            color_attachment: AttachmentDesc {
                format: TextureFormat::RGBA8,
                sample_count: SampleCount::Four,
                ..AttachmentDesc::default()
            },
            depth_stencil_attachment: AttachmentDesc {
                format: TextureFormat::D24_S8,
                sample_count: SampleCount::Four,
                ..AttachmentDesc::default()
            },
            ..RenderPassDesc::default()
        };
        let key = c.get_render_pass_desc_key_for_pipeline(&desc, false);
        assert_eq!(
            key,
            ((TextureFormat::RGBA8 as u32) << 15)
                | (2 << 12)
                | ((TextureFormat::D24_S8 as u32) << 4)
                | (2 << 1)
        );
        assert_eq!(
            c.get_render_pass_desc_key_for_pipeline(&desc, true),
            key | (1 << 23)
        );

        // Loading the resolve texture changes the key when the load op exists.
        desc.color_resolve_attachment = AttachmentDesc {
            format: TextureFormat::RGBA8,
            load_op: LoadOp::Load,
            ..AttachmentDesc::default()
        };
        assert_eq!(
            c.get_render_pass_desc_key_for_pipeline(&desc, false),
            key | 1
        );
        // …but not when the device cannot (Vulkan emulates it).
        let vulkan = caps(&CapsProfile::dawn_vulkan());
        assert_eq!(
            vulkan.get_render_pass_desc_key_for_pipeline(&desc, false),
            key
        );
    }

    #[test]
    fn texture_keys_distinguish_properties() {
        use crate::graphite::graphite_resource_key::GraphiteResourceKey;
        let c = caps(&CapsProfile::dawn_d3d12());
        let key_of = |dims: ISize, info: &TextureInfo| {
            let mut key = GraphiteResourceKey::new();
            c.build_key_for_texture(dims, info, 7, &mut key);
            key
        };
        let usage = wgpu::TextureUsages::TEXTURE_BINDING;
        let base = rgba8_info(usage, SampleCount::One);
        let size = ISize::new(16, 8);
        assert_eq!(key_of(size, &base), key_of(size, &base));
        assert_ne!(key_of(size, &base), key_of(ISize::new(8, 16), &base));
        assert_ne!(
            key_of(size, &base),
            key_of(
                size,
                &rgba8_info(usage | wgpu::TextureUsages::COPY_SRC, SampleCount::One)
            )
        );
        assert_ne!(
            key_of(size, &base),
            key_of(
                size,
                &rgba8_info(wgpu::TextureUsages::RENDER_ATTACHMENT, SampleCount::Four)
            )
        );
    }
}
