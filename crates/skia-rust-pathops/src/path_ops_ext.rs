// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
//! `PathOpsExt`: the boolean path operations as methods on [`Path`].
//!
//! `Path` lives in `skia-rust-core`, which `skia-rust-pathops` depends on, so the operations
//! cannot be inherent methods of `Path` without a cycle. An extension trait gives the same
//! method-call syntax as skia-safe (`path.op(&other, PathOp::Union)`, `path.simplify()`),
//! brought into scope by `use skia_rust::PathOpsExt`. The free functions in this crate's root
//! (`op`, `simplify`, `tight_bounds`, `as_winding`) are the same operations.

use skia_rust_core::path::Path;
use skia_rust_core::rect::Rect;

use crate::path_op::PathOp;

/// The path operations of `skia-rust-pathops` as methods on [`Path`].
#[doc(alias = "SkPathOps")]
pub trait PathOpsExt {
    /// `Op(*this, other, op)`: the boolean combination of the two paths, or `None` if the
    /// operation fails.
    #[doc(alias = "Op")]
    #[must_use]
    fn op(&self, other: &Path, op: PathOp) -> Option<Path>;

    /// `Simplify(*this)`: the path with its self-intersections resolved, or `None` on failure.
    #[doc(alias = "Simplify")]
    #[must_use]
    fn simplify(&self) -> Option<Path>;

    /// `ComputeTightBounds(*this)`: the bounds of the curves' extrema, or `None` on failure.
    #[doc(alias = "ComputeTightBounds")]
    #[must_use]
    fn tight_bounds(&self) -> Option<Rect>;

    /// `AsWinding(*this)`: the equivalent path with the winding fill type, or `None` on failure.
    #[doc(alias = "AsWinding")]
    #[must_use]
    fn as_winding(&self) -> Option<Path>;
}

impl PathOpsExt for Path {
    fn op(&self, other: &Path, op: PathOp) -> Option<Path> {
        crate::op(self, other, op)
    }

    fn simplify(&self) -> Option<Path> {
        crate::simplify(self)
    }

    fn tight_bounds(&self) -> Option<Rect> {
        crate::tight_bounds(self)
    }

    fn as_winding(&self) -> Option<Path> {
        crate::as_winding(self)
    }
}
