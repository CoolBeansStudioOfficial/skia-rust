// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsSimplify.cpp (chrome/m156)

//! Simplify: removes self-intersections, and gives every contour a consistent fill
//! (`SkPathOpsSimplify.cpp`, `Simplify(path)`).
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

use skia_rust_core::path::{Iter, Path, Verb};
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;

use crate::op_add_intersections::add_intersect_ts;
use crate::op_edge_builder::EdgeBuilder;
use crate::op_state::{ContourId, OpState, SpanId};
use crate::path_writer::PathWriter;

/// `bridgeWinding(contourList, writer)`: walks the spans by winding, writing each closed
/// contour. Returns false if the path cannot be written.
// Port of: src/pathops/SkPathOpsSimplify.cpp#L11-L69 (chrome/m156)
fn bridge_winding(state: &mut OpState, contour_list: ContourId, writer: &mut PathWriter) -> bool {
    let mut unsortable = false;
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
            if state.seg_active_winding(current, start, end) {
                loop {
                    if !unsortable && state.seg_done(current) {
                        break;
                    }
                    let mut next_start = start;
                    let mut next_end = end;
                    let Some(next) = state.seg_find_next_winding(
                        current,
                        &mut chase,
                        &mut next_start,
                        &mut next_end,
                        &mut unsortable,
                    ) else {
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
            let mut end_opt = Some(end);
            let found = state.find_chase(&mut chase, &mut start, &mut end_opt);
            if let Some(e) = end_opt {
                end = e;
            }
            match found {
                Some(next) => current = next,
                None => break,
            }
        }
    }
    true
}

/// `bridgeXor(contourList, writer)`: the even-odd version of [`bridge_winding`].
// Port of: src/pathops/SkPathOpsSimplify.cpp#L71-L119 (chrome/m156)
fn bridge_xor(state: &mut OpState, contour_list: ContourId, writer: &mut PathWriter) -> bool {
    let mut unsortable = false;
    let mut safety_net: i32 = 1_000_000;
    loop {
        let Some(span) = state.find_undone(contour_list) else {
            break;
        };
        let mut current = state.span_segment(span);
        let mut start = state
            .span_next(span)
            .expect("an undone span has a next span");
        let mut end = span;
        loop {
            safety_net -= 1;
            if safety_net < 0 {
                return false;
            }
            if !unsortable && state.seg_done(current) {
                break;
            }
            let mut next_start = start;
            let mut next_end = end;
            let Some(next) =
                state.seg_find_next_xor(current, &mut next_start, &mut next_end, &mut unsortable)
            else {
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
        if !writer.is_closed(state) {
            let span_start = state.span_starter(start, end);
            if !state.span_done(span_start) {
                return false;
            }
        }
        writer.finish(state);
    }
    true
}

/// `path_is_trivial(path)`: every point of the path lies on one straight line per contour, so
/// the path has no area.
// Port of: src/pathops/SkPathOpsSimplify.cpp#L121-L164 (chrome/m156)
fn path_is_trivial(path: &Path) -> bool {
    struct Trivializer {
        prev_pt: Point,
        prev_vec: Point,
    }
    impl Trivializer {
        fn move_to(&mut self, curr_pt: Point) {
            self.prev_pt = curr_pt;
            self.prev_vec = Point::new(0.0, 0.0);
        }

        fn add_trivial_contour_point(&mut self, curr_pt: Point) -> bool {
            if curr_pt == self.prev_pt {
                return true;
            }
            let curr_vec = Point::new(curr_pt.x - self.prev_pt.x, curr_pt.y - self.prev_pt.y);
            // SkPoint::CrossProduct(prevVec, currVec).
            let cross = self.prev_vec.x * curr_vec.y - self.prev_vec.y * curr_vec.x;
            if cross != 0.0 {
                return false;
            }
            self.prev_vec = curr_vec;
            self.prev_pt = curr_pt;
            true
        }
    }
    let mut triv = Trivializer {
        prev_pt: Point::new(0.0, 0.0),
        prev_vec: Point::new(0.0, 0.0),
    };
    for (verb, points) in Iter::new(path, true) {
        match verb {
            Verb::Move => triv.move_to(points[0]),
            Verb::Cubic => {
                if !triv.add_trivial_contour_point(points[3]) {
                    return false;
                }
                if !triv.add_trivial_contour_point(points[2]) {
                    return false;
                }
                if !triv.add_trivial_contour_point(points[1]) {
                    return false;
                }
                if !triv.add_trivial_contour_point(points[0]) {
                    return false;
                }
            }
            Verb::Conic | Verb::Quad => {
                if !triv.add_trivial_contour_point(points[2]) {
                    return false;
                }
                if !triv.add_trivial_contour_point(points[1]) {
                    return false;
                }
                if !triv.add_trivial_contour_point(points[0]) {
                    return false;
                }
            }
            Verb::Line => {
                if !triv.add_trivial_contour_point(points[1]) {
                    return false;
                }
                if !triv.add_trivial_contour_point(points[0]) {
                    return false;
                }
            }
            Verb::Close | Verb::Done => {}
        }
    }
    true
}

/// `Simplify(path)`: returns the path with its self-intersections resolved, or `None` if the
/// path cannot be simplified.
// Port of: src/pathops/SkPathOpsSimplify.cpp#L166-L290 (chrome/m156)
#[must_use]
pub fn simplify(path: &Path) -> Option<Path> {
    let fill_type = if path.is_inverse_fill_type() {
        PathFillType::InverseEvenOdd
    } else {
        PathFillType::EvenOdd
    };
    if path.is_convex() {
        let mut result = Path::new();
        if !path_is_trivial(path) {
            result = path.clone();
        }
        return Some(result.with_fill_type(fill_type));
    }
    let mut state = OpState::new();
    let mut contour_list = state.contour_head();
    let coincidence = state.coin_new_set();
    let mut builder = EdgeBuilder::new(path, contour_list);
    if !builder.finish(&mut state) {
        return None;
    }
    if !state.sort_contour_list(&mut contour_list, false, false) {
        return Some(Path::new_with_fill_type(fill_type));
    }
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
    let mut writer = PathWriter::new(fill_type);
    // The builder's xor mask for the current operand decides which bridge applies.
    let winding = !builder.xor_mask();
    let bridged = if winding {
        bridge_winding(&mut state, contour_list, &mut writer)
    } else {
        bridge_xor(&mut state, contour_list, &mut writer)
    };
    if !bridged {
        return None;
    }
    // If some edges could not be resolved, assemble the remaining ones.
    state.writer_assemble(&mut writer);
    Some(writer.native_path())
}
