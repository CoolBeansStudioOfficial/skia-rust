// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/nanobench.cpp (now_ms, estimate_timer_overhead)

//! The timer: `now_ms()` and `estimate_timer_overhead()`.

use std::sync::OnceLock;
use std::time::Instant;

/// `now_ms()`: `SkTime::GetNSecs() * 1e-6`, here nanoseconds since the first call.
// Port of: bench/nanobench.cpp#L240-L240 (chrome/m156)
#[must_use]
#[allow(clippy::cast_precision_loss)] // mirrors `GetNSecs() * 1e-6`: an integer times a double
pub fn now_ms() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_nanos() as f64 * 1e-6
}

/// `estimate_timer_overhead()`: the average cost of an empty `now_ms()` pair over
/// `overhead_loops` (`--overheadLoops`) iterations.
// Port of: bench/nanobench.cpp#L439-L446 (chrome/m156)
#[must_use]
pub fn estimate_timer_overhead(overhead_loops: i32) -> f64 {
    let mut overhead = 0.0;
    for _ in 0..overhead_loops {
        let start = now_ms();
        overhead += now_ms() - start;
    }
    overhead / f64::from(overhead_loops)
}
