// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsCurve.h (the CurveXxx dispatch tables and the static
// helpers behind them, SkDCurve, SkDCurveSweep), src/pathops/SkPathOpsCurve.cpp.

//! The verb dispatch used by the op graph: a segment's points and verb select a line, quad,
//! conic or cubic routine. Skia does this with function-pointer tables indexed by verb; here a
//! `match` on the verb calls the same routines.

use skia_rust_core::path::Verb;
use skia_rust_core::point::Point;

use crate::conic::DConic;
use crate::cubic::DCubic;
use crate::intersections::Intersections;
use crate::line::DLine;
use crate::point::{DPoint, DVector};
use crate::quad::DQuad;
use crate::rect::{Bounds, DRect};
use crate::types::{
    almost_between_ulps, almost_equal_ulps, almost_equal_ulps_pin, pin_t, roughly_zero_when_compared_to,
    std_max, std_min,
};

/// `SkPathOpsVerbToPoints(verb)`: the number of control points after the start point.
// Port of: src/pathops/SkPathOpsTypes.h (SkPathOpsVerbToPoints) (chrome/m156)
#[must_use]
pub(crate) fn verb_points(verb: Verb) -> usize {
    crate::types::verb_to_points(verb) as usize
}

/// `SkDCurve`: the points of a curve of any verb, with a conic weight. Skia's union is
/// read by verb, so the points are stored once and each verb views the prefix it needs.
// Port of: src/pathops/SkPathOpsCurve.h#L17-L50 (chrome/m156)
#[doc(alias = "SkDCurve")]
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct DCurveBuf {
    /// The four points (`fCubic`); a line uses 0..2, a quad 0..3, a conic 0..3.
    pub(crate) pts: [DPoint; 4],
    /// `fConic.fWeight`.
    pub(crate) weight: f32,
}

impl DCurveBuf {
    /// `SkDCurve::fLine`.
    #[must_use]
    pub(crate) fn line(&self) -> DLine {
        DLine::new([self.pts[0], self.pts[1]])
    }

    /// `SkDCurve::fQuad`.
    #[must_use]
    pub(crate) fn quad(&self) -> DQuad {
        DQuad {
            pts: [self.pts[0], self.pts[1], self.pts[2]],
        }
    }

    /// `SkDCurve::fConic`.
    #[must_use]
    pub(crate) fn conic(&self) -> DConic {
        DConic {
            pts: self.quad(),
            weight: self.weight,
        }
    }

    /// `SkDCurve::fCubic`.
    #[must_use]
    pub(crate) fn cubic(&self) -> DCubic {
        DCubic { pts: self.pts }
    }

    /// `SkDCurve::operator[]`.
    #[must_use]
    pub(crate) fn at(&self, n: usize) -> DPoint {
        self.pts[n]
    }

    /// `SkDCurve::nearPoint(verb, xy, opp)`.
    // Port of: src/pathops/SkPathOpsCurve.cpp#L11-L56 (chrome/m156)
    #[must_use]
    pub(crate) fn near_point(&self, verb: Verb, xy: DPoint, opp: DPoint) -> f64 {
        let count = verb_points(verb);
        let mut min_x = self.pts[0].x;
        let mut max_x = min_x;
        for index in 1..=count {
            min_x = std_min(min_x, self.pts[index].x);
            max_x = std_max(max_x, self.pts[index].x);
        }
        if !almost_between_ulps(min_x, xy.x, max_x) {
            return -1.0;
        }
        let mut min_y = self.pts[0].y;
        let mut max_y = min_y;
        for index in 1..=count {
            min_y = std_min(min_y, self.pts[index].y);
            max_y = std_max(max_y, self.pts[index].y);
        }
        if !almost_between_ulps(min_y, xy.y, max_y) {
            return -1.0;
        }
        let mut i = Intersections::default();
        let perp = DLine::new([
            xy,
            DPoint::new(xy.x + opp.y - xy.y, xy.y + xy.x - opp.x),
        ]);
        curve_d_intersect_ray(verb, self, &perp, &mut i);
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
        let min_index = min_index as usize;
        let largest = std_max(std_max(max_x, max_y), -std_min(min_x, min_y));
        if !almost_equal_ulps_pin(largest, largest + min_dist) {
            // is distance within ULPS tolerance?
            return -1.0;
        }
        pin_t(i.t(0, min_index))
    }

    /// `SkDCurve::setConicBounds(curve, weight, tStart, tEnd, bounds)`.
    // Port of: src/pathops/SkPathOpsCurve.cpp#L58-L68 (chrome/m156)
    pub(crate) fn set_conic_bounds(
        &self,
        curve: [Point; 3],
        curve_weight: f32,
        t_start: f64,
        t_end: f64,
        bounds: &mut Bounds,
    ) {
        let mut d_curve = DConic::default();
        d_curve.set(curve, curve_weight);
        let mut d_rect = DRect::default();
        d_rect.set_bounds_conic_sub(&d_curve, &self.conic(), t_start, t_end);
        set_ltrb(bounds, &d_rect);
    }

    /// `SkDCurve::setCubicBounds(curve, weight, tStart, tEnd, bounds)`.
    // Port of: src/pathops/SkPathOpsCurve.cpp#L70-L78 (chrome/m156)
    pub(crate) fn set_cubic_bounds(
        &self,
        curve: [Point; 4],
        _curve_weight: f32,
        t_start: f64,
        t_end: f64,
        bounds: &mut Bounds,
    ) {
        let mut d_curve = DCubic::default();
        d_curve.set(curve);
        let mut d_rect = DRect::default();
        d_rect.set_bounds_cubic_sub(&d_curve, &self.cubic(), t_start, t_end);
        set_ltrb(bounds, &d_rect);
    }

    /// `SkDCurve::setQuadBounds(curve, weight, tStart, tEnd, bounds)`.
    // Port of: src/pathops/SkPathOpsCurve.cpp#L80-L88 (chrome/m156)
    pub(crate) fn set_quad_bounds(
        &self,
        curve: [Point; 3],
        _curve_weight: f32,
        t_start: f64,
        t_end: f64,
        bounds: &mut Bounds,
    ) {
        let mut d_curve = DQuad::default();
        d_curve.set(curve);
        let mut d_rect = DRect::default();
        d_rect.set_bounds_quad_sub(&d_curve, &self.quad(), t_start, t_end);
        set_ltrb(bounds, &d_rect);
    }
}

/// `bounds->setLTRB(SkDoubleToScalar(dRect.fLeft), ...)`.
fn set_ltrb(bounds: &mut Bounds, d_rect: &DRect) {
    bounds.left = d_rect.left as f32;
    bounds.top = d_rect.top as f32;
    bounds.right = d_rect.right as f32;
    bounds.bottom = d_rect.bottom as f32;
}

/// `SkDCurveSweep`: a curve with the vectors that bound its convex hull.
// Port of: src/pathops/SkPathOpsCurve.h#L93-L105 (chrome/m156)
#[doc(alias = "SkDCurveSweep")]
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct DCurveSweep {
    /// `SkDCurve fCurve`.
    pub(crate) curve: DCurveBuf,
    /// `SkDVector fSweep[2]`.
    pub(crate) sweep: [DVector; 2],
    /// `bool fIsCurve`.
    pub(crate) is_curve: bool,
    /// `bool fOrdered`: cleared when a cubic's control point isn't between the sweep vectors.
    pub(crate) ordered: bool,
}

impl DCurveSweep {
    /// `SkDCurveSweep::setCurveHullSweep(verb)`.
    // Port of: src/pathops/SkPathOpsCurve.cpp#L90-L133 (chrome/m156)
    pub(crate) fn set_curve_hull_sweep(&mut self, verb: Verb) {
        self.ordered = true;
        self.sweep[0] = self.curve.pts[1] - self.curve.pts[0];
        if verb == Verb::Line {
            self.sweep[1] = self.sweep[0];
            self.is_curve = false;
            return;
        }
        self.sweep[1] = self.curve.pts[2] - self.curve.pts[0];
        let mut max_val = 0.0_f64;
        for index in 0..=verb_points(verb) {
            max_val = std_max(
                max_val,
                std_max(self.curve.pts[index].x.abs(), self.curve.pts[index].y.abs()),
            );
        }
        'set_is_curve: {
            if verb != Verb::Cubic {
                if roughly_zero_when_compared_to(self.sweep[0].x, max_val)
                    && roughly_zero_when_compared_to(self.sweep[0].y, max_val)
                {
                    self.sweep[0] = self.sweep[1];
                }
                break 'set_is_curve;
            }
            let third_sweep = self.curve.pts[3] - self.curve.pts[0];
            if self.sweep[0].x == 0.0 && self.sweep[0].y == 0.0 {
                self.sweep[0] = self.sweep[1];
                self.sweep[1] = third_sweep;
                if roughly_zero_when_compared_to(self.sweep[0].x, max_val)
                    && roughly_zero_when_compared_to(self.sweep[0].y, max_val)
                {
                    self.sweep[0] = self.sweep[1];
                    self.curve.pts[1] = self.curve.pts[3];
                }
                break 'set_is_curve;
            }
            let s1x3 = self.sweep[0].cross_check(third_sweep);
            let s3x2 = third_sweep.cross_check(self.sweep[1]);
            if s1x3 * s3x2 >= 0.0 {
                // if third vector is on or between first two vectors
                break 'set_is_curve;
            }
            let s2x1 = self.sweep[1].cross_check(self.sweep[0]);
            if s3x2 * s2x1 < 0.0 {
                self.sweep[0] = self.sweep[1];
                self.ordered = false;
            }
            self.sweep[1] = third_sweep;
        }
        self.is_curve = self.sweep[0].cross_check(self.sweep[1]) != 0.0;
    }
}

/// `CurveDPointAtT[verb](pts, weight, t)`: the point at `t` on a curve given by points.
// Port of: src/pathops/SkPathOpsCurve.h#L110-L140 (chrome/m156)
#[must_use]
pub(crate) fn curve_d_point_at_t(verb: Verb, pts: &[Point], weight: f32, t: f64) -> DPoint {
    match verb {
        Verb::Line => DLine::new([DPoint::from_sk_point(pts[0]), DPoint::from_sk_point(pts[1])])
            .pt_at_t(t),
        Verb::Quad => quad_from(pts).pt_at_t(t),
        Verb::Conic => conic_from(pts, weight).pt_at_t(t),
        Verb::Cubic => cubic_from(pts).pt_at_t(t),
        _ => unreachable!("curve verb"),
    }
}

/// `CurvePointAtT[verb](pts, weight, t)`: the `SkPoint` at `t`.
// Port of: src/pathops/SkPathOpsCurve.h#L140-L170 (chrome/m156)
#[must_use]
pub(crate) fn curve_point_at_t(verb: Verb, pts: &[Point], weight: f32, t: f64) -> Point {
    curve_d_point_at_t(verb, pts, weight, t).as_sk_point()
}

/// `CurveDDPointAtT[verb](curve, t)`.
// Port of: src/pathops/SkPathOpsCurve.h#L180-L215 (chrome/m156)
#[must_use]
pub(crate) fn curve_dd_point_at_t(verb: Verb, curve: &DCurveBuf, t: f64) -> DPoint {
    match verb {
        Verb::Line => curve.line().pt_at_t(t),
        Verb::Quad => curve.quad().pt_at_t(t),
        Verb::Conic => curve.conic().pt_at_t(t),
        Verb::Cubic => curve.cubic().pt_at_t(t),
        _ => unreachable!("curve verb"),
    }
}

/// `CurveDSlopeAtT[verb](pts, weight, t)`: the derivative at `t`.
// Port of: src/pathops/SkPathOpsCurve.h#L172-L200 (chrome/m156)
#[must_use]
pub(crate) fn curve_d_slope_at_t(verb: Verb, pts: &[Point], weight: f32, t: f64) -> DVector {
    match verb {
        Verb::Line => DPoint::from_sk_point(pts[1]) - DPoint::from_sk_point(pts[0]),
        Verb::Quad => quad_from(pts).dxdy_at_t(t),
        Verb::Conic => conic_from(pts, weight).dxdy_at_t(t),
        Verb::Cubic => cubic_from(pts).dxdy_at_t(t),
        _ => unreachable!("curve verb"),
    }
}

/// `CurveDDSlopeAtT[verb](curve, t)`.
// Port of: src/pathops/SkPathOpsCurve.h#L240-L270 (chrome/m156)
#[must_use]
pub(crate) fn curve_dd_slope_at_t(verb: Verb, curve: &DCurveBuf, t: f64) -> DVector {
    match verb {
        Verb::Line => curve.pts[1] - curve.pts[0],
        Verb::Quad => curve.quad().dxdy_at_t(t),
        Verb::Conic => curve.conic().dxdy_at_t(t),
        Verb::Cubic => curve.cubic().dxdy_at_t(t),
        _ => unreachable!("curve verb"),
    }
}

/// `CurveSlopeAtT[verb](pts, weight, t)`: the derivative as an `SkVector`.
// Port of: src/pathops/SkPathOpsCurve.h#L272-L297 (chrome/m156)
#[must_use]
pub(crate) fn curve_slope_at_t(verb: Verb, pts: &[Point], weight: f32, t: f64) -> Point {
    match verb {
        Verb::Line => pts[1] - pts[0],
        _ => curve_d_slope_at_t(verb, pts, weight, t).as_sk_vector(),
    }
}

/// `CurveIsVertical[verb](pts, weight, startT, endT)`.
// Port of: src/pathops/SkPathOpsCurve.h#L299-L355 (chrome/m156)
#[must_use]
pub(crate) fn curve_is_vertical(verb: Verb, pts: &[Point], weight: f32, start_t: f64, end_t: f64) -> bool {
    match verb {
        Verb::Line => {
            let line = DLine::new([DPoint::from_sk_point(pts[0]), DPoint::from_sk_point(pts[1])]);
            let dst = [line.pt_at_t(start_t), line.pt_at_t(end_t)];
            almost_equal_ulps(dst[0].x, dst[1].x)
        }
        Verb::Quad => {
            let dst = quad_from(pts).sub_divide(start_t, end_t);
            almost_equal_ulps(dst.pts[0].x, dst.pts[1].x)
                && almost_equal_ulps(dst.pts[1].x, dst.pts[2].x)
        }
        Verb::Conic => {
            let dst = conic_from(pts, weight).sub_divide(start_t, end_t);
            almost_equal_ulps(dst.pts.pts[0].x, dst.pts.pts[1].x)
                && almost_equal_ulps(dst.pts.pts[1].x, dst.pts.pts[2].x)
        }
        Verb::Cubic => {
            let dst = cubic_from(pts).sub_divide(start_t, end_t);
            almost_equal_ulps(dst.pts[0].x, dst.pts[1].x)
                && almost_equal_ulps(dst.pts[1].x, dst.pts[2].x)
                && almost_equal_ulps(dst.pts[2].x, dst.pts[3].x)
        }
        _ => unreachable!("curve verb"),
    }
}

/// `CurveIntersectRay[verb](pts, weight, ray, i)`.
// Port of: src/pathops/SkPathOpsCurve.h#L357-L410 (chrome/m156)
pub(crate) fn curve_intersect_ray(
    verb: Verb,
    pts: &[Point],
    weight: f32,
    ray: &DLine,
    i: &mut Intersections,
) {
    match verb {
        Verb::Line => {
            let line = DLine::new([DPoint::from_sk_point(pts[0]), DPoint::from_sk_point(pts[1])]);
            i.intersect_ray_line(&line, ray);
        }
        Verb::Quad => {
            i.intersect_ray_quad(&quad_from(pts), ray);
        }
        Verb::Conic => {
            i.intersect_ray_conic(&conic_from(pts, weight), ray);
        }
        Verb::Cubic => {
            i.intersect_ray_cubic(&cubic_from(pts), ray);
        }
        _ => unreachable!("curve verb"),
    }
}

/// `CurveDIntersectRay[verb](curve, ray, i)`.
// Port of: src/pathops/SkPathOpsCurve.h#L412-L440 (chrome/m156)
pub(crate) fn curve_d_intersect_ray(verb: Verb, curve: &DCurveBuf, ray: &DLine, i: &mut Intersections) {
    match verb {
        Verb::Line => {
            i.intersect_ray_line(&curve.line(), ray);
        }
        Verb::Quad => {
            i.intersect_ray_quad(&curve.quad(), ray);
        }
        Verb::Conic => {
            i.intersect_ray_conic(&curve.conic(), ray);
        }
        Verb::Cubic => {
            i.intersect_ray_cubic(&curve.cubic(), ray);
        }
        _ => unreachable!("curve verb"),
    }
}

/// `SkDQuad::set(pts)`.
#[must_use]
fn quad_from(pts: &[Point]) -> DQuad {
    let mut quad = DQuad::default();
    quad.set([pts[0], pts[1], pts[2]]);
    quad
}

/// `SkDConic::set(pts, weight)`.
#[must_use]
fn conic_from(pts: &[Point], weight: f32) -> DConic {
    let mut conic = DConic::default();
    conic.set([pts[0], pts[1], pts[2]], weight);
    conic
}

/// `SkDCubic::set(pts)`.
#[must_use]
fn cubic_from(pts: &[Point]) -> DCubic {
    let mut cubic = DCubic::default();
    cubic.set([pts[0], pts[1], pts[2], pts[3]]);
    cubic
}
