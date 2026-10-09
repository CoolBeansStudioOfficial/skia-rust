// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/geom/EdgeAAQuad.h

//! `skgpu::graphite::EdgeAAQuad`: a convex quadrilateral with per-edge anti-aliasing flags.

use bitflags::bitflags;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect as SkRect;
use skia_rust_simd::vx::{self, Float2, Float4};

use crate::graphite::geom::rect::Rect;

bitflags! {
    /// `EdgeAAQuad::Flags`: which edges of the quad are anti-aliased. This is a typesafe
    /// equivalent to `SkCanvas::QuadAAFlags`.
    // Port of: src/gpu/graphite/geom/EdgeAAQuad.h#L29-L39 (chrome/m156)
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Flags: u8 {
        /// The left edge (p0-p3).
        const LEFT = 0b0001;
        /// The top edge (p1-p0).
        const TOP = 0b0010;
        /// The right edge (p2-p1).
        const RIGHT = 0b0100;
        /// The bottom edge (p3-p2).
        const BOTTOM = 0b1000;

        /// No edges are anti-aliased.
        const NONE = 0b0000;
        /// All edges are anti-aliased.
        const ALL = 0b1111;
    }
}

/// `EdgeAAQuad`: `(x, y)` coordinates for the four corners of a quadrilateral, assumed to be
/// convex and in a consistent winding (CW vs. CCW is fine). Locally, the vertices are ordered
/// "top-left", "top-right", "bottom-right", "bottom-left". The edges are in order left (p0-p3),
/// top (p1-p0), right (p2-p1), and bottom (p3-p2).
// Port of: src/gpu/graphite/geom/EdgeAAQuad.h#L23-L86 (chrome/m156)
#[doc(alias = "skgpu::graphite::EdgeAAQuad")]
#[derive(Clone, Copy, Debug)]
pub struct EdgeAAQuad {
    xs: Float4,
    ys: Float4,
    edge_flags: Flags,
    is_rect: bool,
}

impl EdgeAAQuad {
    /// `EdgeAAQuad(const SkRect&, edgeFlags)`: a rect, stored as four corners.
    // Port of: src/gpu/graphite/geom/EdgeAAQuad.h#L42-L46 (chrome/m156)
    #[must_use]
    pub fn from_sk_rect(rect: &SkRect, edge_flags: Flags) -> Self {
        Self {
            xs: Float4::new(rect.left, rect.right, rect.right, rect.left),
            ys: Float4::new(rect.top, rect.top, rect.bottom, rect.bottom),
            edge_flags,
            is_rect: true,
        }
    }

    /// `EdgeAAQuad(const Rect&, edgeFlags)`.
    // Port of: src/gpu/graphite/geom/EdgeAAQuad.h#L47-L51 (chrome/m156)
    #[must_use]
    pub fn from_rect(rect: Rect, edge_flags: Flags) -> Self {
        let ltrb = rect.ltrb();
        Self {
            xs: vx::shuffle(ltrb, [0, 2, 2, 0]),
            ys: vx::shuffle(ltrb, [1, 1, 3, 3]),
            edge_flags,
            is_rect: true,
        }
    }

    /// `EdgeAAQuad(const SkPoint points[4], edgeFlags)`: four arbitrary corners.
    // Port of: src/gpu/graphite/geom/EdgeAAQuad.h#L52-L56 (chrome/m156)
    #[must_use]
    pub fn from_points(points: &[Point; 4], edge_flags: Flags) -> Self {
        Self {
            xs: Float4::new(points[0].x, points[1].x, points[2].x, points[3].x),
            ys: Float4::new(points[0].y, points[1].y, points[2].y, points[3].y),
            edge_flags,
            is_rect: false,
        }
    }

    /// `EdgeAAQuad(const float4& xs, const float4& ys, edgeFlags)`.
    // Port of: src/gpu/graphite/geom/EdgeAAQuad.h#L57-L61 (chrome/m156)
    #[must_use]
    pub fn from_xs_ys(xs: Float4, ys: Float4, edge_flags: Flags) -> Self {
        Self {
            xs,
            ys,
            edge_flags,
            is_rect: false,
        }
    }

    /// `bounds()`: the bounding box of the quadrilateral (not counting any outsetting for
    /// anti-aliasing).
    // Port of: src/gpu/graphite/geom/EdgeAAQuad.h#L64-L77 (chrome/m156)
    #[must_use]
    pub fn bounds(&self) -> Rect {
        if self.is_rect {
            return Rect::from_corners(
                Float2::new(self.xs[0], self.ys[0]),
                Float2::new(self.xs[2], self.ys[2]),
            );
        }

        let p0p1 = Rect::ltrb_vals(vx::shuffle(
            Float4::from_xy_zw(self.xs.lo(), self.ys.lo()),
            [0, 2, 1, 3],
        ))
        .make_sorted();
        let p2p3 = Rect::ltrb_vals(vx::shuffle(
            Float4::from_xy_zw(self.xs.hi(), self.ys.hi()),
            [0, 2, 1, 3],
        ))
        .make_sorted();
        p0p1.make_join(p2p3)
    }

    /// `xs()`: the x coordinates of the four corners.
    #[must_use]
    pub fn xs(&self) -> Float4 {
        self.xs
    }

    /// `ys()`: the y coordinates of the four corners.
    #[must_use]
    pub fn ys(&self) -> Float4 {
        self.ys
    }

    /// `edgeFlags()`.
    #[must_use]
    pub fn edge_flags(&self) -> Flags {
        self.edge_flags
    }

    /// `isRect()`.
    #[must_use]
    pub fn is_rect(&self) -> bool {
        self.is_rect
    }
}
