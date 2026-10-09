// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/utils/SkPolyUtils.h, src/utils/SkPolyUtils.cpp

//! Polygon utilities (`SkPolyUtils.h`): winding, convexity and simplicity tests, inset and offset
//! polygons, radial steps for round joins, and ear-clipping triangulation.
//!
//! Skia's pointer graphs (the offset-edge linked lists, the active-edge red-black tree and the
//! intrusive vertex lists) are stored in `Vec`s and linked by index. The `Option<usize>` links
//! play the role of the C++ `nullptr` pointers, and every algorithm keeps Skia's control flow and
//! evaluation order.
//!
//! Skia's `SkTDArray` outputs become `&mut Vec<T>`, and `(const SkPoint*, int count)` inputs
//! become slices; the sizes Skia passes separately are the slice lengths.

use std::cmp::{max, min};

use crate::floating_point::{float_round2int, ieee_float_divide, is_finite, is_finite_all};
use crate::point::point_priv;
use crate::point::{Point, Vector};
use crate::rect::{Rect, rect_priv};
use crate::scalar::{
    SCALAR_1, SCALAR_MAX, SCALAR_MIN, SCALAR_NEARLY_ZERO, Scalar, int_to_scalar, scalar,
    scalar_abs, scalar_atan2, scalar_cos, scalar_sin, scalar_sqrt,
};

/// `kCrossTolerance` in `SkPolyUtils.cpp`: `SK_ScalarNearlyZero * SK_ScalarNearlyZero`.
// Port of: src/utils/SkPolyUtils.cpp#L42 (chrome/m156)
const K_CROSS_TOLERANCE: scalar = SCALAR_NEARLY_ZERO * SCALAR_NEARLY_ZERO;

/// The square of `SK_ScalarNearlyZero`, used by the reflex tests of the triangulation.
// Port of: src/utils/SkPolyUtils.cpp#L1624 (chrome/m156)
const K_NEARLY_ZERO_SQUARED: scalar = SCALAR_NEARLY_ZERO * SCALAR_NEARLY_ZERO;

/// Tolerance for merging nearly coincident output vertices (`kCleanupTolerance`).
// Port of: src/utils/SkPolyUtils.cpp#L462 (chrome/m156)
const K_CLEANUP_TOLERANCE: scalar = 0.01;

/// Maximum value of a `uint16_t` index, as `std::numeric_limits<uint16_t>::max()` (promoted to
/// `int` in the C++ comparisons).
const U16_MAX: usize = u16::MAX as usize;

/// Converts a vertex index to the `uint16_t` Skia stores it in. Callers only pass indices below
/// the polygon size, which the public entry points cap below `uint16_t::MAX`.
fn to_index(i: usize) -> u16 {
    u16::try_from(i).expect("polygon index fits in uint16_t")
}

// std::min(a, b) for floats: returns b only when b < a.
fn std_min(a: scalar, b: scalar) -> scalar {
    if b < a { b } else { a }
}

// std::max(a, b) for floats: returns b only when a < b.
fn std_max(a: scalar, b: scalar) -> scalar {
    if a < b { b } else { a }
}

// Port of: src/utils/SkPolyUtils.cpp#L37-L40 (chrome/m156)
#[derive(Copy, Clone, Default, Debug)]
struct OffsetSegment {
    p0: Point,
    v: Vector,
}

// Computes perpDot for point p compared to segment defined by origin p0 and vector v.
// A positive value means the point is to the left of the segment,
// negative is to the right, 0 is collinear.
// Port of: src/utils/SkPolyUtils.cpp#L47-L55 (chrome/m156)
fn compute_side(p0: Point, v: Vector, p: Point) -> i32 {
    let w = p - p0;
    let perp_dot = v.cross(w);
    if !perp_dot.nearly_zero(K_CROSS_TOLERANCE) {
        return if perp_dot > 0.0 { 1 } else { -1 };
    }

    0
}

/// Determines the winding direction of a polygon. Returns 1 for cw, -1 for ccw, and 0 if the
/// signed area is zero (either degenerate or self-intersecting). The y-axis points down.
///
/// The input polygon must be simple or the result will be meaningless.
// Port of: src/utils/SkPolyUtils.cpp#L58-L76 (chrome/m156)
#[doc(alias = "SkGetPolygonWinding")]
#[must_use]
pub fn get_polygon_winding(polygon_verts: &[Point]) -> i32 {
    if polygon_verts.len() < 3 {
        return 0;
    }

    // compute area and use sign to determine winding
    let mut quad_area: scalar = 0.0;
    let mut v0 = polygon_verts[1] - polygon_verts[0];
    for curr in 2..polygon_verts.len() {
        let v1 = polygon_verts[curr] - polygon_verts[0];
        quad_area += v0.cross(v1);
        v0 = v1;
    }
    if quad_area.nearly_zero(K_CROSS_TOLERANCE) {
        return 0;
    }
    // 1 == ccw, -1 == cw
    if quad_area > 0.0 { 1 } else { -1 }
}

// Computes the difference vector to offset p0-p1 'offset' units in direction specified by 'side'.
// Port of: src/utils/SkPolyUtils.cpp#L79-L89 (chrome/m156)
fn compute_offset_vector(p0: Point, p1: Point, offset: scalar, side: i32) -> Option<Vector> {
    debug_assert!(side == -1 || side == 1);
    // if distances are equal, can just outset by the perpendicular
    let mut perp = Vector::new(p0.y - p1.y, p1.x - p0.x);
    if !perp.set_length(offset * int_to_scalar(side)) {
        return None;
    }
    Some(perp)
}

// check interval to see if intersection is in segment
// Port of: src/utils/SkPolyUtils.cpp#L92-L95 (chrome/m156)
fn outside_interval(numer: scalar, denom: scalar, denom_positive: bool) -> bool {
    (denom_positive && (numer < 0.0 || numer > denom))
        || (!denom_positive && (numer > 0.0 || numer < denom))
}

// special zero-length test when we're using vdotv as a denominator
// Port of: src/utils/SkPolyUtils.cpp#L98-L100 (chrome/m156)
fn zero_length(v: Vector, vdotv: scalar) -> bool {
    !(is_finite_all(v.x, &[v.y]) && vdotv != 0.0)
}

// Compute the intersection 'p' between segments s0 and s1, if any.
// 's' is the parametric value for the intersection along 's0' & 't' is the same for 's1'.
// Returns None if there is no intersection; otherwise returns (p, s, t).
// If the length squared of a segment is 0, then we treat the segment as degenerate
// and use only the first endpoint for tests.
// Port of: src/utils/SkPolyUtils.cpp#L107-L195 (chrome/m156)
fn compute_intersection(s0: &OffsetSegment, s1: &OffsetSegment) -> Option<(Point, scalar, scalar)> {
    let v0 = s0.v;
    let v1 = s1.v;
    let w = s1.p0 - s0.p0;
    let mut denom = v0.cross(v1);
    let denom_positive = denom > 0.0;
    let mut s_numer: scalar;
    let mut t_numer: scalar;
    if denom.nearly_zero(K_CROSS_TOLERANCE) {
        // segments are parallel, but not collinear
        if !w.cross(v0).nearly_zero(K_CROSS_TOLERANCE)
            || !w.cross(v1).nearly_zero(K_CROSS_TOLERANCE)
        {
            return None;
        }

        // Check for zero-length segments
        let v0dotv0 = v0.dot(v0);
        if zero_length(v0, v0dotv0) {
            // Both are zero-length
            let v1dotv1 = v1.dot(v1);
            if zero_length(v1, v1dotv1) {
                // Check if they're the same point
                if point_priv::can_normalize(w.x, w.y) {
                    // Intersection is indeterminate
                    return None;
                }
                return Some((s0.p0, 0.0, 0.0));
            }
            // Otherwise project segment0's origin onto segment1
            t_numer = v1.dot(-w);
            denom = v1dotv1;
            if outside_interval(t_numer, denom, true) {
                return None;
            }
            s_numer = 0.0;
        } else {
            // Project segment1's endpoints onto segment0
            s_numer = v0.dot(w);
            denom = v0dotv0;
            t_numer = 0.0;
            if outside_interval(s_numer, denom, true) {
                // The first endpoint doesn't lie on segment0
                // If segment1 is degenerate, then there's no collision
                let v1dotv1 = v1.dot(v1);
                if zero_length(v1, v1dotv1) {
                    return None;
                }

                // Otherwise try the other one
                let old_s_numer = s_numer;
                s_numer = v0.dot(w + v1);
                t_numer = denom;
                if outside_interval(s_numer, denom, true) {
                    // it's possible that segment1's interval surrounds segment0
                    // this is false if params have the same signs, and in that case no collision
                    if s_numer * old_s_numer > 0.0 {
                        return None;
                    }
                    // otherwise project segment0's endpoint onto segment1 instead
                    s_numer = 0.0;
                    t_numer = v1.dot(-w);
                    denom = v1dotv1;
                }
            }
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

/// Determines whether a polygon is convex.
// Port of: src/utils/SkPolyUtils.cpp#L197-L251 (chrome/m156)
#[doc(alias = "SkIsConvexPolygon")]
#[must_use]
#[allow(clippy::float_cmp, clippy::similar_names)] // mirrors `0 != perpDot` and Skia's v0/v1
pub fn is_convex_polygon(polygon_verts: &[Point]) -> bool {
    let polygon_size = polygon_verts.len();
    if polygon_size < 3 {
        return false;
    }

    let mut last_perp_dot: scalar = 0.0;
    let mut x_sign_change_count = 0;
    let mut y_sign_change_count = 0;

    let prev_index = polygon_size - 1;
    let mut curr_index = 0;
    let mut next_index = 1;
    let mut v0 = polygon_verts[curr_index] - polygon_verts[prev_index];
    let mut last_vx = v0.x;
    let mut last_vy = v0.y;
    let mut v1 = polygon_verts[next_index] - polygon_verts[curr_index];
    for vert in polygon_verts {
        if !vert.is_finite() {
            return false;
        }

        // Check that winding direction is always the same (otherwise we have a reflex vertex)
        let perp_dot = v0.cross(v1);
        if last_perp_dot * perp_dot < 0.0 {
            return false;
        }
        if 0.0 != perp_dot {
            last_perp_dot = perp_dot;
        }

        // Check that the signs of the edge vectors don't change more than twice per coordinate
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
        next_index = (curr_index + 1) % polygon_size;
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

// An edge of an offset polygon, linked to its neighbours by index (`OffsetEdge`).
// Port of: src/utils/SkPolyUtils.cpp#L253-L314 (chrome/m156)
#[derive(Copy, Clone, Default, Debug)]
struct OffsetEdge {
    prev: Option<usize>,
    next: Option<usize>,
    offset: OffsetSegment,
    intersection: Point,
    t_value: scalar,
    index: u16,
    end: u16,
}

impl OffsetEdge {
    // Port of: src/utils/SkPolyUtils.cpp#L262-L267 (chrome/m156)
    fn init(&mut self, start: u16, end: u16) {
        self.intersection = self.offset.p0;
        self.t_value = SCALAR_MIN;
        self.index = start;
        self.end = end;
    }

    // special intersection check that looks for endpoint intersection
    // Port of: src/utils/SkPolyUtils.cpp#L270-L283 (chrome/m156)
    fn check_intersection(&self, that: &OffsetEdge) -> Option<(Point, scalar, scalar)> {
        if self.end == that.index {
            let p1 = self.offset.p0 + self.offset.v;
            if point_priv::equals_within_tolerance(p1, that.offset.p0) {
                return Some((p1, SCALAR_1, 0.0));
            }
        }

        compute_intersection(&self.offset, &that.offset)
    }

    // computes the line intersection and then the "distance" from that to this
    // this is really a signed squared distance, where negative means that
    // the intersection lies inside this->fOffset
    // Port of: src/utils/SkPolyUtils.cpp#L288-L312 (chrome/m156)
    fn compute_crossing_distance(&self, that: &OffsetEdge) -> scalar {
        let s0 = &self.offset;
        let s1 = &that.offset;
        let v0 = s0.v;
        let v1 = s1.v;

        let denom = v0.cross(v1);
        if denom.nearly_zero(K_CROSS_TOLERANCE) {
            // segments are parallel
            return SCALAR_MAX;
        }

        let w = s1.p0 - s0.p0;
        let mut local_s = w.cross(v1) / denom;
        if local_s < 0.0 {
            local_s = -local_s;
        } else {
            local_s -= SCALAR_1;
        }

        local_s *= scalar_abs(local_s);
        local_s *= v0.dot(v0);

        local_s
    }
}

// Removes `node` from the circular list of offset edges, moving `head` if it was the head.
// Port of: src/utils/SkPolyUtils.cpp#L316-L323 (chrome/m156)
fn remove_node(edges: &mut [OffsetEdge], node: usize, head: &mut Option<usize>) {
    // remove from linked list
    let prev = edges[node].prev.expect("offset edges form a closed list");
    let next = edges[node].next.expect("offset edges form a closed list");
    edges[prev].next = Some(next);
    edges[next].prev = Some(prev);
    if Some(node) == *head {
        *head = if edges[node].next == Some(node) {
            None
        } else {
            edges[node].next
        };
    }
}

//////////////////////////////////////////////////////////////////////////////////

// The objective here is to inset all of the edges by the given distance, and then
// remove any invalid inset edges by detecting right-hand turns. In a ccw polygon,
// we should only be making left-hand turns (for cw polygons, we use the winding
// parameter to reverse this). We detect this by checking whether the second intersection
// on an edge is closer to its tail than the first one.
//
// We might also have the case that there is no intersection between two neighboring inset edges.
// In this case, one edge will lie to the right of the other and should be discarded along with
// its previous intersection (if any).
//
// Note: the assumption is that inputPolygon is convex and has no coincident points.
//
/// Generates a polygon that is inset a constant from the boundary of a given convex polygon.
/// The input polygon is expected to have values clamped to the nearest 1/16th, should be convex
/// and have no coincident points. `inset` should be a positive value.
///
/// Appends the inset polygon to `inset_polygon` (after clearing it) and returns true if an inset
/// polygon exists.
///
/// # Panics
///
/// Does not panic for any input: the edge list is closed by construction, and the `expect`s check
/// that invariant.
// Port of: src/utils/SkPolyUtils.cpp#L339-L486 (chrome/m156)
#[doc(alias = "SkInsetConvexPolygon")]
#[allow(clippy::too_many_lines)] // mirrors the single C++ function
pub fn inset_convex_polygon(
    input_polygon_verts: &[Point],
    inset: scalar,
    inset_polygon: &mut Vec<Point>,
) -> bool {
    let input_polygon_size = input_polygon_verts.len();
    if input_polygon_size < 3 {
        return false;
    }

    // restrict this to match other routines
    // practically we don't want anything bigger than this anyway
    if input_polygon_size > U16_MAX {
        return false;
    }

    // can't inset by a negative or non-finite amount
    if inset < -SCALAR_NEARLY_ZERO || !is_finite(inset) {
        return false;
    }

    // insetting close to zero just returns the original poly
    if inset <= SCALAR_NEARLY_ZERO {
        inset_polygon.extend_from_slice(input_polygon_verts);
        return true;
    }

    // get winding direction
    let winding = get_polygon_winding(input_polygon_verts);
    if 0 == winding {
        return false;
    }

    // set up
    let mut edge_data = vec![OffsetEdge::default(); input_polygon_size];
    let mut prev = input_polygon_size - 1;
    for curr in 0..input_polygon_size {
        let next = (curr + 1) % input_polygon_size;
        if !input_polygon_verts[curr].is_finite() {
            return false;
        }
        // check for convexity just to be sure
        if compute_side(
            input_polygon_verts[prev],
            input_polygon_verts[curr] - input_polygon_verts[prev],
            input_polygon_verts[next],
        ) * winding
            < 0
        {
            return false;
        }
        let v = input_polygon_verts[next] - input_polygon_verts[curr];
        let mut perp = Vector::new(-v.y, v.x);
        // The return value is ignored, as in Skia: a failed length leaves `perp` at zero.
        let _ = perp.set_length(inset * int_to_scalar(winding));
        edge_data[curr].prev = Some(prev);
        edge_data[curr].next = Some(next);
        edge_data[curr].offset.p0 = input_polygon_verts[curr] + perp;
        edge_data[curr].offset.v = v;
        edge_data[curr].init(0, 0);
        prev = curr;
    }

    let mut head: Option<usize> = Some(0);
    let mut curr_edge = 0usize;
    let mut prev_edge = edge_data[curr_edge]
        .prev
        .expect("offset edges form a closed list");
    // SkTDArray::reserve(insetVertexCount) is only a capacity hint, so the count is not kept.
    let max_iterations = (input_polygon_size as u64) * (input_polygon_size as u64);
    let mut iterations: u64 = 0;
    while head.is_some() && prev_edge != curr_edge {
        iterations += 1;
        // we should check each edge against each other edge at most once
        if iterations > max_iterations {
            return false;
        }

        if let Some((intersection, s, t)) =
            compute_intersection(&edge_data[prev_edge].offset, &edge_data[curr_edge].offset)
        {
            // if new intersection is further back on previous inset from the prior intersection
            if s < edge_data[prev_edge].t_value {
                // no point in considering this one again
                remove_node(&mut edge_data, prev_edge, &mut head);
                // go back one segment
                prev_edge = edge_data[prev_edge]
                    .prev
                    .expect("offset edges form a closed list");
            // we've already considered this intersection, we're done
            } else if edge_data[curr_edge].t_value > SCALAR_MIN
                && point_priv::equals_within_tolerance_tol(
                    intersection,
                    edge_data[curr_edge].intersection,
                    1.0e-6,
                )
            {
                break;
            } else {
                // add intersection
                edge_data[curr_edge].intersection = intersection;
                edge_data[curr_edge].t_value = t;

                // go to next segment
                prev_edge = curr_edge;
                curr_edge = edge_data[curr_edge]
                    .next
                    .expect("offset edges form a closed list");
            }
        } else {
            // if prev to right side of curr
            let side = winding
                * compute_side(
                    edge_data[curr_edge].offset.p0,
                    edge_data[curr_edge].offset.v,
                    edge_data[prev_edge].offset.p0,
                );
            if side < 0
                && side
                    == winding
                        * compute_side(
                            edge_data[curr_edge].offset.p0,
                            edge_data[curr_edge].offset.v,
                            edge_data[prev_edge].offset.p0 + edge_data[prev_edge].offset.v,
                        )
            {
                // no point in considering this one again
                remove_node(&mut edge_data, prev_edge, &mut head);
                // go back one segment
                prev_edge = edge_data[prev_edge]
                    .prev
                    .expect("offset edges form a closed list");
            } else {
                // move to next segment
                remove_node(&mut edge_data, curr_edge, &mut head);
                curr_edge = edge_data[curr_edge]
                    .next
                    .expect("offset edges form a closed list");
            }
        }
    }

    // store all the valid intersections that aren't nearly coincident
    // TODO: look at the main algorithm and see if we can detect these better
    inset_polygon.clear();
    let Some(head) = head else {
        return false;
    };

    let mut curr_index = 0usize;
    inset_polygon.push(edge_data[head].intersection);
    let mut curr = edge_data[head]
        .next
        .expect("offset edges form a closed list");
    while curr != head {
        if !point_priv::equals_within_tolerance_tol(
            edge_data[curr].intersection,
            inset_polygon[curr_index],
            K_CLEANUP_TOLERANCE,
        ) {
            inset_polygon.push(edge_data[curr].intersection);
            curr_index += 1;
        }
        curr = edge_data[curr]
            .next
            .expect("offset edges form a closed list");
    }
    // make sure the first and last points aren't coincident
    if curr_index >= 1
        && point_priv::equals_within_tolerance_tol(
            inset_polygon[0],
            inset_polygon[curr_index],
            K_CLEANUP_TOLERANCE,
        )
    {
        inset_polygon.pop();
    }

    is_convex_polygon(inset_polygon)
}

///////////////////////////////////////////////////////////////////////////////////////////

// compute the number of points needed for a circular join when offsetting a reflex vertex
/// Computes the number of points needed for a circular join when offsetting a vertex.
///
/// Returns `(rot_sin, rot_cos, n)`: the sine and cosine of the rotation delta per step, and the
/// number of steps to fill out the arc. Returns `None` if the offset cannot be represented.
/// The offset vectors' lengths don't have to equal `|offset|`; only their directions matter.
/// The segment lengths will be approximately four pixels.
// Port of: src/utils/SkPolyUtils.cpp#L490-L523 (chrome/m156)
#[doc(alias = "SkComputeRadialSteps")]
#[must_use]
#[allow(clippy::float_cmp)] // mirrors `*rotSin == 0 || *rotCos == 1`
pub fn compute_radial_steps(
    v1: Vector,
    v2: Vector,
    offset: scalar,
) -> Option<(scalar, scalar, i32)> {
    const K_RECIP_PIXELS_PER_ARC_SEGMENT: scalar = 0.25;

    let r_cos = v1.dot(v2);
    if !is_finite(r_cos) {
        return None;
    }
    let r_sin = v1.cross(v2);
    if !is_finite(r_sin) {
        return None;
    }
    let theta = scalar_atan2(r_sin, r_cos);

    let float_steps = scalar_abs(offset * theta * K_RECIP_PIXELS_PER_ARC_SEGMENT);
    // limit the number of steps to at most max uint16_t (that's all we can index)
    // knock one value off the top to account for rounding
    if float_steps >= f32::from(u16::MAX) {
        return None;
    }
    let steps = float_round2int(float_steps);

    let d_theta = if steps > 0 {
        theta / int_to_scalar(steps)
    } else {
        0.0
    };
    let rot_sin = scalar_sin(d_theta);
    let rot_cos = scalar_cos(d_theta);
    // Our offset may be so large that we end up with a tiny dTheta, in which case we
    // lose precision when computing rotSin and rotCos.
    if steps > 0 && (rot_sin == 0.0 || rot_cos == 1.0) {
        return None;
    }
    Some((rot_sin, rot_cos, steps))
}

///////////////////////////////////////////////////////////////////////////////////////////

// a point is "left" to another if its x-coord is less, or if equal, its y-coord is greater
// Port of: src/utils/SkPolyUtils.cpp#L528-L530 (chrome/m156)
#[allow(clippy::neg_cmp_op_on_partial_ord)] // mirrors Skia's `!(p0.fX > p1.fX)` tie-break
fn left(p0: Point, p1: Point) -> bool {
    p0.x < p1.x || (!(p0.x > p1.x) && p0.y > p1.y)
}

// Port of: src/utils/SkPolyUtils.cpp#L537-L548 (chrome/m156)
#[derive(Copy, Clone, Debug)]
struct Vertex {
    position: Point,
    index: u16,
    prev_index: u16,
    next_index: u16,
    flags: u16,
}

// Port of: src/utils/SkPolyUtils.cpp#L550-L553 (chrome/m156)
const K_PREV_LEFT_VERTEX_FLAG: u16 = 0x1;
// Port of: src/utils/SkPolyUtils.cpp#L550-L553 (chrome/m156)
const K_NEXT_LEFT_VERTEX_FLAG: u16 = 0x2;

// Port of: src/utils/SkPolyUtils.cpp#L537-L540 (chrome/m156), `Vertex::Left`
fn vertex_left(qv0: &Vertex, qv1: &Vertex) -> bool {
    left(qv0.position, qv1.position)
}

// The priority queue of `SkTDPQueue<Vertex, Vertex::Left>`, without the index callback.
// Port of: src/core/SkTDPQueue.h#L33-L197 (chrome/m156)
struct VertexQueue {
    array: Vec<Vertex>,
}

impl VertexQueue {
    // Port of: src/core/SkTDPQueue.h#L36 (chrome/m156)
    fn with_capacity(reserve: usize) -> Self {
        Self {
            array: Vec::with_capacity(reserve),
        }
    }

    // Port of: src/core/SkTDPQueue.h#L44-L45 (chrome/m156)
    fn count(&self) -> usize {
        self.array.len()
    }

    // Port of: src/core/SkTDPQueue.h#L48 (chrome/m156)
    fn peek(&self) -> &Vertex {
        &self.array[0]
    }

    // Port of: src/core/SkTDPQueue.h#L52-L66 (chrome/m156)
    fn pop(&mut self) {
        if 1 == self.array.len() {
            self.array.pop();
            return;
        }

        let last = self.array[self.array.len() - 1];
        self.array[0] = last;
        self.array.pop();
        self.percolate_down_if_necessary(0);
    }

    // Port of: src/core/SkTDPQueue.h#L69-L76 (chrome/m156)
    fn insert(&mut self, entry: Vertex) {
        let index = self.array.len();
        self.array.push(entry);
        self.percolate_up_if_necessary(index);
    }

    // Port of: src/core/SkTDPQueue.h#L137-L158 (chrome/m156)
    fn percolate_up_if_necessary(&mut self, mut index: usize) -> bool {
        let mut percolated = false;
        loop {
            if 0 == index {
                return percolated;
            }
            let p = (index - 1) >> 1;
            if vertex_left(&self.array[index], &self.array[p]) {
                self.array.swap(index, p);
                index = p;
                percolated = true;
            } else {
                return percolated;
            }
        }
    }

    // Port of: src/core/SkTDPQueue.h#L160-L197 (chrome/m156)
    fn percolate_down_if_necessary(&mut self, mut index: usize) {
        loop {
            let mut child = 2 * index + 1;

            if child >= self.array.len() {
                // We're a leaf.
                return;
            }

            if child + 1 >= self.array.len() {
                // We only have a left child.
                if vertex_left(&self.array[child], &self.array[index]) {
                    self.array.swap(child, index);
                    return;
                }
            } else if vertex_left(&self.array[child + 1], &self.array[child]) {
                // The right child is the one we should swap with, if we swap.
                child += 1;
            }

            // Check if we need to swap.
            if vertex_left(&self.array[child], &self.array[index]) {
                self.array.swap(child, index);
                index = child;
            } else {
                // We're less than both our children.
                return;
            }
        }
    }
}

// Index of the sentinel head of the active edge tree (`fTreeHead`).
const TREE_HEAD: usize = 0;

// An edge in the sweep line (`ActiveEdge`). Children, above and below are arena indices.
// Port of: src/utils/SkPolyUtils.cpp#L555-L566 (chrome/m156)
#[derive(Copy, Clone, Debug)]
struct ActiveEdge {
    segment: OffsetSegment,
    index0: u16,
    index1: u16,
    child: [Option<usize>; 2],
    above: Option<usize>,
    below: Option<usize>,
    red: bool,
}

impl ActiveEdge {
    // The default-constructed `fTreeHead` sentinel.
    // Port of: src/utils/SkPolyUtils.cpp#L556 (chrome/m156)
    fn head() -> Self {
        Self {
            segment: OffsetSegment::default(),
            index0: 0,
            index1: 0,
            child: [None, None],
            above: None,
            below: None,
            red: false,
        }
    }

    // Port of: src/utils/SkPolyUtils.cpp#L557-L566 (chrome/m156)
    fn new(p0: Point, v: Vector, index0: u16, index1: u16) -> Self {
        Self {
            segment: OffsetSegment { p0, v },
            index0,
            index1,
            child: [None, None],
            above: None,
            below: None,
            red: true,
        }
    }
}

// The sweep line: a red-black tree of active edges (`ActiveEdgeList`).
// Port of: src/utils/SkPolyUtils.cpp#L698-L1085 (chrome/m156)
struct ActiveEdgeList {
    // nodes[TREE_HEAD] is fTreeHead; the rest are allocations in order.
    nodes: Vec<ActiveEdge>,
    max_free: usize,
}

impl ActiveEdgeList {
    // Port of: src/utils/SkPolyUtils.cpp#L700-L704 (chrome/m156)
    fn new(max_edges: usize) -> Self {
        let mut nodes = Vec::with_capacity(max_edges + 1);
        nodes.push(ActiveEdge::head());
        Self {
            nodes,
            max_free: max_edges,
        }
    }

    fn root(&self) -> Option<usize> {
        self.nodes[TREE_HEAD].child[1]
    }

    fn set_root(&mut self, root: Option<usize>) {
        self.nodes[TREE_HEAD].child[1] = root;
    }

    // Port of: src/utils/SkPolyUtils.cpp#L1000-L1007 (chrome/m156), `allocate`
    fn allocate(&mut self, p0: Point, p1: Point, index0: u16, index1: u16) -> Option<usize> {
        if self.nodes.len() > self.max_free {
            return None;
        }
        self.nodes.push(ActiveEdge::new(p0, p1, index0, index1));
        Some(self.nodes.len() - 1)
    }

    fn is_red(&self, node: Option<usize>) -> bool {
        node.is_some_and(|n| self.nodes[n].red)
    }

    fn equals(&self, node: usize, index0: u16, index1: u16) -> bool {
        self.nodes[node].index0 == index0 && self.nodes[node].index1 == index1
    }

    // Port of: src/utils/SkPolyUtils.cpp#L635-L672 (chrome/m156), `ActiveEdge::intersect`
    fn intersect_segment(
        &self,
        node: usize,
        q0: Point,
        w: Vector,
        index0: u16,
        index1: u16,
    ) -> bool {
        let edge = &self.nodes[node];
        // check first to see if these edges are neighbors in the polygon
        if edge.index0 == index0
            || edge.index1 == index0
            || edge.index0 == index1
            || edge.index1 == index1
        {
            return false;
        }

        // We don't need the exact intersection point so we can do a simpler test here.
        let p0 = edge.segment.p0;
        let v = edge.segment.v;
        let p1 = p0 + v;
        let q1 = q0 + w;

        // We assume some x-overlap due to how the edgelist works
        // This allows us to simplify our test
        debug_assert!(q0.x <= p1.x + SCALAR_NEARLY_ZERO);

        // if each segment straddles the other (i.e., the endpoints have different sides)
        // then they intersect
        if p0.x < q0.x {
            if q1.x < p1.x {
                compute_side(p0, v, q0) * compute_side(p0, v, q1) < 0
            } else {
                compute_side(p0, v, q0) * compute_side(q0, w, p1) > 0
            }
        } else if p1.x < q1.x {
            compute_side(q0, w, p0) * compute_side(q0, w, p1) < 0
        } else {
            compute_side(q0, w, p0) * compute_side(p0, v, q1) > 0
        }
    }

    // `ActiveEdge::intersect(const ActiveEdge* edge)` with `edge` an arena node.
    // Port of: src/utils/SkPolyUtils.cpp#L674-L676 (chrome/m156)
    fn intersect_node(&self, node: usize, edge: usize) -> bool {
        let e = &self.nodes[edge];
        self.intersect_segment(node, e.segment.p0, e.segment.v, e.index0, e.index1)
    }

    // Port of: src/utils/SkPolyUtils.cpp#L710-L825 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the single C++ function
    fn insert(&mut self, p0: Point, p1: Point, index0: u16, index1: u16) -> bool {
        let v = p1 - p0;
        if !v.is_finite() {
            return false;
        }
        // empty tree case -- easy
        if self.root().is_none() {
            let root = self.allocate(p0, v, index0, index1);
            self.set_root(root);
            let Some(root) = root else {
                return false;
            };
            self.nodes[root].red = false;
            return true;
        }

        // set up helpers
        let mut top = TREE_HEAD;
        let mut grandparent: Option<usize> = None;
        let mut parent: Option<usize> = None;
        let mut curr: Option<usize> = self.root();
        let mut dir: usize = 0;
        // The initial value is overwritten before any read, since the loop always runs at least
        // once with a non-null `curr` (the tree has a root).
        #[allow(unused_assignments)]
        let mut last: usize = 0; // ?
        // predecessor and successor, for intersection check
        let mut pred: Option<usize> = None;
        let mut succ: Option<usize> = None;

        // search down the tree
        loop {
            let Some(cur) = curr else {
                // check for intersection with predecessor and successor
                if pred.is_some_and(|n| self.intersect_segment(n, p0, v, index0, index1))
                    || succ.is_some_and(|n| self.intersect_segment(n, p0, v, index0, index1))
                {
                    return false;
                }
                // insert new node at bottom
                let new_node = self.allocate(p0, v, index0, index1);
                let parent_index = parent.expect("the tree has a root, so there is a parent");
                self.nodes[parent_index].child[dir] = new_node;
                let Some(c) = new_node else {
                    return false;
                };
                self.nodes[c].above = pred;
                self.nodes[c].below = succ;
                if let Some(p) = pred {
                    if self.nodes[p].segment.p0 == self.nodes[c].segment.p0
                        && self.nodes[p].segment.v == self.nodes[c].segment.v
                    {
                        return false;
                    }
                    self.nodes[p].below = Some(c);
                }
                if let Some(s) = succ {
                    if self.nodes[s].segment.p0 == self.nodes[c].segment.p0
                        && self.nodes[s].segment.v == self.nodes[c].segment.v
                    {
                        return false;
                    }
                    self.nodes[s].above = Some(c);
                }
                if self.is_red(parent) {
                    let dir2 = usize::from(self.nodes[top].child[1] == grandparent);
                    let gp = grandparent.expect("a red parent has a grandparent");
                    if Some(c) == self.nodes[parent_index].child[last] {
                        let rotated = self.single_rotation(gp, 1 - last);
                        self.nodes[top].child[dir2] = Some(rotated);
                    } else {
                        let rotated = self.double_rotation(gp, 1 - last);
                        self.nodes[top].child[dir2] = Some(rotated);
                    }
                }
                break;
            };

            if self.is_red(self.nodes[cur].child[0]) && self.is_red(self.nodes[cur].child[1]) {
                // color flip
                self.nodes[cur].red = true;
                if let Some(c0) = self.nodes[cur].child[0] {
                    self.nodes[c0].red = false;
                }
                if let Some(c1) = self.nodes[cur].child[1] {
                    self.nodes[c1].red = false;
                }
                if self.is_red(parent) {
                    let dir2 = usize::from(self.nodes[top].child[1] == grandparent);
                    let gp = grandparent.expect("a red parent has a grandparent");
                    let parent_index = parent.expect("a red parent exists");
                    if Some(cur) == self.nodes[parent_index].child[last] {
                        let rotated = self.single_rotation(gp, 1 - last);
                        self.nodes[top].child[dir2] = Some(rotated);
                    } else {
                        let rotated = self.double_rotation(gp, 1 - last);
                        self.nodes[top].child[dir2] = Some(rotated);
                    }
                }
            }

            last = dir;
            // check to see if segment is above or below
            let side = if self.nodes[cur].index0 == index0 {
                compute_side(self.nodes[cur].segment.p0, self.nodes[cur].segment.v, p1)
            } else {
                compute_side(self.nodes[cur].segment.p0, self.nodes[cur].segment.v, p0)
            };
            if 0 == side {
                return false;
            }
            dir = usize::from(side < 0);

            if 0 == dir {
                succ = Some(cur);
            } else {
                pred = Some(cur);
            }

            // update helpers
            if let Some(gp) = grandparent {
                top = gp;
            }
            grandparent = parent;
            parent = Some(cur);
            curr = self.nodes[cur].child[dir];
        }

        // update root and make it black
        if let Some(root) = self.root() {
            self.nodes[root].red = false;
        }

        true
    }

    // replaces edge p0p1 with p1p2
    // Port of: src/utils/SkPolyUtils.cpp#L828-L885 (chrome/m156)
    fn replace(
        &mut self,
        p0: Point,
        p1: Point,
        p2: Point,
        index0: u16,
        index1: u16,
        index2: u16,
    ) -> bool {
        if self.root().is_none() {
            return false;
        }

        let v = p2 - p1;
        let mut curr = TREE_HEAD;
        let mut found: Option<usize> = None;
        let mut dir: usize = 1;

        // search
        while let Some(next) = self.nodes[curr].child[dir] {
            // update helpers
            curr = next;
            // save found node
            if self.equals(curr, index0, index1) {
                found = Some(curr);
                break;
            }
            // check to see if segment is above or below
            let seg = self.nodes[curr].segment;
            let side = if self.nodes[curr].index1 == index1 {
                compute_side(seg.p0, seg.v, p0)
            } else {
                compute_side(seg.p0, seg.v, p1)
            };
            if 0 == side {
                return false;
            }
            dir = usize::from(side < 0);
        }

        let Some(found) = found else {
            return false;
        };

        // replace if found
        let pred = self.nodes[found].above;
        let succ = self.nodes[found].below;
        // check deletion and insert intersection cases
        if pred.is_some_and(|n| {
            self.intersect_node(n, found) || self.intersect_segment(n, p1, v, index1, index2)
        }) {
            return false;
        }
        if succ.is_some_and(|n| {
            self.intersect_node(n, found) || self.intersect_segment(n, p1, v, index1, index2)
        }) {
            return false;
        }
        self.nodes[found].segment.p0 = p1;
        self.nodes[found].segment.v = v;
        self.nodes[found].index0 = index1;
        self.nodes[found].index1 = index2;
        // above and below should stay the same

        true
    }

    // Port of: src/utils/SkPolyUtils.cpp#L887-L996 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the single C++ function
    fn remove(&mut self, p0: Point, p1: Point, index0: u16, index1: u16) -> bool {
        if self.root().is_none() {
            return false;
        }

        let mut curr = TREE_HEAD;
        let mut parent: Option<usize> = None;
        let mut grandparent: Option<usize>;
        let mut found: Option<usize> = None;
        let mut dir: usize = 1;

        // search and push a red node down
        while let Some(next) = self.nodes[curr].child[dir] {
            let last = dir;

            // update helpers
            grandparent = parent;
            parent = Some(curr);
            curr = next;
            // save found node
            if self.equals(curr, index0, index1) {
                found = Some(curr);
                dir = 0;
            } else {
                // check to see if segment is above or below
                let seg = self.nodes[curr].segment;
                let side = if self.nodes[curr].index1 == index1 {
                    compute_side(seg.p0, seg.v, p0)
                } else {
                    compute_side(seg.p0, seg.v, p1)
                };
                if 0 == side {
                    return false;
                }
                dir = usize::from(side < 0);
            }

            // push the red node down
            let child_dir = self.nodes[curr].child[dir];
            if !self.is_red(Some(curr)) && !self.is_red(child_dir) {
                let other = self.nodes[curr].child[1 - dir];
                if self.is_red(other) {
                    let rotated = self.single_rotation(curr, dir);
                    let parent_index = parent.expect("the head is never the rotated node");
                    self.nodes[parent_index].child[last] = Some(rotated);
                    parent = Some(rotated);
                } else {
                    let parent_index = parent.expect("the head is never the rotated node");
                    let s = self.nodes[parent_index].child[1 - last];

                    if let Some(s) = s {
                        if !self.is_red(self.nodes[s].child[1 - last])
                            && !self.is_red(self.nodes[s].child[last])
                        {
                            // color flip
                            self.nodes[parent_index].red = false;
                            self.nodes[s].red = true;
                            self.nodes[curr].red = true;
                        } else {
                            let gp = grandparent.expect("a sibling implies a grandparent");
                            let dir2 = usize::from(self.nodes[gp].child[1] == parent);

                            if self.is_red(self.nodes[s].child[last]) {
                                let rotated = self.double_rotation(parent_index, last);
                                self.nodes[gp].child[dir2] = Some(rotated);
                            } else if self.is_red(self.nodes[s].child[1 - last]) {
                                let rotated = self.single_rotation(parent_index, last);
                                self.nodes[gp].child[dir2] = Some(rotated);
                            }

                            // ensure correct coloring
                            let new_top = self.nodes[gp].child[dir2]
                                .expect("rotation result is linked into the grandparent");
                            self.nodes[curr].red = true;
                            self.nodes[new_top].red = true;
                            if let Some(c0) = self.nodes[new_top].child[0] {
                                self.nodes[c0].red = false;
                            }
                            if let Some(c1) = self.nodes[new_top].child[1] {
                                self.nodes[c1].red = false;
                            }
                        }
                    }
                }
            }
        }

        // replace and remove if found
        if let Some(found) = found {
            let mut pred = self.nodes[found].above;
            let succ = self.nodes[found].below;
            if pred.is_some_and(|n| self.intersect_node(n, found))
                || succ.is_some_and(|n| self.intersect_node(n, found))
            {
                return false;
            }
            if found != curr {
                self.nodes[found].segment = self.nodes[curr].segment;
                self.nodes[found].index0 = self.nodes[curr].index0;
                self.nodes[found].index1 = self.nodes[curr].index1;
                self.nodes[found].above = self.nodes[curr].above;
                pred = self.nodes[found].above;
                // we don't need to set found->fBelow here
            } else if let Some(s) = succ {
                self.nodes[s].above = pred;
            }
            if let Some(p) = pred {
                self.nodes[p].below = self.nodes[curr].below;
            }
            let parent_index = parent.expect("a found node has a parent");
            let slot = usize::from(self.nodes[parent_index].child[1] == Some(curr));
            // `curr->fChild[!curr->fChild[0]]`: the child that is not null, or the right one.
            let replacement =
                self.nodes[curr].child[usize::from(self.nodes[curr].child[0].is_none())];
            self.nodes[parent_index].child[slot] = replacement;

            // no need to delete (Skia stores 0xdeadbeef sentinels here, which nothing reads)
            self.nodes[curr].above = None;
            self.nodes[curr].below = None;
            if let Some(root) = self.root() {
                self.nodes[root].red = false;
            }
        }

        // update root and make it black
        if let Some(root) = self.root() {
            self.nodes[root].red = false;
        }

        true
    }

    // Red-black tree methods
    // Port of: src/utils/SkPolyUtils.cpp#L1016-L1026 (chrome/m156), `SingleRotation`
    fn single_rotation(&mut self, node: usize, dir: usize) -> usize {
        let tmp = self.nodes[node].child[1 - dir].expect("rotation has a child to lift");

        self.nodes[node].child[1 - dir] = self.nodes[tmp].child[dir];
        self.nodes[tmp].child[dir] = Some(node);

        self.nodes[node].red = true;
        self.nodes[tmp].red = false;

        tmp
    }

    // Port of: src/utils/SkPolyUtils.cpp#L1028-L1032 (chrome/m156), `DoubleRotation`
    fn double_rotation(&mut self, node: usize, dir: usize) -> usize {
        let inner = self.nodes[node].child[1 - dir].expect("rotation has a child to lift");
        let rotated = self.single_rotation(inner, 1 - dir);
        self.nodes[node].child[1 - dir] = Some(rotated);

        self.single_rotation(node, dir)
    }
}

/// Determines whether a polygon is simple (i.e., not self-intersecting).
///
/// The input polygon must have no coincident vertices or the test will fail. It is also expected
/// to have values clamped to the nearest 1/16th.
///
/// Here a sweep line algorithm is used. The vertices are inserted into a priority queue sorting
/// horizontally from left to right. Popping them generates events that add or remove edges from
/// an edge list; any intersection found in the edge list means the polygon is not simple.
// Port of: src/utils/SkPolyUtils.cpp#L1093-L1174 (chrome/m156)
#[doc(alias = "SkIsSimplePolygon")]
#[must_use]
pub fn is_simple_polygon(polygon: &[Point]) -> bool {
    let polygon_size = polygon.len();
    if polygon_size < 3 {
        return false;
    }

    // If it's convex, it's simple
    if is_convex_polygon(polygon) {
        return true;
    }

    // practically speaking, it takes too long to process large polygons
    if polygon_size > 2048 {
        return false;
    }

    let mut vertex_queue = VertexQueue::with_capacity(polygon_size);
    for (i, &point) in polygon.iter().enumerate() {
        if !point.is_finite() {
            return false;
        }
        let prev_index = (i + polygon_size - 1) % polygon_size;
        let next_index = (i + 1) % polygon_size;
        let mut new_vertex = Vertex {
            position: point,
            index: to_index(i),
            prev_index: to_index(prev_index),
            next_index: to_index(next_index),
            flags: 0,
        };
        // The two edges adjacent to this vertex are the same, so polygon is not simple
        if polygon[prev_index] == polygon[next_index] {
            return false;
        }
        if left(polygon[prev_index], point) {
            new_vertex.flags |= K_PREV_LEFT_VERTEX_FLAG;
        }
        if left(polygon[next_index], point) {
            new_vertex.flags |= K_NEXT_LEFT_VERTEX_FLAG;
        }
        vertex_queue.insert(new_vertex);
    }

    // pop each vertex from the queue and generate events depending on
    // where it lies relative to its neighboring edges
    let mut sweep_line = ActiveEdgeList::new(polygon_size);
    while vertex_queue.count() > 0 {
        let v = *vertex_queue.peek();
        let prev = usize::from(v.prev_index);
        let next = usize::from(v.next_index);

        // both to the right -- insert both
        if v.flags == 0 {
            if !sweep_line.insert(v.position, polygon[prev], v.index, v.prev_index) {
                break;
            }
            if !sweep_line.insert(v.position, polygon[next], v.index, v.next_index) {
                break;
            }
        // both to the left -- remove both
        } else if v.flags == (K_PREV_LEFT_VERTEX_FLAG | K_NEXT_LEFT_VERTEX_FLAG) {
            if !sweep_line.remove(polygon[prev], v.position, v.prev_index, v.index) {
                break;
            }
            if !sweep_line.remove(polygon[next], v.position, v.next_index, v.index) {
                break;
            }
        // one to left and right -- replace one with another
        } else if v.flags & K_PREV_LEFT_VERTEX_FLAG != 0 {
            if !sweep_line.replace(
                polygon[prev],
                v.position,
                polygon[next],
                v.prev_index,
                v.index,
                v.next_index,
            ) {
                break;
            }
        } else {
            debug_assert!(v.flags & K_NEXT_LEFT_VERTEX_FLAG != 0);
            if !sweep_line.replace(
                polygon[next],
                v.position,
                polygon[prev],
                v.next_index,
                v.index,
                v.prev_index,
            ) {
                break;
            }
        }

        vertex_queue.pop();
    }

    vertex_queue.count() == 0
}

///////////////////////////////////////////////////////////////////////////////////////////

// helper function for SkOffsetSimplePolygon
// Port of: src/utils/SkPolyUtils.cpp#L1179-L1185 (chrome/m156)
fn setup_offset_edge(
    curr_edge: &mut OffsetEdge,
    endpoint0: Point,
    endpoint1: Point,
    start_index: u16,
    end_index: u16,
) {
    curr_edge.offset.p0 = endpoint0;
    curr_edge.offset.v = endpoint1 - endpoint0;
    curr_edge.init(start_index, end_index);
}

// Port of: src/utils/SkPolyUtils.cpp#L1187-L1194 (chrome/m156)
fn is_reflex_vertex(
    input_polygon_verts: &[Point],
    winding: i32,
    offset: scalar,
    prev_index: usize,
    curr_index: usize,
    next_index: usize,
) -> bool {
    let side = compute_side(
        input_polygon_verts[prev_index],
        input_polygon_verts[curr_index] - input_polygon_verts[prev_index],
        input_polygon_verts[next_index],
    );
    // if reflex point, we need to add extra edges
    int_to_scalar(side * winding) * offset < 0.0
}

/// Generates a simple polygon (if possible) that is offset a constant distance from the boundary
/// of a given simple polygon.
///
/// The input polygon must be simple, have no coincident vertices or collinear edges, and have
/// values clamped to the nearest 1/16th. `offset` is positive for insetting and negative for
/// outsetting. Appends the offset polygon to `offset_polygon` and, when `polygon_indices` is
/// given, the index in the input polygon of each offset vertex. Returns true if an offset simple
/// polygon exists.
///
/// # Panics
///
/// Does not panic for any input: the edge list is closed by construction, and the `expect`s check
/// that invariant.
// Port of: src/utils/SkPolyUtils.cpp#L1196-L1469 (chrome/m156)
#[doc(alias = "SkOffsetSimplePolygon")]
#[allow(clippy::too_many_lines)] // mirrors the single C++ function
pub fn offset_simple_polygon(
    input_polygon_verts: &[Point],
    bounds: &Rect,
    offset: scalar,
    offset_polygon: &mut Vec<Point>,
    mut polygon_indices: Option<&mut Vec<i32>>,
) -> bool {
    let input_polygon_size = input_polygon_verts.len();
    if input_polygon_size < 3 {
        return false;
    }

    // need to be able to represent all the vertices in the 16-bit indices
    if input_polygon_size >= U16_MAX {
        return false;
    }

    if !is_finite(offset) {
        return false;
    }

    // can't inset more than the half bounds of the polygon
    if offset
        > std_min(
            scalar_abs(rect_priv::half_width(bounds)),
            scalar_abs(rect_priv::half_height(bounds)),
        )
    {
        return false;
    }

    // offsetting close to zero just returns the original poly
    if offset.nearly_zero(SCALAR_NEARLY_ZERO) {
        for (i, &point) in input_polygon_verts.iter().enumerate() {
            offset_polygon.push(point);
            if let Some(indices) = polygon_indices.as_mut() {
                indices.push(i32::try_from(i).expect("polygon size fits in int"));
            }
        }
        return true;
    }

    // get winding direction
    let winding = get_polygon_winding(input_polygon_verts);
    if 0 == winding {
        return false;
    }

    // build normals
    let mut normals = vec![Vector::default(); input_polygon_size];
    let mut num_edges: usize = 0;
    let mut prev_index = input_polygon_size - 1;
    for curr_index in 0..input_polygon_size {
        if !input_polygon_verts[curr_index].is_finite() {
            return false;
        }
        let next_index = (curr_index + 1) % input_polygon_size;
        let Some(normal) = compute_offset_vector(
            input_polygon_verts[curr_index],
            input_polygon_verts[next_index],
            offset,
            winding,
        ) else {
            return false;
        };
        normals[curr_index] = normal;
        if curr_index > 0 {
            // if reflex point, we need to add extra edges
            if is_reflex_vertex(
                input_polygon_verts,
                winding,
                offset,
                prev_index,
                curr_index,
                next_index,
            ) {
                let Some((_, _, num_steps)) =
                    compute_radial_steps(normals[prev_index], normals[curr_index], offset)
                else {
                    return false;
                };
                num_edges += usize::try_from(num_steps.max(1)).expect("steps are non-negative");
            }
        }
        num_edges += 1;
        prev_index = curr_index;
    }
    // finish up the edge counting
    if is_reflex_vertex(
        input_polygon_verts,
        winding,
        offset,
        input_polygon_size - 1,
        0,
        1,
    ) {
        let Some((_, _, num_steps)) =
            compute_radial_steps(normals[input_polygon_size - 1], normals[0], offset)
        else {
            return false;
        };
        num_edges += usize::try_from(num_steps.max(1)).expect("steps are non-negative");
    }

    // Make sure we don't overflow the max array count.
    // We shouldn't overflow numEdges, as SkComputeRadialSteps returns a max of 2^16-1,
    // and we have a max of 2^16-1 original vertices.
    if num_edges > i32::MAX as usize {
        return false;
    }

    // build initial offset edge list
    let mut edge_data: Vec<OffsetEdge> = Vec::with_capacity(num_edges);
    let mut prev_edge: Option<usize> = None;
    let mut prev_index = input_polygon_size - 1;
    for curr_index in 0..input_polygon_size {
        let next_index = (curr_index + 1) % input_polygon_size;
        // if reflex point, fill in curve
        if is_reflex_vertex(
            input_polygon_verts,
            winding,
            offset,
            prev_index,
            curr_index,
            next_index,
        ) {
            let mut prev_normal = normals[prev_index];
            let Some((rot_sin, rot_cos, num_steps)) =
                compute_radial_steps(prev_normal, normals[curr_index], offset)
            else {
                return false;
            };
            let first = edge_data.len();
            edge_data.resize(
                first + usize::try_from(num_steps.max(1)).expect("non-negative"),
                OffsetEdge::default(),
            );
            let mut curr_edge = first;
            for _ in 0..(num_steps - 1) {
                let curr_normal = Vector::new(
                    prev_normal.x * rot_cos - prev_normal.y * rot_sin,
                    prev_normal.y * rot_cos + prev_normal.x * rot_sin,
                );
                setup_offset_edge(
                    &mut edge_data[curr_edge],
                    input_polygon_verts[curr_index] + prev_normal,
                    input_polygon_verts[curr_index] + curr_normal,
                    to_index(curr_index),
                    to_index(curr_index),
                );
                prev_normal = curr_normal;
                edge_data[curr_edge].prev = prev_edge;
                if let Some(pe) = prev_edge {
                    edge_data[pe].next = Some(curr_edge);
                }
                prev_edge = Some(curr_edge);
                curr_edge += 1;
            }
            setup_offset_edge(
                &mut edge_data[curr_edge],
                input_polygon_verts[curr_index] + prev_normal,
                input_polygon_verts[curr_index] + normals[curr_index],
                to_index(curr_index),
                to_index(curr_index),
            );
            edge_data[curr_edge].prev = prev_edge;
            if let Some(pe) = prev_edge {
                edge_data[pe].next = Some(curr_edge);
            }
            prev_edge = Some(curr_edge);
        }

        // Add the edge
        let curr_edge = edge_data.len();
        edge_data.push(OffsetEdge::default());
        setup_offset_edge(
            &mut edge_data[curr_edge],
            input_polygon_verts[curr_index] + normals[curr_index],
            input_polygon_verts[next_index] + normals[curr_index],
            to_index(curr_index),
            to_index(next_index),
        );
        edge_data[curr_edge].prev = prev_edge;
        if let Some(pe) = prev_edge {
            edge_data[pe].next = Some(curr_edge);
        }
        prev_edge = Some(curr_edge);
        prev_index = curr_index;
    }
    // close up the linked list
    debug_assert!(prev_edge.is_some());
    let Some(last_edge) = prev_edge else {
        return false;
    };
    edge_data[last_edge].next = Some(0);
    edge_data[0].prev = Some(last_edge);

    // now clip edges
    debug_assert_eq!(edge_data.len(), num_edges);
    let mut head: Option<usize> = Some(0);
    let mut curr_edge = 0usize;
    let mut prev_edge = last_edge;
    let mut offset_vertex_count = num_edges;
    let mut iterations: u64 = 0;
    let max_iterations = (num_edges as u64) * (num_edges as u64);
    while head.is_some() && prev_edge != curr_edge && offset_vertex_count > 0 {
        iterations += 1;
        // we should check each edge against each other edge at most once
        if iterations > max_iterations {
            return false;
        }

        if let Some((intersection, s, t)) =
            edge_data[prev_edge].check_intersection(&edge_data[curr_edge])
        {
            // if new intersection is further back on previous inset from the prior intersection
            if s < edge_data[prev_edge].t_value {
                // no point in considering this one again
                remove_node(&mut edge_data, prev_edge, &mut head);
                offset_vertex_count -= 1;
                // go back one segment
                prev_edge = edge_data[prev_edge]
                    .prev
                    .expect("offset edges form a closed list");
            // we've already considered this intersection, we're done
            } else if edge_data[curr_edge].t_value > SCALAR_MIN
                && point_priv::equals_within_tolerance_tol(
                    intersection,
                    edge_data[curr_edge].intersection,
                    1.0e-6,
                )
            {
                break;
            } else {
                // add intersection
                edge_data[curr_edge].intersection = intersection;
                edge_data[curr_edge].t_value = t;
                edge_data[curr_edge].index = edge_data[prev_edge].end;

                // go to next segment
                prev_edge = curr_edge;
                curr_edge = edge_data[curr_edge]
                    .next
                    .expect("offset edges form a closed list");
            }
        } else {
            // If there is no intersection, we want to minimize the distance between
            // the point where the segment lines cross and the segments themselves.
            let prev_prev_edge = edge_data[prev_edge]
                .prev
                .expect("offset edges form a closed list");
            let curr_next_edge = edge_data[curr_edge]
                .next
                .expect("offset edges form a closed list");
            let dist0 = edge_data[curr_edge].compute_crossing_distance(&edge_data[prev_prev_edge]);
            let dist1 = edge_data[prev_edge].compute_crossing_distance(&edge_data[curr_next_edge]);
            // if both lead to direct collision
            if dist0 < 0.0 && dist1 < 0.0 {
                // check first to see if either represent parts of one contour
                let mut p1 =
                    edge_data[prev_prev_edge].offset.p0 + edge_data[prev_prev_edge].offset.v;
                let prev_same_contour =
                    point_priv::equals_within_tolerance(p1, edge_data[prev_edge].offset.p0);
                p1 = edge_data[curr_edge].offset.p0 + edge_data[curr_edge].offset.v;
                let curr_same_contour =
                    point_priv::equals_within_tolerance(p1, edge_data[curr_next_edge].offset.p0);

                // want to step along contour to find intersections rather than jump to new one
                if curr_same_contour && !prev_same_contour {
                    remove_node(&mut edge_data, curr_edge, &mut head);
                    curr_edge = curr_next_edge;
                    offset_vertex_count -= 1;
                    continue;
                } else if prev_same_contour && !curr_same_contour {
                    remove_node(&mut edge_data, prev_edge, &mut head);
                    prev_edge = prev_prev_edge;
                    offset_vertex_count -= 1;
                    continue;
                }
            }

            // otherwise minimize collision distance along segment
            if dist0 < dist1 {
                remove_node(&mut edge_data, prev_edge, &mut head);
                prev_edge = prev_prev_edge;
            } else {
                remove_node(&mut edge_data, curr_edge, &mut head);
                curr_edge = curr_next_edge;
            }
            offset_vertex_count -= 1;
        }
    }

    // store all the valid intersections that aren't nearly coincident
    // TODO: look at the main algorithm and see if we can detect these better
    offset_polygon.clear();
    let Some(head) = head else {
        return false;
    };
    if offset_vertex_count == 0 || offset_vertex_count >= U16_MAX {
        return false;
    }

    offset_polygon.reserve(offset_vertex_count);
    let mut curr_index = 0usize;
    offset_polygon.push(edge_data[head].intersection);
    if let Some(indices) = polygon_indices.as_mut() {
        indices.push(i32::from(edge_data[head].index));
    }
    let mut curr = edge_data[head]
        .next
        .expect("offset edges form a closed list");
    while curr != head {
        if !point_priv::equals_within_tolerance_tol(
            edge_data[curr].intersection,
            offset_polygon[curr_index],
            K_CLEANUP_TOLERANCE,
        ) {
            offset_polygon.push(edge_data[curr].intersection);
            if let Some(indices) = polygon_indices.as_mut() {
                indices.push(i32::from(edge_data[curr].index));
            }
            curr_index += 1;
        }
        curr = edge_data[curr]
            .next
            .expect("offset edges form a closed list");
    }
    // make sure the first and last points aren't coincident
    if curr_index >= 1
        && point_priv::equals_within_tolerance_tol(
            offset_polygon[0],
            offset_polygon[curr_index],
            K_CLEANUP_TOLERANCE,
        )
    {
        offset_polygon.pop();
        if let Some(indices) = polygon_indices.as_mut() {
            indices.pop();
        }
    }

    // check winding of offset polygon (it should be same as the original polygon)
    let offset_winding = get_polygon_winding(offset_polygon);

    winding * offset_winding > 0 && is_simple_polygon(offset_polygon)
}

//////////////////////////////////////////////////////////////////////////////////////////

// Port of: src/utils/SkPolyUtils.cpp#L1473-L1483 (chrome/m156)
#[derive(Copy, Clone, Debug)]
struct TriangulationVertex {
    position: Point,
    vertex_type: VertexType,
    index: u16,
    prev_index: u16,
    next_index: u16,
    // The intrusive list links (`SK_DECLARE_INTERNAL_LLIST_INTERFACE`): a vertex is in at most one
    // of the convex list and a reflex grid cell at a time.
    prev: Option<usize>,
    next: Option<usize>,
}

// Port of: src/utils/SkPolyUtils.cpp#L1476 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum VertexType {
    Convex,
    Reflex,
}

impl Default for TriangulationVertex {
    // `TriangulationVertex{}`: zero-initialised, so the vertex type is `kConvex`.
    fn default() -> Self {
        Self {
            position: Point::default(),
            vertex_type: VertexType::Convex,
            index: 0,
            prev_index: 0,
            next_index: 0,
            prev: None,
            next: None,
        }
    }
}

// An intrusive doubly linked list over `TriangulationVertex` arena entries (`SkTInternalLList`).
// Port of: src/core/SkTInternalLList.h#L30-L110 (chrome/m156)
#[derive(Copy, Clone, Debug, Default)]
struct VertexList {
    head: Option<usize>,
    tail: Option<usize>,
}

impl VertexList {
    // Port of: src/core/SkTInternalLList.h#L38-L62 (chrome/m156), `remove`
    fn remove(&mut self, verts: &mut [TriangulationVertex], entry: usize) {
        let prev = verts[entry].prev;
        let next = verts[entry].next;
        if let Some(p) = prev {
            verts[p].next = next;
        } else {
            self.head = next;
        }
        if let Some(n) = next {
            verts[n].prev = prev;
        } else {
            self.tail = prev;
        }
        verts[entry].prev = None;
        verts[entry].next = None;
    }

    // Port of: src/core/SkTInternalLList.h#L64-L81 (chrome/m156), `addToHead`
    fn add_to_head(&mut self, verts: &mut [TriangulationVertex], entry: usize) {
        verts[entry].prev = None;
        verts[entry].next = self.head;
        if let Some(h) = self.head {
            verts[h].prev = Some(entry);
        }
        self.head = Some(entry);
        if self.tail.is_none() {
            self.tail = Some(entry);
        }
    }

    // Port of: src/core/SkTInternalLList.h#L83-L100 (chrome/m156), `addToTail`
    fn add_to_tail(&mut self, verts: &mut [TriangulationVertex], entry: usize) {
        verts[entry].prev = self.tail;
        verts[entry].next = None;
        if let Some(t) = self.tail {
            verts[t].next = Some(entry);
        }
        self.tail = Some(entry);
        if self.head.is_none() {
            self.head = Some(entry);
        }
    }
}

// Port of: src/utils/SkPolyUtils.cpp#L1485-L1494 (chrome/m156)
fn compute_triangle_bounds(p0: Point, p1: Point, p2: Point) -> Rect {
    // The four lanes are (x0, y0, x0, y0) against (x1, y1, x2, y2), as in the float4 of Skia.
    let mut min = [p0.x, p0.y, p0.x, p0.y];
    let mut max = [p0.x, p0.y, p0.x, p0.y];
    let xy = [p1.x, p1.y, p2.x, p2.y];
    for lane in 0..4 {
        // skvx::min and skvx::max are per-lane `a < b ? a : b` and `a > b ? a : b`.
        min[lane] = if min[lane] < xy[lane] {
            min[lane]
        } else {
            xy[lane]
        };
        max[lane] = if max[lane] > xy[lane] {
            max[lane]
        } else {
            xy[lane]
        };
    }
    Rect {
        left: std_min(min[0], min[2]),
        top: std_min(min[1], min[3]),
        right: std_max(max[0], max[2]),
        bottom: std_max(max[1], max[3]),
    }
}

// test to see if point p is in triangle p0p1p2.
// for now assuming strictly inside -- if on the edge it's outside
// Port of: src/utils/SkPolyUtils.cpp#L1498-L1521 (chrome/m156)
fn point_in_triangle(p0: Point, p1: Point, p2: Point, p: Point) -> bool {
    let v0 = p1 - p0;
    let v1 = p2 - p1;
    let n = v0.cross(v1);

    let w0 = p - p0;
    if n * v0.cross(w0) < SCALAR_NEARLY_ZERO {
        return false;
    }

    let w1 = p - p1;
    if n * v1.cross(w1) < SCALAR_NEARLY_ZERO {
        return false;
    }

    let v2 = p0 - p2;
    let w2 = p - p2;
    if n * v2.cross(w2) < SCALAR_NEARLY_ZERO {
        return false;
    }

    true
}

// Data structure to track reflex vertices and check whether any are inside a given triangle
// Port of: src/utils/SkPolyUtils.cpp#L1524-L1615 (chrome/m156)
#[derive(Default)]
struct ReflexHash {
    bounds: Rect,
    h_count: i32,
    v_count: i32,
    num_verts: i32,
    // converts distance from the origin to a grid location (when cast to int)
    grid_conversion: Vector,
    grid: Vec<VertexList>,
}

impl ReflexHash {
    // Port of: src/utils/SkPolyUtils.cpp#L1526-L1554 (chrome/m156)
    fn init(&mut self, bounds: Rect, vertex_count: usize) -> bool {
        self.bounds = bounds;
        self.num_verts = 0;
        let width = bounds.width();
        let height = bounds.height();
        if !is_finite_all(width, &[height]) {
            return false;
        }

        // We want vertexCount grid cells, roughly distributed to match the bounds ratio
        let vertex_count = i32::try_from(vertex_count).expect("polygon size fits in int");
        #[allow(clippy::cast_precision_loss)] // mirrors `vertexCount*width` (int promoted to float)
        let h_count_f = scalar_sqrt(ieee_float_divide(vertex_count as scalar * width, height));
        if !is_finite(h_count_f) {
            return false;
        }
        self.h_count = max(min(float_round2int(h_count_f), vertex_count), 1);
        self.v_count = vertex_count / self.h_count;
        #[allow(clippy::cast_precision_loss)] // mirrors `fHCount - 0.001f` (int promoted to float)
        let (h_count_s, v_count_s) = (self.h_count as scalar, self.v_count as scalar);
        self.grid_conversion = Vector::new(
            ieee_float_divide(h_count_s - 0.001, width),
            ieee_float_divide(v_count_s - 0.001, height),
        );
        if !self.grid_conversion.is_finite() {
            return false;
        }

        // Both counts are at least one: h_count <= vertex_count and v_count = vertex_count / h_count.
        #[allow(clippy::cast_sign_loss)] // grid dimensions are positive
        let cells = (self.h_count * self.v_count) as usize;
        self.grid = vec![VertexList::default(); cells];

        true
    }

    // Port of: src/utils/SkPolyUtils.cpp#L1556-L1560 (chrome/m156)
    fn add(&mut self, verts: &mut [TriangulationVertex], v: usize) {
        let index = self.hash(verts[v].position);
        self.grid[index].add_to_tail(verts, v);
        self.num_verts += 1;
    }

    // Port of: src/utils/SkPolyUtils.cpp#L1562-L1566 (chrome/m156)
    fn remove(&mut self, verts: &mut [TriangulationVertex], v: usize) {
        let index = self.hash(verts[v].position);
        self.grid[index].remove(verts, v);
        self.num_verts -= 1;
    }

    // Port of: src/utils/SkPolyUtils.cpp#L1568-L1598 (chrome/m156)
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // (int) grid cells, in bounds
    fn check_triangle(
        &self,
        verts: &[TriangulationVertex],
        p0: Point,
        p1: Point,
        p2: Point,
        ignore_index0: u16,
        ignore_index1: u16,
    ) -> bool {
        if self.num_verts == 0 {
            return false;
        }

        let tri_bounds = compute_triangle_bounds(p0, p1, p2);
        // (int) truncation of values inside the grid, as in Skia
        let h0 = ((tri_bounds.left - self.bounds.left) * self.grid_conversion.x) as i32;
        let h1 = ((tri_bounds.right - self.bounds.left) * self.grid_conversion.x) as i32;
        let v0 = ((tri_bounds.top - self.bounds.top) * self.grid_conversion.y) as i32;
        let v1 = ((tri_bounds.bottom - self.bounds.top) * self.grid_conversion.y) as i32;

        for v in v0..=v1 {
            for h in h0..=h1 {
                let i = v * self.h_count + h;
                let mut cursor = self.grid[i as usize].head;
                while let Some(reflex) = cursor {
                    let rv = &verts[reflex];
                    if rv.index != ignore_index0
                        && rv.index != ignore_index1
                        && point_in_triangle(p0, p1, p2, rv.position)
                    {
                        return true;
                    }
                    cursor = rv.next;
                }
            }
        }

        false
    }

    // Port of: src/utils/SkPolyUtils.cpp#L1601-L1606 (chrome/m156)
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // (int) grid cells, in bounds
    fn hash(&self, position: Point) -> usize {
        let h = ((position.x - self.bounds.left) * self.grid_conversion.x) as i32;
        let v = ((position.y - self.bounds.top) * self.grid_conversion.y) as i32;
        debug_assert!(v * self.h_count + h >= 0);
        (v * self.h_count + h) as usize
    }
}

// Check to see if a reflex vertex has become a convex vertex after clipping an ear
// Port of: src/utils/SkPolyUtils.cpp#L1618-L1631 (chrome/m156)
fn reclassify_vertex(
    p: usize,
    verts: &mut [TriangulationVertex],
    polygon_verts: &[Point],
    winding: i32,
    reflex_hash: &mut ReflexHash,
    convex_list: &mut VertexList,
) {
    if VertexType::Reflex == verts[p].vertex_type {
        let v0 = verts[p].position - polygon_verts[usize::from(verts[p].prev_index)];
        let v1 = polygon_verts[usize::from(verts[p].next_index)] - verts[p].position;
        if int_to_scalar(winding) * v0.cross(v1) > K_NEARLY_ZERO_SQUARED {
            verts[p].vertex_type = VertexType::Convex;
            reflex_hash.remove(verts, p);
            verts[p].prev = None;
            verts[p].next = None;
            convex_list.add_to_tail(verts, p);
        }
    }
}

/// Computes indices to triangulate the given polygon (ear clipping).
///
/// The input polygon must be simple (i.e. not self-intersecting) and have no coincident vertices
/// or collinear edges. `index_map` maps each index of `polygon_verts` to the final index in the
/// triangulation. Appends the triangle indices to `triangle_indices` and returns true if
/// successful.
// Port of: src/utils/SkPolyUtils.cpp#L1633-L1772 (chrome/m156)
#[doc(alias = "SkTriangulateSimplePolygon")]
#[allow(clippy::too_many_lines)] // mirrors the single C++ function
pub fn triangulate_simple_polygon(
    polygon_verts: &[Point],
    index_map: &[u16],
    triangle_indices: &mut Vec<u16>,
) -> bool {
    let polygon_size = polygon_verts.len();
    if polygon_size < 3 {
        return false;
    }
    // need to be able to represent all the vertices in the 16-bit indices
    if polygon_size >= U16_MAX {
        return false;
    }

    // get bounds
    let mut bounds = Rect::new_empty();
    if !bounds.set_bounds_check(polygon_verts) {
        return false;
    }
    // get winding direction
    let winding = get_polygon_winding(polygon_verts);
    if 0 == winding {
        return false;
    }

    // Set up vertices
    let mut verts = vec![TriangulationVertex::default(); polygon_size];
    let mut prev_index = polygon_size - 1;
    let mut v0 = polygon_verts[0] - polygon_verts[prev_index];
    for curr_index in 0..polygon_size {
        let next_index = (curr_index + 1) % polygon_size;

        verts[curr_index].position = polygon_verts[curr_index];
        verts[curr_index].index = to_index(curr_index);
        verts[curr_index].prev_index = to_index(prev_index);
        verts[curr_index].next_index = to_index(next_index);
        let v1 = polygon_verts[next_index] - polygon_verts[curr_index];
        verts[curr_index].vertex_type =
            if int_to_scalar(winding) * v0.cross(v1) > K_NEARLY_ZERO_SQUARED {
                VertexType::Convex
            } else {
                VertexType::Reflex
            };

        prev_index = curr_index;
        v0 = v1;
    }

    // Classify initial vertices into a list of convex vertices and a hash of reflex vertices
    // TODO: possibly sort the convexList in some way to get better triangles
    let mut convex_list = VertexList::default();
    let mut reflex_hash = ReflexHash::default();
    if !reflex_hash.init(bounds, polygon_size) {
        return false;
    }
    prev_index = polygon_size - 1;
    for curr_index in 0..polygon_size {
        let curr_type = verts[curr_index].vertex_type;
        if VertexType::Convex == curr_type {
            let next_index = (curr_index + 1) % polygon_size;
            let prev_type = verts[prev_index].vertex_type;
            let next_type = verts[next_index].vertex_type;
            // We prioritize clipping vertices with neighboring reflex vertices.
            // The intent here is that it will cull reflex vertices more quickly.
            if VertexType::Reflex == prev_type || VertexType::Reflex == next_type {
                convex_list.add_to_head(&mut verts, curr_index);
            } else {
                convex_list.add_to_tail(&mut verts, curr_index);
            }
        } else {
            // We treat near collinear vertices as reflex
            reflex_hash.add(&mut verts, curr_index);
        }
        prev_index = curr_index;
    }

    // The general concept: We are trying to find three neighboring vertices where
    // no other vertex lies inside the triangle (an "ear"). If we find one, we clip
    // that ear off, and then repeat on the new polygon. Once we get down to three vertices
    // we have triangulated the entire polygon.
    // In the worst case this is an n^2 algorithm. We can cut down the search space somewhat by
    // noting that only convex vertices can be potential ears, and we only need to check whether
    // any reflex vertices lie inside the ear.
    let mut vertex_count = polygon_size;
    while vertex_count > 3 {
        // find a convex vertex to clip: (ear, p0, p2)
        let mut ear = None;
        let mut convex_iter = convex_list.head;
        while let Some(candidate) = convex_iter {
            debug_assert_ne!(VertexType::Reflex, verts[candidate].vertex_type);

            let p0 = usize::from(verts[candidate].prev_index);
            let p2 = usize::from(verts[candidate].next_index);

            // see if any reflex vertices are inside the ear
            let failed = reflex_hash.check_triangle(
                &verts,
                verts[p0].position,
                verts[candidate].position,
                verts[p2].position,
                verts[p0].index,
                verts[p2].index,
            );
            if !failed {
                // found one we can clip
                ear = Some((candidate, p0, p2));
                break;
            }
            convex_iter = verts[candidate].next;
        }
        // If we can't find any ears to clip, this probably isn't a simple polygon
        let Some((ear_vertex, p0, p2)) = ear else {
            return false;
        };

        // add indices
        triangle_indices.push(index_map[usize::from(verts[p0].index)]);
        triangle_indices.push(index_map[usize::from(verts[ear_vertex].index)]);
        triangle_indices.push(index_map[usize::from(verts[p2].index)]);

        // clip the ear
        convex_list.remove(&mut verts, ear_vertex);
        vertex_count -= 1;

        // reclassify reflex verts
        verts[p0].next_index = verts[ear_vertex].next_index;
        reclassify_vertex(
            p0,
            &mut verts,
            polygon_verts,
            winding,
            &mut reflex_hash,
            &mut convex_list,
        );

        verts[p2].prev_index = verts[ear_vertex].prev_index;
        reclassify_vertex(
            p2,
            &mut verts,
            polygon_verts,
            winding,
            &mut reflex_hash,
            &mut convex_list,
        );
    }

    // output indices
    let mut vertex_iter = convex_list.head;
    while let Some(vertex) = vertex_iter {
        triangle_indices.push(index_map[usize::from(verts[vertex].index)]);
        vertex_iter = verts[vertex].next;
    }

    true
}
