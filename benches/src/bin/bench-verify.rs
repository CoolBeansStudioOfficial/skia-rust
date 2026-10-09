// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `bench-verify`: gives every registered benchmark site a verdict for `cargo xtask inventory
//! verify` (design `docs/design/bench.md` §3.3), like `gm-verify` does for GMs.
//!
//! A site is smoke-run (criterion C2) in every smoke config it is suitable for. The verdict is:
//! - `failing`: a benchmark panicked, or the site breaks its registration rules.
//! - `passing`: C2 holds and the site has no output to compare (every benchmark is
//!   non-rendering).
//! - `ported`: C2 holds, but the site renders, and the bench goldens that criterion C3 needs do
//!   not exist yet (design §3.1). `inventory verify --update` marks it `ported`, not `passing`.
//!
//! ```text
//! bench-verify [--match SUBSTRING] [--report FILE]
//! ```

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::ExitCode;

use serde_json::json;
use skia_rust_bench::registry::{self, BenchRegistration};
use skia_rust_bench::smoke::{run_smoke_test, smoke_bench};

/// The verdict of one site.
fn verdict(reg: &BenchRegistration) -> (&'static str, String) {
    // The smoke test itself (names, sizes, units, one sample per config).
    if let Err(e) = catch_unwind(AssertUnwindSafe(|| run_smoke_test(reg))) {
        return ("failing", panic_message(e.as_ref()));
    }
    // Whether anything renders: a fresh set, since a bench keeps state between samples.
    let rendering = catch_unwind(AssertUnwindSafe(|| {
        (reg.factory)()
            .iter_mut()
            .any(|b| smoke_bench(b.as_mut()).iter().any(|r| r.bytes.is_some()))
    }));
    match rendering {
        Ok(false) => ("passing", String::new()),
        Ok(true) => ("ported", "renders; no bench goldens (C3) yet".to_owned()),
        Err(e) => ("failing", panic_message(e.as_ref())),
    }
}

fn panic_message(e: &(dyn std::any::Any + Send)) -> String {
    e.downcast_ref::<String>()
        .cloned()
        .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "panicked".to_owned())
}

fn main() -> ExitCode {
    let mut filter: Option<String> = None;
    let mut report: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--match" => filter = args.next(),
            "--report" => report = args.next(),
            other => {
                eprintln!("bench-verify: unknown argument `{other}`");
                return ExitCode::FAILURE;
            }
        }
    }

    let mut entries = Vec::new();
    let mut failed = 0;
    for reg in registry::all() {
        let key = reg.key();
        if filter.as_ref().is_some_and(|f| !key.contains(f.as_str())) {
            continue;
        }
        let (verdict, detail) = verdict(reg);
        if verdict == "failing" {
            failed += 1;
        }
        println!("{verdict:8} {key} {detail}");
        entries.push(json!({"key": key, "verdict": verdict, "detail": detail}));
    }
    if let Some(path) = report {
        let text = json!({"benches": entries}).to_string();
        if let Err(e) = std::fs::write(&path, text) {
            eprintln!("bench-verify: {path}: {e}");
            return ExitCode::FAILURE;
        }
    }
    // Like gm-verify, the exit status is about running, not about verdicts: xtask reads the report.
    if failed > 0 {
        eprintln!("bench-verify: {failed} failing");
    }
    ExitCode::SUCCESS
}
