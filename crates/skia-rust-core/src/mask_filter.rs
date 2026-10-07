// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkMaskFilter.h, src/core/SkMaskFilterBase.h

//! `SkMaskFilter`: filters applied to the coverage mask of a draw (e.g. blurs).
//!
//! skia-rust: a stub (D2). Only what `SkPaint` needs is here: the [`MaskFilter`] handle and
//! [`MaskFilterBase::compute_fast_bounds`]. The mask filters (`SkMaskFilter::MakeBlur`, ...) and
//! the rest of `SkMaskFilterBase` (`filterMask`, `getFormat`, `asABlur`, ...) are Phase 3 and
//! extend the trait.

use core::any::Any;
use core::fmt;
use std::sync::Arc;

use crate::rect::Rect;

/// The virtual interface of a mask filter (`SkMaskFilterBase`), reduced to what is ported.
// Port of: src/core/SkMaskFilterBase.h#L40-L206 (chrome/m156)
#[doc(alias = "SkMaskFilterBase")]
pub trait MaskFilterBase: Any + fmt::Debug + Send + Sync {
    /// The bounds of the filtered mask of geometry with bounds `src`, conservatively
    /// (`computeFastBounds(src, dest)`).
    #[doc(alias = "computeFastBounds")]
    fn compute_fast_bounds(&self, src: &Rect) -> Rect;
}

/// A shared mask filter (`sk_sp<SkMaskFilter>`): a cheaply clonable handle to a
/// [`MaskFilterBase`].
///
/// Equality is identity, as Skia compares `sk_sp`s ([`MaskFilter::ptr_eq`]).
// Port of: include/core/SkMaskFilter.h#L26-L43 (chrome/m156)
#[doc(alias = "SkMaskFilter")]
#[derive(Clone)]
pub struct MaskFilter(Arc<dyn MaskFilterBase>);

impl MaskFilter {
    /// Wraps a mask filter implementation.
    #[must_use]
    pub fn from_base(base: impl MaskFilterBase) -> MaskFilter {
        MaskFilter(Arc::new(base))
    }

    /// The implementation (`as_MFB`).
    #[doc(alias = "as_MFB")]
    #[must_use]
    pub fn as_base(&self) -> &dyn MaskFilterBase {
        &*self.0
    }

    /// True if `self` and `other` are the same filter (Skia's `sk_sp` comparison).
    #[must_use]
    pub fn ptr_eq(&self, other: &MaskFilter) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl PartialEq for MaskFilter {
    /// Identity, as Skia's `sk_sp<SkMaskFilter>` `operator==`.
    fn eq(&self, other: &MaskFilter) -> bool {
        self.ptr_eq(other)
    }
}

impl fmt::Debug for MaskFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MaskFilter").field(&self.0).finish()
    }
}
