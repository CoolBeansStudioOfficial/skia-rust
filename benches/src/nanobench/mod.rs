// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/nanobench.cpp (the CPU path), tools/flags/CommandLineFlags.cpp
// (ShouldSkip)

//! A port of the CPU path of nanobench (design `docs/design/bench.md` §1.3 and §4): configs
//! `nonrendering` and the raster color types, loop calibration, the first-bench warm-up,
//! `--samples`/`--ms`/`--loops`/`--maxLoops`/`--overheadGoal`/`--match`/`--config`/`--dryRun`,
//! `Stats` and nanobench's `--outResultsFile` JSON. GPU targets, `BenchmarkStream`'s SKP, SVG,
//! image and GM sources, tracing and pprof are left out.
//!
//! The runner's one addition is `--writeRaw <dir>`: the canvas bytes of every
//! (benchmark, config) after the run, for the output check of design §3.1 (C3).

pub mod config;
pub mod results_writer;
pub mod stats;
pub mod target;
pub mod timer;

use std::path::PathBuf;

use skia_rust_core::color::Color;

use self::config::Config;
use self::results_writer::{BenchResult, NanoJsonResultsWriter};
use self::stats::Stats;
use self::target::{Target, is_enabled};
use self::timer::{estimate_timer_overhead, now_ms};
use crate::registry::BenchRegistration;
use crate::{Benchmark, draw};

/// `kAutoTuneLoops`.
// Port of: bench/nanobench.cpp#L133-L133 (chrome/m156)
pub const AUTO_TUNE_LOOPS: i32 = 0;

/// `kFailedLoops`.
// Port of: bench/nanobench.cpp#L496-L496 (chrome/m156)
const FAILED_LOOPS: i32 = -2;

/// The command-line flags of the CPU path, with nanobench's defaults.
// Port of: bench/nanobench.cpp#L133-L163 (chrome/m156)
#[derive(Clone, Debug)]
pub struct Flags {
    /// `--samples`: samples to measure per bench.
    pub samples: i32,
    /// `--ms`: if > 0, run each bench for this many ms instead of obeying `--samples`.
    pub ms: i32,
    /// `--loops`: 0 (`kAutoTuneLoops`) tunes; negative runs forever.
    pub loops: i32,
    /// `--overheadLoops`: loops to estimate timer overhead.
    pub overhead_loops: i32,
    /// `--overheadGoal`: loop until timer overhead is at most this fraction of the sample.
    pub overhead_goal: f64,
    /// `--maxCalibrationAttempts`.
    pub max_calibration_attempts: i32,
    /// `--maxLoops`: never run a bench more times than this.
    pub max_loops: i32,
    /// `--match`: `CommandLineFlags::ShouldSkip` patterns on the unique name.
    pub match_: Vec<String>,
    /// `--config`: config tags.
    pub configs: Vec<String>,
    /// `--quiet`: one terse line per result.
    pub quiet: bool,
    /// `--dryRun`: print what would run and do not run it.
    pub dry_run: bool,
    /// `--outResultsFile`.
    pub out_results_file: Option<PathBuf>,
    /// `--writeRaw`.
    pub write_raw: Option<PathBuf>,
}

impl Default for Flags {
    fn default() -> Self {
        Self {
            samples: 10,
            ms: 0,
            loops: AUTO_TUNE_LOOPS,
            overhead_loops: 100_000,
            overhead_goal: 0.0001,
            max_calibration_attempts: 3,
            max_loops: 1_000_000,
            match_: Vec::new(),
            configs: config::DEFAULT_CONFIGS.map(str::to_owned).to_vec(),
            quiet: false,
            dry_run: false,
            out_results_file: None,
            write_raw: None,
        }
    }
}

/// `CommandLineFlags::ShouldSkip()`: whether `name` is filtered out by `--match` patterns
/// (`~` excludes, a leading `^` anchors the start, a trailing `$` anchors the end).
// Port of: tools/flags/CommandLineFlags.cpp#L360-L390 (chrome/m156)
#[must_use]
pub fn should_skip(strings: &[String], name: &str) -> bool {
    let mut any_exclude = strings.is_empty();
    for s in strings {
        let mut match_name = s.as_str();
        let match_exclude = match_name.starts_with('~');
        if match_exclude {
            any_exclude = true;
            match_name = &match_name[1..];
        }
        let match_start = match_name.starts_with('^');
        if match_start {
            match_name = &match_name[1..];
        }
        let match_end = match_name.ends_with('$');
        let needle = if match_end {
            &match_name[..match_name.len() - 1]
        } else {
            match_name
        };
        let hit = if match_start {
            (!match_end || needle.len() == name.len()) && name.starts_with(needle)
        } else if match_end {
            name.ends_with(needle)
        } else {
            name.contains(match_name)
        };
        if hit {
            return match_exclude;
        }
    }
    !any_exclude
}

/// `time()`: one timed `draw` of `loops` loops, in ms. Clears the canvas to white, `preDraw`,
/// times `draw`, `postDraw`.
// Port of: bench/nanobench.cpp#L421-L437 (chrome/m156)
pub fn time(loops: i32, bench: &mut dyn Benchmark, target: &mut Target) -> f64 {
    let canvas = target.canvas();
    if let Some(canvas) = canvas {
        canvas.clear(Color::WHITE);
    }
    bench.on_pre_draw(canvas);
    let start = now_ms();
    draw(bench, loops, canvas);
    let elapsed = now_ms() - start;
    bench.on_post_draw(canvas);
    elapsed
}

/// `detect_forever_loops()`: a negative count is the magic run-forever value.
// Port of: bench/nanobench.cpp#L448-L454 (chrome/m156)
fn detect_forever_loops(loops: i32) -> i32 {
    if loops < 0 { i32::MAX } else { loops }
}

/// `clamp_loops()`.
// Port of: bench/nanobench.cpp#L456-L467 (chrome/m156)
fn clamp_loops(loops: i32, max_loops: i32) -> i32 {
    if loops < 1 {
        eprintln!(
            "ERROR: clamping loops from {loops} to 1. There's probably something wrong with the bench."
        );
        return 1;
    }
    if loops > max_loops {
        eprintln!("WARNING: clamping loops from {loops} to FLAGS_maxLoops, {max_loops}.");
        return max_loops;
    }
    loops
}

/// `setup_cpu_bench()`: how many loops one sample runs, or `None` when the bench cannot be
/// timed (`kFailedLoops`).
// Port of: bench/nanobench.cpp#L503-L545 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors `(int)ceil(numer / denom)`
fn setup_cpu_bench(
    flags: &Flags,
    overhead: f64,
    target: &mut Target,
    bench: &mut dyn Benchmark,
) -> Option<i32> {
    // First figure out approximately how many loops of bench it takes to make overhead
    // negligible.
    let mut bench_plus_overhead = 0.0;
    let mut round = 0;
    let mut loops = if bench.should_loop() { flags.loops } else { 1 };
    if loops == AUTO_TUNE_LOOPS {
        while bench_plus_overhead < overhead {
            if round == flags.max_calibration_attempts {
                eprintln!(
                    "WARNING: Can't estimate loops for {} ({bench_plus_overhead} ms vs. {overhead} ms); skipping.",
                    bench.unique_name()
                );
                return None;
            }
            round += 1;
            bench_plus_overhead = time(1, bench, target);
        }
    }

    // Later we'll just start and stop the timer once but loop N times. We pick N to make timer
    // overhead negligible (the derivation is in the C++).
    if loops == AUTO_TUNE_LOOPS {
        let numer = overhead / flags.overhead_goal - overhead;
        let denom = bench_plus_overhead - overhead;
        loops = (numer / denom).ceil() as i32;
        loops = clamp_loops(loops, flags.max_loops);
    } else {
        loops = detect_forever_loops(loops);
    }
    debug_assert!(loops != FAILED_LOOPS);
    Some(loops)
}

/// One finished (benchmark, config) measurement.
#[derive(Clone, Debug)]
pub struct Measurement {
    pub unique_name: String,
    pub config: &'static str,
    pub loops: i32,
    /// Per-loop, per-unit times in ms.
    pub samples: Vec<f64>,
    pub stats: Stats,
}

/// Runs every registered benchmark that passes `--match` on every enabled `--config`, as
/// `main()` of nanobench does, and returns the JSON document.
///
/// `manifest_ids` names a site by its manifest registration name (`options.manifest_id` in the
/// JSON, design §4).
///
/// # Errors
/// If a config tag is unknown, or `--writeRaw` cannot write.
// Port of: bench/nanobench.cpp#L1380-L1700 (chrome/m156)
pub fn run(flags: &Flags, sites: &[&BenchRegistration]) -> Result<NanoJsonResultsWriter, String> {
    let mut configs: Vec<Config> = Vec::new();
    for tag in &flags.configs {
        configs.push(config::find(tag).ok_or_else(|| format!("Unknown config '{tag}'."))?);
    }

    let mut log = NanoJsonResultsWriter::new();
    log.add_key("impl", "skia-rust");
    log.add_key("tier", skia_rust_simd::Tier::detect().name());
    log.add_key("os", std::env::consts::OS);
    log.add_key("arch", std::env::consts::ARCH);
    log.add_option("loops", &flags.loops.to_string());
    log.add_option("samples", &flags.samples.to_string());

    let overhead = estimate_timer_overhead(flags.overhead_loops);
    if !flags.quiet {
        eprintln!("Timer overhead: {overhead} ms");
    }

    let mut runs = 0usize;
    for site in sites {
        for mut bench in (site.factory)() {
            if should_skip(&flags.match_, &bench.unique_name()) {
                continue;
            }
            let size = bench.size();
            let bench_id =
                NanoJsonResultsWriter::bench_id(&bench.unique_name(), size.width, size.height);
            if !configs.is_empty() {
                bench.on_delayed_setup();
            }
            for &config in &configs {
                let Some(mut target) = is_enabled(bench.as_mut(), config) else {
                    continue;
                };
                if flags.dry_run {
                    eprintln!("Running {}\t{}", bench.unique_name(), config.name);
                    continue;
                }
                let m = measure(flags, overhead, runs, bench.as_mut(), &mut target);
                let Some(m) = m else { continue };
                if let Some(dir) = &flags.write_raw {
                    write_raw(dir, &m, size.width, size.height, &mut target)?;
                }
                log.add_result(&BenchResult {
                    bench_id: &bench_id,
                    config: config.name,
                    name: &bench.name(),
                    options: &[
                        ("manifest_id", site.name.to_owned()),
                        ("loops", m.loops.to_string()),
                    ],
                    min_ms: m.stats.min,
                    min_ratio: m.stats.median / m.stats.min,
                    samples: &m.samples,
                });
                print_result(flags, &m);
                runs += 1;
            }
        }
    }
    Ok(log)
}

/// The per-config part of nanobench's loop: `setup`, `perCanvasPreDraw`, loop calibration, the
/// first-bench warm-up, sampling, scaling to units, `perCanvasPostDraw`.
// Port of: bench/nanobench.cpp#L1527-L1625 (chrome/m156)
fn measure(
    flags: &Flags,
    overhead: f64,
    runs: usize,
    bench: &mut dyn Benchmark,
    target: &mut Target,
) -> Option<Measurement> {
    bench.on_per_canvas_pre_draw(target.canvas());

    let Some(loops) = setup_cpu_bench(flags, overhead, target, bench) else {
        // Can't be timed. A warning note has already been printed (cleanup_run).
        bench.on_per_canvas_post_draw(target.canvas());
        return None;
    };

    if runs == 0 && flags.ms < 1000 && flags.loops == AUTO_TUNE_LOOPS {
        // Run the first bench for 1000ms to warm up the nanobench if FLAGS_ms < 1000.
        // Otherwise, the first few benches' measurements will be inaccurate.
        let stop = now_ms() + 1000.0;
        loop {
            time(loops, bench, target);
            if now_ms() >= stop {
                break;
            }
        }
    }

    let mut samples: Vec<f64> = Vec::new();
    if flags.ms != 0 {
        let stop = now_ms() + f64::from(flags.ms);
        loop {
            samples.push(time(loops, bench, target) / f64::from(loops));
            if now_ms() >= stop {
                break;
            }
        }
    } else {
        for _ in 0..flags.samples.max(0) {
            samples.push(time(loops, bench, target) / f64::from(loops));
        }
    }

    // Scale each result to the benchmark's own units, time/unit.
    for sample in &mut samples {
        *sample *= 1.0 / f64::from(bench.units());
    }

    bench.on_per_canvas_post_draw(target.canvas());

    let stats = Stats::new(&samples);
    Some(Measurement {
        unique_name: bench.unique_name(),
        config: target.config.name,
        loops,
        samples,
        stats,
    })
}

/// `--writeRaw`: `<dir>/<config>/<unique name>.raw` (the canvas bytes, rows without padding) and
/// `.json` (size and color info).
fn write_raw(
    dir: &std::path::Path,
    m: &Measurement,
    width: i32,
    height: i32,
    target: &mut Target,
) -> Result<(), String> {
    let Some(bytes) = target.packed_bytes() else {
        return Ok(()); // nonrendering: no pixels
    };
    let out = dir.join(m.config);
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let stem = out.join(&m.unique_name);
    std::fs::write(stem.with_extension("raw"), &bytes)
        .map_err(|e| format!("{}: {e}", stem.display()))?;
    let meta = serde_json::json!({
        "name": m.unique_name,
        "config": m.config,
        "width": width,
        "height": height,
        "color_type": format!("{:?}", target.config.color),
        "alpha_type": format!("{:?}", target.config.alpha),
        "loops": m.loops,
        "samples": m.samples.len(),
        "bytes": bytes.len(),
    });
    std::fs::write(stem.with_extension("json"), meta.to_string())
        .map_err(|e| format!("{}: {e}", stem.display()))
}

/// The console line of a result (`--quiet`: `median(us) mark name config`).
// Port of: bench/nanobench.cpp#L1676-L1720 (chrome/m156)
fn print_result(flags: &Flags, m: &Measurement) {
    let stddev_percent = 100.0 * m.stats.var.sqrt() / m.stats.mean;
    if flags.quiet {
        let mut mark = " ";
        if stddev_percent > 5.0 {
            mark = "?";
        }
        if stddev_percent > 10.0 {
            mark = "!";
        }
        println!(
            "{:10.2} {mark}\t{}\t{}",
            m.stats.median * 1e3,
            m.unique_name,
            m.config
        );
    } else {
        println!(
            "{:8.2}us\t{:8.2}us\t{:8.2}us\t{:8.2}us\t{:5.1}%\t{}\t{}",
            m.stats.min * 1e3,
            m.stats.median * 1e3,
            m.stats.mean * 1e3,
            m.stats.max * 1e3,
            stddev_percent,
            m.unique_name,
            m.config
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pats(p: &[&str]) -> Vec<String> {
        p.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn should_skip_matches_command_line_flags() {
        // No patterns: nothing is skipped.
        assert!(!should_skip(&[], "math_noOp"));
        // Substring include; everything else is skipped.
        assert!(!should_skip(&pats(&["noOp"]), "math_noOp"));
        assert!(should_skip(&pats(&["noOp"]), "math_slowIsqrt"));
        // Anchors.
        assert!(!should_skip(&pats(&["^math"]), "math_noOp"));
        assert!(should_skip(&pats(&["^noOp"]), "math_noOp"));
        assert!(!should_skip(&pats(&["noOp$"]), "math_noOp"));
        assert!(should_skip(&pats(&["^math$"]), "math_noOp"));
        assert!(!should_skip(&pats(&["^math_noOp$"]), "math_noOp"));
        // Exclusion keeps everything else.
        assert!(should_skip(&pats(&["~noOp"]), "math_noOp"));
        assert!(!should_skip(&pats(&["~noOp"]), "math_slowIsqrt"));
    }

    #[test]
    fn runs_a_bench_and_writes_nanobench_json() {
        let flags = Flags {
            samples: 3,
            loops: 2,
            overhead_loops: 10,
            match_: vec!["^fullscreen_rects$".to_owned()],
            configs: vec!["8888".to_owned(), "nonrendering".to_owned()],
            quiet: true,
            ..Flags::default()
        };
        let log = run(&flags, &crate::registry::all()).expect("runs");
        let j = log.to_json();
        let r = &j["results"]["fullscreen_rects_640_480"];
        // Suitable for 8888 only.
        assert!(r.get("nonrendering").is_none());
        let r = &r["8888"];
        assert_eq!(r["options"]["name"], "fullscreen_rects");
        assert_eq!(r["options"]["manifest_id"], "FSRectBench()");
        assert_eq!(r["options"]["loops"], "2");
        assert_eq!(r["samples"].as_array().unwrap().len(), 3);
        assert_eq!(j["key"]["impl"], "skia-rust");
    }

    #[test]
    fn unknown_config_is_an_error() {
        let flags = Flags {
            configs: vec!["gl".to_owned()],
            ..Flags::default()
        };
        assert!(run(&flags, &[]).is_err());
    }

    #[test]
    fn forever_and_clamp() {
        assert_eq!(detect_forever_loops(-1), i32::MAX);
        assert_eq!(detect_forever_loops(5), 5);
        assert_eq!(clamp_loops(5, 10), 5);
        assert_eq!(clamp_loops(50, 10), 10);
        assert_eq!(clamp_loops(0, 10), 1);
    }
}
