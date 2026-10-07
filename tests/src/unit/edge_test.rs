// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/EdgeTest.cpp (chrome/m156)

#![cfg(test)]

use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::fixed::{fixed_to_float, float_to_fixed};
use skia_rust_core::floating_point::float_round2int;
use skia_rust_core::geometry::{CubicCoeff, QuadCoeff};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_raster::edge::{CubicEdge, Edge, QuadraticEdge, Winding};
use skia_rust_raster::edge_builder::{BasicEdgeBuilder, EdgeBuilder};
use skia_rust_simd::vx::Float2;

// Port of: tests/EdgeTest.cpp#L19-L35 (chrome/m156)
def_test!(
    SkEdge_CWLineWithoutClip_PublicMembersMatchMathematics,
    |r| {
        let mut e = Edge::default();
        e.set_line(Point::new(0.0, 4.0), Point::new(10.0, 20.0));

        reporter_assert!(r, e.winding == Winding::CW);
        // The slope of X with respect to Y of this line segment is (10 - 0) / (20 - 4) or 0.625
        reporter_assert!(r, e.dx_dy == float_to_fixed(0.625));
        // The Y coordinate conceptually starts at 0.5, so make sure the X coordinate matches is
        // 0.5 * 0.625 (the slope)
        reporter_assert!(r, e.x == float_to_fixed(0.3125));
        // Even though these are stored as integers, they conceptually represent the midpoint of
        // the line (4.5 and 19.5).
        reporter_assert!(r, e.first_y == 4);
        reporter_assert!(r, e.last_y == 19);
        // Lines are "approximated" in one shot and have no need for follow-up approximations.
        reporter_assert!(r, !e.has_next_segment());
    }
);

// Port of: tests/EdgeTest.cpp#L37-L53 (chrome/m156)
def_test!(SkEdge_CWLineNullClip_PublicMembersMatchMathematics, |r| {
    let mut e = Edge::default();
    e.set_line_clipped(Point::new(0.0, 4.0), Point::new(10.0, 20.0), None);

    reporter_assert!(r, e.winding == Winding::CW);
    // The slope of X with respect to Y of this line segment is (10 - 0) / (20 - 4) or 0.625
    reporter_assert!(r, e.dx_dy == float_to_fixed(0.625));
    // The Y coordinate conceptually starts at 0.5, so make sure the X coordinate matches is
    // 0.5 * 0.625 (the slope)
    reporter_assert!(r, e.x == float_to_fixed(0.3125));
    // Even though these are stored as integers, they conceptually represent the midpoint of
    // the line (4.5 and 19.5).
    reporter_assert!(r, e.first_y == 4);
    reporter_assert!(r, e.last_y == 19);
    // Lines are "approximated" in one shot and have no need for follow-up approximations.
    reporter_assert!(r, !e.has_next_segment());
});

// Port of: tests/EdgeTest.cpp#L55-L69 (chrome/m156)
def_test!(
    SkEdge_CCWLineWithoutClip_PublicMembersMatchMathematics,
    |r| {
        let mut e = Edge::default();
        e.set_line(Point::new(0.0, -4.0), Point::new(10.0, -20.0));
        reporter_assert!(r, e.winding == Winding::CCW);
        // The slope of X with respect to Y of this line segment is (10 - 0) / (-20 - -4) or -0.625
        reporter_assert!(r, e.dx_dy == float_to_fixed(-0.625));
        // The Y coordinate conceptually starts at 0.5, so make sure the X coordinate matches is
        // 10 + 0.5 * -0.625 (the slope)
        reporter_assert!(r, e.x == float_to_fixed(9.6875));
        // Even though these are stored as integers, they conceptually represent the midpoint of
        // the line (-19.5 and -4.5).
        reporter_assert!(r, e.first_y == -20);
        reporter_assert!(r, e.last_y == -5);
        reporter_assert!(r, !e.has_next_segment());
    }
);

// Port of: tests/EdgeTest.cpp#L71-L84 (chrome/m156)
#[allow(clippy::similar_names)] // names follow the C++
fn quad_slope_at(pts: &[Point; 3], t: f32) -> f32 {
    debug_assert!((0.0..=1.0).contains(&t));
    // A quadratic Bezier curve can be written in polynomial form as
    //   Q(t) = At^2 + Bt + C
    // Where A = p0 - 2p1 + p2, B = 2(p1 - p0), C = p0
    // and p0 is the start point, p2 is the end point and p1 is the control point.
    // With some derivatives and algebra, the slope of X with respect to Y is
    // ((2*x0 - 4*x1 + 2*x2)t + (2*x1 - 2*x0)) / (2*y0 - 4*y1 + 2*y2)*t + (2*y1 - 2*y0)
    let (x0, x1, x2) = (pts[0].x, pts[1].x, pts[2].x);
    let dxdt = (2.0 * x0 - 4.0 * x1 + 2.0 * x2) * t + (2.0 * x1 - 2.0 * x0);
    let (y0, y1, y2) = (pts[0].y, pts[1].y, pts[2].y);
    let dydt = (2.0 * y0 - 4.0 * y1 + 2.0 * y2) * t + (2.0 * y1 - 2.0 * y0);
    dxdt / dydt
}

// Port of: tests/EdgeTest.cpp#L86-L106 (chrome/m156)
#[allow(clippy::similar_names)] // names follow the C++
fn cubic_slope_at(pts: &[Point; 4], t: f32) -> f32 {
    debug_assert!((0.0..=1.0).contains(&t));
    // A cubic Bezier curve can be written in polynomial form as
    //   C(t) = At^3 + Bt^2 + Ct + D
    // Where A = -p0 + 3p1 + -3p2 + p3
    //       B = 3p0 - 6p1 + 3p2
    //       C = -3p0 + 3p1
    //       D = p0
    // and p0 is the start point, p3 is the end point and p1/p2 are the control points.
    // With some derivatives and algebra, the slope of X with respect to Y is
    //  ((-3*x0 + 9*x1 + -9*x2 + 3*x3)*t*t + (6*x0 - 12*x1 + 6*x2)*t + (-3*x0 + 3*x1)) /
    //  ((-3*y0 + 9*y1 + -9*y2 + 3*y3)*t*t + (6*y0 - 12*y1 + 6*y2)*t + (-3*y0 + 3*y1))
    //
    let (x0, x1, x2, x3) = (pts[0].x, pts[1].x, pts[2].x, pts[3].x);
    let dxdt = (-3.0 * x0 + 9.0 * x1 + -9.0 * x2 + 3.0 * x3) * t * t
        + (6.0 * x0 - 12.0 * x1 + 6.0 * x2) * t
        + (-3.0 * x0 + 3.0 * x1);
    let (y0, y1, y2, y3) = (pts[0].y, pts[1].y, pts[2].y, pts[3].y);
    let dydt = (-3.0 * y0 + 9.0 * y1 + -9.0 * y2 + 3.0 * y3) * t * t
        + (6.0 * y0 - 12.0 * y1 + 6.0 * y2) * t
        + (-3.0 * y0 + 3.0 * y1);
    dxdt / dydt
}

// Port of: tests/EdgeTest.cpp#L108-L118 (chrome/m156)
#[allow(clippy::neg_cmp_op_on_partial_ord)] // inside reporter_assert!'s `!(cond)`
fn assert_within_n_percent(r: &mut Reporter, actual: f32, expected: f32, delta: f32) {
    // Make sure we are passing in something like 5% and not 0.05
    debug_assert!((1.0..=25.0).contains(&delta));
    let percent_diff = (actual - expected).abs() / expected;
    reporter_assert!(
        r,
        percent_diff < (delta / 100.0),
        "{} was not within {}% of {}",
        actual,
        delta,
        expected
    );
}

// Port of: tests/EdgeTest.cpp#L120-L126 (chrome/m156)
fn eval_x_at_quad(quad: &QuadCoeff, t: f32) -> f32 {
    quad.eval(Float2::from_list(&[t, t]))[0]
}

fn eval_y_at_quad(quad: &QuadCoeff, t: f32) -> f32 {
    quad.eval(Float2::from_list(&[t, t]))[1]
}

fn eval_x_at_cubic(cubic: &CubicCoeff, t: f32) -> f32 {
    cubic.eval(Float2::from_list(&[t, t]))[0]
}

fn eval_y_at_cubic(cubic: &CubicCoeff, t: f32) -> f32 {
    cubic.eval(Float2::from_list(&[t, t]))[1]
}

// Port of: tests/EdgeTest.cpp#L128-L184 (chrome/m156)
def_test!(
    #[allow(clippy::cast_precision_loss, clippy::float_cmp)] // mirrors the C++ int/float mixing
    SkEdge_Quad_ApproximatedWithMultipleLineSegments,
    |r| {
        let mut e = QuadraticEdge::default();
        let points = [
            Point::new(0.0, 0.0),
            Point::new(2.5, 15.0),
            Point::new(10.0, 20.0),
        ];
        reporter_assert!(r, e.set_quadratic(&points));

        let exact = QuadCoeff::from_points(&points);
        let assert_x_on_curve_between = |r: &mut Reporter, x: i32, t_lower: f32, t_upper: f32| {
            let lower = eval_x_at_quad(&exact, t_lower);
            let upper = eval_x_at_quad(&exact, t_upper);
            reporter_assert!(
                r,
                fixed_to_float(x) >= lower && fixed_to_float(x) < upper,
                "{} was not in [{}, {}]",
                fixed_to_float(x),
                lower,
                upper
            );
        };
        let assert_y_starts_at = |r: &mut Reporter, y: i32, at_t: f32| {
            let start = float_round2int(eval_y_at_quad(&exact, at_t)) as f32;
            reporter_assert!(r, y as f32 == start, "{} was not {}", y, start);
        };
        let assert_y_ends_at = |r: &mut Reporter, y: i32, at_t: f32| {
            let end = float_round2int(eval_y_at_quad(&exact, at_t)) as f32 - 1.0; // this range is exclusive
            reporter_assert!(r, y as f32 == end, "{} was not {}", y, end);
        };

        // SkQuadraticEdge will approximate a quadratic Bezier curve with N line segments.
        // There are some heuristics to determine how many segments to use. For this curve, there are
        // 4 total segments - the current segment followed by 3 more.
        let k_num_segments = 4;
        let k_segment_width = 1.0_f32 / k_num_segments as f32;
        let mut t = 0.0_f32;
        for segments in (0..k_num_segments).rev() {
            // skia-rust: `skiatest::ReporterContext` is not ported; the context is unused.
            reporter_assert!(r, e.winding == Winding::CW);
            reporter_assert!(r, i32::from(e.segments_left()) == segments);

            // The slope for a line segment should be close to the curve at the midpoint between this
            // section of curve. e.g. for the segment [0.0, 0.25], it should be the slope at t=0.125
            let segment1_slope = quad_slope_at(&points, k_segment_width / 2.0 + t);
            assert_within_n_percent(r, fixed_to_float(e.dx_dy), segment1_slope, 1.0);

            // The x value could vary a bit depending on implementation and rounding
            assert_x_on_curve_between(r, e.x, t, t + k_segment_width);

            // The start and stopping y values should match the actual curve.
            assert_y_starts_at(r, e.first_y, t);
            assert_y_ends_at(r, e.last_y, t + k_segment_width);

            if e.has_next_segment() {
                e.next_segment();
                t += k_segment_width;
            }
        }
    }
);

// Port of: tests/EdgeTest.cpp#L186-L211 (chrome/m156)
def_test!(SkEdge_MostlyFlatQuad_SkipsZeroHeightSegments, |r| {
    let mut e = QuadraticEdge::default();
    let points = [
        Point::new(4.0, 19.0),
        Point::new(5.5, 18.2),
        Point::new(10.0, 18.0),
    ];

    reporter_assert!(r, e.set_quadratic(&points));
    reporter_assert!(r, e.winding == Winding::CCW);

    reporter_assert!(r, e.first_y == 18);
    reporter_assert!(r, e.last_y == 18);

    // The x coordinate should be somewhere between B_x(0.25) and B_x(0.75). The exact value is
    // subject to the implementation's choice of dealing with segments that cover 0 height.
    let exact = QuadCoeff::from_points(&points);
    let k_one_quarter_t = exact.eval(Float2::from_list(&[0.25, 0.0]))[0]; // about 4.9375
    let k_three_quarter_t = exact.eval(Float2::from_list(&[0.75, 0.0]))[0]; // about 7.9375
    reporter_assert!(
        r,
        e.x > float_to_fixed(k_one_quarter_t) && e.x < float_to_fixed(k_three_quarter_t)
    );

    // The slope should be negative, but precisely how much again depends the implementation.
    // The exact value doesn't matter much for this segment because it's 1) the last segment
    // and 2) only of height 1.
    reporter_assert!(r, e.dx_dy < 0);

    // This edge has covered the entire scanline, so there shouldn't be any more segments
    reporter_assert!(r, !e.has_next_segment());
});

// Port of: tests/EdgeTest.cpp#L213-L218 (chrome/m156)
def_test!(SkEdge_HorizontalishQuad_ReturnsFalse, |r| {
    let mut e = QuadraticEdge::default();
    let points = [
        Point::new(4.0, 18.0),
        Point::new(5.5, 18.2),
        Point::new(10.0, 18.4),
    ];

    reporter_assert!(r, !e.set_quadratic(&points));
});

// Port of: tests/EdgeTest.cpp#L220-L275 (chrome/m156)
def_test!(
    #[allow(clippy::cast_precision_loss, clippy::float_cmp)] // mirrors the C++ int/float mixing
    SkEdge_Cubic_ApproximatedWithMultipleLineSegments,
    |r| {
        let mut e = CubicEdge::default();
        let points = [
            Point::new(0.0, 0.0),
            Point::new(2.0, 13.0),
            Point::new(8.0, 6.0),
            Point::new(10.0, 20.0),
        ];

        reporter_assert!(r, e.set_cubic(&points));

        let exact = CubicCoeff::new(&points);
        let assert_x_on_curve_between = |r: &mut Reporter, x: i32, t_lower: f32, t_upper: f32| {
            let lower = eval_x_at_cubic(&exact, t_lower);
            let upper = eval_x_at_cubic(&exact, t_upper);
            reporter_assert!(
                r,
                fixed_to_float(x) >= lower && fixed_to_float(x) < upper,
                "{} was not in [{}, {}]",
                fixed_to_float(x),
                lower,
                upper
            );
        };
        let assert_y_starts_at = |r: &mut Reporter, y: i32, at_t: f32| {
            let start = float_round2int(eval_y_at_cubic(&exact, at_t)) as f32;
            reporter_assert!(r, y as f32 == start, "{} was not {}", y, start);
        };
        let assert_y_ends_at = |r: &mut Reporter, y: i32, at_t: f32| {
            let end = float_round2int(eval_y_at_cubic(&exact, at_t)) as f32 - 1.0; // this range is exclusive
            reporter_assert!(r, y as f32 == end, "{} was not {}", y, end);
        };

        // SkCubicEdge will approximate a cubic Bezier curve with N line segments.
        // There are some heuristics to determine how many segments to use. For this curve, there are
        // 8 total segments - the current segment followed by 7 more.
        let k_num_segments = 8;
        let k_segment_width = 1.0_f32 / k_num_segments as f32;
        let mut t = 0.0_f32;
        for segments in (0..k_num_segments).rev() {
            // skia-rust: `skiatest::ReporterContext` is not ported; the context is unused.
            reporter_assert!(r, e.winding == Winding::CW);
            reporter_assert!(r, i32::from(e.segments_left()) == segments);

            // The slope for a line segment should be close to the curve at the midpoint between this
            // section of curve. e.g. for the segment [0.0, 0.25], it should be the slope at t=0.125
            let segment1_slope = cubic_slope_at(&points, k_segment_width / 2.0 + t);
            assert_within_n_percent(r, fixed_to_float(e.dx_dy), segment1_slope, 3.0);

            // The x value could vary a bit depending on implementation and rounding
            assert_x_on_curve_between(r, e.x, t, t + k_segment_width);

            // The start and stopping y values should match the actual curve.
            assert_y_starts_at(r, e.first_y, t);
            assert_y_ends_at(r, e.last_y, t + k_segment_width);

            if e.has_next_segment() {
                e.next_segment();
                t += k_segment_width;
            }
        }
    }
);

// Port of: tests/EdgeTest.cpp#L277-L321 (chrome/m156)
def_test!(SkBasicEdgeBuilder_TrapezoidNoClip_HasTwoEdges, |r| {
    let mut pb = PathBuilder::new();
    let path = pb
        .move_to(Point::new(5.0, 0.0))
        .line_to(Point::new(0.0, 20.0))
        .line_to(Point::new(20.0, 20.0))
        .line_to(Point::new(10.0, 0.0))
        .close()
        .detach();

    let mut eb = BasicEdgeBuilder::new();
    let num_edges = eb.build_edges_path(&path, None);
    reporter_assert!(r, num_edges == 2);

    let edges = eb.edge_list();
    let (mut left, mut right) = (0, 1);

    if edges[left].x > edges[right].x {
        // An implementation could return these edges in either order
        std::mem::swap(&mut left, &mut right);
    }
    let (left, right) = (&edges[left], &edges[right]);

    // left edge should go from (5,0) -> (0,20) (clockwise) which is a X/Y slope of -5/20
    reporter_assert!(r, left.winding == Winding::CW);
    let k_left_slope = -0.25_f32;
    reporter_assert!(r, left.dx_dy == float_to_fixed(k_left_slope));
    // fX will be slightly less than 5 because it's evaluated at y=0.5
    let k_left_x = 5.0_f32 + k_left_slope * 0.5; // 4.875
    reporter_assert!(r, left.x == float_to_fixed(k_left_x));
    reporter_assert!(r, left.first_y == 0);
    reporter_assert!(r, left.last_y == 19); // this represents 19.5 and is inclusive

    // right edge should go from (20,20) -> (10,0) (counter clockwise) which is a X/Y slope of 10/20
    reporter_assert!(r, right.winding == Winding::CCW);
    let k_right_slope = 0.5_f32;
    reporter_assert!(r, right.dx_dy == float_to_fixed(k_right_slope));
    // fX will be slightly more than 10 because it's evaluated at y=0.5
    let k_right_x = 10.0_f32 + k_right_slope * 0.5; // 10.25
    reporter_assert!(r, right.x == float_to_fixed(k_right_x));
    reporter_assert!(r, right.first_y == 0);
    reporter_assert!(r, right.last_y == 19); // this represents 19.5 and is inclusive

    // Lines are "approximated" in one shot and have no need for follow-up approximations.
    reporter_assert!(r, !left.has_next_segment());
    reporter_assert!(r, !right.has_next_segment());
});
