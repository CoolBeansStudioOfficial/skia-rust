// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/task/CopyTask.h, src/gpu/graphite/task/CopyTask.cpp

//! `CopyBufferToBufferTask`, `CopyTextureToBufferTask` and `CopyTextureToTextureTask`.

use std::sync::Arc;

use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;

use crate::gpu::sk_log::skia_log_e;
use crate::graphite::buffer::Buffer;
use crate::graphite::command_buffer::CommandBuffer;
use crate::graphite::context_priv::ContextPriv;
use crate::graphite::resource::{Resource, ResourceRef};
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::scratch_resource_manager::ScratchResourceManager;
use crate::graphite::task::{ReplayTargetData, Status, Task, TaskRef};
use crate::graphite::texture_format::texture_format_name;
use crate::graphite::texture_proxy::TextureProxy;

/// Copies a range of one buffer to another.
// Port of: src/gpu/graphite/task/CopyTask.h#L28-L69 (chrome/m156)
#[doc(alias = "skgpu::graphite::CopyBufferToBufferTask")]
#[derive(Debug)]
pub struct CopyBufferToBufferTask {
    // The srcBuffer for this Task is always a transfer buffer which is owned by the
    // UploadBufferManager. Thus we don't have to take a ref to it as the UploadBufferManager
    // will handle its refs and passing them to the Recording.
    src_buffer: Arc<Resource<Buffer>>,
    src_offset: usize,
    dst_buffer: ResourceRef<Buffer>,
    dst_offset: usize,
    size: usize,
}

impl CopyBufferToBufferTask {
    /// `Make(srcBuffer, srcOffset, dstBuffer, dstOffset, size)`.
    // Port of: src/gpu/graphite/task/CopyTask.cpp#L20-L35 (chrome/m156)
    #[must_use]
    pub fn make(
        src_buffer: &Arc<Resource<Buffer>>,
        src_offset: usize,
        dst_buffer: ResourceRef<Buffer>,
        dst_offset: usize,
        size: usize,
    ) -> TaskRef {
        debug_assert!(size <= src_buffer.size() - src_offset);
        debug_assert!(size <= dst_buffer.size() - dst_offset);
        Task::CopyBufferToBuffer(CopyBufferToBufferTask {
            src_buffer: src_buffer.clone(),
            src_offset,
            dst_buffer,
            dst_offset,
            size,
        })
        .into_ref()
    }

    /// `prepareResources()`.
    pub fn prepare_resources(
        &mut self,
        _resource_provider: &mut ResourceProvider,
        _scratch_manager: &mut ScratchResourceManager,
        _runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
    ) -> Status {
        Status::Success
    }

    /// `addCommands()`.
    // Port of: src/gpu/graphite/task/CopyTask.cpp#L50-L60 (chrome/m156)
    pub fn add_commands(
        &mut self,
        _context: &mut dyn ContextPriv,
        command_buffer: &mut dyn CommandBuffer,
        _replay_data: &ReplayTargetData,
    ) -> Status {
        if command_buffer.copy_buffer_to_buffer(
            &self.src_buffer,
            self.src_offset,
            self.dst_buffer.clone(),
            self.dst_offset,
            self.size,
        ) {
            Status::Success
        } else {
            Status::Fail
        }
    }
}

/// Copies a rectangle of a texture to a buffer.
// Port of: src/gpu/graphite/task/CopyTask.h#L72-L113 (chrome/m156)
#[doc(alias = "skgpu::graphite::CopyTextureToBufferTask")]
#[derive(Debug)]
pub struct CopyTextureToBufferTask {
    texture_proxy: Arc<TextureProxy>,
    src_rect: IRect,
    // `std::move`d into the command buffer by addCommands().
    buffer: Option<ResourceRef<Buffer>>,
    buffer_offset: usize,
    buffer_row_bytes: usize,
}

impl CopyTextureToBufferTask {
    /// `Make(textureProxy, srcRect, buffer, bufferOffset, bufferRowBytes)`: `None` if there is
    /// no texture proxy.
    // Port of: src/gpu/graphite/task/CopyTask.cpp#L62-L76 (chrome/m156)
    #[must_use]
    pub fn make(
        texture_proxy: Option<Arc<TextureProxy>>,
        src_rect: IRect,
        buffer: ResourceRef<Buffer>,
        buffer_offset: usize,
        buffer_row_bytes: usize,
    ) -> Option<TaskRef> {
        let texture_proxy = texture_proxy?;
        Some(
            Task::CopyTextureToBuffer(CopyTextureToBufferTask {
                texture_proxy,
                src_rect,
                buffer: Some(buffer),
                buffer_offset,
                buffer_row_bytes,
            })
            .into_ref(),
        )
    }

    /// `prepareResources()`.
    // Port of: src/gpu/graphite/task/CopyTask.cpp#L90-L104 (chrome/m156)
    pub fn prepare_resources(
        &mut self,
        _resource_provider: &mut ResourceProvider,
        _scratch_manager: &mut ScratchResourceManager,
        _runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
    ) -> Status {
        // If the source texture hasn't been instantiated yet, it means there was no prior task
        // that could have initialized its contents so a readback to a buffer does not make
        // sense.
        debug_assert!(self.texture_proxy.is_instantiated() || self.texture_proxy.is_lazy());

        // TODO: The copy is also a consumer of the source, so it should participate in returning
        // scratch resources like RenderPassTask does. For now, though, all copy tasks side step
        // reuse entirely and they cannot participate until they've been moved into scoping
        // tasks like DrawTask first.
        Status::Success
    }

    /// `addCommands()`.
    // Port of: src/gpu/graphite/task/CopyTask.cpp#L106-L121 (chrome/m156)
    pub fn add_commands(
        &mut self,
        _context: &mut dyn ContextPriv,
        command_buffer: &mut dyn CommandBuffer,
        _replay_data: &ReplayTargetData,
    ) -> Status {
        // C++ moves a null buffer into the command buffer on a second run; the port fails the
        // step instead.
        let (Some(texture), Some(buffer)) = (self.texture_proxy.ref_texture(), self.buffer.take())
        else {
            return Status::Fail;
        };
        if command_buffer.copy_texture_to_buffer(
            texture,
            self.src_rect,
            buffer,
            self.buffer_offset,
            self.buffer_row_bytes,
        ) {
            // TODO(b/332681367): CopyTextureToBuffer is currently only used for readback
            // operations, which are a one-time event. Should this just default to returning
            // kDiscard?
            Status::Success
        } else {
            Status::Fail
        }
    }

    /// `visitProxies()`: the texture is the source of the copy, so it's always read.
    pub fn visit_proxies(
        &mut self,
        visitor: &mut dyn FnMut(&Arc<TextureProxy>) -> bool,
        _reads_only: bool,
    ) -> bool {
        visitor(&self.texture_proxy)
    }
}

/// Copies a rectangle of one texture to another.
// Port of: src/gpu/graphite/task/CopyTask.h#L116-L165 (chrome/m156)
#[doc(alias = "skgpu::graphite::CopyTextureToTextureTask")]
#[derive(Debug)]
pub struct CopyTextureToTextureTask {
    src_proxy: Arc<TextureProxy>,
    src_rect: IRect,
    dst_proxy: Arc<TextureProxy>,
    dst_point: IPoint,
    dst_level: i32,
}

impl CopyTextureToTextureTask {
    /// `Make(srcProxy, srcRect, dstProxy, dstPoint, dstLevel)`: `None` if a proxy is missing or
    /// the formats differ.
    // Port of: src/gpu/graphite/task/CopyTask.cpp#L123-L152 (chrome/m156)
    #[must_use]
    pub fn make(
        src_proxy: Option<Arc<TextureProxy>>,
        src_rect: IRect,
        dst_proxy: Option<Arc<TextureProxy>>,
        dst_point: IPoint,
        dst_level: i32,
    ) -> Option<TaskRef> {
        let (src_proxy, dst_proxy) = (src_proxy?, dst_proxy?);

        // Texture-to-texture copies do not do format conversions
        let src_format = src_proxy.format();
        let dst_format = dst_proxy.format();
        if src_format != dst_format {
            skia_log_e!(
                "Unable to copy between textures of different formats, src = {}, dst = {}",
                texture_format_name(src_format),
                texture_format_name(dst_format)
            );
            return None;
        }

        Some(
            Task::CopyTextureToTexture(CopyTextureToTextureTask {
                src_proxy,
                src_rect,
                dst_proxy,
                dst_point,
                dst_level,
            })
            .into_ref(),
        )
    }

    /// `prepareResources()`.
    // Port of: src/gpu/graphite/task/CopyTask.cpp#L165-L191 (chrome/m156)
    pub fn prepare_resources(
        &mut self,
        resource_provider: &mut ResourceProvider,
        _scratch_manager: &mut ScratchResourceManager,
        _runtime_dict: Option<&Arc<RuntimeEffectDictionary>>,
    ) -> Status {
        // Do not instantiate the src proxy. If the source texture hasn't been instantiated yet,
        // it means there was no prior task that could have initialized its contents so
        // propagating the undefined contents to the dst does not make sense.
        // TODO(b/333729316): Assert that fSrcProxy is instantiated or lazy; right now it may not
        // be instantatiated if this is a dst readback copy for a scratch Device. In that case, a
        // RenderPassTask will immediately follow this copy task and instantiate the source proxy
        // so that addCommands() has a texture to operate on. That said, the texture's contents
        // will be undefined when the copy is executed ideally it just shouldn't happen.

        // TODO: The copy is also a consumer of the source, so it should participate in returning
        // scratch resources like RenderPassTask does. For now, though, all copy tasks side step
        // reuse entirely and they cannot participate until they've been moved into scoping tasks
        // like DrawTask first. In particular, for texture-to-texture copies, they should be
        // scoped to not invoke pending listeners for a subsequent RenderPassTask.

        // TODO: Use the scratch resource manager to instantiate fDstProxy, although the details
        // of when that texture can be returned need to be worked out. While brittle, all current
        // use cases of scratch texture-to-texture copies have the dst used immediately by the
        // next task, so it could just add a pending listener that returns the texture w/o any
        // read counting.
        if !TextureProxy::instantiate_if_not_lazy(resource_provider, &self.dst_proxy) {
            skia_log_e!("Could not instantiate dst texture proxy for CopyTextureToTextureTask!");
            return Status::Fail;
        }
        Status::Success
    }

    /// `addCommands()`.
    // Port of: src/gpu/graphite/task/CopyTask.cpp#L193-L212 (chrome/m156)
    pub fn add_commands(
        &mut self,
        _context: &mut dyn ContextPriv,
        command_buffer: &mut dyn CommandBuffer,
        _replay_data: &ReplayTargetData,
    ) -> Status {
        // prepareResources() doesn't instantiate the source assuming that a prior task will have
        // do so as part of initializing the texture contents.
        debug_assert!(self.src_proxy.is_instantiated());
        let (Some(src), Some(dst)) = (self.src_proxy.ref_texture(), self.dst_proxy.ref_texture())
        else {
            return Status::Fail;
        };

        if command_buffer.copy_texture_to_texture(
            src,
            self.src_rect,
            dst,
            self.dst_point,
            self.dst_level,
        ) {
            // TODO(b/332681367): The calling context should be able to specify whether or not
            // this copy is a repeatable operation (e.g. dst readback copy for blending) or one
            // time (e.g. client asked for a copy of an image or surface).
            Status::Success
        } else {
            Status::Fail
        }
    }

    /// `visitProxies()`: only visit the dst if `reads_only` is false; the src is the only
    /// texture being read.
    pub fn visit_proxies(
        &mut self,
        visitor: &mut dyn FnMut(&Arc<TextureProxy>) -> bool,
        reads_only: bool,
    ) -> bool {
        visitor(&self.src_proxy) && (reads_only || visitor(&self.dst_proxy))
    }
}
