// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/Caps.h (only the queries G9a code makes)

//! The slice of `skgpu::graphite::Caps` that the recorder, tasks and buffer managers read.
//!
//! `Caps` itself (the backend-neutral half plus the wgpu one) is ported with G6/G11a as one
//! concrete struct (`docs/design/gpu.md` §4.1). Until then this trait is the seam: every method
//! keeps the name and meaning of the C++ `Caps` accessor, so the concrete struct implements the
//! trait (or replaces it, keeping these method names) without touching the callers.

use std::fmt::Debug;

use skia_rust_core::size::ISize;

use crate::gpu::gpu_types::Protected;
use crate::graphite::graphite_types::{DepthStencilFlags, SampleCount};
use crate::graphite::render_pass_desc::AttachmentDesc;
use crate::graphite::resource_types::{Discardable, ImmutableSamplerInfo};
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

/// The `Caps` queries the recorder, tasks and buffer managers make.
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

    /// `toString(const ImmutableSamplerInfo&)`: a description of the immutable sampler for
    /// `PaintParamsKey::toString`. Empty by default, and for backends without YCbCr samplers.
    // Port of: src/gpu/graphite/Caps.h#L230 (chrome/m156)
    #[doc(alias = "toString")]
    fn immutable_sampler_info_to_string(&self, _info: &ImmutableSamplerInfo) -> String {
        String::new()
    }
}
