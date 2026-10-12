// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The `conformance` section: the UAX #29 (`GraphemeBreakTest`, `WordBreakTest`, `SentenceBreakTest`),
//! UAX #14 (`LineBreakTest`) and UAX #9 (`BidiCharacterTest`) files of Unicode 17.0.0
//! (`inventory/text-deps.lock` `[ucd]`, fetched like the other inputs) through the break and bidi
//! calls of the other sections.
//!
//! Output: `conf <file> total <n>` and `conf <file> fail <line>` for every test line the engine does
//! not reproduce. The comparison is between the two engines' failing lines: a line ICU fails and the
//! twin passes (or the reverse) is a divergence.

use std::fmt::Write as _;

use crate::bidi::{Para, levels_utf16};
use crate::breaks::{Kind, Text, boundaries};

/// The rules a test line's comment names, one per slot: the `[9.0]` items.
fn comment_rules(comment: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = comment;
    while let Some(i) = rest.find('[') {
        let Some(j) = rest[i..].find(']') else { break };
        let r = &rest[i + 1..i + j];
        if !r.is_empty() && r.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
            out.push(r);
        }
        rest = &rest[i + j + 1..];
    }
    out
}

fn breaks_file(
    out: &mut String,
    dir: &std::path::Path,
    file: &str,
    kind: Kind,
) -> Result<(), String> {
    let text = std::fs::read_to_string(dir.join(file)).map_err(|e| format!("{file}: {e}"))?;
    let mut total = 0;
    let mut failing = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let (data, comment) = line.split_once('#').unwrap_or((line, ""));
        let mut s = String::new();
        // Slot k is the position after k characters; `want[k]` says whether the file has a break there.
        let mut offsets = Vec::new();
        let mut want = Vec::new();
        for (i, tok) in data.split_whitespace().enumerate() {
            if i % 2 == 0 {
                offsets.push(s.len());
                want.push(tok == "\u{f7}");
            } else {
                s.push(
                    u32::from_str_radix(tok, 16)
                        .ok()
                        .and_then(char::from_u32)
                        .ok_or_else(|| format!("{file}:{}: bad token `{tok}`", n + 1))?,
                );
            }
        }
        if offsets.is_empty() {
            continue;
        }
        total += 1;
        // ICU always reports the boundary at 0 (the line test files start with × by LB2).
        want[0] = true;
        let actual: Vec<usize> = boundaries(kind, Text::Utf8(&s))
            .into_iter()
            .map(|(p, _)| p)
            .collect();
        let rules = comment_rules(comment);
        for (k, off) in offsets.iter().enumerate() {
            if actual.contains(off) != want[k] {
                failing.push((n + 1, rules.get(k).copied().unwrap_or("?")));
                break;
            }
        }
    }
    let _ = writeln!(out, "conf {file} total {total}");
    for (l, rule) in failing {
        let _ = writeln!(out, "conf {file} fail {l} {rule}");
    }
    Ok(())
}

fn bidi_file(out: &mut String, dir: &std::path::Path) -> Result<(), String> {
    let file = "BidiCharacterTest.txt";
    let text = std::fs::read_to_string(dir.join(file)).map_err(|e| format!("{file}: {e}"))?;
    let mut total = 0;
    let mut failing = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split(';').collect();
        if f.len() < 5 {
            continue;
        }
        total += 1;
        let cps: Vec<char> = f[0]
            .split_whitespace()
            .map(|t| {
                u32::from_str_radix(t, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or_else(|| format!("{file}:{}: bad token `{t}`", n + 1))
            })
            .collect::<Result<_, _>>()?;
        let para = match f[1] {
            "0" => Para::Ltr,
            "1" => Para::Rtl,
            _ => Para::AutoLtr,
        };
        let s: String = cps.iter().collect();
        let levels = levels_utf16(&s, para);
        let mut unit = 0;
        let mut ok = true;
        for (c, lv) in cps.iter().zip(f[3].split_whitespace()) {
            if lv != "x" && levels.get(unit).map(u8::to_string).as_deref() != Some(lv) {
                ok = false;
            }
            unit += c.len_utf16();
        }
        if !ok {
            failing.push(n + 1);
        }
    }
    let _ = writeln!(out, "conf {file} total {total}");
    for l in failing {
        let _ = writeln!(out, "conf {file} fail {l} -");
    }
    Ok(())
}

/// Prints the section.
///
/// # Errors
/// If the conformance files cannot be fetched or read.
pub fn run() -> Result<String, String> {
    let dir = skia_rust_text_deps::ucd()?;
    let mut out = String::new();
    breaks_file(&mut out, &dir, "GraphemeBreakTest.txt", Kind::Char)?;
    breaks_file(&mut out, &dir, "WordBreakTest.txt", Kind::Word)?;
    breaks_file(&mut out, &dir, "SentenceBreakTest.txt", Kind::Sentence)?;
    breaks_file(&mut out, &dir, "LineBreakTest.txt", Kind::Line)?;
    bidi_file(&mut out, &dir)?;
    Ok(out)
}
