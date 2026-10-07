// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkEdge.h, src/core/SkEdge.cpp

//! Edges for the non-antialiased scan converter (`SkEdge.h`).
//!
//! An edge approximates a monotonic curve with line segments in a way that makes computing scan
//! lines efficient. All arithmetic is fixed point and mirrors Skia exactly. Like the C++ it relies
//! on two's complement wrapping, so integer operations use the `wrapping_*` forms.
//!
//! `SK_RASTERIZE_EVEN_ROUNDING` is not defined in Skia's builds, so the `SkFloatToFDot6` branch
//! of each `set*` function is the one ported.

use std::ops::{Deref, DerefMut};

use skia_rust_core::fdot6::{
    Fdot6, fdot6_div, fdot6_round, fdot6_to_fixed, fixed_to_fdot6, float_to_fdot6,
};
use skia_rust_core::fixed::{Fixed, fixed_mul};
use skia_rust_core::math::left_shift;
use skia_rust_core::point::Point;
use skia_rust_core::rect::IRect;
use skia_rust_core::safe32::abs32;

/// `SkEdge_Compute_DY`: this correctly favors the lower-pixel when `y0` is on a 1/2 pixel
/// boundary.
// Port of: src/core/SkEdge.h#L24 (chrome/m156)
#[doc(alias = "SkEdge_Compute_DY")]
fn edge_compute_dy(top: i32, y0: Fdot6) -> Fdot6 {
    left_shift(top, 6).wrapping_add(32).wrapping_sub(y0)
}

/// The kind of curve an edge was created from (`SkEdge::Type`).
// Port of: src/core/SkEdge.h#L42-L46 (chrome/m156)
#[doc(alias = "SkEdge::Type")]
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
#[repr(i8)]
pub enum EdgeType {
    #[default]
    Line,
    Quad,
    Cubic,
}

/// The winding of an edge (`SkEdge::Winding`).
// Port of: src/core/SkEdge.h#L47-L50 (chrome/m156)
#[doc(alias = "SkEdge::Winding")]
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug)]
#[repr(i8)]
pub enum Winding {
    /// Clockwise.
    #[default]
    CW = 1,
    /// Counter clockwise.
    CCW = -1,
}

/// A line edge, and the state shared by the curve edges.
///
/// The current line segment starts at `(x, first_y + 0.5)`. It has slope `dx_dy` (run over rise,
/// because this is geared toward horizontal scanlines) and stops once Y gets to `last_y + 0.5`.
///
/// skia-rust: `fNext`/`fPrev` are indices into whatever array holds the edges (the C++ links
/// raw pointers); the scan converter decides what the sentinel indices are.
// Port of: src/core/SkEdge.h#L39-L112 (chrome/m156)
#[doc(alias = "SkEdge")]
#[derive(Copy, Clone, Default, Debug)]
pub struct Edge {
    /// `fNext`: can be used to join edges together.
    pub next: usize,
    /// `fPrev`.
    pub prev: usize,
    /// `fX`.
    pub x: Fixed,
    /// `fDxDy`.
    pub dx_dy: Fixed,
    /// `fFirstY`: an integer because it represents a discrete pixel. Mathematically it is treated
    /// as half way inside the pixel, so 6 -> 6.5.
    pub first_y: i32,
    /// `fLastY`.
    pub last_y: i32,
    /// `fEdgeType`: remembers the *initial* edge type.
    pub edge_type: EdgeType,
    /// `fWinding`.
    pub winding: Winding,
    // `fSegmentCount`: only non-zero for quads and cubics.
    segment_count: u8,
    // `fCurveShift`: how much to shift the derivatives to multiply by deltaT when doing
    // forward-differencing. For cubics this is log_2(N) and for quadratics log_2(N) - 1.
    curve_shift: u8,
}

impl Edge {
    /// Represents a straight line. Returns false if the line has height 0.
    ///
    /// skia-rust: the `setLine(p0, p1)` overload without a clip.
    // Port of: src/core/SkEdge.h#L168-L213 (chrome/m156)
    #[doc(alias = "setLine")]
    pub fn set_line(&mut self, p0: Point, p1: Point) -> bool {
        self.set_line_clipped(p0, p1, None)
    }

    /// Represents a straight line with an optional clip. This will always be a single segment.
    /// Returns false if the line has height 0 or is completely above or below the clip.
    // Port of: src/core/SkEdge.cpp#L120-L171 (chrome/m156)
    #[doc(alias = "setLine")]
    pub fn set_line_clipped(&mut self, p0: Point, p1: Point, clip: Option<&IRect>) -> bool {
        let mut x0 = float_to_fdot6(p0.x);
        let mut y0 = float_to_fdot6(p0.y);
        let mut x1 = float_to_fdot6(p1.x);
        let mut y1 = float_to_fdot6(p1.y);

        let mut winding = Winding::CW;
        if y0 > y1 {
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
            winding = Winding::CCW;
        }

        let top = fdot6_round(y0);
        let bot = fdot6_round(y1);

        // are we a zero-height line?
        if top == bot {
            return false;
        }
        // are we completely above or below the clip?
        if let Some(clip) = clip
            && (top >= clip.bottom || bot <= clip.top)
        {
            return false;
        }

        let slope = fdot6_div(x1.wrapping_sub(x0), y1.wrapping_sub(y0));
        let dy = edge_compute_dy(top, y0);

        // Note that SkFixedMul(SkFixed, SkFDot6) produces results in SkFDot6
        self.x = fdot6_to_fixed(x0.wrapping_add(fixed_mul(slope, dy)));
        self.dx_dy = slope;
        self.first_y = top;
        self.last_y = bot.wrapping_sub(1);
        self.edge_type = EdgeType::Line;
        self.segment_count = 0;
        self.winding = winding;
        self.curve_shift = 0;

        if let Some(clip) = clip {
            self.chop_line_with_clip(clip);
        }
        true
    }

    /// Whether there are more segments to step to.
    // Port of: src/core/SkEdge.h#L76-L78 (chrome/m156)
    #[doc(alias = "hasNextSegment")]
    #[must_use]
    pub fn has_next_segment(&self) -> bool {
        self.segment_count != 0
    }

    /// The number of segments left (`segmentsLeft`).
    // Port of: src/core/SkEdge.h#L85-L87 (chrome/m156)
    #[doc(alias = "segmentsLeft")]
    #[must_use]
    pub fn segments_left(&self) -> u8 {
        self.segment_count
    }

    /// `SkEdge::nextSegment`: a linear edge has no next segment.
    // Port of: src/core/SkEdge.cpp#L173-L176 (chrome/m156)
    #[doc(alias = "nextSegment")]
    pub fn next_segment(&mut self) -> bool {
        debug_assert!(
            false,
            "Shouldn't be asking a linear edge to go to the next curve."
        );
        false
    }

    // Draws a line between the provided points and then calculates the slope and starting
    // x value to line up with the closest pixel center. Updates the fields in the Edge
    // base class appropriately. Returns false if this edge would start and stop in the
    // same row.
    // Port of: src/core/SkEdge.cpp#L182-L214 (chrome/m156)
    fn update_line(&mut self, x_start: Fixed, y_start: Fixed, x_end: Fixed, y_end: Fixed) -> bool {
        debug_assert!(self.segment_count != 0);

        let y0 = fixed_to_fdot6(y_start);
        let y1 = fixed_to_fdot6(y_end);

        debug_assert!(y0 <= y1);

        let top = fdot6_round(y0);
        let bot = fdot6_round(y1);

        // are we a zero-height line?
        if top == bot {
            return false;
        }

        let x0 = fixed_to_fdot6(x_start);
        let x1 = fixed_to_fdot6(x_end);

        let slope = fdot6_div(x1.wrapping_sub(x0), y1.wrapping_sub(y0));
        let dy = edge_compute_dy(top, y0);

        // We could do this math in fixed point, but it would potentially require some
        // rebaselining https://codereview.chromium.org/960353005/#msg6
        // Note that SkFixedMul(SkFixed, SkFDot6) produces results in SkFDot6
        self.x = fdot6_to_fixed(x0.wrapping_add(fixed_mul(slope, dy)));
        self.dx_dy = slope;
        self.first_y = top;
        self.last_y = bot.wrapping_sub(1);

        true
    }

    // Port of: src/core/SkEdge.cpp#L216-L229 (chrome/m156)
    fn chop_line_with_clip(&mut self, clip: &IRect) {
        let top = self.first_y;

        debug_assert!(top < clip.bottom);

        // clip the line to the top
        if top < clip.top {
            debug_assert!(self.last_y >= clip.top);
            self.x = self
                .x
                .wrapping_add(self.dx_dy.wrapping_mul(clip.top.wrapping_sub(top)));
            self.first_y = clip.top;
        }
    }
}

// Port of: src/core/SkEdge.cpp#L53-L57 (chrome/m156)
fn fdot6_to_fixed_div2(value: Fdot6) -> Fixed {
    // we want to return SkFDot6ToFixed(value >> 1), but we don't want to throw
    // away data in value, so just perform a modify up-shift
    left_shift(value, 16 - 6 - 1)
}

/// This limits the number of lines we use to approximate a curve. If we need to increase this, we
/// need to store `fSegmentCount` in a larger data type.
// Port of: src/core/SkEdge.cpp#L237 (chrome/m156)
const MAX_COEFF_SHIFT: i32 = 6;

// Approximate the distance from (0,0) to (dx, dy).
// When dx and dy are about the same
//   sqrt(dx^2 + dy^2) => sqrt(2dx^2) => dx sqrt(2) = 1.41 * dx
// When dx >> dy
//   sqrt(dx^2 + dy^2) => sqrt(dx^2) => dx
// So this is a reasonable approximation
// Port of: src/core/SkEdge.cpp#L245-L253 (chrome/m156)
fn cheap_distance(dx: Fdot6, dy: Fdot6) -> Fdot6 {
    let dx = abs32(dx);
    let dy = abs32(dy);
    // return max + min/2
    if dx > dy {
        return dx.wrapping_add(dy / 2);
    }
    dy.wrapping_add(dx / 2)
}

// Port of: src/core/SkEdge.cpp#L255-L270 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // dist is never negative (it is a sum of absolute values)
#[allow(clippy::cast_possible_wrap)] // 32 - clz is at most 32
fn diff_to_steps(dx: Fdot6, dy: Fdot6, accuracy: i32) -> i32 {
    // cheap calc of distance from center of p0-p2 to the center of the curve
    let mut dist = cheap_distance(dx, dy);

    // shift down dist (it is currently in dot6)
    // down by 3 should give us 1/8 pixel accuracy (assuming our dist is accurate...)
    // this is chosen by heuristic: make it as big as possible (to minimize segments)
    // ... but small enough so that our curves still look smooth
    // When shift > 0, we're using AA and everything is scaled up so we can
    // lower the accuracy.
    // For cubics still, we have shift > 0.
    dist = dist.wrapping_add(1 << (2 + accuracy)) >> (3 + accuracy);

    // each subdivision (shift value) cuts this dist (error) by 1/4
    (32 - (dist as u32).leading_zeros() as i32) >> 1
}

/// A quadratic edge: approximates a monotonic quad with `2^shift` line segments.
// Port of: src/core/SkEdge.h#L114-L140 (chrome/m156)
#[doc(alias = "SkQuadraticEdge")]
#[derive(Copy, Clone, Default, Debug)]
pub struct QuadraticEdge {
    base: Edge,
    // These are the non-rounded points that the current line segment ends at.
    qx: Fixed,
    qy: Fixed,
    // These represent the first derivatives of the quadratic curve evaluated at the midpoint of
    // the next line segment. To avoid overflows, we store them as half their normal value. During
    // the forward-difference step, instead of multiplying by a deltaT of 1/N, we'll multiply by
    // 2/N instead.
    q_dx_dt: Fixed,
    q_dy_dt: Fixed,
    // These are the second derivatives of the quadratic curve pre-multiplied by 1/N.
    q_d2x_dt2: Fixed,
    q_d2y_dt2: Fixed,
    // The non-rounded end points for the entire curve. On the last segment, these will be used
    // instead of the results from our forward-difference technique to make sure cumulative error
    // doesn't result in a dramatically different line.
    q_last_x: Fixed,
    q_last_y: Fixed,
}

impl Deref for QuadraticEdge {
    type Target = Edge;
    fn deref(&self) -> &Edge {
        &self.base
    }
}

impl DerefMut for QuadraticEdge {
    fn deref_mut(&mut self) -> &mut Edge {
        &mut self.base
    }
}

impl QuadraticEdge {
    /// Sets up the line segments of the (monotonic in Y) quad `pts[0..3]`. Returns false if the
    /// line would be of height 0.
    // Port of: src/core/SkEdge.cpp#L272-L387 (chrome/m156)
    #[doc(alias = "setQuadratic")]
    #[allow(clippy::many_single_char_names)] // mirrors the C++ names
    #[allow(clippy::cast_possible_truncation)] // mirrors SkToU8 (values are small)
    #[allow(clippy::cast_sign_loss)] // mirrors SkToU8 (values are non-negative)
    pub fn set_quadratic(&mut self, pts: &[Point]) -> bool {
        let mut x0 = float_to_fdot6(pts[0].x);
        let mut y0 = float_to_fdot6(pts[0].y);
        let x1 = float_to_fdot6(pts[1].x);
        let y1 = float_to_fdot6(pts[1].y);
        let mut x2 = float_to_fdot6(pts[2].x);
        let mut y2 = float_to_fdot6(pts[2].y);

        let mut winding = Winding::CW;
        if y0 > y2 {
            std::mem::swap(&mut x0, &mut x2);
            std::mem::swap(&mut y0, &mut y2);
            winding = Winding::CCW;
        }
        debug_assert!(y0 <= y1 && y1 <= y2, "curve must be monotonic");

        let top = fdot6_round(y0);
        let bot = fdot6_round(y2);

        // are we a zero-height quad (line)?
        if top == bot {
            return false;
        }

        // compute number of steps needed (2^shift) based on the distance between
        // this curve at the half-way point (t=0.5) and the midpoint of a straight
        // line between p0 and p2.
        // B(1/2) = p0 (1-t)^2 + 2 p1 t(1-t) + p2 t^2; t = 1/2
        //        = p0 (1/2)^2 + 2 p1 (1/2)(1/2) + p2 (1/2)^2
        //        = 1/4 (p0 + 2 p1 + p2)
        // Midpoint of p0 and p2 is M(p0, p2) = (p2 + p0) / 2
        // Subtracting the two terms to get the vector representing the difference
        // distance = B(1/2) - M(p0, p2)
        //          = 1/4 (p0 + 2 p1 + p2) - (p2 + p0) / 2
        //          = 1/4 (p0 + 2 p1 + p2) - (2 p2 + 2 p0) / 4
        //          = 1/4 (-p0 + 2 p1 - p2)
        let delta_x = (2_i32.wrapping_mul(x1).wrapping_sub(x0).wrapping_sub(x2)) >> 2;
        let delta_y = (2_i32.wrapping_mul(y1).wrapping_sub(y0).wrapping_sub(y2)) >> 2;
        // We pass those points into this function which will find the total distance
        // and use a heuristic to reduce the error to some threshold.
        let mut shift = diff_to_steps(delta_x, delta_y, 0);
        debug_assert!(shift >= 0);

        // We need at least 2 line segments for us to be able to save the derivatives as
        // half their values to avoid overflow.
        if shift == 0 {
            shift = 1;
        } else if shift > MAX_COEFF_SHIFT {
            shift = MAX_COEFF_SHIFT;
        }

        self.base.winding = winding;
        self.base.edge_type = EdgeType::Quad;
        self.base.segment_count = (1_i32 << shift) as u8;

        //  By re-arranging the Bezier curve in polynomial form, it is easier to
        //  find the derivatives and forward-differentiate from one segment to the next.
        //
        //  p0 (1-t)^2 + 2 p1 t(1-t) + p2 t^2 ==> At^2 + Bt + C
        //
        //  A = p0 - 2p1 + p2
        //  B = 2(p1 - p0)
        //  C = p0
        //
        //  Our caller must have constrained our inputs (p0..p2) to all fit into
        //  16.16. However, as seen above, we sometimes compute values that can be
        //  larger (e.g. B = 2*(p1 - p0)). To guard against overflow, we will store
        //  A and B at 1/2 of their actual value, and just apply a 2x scale during
        //  application in nextSegment(). Hence we store (shift - 1) in
        //  fCurveShift.

        self.base.curve_shift = (shift - 1) as u8;

        // The extra 1/2 factor avoids overflow
        let mut a_half = fdot6_to_fixed_div2(x0.wrapping_sub(x1).wrapping_sub(x1).wrapping_add(x2));
        let mut b_half = fdot6_to_fixed(x1.wrapping_sub(x0));

        // We want to calculate the slope at the midpoint of our first segment. This means
        // evaluating
        //   dx/dt = 2A*t + B
        //   dx^2/dt^2 = 2A
        // at t = 1/N * 1/2
        // There's an extra 1/2 on the whole expression to avoid overflows (as above).
        //  1/2 ( 2A*t + B) => 1/2 (2A*1/2N + B) => A/2*1/N + B/2 => A/2 * 1/2^shift + B/2
        self.q_dx_dt = b_half.wrapping_add(a_half >> shift);
        // The second derivatives are constant, so we can pre-multiply them by 1/N to save having
        // to do it in nextSegment(). Since A_half was already calculated we can use a smaller
        // shift.
        // 1/2 (2A * 1/N) => A * 1/N => A * 1/2^shift => A/2 * 1/2^(shift-1)
        self.q_d2x_dt2 = a_half >> (shift - 1);

        a_half = fdot6_to_fixed_div2(y0.wrapping_sub(y1).wrapping_sub(y1).wrapping_add(y2));
        b_half = fdot6_to_fixed(y1.wrapping_sub(y0));

        self.q_dy_dt = b_half.wrapping_add(a_half >> shift);
        self.q_d2y_dt2 = a_half >> (shift - 1);

        self.qx = fdot6_to_fixed(x0);
        self.qy = fdot6_to_fixed(y0);
        self.q_last_x = fdot6_to_fixed(x2);
        self.q_last_y = fdot6_to_fixed(y2);

        self.next_segment()
    }

    /// Steps to the next line segment, skipping any of zero height. Returns false if only
    /// zero-height segments remained.
    // Port of: src/core/SkEdge.cpp#L389-L424 (chrome/m156)
    #[doc(alias = "nextSegment")]
    #[allow(clippy::cast_possible_truncation)] // mirrors SkToU8 (count fits)
    #[allow(clippy::cast_sign_loss)] // mirrors SkToU8 (count is non-negative)
    #[allow(clippy::cast_possible_wrap)] // count is at most 64
    #[allow(clippy::similar_names)] // mirrors the C++ oldx/oldy, newx/newy
    pub fn next_segment(&mut self) -> bool {
        let mut success;
        let mut count = i32::from(self.base.segment_count);
        let mut oldx = self.qx;
        let mut oldy = self.qy;
        let mut dx = self.q_dx_dt;
        let mut dy = self.q_dy_dt;
        let mut newx;
        let mut newy;
        let shift = i32::from(self.base.curve_shift);

        debug_assert!(count > 0);

        loop {
            count -= 1;
            if count > 0 {
                newx = oldx.wrapping_add(dx >> shift);
                dx = dx.wrapping_add(self.q_d2x_dt2);
                newy = oldy.wrapping_add(dy >> shift);
                dy = dy.wrapping_add(self.q_d2y_dt2);
            } else {
                // last segment
                newx = self.q_last_x;
                newy = self.q_last_y;
            }
            success = self.base.update_line(oldx, oldy, newx, newy);
            oldx = newx;
            oldy = newy;
            if count <= 0 || success {
                break;
            }
        }

        self.qx = newx;
        self.qy = newy;
        self.q_dx_dt = dx;
        self.q_dy_dt = dy;
        self.base.segment_count = count as u8;
        success
    }
}

// Port of: src/core/SkEdge.cpp#L428-L431 (chrome/m156)
fn fdot6_up_shift(x: Fdot6, up_shift: i32) -> Fdot6 {
    debug_assert_eq!(left_shift(x, up_shift) >> up_shift, x);
    left_shift(x, up_shift)
}

// f(1/3) = (8a + 12b + 6c + d) / 27
// f(2/3) = (a + 6b + 12c + 8d) / 27
//
// f(1/3)-b = (8a - 15b + 6c + d) / 27
// f(2/3)-c = (a + 6b - 15c + 8d) / 27
//
// use 16/512 to approximate 1/27
// Port of: src/core/SkEdge.cpp#L441-L448 (chrome/m156)
fn cubic_delta_from_line(a: Fdot6, b: Fdot6, c: Fdot6, d: Fdot6) -> Fdot6 {
    // since our parameters may be negative, we don't use << to avoid ASAN warnings
    let one_third = (a
        .wrapping_mul(8)
        .wrapping_sub(b.wrapping_mul(15))
        .wrapping_add(6_i32.wrapping_mul(c))
        .wrapping_add(d))
    .wrapping_mul(19)
        >> 9;
    let two_third = (a
        .wrapping_add(6_i32.wrapping_mul(b))
        .wrapping_sub(c.wrapping_mul(15))
        .wrapping_add(d.wrapping_mul(8)))
    .wrapping_mul(19)
        >> 9;

    abs32(one_third).max(abs32(two_third))
}

/// A cubic edge: approximates a monotonic cubic with `2^stepExponent` line segments.
// Port of: src/core/SkEdge.h#L142-L166 (chrome/m156)
#[doc(alias = "SkCubicEdge")]
#[derive(Copy, Clone, Default, Debug)]
pub struct CubicEdge {
    base: Edge,
    // These are the non-rounded points that the current line segment ends at.
    cx: Fixed,
    cy: Fixed,
    c_dx_dt: Fixed,
    c_dy_dt: Fixed,
    c_d2x_dt2: Fixed,
    c_d2y_dt2: Fixed,
    c_d3x_dt3: Fixed,
    c_d3y_dt3: Fixed,
    // The non-rounded end points for the entire curve. On the last segment, these will be used
    // instead of the results from our forward-difference technique to make sure cumulative error
    // doesn't result in a dramatically different line.
    c_last_x: Fixed,
    c_last_y: Fixed,
    // Applied to fCDxDt and fCDyDt in nextSegment() to align to SkFixed.
    to_fixed_shift: u8,
}

impl Deref for CubicEdge {
    type Target = Edge;
    fn deref(&self) -> &Edge {
        &self.base
    }
}

impl DerefMut for CubicEdge {
    fn deref_mut(&mut self) -> &mut Edge {
        &mut self.base
    }
}

impl CubicEdge {
    /// Sets up the line segments of the (monotonic in Y) cubic `pts[0..4]`. Returns false if the
    /// line would be of height 0.
    // Port of: src/core/SkEdge.cpp#L450-L601 (chrome/m156)
    #[doc(alias = "setCubic")]
    #[allow(clippy::many_single_char_names)] // mirrors the C++ names
    #[allow(clippy::cast_possible_truncation)] // mirrors SkToU8 (values are small)
    #[allow(clippy::cast_sign_loss)] // mirrors SkToU8 (values are non-negative)
    pub fn set_cubic(&mut self, pts: &[Point]) -> bool {
        let mut x0 = float_to_fdot6(pts[0].x);
        let mut y0 = float_to_fdot6(pts[0].y);
        let mut x1 = float_to_fdot6(pts[1].x);
        let mut y1 = float_to_fdot6(pts[1].y);
        let mut x2 = float_to_fdot6(pts[2].x);
        let mut y2 = float_to_fdot6(pts[2].y);
        let mut x3 = float_to_fdot6(pts[3].x);
        let mut y3 = float_to_fdot6(pts[3].y);

        let mut winding = Winding::CW;
        if y0 > y3 {
            std::mem::swap(&mut x0, &mut x3);
            std::mem::swap(&mut x1, &mut x2);
            std::mem::swap(&mut y0, &mut y3);
            std::mem::swap(&mut y1, &mut y2);
            winding = Winding::CCW;
        }

        let top = fdot6_round(y0);
        let bot = fdot6_round(y3);

        // are we a zero-height cubic (line)?
        if top == bot {
            return false;
        }

        // compute number of steps needed (1 << stepExponent)
        // Can't use (center of curve - center of baseline), since center-of-curve
        // need not be the max delta from the baseline (it could even be coincident)
        // so we try just looking at the two off-curve points
        let dx = cubic_delta_from_line(x0, x1, x2, x3);
        let dy = cubic_delta_from_line(y0, y1, y2, y3);
        // add 1 (by observation)
        let mut step_exponent = diff_to_steps(dx, dy, 2) + 1;
        // need at least 1 subdivision for our bias trick
        debug_assert!(step_exponent > 0);
        if step_exponent > MAX_COEFF_SHIFT {
            step_exponent = MAX_COEFF_SHIFT;
        }

        // To maintain maximum precision and avoid intermediate divisions, we manage
        // three different "shifts" for the cubic forward-differencing math:
        //
        // 1. "stepExponent" (Subdivision Exponent):
        //    - The exponent for the number of segments: N = 2^stepExponent.
        //    - Parametric step size: h = 1/N = 2^-stepExponent.
        //    - Stored in fCurveShift. Used to update the first derivative step-to-step.
        //
        // 2. "precisionUpScale" (Precision Upscale Shift):
        //    - We scale up the incoming FDot6 coordinates by 2^precisionUpScale (via
        //      SkFDot6UpShift) before constructing our polynomial coefficients (A, B, C).
        //    - This prevents fractional bits from being shifted off and lost when dividing
        //      coefficients by powers of 2^stepExponent (i.e. >> stepExponent or
        //      >> 2*stepExponent) during setup.
        //    - Capped at 6 to prevent signed 32-bit integer overflow during intermediate
        //      computations (which involve multiplications by 3 and 6).
        //
        // 3. "toFixedShift" (Coordinate Realignment Downshift):
        //    - Stored in fToFixedShift. Used in nextSegment() to scale the step delta
        //      back down to standard SkFixed (FDot16) format.
        //    - Since the coefficients are scaled up by 2^precisionUpScale, the step size h is
        //      2^-stepExponent, and standard SkFixed has 10 more fractional bits than
        //      FDot6 (16 - 6 = 10), the alignment factor for the position update
        //      (x + fCDxDt * h) is:
        //          2^10 / (2^precisionUpScale * 2^stepExponent) =
        //             1 / 2^(stepExponent + precisionUpScale - 10)
        //      which is implemented as a right-shift by:
        //          toFixedShift = stepExponent + precisionUpScale - 10.
        //    - If toFixedShift is negative (which would require an unsupported left-shift),
        //      we clamp toFixedShift to 0 and reduce precisionUpScale accordingly to
        //      10 - stepExponent.
        let mut precision_up_scale = 6; // largest safe value
        let mut to_fixed_shift = step_exponent + precision_up_scale - 10;
        if to_fixed_shift < 0 {
            to_fixed_shift = 0;
            precision_up_scale = 10 - step_exponent;
        }

        self.base.winding = winding;
        self.base.edge_type = EdgeType::Cubic;
        self.base.segment_count = left_shift(1, step_exponent) as u8;
        self.base.curve_shift = step_exponent as u8;
        self.to_fixed_shift = to_fixed_shift as u8;

        // By re-arranging the Bezier curve in polynomial form, it is easier to
        // find the derivatives and forward-differentiate from one segment to the next.

        // p0 (1-t)^3 + 3 p1 t(1-t)^2 + 3 p2 t^2 (1-t) + p3 t^3 ==> At^3 + Bt^2 + Ct + D
        // Where A = -p0 + 3p1 + -3p2 + p3
        //       B = 3p0 - 6p1 + 3p2
        //       C = -3p0 + 3p1
        //       D = p0
        let mut a_scaled = fdot6_up_shift(
            x3.wrapping_add(3_i32.wrapping_mul(x1.wrapping_sub(x2)))
                .wrapping_sub(x0),
            precision_up_scale,
        );
        let mut b_scaled = fdot6_up_shift(
            3_i32.wrapping_mul(x0.wrapping_sub(2_i32.wrapping_mul(x1)).wrapping_add(x2)),
            precision_up_scale,
        );
        let mut c_scaled =
            fdot6_up_shift(3_i32.wrapping_mul(x1.wrapping_sub(x0)), precision_up_scale);

        // The cubic curve in polynomial form is: x(t) = A*t^3 + B*t^2 + C*t + D
        // With a step size of h = 1/N = 1/(2^stepExponent), the forward differences at t=0 are:
        //   1) First Difference:  Δx(0) = x(h) - x(0)      = A*h^3 + B*h^2 + C*h
        //   2) Second Difference: Δ²x(0) = Δx(h) - Δx(0)    = 6A*h^3 + 2B*h^2
        //   3) Third Difference:  Δ³x(0) = Δ²x(h) - Δ²x(0)  = 6A*h^3

        // To keep the math as precise as possible, we scale up each difference term.
        // Because the step size h is a power of 2 (1 / 2^stepExponent), scaling them up
        // allows us to perform all loop updates using bit-shifts instead of slow division:

        // - fCDxDt   = Δx(0)  / h  = A*h^2 + B*h + C
        //                          = A/(2^(2*stepExponent)) + B/(2^stepExponent) + C
        // - fCD2xDt2 = Δ²x(0) / h² = 6A*h + 2B
        //                          = 6A*(1/2^stepExponent) + 2B # cancel 2 on top and bottom of A
        //                          = 3A/2^(stepExponent-1) + 2B
        // - fCD3xDt3 = Δ³x(0) / h² = 6A*h
        //                          = 6A*(1/2^stepExponent)
        //                          = 3A/2^(stepExponent-1)
        // These are stored in the edge struct as SkFixedScaled because they must be scaled down
        // by toFixedShift or stepExponent (ddshift) before they can be added to standard SkFixed
        // coordinates or used to update other derivative terms.
        self.c_dx_dt = (a_scaled >> (2 * step_exponent))
            .wrapping_add(b_scaled >> step_exponent)
            .wrapping_add(c_scaled);
        self.c_d2x_dt2 = (3_i32.wrapping_mul(a_scaled) >> (step_exponent - 1))
            .wrapping_add(2_i32.wrapping_mul(b_scaled));
        self.c_d3x_dt3 = 3_i32.wrapping_mul(a_scaled) >> (step_exponent - 1);

        a_scaled = fdot6_up_shift(
            y3.wrapping_add(3_i32.wrapping_mul(y1.wrapping_sub(y2)))
                .wrapping_sub(y0),
            precision_up_scale,
        );
        b_scaled = fdot6_up_shift(
            3_i32.wrapping_mul(y0.wrapping_sub(2_i32.wrapping_mul(y1)).wrapping_add(y2)),
            precision_up_scale,
        );
        c_scaled = fdot6_up_shift(3_i32.wrapping_mul(y1.wrapping_sub(y0)), precision_up_scale);

        self.c_dy_dt = (a_scaled >> (2 * step_exponent))
            .wrapping_add(b_scaled >> step_exponent)
            .wrapping_add(c_scaled);
        self.c_d2y_dt2 = (3_i32.wrapping_mul(a_scaled) >> (step_exponent - 1))
            .wrapping_add(2_i32.wrapping_mul(b_scaled));
        self.c_d3y_dt3 = 3_i32.wrapping_mul(a_scaled) >> (step_exponent - 1);

        self.cx = fdot6_to_fixed(x0);
        self.cy = fdot6_to_fixed(y0);
        self.c_last_x = fdot6_to_fixed(x3);
        self.c_last_y = fdot6_to_fixed(y3);

        self.next_segment()
    }

    /// Steps to the next line segment, skipping any of zero height. Returns false if only
    /// zero-height segments remained.
    // Port of: src/core/SkEdge.cpp#L603-L663 (chrome/m156)
    #[doc(alias = "nextSegment")]
    #[allow(clippy::cast_possible_truncation)] // mirrors SkToU8 (count fits)
    #[allow(clippy::cast_sign_loss)] // mirrors SkToU8 (count is non-negative)
    #[allow(clippy::similar_names)] // mirrors the C++ oldx/oldy, newx/newy
    pub fn next_segment(&mut self) -> bool {
        let mut success;
        let mut count = i32::from(self.base.segment_count);
        let mut oldx = self.cx;
        let mut oldy = self.cy;
        let mut newx;
        let mut newy;
        let step_exponent = i32::from(self.base.curve_shift); // Subdivision exponent
        let to_fixed_shift = i32::from(self.to_fixed_shift);

        debug_assert!(count > 0);

        loop {
            count -= 1;
            if count > 0 {
                // 1. Position Update: x_next = x + (fCDxDt * h)
                //    Since fCDxDt has units of FDot6 scaled up by precisionUpScale, and is stored
                //    as (Δx / h) (which scales it up by 1/h = 2^stepExponent), we must:
                //      a) Divide by 2^stepExponent to multiply by step-size h
                //      b) Divide by 2^precisionUpScale to remove the precision upscaling
                //      c) Multiply by 2^10 to convert from FDot6 to SkFixed (since 16 - 6 = 10)
                //    Combining these: Δx = fCDxDt * (2^10) / (2^stepExponent * 2^precisionUpScale)
                //                        = fCDxDt >> (stepExponent + precisionUpScale - 10)
                //    This is implemented as shifting right by toFixedShift.
                newx = oldx.wrapping_add(self.c_dx_dt >> to_fixed_shift);

                // 2. First Difference Update: fCDxDt_next = fCDxDt + h * fCD2xDt2
                //    Since fCD2xDt2 has units of Δ²x / h², multiplying by h yields Δ²x / h.
                //    This is accomplished by right-shifting fCD2xDt2 by stepExponent
                //    (h = 1/2^stepExponent).
                self.c_dx_dt = self.c_dx_dt.wrapping_add(self.c_d2x_dt2 >> step_exponent);

                // 3. Second Difference Update: fCD2xDt2_next = fCD2xDt2 + fCD3xDt3
                //    Since both fCD2xDt2 and fCD3xDt3 are scaled by 1/h², we add them directly.
                self.c_d2x_dt2 = self.c_d2x_dt2.wrapping_add(self.c_d3x_dt3);

                newy = oldy.wrapping_add(self.c_dy_dt >> to_fixed_shift);
                self.c_dy_dt = self.c_dy_dt.wrapping_add(self.c_d2y_dt2 >> step_exponent);
                self.c_d2y_dt2 = self.c_d2y_dt2.wrapping_add(self.c_d3y_dt3);
            } else {
                // last segment
                newx = self.c_last_x;
                newy = self.c_last_y;
            }

            // we want to say SkASSERT(oldy <= newy), but our finite fixedpoint
            // doesn't always achieve that, so we have to explicitly pin it here.
            if newy < oldy {
                newy = oldy;
            }

            success = self.base.update_line(oldx, oldy, newx, newy);
            oldx = newx;
            oldy = newy;
            if count <= 0 || success {
                break;
            }
        }

        self.cx = newx;
        self.cy = newy;
        self.base.segment_count = count as u8;
        success
    }
}

/// Any of the edge kinds, as stored by an edge builder (the C++ stores `SkEdge*` and dispatches
/// `nextSegment` virtually). Dereferences to the shared [`Edge`] state.
#[doc(alias = "SkEdge")]
#[derive(Copy, Clone, Debug)]
pub enum AnyEdge {
    Line(Edge),
    Quad(QuadraticEdge),
    Cubic(CubicEdge),
}

impl AnyEdge {
    /// `nextSegment()`, dispatched on the kind of edge.
    #[doc(alias = "nextSegment")]
    pub fn next_segment(&mut self) -> bool {
        match self {
            Self::Line(e) => e.next_segment(),
            Self::Quad(e) => e.next_segment(),
            Self::Cubic(e) => e.next_segment(),
        }
    }
}

impl Deref for AnyEdge {
    type Target = Edge;
    fn deref(&self) -> &Edge {
        match self {
            Self::Line(e) => e,
            Self::Quad(e) => &e.base,
            Self::Cubic(e) => &e.base,
        }
    }
}

impl DerefMut for AnyEdge {
    fn deref_mut(&mut self) -> &mut Edge {
        match self {
            Self::Line(e) => e,
            Self::Quad(e) => &mut e.base,
            Self::Cubic(e) => &mut e.base,
        }
    }
}
