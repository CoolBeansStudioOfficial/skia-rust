// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The `case` section: `u_strToUpper` (`SkUnicode_icu.cpp#L495-L520`) through `icu_casemap =2.3.0`,
//! over every code point (`cp <locale> <first> <last> <value>` runs) and every corpus string
//! (`upper-<locale>` rows), for the locales root, `tr`, `az`, `lt`, `el`.
//!
//! A run value is `=` when the code point maps to itself, `-` for a surrogate, else the result's UTF-16
//! units in uppercase hex joined by `.`.

use std::fmt::Write as _;

use icu_casemap::CaseMapper;
use icu_locale_core::LanguageIdentifier;

use crate::props::runs;
use crate::{Source, row};

fn locale(tag: &str) -> LanguageIdentifier {
    if tag.is_empty() {
        LanguageIdentifier::UNKNOWN
    } else {
        tag.parse().expect("a valid language tag")
    }
}

fn hex_units(s: &str) -> String {
    let mut out = String::new();
    for u in s.encode_utf16() {
        if !out.is_empty() {
            out.push('.');
        }
        let _ = write!(out, "{u:04X}");
    }
    out
}

fn upper(
    mapper: icu_casemap::CaseMapperBorrowed<'_>,
    s: &str,
    lang: &LanguageIdentifier,
) -> String {
    let r = hex_units(&mapper.uppercase_to_string(s, lang));
    if r.is_empty() { String::from("-") } else { r }
}

/// Prints the section.
#[must_use]
pub fn run(corpus: &[Source], dump: bool) -> String {
    let mapper = CaseMapper::new();
    let mut out = String::new();
    for tag in ["", "tr", "az", "lt", "el"] {
        let lang = locale(tag);
        let name = if tag.is_empty() { "root" } else { tag };
        runs(
            &mut out,
            "cp",
            name,
            &|c| {
                let Some(ch) = char::from_u32(c) else {
                    return String::from("-");
                };
                let mut buf = [0u8; 4];
                let s: &str = ch.encode_utf8(&mut buf);
                let r = upper(mapper, s, &lang);
                if r == hex_units(s) {
                    String::from("=")
                } else {
                    r
                }
            },
            &|v| v == "=" || v == "-",
        );
    }
    for tag in ["", "tr", "lt", "el"] {
        let lang = locale(tag);
        let case = format!("upper-{}", if tag.is_empty() { "root" } else { tag });
        for s in corpus {
            for (i, text) in s.strings.iter().enumerate() {
                out += &row(
                    &case,
                    s,
                    i,
                    text.encode_utf16().count(),
                    &upper(mapper, text, &lang),
                    dump,
                );
            }
        }
    }
    out
}
