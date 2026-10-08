// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkLineParameters.h

//! The implicit line `a*x + b*y + c = 0` through a curve's end points (`SkLineParameters`).

use crate::cubic::DCubic;
use crate::line::DLine;
use crate::point::DPoint;
use crate::quad::DQuad;
use crate::types::{approximately_zero, not_almost_equal_ulps};

/// `SkLineParameters`.
// Port of: src/pathops/SkLineParameters.h (chrome/m156)
#[doc(alias = "SkLineParameters")]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct LineParameters {
    a: f64,
    b: f64,
    c: f64,
}

impl LineParameters {
    /// `bool cubicEndPoints(const SkDCubic& pts)`: picks the end points that give a usable line,
    /// returning `true` when the line is not degenerate.
    // Port of: src/pathops/SkLineParameters.h (chrome/m156)
    #[doc(alias = "cubicEndPoints")]
    #[allow(clippy::manual_assert_eq)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn cubic_end_points_full(&mut self, pts: &DCubic) -> bool {
        let mut end_index = 1;
        self.cubic_end_points(pts, 0, end_index);
        if self.dy() != 0.0 {
            return true;
        }
        if self.dx() == 0.0 {
            end_index += 1;
            self.cubic_end_points(pts, 0, end_index);
            debug_assert!(end_index == 2);
            if self.dy() != 0.0 {
                return true;
            }
            if self.dx() == 0.0 {
                end_index += 1;
                self.cubic_end_points(pts, 0, end_index); // line
                debug_assert!(end_index == 3);
                return false;
            }
        }
        if self.dx() < 0.0 {
            // only worry about y bias when breaking cw/ccw tie
            return true;
        }
        end_index += 1;
        if not_almost_equal_ulps(pts.pts[0].y, pts.pts[end_index].y) {
            if pts.pts[0].y > pts.pts[end_index].y {
                self.a = f64::EPSILON; // push it from 0 to slightly negative (y() returns -a)
            }
            return true;
        }
        if end_index == 3 {
            return true;
        }
        debug_assert!(end_index == 2);
        if pts.pts[0].y > pts.pts[3].y {
            self.a = f64::EPSILON; // push it from 0 to slightly negative (y() returns -a)
        }
        true
    }

    /// `void cubicEndPoints(const SkDCubic& pts, int s, int e)`.
    // Port of: src/pathops/SkLineParameters.h#L31-L69 (chrome/m156)
    #[doc(alias = "cubicEndPoints")]
    pub fn cubic_end_points(&mut self, pts: &DCubic, s: usize, e: usize) {
        self.a = pts[s].y - pts[e].y;
        self.b = pts[e].x - pts[s].x;
        self.c = pts[s].x * pts[e].y - pts[e].x * pts[s].y;
    }

    /// `double cubicPart(const SkDCubic& part)`.
    // Port of: src/pathops/SkLineParameters.h#L77-L83 (chrome/m156)
    pub fn cubic_part(&mut self, part: &DCubic) -> f64 {
        self.cubic_end_points_full(part);
        if part[0] == part[1] || DLine::new([part[0], part[1]]).near_ray(part[2]) {
            return self.point_distance(part[3]);
        }
        self.point_distance(part[2])
    }

    /// `void lineEndPoints(const SkDLine& pts)`.
    // Port of: src/pathops/SkLineParameters.h#L85-L89 (chrome/m156)
    #[doc(alias = "lineEndPoints")]
    pub fn line_end_points(&mut self, pts: &DLine) {
        self.a = pts[0].y - pts[1].y;
        self.b = pts[1].x - pts[0].x;
        self.c = pts[0].x * pts[1].y - pts[1].x * pts[0].y;
    }

    /// `bool quadEndPoints(const SkDQuad& pts)`.
    // Port of: src/pathops/SkLineParameters.h (chrome/m156)
    #[doc(alias = "quadEndPoints")]
    pub fn quad_end_points_full(&mut self, pts: &DQuad) -> bool {
        self.quad_end_points(pts, 0, 1);
        if self.dy() != 0.0 {
            return true;
        }
        if self.dx() == 0.0 {
            self.quad_end_points(pts, 0, 2);
            return false;
        }
        if self.dx() < 0.0 {
            // only worry about y bias when breaking cw/ccw tie
            return true;
        }
        if pts[0].y > pts[2].y {
            self.a = f64::EPSILON;
        }
        true
    }

    /// `void quadEndPoints(const SkDQuad& pts, int s, int e)`.
    // Port of: src/pathops/SkLineParameters.h#L91-L108 (chrome/m156)
    #[doc(alias = "quadEndPoints")]
    pub fn quad_end_points(&mut self, pts: &DQuad, s: usize, e: usize) {
        self.a = pts[s].y - pts[e].y;
        self.b = pts[e].x - pts[s].x;
        self.c = pts[s].x * pts[e].y - pts[e].x * pts[s].y;
    }

    /// `double quadPart(const SkDQuad& part)`.
    // Port of: src/pathops/SkLineParameters.h#L116-L119 (chrome/m156)
    pub fn quad_part(&mut self, part: &DQuad) -> f64 {
        self.quad_end_points_full(part);
        self.point_distance(part[2])
    }

    /// `double normalSquared() const`.
    #[must_use]
    pub fn normal_squared(&self) -> f64 {
        self.a * self.a + self.b * self.b
    }

    /// `bool normalize()`.
    // Port of: src/pathops/SkLineParameters.h#L125-L136 (chrome/m156)
    pub fn normalize(&mut self) -> bool {
        let normal = self.normal_squared().sqrt();
        if approximately_zero(normal) {
            self.a = 0.0;
            self.b = 0.0;
            self.c = 0.0;
            return false;
        }
        let reciprocal = 1.0 / normal;
        self.a *= reciprocal;
        self.b *= reciprocal;
        self.c *= reciprocal;
        true
    }

    /// `void cubicDistanceY(const SkDCubic& pts, SkDCubic& distance) const`.
    // Port of: src/pathops/SkLineParameters.h#L138-L144 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn cubic_distance_y(&self, pts: &DCubic, distance: &mut DCubic) {
        let one_third = 1.0 / 3.0;
        for index in 0..4 {
            distance[index].x = index as f64 * one_third;
            distance[index].y = self.a * pts[index].x + self.b * pts[index].y + self.c;
        }
    }

    /// `void quadDistanceY(const SkDQuad& pts, SkDQuad& distance) const`.
    // Port of: src/pathops/SkLineParameters.h#L146-L152 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn quad_distance_y(&self, pts: &DQuad, distance: &mut DQuad) {
        let one_half = 1.0 / 2.0;
        for index in 0..3 {
            distance[index].x = index as f64 * one_half;
            distance[index].y = self.a * pts[index].x + self.b * pts[index].y + self.c;
        }
    }

    /// `double controlPtDistance(const SkDCubic& pts, int index) const`.
    // Port of: src/pathops/SkLineParameters.h (chrome/m156)
    #[must_use]
    pub fn control_pt_distance_cubic(&self, pts: &DCubic, index: usize) -> f64 {
        debug_assert!(index == 1 || index == 2);
        self.a * pts[index].x + self.b * pts[index].y + self.c
    }

    /// `double controlPtDistance(const SkDQuad& pts) const`.
    // Port of: src/pathops/SkLineParameters.h (chrome/m156)
    #[must_use]
    pub fn control_pt_distance_quad(&self, pts: &DQuad) -> f64 {
        self.a * pts[1].x + self.b * pts[1].y + self.c
    }

    /// `double pointDistance(const SkDPoint& pt) const`.
    // Port of: src/pathops/SkLineParameters.h#L163-L165 (chrome/m156)
    #[must_use]
    pub fn point_distance(&self, pt: DPoint) -> f64 {
        self.a * pt.x + self.b * pt.y + self.c
    }

    /// `double dx() const`.
    #[must_use]
    pub fn dx(&self) -> f64 {
        self.b
    }

    /// `double dy() const`.
    #[must_use]
    pub fn dy(&self) -> f64 {
        -self.a
    }
}
