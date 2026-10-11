// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkStrike.{h,cpp}, src/core/SkStrikeRef.cpp, src/text/StrikeForGPU.{h,cpp}

//! `SkStrike`: the glyphs of one strike (a descriptor with its scaler context), the cache of
//! their metrics, images, paths and drawables, and the memory each one adds.
//!
//! Locking. C++ locks the strike around each operation. Here [`Strike::lock`] returns a
//! [`StrikeGuard`] that holds the strike's lock while a caller prepares glyphs and draws them, as
//! the CPU painter needs to (`SkGlyphRunPainter.cpp#L46-L65`). The public methods on [`Strike`]
//! each take the lock for their own call. Memory is added to the cache when the guard (or the
//! method) is dropped, as `SkStrike::unlock` does.
//!
//! Not ported: `findIntercepts` (T15b), `glyphIDsToPaths`/`glyphIDsToDrawables` (GPU, Phase 6),
//! and `dump`.

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use crate::descriptor::{AutoDescriptor, Descriptor};
use crate::font_metrics::FontMetrics;
use crate::font_types::GlyphId;
use crate::glyph::{ActionType, Glyph, GlyphAction, GlyphDigest, GlyphPositionRoundingSpec};
use crate::mask::MaskFormat;
use crate::packed_glyph_id::PackedGlyphId;
use crate::read_buffer::ReadBuffer;
use crate::rect::Rect;
use crate::scalar::scalar;
use crate::scaler_context::ScalerContext;
use crate::strike_cache::{CacheState, StrikeCache, lock_unpoisoned};
use crate::strike_spec::StrikeSpec;
use crate::write_buffer::BinaryWriteBuffer;

/// `sizeof(SkStrike)` on x64 Linux (g++). The strike starts with this much memory accounted to
/// it (`fMemoryUsed{sizeof(SkStrike)}`). Tests compare totals, so the value is the C++ size, not
/// `size_of::<Strike>()`.
///
/// MSVC x64 (the oracle's ABI, no longer built): `SkMutex` wraps `SkSemaphore` at this pin
/// (`include/private/SkMutex.h`, `SkSemaphore.h`), which is `int` + `uint8` + pointer = 16 bytes on
/// both ABIs. It is not a `pthread_mutex_t`. The other members are pointers, sized integers,
/// `std::vector` and `std::unique_ptr`, whose sizes agree on both ABIs. The arena and hash table
/// were not checked member by member, so the MSVC value is unverified. No ported test depends on
/// it: the tests compare totals only with zero or with "> 0".
// Port of: sizeof(SkStrike), src/core/SkStrike.h#L41-L215 (measured with g++, x64 Linux)
pub(crate) const SIZEOF_STRIKE: usize = 424;

/// `sizeof(SkGlyph)` on x64 (`fMemoryIncrease += sizeof(SkGlyph)` for each new glyph). The
/// same MSVC caveat as [`SIZEOF_STRIKE`] applies: no ported test compares this value.
// Port of: sizeof(SkGlyph), src/core/SkGlyph.h (measured with g++, x64 Linux)
pub(crate) const SIZEOF_GLYPH: usize = 56;

/// `SkStrikePinner`: a pinned strike stays in the cache until its pinner allows its removal.
// Port of: src/core/SkStrike.h#L27-L33 (chrome/m156)
#[doc(alias = "SkStrikePinner")]
pub trait StrikePinner: Send {
    /// Whether the strike may be removed from the cache (`canDelete`).
    fn can_delete(&mut self) -> bool;

    /// Checks the pinner's invariants (`assertValid`).
    fn assert_valid(&self) {}
}

/// The state that the strike lock guards (`SkStrike`'s `fStrikeLock`-protected members).
struct StrikeInner {
    /// `fDigestForPackedGlyphID`: the digest of each packed id.
    digests: HashMap<PackedGlyphId, GlyphDigest>,
    /// `fGlyphForIndex`: the glyphs, indexed by a digest's index.
    glyphs: Vec<Glyph>,
    /// `fScalerContext`: makes glyphs, images, paths and drawables.
    scaler: ScalerContext,
    /// `fMemoryIncrease`: the memory added since the lock was taken.
    memory_increase: usize,
}

/// The glyphs of one (descriptor, scaler context) pair (`SkStrike`).
// Port of: src/core/SkStrike.h#L41-L215 (chrome/m156)
#[doc(alias = "SkStrike")]
pub struct Strike {
    /// `fFontMetrics`.
    font_metrics: FontMetrics,
    /// `fRoundingSpec`.
    rounding_spec: GlyphPositionRoundingSpec,
    /// `fStrikeSpec`.
    spec: StrikeSpec,
    /// `fStrikeCache`: the cache whose totals this strike updates. Weak, so that a dropped cache
    /// does not stay alive through its own strikes.
    cache: Weak<Mutex<CacheState>>,
    /// `fPinner`.
    pinner: Mutex<Option<Box<dyn StrikePinner>>>,
    /// `fPinner != nullptr`, fixed at construction.
    has_pinner: bool,
    /// `fMemoryUsed`. Written only with the cache lock held.
    memory_used: AtomicUsize,
    /// `fRemoved`. Written only with the cache lock held.
    removed: AtomicBool,
    /// The lock and the state it guards.
    inner: Mutex<StrikeInner>,
}

impl fmt::Debug for Strike {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Strike")
            .field("descriptor", self.descriptor())
            .field("memory_used", &self.memory_used())
            .field("has_pinner", &self.has_pinner)
            .finish_non_exhaustive()
    }
}

impl Strike {
    /// `SkStrike::SkStrike`: the metrics come from `metrics` if given, else from the scaler
    /// context. Memory starts at `sizeof(SkStrike)`.
    // Port of: src/core/SkStrike.cpp#L38-L51 (chrome/m156)
    pub(crate) fn new(
        cache: Weak<Mutex<CacheState>>,
        spec: &StrikeSpec,
        mut scaler: ScalerContext,
        metrics: Option<FontMetrics>,
        pinner: Option<Box<dyn StrikePinner>>,
    ) -> Self {
        let font_metrics = metrics.unwrap_or_else(|| scaler.get_font_metrics());
        let rounding_spec = GlyphPositionRoundingSpec::new(
            scaler.is_subpixel(),
            scaler.compute_axis_alignment_for_h_text(),
        );
        Self {
            font_metrics,
            rounding_spec,
            spec: spec.clone(),
            cache,
            has_pinner: pinner.is_some(),
            pinner: Mutex::new(pinner),
            memory_used: AtomicUsize::new(SIZEOF_STRIKE),
            removed: AtomicBool::new(false),
            inner: Mutex::new(StrikeInner {
                digests: HashMap::new(),
                glyphs: Vec::new(),
                scaler,
                memory_increase: 0,
            }),
        }
    }

    /// `SkStrike::getFontMetrics`.
    // Port of: src/core/SkStrike.h#L108-L110 (chrome/m156)
    #[must_use]
    pub fn font_metrics(&self) -> &FontMetrics {
        &self.font_metrics
    }

    /// `SkStrike::roundingSpec`.
    // Port of: src/core/SkStrike.h#L124-L126 (chrome/m156)
    #[must_use]
    pub fn rounding_spec(&self) -> &GlyphPositionRoundingSpec {
        &self.rounding_spec
    }

    /// `SkStrike::strikeSpec`.
    // Port of: src/core/SkStrike.h#L153-L155 (chrome/m156)
    #[must_use]
    pub fn strike_spec(&self) -> &StrikeSpec {
        &self.spec
    }

    /// `SkStrike::getDescriptor`.
    // Port of: src/core/SkStrike.h#L120-L122 (chrome/m156)
    #[must_use]
    pub fn descriptor(&self) -> &Descriptor {
        self.spec.descriptor()
    }

    /// The memory accounted to this strike (`fMemoryUsed`).
    pub(crate) fn memory_used(&self) -> usize {
        self.memory_used.load(Ordering::Relaxed)
    }

    /// Whether this strike has a pinner (`fPinner != nullptr`).
    pub(crate) fn has_pinner(&self) -> bool {
        self.has_pinner
    }

    /// Marks the strike as no longer in the cache (`fRemoved = true`).
    pub(crate) fn set_removed(&self) {
        self.removed.store(true, Ordering::Relaxed);
    }

    /// `SkStrikePinner::canDelete`, or true for an unpinned strike.
    pub(crate) fn can_delete(&self) -> bool {
        match lock_unpoisoned(&self.pinner).as_mut() {
            Some(pinner) => pinner.can_delete(),
            None => true,
        }
    }

    /// `SkStrike::updateMemoryUsage`: adds `increase` bytes to this strike and, if it is still
    /// cached, to the cache's total.
    // Port of: src/core/SkStrike.cpp#L461-L468 (chrome/m156)
    fn update_memory_usage(&self, increase: usize) {
        if increase == 0 {
            return;
        }
        let Some(cache) = self.cache.upgrade() else {
            // The cache is gone, so nothing counts this memory any more.
            return;
        };
        let mut state = lock_unpoisoned(&cache);
        self.memory_used.fetch_add(increase, Ordering::Relaxed);
        if !self.removed.load(Ordering::Relaxed) {
            state.total_memory_used += increase;
        }
    }

    /// `SkStrike::lock`: takes the strike's lock. The guard adds the memory of the glyphs it
    /// makes when it is dropped.
    // Port of: src/core/SkStrike.cpp#L75-L79 (chrome/m156)
    pub fn lock(&self) -> StrikeGuard<'_> {
        let mut inner = lock_unpoisoned(&self.inner);
        inner.memory_increase = 0;
        StrikeGuard {
            strike: self,
            inner,
        }
    }

    /// `SkStrike::verifyPinnedStrike`: checks the pinner's invariants, if there is a pinner.
    // Port of: src/core/SkStrike.h#L132-L136 (chrome/m156)
    #[doc(alias = "verifyPinnedStrike")]
    pub fn verify_pinned_strike(&self) {
        if let Some(pinner) = lock_unpoisoned(&self.pinner).as_ref() {
            pinner.assert_valid();
        }
    }

    /// `SkStrike::FlattenGlyphsByType`: writes the count and then the metrics and data of each of
    /// the glyphs with images, paths and drawables.
    ///
    /// # Panics
    /// If a count does not fit an `int`, where C++ has `SkASSERT_RELEASE`.
    // Port of: src/core/SkStrike.cpp#L86-L115 (chrome/m156)
    #[doc(alias = "FlattenGlyphsByType")]
    pub fn flatten_glyphs_by_type(
        buffer: &mut BinaryWriteBuffer,
        images: &[Glyph],
        paths: &[Glyph],
        drawables: &[Glyph],
    ) {
        let count = |n: usize| i32::try_from(n).expect("SkASSERT_RELEASE(SkTFitsIn<int>)");

        buffer.write_int(count(images.len()));
        for glyph in images {
            glyph.flatten_metrics(buffer);
            glyph.flatten_image(buffer);
        }

        buffer.write_int(count(paths.len()));
        for glyph in paths {
            glyph.flatten_metrics(buffer);
            glyph.flatten_path(buffer);
        }

        buffer.write_int(count(drawables.len()));
        for glyph in drawables {
            glyph.flatten_metrics(buffer);
            glyph.flatten_drawable(buffer);
        }
    }

    /// `SkStrike::mergeFromBuffer`: reads the glyphs that
    /// [`flatten_glyphs_by_type`](Self::flatten_glyphs_by_type) wrote, and adds their images,
    /// paths and drawables to this strike. Returns false if the buffer is invalid.
    // Port of: src/core/SkStrike.cpp#L117-L162 (chrome/m156)
    #[doc(alias = "mergeFromBuffer")]
    pub fn merge_from_buffer(&self, buffer: &mut ReadBuffer<'_>) -> bool {
        // Read glyphs with images for the current strike.
        let images_count = buffer.read_int();
        if images_count == 0 && !buffer.is_valid() {
            return false;
        }
        {
            let mut guard = self.lock();
            for _ in 0..images_count {
                if !guard.merge_glyph_and_image_from_buffer(buffer) {
                    return false;
                }
            }
        }

        // Read glyphs with paths for the current strike.
        let paths_count = buffer.read_int();
        if paths_count == 0 && !buffer.is_valid() {
            return false;
        }
        {
            let mut guard = self.lock();
            for _ in 0..paths_count {
                if !guard.merge_glyph_and_path_from_buffer(buffer) {
                    return false;
                }
            }
        }

        // Read glyphs with drawables for the current strike.
        let drawables_count = buffer.read_int();
        if drawables_count == 0 && !buffer.is_valid() {
            return false;
        }
        {
            let mut guard = self.lock();
            for _ in 0..drawables_count {
                if !guard.merge_glyph_and_drawable_from_buffer(buffer) {
                    return false;
                }
            }
        }

        true
    }

    /// `SkStrikeTestingPeer::GetGlyph`: a copy of the glyph for `packed_id`, made if it is new.
    // Port of: tests/SkStrikeTest.cpp#L184-L189 (chrome/m156)
    #[doc(hidden)]
    #[must_use]
    pub fn glyph_for_testing(&self, packed_id: PackedGlyphId) -> Glyph {
        self.lock().glyph_clone_for_id(packed_id)
    }

    /// `SkStrike::metrics`: the metrics of each glyph (no images or paths). The glyphs are copies
    /// of the strike's records, so an image or path already prepared comes along.
    // Port of: src/core/SkStrike.cpp#L218-L223 (chrome/m156), kMetricsOnly
    #[must_use]
    pub fn metrics(&self, glyph_ids: &[GlyphId]) -> Vec<Glyph> {
        let mut guard = self.lock();
        glyph_ids
            .iter()
            .map(|&id| guard.glyph_clone_for_id(PackedGlyphId::from_glyph_id(id)))
            .collect()
    }

    /// `SkStrike::preparePaths`: the metrics and path of each glyph.
    // Port of: src/core/SkStrike.cpp#L242-L246 (chrome/m156), kMetricsAndPath
    #[must_use]
    pub fn prepare_paths(&self, glyph_ids: &[GlyphId]) -> Vec<Glyph> {
        let mut guard = self.lock();
        glyph_ids
            .iter()
            .map(|&id| {
                let digest =
                    guard.digest_for_id(ActionType::DirectMask, PackedGlyphId::from_glyph_id(id));
                guard.inner.prepare_for_path(digest.index());
                guard.inner.glyphs[digest.index()].clone()
            })
            .collect()
    }

    /// `SkStrike::prepareImages`: the metrics and mask image of each glyph.
    // Port of: src/core/SkStrike.cpp#L248-L260 (chrome/m156)
    #[must_use]
    pub fn prepare_images(&self, packed_ids: &[PackedGlyphId]) -> Vec<Glyph> {
        let mut guard = self.lock();
        packed_ids
            .iter()
            .map(|&id| {
                let digest = guard.digest_for_id(ActionType::DirectMask, id);
                guard.inner.prepare_for_image(digest.index());
                guard.inner.glyphs[digest.index()].clone()
            })
            .collect()
    }

    /// `SkStrike::prepareDrawables`: the metrics and drawable of each glyph.
    // Port of: src/core/SkStrike.cpp#L261-L275 (chrome/m156)
    #[must_use]
    pub fn prepare_drawables(&self, glyph_ids: &[GlyphId]) -> Vec<Glyph> {
        let mut guard = self.lock();
        glyph_ids
            .iter()
            .map(|&id| {
                let digest =
                    guard.digest_for_id(ActionType::DirectMask, PackedGlyphId::from_glyph_id(id));
                guard.inner.prepare_for_drawable(digest.index());
                guard.inner.glyphs[digest.index()].clone()
            })
            .collect()
    }

    /// `SkStrike::getWidthsStrided` for contiguous glyph ids: the advance of each glyph times
    /// `scale`.
    // Port of: src/core/SkStrike.cpp#L224-L240 (chrome/m156)
    #[must_use]
    pub fn get_widths(&self, glyph_ids: &[GlyphId], scale: scalar) -> Vec<scalar> {
        let mut guard = self.lock();
        glyph_ids
            .iter()
            .map(|&id| {
                let index = guard.glyph_index_for_id(PackedGlyphId::from_glyph_id(id));
                guard.inner.glyphs[index].advance_x() * scale
            })
            .collect()
    }
}

/// The lock of a [`Strike`] (`SkAutoMutexExclusive` on `fStrikeLock`, with `SkStrike::Monitor`).
/// Dropping it adds the memory of the glyphs made under it to the cache.
#[must_use = "the strike is unlocked when the guard is dropped"]
pub struct StrikeGuard<'a> {
    strike: &'a Strike,
    inner: MutexGuard<'a, StrikeInner>,
}

impl fmt::Debug for StrikeGuard<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StrikeGuard")
            .field("strike", &self.strike)
            .finish_non_exhaustive()
    }
}

impl StrikeGuard<'_> {
    /// `SkStrike::digestFor`: the digest of `packed_id` for `action`, preparing the glyph for that
    /// action if it has not been decided yet.
    // Port of: src/core/SkStrike.cpp#L346-L365 (chrome/m156)
    pub fn digest_for(&mut self, action: ActionType, packed_id: PackedGlyphId) -> GlyphDigest {
        self.digest_for_id(action, packed_id)
    }

    /// `SkStrike::glyph(SkGlyphDigest)`: the glyph a digest refers to.
    // Port of: src/core/SkStrike.cpp#L337-L339 (chrome/m156)
    #[must_use]
    pub fn glyph(&self, digest: GlyphDigest) -> &Glyph {
        &self.inner.glyphs[digest.index()]
    }

    /// `SkStrike::prepareForImage`: makes the mask image of the glyph, if it has none. Returns
    /// whether the glyph has an image.
    // Port of: src/core/SkStrike.cpp#L374-L380 (chrome/m156)
    pub fn prepare_for_image(&mut self, digest: GlyphDigest) -> bool {
        self.inner.prepare_for_image(digest.index())
    }

    /// `SkStrike::prepareForPath`: makes the path of the glyph, if it has none. Returns whether
    /// the glyph has a path.
    // Port of: src/core/SkStrike.cpp#L381-L387 (chrome/m156)
    pub fn prepare_for_path(&mut self, digest: GlyphDigest) -> bool {
        self.inner.prepare_for_path(digest.index())
    }

    /// `SkStrike::prepareForDrawable`: makes the drawable of the glyph, if it has none. Returns
    /// whether the glyph has a drawable.
    // Port of: src/core/SkStrike.cpp#L388-L396 (chrome/m156)
    pub fn prepare_for_drawable(&mut self, digest: GlyphDigest) -> bool {
        self.inner.prepare_for_drawable(digest.index())
    }

    /// `SkStrike::mergeGlyphFromBuffer`: reads a glyph's metrics, and adds a glyph for them if
    /// the strike has none for the id yet. Returns the index of the strike's glyph.
    // Port of: src/core/SkStrike.cpp#L397-L415 (chrome/m156)
    fn merge_glyph_from_buffer(&mut self, buffer: &mut ReadBuffer<'_>) -> Option<usize> {
        debug_assert!(buffer.is_valid());
        let prototype_glyph = Glyph::make_from_buffer(buffer);
        if !buffer.validate(prototype_glyph.is_some()) {
            return None;
        }
        let prototype_glyph = prototype_glyph?;

        // Check if this glyph has already been seen.
        if let Some(digest) = self.inner.digests.get(&prototype_glyph.packed_id()) {
            return Some(digest.index());
        }

        // This is the first time. Allocate a new glyph.
        self.inner.memory_increase += SIZEOF_GLYPH;
        Some(self.inner.add_glyph_and_digest(prototype_glyph).index())
    }

    /// `SkStrike::mergeGlyphAndImageFromBuffer`.
    // Port of: src/core/SkStrike.cpp#L417-L425 (chrome/m156)
    fn merge_glyph_and_image_from_buffer(&mut self, buffer: &mut ReadBuffer<'_>) -> bool {
        debug_assert!(buffer.is_valid());
        let index = self.merge_glyph_from_buffer(buffer);
        if !buffer.validate(index.is_some()) {
            return false;
        }
        let Some(index) = index else { return false };
        self.inner.memory_increase += self.inner.glyphs[index].add_image_from_buffer(buffer);
        buffer.is_valid()
    }

    /// `SkStrike::mergeGlyphAndPathFromBuffer`.
    // Port of: src/core/SkStrike.cpp#L427-L435 (chrome/m156)
    fn merge_glyph_and_path_from_buffer(&mut self, buffer: &mut ReadBuffer<'_>) -> bool {
        debug_assert!(buffer.is_valid());
        let index = self.merge_glyph_from_buffer(buffer);
        if !buffer.validate(index.is_some()) {
            return false;
        }
        let Some(index) = index else { return false };
        self.inner.memory_increase += self.inner.glyphs[index].add_path_from_buffer(buffer);
        buffer.is_valid()
    }

    /// `SkStrike::mergeGlyphAndDrawableFromBuffer`.
    // Port of: src/core/SkStrike.cpp#L437-L445 (chrome/m156)
    fn merge_glyph_and_drawable_from_buffer(&mut self, buffer: &mut ReadBuffer<'_>) -> bool {
        debug_assert!(buffer.is_valid());
        let index = self.merge_glyph_from_buffer(buffer);
        if !buffer.validate(index.is_some()) {
            return false;
        }
        let Some(index) = index else { return false };
        self.inner.memory_increase += self.inner.glyphs[index].add_drawable_from_buffer(buffer);
        buffer.is_valid()
    }

    /// `SkStrike::glyph(SkPackedGlyphID)`: the digest for `DirectMask`, and the glyph (cloned).
    fn glyph_clone_for_id(&mut self, packed_id: PackedGlyphId) -> Glyph {
        let index = self.glyph_index_for_id(packed_id);
        self.inner.glyphs[index].clone()
    }

    /// The index of the glyph for `packed_id`, made with the `DirectMask` action
    /// (`SkStrike::glyph(SkPackedGlyphID)`).
    // Port of: src/core/SkStrike.cpp#L341-L345 (chrome/m156)
    fn glyph_index_for_id(&mut self, packed_id: PackedGlyphId) -> usize {
        self.digest_for_id(ActionType::DirectMask, packed_id)
            .index()
    }

    /// `SkStrike::digestFor` on a packed id.
    // Port of: src/core/SkStrike.cpp#L346-L365 (chrome/m156)
    fn digest_for_id(&mut self, action: ActionType, packed_id: PackedGlyphId) -> GlyphDigest {
        self.inner.digest_for(action, packed_id)
    }
}

impl Drop for StrikeGuard<'_> {
    /// `SkStrike::unlock`.
    // Port of: src/core/SkStrike.cpp#L80-L86 (chrome/m156)
    fn drop(&mut self) {
        let increase = std::mem::take(&mut self.inner.memory_increase);
        self.strike.update_memory_usage(increase);
    }
}

impl StrikeInner {
    /// `SkStrike::addGlyphAndDigest`: stores the glyph and its digest, returning the digest.
    // Port of: src/core/SkStrike.cpp#L366-L373 (chrome/m156)
    fn add_glyph_and_digest(&mut self, glyph: Glyph) -> GlyphDigest {
        let index = self.glyphs.len();
        let digest = GlyphDigest::new(index, &glyph);
        self.glyphs.push(glyph);
        self.digests.insert(digest.key(), digest);
        digest
    }

    /// `SkStrike::digestFor`.
    // Port of: src/core/SkStrike.cpp#L346-L365 (chrome/m156)
    fn digest_for(&mut self, action: ActionType, packed_id: PackedGlyphId) -> GlyphDigest {
        let existing = self.digests.get(&packed_id).copied();
        if let Some(digest) = existing.filter(|d| d.action_for(action) != GlyphAction::Unset) {
            return digest;
        }
        let mut digest = if let Some(digest) = existing {
            digest
        } else {
            let glyph = self.scaler.make_glyph(packed_id);
            self.memory_increase += SIZEOF_GLYPH;
            self.add_glyph_and_digest(glyph)
        };
        let index = digest.index();
        self.set_action_for(&mut digest, action, index);
        self.digests.insert(digest.key(), digest);
        digest
    }

    /// `SkGlyphDigest::setActionFor`: decides the action for `action_type` from the glyph's
    /// preparation, and records it in the digest.
    // Port of: src/core/SkGlyph.cpp#L640-L685 (chrome/m156)
    fn set_action_for(&mut self, digest: &mut GlyphDigest, action_type: ActionType, index: usize) {
        // A glyph marked `Drop` because it is empty needs nothing more.
        if digest.action_for(action_type) != GlyphAction::Unset {
            return;
        }
        let mut action = GlyphAction::Reject;
        match action_type {
            ActionType::DirectMask => {
                if digest.fits_in_atlas_direct() {
                    action = GlyphAction::Accept;
                }
            }
            ActionType::DirectMaskCpu => {
                if self.prepare_for_image(index) {
                    debug_assert!(!self.glyphs[index].is_empty());
                    action = GlyphAction::Accept;
                }
            }
            ActionType::Mask => {
                if digest.fits_in_atlas_interpolated() {
                    action = GlyphAction::Accept;
                }
            }
            ActionType::Sdft => {
                if digest.fits_in_atlas_direct() && digest.mask_format() == MaskFormat::Sdf {
                    action = GlyphAction::Accept;
                }
            }
            ActionType::Path => {
                if self.prepare_for_path(index) {
                    action = GlyphAction::Accept;
                }
            }
            ActionType::Drawable => {
                if self.prepare_for_drawable(index) {
                    action = GlyphAction::Accept;
                }
            }
        }
        digest.set_action(action_type, action);
    }

    /// `SkStrike::prepareForImage`: makes the image if the glyph has none, and accounts for it.
    // Port of: src/core/SkStrike.cpp#L374-L380 (chrome/m156)
    fn prepare_for_image(&mut self, index: usize) -> bool {
        let glyph = &mut self.glyphs[index];
        if !glyph.set_image_has_been_called() {
            self.scaler.get_image(glyph);
            self.memory_increase += glyph.image_size();
        }
        glyph.image().is_some()
    }

    /// `SkStrike::prepareForPath`: makes the path if the glyph has none, and accounts for it.
    // Port of: src/core/SkStrike.cpp#L381-L387 (chrome/m156)
    fn prepare_for_path(&mut self, index: usize) -> bool {
        let glyph = &mut self.glyphs[index];
        if !glyph.set_path_has_been_called() {
            self.scaler.get_path(glyph);
            if let Some(path) = glyph.path() {
                self.memory_increase += path.approximate_bytes_used();
            }
        }
        glyph.path().is_some()
    }

    /// `SkStrike::prepareForDrawable`: makes the drawable if the glyph has none, and accounts for
    /// it.
    // Port of: src/core/SkStrike.cpp#L388-L396 (chrome/m156)
    fn prepare_for_drawable(&mut self, index: usize) -> bool {
        let glyph = &mut self.glyphs[index];
        if !glyph.set_drawable_has_been_called() {
            let drawable = self.scaler.get_drawable(glyph);
            glyph.set_drawable(drawable);
            if let Some(drawable) = glyph.drawable() {
                self.memory_increase += drawable.approximate_bytes_used();
            }
        }
        glyph.drawable().is_some()
    }
}

/// `StrikePromise`: a strike, or the spec of one that has not been made yet
/// (`sktext::SkStrikePromise`).
// Port of: src/text/StrikeForGPU.h#L23-L45 (chrome/m156)
#[doc(alias = "SkStrikePromise")]
#[derive(Debug)]
pub enum StrikePromise {
    /// A strike that exists.
    Strike(Arc<Strike>),
    /// A spec whose strike is made when it is first needed.
    Spec(Box<StrikeSpec>),
}

impl StrikePromise {
    /// `SkStrikePromise(sk_sp<SkStrike>&&)`.
    // Port of: src/text/StrikeForGPU.cpp#L22-L23 (chrome/m156)
    #[must_use]
    pub fn from_strike(strike: Arc<Strike>) -> Self {
        Self::Strike(strike)
    }

    /// `SkStrikePromise(const SkStrikeSpec&)`.
    // Port of: src/text/StrikeForGPU.cpp#L24-L26 (chrome/m156)
    #[must_use]
    pub fn from_spec(spec: &StrikeSpec) -> Self {
        Self::Spec(Box::new(spec.clone()))
    }

    /// `SkStrikePromise::strike`: turns a spec into a strike (in the global cache) and returns it.
    // Port of: src/text/StrikeForGPU.cpp#L27-L35 (chrome/m156)
    pub fn strike(&mut self) -> &Arc<Strike> {
        if let Self::Spec(spec) = self {
            let strike = spec.find_or_create_strike();
            *self = Self::Strike(strike);
        }
        match self {
            Self::Strike(strike) => strike,
            Self::Spec(_) => unreachable!("the spec was made into a strike above"),
        }
    }

    /// `SkStrikePromise::resetStrike`: drops the reference to the strike, so that it can be
    /// purged from the cache if needed.
    ///
    /// skia-rust: C++ leaves a null strike; here the promise goes back to the spec of the strike,
    /// which makes a strike again if [`strike`](Self::strike) is called.
    // Port of: src/text/StrikeForGPU.cpp#L37-L39 (chrome/m156)
    pub fn reset_strike(&mut self) {
        if let Self::Strike(strike) = self {
            *self = Self::Spec(Box::new(strike.strike_spec().clone()));
        }
    }

    /// `SkStrikePromise::MakeFromBuffer`: reads the descriptor that [`flatten`](Self::flatten)
    /// wrote and promises the strike in `strike_cache` that it names. If there is a client, then
    /// the descriptor is from a different process: its typeface id is translated from the
    /// strike server's to the client's. Returns `None` if the buffer is bad, the translation
    /// fails or the cache has no such strike.
    // Port of: src/text/gpu/StrikeCache.cpp#L174-L197 (chrome/m156)
    #[doc(alias = "MakeFromBuffer")]
    #[must_use]
    pub fn make_from_buffer(
        buffer: &mut ReadBuffer<'_>,
        client: Option<&dyn TypefaceIdTranslator>,
        strike_cache: &StrikeCache,
    ) -> Option<Self> {
        let descriptor = AutoDescriptor::make_from_buffer(buffer);
        if !buffer.validate(descriptor.is_some()) {
            return None;
        }
        let mut descriptor = descriptor?;

        // If there is a client, then this from a different process. Translate the SkTypefaceID
        // from the strike server (Renderer) process to strike client (GPU) process.
        if let Some(client) = client {
            if !client.translate_typeface_id(&mut descriptor) {
                return None;
            }
        }

        // C++ has SkASSERT(strike != nullptr) here, which aborts a debug build on a descriptor
        // from bad data. The validation below is what release builds do.
        let strike = strike_cache.find_strike(descriptor.get_desc());
        if !buffer.validate(strike.is_some()) {
            return None;
        }

        strike.map(Self::from_strike)
    }

    /// `SkStrikePromise::flatten`: writes the descriptor.
    // Port of: src/text/StrikeForGPU.cpp#L49-L51 (chrome/m156)
    pub fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        self.descriptor().flatten(buffer);
    }

    /// `SkStrikePromise::descriptor`.
    // Port of: src/text/StrikeForGPU.cpp#L41-L47 (chrome/m156)
    #[must_use]
    pub fn descriptor(&self) -> &Descriptor {
        match self {
            Self::Strike(strike) => strike.descriptor(),
            Self::Spec(spec) => spec.descriptor(),
        }
    }
}

/// What `SkStrikePromise::MakeFromBuffer` needs of the strike client
/// (`SkStrikeClient::translateTypefaceID`): rewrites the typeface id in a descriptor from the
/// strike server's to the client's.
// Port of: include/private/chromium/SkChromeRemoteGlyphCache.h#L141-L143 (chrome/m156)
pub trait TypefaceIdTranslator {
    /// `SkStrikeClient::translateTypefaceID`. Returns false if the descriptor has no record or
    /// names a typeface the client has not received.
    fn translate_typeface_id(&self, descriptor: &mut AutoDescriptor) -> bool;
}

/// A strike as the GPU text code sees it (`sktext::StrikeForGPU`): a real [`Strike`], or the
/// strike server's record of the glyphs a client needs.
///
/// skia-rust: `prepareForImage`, `prepareForPath` and `prepareForDrawable` are not part of the
/// trait. Only the remote strike calls them (on its own glyphs) and the strike keeps its own.
// Port of: src/text/StrikeForGPU.h#L70-L93 (chrome/m156)
#[doc(alias = "StrikeForGPU")]
pub trait StrikeForGpu: Send + Sync {
    /// `lock` and `unlock` as `StrikeMutationMonitor` uses them: the lock is held until the
    /// result is dropped.
    #[doc(alias = "StrikeMutationMonitor")]
    fn lock_for_gpu(&self) -> Box<dyn StrikeForGpuLock + '_>;

    /// `getDescriptor`.
    fn descriptor_for_gpu(&self) -> &Descriptor;

    /// `roundingSpec`.
    fn rounding_spec_for_gpu(&self) -> &GlyphPositionRoundingSpec;

    /// `strikePromise`. Takes the handle, as the strike hands a reference to itself to the
    /// promise (`sk_ref_sp<SkStrike>(this)`).
    fn strike_promise(self: Arc<Self>) -> StrikePromise;
}

/// The locked strike (`StrikeMutationMonitor`): the operations that need the lock.
// Port of: src/text/StrikeForGPU.h#L70-L93 (chrome/m156)
pub trait StrikeForGpuLock {
    /// `digestFor`: the digest of `packed_id` for `action`, deciding its action if it has not
    /// been decided yet.
    fn digest_for(&mut self, action: ActionType, packed_id: PackedGlyphId) -> GlyphDigest;
}

/// `sktext::StrikeForGPUCacheInterface`: where the GPU text code gets its strikes.
// Port of: src/text/StrikeForGPU.h#L121-L125 (chrome/m156)
#[doc(alias = "StrikeForGPUCacheInterface")]
pub trait StrikeForGpuCacheInterface {
    /// `findOrCreateScopedStrike`.
    fn find_or_create_scoped_strike(&self, spec: &StrikeSpec) -> Arc<dyn StrikeForGpu>;
}

impl StrikeForGpu for Strike {
    // Port of: src/core/SkStrike.h (lock/unlock, chrome/m156)
    fn lock_for_gpu(&self) -> Box<dyn StrikeForGpuLock + '_> {
        Box::new(self.lock())
    }

    // Port of: src/core/SkStrike.h#L120-L122 (chrome/m156)
    fn descriptor_for_gpu(&self) -> &Descriptor {
        self.descriptor()
    }

    // Port of: src/core/SkStrike.h#L124-L126 (chrome/m156)
    fn rounding_spec_for_gpu(&self) -> &GlyphPositionRoundingSpec {
        self.rounding_spec()
    }

    // Port of: src/core/SkStrike.h#L118-L120 (chrome/m156)
    fn strike_promise(self: Arc<Self>) -> StrikePromise {
        StrikePromise::from_strike(self)
    }
}

impl StrikeForGpuLock for StrikeGuard<'_> {
    fn digest_for(&mut self, action: ActionType, packed_id: PackedGlyphId) -> GlyphDigest {
        self.digest_for_id(action, packed_id)
    }
}

impl StrikeForGpuCacheInterface for StrikeCache {
    // Port of: src/core/SkStrikeCache.cpp (findOrCreateScopedStrike, chrome/m156)
    fn find_or_create_scoped_strike(&self, spec: &StrikeSpec) -> Arc<dyn StrikeForGpu> {
        self.find_or_create_strike(spec)
    }
}

/// `SkStrikeRef`: a strike with the scale from its size to the source's (for text drawn as
/// canonical-size paths, `SkFont::setupForAsPaths`).
// Port of: src/core/SkStrikeRef.cpp#L36-L39 (chrome/m156)
#[doc(alias = "SkStrikeRef")]
#[derive(Debug, Clone)]
pub struct StrikeRef {
    strike: Arc<Strike>,
    strike_to_source_scale: scalar,
}

/// `scale_rect` of `SkStrikeRef.cpp`.
// Port of: src/core/SkStrikeRef.cpp#L13-L15 (chrome/m156)
fn scale_rect(r: Rect, s: scalar) -> Rect {
    Rect::from_ltrb(r.left * s, r.top * s, r.right * s, r.bottom * s)
}

impl StrikeRef {
    /// `SkStrikeRef(sk_sp<SkStrike>, SkScalar)`.
    // Port of: src/core/SkStrikeRef.cpp#L36-L39 (chrome/m156)
    #[must_use]
    pub fn new(strike: Arc<Strike>, strike_to_source_scale: scalar) -> Self {
        Self {
            strike,
            strike_to_source_scale,
        }
    }

    /// `SkStrikeRef::getWidths`.
    // Port of: src/core/SkStrikeRef.cpp#L41-L43 (chrome/m156)
    pub fn get_widths(&self, glyphs: &[GlyphId], widths: &mut [scalar]) {
        self.get_widths_bounds(glyphs, widths, &mut []);
    }

    /// `SkStrikeRef::getWidth`.
    // Port of: src/core/SkStrikeRef.cpp#L45-L49 (chrome/m156)
    #[must_use]
    pub fn get_width(&self, glyph: GlyphId) -> scalar {
        let mut width = 0.0;
        self.get_widths(&[glyph], std::slice::from_mut(&mut width));
        width
    }

    /// `SkStrikeRef::getWidthsBounds`: the widths and bounds of the glyphs, scaled to the source
    /// size. An empty `widths` or `bounds` is not written.
    // Port of: src/core/SkStrikeRef.cpp#L51-L72 (chrome/m156)
    pub fn get_widths_bounds(
        &self,
        glyphs: &[GlyphId],
        widths: &mut [scalar],
        bounds: &mut [Rect],
    ) {
        let results = self.strike.metrics(glyphs);
        if !bounds.is_empty() {
            let n = bounds.len().min(results.len());
            for (bound, glyph) in bounds[..n].iter_mut().zip(&results[..n]) {
                *bound = scale_rect(glyph.rect(), self.strike_to_source_scale);
            }
        }
        if !widths.is_empty() {
            let n = widths.len().min(results.len());
            for (width, glyph) in widths[..n].iter_mut().zip(&results[..n]) {
                *width = glyph.advance_x() * self.strike_to_source_scale;
            }
        }
    }
}
