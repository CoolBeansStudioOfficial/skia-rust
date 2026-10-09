// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/Matrix44Bench.cpp

//! Non-rendering `SkM44` benchmarks (`bench/Matrix44Bench.cpp`).

use skia_rust_core::m44::{M44, V4};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::matrix_priv;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{SCALAR_PI, degrees_to_radians};

use crate::def_bench;
use crate::prelude::*;

/// The `SkM44` fields of `class M4Bench`: `fM0`, `fM1`, `fM2`.
struct M4Fields {
    m0: M44,
    m1: M44,
    m2: M44,
}

/// The `performTest` of a `M4Bench` subclass. The subclass's own members are the implementor's
/// fields.
trait M4Test {
    fn perform_test(&mut self, m: &mut M4Fields);
}

/// `class M4Bench`: the base class, generic over the `performTest` of its subclass.
// Port of: bench/Matrix44Bench.cpp#L15-L49 (chrome/m156)
struct M4Bench<T> {
    name: String,
    m: M4Fields,
    test: T,
}

impl<T: M4Test> M4Bench<T> {
    fn new(name: &str, test: T) -> Self {
        // The constructor's randomness: the base constructor runs before the subclass's.
        let mut rand = Random::default();
        let mut value = [0.0_f32; 32];
        for v in &mut value {
            *v = rand.next_f();
        }
        let col_major = |range: std::ops::Range<usize>| -> M44 {
            let c: [f32; 16] = value[range].try_into().expect("16 floats");
            M44::col_major(&c)
        };
        Self {
            name: format!("m4_{name}"),
            m: M4Fields {
                m0: M44::default(),
                m1: col_major(0..16),
                m2: col_major(16..32),
            },
            test,
        }
    }
}

impl<T: M4Test> Benchmark for M4Bench<T> {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            self.test.perform_test(&mut self.m);
        }
    }
}

// ---------------------------------------------------------------------------------------------

/// `class M4NEQ`.
// Port of: bench/Matrix44Bench.cpp#L51-L63 (chrome/m156)
struct M4Neq {
    eq: bool,
}

impl M4Test for M4Neq {
    fn perform_test(&mut self, m: &mut M4Fields) {
        for _ in 0..10000 {
            self.eq = m.m2 == m.m1; // should always be false
        }
    }
}

/// `class M4EQ`.
// Port of: bench/Matrix44Bench.cpp#L65-L78 (chrome/m156)
struct M4Eq {
    eq: bool,
}

impl M4Test for M4Eq {
    fn perform_test(&mut self, m: &mut M4Fields) {
        m.m2 = m.m1;
        for _ in 0..10000 {
            self.eq = m.m2 == m.m1; // should always be true
        }
    }
}

/// `class M4Concat`.
// Port of: bench/Matrix44Bench.cpp#L80-L91 (chrome/m156)
struct M4Concat;

impl M4Test for M4Concat {
    fn perform_test(&mut self, m: &mut M4Fields) {
        for _ in 0..10000 {
            m.m0 = M44::concat(&m.m1, &m.m2); // SkM44(fM1, fM2)
        }
    }
}

/// `class M4SetConcat`.
// Port of: bench/Matrix44Bench.cpp#L93-L104 (chrome/m156)
struct M4SetConcat;

impl M4Test for M4SetConcat {
    fn perform_test(&mut self, m: &mut M4Fields) {
        for _ in 0..10000 {
            m.m0.set_concat(&m.m1, &m.m2);
        }
    }
}

// Port of: bench/Matrix44Bench.cpp#L106-L109 (chrome/m156)
def_bench!(m4_eq = "M4EQ()", M4Bench::new("eq", M4Eq { eq: false }));
def_bench!(m4_neq = "M4NEQ()", M4Bench::new("neq", M4Neq { eq: false }));
def_bench!(
    m4_concat = "M4Concat()",
    M4Bench::new("op_concat", M4Concat)
);
def_bench!(
    m4_set_concat = "M4SetConcat()",
    M4Bench::new("set_concat", M4SetConcat)
);

/// `class M4_map4`.
// Port of: bench/Matrix44Bench.cpp#L111-L123 (chrome/m156)
struct M4Map4 {
    v: V4,
}

impl M4Test for M4Map4 {
    fn perform_test(&mut self, m: &mut M4Fields) {
        let v = V4::new(1.0, 2.0, 3.0, 4.0);
        for _ in 0..100_000 {
            self.v = &m.m0 * v;
        }
    }
}

// Port of: bench/Matrix44Bench.cpp#L124-L124 (chrome/m156)
def_bench!(
    m4_map4 = "M4_map4()",
    M4Bench::new("map4", M4Map4 { v: V4::default() })
);

/// `class M4_map2`.
// Port of: bench/Matrix44Bench.cpp#L126-L140 (chrome/m156)
struct M4Map2 {
    v: Point,
}

impl M4Test for M4Map2 {
    fn perform_test(&mut self, _m: &mut M4Fields) {
        let mut m = Matrix::default();
        m.set_rotate(1.0, None);
        for _ in 0..100_000 {
            self.v = m.map_point(Point::new(5.0, 6.0));
        }
    }
}

// Port of: bench/Matrix44Bench.cpp#L141-L141 (chrome/m156)
def_bench!(
    m4_map2 = "M4_map2()",
    M4Bench::new(
        "map2",
        M4Map2 {
            v: Point::default()
        }
    )
);

// ---------------------------------------------------------------------------------------------

/// `enum class MapMatrixType`.
// Port of: bench/Matrix44Bench.cpp#L143-L149 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MapMatrixType {
    TranslateOnly,
    ScaleTranslate,
    Rotate,
    Perspective,
    PerspectiveClipped,
}

/// `class MapRectBench`: the matrix and the rectangles, shared by the `M4` and `M33` variants.
struct MapRectFields {
    m: M44,
    s: Rect,
    d: Rect,
}

/// The `performTest` of a `MapRectBench` subclass.
trait MapRectTest {
    fn perform_test(&mut self, f: &mut MapRectFields);
}

/// `class MapRectBench`. The subclass's state is built from the matrix (`make_test`), as the
/// C++ subclass constructor runs after the base constructor.
// Port of: bench/Matrix44Bench.cpp#L151-L199 (chrome/m156)
struct MapRectBench<T> {
    name: String,
    f: MapRectFields,
    test: T,
}

impl<T: MapRectTest> MapRectBench<T> {
    #[allow(clippy::many_single_char_names)] // x, y, w, h mirror the C++ MakeXYWH arguments
    fn new(kind: MapMatrixType, name: &str, make_test: impl FnOnce(&M44) -> T) -> Self {
        let mut rand = Random::default();
        let (type_name, m) = match kind {
            MapMatrixType::TranslateOnly => {
                // Argument order: left to right, as Clang evaluates them.
                let x = rand.next_f();
                let y = rand.next_f();
                ("t", M44::translate(x, y, 0.0))
            }
            MapMatrixType::ScaleTranslate => {
                let x = rand.next_f();
                let y = rand.next_f();
                let mut m = M44::scale(x, y, 1.0);
                let tx = rand.next_f();
                let ty = rand.next_f();
                m.post_translate(tx, ty, None);
                ("s+t", m)
            }
            MapMatrixType::Rotate => (
                "r",
                M44::rotate(
                    skia_rust_core::m44::V3::new(0.0, 0.0, 1.0),
                    degrees_to_radians(45.0),
                ),
            ),
            MapMatrixType::Perspective => {
                // Hand chosen to have all corners with w > 0 and w != 1
                let mut m = M44::perspective(0.01, 10.0, SCALAR_PI / 3.0);
                m.pre_translate(0.0, 5.0, -0.1);
                m.pre_concat(&M44::rotate(
                    skia_rust_core::m44::V3::new(0.0, 1.0, 0.0),
                    0.008, // radians
                ));
                ("p", m)
            }
            MapMatrixType::PerspectiveClipped => {
                // Hand chosen to have some corners with w > 0 and some with w < 0
                let mut m = M44::new_identity();
                m.set_row(3, &V4::new(-0.2, -0.6, 0.0, 8.0));
                ("pc", m)
            }
        };
        // SkRect::MakeXYWH(x, y, w, h) is {x, y, x + w, y + h}; the arguments are evaluated
        // left to right.
        let x = 10.0 * rand.next_f();
        let y = 10.0 * rand.next_f();
        let w = 150.0 * rand.next_f();
        let h = 150.0 * rand.next_f();
        let s = Rect::new(x, y, x + w, y + h);

        let test = make_test(&m);
        Self {
            name: format!("mapRect_{name}_{type_name}"),
            f: MapRectFields {
                m,
                s,
                d: Rect::default(),
            },
            test,
        }
    }
}

impl<T: MapRectTest> Benchmark for MapRectBench<T> {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            self.test.perform_test(&mut self.f);
        }
    }
}

/// `class M4_mapRectBench`.
// Port of: bench/Matrix44Bench.cpp#L201-L217 (chrome/m156)
struct M4MapRect;

impl MapRectTest for M4MapRect {
    fn perform_test(&mut self, f: &mut MapRectFields) {
        for _ in 0..100_000 {
            f.d = matrix_priv::map_rect(&f.m, &f.s);
        }
    }
}

/// `class M33_mapRectBench`. `fM33 = fM.asM33()` is made by the constructor.
// Port of: bench/Matrix44Bench.cpp#L219-L244 (chrome/m156)
struct M33MapRect {
    m33: Matrix,
}

impl MapRectTest for M33MapRect {
    fn perform_test(&mut self, f: &mut MapRectFields) {
        for _ in 0..100_000 {
            // C++ `SkRect SkMatrix::mapRect(const SkRect&)` drops the bool of the
            // `mapRect(SkRect* dst, ...)` overload; that is the `.0`.
            f.d = self.m33.map_rect(f.s).0;
        }
    }
}

// Port of: bench/Matrix44Bench.cpp#L218-L218 (chrome/m156)
def_bench!(
    m4_map_rect_bench_translate_only = "M4_mapRectBench(MapMatrixType::kTranslateOnly)",
    MapRectBench::new(MapMatrixType::TranslateOnly, "m4", |_| M4MapRect)
);
def_bench!(
    m4_map_rect_bench_scale_translate = "M4_mapRectBench(MapMatrixType::kScaleTranslate)",
    MapRectBench::new(MapMatrixType::ScaleTranslate, "m4", |_| M4MapRect)
);
def_bench!(
    m4_map_rect_bench_rotate = "M4_mapRectBench(MapMatrixType::kRotate)",
    MapRectBench::new(MapMatrixType::Rotate, "m4", |_| M4MapRect)
);
def_bench!(
    m4_map_rect_bench_perspective = "M4_mapRectBench(MapMatrixType::kPerspective)",
    MapRectBench::new(MapMatrixType::Perspective, "m4", |_| M4MapRect)
);
def_bench!(
    m4_map_rect_bench_perspective_clipped = "M4_mapRectBench(MapMatrixType::kPerspectiveClipped)",
    MapRectBench::new(MapMatrixType::PerspectiveClipped, "m4", |_| M4MapRect)
);

// Port of: bench/Matrix44Bench.cpp#L246-L259 (chrome/m156)
def_bench!(
    m33_map_rect_bench_translate_only = "M33_mapRectBench(MapMatrixType::kTranslateOnly)",
    MapRectBench::new(MapMatrixType::TranslateOnly, "m33", |m| M33MapRect {
        m33: m.to_m33()
    })
);
def_bench!(
    m33_map_rect_bench_scale_translate = "M33_mapRectBench(MapMatrixType::kScaleTranslate)",
    MapRectBench::new(MapMatrixType::ScaleTranslate, "m33", |m| M33MapRect {
        m33: m.to_m33()
    })
);
def_bench!(
    m33_map_rect_bench_rotate = "M33_mapRectBench(MapMatrixType::kRotate)",
    MapRectBench::new(MapMatrixType::Rotate, "m33", |m| M33MapRect {
        m33: m.to_m33()
    })
);
def_bench!(
    m33_map_rect_bench_perspective = "M33_mapRectBench(MapMatrixType::kPerspective)",
    MapRectBench::new(MapMatrixType::Perspective, "m33", |m| M33MapRect {
        m33: m.to_m33()
    })
);
def_bench!(
    m33_map_rect_bench_perspective_clipped = "M33_mapRectBench(MapMatrixType::kPerspectiveClipped)",
    MapRectBench::new(MapMatrixType::PerspectiveClipped, "m33", |m| M33MapRect {
        m33: m.to_m33()
    })
);
