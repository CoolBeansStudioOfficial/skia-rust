// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkOpSpan.h (SkOpPtT, SkOpSpanBase, SkOpSpan),
// src/pathops/SkOpSpan.cpp (their methods)

//! The points and spans of the op graph (`SkOpPtT`, `SkOpSpanBase`, `SkOpSpan`).
//!
//! Skia's `SkOpSpanBase` embeds an `SkOpPtT` and `SkOpSpan` extends `SkOpSpanBase`. Both live
//! in one arena here: a [`Span`] holds every field of both classes, and the base-only nodes
//! (a segment's tail) simply never read the `SkOpSpan` fields. Pointers are [`SpanId`] and
//! [`PtTId`] values; "points to self" is the id of the record itself.

use skia_rust_core::point::Point;

use crate::op_angle::AngleId;
use crate::op_state::{ContourId, MAX_WINDING_TRIES, OpState, PtTId, SK_MIN_S32, SegId, SpanId};
use crate::types::{between, std_min, std_max, zero_or_one};

/// `SkOpPtT`: a point on a segment, at parameter `t`, linked into a ring of points that
/// describe the same place.
// Port of: src/pathops/SkOpSpan.h#L19-L58 (chrome/m156)
#[doc(alias = "SkOpPtT")]
#[derive(Copy, Clone, Debug)]
pub struct PtT {
    /// `double fT`.
    pub(crate) t: f64,
    /// `SkPoint fPt`: cache of the point value at this `t`.
    pub(crate) pt: Point,
    /// `SkOpSpanBase* fSpan`: contains winding data.
    pub(crate) span: SpanId,
    /// `SkOpPtT* fNext`: intersection on the opposite curve, or alias on this curve.
    pub(crate) next: PtTId,
    /// `bool fDeleted`: set if removed from the span list.
    pub(crate) deleted: bool,
    /// `bool fDuplicatePt`: set if an identical point is somewhere in the next loop.
    pub(crate) duplicate_pt: bool,
    /// `mutable bool fCoincident`: set if at some point a coincident span pointed here.
    pub(crate) coincident: bool,
}

impl PtT {
    /// A point record on `span`, looping to itself (`SkOpPtT::init` sets the rest).
    pub(crate) fn new(span: SpanId) -> Self {
        Self {
            t: 0.0,
            pt: Point::default(),
            span,
            next: PtTId(usize::MAX),
            deleted: false,
            duplicate_pt: false,
            coincident: false,
        }
    }
}

/// `SkOpSpanBase` and `SkOpSpan` in one record. The base fields come first, the `SkOpSpan`
/// fields after. The tail of a segment is a base-only span.
// Port of: src/pathops/SkOpSpan.h#L60-L166 (chrome/m156)
#[doc(alias = "SkOpSpanBase")]
#[doc(alias = "SkOpSpan")]
#[derive(Copy, Clone, Debug)]
pub struct Span {
    // --- SkOpSpanBase ---
    /// `SkOpPtT fPtT`: the point list and t value at the start of this span.
    pub(crate) ptt: PtTId,
    /// `SkOpSegment* fSegment`: the segment that contains this span.
    pub(crate) segment: SegId,
    /// `SkOpSpanBase* fCoinEnd`: coincident spans that end here (may point to itself).
    pub(crate) coin_end: SpanId,
    /// `SkOpAngle* fFromAngle`: the next angle from span start to end.
    pub(crate) from_angle: Option<AngleId>,
    /// `SkOpSpan* fPrev`: the previous intersection point.
    pub(crate) prev: Option<SpanId>,
    /// `int fSpanAdds`: number of times intersections were added to this span.
    pub(crate) span_adds: i32,
    /// `bool fAligned`.
    pub(crate) aligned: bool,
    /// `bool fChased`: set after the span was added to the chase array.
    pub(crate) chased: bool,

    // --- SkOpSpan ---
    /// `SkOpSpan* fCoincident`: spans coincident with this one (may point to itself).
    pub(crate) coincident: SpanId,
    /// `SkOpAngle* fToAngle`: the next angle from span start to end.
    pub(crate) to_angle: Option<AngleId>,
    /// `SkOpSpanBase* fNext`: the next intersection point.
    pub(crate) next: Option<SpanId>,
    /// `int fWindSum`: accumulated from contours surrounding this one.
    pub(crate) wind_sum: i32,
    /// `int fOppSum`: for binary operators, the opposite winding sum.
    pub(crate) opp_sum: i32,
    /// `int fWindValue`: 0 == canceled; 1 == normal; >1 == coincident.
    pub(crate) wind_value: i32,
    /// `int fOppValue`: normally 0; when binary coincident edges combine, the opp value goes here.
    pub(crate) opp_value: i32,
    /// `int fTopTTry`: specifies direction and t value to try next.
    pub(crate) top_t_try: i32,
    /// `bool fDone`: if set, this span to the next higher T has been processed.
    pub(crate) done: bool,
    /// `bool fAlreadyAdded`.
    pub(crate) already_added: bool,
}

impl Span {
    /// A span whose point record is `ptt`. The fields are set by `init_base` / `init`.
    pub(crate) fn new(ptt: PtTId) -> Self {
        Self {
            ptt,
            segment: SegId(usize::MAX),
            coin_end: SpanId(usize::MAX),
            from_angle: None,
            prev: None,
            span_adds: 0,
            aligned: true,
            chased: false,
            coincident: SpanId(usize::MAX),
            to_angle: None,
            next: None,
            wind_sum: SK_MIN_S32,
            opp_sum: SK_MIN_S32,
            wind_value: 1,
            opp_value: 0,
            top_t_try: 0,
            done: false,
            already_added: false,
        }
    }
}

/// `SkOpSpanBase::Collapsed`.
// Port of: src/pathops/SkOpSpan.h#L69-L73 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Collapsed {
    No,
    Yes,
    Error,
}

impl OpState {
    /// `SkOpPtT::span()` (the span that owns this point record).
    #[must_use]
    pub(crate) fn ptt_span(&self, p: PtTId) -> SpanId {
        self.ptts[p.0].span
    }

    /// `SkOpPtT::next()`.
    #[must_use]
    pub(crate) fn ptt_next(&self, p: PtTId) -> PtTId {
        self.ptts[p.0].next
    }

    /// `SkOpPtT::segment()`.
    #[must_use]
    pub(crate) fn ptt_segment(&self, p: PtTId) -> SegId {
        self.spans[self.ptts[p.0].span.0].segment
    }

    /// `SkOpPtT::contour()`.
    #[must_use]
    pub(crate) fn ptt_contour(&self, p: PtTId) -> ContourId {
        self.segments[self.ptt_segment(p).0].contour
    }

    /// `SkOpPtT::deleted()`.
    #[must_use]
    pub(crate) fn ptt_deleted(&self, p: PtTId) -> bool {
        self.ptts[p.0].deleted
    }

    /// `SkOpPtT::coincident()`.
    #[must_use]
    pub(crate) fn ptt_coincident(&self, p: PtTId) -> bool {
        self.ptts[p.0].coincident
    }

    /// `SkOpPtT::setCoincident() const`.
    pub(crate) fn ptt_set_coincident(&mut self, p: PtTId) {
        self.ptts[p.0].coincident = true;
    }

    /// `SkOpPtT::duplicate()`.
    #[must_use]
    pub(crate) fn ptt_duplicate(&self, p: PtTId) -> bool {
        self.ptts[p.0].duplicate_pt
    }

    /// `SkOpPtT::setDeleted()`.
    pub(crate) fn ptt_set_deleted(&mut self, p: PtTId) {
        self.ptts[p.0].deleted = true;
    }

    /// `SkOpPtT::setSpan(span)`.
    pub(crate) fn ptt_set_span(&mut self, p: PtTId, span: SpanId) {
        self.ptts[p.0].span = span;
    }

    /// `SkOpPtT::addOpp(opp, oppPrev)`: `this` is followed by `opp`, and `oppPrev` by the old
    /// successor of `this`.
    // Port of: src/pathops/SkOpSpan.h#L27-L34 (chrome/m156)
    pub(crate) fn ptt_add_opp(&mut self, this: PtTId, opp: PtTId, opp_prev: PtTId) {
        let old_next = self.ptts[this.0].next;
        self.ptts[this.0].next = opp;
        self.ptts[opp_prev.0].next = old_next;
    }

    /// `SkOpPtT::alias()`: this point is not the one its span starts with.
    // Port of: src/pathops/SkOpSpan.cpp#L23-L25 (chrome/m156)
    #[must_use]
    pub(crate) fn ptt_alias(&self, p: PtTId) -> bool {
        self.spans[self.ptts[p.0].span.0].ptt != p
    }

    /// `SkOpPtT::active()`: this point, or a live point of the same span.
    // Port of: src/pathops/SkOpSpan.cpp#L27-L39 (chrome/m156)
    #[must_use]
    pub(crate) fn ptt_active(&self, p: PtTId) -> Option<PtTId> {
        if !self.ptts[p.0].deleted {
            return Some(p);
        }
        let stop = p;
        let mut ptt = p;
        loop {
            ptt = self.ptts[ptt.0].next;
            if ptt == stop {
                break;
            }
            if self.ptts[ptt.0].span == self.ptts[p.0].span && !self.ptts[ptt.0].deleted {
                return Some(ptt);
            }
        }
        None
    }

    /// `SkOpPtT::contains(const SkOpPtT* check)`: `check` is in this point's ring.
    // Port of: src/pathops/SkOpSpan.cpp#L41-L50 (chrome/m156)
    #[must_use]
    pub(crate) fn ptt_contains_ptt(&self, p: PtTId, check: PtTId) -> bool {
        let stop = p;
        let mut ptt = p;
        loop {
            ptt = self.ptts[ptt.0].next;
            if ptt == stop {
                return false;
            }
            if ptt == check {
                return true;
            }
        }
    }

    /// `SkOpPtT::contains(const SkOpSegment*, const SkPoint&)`.
    // Port of: src/pathops/SkOpSpan.cpp#L52-L62 (chrome/m156)
    #[must_use]
    pub(crate) fn ptt_contains_seg_pt(&self, p: PtTId, segment: SegId, pt: Point) -> bool {
        let stop = p;
        let mut ptt = p;
        loop {
            ptt = self.ptts[ptt.0].next;
            if ptt == stop {
                return false;
            }
            if self.ptts[ptt.0].pt == pt && self.ptt_segment(ptt) == segment {
                return true;
            }
        }
    }

    /// `SkOpPtT::contains(const SkOpSegment*, double t)`.
    // Port of: src/pathops/SkOpSpan.cpp#L64-L74 (chrome/m156)
    #[must_use]
    pub(crate) fn ptt_contains_seg_t(&self, p: PtTId, segment: SegId, t: f64) -> bool {
        let stop = p;
        let mut ptt = p;
        loop {
            ptt = self.ptts[ptt.0].next;
            if ptt == stop {
                return false;
            }
            if self.ptts[ptt.0].t == t && self.ptt_segment(ptt) == segment {
                return true;
            }
        }
    }

    /// `SkOpPtT::contains(const SkOpSegment*)`: the first live point on `segment` in the ring.
    // Port of: src/pathops/SkOpSpan.cpp#L76-L86 (chrome/m156)
    #[must_use]
    pub(crate) fn ptt_contains_seg(&self, p: PtTId, segment: SegId) -> Option<PtTId> {
        let stop = p;
        let mut ptt = p;
        loop {
            ptt = self.ptts[ptt.0].next;
            if ptt == stop {
                return None;
            }
            if self.ptt_segment(ptt) == segment && !self.ptts[ptt.0].deleted {
                return Some(ptt);
            }
        }
    }

    /// `SkOpPtT::find(const SkOpSegment*)`: starts with this point, then walks the ring.
    // Port of: src/pathops/SkOpSpan.cpp#L97-L109 (chrome/m156)
    #[must_use]
    pub(crate) fn ptt_find(&self, p: PtTId, segment: SegId) -> Option<PtTId> {
        let stop = p;
        let mut ptt = p;
        loop {
            if self.ptt_segment(ptt) == segment && !self.ptts[ptt.0].deleted {
                return Some(ptt);
            }
            ptt = self.ptts[ptt.0].next;
            if ptt == stop {
                return None;
            }
        }
    }

    /// `SkOpPtT::init(span, t, pt, duplicate)`.
    // Port of: src/pathops/SkOpSpan.cpp#L118-L127 (chrome/m156)
    pub(crate) fn ptt_init(&mut self, p: PtTId, span: SpanId, t: f64, pt: Point, duplicate: bool) {
        let ptt = &mut self.ptts[p.0];
        ptt.t = t;
        ptt.pt = pt;
        ptt.span = span;
        ptt.next = p;
        ptt.duplicate_pt = duplicate;
        ptt.deleted = false;
        ptt.coincident = false;
    }

    /// `SkOpPtT::onEnd()`: this point is the first or last point of its segment.
    // Port of: src/pathops/SkOpSpan.cpp#L129-L136 (chrome/m156)
    #[must_use]
    pub(crate) fn ptt_on_end(&self, p: PtTId) -> bool {
        let span = self.ptts[p.0].span;
        if self.spans[span.0].ptt != p {
            return false;
        }
        let segment = self.spans[span.0].segment;
        span == self.segments[segment.0].head || span == self.segments[segment.0].tail
    }

    /// `SkOpPtT::ptAlreadySeen(const SkOpPtT* check)`: `check` onward, up to `this`, has a
    /// point equal to this one's.
    // Port of: src/pathops/SkOpSpan.cpp#L138-L146 (chrome/m156)
    #[must_use]
    pub(crate) fn ptt_pt_already_seen(&self, p: PtTId, check: PtTId) -> bool {
        let mut check = check;
        while p != check {
            if self.ptts[p.0].pt == self.ptts[check.0].pt {
                return true;
            }
            check = self.ptts[check.0].next;
        }
        false
    }

    /// `SkOpPtT::prev()`: the point before this one in the ring (`this` if alone).
    // Port of: src/pathops/SkOpSpan.cpp#L148-L156 (chrome/m156)
    #[must_use]
    pub(crate) fn ptt_prev(&self, p: PtTId) -> PtTId {
        let mut result = p;
        let mut next = p;
        loop {
            next = self.ptts[next.0].next;
            if next == p {
                break;
            }
            result = next;
        }
        result
    }

    /// `SkOpPtT::oppPrev(const SkOpPtT* opp)`: the point before `opp` in this ring, or
    /// `None` if `opp` is already in this ring.
    // Port of: src/pathops/SkOpSpan.h#L36-L48 (chrome/m156)
    #[must_use]
    pub(crate) fn ptt_opp_prev(&self, p: PtTId, opp: PtTId) -> Option<PtTId> {
        let mut opp_prev = self.ptts[opp.0].next;
        if opp_prev == p {
            return None;
        }
        while self.ptts[opp_prev.0].next != opp {
            opp_prev = self.ptts[opp_prev.0].next;
            if opp_prev == p {
                return None;
            }
        }
        Some(opp_prev)
    }

    /// `SkOpPtT::insert(span)`: links `span` in after this point.
    // Port of: src/pathops/SkOpSpan.h#L92-L96 (chrome/m156)
    pub(crate) fn ptt_insert(&mut self, p: PtTId, span: PtTId) {
        self.ptts[span.0].next = self.ptts[p.0].next;
        self.ptts[p.0].next = span;
    }

    /// `SkOpPtT::starter(end)`: the point with the smaller `t`.
    // Port of: src/pathops/SkOpSpan.h#L143-L145 (chrome/m156)
    #[must_use]
    pub(crate) fn ptt_starter(&self, p: PtTId, end: PtTId) -> PtTId {
        if self.ptts[p.0].t < self.ptts[end.0].t {
            p
        } else {
            end
        }
    }

    /// `SkOpPtT::Overlaps(s1, e1, s2, e2, &sOut, &eOut)`.
    // Port of: src/pathops/SkOpSpan.h#L99-L118 (chrome/m156)
    #[must_use]
    pub(crate) fn ptt_overlaps(
        &self,
        s1: PtTId,
        e1: PtTId,
        s2: PtTId,
        e2: PtTId,
    ) -> (bool, Option<PtTId>, Option<PtTId>) {
        let t = |p: PtTId| self.ptts[p.0].t;
        let start1 = if t(s1) < t(e1) { s1 } else { e1 };
        let start2 = if t(s2) < t(e2) { s2 } else { e2 };
        let s_out = if between(t(s1), t(start2), t(e1)) {
            Some(start2)
        } else if between(t(s2), t(start1), t(e2)) {
            Some(start1)
        } else {
            None
        };
        let end1 = if t(s1) < t(e1) { e1 } else { s1 };
        let end2 = if t(s2) < t(e2) { e2 } else { s2 };
        let e_out = if between(t(s1), t(end2), t(e1)) {
            Some(end2)
        } else if between(t(s2), t(end1), t(e2)) {
            Some(end1)
        } else {
            None
        };
        if s_out == e_out {
            return (false, s_out, e_out);
        }
        (s_out.is_some() && e_out.is_some(), s_out, e_out)
    }

    /// `SkOpSpanBase::pt()`: the point cache of the span's own point record.
    #[must_use]
    pub(crate) fn span_pt(&self, s: SpanId) -> Point {
        self.ptts[self.spans[s.0].ptt.0].pt
    }

    /// `SkOpSpanBase::t()`.
    #[must_use]
    pub(crate) fn span_t(&self, s: SpanId) -> f64 {
        self.ptts[self.spans[s.0].ptt.0].t
    }

    /// `SkOpSpanBase::ptT()`.
    #[must_use]
    pub(crate) fn span_ptt(&self, s: SpanId) -> PtTId {
        self.spans[s.0].ptt
    }

    /// `SkOpSpanBase::segment()`.
    #[must_use]
    pub(crate) fn span_segment(&self, s: SpanId) -> SegId {
        self.spans[s.0].segment
    }

    /// `SkOpSpanBase::final()`: this span ends at `t == 1`.
    #[must_use]
    pub(crate) fn span_final(&self, s: SpanId) -> bool {
        self.span_t(s) == 1.0
    }

    /// `SkOpSpanBase::upCastable()`: this span as an `SkOpSpan`, unless it is the tail.
    #[must_use]
    pub(crate) fn span_up_castable(&self, s: SpanId) -> Option<SpanId> {
        if self.span_final(s) { None } else { Some(s) }
    }

    /// `SkOpSpanBase::upCast()`: this span as an `SkOpSpan` (not the tail).
    #[must_use]
    pub(crate) fn span_up_cast(&self, s: SpanId) -> SpanId {
        debug_assert!(!self.span_final(s));
        s
    }

    /// `SkOpSpanBase::prev()`.
    #[must_use]
    pub(crate) fn span_prev(&self, s: SpanId) -> Option<SpanId> {
        self.spans[s.0].prev
    }

    /// `SkOpSpanBase::fromAngle()`.
    #[must_use]
    pub(crate) fn span_from_angle(&self, s: SpanId) -> Option<AngleId> {
        self.spans[s.0].from_angle
    }

    /// `SkOpSpanBase::setFromAngle(angle)`.
    pub(crate) fn span_set_from_angle(&mut self, s: SpanId, angle: Option<AngleId>) {
        self.spans[s.0].from_angle = angle;
    }

    /// `SkOpSpanBase::setPrev(prev)`.
    pub(crate) fn span_set_prev(&mut self, s: SpanId, prev: Option<SpanId>) {
        self.spans[s.0].prev = prev;
    }

    /// `SkOpSpan::next()`.
    #[must_use]
    pub(crate) fn span_next(&self, s: SpanId) -> Option<SpanId> {
        self.spans[s.0].next
    }

    /// `SkOpSpan::setNext(nextT)`.
    pub(crate) fn span_set_next(&mut self, s: SpanId, next: Option<SpanId>) {
        self.spans[s.0].next = next;
    }

    /// `SkOpSpanBase::deleted()`.
    #[must_use]
    pub(crate) fn span_deleted(&self, s: SpanId) -> bool {
        self.ptts[self.spans[s.0].ptt.0].deleted
    }

    /// `SkOpSpanBase::spanAddsCount()`.
    #[must_use]
    pub(crate) fn span_adds_count(&self, s: SpanId) -> i32 {
        self.spans[s.0].span_adds
    }

    /// `SkOpSpanBase::bumpSpanAdds()`.
    pub(crate) fn span_bump_span_adds(&mut self, s: SpanId) {
        self.spans[s.0].span_adds += 1;
    }

    /// `SkOpSpanBase::chased()`.
    #[must_use]
    pub(crate) fn span_chased(&self, s: SpanId) -> bool {
        self.spans[s.0].chased
    }

    /// `SkOpSpanBase::setChased(chased)`.
    pub(crate) fn span_set_chased(&mut self, s: SpanId, chased: bool) {
        self.spans[s.0].chased = chased;
    }

    /// `SkOpSpanBase::final()` and `SkOpSpanBase::setAligned()` / `unaligned()`.
    pub(crate) fn span_set_aligned(&mut self, s: SpanId, aligned: bool) {
        self.spans[s.0].aligned = aligned;
    }

    /// `SkOpSpanBase::coinEnd()`.
    #[must_use]
    pub(crate) fn span_coin_end(&self, s: SpanId) -> SpanId {
        self.spans[s.0].coin_end
    }

    /// `SkOpSpanBase::containsCoinEnd(coin)`: `coin` is in this span's coincident-end ring.
    // Port of: src/pathops/SkOpSpan.h#L124-L134 (chrome/m156)
    #[must_use]
    pub(crate) fn span_contains_coin_end(&self, s: SpanId, coin: SpanId) -> bool {
        let mut next = s;
        loop {
            next = self.spans[next.0].coin_end;
            if next == s {
                return false;
            }
            if next == coin {
                return true;
            }
        }
    }

    /// `SkOpSpanBase::containsCoinEnd(const SkOpSegment*)`.
    // Port of: src/pathops/SkOpSpan.cpp#L257-L264 (chrome/m156)
    #[must_use]
    pub(crate) fn span_contains_coin_end_seg(&self, s: SpanId, segment: SegId) -> bool {
        let mut next = s;
        loop {
            next = self.spans[next.0].coin_end;
            if next == s {
                return false;
            }
            if self.spans[next.0].segment == segment {
                return true;
            }
        }
    }

    /// `SkOpSpanBase::insertCoinEnd(coin)`.
    // Port of: src/pathops/SkOpSpan.h#L136-L148 (chrome/m156)
    pub(crate) fn span_insert_coin_end(&mut self, s: SpanId, coin: SpanId) {
        if self.span_contains_coin_end(s, coin) {
            return;
        }
        let coin_next = self.spans[coin.0].coin_end;
        self.spans[coin.0].coin_end = self.spans[s.0].coin_end;
        self.spans[s.0].coin_end = coin_next;
    }

    /// `SkOpSpanBase::contains(const SkOpSpanBase* span)`: `span`'s point record is in this
    /// span's ring.
    // Port of: src/pathops/SkOpSpan.cpp#L165-L175 (chrome/m156)
    #[must_use]
    pub(crate) fn span_contains_span(&self, s: SpanId, span: SpanId) -> bool {
        let start = self.spans[s.0].ptt;
        let check = self.spans[span.0].ptt;
        let mut walk = start;
        loop {
            walk = self.ptts[walk.0].next;
            if walk == start {
                return false;
            }
            if walk == check {
                return true;
            }
        }
    }

    /// `SkOpSpanBase::contains(const SkOpSegment*)`: the first live point on `segment`
    /// that is the start of its own span.
    // Port of: src/pathops/SkOpSpan.cpp#L177-L190 (chrome/m156)
    #[must_use]
    pub(crate) fn span_contains_seg(&self, s: SpanId, segment: SegId) -> Option<PtTId> {
        let start = self.spans[s.0].ptt;
        let mut walk = start;
        loop {
            walk = self.ptts[walk.0].next;
            if walk == start {
                return None;
            }
            if self.ptts[walk.0].deleted {
                continue;
            }
            if self.ptt_segment(walk) == segment && self.spans[self.ptts[walk.0].span.0].ptt == walk
            {
                return Some(walk);
            }
        }
    }

    /// `SkOpSpanBase::collapsed(double s, double e)`.
    // Port of: src/pathops/SkOpSpan.cpp#L100-L129 (chrome/m156)
    #[must_use]
    pub(crate) fn span_collapsed(&self, s: SpanId, start_t: f64, end_t: f64) -> Collapsed {
        let start = self.spans[s.0].ptt;
        let mut start_next: Option<PtTId> = None;
        let mut walk = start;
        let mut min = self.ptts[walk.0].t;
        let mut max = min;
        let segment = self.span_segment(s);
        let mut safety_net = 100_000;
        loop {
            walk = self.ptts[walk.0].next;
            if walk == start {
                break;
            }
            safety_net -= 1;
            if safety_net == 0 {
                return Collapsed::Error;
            }
            if Some(walk) == start_next {
                return Collapsed::Error;
            }
            if self.ptt_segment(walk) != segment {
                continue;
            }
            min = std_min(min, self.ptts[walk.0].t);
            max = std_max(max, self.ptts[walk.0].t);
            if between(min, start_t, max) && between(min, end_t, max) {
                return Collapsed::Yes;
            }
            start_next = Some(self.ptts[start.0].next);
        }
        Collapsed::No
    }

    /// `SkOpSpanBase::initBase(segment, prev, t, pt)`.
    // Port of: src/pathops/SkOpSpan.cpp#L210-L220 (chrome/m156)
    pub(crate) fn span_init_base(
        &mut self,
        s: SpanId,
        segment: SegId,
        prev: Option<SpanId>,
        t: f64,
        pt: Point,
    ) {
        self.spans[s.0].segment = segment;
        let ptt = self.spans[s.0].ptt;
        self.ptt_init(ptt, s, t, pt, false);
        self.spans[s.0].coin_end = s;
        self.spans[s.0].from_angle = None;
        self.spans[s.0].prev = prev;
        self.spans[s.0].span_adds = 0;
        self.spans[s.0].aligned = true;
        self.spans[s.0].chased = false;
    }

    /// `SkOpSpanBase::merge(SkOpSpan* span)`: `span`'s points join this span's ring.
    // Port of: src/pathops/SkOpSpan.cpp#L222-L257 (chrome/m156)
    pub(crate) fn span_merge(&mut self, s: SpanId, span: SpanId) {
        let span_ptt = self.spans[span.0].ptt;
        let this_ptt = self.spans[s.0].ptt;
        self.span_release(span, this_ptt);
        if self.span_contains_span(s, span) {
            return; // merge is already in the ptT loop
        }
        let mut remainder = self.ptts[span_ptt.0].next;
        self.ptt_insert(this_ptt, span_ptt);
        while remainder != span_ptt {
            let next = self.ptts[remainder.0].next;
            let mut compare = self.ptts[span_ptt.0].next;
            let mut duplicate = false;
            while compare != span_ptt {
                let next_c = self.ptts[compare.0].next;
                if self.ptt_span(next_c) == self.ptt_span(remainder)
                    && self.ptts[next_c.0].t == self.ptts[remainder.0].t
                {
                    duplicate = true;
                    break;
                }
                compare = next_c;
            }
            if !duplicate {
                self.ptt_insert(span_ptt, remainder);
            }
            remainder = next;
        }
        let adds = self.spans[span.0].span_adds;
        self.spans[s.0].span_adds += adds;
    }

    /// `SkOpSpanBase::checkForCollapsedCoincidence()`.
    // Port of: src/pathops/SkOpSpan.cpp#L259-L270 (chrome/m156)
    pub(crate) fn span_check_for_collapsed_coincidence(&mut self, s: SpanId) {
        if self.coincidence_is_empty() {
            return;
        }
        let head = self.spans[s.0].ptt;
        let mut test = head;
        loop {
            if self.ptts[test.0].coincident {
                self.coin_mark_collapsed(test);
            }
            test = self.ptts[test.0].next;
            if test == head {
                break;
            }
        }
        self.coin_release_deleted();
    }

    /// `SkOpSpanBase::addOpp(SkOpSpanBase* opp)`.
    // Port of: src/pathops/SkOpSpan.cpp#L65-L76 (chrome/m156)
    pub(crate) fn span_add_opp(&mut self, s: SpanId, opp: SpanId) -> bool {
        let this_ptt = self.spans[s.0].ptt;
        let opp_ptt = self.spans[opp.0].ptt;
        let Some(opp_prev) = self.ptt_opp_prev(this_ptt, opp_ptt) else {
            return true;
        };
        if !self.span_merge_matches(s, opp) {
            return false;
        }
        self.ptt_add_opp(this_ptt, opp_ptt, opp_prev);
        self.span_check_for_collapsed_coincidence(s);
        true
    }

    /// `SkOpSpanBase::mergeMatches(SkOpSpanBase* opp)`.
    // Port of: src/pathops/SkOpSpan.cpp#L278-L332 (chrome/m156)
    pub(crate) fn span_merge_matches(&mut self, s: SpanId, opp: SpanId) -> bool {
        let stop = self.spans[s.0].ptt;
        let mut test = stop;
        let mut safety_hatch = 1_000_000;
        loop {
            safety_hatch -= 1;
            if safety_hatch == 0 {
                return false;
            }
            let test_next = self.ptts[test.0].next;
            if self.ptts[test.0].deleted {
                if test_next == stop {
                    break;
                }
                test = test_next;
                continue;
            }
            let test_base = self.ptts[test.0].span;
            let segment = self.ptt_segment(test);
            if self.seg_done(segment) {
                if test_next == stop {
                    break;
                }
                test = test_next;
                continue;
            }
            let inner_stop = self.spans[opp.0].ptt;
            let mut inner = inner_stop;
            loop {
                if self.ptt_segment(inner) == segment && !self.ptts[inner.0].deleted {
                    let inner_base = self.ptts[inner.0].span;
                    // when the intersection is first detected, the span base is marked if there
                    // are more than one point in the intersection.
                    if !zero_or_one(self.ptts[inner.0].t) {
                        self.span_release(inner_base, test);
                    } else if !zero_or_one(self.ptts[test.0].t) {
                        self.span_release(test_base, inner);
                    } else {
                        self.seg_mark_all_done(segment); // mark segment as collapsed
                        self.ptt_set_deleted(test);
                        self.ptt_set_deleted(inner);
                    }
                    break;
                }
                inner = self.ptts[inner.0].next;
                if inner == inner_stop {
                    break;
                }
            }
            if test_next == stop {
                break;
            }
            test = test_next;
        }
        self.span_check_for_collapsed_coincidence(s);
        true
    }

    /// `SkOpSpan::release(const SkOpPtT* kept)`.
    // Port of: src/pathops/SkOpSpan.cpp#L334-L357 (chrome/m156)
    pub(crate) fn span_release(&mut self, s: SpanId, kept: PtTId) {
        let prev = self.spans[s.0].prev.expect("released span has a previous span");
        let next = self.spans[s.0].next.expect("released span has a next span");
        self.spans[prev.0].next = Some(next);
        self.spans[next.0].prev = Some(prev);
        let segment = self.span_segment(s);
        self.seg_release(segment, s);
        self.coin_fix_up(self.spans[s.0].ptt, kept);
        let this_ptt = self.spans[s.0].ptt;
        self.ptt_set_deleted(this_ptt);
        let stop = this_ptt;
        let mut test_ptt = stop;
        let kept_span = self.ptt_span(kept);
        loop {
            if s == self.ptts[test_ptt.0].span {
                self.ptts[test_ptt.0].span = kept_span;
            }
            test_ptt = self.ptts[test_ptt.0].next;
            if test_ptt == stop {
                break;
            }
        }
    }

    /// `SkOpSpan::computeWindSum()`.
    // Port of: src/pathops/SkOpSpan.cpp#L359-L366 (chrome/m156)
    pub(crate) fn span_compute_wind_sum(&mut self, s: SpanId) -> i32 {
        let contour_head = self.contour_head();
        let mut wind_try = 0;
        loop {
            if self.span_sortable_top(s, contour_head) {
                break;
            }
            wind_try += 1;
            if wind_try >= MAX_WINDING_TRIES {
                break;
            }
        }
        self.spans[s.0].wind_sum
    }

    /// `SkOpSpan::containsCoincidence(const SkOpSegment*)`.
    // Port of: src/pathops/SkOpSpan.cpp#L368-L378 (chrome/m156)
    #[must_use]
    pub(crate) fn span_contains_coincidence_seg(&self, s: SpanId, segment: SegId) -> bool {
        let mut next = self.spans[s.0].coincident;
        loop {
            if self.spans[next.0].segment == segment {
                return true;
            }
            next = self.spans[next.0].coincident;
            if next == s {
                return false;
            }
        }
    }

    /// `SkOpSpan::containsCoincidence(const SkOpSpan* coin)`.
    // Port of: src/pathops/SkOpSpan.h#L166-L175 (chrome/m156)
    #[must_use]
    pub(crate) fn span_contains_coincidence(&self, s: SpanId, coin: SpanId) -> bool {
        let mut next = s;
        loop {
            next = self.spans[next.0].coincident;
            if next == s {
                return false;
            }
            if next == coin {
                return true;
            }
        }
    }

    /// `SkOpSpan::init(segment, prev, t, pt)`.
    // Port of: src/pathops/SkOpSpan.cpp#L380-L392 (chrome/m156)
    pub(crate) fn span_init(
        &mut self,
        s: SpanId,
        segment: SegId,
        prev: Option<SpanId>,
        t: f64,
        pt: Point,
    ) {
        self.span_init_base(s, segment, prev, t, pt);
        self.spans[s.0].coincident = s;
        self.spans[s.0].to_angle = None;
        self.spans[s.0].wind_sum = SK_MIN_S32;
        self.spans[s.0].opp_sum = SK_MIN_S32;
        self.spans[s.0].wind_value = 1;
        self.spans[s.0].opp_value = 0;
        self.spans[s.0].top_t_try = 0;
        self.spans[s.0].chased = false;
        self.spans[s.0].done = false;
        self.seg_bump_count(segment);
        self.spans[s.0].already_added = false;
    }

    /// `SkOpSpan::insertCoincidence(const SkOpSegment*, bool flipped, bool ordered)`.
    // Port of: src/pathops/SkOpSpan.cpp#L394-L431 (chrome/m156)
    pub(crate) fn span_insert_coincidence_seg(
        &mut self,
        s: SpanId,
        segment: SegId,
        flipped: bool,
        ordered: bool,
    ) -> bool {
        if self.span_contains_coincidence_seg(s, segment) {
            return true;
        }
        let start_ptt = self.spans[s.0].ptt;
        let mut next = start_ptt;
        loop {
            next = self.ptts[next.0].next;
            if next == start_ptt {
                break;
            }
            if self.ptt_segment(next) == segment {
                let base = self.ptts[next.0].span;
                let span = if !ordered {
                    let Some(span_end_ptt) = self.span_contains_seg(self.spans[s.0].next.expect("span has next"), segment) else {
                        return false;
                    };
                    let span_end = self.ptts[span_end_ptt.0].span;
                    let start = self.ptt_starter(self.spans[base.0].ptt, self.spans[span_end.0].ptt);
                    let start_span = self.ptts[start.0].span;
                    let Some(start_span) = self.span_up_castable(start_span) else {
                        return false;
                    };
                    start_span
                } else if flipped {
                    let Some(span) = self.spans[base.0].prev else {
                        return false;
                    };
                    span
                } else {
                    let Some(span) = self.span_up_castable(base) else {
                        return false;
                    };
                    span
                };
                self.span_insert_coincidence(s, span);
                return true;
            }
        }
        true
    }

    /// `SkOpSpan::insertCoincidence(SkOpSpan* coin)`.
    // Port of: src/pathops/SkOpSpan.h#L177-L188 (chrome/m156)
    pub(crate) fn span_insert_coincidence(&mut self, s: SpanId, coin: SpanId) {
        if self.span_contains_coincidence(s, coin) {
            return;
        }
        let coin_next = self.spans[coin.0].coincident;
        self.spans[coin.0].coincident = self.spans[s.0].coincident;
        self.spans[s.0].coincident = coin_next;
    }

    /// `SkOpSpan::setOppSum(int oppSum)`.
    // Port of: src/pathops/SkOpSpan.cpp#L440-L449 (chrome/m156)
    pub(crate) fn span_set_opp_sum(&mut self, s: SpanId, opp_sum: i32) {
        if self.spans[s.0].opp_sum != SK_MIN_S32 && self.spans[s.0].opp_sum != opp_sum {
            self.set_winding_failed();
            return;
        }
        self.spans[s.0].opp_sum = opp_sum;
    }

    /// `SkOpSpan::setWindSum(int windSum)`.
    // Port of: src/pathops/SkOpSpan.cpp#L451-L458 (chrome/m156)
    pub(crate) fn span_set_wind_sum(&mut self, s: SpanId, wind_sum: i32) {
        if self.spans[s.0].wind_sum != SK_MIN_S32 && self.spans[s.0].wind_sum != wind_sum {
            self.set_winding_failed();
            return;
        }
        self.spans[s.0].wind_sum = wind_sum;
    }

    /// `SkOpSpan::isCanceled()`.
    #[must_use]
    pub(crate) fn span_is_canceled(&self, s: SpanId) -> bool {
        self.spans[s.0].wind_value == 0 && self.spans[s.0].opp_value == 0
    }

    /// `SkOpSpan::isCoincident()`.
    #[must_use]
    pub(crate) fn span_is_coincident(&self, s: SpanId) -> bool {
        self.spans[s.0].coincident != s
    }

    /// `SkOpSpan::clearCoincident()`.
    pub(crate) fn span_clear_coincident(&mut self, s: SpanId) -> bool {
        if self.spans[s.0].coincident == s {
            return false;
        }
        self.spans[s.0].coincident = s;
        true
    }

    /// `SkOpSpan::alreadyAdded()`.
    #[must_use]
    pub(crate) fn span_already_added(&self, s: SpanId) -> bool {
        self.spans[s.0].already_added
    }

    /// `SkOpSpan::markAdded()`.
    pub(crate) fn span_mark_added(&mut self, s: SpanId) {
        self.spans[s.0].already_added = true;
    }

    /// `SkOpSpan::done()`.
    #[must_use]
    pub(crate) fn span_done(&self, s: SpanId) -> bool {
        self.spans[s.0].done
    }

    /// `SkOpSpan::setDone(bool)`.
    pub(crate) fn span_set_done(&mut self, s: SpanId, done: bool) {
        self.spans[s.0].done = done;
    }

    /// `SkOpSpan::windSum()`.
    #[must_use]
    pub(crate) fn span_wind_sum(&self, s: SpanId) -> i32 {
        self.spans[s.0].wind_sum
    }

    /// `SkOpSpan::windValue()`.
    #[must_use]
    pub(crate) fn span_wind_value(&self, s: SpanId) -> i32 {
        self.spans[s.0].wind_value
    }

    /// `SkOpSpan::setWindValue(int)`.
    pub(crate) fn span_set_wind_value(&mut self, s: SpanId, value: i32) {
        self.spans[s.0].wind_value = value;
    }

    /// `SkOpSpan::oppSum()`.
    #[must_use]
    pub(crate) fn span_opp_sum(&self, s: SpanId) -> i32 {
        self.spans[s.0].opp_sum
    }

    /// `SkOpSpan::oppValue()`.
    #[must_use]
    pub(crate) fn span_opp_value(&self, s: SpanId) -> i32 {
        self.spans[s.0].opp_value
    }

    /// `SkOpSpan::setOppValue(int)`.
    pub(crate) fn span_set_opp_value(&mut self, s: SpanId, value: i32) {
        self.spans[s.0].opp_value = value;
    }

    /// `SkOpSpan::toAngle()`.
    #[must_use]
    pub(crate) fn span_to_angle(&self, s: SpanId) -> Option<AngleId> {
        self.spans[s.0].to_angle
    }

    /// `SkOpSpan::setToAngle(angle)`.
    pub(crate) fn span_set_to_angle(&mut self, s: SpanId, angle: Option<AngleId>) {
        self.spans[s.0].to_angle = angle;
    }

    /// `SkOpSpanBase::fromAngle()` on the base and `SkOpSpan::toAngle()`: the angle at a span
    /// start that goes to `end` (`SkOpSegment::spanToAngle`).
    // Port of: src/pathops/SkOpSegment.h#L316-L318 (chrome/m156)
    #[must_use]
    pub(crate) fn span_to_angle_toward(&self, start: SpanId, end: SpanId) -> Option<AngleId> {
        if self.span_t(start) < self.span_t(end) {
            self.span_to_angle(start)
        } else {
            self.span_from_angle(start)
        }
    }

    /// `SkOpSpan::setTopTTry`/`topTTry` accessors used by the sortable-top search.
    pub(crate) fn span_top_t_try(&self, s: SpanId) -> i32 {
        self.spans[s.0].top_t_try
    }

    /// `SkOpSpan::setTopTTry`.
    pub(crate) fn span_set_top_t_try(&mut self, s: SpanId, value: i32) {
        self.spans[s.0].top_t_try = value;
    }
}
