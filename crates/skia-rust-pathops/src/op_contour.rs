// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkOpContour.h, src/pathops/SkOpContour.cpp
// (SkOpContour, SkOpContourHead, SkOpContourBuilder).

//! A closed sub-path of the op graph (`SkOpContour`): a chain of segments with the operand
//! and fill flags of the path it came from.
//!
//! Contour 0 of an [`OpState`] is Skia's `SkOpContourHead`; the other contours form a list
//! after it. A contour's first segment is `head`; its segments form a list through
//! `Segment::next`, ending at `tail`.

use skia_rust_core::path::Verb;
use skia_rust_core::point::Point;

use crate::op_state::{ContourId, OpState, SegId};
use crate::rect::Bounds;

/// `SkOpContour`: a list of segments that form one closed contour of the input.
// Port of: src/pathops/SkOpContour.h#L24-L30 (chrome/m156)
#[doc(alias = "SkOpContour")]
#[derive(Clone, Debug, Default)]
pub(crate) struct Contour {
    /// `SkOpSegment fHead`: the first segment (`None` until the first segment is appended).
    pub(crate) head: Option<SegId>,
    /// `SkOpSegment* fTail`.
    pub(crate) tail: Option<SegId>,
    /// `SkOpContour* fNext`.
    pub(crate) next: Option<ContourId>,
    /// `SkPathOpsBounds fBounds`.
    pub(crate) bounds: Bounds,
    /// `int fCcw`.
    pub(crate) ccw: i32,
    /// `int fCount`: number of segments.
    pub(crate) count: i32,
    /// `int fFirstSorted`.
    pub(crate) first_sorted: i32,
    /// `bool fDone`: set by the find-top-segment search.
    pub(crate) done: bool,
    /// `bool fOperand`: true for the second argument to a binary operator.
    pub(crate) operand: bool,
    /// `bool fReverse`: true if the contour is reverse written (used only by fix winding).
    pub(crate) reverse: bool,
    /// `bool fXor`: set if the original path had even-odd fill.
    pub(crate) xor: bool,
    /// `bool fOppXor`: set if the opposite path had even-odd fill.
    pub(crate) opp_xor: bool,
}

/// `SkOpContourBuilder`: merges a line followed by its reverse, and appends curves to a
/// contour.
// Port of: src/pathops/SkOpContour.h#L400-L430 (chrome/m156)
#[doc(alias = "SkOpContourBuilder")]
#[derive(Clone, Copy, Debug)]
pub(crate) struct ContourBuilder {
    /// `SkOpContour* fContour`.
    pub(crate) contour: ContourId,
    /// `SkPoint fLastLine[2]`.
    last_line: [Point; 2],
    /// `bool fLastIsLine`.
    last_is_line: bool,
}

/// `SkPathOpsBounds` from a point list (`SkRect::setBounds`).
// Port of: src/core/SkRect.cpp (SkRect::setBounds) (chrome/m156)
#[must_use]
pub(crate) fn bounds_of_points(pts: &[Point]) -> Bounds {
    let mut b = Bounds {
        left: pts[0].x,
        top: pts[0].y,
        right: pts[0].x,
        bottom: pts[0].y,
    };
    for pt in &pts[1..] {
        if pt.x < b.left {
            b.left = pt.x;
        }
        if pt.y < b.top {
            b.top = pt.y;
        }
        if pt.x > b.right {
            b.right = pt.x;
        }
        if pt.y > b.bottom {
            b.bottom = pt.y;
        }
    }
    b
}

impl OpState {
    /// `SkOpContour::first()`: the first segment.
    #[must_use]
    pub(crate) fn contour_first(&self, c: ContourId) -> Option<SegId> {
        self.contours[c.0].head
    }

    /// `SkOpContour::count()`.
    #[must_use]
    pub(crate) fn contour_count(&self, c: ContourId) -> i32 {
        self.contours[c.0].count
    }

    /// `SkOpContour::next()`.
    #[must_use]
    pub(crate) fn contour_next(&self, c: ContourId) -> Option<ContourId> {
        self.contours[c.0].next
    }

    /// `SkOpContour::setNext(contour)`.
    pub(crate) fn contour_set_next(&mut self, c: ContourId, next: Option<ContourId>) {
        self.contours[c.0].next = next;
    }

    /// `SkOpContour::operand()`.
    #[must_use]
    pub(crate) fn contour_operand(&self, c: ContourId) -> bool {
        self.contours[c.0].operand
    }

    /// `SkOpContour::setOperand(isOp)`.
    pub(crate) fn contour_set_operand(&mut self, c: ContourId, is_op: bool) {
        self.contours[c.0].operand = is_op;
    }

    /// `SkOpContour::oppXor()`.
    #[must_use]
    pub(crate) fn contour_opp_xor(&self, c: ContourId) -> bool {
        self.contours[c.0].opp_xor
    }

    /// `SkOpContour::setOppXor(isOppXor)`.
    pub(crate) fn contour_set_opp_xor(&mut self, c: ContourId, is_opp_xor: bool) {
        self.contours[c.0].opp_xor = is_opp_xor;
    }

    /// `SkOpContour::isXor()`.
    #[must_use]
    pub(crate) fn contour_xor(&self, c: ContourId) -> bool {
        self.contours[c.0].xor
    }

    /// `SkOpContour::setXor(isXor)`.
    pub(crate) fn contour_set_xor(&mut self, c: ContourId, is_xor: bool) {
        self.contours[c.0].xor = is_xor;
    }

    /// `SkOpContour::reversed()`.
    #[must_use]
    pub(crate) fn contour_reversed(&self, c: ContourId) -> bool {
        self.contours[c.0].reverse
    }

    /// `SkOpContour::setReverse()`.
    pub(crate) fn contour_set_reverse(&mut self, c: ContourId) {
        self.contours[c.0].reverse = true;
    }

    /// `SkOpContour::isCcw()`.
    #[must_use]
    pub(crate) fn contour_is_ccw(&self, c: ContourId) -> i32 {
        self.contours[c.0].ccw
    }

    /// `SkOpContour::setCcw(ccw)`.
    pub(crate) fn contour_set_ccw(&mut self, c: ContourId, ccw: i32) {
        self.contours[c.0].ccw = ccw;
    }

    /// `SkOpContour::done()`.
    #[must_use]
    pub(crate) fn contour_done(&self, c: ContourId) -> bool {
        self.contours[c.0].done
    }

    /// `SkOpContour::bounds()`.
    #[must_use]
    pub(crate) fn contour_bounds(&self, c: ContourId) -> Bounds {
        self.contours[c.0].bounds
    }

    /// `SkOpContour::init(globalState, operand, isXor)`.
    // Port of: src/pathops/SkOpContour.h#L108-L113 (chrome/m156)
    pub(crate) fn contour_init(&mut self, c: ContourId, operand: bool, is_xor: bool) {
        let contour = &mut self.contours[c.0];
        contour.operand = operand;
        contour.xor = is_xor;
    }

    /// `SkOpContour::resetReverse()` for the list from `c` on.
    // Port of: src/pathops/SkOpContour.h#L200-L210 (chrome/m156)
    pub(crate) fn contour_reset_reverse(&mut self, c: ContourId) {
        let mut next = Some(c);
        while let Some(n) = next {
            if self.contours[n.0].count != 0 {
                self.contours[n.0].ccw = -1;
                self.contours[n.0].reverse = false;
            }
            next = self.contours[n.0].next;
        }
    }

    /// `SkOpContour::appendSegment()`: adds an empty segment at the end of the contour.
    // Port of: src/pathops/SkOpContour.h#L36-L46 (chrome/m156)
    fn contour_append_segment(&mut self, c: ContourId) -> SegId {
        let old_count = self.contours[c.0].count;
        self.contours[c.0].count += 1;
        let result = self.alloc_segment();
        if old_count == 0 {
            self.contours[c.0].head = Some(result);
        }
        let tail = self.contours[c.0].tail;
        self.segments[result.0].prev = tail;
        if let Some(t) = tail {
            self.segments[t.0].next = Some(result);
        }
        self.contours[c.0].tail = Some(result);
        result
    }

    /// `SkOpContour::addConic(pts, weight)`.
    // Port of: src/pathops/SkOpContour.h#L29-L31 (chrome/m156)
    pub(crate) fn contour_add_conic(&mut self, c: ContourId, pts: [Point; 3], weight: f32) -> SegId {
        let seg = self.contour_append_segment(c);
        self.seg_init(seg, &pts, weight, c, Verb::Conic);
        self.seg_set_conic_bounds(seg);
        seg
    }

    /// `SkOpContour::addCubic(pts)`.
    // Port of: src/pathops/SkOpContour.h#L32-L34 (chrome/m156)
    pub(crate) fn contour_add_cubic(&mut self, c: ContourId, pts: [Point; 4]) -> SegId {
        let seg = self.contour_append_segment(c);
        self.seg_init(seg, &pts, 1.0, c, Verb::Cubic);
        self.seg_set_poly_bounds(seg);
        seg
    }

    /// `SkOpContour::addLine(pts)`.
    // Port of: src/pathops/SkOpContour.h#L35-L37 (chrome/m156)
    pub(crate) fn contour_add_line(&mut self, c: ContourId, pts: [Point; 2]) -> SegId {
        let seg = self.contour_append_segment(c);
        self.seg_init(seg, &pts, 1.0, c, Verb::Line);
        self.segments[seg.0].bounds = bounds_of_points(&pts);
        seg
    }

    /// `SkOpContour::addQuad(pts)`.
    // Port of: src/pathops/SkOpContour.h#L38-L40 (chrome/m156)
    pub(crate) fn contour_add_quad(&mut self, c: ContourId, pts: [Point; 3]) -> SegId {
        let seg = self.contour_append_segment(c);
        self.seg_init(seg, &pts, 1.0, c, Verb::Quad);
        self.seg_set_poly_bounds(seg);
        seg
    }

    /// `SkOpContour::setBounds()`: the union of the segment bounds.
    // Port of: src/pathops/SkOpContour.h#L299-L306 (chrome/m156)
    pub(crate) fn contour_set_bounds(&mut self, c: ContourId) {
        let mut segment = self.contours[c.0].head.expect("contour has a segment");
        let mut bounds = self.segments[segment.0].bounds;
        while let Some(next) = self.segments[segment.0].next {
            segment = next;
            bounds.add_bounds(&self.segments[segment.0].bounds);
        }
        self.contours[c.0].bounds = bounds;
    }

    /// `SkOpContour::complete()`.
    // Port of: src/pathops/SkOpContour.h#L51-L53 (chrome/m156)
    pub(crate) fn contour_complete(&mut self, c: ContourId) {
        self.contour_set_bounds(c);
    }

    /// Calls `f` on each segment of `c`, in order, stopping early when `f` returns `false`.
    /// Mirrors the `do { ... } while ((segment = segment->next()))` loops of `SkOpContour`.
    fn contour_each_segment(&mut self, c: ContourId, mut f: impl FnMut(&mut Self, SegId) -> bool) -> bool {
        let Some(mut segment) = self.contours[c.0].head else {
            return true;
        };
        loop {
            if !f(self, segment) {
                return false;
            }
            match self.segments[segment.0].next {
                Some(next) => segment = next,
                None => return true,
            }
        }
    }

    /// `SkOpContour::calcAngles()`.
    // Port of: src/pathops/SkOpContour.h#L56-L60 (chrome/m156)
    pub(crate) fn contour_calc_angles(&mut self, c: ContourId) {
        self.contour_each_segment(c, |state, seg| {
            state.seg_calc_angles(seg);
            true
        });
    }

    /// `SkOpContour::joinSegments()`.
    // Port of: src/pathops/SkOpContour.h#L165-L172 (chrome/m156)
    pub(crate) fn contour_join_segments(&mut self, c: ContourId) {
        let head = self.contours[c.0].head.expect("contour has a segment");
        let mut segment = head;
        loop {
            let next = self.segments[segment.0].next;
            let start = next.unwrap_or(head);
            // segment->joinEnds(next ? next : &fHead)
            let tail_ptt = self.span_ptt(self.segments[segment.0].tail);
            let start_ptt = self.span_ptt(self.segments[start.0].head);
            self.ptt_add_opp(tail_ptt, start_ptt, start_ptt);
            match next {
                Some(n) => segment = n,
                None => break,
            }
        }
    }

    /// `SkOpContour::markAllDone()`.
    // Port of: src/pathops/SkOpContour.h#L174-L179 (chrome/m156)
    pub(crate) fn contour_mark_all_done(&mut self, c: ContourId) {
        self.contour_each_segment(c, |state, seg| {
            state.seg_mark_all_done(seg);
            true
        });
    }

    /// `SkOpContour::missingCoincidence()`.
    // Port of: src/pathops/SkOpContour.h#L181-L192 (chrome/m156)
    pub(crate) fn contour_missing_coincidence(&mut self, c: ContourId) -> bool {
        let mut result = false;
        self.contour_each_segment(c, |state, seg| {
            if state.seg_missing_coincidence(seg) {
                result = true;
            }
            true
        });
        result
    }

    /// `SkOpContour::moveMultiples()`.
    // Port of: src/pathops/SkOpContour.h#L194-L202 (chrome/m156)
    pub(crate) fn contour_move_multiples(&mut self, c: ContourId) -> bool {
        self.contour_each_segment(c, |state, seg| state.seg_move_multiples(seg))
    }

    /// `SkOpContour::moveNearby()`.
    // Port of: src/pathops/SkOpContour.h#L204-L212 (chrome/m156)
    pub(crate) fn contour_move_nearby(&mut self, c: ContourId) -> bool {
        self.contour_each_segment(c, |state, seg| state.seg_move_nearby(seg))
    }

    /// `SkOpContour::sortAngles()`.
    // Port of: src/pathops/SkOpContour.h#L250-L257 (chrome/m156)
    pub(crate) fn contour_sort_angles(&mut self, c: ContourId) -> bool {
        self.contour_each_segment(c, |state, seg| state.seg_sort_angles(seg))
    }

    /// `SkOpContour::undoneSpan()`.
    // Port of: src/pathops/SkOpContour.cpp#L59-L72 (chrome/m156)
    pub(crate) fn contour_undone_span(&mut self, c: ContourId) -> Option<crate::op_state::SpanId> {
        let mut test = self.contours[c.0].head;
        while let Some(segment) = test {
            if !self.seg_done(segment) {
                if let Some(span) = self.seg_undone_span(segment) {
                    return Some(span);
                }
                // Skia returns the segment's result, which may be nullptr.
                return None;
            }
            test = self.segments[segment.0].next;
        }
        self.contours[c.0].done = true;
        None
    }

    /// `SkOpContour::markAllDone` for every contour from `c` on: `SkOpContourHead::joinAllSegments`.
    // Port of: src/pathops/SkOpContour.h#L390-L400 (chrome/m156)
    pub(crate) fn contour_join_all_segments(&mut self, c: ContourId) {
        let mut next = Some(c);
        while let Some(n) = next {
            if self.contours[n.0].count != 0 {
                self.contour_join_segments(n);
            }
            next = self.contours[n.0].next;
        }
    }

    /// `SkOpContourHead::appendContour()`: appends a new contour to the list.
    // Port of: src/pathops/SkOpContour.h#L347-L358 (chrome/m156)
    pub(crate) fn contour_append_contour(&mut self, head: ContourId) -> ContourId {
        let contour = self.alloc_contour();
        self.contours[contour.0].next = None;
        let mut prev = head;
        while let Some(next) = self.contours[prev.0].next {
            prev = next;
        }
        self.contours[prev.0].next = Some(contour);
        contour
    }

    /// `SkOpContourHead::remove(contour)`.
    // Port of: src/pathops/SkOpContour.h#L360-L372 (chrome/m156)
    pub(crate) fn contour_head_remove(&mut self, head: ContourId, contour: ContourId) {
        if contour == head {
            return;
        }
        let mut prev = head;
        loop {
            let next = self.contours[prev.0].next.expect("contour is in the list");
            if next == contour {
                break;
            }
            prev = next;
        }
        self.contours[prev.0].next = None;
    }
}

impl ContourBuilder {
    /// `SkOpContourBuilder(SkOpContour* contour)`.
    #[must_use]
    pub(crate) fn new(contour: ContourId) -> Self {
        Self {
            contour,
            last_line: [Point::default(); 2],
            last_is_line: false,
        }
    }

    /// `SkOpContourBuilder::addLine(pts)`.
    // Port of: src/pathops/SkOpContour.cpp#L95-L106 (chrome/m156)
    pub(crate) fn add_line(&mut self, state: &mut OpState, pts: [Point; 2]) {
        if self.last_is_line {
            if self.last_line[0] == pts[1] && self.last_line[1] == pts[0] {
                self.last_is_line = false;
                return;
            }
            self.flush(state);
        }
        self.last_line = pts;
        self.last_is_line = true;
    }

    /// `SkOpContourBuilder::addCurve(verb, pts, weight)`.
    // Port of: src/pathops/SkOpContour.cpp#L36-L60 (chrome/m156)
    pub(crate) fn add_curve(&mut self, state: &mut OpState, verb: Verb, pts: &[Point], weight: f32) {
        if verb == Verb::Line {
            self.add_line(state, [pts[0], pts[1]]);
            return;
        }
        match verb {
            Verb::Quad => {
                self.add_quad(state, [pts[0], pts[1], pts[2]]);
            }
            Verb::Conic => {
                self.add_conic(state, [pts[0], pts[1], pts[2]], weight);
            }
            Verb::Cubic => {
                self.add_cubic(state, [pts[0], pts[1], pts[2], pts[3]]);
            }
            _ => {}
        }
    }

    /// `SkOpContourBuilder::addConic(pts, weight)`.
    // Port of: src/pathops/SkOpContour.cpp#L24-L28 (chrome/m156)
    pub(crate) fn add_conic(&mut self, state: &mut OpState, pts: [Point; 3], weight: f32) {
        self.flush(state);
        state.contour_add_conic(self.contour, pts, weight);
    }

    /// `SkOpContourBuilder::addCubic(pts)`.
    // Port of: src/pathops/SkOpContour.cpp#L30-L34 (chrome/m156)
    pub(crate) fn add_cubic(&mut self, state: &mut OpState, pts: [Point; 4]) {
        self.flush(state);
        state.contour_add_cubic(self.contour, pts);
    }

    /// `SkOpContourBuilder::addQuad(pts)`.
    // Port of: src/pathops/SkOpContour.cpp#L86-L90 (chrome/m156)
    pub(crate) fn add_quad(&mut self, state: &mut OpState, pts: [Point; 3]) {
        self.flush(state);
        state.contour_add_quad(self.contour, pts);
    }

    /// `SkOpContourBuilder::flush()`.
    // Port of: src/pathops/SkOpContour.cpp#L108-L116 (chrome/m156)
    pub(crate) fn flush(&mut self, state: &mut OpState) {
        if !self.last_is_line {
            return;
        }
        state.contour_add_line(self.contour, self.last_line);
        self.last_is_line = false;
    }

    /// `SkOpContourBuilder::contour()`.
    #[must_use]
    pub(crate) fn contour(&self) -> ContourId {
        self.contour
    }

    /// `SkOpContourBuilder::setContour(contour)`: flushes, then switches contours.
    pub(crate) fn set_contour(&mut self, state: &mut OpState, contour: ContourId) {
        self.flush(state);
        self.contour = contour;
    }
}
