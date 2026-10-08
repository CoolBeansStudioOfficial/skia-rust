// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsCurve.h (SkDCurve), src/pathops/SkPathOpsCurve.cpp

//! The curve sum type of `PathOps` (`SkDCurve`): a line, quad, conic or cubic, with the
//! `nearPoint` query used when matching end points.

use skia_rust_core::path::Verb;

use crate::conic::DConic;
use crate::cubic::DCubic;
use crate::intersections::Intersections;
use crate::line::DLine;
use crate::point::DPoint;
use crate::quad::DQuad;
use crate::types::{almost_between_ulps, almost_equal_ulps_pin, pin_t, std_max, std_min};

/// `SkDCurve`: one of the four curve kinds, as a Rust enum instead of a C++ union.
// Port of: src/pathops/SkPathOpsCurve.h (chrome/m156)
#[doc(alias = "SkDCurve")]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum DCurve {
    Line(DLine),
    Quad(DQuad),
    Conic(DConic),
    Cubic(DCubic),
}

impl DCurve {
    /// The control points, padded with the last point for curves with fewer than four.
    #[must_use]
    pub fn points(&self) -> [DPoint; 4] {
        match self {
            Self::Line(line) => [line.pts[0], line.pts[1], line.pts[1], line.pts[1]],
            Self::Quad(quad) => [quad.pts[0], quad.pts[1], quad.pts[2], quad.pts[2]],
            Self::Conic(conic) => [conic.pts[0], conic.pts[1], conic.pts[2], conic.pts[2]],
            Self::Cubic(cubic) => cubic.pts,
        }
    }

    /// `intersectRay(i, perp)` for the curve's kind: `CurveDIntersectRay[verb]`.
    // Port of: src/pathops/SkPathOpsCurve.cpp (chrome/m156)
    fn intersect_ray(&self, i: &mut Intersections, perp: &DLine) {
        match self {
            Self::Line(line) => {
                i.intersect_ray_line(line, perp);
            }
            Self::Quad(quad) => {
                i.intersect_ray_quad(quad, perp);
            }
            Self::Conic(conic) => {
                i.intersect_ray_conic(conic, perp);
            }
            Self::Cubic(cubic) => {
                i.intersect_ray_cubic(cubic, perp);
            }
        }
    }
}

/// `SkDCurve::nearPoint(SkPath::Verb verb, const SkDPoint& xy, const SkDPoint& opp) const`:
/// the t of the point on the curve nearest `xy` along the perpendicular through `xy`, or -1 when
/// no point is within ULPS tolerance.
// Port of: src/pathops/SkPathOpsCurve.cpp (chrome/m156)
#[doc(alias = "nearPoint")]
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
pub fn curve_near_point(curve: &DCurve, verb: Verb, xy: DPoint, opp: DPoint) -> f64 {
    let count = crate::types::verb_to_points(verb) as usize;
    let pts = curve.points();
    let mut min_x = pts[0].x;
    let mut max_x = min_x;
    for pt in &pts[1..=count] {
        min_x = std_min(min_x, pt.x);
        max_x = std_max(max_x, pt.x);
    }
    if !almost_between_ulps(min_x, xy.x, max_x) {
        return -1.0;
    }
    let mut min_y = pts[0].y;
    let mut max_y = min_y;
    for pt in &pts[1..=count] {
        min_y = std_min(min_y, pt.y);
        max_y = std_max(max_y, pt.y);
    }
    if !almost_between_ulps(min_y, xy.y, max_y) {
        return -1.0;
    }
    let mut i = Intersections::default();
    let perp = DLine::new([xy, DPoint::new(xy.x + opp.y - xy.y, xy.y + xy.x - opp.x)]);
    curve.intersect_ray(&mut i, &perp);
    let mut min_index: i32 = -1;
    let mut min_dist = f64::from(f32::MAX);
    for index in 0..i.used() {
        let dist = xy.distance(i.pt(index));
        if min_dist > dist {
            min_dist = dist;
            min_index = index as i32;
        }
    }
    if min_index < 0 {
        return -1.0;
    }
    let largest = std_max(std_max(max_x, max_y), -std_min(min_x, min_y));
    if !almost_equal_ulps_pin(largest, largest + min_dist) {
        // is distance within ULPS tolerance?
        return -1.0;
    }
    pin_t(i.t(0, min_index as usize))
}
