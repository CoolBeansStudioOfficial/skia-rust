// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsTCurve.h (SkTCurve), src/pathops/SkPathOpsQuad.cpp
// (SkTQuad), src/pathops/SkPathOpsConic.cpp (SkTConic), src/pathops/SkPathOpsCubic.cpp (SkTCubic)

//! The curve interface of the T-intersection code (`SkTCurve` and its `SkTQuad`, `SkTConic`,
//! `SkTCubic` subclasses). Skia dispatches these virtually; here they are an enum.

use crate::conic::DConic;
use crate::cubic::DCubic;
use crate::intersections::Intersections;
use crate::line::DLine;
use crate::point::{DPoint, DVector};
use crate::quad::DQuad;
use crate::rect::DRect;

/// `SkTCurve`: a quad, conic or cubic in the T-intersection code.
// Port of: src/pathops/SkPathOpsTCurve.h#L19-L19 (chrome/m156)
#[doc(alias = "SkTCurve")]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TCurve {
    /// `SkTQuad`.
    Quad(DQuad),
    /// `SkTConic`.
    Conic(DConic),
    /// `SkTCubic`.
    Cubic(DCubic),
}

impl TCurve {
    /// `int pointCount() const`: the number of control points.
    #[must_use]
    #[allow(clippy::match_same_arms)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn point_count(&self) -> usize {
        match self {
            Self::Quad(_) => 3,
            Self::Conic(_) => 3,
            Self::Cubic(_) => 4,
        }
    }

    /// `int pointLast() const`.
    #[must_use]
    pub fn point_last(&self) -> usize {
        self.point_count() - 1
    }

    /// `bool IsConic() const`.
    #[must_use]
    pub fn is_conic(&self) -> bool {
        matches!(self, Self::Conic(_))
    }

    /// `int maxIntersections() const`.
    #[must_use]
    #[allow(clippy::match_same_arms)] // mirrors Skia's C++ arithmetic: exact float comparisons and C-style casts, kept as written
    pub fn max_intersections(&self) -> usize {
        match self {
            Self::Quad(_) => 4,
            Self::Conic(_) => 4,
            Self::Cubic(_) => 9,
        }
    }

    /// `operator[](int n) const`: control point `n`.
    #[must_use]
    pub fn point(&self, n: usize) -> DPoint {
        match self {
            Self::Quad(q) => q[n],
            Self::Conic(c) => c[n],
            Self::Cubic(c) => c[n],
        }
    }

    /// `bool collapsed() const`.
    #[must_use]
    pub fn collapsed(&self) -> bool {
        match self {
            Self::Quad(q) => q.collapsed(),
            Self::Conic(c) => c.collapsed(),
            Self::Cubic(c) => c.collapsed(),
        }
    }

    /// `bool controlsInside() const`.
    #[must_use]
    pub fn controls_inside(&self) -> bool {
        match self {
            Self::Quad(q) => q.controls_inside(),
            Self::Conic(c) => c.controls_inside(),
            Self::Cubic(c) => c.controls_inside(),
        }
    }

    /// `SkDVector dxdyAtT(double t) const`.
    #[must_use]
    pub fn dxdy_at_t(&self, t: f64) -> DVector {
        match self {
            Self::Quad(q) => q.dxdy_at_t(t),
            Self::Conic(c) => c.dxdy_at_t(t),
            Self::Cubic(c) => c.dxdy_at_t(t),
        }
    }

    /// `SkDPoint ptAtT(double t) const`.
    #[must_use]
    pub fn pt_at_t(&self, t: f64) -> DPoint {
        match self {
            Self::Quad(q) => q.pt_at_t(t),
            Self::Conic(c) => c.pt_at_t(t),
            Self::Cubic(c) => c.pt_at_t(t),
        }
    }

    /// `void otherPts(int oddMan, const SkDPoint* endPt[2]) const`.
    #[must_use]
    pub fn other_pts(&self, odd_man: usize) -> Vec<DPoint> {
        match self {
            Self::Quad(q) => q.other_pts(odd_man).to_vec(),
            Self::Conic(c) => c.other_pts(odd_man).to_vec(),
            Self::Cubic(c) => c.other_pts(odd_man).to_vec(),
        }
    }

    /// `void setBounds(SkDRect* rect) const`.
    // Port of: src/pathops/SkPathOpsRect.cpp#L16-L31 (chrome/m156)
    pub fn set_bounds(&self, rect: &mut DRect) {
        match self {
            Self::Quad(q) => rect.set_bounds_quad(q),
            Self::Conic(c) => rect.set_bounds_conic(c),
            Self::Cubic(c) => rect.set_bounds_cubic(c),
        }
    }

    /// `void subDivide(double t1, double t2, SkTCurve* curve) const`: the sub-curve of the same kind.
    #[must_use]
    pub fn sub_divide(&self, t1: f64, t2: f64) -> Self {
        match self {
            Self::Quad(q) => Self::Quad(q.sub_divide(t1, t2)),
            Self::Conic(c) => Self::Conic(c.sub_divide(t1, t2)),
            Self::Cubic(c) => Self::Cubic(c.sub_divide(t1, t2)),
        }
    }

    /// `int intersectRay(SkIntersections* i, const SkDLine& line) const`.
    // Port of: src/pathops/SkPathOpsQuad.cpp#L317-L317 (chrome/m156)
    pub fn intersect_ray(&self, i: &mut Intersections, line: &DLine) -> usize {
        match self {
            Self::Quad(q) => i.intersect_ray_quad(q, line),
            Self::Conic(c) => i.intersect_ray_conic(c, line),
            Self::Cubic(c) => i.intersect_ray_cubic(c, line),
        }
    }

    /// `SkTQuad::hullIntersects(const SkDQuad& quad)` and its siblings: the hull test of this
    /// curve against a quad. The C++ dispatch passes the argument on to the other operand's
    /// `hullIntersects`, so the receiver differs by kind; mirrored exactly here.
    // Port of: src/pathops/SkPathOpsQuad.cpp (chrome/m156)
    #[must_use]
    fn hull_with_quad(&self, quad: &DQuad) -> Option<bool> {
        match self {
            Self::Quad(q) => quad.hull_intersects_quad(q),
            Self::Conic(c) => c.hull_intersects_quad(quad),
            Self::Cubic(c) => c.hull_intersects_quad(quad),
        }
    }

    /// `hullIntersects(const SkDConic& conic)` of this curve.
    // Port of: src/pathops/SkPathOpsQuad.cpp (chrome/m156)
    #[must_use]
    fn hull_with_conic(&self, conic: &DConic) -> Option<bool> {
        match self {
            Self::Quad(q) => conic.hull_intersects_quad(q),
            Self::Conic(c) => conic.hull_intersects_conic(c),
            Self::Cubic(c) => conic.hull_intersects_cubic(c),
        }
    }

    /// `hullIntersects(const SkDCubic& cubic)` of this curve.
    // Port of: src/pathops/SkPathOpsQuad.cpp (chrome/m156)
    #[must_use]
    fn hull_with_cubic(&self, cubic: &DCubic) -> Option<bool> {
        match self {
            Self::Quad(q) => cubic.hull_intersects_quad(q),
            Self::Conic(c) => cubic.hull_intersects_conic(c),
            Self::Cubic(c) => cubic.hull_intersects_cubic(c),
        }
    }

    /// `bool hullIntersects(const SkTCurve& curve, bool* isLinear) const` of `self`: passes
    /// `self`'s own control points to `other`'s `hullIntersects`, as the C++ does. `None` when the
    /// hulls do not intersect, else `Some(is_linear)`.
    // Port of: src/pathops/SkPathOpsQuad.cpp#L53-L89 (chrome/m156)
    #[must_use]
    pub fn hull_intersects(&self, other: &Self) -> Option<bool> {
        match self {
            Self::Quad(q) => other.hull_with_quad(q),
            Self::Conic(c) => other.hull_with_conic(c),
            Self::Cubic(c) => other.hull_with_cubic(c),
        }
    }
}
