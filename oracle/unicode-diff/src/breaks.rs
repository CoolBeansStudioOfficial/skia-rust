// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The `breaks` section: `ubrk_open`, `ubrk_setUText`, `ubrk_first`, `ubrk_next`, `ubrk_current`,
//! `ubrk_getRuleStatus`, `ubrk_following` and `ubrk_preceding` of `SkUnicode_icu.cpp#L330-L410`,
//! through `skia_rust_unicode::icu_shim` (milestone M2): ICU4X's segmenters (compiled data) with
//! ICU's rule status and navigation emulated, opened for the locale `en` as the driver does.
//!
//! What still differs is the break data (Chromium's patched rules and dictionaries, milestone M4).

use std::fmt::Write as _;

use skia_rust_unicode::icu_shim::{UBRK_DONE, UBreakIterator, UBreakIteratorType};

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

/// `ubrk_open(kind, "en")` + `ubrk_setUText(text)`.
///
/// # Panics
/// Never: the root rules of every kind exist.
#[must_use]
pub fn open(kind: Kind, text: Text<'_>) -> UBreakIterator {
    let kind = match kind {
        Kind::Char => UBreakIteratorType::Character,
        Kind::Word => UBreakIteratorType::Word,
        Kind::Line => UBreakIteratorType::Line,
        Kind::Sentence => UBreakIteratorType::Sentence,
    };
    let mut it = UBreakIterator::open(kind, Some("en")).expect("the root rules exist");
    match text {
        Text::Utf8(s) => it.set_text_utf8(s.as_bytes()),
        Text::Utf16(s) => it.set_text_utf16(s),
    }
    it
}

/// `(position, rule status)` for `first()` and every `next()` until `UBRK_DONE`.
#[must_use]
pub fn boundaries(kind: Kind, text: Text<'_>) -> Vec<(usize, i32)> {
    let mut it = open(kind, text);
    let mut out = Vec::new();
    let mut pos = it.first();
    while pos != UBRK_DONE {
        assert_eq!(it.current(), pos, "ubrk_current != position");
        out.push((
            usize::try_from(pos).expect("positions are not negative"),
            it.get_rule_status(),
        ));
        pos = it.next();
    }
    out
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

/// `ubrk_following(i)` and `ubrk_preceding(i)` for every offset `0..=len` (`-1` is `UBRK_DONE`).
fn navigate(kind: Kind, text: Text<'_>, len: usize) -> (String, usize) {
    let mut it = open(kind, text);
    let mut out = String::new();
    for i in 0..=len {
        if i > 0 {
            out.push(' ');
        }
        let offset = i32::try_from(i).expect("corpus strings are short");
        let f = it.following(offset);
        let p = it.preceding(offset);
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
