// Copyright 2008 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkStroke.h, src/core/SkStroke.cpp

//! `SkStroke`: constructs paths by stroking geometries (lines, rects, ovals, round rects,
//! paths). The curve approximation (`SkPathStroker`) is private, as in Skia.

use crate::floating_point::{ieee_float_divide, is_nan};
use crate::geometry::{
    Conic, chop_cubic_at, eval_cubic_at, eval_quad_at, eval_quad_at_pos_tangent, find_cubic_cusp,
    find_cubic_inflections, find_cubic_max_curvature, find_quad_max_curvature,
    find_unit_quad_roots,
};
use crate::paint::{Cap, DEFAULT_MITER_LIMIT, Join};
use crate::path::{Iter, Path};
use crate::path_builder::PathBuilder;
use crate::path_enums::PathFirstDirection;
use crate::path_enums::ResolveConvexity;
use crate::path_priv;
use crate::path_types::{PathDirection, PathSegmentMask, PathVerb};
use crate::point::{Point, Vector, point_priv};
use crate::rect::{Contains, Rect};
use crate::rrect::RRect;
use crate::scalar::{
    SCALAR_1, SCALAR_HALF, SCALAR_NEARLY_ZERO, SCALAR_SQRT2, Scalar, scalar, scalar_invert,
};
use crate::stroker_priv::{CapProc, JoinProc, StrokerPriv};

// Port of: src/core/SkStroke.cpp#L30-L35 (chrome/m156)
const TANGENT_RECURSIVE_LIMIT: usize = 0;
const CUBIC_RECURSIVE_LIMIT: usize = 1;
const CONIC_RECURSIVE_LIMIT: usize = 2;
const QUAD_RECURSIVE_LIMIT: usize = 3;

// quads with extreme widths (e.g. (0,1) (1,6) (0,3) width=5e7) recurse to point of failure
// largest seen for normal cubics : 5, 26
// largest seen for normal quads : 11
// The kRecursiveLimits are somewhat arbitrarily chosen: we simply try to choose the largest depth
// that won't timeout the fuzzer. For cubics that's 24; for quads and conics, that's 16.
// Port of: src/core/SkStroke.cpp#L37-L42 (chrome/m156)
const RECURSIVE_LIMITS: [i32; 4] = [5 * 3, 24, 16, 16];

const _: () = assert!(
    TANGENT_RECURSIVE_LIMIT == 0,
    "cubic_stroke_relies_on_tangent_equalling_zero"
);
const _: () = assert!(
    CUBIC_RECURSIVE_LIMIT == 1,
    "cubic_stroke_relies_on_cubic_equalling_one"
);
const _: () = assert!(
    RECURSIVE_LIMITS.len() == QUAD_RECURSIVE_LIMIT + 1,
    "recursive_limits_mismatch"
);

// `std::max(a, b)`: `(a < b) ? b : a`.
fn std_max(a: scalar, b: scalar) -> scalar {
    if a < b { b } else { a }
}

// `std::min(a, b)`: `(b < a) ? b : a`.
fn std_min(a: scalar, b: scalar) -> scalar {
    if b < a { b } else { a }
}

// Port of: src/core/SkStroke.cpp#L99-L101 (chrome/m156)
fn degenerate_vector(v: Vector) -> bool {
    !point_priv::can_normalize(v.x, v.y)
}

// Port of: src/core/SkStroke.cpp#L103-L113 (chrome/m156)
fn set_normal_unitnormal(
    before: Point,
    after: Point,
    scale: scalar,
    radius: scalar,
    normal: &mut Vector,
    unit_normal: &mut Vector,
) -> bool {
    if !unit_normal.set_normalize((after.x - before.x) * scale, (after.y - before.y) * scale) {
        return false;
    }
    point_priv::rotate_ccw_in_place(unit_normal);
    *normal = unit_normal.scaled(radius);
    true
}

// Port of: src/core/SkStroke.cpp#L115-L124 (chrome/m156)
fn set_normal_unitnormal_vec(
    vec: Vector,
    radius: scalar,
    normal: &mut Vector,
    unit_normal: &mut Vector,
) -> bool {
    if !unit_normal.set_normalize(vec.x, vec.y) {
        return false;
    }
    point_priv::rotate_ccw_in_place(unit_normal);
    *normal = unit_normal.scaled(radius);
    true
}

///////////////////////////////////////////////////////////////////////////////

/// The state of the quad stroke under construction (`SkQuadConstruct`).
// Port of: src/core/SkStroke.cpp#L128-L167 (chrome/m156)
#[derive(Copy, Clone, Debug, Default)]
struct QuadConstruct {
    quad: [Point; 3],        // the stroked quad parallel to the original curve
    tangent_start: Vector,   // tangent vector at quad[0]
    tangent_end: Vector,     // tangent vector at quad[2]
    start_t: scalar,         // a segment of the original curve
    mid_t: scalar,           //              "
    end_t: scalar,           //              "
    start_set: bool,         // state to share common points across structs
    end_set: bool,           //                     "
    opposite_tangents: bool, // set if coincident tangents have opposite directions
}

impl QuadConstruct {
    // return false if start and end are too close to have a unique middle
    fn init(&mut self, start: scalar, end: scalar) -> bool {
        self.start_t = start;
        self.mid_t = (start + end) * SCALAR_HALF;
        self.end_t = end;
        self.start_set = false;
        self.end_set = false;
        self.start_t < self.mid_t && self.mid_t < self.end_t
    }

    fn init_with_start(&mut self, parent: &QuadConstruct) -> bool {
        if !self.init(parent.start_t, parent.mid_t) {
            return false;
        }
        self.quad[0] = parent.quad[0];
        self.tangent_start = parent.tangent_start;
        self.start_set = true;
        true
    }

    fn init_with_end(&mut self, parent: &QuadConstruct) -> bool {
        if !self.init(parent.mid_t, parent.end_t) {
            return false;
        }
        self.quad[2] = parent.quad[2];
        self.tangent_end = parent.tangent_end;
        self.end_set = true;
        true
    }
}

// Port of: src/core/SkStroke.cpp#L169-L182 (chrome/m156)
fn is_zero_length_since_point(span: &[Point], start_pt_index: usize) -> bool {
    let count = span.len().saturating_sub(start_pt_index);
    if count < 2 {
        return true;
    }
    let pts = &span[start_pt_index..];
    let first = pts[0];
    for pt in &pts[1..count] {
        if first != *pt {
            return false;
        }
    }
    true
}

// `use sign-opposite values later to flip perpendicular axis`
#[derive(Copy, Clone, PartialEq, Eq)]
enum StrokeType {
    Outer = 1,
    Inner = -1,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum ResultType {
    Split,      // the caller should split the quad stroke in two
    Degenerate, // the caller should add a line
    Quad,       // the caller should (continue to try to) add a quad stroke
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum ReductionType {
    Point,       // all curve points are practically identical
    Line,        // the control point is on the line between the ends
    Quad,        // the control point is outside the line between the ends
    Degenerate,  // the control point is on the line but outside the ends
    Degenerate2, // two control points are on the line but outside ends (cubic)
    Degenerate3, // three areas of max curvature found (for cubic)
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum IntersectRayType {
    CtrlPt,
    ResultType,
}

// Port of: src/core/SkStroke.cpp#L184-L309 (chrome/m156)
#[allow(clippy::struct_excessive_bools)] // mirrors the C++ members
struct PathStroker {
    radius: scalar,
    inv_miter_limit: scalar,
    res_scale: scalar,
    inv_res_scale: scalar,
    inv_res_scale_squared: scalar,

    first_normal: Vector,
    prev_normal: Vector,
    first_unit_normal: Vector,
    prev_unit_normal: Vector,
    first_pt: Point,
    prev_pt: Point, // on original path
    first_outer_pt: Point,
    first_outer_pt_index_in_contour: usize,
    segment_count: i32,
    prev_is_line: bool,
    can_ignore_center: bool,

    cap: Cap,
    capper: CapProc,
    joiner: JoinProc,

    inner: PathBuilder,
    outer: PathBuilder, // outer is our working answer, inner is temp
    cusper: PathBuilder,

    stroke_type: StrokeType,

    recursion_depth: i32, // track stack depth to abort if numerics run amok
    found_tangents: bool, // do less work until tangents meet (cubic)
    join_completed: bool, // previous join was not degenerate
}

///////////////////////////////////////////////////////////////////////////////

impl PathStroker {
    // Port of: src/core/SkStroke.cpp#L313-L341 (chrome/m156)
    fn pre_join_to(
        &mut self,
        curr_pt: Point,
        normal: &mut Vector,
        unit_normal: &mut Vector,
        curr_is_line: bool,
    ) -> bool {
        debug_assert!(self.segment_count >= 0);

        if !set_normal_unitnormal(
            self.prev_pt,
            curr_pt,
            self.res_scale,
            self.radius,
            normal,
            unit_normal,
        ) {
            if Cap::Butt == self.cap {
                return false;
            }
            /* Square caps and round caps draw even if the segment length is zero.
            Since the zero length segment has no direction, set the orientation
            to upright as the default orientation */
            *normal = Point::new(self.radius, 0.0);
            *unit_normal = Point::new(1.0, 0.0);
        }

        if self.segment_count == 0 {
            self.first_normal = *normal;
            self.first_unit_normal = *unit_normal;
            self.first_outer_pt = self.prev_pt + *normal;

            self.outer.move_to(self.first_outer_pt);
            self.inner.move_to(self.prev_pt - *normal);
        } else {
            // we have a previous segment
            (self.joiner)(
                &mut self.outer,
                &mut self.inner,
                self.prev_unit_normal,
                self.prev_pt,
                *unit_normal,
                self.radius,
                self.inv_miter_limit,
                self.prev_is_line,
                curr_is_line,
            );
        }
        self.prev_is_line = curr_is_line;
        true
    }

    // Port of: src/core/SkStroke.cpp#L343-L350 (chrome/m156)
    fn post_join_to(&mut self, curr_pt: Point, normal: Vector, unit_normal: Vector) {
        self.join_completed = true;
        self.prev_pt = curr_pt;
        self.prev_unit_normal = unit_normal;
        self.prev_normal = normal;
        self.segment_count += 1;
    }

    // Port of: src/core/SkStroke.cpp#L352-L391 (chrome/m156)
    fn finish_contour(&mut self, close: bool, curr_is_line: bool) {
        if self.segment_count > 0 {
            if close {
                (self.joiner)(
                    &mut self.outer,
                    &mut self.inner,
                    self.prev_unit_normal,
                    self.prev_pt,
                    self.first_unit_normal,
                    self.radius,
                    self.inv_miter_limit,
                    self.prev_is_line,
                    curr_is_line,
                );
                self.outer.close();

                if self.can_ignore_center {
                    // If we can ignore the center just make sure the larger of the two paths
                    // is preserved and don't add the smaller one.
                    if self
                        .inner
                        .compute_bounds()
                        .contains(self.outer.compute_bounds())
                    {
                        self.outer = self.inner.clone();
                    }
                } else {
                    // now add fInner as its own contour
                    if let Some(pt) = self.inner.get_last_pt() {
                        self.outer.move_to(pt);
                        let inner = self.inner.detach();
                        self.outer.private_reverse_path_to(&inner); // todo: take builder or raw
                        self.outer.close();
                    }
                }
            } else {
                // add caps to start and end
                // cap the end
                if let Some(pt) = self.inner.get_last_pt() {
                    (self.capper)(
                        &mut self.outer,
                        self.prev_pt,
                        self.prev_normal,
                        pt,
                        curr_is_line,
                    );
                    let inner = self.inner.detach();
                    self.outer.private_reverse_path_to(&inner);
                    // cap the start
                    (self.capper)(
                        &mut self.outer,
                        self.first_pt,
                        -self.first_normal,
                        self.first_outer_pt,
                        self.prev_is_line,
                    );
                    self.outer.close();
                }
            }
            if !self.cusper.is_empty() {
                let cusper = self.cusper.detach();
                self.outer.add_path(&cusper, None);
            }
        }
        self.inner.reset();
        self.segment_count = -1;
        self.first_outer_pt_index_in_contour = self.outer.count_points();
    }

    ///////////////////////////////////////////////////////////////////////////////

    // Port of: src/core/SkStroke.cpp#L395-L436 (chrome/m156)
    fn new(
        src: &Path,
        radius: scalar,
        miter_limit: scalar,
        cap: Cap,
        join: Join,
        res_scale: scalar,
        can_ignore_center: bool,
    ) -> Self {
        let mut join = join;
        /*  This is only used when join is miter_join, but we initialize it here
            so that it is always defined, to fix sanitizer warnings.
        */
        let mut inv_miter_limit = 0.0;

        if join == Join::Miter {
            if miter_limit <= SCALAR_1 {
                join = Join::Bevel;
            } else {
                inv_miter_limit = scalar_invert(miter_limit);
            }
        }
        let capper = StrokerPriv::cap_factory(cap);
        let joiner = StrokerPriv::join_factory(join);

        // Need some estimate of how large our final result (fOuter)
        // and our per-contour temp (fInner) will be, so we don't spend
        // extra time repeatedly growing these arrays.
        //
        // 3x for result == inner + outer + join (swag)
        // 1x for inner == 'wag' (worst contour length would be better guess)
        let mut outer = PathBuilder::new();
        let mut inner = PathBuilder::new();
        let count = i32::try_from(src.points().len()).unwrap_or(i32::MAX);
        outer.inc_reserve(count.saturating_mul(3), count.saturating_mul(3), 0);
        outer.set_is_volatile(true);
        inner.inc_reserve(count, count, 0);
        inner.set_is_volatile(true);
        // TODO : write a common error function used by stroking and filling
        // The '4' below matches the fill scan converter's error term
        let inv_res_scale = scalar_invert(res_scale * 4.0);
        let inv_res_scale_squared = inv_res_scale * inv_res_scale;

        Self {
            radius,
            inv_miter_limit,
            res_scale,
            inv_res_scale,
            inv_res_scale_squared,
            first_normal: Vector::default(),
            prev_normal: Vector::default(),
            first_unit_normal: Vector::default(),
            prev_unit_normal: Vector::default(),
            first_pt: Point::default(),
            prev_pt: Point::default(),
            first_outer_pt: Point::default(),
            first_outer_pt_index_in_contour: 0,
            segment_count: -1,
            prev_is_line: false,
            can_ignore_center,
            cap,
            capper,
            joiner,
            inner,
            outer,
            cusper: PathBuilder::new(),
            stroke_type: StrokeType::Outer,
            recursion_depth: 0,
            found_tangents: false,
            join_completed: false,
        }
    }

    fn has_only_move_to(&self) -> bool {
        0 == self.segment_count
    }

    fn move_to_pt(&self) -> Point {
        self.first_pt
    }

    // Port of: src/core/SkStroke.cpp#L438-L445 (chrome/m156)
    fn move_to(&mut self, pt: Point) {
        if self.segment_count > 0 {
            self.finish_contour(false, false);
        }
        self.segment_count = 0;
        self.first_pt = pt;
        self.prev_pt = pt;
        self.join_completed = false;
    }

    // Port of: src/core/SkStroke.cpp#L447-L450 (chrome/m156)
    fn line_to_normal(&mut self, curr_pt: Point, normal: Vector) {
        self.outer.line_to(curr_pt + normal);
        self.inner.line_to(curr_pt - normal);
    }

    fn close(&mut self, is_line: bool) {
        self.finish_contour(true, is_line);
    }

    fn done(&mut self, dst: &mut PathBuilder, is_line: bool) {
        self.finish_contour(false, is_line);
        *dst = std::mem::take(&mut self.outer);
    }

    fn is_current_contour_empty(&self) -> bool {
        is_zero_length_since_point(self.inner.points(), 0)
            && is_zero_length_since_point(self.outer.points(), self.first_outer_pt_index_in_contour)
    }

    // Port of: src/core/SkStroke.cpp#L482-L497 (chrome/m156)
    fn line_to(&mut self, curr_pt: Point, iter: Option<&Iter<'_>>) {
        let teeny_line = point_priv::equals_within_tolerance_tol(
            self.prev_pt,
            curr_pt,
            SCALAR_NEARLY_ZERO * self.inv_res_scale,
        );
        if Cap::Butt == self.cap && teeny_line {
            return;
        }
        if teeny_line && (self.join_completed || iter.is_some_and(has_valid_tangent)) {
            return;
        }
        let mut normal = Vector::default();
        let mut unit_normal = Vector::default();

        if !self.pre_join_to(curr_pt, &mut normal, &mut unit_normal, true) {
            return;
        }
        self.line_to_normal(curr_pt, normal);
        self.post_join_to(curr_pt, normal, unit_normal);
    }

    // Port of: src/core/SkStroke.cpp#L499-L505 (chrome/m156)
    fn set_quad_end_normal(
        &self,
        quad: &[Point],
        normal_ab: Vector,
        unit_normal_ab: Vector,
        normal_bc: &mut Vector,
        unit_normal_bc: &mut Vector,
    ) {
        if !set_normal_unitnormal(
            quad[1],
            quad[2],
            self.res_scale,
            self.radius,
            normal_bc,
            unit_normal_bc,
        ) {
            *normal_bc = normal_ab;
            *unit_normal_bc = unit_normal_ab;
        }
    }

    // Port of: src/core/SkStroke.cpp#L507-L510 (chrome/m156)
    fn set_conic_end_normal(
        &self,
        conic: &Conic,
        normal_ab: Vector,
        unit_normal_ab: Vector,
        normal_bc: &mut Vector,
        unit_normal_bc: &mut Vector,
    ) {
        self.set_quad_end_normal(
            &conic.pts,
            normal_ab,
            unit_normal_ab,
            normal_bc,
            unit_normal_bc,
        );
    }

    // Port of: src/core/SkStroke.cpp#L512-L539 (chrome/m156)
    fn set_cubic_end_normal(
        &self,
        cubic: &[Point],
        normal_ab: Vector,
        unit_normal_ab: Vector,
        normal_cd: &mut Vector,
        unit_normal_cd: &mut Vector,
    ) {
        let mut ab = cubic[1] - cubic[0];
        let mut cd = cubic[3] - cubic[2];

        let mut degenerate_ab = degenerate_vector(ab);
        let mut degenerate_cd = degenerate_vector(cd);

        // `DEGENERATE_NORMAL` label of the C++.
        let degenerate_normal = 'degenerate: {
            if degenerate_ab && degenerate_cd {
                break 'degenerate true;
            }

            if degenerate_ab {
                ab = cubic[2] - cubic[0];
                degenerate_ab = degenerate_vector(ab);
            }
            if degenerate_cd {
                cd = cubic[3] - cubic[1];
                degenerate_cd = degenerate_vector(cd);
            }
            degenerate_ab || degenerate_cd
        };
        if degenerate_normal {
            *normal_cd = normal_ab;
            *unit_normal_cd = unit_normal_ab;
            return;
        }
        let ok = set_normal_unitnormal_vec(cd, self.radius, normal_cd, unit_normal_cd);
        debug_assert!(ok);
    }

    // Port of: src/core/SkStroke.cpp#L541-L547 (chrome/m156)
    fn init(
        &mut self,
        stroke_type: StrokeType,
        quad_pts: &mut QuadConstruct,
        t_start: scalar,
        t_end: scalar,
    ) {
        self.stroke_type = stroke_type;
        self.found_tangents = false;
        self.recursion_depth = 0;
        quad_pts.init(t_start, t_end);
    }

    // the sink the current stroke type draws into
    fn sink(&mut self) -> &mut PathBuilder {
        if self.stroke_type == StrokeType::Outer {
            &mut self.outer
        } else {
            &mut self.inner
        }
    }

    // Port of: src/core/SkStroke.cpp#L669-L706 (chrome/m156)
    fn check_cubic_linear(
        cubic: &[Point],
        reduction: &mut [Point; 3],
        tangent_pt: &mut Point,
    ) -> ReductionType {
        let degenerate_ab = degenerate_vector(cubic[1] - cubic[0]);
        let degenerate_bc = degenerate_vector(cubic[2] - cubic[1]);
        let degenerate_cd = degenerate_vector(cubic[3] - cubic[2]);
        if degenerate_ab & degenerate_bc & degenerate_cd {
            return ReductionType::Point;
        }
        if i32::from(degenerate_ab) + i32::from(degenerate_bc) + i32::from(degenerate_cd) == 2 {
            return ReductionType::Line;
        }
        if !cubic_in_line(cubic) {
            *tangent_pt = if degenerate_ab { cubic[2] } else { cubic[1] };
            return ReductionType::Quad;
        }
        let mut t_values = [0.0; 3];
        let count = find_cubic_max_curvature(cubic, &mut t_values);
        let mut r_count = 0;
        // Now loop over the t-values, and reject any that evaluate to either end-point
        for &t in &t_values[..count] {
            if 0.0 >= t || t >= 1.0 {
                continue;
            }
            eval_cubic_at(cubic, t, Some(&mut reduction[r_count]), None, None);
            if reduction[r_count] != cubic[0] && reduction[r_count] != cubic[3] {
                r_count += 1;
            }
        }
        match r_count {
            0 => ReductionType::Line,
            1 => ReductionType::Degenerate,
            2 => ReductionType::Degenerate2,
            _ => ReductionType::Degenerate3,
        }
    }

    // Port of: src/core/SkStroke.cpp#L708-L729 (chrome/m156)
    fn check_conic_linear(conic: &Conic, reduction: &mut Point) -> ReductionType {
        let degenerate_ab = degenerate_vector(conic.pts[1] - conic.pts[0]);
        let degenerate_bc = degenerate_vector(conic.pts[2] - conic.pts[1]);
        if degenerate_ab & degenerate_bc {
            return ReductionType::Point;
        }
        if degenerate_ab | degenerate_bc {
            return ReductionType::Line;
        }
        if !conic_in_line(conic) {
            return ReductionType::Quad;
        }
        // SkFindConicMaxCurvature would be a better solution, once we know how to
        // implement it. Quad curvature is a reasonable substitute
        let t = find_quad_max_curvature(&conic.pts);
        if 0.0 == t || is_nan(t) {
            return ReductionType::Line;
        }
        conic.eval_at_pos_tangent(t, Some(reduction), None);
        ReductionType::Degenerate
    }

    // Port of: src/core/SkStroke.cpp#L731-L750 (chrome/m156)
    #[allow(clippy::float_cmp)] // exact comparison, as in Skia
    fn check_quad_linear(quad: &[Point], reduction: &mut Point) -> ReductionType {
        let degenerate_ab = degenerate_vector(quad[1] - quad[0]);
        let degenerate_bc = degenerate_vector(quad[2] - quad[1]);
        if degenerate_ab & degenerate_bc {
            return ReductionType::Point;
        }
        if degenerate_ab | degenerate_bc {
            return ReductionType::Line;
        }
        if !quad_in_line(quad) {
            return ReductionType::Quad;
        }
        let t = find_quad_max_curvature(quad);
        if 0.0 == t || 1.0 == t {
            return ReductionType::Line;
        }
        *reduction = eval_quad_at(quad, t);
        ReductionType::Degenerate
    }

    // Port of: src/core/SkStroke.cpp#L752-L788 (chrome/m156)
    fn conic_to(&mut self, pt1: Point, pt2: Point, weight: scalar) {
        let conic = Conic::new(self.prev_pt, pt1, pt2, weight);
        let mut reduction = Point::default();
        let reduction_type = Self::check_conic_linear(&conic, &mut reduction);
        if ReductionType::Point == reduction_type {
            /* If the stroke consists of a moveTo followed by a degenerate curve, treat it
            as if it were followed by a zero-length line. Lines without length
            can have square and round end caps. */
            self.line_to(pt2, None);
            return;
        }
        if ReductionType::Line == reduction_type {
            self.line_to(pt2, None);
            return;
        }
        if ReductionType::Degenerate == reduction_type {
            self.line_to(reduction, None);
            let save_joiner = self.joiner;
            self.joiner = StrokerPriv::join_factory(Join::Round);
            self.line_to(pt2, None);
            self.joiner = save_joiner;
            return;
        }
        debug_assert!(ReductionType::Quad == reduction_type);
        let mut normal_ab = Vector::default();
        let mut unit_ab = Vector::default();
        let mut normal_bc = Vector::default();
        let mut unit_bc = Vector::default();
        if !self.pre_join_to(pt1, &mut normal_ab, &mut unit_ab, false) {
            self.line_to(pt2, None);
            return;
        }
        let mut quad_pts = QuadConstruct::default();
        self.init(StrokeType::Outer, &mut quad_pts, 0.0, 1.0);
        let _ = self.conic_stroke(&conic, &mut quad_pts);
        self.init(StrokeType::Inner, &mut quad_pts, 0.0, 1.0);
        let _ = self.conic_stroke(&conic, &mut quad_pts);
        self.set_conic_end_normal(&conic, normal_ab, unit_ab, &mut normal_bc, &mut unit_bc);
        self.post_join_to(pt2, normal_bc, unit_bc);
    }

    // Port of: src/core/SkStroke.cpp#L790-L827 (chrome/m156)
    fn quad_to(&mut self, pt1: Point, pt2: Point) {
        let quad = [self.prev_pt, pt1, pt2];
        let mut reduction = Point::default();
        let reduction_type = Self::check_quad_linear(&quad, &mut reduction);
        if ReductionType::Point == reduction_type {
            /* If the stroke consists of a moveTo followed by a degenerate curve, treat it
            as if it were followed by a zero-length line. Lines without length
            can have square and round end caps. */
            self.line_to(pt2, None);
            return;
        }
        if ReductionType::Line == reduction_type {
            self.line_to(pt2, None);
            return;
        }
        if ReductionType::Degenerate == reduction_type {
            self.line_to(reduction, None);
            let save_joiner = self.joiner;
            self.joiner = StrokerPriv::join_factory(Join::Round);
            self.line_to(pt2, None);
            self.joiner = save_joiner;
            return;
        }
        debug_assert!(ReductionType::Quad == reduction_type);
        let mut normal_ab = Vector::default();
        let mut unit_ab = Vector::default();
        let mut normal_bc = Vector::default();
        let mut unit_bc = Vector::default();
        if !self.pre_join_to(pt1, &mut normal_ab, &mut unit_ab, false) {
            self.line_to(pt2, None);
            return;
        }
        let mut quad_pts = QuadConstruct::default();
        self.init(StrokeType::Outer, &mut quad_pts, 0.0, 1.0);
        let _ = self.quad_stroke(&quad, &mut quad_pts);
        self.init(StrokeType::Inner, &mut quad_pts, 0.0, 1.0);
        let _ = self.quad_stroke(&quad, &mut quad_pts);
        self.set_quad_end_normal(&quad, normal_ab, unit_ab, &mut normal_bc, &mut unit_bc);

        self.post_join_to(pt2, normal_bc, unit_bc);
    }

    // Given a point on the curve and its derivative, scale the derivative by the radius, and
    // compute the perpendicular point and its tangent.
    // Port of: src/core/SkStroke.cpp#L831-L842 (chrome/m156)
    fn set_ray_pts(
        &self,
        t_pt: Point,
        dxy: &mut Vector,
        on_pt: &mut Point,
        tangent: Option<&mut Vector>,
    ) {
        if !dxy.set_length(self.radius) {
            *dxy = Point::new(self.radius, 0.0);
        }
        let axis_flip = scalar::from(self.stroke_type as i8); // go opposite ways for outer, inner
        on_pt.x = t_pt.x + axis_flip * dxy.y;
        on_pt.y = t_pt.y - axis_flip * dxy.x;
        if let Some(tangent) = tangent {
            *tangent = *dxy;
        }
    }

    // Given a conic and t, return the point on curve, its perpendicular, and the perpendicular
    // tangent. Returns false if the perpendicular could not be computed (because the derivative
    // collapsed to 0)
    // Port of: src/core/SkStroke.cpp#L846-L854 (chrome/m156)
    fn conic_perp_ray(
        &self,
        conic: &Conic,
        t: scalar,
        t_pt: &mut Point,
        on_pt: &mut Point,
        tangent: Option<&mut Vector>,
    ) {
        let mut dxy = Vector::default();
        conic.eval_at_pos_tangent(t, Some(t_pt), Some(&mut dxy));
        if dxy.is_zero() {
            dxy = conic.pts[2] - conic.pts[0];
        }
        self.set_ray_pts(*t_pt, &mut dxy, on_pt, tangent);
    }

    // Given a conic and a t range, find the start and end if they haven't been found already.
    // Port of: src/core/SkStroke.cpp#L857-L870 (chrome/m156)
    fn conic_quad_ends(&self, conic: &Conic, quad_pts: &mut QuadConstruct) {
        if !quad_pts.start_set {
            let mut conic_start_pt = Point::default();
            let (mut q0, mut ts) = (quad_pts.quad[0], quad_pts.tangent_start);
            self.conic_perp_ray(
                conic,
                quad_pts.start_t,
                &mut conic_start_pt,
                &mut q0,
                Some(&mut ts),
            );
            quad_pts.quad[0] = q0;
            quad_pts.tangent_start = ts;
            quad_pts.start_set = true;
        }
        if !quad_pts.end_set {
            let mut conic_end_pt = Point::default();
            let (mut q2, mut te) = (quad_pts.quad[2], quad_pts.tangent_end);
            self.conic_perp_ray(
                conic,
                quad_pts.end_t,
                &mut conic_end_pt,
                &mut q2,
                Some(&mut te),
            );
            quad_pts.quad[2] = q2;
            quad_pts.tangent_end = te;
            quad_pts.end_set = true;
        }
    }

    // Given a cubic and t, return the point on curve, its perpendicular, and the perpendicular
    // tangent.
    // Port of: src/core/SkStroke.cpp#L874-L900 (chrome/m156)
    fn cubic_perp_ray(
        &self,
        cubic: &[Point],
        t: scalar,
        t_pt: &mut Point,
        on_pt: &mut Point,
        tangent: Option<&mut Vector>,
    ) {
        let mut dxy = Vector::default();
        let mut chopped = [Point::default(); 7];
        eval_cubic_at(cubic, t, Some(t_pt), Some(&mut dxy), None);
        if dxy.is_zero() {
            let mut c_pts: &[Point] = cubic;
            if t.nearly_zero(None) {
                dxy = cubic[2] - cubic[0];
            } else if (1.0 - t).nearly_zero(None) {
                dxy = cubic[3] - cubic[1];
            } else {
                // If the cubic inflection falls on the cusp, subdivide the cubic
                // to find the tangent at that point.
                chop_cubic_at(cubic, &mut chopped, t);
                dxy = chopped[3] - chopped[2];
                if dxy.is_zero() {
                    dxy = chopped[3] - chopped[1];
                    c_pts = &chopped;
                }
            }
            if dxy.is_zero() {
                dxy = c_pts[3] - c_pts[0];
            }
        }
        self.set_ray_pts(*t_pt, &mut dxy, on_pt, tangent);
    }

    // Given a cubic and a t range, find the start and end if they haven't been found already.
    // Port of: src/core/SkStroke.cpp#L903-L916 (chrome/m156)
    fn cubic_quad_ends(&self, cubic: &[Point], quad_pts: &mut QuadConstruct) {
        if !quad_pts.start_set {
            let mut cubic_start_pt = Point::default();
            let (mut q0, mut ts) = (quad_pts.quad[0], quad_pts.tangent_start);
            self.cubic_perp_ray(
                cubic,
                quad_pts.start_t,
                &mut cubic_start_pt,
                &mut q0,
                Some(&mut ts),
            );
            quad_pts.quad[0] = q0;
            quad_pts.tangent_start = ts;
            quad_pts.start_set = true;
        }
        if !quad_pts.end_set {
            let mut cubic_end_pt = Point::default();
            let (mut q2, mut te) = (quad_pts.quad[2], quad_pts.tangent_end);
            self.cubic_perp_ray(
                cubic,
                quad_pts.end_t,
                &mut cubic_end_pt,
                &mut q2,
                Some(&mut te),
            );
            quad_pts.quad[2] = q2;
            quad_pts.tangent_end = te;
            quad_pts.end_set = true;
        }
    }

    // Port of: src/core/SkStroke.cpp#L918-L922 (chrome/m156)
    fn cubic_quad_mid(&self, cubic: &[Point], quad_pts: &QuadConstruct, mid: &mut Point) {
        let mut cubic_mid_pt = Point::default();
        self.cubic_perp_ray(cubic, quad_pts.mid_t, &mut cubic_mid_pt, mid, None);
    }

    // Given a quad and t, return the point on curve, its perpendicular, and the perpendicular
    // tangent.
    // Port of: src/core/SkStroke.cpp#L925-L933 (chrome/m156)
    fn quad_perp_ray(
        &self,
        quad: &[Point],
        t: scalar,
        t_pt: &mut Point,
        on_pt: &mut Point,
        tangent: Option<&mut Vector>,
    ) {
        let mut dxy = Vector::default();
        eval_quad_at_pos_tangent(quad, t, Some(t_pt), Some(&mut dxy));
        if dxy.is_zero() {
            dxy = quad[2] - quad[0];
        }
        self.set_ray_pts(*t_pt, &mut dxy, on_pt, tangent);
    }

    // Find the intersection of the stroke tangents to construct a stroke quad.
    // Return whether the stroke is a degenerate (a line), a quad, or must be split.
    // Optionally compute the quad's control point.
    // Port of: src/core/SkStroke.cpp#L938-L989 (chrome/m156)
    fn intersect_ray(
        &self,
        quad_pts: &mut QuadConstruct,
        intersect_ray_type: IntersectRayType,
    ) -> ResultType {
        let start = quad_pts.quad[0];
        let end = quad_pts.quad[2];
        let a_len = quad_pts.tangent_start;
        let b_len = quad_pts.tangent_end;
        /* Slopes match when denom goes to zero:
                          axLen / ayLen ==                   bxLen / byLen
        (ayLen * byLen) * axLen / ayLen == (ayLen * byLen) * bxLen / byLen
                 byLen  * axLen         ==  ayLen          * bxLen
                 byLen  * axLen         -   ayLen          * bxLen         ( == denom )
         */
        let denom = a_len.cross(b_len);
        if denom == 0.0 || !denom.is_finite() {
            quad_pts.opposite_tangents = a_len.dot(b_len) < 0.0;
            return ResultType::Degenerate;
        }
        quad_pts.opposite_tangents = false;
        let ab0 = start - end;
        let mut numer_a = b_len.cross(ab0);
        let numer_b = a_len.cross(ab0);
        if (numer_a >= 0.0) == (numer_b >= 0.0) {
            // if the control point is outside the quad ends
            // if the perpendicular distances from the quad points to the opposite tangent line
            // are small, a straight line is good enough
            let dist1 = pt_to_tangent_line(start, end, quad_pts.tangent_end);
            let dist2 = pt_to_tangent_line(end, start, quad_pts.tangent_start);
            if std_max(dist1, dist2) <= self.inv_res_scale_squared {
                return ResultType::Degenerate;
            }
            return ResultType::Split;
        }
        // check to see if the denominator is teeny relative to the numerator
        // if the offset by one will be lost, the ratio is too large
        numer_a /= denom;
        let valid_divide = numer_a > numer_a - 1.0;
        if valid_divide {
            if IntersectRayType::CtrlPt == intersect_ray_type {
                // the intersection of the tangents need not be on the tangent segment
                // so 0 <= numerA <= 1 is not necessarily true
                quad_pts.quad[1] = start + quad_pts.tangent_start * numer_a;
            }
            return ResultType::Quad;
        }
        quad_pts.opposite_tangents = a_len.dot(b_len) < 0.0;
        // if the lines are parallel, straight line is good enough
        ResultType::Degenerate
    }

    // Given a cubic and a t-range, determine if the stroke can be described by a quadratic.
    // Port of: src/core/SkStroke.cpp#L992-L996 (chrome/m156)
    fn tangents_meet(&self, cubic: &[Point], quad_pts: &mut QuadConstruct) -> ResultType {
        self.cubic_quad_ends(cubic, quad_pts);
        self.intersect_ray(quad_pts, IntersectRayType::ResultType)
    }

    // Return true if the point is close to the bounds of the quad. This is used as a quick
    // reject.
    // Port of: src/core/SkStroke.cpp#L1014-L1032 (chrome/m156)
    fn pt_in_quad_bounds(&self, quad: &[Point; 3], pt: Point) -> bool {
        let x_min = min3(quad[0].x, quad[1].x, quad[2].x);
        if pt.x + self.inv_res_scale < x_min {
            return false;
        }
        let x_max = max3(quad[0].x, quad[1].x, quad[2].x);
        if pt.x - self.inv_res_scale > x_max {
            return false;
        }
        let y_min = min3(quad[0].y, quad[1].y, quad[2].y);
        if pt.y + self.inv_res_scale < y_min {
            return false;
        }
        let y_max = max3(quad[0].y, quad[1].y, quad[2].y);
        if pt.y - self.inv_res_scale > y_max {
            return false;
        }
        true
    }

    // Port of: src/core/SkStroke.cpp#L1055-L1102 (chrome/m156)
    fn stroke_close_enough(
        &self,
        stroke: &[Point; 3],
        ray: &[Point; 2],
        quad_pts: &QuadConstruct,
    ) -> ResultType {
        let stroke_mid = eval_quad_at(stroke, SCALAR_HALF);
        // measure the distance from the curve to the quad-stroke midpoint, compare to radius
        if points_within_dist(ray[0], stroke_mid, self.inv_res_scale) {
            // if the difference is small
            if sharp_angle(&quad_pts.quad) {
                return ResultType::Split;
            }
            return ResultType::Quad;
        }
        // measure the distance to quad's bounds (quick reject)
        // an alternative : look for point in triangle
        if !self.pt_in_quad_bounds(stroke, ray[0]) {
            // if far, subdivide
            return ResultType::Split;
        }
        // measure the curve ray distance to the quad-stroke
        let mut roots = [0.0; 2];
        let root_count = intersect_quad_ray(ray, stroke, &mut roots);
        if root_count != 1 {
            return ResultType::Split;
        }
        let quad_pt = eval_quad_at(stroke, roots[0]);
        let error = self.inv_res_scale * (SCALAR_1 - (roots[0] - 0.5).abs() * 2.0);
        if points_within_dist(ray[0], quad_pt, error) {
            // if the difference is small, we're done
            if sharp_angle(&quad_pts.quad) {
                return ResultType::Split;
            }
            return ResultType::Quad;
        }
        // otherwise, subdivide
        ResultType::Split
    }

    // Port of: src/core/SkStroke.cpp#L1104-L1118 (chrome/m156)
    fn compare_quad_cubic(&self, cubic: &[Point], quad_pts: &mut QuadConstruct) -> ResultType {
        // get the quadratic approximation of the stroke
        self.cubic_quad_ends(cubic, quad_pts);
        let result_type = self.intersect_ray(quad_pts, IntersectRayType::CtrlPt);
        if result_type != ResultType::Quad {
            return result_type;
        }
        // project a ray from the curve to the stroke
        // points near midpoint on quad, midpoint on cubic
        let (mut r0, mut r1) = (Point::default(), Point::default());
        self.cubic_perp_ray(cubic, quad_pts.mid_t, &mut r1, &mut r0, None);
        let ray = [r0, r1];
        self.stroke_close_enough(&quad_pts.quad, &ray, quad_pts)
    }

    // Port of: src/core/SkStroke.cpp#L1120-L1134 (chrome/m156)
    fn compare_quad_conic(&self, conic: &Conic, quad_pts: &mut QuadConstruct) -> ResultType {
        // get the quadratic approximation of the stroke
        self.conic_quad_ends(conic, quad_pts);
        let result_type = self.intersect_ray(quad_pts, IntersectRayType::CtrlPt);
        if result_type != ResultType::Quad {
            return result_type;
        }
        // project a ray from the curve to the stroke
        let (mut r0, mut r1) = (Point::default(), Point::default()); // points near midpoint on quad, midpoint on conic
        self.conic_perp_ray(conic, quad_pts.mid_t, &mut r1, &mut r0, None);
        let ray = [r0, r1];
        self.stroke_close_enough(&quad_pts.quad, &ray, quad_pts)
    }

    // Port of: src/core/SkStroke.cpp#L1136-L1161 (chrome/m156)
    fn compare_quad_quad(&self, quad: &[Point], quad_pts: &mut QuadConstruct) -> ResultType {
        // get the quadratic approximation of the stroke
        if !quad_pts.start_set {
            let mut quad_start_pt = Point::default();
            let (mut q0, mut ts) = (quad_pts.quad[0], quad_pts.tangent_start);
            self.quad_perp_ray(
                quad,
                quad_pts.start_t,
                &mut quad_start_pt,
                &mut q0,
                Some(&mut ts),
            );
            quad_pts.quad[0] = q0;
            quad_pts.tangent_start = ts;
            quad_pts.start_set = true;
        }
        if !quad_pts.end_set {
            let mut quad_end_pt = Point::default();
            let (mut q2, mut te) = (quad_pts.quad[2], quad_pts.tangent_end);
            self.quad_perp_ray(
                quad,
                quad_pts.end_t,
                &mut quad_end_pt,
                &mut q2,
                Some(&mut te),
            );
            quad_pts.quad[2] = q2;
            quad_pts.tangent_end = te;
            quad_pts.end_set = true;
        }
        let result_type = self.intersect_ray(quad_pts, IntersectRayType::CtrlPt);
        if result_type != ResultType::Quad {
            return result_type;
        }
        // project a ray from the curve to the stroke
        let (mut r0, mut r1) = (Point::default(), Point::default());
        self.quad_perp_ray(quad, quad_pts.mid_t, &mut r1, &mut r0, None);
        let ray = [r0, r1];
        self.stroke_close_enough(&quad_pts.quad, &ray, quad_pts)
    }

    // Port of: src/core/SkStroke.cpp#L1163-L1167 (chrome/m156)
    fn add_degenerate_line(&mut self, quad_pts: &QuadConstruct) {
        let quad = &quad_pts.quad;
        self.sink().line_to(quad[2]);
    }

    // Port of: src/core/SkStroke.cpp#L1169-L1174 (chrome/m156)
    fn cubic_mid_on_line(&self, cubic: &[Point], quad_pts: &QuadConstruct) -> bool {
        let mut stroke_mid = Point::default();
        self.cubic_quad_mid(cubic, quad_pts, &mut stroke_mid);
        let dist = pt_to_line(stroke_mid, quad_pts.quad[0], quad_pts.quad[2]);
        dist < self.inv_res_scale_squared
    }

    // Port of: src/core/SkStroke.cpp#L1176-L1244 (chrome/m156)
    fn cubic_stroke(&mut self, cubic: &[Point], quad_pts: &mut QuadConstruct) -> bool {
        if !self.found_tangents {
            let result_type = self.tangents_meet(cubic, quad_pts);
            if ResultType::Quad == result_type {
                self.found_tangents = true;
            } else if (ResultType::Degenerate == result_type
                || points_within_dist(quad_pts.quad[0], quad_pts.quad[2], self.inv_res_scale))
                && self.cubic_mid_on_line(cubic, quad_pts)
            {
                self.add_degenerate_line(quad_pts);
                return true;
            }
        }
        if self.found_tangents {
            let result_type = self.compare_quad_cubic(cubic, quad_pts);
            if ResultType::Quad == result_type {
                let stroke = quad_pts.quad;
                self.sink().quad_to(stroke[1], stroke[2]);
                return true;
            }
            if ResultType::Degenerate == result_type && !quad_pts.opposite_tangents {
                self.add_degenerate_line(quad_pts);
                return true;
            }
        }
        if !quad_pts.quad[2].is_finite() {
            return false; // just abort if projected quad isn't representable
        }
        self.recursion_depth += 1;
        if self.recursion_depth > RECURSIVE_LIMITS[usize::from(self.found_tangents)] {
            // If we stop making progress, just emit a line and move on
            self.add_degenerate_line(quad_pts);
            self.recursion_depth -= 1;
            return true;
        }
        let mut half = QuadConstruct::default();
        if !half.init_with_start(quad_pts) {
            self.add_degenerate_line(quad_pts);
            self.recursion_depth -= 1;
            return true;
        }
        if !self.cubic_stroke(cubic, &mut half) {
            return false;
        }
        if !half.init_with_end(quad_pts) {
            self.add_degenerate_line(quad_pts);
            self.recursion_depth -= 1;
            return true;
        }
        if !self.cubic_stroke(cubic, &mut half) {
            return false;
        }
        self.recursion_depth -= 1;
        true
    }

    // Port of: src/core/SkStroke.cpp#L1246-L1290 (chrome/m156)
    fn conic_stroke(&mut self, conic: &Conic, quad_pts: &mut QuadConstruct) -> bool {
        let result_type = self.compare_quad_conic(conic, quad_pts);
        if ResultType::Quad == result_type {
            let stroke = quad_pts.quad;
            self.sink().quad_to(stroke[1], stroke[2]);
            return true;
        }
        if ResultType::Degenerate == result_type {
            self.add_degenerate_line(quad_pts);
            return true;
        }
        if !quad_pts.quad[2].is_finite() {
            return false; // just abort if projected quad isn't representable
        }
        self.recursion_depth += 1;
        if self.recursion_depth > RECURSIVE_LIMITS[CONIC_RECURSIVE_LIMIT] {
            // If we stop making progress, just emit a line and move on
            self.add_degenerate_line(quad_pts);
            self.recursion_depth -= 1;
            return true;
        }
        let mut half = QuadConstruct::default();
        if !half.init_with_start(quad_pts) {
            self.add_degenerate_line(quad_pts);
            self.recursion_depth -= 1;
            return true;
        }
        if !self.conic_stroke(conic, &mut half) {
            return false;
        }
        if !half.init_with_end(quad_pts) {
            self.add_degenerate_line(quad_pts);
            self.recursion_depth -= 1;
            return true;
        }
        if !self.conic_stroke(conic, &mut half) {
            return false;
        }
        self.recursion_depth -= 1;
        true
    }

    // Port of: src/core/SkStroke.cpp#L1292-L1336 (chrome/m156)
    fn quad_stroke(&mut self, quad: &[Point], quad_pts: &mut QuadConstruct) -> bool {
        let result_type = self.compare_quad_quad(quad, quad_pts);
        if ResultType::Quad == result_type {
            let stroke = quad_pts.quad;
            self.sink().quad_to(stroke[1], stroke[2]);
            return true;
        }
        if ResultType::Degenerate == result_type {
            self.add_degenerate_line(quad_pts);
            return true;
        }
        if !quad_pts.quad[2].is_finite() {
            return false; // just abort if projected quad isn't representable
        }
        self.recursion_depth += 1;
        if self.recursion_depth > RECURSIVE_LIMITS[QUAD_RECURSIVE_LIMIT] {
            // If we stop making progress, just emit a line and move on
            self.add_degenerate_line(quad_pts);
            self.recursion_depth -= 1;
            return true;
        }
        let mut half = QuadConstruct::default();
        if !half.init_with_start(quad_pts) {
            self.add_degenerate_line(quad_pts);
            self.recursion_depth -= 1;
            return true;
        }
        if !self.quad_stroke(quad, &mut half) {
            return false;
        }
        if !half.init_with_end(quad_pts) {
            self.add_degenerate_line(quad_pts);
            self.recursion_depth -= 1;
            return true;
        }
        if !self.quad_stroke(quad, &mut half) {
            return false;
        }
        self.recursion_depth -= 1;
        true
    }

    // Port of: src/core/SkStroke.cpp#L1338-L1398 (chrome/m156)
    fn cubic_to(&mut self, pt1: Point, pt2: Point, pt3: Point) {
        let cubic = [self.prev_pt, pt1, pt2, pt3];
        let mut reduction = [Point::default(); 3];
        let mut tangent_pt = Point::default();
        let reduction_type = Self::check_cubic_linear(&cubic, &mut reduction, &mut tangent_pt);
        if ReductionType::Point == reduction_type {
            /* If the stroke consists of a moveTo followed by a degenerate curve, treat it
            as if it were followed by a zero-length line. Lines without length
            can have square and round end caps. */
            self.line_to(pt3, None);
            return;
        }
        if ReductionType::Line == reduction_type {
            self.line_to(pt3, None);
            return;
        }
        if ReductionType::Degenerate as i32 <= reduction_type as i32
            && ReductionType::Degenerate3 as i32 >= reduction_type as i32
        {
            self.line_to(reduction[0], None);
            let save_joiner = self.joiner;
            self.joiner = StrokerPriv::join_factory(Join::Round);
            if ReductionType::Degenerate2 as i32 <= reduction_type as i32 {
                self.line_to(reduction[1], None);
            }
            if ReductionType::Degenerate3 == reduction_type {
                self.line_to(reduction[2], None);
            }
            self.line_to(pt3, None);
            self.joiner = save_joiner;
            return;
        }
        debug_assert!(ReductionType::Quad == reduction_type);
        let mut normal_ab = Vector::default();
        let mut unit_ab = Vector::default();
        let mut normal_cd = Vector::default();
        let mut unit_cd = Vector::default();
        if !self.pre_join_to(tangent_pt, &mut normal_ab, &mut unit_ab, false) {
            self.line_to(pt3, None);
            return;
        }
        let mut t_values = [0.0; 2];
        let count = find_cubic_inflections(&cubic, &mut t_values);
        let mut last_t = 0.0;
        #[allow(clippy::needless_range_loop)] // `index == count` selects the end t
        for index in 0..=count {
            let next_t = if index < count { t_values[index] } else { 1.0 };
            let mut quad_pts = QuadConstruct::default();
            self.init(StrokeType::Outer, &mut quad_pts, last_t, next_t);
            let _ = self.cubic_stroke(&cubic, &mut quad_pts);
            self.init(StrokeType::Inner, &mut quad_pts, last_t, next_t);
            let _ = self.cubic_stroke(&cubic, &mut quad_pts);
            last_t = next_t;
        }
        let cusp = find_cubic_cusp(&cubic);
        if cusp > 0.0 {
            let mut cusp_loc = Point::default();
            eval_cubic_at(&cubic, cusp, Some(&mut cusp_loc), None, None);
            self.cusper
                .add_circle((cusp_loc.x, cusp_loc.y), self.radius, None);
        }
        // emit the join even if one stroke succeeded but the last one failed
        // this avoids reversing an inner stroke with a partial path followed by another moveto
        self.set_cubic_end_normal(&cubic, normal_ab, unit_ab, &mut normal_cd, &mut unit_cd);

        self.post_join_to(pt3, normal_cd, unit_cd);
    }
}

// `std::min({a, b, c})`: the first smallest element (`(b < a) ? b : a`, folded).
fn min3(a: scalar, b: scalar, c: scalar) -> scalar {
    std_min(std_min(a, b), c)
}

// `std::max({a, b, c})`: the first largest element (`(a < b) ? b : a`, folded).
fn max3(a: scalar, b: scalar, c: scalar) -> scalar {
    std_max(std_max(a, b), c)
}

// Port of: src/core/SkStroke.cpp#L452-L480 (chrome/m156)
fn has_valid_tangent(iter: &Iter<'_>) -> bool {
    let mut copy = iter.clone();
    while let Some(rec) = copy.next_rec() {
        let pts = rec.points();
        match rec.verb() {
            PathVerb::Move | PathVerb::Close => return false,
            PathVerb::Line => {
                if pts[0] == pts[1] {
                    continue;
                }
                return true;
            }
            PathVerb::Quad | PathVerb::Conic => {
                if pts[0] == pts[1] && pts[0] == pts[2] {
                    continue;
                }
                return true;
            }
            PathVerb::Cubic => {
                if pts[0] == pts[1] && pts[0] == pts[2] && pts[0] == pts[3] {
                    continue;
                }
                return true;
            }
        }
    }
    false
}

// returns the distance squared from the point to the line
// Port of: src/core/SkStroke.cpp#L550-L562 (chrome/m156)
fn pt_to_line(pt: Point, line_start: Point, line_end: Point) -> scalar {
    let dxy = line_end - line_start;
    let ab0 = pt - line_start;
    let numer = dxy.dot(ab0);
    let denom = dxy.dot(dxy);
    let t = ieee_float_divide(numer, denom);
    if (0.0..=1.0).contains(&t) {
        let hit = line_start * (1.0 - t) + line_end * t;
        point_priv::distance_to_sqd(hit, pt)
    } else {
        point_priv::distance_to_sqd(pt, line_start)
    }
}

// returns the distance squared from the point to the line
// Port of: src/core/SkStroke.cpp#L565-L579 (chrome/m156)
fn pt_to_tangent_line(pt: Point, line_start: Point, tangent: Vector) -> scalar {
    let dxy = tangent;
    let ab0 = pt - line_start;
    let numer = dxy.dot(ab0);
    let denom = dxy.dot(dxy);
    let t = ieee_float_divide(numer, denom);
    if (0.0..=1.0).contains(&t) {
        let hit = line_start + tangent * t;
        point_priv::distance_to_sqd(hit, pt)
    } else {
        point_priv::distance_to_sqd(pt, line_start)
    }
}

/*  Given a cubic, determine if all four points are in a line.
    Return true if the inner points is close to a line connecting the outermost points.

    Find the outermost point by looking for the largest difference in X or Y.
    Given the indices of the outermost points, and that outer_1 is greater than outer_2,
    this table shows the index of the smaller of the remaining points:

                      outer_2
                  0    1    2    3
      outer_1     ----------------
         0     |  -    2    1    1
         1     |  -    -    0    0
         2     |  -    -    -    0
         3     |  -    -    -    -

    If outer_1 == 0 and outer_2 == 1, the smaller of the remaining indices (2 and 3) is 2.

    This table can be collapsed to: (1 + (2 >> outer_2)) >> outer_1

    Given three indices (outer_1 outer_2 mid_1) from 0..3, the remaining index is:

               mid_2 == (outer_1 ^ outer_2 ^ mid_1)
*/
// Port of: src/core/SkStroke.cpp#L604-L632 (chrome/m156)
fn cubic_in_line(cubic: &[Point]) -> bool {
    let mut pt_max: scalar = -1.0;
    let mut outer1: usize = 0;
    let mut outer2: usize = 0;
    for index in 0..3 {
        for inner in (index + 1)..4 {
            let test_diff = cubic[inner] - cubic[index];
            let test_max = std_max(test_diff.x.abs(), test_diff.y.abs());
            if pt_max < test_max {
                outer1 = index;
                outer2 = inner;
                pt_max = test_max;
            }
        }
    }
    debug_assert!(outer1 <= 2);
    debug_assert!((1..=3).contains(&outer2));
    debug_assert!(outer1 < outer2);
    let mid1 = (1 + (2 >> outer2)) >> outer1;
    debug_assert!(mid1 <= 2);
    debug_assert!(outer1 != mid1 && outer2 != mid1);
    let mid2 = outer1 ^ outer2 ^ mid1;
    debug_assert!((1..=3).contains(&mid2));
    debug_assert!(mid2 != outer1 && mid2 != outer2 && mid2 != mid1);
    debug_assert_eq!(
        (1 << outer1) | (1 << outer2) | (1 << mid1) | (1 << mid2),
        0x0f
    );
    let line_slop = pt_max * pt_max * 0.00001; // this multiplier is pulled out of the air
    pt_to_line(cubic[mid1], cubic[outer1], cubic[outer2]) <= line_slop
        && pt_to_line(cubic[mid2], cubic[outer1], cubic[outer2]) <= line_slop
}

/* Given quad, see if all there points are in a line.
   Return true if the inside point is close to a line connecting the outermost points.

   Find the outermost point by looking for the largest difference in X or Y.
   Since the XOR of the indices is 3  (0 ^ 1 ^ 2)
   the missing index equals: outer_1 ^ outer_2 ^ 3
*/
// Port of: src/core/SkStroke.cpp#L641-L663 (chrome/m156)
fn quad_in_line(quad: &[Point]) -> bool {
    const CURVATURE_SLOP: f32 = 0.000_005; // this multiplier is pulled out of the air
    let mut pt_max: scalar = -1.0;
    let mut outer1: usize = 0;
    let mut outer2: usize = 0;
    for index in 0..2 {
        for inner in (index + 1)..3 {
            let test_diff = quad[inner] - quad[index];
            let test_max = std_max(test_diff.x.abs(), test_diff.y.abs());
            if pt_max < test_max {
                outer1 = index;
                outer2 = inner;
                pt_max = test_max;
            }
        }
    }
    debug_assert!(outer1 <= 1);
    debug_assert!((1..=2).contains(&outer2));
    debug_assert!(outer1 < outer2);
    let mid = outer1 ^ outer2 ^ 3;
    let line_slop = pt_max * pt_max * CURVATURE_SLOP;
    pt_to_line(quad[mid], quad[outer1], quad[outer2]) <= line_slop
}

// Port of: src/core/SkStroke.cpp#L665-L667 (chrome/m156)
fn conic_in_line(conic: &Conic) -> bool {
    quad_in_line(&conic.pts)
}

// Intersect the line with the quad and return the t values on the quad where the line crosses.
// Port of: src/core/SkStroke.cpp#L999-L1011 (chrome/m156)
fn intersect_quad_ray(line: &[Point; 2], quad: &[Point; 3], roots: &mut [scalar; 2]) -> usize {
    let vec = line[1] - line[0];
    let mut r = [0.0; 3];
    for n in 0..3 {
        r[n] = vec.cross(quad[n] - line[0]);
    }
    let mut a = r[2];
    let mut b = r[1];
    let c = r[0];
    a += c - 2.0 * b; // A = a - 2*b + c
    b -= c; // B = -(b - c)
    find_unit_quad_roots(a, 2.0 * b, c, roots)
}

// Port of: src/core/SkStroke.cpp#L1034-L1036 (chrome/m156)
fn points_within_dist(near_pt: Point, far_pt: Point, limit: scalar) -> bool {
    point_priv::distance_to_sqd(near_pt, far_pt) <= limit * limit
}

// Port of: src/core/SkStroke.cpp#L1038-L1053 (chrome/m156)
fn sharp_angle(quad: &[Point; 3]) -> bool {
    let mut smaller = quad[1] - quad[0];
    let mut larger = quad[1] - quad[2];
    let smaller_len = point_priv::length_sqd(smaller);
    let mut larger_len = point_priv::length_sqd(larger);
    if smaller_len > larger_len {
        std::mem::swap(&mut smaller, &mut larger);
        larger_len = smaller_len;
    }
    if !smaller.set_length(larger_len) {
        return false;
    }
    let dot = smaller.dot(larger);
    dot > 0.0
}

///////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////

/// Constructs paths by stroking geometries (`SkStroke`).
// Port of: src/core/SkStroke.h#L35-L84 (chrome/m156)
#[doc(alias = "SkStroke")]
#[derive(Copy, Clone, Debug)]
pub struct Stroke {
    width: scalar,
    miter_limit: scalar,
    res_scale: scalar,
    cap: Cap,
    join: Join,
    do_fill: bool,
}

impl Default for Stroke {
    fn default() -> Self {
        Self::new()
    }
}

impl Stroke {
    /// A stroke with width 1, the default miter limit, cap and join (`SkStroke()`).
    ///
    /// skia-rust: the `SkStroke(const SkPaint&)` constructors need `SkPaint`, which is not
    /// ported yet; use the setters.
    // Port of: src/core/SkStroke.cpp#L1405-L1412 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self {
            width: SCALAR_1,
            miter_limit: DEFAULT_MITER_LIMIT,
            res_scale: 1.0,
            cap: Cap::DEFAULT,
            join: Join::DEFAULT,
            do_fill: false,
        }
    }

    /// The cap.
    #[doc(alias = "getCap")]
    #[must_use]
    pub fn cap(&self) -> Cap {
        self.cap
    }

    /// Sets the cap.
    // Port of: src/core/SkStroke.cpp#L1442-L1445 (chrome/m156)
    #[doc(alias = "setCap")]
    pub fn set_cap(&mut self, cap: Cap) {
        self.cap = cap;
    }

    /// The join.
    #[doc(alias = "getJoin")]
    #[must_use]
    pub fn join(&self) -> Join {
        self.join
    }

    /// Sets the join.
    // Port of: src/core/SkStroke.cpp#L1447-L1450 (chrome/m156)
    #[doc(alias = "setJoin")]
    pub fn set_join(&mut self, join: Join) {
        self.join = join;
    }

    /// Sets the miter limit.
    // Port of: src/core/SkStroke.cpp#L1437-L1440 (chrome/m156)
    #[doc(alias = "setMiterLimit")]
    pub fn set_miter_limit(&mut self, miter_limit: scalar) {
        debug_assert!(miter_limit >= 0.0);
        self.miter_limit = miter_limit;
    }

    /// Sets the stroke width.
    // Port of: src/core/SkStroke.cpp#L1432-L1435 (chrome/m156)
    #[doc(alias = "setWidth")]
    pub fn set_width(&mut self, width: scalar) {
        debug_assert!(width >= 0.0);
        self.width = width;
    }

    /// True if the original geometry is filled as well as stroked.
    #[doc(alias = "getDoFill")]
    #[must_use]
    pub fn do_fill(&self) -> bool {
        self.do_fill
    }

    /// Sets whether the original geometry is filled as well as stroked.
    #[doc(alias = "setDoFill")]
    pub fn set_do_fill(&mut self, do_fill: bool) {
        self.do_fill = do_fill;
    }

    /// The "intended" resolution for the output. Default is 1.0. Larger values (res > 1)
    /// indicate that the result should be more precise, since it will be zoomed up, and small
    /// errors will be magnified. Smaller values (0 < res < 1) indicate that the result can be
    /// less precise, since it will be zoomed down, and small errors may be invisible.
    #[doc(alias = "getResScale")]
    #[must_use]
    pub fn res_scale(&self) -> scalar {
        self.res_scale
    }

    /// Sets the resolution scale; `rs` must be positive and finite.
    // Port of: src/core/SkStroke.h#L60-L63 (chrome/m156)
    #[doc(alias = "setResScale")]
    pub fn set_res_scale(&mut self, rs: scalar) {
        debug_assert!(rs > 0.0 && rs.is_finite());
        self.res_scale = rs;
    }

    /// Strokes `src` into `dst`.
    // Port of: src/core/SkStroke.cpp#L1454-L1574 (chrome/m156)
    #[doc(alias = "strokePath")]
    pub fn stroke_path(&self, src: &Path, dst: &mut PathBuilder) {
        let radius: scalar = self.width / 2.0;

        if radius <= 0.0 {
            return;
        }

        let Some(raw) = path_priv::raw(src, ResolveConvexity::No) else {
            return;
        };

        // If src is really a rect, call our specialty strokeRect() method
        if let Some((rect, is_closed, dir)) = src.is_rect()
            && is_closed
        {
            self.stroke_rect(&rect, dst, dir);
            // our answer should preserve the inverseness of the src
            if src.is_inverse_fill_type() {
                debug_assert!(!dst.is_inverse_fill_type());
                dst.toggle_inverse_fill_type();
            }
            return;
        }

        // We can always ignore centers for stroke and fill convex line-only paths
        // TODO: remove the line-only restriction
        let ignore_center = self.do_fill
            && (src.segment_masks() == PathSegmentMask::LINE)
            && src.is_last_contour_closed()
            && src.is_convex();

        let mut stroker = PathStroker::new(
            src,
            radius,
            self.miter_limit,
            self.cap,
            self.join,
            self.res_scale,
            ignore_center,
        );

        let mut iter = Iter::new(src, false);
        let mut last_segment = PathVerb::Move;
        while let Some(rec) = iter.next_rec() {
            let pts = rec.points();
            match rec.verb() {
                PathVerb::Move => {
                    stroker.move_to(pts[0]);
                }
                PathVerb::Line => {
                    stroker.line_to(pts[1], Some(&iter));
                    last_segment = PathVerb::Line;
                }
                PathVerb::Quad => {
                    stroker.quad_to(pts[1], pts[2]);
                    last_segment = PathVerb::Quad;
                }
                PathVerb::Conic => {
                    stroker.conic_to(pts[1], pts[2], rec.conic_weight());
                    last_segment = PathVerb::Conic;
                }
                PathVerb::Cubic => {
                    stroker.cubic_to(pts[1], pts[2], pts[3]);
                    last_segment = PathVerb::Cubic;
                }
                PathVerb::Close => {
                    if Cap::Butt != self.cap {
                        /* If the stroke consists of a moveTo followed by a close, treat it
                        as if it were followed by a zero-length line. Lines without length
                        can have square and round end caps. */
                        if stroker.has_only_move_to() {
                            let pt = stroker.move_to_pt();
                            stroker.line_to(pt, None);
                            // goto ZERO_LENGTH
                            last_segment = PathVerb::Line;
                            continue;
                        }
                        /* If the stroke consists of a moveTo followed by one or more zero-length
                        verbs, then followed by a close, treat is as if it were followed by a
                        zero-length line. Lines without length can have square & round end caps. */
                        if stroker.is_current_contour_empty() {
                            // ZERO_LENGTH:
                            last_segment = PathVerb::Line;
                            continue;
                        }
                    }
                    stroker.close(last_segment == PathVerb::Line);
                }
            }
        }
        stroker.done(dst, last_segment == PathVerb::Line);

        if self.do_fill && !ignore_center {
            let d = path_priv::compute_first_direction(&raw);
            if d == PathFirstDirection::CCW {
                dst.private_reverse_add_path(src);
            } else {
                dst.add_path(src, None);
            }
        }

        // our answer should preserve the inverseness of the src
        if src.is_inverse_fill_type() {
            debug_assert!(!dst.is_inverse_fill_type());
            dst.toggle_inverse_fill_type();
        }
    }

    /// Strokes the rect, winding it in the specified direction (`dir`; `PathDirection::CW` is
    /// the C++ default).
    // Port of: src/core/SkStroke.cpp#L1608-L1656 (chrome/m156)
    #[doc(alias = "strokeRect")]
    pub fn stroke_rect(&self, orig_rect: &Rect, dst: &mut PathBuilder, dir: PathDirection) {
        let mut dir = dir;
        dst.reset();

        let radius: scalar = self.width / 2.0;
        if radius <= 0.0 {
            return;
        }

        let mut rw = orig_rect.width();
        let mut rh = orig_rect.height();
        if (rw < 0.0) ^ (rh < 0.0) {
            dir = reverse_direction(dir);
        }
        let mut rect = *orig_rect;
        rect.sort();
        // reassign these, now that we know they'll be >= 0
        rw = rect.width();
        rh = rect.height();

        let mut r = rect;
        r.outset((radius, radius));

        let mut join = self.join;
        if Join::Miter == join && self.miter_limit < SCALAR_SQRT2 {
            join = Join::Bevel;
        }

        match join {
            Join::Miter => {
                dst.add_rect(r, dir, None);
            }
            Join::Bevel => {
                add_bevel(dst, &rect, &r, dir);
            }
            Join::Round => {
                dst.add_rrect(RRect::new_rect_xy(r, radius, radius), dir, None);
            }
        }

        if self.width < std_min(rw, rh) && !self.do_fill {
            r = rect;
            r.inset((radius, radius));
            dst.add_rect(r, reverse_direction(dir), None);
        }
    }
}

// Port of: src/core/SkStroke.cpp#L1576-L1580 (chrome/m156)
fn reverse_direction(dir: PathDirection) -> PathDirection {
    match dir {
        PathDirection::CW => PathDirection::CCW,
        PathDirection::CCW => PathDirection::CW,
    }
}

// Port of: src/core/SkStroke.cpp#L1582-L1606 (chrome/m156)
fn add_bevel(path: &mut PathBuilder, r: &Rect, outer: &Rect, dir: PathDirection) {
    let mut pts = [Point::default(); 8];

    if PathDirection::CW == dir {
        pts[0] = Point::new(r.left, outer.top);
        pts[1] = Point::new(r.right, outer.top);
        pts[2] = Point::new(outer.right, r.top);
        pts[3] = Point::new(outer.right, r.bottom);
        pts[4] = Point::new(r.right, outer.bottom);
        pts[5] = Point::new(r.left, outer.bottom);
        pts[6] = Point::new(outer.left, r.bottom);
        pts[7] = Point::new(outer.left, r.top);
    } else {
        pts[7] = Point::new(r.left, outer.top);
        pts[6] = Point::new(r.right, outer.top);
        pts[5] = Point::new(outer.right, r.top);
        pts[4] = Point::new(outer.right, r.bottom);
        pts[3] = Point::new(r.right, outer.bottom);
        pts[2] = Point::new(r.left, outer.bottom);
        pts[1] = Point::new(outer.left, r.bottom);
        pts[0] = Point::new(outer.left, r.top);
    }
    path.add_polygon(&pts, true);
}
