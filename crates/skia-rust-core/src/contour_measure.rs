// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkContourMeasure.h, src/core/SkContourMeasure.cpp,
// src/core/SkPathMeasurePriv.h

//! Measuring the length of path contours (`SkContourMeasure.h`).

use std::sync::Arc;

use bitflags::bitflags;

use crate::floating_point::{is_finite, is_nan};
use crate::geometry::Conic;
use crate::geometry::{
    chop_cubic_at, chop_cubic_at_half, chop_quad_at, chop_quad_at_half, eval_cubic_at,
    eval_quad_at_pos_tangent,
};
use crate::matrix::Matrix;
use crate::path::Path;
use crate::path_builder::PathBuilder;
use crate::path_priv;
use crate::path_types::PathVerb;
use crate::point::{Point, Vector};
use crate::scalar::{SCALAR_1, scalar, scalar_abs, scalar_interp};

const MAX_T_VALUE: i32 = 0x3FFF_FFFF;

// Port of: src/core/SkContourMeasure.cpp#L29-L34 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // int to float, as in C++
#[allow(clippy::manual_range_contains)] // mirrors the C++ comparisons
fn t_value_2_scalar(t: i32) -> scalar {
    debug_assert!(t >= 0 && t <= MAX_T_VALUE);
    // 1/kMaxTValue can't be represented as a float, but it's close and the limits work fine.
    let max_t_reciprocal: scalar = 1.0 / (MAX_T_VALUE as scalar);
    t as scalar * max_t_reciprocal
}

/// The segment types (`SkSegType`), stored in 2 bits.
// Port of: src/core/SkPathMeasurePriv.h#L18-L23 (chrome/m156)
const LINE_SEG_TYPE: u8 = 0;
const QUAD_SEG_TYPE: u8 = 1;
const CUBIC_SEG_TYPE: u8 = 2;
const CONIC_SEG_TYPE: u8 = 3;

// Port of: include/core/SkContourMeasure.h#L172-L190 (chrome/m156)
#[derive(Copy, Clone, Debug)]
struct Segment {
    distance: scalar, // total distance up to this point
    pt_index: u32,    // index into the fPts array
    t_value: u32,     // 30 bits
    ty: u8,           // actually the enum SkSegType
}

impl Segment {
    // Port of: src/core/SkContourMeasure.cpp#L39-L41 (chrome/m156)
    #[allow(clippy::cast_possible_wrap)] // 30-bit value
    fn get_scalar_t(&self) -> scalar {
        t_value_2_scalar(self.t_value as i32)
    }
}

bitflags! {
    /// What [`ContourMeasure::get_matrix`] computes.
    // Port of: include/core/SkContourMeasure.h#L49-L53 (chrome/m156)
    #[doc(alias = "SkContourMeasure::MatrixFlags")]
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
    pub struct MatrixFlags: u32 {
        const GET_POSITION = 0x01;
        const GET_TANGENT = 0x02;
        const GET_POS_AND_TAN = Self::GET_POSITION.bits() | Self::GET_TANGENT.bits();
    }
}

impl Default for MatrixFlags {
    fn default() -> Self {
        Self::GET_POS_AND_TAN
    }
}

// Port of: src/core/SkContourMeasure.cpp#L43-L135 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparisons, as in C++
fn contour_measure_seg_to(
    pts: &[Point],
    seg_type: u8,
    start_t: scalar,
    stop_t: scalar,
    dst: &mut PathBuilder,
) {
    debug_assert!((0.0..=SCALAR_1).contains(&start_t));
    debug_assert!((0.0..=SCALAR_1).contains(&stop_t));
    debug_assert!(start_t <= stop_t);

    if start_t == stop_t {
        if !dst.is_empty() {
            /* if the dash as a zero-length on segment, add a corresponding zero-length line.
            The stroke code will add end caps to zero length lines as appropriate */
            let last_pt = dst
                .get_last_pt()
                .expect("non-empty builder has a last point");
            dst.line_to(last_pt);
        }
        return;
    }

    let mut tmp0 = [Point::default(); 7];
    let mut tmp1 = [Point::default(); 7];

    match seg_type {
        LINE_SEG_TYPE => {
            if SCALAR_1 == stop_t {
                dst.line_to(pts[1]);
            } else {
                dst.line_to((
                    scalar_interp(pts[0].x, pts[1].x, stop_t),
                    scalar_interp(pts[0].y, pts[1].y, stop_t),
                ));
            }
        }
        QUAD_SEG_TYPE => {
            if 0.0 == start_t {
                if SCALAR_1 == stop_t {
                    dst.quad_to(pts[1], pts[2]);
                } else {
                    chop_quad_at(pts, &mut tmp0, stop_t);
                    dst.quad_to(tmp0[1], tmp0[2]);
                }
            } else {
                chop_quad_at(pts, &mut tmp0, start_t);
                if SCALAR_1 == stop_t {
                    dst.quad_to(tmp0[3], tmp0[4]);
                } else {
                    let src = [tmp0[2], tmp0[3], tmp0[4]];
                    chop_quad_at(&src, &mut tmp1, (stop_t - start_t) / (1.0 - start_t));
                    dst.quad_to(tmp1[1], tmp1[2]);
                }
            }
        }
        CONIC_SEG_TYPE => {
            let conic = Conic::new(pts[0], pts[2], pts[3], pts[1].x);

            if 0.0 == start_t {
                if SCALAR_1 == stop_t {
                    dst.conic_to(conic.pts[1], conic.pts[2], conic.w);
                } else {
                    let mut tmp = [Conic::default(); 2];
                    if conic.chop_at(stop_t, &mut tmp) {
                        dst.conic_to(tmp[0].pts[1], tmp[0].pts[2], tmp[0].w);
                    }
                }
            } else if SCALAR_1 == stop_t {
                let mut tmp = [Conic::default(); 2];
                if conic.chop_at(start_t, &mut tmp) {
                    dst.conic_to(tmp[1].pts[1], tmp[1].pts[2], tmp[1].w);
                }
            } else {
                let mut tmp = Conic::default();
                conic.chop_at_interval(start_t, stop_t, &mut tmp);
                dst.conic_to(tmp.pts[1], tmp.pts[2], tmp.w);
            }
        }
        CUBIC_SEG_TYPE => {
            if 0.0 == start_t {
                if SCALAR_1 == stop_t {
                    dst.cubic_to(pts[1], pts[2], pts[3]);
                } else {
                    chop_cubic_at(pts, &mut tmp0, stop_t);
                    dst.cubic_to(tmp0[1], tmp0[2], tmp0[3]);
                }
            } else {
                chop_cubic_at(pts, &mut tmp0, start_t);
                if SCALAR_1 == stop_t {
                    dst.cubic_to(tmp0[4], tmp0[5], tmp0[6]);
                } else {
                    let src = [tmp0[3], tmp0[4], tmp0[5], tmp0[6]];
                    chop_cubic_at(&src, &mut tmp1, (stop_t - start_t) / (1.0 - start_t));
                    dst.cubic_to(tmp1[1], tmp1[2], tmp1[3]);
                }
            }
        }
        _ => panic!("unknown segType"),
    }
}

// Port of: src/core/SkContourMeasure.cpp#L139-L142 (chrome/m156)
#[allow(clippy::manual_range_contains)] // mirrors the C++ comparisons
fn tspan_big_enough(tspan: i32) -> i32 {
    debug_assert!(tspan >= 0 && tspan <= MAX_T_VALUE);
    tspan >> 10
}

// can't use tangents, since we need [0..1..................2] to be seen
// as definitely not a line (it is when drawn, but not parametrically)
// so we compare midpoints
const CHEAP_DIST_LIMIT: scalar = SCALAR_1 / 2.0; // just made this value up

// Port of: src/core/SkContourMeasure.cpp#L149-L159 (chrome/m156)
#[allow(clippy::manual_midpoint)] // mirrors the C++ `(a + b) / 2` arithmetic
fn quad_too_curvy(pts: &[Point], tolerance: scalar) -> bool {
    // diff = (a/4 + b/2 + c/4) - (a/2 + c/2)
    // diff = -a/4 + b/2 - c/4
    let dx = (pts[1].x / 2.0) - (((pts[0].x + pts[2].x) / 2.0) / 2.0);
    let dy = (pts[1].y / 2.0) - (((pts[0].y + pts[2].y) / 2.0) / 2.0);

    let dist = std_max(dx.abs(), dy.abs());
    dist > tolerance
}

// Port of: src/core/SkContourMeasure.cpp#L161-L168 (chrome/m156)
fn conic_too_curvy(first_pt: Point, mid_t_pt: Point, last_pt: Point, tolerance: scalar) -> bool {
    let mut mid_ends = first_pt + last_pt;
    mid_ends *= 0.5;
    let dxy = mid_t_pt - mid_ends;
    let dist = std_max(scalar_abs(dxy.x), scalar_abs(dxy.y));
    dist > tolerance
}

// Port of: src/core/SkContourMeasure.cpp#L170-L175 (chrome/m156)
fn cheap_dist_exceeds_limit(pt: Point, x: scalar, y: scalar, tolerance: scalar) -> bool {
    let dist = std_max(scalar_abs(x - pt.x), scalar_abs(y - pt.y));
    // just made up the 1/2
    dist > tolerance
}

// Port of: src/core/SkContourMeasure.cpp#L177-L185 (chrome/m156)
fn cubic_too_curvy(pts: &[Point], tolerance: scalar) -> bool {
    cheap_dist_exceeds_limit(
        pts[1],
        scalar_interp(pts[0].x, pts[3].x, SCALAR_1 / 3.0),
        scalar_interp(pts[0].y, pts[3].y, SCALAR_1 / 3.0),
        tolerance,
    ) || cheap_dist_exceeds_limit(
        pts[2],
        scalar_interp(pts[0].x, pts[3].x, SCALAR_1 * 2.0 / 3.0),
        scalar_interp(pts[0].y, pts[3].y, SCALAR_1 * 2.0 / 3.0),
        tolerance,
    )
}

/// `std::max(a, b)`: `(a < b) ? b : a`.
fn std_max(a: scalar, b: scalar) -> scalar {
    if a < b { b } else { a }
}

// puts a cap on the total size of our output, since the client can pass in
// arbitrarily large values for resScale.
const MAX_RECURSION_DEPTH: i32 = 8;

// Port of: src/core/SkContourMeasure.cpp#L191-L222 (chrome/m156)
#[derive(Debug)]
struct Impl {
    path: Path,
    // SkPathPriv::RangeIter, as a resumable cursor into `path`.
    iter: (usize, usize, usize),
    tolerance: scalar,
    force_closed: bool,

    // temporary
    segments: Vec<Segment>,
    pts: Vec<Point>, // Points used to define the segments
}

#[allow(clippy::cast_sign_loss)] // ptIndex / t values are non-negative where stored
impl Impl {
    // Port of: src/core/SkContourMeasure.cpp#L193-L197 (chrome/m156)
    fn new(path: &Path, force_closed: bool, res_scale: scalar) -> Self {
        Self {
            path: path.clone(),
            iter: (0, 0, 0),
            tolerance: CHEAP_DIST_LIMIT * crate::floating_point::ieee_float_divide(1.0, res_scale),
            force_closed,
            segments: Vec::new(),
            pts: Vec::new(),
        }
    }

    // Port of: src/core/SkContourMeasure.cpp#L199 (chrome/m156)
    fn has_next_segments(&self) -> bool {
        let mut it = path_priv::iterate(&self.path);
        it.set_position(self.iter);
        !it.is_done()
    }

    fn push_segment(&mut self, distance: scalar, pt_index: i32, ty: u8, t_value: i32) {
        debug_assert!((pt_index as usize) < self.pts.len());
        self.segments.push(Segment {
            distance,
            pt_index: pt_index as u32,
            t_value: t_value as u32,
            ty,
        });
    }

    // Port of: src/core/SkContourMeasure.cpp#L224-L250 (chrome/m156)
    fn compute_quad_segs(
        &mut self,
        pts: &[Point],
        distance: scalar,
        mint: i32,
        maxt: i32,
        pt_index: i32,
        recursion_depth: i32,
    ) -> scalar {
        let mut distance = distance;
        if recursion_depth < MAX_RECURSION_DEPTH
            && tspan_big_enough(maxt - mint) != 0
            && quad_too_curvy(pts, self.tolerance)
        {
            let mut tmp = [Point::default(); 5];
            let halft = (mint + maxt) >> 1;

            chop_quad_at_half(pts, &mut tmp);
            let recursion_depth = recursion_depth + 1;
            distance = self.compute_quad_segs(
                &tmp[0..3],
                distance,
                mint,
                halft,
                pt_index,
                recursion_depth,
            );
            distance = self.compute_quad_segs(
                &tmp[2..5],
                distance,
                halft,
                maxt,
                pt_index,
                recursion_depth,
            );
        } else {
            let d = Point::distance(pts[0], pts[2]);
            let prev_d = distance;
            distance += d;
            if distance > prev_d {
                self.push_segment(distance, pt_index, QUAD_SEG_TYPE, maxt);
            }
        }
        distance
    }

    // Port of: src/core/SkContourMeasure.cpp#L252-L283 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    fn compute_conic_segs(
        &mut self,
        conic: &Conic,
        distance: scalar,
        mint: i32,
        min_pt: Point,
        maxt: i32,
        max_pt: Point,
        pt_index: i32,
        recursion_depth: i32,
    ) -> scalar {
        let mut distance = distance;
        let halft = (mint + maxt) >> 1;
        let half_pt = conic.eval_at(t_value_2_scalar(halft));
        if !half_pt.is_finite() {
            return distance;
        }
        if recursion_depth < MAX_RECURSION_DEPTH
            && tspan_big_enough(maxt - mint) != 0
            && conic_too_curvy(min_pt, half_pt, max_pt, self.tolerance)
        {
            let recursion_depth = recursion_depth + 1;
            distance = self.compute_conic_segs(
                conic,
                distance,
                mint,
                min_pt,
                halft,
                half_pt,
                pt_index,
                recursion_depth,
            );
            distance = self.compute_conic_segs(
                conic,
                distance,
                halft,
                half_pt,
                maxt,
                max_pt,
                pt_index,
                recursion_depth,
            );
        } else {
            let d = Point::distance(min_pt, max_pt);
            let prev_d = distance;
            distance += d;
            if distance > prev_d {
                self.push_segment(distance, pt_index, CONIC_SEG_TYPE, maxt);
            }
        }
        distance
    }

    // Port of: src/core/SkContourMeasure.cpp#L285-L314 (chrome/m156)
    fn compute_cubic_segs(
        &mut self,
        pts: &[Point],
        distance: scalar,
        mint: i32,
        maxt: i32,
        pt_index: i32,
        recursion_depth: i32,
    ) -> scalar {
        let mut distance = distance;
        if recursion_depth < MAX_RECURSION_DEPTH
            && tspan_big_enough(maxt - mint) != 0
            && cubic_too_curvy(pts, self.tolerance)
        {
            let mut tmp = [Point::default(); 7];
            let halft = (mint + maxt) >> 1;

            chop_cubic_at_half(pts, &mut tmp);
            let recursion_depth = recursion_depth + 1;
            distance = self.compute_cubic_segs(
                &tmp[0..4],
                distance,
                mint,
                halft,
                pt_index,
                recursion_depth,
            );
            distance = self.compute_cubic_segs(
                &tmp[3..7],
                distance,
                halft,
                maxt,
                pt_index,
                recursion_depth,
            );
        } else {
            let d = Point::distance(pts[0], pts[3]);
            let prev_d = distance;
            distance += d;
            if distance > prev_d {
                self.push_segment(distance, pt_index, CUBIC_SEG_TYPE, maxt);
            }
        }
        distance
    }

    // Port of: src/core/SkContourMeasure.cpp#L316-L331 (chrome/m156)
    fn compute_line_seg(
        &mut self,
        p0: Point,
        p1: Point,
        distance: scalar,
        pt_index: i32,
    ) -> scalar {
        let mut distance = distance;
        let d = Point::distance(p0, p1);
        debug_assert!(d >= 0.0);
        let prev_d = distance;
        distance += d;
        if distance > prev_d {
            self.push_segment(distance, pt_index, LINE_SEG_TYPE, MAX_T_VALUE);
        }
        distance
    }

    // Port of: src/core/SkContourMeasure.cpp#L360-L463 (chrome/m156)
    fn build_segments(&mut self) -> Option<ContourMeasure> {
        let mut pt_index: i32 = -1;
        let mut distance: scalar = 0.0;
        let mut have_seen_close = self.force_closed;
        let mut have_seen_move_to = false;

        /*  Note:
         *  as we accumulate distance, we have to check that the result of +=
         *  actually made it larger, since a very small delta might be > 0, but
         *  still have no effect on distance (if distance >>> delta).
         *
         *  We do this check below, and in compute_quad_segs and compute_cubic_segs
         */

        self.segments.clear();
        self.pts.clear();

        let path = self.path.clone();
        let mut it = path_priv::iterate(&path);
        it.set_position(self.iter);
        while let Some(verb) = it.peek_verb() {
            if have_seen_move_to && verb == PathVerb::Move {
                break;
            }
            let (verb, pts, w) = it.next().expect("peeked");
            match verb {
                PathVerb::Move => {
                    pt_index += 1;
                    self.pts.push(pts[0]);
                    debug_assert!(!have_seen_move_to);
                    have_seen_move_to = true;
                }
                PathVerb::Line => {
                    debug_assert!(have_seen_move_to);
                    let prev_d = distance;
                    distance = self.compute_line_seg(pts[0], pts[1], distance, pt_index);
                    if distance > prev_d {
                        self.pts.push(pts[1]);
                        pt_index += 1;
                    }
                }
                PathVerb::Quad => {
                    debug_assert!(have_seen_move_to);
                    let prev_d = distance;
                    distance = self.compute_quad_segs(pts, distance, 0, MAX_T_VALUE, pt_index, 0);
                    if distance > prev_d {
                        self.pts.extend_from_slice(&pts[1..3]);
                        pt_index += 2;
                    }
                }
                PathVerb::Conic => {
                    debug_assert!(have_seen_move_to);
                    let conic = Conic::from_points(pts, w.unwrap_or(1.0));
                    let prev_d = distance;
                    distance = self.compute_conic_segs(
                        &conic,
                        distance,
                        0,
                        conic.pts[0],
                        MAX_T_VALUE,
                        conic.pts[2],
                        pt_index,
                        0,
                    );
                    if distance > prev_d {
                        // we store the conic weight in our next point, followed by the last 2 pts
                        // thus to reconstitue a conic, you'd need to say
                        // SkConic(pts[0], pts[2], pts[3], weight = pts[1].fX)
                        self.pts.push(Point::new(conic.w, 0.0));
                        self.pts.extend_from_slice(&pts[1..3]);
                        pt_index += 3;
                    }
                }
                PathVerb::Cubic => {
                    debug_assert!(have_seen_move_to);
                    let prev_d = distance;
                    distance = self.compute_cubic_segs(pts, distance, 0, MAX_T_VALUE, pt_index, 0);
                    if distance > prev_d {
                        self.pts.extend_from_slice(&pts[1..4]);
                        pt_index += 3;
                    }
                }
                PathVerb::Close => {
                    have_seen_close = true;
                }
            }
        }
        self.iter = it.position();

        if !is_finite(distance) {
            return None;
        }
        if self.segments.is_empty() {
            return None;
        }

        if have_seen_close {
            let prev_d = distance;
            let first_pt = self.pts[0];
            distance =
                self.compute_line_seg(self.pts[pt_index as usize], first_pt, distance, pt_index);
            if distance > prev_d {
                self.pts.push(first_pt);
            }
        }

        Some(ContourMeasure {
            inner: Arc::new(ContourMeasureData {
                segments: std::mem::take(&mut self.segments),
                pts: std::mem::take(&mut self.pts),
                length: distance,
                is_closed: have_seen_close,
            }),
        })
    }
}

// Port of: src/core/SkContourMeasure.cpp#L465-L498 (chrome/m156)
fn compute_pos_tan(
    pts: &[Point],
    seg_type: u8,
    t: scalar,
    pos: Option<&mut Point>,
    tangent: Option<&mut Vector>,
) {
    match seg_type {
        LINE_SEG_TYPE => {
            if let Some(pos) = pos {
                pos.set(
                    scalar_interp(pts[0].x, pts[1].x, t),
                    scalar_interp(pts[0].y, pts[1].y, t),
                );
            }
            if let Some(tangent) = tangent {
                tangent.set_normalize(pts[1].x - pts[0].x, pts[1].y - pts[0].y);
            }
        }
        QUAD_SEG_TYPE => {
            let has_tangent = tangent.is_some();
            let mut tan = Vector::default();
            eval_quad_at_pos_tangent(pts, t, pos, has_tangent.then_some(&mut tan));
            if let Some(tangent) = tangent {
                tan.normalize();
                *tangent = tan;
            }
        }
        CONIC_SEG_TYPE => {
            let has_tangent = tangent.is_some();
            let mut tan = Vector::default();
            Conic::new(pts[0], pts[2], pts[3], pts[1].x).eval_at_pos_tangent(
                t,
                pos,
                has_tangent.then_some(&mut tan),
            );
            if let Some(tangent) = tangent {
                tan.normalize();
                *tangent = tan;
            }
        }
        CUBIC_SEG_TYPE => {
            let has_tangent = tangent.is_some();
            let mut tan = Vector::default();
            eval_cubic_at(pts, t, pos, has_tangent.then_some(&mut tan), None);
            if let Some(tangent) = tangent {
                tan.normalize();
                *tangent = tan;
            }
        }
        _ => debug_assert!(false, "unknown segType"),
    }
}

#[derive(Debug)]
struct ContourMeasureData {
    segments: Vec<Segment>,
    pts: Vec<Point>, // Points used to define the segments
    length: scalar,
    is_closed: bool,
}

/// The measurements of one contour of a path. Cheap to clone (shared).
// Port of: include/core/SkContourMeasure.h#L24-L194 (chrome/m156)
#[doc(alias = "SkContourMeasure")]
#[derive(Clone, Debug)]
pub struct ContourMeasure {
    inner: Arc<ContourMeasureData>,
}

// Port of: src/core/SkContourMeasure.cpp#L549-L577 (chrome/m156)
#[allow(clippy::manual_midpoint)] // mirrors the C++ `(a + b) / 2` arithmetic
#[allow(clippy::cast_sign_loss)] // mirrors the C++ casts
fn tk_search(base: &[Segment], key: scalar) -> i32 {
    let count = base.len();
    if count == 0 {
        return !0;
    }

    let mut lo: usize = 0;
    let mut hi: usize = count - 1;

    while lo < hi {
        let mid = (hi + lo) >> 1;
        if base[mid].distance < key {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // int index
    let mut hi = hi as i32;
    if base[hi as usize].distance < key {
        hi += 1;
        hi = !hi;
    } else if key < base[hi as usize].distance {
        hi = !hi;
    }
    hi
}

impl ContourMeasure {
    /// The length of the contour.
    #[must_use]
    pub fn length(&self) -> scalar {
        self.inner.length
    }

    /// True if the contour is closed.
    #[doc(alias = "isClosed")]
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed
    }

    /// The number of segments used to measure the contour (`SkPathMeasurePriv::CountSegments`).
    #[must_use]
    pub(crate) fn count_segments(&self) -> usize {
        self.inner.segments.len()
    }

    // Port of: src/core/SkContourMeasure.cpp#L579-L609 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // index >= 0 after the xor
    fn distance_to_segment(&self, distance: scalar) -> (usize, scalar) {
        debug_assert!(distance >= 0.0 && distance <= self.length());

        let segs = &self.inner.segments;

        let mut index = tk_search(segs, distance);
        // don't care if we hit an exact match or not, so we xor index if it is negative
        index ^= index >> 31;
        let index = index as usize;
        let seg = &segs[index];

        // now interpolate t-values with the prev segment (if possible)
        let mut start_t = 0.0;
        let mut start_d = 0.0;
        // check if the prev segment is legal, and references the same set of points
        if index > 0 {
            start_d = segs[index - 1].distance;
            if segs[index - 1].pt_index == seg.pt_index {
                debug_assert_eq!(segs[index - 1].ty, seg.ty);
                start_t = segs[index - 1].get_scalar_t();
            }
        }

        debug_assert!(seg.get_scalar_t() > start_t);
        debug_assert!(distance >= start_d);
        debug_assert!(seg.distance > start_d);

        let t = start_t
            + (seg.get_scalar_t() - start_t) * (distance - start_d) / (seg.distance - start_d);
        (index, t)
    }

    /// Writes the position and/or tangent at `distance` (pinned to `0..=length`). Returns false
    /// if there is no answer (e.g. `distance` is NaN).
    // Port of: src/core/SkContourMeasure.cpp#L611-L635 (chrome/m156)
    #[doc(alias = "getPosTan")]
    #[must_use]
    pub fn get_pos_tan(
        &self,
        distance: scalar,
        position: Option<&mut Point>,
        tangent: Option<&mut Vector>,
    ) -> bool {
        if is_nan(distance) {
            return false;
        }

        let length = self.length();
        debug_assert!(length > 0.0 && !self.inner.segments.is_empty());

        // pin the distance to a legal range
        let distance = if distance < 0.0 {
            0.0
        } else if distance > length {
            length
        } else {
            distance
        };

        let (index, t) = self.distance_to_segment(distance);
        if is_nan(t) {
            return false;
        }

        let seg = &self.inner.segments[index];
        debug_assert!((seg.pt_index as usize) < self.inner.pts.len());
        compute_pos_tan(
            &self.inner.pts[seg.pt_index as usize..],
            seg.ty,
            t,
            position,
            tangent,
        );
        true
    }

    /// The position and tangent at `distance`.
    #[must_use]
    pub fn pos_tan(&self, distance: scalar) -> Option<(Point, Vector)> {
        let mut p = Point::default();
        let mut v = Vector::default();
        self.get_pos_tan(distance, Some(&mut p), Some(&mut v))
            .then_some((p, v))
    }

    /// A matrix positioned and/or rotated to the point at `distance`.
    // Port of: src/core/SkContourMeasure.cpp#L637-L655 (chrome/m156)
    #[doc(alias = "getMatrix")]
    #[must_use]
    pub fn get_matrix(
        &self,
        distance: scalar,
        flags: impl Into<Option<MatrixFlags>>,
    ) -> Option<Matrix> {
        let flags = flags.into().unwrap_or_default();
        let (position, tangent) = self.pos_tan(distance)?;
        let mut matrix = Matrix::default();
        if flags.contains(MatrixFlags::GET_TANGENT) {
            matrix.set_sin_cos((tangent.y, tangent.x), Point::new(0.0, 0.0));
        } else {
            matrix.reset();
        }
        if flags.contains(MatrixFlags::GET_POSITION) {
            matrix.post_translate((position.x, position.y));
        }
        Some(matrix)
    }

    /// Appends the part of the contour between `start_d` and `stop_d` to `dst`.
    // Port of: src/core/SkContourMeasure.cpp#L657-L704 (chrome/m156)
    #[doc(alias = "getSegment")]
    #[must_use]
    #[allow(clippy::neg_cmp_op_on_partial_ord)] // NaN must fail the test, as in C++
    pub fn get_segment(
        &self,
        start_d: scalar,
        stop_d: scalar,
        dst: &mut PathBuilder,
        start_with_move_to: bool,
    ) -> bool {
        let length = self.length(); // ensure we have built our segments

        let start_d = if start_d < 0.0 { 0.0 } else { start_d };
        let stop_d = if stop_d > length { length } else { stop_d };
        if !(start_d <= stop_d) {
            // catch NaN values as well
            return false;
        }
        let segs = &self.inner.segments;
        let pts = &self.inner.pts;
        if segs.is_empty() {
            return false;
        }

        let (mut seg, mut start_t) = self.distance_to_segment(start_d);
        if !is_finite(start_t) {
            return false;
        }
        let (stop_seg, stop_t) = self.distance_to_segment(stop_d);
        if !is_finite(stop_t) {
            return false;
        }
        debug_assert!(seg <= stop_seg);
        if start_with_move_to {
            let mut p = Point::default();
            compute_pos_tan(
                &pts[segs[seg].pt_index as usize..],
                segs[seg].ty,
                start_t,
                Some(&mut p),
                None,
            );
            dst.move_to(p);
        }

        if segs[seg].pt_index == segs[stop_seg].pt_index {
            contour_measure_seg_to(
                &pts[segs[seg].pt_index as usize..],
                segs[seg].ty,
                start_t,
                stop_t,
                dst,
            );
        } else {
            loop {
                contour_measure_seg_to(
                    &pts[segs[seg].pt_index as usize..],
                    segs[seg].ty,
                    start_t,
                    SCALAR_1,
                    dst,
                );
                // Segment::Next
                let pt_index = segs[seg].pt_index;
                loop {
                    seg += 1;
                    if segs[seg].pt_index != pt_index {
                        break;
                    }
                }
                start_t = 0.0;
                if segs[seg].pt_index >= segs[stop_seg].pt_index {
                    break;
                }
            }
            contour_measure_seg_to(
                &pts[segs[seg].pt_index as usize..],
                segs[seg].ty,
                0.0,
                stop_t,
                dst,
            );
        }

        true
    }

    /// Iterates the verbs of the contour with their cumulative distances.
    // Port of: include/core/SkContourMeasure.h#L157-L163 (chrome/m156)
    #[doc(alias = "begin")]
    #[must_use]
    pub fn verbs(&self) -> ForwardVerbIterator<'_> {
        ForwardVerbIterator {
            segments: last_seg_for_current_verb(&self.inner.segments),
            pts: &self.inner.pts,
        }
    }
}

/// One verb of a contour, with its cumulative distance (`SkContourMeasure::VerbMeasure`).
// Port of: include/core/SkContourMeasure.h#L105-L109 (chrome/m156)
#[doc(alias = "SkContourMeasure::VerbMeasure")]
#[derive(Copy, Clone, Debug)]
pub struct VerbMeasure<'a> {
    distance: scalar,
    verb: PathVerb,
    pts: &'a [Point],
}

impl VerbMeasure<'_> {
    /// The verb.
    #[must_use]
    pub fn verb(&self) -> PathVerb {
        self.verb
    }

    /// Cumulative distance along the current contour.
    #[must_use]
    pub fn distance(&self) -> scalar {
        self.distance
    }

    /// The verb's points (a conic's weight is stored as `{weight, 0}` after the first point).
    #[must_use]
    pub fn points(&self) -> &[Point] {
        self.pts
    }
}

// Port of: include/core/SkContourMeasure.h#L141-L147 (chrome/m156)
fn last_seg_for_current_verb(segs: &[Segment]) -> &[Segment] {
    let mut i = 1;
    while i < segs.len() && segs[0].pt_index == segs[i].pt_index {
        i += 1;
    }
    &segs[i - 1..]
}

/// Iterates the verbs of a [`ContourMeasure`].
// Port of: include/core/SkContourMeasure.h#L114-L152 (chrome/m156)
#[doc(alias = "SkContourMeasure::ForwardVerbIterator")]
#[derive(Clone, Debug)]
pub struct ForwardVerbIterator<'a> {
    segments: &'a [Segment],
    pts: &'a [Point],
}

impl<'a> Iterator for ForwardVerbIterator<'a> {
    type Item = VerbMeasure<'a>;

    // Port of: src/core/SkContourMeasure.cpp#L706-L733 (chrome/m156)
    fn next(&mut self) -> Option<VerbMeasure<'a>> {
        const SEG_PT_COUNT: [usize; 4] = [
            2, // kLine  (current_pt, 1 line pt)
            3, // kQuad  (current_pt, 2 quad pts)
            4, // kCubic (current_pt, 3 cubic pts)
            4, // kConic (current_pt, {weight, 0}, 2 conic pts)
        ];
        const SEG_VERB: [PathVerb; 4] = [
            PathVerb::Line,
            PathVerb::Quad,
            PathVerb::Cubic,
            PathVerb::Conic,
        ];

        let front = self.segments.first()?;
        let ty = usize::from(front.ty);
        let start = front.pt_index as usize;
        debug_assert!(start + SEG_PT_COUNT[ty] <= self.pts.len());
        let vm = VerbMeasure {
            distance: front.distance,
            verb: SEG_VERB[ty],
            pts: &self.pts[start..start + SEG_PT_COUNT[ty]],
        };
        self.segments = last_seg_for_current_verb(&self.segments[1..]);
        Some(vm)
    }
}

/// Iterates the contours of a path, returning a [`ContourMeasure`] for each non-empty one.
// Port of: include/core/SkContourMeasure.h#L196-L213 (chrome/m156)
#[doc(alias = "SkContourMeasureIter")]
#[derive(Debug, Default)]
pub struct ContourMeasureIter {
    imp: Option<Impl>,
}

impl ContourMeasureIter {
    /// Iterates `path`; if `force_closed`, each contour is measured as closed. `res_scale`
    /// (default 1) controls the precision of the measure (> 1 increases precision).
    // Port of: src/core/SkContourMeasure.cpp#L507-L510 (chrome/m156)
    #[must_use]
    pub fn new(path: &Path, force_closed: bool, res_scale: impl Into<Option<scalar>>) -> Self {
        let mut it = Self::default();
        it.reset(path, force_closed, res_scale);
        it
    }

    /// Restarts on a new path (or none, if it is not finite).
    // Port of: src/core/SkContourMeasure.cpp#L519-L525 (chrome/m156)
    pub fn reset(
        &mut self,
        path: &Path,
        force_closed: bool,
        res_scale: impl Into<Option<scalar>>,
    ) -> &mut Self {
        if path.is_finite() {
            self.imp = Some(Impl::new(
                path,
                force_closed,
                res_scale.into().unwrap_or(1.0),
            ));
        } else {
            self.imp = None;
        }
        self
    }
}

impl Iterator for ContourMeasureIter {
    type Item = ContourMeasure;

    /// The measure of the next contour with a non-zero length, or `None` at the end.
    // Port of: src/core/SkContourMeasure.cpp#L527-L538 (chrome/m156)
    fn next(&mut self) -> Option<ContourMeasure> {
        let imp = self.imp.as_mut()?;
        while imp.has_next_segments() {
            if let Some(cm) = imp.build_segments() {
                return Some(cm);
            }
        }
        None
    }
}
