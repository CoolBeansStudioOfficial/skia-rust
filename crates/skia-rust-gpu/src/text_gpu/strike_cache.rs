// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/text/gpu/StrikeCache.h, src/text/gpu/StrikeCache.cpp

//! [`StrikeCache`]: the strikes of the GPU glyph cache of a recorder (one per strike spec), with
//! a count and a byte budget. Strikes are purged from the tail of the list, oldest first.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use skia_rust_core::descriptor::Descriptor;

use crate::graphite::text::text_strike::TextStrike;

/// `SK_DEFAULT_GPU_FONT_CACHE_COUNT_LIMIT`.
// Port of: src/text/gpu/StrikeCache.h#L25-L27 (chrome/m156)
pub const DEFAULT_GPU_FONT_CACHE_COUNT_LIMIT: i32 = 2048;

/// `SK_DEFAULT_GPU_FONT_CACHE_LIMIT`: 2 MiB.
// Port of: src/text/gpu/StrikeCache.h#L29-L31 (chrome/m156)
pub const DEFAULT_GPU_FONT_CACHE_LIMIT: usize = 2 * 1024 * 1024;

/// Manages strikes, which are indexed by a strike spec. These strikes can then be used to
/// generate individual glyph masks (`sktext::gpu::StrikeCache`).
///
/// skia-rust: the intrusive list of C++ is a `VecDeque` (head at the front) and the hash table
/// is a `HashMap` of descriptors. Strikes are shared with the glyph vectors that use them.
// Port of: src/text/gpu/StrikeCache.h#L63-L120 (chrome/m156)
#[doc(alias = "sktext::gpu::StrikeCache")]
#[derive(Debug)]
pub struct StrikeCache {
    /// `fHead`..`fTail`: most recently added first.
    list: VecDeque<Arc<TextStrike>>,
    /// `fCache`.
    cache: HashMap<Descriptor, Arc<TextStrike>>,
    /// `fCacheSizeLimit`.
    cache_size_limit: usize,
    /// `fTotalMemoryUsed`: shared with the strikes, which add to it as they make glyphs.
    total_memory_used: Arc<AtomicUsize>,
    /// `fCacheCountLimit`.
    cache_count_limit: i32,
    /// `fCacheCount`.
    cache_count: i32,
}

impl Default for StrikeCache {
    fn default() -> Self {
        Self {
            list: VecDeque::new(),
            cache: HashMap::new(),
            cache_size_limit: DEFAULT_GPU_FONT_CACHE_LIMIT,
            total_memory_used: Arc::new(AtomicUsize::new(0)),
            cache_count_limit: DEFAULT_GPU_FONT_CACHE_COUNT_LIMIT,
            cache_count: 0,
        }
    }
}

impl StrikeCache {
    /// An empty cache with the default limits.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The shared byte total, for the strikes to add to as they make glyphs.
    pub(crate) fn total_memory_used_handle(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.total_memory_used)
    }

    /// `fTotalMemoryUsed`.
    #[must_use]
    pub fn total_memory_used(&self) -> usize {
        self.total_memory_used.load(Ordering::Relaxed)
    }

    /// `fCacheCount`.
    #[must_use]
    pub fn cache_count(&self) -> i32 {
        self.cache_count
    }

    /// Sets the limits of the cache (the members are private in C++ and set by tests through
    /// friend access).
    pub fn set_limits(&mut self, size_limit: usize, count_limit: i32) {
        self.cache_size_limit = size_limit;
        self.cache_count_limit = count_limit;
    }

    /// `TextStrikeBase::Find(cache, desc)`.
    // Port of: src/text/gpu/StrikeCache.h#L122-L126 (chrome/m156)
    #[must_use]
    pub fn find(&self, desc: &Descriptor) -> Option<Arc<TextStrike>> {
        self.cache.get(desc).cloned()
    }

    /// `TextStrikeBase::Add(cache, strike)`.
    // Port of: src/text/gpu/StrikeCache.h#L128-L132 (chrome/m156)
    pub fn add(&mut self, strike: Arc<TextStrike>) {
        debug_assert!(self.find(strike.descriptor()).is_none());
        self.internal_attach_to_head(strike);
        self.internal_purge(0);
    }

    /// `freeAll()`.
    // Port of: src/text/gpu/StrikeCache.cpp#L36-L38 (chrome/m156)
    pub fn free_all(&mut self) {
        self.internal_purge(self.total_memory_used());
    }

    /// `internalPurge(minBytesNeeded)`: checks the budgets, modulated by the specified
    /// min-bytes-needed-to-purge, and attempts to purge strikes to match. Returns the number of
    /// bytes freed.
    // Port of: src/text/gpu/StrikeCache.cpp#L40-L97 (chrome/m156)
    fn internal_purge(&mut self, min_bytes_needed: usize) -> usize {
        let total = self.total_memory_used();
        let mut bytes_needed = 0;
        if total > self.cache_size_limit {
            bytes_needed = total - self.cache_size_limit;
        }
        bytes_needed = bytes_needed.max(min_bytes_needed);
        if bytes_needed != 0 {
            // no small purges!
            bytes_needed = bytes_needed.max(total >> 2);
        }

        let mut count_needed = 0;
        if self.cache_count > self.cache_count_limit {
            count_needed = self.cache_count - self.cache_count_limit;
            // no small purges!
            count_needed = count_needed.max(self.cache_count >> 2);
        }

        // early exit
        if count_needed == 0 && bytes_needed == 0 {
            return 0;
        }

        let mut bytes_freed = 0;
        let mut count_freed = 0;

        // Start at the tail and proceed backwards deleting; the list is in LRU order, with
        // unimportant entries at the tail.
        while (bytes_freed < bytes_needed || count_freed < count_needed) && !self.list.is_empty() {
            let last = self.list.len() - 1;
            bytes_freed += self.list[last].memory_used();
            count_freed += 1;
            self.internal_remove_strike(last);
        }

        self.validate();
        bytes_freed
    }

    /// `internalAttachToHead(strike)`.
    // Port of: src/text/gpu/StrikeCache.cpp#L99-L120 (chrome/m156)
    fn internal_attach_to_head(&mut self, strike: Arc<TextStrike>) {
        debug_assert!(!self.cache.contains_key(strike.descriptor()));
        self.cache
            .insert(strike.descriptor().clone(), Arc::clone(&strike));

        self.cache_count += 1;
        self.total_memory_used
            .fetch_add(strike.memory_used(), Ordering::Relaxed);
        self.list.push_front(strike);
    }

    /// `internalRemoveStrike(strike)`, of the strike at `index` in the list.
    // Port of: src/text/gpu/StrikeCache.cpp#L122-L141 (chrome/m156)
    fn internal_remove_strike(&mut self, index: usize) {
        debug_assert!(self.cache_count > 0);
        self.cache_count -= 1;
        let strike = self.list.remove(index).expect("index is in range");
        self.total_memory_used
            .fetch_sub(strike.memory_used(), Ordering::Relaxed);

        strike.set_removed();
        self.cache.remove(strike.descriptor());
    }

    /// `validate()`: a simple accounting of what each glyph cache reports and the strike cache
    /// total.
    // Port of: src/text/gpu/StrikeCache.cpp#L143-L170 (chrome/m156)
    fn validate(&self) {
        #[cfg(debug_assertions)]
        {
            let computed_bytes: usize = self.list.iter().map(|s| s.memory_used()).sum();
            let computed_count = i32::try_from(self.list.len()).expect("fits");
            assert_eq!(self.cache_count, computed_count);
            assert_eq!(self.total_memory_used(), computed_bytes);
            assert!(
                self.list
                    .iter()
                    .all(|s| self.cache.contains_key(s.descriptor()))
            );
        }
    }
}

impl Drop for StrikeCache {
    // Port of: src/text/gpu/StrikeCache.cpp#L32-L34 (chrome/m156)
    fn drop(&mut self) {
        self.free_all();
    }
}
