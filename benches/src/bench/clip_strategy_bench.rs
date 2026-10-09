// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/ClipStrategyBench.cpp

//! `ClipStrategyBench`: `count` circles clipped as one anti-aliased path (`kClipPath`), or
//! drawn into a layer that is masked by them (`kMask`), then a green fill (rendering).

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::Color;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;

use crate::def_bench;
use crate::prelude::*;

/// `ClipStrategyBench::Mode`.
// Port of: bench/ClipStrategyBench.cpp#L13-L17 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// `kClipPath`.
    ClipPath,
    /// `kMask`.
    Mask,
}

/// The canvas width of `Benchmark::getSize()` (`onGetSize()`'s default, 640 x 480).
const CANVAS_WIDTH: i32 = 640;

/// `class ClipStrategyBench`.
// Port of: bench/ClipStrategyBench.cpp#L13-L84 (chrome/m156)
struct ClipStrategyBench {
    mode: Mode,
    count: usize,
    /// `fName`.
    name: String,
    /// `fClipPath`: the circles as one path (`kClipPath` only).
    clip_path: Path,
}

impl ClipStrategyBench {
    // Port of: bench/ClipStrategyBench.cpp#L18-L33 (chrome/m156)
    fn new(mode: Mode, count: usize) -> Self {
        // fName("clip_strategy_")
        let mut name = "clip_strategy_".to_owned();
        let mut clip_path = Path::default();
        if mode == Mode::ClipPath {
            // fName.append("path_");
            name.push_str("path_");
            let mut builder = PathBuilder::new();
            for_each_clip_circle(count, |x, y, r| {
                // builder.addCircle(x, y, r);
                builder.add_circle((x, y), r, None);
            });
            // fClipPath = builder.detach();
            clip_path = builder.detach();
        } else {
            // fName.append("mask_");
            name.push_str("mask_");
        }
        // fName.appendf("%zu", count);
        name.push_str(&count.to_string());
        Self {
            mode,
            count,
            name,
            clip_path,
        }
    }
}

/// `forEachClipCircle(func)`: `count` circles of radius `q / 2` at `(q * i, q * i)`, where
/// `q = width / (count + 1)`.
// Port of: bench/ClipStrategyBench.cpp#L72-L81 (chrome/m156)
// The usize-to-float casts mirror the C++ `static_cast<float>` and `q * i` conversions.
#[allow(clippy::cast_precision_loss)]
fn for_each_clip_circle(count: usize, mut func: impl FnMut(f32, f32, f32)) {
    // auto q = static_cast<float>(this->getSize().width()) / (fCount + 1);
    let q = CANVAS_WIDTH as f32 / (count + 1) as f32;
    // for (size_t i = 1; i <= fCount; ++i) { auto x = q * i; func(x, x, q / 2); }
    for i in 1..=count {
        let x = q * i as f32;
        func(x, x, q / 2.0);
    }
}

impl Benchmark for ClipStrategyBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/ClipStrategyBench.cpp#L40-L70 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("ClipStrategyBench is a rendering bench");
        // SkPaint p, srcIn;
        let mut p = Paint::default();
        // p.setAntiAlias(true);
        p.set_anti_alias(true);
        let mut src_in = Paint::default();
        // srcIn.setBlendMode(SkBlendMode::kSrcIn);
        src_in.set_blend_mode(BlendMode::SrcIn);
        for _ in 0..loops {
            // SkAutoCanvasRestore acr(canvas, false);
            let save_count = canvas.save_count();
            if self.mode == Mode::ClipPath {
                // canvas->save();
                canvas.save();
                // canvas->clipPath(fClipPath, true);
                canvas.clip_path(&self.clip_path, ClipOp::Intersect, true);
            } else {
                // canvas->saveLayer(nullptr, nullptr);
                canvas.save_layer(&SaveLayerRec::default());
                let count = self.count;
                for_each_clip_circle(count, |x, y, r| {
                    // canvas->drawCircle(x, y, r, p);
                    canvas.draw_circle((x, y), r, &p);
                });
                // canvas->saveLayer(nullptr, &srcIn);
                canvas.save_layer(&SaveLayerRec::default().paint(&src_in));
            }
            // canvas->drawColor(SK_ColorGREEN);
            canvas.draw_color(Color::GREEN, None);
            canvas.restore_to_count(save_count);
        }
    }
}

// Port of: bench/ClipStrategyBench.cpp#L85-L88 (chrome/m156)
def_bench!(
    clip_strategy_path_1 = "ClipStrategyBench(ClipStrategyBench::Mode::kClipPath, 1 )",
    ClipStrategyBench::new(Mode::ClipPath, 1)
);
// Port of: bench/ClipStrategyBench.cpp#L86 (chrome/m156)
def_bench!(
    clip_strategy_path_5 = "ClipStrategyBench(ClipStrategyBench::Mode::kClipPath, 5 )",
    ClipStrategyBench::new(Mode::ClipPath, 5)
);
// Port of: bench/ClipStrategyBench.cpp#L87 (chrome/m156)
def_bench!(
    clip_strategy_path_10 = "ClipStrategyBench(ClipStrategyBench::Mode::kClipPath, 10 )",
    ClipStrategyBench::new(Mode::ClipPath, 10)
);
// Port of: bench/ClipStrategyBench.cpp#L88 (chrome/m156)
def_bench!(
    clip_strategy_path_100 = "ClipStrategyBench(ClipStrategyBench::Mode::kClipPath, 100)",
    ClipStrategyBench::new(Mode::ClipPath, 100)
);
// Port of: bench/ClipStrategyBench.cpp#L90 (chrome/m156)
def_bench!(
    clip_strategy_mask_1 = "ClipStrategyBench(ClipStrategyBench::Mode::kMask, 1 )",
    ClipStrategyBench::new(Mode::Mask, 1)
);
// Port of: bench/ClipStrategyBench.cpp#L91 (chrome/m156)
def_bench!(
    clip_strategy_mask_5 = "ClipStrategyBench(ClipStrategyBench::Mode::kMask, 5 )",
    ClipStrategyBench::new(Mode::Mask, 5)
);
// Port of: bench/ClipStrategyBench.cpp#L92 (chrome/m156)
def_bench!(
    clip_strategy_mask_10 = "ClipStrategyBench(ClipStrategyBench::Mode::kMask, 10 )",
    ClipStrategyBench::new(Mode::Mask, 10)
);
// Port of: bench/ClipStrategyBench.cpp#L93 (chrome/m156)
def_bench!(
    clip_strategy_mask_100 = "ClipStrategyBench(ClipStrategyBench::Mode::kMask, 100)",
    ClipStrategyBench::new(Mode::Mask, 100)
);
