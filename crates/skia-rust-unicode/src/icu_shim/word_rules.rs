// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The rule-based pass of ICU's word break iterator over Chromium's `word.txt`: the ranges ICU's
//! rules find before the dictionaries split the ones that hold dictionary characters, and the rule
//! status of each.
//!
//! ICU compiles the rules into a DFA (`rbbirb.cpp`) and runs it from each boundary
//! (`rbbi.cpp` `handleNext`): with `!!chain`, a rule may start on the last character a rule matched,
//! the match ends at the last position where some chain of rules accepted, and the status is the
//! largest tag of the rules that accept there. This file runs the rules of `word.txt` that way
//! directly, which gives the same ranges and tags. ICU4X's own word rules are the stock UAX #29 ones
//! and do not treat complex-script, kana and kanji runs and Hangul syllables as ICU's rules do, so
//! they cannot stand for this pass.

#![allow(clippy::doc_markdown)] // `$ALetterPlus`, ... are rule-set names, not code.

use icu_properties::props::{
    ExtendedPictographic, GraphemeClusterBreak, Ideographic, LineBreak, Script, WordBreak,
};
use icu_properties::{CodePointMapData, CodePointSetData};

use super::uchar::line_break;

// Port of: source/data/brkitr/rules/word.txt#L30-L79 (chromium/deps/icu d578f2e8): the sets, one
// bit each.
const CR: u32 = 1 << 0;
const LF: u32 = 1 << 1;
const NEWLINE: u32 = 1 << 2;
const EXTEND: u32 = 1 << 3;
const ZWJ: u32 = 1 << 4;
const REGIONAL_INDICATOR: u32 = 1 << 5;
const FORMAT: u32 = 1 << 6;
const KATAKANA: u32 = 1 << 7;
const HEBREW_LETTER: u32 = 1 << 8;
const SINGLE_QUOTE: u32 = 1 << 9;
const DOUBLE_QUOTE: u32 = 1 << 10;
const MID_NUM_LET: u32 = 1 << 11;
const MID_LETTER: u32 = 1 << 12;
const MID_NUM: u32 = 1 << 13;
const NUMERIC: u32 = 1 << 14;
const EXTEND_NUM_LET: u32 = 1 << 15;
const WSEG_SPACE: u32 = 1 << 16;
const EXTENDED_PICT: u32 = 1 << 17;
const HIRAGANA: u32 = 1 << 18;
const IDEOGRAPHIC: u32 = 1 << 19;
const HANGUL_SYLLABLE: u32 = 1 << 20;
const KANA_KANJI: u32 = 1 << 21;
const ALETTER_PLUS: u32 = 1 << 22;
/// `$ExFm = [$Extend $Format $ZWJ]`.
const EX_FM: u32 = EXTEND | FORMAT | ZWJ;
/// `($ALetterPlus | $Hebrew_Letter)`.
const AHLETTER: u32 = ALETTER_PLUS | HEBREW_LETTER;

/// The sets of `word.txt` that `c` is in.
fn sets(c: u32) -> u32 {
    let wb = CodePointMapData::<WordBreak>::new().get32(c);
    let script = CodePointMapData::<Script>::new().get32(c);
    // `$Han = [:Han:]`, `$Hiragana = [:Hiragana:]`.
    let han = script == Script::Han;
    let hiragana = script == Script::Hiragana;
    let katakana = wb == WordBreak::Katakana;
    // `$Extend = [\p{Word_Break = Extend}-$Han]`.
    let extend = wb == WordBreak::Extend && !han;
    // `$Control = [\p{Grapheme_Cluster_Break = Control}]`.
    let control =
        CodePointMapData::<GraphemeClusterBreak>::new().get32(c) == GraphemeClusterBreak::Control;
    // `$HangulSyllable = [가-힣]`.
    let hangul = (0xAC00..=0xD7A3).contains(&c);
    // `$KanaKanji = [$Han $Hiragana $Katakana]`, `$dictionaryCJK = [$KanaKanji $HangulSyllable]`.
    let kana_kanji = han || hiragana || katakana;
    let dictionary_cjk = kana_kanji || hangul;
    // `$ComplexContext = [:LineBreak = Complex_Context:]`.
    let complex_context = line_break(c) == LineBreak::ComplexContext;
    let mut s = 0;
    let mut set = |bit: u32, on: bool| {
        if on {
            s |= bit;
        }
    };
    set(CR, wb == WordBreak::CR);
    set(LF, wb == WordBreak::LF);
    set(NEWLINE, wb == WordBreak::Newline);
    set(EXTEND, extend);
    set(ZWJ, wb == WordBreak::ZWJ);
    set(REGIONAL_INDICATOR, wb == WordBreak::RegionalIndicator);
    set(FORMAT, wb == WordBreak::Format);
    set(KATAKANA, katakana);
    set(HEBREW_LETTER, wb == WordBreak::HebrewLetter);
    set(SINGLE_QUOTE, wb == WordBreak::SingleQuote);
    set(DOUBLE_QUOTE, wb == WordBreak::DoubleQuote);
    // Chromium's wordbrk.patch: `.` and U+FF0E move from MidNumLet to MidNum, and `:`, U+FE55 and
    // U+FF1A leave MidLetter.
    let dot = c == 0x2E || c == 0xFF0E;
    set(MID_NUM_LET, wb == WordBreak::MidNumLet && !dot);
    set(
        MID_LETTER,
        wb == WordBreak::MidLetter && !matches!(c, 0x3A | 0xFE55 | 0xFF1A),
    );
    set(MID_NUM, wb == WordBreak::MidNum || dot);
    set(NUMERIC, wb == WordBreak::Numeric);
    set(EXTEND_NUM_LET, wb == WordBreak::ExtendNumLet);
    set(WSEG_SPACE, wb == WordBreak::WSegSpace);
    set(
        EXTENDED_PICT,
        CodePointSetData::new::<ExtendedPictographic>().contains32(c),
    );
    set(HIRAGANA, hiragana);
    set(
        IDEOGRAPHIC,
        CodePointSetData::new::<Ideographic>().contains32(c),
    );
    set(HANGUL_SYLLABLE, hangul);
    set(KANA_KANJI, kana_kanji);
    // `$ALetterPlus = [$ALetter-$dictionaryCJK [$ComplexContext-$Extend-$Control]]`.
    set(
        ALETTER_PLUS,
        (wb == WordBreak::ALetter && !dictionary_cjk) || (complex_context && !extend && !control),
    );
    s
}

/// One element of a rule: a set (or its complement), once or any number of times.
#[derive(Clone, Copy)]
struct Item {
    set: u32,
    negated: bool,
    star: bool,
}

const fn one(set: u32) -> Item {
    Item {
        set,
        negated: false,
        star: false,
    }
}

const fn star(set: u32) -> Item {
    Item {
        set,
        negated: false,
        star: true,
    }
}

/// `[^set]`.
const fn not(set: u32) -> Item {
    Item {
        set,
        negated: true,
        star: false,
    }
}

/// `.`: any character.
const ANY: Item = not(0);

/// A rule: `^` (only at the start of a match), its items and its tag.
struct Rule {
    anchored: bool,
    items: &'static [Item],
    tag: i32,
}

const fn rule(items: &'static [Item], tag: i32) -> Rule {
    Rule {
        anchored: false,
        items,
        tag,
    }
}

// Port of: source/data/brkitr/rules/word.txt#L86-L180 (chromium/deps/icu d578f2e8), in order.
const RULES: &[Rule] = &[
    // 3: `$CR $LF;`
    rule(&[one(CR), one(LF)], 0),
    // 3c: `$ZWJ $Extended_Pict;`
    rule(&[one(ZWJ), one(EXTENDED_PICT)], 0),
    // 3d: `$WSegSpace $WSegSpace;`
    rule(&[one(WSEG_SPACE), one(WSEG_SPACE)], 0),
    // 4: `^$ExFm+;`
    Rule {
        anchored: true,
        items: &[one(EX_FM), star(EX_FM)],
        tag: 0,
    },
    // `[^$CR $LF $Newline $ExFm] $ExFm*;`
    rule(&[not(CR | LF | NEWLINE | EX_FM), star(EX_FM)], 0),
    rule(&[one(NUMERIC), star(EX_FM)], 100),
    rule(&[one(ALETTER_PLUS), star(EX_FM)], 200),
    rule(&[one(HANGUL_SYLLABLE)], 200),
    rule(&[one(HEBREW_LETTER), star(EX_FM)], 200),
    rule(&[one(KATAKANA), star(EX_FM)], 400),
    rule(&[one(HIRAGANA), star(EX_FM)], 400),
    rule(&[one(IDEOGRAPHIC), star(EX_FM)], 400),
    // 5: `($ALetterPlus | $Hebrew_Letter) $ExFm* ($ALetterPlus | $Hebrew_Letter);`
    rule(&[one(AHLETTER), star(EX_FM), one(AHLETTER)], 0),
    // 6, 7.
    rule(
        &[
            one(AHLETTER),
            star(EX_FM),
            one(MID_LETTER | MID_NUM_LET | SINGLE_QUOTE),
            star(EX_FM),
            one(AHLETTER),
        ],
        200,
    ),
    // 7a, 7b, 7c.
    rule(&[one(HEBREW_LETTER), star(EX_FM), one(SINGLE_QUOTE)], 200),
    rule(
        &[
            one(HEBREW_LETTER),
            star(EX_FM),
            one(DOUBLE_QUOTE),
            star(EX_FM),
            one(HEBREW_LETTER),
        ],
        0,
    ),
    // 8, 9, 10.
    rule(&[one(NUMERIC), star(EX_FM), one(NUMERIC)], 0),
    rule(&[one(AHLETTER), star(EX_FM), one(NUMERIC)], 0),
    rule(&[one(NUMERIC), star(EX_FM), one(AHLETTER)], 0),
    // 11, 12.
    rule(
        &[
            one(NUMERIC),
            star(EX_FM),
            one(MID_NUM | MID_NUM_LET | SINGLE_QUOTE),
            star(EX_FM),
            one(NUMERIC),
        ],
        0,
    ),
    // 13.
    rule(&[one(KATAKANA), star(EX_FM), one(KATAKANA)], 400),
    // 13a.
    rule(&[one(ALETTER_PLUS), star(EX_FM), one(EXTEND_NUM_LET)], 200),
    rule(&[one(HEBREW_LETTER), star(EX_FM), one(EXTEND_NUM_LET)], 200),
    rule(&[one(NUMERIC), star(EX_FM), one(EXTEND_NUM_LET)], 100),
    rule(&[one(KATAKANA), star(EX_FM), one(EXTEND_NUM_LET)], 400),
    rule(
        &[one(EXTEND_NUM_LET), star(EX_FM), one(EXTEND_NUM_LET)],
        200,
    ),
    // 13b.
    rule(&[one(EXTEND_NUM_LET), star(EX_FM), one(ALETTER_PLUS)], 200),
    rule(&[one(EXTEND_NUM_LET), star(EX_FM), one(HEBREW_LETTER)], 200),
    rule(&[one(EXTEND_NUM_LET), star(EX_FM), one(NUMERIC)], 100),
    rule(&[one(EXTEND_NUM_LET), star(EX_FM), one(KATAKANA)], 400),
    // 15, 16: `^$Regional_Indicator $ExFm* $Regional_Indicator;`
    Rule {
        anchored: true,
        items: &[
            one(REGIONAL_INDICATOR),
            star(EX_FM),
            one(REGIONAL_INDICATOR),
        ],
        tag: 0,
    },
    // `$HangulSyllable $HangulSyllable {200};`
    rule(&[one(HANGUL_SYLLABLE), one(HANGUL_SYLLABLE)], 200),
    // `$KanaKanji $KanaKanji {400};`
    rule(&[one(KANA_KANJI), one(KANA_KANJI)], 400),
    // 199: `.;`
    rule(&[ANY], 0),
];

/// The ends (exclusive) of every match of `items` that starts at `start`.
fn match_items(items: &[Item], sets: &[u32], start: usize, ends: &mut Vec<usize>) {
    let test =
        |item: &Item, p: usize| p < sets.len() && ((sets[p] & item.set != 0) != item.negated);
    let mut cur = vec![start];
    for item in items {
        let mut next = Vec::new();
        for &p in &cur {
            if item.star {
                let mut q = p;
                next.push(q);
                while test(item, q) {
                    q += 1;
                    next.push(q);
                }
            } else if test(item, p) {
                next.push(p + 1);
            }
        }
        next.sort_unstable();
        next.dedup();
        cur = next;
    }
    ends.extend(cur.into_iter().filter(|&e| e > start));
}

/// The end of the match from `start` and its rule status: the last position where a chain of
/// rules accepts, and the largest tag of the rules that accept there.
fn next_boundary(sets: &[u32], start: usize) -> (usize, i32) {
    let mut best = (start + 1, 0);
    let mut seen = vec![false; sets.len()];
    seen[start] = true;
    let mut chain = vec![start];
    let mut ends = Vec::new();
    while let Some(p) = chain.pop() {
        for rule in RULES {
            if rule.anchored && p != start {
                continue;
            }
            ends.clear();
            match_items(rule.items, sets, p, &mut ends);
            for &e in &ends {
                if e > best.0 {
                    best = (e, rule.tag);
                } else if e == best.0 {
                    best.1 = best.1.max(rule.tag);
                }
                // `!!chain`: the next rule may start on the last character of this one.
                if !seen[e - 1] {
                    seen[e - 1] = true;
                    chain.push(e - 1);
                }
            }
        }
    }
    best
}

/// The boundaries of ICU's rule-based pass over `cps` (offsets and code points) and their rule
/// status: `(offset, status)`, starting with `(0, 0)` and ending at `len`.
pub(super) fn ranges(cps: &[(usize, u32)], len: usize) -> Vec<(usize, i32)> {
    let sets: Vec<u32> = cps.iter().map(|&(_, c)| sets(c)).collect();
    let mut out = vec![(0, 0)];
    let mut i = 0;
    while i < sets.len() {
        let (end, status) = next_boundary(&sets, i);
        i = end;
        out.push((cps.get(end).map_or(len, |&(p, _)| p), status));
    }
    out
}
