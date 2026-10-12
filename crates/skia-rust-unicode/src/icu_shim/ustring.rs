// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `u_strToUpper` on `icu_casemap`, with ICU's choice of case-mapping locale.
//!
//! `oracle/unicode-diff`'s `case` section compares ICU4X's uppercasing with Chromium's ICU for the
//! root, Turkish, Azerbaijani, Lithuanian and Greek rules over every code point and every corpus
//! string: all exact. What this file adds is ICU's mapping from a locale ID to those rules
//! ([`ucase_get_case_locale`]).

use icu_casemap::CaseMapper;
use icu_locale_core::{LanguageIdentifier, langid};

use super::uloc::uloc_get_default;

/// ICU's case-mapping locales (`ucase.h` `UCASE_LOC_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaseLocale {
    /// `UCASE_LOC_ROOT`.
    Root,
    /// `UCASE_LOC_TURKISH`: `tr`, `tur`, `az`, `aze`.
    Turkish,
    /// `UCASE_LOC_LITHUANIAN`: `lt`, `lit`.
    Lithuanian,
    /// `UCASE_LOC_GREEK`: `el`, `ell`.
    Greek,
    /// `UCASE_LOC_DUTCH`: `nl`, `nld` (titlecasing only).
    Dutch,
    /// `UCASE_LOC_ARMENIAN`: `hy`, `hye` (not `hyw`).
    Armenian,
}

// Port of: icu4c/source/common/ucase.cpp#L795-L989 (chromium/deps/icu d578f2e8)
/// `ucase_getCaseLocale(locale)`: the case-mapping rules of a locale ID, from its language code
/// alone (2 or 3 letters, any case, followed by `_`, `-` or the end).
#[doc(alias = "ucase_getCaseLocale")]
#[must_use]
pub fn ucase_get_case_locale(locale: &str) -> CaseLocale {
    let b = locale.as_bytes();
    // `*locale++` past the end reads the terminating NUL, which `is_sep` accepts.
    let at = |i: usize| b.get(i).copied().unwrap_or(0);
    let is_sep = |c: u8| c == b'_' || c == b'-' || c == 0;
    let is = |c: u8, letter: u8| c.eq_ignore_ascii_case(&letter);
    // The language code `first` + `second` [+ `third`], then a separator.
    let lang = |second: u8, third: u8| -> bool {
        if !is(at(1), second) {
            return false;
        }
        let mut c = at(2);
        if is(c, third) {
            c = at(3);
        }
        is_sep(c)
    };
    // `tr`/`tur` and `lt`/`lit` have the optional middle letter: the third letter of the 3-letter
    // code is the second of the 2-letter code.
    let lang_mid = |mid: u8, last: u8| -> bool {
        let mut i = 1;
        if is(at(i), mid) {
            i += 1;
        }
        is(at(i), last) && is_sep(at(i + 1))
    };
    match at(0) {
        // Lowercase `e` and uppercase `E`: only `el`/`ell`. ICU checks lowercase `e` before the
        // other lowercase letters and `z` (Chinese) first; neither changes the result.
        b'e' | b'E' if lang(b'l', b'l') => CaseLocale::Greek,
        b't' | b'T' if lang_mid(b'u', b'r') => CaseLocale::Turkish,
        b'a' | b'A' if lang(b'z', b'e') => CaseLocale::Turkish,
        b'l' | b'L' if lang_mid(b'i', b't') => CaseLocale::Lithuanian,
        b'n' | b'N' if lang(b'l', b'd') => CaseLocale::Dutch,
        b'h' | b'H' if lang(b'y', b'e') => CaseLocale::Armenian,
        _ => CaseLocale::Root,
    }
}

/// The ICU4X language that selects the same rules as `locale` does in ICU.
fn case_language(locale: CaseLocale) -> LanguageIdentifier {
    match locale {
        CaseLocale::Root => LanguageIdentifier::UNKNOWN,
        CaseLocale::Turkish => langid!("tr"),
        CaseLocale::Lithuanian => langid!("lt"),
        CaseLocale::Greek => langid!("el"),
        CaseLocale::Dutch => langid!("nl"),
        CaseLocale::Armenian => langid!("hy"),
    }
}

// Port of: icu4c/source/common/ustrcase.cpp u_strToUpper (chromium/deps/icu d578f2e8)
/// `u_strToUpper(dest, destCapacity, src, srcLength, locale, &status)`: the uppercase of the UTF-16
/// text `src` with the case rules of `locale` (the default locale when `None`, as for a null
/// pointer in ICU). The result is what ICU writes; its length is ICU's return value.
///
/// Unpaired surrogates are copied unchanged, as ICU does; the text between them is mapped as a
/// whole.
#[doc(alias = "u_strToUpper")]
#[must_use]
pub fn u_str_to_upper(src: &[u16], locale: Option<&str>) -> Vec<u16> {
    let default;
    let locale = if let Some(l) = locale {
        l
    } else {
        default = uloc_get_default();
        default
    };
    let lang = case_language(ucase_get_case_locale(locale));
    let mapper = CaseMapper::new();
    let mut out = Vec::with_capacity(src.len());
    let mut run = String::new();
    for unit in char::decode_utf16(src.iter().copied()) {
        match unit {
            Ok(c) => run.push(c),
            Err(e) => {
                out.extend(mapper.uppercase_to_string(&run, &lang).encode_utf16());
                run.clear();
                out.push(e.unpaired_surrogate());
            }
        }
    }
    out.extend(mapper.uppercase_to_string(&run, &lang).encode_utf16());
    out
}
