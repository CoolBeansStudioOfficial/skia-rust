// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkTypefaceCache.{h,cpp}, src/core/SkGraphics.cpp (typeface count limit)

//! The typeface cache (`SkTypefaceCache`) and the typeface id counter it shares with
//! [`Typeface`](crate::typeface::Typeface).

use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::sync::{LazyLock, Mutex};

use crate::typeface::{Typeface, TypefaceId};

static NEXT_TYPEFACE_ID: AtomicU32 = AtomicU32::new(1);

/// `SkTypefaceCache::NewTypefaceID`: the next unique id. Ids start at 1 and never repeat.
// Port of: src/core/SkTypefaceCache.cpp#L71-L74 (chrome/m156)
#[doc(alias = "SkTypefaceCache::NewTypefaceID")]
#[must_use]
pub fn new_typeface_id() -> TypefaceId {
    NEXT_TYPEFACE_ID.fetch_add(1, Ordering::Relaxed)
}

/// The largest number of typefaces the cache keeps before it purges (`SkGraphics`
/// `gTypefaceCacheCountLimit`, historical default 1024).
static TYPEFACE_CACHE_COUNT_LIMIT: AtomicI32 = AtomicI32::new(1024);

/// `SkGraphics::GetTypefaceCacheCountLimit`.
// Port of: src/core/SkGraphics.cpp#L104-L106 (chrome/m156)
#[doc(alias = "SkGraphics::GetTypefaceCacheCountLimit")]
#[must_use]
pub fn typeface_cache_count_limit() -> i32 {
    TYPEFACE_CACHE_COUNT_LIMIT.load(Ordering::Relaxed)
}

/// `SkGraphics::SetTypefaceCacheCountLimit`. Returns the previous limit.
// Port of: src/core/SkGraphics.cpp#L108-L112 (chrome/m156)
#[doc(alias = "SkGraphics::SetTypefaceCacheCountLimit")]
pub fn set_typeface_cache_count_limit(count: i32) -> i32 {
    TYPEFACE_CACHE_COUNT_LIMIT.swap(count, Ordering::Relaxed)
}

/// A list of typefaces that keeps those still in use alive, and drops the others when it needs
/// room (`SkTypefaceCache`).
// Port of: src/core/SkTypefaceCache.h#L17-L72 (chrome/m156)
#[doc(alias = "SkTypefaceCache")]
#[derive(Debug, Default)]
pub struct TypefaceCache {
    typefaces: Vec<Typeface>,
}

impl TypefaceCache {
    /// `SkTypefaceCache::SkTypefaceCache()`: an empty cache.
    // Port of: src/core/SkTypefaceCache.cpp#L21 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `SkTypefaceCache::add`: adds a typeface, purging a quarter of the limit first if the cache
    /// is full. A limit of zero or less means nothing is kept.
    // Port of: src/core/SkTypefaceCache.cpp#L23-L33 (chrome/m156)
    pub fn add(&mut self, face: Typeface) {
        let limit = typeface_cache_count_limit();
        if i64::try_from(self.typefaces.len()).unwrap_or(i64::MAX) >= i64::from(limit) {
            self.purge(limit >> 2);
        }
        if limit > 0 {
            self.typefaces.push(face);
        }
    }

    /// `SkTypefaceCache::findByProcAndRef`: the first typeface for which `proc` returns true. The
    /// typeface is cloned out, so the caller holds its own reference.
    // Port of: src/core/SkTypefaceCache.cpp#L35-L42 (chrome/m156)
    #[doc(alias = "findByProcAndRef")]
    pub fn find_by_proc_and_ref(
        &self,
        mut proc: impl FnMut(&Typeface) -> bool,
    ) -> Option<Typeface> {
        self.typefaces.iter().find(|face| proc(face)).cloned()
    }

    /// `SkTypefaceCache::purge`: drops up to `num_to_purge` typefaces that nothing else holds. The
    /// removal swaps in the last element, as `removeShuffle` does, so the order is not kept.
    // Port of: src/core/SkTypefaceCache.cpp#L44-L58 (chrome/m156)
    pub fn purge(&mut self, mut num_to_purge: i32) {
        let mut i = 0;
        while i < self.typefaces.len() {
            if self.typefaces[i].is_unique() {
                self.typefaces.swap_remove(i);
                num_to_purge -= 1;
                if num_to_purge == 0 {
                    return;
                }
            } else {
                i += 1;
            }
        }
    }

    /// `SkTypefaceCache::purgeAll`: purges every typeface nothing else holds.
    // Port of: src/core/SkTypefaceCache.cpp#L60-L63 (chrome/m156)
    #[doc(alias = "purgeAll")]
    pub fn purge_all(&mut self) {
        let count = i32::try_from(self.typefaces.len()).unwrap_or(i32::MAX);
        self.purge(count);
    }

    /// The process-wide cache (`SkTypefaceCache::Get`), behind a mutex as `SkTypefaceCache::Add`
    /// and its siblings take `typeface_cache_mutex()`.
    // Port of: src/core/SkTypefaceCache.cpp#L66-L69 (chrome/m156)
    #[doc(alias = "SkTypefaceCache::Get")]
    #[must_use]
    pub fn global() -> &'static Mutex<TypefaceCache> {
        static GLOBAL: LazyLock<Mutex<TypefaceCache>> =
            LazyLock::new(|| Mutex::new(TypefaceCache::new()));
        &GLOBAL
    }
}
