// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/Sk2DPathEffect.h, src/effects/Sk2DPathEffect.cpp

//! `Sk2DPathEffect`: the base of the 2D path effects (`SkLine2DPathEffect`,
//! `SkPath2DPathEffect`). It walks the lattice of the path's inverse-transformed region and
//! calls the subclass for each span of lattice points.
//!
//! skia-rust: the C++ virtual `begin`/`next`/`end`/`nextSpan` hooks are the [`Sk2DKind`] trait.
//! Neither subclass overrides `begin` or `end`, so they are not part of the trait.

use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::region::Region;
use skia_rust_core::scalar::{SCALAR_1, SCALAR_HALF, int_to_scalar};
use skia_rust_raster::region_path::RegionExt;

/// The virtual hooks of a 2D path effect (`next`, `nextSpan`).
// Port of: src/effects/Sk2DPathEffect.cpp#L22-L41 (chrome/m156)
pub(crate) trait Sk2DKind {
    /// Called for each lattice location (`next(loc, u, v, dst)`). Default: does nothing.
    fn next(&self, _loc: Point, _u: i32, _v: i32, _dst: &mut PathBuilder) {}

    /// Called once per span of lattice locations in the u-direction (`nextSpan`). The default
    /// calls [`Sk2DKind::next`] with each location.
    // Port of: src/effects/Sk2DPathEffect.cpp#L43-L62 (chrome/m156)
    fn next_span(&self, base: &Sk2DBase, x: i32, y: i32, ucount: i32, dst: &mut PathBuilder) {
        if base.inverse.is_none() {
            return;
        }

        let mat = &base.matrix;
        let mut x = x;
        let mut ucount = ucount;
        let mut src = Point::new(
            int_to_scalar(x) + SCALAR_HALF,
            int_to_scalar(y) + SCALAR_HALF,
        );
        loop {
            let dst_pt = mat.map_point(src);
            self.next(dst_pt, x, y, dst);
            x += 1;
            src.x += SCALAR_1;
            ucount -= 1;
            if ucount <= 0 {
                break;
            }
        }
    }
}

/// The state shared by the 2D path effects (`Sk2DPathEffect`'s `fMatrix`, `fInverse` and
/// `fMatrixIsInvertible`).
// Port of: src/effects/Sk2DPathEffect.cpp#L17-L24 (chrome/m156)
#[derive(Clone, Debug)]
pub(crate) struct Sk2DBase {
    matrix: Matrix,
    // `None` when the matrix is not invertible (`fMatrixIsInvertible` is false).
    inverse: Option<Matrix>,
}

impl Sk2DBase {
    // Port of: src/effects/Sk2DPathEffect.cpp#L17-L24 (chrome/m156)
    pub(crate) fn new(matrix: Matrix) -> Self {
        // Calling invert will set the type mask on both matrices, making them thread safe.
        let inverse = matrix.invert();
        Self { matrix, inverse }
    }

    /// The matrix mapping lattice locations to the path (`getMatrix`).
    pub(crate) fn matrix(&self) -> &Matrix {
        &self.matrix
    }

    /// Filters `src` (`Sk2DPathEffect::onFilterPath`), calling the hooks of `kind`.
    // Port of: src/effects/Sk2DPathEffect.cpp#L64-L95 (chrome/m156)
    pub(crate) fn filter_path(
        &self,
        kind: &impl Sk2DKind,
        dst: &mut PathBuilder,
        src: &Path,
    ) -> bool {
        let Some(inverse) = &self.inverse else {
            return false;
        };

        let tmp = src.make_transform(inverse);
        let ir = tmp.bounds().round();
        if !ir.is_empty() {
            let mut rgn = Region::new();
            rgn.set_path(&tmp, &Region::from_rect(ir));
            for rect in skia_rust_core::region::Iterator::new(&rgn) {
                for y in rect.top..rect.bottom {
                    kind.next_span(self, rect.left, y, rect.width(), dst);
                }
            }
        }
        true
    }
}
