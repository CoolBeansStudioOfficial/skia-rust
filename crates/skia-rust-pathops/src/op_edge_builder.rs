// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkOpEdgeBuilder.h, src/pathops/SkOpEdgeBuilder.cpp (chrome/m156)

//! Turns a path into the contours of the op graph (`SkOpEdgeBuilder`).
//!
//! `preFetch` copies the path into flat point and verb arrays, dropping degenerate verbs and
//! closing open contours. `walk` then splits curves that need it (at maximum curvature, or at
//! complex cubic breaks) and adds each edge to the contour it belongs to.
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

use skia_rust_core::geometry::{Conic, chop_quad_at_max_curvature, find_quad_max_curvature};
use skia_rust_core::path::{Path, Verb};
use skia_rust_core::path_priv::iterate;
use skia_rust_core::path_types::{PathFillType, PathVerb};
use skia_rust_core::point::Point;
use skia_rust_core::t_sort::t_q_sort;

use crate::cubic::DCubic;
use crate::op_contour::ContourBuilder;
use crate::op_state::{ContourId, OpState};
use crate::point::DPoint;
use crate::reduce_order::ReduceOrder;
use crate::types::FLT_EPSILON_ORDERABLE_ERR;

/// `SkPathOpsVerbToPoints(verb)`: the points a verb adds after its start point.
// Port of: src/pathops/SkPathOpsTypes.h (SkPathOpsVerbToPoints, chrome/m156)
fn verb_to_points(verb: Verb) -> usize {
    match verb {
        Verb::Line => 1,
        Verb::Quad | Verb::Conic => 2,
        Verb::Cubic => 3,
        _ => 0,
    }
}

/// Maps the iterator's verb to the path verb (`SkPath::Verb`).
fn to_verb(verb: PathVerb) -> Verb {
    match verb {
        PathVerb::Move => Verb::Move,
        PathVerb::Line => Verb::Line,
        PathVerb::Quad => Verb::Quad,
        PathVerb::Conic => Verb::Conic,
        PathVerb::Cubic => Verb::Cubic,
        PathVerb::Close => Verb::Close,
    }
}

/// `force_small_to_zero(pt)`: very tiny coordinates cause numerical instability.
// Port of: src/pathops/SkOpEdgeBuilder.cpp#L30-L40 (chrome/m156)
fn force_small_to_zero(pt: Point) -> Point {
    let mut ret = pt;
    if f64::from(ret.x.abs()) < FLT_EPSILON_ORDERABLE_ERR {
        ret.x = 0.0;
    }
    if f64::from(ret.y.abs()) < FLT_EPSILON_ORDERABLE_ERR {
        ret.y = 0.0;
    }
    ret
}

/// `can_add_curve(verb, curve)`: forces the small coordinates to zero and returns whether the
/// curve is worth adding.
// Port of: src/pathops/SkOpEdgeBuilder.cpp#L42-L52 (chrome/m156)
fn can_add_curve(verb: Verb, curve: &mut [Point]) -> bool {
    if verb == Verb::Move {
        return false;
    }
    for point in curve.iter_mut().take(verb_to_points(verb) + 1) {
        *point = force_small_to_zero(*point);
    }
    verb != Verb::Line || !DPoint::approximately_equal_points(curve[0], curve[1])
}

/// `SkPath::FillType & 1`: whether the fill is even-odd (or its inverse).
fn is_even_odd(fill: PathFillType) -> bool {
    (fill as i32 & 1) != 0
}

/// `SkDVector::dot`, in single precision as `SkVector` is.
fn dot(a: Point, b: Point) -> f32 {
    a.x * b.x + a.y * b.y
}

/// `SkOpEdgeBuilder`: builds the contours of one or two paths.
// Port of: src/pathops/SkOpEdgeBuilder.h#L14-L60 (chrome/m156)
#[doc(alias = "SkOpEdgeBuilder")]
#[derive(Debug)]
pub(crate) struct EdgeBuilder<'a> {
    /// `const SkPath* fPath`.
    path: &'a Path,
    /// `SkTDArray<SkPoint> fPathPts`.
    path_pts: Vec<Point>,
    /// `SkTDArray<SkScalar> fWeights`.
    weights: Vec<f32>,
    /// `SkTDArray<uint8_t> fPathVerbs`: ends with `kDone`.
    path_verbs: Vec<Verb>,
    /// `SkOpContourBuilder fContourBuilder`.
    contour_builder: ContourBuilder,
    /// `SkOpContourHead* fContoursHead`.
    contours_head: ContourId,
    /// `fXorMask[2]`: `true` where Skia's mask is `kEvenOdd_PathOpsMask`.
    xor_mask: [bool; 2],
    /// `int fSecondHalf`: the index of the first path's `kDone` verb.
    second_half: usize,
    /// `bool fOperand`.
    operand: bool,
    /// `bool fAllowOpenContours`.
    allow_open_contours: bool,
    /// `bool fUnparseable`.
    unparseable: bool,
}

impl<'a> EdgeBuilder<'a> {
    /// `SkOpEdgeBuilder(path, contours2, globalState)`, followed by `init()`.
    // Port of: src/pathops/SkOpEdgeBuilder.h#L16-L24 (chrome/m156)
    #[must_use]
    pub(crate) fn new(path: &'a Path, contours_head: ContourId) -> Self {
        let mut builder = Self {
            path,
            path_pts: Vec::new(),
            weights: Vec::new(),
            path_verbs: Vec::new(),
            contour_builder: ContourBuilder::new(Some(contours_head)),
            contours_head,
            xor_mask: [false; 2],
            second_half: 0,
            operand: false,
            allow_open_contours: false,
            unparseable: false,
        };
        builder.init();
        builder
    }

    /// `SkOpEdgeBuilder::init()`.
    // Port of: src/pathops/SkOpEdgeBuilder.cpp#L80-L87 (chrome/m156)
    fn init(&mut self) {
        self.operand = false;
        let even_odd = is_even_odd(self.path.fill_type());
        self.xor_mask = [even_odd, even_odd];
        self.unparseable = false;
        self.second_half = self.pre_fetch();
    }

    /// `SkOpEdgeBuilder::addOperand(path)`: adds the second operand's verbs and points.
    // Port of: src/pathops/SkOpEdgeBuilder.cpp#L107-L115 (chrome/m156)
    pub(crate) fn add_operand(&mut self, path: &'a Path) {
        self.path_verbs.pop();
        self.path = path;
        self.xor_mask[1] = is_even_odd(path.fill_type());
        self.pre_fetch();
    }

    /// `SkOpEdgeBuilder::complete()`: completes the current contour.
    // Port of: src/pathops/SkOpEdgeBuilder.h#L28-L37 (chrome/m156)
    pub(crate) fn complete(&mut self, state: &mut OpState) {
        self.contour_builder.flush(state);
        if let Some(contour) = self.contour_builder.contour()
            && state.contour_count(contour) != 0
        {
            state.contour_complete(contour);
            self.contour_builder.set_contour(state, None);
        }
    }

    /// `SkOpEdgeBuilder::finish()`: walks the verbs and completes the contours.
    // Port of: src/pathops/SkOpEdgeBuilder.cpp#L117-L128 (chrome/m156)
    pub(crate) fn finish(&mut self, state: &mut OpState) -> bool {
        self.operand = false;
        if self.unparseable || !self.walk(state) {
            return false;
        }
        self.complete(state);
        if let Some(contour) = self.contour_builder.contour()
            && state.contour_count(contour) == 0
        {
            state.contour_head_remove(self.contours_head, contour);
        }
        true
    }

    /// `SkOpEdgeBuilder::xorMask()`: the mask of the operand being built.
    #[must_use]
    pub(crate) fn xor_mask(&self) -> bool {
        self.xor_mask[usize::from(self.operand)]
    }

    /// `SkOpEdgeBuilder::unparseable()`.
    #[must_use]
    pub(crate) fn unparseable(&self) -> bool {
        self.unparseable
    }

    /// `SkOpEdgeBuilder::closeContour(curveEnd, curveStart)`.
    // Port of: src/pathops/SkOpEdgeBuilder.cpp#L130-L147 (chrome/m156)
    fn close_contour(&mut self, curve_end: Point, curve_start: Point) {
        if DPoint::approximately_equal_points(curve_end, curve_start) {
            let verb_count = self.path_verbs.len();
            let pts_count = self.path_pts.len();
            if self.path_verbs[verb_count - 1] == Verb::Line
                && self.path_pts[pts_count - 2] == curve_start
            {
                self.path_verbs.pop();
                self.path_pts.pop();
            } else {
                self.path_pts[pts_count - 1] = curve_start;
            }
        } else {
            self.path_verbs.push(Verb::Line);
            self.path_pts.push(curve_start);
        }
        self.path_verbs.push(Verb::Close);
    }

    /// `SkOpEdgeBuilder::preFetch()`: copies the path into flat arrays. Returns the index of the
    /// `kDone` verb that ends the first path.
    // Port of: src/pathops/SkOpEdgeBuilder.cpp#L149-L232 (chrome/m156)
    fn pre_fetch(&mut self) -> usize {
        if !self.path.is_finite() {
            self.unparseable = true;
            return 0;
        }
        let mut curve_start = Point::default();
        let mut curve = [Point::default(); 4];
        let mut last_curve = false;
        let path = self.path;
        for (path_verb, pts, weight) in iterate(path) {
            let mut verb = to_verb(path_verb);
            match verb {
                Verb::Move => {
                    if !self.allow_open_contours && last_curve {
                        self.close_contour(curve[0], curve_start);
                    }
                    self.path_verbs.push(verb);
                    curve[0] = force_small_to_zero(pts[0]);
                    self.path_pts.push(curve[0]);
                    curve_start = curve[0];
                    last_curve = false;
                    continue;
                }
                Verb::Line => {
                    curve[1] = force_small_to_zero(pts[1]);
                    if DPoint::approximately_equal_points(curve[0], curve[1]) {
                        let last_verb = *self.path_verbs.last().expect("a move precedes a line");
                        if last_verb != Verb::Line && last_verb != Verb::Move {
                            let last = self.path_pts.len() - 1;
                            curve[0] = curve[1];
                            self.path_pts[last] = curve[0];
                        }
                        // Skip degenerate points.
                        continue;
                    }
                }
                Verb::Quad => {
                    curve[1] = force_small_to_zero(pts[1]);
                    curve[2] = force_small_to_zero(pts[2]);
                    let input = [curve[0], curve[1], curve[2]];
                    verb = ReduceOrder::quad_verb(input, &mut curve);
                    if verb == Verb::Move {
                        // Skip degenerate points.
                        continue;
                    }
                }
                Verb::Conic => {
                    curve[1] = force_small_to_zero(pts[1]);
                    curve[2] = force_small_to_zero(pts[2]);
                    let input = [curve[0], curve[1], curve[2]];
                    verb = ReduceOrder::quad_verb(input, &mut curve);
                    let w = weight.unwrap_or(1.0);
                    if verb == Verb::Quad && w != 1.0 {
                        verb = Verb::Conic;
                    } else if verb == Verb::Move {
                        // Skip degenerate points.
                        continue;
                    }
                }
                Verb::Cubic => {
                    curve[1] = force_small_to_zero(pts[1]);
                    curve[2] = force_small_to_zero(pts[2]);
                    curve[3] = force_small_to_zero(pts[3]);
                    let input = [curve[0], curve[1], curve[2], curve[3]];
                    verb = ReduceOrder::cubic_verb(input, &mut curve);
                    if verb == Verb::Move {
                        // Skip degenerate points.
                        continue;
                    }
                }
                Verb::Close => {
                    self.close_contour(curve[0], curve_start);
                    last_curve = false;
                    continue;
                }
                Verb::Done => continue,
            }
            self.path_verbs.push(verb);
            let pt_count = verb_to_points(verb);
            self.path_pts.extend_from_slice(&curve[1..=pt_count]);
            if verb == Verb::Conic {
                self.weights.push(weight.unwrap_or(1.0));
            }
            curve[0] = curve[pt_count];
            last_curve = true;
        }
        if !self.allow_open_contours && last_curve {
            self.close_contour(curve[0], curve_start);
        }
        self.path_verbs.push(Verb::Done);
        self.path_verbs.len() - 1
    }

    /// `SkOpEdgeBuilder::walk()`: adds every verb to the contours.
    // Port of: src/pathops/SkOpEdgeBuilder.cpp#L234-L345 (chrome/m156)
    fn walk(&mut self, state: &mut OpState) -> bool {
        let end_of_first_half = self.second_half;
        let mut verb_index = 0_usize;
        let mut points_index = 0_usize;
        let mut weight_index = 0_usize;
        let mut contour = self.contour_builder.contour();
        let mut move_to_ptr_bump = 0_usize;
        loop {
            let verb = self.path_verbs[verb_index];
            if verb == Verb::Done {
                break;
            }
            if verb_index == end_of_first_half {
                self.operand = true;
            }
            verb_index += 1;
            match verb {
                Verb::Move => {
                    if let Some(c) = contour
                        && state.contour_count(c) != 0
                    {
                        if self.allow_open_contours {
                            self.complete(state);
                        } else if !self.close(state) {
                            return false;
                        }
                    }
                    if contour.is_none() {
                        let created = state.contour_append_contour(self.contours_head);
                        self.contour_builder.set_contour(state, Some(created));
                        contour = Some(created);
                    }
                    let c = contour.expect("a move starts a contour");
                    state.contour_init(c, self.operand, self.xor_mask[usize::from(self.operand)]);
                    points_index += move_to_ptr_bump;
                    move_to_ptr_bump = 1;
                    continue;
                }
                Verb::Line => {
                    let pts = [self.path_pts[points_index], self.path_pts[points_index + 1]];
                    self.contour_builder.add_line(state, pts);
                }
                Verb::Quad => {
                    if !self.walk_quad(state, points_index) {
                        return false;
                    }
                }
                Verb::Conic => {
                    let weight = self.weights[weight_index];
                    weight_index += 1;
                    self.walk_conic(state, points_index, weight);
                }
                Verb::Cubic => {
                    if !self.walk_cubic(state, points_index) {
                        return false;
                    }
                }
                Verb::Close => {
                    if !self.close(state) {
                        return false;
                    }
                    contour = None;
                    continue;
                }
                // `kDone` ends the loop above, and `preFetch` emits no other verbs.
                Verb::Done => return false,
            }
            points_index += verb_to_points(verb);
        }
        self.contour_builder.flush(state);
        if let Some(c) = contour
            && state.contour_count(c) != 0
            && !self.allow_open_contours
            && !self.close(state)
        {
            return false;
        }
        true
    }

    /// `SkOpEdgeBuilder::close()`.
    // Port of: src/pathops/SkOpEdgeBuilder.cpp#L234-L238 (chrome/m156)
    fn close(&mut self, state: &mut OpState) -> bool {
        self.complete(state);
        true
    }

    /// The quad case of `walk`: splits a quad whose control polygon turns back at its maximum
    /// curvature, when both halves are usable.
    // Port of: src/pathops/SkOpEdgeBuilder.cpp#L250-L281 (chrome/m156)
    fn walk_quad(&mut self, state: &mut OpState, points_index: usize) -> bool {
        let pts = [
            self.path_pts[points_index],
            self.path_pts[points_index + 1],
            self.path_pts[points_index + 2],
        ];
        let vec1 = Point::new(pts[1].x - pts[0].x, pts[1].y - pts[0].y);
        let vec2 = Point::new(pts[2].x - pts[1].x, pts[2].y - pts[1].y);
        if dot(vec1, vec2) < 0.0 {
            let mut pair = [Point::default(); 5];
            if chop_quad_at_max_curvature(&pts, &mut pair) != 1 {
                if !pair.iter().all(|p| p.x.is_finite() && p.y.is_finite()) {
                    return false;
                }
                for point in &mut pair {
                    *point = force_small_to_zero(*point);
                }
                // The two halves share pair[2]. Each half is reduced and checked from its own
                // copy, which holds the same values Skia reads through the shared array.
                let mut storage1 = [Point::default(); 3];
                let mut storage2 = [Point::default(); 3];
                let first_half = [pair[0], pair[1], pair[2]];
                let second_half = [pair[2], pair[3], pair[4]];
                let v1 = ReduceOrder::quad_verb(first_half, &mut storage1);
                let v2 = ReduceOrder::quad_verb(second_half, &mut storage2);
                let mut curve1 = if v1 == Verb::Line {
                    storage1
                } else {
                    first_half
                };
                let mut curve2 = if v2 == Verb::Line {
                    storage2
                } else {
                    second_half
                };
                if can_add_curve(v1, &mut curve1) && can_add_curve(v2, &mut curve2) {
                    self.contour_builder.add_curve(state, v1, &curve1, 1.0);
                    self.contour_builder.add_curve(state, v2, &curve2, 1.0);
                    return true;
                }
            }
        }
        self.contour_builder.add_quad(state, pts);
        true
    }

    /// The conic case of `walk`: splits a conic at its maximum curvature when that helps.
    // Port of: src/pathops/SkOpEdgeBuilder.cpp#L282-L315 (chrome/m156)
    fn walk_conic(&mut self, state: &mut OpState, points_index: usize, weight: f32) {
        let pts = [
            self.path_pts[points_index],
            self.path_pts[points_index + 1],
            self.path_pts[points_index + 2],
        ];
        let vec1 = Point::new(pts[1].x - pts[0].x, pts[1].y - pts[0].y);
        let vec2 = Point::new(pts[2].x - pts[1].x, pts[2].y - pts[1].y);
        if dot(vec1, vec2) < 0.0 {
            // FIXME in Skia: max curvature for conics hasn't been implemented; use placeholder.
            let max_curvature = find_quad_max_curvature(&pts);
            if 0.0 < max_curvature && max_curvature < 1.0 {
                let conic = Conic::new(pts[0], pts[1], pts[2], weight);
                let mut pair = [Conic::default(); 2];
                if !conic.chop_at(max_curvature, &mut pair) {
                    // If the result can't be computed, use the original.
                    self.contour_builder.add_conic(state, pts, weight);
                    return;
                }
                let mut storage1 = [Point::default(); 3];
                let mut storage2 = [Point::default(); 3];
                let v1 = ReduceOrder::conic_verb(pair[0].pts, pair[0].w, &mut storage1);
                let v2 = ReduceOrder::conic_verb(pair[1].pts, pair[1].w, &mut storage2);
                let mut curve1 = if v1 == Verb::Line {
                    storage1
                } else {
                    pair[0].pts
                };
                let mut curve2 = if v2 == Verb::Line {
                    storage2
                } else {
                    pair[1].pts
                };
                if can_add_curve(v1, &mut curve1) && can_add_curve(v2, &mut curve2) {
                    self.contour_builder
                        .add_curve(state, v1, &curve1, pair[0].w);
                    self.contour_builder
                        .add_curve(state, v2, &curve2, pair[1].w);
                    return;
                }
            }
        }
        self.contour_builder.add_conic(state, pts, weight);
    }

    /// The cubic case of `walk`: splits complex cubics in two before adding them.
    // Port of: src/pathops/SkOpEdgeBuilder.cpp#L316-L372 (chrome/m156)
    fn walk_cubic(&mut self, state: &mut OpState, points_index: usize) -> bool {
        let pts = [
            self.path_pts[points_index],
            self.path_pts[points_index + 1],
            self.path_pts[points_index + 2],
            self.path_pts[points_index + 3],
        ];
        // Split complex cubics (such as self-intersecting curves or ones with difficult
        // curvature) in two before proceeding. This can be required for intersection to succeed.
        let mut split_t = [0.0_f32; 3];
        let breaks = DCubic::complex_break(pts, &mut split_t);
        if breaks == 0 {
            self.contour_builder.add_cubic(state, pts);
            return true;
        }
        let breaks = breaks as usize;
        #[derive(Clone, Copy)]
        struct Splitsville {
            t: [f64; 2],
            pts: [Point; 4],
            reduced: [Point; 4],
            verb: Verb,
            can_add: bool,
        }
        let empty = Splitsville {
            t: [0.0; 2],
            pts: [Point::default(); 4],
            reduced: [Point::default(); 4],
            verb: Verb::Move,
            can_add: false,
        };
        let mut splits = [empty; 4];
        t_q_sort(&mut split_t[..breaks], |a: &f32, b: &f32| a < b);
        for index in 0..=breaks {
            let split = &mut splits[index];
            split.t[0] = if index > 0 {
                f64::from(split_t[index - 1])
            } else {
                0.0
            };
            split.t[1] = if index < breaks {
                f64::from(split_t[index])
            } else {
                1.0
            };
            let mut cubic = DCubic::default();
            cubic.set(pts);
            let part = cubic.sub_divide(split.t[0], split.t[1]);
            let Some(float_pts) = part.to_float_points() else {
                return false;
            };
            split.pts = float_pts;
            split.verb = ReduceOrder::cubic_verb(split.pts, &mut split.reduced);
            split.can_add = if split.verb == Verb::Cubic {
                can_add_curve(split.verb, &mut split.pts)
            } else {
                can_add_curve(split.verb, &mut split.reduced)
            };
        }
        for index in 0..=breaks {
            if !splits[index].can_add {
                continue;
            }
            let mut prior = index;
            while prior > 0 && !splits[prior - 1].can_add {
                prior -= 1;
            }
            if prior < index {
                splits[index].t[0] = splits[prior].t[0];
                splits[index].pts[0] = splits[prior].pts[0];
            }
            let mut next = index;
            let break_limit = breaks.min(splits.len() - 1);
            while next < break_limit && !splits[next + 1].can_add {
                next += 1;
            }
            if next > index {
                splits[index].t[1] = splits[next].t[1];
                splits[index].pts[3] = splits[next].pts[3];
            }
            if prior < index || next > index {
                let split = &mut splits[index];
                split.verb = ReduceOrder::cubic_verb(split.pts, &mut split.reduced);
            }
            let split = &mut splits[index];
            let mut curve = if split.verb == Verb::Cubic {
                split.pts
            } else {
                split.reduced
            };
            if !can_add_curve(split.verb, &mut curve) {
                return false;
            }
            let verb = split.verb;
            self.contour_builder.add_curve(state, verb, &curve, 1.0);
        }
        true
    }
}
