// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/UploadBufferManager.h, src/gpu/graphite/UploadBufferManager.cpp

//! `UploadBufferManager`: hands out ranges of mapped CPU-to-GPU transfer buffers.
//!
//! skia-rust: the mapped memory is the buffers' CPU staging blocks (see
//! [`crate::graphite::buffer`]), which the manager owns until the buffers are transferred to a
//! `Recording` or `CommandBuffer`, where they are handed back with
//! [`Buffer::unmap_with`](crate::graphite::buffer::Buffer::unmap_with). The writer for a range
//! therefore borrows the manager.

use skia_rust_core::align::align_non_pow2;

use crate::gpu::buffer_writer::TextureUploadWriter;
use crate::graphite::buffer::{BindBufferInfo, Buffer, MappedData};
use crate::graphite::caps::Caps;
use crate::graphite::command_buffer::CommandBuffer;
use crate::graphite::context_priv::SharedResourceProvider;
use crate::graphite::recording::Recording;
use crate::graphite::resource::{Resource, ResourceRef};
use crate::graphite::resource_types::{AccessPattern, BufferType};

const REUSED_BUFFER_SIZE: usize = 64 << 10; // 64 KB

/// Hands out ranges of mapped transfer buffers for CPU data going to the GPU.
#[doc(alias = "skgpu::graphite::UploadBufferManager")]
pub struct UploadBufferManager {
    resource_provider: SharedResourceProvider,
    reused_buffer: Option<ResourceRef<Buffer>>,
    // The staging block of `reused_buffer` (empty if there is none).
    reused_data: MappedData,
    min_alignment: u32,
    reused_buffer_offset: u32,
    used_buffers: Vec<(ResourceRef<Buffer>, MappedData)>,
}

impl std::fmt::Debug for UploadBufferManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadBufferManager")
            .field("min_alignment", &self.min_alignment)
            .field("reused_buffer_offset", &self.reused_buffer_offset)
            .field("num_used_buffers", &self.used_buffers.len())
            .finish_non_exhaustive()
    }
}

impl UploadBufferManager {
    /// `UploadBufferManager(resourceProvider, caps)`.
    // Port of: src/gpu/graphite/UploadBufferManager.cpp#L22-L26 (chrome/m156)
    ///
    /// # Panics
    /// If the caps' transfer buffer alignment does not fit in 32 bits.
    #[must_use]
    pub fn new(resource_provider: SharedResourceProvider, caps: &dyn Caps) -> Self {
        Self {
            resource_provider,
            reused_buffer: None,
            reused_data: Vec::new(),
            min_alignment: u32::try_from(caps.required_transfer_buffer_alignment())
                .expect("transfer buffer alignment fits in 32 bits"),
            reused_buffer_offset: 0,
            used_buffers: Vec::new(),
        }
    }

    /// `getTextureUploadWriter()`: a writer over `required_bytes` of mapped transfer memory and
    /// the range of the buffer it fills, or `None` if no buffer could be made or mapped.
    // Port of: src/gpu/graphite/UploadBufferManager.cpp#L30-L42 (chrome/m156)
    #[doc(alias = "getTextureUploadWriter")]
    pub fn get_texture_upload_writer(
        &mut self,
        required_bytes: usize,
        required_alignment: usize,
    ) -> Option<(TextureUploadWriter<'_>, BindBufferInfo)> {
        let (map, bind_info) =
            self.make_bind_info(required_bytes, required_alignment, "TextureUploadBuffer")?;
        Some((
            TextureUploadWriter::new(&mut map[..required_bytes]),
            bind_info,
        ))
    }

    /// `makeBindInfo()`: `required_bytes` (aligned up to the alignment) of mapped memory, and
    /// the range it covers. The memory is at least `required_bytes` long.
    ///
    /// Unlike the draw and static buffer managers, this does not track whether a buffer mapping
    /// has failed. It is common for uploads to be scoped to a specific image creation. In that
    /// case, the image can be returned as null to signal a very isolated failure instead of
    /// taking down the entire `Recording`.
    ///
    /// # Panics
    /// If the alignment does not fit in 32 bits.
    // Port of: src/gpu/graphite/UploadBufferManager.cpp#L44-L106 (chrome/m156)
    #[doc(alias = "makeBindInfo")]
    pub fn make_bind_info(
        &mut self,
        required_bytes: usize,
        required_alignment: usize,
        label: &str,
    ) -> Option<(&mut [u8], BindBufferInfo)> {
        let Ok(required_bytes) = u32::try_from(required_bytes) else {
            return None;
        };
        let required_alignment32 = u32::try_from(required_alignment)
            .expect("alignment fits in 32 bits")
            .max(self.min_alignment);
        let required_bytes32 = align_non_pow2(required_bytes, required_alignment32);

        if required_bytes32 as usize > REUSED_BUFFER_SIZE {
            // Create a dedicated buffer for this request.
            let buffer = self.lock_provider().find_or_create_non_shareable_buffer(
                required_bytes32 as usize,
                BufferType::XferCpuToGpu,
                AccessPattern::HostVisible,
                label,
            );
            let data = buffer.as_ref().and_then(|buffer| buffer.map());
            let (Some(buffer), Some(data)) = (buffer, data) else {
                return None;
            };
            let bind_info = BindBufferInfo::new(&buffer, 0, required_bytes32);
            self.used_buffers.push((buffer, data));
            let (_, data) = self.used_buffers.last_mut().expect("just pushed");
            return Some((&mut data[..], bind_info));
        }

        // Try to reuse an already-allocated buffer.
        self.reused_buffer_offset = align_non_pow2(self.reused_buffer_offset, required_alignment32);
        if let Some(reused) = &self.reused_buffer
            // `size_t` arithmetic in C++: an offset past the end wraps to a huge value.
            && required_bytes32 as usize
                > reused.size().wrapping_sub(self.reused_buffer_offset as usize)
        {
            let buffer = self.reused_buffer.take().expect("checked above");
            let data = std::mem::take(&mut self.reused_data);
            self.used_buffers.push((buffer, data));
        }
        if self.reused_buffer.is_none() {
            let buffer = self.lock_provider().find_or_create_non_shareable_buffer(
                REUSED_BUFFER_SIZE,
                BufferType::XferCpuToGpu,
                AccessPattern::HostVisible,
                label,
            );
            self.reused_buffer_offset = 0;
            let data = buffer.as_ref().and_then(|buffer| buffer.map());
            let (Some(buffer), Some(data)) = (buffer, data) else {
                self.reused_buffer = None;
                return None;
            };
            self.reused_buffer = Some(buffer);
            self.reused_data = data;
        }

        let reused = self.reused_buffer.as_ref().expect("created above");
        let bind_info = BindBufferInfo::new(reused, self.reused_buffer_offset, required_bytes32);
        let start = self.reused_buffer_offset as usize;
        self.reused_buffer_offset += required_bytes32;
        Some((
            &mut self.reused_data[start..start + required_bytes32 as usize],
            bind_info,
        ))
    }

    /// Copies `data` to the start of `binding`'s range of a buffer this manager still holds
    /// (the staging block of a transfer buffer). This is how a `DrawBufferManager` buffer that
    /// goes through a transfer buffer fills it: C++ wrote through the pointer `makeBindInfo()`
    /// returned, and the staged bytes cannot outlive a borrow of the manager here.
    ///
    /// # Panics
    /// If the manager does not hold the binding's buffer.
    pub fn write_range(&mut self, binding: &BindBufferInfo, data: &[u8]) {
        let target = binding.buffer.as_ref().expect("binding has a buffer");
        let staged = if self
            .reused_buffer
            .as_ref()
            .is_some_and(|reused| std::sync::Arc::ptr_eq(reused.as_arc(), target))
        {
            &mut self.reused_data
        } else {
            self.used_buffers
                .iter_mut()
                .find(|(buffer, _)| std::sync::Arc::ptr_eq(buffer.as_arc(), target))
                .map(|(_, staged)| staged)
                .expect("the upload buffer manager holds the transfer buffer")
        };
        let start = binding.offset as usize;
        let len = data.len().min(staged.len() - start);
        staged[start..start + len].copy_from_slice(&data[..len]);
    }

    /// The mapped memory of `buffer`, which this manager still holds mapped: what
    /// `Buffer::map()` returns for a buffer that is already mapped (the same pointer), which the
    /// port's `Buffer::map()` hands out only once, to this manager. `None` if the manager does
    /// not hold the buffer (it was transferred to a recording or command buffer, which commit
    /// the bytes to the GPU buffer).
    #[must_use]
    pub fn mapped_data(&self, buffer: &std::sync::Arc<Resource<Buffer>>) -> Option<&[u8]> {
        if let Some(reused) = &self.reused_buffer
            && std::sync::Arc::ptr_eq(reused.as_arc(), buffer)
        {
            return Some(&self.reused_data);
        }
        self.used_buffers
            .iter()
            .find(|(used, _)| std::sync::Arc::ptr_eq(used.as_arc(), buffer))
            .map(|(_, data)| &data[..])
    }

    fn lock_provider(
        &self,
    ) -> std::sync::MutexGuard<'_, crate::graphite::resource_provider::ResourceProvider> {
        self.resource_provider
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `transferToRecording()`: finalizes all buffers and transfers ownership of them to a
    /// `Recording`.
    // Port of: src/gpu/graphite/UploadBufferManager.cpp#L108-L119 (chrome/m156)
    #[doc(alias = "transferToRecording")]
    pub fn transfer_to_recording(&mut self, recording: &mut Recording) {
        for (buffer, data) in self.used_buffers.drain(..) {
            buffer.unmap_with(&data);
            recording.priv_().add_resource_ref(buffer.into_any());
        }
        if let Some(buffer) = self.reused_buffer.take() {
            let data = std::mem::take(&mut self.reused_data);
            buffer.unmap_with(&data);
            recording.priv_().add_resource_ref(buffer.into_any());
        }
    }

    /// `transferToCommandBuffer()`.
    // Port of: src/gpu/graphite/UploadBufferManager.cpp#L121-L132 (chrome/m156)
    #[doc(alias = "transferToCommandBuffer")]
    pub fn transfer_to_command_buffer(&mut self, command_buffer: &mut dyn CommandBuffer) {
        for buffer in self.take_buffers() {
            command_buffer.track_resource(buffer.into_any());
        }
    }

    /// Finalizes all buffers (commits their staging blocks to the GPU buffers) and hands them
    /// to the caller, who must keep them alive until the GPU work that reads them finishes. It is
    /// `transferToCommandBuffer()` for a caller that has no command buffer yet.
    #[must_use]
    pub fn take_buffers(&mut self) -> Vec<ResourceRef<Buffer>> {
        let mut buffers = Vec::with_capacity(self.used_buffers.len() + 1);
        for (buffer, data) in self.used_buffers.drain(..) {
            buffer.unmap_with(&data);
            buffers.push(buffer);
        }
        if let Some(buffer) = self.reused_buffer.take() {
            let data = std::mem::take(&mut self.reused_data);
            buffer.unmap_with(&data);
            buffers.push(buffer);
        }
        buffers
    }
}
