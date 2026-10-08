// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkOpSegment.h, src/pathops/SkOpSegment.cpp
// (the SkOpSegment methods used by the op graph).

//! A curve (line, quad, conic or cubic) of the op graph, split into spans (`SkOpSegment`).
//!
//! A segment owns a list of spans from its head to its tail. Those spans and the points they
//! use live in [`OpState`]; the segment keeps the ids of its first and last span, its curve,
//! and its winding bookkeeping. Methods that Skia writes on `SkOpSegment` are `OpState` methods
//! taking a [`SegId`].

use skia_rust_core::path::Verb;
use skia_rust_core::point::Point;

use crate::op_angle::IncludeType;
use crate::op_curve::{
    DCurveBuf, curve_d_intersect_ray, curve_d_point_at_t, curve_d_slope_at_t, curve_dd_point_at_t,
    curve_dd_slope_at_t, curve_intersect_ray, curve_is_vertical, curve_point_at_t, verb_points,
};
use crate::op_span::Collapsed;
use crate::op_state::{AngleId, ContourId, OpState, PtTId, SK_MIN_S32, SegId, SpanId};
use crate::path_op::PathOp;
use crate::intersections::Intersections;
use crate::line::DLine;
use crate::point::{DPoint, DVector};
use crate::rect::Bounds;
use crate::types::{between, precisely_equal, roughly_equal, std_max, zero_or_one};

/// `kActiveEdge[op][miFrom][miTo][suFrom][suTo]`: which edges survive each operator.
// Port of: src/pathops/SkOpSegment.cpp#L26-L44 (chrome/m156)
const K_ACTIVE_EDGE: [[[[[bool; 2]; 2]; 2]; 2]; 4] = [
    [[[[false, false], [false, false]], [[true, false], [true, false]]], [[[true, true], [false, false]], [[false, true], [true, false]]]],
    [[[[false, false], [false, false]], [[false, true], [false, true]]], [[[false, false], [true, true]], [[false, true], [true, false]]]],
    [[[[false, true], [true, false]], [[true, true], [false, false]]], [[[true, false], [true, false]], [[false, false], [false, false]]]],
    [[[[false, true], [true, false]], [[true, false], [false, true]]], [[[true, false], [false, true]], [[false, true], [true, false]]]],
];

/// `kUnaryActiveEdge[from][to]`.
// Port of: src/pathops/SkOpSegment.cpp#L24-L25 (chrome/m156)
const K_UNARY_ACTIVE_EDGE: [[bool; 2]; 2] = [[false, true], [true, false]];

/// `SkOpSegment`: one curve of the op graph, with its spans and winding bookkeeping.
// Port of: src/pathops/SkOpSegment.h#L24-L100 (chrome/m156)
#[doc(alias = "SkOpSegment")]
#[derive(Clone, Debug)]
pub(crate) struct Segment {
    /// `SkOpContour* fContour`.
    pub(crate) contour: ContourId,
    /// `SkOpSpan fHead`: the head span always has its t set to zero.
    pub(crate) head: SpanId,
    /// `SkOpSpanBase fTail`: the tail span always has its t set to one.
    pub(crate) tail: SpanId,
    /// `SkOpSegment* fNext`: forward-only list used by the contour to walk its segments.
    pub(crate) next: Option<SegId>,
    /// `const SkOpSegment* fPrev`.
    pub(crate) prev: Option<SegId>,
    /// `SkPoint* fPts`: the curve's points (`fPts[0..=points]`).
    pub(crate) pts: [Point; 4],
    /// `SkScalar fWeight`.
    pub(crate) weight: f32,
    /// `int fCount`: number of spans (one for a non-intersecting segment).
    pub(crate) count: i32,
    /// `int fDoneCount`: number of processed spans (zero initially).
    pub(crate) done_count: i32,
    /// `SkPath::Verb fVerb`.
    pub(crate) verb: Verb,
    /// `bool fVisited`: used by the missing coincidence check.
    pub(crate) visited: bool,
    /// `SkPathOpsBounds fBounds`: tight bounds.
    pub(crate) bounds: Bounds,
}

impl Default for Segment {
    fn default() -> Self {
        Self {
            contour: ContourId(0),
            head: SpanId(usize::MAX),
            tail: SpanId(usize::MAX),
            next: None,
            prev: None,
            pts: [Point::default(); 4],
            weight: 1.0,
            count: 0,
            done_count: 0,
            verb: Verb::Move,
            visited: false,
            bounds: Bounds::default(),
        }
    }
}

/// `SkOpSegment::StepSign`-style step: `step(end)`, the direction from `start` to `end`.
#[must_use]
fn step_from(start_t: f64, end_t: f64) -> i32 {
    if start_t < end_t { 1 } else { -1 }
}

impl OpState {
    /// `SkOpSegment::init(pts, weight, contour, verb)`: the head and tail spans get their
    /// t values and the head links to the tail.
    // Port of: src/pathops/SkOpSegment.cpp#L823-L838 (chrome/m156)
    pub(crate) fn seg_init(
        &mut self,
        seg: SegId,
        pts: &[Point],
        weight: f32,
        contour: ContourId,
        verb: Verb,
    ) {
        let head = self.alloc_span();
        let tail = self.alloc_span();
        let mut stored = [Point::default(); 4];
        stored[..pts.len()].copy_from_slice(pts);
        let s = &mut self.segments[seg.0];
        s.contour = contour;
        s.next = None;
        s.pts = stored;
        s.weight = weight;
        s.verb = verb;
        s.count = 0;
        s.done_count = 0;
        s.visited = false;
        s.head = head;
        s.tail = tail;
        let zero_span = head;
        let p0 = stored[0];
        let one_pt = stored[verb_points(verb)];
        self.span_init(zero_span, seg, None, 0.0, p0);
        self.spans[zero_span.0].next = Some(tail);
        self.span_init_base(tail, seg, Some(zero_span), 1.0, one_pt);
    }

    /// `SkOpSegment::addConic(pts, weight, parent)`'s bounds, after `init`.
    // Port of: src/pathops/SkOpSegment.h#L28-L35 (chrome/m156)
    pub(crate) fn seg_set_conic_bounds(&mut self, seg: SegId) {
        let (pts, weight) = {
            let s = &self.segments[seg.0];
            (s.pts, s.weight)
        };
        let mut curve = DCurveBuf::default();
        curve.pts[0] = DPoint::from_sk_point(pts[0]);
        curve.pts[1] = DPoint::from_sk_point(pts[1]);
        curve.pts[2] = DPoint::from_sk_point(pts[2]);
        curve.weight = weight;
        let mut bounds = Bounds::default();
        curve.set_conic_bounds([pts[0], pts[1], pts[2]], weight, 0.0, 1.0, &mut bounds);
        self.segments[seg.0].bounds = bounds;
    }

    /// The `SkDCurve::setCubicBounds`/`setQuadBounds` step of `addCubic`/`addQuad`.
    // Port of: src/pathops/SkOpSegment.h#L37-L58 (chrome/m156)
    pub(crate) fn seg_set_poly_bounds(&mut self, seg: SegId) {
        let (pts, weight, verb) = {
            let s = &self.segments[seg.0];
            (s.pts, s.weight, s.verb)
        };
        let mut curve = DCurveBuf::default();
        for (dst, src) in curve.pts.iter_mut().zip(pts) {
            *dst = DPoint::from_sk_point(src);
        }
        let mut bounds = Bounds::default();
        if verb == Verb::Cubic {
            curve.set_cubic_bounds(pts, 1.0, 0.0, 1.0, &mut bounds);
        } else {
            curve.set_quad_bounds([pts[0], pts[1], pts[2]], 1.0, 0.0, 1.0, &mut bounds);
        }
        self.segments[seg.0].bounds = bounds;
    }

    /// `SkOpSegment::contour()`.
    #[must_use]
    pub(crate) fn seg_contour(&self, seg: SegId) -> ContourId {
        self.segments[seg.0].contour
    }

    /// `SkOpSegment::head()`.
    #[must_use]
    pub(crate) fn seg_head(&self, seg: SegId) -> SpanId {
        self.segments[seg.0].head
    }

    /// `SkOpSegment::tail()`.
    #[must_use]
    pub(crate) fn seg_tail(&self, seg: SegId) -> SpanId {
        self.segments[seg.0].tail
    }

    /// `SkOpSegment::next()`.
    #[must_use]
    pub(crate) fn seg_next(&self, seg: SegId) -> Option<SegId> {
        self.segments[seg.0].next
    }

    /// `SkOpSegment::prev()`.
    #[must_use]
    pub(crate) fn seg_prev(&self, seg: SegId) -> Option<SegId> {
        self.segments[seg.0].prev
    }

    /// `SkOpSegment::setNext(next)`.
    pub(crate) fn seg_set_next(&mut self, seg: SegId, next: Option<SegId>) {
        self.segments[seg.0].next = next;
    }

    /// `SkOpSegment::setPrev(prev)`.
    pub(crate) fn seg_set_prev(&mut self, seg: SegId, prev: Option<SegId>) {
        self.segments[seg.0].prev = prev;
    }

    /// `SkOpSegment::setContour(contour)`.
    pub(crate) fn seg_set_contour(&mut self, seg: SegId, contour: ContourId) {
        self.segments[seg.0].contour = contour;
    }

    /// `SkOpSegment::verb()`.
    #[must_use]
    pub(crate) fn seg_verb(&self, seg: SegId) -> Verb {
        self.segments[seg.0].verb
    }

    /// `SkOpSegment::weight()`.
    #[must_use]
    pub(crate) fn seg_weight(&self, seg: SegId) -> f32 {
        self.segments[seg.0].weight
    }

    /// `SkOpSegment::pts()`.
    #[must_use]
    pub(crate) fn seg_pts(&self, seg: SegId) -> [Point; 4] {
        self.segments[seg.0].pts
    }

    /// `SkOpSegment::lastPt()`.
    #[must_use]
    pub(crate) fn seg_last_pt(&self, seg: SegId) -> Point {
        let s = &self.segments[seg.0];
        s.pts[verb_points(s.verb)]
    }

    /// `SkOpSegment::bounds()`.
    #[must_use]
    pub(crate) fn seg_bounds(&self, seg: SegId) -> Bounds {
        self.segments[seg.0].bounds
    }

    /// `SkOpSegment::bumpCount()`.
    pub(crate) fn seg_bump_count(&mut self, seg: SegId) {
        self.segments[seg.0].count += 1;
    }

    /// `SkOpSegment::count()`.
    #[must_use]
    pub(crate) fn seg_count(&self, seg: SegId) -> i32 {
        self.segments[seg.0].count
    }

    /// `SkOpSegment::done()`: every span is processed.
    #[must_use]
    pub(crate) fn seg_done(&self, seg: SegId) -> bool {
        let s = &self.segments[seg.0];
        s.done_count == s.count
    }

    /// `SkOpSegment::done(const SkOpAngle* angle)`: `angle->start()->starter(angle->end())->done()`.
    // Port of: src/pathops/SkOpSegment.h#L203-L205 (chrome/m156)
    #[must_use]
    pub(crate) fn seg_done_angle(&self, _seg: SegId, angle: AngleId) -> bool {
        let lesser = self.span_starter(self.angle_start(angle), self.angle_end(angle));
        self.span_done(lesser)
    }

    /// `SkOpSegment::visited()`: returns the old flag and sets it.
    // Port of: src/pathops/SkOpSegment.h#L370-L378 (chrome/m156)
    pub(crate) fn seg_visited(&mut self, seg: SegId) -> bool {
        let s = &mut self.segments[seg.0];
        if !s.visited {
            s.visited = true;
            return false;
        }
        true
    }

    /// `SkOpSegment::resetVisited()`.
    pub(crate) fn seg_reset_visited(&mut self, seg: SegId) {
        self.segments[seg.0].visited = false;
    }

    /// `SkOpSegment::operand()`.
    #[must_use]
    pub(crate) fn seg_operand(&self, seg: SegId) -> bool {
        self.contour_operand(self.segments[seg.0].contour)
    }

    /// `SkOpSegment::oppXor()`.
    #[must_use]
    pub(crate) fn seg_opp_xor(&self, seg: SegId) -> bool {
        self.contour_opp_xor(self.segments[seg.0].contour)
    }

    /// `SkOpSegment::isXor()`.
    // Port of: src/pathops/SkOpSegment.cpp#L855-L857 (chrome/m156)
    #[must_use]
    pub(crate) fn seg_is_xor(&self, seg: SegId) -> bool {
        self.contour_xor(self.segments[seg.0].contour)
    }

    /// `SkOpSegment::isVertical()`.
    #[must_use]
    pub(crate) fn seg_is_vertical_bounds(&self, seg: SegId) -> bool {
        let b = self.segments[seg.0].bounds;
        b.left == b.right
    }

    /// `SkOpSegment::isHorizontal()`.
    #[must_use]
    pub(crate) fn seg_is_horizontal(&self, seg: SegId) -> bool {
        let b = self.segments[seg.0].bounds;
        b.top == b.bottom
    }

    /// `SkOpSegment::isVertical(start, end)`.
    #[must_use]
    pub(crate) fn seg_is_vertical_span(&self, seg: SegId, start: SpanId, end: SpanId) -> bool {
        let s = &self.segments[seg.0];
        curve_is_vertical(s.verb, &s.pts, s.weight, self.span_t(start), self.span_t(end))
    }

    /// `SkOpSegment::ptAtT(mid)`.
    #[must_use]
    pub(crate) fn seg_pt_at_t(&self, seg: SegId, mid: f64) -> Point {
        let s = &self.segments[seg.0];
        curve_point_at_t(s.verb, &s.pts, s.weight, mid)
    }

    /// `SkOpSegment::dPtAtT(mid)`.
    #[must_use]
    pub(crate) fn seg_d_pt_at_t(&self, seg: SegId, mid: f64) -> DPoint {
        let s = &self.segments[seg.0];
        curve_d_point_at_t(s.verb, &s.pts, s.weight, mid)
    }

    /// `SkOpSegment::dSlopeAtT(mid)`.
    #[must_use]
    pub(crate) fn seg_d_slope_at_t(&self, seg: SegId, mid: f64) -> DVector {
        let s = &self.segments[seg.0];
        curve_d_slope_at_t(s.verb, &s.pts, s.weight, mid)
    }

    /// `SkOpSegment::markDone(span)`.
    // Port of: src/pathops/SkOpSegment.cpp#L1015-L1021 (chrome/m156)
    pub(crate) fn seg_mark_done(&mut self, seg: SegId, span: SpanId) {
        if self.span_done(span) {
            return;
        }
        self.span_set_done(span, true);
        self.segments[seg.0].done_count += 1;
    }

    /// `SkOpSegment::markAllDone()`.
    // Port of: src/pathops/SkOpSegment.cpp#L859-L866 (chrome/m156)
    pub(crate) fn seg_mark_all_done(&mut self, seg: SegId) {
        let mut span = self.seg_head(seg);
        loop {
            self.seg_mark_done(seg, span);
            let Some(next) = self.span_next(span) else {
                break;
            };
            match self.span_up_castable(next) {
                Some(s) => span = s,
                None => break,
            }
        }
    }

    /// `SkOpSegment::markWinding(span, winding)`.
    // Port of: src/pathops/SkOpSegment.cpp#L1028-L1035 (chrome/m156)
    pub(crate) fn seg_mark_winding(&mut self, span: SpanId, winding: i32) -> bool {
        if self.span_done(span) {
            return false;
        }
        self.span_set_wind_sum(span, winding);
        true
    }

    /// `SkOpSegment::markWinding(span, winding, oppWinding)`.
    // Port of: src/pathops/SkOpSegment.cpp#L1042-L1050 (chrome/m156)
    pub(crate) fn seg_mark_winding_opp(&mut self, span: SpanId, winding: i32, opp_winding: i32) -> bool {
        if self.span_done(span) {
            return false;
        }
        self.span_set_wind_sum(span, winding);
        self.span_set_opp_sum(span, opp_winding);
        true
    }

    /// `SkOpSegment::release(const SkOpSpan* span)`.
    // Port of: src/pathops/SkOpSegment.cpp#L505-L512 (chrome/m156)
    pub(crate) fn seg_release(&mut self, seg: SegId, span: SpanId) {
        if self.span_done(span) {
            self.segments[seg.0].done_count -= 1;
        }
        self.segments[seg.0].count -= 1;
    }

    /// `SkOpSegment::contains(double newT)`.
    // Port of: src/pathops/SkOpSegment.cpp#L491-L503 (chrome/m156)
    #[must_use]
    pub(crate) fn seg_contains(&self, seg: SegId, new_t: f64) -> bool {
        let mut span_base = self.seg_head(seg);
        let tail = self.seg_tail(seg);
        loop {
            if self.ptt_contains_seg_t(self.span_ptt(span_base), seg, new_t) {
                return true;
            }
            if span_base == tail {
                break;
            }
            span_base = self.span_next(span_base).expect("span has next");
        }
        false
    }

    /// `SkOpSegment::existing(double t, const SkOpSegment* opp)`.
    // Port of: src/pathops/SkOpSegment.cpp#L204-L234 (chrome/m156)
    #[must_use]
    pub(crate) fn seg_existing(&self, seg: SegId, t: f64, opp: Option<SegId>) -> Option<PtTId> {
        let pt = self.seg_pt_at_t(seg, t);
        let mut test = self.seg_head(seg);
        let mut test_ptt;
        loop {
            test_ptt = self.span_ptt(test);
            if self.ptts[test_ptt.0].t == t {
                break;
            }
            if !self.seg_match(seg, test_ptt, seg, t, pt) {
                if t < self.ptts[test_ptt.0].t {
                    return None;
                }
            } else {
                if opp.is_none() {
                    return Some(test_ptt);
                }
                let mut lp = self.ptt_next(test_ptt);
                let mut found = false;
                while lp != test_ptt {
                    if self.ptt_segment(lp) == seg && self.ptts[lp.0].t == t && self.ptts[lp.0].pt == pt {
                        found = true;
                        break;
                    }
                    lp = self.ptt_next(lp);
                }
                if !found {
                    return None;
                }
                break; // foundMatch
            }
            test = self.span_next(test)?;
        }
        // foundMatch: `opp && !test->contains(opp) ? nullptr : testPtT`
        if let Some(opp) = opp {
            if self.span_contains_seg(test, opp).is_none() {
                return None;
            }
        }
        Some(test_ptt)
    }

    /// `SkOpSegment::match(base, testParent, testT, testPt)`.
    // Port of: src/pathops/SkOpSegment.cpp#L1057-L1076 (chrome/m156)
    #[must_use]
    pub(crate) fn seg_match(
        &self,
        seg: SegId,
        base: PtTId,
        test_parent: SegId,
        test_t: f64,
        test_pt: Point,
    ) -> bool {
        if seg == test_parent && precisely_equal(self.ptts[base.0].t, test_t) {
            return true;
        }
        if !approx_equal_points(test_pt, self.ptts[base.0].pt) {
            return false;
        }
        seg != test_parent
            || !self.seg_pts_disjoint(
                seg,
                self.ptts[base.0].t,
                self.ptts[base.0].pt,
                test_t,
                test_pt,
            )
    }

    /// `SkOpSegment::ptsDisjoint(t1, pt1, t2, pt2)`.
    // Port of: src/pathops/SkOpSegment.cpp#L1505-L1519 (chrome/m156)
    #[must_use]
    pub(crate) fn seg_pts_disjoint(&self, seg: SegId, t1: f64, pt1: Point, t2: f64, pt2: Point) -> bool {
        if self.segments[seg.0].verb == Verb::Line {
            return false;
        }
        let mid_t = (t1 + t2) / 2.0;
        let mid_pt = self.seg_pt_at_t(seg, mid_t);
        let se_dist_sq = std_max(dist_sq(pt1, pt2) * 2.0, f32::EPSILON * 2.0);
        dist_sq(mid_pt, pt1) > se_dist_sq || dist_sq(mid_pt, pt2) > se_dist_sq
    }

    /// `SkOpSegment::addT(double t, const SkPoint& pt)`.
    // Port of: src/pathops/SkOpSegment.cpp#L260-L287 (chrome/m156)
    pub(crate) fn seg_add_t_pt(&mut self, seg: SegId, t: f64, pt: Point) -> Option<PtTId> {
        let mut span_base = self.seg_head(seg);
        let tail = self.seg_tail(seg);
        loop {
            let result = self.span_ptt(span_base);
            if t == self.ptts[result.0].t
                || (!zero_or_one(t) && self.seg_match(seg, result, seg, t, pt))
            {
                self.span_bump_span_adds(span_base);
                return Some(result);
            }
            if t < self.ptts[result.0].t {
                let span_of = self.ptts[result.0].span;
                let prev = self.span_prev(span_of)?;
                let span = self.insert_span(prev);
                self.span_init(span, seg, Some(prev), t, pt);
                self.span_bump_span_adds(span);
                return Some(self.span_ptt(span));
            }
            if span_base == tail {
                return None;
            }
            span_base = self.span_next(span_base)?;
        }
    }

    /// `SkOpSegment::addT(double t)`.
    // Port of: src/pathops/SkOpSegment.cpp#L289-L291 (chrome/m156)
    pub(crate) fn seg_add_t(&mut self, seg: SegId, t: f64) -> Option<PtTId> {
        let pt = self.seg_pt_at_t(seg, t);
        self.seg_add_t_pt(seg, t, pt)
    }

    /// `SkOpSegment::addExpanded(newT, test, startOver)`.
    // Port of: src/pathops/SkOpSegment.cpp#L236-L258 (chrome/m156)
    pub(crate) fn seg_add_expanded(
        &mut self,
        seg: SegId,
        new_t: f64,
        test: SpanId,
        start_over: &mut bool,
    ) -> bool {
        if self.seg_contains(seg, new_t) {
            return true;
        }
        self.reset_allocated_op_span();
        if !between(0.0, new_t, 1.0) {
            return false;
        }
        let Some(new_ptt) = self.seg_add_t(seg, new_t) else {
            *start_over |= self.allocated_op_span();
            return false;
        };
        *start_over |= self.allocated_op_span();
        let pt = self.seg_pt_at_t(seg, new_t);
        self.ptts[new_ptt.0].pt = pt;
        let test_ptt = self.span_ptt(test);
        if let Some(opp_prev) = self.ptt_opp_prev(test_ptt, new_ptt) {
            let new_span = self.ptt_span(new_ptt);
            self.span_merge_matches(test, new_span);
            self.ptt_add_opp(test_ptt, new_ptt, opp_prev);
            self.span_check_for_collapsed_coincidence(test);
        }
        true
    }

    /// `SkOpSegment::addCurveTo(start, end, path)`.
    // Port of: src/pathops/SkOpSegment.cpp#L173-L202 (chrome/m156)
    pub(crate) fn seg_add_curve_to(
        &mut self,
        seg: SegId,
        start: SpanId,
        end: SpanId,
        path: &mut crate::path_writer::PathWriter,
    ) -> bool {
        let span_start = self.span_starter(start, end);
        if self.span_already_added(span_start) {
            return false;
        }
        self.span_mark_added(span_start);
        let start_seg = self.span_segment(start);
        let mut curve_part = crate::op_curve::DCurveSweep::default();
        self.seg_sub_divide(start_seg, start, end, &mut curve_part.curve);
        curve_part.set_curve_hull_sweep(self.segments[seg.0].verb);
        let verb = if curve_part.is_curve {
            self.segments[seg.0].verb
        } else {
            Verb::Line
        };
        path.deferred_move(self.ptt_pt_of_span(start));
        match verb {
            Verb::Line => {
                if !path.deferred_line(self.ptt_pt_of_span(end)) {
                    return false;
                }
            }
            Verb::Quad => {
                path.quad_to(curve_part.curve.pts[1].as_sk_point(), self.ptt_pt_of_span(end));
            }
            Verb::Conic => {
                path.conic_to(
                    curve_part.curve.pts[1].as_sk_point(),
                    self.ptt_pt_of_span(end),
                    curve_part.curve.weight,
                );
            }
            Verb::Cubic => {
                path.cubic_to(
                    curve_part.curve.pts[1].as_sk_point(),
                    curve_part.curve.pts[2].as_sk_point(),
                    self.ptt_pt_of_span(end),
                );
            }
            _ => {}
        }
        true
    }

    /// The point of a span's point record (`SkOpSpanBase::ptT()->fPt`), as `addCurveTo` passes it
    /// to the path writer.
    #[must_use]
    fn ptt_pt_of_span(&self, span: SpanId) -> Point {
        self.ptts[self.span_ptt(span).0].pt
    }

    /// `SkOpSegment::subDivide(start, end, edge)`: the sub-curve from `start` to `end`.
    // Port of: src/pathops/SkOpSegment.cpp#L1625-L1670 (chrome/m156)
    pub(crate) fn seg_sub_divide(
        &self,
        seg: SegId,
        start: SpanId,
        end: SpanId,
        edge: &mut DCurveBuf,
    ) -> bool {
        let s = &self.segments[seg.0];
        let start_ptt = self.ptts[self.span_ptt(start).0];
        let end_ptt = self.ptts[self.span_ptt(end).0];
        edge.pts[0] = DPoint::from_sk_point(start_ptt.pt);
        let points = verb_points(s.verb);
        edge.pts[points] = DPoint::from_sk_point(end_ptt.pt);
        if s.verb == Verb::Line {
            return false;
        }
        let start_t = start_ptt.t;
        let end_t = end_ptt.t;
        if (start_t == 0.0 || end_t == 0.0) && (start_t == 1.0 || end_t == 1.0) {
            if s.verb == Verb::Quad {
                edge.pts[1] = DPoint::from_sk_point(s.pts[1]);
                return false;
            }
            if s.verb == Verb::Conic {
                edge.pts[1] = DPoint::from_sk_point(s.pts[1]);
                edge.weight = s.weight;
                return false;
            }
            if start_t == 0.0 {
                edge.pts[1] = DPoint::from_sk_point(s.pts[1]);
                edge.pts[2] = DPoint::from_sk_point(s.pts[2]);
                return false;
            }
            edge.pts[1] = DPoint::from_sk_point(s.pts[2]);
            edge.pts[2] = DPoint::from_sk_point(s.pts[1]);
            return false;
        }
        match s.verb {
            Verb::Quad => {
                // edge->fQuad[1] = SkDQuad::SubDivide(fPts, edge->fQuad[0], edge->fQuad[2], ...)
                let mut quad = crate::quad::DQuad::default();
                quad.set([s.pts[0], s.pts[1], s.pts[2]]);
                edge.pts[1] = quad.sub_divide_points(edge.pts[0], edge.pts[2], start_t, end_t);
            }
            Verb::Conic => {
                // edge->fConic[1] = SkDConic::SubDivide(fPts, fWeight, ..., &edge->fConic.fWeight)
                let mut conic = crate::conic::DConic::default();
                conic.set([s.pts[0], s.pts[1], s.pts[2]], s.weight);
                edge.pts[1] = conic.sub_divide_points(edge.pts[0], edge.pts[2], start_t, end_t);
                edge.weight = conic.sub_divide(start_t, end_t).weight;
            }
            _ => {
                // SkDCubic::SubDivide(fPts, edge->fCubic[0], edge->fCubic[3], ..., &edge->fCubic[1])
                let mut cubic = crate::cubic::DCubic::default();
                cubic.set([s.pts[0], s.pts[1], s.pts[2], s.pts[3]]);
                let mid = cubic.sub_divide_ad(edge.pts[0], edge.pts[3], start_t, end_t);
                edge.pts[1] = mid[0];
                edge.pts[2] = mid[1];
            }
        }
        true
    }

    /// `SkOpSegment::calcAngles()`.
    // Port of: src/pathops/SkOpSegment.cpp#L293-L322 (chrome/m156)
    pub(crate) fn seg_calc_angles(&mut self, seg: SegId) {
        let head = self.seg_head(seg);
        let tail = self.seg_tail(seg);
        let mut active_prior = !self.span_is_canceled(head);
        if active_prior && !self.span_simple(head) {
            self.seg_add_start_span(seg);
        }
        let mut prior = head;
        let mut span_base = self.span_next(head).expect("head has next");
        while span_base != tail {
            if active_prior {
                let prior_angle = self.alloc_angle();
                self.angle_set(prior_angle, span_base, prior);
                self.span_set_from_angle(span_base, Some(prior_angle));
            }
            let span = span_base;
            let active = !self.span_is_canceled(span);
            let next = self.span_next(span).expect("span has next");
            if active {
                let angle = self.alloc_angle();
                self.angle_set(angle, span, next);
                self.span_set_to_angle(span, Some(angle));
            }
            active_prior = active;
            prior = span;
            span_base = next;
        }
        if active_prior && !self.span_simple(tail) {
            self.seg_add_end_span(seg);
        }
    }

    /// `SkOpSpanBase::simple()`: the point ring has exactly two points.
    #[must_use]
    pub(crate) fn span_simple(&self, span: SpanId) -> bool {
        let ptt = self.span_ptt(span);
        let next = self.ptt_next(ptt);
        self.ptt_next(next) == ptt
    }

    /// `SkOpSegment::addStartSpan()`.
    // Port of: src/pathops/SkOpSegment.h#L140-L146 (chrome/m156)
    pub(crate) fn seg_add_start_span(&mut self, seg: SegId) -> AngleId {
        let angle = self.alloc_angle();
        let head = self.seg_head(seg);
        let next = self.span_next(head).expect("head has next");
        self.angle_set(angle, head, next);
        self.span_set_to_angle(head, Some(angle));
        angle
    }

    /// `SkOpSegment::addEndSpan()`.
    // Port of: src/pathops/SkOpSegment.h#L123-L129 (chrome/m156)
    pub(crate) fn seg_add_end_span(&mut self, seg: SegId) -> AngleId {
        let angle = self.alloc_angle();
        let tail = self.seg_tail(seg);
        let prev = self.span_prev_base(tail);
        self.angle_set(angle, tail, prev);
        self.span_set_from_angle(tail, Some(angle));
        angle
    }

    /// `SkOpSpanBase::prev()` for a base-only tail: the span before it.
    #[must_use]
    pub(crate) fn span_prev_base(&self, span: SpanId) -> SpanId {
        self.spans[span.0].prev.expect("tail has a previous span")
    }

    /// `SkOpSegment::clearAll()`.
    // Port of: src/pathops/SkOpSegment.cpp#L324-L331 (chrome/m156)
    pub(crate) fn seg_clear_all(&mut self, seg: SegId) {
        let mut span = self.seg_head(seg);
        loop {
            self.seg_clear_one(seg, span);
            let Some(next) = self.span_next(span) else {
                break;
            };
            match self.span_up_castable(next) {
                Some(s) => span = s,
                None => break,
            }
        }
        self.coin_release_seg(seg);
    }

    /// `SkOpSegment::clearOne(span)`.
    // Port of: src/pathops/SkOpSegment.cpp#L333-L337 (chrome/m156)
    pub(crate) fn seg_clear_one(&mut self, seg: SegId, span: SpanId) {
        self.span_set_wind_value(span, 0);
        self.span_set_opp_value(span, 0);
        self.seg_mark_done(seg, span);
    }

    /// `SkOpSegment::collapsed(s, e)`.
    // Port of: src/pathops/SkOpSegment.cpp#L339-L348 (chrome/m156)
    #[must_use]
    pub(crate) fn seg_collapsed(&self, seg: SegId, start_t: f64, end_t: f64) -> Collapsed {
        let mut span = self.seg_head(seg);
        loop {
            let result = self.span_collapsed(span, start_t, end_t);
            if result != Collapsed::No {
                return result;
            }
            match self.span_up_castable(span).and_then(|s| self.span_next(s)) {
                Some(next) => span = next,
                None => break,
            }
        }
        Collapsed::No
    }

    /// `SkOpSegment::computeSum(start, end, includeType)`: the winding sum that the angles
    /// around `start` produce, or `SK_NaN32` when they can't be ordered.
    // Port of: src/pathops/SkOpSegment.cpp#L421-L489 (chrome/m156)
    pub(crate) fn seg_compute_sum(
        &mut self,
        seg: SegId,
        start: SpanId,
        end: SpanId,
        include_type: IncludeType,
    ) -> i32 {
        let Some(mut first_angle) = self.span_to_angle_toward(end, start) else {
            return SK_NAN32;
        };
        if self.angle_next(first_angle).is_none() {
            return SK_NAN32;
        }
        let mut base_angle: Option<AngleId> = None;
        let mut try_reverse = false;
        let mut angle = self.angle_previous(first_angle);
        let mut next = self.angle_next(angle).expect("angle has next");
        first_angle = next;
        loop {
            let prior = angle;
            angle = next;
            next = self.angle_next(angle).expect("angle has next");
            if self.angle_unorderable(prior)
                || self.angle_unorderable(angle)
                || self.angle_unorderable(next)
            {
                base_angle = None;
            } else {
                let test_winding = self.span_wind_sum(self.angle_starter(angle));
                if SK_MIN_S32 != test_winding {
                    base_angle = Some(angle);
                    try_reverse = true;
                } else if let Some(base) = base_angle {
                    self.compute_one_sum(base, angle, include_type);
                    base_angle = if SK_MIN_S32 != self.span_wind_sum(self.angle_starter(angle)) {
                        Some(angle)
                    } else {
                        None
                    };
                }
            }
            if next == first_angle {
                break;
            }
        }
        if let Some(base) = base_angle {
            if SK_MIN_S32 == self.span_wind_sum(self.angle_starter(first_angle)) {
                first_angle = base;
                try_reverse = true;
            }
        }
        if try_reverse {
            base_angle = None;
            let mut prior = first_angle;
            loop {
                angle = prior;
                prior = self.angle_previous(angle);
                next = self.angle_next(angle).expect("angle has next");
                if self.angle_unorderable(prior)
                    || self.angle_unorderable(angle)
                    || self.angle_unorderable(next)
                {
                    base_angle = None;
                } else {
                    let test_winding = self.span_wind_sum(self.angle_starter(angle));
                    if SK_MIN_S32 != test_winding {
                        base_angle = Some(angle);
                    } else if let Some(base) = base_angle {
                        self.compute_one_sum_reverse(base, angle, include_type);
                        base_angle = if SK_MIN_S32 != self.span_wind_sum(self.angle_starter(angle)) {
                            Some(angle)
                        } else {
                            None
                        };
                    }
                }
                if prior == first_angle {
                    break;
                }
            }
        }
        self.span_wind_sum(self.span_starter(start, end))
    }

    /// `SkOpSegment::ComputeOneSum(baseAngle, nextAngle, includeType)`.
    // Port of: src/pathops/SkOpSegment.cpp#L350-L383 (chrome/m156)
    fn compute_one_sum(&mut self, base_angle: AngleId, next_angle: AngleId, include_type: IncludeType) -> bool {
        let base_seg = self.angle_segment(base_angle);
        let mut sum_mi_winding = self.seg_update_winding_reverse(base_seg, base_angle);
        let mut sum_su_winding = 0;
        let binary = include_type >= IncludeType::BinarySingle;
        if binary {
            sum_su_winding = self.seg_update_opp_winding_reverse(base_seg, base_angle);
            if self.seg_operand(base_seg) {
                std::mem::swap(&mut sum_mi_winding, &mut sum_su_winding);
            }
        }
        let next_seg = self.angle_segment(next_angle);
        let next_start = self.angle_start(next_angle);
        let next_end = self.angle_end(next_angle);
        let mut last: Option<SpanId> = None;
        if binary {
            let (max_winding, sum_winding, opp_max_winding, opp_sum_winding) = self.seg_set_up_windings_opp(
                next_seg,
                next_start,
                next_end,
                &mut sum_mi_winding,
                &mut sum_su_winding,
            );
            if !self.seg_mark_angle_opp(
                next_seg,
                max_winding,
                sum_winding,
                opp_max_winding,
                opp_sum_winding,
                next_angle,
                &mut last,
            ) {
                return false;
            }
        } else {
            let (max_winding, sum_winding) = self.seg_set_up_windings(
                next_seg,
                next_start,
                next_end,
                &mut sum_mi_winding,
            );
            if !self.seg_mark_angle(next_seg, max_winding, sum_winding, next_angle, &mut last) {
                return false;
            }
        }
        self.angle_set_last_marked(next_angle, last);
        true
    }

    /// `SkOpSegment::ComputeOneSumReverse(baseAngle, nextAngle, includeType)`.
    // Port of: src/pathops/SkOpSegment.cpp#L385-L419 (chrome/m156)
    fn compute_one_sum_reverse(&mut self, base_angle: AngleId, next_angle: AngleId, include_type: IncludeType) -> bool {
        let base_seg = self.angle_segment(base_angle);
        let mut sum_mi_winding = self.seg_update_winding(base_seg, base_angle);
        let mut sum_su_winding = 0;
        let binary = include_type >= IncludeType::BinarySingle;
        if binary {
            sum_su_winding = self.seg_update_opp_winding(base_seg, base_angle);
            if self.seg_operand(base_seg) {
                std::mem::swap(&mut sum_mi_winding, &mut sum_su_winding);
            }
        }
        let next_seg = self.angle_segment(next_angle);
        let next_start = self.angle_end(next_angle);
        let next_end = self.angle_start(next_angle);
        let mut last: Option<SpanId> = None;
        if binary {
            let (max_winding, sum_winding, opp_max_winding, opp_sum_winding) = self.seg_set_up_windings_opp(
                next_seg,
                next_start,
                next_end,
                &mut sum_mi_winding,
                &mut sum_su_winding,
            );
            if !self.seg_mark_angle_opp(
                next_seg,
                max_winding,
                sum_winding,
                opp_max_winding,
                opp_sum_winding,
                next_angle,
                &mut last,
            ) {
                return false;
            }
        } else {
            let (max_winding, sum_winding) = self.seg_set_up_windings(
                next_seg,
                next_start,
                next_end,
                &mut sum_mi_winding,
            );
            if !self.seg_mark_angle(next_seg, max_winding, sum_winding, next_angle, &mut last) {
                return false;
            }
        }
        self.angle_set_last_marked(next_angle, last);
        true
    }

    /// `SkOpSegment::setUpWindings(start, end, sumMiWinding, maxWinding, sumWinding)`.
    /// Returns `(maxWinding, sumWinding)` and updates `sum_mi_winding`.
    // Port of: src/pathops/SkOpSegment.cpp#L1522-L1528 (chrome/m156)
    pub(crate) fn seg_set_up_windings(
        &self,
        _seg: SegId,
        start: SpanId,
        end: SpanId,
        sum_mi_winding: &mut i32,
    ) -> (i32, i32) {
        let delta_sum = self.span_sign(start, end);
        let max_winding = *sum_mi_winding;
        *sum_mi_winding -= delta_sum;
        (max_winding, *sum_mi_winding)
    }

    /// `SkOpSegment::setUpWindings(start, end, sumMiWinding, sumSuWinding, maxWinding,
    /// sumWinding, oppMaxWinding, oppSumWinding)`. Returns the four outputs.
    // Port of: src/pathops/SkOpSegment.cpp#L1530-L1548 (chrome/m156)
    pub(crate) fn seg_set_up_windings_opp(
        &self,
        seg: SegId,
        start: SpanId,
        end: SpanId,
        sum_mi_winding: &mut i32,
        sum_su_winding: &mut i32,
    ) -> (i32, i32, i32, i32) {
        let delta_sum = self.span_sign(start, end);
        let opp_delta_sum = self.span_opp_sign(start, end);
        if self.seg_operand(seg) {
            let max_winding = *sum_su_winding;
            *sum_su_winding -= delta_sum;
            let sum_winding = *sum_su_winding;
            let opp_max_winding = *sum_mi_winding;
            *sum_mi_winding -= opp_delta_sum;
            (max_winding, sum_winding, opp_max_winding, *sum_mi_winding)
        } else {
            let max_winding = *sum_mi_winding;
            *sum_mi_winding -= delta_sum;
            let sum_winding = *sum_mi_winding;
            let opp_max_winding = *sum_su_winding;
            *sum_su_winding -= opp_delta_sum;
            (max_winding, sum_winding, opp_max_winding, *sum_su_winding)
        }
    }

    /// `SkOpSegment::SpanSign(start, end)`.
    // Port of: src/pathops/SkOpSegment.h#L339-L344 (chrome/m156)
    #[must_use]
    pub(crate) fn span_sign(&self, start: SpanId, end: SpanId) -> i32 {
        if self.span_t(start) < self.span_t(end) {
            -self.span_wind_value(start)
        } else {
            self.span_wind_value(end)
        }
    }

    /// `SkOpSegment::OppSign(start, end)`.
    // Port of: src/pathops/SkOpSegment.h#L213-L219 (chrome/m156)
    #[must_use]
    pub(crate) fn span_opp_sign(&self, start: SpanId, end: SpanId) -> i32 {
        if self.span_t(start) < self.span_t(end) {
            -self.span_opp_value(start)
        } else {
            self.span_opp_value(end)
        }
    }

    /// `SkOpSegment::UseInnerWinding(outerWinding, innerWinding)`.
    // Port of: src/pathops/SkOpSegment.cpp#L1776-L1783 (chrome/m156)
    #[must_use]
    pub(crate) fn use_inner_winding(outer_winding: i32, inner_winding: i32) -> bool {
        let abs_out = outer_winding.abs();
        let abs_in = inner_winding.abs();
        if abs_out == abs_in {
            outer_winding < 0
        } else {
            abs_out < abs_in
        }
    }

    /// `SkOpSegment::markAngle(maxWinding, sumWinding, angle, result)`.
    // Port of: src/pathops/SkOpSegment.cpp#L961-L983 (chrome/m156)
    pub(crate) fn seg_mark_angle(
        &mut self,
        seg: SegId,
        mut max_winding: i32,
        sum_winding: i32,
        angle: AngleId,
        result: &mut Option<SpanId>,
    ) -> bool {
        if Self::use_inner_winding(max_winding, sum_winding) {
            max_winding = sum_winding;
        }
        let start = self.angle_start(angle);
        let end = self.angle_end(angle);
        if !self.seg_mark_and_chase_winding(seg, start, end, max_winding, result) {
            return false;
        }
        true
    }

    /// `SkOpSegment::markAngle(maxWinding, sumWinding, oppMaxWinding, oppSumWinding, angle,
    /// result)`.
    // Port of: src/pathops/SkOpSegment.cpp#L985-L1013 (chrome/m156)
    pub(crate) fn seg_mark_angle_opp(
        &mut self,
        seg: SegId,
        mut max_winding: i32,
        sum_winding: i32,
        mut opp_max_winding: i32,
        opp_sum_winding: i32,
        angle: AngleId,
        result: &mut Option<SpanId>,
    ) -> bool {
        if Self::use_inner_winding(max_winding, sum_winding) {
            max_winding = sum_winding;
        }
        if opp_max_winding != opp_sum_winding && Self::use_inner_winding(opp_max_winding, opp_sum_winding) {
            opp_max_winding = opp_sum_winding;
        }
        let start = self.angle_start(angle);
        let end = self.angle_end(angle);
        self.seg_mark_and_chase_winding_opp(seg, start, end, max_winding, opp_max_winding, result)
    }

    /// `SkOpSegment::markAndChaseWinding(start, end, winding, lastPtr)`.
    // Port of: src/pathops/SkOpSegment.cpp#L899-L922 (chrome/m156)
    pub(crate) fn seg_mark_and_chase_winding(
        &mut self,
        seg: SegId,
        mut start: SpanId,
        end: SpanId,
        winding: i32,
        last_ptr: &mut Option<SpanId>,
    ) -> bool {
        let mut span_start = self.span_starter(start, end);
        let mut step = step_from(self.span_t(start), self.span_t(end));
        let success = self.seg_mark_winding(span_start, winding);
        let mut last: Option<SpanId> = None;
        let mut other = Some(seg);
        let mut safety_net = 1000;
        while let Some(o) = other {
            other = self.next_chase(o, &mut start, &mut step, Some(&mut span_start), Some(&mut last));
            let Some(o) = other else {
                break;
            };
            safety_net -= 1;
            if safety_net == 0 {
                return false;
            }
            if self.span_wind_sum(span_start) != SK_MIN_S32 {
                break;
            }
            let _ = self.seg_mark_winding(span_start, winding);
            let _ = o;
        }
        *last_ptr = last;
        success
    }

    /// `SkOpSegment::markAndChaseWinding(start, end, winding, oppWinding, lastPtr)`.
    // Port of: src/pathops/SkOpSegment.cpp#L924-L959 (chrome/m156)
    pub(crate) fn seg_mark_and_chase_winding_opp(
        &mut self,
        seg: SegId,
        mut start: SpanId,
        end: SpanId,
        winding: i32,
        opp_winding: i32,
        last_ptr: &mut Option<SpanId>,
    ) -> bool {
        let mut span_start = self.span_starter(start, end);
        let mut step = step_from(self.span_t(start), self.span_t(end));
        let success = self.seg_mark_winding_opp(span_start, winding, opp_winding);
        let mut last: Option<SpanId> = None;
        let mut other = Some(seg);
        let mut safety_net = 1000;
        while let Some(o) = other {
            other = self.next_chase(o, &mut start, &mut step, Some(&mut span_start), Some(&mut last));
            let Some(o) = other else {
                break;
            };
            safety_net -= 1;
            if safety_net == 0 {
                return false;
            }
            if self.span_wind_sum(span_start) != SK_MIN_S32 {
                if self.seg_operand(seg) == self.seg_operand(o) {
                    if self.span_wind_sum(span_start) != winding || self.span_opp_sum(span_start) != opp_winding {
                        self.set_winding_failed();
                        return true; // ... but let it succeed anyway
                    }
                } else {
                    if self.span_wind_sum(span_start) != opp_winding {
                        return false;
                    }
                    if self.span_opp_sum(span_start) != winding {
                        return false;
                    }
                }
                break;
            }
            if self.seg_operand(seg) == self.seg_operand(o) {
                let _ = self.seg_mark_winding_opp(span_start, winding, opp_winding);
            } else {
                let _ = self.seg_mark_winding_opp(span_start, opp_winding, winding);
            }
        }
        *last_ptr = last;
        success
    }

    /// `SkOpSegment::nextChase(startPtr, stepPtr, minPtr, last)`: the segment that continues a
    /// chain of coincident winding, from `seg`. Updates `start`, `step` and `min` in place.
    // Port of: src/pathops/SkOpSegment.cpp#L1078-L1139 (chrome/m156)
    pub(crate) fn next_chase(
        &self,
        seg: SegId,
        start_ptr: &mut SpanId,
        step_ptr: &mut i32,
        min_ptr: Option<&mut SpanId>,
        last: Option<&mut Option<SpanId>>,
    ) -> Option<SegId> {
        let _ = seg;
        let orig_start = *start_ptr;
        let step = *step_ptr;
        let mut end_span = if step > 0 {
            self.span_up_castable(orig_start).and_then(|s| self.span_next(s))?
        } else {
            self.span_prev(orig_start)?
        };
        let angle = if step > 0 {
            self.span_from_angle(end_span)
        } else {
            self.span_to_angle(end_span)
        };
        let found_span;
        let other;
        let other_end;
        if let Some(angle) = angle {
            let loop_count = self.angle_loop_count(angle);
            if loop_count > 2 {
                if let Some(last) = last {
                    *last = Some(end_span);
                }
                return None;
            }
            let Some(next) = self.angle_next(angle) else {
                return None;
            };
            other = self.angle_segment(next);
            end_span = self.angle_start(next);
            found_span = end_span;
            other_end = Some(self.angle_end(next));
        } else {
            if self.span_t(end_span) != 0.0 && self.span_t(end_span) != 1.0 {
                return None;
            }
            let other_ptt = self.ptt_next(self.span_ptt(end_span));
            other = self.ptt_segment(other_ptt);
            found_span = self.ptts[other_ptt.0].span;
            other_end = if step > 0 {
                self.span_up_castable(found_span)
                    .and_then(|s| self.span_next(s))
            } else {
                self.span_prev(found_span)
            };
        }
        let Some(other_end) = other_end else {
            return None;
        };
        let found_step = step_from(self.span_t(found_span), self.span_t(other_end));
        if *step_ptr != found_step {
            if let Some(last) = last {
                *last = Some(end_span);
            }
            return None;
        }
        let orig_min = if step < 0 {
            self.span_prev(orig_start).expect("span has prev")
        } else {
            orig_start
        };
        let found_min = self.span_starter(found_span, other_end);
        if self.span_wind_value(found_min) != self.span_wind_value(orig_min)
            || self.span_opp_value(found_min) != self.span_opp_value(orig_min)
        {
            if let Some(last) = last {
                *last = Some(end_span);
            }
            return None;
        }
        *start_ptr = found_span;
        *step_ptr = found_step;
        if let Some(min_ptr) = min_ptr {
            *min_ptr = found_min;
        }
        Some(other)
    }

    /// `SkOpSegment::markAndChaseDone(start, end, found)`.
    // Port of: src/pathops/SkOpSegment.cpp#L867-L898 (chrome/m156)
    pub(crate) fn seg_mark_and_chase_done(
        &mut self,
        seg: SegId,
        mut start: SpanId,
        end: SpanId,
        found: Option<&mut Option<SpanId>>,
    ) -> bool {
        let mut step = step_from(self.span_t(start), self.span_t(end));
        let mut min_span = self.span_starter(start, end);
        self.seg_mark_done(seg, min_span);
        let mut last: Option<SpanId> = None;
        let mut other = Some(seg);
        let mut prior_done: Option<SpanId> = None;
        let mut last_done: Option<SpanId> = None;
        let mut safety_net = 1000;
        while let Some(o) = other {
            other = self.next_chase(o, &mut start, &mut step, Some(&mut min_span), Some(&mut last));
            let Some(o) = other else {
                break;
            };
            safety_net -= 1;
            if safety_net == 0 {
                return false;
            }
            if self.seg_done(o) {
                break;
            }
            if last_done == Some(min_span) || prior_done == Some(min_span) {
                if let Some(found) = found {
                    *found = None;
                }
                return true;
            }
            self.seg_mark_done(o, min_span);
            prior_done = last_done;
            last_done = Some(min_span);
        }
        if let Some(found) = found {
            *found = last;
        }
        true
    }

    /// `SkOpSegment::activeAngleInner(start, startPtr, endPtr, done)`.
    // Port of: src/pathops/SkOpSegment.cpp#L66-L105 (chrome/m156)
    pub(crate) fn seg_active_angle_inner(
        &self,
        start: SpanId,
        start_ptr: &mut SpanId,
        end_ptr: &mut Option<SpanId>,
        done: &mut bool,
    ) -> Option<AngleId> {
        if let Some(up_span) = self.span_up_castable(start) {
            if self.span_wind_value(up_span) != 0 || self.span_opp_value(up_span) != 0 {
                let next = self.span_next(up_span).expect("span has next");
                if end_ptr.is_none() {
                    *start_ptr = start;
                    *end_ptr = Some(next);
                }
                if !self.span_done(up_span) {
                    if self.span_wind_sum(up_span) != SK_MIN_S32 {
                        return self.span_to_angle_toward(start, next);
                    }
                    *done = false;
                }
            }
        }
        if let Some(down_span) = self.span_prev(start) {
            if self.span_wind_value(down_span) != 0 || self.span_opp_value(down_span) != 0 {
                if end_ptr.is_none() {
                    *start_ptr = start;
                    *end_ptr = Some(down_span);
                }
                if !self.span_done(down_span) {
                    if self.span_wind_sum(down_span) != SK_MIN_S32 {
                        return self.span_to_angle_toward(start, down_span);
                    }
                    *done = false;
                }
            }
        }
        None
    }

    /// `SkOpSegment::activeAngleOther(start, startPtr, endPtr, done)`.
    // Port of: src/pathops/SkOpSegment.cpp#L107-L113 (chrome/m156)
    pub(crate) fn seg_active_angle_other(
        &self,
        start: SpanId,
        start_ptr: &mut SpanId,
        end_ptr: &mut Option<SpanId>,
        done: &mut bool,
    ) -> Option<AngleId> {
        let o_ptt = self.ptt_next(self.span_ptt(start));
        let other_span = self.ptts[o_ptt.0].span;
        self.seg_active_angle_inner(other_span, start_ptr, end_ptr, done)
    }

    /// `SkOpSegment::activeAngle(start, startPtr, endPtr, done)`.
    // Port of: src/pathops/SkOpSegment.cpp#L47-L53 (chrome/m156)
    pub(crate) fn seg_active_angle(
        &self,
        start: SpanId,
        start_ptr: &mut SpanId,
        end_ptr: &mut Option<SpanId>,
        done: &mut bool,
    ) -> Option<AngleId> {
        if let Some(result) = self.seg_active_angle_inner(start, start_ptr, end_ptr, done) {
            return Some(result);
        }
        self.seg_active_angle_other(start, start_ptr, end_ptr, done)
    }

    /// `SkOpSegment::activeOp(start, end, xorMiMask, xorSuMask, op)`.
    // Port of: src/pathops/SkOpSegment.cpp#L115-L128 (chrome/m156)
    pub(crate) fn seg_active_op(
        &mut self,
        seg: SegId,
        start: SpanId,
        end: SpanId,
        xor_mi_mask: i32,
        xor_su_mask: i32,
        op: PathOp,
    ) -> bool {
        let mut sum_mi_winding = self.seg_update_winding_span(seg, end, start);
        let mut sum_su_winding = self.seg_update_opp_winding_span(seg, end, start);
        if self.seg_operand(seg) {
            std::mem::swap(&mut sum_mi_winding, &mut sum_su_winding);
        }
        self.seg_active_op_sums(
            seg,
            xor_mi_mask,
            xor_su_mask,
            start,
            end,
            op,
            &mut sum_mi_winding,
            &mut sum_su_winding,
        )
    }

    /// `SkOpSegment::activeOp(xorMiMask, xorSuMask, start, end, op, sumMiWinding, sumSuWinding)`.
    // Port of: src/pathops/SkOpSegment.cpp#L130-L157 (chrome/m156)
    pub(crate) fn seg_active_op_sums(
        &self,
        seg: SegId,
        xor_mi_mask: i32,
        xor_su_mask: i32,
        start: SpanId,
        end: SpanId,
        op: PathOp,
        sum_mi_winding: &mut i32,
        sum_su_winding: &mut i32,
    ) -> bool {
        let (max_winding, sum_winding, opp_max_winding, opp_sum_winding) =
            self.seg_set_up_windings_opp(seg, start, end, sum_mi_winding, sum_su_winding);
        let (mi_from, mi_to, su_from, su_to) = if self.seg_operand(seg) {
            (
                (opp_max_winding & xor_mi_mask) != 0,
                (opp_sum_winding & xor_mi_mask) != 0,
                (max_winding & xor_su_mask) != 0,
                (sum_winding & xor_su_mask) != 0,
            )
        } else {
            (
                (max_winding & xor_mi_mask) != 0,
                (sum_winding & xor_mi_mask) != 0,
                (opp_max_winding & xor_su_mask) != 0,
                (opp_sum_winding & xor_su_mask) != 0,
            )
        };
        K_ACTIVE_EDGE[op as usize][usize::from(mi_from)][usize::from(mi_to)][usize::from(su_from)]
            [usize::from(su_to)]
    }

    /// `SkOpSegment::activeWinding(start, end)`.
    // Port of: src/pathops/SkOpSegment.cpp#L159-L162 (chrome/m156)
    pub(crate) fn seg_active_winding(&mut self, seg: SegId, start: SpanId, end: SpanId) -> bool {
        let mut sum_winding = self.seg_update_winding_span(seg, end, start);
        self.seg_active_winding_sum(start, end, &mut sum_winding)
    }

    /// `SkOpSegment::activeWinding(start, end, sumWinding)`.
    // Port of: src/pathops/SkOpSegment.cpp#L164-L171 (chrome/m156)
    pub(crate) fn seg_active_winding_sum(&self, start: SpanId, end: SpanId, sum_winding: &mut i32) -> bool {
        let max_winding = self.seg_set_up_winding(start, end, sum_winding);
        let from = max_winding != 0;
        let to = *sum_winding != 0;
        K_UNARY_ACTIVE_EDGE[usize::from(from)][usize::from(to)]
    }

    /// `SkOpSegment::setUpWinding(start, end, maxWinding, sumWinding)`: returns `maxWinding`.
    // Port of: src/pathops/SkOpSegment.h#L346-L354 (chrome/m156)
    #[must_use]
    pub(crate) fn seg_set_up_winding(&self, start: SpanId, end: SpanId, sum_winding: &mut i32) -> i32 {
        let delta_sum = self.span_sign(start, end);
        let max_winding = *sum_winding;
        if *sum_winding == SK_MIN_S32 {
            return max_winding;
        }
        *sum_winding -= delta_sum;
        max_winding
    }

    /// `SkOpSegment::updateWinding(start, end)` on the segment (`this`).
    // Port of: src/pathops/SkOpSegment.cpp#L1744-L1759 (chrome/m156)
    pub(crate) fn seg_update_winding_span(&mut self, _seg: SegId, start: SpanId, end: SpanId) -> i32 {
        let lesser = self.span_starter(start, end);
        let mut winding = self.span_wind_sum(lesser);
        if winding == SK_MIN_S32 {
            winding = self.span_compute_wind_sum(lesser);
        }
        if winding == SK_MIN_S32 {
            return winding;
        }
        let span_winding = self.span_sign(start, end);
        if winding != 0 && Self::use_inner_winding(winding - span_winding, winding) && winding != i32::MAX {
            winding -= span_winding;
        }
        winding
    }

    /// `SkOpSegment::updateOppWinding(start, end)`.
    // Port of: src/pathops/SkOpSegment.cpp#L1721-L1730 (chrome/m156)
    #[must_use]
    pub(crate) fn seg_update_opp_winding_span(&self, _seg: SegId, start: SpanId, end: SpanId) -> i32 {
        let lesser = self.span_starter(start, end);
        let mut opp_winding = self.span_opp_sum(lesser);
        let opp_span_winding = self.span_opp_sign(start, end);
        if opp_span_winding != 0
            && Self::use_inner_winding(opp_winding - opp_span_winding, opp_winding)
            && opp_winding != i32::MAX
        {
            opp_winding -= opp_span_winding;
        }
        opp_winding
    }

    /// `SkOpSegment::updateWinding(angle)`.
    #[must_use]
    pub(crate) fn seg_update_winding(&mut self, seg: SegId, angle: AngleId) -> i32 {
        let start = self.angle_start(angle);
        let end = self.angle_end(angle);
        self.seg_update_winding_span(seg, end, start)
    }

    /// `SkOpSegment::updateWindingReverse(angle)`.
    #[must_use]
    pub(crate) fn seg_update_winding_reverse(&mut self, seg: SegId, angle: AngleId) -> i32 {
        let start = self.angle_start(angle);
        let end = self.angle_end(angle);
        self.seg_update_winding_span(seg, start, end)
    }

    /// `SkOpSegment::updateOppWinding(angle)`.
    #[must_use]
    pub(crate) fn seg_update_opp_winding(&self, seg: SegId, angle: AngleId) -> i32 {
        let start = self.angle_start(angle);
        let end = self.angle_end(angle);
        self.seg_update_opp_winding_span(seg, end, start)
    }

    /// `SkOpSegment::updateOppWindingReverse(angle)`.
    #[must_use]
    pub(crate) fn seg_update_opp_winding_reverse(&self, seg: SegId, angle: AngleId) -> i32 {
        let start = self.angle_start(angle);
        let end = self.angle_end(angle);
        self.seg_update_opp_winding_span(seg, start, end)
    }

    /// `SkOpSegment::windSum(angle)`.
    // Port of: src/pathops/SkOpSegment.cpp#L1785-L1789 (chrome/m156)
    #[must_use]
    pub(crate) fn seg_wind_sum_angle(&self, angle: AngleId) -> i32 {
        let min_span = self.span_starter(self.angle_start(angle), self.angle_end(angle));
        self.span_wind_sum(min_span)
    }

    /// `SkOpSegment::findNextWinding` / `findNextOp` / `findNextXor`: the common step that
    /// finds the next segment of a chain. Returns the segment and updates `next_start` and
    /// `next_end`, or `None`.
    // Port of: src/pathops/SkOpSegment.cpp#L545-L746 (chrome/m156)
    pub(crate) fn seg_find_next_op(
        &mut self,
        seg: SegId,
        chase: &mut Vec<SpanId>,
        next_start: &mut SpanId,
        next_end: &mut SpanId,
        unsortable: &mut bool,
        simple: &mut bool,
        op: PathOp,
        xor_mi_mask: i32,
        xor_su_mask: i32,
    ) -> Option<SegId> {
        let start = *next_start;
        let end = *next_end;
        let mut step = step_from(self.span_t(start), self.span_t(end));
        let other = self.next_chase(seg, next_start, &mut step, None, None);
        *simple = other.is_some();
        if let Some(other) = other {
            let start_span = self.span_starter(start, end);
            if self.span_done(start_span) {
                return None;
            }
            self.seg_mark_done(seg, start_span);
            *next_end = if step > 0 {
                let s = self.span_up_castable(*next_start).expect("upCast");
                self.span_next(s).expect("span has next")
            } else {
                self.span_prev(*next_start).expect("span has prev")
            };
            return Some(other);
        }
        let end_near = if step > 0 {
            let s = self.span_up_castable(*next_start).expect("upCast");
            self.span_next(s).expect("span has next")
        } else {
            self.span_prev(*next_start).expect("span has prev")
        };
        let calc_winding = self.seg_compute_sum(seg, start, end_near, IncludeType::BinaryOpp);
        let sortable = calc_winding != SK_NAN32;
        if !sortable {
            *unsortable = true;
            let s = self.span_starter(start, end);
            self.seg_mark_done(seg, s);
            return None;
        }
        let Some(angle) = self.span_to_angle_toward(end, start) else {
            *unsortable = true;
            let s = self.span_starter(start, end);
            self.seg_mark_done(seg, s);
            return None;
        };
        if self.angle_unorderable(angle) {
            *unsortable = true;
            let s = self.span_starter(start, end);
            self.seg_mark_done(seg, s);
            return None;
        }
        let mut sum_mi_winding = self.seg_update_winding_span(seg, end, start);
        if sum_mi_winding == SK_MIN_S32 {
            *unsortable = true;
            let s = self.span_starter(start, end);
            self.seg_mark_done(seg, s);
            return None;
        }
        let mut sum_su_winding = self.seg_update_opp_winding_span(seg, end, start);
        if self.seg_operand(seg) {
            std::mem::swap(&mut sum_mi_winding, &mut sum_su_winding);
        }
        let mut next_angle = self.angle_next(angle).expect("angle has next");
        let mut found_angle: Option<AngleId> = None;
        let mut found_done = false;
        let mut active_count = 0;
        loop {
            let next_segment = self.angle_segment(next_angle);
            let n_start = self.angle_start(next_angle);
            let n_end = self.angle_end(next_angle);
            let active_angle = self.seg_active_op_sums(
                next_segment,
                xor_mi_mask,
                xor_su_mask,
                n_start,
                n_end,
                op,
                &mut sum_mi_winding,
                &mut sum_su_winding,
            );
            if active_angle {
                active_count += 1;
                if found_angle.is_none() || (found_done && active_count & 1 != 0) {
                    found_angle = Some(next_angle);
                    found_done = self.seg_done_angle(next_segment, next_angle);
                }
            }
            if !self.seg_done(next_segment) {
                if !active_angle {
                    let mut dummy = None;
                    let _ = self.seg_mark_and_chase_done(next_segment, n_start, n_end, Some(&mut dummy));
                }
                if let Some(last) = self.angle_last_marked(next_angle) {
                    chase.push(last);
                }
            }
            next_angle = self.angle_next(next_angle).expect("angle has next");
            if next_angle == angle {
                break;
            }
        }
        let s = self.span_starter(start, end);
        self.seg_mark_done(seg, s);
        let found = found_angle?;
        *next_start = self.angle_start(found);
        *next_end = self.angle_end(found);
        Some(self.angle_segment(found))
    }

    /// `SkOpSegment::findNextWinding(chase, nextStart, nextEnd, unsortable)`.
    // Port of: src/pathops/SkOpSegment.cpp#L652-L746 (chrome/m156)
    pub(crate) fn seg_find_next_winding(
        &mut self,
        seg: SegId,
        chase: &mut Vec<SpanId>,
        next_start: &mut SpanId,
        next_end: &mut SpanId,
        unsortable: &mut bool,
    ) -> Option<SegId> {
        let start = *next_start;
        let end = *next_end;
        let mut step = step_from(self.span_t(start), self.span_t(end));
        let other = self.next_chase(seg, next_start, &mut step, None, None);
        if let Some(other) = other {
            let start_span = self.span_starter(start, end);
            if self.span_done(start_span) {
                return None;
            }
            self.seg_mark_done(seg, start_span);
            *next_end = if step > 0 {
                let s = self.span_up_castable(*next_start).expect("upCast");
                self.span_next(s).expect("span has next")
            } else {
                self.span_prev(*next_start).expect("span has prev")
            };
            return Some(other);
        }
        let end_near = if step > 0 {
            let s = self.span_up_castable(*next_start).expect("upCast");
            self.span_next(s).expect("span has next")
        } else {
            self.span_prev(*next_start).expect("span has prev")
        };
        let calc_winding = self.seg_compute_sum(seg, start, end_near, IncludeType::UnaryWinding);
        if calc_winding == SK_NAN32 {
            *unsortable = true;
            let s = self.span_starter(start, end);
            self.seg_mark_done(seg, s);
            return None;
        }
        let Some(angle) = self.span_to_angle_toward(end, start) else {
            *unsortable = true;
            let s = self.span_starter(start, end);
            self.seg_mark_done(seg, s);
            return None;
        };
        if self.angle_unorderable(angle) {
            *unsortable = true;
            let s = self.span_starter(start, end);
            self.seg_mark_done(seg, s);
            return None;
        }
        let mut sum_winding = self.seg_update_winding_span(seg, end, start);
        let mut next_angle = self.angle_next(angle).expect("angle has next");
        let mut found_angle: Option<AngleId> = None;
        let mut found_done = false;
        let mut active_count = 0;
        loop {
            let next_segment = self.angle_segment(next_angle);
            let n_start = self.angle_start(next_angle);
            let n_end = self.angle_end(next_angle);
            let active_angle = self.seg_active_winding_sum(n_start, n_end, &mut sum_winding);
            if active_angle {
                active_count += 1;
                if found_angle.is_none() || (found_done && active_count & 1 != 0) {
                    found_angle = Some(next_angle);
                    found_done = self.seg_done_angle(next_segment, next_angle);
                }
            }
            if !self.seg_done(next_segment) {
                if !active_angle {
                    let mut dummy = None;
                    let _ = self.seg_mark_and_chase_done(next_segment, n_start, n_end, Some(&mut dummy));
                }
                if let Some(last) = self.angle_last_marked(next_angle) {
                    chase.push(last);
                }
            }
            next_angle = self.angle_next(next_angle).expect("angle has next");
            if next_angle == angle {
                break;
            }
        }
        let s = self.span_starter(start, end);
        self.seg_mark_done(seg, s);
        let found = found_angle?;
        *next_start = self.angle_start(found);
        *next_end = self.angle_end(found);
        Some(self.angle_segment(found))
    }

    /// `SkOpSegment::findNextXor(nextStart, nextEnd, unsortable)`.
    // Port of: src/pathops/SkOpSegment.cpp#L748-L817 (chrome/m156)
    pub(crate) fn seg_find_next_xor(
        &mut self,
        seg: SegId,
        next_start: &mut SpanId,
        next_end: &mut SpanId,
        unsortable: &mut bool,
    ) -> Option<SegId> {
        let start = *next_start;
        let end = *next_end;
        let mut step = step_from(self.span_t(start), self.span_t(end));
        let other = self.next_chase(seg, next_start, &mut step, None, None);
        if let Some(other) = other {
            let start_span = self.span_starter(start, end);
            if self.span_done(start_span) {
                return None;
            }
            self.seg_mark_done(seg, start_span);
            *next_end = if step > 0 {
                let s = self.span_up_castable(*next_start).expect("upCast");
                self.span_next(s).expect("span has next")
            } else {
                self.span_prev(*next_start).expect("span has prev")
            };
            return Some(other);
        }
        let Some(angle) = self.span_to_angle_toward(end, start) else {
            *unsortable = true;
            let s = self.span_starter(start, end);
            self.seg_mark_done(seg, s);
            return None;
        };
        if self.angle_unorderable(angle) {
            *unsortable = true;
            let s = self.span_starter(start, end);
            self.seg_mark_done(seg, s);
            return None;
        }
        let mut next_angle = self.angle_next(angle).expect("angle has next");
        let mut found_angle: Option<AngleId> = None;
        let mut found_done = false;
        let mut active_count = 0;
        loop {
            let next_segment = self.angle_segment(next_angle);
            active_count += 1;
            if found_angle.is_none() || (found_done && active_count & 1 != 0) {
                found_angle = Some(next_angle);
                found_done = self.seg_done_angle(next_segment, next_angle);
                if !found_done {
                    break;
                }
            }
            next_angle = self.angle_next(next_angle).expect("angle has next");
            if next_angle == angle {
                break;
            }
        }
        let s = self.span_starter(start, end);
        self.seg_mark_done(seg, s);
        let found = found_angle?;
        *next_start = self.angle_start(found);
        *next_end = self.angle_end(found);
        Some(self.angle_segment(found))
    }

    /// `SkOpSegment::undoneSpan()`.
    // Port of: src/pathops/SkOpSegment.cpp#L1709-L1719 (chrome/m156)
    #[must_use]
    pub(crate) fn seg_undone_span(&self, seg: SegId) -> Option<SpanId> {
        let mut span = self.seg_head(seg);
        loop {
            let next = self.span_next(span)?;
            if !self.span_done(span) {
                return Some(span);
            }
            if self.span_final(next) {
                return None;
            }
            span = next;
        }
    }

    /// `SkOpSegment::markAllDone` / `ClearVisited(span)`: resets the visited flag of every
    /// segment that has a point in `span`'s ring.
    // Port of: src/pathops/SkOpSegment.cpp#L1141-L1151 (chrome/m156)
    pub(crate) fn seg_clear_visited(&mut self, span: SpanId) {
        let mut span = span;
        loop {
            let stop = self.span_ptt(span);
            let mut ptt = stop;
            loop {
                ptt = self.ptt_next(ptt);
                if ptt == stop {
                    break;
                }
                let opp = self.ptt_segment(ptt);
                self.seg_reset_visited(opp);
            }
            if self.span_final(span) {
                break;
            }
            match self.span_up_castable(span).and_then(|s| self.span_next(s)) {
                Some(next) => span = next,
                None => break,
            }
        }
    }

    /// `SkOpSegment::isClose(t, opp)`: `t` lies on a segment `opp` within rounding distance.
    // Port of: src/pathops/SkOpSegment.cpp#L840-L853 (chrome/m156)
    #[must_use]
    pub(crate) fn seg_is_close(&self, seg: SegId, t: f64, opp: SegId) -> bool {
        let c_pt = self.seg_d_pt_at_t(seg, t);
        let dxdy = self.seg_d_slope_at_t(seg, t);
        let perp = DLine::new([c_pt, DPoint::new(c_pt.x + dxdy.y, c_pt.y - dxdy.x)]);
        let mut i = Intersections::default();
        let s = &self.segments[opp.0];
        curve_intersect_ray(s.verb, &s.pts, s.weight, &perp, &mut i);
        let used = i.used();
        for index in 0..used {
            if c_pt.roughly_equal(i.pt(index)) {
                return true;
            }
        }
        false
    }

    /// `SkOpSegment::distSq(t, oppAngle)`.
    // Port of: src/pathops/SkOpSegment.cpp#L515-L543 (chrome/m156)
    #[must_use]
    pub(crate) fn seg_dist_sq(&self, seg: SegId, t: f64, opp_angle: AngleId) -> f64 {
        let test_pt = self.seg_d_pt_at_t(seg, t);
        let slope = self.seg_d_slope_at_t(seg, t);
        let perp = DLine::new([
            test_pt,
            DPoint::new(test_pt.x + slope.y, test_pt.y - slope.x),
        ]);
        let mut i = Intersections::default();
        let opp_seg = self.angle_segment(opp_angle);
        let s = &self.segments[opp_seg.0];
        curve_intersect_ray(s.verb, &s.pts, s.weight, &perp, &mut i);
        let mut closest_dist_sq = f64::INFINITY;
        let start_t = self.span_t(self.angle_start(opp_angle));
        let end_t = self.span_t(self.angle_end(opp_angle));
        for index in 0..i.used() {
            if !between(start_t, i.t(0, index), end_t) {
                continue;
            }
            let test_dist_sq = test_pt.distance_squared(i.pt(index));
            if closest_dist_sq > test_dist_sq {
                closest_dist_sq = test_dist_sq;
            }
        }
        closest_dist_sq
    }

    /// `SkOpSegment::sortAngles()`.
    // Port of: src/pathops/SkOpSegment.cpp#L1550-L1623 (chrome/m156)
    pub(crate) fn seg_sort_angles(&mut self, seg: SegId) -> bool {
        let tail = self.seg_tail(seg);
        let mut span = self.seg_head(seg);
        loop {
            let from_angle = self.span_from_angle(span);
            let to_angle = if self.span_final(span) {
                None
            } else {
                self.span_to_angle(span)
            };
            if from_angle.is_none() && to_angle.is_none() {
                if span == tail || self.span_final(span) {
                    break;
                }
                span = self.span_up_castable(span).and_then(|s| self.span_next(s)).expect("span has next");
                continue;
            }
            let mut wrote_after_header = false;
            let mut base_angle = from_angle;
            if let (Some(from), Some(to)) = (from_angle, to_angle) {
                wrote_after_header = true;
                if !self.angle_insert(from, to) {
                    return false;
                }
            } else if from_angle.is_none() {
                base_angle = to_angle;
            }
            let base = base_angle.expect("base angle");
            let stop = self.span_ptt(span);
            let mut ptt = stop;
            let mut safety_net = 1000;
            loop {
                safety_net -= 1;
                if safety_net == 0 {
                    return false;
                }
                let o_span = self.ptts[ptt.0].span;
                if o_span != span {
                    let o_angle = self.span_from_angle(o_span);
                    if let Some(o_angle) = o_angle {
                        if !wrote_after_header {
                            wrote_after_header = true;
                        }
                        if !self.angle_loop_contains(o_angle, base) {
                            self.angle_insert(base, o_angle);
                        }
                    }
                    if !self.span_final(o_span) {
                        if let Some(o_angle) = self.span_to_angle(o_span) {
                            if !wrote_after_header {
                                wrote_after_header = true;
                            }
                            if !self.angle_loop_contains(o_angle, base) {
                                self.angle_insert(base, o_angle);
                            }
                        }
                    }
                }
                ptt = self.ptts[ptt.0].next;
                if ptt == stop {
                    break;
                }
            }
            if self.angle_loop_count(base) == 1 {
                self.span_set_from_angle(span, None);
                if to_angle.is_some() {
                    self.span_set_to_angle(span, None);
                }
            }
            if self.span_final(span) {
                break;
            }
            span = self.span_up_castable(span).and_then(|s| self.span_next(s)).expect("span has next");
        }
        true
    }

    /// `SkOpSegment::spanToAngle(start, end)`.
    #[must_use]
    pub(crate) fn seg_span_to_angle(&self, start: SpanId, end: SpanId) -> Option<AngleId> {
        self.span_to_angle_toward(start, end)
    }

    /// `SkOpSegment::markAndChaseWinding` helper: the `markAngle` result span.
    #[must_use]
    pub(crate) fn seg_fixup_collapsed_coin(&self, _seg: SegId) -> bool {
        false
    }

    /// `SkOpSegment::spansNearby(refSpan, checkSpan, found)`. `None` is Skia's `return false`.
    // Port of: src/pathops/SkOpSegment.cpp#L1371-L1440 (chrome/m156)
    pub(crate) fn seg_spans_nearby(&self, ref_span: SpanId, check_span: SpanId, found: &mut bool) -> Option<()> {
        let ref_head = self.span_ptt(ref_span);
        let check_head = self.span_ptt(check_span);
        if !DPoint::way_roughly_equal(self.ptts[ref_head.0].pt, self.ptts[check_head.0].pt) {
            *found = false;
            return Some(());
        }
        let mut dist_sq_best = f32::MAX;
        let mut ref_best: Option<PtTId> = None;
        let mut check_best: Option<PtTId> = None;
        let mut ref_ptt = ref_head;
        loop {
            // do { ... } while ((ref = ref->next()) != refHead);
            let mut done_distance = false;
            'next_ref: {
                if self.ptts[ref_ptt.0].deleted {
                    break 'next_ref; // continue
                }
                while self.ptt_pt_already_seen(ref_ptt, ref_head) {
                    ref_ptt = self.ptts[ref_ptt.0].next;
                    if ref_ptt == ref_head {
                        done_distance = true; // goto doneCheckingDistance
                        break 'next_ref;
                    }
                }
                let mut check = check_head;
                let ref_seg = self.ptt_segment(ref_ptt);
                let mut escape_hatch = 100; // defend against infinite loops
                loop {
                    // do { ... } while ((check = check->next()) != checkHead);
                    if !self.ptts[check.0].deleted {
                        while self.ptt_pt_already_seen(check, check_head) {
                            check = self.ptts[check.0].next;
                            if check == check_head {
                                break 'next_ref; // goto nextRef
                            }
                        }
                        let distance = dist_sq(self.ptts[ref_ptt.0].pt, self.ptts[check.0].pt);
                        if dist_sq_best > distance
                            && (ref_seg != self.ptt_segment(check)
                                || !self.seg_pts_disjoint_ptt(ref_seg, ref_ptt, check))
                        {
                            dist_sq_best = distance;
                            ref_best = Some(ref_ptt);
                            check_best = Some(check);
                        }
                        escape_hatch -= 1;
                        if escape_hatch <= 0 {
                            return None;
                        }
                    }
                    check = self.ptts[check.0].next;
                    if check == check_head {
                        break;
                    }
                }
            }
            if done_distance {
                break;
            }
            ref_ptt = self.ptts[ref_ptt.0].next;
            if ref_ptt == ref_head {
                break;
            }
        }
        // doneCheckingDistance:
        *found = match (ref_best, check_best) {
            (Some(rb), Some(cb)) => {
                let cseg = self.ptt_segment(cb);
                let t = self.ptts[cb.0].t;
                let pt = self.ptts[cb.0].pt;
                self.seg_match(self.ptt_segment(rb), rb, cseg, t, pt)
            }
            _ => false,
        };
        Some(())
    }

    /// `SkOpPtT::ptAlreadySeen` with the owning point's segment: `ptsDisjoint(*ref, *check)`.
    #[must_use]
    fn seg_pts_disjoint_ptt(&self, seg: SegId, a: PtTId, b: PtTId) -> bool {
        self.seg_pts_disjoint(
            seg,
            self.ptts[a.0].t,
            self.ptts[a.0].pt,
            self.ptts[b.0].t,
            self.ptts[b.0].pt,
        )
    }
    /// `SkOpSegment::testForCoincidence(priorPtT, ptT, prior, spanBase, opp)`.
    // Port of: src/pathops/SkOpSegment.cpp#L1672-L1707 (chrome/m156)
    #[must_use]
    pub(crate) fn seg_test_for_coincidence(
        &self,
        seg: SegId,
        prior_ptt: PtTId,
        ptt: PtTId,
        prior: SpanId,
        span_base: SpanId,
        opp: SegId,
    ) -> bool {
        let mid_t = (self.span_t(prior) + self.span_t(span_base)) / 2.0;
        let mid_pt = self.seg_pt_at_t(seg, mid_t);
        let mut coincident = true;
        let mid_d = DPoint::from_sk_point(mid_pt);
        if !approx_equal_points(self.ptts[prior_ptt.0].pt, mid_pt)
            && !approx_equal_points(self.ptts[ptt.0].pt, mid_pt)
        {
            if self.ptt_span(prior_ptt) == self.ptt_span(ptt) {
                return false;
            }
            coincident = false;
            let mut curve_part = DCurveBuf::default();
            let verb = self.seg_verb(seg);
            self.seg_sub_divide(seg, prior, span_base, &mut curve_part);
            let dxdy = curve_dd_slope_at_t(verb, &curve_part, 0.5);
            let part_mid_pt = curve_dd_point_at_t(verb, &curve_part, 0.5);
            let ray = DLine::new([
                mid_d,
                DPoint::new(part_mid_pt.x + dxdy.y, part_mid_pt.y - dxdy.x),
            ]);
            let mut opp_part = DCurveBuf::default();
            self.seg_sub_divide(
                opp,
                self.ptt_span(prior_ptt),
                self.ptt_span(ptt),
                &mut opp_part,
            );
            let mut i = Intersections::default();
            curve_d_intersect_ray(self.seg_verb(opp), &opp_part, &ray, &mut i);
            for index in 0..i.used() {
                if !between(0.0, i.t(0, index), 1.0) {
                    continue;
                }
                let opp_pt = i.pt(index);
                if opp_pt.approximately_d_equal(mid_d) {
                    coincident = true;
                }
            }
        }
        coincident
    }

    /// `SkOpSegment::missingCoincidence()`.
    // Port of: src/pathops/SkOpSegment.cpp#L1162-L1270 (chrome/m156)
    pub(crate) fn seg_missing_coincidence(&mut self, seg: SegId) -> bool {
        if self.seg_done(seg) {
            return false;
        }
        let mut prior: Option<SpanId> = None;
        let mut span_base = self.seg_head(seg);
        let mut result = false;
        let mut safety_net = 1000;
        loop {
            let span_stop = self.span_ptt(span_base);
            let mut ptt = span_stop;
            loop {
                ptt = self.ptt_next(ptt);
                if ptt == span_stop {
                    break;
                }
                safety_net -= 1;
                if safety_net == 0 {
                    return false;
                }
                if self.ptts[ptt.0].deleted {
                    continue;
                }
                let opp = self.ptt_segment(ptt);
                if self.seg_done(opp) {
                    continue;
                }
                if !self.seg_visited(opp) {
                    continue;
                }
                if span_base == self.seg_head(seg) {
                    continue;
                }
                if self.ptt_segment(ptt) == seg {
                    continue;
                }
                if let Some(span) = self.span_up_castable(span_base) {
                    if self.span_contains_coincidence_seg(span, opp) {
                        continue;
                    }
                }
                if self.span_contains_coin_end_seg(span_base, opp) {
                    continue;
                }
                let mut prior_ptt: PtTId = span_stop;
                let mut prior_opp: Option<SegId> = None;
                let mut prior_test = self.span_prev(span_base);
                while prior_opp.is_none() {
                    let Some(pt_span) = prior_test else {
                        break;
                    };
                    let prior_stop = self.span_ptt(pt_span);
                    prior_ptt = prior_stop;
                    loop {
                        prior_ptt = self.ptt_next(prior_ptt);
                        if prior_ptt == prior_stop {
                            break;
                        }
                        if self.ptts[prior_ptt.0].deleted {
                            continue;
                        }
                        let segment = self.ptt_segment(prior_ptt);
                        if segment == opp {
                            prior = Some(pt_span);
                            prior_opp = Some(opp);
                            break;
                        }
                    }
                    prior_test = self.span_prev(pt_span);
                }
                if prior_opp.is_none() {
                    continue;
                }
                if prior_ptt == ptt {
                    continue;
                }
                let prior_span = prior.expect("prior set with prior_opp");
                let mut opp_start = self.span_ptt(prior_span);
                let mut opp_end = self.span_ptt(span_base);
                let mut p1 = prior_ptt;
                let mut p2 = ptt;
                let swapped = self.ptts[p1.0].t > self.ptts[p2.0].t;
                if swapped {
                    std::mem::swap(&mut p1, &mut p2);
                    std::mem::swap(&mut opp_start, &mut opp_end);
                }
                let root_prior = self.span_ptt(self.ptt_span(p1));
                let root_ptt = self.span_ptt(self.ptt_span(p2));
                let root_opp_start = self.span_ptt(self.ptt_span(opp_start));
                let root_opp_end = self.span_ptt(self.ptt_span(opp_end));
                if self.coin_contains(root_prior, root_ptt, root_opp_start, root_opp_end) {
                    // goto swapBack
                    continue;
                }
                if self.seg_test_for_coincidence(seg, root_prior, root_ptt, prior_span, span_base, opp) {
                    if !self.coin_extend(root_prior, root_ptt, root_opp_start, root_opp_end) {
                        self.coin_add(root_prior, root_ptt, root_opp_start, root_opp_end);
                    }
                    result = true;
                }
            }
            match self.span_up_castable(span_base) {
                Some(span) => match self.span_next(span) {
                    Some(next) => span_base = next,
                    None => break,
                },
                None => break,
            }
        }
        self.clear_visited_from_head(seg);
        result
    }

    /// `ClearVisited(&fHead)` for `seg`.
    fn clear_visited_from_head(&mut self, seg: SegId) {
        let head = self.seg_head(seg);
        self.seg_clear_visited(head);
    }

    /// `SkOpSegment::moveMultiples()`.
    // Port of: src/pathops/SkOpSegment.cpp#L1272-L1369 (chrome/m156)
    pub(crate) fn seg_move_multiples(&mut self, seg: SegId) -> bool {
        let mut test = self.seg_head(seg);
        loop {
            'check_next_span: {
                let add_count = self.span_adds_count(test);
                if add_count <= 1 {
                    break 'check_next_span;
                }
                let start_ptt = self.span_ptt(test);
                let mut test_ptt = start_ptt;
                let mut safety_hatch = 1000;
                loop {
                    // do { ... } while ((testPtT = testPtT->next()) != startPtT);
                    'next_test_ptt: {
                        safety_hatch -= 1;
                        if safety_hatch == 0 {
                            return false;
                        }
                        let opp_span = self.ptt_span(test_ptt); // iterate through all spans
                        if self.span_adds_count(opp_span) == add_count {
                            break 'next_test_ptt;
                        }
                        if self.span_deleted(opp_span) {
                            break 'next_test_ptt;
                        }
                        let opp_segment = self.span_segment(opp_span);
                        if opp_segment == seg {
                            break 'next_test_ptt;
                        }
                        let mut opp_prev = opp_span;
                        let mut opp_first = opp_span;
                        while let Some(prev) = self.span_prev(opp_prev) {
                            opp_prev = prev;
                            if !roughly_equal(self.span_t(opp_prev), self.span_t(opp_span)) {
                                break;
                            }
                            if self.span_adds_count(opp_prev) == add_count {
                                continue;
                            }
                            if self.span_deleted(opp_prev) {
                                continue;
                            }
                            opp_first = opp_prev;
                        }
                        let mut opp_next = opp_span;
                        let mut opp_last = opp_span;
                        loop {
                            if self.span_final(opp_next) {
                                break;
                            }
                            let Some(next) = self.span_next(opp_next) else {
                                break;
                            };
                            opp_next = next;
                            if !roughly_equal(self.span_t(opp_next), self.span_t(opp_span)) {
                                break;
                            }
                            if self.span_adds_count(opp_next) == add_count {
                                continue;
                            }
                            if self.span_deleted(opp_next) {
                                continue;
                            }
                            opp_last = opp_next;
                        }
                        if opp_first == opp_last {
                            break 'next_test_ptt;
                        }
                        let mut opp_test = opp_first;
                        loop {
                            // do { ... } while (oppTest != oppLast && (oppTest = next));
                            'try_next_span: {
                                if opp_test == opp_span {
                                    break 'try_next_span;
                                }
                                let opp_start_ptt = self.span_ptt(opp_test);
                                let mut opp_ptt = opp_start_ptt;
                                loop {
                                    opp_ptt = self.ptt_next(opp_ptt);
                                    if opp_ptt == opp_start_ptt {
                                        break;
                                    }
                                    let opp_ptt_segment = self.ptt_segment(opp_ptt);
                                    if opp_ptt_segment == seg {
                                        break 'try_next_span;
                                    }
                                    let mut match_ptt = start_ptt;
                                    let mut found = false;
                                    loop {
                                        if self.ptt_segment(match_ptt) == opp_ptt_segment {
                                            found = true;
                                            break;
                                        }
                                        match_ptt = self.ptt_next(match_ptt);
                                        if match_ptt == start_ptt {
                                            break;
                                        }
                                    }
                                    if !found {
                                        break 'try_next_span;
                                    }
                                    // foundMatch: merge oppTest and oppSpan
                                    self.span_merge_matches(opp_test, opp_span);
                                    self.span_add_opp(opp_test, opp_span);
                                    break 'check_next_span;
                                }
                            }
                            if opp_test == opp_last {
                                break;
                            }
                            match self.span_next(opp_test) {
                                Some(next) => opp_test = next,
                                None => break,
                            }
                        }
                    }
                    test_ptt = self.ptt_next(test_ptt);
                    if test_ptt == start_ptt {
                        break;
                    }
                }
            }
            if self.span_final(test) {
                break;
            }
            match self.span_next(test) {
                Some(next) => test = next,
                None => break,
            }
        }
        true
    }

    /// `SkOpSegment::moveNearby()`.
    // Port of: src/pathops/SkOpSegment.cpp#L1442-L1495 (chrome/m156)
    pub(crate) fn seg_move_nearby(&mut self, seg: SegId) -> bool {
        let mut span_base = self.seg_head(seg);
        let mut escape_hatch = 9999; // the largest count for a regular test is 50
        loop {
            let head_ptt = self.span_ptt(span_base);
            let mut ptt = head_ptt;
            loop {
                ptt = self.ptt_next(ptt);
                if ptt == head_ptt {
                    break;
                }
                escape_hatch -= 1;
                if escape_hatch == 0 {
                    return false;
                }
                let test = self.ptts[ptt.0].span;
                if self.ptt_segment(ptt) == seg
                    && !self.ptts[ptt.0].deleted
                    && test != span_base
                    && self.span_ptt(test) == ptt
                {
                    if self.span_final(test) {
                        if span_base == self.seg_head(seg) {
                            self.seg_clear_all(seg);
                            return true;
                        }
                        self.span_release(span_base, ptt);
                    } else if self.span_prev(test).is_some() {
                        self.span_release(test, head_ptt);
                    }
                    break;
                }
            }
            span_base = self
                .span_up_castable(span_base)
                .and_then(|s| self.span_next(s))
                .expect("span has next");
            if self.span_final(span_base) {
                break;
            }
        }
        span_base = self.seg_head(seg);
        loop {
            let test = self.span_next(span_base).expect("span has next");
            let mut found = false;
            if self.spans_nearby_ok(span_base, test, &mut found).is_none() {
                return false;
            }
            if found {
                if self.span_final(test) {
                    if self.span_prev(span_base).is_some() {
                        self.span_merge(test, span_base);
                    } else {
                        self.seg_clear_all(seg);
                        return true;
                    }
                } else {
                    self.span_merge(span_base, test);
                }
            }
            span_base = test;
            if self.span_final(span_base) {
                break;
            }
        }
        true
    }

    /// `SkOpSegment::spansNearby` as called by `moveNearby`, where `None` means `return false`.
    fn spans_nearby_ok(&self, ref_span: SpanId, check_span: SpanId, found: &mut bool) -> Option<()> {
        self.seg_spans_nearby(ref_span, check_span, found)
    }

}

/// `SK_NaN32`.
pub(crate) const SK_NAN32: i32 = i32::MIN + 1;

/// `SkPointPriv::DistanceToSqd(a, b)`: the squared distance in `double` after subtraction in
/// `float`, as Skia computes it.
// Port of: src/core/SkPointPriv.h (SkPointPriv::DistanceToSqd) (chrome/m156)
#[must_use]
pub(crate) fn dist_sq(a: Point, b: Point) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

/// `SkDPoint::ApproximatelyEqual(SkPoint, SkPoint)`.
// Port of: src/pathops/SkPathOpsPoint.h (SkDPoint::ApproximatelyEqual) (chrome/m156)
#[must_use]
pub(crate) fn approx_equal_points(a: Point, b: Point) -> bool {
    DPoint::approximately_equal_points(a, b)
}

/// `SkDPoint::roughlyEqual` helper for `Point` values, used where Skia passes `SkPoint`s.
#[must_use]
pub(crate) fn roughly_equal_points(a: Point, b: Point) -> bool {
    roughly_equal(f64::from(a.x), f64::from(b.x)) && roughly_equal(f64::from(a.y), f64::from(b.y))
}
