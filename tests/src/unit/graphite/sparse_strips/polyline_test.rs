// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/sparse_strips/PolylineTest.cpp (chrome/m156)

#![cfg(test)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::too_many_lines
)]

use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::SCALAR_NAN;
use skia_rust_gpu::graphite::sparse_strips::flatten::Flatten;
use skia_rust_gpu::graphite::sparse_strips::polyline::{LineIterator, Polyline};
use skia_rust_gpu::graphite::sparse_strips::types::FlattenMode;

use crate::{def_test, reporter_assert};

def_test!(SparseStrips_Polyline, |reporter| {
    // Port of: tests/graphite/sparse_strips/PolylineTest.cpp#L19-L154 (chrome/m156)

    // Append Deduplications
    {
        let mut polyline = Polyline::new();
        reporter_assert!(reporter, polyline.is_empty());
        reporter_assert!(reporter, polyline.count() == 0);

        polyline.append_point(Point::new(0.0, 0.0));
        reporter_assert!(reporter, polyline.count() == 1);

        polyline.append_point(Point::new(0.0, 0.0));
        reporter_assert!(reporter, polyline.count() == 1);

        polyline.append_point(Point::new(1.0, 1.0));
        reporter_assert!(reporter, polyline.count() == 2);

        polyline.append_sentinel();
        reporter_assert!(reporter, polyline.count() == 3);
        reporter_assert!(reporter, polyline.points().last().unwrap().x.is_nan());
        reporter_assert!(reporter, polyline.points().last().unwrap().y.is_nan());

        polyline.append_sentinel();
        reporter_assert!(reporter, polyline.count() == 3);

        polyline.reset();
        reporter_assert!(reporter, polyline.is_empty());
        polyline.append_sentinel();
        reporter_assert!(reporter, polyline.is_empty());
    }

    // Iterator
    {
        let mut polyline = Polyline::new();

        polyline.append_point(Point::new(0.0, 0.0));
        polyline.append_point(Point::new(1.0, 0.0));
        polyline.append_point(Point::new(1.0, 1.0));
        polyline.append_sentinel();

        polyline.append_point(Point::new(5.0, 5.0));
        polyline.append_point(Point::new(6.0, 6.0));
        polyline.append_sentinel();

        polyline.append_point(Point::new(10.0, 10.0));
        polyline.append_sentinel();

        let expected_indices = [0, 1, 4];
        let mut count = 0;

        let mut it = polyline.begin();
        let end = polyline.end();
        while it.differs_from(&end) {
            let idx = it.current().1;
            reporter_assert!(reporter, count < 3);
            if count < 3 {
                reporter_assert!(reporter, idx == expected_indices[count]);
            }
            count += 1;
            it.advance();
        }

        reporter_assert!(reporter, count == 3);
    }

    // Malformed inputs
    {
        const K_NAN: f32 = SCALAR_NAN;
        let nan = Point::new(K_NAN, K_NAN);
        let k_pathological_pts = [
            nan,
            nan,
            nan,
            nan,
            nan,
            Point::new(0.0, 0.0),
            nan,
            nan,
            Point::new(1.0, 1.0),
            Point::new(2.0, 2.0),
            nan,
            nan,
            nan,
            nan,
            nan,
            nan,
            Point::new(3.0, 3.0),
            nan,
            Point::new(4.0, 4.0),
            Point::new(5.0, 5.0),
            Point::new(6.0, 6.0),
            nan,
            Point::new(7.0, 7.0),
            nan,
            nan,
            nan,
            nan,
        ];

        let count = k_pathological_pts.len() as i32;
        let mut it = LineIterator::new(&k_pathological_pts, 0, count);
        let end = LineIterator::new(&k_pathological_pts, count, count);

        reporter_assert!(reporter, it.differs_from(&end));
        reporter_assert!(reporter, it.current().1 == 8);
        it.advance();

        reporter_assert!(reporter, it.differs_from(&end));
        reporter_assert!(reporter, it.current().1 == 18);
        it.advance();

        reporter_assert!(reporter, it.differs_from(&end));
        reporter_assert!(reporter, it.current().1 == 19);
        it.advance();

        reporter_assert!(reporter, !it.differs_from(&end));
    }

    // Empty/Null
    {
        let polyline = Polyline::new();

        let it = polyline.begin();
        let end = polyline.end();

        reporter_assert!(reporter, !it.differs_from(&end));

        let test = [Point::new(0.0, 0.0)];
        let raw_it = LineIterator::new(&test, 0, 0);
        let raw_end = LineIterator::new(&test, 0, 0);

        reporter_assert!(reporter, !raw_it.differs_from(&raw_end));
    }

    // Single Point
    {
        let pts = [Point::new(1.0, 1.0)];
        let it = LineIterator::new(&pts, 0, 1);
        let end = LineIterator::new(&pts, 1, 1);

        reporter_assert!(reporter, !it.differs_from(&end));
    }
});

def_test!(
    #[ignore = "see notes/tests_graphite_sparse_strips_PolylineTest_cpp_SparseStrips_Polyline_Integrated.md"]
    SparseStrips_Polyline_Integrated,
    |reporter| {
        // Port of: tests/graphite/sparse_strips/PolylineTest.cpp#L156-L435 (chrome/m156)

        // Simple rect
        {
            let mut builder = PathBuilder::new();
            builder.add_rect(Rect::from_xywh(10.0, 10.0, 50.0, 50.0), None, None);
            let path: Path = builder.detach();

            let mut flatten = Flatten::new();
            let mut polyline = Polyline::new();

            flatten.process_paths(
                FlattenMode::Scalar,
                &path,
                &Matrix::new_identity(),
                100.0,
                100.0,
                &mut polyline,
            );

            reporter_assert!(reporter, polyline.count() == 6);
            let pts = polyline.points();
            reporter_assert!(reporter, pts[0] == Point::new(10.0, 10.0));
            reporter_assert!(reporter, pts[4] == Point::new(10.0, 10.0));
            reporter_assert!(reporter, pts[5].x.is_nan());

            let mut line_count = 0;
            for (line, _index) in &polyline {
                line_count += 1;
                reporter_assert!(reporter, !line.p0.x.is_nan() && !line.p1.x.is_nan());
            }
            reporter_assert!(reporter, line_count == 4);
        }

        // NaN injection between sub-paths
        {
            let mut builder = PathBuilder::new();
            builder.move_to((0.0, 0.0));
            builder.line_to((10.0, 0.0));

            builder.move_to((20.0, 20.0)); // Triggers sentinel for previous open path
            builder.line_to((30.0, 20.0));
            let path = builder.detach();

            let mut flatten = Flatten::new();
            let mut polyline = Polyline::new();

            flatten.process_paths(
                FlattenMode::Scalar,
                &path,
                &Matrix::new_identity(),
                100.0,
                100.0,
                &mut polyline,
            );

            reporter_assert!(reporter, polyline.count() == 8);

            let pts = polyline.points();
            reporter_assert!(reporter, pts[1] == Point::new(10.0, 0.0));
            reporter_assert!(reporter, pts[2] == Point::new(0.0, 0.0));
            reporter_assert!(reporter, pts[3].x.is_nan());

            reporter_assert!(reporter, pts[4] == Point::new(20.0, 20.0));
            reporter_assert!(reporter, pts[7].x.is_nan());

            let mut line_count = 0;
            for (_line, index) in &polyline {
                line_count += 1;
                reporter_assert!(
                    reporter,
                    index != 2 && index != 3 && index != 6 && index != 7
                );
            }
            reporter_assert!(reporter, line_count == 4);
        }

        // Deduplication
        {
            let mut builder = PathBuilder::new();
            builder.move_to((0.0, 0.0));
            builder.line_to((10.0, 10.0));
            builder.line_to((10.0, 10.0));
            builder.line_to((20.0, 0.0));
            builder.close();
            let path = builder.detach();

            let mut flatten = Flatten::new();
            let mut polyline = Polyline::new();

            flatten.process_paths(
                FlattenMode::Scalar,
                &path,
                &Matrix::new_identity(),
                100.0,
                100.0,
                &mut polyline,
            );

            reporter_assert!(reporter, polyline.count() == 5);
            let pts = polyline.points();
            reporter_assert!(reporter, pts[1] == Point::new(10.0, 10.0));
            reporter_assert!(reporter, pts[2] == Point::new(20.0, 0.0));
        }

        // Simplification
        {
            let mut builder = PathBuilder::new();

            // Left of Viewport
            // Quad
            builder.move_to((-10.0, 10.0));
            builder.quad_to((-20.0, 20.0), (-30.0, 10.0));
            // Conic
            builder.move_to((-10.0, 30.0));
            builder.conic_to((-20.0, 40.0), (-30.0, 30.0), 0.5);
            // Cubic
            builder.move_to((-10.0, 50.0));
            builder.cubic_to((-20.0, 60.0), (-30.0, 60.0), (-40.0, 50.0));

            // Visually flat
            // Quad (Control point is collinear with start and end)
            builder.move_to((10.0, 10.0));
            builder.quad_to((20.0, 10.0), (30.0, 10.0));
            // Conic (Control point is collinear)
            builder.move_to((10.0, 30.0));
            builder.conic_to((20.0, 30.0), (30.0, 30.0), 0.5);
            // Cubic (Both control points are collinear)
            builder.move_to((10.0, 50.0));
            builder.cubic_to((20.0, 50.0), (30.0, 50.0), (40.0, 50.0));

            let path = builder.detach();

            let mut flatten = Flatten::new();
            let mut polyline = Polyline::new();
            flatten.process_paths(
                FlattenMode::Scalar,
                &path,
                &Matrix::new_identity(),
                100.0,
                100.0,
                &mut polyline,
            );
            // Expected per contour: Start Pt, Simplified End Pt, Implicit Close Pt, NaN.
            // 6 contours * 4 pts = 24 points total
            reporter_assert!(reporter, polyline.count() == 24);
            let pts = polyline.points();

            // Left of Viewport Simplification
            reporter_assert!(reporter, pts[0] == Point::new(-10.0, 10.0));
            reporter_assert!(reporter, pts[1] == Point::new(-30.0, 10.0)); // Quad simplified
            reporter_assert!(reporter, pts[2] == Point::new(-10.0, 10.0)); // Close
            reporter_assert!(reporter, pts[3].x.is_nan());

            reporter_assert!(reporter, pts[4] == Point::new(-10.0, 30.0));
            reporter_assert!(reporter, pts[5] == Point::new(-30.0, 30.0)); // Conic simplified
            reporter_assert!(reporter, pts[6] == Point::new(-10.0, 30.0)); // Close
            reporter_assert!(reporter, pts[7].x.is_nan());

            reporter_assert!(reporter, pts[8] == Point::new(-10.0, 50.0));
            reporter_assert!(reporter, pts[9] == Point::new(-40.0, 50.0)); // Cubic simplified
            reporter_assert!(reporter, pts[10] == Point::new(-10.0, 50.0)); // Close
            reporter_assert!(reporter, pts[11].x.is_nan());

            // Visually Flat Simplification
            reporter_assert!(reporter, pts[12] == Point::new(10.0, 10.0));
            reporter_assert!(reporter, pts[13] == Point::new(30.0, 10.0)); // Quad flat
            reporter_assert!(reporter, pts[14] == Point::new(10.0, 10.0)); // Close
            reporter_assert!(reporter, pts[15].x.is_nan());

            reporter_assert!(reporter, pts[16] == Point::new(10.0, 30.0));
            reporter_assert!(reporter, pts[17] == Point::new(30.0, 30.0)); // Conic flat
            reporter_assert!(reporter, pts[18] == Point::new(10.0, 30.0)); // Close
            reporter_assert!(reporter, pts[19].x.is_nan());

            reporter_assert!(reporter, pts[20] == Point::new(10.0, 50.0));
            reporter_assert!(reporter, pts[21] == Point::new(40.0, 50.0)); // Cubic flat
            reporter_assert!(reporter, pts[22] == Point::new(10.0, 50.0)); // Close
            reporter_assert!(reporter, pts[23].x.is_nan());
        }

        // Culling (Entirely top, right, or bottom of the viewport)
        {
            let mut builder = PathBuilder::new();
            builder.move_to((10.0, -10.0));
            builder.quad_to((20.0, -20.0), (30.0, -10.0));

            builder.move_to((110.0, 10.0));
            builder.conic_to((120.0, 20.0), (130.0, 10.0), 0.5);

            builder.move_to((10.0, 110.0));
            builder.cubic_to((20.0, 120.0), (30.0, 120.0), (40.0, 110.0));
            let path = builder.detach();

            let mut flatten = Flatten::new();
            let mut polyline = Polyline::new();
            flatten.process_paths(
                FlattenMode::Scalar,
                &path,
                &Matrix::new_identity(),
                100.0,
                100.0,
                &mut polyline,
            );

            // Curves are fully dropped. The implicit close points match the moveTo points
            // exactly, so Polyline drops the close point via deduplication!
            // Expected per contour: Start Pt, NaN. (2 pts * 3 = 6)
            reporter_assert!(reporter, polyline.count() == 6);
            let pts = polyline.points();

            reporter_assert!(reporter, pts[0] == Point::new(10.0, -10.0));
            reporter_assert!(reporter, pts[1].x.is_nan());

            reporter_assert!(reporter, pts[2] == Point::new(110.0, 10.0));
            reporter_assert!(reporter, pts[3].x.is_nan());

            reporter_assert!(reporter, pts[4] == Point::new(10.0, 110.0));
            reporter_assert!(reporter, pts[5].x.is_nan());
        }

        // Interleaved valid geometry with culling and simplification
        {
            let mut builder = PathBuilder::new();

            // Contour 1: Weaves outside to the left and back
            builder.move_to((10.0, 10.0));
            builder.line_to((20.0, 10.0));
            builder.line_to((-10.0, 10.0)); // Exits left
            builder.quad_to((-20.0, 20.0), (-30.0, 10.0)); // Simplifies to line (-30, 10)
            builder.line_to((10.0, 20.0)); // Re-enters viewport

            // Contour 2: Completely culled contour
            builder.move_to((150.0, 150.0));
            builder.cubic_to((160.0, 160.0), (170.0, 160.0), (180.0, 150.0));

            // Contour 3: Valid normal contour
            builder.move_to((30.0, 30.0));
            builder.line_to((40.0, 30.0));

            let path = builder.detach();

            let mut flatten = Flatten::new();
            let mut polyline = Polyline::new();
            flatten.process_paths(
                FlattenMode::Scalar,
                &path,
                &Matrix::new_identity(),
                100.0,
                100.0,
                &mut polyline,
            );

            // Contour 1: moveTo + 3 lineTos + simplified quad + lineTo + close + NaN = 7 points
            // Contour 2: moveTo + NaN (close is deduplicated due to complete culling) = 2 points
            // Contour 3: moveTo + lineTo + close + NaN = 4 points
            // Total expected = 13 points
            reporter_assert!(reporter, polyline.count() == 13);

            let pts = polyline.points();

            // Verify Contour 1 simplified correctly without dropping points
            reporter_assert!(reporter, pts[0] == Point::new(10.0, 10.0));
            reporter_assert!(reporter, pts[3] == Point::new(-30.0, 10.0)); // Simplified Quad
            reporter_assert!(reporter, pts[4] == Point::new(10.0, 20.0)); // Return line
            reporter_assert!(reporter, pts[5] == Point::new(10.0, 10.0)); // Implicit close
            reporter_assert!(reporter, pts[6].x.is_nan());

            // Verify Contour 2 was fully culled
            reporter_assert!(reporter, pts[7] == Point::new(150.0, 150.0));
            reporter_assert!(reporter, pts[8].x.is_nan());

            // Verify Contour 3 generated successfully despite the previous culling
            reporter_assert!(reporter, pts[9] == Point::new(30.0, 30.0));
            reporter_assert!(reporter, pts[10] == Point::new(40.0, 30.0));
            reporter_assert!(reporter, pts[11] == Point::new(30.0, 30.0));
            reporter_assert!(reporter, pts[12].x.is_nan());
        }

        // Interleaved culled curve within a single contour
        {
            let mut builder = PathBuilder::new();
            builder.move_to((10.0, 10.0));
            builder.line_to((20.0, -10.0)); // Exits top
            builder.quad_to((20.0, -20.0), (30.0, -10.0)); // Culled curve (top of viewport)
            builder.line_to((40.0, 10.0)); // Re-enters viewport
            builder.close();
            let path = builder.detach();

            let mut test_mode = |process_fn: &dyn Fn(&mut Flatten, &Path, &mut Polyline),
                                 name: &str| {
                let mut flatten = Flatten::new();
                let mut polyline = Polyline::new();
                process_fn(&mut flatten, &path, &mut polyline);

                let mut line_count = 0;
                for (line, _index) in &polyline {
                    line_count += 1;
                    // Assert that no line segment directly bridges from (20, -10) to (40, 10)
                    reporter_assert!(
                        reporter,
                        !(line.p0 == Point::new(20.0, -10.0) && line.p1 == Point::new(40.0, 10.0)),
                        "[{}] Found invalid bridge line across culled curve!",
                        name
                    );
                }

                // Expected lines: (10, 10)->(20, -10), (30, -10)->(40, 10), and (40, 10)->(10, 10)
                reporter_assert!(
                    reporter,
                    line_count == 3,
                    "[{}] Expected 3 valid line segments, got {}",
                    name,
                    line_count
                );
            };

            test_mode(
                &|f: &mut Flatten, p: &Path, pl: &mut Polyline| {
                    f.process_paths(
                        FlattenMode::Scalar,
                        p,
                        &Matrix::new_identity(),
                        100.0,
                        100.0,
                        pl,
                    );
                },
                "Scalar",
            );

            test_mode(
                &|f: &mut Flatten, p: &Path, pl: &mut Polyline| {
                    f.process_paths(
                        FlattenMode::Simd,
                        p,
                        &Matrix::new_identity(),
                        100.0,
                        100.0,
                        pl,
                    );
                },
                "SIMD",
            );
        }
    }
);
