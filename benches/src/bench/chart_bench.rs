// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/ChartBench.cpp

//! `ChartBench`: five scrolling line plots, each with the area below it filled, animated by
//! shifting their data (rendering). Stresses path filling.

use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::path::Path;

use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::random::Random;
use skia_rust_core::scalar::scalar;

use crate::def_bench;
use crate::prelude::*;

/// `kNumGraphs`, `kPixelsPerTick` and `kShiftPerFrame`.
// Port of: bench/ChartBench.cpp#L121-L125 (chrome/m156)
const NUM_GRAPHS: usize = 5;
const PIXELS_PER_TICK: i32 = 3;
const SHIFT_PER_FRAME: i32 = 1;

/// `gen_data`: `count` y values in `[yAvg - ySpread/2, yAvg + ySpread/2)`.
// Port of: bench/ChartBench.cpp#L13-L21 (chrome/m156)
fn gen_data(y_avg: scalar, y_spread: scalar, count: usize, random: &mut Random) -> Vec<scalar> {
    // dataPts->resize(count); (*dataPts)[i] = random->nextRangeScalar(...);
    (0..count)
        .map(|_| random.next_range_scalar(y_avg - (y_spread / 2.0), y_avg + (y_spread / 2.0)))
        .collect()
}

/// `gen_paths`: the stroked plot along the top of a data set and the fill below it, bounded by
/// `bottom` (the previous plot) or the horizontal line `y_base`. The plots are rotated by
/// `left_shift` points.
// Port of: bench/ChartBench.cpp#L24-L76 (chrome/m156)
fn gen_paths(
    top: &[scalar],
    bottom: Option<&[scalar]>,
    y_base: scalar,
    x_left: scalar,
    x_delta: scalar,
    left_shift: i32,
) -> (Path, Path) {
    // SkPathBuilder plot, fill;
    let mut plot = PathBuilder::new();
    let mut fill = PathBuilder::new();
    let count = i32::try_from(top.len()).expect("a data set fits in an int");
    // plot.incReserve(topData.size());
    plot.inc_reserve(count, count, 0);
    // fill.incReserve(...)
    match bottom {
        None => fill.inc_reserve(count + 2, count + 2, 0),
        Some(_) => fill.inc_reserve(2 * count, 2 * count, 0),
    }
    // leftShift %= topData.size();
    let left_shift = left_shift % count;
    let mut x = x_left;
    // int shiftToEndCount = topData.size() - leftShift;
    let shift_to_end_count = count - left_shift;
    let shift = usize::try_from(left_shift).expect("non-negative");
    let end_count = usize::try_from(shift_to_end_count).expect("non-negative");
    // plot.moveTo(x, topData[leftShift]); fill.moveTo(x, topData[leftShift]);
    plot.move_to((x, top[shift]));
    fill.move_to((x, top[shift]));
    // for (int i = 1; i < shiftToEndCount; ++i)
    for i in 1..end_count {
        plot.line_to((x, top[i + shift]));
        fill.line_to((x, top[i + shift]));
        x += x_delta;
    }
    // for (int i = 0; i < leftShift; ++i)
    for &y in &top[..shift] {
        plot.line_to((x, y));
        fill.line_to((x, y));
        x += x_delta;
    }
    if let Some(bottom) = bottom {
        // SkASSERT(bottomData->size() == topData.size());
        debug_assert_eq!(bottom.len(), top.len());
        // Iterate backwards over the previous graph's data to generate the bottom of the filled
        // area (and account for leftShift).
        for i in 0..shift {
            x -= x_delta;
            fill.line_to((x, bottom[shift - 1 - i]));
        }
        for i in 0..end_count {
            x -= x_delta;
            fill.line_to((x, bottom[bottom.len() - 1 - i]));
        }
    } else {
        fill.line_to((x - x_delta, y_base));
        fill.line_to((x_left, y_base));
    }
    // *plotOut = plot.detach(); *fillOut = fill.detach();
    (plot.detach(), fill.detach())
}

/// `class ChartBench`.
// Port of: bench/ChartBench.cpp#L95-L197 (chrome/m156)
struct ChartBench {
    shift: i32,
    /// `fSize`: the last base layer size (-1, -1 at first).
    size: (i32, i32),
    /// `fData`: the data of each graph.
    data: [Vec<scalar>; NUM_GRAPHS],
    aa: bool,
}

impl ChartBench {
    // Port of: bench/ChartBench.cpp#L100-L105 (chrome/m156)
    fn new(aa: bool) -> Self {
        Self {
            shift: 0,
            size: (-1, -1),
            data: std::array::from_fn(|_| Vec::new()),
            aa,
        }
    }
}

impl Benchmark for ChartBench {
    // Port of: bench/ChartBench.cpp#L107-L113 (chrome/m156)
    fn name(&self) -> String {
        if self.aa {
            "chart_aa".to_owned()
        } else {
            "chart_bw".to_owned()
        }
    }

    // Port of: bench/ChartBench.cpp#L114-L177 (chrome/m156)
    // The int-to-scalar casts mirror SkIntToScalar and the C++ int arithmetic of the bench.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("ChartBench is a rendering bench");
        // if (canvas->getBaseLayerSize() != fSize) { fSize = ...; sizeChanged = true; }
        let base = canvas.base_layer_size();
        let base_size = (base.width, base.height);
        let mut size_changed = false;
        if base_size != self.size {
            self.size = base_size;
            size_changed = true;
        }
        let (width, height_px) = self.size;
        // SkScalar ySpread = SkIntToScalar(fSize.fHeight / 20);
        let y_spread = (height_px / 20) as scalar;
        // SkScalar height = SkIntToScalar(fSize.fHeight);
        let height = height_px as scalar;
        if size_changed {
            // int dataPointCount = std::max(fSize.fWidth / kPixelsPerTick + 1, 2);
            let data_point_count =
                usize::try_from((width / PIXELS_PER_TICK + 1).max(2)).expect("positive");
            // SkRandom random;
            let mut random = Random::default();
            for i in 0..NUM_GRAPHS {
                // SkScalar y = (kNumGraphs - i) * (height - ySpread) / (kNumGraphs + 1);
                let y =
                    (NUM_GRAPHS - i) as scalar * (height - y_spread) / (NUM_GRAPHS + 1) as scalar;
                // fData[i].reset(); gen_data(y, ySpread, dataPointCount, &random, &fData[i]);
                self.data[i] = gen_data(y, y_spread, data_point_count, &mut random);
            }
        }
        // SkRandom colorRand;
        let mut color_rand = Random::default();
        let mut colors = [0u32; NUM_GRAPHS];
        for color in &mut colors {
            // colors[i] = colorRand.nextU() | 0xff000000;
            *color = color_rand.next_u() | 0xff00_0000;
        }
        for _ in 0..loops {
            // static const SkScalar kStrokeWidth = SkIntToScalar(2);
            let stroke_width: scalar = 2.0;
            // SkPaint plotPaint; SkPaint fillPaint;
            let mut plot_paint = Paint::default();
            let mut fill_paint = Paint::default();
            plot_paint.set_anti_alias(self.aa);
            plot_paint.set_style(Style::Stroke);
            plot_paint.set_stroke_width(stroke_width);
            plot_paint.set_stroke_cap(Cap::Round);
            plot_paint.set_stroke_join(Join::Round);
            fill_paint.set_anti_alias(self.aa);
            fill_paint.set_style(Style::Fill);
            // SkTDArray<SkScalar>* prevData = nullptr;
            let mut prev_data: Option<usize> = None;
            for (i, &color) in colors.iter().enumerate() {
                // gen_paths(fData[i], prevData, height, 0, SkIntToScalar(kPixelsPerTick), fShift, ...)
                let bottom = prev_data.map(|p| self.data[p].as_slice());
                let (plot_path, fill_path) = gen_paths(
                    &self.data[i],
                    bottom,
                    height,
                    0.0,
                    PIXELS_PER_TICK as scalar,
                    self.shift,
                );
                // Make the fills partially transparent
                // fillPaint.setColor((colors[i] & 0x00ffffff) | 0x80000000);
                fill_paint.set_color((color & 0x00ff_ffff) | 0x8000_0000);
                canvas.draw_path(&fill_path, &fill_paint);
                plot_paint.set_color(color);
                canvas.draw_path(&plot_path, &plot_paint);
                // prevData = &fData[i];
                prev_data = Some(i);
            }
            // fShift += kShiftPerFrame;
            self.shift += SHIFT_PER_FRAME;
        }
    }
}

// Port of: bench/ChartBench.cpp#L197 (chrome/m156)
def_bench!(chart_aa = "ChartBench(true)", ChartBench::new(true));
// Port of: bench/ChartBench.cpp#L198 (chrome/m156)
def_bench!(chart_bw = "ChartBench(false)", ChartBench::new(false));
