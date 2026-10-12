// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The data seam of the shim: where the segmenters behind `ubrk_*` come from.
//!
//! The shim asks a [`BreakDataSource`] for an ICU4X segmenter per break-rule file ICU would load
//! (`char.brk`, `word.brk`, `sent.brk`, `sent_el.brk`, `line*.brk`). The boundaries the shim reports
//! are that segmenter's. For lines it also asks for the rule-based segmenter alone: ICU computes its
//! boundaries in two passes (`rbbi.cpp` `handleNext`, then
//! `RuleBasedBreakIterator::DictionaryCache::populateDictionary` over each rule-based range that
//! holds dictionary characters), and the rule status of a dictionary boundary is the status of the
//! rule-based boundary that ends its range. (For words the shim runs the rule-based pass of
//! Chromium's `word.txt` itself, `super::word_rules`, because the statuses depend on more than the
//! ranges.)
//!
//! [`CompiledData`] is ICU4X's compiled data (stock UAX #14 / #29 rules, LSTM models for Thai, Lao,
//! Khmer and Myanmar, the CJ dictionary). Milestone M4 (`docs/design/modules.md` §9) adds a source
//! with Chromium's patched ICU break data: it implements this trait over its own provider (the
//! `try_new_*_unstable` constructors of `icu_segmenter`), and
//! [`UBreakIterator::open_with_data`](super::UBreakIterator::open_with_data) takes it.

use std::fmt::Debug;

use icu_locale_core::{LanguageIdentifier, langid};
use icu_segmenter::options::{
    LineBreakOptions, LineBreakStrictness, LineBreakWordOption, SentenceBreakInvariantOptions,
    SentenceBreakOptions, WordBreakInvariantOptions,
};
use icu_segmenter::{GraphemeClusterSegmenter, LineSegmenter, SentenceSegmenter, WordSegmenter};

/// The strictness part of a line-rule file name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineStrictness {
    /// `line.brk` (`line_strict`): CSS `line-break: strict`.
    Strict,
    /// `line_normal.brk`: CSS `line-break: normal`, Chromium's default (`line_normal.patch`).
    Normal,
    /// `line_loose.brk`: CSS `line-break: loose`.
    Loose,
}

/// The source of the segmenters the shim runs. See the module documentation.
///
/// A break iterator asks for its segmenters once, when it is opened (as ICU loads its rule file in
/// `ubrk_open`), and shares them with its clones.
pub trait BreakDataSource: Debug + Send + Sync {
    /// The grapheme-cluster segmenter (`char.brk`).
    fn grapheme(&self) -> GraphemeClusterSegmenter;

    /// The word segmenter (`word.brk`), with the dictionaries or models of complex scripts.
    fn word(&self) -> WordSegmenter;

    /// The sentence segmenter (`sent.brk`, or `sent_el.brk` when `greek`).
    fn sentence(&self, greek: bool) -> SentenceSegmenter;

    /// The line segmenter for a `line*.brk` file (`cj`: the `_cj` variants): with `complex`,
    /// including the dictionaries or models of complex scripts; without, the rules alone.
    fn line(&self, strictness: LineStrictness, cj: bool, complex: bool) -> LineSegmenter;
}

/// The ICU4X options that stand for a line-rule file.
#[must_use]
pub fn line_options(strictness: LineStrictness, cj: bool) -> LineBreakOptions<'static> {
    // ICU4X applies its Chinese/Japanese tailoring when the content language is `ja` or `zh`;
    // either stands for ICU's `_cj` rule files.
    static ZH: LanguageIdentifier = langid!("zh");
    let mut options = LineBreakOptions::default();
    options.strictness = Some(match strictness {
        LineStrictness::Strict => LineBreakStrictness::Strict,
        LineStrictness::Normal => LineBreakStrictness::Normal,
        LineStrictness::Loose => LineBreakStrictness::Loose,
    });
    options.word_option = Some(LineBreakWordOption::Normal);
    options.content_locale = if cj { Some(&ZH) } else { None };
    options
}

/// The ICU4X options that stand for `sent.brk` or `sent_el.brk`.
#[must_use]
pub fn sentence_options(greek: bool) -> SentenceBreakOptions<'static> {
    static EL: LanguageIdentifier = langid!("el");
    let mut options = SentenceBreakOptions::default();
    options.content_locale = if greek { Some(&EL) } else { None };
    options
}

/// ICU4X's compiled data: stock UAX #14 / #29 rules, LSTM models for the South-East Asian
/// scripts and the CJ dictionary (the `new_auto` constructors).
#[derive(Debug, Default, Clone, Copy)]
pub struct CompiledData;

impl BreakDataSource for CompiledData {
    fn grapheme(&self) -> GraphemeClusterSegmenter {
        GraphemeClusterSegmenter::new().static_to_owned()
    }

    fn word(&self) -> WordSegmenter {
        WordSegmenter::new_auto(WordBreakInvariantOptions::default()).static_to_owned()
    }

    fn sentence(&self, greek: bool) -> SentenceSegmenter {
        // Loading compiled data does not fail; the invariant segmenter is what a source without
        // the Greek tailoring would give.
        SentenceSegmenter::try_new(sentence_options(greek)).unwrap_or_else(|_| {
            SentenceSegmenter::new(SentenceBreakInvariantOptions::default()).static_to_owned()
        })
    }

    fn line(&self, strictness: LineStrictness, cj: bool, complex: bool) -> LineSegmenter {
        let options = line_options(strictness, cj);
        if complex {
            LineSegmenter::new_auto(options).static_to_owned()
        } else {
            LineSegmenter::new_for_non_complex_scripts(options).static_to_owned()
        }
    }
}
