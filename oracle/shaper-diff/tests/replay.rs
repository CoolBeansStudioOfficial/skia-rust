// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: the call sequence of oracle/shaper-diff/cpp/shaper_diff.cc (Skia's SkShaper_harfbuzz.cpp
// over HarfBuzz 9cb1fee5), replayed against HarfRust 0.9.0.

// `HarfBuzz` and `HarfRust` are proper names, not code.
#![allow(clippy::doc_markdown)]

//! Replays the stored HarfBuzz results (`expected/*.txt`) against HarfRust, with no C/C++ build.
//!
//! Every row (config, font, text source) is either exact or listed with its class in
//! `known-divergences.txt`; there is no tolerance. Modes a and b need the paragraph test fonts
//! (`inventory/test-fonts.lock`: fetched and cached like the goldens, or
//! `$SKIA_RUST_SHAPER_DIFF_FONTS` for a directory of them); mode c needs nothing.

use skia_rust_shaper_diff::{Mode, Report, check, compute};

fn replay(mode: Mode) {
    let rows = compute(mode, None, None).unwrap_or_else(|e| panic!("{mode:?}: {e}"));
    let report = check(&rows, mode).unwrap_or_else(|e| panic!("{mode:?}: {e}"));
    assert_clean(mode, &report);
}

fn assert_clean(mode: Mode, report: &Report) {
    let problems: Vec<&String> = report
        .unexpected
        .iter()
        .chain(&report.stale)
        .chain(&report.missing)
        .take(20)
        .collect();
    assert!(
        report.is_clean(),
        "{mode:?}: {} rows, {} exact, {} known; problems (first 20 of {}): {problems:#?}\n\
         Regenerate with `cargo xtask oracle shaper-diff --update` only if the corpus or a pin changed.",
        report.compared,
        report.exact,
        report.known.values().sum::<usize>(),
        report.unexpected.len() + report.stale.len() + report.missing.len(),
    );
    assert!(report.compared > 0, "{mode:?}: nothing was compared");
}

#[test]
fn mode_a_builtin_font_functions() {
    replay(Mode::Builtin);
}

#[test]
fn mode_b_skia_font_functions() {
    replay(Mode::SkiaFuncs);
}

#[test]
fn mode_c_empty_face() {
    replay(Mode::EmptyFace);
}

/// The expected files cover the whole matrix of configs, fonts and sources.
#[test]
fn expected_files_cover_the_matrix() {
    let configs = skia_rust_shaper_diff::shape::ShapeConfig::load().unwrap();
    let fonts = skia_rust_shaper_diff::font_list().unwrap().len();
    let sources = skia_rust_shaper_diff::Corpus::load().unwrap().sources.len();
    for mode in Mode::ALL {
        let n = configs.iter().filter(|c| c.mode == mode).count();
        let per_config = if mode == Mode::EmptyFace { 1 } else { fonts } * sources;
        let rows = skia_rust_shaper_diff::read_expected(mode).unwrap();
        assert_eq!(rows.len(), n * per_config, "{mode:?}");
    }
}

/// The classes listed in `known-divergences.txt` name rows that exist.
#[test]
fn known_divergences_name_expected_rows() {
    let known = skia_rust_shaper_diff::read_known().unwrap();
    let mut keys = std::collections::BTreeSet::new();
    for mode in Mode::ALL {
        for row in skia_rust_shaper_diff::read_expected(mode).unwrap() {
            keys.insert(row.key());
        }
    }
    for key in known.keys() {
        assert!(
            keys.contains(key),
            "known divergence `{key}` is not an expected row"
        );
    }
}
