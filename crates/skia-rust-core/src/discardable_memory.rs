// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/private/chromium/SkDiscardableMemory.h (chrome/m156)

//! Discardable memory: memory the system may take back while it is unlocked.
//!
//! The embedder normally supplies the implementation. skia-rust supplies the default one, a pool
//! that counts unlocked memory against a budget (see [`crate::discardable_memory_pool`]).

use crate::discardable_memory_pool::global_pool;

/// Interface for discardable memory (`SkDiscardableMemory`).
///
/// Memory is locked when it is created. [`DiscardableMemory::data`] is only meaningful while the
/// memory is locked; an unlocked block may be purged at any time, and a later [`lock`] reports
/// whether its contents survived.
///
/// [`lock`]: DiscardableMemory::lock
#[doc(alias = "SkDiscardableMemory")]
pub trait DiscardableMemory: Send {
    /// Locks the memory, preventing it from being discarded.
    ///
    /// Returns `false` if the memory was discarded and the lock failed. Nested locks are not
    /// allowed.
    #[must_use]
    fn lock(&mut self) -> bool;

    /// Returns the memory. Only valid while locked; `None` once the memory has been discarded.
    fn data(&self) -> Option<&[u8]>;

    /// Returns the memory for writing. Only valid while locked; `None` once discarded.
    fn data_mut(&mut self) -> Option<&mut [u8]>;

    /// Unlocks the memory so that it can be purged. Must follow every successful lock.
    fn unlock(&mut self);
}

/// A factory that returns a locked discardable memory of the given size, or `None` on failure
/// (`SkResourceCache::DiscardableFactory`).
pub type DiscardableFactory = fn(usize) -> Option<Box<dyn DiscardableMemory>>;

/// Returns a locked discardable memory of `bytes` from the global pool, or `None` on failure
/// (`SkDiscardableMemory::Create`).
#[doc(alias = "SkDiscardableMemory::Create")]
#[must_use]
pub fn create(bytes: usize) -> Option<Box<dyn DiscardableMemory>> {
    global_pool().create(bytes)
}
