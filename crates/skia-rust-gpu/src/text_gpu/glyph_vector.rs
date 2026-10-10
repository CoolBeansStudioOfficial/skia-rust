// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/text/gpu/GlyphVector.h, src/text/gpu/GlyphVector.cpp

//! [`GlyphVector`]: the glyphs of a sub run, as packed ids until the code is running on the GPU
//! side, where they become atlas entries.
//!
//! GlyphVector provides a way to delay the lookup of glyphs until the code is running on the GPU
//! in single threaded mode. The GlyphVector is created in a multi-threaded environment, but the
//! StrikeCache is only single threaded (and must be single threaded because of the atlas).
//!
//! Once in the single-threaded GPU environment the glyph packed IDs are converted into a GPU
//! backend specific entry type from which each glyph's atlas location can be determined.
//!
//! skia-rust: C++ stores the glyphs in untyped storage that holds a packed id and then a backend
//! `Glyph` in place, with templated backend data. Graphite is the only backend here, so the
//! backend data is [`GlyphData`] and the ids stay available beside it. Not ported: `flatten` and
//! `MakeFromBuffer` (the remote glyph cache, T23).

use std::sync::{Mutex, MutexGuard, PoisonError};

use skia_rust_core::packed_glyph_id::PackedGlyphId;
use skia_rust_core::strike::StrikePromise;

use crate::gpu::mask_format::MaskFormat;
use crate::graphite::recorder::Recorder;
use crate::graphite::text::glyph_data::GlyphData;
use crate::graphite::text::text_strike::TextStrike;

/// Parameters that apply to all glyphs in a [`GlyphVector`] so that the GPU backend knows how
/// they are meant to be rendered (`sktext::gpu::RendererData`).
// Port of: src/text/gpu/GlyphVector.h#L80-L85 (chrome/m156)
#[doc(alias = "sktext::gpu::RendererData")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RendererData {
    /// `srcPadding`.
    pub src_padding: i32,
    /// `isSDF`.
    pub is_sdf: bool,
    /// `isLCD`.
    pub is_lcd: bool,
    /// `maskFormat`.
    pub mask_format: MaskFormat,
}

/// The glyphs of a sub run (`sktext::gpu::GlyphVector`).
// Port of: src/text/gpu/GlyphVector.h#L87-L240 (chrome/m156)
#[doc(alias = "sktext::gpu::GlyphVector")]
#[derive(Debug)]
pub struct GlyphVector {
    /// `fStrikePromise`.
    strike_promise: Mutex<StrikePromise>,
    /// The packed ids of the glyphs (the first bytes of each of `fGlyphs`).
    packed_ids: Vec<PackedGlyphId>,
    /// The backend data and the glyphs converted to the backend's entries (`fBackendDataBytes`
    /// and the in-place glyphs). `None` until `init_backend_data`.
    backend: Mutex<Option<GlyphData>>,
}

impl GlyphVector {
    /// `Make(promise, packedIDs, alloc)`.
    ///
    /// # Panics
    /// If `packed_ids` is empty.
    // Port of: src/text/gpu/GlyphVector.cpp#L49-L60 (chrome/m156)
    #[must_use]
    pub fn make(promise: StrikePromise, packed_ids: &[PackedGlyphId]) -> Self {
        assert!(!packed_ids.is_empty());
        Self {
            strike_promise: Mutex::new(promise),
            packed_ids: packed_ids.to_vec(),
            backend: Mutex::new(None),
        }
    }

    /// `glyphCount()`.
    // Port of: src/text/gpu/GlyphVector.h#L119 (chrome/m156)
    #[must_use]
    pub fn glyph_count(&self) -> usize {
        self.packed_ids.len()
    }

    /// `unflattenSize()`: doesn't include the size of the vector itself because it is embedded in
    /// each of the sub runs.
    // Port of: src/text/gpu/GlyphVector.h#L127 (chrome/m156)
    #[must_use]
    pub fn unflatten_size(&self) -> usize {
        std::mem::size_of::<usize>() * self.packed_ids.len()
    }

    /// `getPackedGlyphID(index)`.
    // Port of: src/text/gpu/GlyphVector.h#L129-L136 (chrome/m156)
    #[must_use]
    pub fn get_packed_glyph_id(&self, index: usize) -> PackedGlyphId {
        self.packed_ids[index]
    }

    /// The strike promise (`strikePromise()`).
    // Port of: src/text/gpu/GlyphVector.h#L138-L139 (chrome/m156)
    pub fn strike_promise(&self) -> MutexGuard<'_, StrikePromise> {
        self.strike_promise
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// `hasBackendData()`.
    // Port of: src/text/gpu/GlyphVector.h#L141-L144 (chrome/m156)
    #[must_use]
    pub fn has_backend_data(&self) -> bool {
        self.backend().is_some()
    }

    /// `accessBackendData<GlyphData>()` and `accessBackendGlyphs<Glyph>()`: the backend data,
    /// which holds the glyphs.
    // Port of: src/text/gpu/GlyphVector.h#L146-L161 (chrome/m156)
    pub fn backend(&self) -> MutexGuard<'_, Option<GlyphData>> {
        self.backend.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `initBackendData<GlyphData>(cache, recorder, rendererData)`: finds the backend strike in
    /// the recorder's strike cache and converts each packed glyph id to the backend's glyph.
    // Port of: src/text/gpu/GlyphVector.h#L163-L195 (chrome/m156)
    pub fn init_backend_data(&self, recorder: &Recorder, renderer_data: RendererData) {
        let mut backend = self.backend();
        debug_assert!(backend.is_none());

        let mut promise = self.strike_promise();
        let strike = std::sync::Arc::clone(promise.strike());
        let spec = strike.strike_spec();
        let backend_strike = {
            let priv_ = recorder.priv_();
            let mut cache = priv_.strike_cache().borrow_mut();
            TextStrike::get_or_create(&mut cache, spec)
        };

        *backend = Some(GlyphData::new(
            backend_strike,
            recorder,
            renderer_data,
            &self.packed_ids,
        ));

        // Drop the reference to the strike so that it can be purged from the cache if needed.
        promise.reset_strike();
    }
}
