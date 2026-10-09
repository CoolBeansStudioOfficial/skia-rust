// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnCommandBuffer.h (the backend half only)

//! The wgpu half of `CommandBuffer`.
//!
//! The neutral half is [`CommandBufferCore`]. This is its backend, [`WgpuCommandBufferBackend`].
//! Recording GPU work (render passes, compute passes, copies, readback) is G11c: until then every
//! hook reports failure, so [`CommandBufferCore`] never holds a command. A command buffer that
//! only tracks resources and finished procs (an empty Recording, or one whose tasks add nothing)
//! is then complete, and submitting it is a fence.

use crate::gpu::gpu_types::Protected;
use crate::graphite::buffer::Buffer;
use crate::graphite::command_buffer::{
    BufferTextureCopyData, CommandBufferBackend, CommandBufferCore, RenderPassCall, ReplayState,
};
use crate::graphite::context_priv::SharedResourceProvider;
use crate::graphite::task::compute_task::DispatchGroup;
use crate::graphite::texture::Texture;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;

/// The wgpu command buffer: the neutral core over the wgpu backend half.
// Port of: src/gpu/graphite/dawn/DawnCommandBuffer.h (chrome/m156)
#[doc(alias = "skgpu::graphite::DawnCommandBuffer")]
pub type WgpuCommandBuffer = CommandBufferCore<WgpuCommandBufferBackend>;

/// Creates a wgpu command buffer for `is_protected` work.
// Port of: src/gpu/graphite/dawn/DawnQueueManager.cpp#L110-L115 (chrome/m156)
#[must_use]
pub fn new_wgpu_command_buffer(
    is_protected: Protected,
    resource_provider: SharedResourceProvider,
) -> WgpuCommandBuffer {
    CommandBufferCore::new(is_protected, resource_provider, WgpuCommandBufferBackend)
}

/// The backend half of [`WgpuCommandBuffer`]. Its hooks are the recording calls, which are not
/// ported yet (G11c).
// Port of: src/gpu/graphite/dawn/DawnCommandBuffer.h (chrome/m156)
#[derive(Debug, Default)]
pub struct WgpuCommandBufferBackend;

impl CommandBufferBackend for WgpuCommandBufferBackend {
    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp (setNewCommandBufferResources, chrome/m156)
    fn set_new_command_buffer_resources(&mut self) -> bool {
        true
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp (onResetCommandBuffer, chrome/m156)
    fn on_reset_command_buffer(&mut self) {}

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp (onAddRenderPass, chrome/m156)
    // Not ported yet (G11c): render passes are not recorded.
    fn on_add_render_pass(&mut self, _state: &ReplayState, _call: &RenderPassCall<'_>) -> bool {
        false
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp (onAddComputePass, chrome/m156)
    // Not ported yet (G11c): compute passes are not recorded.
    fn on_add_compute_pass(&mut self, _dispatch_groups: &[Box<dyn DispatchGroup>]) -> bool {
        false
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp (onCopyBufferToBuffer, chrome/m156)
    // Not ported yet (G11c): copies are not recorded.
    fn on_copy_buffer_to_buffer(
        &mut self,
        _src_buffer: &Buffer,
        _src_offset: usize,
        _dst_buffer: &Buffer,
        _dst_offset: usize,
        _size: usize,
    ) -> bool {
        false
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp (onCopyTextureToBuffer, chrome/m156)
    // Not ported yet (G11c): readback copies are not recorded.
    fn on_copy_texture_to_buffer(
        &mut self,
        _texture: &Texture,
        _src_rect: IRect,
        _buffer: &Buffer,
        _buffer_offset: usize,
        _buffer_row_bytes: usize,
    ) -> bool {
        false
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp (onCopyBufferToTexture, chrome/m156)
    // Not ported yet (G11c): uploads are not recorded.
    fn on_copy_buffer_to_texture(
        &mut self,
        _buffer: &Buffer,
        _texture: &Texture,
        _copy_data: &[BufferTextureCopyData],
    ) -> bool {
        false
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp (onCopyTextureToTexture, chrome/m156)
    // Not ported yet (G11c): texture copies are not recorded.
    fn on_copy_texture_to_texture(
        &mut self,
        _src: &Texture,
        _src_rect: IRect,
        _dst: &Texture,
        _dst_point: IPoint,
        _mip_level: i32,
    ) -> bool {
        false
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp (onSynchronizeBufferToCpu, chrome/m156)
    // Not ported yet (G11c): buffer synchronization is not recorded.
    fn on_synchronize_buffer_to_cpu(
        &mut self,
        _buffer: &Buffer,
        _did_result_in_work: &mut bool,
    ) -> bool {
        false
    }

    // Port of: src/gpu/graphite/dawn/DawnCommandBuffer.cpp (onClearBuffer, chrome/m156)
    // Not ported yet (G11c): buffer clears are not recorded.
    fn on_clear_buffer(&mut self, _buffer: &Buffer, _offset: usize, _size: usize) -> bool {
        false
    }
}
