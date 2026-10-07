// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathDataTest.cpp (chrome/m156)

#![cfg(test)]

use std::sync::Arc;

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_data::PathData;
use skia_rust_core::path_enums::{PathConvexity, ResolveConvexity};
use skia_rust_core::path_priv;
use skia_rust_core::path_types::{PathDirection, PathFillType, PathVerb};
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

// Port of: tests/PathDataTest.cpp#L19-L29 (chrome/m156)
fn spaneq<T: PartialEq>(a: &[T], b: &[T]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for i in 0..a.len() {
        if a[i] != b[i] {
            return false;
        }
    }
    true
}

// Port of: tests/PathDataTest.cpp#L32-L50 (chrome/m156)
def_test!(pathdata_empty, |reporter| {
    let pdata = PathData::empty();

    reporter_assert!(reporter, pdata.is_empty());
    reporter_assert!(reporter, pdata.points().is_empty());
    reporter_assert!(reporter, pdata.conics().is_empty());
    reporter_assert!(reporter, pdata.verbs().is_empty());

    reporter_assert!(reporter, *pdata.bounds() == Rect::new_empty());
    reporter_assert!(reporter, pdata.segment_mask() == 0);

    reporter_assert!(reporter, pdata.as_line().is_none());
    reporter_assert!(reporter, pdata.as_oval().is_none());
    reporter_assert!(reporter, pdata.as_rect().is_none());
    reporter_assert!(reporter, pdata.as_rrect().is_none());

    let xformed = pdata.make_transform(&Matrix::scale((2.0, 3.0)));
    reporter_assert!(reporter, xformed.is_some_and(|x| *pdata == *x));
});

type IsAPredicate = fn(&PathData) -> bool;

// Port of: tests/PathDataTest.cpp#L54-L72 (chrome/m156)
fn check_as_a_transforms(
    reporter: &mut Reporter,
    orig: &Arc<PathData>,
    returns_as_a: IsAPredicate,
) {
    debug_assert!(returns_as_a(orig));

    let pairs = [
        (Matrix::i().clone(), true),
        (Matrix::translate((1.0, 2.0)), true),
        (Matrix::scale((2.0, 3.0)), true),
        (Matrix::rotate_deg(30.0), false),
    ];

    for (mx, expected_as_a) in &pairs {
        let pdata = orig.make_transform(mx).expect("finite transform");
        reporter_assert!(reporter, returns_as_a(&pdata) == *expected_as_a);
    }
}

/*
 *  Different ways to "make" a rectangular PathData
 */
type RectMaker = fn(&Rect, PathDirection) -> Option<Arc<PathData>>;

// Port of: tests/PathDataTest.cpp#L79-L81 (chrome/m156)
fn factory_rect(r: &Rect, d: PathDirection) -> Option<Arc<PathData>> {
    PathData::rect(r, d, 0)
}
// Port of: tests/PathDataTest.cpp#L82-L85 (chrome/m156)
fn poly4_rect(r: &Rect, d: PathDirection) -> Option<Arc<PathData>> {
    let pts = r.to_quad(d);
    PathData::polygon(&pts, true)
}
// Port of: tests/PathDataTest.cpp#L86-L91 (chrome/m156)
fn poly5_rect(r: &Rect, d: PathDirection) -> Option<Arc<PathData>> {
    let mut pts = [Point::default(); 5];
    r.copy_to_quad(&mut pts, d);
    pts[4] = pts[0]; // explicly add the closing line
    PathData::polygon(&pts, true)
}
// Port of: tests/PathDataTest.cpp#L92-L96 (chrome/m156)
fn builder_rect_rect(r: &Rect, d: PathDirection) -> Option<Arc<PathData>> {
    let mut bu = PathBuilder::new();
    bu.add_rect(r, d, None);
    bu.detach_data()
}
// Port of: tests/PathDataTest.cpp#L97-L102 (chrome/m156)
fn builder_poly4_rect(r: &Rect, d: PathDirection) -> Option<Arc<PathData>> {
    let pts = r.to_quad(d);
    let mut bu = PathBuilder::new();
    bu.add_polygon(&pts, true);
    bu.detach_data()
}
// Port of: tests/PathDataTest.cpp#L103-L110 (chrome/m156)
fn builder_poly5_rect(r: &Rect, d: PathDirection) -> Option<Arc<PathData>> {
    let mut pts = [Point::default(); 5];
    r.copy_to_quad(&mut pts, d);
    pts[4] = pts[0]; // explicly add the closing line
    let mut bu = PathBuilder::new();
    bu.add_polygon(&pts, true);
    bu.detach_data()
}

const RECT_MAKERS: [RectMaker; 6] = [
    factory_rect,
    poly4_rect,
    poly5_rect,
    builder_rect_rect,
    builder_poly4_rect,
    builder_poly5_rect,
];

// Port of: tests/PathDataTest.cpp#L121-L165 (chrome/m156)
def_test!(
    #[allow(clippy::float_cmp)] // exact float comparisons, as in C++
    pathdata_rect,
    |reporter| {
        let r = Rect::new(1.0, 2.0, 3.0, 4.0);

        let sign = |x: f32| -> f32 {
            if x == 0.0 {
                0.0
            } else if x > 0.0 {
                1.0
            } else {
                -1.0
            }
        };

        for maker in RECT_MAKERS {
            for dir in [PathDirection::CW, PathDirection::CCW] {
                let pdata = maker(&r, dir).expect("valid rect");
                reporter_assert!(reporter, r == *pdata.bounds());

                // 1. manually determine if *we* think it is a rect

                let pts = pdata.points();
                reporter_assert!(reporter, Rect::bounds(pts) == Some(r));

                let cross_sign: f32 = if dir == PathDirection::CW { 1.0 } else { -1.0 };
                for i in 1..3 {
                    let u: Vector = pts[i] - pts[i - 1];
                    let v: Vector = pts[i + 1] - pts[i];
                    let cross = u.cross(v);
                    let dot = u.dot(v);
                    reporter_assert!(reporter, dot == 0.0);
                    reporter_assert!(reporter, cross_sign == sign(cross));
                }

                // 2. now ask the pathdata

                let isa = pdata.as_rect();
                reporter_assert!(reporter, isa.is_some());
                if let Some(isa) = isa {
                    reporter_assert!(reporter, isa.rect == r);
                    reporter_assert!(reporter, isa.direction == dir);
                    reporter_assert!(reporter, isa.start_index == 0);
                }

                check_as_a_transforms(reporter, &pdata, |pd| pd.as_rect().is_some());
            }
        }
    }
);

// Port of: tests/PathDataTest.cpp#L167-L169 (chrome/m156)
fn factory_poly(pts: &[Point], is_closed: bool) -> Option<Arc<PathData>> {
    PathData::polygon(pts, is_closed)
}
// Port of: tests/PathDataTest.cpp#L170-L174 (chrome/m156)
fn builder_poly(pts: &[Point], is_closed: bool) -> Option<Arc<PathData>> {
    let mut bu = PathBuilder::new();
    bu.add_polygon(pts, is_closed);
    bu.detach_data()
}

// Port of: tests/PathDataTest.cpp#L176-L222 (chrome/m156)
def_test!(
    #[allow(clippy::type_complexity)] // mirrors the C++ helper signature
    pathdata_polygon,
    |reporter| {
        let points = [
            Point::new(0.0, 1.0),
            Point::new(2.0, 3.0),
            Point::new(4.0, 5.0),
            Point::new(6.0, 7.0),
            Point::new(8.0, 9.0),
        ];

        let makers: [fn(&[Point], bool) -> Option<Arc<PathData>>; 2] = [factory_poly, builder_poly];
        for maker in makers {
            for is_closed in [false, true] {
                for n in 0..=points.len() {
                    let pts = &points[..n];
                    let pdata = maker(pts, is_closed).expect("valid polygon");

                    let should_be_empty = if is_closed { n == 0 } else { n <= 1 };
                    if should_be_empty {
                        reporter_assert!(reporter, pdata.is_empty());
                        continue;
                    }

                    reporter_assert!(reporter, spaneq(pdata.points(), pts));
                    reporter_assert!(reporter, pdata.conics().is_empty());

                    let line = pdata.as_line();
                    if n == 2 && !is_closed {
                        reporter_assert!(reporter, line.is_some());
                        if let Some(line) = line {
                            reporter_assert!(reporter, line[0] == points[0]);
                            reporter_assert!(reporter, line[1] == points[1]);
                        }

                        let pline = PathData::line(points[0], points[1]).expect("valid line");
                        reporter_assert!(reporter, *pline == *pdata);
                    } else {
                        reporter_assert!(reporter, line.is_none());
                    }

                    let expected_verbs = pts.len() + usize::from(is_closed);
                    let vbs = pdata.verbs();
                    reporter_assert!(reporter, vbs.len() == expected_verbs);

                    reporter_assert!(reporter, vbs[0] == PathVerb::Move);
                    for v in &vbs[1..pts.len()] {
                        reporter_assert!(reporter, *v == PathVerb::Line);
                    }
                    if is_closed {
                        reporter_assert!(reporter, vbs[vbs.len() - 1] == PathVerb::Close);
                    }
                }
            }
        }
    }
);

// Port of: tests/PathDataTest.cpp#L224-L226 (chrome/m156)
fn factory_oval(r: &Rect, dir: PathDirection, start: u32) -> Option<Arc<PathData>> {
    PathData::oval(r, dir, start)
}
// Port of: tests/PathDataTest.cpp#L227-L231 (chrome/m156)
fn builder_oval(r: &Rect, dir: PathDirection, start: u32) -> Option<Arc<PathData>> {
    let mut bu = PathBuilder::new();
    bu.add_oval(r, dir, start as usize);
    bu.detach_data()
}

type OvalMaker = fn(&Rect, PathDirection, u32) -> Option<Arc<PathData>>;

// Port of: tests/PathDataTest.cpp#L233-L256 (chrome/m156)
def_test!(
    #[allow(clippy::items_after_statements)] // constants stay next to the C++ code they mirror
    pathdata_oval,
    |reporter| {
        let bounds = Rect::new(1.0, 2.0, 3.0, 4.0);
        const START_INDEX_COUNT: u32 = 4;

        let makers: [OvalMaker; 2] = [factory_oval, builder_oval];
        for maker in makers {
            for dir in [PathDirection::CW, PathDirection::CCW] {
                for start in 0..START_INDEX_COUNT {
                    let pdata = maker(&bounds, dir, start).expect("valid oval");

                    reporter_assert!(reporter, *pdata.bounds() == bounds);

                    let oval = pdata.as_oval();
                    reporter_assert!(reporter, oval.is_some());
                    if let Some(oval) = oval {
                        reporter_assert!(reporter, oval.bounds == bounds);
                        reporter_assert!(reporter, oval.direction == dir);
                        reporter_assert!(reporter, u32::from(oval.start_index) == start);
                    }

                    check_as_a_transforms(reporter, &pdata, |pd| pd.as_oval().is_some());
                }
            }
        }
    }
);

// Port of: tests/PathDataTest.cpp#L258-L260 (chrome/m156)
fn factory_rrect(r: &RRect, dir: PathDirection, start: u32) -> Option<Arc<PathData>> {
    PathData::rrect(r, dir, start)
}
// Port of: tests/PathDataTest.cpp#L261-L265 (chrome/m156)
fn builder_rrect(r: &RRect, dir: PathDirection, start: u32) -> Option<Arc<PathData>> {
    let mut bu = PathBuilder::new();
    bu.add_rrect(r, dir, start as usize);
    bu.detach_data()
}

type RRectMaker = fn(&RRect, PathDirection, u32) -> Option<Arc<PathData>>;

// Port of: tests/PathDataTest.cpp#L267-L291 (chrome/m156)
def_test!(
    #[allow(clippy::items_after_statements)] // constants stay next to the C++ code they mirror
    pathdata_rrect,
    |reporter| {
        let bounds = Rect::new(0.0, 0.0, 20.0, 30.0);
        let rrect = RRect::new_rect_xy(bounds, 2.0, 3.0);
        const START_INDEX_COUNT: u32 = 8;

        let makers: [RRectMaker; 2] = [factory_rrect, builder_rrect];
        for maker in makers {
            for dir in [PathDirection::CW, PathDirection::CCW] {
                for start in 0..START_INDEX_COUNT {
                    let pdata = maker(&rrect, dir, start).expect("valid rrect");

                    reporter_assert!(reporter, *pdata.bounds() == bounds);

                    let rr = pdata.as_rrect();
                    reporter_assert!(reporter, rr.is_some());
                    if let Some(rr) = rr {
                        reporter_assert!(reporter, rr.rrect == rrect);
                        reporter_assert!(reporter, rr.direction == dir);
                        reporter_assert!(reporter, u32::from(rr.start_index) == start);
                    }

                    check_as_a_transforms(reporter, &pdata, |pd| pd.as_rrect().is_some());
                }
            }
        }
    }
);

// Port of: tests/PathDataTest.cpp#L293-L360 (chrome/m156)
def_test!(
    #[allow(clippy::items_after_statements)] // constants stay next to the C++ code they mirror
    pathdata_make_edgecases,
    |reporter| {
        // just create some points for our tests
        let mut pts = [Point::default(); 20];
        for (i, p) in pts.iter_mut().enumerate() {
            #[allow(clippy::cast_precision_loss)] // small index, as in C++
            let f = i as f32 * 1.0;
            *p = Point::new(f, f);
        }
        let conic_weights = [1.5f32, 2.0, 3.0];

        use PathVerb::{Close as X, Conic as K, Cubic as C, Line as L, Move as M, Quad as Q};

        // only these two sequence will result in an "empty" PathData

        let mv = [M]; // the M will not be trimmed

        reporter_assert!(
            reporter,
            PathData::make(&[], &[], &[]).is_some_and(|p| p.is_empty())
        );
        reporter_assert!(
            reporter,
            PathData::make(&pts[..1], &mv, &[]).is_some_and(|p| !p.is_empty())
        );

        // these sequenes are all illegal (bad verb sequencing)

        let bad0 = [L]; // didn't start with M
        let bad1 = [M, M, L]; // consecutive Ms
        let bad2 = [M, L, X, X]; // consecutive Xs
        let bad3 = [M, L, M, M]; // consecutive Ms

        reporter_assert!(reporter, PathData::make(&pts[..1], &bad0, &[]).is_none());
        reporter_assert!(reporter, PathData::make(&pts[..3], &bad1, &[]).is_none());
        reporter_assert!(reporter, PathData::make(&pts[..2], &bad2, &[]).is_none());
        reporter_assert!(reporter, PathData::make(&pts[..4], &bad3, &[]).is_none());

        // Odd but legal, the trailing M is preserved, but not part of the bounds

        let trimmed = [M, L, M]; // legal
        let pdata = PathData::make(&pts[..3], &trimmed, &[]).expect("legal");

        reporter_assert!(reporter, pdata.points().len() == 3);
        reporter_assert!(reporter, pdata.verbs().len() == 3);
        reporter_assert!(reporter, Some(*pdata.bounds()) == Rect::bounds(&pts[..2]));

        // Now check on # of points and conic weights

        let verbs = [M, L, Q, K, C, X, M]; // 1+1+2+2+3+0+1 = 10 + 1 conic weight

        struct Combo {
            n_pts: usize,
            n_conics: usize,
            success: bool,
        }
        let combos = [
            Combo {
                n_pts: 10,
                n_conics: 1,
                success: true,
            }, // just right
            Combo {
                n_pts: 9,
                n_conics: 1,
                success: false,
            }, // not enough points
            Combo {
                n_pts: 11,
                n_conics: 1,
                success: false,
            }, // too many points
            Combo {
                n_pts: 10,
                n_conics: 0,
                success: false,
            }, // not enough conics
            Combo {
                n_pts: 10,
                n_conics: 2,
                success: false,
            }, // too many conics
            Combo {
                n_pts: 0,
                n_conics: 1,
                success: false,
            }, // degenerate, should not crash on moveto trim
        ];
        for c in &combos {
            let pdata = PathData::make(&pts[..c.n_pts], &verbs, &conic_weights[..c.n_conics]);
            if c.success {
                reporter_assert!(reporter, pdata.is_some());
            } else {
                reporter_assert!(reporter, pdata.is_none());
            }
        }
    }
);

// Port of: tests/PathDataTest.cpp#L362-L370 (chrome/m156)
fn make_bad_rrect() -> Option<RRect> {
    let big = f32::MAX;
    let mut rr = RRect::new_rect_xy(Rect::new(0.0, 0.0, big, big), 4.0, 4.0);
    rr.offset((big, big));
    if !rr.rect().is_finite() {
        return Some(rr);
    }
    None // failed to make a non-finite rrect
}

/*
 *  Test that we cannot make a non-finite PathData
 */
// Port of: tests/PathDataTest.cpp#L375-L415 (chrome/m156)
def_test!(pathdata_make_nonfinite, |reporter| {
    let inf = f32::INFINITY;

    let mut pts = [
        Point::new(0.0, 0.0),
        Point::new(inf, 1.0),
        Point::new(2.0, 4.0),
    ];
    let vbs = [PathVerb::Move, PathVerb::Conic];
    let mut weights = [2.0f32];

    let pdata = PathData::make(&pts, &vbs, &weights);
    reporter_assert!(reporter, pdata.is_none());

    pts[1].x = 3.0; // remove non-finite from pts
    #[allow(clippy::zero_divided_by_zero)]
    let bad_w_values = [-1.0, inf, -inf, inf * 0.0 /* nan */];
    for bad in bad_w_values {
        weights[0] = bad;
        reporter_assert!(reporter, PathData::make(&pts, &vbs, &weights).is_none());
    }

    let r = Rect::new(1.0, 2.0, inf, 4.0);
    reporter_assert!(reporter, PathData::rect_default(&r).is_none());
    reporter_assert!(reporter, PathData::oval_default(&r).is_none());

    // Most RRect methods 'sanitize' the values before returning the RRect, so it hard to
    // actually make one for testing. If our attempt suceeds, we will test with it.
    if let Some(rr) = make_bad_rrect() {
        reporter_assert!(
            reporter,
            PathData::rrect_default(&rr, PathDirection::DEFAULT).is_none()
        );
    }

    pts[1].x = inf; // restore non-finite value
    reporter_assert!(reporter, PathData::polygon(&pts, false).is_none());

    {
        // Non-finite trailing moves should also be rejected.
        let v = [PathVerb::Move, PathVerb::Line, PathVerb::Move];
        let p = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(inf, 20.0),
        ];
        reporter_assert!(reporter, PathData::make(&p, &v, &[]).is_none());
    }
});

// Port of: tests/PathDataTest.cpp#L417-L447 (chrome/m156)
def_test!(pathdata_transform, |reporter| {
    let r = Rect::new(10.0, 20.0, 30.0, 40.0);
    let data = PathData::oval_default(&r).expect("finite oval");

    let mx = Matrix::i();
    let newd = data.make_transform(mx);
    reporter_assert!(reporter, newd.is_some_and(|n| *n == *data));

    let mx = Matrix::translate((5.0, 6.0));
    let newd = data.make_transform(&mx);
    reporter_assert!(
        reporter,
        newd.is_some_and(|n| *n.bounds() == r.with_offset((5.0, 6.0)))
    );

    let mx = Matrix::scale((0.5, 2.0));
    let newd = data.make_transform(&mx);
    let r2 = Rect::new(r.left * 0.5, r.top * 2.0, r.right * 0.5, r.bottom * 2.0);
    reporter_assert!(reporter, newd.is_some_and(|n| *n.bounds() == r2));

    let mx = Matrix::scale((f32::INFINITY, 2.0));
    let newd = data.make_transform(&mx);
    reporter_assert!(reporter, newd.is_none());

    let mx = Matrix::scale((f32::NAN, 2.0));
    let newd = data.make_transform(&mx);
    reporter_assert!(reporter, newd.is_none());
});

/*
 *  This tests how convexity is tracked under transformation
 *  1. unknown stays unknown (we don't actively compute convexity)
 *  2. concave stays concave
 *  3. convex ... may stay convex -- it depends if we feel it is (numerically) safe.
 *     See SkPathPriv::TransformConvexity() for the current heuristics.
 *  4. The (above) helper is shared with SkPath::transform(), so it and SkPathData
 *     should handle transforms + convexity the same.
 */
// Port of: tests/PathDataTest.cpp#L458-L507 (chrome/m156)
def_test!(pathdata_transform_convexity, |reporter| {
    let pts = [
        Point::new(0.0, 0.0),
        Point::new(100.0, 0.0),
        Point::new(200.0, 0.0),
        Point::new(200.0, 200.0),
    ];
    // needed late for our assumpts about convexity preservation
    reporter_assert!(reporter, path_priv::is_axis_aligned(&pts));

    let src = PathData::polygon(&pts, true).expect("finite polygon");
    let convexity = path_priv::get_convexity_or_unknown_data(&src);

    // don't do any work we didn't ask for
    reporter_assert!(reporter, convexity == PathConvexity::Unknown);
    let mut raw = src.raw(PathFillType::DEFAULT, ResolveConvexity::No);
    reporter_assert!(reporter, raw.convexity == PathConvexity::Unknown);
    // now ask for it
    raw = src.raw(PathFillType::DEFAULT, ResolveConvexity::Yes);
    reporter_assert!(reporter, raw.is_known_to_be_convex());

    // For these matrices, given that our points are axis-aligned, we should be able
    // to preserve whatever convexity our src has.

    let safe_matrices = [
        Matrix::default(),
        Matrix::translate((1.0, 2.0)),
        Matrix::scale((2.0, 3.0)),
    ];
    let convexities = [
        PathConvexity::Unknown,
        PathConvexity::ConvexCW, // matches our test data
        PathConvexity::Concave,
    ];
    for mx in &safe_matrices {
        for &conv in &convexities {
            raw.convexity = conv;
            let dst = PathData::make_transform_raw(&raw, mx).expect("finite");
            let convexity = path_priv::get_convexity_or_unknown_data(&dst);
            reporter_assert!(reporter, convexity == conv);
        }
    }

    // for this matrix, we do not expect to preserve convexity
    // (since we don't choose to actually compute convexity at this stage)
    let mx = Matrix::rotate_deg(30.0);
    for &conv in &convexities {
        raw.convexity = conv;
        let dst = PathData::make_transform_raw(&raw, &mx).expect("finite");
        let convexity = path_priv::get_convexity_or_unknown_data(&dst);
        let expected = if conv.is_convex() {
            PathConvexity::Unknown
        } else {
            conv
        };
        reporter_assert!(reporter, convexity == expected);
    }
});

// Port of: tests/PathDataTest.cpp#L509-L552 (chrome/m156)
def_test!(pathdata_inverted_bounds, |reporter| {
    let check = |reporter: &mut Reporter, maker: &dyn Fn(&Rect) -> Option<Arc<PathData>>| {
        let bounds = Rect::new(-10.0, -10.0, 10.0, 10.0);
        let inverted_bounds = Rect::new(10.0, 10.0, -10.0, -10.0);
        reporter_assert!(
            reporter,
            maker(&bounds).is_some_and(|p| *p.bounds() == bounds)
        );
        reporter_assert!(
            reporter,
            maker(&inverted_bounds).is_some_and(|p| *p.bounds() == bounds)
        );
    };

    {
        let makers: [RectMaker; 2] = [factory_rect, builder_rect_rect];
        for maker in makers {
            for dir in [PathDirection::CW, PathDirection::CCW] {
                check(reporter, &|r: &Rect| maker(r, dir));
            }
        }
    }

    {
        const START_INDEX_COUNT: u32 = 4;
        let makers: [OvalMaker; 2] = [factory_oval, builder_oval];
        for maker in makers {
            for dir in [PathDirection::CW, PathDirection::CCW] {
                for start in 0..START_INDEX_COUNT {
                    check(reporter, &|r: &Rect| maker(r, dir, start));
                }
            }
        }
    }

    {
        const START_INDEX_COUNT: u32 = 8;
        let makers: [RRectMaker; 2] = [factory_rrect, builder_rrect];
        for maker in makers {
            for dir in [PathDirection::CW, PathDirection::CCW] {
                for start in 0..START_INDEX_COUNT {
                    for sign in [1.0f32, -1.0] {
                        check(reporter, &|r: &Rect| {
                            let rrect = RRect::new_rect_xy(r, sign * 2.0, sign * 3.0);
                            maker(&rrect, dir, start)
                        });
                    }
                }
            }
        }
    }
});
