// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/gpu/tessellate/MidpointContourParser.h (chrome/m156)

//! Parses out each contour in a path and tracks the midpoint of its vertices. Example usage:
//!
//! ```text
//! let mut parser = MidpointContourParser::new(&path);
//! while parser.parse_next_contour() {
//!     let midpoint = parser.current_midpoint();
//!     for (verb, pts, weight) in parser.current_contour() {
//!         // ...
//!     }
//! }
//! ```

use skia_rust_core::path::Path;
use skia_rust_core::path_priv::{RangeIter, iterate_raw};
use skia_rust_core::path_types::PathVerb;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::scalar;

/// Parses each contour of a path and tracks the midpoint of its vertices.
// Port of: src/gpu/tessellate/MidpointContourParser.h#L24-L113 (chrome/m156), class `MidpointContourParser`.
#[derive(Debug)]
pub struct MidpointContourParser<'a> {
    // The verbs, points and weights from the start of the current contour onwards.
    verbs: &'a [PathVerb],
    verbs_idx: usize,
    points: &'a [Point],
    pts_idx: usize,
    weights: &'a [scalar],
    wts_idx: usize,
    midpoint: Point,
    midpoint_weight: i32,
}

impl<'a> MidpointContourParser<'a> {
    /// `MidpointContourParser(const SkPath& path)`.
    #[must_use]
    pub fn new(path: &'a Path) -> Self {
        Self {
            verbs: path.verbs(),
            verbs_idx: 0,
            points: path.points(),
            pts_idx: 0,
            weights: path.conic_weights(),
            wts_idx: 0,
            midpoint: Point::new(0.0, 0.0),
            midpoint_weight: 0,
        }
    }

    /// Advances the internal state to the next contour in the path. Returns false if there are no
    /// more contours.
    // Port of: src/gpu/tessellate/MidpointContourParser.h#L35-L87 (chrome/m156), `parseNextContour`.
    pub fn parse_next_contour(&mut self) -> bool {
        let mut has_geometry = false;
        while self.verbs_idx < self.verbs.len() {
            match self.verbs[self.verbs_idx] {
                PathVerb::Move => {
                    if !has_geometry {
                        self.midpoint = Point::new(0.0, 0.0);
                        self.midpoint_weight = 0;
                        self.advance(); // Resets fPtsIdx to 0 and advances fPoints.
                        self.pts_idx = 1; // Increment fPtsIdx past the kMove.
                        // The `continue` of the C++ loop also increments fVerbsIdx.
                        self.verbs_idx += 1;
                        continue;
                    }
                    #[allow(clippy::float_cmp)] // SkPoint's operator!= is an exact comparison
                    if self.points[0] != self.points[self.pts_idx - 1] {
                        // There's an implicit close at the end. Add the start point to our mean.
                        self.midpoint += self.points[0];
                        self.midpoint_weight += 1;
                    }
                    return true;
                }
                PathVerb::Close => {
                    // `default: continue;`
                    self.verbs_idx += 1;
                    continue;
                }
                PathVerb::Line => self.pts_idx += 1,
                PathVerb::Conic => {
                    self.wts_idx += 1;
                    self.pts_idx += 2;
                }
                PathVerb::Quad => self.pts_idx += 2,
                PathVerb::Cubic => self.pts_idx += 3,
            }
            self.midpoint += self.points[self.pts_idx - 1];
            self.midpoint_weight += 1;
            has_geometry = true;
            self.verbs_idx += 1;
        }
        #[allow(clippy::float_cmp)] // SkPoint's operator!= is an exact comparison
        if has_geometry && self.points[0] != self.points[self.pts_idx - 1] {
            // There's an implicit close at the end. Add the start point to our mean.
            self.midpoint += self.points[0];
            self.midpoint_weight += 1;
        }
        has_geometry
    }

    /// Allows for iterating the current contour (`SkPathPriv::Iterate` over its verbs).
    // Port of: src/gpu/tessellate/MidpointContourParser.h#L89-L91 (chrome/m156), `currentContour`.
    #[must_use]
    pub fn current_contour(&self) -> RangeIter<'a> {
        // Copy the `&'a` slice out of `self` so the iterator borrows for `'a`, not for `&self`.
        let verbs = self.verbs;
        iterate_raw(&verbs[..self.verbs_idx], self.points, self.weights)
    }

    /// The mean of the vertices of the current contour, including the implicit close.
    // Port of: src/gpu/tessellate/MidpointContourParser.h#L93 (chrome/m156), `currentMidpoint`.
    #[must_use]
    pub fn current_midpoint(&self) -> Point {
        self.midpoint * (1.0 / int_to_float(self.midpoint_weight))
    }

    // Port of: src/gpu/tessellate/MidpointContourParser.h#L95-L104 (chrome/m156), `advance`.
    fn advance(&mut self) {
        self.verbs = &self.verbs[self.verbs_idx..];
        self.verbs_idx = 0;
        self.points = &self.points[self.pts_idx..];
        self.pts_idx = 0;
        self.weights = &self.weights[self.wts_idx..];
        self.wts_idx = 0;
    }
}

/// The `int` to `float` conversion the C++ performs implicitly.
#[inline]
#[allow(clippy::cast_precision_loss)] // mirrors the implicit int -> float conversion in C++
fn int_to_float(x: i32) -> f32 {
    x as f32
}
