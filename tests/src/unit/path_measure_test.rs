// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathMeasureTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::contour_measure::{ContourMeasure, ContourMeasureIter};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_measure::{PathMeasure, path_measure_priv};
use skia_rust_core::path_types::PathVerb;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::scalar::{SCALAR_1, SCALAR_HALF, SCALAR_PI, Scalar, scalar};

// Port of: tests/PathMeasureTest.cpp#L23-L60 (chrome/m156)
fn test_small_segment3(reporter: &mut Reporter) {
    let pts = [
        Point::new(0.0, 0.0),
        Point::new(100_000_000_000.0, 100_000_000_000.0),
        Point::new(0.0, 0.0),
        Point::new(10.0, 10.0),
        Point::new(10.0, 10.0),
        Point::new(0.0, 0.0),
        Point::new(10.0, 10.0),
    ];

    let mut builder = PathBuilder::new();
    builder.move_to(pts[0]);
    let mut i = 1;
    while i < pts.len() {
        builder.cubic_to(pts[i], pts[i + 1], pts[i + 2]);
        i += 3;
    }

    let path = builder.detach();
    let meas = PathMeasure::new(&path, false, None);
    let _ = meas.length();

    // Now check that we cap the segment size even with very large resolution scales.
    // Earlier versions allowed the pathmeasure to recurse without limit in the face
    // of a very large scale.
    //
    //  Before this limit, the above meas had 15K segments, and when built with
    // a resScale of 100, it had 184K segments -- for 1 cubic!
    {
        let n = path_measure_priv::count_segments(&meas);
        reporter_assert!(reporter, n < 300);

        let res_scale = 1000.0;
        let n = path_measure_priv::count_segments(&PathMeasure::new(&path, false, res_scale));
        reporter_assert!(reporter, n < 300);
    }
}

// Port of: tests/PathMeasureTest.cpp#L62-L77 (chrome/m156)
fn test_small_segment2() {
    let pts = [
        Point::new(0.0, 0.0),
        Point::new(100_000_000_000.0, 100_000_000_000.0),
        Point::new(0.0, 0.0),
        Point::new(10.0, 10.0),
        Point::new(0.0, 0.0),
    ];

    let mut builder = PathBuilder::new();
    builder.move_to(pts[0]);
    let mut i = 1;
    while i < pts.len() {
        builder.quad_to(pts[i], pts[i + 1]);
        i += 2;
    }
    let meas = PathMeasure::new(&builder.detach(), false, None);
    let _ = meas.length();
}

// Port of: tests/PathMeasureTest.cpp#L79-L104 (chrome/m156)
fn test_small_segment() {
    let pts = [
        Point::new(100_000.0, 100_000.0),
        // big jump between these points, makes a big segment
        Point::new(1.0005, 0.9999),
        // tiny (non-zero) jump between these points
        Point::new(SCALAR_1, SCALAR_1),
    ];

    let meas = PathMeasure::new(&Path::polygon(&pts, false, None, None), false, None);

    /*  this would assert (before a fix) because we added a segment with
       the same length as the prev segment, due to the follow (bad) pattern

       d = distance(pts[0], pts[1]);
       distance += d;
       seg->fDistance = distance;

       SkASSERT(d > 0);    // TRUE
       SkASSERT(seg->fDistance > prevSeg->fDistance);  // FALSE

       This 2nd assert failes because (distance += d) didn't affect distance
       because distance >>> d.
    */
    let _ = meas.length();
}

// Port of: tests/PathMeasureTest.cpp#L106-L237 (chrome/m156)
def_test!(
    #[allow(clippy::float_cmp)] // exact float comparisons, as in C++
    PathMeasure,
    |reporter| {
        let mut path = PathBuilder::new()
            .move_to((0.0, 0.0))
            .line_to((1.0, 0.0))
            .line_to((1.0, 1.0))
            .line_to((0.0, 1.0))
            .detach();

        let mut meas = PathMeasure::new(&path, true, None);
        let mut length = meas.length();
        debug_assert_eq!(length, 4.0);

        path = Path::line((0.0, 0.0), (3.0, 4.0));
        meas.set_path(&path, false);
        length = meas.length();
        reporter_assert!(reporter, length == 5.0);

        path = Path::circle((0.0, 0.0), 1.0, None);
        meas.set_path(&path, true);
        let _ = meas.length();
        //    SkDebugf("circle arc-length = %g\n", length);

        // Test the behavior following a close not followed by a move.
        path = PathBuilder::new()
            .line_to((1.0, 0.0))
            .line_to((1.0, 1.0))
            .line_to((0.0, 1.0))
            .close()
            .line_to((-1.0, 0.0))
            .detach();
        meas.set_path(&path, false);
        length = meas.length();
        reporter_assert!(reporter, length == SCALAR_1 * 4.0);
        meas.next_contour();
        length = meas.length();
        reporter_assert!(reporter, length == SCALAR_1);
        let mut position = Point::default();
        let mut tangent = Vector::default();
        reporter_assert!(
            reporter,
            meas.get_pos_tan(SCALAR_HALF, Some(&mut position), Some(&mut tangent))
        );
        reporter_assert!(
            reporter,
            scalar::nearly_equal(position.x, -SCALAR_HALF, 0.0001)
        );
        reporter_assert!(reporter, position.y == 0.0);
        reporter_assert!(reporter, tangent.x == -SCALAR_1);
        reporter_assert!(reporter, tangent.y == 0.0);

        // Test degenerate paths
        path = PathBuilder::new()
            .move_to((0.0, 0.0))
            .line_to((0.0, 0.0))
            .line_to((1.0, 0.0))
            .quad_to((1.0, 0.0), (1.0, 0.0))
            .quad_to((1.0, 1.0), (1.0, 2.0))
            .cubic_to((1.0, 2.0), (1.0, 2.0), (1.0, 2.0))
            .cubic_to((2.0, 2.0), (3.0, 2.0), (4.0, 2.0))
            .detach();
        meas.set_path(&path, false);
        length = meas.length();
        reporter_assert!(reporter, length == SCALAR_1 * 6.0);
        reporter_assert!(
            reporter,
            meas.get_pos_tan(SCALAR_HALF, Some(&mut position), Some(&mut tangent))
        );
        reporter_assert!(
            reporter,
            scalar::nearly_equal(position.x, SCALAR_HALF, 0.0001)
        );
        reporter_assert!(reporter, position.y == 0.0);
        reporter_assert!(reporter, tangent.x == SCALAR_1);
        reporter_assert!(reporter, tangent.y == 0.0);
        reporter_assert!(
            reporter,
            meas.get_pos_tan(2.5, Some(&mut position), Some(&mut tangent))
        );
        reporter_assert!(reporter, scalar::nearly_equal(position.x, SCALAR_1, 0.0001));
        reporter_assert!(reporter, scalar::nearly_equal(position.y, 1.5, None));
        reporter_assert!(reporter, tangent.x == 0.0);
        reporter_assert!(reporter, tangent.y == SCALAR_1);
        reporter_assert!(
            reporter,
            meas.get_pos_tan(4.5, Some(&mut position), Some(&mut tangent))
        );
        reporter_assert!(reporter, scalar::nearly_equal(position.x, 2.5, 0.0001));
        reporter_assert!(reporter, scalar::nearly_equal(position.y, 2.0, 0.0001));
        reporter_assert!(reporter, tangent.x == SCALAR_1);
        reporter_assert!(reporter, tangent.y == 0.0);

        path = PathBuilder::new()
            .move_to((0.0, 0.0))
            .line_to((1.0, 0.0))
            .move_to((1.0, 1.0))
            .move_to((2.0, 2.0))
            .line_to((1.0, 2.0))
            .detach();
        meas.set_path(&path, false);
        length = meas.length();
        reporter_assert!(reporter, length == 1.0);
        reporter_assert!(
            reporter,
            meas.get_pos_tan(0.5, Some(&mut position), Some(&mut tangent))
        );
        reporter_assert!(reporter, scalar::nearly_equal(position.x, 0.5, 0.0001));
        reporter_assert!(reporter, position.y == 0.0);
        reporter_assert!(reporter, tangent.x == 1.0);
        reporter_assert!(reporter, tangent.y == 0.0);
        meas.next_contour();
        length = meas.length();
        reporter_assert!(reporter, length == 1.0);
        reporter_assert!(
            reporter,
            meas.get_pos_tan(0.5, Some(&mut position), Some(&mut tangent))
        );
        reporter_assert!(reporter, scalar::nearly_equal(position.x, 1.5, 0.0001));
        reporter_assert!(reporter, scalar::nearly_equal(position.y, 2.0, 0.0001));
        reporter_assert!(reporter, tangent.x == -1.0);
        reporter_assert!(reporter, tangent.y == 0.0);

        test_small_segment();
        test_small_segment2();
        test_small_segment3(reporter);

        // SkPathMeasure isn't copyable, but it should be move-able
        let meas2 = meas; // SkPathMeasure meas2(std::move(meas));
        let moved_back = meas2; // meas = std::move(meas2);
        drop(moved_back);
    }
);

// Port of: tests/PathMeasureTest.cpp#L239-L255 (chrome/m156)
def_test!(PathMeasureConic, |reporter| {
    let mut std_p = Point::default();
    let mut hi_p = Point::default();
    let pts = [
        Point::new(0.0, 0.0),
        Point::new(100.0, 0.0),
        Point::new(100.0, 0.0),
    ];
    let mut p = PathBuilder::new()
        .move_to((0.0, 0.0))
        .conic_to(pts[1], pts[2], 1.0)
        .detach();
    let mut stdm = PathMeasure::new(&p, false, None);
    reporter_assert!(reporter, stdm.get_pos_tan(20.0, Some(&mut std_p), None));
    p = PathBuilder::new()
        .move_to((0.0, 0.0))
        .conic_to(pts[1], pts[2], 10.0)
        .detach();
    stdm.set_path(&p, false);
    reporter_assert!(reporter, stdm.get_pos_tan(20.0, Some(&mut hi_p), None));
    reporter_assert!(reporter, 19.5 < std_p.x && std_p.x < 20.5);
    reporter_assert!(reporter, 19.5 < hi_p.x && hi_p.x < 20.5);
});

// Regression test for b/26425223
// Port of: tests/PathMeasureTest.cpp#L258-L264 (chrome/m156)
def_test!(PathMeasure_nextctr, |reporter| {
    let path = Path::line((0.0, 0.0), (100.0, 0.0));

    let mut meas = PathMeasure::new(&path, false, None);
    // only expect 1 contour, even if we didn't explicitly call getLength() ourselves
    reporter_assert!(reporter, !meas.next_contour());
});

// Port of: tests/PathMeasureTest.cpp#L266-L279 (chrome/m156)
fn test_90_degrees(cm: &ContourMeasure, radius: scalar, reporter: &mut Reporter) {
    let mut pos = Point::default();
    let mut tan = Vector::default();
    let distance = cm.length() / 4.0;
    let success = cm.get_pos_tan(distance, Some(&mut pos), Some(&mut tan));

    reporter_assert!(reporter, success);
    reporter_assert!(reporter, scalar::nearly_equal(pos.x, 0.0, None));
    reporter_assert!(reporter, scalar::nearly_equal(pos.y, radius, None));
    reporter_assert!(reporter, scalar::nearly_equal(tan.x, -1.0, None));
    reporter_assert!(reporter, scalar::nearly_equal(tan.y, 0.0, None));
}

// Port of: tests/PathMeasureTest.cpp#L281-L303 (chrome/m156)
fn test_empty_contours(reporter: &mut Reporter) {
    let path = PathBuilder::new()
        .move_to((0.0, 0.0))
        .line_to((100.0, 100.0))
        .line_to((200.0, 100.0))
        .move_to((2.0, 2.0))
        .move_to((3.0, 3.0)) // zero-length(s)
        .move_to((4.0, 4.0))
        .close()
        .close()
        .close() // zero-length
        .move_to((5.0, 5.0))
        .line_to((5.0, 5.0)) // zero-length
        .move_to((5.0, 5.0))
        .line_to((5.0, 5.0))
        .close() // zero-length
        .move_to((5.0, 5.0))
        .line_to((5.0, 5.0))
        .close()
        .close() // zero-length
        .move_to((6.0, 6.0))
        .line_to((7.0, 7.0))
        .move_to((10.0, 10.0)) // zero-length
        .detach();

    let mut fact = ContourMeasureIter::new(&path, false, None);

    // given the above construction, we expect only 2 contours (the rest are "empty")

    reporter_assert!(reporter, fact.next().is_some());
    reporter_assert!(reporter, fact.next().is_some());
    reporter_assert!(reporter, fact.next().is_none());
}

// Port of: tests/PathMeasureTest.cpp#L305-L315 (chrome/m156)
fn test_mlm_contours(reporter: &mut Reporter) {
    // This odd sequence (with a trailing moveTo) used to return a 2nd contour, which is
    // wrong, since the contract for a measure is to only return non-zero length contours.
    let path = PathBuilder::new()
        .move_to((10.0, 10.0))
        .line_to((20.0, 20.0))
        .move_to((30.0, 30.0))
        .detach();

    for force_closed in [false, true] {
        let mut fact = ContourMeasureIter::new(&path, force_closed, None);
        reporter_assert!(reporter, fact.next().is_some());
        reporter_assert!(reporter, fact.next().is_none());
    }
}

// Port of: tests/PathMeasureTest.cpp#L317-L344 (chrome/m156)
def_test!(contour_measure, |reporter| {
    let path = PathBuilder::new()
        .add_circle((0.0, 0.0), 100.0, None)
        .add_circle((0.0, 0.0), 10.0, None)
        .detach();

    let mut fact = ContourMeasureIter::new(&path, false, None);

    let cm0 = fact.next().expect("first contour");
    let cm1 = fact.next().expect("second contour");

    reporter_assert!(reporter, cm0.is_closed());
    reporter_assert!(
        reporter,
        scalar::nearly_equal(cm0.length(), 200.0 * SCALAR_PI, 1.5)
    );

    test_90_degrees(&cm0, 100.0, reporter);

    reporter_assert!(reporter, cm1.is_closed());
    reporter_assert!(
        reporter,
        scalar::nearly_equal(cm1.length(), 20.0 * SCALAR_PI, 0.5)
    );

    test_90_degrees(&cm1, 10.0, reporter);

    let cm2 = fact.next();
    reporter_assert!(reporter, cm2.is_none());

    test_empty_contours(reporter);
    test_mlm_contours(reporter);
});

// Port of: tests/PathMeasureTest.cpp#L346-L430 (chrome/m156)
def_test!(
    #[allow(clippy::excessive_precision)] // float literals are copied verbatim from the C++
    #[allow(clippy::neg_cmp_op_on_partial_ord)] // inside reporter_assert!'s `!(cond)`
    contour_measure_verbs,
    |reporter| {
        let path = PathBuilder::new()
            .move_to((10.0, 10.0))
            .line_to((10.0, 30.0))
            .line_to((30.0, 30.0))
            .quad_to((40.0, 30.0), (40.0, 40.0))
            .cubic_to((50.0, 40.0), (50.0, 50.0), (40.0, 50.0))
            .conic_to((50.0, 50.0), (50.0, 60.0), 1.2)
            .detach();

        let mut measure = ContourMeasureIter::new(&path, false, None);

        let cmeasure = measure.next();
        reporter_assert!(reporter, cmeasure.is_some());
        let Some(cmeasure) = cmeasure else { return };

        // skia-rust: `begin()` / `++viter` / `viter != end()` are the Rust iterator's `next()`.
        let mut viter = cmeasure.verbs();
        {
            let vmeasure = viter.next();
            reporter_assert!(reporter, vmeasure.is_some());
            let vmeasure = vmeasure.expect("a verb");
            reporter_assert!(reporter, vmeasure.verb() == PathVerb::Line);
            reporter_assert!(
                reporter,
                scalar::nearly_equal(vmeasure.distance(), 20.0, None)
            );
            reporter_assert!(reporter, vmeasure.points().len() == 2);
            reporter_assert!(reporter, vmeasure.points()[0] == Point::new(10.0, 10.0));
            reporter_assert!(reporter, vmeasure.points()[1] == Point::new(10.0, 30.0));
        }

        {
            let vmeasure = viter.next();
            reporter_assert!(reporter, vmeasure.is_some());
            let vmeasure = vmeasure.expect("a verb");
            reporter_assert!(reporter, vmeasure.verb() == PathVerb::Line);
            reporter_assert!(
                reporter,
                scalar::nearly_equal(vmeasure.distance(), 40.0, None)
            );
            reporter_assert!(reporter, vmeasure.points().len() == 2);
            reporter_assert!(reporter, vmeasure.points()[0] == Point::new(10.0, 30.0));
            reporter_assert!(reporter, vmeasure.points()[1] == Point::new(30.0, 30.0));
        }

        {
            let vmeasure = viter.next();
            reporter_assert!(reporter, vmeasure.is_some());
            let vmeasure = vmeasure.expect("a verb");
            reporter_assert!(reporter, vmeasure.verb() == PathVerb::Quad);
            reporter_assert!(
                reporter,
                scalar::nearly_equal(vmeasure.distance(), 56.127_525, None)
            );
            reporter_assert!(reporter, vmeasure.points().len() == 3);
            reporter_assert!(reporter, vmeasure.points()[0] == Point::new(30.0, 30.0));
            reporter_assert!(reporter, vmeasure.points()[1] == Point::new(40.0, 30.0));
            reporter_assert!(reporter, vmeasure.points()[2] == Point::new(40.0, 40.0));
        }

        {
            let vmeasure = viter.next();
            reporter_assert!(reporter, vmeasure.is_some());
            let vmeasure = vmeasure.expect("a verb");
            reporter_assert!(reporter, vmeasure.verb() == PathVerb::Cubic);
            reporter_assert!(
                reporter,
                scalar::nearly_equal(vmeasure.distance(), 76.004_692, None)
            );
            reporter_assert!(reporter, vmeasure.points().len() == 4);
            reporter_assert!(reporter, vmeasure.points()[0] == Point::new(40.0, 40.0));
            reporter_assert!(reporter, vmeasure.points()[1] == Point::new(50.0, 40.0));
            reporter_assert!(reporter, vmeasure.points()[2] == Point::new(50.0, 50.0));
            reporter_assert!(reporter, vmeasure.points()[3] == Point::new(40.0, 50.0));
        }

        {
            let vmeasure = viter.next();
            reporter_assert!(reporter, vmeasure.is_some());
            let vmeasure = vmeasure.expect("a verb");
            reporter_assert!(reporter, vmeasure.verb() == PathVerb::Conic);
            reporter_assert!(
                reporter,
                scalar::nearly_equal(vmeasure.distance(), 92.428_185, None)
            );
            reporter_assert!(reporter, vmeasure.points().len() == 4);
            reporter_assert!(reporter, vmeasure.points()[0] == Point::new(40.0, 50.0));
            reporter_assert!(reporter, vmeasure.points()[1] == Point::new(1.2, 0.0));
            reporter_assert!(reporter, vmeasure.points()[2] == Point::new(50.0, 50.0));
            reporter_assert!(reporter, vmeasure.points()[3] == Point::new(50.0, 60.0));

            // The last verb distance should also match the contour length.
            reporter_assert!(
                reporter,
                scalar::nearly_equal(vmeasure.distance(), cmeasure.length(), None)
            );
        }

        {
            reporter_assert!(reporter, viter.next().is_none());
        }

        // Exercise the range iterator form.
        let mut current_distance = 0.0;
        let mut verb_count = 0;
        for vmeasure in cmeasure.verbs() {
            reporter_assert!(reporter, vmeasure.distance() > current_distance);
            current_distance = vmeasure.distance();
            verb_count += 1;
        }
        reporter_assert!(reporter, verb_count == 5);
    }
);
