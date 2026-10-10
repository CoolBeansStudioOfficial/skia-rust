// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skunicode/include/SkUnicode.h, modules/skunicode/src/SkUnicode.cpp

//! The `SkUnicode` API: code-unit flags, bidi and break iterators, UTF conversions, and the
//! [`Unicode`] trait that every implementation (hardcoded, client, and later ICU) provides.

use skia_rust_core::utf::{self, Unichar};

use crate::unicode_hardcoded;

// Port of: modules/skunicode/include/SkUnicode.h#L44-L65 (chrome/m156)
/// `SkBidiIterator::Position`.
pub type BidiPosition = i32;

/// `SkBidiIterator::Level` and `SkUnicode::BidiLevel`.
pub type BidiLevel = u8;

// Port of: modules/skunicode/include/SkUnicode.h#L67-L82 (chrome/m156)
/// `SkBreakIterator::Position`.
pub type BreakPosition = i32;

/// `SkBreakIterator::Status`: the rule status, as a `CodeUnitFlags` value for client iterators.
pub type BreakStatus = i32;

/// `SkUnicode::Position`: an index into UTF-8 or UTF-16 text, in code units.
#[doc(alias = "SkUnicode::Position")]
pub type Position = usize;

// Port of: modules/skunicode/include/SkUnicode.h#L86-L100 (chrome/m156)
bitflags::bitflags! {
    /// Flags for each code unit of a text (`SkUnicode::CodeUnitFlags`).
    #[doc(alias = "SkUnicode::CodeUnitFlags")]
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
    pub struct CodeUnitFlags: u32 {
        /// `kNoCodeUnitFlag`.
        const NO_CODE_UNIT_FLAG = 0x00;
        /// `kPartOfWhiteSpaceBreak`.
        const PART_OF_WHITE_SPACE_BREAK = 0x01;
        /// `kGraphemeStart`.
        const GRAPHEME_START = 0x02;
        /// `kSoftLineBreakBefore`.
        const SOFT_LINE_BREAK_BEFORE = 0x04;
        /// `kHardLineBreakBefore`.
        const HARD_LINE_BREAK_BEFORE = 0x08;
        /// `kPartOfIntraWordBreak`.
        const PART_OF_INTRA_WORD_BREAK = 0x10;
        /// `kControl`.
        const CONTROL = 0x20;
        /// `kTabulation`.
        const TABULATION = 0x40;
        /// `kGlyphClusterStart`.
        const GLYPH_CLUSTER_START = 0x80;
        /// `kIdeographic`.
        const IDEOGRAPHIC = 0x100;
        /// `kEmoji`.
        const EMOJI = 0x200;
        /// `kWordBreak`.
        const WORD_BREAK = 0x400;
        /// `kSentenceBreak`.
        const SENTENCE_BREAK = 0x800;
    }
}

impl CodeUnitFlags {
    // Port of: modules/skunicode/src/SkUnicode.cpp#L47-L49 (chrome/m156)
    /// `SkUnicode::hasTabulationFlag`.
    #[doc(alias = "hasTabulationFlag")]
    #[must_use]
    pub fn has_tabulation_flag(self) -> bool {
        self.contains(Self::TABULATION)
    }

    // Port of: modules/skunicode/src/SkUnicode.cpp#L51-L53 (chrome/m156)
    /// `SkUnicode::hasHardLineBreakFlag`.
    #[doc(alias = "hasHardLineBreakFlag")]
    #[must_use]
    pub fn has_hard_line_break_flag(self) -> bool {
        self.contains(Self::HARD_LINE_BREAK_BEFORE)
    }

    // Port of: modules/skunicode/src/SkUnicode.cpp#L55-L57 (chrome/m156)
    /// `SkUnicode::hasSoftLineBreakFlag`.
    #[doc(alias = "hasSoftLineBreakFlag")]
    #[must_use]
    pub fn has_soft_line_break_flag(self) -> bool {
        self.contains(Self::SOFT_LINE_BREAK_BEFORE)
    }

    // Port of: modules/skunicode/src/SkUnicode.cpp#L59-L61 (chrome/m156)
    /// `SkUnicode::hasGraphemeStartFlag`.
    #[doc(alias = "hasGraphemeStartFlag")]
    #[must_use]
    pub fn has_grapheme_start_flag(self) -> bool {
        self.contains(Self::GRAPHEME_START)
    }

    // Port of: modules/skunicode/src/SkUnicode.cpp#L63-L65 (chrome/m156)
    /// `SkUnicode::hasControlFlag`.
    #[doc(alias = "hasControlFlag")]
    #[must_use]
    pub fn has_control_flag(self) -> bool {
        self.contains(Self::CONTROL)
    }

    // Port of: modules/skunicode/src/SkUnicode.cpp#L67-L69 (chrome/m156)
    /// `SkUnicode::hasPartOfWhiteSpaceBreakFlag`.
    #[doc(alias = "hasPartOfWhiteSpaceBreakFlag")]
    #[must_use]
    pub fn has_part_of_white_space_break_flag(self) -> bool {
        self.contains(Self::PART_OF_WHITE_SPACE_BREAK)
    }
}

// Port of: modules/skunicode/include/SkUnicode.h#L101-L104 (chrome/m156)
/// `SkUnicode::TextDirection`.
#[doc(alias = "SkUnicode::TextDirection")]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum TextDirection {
    /// `kLTR`.
    Ltr,
    /// `kRTL`.
    Rtl,
}

// Port of: modules/skunicode/include/SkUnicode.h#L44-L65 (chrome/m156)
/// `SkBidiIterator::Direction`.
#[doc(alias = "SkBidiIterator::Direction")]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BidiDirection {
    /// `kLTR`.
    Ltr,
    /// `kRTL`.
    Rtl,
}

// Port of: modules/skunicode/include/SkUnicode.h#L48-L53 (chrome/m156)
/// `SkBidiIterator::Region`: a run of text at one embedding level.
#[doc(alias = "SkBidiIterator::Region")]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct BidiIteratorRegion {
    /// `start`.
    pub start: BidiPosition,
    /// `end`.
    pub end: BidiPosition,
    /// `level`.
    pub level: BidiLevel,
}

impl BidiIteratorRegion {
    /// `SkBidiIterator::Region::Region`.
    #[must_use]
    pub fn new(start: BidiPosition, end: BidiPosition, level: BidiLevel) -> Self {
        Self { start, end, level }
    }
}

// Port of: modules/skunicode/include/SkUnicode.h#L107-L112 (chrome/m156)
/// `SkUnicode::BidiRegion`: a run of text (in UTF-16 code units) at one level.
#[doc(alias = "SkUnicode::BidiRegion")]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct BidiRegion {
    /// `start`.
    pub start: Position,
    /// `end`.
    pub end: Position,
    /// `level`.
    pub level: BidiLevel,
}

impl BidiRegion {
    /// `SkUnicode::BidiRegion::BidiRegion`.
    #[must_use]
    pub fn new(start: Position, end: Position, level: BidiLevel) -> Self {
        Self { start, end, level }
    }
}

// Port of: modules/skunicode/include/SkUnicode.h#L114-L117 (chrome/m156)
/// `SkUnicode::LineBreakType`. The discriminants are Skia's.
#[doc(alias = "SkUnicode::LineBreakType")]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum LineBreakType {
    /// `kSoftLineBreak`.
    SoftLineBreak = 0,
    /// `kHardLineBreak`.
    HardLineBreak = 100,
}

// Port of: modules/skunicode/include/SkUnicode.h#L119 (chrome/m156)
/// `SkUnicode::BreakType`.
#[doc(alias = "SkUnicode::BreakType")]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BreakType {
    /// `kWords`.
    Words,
    /// `kGraphemes`.
    Graphemes,
    /// `kLines`.
    Lines,
    /// `kSentences`.
    Sentences,
}

// Port of: modules/skunicode/include/SkUnicode.h#L120-L124 (chrome/m156)
/// `SkUnicode::LineBreakBefore`: a line break before the code unit at `pos`.
#[doc(alias = "SkUnicode::LineBreakBefore")]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct LineBreakBefore {
    /// `pos`.
    pub pos: Position,
    /// `breakType`.
    pub break_type: LineBreakType,
}

impl LineBreakBefore {
    /// `SkUnicode::LineBreakBefore::LineBreakBefore`.
    #[must_use]
    pub fn new(pos: Position, break_type: LineBreakType) -> Self {
        Self { pos, break_type }
    }
}

// Port of: modules/skunicode/include/SkUnicode.h#L44-L65 (chrome/m156)
/// `SkBidiIterator`: the embedding levels of a text.
#[doc(alias = "SkBidiIterator")]
pub trait BidiIterator {
    /// `getLength`.
    fn get_length(&mut self) -> BidiPosition;
    /// `getLevelAt`.
    fn get_level_at(&mut self, pos: BidiPosition) -> BidiLevel;
}

// Port of: modules/skunicode/include/SkUnicode.h#L67-L82 (chrome/m156)
/// `SkBreakIterator`: the boundaries of a text, and the rule status at each.
#[doc(alias = "SkBreakIterator")]
pub trait BreakIterator {
    /// `first`.
    fn first(&mut self) -> BreakPosition;
    /// `current`.
    fn current(&mut self) -> BreakPosition;
    /// `next`.
    fn next(&mut self) -> BreakPosition;
    /// `status`.
    fn status(&mut self) -> BreakStatus;
    /// `isDone`.
    fn is_done(&mut self) -> bool;
    /// `setText(const char utftext8[], int utf8Units)`.
    fn set_text_utf8(&mut self, utf8: &[u8]) -> bool;
    /// `setText(const char16_t utftext16[], int utf16Units)`.
    fn set_text_utf16(&mut self, utf16: &[u16]) -> bool;
}

// Port of: modules/skunicode/include/SkUnicode.h#L84 (chrome/m156)
/// `SkUnicode`: the Unicode services of the text stack.
///
/// The character properties (`isControl` … `isIdeographic`) default to the hardcoded
/// implementation of `SkUnicodeHardCodedCharProperties`. An implementation with Unicode data
/// (ICU, milestone M2) overrides them.
#[doc(alias = "SkUnicode")]
pub trait Unicode: Send + Sync {
    // Port of: modules/skunicode/include/SkUnicode.h#L130-L131 (chrome/m156)
    /// `toUpper(const SkString&)`, deprecated in Skia.
    #[doc(alias = "toUpper")]
    fn to_upper(&self, s: &str) -> String;

    // Port of: modules/skunicode/include/SkUnicode.h#L131 (chrome/m156)
    /// `toUpper(const SkString&, const char* locale)`.
    #[doc(alias = "toUpper")]
    fn to_upper_with_locale(&self, s: &str, locale: Option<&str>) -> String;

    // Port of: modules/skunicode/src/SkUnicode_hardcoded.cpp#L14-L18 (chrome/m156)
    /// `isControl`.
    #[doc(alias = "isControl")]
    fn is_control(&self, utf8: Unichar) -> bool {
        unicode_hardcoded::is_control(utf8)
    }

    /// `isWhitespace`.
    #[doc(alias = "isWhitespace")]
    fn is_whitespace(&self, utf8: Unichar) -> bool {
        unicode_hardcoded::is_whitespace(utf8)
    }

    /// `isSpace`.
    #[doc(alias = "isSpace")]
    fn is_space(&self, utf8: Unichar) -> bool {
        unicode_hardcoded::is_space(utf8)
    }

    /// `isTabulation`.
    #[doc(alias = "isTabulation")]
    fn is_tabulation(&self, utf8: Unichar) -> bool {
        unicode_hardcoded::is_tabulation(utf8)
    }

    /// `isHardBreak`.
    #[doc(alias = "isHardBreak")]
    fn is_hard_break(&self, utf8: Unichar) -> bool {
        unicode_hardcoded::is_hard_break(utf8)
    }

    /// `isEmoji`. The hardcoded implementation does not know emoji (`SkDEBUGFAIL`).
    #[doc(alias = "isEmoji")]
    fn is_emoji(&self, utf8: Unichar) -> bool {
        unicode_hardcoded::is_emoji(utf8)
    }

    /// `isEmojiComponent`. The hardcoded implementation does not know emoji (`SkDEBUGFAIL`).
    #[doc(alias = "isEmojiComponent")]
    fn is_emoji_component(&self, utf8: Unichar) -> bool {
        unicode_hardcoded::is_emoji_component(utf8)
    }

    /// `isEmojiModifierBase`. The hardcoded implementation does not know emoji (`SkDEBUGFAIL`).
    #[doc(alias = "isEmojiModifierBase")]
    fn is_emoji_modifier_base(&self, utf8: Unichar) -> bool {
        unicode_hardcoded::is_emoji_modifier_base(utf8)
    }

    /// `isEmojiModifier`. The hardcoded implementation does not know emoji (`SkDEBUGFAIL`).
    #[doc(alias = "isEmojiModifier")]
    fn is_emoji_modifier(&self, utf8: Unichar) -> bool {
        unicode_hardcoded::is_emoji_modifier(utf8)
    }

    /// `isRegionalIndicator`. The hardcoded implementation does not know it (`SkDEBUGFAIL`).
    #[doc(alias = "isRegionalIndicator")]
    fn is_regional_indicator(&self, utf8: Unichar) -> bool {
        unicode_hardcoded::is_regional_indicator(utf8)
    }

    /// `isIdeographic`.
    #[doc(alias = "isIdeographic")]
    fn is_ideographic(&self, utf8: Unichar) -> bool {
        unicode_hardcoded::is_ideographic(utf8)
    }

    /// `makeBidiIterator(const uint16_t text[], int count, Direction)`.
    #[doc(alias = "makeBidiIterator")]
    fn make_bidi_iterator_utf16(
        &self,
        text: &[u16],
        dir: BidiDirection,
    ) -> Option<Box<dyn BidiIterator>>;

    /// `makeBidiIterator(const char text[], int count, Direction)`.
    #[doc(alias = "makeBidiIterator")]
    fn make_bidi_iterator_utf8(
        &self,
        text: &[u8],
        dir: BidiDirection,
    ) -> Option<Box<dyn BidiIterator>>;

    /// `makeBreakIterator(const char locale[], BreakType)`.
    #[doc(alias = "makeBreakIterator")]
    fn make_break_iterator_with_locale(
        &self,
        locale: &str,
        break_type: BreakType,
    ) -> Option<Box<dyn BreakIterator + '_>>;

    /// `makeBreakIterator(BreakType)`.
    #[doc(alias = "makeBreakIterator")]
    fn make_break_iterator(&self, break_type: BreakType) -> Option<Box<dyn BreakIterator + '_>>;

    // Port of: modules/skunicode/include/SkUnicode.h#L161-L166 (chrome/m156)
    // The static helpers are the `CodeUnitFlags::has_*` methods above.

    /// `getBidiRegions`.
    #[doc(alias = "getBidiRegions")]
    fn get_bidi_regions(
        &self,
        utf8: &[u8],
        dir: TextDirection,
        results: &mut Vec<BidiRegion>,
    ) -> bool;

    /// `getWords`: the word boundaries (in UTF-16 code units) of the text.
    #[doc(alias = "getWords")]
    fn get_words(&self, utf8: &[u8], locale: Option<&str>, results: &mut Vec<Position>) -> bool;

    /// `getUtf8Words`.
    #[doc(alias = "getUtf8Words")]
    fn get_utf8_words(
        &self,
        utf8: &[u8],
        locale: Option<&str>,
        results: &mut Vec<Position>,
    ) -> bool;

    /// `getSentences`.
    #[doc(alias = "getSentences")]
    fn get_sentences(&self, utf8: &[u8], locale: Option<&str>, results: &mut Vec<Position>)
    -> bool;

    /// `computeCodeUnitFlags(char utf8[], int utf8Units, bool replaceTabs, …)`. With
    /// `replace_tabs`, each tab in `utf8` is replaced by a space, in place, as in Skia.
    #[doc(alias = "computeCodeUnitFlags")]
    fn compute_code_unit_flags_utf8(
        &self,
        utf8: &mut [u8],
        replace_tabs: bool,
        results: &mut Vec<CodeUnitFlags>,
    ) -> bool;

    /// `computeCodeUnitFlags(char16_t utf16[], int utf16Units, bool replaceTabs, …)`.
    #[doc(alias = "computeCodeUnitFlags")]
    fn compute_code_unit_flags_utf16(
        &self,
        utf16: &[u16],
        replace_tabs: bool,
        results: &mut Vec<CodeUnitFlags>,
    ) -> bool;

    /// `reorderVisual(const BidiLevel runLevels[], int levelsCount, int32_t logicalFromVisual[])`.
    #[doc(alias = "reorderVisual")]
    fn reorder_visual(&self, run_levels: &[BidiLevel], logical_from_visual: &mut [i32]);

    // Port of: modules/skunicode/include/SkUnicode.h#L272-L301 (chrome/m156)
    /// `forEachBidiRegion`: calls `callback(start, end, level)` for each run of one level in
    /// the UTF-16 text.
    #[doc(alias = "forEachBidiRegion")]
    fn for_each_bidi_region(
        &self,
        utf16: &[u16],
        dir: BidiDirection,
        callback: &mut dyn FnMut(Position, Position, BidiLevel),
    ) {
        // makeBidiIterator returns a null unique_ptr only if the implementation has no bidi,
        // which Skia's own callers never ask of.
        let mut iter = self
            .make_bidi_iterator_utf16(utf16, dir)
            .expect("makeBidiIterator returned null");
        let end16 = utf16.len();
        // `start16` and `pos16` are code-unit indices of the C++ pointers and positions.
        let mut start16: usize = 0;
        let mut current_level: BidiLevel = 0;
        let mut pos16: Position = 0;
        loop {
            // Skia's getLength() is an int32 compared with a size_t: converted as C++ does.
            #[allow(clippy::cast_sign_loss)] // mirrors the int32 -> size_t conversion of C++
            let length = iter.get_length() as usize;
            if pos16 > length {
                break;
            }
            let next_pos16 = start16;
            let level = iter.get_level_at(BidiPosition::try_from(next_pos16).expect("in range"));
            if next_pos16 == 0 {
                current_level = level;
            } else if level != current_level {
                callback(pos16, next_pos16, current_level);
                current_level = level;
                pos16 = next_pos16;
            }
            if start16 == end16 {
                if pos16 != next_pos16 {
                    callback(pos16, next_pos16, current_level);
                }
                return;
            }
            start16 = next_utf16_index(utf16, start16);
        }
    }

    // Port of: modules/skunicode/include/SkUnicode.h#L303-L312 (chrome/m156)
    /// `forEachBreak`: calls `callback(position, status)` for each break of the UTF-16 text.
    #[doc(alias = "forEachBreak")]
    fn for_each_break(
        &self,
        utf16: &[u16],
        break_type: BreakType,
        callback: &mut dyn FnMut(BreakPosition, BreakStatus),
    ) {
        let mut iter = self
            .make_break_iterator(break_type)
            .expect("makeBreakIterator returned null");
        iter.set_text_utf16(utf16);
        let mut pos = iter.first();
        loop {
            callback(pos, iter.status());
            pos = iter.next();
            if iter.is_done() {
                break;
            }
        }
    }
}

// Port of: modules/skunicode/src/SkUnicode.cpp#L13-L24 (chrome/m156)
/// `SkUnicode::convertUtf16ToUtf8`. Invalid UTF-16 gives an empty string (Skia's debug-only
/// `SkDEBUGF` message is not ported).
///
/// # Panics
///
/// Never: the bytes come from `utf16_to_utf8`, which produces valid UTF-8 or fails early.
#[doc(alias = "convertUtf16ToUtf8")]
#[must_use]
pub fn convert_utf16_to_utf8(utf16: &[u16]) -> String {
    let utf8_units = utf::utf16_to_utf8(None, utf16);
    if utf8_units < 0 {
        return String::new();
    }
    #[allow(clippy::cast_sign_loss)] // checked to be non-negative above
    let mut bytes = vec![0u8; utf8_units as usize];
    let dst_len = utf::utf16_to_utf8(Some(&mut bytes), utf16);
    debug_assert_eq!(dst_len, utf8_units);
    // Valid UTF-16 converts to valid UTF-8, so the bytes are a string.
    String::from_utf8(bytes).expect("UTF-16 to UTF-8 conversion gives valid UTF-8")
}

// Port of: modules/skunicode/src/SkUnicode.cpp#L30-L41 (chrome/m156)
/// `SkUnicode::convertUtf8ToUtf16`. Invalid UTF-8 gives an empty vector (Skia's debug-only
/// `SkDEBUGF` message is not ported). The `SkString` overload is `convert_utf8_to_utf16(s.as_bytes())`.
#[doc(alias = "convertUtf8ToUtf16")]
#[must_use]
pub fn convert_utf8_to_utf16(utf8: &[u8]) -> Vec<u16> {
    let utf16_units = utf::utf8_to_utf16(None, utf8);
    if utf16_units < 0 {
        return Vec::new();
    }
    #[allow(clippy::cast_sign_loss)] // checked to be non-negative above
    let mut units = vec![0u16; utf16_units as usize];
    let dst_len = utf::utf8_to_utf16(Some(&mut units), utf8);
    debug_assert_eq!(dst_len, utf16_units);
    units
}

// Port of: modules/skunicode/include/SkUnicode.h#L198-L244 (chrome/m156)
/// `SkUnicode::extractUtfConversionMapping`: calls `appender8(index)` for each UTF-16 code unit
/// of the mapping (its UTF-8 index) and `appender16(index)` for each UTF-8 code unit (its UTF-16
/// index), then one final call each for the end. Returns false if the two counts disagree.
#[doc(alias = "extractUtfConversionMapping")]
pub fn extract_utf_conversion_mapping<A8, A16>(
    utf8: &[u8],
    mut appender8: A8,
    mut appender16: A16,
) -> bool
where
    A8: FnMut(usize),
    A16: FnMut(usize),
{
    let mut size8: usize = 0;
    let mut size16: usize = 0;
    let end = utf8.len();
    let mut ptr: usize = 0;
    while ptr < end {
        let index = ptr;
        let mut rest = &utf8[ptr..];
        let uni = utf::next_utf8(&mut rest);
        ptr = end - rest.len();

        // All UTF8 code units refer to the same codepoint
        let next = ptr;
        for _ in index..next {
            appender16(size8);
            size16 += 1;
        }
        debug_assert_eq!(size16, next);
        if size16 != next {
            return false;
        }

        // One or two UTF16 code units refer to the same codepoint
        let mut buffer = [0u16; 2];
        let count = utf::to_utf16(uni, Some(&mut buffer));
        appender8(index);
        size8 += 1;
        if count > 1 {
            appender8(index);
            size8 += 1;
        }
    }
    appender16(size8);
    appender8(utf8.len());
    true
}

// Port of: modules/skunicode/include/SkUnicode.h#L246-L258 (chrome/m156)
/// `SkUnicode::forEachCodepoint` for UTF-8. `callback(unichar, before, after, count)` gets each
/// code point's byte range and its UTF-16 length; invalid bytes are U+FFFD.
#[doc(alias = "forEachCodepoint")]
pub fn for_each_codepoint_utf8(
    utf8: &[u8],
    mut callback: impl FnMut(Unichar, usize, usize, usize),
) {
    let end = utf8.len();
    let mut current: usize = 0;
    while current < end {
        let before = current;
        let mut rest = &utf8[current..];
        let mut unichar = utf::next_utf8(&mut rest);
        current = end - rest.len();
        if unichar < 0 {
            unichar = 0xFFFD;
        }
        let after = current;
        let mut buffer = [0u16; 2];
        let count = utf::to_utf16(unichar, Some(&mut buffer));
        callback(unichar, before, after, count);
    }
}

// Port of: modules/skunicode/include/SkUnicode.h#L261-L270 (chrome/m156)
/// `SkUnicode::forEachCodepoint` for UTF-16. `callback(unichar, before, after)` gets each code
/// point's code-unit range; unpaired surrogates give -1.
#[doc(alias = "forEachCodepoint")]
pub fn for_each_codepoint_utf16(utf16: &[u16], mut callback: impl FnMut(Unichar, usize, usize)) {
    let end = utf16.len();
    let mut current: usize = 0;
    while current < end {
        let before = current;
        let mut rest = &utf16[current..];
        let unichar = utf::next_utf16(&mut rest);
        current = end - rest.len();
        callback(unichar, before, current);
    }
}

/// The index after the UTF-16 code point that starts at `index` (`SkUTF::NextUTF16` on a
/// pointer, which a failure moves to the end).
fn next_utf16_index(utf16: &[u16], index: usize) -> usize {
    let mut rest = &utf16[index..];
    utf::next_utf16(&mut rest);
    utf16.len() - rest.len()
}
