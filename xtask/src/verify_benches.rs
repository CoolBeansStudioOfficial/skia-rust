//! Maps ported benchmarks back to manifest entries and checks the manifest's claims against the
//! bench harness (`benches`, `bench-verify`; design `docs/design/bench.md` §3.3).
//!
//! A bench registration `bench/<File>.cpp::<Name>` is registered by `def_bench!` in the module
//! `bench::<file_snake>` of the `skia-rust-bench` crate under the manifest name verbatim; the
//! registry key is `bench::<file_snake>::<Name>`. A file name that starts with a digit gets a
//! `_` prefix, as for GMs.
//!
//! `bench-verify` gives every registration a verdict: `passing` (smoke run clean, nothing to
//! compare), `ported` (smoke run clean, but it renders and no bench goldens exist yet, so output
//! parity is unchecked), or `failing`. A `ported` verdict never makes an entry `passing`.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

use crate::verify::{Report, snake_case};

/// A registration's verdict from `bench-verify`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Passing,
    Ported,
    Failing,
    NotCheckable,
}

#[derive(Debug, Deserialize)]
struct BenchVerifyReport {
    benches: Vec<BenchEntry>,
}

#[derive(Debug, Deserialize)]
struct BenchEntry {
    key: String,
    verdict: Verdict,
}

/// The registry key (`bench::math_bench::Floor2IntBench(true)`) for a manifest bench id
/// (`bench/MathBench.cpp::Floor2IntBench(true)`), or `None` if the id isn't a bench in `bench/`.
pub fn bench_key(id: &str) -> Option<String> {
    let (file, name) = id.split_once("::")?;
    let rest = file.strip_prefix("bench/")?.strip_suffix(".cpp")?;
    let mut parts = vec!["bench".to_owned()];
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

/// Runs `bench-verify` over every registered benchmark and returns each one's verdict by key.
pub fn run_bench_verify(root: &Path) -> Result<BTreeMap<String, Verdict>> {
    let report_dir = root.join("target");
    std::fs::create_dir_all(&report_dir)?;
    let report_path = report_dir.join("bench-verify.json");
    let _ = std::fs::remove_file(&report_path);
    // The release profile, like gm-verify: it shares the release build of the raster crates.
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
        .current_dir(root)
        .args([
            "run",
            "-q",
            "--release",
            "-p",
            "skia-rust-bench",
            "--bin",
            "bench-verify",
            "--",
            "--report",
        ])
        .arg(&report_path)
        .status()
        .context("running bench-verify")?;
    ensure!(status.success(), "bench-verify failed with {status}");
    let text = std::fs::read_to_string(&report_path)
        .with_context(|| format!("reading {}", report_path.display()))?;
    let report: BenchVerifyReport = serde_json::from_str(&text)
        .with_context(|| format!("parsing {}", report_path.display()))?;
    Ok(report
        .benches
        .into_iter()
        .map(|b| (b.key, b.verdict))
        .collect())
}

/// Compares bench verdicts with manifest statuses. `entries` is (id, status) for every bench in
/// the manifest.
///
/// A `ported` verdict is only an upgrade from `todo`/`stale`/`failing`. For an entry marked
/// `passing` it is a regression: the claim included output parity, which cannot be shown.
pub fn check(entries: &[(String, String)], results: &BTreeMap<String, Verdict>) -> Report {
    let mut report = Report::default();
    let mut by_key: BTreeMap<String, (&str, &str)> = BTreeMap::new();
    for (id, status) in entries {
        if let Some(key) = bench_key(id) {
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
            (_, Verdict::NotCheckable) => report.not_checkable.push(id),
            ("passing", Verdict::Passing) | ("ported", Verdict::Ported) => {}
            ("passing", Verdict::Failing | Verdict::Ported) => report.regressions.push(id),
            (other, Verdict::Passing) => report.newly_passing.push((id, other.to_owned())),
            (other, Verdict::Ported) => report.newly_ported.push((id, other.to_owned())),
            (_, Verdict::Failing) => report.failing.push(id),
        }
    }
    for (key, (id, status)) in &by_key {
        if matches!(*status, "passing" | "ported") && !results.contains_key(key) {
            report.regressions.push(format!("{id} (no bench {key})"));
        }
    }
    report
}

/// The Rust name literal and registry key of a manifest bench id, for `inventory module-path`.
pub fn describe(id: &str) -> Option<(String, String)> {
    let key = bench_key(id)?;
    let (_, name) = id.split_once("::")?;
    Some((key, name.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bench_keys() {
        assert_eq!(
            bench_key("bench/MathBench.cpp::Floor2IntBench(true)").as_deref(),
            Some("bench::math_bench::Floor2IntBench(true)")
        );
        assert_eq!(
            bench_key("bench/graphite/FooBench.cpp::Foo").as_deref(),
            Some("bench::graphite::foo_bench::Foo")
        );
        assert_eq!(bench_key("gm/a.cpp::x"), None);
    }

    #[test]
    fn check_classifies() {
        let e = |id: &str, s: &str| (format!("bench/A.cpp::{id}"), s.to_owned());
        let entries = vec![
            e("pass", "passing"),
            e("broke", "passing"),
            e("lost_goldens", "passing"),
            e("new", "todo"),
            e("new_render", "todo"),
            e("render", "ported"),
            e("render_up", "ported"),
            e("bad", "todo"),
            e("still_bad", "failing"),
            e("gone", "passing"),
            e("ex", "excluded"),
        ];
        let results = BTreeMap::from([
            ("bench::a::pass".to_owned(), Verdict::Passing),
            ("bench::a::broke".to_owned(), Verdict::Failing),
            ("bench::a::lost_goldens".to_owned(), Verdict::Ported),
            ("bench::a::new".to_owned(), Verdict::Passing),
            ("bench::a::new_render".to_owned(), Verdict::Ported),
            ("bench::a::render".to_owned(), Verdict::Ported),
            ("bench::a::render_up".to_owned(), Verdict::Passing),
            ("bench::a::bad".to_owned(), Verdict::Failing),
            ("bench::a::still_bad".to_owned(), Verdict::Failing),
            ("bench::a::ex".to_owned(), Verdict::Passing),
            ("bench::nope::x".to_owned(), Verdict::Passing),
        ]);
        let r = check(&entries, &results);
        assert_eq!(r.unknown, ["bench::nope::x"]);
        assert_eq!(r.excluded_but_ported, ["bench/A.cpp::ex"]);
        assert_eq!(
            r.regressions,
            [
                "bench/A.cpp::broke",
                "bench/A.cpp::lost_goldens",
                "bench/A.cpp::gone (no bench bench::a::gone)"
            ]
        );
        assert_eq!(
            r.newly_passing,
            [
                ("bench/A.cpp::new".to_owned(), "todo".to_owned()),
                ("bench/A.cpp::render_up".to_owned(), "ported".to_owned())
            ]
        );
        assert_eq!(
            r.newly_ported,
            [("bench/A.cpp::new_render".to_owned(), "todo".to_owned())]
        );
        assert_eq!(r.failing, ["bench/A.cpp::bad", "bench/A.cpp::still_bad"]);
    }
}
