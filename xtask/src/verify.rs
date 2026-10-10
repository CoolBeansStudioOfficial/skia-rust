//! Maps ported Rust tests back to manifest entries and checks the manifest's
//! `passing` claims against an actual `cargo test` run.
//!
//! A Skia unit test `<dir>/<File>.cpp::<Name>` is ported as the Rust test
//! `<prefix>::<file_snake>::<Name>` in the `skia-rust-tests` crate:
//!
//! | Skia path | Rust module prefix |
//! |---|---|
//! | `tests/…` | `unit` |
//! | `modules/<m>/tests/…` | `modules::<m>` |
//!
//! GMs (`gm/…`) live in the `skia-rust-gm` crate and are checked by [`crate::verify_gms`].

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};

/// `PointTest` -> `point_test`, `RRectInPathTest` -> `r_rect_in_path_test`,
/// `M44Test` -> `m44_test`, `SkVxTest` -> `sk_vx_test`.
pub fn snake_case(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).is_some_and(char::is_ascii_lowercase);
            if prev.is_ascii_lowercase()
                || (prev.is_ascii_digit() && !out.ends_with('_'))
                || (prev.is_ascii_uppercase() && next_lower)
            {
                out.push('_');
            }
        }
        if c == '-' || c == '.' {
            out.push('_');
        } else {
            out.push(c.to_ascii_lowercase());
        }
    }
    out
}

/// The Rust test path (`unit::point_test::Point`) for a manifest unit-test id
/// (`tests/PointTest.cpp::Point`), or `None` if the id isn't a mappable unit test.
pub fn module_path(id: &str) -> Option<String> {
    let (file, name) = id.split_once("::")?;
    let name = name.replace('#', "_");
    let file = file.strip_suffix(".cpp")?;
    let mut parts: Vec<String> = Vec::new();
    let rest = if let Some(rest) = file.strip_prefix("tests/") {
        parts.push("unit".to_owned());
        rest
    } else if let Some(rest) = file.strip_prefix("gm/") {
        parts.push("gm".to_owned());
        rest
    } else {
        let (module, rest) = file.strip_prefix("modules/")?.split_once('/')?;
        parts.push("modules".to_owned());
        parts.push(snake_case(module));
        rest.strip_prefix("tests/")?
    };
    parts.extend(rest.split('/').map(snake_case));
    parts.push(name);
    Some(parts.join("::"))
}

/// The `ignore` reason of the adapter-gated tests (`def_graphite_adapter_test!` in
/// `tests/src/lib.rs`, and the GPU crate's pixel tests). They are ignored only without the
/// `skia_rust_adapter_tests` cfg, so this must match that `cfg_attr`'s reason exactly.
pub const ADAPTER_IGNORE_REASON: &str = "needs a real adapter in CI (lavapipe job)";

/// The cfg that runs the adapter-gated tests (declared in the workspace `Cargo.toml`).
pub const ADAPTER_CFG: &str = "skia_rust_adapter_tests";

/// Outcome of one Rust test in a `cargo test` run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    Failed,
    Ignored,
    /// Ignored because the adapter cfg is off (`ADAPTER_IGNORE_REASON`): the test did not run
    /// here, but it runs on the GPU CI jobs, which set the cfg.
    AdapterGated,
}

/// Parses libtest's plain output: `test unit::point_test::Point ... ok`.
pub fn parse_test_output(stdout: &str) -> BTreeMap<String, Outcome> {
    let mut out = BTreeMap::new();
    for line in stdout.lines() {
        let Some(rest) = line.strip_prefix("test ") else {
            continue;
        };
        let Some((name, result)) = rest.rsplit_once(" ... ") else {
            continue;
        };
        let outcome = match result.trim() {
            "ok" => Outcome::Ok,
            "FAILED" => Outcome::Failed,
            r if r.starts_with("ignored") && r.contains(ADAPTER_IGNORE_REASON) => {
                Outcome::AdapterGated
            }
            r if r.starts_with("ignored") => Outcome::Ignored,
            _ => continue,
        };
        out.insert(name.trim().to_owned(), outcome);
    }
    out
}

/// Whether `RUSTFLAGS` (as `cargo` will see it) sets [`ADAPTER_CFG`], so the adapter-gated tests
/// run in the `cargo test` that `run_ported_tests` starts.
pub fn adapter_cfg_enabled() -> bool {
    std::env::var("RUSTFLAGS").is_ok_and(|flags| {
        flags
            .split_whitespace()
            .any(|flag| flag == ADAPTER_CFG || flag == format!("--cfg={ADAPTER_CFG}"))
    })
}

/// Runs the ported tests and returns each one's outcome, keyed by Rust test path.
pub fn run_ported_tests(root: &Path) -> Result<BTreeMap<String, Outcome>> {
    let out = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
        .current_dir(root)
        .args([
            "test",
            "-p",
            "skia-rust-tests",
            "--lib",
            "--no-fail-fast",
            "--",
            "--test-threads",
            "8",
        ])
        .output()
        .context("running cargo test")?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let results = parse_test_output(&stdout);
    ensure!(
        !results.is_empty() || out.status.success(),
        "cargo test failed before running tests:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(results)
}

/// What `verify` found.
#[derive(Debug, Default)]
pub struct Report {
    /// Rust tests that map to no manifest unit test.
    pub unknown: Vec<String>,
    /// Entries marked `passing` whose test is missing, failed or ignored.
    pub regressions: Vec<String>,
    /// Entries whose test passes but which aren't marked `passing` (id, old status).
    pub newly_passing: Vec<(String, String)>,
    /// Benchmarks whose smoke run passes but which render and have no output check, so they are
    /// `ported`, not `passing` (id, old status).
    pub newly_ported: Vec<(String, String)>,
    /// Entries marked excluded whose test exists anyway.
    pub excluded_but_ported: Vec<String>,
    /// Entries whose test exists but fails (id), for `--update`.
    pub failing: Vec<String>,
    /// Entries that could not be checked on this host (never marked passing, never a regression):
    /// GMs not checkable on every tier, and adapter-gated tests run without the adapter cfg.
    pub not_checkable: Vec<String>,
}

/// Compares test outcomes with manifest statuses. `entries` is (id, status) for every
/// unit test in the manifest; harness self-tests (`tests::…`) are ignored.
///
/// `adapter_cfg` says whether the adapter-gated tests ran (see [`adapter_cfg_enabled`]). Without
/// it, a `passing` entry whose test is adapter-gated is not a regression: it is listed in
/// `not_checkable`. With it, that ignore is a regression, since the test should have run.
pub fn check(
    entries: &[(String, String)],
    results: &BTreeMap<String, Outcome>,
    adapter_cfg: bool,
) -> Report {
    let mut report = Report::default();
    let mut by_path: BTreeMap<String, (&str, &str)> = BTreeMap::new();
    for (id, status) in entries {
        if let Some(path) = module_path(id) {
            by_path.insert(path, (id, status));
        }
    }
    for (path, outcome) in results {
        // Only `unit::` and `modules::` hold 1:1 Skia ports; anything else in the crate
        // (`tests::`, the self-tests of the ported tools under `tools::`) has no manifest entry.
        if !(path.starts_with("unit::") || path.starts_with("modules::")) {
            continue;
        }
        let Some((id, status)) = by_path.get(path) else {
            report.unknown.push(path.clone());
            continue;
        };
        match (*status, outcome) {
            ("excluded", _) => report.excluded_but_ported.push((*id).to_owned()),
            ("passing", Outcome::Ok) => {}
            ("passing", Outcome::AdapterGated) if !adapter_cfg => {
                report.not_checkable.push((*id).to_owned());
            }
            ("passing", _) => report.regressions.push((*id).to_owned()),
            (other, Outcome::Ok) => report
                .newly_passing
                .push(((*id).to_owned(), other.to_owned())),
            (_, Outcome::Failed) => report.failing.push((*id).to_owned()),
            (_, Outcome::Ignored | Outcome::AdapterGated) => {}
        }
    }
    for (path, (id, status)) in &by_path {
        if *status == "passing" && !results.contains_key(path) {
            report.regressions.push(format!("{id} (no test {path})"));
        }
    }
    report
}

/// Prints `report` (about `what`: `"unit test"` or `"GM"`) and fails unless it's clean. With
/// `update`, the caller has already written the new statuses, so only hard errors fail.
pub fn finish(report: &Report, update: bool, what: &str) -> Result<()> {
    for p in &report.unknown {
        println!("UNKNOWN {p} (no manifest {what} maps to it)");
    }
    for id in &report.not_checkable {
        println!("NOT CHECKABLE on this host (status unchanged) {id}");
    }
    for id in &report.excluded_but_ported {
        println!("EXCLUDED BUT PORTED {id}");
    }
    for id in &report.regressions {
        println!("REGRESSION {id}");
    }
    for (id, old) in &report.newly_passing {
        let verb = if update {
            "now passing"
        } else {
            "passes but is marked"
        };
        println!("{verb} {id} ({old})");
    }
    for (id, old) in &report.newly_ported {
        let verb = if update {
            "now ported"
        } else {
            "runs clean but is marked"
        };
        println!("{verb} {id} ({old})");
    }
    let hard = report.unknown.len() + report.excluded_but_ported.len() + report.regressions.len();
    if hard > 0 {
        bail!("{hard} problem(s) between ported {what}s and the manifest");
    }
    if !update && !report.newly_ported.is_empty() {
        bail!(
            "{} ported {what}(s) not marked ported; run `cargo xtask inventory verify --update`",
            report.newly_ported.len()
        );
    }
    if !update && !report.newly_passing.is_empty() {
        bail!(
            "{} passing {what}(s) not marked passing; run `cargo xtask inventory verify --update`",
            report.newly_passing.len()
        );
    }
    println!("ported {what}s and manifest agree");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_matches_skia_file_names() {
        assert_eq!(snake_case("PointTest"), "point_test");
        assert_eq!(snake_case("RRectInPathTest"), "r_rect_in_path_test");
        assert_eq!(snake_case("M44Test"), "m44_test");
        assert_eq!(snake_case("SkVxTest"), "sk_vx_test");
        assert_eq!(snake_case("GrQuadBufferTest"), "gr_quad_buffer_test");
        assert_eq!(snake_case("PathOpsDCubicTest"), "path_ops_d_cubic_test");
        assert_eq!(snake_case("Float16Test"), "float16_test");
        assert_eq!(snake_case("SkUTFTest"), "sk_utf_test");
        assert_eq!(snake_case("HSVRoundTripTest"), "hsv_round_trip_test");
    }

    #[test]
    fn module_paths() {
        assert_eq!(
            module_path("tests/PointTest.cpp::Point").as_deref(),
            Some("unit::point_test::Point")
        );
        assert_eq!(
            module_path("tests/graphite/RectTest.cpp::GraphiteRect").as_deref(),
            Some("unit::graphite::rect_test::GraphiteRect")
        );
        assert_eq!(
            module_path("modules/svg/tests/Text.cpp::Svg_Text").as_deref(),
            Some("modules::svg::text::Svg_Text")
        );
        assert_eq!(
            module_path("tests/GrMeshTest.cpp::GrMeshTest#2").as_deref(),
            Some("unit::gr_mesh_test::GrMeshTest_2")
        );
        assert_eq!(module_path("gm/aarectmodes.cpp"), None);
    }

    #[test]
    fn parses_libtest_output() {
        let out = "running 3 tests\n\
                   test unit::point_test::Point ... ok\n\
                   test unit::rect_test::Rect ... FAILED\n\
                   test unit::x::Y ... ignored, needs gpu\n\
                   test unit::graphite::A ... ignored, needs a real adapter in CI (lavapipe job)\n\
                   test result: FAILED.";
        let r = parse_test_output(out);
        assert_eq!(r["unit::point_test::Point"], Outcome::Ok);
        assert_eq!(r["unit::rect_test::Rect"], Outcome::Failed);
        assert_eq!(r["unit::x::Y"], Outcome::Ignored);
        assert_eq!(r["unit::graphite::A"], Outcome::AdapterGated);
    }

    #[test]
    fn check_classifies() {
        let entries = vec![
            (
                "tests/PointTest.cpp::Point".to_owned(),
                "passing".to_owned(),
            ),
            ("tests/RectTest.cpp::Rect".to_owned(), "todo".to_owned()),
            ("tests/RectTest.cpp::Gone".to_owned(), "passing".to_owned()),
            (
                "tests/ArenaAllocTest.cpp::A".to_owned(),
                "excluded".to_owned(),
            ),
        ];
        let results = BTreeMap::from([
            ("unit::point_test::Point".to_owned(), Outcome::Failed),
            ("unit::rect_test::Rect".to_owned(), Outcome::Ok),
            ("unit::arena_alloc_test::A".to_owned(), Outcome::Ok),
            ("unit::nope::X".to_owned(), Outcome::Ok),
            ("tests::harness".to_owned(), Outcome::Ok),
        ]);
        let r = check(&entries, &results, false);
        assert_eq!(r.unknown, ["unit::nope::X"]);
        assert_eq!(r.excluded_but_ported, ["tests/ArenaAllocTest.cpp::A"]);
        assert_eq!(r.newly_passing.len(), 1);
        assert_eq!(r.regressions.len(), 2);
    }

    #[test]
    fn adapter_gated_ignore_is_a_regression_only_with_the_cfg() {
        let entries = vec![
            (
                "tests/graphite/ComputeTest.cpp::Compute".to_owned(),
                "passing".to_owned(),
            ),
            (
                "tests/graphite/ComputeTest.cpp::Other".to_owned(),
                "todo".to_owned(),
            ),
        ];
        let results = BTreeMap::from([
            (
                "unit::graphite::compute_test::Compute".to_owned(),
                Outcome::AdapterGated,
            ),
            (
                "unit::graphite::compute_test::Other".to_owned(),
                Outcome::AdapterGated,
            ),
        ]);
        // Without the cfg: not run, not a regression, and the todo entry stays todo.
        let off = check(&entries, &results, false);
        assert_eq!(off.regressions, Vec::<String>::new());
        assert_eq!(off.newly_passing, Vec::<(String, String)>::new());
        assert_eq!(
            off.not_checkable,
            ["tests/graphite/ComputeTest.cpp::Compute"]
        );
        // With the cfg, the same ignore means the test did not run: a regression.
        let on = check(&entries, &results, true);
        assert_eq!(on.regressions, ["tests/graphite/ComputeTest.cpp::Compute"]);
        assert_eq!(on.not_checkable, Vec::<String>::new());
    }
}
