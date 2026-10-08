// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkDQuadLineIntersection.cpp, src/pathops/SkDConicLineIntersection.cpp
// LineQuadraticIntersections and LineConicIntersections differ only in their root finding and
// their setMax, so the shared logic is written once over the `LineCurve` trait.

//! Quadratic and conic / line intersections, and the `SkIntersections` entry points for them.

use skia_rust_core::path::Verb;

use crate::conic::DConic;
use crate::curve::{DCurve, curve_near_point};
use crate::intersections::Intersections;
use crate::line::DLine;
use crate::point::DPoint;
use crate::quad::DQuad;
use crate::types::{
    approximately_equal, approximately_one_or_less_double, approximately_zero_or_more_double, pin_t,
};

/// `PinTPoint` of the line/curve intersectors.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum PinTPoint {
    Uninitialized,
    Initialized,
}

/// The curves that intersect a line through a quadratic-style root finder: [`DQuad`] and
/// [`DConic`].
pub(crate) trait LineCurve: Copy {
    /// The `setMax` of the intersector's constructor.
    const MAX_INTERSECTIONS: usize;
    /// The verb `SkDCurve::nearPoint` is called with.
    const VERB: Verb;
    /// `fQuad[n]` / `fConic[n]`.
    fn point(&self, n: usize) -> DPoint;
    /// `ptAtT(t)`.
    fn pt_at_t(&self, t: f64) -> DPoint;
    /// The curve as the `SkDCurve` sum type.
    fn as_dcurve(&self) -> DCurve;
    /// `intersectRay(roots)` of the intersector: the roots in the line's frame.
    fn ray_roots(&self, line: &DLine, roots: &mut [f64; 2]) -> usize;
    /// `horizontalIntersect(axisIntercept, roots)`.
    fn horizontal_roots(&self, axis_intercept: f64, roots: &mut [f64; 2]) -> usize;
    /// `verticalIntersect(axisIntercept, roots)`.
    fn vertical_roots(&self, axis_intercept: f64, roots: &mut [f64; 2]) -> usize;
}

/// `LineConicIntersections::validT`: the roots of a conic against an axis.
// Port of: src/pathops/SkDConicLineIntersection.cpp (chrome/m156)
#[allow(clippy::many_single_char_names)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
fn conic_valid_t(conic: &DConic, r: [f64; 3], axis_intercept: f64, roots: &mut [f64; 2]) -> usize {
    let w = f64::from(conic.weight);
    let mut a = r[2];
    let mut b = r[1] * w - axis_intercept * w + axis_intercept;
    let mut c = r[0];
    a += c - 2.0 * b; // A = a + c - 2*(b*w - xCept*w + xCept)
    b -= c; // B = b*w - w * xCept + xCept - a
    c -= axis_intercept;
    DQuad::roots_valid_t(a, 2.0 * b, c, roots)
}

impl LineCurve for DQuad {
    const MAX_INTERSECTIONS: usize = 5;
    const VERB: Verb = Verb::Quad;

    fn point(&self, n: usize) -> DPoint {
        self.pts[n]
    }

    fn pt_at_t(&self, t: f64) -> DPoint {
        DQuad::pt_at_t(self, t)
    }

    fn as_dcurve(&self) -> DCurve {
        DCurve::Quad(*self)
    }

    // Port of: src/pathops/SkDQuadLineIntersection.cpp (chrome/m156)
    fn ray_roots(&self, line: &DLine, roots: &mut [f64; 2]) -> usize {
        // Solve by rotating line+quad so the line is horizontal, then finding the roots.
        let adj = line[1].x - line[0].x;
        let opp = line[1].y - line[0].y;
        let mut r = [0.0; 3];
        for (n, value) in r.iter_mut().enumerate() {
            *value = (self.pts[n].y - line[0].y) * adj - (self.pts[n].x - line[0].x) * opp;
        }
        let mut a = r[2];
        let mut b = r[1];
        let c = r[0];
        a += c - 2.0 * b; // A = a - 2*b + c
        b -= c; // B = -(b - c)
        DQuad::roots_valid_t(a, 2.0 * b, c, roots)
    }

    // Port of: src/pathops/SkDQuadLineIntersection.cpp (chrome/m156)
    fn horizontal_roots(&self, axis_intercept: f64, roots: &mut [f64; 2]) -> usize {
        self.horizontal_intersect(axis_intercept, roots)
    }

    // Port of: src/pathops/SkDQuadLineIntersection.cpp (chrome/m156)
    fn vertical_roots(&self, axis_intercept: f64, roots: &mut [f64; 2]) -> usize {
        self.vertical_intersect(axis_intercept, roots)
    }
}

impl LineCurve for DConic {
    const MAX_INTERSECTIONS: usize = 4;
    const VERB: Verb = Verb::Conic;

    fn point(&self, n: usize) -> DPoint {
        self.pts[n]
    }

    fn pt_at_t(&self, t: f64) -> DPoint {
        DConic::pt_at_t(self, t)
    }

    fn as_dcurve(&self) -> DCurve {
        DCurve::Conic(*self)
    }

    // Port of: src/pathops/SkDConicLineIntersection.cpp (chrome/m156)
    fn ray_roots(&self, line: &DLine, roots: &mut [f64; 2]) -> usize {
        let adj = line[1].x - line[0].x;
        let opp = line[1].y - line[0].y;
        let mut r = [0.0; 3];
        for (n, value) in r.iter_mut().enumerate() {
            *value = (self.pts[n].y - line[0].y) * adj - (self.pts[n].x - line[0].x) * opp;
        }
        conic_valid_t(self, r, 0.0, roots)
    }

    // Port of: src/pathops/SkDConicLineIntersection.cpp (chrome/m156)
    fn horizontal_roots(&self, axis_intercept: f64, roots: &mut [f64; 2]) -> usize {
        let vals = [self.pts[0].y, self.pts[1].y, self.pts[2].y];
        conic_valid_t(self, vals, axis_intercept, roots)
    }

    // Port of: src/pathops/SkDConicLineIntersection.cpp (chrome/m156)
    fn vertical_roots(&self, axis_intercept: f64, roots: &mut [f64; 2]) -> usize {
        let vals = [self.pts[0].x, self.pts[1].x, self.pts[2].x];
        conic_valid_t(self, vals, axis_intercept, roots)
    }
}

/// `LineQuadraticIntersections` / `LineConicIntersections`.
// Port of: src/pathops/SkDQuadLineIntersection.cpp (chrome/m156)
pub(crate) struct LineCurveIntersections<'a, C: LineCurve> {
    curve: C,
    line: DLine,
    intersections: &'a mut Intersections,
    allow_near: bool,
}

impl<'a, C: LineCurve> LineCurveIntersections<'a, C> {
    // Port of: src/pathops/SkDQuadLineIntersection.cpp (chrome/m156)
    fn new(curve: C, line: DLine, intersections: &'a mut Intersections) -> Self {
        intersections.set_max(C::MAX_INTERSECTIONS);
        Self {
            curve,
            line,
            intersections,
            allow_near: true,
        }
    }

    /// `void checkCoincident()`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp#L125-L146 (chrome/m156)
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss,
        clippy::manual_midpoint,
        clippy::similar_names
    )] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn check_coincident(&mut self) {
        let mut last = self.intersections.used() as i64 - 1;
        let mut index: i64 = 0;
        while index < last {
            let i = index as usize;
            let mid_t = (self.intersections.t(0, i) + self.intersections.t(0, i + 1)) / 2.0;
            let mid_pt = self.curve.pt_at_t(mid_t);
            let t = self.line.near_point(mid_pt, None);
            if t < 0.0 {
                index += 1;
                continue;
            }
            if self.intersections.is_coincident(i) {
                self.intersections.remove_one(i);
                last -= 1;
            } else if self.intersections.is_coincident(i + 1) {
                self.intersections.remove_one(i + 1);
                last -= 1;
            } else {
                self.intersections.set_coincident(i);
                index += 1;
            }
            self.intersections.set_coincident(index as usize);
        }
    }

    /// `int intersect()`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp#L178-L195 (chrome/m156)
    fn intersect(&mut self) -> usize {
        self.add_exact_end_points();
        if self.allow_near {
            self.add_near_end_points();
        }
        let mut root_vals = [0.0; 2];
        let roots = self.curve.ray_roots(&self.line, &mut root_vals);
        for &root in &root_vals[..roots] {
            let mut curve_t = root;
            let mut line_t = self.find_line_t(curve_t);
            let mut pt = DPoint::default();
            if self.pin_ts(&mut curve_t, &mut line_t, &mut pt, PinTPoint::Uninitialized)
                && self.unique_answer(curve_t, pt)
            {
                self.intersections.insert(curve_t, line_t, pt);
            }
        }
        self.check_coincident();
        self.intersections.used()
    }

    /// `int horizontalIntersect(double axisIntercept, double left, double right, bool flipped)`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp (chrome/m156)
    fn horizontal_intersect_segment(
        &mut self,
        axis_intercept: f64,
        left: f64,
        right: f64,
        flipped: bool,
    ) -> usize {
        self.add_exact_horizontal_end_points(left, right, axis_intercept);
        if self.allow_near {
            self.add_near_horizontal_end_points(left, right, axis_intercept);
        }
        let mut root_vals = [0.0; 2];
        let roots = self.curve.horizontal_roots(axis_intercept, &mut root_vals);
        for &root in &root_vals[..roots] {
            let mut curve_t = root;
            let mut pt = self.curve.pt_at_t(curve_t);
            let mut line_t = (pt.x - left) / (right - left);
            if self.pin_ts(&mut curve_t, &mut line_t, &mut pt, PinTPoint::Initialized)
                && self.unique_answer(curve_t, pt)
            {
                self.intersections.insert(curve_t, line_t, pt);
            }
        }
        if flipped {
            self.intersections.flip();
        }
        self.check_coincident();
        self.intersections.used()
    }

    /// `int verticalIntersect(double axisIntercept, double top, double bottom, bool flipped)`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp (chrome/m156)
    fn vertical_intersect_segment(
        &mut self,
        axis_intercept: f64,
        top: f64,
        bottom: f64,
        flipped: bool,
    ) -> usize {
        self.add_exact_vertical_end_points(top, bottom, axis_intercept);
        if self.allow_near {
            self.add_near_vertical_end_points(top, bottom, axis_intercept);
        }
        let mut root_vals = [0.0; 2];
        let roots = self.curve.vertical_roots(axis_intercept, &mut root_vals);
        for &root in &root_vals[..roots] {
            let mut curve_t = root;
            let mut pt = self.curve.pt_at_t(curve_t);
            let mut line_t = (pt.y - top) / (bottom - top);
            if self.pin_ts(&mut curve_t, &mut line_t, &mut pt, PinTPoint::Initialized)
                && self.unique_answer(curve_t, pt)
            {
                self.intersections.insert(curve_t, line_t, pt);
            }
        }
        if flipped {
            self.intersections.flip();
        }
        self.check_coincident();
        self.intersections.used()
    }

    /// `bool uniqueAnswer(double quadT, const SkDPoint& pt)`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp#L229-L251 (chrome/m156)
    #[allow(clippy::float_cmp, clippy::manual_midpoint, clippy::similar_names)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn unique_answer(&self, curve_t: f64, pt: DPoint) -> bool {
        for inner in 0..self.intersections.used() {
            if self.intersections.pt(inner) != pt {
                continue;
            }
            let existing_t = self.intersections.t(0, inner);
            if curve_t == existing_t {
                return false;
            }
            let mid_t = (existing_t + curve_t) / 2.0;
            let mid_pt = self.curve.pt_at_t(mid_t);
            if mid_pt.approximately_equal(pt) {
                return false;
            }
        }
        true
    }

    /// `void addExactEndPoints()`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp#L287-L296 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn add_exact_end_points(&mut self) {
        for c_index in [0usize, 2] {
            let line_t = self.line.exact_point(self.curve.point(c_index));
            if line_t < 0.0 {
                continue;
            }
            let curve_t = (c_index >> 1) as f64;
            self.intersections
                .insert(curve_t, line_t, self.curve.point(c_index));
        }
    }

    /// `void addNearEndPoints()`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp#L298-L311 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn add_near_end_points(&mut self) {
        for c_index in [0usize, 2] {
            let curve_t = (c_index >> 1) as f64;
            if self.intersections.has_t(curve_t) {
                continue;
            }
            let line_t = self.line.near_point(self.curve.point(c_index), None);
            if line_t < 0.0 {
                continue;
            }
            self.intersections
                .insert(curve_t, line_t, self.curve.point(c_index));
        }
        self.add_line_near_end_points();
    }

    /// `void addLineNearEndPoints()`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp#L313-L326 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn add_line_near_end_points(&mut self) {
        for l_index in 0..2usize {
            let line_t = l_index as f64;
            if self.intersections.has_opp_t(line_t) {
                continue;
            }
            let curve_t = curve_near_point(
                &self.curve.as_dcurve(),
                C::VERB,
                self.line[l_index],
                self.line[usize::from(l_index == 0)],
            );
            if curve_t < 0.0 {
                continue;
            }
            self.intersections
                .insert(curve_t, line_t, self.line[l_index]);
        }
    }

    /// `void addExactHorizontalEndPoints(double left, double right, double y)`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp#L328-L337 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn add_exact_horizontal_end_points(&mut self, left: f64, right: f64, y: f64) {
        for c_index in [0usize, 2] {
            let line_t = DLine::exact_point_h(self.curve.point(c_index), left, right, y);
            if line_t < 0.0 {
                continue;
            }
            let curve_t = (c_index >> 1) as f64;
            self.intersections
                .insert(curve_t, line_t, self.curve.point(c_index));
        }
    }

    /// `void addNearHorizontalEndPoints(double left, double right, double y)`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp#L339-L352 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn add_near_horizontal_end_points(&mut self, left: f64, right: f64, y: f64) {
        for c_index in [0usize, 2] {
            let curve_t = (c_index >> 1) as f64;
            if self.intersections.has_t(curve_t) {
                continue;
            }
            let line_t = DLine::near_point_h(self.curve.point(c_index), left, right, y);
            if line_t < 0.0 {
                continue;
            }
            self.intersections
                .insert(curve_t, line_t, self.curve.point(c_index));
        }
        self.add_line_near_end_points();
    }

    /// `void addExactVerticalEndPoints(double top, double bottom, double x)`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp#L354-L363 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn add_exact_vertical_end_points(&mut self, top: f64, bottom: f64, x: f64) {
        for c_index in [0usize, 2] {
            let line_t = DLine::exact_point_v(self.curve.point(c_index), top, bottom, x);
            if line_t < 0.0 {
                continue;
            }
            let curve_t = (c_index >> 1) as f64;
            self.intersections
                .insert(curve_t, line_t, self.curve.point(c_index));
        }
    }

    /// `void addNearVerticalEndPoints(double top, double bottom, double x)`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp#L365-L378 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn add_near_vertical_end_points(&mut self, top: f64, bottom: f64, x: f64) {
        for c_index in [0usize, 2] {
            let curve_t = (c_index >> 1) as f64;
            if self.intersections.has_t(curve_t) {
                continue;
            }
            let line_t = DLine::near_point_v(self.curve.point(c_index), top, bottom, x);
            if line_t < 0.0 {
                continue;
            }
            self.intersections
                .insert(curve_t, line_t, self.curve.point(c_index));
        }
        self.add_line_near_end_points();
    }

    /// `double findLineT(double t)`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp#L380-L388 (chrome/m156)
    fn find_line_t(&self, t: f64) -> f64 {
        let xy = self.curve.pt_at_t(t);
        let dx = self.line[1].x - self.line[0].x;
        let dy = self.line[1].y - self.line[0].y;
        if dx.abs() > dy.abs() {
            return (xy.x - self.line[0].x) / dx;
        }
        (xy.y - self.line[0].y) / dy
    }

    /// `bool pinTs(double* quadT, double* lineT, SkDPoint* pt, PinTPoint ptSet)`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp#L390-L423 (chrome/m156)
    #[allow(clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn pin_ts(
        &self,
        curve_t: &mut f64,
        line_t: &mut f64,
        pt: &mut DPoint,
        pt_set: PinTPoint,
    ) -> bool {
        if !approximately_one_or_less_double(*line_t) {
            return false;
        }
        if !approximately_zero_or_more_double(*line_t) {
            return false;
        }
        let c_t = pin_t(*curve_t);
        *curve_t = c_t;
        let l_t = pin_t(*line_t);
        *line_t = l_t;
        if l_t == 0.0
            || l_t == 1.0
            || (pt_set == PinTPoint::Uninitialized && c_t != 0.0 && c_t != 1.0)
        {
            *pt = self.line.pt_at_t(l_t);
        } else if pt_set == PinTPoint::Uninitialized {
            *pt = self.curve.pt_at_t(c_t);
        }
        let grid_pt = pt.as_sk_point();
        if DPoint::approximately_equal_points(grid_pt, self.line[0].as_sk_point()) {
            *pt = self.line[0];
            *line_t = 0.0;
        } else if DPoint::approximately_equal_points(grid_pt, self.line[1].as_sk_point()) {
            *pt = self.line[1];
            *line_t = 1.0;
        }
        if self.intersections.used() > 0 && approximately_equal(self.intersections.t(1, 0), *line_t)
        {
            return false;
        }
        if grid_pt == self.curve.point(0).as_sk_point() {
            *pt = self.curve.point(0);
            *curve_t = 0.0;
        } else if grid_pt == self.curve.point(2).as_sk_point() {
            *pt = self.curve.point(2);
            *curve_t = 1.0;
        }
        true
    }
}

impl DQuad {
    /// `int horizontalIntersect(double yIntercept, double roots[2]) const`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp#L197-L205 (chrome/m156)
    #[doc(alias = "horizontalIntersect")]
    #[must_use]
    pub fn horizontal_intersect(&self, y_intercept: f64, roots: &mut [f64; 2]) -> usize {
        let mut d = self.pts[2].y; // f
        let mut e = self.pts[1].y; // e
        let mut f = self.pts[0].y; // d
        d += f - 2.0 * e; // D = d - 2*e + f
        e -= f; // E = -(d - e)
        f -= y_intercept;
        Self::roots_valid_t(d, 2.0 * e, f, roots)
    }

    /// `int verticalIntersect(double xIntercept, double roots[2]) const`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp#L253-L261 (chrome/m156)
    #[doc(alias = "verticalIntersect")]
    #[must_use]
    pub fn vertical_intersect(&self, x_intercept: f64, roots: &mut [f64; 2]) -> usize {
        let mut d = self.pts[2].x; // f
        let mut e = self.pts[1].x; // e
        let mut f = self.pts[0].x; // d
        d += f - 2.0 * e; // D = d - 2*e + f
        e -= f; // E = -(d - e)
        f -= x_intercept;
        Self::roots_valid_t(d, 2.0 * e, f, roots)
    }
}

impl Intersections {
    /// `static int HorizontalIntercept(const SkDQuad& quad, SkScalar y, double* roots)`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp (chrome/m156)
    #[doc(alias = "HorizontalIntercept")]
    pub fn horizontal_intercept_quad(quad: &DQuad, y: f32, roots: &mut [f64; 2]) -> usize {
        quad.horizontal_intersect(f64::from(y), roots)
    }

    /// `static int VerticalIntercept(const SkDQuad& quad, SkScalar x, double* roots)`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp (chrome/m156)
    #[doc(alias = "VerticalIntercept")]
    pub fn vertical_intercept_quad(quad: &DQuad, x: f32, roots: &mut [f64; 2]) -> usize {
        quad.vertical_intersect(f64::from(x), roots)
    }

    /// `int horizontal(const SkDQuad& quad, double left, double right, double y, bool flipped)`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp (chrome/m156)
    pub fn horizontal_quad(
        &mut self,
        quad: &DQuad,
        left: f64,
        right: f64,
        y: f64,
        flipped: bool,
    ) -> usize {
        let line = DLine::new([DPoint::new(left, y), DPoint::new(right, y)]);
        LineCurveIntersections::new(*quad, line, self)
            .horizontal_intersect_segment(y, left, right, flipped)
    }

    /// `int vertical(const SkDQuad& quad, double top, double bottom, double x, bool flipped)`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp (chrome/m156)
    pub fn vertical_quad(
        &mut self,
        quad: &DQuad,
        top: f64,
        bottom: f64,
        x: f64,
        flipped: bool,
    ) -> usize {
        let line = DLine::new([DPoint::new(x, top), DPoint::new(x, bottom)]);
        LineCurveIntersections::new(*quad, line, self)
            .vertical_intersect_segment(x, top, bottom, flipped)
    }

    /// `int intersect(const SkDQuad& quad, const SkDLine& line)`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp (chrome/m156)
    pub fn intersect_quad_line(&mut self, quad: &DQuad, line: &DLine) -> usize {
        let allow = self.allow_near;
        let mut c = LineCurveIntersections::new(*quad, *line, self);
        c.allow_near = allow;
        c.intersect()
    }

    /// `int intersectRay(const SkDQuad& quad, const SkDLine& line)`.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp (chrome/m156)
    pub fn intersect_ray_quad(&mut self, quad: &DQuad, line: &DLine) -> usize {
        self.intersect_ray_curve(quad, line)
    }

    /// `int horizontal(const SkDConic& conic, double left, double right, double y, bool flipped)`.
    // Port of: src/pathops/SkDConicLineIntersection.cpp (chrome/m156)
    pub fn horizontal_conic(
        &mut self,
        conic: &DConic,
        left: f64,
        right: f64,
        y: f64,
        flipped: bool,
    ) -> usize {
        let line = DLine::new([DPoint::new(left, y), DPoint::new(right, y)]);
        LineCurveIntersections::new(*conic, line, self)
            .horizontal_intersect_segment(y, left, right, flipped)
    }

    /// `int vertical(const SkDConic& conic, double top, double bottom, double x, bool flipped)`.
    // Port of: src/pathops/SkDConicLineIntersection.cpp (chrome/m156)
    pub fn vertical_conic(
        &mut self,
        conic: &DConic,
        top: f64,
        bottom: f64,
        x: f64,
        flipped: bool,
    ) -> usize {
        let line = DLine::new([DPoint::new(x, top), DPoint::new(x, bottom)]);
        LineCurveIntersections::new(*conic, line, self)
            .vertical_intersect_segment(x, top, bottom, flipped)
    }

    /// `int intersect(const SkDConic& conic, const SkDLine& line)`.
    // Port of: src/pathops/SkDConicLineIntersection.cpp (chrome/m156)
    pub fn intersect_conic_line(&mut self, conic: &DConic, line: &DLine) -> usize {
        let allow = self.allow_near;
        let mut c = LineCurveIntersections::new(*conic, *line, self);
        c.allow_near = allow;
        c.intersect()
    }

    /// `int intersectRay(const SkDConic& conic, const SkDLine& line)`.
    // Port of: src/pathops/SkDConicLineIntersection.cpp (chrome/m156)
    pub fn intersect_ray_conic(&mut self, conic: &DConic, line: &DLine) -> usize {
        self.intersect_ray_curve(conic, line)
    }

    /// `static int HorizontalIntercept(const SkDConic& conic, SkScalar y, double* roots)`.
    // Port of: src/pathops/SkDConicLineIntersection.cpp (chrome/m156)
    pub fn horizontal_intercept_conic(conic: &DConic, y: f32, roots: &mut [f64; 2]) -> usize {
        LineCurve::horizontal_roots(conic, f64::from(y), roots)
    }

    /// `static int VerticalIntercept(const SkDConic& conic, SkScalar x, double* roots)`.
    // Port of: src/pathops/SkDConicLineIntersection.cpp (chrome/m156)
    pub fn vertical_intercept_conic(conic: &DConic, x: f32, roots: &mut [f64; 2]) -> usize {
        LineCurve::vertical_roots(conic, f64::from(x), roots)
    }

    /// The shared body of `intersectRay(quad|conic, line)`: the roots of the curve, then their
    /// points.
    // Port of: src/pathops/SkDQuadLineIntersection.cpp (chrome/m156)
    #[allow(clippy::cast_possible_truncation)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn intersect_ray_curve<C: LineCurve>(&mut self, curve: &C, line: &DLine) -> usize {
        // LineQuadraticIntersections / LineConicIntersections set the max in their constructors.
        self.set_max(C::MAX_INTERSECTIONS);
        let mut roots = [0.0; 2];
        let used = curve.ray_roots(line, &mut roots);
        self.t[0][..used].copy_from_slice(&roots[..used]);
        self.used = used as u8;
        for index in 0..used {
            self.pt[index] = curve.pt_at_t(self.t[0][index]);
        }
        usize::from(self.used)
    }
}
