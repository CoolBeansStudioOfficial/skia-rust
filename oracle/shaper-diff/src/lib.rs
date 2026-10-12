// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `oracle/shaper-diff`: HarfRust against HarfBuzz at Skia's pin (`docs/design/modules.md` §2.4).
//!
//! The C++ driver (`cpp/shaper_diff.cc`, built by `build.sh` from the sources
//! `cargo xtask text-deps fetch` downloads) shapes a corpus with HarfBuzz and prints, per
//! (config, font, text source), the line and glyph counts and the SHA-256 of the serialized
//! result. The committed `expected/*.txt` are that output. This crate does the same with
//! HarfRust `=0.9.0` and [`check`] compares the two, the way `oracle/rp-diff` replays its stored
//! Skia results, so `cargo test` needs no C/C++.
//!
//! The three modes are in `configs.txt`. Rows where HarfRust differs from HarfBuzz are listed in
//! `known-divergences.txt` with a class; every row is either exact or listed, and a listed row
//! that starts matching fails the test (so the list only shrinks).

// `HarfBuzz` and `HarfRust` are proper names, not code.
#![allow(clippy::doc_markdown)]

mod compute;
pub mod shape;
pub mod synthetic;
pub mod tables;

pub use compute::{compute, write_tables};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use shape::{Mode, ShapeConfig};

/// `oracle/shaper-diff`.
#[must_use]
pub fn crate_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// One expected (or computed) result: `<config> <font> <source> <lines> <glyphs> <sha256>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Config name from `configs.txt`.
    pub config: String,
    /// Font file name, or `(empty)` for mode c.
    pub font: String,
    /// Text source id (corpus file stem).
    pub source: String,
    /// Number of corpus lines shaped.
    pub lines: usize,
    /// Number of glyph records.
    pub glyphs: usize,
    /// SHA-256 of the serialized lines.
    pub sha256: String,
}

impl Row {
    /// The (config, font, source) key.
    #[must_use]
    pub fn key(&self) -> String {
        format!("{} {} {}", self.config, self.font, self.source)
    }

    /// The row as printed by the drivers.
    #[must_use]
    pub fn to_line(&self) -> String {
        format!(
            "{} {} {} {} {} {}",
            self.config, self.font, self.source, self.lines, self.glyphs, self.sha256
        )
    }

    /// Parses a row line.
    ///
    /// # Errors
    /// If the line does not have six fields.
    pub fn parse(line: &str) -> Result<Row, String> {
        let f: Vec<&str> = line.split_whitespace().collect();
        let [config, font, source, lines, glyphs, sha256] = f[..] else {
            return Err(format!("bad row `{line}`"));
        };
        Ok(Row {
            config: config.to_owned(),
            font: font.to_owned(),
            source: source.to_owned(),
            lines: lines.parse().map_err(|e| format!("{line}: {e}"))?,
            glyphs: glyphs.parse().map_err(|e| format!("{line}: {e}"))?,
            sha256: sha256.to_owned(),
        })
    }
}

/// Reads a whitespace-separated table, skipping blank lines and `#` comments.
///
/// # Errors
/// If the file cannot be read.
pub fn read_table(path: &Path) -> Result<Vec<Vec<String>>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| l.split_whitespace().map(str::to_owned).collect())
        .collect())
}

/// The corpus (`corpus/index.txt`): text sources in the drivers' order.
#[derive(Debug)]
pub struct Corpus {
    /// `(id, lines)` per source file.
    pub sources: Vec<(String, Vec<String>)>,
}

impl Corpus {
    /// Loads `oracle/shaper-diff/corpus/`.
    ///
    /// # Errors
    /// If a file cannot be read.
    pub fn load() -> Result<Corpus, String> {
        let dir = crate_dir().join("corpus");
        let mut sources = Vec::new();
        for row in read_table(&dir.join("index.txt"))? {
            let name = &row[0];
            let text =
                std::fs::read_to_string(dir.join(name)).map_err(|e| format!("{name}: {e}"))?;
            // The C++ driver's split: at '\n', '\r' stripped, no empty piece after a final '\n'.
            let mut lines = Vec::new();
            let mut rest = text.as_str();
            while !rest.is_empty() {
                let (line, tail) = rest.split_once('\n').unwrap_or((rest, ""));
                lines.push(line.strip_suffix('\r').unwrap_or(line).to_owned());
                rest = tail;
            }
            sources.push((name.trim_end_matches(".txt").to_owned(), lines));
        }
        Ok(Corpus { sources })
    }
}

/// The directory of the fonts of `fonts.txt`: `$SKIA_RUST_SHAPER_DIFF_FONTS`, else the
/// paragraph test fonts (`inventory/test-fonts.lock`, fetched and cached).
///
/// # Errors
/// If the fonts cannot be fetched.
pub fn fonts_dir() -> Result<PathBuf, String> {
    match std::env::var_os("SKIA_RUST_SHAPER_DIFF_FONTS").filter(|v| !v.is_empty()) {
        Some(dir) => Ok(PathBuf::from(dir)),
        None => skia_rust_text_deps::test_fonts(),
    }
}

/// `fonts.txt`: font file name and SHA-256 of the expected contents.
///
/// # Errors
/// If the file cannot be read.
pub fn font_list() -> Result<Vec<(String, String)>, String> {
    Ok(read_table(&crate_dir().join("fonts.txt"))?
        .into_iter()
        .map(|r| (r[0].clone(), r[1].clone()))
        .collect())
}

/// The expected-results file of a mode.
#[must_use]
pub fn expected_path(mode: Mode) -> PathBuf {
    crate_dir().join("expected").join(match mode {
        Mode::Builtin => "builtin.txt",
        Mode::SkiaFuncs => "skia-funcs.txt",
        Mode::EmptyFace => "empty-face.txt",
    })
}

/// Reads the expected rows of `mode`.
///
/// # Errors
/// If the file is missing or malformed.
pub fn read_expected(mode: Mode) -> Result<Vec<Row>, String> {
    let path = expected_path(mode);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(Row::parse)
        .collect()
}

/// `known-divergences.txt`: `<config> <font> <source> <class>` per row where HarfRust differs.
///
/// # Errors
/// If the file cannot be read.
pub fn read_known() -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    for row in read_table(&crate_dir().join("known-divergences.txt"))? {
        let [config, font, source, class] = &row[..] else {
            return Err(format!("bad known-divergences row {row:?}"));
        };
        out.insert(format!("{config} {font} {source}"), class.clone());
    }
    Ok(out)
}

/// The result of comparing computed rows with the expected ones.
#[derive(Debug, Default)]
pub struct Report {
    /// Rows compared.
    pub compared: usize,
    /// Rows that match exactly.
    pub exact: usize,
    /// Rows that differ and are not in `known-divergences.txt`.
    pub unexpected: Vec<String>,
    /// Rows in `known-divergences.txt` that now match.
    pub stale: Vec<String>,
    /// Rows that differ, by class, as listed.
    pub known: BTreeMap<String, usize>,
    /// Rows missing on one side.
    pub missing: Vec<String>,
}

impl Report {
    /// Whether the replay is clean.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.unexpected.is_empty() && self.stale.is_empty() && self.missing.is_empty()
    }
}

/// Compares `computed` rows against the expected rows of the same configs.
///
/// # Errors
/// If a file cannot be read.
pub fn check(computed: &[Row], mode: Mode) -> Result<Report, String> {
    let expected: BTreeMap<String, Row> = read_expected(mode)?
        .into_iter()
        .map(|r| (r.key(), r))
        .collect();
    let known = read_known()?;
    let mut report = Report::default();
    let mut seen = std::collections::BTreeSet::new();
    for row in computed {
        let key = row.key();
        seen.insert(key.clone());
        let Some(want) = expected.get(&key) else {
            report
                .missing
                .push(format!("{key}: not in the expected file"));
            continue;
        };
        report.compared += 1;
        if want == row {
            report.exact += 1;
            if known.contains_key(&key) {
                report.stale.push(format!(
                    "{key}: listed in known-divergences.txt but now exact"
                ));
            }
        } else if let Some(class) = known.get(&key) {
            *report.known.entry(class.clone()).or_default() += 1;
        } else {
            report.unexpected.push(format!(
                "{key}: expected {} lines / {} glyphs / {}, got {} / {} / {}",
                want.lines, want.glyphs, want.sha256, row.lines, row.glyphs, row.sha256
            ));
        }
    }
    for key in expected.keys() {
        if !seen.contains(key) {
            report
                .missing
                .push(format!("{key}: expected but not computed"));
        }
    }
    Ok(report)
}
