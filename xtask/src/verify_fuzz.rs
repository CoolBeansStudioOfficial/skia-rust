//! Maps ported fuzzers back to manifest entries and checks the manifest's claims against their
//! corpus replay (design `docs/design/fuzz.md` §1).
//!
//! A fuzzer `fuzz/<File>.cpp::<Name>` is ported in the module `fuzz::<file_snake>` of the
//! `skia-rust-fuzz` crate, whose `def_fuzz!` generates the replay test
//! `fuzz::<file_snake>::<Name>` (a second registration with the same name, `<Name>#2`, is
//! `<Name>_2`). The test passes when the fuzzer runs its committed seeds and crash reproducers
//! without a panic or a hang; that is what `passing` means for a fuzz entry.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, ensure};

use crate::verify::{Outcome, Report, parse_test_output, snake_case};

/// The replay test path (`fuzz::fuzz_cubic_roots::CubicRoots`) for a manifest fuzz id
/// (`fuzz/FuzzCubicRoots.cpp::CubicRoots`), or `None` if the id isn't a fuzzer in `fuzz/`.
pub fn fuzz_key(id: &str) -> Option<String> {
    let (file, name) = id.split_once("::")?;
    let rest = file.strip_prefix("fuzz/")?.strip_suffix(".cpp")?;
    let mut parts = vec!["fuzz".to_owned()];
    parts.extend(rest.split('/').map(snake_case));
    parts.push(name.replace('#', "_"));
    Some(parts.join("::"))
}

/// Runs the fuzzers' replay tests and returns each one's outcome, keyed by test path.
pub fn run_fuzz_replay(root: &Path) -> Result<BTreeMap<String, Outcome>> {
    let out = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
        .current_dir(root)
        .args([
            "test",
            "-p",
            "skia-rust-fuzz",
            "--lib",
            "--no-fail-fast",
            "--",
            "--test-threads",
            "8",
        ])
        .output()
        .context("running the fuzz replay tests")?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let results = parse_test_output(&stdout);
    ensure!(
        !results.is_empty() || out.status.success(),
        "cargo test -p skia-rust-fuzz failed before running tests:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(results)
}

/// Compares replay outcomes with manifest statuses. `entries` is (id, status) for every fuzz
/// entry in the manifest; the crate's own tests (outside `fuzz::`) are ignored.
pub fn check(entries: &[(String, String)], results: &BTreeMap<String, Outcome>) -> Report {
    let mut report = Report::default();
    let mut by_key: BTreeMap<String, (&str, &str)> = BTreeMap::new();
    for (id, status) in entries {
        if let Some(key) = fuzz_key(id) {
            by_key.insert(key, (id, status));
        }
    }
    for (key, outcome) in results {
        if !key.starts_with("fuzz::") {
            continue;
        }
        let Some((id, status)) = by_key.get(key) else {
            report.unknown.push(key.clone());
            continue;
        };
        let id = (*id).to_owned();
        match (*status, outcome) {
            ("excluded", _) => report.excluded_but_ported.push(id),
            ("passing", Outcome::Ok) => {}
            ("passing", _) => report.regressions.push(id),
            (other, Outcome::Ok) => report.newly_passing.push((id, other.to_owned())),
            (_, Outcome::Failed) => report.failing.push(id),
            (_, Outcome::Ignored | Outcome::AdapterGated) => {}
        }
    }
    for (key, (id, status)) in &by_key {
        if *status == "passing" && !results.contains_key(key) {
            report
                .regressions
                .push(format!("{id} (no replay test {key})"));
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzz_keys() {
        assert_eq!(
            fuzz_key("fuzz/FuzzCubicRoots.cpp::CubicRoots").as_deref(),
            Some("fuzz::fuzz_cubic_roots::CubicRoots")
        );
        assert_eq!(
            fuzz_key("fuzz/FuzzPolyUtils.cpp::PolyUtils#2").as_deref(),
            Some("fuzz::fuzz_poly_utils::PolyUtils_2")
        );
        assert_eq!(
            fuzz_key("fuzz/FuzzEncoders.cpp::_MakeEncoderCorpus").as_deref(),
            Some("fuzz::fuzz_encoders::_MakeEncoderCorpus")
        );
        assert_eq!(
            fuzz_key("fuzz/FuzzPathop.cpp::LegacyChromiumPathop").as_deref(),
            Some("fuzz::fuzz_pathop::LegacyChromiumPathop")
        );
        assert_eq!(fuzz_key("tests/A.cpp::B"), None);
        assert_eq!(fuzz_key("fuzz/oss_fuzz/FuzzX.cpp"), None);
    }

    #[test]
    fn check_classifies() {
        let e = |file: &str, id: &str, s: &str| (format!("fuzz/{file}.cpp::{id}"), s.to_owned());
        let entries = vec![
            e("FuzzA", "Pass", "passing"),
            e("FuzzA", "Broke", "passing"),
            e("FuzzA", "Gone", "passing"),
            e("FuzzA", "New", "todo"),
            e("FuzzA", "Bad", "todo"),
            e("FuzzA", "Ignored", "failing"),
            e("FuzzA", "Ex", "excluded"),
        ];
        let results = BTreeMap::from([
            ("fuzz::fuzz_a::Pass".to_owned(), Outcome::Ok),
            ("fuzz::fuzz_a::Broke".to_owned(), Outcome::Failed),
            ("fuzz::fuzz_a::New".to_owned(), Outcome::Ok),
            ("fuzz::fuzz_a::Bad".to_owned(), Outcome::Failed),
            ("fuzz::fuzz_a::Ignored".to_owned(), Outcome::Ignored),
            ("fuzz::fuzz_a::Ex".to_owned(), Outcome::Ok),
            ("fuzz::nope::X".to_owned(), Outcome::Ok),
            ("reader::tests::short_reads".to_owned(), Outcome::Ok),
        ]);
        let r = check(&entries, &results);
        assert_eq!(r.unknown, ["fuzz::nope::X"]);
        assert_eq!(r.excluded_but_ported, ["fuzz/FuzzA.cpp::Ex"]);
        assert_eq!(
            r.regressions,
            [
                "fuzz/FuzzA.cpp::Broke",
                "fuzz/FuzzA.cpp::Gone (no replay test fuzz::fuzz_a::Gone)"
            ]
        );
        assert_eq!(
            r.newly_passing,
            [("fuzz/FuzzA.cpp::New".to_owned(), "todo".to_owned())]
        );
        assert_eq!(r.failing, ["fuzz/FuzzA.cpp::Bad"]);
        assert_eq!(r.not_checkable, Vec::<String>::new());
    }
}
