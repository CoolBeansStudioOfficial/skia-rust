// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! End-to-end self-tests of the harness: fake GMs against a fixture hashes file, and one real
//! comparison against the oracle goldens.

use std::path::{Path, PathBuf};

use super::*;
use crate::canvas::{BlendMode, Canvas};
use crate::goldens::Objects;
use crate::prelude::{Color, ISize};
use crate::{DrawResult, GM};

/// A fake GM: clears to `bg` (red by default), then fills with opaque blue (8888 bytes
/// `ff 00 00 ff`, 565 `1f 00`, f16 `0 0 1 1`), or returns `result`.
struct Fake {
    name: &'static str,
    size: ISize,
    result: DrawResult,
}

impl GM for Fake {
    fn name(&self) -> String {
        self.name.to_owned()
    }

    fn size(&mut self) -> ISize {
        self.size
    }

    fn bg_color(&self) -> Color {
        Color::RED
    }

    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        // Inside GM::drawContent's SkAutoCanvasRestore.
        assert_eq!(canvas.save_count(), 2);
        if self.result == DrawResult::Ok {
            canvas.save();
            canvas.draw_color(Color::BLUE, None);
        } else {
            error_msg.push_str("fake says no");
        }
        self.result
    }
}

fn fake(name: &'static str, result: DrawResult) -> Box<dyn GM> {
    Box::new(Fake {
        name,
        size: ISize::new(4, 3),
        result,
    })
}

fn fixture_store(objects: Objects) -> GoldenStore {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("hashes-fixture.json");
    GoldenStore::from_release_json(&path, "fixture", objects).unwrap()
}

/// Every tier by a selection valid on every host, so the fixture results don't depend on it.
fn fixture_plan(store: &GoldenStore) -> Vec<TierPlan> {
    plan_with(store, |tier| {
        Ok(match tier {
            Tier::Scalar => Selection::native(tier),
            Tier::Neon => Selection::model(tier, Estimates::Arm),
            t => Selection::model(t, Estimates::AmdZen4),
        })
    })
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("skia-rust-gm-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn decisive(report: &GmReport) -> impl Iterator<Item = &Check> {
    report.checks.iter().filter(|c| !c.proxy)
}

#[test]
fn fixture_plan_maps_oracle_tiers() {
    let store = fixture_store(Objects::None);
    let plan = fixture_plan(&store);
    let get = |t: Tier| plan.iter().find(|p| p.tier == t).unwrap();
    assert_eq!(
        get(Tier::Sse2).oracle_tiers,
        ["cpu-x64-sse2", "cpu-x64-sse2-rt-ssse3"]
    );
    assert_eq!(get(Tier::Sse41).oracle_tiers, ["cpu-x64-sse41"]);
    assert_eq!(get(Tier::Ml3).oracle_tiers, ["cpu-x64-sse2-rt-ml3"]);
    assert_eq!(get(Tier::Ml4).oracle_tiers, ["cpu-x64-sse2-rt-ml4"]);
    assert_eq!(get(Tier::Scalar).oracle_tiers, ["cpu-x64-scalar"]);
    assert!(!get(Tier::Scalar).is_authoritative());
    assert_eq!(get(Tier::Neon).oracle_tiers, [] as [&str; 0]);
}

#[test]
fn fake_gm_matches_fixture_on_every_tier_and_config() {
    let store = fixture_store(Objects::None);
    let plan = fixture_plan(&store);
    let src = GmSrc::new(|| fake("fixture_blue", DrawResult::Ok));
    let report = check_gm(
        "gm::fixture::blue",
        &src,
        &store,
        &plan,
        &Options::default(),
    );
    assert_eq!(report.verdict, Verdict::Passing, "{report}");
    // 3 configs x (2 Sse2 + Sse41 + Ml3 + Ml4) decisive comparisons, all matches.
    assert_eq!(decisive(&report).count(), 15, "{report}");
    assert!(decisive(&report).all(|c| c.outcome == Outcome::Match));
    // The proxy disagrees on 8888 only: reported, not decisive.
    let proxy: Vec<_> = report.checks.iter().filter(|c| c.proxy).collect();
    assert_eq!(proxy.len(), 3);
    assert!(
        proxy
            .iter()
            .all(|c| c.oracle_tier == Some("cpu-x64-scalar"))
    );
    let mismatched: Vec<_> = proxy
        .iter()
        .filter(|c| c.outcome != Outcome::Match)
        .collect();
    assert_eq!(mismatched.len(), 1);
    assert_eq!(mismatched[0].config, "8888");
}

#[test]
fn mismatch_fails_and_writes_a_diff() {
    // The golden object behind the wrong 8888 hash on cpu-x64-sse41: all red.
    let objects = temp_dir("fixture-objects");
    let red: Vec<u8> = [0u8, 0, 0xff, 0xff].repeat(12);
    let sha = sha256_hex(&red);
    std::fs::create_dir_all(objects.join(&sha[..2])).unwrap();
    std::fs::write(
        objects.join(&sha[..2]).join(format!("{sha}.zst")),
        zstd::encode_all(red.as_slice(), 3).unwrap(),
    )
    .unwrap();
    let diffs = temp_dir("fixture-diffs");
    let store = fixture_store(Objects::Dir(objects.clone()));
    let plan = fixture_plan(&store);
    let src = GmSrc::new(|| fake("fixture_wrong", DrawResult::Ok));
    let opts = Options {
        configs: Config::ALL.to_vec(),
        diffs: Some(diffs.clone()),
    };
    let report = check_gm("gm::fixture::wrong", &src, &store, &plan, &opts);
    assert_eq!(report.verdict, Verdict::Failing, "{report}");
    let failures: Vec<_> = decisive(&report)
        .filter(|c| c.outcome != Outcome::Match)
        .collect();
    assert_eq!(failures.len(), 1, "{report}");
    let Outcome::Mismatch { golden, diff, .. } = &failures[0].outcome else {
        panic!("{report}");
    };
    assert_eq!(failures[0].tier, "sse41");
    assert_eq!(failures[0].oracle_tier, Some("cpu-x64-sse41"));
    assert_eq!(*golden, sha);
    let png = PathBuf::from(diff);
    assert!(png.is_file(), "{report}");
    assert_eq!(
        png,
        diffs
            .join("cpu-x64-sse41")
            .join("8888")
            .join("fixture_wrong.png")
    );
    std::fs::remove_dir_all(&objects).unwrap();
    std::fs::remove_dir_all(&diffs).unwrap();
}

#[test]
fn skips_match_only_when_the_oracle_skipped_too() {
    let store = fixture_store(Objects::None);
    let plan = fixture_plan(&store);

    let src = GmSrc::new(|| fake("fixture_skip", DrawResult::Skip));
    let report = check_gm(
        "gm::fixture::skip",
        &src,
        &store,
        &plan,
        &Options::default(),
    );
    assert_eq!(report.verdict, Verdict::Passing, "{report}");

    let src = GmSrc::new(|| fake("fixture_skip_golden", DrawResult::Skip));
    let report = check_gm("gm::fixture::sg", &src, &store, &plan, &Options::default());
    assert_eq!(report.verdict, Verdict::Failing, "{report}");
    let bad: Vec<_> = decisive(&report)
        .filter(|c| c.outcome != Outcome::Match)
        .collect();
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].oracle_tier, Some("cpu-x64-sse2"));
    assert!(
        matches!(bad[0].outcome, Outcome::SkippedButGolden { ref msg } if msg == "fake says no")
    );
}

#[test]
fn empty_sources_skip_like_dm() {
    let src = GmSrc::new(|| {
        Box::new(Fake {
            name: "empty",
            size: ISize::new(0, 5),
            result: DrawResult::Ok,
        })
    });
    let rendered = RasterSink::new(Config::N32).draw(&src);
    assert_eq!(rendered.result.status, Status::Skip);
    assert_eq!(rendered.result.msg, "Skipping empty source: empty");
    assert!(rendered.bitmap.is_none());
}

/// Draws a translucent color, which the canvas stub cannot do.
struct Translucent;

impl GM for Translucent {
    fn name(&self) -> String {
        "fixture_blue".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(4, 3)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.draw_color(Color::from_argb(0x80, 0, 0, 0xff), BlendMode::SrcOver);
    }
}

#[test]
fn failures_panics_and_missing_goldens_fail() {
    let store = fixture_store(Objects::None);
    let plan = fixture_plan(&store);

    let src = GmSrc::new(|| fake("fixture_blue", DrawResult::Fail));
    let report = check_gm("gm::fixture::f", &src, &store, &plan, &Options::default());
    assert_eq!(report.verdict, Verdict::Failing);
    assert!(
        decisive(&report)
            .all(|c| matches!(&c.outcome, Outcome::DrawFailed { msg } if msg == "fake says no"))
    );

    // Not in the fixture: the oracle has no such result.
    let src = GmSrc::new(|| fake("fixture_unknown", DrawResult::Ok));
    let report = check_gm("gm::fixture::u", &src, &store, &plan, &Options::default());
    assert_eq!(report.verdict, Verdict::Failing);
    assert!(decisive(&report).all(|c| matches!(c.outcome, Outcome::NoGolden { .. })));

    // A draw the stub cannot do panics; the harness reports it and keeps going.
    let src = GmSrc::new(|| Box::new(Translucent));
    let report = check_gm("gm::fixture::p", &src, &store, &plan, &Options::default());
    assert_eq!(report.verdict, Verdict::Failing);
    assert!(
        decisive(&report)
            .all(|c| matches!(&c.outcome, Outcome::Panicked { msg } if msg.contains("D6")))
    );
}

#[test]
fn not_checkable_tiers_never_pass() {
    let store = fixture_store(Objects::None);
    let plan = plan_with(&store, |tier| match tier {
        Tier::Ml4 => Err("no Ml4 here".to_owned()),
        Tier::Scalar => Ok(Selection::native(tier)),
        Tier::Neon => Ok(Selection::model(tier, Estimates::Arm)),
        t => Ok(Selection::model(t, Estimates::AmdZen4)),
    });
    let src = GmSrc::new(|| fake("fixture_blue", DrawResult::Ok));
    let report = check_gm("gm::fixture::nc", &src, &store, &plan, &Options::default());
    assert_eq!(report.verdict, Verdict::NotCheckable, "{report}");
    assert_eq!(
        decisive(&report)
            .filter(|c| matches!(c.outcome, Outcome::NotCheckable { .. }))
            .count(),
        3
    );
}

#[test]
fn host_policy_is_consistent() {
    // Whatever this host is, the policy only picks selections it can execute, never runs Ml4 by
    // a model, and runs Scalar natively.
    let fp = host_fingerprints();
    for tier in Tier::ALL {
        match selection_for(tier, fp) {
            Ok(sel) => {
                assert_eq!(sel.tier, tier);
                assert!(sel.check().is_ok());
                if tier == Tier::Ml4 {
                    assert_eq!(sel.backend, skia_rust_simd::Backend::Native);
                }
            }
            Err(reason) => assert_eq!(tier, Tier::Ml4, "{reason}"),
        }
    }
    assert_eq!(
        selection_for(Tier::Scalar, fp),
        Ok(Selection::native(Tier::Scalar))
    );
    // A host whose estimates match the oracle's runs its native x86 tiers natively.
    for tier in Tier::ALL
        .into_iter()
        .filter(|t| t.is_x86() && t.is_native())
    {
        assert_eq!(selection_for(tier, &AMD_ZEN4), Ok(Selection::native(tier)));
    }
}

/// The background-only stand-in for `gm/dashing.cpp::path_effect_empty_result`.
///
/// The real GM strokes a degenerate square with a dash path effect, which produces an empty path,
/// so DM's output is the white background alone: the goldens are solid white on every tier and
/// config. Drawing only the background checks `RasterSink` (color types, alpha types, zeroed
/// allocation), `GM::drawBackground` and the byte layout against real Skia. It is not the port
/// of that GM (it is not registered), which needs paths and dashing.
struct BackgroundOnly;

impl GM for BackgroundOnly {
    fn name(&self) -> String {
        "path_effect_empty_result".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(100, 100)
    }

    fn on_draw(&mut self, _canvas: &Canvas) {}
}

#[test]
fn background_matches_real_goldens() {
    let store = GoldenStore::shared().unwrap_or_else(|e| panic!("loading goldens: {e}"));
    let plan = plan(store);
    let src = GmSrc::new(|| Box::new(BackgroundOnly));
    let report = check_gm(
        "gm::dashing::path_effect_empty_result",
        &src,
        store,
        &plan,
        &Options::default(),
    );
    eprintln!("goldens: {}\n{report}", store.source);
    assert_ne!(report.verdict, Verdict::Failing, "{report}");
    // Sse2 is checkable on every host (natively or by its model), on all three configs.
    for config in Config::ALL {
        assert!(
            report.checks.iter().any(|c| c.config == config.tag()
                && c.oracle_tier == Some("cpu-x64-sse2")
                && c.outcome == Outcome::Match),
            "{report}"
        );
    }
}
