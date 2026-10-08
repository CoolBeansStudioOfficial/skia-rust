// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathWriter.h, src/pathops/SkPathWriter.cpp (chrome/m156)

//! Builds the output path of an operation, one contour at a time (`SkPathWriter`).
//!
//! A closed contour goes straight to the output. An open one is kept as a partial contour, and
//! [`OpState::writer_assemble`] joins the partials into closed contours, pairing their ends by
//! distance.
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

use skia_rust_core::path::{AddPathMode, Path};
use skia_rust_core::path_builder::{PathBuilder, Reserve};
use skia_rust_core::path_enums::ResolveConvexity;
use skia_rust_core::path_priv::{raw_builder, reverse_add_path, reverse_path_to};
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::t_sort::t_q_sort;

use crate::op_state::{OpState, PtTId, SegId, SpanId};
use crate::types::zero_or_one;

/// `SK_MaxS32`: the "no link" value of the assembly tables.
const SK_MAX_S32: i32 = i32::MAX;

/// `SkPathWriter`: builds the path one contour at a time.
// Port of: src/pathops/SkPathWriter.h#L19-L57 (chrome/m156)
#[doc(alias = "SkPathWriter")]
#[derive(Debug)]
pub(crate) struct PathWriter {
    /// `SkPathBuilder fBuilder`: the output.
    builder: PathBuilder,
    /// `SkPathBuilder fCurrent`: the contour under construction.
    current: PathBuilder,
    /// `TArray<SkPathBuilder> fPartials`: contours with mismatched starts and ends.
    partials: Vec<PathBuilder>,
    /// `TArray<const SkOpPtT*> fEndPtTs`: possible point values for partial starts and ends.
    end_ptts: Vec<PtTId>,
    /// `const SkOpPtT* fDefer[2]`: `[0]` is the deferred move, `[1]` the deferred line.
    defer: [Option<PtTId>; 2],
    /// `const SkOpPtT* fFirstPtT`: the first point of the current contour.
    first_ptt: Option<PtTId>,
}

impl PathWriter {
    /// `SkPathWriter(SkPathFillType ft)`.
    // Port of: src/pathops/SkPathWriter.cpp#L16-L18 (chrome/m156)
    #[must_use]
    pub(crate) fn new(fill_type: PathFillType) -> Self {
        let mut writer = Self {
            builder: PathBuilder::new_with_fill_type(fill_type),
            current: PathBuilder::new(),
            partials: Vec::new(),
            end_ptts: Vec::new(),
            defer: [None, None],
            first_ptt: None,
        };
        writer.init();
        writer
    }

    /// `SkPathWriter::init()`.
    // Port of: src/pathops/SkPathWriter.cpp#L168-L172 (chrome/m156)
    fn init(&mut self) {
        self.current.reset();
        self.first_ptt = None;
        self.defer = [None, None];
    }

    /// `SkPathWriter::matchedLast(test)`.
    // Port of: src/pathops/SkPathWriter.cpp#L134-L144 (chrome/m156)
    fn matched_last(&self, state: &OpState, test: Option<PtTId>) -> bool {
        if test == self.defer[1] {
            return true;
        }
        let Some(test) = test else {
            return false;
        };
        let Some(defer1) = self.defer[1] else {
            return false;
        };
        state.ptt_contains_ptt(test, defer1)
    }

    /// `SkPathWriter::changedSlopes(ptT)`.
    // Port of: src/pathops/SkPathWriter.cpp#L212-L222 (chrome/m156)
    fn changed_slopes(&self, state: &OpState, ptt: PtTId) -> bool {
        let (Some(defer0), Some(defer1)) = (self.defer[0], self.defer[1]) else {
            return false;
        };
        if self.matched_last(state, self.defer[0]) {
            return false;
        }
        let p0 = state.ptt_pt(defer0);
        let p1 = state.ptt_pt(defer1);
        let pt = state.ptt_pt(ptt);
        // SkVector arithmetic, in single precision.
        let defer_dx = p1.x - p0.x;
        let defer_dy = p1.y - p0.y;
        let line_dx = pt.x - p1.x;
        let line_dy = pt.y - p1.y;
        defer_dx * line_dy != defer_dy * line_dx
    }

    /// `SkPathWriter::lineTo()`.
    // Port of: src/pathops/SkPathWriter.cpp#L153-L160 (chrome/m156)
    fn line_to(&mut self, state: &OpState) {
        if self.current.is_empty() {
            self.move_to(state);
        }
        if let Some(defer1) = self.defer[1] {
            self.current.line_to(state.ptt_pt(defer1));
        }
    }

    /// `SkPathWriter::moveTo()`.
    // Port of: src/pathops/SkPathWriter.cpp#L162-L166 (chrome/m156)
    fn move_to(&mut self, state: &OpState) {
        if let Some(first) = self.first_ptt {
            self.current.move_to(state.ptt_pt(first));
        }
    }

    /// `SkPathWriter::finishContour()`.
    // Port of: src/pathops/SkPathWriter.cpp#L79-L97 (chrome/m156)
    fn finish_contour(&mut self, state: &OpState) {
        if !self.matched_last(state, self.defer[0]) {
            if self.defer[1].is_none() {
                return;
            }
            self.line_to(state);
        }
        if self.current.is_empty() {
            return;
        }
        if self.is_closed(state) {
            self.close();
        } else {
            if let Some(first) = self.first_ptt {
                self.end_ptts.push(first);
            }
            if let Some(defer1) = self.defer[1] {
                self.end_ptts.push(defer1);
            }
            let contour = std::mem::replace(&mut self.current, PathBuilder::new());
            self.partials.push(contour);
            self.init();
        }
    }

    /// `SkPathWriter::close()`: copies the closed contour to the output.
    // Port of: src/pathops/SkPathWriter.cpp#L40-L52 (chrome/m156)
    fn close(&mut self) {
        if self.current.is_empty() {
            return;
        }
        self.current.close();
        if let Some(raw) = raw_builder(&self.current, ResolveConvexity::No) {
            self.builder.add_raw(&raw, Reserve::Exact);
        }
        self.init();
    }

    /// `SkPathWriter::update(ptT)`: if the last point to be written matches the current path's
    /// first point, alter the last to avoid writing a degenerate `lineTo` when the path is closed.
    // Port of: src/pathops/SkPathWriter.cpp#L183-L199 (chrome/m156)
    fn update(&mut self, state: &OpState, ptt: PtTId) -> Point {
        if self.defer[1].is_none() {
            self.move_to(state);
        } else if !self.matched_last(state, self.defer[0]) {
            self.line_to(state);
        }
        let mut result = state.ptt_pt(ptt);
        if let Some(first) = self.first_ptt {
            let first_pt = state.ptt_pt(first);
            if result != first_pt && state.ptt_contains_ptt(first, ptt) {
                result = first_pt;
            }
        }
        // Both are set so that there is no pending deferred line.
        self.defer = [Some(ptt), Some(ptt)];
        result
    }

    /// `SkPathWriter::deferredMove(ptT)`.
    // Port of: src/pathops/SkPathWriter.cpp#L56-L66 (chrome/m156)
    pub(crate) fn deferred_move(&mut self, state: &OpState, ptt: PtTId) {
        if self.defer[1].is_none() {
            self.first_ptt = Some(ptt);
            self.defer[0] = Some(ptt);
            return;
        }
        if !self.matched_last(state, Some(ptt)) {
            self.finish_contour(state);
            self.first_ptt = Some(ptt);
            self.defer[0] = Some(ptt);
        }
    }

    /// `SkPathWriter::deferredLine(ptT)`: returns false if the line matches the last one.
    // Port of: src/pathops/SkPathWriter.cpp#L68-L92 (chrome/m156)
    pub(crate) fn deferred_line(&mut self, state: &OpState, ptt: PtTId) -> bool {
        if self.defer[0] == Some(ptt) {
            // Skia adds a degenerate line here; the caller should have checked.
            return true;
        }
        if let Some(defer0) = self.defer[0]
            && state.ptt_contains_ptt(ptt, defer0)
        {
            return true;
        }
        if self.matched_last(state, Some(ptt)) {
            return false;
        }
        if self.defer[1].is_some() && self.changed_slopes(state, ptt) {
            self.line_to(state);
            self.defer[0] = self.defer[1];
        }
        self.defer[1] = Some(ptt);
        true
    }

    /// `SkPathWriter::quadTo(pt1, ptT)`.
    // Port of: src/pathops/SkPathWriter.cpp#L240-L247 (chrome/m156)
    pub(crate) fn quad_to(&mut self, state: &OpState, pt1: Point, ptt: PtTId) {
        let pt2 = self.update(state, ptt);
        self.current.quad_to(pt1, pt2);
    }

    /// `SkPathWriter::conicTo(pt1, ptT, weight)`.
    // Port of: src/pathops/SkPathWriter.cpp#L219-L227 (chrome/m156)
    pub(crate) fn conic_to(&mut self, state: &OpState, pt1: Point, ptt: PtTId, weight: f32) {
        let pt2 = self.update(state, ptt);
        self.current.conic_to(pt1, pt2, weight);
    }

    /// `SkPathWriter::cubicTo(pt1, pt2, ptT)`.
    // Port of: src/pathops/SkPathWriter.cpp#L229-L238 (chrome/m156)
    pub(crate) fn cubic_to(&mut self, state: &OpState, pt1: Point, pt2: Point, ptt: PtTId) {
        let pt3 = self.update(state, ptt);
        self.current.cubic_to(pt1, pt2, pt3);
    }

    /// `SkPathWriter::finishContour()`, as called by `addCurveTo`'s callers.
    pub(crate) fn finish(&mut self, state: &OpState) {
        self.finish_contour(state);
    }

    /// `SkPathWriter::isClosed()`: the current contour ends where it starts.
    // Port of: src/pathops/SkPathWriter.cpp#L163-L165 (chrome/m156)
    /// `SkPathWriter::hasMove()`: no contour has been started yet.
    // Port of: src/pathops/SkPathWriter.h#L31 (chrome/m156)
    pub(crate) fn has_move(&self) -> bool {
        self.first_ptt.is_none()
    }

    pub(crate) fn is_closed(&self, state: &OpState) -> bool {
        self.matched_last(state, self.first_ptt)
    }

    /// `SkPathWriter::nativePath()`: the output path, detached from the writer.
    pub(crate) fn native_path(&mut self) -> Path {
        self.builder.detach()
    }

    /// `SkPathWriter::someAssemblyRequired()`.
    // Port of: src/pathops/SkPathWriter.cpp#L199-L203 (chrome/m156)
    fn some_assembly_required(&mut self, state: &OpState) -> bool {
        self.finish_contour(state);
        !self.end_ptts.is_empty()
    }

    /// `partWriter.fPartials`: the partial contours a writer holds.
    fn take_partials(&mut self) -> Vec<PathBuilder> {
        std::mem::take(&mut self.partials)
    }
}

impl OpState {
    /// `SkOpSegment::isSimple(end, step)`: the segment that continues a chain without a choice
    /// of next curve, or `None`.
    // Port of: src/pathops/SkOpSegment.h#L265-L267 (chrome/m156)
    pub(crate) fn seg_is_simple(
        &self,
        seg: SegId,
        start: &mut SpanId,
        step: &mut i32,
    ) -> Option<SegId> {
        self.next_chase(seg, start, step, None, None)
    }

    /// `SkPathWriter::assemble()`: joins the partial contours into closed contours, pairing the
    /// ends that are closest to each other.
    // Port of: src/pathops/SkPathWriter.cpp#L205-L440 (chrome/m156)
    pub(crate) fn writer_assemble(&mut self, writer: &mut PathWriter) {
        if !writer.some_assembly_required(self) {
            return;
        }
        // Limit the number of partial contours to avoid O(N^2) complexity and integer overflows.
        // 10,000 partial contours results in 20,000 ends and ~200,000,000 distance entries.
        const K_MAX_PARTIAL_CONTOURS: usize = 10_000;
        let end_count = writer.end_ptts.len();
        if end_count > K_MAX_PARTIAL_CONTOURS * 2 {
            return;
        }

        // Lengthen any partial contour adjacent to a simple segment.
        for p_index in 0..end_count {
            let mut op_ptt = writer.end_ptts[p_index];
            let mut part_writer = PathWriter::new(PathFillType::Winding);
            loop {
                let t = self.ptt_t(op_ptt);
                if !zero_or_one(t) {
                    break;
                }
                let op_span_base = self.ptt_span(op_ptt);
                let mut start = if t == 0.0 {
                    match self.span_next(self.span_up_cast(op_span_base)) {
                        Some(s) => s,
                        None => break,
                    }
                } else {
                    match self.span_prev(op_span_base) {
                        Some(s) => s,
                        None => break,
                    }
                };
                let mut step: i32 = if t == 0.0 { -1 } else { 1 };
                let op_segment = self.span_segment(op_span_base);
                let Some(next_segment) = self.seg_is_simple(op_segment, &mut start, &mut step)
                else {
                    break;
                };
                let op_span_end = if self.span_t(start) == 0.0 {
                    match self.span_next(self.span_up_cast(start)) {
                        Some(s) => s,
                        None => break,
                    }
                } else {
                    match self.span_prev(start) {
                        Some(s) => s,
                        None => break,
                    }
                };
                let starter = self.span_starter(start, op_span_end);
                if self.span_already_added(starter) {
                    break;
                }
                self.seg_add_curve_to(next_segment, start, op_span_end, &mut part_writer);
                op_ptt = self.span_ptt(op_span_end);
                writer.end_ptts[p_index] = op_ptt;
            }
            part_writer.finish(self);
            let part_partials = part_writer.take_partials();
            if part_partials.is_empty() {
                continue;
            }
            // If pIndex is even, reverse and prepend to fPartials; otherwise, append.
            let partial_index = p_index >> 1;
            let part: Path = part_partials[0].snapshot();
            if p_index & 1 != 0 {
                writer.partials[partial_index].add_path(&part, AddPathMode::Extend);
            } else {
                let mut reverse = PathBuilder::new();
                reverse_add_path(&mut reverse, &part);
                let partial_path = writer.partials[partial_index].detach();
                reverse.add_path(&partial_path, AddPathMode::Extend);
                writer.partials[partial_index] = reverse;
            }
        }

        let link_count = end_count / 2;
        let mut s_link = vec![SK_MAX_S32; link_count];
        let mut e_link = vec![SK_MAX_S32; link_count];
        // Folded triangle of distances between every pair of ends.
        let entries = end_count * (end_count - 1) / 2;
        let mut distances: Vec<f64> = Vec::with_capacity(entries);
        let mut dist_lookup: Vec<usize> = Vec::with_capacity(entries);
        let mut sorted_dist: Vec<i32> = Vec::with_capacity(entries);
        let mut r_row = 0_usize;
        let mut d_index = 0_usize;
        for r_index in 0..end_count.saturating_sub(1) {
            let o_pt = self.ptt_pt(writer.end_ptts[r_index]);
            for i_index in (r_index + 1)..end_count {
                let i_pt = self.ptt_pt(writer.end_ptts[i_index]);
                // The differences are single precision (SkScalar) before they widen to double.
                let dx = f64::from(i_pt.x - o_pt.x);
                let dy = f64::from(i_pt.y - o_pt.y);
                let dist = dx * dx + dy * dy;
                dist_lookup.push(r_row + i_index);
                distances.push(dist);
                sorted_dist.push(d_index as i32);
                d_index += 1;
            }
            r_row += end_count;
        }
        t_q_sort(&mut sorted_dist, |one: &i32, two: &i32| {
            distances[*one as usize] < distances[*two as usize]
        });
        let mut remaining = link_count;
        for &sorted in &sorted_dist {
            let pair = dist_lookup[sorted as usize];
            let row = pair / end_count;
            let col = pair - row * end_count;
            let ndx_one = row >> 1;
            let end_one = row & 1 != 0;
            if link_get(&s_link, &e_link, end_one, ndx_one) != SK_MAX_S32 {
                continue;
            }
            let ndx_two = col >> 1;
            let end_two = col & 1 != 0;
            if link_get(&s_link, &e_link, end_two, ndx_two) != SK_MAX_S32 {
                continue;
            }
            let flip = end_one == end_two;
            let value_one = if flip {
                !(ndx_two as i32)
            } else {
                ndx_two as i32
            };
            let value_two = if flip {
                !(ndx_one as i32)
            } else {
                ndx_one as i32
            };
            link_set(&mut s_link, &mut e_link, end_one, ndx_one, value_one);
            link_set(&mut s_link, &mut e_link, end_two, ndx_two, value_two);
            remaining -= 1;
            if remaining == 0 {
                break;
            }
        }

        // Walk the links, emitting each closed contour.
        let mut r_index = 0_usize;
        loop {
            let mut forward = true;
            let mut first = true;
            let s_index = s_link[r_index];
            s_link[r_index] = SK_MAX_S32;
            let mut e_index: i32;
            if s_index < 0 {
                let idx = (!s_index) as usize;
                e_index = s_link[idx];
                s_link[idx] = SK_MAX_S32;
            } else {
                e_index = e_link[s_index as usize];
                e_link[s_index as usize] = SK_MAX_S32;
            }
            loop {
                let contour = writer.partials[r_index].snapshot();
                if !first {
                    if writer.builder.get_last_pt().is_none() {
                        return;
                    }
                    // Skia compares the prior point with the contour's next point here, and the
                    // comparison has no effect. An empty backward contour is an error.
                    if !forward && contour.points().is_empty() {
                        return;
                    }
                }
                if forward {
                    let mode = if first {
                        AddPathMode::Append
                    } else {
                        AddPathMode::Extend
                    };
                    writer.builder.add_path(&contour, mode);
                } else {
                    reverse_path_to(&mut writer.builder, &contour);
                }
                first = false;
                let r = r_index as i32;
                let close_value = if (r != e_index) ^ forward {
                    e_index
                } else {
                    !e_index
                };
                if s_index == close_value {
                    writer.builder.close();
                    break;
                }
                if forward {
                    e_index = e_link[r_index];
                    e_link[r_index] = SK_MAX_S32;
                    if e_index >= 0 {
                        s_link[e_index as usize] = SK_MAX_S32;
                    } else {
                        e_link[(!e_index) as usize] = SK_MAX_S32;
                    }
                } else {
                    e_index = s_link[r_index];
                    s_link[r_index] = SK_MAX_S32;
                    if e_index >= 0 {
                        e_link[e_index as usize] = SK_MAX_S32;
                    } else {
                        s_link[(!e_index) as usize] = SK_MAX_S32;
                    }
                }
                let mut next_r = e_index;
                if next_r < 0 {
                    forward = !forward;
                    next_r = !next_r;
                }
                r_index = next_r as usize;
            }
            match s_link.iter().position(|&link| link != SK_MAX_S32) {
                Some(next) => r_index = next,
                None => break,
            }
        }
    }
}

/// `linkOne[ndx]`, where `linkOne` is `eLink` when `end` is set and `sLink` otherwise.
fn link_get(s_link: &[i32], e_link: &[i32], end: bool, ndx: usize) -> i32 {
    if end { e_link[ndx] } else { s_link[ndx] }
}

/// Sets `linkOne[ndx]` (see [`link_get`]).
fn link_set(s_link: &mut [i32], e_link: &mut [i32], end: bool, ndx: usize, value: i32) {
    if end {
        e_link[ndx] = value;
    } else {
        s_link[ndx] = value;
    }
}
