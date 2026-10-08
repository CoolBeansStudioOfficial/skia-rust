// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkDCubicLineIntersection.cpp

//! Cubic/line intersections (`LineCubicIntersections` and the `SkIntersections` entry points).

use skia_rust_core::path::Verb;

use crate::cubic::{DCubic, SearchAxis};
use crate::curve::{DCurve, curve_near_point};
use crate::intersections::Intersections;
use crate::line::DLine;
use crate::point::DPoint;
use crate::types::{
    approximately_equal, approximately_one_or_less, approximately_zero, approximately_zero_or_more,
    pin_t,
};

/// `PinTPoint` of `LineCubicIntersections`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum PinTPoint {
    Uninitialized,
    Initialized,
}

/// `LineCubicIntersections`: intersects a cubic with a line.
// Port of: src/pathops/SkDCubicLineIntersection.cpp#L94-L94 (chrome/m156)
struct LineCubicIntersections<'a> {
    cubic: DCubic,
    line: DLine,
    intersections: &'a mut Intersections,
    allow_near: bool,
}

impl<'a> LineCubicIntersections<'a> {
    // Port of: src/pathops/SkDCubicLineIntersection.cpp (chrome/m156)
    fn new(cubic: DCubic, line: DLine, intersections: &'a mut Intersections) -> Self {
        intersections.set_max(4);
        Self {
            cubic,
            line,
            intersections,
            allow_near: true,
        }
    }

    /// `void checkCoincident()`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp#L106-L127 (chrome/m156)
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
            let mid_pt = self.cubic.pt_at_t(mid_t);
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

    /// `int intersectRay(double roots[3])`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp (chrome/m156)
    fn intersect_ray_roots(&self, roots: &mut [f64; 3]) -> usize {
        let adj = self.line[1].x - self.line[0].x;
        let opp = self.line[1].y - self.line[0].y;
        let mut c = DCubic::default();
        for n in 0..4 {
            c.pts[n].x = (self.cubic.pts[n].y - self.line[0].y) * adj
                - (self.cubic.pts[n].x - self.line[0].x) * opp;
        }
        let xs = [c.pts[0].x, c.pts[1].x, c.pts[2].x, c.pts[3].x];
        let (a, b, cc, d) = DCubic::coefficients(xs);
        let mut count = DCubic::roots_valid_t(a, b, cc, d, roots);
        for index in 0..count {
            let calc_pt = c.pt_at_t(roots[index]);
            if !approximately_zero(calc_pt.x) {
                for n in 0..4 {
                    c.pts[n].y = (self.cubic.pts[n].y - self.line[0].y) * opp
                        + (self.cubic.pts[n].x - self.line[0].x) * adj;
                }
                let mut extreme_ts = [0.0; 6];
                let xs = [c.pts[0].x, c.pts[1].x, c.pts[2].x, c.pts[3].x];
                let mut ext = [0.0; 2];
                let extrema = DCubic::find_extrema(xs, &mut ext);
                extreme_ts[..extrema].copy_from_slice(&ext[..extrema]);
                count = c.search_roots(&mut extreme_ts, extrema, 0.0, SearchAxis::XAxis, roots);
                break;
            }
        }
        count
    }

    /// `int intersect()`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp#L157-L174 (chrome/m156)
    fn intersect(&mut self) -> usize {
        self.add_exact_end_points();
        if self.allow_near {
            self.add_near_end_points();
        }
        let mut root_vals = [0.0; 3];
        let roots = self.intersect_ray_roots(&mut root_vals);
        for &root in &root_vals[..roots] {
            let mut cubic_t = root;
            let mut line_t = self.find_line_t(cubic_t);
            let mut pt = DPoint::default();
            if self.pin_ts(&mut cubic_t, &mut line_t, &mut pt, PinTPoint::Uninitialized)
                && self.unique_answer(cubic_t, pt)
            {
                self.intersections.insert(cubic_t, line_t, pt);
            }
        }
        self.check_coincident();
        self.intersections.used()
    }

    /// `int horizontalIntersect(double axisIntercept, double left, double right, bool flipped)`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp (chrome/m156)
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
        let mut roots = [0.0; 3];
        let count = DCubic::horizontal_intersect_roots(&self.cubic, axis_intercept, &mut roots);
        for &root in &roots[..count] {
            let mut cubic_t = root;
            let mut pt = DPoint::new(self.cubic.pt_at_t(cubic_t).x, axis_intercept);
            let mut line_t = (pt.x - left) / (right - left);
            if self.pin_ts(&mut cubic_t, &mut line_t, &mut pt, PinTPoint::Initialized)
                && self.unique_answer(cubic_t, pt)
            {
                self.intersections.insert(cubic_t, line_t, pt);
            }
        }
        if flipped {
            self.intersections.flip();
        }
        self.check_coincident();
        self.intersections.used()
    }

    /// `int verticalIntersect(double axisIntercept, double top, double bottom, bool flipped)`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp (chrome/m156)
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
        let mut roots = [0.0; 3];
        let count = DCubic::vertical_intersect_roots(&self.cubic, axis_intercept, &mut roots);
        for &root in &roots[..count] {
            let mut cubic_t = root;
            let mut pt = DPoint::new(axis_intercept, self.cubic.pt_at_t(cubic_t).y);
            let mut line_t = (pt.y - top) / (bottom - top);
            if self.pin_ts(&mut cubic_t, &mut line_t, &mut pt, PinTPoint::Initialized)
                && self.unique_answer(cubic_t, pt)
            {
                self.intersections.insert(cubic_t, line_t, pt);
            }
        }
        if flipped {
            self.intersections.flip();
        }
        self.check_coincident();
        self.intersections.used()
    }

    /// `bool uniqueAnswer(double cubicT, const SkDPoint& pt)`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp (chrome/m156)
    #[allow(clippy::float_cmp, clippy::manual_midpoint, clippy::similar_names)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn unique_answer(&self, cubic_t: f64, pt: DPoint) -> bool {
        for inner in 0..self.intersections.used() {
            if self.intersections.pt(inner) != pt {
                continue;
            }
            let existing_cubic_t = self.intersections.t(0, inner);
            if cubic_t == existing_cubic_t {
                return false;
            }
            let cubic_mid_t = (existing_cubic_t + cubic_t) / 2.0;
            let cubic_mid_pt = self.cubic.pt_at_t(cubic_mid_t);
            if cubic_mid_pt.approximately_equal(pt) {
                return false;
            }
        }
        true
    }

    /// `void addExactEndPoints()`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp#L280-L289 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn add_exact_end_points(&mut self) {
        for c_index in [0usize, 3] {
            let line_t = self.line.exact_point(self.cubic[c_index]);
            if line_t < 0.0 {
                continue;
            }
            let cubic_t = (c_index >> 1) as f64;
            self.intersections
                .insert(cubic_t, line_t, self.cubic[c_index]);
        }
    }

    /// `void addNearEndPoints()`.
    /// Note that this does not look for endpoints of the line that are near the cubic. Those are
    /// found later when check ends looks for missing points.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp#L293-L306 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn add_near_end_points(&mut self) {
        for c_index in [0usize, 3] {
            let cubic_t = (c_index >> 1) as f64;
            if self.intersections.has_t(cubic_t) {
                continue;
            }
            let line_t = self.line.near_point(self.cubic[c_index], None);
            if line_t < 0.0 {
                continue;
            }
            self.intersections
                .insert(cubic_t, line_t, self.cubic[c_index]);
        }
        self.add_line_near_end_points();
    }

    /// `void addLineNearEndPoints()`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp#L308-L321 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn add_line_near_end_points(&mut self) {
        for l_index in 0..2usize {
            let line_t = l_index as f64;
            if self.intersections.has_opp_t(line_t) {
                continue;
            }
            let cubic_t = curve_near_point(
                &DCurve::Cubic(self.cubic),
                Verb::Cubic,
                self.line[l_index],
                self.line[usize::from(l_index == 0)],
            );
            if cubic_t < 0.0 {
                continue;
            }
            self.intersections
                .insert(cubic_t, line_t, self.line[l_index]);
        }
    }

    /// `void addExactHorizontalEndPoints(double left, double right, double y)`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp#L323-L332 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn add_exact_horizontal_end_points(&mut self, left: f64, right: f64, y: f64) {
        for c_index in [0usize, 3] {
            let line_t = DLine::exact_point_h(self.cubic[c_index], left, right, y);
            if line_t < 0.0 {
                continue;
            }
            let cubic_t = (c_index >> 1) as f64;
            self.intersections
                .insert(cubic_t, line_t, self.cubic[c_index]);
        }
    }

    /// `void addNearHorizontalEndPoints(double left, double right, double y)`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp#L334-L347 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn add_near_horizontal_end_points(&mut self, left: f64, right: f64, y: f64) {
        for c_index in [0usize, 3] {
            let cubic_t = (c_index >> 1) as f64;
            if self.intersections.has_t(cubic_t) {
                continue;
            }
            let line_t = DLine::near_point_h(self.cubic[c_index], left, right, y);
            if line_t < 0.0 {
                continue;
            }
            self.intersections
                .insert(cubic_t, line_t, self.cubic[c_index]);
        }
        self.add_line_near_end_points();
    }

    /// `void addExactVerticalEndPoints(double top, double bottom, double x)`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp#L349-L358 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn add_exact_vertical_end_points(&mut self, top: f64, bottom: f64, x: f64) {
        for c_index in [0usize, 3] {
            let line_t = DLine::exact_point_v(self.cubic[c_index], top, bottom, x);
            if line_t < 0.0 {
                continue;
            }
            let cubic_t = (c_index >> 1) as f64;
            self.intersections
                .insert(cubic_t, line_t, self.cubic[c_index]);
        }
    }

    /// `void addNearVerticalEndPoints(double top, double bottom, double x)`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp#L360-L373 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn add_near_vertical_end_points(&mut self, top: f64, bottom: f64, x: f64) {
        for c_index in [0usize, 3] {
            let cubic_t = (c_index >> 1) as f64;
            if self.intersections.has_t(cubic_t) {
                continue;
            }
            let line_t = DLine::near_point_v(self.cubic[c_index], top, bottom, x);
            if line_t < 0.0 {
                continue;
            }
            self.intersections
                .insert(cubic_t, line_t, self.cubic[c_index]);
        }
        self.add_line_near_end_points();
    }

    /// `double findLineT(double t)`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp#L375-L383 (chrome/m156)
    fn find_line_t(&self, t: f64) -> f64 {
        let xy = self.cubic.pt_at_t(t);
        let dx = self.line[1].x - self.line[0].x;
        let dy = self.line[1].y - self.line[0].y;
        if dx.abs() > dy.abs() {
            return (xy.x - self.line[0].x) / dx;
        }
        (xy.y - self.line[0].y) / dy
    }

    /// `bool pinTs(double* cubicT, double* lineT, SkDPoint* pt, PinTPoint ptSet)`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp#L385-L418 (chrome/m156)
    #[allow(clippy::float_cmp, clippy::similar_names)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    fn pin_ts(
        &self,
        cubic_t: &mut f64,
        line_t: &mut f64,
        pt: &mut DPoint,
        pt_set: PinTPoint,
    ) -> bool {
        if !approximately_one_or_less(*line_t) {
            return false;
        }
        if !approximately_zero_or_more(*line_t) {
            return false;
        }
        let c_t = pin_t(*cubic_t);
        *cubic_t = c_t;
        let l_t = pin_t(*line_t);
        *line_t = l_t;
        let l_pt = self.line.pt_at_t(l_t);
        let c_pt = self.cubic.pt_at_t(c_t);
        if !l_pt.roughly_equal(c_pt) {
            return false;
        }
        if l_t == 0.0
            || l_t == 1.0
            || (pt_set == PinTPoint::Uninitialized && c_t != 0.0 && c_t != 1.0)
        {
            *pt = l_pt;
        } else if pt_set == PinTPoint::Uninitialized {
            *pt = c_pt;
        }
        let grid_pt = pt.as_sk_point();
        if grid_pt == self.line[0].as_sk_point() {
            *line_t = 0.0;
        } else if grid_pt == self.line[1].as_sk_point() {
            *line_t = 1.0;
        }
        if grid_pt == self.cubic[0].as_sk_point() && approximately_equal(*cubic_t, 0.0) {
            *cubic_t = 0.0;
        } else if grid_pt == self.cubic[3].as_sk_point() && approximately_equal(*cubic_t, 1.0) {
            *cubic_t = 1.0;
        }
        true
    }
}

impl DCubic {
    /// `static int HorizontalIntersect(const SkDCubic& c, double axisIntercept, double roots[3])`,
    /// the root finder behind `horizontalIntersect`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp (chrome/m156)
    fn horizontal_intersect_roots(c: &Self, axis_intercept: f64, roots: &mut [f64; 3]) -> usize {
        let ys = [c.pts[0].y, c.pts[1].y, c.pts[2].y, c.pts[3].y];
        let (a, b, cc, mut d) = Self::coefficients(ys);
        d -= axis_intercept;
        let mut count = Self::roots_valid_t(a, b, cc, d, roots);
        for index in 0..count {
            let calc_pt = c.pt_at_t(roots[index]);
            if !approximately_equal(calc_pt.y, axis_intercept) {
                let mut extreme_ts = [0.0; 6];
                let mut ext = [0.0; 2];
                let extrema = Self::find_extrema(ys, &mut ext);
                extreme_ts[..extrema].copy_from_slice(&ext[..extrema]);
                count = c.search_roots(
                    &mut extreme_ts,
                    extrema,
                    axis_intercept,
                    SearchAxis::YAxis,
                    roots,
                );
                break;
            }
        }
        count
    }

    /// `static int VerticalIntersect(const SkDCubic& c, double axisIntercept, double roots[3])`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp (chrome/m156)
    fn vertical_intersect_roots(c: &Self, axis_intercept: f64, roots: &mut [f64; 3]) -> usize {
        let xs = [c.pts[0].x, c.pts[1].x, c.pts[2].x, c.pts[3].x];
        let (a, b, cc, mut d) = Self::coefficients(xs);
        d -= axis_intercept;
        let mut count = Self::roots_valid_t(a, b, cc, d, roots);
        for index in 0..count {
            let calc_pt = c.pt_at_t(roots[index]);
            if !approximately_equal(calc_pt.x, axis_intercept) {
                let mut extreme_ts = [0.0; 6];
                let mut ext = [0.0; 2];
                let extrema = Self::find_extrema(xs, &mut ext);
                extreme_ts[..extrema].copy_from_slice(&ext[..extrema]);
                count = c.search_roots(
                    &mut extreme_ts,
                    extrema,
                    axis_intercept,
                    SearchAxis::XAxis,
                    roots,
                );
                break;
            }
        }
        count
    }

    /// `int horizontalIntersect(double yIntercept, double roots[3]) const`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp#L193-L213 (chrome/m156)
    #[doc(alias = "horizontalIntersect")]
    #[must_use]
    pub fn horizontal_intersect(&self, y_intercept: f64, roots: &mut [f64; 3]) -> usize {
        Self::horizontal_intersect_roots(self, y_intercept, roots)
    }

    /// `int verticalIntersect(double xIntercept, double roots[3]) const`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp#L256-L276 (chrome/m156)
    #[doc(alias = "verticalIntersect")]
    #[must_use]
    pub fn vertical_intersect(&self, x_intercept: f64, roots: &mut [f64; 3]) -> usize {
        Self::vertical_intersect_roots(self, x_intercept, roots)
    }
}

impl Intersections {
    /// `int horizontal(const SkDCubic& cubic, double left, double right, double y, bool flipped)`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp (chrome/m156)
    pub fn horizontal_cubic(
        &mut self,
        cubic: &DCubic,
        left: f64,
        right: f64,
        y: f64,
        flipped: bool,
    ) -> usize {
        let line = DLine::new([DPoint::new(left, y), DPoint::new(right, y)]);
        LineCubicIntersections::new(*cubic, line, self)
            .horizontal_intersect_segment(y, left, right, flipped)
    }

    /// `int vertical(const SkDCubic& cubic, double top, double bottom, double x, bool flipped)`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp (chrome/m156)
    pub fn vertical_cubic(
        &mut self,
        cubic: &DCubic,
        top: f64,
        bottom: f64,
        x: f64,
        flipped: bool,
    ) -> usize {
        let line = DLine::new([DPoint::new(x, top), DPoint::new(x, bottom)]);
        LineCubicIntersections::new(*cubic, line, self)
            .vertical_intersect_segment(x, top, bottom, flipped)
    }

    /// `int intersect(const SkDCubic& cubic, const SkDLine& line)`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp (chrome/m156)
    pub fn intersect_cubic_line(&mut self, cubic: &DCubic, line: &DLine) -> usize {
        let allow = self.allow_near;
        let mut c = LineCubicIntersections::new(*cubic, *line, self);
        c.allow_near = allow;
        c.intersect()
    }

    /// `int intersectRay(const SkDCubic& cubic, const SkDLine& line)`.
    // Port of: src/pathops/SkDCubicLineIntersection.cpp (chrome/m156)
    #[allow(clippy::cast_possible_truncation)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn intersect_ray_cubic(&mut self, cubic: &DCubic, line: &DLine) -> usize {
        let mut roots = [0.0; 3];
        let used = {
            let c = LineCubicIntersections::new(*cubic, *line, self);
            c.intersect_ray_roots(&mut roots)
        };
        self.t[0][..used].copy_from_slice(&roots[..used]);
        self.used = used as u8;
        for index in 0..used {
            self.pt[index] = cubic.pt_at_t(self.t[0][index]);
        }
        usize::from(self.used)
    }
}
