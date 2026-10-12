// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The `breaks` section: `ubrk_open`, `ubrk_setUText`, `ubrk_first`, `ubrk_next`, `ubrk_current`,
//! `ubrk_getRuleStatus`, `ubrk_following` and `ubrk_preceding` of `SkUnicode_icu.cpp#L330-L410`, through
//! ICU4X's segmenters (compiled data, `new_auto`, line strictness `Normal` as Chromium's
//! `line_normal.patch` makes ICU's default).
//!
//! ICU4X has no rule status except `WordType`, so the twin reports what it has and nothing is
//! emulated here: words `None` = 0, `Number` = 100, `Letter` = 200; everything else 0. The differences
//! this leaves are what the shim of M2 has to emulate.

use std::fmt::Write as _;

use icu_segmenter::options::{
    LineBreakOptions, LineBreakStrictness, LineBreakWordOption, SentenceBreakInvariantOptions,
    WordBreakInvariantOptions, WordType,
};
use icu_segmenter::{GraphemeClusterSegmenter, LineSegmenter, SentenceSegmenter, WordSegmenter};

use crate::{Source, row, utf16};

/// The kind of boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `UBRK_CHARACTER`.
    Char,
    /// `UBRK_WORD`.
    Word,
    /// `UBRK_LINE`.
    Line,
    /// `UBRK_SENTENCE`.
    Sentence,
}

/// The text a break iterator is run over.
#[derive(Clone, Copy, Debug)]
pub enum Text<'a> {
    /// `utext_openUTF8`: offsets are bytes.
    Utf8(&'a str),
    /// `utext_openUChars`: offsets are UTF-16 code units.
    Utf16(&'a [u16]),
}

/// `(position, rule status)` for `first()` and every `next()` until `UBRK_DONE`.
///
/// # Panics
/// Never: the segmenters are built from compiled data.
#[must_use]
pub fn boundaries(kind: Kind, text: Text<'_>) -> Vec<(usize, i32)> {
    macro_rules! plain {
        ($it:expr) => {{ $it.map(|p| (p, 0)).collect() }};
    }
    match kind {
        Kind::Char => {
            let seg = GraphemeClusterSegmenter::new();
            match text {
                Text::Utf8(s) => plain!(seg.segment_str(s)),
                Text::Utf16(s) => plain!(seg.segment_utf16(s)),
            }
        }
        Kind::Sentence => {
            let seg = SentenceSegmenter::new(SentenceBreakInvariantOptions::default());
            match text {
                Text::Utf8(s) => plain!(seg.segment_str(s)),
                Text::Utf16(s) => plain!(seg.segment_utf16(s)),
            }
        }
        Kind::Line => {
            let mut options = LineBreakOptions::default();
            options.strictness = Some(LineBreakStrictness::Normal);
            options.word_option = Some(LineBreakWordOption::Normal);
            let seg = LineSegmenter::new_auto(options);
            match text {
                Text::Utf8(s) => plain!(seg.segment_str(s)),
                Text::Utf16(s) => plain!(seg.segment_utf16(s)),
            }
        }
        Kind::Word => {
            let seg = WordSegmenter::new_auto(WordBreakInvariantOptions::default());
            let status = |t: WordType| match t {
                WordType::Number => 100,
                WordType::Letter => 200,
                _ => 0,
            };
            match text {
                Text::Utf8(s) => seg
                    .segment_str(s)
                    .iter_with_word_type()
                    .map(|(p, t)| (p, status(t)))
                    .collect(),
                Text::Utf16(s) => seg
                    .segment_utf16(s)
                    .iter_with_word_type()
                    .map(|(p, t)| (p, status(t)))
                    .collect(),
            }
        }
    }
}

fn enumerate(kind: Kind, text: Text<'_>) -> (String, usize) {
    let b = boundaries(kind, text);
    let mut out = String::new();
    for (i, (p, s)) in b.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let _ = write!(out, "{p}:{s}");
    }
    (out, b.len())
}

/// `ubrk_following(i)` and `ubrk_preceding(i)` for every offset `0..=len` (`-1` is `UBRK_DONE`),
/// from the boundary list.
fn navigate(kind: Kind, text: Text<'_>, len: usize) -> (String, usize) {
    let b: Vec<usize> = boundaries(kind, text).into_iter().map(|(p, _)| p).collect();
    let mut out = String::new();
    for i in 0..=len {
        if i > 0 {
            out.push(' ');
        }
        let f = b
            .iter()
            .find(|&&p| p > i)
            .map_or(String::from("-1"), ToString::to_string);
        let p = b
            .iter()
            .rev()
            .find(|&&p| p < i)
            .map_or(String::from("-1"), ToString::to_string);
        let _ = write!(out, "{i}>{f}<{p}");
    }
    (out, len + 1)
}

/// The cases of the driver: name, kind, UTF-16 text, navigation instead of enumeration.
const CASES: [(&str, Kind, bool, bool); 10] = [
    ("char-u8", Kind::Char, false, false),
    ("char-u16", Kind::Char, true, false),
    ("line-u8", Kind::Line, false, false),
    ("line-u16", Kind::Line, true, false),
    ("word-u8", Kind::Word, false, false),
    ("word-u16", Kind::Word, true, false),
    ("sent-u8", Kind::Sentence, false, false),
    ("sent-u16", Kind::Sentence, true, false),
    ("nav-line-u8", Kind::Line, false, true),
    ("nav-word-u16", Kind::Word, true, true),
];

/// Prints the section.
#[must_use]
pub fn run(corpus: &[Source], dump: bool) -> String {
    let mut out = String::new();
    for (name, kind, is16, nav) in CASES {
        for s in corpus {
            for (i, text) in s.strings.iter().enumerate() {
                let u16s = utf16(text);
                let (t, len) = if is16 {
                    (Text::Utf16(&u16s), u16s.len())
                } else {
                    (Text::Utf8(text), text.len())
                };
                let (detail, n) = if nav {
                    navigate(kind, t, len)
                } else {
                    enumerate(kind, t)
                };
                out += &row(name, s, i, n, &detail, dump);
            }
        }
    }
    out
}
