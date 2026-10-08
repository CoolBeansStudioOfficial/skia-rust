// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkOpBuilder.cpp (chrome/m156), include/pathops/SkPathOps.h
//! `OpBuilder` (`SkOpBuilder`): a series of path operations, optimized for unioning many paths.
//! Union-only inputs that are convex, or that do not overlap, are resolved without the op graph.

use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_enums::{PathFirstDirection, ResolveConvexity};
use skia_rust_core::path_priv;
use skia_rust_core::path_types::{PathFillType, PathVerb};
use skia_rust_core::rect::Rect;

use crate::op_edge_builder::EdgeBuilder;
use crate::op_op::op;
use crate::op_simplify::simplify;
use crate::op_state::{ContourId, OpPhase, OpState};
use crate::path_op::PathOp;
use crate::path_writer::PathWriter;

/// `one_contour(path)`: true if the path has a single contour (no `kMove` after its first verb).
// Port of: src/pathops/SkOpBuilder.cpp#L22-L35 (chrome/m156)
fn one_contour(path: &Path) -> bool {
    let Some(raw) = path_priv::raw(path, ResolveConvexity::No) else {
        return false;
    };
    !raw.verbs()
        .iter()
        .skip(1)
        .any(|&verb| verb == PathVerb::Move)
}

/// `SkOpContour::toPath(SkPathWriter*)` and `toReversePath`, on the contour `c` of `state`.
// Port of: src/pathops/SkOpContour.cpp#L13-L35 (chrome/m156)
fn contour_to_path(state: &mut OpState, c: ContourId, path: &mut PathWriter, reverse: bool) {
    if state.contour_count(c) == 0 {
        return;
    }
    if reverse {
        // const SkOpSegment* segment = fTail; do { addCurveTo(tail, head) } while (prev)
        let mut segment = state.contours[c.0].tail;
        while let Some(seg) = segment {
            let (start, end) = (state.seg_tail(seg), state.seg_head(seg));
            let ok = state.seg_add_curve_to(seg, start, end, path);
            debug_assert!(ok);
            segment = state.seg_prev(seg);
        }
    } else {
        // const SkOpSegment* segment = &fHead; do { addCurveTo(head, tail) } while (next)
        let mut segment = state.contour_first(c);
        while let Some(seg) = segment {
            let (start, end) = (state.seg_head(seg), state.seg_tail(seg));
            let ok = state.seg_add_curve_to(seg, start, end, path);
            debug_assert!(ok);
            segment = state.seg_next(seg);
        }
    }
    path.finish_contour(state);
    state.writer_assemble(path);
}

/// `SkOpBuilder`: a series of path operations, optimized for unioning many paths together.
// Port of: include/pathops/SkPathOps.h#L115-L146 (chrome/m156), src/pathops/SkOpBuilder.cpp
#[doc(alias = "SkOpBuilder")]
#[derive(Clone, Debug, Default)]
pub struct OpBuilder {
    path_refs: Vec<Path>,
    ops: Vec<PathOp>,
}

impl OpBuilder {
    /// An empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one or more paths and their operand. The builder is empty before the first path is
    /// added, so the result of a single add is (emptyPath OP path).
    // Port of: src/pathops/SkOpBuilder.cpp#L144-L151 (chrome/m156)
    pub fn add(&mut self, path: &Path, op: PathOp) {
        if self.ops.is_empty() && op != PathOp::Union {
            self.path_refs.push(Path::new());
            self.ops.push(PathOp::Union);
        }
        self.path_refs.push(path.clone());
        self.ops.push(op);
    }

    /// Computes the sum of all paths and operands, and resets the builder to its initial state.
    /// Returns `None` on failure.
    // Port of: src/pathops/SkOpBuilder.cpp#L153-L210 (chrome/m156)
    pub fn resolve(&mut self) -> Option<Path> {
        let count = self.ops.len();
        let mut all_union = true;
        let mut first_dir = PathFirstDirection::Unknown;
        for index in 0..count {
            if PathOp::Union != self.ops[index] || self.path_refs[index].is_inverse_fill_type() {
                all_union = false;
                break;
            }
            // If all paths are convex, track direction, reversing as needed.
            if self.path_refs[index].is_convex() {
                let dir = path_priv::compute_first_direction_path(&self.path_refs[index]);
                if dir == PathFirstDirection::Unknown {
                    all_union = false;
                    break;
                }
                if first_dir == PathFirstDirection::Unknown {
                    first_dir = dir;
                } else if first_dir != dir {
                    reverse_path(&mut self.path_refs[index]);
                }
                continue;
            }
            // If the path is not convex but its bounds do not intersect the others, simplify is
            // enough.
            let test_bounds = *self.path_refs[index].bounds();
            for inner in 0..index {
                // OPTIMIZE: check to see if the contour bounds do not intersect other contour
                // bounds?
                if Rect::intersects(self.path_refs[inner].bounds(), test_bounds) {
                    all_union = false;
                    break;
                }
            }
        }
        if !all_union {
            let mut result = self.path_refs[0].clone();
            for index in 1..count {
                if let Some(res) = op(&result, &self.path_refs[index], self.ops[index]) {
                    result = res;
                } else {
                    self.reset();
                    return None;
                }
            }
            self.reset();
            return Some(result);
        }
        let mut sum = PathBuilder::new();
        for index in 0..count {
            let Some(result) = simplify(&self.path_refs[index]) else {
                self.reset();
                return None;
            };
            self.path_refs[index] = result;
            if !self.path_refs[index].is_empty() {
                // convert the even odd result back to winding form before accumulating it
                if !fix_winding(&mut self.path_refs[index]) {
                    return None;
                }
                sum.add_path(&self.path_refs[index], None);
            }
        }
        self.reset();

        simplify(&sum.detach())
    }

    /// `SkOpBuilder::reset()`.
    // Port of: src/pathops/SkOpBuilder.cpp#L137-L140 (chrome/m156)
    fn reset(&mut self) {
        self.path_refs.clear();
        self.ops.clear();
    }
}

/// `SkOpBuilder::ReversePath(SkPath* path)`.
// Port of: src/pathops/SkOpBuilder.cpp#L40-L47 (chrome/m156)
fn reverse_path(path: &mut Path) {
    let Some(last_pt) = path.last_pt() else {
        debug_assert!(false, "ReversePath needs a last point");
        return;
    };
    let mut temp = PathBuilder::new();
    temp.move_to(last_pt);
    path_priv::reverse_path_to(&mut temp, path);
    temp.close();
    *path = temp.detach();
}

/// `SkOpBuilder::FixWinding(SkPath* path)`: rewrites the path so that its fill type applies to
/// winding-ordered contours. Returns false if the path cannot be parsed.
// Port of: src/pathops/SkOpBuilder.cpp#L49-L123 (chrome/m156)
fn fix_winding(path: &mut Path) -> bool {
    let mut fill_type = path.fill_type();
    if fill_type == PathFillType::InverseEvenOdd {
        fill_type = PathFillType::InverseWinding;
    } else if fill_type == PathFillType::EvenOdd {
        fill_type = PathFillType::Winding;
    }
    if one_contour(path) {
        let dir = path_priv::compute_first_direction_path(path);
        if dir != PathFirstDirection::Unknown {
            if dir == PathFirstDirection::CW {
                reverse_path(path);
            }
            *path = path.with_fill_type(fill_type);
            return true;
        }
    }
    let mut state = OpState::new();
    let contour_head = state.contour_head();
    let mut builder = EdgeBuilder::new(path, contour_head);
    if builder.unparseable() || !builder.finish(&mut state) {
        return false;
    }
    if state.contour_count(contour_head) == 0 {
        return true;
    }
    if state.contour_next(contour_head).is_none() {
        return false;
    }
    state.contour_join_all_segments(contour_head);
    state.contour_reset_reverse(contour_head);
    let mut write_path = false;
    state.set_phase(OpPhase::FixWinding);
    while let Some(top_span) = state.find_sortable_top(contour_head) {
        let top_segment = state.spans[top_span.0].segment;
        let top_contour = state.seg_contour(top_segment);
        debug_assert!(state.contour_is_ccw(top_contour) >= 0);
        if (state.nested() & 1) != i32::from(state.contour_is_ccw(top_contour) != 0) {
            state.contour_set_reverse(top_contour);
            write_path = true;
        }
        state.contour_mark_all_done(top_contour);
        state.clear_nested();
    }
    if !write_path {
        *path = path.with_fill_type(fill_type);
        return true;
    }

    let mut wound_path = PathWriter::new(fill_type);
    let mut test: ContourId = contour_head;
    // do { ... } while ((test = test->next()));
    loop {
        if state.contour_count(test) != 0 {
            let reversed = state.contour_reversed(test);
            contour_to_path(&mut state, test, &mut wound_path, reversed);
        }
        match state.contour_next(test) {
            Some(next) => test = next,
            None => break,
        }
    }
    *path = wound_path.native_path();
    true
}
