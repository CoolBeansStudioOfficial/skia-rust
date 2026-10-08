// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsQuad.h, src/pathops/SkPathOpsQuad.cpp

//! Double-precision quadratic Bezier curves (`SkDQuad`, `SkDQuadPair`).

use std::ops::{Index, IndexMut};

use skia_rust_core::point::Point;

use crate::conic::DConic;
use crate::cubic::DCubic;
use crate::intersections::Intersections;
use crate::line::DLine;
use crate::line_parameters::LineParameters;
use crate::point::{DPoint, DVector};
use crate::types::{
    almost_bequal_ulps, almost_dequal_ulps, approximately_equal, approximately_greater_than_one,
    approximately_less_than_zero, approximately_one_or_less, approximately_zero,
    approximately_zero_inverse, approximately_zero_or_more, approximately_zero_when_compared_to,
    between, d_interp, precisely_zero, std_max, std_min, zero_or_one,
};

/// `SkDQuadPair`: a quad chopped at a t value into two quads sharing the middle point.
// Port of: src/pathops/SkPathOpsQuad.h (chrome/m156)
#[doc(alias = "SkDQuadPair")]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct DQuadPair {
    pub pts: [DPoint; 5],
}

impl DQuadPair {
    /// The first quad of the pair.
    #[must_use]
    pub fn first(&self) -> DQuad {
        DQuad::new([self.pts[0], self.pts[1], self.pts[2]])
    }

    /// The second quad of the pair.
    #[must_use]
    pub fn second(&self) -> DQuad {
        DQuad::new([self.pts[2], self.pts[3], self.pts[4]])
    }
}

/// `SkDQuad`: a double-precision quadratic Bezier curve.
// Port of: src/pathops/SkPathOpsQuad.h (chrome/m156)
#[doc(alias = "SkDQuad")]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct DQuad {
    pub pts: [DPoint; 3],
}

/// `SkDQuad::kPointCount`.
pub const QUAD_POINT_COUNT: usize = 3;
/// `SkDQuad::kPointLast`.
pub const QUAD_POINT_LAST: usize = QUAD_POINT_COUNT - 1;
/// `SkDQuad::kMaxIntersections`.
pub const QUAD_MAX_INTERSECTIONS: usize = 4;

/// Port of `pointInTriangle`, the test used by `hullIntersects`.
// Port of: src/pathops/SkPathOpsQuad.cpp#L21-L39 (chrome/m156)
fn point_in_triangle(pts: &[DPoint; 3], test: DPoint) -> bool {
    let v0 = pts[2] - pts[0];
    let v1 = pts[1] - pts[0];
    let v2 = test - pts[0];
    let dot00 = v0.dot(v0);
    let dot01 = v0.dot(v1);
    let dot02 = v0.dot(v2);
    let dot11 = v1.dot(v1);
    let dot12 = v1.dot(v2);
    let denom = dot00 * dot11 - dot01 * dot01;
    let u = dot11 * dot02 - dot01 * dot12;
    let v = dot00 * dot12 - dot01 * dot02;
    if denom >= 0.0 {
        return u >= 0.0 && v >= 0.0 && u + v < denom;
    }
    u <= 0.0 && v <= 0.0 && u + v > denom
}

/// Port of `matchesEnd`.
// Port of: src/pathops/SkPathOpsQuad.cpp#L41-L43 (chrome/m156)
fn matches_end(pts: &[DPoint; 3], test: DPoint) -> bool {
    pts[0] == test || pts[2] == test
}

/// Port of `valid_unit_divide`.
// Port of: src/pathops/SkPathOpsQuad.cpp#L362-L362 (chrome/m156)
fn valid_unit_divide(mut numer: f64, mut denom: f64, ratio: &mut f64) -> bool {
    if numer < 0.0 {
        numer = -numer;
        denom = -denom;
    }
    if denom == 0.0 || numer == 0.0 || numer >= denom {
        return false;
    }
    let r = numer / denom;
    if r == 0.0 {
        // catch underflow if numer <<<< denom
        return false;
    }
    *ratio = r;
    true
}

/// Port of `handle_zero`.
// Port of: src/pathops/SkPathOpsQuad.cpp#L151-L158 (chrome/m156)
fn handle_zero(b: f64, c: f64, s: &mut [f64; 2]) -> usize {
    if approximately_zero(b) {
        s[0] = 0.0;
        return usize::from(c == 0.0);
    }
    s[0] = -c / b;
    1
}

/// Port of `interp_quad_coords(const double* src, double t)`: `src` is the x (or y) coordinate of
/// the three control points.
// Port of: src/pathops/SkPathOpsQuad.cpp#L240-L251 (chrome/m156)
#[allow(clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
fn interp_quad_coords(src: [f64; 3], t: f64) -> f64 {
    if t == 0.0 {
        return src[0];
    }
    if t == 1.0 {
        return src[2];
    }
    let ab = d_interp(src[0], src[1], t);
    let bc = d_interp(src[1], src[2], t);
    d_interp(ab, bc, t)
}

impl DQuad {
    #[must_use]
    pub const fn new(pts: [DPoint; 3]) -> Self {
        Self { pts }
    }

    /// `const SkDQuad& set(const SkPoint pts[kPointCount])`.
    pub fn set(&mut self, pts: [Point; 3]) -> &mut Self {
        self.pts[0] = DPoint::from_sk_point(pts[0]);
        self.pts[1] = DPoint::from_sk_point(pts[1]);
        self.pts[2] = DPoint::from_sk_point(pts[2]);
        self
    }

    /// `SkDQuad::SubDivide(const SkPoint a[kPointCount], double t1, double t2)`.
    #[must_use]
    pub fn sub_divide_sk(a: [Point; 3], t1: f64, t2: f64) -> Self {
        let mut quad = Self::default();
        quad.set(a);
        quad.sub_divide(t1, t2)
    }

    /// `bool collapsed() const`.
    #[must_use]
    pub fn collapsed(&self) -> bool {
        self.pts[0].approximately_equal(self.pts[1]) && self.pts[0].approximately_equal(self.pts[2])
    }

    /// `bool controlsInside() const`.
    #[must_use]
    pub fn controls_inside(&self) -> bool {
        let v01 = self.pts[0] - self.pts[1];
        let v02 = self.pts[0] - self.pts[2];
        let v12 = self.pts[1] - self.pts[2];
        v02.dot(v01) > 0.0 && v02.dot(v12) > 0.0
    }

    /// `SkDQuad flip() const`.
    #[must_use]
    pub fn flip(&self) -> Self {
        Self::new([self.pts[2], self.pts[1], self.pts[0]])
    }

    /// `static bool IsConic()`.
    #[must_use]
    pub const fn is_conic() -> bool {
        false
    }

    /// `static int AddValidTs(double s[], int realRoots, double* t)`: keeps the roots in `[0, 1]`
    /// (clamped to the ends), dropping duplicates. Returns the number of roots written to `t`.
    // Port of: src/pathops/SkPathOpsQuad.cpp#L116-L137 (chrome/m156)
    #[doc(alias = "AddValidTs")]
    pub fn add_valid_ts(s: &[f64], t: &mut [f64]) -> usize {
        let mut found_roots = 0;
        for &value in s {
            let mut t_value = value;
            if approximately_zero_or_more(t_value) && approximately_one_or_less(t_value) {
                if approximately_less_than_zero(t_value) {
                    t_value = 0.0;
                } else if approximately_greater_than_one(t_value) {
                    t_value = 1.0;
                }
                if !t[..found_roots]
                    .iter()
                    .any(|&prev| approximately_equal(prev, t_value))
                {
                    t[found_roots] = t_value;
                    found_roots += 1;
                }
            }
        }
        found_roots
    }

    /// `static int RootsValidT(double A, double B, double C, double t[2])`.
    // Port of: src/pathops/SkPathOpsQuad.cpp#L144-L149 (chrome/m156)
    #[doc(alias = "RootsValidT")]
    #[must_use]
    #[allow(clippy::many_single_char_names)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn roots_valid_t(a: f64, b: f64, c: f64, t: &mut [f64; 2]) -> usize {
        let mut s = [0.0; 2];
        let real_roots = Self::roots_real(a, b, c, &mut s);
        Self::add_valid_ts(&s[..real_roots], t)
    }

    /// `static int RootsReal(double A, double B, double C, double s[2])`.
    // Port of: src/pathops/SkPathOpsQuad.cpp#L146-L146 (chrome/m156)
    #[doc(alias = "RootsReal")]
    #[must_use]
    #[allow(clippy::many_single_char_names)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn roots_real(a: f64, b: f64, c: f64, s: &mut [f64; 2]) -> usize {
        if a == 0.0 {
            return handle_zero(b, c, s);
        }
        let p = b / (2.0 * a);
        let q = c / a;
        if approximately_zero(a) && (approximately_zero_inverse(p) || approximately_zero_inverse(q))
        {
            return handle_zero(b, c, s);
        }
        // normal form: x^2 + px + q = 0
        let p2 = p * p;
        if !almost_dequal_ulps(p2, q) && p2 < q {
            return 0;
        }
        let mut sqrt_d = 0.0;
        if p2 > q {
            sqrt_d = (p2 - q).sqrt();
        }
        s[0] = sqrt_d - p;
        s[1] = -sqrt_d - p;
        1 + usize::from(!almost_dequal_ulps(s[0], s[1]))
    }

    /// `bool isLinear(int startIndex, int endIndex) const`.
    // Port of: src/pathops/SkPathOpsQuad.cpp#L192-L204 (chrome/m156)
    #[must_use]
    pub fn is_linear(&self, start_index: usize, end_index: usize) -> bool {
        let mut line_parameters = LineParameters::default();
        line_parameters.quad_end_points(self, start_index, end_index);
        line_parameters.normalize();
        let distance = line_parameters.control_pt_distance_quad(self);
        let p = &self.pts;
        let tiniest = std_min(
            std_min(
                std_min(std_min(std_min(p[0].x, p[0].y), p[1].x), p[1].y),
                p[2].x,
            ),
            p[2].y,
        );
        let mut largest = std_max(
            std_max(
                std_max(std_max(std_max(p[0].x, p[0].y), p[1].x), p[1].y),
                p[2].x,
            ),
            p[2].y,
        );
        largest = std_max(largest, -tiniest);
        approximately_zero_when_compared_to(distance, largest)
    }

    /// `SkDVector dxdyAtT(double t) const`.
    // Port of: src/pathops/SkPathOpsQuad.cpp#L206-L221 (chrome/m156)
    #[must_use]
    pub fn dxdy_at_t(&self, t: f64) -> DVector {
        let a = t - 1.0;
        let b = 1.0 - 2.0 * t;
        let c = t;
        let mut result = DVector::new(
            a * self.pts[0].x + b * self.pts[1].x + c * self.pts[2].x,
            a * self.pts[0].y + b * self.pts[1].y + c * self.pts[2].y,
        );
        if result.x == 0.0 && result.y == 0.0 && zero_or_one(t) {
            result = self.pts[2] - self.pts[0];
        }
        result
    }

    /// `SkDPoint ptAtT(double t) const`.
    // Port of: src/pathops/SkPathOpsQuad.cpp#L224-L238 (chrome/m156)
    #[doc(alias = "ptAtT")]
    #[must_use]
    #[allow(clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn pt_at_t(&self, t: f64) -> DPoint {
        if t == 0.0 {
            return self.pts[0];
        }
        if t == 1.0 {
            return self.pts[2];
        }
        let one_t = 1.0 - t;
        let a = one_t * one_t;
        let b = 2.0 * one_t * t;
        let c = t * t;
        DPoint::new(
            a * self.pts[0].x + b * self.pts[1].x + c * self.pts[2].x,
            a * self.pts[0].y + b * self.pts[1].y + c * self.pts[2].y,
        )
    }

    /// `bool monotonicInX() const`.
    #[must_use]
    pub fn monotonic_in_x(&self) -> bool {
        between(self.pts[0].x, self.pts[1].x, self.pts[2].x)
    }

    /// `bool monotonicInY() const`.
    #[must_use]
    pub fn monotonic_in_y(&self) -> bool {
        between(self.pts[0].y, self.pts[1].y, self.pts[2].y)
    }

    /// `void otherPts(int oddMan, const SkDPoint* endPt[2]) const`: the two points other than
    /// `odd_man`, in order.
    // Port of: src/pathops/SkPathOpsQuad.cpp#L108-L114 (chrome/m156)
    #[must_use]
    #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn other_pts(&self, odd_man: usize) -> [DPoint; 2] {
        let mut end_pt = [DPoint::default(); 2];
        for opp in 1..QUAD_POINT_COUNT {
            // choose a value not equal to oddMan
            let mut end = (odd_man ^ opp).wrapping_sub(odd_man) as isize;
            // if the value went negative, set it to zero
            end &= !(end >> 2);
            end_pt[opp - 1] = self.pts[end as usize];
        }
        end_pt
    }

    /// `static int FindExtrema(const double src[], double tValue[1])`: `src` is one coordinate of
    /// the three control points.
    // Port of: src/pathops/SkPathOpsQuad.cpp#L384-L392 (chrome/m156)
    #[doc(alias = "FindExtrema")]
    #[must_use]
    pub fn find_extrema(src: [f64; 3], t_value: &mut [f64; 1]) -> usize {
        // At + B == 0; t = -B / A
        let a = src[0];
        let b = src[1];
        let c = src[2];
        let mut t = 0.0;
        if valid_unit_divide(a - b, a - b - b + c, &mut t) {
            t_value[0] = t;
            1
        } else {
            0
        }
    }

    /// `static void SetABC(const double* quad, double* a, double* b, double* c)`: `quad` is one
    /// coordinate of the three control points.
    // Port of: src/pathops/SkPathOpsQuad.cpp (chrome/m156)
    #[doc(alias = "SetABC")]
    #[must_use]
    pub fn set_abc(quad: [f64; 3]) -> (f64, f64, f64) {
        let mut a = quad[0]; // a = A
        let mut b = 2.0 * quad[1]; // b =     2*B
        let c = quad[2]; // c =             C
        b -= c; // b =     2*B -   C
        a -= b; // a = A - 2*B +   C
        b -= c; // b =     2*B - 2*C
        (a, b, c)
    }

    /// `SkDQuad subDivide(double t1, double t2) const`.
    // Port of: src/pathops/SkPathOpsQuad.cpp#L285-L299 (chrome/m156)
    #[must_use]
    #[allow(clippy::float_cmp, clippy::manual_midpoint)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn sub_divide(&self, t1: f64, t2: f64) -> Self {
        if t1 == 0.0 && t2 == 1.0 {
            return *self;
        }
        let xs = [self.pts[0].x, self.pts[1].x, self.pts[2].x];
        let ys = [self.pts[0].y, self.pts[1].y, self.pts[2].y];
        let mut dst = Self::default();
        let ax = interp_quad_coords(xs, t1);
        dst.pts[0].x = ax;
        let ay = interp_quad_coords(ys, t1);
        dst.pts[0].y = ay;
        let dx = interp_quad_coords(xs, (t1 + t2) / 2.0);
        let dy = interp_quad_coords(ys, (t1 + t2) / 2.0);
        let cx = interp_quad_coords(xs, t2);
        dst.pts[2].x = cx;
        let cy = interp_quad_coords(ys, t2);
        dst.pts[2].y = cy;
        dst.pts[1].x = 2.0 * dx - (ax + cx) / 2.0;
        dst.pts[1].y = 2.0 * dy - (ay + cy) / 2.0;
        dst
    }

    /// `void subDivide(double t1, double t2, SkDQuad* quad) const`.
    pub fn sub_divide_into(&self, t1: f64, t2: f64, quad: &mut Self) {
        *quad = self.sub_divide(t1, t2);
    }

    /// `SkDPoint subDivide(const SkDPoint& a, const SkDPoint& c, double t1, double t2) const`.
    // Port of: src/pathops/SkPathOpsQuad.cpp (chrome/m156)
    #[must_use]
    #[allow(clippy::float_cmp, clippy::manual_assert_eq)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn sub_divide_points(&self, a: DPoint, c: DPoint, t1: f64, t2: f64) -> DPoint {
        debug_assert!(t1 != t2);
        let sub = self.sub_divide(t1, t2);
        let b0 = DLine::new([a, sub.pts[1] + (a - sub.pts[0])]);
        let b1 = DLine::new([c, sub.pts[1] + (c - sub.pts[2])]);
        let mut i = Intersections::default();
        i.intersect_ray_line(&b0, &b1);
        let mut b = if i.used() == 1 && i.t(0, 0) >= 0.0 && i.t(1, 0) >= 0.0 {
            i.pt(0)
        } else {
            debug_assert!(i.used() <= 2);
            return DPoint::mid(b0.pts[1], b1.pts[1]);
        };
        if t1 == 0.0 || t2 == 0.0 {
            self.align(0, &mut b);
        }
        if t1 == 1.0 || t2 == 1.0 {
            self.align(2, &mut b);
        }
        if almost_bequal_ulps(b.x, a.x) {
            b.x = a.x;
        } else if almost_bequal_ulps(b.x, c.x) {
            b.x = c.x;
        }
        if almost_bequal_ulps(b.y, a.y) {
            b.y = a.y;
        } else if almost_bequal_ulps(b.y, c.y) {
            b.y = c.y;
        }
        b
    }

    /// `void align(int endIndex, SkDPoint* dstPt) const`.
    // Port of: src/pathops/SkPathOpsQuad.cpp#L301-L308 (chrome/m156)
    #[allow(clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn align(&self, end_index: usize, dst_pt: &mut DPoint) {
        if self.pts[end_index].x == self.pts[1].x {
            dst_pt.x = self.pts[end_index].x;
        }
        if self.pts[end_index].y == self.pts[1].y {
            dst_pt.y = self.pts[end_index].y;
        }
    }

    /// `SkDQuadPair chopAt(double t) const`.
    // Port of: src/pathops/SkPathOpsQuad.cpp#L354-L354 (chrome/m156)
    #[must_use]
    pub fn chop_at(&self, t: f64) -> DQuadPair {
        let mut dst = DQuadPair::default();
        let xs = [self.pts[0].x, self.pts[1].x, self.pts[2].x];
        let ys = [self.pts[0].y, self.pts[1].y, self.pts[2].y];
        for (axis, src) in [xs, ys].into_iter().enumerate() {
            let ab = d_interp(src[0], src[1], t);
            let bc = d_interp(src[1], src[2], t);
            let values = [src[0], ab, d_interp(ab, bc, t), bc, src[2]];
            for (k, value) in values.into_iter().enumerate() {
                if axis == 0 {
                    dst.pts[k].x = value;
                } else {
                    dst.pts[k].y = value;
                }
            }
        }
        dst
    }

    /// `static bool pointInTriangle` helper: `hullIntersects(const SkDQuad& q2, bool* isLinear)`.
    /// Returns `None` when the hulls do not intersect, else `Some(is_linear)`.
    // Port of: src/pathops/SkPathOpsQuad.cpp (chrome/m156)
    #[doc(alias = "hullIntersects")]
    #[must_use]
    #[allow(clippy::collapsible_if)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn hull_intersects_quad(&self, q2: &Self) -> Option<bool> {
        let mut linear = true;
        for odd_man in 0..QUAD_POINT_COUNT {
            let end_pt = self.other_pts(odd_man);
            let orig_x = end_pt[0].x;
            let orig_y = end_pt[0].y;
            let adj = end_pt[1].x - orig_x;
            let opp = end_pt[1].y - orig_y;
            let sign = (self.pts[odd_man].y - orig_y) * adj - (self.pts[odd_man].x - orig_x) * opp;
            if approximately_zero(sign) {
                continue;
            }
            linear = false;
            let mut found_outlier = false;
            for n in 0..QUAD_POINT_COUNT {
                let test = (q2.pts[n].y - orig_y) * adj - (q2.pts[n].x - orig_x) * opp;
                if test * sign > 0.0 && !precisely_zero(test) {
                    found_outlier = true;
                    break;
                }
            }
            if !found_outlier {
                return None;
            }
        }
        if linear && !matches_end(&self.pts, q2.pts[0]) && !matches_end(&self.pts, q2.pts[2]) {
            if point_in_triangle(&self.pts, q2.pts[0]) || point_in_triangle(&self.pts, q2.pts[2]) {
                linear = false;
            }
        }
        Some(linear)
    }

    /// `hullIntersects(const SkDConic& conic, bool* isLinear)`.
    #[must_use]
    pub fn hull_intersects_conic(&self, conic: &DConic) -> Option<bool> {
        conic.hull_intersects_quad(self)
    }

    /// `hullIntersects(const SkDCubic& cubic, bool* isLinear)`.
    #[must_use]
    pub fn hull_intersects_cubic(&self, cubic: &DCubic) -> Option<bool> {
        cubic.hull_intersects_quad(self)
    }
}

impl Index<usize> for DQuad {
    type Output = DPoint;
    fn index(&self, n: usize) -> &DPoint {
        &self.pts[n]
    }
}

impl IndexMut<usize> for DQuad {
    fn index_mut(&mut self, n: usize) -> &mut DPoint {
        &mut self.pts[n]
    }
}
