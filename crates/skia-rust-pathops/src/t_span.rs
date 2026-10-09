// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsTSect.h (SkTCoincident, SkTSpanBounded, SkTSpan),
// src/pathops/SkPathOpsTSect.cpp (SkTCoincident::setPerp, the SkTSpan methods)

//! The spans of the T-intersection search (`SkTSpan`, `SkTCoincident`, `SkTSpanBounded`).
//!
//! Skia links spans into lists with raw pointers and allocates them in arenas. Here every span,
//! bounded-span node and T-section lives in a `Vec` owned by [`TSectArena`], and pointers are
//! typed indices ([`SpanId`], [`BoundedId`], [`SectId`]). Freed spans go on the same free list
//! Skia uses, so spans are reused in the same order.
//!
//! The SkASSERT-only `validate*` methods and the debug dumps are not ported: they check
//! invariants without changing results.

use crate::intersections::Intersections;
use crate::line::DLine;
use crate::point::DPoint;
use crate::rect::DRect;
use crate::t_curve::TCurve;
use crate::t_sect::TSect;
use crate::types::{
    approximately_zero_when_compared_to, between, precisely_zero_when_compared_to, std_max,
};

/// Index of a [`TSpan`] in a [`TSectArena`]: Skia's `SkTSpan*`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SpanId(pub(crate) usize);

/// Index of a [`TSpanBounded`] node in a [`TSectArena`]: Skia's `SkTSpanBounded*`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BoundedId(pub(crate) usize);

/// Index of a [`TSect`] in a [`TSectArena`]: Skia's `SkTSect*`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SectId(pub(crate) usize);

/// `SkTCoincident`: the perpendicular match of a span end point on the opposite curve.
// Port of: src/pathops/SkPathOpsTSect.h#L39-L82 (chrome/m156)
#[doc(alias = "SkTCoincident")]
#[derive(Copy, Clone, Debug)]
pub struct TCoincident {
    /// `SkDPoint fPerpPt`.
    perp_pt: DPoint,
    /// `double fPerpT`: perpendicular intersection on the opposite curve.
    perp_t: f64,
    /// `SkOpDebugBool fMatch`.
    is_match: bool,
}

impl Default for TCoincident {
    /// `SkTCoincident()`: calls `init()`.
    fn default() -> Self {
        let mut coin = Self {
            perp_pt: DPoint::default(),
            perp_t: 0.0,
            is_match: false,
        };
        coin.init();
        coin
    }
}

impl TCoincident {
    /// `bool isMatch() const`.
    #[doc(alias = "isMatch")]
    #[must_use]
    pub fn is_match(&self) -> bool {
        self.is_match
    }

    /// `void init()`.
    // Port of: src/pathops/SkPathOpsTSect.h#L53-L58 (chrome/m156)
    pub fn init(&mut self) {
        self.perp_t = -1.0;
        self.is_match = false;
        self.perp_pt = DPoint::new(f64::NAN, f64::NAN);
    }

    /// `void markCoincident()`.
    // Port of: src/pathops/SkPathOpsTSect.h#L60-L65 (chrome/m156)
    pub fn mark_coincident(&mut self) {
        if !self.is_match {
            self.perp_t = -1.0;
        }
        self.is_match = true;
    }

    /// `const SkDPoint& perpPt() const`.
    #[must_use]
    pub fn perp_pt(&self) -> DPoint {
        self.perp_pt
    }

    /// `double perpT() const`.
    #[doc(alias = "perpT")]
    #[must_use]
    pub fn perp_t(&self) -> f64 {
        self.perp_t
    }

    /// `void setPerp(const SkTCurve& c1, double t, const SkDPoint& cPt, const SkTCurve& c2)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L29-L62 (chrome/m156)
    #[allow(clippy::similar_names)] // dist_sq and dist2_sq are the names used by SkTCoincident::setPerp
    pub fn set_perp(&mut self, c1: &TCurve, t: f64, c_pt: DPoint, c2: &TCurve) {
        let dxdy = c1.dxdy_at_t(t);
        let perp = DLine::new([c_pt, DPoint::new(c_pt.x + dxdy.y, c_pt.y - dxdy.x)]);
        let mut i = Intersections::default();
        let used = c2.intersect_ray(&mut i, &perp);
        // only keep closest
        if used == 0 || used == 3 {
            self.init();
            return;
        }
        self.perp_t = i.t(0, 0);
        self.perp_pt = i.pt(0);
        debug_assert!(used <= 2);
        if used == 2 {
            let dist_sq = (self.perp_pt - c_pt).length_squared();
            let dist2_sq = (i.pt(1) - c_pt).length_squared();
            if dist2_sq < dist_sq {
                self.perp_t = i.t(0, 1);
                self.perp_pt = i.pt(1);
            }
        }
        self.is_match = c_pt.approximately_equal(self.perp_pt);
    }
}

/// `SkTSpanBounded`: a node of a span's list of bounded opposite spans.
// Port of: src/pathops/SkPathOpsTSect.h#L84-L87 (chrome/m156)
#[doc(alias = "SkTSpanBounded")]
#[derive(Copy, Clone, Debug)]
pub struct TSpanBounded {
    /// `SkTSpan* fBounded`.
    pub(crate) span: SpanId,
    /// `SkTSpanBounded* fNext`.
    pub(crate) next: Option<BoundedId>,
}

/// `SkTSpan`: a sub-curve of a T-section, with its bounds and its links.
// Port of: src/pathops/SkPathOpsTSect.h#L89-L233 (chrome/m156)
#[doc(alias = "SkTSpan")]
#[derive(Copy, Clone, Debug)]
#[allow(clippy::struct_excessive_bools)] // the flags are SkOpDebugBool members of SkTSpan, kept as separate fields
pub struct TSpan {
    /// `SkTCurve* fPart`.
    pub(crate) part: TCurve,
    /// `SkTCoincident fCoinStart`.
    pub(crate) coin_start: TCoincident,
    /// `SkTCoincident fCoinEnd`.
    pub(crate) coin_end: TCoincident,
    /// `SkTSpanBounded* fBounded`.
    pub(crate) bounded: Option<BoundedId>,
    /// `SkTSpan* fPrev`.
    pub(crate) prev: Option<SpanId>,
    /// `SkTSpan* fNext`.
    pub(crate) next: Option<SpanId>,
    /// `SkDRect fBounds`.
    pub(crate) bounds: DRect,
    /// `double fStartT`.
    pub(crate) start_t: f64,
    /// `double fEndT`.
    pub(crate) end_t: f64,
    /// `double fBoundsMax`.
    pub(crate) bounds_max: f64,
    /// `SkOpDebugBool fCollapsed`.
    pub(crate) collapsed: bool,
    /// `SkOpDebugBool fHasPerp`.
    pub(crate) has_perp: bool,
    /// `SkOpDebugBool fIsLinear`.
    pub(crate) is_linear: bool,
    /// `SkOpDebugBool fIsLine`.
    pub(crate) is_line: bool,
    /// `SkOpDebugBool fDeleted`.
    pub(crate) deleted: bool,
}

impl TSpan {
    /// `SkTSpan(const SkTCurve& curve, SkArenaAlloc& heap)`: the part is a copy of the curve.
    /// The other fields are set by the caller before use, as in Skia.
    // Port of: src/pathops/SkPathOpsTSect.h#L92-L94 (chrome/m156)
    #[must_use]
    pub(crate) fn new(curve: TCurve) -> Self {
        Self {
            part: curve,
            coin_start: TCoincident::default(),
            coin_end: TCoincident::default(),
            bounded: None,
            prev: None,
            next: None,
            bounds: DRect::default(),
            start_t: 0.0,
            end_t: 0.0,
            bounds_max: 0.0,
            collapsed: false,
            has_perp: false,
            is_linear: false,
            is_line: false,
            deleted: false,
        }
    }
}

/// The arenas of one T-intersection search: all spans, bounded nodes and sections.
#[derive(Debug, Default)]
pub struct TSectArena {
    pub(crate) spans: Vec<TSpan>,
    pub(crate) bounded: Vec<TSpanBounded>,
    pub(crate) sects: Vec<TSect>,
}

impl TSectArena {
    pub(crate) fn span(&self, id: SpanId) -> &TSpan {
        &self.spans[id.0]
    }

    pub(crate) fn span_mut(&mut self, id: SpanId) -> &mut TSpan {
        &mut self.spans[id.0]
    }

    pub(crate) fn bounded_node(&self, id: BoundedId) -> TSpanBounded {
        self.bounded[id.0]
    }

    pub(crate) fn sect(&self, id: SectId) -> &TSect {
        &self.sects[id.0]
    }

    pub(crate) fn sect_mut(&mut self, id: SectId) -> &mut TSect {
        &mut self.sects[id.0]
    }

    /// Appends a span that is on no list (Skia's `SkTSpan work(curve, heap)` on the stack).
    pub(crate) fn new_span(&mut self, curve: TCurve) -> SpanId {
        self.spans.push(TSpan::new(curve));
        SpanId(self.spans.len() - 1)
    }

    /// `SkTSpan::pointFirst()`.
    #[must_use]
    pub(crate) fn point_first(&self, id: SpanId) -> DPoint {
        self.span(id).part.point(0)
    }

    /// `SkTSpan::pointLast()`.
    #[must_use]
    pub(crate) fn point_last(&self, id: SpanId) -> DPoint {
        let part = self.span(id).part;
        part.point(part.point_last())
    }

    /// `SkTSpan::pointCount()`.
    #[must_use]
    pub(crate) fn point_count(&self, id: SpanId) -> usize {
        self.span(id).part.point_count()
    }

    /// `SkTSpan::startT()`.
    #[must_use]
    pub(crate) fn start_t(&self, id: SpanId) -> f64 {
        self.span(id).start_t
    }

    /// `SkTSpan::endT()`.
    #[must_use]
    pub(crate) fn end_t(&self, id: SpanId) -> f64 {
        self.span(id).end_t
    }

    /// `SkTSpan::next()`.
    #[must_use]
    pub(crate) fn next(&self, id: SpanId) -> Option<SpanId> {
        self.span(id).next
    }

    /// `SkTSpan::isBounded()`.
    #[must_use]
    pub(crate) fn is_bounded(&self, id: SpanId) -> bool {
        self.span(id).bounded.is_some()
    }

    /// `void SkTSpan::addBounded(SkTSpan* span, SkArenaAlloc* heap)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L64-L69 (chrome/m156)
    pub(crate) fn add_bounded(&mut self, this: SpanId, span: SpanId) {
        self.bounded.push(TSpanBounded {
            span,
            next: self.span(this).bounded,
        });
        let node = BoundedId(self.bounded.len() - 1);
        self.span_mut(this).bounded = Some(node);
    }

    /// `double SkTSpan::closestBoundedT(const SkDPoint& pt) const`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L119-L141 (chrome/m156)
    #[must_use]
    pub(crate) fn closest_bounded_t(&self, this: SpanId, pt: DPoint) -> f64 {
        let mut result = -1.0;
        let mut closest = f64::MAX;
        let mut node = self.span(this).bounded;
        while let Some(n) = node {
            let bounded = self.bounded_node(n);
            let test = bounded.span;
            let start_dist = self.point_first(test).distance_squared(pt);
            if closest > start_dist {
                closest = start_dist;
                result = self.start_t(test);
            }
            let end_dist = self.point_last(test).distance_squared(pt);
            if closest > end_dist {
                closest = end_dist;
                result = self.end_t(test);
            }
            node = bounded.next;
        }
        debug_assert!(between(0.0, result, 1.0));
        result
    }

    /// `bool SkTSpan::contains(double t) const`: whether `t` is in this span or a later one.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L154-L162 (chrome/m156)
    #[must_use]
    pub(crate) fn span_contains(&self, this: SpanId, t: f64) -> bool {
        let mut work = Some(this);
        while let Some(w) = work {
            let span = self.span(w);
            if between(span.start_t, t, span.end_t) {
                return true;
            }
            work = span.next;
        }
        false
    }

    /// `SkTSpan* SkTSpan::findOppSpan(const SkTSpan* opp) const`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L168-L184 (chrome/m156)
    #[must_use]
    pub(crate) fn find_opp_span(&self, this: SpanId, opp: SpanId) -> Option<SpanId> {
        let mut node = self.span(this).bounded;
        while let Some(n) = node {
            let bounded = self.bounded_node(n);
            if opp == bounded.span {
                return Some(bounded.span);
            }
            node = bounded.next;
        }
        None
    }

    /// `int SkTSpan::hullCheck(const SkTSpan* opp, bool* start, bool* oppStart)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L186-L211 (chrome/m156)
    fn hull_check(
        &mut self,
        this: SpanId,
        opp: SpanId,
        start: &mut bool,
        opp_start: &mut bool,
    ) -> i32 {
        if self.span(this).is_linear {
            return -1;
        }
        let (only_end_points, pts_in_common) =
            self.only_end_points_in_common(this, opp, start, opp_start);
        if only_end_points {
            debug_assert!(pts_in_common);
            return 2;
        }
        let part = self.span(this).part;
        let opp_part = self.span(opp).part;
        if let Some(linear) = part.hull_intersects(&opp_part) {
            if !linear {
                // check set true if linear
                return 1;
            }
            let span = self.span_mut(this);
            span.is_linear = true;
            span.is_line = part.controls_inside();
            return if pts_in_common { 1 } else { -1 };
        }
        // hull is not linear; check set true if intersected at the end points
        i32::from(pts_in_common) << 1 // 0 or 2
    }

    /// `int SkTSpan::hullsIntersect(SkTSpan* opp, bool* start, bool* oppStart)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L213-L227 (chrome/m156)
    pub(crate) fn hulls_intersect(
        &mut self,
        this: SpanId,
        opp: SpanId,
        start: &mut bool,
        opp_start: &mut bool,
    ) -> i32 {
        if !self.span(this).bounds.intersects(&self.span(opp).bounds) {
            return 0;
        }
        let hull_sect = self.hull_check(this, opp, start, opp_start);
        if hull_sect >= 0 {
            return hull_sect;
        }
        let hull_sect = self.hull_check(opp, this, opp_start, start);
        if hull_sect >= 0 {
            return hull_sect;
        }
        -1
    }

    /// `void SkTSpan::init(const SkTCurve& c)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L229-L235 (chrome/m156)
    pub(crate) fn init_span(&mut self, this: SpanId, curve: TCurve) {
        let span = self.span_mut(this);
        span.prev = None;
        span.next = None;
        span.start_t = 0.0;
        span.end_t = 1.0;
        span.bounded = None;
        self.reset_bounds(this, curve);
    }

    /// `bool SkTSpan::initBounds(const SkTCurve& c)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L237-L255 (chrome/m156)
    pub(crate) fn init_bounds(&mut self, this: SpanId, curve: TCurve) -> bool {
        let (start_t, end_t) = (self.start_t(this), self.end_t(this));
        if start_t.is_nan() || end_t.is_nan() {
            return false;
        }
        let part = curve.sub_divide(start_t, end_t);
        let span = self.span_mut(this);
        span.part = part;
        span.bounds.set_bounds_tcurve(&part);
        span.coin_start.init();
        span.coin_end.init();
        span.bounds_max = std_max(span.bounds.width(), span.bounds.height());
        span.collapsed = part.collapsed();
        span.has_perp = false;
        span.deleted = false;
        span.bounds.valid()
    }

    /// `void SkTSpan::resetBounds(const SkTCurve& curve)`.
    // Port of: src/pathops/SkPathOpsTSect.h#L180-L184 (chrome/m156)
    pub(crate) fn reset_bounds(&mut self, this: SpanId, curve: TCurve) {
        let span = self.span_mut(this);
        span.is_linear = false;
        span.is_line = false;
        self.init_bounds(this, curve);
    }

    /// `bool SkTSpan::linearsIntersect(SkTSpan* span)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L257-L266 (chrome/m156)
    pub(crate) fn linears_intersect(&self, this: SpanId, span: SpanId) -> bool {
        let span_part = self.span(span).part;
        let result = self.linear_intersects(this, &span_part);
        if result <= 1 {
            return result != 0;
        }
        debug_assert!(self.span(span).is_linear);
        let this_part = self.span(this).part;
        self.linear_intersects(span, &this_part) != 0
    }

    /// `int SkTSpan::linearIntersects(const SkTCurve& q2) const`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L275-L319 (chrome/m156)
    #[must_use]
    pub(crate) fn linear_intersects(&self, this: SpanId, q2: &TCurve) -> i32 {
        let part = self.span(this).part;
        // looks like q1 is near-linear
        let mut start = 0;
        // the outside points are usually the extremes
        let mut end = part.point_last();
        if !part.controls_inside() {
            // if there's any question, compute distance to find best outsiders
            let mut dist = 0.0;
            for outer in 0..self.point_count(this) - 1 {
                for inner in outer + 1..self.point_count(this) {
                    let test = (part.point(outer) - part.point(inner)).length_squared();
                    if dist > test {
                        continue;
                    }
                    dist = test;
                    start = outer;
                    end = inner;
                }
            }
        }
        // see if q2 is on one side of the line formed by the extreme points
        let orig_x = part.point(start).x;
        let orig_y = part.point(start).y;
        let adj = part.point(end).x - orig_x;
        let opp = part.point(end).y - orig_y;
        let max_part = std_max(adj.abs(), opp.abs());
        let mut sign = 0.0; // initialization to shut up warning in release build
        for n in 0..q2.point_count() {
            let dx = q2.point(n).y - orig_y;
            let dy = q2.point(n).x - orig_x;
            let max_val = std_max(max_part, std_max(dx.abs(), dy.abs()));
            let test = (q2.point(n).y - orig_y) * adj - (q2.point(n).x - orig_x) * opp;
            if precisely_zero_when_compared_to(test, max_val) {
                return 1;
            }
            if approximately_zero_when_compared_to(test, max_val) {
                return 3;
            }
            if n == 0 {
                sign = test;
                continue;
            }
            if test * sign < 0.0 {
                return 1;
            }
        }
        0
    }

    /// `bool SkTSpan::onlyEndPointsInCommon(const SkTSpan* opp, bool* start, bool* oppStart,
    /// bool* ptsInCommon)`. Returns `(result, ptsInCommon)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L321-L354 (chrome/m156)
    pub(crate) fn only_end_points_in_common(
        &self,
        this: SpanId,
        opp: SpanId,
        start: &mut bool,
        opp_start: &mut bool,
    ) -> (bool, bool) {
        if self.point_first(opp) == self.point_first(this) {
            *start = true;
            *opp_start = true;
        } else if self.point_first(opp) == self.point_last(this) {
            *start = false;
            *opp_start = true;
        } else if self.point_last(opp) == self.point_first(this) {
            *start = true;
            *opp_start = false;
        } else if self.point_last(opp) == self.point_last(this) {
            *start = false;
            *opp_start = false;
        } else {
            return (false, false);
        }
        let this_part = self.span(this).part;
        let opp_part = self.span(opp).part;
        let base_index = if *start { 0 } else { this_part.point_last() };
        let other_pts = this_part.other_pts(base_index);
        let opp_other_pts = opp_part.other_pts(if *opp_start { 0 } else { opp_part.point_last() });
        let base = this_part.point(base_index);
        for &other in other_pts.iter().take(self.point_count(this) - 1) {
            let v1 = other - base;
            for &opp_other in opp_other_pts.iter().take(self.point_count(opp) - 1) {
                let v2 = opp_other - base;
                if v2.dot(v1) >= 0.0 {
                    return (false, true);
                }
            }
        }
        (true, true)
    }

    /// `SkTSpan* SkTSpan::oppT(double t) const`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L356-L366 (chrome/m156)
    #[must_use]
    pub(crate) fn opp_t(&self, this: SpanId, t: f64) -> Option<SpanId> {
        let mut node = self.span(this).bounded;
        while let Some(n) = node {
            let bounded = self.bounded_node(n);
            let test = bounded.span;
            if between(self.start_t(test), t, self.end_t(test)) {
                return Some(test);
            }
            node = bounded.next;
        }
        None
    }

    /// `bool SkTSpan::removeAllBounded()`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L368-L377 (chrome/m156)
    pub(crate) fn remove_all_bounded(&mut self, this: SpanId) -> bool {
        let mut delete_span = false;
        let mut node = self.span(this).bounded;
        while let Some(n) = node {
            let bounded = self.bounded_node(n);
            let opp = bounded.span;
            delete_span |= self.remove_bounded(opp, this);
            node = bounded.next;
        }
        delete_span
    }

    /// `bool SkTSpan::removeBounded(const SkTSpan* opp)`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L379-L416 (chrome/m156)
    pub(crate) fn remove_bounded(&mut self, this: SpanId, opp: SpanId) -> bool {
        if self.span(this).has_perp {
            let mut found_start = false;
            let mut found_end = false;
            let (perp_start, perp_end) = {
                let span = self.span(this);
                (span.coin_start.perp_t(), span.coin_end.perp_t())
            };
            let mut node = self.span(this).bounded;
            while let Some(n) = node {
                let bounded = self.bounded_node(n);
                let test = bounded.span;
                if opp != test {
                    found_start |= between(self.start_t(test), perp_start, self.end_t(test));
                    found_end |= between(self.start_t(test), perp_end, self.end_t(test));
                }
                node = bounded.next;
            }
            if !found_start || !found_end {
                let span = self.span_mut(this);
                span.has_perp = false;
                span.coin_start.init();
                span.coin_end.init();
            }
        }
        let mut node = self.span(this).bounded;
        let mut prev: Option<BoundedId> = None;
        while let Some(n) = node {
            let bounded = self.bounded_node(n);
            let bounded_next = bounded.next;
            if opp == bounded.span {
                if let Some(p) = prev {
                    self.bounded[p.0].next = bounded_next;
                    return false;
                }
                self.span_mut(this).bounded = bounded_next;
                return bounded_next.is_none();
            }
            prev = Some(n);
            node = bounded_next;
        }
        debug_assert!(
            false,
            "SkOPASSERT(0): removeBounded of a span that is not bounded"
        );
        false
    }

    /// `bool SkTSpan::splitAt(SkTSpan* work, double t, SkArenaAlloc* heap)`: `this` becomes the
    /// second part of `work` from `t`.
    // Port of: src/pathops/SkPathOpsTSect.cpp#L418-L452 (chrome/m156)
    #[allow(clippy::float_cmp)] // exact comparisons of span end points, as SkTSpan::splitAt
    pub(crate) fn split_at(&mut self, this: SpanId, work: SpanId, t: f64) -> bool {
        self.span_mut(this).start_t = t;
        self.span_mut(this).end_t = self.end_t(work);
        if self.start_t(this) == self.end_t(this) {
            self.span_mut(this).collapsed = true;
            return false;
        }
        self.span_mut(work).end_t = t;
        if self.start_t(work) == self.end_t(work) {
            self.span_mut(work).collapsed = true;
            return false;
        }
        self.span_mut(this).prev = Some(work);
        let work_next = self.span(work).next;
        self.span_mut(this).next = work_next;
        let (work_is_linear, work_is_line) = (self.span(work).is_linear, self.span(work).is_line);
        self.span_mut(this).is_linear = work_is_linear;
        self.span_mut(this).is_line = work_is_line;

        self.span_mut(work).next = Some(this);
        if let Some(n) = work_next {
            self.span_mut(n).prev = Some(this);
        }
        let mut bounded = self.span(work).bounded;
        self.span_mut(this).bounded = None;
        while let Some(n) = bounded {
            let node = self.bounded_node(n);
            self.add_bounded(this, node.span);
            bounded = node.next;
        }
        let mut bounded = self.span(this).bounded;
        while let Some(n) = bounded {
            let node = self.bounded_node(n);
            self.add_bounded(node.span, this);
            bounded = node.next;
        }
        true
    }

    /// `bool SkTSpan::split(SkTSpan* work, SkArenaAlloc* heap)`.
    // Port of: src/pathops/SkPathOpsTSect.h#L146-L148 (chrome/m156)
    #[allow(clippy::manual_midpoint)] // `(start + end) * 0.5` is the C++ expression of SkTSpan::split
    pub(crate) fn split(&mut self, this: SpanId, work: SpanId) -> bool {
        let mid = (self.start_t(work) + self.end_t(work)) * 0.5;
        self.split_at(this, work, mid)
    }

    /// `void SkTSpan::markCoincident()`.
    // Port of: src/pathops/SkPathOpsTSect.h#L115-L118 (chrome/m156)
    pub(crate) fn mark_coincident(&mut self, this: SpanId) {
        let span = self.span_mut(this);
        span.coin_start.mark_coincident();
        span.coin_end.mark_coincident();
    }
}
