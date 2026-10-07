// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkPathMakers.h

//! Point iterators for building rect, oval and rrect contours (`SkPathMakers.h`).

use crate::path_types::PathDirection;
use crate::point::Point;
use crate::rect::Rect;
use crate::rrect::{Corner, RRect};

/// Walks `N` points in a direction, starting at an index.
// Port of: src/core/SkPathMakers.h#L16-L41 (chrome/m156)
#[doc(alias = "SkPath_PointIterator")]
#[derive(Clone, Debug)]
pub struct PointIterator<const N: usize> {
    pts: [Point; N],
    current: usize,
    advance: usize,
}

impl<const N: usize> PointIterator<N> {
    fn new(pts: [Point; N], dir: PathDirection, start_index: u32) -> Self {
        Self {
            pts,
            current: start_index as usize % N,
            advance: if dir == PathDirection::CW { 1 } else { N - 1 },
        }
    }

    #[must_use]
    pub fn current(&self) -> Point {
        debug_assert!(self.current < N);
        self.pts[self.current]
    }

    #[allow(clippy::should_implement_trait)] // mirrors the C++ method name
    pub fn next(&mut self) -> Point {
        self.current = (self.current + self.advance) % N;
        self.current()
    }
}

/// `SkPath_RectPointIterator`.
pub type RectPointIterator = PointIterator<4>;
/// `SkPath_OvalPointIterator`.
pub type OvalPointIterator = PointIterator<4>;
/// `SkPath_RRectPointIterator`.
pub type RRectPointIterator = PointIterator<8>;

/// The corners of `rect`, starting at `start_index`.
// Port of: src/core/SkPathMakers.h#L43-L53 (chrome/m156)
#[doc(alias = "SkPath_RectPointIterator")]
#[must_use]
pub fn rect_point_iterator(rect: &Rect, dir: PathDirection, start_index: u32) -> RectPointIterator {
    PointIterator::new(
        [
            Point::new(rect.left, rect.top),
            Point::new(rect.right, rect.top),
            Point::new(rect.right, rect.bottom),
            Point::new(rect.left, rect.bottom),
        ],
        dir,
        start_index,
    )
}

/// The four extreme points of the oval in `oval`, starting at `start_index`.
// Port of: src/core/SkPathMakers.h#L55-L67 (chrome/m156)
#[doc(alias = "SkPath_OvalPointIterator")]
#[must_use]
pub fn oval_point_iterator(oval: &Rect, dir: PathDirection, start_index: u32) -> OvalPointIterator {
    let cx = oval.center_x();
    let cy = oval.center_y();
    PointIterator::new(
        [
            Point::new(cx, oval.top),
            Point::new(oval.right, cy),
            Point::new(cx, oval.bottom),
            Point::new(oval.left, cy),
        ],
        dir,
        start_index,
    )
}

/// The eight tangent points of `rrect`, starting at `start_index`.
// Port of: src/core/SkPathMakers.h#L69-L88 (chrome/m156)
#[doc(alias = "SkPath_RRectPointIterator")]
#[must_use]
pub fn rrect_point_iterator(
    rrect: &RRect,
    dir: PathDirection,
    start_index: u32,
) -> RRectPointIterator {
    let bounds = rrect.rect();
    let l = bounds.left;
    let t = bounds.top;
    let r = bounds.right;
    let b = bounds.bottom;
    PointIterator::new(
        [
            Point::new(l + rrect.radii(Corner::UpperLeft).x, t),
            Point::new(r - rrect.radii(Corner::UpperRight).x, t),
            Point::new(r, t + rrect.radii(Corner::UpperRight).y),
            Point::new(r, b - rrect.radii(Corner::LowerRight).y),
            Point::new(r - rrect.radii(Corner::LowerRight).x, b),
            Point::new(l + rrect.radii(Corner::LowerLeft).x, b),
            Point::new(l, b - rrect.radii(Corner::LowerLeft).y),
            Point::new(l, t + rrect.radii(Corner::UpperLeft).y),
        ],
        dir,
        start_index,
    )
}
