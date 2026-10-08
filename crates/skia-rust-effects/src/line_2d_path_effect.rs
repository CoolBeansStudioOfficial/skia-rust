// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/Sk2DPathEffect.h, src/effects/Sk2DPathEffect.cpp

//! `SkLine2DPathEffect`: maps a path onto a lattice of lines and strokes them with the given
//! width.
//!
//! skia-rust: flattening (`CreateProc`, `flatten`) is not ported.

use skia_rust_core::flattenable::FlattenableRegistry;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_effect::{PathEffect, PathEffectBase};
use skia_rust_core::point::Point;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{SCALAR_HALF, int_to_scalar, scalar};
use skia_rust_core::stroke_rec::StrokeRec;
use skia_rust_core::write_buffer::BinaryWriteBuffer;

use crate::two_d_path_effect::{Sk2DBase, Sk2DKind};

// Port of: src/effects/Sk2DPathEffect.cpp#L114-L157 (chrome/m156)
#[derive(Clone, Debug)]
struct Line2DPathEffectImpl {
    base: Sk2DBase,
    width: scalar,
}

impl Sk2DKind for Line2DPathEffectImpl {
    // Port of: src/effects/Sk2DPathEffect.cpp#L130-L141 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // mirrors the C++ `int` -> `SkScalar` conversion
    fn next_span(&self, base: &Sk2DBase, u: i32, v: i32, ucount: i32, dst: &mut PathBuilder) {
        if ucount > 1 {
            let src = [
                Point::new(
                    int_to_scalar(u) + SCALAR_HALF,
                    int_to_scalar(v) + SCALAR_HALF,
                ),
                Point::new(
                    int_to_scalar(u + ucount) + SCALAR_HALF,
                    int_to_scalar(v) + SCALAR_HALF,
                ),
            ];
            let mut dst_p = [Point::default(); 2];
            base.matrix().map_points(&mut dst_p, &src);

            dst.move_to(dst_p[0]);
            dst.line_to(dst_p[1]);
        }
    }
}

/// `SkLine2DPathEffect::CreateProc`: the matrix, then the width.
// Port of: src/effects/Sk2DPathEffect.cpp#L149-L154 (chrome/m156)
pub fn create_proc(
    buffer: &mut ReadBuffer<'_>,
    _registry: &FlattenableRegistry,
) -> Option<PathEffect> {
    let matrix = buffer.read_matrix();
    let width = buffer.read_scalar();
    new(width, &matrix)
}

impl PathEffectBase for Line2DPathEffectImpl {
    // Port of: src/effects/Sk2DPathEffect.cpp#L162 (chrome/m156)
    fn type_name(&self) -> &'static str {
        "SkLine2DPathEffect"
    }

    // Port of: src/effects/Sk2DPathEffect.cpp#L156-L159 (chrome/m156)
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        buffer.write_matrix(self.base.matrix());
        buffer.write_scalar(self.width);
    }

    // Port of: src/effects/Sk2DPathEffect.cpp#L120-L128 (chrome/m156)
    fn on_filter_path(
        &self,
        dst: &mut PathBuilder,
        src: &Path,
        rec: &mut StrokeRec,
        _cull_rect: Option<&Rect>,
        _ctm: &Matrix,
    ) -> bool {
        if self.base.filter_path(self, dst, src) {
            rec.set_stroke_style(self.width, false);
            return true;
        }
        false
    }

    // Port of: src/effects/Sk2DPathEffect.cpp#L35-L37 (chrome/m156)
    // "For simplicity, assume fast bounds cannot be computed"
    fn compute_fast_bounds(&self, _bounds: Option<&mut Rect>) -> bool {
        false
    }
}

/// Provides `SkLine2DPathEffect::Make`: maps a path onto a line and strokes it with `width`.
///
/// Returns `None` if `width` is not non-negative (or is NaN).
// Port of: src/effects/Sk2DPathEffect.cpp#L159-L164 (chrome/m156)
#[doc(alias = "SkLine2DPathEffect::Make")]
#[must_use]
pub fn new(width: scalar, matrix: &Matrix) -> Option<PathEffect> {
    // `!(width >= 0)`: also rejects NaN.
    if width.is_nan() || width < 0.0 {
        return None;
    }
    Some(PathEffect::from_base(Line2DPathEffectImpl {
        base: Sk2DBase::new(matrix.clone()),
        width,
    }))
}

/// Provides `PathEffect::line_2d`, as `skia-safe` has it as an inherent method (Rust does not
/// allow inherent impls outside the defining crate).
pub trait Line2DPathEffectExt {
    /// Maps a path onto a line and strokes it with `width`; see [`new`].
    #[doc(alias = "SkLine2DPathEffect::Make")]
    fn line_2d(width: scalar, matrix: &Matrix) -> Option<PathEffect>;
}

impl Line2DPathEffectExt for PathEffect {
    fn line_2d(width: scalar, matrix: &Matrix) -> Option<PathEffect> {
        new(width, matrix)
    }
}
