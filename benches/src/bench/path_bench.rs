// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/PathBench.cpp

//! Path construction, transformation, copying, equality, bounds, rect detection and convexity
//! benches: stroked and filled shapes, random verb streams, `SkPathBuilder` and `SkPathData`
//! factories, and the conic helpers that are still registered.
//!
//! The C++ `complexity()` virtuals are metadata that nanobench does not use for timing, so they
//! are not ported. The four `ConicBench_*` registrations that are commented out in Skia
//! (`ConicBench_Chop5`, `ConicBench_ComputeError`, `ConicBench_asQuadTol`,
//! `ConicBench_quadPow2`) are not registered; their manifest entries stay `todo`.

// The int-to-scalar casts and the `int` index arithmetic mirror the C++ code they port.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]

use std::sync::Arc;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::geometry::Conic;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::matrix_priv::compute_res_scale_for_stroking;
use skia_rust_core::paint::{Join, Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_data::PathData;
use skia_rust_core::path_enums::PathConvexity;
use skia_rust_core::path_priv::set_convexity;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::path_utils::fill_path_with_paint;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::scalar::{scalar, scalar_cos};

use crate::def_bench;
use crate::prelude::*;

/// `enum Flags`: `kStroke_Flag` and `kBig_Flag`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Flags {
    stroke: bool,
    big: bool,
}

// Port of: bench/PathBench.cpp#L30-L40 (chrome/m156)
const FLAGS00: Flags = Flags {
    stroke: false,
    big: false,
};
const FLAGS01: Flags = Flags {
    stroke: true,
    big: false,
};
const FLAGS10: Flags = Flags {
    stroke: false,
    big: true,
};
const FLAGS11: Flags = Flags {
    stroke: true,
    big: true,
};

/// The `appendName`, `makePath` and `setupPaint` overrides of the `PathBench` subclasses.
trait PathMaker {
    /// `appendName(SkString*)`.
    fn append_name(&self) -> String;
    /// `makePath()`.
    fn make_path(&self) -> Path;
    /// `setupPaint(SkPaint*)`: the default is `Benchmark::setupPaint` (anti-aliasing on).
    fn setup_paint(&self, paint: &mut Paint) {
        paint.set_anti_alias(true);
    }
}

/// `class PathBench`: a fill or stroke of one shape, optionally scaled 10x.
// Port of: bench/PathBench.cpp#L42-L83 (chrome/m156)
struct PathBench<M: PathMaker> {
    paint: Paint,
    flags: Flags,
    maker: M,
}

impl<M: PathMaker> PathBench<M> {
    // Port of: bench/PathBench.cpp#L47-L52 (chrome/m156)
    fn new(flags: Flags, maker: M) -> Self {
        let mut paint = Paint::default();
        paint.set_style(if flags.stroke {
            Style::Stroke
        } else {
            Style::Fill
        });
        paint.set_stroke_width(5.0);
        paint.set_stroke_join(Join::Bevel);
        Self {
            paint,
            flags,
            maker,
        }
    }
}

impl<M: PathMaker> Benchmark for PathBench<M> {
    // Port of: bench/PathBench.cpp#L59-L65 (chrome/m156)
    fn name(&self) -> String {
        format!(
            "path_{}_{}_{}",
            if self.flags.stroke { "stroke" } else { "fill" },
            if self.flags.big { "big" } else { "small" },
            self.maker.append_name()
        )
    }

    // Port of: bench/PathBench.cpp#L67-L79 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("PathBench is a rendering bench");
        let mut paint = self.paint.clone();
        self.maker.setup_paint(&mut paint);

        let mut path = self.maker.make_path();
        if self.flags.big {
            path = path.make_transform(&Matrix::scale((10.0, 10.0)));
        }

        for _ in 0..loops {
            canvas.draw_path(&path, &paint);
        }
    }
}

/// `TrianglePathBench`.
// Port of: bench/PathBench.cpp#L85-L100 (chrome/m156)
struct TrianglePath;

impl PathMaker for TrianglePath {
    fn append_name(&self) -> String {
        "triangle".to_owned()
    }

    fn make_path(&self) -> Path {
        let pts = [
            Point::new(10.0, 10.0),
            Point::new(15.0, 5.0),
            Point::new(20.0, 20.0),
        ];
        Path::polygon(&pts, true, None, None)
    }
}

/// `RectPathBench`.
// Port of: bench/PathBench.cpp#L102-L114 (chrome/m156)
struct RectPath;

impl PathMaker for RectPath {
    fn append_name(&self) -> String {
        "rect".to_owned()
    }

    fn make_path(&self) -> Path {
        Path::rect(
            Rect::from_ltrb(10.0, 10.0, 20.0, 20.0),
            None::<PathDirection>,
        )
    }
}

/// `RotatedRectBench`: a rect rotated by `degrees`, with its own anti-aliasing.
// Port of: bench/PathBench.cpp#L116-L142 (chrome/m156)
struct RotatedRectPath {
    aa: bool,
    degrees: scalar,
}

impl PathMaker for RotatedRectPath {
    // Port of: bench/PathBench.cpp#L123-L127 (chrome/m156)
    fn append_name(&self) -> String {
        format!(
            "rotated_rect_{}_{}",
            if self.aa { "aa" } else { "noaa" },
            self.degrees
        )
    }

    // Port of: bench/PathBench.cpp#L129-L132 (chrome/m156)
    fn make_path(&self) -> Path {
        let path = Path::rect(
            Rect::from_ltrb(10.0, 10.0, 20.0, 20.0),
            None::<PathDirection>,
        );
        path.make_transform(&Matrix::rotate_deg(self.degrees))
    }

    // Port of: bench/PathBench.cpp#L134-L137 (chrome/m156)
    fn setup_paint(&self, paint: &mut Paint) {
        paint.set_anti_alias(true);
        paint.set_anti_alias(self.aa);
    }
}

/// `OvalPathBench`.
// Port of: bench/PathBench.cpp#L144-L156 (chrome/m156)
struct OvalPath;

impl PathMaker for OvalPath {
    fn append_name(&self) -> String {
        "oval".to_owned()
    }

    fn make_path(&self) -> Path {
        Path::oval(
            Rect::from_ltrb(10.0, 10.0, 23.0, 20.0),
            None::<PathDirection>,
        )
    }
}

/// `CirclePathBench`.
// Port of: bench/PathBench.cpp#L158-L170 (chrome/m156)
struct CirclePath;

impl PathMaker for CirclePath {
    fn append_name(&self) -> String {
        "circle".to_owned()
    }

    fn make_path(&self) -> Path {
        Path::circle((20.0, 20.0), 10.0, None::<PathDirection>)
    }
}

/// `NonAACirclePathBench`: a circle with anti-aliasing off.
// Port of: bench/PathBench.cpp#L172-L187 (chrome/m156)
struct NonAACirclePath;

impl PathMaker for NonAACirclePath {
    fn append_name(&self) -> String {
        "nonaacircle".to_owned()
    }

    fn make_path(&self) -> Path {
        CirclePath.make_path()
    }

    // Port of: bench/PathBench.cpp#L180-L183 (chrome/m156)
    fn setup_paint(&self, paint: &mut Paint) {
        paint.set_anti_alias(true);
        paint.set_anti_alias(false);
    }
}

/// `AAAConcavePathBench`: max speedup of analytic AA for concave paths.
// Port of: bench/PathBench.cpp#L189-L207 (chrome/m156)
struct AaaConcavePath;

impl PathMaker for AaaConcavePath {
    fn append_name(&self) -> String {
        "concave_aaa".to_owned()
    }

    fn make_path(&self) -> Path {
        let pts = [
            Point::new(10.0, 10.0),
            Point::new(15.0, 10.0),
            Point::new(15.0, 5.0),
            Point::new(40.0, 40.0),
        ];
        Path::polygon(&pts, true, None, None)
    }
}

/// `AAAConvexPathBench`: max speedup of analytic AA for convex paths.
// Port of: bench/PathBench.cpp#L209-L227 (chrome/m156)
struct AaaConvexPath;

impl PathMaker for AaaConvexPath {
    fn append_name(&self) -> String {
        "convex_aaa".to_owned()
    }

    fn make_path(&self) -> Path {
        let pts = [
            Point::new(10.0, 10.0),
            Point::new(15.0, 10.0),
            Point::new(40.0, 50.0),
        ];
        Path::polygon(&pts, true, None, None)
    }
}

/// `SawToothPathBench`.
// Port of: bench/PathBench.cpp#L229-L259 (chrome/m156)
struct SawToothPath;

impl PathMaker for SawToothPath {
    fn append_name(&self) -> String {
        "sawtooth".to_owned()
    }

    fn make_path(&self) -> Path {
        let mut x: scalar = 20.0;
        let y: scalar = 20.0;
        let x0 = x;
        let dx: scalar = 5.0; // SK_Scalar1 * 5
        let dy: scalar = 10.0; // SK_Scalar1 * 10

        let mut builder = PathBuilder::new();
        builder.move_to((x, y));
        for _ in 0..32 {
            x += dx;
            builder.line_to((x, y - dy));
            x += dx;
            builder.line_to((x, y + dy));
        }
        builder.line_to((x, y + 2.0 * dy));
        builder.line_to((x0, y + 2.0 * dy));
        builder.close();
        builder.detach()
    }
}

/// `LongCurvedPathBench`: 100 quads from a generator seeded with 12.
// Port of: bench/PathBench.cpp#L261-L281 (chrome/m156)
struct LongCurvedPath;

impl PathMaker for LongCurvedPath {
    fn append_name(&self) -> String {
        "long_curved".to_owned()
    }

    fn make_path(&self) -> Path {
        let mut rand = Random::new(12);
        let mut builder = PathBuilder::new();
        for _ in 0..100 {
            // The four draws are evaluated left to right.
            let x1 = rand.next_u_scalar1() * 640.0;
            let y1 = rand.next_u_scalar1() * 480.0;
            let x2 = rand.next_u_scalar1() * 640.0;
            let y2 = rand.next_u_scalar1() * 480.0;
            builder.quad_to((x1, y1), (x2, y2));
        }
        builder.close();
        builder.detach()
    }
}

/// `LongLinePathBench`: 100 random line segments.
// Port of: bench/PathBench.cpp#L283-L302 (chrome/m156)
struct LongLinePath;

impl PathMaker for LongLinePath {
    fn append_name(&self) -> String {
        "long_line".to_owned()
    }

    fn make_path(&self) -> Path {
        let mut rand = Random::default();
        let mut builder = PathBuilder::new();
        let x = rand.next_u_scalar1() * 640.0;
        let y = rand.next_u_scalar1() * 480.0;
        builder.move_to((x, y));
        for _ in 1..100 {
            let x = rand.next_u_scalar1() * 640.0;
            let y = rand.next_u_scalar1() * 480.0;
            builder.line_to((x, y));
        }
        builder.detach()
    }
}

// ---------------------------------------------------------------------------------------------

/// `SkPathVerb`, as the random verb stream draws it (`kClose` is never drawn: the range is
/// `[0, kClose)`, but it is kept for the `switch`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verb {
    Move,
    Line,
    Quad,
    Conic,
    Cubic,
    Close,
}

impl Verb {
    fn from_index(i: u32) -> Self {
        match i {
            0 => Verb::Move,
            1 => Verb::Line,
            2 => Verb::Quad,
            3 => Verb::Conic,
            4 => Verb::Cubic,
            _ => Verb::Close,
        }
    }
}

/// `RandomPathBench`'s data and the path it builds from it.
// Port of: bench/PathBench.cpp#L304-L405 (chrome/m156)
struct RandomPaths {
    verb_cnts: Vec<i32>,
    verbs: Vec<Verb>,
    points: Vec<Point>,
    curr_path: usize,
    curr_verb: usize,
    curr_point: usize,
    random: Random,
}

/// `kNumVerbCnts`, `kNumVerbs`, `kNumPoints`: powers of two, so the index masks work.
const NUM_VERB_CNTS: usize = 1 << 5;
const NUM_VERBS: usize = 1 << 5;
const NUM_POINTS: usize = 1 << 5;

impl RandomPaths {
    fn new() -> Self {
        Self {
            verb_cnts: Vec::new(),
            verbs: Vec::new(),
            points: Vec::new(),
            curr_path: 0,
            curr_verb: 0,
            curr_point: 0,
            random: Random::default(),
        }
    }

    // Port of: bench/PathBench.cpp#L311-L336 (chrome/m156)
    fn create_data(&mut self, min_verbs: i32, max_verbs: i32, allow_moves: bool) {
        // The default bounds are {0, 0, 1, 1}.
        let bounds = Rect::from_xywh(0.0, 0.0, 1.0, 1.0);
        self.verb_cnts = (0..NUM_VERB_CNTS)
            .map(|_| {
                self.random
                    .next_range_u(min_verbs as u32, (max_verbs + 1) as u32) as i32
            })
            .collect();
        self.verbs = Vec::with_capacity(NUM_VERBS);
        for _ in 0..NUM_VERBS {
            loop {
                let verb = Verb::from_index(self.random.next_u_less_than(5));
                if allow_moves || verb != Verb::Move {
                    self.verbs.push(verb);
                    break;
                }
            }
        }
        self.points = Vec::with_capacity(NUM_POINTS);
        for _ in 0..NUM_POINTS {
            // The two draws are taken left to right.
            let x = self.random.next_range_scalar(bounds.left, bounds.right);
            let y = self.random.next_range_scalar(bounds.top, bounds.bottom);
            self.points.push(Point::new(x, y));
        }
        self.restart_making_paths();
    }

    // Port of: bench/PathBench.cpp#L338-L342 (chrome/m156)
    fn restart_making_paths(&mut self) {
        self.curr_path = 0;
        self.curr_verb = 0;
        self.curr_point = 0;
    }

    // Port of: bench/PathBench.cpp#L344-L382 (chrome/m156)
    fn make_path(&mut self) -> Path {
        let mut builder = PathBuilder::new();
        let v_count = self.verb_cnts[self.curr_path & (NUM_VERB_CNTS - 1)];
        self.curr_path += 1;
        for _ in 0..v_count {
            let verb = self.verbs[self.curr_verb & (NUM_VERBS - 1)];
            self.curr_verb += 1;
            let p = |i: usize| self.points[i & (NUM_POINTS - 1)];
            match verb {
                Verb::Move => {
                    builder.move_to(p(self.curr_point));
                    self.curr_point += 1;
                }
                Verb::Line => {
                    builder.line_to(p(self.curr_point));
                    self.curr_point += 1;
                }
                Verb::Quad => {
                    builder.quad_to(p(self.curr_point), p(self.curr_point + 1));
                    self.curr_point += 2;
                }
                Verb::Conic => {
                    // SK_ScalarHalf
                    builder.conic_to(p(self.curr_point), p(self.curr_point + 1), 0.5);
                    self.curr_point += 2;
                }
                Verb::Cubic => {
                    builder.cubic_to(
                        p(self.curr_point),
                        p(self.curr_point + 1),
                        p(self.curr_point + 2),
                    );
                    self.curr_point += 3;
                }
                Verb::Close => {
                    builder.close();
                }
            }
        }
        builder.detach()
    }

    // Port of: bench/PathBench.cpp#L384-L388 (chrome/m156)
    fn finished_making_paths(&mut self) {
        self.verb_cnts.clear();
        self.verbs.clear();
        self.points.clear();
    }
}

/// `class PathCreateBench`: `makePath()` over random verbs.
// Port of: bench/PathBench.cpp#L407-L430 (chrome/m156)
struct PathCreateBench {
    data: RandomPaths,
}

impl Benchmark for PathCreateBench {
    fn name(&self) -> String {
        "path_create".to_owned()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/PathBench.cpp#L417-L419 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        self.data.create_data(10, 100, true);
    }

    // Port of: bench/PathBench.cpp#L421-L426 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            // (void)this->makePath()
            std::hint::black_box(self.data.make_path());
        }
        self.data.restart_making_paths();
    }
}

/// `class PathCopyBench`: copies of 32 random paths.
// Port of: bench/PathBench.cpp#L432-L466 (chrome/m156)
struct PathCopyBench {
    data: RandomPaths,
    paths: Vec<Path>,
    copies: Vec<Path>,
}

impl Benchmark for PathCopyBench {
    fn name(&self) -> String {
        "path_copy".to_owned()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/PathBench.cpp#L441-L449 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        self.data.create_data(10, 100, true);
        self.paths = (0..NUM_VERB_CNTS).map(|_| self.data.make_path()).collect();
        self.copies = vec![Path::new(); NUM_VERB_CNTS];
        self.data.finished_making_paths();
    }

    // Port of: bench/PathBench.cpp#L450-L455 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for i in 0..loops {
            let idx = i as usize & (NUM_VERB_CNTS - 1);
            self.copies[idx] = self.paths[idx].clone();
        }
    }
}

/// `class PathEqualityBench`: `==` between a path and every other copy.
// Port of: bench/PathBench.cpp#L595-L632 (chrome/m156)
struct PathEqualityBench {
    data: RandomPaths,
    parity: bool,
    paths: Vec<Path>,
    copies: Vec<Path>,
}

impl Benchmark for PathEqualityBench {
    fn name(&self) -> String {
        "path_equality_50%".to_owned()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/PathBench.cpp#L604-L614 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        self.parity = false;
        self.data.create_data(10, 100, true);
        self.paths = Vec::with_capacity(NUM_VERB_CNTS);
        self.copies = Vec::with_capacity(NUM_VERB_CNTS);
        for _ in 0..NUM_VERB_CNTS {
            let path = self.data.make_path();
            self.copies.push(path.clone());
            self.paths.push(path);
        }
        self.data.finished_making_paths();
    }

    // Port of: bench/PathBench.cpp#L616-L621 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for i in 0..loops {
            let idx = i as usize & (NUM_VERB_CNTS - 1);
            // idx & ~0x1
            self.parity ^= self.paths[idx] == self.copies[idx & !0x1];
        }
    }
}

/// `BenchPathType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BenchPathType {
    Path,
    Builder,
    Data,
}

impl BenchPathType {
    /// `gBenchPathTypeNames`.
    fn name(self) -> &'static str {
        match self {
            BenchPathType::Path => "path",
            BenchPathType::Builder => "builder",
            BenchPathType::Data => "data",
        }
    }
}

/// `class PathTransformBench`: transform, then bounds, of an oval-pair path in three forms.
// Port of: bench/PathBench.cpp#L468-L534 (chrome/m156)
struct PathTransformBench {
    name: String,
    ty: BenchPathType,
    perspective: bool,
    path_src: Path,
    builder_src: PathBuilder,
    p_data: Option<Arc<PathData>>,
    matrix: Matrix,
}

impl PathTransformBench {
    // Port of: bench/PathBench.cpp#L477-L480 (chrome/m156)
    fn new(ty: BenchPathType, perspective: bool) -> Self {
        let mx = if perspective { "persp" } else { "affine" };
        Self {
            name: format!("path_transform_{mx}_{}", ty.name()),
            ty,
            perspective,
            path_src: Path::new(),
            builder_src: PathBuilder::new(),
            p_data: None,
            matrix: Matrix::new_identity(),
        }
    }
}

impl Benchmark for PathTransformBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/PathBench.cpp#L491-L503 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        let r = Rect::from_ltrb(0.0, 0.0, 100.0, 100.0);
        self.builder_src
            .add_oval(r, None::<PathDirection>, None::<usize>);
        self.builder_src.add_oval(
            r.with_inset((10.0, 10.0)),
            None::<PathDirection>,
            None::<usize>,
        );
        self.path_src = self.builder_src.snapshot();
        self.p_data = self.builder_src.snapshot_data();

        if self.perspective {
            self.matrix.set_persp_x(1e-7);
        } else {
            self.matrix.set_scale_x(1.000_001);
        }
    }

    // Port of: bench/PathBench.cpp#L505-L523 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        // We ask for bounds each time, to ensure we're playing fair, as some techniques compute
        // bounds up front after a transform, and some defer it until it is first requested.
        for _ in 0..loops {
            match self.ty {
                BenchPathType::Path => {
                    std::hint::black_box(self.path_src.make_transform(&self.matrix).bounds());
                }
                BenchPathType::Builder => {
                    self.builder_src.transform(&self.matrix);
                    std::hint::black_box(self.builder_src.snapshot().bounds());
                }
                BenchPathType::Data => {
                    let data = self
                        .p_data
                        .as_ref()
                        .expect("SkPathData snapshot")
                        .make_transform(&self.matrix)
                        .expect("SkPathData transform");
                    std::hint::black_box(data.bounds());
                }
            }
        }
    }
}

/// `builder_from_rect`.
// Port of: bench/PathBench.cpp#L536-L538 (chrome/m156)
fn builder_from_rect(r: &RRect) {
    let mut bu = PathBuilder::new();
    bu.add_rect(r.rect(), None::<PathDirection>, None::<usize>);
    std::hint::black_box(bu.detach());
}

/// `builder_from_oval`.
// Port of: bench/PathBench.cpp#L539-L541 (chrome/m156)
fn builder_from_oval(r: &RRect) {
    let mut bu = PathBuilder::new();
    bu.add_oval(r.rect(), None::<PathDirection>, None::<usize>);
    std::hint::black_box(bu.detach());
}

/// `builder_from_rrect`.
// Port of: bench/PathBench.cpp#L542-L544 (chrome/m156)
fn builder_from_rrect(r: &RRect) {
    let mut bu = PathBuilder::new();
    bu.add_rrect(r, None::<PathDirection>, None::<usize>);
    std::hint::black_box(bu.detach());
}

/// `path_from_rect`.
// Port of: bench/PathBench.cpp#L546-L548 (chrome/m156)
fn path_from_rect(r: &RRect) {
    std::hint::black_box(Path::rect(r.rect(), None::<PathDirection>));
}

/// `path_from_oval`.
// Port of: bench/PathBench.cpp#L549-L551 (chrome/m156)
fn path_from_oval(r: &RRect) {
    std::hint::black_box(Path::oval(r.rect(), None::<PathDirection>));
}

/// `path_from_rrect`.
// Port of: bench/PathBench.cpp#L552-L554 (chrome/m156)
fn path_from_rrect(r: &RRect) {
    std::hint::black_box(Path::rrect(r, None::<PathDirection>));
}

/// `pdata_from_rect`: `SkPathData::Rect(r)` with the default direction and start index.
// Port of: bench/PathBench.cpp#L556-L558 (chrome/m156)
fn pdata_from_rect(r: &RRect) {
    std::hint::black_box(PathData::rect(r.rect(), PathDirection::CW, 0));
}

/// `pdata_from_oval`: `SkPathData::Oval(r)`, start index 1.
// Port of: bench/PathBench.cpp#L559-L561 (chrome/m156)
fn pdata_from_oval(r: &RRect) {
    std::hint::black_box(PathData::oval(r.rect(), PathDirection::CW, 1));
}

/// `pdata_from_rrect`: `SkPathData::RRect(r)`, start index 6 for clockwise.
// Port of: bench/PathBench.cpp#L562-L564 (chrome/m156)
fn pdata_from_rrect(r: &RRect) {
    std::hint::black_box(PathData::rrect(r, PathDirection::CW, 6));
}

/// `class PathMakeFromBench`: one factory applied to a fixed round rect.
// Port of: bench/PathBench.cpp#L566-L593 (chrome/m156)
struct PathMakeFromBench {
    name: String,
    maker: fn(&RRect),
}

impl PathMakeFromBench {
    // Port of: bench/PathBench.cpp#L570-L572 (chrome/m156)
    fn new(name: &str, maker: fn(&RRect)) -> Self {
        Self {
            name: format!("pathmaker_{name}"),
            maker,
        }
    }
}

impl Benchmark for PathMakeFromBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/PathBench.cpp#L583-L588 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let rr = RRect::new_rect_xy(Rect::from_ltrb(10.0, 20.0, 30.0, 40.0), 2.0, 3.0);
        for _ in 0..loops {
            (self.maker)(&rr);
        }
    }
}

/// `class CirclesBench`: `drawPath` of the two-arc "circle" Chrome draws.
// Port of: bench/PathBench.cpp#L634-L687 (chrome/m156)
struct CirclesBench {
    stroke: bool,
    name: String,
}

impl CirclesBench {
    // Port of: bench/PathBench.cpp#L640-L642 (chrome/m156)
    fn new(flags: Flags) -> Self {
        Self {
            stroke: flags.stroke,
            name: format!("circles_{}", if flags.stroke { "stroke" } else { "fill" }),
        }
    }
}

impl Benchmark for CirclesBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/PathBench.cpp#L649-L682 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("CirclesBench is a rendering bench");
        let mut paint = Paint::default();

        paint.set_color(Color::BLACK);
        paint.set_anti_alias(true);
        if self.stroke {
            paint.set_style(Style::Stroke);
        }

        let mut rand = Random::default();

        for _ in 0..loops {
            let radius = rand.next_u_scalar1() * 3.0;
            let left = rand.next_u_scalar1() * 300.0;
            let top = rand.next_u_scalar1() * 300.0;
            let r = Rect::from_ltrb(left, top, left + 2.0 * radius, top + 2.0 * radius);

            if self.stroke {
                paint.set_stroke_width(rand.next_u_scalar1() * 5.0);
            }

            // Mimic how Chrome does circles.
            let mut builder = PathBuilder::new();
            builder.arc_to(r, 0.0, 0.0, false);
            builder.add_oval(r, PathDirection::CCW, None::<usize>);
            builder.arc_to(r, 360.0, 0.0, true);
            builder.close();
            let temp = builder.detach();

            canvas.draw_path(&temp, &paint);
        }
    }
}

/// `class ArbRoundRectBench`: round rects with arbitrary corner radii, or zero radii.
// Port of: bench/PathBench.cpp#L689-L792 (chrome/m156)
struct ArbRoundRectBench {
    zero_rad: bool,
    name: String,
}

impl ArbRoundRectBench {
    // Port of: bench/PathBench.cpp#L699-L705 (chrome/m156)
    fn new(zero_rad: bool) -> Self {
        Self {
            zero_rad,
            name: if zero_rad {
                "zeroradroundrect".to_owned()
            } else {
                "arbroundrect".to_owned()
            },
        }
    }
}

/// `add_corner_arc`: one quarter arc of a round rect, placed at a corner.
// Port of: bench/PathBench.cpp#L712-L740 (chrome/m156)
fn add_corner_arc(builder: &mut PathBuilder, rect: &Rect, x_in: scalar, y_in: scalar, start: i32) {
    let rx = rect.width().min(x_in);
    let ry = rect.height().min(y_in);

    let mut arc_rect = Rect::from_ltrb(-rx, -ry, rx, ry);
    match start {
        0 => arc_rect.offset((rect.right - arc_rect.right, rect.bottom - arc_rect.bottom)),
        90 => arc_rect.offset((rect.left - arc_rect.left, rect.bottom - arc_rect.bottom)),
        180 => arc_rect.offset((rect.left - arc_rect.left, rect.top - arc_rect.top)),
        270 => arc_rect.offset((rect.right - arc_rect.right, rect.top - arc_rect.top)),
        _ => {}
    }

    builder.arc_to(arc_rect, start as scalar, 90.0, false);
}

/// `make_arb_round_rect`: the same x and y radius at every corner.
// Port of: bench/PathBench.cpp#L742-L754 (chrome/m156)
fn make_arb_round_rect(r: &Rect, x_corner: scalar, y_corner: scalar) -> Path {
    let mut builder = PathBuilder::new();
    add_corner_arc(&mut builder, r, x_corner, y_corner, 270);
    add_corner_arc(&mut builder, r, x_corner, y_corner, 0);
    add_corner_arc(&mut builder, r, x_corner, y_corner, 90);
    add_corner_arc(&mut builder, r, x_corner, y_corner, 180);
    builder.close();
    builder.detach()
}

impl Benchmark for ArbRoundRectBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/PathBench.cpp#L756-L785 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("ArbRoundRectBench is a rendering bench");
        let mut rand = Random::default();

        for _ in 0..loops {
            let mut paint = Paint::default();
            paint.set_color(Color::from(0xff00_0000 | rand.next_u()));
            paint.set_anti_alias(true);

            let size = rand.next_u_scalar1() * 30.0;
            if size < 1.0 {
                continue;
            }
            let left = rand.next_u_scalar1() * 300.0;
            let top = rand.next_u_scalar1() * 300.0;
            let r = Rect::from_ltrb(left, top, left + 2.0 * size, top + 2.0 * size);

            let temp = if self.zero_rad {
                make_arb_round_rect(&r, 0.0, 0.0)
            } else {
                make_arb_round_rect(&r, r.width() / 10.0, r.height() / 15.0)
            };

            canvas.draw_path(&temp, &paint);
        }
    }
}

/// `ConservativelyContainsBench::Type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ContainsType {
    Rect,
    RoundRect,
    Oval,
}

/// `class ConservativelyContainsBench`: `conservativelyContainsRect` of 400 random queries.
// Port of: bench/PathBench.cpp#L794-L868 (chrome/m156)
struct ConservativelyContainsBench {
    name: String,
    path: Path,
    parity: bool,
    query_rects: Vec<Rect>,
}

/// `kQueryRectCnt`.
const QUERY_RECT_CNT: usize = 400;

impl ConservativelyContainsBench {
    // Port of: bench/PathBench.cpp#L802-L819 (chrome/m156)
    fn new(ty: ContainsType) -> Self {
        // kBaseRect = XYWH(25, 25, 50, 50); kRRRadii = {5, 10}.
        let base = Rect::from_xywh(25.0, 25.0, 50.0, 50.0);
        let (suffix, path) = match ty {
            ContainsType::Rect => ("rect", Path::rect(base, None::<PathDirection>)),
            ContainsType::RoundRect => (
                "round_rect",
                Path::rrect(RRect::new_rect_xy(base, 5.0, 10.0), None::<PathDirection>),
            ),
            ContainsType::Oval => ("oval", Path::oval(base, None::<PathDirection>)),
        };
        Self {
            name: format!("conservatively_contains_{suffix}"),
            path,
            parity: false,
            query_rects: Vec::new(),
        }
    }
}

impl Benchmark for ConservativelyContainsBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/PathBench.cpp#L837-L851 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        // kQueryMin = {1, 1}, kQueryMax = {40, 40}, kBounds = WH(100, 100).
        let mut rand = Random::default();
        self.query_rects = Vec::with_capacity(QUERY_RECT_CNT);
        for _ in 0..QUERY_RECT_CNT {
            let width = rand.next_range_scalar(1.0, 40.0);
            let height = rand.next_range_scalar(1.0, 40.0);
            let x = rand.next_range_scalar(0.0, 100.0 - width);
            let y = rand.next_range_scalar(0.0, 100.0 - height);
            self.query_rects.push(Rect::from_xywh(x, y, width, height));
        }
    }

    // Port of: bench/PathBench.cpp#L830-L835 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for i in 0..loops {
            let rect = self.query_rects[i as usize % QUERY_RECT_CNT];
            self.parity = self.parity != self.path.conservatively_contains_rect(rect);
        }
    }
}

// ---------------------------------------------------------------------------------------------

/// `class ConicBench_Chop`: `SkConic::chop` of one quarter-circle conic.
// Port of: bench/PathBench.cpp#L874-L900 (chrome/m156)
struct ConicBenchChop {
    name: String,
    rq: Conic,
    dst: [Conic; 2],
}

impl ConicBenchChop {
    // Port of: bench/PathBench.cpp#L879-L884 (chrome/m156)
    fn new() -> Self {
        let rq = Conic::new(
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            // SkScalarCos(SK_ScalarPI/4)
            scalar_cos(std::f32::consts::PI / 4.0),
        );
        Self {
            name: "conic-chop".to_owned(),
            rq,
            dst: [rq, rq],
        }
    }
}

impl Benchmark for ConicBenchChop {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/PathBench.cpp#L893-L897 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            self.rq.chop(&mut self.dst);
        }
    }
}

/// `class ConicBench_EvalPos`: `evalAt(0.4)` 1000 times per loop. Both variants evaluate the
/// position; the C++ V2 returns it, the other writes it through a pointer.
// Port of: bench/PathBench.cpp#L903-L924 (chrome/m156)
struct ConicBenchEvalPos {
    base: ConicBenchChop,
    use_v2: bool,
}

impl ConicBenchEvalPos {
    // Port of: bench/PathBench.cpp#L906-L908 (chrome/m156)
    fn new(use_v2: bool) -> Self {
        let mut base = ConicBenchChop::new();
        base.name = format!("conic-eval-pos{}", i32::from(use_v2));
        Self { base, use_v2 }
    }
}

impl Benchmark for ConicBenchEvalPos {
    fn name(&self) -> String {
        self.base.name.clone()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/PathBench.cpp#L909-L923 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let _ = self.use_v2;
        for _ in 0..loops {
            for _ in 0..1000 {
                self.base.dst[0].pts[0] = self.base.rq.eval_at(0.4);
            }
        }
    }
}

/// `class ConicBench_EvalTan`: `evalTangentAt(0.4)` 1000 times per loop.
// Port of: bench/PathBench.cpp#L928-L949 (chrome/m156)
struct ConicBenchEvalTan {
    base: ConicBenchChop,
    use_v2: bool,
}

impl ConicBenchEvalTan {
    // Port of: bench/PathBench.cpp#L931-L933 (chrome/m156)
    fn new(use_v2: bool) -> Self {
        let mut base = ConicBenchChop::new();
        base.name = format!("conic-eval-tan{}", i32::from(use_v2));
        Self { base, use_v2 }
    }
}

impl Benchmark for ConicBenchEvalTan {
    fn name(&self) -> String {
        self.base.name.clone()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/PathBench.cpp#L934-L948 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let _ = self.use_v2;
        for _ in 0..loops {
            for _ in 0..1000 {
                let t = self.base.rq.eval_tangent_at(0.4);
                self.base.dst[0].pts[0] = Point::new(t.x, t.y);
            }
        }
    }
}

/// `class ConicBench_TinyError`: a stroked cubic under a matrix with a tiny error threshold.
// Port of: bench/PathBench.cpp#L953-L989 (chrome/m156)
struct ConicBenchTinyError {
    name: String,
}

impl Benchmark for ConicBenchTinyError {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/PathBench.cpp#L963-L985 (chrome/m156)
    // The matrix literals are the C++ `float` constants verbatim; clippy's shorter spelling would
    // round to the same f32, but the constants are kept as written in the C++.
    #[allow(clippy::excessive_precision)]
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let mut paint = Paint::default();
        paint.set_color(Color::BLACK);
        paint.set_anti_alias(true);
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(2.0);

        let path = PathBuilder::new()
            .move_to((-100.0, 1.0))
            .cubic_to((-101.0, 1.0), (-118.0, -47.0), (-138.0, -44.0))
            .detach();

        // The large y scale factor produces a tiny error threshold.
        let mtx = Matrix::new_all(
            3.072_940_35,
            0.833_333_373,
            361.111_115,
            0.0,
            6_222_222.5,
            28_333.334,
            0.0,
            0.0,
            1.0,
        );
        let scale = compute_res_scale_for_stroking(&mtx);
        let mx = Matrix::scale((scale, scale));

        for _ in 0..loops {
            let mut dst = PathBuilder::new();
            fill_path_with_paint(&path, &paint, &mut dst, None::<&Rect>, Some(mx.clone()));
        }
    }
}

/// `class TightBoundsBench`: `proc(path)` of a 500-verb path, 100 times per loop.
// Port of: bench/PathBench.cpp#L1094-L1133 (chrome/m156)
struct TightBoundsBench {
    path: Path,
    name: String,
    proc_: fn(&Path) -> Rect,
}

impl TightBoundsBench {
    // Port of: bench/PathBench.cpp#L1100-L1116 (chrome/m156)
    fn new(proc_: fn(&Path) -> Rect, suffix: &str) -> Self {
        let mut builder = PathBuilder::new();
        let mut rand = Random::default();
        for _ in 0..100 {
            // Each call's arguments are drawn left to right.
            builder.move_to((rand.next_f() * 100.0, rand.next_f() * 100.0));
            builder.line_to((rand.next_f() * 100.0, rand.next_f() * 100.0));
            builder.quad_to(
                (rand.next_f() * 100.0, rand.next_f() * 100.0),
                (rand.next_f() * 100.0, rand.next_f() * 100.0),
            );
            let p1 = (rand.next_f() * 100.0, rand.next_f() * 100.0);
            let p2 = (rand.next_f() * 100.0, rand.next_f() * 100.0);
            builder.conic_to(p1, p2, rand.next_f() * 10.0);
            builder.cubic_to(
                (rand.next_f() * 100.0, rand.next_f() * 100.0),
                (rand.next_f() * 100.0, rand.next_f() * 100.0),
                (rand.next_f() * 100.0, rand.next_f() * 100.0),
            );
        }
        Self {
            path: builder.detach(),
            name: format!("tight_bounds_{suffix}"),
            proc_,
        }
    }
}

impl Benchmark for TightBoundsBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/PathBench.cpp#L1125-L1129 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops * 100 {
            std::hint::black_box((self.proc_)(&self.path));
        }
    }
}

/// `class CommonConvexBench`: a convex round rect, optionally forced concave, drawn 100 times.
// Port of: bench/PathBench.cpp#L1232-L1271 (chrome/m156)
struct CommonConvexBench {
    name: String,
    path: Path,
    aa: bool,
}

impl CommonConvexBench {
    // Port of: bench/PathBench.cpp#L1239-L1251 (chrome/m156)
    fn new(w: i32, h: i32, force_concave: bool, aa: bool) -> Self {
        let name = format!(
            "convex_path_{w}_{h}_{}_{}",
            i32::from(force_concave),
            i32::from(aa)
        );
        let r = Rect::from_xywh(10.0, 10.0, w as scalar * 1.0, h as scalar * 1.0);
        let path = Path::rrect(
            RRect::new_rect_xy(r, w as scalar / 8.0, h as scalar / 8.0),
            None::<PathDirection>,
        );
        if force_concave {
            set_convexity(&path, PathConvexity::Concave);
        }
        Self { name, path, aa }
    }
}

impl Benchmark for CommonConvexBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/PathBench.cpp#L1258-L1267 (chrome/m156)
    fn on_draw(&mut self, loops: i32, canvas: Option<&Canvas>) {
        let canvas = canvas.expect("CommonConvexBench is a rendering bench");
        let mut paint = Paint::default();
        paint.set_anti_alias(self.aa);

        for _ in 0..loops {
            for _ in 0..100 {
                canvas.draw_path(&self.path, &paint);
            }
        }
    }
}

/// `class PathBuildBench`: one factory applied to a fixed rect, then its bounds.
// Port of: bench/PathBench.cpp#L1283-L1313 (chrome/m156)
struct PathBuildBench {
    name: String,
    builder: fn(&Rect) -> Path,
}

impl PathBuildBench {
    // Port of: bench/PathBench.cpp#L1290-L1292 (chrome/m156)
    fn new(name: &str, builder: fn(&Rect) -> Path) -> Self {
        Self {
            // The "buider" spelling is Skia's own.
            name: format!("path_buider_{name}"),
            builder,
        }
    }
}

impl Benchmark for PathBuildBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/PathBench.cpp#L1299-L1305 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let r = Rect::from_ltrb(1.0, 2.0, 3.0, 4.0);
        for _ in 0..loops {
            let path = (self.builder)(&r);
            std::hint::black_box(path.bounds());
        }
    }
}

/// `class PathIsRectBench`: `isRect` of a fixed path.
// Port of: bench/PathBench.cpp#L1325-L1355 (chrome/m156)
struct PathIsRectBench {
    name: String,
    path: Path,
}

impl PathIsRectBench {
    // Port of: bench/PathBench.cpp#L1327-L1332 (chrome/m156)
    fn new(name: &str, path: Path) -> Self {
        let this = Self {
            name: format!("path_isrect_{name}"),
            path,
        };
        // SkASSERT_RELEASE(fName.endsWith("norect") == !fPath.isRect(nullptr)), checked in
        // release builds too.
        assert!(
            name.ends_with("norect") == this.path.is_rect().is_none(),
            "path_isrect_{name}: the name and isRect disagree"
        );
        this
    }
}

impl Benchmark for PathIsRectBench {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/PathBench.cpp#L1343-L1350 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            std::hint::black_box(self.path.is_rect());
        }
    }
}

/// `class PathBuilderManyShapes`: 10000 appends of one shape kind to one builder.
// Port of: bench/PathBench.cpp#L1388-L1416 (chrome/m156)
struct PathBuilderManyShapes {
    name: String,
    proc_: fn(&mut PathBuilder),
}

impl PathBuilderManyShapes {
    // Port of: bench/PathBench.cpp#L1391-L1394 (chrome/m156)
    fn new(suffix: &str, proc_: fn(&mut PathBuilder)) -> Self {
        Self {
            name: format!("pathbuilder_many_{suffix}"),
            proc_,
        }
    }
}

impl Benchmark for PathBuilderManyShapes {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    // Port of: bench/PathBench.cpp#L1403-L1411 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        // kManyCount
        const MANY_COUNT: usize = 10_000;
        for _ in 0..loops {
            let mut builder = PathBuilder::new();
            for _ in 0..MANY_COUNT {
                (self.proc_)(&mut builder);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Registrations, in the order of the DEF_BENCH lines.

// Port of: bench/PathBench.cpp#L1141-L1144 (chrome/m156)
def_bench!(
    path_bench_triangle_00 = "TrianglePathBench(FLAGS00)",
    PathBench::new(FLAGS00, TrianglePath)
);
// Port of: bench/PathBench.cpp#L1142-L1142 (chrome/m156)
def_bench!(
    path_bench_triangle_01 = "TrianglePathBench(FLAGS01)",
    PathBench::new(FLAGS01, TrianglePath)
);
// Port of: bench/PathBench.cpp#L1143-L1143 (chrome/m156)
def_bench!(
    path_bench_triangle_10 = "TrianglePathBench(FLAGS10)",
    PathBench::new(FLAGS10, TrianglePath)
);
// Port of: bench/PathBench.cpp#L1144-L1144 (chrome/m156)
def_bench!(
    path_bench_triangle_11 = "TrianglePathBench(FLAGS11)",
    PathBench::new(FLAGS11, TrianglePath)
);

// Port of: bench/PathBench.cpp#L1146-L1149 (chrome/m156)
def_bench!(
    path_bench_rect_00 = "RectPathBench(FLAGS00)",
    PathBench::new(FLAGS00, RectPath)
);
// Port of: bench/PathBench.cpp#L1147-L1147 (chrome/m156)
def_bench!(
    path_bench_rect_01 = "RectPathBench(FLAGS01)",
    PathBench::new(FLAGS01, RectPath)
);
// Port of: bench/PathBench.cpp#L1148-L1148 (chrome/m156)
def_bench!(
    path_bench_rect_10 = "RectPathBench(FLAGS10)",
    PathBench::new(FLAGS10, RectPath)
);
// Port of: bench/PathBench.cpp#L1149-L1149 (chrome/m156)
def_bench!(
    path_bench_rect_11 = "RectPathBench(FLAGS11)",
    PathBench::new(FLAGS11, RectPath)
);

// Port of: bench/PathBench.cpp#L1151-L1154 (chrome/m156)
def_bench!(
    path_bench_rotated_00_noaa = "RotatedRectBench(FLAGS00, false, 45)",
    PathBench::new(
        FLAGS00,
        RotatedRectPath {
            aa: false,
            degrees: 45.0
        }
    )
);
// Port of: bench/PathBench.cpp#L1152-L1152 (chrome/m156)
def_bench!(
    path_bench_rotated_10_noaa = "RotatedRectBench(FLAGS10, false, 45)",
    PathBench::new(
        FLAGS10,
        RotatedRectPath {
            aa: false,
            degrees: 45.0
        }
    )
);
// Port of: bench/PathBench.cpp#L1153-L1153 (chrome/m156)
def_bench!(
    path_bench_rotated_00_aa = "RotatedRectBench(FLAGS00, true, 45)",
    PathBench::new(
        FLAGS00,
        RotatedRectPath {
            aa: true,
            degrees: 45.0
        }
    )
);
// Port of: bench/PathBench.cpp#L1154-L1154 (chrome/m156)
def_bench!(
    path_bench_rotated_10_aa = "RotatedRectBench(FLAGS10, true, 45)",
    PathBench::new(
        FLAGS10,
        RotatedRectPath {
            aa: true,
            degrees: 45.0
        }
    )
);

// Port of: bench/PathBench.cpp#L1156-L1159 (chrome/m156)
def_bench!(
    path_bench_oval_00 = "OvalPathBench(FLAGS00)",
    PathBench::new(FLAGS00, OvalPath)
);
// Port of: bench/PathBench.cpp#L1157-L1157 (chrome/m156)
def_bench!(
    path_bench_oval_01 = "OvalPathBench(FLAGS01)",
    PathBench::new(FLAGS01, OvalPath)
);
// Port of: bench/PathBench.cpp#L1158-L1158 (chrome/m156)
def_bench!(
    path_bench_oval_10 = "OvalPathBench(FLAGS10)",
    PathBench::new(FLAGS10, OvalPath)
);
// Port of: bench/PathBench.cpp#L1159-L1159 (chrome/m156)
def_bench!(
    path_bench_oval_11 = "OvalPathBench(FLAGS11)",
    PathBench::new(FLAGS11, OvalPath)
);

// Port of: bench/PathBench.cpp#L1161-L1164 (chrome/m156)
def_bench!(
    path_bench_circle_00 = "CirclePathBench(FLAGS00)",
    PathBench::new(FLAGS00, CirclePath)
);
// Port of: bench/PathBench.cpp#L1162-L1162 (chrome/m156)
def_bench!(
    path_bench_circle_01 = "CirclePathBench(FLAGS01)",
    PathBench::new(FLAGS01, CirclePath)
);
// Port of: bench/PathBench.cpp#L1163-L1163 (chrome/m156)
def_bench!(
    path_bench_circle_10 = "CirclePathBench(FLAGS10)",
    PathBench::new(FLAGS10, CirclePath)
);
// Port of: bench/PathBench.cpp#L1164-L1164 (chrome/m156)
def_bench!(
    path_bench_circle_11 = "CirclePathBench(FLAGS11)",
    PathBench::new(FLAGS11, CirclePath)
);

// Port of: bench/PathBench.cpp#L1166-L1167 (chrome/m156)
def_bench!(
    path_bench_nonaa_circle_00 = "NonAACirclePathBench(FLAGS00)",
    PathBench::new(FLAGS00, NonAACirclePath)
);
// Port of: bench/PathBench.cpp#L1167-L1167 (chrome/m156)
def_bench!(
    path_bench_nonaa_circle_10 = "NonAACirclePathBench(FLAGS10)",
    PathBench::new(FLAGS10, NonAACirclePath)
);

// Port of: bench/PathBench.cpp#L1169-L1172 (chrome/m156)
def_bench!(
    path_bench_aaa_concave_00 = "AAAConcavePathBench(FLAGS00)",
    PathBench::new(FLAGS00, AaaConcavePath)
);
// Port of: bench/PathBench.cpp#L1170-L1170 (chrome/m156)
def_bench!(
    path_bench_aaa_concave_10 = "AAAConcavePathBench(FLAGS10)",
    PathBench::new(FLAGS10, AaaConcavePath)
);
// Port of: bench/PathBench.cpp#L1171-L1171 (chrome/m156)
def_bench!(
    path_bench_aaa_convex_00 = "AAAConvexPathBench(FLAGS00)",
    PathBench::new(FLAGS00, AaaConvexPath)
);
// Port of: bench/PathBench.cpp#L1172-L1172 (chrome/m156)
def_bench!(
    path_bench_aaa_convex_10 = "AAAConvexPathBench(FLAGS10)",
    PathBench::new(FLAGS10, AaaConvexPath)
);

// Port of: bench/PathBench.cpp#L1174-L1175 (chrome/m156)
def_bench!(
    path_bench_sawtooth_00 = "SawToothPathBench(FLAGS00)",
    PathBench::new(FLAGS00, SawToothPath)
);
// Port of: bench/PathBench.cpp#L1175-L1175 (chrome/m156)
def_bench!(
    path_bench_sawtooth_01 = "SawToothPathBench(FLAGS01)",
    PathBench::new(FLAGS01, SawToothPath)
);

// Port of: bench/PathBench.cpp#L1177-L1178 (chrome/m156)
def_bench!(
    path_bench_long_curved_00 = "LongCurvedPathBench(FLAGS00)",
    PathBench::new(FLAGS00, LongCurvedPath)
);
// Port of: bench/PathBench.cpp#L1178-L1178 (chrome/m156)
def_bench!(
    path_bench_long_curved_01 = "LongCurvedPathBench(FLAGS01)",
    PathBench::new(FLAGS01, LongCurvedPath)
);
// Port of: bench/PathBench.cpp#L1179-L1179 (chrome/m156)
def_bench!(
    path_bench_long_line_00 = "LongLinePathBench(FLAGS00)",
    PathBench::new(FLAGS00, LongLinePath)
);
// Port of: bench/PathBench.cpp#L1180-L1180 (chrome/m156)
def_bench!(
    path_bench_long_line_01 = "LongLinePathBench(FLAGS01)",
    PathBench::new(FLAGS01, LongLinePath)
);

// Port of: bench/PathBench.cpp#L1182-L1182 (chrome/m156)
def_bench!(
    path_bench_path_create = "PathCreateBench()",
    PathCreateBench {
        data: RandomPaths::new()
    }
);
// Port of: bench/PathBench.cpp#L1183-L1183 (chrome/m156)
def_bench!(
    path_bench_path_copy = "PathCopyBench()",
    PathCopyBench {
        data: RandomPaths::new(),
        paths: Vec::new(),
        copies: Vec::new(),
    }
);
// Port of: bench/PathBench.cpp#L1184-L1184 (chrome/m156)
def_bench!(
    path_bench_path_equality = "PathEqualityBench()",
    PathEqualityBench {
        data: RandomPaths::new(),
        parity: false,
        paths: Vec::new(),
        copies: Vec::new(),
    }
);

// Port of: bench/PathBench.cpp#L1186-L1186 (chrome/m156)
def_bench!(
    path_bench_transform_data_persp = "PathTransformBench(BenchPathType::kData, true)",
    PathTransformBench::new(BenchPathType::Data, true)
);
// Port of: bench/PathBench.cpp#L1187-L1187 (chrome/m156)
def_bench!(
    path_bench_transform_builder_persp = "PathTransformBench(BenchPathType::kBuilder, true)",
    PathTransformBench::new(BenchPathType::Builder, true)
);
// Port of: bench/PathBench.cpp#L1188-L1188 (chrome/m156)
def_bench!(
    path_bench_transform_path_persp = "PathTransformBench(BenchPathType::kPath, true)",
    PathTransformBench::new(BenchPathType::Path, true)
);
// Port of: bench/PathBench.cpp#L1190-L1190 (chrome/m156)
def_bench!(
    path_bench_transform_data = "PathTransformBench(BenchPathType::kData, false)",
    PathTransformBench::new(BenchPathType::Data, false)
);
// Port of: bench/PathBench.cpp#L1191-L1191 (chrome/m156)
def_bench!(
    path_bench_transform_builder = "PathTransformBench(BenchPathType::kBuilder, false)",
    PathTransformBench::new(BenchPathType::Builder, false)
);
// Port of: bench/PathBench.cpp#L1192-L1192 (chrome/m156)
def_bench!(
    path_bench_transform_path = "PathTransformBench(BenchPathType::kPath, false)",
    PathTransformBench::new(BenchPathType::Path, false)
);

// MAKEFROM(name) is PathMakeFromBench(#name, name).
// Port of: bench/PathBench.cpp#L1196-L1196 (chrome/m156)
def_bench!(
    path_bench_makefrom_pdata_from_rrect = "MAKEFROM(pdata_from_rrect)",
    PathMakeFromBench::new("pdata_from_rrect", pdata_from_rrect)
);
// Port of: bench/PathBench.cpp#L1197-L1197 (chrome/m156)
def_bench!(
    path_bench_makefrom_builder_from_rrect = "MAKEFROM(builder_from_rrect)",
    PathMakeFromBench::new("builder_from_rrect", builder_from_rrect)
);
// Port of: bench/PathBench.cpp#L1198-L1198 (chrome/m156)
def_bench!(
    path_bench_makefrom_path_from_rrect = "MAKEFROM(path_from_rrect)",
    PathMakeFromBench::new("path_from_rrect", path_from_rrect)
);
// Port of: bench/PathBench.cpp#L1200-L1200 (chrome/m156)
def_bench!(
    path_bench_makefrom_pdata_from_oval = "MAKEFROM(pdata_from_oval)",
    PathMakeFromBench::new("pdata_from_oval", pdata_from_oval)
);
// Port of: bench/PathBench.cpp#L1201-L1201 (chrome/m156)
def_bench!(
    path_bench_makefrom_builder_from_oval = "MAKEFROM(builder_from_oval)",
    PathMakeFromBench::new("builder_from_oval", builder_from_oval)
);
// Port of: bench/PathBench.cpp#L1202-L1202 (chrome/m156)
def_bench!(
    path_bench_makefrom_path_from_oval = "MAKEFROM(path_from_oval)",
    PathMakeFromBench::new("path_from_oval", path_from_oval)
);
// Port of: bench/PathBench.cpp#L1204-L1204 (chrome/m156)
def_bench!(
    path_bench_makefrom_pdata_from_rect = "MAKEFROM(pdata_from_rect)",
    PathMakeFromBench::new("pdata_from_rect", pdata_from_rect)
);
// Port of: bench/PathBench.cpp#L1205-L1205 (chrome/m156)
def_bench!(
    path_bench_makefrom_builder_from_rect = "MAKEFROM(builder_from_rect)",
    PathMakeFromBench::new("builder_from_rect", builder_from_rect)
);
// Port of: bench/PathBench.cpp#L1206-L1206 (chrome/m156)
def_bench!(
    path_bench_makefrom_path_from_rect = "MAKEFROM(path_from_rect)",
    PathMakeFromBench::new("path_from_rect", path_from_rect)
);

// Port of: bench/PathBench.cpp#L1210-L1211 (chrome/m156)
def_bench!(
    path_bench_circles_00 = "CirclesBench(FLAGS00)",
    CirclesBench::new(FLAGS00)
);
// Port of: bench/PathBench.cpp#L1211-L1211 (chrome/m156)
def_bench!(
    path_bench_circles_01 = "CirclesBench(FLAGS01)",
    CirclesBench::new(FLAGS01)
);
// Port of: bench/PathBench.cpp#L1212-L1212 (chrome/m156)
def_bench!(
    path_bench_arb_round_rect_false = "ArbRoundRectBench(false)",
    ArbRoundRectBench::new(false)
);
// Port of: bench/PathBench.cpp#L1213-L1213 (chrome/m156)
def_bench!(
    path_bench_arb_round_rect_true = "ArbRoundRectBench(true)",
    ArbRoundRectBench::new(true)
);
// Port of: bench/PathBench.cpp#L1214-L1214 (chrome/m156)
def_bench!(
    path_bench_conservatively_contains_rect =
        "ConservativelyContainsBench(ConservativelyContainsBench::kRect_Type)",
    ConservativelyContainsBench::new(ContainsType::Rect)
);
// Port of: bench/PathBench.cpp#L1215-L1215 (chrome/m156)
def_bench!(
    path_bench_conservatively_contains_round_rect =
        "ConservativelyContainsBench(ConservativelyContainsBench::kRoundRect_Type)",
    ConservativelyContainsBench::new(ContainsType::RoundRect)
);
// Port of: bench/PathBench.cpp#L1216-L1216 (chrome/m156)
def_bench!(
    path_bench_conservatively_contains_oval =
        "ConservativelyContainsBench(ConservativelyContainsBench::kOval_Type)",
    ConservativelyContainsBench::new(ContainsType::Oval)
);

// Port of: bench/PathBench.cpp#L1221-L1222 (chrome/m156)
def_bench!(
    path_bench_tight_bounds_priv =
        r#"TightBoundsBench([](const SkPath& path){ return path.computeTightBounds();}, "priv")"#,
    TightBoundsBench::new(Path::compute_tight_bounds, "priv")
);

// The ConicBench_Chop5, ConicBench_ComputeError, ConicBench_asQuadTol and ConicBench_quadPow2
// registrations are commented out in Skia (optimized away); they are not registered here.

// Port of: bench/PathBench.cpp#L901-L901 (chrome/m156)
def_bench!(
    path_bench_conic_chop = "ConicBench_Chop",
    ConicBenchChop::new()
);
// Port of: bench/PathBench.cpp#L925-L925 (chrome/m156)
def_bench!(
    path_bench_conic_eval_pos_false = "ConicBench_EvalPos(false)",
    ConicBenchEvalPos::new(false)
);
// Port of: bench/PathBench.cpp#L926-L926 (chrome/m156)
def_bench!(
    path_bench_conic_eval_pos_true = "ConicBench_EvalPos(true)",
    ConicBenchEvalPos::new(true)
);
// Port of: bench/PathBench.cpp#L950-L950 (chrome/m156)
def_bench!(
    path_bench_conic_eval_tan_false = "ConicBench_EvalTan(false)",
    ConicBenchEvalTan::new(false)
);
// Port of: bench/PathBench.cpp#L951-L951 (chrome/m156)
def_bench!(
    path_bench_conic_eval_tan_true = "ConicBench_EvalTan(true)",
    ConicBenchEvalTan::new(true)
);
// Port of: bench/PathBench.cpp#L990-L990 (chrome/m156)
def_bench!(
    path_bench_conic_tiny_error = "ConicBench_TinyError",
    ConicBenchTinyError {
        name: "conic-tinyerror".to_owned()
    }
);

// Port of: bench/PathBench.cpp#L1273-L1276 (chrome/m156)
def_bench!(
    path_bench_convex_16_16_false_false = "CommonConvexBench( 16, 16, false, false)",
    CommonConvexBench::new(16, 16, false, false)
);
// Port of: bench/PathBench.cpp#L1274-L1274 (chrome/m156)
def_bench!(
    path_bench_convex_16_16_true_false = "CommonConvexBench( 16, 16, true, false)",
    CommonConvexBench::new(16, 16, true, false)
);
// Port of: bench/PathBench.cpp#L1275-L1275 (chrome/m156)
def_bench!(
    path_bench_convex_16_16_false_true = "CommonConvexBench( 16, 16, false, true)",
    CommonConvexBench::new(16, 16, false, true)
);
// Port of: bench/PathBench.cpp#L1276-L1276 (chrome/m156)
def_bench!(
    path_bench_convex_16_16_true_true = "CommonConvexBench( 16, 16, true, true)",
    CommonConvexBench::new(16, 16, true, true)
);
// Port of: bench/PathBench.cpp#L1278-L1281 (chrome/m156)
def_bench!(
    path_bench_convex_200_16_false_false = "CommonConvexBench(200, 16, false, false)",
    CommonConvexBench::new(200, 16, false, false)
);
// Port of: bench/PathBench.cpp#L1279-L1279 (chrome/m156)
def_bench!(
    path_bench_convex_200_16_true_false = "CommonConvexBench(200, 16, true, false)",
    CommonConvexBench::new(200, 16, true, false)
);
// Port of: bench/PathBench.cpp#L1280-L1280 (chrome/m156)
def_bench!(
    path_bench_convex_200_16_false_true = "CommonConvexBench(200, 16, false, true)",
    CommonConvexBench::new(200, 16, false, true)
);
// Port of: bench/PathBench.cpp#L1281-L1281 (chrome/m156)
def_bench!(
    path_bench_convex_200_16_true_true = "CommonConvexBench(200, 16, true, true)",
    CommonConvexBench::new(200, 16, true, true)
);

// Port of: bench/PathBench.cpp#L1315-L1317 (chrome/m156)
def_bench!(
    path_bench_build_add_rect =
        r#"PathBuildBench("addRect", [](const SkRect& r) { return SkPath::Rect(r); })"#,
    PathBuildBench::new("addRect", |r| Path::rect(r, None::<PathDirection>))
);
// Port of: bench/PathBench.cpp#L1318-L1320 (chrome/m156)
def_bench!(
    path_bench_build_add_oval =
        r#"PathBuildBench("addOval", [](const SkRect& r) { return SkPath::Oval(r); })"#,
    PathBuildBench::new("addOval", |r| Path::oval(r, None::<PathDirection>))
);
// Port of: bench/PathBench.cpp#L1321-L1323 (chrome/m156)
def_bench!(
    path_bench_build_add_rrect = r#"PathBuildBench("addRRect", [](const SkRect& r) { return SkPath::RRect(SkRRect::MakeRectXY(r, 0.1f, 0.1f)); })"#,
    PathBuildBench::new("addRRect", |r| Path::rrect(
        RRect::new_rect_xy(r, 0.1, 0.1),
        None::<PathDirection>
    ))
);

// Port of: bench/PathBench.cpp#L1357-L1357 (chrome/m156)
def_bench!(
    path_bench_isrect_trivial = r#"PathIsRectBench("trivial", SkPath::Rect({10, 10, 100, 50}))"#,
    PathIsRectBench::new(
        "trivial",
        Path::rect(
            Rect::from_ltrb(10.0, 10.0, 100.0, 50.0),
            None::<PathDirection>
        )
    )
);
// Port of: bench/PathBench.cpp#L1358-L1369 (chrome/m156)
def_bench!(
    path_bench_isrect_complex = r#"PathIsRectBench("complex", SkPathBuilder() .moveTo( 10, 10) .lineTo( 50, 10) .lineTo(100, 10) .lineTo(100, 25) .lineTo(100, 50) .lineTo( 50, 50) .lineTo( 10, 50) .lineTo( 10, 25) .lineTo( 10, 10) .close() .detach())"#,
    PathIsRectBench::new("complex", {
        let mut b = PathBuilder::new();
        b.move_to((10.0, 10.0))
            .line_to((50.0, 10.0))
            .line_to((100.0, 10.0))
            .line_to((100.0, 25.0))
            .line_to((100.0, 50.0))
            .line_to((50.0, 50.0))
            .line_to((10.0, 50.0))
            .line_to((10.0, 25.0))
            .line_to((10.0, 10.0))
            .close();
        b.detach()
    })
);
// Port of: bench/PathBench.cpp#L1370-L1370 (chrome/m156)
def_bench!(
    path_bench_isrect_empty_norect = r#"PathIsRectBench("empty_norect", SkPath())"#,
    PathIsRectBench::new("empty_norect", Path::new())
);
// Port of: bench/PathBench.cpp#L1371-L1383 (chrome/m156)
def_bench!(
    path_bench_isrect_complex_norect = r#"PathIsRectBench("complex_norect", SkPathBuilder() .moveTo( 10, 10) .lineTo( 50, 10) .lineTo(100, 10) .lineTo(100, 25) .lineTo(100, 50) .lineTo( 50, 50) .lineTo( 10, 50) .lineTo( 10, 25) .lineTo( 10, 10) .conicTo(10, 20, 20, 20, .7f) .close() .detach())"#,
    PathIsRectBench::new("complex_norect", {
        let mut b = PathBuilder::new();
        b.move_to((10.0, 10.0))
            .line_to((50.0, 10.0))
            .line_to((100.0, 10.0))
            .line_to((100.0, 25.0))
            .line_to((100.0, 50.0))
            .line_to((50.0, 50.0))
            .line_to((10.0, 50.0))
            .line_to((10.0, 25.0))
            .line_to((10.0, 10.0))
            .conic_to((10.0, 20.0), (20.0, 20.0), 0.7)
            .close();
        b.detach()
    })
);

// Port of: bench/PathBench.cpp#L1418-L1419 (chrome/m156)
def_bench!(
    path_bench_many_rects =
        r#"PathBuilderManyShapes("rects", [](SkPathBuilder* b) { b->addRect({0, 0, 10, 20}); })"#,
    PathBuilderManyShapes::new("rects", |b| {
        b.add_rect(
            Rect::from_ltrb(0.0, 0.0, 10.0, 20.0),
            None::<PathDirection>,
            None::<usize>,
        );
    })
);
// Port of: bench/PathBench.cpp#L1420-L1421 (chrome/m156)
def_bench!(
    path_bench_many_rrects = r#"PathBuilderManyShapes("rrects", [](SkPathBuilder* b) { b->addRRect(SkRRect::MakeRect({0, 0, 10, 20})); })"#,
    PathBuilderManyShapes::new("rrects", |b| {
        b.add_rrect(
            RRect::new_rect(Rect::from_ltrb(0.0, 0.0, 10.0, 20.0)),
            None::<PathDirection>,
            None::<usize>,
        );
    })
);
// Port of: bench/PathBench.cpp#L1422-L1423 (chrome/m156)
def_bench!(
    path_bench_many_ovals =
        r#"PathBuilderManyShapes("ovals", [](SkPathBuilder* b) { b->addOval({0, 0, 10, 20}); })"#,
    PathBuilderManyShapes::new("ovals", |b| {
        b.add_oval(
            Rect::from_ltrb(0.0, 0.0, 10.0, 20.0),
            None::<PathDirection>,
            None::<usize>,
        );
    })
);
// Port of: bench/PathBench.cpp#L1424-L1425 (chrome/m156)
def_bench!(
    path_bench_many_circles =
        r#"PathBuilderManyShapes("circles", [](SkPathBuilder* b) { b->addCircle({10, 10}, 5); })"#,
    PathBuilderManyShapes::new("circles", |b| {
        b.add_circle((10.0, 10.0), 5.0, None::<PathDirection>);
    })
);
