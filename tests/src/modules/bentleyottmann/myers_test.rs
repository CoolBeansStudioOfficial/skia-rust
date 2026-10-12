// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/bentleyottmann/tests/MyersTest.cpp (chrome/m156)

#![cfg(test)]

use std::time::{Duration, Instant};

use skia_rust_bentleyottmann::myers::{
    Point, Segment, brute_force_crossings, myers_find_crossings, s0_intersects_s1,
    s0_less_than_s1_at_y, segment_less_than_upper_to_insert, slope_s0_less_than_slope_s1,
};
use skia_rust_core::floating_point::{FLOAT_PI, float_round2int};
use skia_rust_core::random::Random;

use crate::{Reporter, def_test, reporter_assert};

/// `std::minmax(a, b)` for points: the pair in ascending order.
fn minmax_points(a: Point, b: Point) -> (Point, Point) {
    if b < a { (b, a) } else { (a, b) }
}

/// `Segment` from endpoints given as `(x, y)` pairs.
fn seg(p0: (i32, i32), p1: (i32, i32)) -> Segment {
    Segment::new(Point::new(p0.0, p0.1), Point::new(p1.0, p1.1))
}

/// `swap_ends`: the same segment with its endpoints in the other order.
fn swap_ends(s: &Segment) -> Segment {
    Segment::new(s.lower(), s.upper())
}

def_test!(MFC_order_segment_points, |r| {
    {
        let p0 = Point::new(0, 0);
        let p1 = Point::new(1, 1);
        reporter_assert!(r, minmax_points(p0, p1) == (p0, p1));
        reporter_assert!(r, minmax_points(p1, p0) == (p0, p1));
    }
    {
        let p0 = Point::new(0, 0);
        let p1 = Point::new(-1, 1);
        reporter_assert!(r, minmax_points(p0, p1) == (p0, p1));
        reporter_assert!(r, minmax_points(p1, p0) == (p0, p1));
    }
    {
        let p0 = Point::new(0, 0);
        let p1 = Point::new(0, 1);
        reporter_assert!(r, minmax_points(p0, p1) == (p0, p1));
        reporter_assert!(r, minmax_points(p1, p0) == (p0, p1));
    }
});

def_test!(MFC_segment_ctor, |r| {
    {
        let p0 = Point::new(0, 0);
        let p1 = Point::new(1, 1);
        let s = Segment::new(p1, p0);
        let (u, l) = (s.upper(), s.lower());
        reporter_assert!(r, u == s.upper() && u == p0);
        reporter_assert!(r, l == s.lower() && l == p1);
    }
    {
        let p0 = Point::new(0, 0);
        let p1 = Point::new(0, 1);
        let s = Segment::new(p1, p0);
        let (u, l) = (s.upper(), s.lower());
        reporter_assert!(r, u == s.upper() && u == p0);
        reporter_assert!(r, l == s.lower() && l == p1);
    }
});

def_test!(MFC_slope_less_than, |r| {
    {
        let s0 = seg((0, 0), (1, 1));
        let s1 = seg((0, 0), (-1, 1));
        reporter_assert!(r, !slope_s0_less_than_slope_s1(&s0, &s1));
        reporter_assert!(r, slope_s0_less_than_slope_s1(&s1, &s0));
        reporter_assert!(r, !slope_s0_less_than_slope_s1(&s0, &s0));
    }
    {
        let s = seg((0, 0), (0, 1));
        reporter_assert!(r, !slope_s0_less_than_slope_s1(&s, &s));
    }
    {
        // Check slopes for horizontals.
        let s0 = seg((-2, 0), (1, 0));
        let s1 = seg((-1, 0), (2, 0));
        reporter_assert!(r, !slope_s0_less_than_slope_s1(&s0, &s1));
        reporter_assert!(r, !slope_s0_less_than_slope_s1(&s1, &s0));
    }
    {
        // Check slopes for horizontals.
        let s0 = seg((-2, 0), (1, 0));
        let s1 = seg((0, 0), (1, 1));
        reporter_assert!(r, !slope_s0_less_than_slope_s1(&s0, &s1));
        reporter_assert!(r, slope_s0_less_than_slope_s1(&s1, &s0));
    }
});

def_test!(MFC_segment_less_than_upper_to_insert, |r| {
    let s0 = seg((-10, -10), (10, 10));
    let s1 = seg((10, -10), (-10, 10));
    let to_insert = seg((0, 0), (0, 3));

    // Above y = 0, the sweepLine is {s0, s1}, but at y=0 s1 and s0 swap because of their slopes.
    let sweep_line = [s1, s0];
    let insertion_point =
        sweep_line.partition_point(|s| segment_less_than_upper_to_insert(s, &to_insert));

    // The insertion point is between s1 and s0.
    reporter_assert!(r, sweep_line[insertion_point] == s0);
    reporter_assert!(r, sweep_line[insertion_point - 1] == s1);
});

def_test!(MFC_less_than_at_y, |r| {
    {
        let s0 = seg((0, 0), (2, 2));
        let s1 = seg((0, 0), (-2, 2));
        reporter_assert!(r, !s0_less_than_s1_at_y(&s0, &s1, 1));
        reporter_assert!(r, s0_less_than_s1_at_y(&s1, &s0, 1));
    }
    {
        // cross at 0 use slope to break tie.
        let s0 = seg((-2, -2), (2, 2));
        let s1 = seg((2, -2), (-2, 2));
        reporter_assert!(r, s0_less_than_s1_at_y(&s0, &s1, -1));
        reporter_assert!(r, !s0_less_than_s1_at_y(&s1, &s0, -1));
        reporter_assert!(r, !s0_less_than_s1_at_y(&s0, &s1, 0));
        reporter_assert!(r, s0_less_than_s1_at_y(&s1, &s0, 0));
        reporter_assert!(r, !s0_less_than_s1_at_y(&s0, &s1, 1));
        reporter_assert!(r, s0_less_than_s1_at_y(&s1, &s0, 1));
    }
    {
        let s0 = seg((-2, -100), (-2, 89));
        let s1 = seg((6, -70), (-2, 72));
        reporter_assert!(r, !s0_less_than_s1_at_y(&s0, &s1, 72));
    }
});

def_test!(
    #[allow(clippy::too_many_lines)]
    MFC_has_inner_intersection,
    |r| {
        let check_intersection = |r: &mut Reporter, s0: Segment, s1: Segment| {
            reporter_assert!(r, s0_intersects_s1(&s0, &s1));
            reporter_assert!(r, s0_intersects_s1(&s1, &s0));
            reporter_assert!(r, s0_intersects_s1(&swap_ends(&s0), &swap_ends(&s1)));
            reporter_assert!(r, s0_intersects_s1(&swap_ends(&s1), &swap_ends(&s0)));
        };

        {
            let s0 = seg((-1, 0), (1, 0));
            let s1 = seg((0, 1), (0, -1));
            check_intersection(r, s0, s1);
        }
        {
            let s0 = seg((-1, 0), (5, 0));
            let s1 = seg((0, 1), (0, -1));
            check_intersection(r, s0, s1);
        }
        {
            let s0 = seg((5, 0), (-1, 0));
            let s1 = seg((0, -1), (0, 1));
            check_intersection(r, s0, s1);
        }
        {
            let s0 = seg((-5, -5), (5, 5));
            let s1 = seg((-5, 5), (5, -5));
            check_intersection(r, s0, s1);
        }

        // Test very close segments (x0, 0) -> (x1, 1) & (x2, 0) -> (x3, 1)
        for x0 in -10_i32..=10 {
            for x1 in -10_i32..=10 {
                for x2 in -10_i32..=10 {
                    for x3 in -10_i32..=10 {
                        let p0 = Point::new(x0, 0);
                        let p1 = Point::new(x1, 1);
                        let p2 = Point::new(x2, 0);
                        let p3 = Point::new(x3, 1);
                        let actual = s0_intersects_s1(&Segment::new(p0, p1), &Segment::new(p2, p3));
                        let expected = (x0 <= x2 && x3 <= x1) || (x2 <= x0 && x1 <= x3);
                        if actual != expected {
                            let _ = s0_intersects_s1(&Segment::new(p0, p1), &Segment::new(p2, p3));
                            reporter_assert!(r, actual == expected);
                        }
                    }
                }
            }
        }
        {
            let s0 = seg((0, -100), (0, -50));
            let s1 = seg((100, -100), (-100, 100)); // goes through (0,0)
            reporter_assert!(r, !s0_intersects_s1(&s0, &s1));
            reporter_assert!(r, !s0_intersects_s1(&s1, &s0));
        }
        {
            let s0 = seg((0, 100), (0, 50));
            let s1 = seg((100, -100), (-100, 100)); // goes through (0,0)
            reporter_assert!(r, !s0_intersects_s1(&s0, &s1));
            reporter_assert!(r, !s0_intersects_s1(&s1, &s0));
        }
        {
            let s0 = seg((0, -101), (0, -50));
            let s1 = seg((100, -100), (-100, 100)); // goes through (0,0)
            reporter_assert!(r, !s0_intersects_s1(&s0, &s1));
            reporter_assert!(r, !s0_intersects_s1(&s1, &s0));
        }
    }
);

/// The test cases of `MFC_myers_brute_force_comparison`: each is a list of segments.
#[allow(clippy::too_many_lines)] // the data of the C++ test, one line per case
fn comparison_cases() -> Vec<Vec<Segment>> {
    vec![
        vec![
            seg((-58, -100), (75, 105)),
            seg((149, -58), (-156, 49)),
            seg((-34, -55), (37, 49)),
            seg((-58, -100), (75, 105)),
            seg((-147, -229), (143, 220)),
        ],
        vec![seg((-57, -138), (56, 178)), seg((14, -146), (-22, 132))],
        vec![
            seg((-4, -23), (-11, 11)),
            seg((6, -2), (-11, 11)),
            seg((159, -244), (-159, 233)),
        ],
        vec![
            seg((-7, -22), (10, 14)),
            seg((-7, -71), (-7, 80)),
            seg((-7, -22), (-4, 5)),
        ],
        vec![
            seg((91, -22), (-93, 24)),
            seg((31, -18), (-25, 7)),
            seg((-25, 7), (33, 12)),
            seg((-26, -24), (18, 20)),
        ],
        vec![
            seg((2, -21), (-16, 7)),
            seg((-45, -28), (51, 35)),
            seg((39, -48), (-53, 44)),
            seg((-16, 7), (26, 7)),
        ],
        vec![
            seg((142, -82), (-128, 64)),
            seg((208, -16), (-217, -3)),
            seg((91, -22), (-93, 24)),
            seg((31, -18), (-25, 7)),
            seg((-25, 7), (33, 12)),
        ],
        vec![
            seg((-159, -101), (167, 91)),
            seg((-96, -117), (99, 117)),
            seg((-16, -21), (12, 35)),
            seg((-48, -55), (33, 63)),
            seg((-16, -21), (26, 41)),
        ],
        vec![
            seg((-51, -18), (34, 1)),
            seg((189, -169), (-171, 150)),
            seg((24, -8), (-5, 7)),
            seg((24, -8), (-26, 16)),
            seg((54, -22), (-36, 20)),
        ],
        vec![seg((-29, -3), (15, -3)), seg((-28, -7), (15, -3))],
        vec![
            seg((20, -149), (-32, 130)),
            seg((-29, -3), (15, -3)),
            seg((-28, -7), (15, -3)),
        ],
        vec![
            seg((-32, -8), (16, -8)),
            seg((-28, -104), (23, 88)),
            seg((-17, -11), (16, -8)),
        ],
        vec![
            seg((-59, -9), (48, 11)),
            seg((-59, -9), (75, -9)),
            seg((173, -20), (-178, 13)),
        ],
        vec![seg((-11, 1), (12, 1)), seg((-42, -35), (54, 29))],
        vec![seg((14, -11), (-15, -2)), seg((-9, -2), (13, -2))], // both end same s0 horz s1 < s0
        vec![seg((-38, 7), (47, 7)), seg((-148, 6), (166, 7))],   // just sort of s0 along s1
        vec![seg((-26, -22), (9, 21)), seg((-32, -28), (13, 17))], // s1 endpoint on s0
        vec![seg((23, -2), (-12, 3)), seg((22, -13), (-5, 2))],   // s1 endpoint on s0
        vec![seg((-2, -100), (-2, 89)), seg((6, -70), (-2, 72))],
        vec![seg((8, -1), (-8, 19)), seg((-130, -93), (137, 85))], // Endpoint of s0 lies on s1
        vec![seg((-39, -111), (25, 119)), seg((-26, -112), (25, 119))], // Same end points
        vec![seg((-9, -5), (16, -5)), seg((90, -134), (-71, 144))], // Diag crossing horizontal
        vec![seg((-1, -1), (1, 1)), seg((1, -1), (-1, 1))],        // Crossing
        vec![seg((-1, -1), (-1, 1)), seg((1, -1), (1, 1))],        // Two vertical lines
        vec![seg((-1, -1), (1, -1)), seg((-1, 1), (1, 1))],        // Two horizontal lines
        vec![seg((-2, 1), (1, 1)), seg((-1, 1), (2, 1))],          // Overlapping horizontal lines
        vec![seg((0, -100), (0, -50)), seg((100, -100), (-100, 100))],
        vec![seg((0, 100), (0, 50)), seg((100, -100), (-100, 100))],
        vec![seg((0, -101), (0, -50)), seg((100, -100), (-100, 100))],
        vec![seg((0, 0), (0, 50)), seg((100, -100), (-100, 100))],
        vec![
            seg((-10, -10), (10, 10)),
            seg((-10, -10), (11, 11)),
            seg((10, -10), (-10, 10)),
        ],
        vec![
            seg((10, -10), (-10, 10)),
            seg((10, -10), (-11, 11)),
            seg((-10, -10), (10, 10)),
        ],
        vec![
            seg((-11, -11), (10, 10)),
            seg((-10, -10), (11, 11)),
            seg((10, -10), (-10, 10)),
        ],
    ]
}

def_test!(MFC_myers_brute_force_comparison, |r| {
    for segments in comparison_cases() {
        let mut myers_response = myers_find_crossings(&segments);
        let mut brute_response = brute_force_crossings(&segments);
        myers_response.sort();
        brute_response.sort();

        reporter_assert!(r, myers_response.len() == brute_response.len());
        // There should be no duplicate crossings.
        let mut myers_unique = myers_response.clone();
        myers_unique.dedup();
        reporter_assert!(r, myers_unique.len() == myers_response.len());
        let mut brute_unique = brute_response.clone();
        brute_unique.dedup();
        reporter_assert!(r, brute_unique.len() == brute_response.len());

        // Both should be equal.
        reporter_assert!(r, myers_response == brute_response);
    }
});

/// `StopWatch`: accumulates the time of timed sections.
struct StopWatch {
    count: i64,
    start: Instant,
    accumulated_time: Duration,
}

impl StopWatch {
    fn new() -> Self {
        Self {
            count: 0,
            start: Instant::now(),
            accumulated_time: Duration::ZERO,
        }
    }

    fn start(&mut self) {
        self.start = Instant::now();
    }

    fn stop(&mut self) {
        let stop = Instant::now();
        self.accumulated_time += stop - self.start;
        self.count += 1;
    }

    fn print(&self) {
        let average =
            i64::try_from(self.accumulated_time.as_micros()).unwrap_or(i64::MAX) / self.count;
        eprintln!("average time: {average} µs");
    }
}

/// `kRunRandomTest`: the random comparison is disabled in Skia, so the test returns at once.
const K_RUN_RANDOM_TEST: bool = false;

def_test!(
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::many_single_char_names,
        clippy::similar_names
    )]
    MFC_myers_brute_force_random_comparison,
    |r| {
        if !K_RUN_RANDOM_TEST {
            return;
        }
        let n: i32 = 200;
        let box_size: i32 = 20000;
        let mut random = Random::new((n + box_size).cast_unsigned());
        let mut segments: Vec<Segment> = Vec::new();
        let mut myers_stop_watch = StopWatch::new();
        let mut brute_stop_watch = StopWatch::new();
        for _trials in 0..100_000 {
            for _i in 0..n {
                let x = random.next_range_f(-box_size as f32, box_size as f32);
                let y = random.next_range_f(-box_size as f32, box_size as f32);
                let angle = random.next_f() * FLOAT_PI;
                let distance = random.next_range_f(10.0, 300.0);
                let p0 = Point::new(
                    float_round2int(x + skia_rust_core::libm::cosf(angle) * distance),
                    float_round2int(y + skia_rust_core::libm::sinf(angle) * distance),
                );
                let p1 = Point::new(
                    float_round2int(x - skia_rust_core::libm::cosf(angle) * distance),
                    float_round2int(y - skia_rust_core::libm::sinf(angle) * distance),
                );
                segments.push(Segment::new(p0, p1));
            }

            let mut myers_segments = segments.clone();
            let mut brute_segments = segments.clone();

            myers_stop_watch.start();
            let mut myers_response = myers_find_crossings(&myers_segments);
            myers_stop_watch.stop();

            brute_stop_watch.start();
            let mut brute_response = brute_force_crossings(&brute_segments);
            brute_stop_watch.stop();

            myers_response.sort();
            brute_response.sort();

            //SkDebugf("myers size: %zu brute size: %zu\n", myersResponse.size(), bruteResponse.size());
            reporter_assert!(r, myers_response.len() == brute_response.len());
            if myers_response.len() != brute_response.len() {
                eprintln!(
                    "myers size: {} brute size: {}",
                    myers_response.len(),
                    brute_response.len()
                );
                eprint!("{{");
                for s in &segments {
                    eprint!(
                        "{{{{{}, {}}}, {{{}, {}}}}}, ",
                        s.upper().x,
                        s.upper().y,
                        s.lower().x,
                        s.lower().y
                    );
                }
                eprintln!("}},");
            }

            // There should be no duplicate crossings.
            let mut myers_unique = myers_response.clone();
            myers_unique.dedup();
            reporter_assert!(r, myers_unique.len() == myers_response.len());
            let mut brute_unique = brute_response.clone();
            brute_unique.dedup();
            reporter_assert!(r, brute_unique.len() == brute_response.len());

            // Both should be equal.
            reporter_assert!(r, myers_response == brute_response);

            segments.clear();
            myers_segments.clear();
            brute_segments.clear();
        }
        myers_stop_watch.print();
        eprint!("brute ");
        brute_stop_watch.print();
    }
);
