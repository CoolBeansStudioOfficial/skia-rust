// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skunicode/src/SkUnicode_client.cpp,
// modules/skunicode/include/SkUnicode_client.h (chrome/m156)

//! The client implementation of `SkUnicode` (`SkUnicodes::Client`): the client supplies the
//! words, grapheme breaks and line breaks of its text, and this implementation reports them.
//!
//! Not ported yet, and panicking with `unimplemented!` rather than returning a result Skia
//! would not: the bidi methods (`makeBidiIterator`, `getBidiRegions`, `reorderVisual`). They
//! need the bidi factory of milestone M3 (`docs/design/modules.md`). `SkUnicode_client::reset()`
//! is not ported: nothing calls it, and the class is not exposed.

use std::sync::Arc;

use skia_rust_core::utf::{self, Unichar};

use crate::unicode::{
    BidiDirection, BidiIterator, BidiLevel, BidiRegion, BreakIterator, BreakPosition, BreakStatus,
    BreakType, CodeUnitFlags, LineBreakBefore, LineBreakType, Position, TextDirection, Unicode,
};

// Port of: modules/skunicode/src/SkUnicode_client.cpp#L40-L64 (chrome/m156)
/// `SkUnicode_client::Data`: the client's text and breaks, shared with its break iterators.
struct Data<'a> {
    /// `fText8`: borrowed from the client, as Skia's `SkSpan` is.
    text8: &'a [u8],
    /// `fText16`: always empty, since the client gives UTF-8 text.
    text16: &'a [u16],
    words: Vec<Position>,
    grapheme_breaks: Vec<Position>,
    line_breaks: Vec<LineBreakBefore>,
}

// Port of: modules/skunicode/src/SkUnicode_client.cpp#L39-L217 (chrome/m156)
/// `SkUnicode_client`, the `SkUnicode` that reports the client's breaks.
struct UnicodeClient<'a> {
    data: Arc<Data<'a>>,
}

// Port of: modules/skunicode/include/SkUnicode_client.h#L17-L23 (chrome/m156)
// Port of: modules/skunicode/src/SkUnicode_client.cpp#L273-L284 (chrome/m156)
/// `SkUnicodes::Client::Make`: a `SkUnicode` over `text`, whose words, grapheme breaks and line
/// breaks are the given lists. The text is borrowed, so it must outlive the result.
#[doc(alias = "SkUnicodes::Client::Make")]
#[must_use]
pub fn make<'a>(
    text: &'a [u8],
    words: Vec<Position>,
    grapheme_breaks: Vec<Position>,
    line_breaks: Vec<LineBreakBefore>,
) -> Arc<dyn Unicode + 'a> {
    Arc::new(UnicodeClient {
        data: Arc::new(Data {
            text8: text,
            text16: &[],
            words,
            grapheme_breaks,
            line_breaks,
        }),
    })
}

const BIDI_NOT_PORTED: &str = "bidi of the client needs the bidi factory of milestone M3 \
                               (docs/design/modules.md)";

impl UnicodeClient<'_> {
    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L114-L121 (chrome/m156), the flag
    // part of computeCodeUnitFlags that the UTF-8 and UTF-16 versions share.
    fn mark_breaks(&self, results: &mut [CodeUnitFlags]) {
        for line_break in &self.data.line_breaks {
            results[line_break.pos] |= match line_break.break_type {
                LineBreakType::HardLineBreak => CodeUnitFlags::HARD_LINE_BREAK_BEFORE,
                LineBreakType::SoftLineBreak => CodeUnitFlags::SOFT_LINE_BREAK_BEFORE,
            };
        }
        for &grapheme in &self.data.grapheme_breaks {
            results[grapheme] |= CodeUnitFlags::GRAPHEME_START;
        }
    }

    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L111-L187 (chrome/m156) and the same
    // checks in the UTF-16 version: the character properties of one code point.
    fn add_character_flags(&self, unichar: Unichar, flags: &mut CodeUnitFlags) {
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

impl Unicode for UnicodeClient<'_> {
    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L194-L200 (chrome/m156)
    // The client has no case mapping: Skia returns the client's own text, whatever the input.
    fn to_upper(&self, s: &str) -> String {
        self.to_upper_with_locale(s, None)
    }

    fn to_upper_with_locale(&self, _s: &str, _locale: Option<&str>) -> String {
        // Text that is not UTF-8 is not valid Rust text: it is replaced, where Skia copies it.
        String::from_utf8_lossy(self.data.text8).into_owned()
    }

    fn make_bidi_iterator_utf16(
        &self,
        _text: &[u16],
        _dir: BidiDirection,
    ) -> Option<Box<dyn BidiIterator>> {
        unimplemented!("{BIDI_NOT_PORTED}")
    }

    fn make_bidi_iterator_utf8(
        &self,
        _text: &[u8],
        _dir: BidiDirection,
    ) -> Option<Box<dyn BidiIterator>> {
        unimplemented!("{BIDI_NOT_PORTED}")
    }

    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L265-L271 (chrome/m156)
    // The break iterator does not depend on the locale or the break type.
    fn make_break_iterator_with_locale(
        &self,
        _locale: &str,
        _break_type: BreakType,
    ) -> Option<Box<dyn BreakIterator + '_>> {
        Some(Box::new(BreakIteratorClient::new(Arc::clone(&self.data))))
    }

    fn make_break_iterator(&self, _break_type: BreakType) -> Option<Box<dyn BreakIterator + '_>> {
        Some(Box::new(BreakIteratorClient::new(Arc::clone(&self.data))))
    }

    fn get_bidi_regions(
        &self,
        _utf8: &[u8],
        _dir: TextDirection,
        _results: &mut Vec<BidiRegion>,
    ) -> bool {
        unimplemented!("{BIDI_NOT_PORTED}")
    }

    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L189-L192 (chrome/m156)
    fn get_words(&self, _utf8: &[u8], _locale: Option<&str>, results: &mut Vec<Position>) -> bool {
        results.clone_from(&self.data.words);
        true
    }

    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L95-L101 (chrome/m156)
    // Skia's `SkDEBUGF` message is not ported (it is debug-only logging).
    fn get_utf8_words(
        &self,
        _utf8: &[u8],
        _locale: Option<&str>,
        _results: &mut Vec<Position>,
    ) -> bool {
        false
    }

    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L103-L109 (chrome/m156)
    fn get_sentences(
        &self,
        _utf8: &[u8],
        _locale: Option<&str>,
        _results: &mut Vec<Position>,
    ) -> bool {
        false
    }

    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L111-L156 (chrome/m156)
    fn compute_code_unit_flags_utf8(
        &self,
        utf8: &mut [u8],
        replace_tabs: bool,
        results: &mut Vec<CodeUnitFlags>,
    ) -> bool {
        results.clear();
        results.resize(utf8.len() + 1, CodeUnitFlags::NO_CODE_UNIT_FLAG);
        self.mark_breaks(results);
        let end = utf8.len();
        let mut current: usize = 0;
        while current < end {
            let before = current;
            let mut rest = &utf8[current..];
            let mut unichar = utf::next_utf8(&mut rest);
            current = end - rest.len();
            let after = current;
            if unichar < 0 {
                unichar = 0xFFFD;
            }
            if replace_tabs && self.is_tabulation(unichar) {
                results[before] |= CodeUnitFlags::TABULATION;
                // The tab is replaced in the caller's text, as Skia does.
                unichar = Unichar::from(b' ');
                utf8[before] = b' ';
            }
            for flags in &mut results[before..after] {
                self.add_character_flags(unichar, flags);
            }
        }
        true
    }

    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L158-L187 (chrome/m156)
    // Skia's UTF-16 version never replaces tabs: `replace_tabs` is ignored, as in Skia.
    fn compute_code_unit_flags_utf16(
        &self,
        utf16: &[u16],
        _replace_tabs: bool,
        results: &mut Vec<CodeUnitFlags>,
    ) -> bool {
        results.clear();
        results.resize(utf16.len() + 1, CodeUnitFlags::NO_CODE_UNIT_FLAG);
        self.mark_breaks(results);
        for (i, &unit) in utf16.iter().enumerate() {
            self.add_character_flags(Unichar::from(unit), &mut results[i]);
        }
        true
    }

    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L202-L211 (chrome/m156)
    // The empty case returns before any bidi work, as in Skia. The rest needs M3.
    fn reorder_visual(&self, run_levels: &[BidiLevel], _logical_from_visual: &mut [i32]) {
        if run_levels.is_empty() {
            // To avoid an assert in unicode
            return;
        }
        unimplemented!("{BIDI_NOT_PORTED}")
    }
}

// Port of: modules/skunicode/src/SkUnicode_client.cpp#L219-L255 (chrome/m156)
/// `SkBreakIterator_client`: walks the client's line breaks.
struct BreakIteratorClient<'a> {
    data: Arc<Data<'a>>,
    /// `fLastResult`: an index into the line breaks, relative to `start`.
    last_result: i32,
    /// `fStart`: the index in the line breaks of the text set by `setText`.
    start: i32,
    /// `fEnd`: the index just past the end of that text.
    end: i32,
}

impl<'a> BreakIteratorClient<'a> {
    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L225 (chrome/m156)
    fn new(data: Arc<Data<'a>>) -> Self {
        Self {
            data,
            last_result: 0,
            start: 0,
            end: 0,
        }
    }

    /// The `pos` of the line break at `index`, which Skia's `int32` `Position` holds.
    fn line_break_pos(&self, index: i32) -> BreakPosition {
        let line_break = &self.data.line_breaks[usize::try_from(index).expect("index >= 0")];
        i32::try_from(line_break.pos).expect("break positions are int32 in Skia")
    }
}

/// The offset of the sub-slice `sub` in `text`, in elements: `utftext8 - fData->fText8.data()`.
/// `sub` must be a sub-slice of `text`, as `SkBreakIterator_client::setText` requires.
fn text_offset<T>(text: &[T], sub: &[T]) -> i32 {
    let bytes = (sub.as_ptr() as isize).wrapping_sub(text.as_ptr() as isize);
    let elements = bytes / isize::try_from(size_of::<T>()).expect("element size fits");
    i32::try_from(elements).expect("text offsets are int32 in Skia")
}

impl BreakIterator for BreakIteratorClient<'_> {
    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L226-L227 (chrome/m156)
    fn first(&mut self) -> BreakPosition {
        self.last_result = 0;
        self.line_break_pos(self.start + self.last_result)
    }

    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L228-L229 (chrome/m156)
    fn current(&mut self) -> BreakPosition {
        self.line_break_pos(self.start + self.last_result)
    }

    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L230-L231 (chrome/m156)
    // Skia's `next()` returns the break after the current one but does not move to it, so a
    // loop over `next()` repeats the same position. That is ported as it is.
    fn next(&mut self) -> BreakPosition {
        self.line_break_pos(self.start + self.last_result + 1)
    }

    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L232-L237 (chrome/m156)
    fn status(&mut self) -> BreakStatus {
        let flag = match self.data.line_breaks
            [usize::try_from(self.start + self.last_result).expect("index >= 0")]
        .break_type
        {
            LineBreakType::HardLineBreak => CodeUnitFlags::HARD_LINE_BREAK_BEFORE,
            LineBreakType::SoftLineBreak => CodeUnitFlags::SOFT_LINE_BREAK_BEFORE,
        };
        i32::try_from(flag.bits()).expect("flag bits are small")
    }

    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L238 (chrome/m156)
    fn is_done(&mut self) -> bool {
        self.start + self.last_result == self.end
    }

    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L239-L246 (chrome/m156)
    fn set_text_utf8(&mut self, utf8: &[u8]) -> bool {
        debug_assert!(
            (utf8.as_ptr() as usize) >= (self.data.text8.as_ptr() as usize)
                && utf8.len() <= self.data.text8.len()
        );
        self.start = text_offset(self.data.text8, utf8);
        self.end = self.start + i32::try_from(utf8.len()).expect("text length is int32");
        self.last_result = 0;
        true
    }

    // Port of: modules/skunicode/src/SkUnicode_client.cpp#L247-L254 (chrome/m156)
    fn set_text_utf16(&mut self, utf16: &[u16]) -> bool {
        debug_assert!(
            (utf16.as_ptr() as usize) >= (self.data.text16.as_ptr() as usize)
                && utf16.len() <= self.data.text16.len()
        );
        self.start = text_offset(self.data.text16, utf16);
        self.end = self.start + i32::try_from(utf16.len()).expect("text length is int32");
        self.last_result = 0;
        true
    }
}
