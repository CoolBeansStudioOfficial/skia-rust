// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/DashBench.cpp

//! Dashed paths and lines: `SkDashPathEffect` over horizontal lines, shapes, grids and giant lines
//! (rendering benches).
//!
//! `RectDashBench` is declared in `bench/DashBench.cpp` but never registered, so it is not ported.

use skia_rust_core::canvas::PointMode;
use skia_rust_core::color::Color;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Cap, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_effect::PathEffect;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::{SCALAR_PI, int_to_scalar, scalar, scalar_cos, scalar_sin};
use skia_rust_core::stroke_rec::{InitStyle, StrokeRec};
use skia_rust_effects::dash_path_effect;

use crate::def_bench;
use crate::prelude::*;

/// `static const SkScalar gDots[] = { SK_Scalar1, SK_Scalar1 };`
// Port of: bench/DashBench.cpp#L445-L445 (chrome/m156)
const G_DOTS: [scalar; 2] = [1.0, 1.0];

/// `static SkPath path_hline()`.
// Port of: bench/DashBench.cpp#L30-L32 (chrome/m156)
fn path_hline() -> Path {
    Path::line((10.0, 10.0), (600.0, 10.0))
}

/// `class DashBench`: a horizontal line dashed by `intervals * width`, optionally clipped.
// Port of: bench/DashBench.cpp#L34-L98 (chrome/m156)
struct DashBench {
    name: String,
    intervals: Vec<scalar>,
    width: i32,
    do_clip: bool,
}

impl DashBench {
    fn new(intervals: &[scalar], width: i32, do_clip: bool) -> Self {
        // fIntervals[i] *= width;
        let scale = int_to_scalar(width);
        let intervals: Vec<scalar> = intervals.iter().map(|&i| i * scale).collect();
        Self {
            // fName.printf("dash_%d_%s", width, doClip ? "clipped" : "noclip");
            name: format!(
                "dash_{width}_{}",
                if do_clip { "clipped" } else { "noclip" }
            ),
            // fPts (the endpoints of the line) are only used by the commented-out drawPoints in
            // handlePath, so the port does not keep them.
            intervals,
            width,
            do_clip,
        }
    }

    /// `virtual void handlePath(...)`: `drawPath` `n` times.
    fn handle_path(canvas: &Canvas, path: &Path, paint: &Paint, n: i32) {
        for _ in 0..n {
            // canvas->drawPoints(kLines_PointMode, 2, fPts, paint) is commented out in C++.
            canvas.draw_path(path, paint);
        }
    }
}

impl Benchmark for DashBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("DashBench is a rendering bench");
        let mut paint = Paint::default();
        self.setup_paint(&mut paint);
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(int_to_scalar(self.width));
        paint.set_anti_alias(false);
        // this->makePath(): the only DashBench (no subclass overrides it, since RectDashBench is
        // never registered), so it is path_hline().
        let path = path_hline();
        paint.set_path_effect(dash_path_effect::new(&self.intervals, 0.0));
        if self.do_clip {
            let mut r = *path.bounds();
            // r.inset(-SkIntToScalar(20), -SkIntToScalar(20));
            r.inset((-int_to_scalar(20), -int_to_scalar(20)));
            // now move it so we don't intersect
            // r.offset(0, r.height() * 3 / 2);
            r.offset((0.0, r.height() * 3.0 / 2.0));
            canvas.clip_rect(r, None, None);
        }
        Self::handle_path(canvas, &path, &paint, loops);
    }
}

// Port of: bench/DashBench.cpp#L449-L449 (chrome/m156)
def_bench!(
    dash_bench_dots_0 = "DashBench(PARAM(gDots), 0)",
    DashBench::new(&G_DOTS, 0, false)
);
// Port of: bench/DashBench.cpp#L450-L450 (chrome/m156)
def_bench!(
    dash_bench_dots_1 = "DashBench(PARAM(gDots), 1)",
    DashBench::new(&G_DOTS, 1, false)
);
// Port of: bench/DashBench.cpp#L451-L451 (chrome/m156)
def_bench!(
    dash_bench_dots_1_clipped = "DashBench(PARAM(gDots), 1, true)",
    DashBench::new(&G_DOTS, 1, true)
);
// Port of: bench/DashBench.cpp#L452-L452 (chrome/m156)
def_bench!(
    dash_bench_dots_4 = "DashBench(PARAM(gDots), 4)",
    DashBench::new(&G_DOTS, 4, false)
);

/// `static SkPath make_unit_star(int n)`.
// Port of: bench/DashBench.cpp#L140-L152 (chrome/m156)
fn make_unit_star(n: i32) -> Path {
    let mut rad: scalar = -SCALAR_PI / 2.0;
    // const SkScalar drad = (n >> 1) * SK_ScalarPI * 2 / n;
    let drad: scalar = int_to_scalar(n >> 1) * SCALAR_PI * 2.0 / int_to_scalar(n);
    let mut builder = PathBuilder::new();
    builder.move_to((0.0, -1.0));
    for _ in 1..n {
        rad += drad;
        builder.line_to((scalar_cos(rad), scalar_sin(rad)));
    }
    builder.close();
    builder.detach()
}

/// `static SkPath make_poly()`.
// Port of: bench/DashBench.cpp#L154-L156 (chrome/m156)
fn make_poly() -> Path {
    make_unit_star(9).make_transform(&Matrix::scale((100.0, 100.0)))
}

/// `static SkPath make_quad()`.
// Port of: bench/DashBench.cpp#L158-L166 (chrome/m156)
fn make_quad() -> Path {
    let x0 = int_to_scalar(10);
    let y0 = int_to_scalar(10);
    let mut builder = PathBuilder::new();
    builder.move_to((x0, y0));
    builder.quad_to((x0, y0 + 400.0 * 1.0), (x0 + 600.0 * 1.0, y0 + 400.0 * 1.0));
    builder.detach()
}

/// `static SkPath make_cubic()`.
// Port of: bench/DashBench.cpp#L168-L177 (chrome/m156)
fn make_cubic() -> Path {
    let x0 = int_to_scalar(10);
    let y0 = int_to_scalar(10);
    let mut builder = PathBuilder::new();
    builder.move_to((x0, y0));
    builder.cubic_to(
        (x0, y0 + 400.0 * 1.0),
        (x0 + 600.0 * 1.0, y0 + 400.0 * 1.0),
        (x0 + 600.0 * 1.0, y0),
    );
    builder.detach()
}

/// `class MakeDashBench`: `filterPath` of a fixed path with a 4-on, 4-off dash, no canvas.
// Port of: bench/DashBench.cpp#L179-L213 (chrome/m156)
struct MakeDashBench {
    name: String,
    path: Path,
    pe: PathEffect,
}

impl MakeDashBench {
    fn new(proc_: fn() -> Path, name: &str) -> Self {
        // fName.printf("makedash_%s", name);
        let vals = [int_to_scalar(4), int_to_scalar(4)];
        Self {
            name: format!("makedash_{name}"),
            path: proc_(),
            pe: dash_path_effect::new(&vals, 0.0).expect("valid dash intervals"),
        }
    }
}

impl Benchmark for MakeDashBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let mut dst = PathBuilder::new();
        for _ in 0..loops {
            let mut rec = StrokeRec::new(InitStyle::Hairline);
            // fPE->filterPath(&dst, fPath, &rec);
            self.pe
                .filter_path_inplace(&mut dst, &self.path, &mut rec, None);
            dst.reset();
        }
    }
}

// Port of: bench/DashBench.cpp#L453-L453 (chrome/m156)
def_bench!(
    make_dash_bench_poly = "MakeDashBench(make_poly, \"poly\")",
    MakeDashBench::new(make_poly, "poly")
);
// Port of: bench/DashBench.cpp#L454-L454 (chrome/m156)
def_bench!(
    make_dash_bench_quad = "MakeDashBench(make_quad, \"quad\")",
    MakeDashBench::new(make_quad, "quad")
);
// Port of: bench/DashBench.cpp#L455-L455 (chrome/m156)
def_bench!(
    make_dash_bench_cubic = "MakeDashBench(make_cubic, \"cubic\")",
    MakeDashBench::new(make_cubic, "cubic")
);

/// `class DashLineBench`: a dashed line with square or round caps (`dashline_%g_%s`).
///
/// The C++ special cases square dashes whose intervals equal the stroke width only in comments.
// Port of: bench/DashBench.cpp#L215-L250 (chrome/m156)
struct DashLineBench {
    name: String,
    stroke_width: scalar,
    is_round: bool,
    pe: PathEffect,
}

impl DashLineBench {
    fn new(width: scalar, is_round: bool) -> Self {
        // fName.printf("dashline_%g_%s", width, isRound ? "circle" : "square");
        let vals = [1.0, 1.0]; // SK_Scalar1, SK_Scalar1
        Self {
            name: format!(
                "dashline_{width}_{}",
                if is_round { "circle" } else { "square" }
            ),
            stroke_width: width,
            is_round,
            pe: dash_path_effect::new(&vals, 0.0).expect("valid dash intervals"),
        }
    }
}

impl Benchmark for DashLineBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("DashLineBench is a rendering bench");
        let mut paint = Paint::default();
        self.setup_paint(&mut paint);
        paint.set_stroke_width(self.stroke_width);
        paint.set_stroke_cap(if self.is_round {
            Cap::Round
        } else {
            Cap::Square
        });
        paint.set_path_effect(self.pe.clone());
        for _ in 0..loops {
            canvas.draw_line((10.0, 10.0), (640.0, 10.0), &paint);
        }
    }
}

// Port of: bench/DashBench.cpp#L456-L456 (chrome/m156)
def_bench!(
    dash_line_bench_0_false = "DashLineBench(0, false)",
    DashLineBench::new(0.0, false)
);
// Port of: bench/DashBench.cpp#L457-L457 (chrome/m156)
def_bench!(
    dash_line_bench_1_false = "DashLineBench(SK_Scalar1, false)",
    DashLineBench::new(1.0, false)
);
// Port of: bench/DashBench.cpp#L458-L458 (chrome/m156)
def_bench!(
    dash_line_bench_2_false = "DashLineBench(2 * SK_Scalar1, false)",
    DashLineBench::new(2.0, false)
);
// Port of: bench/DashBench.cpp#L459-L459 (chrome/m156)
def_bench!(
    dash_line_bench_0_true = "DashLineBench(0, true)",
    DashLineBench::new(0.0, true)
);
// Port of: bench/DashBench.cpp#L460-L460 (chrome/m156)
def_bench!(
    dash_line_bench_1_true = "DashLineBench(SK_Scalar1, true)",
    DashLineBench::new(1.0, true)
);
// Port of: bench/DashBench.cpp#L461-L461 (chrome/m156)
def_bench!(
    dash_line_bench_2_true = "DashLineBench(2 * SK_Scalar1, true)",
    DashLineBench::new(2.0, true)
);

/// `class DrawPointsDashingBench`: `drawPoints` of one dashed line that moves down the canvas.
// Port of: bench/DashBench.cpp#L252-L298 (chrome/m156)
struct DrawPointsDashingBench {
    name: String,
    stroke_width: i32,
    do_aa: bool,
    path_effect: PathEffect,
}

impl DrawPointsDashingBench {
    fn new(dash_length: i32, stroke_width: i32, do_aa: bool) -> Self {
        // fName.printf("drawpointsdash_%d_%d%s", dashLength, strokeWidth, doAA ? "_aa" : "_bw");
        let vals = [int_to_scalar(dash_length), int_to_scalar(dash_length)];
        Self {
            name: format!(
                "drawpointsdash_{dash_length}_{stroke_width}{}",
                if do_aa { "_aa" } else { "_bw" }
            ),
            stroke_width,
            do_aa,
            path_effect: dash_path_effect::new(&vals, 1.0).expect("valid dash intervals"),
        }
    }
}

impl Benchmark for DrawPointsDashingBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("DrawPointsDashingBench is a rendering bench");
        let mut p = Paint::default();
        self.setup_paint(&mut p);
        p.set_color(Color::new(0xFF00_0000)); // SK_ColorBLACK
        p.set_style(Style::Stroke);
        p.set_stroke_width(int_to_scalar(self.stroke_width));
        p.set_path_effect(self.path_effect.clone());
        p.set_anti_alias(self.do_aa);
        let mut pts = [
            Point::new(int_to_scalar(10), 0.0),
            Point::new(int_to_scalar(640), 0.0),
        ];
        for i in 0..loops {
            // pts[0].fY = pts[1].fY = SkIntToScalar(i % 480);
            let y = int_to_scalar(i % 480);
            pts[0].y = y;
            pts[1].y = y;
            canvas.draw_points(PointMode::Lines, &pts, &p);
        }
    }
}

// Port of: bench/DashBench.cpp#L463-L463 (chrome/m156)
def_bench!(
    draw_points_dashing_bench_1_1_false = "DrawPointsDashingBench(1, 1, false)",
    DrawPointsDashingBench::new(1, 1, false)
);
// Port of: bench/DashBench.cpp#L464-L464 (chrome/m156)
def_bench!(
    draw_points_dashing_bench_1_1_true = "DrawPointsDashingBench(1, 1, true)",
    DrawPointsDashingBench::new(1, 1, true)
);
// Port of: bench/DashBench.cpp#L465-L465 (chrome/m156)
def_bench!(
    draw_points_dashing_bench_3_1_false = "DrawPointsDashingBench(3, 1, false)",
    DrawPointsDashingBench::new(3, 1, false)
);
// Port of: bench/DashBench.cpp#L466-L466 (chrome/m156)
def_bench!(
    draw_points_dashing_bench_3_1_true = "DrawPointsDashingBench(3, 1, true)",
    DrawPointsDashingBench::new(3, 1, true)
);
// Port of: bench/DashBench.cpp#L467-L467 (chrome/m156)
def_bench!(
    draw_points_dashing_bench_5_5_false = "DrawPointsDashingBench(5, 5, false)",
    DrawPointsDashingBench::new(5, 5, false)
);
// Port of: bench/DashBench.cpp#L468-L468 (chrome/m156)
def_bench!(
    draw_points_dashing_bench_5_5_true = "DrawPointsDashingBench(5, 5, true)",
    DrawPointsDashingBench::new(5, 5, true)
);

/// `GiantDashBench::LineType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineType {
    Hori,
    Vert,
    Diag,
}

impl LineType {
    /// `LineTypeName`.
    fn name(self) -> &'static str {
        match self {
            LineType::Hori => "hori",
            LineType::Vert => "vert",
            LineType::Diag => "diag",
        }
    }
}

/// `class GiantDashBench`: a dashed line far longer than the canvas, of which 99% is clipped.
// Port of: bench/DashBench.cpp#L300-L376 (chrome/m156)
struct GiantDashBench {
    name: String,
    stroke_width: scalar,
    pts: [Point; 2],
    path_effect: PathEffect,
}

impl GiantDashBench {
    fn new(lt: LineType, width: scalar) -> Self {
        // fName.printf("giantdashline_%s_%g", LineTypeName(lt), width);
        let name = format!("giantdashline_{}_{width}", lt.name());
        // Deliberately pick intervals that won't be caught by asPoints(), so we can test the
        // filterPath code-path.
        let intervals = [20.0, 10.0, 10.0, 10.0];
        let path_effect = dash_path_effect::new(&intervals, 0.0).expect("valid dash intervals");

        let cx = 640.0 / 2.0; // center X (int division in C++)
        let cy = 480.0 / 2.0; // center Y (int division in C++)
        let mut matrix = Matrix::default();
        match lt {
            LineType::Hori => {
                matrix.set_identity();
            }
            LineType::Vert => {
                matrix.set_rotate(90.0, Point::new(cx, cy));
            }
            LineType::Diag => {
                matrix.set_rotate(45.0, Point::new(cx, cy));
            }
        }
        let overshoot: scalar = 100.0 * 1000.0;
        let pts = [
            Point::new(-overshoot, cy),
            Point::new(640.0 + overshoot, cy),
        ];
        // matrix.mapPoints(fPts, pts);
        let fpts = [matrix.map_point(pts[0]), matrix.map_point(pts[1])];
        Self {
            name,
            stroke_width: width,
            pts: fpts,
            path_effect,
        }
    }
}

impl Benchmark for GiantDashBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("GiantDashBench is a rendering bench");
        let mut p = Paint::default();
        self.setup_paint(&mut p);
        p.set_style(Style::Stroke);
        p.set_stroke_width(self.stroke_width);
        p.set_path_effect(self.path_effect.clone());
        for _ in 0..loops {
            canvas.draw_points(PointMode::Lines, &self.pts, &p);
        }
    }
}

// Port of: bench/DashBench.cpp#L474-L474 (chrome/m156)
def_bench!(
    giant_dash_bench_hori_0 = "GiantDashBench(GiantDashBench::kHori_LineType, 0)",
    GiantDashBench::new(LineType::Hori, 0.0)
);
// Port of: bench/DashBench.cpp#L475-L475 (chrome/m156)
def_bench!(
    giant_dash_bench_vert_0 = "GiantDashBench(GiantDashBench::kVert_LineType, 0)",
    GiantDashBench::new(LineType::Vert, 0.0)
);
// Port of: bench/DashBench.cpp#L476-L476 (chrome/m156)
def_bench!(
    giant_dash_bench_diag_0 = "GiantDashBench(GiantDashBench::kDiag_LineType, 0)",
    GiantDashBench::new(LineType::Diag, 0.0)
);
// pass 2 to explicitly avoid any 1-is-the-same-as-hairline special casing
// hori_2 is just too slow to enable at the moment (C++ comment).
// Port of: bench/DashBench.cpp#L481-L481 (chrome/m156)
def_bench!(
    giant_dash_bench_hori_2 = "GiantDashBench(GiantDashBench::kHori_LineType, 2)",
    GiantDashBench::new(LineType::Hori, 2.0)
);
// Port of: bench/DashBench.cpp#L482-L482 (chrome/m156)
def_bench!(
    giant_dash_bench_vert_2 = "GiantDashBench(GiantDashBench::kVert_LineType, 2)",
    GiantDashBench::new(LineType::Vert, 2.0)
);
// Port of: bench/DashBench.cpp#L483-L483 (chrome/m156)
def_bench!(
    giant_dash_bench_diag_2 = "GiantDashBench(GiantDashBench::kDiag_LineType, 2)",
    GiantDashBench::new(LineType::Diag, 2.0)
);

/// `class DashGridBench`: a spreadsheet-like grid of short dashed lines, alternating horizontal
/// and vertical.
// Port of: bench/DashBench.cpp#L378-L443 (chrome/m156)
struct DashGridBench {
    name: String,
    stroke_width: i32,
    do_aa: bool,
    path_effect: PathEffect,
}

impl DashGridBench {
    fn new(dash_length: i32, stroke_width: i32, do_aa: bool) -> Self {
        // fName.printf("dashgrid_%d_%d%s", dashLength, strokeWidth, doAA ? "_aa" : "_bw");
        let vals = [int_to_scalar(dash_length), int_to_scalar(dash_length)];
        Self {
            name: format!(
                "dashgrid_{dash_length}_{stroke_width}{}",
                if do_aa { "_aa" } else { "_bw" }
            ),
            stroke_width,
            do_aa,
            path_effect: dash_path_effect::new(&vals, 1.0).expect("valid dash intervals"),
        }
    }
}

impl Benchmark for DashGridBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("DashGridBench is a rendering bench");
        let mut p = Paint::default();
        self.setup_paint(&mut p);
        p.set_color(Color::new(0xFF00_0000)); // SK_ColorBLACK
        p.set_style(Style::Stroke);
        p.set_stroke_width(int_to_scalar(self.stroke_width));
        p.set_path_effect(self.path_effect.clone());
        p.set_anti_alias(self.do_aa);
        let pts = [
            Point::new(int_to_scalar(0), 20.5),
            Point::new(int_to_scalar(20), 20.5),
            Point::new(20.5, int_to_scalar(0)),
            Point::new(20.5, int_to_scalar(20)),
        ];
        for _ in 0..loops {
            for j in 0..10 {
                for k in 0..10 {
                    let kf = int_to_scalar(k) * 22.0;
                    let jf = int_to_scalar(j) * 22.0;
                    // Horizontal line.
                    let hor_pts = [
                        Point::new(pts[0].x + kf, pts[0].y + jf),
                        Point::new(pts[1].x + kf, pts[1].y + jf),
                    ];
                    canvas.draw_points(PointMode::Lines, &hor_pts, &p);
                    // Vertical line.
                    let vert_pts = [
                        Point::new(pts[2].x + kf, pts[2].y + jf),
                        Point::new(pts[3].x + kf, pts[3].y + jf),
                    ];
                    canvas.draw_points(PointMode::Lines, &vert_pts, &p);
                }
            }
        }
    }
}

// Port of: bench/DashBench.cpp#L485-L485 (chrome/m156)
def_bench!(
    dash_grid_bench_1_1_true = "DashGridBench(1, 1, true)",
    DashGridBench::new(1, 1, true)
);
// Port of: bench/DashBench.cpp#L486-L486 (chrome/m156)
def_bench!(
    dash_grid_bench_1_1_false = "DashGridBench(1, 1, false)",
    DashGridBench::new(1, 1, false)
);
// Port of: bench/DashBench.cpp#L487-L487 (chrome/m156)
def_bench!(
    dash_grid_bench_3_1_true = "DashGridBench(3, 1, true)",
    DashGridBench::new(3, 1, true)
);
// Port of: bench/DashBench.cpp#L488-L488 (chrome/m156)
def_bench!(
    dash_grid_bench_3_1_false = "DashGridBench(3, 1, false)",
    DashGridBench::new(3, 1, false)
);
