// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkCachedData.{h,cpp} (chrome/m156)

//! Reference-counted data that a cache can unlock, so that it can be purged.
//!
//! The reference count is kept explicitly, as in Skia, because it counts the cache's own ownership
//! separately from the clients'. [`CachedData::add_ref`] and [`CachedData::unref`] stand for
//! `ref()` and `unref()`. [`CachedData::attach_to_cache_and_ref`] and
//! [`CachedData::detach_from_cache_and_unref`] are the cache's pair. A reference count that reaches
//! zero releases the storage, as the deletion does in Skia; the object itself stays in its `Arc`
//! until the Rust owners drop it.
//!
//! The Rust type is always used behind an `Arc`, which the callers hold, so every method takes
//! `&self`.

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::discardable_memory::DiscardableMemory;
use crate::strike_cache::lock_unpoisoned;

/// The memory behind a [`CachedData`].
enum Storage {
    /// Plain heap memory, which is always available.
    Malloc(Box<[u8]>),
    /// Memory that the system can discard while it is unlocked.
    Discardable(Box<dyn DiscardableMemory>),
}

#[derive(Debug)]
struct State {
    storage: Option<Storage>,
    /// Whether `fData` is non-null: the data is locked and, for discardable memory, the lock
    /// succeeded.
    has_data: bool,
    /// `fRefCnt`: the low bit in Skia means the cache owns it. Here the cache's reference is
    /// counted in the same total, as in Skia.
    ref_cnt: i32,
    in_cache: bool,
    is_locked: bool,
}

impl std::fmt::Debug for Storage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malloc(bytes) => f.debug_tuple("Malloc").field(&bytes.len()).finish(),
            Self::Discardable(_) => f.write_str("Discardable"),
        }
    }
}

impl State {
    fn bytes(&self) -> Option<&[u8]> {
        if !self.has_data {
            return None;
        }
        match self.storage.as_ref()? {
            Storage::Malloc(bytes) => Some(bytes),
            Storage::Discardable(dm) => dm.data(),
        }
    }

    fn bytes_mut(&mut self) -> Option<&mut [u8]> {
        if !self.has_data {
            return None;
        }
        match self.storage.as_mut()? {
            Storage::Malloc(bytes) => Some(bytes),
            Storage::Discardable(dm) => dm.data_mut(),
        }
    }

    /// `SkCachedData::inMutexLock`
    fn in_mutex_lock(&mut self) {
        debug_assert!(!self.is_locked);
        self.is_locked = true;
        match self.storage.as_mut() {
            Some(Storage::Discardable(dm)) => {
                // A failed lock means the contents are gone.
                self.has_data = dm.lock();
            }
            Some(Storage::Malloc(_)) => self.has_data = true,
            None => self.has_data = false,
        }
    }

    /// `SkCachedData::inMutexUnlock`
    fn in_mutex_unlock(&mut self) {
        debug_assert!(self.is_locked);
        self.is_locked = false;
        // Only when the previous lock succeeded.
        if let (true, Some(Storage::Discardable(dm))) = (self.has_data, self.storage.as_mut()) {
            dm.unlock();
        }
        // Malloc storage has nothing to do: its memory stays allocated.
        self.has_data = false;
    }

    /// `SkCachedData::inMutexRef`
    fn in_mutex_ref(&mut self, from_cache: bool) {
        if self.ref_cnt == 1 && self.in_cache {
            self.in_mutex_lock();
        }
        self.ref_cnt += 1;
        if from_cache {
            debug_assert!(!self.in_cache);
            self.in_cache = true;
        }
    }

    /// `SkCachedData::inMutexUnref`
    fn in_mutex_unref(&mut self, from_cache: bool) {
        self.ref_cnt -= 1;
        match self.ref_cnt {
            0 => {
                // We are going to be deleted, so we need to be unlocked (for DiscardableMemory).
                if self.is_locked {
                    self.in_mutex_unlock();
                }
                // The deletion: the storage goes with the last reference.
                self.storage = None;
            }
            // If we're down to 1 owner, and that owner is the cache, it is safe to unlock.
            1 if self.in_cache && !from_cache => self.in_mutex_unlock(),
            _ => {}
        }
        if from_cache {
            debug_assert!(self.in_cache);
            self.in_cache = false;
        }
    }
}

/// Reference-counted data that can be unlocked when the cache is its only owner
/// (`SkCachedData`).
#[doc(alias = "SkCachedData")]
#[derive(Debug)]
pub struct CachedData {
    size: usize,
    state: Mutex<State>,
}

/// Borrows the data of a [`CachedData`] while it is locked. The borrow holds the object's lock, so
/// drop it before calling any other method on the same object.
#[derive(Debug)]
pub struct CachedDataGuard<'a> {
    state: MutexGuard<'a, State>,
}

impl Deref for CachedDataGuard<'_> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        // The guard exists only when there is data.
        self.state.bytes().unwrap_or(&[])
    }
}

impl DerefMut for CachedDataGuard<'_> {
    fn deref_mut(&mut self) -> &mut [u8] {
        self.state.bytes_mut().unwrap_or(&mut [])
    }
}

impl CachedData {
    /// Creates data backed by heap memory, with one reference (`SkCachedData(void*, size_t)`).
    #[doc(alias = "SkCachedData")]
    #[must_use]
    pub fn new_malloc(data: Vec<u8>) -> Arc<Self> {
        let size = data.len();
        Arc::new(Self::with_storage(
            size,
            Storage::Malloc(data.into_boxed_slice()),
            true,
        ))
    }

    /// Creates data backed by `dm`, which must already be locked, with one reference
    /// (`SkCachedData(size_t, SkDiscardableMemory*)`).
    #[doc(alias = "SkCachedData")]
    #[must_use]
    pub fn new_discardable(size: usize, dm: Box<dyn DiscardableMemory>) -> Arc<Self> {
        let has_data = dm.data().is_some();
        Arc::new(Self::with_storage(
            size,
            Storage::Discardable(dm),
            has_data,
        ))
    }

    fn with_storage(size: usize, storage: Storage, has_data: bool) -> Self {
        Self {
            size,
            state: Mutex::new(State {
                storage: Some(storage),
                has_data,
                ref_cnt: 1,
                in_cache: false,
                is_locked: true,
            }),
        }
    }

    /// Returns the size of the data in bytes (`size`).
    #[must_use]
    pub fn size(&self) -> usize {
        self.size
    }

    /// Borrows the data, or returns `None` if it is unlocked or the lock failed (`data`).
    #[must_use]
    pub fn data(&self) -> Option<CachedDataGuard<'_>> {
        let state = lock_unpoisoned(&self.state);
        state.has_data.then_some(CachedDataGuard { state })
    }

    /// Borrows the data for writing, or returns `None` if it is unlocked or the lock failed
    /// (`writable_data`).
    #[must_use]
    pub fn writable_data(&self) -> Option<CachedDataGuard<'_>> {
        self.data()
    }

    /// Adds a reference (`ref`).
    pub fn add_ref(&self) {
        lock_unpoisoned(&self.state).in_mutex_ref(false);
    }

    /// Drops a reference (`unref`).
    pub fn unref(&self) {
        lock_unpoisoned(&self.state).in_mutex_unref(false);
    }

    /// Called when the cache adds this data to a record: takes the cache's reference
    /// (`attachToCacheAndRef`).
    pub fn attach_to_cache_and_ref(&self) {
        lock_unpoisoned(&self.state).in_mutex_ref(true);
    }

    /// Called when the cache removes this data from a record: drops the cache's reference
    /// (`detachFromCacheAndUnref`).
    pub fn detach_from_cache_and_unref(&self) {
        lock_unpoisoned(&self.state).in_mutex_unref(true);
    }

    /// Returns the reference count (`testing_only_getRefCnt`).
    #[must_use]
    pub fn testing_only_get_ref_cnt(&self) -> i32 {
        lock_unpoisoned(&self.state).ref_cnt
    }

    /// Returns whether the data is locked (`testing_only_isLocked`).
    #[must_use]
    pub fn testing_only_is_locked(&self) -> bool {
        lock_unpoisoned(&self.state).is_locked
    }

    /// Returns whether the cache owns a reference (`testing_only_isInCache`).
    #[must_use]
    pub fn testing_only_is_in_cache(&self) -> bool {
        lock_unpoisoned(&self.state).in_cache
    }
}
