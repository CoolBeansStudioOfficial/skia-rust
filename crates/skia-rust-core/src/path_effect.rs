// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPathEffect.h, src/core/SkPathEffect.cpp,
// src/core/SkPathEffectBase.h

//! `SkPathEffect`: effects that modify the path before it is drawn (dashing, corners, ...).
//!
//! skia-rust: D2 ports the interface `SkPaint` needs: the [`PathEffect`] handle and the core
//! virtuals of [`PathEffectBase`]. The path effects themselves (dash, sum, compose, ...) and
//! the rest of `SkPathEffectBase` (`asPoints`, `asADash`) come with task C7, which extends
//! this module.

use core::any::Any;
use core::fmt;
use std::sync::Arc;

use crate::matrix::Matrix;
use crate::path::Path;
use crate::path_builder::PathBuilder;
use crate::rect::Rect;
use crate::stroke_rec::StrokeRec;

/// The virtual interface of a path effect (`SkPathEffectBase`).
///
/// Implementations are wrapped in a [`PathEffect`] with [`PathEffect::from_base`].
// Port of: src/core/SkPathEffectBase.h#L22-L137 (chrome/m156)
#[doc(alias = "SkPathEffectBase")]
pub trait PathEffectBase: Any + fmt::Debug + Send + Sync {
    /// Filters the input path into `dst`; false if the effect does not apply
    /// (`onFilterPath`). The output must be in the original (input) coordinate system,
    /// regardless of whether the path effect uses `ctm`.
    #[doc(alias = "onFilterPath")]
    fn on_filter_path(
        &self,
        dst: &mut PathBuilder,
        src: &Path,
        rec: &mut StrokeRec,
        cull_rect: Option<&Rect>,
        ctm: &Matrix,
    ) -> bool;

    /// Path effects *requiring* a valid CTM override this to return true (`onNeedsCTM`).
    #[doc(alias = "onNeedsCTM")]
    fn on_needs_ctm(&self) -> bool {
        false
    }

    /// Computes a conservative bounds for the effect, given the bounds of the path. `bounds` is
    /// both the input and output; if false is returned, fast bounds could not be calculated
    /// and `bounds` is undefined. If `bounds` is `None`, performs a dry run determining if
    /// bounds could be computed (`computeFastBounds`).
    #[doc(alias = "computeFastBounds")]
    fn compute_fast_bounds(&self, bounds: Option<&mut Rect>) -> bool;
}

/// A shared path effect (`sk_sp<SkPathEffect>`): a cheaply clonable handle to a
/// [`PathEffectBase`].
///
/// Equality is identity, as Skia compares `sk_sp`s ([`PathEffect::ptr_eq`]).
// Port of: include/core/SkPathEffect.h#L34-L86 (chrome/m156)
#[doc(alias = "SkPathEffect")]
#[derive(Clone)]
pub struct PathEffect(Arc<dyn PathEffectBase>);

impl PathEffect {
    /// Wraps a path effect implementation.
    #[must_use]
    pub fn from_base(base: impl PathEffectBase) -> PathEffect {
        PathEffect(Arc::new(base))
    }

    /// The implementation (`as_PEB`).
    #[doc(alias = "as_PEB")]
    #[must_use]
    pub fn as_base(&self) -> &dyn PathEffectBase {
        &*self.0
    }

    /// True if `self` and `other` are the same path effect (Skia's `sk_sp` comparison).
    #[must_use]
    pub fn ptr_eq(&self, other: &PathEffect) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Given a src path and a stroke rec, applies this effect to the src path, putting the
    /// result into `dst`, and modifies the rec as needed; false if the effect does not apply
    /// (`filterPath(dst, src, rec, cullR, ctm)`).
    // Port of: src/core/SkPathEffect.cpp#L31-L34 (chrome/m156)
    #[doc(alias = "filterPath")]
    pub fn filter_path_inplace_with_matrix(
        &self,
        dst: &mut PathBuilder,
        src: &Path,
        stroke_rec: &mut StrokeRec,
        cull_rect: Option<&Rect>,
        ctm: &Matrix,
    ) -> bool {
        self.0.on_filter_path(dst, src, stroke_rec, cull_rect, ctm)
    }

    /// [`filter_path_inplace_with_matrix`](Self::filter_path_inplace_with_matrix) with no cull
    /// rect and the identity matrix (`filterPath(dst, src, rec)`).
    // Port of: src/core/SkPathEffect.cpp#L27-L29 (chrome/m156)
    #[doc(alias = "filterPath")]
    pub fn filter_path_inplace(
        &self,
        dst: &mut PathBuilder,
        src: &Path,
        stroke_rec: &mut StrokeRec,
    ) -> bool {
        self.filter_path_inplace_with_matrix(dst, src, stroke_rec, None, Matrix::i())
    }

    /// True if this path effect requires a valid CTM (`needsCTM`).
    // Port of: src/core/SkPathEffect.cpp#L41-L43 (chrome/m156)
    #[doc(alias = "needsCTM")]
    #[must_use]
    pub fn needs_ctm(&self) -> bool {
        self.0.on_needs_ctm()
    }
}

impl PartialEq for PathEffect {
    /// Identity, as Skia's `sk_sp<SkPathEffect>` `operator==`.
    fn eq(&self, other: &PathEffect) -> bool {
        self.ptr_eq(other)
    }
}

impl fmt::Debug for PathEffect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PathEffect").field(&self.0).finish()
    }
}
