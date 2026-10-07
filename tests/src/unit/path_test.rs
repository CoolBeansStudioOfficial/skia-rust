// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathTest.cpp (chrome/m156)

#![cfg(test)]

// Not ported yet (manifest stays `todo`): DEF_TESTs that need SkCanvas / SkSurface / SkPaint,
// the stroker (`skpathutils::FillPathWithPaint`), SkRegion::setPath or PathOps:
//   Paths (draws through SkSurface; also SkRegion::setPath, SkStrokeRec, the stroker),
//   PathBigCubic, HugeGeometry, ClipPath_nonfinite, skbug_6450, triangle_onehalf, triangle_big,
//   path_walk_simple_edges_1154864, path_walk_edges_concave_large_dx (SkSurface / SkCanvas).

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::float_bits::bits_to_float;
use skia_rust_core::floating_point::is_finite;
use skia_rust_core::geometry::{Conic, eval_cubic_at, eval_quad_at, eval_quad_at_pos_tangent};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::paint::Style;
use skia_rust_core::path::{Iter, Path};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_data::PathData;
use skia_rust_core::path_enums::ResolveConvexity;
use skia_rust_core::path_iter::PathIter;
use skia_rust_core::path_priv::{self, PathEdgeIter};
use skia_rust_core::path_types::{PathDirection, PathFillType, PathVerb};
use skia_rust_core::path_utils::fill_path_with_paint;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::scalar::{SCALAR_INFINITY, Scalar, scalar, scalar_sqrt};
use skia_rust_core::utils::parse_path;

// Port of: tests/PathTest.cpp#L4126-L4137 (chrome/m156)
fn test_contains_pt(reporter: &mut Reporter, bu: &PathBuilder, pt: Point, expected_contains: bool) {
    let path = bu.snapshot();

    let raw = path_priv::raw_builder(bu, ResolveConvexity::No);
    reporter_assert!(reporter, raw.is_some());
    let Some(raw) = raw else { return };
    let pdata = PathData::make(raw.points(), raw.verbs(), raw.conics());

    reporter_assert!(reporter, bu.contains(pt) == expected_contains);
    reporter_assert!(reporter, path.contains(pt) == expected_contains);
    reporter_assert!(
        reporter,
        pdata.is_some_and(|d| d.contains(pt, raw.fill_type()) == expected_contains)
    );
}

// Port of: tests/PathTest.cpp#L4139-L4320 (chrome/m156)
#[allow(clippy::too_many_lines)] // mirrors the C++ function
#[allow(clippy::excessive_precision)] // float literals are copied verbatim from the C++
fn test_contains(reporter: &mut Reporter) {
    let b = bits_to_float;
    let mut bu = PathBuilder::new();
    bu.move_to((b(0xe085_e7b1), b(0x5f51_2c00))) // -7.7191e+19f, 1.50724e+19f
        .conic_to(
            (b(0xdfda_a221), b(0x5eaa_c338)),
            (b(0x6034_2f13), b(0xdf0c_bb58)),
            b(0x3f35_04f3),
        ) // -3.15084e+19f, 6.15237e+18f, 5.19345e+19f, -1.01408e+19f, 0.707107f
        .conic_to(
            (b(0x60ea_d799), b(0xdfb7_6c24)),
            (b(0x609b_9872), b(0xdf73_0de8)),
            b(0x3f35_04f4),
        ) // 1.35377e+20f, -2.6434e+19f, 8.96947e+19f, -1.75139e+19f, 0.707107f
        .line_to((b(0x609b_9872), b(0xdf73_0de8))) // 8.96947e+19f, -1.75139e+19f
        .conic_to(
            (b(0x6018_b296), b(0xdeee_870d)),
            (b(0xe008_cd8e), b(0x5ed5_b2db)),
            b(0x3f35_04f3),
        ) // 4.40121e+19f, -8.59386e+18f, -3.94308e+19f, 7.69931e+18f, 0.707107f
        .conic_to(
            (b(0xe0d5_26d9), b(0x5fa6_7b31)),
            (b(0xe085_e7b2), b(0x5f51_2c01)),
            b(0x3f35_04f3),
        ); // -1.22874e+20f, 2.39925e+19f, -7.7191e+19f, 1.50724e+19f, 0.707107
    // this may return true or false, depending on the platform's numerics, but it should not crash
    let _ = bu.contains((-77.202_766_4, 15.306_605_3));

    let check = |reporter: &mut Reporter, bu: &PathBuilder, p: (scalar, scalar), expected: bool| {
        test_contains_pt(reporter, bu, Point::new(p.0, p.1), expected);
    };

    bu.reset();
    bu.set_fill_type(PathFillType::InverseWinding);
    check(reporter, &bu, (0.0, 0.0), true);
    bu.set_fill_type(PathFillType::Winding);
    check(reporter, &bu, (0.0, 0.0), false);
    bu.set_fill_type(PathFillType::Winding)
        .move_to((4.0, 4.0))
        .line_to((6.0, 8.0))
        .line_to((8.0, 4.0));
    // test on edge
    check(reporter, &bu, (6.0, 4.0), true);
    check(reporter, &bu, (5.0, 6.0), true);
    check(reporter, &bu, (7.0, 6.0), true);
    // test quick reject
    check(reporter, &bu, (4.0, 0.0), false);
    check(reporter, &bu, (0.0, 4.0), false);
    check(reporter, &bu, (4.0, 10.0), false);
    check(reporter, &bu, (10.0, 4.0), false);
    // test various crossings in x
    check(reporter, &bu, (5.0, 7.0), false);
    check(reporter, &bu, (6.0, 7.0), true);
    check(reporter, &bu, (7.0, 7.0), false);
    bu = PathBuilder::new();
    bu.move_to((4.0, 4.0))
        .line_to((8.0, 6.0))
        .line_to((4.0, 8.0));
    // test on edge
    check(reporter, &bu, (4.0, 6.0), true);
    check(reporter, &bu, (6.0, 5.0), true);
    check(reporter, &bu, (6.0, 7.0), true);
    // test various crossings in y
    check(reporter, &bu, (7.0, 5.0), false);
    check(reporter, &bu, (7.0, 6.0), true);
    check(reporter, &bu, (7.0, 7.0), false);
    bu = PathBuilder::new();
    bu.move_to((4.0, 4.0))
        .line_to((8.0, 4.0))
        .line_to((8.0, 8.0))
        .line_to((4.0, 8.0));
    // test on vertices
    check(reporter, &bu, (4.0, 4.0), true);
    check(reporter, &bu, (8.0, 4.0), true);
    check(reporter, &bu, (8.0, 8.0), true);
    check(reporter, &bu, (4.0, 8.0), true);
    bu = PathBuilder::new();
    bu.move_to((4.0, 4.0))
        .line_to((6.0, 8.0))
        .line_to((2.0, 8.0));
    // test on edge
    check(reporter, &bu, (5.0, 6.0), true);
    check(reporter, &bu, (4.0, 8.0), true);
    check(reporter, &bu, (3.0, 6.0), true);
    bu = PathBuilder::new();
    bu.move_to((4.0, 4.0))
        .line_to((0.0, 6.0))
        .line_to((4.0, 8.0));
    // test on edge
    check(reporter, &bu, (2.0, 5.0), true);
    check(reporter, &bu, (2.0, 7.0), true);
    check(reporter, &bu, (4.0, 6.0), true);
    // test canceling coincident edge (a smaller triangle is coincident with a larger one)
    bu = PathBuilder::new();
    bu.move_to((4.0, 0.0))
        .line_to((6.0, 4.0))
        .line_to((2.0, 4.0))
        .move_to((4.0, 0.0))
        .line_to((0.0, 8.0))
        .line_to((8.0, 8.0));
    check(reporter, &bu, (1.0, 2.0), false);
    check(reporter, &bu, (3.0, 2.0), false);
    check(reporter, &bu, (4.0, 0.0), false);
    check(reporter, &bu, (4.0, 4.0), true);

    // test quads
    bu = PathBuilder::new();
    bu.move_to((4.0, 4.0))
        .quad_to((6.0, 6.0), (8.0, 8.0))
        .quad_to((6.0, 8.0), (4.0, 8.0))
        .quad_to((4.0, 6.0), (4.0, 4.0));
    check(reporter, &bu, (5.0, 6.0), true);
    check(reporter, &bu, (6.0, 5.0), false);
    // test quad edge
    check(reporter, &bu, (5.0, 5.0), true);
    check(reporter, &bu, (5.0, 8.0), true);
    check(reporter, &bu, (4.0, 5.0), true);
    // test quad endpoints
    check(reporter, &bu, (4.0, 4.0), true);
    check(reporter, &bu, (8.0, 8.0), true);
    check(reporter, &bu, (4.0, 8.0), true);

    bu.reset();
    let q_pts = [
        Point::new(6.0, 6.0),
        Point::new(8.0, 8.0),
        Point::new(6.0, 8.0),
        Point::new(4.0, 8.0),
        Point::new(4.0, 6.0),
        Point::new(4.0, 4.0),
        Point::new(6.0, 6.0),
    ];
    bu.move_to(q_pts[0]);
    let mut index = 1;
    while index < q_pts.len() {
        bu.quad_to(q_pts[index], q_pts[index + 1]);
        index += 2;
    }
    check(reporter, &bu, (5.0, 6.0), true);
    check(reporter, &bu, (6.0, 5.0), false);
    // test quad edge
    let mut halfway = Point::default();
    let mut index = 0;
    while index < q_pts.len() - 2 {
        eval_quad_at_pos_tangent(&q_pts[index..], 0.5, Some(&mut halfway), None);
        check(reporter, &bu, (halfway.x, halfway.y), true);
        index += 2;
    }

    // test conics
    bu.reset();
    let k_pts = [
        Point::new(4.0, 4.0),
        Point::new(6.0, 6.0),
        Point::new(8.0, 8.0),
        Point::new(6.0, 8.0),
        Point::new(4.0, 8.0),
        Point::new(4.0, 6.0),
        Point::new(4.0, 4.0),
    ];
    bu.move_to(k_pts[0]);
    let mut index = 1;
    while index < k_pts.len() {
        bu.conic_to(k_pts[index], k_pts[index + 1], 0.5);
        index += 2;
    }
    check(reporter, &bu, (5.0, 6.0), true);
    check(reporter, &bu, (6.0, 5.0), false);
    // test conic edge
    let mut index = 0;
    while index < k_pts.len() - 2 {
        let conic = Conic::from_points(&k_pts[index..], 0.5);
        halfway = conic.eval_at(0.5);
        check(reporter, &bu, (halfway.x, halfway.y), true);
        index += 2;
    }
    // test conic end points
    check(reporter, &bu, (4.0, 4.0), true);
    check(reporter, &bu, (8.0, 8.0), true);
    check(reporter, &bu, (4.0, 8.0), true);

    // test cubics
    let pts = [
        Point::new(5.0, 4.0),
        Point::new(6.0, 5.0),
        Point::new(7.0, 6.0),
        Point::new(6.0, 6.0),
        Point::new(4.0, 6.0),
        Point::new(5.0, 7.0),
        Point::new(5.0, 5.0),
        Point::new(5.0, 4.0),
        Point::new(6.0, 5.0),
        Point::new(7.0, 6.0),
    ];
    for i in 0..3 {
        bu = PathBuilder::new_with_fill_type(PathFillType::EvenOdd);
        bu.move_to((pts[i].x, pts[i].y))
            .cubic_to(pts[i + 1], pts[i + 2], pts[i + 3])
            .cubic_to(pts[i + 4], pts[i + 5], pts[i + 6])
            .close();
        check(reporter, &bu, (5.5, 5.5), true);
        check(reporter, &bu, (4.5, 5.5), false);
        // test cubic edge
        eval_cubic_at(&pts[i..], 0.5, Some(&mut halfway), None, None);
        check(reporter, &bu, (halfway.x, halfway.y), true);
        eval_cubic_at(&pts[i + 3..], 0.5, Some(&mut halfway), None, None);
        check(reporter, &bu, (halfway.x, halfway.y), true);
        // test cubic end points
        check(reporter, &bu, (pts[i].x, pts[i].y), true);
        check(reporter, &bu, (pts[i + 3].x, pts[i + 3].y), true);
        check(reporter, &bu, (pts[i + 6].x, pts[i + 6].y), true);
    }
}

// Port of: tests/PathTest.cpp#L4715-L4754 (chrome/m156)
fn test_interp(reporter: &mut Reporter) {
    let mut p1 = Path::new();
    let mut p2 = Path::new();
    let mut out = Path::new();
    reporter_assert!(reporter, p1.is_interpolatable(&p2));
    reporter_assert!(reporter, p1.interpolate_inplace(&p2, 0.0, &mut out));
    reporter_assert!(reporter, p1 == out);
    reporter_assert!(reporter, p1.interpolate_inplace(&p2, 1.0, &mut out));
    reporter_assert!(reporter, p1 == out);
    p1 = PathBuilder::new()
        .move_to((0.0, 2.0))
        .line_to((0.0, 4.0))
        .detach();
    reporter_assert!(reporter, !p1.is_interpolatable(&p2));
    reporter_assert!(reporter, !p1.interpolate_inplace(&p2, 1.0, &mut out));
    p2 = PathBuilder::new()
        .move_to((6.0, 0.0))
        .line_to((8.0, 0.0))
        .detach();
    reporter_assert!(reporter, p1.is_interpolatable(&p2));
    reporter_assert!(reporter, p1.interpolate_inplace(&p2, 0.0, &mut out));
    reporter_assert!(reporter, p2 == out);
    reporter_assert!(reporter, p1.interpolate_inplace(&p2, 1.0, &mut out));
    reporter_assert!(reporter, p1 == out);
    reporter_assert!(reporter, p1.interpolate_inplace(&p2, 0.5, &mut out));
    reporter_assert!(
        reporter,
        *out.bounds() == Rect::from_ltrb(3.0, 1.0, 4.0, 2.0)
    );
    p1 = PathBuilder::new()
        .move_to((4.0, 4.0))
        .conic_to((5.0, 4.0), (5.0, 5.0), 1.0 / scalar_sqrt(2.0))
        .detach();
    p2 = PathBuilder::new()
        .move_to((4.0, 2.0))
        .conic_to((7.0, 2.0), (7.0, 5.0), 1.0 / scalar_sqrt(2.0))
        .detach();
    reporter_assert!(reporter, p1.is_interpolatable(&p2));
    reporter_assert!(reporter, p1.interpolate_inplace(&p2, 0.5, &mut out));
    reporter_assert!(
        reporter,
        *out.bounds() == Rect::from_ltrb(4.0, 3.0, 6.0, 5.0)
    );
    p2 = PathBuilder::new()
        .move_to((4.0, 2.0))
        .conic_to((6.0, 3.0), (6.0, 5.0), 1.0)
        .detach();
    reporter_assert!(reporter, !p1.is_interpolatable(&p2));
    p2 = PathBuilder::new()
        .move_to((4.0, 4.0))
        .conic_to((5.0, 4.0), (5.0, 5.0), 0.5)
        .detach();
    reporter_assert!(reporter, !p1.is_interpolatable(&p2));
}

// Port of: tests/PathTest.cpp#L4756-L4758 (chrome/m156)
def_test!(PathInterp, |reporter| {
    test_interp(reporter);
});

// Port of: tests/PathTest.cpp#L4776-L4778 (chrome/m156)
def_test!(PathContains, |reporter| {
    test_contains(reporter);
});

// Port of: tests/PathTest.cpp#L4875-L4887 (chrome/m156)
def_test!(conservatively_contains_rect, |_reporter| {
    let b = bits_to_float;
    let path = PathBuilder::new()
        .move_to((b(0x4400_0000), b(0x3739_38b8))) // 512, 1.10401e-05f
        // 1.4013e-45f, -9.22346e+18f, 3.58732e-43f, 0, 3.58732e-43f, 0
        .cubic_to(
            (b(0x0000_0001), b(0xdf00_0052)),
            (b(0x0000_0100), b(0x0000_0000)),
            (b(0x0000_0100), b(0x0000_0000)),
        )
        .move_to((0.0, 0.0))
        .detach();

    // this should not assert
    let _ = path.conservatively_contains_rect(Rect::new(-211_747.0, 12.1115, -197_893.0, 25.0321));
});

// Port of: tests/PathTest.cpp#L4914-L4920 (chrome/m156)
def_test!(NonFinitePathIteration, |reporter| {
    let path = PathBuilder::new()
        .move_to((SCALAR_INFINITY, SCALAR_INFINITY))
        .detach();
    let iterate = path_priv::iterate(&path);
    reporter_assert!(reporter, iterate.is_done());
});

// Port of: tests/PathTest.cpp#L4922-L4944 (chrome/m156)
def_test!(AndroidArc, |_reporter| {
    let tests = [
        "M50,0A50,50,0,0 1 100,50 L100,85 A15,15,0,0 1 85,100 L50,100 A50,50,0,0 1 50,0z",
        "M50,0L92,0 A8,8,0,0 1 100,8 L100,92 A8,8,0,0 1 92,100 L8,100 \
         A8,8,0,0 1 0,92 L 0,8 A8,8,0,0 1 8,0z",
        "M50 0A50 50,0,1,1,50 100A50 50,0,1,1,50 0",
    ];
    for test in tests {
        let a_path = parse_path::from_svg(test);
        debug_assert!(a_path.is_some());
        let a_path = a_path.unwrap_or_default();
        debug_assert!(a_path.is_convex());
        let mut scale: scalar = 1.0;
        while scale < 1000.0 {
            let scale_path = a_path.make_transform(&Matrix::scale((scale, scale)));
            debug_assert!(scale_path.is_convex());
            let _ = scale_path;
            scale *= 1.1;
        }
        let mut scale: scalar = 1.0;
        while scale < 0.001 {
            let scale_path = a_path.make_transform(&Matrix::scale((scale, scale)));
            debug_assert!(scale_path.is_convex());
            let _ = scale_path;
            scale /= 1.1;
        }
    }
});

// Port of: tests/PathTest.cpp#L5008-L5159 (chrome/m156)
#[allow(clippy::too_many_lines)] // mirrors the C++ test
#[allow(clippy::similar_names)] // names follow the C++
#[allow(clippy::items_after_statements)] // constants stay next to the C++ code they mirror
fn path_is_rect_body(reporter: &mut Reporter) {
    let make_path = |points: &[Point], close: bool| -> Path {
        let mut builder = PathBuilder::new();
        for (index, p) in points.iter().enumerate() {
            if index < 2 {
                builder.move_to(*p);
            } else {
                builder.line_to(*p);
            }
        }
        if close {
            builder.close();
        }
        builder.detach()
    };
    let make_path2 = |points: &[Point], verbs: &[PathVerb]| -> Path {
        let mut builder = PathBuilder::new();
        let mut pts = points.iter();
        for v in verbs {
            match v {
                PathVerb::Move => {
                    builder.move_to(*pts.next().expect("enough points"));
                }
                PathVerb::Line => {
                    builder.line_to(*pts.next().expect("enough points"));
                }
                PathVerb::Close => {
                    builder.close();
                }
                _ => debug_assert!(false),
            }
        }
        builder.detach()
    };
    // `path.isRect(&rect)`: rect is only written when true.
    let is_rect = |path: &Path, rect: &mut Rect| -> bool {
        if let Some((r, _, _)) = path.is_rect() {
            *rect = r;
            true
        } else {
            false
        }
    };
    let p = Point::new;
    use PathVerb::{Close as C, Line as L, Move as M};

    // isolated from skbug.com/40039046 (bug description)
    let mut rect = Rect::default();
    let points = [
        p(10.0, 10.0),
        p(75.0, 75.0),
        p(150.0, 75.0),
        p(150.0, 150.0),
        p(75.0, 150.0),
    ];
    let mut path = make_path(&points, false);
    reporter_assert!(reporter, is_rect(&path, &mut rect));
    let mut compare = Rect::bounds_or_empty(&points[1..]);
    reporter_assert!(reporter, rect == compare);
    // isolated from skbug.com/40039046#c3
    let points3 = [
        p(75.0, 50.0),
        p(100.0, 75.0),
        p(150.0, 75.0),
        p(150.0, 150.0),
        p(75.0, 150.0),
        p(75.0, 50.0),
    ];
    path = make_path(&points3, true);
    reporter_assert!(reporter, !is_rect(&path, &mut rect));
    // isolated from skbug.com/40039046#c9
    let points9 = [
        p(10.0, 10.0),
        p(75.0, 75.0),
        p(150.0, 75.0),
        p(150.0, 150.0),
        p(75.0, 150.0),
    ];
    path = make_path(&points9, true);
    reporter_assert!(reporter, is_rect(&path, &mut rect));
    compare.set_bounds(&points9[1..]);
    reporter_assert!(reporter, rect == compare);
    // isolated from skbug.com/40039046#c11
    let verbs11 = [M, L, L, L, L, M];
    let points11 = [
        p(75.0, 150.0),
        p(75.0, 75.0),
        p(150.0, 75.0),
        p(150.0, 150.0),
        p(75.0, 150.0),
        p(75.0, 150.0),
    ];
    path = make_path2(&points11, &verbs11);
    reporter_assert!(reporter, is_rect(&path, &mut rect));
    compare.set_bounds(&points11);
    reporter_assert!(reporter, rect == compare);
    // isolated from skbug.com/40039046#c14
    let verbs14 = [M, M, M, M, L, L, L, L, C, L, C];
    let points14 = [
        p(250.0, 75.0),
        p(250.0, 75.0),
        p(250.0, 75.0),
        p(100.0, 75.0),
        p(150.0, 75.0),
        p(150.0, 150.0),
        p(75.0, 150.0),
        p(75.0, 75.0),
        p(0.0, 0.0),
    ];
    path = make_path2(&points14, &verbs14);
    reporter_assert!(reporter, !is_rect(&path, &mut rect));
    // isolated from skbug.com/40039046#c15
    let verbs15 = [M, L, L, L, M];
    let points15 = [
        p(75.0, 75.0),
        p(150.0, 75.0),
        p(150.0, 150.0),
        p(75.0, 150.0),
        p(250.0, 75.0),
    ];
    path = make_path2(&points15, &verbs15);
    reporter_assert!(reporter, is_rect(&path, &mut rect));
    compare.set_bounds(&points15[..points15.len() - 1]);
    reporter_assert!(reporter, rect == compare);
    // isolated from skbug.com/40039046#c17
    let points17 = [
        p(75.0, 10.0),
        p(75.0, 75.0),
        p(150.0, 75.0),
        p(150.0, 150.0),
        p(75.0, 150.0),
        p(75.0, 10.0),
    ];
    path = make_path(&points17, true);
    reporter_assert!(reporter, !is_rect(&path, &mut rect));
    // isolated from skbug.com/40039046#c19
    let verbs19 = [M, L, L, L, L, L, L, C, M, L, L];
    let points19 = [
        p(75.0, 75.0),
        p(75.0, 75.0),
        p(75.0, 75.0),
        p(75.0, 75.0),
        p(150.0, 75.0),
        p(150.0, 150.0),
        p(75.0, 150.0),
        p(10.0, 10.0),
        p(30.0, 10.0),
        p(10.0, 30.0),
    ];
    path = make_path2(&points19, &verbs19);
    reporter_assert!(reporter, !is_rect(&path, &mut rect));
    // isolated from skbug.com/40039046#c23
    let verbs23 = [M, L, M, L, L, L, L, C];
    let points23 = [
        p(75.0, 75.0),
        p(75.0, 75.0),
        p(75.0, 75.0),
        p(75.0, 75.0),
        p(150.0, 75.0),
        p(150.0, 150.0),
        p(75.0, 150.0),
    ];
    path = make_path2(&points23, &verbs23);
    reporter_assert!(reporter, is_rect(&path, &mut rect));
    compare.set_bounds(&points23);
    reporter_assert!(reporter, rect == compare);
    // isolated from skbug.com/40039046#c29
    let verbs29 = [M, L, L, L, L, M, C];
    let points29 = [
        p(75.0, 75.0),
        p(150.0, 75.0),
        p(150.0, 150.0),
        p(75.0, 150.0),
        p(75.0, 250.0),
        p(75.0, 75.0),
    ];
    path = make_path2(&points29, &verbs29);
    reporter_assert!(reporter, !is_rect(&path, &mut rect));
    // isolated from skbug.com/40039046#c31
    let verbs31 = [M, L, L, L, L, M, C];
    let points31 = [
        p(75.0, 75.0),
        p(150.0, 75.0),
        p(150.0, 150.0),
        p(75.0, 150.0),
        p(75.0, 10.0),
        p(75.0, 75.0),
    ];
    path = make_path2(&points31, &verbs31);
    reporter_assert!(reporter, is_rect(&path, &mut rect));
    compare.set_bounds(&points31[..4]);
    reporter_assert!(reporter, rect == compare);
    // isolated from skbug.com/40039046#c36
    let verbs36 = [M, L, L, L, M, L];
    let points36 = [
        p(75.0, 75.0),
        p(150.0, 75.0),
        p(150.0, 150.0),
        p(10.0, 150.0),
        p(75.0, 75.0),
        p(75.0, 75.0),
    ];
    path = make_path2(&points36, &verbs36);
    reporter_assert!(reporter, !is_rect(&path, &mut rect));
    // isolated from skbug.com/40039046#c39
    let verbs39 = [M, L, L, L];
    let points39 = [
        p(150.0, 75.0),
        p(150.0, 150.0),
        p(75.0, 150.0),
        p(75.0, 100.0),
    ];
    path = make_path2(&points39, &verbs39);
    reporter_assert!(reporter, !is_rect(&path, &mut rect));
    // isolated from zero_length_paths_aa
    let verbs_aa = [M, L, L, L, L, L, L, C];
    let points_aa = [
        p(32.0, 9.5),
        p(32.0, 9.5),
        p(32.0, 17.0),
        p(17.0, 17.0),
        p(17.0, 9.5),
        p(17.0, 2.0),
        p(32.0, 2.0),
    ];
    path = make_path2(&points_aa, &verbs_aa);
    reporter_assert!(reporter, is_rect(&path, &mut rect));
    compare.set_bounds(&points_aa);
    reporter_assert!(reporter, rect == compare);
    // isolated from skbug.com/40039046#c41
    let verbs41 = [M, L, L, L, L, M, C];
    let points41 = [
        p(75.0, 75.0),
        p(150.0, 75.0),
        p(150.0, 150.0),
        p(140.0, 150.0),
        p(140.0, 75.0),
        p(75.0, 75.0),
    ];
    path = make_path2(&points41, &verbs41);
    reporter_assert!(reporter, is_rect(&path, &mut rect));
    compare.set_bounds(&points41[1..5]);
    reporter_assert!(reporter, rect == compare);
    // isolated from skbug.com/40039046#c53
    let verbs53 = [M, L, L, L, L, M, C];
    let points53 = [
        p(75.0, 75.0),
        p(150.0, 75.0),
        p(150.0, 150.0),
        p(140.0, 150.0),
        p(140.0, 75.0),
        p(75.0, 75.0),
    ];
    path = make_path2(&points53, &verbs53);
    reporter_assert!(reporter, is_rect(&path, &mut rect));
    compare.set_bounds(&points53[1..5]);
    reporter_assert!(reporter, rect == compare);
}

def_test!(Path_isRect, |reporter| {
    path_is_rect_body(reporter);
});

// Port of: tests/PathTest.cpp#L5201-L5211 (chrome/m156)
def_test!(Path_increserve_handle_neg_crbug_883666, |_r| {
    let mut builder = PathBuilder::new();

    builder.conic_to((0.0, 0.0), (1.0, 1.0), f32::NEG_INFINITY);

    // <== use a copy path object to force SkPathRef::copy() and SkPathRef::resetToSize()
    let mut shallow_builder = builder.clone();

    // make sure we don't assert/crash on this.
    #[allow(clippy::cast_possible_wrap)] // incReserve(0xffffffff) passes int -1
    shallow_builder.inc_reserve(0xffff_ffff_u32 as i32, 0xffff_ffff_u32 as i32, 0);
});

////////////////////////////////////////////////////////////////////////////////////////////////

/*
 *  For speed, we tried to preserve useful/expensive attributes about paths,
 *      - convexity, isrect, isoval, ...
 *  Axis-aligned shapes (rect, oval, rrect) should survive, including convexity if the matrix
 *  is axis-aligned (e.g. scale+translate)
 */

// Port of: tests/PathTest.cpp#L5222-L5231 (chrome/m156)
struct Xforms {
    im: Matrix,
    tm: Matrix,
    sm: Matrix,
    rm: Matrix,
}

impl Xforms {
    fn new() -> Self {
        let mut im = Matrix::default();
        im.reset();
        let mut tm = Matrix::default();
        tm.set_translate((10.0, 20.0));
        let mut sm = Matrix::default();
        sm.set_scale((2.0, 3.0), None);
        let mut rm = Matrix::default();
        rm.set_rotate(30.0, None);
        Self { im, tm, sm, rm }
    }
}

// Port of: tests/PathTest.cpp#L5233-L5236 (chrome/m156)
fn nocompute_isconvex(path: &Path) -> bool {
    let convexity = path_priv::get_convexity_or_unknown(path);
    convexity.is_convex()
}

// expect axis-aligned shape to survive assignment, identity and scale/translate matrices
// Port of: tests/PathTest.cpp#L5238-L5277 (chrome/m156)
fn survive(
    path: &mut Path,
    x: &Xforms,
    is_axis_aligned: bool,
    reporter: &mut Reporter,
    isa_proc: impl Fn(&Path) -> bool,
) {
    reporter_assert!(reporter, isa_proc(path));
    // force the issue (computing convexity) the first time.
    reporter_assert!(reporter, path.is_convex());

    // a path's isa and convexity should survive assignment
    {
        let path2 = path.clone();
        reporter_assert!(reporter, isa_proc(&path2));
        reporter_assert!(reporter, nocompute_isconvex(&path2));
    }

    // a path's isa and convexity should identity transform
    *path = path.make_transform(&x.im);
    reporter_assert!(reporter, isa_proc(path));
    reporter_assert!(reporter, nocompute_isconvex(path));

    // a path's isa should survive translation, convexity depends on axis alignment
    *path = path.make_transform(&x.tm);
    reporter_assert!(reporter, isa_proc(path));
    reporter_assert!(reporter, nocompute_isconvex(path) == is_axis_aligned);

    // a path's isa should survive scaling, convexity depends on axis alignment
    *path = path.make_transform(&x.sm);
    reporter_assert!(reporter, isa_proc(path));
    reporter_assert!(reporter, nocompute_isconvex(path) == is_axis_aligned);

    // For security, post-rotation, we can't assume we're still convex. It might prove to be,
    // in fact, still be convex, be we can't have cached that setting, hence the call to
    // getConvexityOrUnknown() instead of getConvexity().
    *path = path.make_transform(&x.rm);
    reporter_assert!(reporter, !nocompute_isconvex(path));

    if is_axis_aligned {
        reporter_assert!(reporter, !isa_proc(path));
    }
}

// Port of: tests/PathTest.cpp#L5279-L5297 (chrome/m156)
def_test!(Path_survive_transform, |r| {
    let x = Xforms::new();

    let mut path = Path::rect(Rect::new(10.0, 10.0, 40.0, 50.0), None);
    survive(&mut path, &x, true, r, |p| p.is_rect().is_some());

    path = Path::oval(Rect::new(10.0, 10.0, 40.0, 50.0), None);
    survive(&mut path, &x, true, r, |p| p.is_oval().is_some());

    path = Path::rrect(
        RRect::new_rect_xy(Rect::new(10.0, 10.0, 40.0, 50.0), 5.0, 5.0),
        None,
    );
    survive(&mut path, &x, true, r, |p| p.is_rrect().is_some());

    // make a trapazoid; definitely convex, but not marked as axis-aligned (e.g. oval, rrect)
    path = PathBuilder::new()
        .move_to((0.0, 0.0))
        .line_to((100.0, 0.0))
        .line_to((70.0, 100.0))
        .line_to((30.0, 100.0))
        .detach();
    reporter_assert!(r, path.is_convex());
    survive(&mut path, &x, false, r, |_p| true);
});

// Port of: tests/PathTest.cpp#L5299-L5321 (chrome/m156)
fn test_edger(r: &mut Reporter, input: &[PathVerb], expected: &[PathVerb]) {
    let mut builder = PathBuilder::new();
    let mut x: scalar = 0.0;
    let mut y: scalar = 0.0;
    for &v in input {
        match v {
            PathVerb::Move => {
                builder.move_to((x, y));
                x += 1.0;
                y += 1.0;
            }
            PathVerb::Line => {
                builder.line_to((x, y));
                x += 1.0;
                y += 1.0;
            }
            PathVerb::Close => {
                builder.close();
            }
            _ => debug_assert!(false),
        }
    }
    let path = builder.detach();

    let mut iter = PathEdgeIter::from_path(&path);
    for &v in expected {
        let e = iter.next();
        reporter_assert!(r, e.is_some());
        reporter_assert!(r, e.is_some_and(|e| e.edge.to_verb() == v));
    }
    reporter_assert!(r, iter.next().is_none());
}

// Port of: tests/PathTest.cpp#L5323-L5328 (chrome/m156)
fn span_eq<T: PartialEq>(a: &[T], b: &[T]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).all(|(x, y)| x == y)
}

// Port of: tests/PathTest.cpp#L5330-L5335 (chrome/m156)
fn assert_points(reporter: &mut Reporter, path: &Path, list: &[Point]) {
    let praw = path_priv::raw(path, ResolveConvexity::No);
    reporter_assert!(reporter, praw.is_some());
    reporter_assert!(
        reporter,
        praw.is_some_and(|praw| span_eq(praw.points, list))
    );
}

// Port of: tests/PathTest.cpp#L5337-L5385 (chrome/m156)
fn test_add_rect_and_trailing_line_to(reporter: &mut Reporter) {
    let r = Rect::new(1.0, 2.0, 3.0, 4.0);
    // build our default p-array clockwise
    let p = [
        Point::new(r.left, r.top),
        Point::new(r.right, r.top),
        Point::new(r.right, r.bottom),
        Point::new(r.left, r.bottom),
    ];

    for dir in [PathDirection::CW, PathDirection::CCW] {
        let increment = if dir == PathDirection::CW { 1 } else { 3 };
        for i in 0..4 {
            let mut builder = PathBuilder::new();
            builder.add_rect(r, dir, i);
            let path = builder.snapshot();

            // check that we return the 4 ponts in the expected order
            let mut e = [Point::default(); 4];
            for (j, ej) in e.iter_mut().enumerate() {
                let index = (i + j * increment) % 4;
                *ej = p[index];
            }
            assert_points(reporter, &path, &[e[0], e[1], e[2], e[3]]);

            // check that the new line begins where the rect began
            builder.line_to((7.0, 8.0));
            let path = builder.snapshot();
            assert_points(
                reporter,
                &path,
                &[e[0], e[1], e[2], e[3], e[0], Point::new(7.0, 8.0)],
            );
        }
    }

    // now add a moveTo before the rect, just to be sure we don't always look at
    // the "first" point in the path when we handle the trailing lineTo
    let path = PathBuilder::new()
        .move_to((7.0, 8.0)) // will be replaced by rect's first moveTo
        .add_rect(r, PathDirection::CW, 2)
        .line_to((5.0, 6.0))
        .detach();

    assert_points(
        reporter,
        &path,
        &[
            p[2],
            p[3],
            p[0],
            p[1], // rect
            p[2],
            Point::new(5.0, 6.0), // trailing line
        ],
    );
}

// Port of: tests/PathTest.cpp#L5387-L5404 (chrome/m156)
def_test!(pathedger, |r| {
    let m = PathVerb::Move;
    let l = PathVerb::Line;
    let c = PathVerb::Close;

    test_edger(r, &[m], &[]);
    test_edger(r, &[m, m], &[]);
    test_edger(r, &[m, c], &[]);
    test_edger(r, &[m, m, c], &[]);
    test_edger(r, &[m, l], &[l, l]);
    test_edger(r, &[m, l, c], &[l, l]);
    test_edger(r, &[m, l, l], &[l, l, l]);
    test_edger(r, &[m, l, l, c], &[l, l, l]);

    test_edger(r, &[m, l, l, m, l, l], &[l, l, l, l, l, l]);

    test_add_rect_and_trailing_line_to(r);
});

// Port of: tests/PathTest.cpp#L5406-L5416 (chrome/m156)
def_test!(path_convexity_scale_way_down, |r| {
    let path = PathBuilder::new()
        .move_to((0.0, 0.0))
        .line_to((1.0, 0.0))
        .line_to((1.0, 1.0))
        .line_to((0.0, 1.0))
        .detach();

    reporter_assert!(r, path.is_convex());
    let scale: scalar = 1e-8;
    let path2 = path.make_transform(&Matrix::scale((scale, scale)));
    path_priv::force_compute_convexity(&path2);
    reporter_assert!(r, path2.is_convex());
});

// crbug.com/1187385
// Port of: tests/PathTest.cpp#L5419-L5451 (chrome/m156)
def_test!(path_moveto_addrect, |r| {
    // Test both an empty and non-empty rect passed to SkPath::addRect
    let rects = [
        Rect::new(207.0, 237.0, 300.0, 237.0),
        Rect::new(207.0, 237.0, 300.0, 267.0),
    ];

    for rect in rects {
        for num_extra_move_tos in 0..4 {
            let mut builder = PathBuilder::new();
            // Convexity and contains functions treat the path as a simple fill, so consecutive
            // moveTos are collapsed together.
            for i in 0..num_extra_move_tos {
                #[allow(clippy::cast_precision_loss)] // small int to float, as in C++
                let f = i as scalar;
                builder.move_to((f, f));
            }
            builder.add_rect(rect, None, None); // this will replace the prev moveTos
            let path = builder.detach();

            // addRect should mark the path as known convex automatically (i.e. it wasn't set
            // to unknown after edits)
            reporter_assert!(r, nocompute_isconvex(&path));

            // but it should also agree with the regular convexity computation
            path_priv::force_compute_convexity(&path);
            reporter_assert!(r, path.is_convex());

            let query = rect.with_inset((10.0, 0.0));
            let contains = path.conservatively_contains_rect(query);
            // if the rect we used to build the path was empty, then "containing" something
            // else is poorly defined -- so we only assert we contain the query if we're
            // non-empty (i.e. path with some area)
            reporter_assert!(r, rect.is_empty() || contains);
        }
    }
});

// crbug.com/1220754
// Port of: tests/PathTest.cpp#L5454-L5471 (chrome/m156)
def_test!(path_moveto_twopass_convexity, |r| {
    // There had been a bug when the last moveTo index > 0, the calculated point count was
    // incorrect and the BySign convexity pass would not evaluate the entire path, effectively
    // only using the winding rule for determining convexity.
    let path = PathBuilder::new_with_fill_type(PathFillType::Winding)
        .move_to((3.25, 115.5))
        .conic_to(
            (9.980_99e+17, 2.838_74e+15),
            (1.750_98e-30, 1.750_97e-30),
            1.053_85e+18,
        )
        .conic_to(
            (9.969_38e+17, 6.380_4e+19),
            (9.969_34e+17, 1.750_96e-30),
            1.750_96e-30,
        )
        .quad_to((1.288_86e+10, 9.964_7e+17), (9.981_01e+17, 2.610_06e+15))
        .detach();
    reporter_assert!(r, !path.is_convex());

    let path_with_extra_move_to = PathBuilder::new_with_fill_type(PathFillType::Winding)
        .move_to((5.900_43e-39, 1.345_25e-43))
        .add_path(&path, None)
        .detach();
    reporter_assert!(r, !path_with_extra_move_to.is_convex());
});

// Port of: tests/PathTest.cpp#L5512-L5528 (chrome/m156)
def_test!(path_filltype_utils, |r| {
    let p1 = PathBuilder::new()
        .line_to((42.0, 42.0))
        .line_to((42.0, 0.0))
        .close()
        .detach();

    reporter_assert!(r, p1.fill_type() == PathFillType::Winding);

    let p2 = p1.make_fill_type(PathFillType::EvenOdd);
    reporter_assert!(r, p2 != p1);
    reporter_assert!(r, p2.fill_type() == PathFillType::EvenOdd);

    let p3 = p2.with_toggle_inverse_fill_type();
    reporter_assert!(r, p3 != p2);
    reporter_assert!(r, p3.fill_type() == PathFillType::InverseEvenOdd);
});

// To test tight bounds, we ...
// 1. build some random paths that contains curves (that is the tricky part of tight bounds)
// 2. compute an approximation of "tight" bounds by evaluating the curves many times
// 3. ask path/builder/pathdata to compute the tight bounds, and then compare
//
// Port of: tests/PathTest.cpp#L5535-L5555 (chrome/m156)
#[allow(clippy::items_after_statements)] // constants stay next to the C++ code they mirror
fn make_random_builder(rand: &mut Random) -> PathBuilder {
    let mut rpoint = || -> Point {
        let x = rand.next_f() * 100.0;
        let y = rand.next_f() * 100.0;
        Point::new(x, y)
    };

    const N: usize = 9;
    let mut pts = [Point::default(); N];
    for p in &mut pts {
        *p = rpoint();
    }

    let mut bu = PathBuilder::new();
    bu.move_to(pts[0]);
    bu.line_to(pts[1]);
    bu.quad_to(pts[2], pts[3]);
    bu.conic_to(pts[4], pts[5], rand.next_f() * 2.0);
    bu.cubic_to(pts[6], pts[7], pts[8]);
    bu
}

// Port of: tests/PathTest.cpp#L5557-L5562 (chrome/m156)
fn update_bounds(r: &mut Rect, p: Point) {
    r.left = r.left.min(p.x); // std::fminf
    r.top = r.top.min(p.y);
    r.right = r.right.max(p.x); // std::fmaxf
    r.bottom = r.bottom.max(p.y);
}

// Port of: tests/PathTest.cpp#L5564-L5566 (chrome/m156)
fn update_bounds_line(r: &mut Rect, pts: &[Point]) {
    update_bounds(r, pts[1]);
}

const EVAL_LOOP_COUNT: usize = 1000;

#[allow(clippy::cast_precision_loss)] // (float)i / kEvalLoopCount
fn loop_t(i: usize) -> f32 {
    i as f32 / EVAL_LOOP_COUNT as f32
}

// Port of: tests/PathTest.cpp#L5570-L5576 (chrome/m156)
fn update_bounds_quad(r: &mut Rect, pts: &[Point]) {
    for i in 1..EVAL_LOOP_COUNT {
        let t = loop_t(i);
        update_bounds(r, eval_quad_at(pts, t));
    }
    update_bounds(r, pts[2]);
}

// Port of: tests/PathTest.cpp#L5578-L5585 (chrome/m156)
fn update_bounds_conic(r: &mut Rect, pts: &[Point], w: f32) {
    let conic = Conic::from_points(pts, w);
    for i in 1..EVAL_LOOP_COUNT {
        let t = loop_t(i);
        update_bounds(r, conic.eval_at(t));
    }
    update_bounds(r, pts[2]);
}

// Port of: tests/PathTest.cpp#L5587-L5595 (chrome/m156)
fn update_bounds_cubic(r: &mut Rect, pts: &[Point]) {
    for i in 1..EVAL_LOOP_COUNT {
        let t = loop_t(i);
        let mut point = Point::default();
        eval_cubic_at(pts, t, Some(&mut point), None, None);
        update_bounds(r, point);
    }
    update_bounds(r, pts[3]);
}

// Port of: tests/PathTest.cpp#L5597-L5625 (chrome/m156)
fn compute_tight_bounds(iter: PathIter<'_>) -> Rect {
    let mut r = Rect::default();
    let mut first = true;
    for rec in iter {
        match rec.verb() {
            PathVerb::Move => {
                if first {
                    r = Rect::bounds(rec.points()).expect("finite");
                    first = false;
                }
            }
            PathVerb::Line => update_bounds_line(&mut r, rec.points()),
            PathVerb::Quad => update_bounds_quad(&mut r, rec.points()),
            PathVerb::Conic => update_bounds_conic(&mut r, rec.points(), rec.conic_weight()),
            PathVerb::Cubic => update_bounds_cubic(&mut r, rec.points()),
            PathVerb::Close => {}
        }
    }
    r
}

// Port of: tests/PathTest.cpp#L5627-L5653 (chrome/m156)
def_test!(path_computeTightBounds, |reporter| {
    let mut rand = Random::default();

    for _ in 0..100 {
        let bu = make_random_builder(&mut rand);
        let tb = compute_tight_bounds(bu.iter());

        let nearly_eq = |reporter: &mut Reporter, a: &Rect, b: &Rect| {
            reporter_assert!(reporter, scalar::nearly_equal(a.left, b.left, None));
            reporter_assert!(reporter, scalar::nearly_equal(a.top, b.top, None));
            reporter_assert!(reporter, scalar::nearly_equal(a.right, b.right, None));
            reporter_assert!(reporter, scalar::nearly_equal(a.bottom, b.bottom, None));
        };

        let path = bu.snapshot();
        let pdata = PathData::make(bu.points(), bu.verbs(), bu.conic_weights());

        let r0 = bu.compute_tight_bounds().unwrap_or_else(Rect::new_empty);
        let r1 = path.compute_tight_bounds();
        let r2 = pdata.map(|d| d.compute_tight_bounds());

        reporter_assert!(reporter, r0 == r1);
        reporter_assert!(reporter, Some(r0) == r2);

        nearly_eq(reporter, &r0, &tb); // check against our approximation
    }
});

// Port of: tests/PathTest.cpp#L5655-L5698 (chrome/m156)
def_test!(path_trivial_isrect, |reporter| {
    struct Test {
        pts: [(f32, f32); 4],
        isrect: bool,
        expected_rect: Rect,
        expected_dir: PathDirection,
    }
    let t = |pts: [(f32, f32); 4], isrect: bool| Test {
        pts,
        isrect,
        expected_rect: Rect::new_empty(),
        expected_dir: PathDirection::DEFAULT,
    };
    let tests = [
        t([(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)], false),
        t(
            [(10.0, 10.0), (10.0, 10.0), (10.0, 10.0), (10.0, 10.0)],
            false,
        ),
        t(
            [(10.0, 10.0), (20.0, 10.0), (20.0, 10.0), (10.0, 10.0)],
            false,
        ),
        t(
            [(10.0, 10.0), (10.0, 30.0), (10.0, 30.0), (10.0, 10.0)],
            false,
        ),
        t(
            [(10.0, 10.0), (20.0, 30.0), (20.0, 30.0), (10.0, 10.0)],
            false,
        ),
        t(
            [(10.0, 10.0), (20.0, 10.0), (20.0, 30.0), (10.0, 20.0)],
            false,
        ),
        Test {
            pts: [(10.0, 10.0), (20.0, 10.0), (20.0, 30.0), (10.0, 30.0)],
            isrect: true,
            expected_rect: Rect::new(10.0, 10.0, 20.0, 30.0),
            expected_dir: PathDirection::CW,
        },
        Test {
            pts: [(10.0, 10.0), (10.0, 30.0), (20.0, 30.0), (20.0, 10.0)],
            isrect: true,
            expected_rect: Rect::new(10.0, 10.0, 20.0, 30.0),
            expected_dir: PathDirection::CCW,
        },
    ];

    for tst in &tests {
        for i in 0..4 {
            let path = PathBuilder::new()
                .move_to(tst.pts[i])
                .line_to(tst.pts[(i + 1) % 4])
                .line_to(tst.pts[(i + 2) % 4])
                .line_to(tst.pts[(i + 3) % 4])
                .close()
                .detach();

            let result = path.is_rect();
            reporter_assert!(reporter, result.is_some() == tst.isrect);
            if !tst.isrect {
                continue;
            }
            let Some((rect, closed, dir)) = result else {
                continue;
            };

            reporter_assert!(reporter, rect == tst.expected_rect);
            reporter_assert!(reporter, closed);
            reporter_assert!(reporter, dir == tst.expected_dir);
        }
    }
});

// Port of: tests/PathTest.cpp#L5700-L5743 (chrome/m156)
def_test!(path_infinite_transform, |reporter| {
    const COORD: f32 = 1000.0;

    let path = Path::circle((0.0, 0.0), COORD, None);
    reporter_assert!(reporter, path.is_finite());
    reporter_assert!(
        reporter,
        *path.bounds() == Rect::new(-COORD, -COORD, COORD, COORD)
    );

    let scales = [1.0f32, 1e12, 1e24, 1e36];
    debug_assert!(!is_finite(scales[3] * COORD)); // make sure the last one overflows

    let check_finite = |reporter: &mut Reporter, p: &Path| {
        let r = *p.bounds();
        reporter_assert!(reporter, p.is_finite());
        reporter_assert!(reporter, r.is_finite());
        reporter_assert!(reporter, !r.is_empty());
    };

    let check_infinite = |reporter: &mut Reporter, p: &Path| {
        let r = *p.bounds();
        reporter_assert!(reporter, !p.is_finite());
        reporter_assert!(reporter, r.is_empty());
    };

    for scale in scales {
        let mx = Matrix::scale((scale, scale));

        if is_finite(scale * COORD) {
            let maybe = path.try_make_transform(&mx);
            reporter_assert!(reporter, maybe.is_some());
            let Some(maybe) = maybe else { continue };
            check_finite(reporter, &maybe);

            let newpath = path.make_transform(&mx);
            check_finite(reporter, &newpath);

            reporter_assert!(reporter, newpath == maybe);
        } else {
            let maybe = path.try_make_transform(&mx);
            reporter_assert!(reporter, maybe.is_none());

            let newpath = path.make_transform(&mx);
            check_infinite(reporter, &newpath);
        }
    }
});

// Port of: tests/PathTest.cpp#L5745-L5753 (chrome/m156)
def_test!(path_factory_inverted_bounds, |reporter| {
    let bounds = Rect::new(-10.0, -10.0, 10.0, 10.0);
    let inverted_bounds = Rect::new(10.0, 10.0, -10.0, -10.0);

    reporter_assert!(
        reporter,
        *Path::oval(inverted_bounds, None).bounds() == bounds
    );
    reporter_assert!(
        reporter,
        *Path::rect(inverted_bounds, None).bounds() == bounds
    );
    reporter_assert!(
        reporter,
        *Path::rrect(RRect::new_rect(inverted_bounds), None).bounds() == bounds
    );
});

// Port of: tests/PathTest.cpp#L5755-L5825 (chrome/m156)
def_test!(path_trailing_moves_bounds, |reporter| {
    let pt = Point::new(3.0, 4.0);
    let vb = PathVerb::Move;

    // A single move is reflected in the bounds

    let mut bu = PathBuilder::new();
    bu.move_to(pt);
    let mut r = bu.compute_bounds();
    let mut r2 = Rect::new(pt.x, pt.y, pt.x, pt.y);
    reporter_assert!(reporter, r == r2);

    let mut path = bu.detach();
    reporter_assert!(reporter, path.points().len() == 1);
    reporter_assert!(reporter, path.points().last() == Some(&pt));
    r = *path.bounds();
    reporter_assert!(reporter, r == r2);

    path = Path::raw(&[pt], &[vb], &[], PathFillType::DEFAULT, None);
    reporter_assert!(reporter, path.points().len() == 1);
    reporter_assert!(reporter, path.points().last() == Some(&pt));
    r = *path.bounds();
    reporter_assert!(reporter, r == r2);

    // A trailing move, but not the only contour, does not affect the bounds

    bu.move_to((1.0, 2.0));
    bu.line_to((3.0, 4.0)); // same as pt
    let last_pt = Point::new(5.0, 6.0);
    bu.move_to(last_pt);
    r2 = Rect::new(1.0, 2.0, 3.0, 4.0); // does not include last point
    r = bu.compute_bounds();
    reporter_assert!(reporter, r == r2);

    path = bu.detach();
    reporter_assert!(reporter, path.points().len() == 3);
    reporter_assert!(reporter, path.points().last() == Some(&last_pt));
    r = *path.bounds();
    reporter_assert!(reporter, r == r2);

    let pts2 = [
        Point::new(1.0, 2.0),
        Point::new(3.0, 4.0),
        Point::new(5.0, 6.0),
    ];
    let vbs2 = [PathVerb::Move, PathVerb::Line, PathVerb::Move];
    path = Path::raw(&pts2, &vbs2, &[], PathFillType::DEFAULT, None);
    reporter_assert!(reporter, path.points().len() == 3);
    reporter_assert!(reporter, path.points().last() == Some(&last_pt));
    r = *path.bounds();
    reporter_assert!(reporter, r == r2);

    // make sure the trailer survives a round-trip

    let check_trip = |reporter: &mut Reporter, path: &Path| {
        let path_bounds = *path.bounds();

        let mut bu = PathBuilder::new();
        bu.add_path(path, None);
        reporter_assert!(reporter, bu.verbs().len() == path.verbs().len());
        reporter_assert!(reporter, bu.verbs().last() == Some(&PathVerb::Move));
        let mut r = bu.compute_bounds();
        reporter_assert!(reporter, r == path_bounds);

        let mx = Matrix::translate((1.0, 2.0));
        bu.reset().add_path_with_transform(path, &mx, None);
        reporter_assert!(reporter, bu.verbs().len() == path.verbs().len());
        reporter_assert!(reporter, bu.verbs().last() == Some(&PathVerb::Move));
        r = bu.compute_bounds();
        reporter_assert!(reporter, r == path_bounds.with_offset((1.0, 2.0)));
    };

    check_trip(reporter, &bu.reset().move_to((1.0, 2.0)).detach());
    check_trip(
        reporter,
        &bu.reset()
            .move_to((1.0, 2.0))
            .line_to((3.0, 4.0))
            .move_to((5.0, 6.0))
            .detach(),
    );
});

// https://issues.oss-fuzz.com/issues/470703863
// Port of: tests/PathTest.cpp#L5828-L5836 (chrome/m156)
def_test!(path_read_from_memory_corrupt_rrect, |reporter| {
    const NUM_BYTES: usize = 4;
    let data: [u8; NUM_BYTES] = [4, 43, 61, 16];

    let (result, bytes_read) = Path::read_from_memory(&data);
    reporter_assert!(reporter, result.is_none());
    reporter_assert!(reporter, bytes_read == 0);
});

// Port of: tests/PathTest.cpp#L5838-L5843 (chrome/m156)
def_test!(path_empty_iter, |reporter| {
    let p = Path::new();

    let mut iter = Iter::new(&p, false);
    reporter_assert!(reporter, iter.next_rec().is_none());
});

// Port of: tests/PathTest.cpp#L5845-L5860 (chrome/m156)
def_test!(
    #[allow(clippy::excessive_precision)] // float literals are copied verbatim from the C++
    path_b511244869,
    |reporter| {
        let path = PathBuilder::new()
            .move_to((2.115_212_95e+37, 15.913_093_6))
            .conic_to(
                (2.115_212_95e+37, 15.913_093_6),
                (-1.905_395_68e+38, 15.913_093_6),
                0.0,
            )
            .detach();

        let mut m = Matrix::default();
        m.set_all(
            1.356_316e-19,
            1.356_316e-19,
            1.356_316e-19,
            1.356_316e-19,
            -2.128_433e+38,
            1.356_316e-19,
            1.152_428e-41,
            0.000_000e+00,
            0.000_000e+00,
        );

        // This should not crash or trigger an assertion in debug builds.
        let transformed = path.make_transform(&m);
        reporter_assert!(reporter, transformed.is_empty());
    }
);

// Port of: tests/PathTest.cpp#L5862-L5872 (chrome/m156)
def_test!(Path_snapshot_rrect_rotation, |reporter| {
    let mut builder = PathBuilder::new();
    builder.add_rrect(
        RRect::new_rect_xy(Rect::new(0.0, 0.0, 100.0, 100.0), 10.0, 10.0),
        None,
        None,
    );

    // A rotation matrix that is not axis-aligned
    let matrix = Matrix::rotate_deg(45.0);
    let path = builder.snapshot_and_transform(&matrix);

    // Should NOT be recognized as an RRect because it's no longer axis-aligned.
    reporter_assert!(reporter, path.is_rrect().is_none());
});

// Port of: tests/PathTest.cpp#L5874-L5885 (chrome/m156)
def_test!(Path_snapshot_rrect_perspective, |reporter| {
    let mut builder = PathBuilder::new();
    builder.add_rrect(
        RRect::new_rect_xy(Rect::new(0.0, 0.0, 100.0, 100.0), 10.0, 10.0),
        None,
        None,
    );

    // A perspective matrix
    let mut matrix = Matrix::default();
    matrix.set_persp_x(0.01);
    let path = builder.snapshot_and_transform(&matrix);

    // Should NOT be recognized as an RRect because it's twisted
    reporter_assert!(reporter, path.is_rrect().is_none());
});

// Port of: tests/PathTest.cpp#L5887-L5896 (chrome/m156)
def_test!(Path_addRRect_with_move, |reporter| {
    let mut builder = PathBuilder::new();
    builder.move_to((0.0, 0.0));
    builder.close(); // make sure it's not a trailing move
    builder.add_rrect(
        RRect::new_rect_xy(Rect::new(0.0, 0.0, 100.0, 100.0), 10.0, 10.0),
        None,
        None,
    );

    let path = builder.detach();
    // Not an RRect because something comes between the initial move and the rrect.
    reporter_assert!(reporter, path.is_rrect().is_none());
});

// Port of: tests/PathTest.cpp#L5898-L5928 (chrome/m156)
def_test!(Path_snapshot_rrect_success, |reporter| {
    // 1. Translation & Scale (preserves winding direction)
    {
        let mut builder = PathBuilder::new();
        builder.add_rrect(
            RRect::new_rect_xy(Rect::new(10.0, 20.0, 110.0, 120.0), 10.0, 10.0),
            PathDirection::CW,
            0,
        );

        let mut matrix = Matrix::translate((50.0, 100.0));
        matrix.post_scale((2.0, 3.0), None);
        let path = builder.snapshot_and_transform(&matrix);

        reporter_assert!(reporter, path.is_rrect().is_some());
        let info = path_priv::is_rrect(&path);
        reporter_assert!(reporter, info.is_some());
        reporter_assert!(
            reporter,
            info.is_some_and(|i| i.direction == PathDirection::CW)
        );
        reporter_assert!(reporter, info.is_some_and(|i| i.start_index == 0));
    }

    // 2. Mirror/Flip (reverses winding direction)
    {
        let mut builder = PathBuilder::new();
        builder.add_rrect(
            RRect::new_rect_xy(Rect::new(10.0, 20.0, 110.0, 120.0), 10.0, 10.0),
            PathDirection::CW,
            0,
        );

        let matrix = Matrix::scale((-1.0, 1.0));
        let path = builder.snapshot_and_transform(&matrix);

        reporter_assert!(reporter, path.is_rrect().is_some());
        let info = path_priv::is_rrect(&path);
        reporter_assert!(reporter, info.is_some());
        reporter_assert!(
            reporter,
            info.is_some_and(|i| i.direction == PathDirection::CCW)
        );
    }
});

// Port of: tests/PathTest.cpp#L5930-L5949 (chrome/m156)
def_test!(Fuzz_b464232697_ExtremeStrokeBounds, |reporter| {
    // b/464232697: cubic path with coordinates reaching >7 billion in svg_dom
    let path = PathBuilder::new()
        .move_to((5.0, -0.93))
        .cubic_to((7.0, 8088.0), (4_473_540.0, 6.0), (311.0, 7_245_220_098.0))
        .line_to((0.0, 574_404_044.0))
        .detach();

    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(37.002);

    let mut dst_builder = PathBuilder::new();
    let success = fill_path_with_paint(&path, &paint, &mut dst_builder, None, None);
    // skia-rust: SK_BUILD_FOR_FUZZER is not defined, so only the non-fuzzer expectation applies.
    reporter_assert!(reporter, success);
});

// Port of: tests/PathTest.cpp#L5951-L5968 (chrome/m156)
def_test!(Fuzz_b42534575_ExtremeStrokeBounds, |reporter| {
    // b/42534575: cubic path with astronomical coordinates reaching ~1e19 in api_draw_functions
    // skia-rust: the float literals are the shortest forms of the C++ ones that denote the same
    // f32 values (clippy::excessive_precision).
    let path = PathBuilder::new()
        .move_to((1.362_683_7E+19, -1.537_513_5E+19))
        .cubic_to((1.4E-45, 0.0), (0.0, 0.0), (0.0, 0.0))
        .detach();
    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(3.226_380_3E+19);

    let mut dst_builder = PathBuilder::new();
    let success = fill_path_with_paint(&path, &paint, &mut dst_builder, None, None);
    // skia-rust: SK_BUILD_FOR_FUZZER is not defined, so only the non-fuzzer expectation applies.
    reporter_assert!(reporter, success);
});
