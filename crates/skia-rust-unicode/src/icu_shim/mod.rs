// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `icu_shim`: the ICU functions Skia's `SkUnicode_icu.cpp` calls (`SKICU_EMIT_FUNCS`,
//! `modules/skunicode/src/SkUnicode_icupriv.h#L21-L52`), with the semantics of Chromium's ICU 78.2 at
//! Skia's pin (`chromium/deps/icu` d578f2e8), implemented on ICU4X `=2.3.x` (milestone M2 of
//! `docs/design/modules.md`).
//!
//! | ICU | Here |
//! |---|---|
//! | `u_errorName` | [`u_error_name`] |
//! | `u_hasBinaryProperty`, `u_getIntPropertyValue` | [`u_has_binary_property`], [`u_get_int_property_value`] |
//! | `u_iscntrl`, `u_isspace`, `u_isWhitespace` | [`u_iscntrl`], [`u_isspace`], [`u_is_whitespace`] |
//! | `u_strToUpper` | [`u_str_to_upper`] |
//! | `uscript_getScript` | [`uscript_get_script`] |
//! | `uloc_forLanguageTag`, `uloc_getDefault` | [`uloc_for_language_tag`], [`uloc_get_default`] |
//! | `ubrk_open`, `ubrk_close`, `ubrk_clone` | [`ubrk_open`] / [`UBreakIterator::open`], `Drop`, `Clone` |
//! | `utext_openUTF8`, `utext_openUChars`, `utext_close`, `ubrk_setUText`, `ubrk_setText` | [`UBreakIterator::set_text_utf8`], [`UBreakIterator::set_text_utf16`] |
//! | `ubrk_first`, `ubrk_next`, `ubrk_current`, `ubrk_following`, `ubrk_preceding`, `ubrk_getRuleStatus` | the methods of [`UBreakIterator`] |
//! | `ubidi_*` | milestone M3 (`SkUnicode_icu_bidi.cpp`) |
//!
//! What ICU4X does not provide and the shim adds: ICU's numbering of `Line_Break` and `Script`,
//! ICU's definitions of `u_isspace` / `u_isWhitespace` / `u_iscntrl`, ICU's choice of case-mapping
//! locale, and ICU's choice of break rules per locale. Break iterators run, by default, a port of
//! ICU's `RuleBasedBreakIterator` and dictionary engines (`rbbi.cpp`, `rbbi_cache.cpp`,
//! `dictbe.cpp`, milestone M4) over Chromium's compiled break data ([`ChromiumData`], from the
//! `skia-rust-unicode-data` crate); with ICU4X's segmenters instead ([`CompiledData`], milestone
//! M2), the shim emulates the rule status and navigation of ICU around their boundaries
//! ([`data::BreakDataSource`] is the seam).
//!
//! `oracle/unicode-diff` runs every function here against Chromium's ICU over its corpus
//! (`docs/design/modules.md` §3.4-§3.6).

pub mod data;
mod dictbe;
mod dictionary_data;
mod rbbi;
mod rbbi_data;
mod rule_status;
pub mod ubrk;
pub mod uchar;
mod ucptrie;
pub mod uloc;
pub mod ustring;
mod utext;
pub mod utypes;
mod word_rules;

pub use data::{BreakDataSource, ChromiumData, CompiledData, LineStrictness};
pub use ubrk::{
    BreakRules, UBRK_DONE, UBreakIterator, UBreakIteratorType, resolve_rules, ubrk_open,
};
pub use uchar::{
    UChar32, UProperty, u_get_int_property_value, u_has_binary_property, u_is_whitespace,
    u_iscntrl, u_isspace, u_lb, uscript_get_script,
};
pub use uloc::{ForLanguageTag, uloc_for_language_tag, uloc_get_default};
pub use ustring::{CaseLocale, u_str_to_upper, ucase_get_case_locale};
pub use utypes::{UErrorCode, u_error_name, u_failure};

/// `UBRK_WORD_NONE` .. `UBRK_WORD_IDEO`, `UBRK_LINE_SOFT` / `UBRK_LINE_HARD`,
/// `UBRK_SENTENCE_TERM` / `UBRK_SENTENCE_SEP` (`ubrk.h`): the lower bounds of the rule-status ranges.
pub mod status {
    /// `UBRK_WORD_NONE`.
    pub const UBRK_WORD_NONE: i32 = 0;
    /// `UBRK_WORD_NUMBER`.
    pub const UBRK_WORD_NUMBER: i32 = 100;
    /// `UBRK_WORD_LETTER`.
    pub const UBRK_WORD_LETTER: i32 = 200;
    /// `UBRK_WORD_KANA`.
    pub const UBRK_WORD_KANA: i32 = 300;
    /// `UBRK_WORD_IDEO`.
    pub const UBRK_WORD_IDEO: i32 = 400;
    /// `UBRK_LINE_SOFT`.
    pub const UBRK_LINE_SOFT: i32 = 0;
    /// `UBRK_LINE_HARD`.
    pub const UBRK_LINE_HARD: i32 = 100;
    /// `UBRK_SENTENCE_TERM`.
    pub const UBRK_SENTENCE_TERM: i32 = 0;
    /// `UBRK_SENTENCE_SEP`.
    pub const UBRK_SENTENCE_SEP: i32 = 100;
}
