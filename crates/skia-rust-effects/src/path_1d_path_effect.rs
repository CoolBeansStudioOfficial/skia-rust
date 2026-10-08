// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/Sk1DPathEffect.h, src/effects/Sk1DPathEffect.cpp

//! `SkPath1DPathEffect`: dashes a path by replicating another path along it, translating,
//! rotating or morphing it at each point.
//!
//! skia-rust: flattening (`CreateProc`, `flatten`) is not ported. `Sk1DPathEffect` (the abstract
//! base, whose `begin`/`next` are overridden only by `SkPath1DPathEffectImpl`) is folded into
//! [`Path1DPathEffectImpl`].

use skia_rust_core::floating_point::{float_midpoint, is_finite};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::{Iter, Path};
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_effect::{PathEffect, PathEffectBase};
use skia_rust_core::path_measure::PathMeasure;
use skia_rust_core::path_types::PathVerb;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{scalar, scalar_mod};
use skia_rust_core::stroke_rec::StrokeRec;

// Since we are stepping by a float, the do/while loop might go on forever (or nearly so).
// Put in a governor to limit crash values from looping too long (and allocating too much ram).
// Port of: src/effects/Sk1DPathEffect.cpp#L23 (chrome/m156)
const MAX_REASONABLE_ITERATIONS: i32 = 100_000;

/// How the replicated path is transformed at each point (`SkPath1DPathEffect::Style`).
#[doc(alias = "SkPath1DPathEffect::Style")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Style {
    /// Translate the path to each point (`kTranslate_Style`).
    Translate = 0,
    /// Translate and rotate the path to each point, along the tangent (`kRotate_Style`).
    Rotate = 1,
    /// Morph each point of the path onto the followed path (`kMorph_Style`).
    Morph = 2,
}

// Port of: src/effects/Sk1DPathEffect.cpp#L63-L125 (chrome/m156)
#[derive(Clone, Debug)]
struct Path1DPathEffectImpl {
    path: Path,
    advance: scalar,
    initial_offset: scalar,
    style: Style,
}

impl Path1DPathEffectImpl {
    // Port of: src/effects/Sk1DPathEffect.cpp#L17-L40 (chrome/m156), Sk1DPathEffect::onFilterPath
    fn filter_1d(&self, builder: &mut PathBuilder, src: &Path) -> bool {
        let mut meas = PathMeasure::new(src, false, None);
        loop {
            let mut governor = MAX_REASONABLE_ITERATIONS;
            let length = meas.length();
            // `begin(length)` (SkPath1DPathEffectImpl::begin) returns the initial offset.
            let mut distance = self.initial_offset;
            while distance < length {
                governor -= 1;
                if governor < 0 {
                    break;
                }
                let delta = self.next(builder, distance, &meas);
                if delta <= 0.0 {
                    break;
                }
                distance += delta;
            }
            if governor < 0 {
                return false;
            }
            if !meas.next_contour() {
                break;
            }
        }
        true
    }

    // Port of: src/effects/Sk1DPathEffect.cpp#L131-L154 (chrome/m156), SkPath1DPathEffectImpl::next
    fn next(&self, builder: &mut PathBuilder, distance: scalar, meas: &PathMeasure) -> scalar {
        match self.style {
            Style::Translate => {
                let mut pos = Point::default();
                if meas.get_pos_tan(distance, Some(&mut pos), None) {
                    builder.add_path_with_offset(&self.path, pos, None);
                }
            }
            Style::Rotate => {
                if let Some(matrix) = meas.get_matrix(distance, None) {
                    builder.add_path_with_transform(&self.path, &matrix, None);
                }
            }
            Style::Morph => morph_path(builder, &self.path, meas, distance),
        }
        self.advance
    }
}

impl PathEffectBase for Path1DPathEffectImpl {
    // Port of: src/effects/Sk1DPathEffect.cpp#L99-L103 (chrome/m156)
    fn on_filter_path(
        &self,
        builder: &mut PathBuilder,
        src: &Path,
        rec: &mut StrokeRec,
        _cull_rect: Option<&Rect>,
        _ctm: &Matrix,
    ) -> bool {
        rec.set_fill_style();
        self.filter_1d(builder, src)
    }

    // Port of: src/effects/Sk1DPathEffect.cpp#L43-L45 (chrome/m156)
    // "For simplicity, assume fast bounds cannot be computed"
    fn compute_fast_bounds(&self, _bounds: Option<&mut Rect>) -> bool {
        false
    }
}

// Port of: src/effects/Sk1DPathEffect.cpp#L127-L145 (chrome/m156), morphpoints
fn morph_points(dst: &mut [Point], src: &[Point], meas: &PathMeasure, distance: scalar) -> bool {
    debug_assert!(dst.len() >= src.len());
    for (i, s) in src.iter().enumerate() {
        let sx = s.x;
        let sy = s.y;

        let mut pos = Point::default();
        let mut tangent = Vector::default();
        if !meas.get_pos_tan(distance + sx, Some(&mut pos), Some(&mut tangent)) {
            return false;
        }

        let mut matrix = Matrix::new_identity();
        let pt = Point::new(sx, sy);
        matrix.set_sin_cos((tangent.y, tangent.x), Point::new(0.0, 0.0));
        matrix.pre_translate((-sx, 0.0));
        matrix.post_translate((pos.x, pos.y));
        dst[i] = matrix.map_point(pt);
    }
    true
}

// Port of: src/effects/Sk1DPathEffect.cpp#L147-L196 (chrome/m156), morphpath
//
// TODO (from Skia): need differentially more subdivisions when the follow-path is curvy.
fn morph_path(dst: &mut PathBuilder, src: &Path, meas: &PathMeasure, distance: scalar) {
    let mut iter = Iter::new(src, false);
    let mut dst_p = [Point::default(); 3];

    while let Some(rec) = iter.next_rec() {
        let src_p = rec.points();
        match rec.verb() {
            PathVerb::Move => {
                if morph_points(&mut dst_p, src_p, meas, distance) {
                    dst.move_to(dst_p[0]);
                }
            }
            PathVerb::Line => {
                // A line is morphed as a quad whose control point is the midpoint.
                let scratch = [
                    src_p[0],
                    Point::new(
                        float_midpoint(src_p[0].x, src_p[1].x),
                        float_midpoint(src_p[0].y, src_p[1].y),
                    ),
                    src_p[1],
                ];
                if morph_points(&mut dst_p, &scratch[1..], meas, distance) {
                    dst.quad_to(dst_p[0], dst_p[1]);
                }
            }
            PathVerb::Quad => {
                if morph_points(&mut dst_p, &src_p[1..], meas, distance) {
                    dst.quad_to(dst_p[0], dst_p[1]);
                }
            }
            PathVerb::Conic => {
                if morph_points(&mut dst_p, &src_p[1..], meas, distance) {
                    dst.conic_to(dst_p[0], dst_p[1], rec.conic_weight());
                }
            }
            PathVerb::Cubic => {
                if morph_points(&mut dst_p, &src_p[1..], meas, distance) {
                    dst.cubic_to(dst_p[0], dst_p[1], dst_p[2]);
                }
            }
            PathVerb::Close => {
                dst.close();
            }
        }
    }
}

/// Provides `SkPath1DPathEffect::Make`: dashes the path by replicating `path` along it.
///
/// - `path`: the path to replicate (dash)
/// - `advance`: the space between instances of `path`; must be positive
/// - `phase`: distance (mod `advance`) along the path for its initial position
/// - `style`: how to transform `path` at each point (based on the current position and tangent)
///
/// Returns `None` if `advance` is not positive, if `advance` or `phase` is not finite, or if
/// `path` is empty.
// Port of: src/effects/Sk1DPathEffect.cpp#L202-L209 (chrome/m156)
#[doc(alias = "SkPath1DPathEffect::Make")]
#[must_use]
pub fn new(path: &Path, advance: scalar, phase: scalar, style: Style) -> Option<PathEffect> {
    if advance <= 0.0 || !is_finite(advance) || !is_finite(phase) || path.is_empty() {
        return None;
    }

    // cleanup their phase parameter, inverting it so that it becomes an offset along the path
    // (to match the interpretation in PostScript)
    let mut phase = phase;
    if phase < 0.0 {
        phase = -phase;
        if phase > advance {
            phase = scalar_mod(phase, advance);
        }
    } else {
        if phase > advance {
            phase = scalar_mod(phase, advance);
        }
        phase = advance - phase;
    }
    // now catch the edge case where phase == advance (within epsilon)
    if phase >= advance {
        phase = 0.0;
    }
    debug_assert!(phase >= 0.0);

    Some(PathEffect::from_base(Path1DPathEffectImpl {
        path: path.clone(),
        advance,
        initial_offset: phase,
        style,
    }))
}

/// Provides `PathEffect::path_1d`, as `skia-safe` has it as an inherent method (Rust does not
/// allow inherent impls outside the defining crate).
pub trait Path1DPathEffectExt {
    /// Dashes the path by replicating `path` along it; see [`new`].
    #[doc(alias = "SkPath1DPathEffect::Make")]
    fn path_1d(path: &Path, advance: scalar, phase: scalar, style: Style) -> Option<PathEffect>;
}

impl Path1DPathEffectExt for PathEffect {
    fn path_1d(path: &Path, advance: scalar, phase: scalar, style: Style) -> Option<PathEffect> {
        new(path, advance, phase, style)
    }
}
