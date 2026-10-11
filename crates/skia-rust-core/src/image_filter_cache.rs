// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkImageFilterCache.h, src/core/SkImageFilterCache.cpp
//
// skia-rust: Skia's cache is process-wide and guarded by a mutex. A `FilterResult` holds color
// filters, which are not `Send`, and a backend lives on one thread (see `image_filter_types`), so
// the default cache here is one per thread, held by `Rc`. Filter results do not depend on the cache,
// so this only changes how much work is reused across threads.
//
// skia-rust: a filter is identified by its unique ID where Skia uses the `SkImageFilter*` pointer.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::image_filter_result::FilterResult;
use crate::matrix::Matrix;
use crate::rect::IRect;
use crate::special_image::SpecialImage;

/// `kDefaultCacheSize` (128 MiB; the iOS size is not used here).
// Port of: src/core/SkImageFilterCache.cpp#L19-L23 (chrome/m156)
const DEFAULT_CACHE_SIZE: usize = 128 * 1024 * 1024;

/// `SK_InvalidUniqueID`: the ID of a source with no image, which never matches a real image.
// Port of: include/core/SkTypes.h#L191 (chrome/m156)
pub const SK_INVALID_UNIQUE_ID: u32 = 0;

/// `SkImageFilterCacheKey`: the filter's unique ID, CTM, clip bounds, and the source image's
/// generation ID and subset. Two keys are equal only if all five are.
// Port of: src/core/SkImageFilterCache.h#L15-L49 (chrome/m156)
#[doc(alias = "SkImageFilterCacheKey")]
#[derive(Clone, Debug)]
pub struct ImageFilterCacheKey {
    unique_id: u32,
    matrix: Matrix,
    clip_bounds: IRect,
    src_gen_id: u32,
    src_subset: IRect,
}

impl ImageFilterCacheKey {
    /// `SkImageFilterCacheKey(uniqueID, matrix, clipBounds, srcGenID, srcSubset)`.
    // Port of: src/core/SkImageFilterCache.h#L17-L28 (chrome/m156)
    #[must_use]
    pub fn new(
        unique_id: u32,
        matrix: Matrix,
        clip_bounds: IRect,
        src_gen_id: u32,
        src_subset: IRect,
    ) -> ImageFilterCacheKey {
        debug_assert!(matrix.is_finite()); // otherwise we can't rely on == self when comparing keys
        ImageFilterCacheKey {
            unique_id,
            matrix,
            clip_bounds,
            src_gen_id,
            src_subset,
        }
    }
}

impl PartialEq for ImageFilterCacheKey {
    // Port of: src/core/SkImageFilterCache.h#L50-L56 (chrome/m156)
    fn eq(&self, other: &ImageFilterCacheKey) -> bool {
        self.unique_id == other.unique_id
            && self.matrix == other.matrix
            && self.clip_bounds == other.clip_bounds
            && self.src_gen_id == other.src_gen_id
            && self.src_subset == other.src_subset
    }
}

impl Eq for ImageFilterCacheKey {}

impl Hash for ImageFilterCacheKey {
    /// Consistent with `==`: the matrix entries are hashed by value, with `-0.0` as `0.0`.
    // Port of: src/core/SkImageFilterCache.cpp#L43-L45 (chrome/m156), the Hash32 of the key
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.unique_id.hash(state);
        let mut m = [0.0f32; 9];
        self.matrix.get_9(&mut m);
        for v in m {
            let v = if v == 0.0 { 0.0f32 } else { v };
            v.to_bits().hash(state);
        }
        self.clip_bounds.as_i32s().hash(state);
        self.src_gen_id.hash(state);
        self.src_subset.as_i32s().hash(state);
    }
}

/// One cached result (`CacheImpl::Value`).
#[derive(Debug)]
struct Value {
    image: FilterResult,
    /// The unique ID of the filter that produced it (`fFilter`).
    filter: u32,
    /// The bytes of the image, as counted against the budget (`fImage.image()->getSize()`).
    bytes: usize,
    /// The recency stamp: a larger stamp is nearer the LRU head (`fLRU`).
    stamp: u64,
}

/// The state of a cache (`CacheImpl`'s members, behind the `RefCell` of the handle).
#[derive(Debug, Default)]
struct State {
    entries: HashMap<ImageFilterCacheKey, Value>,
    /// Recency order: the first entry is the LRU tail, the last is the head.
    lru: BTreeMap<u64, ImageFilterCacheKey>,
    /// The keys of the results of each filter (`fImageFilterValues`).
    filter_values: HashMap<u32, Vec<ImageFilterCacheKey>>,
    max_bytes: usize,
    current_bytes: usize,
    next_stamp: u64,
}

impl State {
    fn stamp(&mut self) -> u64 {
        let stamp = self.next_stamp;
        self.next_stamp += 1;
        stamp
    }

    // Port of: src/core/SkImageFilterCache.cpp#L58-L72 (chrome/m156)
    fn get(&mut self, key: &ImageFilterCacheKey) -> Option<FilterResult> {
        let stamp = self.stamp();
        let v = self.entries.get_mut(key)?;
        let old = v.stamp;
        v.stamp = stamp;
        let image = v.image.clone();
        self.lru.remove(&old);
        self.lru.insert(stamp, key.clone());
        Some(image)
    }

    // Port of: src/core/SkImageFilterCache.cpp#L74-L97 (chrome/m156)
    fn set(&mut self, key: ImageFilterCacheKey, filter: u32, result: &FilterResult) {
        if self.entries.contains_key(&key) {
            self.remove_internal(&key, true);
        }
        let bytes = result.image().map_or(0, SpecialImage::get_size);
        let stamp = self.stamp();
        self.lru.insert(stamp, key.clone());
        self.current_bytes += bytes;
        self.filter_values
            .entry(filter)
            .or_default()
            .push(key.clone());
        self.entries.insert(
            key,
            Value {
                image: result.clone(),
                filter,
                bytes,
                stamp,
            },
        );

        while self.current_bytes > self.max_bytes {
            let Some((tail_stamp, tail)) = self.lru_tail() else {
                break;
            };
            if tail_stamp == stamp {
                break;
            }
            self.remove_internal(&tail, true);
        }
    }

    // Port of: src/core/SkImageFilterCache.cpp#L99-L107 (chrome/m156)
    fn purge(&mut self) {
        while self.current_bytes > 0 {
            let Some((_, tail)) = self.lru_tail() else {
                break;
            };
            self.remove_internal(&tail, true);
        }
    }

    /// The LRU tail: the stamp and key of the least recently used entry.
    fn lru_tail(&self) -> Option<(u64, ImageFilterCacheKey)> {
        self.lru
            .first_key_value()
            .map(|(stamp, key)| (*stamp, key.clone()))
    }

    // Port of: src/core/SkImageFilterCache.cpp#L109-L123 (chrome/m156)
    fn purge_by_image_filter(&mut self, filter: u32) {
        let Some(values) = self.filter_values.remove(&filter) else {
            return;
        };
        // The filter's list is already taken out, so `remove_internal` must not touch it.
        for key in values {
            self.remove_internal(&key, false);
        }
    }

    // Port of: src/core/SkImageFilterCache.cpp#L125-L147 (chrome/m156)
    fn remove_internal(&mut self, key: &ImageFilterCacheKey, update_filter_values: bool) {
        let Some(v) = self.entries.remove(key) else {
            return;
        };
        let values = if update_filter_values {
            self.filter_values.get_mut(&v.filter)
        } else {
            None
        };
        if let Some(values) = values {
            if values.len() == 1 && values[0] == *key {
                self.filter_values.remove(&v.filter);
            } else if let Some(pos) = values.iter().position(|k| k == key) {
                values.remove(pos);
            }
        }
        self.current_bytes -= v.bytes;
        self.lru.remove(&v.stamp);
    }
}

/// Whether [`ImageFilterCache::get_default`] creates the default cache if it doesn't exist yet
/// (`SkImageFilterCache::CreateIfNecessary`).
// Port of: src/core/SkImageFilterCache.h#L80-L81 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateIfNecessary {
    /// Return `None` if the cache doesn't exist.
    No,
    /// Create the cache if it doesn't exist.
    Yes,
}

/// The image filter cache (`SkImageFilterCache`): results of filters, keyed by [`ImageFilterCacheKey`],
/// evicted least-recently-used first once their images exceed the byte budget.
#[doc(alias = "SkImageFilterCache")]
pub struct ImageFilterCache {
    state: RefCell<State>,
}

thread_local! {
    /// `SkImageFilterCache::Get()`'s cache, one per thread (see the module docs).
    static DEFAULT_CACHE: RefCell<Option<Rc<ImageFilterCache>>> = const { RefCell::new(None) };
}

impl ImageFilterCache {
    /// A cache that keeps at most `max_bytes` of images (`SkImageFilterCache::Create`).
    // Port of: src/core/SkImageFilterCache.cpp#L155-L157 (chrome/m156)
    #[doc(alias = "Create")]
    #[must_use]
    pub fn create(max_bytes: usize) -> Rc<ImageFilterCache> {
        Rc::new(ImageFilterCache {
            state: RefCell::new(State {
                max_bytes,
                ..State::default()
            }),
        })
    }

    /// The default cache of this thread (`SkImageFilterCache::Get`). `None` only if it doesn't
    /// exist and `create` is [`CreateIfNecessary::No`], or if the thread is shutting down.
    // Port of: src/core/SkImageFilterCache.cpp#L159-L167 (chrome/m156)
    #[doc(alias = "Get")]
    #[must_use]
    pub fn get_default(create: CreateIfNecessary) -> Option<Rc<ImageFilterCache>> {
        DEFAULT_CACHE
            .try_with(|cache| {
                let mut cache = cache.borrow_mut();
                if cache.is_none() && create == CreateIfNecessary::Yes {
                    *cache = Some(ImageFilterCache::create(DEFAULT_CACHE_SIZE));
                }
                cache.clone()
            })
            .ok()
            .flatten()
    }

    /// The result cached for `key`, if any; a hit makes it the most recently used
    /// (`get(key, &result)`).
    // Port of: src/core/SkImageFilterCache.cpp#L58-L72 (chrome/m156)
    #[must_use]
    pub fn get(&self, key: &ImageFilterCacheKey) -> Option<FilterResult> {
        self.state.borrow_mut().get(key)
    }

    /// Caches `result` for `key`, made by the filter with unique ID `filter`
    /// (`set(key, filter, result)`).
    // Port of: src/core/SkImageFilterCache.cpp#L74-L97 (chrome/m156)
    pub fn set(&self, key: ImageFilterCacheKey, filter: u32, result: &FilterResult) {
        self.state.borrow_mut().set(key, filter, result);
    }

    /// Drops every cached result (`purge`).
    // Port of: src/core/SkImageFilterCache.cpp#L99-L107 (chrome/m156)
    pub fn purge(&self) {
        self.state.borrow_mut().purge();
    }

    /// Drops every result made by the filter with unique ID `filter`
    /// (`purgeByImageFilter`).
    // Port of: src/core/SkImageFilterCache.cpp#L109-L123 (chrome/m156)
    pub fn purge_by_image_filter(&self, filter: u32) {
        self.state.borrow_mut().purge_by_image_filter(filter);
    }

    /// The number of cached results (`count`, a debug-only query in Skia).
    // Port of: src/core/SkImageFilterCache.h#L77 (chrome/m156)
    #[must_use]
    pub fn count(&self) -> usize {
        self.state.borrow().entries.len()
    }
}

impl fmt::Debug for ImageFilterCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.borrow();
        f.debug_struct("ImageFilterCache")
            .field("count", &state.entries.len())
            .field("current_bytes", &state.current_bytes)
            .field("max_bytes", &state.max_bytes)
            .finish()
    }
}
