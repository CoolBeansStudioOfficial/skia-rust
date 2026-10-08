// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsCubic.h, src/pathops/SkPathOpsCubic.cpp,
// src/pathops/SkOpCubicHull.cpp (SkDCubic::convexHull), src/pathops/SkDCubicToQuads.cpp

//! Double-precision cubic Bezier curves (`SkDCubic`, `SkDCubicPair`).

use std::ops::{Index, IndexMut};

use skia_rust_core::point::Point;
use skia_rust_core::scalar::double_to_scalar;

use crate::conic::DConic;
use crate::line_parameters::LineParameters;
use crate::point::{DPoint, DVector};
use crate::quad::{DQuad, QUAD_POINT_COUNT};
use crate::types::{
    FLT_EPSILON_ORDERABLE_ERR, almost_bequal_ulps, almost_dequal_ulps, approximately_equal,
    approximately_equal_half, approximately_one_or_less, approximately_zero,
    approximately_zero_or_more, approximately_zero_when_compared_to, between, d_interp,
    precisely_between, precisely_zero, std_max, std_min, zero_or_one,
};

/// `SkDCubic::kPointCount`.
pub const CUBIC_POINT_COUNT: usize = 4;
/// `SkDCubic::kPointLast`.
pub const CUBIC_POINT_LAST: usize = CUBIC_POINT_COUNT - 1;
/// `SkDCubic::kMaxIntersections`.
pub const CUBIC_MAX_INTERSECTIONS: usize = 9;
/// `SkDCubic::gPrecisionUnit`.
pub const PRECISION_UNIT: f64 = 256.0;

/// `SkDCubic::SearchAxis`.
#[doc(alias = "SkDCubic::SearchAxis")]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SearchAxis {
    XAxis,
    YAxis,
}

/// `SkDCubicPair`: a cubic chopped at a t value into two cubics sharing the middle point.
// Port of: src/pathops/SkPathOpsCubic.h (chrome/m156)
#[doc(alias = "SkDCubicPair")]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct DCubicPair {
    pub pts: [DPoint; 7],
}

impl DCubicPair {
    /// The first cubic of the pair.
    #[must_use]
    pub fn first(&self) -> DCubic {
        DCubic::new([self.pts[0], self.pts[1], self.pts[2], self.pts[3]])
    }

    /// The second cubic of the pair.
    #[must_use]
    pub fn second(&self) -> DCubic {
        DCubic::new([self.pts[3], self.pts[4], self.pts[5], self.pts[6]])
    }
}

/// `SkDCubic`: a double-precision cubic Bezier curve.
// Port of: src/pathops/SkPathOpsCubic.h (chrome/m156)
#[doc(alias = "SkDCubic")]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct DCubic {
    pub pts: [DPoint; 4],
}

/// Port of `interp_cubic_coords(const double* src, double t)`: `src` is one coordinate of the four
/// control points.
// Port of: src/pathops/SkPathOpsCubic.cpp#L95-L110 (chrome/m156)
fn interp_cubic_coords(src: [f64; 4], t: f64) -> f64 {
    let ab = d_interp(src[0], src[1], t);
    let bc = d_interp(src[1], src[2], t);
    let cd = d_interp(src[2], src[3], t);
    let abc = d_interp(ab, bc, t);
    let bcd = d_interp(bc, cd, t);
    d_interp(abc, bcd, t)
}

/// Port of `derivative_at_t`.
// Port of: src/pathops/SkPathOpsCubic.cpp#L246-L253 (chrome/m156)
#[allow(clippy::many_single_char_names)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
fn derivative_at_t(src: [f64; 4], t: f64) -> f64 {
    let one_t = 1.0 - t;
    let a = src[0];
    let b = src[1];
    let c = src[2];
    let d = src[3];
    3.0 * ((b - a) * one_t * one_t + 2.0 * (c - b) * t * one_t + (d - c) * t * t)
}

/// Port of `formulate_F1DotF2`.
// Port of: src/pathops/SkPathOpsCubic.cpp (chrome/m156)
fn formulate_f1_dot_f2(src: [f64; 4]) -> [f64; 4] {
    let a = src[1] - src[0];
    let b = src[2] - 2.0 * src[1] + src[0];
    let c = src[3] + 3.0 * (src[1] - src[2]) - src[0];
    [c * c, 3.0 * b * c, 2.0 * b * b + c * a, a * b]
}

/// Port of `other_two(int one, int two)`.
// Port of: src/pathops/SkPathOpsCubic.h#L173-L175 (chrome/m156)
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
pub fn other_two(one: usize, two: usize) -> usize {
    let one = one as i32;
    let two = two as i32;
    ((1 >> (3 - (one ^ two))) ^ 3) as usize
}

/// Port of `SkDQuad::RootsReal` for a `double s[3]` output (quadratics write at most two roots).
#[allow(clippy::many_single_char_names)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
fn quad_roots_real_into(a: f64, b: f64, c: f64, s: &mut [f64; 3]) -> usize {
    let mut s2 = [0.0; 2];
    let n = DQuad::roots_real(a, b, c, &mut s2);
    s[..n].copy_from_slice(&s2[..n]);
    n
}

impl DCubic {
    #[must_use]
    pub const fn new(pts: [DPoint; 4]) -> Self {
        Self { pts }
    }

    /// `static bool IsConic()`.
    #[must_use]
    pub const fn is_conic() -> bool {
        false
    }

    /// `const SkDCubic& set(const SkPoint pts[kPointCount])`.
    pub fn set(&mut self, pts: [Point; 4]) -> &mut Self {
        for (dst, src) in self.pts.iter_mut().zip(pts) {
            *dst = DPoint::from_sk_point(src);
        }
        self
    }

    /// `bool collapsed() const`.
    #[must_use]
    pub fn collapsed(&self) -> bool {
        self.pts[0].approximately_equal(self.pts[1])
            && self.pts[0].approximately_equal(self.pts[2])
            && self.pts[0].approximately_equal(self.pts[3])
    }

    /// `bool controlsInside() const`.
    #[must_use]
    pub fn controls_inside(&self) -> bool {
        let v01 = self.pts[0] - self.pts[1];
        let v02 = self.pts[0] - self.pts[2];
        let v03 = self.pts[0] - self.pts[3];
        let v13 = self.pts[1] - self.pts[3];
        let v23 = self.pts[2] - self.pts[3];
        v03.dot(v01) > 0.0 && v03.dot(v02) > 0.0 && v03.dot(v13) > 0.0 && v03.dot(v23) > 0.0
    }

    /// `void align(int endIndex, int ctrlIndex, SkDPoint* dstPt) const`.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L29-L36 (chrome/m156)
    #[allow(clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn align(&self, end_index: usize, ctrl_index: usize, dst_pt: &mut DPoint) {
        if self.pts[end_index].x == self.pts[ctrl_index].x {
            dst_pt.x = self.pts[end_index].x;
        }
        if self.pts[end_index].y == self.pts[ctrl_index].y {
            dst_pt.y = self.pts[end_index].y;
        }
    }

    /// `double binarySearch(double min, double max, double axisIntercept, SearchAxis xAxis) const`.
    /// Returns -1 when no point at the axis intercept is found.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L40-L85 (chrome/m156)
    #[must_use]
    #[allow(clippy::manual_midpoint)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn binary_search(
        &self,
        min: f64,
        max: f64,
        axis_intercept: f64,
        x_axis: SearchAxis,
    ) -> f64 {
        let axis = |p: DPoint| match x_axis {
            SearchAxis::XAxis => p.x,
            SearchAxis::YAxis => p.y,
        };
        let mut t = (min + max) / 2.0;
        let mut step = (t - min) / 2.0;
        let mut cubic_at_t = self.pt_at_t(t);
        let mut calc_pos = axis(cubic_at_t);
        let mut calc_dist = calc_pos - axis_intercept;
        loop {
            let prior_t = std_max(min, t - step);
            let less_pt = self.pt_at_t(prior_t);
            if approximately_equal_half(less_pt.x, cubic_at_t.x)
                && approximately_equal_half(less_pt.y, cubic_at_t.y)
            {
                return -1.0; // binary search found no point at this axis intercept
            }
            let less_dist = axis(less_pt) - axis_intercept;
            let last_step = step;
            step /= 2.0;
            if if calc_dist > 0.0 {
                calc_dist > less_dist
            } else {
                calc_dist < less_dist
            } {
                t = prior_t;
            } else {
                let next_t = t + last_step;
                if next_t > max {
                    return -1.0;
                }
                let more_pt = self.pt_at_t(next_t);
                if approximately_equal_half(more_pt.x, cubic_at_t.x)
                    && approximately_equal_half(more_pt.y, cubic_at_t.y)
                {
                    return -1.0; // binary search found no point at this axis intercept
                }
                let more_dist = axis(more_pt) - axis_intercept;
                if if calc_dist > 0.0 {
                    calc_dist <= more_dist
                } else {
                    calc_dist >= more_dist
                } {
                    continue;
                }
                t = next_t;
            }
            let test_at_t = self.pt_at_t(t);
            cubic_at_t = test_at_t;
            calc_pos = axis(cubic_at_t);
            calc_dist = calc_pos - axis_intercept;
            if approximately_equal(calc_pos, axis_intercept) {
                break;
            }
        }
        t
    }

    /// `double calcPrecision() const`.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L88-L92 (chrome/m156)
    #[must_use]
    pub fn calc_precision(&self) -> f64 {
        ((self.pts[1] - self.pts[0]).length()
            + (self.pts[2] - self.pts[1]).length()
            + (self.pts[3] - self.pts[2]).length())
            / PRECISION_UNIT
    }

    /// `SkDCubicPair chopAt(double t) const`.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L112-L132 (chrome/m156)
    #[must_use]
    #[allow(clippy::float_cmp, clippy::manual_midpoint)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn chop_at(&self, t: f64) -> DCubicPair {
        let mut dst = DCubicPair::default();
        let p = &self.pts;
        if t == 0.5 {
            dst.pts[0] = p[0];
            dst.pts[1].x = (p[0].x + p[1].x) / 2.0;
            dst.pts[1].y = (p[0].y + p[1].y) / 2.0;
            dst.pts[2].x = (p[0].x + 2.0 * p[1].x + p[2].x) / 4.0;
            dst.pts[2].y = (p[0].y + 2.0 * p[1].y + p[2].y) / 4.0;
            dst.pts[3].x = (p[0].x + 3.0 * (p[1].x + p[2].x) + p[3].x) / 8.0;
            dst.pts[3].y = (p[0].y + 3.0 * (p[1].y + p[2].y) + p[3].y) / 8.0;
            dst.pts[4].x = (p[1].x + 2.0 * p[2].x + p[3].x) / 4.0;
            dst.pts[4].y = (p[1].y + 2.0 * p[2].y + p[3].y) / 4.0;
            dst.pts[5].x = (p[2].x + p[3].x) / 2.0;
            dst.pts[5].y = (p[2].y + p[3].y) / 2.0;
            dst.pts[6] = p[3];
            return dst;
        }
        for (axis, src) in [self.xs(), self.ys()].into_iter().enumerate() {
            let ab = d_interp(src[0], src[1], t);
            let bc = d_interp(src[1], src[2], t);
            let cd = d_interp(src[2], src[3], t);
            let abc = d_interp(ab, bc, t);
            let bcd = d_interp(bc, cd, t);
            let abcd = d_interp(abc, bcd, t);
            let values = [src[0], ab, abc, abcd, bcd, cd, src[3]];
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

    /// `static void Coefficients(const double* cubic, double* A, double* B, double* C, double* D)`:
    /// `src` is one coordinate of the four control points.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L135-L143 (chrome/m156)
    #[doc(alias = "Coefficients")]
    #[must_use]
    pub fn coefficients(src: [f64; 4]) -> (f64, f64, f64, f64) {
        let mut a = src[3]; // d
        let mut b = src[2] * 3.0; // 3*c
        let mut c = src[1] * 3.0; // 3*b
        let d = src[0]; // a
        a -= d - c + b; // A =   -a + 3*b - 3*c + d
        b += 3.0 * d - 2.0 * c; // B =  3*a - 6*b + 3*c
        c -= 3.0 * d; // C = -3*a + 3*b
        (a, b, c, d)
    }

    /// `bool endsAreExtremaInXOrY() const`.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L145-L150 (chrome/m156)
    #[must_use]
    pub fn ends_are_extrema_in_x_or_y(&self) -> bool {
        let p = &self.pts;
        (between(p[0].x, p[1].x, p[3].x) && between(p[0].x, p[2].x, p[3].x))
            || (between(p[0].y, p[1].y, p[3].y) && between(p[0].y, p[2].y, p[3].y))
    }

    /// `int findInflections(double tValues[2]) const`.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L354-L354 (chrome/m156)
    #[must_use]
    pub fn find_inflections(&self, t_values: &mut [f64; 2]) -> usize {
        let p = &self.pts;
        let ax = p[1].x - p[0].x;
        let ay = p[1].y - p[0].y;
        let bx = p[2].x - 2.0 * p[1].x + p[0].x;
        let by = p[2].y - 2.0 * p[1].y + p[0].y;
        let cx = p[3].x + 3.0 * (p[1].x - p[2].x) - p[0].x;
        let cy = p[3].y + 3.0 * (p[1].y - p[2].y) - p[0].y;
        DQuad::roots_valid_t(
            bx * cy - by * cx,
            ax * cy - ay * cx,
            ax * by - ay * bx,
            t_values,
        )
    }

    /// `static int FindInflections(const SkPoint a[kPointCount], double tValues[2])`.
    #[doc(alias = "FindInflections")]
    #[must_use]
    pub fn find_inflections_sk(a: [Point; 4], t_values: &mut [f64; 2]) -> usize {
        let mut cubic = Self::default();
        cubic.set(a);
        cubic.find_inflections(t_values)
    }

    /// `static int FindExtrema(const double src[], double tValues[2])`: `src` is one coordinate of
    /// the four control points.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L554-L565 (chrome/m156)
    #[doc(alias = "FindExtrema")]
    #[must_use]
    pub fn find_extrema(src: [f64; 4], t_values: &mut [f64; 2]) -> usize {
        let a = src[0];
        let b = src[1];
        let c = src[2];
        let d = src[3];
        let big_a = d - a + 3.0 * (b - c);
        let big_b = 2.0 * (a - b - b + c);
        let big_c = b - a;
        DQuad::roots_valid_t(big_a, big_b, big_c, t_values)
    }

    /// `int findMaxCurvature(double tValues[]) const`.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L579-L588 (chrome/m156)
    #[must_use]
    pub fn find_max_curvature(&self, t_values: &mut [f64; 3]) -> usize {
        let coeff_x = formulate_f1_dot_f2(self.xs());
        let coeff_y = formulate_f1_dot_f2(self.ys());
        let mut sum = [0.0; 4];
        for i in 0..4 {
            sum[i] = coeff_x[i] + coeff_y[i];
        }
        Self::roots_valid_t(sum[0], sum[1], sum[2], sum[3], t_values)
    }

    /// `SkDVector dxdyAtT(double t) const`.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L508-L524 (chrome/m156)
    #[doc(alias = "dxdyAtT")]
    #[must_use]
    #[allow(clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn dxdy_at_t(&self, t: f64) -> DVector {
        let mut result = DVector::new(derivative_at_t(self.xs(), t), derivative_at_t(self.ys(), t));
        if result.x == 0.0 && result.y == 0.0 {
            if t == 0.0 {
                result = self.pts[2] - self.pts[0];
            } else if t == 1.0 {
                result = self.pts[3] - self.pts[1];
            }
            if result.x == 0.0 && result.y == 0.0 && zero_or_one(t) {
                result = self.pts[3] - self.pts[0];
            }
        }
        result
    }

    /// `SkDPoint ptAtT(double t) const`.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L44-L44 (chrome/m156)
    #[doc(alias = "ptAtT")]
    #[must_use]
    #[allow(clippy::float_cmp, clippy::many_single_char_names)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn pt_at_t(&self, t: f64) -> DPoint {
        if t == 0.0 {
            return self.pts[0];
        }
        if t == 1.0 {
            return self.pts[3];
        }
        let one_t = 1.0 - t;
        let one_t2 = one_t * one_t;
        let a = one_t2 * one_t;
        let b = 3.0 * one_t2 * t;
        let t2 = t * t;
        let c = 3.0 * one_t * t2;
        let d = t2 * t;
        let p = &self.pts;
        DPoint::new(
            a * p[0].x + b * p[1].x + c * p[2].x + d * p[3].x,
            a * p[0].y + b * p[1].y + c * p[2].y + d * p[3].y,
        )
    }

    /// `int RootsReal(double A, double B, double C, double D, double s[3])`.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L383-L383 (chrome/m156)
    #[doc(alias = "RootsReal")]
    #[must_use]
    #[allow(clippy::many_single_char_names, clippy::needless_range_loop)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn roots_real(a: f64, b: f64, c: f64, d: f64, s: &mut [f64; 3]) -> usize {
        if approximately_zero(a)
            && approximately_zero_when_compared_to(a, b)
            && approximately_zero_when_compared_to(a, c)
            && approximately_zero_when_compared_to(a, d)
        {
            // we're just a quadratic
            return quad_roots_real_into(b, c, d, s);
        }
        if approximately_zero_when_compared_to(d, a)
            && approximately_zero_when_compared_to(d, b)
            && approximately_zero_when_compared_to(d, c)
        {
            // 0 is one root
            let mut num = quad_roots_real_into(a, b, c, s);
            for i in 0..num {
                if approximately_zero(s[i]) {
                    return num;
                }
            }
            s[num] = 0.0;
            num += 1;
            return num;
        }
        if approximately_zero(a + b + c + d) {
            // 1 is one root
            let mut num = quad_roots_real_into(a, a + b, -d, s);
            for i in 0..num {
                if almost_dequal_ulps(s[i], 1.0) {
                    return num;
                }
            }
            s[num] = 1.0;
            num += 1;
            return num;
        }
        let (a_coef, b_coef, c_coef) = {
            let inv_a = 1.0 / a;
            (b * inv_a, c * inv_a, d * inv_a)
        };
        let a2 = a_coef * a_coef;
        let q = (a2 - b_coef * 3.0) / 9.0;
        let r_val = (2.0 * a2 * a_coef - 9.0 * a_coef * b_coef + 27.0 * c_coef) / 54.0;
        let r2 = r_val * r_val;
        let q3 = q * q * q;
        let r2_minus_q3 = r2 - q3;
        let adiv3 = a_coef / 3.0;
        let mut count = 0usize;
        if r2_minus_q3 < 0.0 {
            // we have 3 real roots
            // the divide/root can, due to finite precisions, be slightly outside of -1...1
            // skia-rust: libm (acos and cos are not guaranteed to match across libms)
            let theta = std_max(-1.0, std_min(r_val / q3.sqrt(), 1.0)).acos();
            let neg2_root_q = -2.0 * q.sqrt();

            let r = neg2_root_q * (theta / 3.0).cos() - adiv3;
            s[count] = r;
            count += 1;

            let r = neg2_root_q * ((theta + 2.0 * std::f64::consts::PI) / 3.0).cos() - adiv3;
            if !almost_dequal_ulps(s[0], r) {
                s[count] = r;
                count += 1;
            }
            let r = neg2_root_q * ((theta - 2.0 * std::f64::consts::PI) / 3.0).cos() - adiv3;
            if !almost_dequal_ulps(s[0], r) && (count == 1 || !almost_dequal_ulps(s[1], r)) {
                s[count] = r;
                count += 1;
            }
        } else {
            // we have 1 real root
            let sqrt_r2_minus_q3 = r2_minus_q3.sqrt();
            let mut big_a = r_val.abs() + sqrt_r2_minus_q3;
            big_a = big_a.cbrt(); // cube root; skia-rust: libm
            if r_val > 0.0 {
                big_a = -big_a;
            }
            if big_a != 0.0 {
                big_a += q / big_a;
            }
            let r = big_a - adiv3;
            s[count] = r;
            count += 1;
            if almost_dequal_ulps(r2, q3) {
                let r = -big_a / 2.0 - adiv3;
                if !almost_dequal_ulps(s[0], r) {
                    s[count] = r;
                    count += 1;
                }
            }
        }
        count
    }

    /// `static int RootsValidT(double A, double B, double C, double D, double t[3])`.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L381-L408 (chrome/m156)
    #[doc(alias = "RootsValidT")]
    #[must_use]
    #[allow(clippy::collapsible_if, clippy::many_single_char_names)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn roots_valid_t(a: f64, b: f64, c: f64, d: f64, t: &mut [f64; 3]) -> usize {
        let mut s = [0.0; 3];
        let real_roots = Self::roots_real(a, b, c, d, &mut s);
        let mut found_roots = DQuad::add_valid_ts(&s[..real_roots], t);
        for &t_value in &s[..real_roots] {
            if !approximately_one_or_less(t_value) && between(1.0, t_value, 1.00005) {
                if !t[..found_roots]
                    .iter()
                    .any(|&prev| approximately_equal(prev, 1.0))
                {
                    debug_assert!(found_roots < 3);
                    t[found_roots] = 1.0;
                    found_roots += 1;
                }
            } else if !approximately_zero_or_more(t_value) && between(-0.00005, t_value, 0.0) {
                if !t[..found_roots]
                    .iter()
                    .any(|&prev| approximately_equal(prev, 0.0))
                {
                    debug_assert!(found_roots < 3);
                    t[found_roots] = 0.0;
                    found_roots += 1;
                }
            }
        }
        found_roots
    }

    /// `SkDCubic subDivide(double t1, double t2) const`.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L665-L693 (chrome/m156)
    #[must_use]
    #[allow(clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn sub_divide(&self, t1: f64, t2: f64) -> Self {
        if t1 == 0.0 || t2 == 1.0 {
            if t1 == 0.0 && t2 == 1.0 {
                return *self;
            }
            let pair = self.chop_at(if t1 == 0.0 { t2 } else { t1 });
            return if t1 == 0.0 {
                pair.first()
            } else {
                pair.second()
            };
        }
        let xs = self.xs();
        let ys = self.ys();
        let mut dst = Self::default();
        let ax = interp_cubic_coords(xs, t1);
        dst.pts[0].x = ax;
        let ay = interp_cubic_coords(ys, t1);
        dst.pts[0].y = ay;
        let ex = interp_cubic_coords(xs, (t1 * 2.0 + t2) / 3.0);
        let ey = interp_cubic_coords(ys, (t1 * 2.0 + t2) / 3.0);
        let fx = interp_cubic_coords(xs, (t1 + t2 * 2.0) / 3.0);
        let fy = interp_cubic_coords(ys, (t1 + t2 * 2.0) / 3.0);
        let dx = interp_cubic_coords(xs, t2);
        dst.pts[3].x = dx;
        let dy = interp_cubic_coords(ys, t2);
        dst.pts[3].y = dy;
        let mx = ex * 27.0 - ax * 8.0 - dx;
        let my = ey * 27.0 - ay * 8.0 - dy;
        let nx = fx * 27.0 - ax - dx * 8.0;
        let ny = fy * 27.0 - ay - dy * 8.0;
        dst.pts[1].x = (mx * 2.0 - nx) / 18.0;
        dst.pts[1].y = (my * 2.0 - ny) / 18.0;
        dst.pts[2].x = (nx * 2.0 - mx) / 18.0;
        dst.pts[2].y = (ny * 2.0 - my) / 18.0;
        dst
    }

    /// `void subDivide(const SkDPoint& a, const SkDPoint& d, double t1, double t2,
    /// SkDPoint dst[2]) const`.
    // Port of: src/pathops/SkPathOpsCubic.cpp (chrome/m156)
    #[must_use]
    #[allow(
        clippy::bool_to_int_with_if,
        clippy::float_cmp,
        clippy::manual_assert_eq
    )] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn sub_divide_ad(&self, a: DPoint, d: DPoint, t1: f64, t2: f64) -> [DPoint; 2] {
        debug_assert!(t1 != t2);
        let sub = self.sub_divide(t1, t2);
        let mut dst = [sub.pts[1] + (a - sub.pts[0]), sub.pts[2] + (d - sub.pts[3])];
        if t1 == 0.0 || t2 == 0.0 {
            let idx = if t1 == 0.0 { 0 } else { 1 };
            let mut pt = dst[idx];
            self.align(0, 1, &mut pt);
            dst[idx] = pt;
        }
        if t1 == 1.0 || t2 == 1.0 {
            let idx = if t1 == 1.0 { 0 } else { 1 };
            let mut pt = dst[idx];
            self.align(3, 2, &mut pt);
            dst[idx] = pt;
        }
        if almost_bequal_ulps(dst[0].x, a.x) {
            dst[0].x = a.x;
        }
        if almost_bequal_ulps(dst[0].y, a.y) {
            dst[0].y = a.y;
        }
        if almost_bequal_ulps(dst[1].x, d.x) {
            dst[1].x = d.x;
        }
        if almost_bequal_ulps(dst[1].y, d.y) {
            dst[1].y = d.y;
        }
        dst
    }

    /// `double top(const SkDCubic& dCurve, double startT, double endT, SkDPoint* topPt) const`.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L734-L747 (chrome/m156)
    #[must_use]
    #[allow(clippy::float_cmp, clippy::similar_names)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn top(&self, d_curve: &Self, start_t: f64, end_t: f64, top_pt: &mut DPoint) -> f64 {
        let mut extreme_ts = [0.0; 2];
        let mut top_t = -1.0;
        let roots = Self::find_extrema(self.ys(), &mut extreme_ts);
        for &extreme in &extreme_ts[..roots] {
            let t = start_t + (end_t - start_t) * extreme;
            let mid = d_curve.pt_at_t(t);
            if top_pt.y > mid.y || (top_pt.y == mid.y && top_pt.x > mid.x) {
                top_t = t;
                *top_pt = mid;
            }
        }
        top_t
    }

    /// `bool toFloatPoints(SkPoint* pts) const`: `None` if a coordinate is not finite.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L722-L732 (chrome/m156)
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn to_float_points(&self) -> Option<[Point; 4]> {
        let mut out = [Point::new(0.0, 0.0); 4];
        let mut floats = [0.0_f32; 8];
        for (index, value) in floats.iter_mut().enumerate() {
            let d = match index % 2 {
                0 => self.pts[index / 2].x,
                _ => self.pts[index / 2].y,
            };
            let mut f = double_to_scalar(d);
            if f.abs() < FLT_EPSILON_ORDERABLE_ERR as f32 {
                f = 0.0;
            }
            *value = f;
        }
        for (i, pt) in out.iter_mut().enumerate() {
            *pt = Point::new(floats[2 * i], floats[2 * i + 1]);
        }
        if floats.iter().all(|f| f.is_finite()) {
            Some(out)
        } else {
            None
        }
    }

    /// `SkDQuad toQuad() const`: the quadratic that approximates this cubic
    /// (`src/pathops/SkDCubicToQuads.cpp`).
    // Port of: src/pathops/SkDCubicToQuads.cpp#L36-L45 (chrome/m156)
    #[must_use]
    #[allow(clippy::manual_midpoint)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn to_quad(&self) -> DQuad {
        let mut quad = DQuad::default();
        quad[0] = self.pts[0];
        let p = &self.pts;
        let from_c1 = DPoint::new((3.0 * p[1].x - p[0].x) / 2.0, (3.0 * p[1].y - p[0].y) / 2.0);
        let from_c2 = DPoint::new((3.0 * p[2].x - p[3].x) / 2.0, (3.0 * p[2].y - p[3].y) / 2.0);
        quad[1].x = (from_c1.x + from_c2.x) / 2.0;
        quad[1].y = (from_c1.y + from_c2.y) / 2.0;
        quad[2] = p[3];
        quad
    }

    /// `bool isLinear(int startIndex, int endIndex) const`.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L221-L240 (chrome/m156)
    #[must_use]
    pub fn is_linear(&self, start_index: usize, end_index: usize) -> bool {
        if self.pts[0].approximately_d_equal(self.pts[3]) {
            return DQuad::new([self.pts[0], self.pts[1], self.pts[2]]).is_linear(0, 2);
        }
        let mut line_parameters = LineParameters::default();
        line_parameters.cubic_end_points(self, start_index, end_index);
        line_parameters.normalize();
        let p = &self.pts;
        let tiniest = std_min(
            std_min(
                std_min(
                    std_min(
                        std_min(std_min(std_min(p[0].x, p[0].y), p[1].x), p[1].y),
                        p[2].x,
                    ),
                    p[2].y,
                ),
                p[3].x,
            ),
            p[3].y,
        );
        let mut largest = std_max(
            std_max(
                std_max(
                    std_max(
                        std_max(std_max(std_max(p[0].x, p[0].y), p[1].x), p[1].y),
                        p[2].x,
                    ),
                    p[2].y,
                ),
                p[3].x,
            ),
            p[3].y,
        );
        largest = std_max(largest, -tiniest);
        let mut distance = line_parameters.control_pt_distance_cubic(self, 1);
        if !approximately_zero_when_compared_to(distance, largest) {
            return false;
        }
        distance = line_parameters.control_pt_distance_cubic(self, 2);
        approximately_zero_when_compared_to(distance, largest)
    }

    /// `bool monotonicInX() const`.
    #[must_use]
    pub fn monotonic_in_x(&self) -> bool {
        let p = &self.pts;
        precisely_between(p[0].x, p[1].x, p[3].x) && precisely_between(p[0].x, p[2].x, p[3].x)
    }

    /// `bool monotonicInY() const`.
    #[must_use]
    pub fn monotonic_in_y(&self) -> bool {
        let p = &self.pts;
        precisely_between(p[0].y, p[1].y, p[3].y) && precisely_between(p[0].y, p[2].y, p[3].y)
    }

    /// `void otherPts(int index, const SkDPoint* o1Pts[kPointCount - 1]) const`: the three
    /// points other than `index`, in order.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L345-L350 (chrome/m156)
    #[must_use]
    pub fn other_pts(&self, index: usize) -> [DPoint; 3] {
        let offset = usize::from(index == 0);
        [self.pts[offset], self.pts[offset + 1], self.pts[offset + 2]]
    }

    /// `int searchRoots(double extremeTs[6], int extrema, double axisIntercept, SearchAxis xAxis,
    /// double* validRoots) const`.
    // Port of: src/pathops/SkPathOpsCubic.cpp#L352-L375 (chrome/m156)
    #[must_use]
    #[allow(clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn search_roots(
        &self,
        extreme_ts: &mut [f64; 6],
        mut extrema: usize,
        axis_intercept: f64,
        x_axis: SearchAxis,
        valid_roots: &mut [f64],
    ) -> usize {
        let mut inflections = [0.0; 2];
        let inflection_count = self.find_inflections(&mut inflections);
        extreme_ts[extrema..extrema + inflection_count]
            .copy_from_slice(&inflections[..inflection_count]);
        extrema += inflection_count;
        extreme_ts[extrema] = 0.0;
        extrema += 1;
        extreme_ts[extrema] = 1.0;
        debug_assert!(extrema < 6);
        // SkTQSort(extremeTs, extremeTs + extrema + 1)
        extreme_ts[..=extrema]
            .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut valid_count = 0;
        let mut index = 0;
        while index < extrema {
            let min = extreme_ts[index];
            index += 1;
            let max = extreme_ts[index];
            if min == max {
                continue;
            }
            let new_t = self.binary_search(min, max, axis_intercept, x_axis);
            if new_t >= 0.0 {
                if valid_count >= 3 {
                    return 0;
                }
                valid_roots[valid_count] = new_t;
                valid_count += 1;
            }
        }
        valid_count
    }

    /// `int convexHull(char order[kPointCount]) const`: the convex hull of the control points, as
    /// indices in order. Returns the number of points in the hull (3 or 4).
    // Port of: src/pathops/SkOpCubicHull.cpp#L60-L155 (chrome/m156)
    #[doc(alias = "convexHull")]
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::float_cmp,
        clippy::similar_names
    )] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn convex_hull(&self, order: &mut [u8; 4]) -> usize {
        let mut y_min = 0usize;
        for index in 1..4 {
            if self.pts[y_min].y > self.pts[index].y
                || (self.pts[y_min].y == self.pts[index].y && self.pts[y_min].x > self.pts[index].x)
            {
                y_min = index;
            }
        }
        order[0] = y_min as u8;
        let mut mid_x: Option<usize> = None;
        let mut backup_y_min: Option<usize> = None;
        for _pass in 0..2 {
            for index in 0..4 {
                if index == y_min {
                    continue;
                }
                let mask = other_two(y_min, index);
                let side1 = y_min ^ mask;
                let side2 = index ^ mask;
                let Some(rot_path) = rotate(self, y_min, index) else {
                    // ! if cbc[yMin]==cbc[idx]
                    order[1] = side1 as u8;
                    order[2] = side2 as u8;
                    return 3;
                };
                let mut sides = side(rot_path.pts[side1].y - rot_path.pts[y_min].y);
                sides ^= side(rot_path.pts[side2].y - rot_path.pts[y_min].y);
                if sides == 2 {
                    // '2' means one remaining point <0, one >0
                    if mid_x.is_some() {
                        order[0] = 0;
                        order[1] = 3;
                        if self.pts[1] == self.pts[0] || self.pts[1] == self.pts[3] {
                            order[2] = 2;
                            return 3;
                        }
                        if self.pts[2] == self.pts[0] || self.pts[2] == self.pts[3] {
                            order[2] = 1;
                            return 3;
                        }
                        let dist1_0 = self.pts[1].distance_squared(self.pts[0]);
                        let dist1_3 = self.pts[1].distance_squared(self.pts[3]);
                        let dist2_0 = self.pts[2].distance_squared(self.pts[0]);
                        let dist2_3 = self.pts[2].distance_squared(self.pts[3]);
                        let smallest1_dist_sq = std_min(dist1_0, dist1_3);
                        let smallest2_dist_sq = std_min(dist2_0, dist2_3);
                        if approximately_zero(std_min(smallest1_dist_sq, smallest2_dist_sq)) {
                            order[2] = if smallest1_dist_sq < smallest2_dist_sq {
                                2
                            } else {
                                1
                            };
                            return 3;
                        }
                    }
                    mid_x = Some(index);
                } else if sides == 0 {
                    // '0' means both to one side or the other
                    backup_y_min = Some(index);
                }
            }
            if mid_x.is_some() {
                break;
            }
            let Some(backup) = backup_y_min else {
                break;
            };
            y_min = backup;
            backup_y_min = None;
        }
        let mid_x = mid_x.unwrap_or(y_min ^ 3); // choose any other point
        let mask = other_two(y_min, mid_x);
        let least = y_min ^ mask;
        let most = mid_x ^ mask;
        order[0] = y_min as u8;
        order[1] = least as u8;
        let Some(mid_path) = rotate(self, least, most) else {
            // ! if cbc[least]==cbc[most]
            order[2] = mid_x as u8;
            return 3;
        };
        let mut mid_sides = side(mid_path.pts[y_min].y - mid_path.pts[least].y);
        mid_sides ^= side(mid_path.pts[mid_x].y - mid_path.pts[least].y);
        if mid_sides != 2 {
            // if mid point is not between
            order[2] = most as u8;
            return 3; // result is a triangle
        }
        order[2] = mid_x as u8;
        order[3] = most as u8;
        4 // result is a quadralateral
    }

    /// `bool hullIntersects(const SkDPoint* pts, int ptCount, bool* isLinear) const`: `None` when
    /// the hulls do not intersect, else `Some(is_linear)`.
    // Port of: src/pathops/SkOpCubicHull.cpp (chrome/m156)
    #[must_use]
    pub fn hull_intersects_pts(&self, pts: &[DPoint]) -> Option<bool> {
        let mut linear = true;
        let mut hull_order = [0u8; 4];
        let hull_count = self.convex_hull(&mut hull_order);
        let mut end1 = hull_order[0] as usize;
        let mut hull_index = 0usize;
        let mut end_pt = [self.pts[end1], DPoint::default()];
        loop {
            hull_index = (hull_index + 1) % hull_count;
            let end2 = hull_order[hull_index] as usize;
            end_pt[1] = self.pts[end2];
            let orig_x = end_pt[0].x;
            let orig_y = end_pt[0].y;
            let adj = end_pt[1].x - orig_x;
            let opp = end_pt[1].y - orig_y;
            let odd_man_mask = other_two(end1, end2);
            let odd_man = end1 ^ odd_man_mask;
            let mut sign =
                (self.pts[odd_man].y - orig_y) * adj - (self.pts[odd_man].x - orig_x) * opp;
            let odd_man2 = end2 ^ odd_man_mask;
            let sign2 =
                (self.pts[odd_man2].y - orig_y) * adj - (self.pts[odd_man2].x - orig_x) * opp;
            let mut skip = sign * sign2 < 0.0;
            if !skip && approximately_zero(sign) {
                sign = sign2;
                if approximately_zero(sign) {
                    skip = true;
                }
            }
            if !skip {
                linear = false;
                let mut found_outlier = false;
                for pt in pts {
                    let test = (pt.y - orig_y) * adj - (pt.x - orig_x) * opp;
                    if test * sign > 0.0 && !precisely_zero(test) {
                        found_outlier = true;
                        break;
                    }
                }
                if !found_outlier {
                    return None;
                }
                end_pt[0] = end_pt[1];
                end1 = end2;
            }
            if hull_index == 0 {
                break;
            }
        }
        Some(linear)
    }

    /// `hullIntersects(const SkDCubic& c2, bool* isLinear)`.
    #[must_use]
    pub fn hull_intersects_cubic(&self, c2: &Self) -> Option<bool> {
        self.hull_intersects_pts(&c2.pts)
    }

    /// `hullIntersects(const SkDQuad& quad, bool* isLinear)`.
    #[must_use]
    pub fn hull_intersects_quad(&self, quad: &DQuad) -> Option<bool> {
        self.hull_intersects_pts(&quad.pts[..QUAD_POINT_COUNT])
    }

    /// `hullIntersects(const SkDConic& conic, bool* isLinear)`.
    #[must_use]
    pub fn hull_intersects_conic(&self, conic: &DConic) -> Option<bool> {
        self.hull_intersects_quad(&conic.pts)
    }

    /// `SkDQuad toQuad() const` is [`DCubic::to_quad`]. Returns the x coordinates of the points.
    pub(crate) fn xs(&self) -> [f64; 4] {
        [self.pts[0].x, self.pts[1].x, self.pts[2].x, self.pts[3].x]
    }

    /// The y coordinates of the points.
    pub(crate) fn ys(&self) -> [f64; 4] {
        [self.pts[0].y, self.pts[1].y, self.pts[2].y, self.pts[3].y]
    }
}

/// Port of `side`: 0 if negative, 1 if zero, 2 if positive.
// Port of: src/pathops/SkOpCubicHull.cpp#L45-L47 (chrome/m156)
fn side(x: f64) -> usize {
    usize::from(x > 0.0) + usize::from(x >= 0.0)
}

/// Port of `rotate`: rotates `cubic` so that `cubic[zero]` to `cubic[index]` is the x axis.
/// Returns `None` when the two points coincide.
// Port of: src/pathops/SkOpCubicHull.cpp#L14-L41 (chrome/m156)
fn rotate(cubic: &DCubic, zero: usize, index: usize) -> Option<DCubic> {
    let dy = cubic.pts[index].y - cubic.pts[zero].y;
    let dx = cubic.pts[index].x - cubic.pts[zero].x;
    let mut rot_path = *cubic;
    if approximately_zero(dy) {
        if approximately_zero(dx) {
            return None;
        }
        if dy != 0.0 {
            rot_path.pts[index].y = cubic.pts[zero].y;
            let mask = other_two(index, zero);
            let side1 = index ^ mask;
            let side2 = zero ^ mask;
            if approximately_equal(cubic.pts[side1].y, cubic.pts[zero].y) {
                rot_path.pts[side1].y = cubic.pts[zero].y;
            }
            if approximately_equal(cubic.pts[side2].y, cubic.pts[zero].y) {
                rot_path.pts[side2].y = cubic.pts[zero].y;
            }
        }
        return Some(rot_path);
    }
    for i in 0..4 {
        rot_path.pts[i].x = cubic.pts[i].x * dx + cubic.pts[i].y * dy;
        rot_path.pts[i].y = cubic.pts[i].y * dx - cubic.pts[i].x * dy;
    }
    Some(rot_path)
}

impl Index<usize> for DCubic {
    type Output = DPoint;
    fn index(&self, n: usize) -> &DPoint {
        &self.pts[n]
    }
}

impl IndexMut<usize> for DCubic {
    fn index_mut(&mut self, n: usize) -> &mut DPoint {
        &mut self.pts[n]
    }
}
