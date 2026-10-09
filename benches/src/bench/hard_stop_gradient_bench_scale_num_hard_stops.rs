// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/HardStopGradientBench_ScaleNumHardStops.cpp

//! A clamped linear gradient with `color_count` colours and `hard_stop_count` pairs of hard
//! stops, drawn with `drawPaint` (`HardStopGradientBench_ScaleNumHardStops`). Rendering bench.

use skia_rust_core::color::{Color4f, colors};
use skia_rust_core::point::Point;
use skia_rust_core::scalar::{int_to_scalar, scalar};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient_shader;

use crate::def_bench;
use crate::prelude::*;

/// `static const int kSize = 500;`
// Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L84-L84 (chrome/m156)
const K_SIZE: i32 = 500;

/// `class HardStopGradientBench_ScaleNumHardStops`.
// Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L15-L82 (chrome/m156)
struct HardStopGradientBenchScaleNumHardStops {
    name: String,
    color_count: usize,
    hard_stop_count: usize,
    paint: Paint,
}

impl HardStopGradientBenchScaleNumHardStops {
    // Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L17-L25 (chrome/m156)
    fn new(color_count: usize, hard_stop_count: usize) -> Self {
        // SkASSERT(hardStopCount <= colorCount/2);
        assert!(hard_stop_count <= color_count / 2);
        Self {
            // fName.printf("hardstop_scale_num_hard_stops_%03d_colors_%03d_hard_stops", ...);
            name: format!(
                "hardstop_scale_num_hard_stops_{color_count:03}_colors_{hard_stop_count:03}_hard_stops"
            ),
            color_count,
            hard_stop_count,
            // SkPaint fPaint: default, so anti-aliasing is off.
            paint: Paint::default(),
        }
    }
}

impl Benchmark for HardStopGradientBenchScaleNumHardStops {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L31-L33 (chrome/m156)
    fn size(&mut self) -> ISize {
        ISize::new(K_SIZE, K_SIZE)
    }

    /// `onPreDraw`: builds the shader (outside the timer).
    // Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L35-L71 (chrome/m156)
    // The int and size_t counts are converted to float as in C++ (`clippy::cast_precision_loss`
    // is allowed for the whole body for that reason).
    #[allow(clippy::cast_precision_loss)]
    fn on_pre_draw(&mut self, _canvas: Option<&Canvas>) {
        // Left to right.
        let points = [
            Point::new(0.0, int_to_scalar(K_SIZE / 2)),
            Point::new(int_to_scalar(K_SIZE - 1), int_to_scalar(K_SIZE / 2)),
        ];
        // static constexpr std::array<SkColor4f, 4> color_choices
        let color_choices = [colors::RED, colors::GREEN, colors::BLUE, colors::YELLOW];
        let n = self.color_count;
        // Alternate between different choices.
        let color_values: Vec<Color4f> = (0..n)
            .map(|i| color_choices[i % color_choices.len()])
            .collect();
        // Create requisite number of hard stops, and evenly space positions after that.
        let mut positions = vec![0.0; n];
        let mut k = 0;
        for _ in 0..self.hard_stop_count {
            // float val = k/2.0f;
            let val = k as scalar / 2.0;
            // positions[k++] = val / N; positions[k++] = val / N;
            positions[k] = val / n as scalar;
            k += 1;
            positions[k] = val / n as scalar;
            k += 1;
        }
        for (i, position) in positions.iter_mut().enumerate().skip(k) {
            // positions[i] = i / (N - 1.0f);
            *position = i as scalar / (n as scalar - 1.0);
        }
        let shader = gradient_shader::linear(
            (points[0], points[1]),
            &color_values[..],
            &positions[..],
            TileMode::Clamp,
            None,
            None,
        );
        self.paint.set_shader(shader);
    }

    /// `onDraw`: `drawPaint` `loops` times.
    // Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L73-L79 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("HardStopGradientBench is a rendering bench");
        for _ in 0..loops {
            canvas.draw_paint(&self.paint);
        }
    }
}

// Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L102-L102 (chrome/m156)
def_bench!(
    hard_stop_scale_num_hard_stops_10_1 = "HardStopGradientBench_ScaleNumHardStops(10, 1)",
    HardStopGradientBenchScaleNumHardStops::new(10, 1)
);
// Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L103-L103 (chrome/m156)
def_bench!(
    hard_stop_scale_num_hard_stops_10_2 = "HardStopGradientBench_ScaleNumHardStops(10, 2)",
    HardStopGradientBenchScaleNumHardStops::new(10, 2)
);
// Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L104-L104 (chrome/m156)
def_bench!(
    hard_stop_scale_num_hard_stops_10_5 = "HardStopGradientBench_ScaleNumHardStops(10, 5)",
    HardStopGradientBenchScaleNumHardStops::new(10, 5)
);
// Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L106-L106 (chrome/m156)
def_bench!(
    hard_stop_scale_num_hard_stops_20_1 = "HardStopGradientBench_ScaleNumHardStops(20, 1)",
    HardStopGradientBenchScaleNumHardStops::new(20, 1)
);
// Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L107-L107 (chrome/m156)
def_bench!(
    hard_stop_scale_num_hard_stops_20_5 = "HardStopGradientBench_ScaleNumHardStops(20, 5)",
    HardStopGradientBenchScaleNumHardStops::new(20, 5)
);
// Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L108-L108 (chrome/m156)
def_bench!(
    hard_stop_scale_num_hard_stops_20_10 = "HardStopGradientBench_ScaleNumHardStops(20, 10)",
    HardStopGradientBenchScaleNumHardStops::new(20, 10)
);
// Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L110-L110 (chrome/m156)
def_bench!(
    hard_stop_scale_num_hard_stops_50_1 = "HardStopGradientBench_ScaleNumHardStops(50, 1)",
    HardStopGradientBenchScaleNumHardStops::new(50, 1)
);
// Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L111-L111 (chrome/m156)
def_bench!(
    hard_stop_scale_num_hard_stops_50_10 = "HardStopGradientBench_ScaleNumHardStops(50, 10)",
    HardStopGradientBenchScaleNumHardStops::new(50, 10)
);
// Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L112-L112 (chrome/m156)
def_bench!(
    hard_stop_scale_num_hard_stops_50_25 = "HardStopGradientBench_ScaleNumHardStops(50, 25)",
    HardStopGradientBenchScaleNumHardStops::new(50, 25)
);
// Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L114-L114 (chrome/m156)
def_bench!(
    hard_stop_scale_num_hard_stops_100_1 = "HardStopGradientBench_ScaleNumHardStops(100, 1)",
    HardStopGradientBenchScaleNumHardStops::new(100, 1)
);
// Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L115-L115 (chrome/m156)
def_bench!(
    hard_stop_scale_num_hard_stops_100_25 = "HardStopGradientBench_ScaleNumHardStops(100, 25)",
    HardStopGradientBenchScaleNumHardStops::new(100, 25)
);
// Port of: bench/HardStopGradientBench_ScaleNumHardStops.cpp#L116-L116 (chrome/m156)
def_bench!(
    hard_stop_scale_num_hard_stops_100_50 = "HardStopGradientBench_ScaleNumHardStops(100, 50)",
    HardStopGradientBenchScaleNumHardStops::new(100, 50)
);
