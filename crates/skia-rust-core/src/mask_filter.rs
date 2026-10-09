// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkMaskFilter.h, src/core/SkMaskFilter.cpp,
// src/core/SkMaskFilterBase.h, src/core/SkMaskFilterBase.cpp

//! `SkMaskFilter`: filters applied to the coverage mask of a draw (e.g. blurs).
//!
//! The [`MaskFilterBase`] trait is `SkMaskFilterBase`'s virtual interface. Its non-virtual
//! drawing helpers (`filterPath`, `filterRects`, `filterRRect`, which are friends of
//! `skcpu::Draw`) live in `skia_rust_raster::mask_filter_base`, since they need blitters and
//! clips.
//!
//! skia-rust:
//! * `SkResourceCache* cache` parameters (`filterRectsToNine`, `filterRRectToNine`) are not
//!   ported: the cache only memoizes the nine-patch masks, so results are identical without it.
//! * `asImageFilter` is not ported yet (it needs the blur image filter, Phase 3).
//! * Flattening is the `type_name` and `flatten` methods of [`MaskFilterBase`], read back through
//!   [`FlattenableRegistry`](crate::flattenable::FlattenableRegistry); `Deserialize` is not
//!   ported.
//! * `filterRectsToNine`/`filterRRectToNine` draw small masks with `skcpu::Draw`, which lives in
//!   the raster crate. They take a [`MaskRasterizer`], the part of `skcpu::Draw` they use, which
//!   the raster crate implements.

use core::any::Any;
use core::fmt;
use std::sync::Arc;

use crate::blur_mask_filter_impl::BlurMaskFilterImpl;
use crate::blur_types::BlurStyle;
use crate::data::Data;
use crate::flattenable::FlattenableRegistry;
use crate::mask::{Mask, MaskBuilder, MaskFormat};
use crate::matrix::Matrix;
use crate::point::IPoint;
use crate::read_buffer::ReadBuffer;
use crate::rect::{IRect, Rect, RoundOut};
use crate::rrect::RRect;
use crate::scalar::scalar;
use crate::write_buffer::BinaryWriteBuffer;

/// What kind of mask filter a [`MaskFilterBase`] is (`SkMaskFilterBase::Type`).
// Port of: src/core/SkMaskFilterBase.h#L63-L69 (chrome/m156)
#[doc(alias = "SkMaskFilterBase::Type")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MaskFilterType {
    /// `kBlur`.
    Blur,
    /// `kEmboss`.
    Emboss,
    /// `kSDF`.
    Sdf,
    /// `kShader`.
    Shader,
    /// `kTable`.
    Table,
}

/// The sigma and style of a blur (`SkMaskFilterBase::BlurRec`).
// Port of: src/core/SkMaskFilterBase.h#L84-L87 (chrome/m156)
#[doc(alias = "SkMaskFilterBase::BlurRec")]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct BlurRec {
    /// `fSigma`.
    pub sigma: scalar,
    /// `fStyle`.
    pub style: BlurStyle,
}

/// What `filterRects` decided (`SkMaskFilterBase::FilterReturn`).
// Port of: src/core/SkMaskFilterBase.h#L98-L102 (chrome/m156)
#[doc(alias = "SkMaskFilterBase::FilterReturn")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum FilterReturn {
    /// `kFalse`.
    False,
    /// `kTrue`.
    True,
    /// `kUnimplemented`.
    Unimplemented,
}

/// A mask to be stretched over `outer_rect` as a nine-patch (`SkMaskFilterBase::NinePatch`).
///
/// skia-rust: the mask owns its image (Skia's refers to cached data).
// Port of: src/core/SkMaskFilterBase.h#L104-L118 (chrome/m156)
#[doc(alias = "SkMaskFilterBase::NinePatch")]
#[derive(Clone, Debug)]
pub struct NinePatch {
    /// `fMask`; its bounds must have `[0, 0]` as top-left.
    pub mask: MaskBuilder,
    /// `fOuterRect`; its width/height must be at least those of the mask's bounds.
    pub outer_rect: IRect,
    /// `fCenter`: identifies the center row/col for stretching.
    pub center: IPoint,
}

/// The part of `skcpu::Draw` that mask filters use to draw their small nine-patch masks:
/// anti-aliased black geometry drawn into a zeroed A8 mask.
///
/// skia-rust: this is how `SkBlurMaskFilterImpl`'s `draw_into_mask` reaches `skcpu::Draw`, which
/// lives in the raster crate (see the module documentation).
pub trait MaskRasterizer {
    /// Draws `rects` into the prepared A8 `mask` (the `proc` of `draw_rects_into_mask`): one
    /// rect is filled, two are filled even-odd as one path. The mask's pixels are the device,
    /// with the mask's bounds translated to the origin.
    fn draw_rects(&self, mask: &mut MaskBuilder, rects: &[Rect]);

    /// Draws `rrect` into the prepared A8 `mask` (the `proc` of `draw_rrect_into_mask`).
    fn draw_rrect(&self, mask: &mut MaskBuilder, rrect: &RRect);
}

/// The virtual interface of a mask filter (`SkMaskFilterBase`).
// Port of: src/core/SkMaskFilterBase.h#L38-L206 (chrome/m156)
#[doc(alias = "SkMaskFilterBase")]
pub trait MaskFilterBase: Any + fmt::Debug + Send + Sync {
    /// The bounds of the filtered mask of geometry with bounds `src`, conservatively
    /// (`computeFastBounds(src, dest)`).
    ///
    /// The fast bounds function is used to enable the paint to be culled early in the drawing
    /// pipeline. The default calls [`Self::filter_mask`] with a source mask having no image,
    /// but subclasses may override this if they can compute the rect faster.
    // Port of: src/core/SkMaskFilterBase.cpp#L218-L228 (chrome/m156)
    #[doc(alias = "computeFastBounds")]
    fn compute_fast_bounds(&self, src: &Rect) -> Rect {
        let src_m = Mask::new(&[], src.round_out(), 0, MaskFormat::A8);
        let mut dst_m = MaskBuilder::default();

        let mut margin = IPoint::new(0, 0); // ignored
        if self.filter_mask(&mut dst_m, &src_m, Matrix::i(), Some(&mut margin)) {
            Rect::from_irect(dst_m.bounds)
        } else {
            Rect::from_irect(src_m.bounds)
        }
    }

    /// The format of the masks the filter produces (`getFormat`).
    ///
    /// skia-rust: Skia's is pure virtual; the default is [`MaskFormat::A8`], what every mask
    /// filter but the 3D emboss one produces.
    #[doc(alias = "getFormat")]
    fn format(&self) -> MaskFormat {
        MaskFormat::A8
    }

    /// Creates a new mask by filtering the `src` mask under `ctm`.
    ///
    /// If `src` has no image, `dst` does not get an image either but its other fields are
    /// filled out. If `margin` is given, it is set to the buffer dx/dy needed when calculating
    /// the effect: used when drawing a clipped object to know how much larger to allocate the
    /// source before applying the filter. Returns true if `dst` was correctly created.
    ///
    /// skia-rust: Skia's is pure virtual; the default does nothing and returns false, which is
    /// what an unsupported source or matrix gets.
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

    /// What kind of mask filter this is (`type`).
    #[doc(alias = "type")]
    fn filter_type(&self) -> MaskFilterType;

    /// The name the filter is flattened under (`getTypeName`), which the registry maps back to
    /// its factory. The empty name, the default, marks a filter that cannot be flattened.
    #[doc(alias = "getTypeName")]
    fn type_name(&self) -> &'static str {
        ""
    }

    /// Writes the parameters of the filter (`flatten`). Writes nothing by default.
    fn flatten(&self, _buffer: &mut BinaryWriteBuffer) {}

    /// If this filter can be represented by a [`BlurRec`], returns it (`asABlur`).
    // Port of: src/core/SkMaskFilterBase.cpp#L47-L49 (chrome/m156)
    #[doc(alias = "asABlur")]
    fn as_a_blur(&self) -> Option<BlurRec> {
        None
    }

    /// As an optimization, some filters can be applied to a smaller nine-patch instead of the
    /// full-sized rectangle. These nine-patches are not only smaller, but more
    /// re-usable/cacheable. Then, when drawing/blitting, the nine-patch can be expanded to the
    /// desired size.
    ///
    /// Override if your subclass can filter a rect, and return the answer as a nine-patch mask
    /// to be stretched over the returned outer rect. On success set `patch` and return
    /// [`FilterReturn::True`]. On failure (e.g. out of memory) return [`FilterReturn::False`].
    /// If the normal [`Self::filter_mask`] entry-point should be called (the default) return
    /// [`FilterReturn::Unimplemented`].
    ///
    /// By convention, the caller will take the center row/col from the returned mask as the
    /// slice it can replicate horizontally and vertically as we stretch the mask to fit inside
    /// the outer rect.
    // Port of: src/core/SkMaskFilterBase.cpp#L208-L215 (chrome/m156)
    #[doc(alias = "filterRectsToNine")]
    fn filter_rects_to_nine(
        &self,
        _rects: &[Rect],
        _ctm: &Matrix,
        _clip_bounds: &IRect,
        _patch: &mut Option<NinePatch>,
        _rasterizer: &dyn MaskRasterizer,
    ) -> FilterReturn {
        FilterReturn::Unimplemented
    }

    /// Similar to [`Self::filter_rects_to_nine`], except it performs the work on a round rect.
    // Port of: src/core/SkMaskFilterBase.cpp#L203-L206 (chrome/m156)
    #[doc(alias = "filterRRectToNine")]
    fn filter_rrect_to_nine(
        &self,
        _rrect: &RRect,
        _ctm: &Matrix,
        _clip_bounds: &IRect,
        _rasterizer: &dyn MaskRasterizer,
    ) -> Option<NinePatch> {
        None
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

    /// The flattened filter, as `SkFlattenable::serialize` makes it: its name, then its body.
    // Port of: src/core/SkFlattenable.cpp#L127-L139 (chrome/m156)
    #[must_use]
    pub fn serialize(&self) -> Data {
        let mut writer = BinaryWriteBuffer::new();
        writer.write_mask_filter(Some(self));
        writer.snapshot_as_data()
    }

    /// Writes the flattened filter into `memory`, and returns its size, or 0 if it does not fit
    /// (the `serialize(void*, size_t)` overload).
    // Port of: src/core/SkFlattenable.cpp#L141-L150 (chrome/m156)
    pub fn serialize_into(&self, memory: &mut [u8]) -> usize {
        let mut writer = BinaryWriteBuffer::new();
        writer.write_mask_filter(Some(self));
        let size = writer.bytes_written();
        if size > memory.len() {
            return 0;
        }
        writer.write_to_memory(&mut memory[..size]);
        size
    }

    /// Reads back a filter written by [`MaskFilter::serialize`], with the factories of
    /// `registry` (`SkFlattenable::Deserialize`).
    // Port of: src/core/SkFlattenable.cpp#L152-L159 (chrome/m156)
    #[doc(alias = "Deserialize")]
    #[must_use]
    pub fn deserialize(data: &[u8], registry: &FlattenableRegistry) -> Option<MaskFilter> {
        ReadBuffer::new(data).read_mask_filter(registry)
    }

    /// Creates a blur mask filter.
    ///
    /// * `style` - the [`BlurStyle`] to use.
    /// * `sigma` - standard deviation of the Gaussian blur to apply. Must be > 0.
    /// * `respect_ctm` - if true (the default) the blur's sigma is modified by the CTM.
    ///
    /// Returns `None` if `sigma` is not finite and positive.
    // Port of: src/core/SkBlurMaskFilterImpl.cpp#L619-L625 (chrome/m156)
    #[doc(alias = "MakeBlur")]
    #[must_use]
    pub fn blur(
        style: BlurStyle,
        sigma: scalar,
        respect_ctm: impl Into<Option<bool>>,
    ) -> Option<MaskFilter> {
        if sigma.is_finite() && sigma > 0.0 {
            Some(MaskFilter::from_base(BlurMaskFilterImpl::new(
                sigma,
                style,
                respect_ctm.into().unwrap_or(true),
            )))
        } else {
            None
        }
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
