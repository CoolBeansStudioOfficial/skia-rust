// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/HairlinePathBench.cpp

//! Zero-width stroked paths made of random segments picked from a fixed point table, drawn 100
//! times per loop, small or 3x big, with or without anti-aliasing (a rendering bench).

// The int-to-scalar and int-index casts mirror the C++ `SkIntToScalar` and `int` arithmetic.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Style;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::random::Random;
use skia_rust_core::scalar::scalar;

use crate::def_bench;
use crate::prelude::*;

/// `points`: 24 coordinates, read as 12 points.
// Port of: bench/HairlinePathBench.cpp#L29-L34 (chrome/m156)
const POINTS: [i32; 24] = [
    10, 10, 15, 5, 20, 20, 30, 5, 25, 20, 15, 12, 21, 21, 30, 30, 12, 4, 32, 28, 20, 18, 12, 10,
];

/// `kMaxPathSize`.
// Port of: bench/HairlinePathBench.cpp#L36 (chrome/m156)
const MAX_PATH_SIZE: i32 = 10;

/// The `makePath()` of each subclass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PathKind {
    Line,
    Quad,
    Conic,
    Cubic,
}

impl PathKind {
    /// `appendName`.
    // Port of: bench/HairlinePathBench.cpp#L86-L88, #L120-L122, #L154-L156, #L192-L194 (chrome/m156)
    fn name(self) -> &'static str {
        match self {
            PathKind::Line => "line",
            PathKind::Quad => "quad",
            PathKind::Conic => "conic",
            PathKind::Cubic => "cubic",
        }
    }
}

/// `class HairlinePathBench` and its four subclasses.
// Port of: bench/HairlinePathBench.cpp#L38-L80 (chrome/m156)
struct HairlinePathBench {
    kind: PathKind,
    big: bool,
    aa: bool,
    paint: Paint,
    name: String,
}

impl HairlinePathBench {
    // Port of: bench/HairlinePathBench.cpp#L40-L43 (chrome/m156)
    fn new(kind: PathKind, big: bool, aa: bool) -> Self {
        let mut paint = Paint::default();
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(0.0);
        // fName.printf("path_hairline_%s_%s_", big|small, AA|noAA), then appendName.
        let name = format!(
            "path_hairline_{}_{}_{}",
            if big { "big" } else { "small" },
            if aa { "AA" } else { "noAA" },
            kind.name()
        );
        Self {
            kind,
            big,
            aa,
            paint,
            name,
        }
    }

    /// `makePath()` of `LinePathBench`, `QuadPathBench`, `ConicPathBench` and `CubicPathBench`.
    fn make_path(&self) -> Path {
        let mut rand = Random::default();
        // ConicPathBench's weights come from their own generator.
        let mut rand_weight = Random::default();
        let mut builder = PathBuilder::new();
        let size = POINTS.len() as u32;
        let h_size = size / 2;
        for i in 0..MAX_PATH_SIZE {
            let x_trans = 10 + 40 * (i % (MAX_PATH_SIZE / 2));
            let mut y_trans = 0;
            if i > MAX_PATH_SIZE / 2 - 1 {
                y_trans = 40;
            }
            // Each base is an index into the point table; `2 * nextULessThan(hSize)`.
            let mut next_base = || 2 * rand.next_u_less_than(h_size) as usize;
            let pt = |base: usize| -> (scalar, scalar) {
                (
                    (POINTS[base] + x_trans) as scalar,
                    (POINTS[base + 1] + y_trans) as scalar,
                )
            };
            match self.kind {
                PathKind::Line => {
                    let base1 = next_base();
                    let base2 = next_base();
                    let base3 = next_base();
                    builder.move_to(pt(base1));
                    builder.line_to(pt(base2));
                    builder.line_to(pt(base3));
                }
                PathKind::Quad => {
                    let base1 = next_base();
                    let base2 = next_base();
                    let base3 = next_base();
                    builder.move_to(pt(base1));
                    builder.quad_to(pt(base2), pt(base3));
                }
                PathKind::Conic => {
                    let base1 = next_base();
                    let base2 = next_base();
                    let base3 = next_base();
                    let weight = rand_weight.next_range_f(0.0, 2.0);
                    builder.move_to(pt(base1));
                    builder.conic_to(pt(base2), pt(base3), weight);
                }
                PathKind::Cubic => {
                    let base1 = next_base();
                    let base2 = next_base();
                    let base3 = next_base();
                    let base4 = next_base();
                    builder.move_to(pt(base1));
                    builder.cubic_to(pt(base2), pt(base3), pt(base4));
                }
            }
        }
        builder.detach()
    }
}

impl Benchmark for HairlinePathBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/HairlinePathBench.cpp#L57-L73 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("HairlinePathBench is a rendering bench");
        let mut paint = self.paint.clone();
        self.setup_paint(&mut paint);

        paint.set_anti_alias(self.aa);

        let mut path = self.make_path();
        if self.big {
            path = path.make_transform(&Matrix::scale((3.0, 3.0)));
        }

        for _ in 0..loops {
            for _ in 0..100 {
                canvas.draw_path(&path, &paint);
            }
        }
    }
}

// Flags: FLAGS00 is small without AA, FLAGS01 (kBig_Flag) big without AA, FLAGS10 (kAA_Flag) small
// with AA, FLAGS11 big with AA. (Skia's own comment labels FLAGS01 "small"; the flag decides.)
// Port of: bench/HairlinePathBench.cpp#L230-L230 (chrome/m156)
def_bench!(
    hairline_path_bench_line_00 = "LinePathBench(FLAGS00)",
    HairlinePathBench::new(PathKind::Line, false, false)
);
// Port of: bench/HairlinePathBench.cpp#L231-L231 (chrome/m156)
def_bench!(
    hairline_path_bench_line_01 = "LinePathBench(FLAGS01)",
    HairlinePathBench::new(PathKind::Line, true, false)
);
// Port of: bench/HairlinePathBench.cpp#L232-L232 (chrome/m156)
def_bench!(
    hairline_path_bench_line_10 = "LinePathBench(FLAGS10)",
    HairlinePathBench::new(PathKind::Line, false, true)
);
// Port of: bench/HairlinePathBench.cpp#L233-L233 (chrome/m156)
def_bench!(
    hairline_path_bench_line_11 = "LinePathBench(FLAGS11)",
    HairlinePathBench::new(PathKind::Line, true, true)
);

// Port of: bench/HairlinePathBench.cpp#L235-L235 (chrome/m156)
def_bench!(
    hairline_path_bench_quad_00 = "QuadPathBench(FLAGS00)",
    HairlinePathBench::new(PathKind::Quad, false, false)
);
// Port of: bench/HairlinePathBench.cpp#L236-L236 (chrome/m156)
def_bench!(
    hairline_path_bench_quad_01 = "QuadPathBench(FLAGS01)",
    HairlinePathBench::new(PathKind::Quad, true, false)
);
// Port of: bench/HairlinePathBench.cpp#L237-L237 (chrome/m156)
def_bench!(
    hairline_path_bench_quad_10 = "QuadPathBench(FLAGS10)",
    HairlinePathBench::new(PathKind::Quad, false, true)
);
// Port of: bench/HairlinePathBench.cpp#L238-L238 (chrome/m156)
def_bench!(
    hairline_path_bench_quad_11 = "QuadPathBench(FLAGS11)",
    HairlinePathBench::new(PathKind::Quad, true, true)
);

// Don't have default path renderer for conics yet on GPU, so must use AA.
// (The FLAGS00 and FLAGS01 conic registrations are commented out in Skia.)
// Port of: bench/HairlinePathBench.cpp#L243-L243 (chrome/m156)
def_bench!(
    hairline_path_bench_conic_10 = "ConicPathBench(FLAGS10)",
    HairlinePathBench::new(PathKind::Conic, false, true)
);
// Port of: bench/HairlinePathBench.cpp#L244-L244 (chrome/m156)
def_bench!(
    hairline_path_bench_conic_11 = "ConicPathBench(FLAGS11)",
    HairlinePathBench::new(PathKind::Conic, true, true)
);

// Port of: bench/HairlinePathBench.cpp#L246-L246 (chrome/m156)
def_bench!(
    hairline_path_bench_cubic_00 = "CubicPathBench(FLAGS00)",
    HairlinePathBench::new(PathKind::Cubic, false, false)
);
// Port of: bench/HairlinePathBench.cpp#L247-L247 (chrome/m156)
def_bench!(
    hairline_path_bench_cubic_01 = "CubicPathBench(FLAGS01)",
    HairlinePathBench::new(PathKind::Cubic, true, false)
);
// Port of: bench/HairlinePathBench.cpp#L248-L248 (chrome/m156)
def_bench!(
    hairline_path_bench_cubic_10 = "CubicPathBench(FLAGS10)",
    HairlinePathBench::new(PathKind::Cubic, false, true)
);
// Port of: bench/HairlinePathBench.cpp#L249-L249 (chrome/m156)
def_bench!(
    hairline_path_bench_cubic_11 = "CubicPathBench(FLAGS11)",
    HairlinePathBench::new(PathKind::Cubic, true, true)
);
