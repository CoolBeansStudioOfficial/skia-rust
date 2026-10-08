// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsCommon.cpp, src/pathops/SkPathOpsCommon.h (chrome/m156)

//! The steps shared by the boolean operation and by simplify (`SkPathOpsCommon.cpp`): winding of
//! a chain of angles, the search for a span to chase, contour sorting, and the handling of
//! coincident edges.

use skia_rust_core::t_sort::t_q_sort;

use crate::op_state::{AngleId, CoinSetId, ContourId, OpState, SK_MIN_S32, SegId, SpanId};

/// The result of `AngleWinding`: the angle the search stopped at, if any, and the winding it
/// found. `sortable` is only meaningful when `angle` is `Some`.
// Port of: src/pathops/SkPathOpsCommon.cpp#L11-L63 (chrome/m156)
#[derive(Copy, Clone, Debug)]
pub(crate) struct AngleWinding {
    pub(crate) angle: Option<AngleId>,
    pub(crate) winding: i32,
    pub(crate) sortable: bool,
}

impl OpState {
    /// `AngleWinding(start, end, &winding, &sortable)`: finds the first angle of the span from
    /// `start` to `end`, and the winding that the angles around it produce.
    // Port of: src/pathops/SkPathOpsCommon.cpp#L11-L63 (chrome/m156)
    pub(crate) fn angle_winding(&mut self, start: SpanId, end: SpanId) -> AngleWinding {
        let not_found = AngleWinding {
            angle: None,
            winding: SK_MIN_S32,
            sortable: false,
        };
        let Some(first_angle) = self.seg_span_to_angle(start, end) else {
            return not_found;
        };
        let mut angle = first_angle;
        let mut loop_seen = false;
        let mut unorderable = false;
        let mut winding = SK_MIN_S32;
        let compute_winding = loop {
            let Some(next) = self.angle_next(angle) else {
                return not_found;
            };
            angle = next;
            unorderable |= self.angle_unorderable(angle);
            let compute = unorderable || (angle == first_angle && loop_seen);
            if compute {
                // If we get here, there's no winding, the loop is unorderable.
                break true;
            }
            loop_seen |= angle == first_angle;
            winding = self.seg_wind_sum_angle(angle);
            if winding != SK_MIN_S32 {
                break false;
            }
        };
        // If the angle loop contains an unorderable span, the angle order may be useless. Compute
        // the winding directly for each span in this case.
        if compute_winding {
            let first_angle = angle;
            winding = SK_MIN_S32;
            loop {
                let start_span = self.angle_start(angle);
                let end_span = self.angle_end(angle);
                let lesser = self.span_starter(start_span, end_span);
                let mut test_winding = self.span_wind_sum(lesser);
                if test_winding == SK_MIN_S32 {
                    test_winding = self.span_compute_wind_sum(lesser);
                }
                if test_winding != SK_MIN_S32 {
                    winding = test_winding;
                }
                let Some(next) = self.angle_next(angle) else {
                    return not_found;
                };
                angle = next;
                if angle == first_angle {
                    break;
                }
            }
        }
        AngleWinding {
            angle: Some(angle),
            winding,
            sortable: !unorderable,
        }
    }

    /// `FindUndone(contourHead)`: the first span that is not done, in any contour.
    // Port of: src/pathops/SkPathOpsCommon.cpp#L65-L78 (chrome/m156)
    pub(crate) fn find_undone(&mut self, contour_head: ContourId) -> Option<SpanId> {
        let mut contour = Some(contour_head);
        while let Some(c) = contour {
            if !self.contour_done(c) {
                if let Some(result) = self.contour_undone_span(c) {
                    return Some(result);
                }
            }
            contour = self.contour_next(c);
        }
        None
    }

    /// `FindChase(chase, startPtr, endPtr)`: pops spans from the chase until one yields a
    /// segment to follow. Returns that segment and updates the start and end span.
    // Port of: src/pathops/SkPathOpsCommon.cpp#L80-L140 (chrome/m156)
    pub(crate) fn find_chase(
        &mut self,
        chase: &mut Vec<SpanId>,
        start_ptr: &mut SpanId,
        end_ptr: &mut Option<SpanId>,
    ) -> Option<SegId> {
        while let Some(span) = chase.pop() {
            let next_ptt = self.ptt_next(self.span_ptt(span));
            *start_ptr = self.ptt_span(next_ptt);
            let mut done = true;
            *end_ptr = None;
            let active_start = *start_ptr;
            if let Some(last) = self.seg_active_angle(active_start, start_ptr, end_ptr, &mut done) {
                *start_ptr = self.angle_start(last);
                *end_ptr = Some(self.angle_end(last));
                chase.push(span);
                return Some(self.angle_segment(last));
            }
            if done {
                continue;
            }
            // Find the first angle, and initialize the winding to the computed wind sum.
            let Some(end) = *end_ptr else {
                return None;
            };
            let found = self.angle_winding(*start_ptr, end);
            let Some(angle) = found.angle else {
                return None;
            };
            if found.winding == SK_MIN_S32 {
                continue;
            }
            let mut sum_winding = 0_i32;
            let mut segment = self.angle_segment(angle);
            if found.sortable {
                sum_winding = self.seg_update_winding_reverse(segment, angle);
            }
            let mut first: Option<SegId> = None;
            let first_angle = angle;
            let mut angle = angle;
            loop {
                let Some(next) = self.angle_next(angle) else {
                    return None;
                };
                angle = next;
                if angle == first_angle {
                    break;
                }
                segment = self.angle_segment(angle);
                let start = self.angle_start(angle);
                let end = self.angle_end(angle);
                let mut max_winding = 0_i32;
                if found.sortable {
                    max_winding = self.seg_set_up_winding(start, end, &mut sum_winding);
                }
                if !self.seg_done_angle(segment, angle) {
                    if first.is_none()
                        && (found.sortable || self.span_wind_sum(self.span_starter(start, end)) != SK_MIN_S32)
                    {
                        first = Some(segment);
                        *start_ptr = start;
                        *end_ptr = Some(end);
                    }
                    // OPTIMIZATION: should this also add to the chase?
                    if found.sortable {
                        let mut result: Option<SpanId> = None;
                        let marked = self.seg_mark_angle(segment, max_winding, sum_winding, angle, &mut result);
                        // Skia asserts this result; the call itself runs in release builds.
                        debug_assert!(marked, "markAngle failed in FindChase");
                    }
                }
            }
            if let Some(first) = first {
                chase.push(span);
                return Some(first);
            }
        }
        None
    }
}

/// The sort key of `SkOpContour::operator<`: top, then left.
// Port of: src/pathops/SkOpContour.h#L34-L38 (chrome/m156)
fn contour_less(state: &OpState, a: ContourId, b: ContourId) -> bool {
    let ab = state.contour_bounds(a);
    let bb = state.contour_bounds(b);
    if ab.top == bb.top {
        ab.left < bb.left
    } else {
        ab.top < bb.top
    }
}

impl OpState {
    /// `SortContourList(contourList, evenOdd, oppEvenOdd)`: keeps the contours that have edges,
    /// sorts them by bounds, and links them. Returns false if there are no such contours.
    // Port of: src/pathops/SkPathOpsCommon.cpp#L167-L195 (chrome/m156)
    pub(crate) fn sort_contour_list(&mut self, contour_list: &mut ContourId, even_odd: bool, opp_even_odd: bool) -> bool {
        let mut list: Vec<ContourId> = Vec::new();
        let mut contour = Some(*contour_list);
        while let Some(c) = contour {
            if self.contour_count(c) != 0 {
                let opp_xor = if self.contour_operand(c) { even_odd } else { opp_even_odd };
                self.contour_set_opp_xor(c, opp_xor);
                list.push(c);
            }
            contour = self.contour_next(c);
        }
        let count = list.len();
        if count == 0 {
            return false;
        }
        if count > 1 {
            let state: &OpState = self;
            t_q_sort(&mut list, |a: &ContourId, b: &ContourId| contour_less(state, *a, *b));
        }
        let head = list[0];
        self.contour_head = Some(head);
        *contour_list = head;
        for index in 1..count {
            self.contour_set_next(list[index - 1], Some(list[index]));
        }
        self.contour_set_next(list[count - 1], None);
        true
    }

    /// `calc_angles(contourList)`.
    // Port of: src/pathops/SkPathOpsCommon.cpp#L197-L203 (chrome/m156)
    pub(crate) fn calc_angles(&mut self, contour_list: ContourId) {
        let mut contour = Some(contour_list);
        while let Some(c) = contour {
            self.contour_calc_angles(c);
            contour = self.contour_next(c);
        }
    }

    /// `missing_coincidence(contourList)`: every contour is checked, even after one reports a
    /// missing coincidence.
    // Port of: src/pathops/SkPathOpsCommon.cpp#L205-L213 (chrome/m156)
    pub(crate) fn missing_coincidence(&mut self, contour_list: ContourId) -> bool {
        let mut result = false;
        let mut contour = Some(contour_list);
        while let Some(c) = contour {
            result |= self.contour_missing_coincidence(c);
            contour = self.contour_next(c);
        }
        result
    }

    /// `move_multiples(contourList)`.
    // Port of: src/pathops/SkPathOpsCommon.cpp#L215-L224 (chrome/m156)
    pub(crate) fn move_multiples(&mut self, contour_list: ContourId) -> bool {
        let mut contour = Some(contour_list);
        while let Some(c) = contour {
            if !self.contour_move_multiples(c) {
                return false;
            }
            contour = self.contour_next(c);
        }
        true
    }

    /// `move_nearby(contourList)`.
    // Port of: src/pathops/SkPathOpsCommon.cpp#L226-L235 (chrome/m156)
    pub(crate) fn move_nearby(&mut self, contour_list: ContourId) -> bool {
        let mut contour = Some(contour_list);
        while let Some(c) = contour {
            if !self.contour_move_nearby(c) {
                return false;
            }
            contour = self.contour_next(c);
        }
        true
    }

    /// `sort_angles(contourList)`.
    // Port of: src/pathops/SkPathOpsCommon.cpp#L237-L245 (chrome/m156)
    pub(crate) fn sort_angles(&mut self, contour_list: ContourId) -> bool {
        let mut contour = Some(contour_list);
        while let Some(c) = contour {
            if !self.contour_sort_angles(c) {
                return false;
            }
            contour = self.contour_next(c);
        }
        true
    }

    /// `HandleCoincidence(contourList, coincidence)`: matches up the points of coincident runs,
    /// and finds the overlaps between them.
    // Port of: src/pathops/SkPathOpsCommon.cpp#L247-L338 (chrome/m156)
    pub(crate) fn handle_coincidence(&mut self, contour_list: ContourId, coincidence: CoinSetId) -> bool {
        // Match up points within the coincident runs.
        if !self.cs_add_expanded(coincidence) {
            return false;
        }
        // Combine t values when multiple intersections occur on some segments but not others.
        if !self.move_multiples(contour_list) {
            return false;
        }
        // Move t values and points together to eliminate small/tiny gaps.
        if !self.move_nearby(contour_list) {
            return false;
        }
        // Add coincidence formed by pairing on curve points and endpoints.
        self.cs_correct_ends(coincidence);
        if !self.cs_add_end_moved_all(coincidence) {
            return false;
        }
        const SAFETY_COUNT: i32 = 3;
        let mut safety_hatch = SAFETY_COUNT;
        // Look for coincidence present in A-B and A-C but missing in B-C.
        loop {
            let mut added = false;
            if !self.cs_add_missing(coincidence, &mut added) {
                return false;
            }
            if !added {
                break;
            }
            safety_hatch -= 1;
            if safety_hatch == 0 {
                return false;
            }
        }
        // Check to see if, loosely, coincident ranges may be expanded.
        if self.cs_expand(coincidence) {
            let mut added = false;
            if !self.cs_add_missing(coincidence, &mut added) {
                return false;
            }
            if !self.cs_add_expanded(coincidence) {
                return false;
            }
            if !self.move_multiples(contour_list) {
                return false;
            }
            let _ = self.move_nearby(contour_list);
        }
        // The expanded ranges may not align: add the missing spans.
        if !self.cs_add_expanded(coincidence) {
            return false;
        }
        // Mark spans of coincident segments as coincident.
        let _ = self.cs_mark(coincidence);
        // Look for coincidence lines and curves undetected by intersection.
        if self.missing_coincidence(contour_list) {
            let _ = self.cs_expand(coincidence);
            if !self.cs_add_expanded(coincidence) {
                return false;
            }
            if !self.cs_mark(coincidence) {
                return false;
            }
        } else {
            let _ = self.cs_expand(coincidence);
        }
        let _ = self.cs_expand(coincidence);

        // `SkOpCoincidence overlaps(globalState)`: constructing it makes it the global object.
        let overlaps = self.coin_new_set();
        let mut safety_hatch = SAFETY_COUNT;
        loop {
            let pairs = if self.cs_is_empty(overlaps) { coincidence } else { overlaps };
            // Adjust the winding value to account for coincident edges.
            if !self.cs_apply(pairs) {
                return false;
            }
            // For each coincident pair that overlaps another, when the receivers (the first of
            // the pair) are different, construct a new pair to resolve their mutual span.
            if !self.cs_find_overlaps(pairs, overlaps) {
                return false;
            }
            safety_hatch -= 1;
            if safety_hatch == 0 {
                return false;
            }
            if self.cs_is_empty(overlaps) {
                break;
            }
        }
        self.calc_angles(contour_list);
        if !self.sort_angles(contour_list) {
            return false;
        }
        // The overlaps object goes out of scope in Skia; the global pointer goes back to the
        // object that the caller owns.
        self.coin_set_global(coincidence);
        true
    }
}
