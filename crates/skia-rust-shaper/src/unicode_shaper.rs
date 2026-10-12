// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skshaper/src/SkShaper_skunicode.cpp,
// modules/skshaper/include/SkShaper_skunicode.h.

//! `SkShapers::unicode`: the bidi run iterator over a [`Unicode`].

use skia_rust_core::utf::{self, Unichar};
use skia_rust_unicode::unicode::{BidiDirection, BidiIterator, BidiLevel, Unicode};

use crate::run_iterator::{BiDiRunIterator, RunIterator};

// Port of: modules/skshaper/src/SkShaper_skunicode.cpp#L24-L27 (chrome/m156), `utf8_next`.
fn utf8_next(utf8: &[u8], at: &mut usize) -> Unichar {
    let mut rest = &utf8[*at..];
    let val = utf::next_utf8_with_replacement(&mut rest);
    *at = utf8.len() - rest.len();
    val
}

// `SkUTF::ToUTF16 (u)`: the number of UTF-16 code units of a code point.
fn utf16_units(u: Unichar) -> i32 {
    i32::try_from(utf::to_utf16(u, None)).unwrap_or(0)
}

// Port of: modules/skshaper/src/SkShaper_skunicode.cpp#L29-L76 (chrome/m156),
// `SkUnicodeBidiRunIterator`.
struct UnicodeBidiRunIterator {
    bidi: Box<dyn BidiIterator>,
    utf8: Vec<u8>,
    end_of_current_run: usize,
    utf16_logical_position: i32,
    length: i32,
    level: BidiLevel,
}

impl RunIterator for UnicodeBidiRunIterator {
    fn consume(&mut self) {
        debug_assert!(self.utf16_logical_position < self.length);
        let end_position = self.length;
        self.level = self.bidi.get_level_at(self.utf16_logical_position);
        let u = utf8_next(&self.utf8, &mut self.end_of_current_run);
        self.utf16_logical_position += utf16_units(u);
        while self.utf16_logical_position < end_position {
            let level = self.bidi.get_level_at(self.utf16_logical_position);
            if level != self.level {
                break;
            }
            let u = utf8_next(&self.utf8, &mut self.end_of_current_run);

            self.utf16_logical_position += utf16_units(u);
        }
    }

    fn end_of_current_run(&self) -> usize {
        self.end_of_current_run
    }

    fn at_end(&self) -> bool {
        self.utf16_logical_position == self.length
    }
}

impl BiDiRunIterator for UnicodeBidiRunIterator {
    fn current_level(&self) -> u8 {
        self.level
    }
}

// Port of: modules/skshaper/src/SkShaper_skunicode.cpp#L119-L152 (chrome/m156),
// `SkShapers::unicode::BidiRunIterator`.
/// `SkShapers::unicode::BidiRunIterator(unicode, utf8, utf8Bytes, bidiLevel)`: the bidi levels of
/// the text. `None` when the text is not valid UTF-8, is too long, or the bidi iterator cannot be
/// made.
#[must_use]
#[doc(alias = "BidiRunIterator")]
pub fn unicode_bidi_run_iterator(
    unicode: &dyn Unicode,
    utf8: &[u8],
    bidi_level: u8,
) -> Option<Box<dyn BiDiRunIterator>> {
    // ubidi only accepts utf16 (though internally it basically works on utf32 chars).
    // We want an ubidi_setPara(UBiDi*, UText*, UBiDiLevel, UBiDiLevel*, UErrorCode*);
    if i32::try_from(utf8.len()).is_err() {
        return None; // Bidi error: text too long
    }

    let utf16_units = utf::utf8_to_utf16(None, utf8);
    if utf16_units < 0 {
        return None; // Invalid utf8 input
    }

    let mut utf16 = vec![0u16; usize::try_from(utf16_units).ok()?];
    let _ = utf::utf8_to_utf16(Some(&mut utf16), utf8);

    let bidi_dir = if bidi_level.is_multiple_of(2) {
        BidiDirection::Ltr
    } else {
        BidiDirection::Rtl
    };
    let mut bidi = unicode.make_bidi_iterator_utf16(&utf16, bidi_dir)?;
    let length = bidi.get_length();

    Some(Box::new(UnicodeBidiRunIterator {
        bidi,
        utf8: utf8.to_vec(),
        end_of_current_run: 0,
        utf16_logical_position: 0,
        length,
        level: 0, // SkBidiIterator::kLTR
    }))
}
