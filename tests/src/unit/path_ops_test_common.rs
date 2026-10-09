// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PathOpsTestCommon.h (chrome/m156)

#![cfg(test)]

use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathVerb;
use skia_rust_pathops::conic::DConic;
use skia_rust_pathops::cubic::DCubic;
/// `SkDPoint`, as used by the `PathOps` test data (the shared `skia-rust-pathops` type).
pub use skia_rust_pathops::point::DPoint;
use skia_rust_pathops::quad::DQuad;
use skia_rust_pathops::rect::DRect;
use skia_rust_pathops::reduce_order::{Quadratics, ReduceOrder};
use skia_rust_pathops::types::{
    approximately_equal, approximately_greater_than_one, approximately_less_than_zero,
};

/// `QuadPts`.
#[derive(Copy, Clone, Debug)]
pub struct QuadPts {
    pub pts: [DPoint; 3],
}

impl QuadPts {
    #[must_use]
    pub const fn new(pts: [DPoint; 3]) -> Self {
        Self { pts }
    }
}

/// `CubicPts`.
#[derive(Copy, Clone, Debug)]
pub struct CubicPts {
    pub pts: [DPoint; 4],
}

impl CubicPts {
    #[must_use]
    pub const fn new(pts: [DPoint; 4]) -> Self {
        Self { pts }
    }
}

/// `ConicPts`: three control points and a weight.
#[derive(Copy, Clone, Debug)]
pub struct ConicPts {
    pub pts: QuadPts,
    pub weight: f32,
}

impl ConicPts {
    #[must_use]
    pub const fn new(pts: QuadPts, weight: f32) -> Self {
        Self { pts, weight }
    }
}

/// `ValidPoint(const SkDPoint& pt)`: neither coordinate is NaN.
// Port of: tests/PathOpsTestCommon.cpp#L307-L312 (chrome/m156)
#[must_use]
pub fn valid_point(pt: DPoint) -> bool {
    !pt.x.is_nan() && !pt.y.is_nan()
}

/// `ValidCubic(const SkDCubic& cubic)`: no control point is NaN.
// Port of: tests/PathOpsTestCommon.cpp#L289-L296 (chrome/m156)
#[must_use]
pub fn valid_cubic(cubic: &DCubic) -> bool {
    cubic.pts.iter().all(|pt| valid_point(*pt))
}

/// `ValidConic(const SkDConic& conic)`: no point and no weight is NaN.
// Port of: tests/PathOpsTestCommon.cpp#L277-L287 (chrome/m156)
#[must_use]
pub fn valid_conic(conic: &DConic) -> bool {
    conic.pts.pts.iter().all(|pt| valid_point(*pt)) && !conic.weight.is_nan()
}

/// `ValidQuad(const SkDQuad& quad)`.
// Port of: tests/PathOpsTestCommon.cpp#L326-L334 (chrome/m156)
#[must_use]
pub fn valid_quad(quad: &DQuad) -> bool {
    quad.pts.iter().all(|pt| valid_point(*pt))
}

/// `calc_t_div` of `PathOpsTestCommon.cpp`.
// Port of: tests/PathOpsTestCommon.cpp#L31-L52 (chrome/m156)
fn calc_t_div(cubic: &DCubic, precision: f64, start: f64) -> f64 {
    let adjust = 3.0_f64.sqrt() / 36.0;
    let sub;
    let c = if start == 0.0 {
        cubic
    } else {
        // OPTIMIZE: special-case half-split ?
        sub = cubic.sub_divide(start, 1.0);
        &sub
    };
    let p = &c.pts;
    let dx = p[3].x - 3.0 * (p[2].x - p[1].x) - p[0].x;
    let dy = p[3].y - 3.0 * (p[2].y - p[1].y) - p[0].y;
    let dist = (dx * dx + dy * dy).sqrt();
    let t_div3 = precision / (adjust * dist);
    let mut t = t_div3.cbrt();
    if start > 0.0 {
        t = start + (1.0 - start) * t;
    }
    t
}

/// `add_simple_ts` of `PathOpsTestCommon.cpp`.
// Port of: tests/PathOpsTestCommon.cpp#L54-L64 (chrome/m156)
fn add_simple_ts(cubic: &DCubic, precision: f64, ts: &mut Vec<f64>) -> bool {
    let t_div = calc_t_div(cubic, precision, 0.0);
    if t_div >= 1.0 {
        return true;
    }
    if t_div >= 0.5 {
        ts.push(0.5);
        return true;
    }
    false
}

/// `addTs` of `PathOpsTestCommon.cpp`.
// Port of: tests/PathOpsTestCommon.cpp#L66-L76 (chrome/m156)
fn add_ts(cubic: &DCubic, precision: f64, start: f64, end: f64, ts: &mut Vec<f64>) {
    let t_div = calc_t_div(cubic, precision, 0.0);
    let parts = (1.0 / t_div).ceil();
    let mut index = 0.0;
    while index < parts {
        let new_t = start + (index / parts) * (end - start);
        if new_t > 0.0 && new_t < 1.0 {
            ts.push(new_t);
        }
        index += 1.0;
    }
}

/// `toQuadraticTs` of `PathOpsTestCommon.cpp`.
// Port of: tests/PathOpsTestCommon.cpp#L78-L146 (chrome/m156)
fn to_quadratic_ts(cubic: &DCubic, precision: f64, ts: &mut Vec<f64>) {
    let mut reducer = ReduceOrder::default();
    let order = reducer.reduce_cubic(cubic, Quadratics::Allow);
    if order < 3 {
        return;
    }
    let mut infl_t = [0.0_f64; 5];
    let mut found = [0.0_f64; 3];
    let mut two = [0.0_f64; 2];
    let mut inflections = cubic.find_inflections(&mut two);
    infl_t[..inflections].copy_from_slice(&two[..inflections]);
    if !cubic.ends_are_extrema_in_x_or_y() {
        let extra = cubic.find_max_curvature(&mut found);
        infl_t[inflections..inflections + extra].copy_from_slice(&found[..extra]);
        inflections += extra;
    }
    infl_t[..inflections].sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    // OPTIMIZATION: is this filtering common enough that it needs to be pulled out into its
    // own subroutine?
    while inflections > 0 && approximately_less_than_zero(infl_t[0]) {
        // memmove(inflectT, &inflectT[1], sizeof(inflectT[0]) * --inflections)
        inflections -= 1;
        infl_t.copy_within(1..=inflections, 0);
    }
    let mut start = 0;
    let mut next = 1;
    while next < inflections {
        if !approximately_equal(infl_t[start], infl_t[next]) {
            start += 1;
            next += 1;
            continue;
        }
        // memmove(&inflectT[start], &inflectT[next], sizeof(inflectT[0]) * (--inflections - start))
        inflections -= 1;
        infl_t.copy_within(next..next + (inflections - start), start);
    }

    while inflections > 0 && approximately_greater_than_one(infl_t[inflections - 1]) {
        inflections -= 1;
    }
    if inflections == 1 {
        let pair = cubic.chop_at(infl_t[0]);
        let order_p1 = reducer.reduce_cubic(&pair.first(), Quadratics::No);
        if order_p1 < 2 {
            inflections -= 1;
        } else {
            let order_p2 = reducer.reduce_cubic(&pair.second(), Quadratics::No);
            if order_p2 < 2 {
                inflections -= 1;
            }
        }
    }
    if inflections == 0 && add_simple_ts(cubic, precision, ts) {
        return;
    }
    if inflections == 1 {
        let pair = cubic.chop_at(infl_t[0]);
        add_ts(&pair.first(), precision, 0.0, infl_t[0], ts);
        add_ts(&pair.second(), precision, infl_t[0], 1.0, ts);
        return;
    }
    if inflections > 1 {
        let mut part = cubic.sub_divide(0.0, infl_t[0]);
        add_ts(&part, precision, 0.0, infl_t[0], ts);
        let last = inflections - 1;
        for idx in 0..last {
            part = cubic.sub_divide(infl_t[idx], infl_t[idx + 1]);
            add_ts(&part, precision, infl_t[idx], infl_t[idx + 1], ts);
        }
        part = cubic.sub_divide(infl_t[last], 1.0);
        add_ts(&part, precision, infl_t[last], 1.0, ts);
        return;
    }
    add_ts(cubic, precision, 0.0, 1.0, ts);
}

/// `CubicToQuads` of `PathOpsTestCommon.cpp`.
// Port of: tests/PathOpsTestCommon.cpp#L148-L176 (chrome/m156)
fn cubic_to_quads(cubic: &DCubic, precision: f64, quads: &mut Vec<DQuad>) {
    let mut ts = Vec::new();
    to_quadratic_ts(cubic, precision, &mut ts);
    if ts.is_empty() {
        quads.push(cubic.to_quad());
        return;
    }
    let mut t_start = 0.0;
    for i1 in 0..=ts.len() {
        let t_end = if i1 < ts.len() { ts[i1] } else { 1.0 };
        let mut bounds = DRect::default();
        bounds.set_bounds_cubic(cubic);
        let part = cubic.sub_divide(t_start, t_end);
        let mut quad = part.to_quad();
        if quad.pts[1].x < bounds.left {
            quad.pts[1].x = bounds.left;
        } else if quad.pts[1].x > bounds.right {
            quad.pts[1].x = bounds.right;
        }
        if quad.pts[1].y < bounds.top {
            quad.pts[1].y = bounds.top;
        } else if quad.pts[1].y > bounds.bottom {
            quad.pts[1].y = bounds.bottom;
        }
        quads.push(quad);
        t_start = t_end;
    }
}

/// `CubicPathToQuads(const SkPath& cubicPath)`: replaces each cubic of the path with quads.
// Port of: tests/PathOpsTestCommon.cpp#L178-L215 (chrome/m156)
#[must_use]
pub fn cubic_path_to_quads(cubic_path: &Path) -> Path {
    let mut quad_builder = PathBuilder::new();
    for rec in cubic_path.iter() {
        let pts = rec.points();
        match rec.verb() {
            PathVerb::Move => {
                quad_builder.move_to(pts[0]);
            }
            PathVerb::Line => {
                quad_builder.line_to(pts[1]);
            }
            PathVerb::Quad => {
                quad_builder.quad_to(pts[1], pts[2]);
            }
            PathVerb::Cubic => {
                let mut quads = Vec::new();
                let cubic = DCubic::new([
                    DPoint::from(pts[0]),
                    DPoint::from(pts[1]),
                    DPoint::from(pts[2]),
                    DPoint::from(pts[3]),
                ]);
                cubic_to_quads(&cubic, cubic.calc_precision(), &mut quads);
                for quad in &quads {
                    quad_builder.quad_to(quad.pts[1].as_sk_point(), quad.pts[2].as_sk_point());
                }
            }
            PathVerb::Close => {
                quad_builder.close();
            }
            // `default: goto DONE` in Skia: conics and unknown verbs end the conversion.
            PathVerb::Conic => break,
        }
    }
    quad_builder.detach()
}
