// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CharToGlyphCache.cpp (chrome/m156)

#![cfg(test)]
// The C++ test casts the index to unsigned, as written.
#![allow(clippy::cast_sign_loss)]

use skia_rust_core::font_types::GlyphId;
use skia_rust_core::utf::Unichar;
use skia_rust_core::utils::char_to_glyph_cache::CharToGlyphCache;

use crate::{def_test, reporter_assert};

/// `hash_to_glyph`: a deterministic glyph id for a value, spread over the id range.
// Port of: tests/CharToGlyphCache.cpp#L23-L25 (chrome/m156)
fn hash_to_glyph(value: u32) -> GlyphId {
    (((value >> 16) ^ value) & 0xFFFF) as GlyphId
}

/// `UnicharGen`: yields `step`, `2 * step`, ... as unichars.
// Port of: tests/CharToGlyphCache.cpp#L27-L40 (chrome/m156)
struct UnicharGen {
    u: Unichar,
    step: Unichar,
}

impl UnicharGen {
    fn new(step: Unichar) -> Self {
        Self { u: 0, step }
    }

    fn next(&mut self) -> Unichar {
        self.u += self.step;
        self.u
    }
}

// Port of: tests/CharToGlyphCache.cpp#L30-L57 (chrome/m156)
def_test!(chartoglyph_cache, |reporter| {
    let mut cache = CharToGlyphCache::new();
    let step = 3;

    let mut unichars = UnicharGen::new(step);
    for i in 0..500 {
        let mut c = unichars.next();
        let mut glyph = hash_to_glyph(c as u32);

        let mut index = cache.find_glyph_index(c);
        if index >= 0 {
            index = cache.find_glyph_index(c);
        }
        reporter_assert!(reporter, index < 0);
        cache.insert_char_and_glyph(!index as usize, c, glyph);

        let mut gen2 = UnicharGen::new(step);
        for _ in 0..=i {
            c = gen2.next();
            glyph = hash_to_glyph(c as u32);
            index = cache.find_glyph_index(c);
            if (index as u32) != u32::from(glyph) {
                index = cache.find_glyph_index(c);
            }
            reporter_assert!(reporter, (index as u32) == u32::from(glyph));
        }
    }
});
