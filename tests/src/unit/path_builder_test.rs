// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathBuilderTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::{AddPathMode, Path};
use skia_rust_core::path_builder::{DumpFormat, PathBuilder};
use skia_rust_core::path_enums::PathFirstDirection;
use skia_rust_core::path_iter::PathIter;
use skia_rust_core::path_measure::PathMeasure;
use skia_rust_core::path_priv::{self, RangeIter};
use skia_rust_core::path_types::{PathDirection, PathFillType, PathSegmentMask, PathVerb};
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::scalar::{SCALAR_1, Scalar, scalar};

// Port of: tests/PathBuilderTest.cpp#L29-L32 (chrome/m156)
fn is_empty(reporter: &mut Reporter, p: &Path) {
    reporter_assert!(reporter, p.bounds().is_empty());
    reporter_assert!(reporter, p.count_points() == 0);
}

// Port of: tests/PathBuilderTest.cpp#L34-L57 (chrome/m156)
def_test!(pathbuilder, |reporter| {
    let mut b = PathBuilder::new();

    reporter_assert!(reporter, b.is_empty());
    is_empty(reporter, &b.snapshot());
    is_empty(reporter, &b.detach());

    b.move_to((10.0, 10.0))
        .line_to((20.0, 20.0))
        .quad_to((30.0, 10.0), (10.0, 20.0));
    reporter_assert!(reporter, b.count_points() == 4);

    let p0 = b.snapshot();
    let p1 = b.snapshot();
    let p2 = b.detach();

    reporter_assert!(
        reporter,
        *p0.bounds() == Rect::from_ltrb(10.0, 10.0, 30.0, 20.0)
    );
    reporter_assert!(reporter, p0.count_points() == 4);

    reporter_assert!(reporter, p0 == p1);
    reporter_assert!(reporter, p0 == p2);

    reporter_assert!(reporter, b.is_empty());
    is_empty(reporter, &b.snapshot());
    is_empty(reporter, &b.detach());
});

// Port of: tests/PathBuilderTest.cpp#L59-L74 (chrome/m156)
def_test!(pathbuilder_filltype, |reporter| {
    for fill_type in [
        PathFillType::Winding,
        PathFillType::EvenOdd,
        PathFillType::InverseWinding,
        PathFillType::InverseEvenOdd,
    ] {
        let mut b = PathBuilder::new_with_fill_type(fill_type);

        reporter_assert!(reporter, b.fill_type() == fill_type);
        reporter_assert!(reporter, b.is_inverse_fill_type() == fill_type.is_inverse());

        for path in [b.snapshot(), b.detach()] {
            reporter_assert!(reporter, path.fill_type() == fill_type);
            is_empty(reporter, &path);
        }
    }
});

// Port of: tests/PathBuilderTest.cpp#L76-L110 (chrome/m156)
fn check_points(path: &Path, expected: &[Point]) -> bool {
    let mut iter_pts: Vec<Point> = Vec::new();

    for (v, p, _w) in path_priv::iterate(path) {
        match v {
            PathVerb::Move => iter_pts.push(p[0]),
            PathVerb::Line => iter_pts.push(p[1]),
            PathVerb::Quad | PathVerb::Conic => {
                iter_pts.push(p[1]);
                iter_pts.push(p[2]);
            }
            PathVerb::Cubic => {
                iter_pts.push(p[1]);
                iter_pts.push(p[2]);
                iter_pts.push(p[3]);
            }
            PathVerb::Close => {}
        }
    }
    if iter_pts.len() != expected.len() {
        return false;
    }
    for i in 0..expected.len() {
        if iter_pts[i] != expected[i] {
            return false;
        }
    }
    true
}

// Port of: tests/PathBuilderTest.cpp#L112-L127 (chrome/m156)
def_test!(pathbuilder_missing_move, |reporter| {
    let mut b = PathBuilder::new();

    b.line_to((10.0, 10.0)).line_to((20.0, 30.0));
    let pts0 = [
        Point::new(0.0, 0.0),
        Point::new(10.0, 10.0),
        Point::new(20.0, 30.0),
    ];
    reporter_assert!(reporter, check_points(&b.snapshot(), &pts0));

    b.reset()
        .move_to((20.0, 20.0))
        .line_to((10.0, 10.0))
        .line_to((20.0, 30.0))
        .close()
        .line_to((60.0, 60.0));
    let pts1 = [
        Point::new(20.0, 20.0),
        Point::new(10.0, 10.0),
        Point::new(20.0, 30.0),
        Point::new(20.0, 20.0),
        Point::new(60.0, 60.0),
    ];
    reporter_assert!(reporter, check_points(&b.snapshot(), &pts1));
});

// Port of: tests/PathBuilderTest.cpp#L129-L159 (chrome/m156)
def_test!(pathbuilder_addRect, |reporter| {
    let r = Rect::new(10.0, 20.0, 30.0, 40.0);

    for i in 0..4 {
        for dir in [PathDirection::CW, PathDirection::CCW] {
            let mut b = PathBuilder::new();
            b.add_rect(r, dir, i);
            let bp = b.detach();

            reporter_assert!(reporter, bp.is_convex());
            let rect = bp.is_rect();
            reporter_assert!(reporter, rect.is_some());
            let (r2, closed, dir2) = rect.unwrap_or((Rect::default(), false, PathDirection::CW));
            reporter_assert!(reporter, r2 == r);
            reporter_assert!(reporter, closed);
            reporter_assert!(reporter, dir == dir2);

            let p = Path::rect_with_start_index(r, dir, i);
            reporter_assert!(reporter, p == bp);

            // do it again, after the detach
            b.add_rect(r, dir, i);
            b.move_to((3.0, 4.0));
            b.line_to((4.0, 5.0));
            let bp = b.detach();
            reporter_assert!(reporter, !bp.is_convex());
            reporter_assert!(reporter, bp.is_rect().is_none());
        }
    }
});

// Port of: tests/PathBuilderTest.cpp#L161-L201 (chrome/m156)
fn is_eq(a: &Path, b: &Path) -> bool {
    if a != b {
        return false;
    }

    {
        let ra = a.is_oval();
        let rb = b.is_oval();
        if ra.is_some() != rb.is_some() {
            return false;
        }
        if ra.is_some() && (ra != rb) {
            return false;
        }
    }

    {
        let rra = a.is_rrect();
        let rrb = b.is_rrect();
        if rra.is_some() != rrb.is_some() {
            return false;
        }
        if rra.is_some() && (rra != rrb) {
            return false;
        }
    }

    // getConvextity() should be sufficient to test, but internally we sometimes don't want
    // to trigger computing it, so this is the stronger test for equality.
    {
        let ca = path_priv::get_convexity_or_unknown(a);
        let cb = path_priv::get_convexity_or_unknown(b);
        if ca != cb {
            return false;
        }
    }

    true
}

// Port of: tests/PathBuilderTest.cpp#L203-L229 (chrome/m156)
def_test!(pathbuilder_addOval, |reporter| {
    let r = Rect::new(10.0, 20.0, 30.0, 40.0);

    for dir in [PathDirection::CW, PathDirection::CCW] {
        for i in 0..4 {
            let bp = PathBuilder::new().add_oval(r, dir, i).detach();
            let p = Path::oval_with_start_index(r, dir, i);
            reporter_assert!(reporter, is_eq(&p, &bp));

            reporter_assert!(reporter, p.is_oval().is_some());
            reporter_assert!(reporter, bp.is_oval().is_some());
            reporter_assert!(reporter, p.is_convex());
            reporter_assert!(reporter, bp.is_convex());
        }
        let bp = PathBuilder::new().add_oval(r, dir, None).detach();
        let p = Path::oval(r, dir);
        reporter_assert!(reporter, is_eq(&p, &bp));

        // test negative case -- can't have any other segments
        let bp = PathBuilder::new()
            .add_oval(r, dir, None)
            .line_to((10.0, 10.0))
            .detach();
        reporter_assert!(reporter, bp.is_oval().is_none());
        let bp = PathBuilder::new()
            .line_to((10.0, 10.0))
            .add_oval(r, dir, None)
            .detach();
        reporter_assert!(reporter, bp.is_oval().is_none());
    }
});

// Port of: tests/PathBuilderTest.cpp#L231-L254 (chrome/m156)
def_test!(pathbuilder_addRRect, |reporter| {
    let rr = RRect::new_rect_xy(Rect::new(10.0, 20.0, 30.0, 40.0), 5.0, 6.0);

    for dir in [PathDirection::CW, PathDirection::CCW] {
        for i in 0..4 {
            let mut b = PathBuilder::new();
            b.add_rrect(rr, dir, i);
            let bp = b.detach();

            let p = Path::rrect_with_start_index(rr, dir, i);
            reporter_assert!(reporter, is_eq(&p, &bp));
        }
        let bp = PathBuilder::new().add_rrect(rr, dir, None).detach();
        let p = Path::rrect(rr, dir);
        reporter_assert!(reporter, is_eq(&p, &bp));

        // test negative case -- can't have any other segments
        let bp = PathBuilder::new()
            .add_rrect(rr, dir, None)
            .line_to((10.0, 10.0))
            .detach();
        reporter_assert!(reporter, bp.is_rrect().is_none());
        let bp = PathBuilder::new()
            .line_to((10.0, 10.0))
            .add_rrect(rr, dir, None)
            .detach();
        reporter_assert!(reporter, bp.is_rrect().is_none());
    }
});

// Port of: tests/PathBuilderTest.cpp#L256-L274 (chrome/m156)
def_test!(pathbuilder_make, |reporter| {
    const N: usize = 100;
    let mut vbs = [PathVerb::Move; N];
    let mut pts = [Point::default(); N];

    let mut rand = Random::default();
    let mut b = PathBuilder::new();
    b.move_to((0.0, 0.0));
    pts[0] = Point::new(0.0, 0.0);
    vbs[0] = PathVerb::Move;
    for i in 1..N {
        let x = rand.next_f();
        let y = rand.next_f();
        b.line_to((x, y));
        pts[i] = Point::new(x, y);
        vbs[i] = PathVerb::Line;
    }
    let p0 = b.detach();
    let p1 = Path::raw(&pts, &vbs, &[], p0.fill_type(), None);
    reporter_assert!(reporter, p0 == p1);
});

// Port of: tests/PathBuilderTest.cpp#L276-L286 (chrome/m156)
def_test!(pathbuilder_genid, |r| {
    let mut builder = PathBuilder::new();

    builder.line_to((10.0, 10.0));
    let p1 = builder.snapshot();

    builder.line_to((10.0, 20.0));
    let p2 = builder.snapshot();

    reporter_assert!(r, p1.generation_id() != p2.generation_id());
});

// Port of: tests/PathBuilderTest.cpp#L288-L312 (chrome/m156)
def_test!(pathbuilder_addPolygon, |reporter| {
    let pts = [
        Point::new(1.0, 2.0),
        Point::new(3.0, 4.0),
        Point::new(5.0, 6.0),
        Point::new(7.0, 8.0),
    ];

    let addpoly = |pts: &[Point], count: usize, is_closed: bool| {
        let mut builder = PathBuilder::new();
        if count > 0 {
            builder.move_to(pts[0]);
            for p in &pts[1..count] {
                builder.line_to(*p);
            }
            if is_closed {
                builder.close();
            }
        }
        builder.detach()
    };

    for is_closed in [false, true] {
        for i in 0..=pts.len() {
            let path0 = PathBuilder::new()
                .add_polygon(&pts[..i], is_closed)
                .detach();
            let path1 = addpoly(&pts, i, is_closed);
            reporter_assert!(reporter, path0 == path1);
        }
    }
});

// Port of: tests/PathBuilderTest.cpp#L314-L330 (chrome/m156)
fn test_add_path(reporter: &mut Reporter) {
    let mut p = PathBuilder::new();
    let mut q = PathBuilder::new();
    p.line_to((1.0, 2.0));
    q.move_to((4.0, 4.0));
    q.line_to((7.0, 8.0));
    q.conic_to((8.0, 7.0), (6.0, 5.0), 0.5);
    q.quad_to((6.0, 7.0), (8.0, 6.0));
    q.cubic_to((5.0, 6.0), (7.0, 8.0), (7.0, 5.0));
    q.close();
    p.add_path_with_offset(&q.snapshot(), (-4.0, -4.0), None);
    let expected = Rect::new(0.0, 0.0, 4.0, 4.0);
    reporter_assert!(reporter, *p.snapshot().bounds() == expected);
    p.reset();
    path_priv::reverse_add_path(&mut p, &q.snapshot());
    let reverse_expected = Rect::new(4.0, 4.0, 8.0, 8.0);
    reporter_assert!(reporter, *p.snapshot().bounds() == reverse_expected);
}

// Port of: tests/PathBuilderTest.cpp#L332-L349 (chrome/m156)
fn test_add_path_mode(reporter: &mut Reporter, explicit_move_to: bool, extend: bool) {
    let mut p = PathBuilder::new();
    let mut q = PathBuilder::new();
    if explicit_move_to {
        p.move_to((1.0, 1.0));
    }
    p.line_to((1.0, 2.0));
    if explicit_move_to {
        q.move_to((2.0, 1.0));
    }
    q.line_to((2.0, 2.0));
    p.add_path(
        &q.snapshot(),
        if extend {
            AddPathMode::Extend
        } else {
            AddPathMode::Append
        },
    );
    let verbs = path_priv::get_verbs(&p);
    reporter_assert!(reporter, verbs.len() == 4);
    reporter_assert!(reporter, verbs[0] == PathVerb::Move);
    reporter_assert!(reporter, verbs[1] == PathVerb::Line);
    reporter_assert!(
        reporter,
        verbs[2]
            == (if extend {
                PathVerb::Line
            } else {
                PathVerb::Move
            })
    );
    reporter_assert!(reporter, verbs[3] == PathVerb::Line);
}

// Port of: tests/PathBuilderTest.cpp#L351-L356 (chrome/m156)
fn get_point(builder: &PathBuilder, index: i32) -> Option<Point> {
    usize::try_from(index)
        .ok()
        .and_then(|i| builder.points().get(i).copied())
}

// Port of: tests/PathBuilderTest.cpp#L358-L383 (chrome/m156)
fn test_extend_closed_path(reporter: &mut Reporter) {
    let mut p = PathBuilder::new();
    let mut q = PathBuilder::new();
    p.move_to((1.0, 1.0));
    p.line_to((1.0, 2.0));
    p.line_to((2.0, 2.0));
    p.close();
    q.move_to((2.0, 1.0));
    q.line_to((2.0, 3.0));
    p.add_path(&q.detach(), AddPathMode::Extend);
    let verbs = path_priv::get_verbs(&p);
    reporter_assert!(reporter, verbs.len() == 7);
    reporter_assert!(reporter, verbs[0] == PathVerb::Move);
    reporter_assert!(reporter, verbs[1] == PathVerb::Line);
    reporter_assert!(reporter, verbs[2] == PathVerb::Line);
    reporter_assert!(reporter, verbs[3] == PathVerb::Close);
    reporter_assert!(reporter, verbs[4] == PathVerb::Move);
    reporter_assert!(reporter, verbs[5] == PathVerb::Line);
    reporter_assert!(reporter, verbs[6] == PathVerb::Line);

    let pt = p.get_last_pt();
    reporter_assert!(reporter, pt.is_some());
    reporter_assert!(reporter, pt == Some(Point::new(2.0, 3.0)));
    let pt = get_point(&p, 3);
    reporter_assert!(reporter, pt.is_some());
    reporter_assert!(reporter, pt == Some(Point::new(1.0, 1.0)));
}

// Port of: tests/PathBuilderTest.cpp#L385-L399 (chrome/m156)
fn test_add_empty_path(reporter: &mut Reporter, mode: AddPathMode) {
    let mut p = PathBuilder::new();
    let mut q = PathBuilder::new();
    let r = PathBuilder::new();
    // case 1: dst is empty
    p.move_to((2.0, 1.0));
    p.line_to((2.0, 3.0));
    q.add_path(&p.snapshot(), mode);
    reporter_assert!(reporter, q.snapshot() == p.snapshot());
    // case 2: src is empty
    p.add_path(&r.snapshot(), mode);
    reporter_assert!(reporter, q.snapshot() == p.snapshot());
    // case 3: src and dst are empty
    q.reset();
    q.add_path(&r.snapshot(), mode);
    reporter_assert!(reporter, q.is_empty());
}

/*
 *  SkPath allows the caller to "skip" calling moveTo for contours. If lineTo (or a curve) is
 *  called on an empty path, a 'moveTo(0,0)' will automatically be injected. If the path is
 *  not empty, but its last contour has been "closed", then it will inject a moveTo corresponding
 *  to where the last contour itself started (i.e. its moveTo).
 */
// Port of: tests/PathBuilderTest.cpp#L415-L473 (chrome/m156)
fn test_add_path_and_injected_move_to(reporter: &mut Reporter) {
    /*
     *  Given a path, and the expected last-point and last-move-to in it,
     *  assert that, after a lineTo(), that the injected moveTo corresponds
     *  to the expected value.
     */
    let test_before_after_lineto = |reporter: &mut Reporter,
                                    path: &mut PathBuilder,
                                    expected_last_pt: Point,
                                    expected_move_to: Point| {
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // small counts
        let count = |path: &PathBuilder| path.count_points() as i32;
        let p = get_point(path, count(path) - 1);
        reporter_assert!(reporter, p.is_some());
        reporter_assert!(reporter, p == Some(expected_last_pt));

        let new_line_to = Point::new(1234.0, 5678.0);
        path.line_to(new_line_to);

        let p = get_point(path, count(path) - 2);
        reporter_assert!(reporter, p.is_some());
        reporter_assert!(reporter, p == Some(expected_move_to)); // this was injected by lineTo()

        let p = get_point(path, count(path) - 1);
        reporter_assert!(reporter, p.is_some());
        reporter_assert!(reporter, p == Some(new_line_to));
    };

    let mut path1 = PathBuilder::new();
    path1.move_to((230.0, 230.0)); // Needed to show the bug: a moveTo before the addRect
    path1
        .move_to((20.0, 30.0))
        .line_to((40.0, 30.0))
        .line_to((40.0, 50.0))
        .line_to((20.0, 50.0));
    let mut path1c = PathBuilder::new_path(&path1.snapshot());
    path1c.close();

    let mut path2 = PathBuilder::new();
    // If path2 contains zero points, the update calculation isn't tested.
    path2.move_to((144.0, 72.0));
    path2.line_to((146.0, 72.0));
    let mut path2c = PathBuilder::new_path(&path2.snapshot());
    path2c.close();
    let mut path3 = PathBuilder::new_path(&path2.snapshot());
    let mut path3c = PathBuilder::new_path(&path2c.snapshot());

    // Test addPath, adding a path that ends with close.
    // The start point of the last contour added,
    // and the internal flag tracking whether it is closed,
    // must be updated correctly.
    path2.add_path(&path1c.snapshot(), None);
    path2c.add_path(&path1c.snapshot(), None);
    // At this point, path1c, path2, and path2c should end the same way.
    let (p50, p30) = (Point::new(20.0, 50.0), Point::new(20.0, 30.0));
    test_before_after_lineto(reporter, &mut path1c, p50, p30);
    test_before_after_lineto(reporter, &mut path2, p50, p30);
    test_before_after_lineto(reporter, &mut path2c, p50, p30);

    // Test addPath, adding a path not ending in close.
    path3.add_path(&path1.snapshot(), None);
    path3c.add_path(&path1.snapshot(), None);
    // At this point, path1, path3, and path3c should end the same way.
    test_before_after_lineto(reporter, &mut path1, p50, p50);
    test_before_after_lineto(reporter, &mut path3, p50, p50);
    test_before_after_lineto(reporter, &mut path3c, p50, p50);
}

// Port of: tests/PathBuilderTest.cpp#L475-L513 (chrome/m156)
#[allow(clippy::items_after_statements)] // constants stay next to the C++ code they mirror
fn test_add_path_convexity(reporter: &mut Reporter) {
    let circle = Path::circle((10.0, 10.0), 10.0, None);
    reporter_assert!(reporter, circle.is_convex());

    let builder_add = |start_with_move: bool, mode: AddPathMode| {
        let mut builder = PathBuilder::new();
        if start_with_move {
            builder.move_to((0.0, 0.0));
        }
        builder.add_path(&circle, mode);
        builder.detach()
    };

    struct Expect {
        start_with_move: bool,
        mode: AddPathMode,
        should_be_convex: bool,
    }
    let expectations = [
        Expect {
            start_with_move: false,
            mode: AddPathMode::Append,
            should_be_convex: true,
        },
        Expect {
            start_with_move: true,
            mode: AddPathMode::Append,
            should_be_convex: true,
        },
        Expect {
            start_with_move: false,
            mode: AddPathMode::Extend,
            should_be_convex: true,
        },
        Expect {
            start_with_move: true,
            mode: AddPathMode::Extend,
            should_be_convex: false,
        },
    ];

    for e in &expectations {
        let path = builder_add(e.start_with_move, e.mode);
        reporter_assert!(reporter, path.is_convex() == e.should_be_convex);
    }

    let mut pb = PathBuilder::new();
    reporter_assert!(reporter, pb.snapshot().is_convex());
    // Appending to empty preserves convexity.
    pb.add_path(&circle, None);
    reporter_assert!(reporter, pb.snapshot().is_convex());
    // Appending to non-empty should clear convexity.
    pb.add_path(&circle, None);
    reporter_assert!(reporter, !pb.snapshot().is_convex());
}

// Port of: tests/PathBuilderTest.cpp#L515-L539 (chrome/m156)
def_test!(pathbuilder_addPath, |reporter| {
    let p = PathBuilder::new()
        .move_to((10.0, 10.0))
        .line_to((100.0, 10.0))
        .quad_to((200.0, 100.0), (100.0, 200.0))
        .close()
        .move_to((200.0, 200.0))
        .cubic_to((210.0, 200.0), (210.0, 300.0), (200.0, 300.0))
        .conic_to((150.0, 250.0), (100.0, 200.0), 1.4)
        .detach();

    reporter_assert!(
        reporter,
        p == PathBuilder::new().add_path(&p, None).detach()
    );

    test_add_path(reporter);
    test_add_path_mode(reporter, false, false);
    test_add_path_mode(reporter, true, false);
    test_add_path_mode(reporter, false, true);
    test_add_path_mode(reporter, true, true);
    test_extend_closed_path(reporter);
    test_add_empty_path(reporter, AddPathMode::Extend);
    test_add_empty_path(reporter, AddPathMode::Append);
    test_add_path_and_injected_move_to(reporter);

    test_add_path_convexity(reporter);
});

// Port of: tests/PathBuilderTest.cpp#L541-L567 (chrome/m156)
def_test!(pathbuilder_addpath_crbug_1153516, |r| {
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // small counts
    let count = |path: &PathBuilder| path.count_points() as i32;
    // When we add a closed path to another path, verify
    // that the result has the right value for last contour start point.
    let mut p1 = PathBuilder::new();
    let mut p2 = PathBuilder::new();
    p2.line_to((10.0, 20.0));
    p1.add_rect(Rect::new(143.0, 226.0, 200.0, 241.0), None, None);
    p2.add_path(&p1.snapshot(), None);
    p2.line_to((262.0, 513.0)); // this should not assert
    let rectangle_start = Point::new(143.0, 226.0);
    let line_end = Point::new(262.0, 513.0);
    let actual_move_to = get_point(&p2, count(&p2) - 2);
    reporter_assert!(r, actual_move_to.is_some());
    reporter_assert!(r, actual_move_to == Some(rectangle_start));
    let actual_line_to = get_point(&p2, count(&p2) - 1);
    reporter_assert!(r, actual_line_to.is_some());
    reporter_assert!(r, actual_line_to == Some(line_end));

    // Verify adding a closed path to itself
    let snap = p1.snapshot();
    p1.add_path(&snap, None);
    p1.line_to((262.0, 513.0));
    let actual_move_to = get_point(&p1, count(&p1) - 2);
    reporter_assert!(r, actual_move_to.is_some());
    reporter_assert!(r, actual_move_to == Some(rectangle_start));
    let actual_line_to = get_point(&p1, count(&p1) - 1);
    reporter_assert!(r, actual_line_to.is_some());
    reporter_assert!(r, actual_line_to == Some(line_end));
});

// Port of: tests/PathBuilderTest.cpp#L569-L576 (chrome/m156)
#[allow(clippy::float_cmp)] // exact float comparisons, as in C++
fn assert_is_move_to(reporter: &mut Reporter, iter: &mut RangeIter<'_>, x0: scalar, y0: scalar) {
    let (v, pts, _w) = iter.next().expect("iterator is not done");
    reporter_assert!(
        reporter,
        v == PathVerb::Move,
        "{} != {} (move)",
        v as i32,
        PathVerb::Move as i32
    );
    reporter_assert!(
        reporter,
        pts[0].x == x0,
        "X mismatch {} != {}",
        pts[0].x,
        x0
    );
    reporter_assert!(
        reporter,
        pts[0].y == y0,
        "Y mismatch {} != {}",
        pts[0].y,
        y0
    );
}

// Port of: tests/PathBuilderTest.cpp#L578-L586 (chrome/m156)
#[allow(clippy::float_cmp)] // exact float comparisons, as in C++
fn assert_is_line_to(reporter: &mut Reporter, iter: &mut RangeIter<'_>, x1: scalar, y1: scalar) {
    let (v, pts, _w) = iter.next().expect("iterator is not done");
    reporter_assert!(
        reporter,
        v == PathVerb::Line,
        "{} != {} (line)",
        v as i32,
        PathVerb::Line as i32
    );
    // pts[0] is the moveTo before this line. See pts_backset_for_verb in SkPath::RangeIter
    reporter_assert!(
        reporter,
        pts[1].x == x1,
        "X mismatch {} != {}",
        pts[1].x,
        x1
    );
    reporter_assert!(
        reporter,
        pts[1].y == y1,
        "Y mismatch {} != {}",
        pts[1].y,
        y1
    );
}

// Port of: tests/PathBuilderTest.cpp#L588-L590 (chrome/m156)
fn assert_is_done(reporter: &mut Reporter, iter: &RangeIter<'_>) {
    reporter_assert!(reporter, iter.is_done(), "Iterator is not done yet");
}

// Port of: tests/PathBuilderTest.cpp#L592-L613 (chrome/m156)
def_test!(SkPathBuilder_multipleMoveTos, |reporter| {
    let mut pb = PathBuilder::new();
    reporter_assert!(reporter, pb.is_empty());

    let check_last_pt = |reporter: &mut Reporter, pb: &PathBuilder, x: f32, y: f32| {
        let last_pt = pb.get_last_pt();
        reporter_assert!(reporter, last_pt.is_some());
        last_pt == Some(Point::new(x, y))
    };

    pb.move_to((1.0, 2.0));
    reporter_assert!(reporter, pb.points().len() == 1);
    let ok = check_last_pt(reporter, &pb, 1.0, 2.0);
    reporter_assert!(reporter, ok);
    reporter_assert!(
        reporter,
        pb.compute_bounds() == Rect::from_xywh(1.0, 2.0, 0.0, 0.0)
    );

    pb.move_to((3.0, 4.0));
    pb.move_to((5.0, 6.0));
    pb.move_to((7.0, 8.0));
    reporter_assert!(reporter, pb.points().len() == 1);
    let ok = check_last_pt(reporter, &pb, 7.0, 8.0);
    reporter_assert!(reporter, ok);
    reporter_assert!(
        reporter,
        pb.compute_bounds() == Rect::from_xywh(7.0, 8.0, 0.0, 0.0)
    );
});

// Port of: tests/PathBuilderTest.cpp#L615-L632 (chrome/m156)
def_test!(SkPathBuilder_lineToMoveTo, |reporter| {
    let mut pb = PathBuilder::new();
    pb.move_to((20.0, 3.0));
    pb.line_to((7.0, 11.0));
    pb.line_to((8.0, 12.0));
    pb.move_to((2.0, 3.0));
    pb.line_to((20.0, 30.0));

    let result = pb.detach();

    let mut iter = path_priv::iterate(&result);
    assert_is_move_to(reporter, &mut iter, 20.0, 3.0);
    assert_is_line_to(reporter, &mut iter, 7.0, 11.0);
    assert_is_line_to(reporter, &mut iter, 8.0, 12.0);
    assert_is_move_to(reporter, &mut iter, 2.0, 3.0);
    assert_is_line_to(reporter, &mut iter, 20.0, 30.0);
    assert_is_done(reporter, &iter);
});

// Port of: tests/PathBuilderTest.cpp#L634-L657 (chrome/m156)
def_test!(
    SkPathBuilder_arcToPtPtRad_invalidInputsResultInALine,
    |reporter| {
        // skia-rust: `reporter->push(name)` / `pop()` (failure context) are not ported; the name is
        // unused.
        let test = |reporter: &mut Reporter,
                    _name: &str,
                    start: Point,
                    end: Point,
                    radius: scalar,
                    expected_line_to: Point| {
            let mut pb = PathBuilder::new();
            // Remember there is an implicit moveTo(0, 0) if arcTo is the first command called.
            pb.arc_to_tangent(start, end, radius);
            let result = pb.detach();

            let mut iter = path_priv::iterate(&result);
            assert_is_move_to(reporter, &mut iter, 0.0, 0.0);
            assert_is_line_to(reporter, &mut iter, expected_line_to.x, expected_line_to.y);
            assert_is_done(reporter, &iter);
        };
        // From SkPathBuilder docs:
        //   Arc is contained by tangent from last SkPath point to p1, and tangent from p1 to p2. Arc
        //   is part of circle sized to radius, positioned so it touches both tangent lines.
        // If the values cannot construct an arc, a line to the first point is constructed instead.
        let p = Point::new;
        test(
            reporter,
            "first point equals previous point",
            p(0.0, 0.0),
            p(1.0, 2.0),
            1.0,
            p(0.0, 0.0),
        );
        test(
            reporter,
            "two points equal",
            p(5.0, 7.0),
            p(5.0, 7.0),
            1.0,
            p(5.0, 7.0),
        );
        test(
            reporter,
            "radius is zero",
            p(-3.0, 5.0),
            p(-7.0, 11.0),
            0.0,
            p(-3.0, 5.0),
        );
        test(
            reporter,
            "second point equals previous point",
            p(5.0, 4.0),
            p(0.0, 0.0),
            1.0,
            p(5.0, 4.0),
        );
    }
);

// Port of: tests/PathBuilderTest.cpp#L659-L677 (chrome/m156)
def_test!(SkPathBuilder_assign, |reporter| {
    let check_round_trip = |reporter: &mut Reporter, src: &Path| {
        let mut builder = PathBuilder::new();
        builder.assign_path(src);
        let dst = builder.detach();
        reporter_assert!(reporter, *src == dst);
        // Our equality test doesn't look at volatility, which is probably correct, but
        // we want to ensure that our builder faithfully can reproduce the path.
        reporter_assert!(reporter, src.is_volatile() == dst.is_volatile());
    };

    let pts = [
        Point::new(0.0, 0.0),
        Point::new(1.0, 1.0),
        Point::new(2.0, 2.0),
    ];
    let is_closed = false; // doesn't matter for the test

    let is_volatile = false;
    check_round_trip(
        reporter,
        &Path::polygon(&pts, is_closed, PathFillType::Winding, is_volatile),
    );
    let is_volatile = true;
    check_round_trip(
        reporter,
        &Path::polygon(&pts, is_closed, PathFillType::Winding, is_volatile),
    );
});

// Port of: tests/PathBuilderTest.cpp#L679-L689 (chrome/m156)
def_test!(SkPathBuilder_getLastPt, |reporter| {
    let mut b = PathBuilder::new();
    reporter_assert!(reporter, b.get_last_pt().is_none());
    b.move_to((10.0, 10.0));
    let pt = b.get_last_pt();
    reporter_assert!(reporter, pt.is_some());
    reporter_assert!(reporter, pt == Some(Point::new(10.0, 10.0)));
    b.r_line_to((10.0, 10.0));
    let pt = b.get_last_pt();
    reporter_assert!(reporter, pt == Some(Point::new(20.0, 20.0)));
});

// Port of: tests/PathBuilderTest.cpp#L691-L791 (chrome/m156)
def_test!(
    #[allow(clippy::similar_names)] // names follow the C++
    SkPathBuilder_transform,
    |reporter| {
        let mut b = PathBuilder::new();

        // CONIC_PERSPECTIVE_BUG_FIXED is 0 in the C++ test, so the conic points are not added.
        let pts = [
            Point::new(0.0, 0.0),   // move
            Point::new(10.0, 10.0), // line
            Point::new(20.0, 10.0),
            Point::new(20.0, 0.0), // quad
            Point::new(0.0, 0.0),
            Point::new(0.0, 10.0),
            Point::new(1.0, 10.0), // cubic
        ];
        let pt_count = pts.len();

        b.move_to(pts[0]);
        b.line_to(pts[1]);
        b.quad_to(pts[2], pts[3]);
        b.cubic_to(pts[4], pts[5], pts[6]);
        b.close();

        {
            let mut matrix = Matrix::default();
            matrix.reset();
            let p1 = PathBuilder::new_path(&b.snapshot())
                .transform(&matrix)
                .detach();
            reporter_assert!(reporter, b.snapshot() == p1);
        }

        {
            let mut matrix = Matrix::default();
            matrix.set_scale((SCALAR_1 * 2.0, SCALAR_1 * 3.0), None);

            let p1 = PathBuilder::new_path(&b.snapshot())
                .transform(&matrix)
                .detach();
            let pts1 = p1.points();
            reporter_assert!(reporter, pt_count == pts1.len());
            for i in 0..pts1.len() {
                let new_pt = Point::new(pts[i].x * 2.0, pts[i].y * 3.0);
                reporter_assert!(reporter, new_pt == pts1[i]);
            }
        }

        {
            let mut matrix = Matrix::default();
            matrix.reset();
            matrix.set_persp_x(4.0);

            let mut b1 = PathBuilder::new_path(&b.snapshot());
            b1.move_to(Point::new(0.0, 0.0)).transform(&matrix);
            let inverse = matrix.invert();
            reporter_assert!(reporter, inverse.is_some());
            if let Some(inverse) = inverse {
                matrix = inverse;
            }
            b1.transform(&matrix);
            let p_bounds = *b.snapshot().bounds();
            let p1_bounds = *b1.detach().bounds();
            reporter_assert!(
                reporter,
                scalar::nearly_equal(p_bounds.left, p1_bounds.left, None)
            );
            reporter_assert!(
                reporter,
                scalar::nearly_equal(p_bounds.top, p1_bounds.top, None)
            );
            reporter_assert!(
                reporter,
                scalar::nearly_equal(p_bounds.right, p1_bounds.right, None)
            );
            reporter_assert!(
                reporter,
                scalar::nearly_equal(p_bounds.bottom, p1_bounds.bottom, None)
            );
        }

        b.reset();
        b.add_circle((0.0, 0.0), 1.0, PathDirection::CW);

        {
            let mut matrix = Matrix::default();
            matrix.reset();
            let mut b1 = PathBuilder::new_path(&b.snapshot());
            b1.move_to(Point::new(0.0, 0.0));
            b1.transform(&matrix);
            reporter_assert!(
                reporter,
                path_priv::compute_first_direction_path(&b1.detach()) == PathFirstDirection::CW
            );
        }

        {
            let mut matrix = Matrix::default();
            matrix.reset();
            matrix.set_scale_x(-1.0);
            let mut b1 = PathBuilder::new_path(&b.snapshot());
            b1.move_to(Point::new(0.0, 0.0)); // Make b1 unique (i.e., not empty path)

            b1.transform(&matrix);
            reporter_assert!(
                reporter,
                path_priv::compute_first_direction_path(&b1.detach()) == PathFirstDirection::CCW
            );
        }

        {
            let mut matrix = Matrix::default();
            matrix.set_all(1.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0);
            let mut b1 = PathBuilder::new_path(&b.snapshot());
            b1.move_to(Point::new(0.0, 0.0)); // Make p1 unique (i.e., not empty path)

            b1.transform(&matrix);
            reporter_assert!(
                reporter,
                path_priv::compute_first_direction_path(&b1.snapshot())
                    == PathFirstDirection::Unknown
            );
        }
    }
);

// Port of: tests/PathBuilderTest.cpp#L793-L808 (chrome/m156)
def_test!(SkPathBuilder_cleaning, |reporter| {
    // Test that we safely handle meaningless verbs, like repeated kClose
    let mut b = PathBuilder::new();
    b.move_to((1.0, 2.0));
    b.close();
    b.close(); // this call should be silently ignored

    let verbs = b.verbs();
    reporter_assert!(reporter, verbs.len() == 2);
    reporter_assert!(reporter, verbs[0] == PathVerb::Move);
    reporter_assert!(reporter, verbs[1] == PathVerb::Close);

    let pts = b.points();
    reporter_assert!(reporter, pts.len() == 1);
    reporter_assert!(reporter, pts[0] == Point::new(1.0, 2.0));
});

// Port of: tests/PathBuilderTest.cpp#L810-L875 (chrome/m156)
def_test!(SkPathBuilder_path_roundtrip, |reporter| {
    let check_roundtrip = |reporter: &mut Reporter, path: &Path| {
        let rpath = PathBuilder::new_path(path).detach();

        reporter_assert!(reporter, *path == rpath);
        reporter_assert!(reporter, path.is_convex() == rpath.is_convex());

        // convexity is tricky after a (complex) transform ...
        {
            let mx = Matrix::rotate_deg(30.0);
            let mut bu = PathBuilder::new_path(path);
            bu.transform(&mx);
            let bupath = bu.detach();
            let copy = path.make_transform(&mx);

            let ovals = [
                path.is_oval().is_some(),
                rpath.is_oval().is_some(),
                copy.is_oval().is_some(),
                bupath.is_oval().is_some(),
            ];

            reporter_assert!(reporter, ovals[0] == ovals[1]);
            reporter_assert!(reporter, !ovals[2]);
            reporter_assert!(reporter, !ovals[3]);

            reporter_assert!(reporter, bupath.is_convex() == copy.is_convex());
        }

        let is_oval = [path_priv::is_oval(path), path_priv::is_oval(&rpath)];
        reporter_assert!(reporter, is_oval[0].is_some() == is_oval[1].is_some());
        if let (Some(a), Some(b)) = (is_oval[0], is_oval[1]) {
            reporter_assert!(reporter, a.bounds == b.bounds);
            reporter_assert!(reporter, a.direction == b.direction);
            reporter_assert!(reporter, a.start_index == b.start_index);
        }

        let is_rrect = [path_priv::is_rrect(path), path_priv::is_rrect(&rpath)];
        reporter_assert!(reporter, is_rrect[0].is_some() == is_rrect[1].is_some());
        if let (Some(a), Some(b)) = (is_rrect[0], is_rrect[1]) {
            reporter_assert!(reporter, a.rrect == b.rrect);
            reporter_assert!(reporter, a.direction == b.direction);
            reporter_assert!(reporter, a.start_index == b.start_index);
        }
    };

    check_roundtrip(reporter, &Path::new());
    check_roundtrip(
        reporter,
        &Path::circle((10.0, 20.0), 30.0, PathDirection::CCW),
    );
    check_roundtrip(
        reporter,
        &Path::oval_with_start_index(Rect::new(10.0, 20.0, 30.0, 40.0), PathDirection::CCW, 2),
    );
    check_roundtrip(
        reporter,
        &Path::rect_with_start_index(Rect::new(10.0, 20.0, 30.0, 40.0), PathDirection::CCW, 2),
    );
    check_roundtrip(
        reporter,
        &Path::rrect_xy(
            Rect::new(10.0, 20.0, 30.0, 40.0),
            1.0,
            2.0,
            PathDirection::CCW,
        ),
    );
    check_roundtrip(
        reporter,
        &PathBuilder::new()
            .line_to((100.0, 0.0))
            .quad_to((0.0, 0.0), (0.0, 100.0))
            .close()
            .detach(),
    );
});

// Port of: tests/PathBuilderTest.cpp#L877-L883 (chrome/m156)
#[allow(clippy::float_cmp)] // exact float comparisons, as in C++
fn check_move(reporter: &mut Reporter, iter: &mut PathIter<'_>, x0: scalar, y0: scalar) {
    let rec = iter.next().expect("a verb");
    reporter_assert!(reporter, rec.verb() == PathVerb::Move);
    reporter_assert!(reporter, rec.points()[0].x == x0);
    reporter_assert!(reporter, rec.points()[0].y == y0);
}

// Port of: tests/PathBuilderTest.cpp#L885-L891 (chrome/m156)
#[allow(clippy::float_cmp)] // exact float comparisons, as in C++
fn check_line(reporter: &mut Reporter, iter: &mut PathIter<'_>, x1: scalar, y1: scalar) {
    let rec = iter.next().expect("a verb");
    reporter_assert!(reporter, rec.verb() == PathVerb::Line);
    reporter_assert!(reporter, rec.points()[1].x == x1);
    reporter_assert!(reporter, rec.points()[1].y == y1);
}

// Port of: tests/PathBuilderTest.cpp#L893-L896 (chrome/m156)
fn check_close(reporter: &mut Reporter, iter: &mut PathIter<'_>) {
    let rec = iter.next().expect("a verb");
    reporter_assert!(reporter, rec.verb() == PathVerb::Close);
}

// Port of: tests/PathBuilderTest.cpp#L898-L900 (chrome/m156)
fn check_done(reporter: &mut Reporter, iter: &mut PathIter<'_>) {
    reporter_assert!(reporter, iter.next().is_none());
}

// Port of: tests/PathBuilderTest.cpp#L908-L938 (chrome/m156)
def_test!(SkPathBuilder_rMoveTo, |reporter| {
    // skia-rust: `check_done_and_reset(reporter, &p, &iter)` is split into `check_done` and
    // `p.reset()`, since the iterator borrows the builder.
    let mut p = PathBuilder::new();
    p.move_to((10.0, 11.0));
    p.line_to((20.0, 21.0));
    p.close();
    p.r_move_to((30.0, 31.0));
    p.line_to((30.0, 40.0));
    {
        let mut iter = PathIter::new(p.points(), p.verbs(), &[] /* no conics */);
        check_move(reporter, &mut iter, 10.0, 11.0);
        check_line(reporter, &mut iter, 20.0, 21.0);
        check_close(reporter, &mut iter);
        check_move(reporter, &mut iter, 10.0 + 30.0, 11.0 + 31.0);
        check_line(reporter, &mut iter, 30.0, 40.0);
        check_done(reporter, &mut iter);
    }
    p.reset();

    p.move_to((10.0, 11.0));
    p.line_to((20.0, 21.0));
    p.r_move_to((30.0, 31.0));
    p.line_to((30.0, 40.0));
    {
        let mut iter = p.iter();
        check_move(reporter, &mut iter, 10.0, 11.0);
        check_line(reporter, &mut iter, 20.0, 21.0);
        check_move(reporter, &mut iter, 20.0 + 30.0, 21.0 + 31.0);
        check_line(reporter, &mut iter, 30.0, 40.0);
        check_done(reporter, &mut iter);
    }
    p.reset();

    p.r_move_to((30.0, 31.0));
    {
        let mut iter = p.iter();
        //  PathIter, for compat, is snuffing out trailing moves
        check_done(reporter, &mut iter);
    }
    p.reset();
});

const FILL_TYPES: [PathFillType; 4] = [
    PathFillType::Winding,
    PathFillType::EvenOdd,
    PathFillType::InverseWinding,
    PathFillType::InverseEvenOdd,
];

// Port of: tests/PathBuilderTest.cpp#L947-L996 (chrome/m156)
def_test!(SkPathBuilder_equality, |reporter| {
    let check_filltype_eq = |reporter: &mut Reporter, a: &PathBuilder| {
        let mut copy = a.clone();
        reporter_assert!(reporter, *a == copy);

        for ft in FILL_TYPES {
            if ft != a.fill_type() {
                copy.set_fill_type(ft);
                reporter_assert!(reporter, *a != copy);
            }
        }
    };

    let mut a = PathBuilder::new();
    let mut b = PathBuilder::new();

    reporter_assert!(reporter, a == b);
    check_filltype_eq(reporter, &a);

    a.move_to((0.0, 0.0));
    reporter_assert!(reporter, a != b);
    b.move_to((0.0, 0.0));
    reporter_assert!(reporter, a == b);
    check_filltype_eq(reporter, &a);

    b.close();
    reporter_assert!(reporter, a != b);
    a.close();
    reporter_assert!(reporter, a == b);
    check_filltype_eq(reporter, &a);

    let set_segments = |bu: &mut PathBuilder| {
        bu.reset()
            .move_to((1.0, 2.0))
            .line_to((3.0, 4.0))
            .quad_to((5.0, 6.0), (7.0, 8.0))
            .conic_to((9.0, 10.0), (11.0, 12.0), 0.5)
            .cubic_to((13.0, 14.0), (15.0, 16.0), (17.0, 18.0))
            .close();
    };
    set_segments(&mut a);
    set_segments(&mut b);
    reporter_assert!(reporter, a == b);
    check_filltype_eq(reporter, &a);

    // mutate point value, but not verb sequence
    a.set_last_point((-1.0, -2.0));
    reporter_assert!(reporter, a != b);
    check_filltype_eq(reporter, &a);
});

// Port of: tests/PathBuilderTest.cpp#L998-L1023 (chrome/m156)
def_test!(SkPathBuilder_dump, |reporter| {
    let mut builder = PathBuilder::new();
    builder
        .move_to((1.0, 2.0))
        .line_to((3.0, 4.0))
        .quad_to((5.0, 6.0), (7.0, 8.0))
        .conic_to((9.0, 10.0), (11.0, 12.0), 0.5)
        .cubic_to((13.0, 14.0), (15.0, 16.0), (17.0, 18.0))
        .close()
        .move_to((1.0, 2.0))
        .line_to((3.0, 4.0));

    let str = builder.dump_to_string(DumpFormat::Decimal);

    let expected = "SkPathBuilder(SkPathFillType::kWinding)\n\
                    .moveTo(1, 2)\n\
                    .lineTo(3, 4)\n\
                    .quadTo(5, 6, 7, 8)\n\
                    .conicTo(9, 10, 11, 12, 0.5f)\n\
                    .cubicTo(13, 14, 15, 16, 17, 18)\n\
                    .close()\n\
                    .moveTo(1, 2)\n\
                    .lineTo(3, 4)\n";

    reporter_assert!(reporter, str == expected);
});

// Port of: tests/PathBuilderTest.cpp#L1025-L1061 (chrome/m156)
def_test!(SkPathBuilder_trailingmove_addpath, |reporter| {
    let test_with_matrix = |reporter: &mut Reporter, m: &Matrix| {
        {
            // empty src
            let src = Path::new();
            let mut b = PathBuilder::new();
            b.line_to((10.0, 10.0)).move_to((20.0, 20.0));

            let res = b.add_path_with_transform(&src, m, None).detach();
            reporter_assert!(reporter, !res.is_empty());
            reporter_assert!(
                reporter,
                res.points().last() == Some(&Point::new(20.0, 20.0))
            );
        }

        {
            // (implied) moveTo + lineTo
            let src = PathBuilder::new().line_to((100.0, 100.0)).detach();
            let mut b = PathBuilder::new();
            b.line_to((10.0, 10.0)).move_to((20.0, 20.0));

            let res = b.add_path_with_transform(&src, m, None).detach();
            reporter_assert!(reporter, !res.is_empty());
            reporter_assert!(
                reporter,
                res.points().last() != Some(&Point::new(20.0, 20.0))
            );
        }

        {
            // moveTo only
            let src = PathBuilder::new().move_to((100.0, 100.0)).detach();
            let mut b = PathBuilder::new();
            b.line_to((10.0, 10.0)).move_to((20.0, 20.0));

            let res = b.add_path_with_transform(&src, m, None).detach();
            reporter_assert!(reporter, !res.is_empty());
            reporter_assert!(
                reporter,
                res.points().last() != Some(&Point::new(20.0, 20.0))
            );
        }
    };

    test_with_matrix(reporter, Matrix::i());
    // Perspective matrices trigger a different append code path.
    test_with_matrix(
        reporter,
        &Matrix::new_all(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.005, 0.001, 1.0),
    );
});

// Port of: tests/PathBuilderTest.cpp#L1063-L1079 (chrome/m156)
def_test!(SkPathBuilder_b_463584612, |reporter| {
    let mut b = PathBuilder::new();
    // SK_SUPPORT_LEGACY_PATHBUILDER_SETLASTPT is defined
    b.set_last_pt((0.0, 0.0));
    b.close().r_move_to((1.0, 1.0));

    let mut builder = PathBuilder::new();
    builder.add_path_with_transform(&b.snapshot(), Matrix::i(), AddPathMode::Extend);
    builder.r_move_to((3.0, 4.0)); // This crashed

    let p = builder.detach();
    reporter_assert!(reporter, !p.is_empty());
});

// Port of: tests/PathBuilderTest.cpp#L1081-L1125 (chrome/m156)
def_test!(SkPathBuilder_conic_semantics, |reporter| {
    {
        // If w is finite and not one, appends kConic_Verb to verb array;
        // and pt1, pt2 to SkPoint array; and w to conic weights.
        let p = PathBuilder::new()
            .conic_to((10.0, 5.0), (10.0, 10.0), 0.5)
            .detach();
        reporter_assert!(reporter, !p.is_empty());
        reporter_assert!(reporter, p.verbs().len() == 2); // moveTo, conicTo
        reporter_assert!(reporter, p.points().len() == 3);
        reporter_assert!(reporter, p.conic_weights().len() == 1);
        reporter_assert!(reporter, p.segment_masks() == PathSegmentMask::CONIC);
    }

    {
        // If w is one, appends kQuad_Verb to verb array, and
        // pt1, pt2 to SkPoint array.
        let p = PathBuilder::new()
            .conic_to((10.0, 5.0), (10.0, 10.0), 1.0)
            .detach();
        reporter_assert!(reporter, !p.is_empty());
        reporter_assert!(reporter, p.verbs().len() == 2); // moveTo, quadTo
        reporter_assert!(reporter, p.points().len() == 3);
        reporter_assert!(reporter, p.conic_weights().is_empty());
        reporter_assert!(reporter, p.segment_masks() == PathSegmentMask::QUAD);
    }

    {
        // If w is not finite, appends kLine_Verb twice to verb array, and
        // pt1, pt2 to SkPoint array.
        let p = PathBuilder::new()
            .conic_to((10.0, 5.0), (10.0, 10.0), f32::INFINITY)
            .detach();
        reporter_assert!(reporter, !p.is_empty());
        reporter_assert!(reporter, p.verbs().len() == 3); // moveTo, lineTo, lineTo
        reporter_assert!(reporter, p.points().len() == 3);
        reporter_assert!(reporter, p.conic_weights().is_empty());
        reporter_assert!(reporter, p.segment_masks() == PathSegmentMask::LINE);
    }

    {
        // If w is 0, appends kLine_Verb once to verb array, and
        // pt2 to SkPoint array.
        let p = PathBuilder::new()
            .conic_to((10.0, 5.0), (10.0, 10.0), 0.0)
            .detach();
        reporter_assert!(reporter, !p.is_empty());
        reporter_assert!(reporter, p.verbs().len() == 2); // moveTo, lineTo
        reporter_assert!(reporter, p.points().len() == 2);
        reporter_assert!(reporter, p.conic_weights().is_empty());
        reporter_assert!(reporter, p.segment_masks() == PathSegmentMask::LINE);
    }
});

// Port of: tests/PathBuilderTest.cpp#L1127-L1176 (chrome/m156)
def_test!(
    #[allow(clippy::excessive_precision)] // float literals are copied verbatim from the C++
    SkPathBuilder_b520944501,
    |_reporter| {
        let path = PathBuilder::new_with_fill_type(PathFillType::InverseEvenOdd)
            .move_to((0.0, 0.0))
            .cubic_to(
                (8.513_308_95e-18, 4.802_158_61e+30),
                (4.802_153_47e+30, 4.738_883_68e-38),
                (2.369_427_83e-38, 7.548_308_52e-30),
            )
            .move_to((4.802_158_31e+30, 1.039_691_41e-21))
            .conic_to(
                (4.802_158_31e+30, 4.738_883_68e-38),
                (2.401_079_15e+30, 4.738_883_68e-38),
                0.707_106_769,
            )
            .conic_to(
                (2.077_929_4e-21, 4.738_883_68e-38),
                (2.077_929_4e-21, 1.039_691_41e-21),
                0.707_106_769,
            )
            .conic_to(
                (2.077_929_4e-21, 2.079_382_82e-21),
                (2.401_079_15e+30, 2.079_382_82e-21),
                0.707_106_769,
            )
            .conic_to(
                (4.802_158_31e+30, 2.079_382_82e-21),
                (4.802_158_31e+30, 1.039_691_41e-21),
                0.707_106_769,
            )
            .close()
            .detach();

        let mut m = Matrix::default();
        m.set_all(
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10_159_839_284_371_128_320.0,
            36_261_335_138_304_000.0,
            0.0,
            -0.0,
        );

        // This previously caused an issue where one of the conic weights turned to
        // 0, causing an assert later. We should be avoiding that.
        let transformed = path.make_transform(&m);
        let meas = PathMeasure::new(&transformed, false, None);
        let _ = meas.length();
    }
);
