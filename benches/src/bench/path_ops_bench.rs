// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/PathOpsBench.cpp

//! `PathOpsBench` (two ovals combined with a boolean op), `PathOpsSimplifyBench` (`Simplify` of
//! 20 random rectangles) and `PathBuilderBench` (building a path of 1200 verbs with
//! `SkPathBuilder`, or from prebuilt arrays with `SkPath::Raw`). All non-rendering.

use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::{PathFillType, PathVerb};
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::scalar;
use skia_rust_pathops::path_op::PathOp;
use skia_rust_pathops::{op, simplify};

use crate::def_bench;
use crate::prelude::*;

/// `class PathOpsBench`.
// Port of: bench/PathOpsBench.cpp#L13-L44 (chrome/m156)
struct PathOpsBench {
    /// `fName`.
    name: String,
    path1: Path,
    path2: Path,
    op: PathOp,
}

impl PathOpsBench {
    // Port of: bench/PathOpsBench.cpp#L18-L23 (chrome/m156)
    fn new(suffix: &str, op: PathOp) -> Self {
        Self {
            // fName.printf("pathops_%s", suffix);
            name: format!("pathops_{suffix}"),
            // fPath1 = SkPath::Oval({-10, -20, 10, 20});
            path1: Path::oval(Rect::from_ltrb(-10.0, -20.0, 10.0, 20.0), None),
            // fPath2 = SkPath::Oval({-20, -10, 20, 10});
            path2: Path::oval(Rect::from_ltrb(-20.0, -10.0, 20.0, 10.0), None),
            op,
        }
    }
}

impl Benchmark for PathOpsBench {
    // Port of: bench/PathOpsBench.cpp#L25-L27 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/PathOpsBench.cpp#L33-L41 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            for _ in 0..1000 {
                // std::ignore = Op(fPath1, fPath2, fOp);
                let _ = op(&self.path1, &self.path2, self.op);
            }
        }
    }
}

/// `class PathOpsSimplifyBench`.
// Port of: bench/PathOpsBench.cpp#L46-L72 (chrome/m156)
struct PathOpsSimplifyBench {
    /// `fName`.
    name: String,
    path: Path,
}

impl PathOpsSimplifyBench {
    // Port of: bench/PathOpsBench.cpp#L50-L52 (chrome/m156)
    fn new(suffix: &str, path: Path) -> Self {
        Self {
            // fName.printf("pathops_simplify_%s", suffix);
            name: format!("pathops_simplify_{suffix}"),
            path,
        }
    }
}

impl Benchmark for PathOpsSimplifyBench {
    // Port of: bench/PathOpsBench.cpp#L54-L56 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/PathOpsBench.cpp#L62-L70 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            for _ in 0..100 {
                // std::ignore = Simplify(fPath);
                let _ = simplify(&self.path);
            }
        }
    }
}

/// `makerects()`: 20 rectangles of size 100 at random positions in `[0, 100)`.
// Port of: bench/PathOpsBench.cpp#L81-L93 (chrome/m156)
fn makerects() -> Path {
    // SkRandom rand;
    let mut rand = Random::default();
    let mut builder = PathBuilder::new();
    let scale: scalar = 100.0;
    for _ in 0..20 {
        let x = rand.next_u_scalar1() * scale;
        let y = rand.next_u_scalar1() * scale;
        // builder.addRect({x, y, x + scale, y + scale});
        builder.add_rect(Rect::from_ltrb(x, y, x + scale, y + scale), None, None);
    }
    builder.detach()
}

/// `ArrayPath<N>`: a path written into fixed arrays of points and verbs.
// Port of: bench/PathOpsBench.cpp#L98-L121 (chrome/m156)
struct ArrayPath {
    pts: Vec<Point>,
    vbs: Vec<PathVerb>,
}

impl ArrayPath {
    /// `ArrayPath<N*12>`: room for `N * 12` points and verbs.
    fn new(capacity: usize) -> Self {
        Self {
            pts: Vec::with_capacity(capacity),
            vbs: Vec::with_capacity(capacity),
        }
    }
}

/// The builder calls `run_builder` makes, for `SkPathBuilder` and `ArrayPath` alike.
// Port of: bench/PathOpsBench.cpp#L100-L121 (chrome/m156)
trait RunBuilder {
    fn move_to(&mut self, x: scalar, y: scalar);
    fn line_to(&mut self, x: scalar, y: scalar);
    fn quad_to(&mut self, x: scalar, y: scalar, x1: scalar, y1: scalar);
    fn cubic_to(&mut self, x: scalar, y: scalar, x1: scalar, y1: scalar, x2: scalar, y2: scalar);
    fn inc_reserve(&mut self, extra: i32);
}

impl RunBuilder for ArrayPath {
    // Port of: bench/PathOpsBench.cpp#L102-L105 (chrome/m156)
    fn move_to(&mut self, x: scalar, y: scalar) {
        self.vbs.push(PathVerb::Move);
        self.pts.push(Point::new(x, y));
    }

    // Port of: bench/PathOpsBench.cpp#L106-L109 (chrome/m156)
    fn line_to(&mut self, x: scalar, y: scalar) {
        self.vbs.push(PathVerb::Line);
        self.pts.push(Point::new(x, y));
    }

    // Port of: bench/PathOpsBench.cpp#L110-L114 (chrome/m156)
    fn quad_to(&mut self, x: scalar, y: scalar, x1: scalar, y1: scalar) {
        self.vbs.push(PathVerb::Quad);
        self.pts.push(Point::new(x, y));
        self.pts.push(Point::new(x1, y1));
    }

    // Port of: bench/PathOpsBench.cpp#L115-L121 (chrome/m156)
    fn cubic_to(&mut self, x: scalar, y: scalar, x1: scalar, y1: scalar, x2: scalar, y2: scalar) {
        self.vbs.push(PathVerb::Cubic);
        self.pts.push(Point::new(x, y));
        self.pts.push(Point::new(x1, y1));
        self.pts.push(Point::new(x2, y2));
    }

    // Port of: bench/PathOpsBench.cpp#L122 (chrome/m156)
    fn inc_reserve(&mut self, _extra: i32) {}
}

impl RunBuilder for PathBuilder {
    fn move_to(&mut self, x: scalar, y: scalar) {
        PathBuilder::move_to(self, (x, y));
    }

    fn line_to(&mut self, x: scalar, y: scalar) {
        PathBuilder::line_to(self, (x, y));
    }

    fn quad_to(&mut self, x: scalar, y: scalar, x1: scalar, y1: scalar) {
        PathBuilder::quad_to(self, (x, y), (x1, y1));
    }

    fn cubic_to(&mut self, x: scalar, y: scalar, x1: scalar, y1: scalar, x2: scalar, y2: scalar) {
        PathBuilder::cubic_to(self, (x, y), (x1, y1), (x2, y2));
    }

    /// `incReserve(extraPtCount)`: `incReserve(extraPtCount, extraPtCount, 0)`.
    fn inc_reserve(&mut self, extra: i32) {
        PathBuilder::inc_reserve(self, extra, extra, 0);
    }
}

/// `run_builder(b, useReserve, N)`.
// Port of: bench/PathOpsBench.cpp#L124-L136 (chrome/m156)
fn run_builder<T: RunBuilder>(b: &mut T, use_reserve: bool, n: i32) {
    if use_reserve {
        b.inc_reserve(n * 12);
    }
    let (x, y) = (0.0, 0.0);
    b.move_to(x, y);
    for _ in 1..n {
        b.line_to(x, y);
        b.quad_to(x, y, x, y);
        b.cubic_to(x, y, x, y, x, y);
    }
}

/// `enum class MakeType`.
// Port of: bench/PathOpsBench.cpp#L138-L142 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MakeType {
    Snapshot = 0,
    Detach = 1,
    Array = 2,
}

/// `PathBuilderBench::N`.
const N: i32 = 100;

/// `class PathBuilderBench`.
// Port of: bench/PathOpsBench.cpp#L144-L220 (chrome/m156)
struct PathBuilderBench {
    /// `fName`.
    name: String,
    make_type: MakeType,
    use_reserve: bool,
    /// `fArrays`, filled in `on_delayed_setup`.
    arrays: ArrayPath,
}

impl PathBuilderBench {
    // Port of: bench/PathOpsBench.cpp#L150-L160 (chrome/m156)
    fn new(make_type: MakeType, reserve: bool) -> Self {
        // static constexpr auto typenames = {"path", "snapshot", "detach", "arrays"};
        // Indexed by the enum value, as the C++ is (kSnapshot = 0 names "path").
        const TYPENAMES: [&str; 4] = ["path", "snapshot", "detach", "arrays"];
        Self {
            // fName.printf("makepath_%s_%s", typenames[(int)mt], reserve ? "reserve" : "noreserve");
            name: format!(
                "makepath_{}_{}",
                TYPENAMES[make_type as usize],
                if reserve { "reserve" } else { "noreserve" }
            ),
            make_type,
            use_reserve: reserve,
            arrays: ArrayPath::new(usize::try_from(N * 12).expect("positive")),
        }
    }

    // Port of: bench/PathOpsBench.cpp#L170-L200 (chrome/m156)
    fn build(&self) -> Path {
        match self.make_type {
            MakeType::Snapshot | MakeType::Detach => {
                // SkPathBuilder b;
                let mut b = PathBuilder::new();
                run_builder(&mut b, self.use_reserve, N);
                if self.make_type == MakeType::Snapshot {
                    b.snapshot()
                } else {
                    b.detach()
                }
            }
            // return SkPath::Raw({fArrays.fPts, fPIndex}, {fArrays.fVbs, fVIndex}, {}, kWinding);
            MakeType::Array => Path::raw(
                &self.arrays.pts,
                &self.arrays.vbs,
                &[],
                PathFillType::Winding,
                None,
            ),
        }
    }
}

impl Benchmark for PathBuilderBench {
    // Port of: bench/PathOpsBench.cpp#L162-L164 (chrome/m156)
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/PathOpsBench.cpp#L166-L168 (chrome/m156)
    fn on_delayed_setup(&mut self) {
        // run_builder(fArrays, false, N);
        run_builder(&mut self.arrays, false, N);
    }

    // Port of: bench/PathOpsBench.cpp#L202-L217 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            for _ in 0..100 {
                let result = self.build();
                // force bounds calc as part of the test
                if !result.bounds().is_finite() {
                    eprintln!("should never get here!");
                    return;
                }
            }
        }
    }
}

// Port of: bench/PathOpsBench.cpp#L13 (chrome/m156)
def_bench!(
    path_ops_sect = "PathOpsBench(\"sect\", kIntersect_SkPathOp)",
    PathOpsBench::new("sect", PathOp::Intersect)
);
// Port of: bench/PathOpsBench.cpp#L14 (chrome/m156)
def_bench!(
    path_ops_join = "PathOpsBench(\"join\", kUnion_SkPathOp)",
    PathOpsBench::new("join", PathOp::Union)
);
// Port of: bench/PathOpsBench.cpp#L93 (chrome/m156)
def_bench!(
    path_ops_simplify_rects = "PathOpsSimplifyBench(\"rects\", makerects())",
    PathOpsSimplifyBench::new("rects", makerects())
);
// Port of: bench/PathOpsBench.cpp#L220 (chrome/m156)
def_bench!(
    path_builder_snapshot = "PathBuilderBench(MakeType::kSnapshot, false)",
    PathBuilderBench::new(MakeType::Snapshot, false)
);
// Port of: bench/PathOpsBench.cpp#L221 (chrome/m156)
def_bench!(
    path_builder_detach = "PathBuilderBench(MakeType::kDetach, false)",
    PathBuilderBench::new(MakeType::Detach, false)
);
// Port of: bench/PathOpsBench.cpp#L222 (chrome/m156)
def_bench!(
    path_builder_snapshot_reserve = "PathBuilderBench(MakeType::kSnapshot, true)",
    PathBuilderBench::new(MakeType::Snapshot, true)
);
// Port of: bench/PathOpsBench.cpp#L223 (chrome/m156)
def_bench!(
    path_builder_detach_reserve = "PathBuilderBench(MakeType::kDetach, true)",
    PathBuilderBench::new(MakeType::Detach, true)
);
// Port of: bench/PathOpsBench.cpp#L224 (chrome/m156)
def_bench!(
    path_builder_array_reserve = "PathBuilderBench(MakeType::kArray, true)",
    PathBuilderBench::new(MakeType::Array, true)
);
