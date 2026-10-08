// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkDLineIntersection.cpp

//! Line/line and line/axis-aligned-segment intersection routines of `SkIntersections`.

use crate::intersections::Intersections;
use crate::line::DLine;
use crate::point::{DPoint, DVector};
use crate::types::{
    almost_equal_ulps, approximately_equal, approximately_zero, between, not_almost_dequal_ulps,
    not_almost_equal_ulps_pin, pin_t, precisely_between, zero_or_one,
};

/// `horizontal_coincident`.
// Port of: src/pathops/SkDLineIntersection.cpp#L189-L203 (chrome/m156)
fn horizontal_coincident(line: &DLine, y: f64) -> i32 {
    let mut min = line[0].y;
    let mut max = line[1].y;
    if min > max {
        std::mem::swap(&mut min, &mut max);
    }
    if min > y || max < y {
        return 0;
    }
    if almost_equal_ulps(min, max) && max - min < (line[0].x - line[1].x).abs() {
        return 2;
    }
    1
}

/// `vertical_coincident`.
// Port of: src/pathops/SkDLineIntersection.cpp#L267-L281 (chrome/m156)
fn vertical_coincident(line: &DLine, x: f64) -> i32 {
    let mut min = line[0].x;
    let mut max = line[1].x;
    if min > max {
        std::mem::swap(&mut min, &mut max);
    }
    if !precisely_between(min, x, max) {
        return 0;
    }
    if almost_equal_ulps(min, max) {
        return 2;
    }
    1
}

impl Intersections {
    /// `void cleanUpParallelLines(bool parallel)`.
    // Port of: src/pathops/SkDLineIntersection.cpp#L18-L38 (chrome/m156)
    #[allow(clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub(crate) fn clean_up_parallel_lines(&mut self, parallel: bool) {
        while self.used > 2 {
            self.remove_one(1);
        }
        if self.used == 2 && !parallel {
            let start_match = self.t[0][0] == 0.0 || zero_or_one(self.t[1][0]);
            let end_match = self.t[0][1] == 1.0 || zero_or_one(self.t[1][1]);
            if (!start_match && !end_match) || approximately_equal(self.t[0][0], self.t[0][1]) {
                debug_assert!(start_match || end_match);
                if start_match
                    && end_match
                    && (self.t[0][0] != 0.0 || !zero_or_one(self.t[1][0]))
                    && self.t[0][1] == 1.0
                    && zero_or_one(self.t[1][1])
                {
                    self.remove_one(0);
                } else {
                    self.remove_one(usize::from(end_match));
                }
            }
        }
        if self.used == 2 {
            self.is_coincident = [0x03, 0x03];
        }
    }

    /// `void computePoints(const SkDLine& line, int used)`.
    // Port of: src/pathops/SkDLineIntersection.cpp#L40-L45 (chrome/m156)
    pub(crate) fn compute_points(&mut self, line: &DLine, used: u8) {
        self.pt[0] = line.pt_at_t(self.t[0][0]);
        self.used = used;
        if self.used == 2 {
            self.pt[1] = line.pt_at_t(self.t[0][1]);
        }
    }

    /// `int intersectRay(const SkDLine& a, const SkDLine& b)`.
    // Port of: src/pathops/SkDLineIntersection.cpp (chrome/m156)
    #[doc(alias = "intersectRay")]
    #[allow(clippy::if_not_else, clippy::needless_late_init)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn intersect_ray_line(&mut self, a: &DLine, b: &DLine) -> usize {
        self.max = 2;
        let a_len: DVector = a[1] - a[0];
        let b_len: DVector = b[1] - b[0];
        // Slopes match when denom goes to zero (see the C++ derivation).
        let denom = b_len.y * a_len.x - a_len.y * b_len.x;
        let used: u8;
        if !approximately_zero(denom) {
            let ab0 = a[0] - b[0];
            let mut numer_a = ab0.y * b_len.x - b_len.y * ab0.x;
            let mut numer_b = ab0.y * a_len.x - a_len.y * ab0.x;
            numer_a /= denom;
            numer_b /= denom;
            self.t[0][0] = numer_a;
            self.t[1][0] = numer_b;
            used = 1;
        } else {
            // See if the axis intercepts match.
            if !almost_equal_ulps(
                a_len.x * a[0].y - a_len.y * a[0].x,
                a_len.x * b[0].y - a_len.y * b[0].x,
            ) {
                self.used = 0;
                return 0;
            }
            self.t[0][0] = 0.0;
            self.t[1][0] = 0.0;
            self.t[1][0] = 1.0;
            self.t[1][1] = 1.0;
            used = 2;
        }
        self.compute_points(a, used);
        usize::from(self.used)
    }

    /// `int intersect(const SkDLine& a, const SkDLine& b)`.
    // Port of: src/pathops/SkDLineIntersection.cpp (chrome/m156)
    #[doc(alias = "intersect")]
    #[allow(
        clippy::cast_precision_loss,
        clippy::manual_assert_eq,
        clippy::similar_names
    )] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn intersect_line_line(&mut self, a: &DLine, b: &DLine) -> usize {
        self.max = 3; // note that we clean up so that there is no more than two in the end
        for i_a in 0..2 {
            let t = b.exact_point(a[i_a]);
            if t >= 0.0 {
                self.insert(i_a as f64, t, a[i_a]);
            }
        }
        for i_b in 0..2 {
            let t = a.exact_point(b[i_b]);
            if t >= 0.0 {
                self.insert(t, i_b as f64, b[i_b]);
            }
        }
        // Determine the intersection point of two line segments
        // from: http://paulbourke.net/geometry/lineline2d/
        let ax_len = a[1].x - a[0].x;
        let ay_len = a[1].y - a[0].y;
        let bx_len = b[1].x - b[0].x;
        let by_len = b[1].y - b[0].y;
        // Slopes match when denom goes to zero (see the C++ derivation).
        let ax_by_len = ax_len * by_len;
        let ay_bx_len = ay_len * bx_len;
        let unparallel = if self.allow_near {
            not_almost_equal_ulps_pin(ax_by_len, ay_bx_len)
        } else {
            not_almost_dequal_ulps(ax_by_len, ay_bx_len)
        };
        if unparallel && self.used == 0 {
            let ab0y = a[0].y - b[0].y;
            let ab0x = a[0].x - b[0].x;
            let numer_a = ab0y * bx_len - by_len * ab0x;
            let numer_b = ab0y * ax_len - ay_len * ab0x;
            let denom = ax_by_len - ay_bx_len;
            if between(0.0, numer_a, denom) && between(0.0, numer_b, denom) {
                self.t[0][0] = numer_a / denom;
                self.t[1][0] = numer_b / denom;
                self.compute_points(a, 1);
            }
        }
        // Coincident -- even when the end points are not exactly the same. Mark this as a 'wild
        // card' for the end points, so that either point is considered totally coincident.
        // Then, avoid folding the lines over each other, but allow either end to mate to the next
        // set of lines.
        if self.allow_near || !unparallel {
            let mut a_near_b = [0.0_f64; 2];
            let mut b_near_a = [0.0_f64; 2];
            let mut a_not_b = [false; 2];
            let mut b_not_a = [false; 2];
            let mut near_count = 0;
            for index in 0..2 {
                let t = b.near_point(a[index], Some(&mut a_not_b[index]));
                a_near_b[index] = t;
                near_count += i32::from(t >= 0.0);
                let t = a.near_point(b[index], Some(&mut b_not_a[index]));
                b_near_a[index] = t;
                near_count += i32::from(t >= 0.0);
            }
            if near_count > 0 {
                if near_count != 2 || a_not_b[0] == a_not_b[1] {
                    for i_a in 0..2 {
                        if !a_not_b[i_a] {
                            continue;
                        }
                        let nearer = usize::from(a_near_b[i_a] > 0.5);
                        if !b_not_a[nearer] {
                            continue;
                        }
                        debug_assert!(a[i_a] != b[nearer]);
                        self.insert_near(i_a as f64, nearer as f64, a[i_a], b[nearer]);
                        a_near_b[i_a] = -1.0;
                        b_near_a[nearer] = -1.0;
                        near_count -= 2;
                    }
                }
                if near_count > 0 {
                    for i_a in 0..2 {
                        if a_near_b[i_a] >= 0.0 {
                            self.insert(i_a as f64, a_near_b[i_a], a[i_a]);
                        }
                    }
                    for i_b in 0..2 {
                        if b_near_a[i_b] >= 0.0 {
                            self.insert(b_near_a[i_b], i_b as f64, b[i_b]);
                        }
                    }
                }
            }
        }
        self.clean_up_parallel_lines(!unparallel);
        debug_assert!(self.used <= 2);
        usize::from(self.used)
    }

    /// `static double HorizontalIntercept(const SkDLine& line, double y)`.
    // Port of: src/pathops/SkDLineIntersection.cpp (chrome/m156)
    #[doc(alias = "HorizontalIntercept")]
    #[must_use]
    #[allow(clippy::float_cmp, clippy::manual_assert_eq)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn horizontal_intercept_line(line: &DLine, y: f64) -> f64 {
        debug_assert!(line[1].y != line[0].y);
        pin_t((y - line[0].y) / (line[1].y - line[0].y))
    }

    /// `static double VerticalIntercept(const SkDLine& line, double x)`.
    // Port of: src/pathops/SkDLineIntersection.cpp (chrome/m156)
    #[doc(alias = "VerticalIntercept")]
    #[must_use]
    #[allow(clippy::float_cmp, clippy::manual_assert_eq)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn vertical_intercept_line(line: &DLine, x: f64) -> f64 {
        debug_assert!(line[1].x != line[0].x);
        pin_t((x - line[0].x) / (line[1].x - line[0].x))
    }

    /// `int horizontal(const SkDLine& line, double left, double right, double y, bool flipped)`.
    // Port of: src/pathops/SkDLineIntersection.cpp (chrome/m156)
    #[allow(clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn horizontal_line(
        &mut self,
        line: &DLine,
        left: f64,
        right: f64,
        y: f64,
        flipped: bool,
    ) -> usize {
        self.max = 3; // clean up parallel at the end will limit the result to 2 at the most
        let flipped_f = f64::from(u8::from(flipped));
        let left_pt = DPoint::new(left, y);
        let t = line.exact_point(left_pt);
        if t >= 0.0 {
            self.insert(t, flipped_f, left_pt);
        }
        if left != right {
            let right_pt = DPoint::new(right, y);
            let t = line.exact_point(right_pt);
            if t >= 0.0 {
                self.insert(t, f64::from(u8::from(!flipped)), right_pt);
            }
            for index in 0..2 {
                let t = DLine::exact_point_h(line[index], left, right, y);
                if t >= 0.0 {
                    self.insert(index as f64, if flipped { 1.0 - t } else { t }, line[index]);
                }
            }
        }
        let result = horizontal_coincident(line, y);
        if result == 1 && self.used == 0 {
            self.t[0][0] = Self::horizontal_intercept_line(line, y);
            let x_intercept = line[0].x + self.t[0][0] * (line[1].x - line[0].x);
            if between(left, x_intercept, right) {
                self.t[1][0] = (x_intercept - left) / (right - left);
                if flipped {
                    for index in 0..result as usize {
                        self.t[1][index] = 1.0 - self.t[1][index];
                    }
                }
                self.pt[0].x = x_intercept;
                self.pt[0].y = y;
                self.used = 1;
            }
        }
        if self.allow_near || result == 2 {
            let t = line.near_point(left_pt, None);
            if t >= 0.0 {
                self.insert(t, flipped_f, left_pt);
            }
            if left != right {
                let right_pt = DPoint::new(right, y);
                let t = line.near_point(right_pt, None);
                if t >= 0.0 {
                    self.insert(t, f64::from(u8::from(!flipped)), right_pt);
                }
                for index in 0..2 {
                    let t = DLine::near_point_h(line[index], left, right, y);
                    if t >= 0.0 {
                        self.insert(index as f64, if flipped { 1.0 - t } else { t }, line[index]);
                    }
                }
            }
        }
        self.clean_up_parallel_lines(result == 2);
        usize::from(self.used)
    }

    /// `int vertical(const SkDLine& line, double top, double bottom, double x, bool flipped)`.
    // Port of: src/pathops/SkDLineIntersection.cpp (chrome/m156)
    #[allow(clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::float_cmp)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn vertical_line(
        &mut self,
        line: &DLine,
        top: f64,
        bottom: f64,
        x: f64,
        flipped: bool,
    ) -> usize {
        self.max = 3; // cleanup parallel lines will bring this back line
        let flipped_f = f64::from(u8::from(flipped));
        let top_pt = DPoint::new(x, top);
        let t = line.exact_point(top_pt);
        if t >= 0.0 {
            self.insert(t, flipped_f, top_pt);
        }
        if top != bottom {
            let bottom_pt = DPoint::new(x, bottom);
            let t = line.exact_point(bottom_pt);
            if t >= 0.0 {
                self.insert(t, f64::from(u8::from(!flipped)), bottom_pt);
            }
            for index in 0..2 {
                let t = DLine::exact_point_v(line[index], top, bottom, x);
                if t >= 0.0 {
                    self.insert(index as f64, if flipped { 1.0 - t } else { t }, line[index]);
                }
            }
        }
        let result = vertical_coincident(line, x);
        if result == 1 && self.used == 0 {
            self.t[0][0] = Self::vertical_intercept_line(line, x);
            let y_intercept = line[0].y + self.t[0][0] * (line[1].y - line[0].y);
            if between(top, y_intercept, bottom) {
                self.t[1][0] = (y_intercept - top) / (bottom - top);
                if flipped {
                    for index in 0..result as usize {
                        self.t[1][index] = 1.0 - self.t[1][index];
                    }
                }
                self.pt[0].x = x;
                self.pt[0].y = y_intercept;
                self.used = 1;
            }
        }
        if self.allow_near || result == 2 {
            let t = line.near_point(top_pt, None);
            if t >= 0.0 {
                self.insert(t, flipped_f, top_pt);
            }
            if top != bottom {
                let bottom_pt = DPoint::new(x, bottom);
                let t = line.near_point(bottom_pt, None);
                if t >= 0.0 {
                    self.insert(t, f64::from(u8::from(!flipped)), bottom_pt);
                }
                for index in 0..2 {
                    let t = DLine::near_point_v(line[index], top, bottom, x);
                    if t >= 0.0 {
                        self.insert(index as f64, if flipped { 1.0 - t } else { t }, line[index]);
                    }
                }
            }
        }
        self.clean_up_parallel_lines(result == 2);
        debug_assert!(self.used <= 2);
        usize::from(self.used)
    }
}
