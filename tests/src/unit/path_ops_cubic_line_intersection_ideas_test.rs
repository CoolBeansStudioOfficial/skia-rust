// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsCubicLineIntersectionIdeas.cpp (chrome/m156)

#![cfg(test)]

// The `SkDebugf` output of these tests is not ported. The `#if 0` blocks are not ported.

use crate::def_test;
use crate::unit::path_ops_test_common::CubicPts;
use skia_rust_core::random::Random;
use skia_rust_pathops::cubic::DCubic;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::quad::DQuad;
use skia_rust_pathops::types::{std_max, std_min};

/// `gPathOpsCubicLineIntersectionIdeasVerbose`: false, so `PathOpsCubicLineRoots` returns at once.
const VERBOSE: bool = false;

/// `if ((false)) { // disable for now }` in `PathOpsCubicLineFailures` and
/// `PathOpsCubicLineOneFailure`.
const DISABLED_FOR_NOW: bool = false;

/// `struct CubicLineFailures { CubicPts c; double t; SkDPoint p; }`.
// Port of: tests/PathOpsCubicLineIntersectionIdeas.cpp#L22-L26 (chrome/m156)
#[derive(Copy, Clone, Debug)]
struct CubicLineFailure {
    c: CubicPts,
    #[allow(dead_code)] // recorded in the C++ table, not read by the tests
    t: f64,
    p: DPoint,
}

/// `static constexpr auto cubicLineFailures`.
// Port of: tests/PathOpsCubicLineIntersectionIdeas.cpp#L27-L46 (chrome/m156)
#[allow(clippy::unreadable_literal, clippy::excessive_precision)] // the table keeps the C++ literals verbatim
static CUBIC_LINE_FAILURES: &[CubicLineFailure] = &[
    CubicLineFailure {
        c: CubicPts::new([
            DPoint::new(-164.3726806640625, 36.826904296875),
            DPoint::new(-189.045166015625, -953.2220458984375),
            DPoint::new(926.505859375, -897.36175537109375),
            DPoint::new(-139.33489990234375, 204.40771484375),
        ]),
        t: 0.37329583,
        p: DPoint::new(107.54935269006289, -632.13736293162208),
    },
    CubicLineFailure {
        c: CubicPts::new([
            DPoint::new(784.056884765625, -554.8350830078125),
            DPoint::new(67.5489501953125, 509.0224609375),
            DPoint::new(-447.713134765625, 751.375),
            DPoint::new(415.7784423828125, 172.22265625),
        ]),
        t: 0.660005242,
        p: DPoint::new(-32.973148967736151, 478.01341797403569),
    },
    CubicLineFailure {
        c: CubicPts::new([
            DPoint::new(-580.6834716796875, -127.044921875),
            DPoint::new(-872.8983154296875, -945.54302978515625),
            DPoint::new(260.8092041015625, -909.34991455078125),
            DPoint::new(-976.2125244140625, -18.46551513671875),
        ]),
        t: 0.578826774,
        p: DPoint::new(-390.17910153915489, -687.21144412296007),
    },
];

/// `static double binary_search(const SkDCubic& cubic, double step, const SkDPoint& pt, double t,
/// int* iters)`. The failure path's `SkDebugf` output is not ported.
// Port of: tests/PathOpsCubicLineIntersectionIdeas.cpp#L68-L111 (chrome/m156)
fn binary_search(cubic: &DCubic, step: f64, pt: DPoint, t: f64, iters: &mut i32) -> f64 {
    let mut step = step;
    let mut t = t;
    loop {
        *iters += 1;
        let cubic_at_t = cubic.pt_at_t(t);
        if cubic_at_t.approximately_equal(pt) {
            break;
        }
        let calc_x = cubic_at_t.x - pt.x;
        let calc_y = cubic_at_t.y - pt.y;
        let calc_dist = calc_x * calc_x + calc_y * calc_y;
        if step == 0.0 {
            return -1.0;
        }
        let last_step = step;
        step /= 2.0;
        let less_pt = cubic.pt_at_t(t - last_step);
        let less_x = less_pt.x - pt.x;
        let less_y = less_pt.y - pt.y;
        let less_dist = less_x * less_x + less_y * less_y;
        // use larger x/y difference to choose step
        if calc_dist > less_dist {
            t -= step;
            t = std_max(0.0, t);
        } else {
            let more_pt = cubic.pt_at_t(t + last_step);
            let more_x = more_pt.x - pt.x;
            let more_y = more_pt.y - pt.y;
            let more_dist = more_x * more_x + more_y * more_y;
            if calc_dist <= more_dist {
                continue;
            }
            t += step;
            t = std_min(1.0, t);
        }
    }
    t
}

/// The exponent `frexp(x, &e)` returns for a finite `x > 0`: `x = m * 2^e` with `0.5 <= m < 1`.
// Port of: C99 frexp, as used by PathOpsCubicLineRoots
fn frexp_exponent(x: f64) -> i32 {
    debug_assert!(x.is_finite() && x > 0.0);
    // a normal double has its biased exponent in bits 52..63; the mantissa then lies in [1, 2)
    #[allow(clippy::cast_possible_wrap)] // the biased exponent field is 11 bits wide
    let biased = ((x.to_bits() >> 52) & 0x7ff) as i32;
    biased - 1022
}

/// `testOneFailure(const CubicLineFailures& failure)`.
// Port of: tests/PathOpsCubicLineIntersectionIdeas.cpp#L266-L285 (chrome/m156)
#[allow(clippy::many_single_char_names)] // the coefficients are named A, B, C, D as in SkDCubic::Coefficients
fn test_one_failure(failure: &CubicLineFailure) -> f64 {
    let cubic = DCubic::new(failure.c.pts);
    let pt = failure.p;
    let (a, b, c, d) = DCubic::coefficients([
        cubic.pts[0].y,
        cubic.pts[1].y,
        cubic.pts[2].y,
        cubic.pts[3].y,
    ]);
    let d = d - pt.y;
    let mut all_roots = [0.0_f64; 3];
    let mut valid_roots = [0.0_f64; 3];
    let real_roots = DCubic::roots_real(a, b, c, d, &mut all_roots);
    let valid = DQuad::add_valid_ts(&all_roots[..real_roots], &mut valid_roots);
    assert_eq!(valid, 1); // SkASSERT_RELEASE
    assert!(real_roots != 1); // SkASSERT_RELEASE
    let t = valid_roots[0];
    let calc_pt = cubic.pt_at_t(t);
    assert!(!calc_pt.approximately_equal(pt)); // SkASSERT_RELEASE
    let mut iters = 0;
    binary_search(&cubic, 0.1, pt, t, &mut iters)
}

/// The body of `PathOpsCubicLineRoots`, run only with `gPathOpsCubicLineIntersectionIdeasVerbose`.
// Port of: tests/PathOpsCubicLineIntersectionIdeas.cpp#L150-L264 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // large_bits is a frexp exponent, never negative here
#[allow(clippy::many_single_char_names)] // the coefficients are named A, B, C, D as in SkDCubic::Coefficients
fn cubic_line_roots() {
    let mut ran = Random::default();
    let mut worst_step = [0.0_f64; 256];
    let mut iters = 0_i32;
    for _index in 0..1_000_000_000_i32 {
        let origin = DPoint::new(
            f64::from(ran.next_range_f(-1000.0, 1000.0)),
            f64::from(ran.next_range_f(-1000.0, 1000.0)),
        );
        let cu_pts = CubicPts::new([
            origin,
            DPoint::new(
                f64::from(ran.next_range_f(-1000.0, 1000.0)),
                f64::from(ran.next_range_f(-1000.0, 1000.0)),
            ),
            DPoint::new(
                f64::from(ran.next_range_f(-1000.0, 1000.0)),
                f64::from(ran.next_range_f(-1000.0, 1000.0)),
            ),
            DPoint::new(
                f64::from(ran.next_range_f(-1000.0, 1000.0)),
                f64::from(ran.next_range_f(-1000.0, 1000.0)),
            ),
        ]);
        // construct a line at a known intersection
        let t_seed = f64::from(ran.next_range_f(0.0, 1.0));
        let cubic = DCubic::new(cu_pts.pts);
        let pt = cubic.pt_at_t(t_seed);
        // skip answers with no intersections (although note the bug!) or two, or more
        // see if the line / cubic has a fun range of roots
        let (a, b, c, d) = DCubic::coefficients([
            cubic.pts[0].y,
            cubic.pts[1].y,
            cubic.pts[2].y,
            cubic.pts[3].y,
        ]);
        let d = d - pt.y;
        let mut all_roots = [0.0_f64; 3];
        let mut valid_roots = [0.0_f64; 3];
        let real_roots = DCubic::roots_real(a, b, c, d, &mut all_roots);
        let valid = DQuad::add_valid_ts(&all_roots[..real_roots], &mut valid_roots);
        if valid != 1 {
            continue;
        }
        if real_roots == 1 {
            continue;
        }
        let t = valid_roots[0];
        let calc_pt = cubic.pt_at_t(t);
        if calc_pt.approximately_equal(pt) {
            continue;
        }
        let mut largest = std_max(all_roots[0].abs(), all_roots[1].abs());
        if real_roots == 3 {
            largest = std_max(largest, all_roots[2].abs());
        }
        let large_bits: i32 = if largest <= 1.0 {
            let mut smallest = std_min(all_roots[0], all_roots[1]);
            if real_roots == 3 {
                smallest = std_min(smallest, all_roots[2]);
            }
            assert!(smallest < 0.0); // SkASSERT_RELEASE
            assert!(smallest >= -1.0); // SkASSERT_RELEASE
            0
        } else {
            let large_bits = frexp_exponent(largest);
            assert!(large_bits >= 0); // SkASSERT_RELEASE
            assert!(large_bits < 256); // SkASSERT_RELEASE
            large_bits
        };
        let mut step = 1e-6;
        if large_bits > 21 {
            step = 1e-1;
        } else if large_bits > 18 {
            step = 1e-2;
        } else if large_bits > 15 {
            step = 1e-3;
        } else if large_bits > 12 {
            step = 1e-4;
        } else if large_bits > 9 {
            step = 1e-5;
        }
        let diff = loop {
            let new_t = binary_search(&cubic, step, pt, t, &mut iters);
            if new_t >= 0.0 {
                break (t - new_t).abs();
            }
            step *= 1.5;
            assert!(step < 1.0); // SkASSERT_RELEASE
        };
        let slot = large_bits as usize;
        worst_step[slot] = std_max(worst_step[slot], diff);
    }
    // The statistics are printed with SkDebugf, which is not ported.
}

// Port of: tests/PathOpsCubicLineIntersectionIdeas.cpp#L150-L154 (chrome/m156)
def_test!(PathOpsCubicLineRoots, |_reporter| {
    // slow; exclude it by default
    if VERBOSE {
        cubic_line_roots();
    }
});

// Port of: tests/PathOpsCubicLineIntersectionIdeas.cpp#L287-L295 (chrome/m156)
def_test!(PathOpsCubicLineFailures, |_reporter| {
    if DISABLED_FOR_NOW {
        for failure in CUBIC_LINE_FAILURES {
            let new_t = test_one_failure(failure);
            assert!(new_t >= 0.0); // SkASSERT_RELEASE
        }
    }
});

// Port of: tests/PathOpsCubicLineIntersectionIdeas.cpp#L297-L303 (chrome/m156)
def_test!(PathOpsCubicLineOneFailure, |_reporter| {
    if DISABLED_FOR_NOW {
        let failure = &CUBIC_LINE_FAILURES[1];
        let new_t = test_one_failure(failure);
        assert!(new_t >= 0.0); // SkASSERT_RELEASE
    }
});
