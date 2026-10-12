// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skunicode/tests/SkUnicodeTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_unicode::icu_bidi;
use skia_rust_unicode::unicode::{
    BidiDirection, BidiIterator, BidiLevel, BidiRegion, BreakIterator, BreakType, CodeUnitFlags,
    Position, TextDirection, Unicode, convert_utf16_to_utf8,
};
use skia_rust_unicode::unicode_client;

use crate::{def_test, reporter_assert};

// Stands in for `SkUnicodes::ICU::Make()` until milestone M2 (`docs/design/modules.md` §9)
// lands the ICU-backed `SkUnicode`. Skia's `DEF_TEST_ICU_UNICODES` tests run on that object,
// and each of the bidi tests below calls only `getBidiRegions` or `reorderVisual`. Those two
// answer from `icu_bidi`, which is the bidi engine of the ICU implementation. Every other method
// panics, so a test that reached one would fail loudly instead of passing.
struct BidiOnlyUnicode;

impl Unicode for BidiOnlyUnicode {
    fn to_upper(&self, _s: &str) -> String {
        unimplemented!("needs the ICU implementation (M2)")
    }

    fn to_upper_with_locale(&self, _s: &str, _locale: Option<&str>) -> String {
        unimplemented!("needs the ICU implementation (M2)")
    }

    fn make_bidi_iterator_utf16(
        &self,
        text: &[u16],
        dir: BidiDirection,
    ) -> Option<Box<dyn BidiIterator>> {
        icu_bidi::make_bidi_iterator_utf16(text, dir)
    }

    fn make_bidi_iterator_utf8(
        &self,
        text: &[u8],
        dir: BidiDirection,
    ) -> Option<Box<dyn BidiIterator>> {
        icu_bidi::make_bidi_iterator_utf8(text, dir)
    }

    fn make_break_iterator_with_locale(
        &self,
        _locale: &str,
        _break_type: BreakType,
    ) -> Option<Box<dyn BreakIterator + '_>> {
        unimplemented!("needs the ICU implementation (M2)")
    }

    fn make_break_iterator(&self, _break_type: BreakType) -> Option<Box<dyn BreakIterator + '_>> {
        unimplemented!("needs the ICU implementation (M2)")
    }

    fn get_bidi_regions(
        &self,
        utf8: &[u8],
        dir: TextDirection,
        results: &mut Vec<BidiRegion>,
    ) -> bool {
        icu_bidi::extract_bidi(utf8, dir, results)
    }

    fn get_words(&self, _utf8: &[u8], _locale: Option<&str>, _results: &mut Vec<Position>) -> bool {
        unimplemented!("needs the ICU implementation (M2)")
    }

    fn get_utf8_words(
        &self,
        _utf8: &[u8],
        _locale: Option<&str>,
        _results: &mut Vec<Position>,
    ) -> bool {
        unimplemented!("needs the ICU implementation (M2)")
    }

    fn get_sentences(
        &self,
        _utf8: &[u8],
        _locale: Option<&str>,
        _results: &mut Vec<Position>,
    ) -> bool {
        unimplemented!("needs the ICU implementation (M2)")
    }

    fn compute_code_unit_flags_utf8(
        &self,
        _utf8: &mut [u8],
        _replace_tabs: bool,
        _results: &mut Vec<CodeUnitFlags>,
    ) -> bool {
        unimplemented!("needs the ICU implementation (M2)")
    }

    fn compute_code_unit_flags_utf16(
        &self,
        _utf16: &[u16],
        _replace_tabs: bool,
        _results: &mut Vec<CodeUnitFlags>,
    ) -> bool {
        unimplemented!("needs the ICU implementation (M2)")
    }

    fn reorder_visual(&self, run_levels: &[BidiLevel], logical_from_visual: &mut [i32]) {
        icu_bidi::reorder_visual(run_levels, logical_from_visual);
    }
}

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L143-L158 (chrome/m156)
def_test!(SkUnicode_GetBidiRegionsLTR, |reporter| {
    let unicode = BidiOnlyUnicode;
    let text = "1 22 333 4444 55555 666666 7777777";
    let mut results: Vec<BidiRegion> = Vec::new();
    let result = unicode.get_bidi_regions(text.as_bytes(), TextDirection::Ltr, &mut results);
    reporter_assert!(reporter, result);
    reporter_assert!(reporter, results.len() == 1);
    reporter_assert!(
        reporter,
        results[0].start == 0 && results[0].end == text.len() && results[0].level == 0
    );
});

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L160-L175 (chrome/m156)
def_test!(SkUnicode_GetBidiRegionsRTL, |reporter| {
    let unicode = BidiOnlyUnicode;
    let text = "الهيمنة على العالم عبارة قبيحة ، أفضل أن أسميها تحسين العالم.";
    let mut results: Vec<BidiRegion> = Vec::new();
    let result = unicode.get_bidi_regions(text.as_bytes(), TextDirection::Rtl, &mut results);
    reporter_assert!(reporter, result);
    reporter_assert!(reporter, results.len() == 1);
    reporter_assert!(
        reporter,
        results[0].start == 0 && results[0].end == text.len() && results[0].level == 1
    );
});

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L177-L210 (chrome/m156)
def_test!(SkUnicode_GetBidiRegionsMix1, |reporter| {
    let unicode = BidiOnlyUnicode;
    // Spaces become Arabic (RTL) but numbers remain English (LTR)
    let text = "1 22 333 4444 55555 666666 7777777";
    let expected: Vec<BidiRegion> = vec![
        BidiRegion::new(0, 1, 2),
        BidiRegion::new(1, 2, 1),
        BidiRegion::new(2, 4, 2),
        BidiRegion::new(4, 5, 1),
        BidiRegion::new(5, 8, 2),
        BidiRegion::new(8, 9, 1),
        BidiRegion::new(9, 13, 2),
        BidiRegion::new(13, 14, 1),
        BidiRegion::new(14, 19, 2),
        BidiRegion::new(19, 20, 1),
        BidiRegion::new(20, 26, 2),
        BidiRegion::new(26, 27, 1),
        BidiRegion::new(27, 34, 2),
    ];
    let mut results: Vec<BidiRegion> = Vec::new();
    let result = unicode.get_bidi_regions(text.as_bytes(), TextDirection::Rtl, &mut results);
    reporter_assert!(reporter, result);
    reporter_assert!(reporter, results.len() == expected.len());
    for (result, expected) in results.iter().zip(&expected) {
        reporter_assert!(
            reporter,
            result.start == expected.start
                && result.end == expected.end
                && result.level == expected.level
        );
    }
});

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L212-L235 (chrome/m156)
def_test!(SkUnicode_GetBidiRegionsMix2, |reporter| {
    let unicode = BidiOnlyUnicode;
    // Few Russian/English words (ЛТР) in the mix
    let text = "World ЛТР Domination هي عبارة قبيحة ، أفضل أن أسميها World ЛТР Optimization.";
    let expected: Vec<BidiRegion> = vec![
        BidiRegion::new(0, 24, 0),
        BidiRegion::new(24, 80, 1),
        BidiRegion::new(80, 107, 0),
    ];
    let mut results: Vec<BidiRegion> = Vec::new();
    let result = unicode.get_bidi_regions(text.as_bytes(), TextDirection::Ltr, &mut results);
    reporter_assert!(reporter, result);
    reporter_assert!(reporter, results.len() == expected.len());
    for (result, expected) in results.iter().zip(&expected) {
        reporter_assert!(
            reporter,
            result.start == expected.start
                && result.end == expected.end
                && result.level == expected.level
        );
    }
});

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L286-L302 (chrome/m156)
def_test!(SkUnicode_ReorderVisual, |reporter| {
    let unicode = BidiOnlyUnicode;
    let mut reorder = |levels: &[BidiLevel], expected: &[i32]| {
        let mut logical_order = vec![0_i32; levels.len()];
        unicode.reorder_visual(levels, &mut logical_order);
        for i in 0..levels.len() {
            reporter_assert!(reporter, expected[i] == logical_order[i]);
        }
    };
    reorder(&[], &[]);
    reorder(&[0], &[0]);
    reorder(&[1], &[0]);
    reorder(&[0, 1, 0, 1], &[0, 1, 2, 3]);
});

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L73-L84 (chrome/m156)
// Skia runs this only on Unix (`UNIX_ONLY_TEST`). The client implementation is pure Rust with no
// platform code, so the port runs everywhere: a superset, with the same assertions.
def_test!(SkUnicode_Client, |reporter| {
    // u"\U000f2008", the code point U+F2008 as a surrogate pair.
    let text: Vec<u16> = "\u{f2008}".encode_utf16().collect();
    let utf8 = convert_utf16_to_utf8(&text);
    let utf8 = utf8.as_bytes();
    let client = unicode_client::make(utf8, Vec::new(), Vec::new(), Vec::new());
    // The C++ test passes one buffer to Make and to computeCodeUnitFlags. computeCodeUnitFlags
    // may rewrite tabs in its buffer, and Rust cannot also borrow that buffer as the client's
    // text, so the flags are computed on a copy. This test has no tabs, so the copy is exact.
    let mut flags_input = utf8.to_vec();
    let mut results: Vec<CodeUnitFlags> = Vec::new();
    client.compute_code_unit_flags_utf8(&mut flags_input, false, &mut results);

    for flag in results {
        reporter_assert!(reporter, !flag.has_part_of_white_space_break_flag());
    }
});
