// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: bench/MatrixBench.cpp

//! `SkMatrix` operations: scale, inversion and `mapRect`, `mapPoints` and single-point mapping
//! (`bench/MatrixBench.cpp`). `DecomposeMatrixBench` needs `SkDecomposeUpper2x2`, which is not
//! ported, so it is not registered here.

use skia_rust_core::matrix::{Matrix, TypeMask};
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;

use crate::def_bench;
use crate::prelude::*;

/// The `performTest` of a `MatrixBench` subclass.
trait MatrixTest {
    fn perform_test(&mut self);
}

/// `class MatrixBench`: the base class. `onDraw` runs `performTest` `loops` times.
// Port of: bench/MatrixBench.cpp#L12-L36 (chrome/m156)
struct MatrixBench<T> {
    name: String,
    test: T,
}

impl<T: MatrixTest> MatrixBench<T> {
    /// `MatrixBench(const char name[])`: `fName.printf("matrix_%s", name)`.
    fn new(name: &str, test: T) -> Self {
        Self {
            name: format!("matrix_{name}"),
            test,
        }
    }
}

impl<T: MatrixTest> Benchmark for MatrixBench<T> {
    fn is_suitable_for(&self, backend: Backend) -> bool {
        backend == Backend::NonRendering
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn on_draw(&mut self, loops: i32, _canvas: Option<&Canvas>) {
        for _ in 0..loops {
            self.test.perform_test();
        }
    }
}

// ---------------------------------------------------------------------------------------------

/// `class ScaleMatrixBench`.
// Port of: bench/MatrixBench.cpp#L38-L58 (chrome/m156)
struct ScaleMatrix {
    m0: Matrix,
    m1: Matrix,
    m2: Matrix,
    sx: f32,
    sy: f32,
}

impl ScaleMatrix {
    fn new() -> Self {
        let sx = 1.5_f32;
        let sy = sx;
        let mut m0 = Matrix::default();
        m0.reset();
        let m1 = Matrix::scale((sx, sy));
        let m2 = Matrix::translate(Point::new(sx, sy));
        Self { m0, m1, m2, sx, sy }
    }
}

impl MatrixTest for ScaleMatrix {
    fn perform_test(&mut self) {
        let mut m = self.m0.clone();
        m.pre_scale((self.sx, self.sy), None);
        m = self.m1.clone();
        m.pre_scale((self.sx, self.sy), None);
        m = self.m2.clone();
        m.pre_scale((self.sx, self.sy), None);
    }
}

const SCALE_FLAG: u32 = 0x01;
const TRANSLATE_FLAG: u32 = 0x02;
const ROTATE_FLAG: u32 = 0x04;
const PERSPECTIVE_FLAG: u32 = 0x08;
const UNCACHED_TYPE_MASK_FLAG: u32 = 0x10;

/// `class InvertMapRectMatrixBench`. `fIteration` is never read in the C++ and is left out.
// Port of: bench/MatrixBench.cpp#L66-L124 (chrome/m156)
struct InvertMapRect {
    matrix: Matrix,
    flags: u32,
}

impl InvertMapRect {
    fn new(flags: u32) -> Self {
        let mut matrix = Matrix::default();
        matrix.reset();
        if flags & SCALE_FLAG != 0 {
            matrix.post_scale((1.5, 2.5), None);
        }
        if flags & TRANSLATE_FLAG != 0 {
            matrix.post_translate(Point::new(1.5, 2.5));
        }
        if flags & ROTATE_FLAG != 0 {
            matrix.post_rotate(45.0, None);
        }
        if flags & PERSPECTIVE_FLAG != 0 {
            matrix.set_persp_x(1.5);
            matrix.set_persp_y(2.5);
        }
        if flags & UNCACHED_TYPE_MASK_FLAG == 0 {
            // fMatrix.getType(): fills the type cache; the result is not needed.
            let _ = matrix.get_type();
        }
        Self { matrix, flags }
    }
}

impl MatrixTest for InvertMapRect {
    fn perform_test(&mut self) {
        if self.flags & UNCACHED_TYPE_MASK_FLAG != 0 {
            // This will invalidate the typemask without changing the matrix.
            let persp_x = self.matrix.persp_x();
            self.matrix.set_persp_x(persp_x);
        }
        let inv = self.matrix.invert();
        debug_assert!(inv.is_some(), "SkASSERT(invertible)");
        // an arbitrary, small, non-zero rect to transform
        let src_rect = Rect::new(0.0, 0.0, 10.0, 10.0); // SkRect::MakeWH(10, 10)
        if let Some(inv) = inv {
            // inv.mapRect(&transformedRect, srcRect): the result is not used.
            let _ = inv.map_rect(src_rect);
        }
    }
}

// ---------------------------------------------------------------------------------------------

/// `static SkMatrix make_trans()`.
// Port of: bench/MatrixBench.cpp#L126-L126 (chrome/m156)
fn make_trans() -> Matrix {
    Matrix::translate(Point::new(2.0, 3.0))
}

/// `static SkMatrix make_scale()`.
// Port of: bench/MatrixBench.cpp#L127-L127 (chrome/m156)
fn make_scale() -> Matrix {
    let mut m = make_trans();
    m.post_scale((1.5, 0.5), None);
    m
}

/// `static SkMatrix make_afine()`.
// Port of: bench/MatrixBench.cpp#L128-L128 (chrome/m156)
fn make_afine() -> Matrix {
    let mut m = make_trans();
    m.post_rotate(15.0, None);
    m
}

/// `class MapPointsMatrixBench`.
// Port of: bench/MatrixBench.cpp#L129-L153 (chrome/m156)
struct MapPoints {
    m: Matrix,
    src: [Point; MAP_POINTS_N],
    dst: [Point; MAP_POINTS_N],
}

const MAP_POINTS_N: usize = 32;

impl MapPoints {
    fn new(m: Matrix) -> Self {
        let mut rand = Random::default();
        let mut src = [Point::default(); MAP_POINTS_N];
        for pt in &mut src {
            let x = rand.next_s_scalar1();
            let y = rand.next_s_scalar1();
            pt.set(x, y);
        }
        Self {
            m,
            src,
            dst: [Point::default(); MAP_POINTS_N],
        }
    }
}

impl MatrixTest for MapPoints {
    fn perform_test(&mut self) {
        for _ in 0..1_000_000 {
            self.m.map_points(&mut self.dst, &self.src);
        }
    }
}

/// `class MapSinglePointMatrixBench::Use`.
// Port of: bench/MatrixBench.cpp#L158-L162 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Use {
    Points,
    Single,
    Affine,
}

/// `class MapSinglePointMatrixBench`. Its `handle` is a virtual no-op in C++ and is not ported.
// Port of: bench/MatrixBench.cpp#L155-L236 (chrome/m156)
struct MapSinglePoint {
    m: Matrix,
    use_: Use,
    dst: [Point; MAP_POINTS_N],
}

impl MapSinglePoint {
    /// The name is `mappt_` + the use, and for points and single the matrix's type mask.
    fn name(m: &Matrix, use_: Use) -> String {
        let t = m.get_type();
        let mut name = String::from("mappt_");
        name.push_str(match use_ {
            Use::Points => "p",
            Use::Single => "s",
            Use::Affine => "a",
        });
        if use_ != Use::Affine {
            if t == TypeMask::IDENTITY {
                name.push_str("_identity");
            } else {
                if t.contains(TypeMask::AFFINE) {
                    name.push_str("_affine");
                }
                if t.contains(TypeMask::SCALE) {
                    name.push_str("_scale");
                }
                if t.contains(TypeMask::TRANSLATE) {
                    name.push_str("_trans");
                }
            }
        }
        name
    }

    fn new(m: Matrix, use_: Use) -> MatrixBench<Self> {
        let name = Self::name(&m, use_);
        MatrixBench {
            name,
            test: Self {
                m,
                use_,
                dst: [Point::default(); MAP_POINTS_N],
            },
        }
    }
}

impl MatrixTest for MapSinglePoint {
    fn perform_test(&mut self) {
        const K: usize = 1000;
        let mut rand = Random::default();

        match self.use_ {
            Use::Points => {
                for _ in 0..K {
                    let mut src = Point::new(rand.next_s_scalar1(), rand.next_s_scalar1());
                    for j in 0..MAP_POINTS_N {
                        self.m
                            .map_points(&mut self.dst[j..=j], std::slice::from_ref(&src));
                        src.x += 1.0;
                    }
                }
            }
            Use::Single => {
                for _ in 0..K {
                    let mut src = Point::new(rand.next_s_scalar1(), rand.next_s_scalar1());
                    for j in 0..MAP_POINTS_N {
                        self.dst[j] = self.m.map_point(src);
                        src.x += 1.0;
                    }
                }
            }
            Use::Affine => {
                for _ in 0..K {
                    let mut src = Point::new(rand.next_s_scalar1(), rand.next_s_scalar1());
                    for j in 0..MAP_POINTS_N {
                        self.dst[j] = self.m.map_point_affine(src);
                        src.x += 1.0;
                    }
                }
            }
        }
    }
}

/// `m0` … `m3`: the matrices the single-point benches use.
// Port of: bench/MatrixBench.cpp#L238-L249 (chrome/m156)
fn m0() -> Matrix {
    Matrix::new_identity()
}

// Port of: bench/MatrixBench.cpp#L238-L249 (chrome/m156)
fn m1() -> Matrix {
    Matrix::new_all(1.0, 0.0, 1.0, 0.0, 1.0, 2.0, 0.0, 0.0, 1.0)
}

// Port of: bench/MatrixBench.cpp#L238-L249 (chrome/m156)
fn m2() -> Matrix {
    Matrix::new_all(2.0, 0.0, 1.0, 0.0, 3.0, 2.0, 0.0, 0.0, 1.0)
}

// Port of: bench/MatrixBench.cpp#L238-L249 (chrome/m156)
fn m3() -> Matrix {
    Matrix::new_all(2.0, 1.0, 1.0, 1.0, 3.0, 2.0, 0.0, 0.0, 1.0)
}

/// `class MapRectMatrixBench`.
// Port of: bench/MatrixBench.cpp#L287-L318 (chrome/m156)
struct MapRect {
    m: Matrix,
    r: Rect,
    scale_trans: bool,
}

impl MapRect {
    fn new(scale_trans: bool) -> Self {
        let mut m = Matrix::default();
        m.set_scale((2.0, 3.0), None);
        m.post_translate(Point::new(1.0, 2.0));
        Self {
            m,
            r: Rect::new(10.0, 10.0, 100.0, 200.0), // fR.setLTRB(10, 10, 100, 200)
            scale_trans,
        }
    }
}

impl MatrixTest for MapRect {
    fn perform_test(&mut self) {
        const MEGA_LOOP: usize = 1000 * 1000;
        if self.scale_trans {
            for _ in 0..MEGA_LOOP {
                // fM.mapRectScaleTranslate(&dst, fR): the result is not used.
                let _ = self.m.map_rect_scale_translate(self.r);
            }
        } else {
            for _ in 0..MEGA_LOOP {
                // fM.mapRect(&dst, fR): the result is not used.
                let _ = self.m.map_rect(self.r);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------

// Port of: bench/MatrixBench.cpp#L255-L255 (chrome/m156)
def_bench!(
    scale_matrix_bench = "ScaleMatrixBench()",
    MatrixBench::new("scale", ScaleMatrix::new())
);

// Port of: bench/MatrixBench.cpp#L269-L291 (chrome/m156)
def_bench!(
    invert_map_rect_matrix_bench_identity =
        "InvertMapRectMatrixBench(\"invert_maprect_identity\", 0)",
    MatrixBench::new("invert_maprect_identity", InvertMapRect::new(0))
);
def_bench!(
    invert_map_rect_matrix_bench_rectstaysrect = "InvertMapRectMatrixBench( \"invert_maprect_rectstaysrect\", InvertMapRectMatrixBench::kScale_Flag | InvertMapRectMatrixBench::kTranslate_Flag)",
    MatrixBench::new(
        "invert_maprect_rectstaysrect",
        InvertMapRect::new(SCALE_FLAG | TRANSLATE_FLAG)
    )
);
def_bench!(
    invert_map_rect_matrix_bench_translate = "InvertMapRectMatrixBench( \"invert_maprect_translate\", InvertMapRectMatrixBench::kTranslate_Flag)",
    MatrixBench::new(
        "invert_maprect_translate",
        InvertMapRect::new(TRANSLATE_FLAG)
    )
);
def_bench!(
    invert_map_rect_matrix_bench_nonpersp = "InvertMapRectMatrixBench( \"invert_maprect_nonpersp\", InvertMapRectMatrixBench::kScale_Flag | InvertMapRectMatrixBench::kRotate_Flag | InvertMapRectMatrixBench::kTranslate_Flag)",
    MatrixBench::new(
        "invert_maprect_nonpersp",
        InvertMapRect::new(SCALE_FLAG | ROTATE_FLAG | TRANSLATE_FLAG)
    )
);
def_bench!(
    invert_map_rect_matrix_bench_persp = "InvertMapRectMatrixBench( \"invert_maprect_persp\", InvertMapRectMatrixBench::kPerspective_Flag)",
    MatrixBench::new("invert_maprect_persp", InvertMapRect::new(PERSPECTIVE_FLAG))
);
def_bench!(
    invert_map_rect_matrix_bench_typemask_rectstaysrect = "InvertMapRectMatrixBench( \"invert_maprect_typemask_rectstaysrect\", InvertMapRectMatrixBench::kUncachedTypeMask_Flag | InvertMapRectMatrixBench::kScale_Flag | InvertMapRectMatrixBench::kTranslate_Flag)",
    MatrixBench::new(
        "invert_maprect_typemask_rectstaysrect",
        InvertMapRect::new(UNCACHED_TYPE_MASK_FLAG | SCALE_FLAG | TRANSLATE_FLAG)
    )
);
def_bench!(
    invert_map_rect_matrix_bench_typemask_nonpersp = "InvertMapRectMatrixBench( \"invert_maprect_typemask_nonpersp\", InvertMapRectMatrixBench::kUncachedTypeMask_Flag | InvertMapRectMatrixBench::kScale_Flag | InvertMapRectMatrixBench::kRotate_Flag | InvertMapRectMatrixBench::kTranslate_Flag)",
    MatrixBench::new(
        "invert_maprect_typemask_nonpersp",
        InvertMapRect::new(UNCACHED_TYPE_MASK_FLAG | SCALE_FLAG | ROTATE_FLAG | TRANSLATE_FLAG)
    )
);

// Port of: bench/MatrixBench.cpp#L251-L253 (chrome/m156)
def_bench!(
    map_points_matrix_bench_identity =
        "MapPointsMatrixBench(\"mappoints_identity\", SkMatrix::I())",
    MatrixBench::new("mappoints_identity", MapPoints::new(Matrix::new_identity()))
);
def_bench!(
    map_points_matrix_bench_trans = "MapPointsMatrixBench(\"mappoints_trans\", make_trans())",
    MatrixBench::new("mappoints_trans", MapPoints::new(make_trans()))
);
def_bench!(
    map_points_matrix_bench_scale = "MapPointsMatrixBench(\"mappoints_scale\", make_scale())",
    MatrixBench::new("mappoints_scale", MapPoints::new(make_scale()))
);
def_bench!(
    map_points_matrix_bench_affine = "MapPointsMatrixBench(\"mappoints_affine\", make_afine())",
    MatrixBench::new("mappoints_affine", MapPoints::new(make_afine()))
);

// Port of: bench/MatrixBench.cpp#L320-L321 (chrome/m156)
def_bench!(
    map_rect_matrix_bench_maprect = "MapRectMatrixBench(\"maprect\", false)",
    MatrixBench::new("maprect", MapRect::new(false))
);
def_bench!(
    map_rect_matrix_bench_maprectscaletrans = "MapRectMatrixBench(\"maprectscaletrans\", true)",
    MatrixBench::new("maprectscaletrans", MapRect::new(true))
);

// Port of: bench/MatrixBench.cpp#L299-L316 (chrome/m156)
def_bench!(
    map_single_point_matrix_bench_m0_affine =
        "MapSinglePointMatrixBench(m0, MapSinglePointMatrixBench::Use::kAffine)",
    MapSinglePoint::new(m0(), Use::Affine)
);
def_bench!(
    map_single_point_matrix_bench_m0_single =
        "MapSinglePointMatrixBench(m0, MapSinglePointMatrixBench::Use::kSingle)",
    MapSinglePoint::new(m0(), Use::Single)
);
def_bench!(
    map_single_point_matrix_bench_m1_single =
        "MapSinglePointMatrixBench(m1, MapSinglePointMatrixBench::Use::kSingle)",
    MapSinglePoint::new(m1(), Use::Single)
);
def_bench!(
    map_single_point_matrix_bench_m2_single =
        "MapSinglePointMatrixBench(m2, MapSinglePointMatrixBench::Use::kSingle)",
    MapSinglePoint::new(m2(), Use::Single)
);
def_bench!(
    map_single_point_matrix_bench_m3_single =
        "MapSinglePointMatrixBench(m3, MapSinglePointMatrixBench::Use::kSingle)",
    MapSinglePoint::new(m3(), Use::Single)
);
def_bench!(
    map_single_point_matrix_bench_m0_points =
        "MapSinglePointMatrixBench(m0, MapSinglePointMatrixBench::Use::kPoints)",
    MapSinglePoint::new(m0(), Use::Points)
);
def_bench!(
    map_single_point_matrix_bench_m1_points =
        "MapSinglePointMatrixBench(m1, MapSinglePointMatrixBench::Use::kPoints)",
    MapSinglePoint::new(m1(), Use::Points)
);
def_bench!(
    map_single_point_matrix_bench_m2_points =
        "MapSinglePointMatrixBench(m2, MapSinglePointMatrixBench::Use::kPoints)",
    MapSinglePoint::new(m2(), Use::Points)
);
def_bench!(
    map_single_point_matrix_bench_m3_points =
        "MapSinglePointMatrixBench(m3, MapSinglePointMatrixBench::Use::kPoints)",
    MapSinglePoint::new(m3(), Use::Points)
);
