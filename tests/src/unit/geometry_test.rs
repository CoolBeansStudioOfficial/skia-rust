// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/GeometryTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::floating_point::{FLOAT_INFINITY, FLOAT_NAN, is_finite};
use skia_rust_core::geometry::{
    AutoConicToQuads, Conic, CubicType, chop_cubic_at, chop_cubic_at_inflections,
    chop_cubic_at_max_curvature, chop_cubic_at_mid_tangent, chop_cubic_at_t0_t1, chop_cubic_at_ts,
    chop_mono_cubic_at_x, chop_mono_cubic_at_y, chop_quad_at_max_curvature,
    chop_quad_at_mid_tangent, classify_cubic, convert_quad_to_cubic, eval_cubic_at, eval_quad_at,
    eval_quad_at_pos_tangent, eval_quad_tangent_at, find_cubic_cusp,
    measure_non_inflect_cubic_rotation, measure_quad_rotation,
};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::{Point, Vector, point_priv};
use skia_rust_core::random::Random;
use skia_rust_core::scalar::{SCALAR_PI, Scalar, scalar};

use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/GeometryTest.cpp#L29-L31 (chrome/m156)
fn nearly_equal(a: Point, b: Point) -> bool {
    scalar::nearly_equal(a.x, b.x, None) && scalar::nearly_equal(a.y, b.y, None)
}

// Shorthand for `SkPoint{x, y}`.
const fn pt(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

// Port of: tests/GeometryTest.cpp#L33-L130 (chrome/m156)
#[allow(
    clippy::float_cmp,
    clippy::items_after_statements,
    clippy::too_many_lines
)] // constants stay next to the C++ code they mirror; exact float comparisons, as in the C++ test; mirrors the long C++ test
fn test_chop_cubic(reporter: &mut Reporter) {
    /*
       Inspired by this test, which used to assert that the tValues had dups

       <path stroke="#202020" d="M0,0 C0,0 1,1 2190,5130 C2190,5070 2220,5010 2205,4980" />
    */
    let src = [
        pt(2190.0, 5130.0),
        pt(2190.0, 5070.0),
        pt(2220.0, 5010.0),
        pt(2205.0, 4980.0),
    ];
    let mut dst = [Point::default(); 13];
    let mut t_values = [0.0f32; 3];
    // make sure we don't assert internally
    let count = chop_cubic_at_max_curvature(&src, Some(&mut dst), Some(&mut t_values));
    #[allow(clippy::overly_complex_bool_expr)] // `if (false)` in the C++: avoids bit rot
    if false {
        // avoid bit rot, suppress warning
        reporter_assert!(reporter, count != 0);
    }
    let _ = count;
    // Make sure src and dst can be the same pointer.
    {
        let mut pts = [Point::default(); 7];
        for (i, p) in pts.iter_mut().enumerate() {
            #[allow(clippy::cast_precision_loss)] // small integers
            p.set(i as f32, i as f32);
        }
        // skia-rust: not expressible in Rust: `src` and `dst` cannot alias, so the source is
        // copied first (the C++ reads all of `src` before writing `dst`).
        let src_copy = [pts[0], pts[1], pts[2], pts[3]];
        chop_cubic_at(&src_copy, &mut pts, 0.5);
        for (i, p) in pts.iter().enumerate() {
            #[allow(clippy::cast_precision_loss)] // small integers
            let expected = i as f32 * 0.5f32;
            reporter_assert!(reporter, p.x == p.y);
            reporter_assert!(reporter, p.x == expected);
        }
    }

    static CHOP_TS: [f32; 23] = [
        0.0,
        3.0 / 83.0,
        3.0 / 79.0,
        3.0 / 73.0,
        3.0 / 71.0,
        3.0 / 67.0,
        3.0 / 61.0,
        3.0 / 59.0,
        3.0 / 53.0,
        3.0 / 47.0,
        3.0 / 43.0,
        3.0 / 41.0,
        3.0 / 37.0,
        3.0 / 31.0,
        3.0 / 29.0,
        3.0 / 23.0,
        3.0 / 19.0,
        3.0 / 17.0,
        3.0 / 13.0,
        3.0 / 11.0,
        3.0 / 7.0,
        3.0 / 5.0,
        1.0,
    ];
    const ONES: [f32; 5] = [1.0, 1.0, 1.0, 1.0, 1.0];

    // Ensure an odd number of T values so we exercise the single chop code at the end of
    // SkChopCubicAt form multiple T.
    const _: () = assert!(CHOP_TS.len() % 2 == 1);
    const _: () = assert!(ONES.len() % 2 == 1);

    let mut rand = Random::default();
    for _iter_idx in 0..5 {
        let mut pts = [Point::default(); 4];
        for p in &mut pts {
            // Braced initialization evaluates left to right: x first, then y.
            let x = rand.next_f();
            let y = rand.next_f();
            *p = pt(x, y);
        }

        let mut all_chops = [Point::default(); 4 + 23 * 3];
        chop_cubic_at_ts(&pts, Some(&mut all_chops), &CHOP_TS);
        let mut i = 3;
        for chop_t in CHOP_TS {
            // Ensure we chop at approximately the correct points when we chop an entire list.
            let mut expected_pt = Point::default();
            eval_cubic_at(&pts, chop_t, Some(&mut expected_pt), None, None);
            reporter_assert!(
                reporter,
                scalar::nearly_equal(all_chops[i].x, expected_pt.x, None)
            );
            reporter_assert!(
                reporter,
                scalar::nearly_equal(all_chops[i].y, expected_pt.y, None)
            );
            if chop_t == 0.0 {
                reporter_assert!(reporter, all_chops[i] == pts[0]);
            }
            if chop_t == 1.0 {
                reporter_assert!(reporter, all_chops[i] == pts[3]);
            }
            i += 3;

            // Ensure the middle is exactly degenerate when we chop at two equal points.
            let mut local_chops = [Point::default(); 10];
            chop_cubic_at_t0_t1(&pts, &mut local_chops, chop_t, chop_t);
            reporter_assert!(reporter, local_chops[3] == local_chops[4]);
            reporter_assert!(reporter, local_chops[3] == local_chops[5]);
            reporter_assert!(reporter, local_chops[3] == local_chops[6]);
            if chop_t == 0.0 {
                // Also ensure the first curve is exactly p0 when we chop at T=0.
                reporter_assert!(reporter, local_chops[0] == pts[0]);
                reporter_assert!(reporter, local_chops[1] == pts[0]);
                reporter_assert!(reporter, local_chops[2] == pts[0]);
                reporter_assert!(reporter, local_chops[3] == pts[0]);
            }
            if chop_t == 1.0 {
                // Also ensure the last curve is exactly p3 when we chop at T=1.
                reporter_assert!(reporter, local_chops[6] == pts[3]);
                reporter_assert!(reporter, local_chops[7] == pts[3]);
                reporter_assert!(reporter, local_chops[8] == pts[3]);
                reporter_assert!(reporter, local_chops[9] == pts[3]);
            }
        }

        // Now test what happens when SkChopCubicAt does 0/0 and gets NaN values.
        let mut one_chops = [Point::default(); 4 + 5 * 3];
        chop_cubic_at_ts(&pts, Some(&mut one_chops), &ONES);
        reporter_assert!(reporter, one_chops[0] == pts[0]);
        reporter_assert!(reporter, one_chops[1] == pts[1]);
        reporter_assert!(reporter, one_chops[2] == pts[2]);
        for p in &one_chops[3..] {
            reporter_assert!(reporter, *p == pts[3]);
        }
    }
}

// Port of: tests/GeometryTest.cpp#L132-L140 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn check_pairs(
    reporter: &mut Reporter,
    index: i32,
    t: scalar,
    name: &str,
    x0: scalar,
    y0: scalar,
    x1: scalar,
    y1: scalar,
) {
    let eq = scalar::nearly_equal(x0, x1, None) && scalar::nearly_equal(y0, y1, None);
    if !eq {
        eprintln!("{name} [{index} {t}] p0 [{x0:10.8} {y0:10.8}] p1 [{x1:10.8} {y1:10.8}]");
        reporter_assert!(reporter, eq);
    }
}

// Port of: tests/GeometryTest.cpp#L142-L168 (chrome/m156)
fn test_evalquadat(reporter: &mut Reporter) {
    let mut rand = Random::default();
    for i in 0..1000 {
        let mut pts = [Point::default(); 3];
        for p in &mut pts {
            let x = rand.next_s_scalar1() * 100.0;
            let y = rand.next_s_scalar1() * 100.0;
            p.set(x, y);
        }
        let dt = 1.0f32 / 128.0;
        let mut t = dt;
        for _j in 1..128 {
            let mut r0 = Point::default();
            eval_quad_at_pos_tangent(&pts, t, Some(&mut r0), None);
            let r1 = eval_quad_at(&pts, t);
            check_pairs(reporter, i, t, "quad-pos", r0.x, r0.y, r1.x, r1.y);

            let mut v0 = Vector::default();
            eval_quad_at_pos_tangent(&pts, t, None, Some(&mut v0));
            let v1 = eval_quad_tangent_at(&pts, t);
            check_pairs(reporter, i, t, "quad-tan", v0.x, v0.y, v1.x, v1.y);

            t += dt;
        }
    }
}

// Port of: tests/GeometryTest.cpp#L170-L174 (chrome/m156)
fn test_conic_eval_pos(reporter: &mut Reporter, conic: &Conic, t: scalar) {
    let mut p0 = Point::default();
    conic.eval_at_pos_tangent(t, Some(&mut p0), None);
    let p1 = conic.eval_at(t);
    check_pairs(reporter, 0, t, "conic-pos", p0.x, p0.y, p1.x, p1.y);
}

// Port of: tests/GeometryTest.cpp#L176-L180 (chrome/m156)
fn test_conic_eval_tan(reporter: &mut Reporter, conic: &Conic, t: scalar) {
    let mut v0 = Vector::default();
    conic.eval_at_pos_tangent(t, None, Some(&mut v0));
    let v1 = conic.eval_tangent_at(t);
    check_pairs(reporter, 0, t, "conic-tan", v0.x, v0.y, v1.x, v1.y);
}

// Port of: tests/GeometryTest.cpp#L182-L202 (chrome/m156)
fn test_conic(reporter: &mut Reporter) {
    let mut rand = Random::default();
    for _i in 0..1000 {
        let mut pts = [Point::default(); 3];
        for p in &mut pts {
            let x = rand.next_s_scalar1() * 100.0;
            let y = rand.next_s_scalar1() * 100.0;
            p.set(x, y);
        }
        for _k in 0..10 {
            let w = rand.next_u_scalar1() * 2.0;
            let conic = Conic::from_points(&pts, w);

            let dt = 1.0f32 / 128.0;
            let mut t = dt;
            for _j in 1..128 {
                test_conic_eval_pos(reporter, &conic, t);
                test_conic_eval_tan(reporter, &conic, t);
                t += dt;
            }
        }
    }
}

// Port of: tests/GeometryTest.cpp#L204-L223 (chrome/m156)
fn test_quad_tangents(reporter: &mut Reporter) {
    let pts = [
        pt(10.0, 20.0),
        pt(10.0, 20.0),
        pt(20.0, 30.0),
        pt(10.0, 20.0),
        pt(15.0, 25.0),
        pt(20.0, 30.0),
        pt(10.0, 20.0),
        pt(20.0, 30.0),
        pt(20.0, 30.0),
    ];
    let count = pts.len() / 3;
    for index in 0..count {
        let _conic = Conic::from_points(&pts[index * 3..], 0.707);
        let start = eval_quad_tangent_at(&pts[index * 3..], 0.0);
        let mid = eval_quad_tangent_at(&pts[index * 3..], 0.5);
        let end = eval_quad_tangent_at(&pts[index * 3..], 1.0);
        reporter_assert!(reporter, start.x != 0.0 && start.y != 0.0);
        reporter_assert!(reporter, mid.x != 0.0 && mid.y != 0.0);
        reporter_assert!(reporter, end.x != 0.0 && end.y != 0.0);
        reporter_assert!(reporter, start.cross(mid).nearly_zero(None));
        reporter_assert!(reporter, mid.cross(end).nearly_zero(None));
    }
}

// Port of: tests/GeometryTest.cpp#L225-L243 (chrome/m156)
fn test_conic_tangents(reporter: &mut Reporter) {
    let pts = [
        pt(10.0, 20.0),
        pt(10.0, 20.0),
        pt(20.0, 30.0),
        pt(10.0, 20.0),
        pt(15.0, 25.0),
        pt(20.0, 30.0),
        pt(10.0, 20.0),
        pt(20.0, 30.0),
        pt(20.0, 30.0),
    ];
    let count = pts.len() / 3;
    for index in 0..count {
        let conic = Conic::from_points(&pts[index * 3..], 0.707);
        let start = conic.eval_tangent_at(0.0);
        let mid = conic.eval_tangent_at(0.5);
        let end = conic.eval_tangent_at(1.0);
        reporter_assert!(reporter, start.x != 0.0 && start.y != 0.0);
        reporter_assert!(reporter, mid.x != 0.0 && mid.y != 0.0);
        reporter_assert!(reporter, end.x != 0.0 && end.y != 0.0);
        reporter_assert!(reporter, start.cross(mid).nearly_zero(None));
        reporter_assert!(reporter, mid.cross(end).nearly_zero(None));
    }
}

// Port of: tests/GeometryTest.cpp#L245-L252 (chrome/m156)
fn test_this_conic_to_quad(r: &mut Reporter, pts: &[Point], w: scalar) {
    let mut quadder = AutoConicToQuads::new();
    // skia-rust: the points are copied so `count_quads()` can be called while they are in use.
    let qpts = quadder.compute_quads_with_weight(pts, w, 0.25).to_vec();
    let qcount = quadder.count_quads();
    let pcount = qcount * 2 + 1;

    reporter_assert!(r, point_priv::are_finite(&qpts[..pcount]));
}

/**
 *  We need to ensure that when a conic is approximated by quads, that we always return finite
 *  values in the quads.
 *
 *  Inspired by `crbug_627414`
 */
// Port of: tests/GeometryTest.cpp#L254-L278 (chrome/m156)
fn test_conic_to_quads(reporter: &mut Reporter) {
    let triples = [
        pt(0.0, 0.0),
        pt(1.0, 0.0),
        pt(1.0, 1.0),
        pt(0.0, 0.0),
        pt(3.58732e-43, 2.72084),
        pt(3.00392, 3.00392),
        pt(0.0, 0.0),
        pt(100_000.0, 0.0),
        pt(100_000.0, 100_000.0),
        pt(0.0, 0.0),
        pt(1e30, 0.0),
        pt(1e30, 1e30),
    ];
    let n = triples.len();

    for i in (0..n).step_by(3) {
        let pts = &triples[i..];

        let mut w = 1e30f32;
        loop {
            w *= 2.0;
            test_this_conic_to_quad(reporter, pts, w);
            if !is_finite(w) {
                break;
            }
        }
        test_this_conic_to_quad(reporter, pts, FLOAT_NAN);
    }
}

// Port of: tests/GeometryTest.cpp#L280-L300 (chrome/m156)
fn test_cubic_tangents(reporter: &mut Reporter) {
    let pts = [
        pt(10.0, 20.0),
        pt(10.0, 20.0),
        pt(20.0, 30.0),
        pt(30.0, 40.0),
        pt(10.0, 20.0),
        pt(15.0, 25.0),
        pt(20.0, 30.0),
        pt(30.0, 40.0),
        pt(10.0, 20.0),
        pt(20.0, 30.0),
        pt(30.0, 40.0),
        pt(30.0, 40.0),
    ];
    let count = pts.len() / 4;
    for index in 0..count {
        let _conic = Conic::from_points(&pts[index * 3..], 0.707);
        let (mut start, mut mid, mut end) =
            (Vector::default(), Vector::default(), Vector::default());
        eval_cubic_at(&pts[index * 4..], 0.0, None, Some(&mut start), None);
        eval_cubic_at(&pts[index * 4..], 0.5, None, Some(&mut mid), None);
        eval_cubic_at(&pts[index * 4..], 1.0, None, Some(&mut end), None);
        reporter_assert!(reporter, start.x != 0.0 && start.y != 0.0);
        reporter_assert!(reporter, mid.x != 0.0 && mid.y != 0.0);
        reporter_assert!(reporter, end.x != 0.0 && end.y != 0.0);
        reporter_assert!(reporter, start.cross(mid).nearly_zero(None));
        reporter_assert!(reporter, mid.cross(end).nearly_zero(None));
    }
}

// Port of: tests/GeometryTest.cpp#L302-L311 (chrome/m156)
fn check_cubic_type(
    reporter: &mut Reporter,
    bezier_points: &[Point; 4],
    expected_type: CubicType,
    undefined: bool,
) {
    // Classify the cubic even if the results will be undefined: check for crashes and asserts.
    let actual_type = classify_cubic(bezier_points);
    if !undefined {
        reporter_assert!(
            reporter,
            actual_type == expected_type,
            "{} != {}",
            actual_type as i32,
            expected_type as i32
        );
    }
}

// Port of: tests/GeometryTest.cpp#L313-L384 (chrome/m156)
#[allow(clippy::needless_range_loop)] // index loops mirror the C++
fn check_cubic_around_rect(
    _name: &str, // skia-rust: `skiatest::ReporterContext` is not ported; the name is unused
    reporter: &mut Reporter,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    undefined: bool,
) {
    use CubicType::{CuspAtInfinity, LocalCusp, Loop, Serpentine};
    static EXPECTATIONS: [CubicType; 24] = [
        Loop,
        CuspAtInfinity,
        LocalCusp,
        LocalCusp,
        CuspAtInfinity,
        Loop,
        CuspAtInfinity,
        Loop,
        CuspAtInfinity,
        Loop,
        LocalCusp,
        LocalCusp,
        LocalCusp,
        LocalCusp,
        Loop,
        CuspAtInfinity,
        Loop,
        CuspAtInfinity,
        Loop,
        CuspAtInfinity,
        LocalCusp,
        LocalCusp,
        CuspAtInfinity,
        Loop,
    ];
    let points = [pt(x1, y1), pt(x2, y1), pt(x2, y2), pt(x1, y2)];
    let mut bezier = [Point::default(); 4];
    for i in 0..4usize {
        bezier[0] = points[i];
        for j in 0..3usize {
            let jidx = if j < i { j } else { j + 1 };
            bezier[1] = points[jidx];
            let mut k = 0usize;
            let mut kidx = 0usize;
            while k < 2 {
                for _n in 0..2 {
                    kidx = if kidx == i || kidx == jidx {
                        kidx + 1
                    } else {
                        kidx
                    };
                }
                bezier[2] = points[kidx];
                for l in 0..4usize {
                    if l != i && l != jidx && l != kidx {
                        bezier[3] = points[l];
                        break;
                    }
                }
                check_cubic_type(
                    reporter,
                    &bezier,
                    EXPECTATIONS[i * 6 + j * 2 + k],
                    undefined,
                );
                k += 1;
                kidx += 1;
            }
        }
    }
    for i in 0..4usize {
        bezier[0] = points[i];
        for j in 0..3usize {
            let jidx = if j < i { j } else { j + 1 };
            bezier[1] = points[jidx];
            bezier[2] = points[jidx];
            let mut k = 0usize;
            let mut kidx = 0usize;
            while k < 2 {
                for _n in 0..2 {
                    kidx = if kidx == i || kidx == jidx {
                        kidx + 1
                    } else {
                        kidx
                    };
                }
                bezier[3] = points[kidx];
                check_cubic_type(reporter, &bezier, Serpentine, undefined);
                k += 1;
                kidx += 1;
            }
        }
    }
}

// Port of: tests/GeometryTest.cpp#L386-L391 (chrome/m156)
static K_SERPENTINES: [[Point; 4]; 4] = [
    [
        pt(149.325, 107.705),
        pt(149.325, 103.783),
        pt(151.638, 100.127),
        pt(156.263, 96.736),
    ],
    [
        pt(225.694, 223.15),
        pt(209.831, 224.837),
        pt(195.994, 230.237),
        pt(184.181, 239.35),
    ],
    [
        pt(4.873, 5.581),
        pt(5.083, 5.2783),
        pt(5.182, 4.8593),
        pt(5.177, 4.3242),
    ],
    [
        pt(285.625, 499.687),
        pt(411.625, 808.188),
        pt(1064.62, 135.688),
        pt(1042.63, 585.187),
    ],
];

// Port of: tests/GeometryTest.cpp#L393-L398 (chrome/m156)
static K_LOOPS: [[Point; 4]; 4] = [
    [
        pt(635.625, 614.687),
        pt(171.625, 236.188),
        pt(1064.62, 135.688),
        pt(516.625, 570.187),
    ],
    [
        pt(653.050, 725.049),
        pt(663.000, 176.000),
        pt(1189.000, 508.000),
        pt(288.050, 564.950),
    ],
    [
        pt(631.050, 478.049),
        pt(730.000, 302.000),
        pt(870.000, 350.000),
        pt(905.050, 528.950),
    ],
    [
        pt(631.050, 478.0499),
        pt(221.000, 230.000),
        pt(1265.000, 451.000),
        pt(905.050, 528.950),
    ],
];

// Port of: tests/GeometryTest.cpp#L400-L407 (chrome/m156)
static K_LINEAR_CUBICS: [[Point; 4]; 6] = [
    [pt(0.0, 0.0), pt(0.0, 1.0), pt(0.0, 2.0), pt(0.0, 3.0)], // 0-degree flat line.
    [pt(0.0, 0.0), pt(1.0, 0.0), pt(1.0, 0.0), pt(0.0, 0.0)], // 180-degree flat line
    [pt(0.0, 1.0), pt(0.0, 0.0), pt(0.0, 2.0), pt(0.0, 3.0)], // 180-degree flat line
    [pt(0.0, 1.0), pt(0.0, 0.0), pt(0.0, 3.0), pt(0.0, 2.0)], // 360-degree flat line
    [pt(0.0, 0.0), pt(2.0, 0.0), pt(1.0, 0.0), pt(64.0, 0.0)], // 360-degree flat line
    [pt(1.0, 0.0), pt(0.0, 0.0), pt(3.0, 0.0), pt(-64.0, 0.0)], // 360-degree flat line
];

// Port of: tests/GeometryTest.cpp#L409-L447 (chrome/m156)
fn test_classify_cubic(reporter: &mut Reporter) {
    for serp in &K_SERPENTINES {
        check_cubic_type(reporter, serp, CubicType::Serpentine, false);
    }
    for loop_ in &K_LOOPS {
        check_cubic_type(reporter, loop_, CubicType::Loop, false);
    }
    for loop_ in &K_LINEAR_CUBICS {
        check_cubic_type(reporter, loop_, CubicType::LineOrPoint, false);
    }
    check_cubic_around_rect("small box", reporter, 0.0, 0.0, 1.0, 1.0, false);
    check_cubic_around_rect(
        "biggest box",
        reporter,
        -f32::MAX,
        -f32::MAX,
        f32::MAX,
        f32::MAX,
        false,
    );
    check_cubic_around_rect(
        "large quadrant",
        reporter,
        1.0,
        1.0,
        f32::MIN_POSITIVE,
        f32::MAX,
        false,
    );
    check_cubic_around_rect(
        "smallest box",
        reporter,
        -f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
        f32::MIN_POSITIVE,
        f32::MIN_POSITIVE,
        false,
    );
    check_cubic_around_rect(
        "slightly negative box",
        reporter,
        1.0,
        -f32::MIN_POSITIVE,
        -1.0,
        -1.0,
        false,
    );
    check_cubic_around_rect(
        "infinite box",
        reporter,
        -FLOAT_INFINITY,
        -FLOAT_INFINITY,
        FLOAT_INFINITY,
        FLOAT_INFINITY,
        true,
    );
    check_cubic_around_rect(
        "one sided infinite box",
        reporter,
        0.0,
        0.0,
        1.0,
        FLOAT_INFINITY,
        true,
    );
    check_cubic_around_rect(
        "nan box", reporter, -FLOAT_NAN, -FLOAT_NAN, FLOAT_NAN, FLOAT_NAN, true,
    );
    check_cubic_around_rect("partial nan box", reporter, 0.0, 0.0, 1.0, FLOAT_NAN, true);
}

// Port of: tests/GeometryTest.cpp#L449-L454 (chrome/m156)
static K_CUSPS: [[Point; 4]; 4] = [
    [pt(0.0, 0.0), pt(1.0, 1.0), pt(1.0, 0.0), pt(0.0, 1.0)],
    [pt(0.0, 0.0), pt(1.0, 1.0), pt(0.0, 1.0), pt(1.0, 0.0)],
    [pt(0.0, 1.0), pt(1.0, 0.0), pt(0.0, 0.0), pt(1.0, 1.0)],
    [pt(0.0, 1.0), pt(1.0, 0.0), pt(1.0, 1.0), pt(0.0, 0.0)],
];

// Port of: tests/GeometryTest.cpp#L456-L470 (chrome/m156)
#[allow(clippy::neg_cmp_op_on_partial_ord)] // inside reporter_assert!'s `!(cond)`
fn test_cubic_cusps(reporter: &mut Reporter) {
    let no_cusps = [
        [pt(0.0, 0.0), pt(1.0, 1.0), pt(2.0, 2.0), pt(3.0, 3.0)],
        [pt(0.0, 0.0), pt(1.0, 0.0), pt(1.0, 1.0), pt(0.0, 1.0)],
        [pt(0.0, 0.0), pt(1.0, 0.0), pt(2.0, 1.0), pt(2.0, 2.0)],
        [pt(0.0, 0.0), pt(1.0, 0.0), pt(1.0, 1.0), pt(2.0, 1.0)],
    ];
    for no_cusp in no_cusps {
        reporter_assert!(reporter, find_cubic_cusp(&no_cusp) < 0.0);
    }
    for cusp in K_CUSPS {
        reporter_assert!(reporter, find_cubic_cusp(&cusp) > 0.0);
    }
}

// Port of: tests/GeometryTest.cpp#L467-L471 (chrome/m156)
fn k_skew_matrices() -> [Matrix; 3] {
    [
        Matrix::new_all(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0),
        Matrix::new_all(1.0, -1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0),
        Matrix::new_all(0.889, 0.553, 0.0, -0.443, 0.123, 0.0, 0.0, 0.0, 1.0),
    ]
}

// Port of: tests/GeometryTest.cpp#L478-L493 (chrome/m156)
fn test_chop_quad_at_midtangent(reporter: &mut Reporter, pts: &[Point]) {
    const K_TOLERANCE: f32 = 1e-3;
    for m in &k_skew_matrices() {
        let mut mapped = [Point::default(); 3];
        m.map_points(&mut mapped, &pts[..3]);
        let full_rotation = measure_quad_rotation(pts);
        let mut chopped = [Point::default(); 5];
        chop_quad_at_mid_tangent(pts, &mut chopped);
        let left_rotation = measure_quad_rotation(&chopped);
        let right_rotation = measure_quad_rotation(&chopped[2..]);
        reporter_assert!(
            reporter,
            scalar::nearly_equal(left_rotation, full_rotation / 2.0, K_TOLERANCE)
        );
        reporter_assert!(
            reporter,
            scalar::nearly_equal(right_rotation, full_rotation / 2.0, K_TOLERANCE)
        );
    }
}

// Port of: tests/GeometryTest.cpp#L495-L539 (chrome/m156)
#[allow(clippy::neg_cmp_op_on_partial_ord)] // inside reporter_assert!'s `!(cond)`
fn test_chop_cubic_at_midtangent(reporter: &mut Reporter, pts: &[Point], cubic_type: CubicType) {
    const K_TOLERANCE: f32 = 1e-3;
    let skew_matrices = k_skew_matrices();
    let mut n = skew_matrices.len();
    if cubic_type == CubicType::LocalCusp || cubic_type == CubicType::LineOrPoint {
        // FP precision isn't always enough to get the exact correct T value of the mid-tangent on
        // cusps and lines. Only test the identity matrix and the matrix with all 1's.
        n = 2;
    }
    for matrix in &skew_matrices[..n] {
        let mut mapped = [Point::default(); 4];
        matrix.map_points(&mut mapped, &pts[..4]);
        let full_rotation = measure_non_inflect_cubic_rotation(&mapped);
        let mut chopped = [Point::default(); 7];
        chop_cubic_at_mid_tangent(&mapped, &mut chopped);
        let left_rotation = measure_non_inflect_cubic_rotation(&chopped);
        let right_rotation = measure_non_inflect_cubic_rotation(&chopped[3..]);
        if cubic_type == CubicType::LineOrPoint
            && (scalar::nearly_equal(full_rotation, 2.0 * SCALAR_PI, K_TOLERANCE)
                || scalar::nearly_equal(full_rotation, 0.0, K_TOLERANCE))
        {
            // 0- and 360-degree flat lines don't have single points of midtangent.
            // (tangent == midtangent at every point on these curves except the cusp points.)
            // Instead verify the promise from SkChopCubicAtMidTangent that neither side will rotate
            // more than 180 degrees.
            reporter_assert!(reporter, left_rotation.abs() - K_TOLERANCE <= SCALAR_PI);
            reporter_assert!(reporter, right_rotation.abs() - K_TOLERANCE <= SCALAR_PI);
            continue;
        }
        let mut expected_chopped_rotation = full_rotation / 2.0;
        if cubic_type == CubicType::LocalCusp
            || (cubic_type == CubicType::LineOrPoint
                && scalar::nearly_equal(full_rotation, SCALAR_PI, K_TOLERANCE))
        {
            // If we chop a cubic at a cusp, we lose 180 degrees of rotation.
            expected_chopped_rotation = (full_rotation - SCALAR_PI) / 2.0;
        }
        reporter_assert!(
            reporter,
            scalar::nearly_equal(left_rotation, expected_chopped_rotation, K_TOLERANCE)
        );
        reporter_assert!(
            reporter,
            scalar::nearly_equal(right_rotation, expected_chopped_rotation, K_TOLERANCE)
        );
    }
}

// Port of: tests/GeometryTest.cpp#L541-L547 (chrome/m156)
static K_QUADS: [[Point; 3]; 5] = [
    [pt(10.0, 20.0), pt(15.0, 35.0), pt(30.0, 40.0)],
    [
        pt(176.324, 392.705),
        pt(719.325, 205.782),
        pt(297.263, 347.735),
    ],
    [
        pt(652.050, 602.049),
        pt(481.000, 533.000),
        pt(288.050, 564.950),
    ],
    [
        pt(460.625, 557.187),
        pt(707.121, 209.688),
        pt(779.628, 577.687),
    ],
    [
        pt(359.050, 578.049),
        pt(759.000, 274.000),
        pt(288.050, 564.950),
    ],
];

// Port of: tests/GeometryTest.cpp#L549-L551 (chrome/m156)
fn lerp(a: Point, b: Point, t: f32) -> Point {
    a * (1.0 - t) + b * t
}

// Port of: tests/GeometryTest.cpp#L553-L606 (chrome/m156)
#[allow(clippy::items_after_statements)] // constants stay next to the C++ code they mirror
fn test_measure_rotation(reporter: &mut Reporter) {
    static K_FLAT_CUBIC: [Point; 4] = [pt(0.0, 0.0), pt(0.0, 1.0), pt(0.0, 2.0), pt(0.0, 3.0)];
    reporter_assert!(
        reporter,
        measure_non_inflect_cubic_rotation(&K_FLAT_CUBIC).nearly_zero(None)
    );

    static K_FLAT_CUBIC180_1: [Point; 4] = [pt(0.0, 0.0), pt(1.0, 0.0), pt(3.0, 0.0), pt(2.0, 0.0)];
    reporter_assert!(
        reporter,
        scalar::nearly_equal(
            measure_non_inflect_cubic_rotation(&K_FLAT_CUBIC180_1),
            SCALAR_PI,
            None
        )
    );

    static K_FLAT_CUBIC180_2: [Point; 4] = [pt(0.0, 1.0), pt(0.0, 0.0), pt(0.0, 2.0), pt(0.0, 3.0)];
    reporter_assert!(
        reporter,
        scalar::nearly_equal(
            measure_non_inflect_cubic_rotation(&K_FLAT_CUBIC180_2),
            SCALAR_PI,
            None
        )
    );

    static K_FLAT_CUBIC360: [Point; 4] = [pt(0.0, 1.0), pt(0.0, 0.0), pt(0.0, 3.0), pt(0.0, 2.0)];
    reporter_assert!(
        reporter,
        scalar::nearly_equal(
            measure_non_inflect_cubic_rotation(&K_FLAT_CUBIC360),
            2.0 * SCALAR_PI,
            None
        )
    );

    static K_SQUARE180: [Point; 4] = [pt(0.0, 0.0), pt(0.0, 1.0), pt(1.0, 1.0), pt(1.0, 0.0)];
    reporter_assert!(
        reporter,
        scalar::nearly_equal(
            measure_non_inflect_cubic_rotation(&K_SQUARE180),
            SCALAR_PI,
            None
        )
    );

    let check_quad_rotation = |reporter: &mut Reporter, pts: &[Point], expected_rotation: f32| {
        let r = measure_quad_rotation(pts);
        reporter_assert!(reporter, scalar::nearly_equal(r, expected_rotation, None));

        let cubic1 = [pts[0], pts[0], pts[1], pts[2]];
        reporter_assert!(
            reporter,
            scalar::nearly_equal(
                measure_non_inflect_cubic_rotation(&cubic1),
                expected_rotation,
                None
            )
        );

        let cubic2 = [pts[0], pts[1], pts[1], pts[2]];
        reporter_assert!(
            reporter,
            scalar::nearly_equal(
                measure_non_inflect_cubic_rotation(&cubic2),
                expected_rotation,
                None
            )
        );

        let cubic3 = [pts[0], pts[1], pts[2], pts[2]];
        reporter_assert!(
            reporter,
            scalar::nearly_equal(
                measure_non_inflect_cubic_rotation(&cubic3),
                expected_rotation,
                None
            )
        );
    };

    static K_FLAT_QUAD: [Point; 4] = [pt(0.0, 0.0), pt(0.0, 1.0), pt(0.0, 2.0), pt(0.0, 0.0)];
    check_quad_rotation(reporter, &K_FLAT_QUAD, 0.0);

    static K_FLAT_QUAD180_1: [Point; 4] = [pt(1.0, 0.0), pt(0.0, 0.0), pt(2.0, 0.0), pt(0.0, 0.0)];
    check_quad_rotation(reporter, &K_FLAT_QUAD180_1, SCALAR_PI);

    static K_FLAT_QUAD180_2: [Point; 4] = [pt(0.0, 0.0), pt(0.0, 2.0), pt(0.0, 1.0), pt(0.0, 0.0)];
    check_quad_rotation(reporter, &K_FLAT_QUAD180_2, SCALAR_PI);

    let k_tri120: [Point; 3] = [pt(0.0, 0.0), pt(0.5, 3.0f32.sqrt() / 2.0), pt(1.0, 0.0)];
    check_quad_rotation(reporter, &k_tri120, 2.0 * SCALAR_PI / 3.0);
}

// Port of: tests/GeometryTest.cpp#L608-L651 (chrome/m156)
#[allow(clippy::items_after_statements)] // constants stay next to the C++ code they mirror
fn test_chop_at_midtangent(reporter: &mut Reporter) {
    let mut chops = [Point::default(); 10];
    for serp in &K_SERPENTINES {
        reporter_assert!(reporter, classify_cubic(serp) == CubicType::Serpentine);
        let n = chop_cubic_at_inflections(serp, Some(&mut chops));
        for i in 0..n {
            test_chop_cubic_at_midtangent(reporter, &chops[i * 3..], CubicType::Serpentine);
        }
    }
    for loop_ in &K_LOOPS {
        reporter_assert!(reporter, classify_cubic(loop_) == CubicType::Loop);
        test_chop_cubic_at_midtangent(reporter, loop_, CubicType::Loop);
    }
    for line in &K_LINEAR_CUBICS {
        reporter_assert!(reporter, classify_cubic(line) == CubicType::LineOrPoint);
        test_chop_cubic_at_midtangent(reporter, line, CubicType::LineOrPoint);
    }
    for cusp in &K_CUSPS {
        reporter_assert!(reporter, classify_cubic(cusp) == CubicType::LocalCusp);
        test_chop_cubic_at_midtangent(reporter, cusp, CubicType::LocalCusp);
    }
    for quad in &K_QUADS {
        test_chop_quad_at_midtangent(reporter, quad);
        let as_cubic = [
            quad[0],
            lerp(quad[0], quad[1], 2.0 / 3.0),
            lerp(quad[1], quad[2], 1.0 / 3.0),
            quad[2],
        ];
        test_chop_cubic_at_midtangent(reporter, &as_cubic, CubicType::Quadratic);
    }

    static K_EXACT_QUAD: [Point; 4] = [pt(0.0, 0.0), pt(6.0, 2.0), pt(10.0, 2.0), pt(12.0, 0.0)];
    reporter_assert!(
        reporter,
        classify_cubic(&K_EXACT_QUAD) == CubicType::Quadratic
    );
    test_chop_cubic_at_midtangent(reporter, &K_EXACT_QUAD, CubicType::Quadratic);

    static K_EXACT_CUSP_AT_INF: [Point; 4] =
        [pt(0.0, 0.0), pt(1.0, 0.0), pt(0.0, 1.0), pt(1.0, 1.0)];
    reporter_assert!(
        reporter,
        classify_cubic(&K_EXACT_CUSP_AT_INF) == CubicType::CuspAtInfinity
    );
    let n = chop_cubic_at_inflections(&K_EXACT_CUSP_AT_INF, Some(&mut chops));
    for i in 0..n {
        test_chop_cubic_at_midtangent(reporter, &chops[i * 3..], CubicType::CuspAtInfinity);
    }
}

// Port of: tests/GeometryTest.cpp#L653-L698 (chrome/m156)
def_test!(Geometry, |reporter| {
    let mut pts = [Point::default(); 5];

    pts[0].set(0.0, 0.0);
    pts[1].set(100.0, 50.0);
    pts[2].set(0.0, 100.0);

    // skia-rust: not expressible in Rust: `src` and `dst` cannot alias, so the source is copied
    // first. (The C++ comment: "Ensure src and dst can be the same pointer.")
    let src = [pts[0], pts[1], pts[2]];
    let mut count = chop_quad_at_max_curvature(&src, &mut pts);
    reporter_assert!(reporter, count == 1 || count == 2);

    // This previously crashed because the computed t of max curvature is NaN and SkChopQuadAt
    // asserts that the passed t is in 0..1. Passes by not asserting.
    pts[0].set(15.1213, 7.77647);
    pts[1].set(6.2168e+19, 1.51338e+20);
    pts[2].set(1.4579e+19, 1.55558e+21);
    let src = [pts[0], pts[1], pts[2]];
    count = chop_quad_at_max_curvature(&src, &mut pts);
    let _ = count;

    pts[0].set(0.0, 0.0);
    pts[1].set(3.0, 0.0);
    pts[2].set(3.0, 3.0);
    let src = [pts[0], pts[1], pts[2]];
    convert_quad_to_cubic(&src, &mut pts);
    let cubic = [pt(0.0, 0.0), pt(2.0, 0.0), pt(3.0, 1.0), pt(3.0, 3.0)];
    for i in 0..4 {
        reporter_assert!(reporter, nearly_equal(cubic[i], pts[i]));
    }

    test_chop_cubic(reporter);
    test_evalquadat(reporter);
    test_conic(reporter);
    test_cubic_tangents(reporter);
    test_quad_tangents(reporter);
    test_conic_tangents(reporter);
    test_conic_to_quads(reporter);
    test_classify_cubic(reporter);
    test_cubic_cusps(reporter);
    test_measure_rotation(reporter);
    test_chop_at_midtangent(reporter);
});

// Port of: tests/GeometryTest.cpp#L700-L718 (chrome/m156)
fn test_chop_mono_cubic_at_y(
    reporter: &mut Reporter,
    _name: &str, // skia-rust: `skiatest::ReporterContext` is not ported; the name is unused
    curve_inputs: &[Point],
    y_to_chop_at: scalar,
    expected_outputs: &[Point],
) {
    reporter_assert!(
        reporter,
        scalar::nearly_equal(expected_outputs[3].y, y_to_chop_at, None),
        "Invalid test case. 4th point's Y should be {y_to_chop_at}"
    );

    let mut outputs = [Point::default(); 7];
    // Make sure it actually chopped
    reporter_assert!(
        reporter,
        chop_mono_cubic_at_y(curve_inputs, y_to_chop_at, &mut outputs)
    );

    for i in 0..7 {
        reporter_assert!(
            reporter,
            nearly_equal(expected_outputs[i], outputs[i]),
            "({}, {}) != ({}, {}) at index {}",
            expected_outputs[i].x,
            expected_outputs[i].y,
            outputs[i].x,
            outputs[i].y,
            i
        );
    }
}

// Port of: tests/GeometryTest.cpp#L720-L850 (chrome/m156)
def_test!(
    #[allow(clippy::excessive_precision)] // literals copied verbatim from the C++ test
    #[allow(clippy::unreadable_literal)] // literals copied verbatim from the C++ test
    GeometryChopMonoCubicAtY_Successful,
    |reporter| {
        // These cubics are all arbitrary, picked using Desmos for something that looked "nice".

        test_chop_mono_cubic_at_y(
            reporter,
            "straight, positive slope @ 2.5",
            &[pt(0.0, 0.0), pt(0.0, 0.0), pt(10.0, 10.0), pt(10.0, 10.0)],
            2.5,
            &[
                pt(0.000000, 0.000000),
                pt(0.000000, 0.000000),
                pt(1.065055, 1.065055),
                pt(2.500000, 2.500000),
                pt(5.461981, 5.461981),
                pt(10.000000, 10.000000),
                pt(10.000000, 10.000000),
            ],
        );
        test_chop_mono_cubic_at_y(
            reporter,
            "straight, positive slope @ 5.0",
            &[pt(0.0, 0.0), pt(0.0, 0.0), pt(10.0, 10.0), pt(10.0, 10.0)],
            5.0,
            &[
                pt(0.000000, 0.000000),
                pt(0.000000, 0.000000),
                pt(2.500000, 2.500000),
                pt(5.000000, 5.000000),
                pt(7.500000, 7.500000),
                pt(10.000000, 10.000000),
                pt(10.000000, 10.000000),
            ],
        );
        test_chop_mono_cubic_at_y(
            reporter,
            "straight, positive slope @ 9.0",
            &[pt(0.0, 0.0), pt(0.0, 0.0), pt(10.0, 10.0), pt(10.0, 10.0)],
            9.0,
            &[
                pt(0.000000, 0.000000),
                pt(0.000000, 0.000000),
                pt(6.467375, 6.467375),
                pt(9.000000, 9.000000),
                pt(9.616623, 9.616623),
                pt(10.000000, 10.000000),
                pt(10.000000, 10.000000),
            ],
        );
        test_chop_mono_cubic_at_y(
            reporter,
            "straight, positive slope @ 10.0",
            &[pt(0.0, 0.0), pt(0.0, 0.0), pt(10.0, 10.0), pt(10.0, 10.0)],
            10.0,
            &[
                pt(0.000000, 0.000000),
                pt(0.000000, 0.000000),
                pt(10.000000, 10.000000),
                pt(10.000000, 10.000000),
                pt(10.000000, 10.000000),
                pt(10.000000, 10.000000),
                pt(10.000000, 10.000000),
            ],
        );
        test_chop_mono_cubic_at_y(
            reporter,
            "curve, positive slope @ 2.0",
            &[pt(1.0, 1.0), pt(5.0, 2.0), pt(7.0, 4.0), pt(8.0, 7.0)],
            2.0,
            &[
                pt(1.000000, 1.000000),
                pt(2.055050, 1.263763),
                pt(2.970959, 1.597096),
                pt(3.766077, 2.000000),
                pt(5.985480, 3.124621),
                pt(7.263762, 4.791288),
                pt(8.000000, 7.000000),
            ],
        );
        test_chop_mono_cubic_at_y(
            reporter,
            "curve, positive slope @ 5.0",
            &[pt(1.0, 1.0), pt(5.0, 2.0), pt(7.0, 4.0), pt(8.0, 7.0)],
            5.0,
            &[
                pt(1.000000, 1.000000),
                pt(4.033223, 1.758306),
                pt(5.916391, 3.091639),
                pt(7.085550, 5.000000),
                pt(7.458195, 5.608251),
                pt(7.758306, 6.274917),
                pt(8.000000, 7.000000),
            ],
        );
        test_chop_mono_cubic_at_y(
            reporter,
            "curve, negative slope @ 5.0",
            &[pt(2.0, 7.0), pt(3.0, 2.0), pt(6.0, 3.0), pt(11.0, 2.0)],
            5.0,
            &[
                pt(2.000000, 7.000000),
                pt(2.162856, 6.185719),
                pt(2.378757, 5.530570),
                pt(2.647702, 5.000000),
                pt(4.030182, 2.272668),
                pt(6.814281, 2.837144),
                pt(11.000000, 2.000000),
            ],
        );
        test_chop_mono_cubic_at_y(
            reporter,
            "curve, negative slope @ 3.0",
            &[pt(2.0, 7.0), pt(3.0, 2.0), pt(6.0, 3.0), pt(11.0, 2.0)],
            3.0,
            &[
                pt(2.000000, 7.000000),
                pt(2.500000, 4.500000),
                pt(3.500000, 3.500000),
                pt(5.000000, 3.000000),
                pt(6.500000, 2.500000),
                pt(8.500000, 2.500000),
                pt(11.000000, 2.000000),
            ],
        );
        test_chop_mono_cubic_at_y(
            reporter,
            "curve, negative slope @ 2.5",
            &[pt(2.0, 7.0), pt(3.0, 2.0), pt(6.0, 3.0), pt(11.0, 2.0)],
            2.5,
            &[
                pt(2.000000, 7.000000),
                pt(2.750000, 3.250000),
                pt(4.625000, 2.875000),
                pt(7.625000, 2.500000),
                pt(8.625000, 2.375000),
                pt(9.750000, 2.250000),
                pt(11.000000, 2.000000),
            ],
        );
        test_chop_mono_cubic_at_y(
            reporter,
            "inverted curve, negative slope @ 5.0",
            &[pt(11.0, 2.0), pt(6.0, 3.0), pt(3.0, 2.0), pt(2.0, 7.0)],
            5.0,
            &[
                pt(11.000000, 2.000000),
                pt(6.814281, 2.837144),
                pt(4.030182, 2.272668),
                pt(2.647702, 5.000000),
                pt(2.378757, 5.530570),
                pt(2.162856, 6.185719),
                pt(2.000000, 7.000000),
            ],
        );
        test_chop_mono_cubic_at_y(
            reporter,
            "inverted curve, negative slope @ 3.0",
            &[pt(11.0, 2.0), pt(6.0, 3.0), pt(3.0, 2.0), pt(2.0, 7.0)],
            3.0,
            &[
                pt(11.000000, 2.000000),
                pt(8.500000, 2.500000),
                pt(6.500000, 2.500000),
                pt(5.000000, 3.000000),
                pt(3.500000, 3.500000),
                pt(2.500000, 4.500000),
                pt(2.000000, 7.000000),
            ],
        );
        test_chop_mono_cubic_at_y(
            reporter,
            "inverted curve, negative slope @ 2.5",
            &[pt(11.0, 2.0), pt(6.0, 3.0), pt(3.0, 2.0), pt(2.0, 7.0)],
            2.5,
            &[
                pt(11.000000, 2.000000),
                pt(9.750000, 2.250000),
                pt(8.625000, 2.375000),
                pt(7.625000, 2.500000),
                pt(4.625000, 2.875000),
                pt(2.750000, 3.250000),
                pt(2.000000, 7.000000),
            ],
        );
        test_chop_mono_cubic_at_y(
            reporter,
            "big curve, negative slope @ 90",
            &[pt(-2.0, 100.0), pt(0.0, 0.0), pt(0.0, 0.0), pt(100.0, -2.0)],
            90.,
            &[
                pt(-2.000000, 100.000000),
                pt(-1.930979, 96.548965),
                pt(-1.864341, 93.217033),
                pt(-1.795892, 90.000000),
                pt(0.119096, -0.002382),
                pt(3.451032, -0.069021),
                pt(100.000000, -2.000000),
            ],
        );
        test_chop_mono_cubic_at_y(
            reporter,
            "big curve, negative slope @ 10",
            &[pt(-2.0, 100.0), pt(0.0, 0.0), pt(0.0, 0.0), pt(100.0, -2.0)],
            10.,
            &[
                pt(-2.000000, 100.000000),
                pt(-0.937505, 46.875271),
                pt(-0.439458, 21.972910),
                pt(14.787060, 10.000000),
                pt(28.222368, -0.564447),
                pt(53.124729, -1.062495),
                pt(100.000000, -2.000000),
            ],
        );
        test_chop_mono_cubic_at_y(
            reporter,
            "big curve, negative slope @ 0",
            &[pt(-2.0, 100.0), pt(0.0, 0.0), pt(0.0, 0.0), pt(100.0, -2.0)],
            0.,
            &[
                pt(-2.000000, 100.000000),
                pt(-0.426983, 21.349131),
                pt(-0.091157, 4.557854),
                pt(48.633648, 0.000000),
                pt(61.859592, -1.237192),
                pt(78.650871, -1.573017),
                pt(100.000000, -2.000000),
            ],
        );
        test_chop_mono_cubic_at_y(
            reporter,
            "ossfuzz:55680 curve barely crosses Y axis",
            &[
                pt(-250.121582, -1180.09509),
                pt(10.007843, -1180.09509),
                pt(20.015685, -786.041259),
                pt(40.0313721, 2.0664072),
            ],
            0.,
            &[
                pt(-250.121582, -1180.095093),
                pt(9.780392, -1180.095093),
                pt(19.997992, -786.730042),
                pt(39.978889, 0.000000),
                pt(39.996376, 0.688501),
                pt(40.013870, 1.377304),
                pt(40.031372, 2.066407),
            ],
        );
    }
);

// Port of: tests/GeometryTest.cpp#L852-L861 (chrome/m156)
def_test!(GeometryChopMonoCubicAtY_OutOfRangeReturnFalse, |reporter| {
    let inputs = [pt(0.0, 0.0), pt(0.0, 0.0), pt(10.0, 10.0), pt(10.0, 10.0)];
    let mut outputs = [Point::default(); 7];

    // Too low
    reporter_assert!(
        reporter,
        !chop_mono_cubic_at_y(&inputs, -10.0, &mut outputs)
    );
    // Too high
    reporter_assert!(reporter, !chop_mono_cubic_at_y(&inputs, 20.0, &mut outputs));
});

// Port of: tests/GeometryTest.cpp#L863-L884 (chrome/m156)
fn test_chop_mono_cubic_at_x(
    reporter: &mut Reporter,
    _name: &str, // skia-rust: `skiatest::ReporterContext` is not ported; the name is unused
    curve_inputs: &[Point],
    x_to_chop_at: scalar,
    expected_outputs: &[Point],
) {
    reporter_assert!(
        reporter,
        curve_inputs.len() == 4,
        "Invalid test case. Input curve should have 4 points"
    );
    reporter_assert!(
        reporter,
        expected_outputs.len() == 7,
        "Invalid test case. Outputs should have 7 points"
    );
    reporter_assert!(
        reporter,
        scalar::nearly_equal(expected_outputs[3].x, x_to_chop_at, None),
        "Invalid test case. 4th point's X should be {x_to_chop_at}"
    );

    let mut outputs = [Point::default(); 7];
    // Make sure it actually chopped
    reporter_assert!(
        reporter,
        chop_mono_cubic_at_x(curve_inputs, x_to_chop_at, &mut outputs)
    );

    for i in 0..7 {
        reporter_assert!(
            reporter,
            nearly_equal(expected_outputs[i], outputs[i]),
            "({}, {}) != ({}, {}) at index {}",
            expected_outputs[i].x,
            expected_outputs[i].y,
            outputs[i].x,
            outputs[i].y,
            i
        );
    }
}

// Port of: tests/GeometryTest.cpp#L886-L1002 (chrome/m156)
def_test!(
    #[allow(clippy::excessive_precision)] // literals copied verbatim from the C++ test
    #[allow(clippy::unreadable_literal)] // literals copied verbatim from the C++ test
    GeometryChopMonoCubicAtX_Successful,
    |reporter| {
        // These cubics are all arbitrary, picked using Desmos for something that looked "nice".

        test_chop_mono_cubic_at_x(
            reporter,
            "straight, positive slope @ 2.5",
            &[pt(0.0, 0.0), pt(0.0, 0.0), pt(10.0, 10.0), pt(10.0, 10.0)],
            2.5,
            &[
                pt(0.000000, 0.000000),
                pt(0.000000, 0.000000),
                pt(1.065055, 1.065055),
                pt(2.500000, 2.500000),
                pt(5.461981, 5.461981),
                pt(10.000000, 10.000000),
                pt(10.000000, 10.000000),
            ],
        );
        test_chop_mono_cubic_at_x(
            reporter,
            "straight, positive slope @ 5.0",
            &[pt(0.0, 0.0), pt(0.0, 0.0), pt(10.0, 10.0), pt(10.0, 10.0)],
            5.0,
            &[
                pt(0.000000, 0.000000),
                pt(0.000000, 0.000000),
                pt(2.500000, 2.500000),
                pt(5.000000, 5.000000),
                pt(7.500000, 7.500000),
                pt(10.000000, 10.000000),
                pt(10.000000, 10.000000),
            ],
        );
        test_chop_mono_cubic_at_x(
            reporter,
            "straight, positive slope @ 9.0",
            &[pt(0.0, 0.0), pt(0.0, 0.0), pt(10.0, 10.0), pt(10.0, 10.0)],
            9.0,
            &[
                pt(0.000000, 0.000000),
                pt(0.000000, 0.000000),
                pt(6.467375, 6.467375),
                pt(9.000000, 9.000000),
                pt(9.616623, 9.616623),
                pt(10.000000, 10.000000),
                pt(10.000000, 10.000000),
            ],
        );
        test_chop_mono_cubic_at_x(
            reporter,
            "straight, positive slope @ 10.0",
            &[pt(0.0, 0.0), pt(0.0, 0.0), pt(10.0, 10.0), pt(10.0, 10.0)],
            10.0,
            &[
                pt(0.000000, 0.000000),
                pt(0.000000, 0.000000),
                pt(10.000000, 10.000000),
                pt(10.000000, 10.000000),
                pt(10.000000, 10.000000),
                pt(10.000000, 10.000000),
                pt(10.000000, 10.000000),
            ],
        );
        test_chop_mono_cubic_at_x(
            reporter,
            "curve, positive slope @ 2.0",
            &[pt(1.0, 1.0), pt(5.0, 2.0), pt(7.0, 4.0), pt(8.0, 7.0)],
            2.0,
            &[
                pt(1.000000, 1.000000),
                pt(1.348275, 1.087069),
                pt(1.681389, 1.181719),
                pt(2.000000, 1.283949),
                pt(5.340694, 2.355856),
                pt(7.087069, 4.261207),
                pt(8.000000, 7.000000),
            ],
        );
        test_chop_mono_cubic_at_x(
            reporter,
            "curve, positive slope @ 5.0",
            &[pt(1.0, 1.0), pt(5.0, 2.0), pt(7.0, 4.0), pt(8.0, 7.0)],
            5.0,
            &[
                pt(1.000000, 1.000000),
                pt(2.650396, 1.412599),
                pt(3.960316, 1.995436),
                pt(5.000000, 2.748511),
                pt(6.480158, 3.820634),
                pt(7.412599, 5.237797),
                pt(8.000000, 7.000000),
            ],
        );
        test_chop_mono_cubic_at_x(
            reporter,
            "curve, negative slope @ 5.0",
            &[pt(2.0, 7.0), pt(3.0, 2.0), pt(6.0, 3.0), pt(11.0, 2.0)],
            5.0,
            &[
                pt(2.000000, 7.000000),
                pt(2.500000, 4.500000),
                pt(3.500000, 3.500000),
                pt(5.000000, 3.000000),
                pt(6.500000, 2.500000),
                pt(8.500000, 2.500000),
                pt(11.000000, 2.000000),
            ],
        );
        test_chop_mono_cubic_at_x(
            reporter,
            "curve, negative slope @ 3.0",
            &[pt(2.0, 7.0), pt(3.0, 2.0), pt(6.0, 3.0), pt(11.0, 2.0)],
            3.0,
            &[
                pt(2.000000, 7.000000),
                pt(2.228714, 5.856432),
                pt(2.562047, 5.026724),
                pt(3.000000, 4.415163),
                pt(4.476901, 2.352807),
                pt(7.143568, 2.771286),
                pt(11.000000, 2.000000),
            ],
        );
        test_chop_mono_cubic_at_x(
            reporter,
            "curve, negative slope @ 2.5",
            &[pt(2.0, 7.0), pt(3.0, 2.0), pt(6.0, 3.0), pt(11.0, 2.0)],
            2.5,
            &[
                pt(2.000000, 7.000000),
                pt(2.131881, 6.340593),
                pt(2.298548, 5.785543),
                pt(2.500000, 5.316498),
                pt(3.826073, 2.228977),
                pt(6.659407, 2.868119),
                pt(11.000000, 2.000000),
            ],
        );
        test_chop_mono_cubic_at_x(
            reporter,
            "inverted curve, negative slope @ 5.0",
            &[pt(11.0, 2.0), pt(6.0, 3.0), pt(3.0, 2.0), pt(2.0, 7.0)],
            5.0,
            &[
                pt(11.000000, 2.000000),
                pt(8.500000, 2.500000),
                pt(6.500000, 2.500000),
                pt(5.000000, 3.000000),
                pt(3.500000, 3.500000),
                pt(2.500000, 4.500000),
                pt(2.000000, 7.000000),
            ],
        );
        test_chop_mono_cubic_at_x(
            reporter,
            "inverted curve, negative slope @ 3.0",
            &[pt(11.0, 2.0), pt(6.0, 3.0), pt(3.0, 2.0), pt(2.0, 7.0)],
            3.0,
            &[
                pt(11.000000, 2.000000),
                pt(7.143568, 2.771286),
                pt(4.476901, 2.352807),
                pt(3.000000, 4.415163),
                pt(2.562047, 5.026724),
                pt(2.228714, 5.856432),
                pt(2.000000, 7.000000),
            ],
        );
        test_chop_mono_cubic_at_x(
            reporter,
            "inverted curve, negative slope @ 2.5",
            &[pt(11.0, 2.0), pt(6.0, 3.0), pt(3.0, 2.0), pt(2.0, 7.0)],
            2.5,
            &[
                pt(11.000000, 2.000000),
                pt(6.659407, 2.868119),
                pt(3.826073, 2.228977),
                pt(2.500000, 5.316498),
                pt(2.298548, 5.785543),
                pt(2.131881, 6.340593),
                pt(2.000000, 7.000000),
            ],
        );
        test_chop_mono_cubic_at_x(
            reporter,
            "big curve, negative slope @ 90",
            &[pt(-2.0, 100.0), pt(0.0, 0.0), pt(0.0, 0.0), pt(100.0, -2.0)],
            90.,
            &[
                pt(-2.000000, 100.000000),
                pt(-0.069021, 3.451032),
                pt(-0.002382, 0.119096),
                pt(90.000000, -1.795892),
                pt(93.217033, -1.864341),
                pt(96.548965, -1.930979),
                pt(100.000000, -2.000000),
            ],
        );
        test_chop_mono_cubic_at_x(
            reporter,
            "big curve, negative slope @ 10",
            &[pt(-2.0, 100.0), pt(0.0, 0.0), pt(0.0, 0.0), pt(100.0, -2.0)],
            10.,
            &[
                pt(-2.000000, 100.000000),
                pt(-1.062495, 53.124729),
                pt(-0.564447, 28.222368),
                pt(10.000000, 14.787060),
                pt(21.972910, -0.439458),
                pt(46.875271, -0.937505),
                pt(100.000000, -2.000000),
            ],
        );
        test_chop_mono_cubic_at_x(
            reporter,
            "big curve, negative slope @ 0",
            &[pt(-2.0, 100.0), pt(0.0, 0.0), pt(0.0, 0.0), pt(100.0, -2.0)],
            0.,
            &[
                pt(-2.000000, 100.000000),
                pt(-1.573017, 78.650871),
                pt(-1.237192, 61.859592),
                pt(0.000000, 48.633648),
                pt(4.557854, -0.091157),
                pt(21.349131, -0.426983),
                pt(100.000000, -2.000000),
            ],
        );
    }
);

// Port of: tests/GeometryTest.cpp#L1004-L1013 (chrome/m156)
def_test!(GeometryChopMonoCubicAtX_OutOfRangeReturnFalse, |reporter| {
    let inputs = [pt(0.0, 0.0), pt(0.0, 0.0), pt(10.0, 10.0), pt(10.0, 10.0)];
    let mut outputs = [Point::default(); 7];

    // Too low
    reporter_assert!(
        reporter,
        !chop_mono_cubic_at_x(&inputs, -10.0, &mut outputs)
    );
    // Too high
    reporter_assert!(reporter, !chop_mono_cubic_at_x(&inputs, 20.0, &mut outputs));
});

// Port of: tests/GeometryTest.cpp#L1015-L1060 (chrome/m156)
def_test!(
    #[allow(clippy::items_after_statements, clippy::similar_names)]
    // constants stay next to the C++ code they mirror; names follow the C++ test
    ConicsWithCrazyW,
    |reporter| {
        let max = f32::MAX;
        let inf = FLOAT_INFINITY;
        let nanq = FLOAT_NAN;
        // skia-rust: Rust has no distinct signaling NaN constant; this one has the signaling bit
        // pattern (a NaN weight is treated the same either way).
        let nans = f32::from_bits(0x7fa0_0000);

        let weights = [
            0.0,
            1.0f32 / 65535.0,
            1.0,
            65535.0,
            max / 4.0,
            max / 2.0,
            max,
            inf,
            nanq,
            nans,
        ];

        let length = 100.0f32;
        let mut conic = Conic::new(pt(0.0, length), pt(0.0, 0.0), pt(length, 0.0), 1.0);

        // any points on the conic must lie within the convex-hull of the conic's control-points
        let is_in_convex_hull = |p: Point| {
            let sum = p.x + p.y;
            let tiny_slop = 1.0f32 / 32768.0;
            p.x >= 0.0 && p.y >= 0.0 && (sum <= length + tiny_slop)
        };

        let tol = 0.0f32;
        for sign in [1.0f32, -1.0] {
            for w in weights {
                conic.w = w * sign;

                const K_EXTREME_POW2: i32 = 30; // any larger, and 1 << that would overflow
                let pow2 = conic.compute_quad_pow2(tol);

                reporter_assert!(reporter, (0..=K_EXTREME_POW2).contains(&pow2));
                let num_quads = 1usize << pow2;
                reporter_assert!(reporter, num_quads > 0);
                let num_points = num_quads * 2 + 1;
                reporter_assert!(reporter, num_points >= 3);

                let mut pts = vec![Point::default(); num_points];
                let num_quads2 = conic.chop_into_quads_pow2(&mut pts, pow2);

                reporter_assert!(reporter, num_quads2 <= num_quads);
                let num_points2 = num_quads2 * 2 + 1;
                reporter_assert!(reporter, num_points2 <= num_points);

                for p in &pts[..num_points2] {
                    reporter_assert!(reporter, is_finite(p.x) && is_finite(p.y));
                    reporter_assert!(reporter, is_in_convex_hull(*p));
                }
            }
        }
    }
);
