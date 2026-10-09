// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/utils/SkPolyUtils.{h,cpp} (convex subset: winding, convexity, convex
// inset and radial steps, used by the shadow tessellator).

//! Polygon helpers (`SkPolyUtils`). Only the convex-polygon functions are ported so far; the
//! simple-polygon (concave) functions are not yet ported.

// The index and count casts here mirror the C++ int indices of the Skia code this file ports;
// every value stays inside the polygon or vertex count, which is bounded by u16::MAX for meshes.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::similar_names, // sNumer/tNumer etc. keep the C++ names
    clippy::many_single_char_names, // p0/p1/v0/v1 keep the C++ names for auditing the port
)]
use crate::floating_point::is_finite_all;
use crate::point::{Point, Vector, point_priv};
use crate::scalar::{SCALAR_MIN, SCALAR_NEARLY_ZERO, scalar, scalar_round_to_int};

/// Tolerance for merging the inset polygon's points (`kCleanupTolerance`).
// Port of: src/utils/SkPolyUtils.cpp#L477-L478 (chrome/m156)
const CLEANUP_TOLERANCE: scalar = 0.01;

/// `kCrossTolerance = SK_ScalarNearlyZero * SK_ScalarNearlyZero`.
// Port of: src/utils/SkPolyUtils.cpp#L29 (chrome/m156)
const CROSS_TOLERANCE: scalar = SCALAR_NEARLY_ZERO * SCALAR_NEARLY_ZERO;

/// `SkScalarNearlyZero(x, tol)`: `|x| <= tol`.
#[inline]
fn nearly_zero_tol(x: scalar, tol: scalar) -> bool {
    x.abs() <= tol
}

/// A segment with a start point and a (non-normalized) direction (`OffsetSegment`).
// Port of: src/utils/SkPolyUtils.cpp#L31-L34 (chrome/m156)
#[derive(Clone, Copy, Debug)]
struct OffsetSegment {
    p0: Point,
    v: Vector,
}

/// `compute_side`: which side of the directed line `(p0, v)` the point `p` is on, or 0.
// Port of: src/utils/SkPolyUtils.cpp#L47-L56 (chrome/m156)
fn compute_side(p0: Point, v: Vector, p: Point) -> i32 {
    let w = p - p0;
    let perp_dot = v.cross(w);
    if !nearly_zero_tol(perp_dot, CROSS_TOLERANCE) {
        return if perp_dot > 0.0 { 1 } else { -1 };
    }
    0
}

/// `SkGetPolygonWinding`: `1` for counter-clockwise, `-1` for clockwise, `0` if degenerate.
// Port of: src/utils/SkPolyUtils.cpp#L58-L77 (chrome/m156)
#[doc(alias = "SkGetPolygonWinding")]
#[must_use]
pub fn get_polygon_winding(polygon_verts: &[Point]) -> i32 {
    if polygon_verts.len() < 3 {
        return 0;
    }
    let mut quad_area: scalar = 0.0;
    let mut v0 = polygon_verts[1] - polygon_verts[0];
    for curr in 2..polygon_verts.len() {
        let v1 = polygon_verts[curr] - polygon_verts[0];
        quad_area += v0.cross(v1);
        v0 = v1;
    }
    if nearly_zero_tol(quad_area, CROSS_TOLERANCE) {
        return 0;
    }
    if quad_area > 0.0 { 1 } else { -1 }
}

/// `outside_interval`: whether `numer / denom` lies outside `[0, 1]`.
// Port of: src/utils/SkPolyUtils.cpp#L92-L95 (chrome/m156)
#[inline]
fn outside_interval(numer: scalar, denom: scalar, denom_positive: bool) -> bool {
    (denom_positive && (numer < 0.0 || numer > denom))
        || (!denom_positive && (numer > 0.0 || numer < denom))
}

/// `zero_length`: the vector is not finite or has zero squared length.
// Port of: src/utils/SkPolyUtils.cpp#L98-L100 (chrome/m156)
#[inline]
fn zero_length(v: Vector, vdotv: scalar) -> bool {
    !(is_finite_all(v.x, &[v.y]) && vdotv != 0.0)
}

/// `compute_intersection`: the intersection of two offset segments and the parameters `s` and `t`
/// along each, or `None` if they do not intersect within both segments.
// Port of: src/utils/SkPolyUtils.cpp#L107-L195 (chrome/m156)
fn compute_intersection(s0: &OffsetSegment, s1: &OffsetSegment) -> Option<(Point, scalar, scalar)> {
    let v0 = s0.v;
    let v1 = s1.v;
    let w = s1.p0 - s0.p0;
    let mut denom = v0.cross(v1);
    let denom_positive = denom > 0.0;
    let s_numer: scalar;
    let t_numer: scalar;
    if nearly_zero_tol(denom, CROSS_TOLERANCE) {
        if !nearly_zero_tol(w.cross(v0), CROSS_TOLERANCE)
            || !nearly_zero_tol(w.cross(v1), CROSS_TOLERANCE)
        {
            return None;
        }
        let v0dotv0 = v0.dot(v0);
        if zero_length(v0, v0dotv0) {
            let v1dotv1 = v1.dot(v1);
            if zero_length(v1, v1dotv1) {
                if !point_priv::can_normalize(w.x, w.y) {
                    return Some((s0.p0, 0.0, 0.0));
                }
                return None;
            }
            t_numer = v1.dot(-w);
            denom = v1dotv1;
            if outside_interval(t_numer, denom, true) {
                return None;
            }
            s_numer = 0.0;
        } else {
            let mut sn = v0.dot(w);
            denom = v0dotv0;
            let mut tn = 0.0;
            if outside_interval(sn, denom, true) {
                let v1dotv1 = v1.dot(v1);
                if zero_length(v1, v1dotv1) {
                    return None;
                }
                let old_s_numer = sn;
                sn = v0.dot(w + v1);
                tn = denom;
                if outside_interval(sn, denom, true) {
                    if sn * old_s_numer > 0.0 {
                        return None;
                    }
                    sn = 0.0;
                    tn = v1.dot(-w);
                    denom = v1dotv1;
                }
            }
            s_numer = sn;
            t_numer = tn;
        }
    } else {
        s_numer = w.cross(v1);
        if outside_interval(s_numer, denom, denom_positive) {
            return None;
        }
        t_numer = w.cross(v0);
        if outside_interval(t_numer, denom, denom_positive) {
            return None;
        }
    }
    let local_s = s_numer / denom;
    let local_t = t_numer / denom;
    let p = s0.p0 + v0 * local_s;
    Some((p, local_s, local_t))
}

/// `SkIsConvexPolygon`: whether the polygon is convex (no sign changes in the cross products and
/// at most two sign changes in each of x and y).
// Port of: src/utils/SkPolyUtils.cpp#L197-L252 (chrome/m156)
#[doc(alias = "SkIsConvexPolygon")]
#[must_use]
pub fn is_convex_polygon(polygon_verts: &[Point]) -> bool {
    let n = polygon_verts.len();
    if n < 3 {
        return false;
    }
    let mut last_perp_dot: scalar = 0.0;
    let mut x_sign_change_count = 0;
    let mut y_sign_change_count = 0;
    let mut curr_index = 0;
    let mut next_index = 1;
    let mut v0 = polygon_verts[curr_index] - polygon_verts[n - 1];
    let mut last_vx = v0.x;
    let mut last_vy = v0.y;
    let mut v1 = polygon_verts[next_index] - polygon_verts[curr_index];
    for _ in 0..n {
        if !is_finite_all(polygon_verts[curr_index].x, &[polygon_verts[curr_index].y]) {
            return false;
        }
        let perp_dot = v0.cross(v1);
        if last_perp_dot * perp_dot < 0.0 {
            return false;
        }
        if perp_dot != 0.0 {
            last_perp_dot = perp_dot;
        }
        if last_vx * v1.x < 0.0 {
            x_sign_change_count += 1;
        }
        if last_vy * v1.y < 0.0 {
            y_sign_change_count += 1;
        }
        if x_sign_change_count > 2 || y_sign_change_count > 2 {
            return false;
        }
        curr_index = next_index;
        next_index = (curr_index + 1) % n;
        if v1.x != 0.0 {
            last_vx = v1.x;
        }
        if v1.y != 0.0 {
            last_vy = v1.y;
        }
        v0 = v1;
        v1 = polygon_verts[next_index] - polygon_verts[curr_index];
    }
    true
}

/// One edge of the inset polygon, linked to its neighbours by index (`OffsetEdge`).
// Port of: src/utils/SkPolyUtils.cpp#L253-L267 (chrome/m156)
#[derive(Clone, Copy, Debug)]
struct OffsetEdge {
    prev: usize,
    next: usize,
    offset: OffsetSegment,
    intersection: Point,
    t_value: scalar,
}

impl OffsetEdge {
    /// `init`: the intersection starts at the segment start with the smallest `t`.
    // Port of: src/utils/SkPolyUtils.cpp#L262-L267 (chrome/m156)
    fn init(&mut self) {
        self.intersection = self.offset.p0;
        self.t_value = SCALAR_MIN;
    }
}

/// `remove_node`: unlinks `node` from the circular list, moving `head` if it was the head.
// Port of: src/utils/SkPolyUtils.cpp#L316-L323 (chrome/m156)
fn remove_node(edges: &mut [OffsetEdge], node: usize, head: &mut Option<usize>) {
    let prev = edges[node].prev;
    let next = edges[node].next;
    edges[prev].next = next;
    edges[next].prev = prev;
    if Some(node) == *head {
        *head = if next == node { None } else { Some(next) };
    }
}

/// `SkInsetConvexPolygon`: insets a convex polygon by `inset`, or `None` if it cannot be done.
// Port of: src/utils/SkPolyUtils.cpp#L339-L489 (chrome/m156)
#[doc(alias = "SkInsetConvexPolygon")]
#[must_use]
#[allow(clippy::too_many_lines)] // mirrors the length of SkInsetConvexPolygon
pub fn inset_convex_polygon(input_polygon_verts: &[Point], inset: scalar) -> Option<Vec<Point>> {
    let n = input_polygon_verts.len();
    if n < 3 {
        return None;
    }
    if n > usize::from(u16::MAX) {
        return None;
    }
    if inset < -SCALAR_NEARLY_ZERO || !is_finite_all(inset, &[]) {
        return None;
    }
    if inset <= SCALAR_NEARLY_ZERO {
        return Some(input_polygon_verts.to_vec());
    }

    let winding = get_polygon_winding(input_polygon_verts);
    if winding == 0 {
        return None;
    }
    let winding_f = winding as scalar;

    let mut edges: Vec<OffsetEdge> = Vec::with_capacity(n);
    let mut prev = n - 1;
    for curr in 0..n {
        let next = (curr + 1) % n;
        if !is_finite_all(input_polygon_verts[curr].x, &[input_polygon_verts[curr].y]) {
            return None;
        }
        if compute_side(
            input_polygon_verts[prev],
            input_polygon_verts[curr] - input_polygon_verts[prev],
            input_polygon_verts[next],
        ) * winding
            < 0
        {
            return None;
        }
        let v = input_polygon_verts[next] - input_polygon_verts[curr];
        let mut perp = Vector::new(-v.y, v.x);
        let _ = perp.set_length(inset * winding_f);
        let mut edge = OffsetEdge {
            prev,
            next,
            offset: OffsetSegment {
                p0: input_polygon_verts[curr] + perp,
                v,
            },
            intersection: Point::default(),
            t_value: 0.0,
        };
        edge.init();
        edges.push(edge);
        prev = curr;
    }

    let mut head: Option<usize> = Some(0);
    let mut curr_edge = 0usize;
    let mut prev_edge = edges[curr_edge].prev;
    let mut inset_vertex_count = n as i64;
    let mut iterations: u32 = 0;
    let max_iterations = (n * n) as u32;
    while head.is_some() && prev_edge != curr_edge {
        iterations += 1;
        if iterations > max_iterations {
            return None;
        }
        if let Some((intersection, s, t)) =
            compute_intersection(&edges[prev_edge].offset, &edges[curr_edge].offset)
        {
            if s < edges[prev_edge].t_value {
                remove_node(&mut edges, prev_edge, &mut head);
                inset_vertex_count -= 1;
                prev_edge = edges[prev_edge].prev;
            } else if edges[curr_edge].t_value > SCALAR_MIN
                && point_priv::equals_within_tolerance_tol(
                    intersection,
                    edges[curr_edge].intersection,
                    1.0e-6,
                )
            {
                break;
            } else {
                edges[curr_edge].intersection = intersection;
                edges[curr_edge].t_value = t;
                prev_edge = curr_edge;
                curr_edge = edges[curr_edge].next;
            }
        } else {
            let side = winding
                * compute_side(
                    edges[curr_edge].offset.p0,
                    edges[curr_edge].offset.v,
                    edges[prev_edge].offset.p0,
                );
            if side < 0
                && side
                    == winding
                        * compute_side(
                            edges[curr_edge].offset.p0,
                            edges[curr_edge].offset.v,
                            edges[prev_edge].offset.p0 + edges[prev_edge].offset.v,
                        )
            {
                remove_node(&mut edges, prev_edge, &mut head);
                inset_vertex_count -= 1;
                prev_edge = edges[prev_edge].prev;
            } else {
                remove_node(&mut edges, curr_edge, &mut head);
                inset_vertex_count -= 1;
                curr_edge = edges[curr_edge].next;
            }
        }
    }

    let head = head?;
    let mut inset_polygon: Vec<Point> = Vec::with_capacity(inset_vertex_count.max(0) as usize);
    let mut curr_index = 0usize;
    inset_polygon.push(edges[head].intersection);
    let mut e = edges[head].next;
    while e != head {
        if !point_priv::equals_within_tolerance_tol(
            edges[e].intersection,
            inset_polygon[curr_index],
            CLEANUP_TOLERANCE,
        ) {
            inset_polygon.push(edges[e].intersection);
            curr_index += 1;
        }
        e = edges[e].next;
    }
    if curr_index >= 1
        && point_priv::equals_within_tolerance_tol(
            inset_polygon[0],
            inset_polygon[curr_index],
            CLEANUP_TOLERANCE,
        )
    {
        inset_polygon.pop();
    }
    if is_convex_polygon(&inset_polygon) {
        Some(inset_polygon)
    } else {
        None
    }
}

/// `SkComputeRadialSteps`: the rotation between two offsets and the number of arc steps needed to
/// approximate it. Returns `(rot_sin, rot_cos, steps)` or `None`.
// Port of: src/utils/SkPolyUtils.cpp#L491-L526 (chrome/m156)
#[doc(alias = "SkComputeRadialSteps")]
#[must_use]
#[allow(clippy::float_cmp)] // the exact zero/one tests mirror SkComputeRadialSteps
pub fn compute_radial_steps(
    v1: Vector,
    v2: Vector,
    offset: scalar,
) -> Option<(scalar, scalar, i32)> {
    const RECIP_PIXELS_PER_ARC_SEGMENT: scalar = 0.25;
    let r_cos = v1.dot(v2);
    if !is_finite_all(r_cos, &[]) {
        return None;
    }
    let r_sin = v1.cross(v2);
    if !is_finite_all(r_sin, &[]) {
        return None;
    }
    let theta = crate::scalar::scalar_atan2(r_sin, r_cos);
    let float_steps = (offset * theta * RECIP_PIXELS_PER_ARC_SEGMENT).abs();
    if float_steps >= f32::from(u16::MAX) {
        return None;
    }
    let steps = scalar_round_to_int(float_steps);
    let d_theta = if steps > 0 {
        theta / steps as scalar
    } else {
        0.0
    };
    let rot_sin = crate::scalar::scalar_sin(d_theta);
    let rot_cos = crate::scalar::scalar_cos(d_theta);
    if steps > 0 && (rot_sin == 0.0 || rot_cos == 1.0) {
        return None;
    }
    Some((rot_sin, rot_cos, steps))
}
