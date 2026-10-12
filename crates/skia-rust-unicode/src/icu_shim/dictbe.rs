// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! ICU's dictionary break engines (`dictbe.h`, `dictbe.cpp`): `ThaiBreakEngine`, `LaoBreakEngine`,
//! `BurmeseBreakEngine`, `KhmerBreakEngine` (with Chromium's `khmer-dictbe.patch`) and
//! `CjkBreakEngine` for Chinese and Japanese. They split a range of dictionary characters that the
//! rules left whole.
//!
//! Not ported: phrase breaking (`isPhraseBreaking`, ICU's ML phrase engine and the `ja` skip set),
//! which only the `@lw=phrase` line rules ask for and no Skia caller uses; the shim runs such
//! iterators without it. There is no Korean engine: Chromium's data has no Hangul dictionary.

// The engines mirror ICU's `int32_t` / `uint32_t` arithmetic on native indexes and counts.
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]

use icu_normalizer::properties::CanonicalCombiningClassMapBorrowed;
use icu_normalizer::{ComposingNormalizerBorrowed, DecomposingNormalizerBorrowed};
use icu_properties::CodePointMapData;
use icu_properties::props::{GeneralCategory, LineBreak};

use super::dictionary_data::{DictionaryMatcher, MatchOutput};
use super::uchar::{line_break, uscript_get_script};
use super::utext::{U_SENTINEL, UText};

/// `UScriptCode` values (`uscript.h`).
pub(crate) mod script {
    /// `USCRIPT_HAN`.
    pub(crate) const HAN: i32 = 17;
    /// `USCRIPT_HIRAGANA`.
    pub(crate) const HIRAGANA: i32 = 20;
    /// `USCRIPT_KATAKANA`.
    pub(crate) const KATAKANA: i32 = 22;
    /// `USCRIPT_KHMER`.
    pub(crate) const KHMER: i32 = 23;
    /// `USCRIPT_LAO`.
    pub(crate) const LAO: i32 = 24;
    /// `USCRIPT_MYANMAR`.
    pub(crate) const MYANMAR: i32 = 28;
    /// `USCRIPT_THAI`.
    pub(crate) const THAI: i32 = 38;
}

/// The dictionary engines, by the script ICU creates them for (`brkeng.cpp#L196-L228`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum EngineKind {
    /// `ThaiBreakEngine`.
    Thai,
    /// `LaoBreakEngine`.
    Lao,
    /// `BurmeseBreakEngine`.
    Burmese,
    /// `KhmerBreakEngine`.
    Khmer,
    /// `CjkBreakEngine(kChineseJapanese)`.
    ChineseJapanese,
}

impl EngineKind {
    /// Every kind.
    pub(crate) const ALL: [EngineKind; 5] = [
        EngineKind::Thai,
        EngineKind::Lao,
        EngineKind::Burmese,
        EngineKind::Khmer,
        EngineKind::ChineseJapanese,
    ];

    // Port of: source/data/brkitr/root.txt#L17-L25 (`dictionaries`) and icu4c/source/common/
    // brkeng.cpp#L196-L228 (chromium/deps/icu d578f2e8)
    /// The engine `loadEngineFor` creates for a script, if Chromium's data has its dictionary.
    pub(crate) fn for_script(script: i32) -> Option<EngineKind> {
        match script {
            script::THAI => Some(EngineKind::Thai),
            script::LAO => Some(EngineKind::Lao),
            script::MYANMAR => Some(EngineKind::Burmese),
            script::KHMER => Some(EngineKind::Khmer),
            script::HIRAGANA | script::KATAKANA | script::HAN => Some(EngineKind::ChineseJapanese),
            _ => None,
        }
    }

    /// The dictionary file (`brkitr/<name>.dict`) of the root bundle's `dictionaries` table.
    pub(crate) fn dictionary(self) -> &'static str {
        match self {
            EngineKind::Thai => "thaidict",
            EngineKind::Lao => "laodict",
            EngineKind::Burmese => "burmesedict",
            EngineKind::Khmer => "khmerdict",
            EngineKind::ChineseJapanese => "cjdict",
        }
    }

    // Port of: icu4c/source/common/dictbe.cpp#L196-L211, #L448-L462, #L640-L656, #L837-L862 and
    // #L1062-L1077 (the `setCharacters` sets) (chromium/deps/icu d578f2e8)
    /// `fSet.contains(c)`: `[[:Thai:]&[:LineBreak=SA:]]` and its Lao, Myanmar and Khmer
    /// counterparts, or `[[:Han:][:Hiragana:][:Katakana:]ーｰﾞﾟ]`.
    pub(crate) fn handles(self, c: i32) -> bool {
        if c < 0 {
            return false;
        }
        let sa = |s: i32| uscript_get_script(c) == s && is_sa(c);
        match self {
            EngineKind::Thai => sa(script::THAI),
            EngineKind::Lao => sa(script::LAO),
            EngineKind::Burmese => sa(script::MYANMAR),
            EngineKind::Khmer => sa(script::KHMER),
            EngineKind::ChineseJapanese => {
                matches!(
                    uscript_get_script(c),
                    script::HAN | script::HIRAGANA | script::KATAKANA
                ) || matches!(c, 0x30fc | 0xff70 | 0xff9e | 0xff9f)
            }
        }
    }
}

/// `[:LineBreak=SA:]`.
fn is_sa(c: i32) -> bool {
    u32::try_from(c).is_ok_and(|c| c <= 0x10_FFFF && line_break(c) == LineBreak::ComplexContext)
}

/// `[:M:]`.
fn is_mark(c: i32) -> bool {
    u32::try_from(c).is_ok_and(|c| {
        matches!(
            CodePointMapData::<GeneralCategory>::new().get32(c),
            GeneralCategory::NonspacingMark
                | GeneralCategory::SpacingMark
                | GeneralCategory::EnclosingMark
        )
    })
}

/// `UVector32::peeki`: the last element, or 0.
pub(crate) fn peeki(v: &[i32]) -> i32 {
    v.last().copied().unwrap_or(0)
}

/// `UVector32::elementAti`: the element, or 0 out of range.
pub(crate) fn element_ati(v: &[i32], i: i32) -> i32 {
    usize::try_from(i)
        .ok()
        .and_then(|i| v.get(i))
        .copied()
        .unwrap_or(0)
}

/// `UVector32::setElementAt(elem, index)`: ignored out of range.
fn set_element_at(v: &mut [i32], elem: i32, i: i32) {
    if let Some(slot) = usize::try_from(i).ok().and_then(|i| v.get_mut(i)) {
        *slot = elem;
    }
}

// --------------------------------------------------------------------------------------------
// PossibleWord

// Port of: icu4c/source/common/dictbe.cpp#L86-L176 (chromium/deps/icu d578f2e8)
/// `POSSIBLE_WORD_LIST_MAX`.
const POSSIBLE_WORD_LIST_MAX: usize = 20;

/// `PossibleWord`: the dictionary words at one position.
#[derive(Clone, Copy, Debug)]
struct PossibleWord {
    /// Count of candidates.
    count: i32,
    /// The longest match with a dictionary word.
    prefix: i32,
    /// Offset in the text of these candidates.
    offset: i32,
    /// The preferred candidate's offset.
    mark: i32,
    /// The candidate we're currently looking at.
    current: i32,
    /// Word lengths, in code units.
    cu_lengths: [i32; POSSIBLE_WORD_LIST_MAX],
    /// Word lengths, in code points.
    cp_lengths: [i32; POSSIBLE_WORD_LIST_MAX],
}

impl PossibleWord {
    fn new() -> Self {
        PossibleWord {
            count: 0,
            prefix: 0,
            offset: -1,
            mark: 0,
            current: 0,
            cu_lengths: [0; POSSIBLE_WORD_LIST_MAX],
            cp_lengths: [0; POSSIBLE_WORD_LIST_MAX],
        }
    }

    fn at(a: &[i32; POSSIBLE_WORD_LIST_MAX], i: i32) -> i32 {
        a[usize::try_from(i).expect("a candidate index")]
    }

    /// Fill the list of candidates if needed, select the longest, and return the number found.
    fn candidates(&mut self, text: &mut UText, dict: &DictionaryMatcher, range_end: i32) -> i32 {
        let start = text.get_native_index();
        if start != self.offset {
            self.offset = start;
            self.count = dict.matches(
                text,
                range_end - start,
                POSSIBLE_WORD_LIST_MAX as i32,
                MatchOutput {
                    lengths: Some(&mut self.cu_lengths),
                    cp_lengths: Some(&mut self.cp_lengths),
                    values: None,
                    prefix: Some(&mut self.prefix),
                },
            );
            // Dictionary leaves text after longest prefix, not longest word. Back up.
            if self.count <= 0 {
                text.set_native_index(start);
            }
        }
        if self.count > 0 {
            text.set_native_index(start + Self::at(&self.cu_lengths, self.count - 1));
        }
        self.current = self.count - 1;
        self.mark = self.current;
        self.count
    }

    /// Select the currently marked candidate, point after it in the text, and invalidate self.
    fn accept_marked(&self, text: &mut UText) -> i32 {
        text.set_native_index(self.offset + Self::at(&self.cu_lengths, self.mark));
        Self::at(&self.cu_lengths, self.mark)
    }

    /// Back up from the current candidate to the next shorter one; return true if that exists
    /// and point the text after it.
    fn back_up(&mut self, text: &mut UText) -> bool {
        if self.current > 0 {
            self.current -= 1;
            text.set_native_index(self.offset + Self::at(&self.cu_lengths, self.current));
            return true;
        }
        false
    }

    /// Return the longest prefix this candidate location shares with a dictionary word.
    fn longest_prefix(&self) -> i32 {
        self.prefix
    }

    /// Mark the current candidate as the one we like.
    fn mark_current(&mut self) {
        self.mark = self.current;
    }

    /// Get length in code points of the marked word.
    fn marked_cp_length(&self) -> i32 {
        Self::at(&self.cp_lengths, self.mark)
    }
}

// --------------------------------------------------------------------------------------------
// The South-East Asian engines

/// What distinguishes `ThaiBreakEngine`, `LaoBreakEngine`, `BurmeseBreakEngine` and
/// `KhmerBreakEngine`: their constants, sets and two details of the algorithm.
#[derive(Clone, Copy, Debug)]
struct SeaParams {
    /// `*_ROOT_COMBINE_THRESHOLD`.
    root_combine_threshold: i32,
    /// `*_PREFIX_COMBINE_THRESHOLD`.
    prefix_combine_threshold: i32,
    /// Thai checks the minimum span in code points (`utext_moveIndex32`); the others compare
    /// native lengths.
    span_in_code_points: bool,
    /// Thai's suffix handling (PAIYANNOI, MAIYAMOK).
    suffixes: bool,
}

/// `*_LOOKAHEAD`: how many words in a row are "good enough".
const LOOKAHEAD: u32 = 3;
/// `*_MIN_WORD_SPAN` (`*_MIN_WORD * 2`): minimum number of characters for two words.
const MIN_WORD_SPAN: i32 = 2 * 2;
/// `THAI_PAIYANNOI`: elision character.
const THAI_PAIYANNOI: i32 = 0x0E2F;
/// `THAI_MAIYAMOK`: repeat character.
const THAI_MAIYAMOK: i32 = 0x0E46;

impl EngineKind {
    // Port of: icu4c/source/common/dictbe.cpp#L178-L194, #L430-L446, #L622-L638 and #L815-L834
    // (with khmer-dictbe.patch: KHMER_ROOT_COMBINE_THRESHOLD 10, KHMER_PREFIX_COMBINE_THRESHOLD 5)
    // (chromium/deps/icu d578f2e8)
    fn sea_params(self) -> SeaParams {
        let (root_combine_threshold, prefix_combine_threshold) = match self {
            EngineKind::Khmer => (10, 5),
            _ => (3, 3),
        };
        SeaParams {
            root_combine_threshold,
            prefix_combine_threshold,
            span_in_code_points: self == EngineKind::Thai,
            suffixes: self == EngineKind::Thai,
        }
    }

    /// `fMarkSet`: the engine's set ∩ `[:M:]`, plus U+0020.
    fn mark_set(self, c: i32) -> bool {
        c == 0x0020 || (self.handles(c) && is_mark(c))
    }

    /// `fEndWordSet`.
    fn end_word_set(self, c: i32) -> bool {
        self.handles(c)
            && match self {
                // MAI HAN-AKAT, SARA E through SARA AI MAIMALAI
                EngineKind::Thai => !(c == 0x0E31 || (0x0E40..=0x0E44).contains(&c)),
                // prefix vowels
                EngineKind::Lao => !(0x0EC0..=0x0EC4).contains(&c),
                EngineKind::Burmese => true,
                // KHMER SIGN COENG that combines some following characters
                EngineKind::Khmer => c != 0x17D2,
                EngineKind::ChineseJapanese => unreachable!("not a South-East Asian engine"),
            }
    }

    /// `fBeginWordSet`.
    fn begin_word_set(self, c: i32) -> bool {
        match self {
            // KO KAI through HO NOKHUK, SARA E through SARA AI MAIMALAI
            EngineKind::Thai => (0x0E01..=0x0E2E).contains(&c) || (0x0E40..=0x0E44).contains(&c),
            // basic consonants (including holes for corresponding Thai characters), digraph
            // consonants (no Thai equivalent), prefix vowels
            EngineKind::Lao => {
                (0x0E81..=0x0EAE).contains(&c)
                    || (0x0EDC..=0x0EDD).contains(&c)
                    || (0x0EC0..=0x0EC4).contains(&c)
            }
            // basic consonants and independent vowels
            EngineKind::Burmese => (0x1000..=0x102A).contains(&c),
            EngineKind::Khmer => (0x1780..=0x17B3).contains(&c),
            EngineKind::ChineseJapanese => unreachable!("not a South-East Asian engine"),
        }
    }

    /// `fSuffixSet` (Thai).
    fn suffix_set(c: i32) -> bool {
        c == THAI_PAIYANNOI || c == THAI_MAIYAMOK
    }
}

// Port of: icu4c/source/common/dictbe.cpp#L222-L408 (ThaiBreakEngine::divideUpDictionaryRange),
// #L464-L615 (Lao), #L658-L808 (Burmese), #L864-L1046 (Khmer) (chromium/deps/icu d578f2e8)
/// `divideUpDictionaryRange` of the four South-East Asian engines.
// One function in ICU (four near-copies); `cuWordLength` / `cpWordLength` are ICU's names.
#[allow(clippy::too_many_lines, clippy::similar_names)]
fn divide_sea(
    kind: EngineKind,
    dict: &DictionaryMatcher,
    text: &mut UText,
    range_start: i32,
    range_end: i32,
    found_breaks: &mut Vec<i32>,
) -> i32 {
    let p = kind.sea_params();
    if p.span_in_code_points {
        text.set_native_index(range_start);
        text.move_index32(MIN_WORD_SPAN);
        if text.get_native_index() >= range_end {
            return 0; // Not enough characters for two words
        }
        text.set_native_index(range_start);
    } else if (range_end - range_start) < MIN_WORD_SPAN {
        return 0; // Not enough characters for two words
    }

    let mut words_found: u32 = 0;
    let mut words = [PossibleWord::new(); LOOKAHEAD as usize];
    let w = |n: u32| (n % LOOKAHEAD) as usize;

    text.set_native_index(range_start);

    loop {
        let current = text.get_native_index();
        if current >= range_end {
            break;
        }
        let mut cp_word_length = 0;
        let mut cu_word_length = 0;

        // Look for candidate words at the current position
        let candidates = words[w(words_found)].candidates(text, dict, range_end);

        // If we found exactly one, use that
        if candidates == 1 {
            cu_word_length = words[w(words_found)].accept_marked(text);
            cp_word_length = words[w(words_found)].marked_cp_length();
            words_found = words_found.wrapping_add(1);
        }
        // If there was more than one, see which one can take us forward the most words
        else if candidates > 1 {
            'found_best: {
                // If we're already at the end of the range, we're done
                if text.get_native_index() >= range_end {
                    break 'found_best;
                }
                loop {
                    if words[w(words_found.wrapping_add(1))].candidates(text, dict, range_end) > 0 {
                        // Followed by another dictionary word; mark first word as a good candidate
                        words[w(words_found)].mark_current();

                        // If we're already at the end of the range, we're done
                        if text.get_native_index() >= range_end {
                            break 'found_best;
                        }

                        // See if any of the possible second words is followed by a third word
                        loop {
                            // If we find a third word, stop right away
                            if words[w(words_found.wrapping_add(2))]
                                .candidates(text, dict, range_end)
                                != 0
                            {
                                words[w(words_found)].mark_current();
                                break 'found_best;
                            }
                            if !words[w(words_found.wrapping_add(1))].back_up(text) {
                                break;
                            }
                        }
                    }
                    if !words[w(words_found)].back_up(text) {
                        break;
                    }
                }
            }
            // foundBest: Set UText position to after the accepted word.
            cu_word_length = words[w(words_found)].accept_marked(text);
            cp_word_length = words[w(words_found)].marked_cp_length();
            words_found = words_found.wrapping_add(1);
        }

        // We come here after having either found a word or not. We look ahead to the
        // next word. If it's not a dictionary word, we will combine it with the word we
        // just found (if there is one), but only if the preceding word does not exceed
        // the threshold.
        // The text iterator should now be positioned at the end of the word we found.

        // (`UChar32 uc = 0;`: every read follows an assignment.)
        let mut uc: i32;
        if text.get_native_index() < range_end && cp_word_length < p.root_combine_threshold {
            // if it is a dictionary word, do nothing. If it isn't, then if there is
            // no preceding word, or the non-word shares less than the minimum threshold
            // of characters with a dictionary word, then scan to resynchronize
            if words[w(words_found)].candidates(text, dict, range_end) <= 0
                && (cu_word_length == 0
                    || words[w(words_found)].longest_prefix() < p.prefix_combine_threshold)
            {
                // Look for a plausible word boundary
                let mut remaining = range_end - (current + cu_word_length);
                let mut chars = 0;
                loop {
                    let pc_index = text.get_native_index();
                    let pc = text.next32();
                    let pc_size = text.get_native_index() - pc_index;
                    chars += pc_size;
                    remaining -= pc_size;
                    if remaining <= 0 {
                        break;
                    }
                    uc = text.current32();
                    if kind.end_word_set(pc) && kind.begin_word_set(uc) {
                        // Maybe. See if it's in the dictionary.
                        let num_candidates =
                            words[w(words_found.wrapping_add(1))].candidates(text, dict, range_end);
                        text.set_native_index(current + cu_word_length + chars);
                        if num_candidates > 0 {
                            break;
                        }
                    }
                }

                // Bump the word count if there wasn't already one
                if cu_word_length <= 0 {
                    words_found = words_found.wrapping_add(1);
                }

                // Update the length with the passed-over characters
                cu_word_length += chars;
            } else {
                // Back up to where we were for next iteration
                text.set_native_index(current + cu_word_length);
            }
        }

        // Never stop before a combining mark.
        loop {
            let curr_pos = text.get_native_index();
            if !(curr_pos < range_end && kind.mark_set(text.current32())) {
                break;
            }
            text.next32();
            cu_word_length += text.get_native_index() - curr_pos;
        }

        // Look ahead for possible suffixes if a dictionary word does not follow.
        // We do this in code rather than using a rule so that the heuristic
        // resynch continues to function. For example, one of the suffix characters
        // could be a typo in the middle of a word.
        if p.suffixes && text.get_native_index() < range_end && cu_word_length > 0 {
            if words[w(words_found)].candidates(text, dict, range_end) <= 0 && {
                uc = text.current32();
                EngineKind::suffix_set(uc)
            } {
                if uc == THAI_PAIYANNOI {
                    if EngineKind::suffix_set(text.previous32()) {
                        // Restore prior position
                        text.next32();
                    } else {
                        // Skip over previous end and PAIYANNOI
                        text.next32();
                        let paiyannoi_index = text.get_native_index();
                        text.next32();
                        // Add PAIYANNOI to word
                        cu_word_length += text.get_native_index() - paiyannoi_index;
                        uc = text.current32(); // Fetch next character
                    }
                }
                if uc == THAI_MAIYAMOK {
                    if text.previous32() == THAI_MAIYAMOK {
                        // Restore prior position
                        text.next32();
                    } else {
                        // Skip over previous end and MAIYAMOK
                        text.next32();
                        let maiyamok_index = text.get_native_index();
                        text.next32();
                        // Add MAIYAMOK to word
                        cu_word_length += text.get_native_index() - maiyamok_index;
                    }
                }
            } else {
                text.set_native_index(current + cu_word_length);
            }
        }

        // Did we find a word on this iteration? If so, push it on the break stack
        if cu_word_length > 0 {
            found_breaks.push(current + cu_word_length);
        }
    }

    // Don't return a break for the end of the dictionary range if there is one there.
    if peeki(found_breaks) >= range_end {
        found_breaks.pop();
        words_found = words_found.wrapping_sub(1);
    }

    words_found as i32
}

// --------------------------------------------------------------------------------------------
// CjkBreakEngine

// Port of: icu4c/source/common/dictbe.cpp#L1102-L1121 (chromium/deps/icu d578f2e8)
const K_UINT32_MAX: u32 = 0xFFFF_FFFF;
/// The katakanaCost values below are based on the length frequencies of all katakana phrases in
/// the dictionary.
const K_MAX_KATAKANA_LENGTH: i32 = 8;
const K_MAX_KATAKANA_GROUP_LENGTH: i32 = 20;
const MAX_SNLP: i32 = 255;

fn get_katakana_cost(word_length: i32) -> u32 {
    const KATAKANA_COST: [u32; K_MAX_KATAKANA_LENGTH as usize + 1] =
        [8192, 984, 408, 240, 204, 252, 300, 372, 480];
    if word_length > K_MAX_KATAKANA_LENGTH {
        8192
    } else {
        KATAKANA_COST[usize::try_from(word_length).expect("a run length")]
    }
}

fn is_katakana(value: i32) -> bool {
    ((0x30A1..=0x30FE).contains(&value) && value != 0x30FB) || (0xFF66..=0xFF9F).contains(&value)
}

/// `fHangulWordSet`: `[가-힣]`.
fn is_hangul_word(c: i32) -> bool {
    (0xAC00..=0xD7A3).contains(&c)
}

/// A `UnicodeString`: UTF-16 units.
struct UString(Vec<u16>);

impl UString {
    fn len(&self) -> i32 {
        i32::try_from(self.0.len()).expect("ICU string lengths are int32")
    }

    /// `append(UChar32)`.
    fn append(&mut self, c: i32) {
        let c = u32::try_from(c).expect("a code point");
        if c <= 0xffff {
            self.0.push(u16::try_from(c).expect("BMP"));
        } else {
            let c = c - 0x10000;
            self.0
                .push(u16::try_from(0xD800 + (c >> 10)).expect("lead"));
            self.0
                .push(u16::try_from(0xDC00 + (c & 0x3ff)).expect("trail"));
        }
    }

    /// `char32At(offset)`: the code point that holds `offset`.
    fn char32_at(&self, offset: i32) -> i32 {
        let Ok(i) = usize::try_from(offset) else {
            return 0xffff;
        };
        let Some(&u) = self.0.get(i) else {
            return 0xffff;
        };
        let u32_ = i32::from(u);
        if (0xD800..0xDC00).contains(&u32_) {
            if let Some(&t) = self.0.get(i + 1)
                && (0xDC00..0xE000).contains(&t)
            {
                return ((u32_ - 0xD800) << 10) + (i32::from(t) - 0xDC00) + 0x10000;
            }
        } else if (0xDC00..0xE000).contains(&u32_)
            && i > 0
            && (0xD800..0xDC00).contains(&self.0[i - 1])
        {
            return ((i32::from(self.0[i - 1]) - 0xD800) << 10) + (u32_ - 0xDC00) + 0x10000;
        }
        u32_
    }

    /// `moveIndex32(index, 1)`.
    fn move_index32_forward(&self, index: i32) -> i32 {
        let len = self.len();
        if index >= len {
            return len;
        }
        let i = usize::try_from(index).expect("non-negative");
        if (0xD800..0xDC00).contains(&self.0[i])
            && self
                .0
                .get(i + 1)
                .is_some_and(|t| (0xDC00..0xE000).contains(t))
        {
            index + 2
        } else {
            index + 1
        }
    }

    /// `countChar32()`.
    fn count_char32(&self) -> i32 {
        let mut n = 0;
        let mut i = 0;
        while i < self.len() {
            i = self.move_index32_forward(i);
            n += 1;
        }
        n
    }
}

// Port of: icu4c/source/common/normalizer2impl.cpp Normalizer2Impl::hasCompBoundaryBefore
// (chromium/deps/icu d578f2e8)
/// `nfkcNorm2->hasBoundaryBefore(c)` for a character of the engine's set: whether the first
/// character of its NFKD decomposition is a starter (`ccc` 0). ICU also requires that starter not
/// to combine backwards; no character of the set decomposes to a starter that does (the test
/// `cj_set_decomposes_to_forward_starters` checks every one), so for them the two agree.
fn nfkc_has_boundary_before(c: i32) -> bool {
    let Some(ch) = u32::try_from(c).ok().and_then(char::from_u32) else {
        return true;
    };
    let first = DecomposingNormalizerBorrowed::new_nfkd()
        .normalize_iter(core::iter::once(ch))
        .next()
        .unwrap_or(ch);
    CanonicalCombiningClassMapBorrowed::new().get_u8(first) == 0
}

// Port of: icu4c/source/common/dictbe.cpp#L1137-L1438 (CjkBreakEngine::divideUpDictionaryRange,
// without the phrase-breaking branches) (chromium/deps/icu d578f2e8)
// One function in ICU; `cuIdx` / `cpIdx` are ICU's names.
#[allow(clippy::too_many_lines, clippy::similar_names)]
fn divide_cj(
    dict: &DictionaryMatcher,
    in_text: &mut UText,
    range_start: i32,
    range_end: i32,
    found_breaks: &mut Vec<i32>,
) -> i32 {
    if range_start >= range_end {
        return 0;
    }

    // UnicodeString version of input UText, NFKC normalized if necessary.
    let mut in_string = UString(Vec::new());

    // inputMap[inStringIndex] = corresponding native index from UText inText.
    // If None then mapping is 1:1
    let mut input_map: Option<Vec<i32>> = None;

    // if UText has the input string as one contiguous UTF-16 chunk
    if in_text.is_utf16() {
        // Input UText is in one contiguous UTF-16 chunk: its units over the range (the code
        // points, unpaired surrogates included, re-encode to the same units).
        in_text.set_native_index(range_start);
        while in_text.get_native_index() < range_end {
            let c = in_text.next32();
            in_string.append(c);
        }
    } else {
        // Copy the text from the original inText (UText) to inString (UnicodeString).
        // Create a map from UnicodeString indices -> UText offsets.
        in_text.set_native_index(range_start);
        let limit = range_end.min(in_text.native_length());
        let mut map = Vec::new();
        while in_text.get_native_index() < limit {
            let native_position = in_text.get_native_index();
            let c = in_text.next32();
            debug_assert_ne!(c, U_SENTINEL);
            in_string.append(c);
            while (map.len() as i32) < in_string.len() {
                map.push(native_position);
            }
        }
        map.push(limit);
        input_map = Some(map);
    }

    let nfkc = ComposingNormalizerBorrowed::new_nfkc();
    if !nfkc.is_normalized_utf16(&in_string.0) {
        let mut normalized_input = UString(Vec::new());
        // normalizedMap[normalizedInput position] ==  original UText position.
        let mut normalized_map: Vec<i32> = Vec::new();

        let mut src_i = 0;
        while src_i < in_string.len() {
            // Once per normalization chunk
            let mut fragment = UString(Vec::new());
            let fragment_start_i = src_i;
            let mut c = in_string.char32_at(src_i);
            loop {
                fragment.append(c);
                src_i = in_string.move_index32_forward(src_i);
                if src_i == in_string.len() {
                    break;
                }
                c = in_string.char32_at(src_i);
                if nfkc_has_boundary_before(c) {
                    break;
                }
            }
            normalized_input
                .0
                .extend_from_slice(&nfkc.normalize_utf16(&fragment.0));

            // Map every position in the normalized chunk to the start of the chunk
            //   in the original input.
            let fragment_original_start = match &input_map {
                Some(map) => element_ati(map, fragment_start_i),
                None => fragment_start_i + range_start,
            };
            while (normalized_map.len() as i32) < normalized_input.len() {
                normalized_map.push(fragment_original_start);
            }
        }
        let native_end = match &input_map {
            Some(map) => element_ati(map, in_string.len()),
            None => in_string.len() + range_start,
        };
        normalized_map.push(native_end);

        input_map = Some(normalized_map);
        in_string = normalized_input;
    }

    let num_code_pts = in_string.count_char32();
    if num_code_pts != in_string.len() {
        // There are supplementary characters in the input.
        // The dictionary will produce boundary positions in terms of code point indexes,
        //   not in terms of code unit string indexes.
        // Use the inputMap mechanism to take care of this in addition to indexing differences
        //    from normalization and/or UTF-8 input.
        let had_existing_map = input_map.is_some();
        let map = input_map.get_or_insert_with(Vec::new);
        let mut cp_idx = 0;
        let mut cu_idx = 0;
        loop {
            if had_existing_map {
                let v = element_ati(map, cu_idx);
                set_element_at(map, v, cp_idx);
            } else {
                map.push(cu_idx + range_start);
            }
            cp_idx += 1;
            if cu_idx == in_string.len() {
                break;
            }
            cu_idx = in_string.move_index32_forward(cu_idx);
        }
    }

    let n = usize::try_from(num_code_pts).expect("non-negative");
    // bestSnlp[i] is the snlp of the best segmentation of the first i
    // code points in the range to be matched.
    let mut best_snlp = vec![K_UINT32_MAX; n + 1];
    best_snlp[0] = 0;

    // prev[i] is the index of the last CJK code point in the previous word in
    // the best segmentation of the first i characters.
    let mut prev: Vec<i32> = vec![-1; n + 1];

    let max_word_size = 20;
    let mut values = vec![0; n];
    let mut lengths = vec![0; n];

    let mut fu = UText::open_utf16(&in_string.0);

    // Dynamic programming to find the best segmentation.

    // In outer loop, i  is the code point index,
    //                ix is the corresponding string (code unit) index.
    //    They differ when the string contains supplementary characters.
    let mut ix = 0;
    let mut is_prev_katakana = false;
    let snlp_at = |v: &[u32], i: i32| -> u32 {
        usize::try_from(i)
            .ok()
            .and_then(|i| v.get(i))
            .copied()
            .unwrap_or(0)
    };
    for i in 0..num_code_pts {
        let iu = usize::try_from(i).expect("non-negative");
        if best_snlp[iu] != K_UINT32_MAX {
            fu.set_native_index(ix);
            let mut count = dict.matches(
                &mut fu,
                max_word_size,
                num_code_pts,
                MatchOutput {
                    lengths: None,
                    cp_lengths: Some(&mut lengths),
                    values: Some(&mut values),
                    prefix: None,
                },
            );
            // Note: lengths is filled with code point lengths
            //       The nullptr parameter is the ignored code unit lengths.

            // if there are no single character matches found in the dictionary
            // starting with this character, treat character as a 1-character word
            // with the highest value possible, i.e. the least likely to occur.
            // Exclude Korean characters from this treatment, as they should be left
            // together by default.
            if (count == 0 || element_ati(&lengths, 0) != 1)
                && !is_hangul_word(in_string.char32_at(ix))
            {
                set_element_at(&mut values, MAX_SNLP, count);
                set_element_at(&mut lengths, 1, count);
                count += 1;
            }

            for j in 0..count {
                let new_snlp = best_snlp[iu].wrapping_add(element_ati(&values, j) as u32);
                let ln_j_i = element_ati(&lengths, j) + i;
                if new_snlp < snlp_at(&best_snlp, ln_j_i) {
                    let k = usize::try_from(ln_j_i).expect("in range");
                    best_snlp[k] = new_snlp;
                    prev[k] = i;
                }
            }

            // In Japanese,
            // Katakana word in single character is pretty rare. So we apply
            // the following heuristic to Katakana: any continuous run of Katakana
            // characters is considered a candidate word with a default cost
            // specified in the katakanaCost table according to its length.

            let is_katakana_ = is_katakana(in_string.char32_at(ix));
            let mut katakana_run_length = 1;
            if !is_prev_katakana && is_katakana_ {
                let mut j = in_string.move_index32_forward(ix);
                // Find the end of the continuous run of Katakana characters
                while j < in_string.len()
                    && katakana_run_length < K_MAX_KATAKANA_GROUP_LENGTH
                    && is_katakana(in_string.char32_at(j))
                {
                    j = in_string.move_index32_forward(j);
                    katakana_run_length += 1;
                }
                if katakana_run_length < K_MAX_KATAKANA_GROUP_LENGTH {
                    let new_snlp =
                        best_snlp[iu].wrapping_add(get_katakana_cost(katakana_run_length));
                    let k = iu + usize::try_from(katakana_run_length).expect("positive");
                    if new_snlp < best_snlp[k] {
                        best_snlp[k] = new_snlp;
                        prev[k] = i; // prev[j] = i;
                    }
                }
            }
            is_prev_katakana = is_katakana_;
        }
        ix = in_string.move_index32_forward(ix);
    }

    // Start pushing the optimal offset index into t_boundary (t for tentative).
    // prev[numCodePts] is guaranteed to be meaningful.
    // We'll first push in the reverse order, i.e.,
    // t_boundary[0] = numCodePts, and afterwards do a swap.
    let mut t_boundary: Vec<i32> = Vec::with_capacity(n + 1);

    let mut num_breaks = 0;
    // No segmentation found, set boundary to end of range
    if best_snlp[n] == K_UINT32_MAX {
        t_boundary.push(num_code_pts);
        num_breaks += 1;
    } else {
        let mut i = num_code_pts;
        while i > 0 {
            t_boundary.push(i);
            num_breaks += 1;
            i = element_ati(&prev, i);
        }
        debug_assert_eq!(
            element_ati(&prev, element_ati(&t_boundary, num_breaks - 1)),
            0
        );
    }

    // Add a break for the start of the dictionary range if there is not one
    // there already.
    if found_breaks.is_empty() || peeki(found_breaks) < range_start {
        t_boundary.push(0);
        num_breaks += 1;
    }

    // Now that we're done, convert positions in t_boundary[] (indices in
    // the normalized input string) back to indices in the original input UText
    // while reversing t_boundary and pushing values to foundBreaks.
    let mut prev_utext_pos = -1;
    let mut corrected_num_breaks = 0;
    let mut i = num_breaks - 1;
    while i >= 0 {
        let cp_pos = element_ati(&t_boundary, i);
        let utext_pos = match &input_map {
            Some(map) => element_ati(map, cp_pos),
            None => cp_pos + range_start,
        };
        if utext_pos > prev_utext_pos {
            // Boundaries are added to foundBreaks output in ascending order.
            if utext_pos != range_start {
                found_breaks.push(utext_pos);
                corrected_num_breaks += 1;
            }
        } else {
            // Normalization expanded the input text, the dictionary found a boundary
            // within the expansion, giving two boundaries with the same index in the
            // original text. Ignore the second. See ticket #12918.
            // (`--numBreaks;`: not read again.)
        }
        prev_utext_pos = utext_pos;
        i -= 1;
    }

    // `UChar32 nextChar = utext_char32At(inText, rangeEnd);` (used only for phrase breaking; it
    // moves the text, which `findBreaks` then resets).
    in_text.char32_at(range_end);
    if !found_breaks.is_empty() && peeki(found_breaks) == range_end {
        found_breaks.pop();
        corrected_num_breaks -= 1;
    }

    corrected_num_breaks
}

// --------------------------------------------------------------------------------------------
// DictionaryBreakEngine

/// A dictionary engine: its kind and its dictionary.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DictionaryBreakEngine {
    pub(crate) kind: EngineKind,
    dict: DictionaryMatcher,
}

impl DictionaryBreakEngine {
    pub(crate) fn new(kind: EngineKind, dict: DictionaryMatcher) -> Self {
        DictionaryBreakEngine { kind, dict }
    }

    // Port of: icu4c/source/common/dictbe.cpp#L44-L79 (DictionaryBreakEngine::handles and
    // findBreaks) (chromium/deps/icu d578f2e8)
    /// `handles(c, locale)`.
    pub(crate) fn handles(&self, c: i32) -> bool {
        self.kind.handles(c)
    }

    /// `findBreaks(text, startPos, endPos, foundBreaks, isPhraseBreaking = false, status)`.
    pub(crate) fn find_breaks(
        &self,
        text: &mut UText,
        start_pos: i32,
        end_pos: i32,
        found_breaks: &mut Vec<i32>,
    ) -> i32 {
        // Find the span of characters included in the set.
        //   The span to break begins at the current position in the text, and
        //   extends towards the start or end of the text, depending on 'reverse'.
        text.set_native_index(start_pos);
        let start = text.get_native_index();
        let mut current;
        let mut c = text.current32();
        loop {
            current = text.get_native_index();
            if !(current < end_pos && self.kind.handles(c)) {
                break;
            }
            text.next32();
            c = text.current32();
        }
        let range_start = start;
        let range_end = current;
        let result = match self.kind {
            EngineKind::ChineseJapanese => {
                divide_cj(&self.dict, text, range_start, range_end, found_breaks)
            }
            kind => divide_sea(kind, &self.dict, text, range_start, range_end, found_breaks),
        };
        text.set_native_index(current);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use icu_normalizer::properties::Decomposed;
    use icu_normalizer::properties::{
        CanonicalCompositionBorrowed, CanonicalDecompositionBorrowed,
    };

    /// The premise of [`nfkc_has_boundary_before`]: no character of the CJ engine's set
    /// decomposes (NFKD) to a first character that combines backwards (is the second character
    /// of a canonical composition).
    #[test]
    fn cj_set_decomposes_to_forward_starters() {
        let decompose = CanonicalDecompositionBorrowed::new();
        let compose = CanonicalCompositionBorrowed::new();
        let mut backward = std::collections::HashSet::new();
        for c in (0..=0x10_FFFF).filter_map(char::from_u32) {
            if let Decomposed::Expansion(a, b) = decompose.decompose(c)
                && compose.compose(a, b) == Some(c)
            {
                backward.insert(b);
            }
        }
        assert!(backward.contains(&'\u{3099}'));
        let nfkd = DecomposingNormalizerBorrowed::new_nfkd();
        for c in (0..=0x10_FFFF).filter_map(char::from_u32) {
            if EngineKind::ChineseJapanese.handles(c as i32) {
                let first = nfkd.normalize_iter(core::iter::once(c)).next().unwrap();
                // (U+FF9E and U+FF9F decompose to U+3099 and U+309A, which are not starters.)
                if CanonicalCombiningClassMapBorrowed::new().get_u8(first) == 0 {
                    assert!(!backward.contains(&first), "U+{:04X}", c as u32);
                }
            }
        }
    }
}
