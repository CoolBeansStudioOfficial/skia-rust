// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The `breaks` section: `ubrk_open`, `ubrk_setUText`, `ubrk_first`, `ubrk_next`, `ubrk_current`,
//! `ubrk_getRuleStatus`, `ubrk_following` and `ubrk_preceding` of `SkUnicode_icu.cpp#L330-L410`,
//! through `skia_rust_unicode::icu_shim`, opened for the locale `en` as the driver does: the port of
//! ICU's `RuleBasedBreakIterator` and dictionary engines over Chromium's compiled break data
//! (milestone M4; the shim's default). All exact.

use std::fmt::Write as _;

use skia_rust_unicode::icu_shim::{UBRK_DONE, UBreakIterator, UBreakIteratorType, u_error_name};

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

/// `ubrk_open(kind, locale)` + `ubrk_setUText(text)`, or ICU's error.
///
/// # Errors
/// The `UErrorCode` of `ubrk_open`.
pub fn open_locale(kind: Kind, text: Text<'_>, locale: &str) -> Result<UBreakIterator, i32> {
    let kind = match kind {
        Kind::Char => UBreakIteratorType::Character,
        Kind::Word => UBreakIteratorType::Word,
        Kind::Line => UBreakIteratorType::Line,
        Kind::Sentence => UBreakIteratorType::Sentence,
    };
    let mut it = UBreakIterator::open(kind, Some(locale))?;
    match text {
        Text::Utf8(s) => it.set_text_utf8(s.as_bytes()),
        Text::Utf16(s) => it.set_text_utf16(s),
    }
    Ok(it)
}

/// `ubrk_open(kind, "en")` + `ubrk_setUText(text)`.
///
/// # Panics
/// Never: the root rules of every kind exist.
#[must_use]
pub fn open(kind: Kind, text: Text<'_>) -> UBreakIterator {
    open_locale(kind, text, "en").expect("the root rules exist")
}

/// `(position, rule status)` for `first()` and every `next()` until `UBRK_DONE`.
///
/// # Panics
/// If `ubrk_current` is not the position `next` returned (`SkBreakIterator_icu::current`).
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

fn enumerate(kind: Kind, text: Text<'_>, locale: &str) -> (String, usize) {
    let mut it = match open_locale(kind, text, locale) {
        Ok(it) => it,
        Err(e) => return (format!("open-error:{}", u_error_name(e)), 0),
    };
    let mut b = Vec::new();
    let mut pos = it.first();
    while pos != UBRK_DONE {
        b.push((pos, it.get_rule_status()));
        pos = it.next();
    }
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

/// The cases of the driver: name, kind, UTF-16 text, navigation instead of enumeration, locale.
const CASES: [(&str, Kind, bool, bool, &str); 17] = [
    ("char-u8", Kind::Char, false, false, "en"),
    ("char-u16", Kind::Char, true, false, "en"),
    ("line-u8", Kind::Line, false, false, "en"),
    ("line-u16", Kind::Line, true, false, "en"),
    ("word-u8", Kind::Word, false, false, "en"),
    ("word-u16", Kind::Word, true, false, "en"),
    ("sent-u8", Kind::Sentence, false, false, "en"),
    ("sent-u16", Kind::Sentence, true, false, "en"),
    ("nav-line-u8", Kind::Line, false, true, "en"),
    ("nav-word-u16", Kind::Word, true, true, "en"),
    ("line-zh-u8", Kind::Line, false, false, "zh_Hans_CN"),
    ("line-ja-u8", Kind::Line, false, false, "ja_JP"),
    (
        "line-ja-normal-u8",
        Kind::Line,
        false,
        false,
        "ja@lb=normal",
    ),
    ("line-strict-u8", Kind::Line, false, false, "en@lb=strict"),
    ("line-loose-u8", Kind::Line, false, false, "en_US@lb=loose"),
    (
        "line-ko-strict-u8",
        Kind::Line,
        false,
        false,
        "ko@lb=strict",
    ),
    ("sent-el-u8", Kind::Sentence, false, false, "el_GR"),
];

/// The number of per-string cases of the section.
pub const CASE_COUNT: usize = CASES.len();

/// Prints the section.
#[must_use]
pub fn run(corpus: &[Source], dump: bool) -> String {
    let mut out = String::new();
    for (name, kind, is16, nav, locale) in CASES {
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
                    enumerate(kind, t, locale)
                };
                out += &row(name, s, i, n, &detail, dump);
            }
        }
    }
    out
}
