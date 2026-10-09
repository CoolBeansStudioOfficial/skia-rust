// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/CommandBuffer.h (the calls tasks make),
//                   src/gpu/graphite/CommandTypes.h (BufferTextureCopyData)

//! The seam between tasks and the command buffer.
//!
//! `CommandBuffer` (the backend-neutral half plus the wgpu half) is ported with G9b/G11c as one
//! concrete struct. [`CommandBuffer`] lists the calls the tasks, the buffer managers and
//! `Recording` make; the implementation keeps the C++ semantics (each call returns false on
//! failure, and the buffers and textures passed by handle are tracked until the work finishes).

use std::sync::Arc;

use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;

use crate::gpu::ref_cnted_callback::RefCntedCallback;
use crate::graphite::buffer::Buffer;
use crate::graphite::render_pass_desc::RenderPassDesc;
use crate::graphite::resource::{AnyResourceRef, Resource, ResourceRef};
use crate::graphite::task::compute_task::DispatchGroup;
use crate::graphite::task::render_pass_task::DrawPass;
use crate::graphite::texture::Texture;

/// Specifies a single region for copying, either from buffer to texture, or vice versa.
// Port of: src/gpu/graphite/CommandTypes.h#L16-L22 (chrome/m156)
#[doc(alias = "skgpu::graphite::BufferTextureCopyData")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferTextureCopyData {
    /// `fBufferOffset`.
    pub buffer_offset: usize,
    /// `fBufferRowBytes`.
    pub buffer_row_bytes: usize,
    /// `fRect`.
    pub rect: IRect,
    /// `fMipLevel`.
    pub mip_level: u32,
}

/// Specifies a scissor, which can only be subsequently queried given a translation and clip which
/// are assumed to be applied to all commands in the render pass in which the scissor is set.
// Port of: src/gpu/graphite/CommandTypes.h#L28-L42 (chrome/m156)
#[doc(alias = "skgpu::graphite::Scissor")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scissor {
    rect: IRect,
}

impl Scissor {
    /// `Scissor(rect)`.
    #[must_use]
    pub const fn new(rect: IRect) -> Self {
        Self { rect }
    }

    /// `getRect(replayTranslation, replayClip)`.
    #[doc(alias = "getRect")]
    #[must_use]
    pub fn get_rect(&self, replay_translation: IPoint, replay_clip: IRect) -> IRect {
        let rect = self.rect.with_offset(replay_translation);
        IRect::intersect(&rect, &replay_clip).unwrap_or_else(IRect::new_empty)
    }
}

/// The `CommandBuffer` calls made by tasks and managers.
///
/// Buffers and textures that Skia passes as `const T*` (not owned by the task) are passed as
/// `&Arc<Resource<T>>`, the non-owning handle ([`ResourceRef::as_arc`]); those passed as
/// `sk_sp<T>` are passed as [`ResourceRef`]s.
// Port of: src/gpu/graphite/CommandBuffer.h (chrome/m156)
pub trait CommandBuffer {
    /// `trackResource()`: keeps `resource` alive (with a command buffer ref) until the GPU
    /// work finishes.
    #[doc(alias = "trackResource")]
    fn track_resource(&mut self, resource: AnyResourceRef);

    /// `addFinishedProc()`.
    #[doc(alias = "addFinishedProc")]
    fn add_finished_proc(&mut self, finished_proc: Arc<RefCntedCallback>);

    /// `setReplayTranslationAndClip()`: false if the clip does not intersect the render target
    /// bounds, so the whole render pass can be skipped.
    #[doc(alias = "setReplayTranslationAndClip")]
    fn set_replay_translation_and_clip(
        &mut self,
        translation: IPoint,
        clip: IRect,
        render_target_bounds: IRect,
    ) -> bool;

    /// `addRenderPass()`.
    #[doc(alias = "addRenderPass")]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    fn add_render_pass(
        &mut self,
        render_pass_desc: &RenderPassDesc,
        color_texture: ResourceRef<Texture>,
        resolve_texture: Option<ResourceRef<Texture>>,
        depth_stencil_texture: Option<ResourceRef<Texture>>,
        dst_copy: Option<&Arc<Resource<Texture>>>,
        dst_read_bounds: IRect,
        resolve_offset: IPoint,
        viewport_dims: ISize,
        draw_passes: &[Box<dyn DrawPass>],
    ) -> bool;

    /// `addComputePass()`.
    #[doc(alias = "addComputePass")]
    fn add_compute_pass(&mut self, dispatches: &[Box<dyn DispatchGroup>]) -> bool;

    /// `copyBufferToBuffer()`.
    #[doc(alias = "copyBufferToBuffer")]
    fn copy_buffer_to_buffer(
        &mut self,
        src_buffer: &Arc<Resource<Buffer>>,
        src_offset: usize,
        dst_buffer: ResourceRef<Buffer>,
        dst_offset: usize,
        size: usize,
    ) -> bool;

    /// `copyTextureToBuffer()`.
    #[doc(alias = "copyTextureToBuffer")]
    fn copy_texture_to_buffer(
        &mut self,
        texture: ResourceRef<Texture>,
        src_rect: IRect,
        buffer: ResourceRef<Buffer>,
        buffer_offset: usize,
        buffer_row_bytes: usize,
    ) -> bool;

    /// `copyBufferToTexture()`.
    #[doc(alias = "copyBufferToTexture")]
    fn copy_buffer_to_texture(
        &mut self,
        buffer: &Arc<Resource<Buffer>>,
        texture: ResourceRef<Texture>,
        copy_data: &[BufferTextureCopyData],
    ) -> bool;

    /// `copyTextureToTexture()`.
    #[doc(alias = "copyTextureToTexture")]
    fn copy_texture_to_texture(
        &mut self,
        src: ResourceRef<Texture>,
        src_rect: IRect,
        dst: ResourceRef<Texture>,
        dst_point: IPoint,
        dst_level: i32,
    ) -> bool;

    /// `synchronizeBufferToCpu()`.
    #[doc(alias = "synchronizeBufferToCpu")]
    fn synchronize_buffer_to_cpu(&mut self, buffer: ResourceRef<Buffer>) -> bool;

    /// `clearBuffer()`.
    #[doc(alias = "clearBuffer")]
    fn clear_buffer(&mut self, buffer: &Arc<Resource<Buffer>>, offset: usize, size: usize) -> bool;
}
