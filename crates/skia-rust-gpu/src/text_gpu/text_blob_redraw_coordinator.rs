// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/text/gpu/TextBlobRedrawCoordinator.h, src/text/gpu/TextBlobRedrawCoordinator.cpp

//! [`TextBlobRedrawCoordinator`]: reuses data from previous drawing operations using multiple
//! criteria to pick the best data for the draw. In addition, it provides a central service for
//! managing resource usage.
//!
//! The draw data is stored in a three-tiered system. The first tier is keyed by the `SkTextBlob`'s
//! unique ID. The second tier uses the [`TextBlob`]'s key to get a general match for the draw. The
//! last tier queries each sub run using `canReuse` to determine if each sub run can handle the
//! drawing parameters.
//!
//! skia-rust: the least-recently-used list is a `VecDeque` (head at the front), and the lock of
//! C++ (`SkSpinlock`) is a `Mutex`. The message bus that purges the blobs of a destroyed
//! `SkTextBlob` is not ported (`temporaryShuntBlobNotifyAddedToCache`): a cached blob is purged
//! when the budget needs it, or by [`TextBlobRedrawCoordinator::purge_blob`], which is what
//! polling a `PurgeBlobMessage` does. Unique IDs are never reused, so a stale blob is never
//! drawn for a different `SkTextBlob`.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use skia_rust_core::glyph_run::GlyphRunList;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;

use crate::text_gpu::sub_run_container::{StrikeDeviceInfo, SubRunTarget};
use crate::text_gpu::text_blob::{TextBlob, TextBlobKey};

/// `kDefaultBudget`: 4 MiB.
// Port of: src/text/gpu/TextBlobRedrawCoordinator.h#L106 (chrome/m156)
const DEFAULT_BUDGET: usize = 1 << 22;

/// The state that the lock guards.
#[derive(Debug)]
struct State {
    /// `fBlobList`: the blobs, most recently used first.
    blob_list: VecDeque<Arc<TextBlob>>,
    /// `fBlobIDCache` (`BlobIDCacheEntry`): the blobs of each `SkTextBlob` ID. Current clients
    /// don't generate multiple blobs per `SkTextBlob`, so a list with a linear search is
    /// acceptable. If usage changes, we should re-evaluate this structure.
    blob_id_cache: HashMap<u32, Vec<Arc<TextBlob>>>,
    /// `fSizeBudget`.
    size_budget: usize,
    /// `fCurrentSize`.
    current_size: usize,
}

impl State {
    /// `BlobIDCacheEntry::findBlobIndex(key)`.
    // Port of: src/text/gpu/TextBlobRedrawCoordinator.cpp#L242-L249 (chrome/m156)
    fn find_blob_index(&self, key: &TextBlobKey) -> Option<(u32, usize)> {
        let blobs = self.blob_id_cache.get(&key.unique_id)?;
        blobs
            .iter()
            .position(|blob| blob.key() == key)
            .map(|index| (key.unique_id, index))
    }

    /// `BlobIDCacheEntry::find(key)` through `find` of the cache.
    fn find_in_cache(&self, key: &TextBlobKey) -> Option<Arc<TextBlob>> {
        let (id, index) = self.find_blob_index(key)?;
        Some(Arc::clone(&self.blob_id_cache[&id][index]))
    }

    /// `internalRemove(blob)`.
    // Port of: src/text/gpu/TextBlobRedrawCoordinator.cpp#L133-L152 (chrome/m156)
    fn internal_remove(&mut self, blob: &Arc<TextBlob>) {
        let id = blob.key().unique_id;
        let Some(still_exists) = self.find_in_cache(blob.key()) else {
            return;
        };
        if Arc::ptr_eq(blob, &still_exists) {
            self.current_size -= blob.size();
            if let Some(pos) = self.blob_list.iter().position(|b| Arc::ptr_eq(b, blob)) {
                self.blob_list.remove(pos);
            }
            // `removeBlob`: `fBlobs.removeShuffle(index)` swaps the last blob in.
            if let Some(blobs) = self.blob_id_cache.get_mut(&id) {
                if let Some(index) = blobs.iter().position(|b| Arc::ptr_eq(b, blob)) {
                    blobs.swap_remove(index);
                }
                if blobs.is_empty() {
                    self.blob_id_cache.remove(&id);
                }
            }
        }
    }

    /// `internalCheckPurge(blob)`: if we are over budget, unref until we are below budget again,
    /// from the least recently used blob (but not `blob`).
    // Port of: src/text/gpu/TextBlobRedrawCoordinator.cpp#L194-L220 (chrome/m156)
    fn internal_check_purge(&mut self, blob: Option<&Arc<TextBlob>>) {
        if self.current_size > self.size_budget {
            while self.current_size > self.size_budget {
                let Some(lru_blob) = self.blob_list.back().cloned() else {
                    break;
                };
                if blob.is_some_and(|b| Arc::ptr_eq(b, &lru_blob)) {
                    break;
                }
                self.internal_remove(&lru_blob);
            }
        }
    }

    /// `internalAdd(blob)`: adds the blob, or returns the one already in the cache with its key.
    // Port of: src/text/gpu/TextBlobRedrawCoordinator.cpp#L222-L240 (chrome/m156)
    fn internal_add(&mut self, mut blob: Arc<TextBlob>) -> Arc<TextBlob> {
        let id = blob.key().unique_id;
        if let Some(already_in) = self.find_in_cache(blob.key()) {
            blob = already_in;
        } else {
            self.blob_list.push_front(Arc::clone(&blob));
            self.current_size += blob.size();
            self.blob_id_cache
                .entry(id)
                .or_default()
                .push(Arc::clone(&blob));
        }

        self.internal_check_purge(Some(&blob));
        blob
    }
}

/// Reuses the processed text of previous draws (`sktext::gpu::TextBlobRedrawCoordinator`).
// Port of: src/text/gpu/TextBlobRedrawCoordinator.h#L36-L120 (chrome/m156)
#[doc(alias = "sktext::gpu::TextBlobRedrawCoordinator")]
#[derive(Debug)]
pub struct TextBlobRedrawCoordinator {
    /// `fSpinLock` and the members it guards.
    state: Mutex<State>,
    /// `fMessageBusID`: in practice always the unique ID of the owning context.
    message_bus_id: u32,
}

impl TextBlobRedrawCoordinator {
    /// `TextBlobRedrawCoordinator(messageBusID)`.
    // Port of: src/text/gpu/TextBlobRedrawCoordinator.cpp#L40-L43 (chrome/m156)
    #[must_use]
    pub fn new(message_bus_id: u32) -> Self {
        Self {
            state: Mutex::new(State {
                blob_list: VecDeque::new(),
                blob_id_cache: HashMap::new(),
                size_budget: DEFAULT_BUDGET,
                current_size: 0,
            }),
            message_bus_id,
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `fMessageBusID`.
    #[must_use]
    pub fn message_bus_id(&self) -> u32 {
        self.message_bus_id
    }

    /// `drawGlyphRunList(canvas, viewMatrix, glyphRunList, paint, strikeDeviceInfo,
    /// atlasDelegate)`.
    // Port of: src/text/gpu/TextBlobRedrawCoordinator.cpp#L45-L56 (chrome/m156)
    pub fn draw_glyph_run_list(
        &self,
        target: &mut dyn SubRunTarget,
        view_matrix: &Matrix,
        glyph_run_list: &GlyphRunList<'_>,
        paint: &Paint,
        strike_device_info: &StrikeDeviceInfo,
    ) {
        let blob = self.find_or_create_blob(view_matrix, glyph_run_list, paint, strike_device_info);

        blob.draw(target, glyph_run_list.origin(), paint);
    }

    /// `findOrCreateBlob(viewMatrix, glyphRunList, paint, strikeDeviceInfo)`.
    // Port of: src/text/gpu/TextBlobRedrawCoordinator.cpp#L58-L91 (chrome/m156)
    #[must_use]
    #[allow(clippy::missing_panics_doc)] // lock poisoning only
    pub fn find_or_create_blob(
        &self,
        view_matrix: &Matrix,
        glyph_run_list: &GlyphRunList<'_>,
        paint: &Paint,
        strike_device_info: &StrikeDeviceInfo,
    ) -> Arc<TextBlob> {
        let mut position_matrix = view_matrix.clone();
        position_matrix.pre_translate(glyph_run_list.origin());

        let (can_cache, key) =
            TextBlobKey::make(glyph_run_list, paint, &position_matrix, strike_device_info);
        let mut blob = if can_cache { self.find(&key) } else { None };

        if blob
            .as_ref()
            .is_none_or(|blob| !blob.can_reuse(paint, &position_matrix))
        {
            if let Some(blob) = &blob {
                // We have to remake the blob because changes may invalidate our masks.
                self.remove(blob);
            }

            let mut new_blob =
                TextBlob::make(glyph_run_list, paint, &position_matrix, strike_device_info);

            if can_cache {
                new_blob.add_key(&key);
                // The blob may already have been created on a different thread. Use the first
                // one that was there.
                blob = Some(self.add_or_return_existing(Arc::new(new_blob)));
            } else {
                blob = Some(Arc::new(new_blob));
            }
        }

        blob.expect("a blob was found or made")
    }

    /// `addOrReturnExisting(glyphRunList, blob)`: if not already in the cache, then add it else,
    /// return the text blob from the cache.
    // Port of: src/text/gpu/TextBlobRedrawCoordinator.cpp#L100-L107 (chrome/m156)
    fn add_or_return_existing(&self, blob: Arc<TextBlob>) -> Arc<TextBlob> {
        self.lock().internal_add(blob)
    }

    /// `find(key)`: finds the blob and moves it to the head of the list.
    // Port of: src/text/gpu/TextBlobRedrawCoordinator.cpp#L109-L124 (chrome/m156)
    #[must_use]
    #[allow(clippy::missing_panics_doc)] // lock poisoning only
    pub fn find(&self, key: &TextBlobKey) -> Option<Arc<TextBlob>> {
        let mut state = self.lock();
        let blob = state.find_in_cache(key)?;
        if state
            .blob_list
            .front()
            .is_none_or(|head| !Arc::ptr_eq(head, &blob))
        {
            #[allow(clippy::collapsible_if)] // keeps the C++ nesting
            if let Some(pos) = state.blob_list.iter().position(|b| Arc::ptr_eq(b, &blob)) {
                let moved = state.blob_list.remove(pos).expect("position is in range");
                state.blob_list.push_front(moved);
            }
        }
        Some(blob)
    }

    /// `remove(blob)`.
    // Port of: src/text/gpu/TextBlobRedrawCoordinator.cpp#L126-L131 (chrome/m156)
    pub fn remove(&self, blob: &Arc<TextBlob>) {
        self.lock().internal_remove(blob);
    }

    /// `freeAll()`.
    // Port of: src/text/gpu/TextBlobRedrawCoordinator.cpp#L154-L160 (chrome/m156)
    pub fn free_all(&self) {
        let mut state = self.lock();
        state.blob_id_cache.clear();
        state.blob_list.clear();
        state.current_size = 0;
    }

    /// What polling a `PurgeBlobMessage` for `blob_id` does (`internalPurgeStaleBlobs`): removes
    /// all the blobs of the `SkTextBlob` from the cache.
    // Port of: src/text/gpu/TextBlobRedrawCoordinator.cpp#L167-L188 (chrome/m156)
    pub fn purge_blob(&self, blob_id: u32) {
        let mut state = self.lock();
        let Some(blobs) = state.blob_id_cache.remove(&blob_id) else {
            // no cache entries for id
            return;
        };
        // remove all blob entries from the LRU list
        for blob in &blobs {
            state.current_size -= blob.size();
            if let Some(pos) = state.blob_list.iter().position(|b| Arc::ptr_eq(b, blob)) {
                state.blob_list.remove(pos);
            }
        }
    }

    /// `purgeStaleBlobs()`: with no message bus, nothing is posted and nothing is purged.
    // Port of: src/text/gpu/TextBlobRedrawCoordinator.cpp#L162-L165 (chrome/m156)
    pub fn purge_stale_blobs(&self) {}

    /// `usedBytes()`.
    // Port of: src/text/gpu/TextBlobRedrawCoordinator.cpp#L190-L193 (chrome/m156)
    #[must_use]
    pub fn used_bytes(&self) -> usize {
        self.lock().current_size
    }

    /// `isOverBudget()`.
    // Port of: src/text/gpu/TextBlobRedrawCoordinator.cpp#L195-L198 (chrome/m156)
    #[must_use]
    pub fn is_over_budget(&self) -> bool {
        let state = self.lock();
        state.current_size > state.size_budget
    }

    /// Sets the size budget (`fSizeBudget` is set by the testing peer).
    pub fn set_size_budget(&self, size_budget: usize) {
        self.lock().size_budget = size_budget;
    }

    /// The number of blobs in the cache.
    #[must_use]
    pub fn blob_count(&self) -> usize {
        self.lock().blob_list.len()
    }
}
