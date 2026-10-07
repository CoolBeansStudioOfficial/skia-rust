// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkImageFilter.h, src/core/SkImageFilter_Base.h

//! `SkImageFilter`: filters applied to the rendered result of a draw.
//!
//! skia-rust: a stub (D2). Only what `SkPaint` needs is here: the [`ImageFilter`] handle,
//! `computeFastBounds` and `canComputeFastBounds`. The image filters and the rest of
//! `SkImageFilter_Base` are Phase 3 and extend the trait.

use core::any::Any;
use core::fmt;
use std::sync::Arc;

use crate::rect::Rect;

/// The virtual interface of an image filter (`SkImageFilter_Base`), reduced to what is ported.
// Port of: src/core/SkImageFilter_Base.h#L23-L308 (chrome/m156)
#[doc(alias = "SkImageFilter_Base")]
pub trait ImageFilterBase: Any + fmt::Debug + Send + Sync {
    /// The bounds of the filtered result of geometry with bounds `bounds`, conservatively
    /// (`computeFastBounds`; Skia's default is the union of the inputs' bounds).
    #[doc(alias = "computeFastBounds")]
    fn compute_fast_bounds(&self, bounds: &Rect) -> Rect;

    /// True if filtering transparent black can produce something else
    /// (`affectsTransparentBlack`), in which case the filter's bounds are unbounded.
    #[doc(alias = "affectsTransparentBlack")]
    fn affects_transparent_black(&self) -> bool;
}

/// A shared image filter (`sk_sp<SkImageFilter>`): a cheaply clonable handle to an
/// [`ImageFilterBase`].
///
/// Equality is identity, as Skia compares `sk_sp`s ([`ImageFilter::ptr_eq`]).
// Port of: include/core/SkImageFilter.h#L35-L117 (chrome/m156)
#[doc(alias = "SkImageFilter")]
#[derive(Clone)]
pub struct ImageFilter(Arc<dyn ImageFilterBase>);

impl ImageFilter {
    /// Wraps an image filter implementation.
    #[must_use]
    pub fn from_base(base: impl ImageFilterBase) -> ImageFilter {
        ImageFilter(Arc::new(base))
    }

    /// The implementation (`as_IFB`).
    #[doc(alias = "as_IFB")]
    #[must_use]
    pub fn as_base(&self) -> &dyn ImageFilterBase {
        &*self.0
    }

    /// True if `self` and `other` are the same filter (Skia's `sk_sp` comparison).
    #[must_use]
    pub fn ptr_eq(&self, other: &ImageFilter) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// The bounds of the filtered result of geometry with bounds `bounds`
    /// (`computeFastBounds`).
    #[doc(alias = "computeFastBounds")]
    #[must_use]
    pub fn compute_fast_bounds(&self, bounds: impl AsRef<Rect>) -> Rect {
        self.0.compute_fast_bounds(bounds.as_ref())
    }

    /// Can this filter DAG compute the resulting bounds of an object-space rectangle
    /// (`canComputeFastBounds`)?
    // Port of: src/core/SkImageFilter.cpp#L98-L100 (chrome/m156)
    #[doc(alias = "canComputeFastBounds")]
    #[must_use]
    pub fn can_compute_fast_bounds(&self) -> bool {
        !self.0.affects_transparent_black()
    }
}

impl PartialEq for ImageFilter {
    /// Identity, as Skia's `sk_sp<SkImageFilter>` `operator==`.
    fn eq(&self, other: &ImageFilter) -> bool {
        self.ptr_eq(other)
    }
}

impl fmt::Debug for ImageFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ImageFilter").field(&self.0).finish()
    }
}
