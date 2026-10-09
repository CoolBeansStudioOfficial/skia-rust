// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/HardStopGradientBench_ScaleNumColors.cpp

//! A left-to-right linear gradient with `count` colours and a hard stop at the start, drawn with
//! `drawPaint` (`HardStopGradientBench_ScaleNumColors`). Rendering bench.

use skia_rust_core::color::{Color4f, colors};
use skia_rust_core::point::Point;
use skia_rust_core::scalar::{int_to_scalar, scalar};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient_shader;

use super::gradient_bench::tilemode_name;
use crate::def_bench;
use crate::prelude::*;

/// `class HardStopGradientBench_ScaleNumColors`.
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L21-L98 (chrome/m156)
struct HardStopGradientBenchScaleNumColors {
    name: String,
    tile_mode: TileMode,
    color_count: usize,
    paint: Paint,
}

/// `static const int kSize = 500;`
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L86-L86 (chrome/m156)
const K_SIZE: i32 = 500;

impl HardStopGradientBenchScaleNumColors {
    // Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L23-L29 (chrome/m156)
    fn new(tile_mode: TileMode, count: usize) -> Self {
        Self {
            // fName.printf("hardstop_scale_num_colors_%s_%03d_colors", tilemode_name, count);
            name: format!(
                "hardstop_scale_num_colors_{}_{count:03}_colors",
                tilemode_name(tile_mode)
            ),
            tile_mode,
            color_count: count,
            // SkPaint fPaint: default, so anti-aliasing is off.
            paint: Paint::default(),
        }
    }
}

impl Benchmark for HardStopGradientBenchScaleNumColors {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L36-L38 (chrome/m156)
    fn size(&mut self) -> ISize {
        ISize::new(K_SIZE, K_SIZE)
    }

    /// `onPreDraw`: builds the shader (outside the timer).
    // Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L44-L82 (chrome/m156)
    // The counts are converted to float as in C++ (`clippy::cast_precision_loss` is allowed for
    // the whole body for that reason).
    #[allow(clippy::cast_precision_loss)]
    fn on_pre_draw(&mut self, _canvas: Option<&Canvas>) {
        // Left to right: SkPoint::Make(0, kSize/2), SkPoint::Make(kSize-1, kSize/2)
        let points = [
            Point::new(0.0, int_to_scalar(K_SIZE / 2)),
            Point::new(int_to_scalar(K_SIZE - 1), int_to_scalar(K_SIZE / 2)),
        ];
        // static constexpr std::array<SkColor4f, 4> color_choices
        let color_choices = [colors::RED, colors::GREEN, colors::BLUE, colors::YELLOW];
        // Alternate between different choices.
        let colors: Vec<Color4f> = (0..self.color_count)
            .map(|i| color_choices[i % color_choices.len()])
            .collect();
        // Create a hard stop: the first two positions are both 0.
        let mut positions = vec![0.0; self.color_count];
        for i in 2..self.color_count {
            // Evenly spaced afterwards: i / (fColorCount - 1.0f)
            let value = i as scalar / (self.color_count as scalar - 1.0);
            positions[i] = value;
        }
        let shader = gradient_shader::linear(
            (points[0], points[1]),
            colors.as_slice(),
            positions.as_slice(),
            self.tile_mode,
            None,
            None,
        );
        self.paint.set_shader(shader);
    }

    /// `onDraw`: `drawPaint` `loops` times.
    // Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L87-L93 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("HardStopGradientBench is a rendering bench");
        for _ in 0..loops {
            canvas.draw_paint(&self.paint);
        }
    }
}

// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L101-L101 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_clamp_3 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kClamp, 3)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Clamp, 3)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L102-L102 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_clamp_4 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kClamp, 4)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Clamp, 4)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L103-L103 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_clamp_5 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kClamp, 5)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Clamp, 5)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L104-L104 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_clamp_10 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kClamp, 10)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Clamp, 10)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L105-L105 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_clamp_25 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kClamp, 25)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Clamp, 25)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L106-L106 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_clamp_50 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kClamp, 50)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Clamp, 50)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L107-L107 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_clamp_100 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kClamp, 100)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Clamp, 100)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L110-L110 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_repeat_3 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kRepeat, 3)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Repeat, 3)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L111-L111 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_repeat_4 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kRepeat, 4)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Repeat, 4)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L112-L112 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_repeat_5 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kRepeat, 5)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Repeat, 5)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L113-L113 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_repeat_10 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kRepeat, 10)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Repeat, 10)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L114-L114 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_repeat_25 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kRepeat, 25)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Repeat, 25)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L115-L115 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_repeat_50 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kRepeat, 50)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Repeat, 50)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L116-L116 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_repeat_100 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kRepeat, 100)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Repeat, 100)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L119-L119 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_mirror_3 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kMirror, 3)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Mirror, 3)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L120-L120 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_mirror_4 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kMirror, 4)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Mirror, 4)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L121-L121 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_mirror_5 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kMirror, 5)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Mirror, 5)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L122-L122 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_mirror_10 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kMirror, 10)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Mirror, 10)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L123-L123 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_mirror_25 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kMirror, 25)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Mirror, 25)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L124-L124 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_mirror_50 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kMirror, 50)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Mirror, 50)
);
// Port of: bench/HardStopGradientBench_ScaleNumColors.cpp#L125-L125 (chrome/m156)
def_bench!(
    hard_stop_scale_num_colors_mirror_100 = "HardStopGradientBench_ScaleNumColors(SkTileMode::kMirror, 100)",
    HardStopGradientBenchScaleNumColors::new(TileMode::Mirror, 100)
);
