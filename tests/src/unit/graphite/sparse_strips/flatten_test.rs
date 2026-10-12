// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/sparse_strips/FlattenTest.cpp (chrome/m156)

#![cfg(test)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    // `REPORTER_ASSERT(r, a <= b)` is `!(a <= b)` in the macro, as in the C++.
    clippy::neg_cmp_op_on_partial_ord,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Cap, Join};
use skia_rust_core::path::{Iter, Path, Verb};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::SCALAR_NAN;
use skia_rust_core::stroke_rec::{InitStyle, StrokeRec};
use skia_rust_gpu::graphite::sparse_strips::flatten::Flatten;
use skia_rust_gpu::graphite::sparse_strips::polyline::Polyline;
use skia_rust_gpu::graphite::sparse_strips::types::FlattenMode;
use skia_rust_simd::vx::{Float4, Int4, if_then_else, reduce_min, sqrt};

use crate::{Reporter, def_test, reporter_assert};

/// `TrickyCurve`.
// Port of: tests/graphite/sparse_strips/FlattenTest.cpp#L26-L30 (chrome/m156)
struct TrickyCurve {
    points: [Point; 4],
    num_pts: usize,
    scale: f32,
}

// Port of: tests/graphite/sparse_strips/FlattenTest.cpp#L30-L37 (chrome/m156)
fn eval_quad_simd(pts: &[Point], t: Float4) -> (Float4, Float4) {
    let mt = 1.0f32 - t;
    let a = mt * mt;
    let b = 2.0f32 * mt * t;
    let c = t * t;
    let out_x = pts[0].x * a + pts[1].x * b + pts[2].x * c;
    let out_y = pts[0].y * a + pts[1].y * b + pts[2].y * c;
    (out_x, out_y)
}

// Port of: tests/graphite/sparse_strips/FlattenTest.cpp#L39-L49 (chrome/m156)
fn eval_conic_simd(pts: &[Point], w: f32, t: Float4) -> (Float4, Float4) {
    let mt = 1.0f32 - t;
    let a = mt * mt;
    let b = 2.0f32 * w * t * mt;
    let c = t * t;
    let denom = a + b + c;
    let inv_denom = 1.0f32 / denom;
    let out_x = (pts[0].x * a + pts[1].x * b + pts[2].x * c) * inv_denom;
    let out_y = (pts[0].y * a + pts[1].y * b + pts[2].y * c) * inv_denom;
    (out_x, out_y)
}

// Port of: tests/graphite/sparse_strips/FlattenTest.cpp#L51-L61 (chrome/m156)
fn eval_cubic_simd(pts: &[Point], t: Float4) -> (Float4, Float4) {
    let mt = 1.0f32 - t;
    let mt2 = mt * mt;
    let t2 = t * t;
    let a = mt2 * mt;
    let b = 3.0f32 * mt2 * t;
    let c = 3.0f32 * mt * t2;
    let d = t2 * t;
    let out_x = pts[0].x * a + pts[1].x * b + pts[2].x * c + pts[3].x * d;
    let out_y = pts[0].y * a + pts[1].y * b + pts[2].y * c + pts[3].y * d;
    (out_x, out_y)
}

// Port of: tests/graphite/sparse_strips/FlattenTest.cpp#L63-L81 (chrome/m156)
fn dist_sq_point_simd(p: Point, ax: Float4, ay: Float4, bx: Float4, by: Float4) -> Float4 {
    let abx = bx - ax;
    let aby = by - ay;
    let apx = p.x - ax;
    let apy = p.y - ay;
    let len_sq = abx * abx + aby * aby;
    let dot = apx * abx + apy * aby;

    let mut t = if_then_else(len_sq.eq_mask(0.0f32), Float4::splat(0.0), dot / len_sq);
    t = t.pin(Float4::splat(0.0), Float4::splat(1.0));

    let projx = ax + abx * t;
    let projy = ay + aby * t;
    let dx = p.x - projx;
    let dy = p.y - projy;

    dx * dx + dy * dy
}

// Port of: tests/graphite/sparse_strips/FlattenTest.cpp#L83-L102 (chrome/m156)
fn estimate_max_length_simd(pts: &[Point], count: usize) -> f32 {
    if count < 2 {
        return 0.0;
    }
    let mut px = Float4::splat(0.0);
    let mut py = Float4::splat(0.0);
    let mut nx = Float4::splat(0.0);
    let mut ny = Float4::splat(0.0);

    for i in 0..count - 1 {
        px[i] = pts[i].x;
        py[i] = pts[i].y;
        nx[i] = pts[i + 1].x;
        ny[i] = pts[i + 1].y;
    }

    let dx = nx - px;
    let dy = ny - py;
    let mut lens = sqrt(dx * dx + dy * dy);

    let mask: Int4 = Int4::new(0, 1, 2, 3).lt_mask(Int4::splat(count as i32 - 1));
    lens = if_then_else(mask, lens, Float4::splat(0.0));

    lens[0] + lens[1] + lens[2] + lens[3]
}

/// The callback of `for_each_path_segment_SIMD`: `(ax, ay, bx, by, validCount, verbName)`, which
/// returns false to stop the iteration.
type SegmentCallback<'a> = dyn FnMut(Float4, Float4, Float4, Float4, i32, &str) -> bool + 'a;

// Port of: tests/graphite/sparse_strips/FlattenTest.cpp#L104-L195 (chrome/m156)
fn for_each_path_segment_simd(
    path: &Path,
    max_step_size: f32,
    mut callback: impl FnMut(Float4, Float4, Float4, Float4, i32, &str) -> bool,
) {
    let mut iter = Iter::new(path, false);
    let mut pts = [Point::default(); 4];

    let mut last_pt = Point::new(0.0, 0.0);
    let mut contour_start = Point::new(0.0, 0.0);
    let mut has_contour = false;

    // Helper to dispatch a single segment (Line, Close, Move), returns false to early out if the
    // tolerance has been met
    let emit_single_segment =
        |callback: &mut SegmentCallback<'_>, p0: Point, p1: Point, name: &str| -> bool {
            let ax = Float4::splat(p0.x);
            let ay = Float4::splat(p0.y);
            let bx = Float4::splat(p1.x);
            let by = Float4::splat(p1.y);
            callback(ax, ay, bx, by, 1, name)
        };

    loop {
        let verb = iter.next_verb(&mut pts);
        if verb == Verb::Done {
            break;
        }
        match verb {
            Verb::Move => {
                if has_contour
                    && last_pt != contour_start
                    && !emit_single_segment(&mut callback, last_pt, contour_start, "ImplicitClose")
                {
                    return;
                }
                contour_start = pts[0];
                last_pt = pts[0];
                has_contour = true;
                // Emit a 0-length segment so coverage checks evaluate the starting point
                if !emit_single_segment(&mut callback, pts[0], pts[0], "Move") {
                    return;
                }
            }
            Verb::Line => {
                if !emit_single_segment(&mut callback, pts[0], pts[1], "Line") {
                    return;
                }
                last_pt = pts[1];
            }
            Verb::Quad | Verb::Conic | Verb::Cubic => {
                let count_pts: usize = if verb == Verb::Quad || verb == Verb::Conic {
                    3
                } else {
                    4
                };
                let samples = std::cmp::max(
                    1,
                    (estimate_max_length_simd(&pts, count_pts) / max_step_size).ceil() as i32,
                );
                let mut p0 = pts[0];
                let w = if verb == Verb::Conic {
                    iter.conic_weight().unwrap_or(1.0)
                } else {
                    1.0
                };

                let mut i = 0;
                while i < samples {
                    let count = std::cmp::min(4, samples - i);
                    let t = Float4::new(
                        (i + 1) as f32,
                        (i + 2) as f32,
                        (i + 3) as f32,
                        (i + 4) as f32,
                    ) / (samples as f32);

                    let (px, py) = match verb {
                        Verb::Quad => eval_quad_simd(&pts, t),
                        Verb::Conic => eval_conic_simd(&pts, w, t),
                        Verb::Cubic => eval_cubic_simd(&pts, t),
                        _ => unreachable!(),
                    };

                    let mut ax = Float4::splat(0.0);
                    let mut ay = Float4::splat(0.0);
                    ax[0] = p0.x;
                    ax[1] = px[0];
                    ax[2] = px[1];
                    ax[3] = px[2];
                    ay[0] = p0.y;
                    ay[1] = py[0];
                    ay[2] = py[1];
                    ay[3] = py[2];

                    let name = match verb {
                        Verb::Quad => "Quad",
                        Verb::Conic => "Conic",
                        _ => "Cubic",
                    };

                    if !callback(ax, ay, px, py, count, name) {
                        return;
                    }
                    p0 = Point::new(px[count as usize - 1], py[count as usize - 1]);
                    i += 4;
                }
                last_pt = pts[count_pts - 1];
            }
            Verb::Close => {
                if last_pt != contour_start
                    && !emit_single_segment(&mut callback, last_pt, contour_start, "Close")
                {
                    return;
                }
                last_pt = contour_start;
                has_contour = false;
            }
            Verb::Done => {}
        }
    }

    if has_contour && last_pt != contour_start {
        emit_single_segment(&mut callback, last_pt, contour_start, "ImplicitClose");
    }
}

// `FlattenTestRunner<kMode>`'s constants.
// Port of: tests/graphite/sparse_strips/FlattenTest.cpp#L190-L198 (chrome/m156)
const K_QUAD_ERR_TOLERANCE: f32 = Flatten::K_QUAD_ERR_TOLERANCE as f32;
const K_ALLOWED_TOL_SQ: f32 = (Flatten::K_QUAD_TOLERANCE2 + 0.0001f32 as f64) as f32;
const K_MIN_DIST_SQ: f32 = 1e10;
const K_MAX_STEP_SIZE: f32 = if 0.1f32 > K_QUAD_ERR_TOLERANCE * 0.5 {
    0.1
} else {
    K_QUAD_ERR_TOLERANCE * 0.5
};
const K_LINE_SAMPLES: i32 = 100;
const K_LINE_DT: f32 = 1.0 / (K_LINE_SAMPLES as f32);
const K_VIEW_DIM: f32 = 10000.0;

/// `FlattenTestRunner<kMode>::DistSqPointToPath`.
// Port of: tests/graphite/sparse_strips/FlattenTest.cpp#L209-L223 (chrome/m156)
fn dist_sq_point_to_path(pt: Point, path: &Path) -> f32 {
    let mut min_dist_sq = K_MIN_DIST_SQ;
    let process_segment =
        |ax: Float4, ay: Float4, bx: Float4, by: Float4, valid_count: i32, _verb_name: &str| {
            let mut dists = dist_sq_point_simd(pt, ax, ay, bx, by);
            let mask: Int4 = Int4::new(0, 1, 2, 3).lt_mask(Int4::splat(valid_count));
            dists = if_then_else(mask, dists, Float4::splat(K_MIN_DIST_SQ));
            min_dist_sq = f32::min(min_dist_sq, reduce_min(dists));
            // Return false to potentially early out
            min_dist_sq > K_ALLOWED_TOL_SQ
        };
    for_each_path_segment_simd(path, K_MAX_STEP_SIZE, process_segment);
    min_dist_sq
}

/// `FlattenTestRunner<kMode>::CheckFlattenedPath`.
// Port of: tests/graphite/sparse_strips/FlattenTest.cpp#L225-L401 (chrome/m156)
fn check_flattened_path(
    mode: FlattenMode,
    reporter: &mut Reporter,
    original_path: &Path,
    test_name: &str,
) {
    let bounds = original_path.compute_tight_bounds();
    let shift = Matrix::translate((1000.0 - bounds.left, 1000.0 - bounds.top));
    let reference_path = original_path.make_transform(&shift);

    let mut flattener = Flatten::new();
    let mut polyline = Polyline::new();

    flattener.process_paths(
        mode,
        original_path,
        &shift,
        K_VIEW_DIM,
        K_VIEW_DIM,
        &mut polyline,
    );

    if original_path.is_empty() {
        reporter_assert!(
            reporter,
            polyline.is_empty(),
            "[{}] Expected empty output",
            test_name
        );
        return;
    }

    let raw_pts = polyline.points();

    // =========================================================================================
    // Watertightness Check
    // Iterate through the raw flattened points (which are separated by NaNs to denote distinct
    // contours) and assert that the final point of every contour matches its starting point.
    // =========================================================================================
    {
        let mut contour_start = Point::new(0.0, 0.0);
        let mut last_pt = Point::new(0.0, 0.0);
        let mut in_contour = false;
        for raw_pt in raw_pts {
            if raw_pt.x.is_nan() {
                if in_contour {
                    reporter_assert!(
                        reporter,
                        last_pt == contour_start,
                        "[{}] Contour did not close tightly. End: ({:.6}, {:.6}), Start: \
                         ({:.6}, {:.6})",
                        test_name,
                        last_pt.x,
                        last_pt.y,
                        contour_start.x,
                        contour_start.y
                    );
                }
                in_contour = false;
            } else {
                if !in_contour {
                    contour_start = *raw_pt;
                    in_contour = true;
                }
                last_pt = *raw_pt;
            }
        }
        // Catch the final contour if the array doesn't end with a NaN separator. (Should never
        // happen)
        if in_contour {
            reporter_assert!(
                reporter,
                last_pt == contour_start,
                "[{}] Contour did not close tightly. End: ({:.6}, {:.6}), Start: ({:.6}, {:.6})",
                test_name,
                last_pt.x,
                last_pt.y,
                contour_start.x,
                contour_start.y
            );
        }
    }

    // =========================================================================================
    // Polyline-to-Path (Hausdorff Distance Approximation -> True Curve)
    //
    // Ensure that the flattener didn't generate line segments that stray too far from the true
    // path. To avoid numerical solvers, we sample *along the generated line segments* and
    // projecting them back onto the true path to find the shortest distance.
    // =========================================================================================
    for (line, _index) in &polyline {
        let mut i = 0;
        while i < K_LINE_SAMPLES {
            let t03 =
                Float4::new(i as f32, (i + 1) as f32, (i + 2) as f32, (i + 3) as f32) * K_LINE_DT;
            let px03 = line.p0.x * (1.0f32 - t03) + line.p1.x * t03;
            let py03 = line.p0.y * (1.0f32 - t03) + line.p1.y * t03;

            let count = std::cmp::min(4, K_LINE_SAMPLES - i);
            for j in 0..count as usize {
                let sample_pt = Point::new(px03[j], py03[j]);
                let min_dist_sq = dist_sq_point_to_path(sample_pt, &reference_path);
                reporter_assert!(
                    reporter,
                    min_dist_sq <= K_ALLOWED_TOL_SQ,
                    "[{}] Interpolated line point ({:.6}, {:.6}) deviated from true curve by {:.6} \
                     (tol: {:.6})",
                    test_name,
                    sample_pt.x,
                    sample_pt.y,
                    min_dist_sq.sqrt(),
                    K_ALLOWED_TOL_SQ
                );
            }
            i += 4;
        }

        // Check terminal t=1.0 separately
        let pt4 = line.p1;
        let min_dist_sq = dist_sq_point_to_path(pt4, &reference_path);
        reporter_assert!(
            reporter,
            min_dist_sq <= K_ALLOWED_TOL_SQ,
            "[{}] Interpolated line point ({:.6}, {:.6}) deviated from true curve by {:.6} (tol: \
             {:.6})",
            test_name,
            pt4.x,
            pt4.y,
            min_dist_sq.sqrt(),
            K_ALLOWED_TOL_SQ
        );
    }

    // ==============================================================================
    // Path-to-Polyline (Hausdorff Distance True Curve -> Approximation)
    //
    // Ensure that the flattenner produces geometry for the length of the entire original curve,
    // instead of collapsing the curve into a single valid point or drawing an incomplete
    // segment that just happens to lie on the mathematical path.
    // ==============================================================================
    let dist_sq_point_to_polyline = |pt: Point| -> f32 {
        let mut min_dist_sq = K_MIN_DIST_SQ;
        let mut ax = Float4::splat(0.0);
        let mut ay = Float4::splat(0.0);
        let mut bx = Float4::splat(0.0);
        let mut by = Float4::splat(0.0);
        let mut lane = 0usize;

        for (line, _index) in &polyline {
            ax[lane] = line.p0.x;
            ay[lane] = line.p0.y;
            bx[lane] = line.p1.x;
            by[lane] = line.p1.y;

            lane += 1;
            if lane == 4 {
                let dists = dist_sq_point_simd(pt, ax, ay, bx, by);
                min_dist_sq = f32::min(min_dist_sq, reduce_min(dists));
                if min_dist_sq <= K_ALLOWED_TOL_SQ {
                    return min_dist_sq;
                }
                lane = 0;
            }
        }

        if lane > 0 {
            let valid: Int4 = Int4::new(0, 1, 2, 3).lt_mask(Int4::splat(lane as i32));
            let mut dists = dist_sq_point_simd(pt, ax, ay, bx, by);
            dists = if_then_else(valid, dists, Float4::splat(K_MIN_DIST_SQ));
            min_dist_sq = f32::min(min_dist_sq, reduce_min(dists));
            if min_dist_sq <= K_ALLOWED_TOL_SQ {
                return min_dist_sq;
            }
        }

        for raw_pt in raw_pts {
            if !raw_pt.x.is_nan() {
                let d = pt - *raw_pt;
                min_dist_sq = f32::min(min_dist_sq, Point::dot_product(d, d));
                if min_dist_sq <= K_ALLOWED_TOL_SQ {
                    return min_dist_sq;
                }
            }
        }

        min_dist_sq
    };

    let check_path_coverage = |_ax: Float4,
                               _ay: Float4,
                               bx: Float4,
                               by: Float4,
                               valid_count: i32,
                               verb_name: &str|
     -> bool {
        for i in 0..valid_count as usize {
            let pt = Point::new(bx[i], by[i]);
            let min_dist_sq = dist_sq_point_to_polyline(pt);
            reporter_assert!(
                reporter,
                min_dist_sq <= K_ALLOWED_TOL_SQ,
                "[{}] Path {} point ({:.6}, {:.6}) lacks polyline coverage. Deviated by {:.6} \
                 (tol: {:.6})",
                test_name,
                verb_name,
                pt.x,
                pt.y,
                min_dist_sq.sqrt(),
                K_ALLOWED_TOL_SQ
            );
        }
        true // continue checking the rest of the path
    };

    for_each_path_segment_simd(&reference_path, K_MAX_STEP_SIZE, check_path_coverage);
}

/// `FlattenTestRunner<kMode>::TestCulling`.
// Port of: tests/graphite/sparse_strips/FlattenTest.cpp#L403-L463 (chrome/m156)
fn test_culling(mode: FlattenMode, reporter: &mut Reporter) {
    const K_VIEW_W: f32 = 100.0;
    const K_VIEW_H: f32 = 100.0;

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Expect {
        // Removed: Init NaN + Start pt + Cull NaN + Last pt + Start pt (5 points)
        Culled,
        // Left side simplification: Start pt + End pt + Close pt + NaN (4 points)
        Simplified,
        // Intersects viewport: Subdivided curve (> 4 points)
        Subdivided,
    }

    let mut check_path = |path: &Path, expect: Expect, test_name: &str| {
        let mut flattener = Flatten::new();
        let mut polyline = Polyline::new();
        flattener.process_paths(
            mode,
            path,
            &Matrix::new_identity(),
            K_VIEW_W,
            K_VIEW_H,
            &mut polyline,
        );

        let count = polyline.points().len();

        if expect == Expect::Culled {
            reporter_assert!(
                reporter,
                count == 5,
                "[{}] Expected curve to be entirely culled (5 points), got {} points",
                test_name,
                count
            );
        } else if expect == Expect::Simplified {
            reporter_assert!(
                reporter,
                count == 4,
                "[{}] Expected curve to be simplified to 1 line (4 points), got {} points",
                test_name,
                count
            );
        } else {
            reporter_assert!(
                reporter,
                count > 4,
                "[{}] Expected curve to cross viewport and be subdivided (>4 points), got {} \
                 points",
                test_name,
                count
            );
        }
    };

    check_path(
        &PathBuilder::new()
            .move_to((10.0, -20.0))
            .quad_to((50.0, -80.0), (90.0, -20.0))
            .detach(),
        Expect::Culled,
        "Cull_Top",
    );

    check_path(
        &PathBuilder::new()
            .move_to((10.0, 120.0))
            .cubic_to((30.0, 180.0), (70.0, 180.0), (90.0, 120.0))
            .detach(),
        Expect::Culled,
        "Cull_Bottom",
    );

    check_path(
        &PathBuilder::new()
            .move_to((120.0, 10.0))
            .quad_to((180.0, 50.0), (120.0, 90.0))
            .detach(),
        Expect::Culled,
        "Cull_Right",
    );

    check_path(
        &PathBuilder::new()
            .move_to((-50.0, 10.0))
            .cubic_to((-20.0, 30.0), (-20.0, 70.0), (-50.0, 90.0))
            .detach(),
        Expect::Simplified,
        "Simplify_Left",
    );

    check_path(
        &PathBuilder::new()
            .move_to((-20.0, -20.0))
            .quad_to((120.0, -20.0), (120.0, 120.0))
            .detach(),
        Expect::Subdivided,
        "Crosses_Viewport",
    );
}

/// `FlattenTestRunner<kMode>::TestTrickyStrokes`.
// Port of: tests/graphite/sparse_strips/FlattenTest.cpp#L465-L559 (chrome/m156)
fn test_tricky_strokes(mode: FlattenMode, reporter: &mut Reporter) {
    const K_STROKE_WIDTH: f32 = 30.0;
    let p = |x: f32, y: f32| Point::new(x, y);
    let k_tricky_curves = [
        TrickyCurve {
            points: [
                p(122.0, 737.0),
                p(348.0, 553.0),
                p(403.0, 761.0),
                p(400.0, 760.0),
            ],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [
                p(244.0, 520.0),
                p(244.0, 518.0),
                p(1141.0, 634.0),
                p(394.0, 688.0),
            ],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [
                p(550.0, 194.0),
                p(138.0, 130.0),
                p(1035.0, 246.0),
                p(288.0, 300.0),
            ],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [
                p(226.0, 733.0),
                p(556.0, 779.0),
                p(-43.0, 471.0),
                p(348.0, 683.0),
            ],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [
                p(268.0, 204.0),
                p(492.0, 304.0),
                p(352.0, 23.0),
                p(433.0, 412.0),
            ],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [
                p(172.0, 480.0),
                p(396.0, 580.0),
                p(256.0, 299.0),
                p(338.0, 677.0),
            ],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [
                p(731.0, 340.0),
                p(318.0, 252.0),
                p(1026.0, -64.0),
                p(367.0, 265.0),
            ],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [
                p(475.0, 708.0),
                p(62.0, 620.0),
                p(770.0, 304.0),
                p(220.0, 659.0),
            ],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [p(0.0, 0.0), p(128.0, 128.0), p(128.0, 0.0), p(0.0, 128.0)],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [
                p(0.0, 0.01),
                p(128.0, 127.999),
                p(128.0, 0.01),
                p(0.0, 127.99),
            ],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [
                p(0.0, -0.01),
                p(128.0, 128.001),
                p(128.0, -0.01),
                p(0.0, 128.001),
            ],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [p(0.0, 0.0), p(0.0, -10.0), p(0.0, -10.0), p(0.0, 10.0)],
            num_pts: 4,
            scale: 1.098_283,
        },
        TrickyCurve {
            points: [p(10.0, 0.0), p(0.0, 0.0), p(20.0, 0.0), p(10.0, 0.0)],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [p(39.0, -39.0), p(40.0, -40.0), p(40.0, -40.0), p(0.0, 0.0)],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [p(39.0, -39.0), p(40.0, -40.0), p(37.0, -39.0), p(0.0, 0.0)],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [p(40.0, 40.0), p(0.0, 0.0), p(200.0, 200.0), p(0.0, 0.0)],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [p(0.0, 0.0), p(1e-2, 0.0), p(-1e-2, 0.0), p(0.0, 0.0)],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [
                p(400.75, 100.05),
                p(400.75, 100.05),
                p(100.05, 300.95),
                p(100.05, 300.95),
            ],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [p(0.5, 0.0), p(0.0, 0.0), p(20.0, 0.0), p(10.0, 0.0)],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [p(10.0, 0.0), p(0.0, 0.0), p(10.0, 0.0), p(10.0, 0.0)],
            num_pts: 4,
            scale: 1.0,
        },
        TrickyCurve {
            points: [p(1.0, 1.0), p(2.0, 1.0), p(1.0, 1.0), p(1.0, SCALAR_NAN)],
            num_pts: 3,
            scale: 1.0,
        },
        TrickyCurve {
            points: [p(1.0, 1.0), p(100.0, 1.0), p(25.0, 1.0), p(0.3, SCALAR_NAN)],
            num_pts: 3,
            scale: 1.0,
        },
        TrickyCurve {
            points: [p(1.0, 1.0), p(100.0, 1.0), p(25.0, 1.0), p(1.5, SCALAR_NAN)],
            num_pts: 3,
            scale: 1.0,
        },
    ];

    let mut stroke_and_check = |original_path: &Path, name_base: &str, index: usize| {
        let mut miter_style = StrokeRec::new(InitStyle::Hairline);
        miter_style.set_stroke_style(K_STROKE_WIDTH, None);
        miter_style.set_stroke_params(Cap::Butt, Join::Miter, 4.0);

        let mut miter_builder = PathBuilder::new();
        miter_style.apply_to_path(&mut miter_builder, original_path);
        check_flattened_path(
            mode,
            reporter,
            &miter_builder.detach(),
            &format!("{name_base}_{index}_Miter"),
        );

        let mut round_style = StrokeRec::new(InitStyle::Hairline);
        round_style.set_stroke_style(K_STROKE_WIDTH, None);
        round_style.set_stroke_params(Cap::Round, Join::Round, 4.0);

        let mut round_builder = PathBuilder::new();
        round_style.apply_to_path(&mut round_builder, original_path);
        check_flattened_path(
            mode,
            reporter,
            &round_builder.detach(),
            &format!("{name_base}_{index}_Round"),
        );
    };

    for (i, curve) in k_tricky_curves.iter().enumerate() {
        let original_pts = curve.points;
        let num_pts = curve.num_pts;
        let scale = curve.scale;

        let mut pts = [Point::default(); 4];
        pts[..num_pts].copy_from_slice(&original_pts[..num_pts]);
        for pt in pts.iter_mut().take(num_pts) {
            *pt *= scale;
        }

        let mut builder = PathBuilder::new();
        builder.move_to(pts[0]);

        let w = original_pts[3].x;

        if num_pts == 4 {
            builder.cubic_to(pts[1], pts[2], pts[3]);
        } else if w == 1.0 {
            builder.quad_to(pts[1], pts[2]);
        } else {
            builder.conic_to(pts[1], pts[2], w);
        }

        stroke_and_check(&builder.detach(), "TrickyStroke", i);
    }

    let mut large_rad = PathBuilder::new();
    for y in 0..2 {
        let shift = 210.0 * y as f32;
        let dy = 5.0 * y as f32;
        large_rad.move_to((159.429, 149.808 + shift)).cubic_to(
            (232.5, 149.808 + dy + shift),
            (232.5, 149.808 + dy + shift),
            (305.572, 149.808 + shift),
        );
    }

    let mut thick_style = StrokeRec::new(InitStyle::Hairline);
    thick_style.set_stroke_style(200.0, None);
    thick_style.set_stroke_params(Cap::Butt, Join::Miter, 4.0);

    let mut thick_builder = PathBuilder::new();
    let large_rad_path = large_rad.detach();
    thick_style.apply_to_path(&mut thick_builder, &large_rad_path);

    check_flattened_path(
        mode,
        reporter,
        &thick_builder.detach(),
        "TrickyStroke_LargeRadius",
    );
}

/// `FlattenTestRunner<kMode>::RunAll`.
// Port of: tests/graphite/sparse_strips/FlattenTest.cpp#L562-L648 (chrome/m156)
fn run_all(mode: FlattenMode, reporter: &mut Reporter) {
    check_flattened_path(mode, reporter, &Path::default(), "EmptyPath");
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((10.0, 10.0))
            .line_to((50.0, 50.0))
            .line_to((90.0, 10.0))
            .detach(),
        "SimpleLines",
    );
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((10.0, 10.0))
            .quad_to((50.0, 100.0), (90.0, 10.0))
            .detach(),
        "SimpleQuad",
    );
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((10.0, 10.0))
            .cubic_to((30.0, 100.0), (70.0, 100.0), (90.0, 10.0))
            .detach(),
        "SimpleCubic",
    );
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((0.0, 0.0))
            .quad_to((50.0, 100.0), (100.0, 0.0))
            .close()
            .detach(),
        "ClosedQuad",
    );
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((10.0, 10.0))
            .cubic_to((100.0, 10.0), (10.0, 100.0), (100.0, 100.0))
            .detach(),
        "SCurveCubic",
    );
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((10.0, 10.0))
            .quad_to((10.0, 10.0), (10.0, 10.0))
            .cubic_to((10.0, 10.0), (10.0, 10.0), (10.0, 10.0))
            .detach(),
        "DegenerateCurves",
    );
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((10.0, 10.0))
            .cubic_to((40.0, 10.0), (70.0, 10.0), (100.0, 10.0))
            .detach(),
        "CollinearCubic",
    );
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((50.0, 50.0))
            .cubic_to((150.0, -50.0), (150.0, 150.0), (50.0, 50.0))
            .detach(),
        "LoopCubic",
    );
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((10.0, 10.0))
            .line_to((20.0, 20.0))
            .move_to((30.0, 30.0))
            .quad_to((50.0, 80.0), (70.0, 30.0))
            .close()
            .move_to((100.0, 100.0))
            .cubic_to((120.0, 150.0), (180.0, 150.0), (200.0, 100.0))
            .detach(),
        "MultiContour",
    );

    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((10.0, 10.0))
            .conic_to((50.0, 100.0), (90.0, 10.0), 0.5)
            .detach(),
        "SimpleConicWeightHalf",
    );
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((10.0, 10.0))
            .conic_to((50.0, 100.0), (90.0, 10.0), 2.0)
            .detach(),
        "SimpleConicWeightTwo",
    );
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((10.0, 10.0))
            .conic_to((50.0, 100.0), (90.0, 10.0), 1.0)
            .detach(),
        "SimpleConicWeightOne",
    );
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((100.0, 0.0))
            .conic_to((100.0, 100.0), (0.0, 100.0), 2.0f32.sqrt() / 2.0)
            .detach(),
        "QuarterCircleConic",
    );
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((10.0, 10.0))
            .conic_to((50.0, 100.0), (90.0, 10.0), 0.0)
            .detach(),
        "WorstCaseWeightZero",
    );
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((10.0, 10.0))
            .conic_to((50.0, 100.0), (90.0, 10.0), 1e-5)
            .detach(),
        "WorstCaseWeightMicroscopic",
    );
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((10.0, 10.0))
            .conic_to((50.0, 100.0), (90.0, 10.0), 1e5)
            .detach(),
        "WorstCaseWeightMassive",
    );
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((50.0, 50.0))
            .conic_to((50.0, 50.0), (50.0, 50.0), 1.0)
            .detach(),
        "WorstCaseDegenerate",
    );
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((0.0, 0.0))
            .cubic_to((50.0, 0.0), (100.0, 0.0), (100.0, 50.0))
            .detach(),
        "CubicPartialLinear",
    );
    check_flattened_path(
        mode,
        reporter,
        &PathBuilder::new()
            .move_to((10.0, 10.0))
            .line_to((20.0, 10.0))
            .quad_to((20.0, -20.0), (30.0, -10.0))
            .close()
            .detach(),
        "CulledEndQuadClosed",
    );

    test_culling(mode, reporter);
    test_tricky_strokes(mode, reporter);
}

def_test!(SparseStrips_Flatten_Scalar, |reporter| {
    // Port of: tests/graphite/sparse_strips/FlattenTest.cpp#L651-L653 (chrome/m156)
    run_all(FlattenMode::Scalar, reporter);
});

def_test!(SparseStrips_Flatten_SIMD, |reporter| {
    // Port of: tests/graphite/sparse_strips/FlattenTest.cpp#L655-L657 (chrome/m156)
    run_all(FlattenMode::Simd, reporter);
});
