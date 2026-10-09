// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnBuffer.h, DawnBuffer.cpp

//! `DawnBuffer` on wgpu: the backend half of a Graphite [`Buffer`].
//!
//! # Mapping
//!
//! Dawn hands out a raw pointer into the mapped range (`fMapPtr`). The port's `Buffer::map()`
//! hands out an owned CPU staging block instead (see `buffer.rs`), and the bytes the caller wrote
//! are copied into the wgpu mapped range when the block is given back
//! (`Buffer::unmap_with()`), which is what writing through the pointer did. The three states of
//! the wgpu buffer (`wgpu::BufferMapState` in Dawn's `GetMapState()`) are tracked in
//! [`MapState`].
//!
//! # Differences from Dawn
//!
//! - wgpu buffers cannot be relabeled, so the label is set at creation.
//! - There is no `wgpu::BufferMapState` query: [`MapState`] is the buffer's own record.
//! - Map callbacks run when the device is polled (`WgpuSharedContext::tick()`), as Dawn's
//!   `AllowSpontaneous` callbacks run when the instance processes events.

use std::any::Any;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::gpu::gpu_types::{CallbackResult, Protected};
use crate::gpu::sk_log::{skia_log_e, skia_log_w};
use crate::graphite::buffer::{Buffer, BufferBackend, MapFinishedProc, MappedData};
use crate::graphite::resource::{AnyResourceRef, ResourceRef};
use crate::graphite::resource_types::{AccessPattern, BufferType};
use crate::graphite::wgpu::async_wait::create_checked;
use crate::graphite::wgpu::shared_context::WgpuSharedContext;
use crate::graphite::wgpu::texture::backend_label;

/// `wgpu::BufferMapState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum MapState {
    Unmapped = 0,
    /// An async map was requested and has not completed.
    Pending = 1,
    /// Mapped: at creation, or after a completed async map.
    Mapped = 2,
}

/// The state shared with the map callback.
struct Shared {
    /// `fBuffer`; `None` after `freeGpuData()`.
    buffer: Mutex<Option<wgpu::Buffer>>,
    usage: wgpu::BufferUsages,
    state: AtomicU8,
    /// `fAsyncMapCallbacks`.
    async_map_callbacks: Mutex<Vec<MapFinishedProc>>,
    /// `Caps::bufferMapsAreAsync()`.
    buffer_maps_are_async: bool,
    /// `fCachedSingleBufferBindGroups`.
    cached_single_buffer_bind_groups: Mutex<Vec<(usize, wgpu::BindGroup)>>,
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shared")
            .field("usage", &self.usage)
            .field("state", &self.map_state())
            .finish_non_exhaustive()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    fn map_state(&self) -> MapState {
        match self.state.load(Ordering::Acquire) {
            0 => MapState::Unmapped,
            1 => MapState::Pending,
            _ => MapState::Mapped,
        }
    }

    fn set_map_state(&self, state: MapState) {
        self.state.store(state as u8, Ordering::Release);
    }

    // Port of: src/gpu/graphite/dawn/DawnBuffer.cpp#L188-L205 (chrome/m156)
    fn map_callback(&self, result: &Result<(), wgpu::BufferAsyncError>) {
        let callbacks = std::mem::take(&mut *lock(&self.async_map_callbacks));
        let result = match result {
            Ok(()) => {
                self.set_map_state(MapState::Mapped);
                CallbackResult::Success
            }
            Err(error) => {
                self.set_map_state(MapState::Unmapped);
                skia_log_e!("Buffer async map failed with {error}.");
                CallbackResult::Failed
            }
        };
        // The AutoCallbacks run when they are destroyed, with the result set above.
        for callback in callbacks {
            callback(result);
        }
    }

    /// `onAsyncMap()`.
    // Port of: src/gpu/graphite/dawn/DawnBuffer.cpp#L140-L186 (chrome/m156)
    fn async_map(self: &Arc<Self>, finished: Option<MapFinishedProc>) {
        // This function is only useful where we have to use asyncMap().
        debug_assert!(self.buffer_maps_are_async);
        if let Some(finished) = finished {
            if self.map_state() == MapState::Mapped {
                finished(CallbackResult::Success);
                return;
            }
            lock(&self.async_map_callbacks).push(finished);
        }

        if self.map_state() != MapState::Unmapped {
            return;
        }

        let Some(buffer) = lock(&self.buffer).clone() else {
            return;
        };
        debug_assert!(
            self.usage
                .intersects(wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::MAP_WRITE)
        );
        let mode = if self.usage.contains(wgpu::BufferUsages::MAP_WRITE) {
            wgpu::MapMode::Write
        } else {
            wgpu::MapMode::Read
        };
        self.set_map_state(MapState::Pending);
        let shared = Arc::clone(self);
        buffer
            .slice(..)
            .map_async(mode, move |result| shared.map_callback(&result));
    }
}

/// The wgpu half of a [`Buffer`].
// Port of: src/gpu/graphite/dawn/DawnBuffer.h#L21-L66 (chrome/m156)
#[doc(alias = "DawnBuffer")]
#[derive(Debug)]
pub struct WgpuBuffer {
    shared: Arc<Shared>,
}

/// `DawnBuffer::Make()`'s usage for a buffer.
// Port of: src/gpu/graphite/dawn/DawnBuffer.cpp#L100-L160 (chrome/m156)
fn buffer_usage(
    shared_context: &WgpuSharedContext,
    ty: BufferType,
    access_pattern: AccessPattern,
) -> wgpu::BufferUsages {
    use wgpu::BufferUsages as U;
    let mut usage = match ty {
        BufferType::Vertex => U::VERTEX | U::COPY_DST,
        BufferType::Index => U::INDEX | U::COPY_DST,
        BufferType::XferCpuToGpu => U::COPY_SRC | U::MAP_WRITE,
        BufferType::XferGpuToCpu => U::COPY_DST | U::MAP_READ,
        BufferType::Uniform => U::UNIFORM | U::COPY_DST,
        BufferType::Storage => U::STORAGE | U::COPY_DST | U::COPY_SRC,
        BufferType::Query => U::QUERY_RESOLVE | U::COPY_SRC,
        BufferType::Indirect => U::INDIRECT | U::STORAGE | U::COPY_DST,
        BufferType::VertexStorage => U::VERTEX | U::STORAGE,
        BufferType::IndexStorage => U::INDEX | U::STORAGE,
    };
    if shared_context.caps().draw_buffer_can_be_mapped()
        && access_pattern == AccessPattern::HostVisible
        && ty != BufferType::XferGpuToCpu
    {
        if ty == BufferType::Query {
            // We can map the query buffer to get the results directly rather than having to copy
            // to a transfer buffer.
            usage |= U::MAP_READ;
        } else {
            // If the buffer is intended to be mappable, add MapWrite usage and remove CopyDst.
            // We don't want to allow both CPU and GPU to write to the same buffer.
            usage |= U::MAP_WRITE;
            usage &= !U::COPY_DST;
        }
    }
    if access_pattern == AccessPattern::GpuOnlyCopySrc {
        usage |= U::COPY_SRC;
    }
    usage
}

impl WgpuBuffer {
    /// `DawnBuffer::Make()`: creates a buffer resource, or `None` if the size is zero or wgpu
    /// rejects the description.
    // Port of: src/gpu/graphite/dawn/DawnBuffer.cpp#L96-L188 (chrome/m156)
    #[must_use]
    pub fn make(
        shared_context: &WgpuSharedContext,
        size: usize,
        ty: BufferType,
        access_pattern: AccessPattern,
        label: &str,
    ) -> Option<ResourceRef<Buffer>> {
        if size == 0 {
            return None;
        }

        let usage = buffer_usage(shared_context, ty, access_pattern);
        let desc = wgpu::BufferDescriptor {
            label: backend_label(shared_context, label),
            size: size as u64,
            usage,
            // Specifying mappedAtCreation avoids clearing the buffer on the GPU which can cause
            // MapAsync to be very slow as it waits for GPU execution to complete.
            mapped_at_creation: usage.contains(wgpu::BufferUsages::MAP_WRITE),
        };
        let buffer = create_checked(
            shared_context.device(),
            shared_context.caps().allow_scoped_error_checks(),
            || shared_context.device().create_buffer(&desc),
        )?;

        let mapped_at_creation = desc.mapped_at_creation;
        let shared = Arc::new(Shared {
            buffer: Mutex::new(Some(buffer)),
            usage,
            state: AtomicU8::new(if mapped_at_creation {
                MapState::Mapped as u8
            } else {
                MapState::Unmapped as u8
            }),
            async_map_callbacks: Mutex::new(Vec::new()),
            buffer_maps_are_async: shared_context.caps().buffer_maps_are_async(),
            cached_single_buffer_bind_groups: Mutex::new(Vec::new()),
        });
        let may_be_remapped = usage.contains(wgpu::BufferUsages::MAP_WRITE);
        Some(Buffer::make(
            size,
            // Dawn doesn't support protected memory
            Protected::No,
            label,
            /* reusableRequiresPurgeable= */ may_be_remapped,
            // prepareForReturnToCache only needs to be called for a buffer that is mappable for
            // writing
            /* requiresPrepareForReturnToCache= */
            may_be_remapped,
            Box::new(Self { shared }),
        ))
    }

    /// `dawnBuffer()`: the wgpu buffer, absent once freed.
    #[doc(alias = "dawnBuffer")]
    #[must_use]
    pub fn wgpu_buffer(&self) -> Option<wgpu::Buffer> {
        lock(&self.shared.buffer).clone()
    }

    /// `getCachedSingleBufferBindGroup()`.
    // Port of: src/gpu/graphite/dawn/DawnBuffer.cpp#L304-L312 (chrome/m156)
    #[doc(alias = "getCachedSingleBufferBindGroup")]
    #[must_use]
    pub fn get_cached_single_buffer_bind_group(
        &self,
        binding_size: usize,
    ) -> Option<wgpu::BindGroup> {
        lock(&self.shared.cached_single_buffer_bind_groups)
            .iter()
            .find(|(size, _)| *size == binding_size)
            .map(|(_, bind_group)| bind_group.clone())
    }

    /// `addCachedSingleBufferBindGroup()`.
    // Port of: src/gpu/graphite/dawn/DawnBuffer.cpp#L314-L317 (chrome/m156)
    #[doc(alias = "addCachedSingleBufferBindGroup")]
    pub fn add_cached_single_buffer_bind_group(
        &self,
        bind_group: wgpu::BindGroup,
        binding_size: usize,
    ) {
        lock(&self.shared.cached_single_buffer_bind_groups).push((binding_size, bind_group));
    }
}

impl BufferBackend for WgpuBuffer {
    // Port of: src/gpu/graphite/dawn/DawnBuffer.cpp#L282-L293 (chrome/m156)
    fn free_gpu_data(&self) {
        let buffer = lock(&self.shared.buffer).take();
        if let Some(buffer) = buffer {
            // Explicitly destroy the buffer since it might be ref'd by cached bind groups which
            // are not immediately cleaned up. Graphite should already guarantee that all command
            // buffers using this buffer (indirectly via BindGroups) are already completed.
            buffer.destroy();
        }
        lock(&self.shared.cached_single_buffer_bind_groups).clear();
    }

    fn buffer_maps_are_async(&self) -> bool {
        self.shared.buffer_maps_are_async
    }

    // Port of: src/gpu/graphite/dawn/DawnBuffer.cpp#L207-L210 (chrome/m156)
    fn on_map(&self, size: usize) -> Option<MappedData> {
        if self.shared.map_state() != MapState::Mapped {
            skia_log_w!("Synchronous buffer mapping not supported in Dawn. Failing map request.");
            return None;
        }
        let buffer = lock(&self.shared.buffer).clone()?;
        if self.shared.usage.contains(wgpu::BufferUsages::MAP_WRITE) {
            // The contents are unspecified for a buffer the CPU writes.
            Some(vec![0; size])
        } else {
            // If buffer is only created with MapRead usage, the mapped range is read only.
            let view = buffer.slice(..).get_mapped_range().ok()?;
            Some(view.to_vec())
        }
    }

    fn on_async_map(&self, finished: Option<MapFinishedProc>) {
        self.shared.async_map(finished);
    }

    // Port of: src/gpu/graphite/dawn/DawnBuffer.cpp#L212-L217 (chrome/m156)
    fn on_unmap(&self, written: Option<&[u8]>) {
        let Some(buffer) = lock(&self.shared.buffer).clone() else {
            return;
        };
        debug_assert!(self.is_unmappable(false));
        if let Some(written) = written
            && self.shared.map_state() == MapState::Mapped
            && self.shared.usage.contains(wgpu::BufferUsages::MAP_WRITE)
        {
            let mut view = buffer
                .slice(..)
                .get_mapped_range_mut()
                .expect("a mapped buffer has a mapped range");
            view.copy_from_slice(written);
        }
        self.shared.set_map_state(MapState::Unmapped);
        buffer.unmap();
    }

    // Port of: src/gpu/graphite/dawn/DawnBuffer.cpp#L272-L275 (chrome/m156)
    fn is_unmappable(&self, _is_mapped: bool) -> bool {
        self.shared.map_state() != MapState::Unmapped
    }

    // Port of: src/gpu/graphite/dawn/DawnBuffer.cpp#L117-L138 (chrome/m156)
    fn prepare_for_return_to_cache(&self, take_ref: &mut dyn FnMut() -> AnyResourceRef) -> bool {
        // This implementation is almost Dawn-agnostic. However, Buffer base class doesn't have
        // any way of distinguishing a buffer that is mappable for writing from one mappable for
        // reading. We only need to re-map the former.
        debug_assert!(self.shared.usage.contains(wgpu::BufferUsages::MAP_WRITE));

        // Note that the map state cannot change on another thread when we are here. We got here
        // because there were no UsageRefs on the buffer but async mapping holds a UsageRef until
        // it completes.
        if self.shared.map_state() == MapState::Mapped {
            return false;
        }

        let buffer_ref = take_ref();
        self.shared.async_map(Some(Box::new(move |result| {
            if result != CallbackResult::Success
                && let Ok(buffer) = buffer_ref.downcast::<Buffer>()
            {
                buffer.base().set_delete_asap();
            }
            // Dropping the ref returns the buffer to the cache.
        })));
        true
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The wgpu half of `buffer`, if it is a wgpu buffer.
#[must_use]
pub fn as_wgpu_buffer(buffer: &Buffer) -> Option<&WgpuBuffer> {
    buffer.backend().as_any().downcast_ref::<WgpuBuffer>()
}
