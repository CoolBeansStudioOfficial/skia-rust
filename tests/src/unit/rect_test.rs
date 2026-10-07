// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RectTest.cpp (chrome/m156)
//
// Not ported yet (manifest stays `todo`): `Rect`, `Rect_grow` (SkBitmap, SkCanvas, SkPaint),
// `Rect_path_nan` (SkPath) and `big_tiled_rect_crbug_927075` (SkSurface, SkCanvas). Their helpers `has_green_pixels`,
// `test_stroke_width_clipping` and `test_skbug4406` go with `Rect`/`Rect_grow`.

#![cfg(test)]

use skia_rust_core::floating_point::is_finite;
use skia_rust_core::m44::{M44, V3, V4};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::rect_priv::subtract;
use skia_rust_core::rect::rect_priv::{
    closest_disjoint_edge, half_height, half_width, make_i_large, make_i_largest_inverted,
    make_large_s32, make_largest, make_largest_inverted, quad_contains_rect,
    quad_contains_rect_m44, subtract_irect,
};
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::scalar::{SCALAR_INFINITY, SCALAR_MAX, SCALAR_NAN, SCALAR_PI, scalar};

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/RectTest.cpp#L119-L125 (chrome/m156)
def_test!(Rect_largest, |reporter| {
    reporter_assert!(reporter, !make_i_large().is_empty());
    reporter_assert!(reporter, make_i_largest_inverted().is_empty());
    reporter_assert!(reporter, !make_largest().is_empty());
    reporter_assert!(reporter, !make_large_s32().is_empty());
    reporter_assert!(reporter, make_largest_inverted().is_empty());
});

/*
 *  Test the setBounds always handles non-finite values correctly:
 *  - setBoundsCheck should return false, and set the rect to all zeros
 *  - setBoundsNoCheck should ensure that rect.isFinite() is false (definitely NOT all zeros)
 */
// Port of: tests/RectTest.cpp#L133-L161 (chrome/m156)
def_test!(Rect_setbounds, |reporter| {
    let p0 = [
        Point::new(SCALAR_INFINITY, 0.0),
        Point::new(1.0, 1.0),
        Point::new(2.0, 2.0),
        Point::new(3.0, 3.0),
    ];
    let p1 = [
        Point::new(0.0, SCALAR_INFINITY),
        Point::new(1.0, 1.0),
        Point::new(2.0, 2.0),
        Point::new(3.0, 3.0),
    ];
    let p2 = [
        Point::new(SCALAR_NAN, 0.0),
        Point::new(1.0, 1.0),
        Point::new(2.0, 2.0),
        Point::new(3.0, 3.0),
    ];
    let p3 = [
        Point::new(0.0, SCALAR_NAN),
        Point::new(1.0, 1.0),
        Point::new(2.0, 2.0),
        Point::new(3.0, 3.0),
    ];

    let mut r = Rect::default();
    let zeror = Rect::new(0.0, 0.0, 0.0, 0.0);

    for pts in [&p0, &p1, &p2, &p3] {
        for n in 1..=4_usize {
            let isfinite = r.set_bounds_check(&pts[..n]);
            reporter_assert!(reporter, !isfinite);
            reporter_assert!(reporter, r == zeror);

            r.set_bounds_no_check(&pts[..n]);
            if r.is_finite() {
                r.set_bounds_no_check(&pts[..n]);
            }
            reporter_assert!(reporter, !r.is_finite());
        }
    }
});

// Port of: tests/RectTest.cpp#L146-L151 (chrome/m156)
fn make_big_value(_reporter: &Reporter) -> f32 {
    // need to make a big value, one that will cause rect.width() to overflow to inf.
    // however, the windows compiler wants about this if it can see the big value inlined.
    // hence, this stupid trick to try to fool their compiler.
    SCALAR_MAX * 0.75
}

// Port of: tests/RectTest.cpp#L163-L180 (chrome/m156)
def_test!(Rect_whOverflow, |reporter| {
    let big: scalar = make_big_value(reporter);

    let r = Rect::new(-big, -big, big, big);

    reporter_assert!(reporter, r.is_finite());
    reporter_assert!(reporter, !is_finite(r.width()));
    reporter_assert!(reporter, !is_finite(r.height()));

    // ensure we can compute center even when the width/height might overflow
    reporter_assert!(reporter, is_finite(r.center_x()));
    reporter_assert!(reporter, is_finite(r.center_y()));

    // ensure we can compute halfWidth and halfHeight even when width/height might overflow,
    // i.e. for use computing the radii filling a rectangle.
    reporter_assert!(reporter, is_finite(half_width(&r)));
    reporter_assert!(reporter, is_finite(half_height(&r)));
});

// Port of: tests/RectTest.cpp#L182-L253 (chrome/m156)
def_test!(Rect_subtract, |reporter| {
    #[allow(clippy::struct_field_names)] // mirrors the C++ struct's fA/fB/fExpected/fExact
    struct Expectation {
        f_a: IRect,
        f_b: IRect,
        f_expected: IRect,
        f_exact: bool,
    }

    let e = |a: IRect, b: IRect, expected: IRect, exact: bool| Expectation {
        f_a: a,
        f_b: b,
        f_expected: expected,
        f_exact: exact,
    };

    let a = IRect::from_ltrb(2, 3, 12, 15);
    let tests = [
        // B contains A == empty rect
        e(a, a.with_outset((2, 2)), IRect::new_empty(), true),
        // A contains B, producing 4x12 (left), 2x12 (right), 4x10(top), and 5x10(bottom)
        e(
            a,
            IRect::new(6, 6, 10, 10),
            IRect::new(2, 10, 12, 15),
            false,
        ),
        // A is empty, B is not == empty rect
        e(IRect::new_empty(), a, IRect::new_empty(), true),
        // A is not empty, B is empty == a
        e(a, IRect::new_empty(), a, true),
        // A and B are empty == empty
        e(
            IRect::new_empty(),
            IRect::new_empty(),
            IRect::new_empty(),
            true,
        ),
        // A and B do not intersect == a
        e(a, IRect::new(15, 17, 20, 40), a, true),
        // B cuts off left side of A, producing 6x12 (right)
        e(a, IRect::new(0, 0, 6, 20), IRect::new(6, 3, 12, 15), true),
        // B cuts off right side of A, producing 4x12 (left)
        e(a, IRect::new(6, 0, 20, 20), IRect::new(2, 3, 6, 15), true),
        // B cuts off top side of A, producing 10x9 (bottom)
        e(a, IRect::new(0, 0, 20, 6), IRect::new(2, 6, 12, 15), true),
        // B cuts off bottom side of A, producing 10x7 (top)
        e(a, IRect::new(0, 10, 20, 20), IRect::new(2, 3, 12, 10), true),
        // B splits A horizontally, producing 10x3 (top) or 10x5 (bottom)
        e(
            a,
            IRect::new(0, 6, 20, 10),
            IRect::new(2, 10, 12, 15),
            false,
        ),
        // B splits A vertically, producing 4x12 (left) or 2x12 (right)
        e(a, IRect::new(6, 0, 10, 20), IRect::new(2, 3, 6, 15), false),
        // B cuts top-left of A, producing 8x12 (right) or 10x11 (bottom)
        e(a, IRect::new(0, 0, 4, 4), IRect::new(2, 4, 12, 15), false),
        // B cuts top-right of A, producing 8x12 (left) or 10x8 (bottom)
        e(a, IRect::new(10, 0, 14, 7), IRect::new(2, 3, 10, 15), false),
        // B cuts bottom-left of A, producing 7x12 (right) or 10x9 (top)
        e(a, IRect::new(0, 12, 5, 20), IRect::new(2, 3, 12, 12), false),
        // B cuts bottom-right of A, producing 8x12 (left) or 10x9 (top)
        e(
            a,
            IRect::new(10, 12, 20, 20),
            IRect::new(2, 3, 10, 15),
            false,
        ),
        // B crosses the left of A, producing 4x12 (right) or 10x3 (top) or 10x5 (bottom)
        e(a, IRect::new(0, 6, 8, 10), IRect::new(2, 10, 12, 15), false),
        // B crosses the right side of A, producing 6x12 (left) or 10x3 (top) or 10x5 (bottom)
        e(a, IRect::new(8, 6, 20, 10), IRect::new(2, 3, 8, 15), false),
        // B crosses the top side of A, producing 4x12 (left) or 2x12 (right) or 10x8 (bottom)
        e(a, IRect::new(6, 0, 10, 7), IRect::new(2, 7, 12, 15), false),
        // B crosses the bottom side of A, producing 1x12 (left) or 4x12 (right) or 10x3 (top)
        e(a, IRect::new(4, 6, 8, 20), IRect::new(8, 3, 12, 15), false),
    ];

    for e in &tests {
        let mut difference = IRect::default();
        let mut exact = subtract_irect(&e.f_a, &e.f_b, &mut difference);
        reporter_assert!(reporter, exact == e.f_exact);
        reporter_assert!(reporter, difference == e.f_expected);

        // Generate equivalent tests for the SkRect case by moving the input rects by 0.5px
        let mut af = Rect::from_irect(e.f_a);
        let mut bf = Rect::from_irect(e.f_b);
        let mut ef = Rect::from_irect(e.f_expected);
        af.offset((0.5, 0.5));
        bf.offset((0.5, 0.5));
        ef.offset((0.5, 0.5));

        let mut df = Rect::default();
        exact = subtract(&af, &bf, &mut df);
        reporter_assert!(reporter, exact == e.f_exact);
        reporter_assert!(reporter, (df.is_empty() && ef.is_empty()) || (df == ef));
    }
});

// Port of: tests/RectTest.cpp#L255-L279 (chrome/m156)
def_test!(Rect_subtract_overflow, |reporter| {
    // This rectangle is sorted but whose int32 width overflows and appears negative (so
    // isEmpty() returns true).
    let really_big = IRect::from_ltrb(-i32::MAX + 1000, 0, i32::MAX - 1000, 100);
    // However, because it's sorted, an intersection with a reasonably sized rectangle is still
    // valid so the assumption that SkIRect::Intersects() returns false when either input is
    // empty is invalid, leading to incorrect use of negative width (see crbug.com/1243206)
    let reasonable = IRect::from_ltrb(-50, -5, 50, 125);

    // Ignoring overflow, "reallyBig - reasonable" should report exact = false and select either
    // the left or right portion of 'reallyBig' that excludes 'reasonable', e.g.
    // {-INT_MAX+1000, 0, -50, 100} or {150, 0, INT_MAX-1000, 100}.
    // This used to assert, but now it should be detected that 'reallyBig' overflows and is
    // technically empty, so the result should be itself and exact.
    let mut difference = IRect::default();
    let mut exact = subtract_irect(&really_big, &reasonable, &mut difference);
    reporter_assert!(reporter, exact);
    reporter_assert!(reporter, difference == really_big);

    // Similarly, if we subtract 'reallyBig', since it's technically empty then we expect the
    // answer to remain 'reasonable'.
    exact = subtract_irect(&reasonable, &really_big, &mut difference);
    reporter_assert!(reporter, exact);
    reporter_assert!(reporter, difference == reasonable);
});

// Port of: tests/RectTest.cpp#L281-L396 (chrome/m156)
def_test!(Rect_QuadContainsRect, |reporter| {
    struct TestCase {
        label: &'static str,
        expect: bool,
        m: Matrix,
        a: IRect,
        b: IRect,
        tol: f32,
    }

    let tc =
        |label: &'static str, expect: bool, m: Matrix, a: IRect, b: IRect, tol: f32| TestCase {
            label,
            expect,
            m,
            a,
            b,
            tol,
        };

    let epsilon_matrix = {
        let mut m = Matrix::new_all(
            0.984_808, 0.173_648, -98.4808, -0.173_648, 0.984_808, 17.3648, 0.000_000, 0.000_000,
            1.0000,
        );
        m.pre_translate((65.0, 0.0));
        m
    };

    let tests = [
        tc(
            "Identity matrix contains success",
            true,
            Matrix::i().clone(),
            IRect::new(0, 0, 15, 15),
            IRect::new(2, 2, 10, 10),
            0.0,
        ),
        tc(
            "Identity matrix contains failure",
            false,
            Matrix::i().clone(),
            IRect::new(0, 0, 15, 15),
            IRect::new(-2, -2, 10, 10),
            0.0,
        ),
        tc(
            "Identity mapped rect contains itself",
            true,
            Matrix::i().clone(),
            IRect::new(0, 0, 10, 10),
            IRect::new(0, 0, 10, 10),
            0.0,
        ),
        tc(
            "Scaled rect contains success",
            true,
            Matrix::scale((2.0, 3.4)),
            IRect::new(0, 0, 4, 4),
            IRect::new(1, 1, 6, 6),
            0.0,
        ),
        tc(
            "Scaled rect contains failure",
            false,
            Matrix::scale((0.25, 0.3)),
            IRect::new(0, 0, 8, 8),
            IRect::new(0, 0, 5, 5),
            0.0,
        ),
        tc(
            "Rotate rect contains success",
            true,
            Matrix::rotate_deg_pivot(45.0, (10.0, 10.0)),
            IRect::new(0, 0, 20, 20),
            IRect::new(3, 3, 17, 17),
            0.0,
        ),
        tc(
            "Rotate rect contains failure",
            false,
            Matrix::rotate_deg_pivot(45.0, (10.0, 10.0)),
            IRect::new(0, 0, 20, 20),
            IRect::new(2, 2, 18, 18),
            0.0,
        ),
        tc(
            "Negative scale contains success",
            true,
            Matrix::scale((-1.0, 1.0)),
            IRect::new(0, 0, 10, 10),
            IRect::new(-9, 1, -1, 9),
            0.0,
        ),
        tc(
            "Empty rect contains nothing",
            false,
            Matrix::rotate_deg_pivot(45.0, (0.0, 0.0)),
            IRect::new(10, 10, 10, 20),
            IRect::new(10, 14, 10, 16),
            0.0,
        ),
        tc(
            "MakeEmpty() contains nothing",
            false,
            Matrix::rotate_deg_pivot(45.0, (0.0, 0.0)),
            IRect::new_empty(),
            IRect::new(0, 0, 1, 1),
            0.0,
        ),
        tc(
            "Unsorted rect contains nothing",
            false,
            Matrix::i().clone(),
            IRect::new(10, 10, 0, 0),
            IRect::new(2, 2, 8, 8),
            0.0,
        ),
        tc(
            "Unsorted rect is contained",
            true,
            Matrix::i().clone(),
            IRect::new(0, 0, 10, 10),
            IRect::new(8, 8, 2, 2),
            0.0,
        ),
        // NOTE: preTranslate(65.f, 0.f) gives enough of a different matrix that the contains()
        // passes even without the epsilon allowance.
        tc(
            "Epsilon not contained",
            true,
            epsilon_matrix,
            IRect::new(0, 0, 134, 215),
            IRect::new(0, 0, 100, 200),
            0.001,
        ),
    ];

    for t in &tests {
        // `skiatest::ReporterContext c{reporter, t.label}`: the label is added to each message.
        reporter_assert!(
            reporter,
            quad_contains_rect(&t.m, &t.a, &t.b, t.tol) == t.expect,
            "{}",
            t.label
        );

        // Generate equivalent tests for SkRect and SkM44 by translating a by 1/2px and 'b' by
        // 1/2px in post-transform space
        let mut b_offset: Vector = t.m.map_vector((0.5, 0.5));
        let mut af = Rect::from_irect(t.a).with_offset((0.5, 0.5));
        let mut bf = Rect::from_irect(t.b).with_offset((b_offset.x, b_offset.y));
        reporter_assert!(
            reporter,
            quad_contains_rect_m44(&M44::from(&t.m), &af, &bf, t.tol) == t.expect,
            "{}",
            t.label
        );

        if t.tol != 0.0 {
            // Expect the opposite result if we do not provide any tol.
            reporter_assert!(
                reporter,
                quad_contains_rect(&t.m, &t.a, &t.b, 0.0) != t.expect,
                "{}",
                t.label
            );

            b_offset = t.m.map_vector((0.5, 0.5));
            af = Rect::from_irect(t.a).with_offset((0.5, 0.5));
            bf = Rect::from_irect(t.b).with_offset((b_offset.x, b_offset.y));
            reporter_assert!(
                reporter,
                quad_contains_rect_m44(&M44::from(&t.m), &af, &bf, 0.0) != t.expect,
                "{}",
                t.label
            );
        }
    }

    // Test some more complicated scenarios with perspective that don't fit into the TestCase
    // structure as nicely.
    let a = Rect::new(1.83, -0.48, 15.53, 30.68); // arbitrary

    // Perspective matrix where the mapped A has all corners' W > 0
    {
        let label = "Perspective, W > 0";
        let mut p = M44::perspective(0.01, 10.0, SCALAR_PI / 3.0);
        p.pre_translate(0.0, 5.0, -0.1);
        p.pre_concat(&M44::rotate(
            V3::new(0.0, 1.0, 0.0),
            0.008, /* radians */
        ));
        reporter_assert!(
            reporter,
            quad_contains_rect_m44(&p, &a, &Rect::new(4.0, 10.0, 20.0, 45.0), 0.0),
            "{}",
            label
        );
        reporter_assert!(
            reporter,
            !quad_contains_rect_m44(&p, &a, &Rect::new(2.0, 6.0, 23.0, 50.0), 0.0),
            "{}",
            label
        );
    }
    // Perspective matrix where the mapped A has some corners' W < 0
    {
        let label = "Perspective, some W > 0";
        let mut p = M44::default();
        p.set_row(3, &V4::new(-0.2, -0.6, 0.0, 8.0));
        reporter_assert!(
            reporter,
            quad_contains_rect_m44(&p, &a, &Rect::new(10.0, 50.0, 20.0, 60.0), 0.0),
            "{}",
            label
        );
        reporter_assert!(
            reporter,
            !quad_contains_rect_m44(&p, &a, &Rect::new(0.0, 1.0, 10.0, 10.0), 0.0),
            "{}",
            label
        );
    }
    // Perspective matrix where the mapped A has all corners' W < 0)
    // For B, we use the previous success contains query above; a rectangle that is inside the
    // convex hull of the mapped corners of A, projecting each corner with its negative W; and a
    // rectangle that contains said convex hull.
    {
        let label = "Perspective, no W > 0";
        let mut p = M44::default();
        p.set_row(3, &V4::new(-0.2, -0.6, 0.0, 8.0));
        let na = a.with_offset((16.0, 31.0));
        reporter_assert!(
            reporter,
            !quad_contains_rect_m44(&p, &na, &Rect::new(10.0, 50.0, 20.0, 60.0), 0.0),
            "{}",
            label
        );
        reporter_assert!(
            reporter,
            !quad_contains_rect_m44(&p, &na, &Rect::new(-1.1, -1.8, -1.0, -1.79), 0.0),
            "{}",
            label
        );
        reporter_assert!(
            reporter,
            !quad_contains_rect_m44(&p, &na, &Rect::new(-1.9, -2.3, -0.4, -1.6), 0.0),
            "{}",
            label
        );
    }
});

// Port of: tests/RectTest.cpp#L398-L438 (chrome/m156)
def_test!(Rect_ClosestDisjointEdge, |r| {
    // All test cases will use this rect for the src, so dst can be conveniently relative to it.
    const K_SRC: IRect = IRect::new(0, 0, 10, 10);

    struct TestCase {
        label: &'static str,
        dst: IRect,
        expect: IRect,
    }

    let tc = |label: &'static str, dst: IRect, expect: IRect| TestCase { label, dst, expect };

    let tests = [
        tc(
            "src left edge",
            IRect::new(-15, -5, -2, 15),
            IRect::new(0, 0, 1, 10),
        ),
        tc(
            "src left edge clipped to dst",
            IRect::new(-15, 2, -2, 8),
            IRect::new(0, 2, 1, 8),
        ),
        tc(
            "src top-left corner",
            IRect::new(-15, -15, -2, -2),
            IRect::new(0, 0, 1, 1),
        ),
        tc(
            "src top edge",
            IRect::new(-5, -10, 15, -2),
            IRect::new(0, 0, 10, 1),
        ),
        tc(
            "src top edge clipped to dst",
            IRect::new(2, -10, 8, -2),
            IRect::new(2, 0, 8, 1),
        ),
        tc(
            "src top-right corner",
            IRect::new(15, -15, 20, -2),
            IRect::new(9, 0, 10, 1),
        ),
        tc(
            "src right edge",
            IRect::new(15, -5, 20, 15),
            IRect::new(9, 0, 10, 10),
        ),
        tc(
            "src right edge clipped to dst",
            IRect::new(15, 2, 20, 8),
            IRect::new(9, 2, 10, 8),
        ),
        tc(
            "src bottom-right corner",
            IRect::new(15, 15, 20, 20),
            IRect::new(9, 9, 10, 10),
        ),
        tc(
            "src bottom edge",
            IRect::new(-5, 15, 15, 20),
            IRect::new(0, 9, 10, 10),
        ),
        tc(
            "src bottom edge clipped to dst",
            IRect::new(2, 15, 8, 20),
            IRect::new(2, 9, 8, 10),
        ),
        tc(
            "src bottom-left corner",
            IRect::new(-15, 15, -2, 20),
            IRect::new(0, 9, 1, 10),
        ),
        tc(
            "src intersects dst high",
            IRect::new(2, 2, 15, 15),
            IRect::new(2, 2, 10, 10),
        ),
        tc(
            "src intersects dst low",
            IRect::new(-5, -5, 8, 8),
            IRect::new(0, 0, 8, 8),
        ),
        tc(
            "src contains dst",
            IRect::new(2, 2, 8, 8),
            IRect::new(2, 2, 8, 8),
        ),
        tc(
            "src contained in dst",
            IRect::new(-5, -5, 15, 15),
            IRect::new(0, 0, 10, 10),
        ),
    ];

    for t in &tests {
        let actual = closest_disjoint_edge(&K_SRC, &t.dst);
        reporter_assert!(r, actual == t.expect, "{}", t.label);
    }

    // Test emptiness of src and dst
    reporter_assert!(
        r,
        closest_disjoint_edge(&IRect::new_empty(), &IRect::new(0, 0, 8, 8)).is_empty()
    );
    reporter_assert!(
        r,
        closest_disjoint_edge(&IRect::new(0, 0, 8, 8), &IRect::new_empty()).is_empty()
    );
    reporter_assert!(
        r,
        closest_disjoint_edge(&IRect::new(10, 10, -1, 2), &IRect::new(15, 8, -2, 20)).is_empty()
    );
});
