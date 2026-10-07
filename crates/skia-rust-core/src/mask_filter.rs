// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkMaskFilter.h, src/core/SkMaskFilterBase.h

//! `SkMaskFilter`: filters applied to the coverage mask of a draw (e.g. blurs).
//!
//! skia-rust: a stub (D2, extended by D4). Only what `SkPaint` and `SkBlitter::Choose` need is
//! here: the [`MaskFilter`] handle, [`MaskFilterBase::compute_fast_bounds`] and
//! [`MaskFilterBase::format`]. The mask filters (`SkMaskFilter::MakeBlur`, ...) and the rest of
//! `SkMaskFilterBase` (`asABlur`, `filterPath`, `filterRects`, ...) are Phase 3 and extend the
//! trait; D5 added [`MaskFilterBase::filter_mask`], which `SkDraw::drawDevMask` and `DrawToMask`
//! call.

use core::any::Any;
use core::fmt;
use std::sync::Arc;

use crate::mask::{Mask, MaskBuilder, MaskFormat};
use crate::matrix::Matrix;
use crate::point::IPoint;
use crate::rect::Rect;

/// The virtual interface of a mask filter (`SkMaskFilterBase`), reduced to what is ported.
// Port of: src/core/SkMaskFilterBase.h#L40-L206 (chrome/m156)
#[doc(alias = "SkMaskFilterBase")]
pub trait MaskFilterBase: Any + fmt::Debug + Send + Sync {
    /// The bounds of the filtered mask of geometry with bounds `src`, conservatively
    /// (`computeFastBounds(src, dest)`).
    #[doc(alias = "computeFastBounds")]
    fn compute_fast_bounds(&self, src: &Rect) -> Rect;

    /// The format of the masks the filter produces (`getFormat`).
    ///
    /// skia-rust: Skia's is pure virtual; the default is [`MaskFormat::A8`], what every mask
    /// filter but the 3D emboss one produces.
    #[doc(alias = "getFormat")]
    fn format(&self) -> MaskFormat {
        MaskFormat::A8
    }

    /// Filters `src` into `dst` under `ctm` and returns true if it did. If `margin` is given and
    /// `src` has no image (a bounds query), the filter sets it to how far the result extends
    /// beyond the source on each side (`filterMask`).
    ///
    /// skia-rust: Skia's is pure virtual; the default does nothing and returns false, which is
    /// what an unsupported source or matrix gets, until Phase 3's mask filters implement it.
    // Port of: src/core/SkMaskFilterBase.h#L60-L61 (chrome/m156)
    #[doc(alias = "filterMask")]
    fn filter_mask(
        &self,
        _dst: &mut MaskBuilder,
        _src: &Mask<'_>,
        _ctm: &Matrix,
        _margin: Option<&mut IPoint>,
    ) -> bool {
        false
    }
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
