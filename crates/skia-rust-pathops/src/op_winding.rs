// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsWinding.cpp (chrome/m156)

//! Winding of an edge with no winding yet (`SkPathOpsWinding.cpp`).
//!
//! Given a prospective edge, project a ray from a point on it and find the first edge the ray
//! hits. The winding of the edge follows from the hits. The hits of one ray form a linked list
//! in Skia (new hits are prepended, the base hit is at the end). This port keeps them in a `Vec`
//! in insertion order with a `head` index, so the order that `SkTQSort` sees is the same.
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
use skia_rust_core::t_sort::t_q_sort;

use crate::cubic::DCubic;
use crate::intersections::Intersections;
use crate::line::DLine;
use crate::op_curve::{conic_from, cubic_from, quad_from, verb_points};
use crate::op_state::{ContourId, MAX_WINDING_TRIES, OpPhase, OpState, SK_MIN_S32, SegId, SpanId};
use crate::point::{DPoint, DVector};
use crate::rect::Bounds;
use crate::types::{
    approximately_between, approximately_equal, approximately_zero, between, roughly_equal,
};

/// `SkOpRayDir`: the four directions a ray can take. The low bit selects the axis.
// Port of: src/pathops/SkPathOpsWinding.cpp#L26-L31 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum RayDir {
    Left,
    Top,
    Right,
    Bottom,
}

impl RayDir {
    /// `static_cast<SkOpRayDir>(dir + offset)`, for the offsets Skia uses (0 or 1 from Left/Top).
    fn offset(self, by: i32) -> Self {
        match (self as i32 + by) & 3 {
            0 => Self::Left,
            1 => Self::Top,
            2 => Self::Right,
            _ => Self::Bottom,
        }
    }
}

/// `xy_index(dir)`: 0 for a horizontal ray, 1 for a vertical one.
// Port of: src/pathops/SkPathOpsWinding.cpp#L44-L46 (chrome/m156)
fn xy_index(dir: RayDir) -> usize {
    (dir as usize) & 1
}

/// `pt_xy(pt, dir)`: the coordinate along the ray.
// Port of: src/pathops/SkPathOpsWinding.cpp#L48-L50 (chrome/m156)
fn pt_xy(pt: Point, dir: RayDir) -> f32 {
    if xy_index(dir) == 0 { pt.x } else { pt.y }
}

/// `pt_yx(pt, dir)`: the coordinate across the ray.
// Port of: src/pathops/SkPathOpsWinding.cpp#L52-L54 (chrome/m156)
fn pt_yx(pt: Point, dir: RayDir) -> f32 {
    if xy_index(dir) == 0 { pt.y } else { pt.x }
}

/// `pt_dxdy(v, dir)`.
// Port of: src/pathops/SkPathOpsWinding.cpp#L56-L58 (chrome/m156)
fn pt_dxdy(v: DVector, dir: RayDir) -> f64 {
    if xy_index(dir) == 0 { v.x } else { v.y }
}

/// `pt_dydx(v, dir)`.
// Port of: src/pathops/SkPathOpsWinding.cpp#L60-L62 (chrome/m156)
fn pt_dydx(v: DVector, dir: RayDir) -> f64 {
    if xy_index(dir) == 0 { v.y } else { v.x }
}

/// `rect_side(r, dir)`: `(&r.fLeft)[dir]`.
// Port of: src/pathops/SkPathOpsWinding.cpp#L64-L66 (chrome/m156)
fn rect_side(r: Bounds, dir: RayDir) -> f32 {
    match dir {
        RayDir::Left => r.left,
        RayDir::Top => r.top,
        RayDir::Right => r.right,
        RayDir::Bottom => r.bottom,
    }
}

/// `sideways_overlap(rect, pt, dir)`: the point is within the rect's extent across the ray.
// Port of: src/pathops/SkPathOpsWinding.cpp#L68-L71 (chrome/m156)
fn sideways_overlap(rect: Bounds, pt: Point, dir: RayDir) -> bool {
    let i = 1 - xy_index(dir);
    let (rect_lo, rect_hi, pt_v) = if i == 0 {
        (f64::from(rect.left), f64::from(rect.right), f64::from(pt.x))
    } else {
        (f64::from(rect.top), f64::from(rect.bottom), f64::from(pt.y))
    };
    approximately_between(rect_lo, pt_v, rect_hi)
}

/// `less_than(dir)`: the ray runs toward smaller coordinates.
// Port of: src/pathops/SkPathOpsWinding.cpp#L73-L75 (chrome/m156)
fn less_than(dir: RayDir) -> bool {
    (dir as i32 & 2) == 0
}

/// `ccw_dxdy(v, dir)`: whether the edge with slope `v` is counter-clockwise about the ray.
// Port of: src/pathops/SkPathOpsWinding.cpp#L77-L81 (chrome/m156)
fn ccw_dxdy(v: DVector, dir: RayDir) -> bool {
    let v_part_pos = pt_dydx(v, dir) > 0.0;
    let left_bottom = ((dir as i32 + 1) & 2) != 0;
    v_part_pos == left_bottom
}

/// `!x` on a double: true for zero and for NaN.
fn is_falsy(x: f64) -> bool {
    x == 0.0 || x.is_nan()
}

/// `approximately_equal(float, float)`: Skia promotes the scalars to double.
fn approx_equal_scalar(a: f32, b: f32) -> bool {
    approximately_equal(f64::from(a), f64::from(b))
}

/// `SkOpRayHit`: one hit of the projected ray. `span` is `None` when the hit has no span.
// Port of: src/pathops/SkPathOpsWinding.cpp#L83-L95 (chrome/m156)
#[derive(Copy, Clone, Debug)]
struct RayHit {
    /// `SkOpRayHit* fNext`: the next hit in the list (index into [`RayHits::items`]).
    next: Option<usize>,
    /// `SkOpSpan* fSpan`.
    span: Option<SpanId>,
    /// `SkPoint fPt`.
    pt: Point,
    /// `double fT`.
    t: f64,
    /// `SkDVector fSlope`.
    slope: DVector,
    /// `bool fValid`.
    valid: bool,
}

/// The hits of one ray, as the list Skia builds with `allocator->make<SkOpRayHit>()`.
#[derive(Debug, Default)]
struct RayHits {
    items: Vec<RayHit>,
    /// `SkOpRayHit* hits` (the list head).
    head: Option<usize>,
}

impl RayHits {
    /// Prepends a hit (`newHit->fNext = *hits; *hits = newHit;`).
    fn prepend(&mut self, mut hit: RayHit) {
        hit.next = self.head;
        self.items.push(hit);
        self.head = Some(self.items.len() - 1);
    }

    /// The hits in list order, head first.
    fn list_order(&self) -> Vec<usize> {
        let mut order = Vec::new();
        let mut cur = self.head;
        while let Some(i) = cur {
            order.push(i);
            cur = self.items[i].next;
        }
        order
    }
}

/// `CurveIntercept[verb * 2 + xy_index(dir)](pts, weight, axis, roots)`: the roots of the
/// curve against the horizontal (`horizontal`) or vertical intercept.
// Port of: src/pathops/SkPathOpsCurve.h#L300-L426 (chrome/m156)
pub(crate) fn curve_intercept(
    verb: Verb,
    horizontal: bool,
    pts: &[Point],
    weight: f32,
    axis: f32,
    roots: &mut [f64; 3],
) -> usize {
    match verb {
        Verb::Line => {
            let line = DLine::new([
                DPoint::new(f64::from(pts[0].x), f64::from(pts[0].y)),
                DPoint::new(f64::from(pts[1].x), f64::from(pts[1].y)),
            ]);
            if horizontal {
                if pts[0].y == pts[1].y {
                    return 0;
                }
                roots[0] = Intersections::horizontal_intercept_line(&line, f64::from(axis));
            } else {
                if pts[0].x == pts[1].x {
                    return 0;
                }
                roots[0] = Intersections::vertical_intercept_line(&line, f64::from(axis));
            }
            usize::from(between(0.0, roots[0], 1.0))
        }
        Verb::Quad => {
            let quad = quad_from(pts);
            let mut r = [0.0_f64; 2];
            let n = if horizontal {
                Intersections::horizontal_intercept_quad(&quad, axis, &mut r)
            } else {
                Intersections::vertical_intercept_quad(&quad, axis, &mut r)
            };
            roots[0] = r[0];
            roots[1] = r[1];
            n
        }
        Verb::Conic => {
            let conic = conic_from(pts, weight);
            let mut r = [0.0_f64; 2];
            let n = if horizontal {
                Intersections::horizontal_intercept_conic(&conic, axis, &mut r)
            } else {
                Intersections::vertical_intercept_conic(&conic, axis, &mut r)
            };
            roots[0] = r[0];
            roots[1] = r[1];
            n
        }
        Verb::Cubic => {
            let cubic: DCubic = cubic_from(pts);
            if horizontal {
                cubic.horizontal_intersect(f64::from(axis), roots)
            } else {
                cubic.vertical_intersect(f64::from(axis), roots)
            }
        }
        _ => 0,
    }
}

/// `get_t_guess(tTry, &dirOffset)`: the t to try and the direction offset.
// Port of: src/pathops/SkPathOpsWinding.cpp#L202-L217 (chrome/m156)
fn get_t_guess(t_try: i32) -> (f64, i32) {
    let mut t = 0.5_f64;
    let dir_offset = t_try & 1;
    let t_base = t_try >> 1;
    let mut t_bits = 0_i32;
    let mut shifted = t_try;
    loop {
        shifted >>= 1;
        if shifted == 0 {
            break;
        }
        t /= 2.0;
        t_bits += 1;
    }
    if t_bits != 0 {
        let t_index = (t_base - 1) & ((1 << t_bits) - 1);
        t += t * 2.0 * f64::from(t_index);
    }
    (t, dir_offset)
}

/// `SkOpSegment::windingSpanAtT(tHit)`: the span of the segment that contains `t_hit`, or
/// `None` if `t_hit` sits on a span boundary.
// Port of: src/pathops/SkPathOpsWinding.cpp#L140-L157 (chrome/m156)
fn seg_winding_span_at_t(state: &OpState, seg: SegId, t_hit: f64) -> Option<SpanId> {
    let mut span = state.seg_head(seg);
    loop {
        let next = state.span_next(span)?;
        if approximately_equal(t_hit, state.span_t(next)) {
            return None;
        }
        if t_hit < state.span_t(next) {
            return Some(span);
        }
        if state.span_final(next) {
            return None;
        }
        span = state.span_up_cast(next);
    }
}

impl OpState {
    /// `SkOpSegment::rayCheck(base, dir, hits, allocator)`: adds the hits of the ray on this
    /// segment.
    // Port of: src/pathops/SkPathOpsWinding.cpp#L97-L179 (chrome/m156)
    fn seg_ray_check(
        &self,
        seg: SegId,
        base: &RayHit,
        base_span_seg: SegId,
        dir: RayDir,
        hits: &mut RayHits,
    ) {
        let bounds = self.seg_bounds(seg);
        if !sideways_overlap(bounds, base.pt, dir) {
            return;
        }
        let base_xy = pt_xy(base.pt, dir);
        let bounds_xy = rect_side(bounds, dir);
        let check_less_than = less_than(dir);
        if !approx_equal_scalar(base_xy, bounds_xy) && ((base_xy < bounds_xy) == check_less_than) {
            return;
        }
        let mut t_vals = [0.0_f64; 3];
        let base_yx = pt_yx(base.pt, dir);
        let verb = self.seg_verb(seg);
        let pts = self.seg_pts(seg);
        let weight = self.seg_weight(seg);
        let roots = curve_intercept(verb, xy_index(dir) == 0, &pts, weight, base_yx, &mut t_vals);
        for &t in &t_vals[..roots] {
            if base_span_seg == seg && approximately_equal(base.t, t) {
                continue;
            }
            let mut slope = DVector::new(0.0, 0.0);
            let pt: Point;
            let mut valid = false;
            if approximately_zero(t) {
                pt = pts[0];
            } else if approximately_equal(t, 1.0) {
                pt = pts[verb_points(verb)];
            } else {
                pt = self.seg_pt_at_t(seg, t);
                if DPoint::approximately_equal_points(pt, base.pt) {
                    if base_span_seg == seg {
                        continue;
                    }
                } else {
                    let pt_xy_v = pt_xy(pt, dir);
                    if !approx_equal_scalar(base_xy, pt_xy_v)
                        && ((base_xy < pt_xy_v) == check_less_than)
                    {
                        continue;
                    }
                    slope = self.seg_d_slope_at_t(seg, t);
                    if verb == Verb::Cubic
                        && base_span_seg == seg
                        && roughly_equal(base.t, t)
                        && DPoint::roughly_equal_points(pt, base.pt)
                    {
                        // Rarely expected; Skia skips this hit.
                        continue;
                    }
                    if (pt_dydx(slope, dir) * 10000.0).abs() > pt_dxdy(slope, dir).abs() {
                        valid = true;
                    }
                }
            }
            let span = seg_winding_span_at_t(self, seg, t);
            match span {
                None => valid = false,
                Some(s) => {
                    if self.span_wind_value(s) == 0 && self.span_opp_value(s) == 0 {
                        continue;
                    }
                }
            }
            hits.prepend(RayHit {
                next: None,
                span,
                pt,
                t,
                slope,
                valid,
            });
        }
    }

    /// `SkOpContour::rayCheck(base, dir, hits, allocator)`.
    // Port of: src/pathops/SkPathOpsWinding.cpp#L85-L99 (chrome/m156)
    fn contour_ray_check(
        &self,
        contour: ContourId,
        base: &RayHit,
        base_span_seg: SegId,
        dir: RayDir,
        hits: &mut RayHits,
    ) {
        let bounds = self.contour_bounds(contour);
        let base_xy = pt_xy(base.pt, dir);
        let bounds_xy = rect_side(bounds, dir);
        let check_less_than = less_than(dir);
        if !approx_equal_scalar(base_xy, bounds_xy) && ((base_xy < bounds_xy) == check_less_than) {
            return;
        }
        let mut seg = self.contour_first(contour);
        while let Some(s) = seg {
            self.seg_ray_check(s, base, base_span_seg, dir, hits);
            seg = self.seg_next(s);
        }
    }

    /// `SkOpSpan::makeTestBase`: the base hit of the ray, at `t` between the span and its next.
    /// Returns the base hit and the direction the ray prefers.
    // Port of: src/pathops/SkPathOpsWinding.cpp#L97-L112 (chrome/m156)
    fn ray_make_test_base(&self, span: SpanId, t: f64) -> (RayHit, RayDir) {
        let next = self.span_next(span).expect("span has a next span");
        let t_hit = self.span_t(span) * (1.0 - t) + self.span_t(next) * t;
        let seg = self.span_segment(span);
        let slope = self.seg_d_slope_at_t(seg, t_hit);
        let pt = self.seg_pt_at_t(seg, t_hit);
        let dir = if slope.x.abs() < slope.y.abs() {
            RayDir::Left
        } else {
            RayDir::Top
        };
        (
            RayHit {
                next: None,
                span: Some(span),
                pt,
                t: t_hit,
                slope,
                valid: true,
            },
            dir,
        )
    }

    /// `SkOpSpan::sortableTop(contourHead)`: computes the winding of this span by projecting a
    /// ray, returning whether the winding could be determined.
    // Port of: src/pathops/SkPathOpsWinding.cpp#L268-L405 (chrome/m156)
    pub(crate) fn span_sortable_top(&mut self, span: SpanId, contour_head: ContourId) -> bool {
        let t_try = self.span_top_t_try(span);
        self.span_set_top_t_try(span, t_try + 1);
        let (t, dir_offset) = get_t_guess(t_try);
        let (hit_base, base_dir) = self.ray_make_test_base(span, t);
        if hit_base.slope.x == 0.0 && hit_base.slope.y == 0.0 {
            return false;
        }
        let dir = base_dir.offset(dir_offset);
        let span_seg = self.span_segment(span);
        if (self.seg_verb(span_seg) as u8) > (Verb::Line as u8)
            && is_falsy(pt_dydx(hit_base.slope, dir))
        {
            return false;
        }
        let base_span_seg = span_seg;
        let mut hits = RayHits::default();
        hits.prepend(hit_base);
        let mut contour = Some(contour_head);
        while let Some(c) = contour {
            if self.contour_count(c) != 0 {
                self.contour_ray_check(c, &hit_base, base_span_seg, dir, &mut hits);
            }
            contour = self.contour_next(c);
        }
        // Sort hits. Skia uses SkTQSort, which is not stable, so the comparator and the input
        // order both matter.
        let order = hits.list_order();
        let count = order.len();
        let items = &hits.items;
        let mut sorted = order;
        let less = |a: &usize, b: &usize| -> bool {
            let (pa, pb) = (items[*a].pt, items[*b].pt);
            match (xy_index(dir), less_than(dir)) {
                (1, true) => pa.y < pb.y,
                (1, false) => pb.y < pa.y,
                (_, true) => pa.x < pb.x,
                (_, false) => pb.x < pa.x,
            }
        };
        t_q_sort(&mut sorted, less);
        // Verify windings.
        let mut last: Option<Point> = None;
        let mut wind: i32 = 0;
        let mut opp_wind: i32 = 0;
        for index in 0..count {
            let hit = hits.items[sorted[index]];
            if !hit.valid {
                return false;
            }
            let ccw = ccw_dxdy(hit.slope, dir);
            let Some(hit_span) = hit.span else {
                return false;
            };
            let hit_segment = self.span_segment(hit_span);
            if self.span_wind_value(hit_span) == 0 && self.span_opp_value(hit_span) == 0 {
                continue;
            }
            if let Some(l) = last
                && DPoint::approximately_equal_points(l, hit.pt)
            {
                return false;
            }
            if index + 1 < count {
                let next_pt = hits.items[sorted[index + 1]].pt;
                if DPoint::approximately_equal_points(next_pt, hit.pt) {
                    return false;
                }
            }
            let operand = self.seg_operand(hit_segment);
            if operand {
                std::mem::swap(&mut wind, &mut opp_wind);
            }
            let last_wind = wind;
            let last_opp = opp_wind;
            let span_wind = self.span_wind_value(hit_span);
            let span_opp = self.span_opp_value(hit_span);
            let wind_value = if ccw { -span_wind } else { span_wind };
            let opp_value = if ccw { -span_opp } else { span_opp };
            wind += wind_value;
            opp_wind += opp_value;
            let mut sum_set = false;
            let span_sum = self.span_wind_sum(hit_span);
            let wind_sum = if OpState::use_inner_winding(last_wind, wind) {
                wind
            } else {
                last_wind
            };
            if span_sum == SK_MIN_S32 {
                self.span_set_wind_sum(hit_span, wind_sum);
                sum_set = true;
            }
            let o_span_sum = self.span_opp_sum(hit_span);
            let opp_sum = if OpState::use_inner_winding(last_opp, opp_wind) {
                opp_wind
            } else {
                last_opp
            };
            if o_span_sum == SK_MIN_S32 {
                self.span_set_opp_sum(hit_span, opp_sum);
            }
            if sum_set {
                if self.phase() == OpPhase::FixWinding {
                    let contour = self.seg_contour(hit_segment);
                    self.contour_set_ccw(contour, i32::from(ccw));
                } else {
                    let next = self.span_next(hit_span).expect("span has a next span");
                    let _ = self.seg_mark_and_chase_winding_opp(
                        hit_segment,
                        hit_span,
                        next,
                        wind_sum,
                        opp_sum,
                        &mut None,
                    );
                    let _ = self.seg_mark_and_chase_winding_opp(
                        hit_segment,
                        next,
                        hit_span,
                        wind_sum,
                        opp_sum,
                        &mut None,
                    );
                }
            }
            if operand {
                std::mem::swap(&mut wind, &mut opp_wind);
            }
            last = Some(hit.pt);
            self.bump_nested();
        }
        true
    }

    /// `SkOpSegment::findSortableTop(contourHead)`.
    // Port of: src/pathops/SkPathOpsWinding.cpp#L407-L423 (chrome/m156)
    pub(crate) fn seg_find_sortable_top(
        &mut self,
        seg: SegId,
        contour_head: ContourId,
    ) -> Option<SpanId> {
        let mut span = self.seg_head(seg);
        loop {
            let next = self.span_next(span)?;
            'body: {
                if self.span_done(span) {
                    break 'body;
                }
                if self.span_wind_sum(span) != SK_MIN_S32 {
                    return Some(span);
                }
                if self.span_sortable_top(span, contour_head) {
                    return Some(span);
                }
            }
            if self.span_final(next) {
                return None;
            }
            span = self.span_up_cast(next);
        }
    }

    /// `SkOpContour::findSortableTop(contourHead)`.
    // Port of: src/pathops/SkPathOpsWinding.cpp#L425-L444 (chrome/m156)
    pub(crate) fn contour_find_sortable_top(
        &mut self,
        contour: ContourId,
        contour_head: ContourId,
    ) -> Option<SpanId> {
        let mut all_done = true;
        if self.contour_count(contour) != 0 {
            let mut test = self.contour_first(contour);
            while let Some(seg) = test {
                if !self.seg_done(seg) {
                    all_done = false;
                    if let Some(result) = self.seg_find_sortable_top(seg, contour_head) {
                        return Some(result);
                    }
                }
                test = self.seg_next(seg);
            }
        }
        if all_done {
            self.contours[contour.0].done = true;
        }
        None
    }

    /// `FindSortableTop(contourHead)`: tries each contour up to `kMaxWindingTries` times.
    // Port of: src/pathops/SkPathOpsWinding.cpp#L446-L460 (chrome/m156)
    pub(crate) fn find_sortable_top(&mut self, contour_head: ContourId) -> Option<SpanId> {
        for _ in 0..MAX_WINDING_TRIES {
            let mut contour = Some(contour_head);
            while let Some(c) = contour {
                if !self.contour_done(c)
                    && let Some(result) = self.contour_find_sortable_top(c, contour_head)
                {
                    return Some(result);
                }
                contour = self.contour_next(c);
            }
        }
        None
    }
}
