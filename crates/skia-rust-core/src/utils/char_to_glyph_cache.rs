// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/utils/SkCharToGlyphCache.{h,cpp}

//! [`CharToGlyphCache`]: a sorted map from unichar to glyph id, searched by interpolation.
//! Text measuring and text-to-glyph lookups use it to avoid asking the typeface twice.

// The C++ search uses `int` indices (`~index` is the insertion point); a cache never holds
// anywhere near `i32::MAX` entries, so the index casts are exact.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use crate::font_types::GlyphId;
use crate::utf::Unichar;

/// `kSmallCountLimit`: a plain search below this many entries, the slope search above it.
// Port of: src/utils/SkCharToGlyphCache.cpp#L43-L44 (chrome/m156)
const SMALL_COUNT_LIMIT: usize = 16;

/// `kMinCountForSlope`: the slope search needs two real entries plus the two sentinels.
// Port of: src/utils/SkCharToGlyphCache.cpp#L48-L49 (chrome/m156)
const MIN_COUNT_FOR_SLOPE: usize = 4;

/// The unichar-to-glyph cache (`SkCharToGlyphCache`).
///
/// `find_glyph_index` returns the glyph id if the unichar is cached, else `!index`, the place
/// where [`insert_char_and_glyph`](Self::insert_char_and_glyph) puts the new pair.
// Port of: src/utils/SkCharToGlyphCache.h#L19-L63 (chrome/m156)
#[doc(alias = "SkCharToGlyphCache")]
#[derive(Debug, Clone)]
pub struct CharToGlyphCache {
    keys: Vec<Unichar>,
    glyphs: Vec<GlyphId>,
    denom: f64,
}

impl Default for CharToGlyphCache {
    /// `SkCharToGlyphCache()`.
    // Port of: src/utils/SkCharToGlyphCache.cpp#L36-L38 (chrome/m156)
    fn default() -> Self {
        let mut cache = Self {
            keys: Vec::new(),
            glyphs: Vec::new(),
            denom: 0.0,
        };
        cache.reset();
        cache
    }
}

impl CharToGlyphCache {
    /// `SkCharToGlyphCache()`: an empty cache, holding only its sentinels.
    // Port of: src/utils/SkCharToGlyphCache.cpp#L36-L38 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of unichars cached (`count`), not counting the sentinels.
    // Port of: src/utils/SkCharToGlyphCache.h#L26-L28 (chrome/m156)
    #[must_use]
    pub fn count(&self) -> usize {
        self.keys.len()
    }

    /// Forgets all entries, keeping the two sentinels (`reset`).
    // Port of: src/utils/SkCharToGlyphCache.cpp#L40-L52 (chrome/m156)
    pub fn reset(&mut self) {
        self.keys.clear();
        self.glyphs.clear();
        // Add sentinels so the linear searches always stop, in either direction. Neither is a
        // legal unichar, so the glyph id does not matter.
        self.keys.push(i32::MIN); // 0x80000000
        self.glyphs.push(0);
        self.keys.push(i32::MAX); // 0x7FFFFFFF
        self.glyphs.push(0);
        self.denom = 0.0;
    }

    /// `findGlyphIndex`: the glyph id of `c` if cached (zero or more), else `!index`, where the
    /// unichar belongs.
    // Port of: src/utils/SkCharToGlyphCache.cpp#L140-L150 (chrome/m156)
    #[must_use]
    pub fn find_glyph_index(&self, c: Unichar) -> i32 {
        let count = self.keys.len();
        let index = if count <= SMALL_COUNT_LIMIT {
            find_simple(&self.keys, c)
        } else {
            find_with_slope(&self.keys, c, self.denom)
        };
        if index >= 0 {
            return i32::from(self.glyphs[index as usize]);
        }
        index
    }

    /// `insertCharAndGlyph`: inserts a pair at `index` (the `!` of [`find_glyph_index`]).
    // Port of: src/utils/SkCharToGlyphCache.cpp#L152-L170 (chrome/m156)
    pub fn insert_char_and_glyph(&mut self, index: usize, unichar: Unichar, glyph: GlyphId) {
        debug_assert!(index < self.keys.len());
        debug_assert!(unichar < self.keys[index]);
        self.keys.insert(index, unichar);
        self.glyphs.insert(index, glyph);

        // If the first [1] or last [count - 2] entry changed, recompute the slope.
        let count = self.keys.len();
        if count >= MIN_COUNT_FOR_SLOPE && (index == 1 || index == count - 2) {
            debug_assert!(index >= 1 && index <= count - 2);
            self.denom = 1.0 / (f64::from(self.keys[count - 2]) - f64::from(self.keys[1]));
        }
    }

    /// `addCharAndGlyph`: seeds one pair, if it is not cached already.
    // Port of: src/utils/SkCharToGlyphCache.h#L49-L58 (chrome/m156)
    pub fn add_char_and_glyph(&mut self, unichar: Unichar, glyph: GlyphId) {
        let index = self.find_glyph_index(unichar);
        if index >= 0 {
            debug_assert_eq!(index, i32::from(glyph));
        } else {
            self.insert_char_and_glyph(!index as usize, unichar, glyph);
        }
    }
}

/// `find_simple`: a linear search from the front. Returns the index of `value`, or `!index` where
/// it would go.
// Port of: src/utils/SkCharToGlyphCache.cpp#L54-L64 (chrome/m156)
fn find_simple(base: &[Unichar], value: Unichar) -> i32 {
    let mut index = 0usize;
    loop {
        if value <= base[index] {
            return if value < base[index] {
                !(index as i32)
            } else {
                index as i32
            };
        }
        index += 1;
    }
}

/// `find_with_slope`: guesses the index from the slope of the first and last real entries, then
/// searches from the guess.
// Port of: src/utils/SkCharToGlyphCache.cpp#L66-L107 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // the cache size is far below 2^52, exact in f64
fn find_with_slope(base: &[Unichar], value: Unichar, denom: f64) -> i32 {
    let count = base.len();
    debug_assert!(count >= MIN_COUNT_FOR_SLOPE);
    if value <= base[1] {
        let index = 1;
        return if value < base[index] {
            !(index as i32)
        } else {
            index as i32
        };
    }
    if value >= base[count - 2] {
        let index = count - 2;
        return if value > base[index] {
            !((index + 1) as i32)
        } else {
            index as i32
        };
    }
    // Make our guess based on the "slope" of the current values.
    let mut index = 1 + (denom * (count - 2) as f64 * f64::from(value - base[1])) as usize;
    debug_assert!(index >= 1 && index <= count - 2);
    if value >= base[index] {
        loop {
            if value <= base[index] {
                return if value < base[index] {
                    !(index as i32)
                } else {
                    index as i32
                };
            }
            index += 1;
        }
    }
    loop {
        index -= 1;
        if value >= base[index] {
            return if value > base[index] {
                !((index + 1) as i32)
            } else {
                index as i32
            };
        }
    }
}
