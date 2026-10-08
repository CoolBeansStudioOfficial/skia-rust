// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsTSect.h (SkTSect), src/pathops/SkPathOpsTSect.cpp
// (the SkTSect methods, SkClosestRecord, SkClosestSect, SkTSect::BinarySearch), and the
// SkIntersections::intersect overloads for curve pairs at the end of SkPathOpsTSect.cpp.

//! The T-intersection search of `PathOps` (`SkTSect`): two curves are split into spans whose
//! bounding boxes and perpendicular hulls are tested against each other until the intersections
//! are found. The spans and sections live in a [`TSectArena`]; see `t_span` for the arena.

use crate::conic::DConic;
use crate::cubic::DCubic;
use crate::intersections::Intersections;
use crate::line::DLine;
use crate::point::{DPoint, DVector};
use crate::quad::DQuad;
use crate::t_curve::TCurve;
use crate::t_span::{SectId, SpanId, TCoincident, TSectArena};
use crate::types::{
    approximately_greater_than_one, approximately_less_than_zero, between, precisely_zero, std_max,
    std_min,
};
use skia_rust_core::t_sort::t_q_sort;

/// `COINCIDENT_SPAN_COUNT`.
// Port of: src/pathops/SkPathOpsTSect.cpp#L27 (chrome/m156)
const COINCIDENT_SPAN_COUNT: i32 = 9;

/// `kMaxCoinLoopCount`: local to `SkTSect::BinarySearch` in C++; hoisted here so it is not
/// declared after statements.
// Port of: src/pathops/SkPathOpsTSect.cpp#L1814 (chrome/m156)
const MAX_COIN_LOOP_COUNT: i32 = 8;

/// `kZeroS1Set` and friends: which end points of each curve were already reported.
const K_ZERO_S1_SET: i32 = 1;
const K_ONE_S1_SET: i32 = 2;
const K_ZERO_S2_SET: i32 = 4;
const K_ONE_S2_SET: i32 = 8;

/// `SkTSect`: the spans of one curve, its coincident spans, and its bookkeeping.
// Port of: src/pathops/SkPathOpsTSect.h#L234-L306 (chrome/m156)
#[doc(alias = "SkTSect")]
#[derive(Clone, Debug)]
pub struct TSect {
    /// `const SkTCurve& fCurve`.
    pub(crate) curve: TCurve,
    /// `SkTSpan* fHead`.
    pub(crate) head: Option<SpanId>,
    /// `SkTSpan* fCoincident`.
    pub(crate) coincident: Option<SpanId>,
    /// `SkTSpan* fDeleted`: freed spans, linked through `fNext`, reused by `addOne`.
    pub(crate) deleted: Option<SpanId>,
    /// `int fActiveCount`.
    pub(crate) active_count: i32,
    /// `bool fRemovedStartT`.
    pub(crate) removed_start_t: bool,
    /// `bool fRemovedEndT`.
    pub(crate) removed_end_t: bool,
    /// `bool fHung`.
    pub(crate) hung: bool,
}

/// `SkClosestRecord`: the closest pair of span end points found so far.
// Port of: src/pathops/SkPathOpsTSect.cpp#L1652-L1769 (chrome/m156)
#[derive(Copy, Clone, Debug)]
struct ClosestRecord {
    c1_span: Option<SpanId>,
    c2_span: Option<SpanId>,
    c1_start_t: f64,
    c1_end_t: f64,
    c2_start_t: f64,
    c2_end_t: f64,
    closest: f64,
    c1_index: usize,
    c2_index: usize,
}

impl ClosestRecord {
    /// `void reset()`.
    fn reset() -> Self {
        Self {
            c1_span: None,
            c2_span: None,
            c1_start_t: 0.0,
            c1_end_t: 0.0,
            c2_start_t: 0.0,
            c2_end_t: 0.0,
            // FLT_MAX
            closest: f64::from(f32::MAX),
            c1_index: 0,
            c2_index: 0,
        }
    }

    fn reset_in_place(&mut self) {
        *self = Self::reset();
    }

    /// `void addIntersection(SkIntersections* intersections) const`.
    fn add_intersection(&self, arena: &TSectArena, intersections: &mut Intersections) {
        let c1 = self.c1_span.expect("closest record has a span");
        let c2 = self.c2_span.expect("closest record has a span");
        let r1t = if self.c1_index != 0 {
            arena.end_t(c1)
        } else {
            arena.start_t(c1)
        };
        let r2t = if self.c2_index != 0 {
            arena.end_t(c2)
        } else {
            arena.start_t(c2)
        };
        intersections.insert(r1t, r2t, arena.span(c1).part.point(self.c1_index));
    }

    /// `void findEnd(const SkTSpan* span1, const SkTSpan* span2, int c1Index, int c2Index)`.
    fn find_end(
        &mut self,
        arena: &TSectArena,
        span1: SpanId,
        span2: SpanId,
        c1_index: usize,
        c2_index: usize,
    ) {
        let c1 = arena.span(span1).part;
        let c2 = arena.span(span2).part;
        if !c1.point(c1_index).approximately_equal(c2.point(c2_index)) {
            return;
        }
        let dist = c1.point(c1_index).distance_squared(c2.point(c2_index));
        if self.closest < dist {
            return;
        }
        self.c1_span = Some(span1);
        self.c2_span = Some(span2);
        self.c1_start_t = arena.start_t(span1);
        self.c1_end_t = arena.end_t(span1);
        self.c2_start_t = arena.start_t(span2);
        self.c2_end_t = arena.end_t(span2);
        self.c1_index = c1_index;
        self.c2_index = c2_index;
        self.closest = dist;
    }

    /// `bool matesWith(const SkClosestRecord& mate, SkIntersections* i) const`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1699-L1709 (chrome/m156)
    #[allow(clippy::float_cmp)] // exact comparisons of span end points, as SkClosestRecord::matesWith
    fn mates_with(&self, arena: &TSectArena, mate: &Self) -> bool {
        let c1 = self.c1_span.expect("closest record has a span");
        let m1 = mate.c1_span.expect("closest record has a span");
        let c2 = self.c2_span.expect("closest record has a span");
        let m2 = mate.c2_span.expect("closest record has a span");
        debug_assert!(
            c1 == m1
                || arena.end_t(c1) <= arena.start_t(m1)
                || arena.end_t(m1) <= arena.start_t(c1)
        );
        debug_assert!(
            c2 == m2
                || arena.end_t(c2) <= arena.start_t(m2)
                || arena.end_t(m2) <= arena.start_t(c2)
        );
        c1 == m1
            || arena.end_t(c1) == arena.start_t(m1)
            || arena.start_t(c1) == arena.end_t(m1)
            || c2 == m2
            || arena.end_t(c2) == arena.start_t(m2)
            || arena.start_t(c2) == arena.end_t(m2)
    }

    /// `void merge(const SkClosestRecord& mate)`.
    fn merge(&mut self, mate: &Self) {
        self.c1_span = mate.c1_span;
        self.c2_span = mate.c2_span;
        self.closest = mate.closest;
        self.c1_index = mate.c1_index;
        self.c2_index = mate.c2_index;
    }

    /// `void update(const SkClosestRecord& mate)`.
    fn update(&mut self, mate: &Self) {
        self.c1_start_t = std_min(self.c1_start_t, mate.c1_start_t);
        self.c1_end_t = std_max(self.c1_end_t, mate.c1_end_t);
        self.c2_start_t = std_min(self.c2_start_t, mate.c2_start_t);
        self.c2_end_t = std_max(self.c2_end_t, mate.c2_end_t);
    }
}

/// `SkClosestSect`: the closest-record list of one pair of sections.
// Port of: src/pathops/SkPathOpsTSect.cpp#L1771-L1828 (chrome/m156)
#[derive(Debug)]
struct ClosestSect {
    /// `fClosest`: one more record than `used`, the last one free for the next search.
    closest: Vec<ClosestRecord>,
    /// `int fUsed`.
    used: usize,
}

impl ClosestSect {
    /// `SkClosestSect()`.
    fn new() -> Self {
        Self {
            closest: vec![ClosestRecord::reset()],
            used: 0,
        }
    }

    /// `bool find(const SkTSpan* span1, const SkTSpan* span2)`.
    #[allow(clippy::float_cmp)] // FLT_MAX sentinel comparison, as SkClosestSect::find
    fn find(&mut self, arena: &TSectArena, span1: SpanId, span2: SpanId) -> bool {
        let rec_index = self.used;
        let mut record = self.closest[rec_index];
        record.find_end(arena, span1, span2, 0, 0);
        record.find_end(arena, span1, span2, 0, arena.span(span2).part.point_last());
        record.find_end(arena, span1, span2, arena.span(span1).part.point_last(), 0);
        record.find_end(
            arena,
            span1,
            span2,
            arena.span(span1).part.point_last(),
            arena.span(span2).part.point_last(),
        );
        if record.closest == f64::from(f32::MAX) {
            self.closest[rec_index] = record;
            return false;
        }
        for index in 0..self.used {
            let mut test = self.closest[index];
            if test.mates_with(arena, &record) {
                if test.closest > record.closest {
                    test.merge(&record);
                }
                test.update(&record);
                self.closest[index] = test;
                record.reset_in_place();
                self.closest[rec_index] = record;
                return false;
            }
        }
        self.used += 1;
        self.closest[rec_index] = record;
        self.closest.push(ClosestRecord::reset());
        true
    }

    /// `void finish(SkIntersections* intersections) const`.
    fn finish(&self, arena: &TSectArena, intersections: &mut Intersections) {
        let mut order: Vec<usize> = (0..self.used).collect();
        t_q_sort(&mut order, |a, b| {
            self.closest[*a].closest < self.closest[*b].closest
        });
        for index in order {
            self.closest[index].add_intersection(arena, intersections);
        }
    }
}

/// `static bool is_parallel(const SkDLine& thisLine, const SkTCurve& opp)`.
// Port of: src/pathops/SkPathOpsTSect.cpp#L1055-L1080 (chrome/m156)
fn is_parallel(this_line: &DLine, opp: &TCurve) -> bool {
    if !opp.is_conic() {
        return false; // FIXME : breaks a lot of stuff now
    }
    let mut finds = 0;
    let mut this_perp = DLine::default();
    this_perp.pts[0].x = this_line.pts[1].x + (this_line.pts[1].y - this_line.pts[0].y);
    this_perp.pts[0].y = this_line.pts[1].y + (this_line.pts[0].x - this_line.pts[1].x);
    this_perp.pts[1] = this_line.pts[1];
    let mut perp_ray_i = Intersections::default();
    opp.intersect_ray(&mut perp_ray_i, &this_perp);
    for p_index in 0..perp_ray_i.used() {
        finds += i32::from(perp_ray_i.pt(p_index).approximately_equal(this_perp.pts[1]));
    }
    this_perp.pts[1].x = this_line.pts[0].x + (this_line.pts[1].y - this_line.pts[0].y);
    this_perp.pts[1].y = this_line.pts[0].y + (this_line.pts[0].x - this_line.pts[1].x);
    this_perp.pts[0] = this_line.pts[0];
    opp.intersect_ray(&mut perp_ray_i, &this_perp);
    for p_index in 0..perp_ray_i.used() {
        finds += i32::from(perp_ray_i.pt(p_index).approximately_equal(this_perp.pts[0]));
    }
    finds >= 2
}

impl TSectArena {
    /// `SkTSect::SkTSect(const SkTCurve& c)`: the head span covers the whole curve.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L514-L532 (chrome/m156)
    pub(crate) fn new_sect(&mut self, curve: TCurve) -> SectId {
        self.sects.push(TSect {
            curve,
            head: None,
            coincident: None,
            deleted: None,
            active_count: 0,
            removed_start_t: false,
            removed_end_t: false,
            hung: false,
        });
        let sect = SectId(self.sects.len() - 1);
        self.reset_removed_ends(sect);
        let head = self.add_one(sect);
        self.sect_mut(sect).head = Some(head);
        self.init_span(head, curve);
        sect
    }

    fn head(&self, sect: SectId) -> Option<SpanId> {
        self.sect(sect).head
    }

    fn curve(&self, sect: SectId) -> TCurve {
        self.sect(sect).curve
    }

    /// `void SkTSect::resetRemovedEnds()`.
    fn reset_removed_ends(&mut self, sect: SectId) {
        let s = self.sect_mut(sect);
        s.removed_start_t = false;
        s.removed_end_t = false;
    }

    /// `SkTSpan* SkTSect::addOne()`: reuses a freed span when there is one.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L534-L561 (chrome/m156)
    pub(crate) fn add_one(&mut self, sect: SectId) -> SpanId {
        let result = if let Some(deleted) = self.sect(sect).deleted {
            self.sect_mut(sect).deleted = self.span(deleted).next;
            deleted
        } else {
            let curve = self.curve(sect);
            self.new_span(curve)
        };
        let span = self.span_mut(result);
        span.bounded = None;
        span.has_perp = false;
        span.deleted = false;
        self.sect_mut(sect).active_count += 1;
        result
    }

    /// `SkTSpan* SkTSect::addFollowing(SkTSpan* prior)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L71-L92 (chrome/m156)
    fn add_following(&mut self, sect: SectId, prior: Option<SpanId>) -> SpanId {
        let result = self.add_one(sect);
        let start_t = prior.map_or(0.0, |p| self.end_t(p));
        let next = match prior {
            Some(p) => self.span(p).next,
            None => self.head(sect),
        };
        let end_t = next.map_or(1.0, |n| self.start_t(n));
        {
            let span = self.span_mut(result);
            span.start_t = start_t;
            span.end_t = end_t;
            span.prev = prior;
            span.next = next;
        }
        match prior {
            Some(p) => self.span_mut(p).next = Some(result),
            None => self.sect_mut(sect).head = Some(result),
        }
        if let Some(n) = next {
            self.span_mut(n).prev = Some(result);
        }
        let curve = self.curve(sect);
        self.reset_bounds(result, curve);
        result
    }

    /// `void SkTSect::addForPerp(SkTSpan* span, double t)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L94-L117 (chrome/m156)
    fn add_for_perp(&mut self, sect: SectId, span: SpanId, t: f64) {
        if self.opp_t(span, t).is_none() {
            let (found, prior) = self.span_at_t(sect, t);
            let opp = match found {
                Some(opp) => opp,
                None => self.add_following(sect, prior),
            };
            self.add_bounded(opp, span);
            self.add_bounded(span, opp);
        }
    }

    /// `SkTSpan* SkTSect::addSplitAt(SkTSpan* span, double t)`.
    // Port of: src/pathops/SkPathOpsTSect.h#L262-L270 (chrome/m156)
    fn add_split_at(&mut self, sect: SectId, span: SpanId, t: f64) -> SpanId {
        let result = self.add_one(sect);
        self.split_at(result, span, t);
        let curve = self.curve(sect);
        self.init_bounds(result, curve);
        self.init_bounds(span, curve);
        result
    }

    /// `bool SkTSect::binarySearchCoin(SkTSect* sect2, double tStart, double tStep, double* t,
    /// double* oppT, SkTSpan** oppFirst)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L563-L629 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors SkTSect::binarySearchCoin's parameter list (out-parameters by reference)
    fn binary_search_coin(
        &mut self,
        sect: SectId,
        sect2: SectId,
        t_start: f64,
        t_step: f64,
        result_t: &mut f64,
        opp_t_out: &mut f64,
        opp_first: &mut Option<SpanId>,
    ) -> bool {
        let curve = self.curve(sect);
        let work = self.new_span(curve);
        let mut result = t_start;
        self.span_mut(work).start_t = t_start;
        self.span_mut(work).end_t = t_start;
        let mut last = curve.pt_at_t(t_start);
        let mut opp_pt = DPoint::default();
        let mut flip = false;
        let mut contained = false;
        let down = t_step < 0.0;
        let mut t_step = t_step;
        let opp = self.curve(sect2);
        loop {
            t_step *= 0.5;
            self.span_mut(work).start_t += t_step;
            if flip {
                t_step = -t_step;
                flip = false;
            }
            self.init_bounds(work, curve);
            if self.span(work).collapsed {
                return false;
            }
            let first_pt = self.point_first(work);
            if last.approximately_equal(first_pt) {
                break;
            }
            last = first_pt;
            let work_start_t = self.span(work).start_t;
            let mut coin_start = self.span(work).coin_start;
            coin_start.set_perp(&curve, work_start_t, last, &opp);
            self.span_mut(work).coin_start = coin_start;
            if coin_start.is_match() {
                let opp_t_test = coin_start.perp_t();
                let head2 = self.head(sect2).expect("sect has a head");
                if self.span_contains(head2, opp_t_test) {
                    *opp_t_out = opp_t_test;
                    opp_pt = coin_start.perp_pt();
                    contained = true;
                    let fail = if down {
                        result <= work_start_t
                    } else {
                        result >= work_start_t
                    };
                    if fail {
                        *opp_first = None; // signal caller to fail
                        return false;
                    }
                    result = work_start_t;
                    continue;
                }
            }
            t_step = -t_step;
            flip = true;
        }
        if !contained {
            return false;
        }
        if last.approximately_equal(curve.point(0)) {
            result = 0.0;
        } else if last.approximately_equal(self.point_last_of_sect(sect)) {
            result = 1.0;
        }
        if opp_pt.approximately_equal(opp.point(0)) {
            *opp_t_out = 0.0;
        } else if opp_pt.approximately_equal(self.point_last_of_sect(sect2)) {
            *opp_t_out = 1.0;
        }
        *result_t = result;
        true
    }

    /// `this->pointLast()`: the last point of a section's curve.
    fn point_last_of_sect(&self, sect: SectId) -> DPoint {
        let curve = self.curve(sect);
        curve.point(curve.point_last())
    }

    /// `SkTSpan* SkTSect::boundsMax()`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L631-L649 (chrome/m156)
    fn bounds_max(&mut self, sect: SectId) -> Option<SpanId> {
        let head = self.head(sect)?;
        let mut largest = head;
        let mut l_collapsed = self.span(largest).collapsed;
        let mut safety_net = 10000;
        let mut test = head;
        while let Some(next) = self.span(test).next {
            test = next;
            safety_net -= 1;
            if safety_net == 0 {
                self.sect_mut(sect).hung = true;
                return None;
            }
            let t_collapsed = self.span(test).collapsed;
            if (l_collapsed && !t_collapsed)
                || (l_collapsed == t_collapsed
                    && self.span(largest).bounds_max < self.span(test).bounds_max)
            {
                largest = test;
                l_collapsed = self.span(test).collapsed;
            }
        }
        Some(largest)
    }

    /// `bool SkTSect::coincidentCheck(SkTSect* sect2)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L651-L684 (chrome/m156)
    fn coincident_check(&mut self, sect: SectId, sect2: SectId) -> bool {
        let Some(mut first) = self.head(sect) else {
            return false;
        };
        loop {
            let (consecutive, last) = self.count_consecutive_spans(first);
            let next = self.next(last);
            if consecutive >= COINCIDENT_SPAN_COUNT {
                self.compute_perpendiculars(sect, sect2, first, Some(last));
                // check to see if a range of points are on the curve
                let mut coin_start = first;
                loop {
                    let (success, result) =
                        self.extract_coincident(sect, sect2, coin_start, Some(last));
                    if !success {
                        return false;
                    }
                    match result {
                        Some(r) if !self.span(last).deleted => coin_start = r,
                        _ => break,
                    }
                }
                if self.head(sect).is_none() || self.head(sect2).is_none() {
                    break;
                }
                match next {
                    None => break,
                    Some(n) if self.span(n).deleted => break,
                    Some(_) => {}
                }
            }
            match next {
                Some(n) => first = n,
                None => break,
            }
        }
        true
    }

    /// `void SkTSect::coincidentForce(SkTSect* sect2, double start1s, double start1e)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L686-L720 (chrome/m156)
    #[allow(clippy::similar_names, clippy::float_cmp)] // mirrors SkTSect::coincidentForce's names and its exact t comparisons
    fn coincident_force(&mut self, sect: SectId, sect2: SectId, start1s: f64, start1e: f64) {
        let first = self.head(sect);
        let last = self.tail(sect);
        let opp_first = self.head(sect2);
        let opp_last = self.tail(sect2);
        let (Some(first), Some(last), Some(opp_first), Some(opp_last)) =
            (first, last, opp_first, opp_last)
        else {
            return;
        };
        let mut delete_empty_spans = self.update_bounded(sect, first, last, opp_first);
        delete_empty_spans |= self.update_bounded(sect2, opp_first, opp_last, first);
        self.remove_span_range(sect, first, last);
        self.remove_span_range(sect2, opp_first, opp_last);
        let curve = self.curve(sect);
        let curve2 = self.curve(sect2);
        {
            let span = self.span_mut(first);
            span.start_t = start1s;
            span.end_t = start1e;
        }
        self.reset_bounds(first, curve);
        let mut coin_start = self.span(first).coin_start;
        coin_start.set_perp(&curve, start1s, curve.point(0), &curve2);
        self.span_mut(first).coin_start = coin_start;
        let mut coin_end = self.span(first).coin_end;
        coin_end.set_perp(&curve, start1e, self.point_last_of_sect(sect), &curve2);
        self.span_mut(first).coin_end = coin_end;
        let opp_matched = coin_start.perp_t() < coin_end.perp_t();
        let mut opp_start_t = if coin_start.perp_t() == -1.0 {
            0.0
        } else {
            std_max(0.0, coin_start.perp_t())
        };
        let mut opp_end_t = if coin_end.perp_t() == -1.0 {
            1.0
        } else {
            std_min(1.0, coin_end.perp_t())
        };
        if !opp_matched {
            std::mem::swap(&mut opp_start_t, &mut opp_end_t);
        }
        {
            let span = self.span_mut(opp_first);
            span.start_t = opp_start_t;
            span.end_t = opp_end_t;
        }
        self.reset_bounds(opp_first, curve2);
        self.remove_coincident(sect, first, false);
        self.remove_coincident(sect2, opp_first, true);
        if delete_empty_spans {
            self.delete_empty_spans(sect);
            self.delete_empty_spans(sect2);
        }
    }

    /// `bool SkTSect::coincidentHasT(double t)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L722-L731 (chrome/m156)
    fn coincident_has_t(&self, sect: SectId, t: f64) -> bool {
        let mut test = self.sect(sect).coincident;
        while let Some(span) = test {
            if between(self.start_t(span), t, self.end_t(span)) {
                return true;
            }
            test = self.next(span);
        }
        false
    }

    /// `int SkTSect::collapsed() const`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L733-L743 (chrome/m156)
    fn collapsed(&self, sect: SectId) -> i32 {
        let mut result = 0;
        let mut test = self.head(sect);
        while let Some(span) = test {
            if self.span(span).collapsed {
                result += 1;
            }
            test = self.next(span);
        }
        result
    }

    /// `void SkTSect::computePerpendiculars(SkTSect* sect2, SkTSpan* first, SkTSpan* last)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L745-L786 (chrome/m156)
    fn compute_perpendiculars(
        &mut self,
        sect: SectId,
        sect2: SectId,
        first: SpanId,
        last: Option<SpanId>,
    ) {
        let Some(last) = last else {
            return;
        };
        let curve = self.curve(sect);
        let opp = self.curve(sect2);
        let mut work = first;
        let mut prior: Option<SpanId> = None;
        loop {
            if !self.span(work).has_perp && !self.span(work).collapsed {
                if let Some(p) = prior {
                    self.span_mut(work).coin_start = self.span(p).coin_end;
                } else {
                    let (start_t, first_pt) = (self.start_t(work), self.point_first(work));
                    let mut coin = self.span(work).coin_start;
                    coin.set_perp(&curve, start_t, first_pt, &opp);
                    self.span_mut(work).coin_start = coin;
                }
                if self.span(work).coin_start.is_match() {
                    let perp_t = self.span(work).coin_start.perp_t();
                    if self.coincident_has_t(sect2, perp_t) {
                        self.span_mut(work).coin_start.init();
                    } else {
                        self.add_for_perp(sect2, work, perp_t);
                    }
                }
                let (end_t, last_pt) = (self.end_t(work), self.point_last(work));
                let mut coin = self.span(work).coin_end;
                coin.set_perp(&curve, end_t, last_pt, &opp);
                self.span_mut(work).coin_end = coin;
                if self.span(work).coin_end.is_match() {
                    let perp_t = self.span(work).coin_end.perp_t();
                    if self.coincident_has_t(sect2, perp_t) {
                        self.span_mut(work).coin_end.init();
                    } else {
                        self.add_for_perp(sect2, work, perp_t);
                    }
                }
                self.span_mut(work).has_perp = true;
            }
            if work == last {
                break;
            }
            prior = Some(work);
            let Some(next) = self.next(work) else {
                break;
            };
            work = next;
        }
    }

    /// `int SkTSect::countConsecutiveSpans(SkTSpan* first, SkTSpan** lastPtr) const`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L788-L805 (chrome/m156)
    fn count_consecutive_spans(&self, first: SpanId) -> (i32, SpanId) {
        let mut consecutive = 1;
        let mut last = first;
        while let Some(next) = self.next(last) {
            if self.start_t(next) > self.end_t(last) {
                break;
            }
            consecutive += 1;
            last = next;
        }
        (consecutive, last)
    }

    /// `bool SkTSect::hasBounded(const SkTSpan* span) const`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L807-L818 (chrome/m156)
    fn has_bounded(&self, sect: SectId, span: SpanId) -> bool {
        let mut test = self.head(sect);
        while let Some(t) = test {
            if self.find_opp_span(t, span).is_some() {
                return true;
            }
            test = self.next(t);
        }
        false
    }

    /// `bool SkTSect::deleteEmptySpans()`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L820-L836 (chrome/m156)
    fn delete_empty_spans(&mut self, sect: SectId) -> bool {
        let mut next = self.head(sect);
        let mut safety_hatch = 1000;
        while let Some(test) = next {
            next = self.next(test);
            if !self.is_bounded(test) && !self.remove_span(sect, test) {
                return false;
            }
            safety_hatch -= 1;
            if safety_hatch < 0 {
                return false;
            }
        }
        true
    }

    /// `bool SkTSect::extractCoincident(SkTSect* sect2, SkTSpan* first, SkTSpan* last,
    /// SkTSpan** result)`. Returns `(success, result)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L838-L955 (chrome/m156)
    #[allow(clippy::too_many_lines, clippy::float_cmp, clippy::similar_names)] // mirrors SkTSect::extractCoincident, kept as one function as in Skia
    fn extract_coincident(
        &mut self,
        sect: SectId,
        sect2: SectId,
        first_in: SpanId,
        last_in: Option<SpanId>,
    ) -> (bool, Option<SpanId>) {
        let mut last_var = last_in;
        let found_first = self.find_coincident_run(first_in, &mut last_var);
        let (Some(mut first), Some(last)) = (found_first, last_var) else {
            return (true, None);
        };
        // march outwards to find limit of coincidence from here to previous and next spans
        let start_t = self.start_t(first);
        let mut opp_start_t: f64 = 0.0;
        let mut opp_end_t: f64 = 0.0;
        let prev = self.span(first).prev;
        debug_assert!(self.span(first).coin_start.is_match());
        let mut opp_first = self.opp_t(first, self.span(first).coin_start.perp_t());
        debug_assert!(self.span(last).coin_end.is_match());
        let opp_matched = self.span(first).coin_start.perp_t() < self.span(first).coin_end.perp_t();
        let mut coin_start_v: f64 = 0.0;
        // if the previous span ends here, search backwards for where the coincidence starts
        let cut_first = match prev {
            Some(p)
                if self.end_t(p) == start_t
                    && self.binary_search_coin(
                        sect,
                        sect2,
                        start_t,
                        self.start_t(p) - start_t,
                        &mut coin_start_v,
                        &mut opp_start_t,
                        &mut opp_first,
                    ) =>
            {
                if self.start_t(p) < coin_start_v && coin_start_v < start_t {
                    self.opp_t(p, opp_start_t)
                } else {
                    None
                }
            }
            _ => None,
        };
        if let (Some(p), Some(cut)) = (prev, cut_first) {
            opp_first = Some(cut);
            first = self.add_split_at(sect, p, coin_start_v);
            self.mark_coincident(first);
            self.span_mut(p).coin_end.mark_coincident();
            let opp_first_id = cut;
            if self.start_t(opp_first_id) < opp_start_t && opp_start_t < self.end_t(opp_first_id) {
                let opp_half = self.add_split_at(sect2, opp_first_id, opp_start_t);
                if opp_matched {
                    self.span_mut(opp_first_id).coin_end.mark_coincident();
                    self.mark_coincident(opp_half);
                    opp_first = Some(opp_half);
                } else {
                    self.mark_coincident(opp_first_id);
                    self.span_mut(opp_half).coin_start.mark_coincident();
                }
            }
        } else {
            let Some(of) = opp_first else {
                return (false, None);
            };
            opp_start_t = if opp_matched {
                self.start_t(of)
            } else {
                self.end_t(of)
            };
        }
        // FIXME: incomplete : if we're not at the end, find end of coin
        let mut opp_last = self.opp_t(last, self.span(last).coin_end.perp_t());
        if let Some(ol) = opp_last {
            opp_end_t = if opp_matched {
                self.end_t(ol)
            } else {
                self.start_t(ol)
            };
        }
        if !opp_matched {
            std::mem::swap(&mut opp_first, &mut opp_last);
            std::mem::swap(&mut opp_start_t, &mut opp_end_t);
        }
        debug_assert!(opp_start_t < opp_end_t);
        let Some(opp_first_id) = opp_first else {
            return (true, None);
        };
        debug_assert_eq!(opp_start_t, self.start_t(opp_first_id));
        let Some(opp_last_id) = opp_last else {
            return (true, None);
        };
        debug_assert_eq!(opp_end_t, self.end_t(opp_last_id));
        // reduce coincident runs to single entries
        let mut delete_empty_spans = self.update_bounded(sect, first, last, opp_first_id);
        delete_empty_spans |= self.update_bounded(sect2, opp_first_id, opp_last_id, first);
        self.remove_span_range(sect, first, last);
        self.remove_span_range(sect2, opp_first_id, opp_last_id);
        let curve = self.curve(sect);
        let curve2 = self.curve(sect2);
        self.span_mut(first).end_t = self.end_t(last);
        self.reset_bounds(first, curve);
        let (first_start, first_pt) = (self.start_t(first), self.point_first(first));
        let mut coin_start = self.span(first).coin_start;
        coin_start.set_perp(&curve, first_start, first_pt, &curve2);
        self.span_mut(first).coin_start = coin_start;
        let (first_end, first_last_pt) = (self.end_t(first), self.point_last(first));
        let mut coin_end = self.span(first).coin_end;
        coin_end.set_perp(&curve, first_end, first_last_pt, &curve2);
        self.span_mut(first).coin_end = coin_end;
        opp_start_t = coin_start.perp_t();
        opp_end_t = coin_end.perp_t();
        if between(0.0, opp_start_t, 1.0) && between(0.0, opp_end_t, 1.0) {
            if !opp_matched {
                std::mem::swap(&mut opp_start_t, &mut opp_end_t);
            }
            {
                let span = self.span_mut(opp_first_id);
                span.start_t = opp_start_t;
                span.end_t = opp_end_t;
            }
            self.reset_bounds(opp_first_id, curve2);
        }
        let after_first = self.next(first);
        if !self.remove_coincident(sect, first, false) {
            return (false, None);
        }
        if !self.remove_coincident(sect2, opp_first_id, true) {
            return (false, None);
        }
        if delete_empty_spans && (!self.delete_empty_spans(sect) || !self.delete_empty_spans(sect2))
        {
            return (false, None);
        }
        // *result = last && !last->fDeleted && fHead && sect2->fHead ? last : nullptr;
        let result = match after_first {
            Some(l)
                if !self.span(l).deleted
                    && self.head(sect).is_some()
                    && self.head(sect2).is_some() =>
            {
                Some(l)
            }
            _ => None,
        };
        (true, result)
    }

    /// `SkTSpan* SkTSect::findCoincidentRun(SkTSpan* first, SkTSpan** lastPtr)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L957-L996 (chrome/m156)
    fn find_coincident_run(
        &self,
        first_in: SpanId,
        last_ptr: &mut Option<SpanId>,
    ) -> Option<SpanId> {
        let mut work = first_in;
        let mut last_candidate: Option<SpanId> = None;
        let mut first: Option<SpanId> = None;
        // find the first fully coincident span
        loop {
            let span = self.span(work);
            if span.coin_start.is_match() {
                debug_assert!(self.opp_t(work, span.coin_start.perp_t()).is_some());
                if !span.coin_end.is_match() {
                    break;
                }
                last_candidate = Some(work);
                if first.is_none() {
                    first = Some(work);
                }
            } else if first.is_some() && span.collapsed {
                *last_ptr = last_candidate;
                return first;
            } else {
                last_candidate = None;
                debug_assert!(first.is_none());
            }
            if Some(work) == *last_ptr {
                return first;
            }
            // `work = work->fNext; if (!work) return nullptr;`
            work = span.next?;
        }
        if last_candidate.is_some() {
            *last_ptr = last_candidate;
        }
        first
    }

    /// `int SkTSect::intersects(SkTSpan* span, SkTSect* opp, SkTSpan* oppSpan, int* oppResult)`.
    /// Returns `(result, oppResult)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L998-L1053 (chrome/m156)
    fn intersects(
        &mut self,
        sect: SectId,
        span: SpanId,
        opp: SectId,
        opp_span: SpanId,
    ) -> (i32, i32) {
        let mut span_start = false;
        let mut opp_start = false;
        let mut hull_result = self.hulls_intersect(span, opp_span, &mut span_start, &mut opp_start);
        if hull_result >= 0 {
            let mut opp_result = 1;
            if hull_result == 2 {
                // hulls have one point in common
                // `!span->fBounded || !span->fBounded->fNext`
                let span_has_one = self
                    .span(span)
                    .bounded
                    .is_none_or(|b| self.bounded_node(b).next.is_none());
                if span_has_one {
                    debug_assert!(
                        self.span(span)
                            .bounded
                            .is_none_or(|b| self.bounded_node(b).span == opp_span)
                    );
                    if span_start {
                        self.span_mut(span).end_t = self.span(span).start_t;
                    } else {
                        self.span_mut(span).start_t = self.span(span).end_t;
                    }
                } else {
                    hull_result = 1;
                }
                // `!oppSpan->fBounded || !oppSpan->fBounded->fNext`
                let opp_has_one = self
                    .span(opp_span)
                    .bounded
                    .is_none_or(|b| self.bounded_node(b).next.is_none());
                if opp_has_one {
                    // `oppSpan->fBounded && oppSpan->fBounded->fBounded != span`
                    if self
                        .span(opp_span)
                        .bounded
                        .is_some_and(|b| self.bounded_node(b).span != span)
                    {
                        return (0, 0);
                    }
                    if opp_start {
                        self.span_mut(opp_span).end_t = self.span(opp_span).start_t;
                    } else {
                        self.span_mut(opp_span).start_t = self.span(opp_span).end_t;
                    }
                    opp_result = 2;
                } else {
                    opp_result = 1;
                }
            }
            return (hull_result, opp_result);
        }
        if self.span(span).is_line && self.span(opp_span).is_line {
            let mut i = Intersections::default();
            let sects = self.lines_intersect(sect, span, opp, opp_span, &mut i);
            if sects == 2 {
                return (1, 1);
            }
            if sects == 0 {
                return (-1, 0);
            }
            self.removed_end_check(sect, span);
            let t0 = i.t(0, 0);
            self.span_mut(span).start_t = t0;
            self.span_mut(span).end_t = t0;
            self.removed_end_check(opp, opp_span);
            let t1 = i.t(1, 0);
            self.span_mut(opp_span).start_t = t1;
            self.span_mut(opp_span).end_t = t1;
            return (2, 2);
        }
        if self.span(span).is_linear || self.span(opp_span).is_linear {
            let result = i32::from(self.linears_intersect(span, opp_span));
            return (result, result);
        }
        (1, 1)
    }

    /// `int SkTSect::linesIntersect(SkTSpan* span, SkTSect* opp, SkTSpan* oppSpan,
    /// SkIntersections* i)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1083-L1240 (chrome/m156)
    #[allow(
        clippy::too_many_lines,
        clippy::float_cmp,
        clippy::similar_names,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )] // mirrors SkTSect::linesIntersect, kept as one function with Skia's exact arithmetic
    fn lines_intersect(
        &mut self,
        sect: SectId,
        span: SpanId,
        opp: SectId,
        opp_span: SpanId,
        i: &mut Intersections,
    ) -> i32 {
        let curve = self.curve(sect);
        let opp_curve = self.curve(opp);
        let mut this_ray = Intersections::default();
        let mut opp_ray = Intersections::default();
        let mut this_line = DLine::new([self.point_first(span), self.point_last(span)]);
        let mut opp_line = DLine::new([self.point_first(opp_span), self.point_last(opp_span)]);
        let mut loop_count = 0;
        let mut best_dist_sq = f64::MAX;
        if opp_curve.intersect_ray(&mut this_ray, &this_line) == 0 {
            return 0;
        }
        if curve.intersect_ray(&mut opp_ray, &opp_line) == 0 {
            return 0;
        }
        // if the ends of each line intersect the opposite curve, the lines are coincident
        if this_ray.used() > 1 {
            let mut pt_matches = 0;
            for t_index in 0..this_ray.used() {
                for l_index in 0..this_line.pts.len() {
                    pt_matches += i32::from(
                        this_ray
                            .pt(t_index)
                            .approximately_equal(this_line.pts[l_index]),
                    );
                }
            }
            if pt_matches == 2 || is_parallel(&this_line, &opp_curve) {
                return 2;
            }
        }
        if opp_ray.used() > 1 {
            let mut pt_matches = 0;
            for o_index in 0..opp_ray.used() {
                for l_index in 0..opp_line.pts.len() {
                    pt_matches += i32::from(
                        opp_ray
                            .pt(o_index)
                            .approximately_equal(opp_line.pts[l_index]),
                    );
                }
            }
            if pt_matches == 2 || is_parallel(&opp_line, &curve) {
                return 2;
            }
        }
        loop {
            // pick the closest pair of points
            let mut closest = f64::MAX;
            let mut close_index = 0;
            let mut opp_close_index = 0;
            for index in 0..opp_ray.used() {
                if !crate::types::roughly_between(
                    self.start_t(span),
                    opp_ray.t(0, index),
                    self.end_t(span),
                ) {
                    continue;
                }
                for o_index in 0..this_ray.used() {
                    if !crate::types::roughly_between(
                        self.start_t(opp_span),
                        this_ray.t(0, o_index),
                        self.end_t(opp_span),
                    ) {
                        continue;
                    }
                    let dist_sq = this_ray.pt(index).distance_squared(opp_ray.pt(o_index));
                    if closest > dist_sq {
                        closest = dist_sq;
                        close_index = index;
                        opp_close_index = o_index;
                    }
                }
            }
            if closest == f64::MAX {
                break;
            }
            let opp_i_pt = this_ray.pt(opp_close_index);
            let i_pt = opp_ray.pt(close_index);
            if between(
                self.start_t(span),
                opp_ray.t(0, close_index),
                self.end_t(span),
            ) && between(
                self.start_t(opp_span),
                this_ray.t(0, opp_close_index),
                self.end_t(opp_span),
            ) && opp_i_pt.approximately_equal(i_pt)
            {
                i.merge(&opp_ray, close_index, &this_ray, opp_close_index);
                return i32::try_from(i.used()).unwrap_or(i32::MAX);
            }
            let dist_sq = opp_i_pt.distance_squared(i_pt);
            loop_count += 1;
            if best_dist_sq < dist_sq || loop_count > 5 {
                return 0;
            }
            best_dist_sq = dist_sq;
            let opp_start = opp_ray.t(0, close_index);
            this_line.pts[0] = curve.pt_at_t(opp_start);
            this_line.pts[1] = this_line.pts[0] + curve.dxdy_at_t(opp_start);
            if opp_curve.intersect_ray(&mut this_ray, &this_line) == 0 {
                break;
            }
            let start = this_ray.t(0, opp_close_index);
            opp_line.pts[0] = opp_curve.pt_at_t(start);
            opp_line.pts[1] = opp_line.pts[0] + opp_curve.dxdy_at_t(start);
            if curve.intersect_ray(&mut opp_ray, &opp_line) == 0 {
                break;
            }
        }
        // convergence may fail if the curves are nearly coincident
        let mut o_coin_s = TCoincident::default();
        let mut o_coin_e = TCoincident::default();
        o_coin_s.set_perp(
            &opp_curve,
            self.start_t(opp_span),
            self.point_first(opp_span),
            &curve,
        );
        o_coin_e.set_perp(
            &opp_curve,
            self.end_t(opp_span),
            self.point_last(opp_span),
            &curve,
        );
        let mut t_start = o_coin_s.perp_t();
        let mut t_end = o_coin_e.perp_t();
        let swap = t_start > t_end;
        if swap {
            std::mem::swap(&mut t_start, &mut t_end);
        }
        t_start = std_max(t_start, self.start_t(span));
        t_end = std_min(t_end, self.end_t(span));
        if t_start > t_end {
            return 0;
        }
        let perp_s: DVector = if t_start == self.start_t(span) {
            let mut coin_s = TCoincident::default();
            coin_s.set_perp(
                &curve,
                self.start_t(span),
                self.point_first(span),
                &opp_curve,
            );
            self.point_first(span) - coin_s.perp_pt()
        } else if swap {
            o_coin_e.perp_pt() - self.point_last(opp_span)
        } else {
            o_coin_s.perp_pt() - self.point_first(opp_span)
        };
        let perp_e: DVector = if t_end == self.end_t(span) {
            let mut coin_e = TCoincident::default();
            coin_e.set_perp(&curve, self.end_t(span), self.point_last(span), &opp_curve);
            self.point_last(span) - coin_e.perp_pt()
        } else if swap {
            o_coin_s.perp_pt() - self.point_first(opp_span)
        } else {
            o_coin_e.perp_pt() - self.point_last(opp_span)
        };
        if perp_s.dot(perp_e) >= 0.0 {
            return 0;
        }
        let mut coin_w = TCoincident::default();
        let mut work_t = t_start;
        let mut t_step = t_end - t_start;
        let (work_t, work_pt) = loop {
            t_step *= 0.5;
            if precisely_zero(t_step) {
                return 0;
            }
            work_t += t_step;
            let work_pt = curve.pt_at_t(work_t);
            coin_w.set_perp(&curve, work_t, work_pt, &opp_curve);
            let perp_t = coin_w.perp_t();
            let skip = if coin_w.is_match() {
                !between(self.start_t(opp_span), perp_t, self.end_t(opp_span))
            } else {
                perp_t < 0.0
            };
            if skip {
                continue;
            }
            let perp_w = work_pt - coin_w.perp_pt();
            if (perp_s.dot(perp_w) >= 0.0) == (t_step < 0.0) {
                t_step = -t_step;
            }
            if work_pt.approximately_equal(coin_w.perp_pt()) {
                break (work_t, work_pt);
            }
        };
        let opp_t_test = coin_w.perp_t();
        let opp_head = self.head(opp).expect("sect has a head");
        if !self.span_contains(opp_head, opp_t_test) {
            return 0;
        }
        i.set_max(1);
        i.insert(work_t, opp_t_test, work_pt);
        1
    }

    /// `bool SkTSect::markSpanGone(SkTSpan* span)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1242-L1251 (chrome/m156)
    fn mark_span_gone(&mut self, sect: SectId, span: SpanId) -> bool {
        self.sect_mut(sect).active_count -= 1;
        if self.sect(sect).active_count < 0 {
            return false;
        }
        let deleted = self.sect(sect).deleted;
        self.span_mut(span).next = deleted;
        self.sect_mut(sect).deleted = Some(span);
        debug_assert!(!self.span(span).deleted);
        self.span_mut(span).deleted = true;
        true
    }

    /// `void SkTSect::mergeCoincidence(SkTSect* sect2)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1270-L1327 (chrome/m156)
    #[allow(
        clippy::similar_names,
        clippy::neg_cmp_op_on_partial_ord,
        clippy::float_cmp,
        clippy::manual_midpoint
    )] // mirrors SkTSect::mergeCoincidence: NaN-sensitive `!(a < b)` tests and `(a + b) / 2`
    fn merge_coincidence(&mut self, sect: SectId, sect2: SectId) {
        let mut small_limit = 0.0;
        loop {
            // find the smallest unprocessed span
            let mut smaller: Option<SpanId> = None;
            let mut test = self.sect(sect).coincident;
            loop {
                let Some(t) = test else {
                    return;
                };
                if !(self.start_t(t) < small_limit) {
                    let skip = matches!(smaller, Some(s) if self.end_t(s) < self.start_t(t));
                    if !skip {
                        smaller = Some(t);
                    }
                }
                test = self.next(t);
                if test.is_none() {
                    break;
                }
            }
            let Some(smaller) = smaller else {
                return;
            };
            small_limit = self.end_t(smaller);
            // find next larger span
            let mut prior: Option<SpanId> = None;
            let mut larger: Option<SpanId> = None;
            let mut larger_prior: Option<SpanId> = None;
            let mut test = self.sect(sect).coincident;
            while let Some(t) = test {
                if !(self.start_t(t) < self.end_t(smaller)) {
                    debug_assert_ne!(self.start_t(t), self.end_t(smaller));
                    let skip = matches!(larger, Some(l) if self.start_t(l) < self.start_t(t));
                    if !skip {
                        larger_prior = prior;
                        larger = Some(t);
                    }
                }
                prior = Some(t);
                test = self.next(t);
            }
            let Some(larger) = larger else {
                continue;
            };
            // check middle t value to see if it is coincident as well
            let mid_t = (self.end_t(smaller) + self.start_t(larger)) / 2.0;
            let curve2 = self.curve(sect2);
            let mid_pt = self.curve(sect).pt_at_t(mid_t);
            let mut coin = TCoincident::default();
            coin.set_perp(&self.curve(sect), mid_t, mid_pt, &curve2);
            if coin.is_match() {
                let larger_end = self.end_t(larger);
                let larger_coin_end = self.span(larger).coin_end;
                let larger_next = self.next(larger);
                self.span_mut(smaller).end_t = larger_end;
                self.span_mut(smaller).coin_end = larger_coin_end;
                match larger_prior {
                    Some(lp) => self.span_mut(lp).next = larger_next,
                    None => self.sect_mut(sect).coincident = larger_next,
                }
            }
        }
    }

    /// `void SkTSect::recoverCollapsed()`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1341-L1355 (chrome/m156)
    #[allow(clippy::neg_cmp_op_on_partial_ord)] // `!(end <= start)` is the C++ loop condition, which differs from `end > start` for NaN
    fn recover_collapsed(&mut self, sect: SectId) {
        let mut deleted = self.sect(sect).deleted;
        while let Some(d) = deleted {
            let del_next = self.next(d);
            if self.span(d).collapsed {
                // SkTSpan** spanPtr = &fHead: the slot is the head (None) or a span's fNext
                let mut slot: Option<SpanId> = None;
                let mut cur = self.head(sect);
                while let Some(c) = cur {
                    if !(self.end_t(c) <= self.start_t(d)) {
                        break;
                    }
                    slot = Some(c);
                    cur = self.next(c);
                }
                self.span_mut(d).next = cur;
                match slot {
                    Some(s) => self.span_mut(s).next = Some(d),
                    None => self.sect_mut(sect).head = Some(d),
                }
            }
            deleted = del_next;
        }
    }

    /// `void SkTSect::removeAllBut(const SkTSpan* keep, SkTSpan* span, SkTSect* opp)`. The
    /// C++ member `this` is not used by the body, so it is not a parameter here.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1357-L1375 (chrome/m156)
    fn remove_all_but(&mut self, keep: SpanId, span: SpanId, opp: SectId) {
        let mut node = self.span(span).bounded;
        while let Some(n) = node {
            let bounded_node = self.bounded_node(n);
            let bounded = bounded_node.span;
            let next = bounded_node.next;
            // may have been deleted when opp did 'remove all but'
            if bounded != keep && !self.span(bounded).deleted {
                let removed = self.remove_bounded(span, bounded);
                debug_assert!(!removed);
                if self.remove_bounded(bounded, span) {
                    self.remove_span(opp, bounded);
                }
            }
            node = next;
        }
        debug_assert!(!self.span(span).deleted);
        debug_assert!(self.find_opp_span(span, keep).is_some());
        debug_assert!(self.find_opp_span(keep, span).is_some());
    }

    /// `bool SkTSect::removeByPerpendicular(SkTSect* opp)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1377-L1399 (chrome/m156)
    #[allow(clippy::neg_cmp_op_on_partial_ord)] // `!(dot <= 0)` and `!(perp < 0)` are the C++ continue conditions, NaN-sensitive
    fn remove_by_perpendicular(&mut self, sect: SectId, opp: SectId) -> bool {
        let Some(mut test) = self.head(sect) else {
            return true;
        };
        loop {
            let next = self.next(test);
            let coin_start = self.span(test).coin_start;
            let coin_end = self.span(test).coin_end;
            // continue unless neither perpendicular is negative
            if !(coin_start.perp_t() < 0.0 || coin_end.perp_t() < 0.0) {
                let start_v = coin_start.perp_pt() - self.point_first(test);
                let end_v = coin_end.perp_pt() - self.point_last(test);
                // continue unless the perpendicular vectors point away from each other
                if !(start_v.dot(end_v) <= 0.0) && !self.remove_spans(sect, test, opp) {
                    return false;
                }
            }
            match next {
                Some(n) => test = n,
                None => break,
            }
        }
        true
    }

    /// `bool SkTSect::removeCoincident(SkTSpan* span, bool isBetween)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1401-L1413 (chrome/m156)
    fn remove_coincident(&mut self, sect: SectId, span: SpanId, is_between: bool) -> bool {
        if !self.unlink_span(sect, span) {
            return false;
        }
        if is_between || between(0.0, self.span(span).coin_start.perp_t(), 1.0) {
            self.sect_mut(sect).active_count -= 1;
            let coincident = self.sect(sect).coincident;
            self.span_mut(span).next = coincident;
            self.sect_mut(sect).coincident = Some(span);
        } else {
            self.mark_span_gone(sect, span);
        }
        true
    }

    /// `void SkTSect::removedEndCheck(SkTSpan* span)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1415-L1422 (chrome/m156)
    #[allow(clippy::float_cmp)] // mirrors Skia's exact comparisons with the t end points
    fn removed_end_check(&mut self, sect: SectId, span: SpanId) {
        if self.start_t(span) == 0.0 {
            self.sect_mut(sect).removed_start_t = true;
        }
        if self.end_t(span) == 1.0 {
            self.sect_mut(sect).removed_end_t = true;
        }
    }

    /// `bool SkTSect::removeSpan(SkTSpan* span)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1424-L1430 (chrome/m156)
    fn remove_span(&mut self, sect: SectId, span: SpanId) -> bool {
        self.removed_end_check(sect, span);
        if !self.unlink_span(sect, span) {
            return false;
        }
        self.mark_span_gone(sect, span)
    }

    /// `void SkTSect::removeSpanRange(SkTSpan* first, SkTSpan* last)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1432-L1451 (chrome/m156)
    fn remove_span_range(&mut self, sect: SectId, first: SpanId, last: SpanId) {
        if first == last {
            return;
        }
        let final_span = self.next(last);
        let mut next = self.next(first);
        while let Some(span) = next {
            if Some(span) == final_span {
                break;
            }
            next = self.next(span);
            self.mark_span_gone(sect, span);
        }
        if let Some(f) = final_span {
            self.span_mut(f).prev = Some(first);
        }
        self.span_mut(first).next = final_span;
    }

    /// `bool SkTSect::removeSpans(SkTSpan* span, SkTSect* opp)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1453-L1471 (chrome/m156)
    fn remove_spans(&mut self, sect: SectId, span: SpanId, opp: SectId) -> bool {
        let mut bounded = self.span(span).bounded;
        while let Some(n) = bounded {
            let node = self.bounded_node(n);
            let span_bounded = node.span;
            if self.remove_bounded(span, span_bounded) {
                // shuffles last into position 0
                self.remove_span(sect, span);
            }
            if self.remove_bounded(span_bounded, span) {
                self.remove_span(opp, span_bounded);
            }
            if self.span(span).deleted && self.has_bounded(opp, span) {
                return false;
            }
            bounded = node.next;
        }
        true
    }

    /// `SkTSpan* SkTSect::spanAtT(double t, SkTSpan** priorSpan)`. Returns `(span, priorSpan)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1473-L1483 (chrome/m156)
    #[allow(clippy::neg_cmp_op_on_partial_ord)] // `!(end < t)` is the C++ loop condition, NaN-sensitive
    fn span_at_t(&self, sect: SectId, t: f64) -> (Option<SpanId>, Option<SpanId>) {
        let mut test = self.head(sect);
        let mut prev = None;
        while let Some(s) = test {
            if !(self.end_t(s) < t) {
                break;
            }
            prev = Some(s);
            test = self.next(s);
        }
        let result = match test {
            Some(s) if self.start_t(s) <= t => Some(s),
            _ => None,
        };
        (result, prev)
    }

    /// `SkTSpan* SkTSect::tail()`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1485-L1501 (chrome/m156)
    fn tail(&self, sect: SectId) -> Option<SpanId> {
        let head = self.head(sect)?;
        let mut result = head;
        let mut next = head;
        let mut safety_net = 100_000;
        while let Some(n) = self.next(next) {
            next = n;
            safety_net -= 1;
            if safety_net == 0 {
                return None;
            }
            if self.end_t(n) > self.end_t(result) {
                result = n;
            }
        }
        Some(result)
    }

    /// `bool SkTSect::trim(SkTSpan* span, SkTSect* opp)`: each span keeps a range of opposite
    /// spans it intersects; after a split, the range is adjusted to the new size.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1503-L1532 (chrome/m156)
    fn trim(&mut self, sect: SectId, span: SpanId, opp: SectId) -> bool {
        let curve = self.curve(sect);
        if !self.init_bounds(span, curve) {
            return false;
        }
        let mut test_bounded = self.span(span).bounded;
        while let Some(node_id) = test_bounded {
            let node = self.bounded_node(node_id);
            let test = node.span;
            let (sects, opp_sects) = self.intersects(sect, span, opp, test);
            if sects >= 1 {
                if opp_sects == 2 {
                    let opp_curve = self.curve(opp);
                    self.init_bounds(test, opp_curve);
                    self.remove_all_but(span, test, sect);
                }
                if sects == 2 {
                    self.init_bounds(span, curve);
                    self.remove_all_but(test, span, opp);
                    return true;
                }
            } else {
                if self.remove_bounded(span, test) {
                    self.remove_span(sect, span);
                }
                if self.remove_bounded(test, span) {
                    self.remove_span(opp, test);
                }
            }
            test_bounded = node.next;
        }
        true
    }

    /// `bool SkTSect::unlinkSpan(SkTSpan* span)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1534-L1554 (chrome/m156)
    fn unlink_span(&mut self, sect: SectId, span: SpanId) -> bool {
        let prev = self.span(span).prev;
        let next = self.span(span).next;
        if let Some(p) = prev {
            self.span_mut(p).next = next;
            if let Some(n) = next {
                self.span_mut(n).prev = Some(p);
                if self.start_t(n) > self.end_t(n) {
                    return false;
                }
            }
        } else {
            self.sect_mut(sect).head = next;
            if let Some(n) = next {
                self.span_mut(n).prev = None;
            }
        }
        true
    }

    /// `bool SkTSect::updateBounded(SkTSpan* first, SkTSpan* last, SkTSpan* oppFirst)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1556-L1568 (chrome/m156)
    fn update_bounded(
        &mut self,
        _sect: SectId,
        first: SpanId,
        last: SpanId,
        opp_first: SpanId,
    ) -> bool {
        let final_span = self.next(last);
        let mut delete_span = false;
        let mut test = first;
        loop {
            delete_span |= self.remove_all_bounded(test);
            let Some(next) = self.next(test) else {
                break;
            };
            if Some(next) == final_span {
                break;
            }
            test = next;
        }
        self.span_mut(first).bounded = None;
        self.add_bounded(first, opp_first);
        // cannot call validate until remove span range is called
        delete_span
    }

    /// `static int EndsEqual(const SkTSect* sect1, const SkTSect* sect2, SkIntersections*)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1618-L1663 (chrome/m156)
    fn ends_equal(&self, sect1: SectId, sect2: SectId, intersections: &mut Intersections) -> i32 {
        let c1 = self.curve(sect1);
        let c2 = self.curve(sect2);
        let c1_first = c1.point(0);
        let c1_last = self.point_last_of_sect(sect1);
        let c2_first = c2.point(0);
        let c2_last = self.point_last_of_sect(sect2);
        let mut zero_one_set = 0;
        if c1_first == c2_first {
            zero_one_set |= K_ZERO_S1_SET | K_ZERO_S2_SET;
            intersections.insert(0.0, 0.0, c1_first);
        }
        if c1_first == c2_last {
            zero_one_set |= K_ZERO_S1_SET | K_ONE_S2_SET;
            intersections.insert(0.0, 1.0, c1_first);
        }
        if c1_last == c2_first {
            zero_one_set |= K_ONE_S1_SET | K_ZERO_S2_SET;
            intersections.insert(1.0, 0.0, c1_last);
        }
        if c1_last == c2_last {
            zero_one_set |= K_ONE_S1_SET | K_ONE_S2_SET;
            intersections.insert(1.0, 1.0, c1_last);
        }
        // check for zero
        if zero_one_set & (K_ZERO_S1_SET | K_ZERO_S2_SET) == 0
            && c1_first.approximately_equal(c2_first)
        {
            zero_one_set |= K_ZERO_S1_SET | K_ZERO_S2_SET;
            intersections.insert_near(0.0, 0.0, c1_first, c2_first);
        }
        if zero_one_set & (K_ZERO_S1_SET | K_ONE_S2_SET) == 0
            && c1_first.approximately_equal(c2_last)
        {
            zero_one_set |= K_ZERO_S1_SET | K_ONE_S2_SET;
            intersections.insert_near(0.0, 1.0, c1_first, c2_last);
        }
        // check for one
        if zero_one_set & (K_ONE_S1_SET | K_ZERO_S2_SET) == 0
            && c1_last.approximately_equal(c2_first)
        {
            zero_one_set |= K_ONE_S1_SET | K_ZERO_S2_SET;
            intersections.insert_near(1.0, 0.0, c1_last, c2_first);
        }
        if zero_one_set & (K_ONE_S1_SET | K_ONE_S2_SET) == 0 && c1_last.approximately_equal(c2_last)
        {
            zero_one_set |= K_ONE_S1_SET | K_ONE_S2_SET;
            intersections.insert_near(1.0, 1.0, c1_last, c2_last);
        }
        zero_one_set
    }

    /// `static void BinarySearch(SkTSect* sect1, SkTSect* sect2, SkIntersections* intersections)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L1792-L1994 (chrome/m156)
    #[allow(
        clippy::similar_names,
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss,
        clippy::manual_midpoint,
        clippy::float_cmp
    )] // mirrors SkTSect::BinarySearch, kept as one function with Skia's exact arithmetic
    pub(crate) fn binary_search(
        &mut self,
        sect1: SectId,
        sect2: SectId,
        intersections: &mut Intersections,
    ) {
        intersections.reset();
        intersections.set_max(self.curve(sect1).max_intersections() + 4); // give extra for slop
        let span1 = self.head(sect1).expect("sect has a head");
        let span2 = self.head(sect2).expect("sect has a head");
        let (sect, opp_sect) = self.intersects(sect1, span1, sect2, span2);
        if sect == 0 {
            return;
        }
        if sect == 2 && opp_sect == 2 {
            self.ends_equal(sect1, sect2, intersections);
            return;
        }
        self.add_bounded(span1, span2);
        self.add_bounded(span2, span1);
        let mut coin_loop_count = MAX_COIN_LOOP_COUNT;
        let mut start1s = 0.0;
        let mut start1e = 0.0;
        loop {
            // find the largest bounds
            let Some(largest1) = self.bounds_max(sect1) else {
                if self.sect(sect1).hung {
                    return;
                }
                break;
            };
            let largest2 = self.bounds_max(sect2);
            // split it
            let split_first = match largest2 {
                None => true,
                Some(l2) => {
                    self.span(largest1).bounds_max > self.span(l2).bounds_max
                        || (!self.span(largest1).collapsed && self.span(l2).collapsed)
                }
            };
            if split_first {
                if self.sect(sect2).hung {
                    return;
                }
                if self.span(largest1).collapsed {
                    break;
                }
                self.reset_removed_ends(sect1);
                self.reset_removed_ends(sect2);
                // trim parts that don't intersect the opposite
                let half1 = self.add_one(sect1);
                if !self.split(half1, largest1) {
                    break;
                }
                if !self.trim(sect1, largest1, sect2) {
                    return;
                }
                if !self.trim(sect1, half1, sect2) {
                    return;
                }
            } else {
                let largest2 = largest2.expect("split_first is false only with a largest span");
                if self.span(largest2).collapsed {
                    break;
                }
                self.reset_removed_ends(sect1);
                self.reset_removed_ends(sect2);
                // trim parts that don't intersect the opposite
                let half2 = self.add_one(sect2);
                if !self.split(half2, largest2) {
                    break;
                }
                if !self.trim(sect2, largest2, sect1) {
                    return;
                }
                if !self.trim(sect2, half2, sect1) {
                    return;
                }
            }
            // if there are 9 or more continuous spans on both sects, suspect coincidence
            if self.sect(sect1).active_count >= COINCIDENT_SPAN_COUNT
                && self.sect(sect2).active_count >= COINCIDENT_SPAN_COUNT
            {
                if coin_loop_count == MAX_COIN_LOOP_COUNT {
                    start1s = self.start_t(self.head(sect1).expect("sect has a head"));
                    start1e = self.end_t(self.tail(sect1).expect("sect has a tail"));
                }
                if !self.coincident_check(sect1, sect2) {
                    return;
                }
                coin_loop_count -= 1;
                if coin_loop_count == 0 && self.head(sect1).is_some() && self.head(sect2).is_some()
                {
                    /* All known working cases resolve in two tries. Sadly, cubicConicTests[0]
                    gets stuck in a loop. It adds an extension to allow a coincident end
                    perpendicular to track its intersection in the opposite curve. However,
                    the bounding box of the extension does not intersect the original curve,
                    so the extension is discarded, only to be added again the next time around. */
                    self.coincident_force(sect1, sect2, start1s, start1e);
                }
            }
            if self.sect(sect1).active_count >= COINCIDENT_SPAN_COUNT
                && self.sect(sect2).active_count >= COINCIDENT_SPAN_COUNT
            {
                let Some(head1) = self.head(sect1) else {
                    return;
                };
                let tail1 = self.tail(sect1);
                self.compute_perpendiculars(sect1, sect2, head1, tail1);
                let Some(head2) = self.head(sect2) else {
                    return;
                };
                let tail2 = self.tail(sect2);
                self.compute_perpendiculars(sect2, sect1, head2, tail2);
                if !self.remove_by_perpendicular(sect1, sect2) {
                    return;
                }
                if self.collapsed(sect1) > self.curve(sect1).max_intersections() as i32 {
                    break;
                }
            }
            if self.head(sect1).is_none() || self.head(sect2).is_none() {
                break;
            }
        }
        if let Some(coincident) = self.sect(sect1).coincident {
            // if there is more than one coincident span, check loosely to see if they should be joined
            if self.next(coincident).is_some() {
                self.merge_coincidence(sect1, sect2);
            }
            let mut cur = self.sect(sect1).coincident;
            loop {
                let Some(c) = cur else {
                    return;
                };
                let coin_start = self.span(c).coin_start;
                let coin_end = self.span(c).coin_end;
                if coin_start.is_match() && coin_end.is_match() {
                    let perp_t = coin_start.perp_t();
                    if perp_t < 0.0 {
                        return;
                    }
                    let index = intersections.insert_coincident(
                        self.start_t(c),
                        perp_t,
                        self.point_first(c),
                    );
                    if intersections.insert_coincident(
                        self.end_t(c),
                        coin_end.perp_t(),
                        self.point_last(c),
                    ) < 0
                        && index >= 0
                    {
                        intersections.clear_coincidence(index as usize);
                    }
                }
                cur = self.next(c);
                if cur.is_none() {
                    break;
                }
            }
        }
        let zero_one_set = self.ends_equal(sect1, sect2, intersections);
        let c1 = self.curve(sect1);
        let c2 = self.curve(sect2);
        // if the final iteration contains an end (0 or 1), intersect perpendicular with the opposite
        if self.sect(sect1).removed_start_t && zero_one_set & K_ZERO_S1_SET == 0 {
            let mut perp = TCoincident::default();
            perp.set_perp(&c1, 0.0, c1.point(0), &c2);
            if perp.is_match() {
                intersections.insert(0.0, perp.perp_t(), perp.perp_pt());
            }
        }
        if self.sect(sect1).removed_end_t && zero_one_set & K_ONE_S1_SET == 0 {
            let mut perp = TCoincident::default();
            perp.set_perp(&c1, 1.0, self.point_last_of_sect(sect1), &c2);
            if perp.is_match() {
                intersections.insert(1.0, perp.perp_t(), perp.perp_pt());
            }
        }
        if self.sect(sect2).removed_start_t && zero_one_set & K_ZERO_S2_SET == 0 {
            let mut perp = TCoincident::default();
            perp.set_perp(&c2, 0.0, c2.point(0), &c1);
            if perp.is_match() {
                intersections.insert(perp.perp_t(), 0.0, perp.perp_pt());
            }
        }
        if self.sect(sect2).removed_end_t && zero_one_set & K_ONE_S2_SET == 0 {
            let mut perp = TCoincident::default();
            perp.set_perp(&c2, 1.0, self.point_last_of_sect(sect2), &c1);
            if perp.is_match() {
                intersections.insert(perp.perp_t(), 1.0, perp.perp_pt());
            }
        }
        if self.head(sect1).is_none() || self.head(sect2).is_none() {
            return;
        }
        self.recover_collapsed(sect1);
        self.recover_collapsed(sect2);
        let mut result1 = self.head(sect1);
        // check heads and tails for zero and ones and insert them if we haven't already done so
        let head1 = result1.expect("sect has a head");
        if zero_one_set & K_ZERO_S1_SET == 0 && approximately_less_than_zero(self.start_t(head1)) {
            let start1 = c1.point(0);
            if self.is_bounded(head1) {
                let t = self.closest_bounded_t(head1, start1);
                if c2.pt_at_t(t).approximately_equal(start1) {
                    intersections.insert(0.0, t, start1);
                }
            }
        }
        let head2 = self.head(sect2).expect("sect has a head");
        if zero_one_set & K_ZERO_S2_SET == 0 && approximately_less_than_zero(self.start_t(head2)) {
            let start2 = c2.point(0);
            if self.is_bounded(head2) {
                let t = self.closest_bounded_t(head2, start2);
                if c1.pt_at_t(t).approximately_equal(start2) {
                    intersections.insert(t, 0.0, start2);
                }
            }
        }
        if zero_one_set & K_ONE_S1_SET == 0 {
            let Some(tail1) = self.tail(sect1) else {
                return;
            };
            if approximately_greater_than_one(self.end_t(tail1)) {
                let end1 = self.point_last_of_sect(sect1);
                if self.is_bounded(tail1) {
                    let t = self.closest_bounded_t(tail1, end1);
                    if c2.pt_at_t(t).approximately_equal(end1) {
                        intersections.insert(1.0, t, end1);
                    }
                }
            }
        }
        if zero_one_set & K_ONE_S2_SET == 0 {
            let Some(tail2) = self.tail(sect2) else {
                return;
            };
            if approximately_greater_than_one(self.end_t(tail2)) {
                let end2 = self.point_last_of_sect(sect2);
                if self.is_bounded(tail2) {
                    let t = self.closest_bounded_t(tail2, end2);
                    if c1.pt_at_t(t).approximately_equal(end2) {
                        intersections.insert(t, 1.0, end2);
                    }
                }
            }
        }
        let mut closest = ClosestSect::new();
        loop {
            while let Some(r) = result1 {
                if self.span(r).coin_start.is_match() && self.span(r).coin_end.is_match() {
                    result1 = self.next(r);
                } else {
                    break;
                }
            }
            let Some(r1) = result1 else {
                break;
            };
            let mut result2 = self.head(sect2);
            while let Some(r2) = result2 {
                closest.find(self, r1, r2);
                result2 = self.next(r2);
            }
            match self.next(r1) {
                Some(n) => result1 = Some(n),
                None => break,
            }
        }
        closest.finish(self, intersections);
        // if there is more than one intersection and it isn't already coincident, check
        let mut last = intersections.used() as i32 - 1;
        let mut index: i32 = 0;
        while index < last {
            let i = index as usize;
            if intersections.is_coincident(i) && intersections.is_coincident(i + 1) {
                index += 1;
                continue;
            }
            let mid_t = (intersections.t(0, i) + intersections.t(0, i + 1)) / 2.0;
            let mid_pt = c1.pt_at_t(mid_t);
            // intersect perpendicular with opposite curve
            let mut perp = TCoincident::default();
            perp.set_perp(&c1, mid_t, mid_pt, &c2);
            if !perp.is_match() {
                index += 1;
                continue;
            }
            if intersections.is_coincident(i) {
                intersections.remove_one(i);
                last -= 1;
            } else if intersections.is_coincident(i + 1) {
                intersections.remove_one(i + 1);
                last -= 1;
            } else {
                intersections.set_coincident(i);
                index += 1;
            }
            intersections.set_coincident(index as usize);
        }
        debug_assert!(intersections.used() <= c1.max_intersections());
    }
}

/// Port helpers used by the curve-pair entry points: each builds the two sections of its pair
/// and runs `SkTSect::BinarySearch` on them, as the `SkIntersections::intersect` overloads do.
impl Intersections {
    /// `int SkIntersections::intersect(const SkDQuad& q1, const SkDQuad& q2)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L2099-L2106 (chrome/m156)
    pub fn intersect_quad_quad(&mut self, q1: &DQuad, q2: &DQuad) -> usize {
        let mut arena = TSectArena::default();
        let sect1 = arena.new_sect(TCurve::Quad(*q1));
        let sect2 = arena.new_sect(TCurve::Quad(*q2));
        arena.binary_search(sect1, sect2, self);
        self.used()
    }

    /// `int SkIntersections::intersect(const SkDConic& c, const SkDQuad& q)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L2108-L2115 (chrome/m156)
    pub fn intersect_conic_quad(&mut self, c: &DConic, q: &DQuad) -> usize {
        let mut arena = TSectArena::default();
        let sect1 = arena.new_sect(TCurve::Conic(*c));
        let sect2 = arena.new_sect(TCurve::Quad(*q));
        arena.binary_search(sect1, sect2, self);
        self.used()
    }

    /// `int SkIntersections::intersect(const SkDConic& c1, const SkDConic& c2)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L2117-L2124 (chrome/m156)
    pub fn intersect_conic_conic(&mut self, c1: &DConic, c2: &DConic) -> usize {
        let mut arena = TSectArena::default();
        let sect1 = arena.new_sect(TCurve::Conic(*c1));
        let sect2 = arena.new_sect(TCurve::Conic(*c2));
        arena.binary_search(sect1, sect2, self);
        self.used()
    }

    /// `int SkIntersections::intersect(const SkDCubic& c, const SkDQuad& q)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L2126-L2133 (chrome/m156)
    pub fn intersect_cubic_quad(&mut self, c: &DCubic, q: &DQuad) -> usize {
        let mut arena = TSectArena::default();
        let sect1 = arena.new_sect(TCurve::Cubic(*c));
        let sect2 = arena.new_sect(TCurve::Quad(*q));
        arena.binary_search(sect1, sect2, self);
        self.used()
    }

    /// `int SkIntersections::intersect(const SkDCubic& cu, const SkDConic& co)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L2135-L2143 (chrome/m156)
    pub fn intersect_cubic_conic(&mut self, cu: &DCubic, co: &DConic) -> usize {
        let mut arena = TSectArena::default();
        let sect1 = arena.new_sect(TCurve::Cubic(*cu));
        let sect2 = arena.new_sect(TCurve::Conic(*co));
        arena.binary_search(sect1, sect2, self);
        self.used()
    }

    /// `int SkIntersections::intersect(const SkDCubic& c1, const SkDCubic& c2)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L2145-L2152 (chrome/m156)
    pub fn intersect_cubic_cubic(&mut self, c1: &DCubic, c2: &DCubic) -> usize {
        let mut arena = TSectArena::default();
        let sect1 = arena.new_sect(TCurve::Cubic(*c1));
        let sect2 = arena.new_sect(TCurve::Cubic(*c2));
        arena.binary_search(sect1, sect2, self);
        self.used()
    }
}
