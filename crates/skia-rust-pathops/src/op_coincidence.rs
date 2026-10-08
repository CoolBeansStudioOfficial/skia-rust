// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkOpCoincidence.h, src/pathops/SkOpCoincidence.cpp (chrome/m156)

//! Coincident spans of the op graph (`SkCoincidentSpans` and `SkOpCoincidence`).
//!
//! Skia's `SkOpCoincidence` is a pair of intrusive lists of `SkCoincidentSpans` records, `fHead`
//! and `fTop`. The global state points at the coincidence object currently in use. Here each
//! `SkOpCoincidence` is a [`CoinSet`] in [`OpState::coin_sets`], and the global pointer is
//! [`OpState::coin_global`]. Methods that Skia calls on a specific object take its
//! [`CoinSetId`] explicitly (`cs_*`). Methods that Skia calls through
//! `globalState()->coincidence()` are the `coin_*` wrappers.
//!
//! A `SkCoincidentSpans**` head pointer becomes a [`CoinList`] plus, while walking, the id of
//! the previous record (`None` for the list head). Unlinking a record sets the previous link
//! (or the list head) to the record's `next`, which is what `*headPtr = coin->next()` does.
//!
//! C++ `continue` inside a `do { } while` loop jumps to the loop condition, so the advance
//! happens. Those loops use a labeled block (`'body:`) followed by the advance.
// Pedantic lints allowed for this module because it mirrors Skia line by line: SkScalar and
// double comparisons are exact in Skia (no epsilon), the C++ integer casts are kept as they are
// (the ids and counts are small), names follow Skia (pt1, pt2, oppTest), long Skia functions
// keep their structure (goto-shaped control flow that would be harder to check if split), and
// a few loops are Skia's do/while forms that run once.
#![allow(
    clippy::similar_names,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::struct_excessive_bools,
    clippy::items_after_statements,
    clippy::struct_field_names,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::option_option,
    clippy::question_mark,
    clippy::while_let_loop,
    clippy::while_let_on_iterator,
    clippy::unused_self,
    clippy::never_loop,
    clippy::needless_range_loop
)]

use skia_rust_core::path::Verb;
use skia_rust_core::point::Point;

use crate::intersections::Intersections;
use crate::line::DLine;
use crate::op_curve::{curve_intersect_ray, verb_points};
use crate::op_span::Collapsed;
use crate::op_state::{CoinId, CoinSetId, OpState, PtTId, SegId, SpanId};
use crate::point::DPoint;
use crate::types::{between, std_max, std_min, zero_or_one};

/// `SkCoincidentSpans`: one coincident run, as points on two segments.
// Port of: src/pathops/SkOpCoincidence.h#L22-L136 (chrome/m156)
#[doc(alias = "SkCoincidentSpans")]
#[derive(Copy, Clone, Debug)]
pub(crate) struct CoincidentSpans {
    /// `SkCoincidentSpans* fNext`.
    pub(crate) next: Option<CoinId>,
    /// `const SkOpPtT* fCoinPtTStart`.
    pub(crate) coin_start: PtTId,
    /// `const SkOpPtT* fCoinPtTEnd`.
    pub(crate) coin_end: PtTId,
    /// `const SkOpPtT* fOppPtTStart`.
    pub(crate) opp_start: PtTId,
    /// `const SkOpPtT* fOppPtTEnd`.
    pub(crate) opp_end: PtTId,
}

impl Default for CoincidentSpans {
    fn default() -> Self {
        Self {
            next: None,
            coin_start: PtTId(usize::MAX),
            coin_end: PtTId(usize::MAX),
            opp_start: PtTId(usize::MAX),
            opp_end: PtTId(usize::MAX),
        }
    }
}

/// One `SkOpCoincidence` object: the two lists `fHead` and `fTop`.
// Port of: src/pathops/SkOpCoincidence.h#L138-L306 (chrome/m156)
#[doc(alias = "SkOpCoincidence")]
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct CoinSet {
    /// `SkCoincidentSpans* fHead`.
    pub(crate) head: Option<CoinId>,
    /// `SkCoincidentSpans* fTop`.
    pub(crate) top: Option<CoinId>,
}

/// Which `SkCoincidentSpans*` list head of a [`CoinSet`] is meant (`&fHead` or `&fTop`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum CoinList {
    Head,
    Top,
}

/// One of the four `SkOpPtT*` fields of a [`CoincidentSpans`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum CoinPt {
    CoinStart,
    CoinEnd,
    OppStart,
    OppEnd,
}

/// `fFailed` of `SkOpCoincidence`'s methods is `return false`. Skia's `!x` on a double is true
/// for zero and NaN.
// Port of: the `FAIL_IF(!x)` tests on double ranges in src/pathops/SkOpCoincidence.cpp (chrome/m156)
fn is_zero_or_nan(x: f64) -> bool {
    x == 0.0 || x.is_nan()
}

/// Verb ordering used by `SkOpCoincidence::Ordered`: `SkPath::Verb` enum order.
// Port of: src/pathops/SkOpCoincidence.cpp#L1390-L1395 (verb comparisons, chrome/m156)
fn verb_rank(verb: Verb) -> u8 {
    verb as u8
}

impl OpState {
    /// `SkOpPtT::fT`.
    pub(crate) fn ptt_t(&self, ptt: PtTId) -> f64 {
        self.ptts[ptt.0].t
    }

    /// `SkOpPtT::fPt`.
    pub(crate) fn ptt_pt(&self, ptt: PtTId) -> Point {
        self.ptts[ptt.0].pt
    }

    // ----- SkCoincidentSpans accessors -----

    /// `SkCoincidentSpans::fNext`.
    fn rec_next(&self, c: CoinId) -> Option<CoinId> {
        self.coin_spans[c.0].next
    }

    /// `SkCoincidentSpans::coinPtTStart()`.
    fn rec_coin_start(&self, c: CoinId) -> PtTId {
        self.coin_spans[c.0].coin_start
    }

    /// `SkCoincidentSpans::coinPtTEnd()`.
    fn rec_coin_end(&self, c: CoinId) -> PtTId {
        self.coin_spans[c.0].coin_end
    }

    /// `SkCoincidentSpans::oppPtTStart()`.
    fn rec_opp_start(&self, c: CoinId) -> PtTId {
        self.coin_spans[c.0].opp_start
    }

    /// `SkCoincidentSpans::oppPtTEnd()`.
    fn rec_opp_end(&self, c: CoinId) -> PtTId {
        self.coin_spans[c.0].opp_end
    }

    /// `SkCoincidentSpans::setCoinPtTStart(ptT)`.
    // Port of: src/pathops/SkOpCoincidence.h#L93-L99 (chrome/m156)
    fn rec_set_coin_start(&mut self, c: CoinId, ptt: PtTId) {
        self.coin_spans[c.0].coin_start = ptt;
        self.ptt_set_coincident(ptt);
    }

    /// `SkCoincidentSpans::setCoinPtTEnd(ptT)`.
    // Port of: src/pathops/SkOpCoincidence.h#L85-L91 (chrome/m156)
    fn rec_set_coin_end(&mut self, c: CoinId, ptt: PtTId) {
        self.coin_spans[c.0].coin_end = ptt;
        self.ptt_set_coincident(ptt);
    }

    /// `SkCoincidentSpans::setOppPtTStart(ptT)`.
    // Port of: src/pathops/SkOpCoincidence.h#L114-L120 (chrome/m156)
    fn rec_set_opp_start(&mut self, c: CoinId, ptt: PtTId) {
        self.coin_spans[c.0].opp_start = ptt;
        self.ptt_set_coincident(ptt);
    }

    /// `SkCoincidentSpans::setOppPtTEnd(ptT)`.
    // Port of: src/pathops/SkOpCoincidence.h#L106-L112 (chrome/m156)
    fn rec_set_opp_end(&mut self, c: CoinId, ptt: PtTId) {
        self.coin_spans[c.0].opp_end = ptt;
        self.ptt_set_coincident(ptt);
    }

    /// `SkCoincidentSpans::setStarts(coinPtTStart, oppPtTStart)`.
    // Port of: src/pathops/SkOpCoincidence.h#L122-L125 (chrome/m156)
    fn rec_set_starts(&mut self, c: CoinId, coin_start: PtTId, opp_start: PtTId) {
        self.rec_set_coin_start(c, coin_start);
        self.rec_set_opp_start(c, opp_start);
    }

    /// `SkCoincidentSpans::setEnds(coinPtTEnd, oppPtTEnd)`.
    // Port of: src/pathops/SkOpCoincidence.h#L101-L104 (chrome/m156)
    fn rec_set_ends(&mut self, c: CoinId, coin_end: PtTId, opp_end: PtTId) {
        self.rec_set_coin_end(c, coin_end);
        self.rec_set_opp_end(c, opp_end);
    }

    /// `SkCoincidentSpans::flipped()`: the opposite range runs backwards.
    // Port of: src/pathops/SkOpCoincidence.h#L63 (chrome/m156)
    fn rec_flipped(&self, c: CoinId) -> bool {
        self.ptt_t(self.rec_opp_start(c)) > self.ptt_t(self.rec_opp_end(c))
    }

    /// `SkCoincidentSpans::set(next, coinPtTStart, coinPtTEnd, oppPtTStart, oppPtTEnd)`.
    // Port of: src/pathops/SkOpCoincidence.cpp#L110-L117 (chrome/m156)
    fn rec_set(
        &mut self,
        c: CoinId,
        next: Option<CoinId>,
        coin_start: PtTId,
        coin_end: PtTId,
        opp_start: PtTId,
        opp_end: PtTId,
    ) {
        self.coin_spans[c.0].next = next;
        self.rec_set_starts(c, coin_start, opp_start);
        self.rec_set_ends(c, coin_end, opp_end);
    }

    /// `SkCoincidentSpans::collapsed(test)`: returns true if the span's start and end are the
    /// same.
    // Port of: src/pathops/SkOpCoincidence.cpp#L13-L19 (chrome/m156)
    fn rec_collapsed(&self, c: CoinId, test: PtTId) -> bool {
        let coin_start = self.rec_coin_start(c);
        let coin_end = self.rec_coin_end(c);
        let opp_start = self.rec_opp_start(c);
        let opp_end = self.rec_opp_end(c);
        (coin_start == test && self.ptt_contains_ptt(coin_end, test))
            || (coin_end == test && self.ptt_contains_ptt(coin_start, test))
            || (opp_start == test && self.ptt_contains_ptt(opp_end, test))
            || (opp_end == test && self.ptt_contains_ptt(opp_start, test))
    }

    /// `SkCoincidentSpans::contains(s, e)`: both points are inside this record.
    // Port of: src/pathops/SkOpCoincidence.cpp#L71-L88 (chrome/m156)
    fn rec_contains(&self, c: CoinId, s: PtTId, e: PtTId) -> bool {
        let (s, e) = if self.ptt_t(s) > self.ptt_t(e) {
            (e, s)
        } else {
            (s, e)
        };
        let coin_start = self.rec_coin_start(c);
        if self.ptt_segment(s) == self.ptt_segment(coin_start) {
            self.ptt_t(coin_start) <= self.ptt_t(s)
                && self.ptt_t(e) <= self.ptt_t(self.rec_coin_end(c))
        } else {
            let mut opp_ts = self.ptt_t(self.rec_opp_start(c));
            let mut opp_te = self.ptt_t(self.rec_opp_end(c));
            if opp_ts > opp_te {
                std::mem::swap(&mut opp_ts, &mut opp_te);
            }
            opp_ts <= self.ptt_t(s) && self.ptt_t(e) <= opp_te
        }
    }

    /// `SkCoincidentSpans::correctOneEnd(getEnd, setEnd)`: sets the end to the point the
    /// neighbouring span defines.
    // Port of: src/pathops/SkOpCoincidence.cpp#L27-L39 (chrome/m156)
    fn rec_correct_one_end(&mut self, c: CoinId, which: CoinPt) {
        let orig = self.rec_get(c, which);
        let orig_span = self.ptt_span(orig);
        let test = if let Some(prev) = self.span_prev(orig_span) {
            let Some(next) = self.span_next(prev) else {
                return;
            };
            self.span_ptt(next)
        } else {
            let up = self.span_up_cast(orig_span);
            let Some(next) = self.span_next(up) else {
                return;
            };
            let Some(prev) = self.span_prev(next) else {
                return;
            };
            self.span_ptt(prev)
        };
        if orig != test {
            self.rec_set_which(c, which, test);
        }
    }

    /// The `getEnd` member pointer of `SkCoincidentSpans` for `which`.
    fn rec_get(&self, c: CoinId, which: CoinPt) -> PtTId {
        match which {
            CoinPt::CoinStart => self.rec_coin_start(c),
            CoinPt::CoinEnd => self.rec_coin_end(c),
            CoinPt::OppStart => self.rec_opp_start(c),
            CoinPt::OppEnd => self.rec_opp_end(c),
        }
    }

    /// The `setEnd` member pointer of `SkCoincidentSpans` for `which`.
    fn rec_set_which(&mut self, c: CoinId, which: CoinPt, ptt: PtTId) {
        match which {
            CoinPt::CoinStart => self.rec_set_coin_start(c, ptt),
            CoinPt::CoinEnd => self.rec_set_coin_end(c, ptt),
            CoinPt::OppStart => self.rec_set_opp_start(c, ptt),
            CoinPt::OppEnd => self.rec_set_opp_end(c, ptt),
        }
    }

    /// `SkCoincidentSpans::correctEnds()`: makes every end agree with the segment's spans.
    // Port of: src/pathops/SkOpCoincidence.cpp#L41-L50 (chrome/m156)
    fn rec_correct_ends(&mut self, c: CoinId) {
        self.rec_correct_one_end(c, CoinPt::CoinStart);
        self.rec_correct_one_end(c, CoinPt::CoinEnd);
        self.rec_correct_one_end(c, CoinPt::OppStart);
        self.rec_correct_one_end(c, CoinPt::OppEnd);
    }

    /// `SkCoincidentSpans::expand()`: extends the range by checking adjacent spans for
    /// coincidence.
    // Port of: src/pathops/SkOpCoincidence.cpp#L52-L95 (chrome/m156)
    fn rec_expand(&mut self, c: CoinId) -> bool {
        let mut expanded = false;
        let segment = self.ptt_segment(self.rec_coin_start(c));
        let opp_segment = self.ptt_segment(self.rec_opp_start(c));
        loop {
            let start = self.ptt_span(self.rec_coin_start(c));
            let Some(prev) = self.span_prev(start) else {
                break;
            };
            let Some(opp_ptt) = self.span_contains_seg(prev, opp_segment) else {
                break;
            };
            let mid_t = f64::midpoint(self.span_t(prev), self.span_t(start));
            if !self.seg_is_close(segment, mid_t, opp_segment) {
                break;
            }
            let prev_ptt = self.span_ptt(prev);
            self.rec_set_starts(c, prev_ptt, opp_ptt);
            expanded = true;
        }
        loop {
            let end = self.ptt_span(self.rec_coin_end(c));
            let next = if self.span_final(end) {
                None
            } else {
                self.span_next(self.span_up_cast(end))
            };
            if let Some(n) = next
                && self.span_deleted(n)
            {
                break;
            }
            let Some(next) = next else {
                break;
            };
            let Some(opp_ptt) = self.span_contains_seg(next, opp_segment) else {
                break;
            };
            let mid_t = f64::midpoint(self.span_t(end), self.span_t(next));
            if !self.seg_is_close(segment, mid_t, opp_segment) {
                break;
            }
            let next_ptt = self.span_ptt(next);
            self.rec_set_ends(c, next_ptt, opp_ptt);
            expanded = true;
        }
        expanded
    }

    /// `SkCoincidentSpans::extend(...)`: increases the range of this record.
    // Port of: src/pathops/SkOpCoincidence.cpp#L97-L108 (chrome/m156)
    fn rec_extend(
        &mut self,
        c: CoinId,
        coin_start: PtTId,
        coin_end: PtTId,
        opp_start: PtTId,
        opp_end: PtTId,
    ) -> bool {
        let mut result = false;
        if self.ptt_t(self.rec_coin_start(c)) > self.ptt_t(coin_start)
            || (if self.rec_flipped(c) {
                self.ptt_t(self.rec_opp_start(c)) < self.ptt_t(opp_start)
            } else {
                self.ptt_t(self.rec_opp_start(c)) > self.ptt_t(opp_start)
            })
        {
            self.rec_set_starts(c, coin_start, opp_start);
            result = true;
        }
        if self.ptt_t(self.rec_coin_end(c)) < self.ptt_t(coin_end)
            || (if self.rec_flipped(c) {
                self.ptt_t(self.rec_opp_end(c)) > self.ptt_t(opp_end)
            } else {
                self.ptt_t(self.rec_opp_end(c)) < self.ptt_t(opp_end)
            })
        {
            self.rec_set_ends(c, coin_end, opp_end);
            result = true;
        }
        result
    }

    /// `SkCoincidentSpans::ordered(bool* result)`: returns `None` when the walk fails
    /// (Skia returns false without setting `result`).
    // Port of: src/pathops/SkOpCoincidence.cpp#L121-L157 (chrome/m156)
    fn rec_ordered(&self, c: CoinId) -> Option<bool> {
        let start = self.ptt_span(self.rec_coin_start(c));
        let end = self.ptt_span(self.rec_coin_end(c));
        let mut next = self.span_next(self.span_up_cast(start))?;
        if next == end {
            return Some(true);
        }
        let flipped = self.rec_flipped(c);
        let opp_seg = self.ptt_segment(self.rec_opp_start(c));
        let mut opp_last_t = self.ptt_t(self.rec_opp_start(c));
        loop {
            let opp = self.span_contains_seg(next, opp_seg)?;
            if (opp_last_t > self.ptt_t(opp)) != flipped {
                return Some(false);
            }
            opp_last_t = self.ptt_t(opp);
            if next == end {
                break;
            }
            if self.span_up_castable(next).is_none() {
                return Some(false);
            }
            next = self.span_next(self.span_up_cast(next))?;
        }
        Some(true)
    }

    // ----- SkOpCoincidence list plumbing -----

    /// The head of one of a set's lists.
    pub(crate) fn cs_list_head(&self, set: CoinSetId, list: CoinList) -> Option<CoinId> {
        match list {
            CoinList::Head => self.coin_sets[set.0].head,
            CoinList::Top => self.coin_sets[set.0].top,
        }
    }

    /// Sets the head of one of a set's lists.
    pub(crate) fn cs_set_list_head(
        &mut self,
        set: CoinSetId,
        list: CoinList,
        head: Option<CoinId>,
    ) {
        match list {
            CoinList::Head => self.coin_sets[set.0].head = head,
            CoinList::Top => self.coin_sets[set.0].top = head,
        }
    }

    /// `*headPtr = coin->next()`, where `prev` is the record whose `next` is `headPtr`
    /// (`None` when `headPtr` is the list head).
    pub(crate) fn cs_unlink(
        &mut self,
        set: CoinSetId,
        list: CoinList,
        prev: Option<CoinId>,
        cur: CoinId,
    ) {
        let next = self.coin_spans[cur.0].next;
        match prev {
            None => self.cs_set_list_head(set, list, next),
            Some(p) => self.coin_spans[p.0].next = next,
        }
    }

    /// `SkOpCoincidence::isEmpty()`.
    // Port of: src/pathops/SkOpCoincidence.h#L230-L232 (chrome/m156)
    pub(crate) fn cs_is_empty(&self, set: CoinSetId) -> bool {
        self.coin_sets[set.0].head.is_none() && self.coin_sets[set.0].top.is_none()
    }

    /// `SkOpCoincidence::Ordered(coinPtTStart, oppPtTStart)`.
    // Port of: src/pathops/SkOpCoincidence.h#L237-L239 (chrome/m156)
    pub(crate) fn cs_ordered_ptts(&self, coin_start: PtTId, opp_start: PtTId) -> bool {
        self.cs_ordered_segs(self.ptt_segment(coin_start), self.ptt_segment(opp_start))
    }

    /// `SkOpCoincidence::Ordered(coinSeg, oppSeg)`: a total order on segments, by verb and then
    /// by the point coordinates.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1390-L1409 (chrome/m156)
    pub(crate) fn cs_ordered_segs(&self, coin: SegId, opp: SegId) -> bool {
        let coin_verb = self.seg_verb(coin);
        let opp_verb = self.seg_verb(opp);
        if verb_rank(coin_verb) < verb_rank(opp_verb) {
            return true;
        }
        if verb_rank(coin_verb) > verb_rank(opp_verb) {
            return false;
        }
        let count = (verb_points(coin_verb) + 1) * 2;
        let c_pts = self.seg_pts(coin);
        let o_pts = self.seg_pts(opp);
        for index in 0..count {
            let c = float_at(&c_pts, index);
            let o = float_at(&o_pts, index);
            if c < o {
                return true;
            }
            if c > o {
                return false;
            }
        }
        true
    }

    /// `SkOpCoincidence::contains(seg, opp, oppT)`: checks the head and top lists for a record
    /// that covers `opp_t` on `opp`.
    // Port of: src/pathops/SkOpCoincidence.cpp#L859-L866 (chrome/m156)
    pub(crate) fn cs_contains_seg_t(
        &self,
        set: CoinSetId,
        seg: SegId,
        opp: SegId,
        opp_t: f64,
    ) -> bool {
        self.coin_list_contains_seg_t(self.cs_list_head(set, CoinList::Head), seg, opp, opp_t)
            || self.coin_list_contains_seg_t(self.cs_list_head(set, CoinList::Top), seg, opp, opp_t)
    }

    /// `SkOpCoincidence::contains(const SkCoincidentSpans* coin, seg, opp, oppT)`.
    // Port of: src/pathops/SkOpCoincidence.cpp#L868-L886 (chrome/m156)
    fn coin_list_contains_seg_t(
        &self,
        head: Option<CoinId>,
        seg: SegId,
        opp: SegId,
        opp_t: f64,
    ) -> bool {
        let mut coin = head;
        while let Some(c) = coin {
            let coin_start = self.rec_coin_start(c);
            let coin_end = self.rec_coin_end(c);
            let opp_start = self.rec_opp_start(c);
            let opp_end = self.rec_opp_end(c);
            if self.ptt_segment(coin_start) == seg
                && self.ptt_segment(opp_start) == opp
                && between(self.ptt_t(opp_start), opp_t, self.ptt_t(opp_end))
            {
                return true;
            }
            if self.ptt_segment(opp_start) == seg
                && self.ptt_segment(coin_start) == opp
                && between(self.ptt_t(coin_start), opp_t, self.ptt_t(coin_end))
            {
                return true;
            }
            coin = self.rec_next(c);
        }
        false
    }

    /// `SkOpCoincidence::contains(coinPtTStart, coinPtTEnd, oppPtTStart, oppPtTEnd)`.
    // Port of: src/pathops/SkOpCoincidence.cpp#L888-L925 (chrome/m156)
    pub(crate) fn cs_contains_ptts(
        &self,
        set: CoinSetId,
        coin_start: PtTId,
        coin_end: PtTId,
        opp_start: PtTId,
        opp_end: PtTId,
    ) -> bool {
        let Some(mut test) = self.cs_list_head(set, CoinList::Head) else {
            return false;
        };
        let (mut coin_start, mut coin_end, mut opp_start, mut opp_end) =
            (coin_start, coin_end, opp_start, opp_end);
        let mut coin_seg = self.ptt_segment(coin_start);
        let mut opp_seg = self.ptt_segment(opp_start);
        if !self.cs_ordered_ptts(coin_start, opp_start) {
            std::mem::swap(&mut coin_seg, &mut opp_seg);
            std::mem::swap(&mut coin_start, &mut opp_start);
            std::mem::swap(&mut coin_end, &mut opp_end);
            if self.ptt_t(coin_start) > self.ptt_t(coin_end) {
                std::mem::swap(&mut coin_start, &mut coin_end);
                std::mem::swap(&mut opp_start, &mut opp_end);
            }
        }
        let opp_min_t = std_min(self.ptt_t(opp_start), self.ptt_t(opp_end));
        let opp_max_t = std_max(self.ptt_t(opp_start), self.ptt_t(opp_end));
        loop {
            'body: {
                if coin_seg != self.ptt_segment(self.rec_coin_start(test)) {
                    break 'body;
                }
                if self.ptt_t(coin_start) < self.ptt_t(self.rec_coin_start(test)) {
                    break 'body;
                }
                if self.ptt_t(coin_end) > self.ptt_t(self.rec_coin_end(test)) {
                    break 'body;
                }
                if opp_seg != self.ptt_segment(self.rec_opp_start(test)) {
                    break 'body;
                }
                if opp_min_t
                    < std_min(
                        self.ptt_t(self.rec_opp_start(test)),
                        self.ptt_t(self.rec_opp_end(test)),
                    )
                {
                    break 'body;
                }
                if opp_max_t
                    > std_max(
                        self.ptt_t(self.rec_opp_start(test)),
                        self.ptt_t(self.rec_opp_end(test)),
                    )
                {
                    break 'body;
                }
                return true;
            }
            match self.rec_next(test) {
                Some(n) => test = n,
                None => break,
            }
        }
        false
    }

    /// `SkOpCoincidence::extend(...)`: if an existing pair overlaps the addition, extend it.
    // Port of: src/pathops/SkOpCoincidence.cpp#L166-L201 (chrome/m156)
    pub(crate) fn cs_extend(
        &mut self,
        set: CoinSetId,
        coin_start: PtTId,
        coin_end: PtTId,
        opp_start: PtTId,
        opp_end: PtTId,
    ) -> bool {
        let Some(head) = self.cs_list_head(set, CoinList::Head) else {
            return false;
        };
        let (mut coin_start, mut coin_end, mut opp_start, mut opp_end) =
            (coin_start, coin_end, opp_start, opp_end);
        let mut coin_seg = self.ptt_segment(coin_start);
        let mut opp_seg = self.ptt_segment(opp_start);
        if !self.cs_ordered_ptts(coin_start, opp_start) {
            std::mem::swap(&mut coin_seg, &mut opp_seg);
            std::mem::swap(&mut coin_start, &mut opp_start);
            std::mem::swap(&mut coin_end, &mut opp_end);
            if self.ptt_t(coin_start) > self.ptt_t(coin_end) {
                std::mem::swap(&mut coin_start, &mut coin_end);
                std::mem::swap(&mut opp_start, &mut opp_end);
            }
        }
        let opp_min_t = std_min(self.ptt_t(opp_start), self.ptt_t(opp_end));
        let mut test = head;
        loop {
            'body: {
                if coin_seg != self.ptt_segment(self.rec_coin_start(test)) {
                    break 'body;
                }
                if opp_seg != self.ptt_segment(self.rec_opp_start(test)) {
                    break 'body;
                }
                let o_test_min = std_min(
                    self.ptt_t(self.rec_opp_start(test)),
                    self.ptt_t(self.rec_opp_end(test)),
                );
                let o_test_max = std_max(
                    self.ptt_t(self.rec_opp_start(test)),
                    self.ptt_t(self.rec_opp_end(test)),
                );
                if (self.ptt_t(self.rec_coin_start(test)) <= self.ptt_t(coin_end)
                    && self.ptt_t(coin_start) <= self.ptt_t(self.rec_coin_end(test)))
                    || (o_test_min <= o_test_max && opp_min_t <= o_test_max)
                {
                    self.rec_extend(test, coin_start, coin_end, opp_start, opp_end);
                    return true;
                }
            }
            match self.rec_next(test) {
                Some(n) => test = n,
                None => break,
            }
        }
        false
    }

    /// `SkOpCoincidence::add(coinPtTStart, coinPtTEnd, oppPtTStart, oppPtTEnd)`: adds a new
    /// coincident pair to the head list.
    // Port of: src/pathops/SkOpCoincidence.cpp#L208-L232 (chrome/m156)
    pub(crate) fn cs_add(
        &mut self,
        set: CoinSetId,
        coin_start: PtTId,
        coin_end: PtTId,
        opp_start: PtTId,
        opp_end: PtTId,
    ) {
        if !self.cs_ordered_ptts(coin_start, opp_start) {
            if self.ptt_t(opp_start) < self.ptt_t(opp_end) {
                self.cs_add(set, opp_start, opp_end, coin_start, coin_end);
            } else {
                self.cs_add(set, opp_end, opp_start, coin_end, coin_start);
            }
            return;
        }
        // Choose the ptT at the front of the list to track.
        let coin_start = self.ptt_root(coin_start);
        let coin_end = self.ptt_root(coin_end);
        let opp_start = self.ptt_root(opp_start);
        let opp_end = self.ptt_root(opp_end);
        let coin_rec = self.alloc_coin();
        let head = self.coin_sets[set.0].head;
        self.rec_set(coin_rec, head, coin_start, coin_end, opp_start, opp_end);
        self.coin_sets[set.0].head = Some(coin_rec);
    }

    /// `SkOpPtT` root of a point: `ptT->span()->ptT()`.
    fn ptt_root(&self, ptt: PtTId) -> PtTId {
        self.span_ptt(self.ptt_span(ptt))
    }

    /// `SkOpCoincidence::addEndMovedSpans(base, testSpan)`: looks for a missed coincidence along
    /// the implied line between a moved end and the other curve.
    // Port of: src/pathops/SkOpCoincidence.cpp#L235-L300 (chrome/m156)
    pub(crate) fn cs_add_end_moved_base(
        &mut self,
        set: CoinSetId,
        base: SpanId,
        test_span: SpanId,
    ) -> bool {
        let stop = self.span_ptt(test_span);
        let mut test_ptt = stop;
        let base_seg = self.span_segment(base);
        let mut escape_hatch: i32 = 100_000;
        loop {
            test_ptt = self.ptt_next(test_ptt);
            if test_ptt == stop {
                break;
            }
            escape_hatch -= 1;
            if escape_hatch <= 0 {
                return false;
            }
            let test_seg = self.ptt_segment(test_ptt);
            if self.ptt_deleted(test_ptt) {
                continue;
            }
            if test_seg == base_seg {
                continue;
            }
            if self.span_ptt(self.ptt_span(test_ptt)) != test_ptt {
                continue;
            }
            if self.cs_contains_seg_t(set, base_seg, test_seg, self.ptt_t(test_ptt)) {
                continue;
            }
            // Intersect the perpendicular at base with testPtT's segment.
            let dxdy = self.seg_d_slope_at_t(base_seg, self.span_t(base));
            let pt: Point = self.span_pt(base);
            let ray = DLine::new([
                DPoint::new(f64::from(pt.x), f64::from(pt.y)),
                DPoint::new(f64::from(pt.x) + dxdy.y, f64::from(pt.y) - dxdy.x),
            ]);
            let mut i = Intersections::default();
            let test_pts = self.seg_pts(test_seg);
            curve_intersect_ray(
                self.seg_verb(test_seg),
                &test_pts,
                self.seg_weight(test_seg),
                &ray,
                &mut i,
            );
            for index in 0..i.used() {
                let t = i.t(0, index);
                if !between(0.0, t, 1.0) {
                    continue;
                }
                let opp_pt = i.pt(index);
                if !opp_pt.approximately_equal_sk(pt) {
                    continue;
                }
                let Some(opp_start) = self.seg_add_t(test_seg, t) else {
                    return false;
                };
                if opp_start == test_ptt {
                    continue;
                }
                let _ = self.span_add_opp(self.ptt_span(opp_start), base);
                if self.ptt_deleted(opp_start) {
                    continue;
                }
                let mut coin_seg = base_seg;
                let mut opp_seg = self.ptt_segment(opp_start);
                let mut coin_ts;
                let mut coin_te;
                let mut opp_ts;
                let mut opp_te;
                if self.cs_ordered_segs(coin_seg, opp_seg) {
                    coin_ts = self.span_t(base);
                    coin_te = self.span_t(test_span);
                    opp_ts = self.ptt_t(opp_start);
                    opp_te = self.ptt_t(test_ptt);
                } else {
                    std::mem::swap(&mut coin_seg, &mut opp_seg);
                    coin_ts = self.ptt_t(opp_start);
                    coin_te = self.ptt_t(test_ptt);
                    opp_ts = self.span_t(base);
                    opp_te = self.span_t(test_span);
                }
                if coin_ts > coin_te {
                    std::mem::swap(&mut coin_ts, &mut coin_te);
                    std::mem::swap(&mut opp_ts, &mut opp_te);
                }
                let mut added = false;
                if !self.cs_add_or_overlap(
                    set, coin_seg, opp_seg, coin_ts, coin_te, opp_ts, opp_te, &mut added,
                ) {
                    return false;
                }
            }
        }
        true
    }

    /// `SkOpCoincidence::addEndMovedSpans(ptT)`: both spans next to a moved end.
    // Port of: src/pathops/SkOpCoincidence.cpp#L302-L317 (chrome/m156)
    pub(crate) fn cs_add_end_moved_ptt(&mut self, set: CoinSetId, ptt: PtTId) -> bool {
        let base = self.ptt_span(ptt);
        if self.span_up_castable(base).is_none() {
            return false;
        }
        let Some(prev) = self.span_prev(base) else {
            return false;
        };
        if !self.span_is_canceled(prev) && !self.cs_add_end_moved_base(set, base, prev) {
            return false;
        }
        if !self.span_is_canceled(base) {
            let Some(next) = self.span_next(base) else {
                return false;
            };
            if !self.cs_add_end_moved_base(set, base, next) {
                return false;
            }
        }
        true
    }

    /// `SkOpCoincidence::addEndMovedSpans()`: if A is coincident with B and B includes an endpoint
    /// whose matching point in A is not the endpoint, look for a new coincident pair next to B.
    // Port of: src/pathops/SkOpCoincidence.cpp#L319-L363 (chrome/m156)
    pub(crate) fn cs_add_end_moved_all(&mut self, set: CoinSetId) -> bool {
        let Some(head) = self.coin_sets[set.0].head else {
            return true;
        };
        self.coin_sets[set.0].top = Some(head);
        self.coin_sets[set.0].head = None;
        let mut span = head;
        loop {
            let coin_start = self.rec_coin_start(span);
            let opp_start = self.rec_opp_start(span);
            if self.ptt_pt(coin_start) != self.ptt_pt(opp_start) {
                if self.ptt_t(coin_start) == 1.0 {
                    return false;
                }
                let on_end = self.ptt_t(coin_start) == 0.0;
                let o_on_end = zero_or_one(self.ptt_t(opp_start));
                if on_end {
                    if !o_on_end && !self.cs_add_end_moved_ptt(set, opp_start) {
                        return false;
                    }
                } else if o_on_end && !self.cs_add_end_moved_ptt(set, coin_start) {
                    return false;
                }
            }
            let coin_end = self.rec_coin_end(span);
            let opp_end = self.rec_opp_end(span);
            if self.ptt_pt(coin_end) != self.ptt_pt(opp_end) {
                let on_end = self.ptt_t(coin_end) == 1.0;
                let o_on_end = zero_or_one(self.ptt_t(opp_end));
                if on_end {
                    if !o_on_end && !self.cs_add_end_moved_ptt(set, opp_end) {
                        return false;
                    }
                } else if o_on_end && !self.cs_add_end_moved_ptt(set, coin_end) {
                    return false;
                }
            }
            match self.rec_next(span) {
                Some(n) => span = n,
                None => break,
            }
        }
        self.cs_restore_head(set);
        true
    }

    /// `SkOpCoincidence::restoreHead()`: appends the top list to the head, and removes records
    /// whose segments have collapsed in the meantime.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1002-L1024 (chrome/m156)
    pub(crate) fn cs_restore_head(&mut self, set: CoinSetId) {
        let top = self.coin_sets[set.0].top;
        match self.cs_list_last(set, CoinList::Head) {
            None => self.coin_sets[set.0].head = top,
            Some(last) => self.coin_spans[last.0].next = top,
        }
        self.coin_sets[set.0].top = None;
        let mut prev = None;
        let mut cur = self.coin_sets[set.0].head;
        while let Some(c) = cur {
            let next = self.rec_next(c);
            let coin_seg = self.ptt_segment(self.rec_coin_start(c));
            let opp_seg = self.ptt_segment(self.rec_opp_start(c));
            if self.seg_done(coin_seg) || self.seg_done(opp_seg) {
                self.cs_unlink(set, CoinList::Head, prev, c);
            } else {
                prev = Some(c);
            }
            cur = next;
        }
    }

    /// The last record of a list, or `None` for an empty list.
    pub(crate) fn cs_list_last(&self, set: CoinSetId, list: CoinList) -> Option<CoinId> {
        let mut cur = self.cs_list_head(set, list)?;
        while let Some(next) = self.rec_next(cur) {
            cur = next;
        }
        Some(cur)
    }

    /// `SkOpCoincidence::addExpanded()`: for each coincident pair, match the spans. If the spans
    /// do not match, add the missing point to the segment and loop it in the opposite span.
    // Port of: src/pathops/SkOpCoincidence.cpp#L366-L460 (chrome/m156)
    pub(crate) fn cs_add_expanded(&mut self, set: CoinSetId) -> bool {
        let Some(mut coin) = self.coin_sets[set.0].head else {
            return true;
        };
        loop {
            let start_ptt = self.rec_coin_start(coin);
            let o_start_ptt = self.rec_opp_start(coin);
            let mut prior_t = self.ptt_t(start_ptt);
            let mut o_prior_t = self.ptt_t(o_start_ptt);
            if !self.ptt_contains_ptt(start_ptt, o_start_ptt) {
                return false;
            }
            let start = self.ptt_span(start_ptt);
            let o_start = self.ptt_span(o_start_ptt);
            let mut end = self.ptt_span(self.rec_coin_end(coin));
            let mut o_end = self.ptt_span(self.rec_opp_end(coin));
            if self.span_deleted(o_end) {
                return false;
            }
            if self.span_up_castable(start).is_none() {
                return false;
            }
            let Some(mut test) = self.span_next(self.span_up_cast(start)) else {
                return false;
            };
            let flipped = self.rec_flipped(coin);
            if !flipped && self.span_up_castable(o_start).is_none() {
                return false;
            }
            let Some(mut o_test) = (if flipped {
                self.span_prev(o_start)
            } else {
                self.span_next(self.span_up_cast(o_start))
            }) else {
                return false;
            };
            let seg = self.span_segment(start);
            let o_seg = self.span_segment(o_start);
            while test != end || o_test != o_end {
                let contained_opp = self.ptt_contains_seg(self.span_ptt(test), o_seg);
                let contained_this = self.ptt_contains_seg(self.span_ptt(o_test), seg);
                if contained_opp.is_none() || contained_this.is_none() {
                    // Choose the ends, or the first common pt-t list shared by both.
                    let next_t;
                    let o_next_t;
                    if let Some(co) = contained_opp {
                        next_t = self.span_t(test);
                        o_next_t = self.ptt_t(co);
                    } else if let Some(ct) = contained_this {
                        next_t = self.ptt_t(ct);
                        o_next_t = self.span_t(o_test);
                    } else {
                        // Iterate until a pt-t list is found that contains the other.
                        let mut walk = test;
                        let walk_opp = loop {
                            if self.span_up_castable(walk).is_none() {
                                return false;
                            }
                            let Some(n) = self.span_next(self.span_up_cast(walk)) else {
                                return false;
                            };
                            walk = n;
                            let found = self.ptt_contains_seg(self.span_ptt(walk), o_seg);
                            if found.is_some() || walk == self.ptt_span(self.rec_coin_end(coin)) {
                                break found;
                            }
                        };
                        let Some(walk_opp) = walk_opp else {
                            return false;
                        };
                        next_t = self.span_t(walk);
                        o_next_t = self.ptt_t(walk_opp);
                    }
                    // Use t ranges to guess which one is missing.
                    let start_range = next_t - prior_t;
                    if is_zero_or_nan(start_range) {
                        return false;
                    }
                    let start_part = (self.span_t(test) - prior_t) / start_range;
                    let o_start_range = o_next_t - o_prior_t;
                    if is_zero_or_nan(o_start_range) {
                        return false;
                    }
                    let o_start_part = (self.span_t(o_test) - o_prior_t) / o_start_range;
                    if start_part == o_start_part {
                        return false;
                    }
                    let add_to_opp = if contained_opp.is_none() && contained_this.is_none() {
                        start_part < o_start_part
                    } else {
                        contained_this.is_some()
                    };
                    let mut start_over = false;
                    let success = if add_to_opp {
                        self.seg_add_expanded(
                            o_seg,
                            o_prior_t + o_start_range * start_part,
                            test,
                            &mut start_over,
                        )
                    } else {
                        self.seg_add_expanded(
                            seg,
                            prior_t + start_range * o_start_part,
                            o_test,
                            &mut start_over,
                        )
                    };
                    if !success {
                        return false;
                    }
                    if start_over {
                        test = start;
                        o_test = o_start;
                    }
                    end = self.ptt_span(self.rec_coin_end(coin));
                    o_end = self.ptt_span(self.rec_opp_end(coin));
                }
                if test != end {
                    if self.span_up_castable(test).is_none() {
                        return false;
                    }
                    prior_t = self.span_t(test);
                    let Some(n) = self.span_next(self.span_up_cast(test)) else {
                        return false;
                    };
                    test = n;
                }
                if o_test != o_end {
                    o_prior_t = self.span_t(o_test);
                    let next_o = if flipped {
                        self.span_prev(o_test)
                    } else {
                        if self.span_up_castable(o_test).is_none() {
                            return false;
                        }
                        self.span_next(self.span_up_cast(o_test))
                    };
                    let Some(n) = next_o else {
                        return false;
                    };
                    o_test = n;
                }
            }
            match self.rec_next(coin) {
                Some(n) => coin = n,
                None => break,
            }
        }
        true
    }

    /// `SkOpCoincidence::TRange(overS, t, coinSeg)`: maps `t` on `over_s`'s segment to `coin_seg`.
    // Port of: src/pathops/SkOpCoincidence.cpp#L462-L492 (chrome/m156)
    fn coin_t_range(&self, over_s: PtTId, t: f64, coin_seg: SegId) -> f64 {
        let mut work = self.ptt_span(over_s);
        let mut found_start: Option<PtTId> = None;
        let mut found_end: Option<PtTId> = None;
        let mut coin_start: Option<PtTId> = None;
        let mut coin_end: Option<PtTId> = None;
        loop {
            match self.span_contains_seg(work, coin_seg) {
                None => {
                    if self.span_final(work) {
                        break;
                    }
                }
                Some(contained) => {
                    if self.span_t(work) <= t {
                        coin_start = Some(contained);
                        found_start = Some(self.span_ptt(work));
                    }
                    if self.span_t(work) >= t {
                        coin_end = Some(contained);
                        found_end = Some(self.span_ptt(work));
                        break;
                    }
                }
            }
            match self.span_next(self.span_up_cast(work)) {
                Some(n) => work = n,
                None => break,
            }
        }
        let (Some(cs), Some(ce), Some(fs), Some(fe)) =
            (coin_start, coin_end, found_start, found_end)
        else {
            return 1.0;
        };
        // Remap over1s, over1e, coinPtTStart, coinPtTEnd to the smallest range that captures
        // over1s, as Skia does.
        let denom = self.ptt_t(fe) - self.ptt_t(fs);
        let s_ratio = if denom == 0.0 {
            1.0
        } else {
            (t - self.ptt_t(fs)) / denom
        };
        self.ptt_t(cs) + (self.ptt_t(ce) - self.ptt_t(cs)) * s_ratio
    }

    /// `SkOpCoincidence::checkOverlap(check, ...)`: returns false if the span is already
    /// included, and appends partial overlaps to `overlaps`.
    // Port of: src/pathops/SkOpCoincidence.cpp#L494-L545 (chrome/m156)
    // The parameter list mirrors the C++ signature, which takes the same pointers and ranges.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn cs_check_overlap(
        &self,
        check: Option<CoinId>,
        coin_seg: SegId,
        opp_seg: SegId,
        coin_ts: f64,
        coin_te: f64,
        opp_ts: f64,
        opp_te: f64,
        overlaps: &mut Vec<CoinId>,
    ) -> bool {
        if !self.cs_ordered_segs(coin_seg, opp_seg) {
            if opp_ts < opp_te {
                return self.cs_check_overlap(
                    check, opp_seg, coin_seg, opp_ts, opp_te, coin_ts, coin_te, overlaps,
                );
            }
            return self.cs_check_overlap(
                check, opp_seg, coin_seg, opp_te, opp_ts, coin_te, coin_ts, overlaps,
            );
        }
        let swap_opp = opp_ts > opp_te;
        let (opp_ts, opp_te) = if swap_opp {
            (opp_te, opp_ts)
        } else {
            (opp_ts, opp_te)
        };
        let mut cur = check;
        while let Some(c) = cur {
            cur = self.rec_next(c);
            'body: {
                if self.ptt_segment(self.rec_coin_start(c)) != coin_seg {
                    break 'body;
                }
                if self.ptt_segment(self.rec_opp_start(c)) != opp_seg {
                    break 'body;
                }
                let check_ts = self.ptt_t(self.rec_coin_start(c));
                let check_te = self.ptt_t(self.rec_coin_end(c));
                let coin_outside = coin_te < check_ts || coin_ts > check_te;
                let mut o_check_ts = self.ptt_t(self.rec_opp_start(c));
                let mut o_check_te = self.ptt_t(self.rec_opp_end(c));
                if swap_opp {
                    if o_check_ts <= o_check_te {
                        return false;
                    }
                    std::mem::swap(&mut o_check_ts, &mut o_check_te);
                }
                let opp_outside = opp_te < o_check_ts || opp_ts > o_check_te;
                if coin_outside && opp_outside {
                    break 'body;
                }
                let coin_inside = coin_te <= check_te && coin_ts >= check_ts;
                let opp_inside = opp_te <= o_check_te && opp_ts >= o_check_ts;
                if coin_inside && opp_inside {
                    // Already included, do nothing.
                    return false;
                }
                // Partial overlap: extend the existing entry.
                overlaps.push(c);
            }
        }
        true
    }

    /// `SkOpCoincidence::addOrOverlap(...)`: adds the coincident pair, or extends the existing
    /// pairs that overlap it. If the caller is `addEndMovedSpans`, a false return aborts.
    // Port of: src/pathops/SkOpCoincidence.cpp#L547-L668 (chrome/m156)
    // The parameter list mirrors the C++ signature, which takes the same pointers and ranges.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn cs_add_or_overlap(
        &mut self,
        set: CoinSetId,
        coin_seg: SegId,
        opp_seg: SegId,
        coin_ts: f64,
        coin_te: f64,
        opp_ts: f64,
        opp_te: f64,
        added: &mut bool,
    ) -> bool {
        let mut overlaps: Vec<CoinId> = Vec::new();
        let Some(top) = self.coin_sets[set.0].top else {
            return false;
        };
        if !self.cs_check_overlap(
            Some(top),
            coin_seg,
            opp_seg,
            coin_ts,
            coin_te,
            opp_ts,
            opp_te,
            &mut overlaps,
        ) {
            return true;
        }
        let head = self.coin_sets[set.0].head;
        if head.is_some()
            && !self.cs_check_overlap(
                head,
                coin_seg,
                opp_seg,
                coin_ts,
                coin_te,
                opp_ts,
                opp_te,
                &mut overlaps,
            )
        {
            return true;
        }
        let overlap = overlaps.first().copied();
        // Combine overlaps before continuing.
        if let Some(ov) = overlap {
            for index in 1..overlaps.len() {
                let test = overlaps[index];
                if self.ptt_t(self.rec_coin_start(ov)) > self.ptt_t(self.rec_coin_start(test)) {
                    let p = self.rec_coin_start(test);
                    self.rec_set_coin_start(ov, p);
                }
                if self.ptt_t(self.rec_coin_end(ov)) < self.ptt_t(self.rec_coin_end(test)) {
                    let p = self.rec_coin_end(test);
                    self.rec_set_coin_end(ov, p);
                }
                let flipped = self.rec_flipped(ov);
                let set_opp_start = if flipped {
                    self.ptt_t(self.rec_opp_start(ov)) < self.ptt_t(self.rec_opp_start(test))
                } else {
                    self.ptt_t(self.rec_opp_start(ov)) > self.ptt_t(self.rec_opp_start(test))
                };
                if set_opp_start {
                    let p = self.rec_opp_start(test);
                    self.rec_set_opp_start(ov, p);
                }
                let flipped = self.rec_flipped(ov);
                let set_opp_end = if flipped {
                    self.ptt_t(self.rec_opp_end(ov)) > self.ptt_t(self.rec_opp_end(test))
                } else {
                    self.ptt_t(self.rec_opp_end(ov)) < self.ptt_t(self.rec_opp_end(test))
                };
                if set_opp_end {
                    let p = self.rec_opp_end(test);
                    self.rec_set_opp_end(ov, p);
                }
                if self.coin_sets[set.0].head.is_none()
                    || !self.cs_release_rec(set, CoinList::Head, test)
                {
                    let released = self.cs_release_rec(set, CoinList::Top, test);
                    debug_assert!(
                        released,
                        "coincidence record to release is missing from fTop"
                    );
                }
            }
        }
        let cs = self.seg_existing(coin_seg, coin_ts, Some(opp_seg));
        let ce = self.seg_existing(coin_seg, coin_te, Some(opp_seg));
        if let (Some(ov), Some(cs_v), Some(ce_v)) = (overlap, cs, ce)
            && self.rec_contains(ov, cs_v, ce_v)
        {
            return true;
        }
        if cs.is_some() && cs == ce {
            return false;
        }
        let os = self.seg_existing(opp_seg, opp_ts, Some(coin_seg));
        let oe = self.seg_existing(opp_seg, opp_te, Some(coin_seg));
        if let (Some(ov), Some(os_v), Some(oe_v)) = (overlap, os, oe)
            && self.rec_contains(ov, os_v, oe_v)
        {
            return true;
        }
        if cs.is_some_and(|p| self.ptt_deleted(p))
            || os.is_some_and(|p| self.ptt_deleted(p))
            || ce.is_some_and(|p| self.ptt_deleted(p))
            || oe.is_some_and(|p| self.ptt_deleted(p))
        {
            return false;
        }
        let cs_existing = if cs.is_none() {
            self.seg_existing(coin_seg, coin_ts, None)
        } else {
            None
        };
        let ce_existing = if ce.is_none() {
            self.seg_existing(coin_seg, coin_te, None)
        } else {
            None
        };
        if cs_existing.is_some() && cs_existing == ce_existing {
            return false;
        }
        if let Some(ce_ex) = ce_existing {
            let check = cs_existing.or(cs);
            if ce_existing == cs || self.ptt_contains_opt(ce_ex, check) {
                return false;
            }
        }
        let os_existing = if os.is_none() {
            self.seg_existing(opp_seg, opp_ts, None)
        } else {
            None
        };
        let oe_existing = if oe.is_none() {
            self.seg_existing(opp_seg, opp_te, None)
        } else {
            None
        };
        if os_existing.is_some() && os_existing == oe_existing {
            return false;
        }
        if let Some(os_ex) = os_existing {
            let check = oe_existing.or(oe);
            if os_existing == oe || self.ptt_contains_opt(os_ex, check) {
                return false;
            }
        }
        if let Some(oe_ex) = oe_existing {
            let check = os_existing.or(os);
            if oe_existing == os || self.ptt_contains_opt(oe_ex, check) {
                return false;
            }
        }
        let mut cs = cs;
        let mut ce = ce;
        let mut os = os;
        let mut oe = oe;
        if cs.is_none() || os.is_none() {
            let cs_writable = match cs {
                Some(p) => Some(p),
                None => self.seg_add_t(coin_seg, coin_ts),
            };
            if cs_writable == ce {
                return true;
            }
            let os_writable = match os {
                Some(p) => Some(p),
                None => self.seg_add_t(opp_seg, opp_ts),
            };
            let (Some(cs_w), Some(os_w)) = (cs_writable, os_writable) else {
                return false;
            };
            let _ = self.span_add_opp(self.ptt_span(cs_w), self.ptt_span(os_w));
            cs = Some(cs_w);
            os = self.ptt_active(os_w);
            if os.is_none() {
                return false;
            }
            if ce.is_some_and(|p| self.ptt_deleted(p)) || oe.is_some_and(|p| self.ptt_deleted(p)) {
                return false;
            }
        }
        if ce.is_none() || oe.is_none() {
            let ce_writable = match ce {
                Some(p) => Some(p),
                None => self.seg_add_t(coin_seg, coin_te),
            };
            let oe_writable = match oe {
                Some(p) => Some(p),
                None => self.seg_add_t(opp_seg, opp_te),
            };
            let (Some(ce_w), Some(oe_w)) = (ce_writable, oe_writable) else {
                return false;
            };
            if !self.span_add_opp(self.ptt_span(ce_w), self.ptt_span(oe_w)) {
                return false;
            }
            ce = Some(ce_w);
            oe = Some(oe_w);
        }
        let (Some(cs), Some(os), Some(ce), Some(oe)) = (cs, os, ce, oe) else {
            return false;
        };
        if self.ptt_deleted(cs)
            || self.ptt_deleted(os)
            || self.ptt_deleted(ce)
            || self.ptt_deleted(oe)
        {
            return false;
        }
        if self.ptt_contains_ptt(cs, ce) || self.ptt_contains_ptt(os, oe) {
            return false;
        }
        let mut result = true;
        if let Some(ov) = overlap {
            if self.ptt_segment(self.rec_coin_start(ov)) == coin_seg {
                result = self.rec_extend(ov, cs, ce, os, oe);
            } else {
                let (mut cs2, mut ce2, mut os2, mut oe2) = (cs, ce, os, oe);
                if self.ptt_t(os2) > self.ptt_t(oe2) {
                    std::mem::swap(&mut cs2, &mut ce2);
                    std::mem::swap(&mut os2, &mut oe2);
                }
                result = self.rec_extend(ov, os2, oe2, cs2, ce2);
            }
        } else {
            self.cs_add(set, cs, ce, os, oe);
        }
        if result {
            *added = true;
        }
        true
    }

    /// `SkOpCoincidence::addOverlap(...)`: adds a pair found by `findOverlaps` to `set`.
    // Port of: src/pathops/SkOpCoincidence.cpp#L686-L735 (chrome/m156)
    pub(crate) fn cs_add_overlap(
        &mut self,
        set: CoinSetId,
        seg1: SegId,
        seg1o: SegId,
        seg2: SegId,
        seg2o: SegId,
        over_s: PtTId,
        over_e: PtTId,
    ) -> bool {
        let Some(mut s1) = self.ptt_find(over_s, seg1) else {
            return false;
        };
        let Some(mut e1) = self.ptt_find(over_e, seg1) else {
            return false;
        };
        if self.span_wind_value(self.ptt_span(self.ptt_starter(s1, e1))) == 0 {
            let Some(s1o) = self.ptt_find(over_s, seg1o) else {
                return false;
            };
            let Some(e1o) = self.ptt_find(over_e, seg1o) else {
                return false;
            };
            s1 = s1o;
            e1 = e1o;
            if self.span_wind_value(self.ptt_span(self.ptt_starter(s1, e1))) == 0 {
                return true;
            }
        }
        let Some(mut s2) = self.ptt_find(over_s, seg2) else {
            return false;
        };
        let Some(mut e2) = self.ptt_find(over_e, seg2) else {
            return false;
        };
        if self.span_wind_value(self.ptt_span(self.ptt_starter(s2, e2))) == 0 {
            let Some(s2o) = self.ptt_find(over_s, seg2o) else {
                return false;
            };
            let Some(e2o) = self.ptt_find(over_e, seg2o) else {
                return false;
            };
            s2 = s2o;
            e2 = e2o;
            if self.span_wind_value(self.ptt_span(self.ptt_starter(s2, e2))) == 0 {
                return true;
            }
        }
        if self.ptt_segment(s1) == self.ptt_segment(s2) {
            return true;
        }
        if self.ptt_t(s1) > self.ptt_t(e1) {
            std::mem::swap(&mut s1, &mut e1);
            std::mem::swap(&mut s2, &mut e2);
        }
        self.cs_add(set, s1, e1, s2, e2);
        true
    }

    /// `SkOpCoincidence::addIfMissing(...)`: adds the coincident pair for an overlap that
    /// the intersection pass did not find.
    // Port of: src/pathops/SkOpCoincidence.cpp#L735-L764 (chrome/m156)
    // The parameter list mirrors the C++ signature, which takes the same pointers and ranges.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn cs_add_if_missing(
        &mut self,
        set: CoinSetId,
        over1s: PtTId,
        over2s: PtTId,
        t_start: f64,
        t_end: f64,
        coin_seg: SegId,
        opp_seg: SegId,
        added: &mut bool,
    ) -> bool {
        let mut coin_ts = self.coin_t_range(over1s, t_start, coin_seg);
        let mut coin_te = self.coin_t_range(over1s, t_end, coin_seg);
        let result = self.seg_collapsed(coin_seg, coin_ts, coin_te);
        if result != Collapsed::No {
            return result == Collapsed::Yes;
        }
        let mut opp_ts = self.coin_t_range(over2s, t_start, opp_seg);
        let mut opp_te = self.coin_t_range(over2s, t_end, opp_seg);
        let result = self.seg_collapsed(opp_seg, opp_ts, opp_te);
        if result != Collapsed::No {
            return result == Collapsed::Yes;
        }
        if coin_ts > coin_te {
            std::mem::swap(&mut coin_ts, &mut coin_te);
            std::mem::swap(&mut opp_ts, &mut opp_te);
        }
        // The caller treats "nothing to add" as success, so the result is ignored.
        let _ = self.cs_add_or_overlap(
            set, coin_seg, opp_seg, coin_ts, coin_te, opp_ts, opp_te, added,
        );
        true
    }

    /// `SkOpCoincidence::addMissing(added)`: detects overlaps of different coincident runs on the
    /// same segment, and adds the spans they imply.
    // Port of: src/pathops/SkOpCoincidence.cpp#L766-L837 (chrome/m156)
    pub(crate) fn cs_add_missing(&mut self, set: CoinSetId, added: &mut bool) -> bool {
        *added = false;
        let Some(first) = self.coin_sets[set.0].head else {
            return true;
        };
        self.coin_sets[set.0].top = Some(first);
        self.coin_sets[set.0].head = None;
        let mut outer = first;
        loop {
            // addIfMissing can modify the list that this walks; save head so the walker iterates
            // over the old data unperturbed.
            let ocs = self.rec_coin_start(outer);
            if self.ptt_deleted(ocs) {
                return false;
            }
            let outer_coin = self.ptt_segment(ocs);
            if self.seg_done(outer_coin) {
                return false;
            }
            let oos = self.rec_opp_start(outer);
            if self.ptt_deleted(oos) {
                return true;
            }
            let outer_opp = self.ptt_segment(oos);
            let mut inner_opt = self.rec_next(outer);
            while let Some(inner) = inner_opt {
                let ics = self.rec_coin_start(inner);
                if self.ptt_deleted(ics) {
                    return false;
                }
                let inner_coin = self.ptt_segment(ics);
                if self.seg_done(inner_coin) {
                    return false;
                }
                let ios = self.rec_opp_start(inner);
                if self.ptt_deleted(ios) {
                    return false;
                }
                let inner_opp = self.ptt_segment(ios);
                if outer_coin == inner_coin {
                    let oce = self.rec_coin_end(outer);
                    if self.ptt_deleted(oce) {
                        return true;
                    }
                    let ice = self.rec_coin_end(inner);
                    if self.ptt_deleted(ice) {
                        return false;
                    }
                    if outer_opp != inner_opp
                        && let Some((ov_s, ov_e)) = self.coin_overlap(ocs, oce, ics, ice)
                    {
                        let s1 = self.ptt_starter(ocs, oce);
                        let s2 = self.ptt_starter(ics, ice);
                        if !self
                            .cs_add_if_missing(set, s1, s2, ov_s, ov_e, outer_opp, inner_opp, added)
                        {
                            return false;
                        }
                    }
                } else if outer_coin == inner_opp {
                    let oce = self.rec_coin_end(outer);
                    if self.ptt_deleted(oce) {
                        return false;
                    }
                    let ioe = self.rec_opp_end(inner);
                    if self.ptt_deleted(ioe) {
                        return false;
                    }
                    if outer_opp != inner_coin
                        && let Some((ov_s, ov_e)) = self.coin_overlap(ocs, oce, ios, ioe)
                    {
                        let s1 = self.ptt_starter(ocs, oce);
                        let s2 = self.ptt_starter(ios, ioe);
                        if !self.cs_add_if_missing(
                            set, s1, s2, ov_s, ov_e, outer_opp, inner_coin, added,
                        ) {
                            return false;
                        }
                    }
                } else if outer_opp == inner_coin {
                    let ooe = self.rec_opp_end(outer);
                    if self.ptt_deleted(ooe) {
                        return false;
                    }
                    let ice = self.rec_coin_end(inner);
                    if self.ptt_deleted(ice) {
                        return false;
                    }
                    if let Some((ov_s, ov_e)) = self.coin_overlap(oos, ooe, ics, ice) {
                        let s1 = self.ptt_starter(oos, ooe);
                        let s2 = self.ptt_starter(ics, ice);
                        if !self.cs_add_if_missing(
                            set, s1, s2, ov_s, ov_e, outer_coin, inner_opp, added,
                        ) {
                            return false;
                        }
                    }
                } else if outer_opp == inner_opp {
                    let ooe = self.rec_opp_end(outer);
                    if self.ptt_deleted(ooe) {
                        return false;
                    }
                    let ioe = self.rec_opp_end(inner);
                    if self.ptt_deleted(ioe) {
                        return true;
                    }
                    if let Some((ov_s, ov_e)) = self.coin_overlap(oos, ooe, ios, ioe) {
                        let s1 = self.ptt_starter(oos, ooe);
                        let s2 = self.ptt_starter(ios, ioe);
                        if !self.cs_add_if_missing(
                            set, s1, s2, ov_s, ov_e, outer_coin, inner_coin, added,
                        ) {
                            return false;
                        }
                    }
                }
                inner_opt = self.rec_next(inner);
            }
            match self.rec_next(outer) {
                Some(n) => outer = n,
                None => break,
            }
        }
        self.cs_restore_head(set);
        true
    }

    /// `SkOpCoincidence::overlap(coin1s, coin1e, coin2s, coin2e, &overS, &overE)`: returns the
    /// overlapping `t` range, if there is one.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1245-L1251 (chrome/m156)
    fn coin_overlap(
        &self,
        coin1s: PtTId,
        coin1e: PtTId,
        coin2s: PtTId,
        coin2e: PtTId,
    ) -> Option<(f64, f64)> {
        let over_s = std_max(
            std_min(self.ptt_t(coin1s), self.ptt_t(coin1e)),
            std_min(self.ptt_t(coin2s), self.ptt_t(coin2e)),
        );
        let over_e = std_min(
            std_max(self.ptt_t(coin1s), self.ptt_t(coin1e)),
            std_max(self.ptt_t(coin2s), self.ptt_t(coin2e)),
        );
        if over_s < over_e {
            Some((over_s, over_e))
        } else {
            None
        }
    }

    /// `SkOpCoincidence::mark()`: sets up the coincidence links in the segments when the
    /// coincidence crosses multiple spans.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1100-L1137 (chrome/m156)
    pub(crate) fn cs_mark(&mut self, set: CoinSetId) -> bool {
        let Some(mut coin) = self.coin_sets[set.0].head else {
            return true;
        };
        loop {
            let start = self.ptt_span(self.rec_coin_start(coin));
            if self.span_up_castable(start).is_none() {
                return false;
            }
            if self.span_deleted(start) {
                return false;
            }
            let end = self.ptt_span(self.rec_coin_end(coin));
            let mut o_start = self.ptt_span(self.rec_opp_start(coin));
            let mut o_end = self.ptt_span(self.rec_opp_end(coin));
            if self.span_deleted(o_end) {
                return false;
            }
            let flipped = self.rec_flipped(coin);
            if flipped {
                std::mem::swap(&mut o_start, &mut o_end);
            }
            // Coin and opp spans may not match up. Mark the ends, and then let the interior be
            // marked as many times as the spans allow.
            if self.span_up_castable(o_start).is_none() {
                return false;
            }
            let o_start_up = self.span_up_cast(o_start);
            self.span_insert_coincidence(start, o_start_up);
            self.span_insert_coin_end(end, o_end);
            let segment = self.span_segment(start);
            let o_segment = self.span_segment(o_start);
            let Some(ordered) = self.rec_ordered(coin) else {
                return false;
            };
            let mut next = start;
            loop {
                let Some(n) = self.span_next(self.span_up_cast(next)) else {
                    return false;
                };
                next = n;
                if next == end {
                    break;
                }
                if self.span_up_castable(next).is_none() {
                    return false;
                }
                let next_up = self.span_up_cast(next);
                if !self.span_insert_coincidence_seg(next_up, o_segment, flipped, ordered) {
                    return false;
                }
            }
            let mut o_next = o_start;
            loop {
                let Some(n) = self.span_next(self.span_up_cast(o_next)) else {
                    return false;
                };
                o_next = n;
                if o_next == o_end {
                    break;
                }
                if self.span_up_castable(o_next).is_none() {
                    return false;
                }
                let o_next_up = self.span_up_cast(o_next);
                if !self.span_insert_coincidence_seg(o_next_up, segment, flipped, ordered) {
                    return false;
                }
            }
            match self.rec_next(coin) {
                Some(n) => coin = n,
                None => break,
            }
        }
        true
    }

    /// `SkOpCoincidence::markCollapsed(headPtr, test)` for one list.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1139-L1155 (chrome/m156)
    pub(crate) fn cs_mark_collapsed_list(&mut self, set: CoinSetId, list: CoinList, test: PtTId) {
        let mut prev = None;
        let mut cur = self.cs_list_head(set, list);
        while let Some(c) = cur {
            let next = self.rec_next(c);
            if self.rec_collapsed(c, test) {
                if zero_or_one(self.ptt_t(self.rec_coin_start(c)))
                    && zero_or_one(self.ptt_t(self.rec_coin_end(c)))
                {
                    let seg = self.ptt_segment(self.rec_coin_start(c));
                    self.seg_mark_all_done(seg);
                }
                if zero_or_one(self.ptt_t(self.rec_opp_start(c)))
                    && zero_or_one(self.ptt_t(self.rec_opp_end(c)))
                {
                    let seg = self.ptt_segment(self.rec_opp_start(c));
                    self.seg_mark_all_done(seg);
                }
                self.cs_unlink(set, list, prev, c);
            } else {
                prev = Some(c);
            }
            cur = next;
        }
    }

    /// `SkOpCoincidence::markCollapsed(test)` for both lists.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1157-L1164 (chrome/m156)
    pub(crate) fn cs_mark_collapsed(&mut self, set: CoinSetId, test: PtTId) {
        self.cs_mark_collapsed_list(set, CoinList::Head, test);
        self.cs_mark_collapsed_list(set, CoinList::Top, test);
    }

    /// `SkOpCoincidence::release(headPtr, remove)`: unlinks `remove` from `list`.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1046-L1055 (chrome/m156)
    pub(crate) fn cs_release_rec(
        &mut self,
        set: CoinSetId,
        list: CoinList,
        remove: CoinId,
    ) -> bool {
        let mut prev = None;
        let mut cur = self.cs_list_head(set, list);
        while let Some(c) = cur {
            if c == remove {
                self.cs_unlink(set, list, prev, c);
                return true;
            }
            prev = Some(c);
            cur = self.rec_next(c);
        }
        false
    }

    /// `SkOpCoincidence::release(headPtr, deleted)`: removes every record that uses the segment.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1057-L1072 (chrome/m156)
    pub(crate) fn cs_release_seg_list(&mut self, set: CoinSetId, list: CoinList, deleted: SegId) {
        let mut prev = None;
        let mut cur = self.cs_list_head(set, list);
        while let Some(c) = cur {
            let next = self.rec_next(c);
            if self.ptt_segment(self.rec_coin_start(c)) == deleted
                || self.ptt_segment(self.rec_coin_end(c)) == deleted
                || self.ptt_segment(self.rec_opp_start(c)) == deleted
                || self.ptt_segment(self.rec_opp_end(c)) == deleted
            {
                self.cs_unlink(set, list, prev, c);
            } else {
                prev = Some(c);
            }
            cur = next;
        }
    }

    /// `SkOpCoincidence::release(deleted)`: both lists.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1074-L1077 (chrome/m156)
    pub(crate) fn cs_release_seg_all(&mut self, set: CoinSetId, deleted: SegId) {
        self.cs_release_seg_list(set, CoinList::Head, deleted);
        self.cs_release_seg_list(set, CoinList::Top, deleted);
    }

    /// `SkOpCoincidence::releaseDeleted(headPtr)`: removes records whose first point is deleted.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1025-L1040 (chrome/m156)
    pub(crate) fn cs_release_deleted_list(&mut self, set: CoinSetId, list: CoinList) {
        let mut prev = None;
        let mut cur = self.cs_list_head(set, list);
        while let Some(c) = cur {
            let next = self.rec_next(c);
            if self.ptt_deleted(self.rec_coin_start(c)) {
                self.cs_unlink(set, list, prev, c);
            } else {
                prev = Some(c);
            }
            cur = next;
        }
    }

    /// `SkOpCoincidence::releaseDeleted()`: both lists.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1042-L1044 (chrome/m156)
    pub(crate) fn cs_release_deleted(&mut self, set: CoinSetId) {
        self.cs_release_deleted_list(set, CoinList::Head);
        self.cs_release_deleted_list(set, CoinList::Top);
    }

    /// `SkOpCoincidence::fixUp(headPtr, deleted, kept)`: replaces the deleted point by the kept
    /// one, removing records that would become empty.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1166-L1204 (chrome/m156)
    pub(crate) fn cs_fix_up_list(
        &mut self,
        set: CoinSetId,
        list: CoinList,
        deleted: PtTId,
        kept: PtTId,
    ) {
        let mut prev = None;
        let mut cur = self.cs_list_head(set, list);
        while let Some(coin) = cur {
            let next = self.rec_next(coin);
            let mut removed = false;
            if self.rec_coin_start(coin) == deleted {
                if self.ptt_span(self.rec_coin_end(coin)) == self.ptt_span(kept) {
                    self.cs_unlink(set, list, prev, coin);
                    removed = true;
                } else {
                    self.rec_set_coin_start(coin, kept);
                }
            }
            if !removed && self.rec_coin_end(coin) == deleted {
                if self.ptt_span(self.rec_coin_start(coin)) == self.ptt_span(kept) {
                    self.cs_unlink(set, list, prev, coin);
                    removed = true;
                } else {
                    self.rec_set_coin_end(coin, kept);
                }
            }
            if !removed && self.rec_opp_start(coin) == deleted {
                if self.ptt_span(self.rec_opp_end(coin)) == self.ptt_span(kept) {
                    self.cs_unlink(set, list, prev, coin);
                    removed = true;
                } else {
                    self.rec_set_opp_start(coin, kept);
                }
            }
            if !removed && self.rec_opp_end(coin) == deleted {
                if self.ptt_span(self.rec_opp_start(coin)) == self.ptt_span(kept) {
                    self.cs_unlink(set, list, prev, coin);
                    removed = true;
                } else {
                    self.rec_set_opp_end(coin, kept);
                }
            }
            if !removed {
                prev = Some(coin);
            }
            cur = next;
        }
    }

    /// `SkOpCoincidence::fixUp(deleted, kept)`: both lists.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1158-L1164 (chrome/m156)
    pub(crate) fn cs_fix_up(&mut self, set: CoinSetId, deleted: PtTId, kept: PtTId) {
        if self.cs_list_head(set, CoinList::Head).is_some() {
            self.cs_fix_up_list(set, CoinList::Head, deleted, kept);
        }
        if self.cs_list_head(set, CoinList::Top).is_some() {
            self.cs_fix_up_list(set, CoinList::Top, deleted, kept);
        }
    }

    /// `SkOpCoincidence::expand()`: expands the coincident ranges by checking adjacent spans.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1206-L1233 (chrome/m156)
    pub(crate) fn cs_expand(&mut self, set: CoinSetId) -> bool {
        let Some(mut coin) = self.coin_sets[set.0].head else {
            return false;
        };
        let mut expanded = false;
        loop {
            if self.rec_expand(coin) {
                // Check whether multiple spans expanded so they are now identical.
                let mut test = self.coin_sets[set.0].head;
                while let Some(t) = test {
                    if t != coin
                        && self.rec_coin_start(coin) == self.rec_coin_start(t)
                        && self.rec_opp_start(coin) == self.rec_opp_start(t)
                    {
                        let _ = self.cs_release_rec(set, CoinList::Head, t);
                        break;
                    }
                    test = self.rec_next(t);
                }
                expanded = true;
            }
            match self.rec_next(coin) {
                Some(n) => coin = n,
                None => break,
            }
        }
        expanded
    }

    /// `SkOpCoincidence::findOverlaps(overlaps)`: for each pair of coincident runs that overlap
    /// another, records the overlap in `overlaps`.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1236-L1278 (chrome/m156)
    pub(crate) fn cs_find_overlaps(&mut self, set: CoinSetId, overlaps: CoinSetId) -> bool {
        self.coin_sets[overlaps.0].head = None;
        self.coin_sets[overlaps.0].top = None;
        let mut outer = self.coin_sets[set.0].head;
        while let Some(o) = outer {
            let outer_coin = self.ptt_segment(self.rec_coin_start(o));
            let outer_opp = self.ptt_segment(self.rec_opp_start(o));
            let mut inner = self.rec_next(o);
            while let Some(i) = inner {
                let inner_coin = self.ptt_segment(self.rec_coin_start(i));
                if outer_coin != inner_coin {
                    let inner_opp = self.ptt_segment(self.rec_opp_start(i));
                    let mut found: Option<(PtTId, PtTId)> = None;
                    if outer_opp == inner_coin {
                        let (ok, s, e) = self.ptt_overlaps(
                            self.rec_opp_start(o),
                            self.rec_opp_end(o),
                            self.rec_coin_start(i),
                            self.rec_coin_end(i),
                        );
                        if ok {
                            found = s.zip(e);
                        }
                    }
                    if found.is_none() && outer_coin == inner_opp {
                        let (ok, s, e) = self.ptt_overlaps(
                            self.rec_coin_start(o),
                            self.rec_coin_end(o),
                            self.rec_opp_start(i),
                            self.rec_opp_end(i),
                        );
                        if ok {
                            found = s.zip(e);
                        }
                    }
                    if found.is_none() && outer_opp == inner_opp {
                        let (ok, s, e) = self.ptt_overlaps(
                            self.rec_opp_start(o),
                            self.rec_opp_end(o),
                            self.rec_opp_start(i),
                            self.rec_opp_end(i),
                        );
                        if ok {
                            found = s.zip(e);
                        }
                    }
                    if let Some((over_s, over_e)) = found
                        && !self.cs_add_overlap(
                            overlaps, outer_coin, outer_opp, inner_coin, inner_opp, over_s, over_e,
                        )
                    {
                        return false;
                    }
                }
                inner = self.rec_next(i);
            }
            outer = self.rec_next(o);
        }
        true
    }

    /// `SkOpCoincidence::apply()`: walks the span sets in parallel, moving winding from one to
    /// the other.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1280-L1385 (chrome/m156)
    pub(crate) fn cs_apply(&mut self, set: CoinSetId) -> bool {
        let Some(mut coin) = self.coin_sets[set.0].head else {
            return true;
        };
        loop {
            'body: {
                let start_base = self.ptt_span(self.rec_coin_start(coin));
                if self.span_up_castable(start_base).is_none() {
                    return false;
                }
                let mut start = start_base;
                if self.span_deleted(start) {
                    break 'body;
                }
                let end = self.ptt_span(self.rec_coin_end(coin));
                if start != self.span_starter(start, end) {
                    return false;
                }
                let flipped = self.rec_flipped(coin);
                let o_start_base = if flipped {
                    self.ptt_span(self.rec_opp_end(coin))
                } else {
                    self.ptt_span(self.rec_opp_start(coin))
                };
                if self.span_up_castable(o_start_base).is_none() {
                    return false;
                }
                let mut o_start = o_start_base;
                if self.span_deleted(o_start) {
                    break 'body;
                }
                let o_end = if flipped {
                    self.ptt_span(self.rec_opp_start(coin))
                } else {
                    self.ptt_span(self.rec_opp_end(coin))
                };
                let segment = self.span_segment(start);
                let o_segment = self.span_segment(o_start);
                let operand_swap = self.seg_operand(segment) != self.seg_operand(o_segment);
                if flipped {
                    if self.span_deleted(o_end) {
                        break 'body;
                    }
                    loop {
                        let Some(o_next) = self.span_next(o_start) else {
                            return false;
                        };
                        if o_next == o_end {
                            break;
                        }
                        if self.span_up_castable(o_next).is_none() {
                            return false;
                        }
                        o_start = self.span_up_cast(o_next);
                    }
                }
                loop {
                    let mut wind_value = self.span_wind_value(start);
                    let mut opp_value = self.span_opp_value(start);
                    let mut o_wind_value = self.span_wind_value(o_start);
                    let mut o_opp_value = self.span_opp_value(o_start);
                    // Winding values are added or subtracted depending on direction and wind
                    // type. Same or opposite values are summed depending on the operand value.
                    let mut wind_diff = if operand_swap {
                        o_opp_value
                    } else {
                        o_wind_value
                    };
                    let mut o_wind_diff = if operand_swap { opp_value } else { wind_value };
                    if !flipped {
                        wind_diff = -wind_diff;
                        o_wind_diff = -o_wind_diff;
                    }
                    let mut add_to_start = wind_value != 0
                        && (wind_value > wind_diff
                            || (wind_value == wind_diff && o_wind_value <= o_wind_diff));
                    let done = if add_to_start {
                        self.span_done(start)
                    } else {
                        self.span_done(o_start)
                    };
                    if done {
                        add_to_start = !add_to_start;
                    }
                    if add_to_start {
                        if operand_swap {
                            std::mem::swap(&mut o_wind_value, &mut o_opp_value);
                        }
                        if flipped {
                            wind_value -= o_wind_value;
                            opp_value -= o_opp_value;
                        } else {
                            wind_value += o_wind_value;
                            opp_value += o_opp_value;
                        }
                        if self.seg_is_xor(segment) {
                            wind_value &= 1;
                        }
                        if self.seg_opp_xor(segment) {
                            opp_value &= 1;
                        }
                        o_wind_value = 0;
                        o_opp_value = 0;
                    } else {
                        if operand_swap {
                            std::mem::swap(&mut wind_value, &mut opp_value);
                        }
                        if flipped {
                            o_wind_value -= wind_value;
                            o_opp_value -= opp_value;
                        } else {
                            o_wind_value += wind_value;
                            o_opp_value += opp_value;
                        }
                        if self.seg_is_xor(o_segment) {
                            o_wind_value &= 1;
                        }
                        if self.seg_opp_xor(o_segment) {
                            o_opp_value &= 1;
                        }
                        wind_value = 0;
                        opp_value = 0;
                    }
                    if wind_value <= -1 {
                        return false;
                    }
                    self.span_set_wind_value(start, wind_value);
                    self.span_set_opp_value(start, opp_value);
                    if o_wind_value <= -1 {
                        return false;
                    }
                    self.span_set_wind_value(o_start, o_wind_value);
                    self.span_set_opp_value(o_start, o_opp_value);
                    if wind_value == 0 && opp_value == 0 {
                        self.seg_mark_done(segment, start);
                    }
                    if o_wind_value == 0 && o_opp_value == 0 {
                        self.seg_mark_done(o_segment, o_start);
                    }
                    let next = self.span_next(start);
                    let o_next = if flipped {
                        self.span_prev(o_start)
                    } else {
                        self.span_next(o_start)
                    };
                    if next == Some(end) {
                        break;
                    }
                    let Some(next) = next else {
                        return false;
                    };
                    if self.span_up_castable(next).is_none() {
                        return false;
                    }
                    start = self.span_up_cast(next);
                    // If the opposite ran out too soon, reuse the last span.
                    let o_next = match o_next {
                        Some(n) if self.span_up_castable(n).is_some() => n,
                        _ => o_start,
                    };
                    o_start = self.span_up_cast(o_next);
                }
            }
            match self.rec_next(coin) {
                Some(n) => coin = n,
                None => break,
            }
        }
        true
    }

    /// `SkOpCoincidence::correctEnds()` on `set`.
    // Port of: src/pathops/SkOpCoincidence.cpp#L575-L583 (chrome/m156)
    pub(crate) fn cs_correct_ends(&mut self, set: CoinSetId) {
        let mut cur = self.coin_sets[set.0].head;
        while let Some(c) = cur {
            self.rec_correct_ends(c);
            cur = self.rec_next(c);
        }
    }

    // ----- SkOpGlobalState's coincidence pointer -----

    /// Creates a coincidence object and makes it the global one (`SkOpCoincidence`'s
    /// constructor calls `globalState->setCoincidence(this)`).
    // Port of: src/pathops/SkOpCoincidence.h#L140-L150 (chrome/m156)
    pub(crate) fn coin_new_set(&mut self) -> CoinSetId {
        let id = CoinSetId(self.coin_sets.len());
        self.coin_sets.push(CoinSet::default());
        self.coin_global = Some(id);
        id
    }

    /// `SkOpGlobalState::setCoincidence(coincidence)`: the object the global calls use.
    // Port of: src/pathops/SkPathOpsTypes.h#L155-L157 (chrome/m156)
    pub(crate) fn coin_set_global(&mut self, set: CoinSetId) {
        self.coin_global = Some(set);
    }

    /// `SkOpGlobalState::coincidence()` isEmpty check (`SkOpSpanBase::checkForCollapsedCoincidence`).
    /// A missing coincidence object counts as empty.
    // Port of: src/pathops/SkOpSpan.cpp#L286-L290 (chrome/m156)
    #[must_use]
    pub(crate) fn coincidence_is_empty(&self) -> bool {
        match self.coin_global {
            Some(set) => self.cs_is_empty(set),
            None => true,
        }
    }

    /// `globalState()->coincidence()->contains(...)` with four points.
    // Port of: src/pathops/SkOpCoincidence.h#L158-L159 (chrome/m156)
    #[must_use]
    pub(crate) fn coin_contains(
        &self,
        coin_start: PtTId,
        coin_end: PtTId,
        opp_start: PtTId,
        opp_end: PtTId,
    ) -> bool {
        match self.coin_global {
            Some(set) => self.cs_contains_ptts(set, coin_start, coin_end, opp_start, opp_end),
            None => false,
        }
    }

    /// `globalState()->coincidence()->extend(...)`.
    // Port of: src/pathops/SkOpCoincidence.h#L217-L218 (chrome/m156)
    pub(crate) fn coin_extend(
        &mut self,
        coin_start: PtTId,
        coin_end: PtTId,
        opp_start: PtTId,
        opp_end: PtTId,
    ) -> bool {
        match self.coin_global {
            Some(set) => self.cs_extend(set, coin_start, coin_end, opp_start, opp_end),
            None => false,
        }
    }

    /// `globalState()->coincidence()->add(...)`.
    // Port of: src/pathops/SkOpCoincidence.cpp#L208 (chrome/m156)
    pub(crate) fn coin_add(
        &mut self,
        coin_start: PtTId,
        coin_end: PtTId,
        opp_start: PtTId,
        opp_end: PtTId,
    ) {
        if let Some(set) = self.coin_global {
            self.cs_add(set, coin_start, coin_end, opp_start, opp_end);
        }
    }

    /// `globalState()->coincidence()->release(seg)`.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1074-L1077 (chrome/m156)
    pub(crate) fn coin_release_seg(&mut self, seg: SegId) {
        if let Some(set) = self.coin_global {
            self.cs_release_seg_all(set, seg);
        }
    }

    /// `globalState()->coincidence()->markCollapsed(test)`.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1157-L1164 (chrome/m156)
    pub(crate) fn coin_mark_collapsed(&mut self, test: PtTId) {
        if let Some(set) = self.coin_global {
            self.cs_mark_collapsed(set, test);
        }
    }

    /// `globalState()->coincidence()->releaseDeleted()`.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1042-L1044 (chrome/m156)
    pub(crate) fn coin_release_deleted(&mut self) {
        if let Some(set) = self.coin_global {
            self.cs_release_deleted(set);
        }
    }

    /// `globalState()->coincidence()->fixUp(deleted, kept)`, if there is a coincidence object.
    // Port of: src/pathops/SkOpCoincidence.cpp#L1158-L1164 (chrome/m156)
    pub(crate) fn coin_fix_up(&mut self, deleted: PtTId, kept: PtTId) {
        if let Some(set) = self.coin_global {
            self.cs_fix_up(set, deleted, kept);
        }
    }
}

/// The `i`-th float of a point array in memory order (`&pts[0].fX`, then `fY`, then the next
/// point). `SkOpCoincidence::Ordered` compares those floats.
// Port of: src/pathops/SkOpCoincidence.cpp#L1396-L1400 (chrome/m156)
fn float_at(pts: &[Point; 4], index: usize) -> f32 {
    let p = pts[index / 2];
    if index.is_multiple_of(2) { p.x } else { p.y }
}

impl OpState {
    /// `SkOpCoincidence::ptT->contains(check)` with an optional `check`: a null pointer is never
    /// contained (Skia's `SkOpPtT::contains` returns false for it).
    // Port of: src/pathops/SkOpSpan.cpp#L16-L23 (chrome/m156)
    fn ptt_contains_opt(&self, ptt: PtTId, check: Option<PtTId>) -> bool {
        check.is_some_and(|c| self.ptt_contains_ptt(ptt, c))
    }
}
