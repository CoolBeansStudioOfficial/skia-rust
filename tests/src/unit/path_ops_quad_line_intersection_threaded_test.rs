// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsQuadLineIntersectionThreadedTest.cpp (chrome/m156)

// The threaded runners are run single-threaded, in the order the C++ runnables are appended, with
// the same case enumeration. The verbose output of the generated test sources is not ported.
#![allow(
    clippy::cast_precision_loss,
    clippy::similar_names,
    clippy::many_single_char_names
)]
#![cfg(test)]

use skia_rust_pathops::intersections::Intersections;
use skia_rust_pathops::line::DLine;
use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::quad::DQuad;
use skia_rust_pathops::reduce_order::ReduceOrder;

use crate::unit::path_ops_quad_line_intersection_test::do_intersect;
use crate::unit::path_ops_test_common::QuadPts;
use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/PathOpsQuadLineIntersectionThreadedTest.cpp#L58-L72 (chrome/m156)
fn test_line_intersect(reporter: &mut Reporter, quad: &DQuad, line: &DLine, _x: f64, _y: f64) {
    let mut intersections = Intersections::default();
    let mut flipped = false;
    let result = do_intersect(&mut intersections, quad, line, &mut flipped);
    let mut found = false;
    for index in 0..result {
        let quad_t = intersections.t(0, index);
        let quad_xy: DPoint = quad.pt_at_t(quad_t);
        let line_t = intersections.t(1, index);
        let line_xy: DPoint = line.pt_at_t(line_t);
        if quad_xy.approximately_equal(line_xy) {
            found = true;
        }
    }
    reporter_assert!(reporter, found);
}

// Port of: tests/PathOpsQuadLineIntersectionThreadedTest.cpp#L74-L125 (chrome/m156)
// find a point on a quad by choosing a t from 0 to 1
// create a vertical span above and below the point
// verify that intersecting the vertical span and the quad returns t
// verify that a vertical span starting at quad[0] intersects at t=0
// verify that a vertical span starting at quad[2] intersects at t=1
fn test_quad_line_intersect_main(reporter: &mut Reporter, a: u8, b: u8, c: u8) {
    let (ax, ay) = (i32::from(a & 0x03), i32::from(a >> 2));
    let (bx, by) = (i32::from(b & 0x03), i32::from(b >> 2));
    let (cx, cy) = (i32::from(c & 0x03), i32::from(c >> 2));
    let q = QuadPts::new([
        DPoint::new(f64::from(ax), f64::from(ay)),
        DPoint::new(f64::from(bx), f64::from(by)),
        DPoint::new(f64::from(cx), f64::from(cy)),
    ]);
    let quad = DQuad::new(q.pts);
    let mut reducer = ReduceOrder::default();
    let order = reducer.reduce_quad(&quad);
    if order < 3 {
        return;
    }
    for t_index in 0..=4 {
        let xy = quad.pt_at_t(f64::from(t_index) / 4.0);
        for h in -2..=2i32 {
            for v in -2..=2i32 {
                if h == v && h.abs() != 1 {
                    continue;
                }
                let x = xy.x;
                let y = xy.y;
                let (fh, fv) = (f64::from(h), f64::from(v));
                let line = DLine::new([DPoint::new(x - fh, y - fv), DPoint::new(x, y)]);
                test_line_intersect(reporter, &quad, &line, x, y);
                reporter.bump_test_count();
                let line2 = DLine::new([DPoint::new(x, y), DPoint::new(x + fh, y + fv)]);
                test_line_intersect(reporter, &quad, &line2, x, y);
                reporter.bump_test_count();
                let line3 = DLine::new([DPoint::new(x - fh, y - fv), DPoint::new(x + fh, y + fv)]);
                test_line_intersect(reporter, &quad, &line3, x, y);
                reporter.bump_test_count();
            }
        }
    }
}

// Port of: tests/PathOpsQuadLineIntersectionThreadedTest.cpp#L127-L138 (chrome/m156)
def_test!(PathOpsQuadLineIntersectionThreaded, |reporter| {
    // Runnables are collected first, as the C++ appends them before render(). With
    // allowExtendedTest() false the enumeration stops after the first `b` iteration.
    let mut runnables = Vec::new();
    'finish: for a in 0..16u8 {
        for b in 0..16u8 {
            for c in 0..16u8 {
                runnables.push((a, b, c));
            }
            if !reporter.allow_extended_test() {
                break 'finish;
            }
        }
    }
    for (a, b, c) in runnables {
        test_quad_line_intersect_main(reporter, a, b, c);
    }
});
