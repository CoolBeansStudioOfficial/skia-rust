// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Checks DM's file-driven sources against the CI-hosted oracle's goldens and writes a JSON
//! report; run by `cargo xtask inventory verify` (leg `dm-image`).
//!
//! ```text
//! dm-verify --src image [--images <dir>] --report <file.json> [--match <substring>]...
//!           [--tier <scalar|sse2|sse41|ml3|ml4|neon>]...
//! ```
//!
//! `--src image` is DM's `--src image --images resources/images`: [`skia_rust_gm::dm_image`].
//! Its goldens are `dm-image-hashes-m156.json` ([`skia_rust_gm::goldens::DM_IMAGE`]): the lock's
//! release asset, or `$SKIA_RUST_DM_IMAGE_HASHES`. Without either, the report says so
//! (`goldens_available: false`) and the run succeeds having checked nothing; `xtask` decides
//! what that means for the manifest. `--tier` (debugging) restricts the CPU tiers checked; the
//! verdicts of such a run are not for the manifest.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::Serialize;
use skia_rust_gm::check::{self, Options, TierPlan};
use skia_rust_gm::dm_check::{self, GroupReport, GroupVerdict, SrcReport};
use skia_rust_gm::dm_image;
use skia_rust_gm::goldens::{DM_IMAGE, DmGoldens, GoldenStore};
use skia_rust_gm::sink::{Config, Src};
use skia_rust_gm::tool_utils::resource_dir;

#[derive(Serialize)]
struct PlanEntry {
    tier: &'static str,
    run: String,
    oracle_tiers: Vec<&'static str>,
    rgba_oracle_tiers: Vec<&'static str>,
}

#[derive(Serialize)]
struct Report {
    src: String,
    /// Where the goldens came from, or empty.
    goldens: String,
    goldens_available: bool,
    /// Why there are no goldens.
    unavailable_reason: String,
    plan: Vec<PlanEntry>,
    groups: Vec<GroupReport>,
}

struct Args {
    report: Option<PathBuf>,
    src: String,
    images: Option<PathBuf>,
    matches: Vec<String>,
    tiers: Vec<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        report: None,
        src: String::from("image"),
        images: None,
        matches: Vec::new(),
        tiers: Vec::new(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--report" => args.report = it.next().map(PathBuf::from),
            "--src" => args.src = it.next().unwrap_or_default(),
            "--images" => args.images = it.next().map(PathBuf::from),
            "--match" => args.matches.extend(it.next()),
            "--tier" => args.tiers.extend(it.next()),
            // The golden objects of this source are not published (hashes only): no diffs.
            "--no-diffs" => {}
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    if args.src != "image" {
        return Err(format!("unknown --src `{}` (image)", args.src));
    }
    Ok(args)
}

/// `push_codec_srcs` for every (selected) file of the directory; a file DM cannot make a codec
/// for has no sources (and its manifest entry is excluded or stays todo).
fn gather_files(dir: &Path, matches: &[String]) -> Vec<(String, Vec<Box<dyn Src>>)> {
    dm_image::collect_images(dir)
        .iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_string_lossy().into_owned();
            if !matches.is_empty() && !matches.iter().any(|m| name.contains(m.as_str())) {
                return None;
            }
            let srcs = dm_image::gather(path);
            (!srcs.is_empty()).then_some((name, srcs))
        })
        .collect()
}

/// Checks the files on a few threads (the tier a render uses is thread-local), in file order.
/// The unit of work is a source, the biggest files first: a few big images have hundreds of
/// sources that each decode them.
fn check_files(
    files: &[(String, Vec<Box<dyn Src>>)],
    dir: &Path,
    store: &GoldenStore,
    plan: &[TierPlan],
) -> Vec<GroupReport> {
    let opts = Options {
        diffs: None,
        ..Options::default()
    };
    let size = |name: &str| std::fs::metadata(dir.join(name)).map_or(0, |m| m.len());
    let mut order: Vec<usize> = (0..files.len()).collect();
    order.sort_by_key(|&f| std::cmp::Reverse(size(&files[f].0)));
    let tasks: Vec<(usize, usize)> = order
        .iter()
        .flat_map(|&f| (0..files[f].1.len()).map(move |s| (f, s)))
        .collect();

    let next = AtomicUsize::new(0);
    let reports: Mutex<Vec<Vec<Option<SrcReport>>>> = Mutex::new(
        files
            .iter()
            .map(|(_, srcs)| vec![None; srcs.len()])
            .collect(),
    );
    let remaining: Vec<AtomicUsize> = files
        .iter()
        .map(|(_, srcs)| AtomicUsize::new(srcs.len()))
        .collect();
    let workers = std::thread::available_parallelism().map_or(2, usize::from);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&(f, s)) = tasks.get(i) else {
                        break;
                    };
                    let (name, srcs) = &files[f];
                    let id = format!("dm/image/{name}");
                    let report = dm_check::check_src(&id, srcs[s].as_ref(), store, plan, &opts);
                    reports
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)[f][s] = Some(report);
                    if remaining[f].fetch_sub(1, Ordering::Relaxed) == 1 {
                        eprintln!("dm-verify: {id} checked");
                    }
                }
            });
        }
    });
    let reports = reports
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    files
        .iter()
        .zip(reports)
        .map(|((name, srcs), reports)| {
            dm_check::merge_group(
                &format!("dm/image/{name}"),
                srcs.len(),
                reports.into_iter().flatten(),
            )
        })
        .collect()
}

/// Goldens for results no source of ours makes: the enumeration is not DM's. They fail the file
/// they belong to.
fn check_unmade(
    groups: &mut [GroupReport],
    files: &[(String, Vec<Box<dyn Src>>)],
    store: &GoldenStore,
) {
    let made = dm_check::made_ids(files, &Config::ALL);
    let names: Vec<String> = files.iter().map(|(n, _)| n.clone()).collect();
    for (file, ids) in dm_check::unmade_goldens(store, "image", &made, &names) {
        let line = |id: &String| format!("UNMADE GOLDEN {id}: no source of ours makes it");
        match groups
            .iter_mut()
            .find(|g| g.id == format!("dm/image/{file}"))
        {
            Some(group) => {
                group.failure_count += ids.len();
                group.failures.extend(ids.iter().take(5).map(line));
                group.verdict = GroupVerdict::Failing;
            }
            None => println!(
                "dm-verify: {} golden results for `{file}` that no source of ours makes, e.g. {}",
                ids.len(),
                ids[0]
            ),
        }
    }
}

fn print_summary(groups: &[GroupReport]) {
    for g in groups {
        if g.verdict != GroupVerdict::Passing {
            println!(
                "{}: {:?} ({} sources, {} matches, {} failures, {} not checkable)",
                g.id, g.verdict, g.srcs, g.matches, g.failure_count, g.not_checkable
            );
            for f in g.failures.iter().take(6) {
                println!("  {f}");
            }
        }
    }
    let passing = groups
        .iter()
        .filter(|g| g.verdict == GroupVerdict::Passing)
        .count();
    println!(
        "{passing}/{} image files pass on every checkable tier",
        groups.len()
    );
}

fn finish(report: &Report, path: Option<&Path>) -> ExitCode {
    if let Some(path) = path {
        let json = serde_json::to_string_pretty(report).expect("report serializes");
        if let Err(e) = std::fs::write(path, json + "\n") {
            eprintln!("dm-verify: writing {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("dm-verify: {e}");
            return ExitCode::from(2);
        }
    };
    let mut report = Report {
        src: args.src.clone(),
        goldens: String::new(),
        goldens_available: false,
        unavailable_reason: String::new(),
        plan: Vec::new(),
        groups: Vec::new(),
    };
    let store = match GoldenStore::shared_dm(&DM_IMAGE) {
        Ok(DmGoldens::Available(store)) => store,
        Ok(DmGoldens::Unavailable(reason)) => {
            println!("dm-verify: no `{}` goldens: {reason}", args.src);
            report.unavailable_reason.clone_from(reason);
            return finish(&report, args.report.as_deref());
        }
        Err(e) => {
            eprintln!("dm-verify: loading goldens: {e}");
            return ExitCode::FAILURE;
        }
    };
    report.goldens_available = true;
    report.goldens.clone_from(&store.source);

    let dir = args
        .images
        .clone()
        .or_else(|| resource_dir().map(|d| d.join("images")))
        .filter(|d| d.is_dir());
    let Some(dir) = dir else {
        eprintln!("dm-verify: Skia's resources/images directory is missing (SKIA_RESOURCES)");
        return ExitCode::FAILURE;
    };

    let mut plan = check::plan(store);
    if !args.tiers.is_empty() {
        plan.retain(|p| args.tiers.iter().any(|t| t == p.tier.name()));
    }
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

    let files = gather_files(&dir, &args.matches);
    report.groups = check_files(&files, &dir, store, &plan);
    if args.matches.is_empty() {
        check_unmade(&mut report.groups, &files, store);
    }
    print_summary(&report.groups);
    finish(&report, args.report.as_deref())
}
