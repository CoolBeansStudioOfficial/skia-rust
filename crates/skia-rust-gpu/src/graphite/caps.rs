// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/Caps.h (only the queries G9a code makes)

//! The backend-neutral half of `skgpu::graphite::Caps`.
//!
//! `docs/design/gpu.md` §4.1 makes `Caps` and `DawnCaps` one concrete struct, a section for each
//! half. The port keeps the split at a trait instead, for one reason: the non-wgpu code (the shader
//! generators, the key layer, the recorder) must run against fake and profile-driven caps in the
//! tests without a device. [`Caps`] is `Caps.h`'s public interface (every method keeps the name and
//! meaning of its C++ accessor); the data types `Caps.h` declares ([`ResourceBindingRequirements`],
//! [`AttachmentSizePolicy`], and `SkSL::ShaderCaps` with [`default_shader_caps`]) live here, and
//! `graphite::wgpu::WgpuCaps` is `DawnCaps` plus the base class's state.

use std::fmt::Debug;

pub use skia_rust_sksl::util::ShaderCaps;

use skia_rust_core::color_type::ColorType;
use skia_rust_core::size::ISize;

use crate::gpu::gpu_types::{BackendApi, Mipmapped, Protected, Renderable};
use crate::gpu::resource_key::UniqueKey;
use crate::graphite::compute_pipeline_desc::ComputePipelineDesc;
use crate::graphite::graphics_pipeline_desc::GraphicsPipelineDesc;
use crate::graphite::graphite_types::{DepthStencilFlags, SampleCount};
use crate::graphite::render_pass_desc::{AttachmentDesc, RenderPassDesc};
use crate::graphite::resource_types::DstReadStrategy;
use crate::graphite::resource_types::{Discardable, ImmutableSamplerInfo, Layout};
use crate::graphite::texture_format::TextureFormat;
use crate::graphite::texture_info::TextureInfo;

/// How the sizes of auxiliary attachments relate to the main texture's.
// Port of: src/gpu/graphite/Caps.h#L138-L147 (chrome/m156)
#[doc(alias = "Caps::AttachmentSizePolicy")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttachmentSizePolicy {
    /// Auxiliary attachments must have the exact same size as the main texture.
    #[doc(alias = "kExact")]
    Exact,
    /// Auxiliary attachments can be made larger via `GetApproxSize()`.
    #[doc(alias = "kApprox")]
    Approx,
    /// MSAA-only attachments can be made smaller to fit the render bounds.
    #[doc(alias = "kMSAARenderArea")]
    MsaaRenderArea,
}

/// `ResourceBindingRequirements`.
// Port of: src/gpu/graphite/Caps.h#L55-L100 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // mirrors the C++ struct
pub struct ResourceBindingRequirements {
    /// The API of the backend currently in use.
    pub backend_api: BackendApi,
    /// The required data layout rules for the contents of a uniform buffer.
    pub uniform_buffer_layout: Layout,
    /// The required data layout rules for the contents of a storage buffer.
    pub storage_buffer_layout: Layout,
    /// Whether textures and samplers are bound separately.
    pub separate_texture_and_sampler_binding: bool,
    /// Whether intrinsic constants are stored as push constants (immediates).
    pub use_push_constants_for_intrinsic_constants: bool,
    /// Whether compute shader textures use separate index ranges from other resources.
    pub compute_uses_distinct_idx_ranges_for_textures: bool,
    /// `fUniformsSetIdx`.
    pub uniforms_set_idx: i32,
    /// `fTextureSamplerSetIdx`.
    pub texture_sampler_set_idx: i32,
    /// `fInputAttachmentSetIdx`.
    pub input_attachment_set_idx: i32,
    /// `fIntrinsicBufferBinding`.
    pub intrinsic_buffer_binding: i32,
    /// `fCombinedUniformBufferBinding`.
    pub combined_uniform_buffer_binding: i32,
    /// `fStorageBufferBinding`.
    pub storage_buffer_binding: i32,
    /// Maximum texture atlas dimension for the storage buffer fallback texture.
    pub max_fallback_texture_size: i32,
    /// `fMaxFallbackTextureBytes`.
    pub max_fallback_texture_bytes: i32,
}

impl ResourceBindingRequirements {
    /// `kUnassigned`.
    pub const UNASSIGNED: i32 = -1;
}

impl Default for ResourceBindingRequirements {
    fn default() -> Self {
        Self {
            backend_api: BackendApi::Unsupported,
            uniform_buffer_layout: Layout::Invalid,
            storage_buffer_layout: Layout::Invalid,
            separate_texture_and_sampler_binding: false,
            use_push_constants_for_intrinsic_constants: false,
            compute_uses_distinct_idx_ranges_for_textures: false,
            uniforms_set_idx: Self::UNASSIGNED,
            texture_sampler_set_idx: Self::UNASSIGNED,
            input_attachment_set_idx: Self::UNASSIGNED,
            intrinsic_buffer_binding: Self::UNASSIGNED,
            combined_uniform_buffer_binding: Self::UNASSIGNED,
            storage_buffer_binding: Self::UNASSIGNED,
            max_fallback_texture_size: Self::UNASSIGNED,
            max_fallback_texture_bytes: Self::UNASSIGNED,
        }
    }
}

/// `Caps::setDefaultShaderCaps()`: the `SkSL::ShaderCaps` every backend starts from.
// Port of: src/gpu/graphite/Caps.cpp#L35-L44 (chrome/m156)
#[doc(alias = "setDefaultShaderCaps")]
#[must_use]
pub fn default_shader_caps() -> ShaderCaps {
    ShaderCaps {
        flat_interpolation_support: true,
        shader_derivative_support: true,
        explicit_texture_lod_support: true,
        sample_mask_support: true,
        infinity_support: true,
        integer_support: true,
        nonsquare_matrix_support: true,
        inverse_hyperbolic_support: true,
        float_is_32_bits: true,
        ..ShaderCaps::default()
    }
}

/// The `Caps` queries the recorder, tasks, buffer managers and shader generators make.
// Port of: src/gpu/graphite/Caps.h (chrome/m156)
pub trait Caps: Send + Sync + Debug {
    /// `maxTextureSize()`.
    #[doc(alias = "maxTextureSize")]
    fn max_texture_size(&self) -> i32;

    /// `requireOrderedRecordings()`.
    #[doc(alias = "requireOrderedRecordings")]
    fn require_ordered_recordings(&self) -> bool;

    /// `drawBufferCanBeMapped()`.
    #[doc(alias = "drawBufferCanBeMapped")]
    fn draw_buffer_can_be_mapped(&self) -> bool;

    /// `bufferMapsAreAsync()`.
    #[doc(alias = "bufferMapsAreAsync")]
    fn buffer_maps_are_async(&self) -> bool;

    /// `supportsHostImageCopy()`: only `VulkanCaps` sets it (with `VK_EXT_host_image_copy`), so
    /// it is `false` for the Dawn-style backends this port has.
    // Port of: src/gpu/graphite/Caps.h#L411 (chrome/m156)
    #[doc(alias = "supportsHostImageCopy")]
    fn supports_host_image_copy(&self) -> bool {
        false
    }

    /// `requiredUniformBufferAlignment()`: a power of two.
    #[doc(alias = "requiredUniformBufferAlignment")]
    fn required_uniform_buffer_alignment(&self) -> usize;

    /// `requiredStorageBufferAlignment()`: a power of two.
    #[doc(alias = "requiredStorageBufferAlignment")]
    fn required_storage_buffer_alignment(&self) -> usize;

    /// `requiredTransferBufferAlignment()`: a power of two.
    #[doc(alias = "requiredTransferBufferAlignment")]
    fn required_transfer_buffer_alignment(&self) -> usize;

    /// `getAlignedTextureDataRowBytes()`: the aligned rowBytes when transferring to or from a
    /// texture.
    #[doc(alias = "getAlignedTextureDataRowBytes")]
    fn get_aligned_texture_data_row_bytes(&self, row_bytes: usize, bytes_per_block: usize)
    -> usize;

    /// `fullCompressedUploadSizeMustAlignToBlockDims()`.
    #[doc(alias = "fullCompressedUploadSizeMustAlignToBlockDims")]
    fn full_compressed_upload_size_must_align_to_block_dims(&self) -> bool;

    /// `avoidDepthMode()`.
    #[doc(alias = "avoidDepthMode")]
    fn avoid_depth_mode(&self) -> bool;

    /// `attachmentSizePolicy()`.
    #[doc(alias = "attachmentSizePolicy")]
    fn attachment_size_policy(&self) -> AttachmentSizePolicy;

    /// `getDepthAttachmentDimensions()`: assumes `color_attachment_dimensions` has already been
    /// adjusted for the attachment size policy.
    #[doc(alias = "getDepthAttachmentDimensions")]
    fn get_depth_attachment_dimensions(
        &self,
        _info: &TextureInfo,
        color_attachment_dimensions: ISize,
    ) -> ISize {
        color_attachment_dimensions
    }

    /// `getDepthStencilFormat()`.
    #[doc(alias = "getDepthStencilFormat")]
    fn get_depth_stencil_format(&self, flags: DepthStencilFlags) -> TextureFormat;

    /// `getDefaultSampledTextureInfo()`.
    #[doc(alias = "getDefaultSampledTextureInfo")]
    fn get_default_sampled_texture_info(
        &self,
        color_type: ColorType,
        mipmapped: Mipmapped,
        is_protected: Protected,
        renderable: Renderable,
    ) -> TextureInfo;

    /// `getDefaultStorageTextureInfo()`.
    #[doc(alias = "getDefaultStorageTextureInfo")]
    fn get_default_storage_texture_info(&self, color_type: ColorType) -> TextureInfo;

    /// `getDefaultReadableTextureInfo()`.
    #[doc(alias = "getDefaultReadableTextureInfo")]
    fn get_default_readable_texture_info(
        &self,
        format: TextureFormat,
        is_protected: Protected,
    ) -> TextureInfo;

    /// `getTextureInfoForSampledCopy()`: the info of a texture that a copy of `info`'s texture is
    /// made into so that it can be sampled.
    #[doc(alias = "getTextureInfoForSampledCopy")]
    fn get_texture_info_for_sampled_copy(
        &self,
        info: &TextureInfo,
        mipmapped: Mipmapped,
    ) -> TextureInfo;

    /// `getDefaultAttachmentTextureInfo()`.
    #[doc(alias = "getDefaultAttachmentTextureInfo")]
    fn get_default_attachment_texture_info(
        &self,
        desc: &AttachmentDesc,
        is_protected: Protected,
        discardable: Discardable,
    ) -> TextureInfo;

    /// `getCompatibleMSAASampleCount()`.
    #[doc(alias = "getCompatibleMSAASampleCount")]
    fn get_compatible_msaa_sample_count(&self, info: &TextureInfo) -> SampleCount;

    /// `isRenderableWithMSRTSS()`.
    #[doc(alias = "isRenderableWithMSRTSS")]
    fn is_renderable_with_msrtss(&self, info: &TextureInfo) -> bool;

    /// `storageBufferSupport()`.
    #[doc(alias = "storageBufferSupport")]
    fn storage_buffer_support(&self) -> bool;

    /// `clampToBorderSupport()`: whether the backend can sample with clamp-to-border tiling.
    // Port of: src/gpu/graphite/Caps.h#L302 (chrome/m156)
    #[doc(alias = "clampToBorderSupport")]
    fn clamp_to_border_support(&self) -> bool;

    /// `getImmutableSamplerInfo(const TextureInfo&)`. Backends can override this to return
    /// sampler conversion info; by default there is no immutable sampler.
    // Port of: src/gpu/graphite/Caps.h#L225-L227 (chrome/m156)
    #[doc(alias = "getImmutableSamplerInfo")]
    fn get_immutable_sampler_info(&self, _info: &TextureInfo) -> ImmutableSamplerInfo {
        ImmutableSamplerInfo::default()
    }

    /// `toString(const ImmutableSamplerInfo&)`: a description of the immutable sampler for
    /// `PaintParamsKey::toString`. Empty by default, and for backends without YCbCr samplers.
    // Port of: src/gpu/graphite/Caps.h#L230 (chrome/m156)
    #[doc(alias = "toString")]
    fn immutable_sampler_info_to_string(&self, _info: &ImmutableSamplerInfo) -> String {
        String::new()
    }

    /// `getDstReadStrategy()`: how a draw obtains the dst color when it needs it.
    #[doc(alias = "getDstReadStrategy")]
    fn get_dst_read_strategy(&self) -> DstReadStrategy;

    /// `supportsHardwareAdvancedBlending()`: whether `blendEquationSupport()` is above basic.
    #[doc(alias = "supportsHardwareAdvancedBlending")]
    fn supports_hardware_advanced_blending(&self) -> bool;

    /// `shaderCaps()->fDualSourceBlendingSupport`.
    #[doc(alias = "fDualSourceBlendingSupport")]
    fn dual_source_blending_support(&self) -> bool;

    /// `shaderCaps()`.
    // Port of: src/gpu/graphite/Caps.h#L242 (chrome/m156)
    #[doc(alias = "shaderCaps")]
    fn shader_caps(&self) -> &ShaderCaps;

    /// `resourceBindingRequirements()`.
    // Port of: src/gpu/graphite/Caps.h#L242-L244 (chrome/m156)
    #[doc(alias = "resourceBindingRequirements")]
    fn resource_binding_requirements(&self) -> &ResourceBindingRequirements;

    /// `maxVaryings()`.
    // Port of: src/gpu/graphite/Caps.h#L250 (chrome/m156)
    #[doc(alias = "maxVaryings")]
    fn max_varyings(&self) -> i32;

    /// `ndcYAxisPointsDown()`.
    // Port of: src/gpu/graphite/Caps.h#L300 (chrome/m156)
    #[doc(alias = "ndcYAxisPointsDown")]
    fn ndc_y_axis_points_down(&self) -> bool;

    /// `protectedSupport()`.
    // Port of: src/gpu/graphite/Caps.h#L304 (chrome/m156)
    #[doc(alias = "protectedSupport")]
    fn protected_support(&self) -> bool;

    /// `semaphoreSupport()`.
    // Port of: src/gpu/graphite/Caps.h#L307 (chrome/m156)
    #[doc(alias = "semaphoreSupport")]
    fn semaphore_support(&self) -> bool;

    /// `allowCpuSync()`.
    // Port of: src/gpu/graphite/Caps.h#L310 (chrome/m156)
    #[doc(alias = "allowCpuSync")]
    fn allow_cpu_sync(&self) -> bool;

    /// `storageBufferSupportForCompute()`.
    // Port of: src/gpu/graphite/Caps.h#L322 (chrome/m156)
    #[doc(alias = "storageBufferSupportForCompute")]
    fn storage_buffer_support_for_compute(&self) -> bool;

    /// `computeSupport()`.
    // Port of: src/gpu/graphite/Caps.h#L339 (chrome/m156)
    #[doc(alias = "computeSupport")]
    fn compute_support(&self) -> bool;

    /// `avoidMSAA()`.
    // Port of: src/gpu/graphite/Caps.h#L124 (chrome/m156)
    #[doc(alias = "avoidMSAA")]
    fn avoid_msaa(&self) -> bool;

    /// `msaaRenderToSingleSampledSupport()`.
    // Port of: src/gpu/graphite/Caps.h#L134 (chrome/m156)
    #[doc(alias = "msaaRenderToSingleSampledSupport")]
    fn msaa_render_to_single_sampled_support(&self) -> bool;

    /// `useDrawListLayer()`.
    // Port of: src/gpu/graphite/Caps.h#L233 (chrome/m156)
    #[doc(alias = "useDrawListLayer")]
    fn use_draw_list_layer(&self) -> bool;

    /// `loadOpAffectsMSAAPipelines()`.
    // Port of: src/gpu/graphite/Caps.h#L122 (chrome/m156)
    #[doc(alias = "loadOpAffectsMSAAPipelines")]
    fn load_op_affects_msaa_pipelines(&self) -> bool;

    /// `maxPathAtlasTextureSize()`.
    // Port of: src/gpu/graphite/Caps.h#L426 (chrome/m156)
    #[doc(alias = "maxPathAtlasTextureSize")]
    fn max_path_atlas_texture_size(&self) -> i32;

    /// `allowMultipleAtlasTextures()`.
    // Port of: src/gpu/graphite/Caps.h#L428 (chrome/m156)
    #[doc(alias = "allowMultipleAtlasTextures")]
    fn allow_multiple_atlas_textures(&self) -> bool;

    /// `supportBilerpFromGlyphAtlas()`.
    // Port of: src/gpu/graphite/Caps.h#L429 (chrome/m156)
    #[doc(alias = "supportBilerpFromGlyphAtlas")]
    fn support_bilerp_from_glyph_atlas(&self) -> bool;

    /// `setBackendLabels()`.
    // Port of: src/gpu/graphite/Caps.h#L435 (chrome/m156)
    #[doc(alias = "setBackendLabels")]
    fn set_backend_labels(&self) -> bool;

    /// `isSampleCountSupported(format, sampleCount)`.
    // Port of: src/gpu/graphite/Caps.h#L168 (chrome/m156)
    #[doc(alias = "isSampleCountSupported")]
    fn is_sample_count_supported(&self, format: TextureFormat, sample_count: SampleCount) -> bool;

    /// `isTexturable(info, allowMSAA)`.
    // Port of: src/gpu/graphite/Caps.h#L204 (chrome/m156)
    #[doc(alias = "isTexturable")]
    fn is_texturable(&self, info: &TextureInfo, allow_msaa: bool) -> bool;

    /// `isReadable(info, allowMSAA)`.
    // Port of: src/gpu/graphite/Caps.h#L207 (chrome/m156)
    #[doc(alias = "isReadable")]
    fn is_readable(&self, info: &TextureInfo, allow_msaa: bool) -> bool;

    /// `isRenderable(info)`.
    // Port of: src/gpu/graphite/Caps.h#L209 (chrome/m156)
    #[doc(alias = "isRenderable")]
    fn is_renderable(&self, info: &TextureInfo) -> bool;

    /// `isCopyableSrc(info)`.
    // Port of: src/gpu/graphite/Caps.h#L214 (chrome/m156)
    #[doc(alias = "isCopyableSrc")]
    fn is_copyable_src(&self, info: &TextureInfo) -> bool;

    /// `isCopyableDst(info)`.
    // Port of: src/gpu/graphite/Caps.h#L217 (chrome/m156)
    #[doc(alias = "isCopyableDst")]
    fn is_copyable_dst(&self, info: &TextureInfo) -> bool;

    /// `isStorage(info)`.
    // Port of: src/gpu/graphite/Caps.h#L219 (chrome/m156)
    #[doc(alias = "isStorage")]
    fn is_storage(&self, info: &TextureInfo) -> bool;

    /// `makeGraphicsPipelineKey(pipelineDesc, renderPassDesc)`: the key of the graphics pipeline
    /// made from the descriptions, in the backend's key domain.
    // Port of: src/gpu/graphite/Caps.h#L114-L115 (chrome/m156)
    #[doc(alias = "makeGraphicsPipelineKey")]
    fn make_graphics_pipeline_key(
        &self,
        pipeline_desc: &GraphicsPipelineDesc,
        render_pass_desc: &RenderPassDesc,
    ) -> UniqueKey;

    /// `makeComputePipelineKey(pipelineDesc)`.
    // Port of: src/gpu/graphite/Caps.h#L116 (chrome/m156)
    #[doc(alias = "makeComputePipelineKey")]
    fn make_compute_pipeline_key(&self, pipeline_desc: &ComputePipelineDesc) -> UniqueKey;
}
