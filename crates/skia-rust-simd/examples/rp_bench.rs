// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/SkRPBench.cpp (the harness shape: one pipeline, `run(0, 0, 128, 1)`
// in a loop)

//! Raster pipeline interpreter micro-benchmark (design §2.6 "Performance plan", R3).
//!
//! Skia's `bench/SkRPBench.cpp` times `p.run(0, 0, 128, 1)` of one-stage pipelines (`div_*`,
//! `gather_*`) that A3 does not port yet, so this runs the same harness over pipelines built
//! from A3's stages; `oracle/rp-bench/rp_bench.cpp` times the identical pipelines in Skia.
//!
//! ```text
//! cargo run --release -p skia-rust-simd --example rp_bench
//! ```
//!
//! Prints, per tier and precision, the best of 7 trials in nanoseconds per `run(0, 0, 128, 1)`,
//! building the program per call (`SkRasterPipeline::run`) and once (`compile`).

use std::hint::black_box;
use std::time::{Duration, Instant};

use skia_rust_simd::rp::{MemPtr, MemSlot, MemView, MemoryBindings, Program, Stage};
use skia_rust_simd::{Selection, Tier};

const W: usize = 128;
/// `W` as a float (exact).
const W_F64: f64 = 128.0;

/// Best-of-7 time per call of `f`, in nanoseconds.
fn time(mut f: impl FnMut()) -> f64 {
    // Calibrate to ~50 ms per trial.
    let mut loops = 1u32;
    loop {
        let t = Instant::now();
        for _ in 0..loops {
            f();
        }
        if t.elapsed() > Duration::from_millis(50) {
            break;
        }
        loops *= 2;
    }
    (0..7)
        .map(|_| {
            let t = Instant::now();
            for _ in 0..loops {
                f();
            }
            t.elapsed().as_secs_f64() * 1e9 / f64::from(loops)
        })
        .fold(f64::INFINITY, f64::min)
}

fn main() {
    let src = MemPtr::new(MemSlot(0), 0);
    let dst = MemPtr::new(MemSlot(1), 0);
    let cases: [(&str, Vec<Stage<'static>>); 3] = [
        ("srcover (1 stage)", vec![Stage::Srcover]),
        (
            "seed_shader, store_src",
            vec![Stage::SeedShader, Stage::StoreSrc(dst)],
        ),
        (
            "load_src, load_dst, srcover, store_dst",
            vec![
                Stage::LoadSrc(src),
                Stage::LoadDst(dst),
                Stage::Srcover,
                Stage::StoreDst(dst),
            ],
        ),
    ];
    let tiers: Vec<Tier> = [
        Tier::Scalar,
        Tier::Sse2,
        Tier::Sse41,
        Tier::Ml3,
        Tier::Ml4,
        Tier::Neon,
    ]
    .into_iter()
    .filter(|t| t.is_native())
    .collect();
    println!("ns per run(0, 0, {W}, 1), best of 7");
    for (name, stages) in &cases {
        println!("{name}");
        for &tier in &tiers {
            for force_highp in [false, true] {
                let mut program = Program::new(stages, Selection::native(tier), force_highp);
                if force_highp && tier == Tier::Scalar {
                    continue;
                }
                let mut a = vec![0x3fu8; 4 * 4 * 16];
                let mut b = vec![0x3eu8; 4 * 4 * 16];
                let mut mem = MemoryBindings::new()
                    .with(MemSlot(0), MemView::write(&mut a))
                    .with(MemSlot(1), MemView::write(&mut b));
                // `SkRasterPipeline::run()`: build the program (fresh scratch) every call.
                let run_ns = time(|| {
                    let mut p =
                        Program::new(black_box(stages), Selection::native(tier), force_highp);
                    p.run(0, 0, W, 1, black_box(&mut mem));
                });
                // `compile()` once, then run the compiled program.
                let ns = time(|| program.run(0, 0, W, 1, black_box(&mut mem)));
                let p = if program.is_lowp() { "lowp " } else { "highp" };
                println!(
                    "  {:<6} {p} run {run_ns:8.1} ns  compiled {ns:8.1} ns  ({:.3} ns/px compiled)",
                    tier.name(),
                    ns / W_F64
                );
            }
        }
    }
}
