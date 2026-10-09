// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/PathIterBench.cpp

//! The five ways to walk a path's verbs and points (`bench/PathIterBench.cpp`).

use skia_rust_core::path::{Path, Verb};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_enums::ResolveConvexity;
use skia_rust_core::path_priv;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;

use crate::def_bench;
use crate::prelude::*;

/// `enum class PathIterType`.
// Port of: bench/PathIterBench.cpp#L18-L24 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PathIterType {
    OldIter,
    NewIter,
    Priv,
    Edge,
    PathIter,
}

impl PathIterType {
    /// `gPathIterNames`.
    // Port of: bench/PathIterBench.cpp#L25-L33 (chrome/m156)
    fn name(self) -> &'static str {
        match self {
            Self::OldIter => "olditer",
            Self::NewIter => "newiter",
            Self::Priv => "priv",
            Self::Edge => "edge",
            Self::PathIter => "pathiter",
        }
    }
}

/// `rand_pts`: one to four random points, returning how many.
// Port of: bench/PathIterBench.cpp#L35-L44 (chrome/m156)
fn rand_pts(rand: &mut Random, pts: &mut [Point; 4]) -> usize {
    let mut n = rand.next_u() & 3;
    n += 1;

    for pt in pts.iter_mut().take(n as usize) {
        pt.x = rand.next_s_scalar1();
        pt.y = rand.next_s_scalar1();
    }
    n as usize
}

/// `handle(verb, pts)`: keeps the results alive. The C++ lambda reads `pts[0]`; a close has no
/// points, so the slice may be empty here and reads as the default point.
// Port of: bench/PathIterBench.cpp#L153-L157 (chrome/m156)
fn handle(verb_inc: &mut i32, x_inc: &mut f32, y_inc: &mut f32, verb: i32, pts: &[Point]) {
    let p0 = pts.first().copied().unwrap_or_default();
    *verb_inc += verb;
    *x_inc += p0.x;
    *y_inc += p0.y;
}

/// `class PathIterBench`.
// Port of: bench/PathIterBench.cpp#L46-L151 (chrome/m156)
struct PathIterBench {
    name: String,
    path: Path,
    ty: PathIterType,
    verb_inc: i32,
    x_inc: f32,
    y_inc: f32,
}

impl PathIterBench {
    fn new(ty: PathIterType) -> Self {
        let name = format!("pathiter_{}", ty.name());

        let mut builder = PathBuilder::new();
        builder.move_to((0.0, 0.0));

        let mut rand = Random::default();
        for _ in 0..1000 {
            let mut pts = [Point::default(); 4];
            let n = rand_pts(&mut rand, &mut pts);
            match n {
                2 => {
                    builder.line_to(pts[1]);
                }
                3 => {
                    builder.quad_to(pts[1], pts[2]);
                }
                4 => {
                    builder.cubic_to(pts[1], pts[2], pts[3]);
                }
                _ => {}
            }
        }
        let path = builder.detach();

        Self {
            name,
            path,
            ty,
            verb_inc: 0,
            x_inc: 0.0,
            y_inc: 0.0,
        }
    }
}

impl Benchmark for PathIterBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        // Need to do *something* with the results, so the compile doesn't elide
        // away the code we want to time.
        match self.ty {
            PathIterType::NewIter => {
                for _ in 0..loops {
                    // SkPath::Iter iter(fPath, false); while (auto rec = iter.next())
                    let mut iter = skia_rust_core::path::Iter::new(&self.path, false);
                    while let Some(rec) = iter.next_rec() {
                        handle(
                            &mut self.verb_inc,
                            &mut self.x_inc,
                            &mut self.y_inc,
                            rec.verb() as i32,
                            rec.points(),
                        );
                    }
                }
            }
            PathIterType::OldIter => {
                for _ in 0..loops {
                    let mut iter = skia_rust_core::path::Iter::new(&self.path, false);
                    loop {
                        let mut pts = [Point::default(); 4];
                        let verb = iter.next_verb(&mut pts);
                        if verb == Verb::Done {
                            break;
                        }
                        handle(
                            &mut self.verb_inc,
                            &mut self.x_inc,
                            &mut self.y_inc,
                            verb as i32,
                            &pts,
                        );
                    }
                }
            }
            PathIterType::Priv => {
                for _ in 0..loops {
                    // SkPathPriv::Iterate(fPath)
                    for (verb, pts, _w) in path_priv::iterate(&self.path) {
                        handle(
                            &mut self.verb_inc,
                            &mut self.x_inc,
                            &mut self.y_inc,
                            verb as i32,
                            pts,
                        );
                    }
                }
            }
            PathIterType::Edge => {
                for _ in 0..loops {
                    // SkPathEdgeIter iter(fPath): the raw view, with convexity not resolved.
                    let raw = path_priv::raw(&self.path, ResolveConvexity::No)
                        .expect("a non-empty path has a raw view");
                    let mut iter = path_priv::PathEdgeIter::new(&raw);
                    while let Some(r) = iter.next() {
                        handle(
                            &mut self.verb_inc,
                            &mut self.x_inc,
                            &mut self.y_inc,
                            r.edge as i32,
                            &r.pts,
                        );
                    }
                }
            }
            PathIterType::PathIter => {
                for _ in 0..loops {
                    // auto iter = fPath.iter(); while (auto r = iter.next())
                    for r in self.path.iter() {
                        handle(
                            &mut self.verb_inc,
                            &mut self.x_inc,
                            &mut self.y_inc,
                            r.verb() as i32,
                            r.points(),
                        );
                    }
                }
            }
        }
    }
}

// Port of: bench/PathIterBench.cpp#L158-L162 (chrome/m156)
def_bench!(
    path_iter_bench_new_iter = "PathIterBench(PathIterType::kNewIter)",
    PathIterBench::new(PathIterType::NewIter)
);
def_bench!(
    path_iter_bench_old_iter = "PathIterBench(PathIterType::kOldIter)",
    PathIterBench::new(PathIterType::OldIter)
);
def_bench!(
    path_iter_bench_priv = "PathIterBench(PathIterType::kPriv)",
    PathIterBench::new(PathIterType::Priv)
);
def_bench!(
    path_iter_bench_path_iter = "PathIterBench(PathIterType::kPathIter)",
    PathIterBench::new(PathIterType::PathIter)
);
def_bench!(
    path_iter_bench_edge = "PathIterBench(PathIterType::kEdge)",
    PathIterBench::new(PathIterType::Edge)
);
