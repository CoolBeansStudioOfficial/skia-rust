// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkStrikeCache.{h,cpp}

//! `SkStrikeCache`: the cache of strikes, with a least-recently-used list, a count and a byte
//! budget, and the purge that enforces them.
//!
//! The cache is one `Mutex` over its list, lookup table and totals, as in C++. Strikes hold only a
//! [`Weak`] reference back to that state, so dropping the cache never leaks it through its own
//! strikes.
//!
//! Not ported: `Dump`, `DumpMemoryStatistics` and `forEachStrike` (trace output), and the
//! `SkStrikeCache` side of remote strikes (`mergeFromBuffer`, T23).

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError};

use crate::descriptor::Descriptor;
use crate::font_metrics::FontMetrics;
use crate::strike::{Strike, StrikePinner};
use crate::strike_spec::StrikeSpec;

/// `SK_DEFAULT_FONT_CACHE_COUNT_LIMIT`.
// Port of: src/core/SkStrikeCache.h#L31-L32 (chrome/m156)
pub const DEFAULT_FONT_CACHE_COUNT_LIMIT: i32 = 2048;

/// `SK_DEFAULT_FONT_CACHE_LIMIT`: 2 MiB.
// Port of: src/core/SkStrikeCache.h#L33-L34 (chrome/m156)
pub const DEFAULT_FONT_CACHE_LIMIT: usize = 2 * 1024 * 1024;

/// Locks a mutex, ignoring poisoning: the guarded state is only ever left consistent between
/// operations, so a panic in another thread does not make it unusable.
pub(crate) fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The state that the cache lock guards (`SkStrikeCache`'s `fLock`-protected members).
#[derive(Debug)]
pub(crate) struct CacheState {
    /// The strikes, most recently used first (`fHead`..`fTail`).
    pub(crate) list: VecDeque<Arc<Strike>>,
    /// The strikes by descriptor (`fStrikeLookup`).
    lookup: HashMap<Descriptor, Arc<Strike>>,
    /// `fTotalMemoryUsed`.
    pub(crate) total_memory_used: usize,
    /// `fCacheSizeLimit`.
    cache_size_limit: usize,
    /// `fCacheCountLimit`.
    cache_count_limit: i32,
    /// `fCacheCount`.
    cache_count: i32,
    /// `fPinnerCount`.
    pinner_count: i32,
}

impl CacheState {
    fn new() -> Self {
        Self {
            list: VecDeque::new(),
            lookup: HashMap::new(),
            total_memory_used: 0,
            cache_size_limit: DEFAULT_FONT_CACHE_LIMIT,
            cache_count_limit: DEFAULT_FONT_CACHE_COUNT_LIMIT,
            cache_count: 0,
            pinner_count: 0,
        }
    }

    /// `SkStrikeCache::internalFindStrikeOrNull`: finds a strike by descriptor and moves it to
    /// the head of the list.
    // Port of: src/core/SkStrikeCache.cpp#L109-L133 (chrome/m156)
    fn internal_find_strike_or_null(&mut self, desc: &Descriptor) -> Option<Arc<Strike>> {
        if let Some(head) = self.list.front().filter(|head| head.descriptor() == desc) {
            return Some(Arc::clone(head));
        }
        let strike = Arc::clone(self.lookup.get(desc)?);
        if let Some(pos) = self
            .list
            .iter()
            .position(|s| Arc::ptr_eq(s, &strike))
            .filter(|&pos| pos != 0)
        {
            let moved = self.list.remove(pos).expect("position is in range");
            self.list.push_front(moved);
        }
        Some(strike)
    }

    /// `SkStrikeCache::internalAttachToHead`: adds a new strike at the head of the list.
    // Port of: src/core/SkStrikeCache.cpp#L277-L298 (chrome/m156)
    fn internal_attach_to_head(&mut self, strike: Arc<Strike>) {
        debug_assert!(!self.lookup.contains_key(strike.descriptor()));
        self.lookup
            .insert(strike.descriptor().clone(), Arc::clone(&strike));
        self.cache_count += 1;
        self.pinner_count += i32::from(strike.has_pinner());
        self.total_memory_used += strike.memory_used();
        self.list.push_front(strike);
    }

    /// `SkStrikeCache::internalRemoveStrike`: removes a strike from the list and the lookup.
    // Port of: src/core/SkStrikeCache.cpp#L299-L320 (chrome/m156)
    fn internal_remove_strike_at(&mut self, pos: usize) {
        let strike = self.list.remove(pos).expect("position is in range");
        debug_assert!(self.cache_count > 0);
        self.cache_count -= 1;
        self.pinner_count -= i32::from(strike.has_pinner());
        self.total_memory_used -= strike.memory_used();
        self.lookup.remove(strike.descriptor());
        strike.set_removed();
    }

    /// `SkStrikeCache::internalPurge`: removes strikes from the tail until the byte and count
    /// budgets are met, skipping pinned strikes that refuse to be deleted. Returns the bytes freed.
    ///
    /// C++ takes a `checkPinners` flag, but `SK_STRIKE_CACHE_DOESNT_AUTO_CHECK_PINNERS` is not
    /// defined, so it always checks pinners. That is the only behavior ported here.
    // Port of: src/core/SkStrikeCache.cpp#L216-L275 (chrome/m156)
    fn internal_purge(&mut self, min_bytes_needed: usize) -> usize {
        let mut bytes_needed = 0;
        if self.total_memory_used > self.cache_size_limit {
            bytes_needed = self.total_memory_used - self.cache_size_limit;
        }
        bytes_needed = bytes_needed.max(min_bytes_needed);
        if bytes_needed != 0 {
            bytes_needed = bytes_needed.max(self.total_memory_used >> 2);
        }

        let mut count_needed = 0;
        if self.cache_count > self.cache_count_limit {
            count_needed = self.cache_count - self.cache_count_limit;
            count_needed = count_needed.max(self.cache_count >> 2);
        }
        if count_needed == 0 && bytes_needed == 0 {
            return 0;
        }

        let mut bytes_freed = 0;
        let mut count_freed = 0;
        // Walk from the tail (least recently used) toward the head.
        let mut pos = self.list.len();
        while pos > 0 && (bytes_freed < bytes_needed || count_freed < count_needed) {
            pos -= 1;
            let strike = &self.list[pos];
            if !strike.has_pinner() || strike.can_delete() {
                bytes_freed += strike.memory_used();
                count_freed += 1;
                self.internal_remove_strike_at(pos);
            }
        }
        self.validate();
        bytes_freed
    }

    /// `SkStrikeCache::validate` (debug builds only in C++).
    // Port of: src/core/SkStrikeCache.cpp#L321-L344 (chrome/m156)
    fn validate(&self) {
        if cfg!(debug_assertions) {
            let computed_bytes: usize = self.list.iter().map(|s| s.memory_used()).sum();
            assert_eq!(
                self.cache_count,
                i32::try_from(self.list.len()).expect("cache count fits in i32"),
                "fCacheCount != computedCount"
            );
            assert_eq!(
                self.total_memory_used, computed_bytes,
                "fTotalMemoryUsed == computedBytes"
            );
        }
    }
}

/// A cache of strikes (`SkStrikeCache`).
///
/// Most code uses [`StrikeCache::global`]. A local cache is for tests that need their own
/// budget, as in C++.
// Port of: src/core/SkStrikeCache.h#L36-L117 (chrome/m156)
#[doc(alias = "SkStrikeCache")]
#[derive(Debug)]
pub struct StrikeCache {
    state: Arc<Mutex<CacheState>>,
}

impl Default for StrikeCache {
    fn default() -> Self {
        Self::new()
    }
}

impl StrikeCache {
    /// An empty cache with the default budget (`SkStrikeCache()`).
    // Port of: src/core/SkStrikeCache.h#L77-L80 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(CacheState::new())),
        }
    }

    /// The process-wide cache (`SkStrikeCache::GlobalStrikeCache`). Its contents never change a
    /// result, only which strikes are kept, so it is the one sanctioned process-wide cache.
    // Port of: src/core/SkStrikeCache.cpp#L35-L48 (chrome/m156), the non-thread-local branch
    #[doc(alias = "GlobalStrikeCache")]
    #[must_use]
    pub fn global() -> &'static StrikeCache {
        static GLOBAL: LazyLock<StrikeCache> = LazyLock::new(StrikeCache::new);
        &GLOBAL
    }

    /// `SkStrikeCache::findStrike`: the strike for `desc`, if it is cached.
    // Port of: src/core/SkStrikeCache.cpp#L102-L107 (chrome/m156)
    #[must_use]
    pub fn find_strike(&self, desc: &Descriptor) -> Option<Arc<Strike>> {
        let mut state = lock_unpoisoned(&self.state);
        let result = state.internal_find_strike_or_null(desc);
        state.internal_purge(0);
        result
    }

    /// `SkStrikeCache::createStrike`: makes a strike and caches it, even if one with the same
    /// descriptor exists (that one is then unreachable by lookup).
    // Port of: src/core/SkStrikeCache.cpp#L135-L141 (chrome/m156)
    #[must_use]
    pub fn create_strike(
        &self,
        spec: &StrikeSpec,
        maybe_metrics: Option<FontMetrics>,
        pinner: Option<Box<dyn StrikePinner>>,
    ) -> Arc<Strike> {
        let mut state = lock_unpoisoned(&self.state);
        Self::internal_create_strike(&mut state, &self.state, spec, maybe_metrics, pinner)
    }

    /// `SkStrikeCache::findOrCreateStrike`: the cached strike for `spec`, or a new one.
    // Port of: src/core/SkStrikeCache.cpp#L49-L57 (chrome/m156)
    #[must_use]
    pub fn find_or_create_strike(&self, spec: &StrikeSpec) -> Arc<Strike> {
        let mut state = lock_unpoisoned(&self.state);
        let strike = match state.internal_find_strike_or_null(spec.descriptor()) {
            Some(strike) => strike,
            None => Self::internal_create_strike(&mut state, &self.state, spec, None, None),
        };
        state.internal_purge(0);
        strike
    }

    /// `SkStrikeCache::internalCreateStrike`: the scaler context, the strike, and its place at
    /// the head of the list.
    // Port of: src/core/SkStrikeCache.cpp#L143-L152 (chrome/m156)
    fn internal_create_strike(
        state: &mut CacheState,
        state_arc: &Arc<Mutex<CacheState>>,
        spec: &StrikeSpec,
        maybe_metrics: Option<FontMetrics>,
        pinner: Option<Box<dyn StrikePinner>>,
    ) -> Arc<Strike> {
        let scaler = spec.create_scaler_context();
        let strike = Arc::new(Strike::new(
            Arc::downgrade(state_arc),
            spec,
            scaler,
            maybe_metrics,
            pinner,
        ));
        state.internal_attach_to_head(Arc::clone(&strike));
        strike
    }

    /// `SkStrikeCache::purgeAll`: purges every strike that can be deleted. Does not change the
    /// budget.
    // Port of: src/core/SkStrikeCache.cpp#L159-L162 (chrome/m156)
    pub fn purge_all(&self) {
        let mut state = lock_unpoisoned(&self.state);
        let total = state.total_memory_used;
        state.internal_purge(total);
    }

    /// `SkStrikeCache::purgePinned`: purges pinned strikes that their pinners allow to go.
    // Port of: src/core/SkStrikeCache.cpp#L154-L157 (chrome/m156)
    pub fn purge_pinned(&self, min_bytes_needed: usize) {
        let mut state = lock_unpoisoned(&self.state);
        state.internal_purge(min_bytes_needed);
    }

    /// `SkStrikeCache::getCacheCountLimit`.
    // Port of: src/core/SkStrikeCache.cpp#L174-L177 (chrome/m156)
    #[must_use]
    pub fn cache_count_limit(&self) -> i32 {
        lock_unpoisoned(&self.state).cache_count_limit
    }

    /// `SkStrikeCache::setCacheCountLimit`: returns the previous limit. A negative limit is 0.
    // Port of: src/core/SkStrikeCache.cpp#L193-L204 (chrome/m156)
    #[must_use]
    pub fn set_cache_count_limit(&self, new_count: i32) -> i32 {
        let new_count = new_count.max(0);
        let mut state = lock_unpoisoned(&self.state);
        let prev_count = state.cache_count_limit;
        state.cache_count_limit = new_count;
        state.internal_purge(0);
        prev_count
    }

    /// `SkStrikeCache::getCacheCountUsed`.
    // Port of: src/core/SkStrikeCache.cpp#L169-L172 (chrome/m156)
    #[must_use]
    pub fn cache_count_used(&self) -> i32 {
        lock_unpoisoned(&self.state).cache_count
    }

    /// `SkStrikeCache::getCacheSizeLimit`.
    // Port of: src/core/SkStrikeCache.cpp#L188-L191 (chrome/m156)
    #[must_use]
    pub fn cache_size_limit(&self) -> usize {
        lock_unpoisoned(&self.state).cache_size_limit
    }

    /// `SkStrikeCache::setCacheSizeLimit`: returns the previous limit.
    // Port of: src/core/SkStrikeCache.cpp#L179-L186 (chrome/m156)
    #[must_use]
    pub fn set_cache_size_limit(&self, new_limit: usize) -> usize {
        let mut state = lock_unpoisoned(&self.state);
        let prev_limit = state.cache_size_limit;
        state.cache_size_limit = new_limit;
        state.internal_purge(0);
        prev_limit
    }

    /// `SkStrikeCache::getTotalMemoryUsed`.
    // Port of: src/core/SkStrikeCache.cpp#L164-L167 (chrome/m156)
    #[must_use]
    pub fn total_memory_used(&self) -> usize {
        lock_unpoisoned(&self.state).total_memory_used
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::Font;
    use crate::paint::Paint;
    use crate::surface_props::SurfaceProps;
    use crate::typeface::Typeface;

    /// The cache mechanics of `SkStrikeCache_CachePurge`, with the empty typeface (the test
    /// proper uses the portable typeface, which arrives with T10).
    #[test]
    fn purge_and_limits_follow_the_budget() {
        let cache = StrikeCache::new();
        let font = Font::from_typeface(Some(Typeface::empty()));
        let spec = StrikeSpec::make_mask(
            &font,
            &Paint::default(),
            &SurfaceProps::default(),
            crate::scaler_context::ScalerContextBuildFlags::NONE,
            crate::matrix::Matrix::i(),
        )
        .expect("a paint without effects has a strike spec");

        assert_eq!(cache.total_memory_used(), 0);
        drop(cache.find_or_create_strike(&spec));
        assert!(cache.total_memory_used() > 0);

        cache.purge_all();
        assert_eq!(cache.total_memory_used(), 0);

        // The smallest cache keeps nothing that it cannot hold.
        let _ = cache.set_cache_size_limit(0);
        drop(cache.find_or_create_strike(&spec));
        assert_eq!(cache.total_memory_used(), 0);
    }

    #[test]
    fn count_limit_is_clamped_and_returns_the_previous_limit() {
        let cache = StrikeCache::new();
        assert_eq!(cache.cache_count_limit(), DEFAULT_FONT_CACHE_COUNT_LIMIT);
        assert_eq!(
            cache.set_cache_count_limit(-5),
            DEFAULT_FONT_CACHE_COUNT_LIMIT
        );
        assert_eq!(cache.cache_count_limit(), 0);
        assert_eq!(cache.set_cache_size_limit(7), DEFAULT_FONT_CACHE_LIMIT);
        assert_eq!(cache.cache_size_limit(), 7);
    }
}
