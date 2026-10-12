// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `UBreakIterator`: `ubrk_open`, `ubrk_setUText` (over `utext_openUTF8` / `utext_openUChars`),
//! `ubrk_first`, `ubrk_next`, `ubrk_current`, `ubrk_following`, `ubrk_preceding`,
//! `ubrk_getRuleStatus` and `ubrk_clone`, with ICU's semantics over ICU4X's segmenters.
//!
//! The boundaries are ICU4X's; everything around them is ICU's:
//!
//! - which rule file a type and locale select (`BreakIterator::makeInstance` and `buildInstance`,
//!   `brkiter.cpp`, over the `brkitr` resources of Chromium's `icudtl.dat`), see [`resolve_rules`];
//! - the rule status of every boundary, which ICU4X does not report, from the rule tags of
//!   Chromium's `word.txt`, `sent.txt` and `line_normal.txt` (see [`super::rule_status`]);
//! - the navigation of `RuleBasedBreakIterator` and its `BreakCache` (`rbbi.cpp`, `rbbi_cache.cpp`):
//!   `following` and `preceding` first move an offset inside a code point to its start, `next` past
//!   the end reports `UBRK_DONE` and leaves `current` at the end.
//!
//! ICU computes boundaries lazily, through a cache; the shim computes all of them when the text is
//! set. The results are the same.

use std::sync::Arc;

use icu_segmenter::{GraphemeClusterSegmenter, LineSegmenter, SentenceSegmenter, WordSegmenter};

use super::data::{BreakDataSource, CompiledData, LineStrictness};
use super::rule_status;
use super::uloc::{LocaleParts, uloc_get_default};
use super::utypes::{U_MISSING_RESOURCE_ERROR, UErrorCode};

/// `UBRK_DONE`.
pub const UBRK_DONE: i32 = -1;

/// `UBreakIteratorType` (`ubrk.h`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UBreakIteratorType {
    /// `UBRK_CHARACTER`.
    Character,
    /// `UBRK_WORD`.
    Word,
    /// `UBRK_LINE`.
    Line,
    /// `UBRK_SENTENCE`.
    Sentence,
    /// `UBRK_TITLE` (deprecated in ICU; its rule file is not in Chromium's data, so it fails to open).
    Title,
}

/// The rule file a break iterator runs, as the shim represents it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BreakRules {
    /// `char.brk`.
    Grapheme,
    /// `word.brk`.
    Word,
    /// `sent.brk`, or `sent_el.brk` when `greek`.
    Sentence {
        /// `sent_el.brk`.
        greek: bool,
    },
    /// One of the `line*.brk` files.
    Line {
        /// `line`, `line_normal`, `line_loose`.
        strictness: LineStrictness,
        /// The `_cj` variants.
        cj: bool,
    },
}

// The `boundaries` tables of Chromium's `brkitr` resources (`source/data/brkitr/{root,ja,ko,zh,
// zh_Hant,el}.txt` at d578f2e8): (bundle, key, rule file). The other bundles of the tree (de, en,
// en_US, es, fr, it, pt, ru) hold only sentence-break exceptions, which apply with `@ss=standard`.
const BOUNDARIES: &[(&str, &str, &str)] = &[
    ("root", "grapheme", "char"),
    ("root", "line", "line_normal"),
    ("root", "line_loose", "line_loose"),
    ("root", "line_normal", "line_normal"),
    ("root", "line_strict", "line"),
    ("root", "sentence", "sent"),
    ("root", "title", "title"),
    ("root", "word", "word"),
    ("ja", "line", "line_normal"),
    ("ja", "line_loose", "line_loose_cj"),
    ("ja", "line_loose_phrase", "line_loose_phrase_cj"),
    ("ja", "line_normal", "line_normal_cj"),
    ("ja", "line_normal_phrase", "line_normal_phrase_cj"),
    ("ja", "line_phrase", "line_phrase_cj"),
    ("ja", "line_strict", "line"),
    ("ja", "line_strict_phrase", "line_phrase_cj"),
    ("ko", "line", "line_normal"),
    ("ko", "line_loose", "line_loose_cj"),
    ("ko", "line_loose_phrase", "line_loose_phrase_cj"),
    ("ko", "line_normal", "line_normal_cj"),
    ("ko", "line_normal_phrase", "line_normal_phrase_cj"),
    ("ko", "line_phrase", "line_phrase_cj"),
    ("ko", "line_strict", "line_cj"),
    ("ko", "line_strict_phrase", "line_phrase_cj"),
    ("zh", "line", "line_normal_cj"),
    ("zh", "line_loose", "line_loose_cj"),
    ("zh", "line_normal", "line_normal_cj"),
    ("zh", "line_strict", "line"),
    ("zh_Hant", "line", "line_normal_cj"),
    ("zh_Hant", "line_loose", "line_loose_cj"),
    ("zh_Hant", "line_normal", "line_normal_cj"),
    ("zh_Hant", "line_strict", "line"),
    ("el", "sentence", "sent_el"),
];

/// The rule files of Chromium's `icudtl.dat` (`brkitr/*.brk`) and what the shim runs for each.
/// `line_cj.brk` and `title.brk` are named by the resources but not in the data, so opening them
/// fails, as in ICU.
///
/// The `_phrase_cj` files are ICU's phrase breaking for Japanese (`@lw=phrase`, ML-based in ICU), which
/// ICU4X has no counterpart for: the shim runs the `_cj` rules of the same strictness for them.
fn rule_file(name: &str) -> Option<BreakRules> {
    use LineStrictness::{Loose, Normal, Strict};
    let line = |strictness, cj| Some(BreakRules::Line { strictness, cj });
    match name {
        "char" => Some(BreakRules::Grapheme),
        "word" => Some(BreakRules::Word),
        "sent" => Some(BreakRules::Sentence { greek: false }),
        "sent_el" => Some(BreakRules::Sentence { greek: true }),
        "line" => line(Strict, false),
        "line_normal" => line(Normal, false),
        "line_loose" => line(Loose, false),
        "line_normal_cj" | "line_normal_phrase_cj" => line(Normal, true),
        "line_loose_cj" | "line_loose_phrase_cj" => line(Loose, true),
        "line_phrase_cj" => line(Strict, true),
        _ => None,
    }
}

// Port of: icu4c/source/common/brkiter.cpp BreakIterator::makeInstance and buildInstance
// (chromium/deps/icu d578f2e8)
/// The rule file ICU loads for `kind` and the ICU locale ID `locale`: the key `kind` selects
/// (`line` with the `lb` and, for Japanese and Korean, `lw` keywords), looked up in the `boundaries`
/// table of the locale's bundle and its parents (`ures_getByKeyWithFallback`).
///
/// # Errors
/// `U_MISSING_RESOURCE_ERROR` when the key or its rule file does not exist (`ko@lb=strict`,
/// `UBRK_TITLE`).
pub fn resolve_rules(kind: UBreakIteratorType, locale: &str) -> Result<BreakRules, UErrorCode> {
    let parts = LocaleParts::parse(locale);
    let key = match kind {
        UBreakIteratorType::Character => String::from("grapheme"),
        UBreakIteratorType::Word => String::from("word"),
        UBreakIteratorType::Sentence => String::from("sentence"),
        UBreakIteratorType::Title => String::from("title"),
        UBreakIteratorType::Line => {
            let mut key = String::from("line");
            if let Some(lb @ ("strict" | "normal" | "loose")) = parts.keyword("lb") {
                key.push('_');
                key.push_str(lb);
            }
            if (parts.language == "ja" || parts.language == "ko")
                && parts.keyword("lw") == Some("phrase")
            {
                key.push_str("_phrase");
            }
            key
        }
    };
    // The bundle chain: the locale ID without keywords, truncated at each `_`, then root.
    let base = locale.split('@').next().unwrap_or("").replace('-', "_");
    let mut chain: Vec<String> = Vec::new();
    let mut name = base.as_str();
    while !name.is_empty() {
        chain.push(name.to_owned());
        name = name.rfind('_').map_or("", |i| &name[..i]);
    }
    chain.push(String::from("root"));
    for bundle in &chain {
        if let Some(&(_, _, file)) = BOUNDARIES
            .iter()
            .find(|(b, k, _)| b == bundle && *k == key)
        {
            return rule_file(file).ok_or(U_MISSING_RESOURCE_ERROR);
        }
    }
    Err(U_MISSING_RESOURCE_ERROR)
}

/// The segmenters of an open break iterator.
#[derive(Debug)]
enum Engines {
    Grapheme(GraphemeClusterSegmenter),
    Word(WordSegmenter),
    Sentence(SentenceSegmenter),
    Line {
        complex: LineSegmenter,
        rules: LineSegmenter,
    },
}

impl Engines {
    fn new(rules: BreakRules, data: &dyn BreakDataSource) -> Self {
        match rules {
            BreakRules::Grapheme => Engines::Grapheme(data.grapheme()),
            BreakRules::Word => Engines::Word(data.word()),
            BreakRules::Sentence { greek } => Engines::Sentence(data.sentence(greek)),
            BreakRules::Line { strictness, cj } => Engines::Line {
                complex: data.line(strictness, cj, true),
                rules: data.line(strictness, cj, false),
            },
        }
    }
}

/// The text of a break iterator: `utext_openUTF8` (offsets are bytes) or `utext_openUChars`
/// (offsets are UTF-16 code units). ICU's `UText` refers to the caller's buffer; the shim copies it.
#[derive(Clone, Debug)]
enum Text {
    Utf8(Vec<u8>),
    Utf16(Vec<u16>),
}

impl Text {
    fn len(&self) -> usize {
        match self {
            Text::Utf8(t) => t.len(),
            Text::Utf16(t) => t.len(),
        }
    }

    fn as_ref(&self) -> TextRef<'_> {
        match self {
            Text::Utf8(t) => TextRef::Utf8(t),
            Text::Utf16(t) => TextRef::Utf16(t),
        }
    }
}

/// The code points of the text with their offsets: UTF-8 decoded as ICU's `U8_NEXT` does (each
/// maximal ill-formed subpart is one U+FFFD), UTF-16 with unpaired surrogates as themselves.
pub(crate) fn code_points(text: &TextRef<'_>) -> Vec<(usize, u32)> {
    let mut out = Vec::new();
    match *text {
        TextRef::Utf8(bytes) => {
            let mut at = 0;
            for chunk in bytes.utf8_chunks() {
                for (i, c) in chunk.valid().char_indices() {
                    out.push((at + i, u32::from(c)));
                }
                at += chunk.valid().len();
                if !chunk.invalid().is_empty() {
                    out.push((at, 0xFFFD));
                    at += chunk.invalid().len();
                }
            }
        }
        TextRef::Utf16(units) => {
            let mut at = 0;
            for c in char::decode_utf16(units.iter().copied()) {
                let (cp, n) = match c {
                    Ok(c) => (u32::from(c), c.len_utf16()),
                    Err(e) => (u32::from(e.unpaired_surrogate()), 1),
                };
                out.push((at, cp));
                at += n;
            }
        }
    }
    out
}

/// A borrowed text, for the boundary computation.
#[derive(Clone, Copy, Debug)]
pub(crate) enum TextRef<'a> {
    Utf8(&'a [u8]),
    Utf16(&'a [u16]),
}

// Port of: source/data/brkitr/rules/line_normal.txt#L68-L102 (chromium/deps/icu d578f2e8), LB 1:
// `$ALPlus = [$AL $AI $SG $XX [$SA-[[:Mn:][:Mc:]]]]` and `$CM = [... [$SA & [[:Mn:][:Mc:]]]]`.
/// The text ICU's line rules see, as a text ICU4X's rules-only segmenter can run: every `SA`
/// character replaced by a character of the class ICU's rules give it (`AL`, or `CM` for a mark),
/// with the same encoded length so that the offsets do not move. `None` when the text has no `SA`
/// character.
///
/// ICU4X's rules-only segmenter ends a range at every edge of a complex-script run (before a space,
/// `(`, `.` or a line feed after Lao text); ICU's rules join `SA` characters to their neighbours as
/// letters, and only the dictionary splits the range.
fn line_rules_text(text: TextRef<'_>) -> Option<Text> {
    use icu_properties::CodePointMapData;
    use icu_properties::props::{GeneralCategory, LineBreak};
    let substitute = |c: u32| -> Option<char> {
        if super::uchar::line_break(c) != LineBreak::ComplexContext {
            return None;
        }
        let mark = matches!(
            CodePointMapData::<GeneralCategory>::new().get32(c),
            GeneralCategory::NonspacingMark | GeneralCategory::SpacingMark
        );
        Some(match (mark, c > 0xFFFF) {
            // U+0800 SAMARITAN LETTER ALAF, U+10000 LINEAR B SYLLABLE B008 A: `AL`.
            (false, false) => '\u{0800}',
            (false, true) => '\u{10000}',
            // U+0900 DEVANAGARI SIGN INVERTED CANDRABINDU, U+11000 BRAHMI SIGN CANDRABINDU: `CM`.
            (true, false) => '\u{0900}',
            (true, true) => '\u{11000}',
        })
    };
    let cps = code_points(&text);
    if !cps.iter().any(|&(_, c)| substitute(c).is_some()) {
        return None;
    }
    Some(match text {
        TextRef::Utf8(t) => {
            let mut out = t.to_vec();
            for &(p, c) in &cps {
                if let Some(s) = substitute(c) {
                    s.encode_utf8(&mut out[p..p + s.len_utf8()]);
                }
            }
            Text::Utf8(out)
        }
        TextRef::Utf16(t) => {
            let mut out = t.to_vec();
            for &(p, c) in &cps {
                if let Some(s) = substitute(c) {
                    s.encode_utf16(&mut out[p..p + s.len_utf16()]);
                }
            }
            Text::Utf16(out)
        }
    })
}

/// The boundaries of `text` and their rule status.
fn compute(engines: &Engines, text: TextRef<'_>) -> (Vec<usize>, Vec<i32>) {
    macro_rules! run {
        ($seg:expr, $text:expr) => {
            match $text {
                TextRef::Utf8(t) => $seg.as_borrowed().segment_utf8(t).collect::<Vec<usize>>(),
                TextRef::Utf16(t) => $seg.as_borrowed().segment_utf16(t).collect::<Vec<usize>>(),
            }
        };
    }
    match engines {
        Engines::Grapheme(seg) => {
            let b = run!(seg, text);
            let s = vec![0; b.len()];
            (b, s)
        }
        Engines::Sentence(seg) => {
            let b = run!(seg, text);
            let s = rule_status::sentence(&code_points(&text), &b, text_len(text));
            (b, s)
        }
        Engines::Word(seg) => {
            let b = run!(seg, text);
            let s = rule_status::word(&code_points(&text), &b, text_len(text));
            (b, s)
        }
        Engines::Line { complex, rules } => {
            let b = run!(complex, text);
            let resolved = line_rules_text(text);
            let r = run!(rules, resolved.as_ref().map_or(text, Text::as_ref));
            let s = rule_status::line(&code_points(&text), &b, &r);
            (b, s)
        }
    }
}

fn text_len(text: TextRef<'_>) -> usize {
    match text {
        TextRef::Utf8(t) => t.len(),
        TextRef::Utf16(t) => t.len(),
    }
}

/// A position as ICU's `int32_t`.
fn to_i32(p: usize) -> i32 {
    i32::try_from(p).expect("ICU text offsets are int32")
}

/// `UBreakIterator`. See the module documentation.
#[derive(Clone, Debug)]
pub struct UBreakIterator {
    rules: BreakRules,
    engines: Arc<Engines>,
    text: Text,
    /// The boundaries, ascending, starting with 0 and ending with the text length.
    boundaries: Vec<usize>,
    /// The rule status of each boundary.
    statuses: Vec<i32>,
    /// `BreakCache::fBufIdx`: the index of the current boundary.
    index: usize,
    /// `fDone`.
    done: bool,
}

impl UBreakIterator {
    // Port of: icu4c/source/common/ubrk.cpp ubrk_open (chromium/deps/icu d578f2e8)
    /// `ubrk_open(type, locale, nullptr, 0, &status)` on ICU4X's compiled data. `None` is the
    /// default locale, as a null `locale` in ICU.
    ///
    /// # Errors
    /// `U_MISSING_RESOURCE_ERROR` when ICU has no rules for the type and locale ([`resolve_rules`]).
    #[doc(alias = "ubrk_open")]
    pub fn open(kind: UBreakIteratorType, locale: Option<&str>) -> Result<Self, UErrorCode> {
        Self::open_with_data(kind, locale, &CompiledData)
    }

    /// [`Self::open`] with the segmenters of `data` (the seam of [`super::data`]).
    ///
    /// # Errors
    /// As [`Self::open`].
    pub fn open_with_data(
        kind: UBreakIteratorType,
        locale: Option<&str>,
        data: &dyn BreakDataSource,
    ) -> Result<Self, UErrorCode> {
        let rules = resolve_rules(kind, locale.unwrap_or_else(|| uloc_get_default()))?;
        let engines = Arc::new(Engines::new(rules, data));
        // An iterator without text runs over the empty text, as after `ubrk_open` with no text.
        let mut it = UBreakIterator {
            rules,
            engines,
            text: Text::Utf16(Vec::new()),
            boundaries: Vec::new(),
            statuses: Vec::new(),
            index: 0,
            done: false,
        };
        it.set_text(TextRef::Utf16(&[]));
        Ok(it)
    }

    /// The rule file this iterator runs.
    #[must_use]
    pub fn rules(&self) -> BreakRules {
        self.rules
    }

    fn set_text(&mut self, text: TextRef<'_>) {
        let (b, s) = compute(&self.engines, text);
        self.boundaries = b;
        self.statuses = s;
        if self.boundaries.is_empty() {
            // ICU4X reports nothing for the empty text in some modes; ICU has the boundary 0.
            self.boundaries.push(0);
            self.statuses.push(0);
        }
        self.text = match text {
            TextRef::Utf8(t) => Text::Utf8(t.to_vec()),
            TextRef::Utf16(t) => Text::Utf16(t.to_vec()),
        };
        self.index = 0;
        self.done = false;
    }

    // Port of: icu4c/source/common/ubrk.cpp ubrk_setUText and rbbi.cpp setText
    // (chromium/deps/icu d578f2e8)
    /// `ubrk_setUText` with `utext_openUTF8(text)`: the iterator moves to the start.
    #[doc(alias = "ubrk_setUText")]
    #[doc(alias = "utext_openUTF8")]
    pub fn set_text_utf8(&mut self, text: &[u8]) {
        self.set_text(TextRef::Utf8(text));
    }

    /// `ubrk_setUText` with `utext_openUChars(text)`, or `ubrk_setText(text)`.
    #[doc(alias = "ubrk_setUText")]
    #[doc(alias = "ubrk_setText")]
    #[doc(alias = "utext_openUChars")]
    pub fn set_text_utf16(&mut self, text: &[u16]) {
        self.set_text(TextRef::Utf16(text));
    }

    // Port of: icu4c/source/common/rbbi.cpp#L541-L549 (chromium/deps/icu d578f2e8)
    /// `ubrk_first`: 0.
    #[doc(alias = "ubrk_first")]
    pub fn first(&mut self) -> i32 {
        self.index = 0;
        self.done = false;
        to_i32(self.boundaries[0])
    }

    // Port of: icu4c/source/common/rbbi.cpp#L555-L562 (chromium/deps/icu d578f2e8)
    /// `ubrk_last`: the text length.
    #[doc(alias = "ubrk_last")]
    pub fn last(&mut self) -> i32 {
        self.index = self.boundaries.len() - 1;
        self.done = false;
        to_i32(self.boundaries[self.index])
    }

    // Port of: icu4c/source/common/rbbi_cache.h#L91-L98 and rbbi_cache.cpp nextOL
    // (chromium/deps/icu d578f2e8)
    /// `ubrk_next`: the next boundary, or `UBRK_DONE` at the end (`current` stays at the end).
    #[doc(alias = "ubrk_next")]
    pub fn next(&mut self) -> i32 {
        if self.index + 1 < self.boundaries.len() {
            // The fast path of `BreakCache::next` leaves `fDone` as it is.
            self.index += 1;
        } else {
            self.done = true;
        }
        if self.done {
            UBRK_DONE
        } else {
            to_i32(self.boundaries[self.index])
        }
    }

    // Port of: icu4c/source/common/rbbi_cache.cpp BreakCache::previous (chromium/deps/icu d578f2e8)
    /// `ubrk_previous`: the previous boundary, or `UBRK_DONE` at the start.
    #[doc(alias = "ubrk_previous")]
    pub fn previous(&mut self) -> i32 {
        if self.index == 0 {
            self.done = true;
            UBRK_DONE
        } else {
            self.index -= 1;
            self.done = false;
            to_i32(self.boundaries[self.index])
        }
    }

    // Port of: icu4c/source/common/rbbi.cpp#L703-L705 (chromium/deps/icu d578f2e8)
    /// `ubrk_current`.
    #[doc(alias = "ubrk_current")]
    #[must_use]
    pub fn current(&self) -> i32 {
        to_i32(self.boundaries[self.index])
    }

    // Port of: icu4c/source/common/rbbi.cpp getRuleStatus (chromium/deps/icu d578f2e8)
    /// `ubrk_getRuleStatus`: the status of the rule that produced the current boundary (the largest
    /// of its tags; 0 at the start).
    #[doc(alias = "ubrk_getRuleStatus")]
    #[must_use]
    pub fn get_rule_status(&self) -> i32 {
        self.statuses[self.index]
    }

    /// `utext_setNativeIndex(offset)` + `utext_getNativeIndex`: the offset moved into the text and
    /// to the start of the code point it is in.
    fn code_point_start(&self, offset: i32) -> usize {
        let offset = usize::try_from(offset).unwrap_or(0).min(self.text.len());
        match &self.text {
            Text::Utf8(t) => {
                if offset == t.len() || (t[offset] & 0xC0) != 0x80 {
                    return offset;
                }
                // The start of the (possibly ill-formed) sequence that holds `offset`: a lead byte
                // at most 3 bytes before it whose sequence reaches past it.
                let mut s = offset;
                while s > 0 && offset - s < 3 && (t[s] & 0xC0) == 0x80 {
                    s -= 1;
                }
                let len = t[s..].utf8_chunks().next().map_or(1, |chunk| {
                    chunk.valid().chars().next().map_or_else(
                        || chunk.invalid().len().max(1),
                        char::len_utf8,
                    )
                });
                if s + len > offset { s } else { offset }
            }
            Text::Utf16(t) => {
                if offset > 0
                    && offset < t.len()
                    && (0xDC00..0xE000).contains(&t[offset])
                    && (0xD800..0xDC00).contains(&t[offset - 1])
                {
                    offset - 1
                } else {
                    offset
                }
            }
        }
    }

    /// `BreakCache::seek(pos)`: the index of the last boundary at or before `pos`.
    fn seek(&self, pos: usize) -> usize {
        self.boundaries.partition_point(|&b| b <= pos) - 1
    }

    // Port of: icu4c/source/common/rbbi.cpp#L618-L635 and rbbi_cache.cpp BreakCache::following
    // (chromium/deps/icu d578f2e8)
    /// `ubrk_following(offset)`: the first boundary after `offset`, or `UBRK_DONE`.
    #[doc(alias = "ubrk_following")]
    pub fn following(&mut self, offset: i32) -> i32 {
        if offset < 0 {
            return self.first();
        }
        let start = self.code_point_start(offset);
        self.index = self.seek(start);
        self.done = false;
        self.next()
    }

    // Port of: icu4c/source/common/rbbi.cpp#L641-L656 and rbbi_cache.cpp BreakCache::preceding
    // (chromium/deps/icu d578f2e8)
    /// `ubrk_preceding(offset)`: the last boundary before `offset`, or `UBRK_DONE`.
    #[doc(alias = "ubrk_preceding")]
    pub fn preceding(&mut self, offset: i32) -> i32 {
        if usize::try_from(offset).is_ok_and(|o| o > self.text.len()) {
            return self.last();
        }
        let start = self.code_point_start(offset);
        self.index = self.seek(start);
        if self.boundaries[self.index] == start {
            self.previous()
        } else {
            self.done = false;
            to_i32(self.boundaries[self.index])
        }
    }

    /// Whether the last `next`, `previous`, `following` or `preceding` ran off the text.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.done
    }
}

// Port of: icu4c/source/common/ubrk.cpp ubrk_open (chromium/deps/icu d578f2e8)
/// `ubrk_open(type, locale, nullptr, 0, &status)`. See [`UBreakIterator::open`].
///
/// # Errors
/// As [`UBreakIterator::open`].
pub fn ubrk_open(
    kind: UBreakIteratorType,
    locale: Option<&str>,
) -> Result<UBreakIterator, UErrorCode> {
    UBreakIterator::open(kind, locale)
}
