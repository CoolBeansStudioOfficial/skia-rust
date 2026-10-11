// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/text/gpu/GlyphVector.h, src/text/gpu/GlyphVector.cpp

//! [`GlyphVector`]: the glyphs of a sub run, as packed ids until the code is running on the GPU
//! side, where they become atlas entries.
//!
//! `GlyphVector` provides a way to delay the lookup of glyphs until the code is running on the GPU
//! in single threaded mode. The `GlyphVector` is created in a multi-threaded environment, but the
//! `StrikeCache` is only single threaded (and must be single threaded because of the atlas).
//!
//! Once in the single-threaded GPU environment the glyph packed IDs are converted into a GPU
//! backend specific entry type from which each glyph's atlas location can be determined.
//!
//! skia-rust: C++ stores the glyphs in untyped storage that holds a packed id and then a backend
//! `Glyph` in place, with templated backend data. Graphite is the only backend here, so the
//! backend data is [`GlyphData`] and the ids stay available beside it.

use std::sync::{Mutex, MutexGuard, PoisonError};

use skia_rust_core::packed_glyph_id::PackedGlyphId;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::strike::{StrikePromise, TypefaceIdTranslator};
use skia_rust_core::strike_cache::StrikeCache;
use skia_rust_core::write_buffer::BinaryWriteBuffer;

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
        assert_ne!(packed_ids.len(), 0);
        Self {
            strike_promise: Mutex::new(promise),
            packed_ids: packed_ids.to_vec(),
            backend: Mutex::new(None),
        }
    }

    /// `MakeFromBuffer(buffer, client, alloc)`: reads a glyph vector that
    /// [`flatten`](Self::flatten) wrote. The strike is looked up in the global strike cache,
    /// after the typeface id is translated if there is a client. Returns `None` if the data is
    /// bad.
    // Port of: src/text/gpu/GlyphVector.cpp#L62-L91 (chrome/m156)
    #[must_use]
    pub fn make_from_buffer(
        buffer: &mut ReadBuffer<'_>,
        client: Option<&dyn TypefaceIdTranslator>,
    ) -> Option<GlyphVector> {
        let promise = StrikePromise::make_from_buffer(buffer, client, StrikeCache::global());
        if !buffer.validate(promise.is_some()) {
            return None;
        }
        let promise = promise?;

        let glyph_count = buffer.read32();
        // Since the glyph count can never be zero. There was a buffer reading problem.
        if !buffer.validate(glyph_count > 0) {
            return None;
        }

        // Make sure we won't overflow the glyphCount math or allocation below.
        if !buffer.validate(will_count_fit_glyph_bytes(glyph_count)) {
            return None;
        }

        // Check for enough bytes to populate the packedGlyphID array. If not enough something has
        // gone wrong.
        let count = usize::try_from(glyph_count).ok()?;
        if !buffer.validate(count * size_of::<u32>() <= buffer.available()) {
            return None;
        }

        let packed_ids: Vec<PackedGlyphId> = (0..count)
            .map(|_| PackedGlyphId::from_raw(buffer.read_uint()))
            .collect();
        Some(GlyphVector::make(promise, &packed_ids))
    }

    /// `flatten(buffer)`: writes the strike's descriptor, the glyph count and the packed ids.
    ///
    /// # Panics
    /// If the glyph count does not fit an `int32_t`.
    // Port of: src/text/gpu/GlyphVector.cpp#L93-L109 (chrome/m156)
    pub fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        // There should never be a glyph vector with zero glyphs.
        debug_assert!(!self.packed_ids.is_empty());
        self.strike_promise().flatten(buffer);

        // Write out the span of packedGlyphIDs.
        buffer.write_int(i32::try_from(self.packed_ids.len()).expect("SkTo<int32_t>"));
        for id in &self.packed_ids {
            buffer.write_uint(id.value());
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

/// `BagOfBytes::WillCountFit<GlyphBytes>(n)`: whether `n` glyphs (of `sizeof(void*)` bytes each
/// in C++) fit an allocation block.
// Port of: src/text/gpu/SubRunAllocator.h#L119-L123 (chrome/m156)
fn will_count_fit_glyph_bytes(n: i32) -> bool {
    /// `BagOfBytes::kMaxByteSize`: `INT_MAX - k4K`.
    const MAX_BYTE_SIZE: i32 = i32::MAX - (1 << 12);
    /// `GlyphVector_Concepts::kMaxGlyphTypeSize`: `sizeof(void*)` on 64-bit targets.
    const GLYPH_BYTES_SIZE: i32 = 8;
    const MAX_N: i32 = MAX_BYTE_SIZE / GLYPH_BYTES_SIZE;
    (0..MAX_N).contains(&n)
}
