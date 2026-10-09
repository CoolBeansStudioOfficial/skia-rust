// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/GeometryBench.cpp

//! Non-rendering geometry benchmarks: rectangles, quadratic and cubic curves, and convexity
//! (`bench/GeometryBench.cpp`).

use skia_rust_core::geometry::{
    chop_cubic_at, chop_quad_at, eval_quad_at, eval_quad_at_pos_tangent, eval_quad_tangent_at,
};
use skia_rust_core::path::Path;
use skia_rust_core::path_priv::force_compute_convexity;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

use crate::def_bench;
use crate::prelude::*;

/// `GeometryBench`: the name, and the sink that defeats the optimizer.
// Port of: bench/GeometryBench.cpp#L16-L44 (chrome/m156)
struct GeometryBench {
    name: String,
    volatile_int: i32,
}

impl GeometryBench {
    /// `GeometryBench(const char suffix[])`: `fName.printf("geo_%s", suffix)`.
    fn new(suffix: &str) -> Self {
        Self {
            name: format!("geo_{suffix}"),
            volatile_int: 0,
        }
    }

    /// `virtualCallToFoilOptimizers(n)`: `fVolatileInt = n`, the volatile store that keeps the
    /// caller's result alive. `black_box` is the Rust sink in the same place.
    // Port of: bench/GeometryBench.cpp#L35-L38 (chrome/m156)
    fn virtual_call_to_foil_optimizers(&mut self, n: i32) {
        self.volatile_int = n;
        std::hint::black_box(self.volatile_int);
    }
}

// ---------------------------------------------------------------------------------------------

/// `GeoRectBench::onDelayedSetup`: 2048 random rectangles.
// Port of: bench/GeometryBench.cpp#L56-L68 (chrome/m156)
fn geo_rect_setup() -> Vec<Rect> {
    let min: f32 = -100.0;
    let max: f32 = 100.0;
    let mut rand = Random::default();
    let mut rects = vec![Rect::default(); GEO_RECT_N];
    for rect in &mut rects {
        // The four arguments are evaluated left to right.
        let x = rand.next_range_scalar(min, max);
        let y = rand.next_range_scalar(min, max);
        let w = rand.next_range_scalar(min, max);
        let h = rand.next_range_scalar(min, max);
        rect.set_xywh(x, y, w, h);
    }
    rects
}

const GEO_RECT_N: usize = 2048;

/// `class GeoRectBench_intersect`.
// Port of: bench/GeometryBench.cpp#L70-L84 (chrome/m156)
struct GeoRectIntersect {
    base: GeometryBench,
    rects: Vec<Rect>,
}

impl Benchmark for GeoRectIntersect {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.base.name.clone()
    }

    fn on_delayed_setup(&mut self) {
        self.rects = geo_rect_setup();
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            let mut count: i32 = 0;
            for i in 0..GEO_RECT_N {
                let mut r = self.rects[0];
                count = count.wrapping_add(i32::from(r.intersect(self.rects[i])));
            }
            self.base.virtual_call_to_foil_optimizers(count);
        }
    }
}

/// `class GeoRectBench_intersect_rect`.
// Port of: bench/GeometryBench.cpp#L86-L100 (chrome/m156)
struct GeoRectIntersectRect {
    base: GeometryBench,
    rects: Vec<Rect>,
}

impl Benchmark for GeoRectIntersectRect {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.base.name.clone()
    }

    fn on_delayed_setup(&mut self) {
        self.rects = geo_rect_setup();
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            let mut count: i32 = 0;
            let mut r = Rect::default();
            for i in 0..GEO_RECT_N {
                count = count.wrapping_add(i32::from(r.intersect2(self.rects[0], self.rects[i])));
            }
            self.base.virtual_call_to_foil_optimizers(count);
        }
    }
}

/// `class GeoRectBench_Intersects`.
// Port of: bench/GeometryBench.cpp#L102-L114 (chrome/m156)
struct GeoRectIntersects {
    base: GeometryBench,
    rects: Vec<Rect>,
}

impl Benchmark for GeoRectIntersects {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.base.name.clone()
    }

    fn on_delayed_setup(&mut self) {
        self.rects = geo_rect_setup();
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            let mut count: i32 = 0;
            for i in 0..GEO_RECT_N {
                count =
                    count.wrapping_add(i32::from(Rect::intersects2(self.rects[0], self.rects[i])));
            }
            self.base.virtual_call_to_foil_optimizers(count);
        }
    }
}

/// `class GeoRectBench_sort`.
// Port of: bench/GeometryBench.cpp#L116-L127 (chrome/m156)
struct GeoRectSort {
    base: GeometryBench,
    rects: Vec<Rect>,
}

impl Benchmark for GeoRectSort {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.base.name.clone()
    }

    fn on_delayed_setup(&mut self) {
        self.rects = geo_rect_setup();
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            for rect in &mut self.rects {
                rect.sort();
            }
        }
    }
}

// Port of: bench/GeometryBench.cpp#L129-L134 (chrome/m156)
fn new_geo_rect_intersect() -> GeoRectIntersect {
    GeoRectIntersect {
        base: GeometryBench::new("rect_intersect"),
        rects: vec![Rect::default(); GEO_RECT_N],
    }
}

fn new_geo_rect_intersect_rect() -> GeoRectIntersectRect {
    GeoRectIntersectRect {
        base: GeometryBench::new("rect_intersect_rect"),
        rects: vec![Rect::default(); GEO_RECT_N],
    }
}

fn new_geo_rect_intersects() -> GeoRectIntersects {
    GeoRectIntersects {
        base: GeometryBench::new("rect_Intersects"),
        rects: vec![Rect::default(); GEO_RECT_N],
    }
}

fn new_geo_rect_sort() -> GeoRectSort {
    GeoRectSort {
        base: GeometryBench::new("rect_sort"),
        rects: vec![Rect::default(); GEO_RECT_N],
    }
}

// Port of: bench/GeometryBench.cpp#L136-L141 (chrome/m156)
def_bench!(
    geo_rect_bench_intersect = "GeoRectBench_intersect",
    new_geo_rect_intersect()
);
def_bench!(
    geo_rect_bench_intersect_rect = "GeoRectBench_intersect_rect",
    new_geo_rect_intersect_rect()
);
def_bench!(
    geo_rect_bench_intersects = "GeoRectBench_Intersects",
    new_geo_rect_intersects()
);
def_bench!(
    geo_rect_bench_sort = "GeoRectBench_sort",
    new_geo_rect_sort()
);

// ---------------------------------------------------------------------------------------------

/// `QuadBenchBase`: four random points.
// Port of: bench/GeometryBench.cpp#L143-L154 (chrome/m156)
struct QuadBenchBase {
    base: GeometryBench,
    pts: [Point; 4],
}

impl QuadBenchBase {
    fn new(name: &str) -> Self {
        let mut rand = Random::default();
        let mut pts = [Point::default(); 4];
        for pt in &mut pts {
            // fPts[i].set(rand.nextUScalar1(), rand.nextUScalar1()): left to right.
            let x = rand.next_u_scalar1();
            let y = rand.next_u_scalar1();
            pt.set(x, y);
        }
        Self {
            base: GeometryBench::new(name),
            pts,
        }
    }
}

/// `class EvalQuadAt0`: the out-parameter `SkEvalQuadAt(pts, t, &result)`.
// Port of: bench/GeometryBench.cpp#L156-L168 (chrome/m156)
struct EvalQuadAt0 {
    q: QuadBenchBase,
}

impl Benchmark for EvalQuadAt0 {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.q.base.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let mut result = Point::default();
        for _ in 0..loops {
            for _ in 0..4 {
                eval_quad_at_pos_tangent(&self.q.pts, 0.5, Some(&mut result), None);
            }
        }
    }
}

/// `class EvalQuadAt1`: the returning `SkEvalQuadAt(pts, t)`.
// Port of: bench/GeometryBench.cpp#L170-L182 (chrome/m156)
struct EvalQuadAt1 {
    q: QuadBenchBase,
}

impl Benchmark for EvalQuadAt1 {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.q.base.name.clone()
    }

    #[allow(unused_assignments)] // C++ overwrites `result` each call; only the work is kept
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let mut result = Point::default();
        for _ in 0..loops {
            result = eval_quad_at(&self.q.pts, 0.5);
            result = eval_quad_at(&self.q.pts, 0.5);
            result = eval_quad_at(&self.q.pts, 0.5);
            result = eval_quad_at(&self.q.pts, 0.5);
        }
        let _ = result;
    }
}

/// `class EvalQuadTangentAt0`: `SkEvalQuadAt(pts, t, nullptr, &result)`, the tangent out-param.
// Port of: bench/GeometryBench.cpp#L187-L199 (chrome/m156)
struct EvalQuadTangentAt0 {
    q: QuadBenchBase,
}

impl Benchmark for EvalQuadTangentAt0 {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.q.base.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let mut result = Vector::default();
        for _ in 0..loops {
            for _ in 0..4 {
                eval_quad_at_pos_tangent(&self.q.pts, 0.5, None, Some(&mut result));
            }
        }
    }
}

/// `class EvalQuadTangentAt1`: the returning `SkEvalQuadTangentAt(pts, t)`.
// Port of: bench/GeometryBench.cpp#L201-L213 (chrome/m156)
struct EvalQuadTangentAt1 {
    q: QuadBenchBase,
}

impl Benchmark for EvalQuadTangentAt1 {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.q.base.name.clone()
    }

    #[allow(unused_assignments)] // C++ overwrites `result` each call; only the work is kept
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let mut result = Vector::default();
        for _ in 0..loops {
            result = eval_quad_tangent_at(&self.q.pts, 0.5);
            result = eval_quad_tangent_at(&self.q.pts, 0.5);
            result = eval_quad_tangent_at(&self.q.pts, 0.5);
            result = eval_quad_tangent_at(&self.q.pts, 0.5);
        }
        let _ = result;
    }
}

/// `class ChopQuadAt`.
// Port of: bench/GeometryBench.cpp#L217-L229 (chrome/m156)
struct ChopQuadAt {
    q: QuadBenchBase,
}

impl Benchmark for ChopQuadAt {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.q.base.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let mut dst = [Point::default(); 5];
        for _ in 0..loops {
            for _ in 0..4 {
                chop_quad_at(&self.q.pts[..3], &mut dst, 0.5);
            }
        }
    }
}

/// `class ChopCubicAt`.
// Port of: bench/GeometryBench.cpp#L231-L243 (chrome/m156)
struct ChopCubicAt {
    q: QuadBenchBase,
}

impl Benchmark for ChopCubicAt {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.q.base.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        let mut dst = [Point::default(); 7];
        for _ in 0..loops {
            for _ in 0..4 {
                chop_cubic_at(&self.q.pts, &mut dst, 0.5);
            }
        }
    }
}

// Port of: bench/GeometryBench.cpp#L156-L245 (chrome/m156)
def_bench!(
    eval_quad_at0 = "EvalQuadAt0",
    EvalQuadAt0 {
        q: QuadBenchBase::new("evalquadat0")
    }
);
def_bench!(
    eval_quad_at1 = "EvalQuadAt1",
    EvalQuadAt1 {
        q: QuadBenchBase::new("evalquadat1")
    }
);
def_bench!(
    eval_quad_tangent_at0 = "EvalQuadTangentAt0",
    EvalQuadTangentAt0 {
        q: QuadBenchBase::new("evalquadtangentat0")
    }
);
def_bench!(
    eval_quad_tangent_at1 = "EvalQuadTangentAt1",
    EvalQuadTangentAt1 {
        q: QuadBenchBase::new("evalquadtangentat1")
    }
);
def_bench!(
    chop_quad_at_bench = "ChopQuadAt",
    ChopQuadAt {
        q: QuadBenchBase::new("chopquadat")
    }
);
def_bench!(
    chop_cubic_at0 = "ChopCubicAt",
    ChopCubicAt {
        q: QuadBenchBase::new("chopcubicat0")
    }
);

// ---------------------------------------------------------------------------------------------

/// `class RRectConvexityBench : public ConvexityBench`. `ConvexityBench` is the base: its name
/// is `convexity_%s`, and `onPreDraw` prepares the path that `onDraw` times.
// Port of: bench/GeometryBench.cpp#L247-L298 (chrome/m156)
struct RRectConvexityBench {
    name: String,
    path: Path,
}

impl RRectConvexityBench {
    /// `RRectConvexityBench()`: `ConvexityBench("rrect")`.
    // Port of: bench/GeometryBench.cpp#L281-L282 (chrome/m156)
    fn new() -> Self {
        Self {
            name: "convexity_rrect".to_owned(),
            path: Path::default(),
        }
    }

    /// `preparePath()`.
    // Port of: bench/GeometryBench.cpp#L283-L288 (chrome/m156)
    fn prepare_path() -> Path {
        let mut rr = RRect::default();
        rr.set_rect_xy(Rect::new(0.0, 0.0, 100.0, 100.0), 20.0, 30.0);
        Path::rrect(rr, None)
    }
}

impl Benchmark for RRectConvexityBench {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    // Port of: bench/GeometryBench.cpp#L264-L266 (chrome/m156)
    fn on_pre_draw(&mut self, _canvas: Option<&Canvas>) {
        self.path = Self::prepare_path();
    }

    // Port of: bench/GeometryBench.cpp#L268-L272 (chrome/m156)
    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            force_compute_convexity(&self.path);
        }
    }
}

// Port of: bench/GeometryBench.cpp#L297-L297 (chrome/m156)
def_bench!(
    rrect_convexity_bench = "RRectConvexityBench",
    RRectConvexityBench::new()
);
