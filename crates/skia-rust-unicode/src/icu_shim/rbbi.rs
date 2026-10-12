// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `RuleBasedBreakIterator` (`rbbi.h`, `rbbi.cpp`, `rbbi_cache.h`, `rbbi_cache.cpp`, and the
//! engine lookup of `brkeng.cpp`): ICU's break iterator over a compiled rule file, with its
//! boundary cache, its dictionary cache and its dictionary engines (`super::dictbe`).
//!
//! The shim runs this over Chromium's compiled data (`skia-rust-unicode-data`), so boundaries, rule
//! status and navigation are ICU's by construction (milestone M4, `docs/design/modules.md` §3.6).
//!
//! One difference in state: ICU keeps the dictionary engines it has created in a process-wide list
//! (`ICULanguageBreakFactory::fEngines`) and reuses any of them whose character set holds a
//! character before it creates one for the character's script. The shim has no process-wide state;
//! it behaves as a process that has created every engine (see
//! [`RuleBasedBreakIterator::language_break_engine`]). The two differ only for the four characters
//! of the Chinese/Japanese set whose script is Common (U+30FC, U+FF70, U+FF9E, U+FF9F) in a process
//! that has not yet met a Han, Hiragana or Katakana dictionary character.

// The iterator mirrors ICU's `int32_t` positions and indexes.
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]

use super::dictbe::{DictionaryBreakEngine, EngineKind, element_ati, peeki};
use super::dictionary_data::DictionaryMatcher;
use super::rbbi_data::{RBBI_BOF_REQUIRED, RbbiData, RowField};
use super::ubrk::{TextRef, UBRK_DONE};
use super::uchar::uscript_get_script;
use super::utext::{U_SENTINEL, UText};

// Port of: icu4c/source/common/rbbi.cpp#L48-L51 (chromium/deps/icu d578f2e8)
const START_STATE: u16 = 1;
const STOP_STATE: u16 = 0;
/// `ACCEPTING_UNCONDITIONAL`.
const ACCEPTING_UNCONDITIONAL: u16 = 1;

// Port of: icu4c/source/common/rbbi_cache.h#L181-L184 (chromium/deps/icu d578f2e8)
const CACHE_SIZE: i32 = 128;

fn mod_chunk_size(index: i32) -> i32 {
    index & (CACHE_SIZE - 1)
}

/// The dictionaries of the data a break iterator was opened with, by [`EngineKind`].
pub(crate) type Dictionaries = [Option<&'static [u8]>; 5];

fn kind_index(kind: EngineKind) -> usize {
    EngineKind::ALL
        .iter()
        .position(|&k| k == kind)
        .expect("every kind is in ALL")
}

/// `RBBIRunMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunMode {
    /// `RBBI_START`: state machine processing is before first char of input.
    Start,
    /// `RBBI_RUN`: state machine processing is in the user text.
    Run,
    /// `RBBI_END`: state machine processing is after end of user text.
    End,
}

/// `UnhandledEngine` (`brkeng.cpp#L52-L95`): takes the characters no dictionary handles.
#[derive(Clone, Copy, Debug, Default)]
struct UnhandledEngine {
    /// `fHandled`: the script whose characters it holds (`applyIntPropertyValue(UCHAR_SCRIPT, ...)`
    /// replaces the set), or `None` while it is empty.
    handled: Option<i32>,
}

impl UnhandledEngine {
    // Port of: icu4c/source/common/brkeng.cpp#L60-L95 (chromium/deps/icu d578f2e8)
    fn handles(self, c: i32) -> bool {
        c >= 0 && self.handled == Some(uscript_get_script(c))
    }

    fn find_breaks(self, text: &mut UText, start_pos: i32, end_pos: i32) -> i32 {
        text.set_native_index(start_pos);
        let mut c = text.current32();
        while text.get_native_index() < end_pos && self.handles(c) {
            text.next32();
            c = text.current32();
        }
        0
    }

    fn handle_character(&mut self, c: i32) {
        if !self.handles(c) {
            // Apply the entire script of the character.
            self.handled = Some(uscript_get_script(c));
        }
    }
}

/// An entry of `fLanguageBreakEngines`.
#[derive(Clone, Copy, Debug)]
enum LanguageBreakEngine {
    Dictionary(DictionaryBreakEngine),
    /// `fUnhandledBreakEngine`.
    Unhandled,
}

/// `RuleBasedBreakIterator::DictionaryCache`.
#[derive(Clone, Debug, Default)]
struct DictionaryCache {
    /// `fBreaks`: a vector of break positions.
    breaks: Vec<i32>,
    /// `fPositionInCache`: index in `fBreaks` of last boundary returned by `following()` or
    /// `preceding()`; -1 if the cache is not active.
    position_in_cache: i32,
    /// `fStart`: text segment start (inclusive).
    start: i32,
    /// `fLimit`: text segment limit (exclusive).
    limit: i32,
    /// `fFirstRuleStatusIndex`: rule status info for first boundary.
    first_rule_status_index: i32,
    /// `fOtherRuleStatusIndex`: rule status info for 2nd through last boundaries.
    other_rule_status_index: i32,
}

impl DictionaryCache {
    // Port of: icu4c/source/common/rbbi_cache.cpp#L36-L43 (chromium/deps/icu d578f2e8)
    fn reset(&mut self) {
        self.position_in_cache = -1;
        self.start = 0;
        self.limit = 0;
        self.first_rule_status_index = 0;
        self.other_rule_status_index = 0;
        self.breaks.clear();
    }

    fn size(&self) -> i32 {
        self.breaks.len() as i32
    }

    // Port of: icu4c/source/common/rbbi_cache.cpp#L45-L77 (chromium/deps/icu d578f2e8)
    /// `following(fromPos, &result, &statusIndex)`.
    fn following(&mut self, from_pos: i32) -> Option<(i32, i32)> {
        if from_pos >= self.limit || from_pos < self.start {
            self.position_in_cache = -1;
            return None;
        }

        // Sequential iteration, move from previous boundary to the following

        if self.position_in_cache >= 0
            && self.position_in_cache < self.size()
            && element_ati(&self.breaks, self.position_in_cache) == from_pos
        {
            self.position_in_cache += 1;
            if self.position_in_cache >= self.size() {
                self.position_in_cache = -1;
                return None;
            }
            let r = element_ati(&self.breaks, self.position_in_cache);
            debug_assert!(r > from_pos);
            return Some((r, self.other_rule_status_index));
        }

        // Random indexing. Linear search for the boundary following the given position.

        self.position_in_cache = 0;
        while self.position_in_cache < self.size() {
            let r = element_ati(&self.breaks, self.position_in_cache);
            if r > from_pos {
                return Some((r, self.other_rule_status_index));
            }
            self.position_in_cache += 1;
        }
        unreachable!("UPRV_UNREACHABLE_EXIT: no dictionary boundary after {from_pos}");
    }

    // Port of: icu4c/source/common/rbbi_cache.cpp#L80-L117 (chromium/deps/icu d578f2e8)
    /// `preceding(fromPos, &result, &statusIndex)`.
    fn preceding(&mut self, from_pos: i32) -> Option<(i32, i32)> {
        if from_pos <= self.start || from_pos > self.limit {
            self.position_in_cache = -1;
            return None;
        }

        if from_pos == self.limit {
            self.position_in_cache = self.size() - 1;
        }

        let status = |r: i32, c: &DictionaryCache| {
            if r == c.start {
                c.first_rule_status_index
            } else {
                c.other_rule_status_index
            }
        };
        if self.position_in_cache > 0
            && self.position_in_cache < self.size()
            && element_ati(&self.breaks, self.position_in_cache) == from_pos
        {
            self.position_in_cache -= 1;
            let r = element_ati(&self.breaks, self.position_in_cache);
            debug_assert!(r < from_pos);
            return Some((r, status(r, self)));
        }

        if self.position_in_cache == 0 {
            self.position_in_cache = -1;
            return None;
        }

        self.position_in_cache = self.size() - 1;
        while self.position_in_cache >= 0 {
            let r = element_ati(&self.breaks, self.position_in_cache);
            if r < from_pos {
                return Some((r, status(r, self)));
            }
            self.position_in_cache -= 1;
        }
        unreachable!("UPRV_UNREACHABLE_EXIT: no dictionary boundary before {from_pos}");
    }
}

/// `RuleBasedBreakIterator::BreakCache`: a circular buffer of boundaries around the iteration
/// position.
#[derive(Clone, Debug)]
struct BreakCache {
    /// `fStartBufIdx`: index of first valid entry.
    start_buf_idx: i32,
    /// `fEndBufIdx`: index of the last valid entry. Inclusive.
    end_buf_idx: i32,
    /// `fTextIdx`: text position of the current boundary.
    text_idx: i32,
    /// `fBufIdx`: index of the current boundary in the buffer.
    buf_idx: i32,
    /// `fBoundaries`.
    boundaries: [i32; CACHE_SIZE as usize],
    /// `fStatuses`.
    statuses: [u16; CACHE_SIZE as usize],
    /// `fSideBuffer`.
    side_buffer: Vec<i32>,
}

impl BreakCache {
    fn new() -> Self {
        let mut c = BreakCache {
            start_buf_idx: 0,
            end_buf_idx: 0,
            text_idx: 0,
            buf_idx: 0,
            boundaries: [0; CACHE_SIZE as usize],
            statuses: [0; CACHE_SIZE as usize],
            side_buffer: Vec::new(),
        };
        c.reset(0, 0);
        c
    }

    fn b(&self, i: i32) -> i32 {
        self.boundaries[i as usize]
    }

    fn s(&self, i: i32) -> i32 {
        i32::from(self.statuses[i as usize])
    }

    // Port of: icu4c/source/common/rbbi_cache.cpp#L211-L218 (chromium/deps/icu d578f2e8)
    fn reset(&mut self, pos: i32, rule_status: i32) {
        self.start_buf_idx = 0;
        self.end_buf_idx = 0;
        self.text_idx = pos;
        self.buf_idx = 0;
        self.boundaries[0] = pos;
        self.statuses[0] = rule_status as u16;
    }

    // Port of: icu4c/source/common/rbbi_cache.cpp#L304-L332 (chromium/deps/icu d578f2e8)
    fn seek(&mut self, pos: i32) -> bool {
        if pos < self.b(self.start_buf_idx) || pos > self.b(self.end_buf_idx) {
            return false;
        }
        if pos == self.b(self.start_buf_idx) {
            // Common case: seek(0), from BreakIterator::first()
            self.buf_idx = self.start_buf_idx;
            self.text_idx = self.b(self.buf_idx);
            return true;
        }
        if pos == self.b(self.end_buf_idx) {
            self.buf_idx = self.end_buf_idx;
            self.text_idx = self.b(self.buf_idx);
            return true;
        }

        let mut min = self.start_buf_idx;
        let mut max = self.end_buf_idx;
        while min != max {
            let mut probe = (min + max + if min > max { CACHE_SIZE } else { 0 }) / 2;
            probe = mod_chunk_size(probe);
            if self.b(probe) > pos {
                max = probe;
            } else {
                min = mod_chunk_size(probe + 1);
            }
        }
        debug_assert!(self.b(max) > pos);
        self.buf_idx = mod_chunk_size(max - 1);
        self.text_idx = self.b(self.buf_idx);
        debug_assert!(self.text_idx <= pos);
        true
    }

    // Port of: icu4c/source/common/rbbi_cache.cpp#L645-L664 (chromium/deps/icu d578f2e8)
    /// `addFollowing(position, ruleStatusIdx, update)`.
    fn add_following(&mut self, position: i32, rule_status_idx: i32, update_cache_position: bool) {
        debug_assert!(position > self.b(self.end_buf_idx));
        let next_idx = mod_chunk_size(self.end_buf_idx + 1);
        if next_idx == self.start_buf_idx {
            self.start_buf_idx = mod_chunk_size(self.start_buf_idx + 6); // TODO: experiment. Probably revert to 1.
        }
        self.boundaries[next_idx as usize] = position;
        self.statuses[next_idx as usize] = rule_status_idx as u16;
        self.end_buf_idx = next_idx;
        if update_cache_position {
            // Set current position to the newly added boundary.
            self.buf_idx = next_idx;
            self.text_idx = position;
        } else {
            // Retaining the original cache position.
            // Check if the added boundary wraps around the buffer, and would over-write the original position.
            // It's the responsibility of callers of this function to not add too many.
            debug_assert_ne!(next_idx, self.buf_idx);
        }
    }

    // Port of: icu4c/source/common/rbbi_cache.cpp#L666-L687 (chromium/deps/icu d578f2e8)
    /// `addPreceding(position, ruleStatusIdx, update)`.
    fn add_preceding(
        &mut self,
        position: i32,
        rule_status_idx: i32,
        update_cache_position: bool,
    ) -> bool {
        debug_assert!(position < self.b(self.start_buf_idx));
        let next_idx = mod_chunk_size(self.start_buf_idx - 1);
        if next_idx == self.end_buf_idx {
            if self.buf_idx == self.end_buf_idx && !update_cache_position {
                // Failure. The insertion of the new boundary would claim the buffer position that is the
                // current iteration position. And we also want to retain the current iteration position.
                // (The buffer is already completely full of entries that precede the iteration position.)
                return false;
            }
            self.end_buf_idx = mod_chunk_size(self.end_buf_idx - 1);
        }
        self.boundaries[next_idx as usize] = position;
        self.statuses[next_idx as usize] = rule_status_idx as u16;
        self.start_buf_idx = next_idx;
        if update_cache_position {
            self.buf_idx = next_idx;
            self.text_idx = position;
        }
        true
    }
}

/// `RuleBasedBreakIterator`. See the module documentation.
#[derive(Debug)]
pub(crate) struct RuleBasedBreakIterator {
    /// `fData`.
    data: RbbiData,
    /// The dictionaries `ICULanguageBreakFactory` can load.
    dictionaries: Dictionaries,
    /// `fText`.
    text: UText,
    /// `fPosition`: the current position of the iterator.
    position: i32,
    /// `fRuleStatusIndex`: index of the rule status of the current boundary.
    rule_status_index: i32,
    /// `fDone`: true if the iterator has run off the start or end of the text.
    done: bool,
    /// `fDictionaryCharCount`: dictionary characters in the last rule-based segment.
    dictionary_char_count: i32,
    /// `fLookAheadMatches`.
    look_ahead_matches: Vec<i32>,
    /// `fBreakCache`.
    break_cache: BreakCache,
    /// `fDictionaryCache`.
    dictionary_cache: DictionaryCache,
    /// `fLanguageBreakEngines`: bottom first.
    language_break_engines: Vec<LanguageBreakEngine>,
    /// `fUnhandledBreakEngine`.
    unhandled_break_engine: Option<UnhandledEngine>,
}

impl Clone for RuleBasedBreakIterator {
    // Port of: icu4c/source/common/rbbi.cpp#L280-L340 (operator=, from a new iterator)
    // (chromium/deps/icu d578f2e8)
    /// `clone()`: the same data and text and position; the caches start over from the current
    /// boundary, and the language engines are rebuilt as needed.
    fn clone(&self) -> Self {
        let mut it = RuleBasedBreakIterator {
            data: self.data,
            dictionaries: self.dictionaries,
            text: self.text.clone(),
            position: self.position,
            rule_status_index: self.rule_status_index,
            done: self.done,
            dictionary_char_count: 0,
            look_ahead_matches: vec![-1; self.look_ahead_matches.len()],
            break_cache: BreakCache::new(),
            dictionary_cache: DictionaryCache::default(),
            language_break_engines: Vec::new(),
            unhandled_break_engine: None,
        };
        it.break_cache.reset(it.position, it.rule_status_index);
        it.dictionary_cache.reset();
        it
    }
}

impl RuleBasedBreakIterator {
    // Port of: icu4c/source/common/rbbi.cpp#L141-L165 and #L201-L229 (chromium/deps/icu d578f2e8)
    /// `RuleBasedBreakIterator(UDataMemory*, status)` over the empty text.
    pub(crate) fn new(data: RbbiData, dictionaries: Dictionaries) -> Self {
        let mut it = RuleBasedBreakIterator {
            data,
            dictionaries,
            text: UText::open(TextRef::Utf16(&[])),
            position: 0,
            rule_status_index: 0,
            done: false,
            dictionary_char_count: 0,
            look_ahead_matches: vec![-1; data.forward.look_ahead_results_size as usize],
            break_cache: BreakCache::new(),
            dictionary_cache: DictionaryCache::default(),
            language_break_engines: Vec::new(),
            unhandled_break_engine: None,
        };
        it.dictionary_cache.reset();
        it
    }

    // Port of: icu4c/source/common/rbbi.cpp#L406-L430 (chromium/deps/icu d578f2e8)
    /// `setText(UText*)`: the iterator moves to the start.
    pub(crate) fn set_text(&mut self, text: TextRef<'_>) {
        self.break_cache.reset(0, 0);
        self.dictionary_cache.reset();
        self.text = UText::open(text);
        self.first();
    }

    // Port of: icu4c/source/common/rbbi.cpp#L541-L549 (chromium/deps/icu d578f2e8)
    /// `first()`.
    pub(crate) fn first(&mut self) -> i32 {
        if !self.cache_seek(0) {
            self.populate_near(0);
        }
        self.cache_current();
        debug_assert_eq!(self.position, 0);
        0
    }

    // Port of: icu4c/source/common/rbbi.cpp#L555-L562 (chromium/deps/icu d578f2e8)
    /// `last()`.
    pub(crate) fn last(&mut self) -> i32 {
        let end_pos = self.text.native_length();
        let end_should_be_boundary = self.is_boundary(end_pos); // Has side effect of setting iterator position.
        debug_assert!(end_should_be_boundary);
        debug_assert_eq!(self.position, end_pos);
        end_pos
    }

    // Port of: icu4c/source/common/rbbi.cpp#L593-L596 (chromium/deps/icu d578f2e8)
    /// `next()`.
    pub(crate) fn next(&mut self) -> i32 {
        self.cache_next();
        if self.done { UBRK_DONE } else { self.position }
    }

    // Port of: icu4c/source/common/rbbi.cpp#L606-L610 (chromium/deps/icu d578f2e8)
    /// `previous()`.
    pub(crate) fn previous(&mut self) -> i32 {
        self.cache_previous();
        if self.done { UBRK_DONE } else { self.position }
    }

    // Port of: icu4c/source/common/rbbi.cpp#L618-L635 (chromium/deps/icu d578f2e8)
    /// `following(startPos)`.
    pub(crate) fn following(&mut self, start_pos: i32) -> i32 {
        // if the supplied position is before the beginning, return the
        // text's starting offset
        if start_pos < 0 {
            return self.first();
        }

        // Move requested offset to a code point start. It might be on a trail surrogate,
        // or on a trail byte if the input is UTF-8. Or it may be beyond the end of the text.
        self.text.set_native_index(start_pos);
        let start_pos = self.text.get_native_index();

        self.cache_following(start_pos);
        if self.done { UBRK_DONE } else { self.position }
    }

    // Port of: icu4c/source/common/rbbi.cpp#L641-L656 (chromium/deps/icu d578f2e8)
    /// `preceding(offset)`.
    pub(crate) fn preceding(&mut self, offset: i32) -> i32 {
        if offset > self.text.native_length() {
            return self.last();
        }

        // Move requested offset to a code point start. It might be on a trail surrogate,
        // or on a trail byte if the input is UTF-8.

        self.text.set_native_index(offset);
        let adjusted_offset = self.text.get_native_index();

        self.cache_preceding(adjusted_offset);
        if self.done { UBRK_DONE } else { self.position }
    }

    // Port of: icu4c/source/common/rbbi.cpp#L665-L697 (chromium/deps/icu d578f2e8)
    /// `isBoundary(offset)`.
    pub(crate) fn is_boundary(&mut self, offset: i32) -> bool {
        // out-of-range indexes are never boundary positions
        if offset < 0 {
            self.first(); // For side effects on current position, tag values.
            return false;
        }

        // Adjust offset to be on a code point boundary and not beyond the end of the text.
        // Note that isBoundary() is always false for offsets that are not on code point boundaries.
        // But we still need the side effect of leaving iteration at the following boundary.

        self.text.set_native_index(offset);
        let adjusted_offset = self.text.get_native_index();

        let mut result = false;
        if self.cache_seek(adjusted_offset) || self.populate_near(adjusted_offset) {
            result = self.cache_current() == offset;
        }

        if result && adjusted_offset < offset && self.text.char32_at(offset) == U_SENTINEL {
            // Original offset is beyond the end of the text. Return false, it's not a boundary,
            // but the iteration position remains set to the end of the text, which is a boundary.
            return false;
        }
        if !result {
            // Not on a boundary. isBoundary() must leave iterator on the following boundary.
            // Cache->seek(), above, left us on the preceding boundary, so advance one.
            self.next();
        }
        result
    }

    // Port of: icu4c/source/common/rbbi.cpp#L703-L705 (chromium/deps/icu d578f2e8)
    /// `current()`.
    pub(crate) fn current(&self) -> i32 {
        self.position
    }

    // Port of: icu4c/source/common/rbbi.cpp#L1049-L1059 (chromium/deps/icu d578f2e8)
    /// `getRuleStatus()`: the last (largest) of the status values of the current boundary.
    pub(crate) fn get_rule_status(&self) -> i32 {
        let idx = self.rule_status_index as usize;
        let n = self.data.rule_status(idx);
        self.data.rule_status(idx + n as usize)
    }

    /// Whether the last move ran off the text (`fDone`).
    pub(crate) fn is_done(&self) -> bool {
        self.done
    }

    /// The trie value of `c`: `TrieFunc8` / `TrieFunc16`, or `ucptrie_get`.
    fn category(&self, c: i32) -> u16 {
        self.data.trie.get(c)
    }

    // Port of: icu4c/source/common/rbbi.cpp#L779-L956 (handleNext) (chromium/deps/icu d578f2e8)
    /// `handleNext()`: run the state machine to find a boundary.
    fn handle_next(&mut self) -> i32 {
        let statetable = self.data.forward;
        let dict_start = statetable.dict_categories_start;
        let mut category: u16 = 0;

        // handleNext always sets the break tag value.
        // Set the default for it.
        self.rule_status_index = 0;

        self.dictionary_char_count = 0;

        // if we're already at the end of the text, return DONE.
        let initial_position = self.position;
        self.text.set_native_index(initial_position);
        let mut result = initial_position;
        let mut c = self.text.next32();
        if c == U_SENTINEL {
            self.done = true;
            return UBRK_DONE;
        }

        //  Set the initial state for the state machine
        let mut state = START_STATE;

        let mut mode = RunMode::Run;
        if statetable.flags & RBBI_BOF_REQUIRED != 0 {
            category = 2;
            mode = RunMode::Start;
        }

        // loop until we reach the end of the text or transition to state 0
        //
        loop {
            if c == U_SENTINEL {
                // Reached end of input string.
                if mode == RunMode::End {
                    // We have already run the loop one last time with the
                    //   character set to the psueudo {eof} value.  Now it is time
                    //   to unconditionally bail out.
                    break;
                }
                // Run the loop one last time with the fake end-of-input character category.
                mode = RunMode::End;
                category = 1;
            }

            //
            // Get the char category.  An incoming category of 1 or 2 means that
            //      we are preset for doing the beginning or end of input, and
            //      that we shouldn't get a category from an actual text input character.
            //
            if mode == RunMode::Run {
                // look up the current character's character category, which tells us
                // which column in the state table to look at.
                category = self.category(c);
                self.dictionary_char_count += i32::from(u32::from(category) >= dict_start);
            }

            // State Transition - move machine to its next state
            //
            debug_assert!(u32::from(category) < self.data.cat_count);
            state = statetable.next_state(state, category);

            let accepting = statetable.field(state, RowField::Accepting);
            if accepting == ACCEPTING_UNCONDITIONAL {
                // Match found, common case.
                if mode != RunMode::Start {
                    result = self.text.get_native_index();
                }
                // Remember the break status (tag) values.
                self.rule_status_index = i32::from(statetable.field(state, RowField::TagsIdx));
            } else if accepting > ACCEPTING_UNCONDITIONAL {
                // Lookahead match is completed.
                debug_assert!(u32::from(accepting) < statetable.look_ahead_results_size);
                let lookahead_result = self.look_ahead_matches[usize::from(accepting)];
                if lookahead_result >= 0 {
                    self.rule_status_index = i32::from(statetable.field(state, RowField::TagsIdx));
                    self.position = lookahead_result;
                    return lookahead_result;
                }
            }

            // If we are at the position of the '/' in a look-ahead (hard break) rule;
            // record the current position, to be returned later, if the full rule matches.
            // TODO: Move this check before the previous check of fAccepting.
            //       This would enable hard-break rules with no following context.
            //       But there are line break test failures when trying this. Investigate.
            //       Issue ICU-20837
            let rule = statetable.field(state, RowField::LookAhead);
            debug_assert!(rule == 0 || rule > ACCEPTING_UNCONDITIONAL);
            if rule > ACCEPTING_UNCONDITIONAL {
                let pos = self.text.get_native_index();
                self.look_ahead_matches[usize::from(rule)] = pos;
            }

            if state == STOP_STATE {
                // This is the normal exit from the lookup state machine.
                // We have advanced through the string until it is certain that no
                //   longer match is possible, no matter what characters follow.
                break;
            }

            // Advance to the next character.
            // If this is a beginning-of-input loop iteration, don't advance
            //    the input position.  The next iteration will be processing the
            //    first real input character.
            if mode == RunMode::Run {
                c = self.text.next32();
            } else if mode == RunMode::Start {
                mode = RunMode::Run;
            }
        }

        // The state machine is done.  Check whether it found a match...

        // If the iterator failed to advance in the match engine, force it ahead by one.
        //   (This really indicates a defect in the break rules.  They should always match
        //    at least one character.)
        if result == initial_position {
            self.text.set_native_index(initial_position);
            self.text.next32();
            result = self.text.get_native_index();
            self.rule_status_index = 0;
        }

        // Leave the iterator at our result position.
        self.position = result;
        result
    }

    // Port of: icu4c/source/common/rbbi.cpp#L964-L1039 (handleSafePrevious)
    // (chromium/deps/icu d578f2e8)
    /// `handleSafePrevious(fromPosition)`: iterate backwards using the safe reverse rules.
    fn handle_safe_previous(&mut self, from_position: i32) -> i32 {
        let state_table = self.data.reverse;
        self.text.set_native_index(from_position);

        // if we're already at the start of the text, return DONE.
        if self.text.get_native_index() == 0 {
            return UBRK_DONE;
        }

        //  Set the initial state for the state machine
        let mut c = self.text.previous32();
        let mut state = START_STATE;

        // loop until we reach the start of the text or transition to state 0
        //
        while c != U_SENTINEL {
            // look up the current character's character category, which tells us
            // which column in the state table to look at.
            //
            //  Off the dictionary flag bit. For reverse iteration it is not used.
            let category = self.category(c);

            // State Transition - move machine to its next state
            //
            debug_assert!(u32::from(category) < self.data.cat_count);
            state = state_table.next_state(state, category);

            if state == STOP_STATE {
                // This is the normal exit from the lookup state machine.
                // Transition to state zero means we have found a safe point.
                break;
            }
            c = self.text.previous32();
        }

        // The state machine is done.  Check whether it found a match...
        self.text.get_native_index()
    }

    // --------------------------------------------------------------------------------------------
    // The language break engines.

    /// A dictionary engine of `kind`, if the data has its dictionary.
    fn dictionary_engine(&self, kind: EngineKind) -> Option<DictionaryBreakEngine> {
        let file = self.dictionaries[kind_index(kind)]?;
        let matcher = DictionaryMatcher::open(file).ok()?;
        Some(DictionaryBreakEngine::new(kind, matcher))
    }

    // Port of: icu4c/source/common/brkeng.cpp#L144-L171 (ICULanguageBreakFactory::getEngineFor)
    // and #L173-L240 (loadEngineFor) (chromium/deps/icu d578f2e8)
    /// `getLanguageBreakEngineFromFactory(c)`: an engine that handles `c` (the factory's list, here
    /// every engine whose dictionary the data has: see the module documentation), else the engine
    /// of `c`'s script. Chromium's data has no LSTM models (`lstm.patch`), so `loadEngineFor` goes
    /// straight to the dictionaries.
    fn factory_engine_for(&self, c: i32) -> Option<DictionaryBreakEngine> {
        for kind in EngineKind::ALL {
            if kind.handles(c)
                && let Some(engine) = self.dictionary_engine(kind)
            {
                return Some(engine);
            }
        }
        EngineKind::for_script(uscript_get_script(c)).and_then(|k| self.dictionary_engine(k))
    }

    // Port of: icu4c/source/common/rbbi.cpp#L1207-L1266 (getLanguageBreakEngine)
    // (chromium/deps/icu d578f2e8)
    /// `getLanguageBreakEngine(c, locale)`: find an appropriate engine for the character `c`.
    fn language_break_engine(&mut self, c: i32) -> LanguageBreakEngine {
        for &lbe in self.language_break_engines.iter().rev() {
            let handles = match lbe {
                LanguageBreakEngine::Dictionary(engine) => engine.handles(c),
                LanguageBreakEngine::Unhandled => self
                    .unhandled_break_engine
                    .is_some_and(|engine| engine.handles(c)),
            };
            if handles {
                return lbe;
            }
        }

        // No existing dictionary took the character. See if a factory wants to
        // give us a new LanguageBreakEngine for this character.
        if let Some(engine) = self.factory_engine_for(c) {
            // If we got one, use it and push it on our stack.
            let lbe = LanguageBreakEngine::Dictionary(engine);
            self.language_break_engines.push(lbe);
            return lbe;
        }

        // No engine is forthcoming for this character. Add it to the
        // reject set. Create the reject break engine if needed.
        if self.unhandled_break_engine.is_none() {
            self.unhandled_break_engine = Some(UnhandledEngine::default());
            // Put it last so that scripts for which we have an engine get tried
            // first.
            self.language_break_engines
                .insert(0, LanguageBreakEngine::Unhandled);
        }

        // Tell the reject engine about the character; at its discretion, it may
        // add more than just the one character.
        if let Some(engine) = self.unhandled_break_engine.as_mut() {
            engine.handle_character(c);
        }

        LanguageBreakEngine::Unhandled
    }

    // --------------------------------------------------------------------------------------------
    // DictionaryCache

    // Port of: icu4c/source/common/rbbi_cache.cpp#L119-L198 (chromium/deps/icu d578f2e8)
    /// `DictionaryCache::populateDictionary(startPos, endPos, firstRuleStatus, otherRuleStatus)`.
    fn populate_dictionary(
        &mut self,
        start_pos: i32,
        end_pos: i32,
        first_rule_status: i32,
        other_rule_status: i32,
    ) {
        if (end_pos - start_pos) <= 1 {
            return;
        }

        self.dictionary_cache.reset();
        self.dictionary_cache.first_rule_status_index = first_rule_status;
        self.dictionary_cache.other_rule_status_index = other_rule_status;

        let range_start = start_pos;
        let range_end = end_pos;

        let mut found_break_count = 0;

        // Loop through the text, looking for ranges of dictionary characters.
        // For each span, find the appropriate break engine, and ask it to find
        // any breaks within the span.

        self.text.set_native_index(range_start);
        let mut c = self.text.current32();
        let mut category = self.category(c);
        let dict_start = self.data.forward.dict_categories_start;

        loop {
            let mut current;
            loop {
                current = self.text.get_native_index();
                if !(current < range_end && u32::from(category) < dict_start) {
                    break;
                }
                self.text.next32(); // TODO: cleaner loop structure.
                c = self.text.current32();
                category = self.category(c);
            }
            if current >= range_end {
                break;
            }

            // We now have a dictionary character. Get the appropriate language object
            // to deal with it.
            let lbe = self.language_break_engine(c);

            // Ask the language object if there are any breaks. It will add them to the cache and
            // leave the text pointer on the other side of its range, ready to search for the next one.
            found_break_count += match lbe {
                LanguageBreakEngine::Dictionary(engine) => engine.find_breaks(
                    &mut self.text,
                    current,
                    range_end,
                    &mut self.dictionary_cache.breaks,
                ),
                LanguageBreakEngine::Unhandled => self
                    .unhandled_break_engine
                    .unwrap_or_default()
                    .find_breaks(&mut self.text, current, range_end),
            };

            // Reload the loop variables for the next go-round
            c = self.text.current32();
            category = self.category(c);
        }

        // If we found breaks, ensure that the first and last entries are
        // the original starting and ending position. And initialize the
        // cache iteration position to the first entry.

        let dc = &mut self.dictionary_cache;
        if found_break_count > 0 {
            if start_pos < element_ati(&dc.breaks, 0) {
                // The dictionary did not place a boundary at the start of the segment of text.
                // Add one now. This should not commonly happen, but it would be easy for interactions
                // of the rules for dictionary segments and the break engine implementations to
                // inadvertently cause it. Cover it here, just in case.
                dc.breaks.insert(0, start_pos);
            }
            if end_pos > peeki(&dc.breaks) {
                dc.breaks.push(end_pos);
            }
            dc.position_in_cache = 0;
            // Note: Dictionary matching may extend beyond the original limit.
            dc.start = element_ati(&dc.breaks, 0);
            dc.limit = peeki(&dc.breaks);
        } else {
            // there were no language-based breaks, even though the segment contained
            // dictionary characters. Subsequent attempts to fetch boundaries from the dictionary cache
            // for this range will fail, and the calling code will fall back to the rule based boundaries.
        }
    }

    // --------------------------------------------------------------------------------------------
    // BreakCache

    // Port of: icu4c/source/common/rbbi_cache.cpp#L221-L226 (chromium/deps/icu d578f2e8)
    /// `BreakCache::current()`.
    fn cache_current(&mut self) -> i32 {
        self.position = self.break_cache.text_idx;
        self.rule_status_index = self.break_cache.s(self.break_cache.buf_idx);
        self.done = false;
        self.break_cache.text_idx
    }

    fn cache_seek(&mut self, pos: i32) -> bool {
        self.break_cache.seek(pos)
    }

    // Port of: icu4c/source/common/rbbi_cache.h#L91-L98 (chromium/deps/icu d578f2e8)
    /// `BreakCache::next()`.
    fn cache_next(&mut self) {
        let bc = &mut self.break_cache;
        if bc.buf_idx == bc.end_buf_idx {
            self.cache_next_ol();
        } else {
            bc.buf_idx = mod_chunk_size(bc.buf_idx + 1);
            bc.text_idx = bc.b(bc.buf_idx);
            self.position = bc.text_idx;
            self.rule_status_index = bc.s(bc.buf_idx);
        }
    }

    // Port of: icu4c/source/common/rbbi_cache.cpp#L229-L265 (chromium/deps/icu d578f2e8)
    /// `BreakCache::following(startPos, status)`.
    fn cache_following(&mut self, start_pos: i32) {
        if start_pos == self.break_cache.text_idx
            || self.cache_seek(start_pos)
            || self.populate_near(start_pos)
        {
            // startPos is in the cache. Do a next() from that position.
            // TODO: an awkward set of interactions with bi->fDone
            //       seek() does not clear it; it can't because of interactions with populateNear().
            //       next() does not clear it in the fast-path case, where everything matters. Maybe it should.
            //       So clear it here, for the case where seek() succeeded on an iterator that had previously run off the end.
            self.done = false;
            self.cache_next();
        }
    }

    /// `BreakCache::preceding(startPos, status)`.
    fn cache_preceding(&mut self, start_pos: i32) {
        if start_pos == self.break_cache.text_idx
            || self.cache_seek(start_pos)
            || self.populate_near(start_pos)
        {
            if start_pos == self.break_cache.text_idx {
                self.cache_previous();
            } else {
                // seek() leaves the BreakCache positioned at the preceding boundary
                //        if the requested position is between two boundaries.
                // current() pushes the BreakCache position out to the BreakIterator itself.
                debug_assert!(start_pos > self.break_cache.text_idx);
                self.cache_current();
            }
        }
    }

    // Port of: icu4c/source/common/rbbi_cache.cpp#L268-L301 (chromium/deps/icu d578f2e8)
    /// `BreakCache::nextOL()`: the cache does not already contain the boundary.
    fn cache_next_ol(&mut self) {
        self.done = !self.populate_following();
        self.position = self.break_cache.text_idx;
        self.rule_status_index = self.break_cache.s(self.break_cache.buf_idx);
    }

    /// `BreakCache::previous(status)`.
    fn cache_previous(&mut self) {
        let initial_buf_idx = self.break_cache.buf_idx;
        if self.break_cache.buf_idx == self.break_cache.start_buf_idx {
            // At start of cache. Prepend to it.
            self.populate_preceding();
        } else {
            // Cache already holds the next boundary
            let bc = &mut self.break_cache;
            bc.buf_idx = mod_chunk_size(bc.buf_idx - 1);
            bc.text_idx = bc.b(bc.buf_idx);
        }
        let bc = &self.break_cache;
        self.done = bc.buf_idx == initial_buf_idx;
        self.position = bc.text_idx;
        self.rule_status_index = bc.s(bc.buf_idx);
    }

    // Port of: icu4c/source/common/rbbi_cache.cpp#L335-L459 (chromium/deps/icu d578f2e8)
    /// `BreakCache::populateNear(position, status)`.
    fn populate_near(&mut self, position: i32) -> bool {
        // Threshold for a text position to be considered near to existing cache contents.
        // TODO: See issue ICU-22024 "perf tuning of Cache needed."
        //       This value is subject to change. See the ticket for more details.
        const CACHE_NEAR: i32 = 15;

        let mut a_boundary = -1;
        let mut rule_status_index = 0;
        let retain_cache;
        let start_b = self.break_cache.b(self.break_cache.start_buf_idx);
        let end_b = self.break_cache.b(self.break_cache.end_buf_idx);
        if (position > start_b - CACHE_NEAR) && position < (end_b + CACHE_NEAR) {
            // Requested position is near the existing cache. Retain it.
            retain_cache = true;
        } else if position <= CACHE_NEAR {
            // Requested position is near the start of the text. Fill cache from start, skipping
            // the need to find a safe point.
            retain_cache = false;
            a_boundary = 0;
        } else {
            // Requested position is not near the existing cache.
            // Find a safe point to refill the cache from.
            let backup_pos = self.handle_safe_previous(position);

            if end_b < position && end_b >= (backup_pos - CACHE_NEAR) {
                // The requested position is beyond the end of the existing cache, but the
                // reverse rules produced a position near or before the cached region.
                // Retain the existing cache, and fill from the end of it.
                retain_cache = true;
            } else if backup_pos < CACHE_NEAR {
                // The safe reverse rules moved us to near the start of text.
                // Take that (index 0) as the backup boundary, avoiding the complication
                // (in the following block) of moving forward from the safe point to a known boundary.
                //
                // Retain the cache if it begins not too far from the requested position.
                a_boundary = 0;
                retain_cache = start_b <= (position + CACHE_NEAR);
            } else {
                // The safe reverse rules produced a position that is neither near the existing
                // cache, nor near the start of text.
                // Advance to the boundary following.
                // There is a complication: the safe reverse rules identify pairs of code points
                // that are safe. If advancing from the safe point moves forwards by less than
                // two code points, we need to advance one more time to ensure that the boundary
                // is good, including a correct rules status value.
                retain_cache = false;
                self.position = backup_pos;
                a_boundary = self.handle_next();
                if a_boundary != UBRK_DONE && a_boundary <= backup_pos + 4 {
                    // +4 is a quick test for possibly having advanced only one codepoint.
                    // Four being the length of the longest potential code point, a supplementary in UTF-8
                    self.text.set_native_index(a_boundary);
                    if backup_pos == self.text.get_previous_native_index() {
                        // The initial handleNext() only advanced by a single code point. Go again.
                        a_boundary = self.handle_next(); // Safe rules identify safe pairs.
                    }
                }
                if a_boundary == UBRK_DONE {
                    // Note (Andy Heninger): I don't think this condition can occur, but it's hard
                    // to prove that it can't. We ran off the end of the string looking a boundary
                    // following a safe point; choose the end of the string as that boundary.
                    a_boundary = self.text.native_length();
                }
                rule_status_index = self.rule_status_index;
            }
        }

        if !retain_cache {
            debug_assert_ne!(a_boundary, -1);
            self.break_cache.reset(a_boundary, rule_status_index); // Reset cache to hold aBoundary as a single starting point.
        }

        // Fill in boundaries between existing cache content and the new requested position.

        if self.break_cache.b(self.break_cache.end_buf_idx) < position {
            // The last position in the cache precedes the requested position.
            // Add following position(s) to the cache.
            while self.break_cache.b(self.break_cache.end_buf_idx) < position {
                if !self.populate_following() {
                    unreachable!("UPRV_UNREACHABLE_EXIT: no boundary before {position}");
                }
            }
            let bc = &mut self.break_cache;
            bc.buf_idx = bc.end_buf_idx; // Set iterator position to the end of the buffer.
            bc.text_idx = bc.b(bc.buf_idx); // Required because populateFollowing may add extra boundaries.
            while self.break_cache.text_idx > position {
                // Move backwards to a position at or preceding the requested pos.
                self.cache_previous();
            }
            return true;
        }

        if self.break_cache.b(self.break_cache.start_buf_idx) > position {
            // The first position in the cache is beyond the requested position.
            // back up more until we get a boundary <= the requested position.
            while self.break_cache.b(self.break_cache.start_buf_idx) > position {
                self.populate_preceding();
            }
            let bc = &mut self.break_cache;
            bc.buf_idx = bc.start_buf_idx; // Set iterator position to the start of the buffer.
            bc.text_idx = bc.b(bc.buf_idx); // Required because populatePreceding may add extra boundaries.
            while self.break_cache.text_idx < position {
                // Move forwards to a position at or following the requested pos.
                self.cache_next();
            }
            if self.break_cache.text_idx > position {
                // If position is not itself a boundary, the next() loop above will overshoot.
                // Back up one, leaving cache position at the boundary preceding the requested position.
                self.cache_previous();
            }
            return true;
        }

        debug_assert_eq!(self.break_cache.text_idx, position);
        true
    }

    // Port of: icu4c/source/common/rbbi_cache.cpp#L463-L512 (chromium/deps/icu d578f2e8)
    /// `BreakCache::populateFollowing()`.
    fn populate_following(&mut self) -> bool {
        let from_position = self.break_cache.b(self.break_cache.end_buf_idx);
        let from_rule_status_idx = self.break_cache.s(self.break_cache.end_buf_idx);

        if let Some((pos, rule_status_idx)) = self.dictionary_cache.following(from_position) {
            self.break_cache.add_following(pos, rule_status_idx, true);
            return true;
        }

        self.position = from_position;
        let mut pos = self.handle_next();
        if pos == UBRK_DONE {
            return false;
        }

        let mut rule_status_idx = self.rule_status_index;
        if self.dictionary_char_count > 0 {
            // The text segment obtained from the rules includes dictionary characters.
            // Subdivide it, with subdivided results going into the dictionary cache.
            self.populate_dictionary(from_position, pos, from_rule_status_idx, rule_status_idx);
            if let Some((p, s)) = self.dictionary_cache.following(from_position) {
                self.break_cache.add_following(p, s, true);
                return true;
                // TODO: may want to move a sizable chunk of dictionary cache to break cache at this point.
                //       But be careful with interactions with populateNear().
            }
        }

        // Rule based segment did not include dictionary characters.
        // Or, it did contain dictionary chars, but the dictionary segmenter didn't handle them,
        //    meaning that we didn't take the return, above.
        // Add its end point to the cache.
        self.break_cache.add_following(pos, rule_status_idx, true);

        // Add several non-dictionary boundaries at this point, to optimize straight forward iteration.
        //    (subsequent calls to BreakIterator::next() will take the fast path, getting cached results.
        //
        for _ in 0..6 {
            pos = self.handle_next();
            if pos == UBRK_DONE || self.dictionary_char_count > 0 {
                break;
            }
            rule_status_idx = self.rule_status_index;
            self.break_cache.add_following(pos, rule_status_idx, false);
        }

        true
    }

    // Port of: icu4c/source/common/rbbi_cache.cpp#L515-L642 (chromium/deps/icu d578f2e8)
    /// `BreakCache::populatePreceding(status)`.
    fn populate_preceding(&mut self) -> bool {
        let from_position = self.break_cache.b(self.break_cache.start_buf_idx);
        if from_position == 0 {
            return false;
        }

        let mut position;
        let mut position_status_idx;

        if let Some((p, s)) = self.dictionary_cache.preceding(from_position) {
            self.break_cache.add_preceding(p, s, true);
            return true;
        }

        let mut backup_position = from_position;

        // Find a boundary somewhere preceding the first already-cached boundary
        loop {
            backup_position -= 30;
            if backup_position <= 0 {
                backup_position = 0;
            } else {
                backup_position = self.handle_safe_previous(backup_position);
            }
            if backup_position == UBRK_DONE || backup_position == 0 {
                position = 0;
                position_status_idx = 0;
            } else {
                // Advance to the boundary following the backup position.
                // There is a complication: the safe reverse rules identify pairs of code points
                // that are safe. If advancing from the safe point moves forwards by less than
                // two code points, we need to advance one more time to ensure that the boundary
                // is good, including a correct rules status value.
                //
                self.position = backup_position;
                position = self.handle_next();
                if position <= backup_position + 4 {
                    // +4 is a quick test for possibly having advanced only one codepoint.
                    // Four being the length of the longest potential code point, a supplementary in UTF-8
                    self.text.set_native_index(position);
                    if backup_position == self.text.get_previous_native_index() {
                        // The initial handleNext() only advanced by a single code point. Go again.
                        position = self.handle_next(); // Safe rules identify safe pairs.
                    }
                }
                position_status_idx = self.rule_status_index;
            }
            if position < from_position {
                break;
            }
        }

        // Find boundaries between the one we just located and the first already-cached boundary
        // Put them in a side buffer, because we don't yet know where they will fall in the circular cache buffer..

        let mut side = std::mem::take(&mut self.break_cache.side_buffer);
        side.clear();
        side.push(position);
        side.push(position_status_idx);

        loop {
            let mut prev_position = position;
            self.position = position;
            let prev_status_idx = position_status_idx;
            position = self.handle_next();
            position_status_idx = self.rule_status_index;
            if position == UBRK_DONE {
                break;
            }

            let mut segment_handled_by_dictionary = false;
            if self.dictionary_char_count != 0 {
                // Segment from the rules includes dictionary characters.
                // Subdivide it, with subdivided results going into the dictionary cache.
                let dict_seg_end_position = position;
                self.populate_dictionary(
                    prev_position,
                    dict_seg_end_position,
                    prev_status_idx,
                    position_status_idx,
                );
                while let Some((p, s)) = self.dictionary_cache.following(prev_position) {
                    position = p;
                    position_status_idx = s;
                    segment_handled_by_dictionary = true;
                    debug_assert!(position > prev_position);
                    if position >= from_position {
                        break;
                    }
                    debug_assert!(position <= dict_seg_end_position);
                    side.push(position);
                    side.push(position_status_idx);
                    prev_position = position;
                }
                debug_assert!(position == dict_seg_end_position || position >= from_position);
            }

            if !segment_handled_by_dictionary && position < from_position {
                side.push(position);
                side.push(position_status_idx);
            }
            if position >= from_position {
                break;
            }
        }

        // Move boundaries from the side buffer to the main circular buffer.
        let mut success = false;
        if let (Some(s), Some(p)) = (side.pop(), side.pop()) {
            self.break_cache.add_preceding(p, s, true);
            success = true;
        }

        while let (Some(s), Some(p)) = (side.pop(), side.pop()) {
            if !self.break_cache.add_preceding(p, s, false) {
                // No space in circular buffer to hold a new preceding result while
                // also retaining the current cache (iteration) position.
                // Bailing out is safe; the cache will refill again if needed.
                break;
            }
        }

        self.break_cache.side_buffer = side;
        success
    }
}
