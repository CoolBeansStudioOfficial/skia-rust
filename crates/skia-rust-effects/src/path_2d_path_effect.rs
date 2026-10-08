// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/Sk2DPathEffect.h, src/effects/Sk2DPathEffect.cpp

//! `SkPath2DPathEffect`: stamps a path at each lattice point of the transformed path.
//!
//! skia-rust: flattening (`CreateProc`, `flatten`) is not ported.

use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_effect::{PathEffect, PathEffectBase};
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::stroke_rec::StrokeRec;

use crate::two_d_path_effect::{Sk2DBase, Sk2DKind};

// Port of: src/effects/Sk2DPathEffect.cpp#L166-L195 (chrome/m156)
#[derive(Clone, Debug)]
struct Path2DPathEffectImpl {
    base: Sk2DBase,
    path: Path,
}

impl Sk2DKind for Path2DPathEffectImpl {
    // Port of: src/effects/Sk2DPathEffect.cpp#L172-L174 (chrome/m156)
    fn next(&self, loc: Point, _u: i32, _v: i32, dst: &mut PathBuilder) {
        dst.add_path_with_offset(&self.path, loc, None);
    }
}

impl PathEffectBase for Path2DPathEffectImpl {
    // Port of: src/effects/Sk2DPathEffect.cpp#L83-L113 (chrome/m156), Sk2DPathEffect::onFilterPath
    fn on_filter_path(
        &self,
        dst: &mut PathBuilder,
        src: &Path,
        _rec: &mut StrokeRec,
        _cull_rect: Option<&Rect>,
        _ctm: &Matrix,
    ) -> bool {
        self.base.filter_path(self, dst, src)
    }

    // Port of: src/effects/Sk2DPathEffect.cpp#L35-L37 (chrome/m156)
    // "For simplicity, assume fast bounds cannot be computed"
    fn compute_fast_bounds(&self, _bounds: Option<&mut Rect>) -> bool {
        false
    }
}

/// Provides `SkPath2DPathEffect::Make`: stamps `path` at each lattice point of the path mapped
/// through the inverse of `matrix`.
// Port of: src/effects/Sk2DPathEffect.cpp#L197-L200 (chrome/m156)
#[doc(alias = "SkPath2DPathEffect::Make")]
#[must_use]
pub fn new(matrix: &Matrix, path: &Path) -> PathEffect {
    PathEffect::from_base(Path2DPathEffectImpl {
        base: Sk2DBase::new(matrix.clone()),
        path: path.clone(),
    })
}

/// Provides `PathEffect::path_2d`, as `skia-safe` has it as an inherent method (Rust does not
/// allow inherent impls outside the defining crate).
pub trait Path2DPathEffectExt {
    /// Stamps `path` at each lattice point of the transformed path; see [`new`].
    #[doc(alias = "SkPath2DPathEffect::Make")]
    fn path_2d(matrix: &Matrix, path: &Path) -> PathEffect;
}

impl Path2DPathEffectExt for PathEffect {
    fn path_2d(matrix: &Matrix, path: &Path) -> PathEffect {
        new(matrix, path)
    }
}
