// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsRect.h, src/pathops/SkPathOpsRect.cpp,
// src/pathops/SkPathOpsBounds.h

//! Double-precision rectangles (`SkDRect`) and single-precision `PathOps` bounds (`SkPathOpsBounds`).

use skia_rust_core::point::Point;
use skia_rust_core::scalar::double_to_scalar;

use crate::conic::DConic;
use crate::cubic::DCubic;
use crate::point::DPoint;
use crate::quad::DQuad;
use crate::types::{almost_less_or_equal_ulps, approximately_between, std_max, std_min};

/// `SkDRect`: a rectangle with double-precision edges.
// Port of: src/pathops/SkPathOpsRect.h (chrome/m156)
#[doc(alias = "SkDRect")]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct DRect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl DRect {
    /// `void add(const SkDPoint& pt)`.
    pub fn add(&mut self, pt: DPoint) {
        self.left = std_min(self.left, pt.x);
        self.top = std_min(self.top, pt.y);
        self.right = std_max(self.right, pt.x);
        self.bottom = std_max(self.bottom, pt.y);
    }

    /// `bool contains(const SkDPoint& pt) const`.
    #[must_use]
    pub fn contains(&self, pt: DPoint) -> bool {
        approximately_between(self.left, pt.x, self.right)
            && approximately_between(self.top, pt.y, self.bottom)
    }

    /// `bool intersects(const SkDRect& r) const`.
    #[must_use]
    pub fn intersects(&self, r: &Self) -> bool {
        debug_assert!(self.left <= self.right);
        debug_assert!(self.top <= self.bottom);
        debug_assert!(r.left <= r.right);
        debug_assert!(r.top <= r.bottom);
        r.left <= self.right && self.left <= r.right && r.top <= self.bottom && self.top <= r.bottom
    }

    /// `void set(const SkDPoint& pt)`.
    pub fn set(&mut self, pt: DPoint) {
        self.left = pt.x;
        self.right = pt.x;
        self.top = pt.y;
        self.bottom = pt.y;
    }

    /// `double width() const`.
    #[must_use]
    pub fn width(&self) -> f64 {
        self.right - self.left
    }

    /// `double height() const`.
    #[must_use]
    pub fn height(&self) -> f64 {
        self.bottom - self.top
    }

    /// `void setBounds(const SkDConic& curve)`.
    pub fn set_bounds_conic(&mut self, curve: &DConic) {
        self.set_bounds_conic_sub(curve, curve, 0.0, 1.0);
    }

    /// `void setBounds(const SkDConic& curve, const SkDConic& sub, double tStart, double tEnd)`.
    // Port of: src/pathops/SkPathOpsRect.cpp (chrome/m156)
    pub fn set_bounds_conic_sub(&mut self, curve: &DConic, sub: &DConic, start_t: f64, end_t: f64) {
        self.set(sub.pts[0]);
        self.add(sub.pts[2]);
        let mut t_values = [0.0; 2];
        let mut roots = 0;
        if !sub.monotonic_in_x() {
            let mut t = [0.0; 1];
            let xs = [sub.pts[0].x, sub.pts[1].x, sub.pts[2].x];
            if DConic::find_extrema(xs, sub.weight, &mut t) == 1 {
                t_values[roots] = t[0];
                roots += 1;
            }
        }
        if !sub.monotonic_in_y() {
            let mut t = [0.0; 1];
            let ys = [sub.pts[0].y, sub.pts[1].y, sub.pts[2].y];
            if DConic::find_extrema(ys, sub.weight, &mut t) == 1 {
                t_values[roots] = t[0];
                roots += 1;
            }
        }
        for &t_value in &t_values[..roots] {
            let t = start_t + (end_t - start_t) * t_value;
            self.add(curve.pt_at_t(t));
        }
    }

    /// `void setBounds(const SkDCubic& curve)`.
    pub fn set_bounds_cubic(&mut self, curve: &DCubic) {
        self.set_bounds_cubic_sub(curve, curve, 0.0, 1.0);
    }

    /// `void setBounds(const SkDCubic& curve, const SkDCubic& sub, double tStart, double tEnd)`.
    // Port of: src/pathops/SkPathOpsRect.cpp (chrome/m156)
    pub fn set_bounds_cubic_sub(&mut self, curve: &DCubic, sub: &DCubic, start_t: f64, end_t: f64) {
        self.set(sub.pts[0]);
        self.add(sub.pts[3]);
        let mut t_values = [0.0; 4];
        let mut roots = 0;
        if !sub.monotonic_in_x() {
            let mut t = [0.0; 2];
            let n = DCubic::find_extrema(sub.xs(), &mut t);
            t_values[..n].copy_from_slice(&t[..n]);
            roots = n;
        }
        if !sub.monotonic_in_y() {
            let mut t = [0.0; 2];
            let n = DCubic::find_extrema(sub.ys(), &mut t);
            t_values[roots..roots + n].copy_from_slice(&t[..n]);
            roots += n;
        }
        for &t_value in &t_values[..roots] {
            let t = start_t + (end_t - start_t) * t_value;
            self.add(curve.pt_at_t(t));
        }
    }

    /// `void setBounds(const SkDQuad& curve)`.
    pub fn set_bounds_quad(&mut self, curve: &DQuad) {
        self.set_bounds_quad_sub(curve, curve, 0.0, 1.0);
    }

    /// `void setBounds(const SkDQuad& curve, const SkDQuad& sub, double tStart, double tEnd)`.
    // Port of: src/pathops/SkPathOpsRect.cpp (chrome/m156)
    pub fn set_bounds_quad_sub(&mut self, curve: &DQuad, sub: &DQuad, start_t: f64, end_t: f64) {
        self.set(sub.pts[0]);
        self.add(sub.pts[2]);
        let mut t_values = [0.0; 2];
        let mut roots = 0;
        if !sub.monotonic_in_x() {
            let mut t = [0.0; 1];
            let xs = [sub.pts[0].x, sub.pts[1].x, sub.pts[2].x];
            roots = DQuad::find_extrema(xs, &mut t);
            t_values[0] = t[0];
        }
        if !sub.monotonic_in_y() {
            let mut t = [0.0; 1];
            let ys = [sub.pts[0].y, sub.pts[1].y, sub.pts[2].y];
            let n = DQuad::find_extrema(ys, &mut t);
            t_values[roots..roots + n].copy_from_slice(&t[..n]);
            roots += n;
        }
        for &t_value in &t_values[..roots] {
            let t = start_t + (end_t - start_t) * t_value;
            self.add(curve.pt_at_t(t));
        }
    }

    /// `void setBounds(const SkTCurve& curve)`: the bounds of a curve of any kind.
    pub fn set_bounds_tcurve(&mut self, curve: &crate::t_curve::TCurve) {
        curve.set_bounds(self);
    }

    /// `bool valid() const`.
    #[must_use]
    pub fn valid(&self) -> bool {
        self.left <= self.right && self.top <= self.bottom
    }
}

/// `SkPathOpsBounds`: a single-precision bounding box used by the op graph.
// Port of: src/pathops/SkPathOpsBounds.h (chrome/m156)
#[doc(alias = "SkPathOpsBounds")]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Bounds {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Bounds {
    /// `static bool Intersects(const SkPathOpsBounds& a, const SkPathOpsBounds& b)`.
    #[doc(alias = "Intersects")]
    #[must_use]
    pub fn intersects(a: &Self, b: &Self) -> bool {
        almost_less_or_equal_ulps(a.left, b.right)
            && almost_less_or_equal_ulps(b.left, a.right)
            && almost_less_or_equal_ulps(a.top, b.bottom)
            && almost_less_or_equal_ulps(b.top, a.bottom)
    }

    /// `void add(SkScalar left, SkScalar top, SkScalar right, SkScalar bottom)`.
    pub fn add_edges(&mut self, left: f32, top: f32, right: f32, bottom: f32) {
        if left < self.left {
            self.left = left;
        }
        if top < self.top {
            self.top = top;
        }
        if right > self.right {
            self.right = right;
        }
        if bottom > self.bottom {
            self.bottom = bottom;
        }
    }

    /// `void add(const SkPathOpsBounds& toAdd)`.
    pub fn add_bounds(&mut self, to_add: &Self) {
        self.add_edges(to_add.left, to_add.top, to_add.right, to_add.bottom);
    }

    /// `void add(const SkPoint& pt)`.
    pub fn add_point(&mut self, pt: Point) {
        if pt.x < self.left {
            self.left = pt.x;
        }
        if pt.y < self.top {
            self.top = pt.y;
        }
        if pt.x > self.right {
            self.right = pt.x;
        }
        if pt.y > self.bottom {
            self.bottom = pt.y;
        }
    }

    /// `void add(const SkDPoint& pt)`.
    pub fn add_dpoint(&mut self, pt: DPoint) {
        if pt.x < f64::from(self.left) {
            self.left = double_to_scalar(pt.x);
        }
        if pt.y < f64::from(self.top) {
            self.top = double_to_scalar(pt.y);
        }
        if pt.x > f64::from(self.right) {
            self.right = double_to_scalar(pt.x);
        }
        if pt.y > f64::from(self.bottom) {
            self.bottom = double_to_scalar(pt.y);
        }
    }

    /// `bool almostContains(const SkPoint& pt) const`.
    #[must_use]
    pub fn almost_contains(&self, pt: Point) -> bool {
        almost_less_or_equal_ulps(self.left, pt.x)
            && almost_less_or_equal_ulps(pt.x, self.right)
            && almost_less_or_equal_ulps(self.top, pt.y)
            && almost_less_or_equal_ulps(pt.y, self.bottom)
    }

    /// `bool contains(const SkPoint& pt) const`.
    #[must_use]
    pub fn contains(&self, pt: Point) -> bool {
        self.left <= pt.x && self.top <= pt.y && self.right >= pt.x && self.bottom >= pt.y
    }
}
