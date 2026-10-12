// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: the call sequence of oracle/unicode-diff/cpp/unicode_diff.cc (Skia's SkUnicode_icu.cpp and
// SkUnicode_icu_bidi.cpp over Chromium ICU 78.2), replayed against ICU4X 2.3 and unicode-bidi 0.3.18.

// `ICU4X` and `unicode-bidi` are proper names, not code.
#![allow(clippy::doc_markdown)]

//! Replays the stored ICU results (`expected/*.txt`) against ICU4X and `unicode-bidi`, with no C/C++.
//!
//! Every string, code point range and conformance line is either exact or listed with its class in
//! `known-divergences.txt`; there is no tolerance. The `conformance` section needs the UAX
//! conformance files (`inventory/text-deps.lock` `[ucd]`: fetched and cached, or
//! `$SKIA_RUST_UCD_DIR` for a directory of them).

use skia_rust_unicode_diff::{Report, Section, check};

fn replay(section: Section) {
    let report = check(section).unwrap_or_else(|e| panic!("{}: {e}", section.name()));
    assert_clean(section, &report);
}

fn assert_clean(section: Section, report: &Report) {
    let problems: Vec<&String> = report
        .unexpected
        .iter()
        .chain(&report.stale)
        .take(20)
        .collect();
    assert!(
        report.is_clean(),
        "{}: {} rows, known {:?}; problems (first 20 of {}): {problems:#?}\n\
         Regenerate with `cargo xtask oracle unicode-diff --update` and classify.py only if the corpus or a pin changed.",
        section.name(),
        report.compared,
        report.known,
        report.unexpected.len() + report.stale.len(),
    );
    assert!(
        report.compared > 0,
        "{}: nothing was compared",
        section.name()
    );
}

#[test]
fn breaks_line_word_char_sentence() {
    replay(Section::Breaks);
}

#[test]
fn bidi_levels_and_direction() {
    replay(Section::Bidi);
}

#[test]
fn reorder_visual() {
    replay(Section::Reorder);
}

#[test]
fn properties_over_all_code_points() {
    replay(Section::Props);
}

#[test]
fn to_upper_per_locale() {
    replay(Section::Case);
}

#[test]
fn uax_conformance_files() {
    replay(Section::Conformance);
}

/// The expected files cover the corpus: every string has a row in every per-string case.
#[test]
fn expected_files_cover_the_corpus() {
    let strings: usize = skia_rust_unicode_diff::load_corpus()
        .unwrap()
        .iter()
        .map(|s| s.strings.len())
        .sum();
    for (section, cases) in [
        (Section::Breaks, 10),
        (Section::Bidi, 4),
        (Section::Case, 4),
    ] {
        let text = skia_rust_unicode_diff::read_expected(section).unwrap();
        let rows = text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.starts_with("cp ") && !l.is_empty())
            .count();
        assert_eq!(rows, cases * strings, "{}", section.name());
    }
}

/// Every key in `known-divergences.txt` names a divergence of its section (a stale entry fails
/// there; this catches a class that is empty or a key with no section).
#[test]
fn known_divergences_are_classified() {
    let known = skia_rust_unicode_diff::read_known().unwrap();
    assert!(!known.is_empty());
    for (key, class) in &known {
        assert!(
            !class.contains("unclassified"),
            "`{key}` is unclassified: root-cause it and extend classify.py"
        );
    }
}
