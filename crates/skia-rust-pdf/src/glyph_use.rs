// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFGlyphUse.h (chrome/m156)

//! `SkPDFGlyphUse`: the set of glyphs of a font that a document used.

use skia_rust_core::font_types::GlyphId;

/// `SkPDFGlyphUse`: a set of the glyph ids in `first_non_zero..=last_glyph`, and glyph 0.
///
/// The ids are stored as codes: glyph 0 is code 0, and glyph `first_non_zero + n` is code
/// `n + 1`. When `first_non_zero` is 1 the codes are the glyph ids.
// Port of: src/pdf/SkPDFGlyphUse.h#L17-L49 (chrome/m156)
#[doc(alias = "SkPDFGlyphUse")]
#[derive(Clone, Debug, Default)]
pub struct PdfGlyphUse {
    /// `fBitSet`: one bit per code.
    bits: Vec<u64>,
    /// The number of bits in `bits` (`SkBitSet::size`).
    size: usize,
    first_non_zero: GlyphId,
    last_glyph: GlyphId,
}

impl PdfGlyphUse {
    /// `SkPDFGlyphUse(firstNonZero, lastGlyph)`.
    ///
    /// # Panics
    ///
    /// Panics where C++ asserts (`firstNonZero >= 1`), and if `last_glyph < first_non_zero`.
    #[must_use]
    pub fn new(first_non_zero: GlyphId, last_glyph: GlyphId) -> Self {
        assert!(first_non_zero >= 1);
        let size = usize::from(last_glyph) - usize::from(first_non_zero) + 2;
        Self {
            bits: vec![0; size.div_ceil(64)],
            size,
            first_non_zero,
            last_glyph,
        }
    }

    /// `firstNonZero`.
    #[must_use]
    pub fn first_non_zero(&self) -> GlyphId {
        self.first_non_zero
    }

    /// `lastGlyph`.
    #[must_use]
    pub fn last_glyph(&self) -> GlyphId {
        self.last_glyph
    }

    /// `toCode`.
    fn to_code(&self, gid: GlyphId) -> usize {
        if gid == 0 || self.first_non_zero == 1 {
            return usize::from(gid);
        }
        debug_assert!(gid >= self.first_non_zero && gid <= self.last_glyph);
        usize::from(gid - self.first_non_zero) + 1
    }

    /// `set`.
    pub fn set(&mut self, gid: GlyphId) {
        let code = self.to_code(gid);
        assert!(code < self.size);
        self.bits[code / 64] |= 1u64 << (code % 64);
    }

    /// `has`.
    #[must_use]
    pub fn has(&self, gid: GlyphId) -> bool {
        let code = self.to_code(gid);
        assert!(code < self.size);
        self.bits[code / 64] & (1u64 << (code % 64)) != 0
    }

    /// `getSetValues`: calls `f` with each glyph id in the set, in increasing order.
    pub fn get_set_values(&self, mut f: impl FnMut(usize)) {
        let offset = usize::from(self.first_non_zero) - 1;
        for (word_index, &word) in self.bits.iter().enumerate() {
            let mut word = word;
            while word != 0 {
                let bit = word.trailing_zeros() as usize;
                word &= word - 1;
                let v = word_index * 64 + bit;
                if self.first_non_zero == 1 || v == 0 {
                    f(v);
                } else {
                    f(v + offset);
                }
            }
        }
    }
}
