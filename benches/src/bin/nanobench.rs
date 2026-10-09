// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/nanobench.cpp (main, the CPU path)

//! `nanobench` for skia-rust: runs the registered benchmarks like Skia's nanobench and writes its
//! `--outResultsFile` JSON. Build the perf binary with
//! `cargo build --release -p skia-rust-bench --bin nanobench`.
//!
//! ```text
//! nanobench [--samples N] [--ms N] [--loops N] [--maxLoops N] [--overheadGoal X]
//!           [--overheadLoops N] [--maxCalibrationAttempts N] [--match PATTERN...]
//!           [--config TAG...] [--quiet] [--dryRun] [--outResultsFile FILE] [--writeRaw DIR]
//! ```

use std::process::ExitCode;

use skia_rust_bench::nanobench::{Flags, run};
use skia_rust_bench::registry;

fn parse_flags(args: &[String]) -> Result<Flags, String> {
    fn value<'a>(args: &'a [String], i: &mut usize, flag: &str) -> Result<&'a str, String> {
        *i += 1;
        args.get(*i)
            .map(String::as_str)
            .ok_or_else(|| format!("{flag} needs a value"))
    }
    fn number<T: std::str::FromStr>(s: &str, flag: &str) -> Result<T, String> {
        s.parse().map_err(|_| format!("{flag}: bad number `{s}`"))
    }

    let mut flags = Flags::default();
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].as_str();
        match flag {
            "--samples" => flags.samples = number(value(args, &mut i, flag)?, flag)?,
            "--ms" => flags.ms = number(value(args, &mut i, flag)?, flag)?,
            "--loops" => flags.loops = number(value(args, &mut i, flag)?, flag)?,
            "--maxLoops" => flags.max_loops = number(value(args, &mut i, flag)?, flag)?,
            "--overheadLoops" => flags.overhead_loops = number(value(args, &mut i, flag)?, flag)?,
            "--overheadGoal" => flags.overhead_goal = number(value(args, &mut i, flag)?, flag)?,
            "--maxCalibrationAttempts" => {
                flags.max_calibration_attempts = number(value(args, &mut i, flag)?, flag)?;
            }
            "--outResultsFile" => {
                flags.out_results_file = Some(value(args, &mut i, flag)?.into());
            }
            "--writeRaw" => flags.write_raw = Some(value(args, &mut i, flag)?.into()),
            "--quiet" => flags.quiet = true,
            "--dryRun" => flags.dry_run = true,
            // Multi-valued flags take every following argument that is not a flag.
            "--match" | "--config" => {
                let mut values = Vec::new();
                while args.get(i + 1).is_some_and(|a| !a.starts_with("--")) {
                    i += 1;
                    values.push(args[i].clone());
                }
                if flag == "--match" {
                    flags.match_ = values;
                } else {
                    flags.configs = values;
                }
            }
            other => return Err(format!("unknown flag `{other}`")),
        }
        i += 1;
    }
    Ok(flags)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flags = match parse_flags(&args) {
        Ok(flags) => flags,
        Err(e) => {
            eprintln!("nanobench: {e}");
            return ExitCode::FAILURE;
        }
    };
    let log = match run(&flags, &registry::all()) {
        Ok(log) => log,
        Err(e) => {
            eprintln!("nanobench: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Some(path) = &flags.out_results_file {
        let text = serde_json::to_string_pretty(&log.to_json()).expect("JSON values serialize");
        if let Err(e) = std::fs::write(path, text) {
            eprintln!("nanobench: {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
