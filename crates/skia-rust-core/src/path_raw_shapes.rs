// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkPathRawShapes.h, src/core/SkPathRawShapes.cpp

//! Stack-allocated path data for known geometries (`SkPathRawShapes.h`).
//!
//! These types provide their own storage for path data, making them efficient alternatives to
//! `Path` for known geometries, avoiding heap allocations. The defaults were chosen to match those
//! in `PathBuilder`.
//!
//! skia-rust: in C++ each shape derives from `SkPathRaw`; here each shape owns its points and
//! [`Shape::raw`] returns the [`PathRaw`] view.

use crate::path_enums::{PathConvexity, direction_to_convexity};
use crate::path_makers::{oval_point_iterator, rect_point_iterator, rrect_point_iterator};
use crate::path_raw::PathRaw;
use crate::path_types::{PathDirection, PathFillType, PathSegmentMask, PathVerb};
use crate::point::Point;
use crate::rect::Rect as SkRect;
use crate::rrect::RRect as SkRRect;
use crate::scalar::SCALAR_ROOT_2_OVER_2;

const DEF_FILL_TYPE: PathFillType = PathFillType::Winding;

const RECT_VERBS: [PathVerb; 5] = [
    PathVerb::Move,
    PathVerb::Line,
    PathVerb::Line,
    PathVerb::Line,
    PathVerb::Close,
];
#[allow(clippy::cast_possible_truncation)] // the mask fits in 4 bits
const RECT_SEG_MASK: u8 = PathSegmentMask::LINE.bits() as u8;

const OVAL_VERBS: [PathVerb; 6] = [
    PathVerb::Move,
    PathVerb::Conic,
    PathVerb::Conic,
    PathVerb::Conic,
    PathVerb::Conic,
    PathVerb::Close,
];
#[allow(clippy::cast_possible_truncation)] // the mask fits in 4 bits
const OVAL_SEG_MASK: u8 = PathSegmentMask::CONIC.bits() as u8;

const FOUR_QUARTER_CIRCLE_CONICS: [f32; 4] = [
    SCALAR_ROOT_2_OVER_2,
    SCALAR_ROOT_2_OVER_2,
    SCALAR_ROOT_2_OVER_2,
    SCALAR_ROOT_2_OVER_2,
];

const RRECT_VERBS_LINE_START: [PathVerb; 10] = [
    PathVerb::Move,
    PathVerb::Line,
    PathVerb::Conic,
    PathVerb::Line,
    PathVerb::Conic,
    PathVerb::Line,
    PathVerb::Conic,
    PathVerb::Line,
    PathVerb::Conic,
    PathVerb::Close,
];

const RRECT_VERBS_CONIC_START: [PathVerb; 9] = [
    PathVerb::Move,
    PathVerb::Conic,
    PathVerb::Line,
    PathVerb::Conic,
    PathVerb::Line,
    PathVerb::Conic,
    PathVerb::Line,
    PathVerb::Conic, // we can skip the last line
    PathVerb::Close,
];

#[allow(clippy::cast_possible_truncation)] // the mask fits in 4 bits
const RRECT_SEG_MASK: u8 = (PathSegmentMask::LINE.bits() | PathSegmentMask::CONIC.bits()) as u8;

/// A shape with storage for up to `N` points.
#[derive(Clone, Debug)]
pub struct Shape<const N: usize> {
    /// The point storage; only the first `npts` points are part of the shape.
    pub storage: [Point; N],
    npts: usize,
    verbs: &'static [PathVerb],
    conics: &'static [f32],
    bounds: SkRect,
    convexity: PathConvexity,
    segment_mask: u8,
}

impl<const N: usize> Shape<N> {
    fn blank() -> Self {
        Self {
            storage: [Point::default(); N],
            npts: 0,
            verbs: &[],
            conics: &[],
            bounds: SkRect::new_empty(),
            convexity: PathConvexity::Unknown,
            segment_mask: 0,
        }
    }

    /// The path view of this shape.
    #[must_use]
    pub fn raw(&self) -> PathRaw<'_> {
        PathRaw {
            points: &self.storage[..self.npts],
            verbs: self.verbs,
            conics: self.conics,
            bounds: self.bounds,
            fill_type: DEF_FILL_TYPE,
            convexity: self.convexity,
            segment_mask: self.segment_mask,
        }
    }

    /// The points in use (`fPoints`).
    #[must_use]
    pub fn points(&self) -> &[Point] {
        &self.storage[..self.npts]
    }

    /// The points in use, mutably (C++ writes through `fStorage`).
    pub fn points_mut(&mut self) -> &mut [Point] {
        &mut self.storage[..self.npts]
    }

    // Port of: src/core/SkPathRawShapes.cpp#L27-L46 (chrome/m156)
    fn set_as_rect(&mut self, r: &SkRect, dir: PathDirection, index: u32) {
        debug_assert!(N >= 4);
        self.npts = 4;
        self.verbs = &RECT_VERBS;
        self.conics = &[];
        self.bounds = *r;
        self.convexity = direction_to_convexity(dir);
        self.segment_mask = RECT_SEG_MASK;

        let mut iter = rect_point_iterator(r, dir, index);
        self.storage[0] = iter.current();
        self.storage[1] = iter.next();
        self.storage[2] = iter.next();
        self.storage[3] = iter.next();
    }

    // Port of: src/core/SkPathRawShapes.cpp#L66-L86 (chrome/m156)
    fn set_as_oval(&mut self, r: &SkRect, dir: PathDirection, index: u32) {
        debug_assert!(N >= 9);
        self.npts = 9;
        self.verbs = &OVAL_VERBS;
        self.conics = &FOUR_QUARTER_CIRCLE_CONICS;
        self.bounds = *r;
        self.convexity = direction_to_convexity(dir);
        self.segment_mask = OVAL_SEG_MASK;

        let mut oval_iter = oval_point_iterator(r, dir, index);
        let mut rect_iter =
            rect_point_iterator(r, dir, index + u32::from(dir != PathDirection::CW));

        self.storage[0] = oval_iter.current();
        for i in 0..4 {
            self.storage[i * 2 + 1] = rect_iter.next();
            self.storage[i * 2 + 2] = oval_iter.next();
        }
    }

    // Port of: src/core/SkPathRawShapes.cpp#L112-L163 (chrome/m156)
    fn set_as_rrect(&mut self, rrect: &SkRRect, dir: PathDirection, index: u32) {
        // we start with a conic on odd indices when moving CW vs. even indices when moving CCW
        let starts_with_conic = (index & 1 == 1) == (dir == PathDirection::CW);
        // if we start with a conic, we end with a line, which we can skip (relying on close())
        let npoints = 13 - usize::from(starts_with_conic);

        let bounds = *rrect.rect();

        debug_assert!(N >= npoints);
        self.npts = npoints;
        if starts_with_conic {
            self.verbs = &RRECT_VERBS_CONIC_START;
        } else {
            self.verbs = &RRECT_VERBS_LINE_START;
        }
        self.conics = &FOUR_QUARTER_CIRCLE_CONICS;
        self.bounds = bounds;
        self.convexity = direction_to_convexity(dir);
        self.segment_mask = RRECT_SEG_MASK;

        let mut rrect_iter = rrect_point_iterator(rrect, dir, index);
        // Corner iterator indices follow the collapsed radii model,
        // adjusted such that the start pt is "behind" the radii start pt.
        let rect_start_index = index / 2 + u32::from(dir != PathDirection::CW);
        let mut rect_iter = rect_point_iterator(&bounds, dir, rect_start_index);

        self.storage[0] = rrect_iter.current();
        if starts_with_conic {
            for i in 0..3 {
                // conic points
                self.storage[i * 3 + 1] = rect_iter.next();
                self.storage[i * 3 + 2] = rrect_iter.next();
                // line point
                self.storage[i * 3 + 3] = rrect_iter.next();
            }
            // last conic points
            self.storage[10] = rect_iter.next();
            self.storage[11] = rrect_iter.next();
            // the final line is accomplished by close()
        } else {
            for i in 0..4 {
                // line point
                self.storage[i * 3 + 1] = rrect_iter.next();
                // conic points
                self.storage[i * 3 + 2] = rect_iter.next();
                self.storage[i * 3 + 3] = rrect_iter.next();
            }
        }
        // close
    }
}

/// A rectangle: move + 3 lines (+ close).
#[doc(alias = "SkPathRawShapes::Rect")]
pub type Rect = Shape<4>;
/// An oval: move + 4 conics (+ close).
#[doc(alias = "SkPathRawShapes::Oval")]
pub type Oval = Shape<9>;
/// A round rectangle: worst case move + 4 conics + 4 lines (+ close).
#[doc(alias = "SkPathRawShapes::RRect")]
pub type RRect = Shape<13>;

impl Shape<4> {
    /// `SkPathRawShapes::Rect(r, dir = kCW, index = 0)`.
    // Port of: src/core/SkPathRawShapes.cpp#L167-L169 (chrome/m156)
    #[must_use]
    pub fn new(r: &SkRect, dir: PathDirection, index: u32) -> Self {
        let mut s = Self::blank();
        s.set_as_rect(r, dir, index);
        s
    }
}

impl Shape<9> {
    /// `SkPathRawShapes::Oval(r, dir = kCW, index = 1)`.
    // Port of: src/core/SkPathRawShapes.cpp#L171-L173 (chrome/m156)
    #[must_use]
    pub fn new(r: &SkRect, dir: PathDirection, index: u32) -> Self {
        let mut s = Self::blank();
        s.set_as_oval(r, dir, index);
        s
    }
}

impl Shape<13> {
    /// `SkPathRawShapes::RRect(rr, dir, index)`.
    // Port of: src/core/SkPathRawShapes.cpp#L175-L187 (chrome/m156)
    #[must_use]
    pub fn new(rrect: &SkRRect, dir: PathDirection, index: u32) -> Self {
        let mut s = Self::blank();
        let bounds = *rrect.rect();
        if rrect.is_rect() || rrect.is_empty() {
            // degenerate(rect) => radii points are collapsing
            s.set_as_rect(&bounds, dir, index.div_ceil(2));
        } else if rrect.is_oval() {
            // degenerate(oval) => line points are collapsing
            s.set_as_oval(&bounds, dir, index / 2);
        } else {
            s.set_as_rrect(rrect, dir, index);
        }
        s
    }

    /// `SkPathRawShapes::RRect(rr, dir)`: start index 6 (CW) or 7 (CCW).
    // Port of: src/core/SkPathRawShapes.h#L42-L43 (chrome/m156)
    #[must_use]
    pub fn with_dir(rrect: &SkRRect, dir: PathDirection) -> Self {
        Self::new(rrect, dir, if dir == PathDirection::CW { 6 } else { 7 })
    }

    /// `SkPathRawShapes::RRect(rr)`: clockwise, start index 6.
    // Port of: src/core/SkPathRawShapes.h#L44 (chrome/m156)
    #[must_use]
    pub fn from_rrect(rrect: &SkRRect) -> Self {
        Self::new(rrect, PathDirection::CW, 6)
    }
}

const TRIANGLE_VERBS: [PathVerb; 4] = [
    PathVerb::Move,
    PathVerb::Line,
    PathVerb::Line,
    PathVerb::Close,
];

// Port of: src/core/SkPathRawShapes.cpp#L198-L206 (chrome/m156)
fn tri_to_convexity(pts: &[Point]) -> PathConvexity {
    let u = pts[1] - pts[0];
    let v = pts[2] - pts[1];
    let cross = u.x * v.y - u.y * v.x;
    if cross > 0.0 {
        PathConvexity::ConvexCW
    } else if cross < 0.0 {
        PathConvexity::ConvexCCW
    } else {
        PathConvexity::ConvexDegenerate
    }
}

/// `SkPathRawShapes::Triangle(threePoints, bounds)`: a closed triangle over borrowed points.
// Port of: src/core/SkPathRawShapes.cpp#L208-L214 (chrome/m156)
#[doc(alias = "SkPathRawShapes::Triangle")]
#[must_use]
pub fn triangle<'a>(three_points: &'a [Point], bounds: &SkRect) -> PathRaw<'a> {
    debug_assert_eq!(three_points.len(), 3);
    #[allow(clippy::cast_possible_truncation)] // the mask fits in 4 bits
    let segment_mask = PathSegmentMask::LINE.bits() as u8;
    PathRaw {
        points: three_points,
        verbs: &TRIANGLE_VERBS,
        conics: &[],
        bounds: *bounds,
        fill_type: PathFillType::DEFAULT,
        convexity: tri_to_convexity(three_points),
        segment_mask,
    }
}
