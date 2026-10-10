// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/text/TextStrike.h, src/gpu/graphite/text/TextStrike.cpp,
// src/text/gpu/StrikeCache.h (TextStrikeBase), src/gpu/graphite/text/GlyphData.h (GlyphEntry)
// (chrome/m156)

//! [`TextStrike`]: the glyph entries (where each glyph is in the atlas) of one strike, kept in the
//! [`StrikeCache`](crate::text_gpu::strike_cache::StrikeCache) of the recorder.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use skia_rust_core::descriptor::Descriptor;
use skia_rust_core::strike_spec::StrikeSpec;

use crate::graphite::draw_atlas::AtlasLocator;
use crate::text_gpu::packed_gpu_glyph_id::PackedGpuGlyphId;
use crate::text_gpu::strike_cache::StrikeCache;

/// Where one glyph is in the atlas (`skgpu::graphite::GlyphEntry`): the key says how its mask is
/// used, and the locator is filled when the glyph is added to the atlas.
// Port of: src/gpu/graphite/text/GlyphData.h#L32-L37 (chrome/m156)
#[doc(alias = "skgpu::graphite::GlyphEntry")]
#[derive(Debug)]
pub struct GlyphEntry {
    /// `fKey`.
    key: PackedGpuGlyphId,
    /// `fAtlasLocator`. The entry is shared (by the strike and by the glyph vectors of sub runs)
    /// and filled in when the glyph is added to the atlas.
    atlas_locator: Mutex<AtlasLocator>,
}

impl GlyphEntry {
    /// `GlyphEntry(key)`.
    #[must_use]
    pub fn new(key: PackedGpuGlyphId) -> Self {
        Self {
            key,
            atlas_locator: Mutex::new(AtlasLocator::default()),
        }
    }

    /// `fKey`.
    #[must_use]
    pub fn key(&self) -> PackedGpuGlyphId {
        self.key
    }

    /// `fAtlasLocator` (a copy).
    #[must_use]
    pub fn atlas_locator(&self) -> AtlasLocator {
        *self
            .atlas_locator
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Sets `fAtlasLocator`.
    pub fn set_atlas_locator(&self, locator: AtlasLocator) {
        *self
            .atlas_locator
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = locator;
    }
}

/// `sizeof(TextStrikeBase)`: the memory a strike starts with. (Accounting only decides when the
/// cache purges; the value is not the C++ one.)
const SIZEOF_TEXT_STRIKE: usize = std::mem::size_of::<TextStrike>();

/// The strike of the GPU glyph cache (`sktext::gpu::TextStrikeBase` and
/// `skgpu::graphite::TextStrike`): the entries of the glyphs of one strike spec.
// Port of: src/gpu/graphite/text/TextStrike.h#L24-L45 (chrome/m156)
#[doc(alias = "skgpu::graphite::TextStrike")]
#[derive(Debug)]
pub struct TextStrike {
    /// `fStrikeSpec`.
    strike_spec: StrikeSpec,
    /// `fCache`: the entries of the glyphs, by key.
    cache: Mutex<HashMap<PackedGpuGlyphId, Arc<GlyphEntry>>>,
    /// `fMemoryUsed`.
    memory_used: AtomicUsize,
    /// `fRemoved`.
    removed: AtomicBool,
    /// `fStrikeCache->fTotalMemoryUsed`, shared with the cache this strike is in.
    cache_total_memory_used: Arc<AtomicUsize>,
}

impl TextStrike {
    /// `TextStrike(strikeCache, strikeSpec)`.
    // Port of: src/gpu/graphite/text/TextStrike.cpp#L17-L18 (chrome/m156)
    #[must_use]
    pub fn new(strike_cache: &StrikeCache, strike_spec: &StrikeSpec) -> Self {
        Self {
            strike_spec: strike_spec.clone(),
            cache: Mutex::new(HashMap::new()),
            memory_used: AtomicUsize::new(SIZEOF_TEXT_STRIKE),
            removed: AtomicBool::new(false),
            cache_total_memory_used: strike_cache.total_memory_used_handle(),
        }
    }

    /// `GetOrCreate(strikeCache, strikeSpec)`: finds or creates the `TextStrike` for the given
    /// strike spec.
    // Port of: src/gpu/graphite/text/TextStrike.cpp#L20-L30 (chrome/m156)
    #[must_use]
    pub fn get_or_create(strike_cache: &mut StrikeCache, strike_spec: &StrikeSpec) -> Arc<Self> {
        if let Some(existing_strike) = strike_cache.find(strike_spec.descriptor()) {
            return existing_strike;
        }

        let new_strike = Arc::new(TextStrike::new(strike_cache, strike_spec));
        strike_cache.add(Arc::clone(&new_strike));
        new_strike
    }

    /// `strikeSpec()`.
    #[must_use]
    pub fn strike_spec(&self) -> &StrikeSpec {
        &self.strike_spec
    }

    /// `getDescriptor()`.
    #[must_use]
    pub fn descriptor(&self) -> &Descriptor {
        self.strike_spec.descriptor()
    }

    /// `memoryUsed()`.
    #[must_use]
    pub fn memory_used(&self) -> usize {
        self.memory_used.load(Ordering::Relaxed)
    }

    /// `fRemoved = true`: the strike is no longer in the cache.
    pub(crate) fn set_removed(&self) {
        self.removed.store(true, Ordering::Relaxed);
    }

    /// `addMemoryUsed(bytes)`: called when allocating glyphs, to update the cache accounting.
    // Port of: src/text/gpu/StrikeCache.cpp#L28-L33 (chrome/m156)
    fn add_memory_used(&self, bytes: usize) {
        self.memory_used.fetch_add(bytes, Ordering::Relaxed);
        if !self.removed.load(Ordering::Relaxed) {
            self.cache_total_memory_used
                .fetch_add(bytes, Ordering::Relaxed);
        }
    }

    /// `getGlyph(packedGlyphID)`: the entry of a glyph, made if there is none.
    // Port of: src/gpu/graphite/text/TextStrike.cpp#L32-L40 (chrome/m156)
    #[must_use]
    pub fn get_glyph(&self, packed_glyph_id: PackedGpuGlyphId) -> Arc<GlyphEntry> {
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(glyph) = cache.get(&packed_glyph_id) {
            return Arc::clone(glyph);
        }
        let glyph = Arc::new(GlyphEntry::new(packed_glyph_id));
        cache.insert(packed_glyph_id, Arc::clone(&glyph));
        drop(cache);
        self.add_memory_used(std::mem::size_of::<GlyphEntry>());
        glyph
    }
}
