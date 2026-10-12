// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skunicode/tests/SkUnicodeTest.cpp (chrome/m156)

#![cfg(test)]

use std::sync::Arc;

use skia_rust_unicode::unicode::{
    BidiLevel, BidiRegion, CodeUnitFlags, TextDirection, Unicode, convert_utf16_to_utf8,
};
use skia_rust_unicode::{unicode_client, unicode_icu};

use crate::{def_test, reporter_assert};

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L32-L68 (chrome/m156)
/// The implementations `DEF_TEST_UNICODES` and `DEF_TEST_ICU_UNICODES` run a test on:
/// `SkUnicodes::ICU::Make()` and `SkUnicodes::ICU4X::Make()` (the oracle build has no libgrapheme).
/// Skia registers one test per implementation (`<name>ICU`, `<name>ICU4X`); the port runs both in
/// the one test the manifest names. Both are the ICU4X-backed implementation here
/// (`docs/design/modules.md` §3.2).
fn icu_unicodes() -> [Option<Arc<dyn Unicode>>; 2] {
    [unicode_icu::make(), unicode_icu::make_icu4x()]
}

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

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L88-L101 (chrome/m156)
// `UNIX_ONLY_TEST` in Skia; the ICU4X-backed implementation has no platform code, so the port
// runs everywhere (the same assertions).
def_test!(SkUnicode_Compiled_Native, |reporter| {
    let icu = unicode_icu::make();
    reporter_assert!(reporter, icu.is_some());
    let Some(icu) = icu else {
        return;
    };
    let text: Vec<u16> = "\u{f2008}".encode_utf16().collect();
    let utf8 = convert_utf16_to_utf8(&text);
    let mut utf8 = utf8.into_bytes();
    let mut results: Vec<CodeUnitFlags> = Vec::new();
    icu.compute_code_unit_flags_utf8(&mut utf8, false, &mut results);
    for flag in results {
        reporter_assert!(reporter, !flag.has_part_of_white_space_break_flag());
    }
});

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L119-L135 (chrome/m156)
def_test!(SkUnicode_Compiled_GetSentences, |reporter| {
    let icu = unicode_icu::make();
    reporter_assert!(reporter, icu.is_some());
    let Some(icu) = icu else {
        return;
    };
    let text = "Hello world! Hello world? Hello world... Not a sentence end: 3.1415926";
    let expected: Vec<usize> = vec![0, 13, 26, 41, 70];
    let mut results = Vec::new();
    let result = icu.get_sentences(text.as_bytes(), None, &mut results);
    reporter_assert!(reporter, result);
    reporter_assert!(reporter, results.len() == expected.len());
    for (result, expected) in results.iter().zip(&expected) {
        reporter_assert!(reporter, result == expected);
    }
});

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L143-L158 (chrome/m156)
def_test!(SkUnicode_GetBidiRegionsLTR, |reporter| {
    for unicode in icu_unicodes() {
        let Some(unicode) = unicode else {
            return;
        };
        let text = "1 22 333 4444 55555 666666 7777777";
        let mut results: Vec<BidiRegion> = Vec::new();
        let result = unicode.get_bidi_regions(text.as_bytes(), TextDirection::Ltr, &mut results);
        reporter_assert!(reporter, result);
        reporter_assert!(reporter, results.len() == 1);
        reporter_assert!(
            reporter,
            results[0].start == 0 && results[0].end == text.len() && results[0].level == 0
        );
    }
});

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L160-L175 (chrome/m156)
def_test!(SkUnicode_GetBidiRegionsRTL, |reporter| {
    for unicode in icu_unicodes() {
        let Some(unicode) = unicode else {
            return;
        };
        let text = "الهيمنة على العالم عبارة قبيحة ، أفضل أن أسميها تحسين العالم.";
        let mut results: Vec<BidiRegion> = Vec::new();
        let result = unicode.get_bidi_regions(text.as_bytes(), TextDirection::Rtl, &mut results);
        reporter_assert!(reporter, result);
        reporter_assert!(reporter, results.len() == 1);
        reporter_assert!(
            reporter,
            results[0].start == 0 && results[0].end == text.len() && results[0].level == 1
        );
    }
});

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L177-L210 (chrome/m156)
def_test!(SkUnicode_GetBidiRegionsMix1, |reporter| {
    for unicode in icu_unicodes() {
        let Some(unicode) = unicode else {
            return;
        };
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
    }
});

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L212-L235 (chrome/m156)
def_test!(SkUnicode_GetBidiRegionsMix2, |reporter| {
    for unicode in icu_unicodes() {
        let Some(unicode) = unicode else {
            return;
        };
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
    }
});

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L237-L249 (chrome/m156)
// Currently, libgrapheme uses different default rules and produces slightly
// different results; it does not matter for text shaping
def_test!(SkUnicode_ToUpper, |reporter| {
    for unicode in icu_unicodes() {
        let Some(unicode) = unicode else {
            return;
        };
        let lower = "abcdefghijklmnopqrstuvwxyz";
        let upper = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let icu_result1 = unicode.to_upper(lower);
        reporter_assert!(reporter, icu_result1 == upper);
        let icu_result2 = unicode.to_upper(upper);
        reporter_assert!(reporter, icu_result2 == upper);
    }
});

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L251-L284 (chrome/m156)
def_test!(SkUnicode_ComputeCodeUnitFlags, |reporter| {
    for unicode in icu_unicodes() {
        let Some(unicode) = unicode else {
            return;
        };
        //SkString text("World domination is such an ugly phrase - I prefer to call it world optimisation");
        let mut text = b"1\n22 333 4444 55555 666666 7777777".to_vec();
        let text_size = text.len();
        // 4 8 13 19 24
        let mut results: Vec<CodeUnitFlags> = Vec::new();
        let result = unicode.compute_code_unit_flags_utf8(
            &mut text,
            /*replaceTabs=*/ true,
            &mut results,
        );
        reporter_assert!(reporter, result);
        reporter_assert!(reporter, results.len() == text_size + 1);
        for (i, &flags) in results.iter().enumerate() {
            let mut expected = CodeUnitFlags::GRAPHEME_START;
            if i == 1 {
                expected |= CodeUnitFlags::CONTROL;
            }
            if i == 2 {
                expected |= CodeUnitFlags::HARD_LINE_BREAK_BEFORE;
            }
            if i == 1 || i == 4 || i == 8 || i == 13 || i == 19 || i == 26 {
                expected |= CodeUnitFlags::PART_OF_WHITE_SPACE_BREAK;
                expected |= CodeUnitFlags::PART_OF_INTRA_WORD_BREAK;
            }
            if i == 0 || i == 2 || i == 5 || i == 9 || i == 14 || i == 20 || i == 27 || i == 34 {
                expected |= CodeUnitFlags::SOFT_LINE_BREAK_BEFORE;
            }
            reporter_assert!(reporter, flags == expected);
        }
    }
});

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L286-L302 (chrome/m156)
def_test!(SkUnicode_ReorderVisual, |reporter| {
    for unicode in icu_unicodes() {
        let Some(unicode) = unicode else {
            return;
        };
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
    }
});

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L304-L313 (chrome/m156)
fn sk_unicode_emoji(icu: &dyn Unicode, reporter: &mut crate::Reporter) {
    let emojis = "😄😁😆😅😂🤣";
    let not_emojis = "満毎行昼本可";
    for e in emojis.chars() {
        reporter_assert!(reporter, icu.is_emoji(e as i32));
    }
    for n in not_emojis.chars() {
        reporter_assert!(reporter, !icu.is_emoji(n as i32));
    }
}

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L315-L324 (chrome/m156)
def_test!(SkUnicode_Compiled_Emoji, |reporter| {
    let icu = unicode_icu::make();
    reporter_assert!(reporter, icu.is_some());
    let Some(icu) = icu else {
        return;
    };
    sk_unicode_emoji(icu.as_ref(), reporter);
});

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L326-L335 (chrome/m156)
def_test!(SkUnicode_ICU4X_Emoji, |reporter| {
    let icu = unicode_icu::make_icu4x();
    reporter_assert!(reporter, icu.is_some());
    let Some(icu) = icu else {
        return;
    };
    sk_unicode_emoji(icu.as_ref(), reporter);
});

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L337-L346 (chrome/m156)
fn sk_unicode_ideographic(icu: &dyn Unicode, reporter: &mut crate::Reporter) {
    let ideographic = "満毎行昼本可";
    let not_ideographic = "😄😁😆😅😂🤣";
    for i in ideographic.chars() {
        reporter_assert!(reporter, icu.is_ideographic(i as i32));
    }
    for n in not_ideographic.chars() {
        reporter_assert!(reporter, !icu.is_ideographic(n as i32));
    }
}

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L348-L357 (chrome/m156)
def_test!(SkUnicode_Compiled_Ideographic, |reporter| {
    let icu = unicode_icu::make();
    reporter_assert!(reporter, icu.is_some());
    let Some(icu) = icu else {
        return;
    };
    sk_unicode_ideographic(icu.as_ref(), reporter);
});

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L359-L368 (chrome/m156)
def_test!(SkUnicode_ICU4X_Ideographic, |reporter| {
    let icu = unicode_icu::make_icu4x();
    reporter_assert!(reporter, icu.is_some());
    let Some(icu) = icu else {
        return;
    };
    sk_unicode_ideographic(icu.as_ref(), reporter);
});
