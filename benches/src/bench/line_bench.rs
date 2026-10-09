// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/LineBench.cpp

//! `SkCanvas::drawPoints(kLines_PointMode, ...)` over 500 random points, stroked (a rendering
//! bench).

use skia_rust_core::canvas::PointMode;
use skia_rust_core::paint::Style;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::scalar::scalar;

use crate::def_bench;
use crate::prelude::*;

const PTS: usize = 500;

/// `class LineBench`.
// Port of: bench/LineBench.cpp#L11-L56 (chrome/m156)
struct LineBench {
    stroke_width: scalar,
    do_aa: bool,
    name: String,
    pts: [Point; PTS],
}

impl LineBench {
    fn new(width: scalar, do_aa: bool) -> Self {
        // fName.printf("lines_%g_%s", width, doAA ? "AA" : "BW");
        // (Rust's shortest float form matches C's %g for the widths registered here.)
        let name = format!("lines_{width}_{}", if do_aa { "AA" } else { "BW" });
        let mut rand = Random::default();
        let mut pts = [Point::default(); PTS];
        for pt in &mut pts {
            // fPts[i].set(rand.nextUScalar1() * 640, rand.nextUScalar1() * 480);
            let x = rand.next_u_scalar1() * 640.0;
            let y = rand.next_u_scalar1() * 480.0;
            *pt = Point::new(x, y);
        }
        Self {
            stroke_width: width,
            do_aa,
            name,
            pts,
        }
    }
}

impl Benchmark for LineBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("LineBench is a rendering bench");
        let mut paint = Paint::default();
        self.setup_paint(&mut paint);
        paint.set_style(Style::Stroke);
        paint.set_anti_alias(self.do_aa);
        paint.set_stroke_width(self.stroke_width);
        for _ in 0..loops {
            canvas.draw_points(PointMode::Lines, &self.pts, &paint);
        }
    }
}

// Port of: bench/LineBench.cpp#L63-L63 (chrome/m156)
def_bench!(
    line_bench_0_false = "LineBench(0, false)",
    LineBench::new(0.0, false)
);
// Port of: bench/LineBench.cpp#L64-L64 (chrome/m156)
def_bench!(
    line_bench_1_false = "LineBench(SK_Scalar1, false)",
    LineBench::new(1.0, false)
);
// Port of: bench/LineBench.cpp#L65-L65 (chrome/m156)
def_bench!(
    line_bench_0_true = "LineBench(0, true)",
    LineBench::new(0.0, true)
);
// Port of: bench/LineBench.cpp#L66-L66 (chrome/m156)
def_bench!(
    line_bench_half_true = "LineBench(SK_Scalar1/2, true)",
    LineBench::new(1.0 / 2.0, true)
);
// Port of: bench/LineBench.cpp#L67-L67 (chrome/m156)
def_bench!(
    line_bench_1_true = "LineBench(SK_Scalar1, true)",
    LineBench::new(1.0, true)
);
