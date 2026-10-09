// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/StrokeBench.cpp

//! `skpathutils::FillPathWithPaint` of random line, quad, conic and cubic paths with a miter,
//! square-capped stroke (a non-rendering bench).

use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Cap, Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_utils::fill_path_with_paint;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;

use crate::def_bench;
use crate::prelude::*;

const N: usize = 100;
const X: scalar = 100.0;
const Y: scalar = 100.0;

/// `class StrokeBench`.
// Port of: bench/StrokeBench.cpp#L13-L51 (chrome/m156)
struct StrokeBench {
    path: Path,
    paint: Paint,
    name: String,
    res: scalar,
}

impl StrokeBench {
    // Port of: bench/StrokeBench.cpp#L16-L22 (chrome/m156)
    fn new(path: Path, paint: Paint, path_type: &str, res: scalar) -> Self {
        // fName.printf("build_stroke_%s_%g_%d_%d", pathType, paint.getStrokeWidth(),
        //              paint.getStrokeJoin(), paint.getStrokeCap());
        let name = format!(
            "build_stroke_{path_type}_{}_{}_{}",
            paint.stroke_width(),
            paint.stroke_join() as i32,
            paint.stroke_cap() as i32
        );
        Self {
            path,
            paint,
            name,
            res,
        }
    }
}

impl Benchmark for StrokeBench {
    // Port of: bench/StrokeBench.cpp#L24-L26 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/StrokeBench.cpp#L30-L41 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let mut paint = self.paint.clone();
        self.setup_paint(&mut paint);

        let mx = Matrix::scale((self.res, self.res));
        for _outer in 0..10 {
            for _ in 0..loops {
                let mut result = PathBuilder::new();
                // SkMatrix is passed by const reference in C++; a copy per call here.
                fill_path_with_paint(
                    &self.path,
                    &paint,
                    &mut result,
                    None::<&Rect>,
                    Some(mx.clone()),
                );
            }
        }
    }
}

// Port of: bench/StrokeBench.cpp#L55-L57 (chrome/m156)
fn rand_pt(rand: &mut Random) -> Point {
    // SkPoint::Make(rand.nextSScalar1() * X, rand.nextSScalar1() * Y): the C++ argument order is
    // unspecified; the two draws are taken left to right here.
    let x = rand.next_s_scalar1() * X;
    let y = rand.next_s_scalar1() * Y;
    Point::new(x, y)
}

// Port of: bench/StrokeBench.cpp#L59-L67 (chrome/m156)
fn line_path_maker() -> Path {
    let mut builder = PathBuilder::new();
    let mut rand = Random::default();
    builder.move_to(rand_pt(&mut rand));
    for _ in 0..N {
        builder.line_to(rand_pt(&mut rand));
    }
    builder.detach()
}

// Port of: bench/StrokeBench.cpp#L68-L76 (chrome/m156)
fn quad_path_maker() -> Path {
    let mut builder = PathBuilder::new();
    let mut rand = Random::default();
    builder.move_to(rand_pt(&mut rand));
    for _ in 0..N {
        let p1 = rand_pt(&mut rand);
        let p2 = rand_pt(&mut rand);
        builder.quad_to(p1, p2);
    }
    builder.detach()
}

// Port of: bench/StrokeBench.cpp#L77-L85 (chrome/m156)
fn conic_path_maker() -> Path {
    let mut builder = PathBuilder::new();
    let mut rand = Random::default();
    builder.move_to(rand_pt(&mut rand));
    for _ in 0..N {
        let p1 = rand_pt(&mut rand);
        let p2 = rand_pt(&mut rand);
        let w = rand.next_u_scalar1();
        builder.conic_to(p1, p2, w);
    }
    builder.detach()
}

// Port of: bench/StrokeBench.cpp#L86-L94 (chrome/m156)
fn cubic_path_maker() -> Path {
    let mut builder = PathBuilder::new();
    let mut rand = Random::default();
    builder.move_to(rand_pt(&mut rand));
    for _ in 0..N {
        let p1 = rand_pt(&mut rand);
        let p2 = rand_pt(&mut rand);
        let p3 = rand_pt(&mut rand);
        builder.cubic_to(p1, p2, p3);
    }
    builder.detach()
}

// Port of: bench/StrokeBench.cpp#L96-L103 (chrome/m156)
fn paint_maker() -> Paint {
    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(X / 10.0);
    paint.set_stroke_join(Join::Miter);
    paint.set_stroke_cap(Cap::Square);
    paint
}

// Port of: bench/StrokeBench.cpp#L109-L122 (chrome/m156)
def_bench!(
    stroke_bench_line_1 = r#"StrokeBench(line_path_maker(), paint_maker(), "line_1", 1)"#,
    StrokeBench::new(line_path_maker(), paint_maker(), "line_1", 1.0)
);
// Port of: bench/StrokeBench.cpp#L110-L110 (chrome/m156)
def_bench!(
    stroke_bench_quad_1 = r#"StrokeBench(quad_path_maker(), paint_maker(), "quad_1", 1)"#,
    StrokeBench::new(quad_path_maker(), paint_maker(), "quad_1", 1.0)
);
// Port of: bench/StrokeBench.cpp#L111-L111 (chrome/m156)
def_bench!(
    stroke_bench_conic_1 = r#"StrokeBench(conic_path_maker(), paint_maker(), "conic_1", 1)"#,
    StrokeBench::new(conic_path_maker(), paint_maker(), "conic_1", 1.0)
);
// Port of: bench/StrokeBench.cpp#L112-L112 (chrome/m156)
def_bench!(
    stroke_bench_cubic_1 = r#"StrokeBench(cubic_path_maker(), paint_maker(), "cubic_1", 1)"#,
    StrokeBench::new(cubic_path_maker(), paint_maker(), "cubic_1", 1.0)
);
// Port of: bench/StrokeBench.cpp#L114-L114 (chrome/m156)
def_bench!(
    stroke_bench_line_4 = r#"StrokeBench(line_path_maker(), paint_maker(), "line_4", 4)"#,
    StrokeBench::new(line_path_maker(), paint_maker(), "line_4", 4.0)
);
// Port of: bench/StrokeBench.cpp#L115-L115 (chrome/m156)
def_bench!(
    stroke_bench_quad_4 = r#"StrokeBench(quad_path_maker(), paint_maker(), "quad_4", 4)"#,
    StrokeBench::new(quad_path_maker(), paint_maker(), "quad_4", 4.0)
);
// Port of: bench/StrokeBench.cpp#L116-L116 (chrome/m156)
def_bench!(
    stroke_bench_conic_4 = r#"StrokeBench(conic_path_maker(), paint_maker(), "conic_4", 4)"#,
    StrokeBench::new(conic_path_maker(), paint_maker(), "conic_4", 4.0)
);
// Port of: bench/StrokeBench.cpp#L117-L117 (chrome/m156)
def_bench!(
    stroke_bench_cubic_4 = r#"StrokeBench(cubic_path_maker(), paint_maker(), "cubic_4", 4)"#,
    StrokeBench::new(cubic_path_maker(), paint_maker(), "cubic_4", 4.0)
);
// Port of: bench/StrokeBench.cpp#L119-L119 (chrome/m156)
def_bench!(
    stroke_bench_line_quarter =
        r#"StrokeBench(line_path_maker(), paint_maker(), "line_.25", .25f)"#,
    StrokeBench::new(line_path_maker(), paint_maker(), "line_.25", 0.25)
);
// Port of: bench/StrokeBench.cpp#L120-L120 (chrome/m156)
def_bench!(
    stroke_bench_quad_quarter =
        r#"StrokeBench(quad_path_maker(), paint_maker(), "quad_.25", .25f)"#,
    StrokeBench::new(quad_path_maker(), paint_maker(), "quad_.25", 0.25)
);
// Port of: bench/StrokeBench.cpp#L121-L121 (chrome/m156)
def_bench!(
    stroke_bench_conic_quarter =
        r#"StrokeBench(conic_path_maker(), paint_maker(), "conic_.25", .25f)"#,
    StrokeBench::new(conic_path_maker(), paint_maker(), "conic_.25", 0.25)
);
// Port of: bench/StrokeBench.cpp#L122-L122 (chrome/m156)
def_bench!(
    stroke_bench_cubic_quarter =
        r#"StrokeBench(cubic_path_maker(), paint_maker(), "cubic_.25", .25f)"#,
    StrokeBench::new(cubic_path_maker(), paint_maker(), "cubic_.25", 0.25)
);
