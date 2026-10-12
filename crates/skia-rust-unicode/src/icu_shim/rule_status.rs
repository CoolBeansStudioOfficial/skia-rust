// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The rule status ICU reports for each boundary (`ubrk_getRuleStatus`), computed around ICU4X's
//! boundaries.
//!
//! In ICU the status of a boundary is the largest tag (`{100}`, `{200}`, ...) of the rules that
//! accept where the match stopped (`rbbi.cpp` `handleNext`, `getRuleStatus`). With `!!chain`, a match
//! is a chain of rules where each one starts on the last character of the one before. From
//! Chromium's rule sources (`source/data/brkitr/rules/{word,sent,line_normal}.txt` at d578f2e8):
//!
//! - **Words** (`word.txt`): the rules carry `UBRK_WORD_NUMBER` (100), `UBRK_WORD_LETTER` (200) and
//!   `UBRK_WORD_IDEO` (400) tags that follow the word's final base character; the rule-based pass is
//!   run as ICU runs it ([`super::word_rules`]).
//! - **Sentences** (`sent.txt`): `UBRK_SENTENCE_SEP` (100) when the range ends in a paragraph
//!   separator (`Sep`, `CR`, `LF`, `CR LF`) or at the end of the text and nothing but
//!   `Extend`/`Format`/`Close`/`Sp` follows its last non-terminator, otherwise `UBRK_SENTENCE_TERM`
//!   (0).
//! - **Lines** (`line_normal.txt` and the other `line*.txt`): `UBRK_LINE_HARD` (100) when the range
//!   ends in a mandatory break (`BK`, `CR`, `LF`, `NL`), else `UBRK_LINE_SOFT` (0).
//!
//! Dictionary boundaries (words and lines in complex scripts and CJ) are not produced by the rules:
//! ICU finds the rule-based range first and then splits it with a dictionary
//! (`RuleBasedBreakIterator::DictionaryCache::populateDictionary`, `rbbi_cache.cpp`), and every
//! boundary it adds reports the status of the rule-based boundary that ends the range
//! (`fOtherRuleStatusIndex`). A boundary that the rule-based pass does not have is such a boundary.

#![allow(clippy::doc_markdown)] // `Extend`, `ALetterPlus`, ... are rule-set names, not code.

use icu_properties::CodePointMapData;
use icu_properties::props::{LineBreak, SentenceBreak};

use super::uchar::line_break;
use super::word_rules;

/// `UBRK_SENTENCE_SEP`.
const SENTENCE_SEP: i32 = 100;
/// `UBRK_LINE_HARD`.
const LINE_HARD: i32 = 100;

// Port of: icu4c/source/common/rbbi_cache.cpp DictionaryCache::populateDictionary and following
// (chromium/deps/icu d578f2e8): a dictionary boundary takes the status of the end of its range.
/// The rule status of each boundary of `bounds`, given the rule-based boundaries and their status
/// (`(offset, status)`, ascending).
fn with_dictionary_ranges(bounds: &[usize], rules: &[(usize, i32)]) -> Vec<i32> {
    bounds
        .iter()
        .map(|&b| {
            if b == 0 {
                return 0;
            }
            // The rule-based boundary at `b`, or the first one after it.
            let i = rules.partition_point(|&(r, _)| r < b);
            rules.get(i).map_or(0, |&(_, s)| s)
        })
        .collect()
}

/// The rule status of each word boundary of a text of `len` code units.
pub(super) fn word(cps: &[(usize, u32)], bounds: &[usize], len: usize) -> Vec<i32> {
    with_dictionary_ranges(bounds, &word_rules::ranges(cps, len))
}

// Port of: source/data/brkitr/rules/line_normal.txt#L130-L137 (chromium/deps/icu d578f2e8):
// `$CR $LF {100}`, `$LB4NonBreaks? $LB4Breaks {100}`, `$CAN_CM $CM* $LB4Breaks {100}`,
// `^$CM+ $LB4Breaks {100}`. Every other line rule is untagged.
/// The rule status of each line boundary, given the boundaries of the rule-based pass (`rules`).
pub(super) fn line(cps: &[(usize, u32)], bounds: &[usize], rules: &[usize]) -> Vec<i32> {
    let mut ranges = Vec::with_capacity(rules.len());
    for &r in rules {
        // The last code point before `r`.
        let i = cps.partition_point(|&(p, _)| p < r);
        let hard = i > 0
            && matches!(
                line_break(cps[i - 1].1),
                LineBreak::MandatoryBreak
                    | LineBreak::CarriageReturn
                    | LineBreak::LineFeed
                    | LineBreak::NextLine
            );
        ranges.push((r, if hard { LINE_HARD } else { 0 }));
    }
    with_dictionary_ranges(bounds, &ranges)
}

// Port of: source/data/brkitr/rules/sent.txt#L83 (chromium/deps/icu d578f2e8): the only tagged
// rule, `[[^$STerm $ATerm $Close $Sp $Sep $LF $CR $Format $Extend]{bof}] ($Extend | $Format |
// $Close | $Sp)* ([$Sep $LF $CR {eof}] | $CR $LF){100}`.
/// The rule status of each sentence boundary of a text of `len` code units.
pub(super) fn sentence(cps: &[(usize, u32)], bounds: &[usize], len: usize) -> Vec<i32> {
    let sb = |c: u32| CodePointMapData::<SentenceBreak>::new().get32(c);
    let mut out = Vec::with_capacity(bounds.len());
    let mut prev = 0;
    for (i, &b) in bounds.iter().enumerate() {
        if i == 0 {
            out.push(0);
            prev = b;
            continue;
        }
        let lo = cps.partition_point(|&(p, _)| p < prev);
        let hi = cps.partition_point(|&(p, _)| p < b);
        let mut range: Vec<SentenceBreak> = cps[lo..hi].iter().map(|&(_, c)| sb(c)).collect();
        prev = b;
        // `[$Sep $LF $CR {eof}] | $CR $LF`.
        let n = range.len();
        let ends_in_separator =
            if n >= 2 && range[n - 2] == SentenceBreak::CR && range[n - 1] == SentenceBreak::LF {
                range.truncate(n - 2);
                true
            } else if n >= 1
                && matches!(
                    range[n - 1],
                    SentenceBreak::Sep | SentenceBreak::LF | SentenceBreak::CR
                )
            {
                range.truncate(n - 1);
                true
            } else {
                false
            };
        if !ends_in_separator && b != len {
            out.push(0);
            continue;
        }
        // `($Extend | $Format | $Close | $Sp)*`.
        while range.last().is_some_and(|&s| {
            matches!(
                s,
                SentenceBreak::Extend
                    | SentenceBreak::Format
                    | SentenceBreak::Close
                    | SentenceBreak::Sp
            )
        }) {
            range.pop();
        }
        // `[[^$STerm $ATerm $Close $Sp $Sep $LF $CR $Format $Extend]{bof}]`. `{bof}` is the start of
        // the range: `handleNext` starts every match with the "beginning" category
        // (`RBBI_BOF_REQUIRED`, rbbi.cpp#L823-L826).
        let status = match range.last().copied() {
            Some(
                SentenceBreak::STerm
                | SentenceBreak::ATerm
                | SentenceBreak::Sep
                | SentenceBreak::LF
                | SentenceBreak::CR,
            ) => 0,
            _ => SENTENCE_SEP,
        };
        out.push(status);
    }
    out
}
