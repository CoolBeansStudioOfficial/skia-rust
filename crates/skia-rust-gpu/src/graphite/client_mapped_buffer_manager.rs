// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ClientMappedBufferManager.h,
//                   src/gpu/graphite/ClientMappedBufferManager.cpp

//! `ClientMappedBufferManager`: `TClientMappedBufferManager` for Graphite buffers and contexts.

use std::sync::atomic::{AtomicU32, Ordering};

use crate::gpu::async_read_types::{
    BufferFinishedMessage, BufferFinishedSender, ClientMappedBuffer, TClientMappedBufferManager,
};
use crate::graphite::buffer::Buffer;
use crate::graphite::resource::ResourceRef;

/// `Context::ContextID`: a process-unique id of a context (0 is invalid).
///
/// The context (G9b) re-exports this as `Context::ContextID`.
// Port of: include/gpu/graphite/Context.h#L316-L331 (chrome/m156)
#[doc(alias = "Context::ContextID")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ContextId(u32);

impl ContextId {
    /// `ContextID::Next()`.
    // Port of: src/gpu/graphite/Context.cpp#L112-L119 (chrome/m156)
    #[must_use]
    pub fn next() -> ContextId {
        static NEXT_ID: AtomicU32 = AtomicU32::new(1);
        loop {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            // SK_InvalidUniqueID is 0
            if id != 0 {
                return ContextId(id);
            }
        }
    }

    /// `makeInvalid()`.
    #[doc(alias = "makeInvalid")]
    pub fn make_invalid(&mut self) {
        self.0 = 0;
    }

    /// `isValid()`.
    #[doc(alias = "isValid")]
    #[must_use]
    pub fn is_valid(self) -> bool {
        self.0 != 0
    }
}

impl ClientMappedBuffer for ResourceRef<Buffer> {
    fn unmap(&self) {
        let buffer: &Buffer = self;
        buffer.unmap();
    }

    fn is_same_buffer(&self, other: &Self) -> bool {
        ResourceRef::ptr_eq(self, other)
    }
}

/// The message that tells the manager a buffer is ready to be unmapped.
pub type ClientBufferFinishedMessage = BufferFinishedMessage<ResourceRef<Buffer>, ContextId>;

/// Posts [`ClientBufferFinishedMessage`]s to a [`ClientMappedBufferManager`].
pub type ClientBufferFinishedSender = BufferFinishedSender<ResourceRef<Buffer>, ContextId>;

/// This is declared as a class rather than an alias to allow for forward declarations.
// Port of: src/gpu/graphite/ClientMappedBufferManager.h#L19-L26 (chrome/m156)
#[doc(alias = "skgpu::graphite::ClientMappedBufferManager")]
#[derive(Debug)]
pub struct ClientMappedBufferManager {
    inner: TClientMappedBufferManager<ResourceRef<Buffer>, ContextId>,
}

impl ClientMappedBufferManager {
    /// `ClientMappedBufferManager(ownerID)`.
    #[must_use]
    pub fn new(owner_id: ContextId) -> Self {
        Self {
            inner: TClientMappedBufferManager::new(owner_id),
        }
    }

    /// `ownerID()`.
    #[doc(alias = "ownerID")]
    #[must_use]
    pub fn owner_id(&self) -> ContextId {
        self.inner.owner_id()
    }

    /// A handle to post `BufferFinishedMessage`s to this manager with.
    #[must_use]
    pub fn sender(&self) -> ClientBufferFinishedSender {
        self.inner.sender()
    }

    /// `insert()`.
    pub fn insert(&mut self, b: ResourceRef<Buffer>) {
        self.inner.insert(b);
    }

    /// `process()`.
    pub fn process(&mut self) {
        self.inner.process();
    }

    /// `abandon()`.
    pub fn abandon(&mut self) {
        self.inner.abandon();
    }

    /// The number of buffers the clients still hold.
    #[must_use]
    pub fn num_client_held_buffers(&self) -> usize {
        self.inner.num_client_held_buffers()
    }
}
