// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkGraphics.h, src/core/SkGraphics.cpp (the font cache parts)

//! The font cache controls of `SkGraphics`: the budget and count of the global strike cache, and
//! purging it. The other `SkGraphics` functions (resource cache, decoders, ...) are not ported.

use crate::strike_cache::{StrikeCache, lock_unpoisoned};
use crate::typeface_cache::TypefaceCache;

/// `SkGraphics::GetFontCacheLimit`: the byte budget of the global strike cache.
// Port of: src/core/SkGraphics.cpp#L48-L50 (chrome/m156)
#[doc(alias = "GetFontCacheLimit")]
#[must_use]
pub fn font_cache_limit() -> usize {
    StrikeCache::global().cache_size_limit()
}

/// `SkGraphics::SetFontCacheLimit`: sets the byte budget and returns the previous one. Only
/// purging depends on it; results do not.
// Port of: src/core/SkGraphics.cpp#L52-L54 (chrome/m156)
#[doc(alias = "SetFontCacheLimit")]
#[must_use]
pub fn set_font_cache_limit(bytes: usize) -> usize {
    StrikeCache::global().set_cache_size_limit(bytes)
}

/// `SkGraphics::GetFontCacheUsed`: the bytes the global strike cache accounts for.
// Port of: src/core/SkGraphics.cpp#L56-L58 (chrome/m156)
#[doc(alias = "GetFontCacheUsed")]
#[must_use]
pub fn font_cache_used() -> usize {
    StrikeCache::global().total_memory_used()
}

/// `SkGraphics::GetFontCacheCountLimit`: the maximum number of strikes kept.
// Port of: src/core/SkGraphics.cpp#L60-L62 (chrome/m156)
#[doc(alias = "GetFontCacheCountLimit")]
#[must_use]
pub fn font_cache_count_limit() -> i32 {
    StrikeCache::global().cache_count_limit()
}

/// `SkGraphics::SetFontCacheCountLimit`: sets the maximum number of strikes and returns the
/// previous limit. A negative count is 0.
// Port of: src/core/SkGraphics.cpp#L64-L66 (chrome/m156)
#[doc(alias = "SetFontCacheCountLimit")]
#[must_use]
pub fn set_font_cache_count_limit(count: i32) -> i32 {
    StrikeCache::global().set_cache_count_limit(count)
}

/// `SkGraphics::GetFontCacheCountUsed`: the number of strikes in the global cache.
// Port of: src/core/SkGraphics.cpp#L68-L70 (chrome/m156)
#[doc(alias = "GetFontCacheCountUsed")]
#[must_use]
pub fn font_cache_count_used() -> i32 {
    StrikeCache::global().cache_count_used()
}

/// `SkGraphics::PurgeFontCache`: purges every strike that can be deleted, then the typeface
/// cache. The budgets are unchanged.
// Port of: src/core/SkGraphics.cpp#L72-L75 (chrome/m156)
#[doc(alias = "PurgeFontCache")]
pub fn purge_font_cache() {
    StrikeCache::global().purge_all();
    lock_unpoisoned(TypefaceCache::global()).purge_all();
}

/// `SkGraphics::PurgePinnedFontCache`: purges the pinned strikes that their pinners allow to go.
// Port of: src/core/SkGraphics.cpp#L77-L79 (chrome/m156)
#[doc(alias = "PurgePinnedFontCache")]
pub fn purge_pinned_font_cache() {
    StrikeCache::global().purge_pinned(0);
}

/// `SkGraphics::GetTypefaceCacheCountLimit`: the maximum number of typefaces kept.
// Port of: src/core/SkGraphics.cpp#L104-L106 (chrome/m156)
#[doc(alias = "GetTypefaceCacheCountLimit")]
#[must_use]
pub fn typeface_cache_count_limit() -> i32 {
    crate::typeface_cache::typeface_cache_count_limit()
}

/// `SkGraphics::SetTypefaceCacheCountLimit`: sets the maximum number of typefaces and returns
/// the previous limit.
// Port of: src/core/SkGraphics.cpp#L108-L112 (chrome/m156)
#[doc(alias = "SetTypefaceCacheCountLimit")]
#[must_use]
pub fn set_typeface_cache_count_limit(count: i32) -> i32 {
    crate::typeface_cache::set_typeface_cache_count_limit(count)
}
