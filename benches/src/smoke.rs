// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/nanobench.cpp (time, is_enabled; the sequence of one sample)

//! The smoke run of design `docs/design/bench.md` §3.1, criterion C2: in each config a benchmark
//! is suitable for, perform `delayedSetup`, `perCanvasPreDraw`, `preDraw`, `draw(1)`, `postDraw`
//! and `perCanvasPostDraw` once, so a panic or a failed `debug_assert!` fails the test.
//!
//! This is the deterministic state nanobench reaches with `--loops 1 --samples 1`, so the canvas
//! bytes it leaves are what the output check (C3) compares with Skia's.

use skia_rust_core::color::Color;

use crate::nanobench::config::{self, SMOKE_CONFIGS};
use crate::nanobench::target::is_enabled;
use crate::registry::BenchRegistration;
use crate::{Benchmark, draw};

/// What one smoke sample left behind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmokeResult {
    pub unique_name: String,
    pub config: &'static str,
    /// The canvas bytes (rows without padding); `None` for nonrendering.
    pub bytes: Option<Vec<u8>>,
}

/// One sample of `bench` on every smoke config it is suitable for, in the order of nanobench's
/// loop: `delayedSetup` once, then per config the calls of `time(1, …)` between
/// `perCanvasPreDraw` and `perCanvasPostDraw`.
///
/// # Panics
/// If the bench panics or a `debug_assert!` in it fails.
// Port of: bench/nanobench.cpp#L421-L437, #L1540-L1625 (chrome/m156)
pub fn smoke_bench(bench: &mut dyn Benchmark) -> Vec<SmokeResult> {
    let mut results = Vec::new();
    bench.on_delayed_setup();
    for tag in SMOKE_CONFIGS {
        let config = config::find(tag).expect("smoke configs are CPU configs");
        let Some(mut target) = is_enabled(bench, config) else {
            continue;
        };
        bench.on_per_canvas_pre_draw(target.canvas());

        // time(1, bench, target)
        let canvas = target.canvas();
        if let Some(canvas) = canvas {
            canvas.clear(Color::WHITE);
        }
        bench.on_pre_draw(canvas);
        draw(bench, 1, canvas);
        bench.on_post_draw(canvas);

        bench.on_per_canvas_post_draw(target.canvas());
        results.push(SmokeResult {
            unique_name: bench.unique_name(),
            config: config.name,
            bytes: target.packed_bytes(),
        });
    }
    results
}

/// The test every registration generates: smoke-runs each benchmark the site creates, and
/// checks the site's runtime benchmarks have distinct, non-empty unique names that do not
/// include a declared omission.
///
/// # Panics
/// On any failure.
pub fn run_smoke_test(reg: &BenchRegistration) {
    let mut benches = (reg.factory)();
    assert!(!benches.is_empty(), "{} registers no benchmarks", reg.key());
    let mut names: Vec<String> = Vec::new();
    for bench in &mut benches {
        let name = bench.unique_name();
        assert!(!name.is_empty(), "{}: empty unique name", reg.key());
        assert!(
            !reg.omitted.contains(&name.as_str()),
            "{}: `{name}` is declared omitted but registered",
            reg.key()
        );
        assert!(
            !names.contains(&name),
            "{}: duplicate unique name `{name}`",
            reg.key()
        );
        let size = bench.size();
        assert!(
            size.width > 0 && size.height > 0,
            "{name}: empty size {size:?}"
        );
        assert!(bench.units() > 0, "{name}: units must be positive");
        smoke_bench(bench.as_mut());
        names.push(name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry;

    fn site(name: &str) -> &'static BenchRegistration {
        registry::all()
            .into_iter()
            .find(|r| r.name == name)
            .expect("registered")
    }

    #[test]
    fn registry_keys_follow_the_manifest() {
        let keys: Vec<String> = registry::all().iter().map(|r| r.key()).collect();
        assert!(keys.contains(&"bench::math_bench::NoOpMathBench()".to_owned()));
        assert!(keys.contains(&"bench::fs_rect_bench::FSRectBench()".to_owned()));
    }

    #[test]
    fn nonrendering_bench_runs_only_without_a_canvas() {
        let results = smoke_bench((site("NoOpMathBench()").factory)().remove(0).as_mut());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].config, "nonrendering");
        assert_eq!(results[0].bytes, None);
    }

    #[test]
    fn rendering_bench_draws_into_every_raster_config() {
        let results = smoke_bench((site("FSRectBench()").factory)().remove(0).as_mut());
        let configs: Vec<_> = results.iter().map(|r| r.config).collect();
        assert_eq!(configs, ["8888", "565", "f16"]);
        for r in &results {
            let bytes = r.bytes.as_ref().expect("pixels");
            // 640 x 480 pixels, and the opaque rects changed the white-cleared canvas.
            assert!(
                bytes.iter().any(|&b| b != 0xff),
                "{}: nothing drawn",
                r.config
            );
            assert!(bytes.iter().any(|&b| b != 0), "{}: all zero", r.config);
        }
    }
}
