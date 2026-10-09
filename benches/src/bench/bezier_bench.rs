// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/BezierBench.cpp

//! Stroked quadratic and cubic Béziers drawn with each cap and join (a rendering bench,
//! `bench/BezierBench.cpp`).

use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::scalar::int_to_scalar;

use crate::def_bench;
use crate::prelude::*;

/// `DrawProc`: which path `draw_quad` or `draw_cubic` draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DrawProc {
    Quad,
    Cubic,
}

impl DrawProc {
    /// The name the proc returns (`draw_quad` returns `"quad"`).
    fn name(self) -> &'static str {
        match self {
            // Port of: bench/BezierBench.cpp#L22-L32 (chrome/m156)
            Self::Quad => "quad",
            // Port of: bench/BezierBench.cpp#L34-L44 (chrome/m156)
            Self::Cubic => "cubic",
        }
    }
}

/// `class BezierBench`.
// Port of: bench/BezierBench.cpp#L46-L97 (chrome/m156)
struct BezierBench {
    name: String,
    cap: Cap,
    join: Join,
    width: f32,
    proc_: DrawProc,
    quad: Path,
    cubic: Path,
}

impl BezierBench {
    fn new(cap: Cap, join: Join, width: i32, proc_: DrawProc) -> Self {
        let cap_name = match cap {
            Cap::Butt => "butt",
            Cap::Round => "round",
            Cap::Square => "square",
        };
        let join_name = match join {
            Join::Miter => "miter",
            Join::Round => "round",
            Join::Bevel => "bevel",
        };
        // fName.printf("draw_stroke_bezier_%s_%s_%s_%g", ...): `%g` of an integer width.
        let name = format!(
            "draw_stroke_bezier_{}_{cap_name}_{join_name}_{width}",
            proc_.name()
        );

        let mut builder = PathBuilder::new();
        builder
            .move_to((20.0, 20.0))
            .quad_to((60.0, 20.0), (60.0, 60.0))
            .quad_to((20.0, 60.0), (20.0, 100.0));
        let quad = builder.detach();

        builder
            .move_to((20.0, 20.0))
            .cubic_to((40.0, 20.0), (60.0, 40.0), (60.0, 60.0))
            .cubic_to((40.0, 60.0), (20.0, 80.0), (20.0, 100.0));
        let cubic = builder.detach();

        Self {
            name,
            cap,
            join,
            width: int_to_scalar(width), // SkIntToScalar(w)
            proc_,
            quad,
            cubic,
        }
    }
}

impl Benchmark for BezierBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend != Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("BezierBench is a rendering bench");
        let mut paint = Paint::default();
        self.setup_paint(&mut paint);
        paint
            .set_style(Style::Stroke)
            .set_stroke_cap(self.cap)
            .set_stroke_join(self.join)
            .set_stroke_width(self.width);
        // draw_quad / draw_cubic: `count` drawPath calls with the paint.
        let path = match self.proc_ {
            DrawProc::Quad => &self.quad,
            DrawProc::Cubic => &self.cubic,
        };
        for _ in 0..loops {
            canvas.draw_path(path, &paint);
        }
    }
}

// Port of: bench/BezierBench.cpp#L99-L107 (chrome/m156)
def_bench!(
    bezier_bench_butt_round_2_quad =
        "BezierBench(SkPaint::kButt_Cap, SkPaint::kRound_Join, 2, draw_quad)",
    BezierBench::new(Cap::Butt, Join::Round, 2, DrawProc::Quad)
);
def_bench!(
    bezier_bench_square_bevel_10_quad =
        "BezierBench(SkPaint::kSquare_Cap, SkPaint::kBevel_Join, 10, draw_quad)",
    BezierBench::new(Cap::Square, Join::Bevel, 10, DrawProc::Quad)
);
def_bench!(
    bezier_bench_round_miter_50_quad =
        "BezierBench(SkPaint::kRound_Cap, SkPaint::kMiter_Join, 50, draw_quad)",
    BezierBench::new(Cap::Round, Join::Miter, 50, DrawProc::Quad)
);
def_bench!(
    bezier_bench_butt_round_2_cubic =
        "BezierBench(SkPaint::kButt_Cap, SkPaint::kRound_Join, 2, draw_cubic)",
    BezierBench::new(Cap::Butt, Join::Round, 2, DrawProc::Cubic)
);
def_bench!(
    bezier_bench_square_bevel_10_cubic =
        "BezierBench(SkPaint::kSquare_Cap, SkPaint::kBevel_Join, 10, draw_cubic)",
    BezierBench::new(Cap::Square, Join::Bevel, 10, DrawProc::Cubic)
);
def_bench!(
    bezier_bench_round_miter_50_cubic =
        "BezierBench(SkPaint::kRound_Cap, SkPaint::kMiter_Join, 50, draw_cubic)",
    BezierBench::new(Cap::Round, Join::Miter, 50, DrawProc::Cubic)
);
