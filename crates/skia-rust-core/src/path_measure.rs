// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPathMeasure.h, src/core/SkPathMeasure.cpp,
// src/core/SkPathMeasurePriv.h

//! Measuring paths one contour at a time (`SkPathMeasure.h`).

use crate::contour_measure::{ContourMeasure, ContourMeasureIter};
use crate::matrix::Matrix;
use crate::path::Path;
use crate::path_builder::PathBuilder;
use crate::point::{Point, Vector};
use crate::scalar::scalar;

pub use crate::contour_measure::MatrixFlags;

/// Measures the current contour of a path; [`next_contour`](Self::next_contour) moves on.
// Port of: include/core/SkPathMeasure.h#L20-L94 (chrome/m156)
#[doc(alias = "SkPathMeasure")]
#[derive(Debug, Default)]
pub struct PathMeasure {
    iter: ContourMeasureIter,
    contour: Option<ContourMeasure>,
}

impl PathMeasure {
    /// Measures `path`; if `force_closed`, contours are measured as closed. `res_scale` (default
    /// 1) controls the precision of the measure.
    // Port of: src/core/SkPathMeasure.cpp#L21-L25 (chrome/m156)
    #[must_use]
    pub fn new(path: &Path, force_closed: bool, res_scale: impl Into<Option<scalar>>) -> Self {
        let mut iter = ContourMeasureIter::new(path, force_closed, res_scale);
        let contour = iter.next();
        Self { iter, contour }
    }

    /// Restarts on `path`.
    // Port of: src/core/SkPathMeasure.cpp#L29-L32 (chrome/m156)
    #[doc(alias = "setPath")]
    pub fn set_path(&mut self, path: &Path, force_closed: bool) -> &mut Self {
        self.iter.reset(path, force_closed, None);
        self.contour = self.iter.next();
        self
    }

    /// The length of the current contour, or 0.
    // Port of: src/core/SkPathMeasure.cpp#L34-L36 (chrome/m156)
    #[doc(alias = "getLength")]
    #[must_use]
    pub fn length(&self) -> scalar {
        self.contour.as_ref().map_or(0.0, ContourMeasure::length)
    }

    /// Writes the position and/or tangent at `distance` on the current contour.
    // Port of: src/core/SkPathMeasure.cpp#L38-L40 (chrome/m156)
    #[doc(alias = "getPosTan")]
    #[must_use]
    pub fn get_pos_tan(
        &self,
        distance: scalar,
        position: Option<&mut Point>,
        tangent: Option<&mut Vector>,
    ) -> bool {
        self.contour
            .as_ref()
            .is_some_and(|c| c.get_pos_tan(distance, position, tangent))
    }

    /// The position and tangent at `distance` on the current contour.
    #[must_use]
    pub fn pos_tan(&self, distance: scalar) -> Option<(Point, Vector)> {
        self.contour.as_ref()?.pos_tan(distance)
    }

    /// A matrix positioned and/or rotated to the point at `distance` on the current contour.
    // Port of: src/core/SkPathMeasure.cpp#L42-L44 (chrome/m156)
    #[doc(alias = "getMatrix")]
    #[must_use]
    pub fn get_matrix(
        &self,
        distance: scalar,
        flags: impl Into<Option<MatrixFlags>>,
    ) -> Option<Matrix> {
        self.contour.as_ref()?.get_matrix(distance, flags)
    }

    /// Appends the part of the current contour between `start_d` and `stop_d` to `dst`.
    // Port of: src/core/SkPathMeasure.cpp#L46-L49 (chrome/m156)
    #[doc(alias = "getSegment")]
    pub fn get_segment(
        &self,
        start_d: scalar,
        stop_d: scalar,
        dst: &mut PathBuilder,
        start_with_move_to: bool,
    ) -> bool {
        self.contour
            .as_ref()
            .is_some_and(|c| c.get_segment(start_d, stop_d, dst, start_with_move_to))
    }

    /// True if the current contour is closed.
    // Port of: src/core/SkPathMeasure.cpp#L51-L53 (chrome/m156)
    #[doc(alias = "isClosed")]
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.contour.as_ref().is_some_and(ContourMeasure::is_closed)
    }

    /// Moves to the next contour; returns false if there is none.
    // Port of: src/core/SkPathMeasure.cpp#L55-L58 (chrome/m156)
    #[doc(alias = "nextContour")]
    pub fn next_contour(&mut self) -> bool {
        self.contour = self.iter.next();
        self.contour.is_some()
    }

    /// The current contour's measure.
    #[doc(alias = "currentMeasure")]
    #[must_use]
    pub fn current_measure(&self) -> &Option<ContourMeasure> {
        &self.contour
    }
}

/// `SkPathMeasurePriv`: test helpers.
#[doc(hidden)]
pub mod path_measure_priv {
    use super::PathMeasure;

    /// The number of segments in the current contour's measure.
    // Port of: src/core/SkPathMeasure.cpp#L68-L73 (chrome/m156)
    #[doc(alias = "CountSegments")]
    #[must_use]
    pub fn count_segments(meas: &PathMeasure) -> usize {
        meas.current_measure()
            .as_ref()
            .map_or(0, super::ContourMeasure::count_segments)
    }
}
