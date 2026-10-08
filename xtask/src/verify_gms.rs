//! Maps ported GMs back to manifest entries and checks the manifest's `passing` claims
//! against the GM harness (`tests/gm`, `gm-verify`).
//!
//! A GM registration `gm/<dir>/<File>.cpp::<Name>` is registered by `def_gm!` & co. in the
//! module `gm::<dir_snake>::<file_snake>` of the `skia-rust-gm` crate, under the manifest name
//! verbatim; the registry key is `gm::<dir_snake>::<file_snake>::<Name>`. A file name that
//! starts with a digit gets a `_` prefix (`gm/3d.cpp` → `gm::_3d`).
//!
//! `gm-verify` gives every registered GM a verdict: `passing` (every config matches on every
//! oracle tier with goldens, all checkable here), `failing` (any mismatch or failure), or
//! `not-checkable` (some tier could not run here, e.g. `Ml4` without AVX-512). A not-checkable
//! GM never becomes `passing`, but is not a regression of a `passing` entry either.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

use crate::verify::{Report, snake_case};

/// A GM's verdict from `gm-verify`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Passing,
    Failing,
    NotCheckable,
}

#[derive(Debug, Deserialize)]
struct GmVerifyReport {
    gms: Vec<GmEntry>,
}

#[derive(Debug, Deserialize)]
struct GmEntry {
    key: String,
    verdict: Verdict,
}

/// The registry key (`gm::dashing::Dashing5GM(true)`) for a manifest GM id
/// (`gm/dashing.cpp::Dashing5GM(true)`), or `None` if the id isn't a GM in `gm/`.
pub fn gm_key(id: &str) -> Option<String> {
    let (file, name) = id.split_once("::")?;
    let rest = file.strip_prefix("gm/")?.strip_suffix(".cpp")?;
    let mut parts = vec!["gm".to_owned()];
    for part in rest.split('/') {
        let snake = snake_case(part);
        parts.push(if snake.starts_with(|c: char| c.is_ascii_digit()) {
            format!("_{snake}")
        } else {
            snake
        });
    }
    parts.push(name.to_owned());
    Some(parts.join("::"))
}

/// Runs `gm-verify` over every registered GM and returns each one's verdict by key.
pub fn run_gm_verify(root: &Path) -> Result<BTreeMap<String, Verdict>> {
    let report_dir = root.join("target");
    std::fs::create_dir_all(&report_dir)?;
    let report_path = report_dir.join("gm-verify.json");
    let _ = std::fs::remove_file(&report_path);
    // The release profile: rendering every GM on every tier in a debug build takes hours, and GM
    // output is identical in both profiles (the `test-release` CI jobs check that).
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
        .current_dir(root)
        .args([
            "run",
            "-q",
            "--release",
            "-p",
            "skia-rust-gm",
            "--bin",
            "gm-verify",
            "--",
            "--no-diffs",
            "--report",
        ])
        .arg(&report_path)
        .status()
        .context("running gm-verify")?;
    ensure!(status.success(), "gm-verify failed with {status}");
    let text = std::fs::read_to_string(&report_path)
        .with_context(|| format!("reading {}", report_path.display()))?;
    let report: GmVerifyReport = serde_json::from_str(&text)
        .with_context(|| format!("parsing {}", report_path.display()))?;
    Ok(report.gms.into_iter().map(|g| (g.key, g.verdict)).collect())
}

/// Compares GM verdicts with manifest statuses. `entries` is (id, status) for every GM in the
/// manifest.
pub fn check(entries: &[(String, String)], results: &BTreeMap<String, Verdict>) -> Report {
    let mut report = Report::default();
    let mut by_key: BTreeMap<String, (&str, &str)> = BTreeMap::new();
    for (id, status) in entries {
        if let Some(key) = gm_key(id) {
            by_key.insert(key, (id, status));
        }
    }
    for (key, verdict) in results {
        let Some((id, status)) = by_key.get(key) else {
            report.unknown.push(key.clone());
            continue;
        };
        let id = (*id).to_owned();
        match (*status, verdict) {
            ("excluded", _) => report.excluded_but_ported.push(id),
            ("passing", Verdict::Passing) => {}
            ("passing", Verdict::Failing) => report.regressions.push(id),
            (_, Verdict::NotCheckable) => report.not_checkable.push(id),
            (other, Verdict::Passing) => report.newly_passing.push((id, other.to_owned())),
            (_, Verdict::Failing) => report.failing.push(id),
        }
    }
    for (key, (id, status)) in &by_key {
        if *status == "passing" && !results.contains_key(key) {
            report.regressions.push(format!("{id} (no GM {key})"));
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gm_keys() {
        assert_eq!(
            gm_key("gm/dashing.cpp::Dashing5GM(true)").as_deref(),
            Some("gm::dashing::Dashing5GM(true)")
        );
        assert_eq!(
            gm_key("gm/fiddle.cpp::fiddle").as_deref(),
            Some("gm::fiddle::fiddle")
        );
        assert_eq!(
            gm_key("gm/3d.cpp::sk3d_simple").as_deref(),
            Some("gm::_3d::sk3d_simple")
        );
        assert_eq!(
            gm_key("gm/asyncrescaleandread.cpp::images/dog.jpg#2").as_deref(),
            Some("gm::asyncrescaleandread::images/dog.jpg#2")
        );
        assert_eq!(gm_key("tests/PointTest.cpp::Point"), None);
    }

    #[test]
    fn check_classifies() {
        let entries = vec![
            ("gm/a.cpp::pass".to_owned(), "passing".to_owned()),
            ("gm/a.cpp::broke".to_owned(), "passing".to_owned()),
            ("gm/a.cpp::nc".to_owned(), "passing".to_owned()),
            ("gm/a.cpp::new".to_owned(), "todo".to_owned()),
            ("gm/a.cpp::bad".to_owned(), "todo".to_owned()),
            ("gm/a.cpp::gone".to_owned(), "passing".to_owned()),
            ("gm/a.cpp::ex".to_owned(), "excluded".to_owned()),
        ];
        let results = BTreeMap::from([
            ("gm::a::pass".to_owned(), Verdict::Passing),
            ("gm::a::broke".to_owned(), Verdict::Failing),
            ("gm::a::nc".to_owned(), Verdict::NotCheckable),
            ("gm::a::new".to_owned(), Verdict::Passing),
            ("gm::a::bad".to_owned(), Verdict::Failing),
            ("gm::a::ex".to_owned(), Verdict::Passing),
            ("gm::nope::x".to_owned(), Verdict::Passing),
        ]);
        let r = check(&entries, &results);
        assert_eq!(r.unknown, ["gm::nope::x"]);
        assert_eq!(r.excluded_but_ported, ["gm/a.cpp::ex"]);
        assert_eq!(
            r.regressions,
            ["gm/a.cpp::broke", "gm/a.cpp::gone (no GM gm::a::gone)"]
        );
        assert_eq!(r.not_checkable, ["gm/a.cpp::nc"]);
        assert_eq!(
            r.newly_passing,
            [("gm/a.cpp::new".to_owned(), "todo".to_owned())]
        );
        assert_eq!(r.failing, ["gm/a.cpp::bad"]);
    }
}
