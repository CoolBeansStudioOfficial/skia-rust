// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkPathPriv.h, src/core/SkPathPriv.cpp, src/core/SkPathRaw.cpp,
// src/core/SkPath.cpp (SkPathPriv / SkPathEdgeIter parts)

//! Private path helpers used by Skia's own code and tests (`SkPathPriv`).

use crate::cubic_clipper::CubicClipper;
use crate::floating_point::{ieee_double_divide, is_finite};
use crate::geometry::Conic;
use crate::geometry::{
    chop_cubic_at_y_extrema, chop_quad_at_y_extrema, eval_cubic_at, eval_quad_at,
    eval_quad_tangent_at, find_cubic_extrema, find_quad_extrema, find_unit_quad_roots,
};
use crate::matrix::{Matrix, Member};
use crate::path::Path;
use crate::path_builder::PathBuilder;
use crate::path_data::PathData;
use crate::path_enums::{PathConvexity, PathFirstDirection, ResolveConvexity};
use crate::path_iter::PathIter;
use crate::path_raw::PathRaw;
use crate::path_ref::{PathOvalInfo, PathRRectInfo, PathRectInfo};
use crate::path_types::{PathDirection, PathFillType, PathSegmentMask, PathVerb};
use crate::point::{Point, Vector, point_priv};
use crate::rect::{Contains, Rect};
use crate::rrect::{Corner, RRect};
use crate::scalar::{SCALAR_1, SCALAR_MAX, Scalar, scalar, scalar_abs, scalar_sign_as_int};

// The edge clipper is only reached through `perspective_clip`.
use crate::edge_clipper::EdgeClipper;

/// `SkPathPriv::kW0PlaneDistance`: not a perfect solution for W plane clipping, but 1/16384 is a
/// reasonable limit (roughly 5e-5). See skbug.com/40041027.
// Port of: src/core/SkPathPriv.h#L67-L69 (chrome/m156)
#[doc(alias = "kW0PlaneDistance")]
#[allow(clippy::cast_precision_loss)] // 1 << 14 is exact
pub const W0_PLANE_DISTANCE: scalar = 1.0 / ((1 << 14) as scalar);

/// What a round rect simplifies to when added to a path.
// Port of: src/core/SkPathPriv.h#L47-L49 (chrome/m156)
#[doc(alias = "SkPathPriv::RRectAsEnum")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RRectAsEnum {
    Rect,
    Oval,
    RRect,
}

/// `SkPathPriv::SimplifyRRect`.
// Port of: src/core/SkPathPriv.h#L50-L58 (chrome/m156)
#[doc(alias = "SimplifyRRect")]
#[must_use]
pub fn simplify_rrect(rr: &RRect, start_index: u32) -> (RRectAsEnum, u32) {
    if rr.is_rect() || rr.is_empty() {
        return (RRectAsEnum::Rect, start_index.div_ceil(2));
    }
    if rr.is_oval() {
        return (RRectAsEnum::Oval, start_index / 2);
    }
    (RRectAsEnum::RRect, start_index)
}

const VERB_TO_SEGMENT_MASK: [u8; 6] = [
    0, // move
    1, // kLine_SkPathSegmentMask
    2, // kQuad_SkPathSegmentMask
    4, // kConic_SkPathSegmentMask
    8, // kCubic_SkPathSegmentMask
    0, // close
];

/// `SkPathPriv::ComputeSegmentMask`.
// Port of: src/core/SkPathRaw.cpp#L23-L31 (chrome/m156)
#[doc(alias = "ComputeSegmentMask")]
#[must_use]
pub fn compute_segment_mask(verbs: &[PathVerb]) -> u8 {
    let mut mask = 0;
    for &v in verbs {
        mask |= VERB_TO_SEGMENT_MASK[v as usize];
    }
    mask
}

/// `SkPathPriv::AsFirstDirection`: since we agree numerically for the values in Direction, we
/// can just cast.
// Port of: src/core/SkPathPriv.h#L71-L74 (chrome/m156)
#[doc(alias = "AsFirstDirection")]
#[must_use]
pub fn as_first_direction(dir: PathDirection) -> PathFirstDirection {
    match dir {
        PathDirection::CW => PathFirstDirection::CW,
        PathDirection::CCW => PathFirstDirection::CCW,
    }
}

/// The opposite of the specified direction. `Unknown` is its own opposite.
// Port of: src/core/SkPathPriv.h#L80-L85 (chrome/m156)
#[doc(alias = "OppositeFirstDirection")]
#[must_use]
pub fn opposite_first_direction(dir: PathFirstDirection) -> PathFirstDirection {
    match dir {
        PathFirstDirection::CW => PathFirstDirection::CCW,
        PathFirstDirection::CCW => PathFirstDirection::CW,
        PathFirstDirection::Unknown => PathFirstDirection::Unknown,
    }
}

/// `SkPathPriv::IsClosedSingleContour(verbs)`.
// Port of: src/core/SkPathPriv.h#L96-L117 (chrome/m156)
#[doc(alias = "IsClosedSingleContour")]
#[must_use]
pub fn is_closed_single_contour(verbs: &[PathVerb]) -> bool {
    if verbs.is_empty() {
        return false;
    }

    let mut move_count = 0;
    for (i, &verb) in verbs.iter().enumerate() {
        match verb {
            PathVerb::Move => {
                move_count += 1;
                if move_count > 1 {
                    return false;
                }
            }
            PathVerb::Close => return i == verbs.len() - 1,
            _ => {}
        }
    }
    false
}

/// `SkPathPriv::IsClosedSingleContour(path)`.
// Port of: src/core/SkPathPriv.h#L119-L121 (chrome/m156)
#[must_use]
pub fn is_closed_single_contour_path(path: &Path) -> bool {
    is_closed_single_contour(path.verbs())
}

/// The index of the last moveTo point, based on the verbs. Returns -1 if `verbs` is empty.
// Port of: src/core/SkPathPriv.cpp#L1530-L1548 (chrome/m156)
#[doc(alias = "FindLastMoveToIndex")]
#[must_use]
pub fn find_last_move_to_index(verbs: &[PathVerb], pt_count: usize) -> i32 {
    if verbs.is_empty() {
        debug_assert_eq!(pt_count, 0);
        return -1;
    }
    debug_assert_eq!(verbs[0], PathVerb::Move);
    debug_assert!(pt_count > 0);

    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // SkToInt
    let mut pt_index = pt_count as i32 - 1;
    for &verb in verbs.iter().rev() {
        if verb == PathVerb::Move {
            break;
        }
        pt_index -= pts_in_verb(verb);
    }
    debug_assert!(pt_index >= 0);
    pt_index
}

/// Returns the number of valid points for each `SkPath::Iter` verb.
// Port of: src/core/SkPathPriv.h#L270-L284 (chrome/m156)
#[doc(alias = "PtsInIter")]
#[must_use]
pub fn pts_in_iter(verb: PathVerb) -> i32 {
    const PTS_IN_VERB: [i32; 6] = [1, 2, 3, 3, 4, 0];
    PTS_IN_VERB[verb as usize]
}

/// Returns the number of valid points for each verb, not including the "starter" point that the
/// iterator adds for line/quad/conic/cubic.
// Port of: src/core/SkPathPriv.h#L288-L302 (chrome/m156)
#[doc(alias = "PtsInVerb")]
#[must_use]
pub fn pts_in_verb(verb: PathVerb) -> i32 {
    const PTS_IN_VERB: [i32; 6] = [1, 1, 2, 2, 3, 0];
    PTS_IN_VERB[verb as usize]
}

/// `pts_in_verb` as a `usize`.
#[must_use]
pub(crate) fn pts_in_verb_usize(verb: PathVerb) -> usize {
    const PTS_IN_VERB: [usize; 6] = [1, 1, 2, 2, 3, 0];
    PTS_IN_VERB[verb as usize]
}

/// True if all segments (between consecutive points) are axis-aligned.
///
/// Conservative (quick) test: multiple contours might give a false-negative, but for speed, we
/// ignore that and just look at the raw points.
// Port of: src/core/SkPathPriv.cpp#L1399-L1410 (chrome/m156)
#[doc(alias = "IsAxisAligned")]
#[must_use]
#[allow(clippy::float_cmp)] // exact float comparisons, as in C++
pub fn is_axis_aligned(pts: &[Point]) -> bool {
    for i in 1..pts.len() {
        if pts[i - 1].x != pts[i].x && pts[i - 1].y != pts[i].y {
            return false;
        }
    }
    true
}

/// True if every point equals the first.
// Port of: src/core/SkPathPriv.h#L308-L315 (chrome/m156)
#[doc(alias = "AllPointsEq")]
#[must_use]
pub fn all_points_eq(pts: &[Point]) -> bool {
    for i in 1..pts.len() {
        if pts[0] != pts[i] {
            return false;
        }
    }
    true
}

/// `SkPathPriv::TooBigForMath`: true if `bounds` is too close to max float for small multiplies
/// (or contains NaN).
// Port of: src/core/SkPathPriv.h#L257-L267 (chrome/m156)
#[doc(alias = "TooBigForMath")]
#[must_use]
pub fn too_big_for_math(bounds: &Rect) -> bool {
    // This value is just a guess. smaller is safer, but we don't want to reject largish paths
    // that we don't have to.
    const SCALE_DOWN_TO_ALLOW_FOR_SMALL_MULTIPLIES: scalar = 0.25;
    const MAX: scalar = SCALAR_MAX * SCALE_DOWN_TO_ALLOW_FOR_SMALL_MULTIPLIES;

    // use ! expression so we return true if bounds contains NaN
    !(bounds.left >= -MAX && bounds.top >= -MAX && bounds.right <= MAX && bounds.bottom <= MAX)
}

/// `SkPathPriv::IsInverseFillType`.
// Port of: src/core/SkPathPriv.h#L346-L348 (chrome/m156)
#[doc(alias = "IsInverseFillType")]
#[must_use]
pub fn is_inverse_fill_type(fill: PathFillType) -> bool {
    fill.is_inverse()
}

/// `SkPathPriv::IsOval`: the oval info if the path was created as an oval or circle.
// Port of: src/core/SkPathPriv.h#L240-L243 (chrome/m156)
#[doc(alias = "IsOval")]
#[must_use]
pub fn is_oval(path: &Path) -> Option<PathOvalInfo> {
    path.get_oval_info()
}

/// `SkPathPriv::IsRRect`: the rrect info if the path was created as one.
// Port of: src/core/SkPathPriv.h#L245-L248 (chrome/m156)
#[doc(alias = "IsRRect")]
#[must_use]
pub fn is_rrect(path: &Path) -> Option<PathRRectInfo> {
    path.get_rrect_info()
}

/// `SkPathPriv::GetConvexity`.
#[doc(alias = "GetConvexity")]
#[must_use]
pub fn get_convexity(path: &Path) -> PathConvexity {
    path.get_convexity()
}

/// `SkPathPriv::GetConvexityOrUnknown(const SkPath&)`.
#[doc(alias = "GetConvexityOrUnknown")]
#[must_use]
pub fn get_convexity_or_unknown(path: &Path) -> PathConvexity {
    path.get_convexity_or_unknown()
}

/// `SkPathPriv::GetConvexityOrUnknown(const SkPathData&)`.
#[must_use]
pub fn get_convexity_or_unknown_data(pdata: &PathData) -> PathConvexity {
    pdata.get_convexity_or_unknown()
}

/// `SkPathPriv::SetConvexity`.
#[doc(alias = "SetConvexity")]
pub fn set_convexity(path: &Path, c: PathConvexity) {
    path.set_convexity(c);
}

/// `SkPathPriv::ForceComputeConvexity`.
// Port of: src/core/SkPathPriv.h#L376-L379 (chrome/m156)
#[doc(alias = "ForceComputeConvexity")]
pub fn force_compute_convexity(path: &Path) {
    path.set_convexity(PathConvexity::Unknown);
    let _ = path.is_convex();
}

/// `SkPathPriv::ReverseAddPath`.
#[doc(alias = "ReverseAddPath")]
pub fn reverse_add_path(builder: &mut PathBuilder, reverse_me: &Path) {
    builder.private_reverse_add_path(reverse_me);
}

/// `SkPathPriv::ReversePathTo`.
#[doc(alias = "ReversePathTo")]
pub fn reverse_path_to(builder: &mut PathBuilder, reverse_me: &Path) {
    builder.private_reverse_path_to(reverse_me);
}

/// `SkPathPriv::ReversePath`.
// Port of: src/core/SkPathPriv.h#L393-L397 (chrome/m156)
#[doc(alias = "ReversePath")]
#[must_use]
pub fn reverse_path(reverse_me: &Path) -> Path {
    let mut bu = PathBuilder::new();
    bu.private_reverse_add_path(reverse_me);
    bu.detach()
}

/// `SkPathPriv::GetVerbs(const SkPathBuilder&)`.
#[doc(alias = "GetVerbs")]
#[must_use]
pub fn get_verbs(builder: &PathBuilder) -> &[PathVerb] {
    builder.verbs()
}

/// `SkPathPriv::CountVerbs(const SkPathBuilder&)`.
#[doc(alias = "CountVerbs")]
#[must_use]
pub fn count_verbs(builder: &PathBuilder) -> usize {
    builder.verbs().len()
}

/// `SkPathPriv::Raw(const SkPath&, SkResolveConvexity)`.
#[doc(alias = "Raw")]
#[must_use]
pub fn raw(path: &Path, rc: ResolveConvexity) -> Option<PathRaw<'_>> {
    path.raw_view(rc)
}

/// `SkPathPriv::Raw(const SkPathBuilder&, SkResolveConvexity)`.
// Port of: src/core/SkPathPriv.h#L411-L433 (chrome/m156)
#[must_use]
pub fn raw_builder(builder: &PathBuilder, rc: ResolveConvexity) -> Option<PathRaw<'_>> {
    let bounds = builder.compute_finite_bounds()?;

    let mut convexity = builder.convexity;
    if convexity == PathConvexity::Unknown && rc == ResolveConvexity::Yes {
        convexity = compute_convexity(builder.points(), builder.verbs(), builder.conic_weights());
    }

    #[allow(clippy::cast_possible_truncation)] // SkTo<uint8_t>: the mask fits in 4 bits
    let segment_mask = builder.segment_mask as u8;
    Some(PathRaw {
        points: builder.points(),
        verbs: builder.verbs(),
        conics: builder.conic_weights(),
        bounds,
        fill_type: builder.fill_type(),
        convexity,
        segment_mask,
    })
}

/// Returns empty if there are no points, `None` if the bounds are not finite.
///
/// A trailing move contributes to the bounds only if it is the only verb in the path.
// Port of: src/core/SkPathPriv.h#L437-L451 (chrome/m156)
#[doc(alias = "TrimmedBounds")]
#[must_use]
pub fn trimmed_bounds(pts: &[Point], vbs: &[PathVerb]) -> Option<Rect> {
    let mut pts = pts;
    // Does a trailing kMove verb contribute to the bounds?
    // - only if it is the only verb in the path
    // - otherwise we ignore it when computing bounds
    if vbs.len() > 1 && vbs.last() == Some(&PathVerb::Move) {
        debug_assert_ne!(pts, []);
        // While trailing moves do not contribute to the bounds, we still reject them.
        if !pts[pts.len() - 1].is_finite() {
            return None;
        }
        pts = &pts[..pts.len() - 1];
    }
    Rect::bounds(pts)
}

///////////////////////////////////////////////////////////////////////////////////////////////////
// Iterate / RangeIter

/// `SkPathPriv::RangeIter` (`SkPath::RangeIter`): iterates a raw range of verbs, points and
/// conic weights. All values are returned unaltered.
///
/// Each item is `(verb, pts, weight)`. `pts` starts one point *before* the verb's own points (the
/// C++ "backset"), so `pts[0]` is the current point: a move yields 1 point, a line 2, quad and
/// conic 3, cubic 4, and close 1 (the last point). `weight` is the conic weight for conics.
// Port of: include/core/SkPath.h#L913-L996 (chrome/m156)
#[doc(alias = "SkPathPriv::RangeIter")]
#[doc(alias = "SkPath::RangeIter")]
#[derive(Clone, Debug)]
pub struct RangeIter<'a> {
    verbs: &'a [PathVerb],
    points: &'a [Point],
    weights: &'a [scalar],
    v: usize,
    p: usize,
    w: usize,
}

impl RangeIter<'_> {
    /// The next verb, without advancing.
    #[doc(alias = "peekVerb")]
    #[must_use]
    pub fn peek_verb(&self) -> Option<PathVerb> {
        self.verbs.get(self.v).copied()
    }

    /// True if the iterator is at the end (`iter == iterate.end()`).
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.v >= self.verbs.len()
    }

    /// The (verb, point, weight) cursor, to resume a later iteration with [`Self::set_position`].
    pub(crate) fn position(&self) -> (usize, usize, usize) {
        (self.v, self.p, self.w)
    }

    pub(crate) fn set_position(&mut self, (v, p, w): (usize, usize, usize)) {
        self.v = v;
        self.p = p;
        self.w = w;
    }
}

impl<'a> Iterator for RangeIter<'a> {
    type Item = (PathVerb, &'a [Point], Option<scalar>);

    fn next(&mut self) -> Option<Self::Item> {
        let verb = *self.verbs.get(self.v)?;
        self.v += 1;
        // We provide the starting point for beziers by peeking backwards from the current
        // point, which works fine as long as there is always a kMove before any geometry.
        let (start, len) = match verb {
            PathVerb::Move => (self.p, 1),
            PathVerb::Line => (self.p - 1, 2),
            PathVerb::Quad | PathVerb::Conic => (self.p - 1, 3),
            PathVerb::Cubic => (self.p - 1, 4),
            PathVerb::Close => (self.p - 1, 1),
        };
        let pts = &self.points[start..start + len];
        self.p += pts_in_verb_usize(verb);
        let weight = if verb == PathVerb::Conic {
            let w = self.weights[self.w];
            self.w += 1;
            Some(w)
        } else {
            None
        };
        Some((verb, pts, weight))
    }
}

/// `SkPathPriv::Iterate(path)`: iterates the verbs, points and weights of `path`. A non-finite
/// path yields nothing.
// Port of: src/core/SkPathPriv.h#L205-L220 (chrome/m156)
#[doc(alias = "Iterate")]
#[must_use]
pub fn iterate(path: &Path) -> RangeIter<'_> {
    let mut it = iterate_raw(path.verbs(), path.points(), path.conic_weights());
    // Don't allow iteration through non-finite points.
    if !path.is_finite() {
        it.v = it.verbs.len();
    }
    it
}

/// `SkPathPriv::Iterate(verbs, points, weights)`.
// Port of: src/core/SkPathPriv.h#L221-L227 (chrome/m156)
#[must_use]
pub fn iterate_raw<'a>(
    verbs: &'a [PathVerb],
    points: &'a [Point],
    weights: &'a [scalar],
) -> RangeIter<'a> {
    RangeIter {
        verbs,
        points,
        weights,
        v: 0,
        p: 0,
        w: 0,
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////
// Rect detection

/*
 Determines if path is a rect by keeping track of changes in direction
 and looking for a loop either clockwise or counterclockwise.

 The direction is computed such that:
  0: vertical up
  1: horizontal left
  2: vertical down
  3: horizontal right

A rectangle cycles up/right/down/left or up/left/down/right.

 directions values:
    0x1 is set if the segment is horizontal
    0x2 is set if the segment is moving to the right or down
 thus:
    two directions are opposites iff (dirA ^ dirB) == 0x2
    two directions are perpendicular iff (dirA ^ dirB) == 0x1
 */
// Port of: src/core/SkPathPriv.cpp#L82-L84 (chrome/m156)
fn rect_make_dir(dx: scalar, dy: scalar) -> i32 {
    i32::from(dx != 0.0) | (i32::from(dx > 0.0 || dy > 0.0) << 1)
}

/// The result of [`is_rect_contour`].
// Port of: src/core/SkPathPriv.h#L317-L323 (chrome/m156)
#[doc(alias = "SkPathPriv::RectContour")]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RectContour {
    pub rect: Rect,
    pub is_closed: bool,
    pub direction: PathDirection,
    pub points_consumed: usize,
    pub verbs_consumed: usize,
}

// Quick check for a "trivial" rect, i.e. a rect created via SkPath::Rect(),
// SkPathBuilder::addRect(), etc.
// Port of: src/core/SkPathPriv.cpp#L88-L144 (chrome/m156)
#[allow(clippy::needless_bitwise_bool)] // mirrors the C++ non-short-circuiting `&`
fn trivial_rect(pts: &[Point], vbs: &[PathVerb]) -> Option<RectContour> {
    const TRIVIAL_VERBS: [PathVerb; 5] = [
        PathVerb::Move,
        PathVerb::Line,
        PathVerb::Line,
        PathVerb::Line,
        PathVerb::Close,
    ];
    if pts.len() != 4 || vbs != TRIVIAL_VERBS {
        return None;
    }

    let v0 = pts[1] - pts[0];
    let v1 = pts[2] - pts[1];
    let v2 = pts[3] - pts[2];
    let v3 = pts[0] - pts[3];

    let axis_aligned_orthogonal = |a: Vector, b: Vector| -> bool {
        // Assuming one of the vectors is known to be axis-aligned,
        // check whether the other one is orthogonal (and implicitly axis-aligned).
        ((a.x == 0.0) ^ (b.x == 0.0)) & ((a.y == 0.0) ^ (b.y == 0.0))
    };

    // We have a rect iff the side vectors are axis aligned and form 3 right corners.
    // This can be further reduced to one axis aligned vector and 3 orthogonal vectors.
    if !(((v0.x == 0.0) ^ (v0.y == 0.0))
        & axis_aligned_orthogonal(v0, v1)
        & axis_aligned_orthogonal(v1, v2)
        & axis_aligned_orthogonal(v2, v3))
    {
        return None;
    }

    let rect = Rect::from_ltrb(pts[0].x, pts[0].y, pts[2].x, pts[2].y).sorted();
    let dir = if Point::cross_product(v0, v1) > 0.0 {
        PathDirection::CW
    } else {
        PathDirection::CCW
    };

    Some(RectContour {
        rect,
        is_closed: true,
        direction: dir,
        points_consumed: pts.len(),
        verbs_consumed: vbs.len(),
    })
}

/// If the (first) contour is a rectangle, returns it.
///
/// The test fails if: the path is closed and followed by a line; a second move creates a new
/// endpoint; a diagonal line is parsed; there are more than four changes of direction; there is a
/// discontinuity on the line; the line reverses direction; the path contains a curve; the path
/// contains fewer than four points; the rectangle doesn't complete a cycle; the final point isn't
/// equal to the first point (the last two are relaxed for 3-edge paths).
// Port of: src/core/SkPathPriv.cpp#L146-L299 (chrome/m156)
#[doc(alias = "IsRectContour")]
#[allow(clippy::too_many_lines)] // mirrors the C++ function
#[must_use]
pub fn is_rect_contour(
    pt_span: &[Point],
    vb_span: &[PathVerb],
    segment_mask: u32,
    allow_partial: bool,
) -> Option<RectContour> {
    if segment_mask != PathSegmentMask::LINE.bits() || pt_span.len() < 4 || vb_span.len() < 4 {
        return None;
    }

    if let Some(rc) = trivial_rect(pt_span, vb_span) {
        return Some(rc);
    }

    let mut curr_verb: usize = 0;
    let verb_cnt = vb_span.len();

    let mut corners: usize = 0;
    let mut close_xy: Point; // used to determine if final line falls on a diagonal
    let mut line_start = Point::new(0.0, 0.0); // used to construct line from previous point
    let mut first_pt: Option<usize> = None; // first point in the rect (last of first moves)
    let mut last_pt: Option<usize> = None; // last point in the rect (last of lines or first if closed)
    let mut first_corner = Point::default();
    let mut third_corner = Point::default();
    let mut pts: usize = 0;
    let mut save_pts: Option<usize> = None; // used to allow caller to iterate through a pair of rects
    // -1 to 3; -1 is uninitialized
    let mut directions: [i32; 5] = [-1, -1, -1, -1, -1];
    let mut closed_or_moved = false;
    let mut auto_close = false;
    let mut insert_close = false;
    while curr_verb < verb_cnt && (!allow_partial || !auto_close) {
        let verb = if insert_close {
            PathVerb::Close
        } else {
            vb_span[curr_verb]
        };
        match verb {
            PathVerb::Close | PathVerb::Line => 'line: {
                if verb == PathVerb::Close {
                    save_pts = Some(pts);
                    auto_close = true;
                    insert_close = false;
                } else {
                    last_pt = Some(pts);
                }
                let line_end = if verb == PathVerb::Close {
                    pt_span[first_pt?]
                } else {
                    let p = pt_span[pts];
                    pts += 1;
                    p
                };
                let line_delta = line_end - line_start;
                if line_delta.x != 0.0 && line_delta.y != 0.0 {
                    return None; // diagonal
                }
                if !line_delta.is_finite() {
                    return None; // path contains infinity or NaN
                }
                if line_start == line_end {
                    break 'line; // single point on side OK
                }
                let next_direction = rect_make_dir(line_delta.x, line_delta.y); // 0 to 3
                if corners == 0 {
                    directions[0] = next_direction;
                    corners = 1;
                    closed_or_moved = false;
                    line_start = line_end;
                    break 'line;
                }
                if closed_or_moved {
                    return None; // closed followed by a line
                }
                if auto_close && next_direction == directions[0] {
                    break 'line; // colinear with first
                }
                closed_or_moved = auto_close;
                if directions[corners - 1] == next_direction {
                    if corners == 3 && verb == PathVerb::Line {
                        third_corner = line_end;
                    }
                    line_start = line_end;
                    break 'line; // colinear segment
                }
                directions[corners] = next_direction;
                corners += 1;
                // opposite lines must point in opposite directions; xoring them should equal 2
                match corners {
                    2 => first_corner = line_start,
                    3 => {
                        if (directions[0] ^ directions[2]) != 2 {
                            return None;
                        }
                        third_corner = line_end;
                    }
                    4 => {
                        if (directions[1] ^ directions[3]) != 2 {
                            return None;
                        }
                    }
                    _ => return None, // too many direction changes
                }
                line_start = line_end;
            }
            PathVerb::Quad | PathVerb::Conic | PathVerb::Cubic => {
                return None; // quadratic, cubic not allowed
            }
            PathVerb::Move => {
                if allow_partial && !auto_close && directions[0] >= 0 {
                    insert_close = true;
                    curr_verb -= 1; // try move again afterwards
                    // C++: goto addMissingClose (skips the `currVerb += 1` below)
                    continue;
                }
                if corners == 0 {
                    first_pt = Some(pts);
                } else {
                    close_xy = pt_span[first_pt?] - pt_span[last_pt?];
                    if close_xy.x != 0.0 && close_xy.y != 0.0 {
                        return None; // we're diagonal, abort
                    }
                }
                line_start = pt_span[pts];
                pts += 1;
                closed_or_moved = true;
            }
        }
        curr_verb += 1;
    }
    // Success if 4 corners and first point equals last
    if !(3..=4).contains(&corners) {
        return None;
    }
    // check if close generates diagonal
    close_xy = pt_span[first_pt?] - pt_span[last_pt?];
    if close_xy.x != 0.0 && close_xy.y != 0.0 {
        return None;
    }

    let mut bounds = Rect::default();
    bounds.set_bounds2(first_corner, third_corner);

    Some(RectContour {
        rect: bounds,
        is_closed: auto_close,
        direction: if directions[0] == ((directions[1] + 1) & 3) {
            PathDirection::CW
        } else {
            PathDirection::CCW
        },
        points_consumed: save_pts.unwrap_or(0),
        verbs_consumed: curr_verb,
    })
}

/// True if the path is equivalent to a nested pair of rectangles when filled; returns the outer
/// and inner rectangles and their directions.
// Port of: src/core/SkPathPriv.cpp#L301-L346 (chrome/m156)
#[doc(alias = "IsNestedFillRects")]
#[must_use]
pub fn is_nested_fill_rects(raw: &PathRaw<'_>) -> Option<([Rect; 2], [PathDirection; 2])> {
    let mut test_dirs = [PathDirection::CW; 2];
    let mut test_rects = [Rect::default(); 2];

    let mut pts = raw.points();
    let mut vbs = raw.verbs();

    let rc = is_rect_contour(pts, vbs, u32::from(raw.segment_mask), true)?;

    test_dirs[0] = rc.direction;
    test_rects[0] = rc.rect;
    pts = &pts[rc.points_consumed..];
    vbs = &vbs[rc.verbs_consumed..];

    if let Some(rc) = is_rect_contour(pts, vbs, u32::from(raw.segment_mask), false) {
        test_dirs[1] = rc.direction;
        test_rects[1] = rc.rect;
        if test_rects[0].contains(&test_rects[1]) {
            return Some((test_rects, test_dirs));
        }
        if test_rects[1].contains(&test_rects[0]) {
            return Some(([test_rects[1], test_rects[0]], [test_dirs[1], test_dirs[0]]));
        }
    }
    None
}

/// `SkPathPriv::IsNestedFillRects(const SkPath&, ...)`.
// Port of: src/core/SkPathPriv.h#L337-L341 (chrome/m156)
#[must_use]
pub fn is_nested_fill_rects_path(path: &Path) -> Option<([Rect; 2], [PathDirection; 2])> {
    let raw = raw(path, ResolveConvexity::No)?;
    is_nested_fill_rects(&raw)
}

///////////////////////////////////////////////////////////////////////////////////////////////////
// Convexity

// Port of: src/core/SkPathPriv.cpp#L422 (chrome/m156)
fn sign(x: scalar) -> i32 {
    i32::from(x < 0.0)
}
const VALUE_NEVER_RETURNED_BY_SIGN: i32 = 2;

// Port of: src/core/SkPathPriv.cpp#L425-L432 (chrome/m156)
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum DirChange {
    Unknown,
    Left,
    Right,
    Straight,
    Backwards, // if double back, allow simple lines to be convex
    Invalid,
}

// only valid for a single contour
// Port of: src/core/SkPathPriv.cpp#L435-L572 (chrome/m156)
#[derive(Debug)]
struct Convexicator {
    first_pt: Point,   // The first point of the contour, e.g. moveTo(x,y)
    first_vec: Vector, // The direction leaving fFirstPt to the next vertex

    last_pt: Point,   // The last point passed to addPt()
    last_vec: Vector, // The direction that brought the path to fLastPt

    expected_dir: DirChange,
    first_direction: PathFirstDirection,
    reversals: i32,
    is_finite: bool,
}

impl Convexicator {
    fn new() -> Self {
        Self {
            first_pt: Point::new(0.0, 0.0),
            first_vec: Vector::new(0.0, 0.0),
            last_pt: Point::new(0.0, 0.0),
            last_vec: Vector::new(0.0, 0.0),
            expected_dir: DirChange::Invalid,
            first_direction: PathFirstDirection::Unknown,
            reversals: 0,
            is_finite: true,
        }
    }

    /** The direction returned is only valid if the path is determined convex */
    fn get_first_direction(&self) -> PathFirstDirection {
        self.first_direction
    }

    fn set_move_pt(&mut self, pt: Point) {
        self.first_pt = pt;
        self.last_pt = pt;
        self.expected_dir = DirChange::Invalid;
    }

    fn add_pt(&mut self, pt: Point) -> bool {
        if self.last_pt == pt {
            return true;
        }
        // should only be true for first non-zero vector after setMovePt was called. It is possible
        // we doubled backed at the start so need to check if fLastVec is zero or not.
        if self.first_pt == self.last_pt
            && self.expected_dir == DirChange::Invalid
            && self.last_vec.equals(0.0, 0.0)
        {
            self.last_vec = pt - self.last_pt;
            self.first_vec = self.last_vec;
        } else if !self.add_vec(pt - self.last_pt) {
            return false;
        }
        self.last_pt = pt;
        true
    }

    #[allow(clippy::similar_names)] // names follow the C++
    fn is_concave_by_sign(points: &[Point]) -> bool {
        let count = points.len();
        if count <= 3 {
            // point, line, or triangle are always convex
            return false;
        }

        let mut curr_pt = points[0];
        let first_pt = curr_pt;
        let mut dxes = 0;
        let mut dyes = 0;
        let mut last_sx = VALUE_NEVER_RETURNED_BY_SIGN;
        let mut last_sy = VALUE_NEVER_RETURNED_BY_SIGN;
        // Returns Some(true) for "concave", None to keep going.
        let mut step = |p: Point, curr_pt: &mut Point| -> bool {
            let vec = p - *curr_pt;
            if !vec.is_zero() {
                // give up if vector construction failed
                if !vec.is_finite() {
                    return true; // treat as concave
                }
                let sx = sign(vec.x);
                let sy = sign(vec.y);
                dxes += i32::from(sx != last_sx);
                dyes += i32::from(sy != last_sy);
                if dxes > 3 || dyes > 3 {
                    return true;
                }
                last_sx = sx;
                last_sy = sy;
            }
            *curr_pt = p;
            false
        };
        // outerLoop == 0: walk all the points.
        for &p in &points[1..] {
            if step(p, &mut curr_pt) {
                return true;
            }
        }
        // outerLoop == 1: one more step, back to the first point.
        if step(first_pt, &mut curr_pt) {
            return true;
        }
        false // that is, it may be convex, don't know yet
    }

    fn close(&mut self) -> bool {
        // If this was an explicit close, there was already a lineTo to fFirstPoint, so this
        // addPt() is a no-op. Otherwise, the addPt implicitly closes the contour. In either case,
        // we have to check the direction change along the first vector in case it is concave.
        self.add_pt(self.first_pt) && self.add_vec(self.first_vec)
    }

    fn reversals(&self) -> i32 {
        self.reversals
    }

    fn direction_change(&self, cur_vec: Vector) -> DirChange {
        let cross = Point::cross_product(self.last_vec, cur_vec);
        if !is_finite(cross) {
            return DirChange::Unknown;
        }
        if cross == 0.0 {
            return if self.last_vec.dot(cur_vec) < 0.0 {
                DirChange::Backwards
            } else {
                DirChange::Straight
            };
        }
        if scalar_sign_as_int(cross) == 1 {
            DirChange::Right
        } else {
            DirChange::Left
        }
    }

    fn add_vec(&mut self, cur_vec: Vector) -> bool {
        let dir = self.direction_change(cur_vec);
        match dir {
            DirChange::Left | DirChange::Right => {
                if self.expected_dir == DirChange::Invalid {
                    self.expected_dir = dir;
                    self.first_direction = if dir == DirChange::Right {
                        PathFirstDirection::CW
                    } else {
                        PathFirstDirection::CCW
                    };
                } else if dir != self.expected_dir {
                    self.first_direction = PathFirstDirection::Unknown;
                    return false;
                }
                self.last_vec = cur_vec;
            }
            DirChange::Straight => {}
            DirChange::Backwards => {
                //  allow path to reverse direction twice
                //    Given path.moveTo(0, 0); path.lineTo(1, 1);
                //    - 1st reversal: direction change formed by line (0,0 1,1), line (1,1 0,0)
                //    - 2nd reversal: direction change formed by line (1,1 0,0), line (0,0 1,1)
                self.last_vec = cur_vec;
                self.reversals += 1;
                return self.reversals < 3;
            }
            DirChange::Unknown => {
                self.is_finite = false;
                return false;
            }
            DirChange::Invalid => panic!("Use of invalid direction change flag"),
        }
        true
    }
}

// Port of: src/core/SkPathPriv.cpp#L574-L584 (chrome/m156)
fn trim_trailing_moves<'a>(pts: &'a [Point], vbs: &'a [PathVerb]) -> (&'a [Point], &'a [PathVerb]) {
    let mut vb_count = vbs.len();
    while vb_count > 0 && vbs[vb_count - 1] == PathVerb::Move {
        vb_count -= 1;
    }
    let delta = vbs.len() - vb_count;
    if delta != 0 {
        debug_assert!(pts.len() >= delta);
        return (&pts[..pts.len() - delta], &vbs[..vb_count]);
    }
    (pts, vbs)
}

/// `SkPathPriv::ComputeConvexity`. Callers need to give finite values.
// Port of: src/core/SkPathPriv.cpp#L586-L659 (chrome/m156)
#[doc(alias = "ComputeConvexity")]
#[must_use]
pub fn compute_convexity(
    points: &[Point],
    vbs: &[PathVerb],
    conic_weights: &[scalar],
) -> PathConvexity {
    // callers need to give us finite values
    debug_assert!(Rect::bounds(points).is_some());

    let (points, vbs) = trim_trailing_moves(points, vbs);

    if vbs.is_empty() {
        return PathConvexity::ConvexDegenerate;
    }

    // Check to see if path changes direction more than three times as quick concave test
    if Convexicator::is_concave_by_sign(points) {
        return PathConvexity::Concave;
    }

    let mut contour_count = 0;
    let mut needs_close = false;
    let mut state = Convexicator::new();

    for rec in PathIter::new(points, vbs, conic_weights) {
        let verb = rec.verb();
        let pts = rec.points();

        // Looking for the last moveTo before non-move verbs start
        if contour_count == 0 {
            if verb == PathVerb::Move {
                state.set_move_pt(pts[0]);
            } else {
                // Starting the actual contour, fall through to c=1 to add the points
                contour_count += 1;
                needs_close = true;
            }
        }
        // Accumulating points into the Convexicator until we hit a close or another move
        if contour_count == 1 {
            if verb == PathVerb::Close || verb == PathVerb::Move {
                if !state.close() {
                    return PathConvexity::Concave;
                }
                needs_close = false;
                contour_count += 1;
            } else {
                // lines add 1 point, cubics add 3, conics and quads add 2
                let count = pts_in_verb_usize(verb);
                debug_assert!(count > 0);
                for &p in &pts[1..=count] {
                    if !state.add_pt(p) {
                        return PathConvexity::Concave;
                    }
                }
            }
        } else if verb != PathVerb::Move {
            // The first contour has closed and anything other than spurious trailing moves means
            // there's multiple contours and the path can't be convex
            return PathConvexity::Concave;
        }
    }

    // If the path isn't explicitly closed do so implicitly
    if needs_close && !state.close() {
        return PathConvexity::Concave;
    }

    let first_dir = state.get_first_direction();
    if first_dir == PathFirstDirection::Unknown && state.reversals() >= 3 {
        return PathConvexity::Concave;
    }
    first_dir.to_convexity()
}

/// `SkPathPriv::TransformConvexity`: what is known about the convexity of `pts` after `matrix`.
// Port of: src/core/SkPathPriv.cpp#L1412-L1441 (chrome/m156)
#[doc(alias = "TransformConvexity")]
#[must_use]
pub fn transform_convexity(
    matrix: &Matrix,
    pts: &[Point],
    convexity: PathConvexity,
) -> PathConvexity {
    let mut convexity = convexity;
    if matrix.is_identity() || pts.is_empty() {
        return convexity;
    }

    // Due to finite/fragile float numerics, we can't assume that a convex path remains
    // convex after a transformation, so mark it as unknown here.
    // However, some transformations are thought to be safe:
    //    axis-aligned values under scale/translate.
    //
    if convexity.is_convex() {
        if !matrix.is_scale_translate() || !is_axis_aligned(pts) {
            // Not safe to still assume we're convex...
            convexity = PathConvexity::Unknown;
        } else {
            let det2x2 = matrix.get(Member::ScaleX) * matrix.get(Member::ScaleY)
                - matrix.get(Member::SkewX) * matrix.get(Member::SkewY);
            if det2x2 < 0.0 {
                convexity = convexity.opposite_convex_direction();
            } else if det2x2 > 0.0 {
                // we keep our direction
            } else {
                // det2x == 0
                convexity = PathConvexity::ConvexDegenerate;
            }
        }
    }
    convexity
}

///////////////////////////////////////////////////////////////////////////////////////////////////
// First direction

// returns cross product of (p1 - p0) and (p2 - p0)
// Port of: src/core/SkPathPriv.cpp#L664-L683 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // SkDoubleToScalar
fn cross_prod(p0: Point, p1: Point, p2: Point) -> scalar {
    let mut cross = Point::cross_product(p1 - p0, p2 - p0);
    // We may get 0 when the above subtracts underflow. We expect this to be
    // very rare and lazily promote to double.
    if cross == 0.0 {
        let p0x = f64::from(p0.x);
        let p0y = f64::from(p0.y);

        let p1x = f64::from(p1.x);
        let p1y = f64::from(p1.y);

        let p2x = f64::from(p2.x);
        let p2y = f64::from(p2.y);

        cross = ((p1x - p0x) * (p2y - p0y) - (p1y - p0y) * (p2x - p0x)) as scalar;
    }
    cross
}

// Returns the first pt with the maximum Y coordinate
// Port of: src/core/SkPathPriv.cpp#L686-L698 (chrome/m156)
fn find_max_y(pts: &[Point]) -> usize {
    debug_assert_ne!(pts, []);
    let mut max = pts[0].y;
    let mut first_index = 0;
    for (i, p) in pts.iter().enumerate().skip(1) {
        let y = p.y;
        if y > max {
            max = y;
            first_index = i;
        }
    }
    first_index
}

// Port of: src/core/SkPathPriv.cpp#L700-L712 (chrome/m156)
fn find_diff_pt(pts: &[Point], index: usize, n: usize, inc: usize) -> usize {
    let mut i = index;
    loop {
        i = (i + inc) % n;
        if i == index {
            // we wrapped around, so abort
            break;
        }
        if pts[index] != pts[i] {
            // found a different point, success!
            break;
        }
    }
    i
}

// Starting at index, and moving forward (incrementing), find the xmin and
// xmax of the contiguous points that have the same Y.
// Port of: src/core/SkPathPriv.cpp#L718-L740 (chrome/m156)
#[allow(clippy::float_cmp)] // exact float comparisons, as in C++
fn find_min_max_x_at_y(pts: &[Point], index: usize, n: usize) -> (usize, usize) {
    let y = pts[index].y;
    let mut min = pts[index].x;
    let mut max = min;
    let mut min_index = index;
    let mut max_index = index;
    for (i, p) in pts.iter().enumerate().take(n).skip(index + 1) {
        if p.y != y {
            break;
        }
        let x = p.x;
        if x < min {
            min = x;
            min_index = i;
        } else if x > max {
            max = x;
            max_index = i;
        }
    }
    (min_index, max_index)
}

// Port of: src/core/SkPathPriv.cpp#L742-L744 (chrome/m156)
fn cross_to_dir(cross: scalar) -> PathFirstDirection {
    if cross > 0.0 {
        PathFirstDirection::CW
    } else {
        PathFirstDirection::CCW
    }
}

// Port of: src/core/SkPathPriv.cpp#L746-L817 (chrome/m156)
struct ContourIter<'a> {
    pts: &'a [Point],
    verbs: &'a [PathVerb],
    curr_pt_count: usize,
    curr_pt: usize,
    curr_verb: usize,
    done: bool,
}

impl<'a> ContourIter<'a> {
    fn new(pts: &'a [Point], vbs: &'a [PathVerb]) -> Self {
        let mut it = Self {
            pts,
            verbs: vbs,
            curr_pt_count: 0,
            curr_pt: 0,
            curr_verb: 0,
            done: false,
        };
        it.next();
        it
    }

    fn done(&self) -> bool {
        self.done
    }

    // if !done() then these may be called
    fn pts(&self) -> &'a [Point] {
        &self.pts[self.curr_pt..self.curr_pt + self.curr_pt_count]
    }

    fn next(&mut self) {
        if self.curr_verb >= self.verbs.len() {
            self.done = true;
        }
        if self.done {
            return;
        }

        // skip pts of prev contour
        self.curr_pt += self.curr_pt_count;

        debug_assert_eq!(self.verbs[self.curr_verb], PathVerb::Move);
        let mut pt_count = 1; // moveTo
        let mut verbs = self.curr_verb + 1;
        while verbs < self.verbs.len() {
            match self.verbs[verbs] {
                PathVerb::Move => break,
                PathVerb::Line => pt_count += 1,
                PathVerb::Conic | PathVerb::Quad => pt_count += 2,
                PathVerb::Cubic => pt_count += 3,
                PathVerb::Close => {}
            }
            verbs += 1;
        }
        self.curr_pt_count = pt_count;
        self.curr_verb = verbs;
    }
}

/// Tries to compute the direction of the outer-most non-degenerate contour.
///
/// We loop through all contours, and keep the computed cross-product of the contour that
/// contained the global y-max. If we just look at the first contour, we may find one that is
/// wound the opposite way (correctly) since it is the interior of a hole (e.g. 'o').
// Port of: src/core/SkPathPriv.cpp#L827-L897 (chrome/m156)
#[doc(alias = "ComputeFirstDirection")]
#[must_use]
#[allow(clippy::cast_precision_loss)] // `cross = minIndex - maxIndex` (int to float)
#[allow(clippy::if_not_else)] // mirrors the C++ control flow
#[allow(clippy::float_cmp)] // exact float comparisons, as in C++
#[allow(clippy::cast_possible_wrap)] // mirrors the C++ casts
pub fn compute_first_direction(raw: &PathRaw<'_>) -> PathFirstDirection {
    let mut iter = ContourIter::new(raw.points(), raw.verbs());

    // initialize with our logical y-min
    let mut ymax = raw.bounds().top;
    let mut ymax_cross: scalar = 0.0;

    while !iter.done() {
        let pts = iter.pts();
        let n = pts.len();
        if n < 3 {
            iter.next();
            continue;
        }

        let mut cross: scalar = 0.0;
        let index = find_max_y(pts);
        if pts[index].y < ymax {
            iter.next();
            continue;
        }

        // If there is more than 1 distinct point at the y-max, we take the
        // x-min and x-max of them and just subtract to compute the dir.
        let mut try_cross_prod = true;
        if pts[(index + 1) % n].y == pts[index].y {
            let (min_index, max_index) = find_min_max_x_at_y(pts, index, n);
            if min_index != max_index {
                debug_assert_eq!(pts[min_index].y, pts[index].y);
                debug_assert_eq!(pts[max_index].y, pts[index].y);
                debug_assert!(pts[min_index].x <= pts[max_index].x);
                // we just subtract the indices, and let that auto-convert to
                // SkScalar, since we just want - or + to signal the direction.
                cross = (min_index as i64 - max_index as i64) as scalar;
                try_cross_prod = false;
            }
        }
        if try_cross_prod {
            // Find a next and prev index to use for the cross-product test,
            // but we try to find pts that form non-zero vectors from pts[index]
            //
            // Its possible that we can't find two non-degenerate vectors, so
            // we have to guard our search (e.g. all the pts could be in the
            // same place).

            // we pass n - 1 instead of -1 so we don't foul up % operator by
            // passing it a negative LH argument.
            let prev = find_diff_pt(pts, index, n, n - 1);
            if prev == index {
                // completely degenerate, skip to next contour
                iter.next();
                continue;
            }
            let next = find_diff_pt(pts, index, n, 1);
            debug_assert_ne!(next, index);
            cross = cross_prod(pts[prev], pts[index], pts[next]);
            // if we get a zero and the points are horizontal, then we look at the spread in
            // x-direction. We really should continue to walk away from the degeneracy until
            // there is a divergence.
            if cross == 0.0 && pts[prev].y == pts[index].y && pts[next].y == pts[index].y {
                // construct the subtract so we get the correct Direction below
                cross = pts[index].x - pts[next].x;
            }
        }

        if cross != 0.0 {
            // record our best guess so far
            ymax = pts[index].y;
            ymax_cross = cross;
        }
        iter.next();
    }

    if ymax_cross != 0.0 {
        cross_to_dir(ymax_cross)
    } else {
        PathFirstDirection::Unknown
    }
}

/// `SkPathPriv::ComputeFirstDirection(const SkPath&)`.
// Port of: src/core/SkPath.cpp#L721-L736 (chrome/m156)
#[must_use]
pub fn compute_first_direction_path(path: &Path) -> PathFirstDirection {
    let convexity = path.get_convexity_or_unknown();
    if convexity.is_convex() {
        // Note, this can return kUnknown. That is valid. If we've determined that the
        // path is convex, then we've already tried to compute its first-direction. If
        // that failed, then kUnknown is the right answer.
        return convexity.to_first_direction();
    }

    // Note, this can compute a 'first' direction, even for non-convex shapes.
    if let Some(raw) = raw(path, ResolveConvexity::No) {
        compute_first_direction(&raw)
    } else {
        PathFirstDirection::Unknown
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////
// Contains

// Port of: src/core/SkPathPriv.cpp#L901-L903 (chrome/m156)
fn poly_eval3(a: f32, b: f32, c: f32, t: f32) -> f32 {
    (a * t + b) * t + c
}

// Port of: src/core/SkPathPriv.cpp#L905-L907 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
fn poly_eval4(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    ((a * t + b) * t + c) * t + d
}

// Port of: src/core/SkPathPriv.cpp#L909-L913 (chrome/m156)
fn between(a: scalar, b: scalar, c: scalar) -> bool {
    (a - b) * (c - b) <= 0.0
}

// Port of: src/core/SkPathPriv.cpp#L915-L922 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
fn eval_cubic_pts(c0: scalar, c1: scalar, c2: scalar, c3: scalar, t: scalar) -> scalar {
    let a = c3 + 3.0 * (c1 - c2) - c0;
    let b = 3.0 * (c2 - c1 - c1 + c0);
    let c = 3.0 * (c1 - c0);
    let d = c0;
    poly_eval4(a, b, c, d, t)
}

// std::min / std::max semantics.
fn std_min(a: scalar, b: scalar) -> scalar {
    if b < a { b } else { a }
}
fn std_max(a: scalar, b: scalar) -> scalar {
    if a < b { b } else { a }
}

// Port of: src/core/SkPathPriv.cpp#L924-L934 (chrome/m156)
fn find_minmax_x(pts: &[Point]) -> (scalar, scalar) {
    let mut min = pts[0].x;
    let mut max = min;
    for p in &pts[1..] {
        min = std_min(min, p.x);
        max = std_max(max, p.x);
    }
    (min, max)
}

// Port of: src/core/SkPathPriv.cpp#L936-L942 (chrome/m156)
#[allow(clippy::float_cmp)] // exact float comparisons, as in C++
fn check_on_curve(x: scalar, y: scalar, start: Point, end: Point) -> bool {
    if start.y == end.y {
        between(start.x, x, end.x) && x != end.x
    } else {
        x == start.x && y == start.y
    }
}

// Port of: src/core/SkPathPriv.cpp#L944-L988 (chrome/m156)
#[allow(clippy::float_cmp)] // exact float comparisons, as in C++
fn winding_mono_cubic(pts: &[Point], x: scalar, y: scalar, on_curve_count: &mut i32) -> i32 {
    let mut y0 = pts[0].y;
    let mut y3 = pts[3].y;

    let mut dir = 1;
    if y0 > y3 {
        std::mem::swap(&mut y0, &mut y3);
        dir = -1;
    }
    if y < y0 || y > y3 {
        return 0;
    }
    if check_on_curve(x, y, pts[0], pts[3]) {
        *on_curve_count += 1;
        return 0;
    }
    if y == y3 {
        return 0;
    }

    // quickreject or quickaccept
    let (min, max) = find_minmax_x(&pts[..4]);
    if x < min {
        return 0;
    }
    if x > max {
        return dir;
    }

    // compute the actual x(t) value
    let Some(t) = CubicClipper::chop_mono_at_y(pts, y) else {
        return 0;
    };
    let xt = eval_cubic_pts(pts[0].x, pts[1].x, pts[2].x, pts[3].x, t);
    if scalar::nearly_equal(xt, x, None) && (x != pts[3].x || y != pts[3].y) {
        // don't test end points; they're start points
        *on_curve_count += 1;
        return 0;
    }
    if xt < x { dir } else { 0 }
}

// Port of: src/core/SkPathPriv.cpp#L990-L998 (chrome/m156)
fn winding_cubic(pts: &[Point], x: scalar, y: scalar, on_curve_count: &mut i32) -> i32 {
    let mut dst = [Point::default(); 10];
    let n = chop_cubic_at_y_extrema(pts, Some(&mut dst));
    let mut w = 0;
    for i in 0..=n {
        w += winding_mono_cubic(&dst[i * 3..], x, y, on_curve_count);
    }
    w
}

// `src` is the x (or y) coordinates of the 3 conic points.
// Port of: src/core/SkPathPriv.cpp#L1000-L1008 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
fn conic_eval_numerator(src: [scalar; 3], w: scalar, t: scalar) -> f64 {
    debug_assert!((0.0..=1.0).contains(&t));
    let src2w = src[1] * w;
    let c = src[0];
    let a = src[2] - 2.0 * src2w + c;
    let b = 2.0 * (src2w - c);
    f64::from(poly_eval3(a, b, c, t))
}

// Port of: src/core/SkPathPriv.cpp#L1011-L1016 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
fn conic_eval_denominator(w: scalar, t: scalar) -> f64 {
    let b = 2.0 * (w - 1.0);
    let c = 1.0;
    let a = -b;
    f64::from(poly_eval3(a, b, c, t))
}

// Port of: src/core/SkPathPriv.cpp#L1018-L1066 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // double result assigned to SkScalar
#[allow(clippy::many_single_char_names)] // names follow the C++
#[allow(clippy::float_cmp)] // exact float comparisons, as in C++
fn winding_mono_conic(conic: &Conic, x: scalar, y: scalar, on_curve_count: &mut i32) -> i32 {
    let pts = &conic.pts;
    let mut y0 = pts[0].y;
    let mut y2 = pts[2].y;

    let mut dir: i32 = 1;
    if y0 > y2 {
        std::mem::swap(&mut y0, &mut y2);
        dir = -1;
    }
    if y < y0 || y > y2 {
        return 0;
    }
    if check_on_curve(x, y, pts[0], pts[2]) {
        *on_curve_count += 1;
        return 0;
    }
    if y == y2 {
        return 0;
    }

    let mut roots = [0.0; 2];
    let mut a = pts[2].y;
    let mut b = pts[1].y * conic.w - y * conic.w + y;
    let mut c = pts[0].y;
    a += c - 2.0 * b; // A = a + c - 2*(b*w - yCept*w + yCept)
    b -= c; // B = b*w - w * yCept + yCept - a
    c -= y;
    let n = find_unit_quad_roots(a, 2.0 * b, c, &mut roots);
    debug_assert!(n <= 1);
    let xt: scalar = if n == 0 {
        // zero roots are returned only when y0 == y
        // Need [0] if dir == 1
        // and  [2] if dir == -1
        #[allow(clippy::cast_sign_loss)] // 1 - dir is 0 or 2
        let idx = (1 - dir) as usize;
        pts[idx].x
    } else {
        let t = roots[0];
        (conic_eval_numerator([pts[0].x, pts[1].x, pts[2].x], conic.w, t)
            / conic_eval_denominator(conic.w, t)) as scalar
    };
    if scalar::nearly_equal(xt, x, None) && (x != pts[2].x || y != pts[2].y) {
        // don't test end points; they're start points
        *on_curve_count += 1;
        return 0;
    }
    if xt < x { dir } else { 0 }
}

// Port of: src/core/SkPathPriv.cpp#L1068-L1078 (chrome/m156)
#[allow(clippy::float_cmp)] // exact float comparisons, as in C++
fn is_mono_quad(y0: scalar, y1: scalar, y2: scalar) -> bool {
    if y0 == y1 {
        return true;
    }
    if y0 < y1 { y1 <= y2 } else { y1 >= y2 }
}

// Port of: src/core/SkPathPriv.cpp#L1080-L1092 (chrome/m156)
fn winding_conic(pts: &[Point], x: scalar, y: scalar, weight: scalar, on_curve: &mut i32) -> i32 {
    let conic = Conic::from_points(pts, weight);
    let mut chopped = [Conic::default(); 2];
    // If the data points are very large, the conic may not be monotonic but may also
    // fail to chop. Then, the chopper does not split the original conic in two.
    let is_mono =
        is_mono_quad(pts[0].y, pts[1].y, pts[2].y) || !conic.chop_at_y_extrema(&mut chopped);
    let mut w = winding_mono_conic(if is_mono { &conic } else { &chopped[0] }, x, y, on_curve);
    if !is_mono {
        w += winding_mono_conic(&chopped[1], x, y, on_curve);
    }
    w
}

// Port of: src/core/SkPathPriv.cpp#L1094-L1147 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
#[allow(clippy::float_cmp)] // exact float comparisons, as in C++
fn winding_mono_quad(pts: &[Point], x: scalar, y: scalar, on_curve_count: &mut i32) -> i32 {
    let mut y0 = pts[0].y;
    let mut y2 = pts[2].y;

    let mut dir: i32 = 1;
    if y0 > y2 {
        std::mem::swap(&mut y0, &mut y2);
        dir = -1;
    }
    if y < y0 || y > y2 {
        return 0;
    }
    if check_on_curve(x, y, pts[0], pts[2]) {
        *on_curve_count += 1;
        return 0;
    }
    if y == y2 {
        return 0;
    }

    let mut roots = [0.0; 2];
    let n = find_unit_quad_roots(
        pts[0].y - 2.0 * pts[1].y + pts[2].y,
        2.0 * (pts[1].y - pts[0].y),
        pts[0].y - y,
        &mut roots,
    );
    debug_assert!(n <= 1);
    let xt: scalar = if n == 0 {
        // zero roots are returned only when y0 == y
        // Need [0] if dir == 1
        // and  [2] if dir == -1
        #[allow(clippy::cast_sign_loss)] // 1 - dir is 0 or 2
        let idx = (1 - dir) as usize;
        pts[idx].x
    } else {
        let t = roots[0];
        let c = pts[0].x;
        let a = pts[2].x - 2.0 * pts[1].x + c;
        let b = 2.0 * (pts[1].x - c);
        poly_eval3(a, b, c, t)
    };
    if scalar::nearly_equal(xt, x, None) && (x != pts[2].x || y != pts[2].y) {
        // don't test end points; they're start points
        *on_curve_count += 1;
        return 0;
    }
    if xt < x { dir } else { 0 }
}

// Port of: src/core/SkPathPriv.cpp#L1149-L1163 (chrome/m156)
fn winding_quad(pts: &[Point], x: scalar, y: scalar, on_curve_count: &mut i32) -> i32 {
    let mut span_storage = [Point::default(); 5];
    let mut n = 0;

    let span: &[Point] = if is_mono_quad(pts[0].y, pts[1].y, pts[2].y) {
        pts
    } else {
        n = chop_quad_at_y_extrema(pts, &mut span_storage);
        &span_storage
    };
    let mut w = winding_mono_quad(span, x, y, on_curve_count);
    if n > 0 {
        w += winding_mono_quad(&span[2..], x, y, on_curve_count);
    }
    w
}

// Port of: src/core/SkPathPriv.cpp#L1165-L1203 (chrome/m156)
#[allow(clippy::float_cmp)] // exact float comparisons, as in C++
fn winding_line(pts: &[Point], x: scalar, y: scalar, on_curve_count: &mut i32) -> i32 {
    let x0 = pts[0].x;
    let mut y0 = pts[0].y;
    let x1 = pts[1].x;
    let mut y1 = pts[1].y;

    let dy = y1 - y0;

    let mut dir = 1;
    if y0 > y1 {
        std::mem::swap(&mut y0, &mut y1);
        dir = -1;
    }
    if y < y0 || y > y1 {
        return 0;
    }
    if check_on_curve(x, y, pts[0], pts[1]) {
        *on_curve_count += 1;
        return 0;
    }
    if y == y1 {
        return 0;
    }
    let cross = (x1 - x0) * (y - pts[0].y) - dy * (x - x0);

    if cross == 0.0 {
        // zero cross means the point is on the line, and since the case where
        // y of the query point is at the end point is handled above, we can be
        // sure that we're on the line (excluding the end point) here
        if x != x1 || y != pts[1].y {
            *on_curve_count += 1;
        }
        dir = 0;
    } else if scalar_sign_as_int(cross) == dir {
        dir = 0;
    }
    dir
}

// Port of: src/core/SkPathPriv.cpp#L1205-L1231 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
fn tangent_cubic(pts: &[Point], x: scalar, y: scalar, tangents: &mut Vec<Vector>) {
    if !between(pts[0].y, y, pts[1].y)
        && !between(pts[1].y, y, pts[2].y)
        && !between(pts[2].y, y, pts[3].y)
    {
        return;
    }
    if !between(pts[0].x, x, pts[1].x)
        && !between(pts[1].x, x, pts[2].x)
        && !between(pts[2].x, x, pts[3].x)
    {
        return;
    }
    let mut dst = [Point::default(); 10];
    let n = chop_cubic_at_y_extrema(pts, Some(&mut dst));
    for i in 0..=n {
        let c = &dst[i * 3..i * 3 + 4];
        let Some(t) = CubicClipper::chop_mono_at_y(c, y) else {
            continue;
        };
        let xt = eval_cubic_pts(c[0].x, c[1].x, c[2].x, c[3].x, t);
        if !scalar::nearly_equal(x, xt, None) {
            continue;
        }
        let mut tangent = Vector::default();
        eval_cubic_at(c, t, None, Some(&mut tangent), None);
        tangents.push(tangent);
    }
}

// Port of: src/core/SkPathPriv.cpp#L1233-L1258 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // double result assigned to SkScalar
#[allow(clippy::many_single_char_names)] // names follow the C++
fn tangent_conic(pts: &[Point], x: scalar, y: scalar, w: scalar, tangents: &mut Vec<Vector>) {
    if !between(pts[0].y, y, pts[1].y) && !between(pts[1].y, y, pts[2].y) {
        return;
    }
    if !between(pts[0].x, x, pts[1].x) && !between(pts[1].x, x, pts[2].x) {
        return;
    }
    let mut roots = [0.0; 2];
    let mut a = pts[2].y;
    let mut b = pts[1].y * w - y * w + y;
    let mut c = pts[0].y;
    a += c - 2.0 * b; // A = a + c - 2*(b*w - yCept*w + yCept)
    b -= c; // B = b*w - w * yCept + yCept - a
    c -= y;
    let n = find_unit_quad_roots(a, 2.0 * b, c, &mut roots);
    for &t in &roots[..n] {
        let xt = (conic_eval_numerator([pts[0].x, pts[1].x, pts[2].x], w, t)
            / conic_eval_denominator(w, t)) as scalar;
        if !scalar::nearly_equal(x, xt, None) {
            continue;
        }
        let conic = Conic::from_points(pts, w);
        tangents.push(conic.eval_tangent_at(t));
    }
}

// Port of: src/core/SkPathPriv.cpp#L1260-L1284 (chrome/m156)
#[allow(clippy::many_single_char_names)] // names follow the C++
fn tangent_quad(pts: &[Point], x: scalar, y: scalar, tangents: &mut Vec<Vector>) {
    if !between(pts[0].y, y, pts[1].y) && !between(pts[1].y, y, pts[2].y) {
        return;
    }
    if !between(pts[0].x, x, pts[1].x) && !between(pts[1].x, x, pts[2].x) {
        return;
    }
    let mut roots = [0.0; 2];
    let n = find_unit_quad_roots(
        pts[0].y - 2.0 * pts[1].y + pts[2].y,
        2.0 * (pts[1].y - pts[0].y),
        pts[0].y - y,
        &mut roots,
    );
    for &t in &roots[..n] {
        let c = pts[0].x;
        let a = pts[2].x - 2.0 * pts[1].x + c;
        let b = 2.0 * (pts[1].x - c);
        let xt = poly_eval3(a, b, c, t);
        if !scalar::nearly_equal(x, xt, None) {
            continue;
        }
        tangents.push(eval_quad_tangent_at(pts, t));
    }
}

// Port of: src/core/SkPathPriv.cpp#L1286-L1306 (chrome/m156)
fn tangent_line(pts: &[Point], x: scalar, y: scalar, tangents: &mut Vec<Vector>) {
    let y0 = pts[0].y;
    let y1 = pts[1].y;
    if !between(y0, y, y1) {
        return;
    }
    let x0 = pts[0].x;
    let x1 = pts[1].x;
    if !between(x0, x, x1) {
        return;
    }
    let dx = x1 - x0;
    let dy = y1 - y0;
    if !scalar::nearly_equal((x - x0) * dy, dx * (y - y0), None) {
        return;
    }
    tangents.push(Vector::new(dx, dy));
}

// Port of: src/core/SkPathPriv.cpp#L1308-L1310 (chrome/m156)
fn contains_inclusive(r: &Rect, p: Point) -> bool {
    r.left <= p.x && p.x <= r.right && r.top <= p.y && p.y <= r.bottom
}

/// True if `p` is inside the path described by `raw` (taking its fill type into account).
///
/// Does not use the convexity in the raw, so it need not be resolved.
// Port of: src/core/SkPathPriv.cpp#L1312-L1395 (chrome/m156)
#[doc(alias = "Contains")]
#[must_use]
pub fn contains(raw: &PathRaw<'_>, p: Point) -> bool {
    let ft = raw.fill_type();
    let is_inverse = ft.is_inverse();
    if raw.is_empty() {
        return is_inverse;
    }

    if !contains_inclusive(&raw.bounds(), p) {
        return is_inverse;
    }

    let mut w = 0;
    let mut on_curve_count = 0;

    let mut iter = PathEdgeIter::new(raw);
    while let Some(rec) = iter.next() {
        match rec.edge {
            Edge::Line => w += winding_line(&rec.pts, p.x, p.y, &mut on_curve_count),
            Edge::Quad => w += winding_quad(&rec.pts, p.x, p.y, &mut on_curve_count),
            Edge::Conic => {
                w += winding_conic(&rec.pts, p.x, p.y, iter.conic_weight(), &mut on_curve_count);
            }
            Edge::Cubic => w += winding_cubic(&rec.pts, p.x, p.y, &mut on_curve_count),
        }
    }
    let even_odd_fill = ft == PathFillType::EvenOdd || ft == PathFillType::InverseEvenOdd;
    if even_odd_fill {
        w &= 1;
    }
    if w != 0 {
        return !is_inverse;
    }
    if on_curve_count <= 1 {
        return (on_curve_count != 0) ^ is_inverse;
    }
    if (on_curve_count & 1) != 0 || even_odd_fill {
        return ((on_curve_count & 1) != 0) ^ is_inverse;
    }
    // If the point touches an even number of curves, and the fill is winding, check for
    // coincidence. Count coincidence as places where the on curve points have identical tangents.
    let mut tangents: Vec<Vector> = Vec::new();
    let mut iter = PathEdgeIter::new(raw);
    while let Some(rec) = iter.next() {
        let old_count = tangents.len();
        match rec.edge {
            Edge::Line => tangent_line(&rec.pts, p.x, p.y, &mut tangents),
            Edge::Quad => tangent_quad(&rec.pts, p.x, p.y, &mut tangents),
            Edge::Conic => tangent_conic(&rec.pts, p.x, p.y, iter.conic_weight(), &mut tangents),
            Edge::Cubic => tangent_cubic(&rec.pts, p.x, p.y, &mut tangents),
        }
        if tangents.len() > old_count {
            let last = tangents.len() - 1;
            let tangent = tangents[last];
            if point_priv::length_sqd(tangent).nearly_zero(None) {
                tangents.remove(last);
            } else {
                for index in 0..last {
                    let test = tangents[index];
                    if test.cross(tangent).nearly_zero(None)
                        && scalar_sign_as_int(tangent.x * test.x) <= 0
                        && scalar_sign_as_int(tangent.y * test.y) <= 0
                    {
                        tangents.remove(last);
                        // SkTDArray::removeShuffle: move the last element into `index`.
                        tangents.swap_remove(index);
                        break;
                    }
                }
            }
        }
    }
    (!tangents.is_empty()) ^ is_inverse
}

///////////////////////////////////////////////////////////////////////////////////////////////////
// Tight bounds

// Port of: src/core/SkPathPriv.cpp#L1445-L1455 (chrome/m156)
fn compute_quad_extremas(src: &[Point], extremas: &mut [Point; 5]) -> usize {
    let mut ts = [0.0; 2];
    let mut t1 = [0.0; 1];
    let mut n = find_quad_extrema(src[0].x, src[1].x, src[2].x, &mut t1);
    ts[0] = t1[0];
    let mut t2 = [0.0; 1];
    let n2 = find_quad_extrema(src[0].y, src[1].y, src[2].y, &mut t2);
    if n2 > 0 {
        ts[n] = t2[0];
    }
    n += n2;
    debug_assert!(n <= 2);
    for i in 0..n {
        extremas[i] = eval_quad_at(src, ts[i]);
    }
    extremas[n] = src[2];
    n + 1
}

// Port of: src/core/SkPathPriv.cpp#L1457-L1468 (chrome/m156)
fn compute_conic_extremas(src: &[Point], w: scalar, extremas: &mut [Point; 5]) -> usize {
    let conic = Conic::new(src[0], src[1], src[2], w);
    let mut ts = [0.0; 2];
    let mut n = 0;
    if let Some(t) = conic.find_x_extrema() {
        ts[n] = t;
        n += 1;
    }
    if let Some(t) = conic.find_y_extrema() {
        ts[n] = t;
        n += 1;
    }
    debug_assert!(n <= 2);
    for i in 0..n {
        extremas[i] = conic.eval_at(ts[i]);
    }
    extremas[n] = src[2];
    n + 1
}

// Port of: src/core/SkPathPriv.cpp#L1470-L1480 (chrome/m156)
fn compute_cubic_extremas(src: &[Point], extremas: &mut [Point; 5]) -> usize {
    let mut ts = [0.0; 4];
    let mut tx = [0.0; 2];
    let nx = find_cubic_extrema(src[0].x, src[1].x, src[2].x, src[3].x, &mut tx);
    ts[..nx].copy_from_slice(&tx[..nx]);
    let mut ty = [0.0; 2];
    let ny = find_cubic_extrema(src[0].y, src[1].y, src[2].y, src[3].y, &mut ty);
    ts[nx..nx + ny].copy_from_slice(&ty[..ny]);
    let n = nx + ny;
    debug_assert!(n <= 4);
    for i in 0..n {
        let mut p = Point::default();
        eval_cubic_at(src, ts[i], Some(&mut p), None, None);
        extremas[i] = p;
    }
    extremas[n] = src[3];
    n + 1
}

/// The bounds of the curves (not just the control points). Returns empty if there are no verbs.
// Port of: src/core/SkPathPriv.cpp#L1482-L1528 (chrome/m156)
#[doc(alias = "ComputeTightBounds")]
#[must_use]
pub fn compute_tight_bounds(
    points: &[Point],
    verbs: &[PathVerb],
    conic_weights: &[scalar],
) -> Rect {
    if verbs.is_empty() {
        return Rect::new_empty();
    }

    // initial with the first MoveTo, so we don't have to check inside the switch
    let mut l = points[0].x;
    let mut t = points[0].y;
    let mut r = points[0].x;
    let mut b = points[0].y;

    for (verb, pts, w) in iterate_raw(verbs, points, conic_weights) {
        let mut extremas = [Point::default(); 5]; // worst-case curve type (cubic) extremas + 1
        let count = match verb {
            PathVerb::Move => {
                extremas[0] = pts[0];
                1
            }
            PathVerb::Line => {
                extremas[0] = pts[1];
                1
            }
            PathVerb::Quad => compute_quad_extremas(pts, &mut extremas),
            PathVerb::Conic => compute_conic_extremas(pts, w.unwrap_or(1.0), &mut extremas),
            PathVerb::Cubic => compute_cubic_extremas(pts, &mut extremas),
            PathVerb::Close => 0,
        };
        for p in &extremas[..count] {
            l = p.x.min(l); // std::fminf
            t = p.y.min(t);
            r = p.x.max(r); // std::fmaxf
            b = p.y.max(b);
        }
    }
    Rect::new(l, t, r, b)
}

///////////////////////////////////////////////////////////////////////////////////////////////////
// Transforms of known shapes

/// For a known shape (oval or rrect) under `matrix`: the new winding direction and start index.
// Port of: src/core/SkPathPriv.cpp#L1550-L1614 (chrome/m156)
#[doc(alias = "TransformDirAndStart")]
#[must_use]
#[allow(clippy::if_not_else)] // mirrors the C++ control flow
#[allow(clippy::bool_to_int_with_if)] // mirrors the C++ ternaries
pub fn transform_dir_and_start(
    matrix: &Matrix,
    is_rrect: bool,
    dir: PathDirection,
    start: u32,
) -> (PathDirection, u32) {
    if matrix.is_identity() {
        return (dir, start);
    }
    let mut in_start = start;
    let mut is_ccw = dir == PathDirection::CCW;

    let mut rm = 0;
    if is_rrect {
        // Degenerate rrect indices to oval indices and remember the remainder.
        // Ovals have one index per side whereas rrects have two.
        rm = in_start & 0b1;
        in_start /= 2;
    }
    // Is the antidiagonal non-zero (otherwise the diagonal is zero)
    let anti_diag: u32;
    // Is the non-zero value in the top row (either kMScaleX or kMSkewX) negative
    let top_neg: u32;
    // Are the two non-zero diagonal or antidiagonal values the same sign.
    let same_sign: u32;
    if matrix.get(Member::ScaleX) != 0.0 {
        anti_diag = 0b00;
        if matrix.get(Member::ScaleX) > 0.0 {
            top_neg = 0b00;
            same_sign = if matrix.get(Member::ScaleY) > 0.0 {
                0b01
            } else {
                0b00
            };
        } else {
            top_neg = 0b10;
            same_sign = if matrix.get(Member::ScaleY) > 0.0 {
                0b00
            } else {
                0b01
            };
        }
    } else {
        anti_diag = 0b01;
        if matrix.get(Member::SkewX) > 0.0 {
            top_neg = 0b00;
            same_sign = if matrix.get(Member::SkewY) > 0.0 {
                0b01
            } else {
                0b00
            };
        } else {
            top_neg = 0b10;
            same_sign = if matrix.get(Member::SkewY) > 0.0 {
                0b00
            } else {
                0b01
            };
        }
    }
    let mut start;
    if same_sign != anti_diag {
        // This is a rotation (and maybe scale). The direction is unchanged.
        // Trust me on the start computation (or draw yourself some pictures)
        start = (in_start + 4 - (top_neg | anti_diag)) % 4;
        debug_assert!(start < 4);
        if is_rrect {
            start = 2 * start + rm;
        }
    } else {
        // This is a mirror (and maybe scale). The direction is reversed.
        is_ccw = !is_ccw;
        // Trust me on the start computation (or draw yourself some pictures)
        start = (6 + (top_neg | anti_diag) - in_start) % 4;
        debug_assert!(start < 4);
        if is_rrect {
            start = 2 * start + if rm != 0 { 0 } else { 1 };
        }
    }

    (
        if is_ccw {
            PathDirection::CCW
        } else {
            PathDirection::CW
        },
        start,
    )
}

/// Recovers the round rect from a contour built from one (the radii are not stored).
// Port of: src/core/SkPathPriv.cpp#L1616-L1673 (chrome/m156)
#[doc(alias = "DeduceRRectFromContour")]
#[must_use]
#[allow(clippy::float_cmp)] // exact float comparisons, as in C++
pub fn deduce_rrect_from_contour(bounds: &Rect, pts: &[Point], vbs: &[PathVerb]) -> RRect {
    if bounds.is_empty() {
        return RRect::new_empty();
    }
    debug_assert_ne!(vbs, []);
    debug_assert_eq!(vbs[0], PathVerb::Move);

    let mut radii = [Vector::new(0.0, 0.0); 4];

    let mut pt_index: usize = 0;
    for &verb in vbs {
        match verb {
            PathVerb::Move => {
                debug_assert_eq!(pt_index, 0); // we only expect 1 move
                pt_index += 1;
            }
            PathVerb::Line => {
                // we only expect horizontal or vertical lines
                debug_assert!({
                    let delta = pts[pt_index] - pts[pt_index - 1];
                    delta.x == 0.0 || delta.y == 0.0
                });
                pt_index += 1;
            }
            PathVerb::Quad | PathVerb::Cubic => debug_assert!(false),
            PathVerb::Conic => {
                let v1_0 = pts[pt_index] - pts[pt_index - 1];
                let v2_1 = pts[pt_index + 1] - pts[pt_index];
                let dxdy = if v1_0.x != 0.0 {
                    debug_assert!(v2_1.x == 0.0 && v1_0.y == 0.0);
                    Vector::new(scalar_abs(v1_0.x), scalar_abs(v2_1.y))
                } else if v1_0.y == 0.0 {
                    debug_assert!(v2_1.x == 0.0 || v2_1.y == 0.0);
                    Vector::new(scalar_abs(v2_1.x), scalar_abs(v2_1.y))
                } else {
                    debug_assert_eq!(v2_1.y, 0.0);
                    Vector::new(scalar_abs(v2_1.x), scalar_abs(v1_0.y))
                };
                let corner = if pts[pt_index].x == bounds.left {
                    if pts[pt_index].y == bounds.top {
                        Corner::UpperLeft
                    } else {
                        Corner::LowerLeft
                    }
                } else if pts[pt_index].y == bounds.top {
                    Corner::UpperRight
                } else {
                    Corner::LowerRight
                };
                debug_assert!(radii[corner as usize].x == 0.0 && radii[corner as usize].y == 0.0);
                radii[corner as usize] = dxdy;
                pt_index += 2;
            }
            PathVerb::Close => {}
        }
    }
    let mut rrect = RRect::default();
    rrect.set_rect_radii(bounds, &radii);
    rrect
}

/// The info for a rect that has a move followed by 3 or 4 lines and a close. If `is_simple_fill`
/// is true, an unclosed rect is also accepted as long as it starts and ends at the same corner.
/// This does not permit degenerate line or point rectangles.
// Port of: src/core/SkPath.cpp#L939-L1035 (chrome/m156)
#[doc(alias = "IsSimpleRect")]
#[must_use]
#[allow(clippy::needless_late_init)] // mirrors the C++ declaration
#[allow(clippy::float_cmp)] // exact float comparisons, as in C++
#[allow(clippy::bool_to_int_with_if)] // mirrors the C++ ternaries
pub fn is_simple_rect(path: &Path, is_simple_fill: bool) -> Option<PathRectInfo> {
    if path.segment_masks() != PathSegmentMask::LINE {
        return None;
    }
    let mut rect_pts = [Point::default(); 5];
    let mut rect_pt_cnt = 0;
    let mut needs_close = !is_simple_fill;
    for (v, verb_pts, _) in iterate(path) {
        match v {
            PathVerb::Move => {
                if rect_pt_cnt != 0 {
                    return None;
                }
                rect_pts[0] = verb_pts[0];
                rect_pt_cnt += 1;
            }
            PathVerb::Line => {
                if rect_pt_cnt == 5 {
                    return None;
                }
                rect_pts[rect_pt_cnt] = verb_pts[1];
                rect_pt_cnt += 1;
            }
            PathVerb::Close => {
                if rect_pt_cnt == 4 {
                    rect_pts[4] = rect_pts[0];
                    rect_pt_cnt = 5;
                }
                needs_close = false;
            }
            PathVerb::Quad | PathVerb::Conic | PathVerb::Cubic => return None,
        }
    }
    if needs_close {
        return None;
    }
    if rect_pt_cnt < 5 {
        return None;
    }
    if rect_pts[0] != rect_pts[4] {
        return None;
    }
    // Check for two cases of rectangles: pts 0 and 3 form a vertical edge or a horizontal edge (
    // and pts 1 and 2 the opposite vertical or horizontal edge).
    let vec03_is_vertical;
    if rect_pts[0].x == rect_pts[3].x
        && rect_pts[1].x == rect_pts[2].x
        && rect_pts[0].y == rect_pts[1].y
        && rect_pts[3].y == rect_pts[2].y
    {
        // Make sure it has non-zero width and height
        if rect_pts[0].x == rect_pts[1].x || rect_pts[0].y == rect_pts[3].y {
            return None;
        }
        vec03_is_vertical = true;
    } else if rect_pts[0].y == rect_pts[3].y
        && rect_pts[1].y == rect_pts[2].y
        && rect_pts[0].x == rect_pts[1].x
        && rect_pts[3].x == rect_pts[2].x
    {
        // Make sure it has non-zero width and height
        if rect_pts[0].y == rect_pts[1].y || rect_pts[0].x == rect_pts[3].x {
            return None;
        }
        vec03_is_vertical = false;
    } else {
        return None;
    }

    // Set sortFlags so that it has the low bit set if pt index 0 is on right edge and second bit
    // set if it is on the bottom edge.
    let sort_flags = (if rect_pts[0].x < rect_pts[2].x {
        0b00
    } else {
        0b01
    }) | (if rect_pts[0].y < rect_pts[2].y {
        0b00
    } else {
        0b10
    });
    let (cw, ccw) = (PathDirection::CW, PathDirection::CCW);
    let info = match sort_flags {
        0b00 => PathRectInfo {
            rect: Rect::new(rect_pts[0].x, rect_pts[0].y, rect_pts[2].x, rect_pts[2].y),
            direction: if vec03_is_vertical { cw } else { ccw },
            start_index: 0,
        },
        0b01 => PathRectInfo {
            rect: Rect::new(rect_pts[2].x, rect_pts[0].y, rect_pts[0].x, rect_pts[2].y),
            direction: if vec03_is_vertical { ccw } else { cw },
            start_index: 1,
        },
        0b10 => PathRectInfo {
            rect: Rect::new(rect_pts[0].x, rect_pts[2].y, rect_pts[2].x, rect_pts[0].y),
            direction: if vec03_is_vertical { ccw } else { cw },
            start_index: 3,
        },
        _ => PathRectInfo {
            rect: Rect::new(rect_pts[2].x, rect_pts[2].y, rect_pts[0].x, rect_pts[0].y),
            direction: if vec03_is_vertical { cw } else { ccw },
            start_index: 2,
        },
    };
    Some(info)
}

///////////////////////////////////////////////////////////////////////////////////////////////////
// Perspective clipping

// Port of: src/core/SkPath.cpp#L762-L832 (chrome/m156)
#[derive(Copy, Clone, Debug)]
struct HalfPlane {
    a: scalar,
    b: scalar,
    c: scalar,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum HalfPlaneResult {
    AllNegative,
    AllPositive,
    Mixed,
}

impl HalfPlane {
    fn eval(&self, x: scalar, y: scalar) -> scalar {
        self.a * x + self.b * y + self.c
    }

    #[allow(clippy::cast_possible_truncation)] // double -> float, as in C++
    fn normalize(&mut self) -> bool {
        let mut a = f64::from(self.a);
        let mut b = f64::from(self.b);
        let mut c = f64::from(self.c);
        let dmag = (a * a + b * b).sqrt();
        // length of initial plane normal is zero
        if dmag == 0.0 {
            self.a = 0.0;
            self.b = 0.0;
            self.c = SCALAR_1;
            return true;
        }
        let dscale = ieee_double_divide(1.0, dmag);
        a *= dscale;
        b *= dscale;
        c *= dscale;
        // check if we're not finite, or normal is zero-length
        if !(is_finite(a) && is_finite(b) && is_finite(c)) || (a == 0.0 && b == 0.0) {
            self.a = 0.0;
            self.b = 0.0;
            self.c = SCALAR_1;
            return false;
        }
        self.a = a as f32;
        self.b = b as f32;
        self.c = c as f32;
        true
    }

    fn test(&self, bounds: &Rect) -> HalfPlaneResult {
        // check whether the diagonal aligned with the normal crosses the plane
        let (min_x, max_x) = if self.a >= 0.0 {
            (bounds.left, bounds.right)
        } else {
            (bounds.right, bounds.left)
        };
        let (min_y, max_y) = if self.b >= 0.0 {
            (bounds.top, bounds.bottom)
        } else {
            (bounds.bottom, bounds.top)
        };
        let test = self.eval(min_x, min_y);
        let sign = test * self.eval(max_x, max_y);
        if sign > 0.0 {
            // the path is either all on one side of the half-plane or the other
            if test < 0.0 {
                return HalfPlaneResult::AllNegative;
            }
            return HalfPlaneResult::AllPositive;
        }
        HalfPlaneResult::Mixed
    }
}

// assumes plane is pre-normalized
// Port of: src/core/SkPath.cpp#L835-L906 (chrome/m156)
fn clip(path: &Path, plane: &HalfPlane) -> Option<Path> {
    let mut mx = Matrix::default();
    let p0 = Point::new(-plane.a * plane.c, -plane.b * plane.c);
    mx.set_all(
        plane.b, plane.a, p0.x, //
        -plane.a, plane.b, p0.y, //
        0.0, 0.0, 1.0,
    );
    let inv = mx.invert()?;

    let rotated = path.try_make_transform(&inv)?;
    let Some(raw) = raw(&rotated, ResolveConvexity::No) else {
        debug_assert!(false); // if rotated was valid, so should the raw
        return None;
    };

    let big = SCALAR_MAX;
    let clip = Rect::new(-big, 0.0, big, big);

    let mut result = PathBuilder::new();
    let mut prev = Point::new(0.0, 0.0);

    EdgeClipper::clip_path(&raw, &clip, false, |clipper, mut new_ctr| {
        let mut add_line_to = false;
        let mut pts = [Point::default(); 4];
        while let Some(verb) = clipper.next(&mut pts) {
            if new_ctr {
                result.move_to(pts[0]);
                prev = pts[0];
                new_ctr = false;
            }

            if add_line_to || pts[0] != prev {
                result.line_to(pts[0]);
            }

            match verb {
                PathVerb::Line => {
                    result.line_to(pts[1]);
                    prev = pts[1];
                }
                PathVerb::Quad => {
                    result.quad_to(pts[1], pts[2]);
                    prev = pts[2];
                }
                PathVerb::Cubic => {
                    result.cubic_to(pts[1], pts[2], pts[3]);
                    prev = pts[3];
                }
                _ => {}
            }
            add_line_to = true;
        }
    });

    result.set_fill_type(path.fill_type());
    let result = result.detach_and_transform(&mx);
    if !result.is_finite() {
        return None;
    }
    Some(result)
}

/// If needed (to not blow up under a perspective matrix), clips the path and returns the result
/// (which might be empty if the path was completely clipped out). Returns `None` if no clipping
/// is needed.
// Port of: src/core/SkPath.cpp#L909-L937 (chrome/m156)
#[doc(alias = "PerspectiveClip")]
#[must_use]
pub fn perspective_clip(path: &Path, matrix: &Matrix) -> Option<Path> {
    if !matrix.has_perspective() {
        return None;
    }

    let mut plane = HalfPlane {
        a: matrix.get(Member::Persp0),
        b: matrix.get(Member::Persp1),
        c: matrix.get(Member::Persp2) - W0_PLANE_DISTANCE,
    };
    if plane.normalize() {
        match plane.test(path.bounds()) {
            HalfPlaneResult::AllPositive => return None,
            HalfPlaneResult::Mixed => {
                // clipped out (or failed) => empty
                return Some(clip(path, &plane).unwrap_or_default());
            }
            HalfPlaneResult::AllNegative => {} // handled outside of the switch
        }
    }
    // clipped out (or failed)
    Some(Path::new())
}

///////////////////////////////////////////////////////////////////////////////////////////////////
// SkPathEdgeIter

/// The kind of segment returned by [`PathEdgeIter`].
// Port of: src/core/SkPathPriv.h#L478-L484 (chrome/m156)
#[doc(alias = "SkPathEdgeIter::Edge")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Edge {
    Line = PathVerb::Line as u8,
    Quad = PathVerb::Quad as u8,
    Conic = PathVerb::Conic as u8,
    Cubic = PathVerb::Cubic as u8,
}

impl Edge {
    /// `SkPathEdgeIter::EdgeToVerb`.
    #[must_use]
    pub fn to_verb(self) -> PathVerb {
        match self {
            Self::Line => PathVerb::Line,
            Self::Quad => PathVerb::Quad,
            Self::Conic => PathVerb::Conic,
            Self::Cubic => PathVerb::Cubic,
        }
    }
}

/// One segment returned by [`PathEdgeIter`].
// Port of: src/core/SkPathPriv.h#L491-L499 (chrome/m156)
#[doc(alias = "SkPathEdgeIter::Result")]
#[derive(Copy, Clone, Debug)]
pub struct EdgeResult {
    /// The segment's points (2 for a line, 3 for quads/conics, 4 for cubics).
    pub pts: PtsBuf,
    pub edge: Edge,
    pub is_new_contour: bool,
}

/// A small inline buffer of up to 4 points that derefs to a slice.
#[derive(Copy, Clone, Debug)]
pub struct PtsBuf {
    pts: [Point; 4],
    len: usize,
}

impl PtsBuf {
    fn new(src: &[Point]) -> Self {
        let mut pts = [Point::default(); 4];
        pts[..src.len()].copy_from_slice(src);
        Self {
            pts,
            len: src.len(),
        }
    }
}

impl std::ops::Deref for PtsBuf {
    type Target = [Point];
    fn deref(&self) -> &[Point] {
        &self.pts[..self.len]
    }
}

/// Lightweight variant of `Path::Iter` that only returns segments (e.g. lines/conics). Does not
/// return moves or closes. Always "auto-closes" each contour. Roughly the same as
/// `Iter::new(path, true)`, but does not return moves or closes.
// Port of: src/core/SkPathPriv.h#L460-L537, src/core/SkPath.cpp#L1039-L1055 (chrome/m156)
#[doc(alias = "SkPathEdgeIter")]
#[derive(Clone, Debug)]
pub struct PathEdgeIter<'a> {
    verbs: &'a [PathVerb],
    pts: &'a [Point],
    conics: &'a [scalar],
    v: usize,
    p: usize,
    move_to: usize,
    // C++ keeps a pointer that begins one behind the first weight; this is the count of conics
    // seen so far.
    conics_seen: usize,
    needs_close_line: bool,
    next_is_new_contour: bool,
    is_conic: bool,
}

impl<'a> PathEdgeIter<'a> {
    /// Iterates the edges of `raw`.
    #[must_use]
    pub fn new(raw: &PathRaw<'a>) -> Self {
        Self {
            verbs: raw.verbs,
            pts: raw.points,
            conics: raw.conics,
            v: 0,
            p: 0,
            move_to: 0,
            conics_seen: 0,
            needs_close_line: false,
            next_is_new_contour: false,
            is_conic: false,
        }
    }

    /// Iterates the edges of `path` (nothing if it is not finite).
    #[must_use]
    pub fn from_path(path: &'a Path) -> Self {
        Self::new(&raw(path, ResolveConvexity::No).unwrap_or(PathRaw::empty(PathFillType::DEFAULT)))
    }

    /// The weight of the last conic returned.
    #[doc(alias = "conicWeight")]
    #[must_use]
    pub fn conic_weight(&self) -> scalar {
        debug_assert!(self.is_conic);
        self.conics[self.conics_seen - 1]
    }

    fn close_line(&mut self) -> EdgeResult {
        let scratch = [self.pts[self.p - 1], self.pts[self.move_to]];
        self.needs_close_line = false;
        self.next_is_new_contour = true;
        EdgeResult {
            pts: PtsBuf::new(&scratch),
            edge: Edge::Line,
            is_new_contour: false,
        }
    }

    /// The next edge, or `None` when the path is done.
    // Port of: src/core/SkPathPriv.h#L501-L536 (chrome/m156)
    #[allow(clippy::should_implement_trait)] // mirrors the C++ API; conic_weight() needs `self`
    #[allow(clippy::manual_midpoint)] // mirrors the C++ `(a + b) / 2` arithmetic
    #[allow(clippy::cast_possible_wrap)] // mirrors the C++ casts
    #[allow(clippy::cast_possible_truncation)] // mirrors the C++ casts
    pub fn next(&mut self) -> Option<EdgeResult> {
        loop {
            debug_assert!(self.v <= self.verbs.len());
            if self.v == self.verbs.len() {
                return if self.needs_close_line {
                    Some(self.close_line())
                } else {
                    None
                };
            }

            self.is_conic = false;

            let verb = self.verbs[self.v];
            self.v += 1;
            match verb {
                PathVerb::Move => {
                    if self.needs_close_line {
                        let res = self.close_line();
                        self.move_to = self.p;
                        self.p += 1;
                        return Some(res);
                    }
                    self.move_to = self.p;
                    self.p += 1;
                    self.next_is_new_contour = true;
                }
                PathVerb::Close => {
                    if self.needs_close_line {
                        return Some(self.close_line());
                    }
                }
                _ => {
                    let v = verb as usize;
                    // Actual edge.
                    let pts_count = (v + 2) / 2;
                    let cws_count = (v & (v - 1)) / 2;
                    debug_assert_eq!(pts_count as i32, pts_in_iter(verb) - 1);

                    self.needs_close_line = true;
                    self.p += pts_count;
                    self.conics_seen += cws_count;

                    self.is_conic = verb == PathVerb::Conic;
                    debug_assert_eq!(self.is_conic, (cws_count > 0));

                    let is_new_contour = self.next_is_new_contour;
                    self.next_is_new_contour = false;
                    let edge = match verb {
                        PathVerb::Line => Edge::Line,
                        PathVerb::Quad => Edge::Quad,
                        PathVerb::Conic => Edge::Conic,
                        _ => Edge::Cubic,
                    };
                    let start = self.p - (pts_count + 1);
                    return Some(EdgeResult {
                        pts: PtsBuf::new(&self.pts[start..self.p]),
                        edge,
                        is_new_contour,
                    });
                }
            }
        }
    }
}
