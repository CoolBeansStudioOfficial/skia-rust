// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `shaper-diff`: the Rust side of `oracle/shaper-diff`, driven by `cargo xtask oracle shaper-diff`.
//!
//! * `shaper-diff tables <dir>` writes mode b's table files for the C++ driver.
//! * `shaper-diff run <a|b|c> [--config NAME] [--font NAME]` prints the rows HarfRust produces, in
//!   the C++ driver's format.
//! * `shaper-diff check` compares every mode with `expected/` and `known-divergences.txt`.

// `HarfBuzz` and `HarfRust` are proper names, not code.
#![allow(clippy::doc_markdown)]

use std::path::PathBuf;
use std::process::ExitCode;

use skia_rust_shaper_diff::{Mode, Report, check, compute, write_tables};

fn mode_of(letter: &str) -> Result<Mode, String> {
    match letter {
        "a" => Ok(Mode::Builtin),
        "b" => Ok(Mode::SkiaFuncs),
        "c" => Ok(Mode::EmptyFace),
        _ => Err(format!("unknown mode `{letter}`")),
    }
}

fn run() -> Result<bool, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("tables") => {
            let dir = PathBuf::from(args.get(1).ok_or("usage: shaper-diff tables <dir>")?);
            let n = write_tables(&dir)?;
            println!("wrote {n} tables to {}", dir.display());
            Ok(true)
        }
        Some("run") => {
            let mode = mode_of(args.get(1).ok_or("usage: shaper-diff run <a|b|c>")?)?;
            let mut config = None;
            let mut font = None;
            let mut rest = args[2..].iter();
            while let Some(a) = rest.next() {
                match a.as_str() {
                    "--config" => config = rest.next().map(String::as_str),
                    "--font" => font = rest.next().map(String::as_str),
                    _ => return Err(format!("unknown argument `{a}`")),
                }
            }
            for row in compute(mode, config, font)? {
                println!("{}", row.to_line());
            }
            Ok(true)
        }
        Some("check") => {
            let mut clean = true;
            for mode in Mode::ALL {
                let report = check(&compute(mode, None, None)?, mode)?;
                print_report(mode, &report);
                clean &= report.is_clean();
            }
            Ok(clean)
        }
        _ => Err("usage: shaper-diff <tables|run|check> ...".to_owned()),
    }
}

fn print_report(mode: Mode, r: &Report) {
    println!(
        "{mode:?}: {} rows, {} exact, {} known divergences, {} unexpected, {} stale, {} missing",
        r.compared,
        r.exact,
        r.known.values().sum::<usize>(),
        r.unexpected.len(),
        r.stale.len(),
        r.missing.len()
    );
    for (class, n) in &r.known {
        println!("  known {class}: {n}");
    }
    for line in r
        .unexpected
        .iter()
        .chain(&r.stale)
        .chain(&r.missing)
        .take(20)
    {
        println!("  {line}");
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("shaper-diff: {e}");
            ExitCode::from(2)
        }
    }
}
