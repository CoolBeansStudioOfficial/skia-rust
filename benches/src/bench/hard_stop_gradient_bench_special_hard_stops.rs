// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/HardStopGradientBench_SpecialHardStops.cpp

//! Clamped linear gradients with hard stops at fixed positions (`001`, `011`, `centered`), drawn
//! with `drawPaint` over a `w`×`h` canvas (`HardStopGradientBench_SpecialHardStops`). Rendering
//! bench.

use skia_rust_core::color::{Color4f, colors};
use skia_rust_core::point::Point;
use skia_rust_core::scalar::{int_to_scalar, scalar};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient_shader;

use crate::def_bench;
use crate::prelude::*;

/// `enum class Kind`.
// Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L12-L16 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    K001,
    K011,
    Centered,
}

impl Kind {
    /// `kindstr(k)`.
    // Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L18-L30 (chrome/m156)
    fn name(self) -> &'static str {
        match self {
            Self::K001 => "001",
            Self::K011 => "011",
            Self::Centered => "centered",
        }
    }
}

/// `class HardStopGradientBench_SpecialHardStops`.
// Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L32-L92 (chrome/m156)
struct HardStopGradientBenchSpecialHardStops {
    name: String,
    w: i32,
    h: i32,
    kind: Kind,
    paint: Paint,
}

impl HardStopGradientBenchSpecialHardStops {
    // Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L35-L41 (chrome/m156)
    fn new(w: i32, h: i32, kind: Kind) -> Self {
        Self {
            // fName.printf("hardstop_special_%03dx%03d_%s", fW, fH, kindstr(fKind));
            name: format!("hardstop_special_{w:03}x{h:03}_{}", kind.name()),
            w,
            h,
            kind,
            // SkPaint fPaint: default, so anti-aliasing is off.
            paint: Paint::default(),
        }
    }
}

impl Benchmark for HardStopGradientBenchSpecialHardStops {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L47-L49 (chrome/m156)
    fn size(&mut self) -> ISize {
        ISize::new(self.w, self.h)
    }

    /// `onPreDraw`: builds the shader (outside the timer).
    // Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L51-L80 (chrome/m156)
    fn on_pre_draw(&mut self, _canvas: Option<&Canvas>) {
        // fH/2.0f and fW+2.0f: the int-to-float conversions of C++.
        let h_half = int_to_scalar(self.h) / 2.0;
        let points = [
            Point::new(0.0, h_half),
            Point::new(int_to_scalar(self.w) + 2.0, h_half),
        ];
        let colors_list: [Color4f; 4] = [colors::RED, colors::GREEN, colors::BLUE, colors::YELLOW];
        let pos_001: [scalar; 3] = [0.0, 0.0, 1.0];
        let pos_011: [scalar; 3] = [0.0, 1.0, 1.0];
        let pos_centered: [scalar; 4] = [0.0, 0.5, 0.5, 1.0];
        // SkScalar* positions = fKind == k001 ? pos_001 : fKind == k011 ? pos_011 : pos_centered;
        let (positions, n): (&[scalar], usize) = match self.kind {
            Kind::K001 => (&pos_001, 3),
            Kind::K011 => (&pos_011, 3),
            Kind::Centered => (&pos_centered, 4),
        };
        // size_t N = fKind == kCentered ? 4 : 3;
        let shader = gradient_shader::linear(
            (points[0], points[1]),
            &colors_list[..n],
            positions,
            TileMode::Clamp,
            None,
            None,
        );
        self.paint.set_shader(shader);
    }

    /// `onDraw`: `drawPaint` `loops` times.
    // Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L82-L87 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("HardStopGradientBench is a rendering bench");
        for _ in 0..loops {
            canvas.draw_paint(&self.paint);
        }
    }
}

// Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L110-L110 (chrome/m156)
def_bench!(
    hard_stop_special_100_k001 = "HardStopGradientBench_SpecialHardStops(100, 100, Kind::k001)",
    HardStopGradientBenchSpecialHardStops::new(100, 100, Kind::K001)
);
// Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L111-L111 (chrome/m156)
def_bench!(
    hard_stop_special_200_k001 = "HardStopGradientBench_SpecialHardStops(200, 200, Kind::k001)",
    HardStopGradientBenchSpecialHardStops::new(200, 200, Kind::K001)
);
// Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L112-L112 (chrome/m156)
def_bench!(
    hard_stop_special_300_k001 = "HardStopGradientBench_SpecialHardStops(300, 300, Kind::k001)",
    HardStopGradientBenchSpecialHardStops::new(300, 300, Kind::K001)
);
// Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L113-L113 (chrome/m156)
def_bench!(
    hard_stop_special_400_k001 = "HardStopGradientBench_SpecialHardStops(400, 400, Kind::k001)",
    HardStopGradientBenchSpecialHardStops::new(400, 400, Kind::K001)
);
// Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L114-L114 (chrome/m156)
def_bench!(
    hard_stop_special_500_k001 = "HardStopGradientBench_SpecialHardStops(500, 500, Kind::k001)",
    HardStopGradientBenchSpecialHardStops::new(500, 500, Kind::K001)
);
// Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L116-L116 (chrome/m156)
def_bench!(
    hard_stop_special_100_k011 = "HardStopGradientBench_SpecialHardStops(100, 100, Kind::k011)",
    HardStopGradientBenchSpecialHardStops::new(100, 100, Kind::K011)
);
// Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L117-L117 (chrome/m156)
def_bench!(
    hard_stop_special_200_k011 = "HardStopGradientBench_SpecialHardStops(200, 200, Kind::k011)",
    HardStopGradientBenchSpecialHardStops::new(200, 200, Kind::K011)
);
// Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L118-L118 (chrome/m156)
def_bench!(
    hard_stop_special_300_k011 = "HardStopGradientBench_SpecialHardStops(300, 300, Kind::k011)",
    HardStopGradientBenchSpecialHardStops::new(300, 300, Kind::K011)
);
// Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L119-L119 (chrome/m156)
def_bench!(
    hard_stop_special_400_k011 = "HardStopGradientBench_SpecialHardStops(400, 400, Kind::k011)",
    HardStopGradientBenchSpecialHardStops::new(400, 400, Kind::K011)
);
// Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L120-L120 (chrome/m156)
def_bench!(
    hard_stop_special_500_k011 = "HardStopGradientBench_SpecialHardStops(500, 500, Kind::k011)",
    HardStopGradientBenchSpecialHardStops::new(500, 500, Kind::K011)
);
// Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L122-L122 (chrome/m156)
def_bench!(
    hard_stop_special_100_centered = "HardStopGradientBench_SpecialHardStops(100, 100, Kind::kCentered)",
    HardStopGradientBenchSpecialHardStops::new(100, 100, Kind::Centered)
);
// Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L123-L123 (chrome/m156)
def_bench!(
    hard_stop_special_200_centered = "HardStopGradientBench_SpecialHardStops(200, 200, Kind::kCentered)",
    HardStopGradientBenchSpecialHardStops::new(200, 200, Kind::Centered)
);
// Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L124-L124 (chrome/m156)
def_bench!(
    hard_stop_special_300_centered = "HardStopGradientBench_SpecialHardStops(300, 300, Kind::kCentered)",
    HardStopGradientBenchSpecialHardStops::new(300, 300, Kind::Centered)
);
// Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L125-L125 (chrome/m156)
def_bench!(
    hard_stop_special_400_centered = "HardStopGradientBench_SpecialHardStops(400, 400, Kind::kCentered)",
    HardStopGradientBenchSpecialHardStops::new(400, 400, Kind::Centered)
);
// Port of: bench/HardStopGradientBench_SpecialHardStops.cpp#L126-L126 (chrome/m156)
def_bench!(
    hard_stop_special_500_centered = "HardStopGradientBench_SpecialHardStops(500, 500, Kind::kCentered)",
    HardStopGradientBenchSpecialHardStops::new(500, 500, Kind::Centered)
);
