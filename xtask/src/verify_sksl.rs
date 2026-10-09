//! Maps the `tests/sksl` goldens to manifest `sksl-golden` entries and checks the manifest's
//! `passing` claims against `sksl-golden-verify` (`tests/src/bin/sksl-golden-verify.rs`).
//!
//! A golden is identified by its path under the Skia checkout (`tests/sksl/blend/BlendClear.wgsl`),
//! which is the manifest id verbatim. Without a checkout there is nothing to check, so the check is
//! skipped.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, ensure};

use crate::verify::{Outcome, Report, parse_test_output};

/// Runs `sksl-golden-verify` and returns each golden's outcome by manifest id, or `None` when the
/// Skia checkout is absent (the binary then has nothing to check).
pub fn run_sksl_golden_verify(root: &Path) -> Result<Option<BTreeMap<String, Outcome>>> {
    if !root.join("third_party/skia/gn/sksl_tests.gni").is_file() {
        println!("no Skia checkout: skipping the SkSL goldens");
        return Ok(None);
    }
    let out = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
        .current_dir(root)
        .args([
            "run",
            "-q",
            "-p",
            "skia-rust-tests",
            "--bin",
            "sksl-golden-verify",
        ])
        .output()
        .context("running sksl-golden-verify")?;
    ensure!(
        out.status.success(),
        "sksl-golden-verify failed with {}:\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(Some(parse_test_output(&String::from_utf8_lossy(
        &out.stdout,
    ))))
}

/// Compares golden outcomes with manifest statuses, as `verify::check` does for unit tests.
/// `entries` is (id, status) for every `sksl-golden` in the manifest.
pub fn check(entries: &[(String, String)], results: &BTreeMap<String, Outcome>) -> Report {
    let mut report = Report::default();
    let by_id: BTreeMap<&str, &str> = entries
        .iter()
        .map(|(id, status)| (id.as_str(), status.as_str()))
        .collect();
    for (id, outcome) in results {
        let Some(status) = by_id.get(id.as_str()) else {
            report.unknown.push(id.clone());
            continue;
        };
        match (*status, outcome) {
            ("excluded", _) => report.excluded_but_ported.push(id.clone()),
            ("passing", Outcome::Ok) => {}
            ("passing", _) => report.regressions.push(id.clone()),
            (other, Outcome::Ok) => report.newly_passing.push((id.clone(), other.to_owned())),
            (_, Outcome::Failed) => report.failing.push(id.clone()),
            (_, Outcome::Ignored) => {}
        }
    }
    for (id, status) in &by_id {
        if *status == "passing" && !results.contains_key(*id) {
            report.regressions.push(format!("{id} (no golden result)"));
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_classifies() {
        let entries = vec![
            ("tests/sksl/a.wgsl".to_owned(), "passing".to_owned()),
            ("tests/sksl/b.wgsl".to_owned(), "passing".to_owned()),
            ("tests/sksl/c.wgsl".to_owned(), "todo".to_owned()),
            ("tests/sksl/d.wgsl".to_owned(), "todo".to_owned()),
            ("tests/sksl/e.wgsl".to_owned(), "todo".to_owned()),
            ("tests/sksl/f.wgsl".to_owned(), "excluded".to_owned()),
        ];
        let results = BTreeMap::from([
            ("tests/sksl/a.wgsl".to_owned(), Outcome::Ok),
            ("tests/sksl/b.wgsl".to_owned(), Outcome::Failed),
            ("tests/sksl/c.wgsl".to_owned(), Outcome::Ok),
            ("tests/sksl/d.wgsl".to_owned(), Outcome::Failed),
            ("tests/sksl/e.wgsl".to_owned(), Outcome::Ignored),
            ("tests/sksl/f.wgsl".to_owned(), Outcome::Ok),
            ("tests/sksl/nope.wgsl".to_owned(), Outcome::Ok),
        ]);
        let r = check(&entries, &results);
        assert_eq!(r.unknown, ["tests/sksl/nope.wgsl"]);
        assert_eq!(r.excluded_but_ported, ["tests/sksl/f.wgsl"]);
        assert_eq!(r.regressions, ["tests/sksl/b.wgsl"]);
        assert_eq!(
            r.newly_passing,
            [("tests/sksl/c.wgsl".to_owned(), "todo".to_owned())]
        );
        assert_eq!(r.failing, ["tests/sksl/d.wgsl"]);
    }

    #[test]
    fn missing_result_for_passing_entry_is_a_regression() {
        let entries = vec![("tests/sksl/a.skrp".to_owned(), "passing".to_owned())];
        let r = check(&entries, &BTreeMap::new());
        assert_eq!(r.regressions, ["tests/sksl/a.skrp (no golden result)"]);
    }
}
