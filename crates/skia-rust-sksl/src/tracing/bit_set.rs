// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/utils/SkBitSet.h (the operations the debug trace player uses).

//! `SkBitSet`: a fixed-size set of bit indices.

const CHUNK_BITS: usize = 64;

/// `SkBitSet`: a set of the indices `0..size`.
#[doc(alias = "SkBitSet")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BitSet {
    size: usize,
    chunks: Vec<u64>,
}

impl BitSet {
    /// `SkBitSet(size)`: an empty set of `size` indices.
    // Port of: src/utils/SkBitSet.h#L23-L25 (chrome/m156)
    #[must_use]
    pub fn new(size: usize) -> Self {
        Self {
            size,
            chunks: vec![0; size.div_ceil(CHUNK_BITS)],
        }
    }

    /// `size()`.
    #[must_use]
    pub fn size(&self) -> usize {
        self.size
    }

    /// `set(index)`.
    ///
    /// # Panics
    /// If `index` is not less than the size.
    // Port of: src/utils/SkBitSet.h#L54-L58 (chrome/m156)
    pub fn set(&mut self, index: usize) {
        assert!(index < self.size, "BitSet::set out of range");
        self.chunks[index / CHUNK_BITS] |= 1 << (index % CHUNK_BITS);
    }

    /// `reset(index)`.
    ///
    /// # Panics
    /// If `index` is not less than the size.
    // Port of: src/utils/SkBitSet.h#L67-L71 (chrome/m156)
    pub fn reset(&mut self, index: usize) {
        assert!(index < self.size, "BitSet::reset out of range");
        self.chunks[index / CHUNK_BITS] &= !(1 << (index % CHUNK_BITS));
    }

    /// `reset()`: clears every bit.
    // Port of: src/utils/SkBitSet.h#L73-L78 (chrome/m156)
    pub fn reset_all(&mut self) {
        self.chunks.fill(0);
    }

    /// `test(index)`.
    ///
    /// # Panics
    /// If `index` is not less than the size.
    // Port of: src/utils/SkBitSet.h#L79-L83 (chrome/m156)
    #[must_use]
    pub fn test(&self, index: usize) -> bool {
        assert!(index < self.size, "BitSet::test out of range");
        self.chunks[index / CHUNK_BITS] & (1 << (index % CHUNK_BITS)) != 0
    }

    /// `forEachSetIndex`: the indices of the set bits, in increasing order.
    // Port of: src/utils/SkBitSet.h#L90-L104 (chrome/m156)
    #[must_use]
    pub fn set_indices(&self) -> Vec<usize> {
        (0..self.size).filter(|&index| self.test(index)).collect()
    }
}
