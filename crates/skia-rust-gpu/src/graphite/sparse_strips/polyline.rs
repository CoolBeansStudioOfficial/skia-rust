// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/sparse_strips/Polyline.h

//! [`Polyline`]: the flattened points of a path, with NaN sentinels between sub-paths.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)] // the C++ indexes with `int`

use skia_rust_core::point::Point;
use skia_rust_core::scalar::SCALAR_NAN;

use crate::graphite::sparse_strips::types::Line;

/// Helper class that uses the watertight property of flattened paths to deduplicate resulting
/// points. Since any two adjacent points imply a continuous line segment, NaN values are injected
/// into the underlying array as sentinels to signal the end of a sub-path. During tile creation,
/// the raw index into the `fPoints` array is stored on the tile, allowing the coverage generation
/// phase to retrieve the correct parent line independent of the deduplication.
// Port of: src/gpu/graphite/sparse_strips/Polyline.h#L27-L144 (chrome/m156)
#[doc(alias = "skgpu::graphite::Polyline")]
#[derive(Clone, Debug, Default)]
pub struct Polyline {
    points: Vec<Point>,
}

impl Polyline {
    /// `Polyline()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `reset()`.
    // Port of: src/gpu/graphite/sparse_strips/Polyline.h#L29-L31 (chrome/m156)
    pub fn reset(&mut self) {
        self.points.clear();
    }

    /// `reserve(count)`.
    // Port of: src/gpu/graphite/sparse_strips/Polyline.h#L33-L35 (chrome/m156)
    pub fn reserve(&mut self, count: usize) {
        self.points.reserve(count);
    }

    /// `appendPoint(pt)`.
    // Port of: src/gpu/graphite/sparse_strips/Polyline.h#L37-L42 (chrome/m156)
    #[doc(alias = "appendPoint")]
    pub fn append_point(&mut self, pt: Point) {
        debug_assert!(!pt.x.is_nan() && !pt.y.is_nan());
        if self.points.last().is_none_or(|last| *last != pt) {
            self.points.push(pt);
        }
    }

    /// `appendPoints(pts)`: bulk append to eliminate repetitive capacity checks and memory hits.
    // Port of: src/gpu/graphite/sparse_strips/Polyline.h#L45-L63 (chrome/m156)
    #[doc(alias = "appendPoints")]
    pub fn append_points(&mut self, pts: &[Point]) {
        if pts.is_empty() {
            return;
        }
        self.points.reserve(pts.len());

        // Cache the last point in a local register.
        // Note: If fPoints.back() is a NaN sentinel, pt != lastPt correctly evaluates to true.
        let mut last_pt = self
            .points
            .last()
            .copied()
            .unwrap_or(Point::new(SCALAR_NAN, SCALAR_NAN));
        for &pt in pts {
            debug_assert!(!pt.x.is_nan() && !pt.y.is_nan());
            if pt != last_pt {
                self.points.push(pt);
                last_pt = pt;
            }
        }
    }

    /// `appendSentinel()`.
    // Port of: src/gpu/graphite/sparse_strips/Polyline.h#L65-L69 (chrome/m156)
    #[doc(alias = "appendSentinel")]
    pub fn append_sentinel(&mut self) {
        if self.points.last().is_some_and(|last| !last.x.is_nan()) {
            self.points.push(Point::new(SCALAR_NAN, SCALAR_NAN));
        }
    }

    /// `getLine(index)`.
    // Port of: src/gpu/graphite/sparse_strips/Polyline.h#L71-L73 (chrome/m156)
    #[doc(alias = "getLine")]
    #[must_use]
    pub fn get_line(&self, index: u32) -> Line {
        Line {
            p0: self.points[index as usize],
            p1: self.points[index as usize + 1],
        }
    }

    /// `begin()`.
    // Port of: src/gpu/graphite/sparse_strips/Polyline.h#L129-L131 (chrome/m156)
    #[must_use]
    pub fn begin(&self) -> LineIterator<'_> {
        LineIterator::new(&self.points, 0, self.points.len() as i32)
    }

    /// `end()`.
    // Port of: src/gpu/graphite/sparse_strips/Polyline.h#L133-L135 (chrome/m156)
    #[must_use]
    pub fn end(&self) -> LineIterator<'_> {
        let count = self.points.len() as i32;
        LineIterator::new(&self.points, count, count)
    }

    /// The lines of the polyline with their raw indices, as `for (auto [line, index] : polyline)`.
    #[must_use]
    pub fn iter(&self) -> LineIterator<'_> {
        self.begin()
    }

    /// `empty()`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// `count()`.
    #[must_use]
    pub fn count(&self) -> usize {
        self.points.len()
    }

    /// `points()`.
    #[must_use]
    pub fn points(&self) -> &[Point] {
        &self.points
    }
}

impl<'a> IntoIterator for &'a Polyline {
    type Item = (Line, i32);
    type IntoIter = LineIterator<'a>;

    fn into_iter(self) -> LineIterator<'a> {
        self.begin()
    }
}

/// `Polyline::LineIterator`: iterates the lines between consecutive points, skipping the NaN
/// sentinels. Is both the C++ iterator ([`current`](Self::current), [`advance`](Self::advance), [`differs_from`](Self::differs_from)) and a Rust
/// [`Iterator`] over `(line, raw index)`.
// Port of: src/gpu/graphite/sparse_strips/Polyline.h#L75-L127 (chrome/m156)
#[derive(Clone, Debug)]
pub struct LineIterator<'a> {
    pts: &'a [Point],
    count: i32,
    index: i32,
}

impl<'a> LineIterator<'a> {
    /// `LineIterator(pts, index, count)`.
    // Port of: src/gpu/graphite/sparse_strips/Polyline.h#L77-L83 (chrome/m156)
    #[must_use]
    pub fn new(pts: &'a [Point], index: i32, count: i32) -> Self {
        let mut it = Self { pts, count, index };
        it.advance_to_next_valid_index();
        // Defensively call this to protect against malformed input. In ordinary usage, this
        // should never be required.
        it.advance_to_valid_line();
        it
    }

    /// `operator*`: the current line and its raw index.
    #[doc(alias = "operator*")]
    // Port of: src/gpu/graphite/sparse_strips/Polyline.h#L85-L88 (chrome/m156)
    #[must_use]
    pub fn current(&self) -> (Line, i32) {
        debug_assert!(self.index + 1 < self.count);
        let i = self.index as usize;
        (
            Line {
                p0: self.pts[i],
                p1: self.pts[i + 1],
            },
            self.index,
        )
    }

    /// `operator++`.
    // Port of: src/gpu/graphite/sparse_strips/Polyline.h#L90-L94 (chrome/m156)
    pub fn advance(&mut self) {
        self.index += 1;
        self.advance_to_valid_line();
    }

    /// `operator!=`.
    #[doc(alias = "operator!=")]
    // Port of: src/gpu/graphite/sparse_strips/Polyline.h#L96-L98 (chrome/m156)
    #[must_use]
    pub fn differs_from(&self, other: &Self) -> bool {
        self.index != other.index
    }

    // Port of: src/gpu/graphite/sparse_strips/Polyline.h#L101-L105 (chrome/m156)
    fn advance_to_next_valid_index(&mut self) {
        while self.index < self.count && self.pts[self.index as usize].x.is_nan() {
            self.index += 1;
        }
    }

    // Port of: src/gpu/graphite/sparse_strips/Polyline.h#L107-L122 (chrome/m156)
    fn advance_to_valid_line(&mut self) {
        while self.index + 1 < self.count {
            if self.pts[(self.index + 1) as usize].x.is_nan() {
                // If the next point in fPoints is a NaN, try to "leap-frog" over it. This will
                // be the majority of cases. Else, walk the remaining points until a valid point
                // or the end of the array is reached.
                self.index += 2;
                // Does nothing if we're already at a valid index
                self.advance_to_next_valid_index();
                continue;
            }
            return;
        }
        self.index = self.count;
    }
}

impl Iterator for LineIterator<'_> {
    type Item = (Line, i32);

    fn next(&mut self) -> Option<(Line, i32)> {
        if self.index >= self.count {
            return None;
        }
        let item = self.current();
        self.advance();
        Some(item)
    }
}
