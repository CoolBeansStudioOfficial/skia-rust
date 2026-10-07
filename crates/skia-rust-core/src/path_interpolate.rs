// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkPath_interpolate.cpp

//! Interpolating between paths with the same verbs (`SkPath_interpolate.cpp`).

use crate::path::Path;
use crate::point::Point;
use crate::scalar::scalar;

// Port of: src/core/SkPath_interpolate.cpp#L21-L28 (chrome/m156)
fn mul(s: f32, p: Point) -> Point {
    Point::new(p.x * s, p.y * s)
}

// Port of: src/core/SkPath_interpolate.cpp#L30-L32 (chrome/m156)
fn lerp(from: Point, to: Point, t: f32) -> Point {
    from + mul(t, to - from)
}

// Returns true iff the paths have the same verbs and conicWeights, and the same number of points.
// Port of: src/core/SkPath_interpolate.cpp#L50-L54 (chrome/m156)
fn can_interpolate(from: &Path, to: &Path) -> bool {
    from.points().len() == to.points().len()
        && from.verbs() == to.verbs()
        && from.conic_weights() == to.conic_weights()
}

// A path whose points are linearly interpolated between from and to, using t
// (logically) newPoint = (1 - t) * fromPoint + t * toPoint
// The result has the same fillType as the 'from' path.
// Port of: src/core/SkPath_interpolate.cpp#L66-L82 (chrome/m156)
fn interpolate(from: &Path, to: &Path, t: f32) -> Option<Path> {
    if !can_interpolate(from, to) {
        return None;
    }

    // note, if from and to have different fill-types, that's up to the client to deal with.
    let fill_type = from.fill_type();
    let dst: Vec<Point> = from
        .points()
        .iter()
        .zip(to.points())
        .map(|(&f, &to)| lerp(f, to, t))
        .collect();
    Some(Path::raw(
        &dst,
        from.verbs(),
        from.conic_weights(),
        fill_type,
        None,
    ))
}

impl Path {
    /// True if the paths have the same verbs and conic weights, and the same number of points.
    // Port of: src/core/SkPath_interpolate.cpp#L88-L90 (chrome/m156)
    #[doc(alias = "isInterpolatable")]
    #[must_use]
    pub fn is_interpolatable(&self, compare: &Path) -> bool {
        can_interpolate(self, compare)
    }

    /// `(this * weight) + ending * (1 - weight)`, or an empty path if not interpolatable.
    // Port of: src/core/SkPath_interpolate.cpp#L92-L94 (chrome/m156)
    #[doc(alias = "makeInterpolate")]
    #[must_use]
    pub fn make_interpolate(&self, ending: &Path, weight: scalar) -> Path {
        interpolate(self, ending, 1.0 - weight).unwrap_or_default()
    }

    /// `(this * weight) + ending * (1 - weight)`, or `None` if not interpolatable.
    #[must_use]
    pub fn interpolate(&self, ending: &Path, weight: scalar) -> Option<Path> {
        interpolate(self, ending, 1.0 - weight)
    }

    /// Writes the interpolation to `out`; returns false (leaving `out` alone) if not
    /// interpolatable.
    // Port of: src/core/SkPath_interpolate.cpp#L96-L102 (chrome/m156)
    pub fn interpolate_inplace(&self, ending: &Path, weight: scalar, out: &mut Path) -> bool {
        if let Some(result) = interpolate(self, ending, 1.0 - weight) {
            *out = result;
            return true;
        }
        false
    }
}
