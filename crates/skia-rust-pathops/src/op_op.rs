// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsOp.cpp (chrome/m156)

//! The boolean operations (`SkPathOpsOp.cpp`): `Op(one, two, op)` and the walk that writes the
//! result contours.
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

use skia_rust_core::path::{Path, Verb};
use skia_rust_core::path_types::PathFillType;

use crate::op_add_intersections::add_intersect_ts;
use crate::op_edge_builder::EdgeBuilder;
use crate::op_simplify::simplify;
use crate::op_state::{ContourId, OpState, SK_MIN_S32, SegId, SpanId};
use crate::path_op::PathOp;
use crate::path_writer::PathWriter;

/// `kWinding_PathOpsMask` and `kEvenOdd_PathOpsMask` of `SkPathOpsTypes.h`: the winding masks
/// that are combined with the sums of the segments.
// Port of: src/pathops/SkPathOpsTypes.h (SkPathOpsMask) (chrome/m156)
const K_WINDING_PATH_OPS_MASK: i32 = -1;
const K_EVEN_ODD_PATH_OPS_MASK: i32 = 1;

/// The `SkPathOpsMask` of an operand, from whether it is even-odd.
// Port of: src/pathops/SkOpEdgeBuilder.cpp (xorMask assignment) (chrome/m156)
fn path_ops_mask(even_odd: bool) -> i32 {
    if even_odd {
        K_EVEN_ODD_PATH_OPS_MASK
    } else {
        K_WINDING_PATH_OPS_MASK
    }
}

/// `gOpInverse[op][one.isInverseFillType()][two.isInverseFillType()]`: the operation that
/// replaces `op` once the inside/outside of each operand is known.
// Port of: src/pathops/SkPathOpsOp.cpp#L220-L228 (chrome/m156)
// The diagram of why this simplification is possible is at https://skia.org/dev/present/pathops
const K_OP_INVERSE: [[[PathOp; 2]; 2]; 5] = [
    [
        [PathOp::Difference, PathOp::Intersect],
        [PathOp::Union, PathOp::ReverseDifference],
    ],
    [
        [PathOp::Intersect, PathOp::Difference],
        [PathOp::ReverseDifference, PathOp::Union],
    ],
    [
        [PathOp::Union, PathOp::ReverseDifference],
        [PathOp::Difference, PathOp::Intersect],
    ],
    [[PathOp::Xor, PathOp::Xor], [PathOp::Xor, PathOp::Xor]],
    [
        [PathOp::ReverseDifference, PathOp::Union],
        [PathOp::Intersect, PathOp::Difference],
    ],
];

/// `gOutInverse[op][one.isInverseFillType()][two.isInverseFillType()]`: whether the result of
/// `op` is inverse-filled.
// Port of: src/pathops/SkPathOpsOp.cpp#L230-L236 (chrome/m156)
const K_OUT_INVERSE: [[[bool; 2]; 2]; 5] = [
    [[false, false], [true, false]], // diff
    [[false, false], [false, true]], // sect
    [[false, true], [true, true]],   // union
    [[false, true], [true, false]],  // xor
    [[false, true], [false, false]], // rev diff
];

/// `findChaseOp(chase, startPtr, endPtr, result)`. The outer `None` is the `false` return of
/// Skia (a failure); `Some(None)` is a `true` return with no segment to follow.
// Port of: src/pathops/SkPathOpsOp.cpp#L28-L120 (chrome/m156)
fn find_chase_op(
    state: &mut OpState,
    chase: &mut Vec<SpanId>,
    start_ptr: &mut SpanId,
    end_ptr: &mut SpanId,
) -> Option<Option<SegId>> {
    while let Some(span) = chase.pop() {
        // OPTIMIZE: prev makes this compatible with old code -- but is it necessary?
        let prev_ptt = state.ptt_prev(state.span_ptt(span));
        *start_ptr = state.ptt_span(prev_ptt);
        let mut segment: SegId;
        let mut done = true;
        let mut end_opt: Option<SpanId> = None;
        let active_start = *start_ptr;
        if let Some(last) = state.seg_active_angle(active_start, start_ptr, &mut end_opt, &mut done)
        {
            *start_ptr = state.angle_start(last);
            *end_ptr = state.angle_end(last);
            chase.push(span);
            return Some(Some(state.angle_segment(last)));
        }
        if done {
            continue;
        }
        let Some(end) = end_opt else {
            return Some(None);
        };
        let found = state.angle_winding(*start_ptr, end);
        let Some(angle) = found.angle else {
            return Some(None);
        };
        if found.winding == SK_MIN_S32 {
            continue;
        }
        let mut sum_mi_winding = 0_i32;
        let mut sum_su_winding = 0_i32;
        if found.sortable {
            segment = state.angle_segment(angle);
            sum_mi_winding = state.seg_update_winding_reverse(segment, angle);
            if sum_mi_winding == SK_MIN_S32 {
                return Some(None);
            }
            sum_su_winding = state.seg_update_opp_winding_reverse(segment, angle);
            if sum_su_winding == SK_MIN_S32 {
                return Some(None);
            }
            if state.seg_operand(segment) {
                std::mem::swap(&mut sum_mi_winding, &mut sum_su_winding);
            }
        }
        let mut first: Option<SegId> = None;
        let first_angle = angle;
        let mut angle = angle;
        loop {
            let Some(next) = state.angle_next(angle) else {
                return Some(None);
            };
            angle = next;
            if angle == first_angle {
                break;
            }
            segment = state.angle_segment(angle);
            let start = state.angle_start(angle);
            let end = state.angle_end(angle);
            let (max_winding, sum_winding, opp_max_winding, opp_sum_winding) = if found.sortable {
                state.seg_set_up_windings_opp(
                    segment,
                    start,
                    end,
                    &mut sum_mi_winding,
                    &mut sum_su_winding,
                )
            } else {
                (0, 0, 0, 0)
            };
            if !state.seg_done_angle(segment, angle) {
                if first.is_none()
                    && (found.sortable
                        || state.span_wind_sum(state.span_starter(start, end)) != SK_MIN_S32)
                {
                    first = Some(segment);
                    *start_ptr = start;
                    *end_ptr = end;
                }
                // OPTIMIZATION: should this also add to the chase?
                if found.sortable {
                    let mut result: Option<SpanId> = None;
                    if !state.seg_mark_angle_opp(
                        segment,
                        max_winding,
                        sum_winding,
                        opp_max_winding,
                        opp_sum_winding,
                        angle,
                        &mut result,
                    ) {
                        return None;
                    }
                }
            }
        }
        if let Some(first) = first {
            chase.push(span);
            return Some(Some(first));
        }
    }
    Some(None)
}

/// `bridgeOp(contourList, op, xorMask, xorOpMask, writer)`: walks the spans of both operands,
/// keeping those that the operation selects, and writes the closed contours to `writer`.
// Port of: src/pathops/SkPathOpsOp.cpp#L122-L215 (chrome/m156)
fn bridge_op(
    state: &mut OpState,
    contour_list: ContourId,
    op: PathOp,
    xor_mask: i32,
    xor_op_mask: i32,
    writer: &mut PathWriter,
) -> bool {
    let mut unsortable = false;
    // Assigned before its first use, in the same pass that reads it.
    let mut last_simple: bool;
    let mut simple = false;
    loop {
        let Some(span) = state.find_sortable_top(contour_list) else {
            break;
        };
        let mut current = state.span_segment(span);
        let mut start = state
            .span_next(span)
            .expect("a sortable span has a next span");
        let mut end = span;
        let mut chase: Vec<SpanId> = Vec::new();
        loop {
            if state.seg_active_op(current, start, end, xor_mask, xor_op_mask, op) {
                loop {
                    if !unsortable && state.seg_done(current) {
                        break;
                    }
                    let mut next_start = start;
                    let mut next_end = end;
                    last_simple = simple;
                    let Some(next) = state.seg_find_next_op(
                        current,
                        &mut chase,
                        &mut next_start,
                        &mut next_end,
                        &mut unsortable,
                        &mut simple,
                        op,
                        xor_mask,
                        xor_op_mask,
                    ) else {
                        if !unsortable
                            && writer.has_move()
                            && !matches!(state.seg_verb(current), Verb::Line)
                            && !writer.is_closed(state)
                        {
                            if !state.seg_add_curve_to(current, start, end, writer) {
                                return false;
                            }
                        } else if last_simple
                            && !state.seg_add_curve_to(current, start, end, writer)
                        {
                            return false;
                        }
                        break;
                    };
                    if !state.seg_add_curve_to(current, start, end, writer) {
                        return false;
                    }
                    current = next;
                    start = next_start;
                    end = next_end;
                    if writer.is_closed(state)
                        || (unsortable && state.span_done(state.span_starter(start, end)))
                    {
                        break;
                    }
                }
                if state.seg_active_winding(current, start, end) && !writer.is_closed(state) {
                    let span_start = state.span_starter(start, end);
                    if !state.span_done(span_start) {
                        if !state.seg_add_curve_to(current, start, end, writer) {
                            return false;
                        }
                        state.seg_mark_done(current, span_start);
                    }
                }
                writer.finish(state);
            } else {
                let mut last: Option<SpanId> = None;
                if !state.seg_mark_and_chase_done(current, start, end, Some(&mut last)) {
                    return false;
                }
                if let Some(last_span) = last
                    && !state.span_chased(last_span)
                {
                    state.span_set_chased(last_span, true);
                    chase.push(last_span);
                }
            }
            let Some(found) = find_chase_op(state, &mut chase, &mut start, &mut end) else {
                return false;
            };
            let Some(next) = found else {
                break;
            };
            current = next;
        }
    }
    true
}

/// `OpDebug(one, two, op)`: the boolean operation of two paths. Returns `None` if the operation
/// fails.
// Port of: src/pathops/SkPathOpsOp.cpp#L252-L373 (chrome/m156)
pub(crate) fn op_debug(one: &Path, two: &Path, op: PathOp) -> Option<Path> {
    let one_inverse = usize::from(one.is_inverse_fill_type());
    let two_inverse = usize::from(two.is_inverse_fill_type());
    let mut op = K_OP_INVERSE[op as usize][one_inverse][two_inverse];
    let inverse_fill = K_OUT_INVERSE[op as usize][one_inverse][two_inverse];
    let fill_type = if inverse_fill {
        PathFillType::InverseEvenOdd
    } else {
        PathFillType::EvenOdd
    };
    if op == PathOp::Intersect
        && let Some((mut rect1, _, _)) = one.is_rect()
        && let Some((rect2, _, _)) = two.is_rect()
    {
        let result = if rect1.intersect(rect2) {
            Path::rect(rect1, None)
        } else {
            Path::new()
        };
        return Some(result.make_fill_type(fill_type));
    }
    if one.is_empty() || two.is_empty() {
        let mut work = match op {
            PathOp::Intersect => Path::new(),
            PathOp::Union | PathOp::Xor => {
                if one.is_empty() {
                    two.clone()
                } else {
                    one.clone()
                }
            }
            PathOp::Difference => {
                if one.is_empty() {
                    Path::new()
                } else {
                    one.clone()
                }
            }
            PathOp::ReverseDifference => {
                if two.is_empty() {
                    Path::new()
                } else {
                    two.clone()
                }
            }
        };
        if inverse_fill != work.is_inverse_fill_type() {
            work.toggle_inverse_fill_type();
        }
        return simplify(&work);
    }
    let mut state = OpState::new();
    let mut contour_list = state.contour_head();
    let coincidence = state.coin_new_set();
    let (mut minuend, mut subtrahend) = (one, two);
    if op == PathOp::ReverseDifference {
        std::mem::swap(&mut minuend, &mut subtrahend);
        op = PathOp::Difference;
    }
    // Turn the path into a list of segments.
    let mut builder = EdgeBuilder::new(minuend, contour_list);
    if builder.unparseable() {
        return None;
    }
    let xor_mask = path_ops_mask(builder.xor_mask());
    builder.add_operand(subtrahend);
    if !builder.finish(&mut state) {
        return None;
    }
    let xor_op_mask = path_ops_mask(builder.xor_mask());
    if !state.sort_contour_list(
        &mut contour_list,
        xor_mask == K_EVEN_ODD_PATH_OPS_MASK,
        xor_op_mask == K_EVEN_ODD_PATH_OPS_MASK,
    ) {
        return Some(Path::new_with_fill_type(fill_type));
    }
    // Find all intersections between segments.
    let mut current = Some(contour_list);
    while let Some(cur) = current {
        let mut next = cur;
        while add_intersect_ts(&mut state, cur, next, coincidence) {
            match state.contour_next(next) {
                Some(n) => next = n,
                None => break,
            }
        }
        current = state.contour_next(cur);
    }
    if !state.handle_coincidence(contour_list, coincidence) {
        return None;
    }
    // Construct closed contours.
    let mut writer = PathWriter::new(fill_type);
    if !bridge_op(
        &mut state,
        contour_list,
        op,
        xor_mask,
        xor_op_mask,
        &mut writer,
    ) {
        return None;
    }
    // If some edges could not be resolved, assemble the remaining ones.
    state.writer_assemble(&mut writer);
    Some(writer.native_path())
}

/// `Op(one, two, op)`: the boolean operation `op` applied to `one` and `two`, or `None` if the
/// operation fails.
// Port of: src/pathops/SkPathOpsOp.cpp#L375-L391 (chrome/m156)
#[doc(alias = "Op")]
#[must_use]
pub fn op(one: &Path, two: &Path, op: PathOp) -> Option<Path> {
    op_debug(one, two, op)
}
