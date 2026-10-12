// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/text/Unicode.cpp (chrome/m156)
//
// A `Unicode` wrapper which suppresses mid-word line breaks.

use std::sync::Arc;

use skia_rust_core::utf::Unichar;
use skia_rust_unicode::unicode::{
    BidiDirection, BidiIterator, BidiLevel, BidiRegion, BreakIterator, BreakPosition, BreakStatus,
    BreakType, CodeUnitFlags, Position, TextDirection, Unicode,
};

// Performs iterator "intersection", i.e. it only returns break positions matching both iterators.
// Port of: modules/skottie/src/text/Unicode.cpp#L18-L100 (chrome/m156) (`IntersectingBreakIterator`)
struct IntersectingBreakIterator<'a> {
    a: Box<dyn BreakIterator + 'a>,
    b: Box<dyn BreakIterator + 'a>,
    current: BreakPosition,
    done: bool,
}

impl<'a> IntersectingBreakIterator<'a> {
    // Port of: modules/skottie/src/text/Unicode.cpp#L21-L28 (chrome/m156)
    fn new(a: Box<dyn BreakIterator + 'a>, b: Box<dyn BreakIterator + 'a>) -> Self {
        Self {
            a,
            b,
            current: 0,
            done: false,
        }
    }
}

impl BreakIterator for IntersectingBreakIterator<'_> {
    // Port of: modules/skottie/src/text/Unicode.cpp#L30-L35 (chrome/m156)
    fn first(&mut self) -> BreakPosition {
        self.current = self.a.first();
        // SkAssertResult(fB->first() == fCurrent)
        let b_first = self.b.first();
        debug_assert_eq!(b_first, self.current);
        self.done = false;
        self.current
    }

    // Port of: modules/skottie/src/text/Unicode.cpp#L37-L39 (chrome/m156)
    fn current(&mut self) -> BreakPosition {
        self.current
    }

    // Port of: modules/skottie/src/text/Unicode.cpp#L41-L44 (chrome/m156)
    fn is_done(&mut self) -> bool {
        debug_assert_eq!(self.a.is_done() || self.b.is_done(), self.done);
        self.done
    }

    // Port of: modules/skottie/src/text/Unicode.cpp#L46-L69 (chrome/m156)
    fn next(&mut self) -> BreakPosition {
        debug_assert!(self.done || (self.a.current() == self.b.current()));
        let mut pos_a = self.current;
        let mut pos_b = self.current;

        while !self.done {
            if pos_a < pos_b {
                pos_a = self.a.next();
                self.done = self.a.is_done();
            } else {
                pos_b = self.b.next();
                self.done = self.b.is_done();
            }

            if pos_a == pos_b {
                break;
            }
        }

        // At this point the positions are either valid and equal (advanced successfully),
        // or one of them is negative (for the done iterator);
        debug_assert!(pos_a == pos_b || self.done);
        debug_assert_eq!(pos_a < 0 || pos_b < 0, self.done);

        self.current = pos_a.min(pos_b);
        self.current
    }

    // Port of: modules/skottie/src/text/Unicode.cpp#L71-L77 (chrome/m156)
    fn set_text_utf8(&mut self, utf8: &[u8]) -> bool {
        self.done = false;
        self.current = 0;
        // `&&` short-circuits as in C++.
        self.a.set_text_utf8(utf8) && self.b.set_text_utf8(utf8)
    }

    // Port of: modules/skottie/src/text/Unicode.cpp#L79-L85 (chrome/m156)
    fn set_text_utf16(&mut self, utf16: &[u16]) -> bool {
        self.done = false;
        self.current = 0;
        self.a.set_text_utf16(utf16) && self.b.set_text_utf16(utf16)
    }

    // Port of: modules/skottie/src/text/Unicode.cpp#L87-L89 (chrome/m156)
    fn status(&mut self) -> BreakStatus {
        unreachable!("IntersectingBreakIterator::status")
    }
}

/// An intersecting break iterator, for the tests (`MakeIntersectingBreakIteratorForTesting`).
// Port of: modules/skottie/src/text/Unicode.cpp#L205-L208 (chrome/m156)
#[doc(hidden)]
#[must_use]
pub fn make_intersecting_break_iterator_for_testing<'a>(
    a: Box<dyn BreakIterator + 'a>,
    b: Box<dyn BreakIterator + 'a>,
) -> Box<dyn BreakIterator + 'a> {
    Box::new(IntersectingBreakIterator::new(a, b))
}

// An `Unicode` wrapper which offers stricter line break semantics (it never breaks words).
// Port of: modules/skottie/src/text/Unicode.cpp#L102-L196 (chrome/m156) (`StrictLinebreakUnicode`)
struct StrictLinebreakUnicode {
    unicode: Arc<dyn Unicode>,
}

impl Unicode for StrictLinebreakUnicode {
    // Port of: modules/skottie/src/text/Unicode.cpp#L108-L123 (chrome/m156)
    fn make_break_iterator_with_locale(
        &self,
        locale: &str,
        break_type: BreakType,
    ) -> Option<Box<dyn BreakIterator + '_>> {
        let brk = self
            .unicode
            .make_break_iterator_with_locale(locale, break_type)?;
        if break_type != BreakType::Lines {
            return Some(brk);
        }

        // When requested a line break iterator, return a composite iterator which prevents
        // breaking mid-word.
        let wbrk = self
            .unicode
            .make_break_iterator_with_locale(locale, BreakType::Words);

        match wbrk {
            Some(wbrk) => Some(Box::new(IntersectingBreakIterator::new(brk, wbrk))),
            None => Some(brk),
        }
    }

    // Proxy everything else.
    fn to_upper(&self, s: &str) -> String {
        self.unicode.to_upper(s)
    }

    fn to_upper_with_locale(&self, s: &str, locale: Option<&str>) -> String {
        self.unicode.to_upper_with_locale(s, locale)
    }

    fn is_control(&self, utf8: Unichar) -> bool {
        self.unicode.is_control(utf8)
    }

    fn is_whitespace(&self, utf8: Unichar) -> bool {
        self.unicode.is_whitespace(utf8)
    }

    fn is_space(&self, utf8: Unichar) -> bool {
        self.unicode.is_space(utf8)
    }

    fn is_tabulation(&self, utf8: Unichar) -> bool {
        self.unicode.is_tabulation(utf8)
    }

    fn is_hard_break(&self, utf8: Unichar) -> bool {
        self.unicode.is_hard_break(utf8)
    }

    fn is_emoji(&self, utf8: Unichar) -> bool {
        self.unicode.is_emoji(utf8)
    }

    fn is_emoji_component(&self, utf8: Unichar) -> bool {
        self.unicode.is_emoji_component(utf8)
    }

    fn is_emoji_modifier_base(&self, utf8: Unichar) -> bool {
        self.unicode.is_emoji_modifier_base(utf8)
    }

    fn is_emoji_modifier(&self, utf8: Unichar) -> bool {
        self.unicode.is_emoji_modifier(utf8)
    }

    fn is_regional_indicator(&self, utf8: Unichar) -> bool {
        self.unicode.is_regional_indicator(utf8)
    }

    fn is_ideographic(&self, utf8: Unichar) -> bool {
        self.unicode.is_ideographic(utf8)
    }

    fn make_bidi_iterator_utf16(
        &self,
        text: &[u16],
        dir: BidiDirection,
    ) -> Option<Box<dyn BidiIterator>> {
        self.unicode.make_bidi_iterator_utf16(text, dir)
    }

    fn make_bidi_iterator_utf8(
        &self,
        text: &[u8],
        dir: BidiDirection,
    ) -> Option<Box<dyn BidiIterator>> {
        self.unicode.make_bidi_iterator_utf8(text, dir)
    }

    fn make_break_iterator(&self, break_type: BreakType) -> Option<Box<dyn BreakIterator + '_>> {
        self.unicode.make_break_iterator(break_type)
    }

    fn get_bidi_regions(
        &self,
        utf8: &[u8],
        dir: TextDirection,
        results: &mut Vec<BidiRegion>,
    ) -> bool {
        self.unicode.get_bidi_regions(utf8, dir, results)
    }

    fn get_words(&self, utf8: &[u8], locale: Option<&str>, results: &mut Vec<Position>) -> bool {
        self.unicode.get_words(utf8, locale, results)
    }

    fn get_utf8_words(
        &self,
        utf8: &[u8],
        locale: Option<&str>,
        results: &mut Vec<Position>,
    ) -> bool {
        self.unicode.get_utf8_words(utf8, locale, results)
    }

    fn get_sentences(
        &self,
        utf8: &[u8],
        locale: Option<&str>,
        results: &mut Vec<Position>,
    ) -> bool {
        self.unicode.get_sentences(utf8, locale, results)
    }

    fn compute_code_unit_flags_utf8(
        &self,
        utf8: &mut [u8],
        replace_tabs: bool,
        results: &mut Vec<CodeUnitFlags>,
    ) -> bool {
        self.unicode
            .compute_code_unit_flags_utf8(utf8, replace_tabs, results)
    }

    fn compute_code_unit_flags_utf16(
        &self,
        utf16: &[u16],
        replace_tabs: bool,
        results: &mut Vec<CodeUnitFlags>,
    ) -> bool {
        self.unicode
            .compute_code_unit_flags_utf16(utf16, replace_tabs, results)
    }

    fn reorder_visual(&self, run_levels: &[BidiLevel], logical_from_visual: &mut [i32]) {
        self.unicode.reorder_visual(run_levels, logical_from_visual);
    }
}

/// Returns a `Unicode` wrapper which suppresses mid-word line breaks
/// (`MakeStrictLinebreakUnicode`).
// Port of: modules/skottie/src/text/Unicode.cpp#L198-L203 (chrome/m156)
#[doc(alias = "MakeStrictLinebreakUnicode")]
#[must_use]
pub fn make_strict_linebreak_unicode(
    unicode: Option<Arc<dyn Unicode>>,
) -> Option<Arc<dyn Unicode>> {
    unicode.map(|unicode| Arc::new(StrictLinebreakUnicode { unicode }) as Arc<dyn Unicode>)
}
