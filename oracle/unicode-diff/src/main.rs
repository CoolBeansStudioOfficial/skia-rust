// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `unicode-diff run <section> [--dump]` prints a section in the C++ driver's format (`classify.py`
//! reads the dumps); `unicode-diff check [section...]` compares the sections with `expected/` and
//! `known-divergences.txt` and prints a report per class; `unicode-diff divergences <section>` lists every
//! divergence for `classify.py`.

use std::process::ExitCode;

use skia_rust_unicode_diff::{Section, check, compute};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => {
            let dump = args.iter().any(|a| a == "--dump");
            let Some(section) = args.get(1).and_then(|s| Section::parse(s)) else {
                eprintln!("usage: unicode-diff run <section> [--dump]");
                return ExitCode::FAILURE;
            };
            match compute(section, dump) {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("divergences") => {
            // Every divergence of a section, `key<TAB>icu<TAB>icu4x` (what classify.py classifies).
            let Some(section) = args.get(1).and_then(|s| Section::parse(s)) else {
                eprintln!("usage: unicode-diff divergences <section>");
                return ExitCode::FAILURE;
            };
            let result = skia_rust_unicode_diff::read_expected(section).and_then(|expected| {
                let computed = compute(section, false)?;
                skia_rust_unicode_diff::diff(&expected, &computed)
            });
            match result {
                Ok(list) => {
                    for d in list {
                        println!("{}\t{}\t{}", d.key, d.icu, d.rust);
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("check") => {
            let sections: Vec<Section> = if args.len() > 1 {
                args[1..].iter().filter_map(|s| Section::parse(s)).collect()
            } else {
                Section::ALL.to_vec()
            };
            let mut ok = true;
            for section in sections {
                match check(section) {
                    Ok(r) => {
                        println!(
                            "{}: {} rows, known {:?}, {} unexpected, {} stale",
                            section.name(),
                            r.compared,
                            r.known,
                            r.unexpected.len(),
                            r.stale.len()
                        );
                        for u in r.unexpected.iter().take(10) {
                            println!("  unexpected: {u}");
                        }
                        for u in r.stale.iter().take(10) {
                            println!("  stale: {u}");
                        }
                        ok &= r.is_clean();
                    }
                    Err(e) => {
                        eprintln!("{}: {e}", section.name());
                        ok = false;
                    }
                }
            }
            if ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        _ => {
            eprintln!(
                "usage: unicode-diff run <section> [--dump] | check [section...] | divergences <section>"
            );
            ExitCode::FAILURE
        }
    }
}
