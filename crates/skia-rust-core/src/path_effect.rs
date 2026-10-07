// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPathEffect.h, src/core/SkPathEffect.cpp,
// src/core/SkPathEffectBase.h

//! `SkPathEffect`: objects that affect the geometry of a drawing primitive before it is
//! transformed by the canvas' matrix and drawn. Dashing is implemented as a path effect (in
//! `skia-rust-effects`).
//!
//! skia-rust: `SkPathEffect` is a cheaply clonable `Arc` around a [`PathEffectBase`] trait
//! object (C++: `SkPathEffect` / `SkPathEffectBase` and its subclasses). Flattening
//! (`flatten`, `CreateProc`, `Deserialize`) is not ported.

use std::fmt;
use std::sync::Arc;

use bitflags::bitflags;

use crate::matrix::Matrix;
use crate::path::Path;
use crate::path_builder::PathBuilder;
use crate::point::{Point, Vector};
use crate::rect::Rect;
use crate::scalar::{SCALAR_1, scalar};
use crate::stroke_rec::StrokeRec;

bitflags! {
    /// Flags that impact the drawing of the points of a [`PointData`]
    /// (`SkPathEffectBase::PointData::PointFlags`). Currently none of these flags are supported.
    // Port of: src/core/SkPathEffectBase.h#L46-L50 (chrome/m156)
    #[doc(alias = "SkPathEffectBase::PointData::PointFlags")]
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct PointFlags: u32 {
        /// Draw points as circles (instead of rects) (`kCircles_PointFlag`).
        const CIRCLES = 0x01;
        /// Draw points as stamps of the returned path (`kUsePath_PointFlag`).
        const USE_PATH = 0x02;
        /// Apply `clip_rect` before drawing the points (`kUseClip_PointFlag`).
        const USE_CLIP = 0x04;
    }
}

/// All the information needed to draw the point primitives returned by
/// [`PathEffect::as_points`] (`SkPathEffectBase::PointData`).
// Port of: src/core/SkPathEffectBase.h#L28-L61 (chrome/m156)
#[doc(alias = "SkPathEffectBase::PointData")]
#[derive(Clone, Debug)]
pub struct PointData {
    /// Flags that impact the drawing of the points (`fFlags`).
    pub flags: PointFlags,
    /// The center point of each generated point (`fPoints`; `fNumPoints` is `points.len()`).
    pub points: Vec<Point>,
    /// The size to draw the points (`fSize`).
    pub size: Vector,
    /// Clip required to draw the points, if `USE_CLIP` is set (`fClipRect`).
    pub clip_rect: Rect,
    /// 'Stamp' to be used at each point, if `USE_PATH` is set (`fPath`).
    pub path: Path,
    /// If not empty, contains geometry for the first point (`fFirst`).
    pub first: Path,
    /// If not empty, contains geometry for the last point (`fLast`).
    pub last: Path,
}

impl Default for PointData {
    // Port of: src/core/SkPathEffectBase.h#L29-L36 (chrome/m156)
    fn default() -> Self {
        Self {
            flags: PointFlags::empty(),
            points: Vec::new(),
            size: Point::new(SCALAR_1, SCALAR_1),
            clip_rect: Rect::default(),
            path: Path::default(),
            first: Path::default(),
            last: Path::default(),
        }
    }
}

impl PointData {
    /// The number of points (`fNumPoints`).
    #[doc(alias = "fNumPoints")]
    #[must_use]
    pub fn num_points(&self) -> usize {
        self.points.len()
    }
}

/// The dash pattern of a path effect that can be represented as a dash
/// (`SkPathEffectBase::DashInfo`).
// Port of: src/core/SkPathEffectBase.h#L109-L112 (chrome/m156)
#[doc(alias = "SkPathEffectBase::DashInfo")]
#[derive(Clone, Debug, PartialEq)]
pub struct DashInfo {
    /// The on/off intervals (`fIntervals`).
    pub intervals: Vec<scalar>,
    /// The phase, in `0..sum(intervals)` (`fPhase`).
    pub phase: scalar,
}

/// The virtual interface of path effects (`SkPathEffectBase`). Implement it and wrap the
/// implementation with [`PathEffect::from_base`].
// Port of: src/core/SkPathEffectBase.h#L21-L131 (chrome/m156)
#[doc(alias = "SkPathEffectBase")]
pub trait PathEffectBase: fmt::Debug + Send + Sync + 'static {
    /// Filters the input path. The `ctm` is provided for path effects that can use the
    /// information; the output must always be in the original (input) coordinate system.
    #[doc(alias = "onFilterPath")]
    fn on_filter_path(
        &self,
        dst: &mut PathBuilder,
        src: &Path,
        rec: &mut StrokeRec,
        cull_rect: Option<&Rect>,
        ctm: &Matrix,
    ) -> bool;

    /// Path effects *requiring* a valid CTM should override to return true.
    #[doc(alias = "onNeedsCTM")]
    fn on_needs_ctm(&self) -> bool {
        false
    }

    /// Does applying this effect to `src` yield a set of points? If so, returns them in
    /// `results`.
    #[doc(alias = "onAsPoints")]
    fn on_as_points(
        &self,
        _results: &mut PointData,
        _src: &Path,
        _rec: &StrokeRec,
        _ctm: &Matrix,
        _cull_rect: Option<&Rect>,
    ) -> bool {
        false
    }

    /// If the effect can be represented as a dash pattern, returns it.
    #[doc(alias = "asADash")]
    fn as_a_dash(&self) -> Option<DashInfo> {
        None
    }

    /// Computes a conservative bounds for its effect, given the bounds of the path. `bounds` is
    /// both the input and output; if false is returned, fast bounds could not be calculated and
    /// `bounds` is undefined. If `bounds` is `None`, performs a dry-run determining if bounds
    /// could be computed.
    #[doc(alias = "computeFastBounds")]
    fn compute_fast_bounds(&self, bounds: Option<&mut Rect>) -> bool;
}

/// The base of objects that affect the geometry of a drawing primitive (`SkPathEffect`).
/// Cloning is cheap (a reference-count bump).
///
/// Equality is identity, as Skia compares `sk_sp`s ([`PathEffect::ptr_eq`]).
// Port of: include/core/SkPathEffect.h#L28-L82 (chrome/m156)
#[doc(alias = "SkPathEffect")]
#[derive(Clone, Debug)]
pub struct PathEffect(Arc<dyn PathEffectBase>);

impl PartialEq for PathEffect {
    /// Identity, as Skia's `sk_sp<SkPathEffect>` `operator==`.
    fn eq(&self, other: &PathEffect) -> bool {
        self.ptr_eq(other)
    }
}

impl PathEffect {
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

    /// Wraps an implementation of [`PathEffectBase`].
    #[must_use]
    pub fn from_base(effect: impl PathEffectBase) -> Self {
        Self(Arc::new(effect))
    }

    /// Returns a path effect that applies each effect (`first` and `second`) to the original
    /// path, and returns a path with the sum of these.
    ///
    /// `result = first(path) + second(path)`
    // Port of: src/core/SkPathEffect.cpp#L180-L182 (chrome/m156)
    #[doc(alias = "MakeSum")]
    #[must_use]
    pub fn sum(first: PathEffect, second: PathEffect) -> Self {
        Self::from_base(SumPathEffect {
            pe0: first,
            pe1: second,
        })
    }

    /// Returns a path effect that applies the inner effect to the path, and then applies the
    /// outer effect to the result of the inner's.
    ///
    /// `result = outer(inner(path))`
    // Port of: src/core/SkPathEffect.cpp#L184-L187 (chrome/m156)
    #[doc(alias = "MakeCompose")]
    #[must_use]
    pub fn compose(outer: PathEffect, inner: PathEffect) -> Self {
        Self::from_base(ComposePathEffect {
            pe0: outer,
            pe1: inner,
        })
    }

    /// Given a `src` path (input) and a stroke-rec (input and output), applies this effect to
    /// the `src` path, returning the new path and stroke-rec. If this effect cannot be applied,
    /// returns `None`.
    ///
    /// The effect can treat the stroke-rec as input only, or it can choose to change it as well
    /// (width, join, style). If this returns `Some`, the caller applies (as needed) the
    /// resulting stroke-rec to the path and then draws.
    // Port of: src/core/SkPathEffect.cpp#L23-L25 (chrome/m156)
    #[doc(alias = "filterPath")]
    #[must_use]
    pub fn filter_path<'a>(
        &self,
        src: &Path,
        stroke_rec: &StrokeRec,
        cull_rect: impl Into<Option<&'a Rect>>,
    ) -> Option<(PathBuilder, StrokeRec)> {
        let mut dst = PathBuilder::new();
        let mut stroke_rec_r = *stroke_rec;
        self.filter_path_inplace(&mut dst, src, &mut stroke_rec_r, cull_rect)
            .then_some((dst, stroke_rec_r))
    }

    /// Applies this effect to the `src` path, returning the new path in `dst`. See
    /// [`PathEffect::filter_path`] for the contract.
    // Port of: src/core/SkPathEffect.cpp#L19-L21 (chrome/m156)
    #[doc(alias = "filterPath")]
    pub fn filter_path_inplace<'a>(
        &self,
        dst: &mut PathBuilder,
        src: &Path,
        stroke_rec: &mut StrokeRec,
        cull_rect: impl Into<Option<&'a Rect>>,
    ) -> bool {
        self.filter_path_inplace_with_matrix(dst, src, stroke_rec, cull_rect, Matrix::i())
    }

    /// Like [`PathEffect::filter_path_inplace`], with the current transformation matrix.
    // Port of: src/core/SkPathEffect.cpp#L23-L25 (chrome/m156)
    #[doc(alias = "filterPath")]
    pub fn filter_path_inplace_with_matrix<'a>(
        &self,
        dst: &mut PathBuilder,
        src: &Path,
        stroke_rec: &mut StrokeRec,
        cull_rect: impl Into<Option<&'a Rect>>,
        ctm: &Matrix,
    ) -> bool {
        self.0
            .on_filter_path(dst, src, stroke_rec, cull_rect.into(), ctm)
    }

    /// True if this path effect requires a valid CTM.
    // Port of: src/core/SkPathEffect.cpp#L33-L35 (chrome/m156)
    #[doc(alias = "needsCTM")]
    #[must_use]
    pub fn needs_ctm(&self) -> bool {
        self.0.on_needs_ctm()
    }

    /// Does applying this effect to `src` yield a set of points? If so, returns them in
    /// `results`.
    ///
    /// skia-rust: `skia-safe` does not expose `SkPathEffectBase::asPoints`.
    // Port of: src/core/SkPathEffect.cpp#L27-L31 (chrome/m156)
    #[doc(alias = "asPoints")]
    pub fn as_points(
        &self,
        results: &mut PointData,
        src: &Path,
        rec: &StrokeRec,
        mx: &Matrix,
        cull_rect: Option<&Rect>,
    ) -> bool {
        self.0.on_as_points(results, src, rec, mx, cull_rect)
    }

    /// If the effect can be represented as a dash pattern, returns it.
    ///
    /// skia-rust: `skia-safe` does not expose `SkPathEffectBase::asADash`.
    #[doc(alias = "asADash")]
    #[must_use]
    pub fn as_a_dash(&self) -> Option<DashInfo> {
        self.0.as_a_dash()
    }

    /// Computes a conservative bounds for this effect, given the bounds of the path; see
    /// [`PathEffectBase::compute_fast_bounds`].
    ///
    /// skia-rust: `skia-safe` does not expose `SkPathEffectBase::computeFastBounds`.
    #[doc(alias = "computeFastBounds")]
    #[must_use]
    pub fn compute_fast_bounds(&self, bounds: Option<&mut Rect>) -> bool {
        self.0.compute_fast_bounds(bounds)
    }
}

// Port of: src/core/SkPathEffect.cpp#L111-L148 (chrome/m156)
#[derive(Debug)]
struct ComposePathEffect {
    // outer
    pe0: PathEffect,
    // inner
    pe1: PathEffect,
}

impl PathEffectBase for ComposePathEffect {
    // Port of: src/core/SkPathEffect.cpp#L125-L136 (chrome/m156)
    fn on_filter_path(
        &self,
        builder: &mut PathBuilder,
        src: &Path,
        rec: &mut StrokeRec,
        cull_rect: Option<&Rect>,
        ctm: &Matrix,
    ) -> bool {
        let mut tmp = Path::default();
        let mut use_tmp = false;

        if self
            .pe1
            .filter_path_inplace_with_matrix(builder, src, rec, cull_rect, ctm)
        {
            tmp = builder.detach();
            use_tmp = true;
        }
        let ptr = if use_tmp { &tmp } else { src };
        self.pe0
            .filter_path_inplace_with_matrix(builder, ptr, rec, cull_rect, ctm)
    }

    // Port of: src/core/SkPathEffect.cpp#L140-L144 (chrome/m156)
    fn compute_fast_bounds(&self, mut bounds: Option<&mut Rect>) -> bool {
        // inner (fPE1) is computed first, automatically updating bounds before computing outer.
        self.pe1.compute_fast_bounds(bounds.as_deref_mut()) && self.pe0.compute_fast_bounds(bounds)
    }
}

// Port of: src/core/SkPathEffect.cpp#L156-L196 (chrome/m156)
#[derive(Debug)]
struct SumPathEffect {
    pe0: PathEffect,
    pe1: PathEffect,
}

impl PathEffectBase for SumPathEffect {
    // Port of: src/core/SkPathEffect.cpp#L174-L180 (chrome/m156)
    fn on_filter_path(
        &self,
        builder: &mut PathBuilder,
        src: &Path,
        rec: &mut StrokeRec,
        cull_rect: Option<&Rect>,
        ctm: &Matrix,
    ) -> bool {
        // always call both, even if the first one succeeds
        let filtered_first = self
            .pe0
            .filter_path_inplace_with_matrix(builder, src, rec, cull_rect, ctm);
        let filtered_second = self
            .pe1
            .filter_path_inplace_with_matrix(builder, src, rec, cull_rect, ctm);
        filtered_first || filtered_second
    }

    // Port of: src/core/SkPathEffect.cpp#L184-L188 (chrome/m156)
    fn compute_fast_bounds(&self, mut bounds: Option<&mut Rect>) -> bool {
        // Unlike Compose(), PE0 modifies the path first for Sum
        self.pe0.compute_fast_bounds(bounds.as_deref_mut()) && self.pe1.compute_fast_bounds(bounds)
    }
}
