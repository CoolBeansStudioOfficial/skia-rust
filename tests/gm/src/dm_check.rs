// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Checks DM's file-driven sources (`--src image` today, `--src lottie` next) against their
//! goldens: one *group* per input file (the manifest entry `dm/<src>/<file>`), made of every
//! [`Src`] DM makes for that file, each rendered for every config on every CPU tier and compared
//! with the oracle tiers that have goldens ([`check_gm`]).
//!
//! A group is [`GroupVerdict::Passing`] when every result of every source matches on every
//! non-proxy oracle tier with goldens, all checkable here, **and** the goldens hold no result
//! for the file that no source made ([`unmade_goldens`]). It is [`GroupVerdict::NoGoldens`] when
//! the goldens have no result at all for the file (the oracle's DM failed on it).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::check::{Check, Options, Outcome, TierPlan, Verdict, check_gm};
use crate::goldens::GoldenStore;
use crate::sink::{Config, Src};

/// How many failure lines a [`GroupReport`] keeps.
const MAX_FAILURES: usize = 40;

/// A group's overall result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GroupVerdict {
    Passing,
    Failing,
    /// Some tier could not be checked on this host (and nothing failed).
    NotCheckable,
    /// The goldens have no result for this file.
    NoGoldens,
}

/// Everything checked for one input file.
#[derive(Clone, Debug, Serialize)]
pub struct GroupReport {
    /// The manifest id: `dm/image/<file>`.
    pub id: String,
    pub verdict: GroupVerdict,
    /// Sources DM makes for the file.
    pub srcs: usize,
    /// Comparisons that matched (a skip the oracle also made counts).
    pub matches: usize,
    /// Comparisons that could not run here.
    pub not_checkable: usize,
    /// The first failing comparisons, one line each (`MAX_FAILURES` at most).
    pub failures: Vec<String>,
    /// How many comparisons failed.
    pub failure_count: usize,
}

fn describe(check: &Check, config_dir: &str, name: &str) -> String {
    let against = check.oracle_tier.unwrap_or("-");
    let how = check.selection.as_deref().unwrap_or("not run");
    let what = match &check.outcome {
        Outcome::Match => "match".to_owned(),
        Outcome::Mismatch { ours, golden, .. } => format!("MISMATCH ours {ours} golden {golden}"),
        Outcome::SkippedButGolden { msg } => format!("SKIPPED ({msg}) but the oracle has a result"),
        Outcome::NoGolden { ours } => format!("NO GOLDEN (ours {ours})"),
        Outcome::DrawFailed { msg } => format!("DRAW FAILED: {msg}"),
        Outcome::Panicked { msg } => format!("PANICKED: {msg}"),
        Outcome::NotCheckable { reason } => format!("not checkable: {reason}"),
    };
    format!(
        "{}/{config_dir}/{name}: tier {} [{how}] vs {against}: {what}",
        check.config, check.tier
    )
}

/// What checking one source found; [`merge_group`] combines the reports of a file's sources.
#[derive(Clone, Debug)]
pub struct SrcReport {
    any_golden: bool,
    not_checkable_verdict: bool,
    only_no_golden: bool,
    matches: usize,
    not_checkable: usize,
    failure_count: usize,
    failures: Vec<String>,
}

/// Checks one source of a file: every config on every tier of `plan`.
///
/// `id` is the manifest id (`dm/image/<file>`). `store` holds the goldens of the source, `plan`
/// says how each tier runs here ([`crate::check::plan`]).
#[must_use]
pub fn check_src(
    id: &str,
    src: &dyn Src,
    store: &GoldenStore,
    plan: &[TierPlan],
    opts: &Options,
) -> SrcReport {
    let dir = src.id_dir();
    let name = src.name();
    let mut report = SrcReport {
        any_golden: false,
        not_checkable_verdict: false,
        only_no_golden: true,
        matches: 0,
        not_checkable: 0,
        failure_count: 0,
        failures: Vec::new(),
    };
    for &config in &opts.configs {
        let rid = config.src_result_id(&dir, &name);
        report.any_golden |= store.tiers().any(|t| store.golden(t, &rid).is_some());
    }
    let r = check_gm(id, src, store, plan, opts);
    report.not_checkable_verdict = r.verdict == Verdict::NotCheckable;
    for check in &r.checks {
        match &check.outcome {
            Outcome::Match => report.matches += 1,
            Outcome::NotCheckable { .. } => report.not_checkable += 1,
            outcome => {
                if check.proxy {
                    continue;
                }
                report.failure_count += 1;
                report.only_no_golden &= matches!(outcome, Outcome::NoGolden { .. });
                if report.failures.len() < MAX_FAILURES {
                    report.failures.push(describe(check, &dir, &name));
                }
            }
        }
    }
    report
}

/// The group report of the file `id` from the reports of its `srcs` sources.
#[must_use]
pub fn merge_group(
    id: &str,
    srcs: usize,
    reports: impl IntoIterator<Item = SrcReport>,
) -> GroupReport {
    let mut report = GroupReport {
        id: id.to_owned(),
        verdict: GroupVerdict::Passing,
        srcs,
        matches: 0,
        not_checkable: 0,
        failures: Vec::new(),
        failure_count: 0,
    };
    let mut any_golden = false;
    let mut only_no_golden = true;
    let mut not_checkable_verdict = false;
    for r in reports {
        any_golden |= r.any_golden;
        not_checkable_verdict |= r.not_checkable_verdict;
        if r.failure_count > 0 {
            only_no_golden &= r.only_no_golden;
        }
        report.matches += r.matches;
        report.not_checkable += r.not_checkable;
        report.failure_count += r.failure_count;
        let room = MAX_FAILURES.saturating_sub(report.failures.len());
        report.failures.extend(r.failures.into_iter().take(room));
    }
    report.verdict = if !any_golden && (report.failure_count == 0 || only_no_golden) {
        // The goldens have no result for the file: the oracle's DM failed on it (or skipped it).
        GroupVerdict::NoGoldens
    } else if report.failure_count > 0 {
        GroupVerdict::Failing
    } else if not_checkable_verdict {
        GroupVerdict::NotCheckable
    } else {
        GroupVerdict::Passing
    };
    report
}

/// Checks the sources of one file (sequentially; see [`check_src`] for a unit of parallel work).
#[must_use]
pub fn check_group(
    id: &str,
    srcs: &[Box<dyn Src>],
    store: &GoldenStore,
    plan: &[TierPlan],
    opts: &Options,
) -> GroupReport {
    merge_group(
        id,
        srcs.len(),
        srcs.iter()
            .map(|src| check_src(id, src.as_ref(), store, plan, opts)),
    )
}

/// The ids of every result the sources of all `groups` make, for every config.
#[must_use]
pub fn made_ids(groups: &[(String, Vec<Box<dyn Src>>)], configs: &[Config]) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for (_, srcs) in groups {
        for src in srcs {
            let (dir, name) = (src.id_dir(), src.name());
            for &config in configs {
                ids.insert(config.src_result_id(&dir, &name));
            }
        }
    }
    ids
}

/// The golden results of source `src_name` (`image`) that no source made, by file: results the
/// goldens have for a file but our enumeration of DM's sources lacks (`made` from
/// [`made_ids`]). `files` are the base names of the input files; a result belongs to the file
/// whose name is the longest prefix of the result's name (`<file>`, `<file>_<scale>`, ...).
#[must_use]
pub fn unmade_goldens(
    store: &GoldenStore,
    src_name: &str,
    made: &BTreeSet<String>,
    files: &[String],
) -> BTreeMap<String, Vec<String>> {
    let mut unmade: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for tier in store.tiers() {
        for rid in store.ids(tier) {
            let mut parts = rid.splitn(3, '/');
            let (_config, src, rest) = (parts.next(), parts.next(), parts.next());
            if src != Some(src_name) || made.contains(rid) {
                continue;
            }
            // `rest` is `<options>/<name>`; the name has no `/`.
            let name = rest.and_then(|r| r.rsplit_once('/')).map_or("", |(_, n)| n);
            let owner = files
                .iter()
                .filter(|f| {
                    name == f.as_str()
                        || name
                            .strip_prefix(f.as_str())
                            .is_some_and(|tail| tail.starts_with('_'))
                })
                .max_by_key(|f| f.len());
            unmade
                .entry(owner.cloned().unwrap_or_default())
                .or_default()
                .insert(rid.to_owned());
        }
    }
    unmade
        .into_iter()
        .map(|(file, ids)| (file, ids.into_iter().collect()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::Canvas;
    use crate::check::plan_with;
    use crate::goldens::{Hashes, Objects, sha256_hex};
    use crate::prelude::{Color, ISize};
    use crate::sink::{DmResult, Status};
    use skia_rust_simd::{Estimates, Selection, Tier};

    /// A source that fills a 4x3 canvas with blue (or fails after doing nothing).
    struct FakeSrc {
        name: &'static str,
        status: Status,
        fatal_pixels: bool,
    }

    impl Src for FakeSrc {
        fn draw(&self, canvas: &Canvas) -> DmResult {
            if self.status != Status::Fatal {
                canvas.draw_color(Color::BLUE, None);
            }
            DmResult::new(self.status, String::new())
        }

        fn size(&self) -> ISize {
            ISize::new(4, 3)
        }

        fn name(&self) -> String {
            self.name.to_owned()
        }

        fn id_dir(&self) -> String {
            "image/fixture".to_owned()
        }

        fn fatal_pixels_are_results(&self) -> bool {
            self.fatal_pixels
        }
    }

    fn opts() -> Options {
        Options {
            configs: vec![Config::Rgb565],
            diffs: None,
            ..Options::default()
        }
    }

    /// The 565 bytes of a 4x3 canvas: blue, or never drawn.
    fn bytes(blue: bool) -> Vec<u8> {
        if blue {
            [0x1f, 0x00].repeat(12)
        } else {
            vec![0; 24]
        }
    }

    fn store(ids: &[(&str, bool)]) -> GoldenStore {
        let results: Hashes = ids
            .iter()
            .map(|&(id, blue)| (id.to_owned(), sha256_hex(&bytes(blue))))
            .collect();
        let tiers = [
            "cpu-x64-sse2",
            "cpu-x64-sse41",
            "cpu-x64-sse2-rt-ml3",
            "cpu-x64-sse2-rt-ml4",
        ]
        .map(|t| (t.to_owned(), results.clone()))
        .into_iter()
        .collect();
        GoldenStore::new("test".to_owned(), tiers, Objects::None)
    }

    fn plan(store: &GoldenStore) -> Vec<TierPlan> {
        plan_with(store, |tier| {
            Ok(match tier {
                Tier::Scalar => Selection::native(tier),
                Tier::Neon => Selection::model(tier, Estimates::Arm),
                t => Selection::model(t, Estimates::AmdZen4),
            })
        })
    }

    fn group(srcs: &[Box<dyn Src>], store: &GoldenStore) -> GroupReport {
        check_group("dm/image/blue.png", srcs, store, &plan(store), &opts())
    }

    fn src(status: Status, fatal_pixels: bool) -> Vec<Box<dyn Src>> {
        vec![Box::new(FakeSrc {
            name: "blue.png",
            status,
            fatal_pixels,
        })]
    }

    #[test]
    fn matching_group_passes() {
        let store = store(&[("565/image/fixture/blue.png", true)]);
        let report = group(&src(Status::Ok, false), &store);
        assert_eq!(report.verdict, GroupVerdict::Passing, "{report:?}");
        assert!(report.matches >= 4);
        assert_eq!(report.failure_count, 0);
    }

    #[test]
    fn differing_group_fails() {
        let store = store(&[("565/image/fixture/blue.png", false)]);
        let report = group(&src(Status::Ok, false), &store);
        assert_eq!(report.verdict, GroupVerdict::Failing);
        assert!(report.failures[0].contains("MISMATCH"));
    }

    /// DM writes the bitmap of a failed task: the goldens of such a source are its blank bitmap.
    #[test]
    fn fatal_pixels_are_compared_when_the_source_says_so() {
        let store = store(&[("565/image/fixture/blue.png", false)]);
        let report = group(&src(Status::Fatal, true), &store);
        assert_eq!(report.verdict, GroupVerdict::Passing, "{report:?}");
        let report = group(&src(Status::Fatal, false), &store);
        assert_eq!(report.verdict, GroupVerdict::Failing);
        assert!(report.failures[0].contains("DRAW FAILED"));
    }

    #[test]
    fn a_skip_the_oracle_also_made_is_a_match() {
        let store = store(&[("565/image/fixture/other.png", true)]);
        let report = group(&src(Status::Skip, false), &store);
        assert_eq!(report.failure_count, 0);
        // Nothing in the goldens for this file at all.
        assert_eq!(report.verdict, GroupVerdict::NoGoldens);
    }

    #[test]
    fn no_goldens_for_the_file() {
        let store = store(&[("565/image/fixture/other.png", true)]);
        let report = group(&src(Status::Ok, false), &store);
        assert_eq!(report.verdict, GroupVerdict::NoGoldens);
    }

    #[test]
    fn goldens_nobody_makes_are_attributed_to_the_longest_file_name() {
        let store = store(&[
            ("565/image/codec_premul/a.png", true),
            ("565/image/scaled_codec_premul_0.500/a.png_0.500", true),
            ("565/image/codec_premul/a.png_x/b.png", true),
            ("565/image/codec_premul/ab.png", true),
            ("565/gm/not-an-image", true),
        ]);
        let made: BTreeSet<String> = ["565/image/codec_premul/a.png".to_owned()].into();
        let files = vec![
            "a.png".to_owned(),
            "a.png_x".to_owned(),
            "ab.png".to_owned(),
        ];
        let unmade = unmade_goldens(&store, "image", &made, &files);
        assert_eq!(
            unmade["a.png"],
            ["565/image/scaled_codec_premul_0.500/a.png_0.500"]
        );
        assert_eq!(unmade["ab.png"], ["565/image/codec_premul/ab.png"]);
        // `b.png` belongs to no file.
        assert_eq!(unmade[""], ["565/image/codec_premul/a.png_x/b.png"]);
    }
}
