// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skunicode/src/SkUnicode_icu.cpp,
// modules/skunicode/src/SkUnicode_icupriv.h, modules/skunicode/include/SkUnicode_icu.h.

//! The ICU implementation of `SkUnicode` (`SkUnicodes::ICU::Make`), over [`crate::icu_shim`] (the
//! `SKICU_EMIT_FUNCS` functions on ICU4X) and the bidi engine of [`crate::icu_bidi`].
//!
//! `SkUnicode_icupriv.h`'s function table (`SkICULib`, `SkLoadICULib`, `SkGetICULib`) has no
//! counterpart: the shim's functions are called directly, and there is no library to load, so
//! [`make`] always succeeds. `SkUnicodes::ICU4X::Make` (`SkUnicode_icu4x.cpp`) is the same
//! implementation here (`docs/design/modules.md` §3.2): [`make_icu4x`].
//!
//! Not ported: the de-duplication of `SkIcuBreakIteratorCache` by the iterator's actual locale
//! (`ubrk_getLocaleByType`, which the shim does not have) and the purge of its cached iterators
//! when more than four are alive. Both only save memory; the iterators are the same.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use skia_rust_core::utf::{self, Unichar};

use crate::icu_bidi;
use crate::icu_shim::{
    UBRK_DONE, UBreakIterator, UBreakIteratorType, UProperty, status, u_get_int_property_value,
    u_has_binary_property, u_is_whitespace, u_iscntrl, u_isspace, u_lb, u_str_to_upper,
    uloc_for_language_tag, uloc_get_default,
};
use crate::unicode::{
    BidiDirection, BidiIterator, BidiLevel, BidiRegion, BreakIterator, BreakPosition, BreakStatus,
    BreakType, CodeUnitFlags, LineBreakType, Position, TextDirection, Unicode,
    convert_utf8_to_utf16, convert_utf16_to_utf8, extract_utf_conversion_mapping,
    for_each_codepoint_utf16,
};

// Port of: modules/skunicode/src/SkUnicode_icu.cpp#L80-L84 (chrome/m156)
/// `utf8_next`: replaces invalid UTF-8 sequences with U+FFFD.
fn utf8_next(ptr: &mut &[u8]) -> Unichar {
    let val = utf::next_utf8(ptr);
    if val < 0 { 0xFFFD } else { val }
}

// Port of: modules/skunicode/src/SkUnicode_icu.cpp#L86-L96 (chrome/m156)
/// `convertType`.
fn convert_type(break_type: BreakType) -> UBreakIteratorType {
    match break_type {
        BreakType::Lines => UBreakIteratorType::Line,
        BreakType::Graphemes => UBreakIteratorType::Character,
        BreakType::Words => UBreakIteratorType::Word,
        BreakType::Sentences => UBreakIteratorType::Sentence,
    }
}

// Port of: modules/skunicode/src/SkUnicode_icu.cpp#L98-L146 (chrome/m156)
/// `SkBreakIterator_icu`.
#[derive(Debug)]
struct BreakIteratorIcu {
    break_iterator: UBreakIterator,
    /// `fLastResult`.
    last_result: BreakPosition,
}

impl BreakIteratorIcu {
    fn new(iter: UBreakIterator) -> Self {
        Self {
            break_iterator: iter,
            last_result: 0,
        }
    }
}

impl BreakIterator for BreakIteratorIcu {
    fn first(&mut self) -> BreakPosition {
        self.last_result = self.break_iterator.first();
        self.last_result
    }

    fn current(&mut self) -> BreakPosition {
        self.last_result = self.break_iterator.current();
        self.last_result
    }

    fn next(&mut self) -> BreakPosition {
        self.last_result = self.break_iterator.next();
        self.last_result
    }

    fn status(&mut self) -> BreakStatus {
        self.break_iterator.get_rule_status()
    }

    fn is_done(&mut self) -> bool {
        self.last_result == UBRK_DONE
    }

    // `utext_openUTF8` and `ubrk_setUText` do not fail on a text Rust can hold.
    fn set_text_utf8(&mut self, utf8: &[u8]) -> bool {
        self.break_iterator.set_text_utf8(utf8);
        self.last_result = 0;
        true
    }

    fn set_text_utf16(&mut self, utf16: &[u16]) -> bool {
        self.break_iterator.set_text_utf16(utf16);
        self.last_result = 0;
        true
    }
}

// Port of: modules/skunicode/src/SkUnicode_icu.cpp#L148-L333 (chrome/m156)
/// `SkIcuBreakIteratorCache`: one opened break iterator per (type, ICU locale), cloned for each
/// request. Skia keeps one process-wide instance; here each `SkUnicode` has its own.
#[derive(Debug, Default)]
struct IcuBreakIteratorCache {
    request_cache: Mutex<HashMap<(BreakType, String), UBreakIterator>>,
}

impl IcuBreakIteratorCache {
    // Port of: modules/skunicode/src/SkUnicode_icu.cpp#L231-L331 (chrome/m156)
    fn make_break_iterator(
        &self,
        break_type: BreakType,
        bcp47: Option<&str>,
    ) -> Option<UBreakIterator> {
        // Get ICU locale for BCP47 langtag.
        let mut locale_id = None;
        if let Some(bcp47) = bcp47 {
            let id = uloc_for_language_tag(bcp47).locale_id;
            if !id.is_empty() {
                locale_id = Some(id);
            }
        }
        let locale_id = locale_id.unwrap_or_else(|| uloc_get_default().to_owned());

        let request = (break_type, locale_id);
        // A poisoned lock only means another thread panicked while holding the cache.
        let mut cache = self
            .request_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // See if this request is already in the cache.
        if let Some(existing) = cache.get(&request) {
            return Some(existing.clone());
        }
        // This request was not in the cache, create an iterator. A failure is `nullptr`.
        let new_iter = UBreakIterator::open(convert_type(break_type), Some(&request.1)).ok()?;
        // `purgeIfNeeded`: if there are too many requests remove some.
        if cache.len() > 100 {
            cache.clear();
        }
        let clone = new_iter.clone();
        cache.insert(request, new_iter);
        Some(clone)
    }
}

// Port of: modules/skunicode/src/SkUnicode_icu.cpp#L335-L690 (chrome/m156)
/// `SkUnicode_icu`.
#[derive(Debug, Default)]
struct UnicodeIcu {
    cache: IcuBreakIteratorCache,
}

impl UnicodeIcu {
    // Port of: modules/skunicode/src/SkUnicode_icu.cpp#L337-L370 (chrome/m156)
    /// `extractWords`: the word boundaries of UTF-16 text.
    fn extract_words(
        &self,
        utf16: &[u16],
        locale: Option<&str>,
        words: &mut Vec<Position>,
    ) -> bool {
        let Some(mut iterator) = self.cache.make_break_iterator(BreakType::Words, locale) else {
            return false;
        };
        iterator.set_text_utf16(utf16);
        // Get the words.
        let mut pos = iterator.first();
        while pos != UBRK_DONE {
            words.push(usize::try_from(pos).expect("boundaries are not negative"));
            pos = iterator.next();
        }
        true
    }

    // Port of: modules/skunicode/src/SkUnicode_icu.cpp#L372-L420 (chrome/m156)
    /// `extractPositions`: calls `set_break(position, status)` for each boundary of the UTF-8 text.
    fn extract_positions(
        &self,
        utf8: &[u8],
        break_type: BreakType,
        locale: Option<&str>,
        mut set_break: impl FnMut(usize, i32),
    ) -> bool {
        let Some(mut iterator) = self.cache.make_break_iterator(break_type, locale) else {
            return false;
        };
        iterator.set_text_utf8(utf8);
        let mut pos = iterator.first();
        while pos != UBRK_DONE {
            let s = if break_type == BreakType::Lines {
                status::UBRK_LINE_SOFT
            } else {
                iterator.get_rule_status()
            };
            set_break(
                usize::try_from(pos).expect("boundaries are not negative"),
                s,
            );
            pos = iterator.next();
        }

        if break_type == BreakType::Lines {
            // This is a workaround for https://bugs.chromium.org/p/skia/issues/detail?id=10715
            // (ICU line break iterator does not work correctly on Thai text with new lines)
            // So, we only use the iterator to collect soft line breaks and
            // scan the text for all hard line breaks ourselves
            let mut ch = utf8;
            while !ch.is_empty() {
                let unichar = utf8_next(&mut ch);
                if Self::is_hard_line_break(unichar) {
                    set_break(utf8.len() - ch.len(), status::UBRK_LINE_HARD);
                }
            }
        }
        true
    }

    // Port of: modules/skunicode/src/SkUnicode_icu.cpp#L466-L469 (chrome/m156)
    /// `isHardLineBreak`: `Line_Break` is `LF` or `BK`.
    fn is_hard_line_break(utf8: Unichar) -> bool {
        let property = u_get_int_property_value(utf8, UProperty::LineBreak);
        property == u_lb::LINE_FEED || property == u_lb::MANDATORY_BREAK
    }
}

impl Unicode for UnicodeIcu {
    // Port of: modules/skunicode/src/SkUnicode_icu.cpp#L494-L496 (chrome/m156)
    fn to_upper(&self, s: &str) -> String {
        self.to_upper_with_locale(s, None)
    }

    // Port of: modules/skunicode/src/SkUnicode_icu.cpp#L498-L520 (chrome/m156)
    fn to_upper_with_locale(&self, s: &str, locale: Option<&str>) -> String {
        // Convert to UTF16 since that's what ICU wants.
        let str16 = convert_utf8_to_utf16(s.as_bytes());
        let upper16 = u_str_to_upper(&str16, locale);
        // `upper16len <= 0`.
        if upper16.is_empty() {
            return String::new();
        }
        // ... and back to utf8 'cause that's what we want.
        convert_utf16_to_utf8(&upper16)
    }

    // Port of: modules/skunicode/src/SkUnicode_icu.cpp#L422-L464 (chrome/m156)
    fn is_control(&self, utf8: Unichar) -> bool {
        u_iscntrl(utf8)
    }

    fn is_whitespace(&self, utf8: Unichar) -> bool {
        u_is_whitespace(utf8)
    }

    fn is_space(&self, utf8: Unichar) -> bool {
        u_isspace(utf8)
    }

    fn is_hard_break(&self, utf8: Unichar) -> bool {
        Self::is_hard_line_break(utf8)
    }

    fn is_emoji(&self, unichar: Unichar) -> bool {
        u_has_binary_property(unichar, UProperty::Emoji)
    }

    fn is_emoji_component(&self, unichar: Unichar) -> bool {
        u_has_binary_property(unichar, UProperty::EmojiComponent)
    }

    fn is_emoji_modifier_base(&self, unichar: Unichar) -> bool {
        u_has_binary_property(unichar, UProperty::EmojiModifierBase)
    }

    fn is_emoji_modifier(&self, unichar: Unichar) -> bool {
        u_has_binary_property(unichar, UProperty::EmojiModifier)
    }

    fn is_regional_indicator(&self, unichar: Unichar) -> bool {
        u_has_binary_property(unichar, UProperty::RegionalIndicator)
    }

    fn is_ideographic(&self, unichar: Unichar) -> bool {
        u_has_binary_property(unichar, UProperty::Ideographic)
    }

    fn is_tabulation(&self, utf8: Unichar) -> bool {
        utf8 == Unichar::from(b'\t')
    }

    // Port of: modules/skunicode/src/SkUnicode_icu.cpp#L473-L481 (chrome/m156)
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

    // Port of: modules/skunicode/src/SkUnicode_icu.cpp#L482-L492 (chrome/m156)
    fn make_break_iterator_with_locale(
        &self,
        locale: &str,
        break_type: BreakType,
    ) -> Option<Box<dyn BreakIterator + '_>> {
        let iterator = self.cache.make_break_iterator(break_type, Some(locale))?;
        Some(Box::new(BreakIteratorIcu::new(iterator)))
    }

    fn make_break_iterator(&self, break_type: BreakType) -> Option<Box<dyn BreakIterator + '_>> {
        self.make_break_iterator_with_locale(uloc_get_default(), break_type)
    }

    // Port of: modules/skunicode/src/SkUnicode_icu.cpp#L522-L527 (chrome/m156)
    fn get_bidi_regions(
        &self,
        utf8: &[u8],
        dir: TextDirection,
        results: &mut Vec<BidiRegion>,
    ) -> bool {
        icu_bidi::extract_bidi(utf8, dir, results)
    }

    // Port of: modules/skunicode/src/SkUnicode_icu.cpp#L529-L536 (chrome/m156)
    fn get_words(&self, utf8: &[u8], locale: Option<&str>, results: &mut Vec<Position>) -> bool {
        // Convert to UTF16 since we want the results in utf16
        let utf16 = convert_utf8_to_utf16(utf8);
        self.extract_words(&utf16, locale, results)
    }

    // Port of: modules/skunicode/src/SkUnicode_icu.cpp#L538-L560 (chrome/m156)
    fn get_utf8_words(
        &self,
        utf8: &[u8],
        locale: Option<&str>,
        results: &mut Vec<Position>,
    ) -> bool {
        // Convert to UTF16 since we want the results in utf16
        let utf16 = convert_utf8_to_utf16(utf8);
        let mut utf16_results = Vec::new();
        if !self.extract_words(&utf16, locale, &mut utf16_results) {
            return false;
        }
        let mut mapping = Vec::new();
        extract_utf_conversion_mapping(utf8, |index| mapping.push(index), |_| {});
        for i16 in utf16_results {
            results.push(mapping[i16]);
        }
        true
    }

    // Port of: modules/skunicode/src/SkUnicode_icu.cpp#L562-L572 (chrome/m156)
    // Skia passes no locale (`nullptr`), whatever the caller gives.
    fn get_sentences(
        &self,
        utf8: &[u8],
        _locale: Option<&str>,
        results: &mut Vec<Position>,
    ) -> bool {
        self.extract_positions(utf8, BreakType::Sentences, None, |pos, _status| {
            results.push(pos);
        });
        true
    }

    // Port of: modules/skunicode/src/SkUnicode_icu.cpp#L574-L622 (chrome/m156)
    fn compute_code_unit_flags_utf8(
        &self,
        utf8: &mut [u8],
        replace_tabs: bool,
        results: &mut Vec<CodeUnitFlags>,
    ) -> bool {
        results.clear();
        results.resize(utf8.len() + 1, CodeUnitFlags::NO_CODE_UNIT_FLAG);

        // TODO: locale (as in Skia).
        self.extract_positions(utf8, BreakType::Lines, None, |pos, status| {
            results[pos] |= if status == status::UBRK_LINE_HARD {
                CodeUnitFlags::HARD_LINE_BREAK_BEFORE
            } else {
                CodeUnitFlags::SOFT_LINE_BREAK_BEFORE
            };
        });

        self.extract_positions(utf8, BreakType::Graphemes, None, |pos, _status| {
            results[pos] |= CodeUnitFlags::GRAPHEME_START;
        });

        let end = utf8.len();
        let mut current = 0;
        while current < end {
            let before = current;
            let mut rest = &utf8[current..];
            let mut unichar = utf::next_utf8(&mut rest);
            current = end - rest.len();
            if unichar < 0 {
                unichar = 0xFFFD;
            }
            let after = current;
            if replace_tabs && self.is_tabulation(unichar) {
                results[before] |= CodeUnitFlags::TABULATION;
                unichar = Unichar::from(b' ');
                utf8[before] = b' ';
            }
            for flags in &mut results[before..after] {
                if self.is_space(unichar) {
                    *flags |= CodeUnitFlags::PART_OF_INTRA_WORD_BREAK;
                }
                if self.is_whitespace(unichar) {
                    *flags |= CodeUnitFlags::PART_OF_WHITE_SPACE_BREAK;
                }
                if self.is_control(unichar) {
                    *flags |= CodeUnitFlags::CONTROL;
                }
                if self.is_ideographic(unichar) {
                    *flags |= CodeUnitFlags::IDEOGRAPHIC;
                }
            }
        }
        true
    }

    // Port of: modules/skunicode/src/SkUnicode_icu.cpp#L624-L675 (chrome/m156)
    // Skia also writes a space over each tab of the caller's UTF-16 text (`utf16[start] = ' '`);
    // the trait gives the text read-only, so only the flags are computed, which are the same.
    fn compute_code_unit_flags_utf16(
        &self,
        utf16: &[u16],
        replace_tabs: bool,
        results: &mut Vec<CodeUnitFlags>,
    ) -> bool {
        results.clear();
        results.resize(utf16.len() + 1, CodeUnitFlags::NO_CODE_UNIT_FLAG);

        // Get white spaces
        for_each_codepoint_utf16(utf16, |mut unichar, start, end| {
            for flags in &mut results[start..end] {
                if replace_tabs && self.is_tabulation(unichar) {
                    *flags |= CodeUnitFlags::TABULATION;
                    unichar = Unichar::from(b' ');
                }
                if self.is_space(unichar) {
                    *flags |= CodeUnitFlags::PART_OF_INTRA_WORD_BREAK;
                }
                if self.is_whitespace(unichar) {
                    *flags |= CodeUnitFlags::PART_OF_WHITE_SPACE_BREAK;
                }
                if self.is_control(unichar) {
                    *flags |= CodeUnitFlags::CONTROL;
                }
            }
        });
        // Get graphemes
        self.for_each_break(utf16, BreakType::Graphemes, &mut |pos, _status| {
            results[usize::try_from(pos).expect("boundaries are not negative")] |=
                CodeUnitFlags::GRAPHEME_START;
        });
        // Get line breaks
        self.for_each_break(utf16, BreakType::Lines, &mut |pos, status| {
            let pos = usize::try_from(pos).expect("boundaries are not negative");
            if status == LineBreakType::HardLineBreak as BreakStatus {
                // Hard line breaks clears off all the other flags
                // TODO: Treat \n as a formatting mark and do not pass it to SkShaper
                results[pos - 1] = CodeUnitFlags::HARD_LINE_BREAK_BEFORE;
            } else {
                results[pos] |= CodeUnitFlags::SOFT_LINE_BREAK_BEFORE;
            }
        });
        true
    }

    // Port of: modules/skunicode/src/SkUnicode_icu.cpp#L677-L686 (chrome/m156)
    fn reorder_visual(&self, run_levels: &[BidiLevel], logical_from_visual: &mut [i32]) {
        if run_levels.is_empty() {
            // To avoid an assert in unicode
            return;
        }
        icu_bidi::reorder_visual(run_levels, logical_from_visual);
    }
}

// Port of: modules/skunicode/src/SkUnicode_icu.cpp#L692-L708 (chrome/m156)
/// `SkUnicodes::ICU::Make`: the ICU implementation of `SkUnicode`. Never `None` (there is no ICU
/// library to load); the `Option` is Skia's `nullptr` contract.
#[doc(alias = "SkUnicodes::ICU::Make")]
#[must_use]
pub fn make() -> Option<Arc<dyn Unicode>> {
    Some(Arc::new(UnicodeIcu::default()))
}

/// `SkUnicodes::ICU4X::Make`: the same implementation as [`make`] (`docs/design/modules.md` §3.2:
/// the `ICU4X_*` tests run against the one ICU4X-backed implementation).
#[doc(alias = "SkUnicodes::ICU4X::Make")]
#[must_use]
pub fn make_icu4x() -> Option<Arc<dyn Unicode>> {
    make()
}
