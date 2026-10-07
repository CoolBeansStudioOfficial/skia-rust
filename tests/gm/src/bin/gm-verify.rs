// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Checks every registered GM against the goldens and writes a JSON report; run by
//! `cargo xtask inventory verify`.
//!
//! ```text
//! gm-verify --report <file.json> [--no-diffs] [--match <substring>]...
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use serde::Serialize;
use skia_rust_gm::check::{self, GmReport, Options, Verdict};
use skia_rust_gm::goldens::GoldenStore;
use skia_rust_gm::registry;
use skia_rust_gm::sink::GmSrc;

#[derive(Serialize)]
struct PlanEntry {
    tier: &'static str,
    run: String,
    oracle_tiers: Vec<&'static str>,
    rgba_oracle_tiers: Vec<&'static str>,
}

#[derive(Serialize)]
struct Report {
    goldens: String,
    plan: Vec<PlanEntry>,
    gms: Vec<GmReport>,
}

fn main() -> ExitCode {
    let mut report_path: Option<PathBuf> = None;
    let mut diffs = true;
    let mut matches: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--report" => report_path = args.next().map(PathBuf::from),
            "--no-diffs" => diffs = false,
            "--match" => matches.extend(args.next()),
            other => {
                eprintln!("gm-verify: unknown argument `{other}`");
                return ExitCode::from(2);
            }
        }
    }

    let gms: Vec<_> = registry::all()
        .into_iter()
        .filter(|r| matches.is_empty() || matches.iter().any(|m| r.key().contains(m.as_str())))
        .collect();
    let mut report = Report {
        goldens: String::new(),
        plan: Vec::new(),
        gms: Vec::new(),
    };
    if !gms.is_empty() {
        let store = match GoldenStore::shared() {
            Ok(store) => store,
            Err(e) => {
                eprintln!("gm-verify: loading goldens: {e}");
                return ExitCode::FAILURE;
            }
        };
        let plan = check::plan(store);
        report.goldens.clone_from(&store.source);
        report.plan = plan
            .iter()
            .map(|p| PlanEntry {
                tier: p.tier.name(),
                run: match &p.run {
                    Ok(sel) => sel.to_string(),
                    Err(reason) => format!("not checkable: {reason}"),
                },
                oracle_tiers: p.oracle_tiers.clone(),
                rgba_oracle_tiers: p.rgba_oracle_tiers.clone(),
            })
            .collect();
        let opts = Options {
            diffs: diffs.then(skia_rust_gm::diff::diff_root),
            ..Options::default()
        };
        for reg in gms {
            let r = check::check_gm(
                &reg.key(),
                &GmSrc::from_registration(reg),
                store,
                &plan,
                &opts,
            );
            if r.verdict != Verdict::Passing {
                print!("{r}");
            }
            report.gms.push(r);
        }
    }
    let passing = report
        .gms
        .iter()
        .filter(|r| r.verdict == Verdict::Passing)
        .count();
    println!(
        "{passing}/{} GMs pass on every checkable tier",
        report.gms.len()
    );

    if let Some(path) = report_path {
        let json = serde_json::to_string_pretty(&report).expect("report serializes");
        if let Err(e) = std::fs::write(&path, json + "\n") {
            eprintln!("gm-verify: writing {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
