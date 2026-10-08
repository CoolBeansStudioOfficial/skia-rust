// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsConic.h, src/pathops/SkPathOpsConic.cpp

//! Double-precision rational quadratic (conic) curves (`SkDConic`).

use skia_rust_core::point::Point;

use crate::cubic::DCubic;
use crate::point::{DPoint, DVector};
use crate::quad::DQuad;
use crate::types::zero_or_one;

/// `SkDConic`: a quadratic with a weight on the middle control point.
// Port of: src/pathops/SkPathOpsConic.h (chrome/m156)
#[doc(alias = "SkDConic")]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct DConic {
    pub pts: DQuad,
    /// `SkScalar fWeight`: a single precision weight, as in Skia.
    pub weight: f32,
}

/// `SkDConic::kPointCount`.
pub const CONIC_POINT_COUNT: usize = 3;
/// `SkDConic::kPointLast`.
pub const CONIC_POINT_LAST: usize = CONIC_POINT_COUNT - 1;
/// `SkDConic::kMaxIntersections`.
pub const CONIC_MAX_INTERSECTIONS: usize = 4;

/// Port of `conic_deriv_coeff`: `src` is one coordinate of the three control points.
// Port of: src/pathops/SkPathOpsConic.cpp#L22-L31 (chrome/m156)
fn conic_deriv_coeff(src: [f64; 3], w: f32, coeff: &mut [f64; 3]) {
    let p20 = src[2] - src[0];
    let p10 = src[1] - src[0];
    let w = f64::from(w);
    let w_p10 = w * p10;
    coeff[0] = w * p20 - p20;
    coeff[1] = p20 - 2.0 * w_p10;
    coeff[2] = w_p10;
}

/// Port of `conic_eval_tan`.
// Port of: src/pathops/SkPathOpsConic.cpp#L33-L37 (chrome/m156)
fn conic_eval_tan(coord: [f64; 3], w: f32, t: f64) -> f64 {
    let mut coeff = [0.0; 3];
    conic_deriv_coeff(coord, w, &mut coeff);
    t * (t * coeff[0] + coeff[1]) + coeff[2]
}

/// Port of `conic_eval_numerator`.
// Port of: src/pathops/SkPathOpsConic.cpp#L73-L81 (chrome/m156)
#[allow(clippy::many_single_char_names)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
fn conic_eval_numerator(src: [f64; 3], w: f32, t: f64) -> f64 {
    debug_assert!((0.0..=1.0).contains(&t));
    let src2w = src[1] * f64::from(w);
    let c = src[0];
    let a = src[2] - 2.0 * src2w + c;
    let b = 2.0 * (src2w - c);
    (a * t + b) * t + c
}

/// Port of `conic_eval_denominator`. The C++ computes `2 * (w - 1)` in `float`, so this does too.
// Port of: src/pathops/SkPathOpsConic.cpp#L84-L89 (chrome/m156)
#[allow(clippy::many_single_char_names)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
fn conic_eval_denominator(w: f32, t: f64) -> f64 {
    let b = f64::from(2.0_f32 * (w - 1.0_f32));
    let c = 1.0;
    let a = -b;
    (a * t + b) * t + c
}

impl DConic {
    /// `DConic` from its points and weight (the constructor-style `set`).
    #[must_use]
    pub const fn new(pts: DQuad, weight: f32) -> Self {
        Self { pts, weight }
    }

    /// `const SkDConic& set(const SkPoint pts[kPointCount], SkScalar weight)`.
    pub fn set(&mut self, pts: [Point; 3], weight: f32) -> &mut Self {
        self.pts.set(pts);
        self.weight = weight;
        self
    }

    /// `bool collapsed() const`.
    #[must_use]
    pub fn collapsed(&self) -> bool {
        self.pts.collapsed()
    }

    /// `bool controlsInside() const`.
    #[must_use]
    pub fn controls_inside(&self) -> bool {
        self.pts.controls_inside()
    }

    /// `SkDConic flip() const`.
    #[must_use]
    pub fn flip(&self) -> Self {
        Self::new(self.pts.flip(), self.weight)
    }

    /// `static bool IsConic()`.
    #[must_use]
    pub const fn is_conic() -> bool {
        true
    }

    /// `static int FindExtrema(const double src[], SkScalar weight, double tValue[1])`.
    // Port of: src/pathops/SkPathOpsConic.cpp#L39-L55 (chrome/m156)
    #[doc(alias = "FindExtrema")]
    #[must_use]
    pub fn find_extrema(src: [f64; 3], w: f32, t: &mut [f64; 1]) -> usize {
        let mut coeff = [0.0; 3];
        conic_deriv_coeff(src, w, &mut coeff);
        let mut t_values = [0.0; 2];
        let roots = DQuad::roots_valid_t(coeff[0], coeff[1], coeff[2], &mut t_values);
        if roots == 1 {
            t[0] = t_values[0];
            return 1;
        }
        0
    }

    /// `SkDVector dxdyAtT(double t) const`.
    // Port of: src/pathops/SkPathOpsConic.cpp#L57-L71 (chrome/m156)
    #[doc(alias = "dxdyAtT")]
    #[must_use]
    pub fn dxdy_at_t(&self, t: f64) -> DVector {
        let xs = self.x_coords();
        let ys = self.y_coords();
        let mut result = DVector::new(
            conic_eval_tan(xs, self.weight, t),
            conic_eval_tan(ys, self.weight, t),
        );
        if result.x == 0.0 && result.y == 0.0 && zero_or_one(t) {
            result = self.pts[2] - self.pts[0];
        }
        result
    }

    /// `SkDPoint ptAtT(double t) const`.
    // Port of: src/pathops/SkPathOpsConic.cpp#L95-L108 (chrome/m156)
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
        let denominator = conic_eval_denominator(self.weight, t);
        DPoint::new(
            conic_eval_numerator(self.x_coords(), self.weight, t) / denominator,
            conic_eval_numerator(self.y_coords(), self.weight, t) / denominator,
        )
    }

    /// `SkDConic subDivide(double t1, double t2) const`.
    // Port of: src/pathops/SkPathOpsConic.cpp#L131-L174 (chrome/m156)
    #[must_use]
    #[allow(clippy::float_cmp, clippy::manual_midpoint)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn sub_divide(&self, t1: f64, t2: f64) -> Self {
        let xs = self.x_coords();
        let ys = self.y_coords();
        let w = self.weight;
        let (ax, ay, az) = if t1 == 0.0 {
            (self.pts[0].x, self.pts[0].y, 1.0)
        } else if t1 != 1.0 {
            (
                conic_eval_numerator(xs, w, t1),
                conic_eval_numerator(ys, w, t1),
                conic_eval_denominator(w, t1),
            )
        } else {
            (self.pts[2].x, self.pts[2].y, 1.0)
        };
        let mid_t = (t1 + t2) / 2.0;
        let dx = conic_eval_numerator(xs, w, mid_t);
        let dy = conic_eval_numerator(ys, w, mid_t);
        let dz = conic_eval_denominator(w, mid_t);
        let (cx, cy, cz) = if t2 == 1.0 {
            (self.pts[2].x, self.pts[2].y, 1.0)
        } else if t2 != 0.0 {
            (
                conic_eval_numerator(xs, w, t2),
                conic_eval_numerator(ys, w, t2),
                conic_eval_denominator(w, t2),
            )
        } else {
            (self.pts[0].x, self.pts[0].y, 1.0)
        };
        let bx = 2.0 * dx - (ax + cx) / 2.0;
        let by = 2.0 * dy - (ay + cy) / 2.0;
        let mut bz = 2.0 * dz - (az + cz) / 2.0;
        if bz == 0.0 {
            // if bz is 0, weight is 0, control point has no effect: any value will do
            bz = 1.0;
        }
        let weight = skia_rust_core::scalar::double_to_scalar(bz / (az * cz).sqrt());
        Self::new(
            DQuad::new([
                DPoint::new(ax / az, ay / az),
                DPoint::new(bx / bz, by / bz),
                DPoint::new(cx / cz, cy / cz),
            ]),
            weight,
        )
    }

    /// `SkDPoint subDivide(const SkDPoint& a, const SkDPoint& c, double t1, double t2,
    /// SkScalar* weight) const`. `weight` is not written by Skia's version.
    // Port of: src/pathops/SkPathOpsConic.cpp (chrome/m156)
    #[must_use]
    pub fn sub_divide_points(&self, _a: DPoint, _c: DPoint, t1: f64, t2: f64) -> DPoint {
        let chopped = self.sub_divide(t1, t2);
        chopped.pts[1]
    }

    /// `hullIntersects(const SkDQuad& quad, bool* isLinear)`; `None` when the hulls do not
    /// intersect, else `Some(is_linear)`.
    #[must_use]
    pub fn hull_intersects_quad(&self, quad: &DQuad) -> Option<bool> {
        self.pts.hull_intersects_quad(quad)
    }

    /// `hullIntersects(const SkDConic& conic, bool* isLinear)`.
    #[must_use]
    pub fn hull_intersects_conic(&self, conic: &Self) -> Option<bool> {
        self.pts.hull_intersects_quad(&conic.pts)
    }

    /// `hullIntersects(const SkDCubic& cubic, bool* isLinear)`.
    #[must_use]
    pub fn hull_intersects_cubic(&self, cubic: &DCubic) -> Option<bool> {
        cubic.hull_intersects_conic(self)
    }

    /// `bool isLinear(int startIndex, int endIndex) const`.
    #[must_use]
    pub fn is_linear(&self, start_index: usize, end_index: usize) -> bool {
        self.pts.is_linear(start_index, end_index)
    }

    /// `bool monotonicInX() const`.
    #[must_use]
    pub fn monotonic_in_x(&self) -> bool {
        self.pts.monotonic_in_x()
    }

    /// `bool monotonicInY() const`.
    #[must_use]
    pub fn monotonic_in_y(&self) -> bool {
        self.pts.monotonic_in_y()
    }

    /// `void otherPts(int oddMan, const SkDPoint* endPt[2]) const`.
    #[must_use]
    pub fn other_pts(&self, odd_man: usize) -> [DPoint; 2] {
        self.pts.other_pts(odd_man)
    }

    /// `void align(int endIndex, SkDPoint* dstPt) const`.
    pub fn align(&self, end_index: usize, dst_pt: &mut DPoint) {
        self.pts.align(end_index, dst_pt);
    }

    fn x_coords(&self) -> [f64; 3] {
        [self.pts[0].x, self.pts[1].x, self.pts[2].x]
    }

    fn y_coords(&self) -> [f64; 3] {
        [self.pts[0].y, self.pts[1].y, self.pts[2].y]
    }
}

impl std::ops::Index<usize> for DConic {
    type Output = DPoint;
    fn index(&self, n: usize) -> &DPoint {
        &self.pts[n]
    }
}

impl std::ops::IndexMut<usize> for DConic {
    fn index_mut(&mut self, n: usize) -> &mut DPoint {
        &mut self.pts[n]
    }
}
