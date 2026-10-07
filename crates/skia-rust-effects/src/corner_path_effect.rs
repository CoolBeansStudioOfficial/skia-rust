// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/SkCornerPathEffect.h, src/effects/SkCornerPathEffect.cpp

//! `SkCornerPathEffect`: a path effect that can turn sharp corners into various treatments
//! (currently rounding).
//!
//! skia-rust: flattening (`CreateProc`, `flatten`) is not ported.

use skia_rust_core::floating_point::is_finite;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::{Iter, Path};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_effect::{PathEffect, PathEffectBase};
use skia_rust_core::path_types::PathVerb;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{SCALAR_HALF, scalar};
use skia_rust_core::stroke_rec::StrokeRec;

// Port of: src/effects/SkCornerPathEffect.cpp#L26-L37 (chrome/m156)
fn compute_step(a: Point, b: Point, radius: scalar, step: &mut Vector) -> bool {
    let dist = Point::distance(a, b);

    *step = b - a;
    if dist <= radius * 2.0 {
        *step *= SCALAR_HALF;
        false
    } else {
        *step *= radius / dist;
        true
    }
}

// Port of: src/effects/SkCornerPathEffect.cpp#L39-L165 (chrome/m156)
#[derive(Clone, Debug)]
struct CornerPathEffectImpl {
    radius: scalar,
}

impl PathEffectBase for CornerPathEffectImpl {
    // Port of: src/effects/SkCornerPathEffect.cpp#L46-L151 (chrome/m156)
    #[allow(clippy::too_many_lines)] // one function in C++
    fn on_filter_path(
        &self,
        dst: &mut PathBuilder,
        src: &Path,
        _rec: &mut StrokeRec,
        _cull_rect: Option<&Rect>,
        _ctm: &Matrix,
    ) -> bool {
        if self.radius <= 0.0 {
            return false;
        }

        // just need a value that won't match when we compare it initially
        let mut prev_verb: Option<PathVerb> = None;

        let mut prev_is_valid = true;

        // to avoid warnings
        let mut step = Vector::new(0.0, 0.0);
        let mut move_to = Point::new(0.0, 0.0);
        let mut first_step = Vector::new(0.0, 0.0);
        let mut last_corner = Point::new(0.0, 0.0);

        let mut iter = Iter::new(src, false);
        while let Some(rec) = iter.next_rec() {
            let pts = rec.points();
            match rec.verb() {
                PathVerb::Move => {
                    // close out the previous (open) contour
                    if Some(PathVerb::Line) == prev_verb {
                        dst.line_to(last_corner);
                    }
                    let closed = iter.is_closed_contour();
                    if closed {
                        move_to = pts[0];
                        prev_is_valid = false;
                    } else {
                        dst.move_to(pts[0]);
                        prev_is_valid = true;
                    }
                }
                PathVerb::Line => {
                    let draw_segment = compute_step(pts[0], pts[1], self.radius, &mut step);
                    // prev corner
                    if prev_is_valid {
                        dst.quad_to(pts[0], Point::new(pts[0].x + step.x, pts[0].y + step.y));
                    } else {
                        // C++ sets prevIsValid = true here; it is set below for both branches.
                        dst.move_to(move_to + step);
                    }
                    if draw_segment {
                        dst.line_to(Point::new(pts[1].x - step.x, pts[1].y - step.y));
                    }
                    last_corner = pts[1];
                    prev_is_valid = true;
                }
                PathVerb::Quad => {
                    // TBD - just replicate the curve for now
                    if !prev_is_valid {
                        dst.move_to(pts[0]);
                        prev_is_valid = true;
                    }
                    dst.quad_to(pts[1], pts[2]);
                    last_corner = pts[2];
                    first_step.set(0.0, 0.0);
                }
                PathVerb::Conic => {
                    // TBD - just replicate the curve for now
                    if !prev_is_valid {
                        dst.move_to(pts[0]);
                        prev_is_valid = true;
                    }
                    dst.conic_to(pts[1], pts[2], rec.conic_weight());
                    last_corner = pts[2];
                    first_step.set(0.0, 0.0);
                }
                PathVerb::Cubic => {
                    if !prev_is_valid {
                        dst.move_to(pts[0]);
                        prev_is_valid = true;
                    }
                    // TBD - just replicate the curve for now
                    dst.cubic_to(pts[1], pts[2], pts[3]);
                    last_corner = pts[3];
                    first_step.set(0.0, 0.0);
                }
                PathVerb::Close => {
                    if first_step.x != 0.0 || first_step.y != 0.0 {
                        dst.quad_to(
                            last_corner,
                            Point::new(last_corner.x + first_step.x, last_corner.y + first_step.y),
                        );
                    }
                    dst.close();
                    prev_is_valid = false;
                }
            }
            if Some(PathVerb::Move) == prev_verb {
                first_step = step;
            }
            prev_verb = Some(rec.verb());
        }
        if prev_is_valid {
            dst.line_to(last_corner);
        }
        true
    }

    // Port of: src/effects/SkCornerPathEffect.cpp#L153-L157 (chrome/m156)
    fn compute_fast_bounds(&self, _bounds: Option<&mut Rect>) -> bool {
        // Rounding sharp corners within a path produces a new path that is still contained within
        // the original's bounds, so leave 'bounds' unmodified.
        true
    }
}

/// Provides the corner path effect (`SkCornerPathEffect::Make`): a path effect that rounds
/// sharp corners with `radius`. Returns `None` if `radius` is not finite and positive.
// Port of: src/effects/SkCornerPathEffect.cpp#L172-L176 (chrome/m156)
#[doc(alias = "SkCornerPathEffect::Make")]
#[must_use]
pub fn new(radius: scalar) -> Option<PathEffect> {
    (is_finite(radius) && (radius > 0.0))
        .then(|| PathEffect::from_base(CornerPathEffectImpl { radius }))
}

/// Provides `PathEffect::corner_path`, as `skia-safe` has it as an inherent method (Rust does
/// not allow inherent impls outside the defining crate).
pub trait CornerPathEffectExt {
    /// Rounds sharp corners with `radius`; see [`new`].
    #[doc(alias = "SkCornerPathEffect::Make")]
    fn corner_path(radius: scalar) -> Option<PathEffect>;
}

impl CornerPathEffectExt for PathEffect {
    fn corner_path(radius: scalar) -> Option<PathEffect> {
        new(radius)
    }
}
