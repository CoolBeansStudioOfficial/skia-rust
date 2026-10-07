// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/StrokerTest.cpp (chrome/m156)

#![cfg(test)]

// skia-rust: the `#if defined(SK_DEBUG) && QUAD_STROKE_APPROX_EXTENDED_DEBUGGING` blocks of the
// C++ are compiled out (QUAD_STROKE_APPROX_EXTENDED_DEBUGGING is 0, it enables a non-thread-safe
// global), so they are not ported.

use super::path_ops_cubic_intersection_test_data as cubic_data;
use super::path_ops_quad_intersection_test_data as quad_data;
use super::path_ops_test_common::{CubicPts, QuadPts};
use crate::def_test;
use skia_rust_core::float_bits::bits_to_float;
use skia_rust_core::paint::Paint;
use skia_rust_core::paint::Style;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_utils::fill_path_with_paint_to_path;
use skia_rust_core::point::{Point, point_priv};
use skia_rust_core::random::Random;
use skia_rust_core::scalar::{double_to_scalar, scalar};
use std::time::Instant;

// Port of: tests/Test.h#L236-L255 (chrome/m156), `skiatest::Timer`
struct Timer {
    start: Instant,
}

impl Timer {
    fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Milliseconds since creation.
    fn elapsed_ms(&self) -> f64 {
        self.start.elapsed().as_secs_f64() * 1e3
    }
}

// Port of: tests/StrokerTest.cpp#L35 (chrome/m156)
// `static DEFINE_bool(timeout, true, "run until alloted time expires");`
const FLAGS_TIMEOUT: bool = true;

// Port of: tests/StrokerTest.cpp#L37 (chrome/m156)
const MS_TEST_DURATION: f64 = 10.0;

// Port of: tests/StrokerTest.cpp#L39-L42 (chrome/m156)
const WIDTHS: [scalar; 32] = [
    -f32::MAX,
    -1.0,
    -0.1,
    -f32::EPSILON,
    0.0,
    f32::EPSILON,
    0.000_000_1,
    0.000_001,
    0.000_01,
    0.000_1,
    0.001,
    0.01,
    0.1,
    0.2,
    0.3,
    0.4,
    0.5,
    1.0,
    1.1,
    2.0,
    10.0,
    10e2,
    10e3,
    10e4,
    10e5,
    10e6,
    10e7,
    10e8,
    10e9,
    10e10,
    10e20,
    f32::MAX,
];

// Port of: tests/StrokerTest.cpp#L45-L52 (chrome/m156)
fn path_test(path: &Path) {
    let mut p = Paint::default();
    p.set_style(Style::Stroke);
    for width in WIDTHS {
        p.set_stroke_width(width);
        let _ = fill_path_with_paint_to_path(path, &p).0;
    }
}

// Port of: tests/StrokerTest.cpp#L54-L61 (chrome/m156)
fn cubic_test(c: &[Point; 4]) {
    let path = PathBuilder::new()
        .move_to(c[0])
        .cubic_to(c[1], c[2], c[3])
        .detach();
    path_test(&path);
}

// Port of: tests/StrokerTest.cpp#L63-L69 (chrome/m156)
fn quad_test(c: &[Point; 3]) {
    let path = PathBuilder::new()
        .move_to(c[0])
        .quad_to(c[1], c[2])
        .detach();
    path_test(&path);
}

// skia-rust: `SkDCubic::debugSet` / `SkDQuad::debugSet` (PathOps) only copy the points; the
// double to float conversions are `(float) d[i].fX`.
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ `(float)` casts of doubles
fn cubic_points(d_pts: &CubicPts) -> [Point; 4] {
    let d = &d_pts.pts;
    [
        Point::new(d[0].x as f32, d[0].y as f32),
        Point::new(d[1].x as f32, d[1].y as f32),
        Point::new(d[2].x as f32, d[2].y as f32),
        Point::new(d[3].x as f32, d[3].y as f32),
    ]
}

#[allow(clippy::cast_possible_truncation)] // mirrors the C++ `(float)` casts of doubles
fn quad_points(d_pts: &QuadPts) -> [Point; 3] {
    let d = &d_pts.pts;
    [
        Point::new(d[0].x as f32, d[0].y as f32),
        Point::new(d[1].x as f32, d[1].y as f32),
        Point::new(d[2].x as f32, d[2].y as f32),
    ]
}

// Port of: tests/StrokerTest.cpp#L71-L83 (chrome/m156)
fn cubic_set_test(d_cubic: &[CubicPts]) {
    let timer = Timer::new();
    for d_pts in d_cubic {
        let c = cubic_points(d_pts);
        cubic_test(&c);
        if FLAGS_TIMEOUT && timer.elapsed_ms() > MS_TEST_DURATION {
            return;
        }
    }
}

// Port of: tests/StrokerTest.cpp#L85-L99 (chrome/m156)
fn cubic_pair_set_test(d_cubic: &[[CubicPts; 2]]) {
    let timer = Timer::new();
    for pair_pts in d_cubic {
        for d_pts in pair_pts {
            let c = cubic_points(d_pts);
            cubic_test(&c);
            if FLAGS_TIMEOUT && timer.elapsed_ms() > MS_TEST_DURATION {
                return;
            }
        }
    }
}

// Port of: tests/StrokerTest.cpp#L101-L113 (chrome/m156)
fn quad_set_test(d_quad: &[QuadPts]) {
    let timer = Timer::new();
    for d_pts in d_quad {
        let c = quad_points(d_pts);
        quad_test(&c);
        if FLAGS_TIMEOUT && timer.elapsed_ms() > MS_TEST_DURATION {
            return;
        }
    }
}

// Port of: tests/StrokerTest.cpp#L115-L129 (chrome/m156)
fn quad_pair_set_test(d_quad: &[[QuadPts; 2]]) {
    let timer = Timer::new();
    for pair_pts in d_quad {
        for d_pts in pair_pts {
            let c = quad_points(d_pts);
            quad_test(&c);
            if FLAGS_TIMEOUT && timer.elapsed_ms() > MS_TEST_DURATION {
                return;
            }
        }
    }
}

// Port of: tests/StrokerTest.cpp#L131-L136 (chrome/m156)
def_test!(QuadStrokerSet, |_reporter| {
    quad_set_test(&quad_data::QUADRATICLINES);
    quad_set_test(&quad_data::QUADRATICPOINTS);
    quad_set_test(&quad_data::QUADRATICMODEPSILONLINES);
    quad_pair_set_test(&quad_data::QUADRATICTESTS);
});

// Port of: tests/StrokerTest.cpp#L138-L148 (chrome/m156)
def_test!(CubicStrokerSet, |_reporter| {
    cubic_set_test(&cubic_data::POINTDEGENERATES);
    cubic_set_test(&cubic_data::NOTPOINTDEGENERATES);
    cubic_set_test(&cubic_data::LINES);
    cubic_set_test(&cubic_data::NOTLINES);
    cubic_set_test(&cubic_data::MODEPSILONLINES);
    cubic_set_test(&cubic_data::LESSEPSILONLINES);
    cubic_set_test(&cubic_data::NEGEPSILONLINES);
    cubic_pair_set_test(&cubic_data::TESTS);
});

// Port of: tests/StrokerTest.cpp#L150-L153 (chrome/m156)
fn unbounded(r: &mut Random) -> scalar {
    let val = r.next_u();
    bits_to_float(val)
}

// Port of: tests/StrokerTest.cpp#L155-L158 (chrome/m156)
fn unbounded_pos(r: &mut Random) -> scalar {
    let val = r.next_u() & 0x7fff_ffff;
    bits_to_float(val)
}

// Port of: tests/StrokerTest.cpp#L160-L193 (chrome/m156)
def_test!(QuadStrokerUnbounded, |_reporter| {
    let mut r = Random::default();
    let mut p = Paint::default();
    p.set_style(Style::Stroke);
    let timer = Timer::new();
    for _ in 0..1_000_000 {
        let path = PathBuilder::new()
            .move_to((unbounded(&mut r), unbounded(&mut r)))
            .quad_to(
                (unbounded(&mut r), unbounded(&mut r)),
                (unbounded(&mut r), unbounded(&mut r)),
            )
            .detach();
        p.set_stroke_width(unbounded_pos(&mut r));
        let _fill = fill_path_with_paint_to_path(&path, &p).0;
        if FLAGS_TIMEOUT && timer.elapsed_ms() > MS_TEST_DURATION {
            return;
        }
    }
});

// Port of: tests/StrokerTest.cpp#L195-L231 (chrome/m156)
def_test!(CubicStrokerUnbounded, |_reporter| {
    let mut r = Random::default();
    let mut p = Paint::default();
    p.set_style(Style::Stroke);
    let timer = Timer::new();
    for _ in 0..1_000_000 {
        let path = PathBuilder::new()
            .move_to((unbounded(&mut r), unbounded(&mut r)))
            .cubic_to(
                (unbounded(&mut r), unbounded(&mut r)),
                (unbounded(&mut r), unbounded(&mut r)),
                (unbounded(&mut r), unbounded(&mut r)),
            )
            .detach();
        p.set_stroke_width(unbounded_pos(&mut r));
        let _fill = fill_path_with_paint_to_path(&path, &p).0;
        if FLAGS_TIMEOUT && timer.elapsed_ms() > MS_TEST_DURATION {
            return;
        }
    }
});

// Port of: tests/StrokerTest.cpp#L233-L281 (chrome/m156)
def_test!(QuadStrokerConstrained, |_reporter| {
    let mut r = Random::default();
    let mut p = Paint::default();
    p.set_style(Style::Stroke);
    let timer = Timer::new();
    for _ in 0..1_000_000 {
        let mut quad = [Point::default(); 3];
        quad[0].x = r.next_range_f(0.0, 500.0);
        quad[0].y = r.next_range_f(0.0, 500.0);
        let half_squared: scalar = 0.5 * 0.5;
        loop {
            quad[1].x = r.next_range_f(0.0, 500.0);
            quad[1].y = r.next_range_f(0.0, 500.0);
            if point_priv::distance_to_sqd(quad[0], quad[1]) >= half_squared {
                break;
            }
        }
        loop {
            quad[2].x = r.next_range_f(0.0, 500.0);
            quad[2].y = r.next_range_f(0.0, 500.0);
            if !(point_priv::distance_to_sqd(quad[0], quad[2]) < half_squared
                || point_priv::distance_to_sqd(quad[1], quad[2]) < half_squared)
            {
                break;
            }
        }
        let path = PathBuilder::new()
            .move_to(quad[0])
            .quad_to(quad[1], quad[2])
            .detach();
        p.set_stroke_width(r.next_range_f(0.0, 500.0));
        let _fill = fill_path_with_paint_to_path(&path, &p).0;
        if FLAGS_TIMEOUT && timer.elapsed_ms() > MS_TEST_DURATION {
            return;
        }
    }
});

// Port of: tests/StrokerTest.cpp#L283-L348 (chrome/m156)
def_test!(CubicStrokerConstrained, |_reporter| {
    let mut r = Random::default();
    let mut p = Paint::default();
    p.set_style(Style::Stroke);
    let timer = Timer::new();
    for _ in 0..1_000_000 {
        let mut cubic = [Point::default(); 4];
        cubic[0].x = r.next_range_f(0.0, 500.0);
        cubic[0].y = r.next_range_f(0.0, 500.0);
        let half_squared: scalar = 0.5 * 0.5;
        loop {
            cubic[1].x = r.next_range_f(0.0, 500.0);
            cubic[1].y = r.next_range_f(0.0, 500.0);
            if point_priv::distance_to_sqd(cubic[0], cubic[1]) >= half_squared {
                break;
            }
        }
        loop {
            cubic[2].x = r.next_range_f(0.0, 500.0);
            cubic[2].y = r.next_range_f(0.0, 500.0);
            if !(point_priv::distance_to_sqd(cubic[0], cubic[2]) < half_squared
                || point_priv::distance_to_sqd(cubic[1], cubic[2]) < half_squared)
            {
                break;
            }
        }
        loop {
            cubic[3].x = r.next_range_f(0.0, 500.0);
            cubic[3].y = r.next_range_f(0.0, 500.0);
            if !(point_priv::distance_to_sqd(cubic[0], cubic[3]) < half_squared
                || point_priv::distance_to_sqd(cubic[1], cubic[3]) < half_squared
                || point_priv::distance_to_sqd(cubic[2], cubic[3]) < half_squared)
            {
                break;
            }
        }
        let path = PathBuilder::new()
            .move_to(cubic[0])
            .cubic_to(cubic[1], cubic[2], cubic[3])
            .detach();
        p.set_stroke_width(r.next_range_f(0.0, 500.0));
        let _fill = fill_path_with_paint_to_path(&path, &p).0;
        if FLAGS_TIMEOUT && timer.elapsed_ms() > MS_TEST_DURATION {
            return;
        }
    }
});

// Port of: tests/StrokerTest.cpp#L350-L386 (chrome/m156)
def_test!(QuadStrokerRange, |_reporter| {
    let mut r = Random::default();
    let mut p = Paint::default();
    p.set_style(Style::Stroke);
    let timer = Timer::new();
    for _ in 0..1_000_000 {
        let mut quad = [Point::default(); 3];
        quad[0].x = r.next_range_f(0.0, 500.0);
        quad[0].y = r.next_range_f(0.0, 500.0);
        quad[1].x = r.next_range_f(0.0, 500.0);
        quad[1].y = r.next_range_f(0.0, 500.0);
        quad[2].x = r.next_range_f(0.0, 500.0);
        quad[2].y = r.next_range_f(0.0, 500.0);
        let path = PathBuilder::new()
            .move_to(quad[0])
            .quad_to(quad[1], quad[2])
            .detach();
        p.set_stroke_width(r.next_range_f(0.0, 500.0));
        let _fill = fill_path_with_paint_to_path(&path, &p).0;
        if FLAGS_TIMEOUT && timer.elapsed_ms() > MS_TEST_DURATION {
            return;
        }
    }
});

// Port of: tests/StrokerTest.cpp#L388-L428 (chrome/m156)
def_test!(CubicStrokerRange, |_reporter| {
    let mut r = Random::default();
    let mut p = Paint::default();
    p.set_style(Style::Stroke);
    let timer = Timer::new();
    for _ in 0..1_000_000 {
        let path = PathBuilder::new()
            .move_to((r.next_range_f(0.0, 500.0), r.next_range_f(0.0, 500.0)))
            .cubic_to(
                (r.next_range_f(0.0, 500.0), r.next_range_f(0.0, 500.0)),
                (r.next_range_f(0.0, 500.0), r.next_range_f(0.0, 500.0)),
                (r.next_range_f(0.0, 500.0), r.next_range_f(0.0, 500.0)),
            )
            .detach();
        p.set_stroke_width(r.next_range_f(0.0, 100.0));
        let _fill = fill_path_with_paint_to_path(&path, &p).0;
        if FLAGS_TIMEOUT && timer.elapsed_ms() > MS_TEST_DURATION {
            return;
        }
    }
});

// Port of: tests/StrokerTest.cpp#L431-L456 (chrome/m156)
def_test!(QuadStrokerOneOff, |reporter| {
    let b = bits_to_float;
    let mut p = Paint::default();
    p.set_style(Style::Stroke);
    p.set_stroke_width(double_to_scalar(164.683_548));

    let path = PathBuilder::new()
        .move_to((b(0x43c9_9223), b(0x42b7_417e)))
        .quad_to(
            (b(0x4285_d839), b(0x43ed_6645)),
            (b(0x43c9_41c8), b(0x42b3_ace3)),
        )
        .detach();
    let fill = fill_path_with_paint_to_path(&path, &p).0;
    if reporter.verbose() {
        println!("\nQuadStrokerOneOff path");
        path.dump();
        println!("fill:");
        fill.dump();
    }
});

// Port of: tests/StrokerTest.cpp#L458-L494 (chrome/m156)
def_test!(CubicStrokerOneOff, |reporter| {
    let b = bits_to_float;
    let mut p = Paint::default();
    p.set_style(Style::Stroke);
    p.set_stroke_width(double_to_scalar(42.835_968));

    let path = PathBuilder::new()
        .move_to((b(0x433f_5370), b(0x43d1_f4b3)))
        .cubic_to(
            (b(0x4331_cb76), b(0x43ea_3340)),
            (b(0x4388_f498), b(0x42f7_f08d)),
            (b(0x43f1_cd32), b(0x4280_2ec1)),
        )
        .detach();
    let fill = fill_path_with_paint_to_path(&path, &p).0;
    if reporter.verbose() {
        println!("\nCubicStrokerOneOff path");
        path.dump();
        println!("fill:");
        fill.dump();
    }
});
