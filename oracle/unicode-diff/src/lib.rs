// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

// `ICU4X`, `HarfBuzz` and similar are proper names, not code.
#![allow(clippy::doc_markdown)]

//! `oracle/unicode-diff`: ICU4X and `unicode-bidi` against Chromium's ICU 78.2 at Skia's pin
//! (`docs/design/modules.md` §3.3, task M0c).
//!
//! The C++ driver (`cpp/unicode_diff.cc`, built by `build.sh` from the sources
//! `cargo xtask text-deps fetch --only icu` downloads) makes the calls of Skia's `SKICU_EMIT_FUNCS`
//! (`modules/skunicode/src/SkUnicode_icupriv.h`) over a corpus and prints the results; the committed
//! `expected/*.txt` are that output. This crate makes the same calls through ICU4X `=2.3.0`
//! (`icu_segmenter`, `icu_properties`, `icu_casemap`) and `unicode-bidi =0.3.18` with the stock
//! compiled data, prints the same text, and [`check`] compares the two, the way `oracle/shaper-diff`
//! replays its stored HarfBuzz results, so `cargo test` needs no C/C++.
//!
//! Sections ([`Section`]): `breaks`, `bidi`, `reorder`, `props`, `case`, `conformance`. A divergence is
//! a string whose result differs ([`Divergence`] with the key `case source index`), a range of code
//! points whose property value differs (`prop name first-last icu/icu4x`), or a conformance-test line
//! one side fails and the other passes. Each is either listed in `known-divergences.txt` with a class
//! or the test fails, and a listed divergence that starts matching fails too (so the list only
//! shrinks). There are no tolerances.

pub mod bidi;
pub mod breaks;
pub mod case;
pub mod conformance;
pub mod props;
pub mod reorder;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// `oracle/unicode-diff`.
#[must_use]
pub fn crate_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// A section of the output: one expected file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Section {
    /// `ubrk_*` over every string: line, word, character and sentence boundaries with the raw rule
    /// status, in UTF-8 and UTF-16, plus `ubrk_following` / `ubrk_preceding`.
    Breaks,
    /// `ubidi_*` over every string: direction, length and levels.
    Bidi,
    /// `ubidi_reorderVisual` over all level sequences of two alphabets.
    Reorder,
    /// `u_hasBinaryProperty`, `u_getIntPropertyValue(UCHAR_LINE_BREAK)`, `u_iscntrl`, `u_isspace`,
    /// `u_isWhitespace` and `uscript_getScript` over all code points.
    Props,
    /// `u_strToUpper` over every code point and string, per locale.
    Case,
    /// The UAX #29, #14 and #9 conformance files through the break and bidi calls.
    Conformance,
}

impl Section {
    /// All sections, in the order of the drivers.
    pub const ALL: [Section; 6] = [
        Section::Breaks,
        Section::Bidi,
        Section::Reorder,
        Section::Props,
        Section::Case,
        Section::Conformance,
    ];

    /// The section's name: the driver's argument and the expected file's stem.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Section::Breaks => "breaks",
            Section::Bidi => "bidi",
            Section::Reorder => "reorder",
            Section::Props => "props",
            Section::Case => "case",
            Section::Conformance => "conformance",
        }
    }

    /// Parses a section name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Section> {
        Section::ALL.into_iter().find(|s| s.name() == name)
    }

    /// `expected/<name>.txt`.
    #[must_use]
    pub fn expected_path(self) -> PathBuf {
        crate_dir()
            .join("expected")
            .join(format!("{}.txt", self.name()))
    }
}

/// One text source of the corpus: a file of `corpus/` and its strings.
#[derive(Debug)]
pub struct Source {
    /// The file stem.
    pub id: String,
    /// The strings, as UTF-8.
    pub strings: Vec<String>,
}

fn hex(s: &str, pos: usize, n: usize) -> Result<u32, String> {
    let digits = s
        .get(pos..pos + n)
        .ok_or_else(|| format!("short escape in `{s}`"))?;
    u32::from_str_radix(digits, 16).map_err(|e| format!("bad escape in `{s}`: {e}"))
}

/// Decodes a `lines` corpus line: the escapes `\\ \n \r \t \xHH \uXXXX \UXXXXXXXX`.
fn unescape(line: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut chars = line.char_indices();
    while let Some((i, c)) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let (_, e) = chars.next().ok_or("dangling backslash")?;
        let (len, cp) = match e {
            'n' => (0, '\n' as u32),
            'r' => (0, '\r' as u32),
            't' => (0, '\t' as u32),
            '\\' => (0, '\\' as u32),
            'x' => (2, hex(line, i + 2, 2)?),
            'u' => (4, hex(line, i + 2, 4)?),
            'U' => (8, hex(line, i + 2, 8)?),
            _ => return Err(format!("bad escape in corpus line `{line}`")),
        };
        for _ in 0..len {
            chars.next();
        }
        out.push(char::from_u32(cp).ok_or_else(|| format!("bad scalar value in `{line}`"))?);
    }
    Ok(out)
}

/// Loads `corpus/` (see `gen_corpus.py`), in the order of `corpus/index.txt`.
///
/// # Errors
/// If a file cannot be read or has a bad escape.
pub fn load_corpus() -> Result<Vec<Source>, String> {
    let dir = crate_dir().join("corpus");
    let index = std::fs::read_to_string(dir.join("index.txt")).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for line in index
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let (name, kind) = line
            .split_once(' ')
            .ok_or_else(|| format!("bad index line `{line}`"))?;
        let text = std::fs::read_to_string(dir.join(name)).map_err(|e| format!("{name}: {e}"))?;
        let strings = if kind == "whole" {
            vec![text]
        } else {
            let mut v = Vec::new();
            for l in text.split_inclusive('\n') {
                v.push(unescape(l.strip_suffix('\n').unwrap_or(l))?);
            }
            v
        };
        out.push(Source {
            id: name.trim_end_matches(".txt").to_owned(),
            strings,
        });
    }
    Ok(out)
}

/// SHA-256 of `text`, the first 16 hex digits (the drivers' per-string hash).
#[must_use]
pub fn hash16(text: &str) -> String {
    let mut out = String::with_capacity(16);
    for b in &Sha256::digest(text.as_bytes())[..8] {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// One string's result, as the driver prints it: `<case> <source> <idx> <n> <hash16>`, or with
/// `dump` the detail instead of `n` and the hash.
#[must_use]
pub fn row(case: &str, source: &Source, idx: usize, n: usize, detail: &str, dump: bool) -> String {
    if dump {
        format!("{case} {} {idx} {detail}\n", source.id)
    } else {
        format!("{case} {} {idx} {n} {}\n", source.id, hash16(detail))
    }
}

/// The UTF-16 code units of `s`.
#[must_use]
pub fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Makes the output of `section` (what the C++ driver prints).
///
/// # Errors
/// If the corpus or (for `conformance`) the conformance files cannot be read.
pub fn compute(section: Section, dump: bool) -> Result<String, String> {
    match section {
        Section::Breaks => Ok(breaks::run(&load_corpus()?, dump)),
        Section::Bidi => Ok(bidi::run(&load_corpus()?, dump)),
        Section::Reorder => Ok(reorder::run(dump)),
        Section::Props => Ok(props::run()),
        Section::Case => Ok(case::run(&load_corpus()?, dump)),
        Section::Conformance => conformance::run(),
    }
}

/// Reads `expected/<section>.txt`.
///
/// # Errors
/// If it is missing.
pub fn read_expected(section: Section) -> Result<String, String> {
    let path = section.expected_path();
    std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
}

/// One place where ICU4X (or `unicode-bidi`) and ICU differ.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    /// The key listed in `known-divergences.txt` (without the class).
    pub key: String,
    /// ICU's result (its row's `n hash`, or the value).
    pub icu: String,
    /// The twin's.
    pub rust: String,
}

/// Conformance totals per file, and the failing lines with the rule of their first wrong slot.
type Fails = (BTreeMap<String, String>, BTreeMap<(String, String), String>);

/// A run of code points with one value: `<first> <last> <value>` of a `prop` or `cp` line.
type Run = (u32, u32, String);

fn parse_runs(lines: &[&str]) -> Result<BTreeMap<String, Vec<Run>>, String> {
    let mut out: BTreeMap<String, Vec<Run>> = BTreeMap::new();
    for l in lines {
        let f: Vec<&str> = l.split(' ').collect();
        let [kind, name, first, last, value] = f[..] else {
            return Err(format!("bad range line `{l}`"));
        };
        let p = |s: &str| u32::from_str_radix(s, 16).map_err(|e| format!("{l}: {e}"));
        out.entry(format!("{kind} {name}")).or_default().push((
            p(first)?,
            p(last)?,
            value.to_owned(),
        ));
    }
    Ok(out)
}

fn diff_runs(group: &str, icu: &[Run], rust: &[Run], out: &mut Vec<Divergence>) {
    let (mut i, mut j) = (0, 0);
    let mut at = 0u32;
    // The pending divergence: (first, last, icu value, rust value).
    let mut pending: Option<(u32, u32, String, String)> = None;
    let flush = |p: &mut Option<(u32, u32, String, String)>, out: &mut Vec<Divergence>| {
        if let Some((first, last, a, b)) = p.take() {
            out.push(Divergence {
                key: format!("{group} {first:x}-{last:x} {a}/{b}"),
                icu: a,
                rust: b,
            });
        }
    };
    while i < icu.len() || j < rust.len() {
        let a = icu.get(i);
        let b = rust.get(j);
        let (av, ae) = a.map_or(("(none)", u32::MAX), |r| (r.2.as_str(), r.1));
        let (bv, be) = b.map_or(("(none)", u32::MAX), |r| (r.2.as_str(), r.1));
        let end = ae.min(be);
        if av == bv {
            flush(&mut pending, out);
        } else {
            match &mut pending {
                Some((_, last, pa, pb)) if *last + 1 == at && pa == av && pb == bv => *last = end,
                _ => {
                    flush(&mut pending, out);
                    pending = Some((at, end, av.to_owned(), bv.to_owned()));
                }
            }
        }
        if ae == end {
            i += 1;
        }
        if be == end {
            j += 1;
        }
        at = end.wrapping_add(1);
    }
    flush(&mut pending, out);
}

fn split_lines(text: &str) -> (Vec<&str>, Vec<&str>, Vec<&str>) {
    let (mut rows, mut ranges, mut conf) = (Vec::new(), Vec::new(), Vec::new());
    for l in text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        match l.split(' ').next() {
            Some("cp" | "prop") => ranges.push(l),
            Some("conf") => conf.push(l),
            _ => rows.push(l),
        }
    }
    (rows, ranges, conf)
}

fn row_map<'a>(rows: &[&'a str]) -> Result<BTreeMap<String, &'a str>, String> {
    let mut out = BTreeMap::new();
    for l in rows {
        let mut it = l.splitn(4, ' ');
        let (Some(a), Some(b), Some(c), Some(rest)) = (it.next(), it.next(), it.next(), it.next())
        else {
            return Err(format!("bad row `{l}`"));
        };
        out.insert(format!("{a} {b} {c}"), rest);
    }
    Ok(out)
}

/// Compares ICU's output with the twin's.
///
/// # Errors
/// If a line is malformed.
pub fn diff(icu_text: &str, rust_text: &str) -> Result<Vec<Divergence>, String> {
    let (icu_rows, icu_ranges, icu_conf) = split_lines(icu_text);
    let (rust_rows, rust_ranges, rust_conf) = split_lines(rust_text);
    let mut out = Vec::new();

    let (icu_rows, rust_rows) = (row_map(&icu_rows)?, row_map(&rust_rows)?);
    for (key, v) in &icu_rows {
        match rust_rows.get(key) {
            Some(r) if r == v => {}
            other => out.push(Divergence {
                key: key.clone(),
                icu: (*v).to_owned(),
                rust: other.map_or("(none)", |s| s).to_owned(),
            }),
        }
    }
    for (key, v) in &rust_rows {
        if !icu_rows.contains_key(key) {
            out.push(Divergence {
                key: key.clone(),
                icu: "(none)".to_owned(),
                rust: (*v).to_owned(),
            });
        }
    }

    let (icu_runs, rust_runs) = (parse_runs(&icu_ranges)?, parse_runs(&rust_ranges)?);
    let groups: BTreeSet<&String> = icu_runs.keys().chain(rust_runs.keys()).collect();
    for g in groups {
        let empty = Vec::new();
        diff_runs(
            g,
            icu_runs.get(g).unwrap_or(&empty),
            rust_runs.get(g).unwrap_or(&empty),
            &mut out,
        );
    }

    // Conformance: `conf <file> total <n>` must be equal; `conf <file> fail <line> <rule>` sets are compared.
    let fails = |lines: &[&str]| -> Result<Fails, String> {
        let mut totals = BTreeMap::new();
        let mut set = BTreeMap::new();
        for l in lines {
            let f: Vec<&str> = l.split(' ').collect();
            match f[..] {
                [_, file, "total", n] => {
                    totals.insert(file.to_owned(), n.to_owned());
                }
                [_, file, "fail", n, rule] => {
                    set.insert((file.to_owned(), n.to_owned()), rule.to_owned());
                }
                _ => return Err(format!("bad conf line `{l}`")),
            }
        }
        Ok((totals, set))
    };
    let ((icu_totals, icu_fail), (rust_totals, rust_fail)) =
        (fails(&icu_conf)?, fails(&rust_conf)?);
    if icu_totals != rust_totals {
        return Err(format!(
            "conformance totals differ: {icu_totals:?} vs {rust_totals:?}"
        ));
    }
    for ((file, line), rule) in &icu_fail {
        if !rust_fail.contains_key(&(file.clone(), line.clone())) {
            out.push(Divergence {
                key: format!("conf {file} {line} icu-only"),
                icu: format!("fail@{rule}"),
                rust: "pass".to_owned(),
            });
        }
    }
    for ((file, line), rule) in &rust_fail {
        if !icu_fail.contains_key(&(file.clone(), line.clone())) {
            out.push(Divergence {
                key: format!("conf {file} {line} icu4x-only"),
                icu: "pass".to_owned(),
                rust: format!("fail@{rule}"),
            });
        }
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(out)
}

/// `known-divergences.txt`: `<key> <class>` per divergence.
///
/// # Errors
/// If the file cannot be read or has a line without a class.
pub fn read_known() -> Result<BTreeMap<String, String>, String> {
    let path = crate_dir().join("known-divergences.txt");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = BTreeMap::new();
    for l in text
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let (key, class) = l
            .rsplit_once(' ')
            .ok_or_else(|| format!("bad known-divergences line `{l}`"))?;
        out.insert(key.to_owned(), class.to_owned());
    }
    Ok(out)
}

/// The result of [`check`].
#[derive(Debug, Default)]
pub struct Report {
    /// Rows, ranges and conformance lines compared (ICU's side).
    pub compared: usize,
    /// Divergences listed in `known-divergences.txt`, per class.
    pub known: BTreeMap<String, usize>,
    /// Divergences that are not listed.
    pub unexpected: Vec<String>,
    /// Listed divergences of this section that no longer diverge.
    pub stale: Vec<String>,
}

impl Report {
    /// No unlisted divergence and no stale listing.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.unexpected.is_empty() && self.stale.is_empty()
    }
}

/// Which section a known-divergence key belongs to.
#[must_use]
pub fn key_section(key: &str) -> Section {
    let first = key.split(' ').next().unwrap_or("");
    match first {
        "prop" => Section::Props,
        "conf" => Section::Conformance,
        "cp" => Section::Case,
        c if c.starts_with("bidi-") => Section::Bidi,
        c if c.starts_with("upper-") => Section::Case,
        "reorder" => Section::Reorder,
        _ => Section::Breaks,
    }
}

/// Runs the twin for `section` and compares it with the expected file and `known-divergences.txt`.
///
/// # Errors
/// If an input cannot be read.
pub fn check(section: Section) -> Result<Report, String> {
    let expected = read_expected(section)?;
    let computed = compute(section, false)?;
    let divergences = diff(&expected, &computed)?;
    let known = read_known()?;
    let mut report = Report {
        compared: expected
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .count(),
        ..Report::default()
    };
    let mut seen = BTreeSet::new();
    for d in &divergences {
        seen.insert(d.key.as_str());
        match known.get(&d.key) {
            Some(class) => *report.known.entry(class.clone()).or_default() += 1,
            None => report
                .unexpected
                .push(format!("{} (icu {} / icu4x {})", d.key, d.icu, d.rust)),
        }
    }
    for key in known.keys() {
        if key_section(key) == section && !seen.contains(key.as_str()) {
            report.stale.push(key.clone());
        }
    }
    Ok(report)
}
