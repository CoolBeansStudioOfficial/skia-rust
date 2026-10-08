// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsAsWinding.cpp (chrome/m156)
// Skia compares SkScalar values exactly (no epsilon) in AsWinding, and the comparisons below keep it.
#![allow(clippy::float_cmp)]

//! `AsWinding(path)`: rewrites a path that uses the even-odd fill rule into an equivalent
//! winding-filled path, by reversing the contours that are nested at an odd depth.

use skia_rust_core::path::{Iter, Path, Verb};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_priv;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{Contains, Rect};

use crate::conic::DConic;
use crate::cubic::DCubic;
use crate::line::DLine;
use crate::op_winding::curve_intercept;
use crate::quad::DQuad;
use crate::types::zero_or_one;

/// `Contour::Direction`: `kCCW = -1`, `kNone`, `kCW`. `SkPathDirection` has no 'none' state.
// Port of: src/pathops/SkPathOpsAsWinding.cpp#L37-L41 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Direction {
    Ccw = -1,
    None = 0,
    Cw = 1,
}

impl Direction {
    /// `-(int) direction`, as `reverseMarkedContours` flips a contour's direction.
    fn negated(self) -> Self {
        match self {
            Self::Ccw => Self::Cw,
            Self::None => Self::None,
            Self::Cw => Self::Ccw,
        }
    }
}

/// `struct Contour`: a contour's bounds, its verb range, and its children in the bounds tree.
/// Contours are stored in an arena; index 0 is the root (`Contour sorted(SkRect(), 0, 0)`).
// Port of: src/pathops/SkPathOpsAsWinding.cpp#L37-L59 (chrome/m156)
#[derive(Clone, Debug)]
struct Contour {
    children: Vec<usize>,
    bounds: Rect,
    min_xy: Point,
    verb_start: usize,
    verb_end: usize,
    direction: Direction,
    contained: bool,
    reverse: bool,
}

impl Contour {
    // Port of: src/pathops/SkPathOpsAsWinding.cpp#L44-L50 (chrome/m156)
    fn new(bounds: Rect, verb_start: usize, verb_end: usize) -> Self {
        Self {
            children: Vec::new(),
            bounds,
            min_xy: Point::new(f32::MAX, f32::MAX),
            verb_start,
            verb_end,
            direction: Direction::None,
            contained: false,
            reverse: false,
        }
    }
}

/// `VerbPtCount(verb)`: the number of points a verb adds (`{1, 1, 2, 2, 3, 0}`).
// Port of: src/pathops/SkPathOpsAsWinding.cpp#L61-L66 (chrome/m156)
fn verb_pt_count(verb: Verb) -> usize {
    match verb {
        Verb::Move | Verb::Line => 1,
        Verb::Quad | Verb::Conic => 2,
        Verb::Cubic => 3,
        Verb::Close | Verb::Done => 0,
    }
}

/// `VerbPtIndex(verb)`: the index of the first point a verb adds (`{0, 1, 1, 1, 1, 0}`).
// Port of: src/pathops/SkPathOpsAsWinding.cpp#L68-L73 (chrome/m156)
fn verb_pt_index(verb: Verb) -> usize {
    match verb {
        Verb::Line | Verb::Quad | Verb::Conic | Verb::Cubic => 1,
        Verb::Move | Verb::Close | Verb::Done => 0,
    }
}

/// `Line <= verb && verb <= Cubic`: the verbs that add curve segments.
fn is_curve(verb: Verb) -> bool {
    matches!(verb, Verb::Line | Verb::Quad | Verb::Conic | Verb::Cubic)
}

/// `to_direction(dy)`.
// Port of: src/pathops/SkPathOpsAsWinding.cpp#L75-L78 (chrome/m156)
fn to_direction(dy: f32) -> Direction {
    if dy > 0.0 {
        Direction::Ccw
    } else if dy < 0.0 {
        Direction::Cw
    } else {
        Direction::None
    }
}

/// `CurvePointAtT[verb](pts, weight, t)`: the point of the curve at `t`, as a float point.
// Port of: src/pathops/SkPathOpsCurve.h#L113-L181 (chrome/m156)
fn curve_point_at_t(verb: Verb, pts: &[Point], weight: f32, t: f64) -> Point {
    match verb {
        Verb::Line => {
            let mut line = DLine::default();
            line.set([pts[0], pts[1]]);
            line.pt_at_t(t).as_sk_point()
        }
        Verb::Quad => {
            let mut quad = DQuad::default();
            quad.set([pts[0], pts[1], pts[2]]);
            quad.pt_at_t(t).as_sk_point()
        }
        Verb::Conic => {
            let mut conic = DConic::default();
            conic.set([pts[0], pts[1], pts[2]], weight);
            conic.pt_at_t(t).as_sk_point()
        }
        _ => {
            let mut cubic = DCubic::default();
            cubic.set([pts[0], pts[1], pts[2], pts[3]]);
            cubic.pt_at_t(t).as_sk_point()
        }
    }
}

/// `CurveSlopeAtT[verb](pts, weight, t)`: the derivative of the curve at `t`, as a float vector.
// Port of: src/pathops/SkPathOpsCurve.h#L262-L300 (chrome/m156)
fn curve_slope_at_t(verb: Verb, pts: &[Point], weight: f32, t: f64) -> Point {
    match verb {
        Verb::Line => pts[1] - pts[0],
        Verb::Quad => {
            let mut quad = DQuad::default();
            quad.set([pts[0], pts[1], pts[2]]);
            vector_to_scalar(quad.dxdy_at_t(t))
        }
        Verb::Conic => {
            let mut conic = DConic::default();
            conic.set([pts[0], pts[1], pts[2]], weight);
            vector_to_scalar(conic.dxdy_at_t(t))
        }
        _ => {
            let mut cubic = DCubic::default();
            cubic.set([pts[0], pts[1], pts[2], pts[3]]);
            vector_to_scalar(cubic.dxdy_at_t(t))
        }
    }
}

/// `SkDVector::asSkVector()`: the double components rounded to `SkScalar`.
#[allow(clippy::cast_possible_truncation)] // SkDoubleToScalar: the (float) cast of asSkVector
fn vector_to_scalar(v: crate::point::DVector) -> Point {
    Point::new(v.x as f32, v.y as f32)
}

/// `contains_edge(pts, verb, weight, edge)`: the winding of the curve about the ray from `edge`
/// to the right.
// Port of: src/pathops/SkPathOpsAsWinding.cpp#L80-L126 (chrome/m156)
fn contains_edge(pts: &[Point], verb: Verb, weight: f32, edge: Point) -> i32 {
    let bounds = Rect::bounds_or_empty(&pts[..=verb_pt_count(verb)]);
    if bounds.top > edge.y {
        return 0;
    }
    if bounds.bottom <= edge.y {
        // check to see if y is at line end to avoid double counting
        return 0;
    }
    if bounds.left >= edge.x {
        return 0;
    }
    let mut t_vals = [0.0_f64; 3];
    // must intersect horz ray with curve in case it intersects more than once
    let mut count = curve_intercept(verb, true, pts, weight, edge.y, &mut t_vals);
    // remove results to the right of edge
    let mut index = 0;
    while index < count {
        let intersect_x = curve_point_at_t(verb, pts, weight, t_vals[index]).x;
        if intersect_x < edge.x {
            index += 1;
            continue;
        }
        if intersect_x > edge.x {
            count -= 1;
            t_vals[index] = t_vals[count];
            continue;
        }
        // if intersect x equals edge x, we need to determine if pts is to the left or right of
        // edge
        if pts[0].x < edge.x && pts[verb_pt_count(verb)].x < edge.x {
            index += 1;
            continue;
        }
        // TODO : other cases need discriminating. need op angle code to figure it out
        count -= 1;
        t_vals[index] = t_vals[count];
    }
    // use first derivative to determine if intersection is contributing +1 or -1 to winding
    let mut directions = [Direction::None; 3];
    for index in 0..count {
        directions[index] = to_direction(curve_slope_at_t(verb, pts, weight, t_vals[index]).y);
    }
    let mut winding = 0;
    for index in 0..count {
        // skip intersections that end at edge and go up
        if zero_or_one(t_vals[index]) && Direction::Ccw != directions[index] {
            continue;
        }
        winding += directions[index] as i32;
    }
    winding // note winding indicates containership, not contour direction
}

/// `left_edge(pts, verb, weight)`: the leftmost point of the curve, its extremum when it has one.
// Port of: src/pathops/SkPathOpsAsWinding.cpp#L132-L192 (chrome/m156)
fn left_edge(pts: &[Point], verb: Verb, weight: f32) -> Point {
    let mut t = [0.0_f64; 1];
    let mut roots = 0;
    if verb == Verb::Line {
        if pts[0].x < pts[1].x { pts[0] } else { pts[1] }
    } else if verb == Verb::Quad {
        let mut quad = DQuad::default();
        quad.set([pts[0], pts[1], pts[2]]);
        if !quad.monotonic_in_x() {
            roots = DQuad::find_extrema([quad.pts[0].x, quad.pts[1].x, quad.pts[2].x], &mut t);
        }
        if roots != 0 {
            quad.pt_at_t(t[0]).as_sk_point()
        } else if pts[0].x < pts[2].x {
            pts[0]
        } else {
            pts[2]
        }
    } else if verb == Verb::Conic {
        let mut conic = DConic::default();
        conic.set([pts[0], pts[1], pts[2]], weight);
        if !conic.monotonic_in_x() {
            roots = DConic::find_extrema(
                [conic.pts[0].x, conic.pts[1].x, conic.pts[2].x],
                weight,
                &mut t,
            );
        }
        if roots != 0 {
            conic.pt_at_t(t[0]).as_sk_point()
        } else if pts[0].x < pts[2].x {
            pts[0]
        } else {
            pts[2]
        }
    } else {
        let mut cubic = DCubic::default();
        cubic.set([pts[0], pts[1], pts[2], pts[3]]);
        let mut result = Point::default();
        if !cubic.monotonic_in_x() {
            let mut t_values = [0.0_f64; 2];
            roots = DCubic::find_extrema(
                [
                    cubic.pts[0].x,
                    cubic.pts[1].x,
                    cubic.pts[2].x,
                    cubic.pts[3].x,
                ],
                &mut t_values,
            );
            for (index, &t_value) in t_values.iter().enumerate().take(roots) {
                let temp = cubic.pt_at_t(t_value).as_sk_point();
                if index == 0 || result.x > temp.x {
                    result = temp;
                }
            }
        }
        // Skia reads `t` here, which only the quad and conic branches set; `SK_INIT_TO_AVOID_WARNING`
        // leaves it at 0.
        if roots != 0 {
            cubic.pt_at_t(t[0]).as_sk_point()
        } else if pts[0].x < pts[3].x {
            pts[0]
        } else {
            pts[3]
        }
    }
}

/// `enum class Edge` of `OpAsWinding::nextEdge`.
// Port of: src/pathops/SkPathOpsAsWinding.cpp#L195-L198 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Edge {
    Initial,
    Compare,
}

/// `class OpAsWinding`: the contour tree of one path.
// Port of: src/pathops/SkPathOpsAsWinding.cpp#L200-L417 (chrome/m156)
struct OpAsWinding<'a> {
    path: &'a Path,
    /// `contours[0]` is the root `sorted`; the contours of the path follow it.
    contours: Vec<Contour>,
}

impl<'a> OpAsWinding<'a> {
    // Port of: src/pathops/SkPathOpsAsWinding.cpp#L201-L202 (chrome/m156)
    fn new(path: &'a Path) -> Self {
        Self {
            path,
            contours: vec![Contour::new(Rect::default(), 0, 0)],
        }
    }

    /// `contourBounds(containers)`: appends the bounds of each contour of the path.
    // Port of: src/pathops/SkPathOpsAsWinding.cpp#L204-L232 (chrome/m156)
    fn contour_bounds(&mut self) {
        let mut bounds = Rect::new_empty();
        let mut last_start = 0;
        let mut verb_start = 0;
        for rec in self.path.iter() {
            let verb = Verb::from(rec.verb());
            let pts = rec.points();
            if verb == Verb::Move {
                if !bounds.is_empty() {
                    self.contours
                        .push(Contour::new(bounds, last_start, verb_start));
                    last_start = verb_start;
                }
                let start = verb_pt_index(Verb::Move);
                bounds.set_bounds(&pts[start..start + verb_pt_count(Verb::Move)]);
            }
            if is_curve(verb) {
                let start = verb_pt_index(verb);
                let mut verb_bounds = Rect::default();
                verb_bounds.set_bounds(&pts[start..start + verb_pt_count(verb)]);
                bounds.join_possibly_empty_rect(verb_bounds);
            }
            verb_start += 1;
        }
        if !bounds.is_empty() {
            verb_start += 1;
            self.contours
                .push(Contour::new(bounds, last_start, verb_start));
        }
    }

    /// `getDirection(contour)`: the sign of the contour's signed area.
    // Port of: src/pathops/SkPathOpsAsWinding.cpp#L234-L262 (chrome/m156)
    fn get_direction(&self, contour: usize) -> Direction {
        let c = &self.contours[contour];
        let mut verb_count: isize = -1;
        let mut total_signed_area: f32 = 0.0;
        for (verb, pts) in Iter::new(self.path, true) {
            verb_count += 1;
            if usize::try_from(verb_count).is_ok_and(|v| v < c.verb_start) {
                continue;
            }
            if usize::try_from(verb_count).is_ok_and(|v| v >= c.verb_end) {
                continue;
            }
            if !is_curve(verb) {
                continue;
            }
            match verb {
                Verb::Line => {
                    total_signed_area += (pts[0].y - pts[1].y) * (pts[0].x + pts[1].x);
                }
                Verb::Quad | Verb::Conic => {
                    total_signed_area += (pts[0].y - pts[2].y) * (pts[0].x + pts[2].x);
                }
                Verb::Cubic => {
                    total_signed_area += (pts[0].y - pts[3].y) * (pts[0].x + pts[3].x);
                }
                _ => {}
            }
        }
        if total_signed_area < 0.0 {
            Direction::Ccw
        } else {
            Direction::Cw
        }
    }

    /// `nextEdge(contour, edge)`: for `kInitial`, updates the contour's leftmost point; for
    /// `kCompare`, returns the winding of the contour's edges about that point.
    // Port of: src/pathops/SkPathOpsAsWinding.cpp#L264-L298 (chrome/m156)
    fn next_edge(&mut self, contour: usize, edge: Edge) -> i32 {
        let (verb_start, verb_end) = (
            self.contours[contour].verb_start,
            self.contours[contour].verb_end,
        );
        let mut verb_count: isize = -1;
        let mut winding = 0;
        // conic weights are consumed in path order, one per conic verb
        let conics = self.path.conic_weights();
        let mut conic_index = 0;
        for (verb, pts) in Iter::new(self.path, true) {
            let weight = if verb == Verb::Conic {
                let w = conics[conic_index];
                conic_index += 1;
                w
            } else {
                1.0
            };
            verb_count += 1;
            if usize::try_from(verb_count).is_ok_and(|v| v < verb_start) {
                continue;
            }
            if usize::try_from(verb_count).is_ok_and(|v| v >= verb_end) {
                continue;
            }
            if !is_curve(verb) {
                continue;
            }
            let mut horizontal = true;
            for index in 1..=verb_pt_count(verb) {
                if pts[0].y != pts[index].y {
                    horizontal = false;
                    break;
                }
            }
            if horizontal {
                continue;
            }
            if edge == Edge::Compare {
                let min_xy = self.contours[contour].min_xy;
                winding += contains_edge(&pts, verb, weight, min_xy);
                continue;
            }
            let min_xy = left_edge(&pts, verb, weight);
            let current = self.contours[contour].min_xy;
            if min_xy.x > current.x {
                continue;
            }
            if min_xy.x == current.x && min_xy.y != current.y {
                continue;
            }
            self.contours[contour].min_xy = min_xy;
        }
        winding
    }

    /// `containerContains(contour, test)`: whether `test` lies inside `contour`, with the winding
    /// check of `nextEdge`.
    // Port of: src/pathops/SkPathOpsAsWinding.cpp#L300-L315 (chrome/m156)
    fn container_contains(&mut self, contour: usize, test: usize) -> bool {
        // find outside point on lesser contour
        // arbitrarily, choose non-horizontal edge where point <= bounds left
        if self.contours[test].min_xy.x == f32::MAX {
            self.next_edge(test, Edge::Initial);
        }
        // find all edges on greater equal or to the left of one on lesser
        self.contours[contour].min_xy = self.contours[test].min_xy;
        let winding = self.next_edge(contour, Edge::Compare);
        // if edge is up, mark contour cw, otherwise, ccw
        // sum of greater edges direction should be cw, 0, ccw
        self.contours[test].contained = winding != 0;
        (-1..=1).contains(&winding)
    }

    /// `inParent(contour, parent)`: moves `contour` into the children of `parent`.
    // Port of: src/pathops/SkPathOpsAsWinding.cpp#L317-L334 (chrome/m156)
    fn in_parent(&mut self, contour: usize, parent: usize) {
        // move contour into sibling list contained by parent
        for i in 0..self.contours[parent].children.len() {
            let test = self.contours[parent].children[i];
            if self.contours[test]
                .bounds
                .contains(self.contours[contour].bounds)
            {
                self.in_parent(contour, test);
                return;
            }
        }
        // move parent's children into contour's children if contained by contour
        let mut i = 0;
        while i < self.contours[parent].children.len() {
            let child = self.contours[parent].children[i];
            if self.contours[contour]
                .bounds
                .contains(self.contours[child].bounds)
            {
                self.contours[contour].children.push(child);
                self.contours[parent].children.remove(i);
                continue;
            }
            i += 1;
        }
        self.contours[parent].children.push(contour);
    }

    /// `checkContainerChildren(parent, child)`.
    // Port of: src/pathops/SkPathOpsAsWinding.cpp#L336-L348 (chrome/m156)
    fn check_container_children(&mut self, parent: Option<usize>, child: usize) -> bool {
        for i in 0..self.contours[child].children.len() {
            let grand_child = self.contours[child].children[i];
            if !self.check_container_children(Some(child), grand_child) {
                return false;
            }
        }
        if let Some(parent) = parent
            && !self.container_contains(parent, child)
        {
            return false;
        }
        true
    }

    /// `markReverse(parent, child)`: sets the directions and the reverse flags, innermost first.
    // Port of: src/pathops/SkPathOpsAsWinding.cpp#L350-L366 (chrome/m156)
    fn mark_reverse(&mut self, parent: Option<usize>, child: usize) -> bool {
        let mut reversed = false;
        for i in 0..self.contours[child].children.len() {
            let grand_child = self.contours[child].children[i];
            let next_parent = if self.contours[grand_child].contained {
                Some(child)
            } else {
                parent
            };
            reversed |= self.mark_reverse(next_parent, grand_child);
        }
        self.contours[child].direction = self.get_direction(child);
        if let Some(parent) = parent
            && self.contours[parent].direction == self.contours[child].direction
        {
            self.contours[child].reverse = true;
            self.contours[child].direction = self.contours[child].direction.negated();
            return true;
        }
        reversed
    }

    /// `reverseMarkedContours(contours, fillType)`: writes the path, reversing the marked
    /// contours.
    // Port of: src/pathops/SkPathOpsAsWinding.cpp#L368-L405 (chrome/m156)
    fn reverse_marked_contours(&self, fill_type: PathFillType) -> Path {
        let mut iter = self.path.iter();
        let mut verb_count = 0;
        let mut result = PathBuilder::new();
        result.set_fill_type(fill_type);
        for contour in &self.contours[1..] {
            let mut reverse = PathBuilder::new();
            loop {
                if verb_count >= contour.verb_end {
                    break;
                }
                let Some(rec) = iter.next() else {
                    break;
                };
                let temp = if contour.reverse {
                    &mut reverse
                } else {
                    &mut result
                };
                let pts = rec.points();
                match Verb::from(rec.verb()) {
                    Verb::Move => {
                        temp.move_to(pts[0]);
                    }
                    Verb::Line => {
                        temp.line_to(pts[1]);
                    }
                    Verb::Quad => {
                        temp.quad_to(pts[1], pts[2]);
                    }
                    Verb::Conic => {
                        temp.conic_to(pts[1], pts[2], rec.conic_weight());
                    }
                    Verb::Cubic => {
                        temp.cubic_to(pts[1], pts[2], pts[3]);
                    }
                    Verb::Close => {
                        temp.close();
                    }
                    Verb::Done => {}
                }
                verb_count += 1;
            }
            if contour.reverse {
                path_priv::reverse_add_path(&mut result, &reverse.detach());
            }
        }
        result.detach()
    }
}

/// `AsWinding(path)`: the path with the winding fill type, or `None` if the path is not finite
/// or cannot be rewritten.
// Port of: src/pathops/SkPathOpsAsWinding.cpp#L419-L460 (chrome/m156)
#[must_use]
pub fn as_winding(path: &Path) -> Option<Path> {
    if !path.is_finite() {
        return None;
    }
    let mut fill_type = path.fill_type();
    if fill_type == PathFillType::Winding || fill_type == PathFillType::InverseWinding {
        return Some(path.clone());
    }
    fill_type = if path.is_inverse_fill_type() {
        PathFillType::InverseWinding
    } else {
        PathFillType::Winding
    };
    if path.is_empty() || path.is_convex() {
        return Some(path.make_fill_type(fill_type));
    }
    // count contours
    let mut winder = OpAsWinding::new(path);
    winder.contour_bounds();
    if winder.contours.len() <= 2 {
        return Some(path.make_fill_type(fill_type));
    }
    // create contour bounding box tree
    for contour in 1..winder.contours.len() {
        winder.in_parent(contour, 0);
    }
    // if sorted has no grandchildren, no child has to fix its children's winding
    let sorted_children = winder.contours[0].children.clone();
    if sorted_children
        .iter()
        .all(|&contour| winder.contours[contour].children.is_empty())
    {
        return Some(path.make_fill_type(fill_type));
    }
    // starting with outermost and moving inward, see if one path contains another
    for &contour in &sorted_children {
        winder.next_edge(contour, Edge::Initial);
        winder.contours[contour].direction = winder.get_direction(contour);
        if !winder.check_container_children(None, contour) {
            return None;
        }
    }
    // starting with outermost and moving inward, mark paths to reverse
    let mut reversed = false;
    for &contour in &sorted_children {
        reversed |= winder.mark_reverse(None, contour);
    }
    if !reversed {
        return Some(path.make_fill_type(fill_type));
    }
    Some(winder.reverse_marked_contours(fill_type))
}
