//! Checks the `dm-image` manifest entries (DM's `--src image`, one entry per file of
//! `resources/images`) against the `dm-verify` binary of `tests/gm`.
//!
//! `dm-verify --src image` renders every source DM makes for each file (`skia_rust_gm::dm_image`)
//! for each config (`8888`, `565`, `f16`) on every CPU tier the host supports, and compares the
//! hashes with `dm-image-hashes-m156.json` (the release asset pinned in `inventory/goldens.lock`,
//! or the file `$SKIA_RUST_DM_IMAGE_HASHES` names). An entry `dm/image/<file>` is `passing` when
//! every result for the file matches on every checkable tier.
//!
//! # Without the goldens
//! The asset is published by the maintainer after the CI-hosted oracle renders it, so CI may not
//! be able to fetch it yet. The leg then behaves as follows (the report has
//! `goldens_available: false`):
//! - no `dm-image` entry is `passing`: the leg is skipped (it neither fails nor marks anything);
//! - some entry is `passing`: every such entry is a regression, because a `passing` claim cannot
//!   be checked. Once entries are `passing`, the goldens are required, as the GM goldens are (a
//!   GM run that cannot load `hashes-m156.json` fails).
//!
//! An entry whose file has no result in the goldens at all (the oracle's DM failed on it) stays
//! as it is and is listed as not checkable.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

use crate::verify::Report;

/// A file's verdict from `dm-verify`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Passing,
    Failing,
    NotCheckable,
    NoGoldens,
}

#[derive(Debug, Deserialize)]
struct DmVerifyReport {
    goldens_available: bool,
    unavailable_reason: String,
    groups: Vec<Group>,
}

#[derive(Debug, Deserialize)]
struct Group {
    id: String,
    verdict: Verdict,
}

/// What `dm-verify` found.
#[derive(Debug)]
pub struct DmResults {
    /// `None` if the goldens are available, else why they are not.
    pub unavailable: Option<String>,
    /// Manifest id (`dm/image/<file>`) to verdict.
    pub verdicts: BTreeMap<String, Verdict>,
}

/// Runs `dm-verify --src <src>` and returns its verdicts by manifest id.
pub fn run_dm_verify(root: &Path, src: &str) -> Result<DmResults> {
    let report_dir = root.join("target");
    std::fs::create_dir_all(&report_dir)?;
    let report_path = report_dir.join(format!("dm-verify-{src}.json"));
    let _ = std::fs::remove_file(&report_path);
    // The release profile, as for the GMs: output is identical in both profiles.
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
        .current_dir(root)
        .args([
            "run",
            "-q",
            "--release",
            "-p",
            "skia-rust-gm",
            "--bin",
            "dm-verify",
            "--",
            "--src",
            src,
            "--report",
        ])
        .arg(&report_path)
        .status()
        .context("running dm-verify")?;
    ensure!(status.success(), "dm-verify failed with {status}");
    let text = std::fs::read_to_string(&report_path)
        .with_context(|| format!("reading {}", report_path.display()))?;
    let report: DmVerifyReport = serde_json::from_str(&text)
        .with_context(|| format!("parsing {}", report_path.display()))?;
    Ok(DmResults {
        unavailable: (!report.goldens_available).then_some(report.unavailable_reason),
        verdicts: report
            .groups
            .into_iter()
            .map(|g| (g.id, g.verdict))
            .collect(),
    })
}

/// Compares file verdicts with manifest statuses. `entries` is (id, status) for every entry of
/// the source in the manifest.
pub fn check(entries: &[(String, String)], results: &DmResults) -> Report {
    let mut report = Report::default();
    if let Some(reason) = &results.unavailable {
        let passing: Vec<&String> = entries
            .iter()
            .filter(|(_, status)| status == "passing")
            .map(|(id, _)| id)
            .collect();
        if passing.is_empty() {
            println!("SKIPPED the dm-image leg: no goldens ({reason}); no entry is passing");
        }
        for id in passing {
            report.regressions.push(format!(
                "{id} (passing, but the goldens are unavailable: {reason})"
            ));
        }
        return report;
    }
    for (id, status) in entries {
        let verdict = results.verdicts.get(id);
        match (status.as_str(), verdict) {
            ("excluded", Some(_)) => report.excluded_but_ported.push(id.clone()),
            ("passing", Some(Verdict::Passing)) => {}
            ("passing", Some(Verdict::NotCheckable)) => report.not_checkable.push(id.clone()),
            ("passing", Some(_)) => report.regressions.push(id.clone()),
            ("passing", None) => report.regressions.push(format!("{id} (no sources)")),
            (_, Some(Verdict::NotCheckable | Verdict::NoGoldens)) => {
                report.not_checkable.push(id.clone());
            }
            (other, Some(Verdict::Passing)) => {
                report.newly_passing.push((id.clone(), other.to_owned()));
            }
            (_, Some(Verdict::Failing)) => report.failing.push(id.clone()),
            (_, None) => {} // no sources: an excluded or todo entry stays as it is
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<(String, String)> {
        [
            ("dm/image/a.png", "passing"),
            ("dm/image/b.png", "passing"),
            ("dm/image/c.png", "todo"),
            ("dm/image/d.png", "todo"),
            ("dm/image/e.png", "todo"),
            ("dm/image/f.avif", "excluded"),
            ("dm/image/g.png", "passing"),
        ]
        .map(|(i, s)| (i.to_owned(), s.to_owned()))
        .to_vec()
    }

    #[test]
    fn classifies() {
        let results = DmResults {
            unavailable: None,
            verdicts: BTreeMap::from([
                ("dm/image/a.png".to_owned(), Verdict::Passing),
                ("dm/image/b.png".to_owned(), Verdict::Failing),
                ("dm/image/c.png".to_owned(), Verdict::Passing),
                ("dm/image/d.png".to_owned(), Verdict::Failing),
                ("dm/image/e.png".to_owned(), Verdict::NoGoldens),
            ]),
        };
        let r = check(&entries(), &results);
        assert_eq!(
            r.regressions,
            ["dm/image/b.png", "dm/image/g.png (no sources)"]
        );
        assert_eq!(
            r.newly_passing,
            [("dm/image/c.png".to_owned(), "todo".to_owned())]
        );
        assert_eq!(r.failing, ["dm/image/d.png"]);
        assert_eq!(r.not_checkable, ["dm/image/e.png"]);
    }

    #[test]
    fn missing_goldens_skip_unless_something_is_passing() {
        let results = DmResults {
            unavailable: Some("not published".to_owned()),
            verdicts: BTreeMap::new(),
        };
        let none_passing: Vec<_> = entries()
            .into_iter()
            .filter(|(_, s)| s != "passing")
            .collect();
        let r = check(&none_passing, &results);
        assert!(r.regressions.is_empty() && r.newly_passing.is_empty() && r.failing.is_empty());

        let r = check(&entries(), &results);
        assert_eq!(r.regressions.len(), 3);
        assert!(r.regressions[0].starts_with("dm/image/a.png (passing, but the goldens"));
    }
}
