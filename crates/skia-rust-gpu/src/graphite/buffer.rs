// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/Buffer.h, src/gpu/graphite/Buffer.cpp,
//                   src/gpu/graphite/ResourceTypes.h (BindBufferInfo)

//! `skgpu::graphite::Buffer`: a GPU buffer resource, and `BindBufferInfo`: a range of one.
//!
//! # Mapping without raw pointers
//!
//! Skia's `Buffer::map()` returns a `void*` into mapped GPU memory that stays valid until
//! `unmap()`. The port has no raw pointers, so mapping hands the caller an owned CPU staging
//! block ([`MappedData`], one `Vec<u8>` of the buffer's size) instead. The buffer stays
//! [`mapped`](Buffer::is_mapped) while the caller holds the block, and
//! [`Buffer::unmap_with`] gives the block back so the backend can commit the bytes to the GPU
//! buffer (the writes a mapped pointer would have made directly). The wgpu backend fills the
//! GPU buffer from the block in `on_unmap`, which is what Dawn's mapped-at-creation range does.
//!
//! The backend half is the [`BufferBackend`] trait (`docs/design/gpu.md` §4.1: the seam until
//! the wgpu backend exists).

use std::any::Any;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::gpu::gpu_types::{CallbackResult, Protected};
use crate::graphite::resource::{Resource, ResourceObject, ResourceRef, synchronize_backend_label};
use crate::graphite::resource_types::Ownership;

/// The CPU staging block of a mapped buffer: `size()` bytes that become the buffer's contents
/// on [`Buffer::unmap_with`].
pub type MappedData = Vec<u8>;

/// `GpuFinishedProc` for an async map: called with the result once the map is ready or failed.
pub type MapFinishedProc = Box<dyn FnOnce(CallbackResult) + Send>;

/// The backend half of a [`Buffer`] (the virtual functions a backend buffer overrides).
// Port of: src/gpu/graphite/Buffer.h#L47-L56 (chrome/m156)
pub trait BufferBackend: Send + Sync + fmt::Debug + 'static {
    /// `freeGpuData()`.
    fn free_gpu_data(&self);

    /// `setBackendLabel()`.
    fn set_backend_label(&self, _label: &str) {}

    /// `Caps::bufferMapsAreAsync()` for this buffer's context.
    fn buffer_maps_are_async(&self) -> bool {
        false
    }

    /// `onMap()`: maps the buffer and returns its staging block, or `None` if mapping failed.
    /// For a buffer that is written by the CPU the block's contents are unspecified (the GPU
    /// buffer's previous contents are not preserved).
    #[doc(alias = "onMap")]
    fn on_map(&self, size: usize) -> Option<MappedData>;

    /// `onAsyncMap()`: starts an asynchronous map. Not supported by default
    /// (`SK_ABORT("Async buffer mapping not supported")`).
    ///
    /// # Panics
    /// By default, as Skia aborts.
    #[doc(alias = "onAsyncMap")]
    fn on_async_map(&self, _finished: Option<MapFinishedProc>) {
        panic!("Async buffer mapping not supported");
    }

    /// `onUnmap()`: unmaps the buffer. `written` is the staging block the caller wrote, if
    /// the caller handed it back with [`Buffer::unmap_with`]; the backend commits it.
    #[doc(alias = "onUnmap")]
    fn on_unmap(&self, written: Option<&[u8]>);

    /// `isUnmappable()` override: `is_mapped` is the base class's answer.
    #[doc(alias = "isUnmappable")]
    fn is_unmappable(&self, is_mapped: bool) -> bool {
        is_mapped
    }

    /// `onUpdateGpuMemorySize()`.
    fn on_update_gpu_memory_size(&self, current: usize) -> usize {
        current
    }

    /// For downcasting to the concrete backend buffer.
    fn as_any(&self) -> &dyn Any;
}

/// A GPU buffer.
// Port of: src/gpu/graphite/Buffer.h#L26-L72 (chrome/m156)
#[doc(alias = "skgpu::graphite::Buffer")]
pub struct Buffer {
    size: usize,
    is_protected: Protected,
    // `fMapPtr != nullptr`.
    is_mapped: AtomicBool,
    backend: Box<dyn BufferBackend>,
}

impl fmt::Debug for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Buffer")
            .field("size", &self.size)
            .field("is_protected", &self.is_protected)
            .field("is_mapped", &self.is_mapped.load(Ordering::Relaxed))
            .field("backend", &self.backend)
            .finish()
    }
}

impl Buffer {
    /// `Buffer(sharedContext, size, isProtected, label, reusableRequiresPurgeable,
    /// requiresPrepareForReturnToCache)`: creates the buffer resource, holding one usage ref.
    // Port of: src/gpu/graphite/Buffer.h#L45-L62 (chrome/m156)
    #[must_use]
    pub fn make(
        size: usize,
        is_protected: Protected,
        label: &str,
        reusable_requires_purgeable: bool,
        requires_prepare_for_return_to_cache: bool,
        backend: Box<dyn BufferBackend>,
    ) -> ResourceRef<Buffer> {
        let buffer = Resource::new(
            Buffer {
                size,
                is_protected,
                is_mapped: AtomicBool::new(false),
                backend,
            },
            Ownership::Owned,
            size,
            label,
            reusable_requires_purgeable,
            requires_prepare_for_return_to_cache,
        );
        synchronize_backend_label(&*buffer.erased());
        buffer
    }

    /// `size()`.
    #[must_use]
    pub fn size(&self) -> usize {
        self.size
    }

    /// `isMapped()`.
    #[doc(alias = "isMapped")]
    #[must_use]
    pub fn is_mapped(&self) -> bool {
        self.is_mapped.load(Ordering::Acquire)
    }

    /// `isUnmappable()`: true if mapped or an `asyncMap()` was started and hasn't been
    /// completed or canceled.
    #[doc(alias = "isUnmappable")]
    #[must_use]
    pub fn is_unmappable(&self) -> bool {
        self.backend.is_unmappable(self.is_mapped())
    }

    /// `map()`: maps the buffer (a synchronous map if it is not mapped yet) and returns the
    /// CPU staging block, or `None` if mapping failed or the block is already held by a
    /// caller. See the module docs.
    // Port of: src/gpu/graphite/Buffer.cpp#L18-L25 (chrome/m156)
    #[must_use]
    pub fn map(&self) -> Option<MappedData> {
        debug_assert!(self.is_unmappable() || !self.backend.buffer_maps_are_async());
        debug_assert!(self.is_protected == Protected::No);
        if self.is_mapped() {
            // The staging block is held by whoever mapped the buffer first.
            return None;
        }
        let data = self.backend.on_map(self.size)?;
        debug_assert_eq!(data.len(), self.size);
        self.is_mapped.store(true, Ordering::Release);
        Some(data)
    }

    /// `asyncMap()`: starts a new asynchronous map.
    // Port of: src/gpu/graphite/Buffer.cpp#L27-L31 (chrome/m156)
    #[doc(alias = "asyncMap")]
    pub fn async_map(&self, finished: Option<MapFinishedProc>) {
        debug_assert!(self.backend.buffer_maps_are_async());
        debug_assert!(self.is_protected == Protected::No);
        self.backend.on_async_map(finished);
    }

    /// `unmap()`: if the buffer is mapped then unmaps it, discarding the staging block. If an
    /// async map is pending then it is cancelled.
    // Port of: src/gpu/graphite/Buffer.cpp#L33-L37 (chrome/m156)
    pub fn unmap(&self) {
        debug_assert!(self.is_unmappable());
        self.backend.on_unmap(None);
        self.is_mapped.store(false, Ordering::Release);
    }

    /// `unmap()` for a buffer whose staging block the caller wrote: the backend commits
    /// `data` to the GPU buffer.
    // Port of: src/gpu/graphite/Buffer.cpp#L33-L37 (chrome/m156)
    pub fn unmap_with(&self, data: &[u8]) {
        debug_assert!(self.is_unmappable());
        debug_assert_eq!(data.len(), self.size);
        self.backend.on_unmap(Some(data));
        self.is_mapped.store(false, Ordering::Release);
    }

    /// `isProtected()`.
    #[doc(alias = "isProtected")]
    #[must_use]
    pub fn is_protected(&self) -> Protected {
        self.is_protected
    }

    /// The backend half.
    #[must_use]
    pub fn backend(&self) -> &dyn BufferBackend {
        &*self.backend
    }
}

impl ResourceObject for Buffer {
    fn resource_type(&self) -> &'static str {
        "Buffer"
    }

    fn free_gpu_data(&self) {
        self.backend.free_gpu_data();
    }

    fn set_backend_label(&self, label: &str) {
        self.backend.set_backend_label(label);
    }

    fn on_update_gpu_memory_size(&self, current: usize) -> usize {
        self.backend.on_update_gpu_memory_size(current)
    }

    fn is_protected(&self) -> Protected {
        self.is_protected
    }
}

/// A range of a [`Buffer`] to bind.
///
/// Like Skia's `const Buffer*` it does not own a usage ref: it keeps the buffer's memory alive
/// but leaves reference counts alone, so the owner (a buffer manager, a `Recording` or a command
/// buffer) must hold the real [`ResourceRef`]. Two infos are equal if they name the same buffer
/// and, when there is one, the same range.
// Port of: src/gpu/graphite/ResourceTypes.h#L197-L208 (chrome/m156)
#[doc(alias = "skgpu::graphite::BindBufferInfo")]
#[derive(Clone, Debug, Default)]
pub struct BindBufferInfo {
    /// `fBuffer`.
    pub buffer: Option<Arc<Resource<Buffer>>>,
    /// `fOffset`.
    pub offset: u32,
    /// `fSize`.
    pub size: u32,
}

impl BindBufferInfo {
    /// A range of `buffer`.
    #[must_use]
    pub fn new(buffer: &ResourceRef<Buffer>, offset: u32, size: u32) -> Self {
        Self {
            buffer: Some(buffer.as_arc().clone()),
            offset,
            size,
        }
    }

    /// `explicit operator bool()`: true if there is a buffer.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.buffer.is_some()
    }

    /// A handle owning a new usage ref on the buffer (`sk_ref_sp(info.fBuffer)`).
    ///
    /// # Panics
    /// If there is no buffer.
    #[must_use]
    pub fn ref_buffer(&self) -> ResourceRef<Buffer> {
        ResourceRef::from_arc(self.buffer.as_ref().expect("BindBufferInfo has a buffer"))
    }
}

impl PartialEq for BindBufferInfo {
    // Port of: src/gpu/graphite/ResourceTypes.h#L204-L206 (chrome/m156)
    fn eq(&self, o: &Self) -> bool {
        let same_buffer = match (&self.buffer, &o.buffer) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        same_buffer && (self.buffer.is_none() || (self.offset == o.offset && self.size == o.size))
    }
}

impl Eq for BindBufferInfo {}
