// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkOpAngle.h, src/pathops/SkOpAngle.cpp

//! The angles leaving a span, sorted counterclockwise (`SkOpAngle`).
//!
//! At an intersection several curves leave the same point. Each leaving curve gets an
//! [`Angle`]; the angles at a point form a circular list ordered by [`OpState::angle_after`]
//! and friends, which is how the walk chooses the next edge.

use skia_rust_core::path::Verb;
use skia_rust_core::t_sort::t_q_sort;

use crate::intersections::Intersections;
use crate::line::DLine;
use crate::line_parameters::LineParameters;
use crate::op_curve::{
    DCurveBuf, DCurveSweep, curve_d_point_at_t, curve_d_slope_at_t, curve_intersect_ray, verb_points,
};
use crate::op_state::{AngleId, OpState, SegId, SpanId};
use crate::point::{DPoint, DVector};
use crate::types::{
    almost_bequal_ulps, approximately_between_orderable, approximately_equal, approximately_equal_orderable,
    approximately_zero, between, std_max, std_min,
};

/// `SkOpAngle::IncludeType`.
// Port of: src/pathops/SkOpAngle.h#L34-L40 (chrome/m156)
#[doc(alias = "SkOpAngle::IncludeType")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum IncludeType {
    UnaryWinding,
    UnaryXor,
    BinarySingle,
    BinaryOpp,
}

/// `SkOpAngle`: the curve leaving a span from `start` towards `end`, with the sector and
/// order data used to sort it among the other angles at the same point.
// Port of: src/pathops/SkOpAngle.h#L32-L110 (chrome/m156)
#[doc(alias = "SkOpAngle")]
#[derive(Clone, Debug)]
pub(crate) struct Angle {
    /// `SkDCurve fOriginalCurvePart`: the curve from start to end.
    pub(crate) original_curve_part: DCurveBuf,
    /// `SkDCurveSweep fPart`: the curve from start to end, offset as needed.
    pub(crate) part: DCurveSweep,
    /// `double fSide`.
    pub(crate) side: f64,
    /// `SkLineParameters fTangentHalf`: used only to sort lines and line-like sections.
    pub(crate) tangent_half: LineParameters,
    /// `SkOpAngle* fNext`.
    pub(crate) next: Option<AngleId>,
    /// `SkOpSpanBase* fLastMarked`.
    pub(crate) last_marked: Option<SpanId>,
    /// `SkOpSpanBase* fStart`.
    pub(crate) start: SpanId,
    /// `SkOpSpanBase* fEnd`.
    pub(crate) end: SpanId,
    /// `SkOpSpanBase* fComputedEnd`.
    pub(crate) computed_end: SpanId,
    /// `int fSectorMask`.
    pub(crate) sector_mask: u32,
    /// `int8_t fSectorStart`: in 32nds of a circle.
    pub(crate) sector_start: i8,
    /// `int8_t fSectorEnd`.
    pub(crate) sector_end: i8,
    /// `bool fUnorderable`.
    pub(crate) unorderable: bool,
    /// `bool fComputeSector`.
    pub(crate) compute_sector: bool,
    /// `bool fComputedSector`.
    pub(crate) computed_sector: bool,
    /// `bool fCheckCoincidence`.
    pub(crate) check_coincidence: bool,
    /// `bool fTangentsAmbiguous`.
    pub(crate) tangents_ambiguous: bool,
}

impl Default for Angle {
    fn default() -> Self {
        Self {
            original_curve_part: DCurveBuf::default(),
            part: DCurveSweep::default(),
            side: 0.0,
            tangent_half: LineParameters::default(),
            next: None,
            last_marked: None,
            start: SpanId(usize::MAX),
            end: SpanId(usize::MAX),
            computed_end: SpanId(usize::MAX),
            sector_mask: 0,
            sector_start: 0,
            sector_end: 0,
            unorderable: false,
            compute_sector: false,
            computed_sector: false,
            check_coincidence: false,
            tangents_ambiguous: false,
        }
    }
}

/// `SkOpAngle::findSector`'s table: `sedecimant[xy sign][y sign][x sign]`.
// Port of: src/pathops/SkOpAngle.cpp#L (SkOpAngle::findSector) (chrome/m156)
const SEDECIMANT: [[[i32; 3]; 3]; 3] = [
    [[4, 3, 2], [7, -1, 15], [10, 11, 12]],  // abs(x) <  abs(y)
    [[5, -1, 1], [-1, -1, -1], [9, -1, 13]], // abs(x) == abs(y)
    [[6, 3, 0], [7, -1, 15], [8, 11, 14]],   // abs(x) >  abs(y)
];

/// `SkOpAngle::lineOnOneSide`, the "origin" form.
// Port of: src/pathops/SkOpAngle.cpp#L (SkOpAngle::lineOnOneSide) (chrome/m156)
fn line_on_one_side(origin: DPoint, line: DVector, test: &Angle, test_verb: Verb, use_original: bool) -> i32 {
    let mut crosses = [0.0_f64; 3];
    let i_max = verb_points(test_verb);
    let test_curve = if use_original {
        test.original_curve_part
    } else {
        test.part.curve
    };
    for index in 1..=i_max {
        let xy1 = line.x * (test_curve.at(index).y - origin.y);
        let xy2 = line.y * (test_curve.at(index).x - origin.x);
        crosses[index - 1] = if almost_bequal_ulps(xy1, xy2) { 0.0 } else { xy1 - xy2 };
    }
    if crosses[0] * crosses[1] < 0.0 {
        return -1;
    }
    if test_verb == Verb::Cubic && (crosses[0] * crosses[2] < 0.0 || crosses[1] * crosses[2] < 0.0) {
        return -1;
    }
    if crosses[0] != 0.0 {
        return i32::from(crosses[0] < 0.0);
    }
    if crosses[1] != 0.0 {
        return i32::from(crosses[1] < 0.0);
    }
    if test_verb == Verb::Cubic && crosses[2] != 0.0 {
        return i32::from(crosses[2] < 0.0);
    }
    -2
}

impl OpState {
    /// `SkOpAngle::start()`.
    #[must_use]
    pub(crate) fn angle_start(&self, a: AngleId) -> SpanId {
        self.angles[a.0].start
    }

    /// `SkOpAngle::end()`.
    #[must_use]
    pub(crate) fn angle_end(&self, a: AngleId) -> SpanId {
        self.angles[a.0].end
    }

    /// `SkOpAngle::next()`.
    #[must_use]
    pub(crate) fn angle_next(&self, a: AngleId) -> Option<AngleId> {
        self.angles[a.0].next
    }

    /// `SkOpAngle::unorderable()`.
    #[must_use]
    pub(crate) fn angle_unorderable(&self, a: AngleId) -> bool {
        self.angles[a.0].unorderable
    }

    /// `SkOpAngle::tangentsAmbiguous()`.
    #[must_use]
    pub(crate) fn angle_tangents_ambiguous(&self, a: AngleId) -> bool {
        self.angles[a.0].tangents_ambiguous
    }

    /// `SkOpAngle::segment()`: the segment of the start span.
    #[must_use]
    pub(crate) fn angle_segment(&self, a: AngleId) -> SegId {
        self.span_segment(self.angles[a.0].start)
    }

    /// `SkOpAngle::starter()`: `fStart->starter(fEnd)`.
    #[must_use]
    pub(crate) fn angle_starter(&self, a: AngleId) -> SpanId {
        self.span_starter(self.angles[a.0].start, self.angles[a.0].end)
    }

    /// `SkOpAngle::setLastMarked(marked)`.
    pub(crate) fn angle_set_last_marked(&mut self, a: AngleId, marked: Option<SpanId>) {
        self.angles[a.0].last_marked = marked;
    }

    /// `SkOpAngle::lastMarked()`: a marked span that was already chased yields nothing;
    /// otherwise it is marked chased.
    // Port of: src/pathops/SkOpAngle.cpp (SkOpAngle::lastMarked) (chrome/m156)
    pub(crate) fn angle_last_marked(&mut self, a: AngleId) -> Option<SpanId> {
        let marked = self.angles[a.0].last_marked?;
        if self.span_chased(marked) {
            return None;
        }
        self.span_set_chased(marked, true);
        Some(marked)
    }

    /// `SkOpAngle::set(start, end)`.
    // Port of: src/pathops/SkOpAngle.cpp#L (SkOpAngle::set) (chrome/m156)
    pub(crate) fn angle_set(&mut self, a: AngleId, start: SpanId, end: SpanId) {
        let angle = &mut self.angles[a.0];
        angle.start = start;
        angle.computed_end = end;
        angle.end = end;
        angle.next = None;
        angle.compute_sector = false;
        angle.computed_sector = false;
        angle.check_coincidence = false;
        angle.tangents_ambiguous = false;
        self.angle_set_spans(a);
        self.angle_set_sector(a);
    }

    /// `SkOpAngle::setSpans()`.
    // Port of: src/pathops/SkOpAngle.cpp (SkOpAngle::setSpans) (chrome/m156)
    fn angle_set_spans(&mut self, a: AngleId) {
        {
            let angle = &mut self.angles[a.0];
            angle.unorderable = false;
            angle.last_marked = None;
        }
        let start = self.angles[a.0].start;
        let end = self.angles[a.0].end;
        let segment = self.span_segment(start);
        let verb = self.seg_verb(segment);
        let pts = self.seg_pts(segment);
        let weight = self.seg_weight(segment);
        let mut edge = DCurveBuf::default();
        self.seg_sub_divide(segment, start, end, &mut edge);
        self.angles[a.0].part.curve = edge;
        self.angles[a.0].original_curve_part = edge;
        {
            let angle = &mut self.angles[a.0];
            angle.part.set_curve_hull_sweep(verb);
        }
        if verb != Verb::Line && !self.angles[a.0].part.is_curve {
            let curve = self.angles[a.0].part.curve;
            self.angles[a.0].part.curve.pts[1] = curve.pts[verb_points(verb)];
            let p1 = self.angles[a.0].part.curve.pts[1];
            self.angles[a.0].original_curve_part.pts[1] = p1;
            // lineHalf[i].set(fPart.fCurve[i].asSkPoint()): the points go through SkPoint (float).
            let line_half = DLine::new([
                DPoint::from_sk_point(self.angles[a.0].part.curve.pts[0].as_sk_point()),
                DPoint::from_sk_point(p1.as_sk_point()),
            ]);
            self.angles[a.0].tangent_half.line_end_points(&line_half);
            self.angles[a.0].side = 0.0;
        }
        match verb {
            Verb::Line => {
                let c_p1 = pts[usize::from(self.span_t(start) < self.span_t(end))];
                let line_half = DLine::new([
                    DPoint::from_sk_point(self.span_pt(start)),
                    DPoint::from_sk_point(c_p1),
                ]);
                let angle = &mut self.angles[a.0];
                angle.tangent_half.line_end_points(&line_half);
                angle.side = 0.0;
            }
            Verb::Quad | Verb::Conic => {
                let mut tangent_part = LineParameters::default();
                let quad = self.angles[a.0].part.curve.quad();
                let _ = tangent_part.quad_end_points_full(&quad);
                let pt2 = self.angles[a.0].part.curve.pts[2];
                self.angles[a.0].side = -tangent_part.point_distance(pt2);
            }
            Verb::Cubic => {
                let mut tangent_part = LineParameters::default();
                let cubic = self.angles[a.0].part.curve.cubic();
                tangent_part.cubic_part(&cubic);
                let pt3 = self.angles[a.0].part.curve.pts[3];
                self.angles[a.0].side = -tangent_part.point_distance(pt3);
                let mut test_ts = [0.0_f64; 4];
                let mut test_count = {
                    let mut t_values = [0.0_f64; 2];
                    let n = crate::cubic::DCubic::find_inflections_sk(pts, &mut t_values);
                    test_ts[..n].copy_from_slice(&t_values[..n]);
                    n
                };
                let start_t = self.span_t(start);
                let end_t = self.span_t(end);
                let limit_t = end_t;
                for test_t in test_ts.iter_mut().take(test_count) {
                    if !between(start_t, *test_t, limit_t) {
                        *test_t = -1.0;
                    }
                }
                test_ts[test_count] = start_t;
                test_count += 1;
                test_ts[test_count] = end_t;
                test_count += 1;
                t_q_sort(&mut test_ts[..test_count], |a, b| a < b);
                let mut best_side = 0.0_f64;
                let test_cases = (test_count << 1) - 1;
                let mut index = 0;
                while test_ts[index] < 0.0 {
                    index += 1;
                }
                index <<= 1;
                while index < test_cases {
                    let test_index = index >> 1;
                    let mut test_t = test_ts[test_index];
                    if index & 1 != 0 {
                        test_t = (test_t + test_ts[test_index + 1]) / 2.0;
                    }
                    let pt = curve_d_point_at_t(Verb::Cubic, &pts, weight, test_t);
                    let mut test_part = LineParameters::default();
                    test_part.cubic_end_points_full(&cubic);
                    let test_side = test_part.point_distance(pt);
                    if best_side.abs() < test_side.abs() {
                        best_side = test_side;
                    }
                    index += 1;
                }
                self.angles[a.0].side = -best_side;
            }
            Verb::Move | Verb::Close | Verb::Done => {}
        }
    }

    /// `SkOpAngle::findSector(verb, x, y)`.
    // Port of: src/pathops/SkOpAngle.cpp (SkOpAngle::findSector) (chrome/m156)
    #[must_use]
    fn angle_find_sector(verb: Verb, x: f64, y: f64) -> i32 {
        let abs_x = x.abs();
        let abs_y = y.abs();
        let xy = if verb == Verb::Line || !crate::types::almost_equal_ulps(abs_x, abs_y) {
            abs_x - abs_y
        } else {
            0.0
        };
        let sector = SEDECIMANT[usize::from(xy >= 0.0) + usize::from(xy > 0.0)]
            [usize::from(y >= 0.0) + usize::from(y > 0.0)]
            [usize::from(x >= 0.0) + usize::from(x > 0.0)];
        sector * 2 + 1
    }

    /// `SkOpAngle::checkCrossesZero()`.
    // Port of: src/pathops/SkOpAngle.cpp (SkOpAngle::checkCrossesZero) (chrome/m156)
    #[must_use]
    fn angle_check_crosses_zero(&self, a: AngleId) -> bool {
        let angle = &self.angles[a.0];
        let start = std_min(angle.sector_start, angle.sector_end);
        let end = std_max(angle.sector_start, angle.sector_end);
        end - start > 16
    }

    /// `SkOpAngle::setSector()`.
    // Port of: src/pathops/SkOpAngle.cpp (SkOpAngle::setSector) (chrome/m156)
    fn angle_set_sector(&mut self, a: AngleId) {
        let segment = self.span_segment(self.angles[a.0].start);
        let verb = self.seg_verb(segment);
        let sweep0 = self.angles[a.0].part.sweep[0];
        let sweep1 = self.angles[a.0].part.sweep[1];
        let sector_start = Self::angle_find_sector(verb, sweep0.x, sweep0.y);
        if sector_start < 0 {
            return self.angle_defer_sector(a);
        }
        self.angles[a.0].sector_start = sector_start as i8;
        if !self.angles[a.0].part.is_curve {
            // if it's a line or line-like, note that both sectors are the same
            self.angles[a.0].sector_end = sector_start as i8;
            self.angles[a.0].sector_mask = 1u32 << sector_start;
            return;
        }
        let sector_end = Self::angle_find_sector(verb, sweep1.x, sweep1.y);
        if sector_end < 0 {
            return self.angle_defer_sector(a);
        }
        self.angles[a.0].sector_end = sector_end as i8;
        let mut s_start = i32::from(self.angles[a.0].sector_start);
        let mut s_end = i32::from(self.angles[a.0].sector_end);
        if s_end == s_start && (s_start & 3) != 3 {
            // if the sector has no span, it can't be an exact angle
            self.angles[a.0].sector_mask = 1u32 << s_start;
            return;
        }
        let mut crosses_zero = self.angle_check_crosses_zero(a);
        let start = std_min(s_start, s_end);
        let curve_bends_ccw = (s_start == start) ^ crosses_zero;
        if (s_start & 3) == 3 {
            s_start = (s_start + if curve_bends_ccw { 1 } else { 31 }) & 0x1f;
        }
        if (s_end & 3) == 3 {
            s_end = (s_end + if curve_bends_ccw { 31 } else { 1 }) & 0x1f;
        }
        self.angles[a.0].sector_start = s_start as i8;
        self.angles[a.0].sector_end = s_end as i8;
        crosses_zero = self.angle_check_crosses_zero(a);
        let start = std_min(s_start, s_end);
        let end = std_max(s_start, s_end);
        let mask = if !crosses_zero {
            (u32::MAX >> (31 - end + start)) << start
        } else {
            (u32::MAX >> (31 - start)) | (u32::MAX << end)
        };
        self.angles[a.0].sector_mask = mask;
    }

    /// The `deferTilLater` label of `setSector`.
    fn angle_defer_sector(&mut self, a: AngleId) {
        let angle = &mut self.angles[a.0];
        angle.sector_start = -1;
        angle.sector_end = -1;
        angle.sector_mask = 0;
        angle.compute_sector = true; // can't determine sector until segment length can be found
    }

    /// `SkOpAngle::computeSector()`.
    // Port of: src/pathops/SkOpAngle.cpp (SkOpAngle::computeSector) (chrome/m156)
    pub(crate) fn angle_compute_sector(&mut self, a: AngleId) -> bool {
        if self.angles[a.0].computed_sector {
            return !self.angles[a.0].unorderable;
        }
        self.angles[a.0].computed_sector = true;
        let start = self.angles[a.0].start;
        let end = self.angles[a.0].end;
        let step_up = self.span_t(start) < self.span_t(end);
        let mut check_end = Some(end);
        if self.span_final(end) && step_up {
            self.angles[a.0].unorderable = true;
            return false;
        }
        let segment = self.span_segment(end);
        let mut recompute = false;
        while let Some(ce) = check_end {
            let other = self.span_segment(ce);
            let mut o_span = self.seg_head(other);
            loop {
                if self.span_segment(o_span) == segment
                    && o_span != ce
                    && approximately_equal(self.span_t(o_span), self.span_t(ce))
                {
                    recompute = true;
                    break;
                }
                if self.span_final(o_span) {
                    break;
                }
                match self.span_next(o_span) {
                    Some(next) => o_span = next,
                    None => break,
                }
            }
            if recompute {
                break;
            }
            check_end = if step_up {
                if !self.span_final(ce) {
                    self.span_next(ce)
                } else {
                    None
                }
            } else {
                self.span_prev(ce)
            };
        }
        // recomputeSector:
        let computed_end = if step_up {
            match check_end {
                Some(ce) => self.span_prev(ce).expect("span has prev"),
                None => self.seg_head(self.span_segment(end)),
            }
        } else {
            match check_end {
                Some(ce) => self.span_next(ce).expect("span has next"),
                None => self.seg_tail(self.span_segment(end)),
            }
        };
        if check_end == Some(end) || computed_end == end || computed_end == start {
            self.angles[a.0].unorderable = true;
            return false;
        }
        if step_up != (self.span_t(start) < self.span_t(computed_end)) {
            self.angles[a.0].unorderable = true;
            return false;
        }
        let save_end = end;
        self.angles[a.0].computed_end = computed_end;
        self.angles[a.0].end = computed_end;
        self.angle_set_spans(a);
        self.angle_set_sector(a);
        self.angles[a.0].end = save_end;
        !self.angles[a.0].unorderable
    }

    /// `SkOpAngle::after(test)`: whether this angle sorts after `test`.
    // Port of: src/pathops/SkOpAngle.cpp#L75-L240 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the single C++ function, so it is kept whole
    pub(crate) fn angle_after(&mut self, this: AngleId, test: AngleId) -> bool {
        let lh = test;
        let rh = self.angles[lh.0].next.expect("angle has next");
        let orig_this = self.angles[this.0].original_curve_part;
        self.angles[this.0].part.curve = orig_this;
        let orig_lh = self.angles[lh.0].original_curve_part;
        self.angles[lh.0].part.curve = orig_lh;
        let this0 = self.angles[this.0].part.curve.pts[0];
        self.angles[lh.0].part.curve.pts[0] = this0;
        let orig_rh = self.angles[rh.0].original_curve_part;
        self.angles[rh.0].part.curve = orig_rh;
        self.angles[rh.0].part.curve.pts[0] = this0;

        if self.angles[lh.0].compute_sector && !self.angle_compute_sector(lh) {
            return true;
        }
        if self.angles[this.0].compute_sector && !self.angle_compute_sector(this) {
            return true;
        }
        if self.angles[rh.0].compute_sector && !self.angle_compute_sector(rh) {
            return true;
        }
        let lh_mask = self.angles[lh.0].sector_mask;
        let rh_mask = self.angles[rh.0].sector_mask;
        let this_mask = self.angles[this.0].sector_mask;
        let ltr_overlap = ((lh_mask | rh_mask) & this_mask) != 0;
        let lr_overlap = (lh_mask & rh_mask) != 0;
        let lh_start = i32::from(self.angles[lh.0].sector_start);
        let lh_end = i32::from(self.angles[lh.0].sector_end);
        let rh_start = i32::from(self.angles[rh.0].sector_start);
        let rh_end = i32::from(self.angles[rh.0].sector_end);
        let this_start = i32::from(self.angles[this.0].sector_start);
        let this_end = i32::from(self.angles[this.0].sector_end);
        let lr_order: i32;
        if !lr_overlap {
            // no lh/rh sector overlap
            if !ltr_overlap {
                // no lh/this/rh sector overlap
                return (lh_end > rh_start) ^ (this_start > lh_end) ^ (this_start > rh_start);
            }
            let lr_gap = (rh_start - lh_start + 32) & 0x1f;
            /* A tiny change can move the start +/- 4. The order can only be determined if
               lr gap is not 12 to 20 or -12 to -20. */
            lr_order = if lr_gap > 20 {
                0
            } else if lr_gap > 11 {
                -1
            } else {
                1
            };
        } else {
            lr_order = self.angle_orderable(lh, rh);
            if !ltr_overlap && lr_order >= 0 {
                return lr_order == 0;
            }
        }
        let mut lt_order: i32 = if lh_mask & this_mask != 0 {
            self.angle_orderable(lh, this)
        } else {
            let lt_gap = (this_start - lh_start + 32) & 0x1f;
            if lt_gap > 20 {
                0
            } else if lt_gap > 11 {
                -1
            } else {
                1
            }
        };
        let mut tr_order: i32 = if rh_mask & this_mask != 0 {
            self.angle_orderable(this, rh)
        } else {
            let tr_gap = (rh_start - this_start + 32) & 0x1f;
            if tr_gap > 20 {
                0
            } else if tr_gap > 11 {
                -1
            } else {
                1
            }
        };
        self.angle_alignment_same_side(this, lh, &mut lt_order);
        self.angle_alignment_same_side(this, rh, &mut tr_order);
        if lr_order >= 0 && lt_order >= 0 && tr_order >= 0 {
            return if lr_order != 0 {
                (lt_order & tr_order) != 0
            } else {
                (lt_order | tr_order) != 0
            };
        }
        if lt_order == 0 && lr_order == 0 {
            return self.angle_opposite_planes(lh, this);
        } else if lt_order == 1 && tr_order == 0 {
            return self.angle_opposite_planes(this, rh);
        } else if lr_order == 1 && tr_order == 1 {
            return self.angle_opposite_planes(lh, rh);
        }
        if self.angles[this.0].unorderable
            || self.angles[lh.0].unorderable
            || self.angles[rh.0].unorderable
        {
            let this_is_curve = self.angles[this.0].part.is_curve;
            let lh_is_curve = self.angles[lh.0].part.is_curve;
            let rh_is_curve = self.angles[rh.0].part.is_curve;
            if !this_is_curve && !lh_is_curve && !rh_is_curve {
                let o_this = self.angles[this.0].original_curve_part.pts[0];
                let o_lh = self.angles[lh.0].original_curve_part.pts[0];
                let o_rh = self.angles[rh.0].original_curve_part.pts[0];
                let lt_share = i32::from(o_lh == o_this);
                let lr_share = i32::from(o_lh == o_rh);
                let tr_share = i32::from(o_this == o_rh);
                if lt_share + lr_share + tr_share == 1 {
                    if lr_share != 0 {
                        let lt_o_order = self.lines_on_original_side(lh, this);
                        let rt_o_order = self.lines_on_original_side(rh, this);
                        if (rt_o_order ^ lt_o_order) == 1 {
                            return lt_o_order != 0;
                        }
                    } else if tr_share != 0 {
                        let tl_o_order = self.lines_on_original_side(this, lh);
                        let rl_o_order = self.lines_on_original_side(rh, lh);
                        if (tl_o_order ^ rl_o_order) == 1 {
                            return rl_o_order != 0;
                        }
                    } else {
                        let tr_o_order = self.lines_on_original_side(rh, this);
                        let lr_o_order = self.lines_on_original_side(lh, rh);
                        if (lr_o_order ^ tr_o_order) == 1 {
                            return tr_o_order != 0;
                        }
                    }
                }
            }
        }
        if lr_order < 0 {
            if lt_order < 0 {
                return tr_order != 0;
            }
            return lt_order != 0;
        }
        lr_order == 0
    }

    /// `SkOpAngle::alignmentSameSide(test, order)`.
    // Port of: src/pathops/SkOpAngle.cpp#L313-L346 (chrome/m156)
    fn angle_alignment_same_side(&mut self, this: AngleId, test: AngleId, order: &mut i32) {
        if *order < 0 {
            return;
        }
        if self.angles[this.0].part.is_curve {
            // This should support all curve types, but only bug that requires this has lines.
            return;
        }
        if self.angles[test.0].part.is_curve {
            return;
        }
        let x_origin = self.angles[test.0].part.curve.pts[0];
        let o_origin = self.angles[test.0].original_curve_part.pts[0];
        if x_origin == o_origin {
            return;
        }
        let verb = self.angle_segment_verb(this);
        let i_max = verb_points(verb);
        let x_line = self.angles[test.0].part.curve.pts[1] - x_origin;
        let o_line = self.angles[test.0].original_curve_part.pts[1] - o_origin;
        for index in 1..=i_max {
            let test_pt = self.angles[this.0].part.curve.pts[index];
            let x_cross = o_line.cross_check(test_pt - x_origin);
            let o_cross = x_line.cross_check(test_pt - o_origin);
            if o_cross * x_cross < 0.0 {
                *order ^= 1;
                break;
            }
        }
    }

    /// `SkOpAngle::segment()->verb()`.
    #[must_use]
    fn angle_segment_verb(&self, a: AngleId) -> Verb {
        self.seg_verb(self.angle_segment(a))
    }

    /// `SkOpAngle::checkParallel(rh)`.
    // Port of: src/pathops/SkOpAngle.cpp#L (SkOpAngle::checkParallel) (chrome/m156)
    fn angle_check_parallel(&mut self, this: AngleId, rh: AngleId) -> bool {
        let sweep = if self.angles[this.0].part.ordered {
            self.angles[this.0].part.sweep
        } else {
            let c = self.angles[this.0].part.curve;
            [c.pts[1] - c.pts[0], DVector::default()]
        };
        let tweep = if self.angles[rh.0].part.ordered {
            self.angles[rh.0].part.sweep
        } else {
            let c = self.angles[rh.0].part.curve;
            [c.pts[1] - c.pts[0], DVector::default()]
        };
        let s0xt0 = sweep[0].cross_check(tweep[0]);
        if self.angle_tangents_diverge(this, rh, s0xt0) {
            return s0xt0 < 0.0;
        }
        if !self.span_contains_span(self.angles[this.0].end, self.angles[rh.0].end) {
            if let Some(ins) = self.angle_end_to_side(this, rh) {
                return ins;
            }
            if let Some(ins) = self.angle_end_to_side(rh, this) {
                return !ins;
            }
        }
        if let Some(ins) = self.angle_mid_to_side(this, rh) {
            return ins;
        }
        if let Some(ins) = self.angle_mid_to_side(rh, this) {
            return !ins;
        }
        let m0 = self.seg_d_pt_at_t(self.angle_segment(this), self.angle_mid_t(this))
            - self.angles[this.0].part.curve.pts[0];
        let m1 = self.seg_d_pt_at_t(self.angle_segment(rh), self.angle_mid_t(rh))
            - self.angles[rh.0].part.curve.pts[0];
        let m0xm1 = m0.cross_check(m1);
        if m0xm1 == 0.0 {
            self.angles[this.0].unorderable = true;
            self.angles[rh.0].unorderable = true;
            return true;
        }
        m0xm1 < 0.0
    }

    /// `SkOpAngle::midT()`.
    #[must_use]
    fn angle_mid_t(&self, a: AngleId) -> f64 {
        (self.span_t(self.angles[a.0].start) + self.span_t(self.angles[a.0].end)) / 2.0
    }

    /// `SkOpAngle::tangentsDiverge(rh, s0xt0)`.
    // Port of: src/pathops/SkOpAngle.cpp (SkOpAngle::tangentsDiverge) (chrome/m156)
    fn angle_tangents_diverge(&mut self, this: AngleId, rh: AngleId, s0xt0: f64) -> bool {
        if s0xt0 == 0.0 {
            return false;
        }
        let sweep = self.angles[this.0].part.sweep;
        let tweep = self.angles[rh.0].part.sweep;
        let s0dt0 = sweep[0].dot(tweep[0]);
        if s0dt0 == 0.0 {
            return true;
        }
        let m = s0xt0 / s0dt0;
        let s_dist = sweep[0].length() * m;
        let t_dist = tweep[0].length() * m;
        let use_s = s_dist.abs() < t_dist.abs();
        let m_factor = if use_s {
            self.angle_dist_end_ratio(this, s_dist)
        } else {
            self.angle_dist_end_ratio(rh, t_dist)
        }
        .abs();
        self.angles[this.0].tangents_ambiguous = m_factor >= 50.0 && m_factor < 200.0;
        m_factor < 50.0 // empirically found limit
    }

    /// `SkOpAngle::distEndRatio(dist)`.
    // Port of: src/pathops/SkOpAngle.cpp (SkOpAngle::distEndRatio) (chrome/m156)
    #[must_use]
    fn angle_dist_end_ratio(&self, a: AngleId, dist: f64) -> f64 {
        let mut longest = 0.0_f64;
        let segment = self.angle_segment(a);
        let pt_count = verb_points(self.seg_verb(segment));
        let pts = self.seg_pts(segment);
        for idx1 in 0..pt_count {
            for idx2 in (idx1 + 1)..=pt_count {
                let v = DVector::new(
                    f64::from(pts[idx2].x - pts[idx1].x),
                    f64::from(pts[idx2].y - pts[idx1].y),
                );
                let len_sq = v.length_squared();
                longest = std_max(longest, len_sq);
            }
        }
        longest.sqrt() / dist
    }

    /// `SkOpAngle::endToSide(rh, inside)`: `Some(inside)` when the answer is known.
    // Port of: src/pathops/SkOpAngle.cpp (SkOpAngle::endToSide) (chrome/m156)
    fn angle_end_to_side(&mut self, this: AngleId, rh: AngleId) -> Option<bool> {
        let segment = self.angle_segment(this);
        let verb = self.seg_verb(segment);
        let pts = self.seg_pts(segment);
        let weight = self.seg_weight(segment);
        let end_t = self.span_t(self.angles[this.0].end);
        let end_pt = self.span_ptt(self.angles[this.0].end);
        let end_point = DPoint::from_sk_point(self.ptts[end_pt.0].pt);
        let mut ray_end = [end_point, end_point];
        let slope_at_end = curve_d_slope_at_t(verb, &pts, weight, end_t);
        ray_end[1].x += slope_at_end.y;
        ray_end[1].y -= slope_at_end.x;
        let mut i_end = Intersections::default();
        let opp_segment = self.angle_segment(rh);
        let opp_verb = self.seg_verb(opp_segment);
        let opp_pts = self.seg_pts(opp_segment);
        let opp_weight = self.seg_weight(opp_segment);
        let ray = DLine::new(ray_end);
        curve_intersect_ray(opp_verb, &opp_pts, opp_weight, &ray, &mut i_end);
        let rh_start_t = self.span_t(self.angles[rh.0].start);
        let rh_end_t = self.span_t(self.angles[rh.0].end);
        let (closest_end, end_dist) = i_end.closest_to(rh_start_t, rh_end_t, ray_end[0]);
        if closest_end < 0 {
            return None;
        }
        if end_dist == 0.0 {
            return None;
        }
        let start_pt = DPoint::from_sk_point(self.ptts[self.span_ptt(self.angles[this.0].start).0].pt);
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let curve = self.angles[rh.0].part.curve;
        let opp_pts_count = verb_points(opp_verb);
        for idx2 in 0..=opp_pts_count {
            min_x = std_min(min_x, curve.at(idx2).x);
            min_y = std_min(min_y, curve.at(idx2).y);
            max_x = std_max(max_x, curve.at(idx2).x);
            max_y = std_max(max_y, curve.at(idx2).y);
        }
        let max_width = std_max(max_x - min_x, max_y - min_y);
        let end_dist = crate::op_angle::ieee_divide(end_dist, max_width);
        if !(end_dist >= 5e-12) {
            // empirically found; ! above catches NaN
            return None;
        }
        let end_pt_d = ray_end[0];
        let opp_pt = i_end.pt(closest_end as usize);
        let v_left = end_pt_d - start_pt;
        let v_right = opp_pt - start_pt;
        let dir = v_left.cross_no_normal_check(v_right);
        if dir == 0.0 {
            return None;
        }
        Some(dir < 0.0)
    }

    /// `SkOpAngle::midToSide(rh, inside)`.
    // Port of: src/pathops/SkOpAngle.cpp (SkOpAngle::midToSide) (chrome/m156)
    fn angle_mid_to_side(&mut self, this: AngleId, rh: AngleId) -> Option<bool> {
        let segment = self.angle_segment(this);
        let verb = self.seg_verb(segment);
        let pts = self.seg_pts(segment);
        let weight = self.seg_weight(segment);
        let start_pt = self.span_pt(self.angles[this.0].start);
        let end_pt = self.span_pt(self.angles[this.0].end);
        let d_start_pt = DPoint::from_sk_point(start_pt);
        let mid_x = (start_pt.x + end_pt.x) / 2.0;
        let mid_y = (start_pt.y + end_pt.y) / 2.0;
        let ray_mid = DLine::new([
            DPoint::new(f64::from(mid_x), f64::from(mid_y)),
            DPoint::new(
                f64::from(mid_x) + f64::from(end_pt.y - start_pt.y),
                f64::from(mid_y) - f64::from(end_pt.x - start_pt.x),
            ),
        ]);
        let mut i_mid = Intersections::default();
        curve_intersect_ray(verb, &pts, weight, &ray_mid, &mut i_mid);
        let i_outside = i_mid.most_outside(
            self.span_t(self.angles[this.0].start),
            self.span_t(self.angles[this.0].end),
            d_start_pt,
        );
        if i_outside < 0 {
            return None;
        }
        let opp_segment = self.angle_segment(rh);
        let opp_verb = self.seg_verb(opp_segment);
        let opp_pts = self.seg_pts(opp_segment);
        let opp_weight = self.seg_weight(opp_segment);
        let mut opp_mid = Intersections::default();
        curve_intersect_ray(opp_verb, &opp_pts, opp_weight, &ray_mid, &mut opp_mid);
        let opp_outside = opp_mid.most_outside(
            self.span_t(self.angles[rh.0].start),
            self.span_t(self.angles[rh.0].end),
            d_start_pt,
        );
        if opp_outside < 0 {
            return None;
        }
        let i_side = i_mid.pt(i_outside as usize) - d_start_pt;
        let opp_side = opp_mid.pt(opp_outside as usize) - d_start_pt;
        let dir = i_side.cross_check(opp_side);
        if dir == 0.0 {
            return None;
        }
        Some(dir < 0.0)
    }

    /// `SkOpAngle::oppositePlanes(rh)`.
    // Port of: src/pathops/SkOpAngle.cpp (SkOpAngle::oppositePlanes) (chrome/m156)
    #[must_use]
    fn angle_opposite_planes(&self, this: AngleId, rh: AngleId) -> bool {
        let start_span = (i32::from(self.angles[rh.0].sector_start)
            - i32::from(self.angles[this.0].sector_start))
        .abs();
        start_span >= 8
    }

    /// `SkOpAngle::lineOnOneSide(test, useOriginal)`.
    // Port of: src/pathops/SkOpAngle.cpp (SkOpAngle::lineOnOneSide) (chrome/m156)
    fn angle_line_on_one_side(&mut self, this: AngleId, test: AngleId, use_original: bool) -> i32 {
        let origin = self.angles[this.0].part.curve.pts[0];
        let line = self.angles[this.0].part.curve.pts[1] - origin;
        let test_verb = self.angle_segment_verb(test);
        let mut result = line_on_one_side(origin, line, &self.angles[test.0], test_verb, use_original);
        if result == -2 {
            self.angles[this.0].unorderable = true;
            result = -1;
        }
        result
    }

    /// `SkOpAngle::linesOnOriginalSide(test)`.
    // Port of: src/pathops/SkOpAngle.cpp (SkOpAngle::linesOnOriginalSide) (chrome/m156)
    fn lines_on_original_side(&mut self, this: AngleId, test: AngleId) -> i32 {
        let origin = self.angles[this.0].original_curve_part.pts[0];
        let line = self.angles[this.0].original_curve_part.pts[1] - origin;
        let mut dots = [0.0_f64; 2];
        let mut crosses = [0.0_f64; 2];
        let test_curve = self.angles[test.0].original_curve_part;
        for index in 0..2 {
            let test_line = test_curve.pts[index] - origin;
            let xy1 = line.x * test_line.y;
            let xy2 = line.y * test_line.x;
            dots[index] = line.x * test_line.x + line.y * test_line.y;
            crosses[index] = if almost_bequal_ulps(xy1, xy2) { 0.0 } else { xy1 - xy2 };
        }
        if crosses[0] * crosses[1] < 0.0 {
            return -1;
        }
        if crosses[0] != 0.0 {
            return i32::from(crosses[0] < 0.0);
        }
        if crosses[1] != 0.0 {
            return i32::from(crosses[1] < 0.0);
        }
        if (dots[0] == 0.0 && dots[1] < 0.0) || (dots[0] < 0.0 && dots[1] == 0.0) {
            return 2; // 180 degrees apart
        }
        self.angles[this.0].unorderable = true;
        -1
    }

    /// `SkOpAngle::orderable(rh)`: `0` or `1` when ordered, `-1` when not.
    // Port of: src/pathops/SkOpAngle.cpp (SkOpAngle::orderable) (chrome/m156)
    pub(crate) fn angle_orderable(&mut self, this: AngleId, rh: AngleId) -> i32 {
        let this_curve = self.angles[this.0].part.is_curve;
        let rh_curve = self.angles[rh.0].part.is_curve;
        if !this_curve {
            if !rh_curve {
                let left_x = self.angles[this.0].tangent_half.dx();
                let left_y = self.angles[this.0].tangent_half.dy();
                let right_x = self.angles[rh.0].tangent_half.dx();
                let right_y = self.angles[rh.0].tangent_half.dy();
                let x_ry = left_x * right_y;
                let rx_y = right_x * left_y;
                if x_ry == rx_y {
                    if left_x * right_x < 0.0 || left_y * right_y < 0.0 {
                        return 1; // exactly 180 degrees apart
                    }
                    return self.angle_unorderable_pair(this, rh);
                }
                return i32::from(x_ry < rx_y);
            }
            let result = self.angle_line_on_one_side(this, rh, false);
            if result >= 0 {
                return result;
            }
            if self.angles[this.0].unorderable || approximately_zero(self.angles[rh.0].side) {
                return self.angle_unorderable_pair(this, rh);
            }
        } else if !rh_curve {
            let result = self.angle_line_on_one_side(rh, this, false);
            if result >= 0 {
                return if result != 0 { 0 } else { 1 };
            }
            if self.angles[rh.0].unorderable || approximately_zero(self.angles[this.0].side) {
                return self.angle_unorderable_pair(this, rh);
            }
        } else {
            let result = self.angle_convex_hull_overlaps(this, rh);
            if result >= 0 {
                return result;
            }
        }
        i32::from(self.angle_ends_intersect(this, rh))
    }

    /// The `unorderable:` label of `SkOpAngle::orderable`.
    fn angle_unorderable_pair(&mut self, this: AngleId, rh: AngleId) -> i32 {
        self.angles[this.0].unorderable = true;
        self.angles[rh.0].unorderable = true;
        -1
    }

    /// `SkOpAngle::convexHullOverlaps(rh)`.
    // Port of: src/pathops/SkOpAngle.cpp (SkOpAngle::convexHullOverlaps) (chrome/m156)
    fn angle_convex_hull_overlaps(&mut self, this: AngleId, rh: AngleId) -> i32 {
        let sweep = self.angles[this.0].part.sweep;
        let tweep = self.angles[rh.0].part.sweep;
        let s0xs1 = sweep[0].cross_check(sweep[1]);
        let s0xt0 = sweep[0].cross_check(tweep[0]);
        let s1xt0 = sweep[1].cross_check(tweep[0]);
        let mut t_between_s = if s0xs1 > 0.0 {
            s0xt0 > 0.0 && s1xt0 < 0.0
        } else {
            s0xt0 < 0.0 && s1xt0 > 0.0
        };
        let s0xt1 = sweep[0].cross_check(tweep[1]);
        let s1xt1 = sweep[1].cross_check(tweep[1]);
        t_between_s |= if s0xs1 > 0.0 {
            s0xt1 > 0.0 && s1xt1 < 0.0
        } else {
            s0xt1 < 0.0 && s1xt1 > 0.0
        };
        let t0xt1 = tweep[0].cross_check(tweep[1]);
        if t_between_s {
            return -1;
        }
        if (s0xt0 == 0.0 && s1xt1 == 0.0) || (s1xt0 == 0.0 && s0xt1 == 0.0) {
            // s0 to s1 equals t0 to t1
            return -1;
        }
        let mut s_between_t = if t0xt1 > 0.0 {
            s0xt0 < 0.0 && s0xt1 > 0.0
        } else {
            s0xt0 > 0.0 && s0xt1 < 0.0
        };
        s_between_t |= if t0xt1 > 0.0 {
            s1xt0 < 0.0 && s1xt1 > 0.0
        } else {
            s1xt0 > 0.0 && s1xt1 < 0.0
        };
        if s_between_t {
            return -1;
        }
        if s0xt0 >= 0.0 && s0xt1 >= 0.0 && s1xt0 >= 0.0 && s1xt1 >= 0.0 {
            return 0;
        }
        if s0xt0 <= 0.0 && s0xt1 <= 0.0 && s1xt0 <= 0.0 && s1xt1 <= 0.0 {
            return 1;
        }
        let m0 = self.seg_d_pt_at_t(self.angle_segment(this), self.angle_mid_t(this))
            - self.angles[this.0].part.curve.pts[0];
        let m1 = self.seg_d_pt_at_t(self.angle_segment(rh), self.angle_mid_t(rh))
            - self.angles[rh.0].part.curve.pts[0];
        let m0xm1 = m0.cross_check(m1);
        if s0xt0 > 0.0 && m0xm1 > 0.0 {
            return 0;
        }
        if s0xt0 < 0.0 && m0xm1 < 0.0 {
            return 1;
        }
        if self.angle_tangents_diverge(this, rh, s0xt0) {
            return i32::from(s0xt0 < 0.0);
        }
        i32::from(m0xm1 < 0.0)
    }

    /// `SkOpAngle::endsIntersect(rh)`.
    // Port of: src/pathops/SkOpAngle.cpp (SkOpAngle::endsIntersect) (chrome/m156)
    fn angle_ends_intersect(&mut self, this: AngleId, rh: AngleId) -> bool {
        let l_verb = self.angle_segment_verb(this);
        let r_verb = self.angle_segment_verb(rh);
        let l_pts = verb_points(l_verb);
        let r_pts = verb_points(r_verb);
        let this_curve = self.angles[this.0].part.curve;
        let rh_curve = self.angles[rh.0].part.curve;
        let rays = [
            DLine::new([this_curve.pts[0], rh_curve.pts[r_pts]]),
            DLine::new([this_curve.pts[0], this_curve.pts[l_pts]]),
        ];
        if self.span_contains_span(self.angles[this.0].end, self.angles[rh.0].end) {
            return self.angle_check_parallel(this, rh);
        }
        let mut small_ts = [-1.0_f64; 2];
        let mut limited = [false; 2];
        for index in 0..2 {
            let c_verb = if index != 0 { r_verb } else { l_verb };
            if c_verb == Verb::Line {
                continue;
            }
            let seg = if index != 0 {
                self.angle_segment(rh)
            } else {
                self.angle_segment(this)
            };
            let seg_pts = self.seg_pts(seg);
            let seg_weight = self.seg_weight(seg);
            let mut i = Intersections::default();
            curve_intersect_ray(c_verb, &seg_pts, seg_weight, &rays[index], &mut i);
            let t_start = if index != 0 {
                self.span_t(self.angles[rh.0].start)
            } else {
                self.span_t(self.angles[this.0].start)
            };
            let t_end = if index != 0 {
                self.span_t(self.angles[rh.0].computed_end)
            } else {
                self.span_t(self.angles[this.0].computed_end)
            };
            let test_ascends = t_start < t_end;
            let mut t = if test_ascends { 0.0 } else { 1.0 };
            for idx2 in 0..i.used() {
                let test_t = i.t(0, idx2);
                if !approximately_between_orderable(t_start, test_t, t_end) {
                    continue;
                }
                if approximately_equal_orderable(t_start, test_t) {
                    continue;
                }
                t = if test_ascends {
                    std_max(t, test_t)
                } else {
                    std_min(t, test_t)
                };
                small_ts[index] = t;
                limited[index] = approximately_equal_orderable(t, t_end);
            }
        }
        let mut s_ray_longer = false;
        let mut s_cept = DVector::default();
        let mut s_cept_t = -1.0_f64;
        let mut s_index: i32 = -1;
        let mut use_intersect = false;
        for index in 0..2 {
            if small_ts[index] < 0.0 {
                continue;
            }
            let seg = if index != 0 {
                self.angle_segment(rh)
            } else {
                self.angle_segment(this)
            };
            let d_pt = self.seg_d_pt_at_t(seg, small_ts[index]);
            let cept = d_pt - rays[index].pts[0];
            if (if index != 0 { l_pts } else { r_pts }) == 1 {
                let total = rays[index].pts[1] - rays[index].pts[0];
                if cept.length_squared() * 2.0 < total.length_squared() {
                    continue;
                }
            }
            let end = rays[index].pts[1] - rays[index].pts[0];
            if cept.x * end.x < 0.0 || cept.y * end.y < 0.0 {
                continue;
            }
            let ray_dist = cept.length();
            let end_dist = end.length();
            let ray_longer = ray_dist > end_dist;
            if limited[0] && limited[1] && ray_longer {
                use_intersect = true;
                s_ray_longer = ray_longer;
                s_cept = cept;
                s_cept_t = small_ts[index];
                s_index = index as i32;
                break;
            }
            let mut delta = (ray_dist - end_dist).abs();
            let curve = if index != 0 {
                self.angles[rh.0].part.curve
            } else {
                self.angles[this.0].part.curve
            };
            let pt_count = if index != 0 { r_pts } else { l_pts };
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for idx2 in 0..=pt_count {
                min_x = std_min(min_x, curve.at(idx2).x);
                min_y = std_min(min_y, curve.at(idx2).y);
                max_x = std_max(max_x, curve.at(idx2).x);
                max_y = std_max(max_y, curve.at(idx2).y);
            }
            let max_width = std_max(max_x - min_x, max_y - min_y);
            delta = ieee_divide(delta, max_width);
            if delta < 4e-3
                && delta > 1e-3
                && !use_intersect
                && self.angles[this.0].part.is_curve
                && self.angles[rh.0].part.is_curve
                && self.angles[this.0].original_curve_part.pts[0] != self.angles[this.0].part.curve.pts[0]
            {
                let origin = self.angles[rh.0].original_curve_part.pts[0];
                let count = verb_points(self.angle_segment_verb(rh));
                let line = self.angles[rh.0].original_curve_part.pts[count] - origin;
                let original_side = self.line_on_one_side_of(this, origin, line, true);
                if original_side >= 0 {
                    let translated_side = self.line_on_one_side_of(this, origin, line, false);
                    if original_side != translated_side {
                        continue;
                    }
                }
            }
            // `delta > 1e-3 && (useIntersect ^= true)`: the toggle only happens past the test.
            if delta > 1e-3 {
                use_intersect ^= true;
                if use_intersect {
                    s_ray_longer = ray_longer;
                    s_cept = cept;
                    s_cept_t = small_ts[index];
                    s_index = index as i32;
                }
            }
        }
        if use_intersect {
            let sindex = s_index as usize;
            let seg = if sindex != 0 {
                self.angle_segment(rh)
            } else {
                self.angle_segment(this)
            };
            let curve = if sindex != 0 {
                self.angles[rh.0].part.curve
            } else {
                self.angles[this.0].part.curve
            };
            let t_start = if sindex != 0 {
                self.span_t(self.angles[rh.0].start)
            } else {
                self.span_t(self.angles[this.0].start)
            };
            let mid = self.seg_d_pt_at_t(seg, t_start + (s_cept_t - t_start) / 2.0) - curve.pts[0];
            let sept_dir = mid.cross_check(s_cept);
            if sept_dir == 0.0 {
                return self.angle_check_parallel(this, rh);
            }
            return s_ray_longer ^ (sindex == 0) ^ (sept_dir < 0.0);
        }
        self.angle_check_parallel(this, rh)
    }

    /// `lineOnOneSide(origin, line, test, useOriginal)`, called on the other angle.
    fn line_on_one_side_of(&self, test: AngleId, origin: DPoint, line: DVector, use_original: bool) -> i32 {
        let test_verb = self.angle_segment_verb(test);
        line_on_one_side(origin, line, &self.angles[test.0], test_verb, use_original)
    }
}

/// `sk_ieee_double_divide(a, b)`: IEEE division, which never traps on zero.
// Port of: src/base/SkFloatingPoint.h (sk_ieee_double_divide) (chrome/m156)
#[must_use]
pub(crate) fn ieee_divide(a: f64, b: f64) -> f64 {
    a / b
}
