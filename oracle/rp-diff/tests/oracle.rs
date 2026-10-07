// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Replays every rp-diff case on every selection this host can run for each x86 Skia code path
//! and checks the outputs against the stored results of real Skia (`expected/<tier>.txt`,
//! written on the oracle host by `cargo xtask oracle rp-diff --update`).

use skia_rust_rp_diff::cases;
use skia_rust_rp_diff::expected::{self, Expected, ORACLE_TIERS};

#[test]
fn stages_match_skia() {
    let cases = cases::all();
    let mut problems = Vec::new();
    for tier in ORACLE_TIERS {
        let Some(stored) = Expected::load(tier).unwrap() else {
            problems.push(format!(
                "{}: no stored results ({}); run `cargo xtask oracle rp-diff --update` on the \
                 oracle host",
                tier.name(),
                expected::expected_path(tier).display()
            ));
            continue;
        };
        let names: Vec<String> = expected::selections(tier)
            .into_iter()
            .map(expected::selection_name)
            .collect();
        println!(
            "{}: {} cases on {}",
            tier.name(),
            cases.len(),
            names.join(", ")
        );
        problems.extend(expected::check(tier, &cases, &stored));
    }
    assert!(
        problems.is_empty(),
        "{} rp-diff problems (first 40):\n{}\nDebug on the oracle host with `cargo xtask oracle \
         rp-diff --tier <tier> --case-glob <name>`, which shows the differing bytes.",
        problems.len(),
        problems
            .iter()
            .take(40)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
