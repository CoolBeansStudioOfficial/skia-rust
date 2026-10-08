// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkBlurMaskFilterImpl.h, src/core/SkBlurMaskFilterImpl.cpp

//! `SkBlurMaskFilterImpl`: the blur mask filter ([`MaskFilter::blur`]).
//!
//! skia-rust: the nine-patch masks are not cached (`SkMaskCache`/`SkResourceCache`), and the
//! small masks are drawn through a [`MaskRasterizer`] (see [`crate::mask_filter`]). `asImageFilter`
//! (which needs the blur image filter) and flattening are not ported yet.

use crate::align::align4;
use crate::blur_mask::BlurMask;
use crate::blur_types::BlurStyle;
use crate::m44::V2;
use crate::mask::{AllocType, CreateMode, Mask, MaskBuilder, MaskFormat};
use crate::mask_filter::{
    BlurRec, FilterReturn, MaskFilterBase, MaskFilterType, MaskRasterizer, NinePatch,
};
use crate::matrix::Matrix;
use crate::point::IPoint;
use crate::rect::{IRect, Rect, RoundOut};
use crate::rrect::{Corner, RRect, Type as RRectType};
use crate::scalar::{int_to_scalar, scalar, scalar_ceil_to_int};

// Port of: src/core/SkBlurMaskFilterImpl.cpp#L49 (chrome/m156)
const MAX_BLUR_DEVICE_SIGMA: scalar = 128.0;

/// The blur mask filter.
// Port of: src/core/SkBlurMaskFilterImpl.h#L35-L87 (chrome/m156)
#[doc(alias = "SkBlurMaskFilterImpl")]
#[derive(Clone, Debug)]
pub struct BlurMaskFilterImpl {
    sigma: scalar,
    blur_style: BlurStyle,
    respect_ctm: bool,
}

impl BlurMaskFilterImpl {
    /// A blur of standard deviation `sigma` (which must be positive).
    // Port of: src/core/SkBlurMaskFilterImpl.cpp#L51-L58 (chrome/m156)
    #[must_use]
    pub fn new(sigma: scalar, style: BlurStyle, respect_ctm: bool) -> BlurMaskFilterImpl {
        debug_assert!(sigma > 0.0);
        BlurMaskFilterImpl {
            sigma,
            blur_style: style,
            respect_ctm,
        }
    }

    /// The device-space sigma under `ctm` (`computeXformedDeviceSigma`).
    // Port of: src/core/SkBlurMaskFilterImpl.cpp#L106-L111 (chrome/m156)
    #[doc(alias = "computeXformedDeviceSigma")]
    #[must_use]
    pub fn compute_xformed_device_sigma(&self, ctm: &Matrix) -> scalar {
        // If we don't do the matrix transform, consider our local sigma as our device sigma for
        // algorithms which rely on device sigma.
        let xformed_sigma = if self.ignore_xform() {
            self.sigma
        } else {
            ctm.map_radius(self.sigma)
        };
        // std::min(a, b) is `(b < a) ? b : a`.
        if MAX_BLUR_DEVICE_SIGMA < xformed_sigma {
            MAX_BLUR_DEVICE_SIGMA
        } else {
            xformed_sigma
        }
    }

    /// The local-space sigma under `ctm` (`computeXformedLocalSigma`); zero if the matrix
    /// cannot be decomposed into a scale.
    // Port of: src/core/SkBlurMaskFilterImpl.cpp#L113-L132 (chrome/m156)
    #[doc(alias = "computeXformedLocalSigma")]
    #[must_use]
    pub fn compute_xformed_local_sigma(&self, ctm: &Matrix) -> V2 {
        let mut local_sigma = V2 { x: 0.0, y: 0.0 };
        if let Some(scale) = ctm.decompose_scale(None) {
            // std::min(a, b) is `(b < a) ? b : a`.
            let min = |a: scalar, b: scalar| if b < a { b } else { a };
            if self.ignore_xform() {
                // In the ignoreXform case, fSigma would normally be interpreted as device-space
                // sigma, so we clamp it and then transform it to a local-space sigma.
                let clamped_sigma = min(self.sigma, MAX_BLUR_DEVICE_SIGMA);
                local_sigma.x = clamped_sigma / scale.width;
                local_sigma.y = clamped_sigma / scale.height;
            } else {
                // Clamp fSigma which is interpreted as our local-space sigma, then clamp it
                // against the equivalent max local-space sigma.
                local_sigma.x = min(self.sigma, MAX_BLUR_DEVICE_SIGMA / scale.width);
                local_sigma.y = min(self.sigma, MAX_BLUR_DEVICE_SIGMA / scale.height);
            }
        }
        local_sigma
    }

    /// The blur style (`blurStyle`).
    #[doc(alias = "blurStyle")]
    #[must_use]
    pub fn blur_style(&self) -> BlurStyle {
        self.blur_style
    }

    /// The sigma (`sigma`).
    #[must_use]
    pub fn sigma(&self) -> scalar {
        self.sigma
    }

    /// True if the blur ignores the CTM (`ignoreXform`).
    #[doc(alias = "ignoreXform")]
    #[must_use]
    pub fn ignore_xform(&self) -> bool {
        !self.respect_ctm
    }

    // Port of: src/core/SkBlurMaskFilterImpl.cpp#L143-L151 (chrome/m156)
    fn filter_rect_mask(
        &self,
        dst: &mut MaskBuilder,
        r: &Rect,
        matrix: &Matrix,
        margin: Option<&mut IPoint>,
        create_mode: CreateMode,
    ) -> bool {
        let sigma = self.compute_xformed_device_sigma(matrix);

        BlurMask::blur_rect(sigma, dst, r, self.blur_style, margin, create_mode)
    }
}

// Port of: src/core/SkBlurMaskFilterImpl.cpp#L153-L170 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // align4 of an i32 width fits in u32
fn prepare_to_draw_into_mask(bounds: &Rect, mask: &mut MaskBuilder) -> bool {
    mask.bounds = bounds.round_out();
    #[allow(clippy::cast_sign_loss)] // the bounds of a rect are not inverted
    {
        mask.row_bytes = align4(mask.bounds.width() as usize) as u32;
    }
    mask.format = MaskFormat::A8;
    let size = mask.compute_image_size();
    if size == 0 {
        return false;
    }
    mask.image = MaskBuilder::alloc_image(size, AllocType::ZeroInit);
    true
}

// Port of: src/core/SkBlurMaskFilterImpl.cpp#L172-L202 (chrome/m156)
fn draw_rects_into_mask(
    rects: &[Rect],
    mask: &mut MaskBuilder,
    rasterizer: &dyn MaskRasterizer,
) -> bool {
    debug_assert!(rects.len() == 1 || rects.len() == 2);
    if !prepare_to_draw_into_mask(&rects[0], mask) {
        return false;
    }
    rasterizer.draw_rects(mask, rects);
    true
}

// Port of: src/core/SkBlurMaskFilterImpl.cpp#L204-L208 (chrome/m156)
fn draw_rrect_into_mask(
    rrect: &RRect,
    mask: &mut MaskBuilder,
    rasterizer: &dyn MaskRasterizer,
) -> bool {
    if !prepare_to_draw_into_mask(rrect.rect(), mask) {
        return false;
    }
    rasterizer.draw_rrect(mask, rrect);
    true
}

// Port of: src/core/SkBlurMaskFilterImpl.cpp#L210-L213 (chrome/m156)
fn rect_exceeds(r: &Rect, v: scalar) -> bool {
    r.left < -v || r.top < -v || r.right > v || r.bottom > v || r.width() > v || r.height() > v
}

// The patch for a filtered mask: its bounds are moved to (0, 0).
fn make_patch(mut mask: MaskBuilder, outer_rect: IRect, center: IPoint) -> NinePatch {
    // The bounds of the blurred mask are at -margin, so we need to offset it back to 0,0.
    mask.bounds.offset_to((0, 0));
    NinePatch {
        mask,
        outer_rect,
        center,
    }
}

impl MaskFilterBase for BlurMaskFilterImpl {
    // Port of: src/core/SkBlurMaskFilterImpl.cpp#L60-L62 (chrome/m156)
    fn format(&self) -> MaskFormat {
        MaskFormat::A8
    }

    // Port of: src/core/SkBlurMaskFilterImpl.cpp#L134-L141 (chrome/m156)
    fn filter_mask(
        &self,
        dst: &mut MaskBuilder,
        src: &Mask<'_>,
        matrix: &Matrix,
        margin: Option<&mut IPoint>,
    ) -> bool {
        let sigma = self.compute_xformed_device_sigma(matrix);
        BlurMask::box_blur(dst, src, sigma, self.blur_style, margin)
    }

    fn filter_type(&self) -> MaskFilterType {
        MaskFilterType::Blur
    }

    // Port of: src/core/SkBlurMaskFilterImpl.cpp#L64-L74 (chrome/m156)
    fn as_a_blur(&self) -> Option<BlurRec> {
        if self.ignore_xform() {
            return None;
        }

        Some(BlurRec {
            sigma: self.sigma,
            style: self.blur_style,
        })
    }

    // Port of: src/core/SkBlurMaskFilterImpl.cpp#L252-L394 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    fn filter_rrect_to_nine(
        &self,
        rrect: &RRect,
        matrix: &Matrix,
        _clip_bounds: &IRect,
        rasterizer: &dyn MaskRasterizer,
    ) -> Option<NinePatch> {
        match rrect.get_type() {
            // Empty: nothing to draw.
            // Oval: the nine patch special case does not handle ovals, and we
            // already have code for rectangles.
            RRectType::Empty | RRectType::Oval => {
                return None;
            }

            RRectType::Rect => {
                // We should have caught this earlier.
                debug_assert!(false, "Should use a different special case");
                return None;
            }

            // These three can take advantage of this fast path.
            RRectType::Simple | RRectType::NinePatch | RRectType::Complex => {}
        }

        // TODO: report correct metrics for innerstyle, where we do not grow the
        // total bounds, but we do need an inset the size of our blur-radius
        if BlurStyle::Inner == self.blur_style {
            return None;
        }

        // TODO: take clipBounds into account to limit our coordinates up front
        // for now, just skip too-large src rects (to take the old code path).
        if rect_exceeds(rrect.rect(), int_to_scalar(32767)) {
            return None;
        }

        // We first figure out how much we need to expand the mask (the margin) to account
        // for the blurred area.
        let mut margin = IPoint::new(0, 0);
        let src_m = Mask::new(&[], rrect.rect().round_out(), 0, MaskFormat::A8);
        let mut dst_m = MaskBuilder::default();
        if !self.filter_mask(&mut dst_m, &src_m, matrix, Some(&mut margin)) {
            return None;
        }

        // Most of the pixels in the center of the bitmap are the same as their neighbors, so
        // blurring is a waste of compute. If we made a smaller nine-patch rrect, blurred
        // that, we could then expand the result later, saving cycles.
        //
        // To figure out the appropriate width and height of the nine patch rrect, we use the
        // larger radius per side as well as the margin, to account for inner blur.
        let ul = rrect.radii(Corner::UpperLeft);
        let ur = rrect.radii(Corner::UpperRight);
        let lr = rrect.radii(Corner::LowerRight);
        let ll = rrect.radii(Corner::LowerLeft);

        // std::max(a, b) is `(a < b) ? b : a`.
        let max = |a: scalar, b: scalar| if a < b { b } else { a };

        // If there's a fractional radii, round up so that our final rrect is an integer width
        // to allow for symmetrical blurring across the x and y axes.
        let left_unstretched = scalar_ceil_to_int(max(ul.x, ll.x)) + margin.x;
        let right_unstretched = scalar_ceil_to_int(max(ur.x, lr.x)) + margin.x;

        // Extra space in the middle to ensure an unchanging piece for stretching.
        let stretch_size = 1;

        let total_small_width = left_unstretched + right_unstretched + stretch_size;
        if int_to_scalar(total_small_width) >= rrect.rect().width() {
            // There is no valid piece to stretch.
            return None;
        }

        let top_unstretched = scalar_ceil_to_int(max(ul.y, ur.y)) + margin.y;
        let bot_unstretched = scalar_ceil_to_int(max(ll.y, lr.y)) + margin.y;

        let total_small_height = top_unstretched + bot_unstretched + stretch_size;
        if int_to_scalar(total_small_height) >= rrect.rect().height() {
            // There is no valid piece to stretch.
            return None;
        }

        // Now make that scaled down nine patch rrect.
        let small_r = Rect::from_iwh(total_small_width, total_small_height);
        let mut small_rr = RRect::default();
        small_rr.set_rect_radii(small_r, rrect.radii_ref());

        // Blit the small rrect into a buffer.
        let mut src_m = MaskBuilder::default();
        if !draw_rrect_into_mask(&small_rr, &mut src_m, rasterizer) {
            return None;
        }

        // Blur the small rrect. This will expand the mask on all sides by margin to account for
        // outer blur.
        let mut filter_m = MaskBuilder::default();
        if !self.filter_mask(&mut filter_m, &src_m.as_mask(), matrix, None) {
            return None;
        }
        debug_assert_eq!(filter_m.bounds.width(), src_m.bounds.width() + 2 * margin.x);
        debug_assert_eq!(
            filter_m.bounds.height(),
            src_m.bounds.height() + 2 * margin.y
        );

        // The ninepatch could be asymmetrical (e.g. if the left rX are wider than the right), so
        // we must tell the caller where the center stretchy bit is in both directions. We added
        // margin once before to unstretched to account for inner blur, but now we add to account
        // for the outer blur.
        let center = IPoint::new(margin.x + left_unstretched, margin.y + top_unstretched);
        Some(make_patch(filter_m, dst_m.bounds, center))
    }

    // Port of: src/core/SkBlurMaskFilterImpl.cpp#L396-L526 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    fn filter_rects_to_nine(
        &self,
        rects: &[Rect],
        matrix: &Matrix,
        _clip_bounds: &IRect,
        patch: &mut Option<NinePatch>,
        rasterizer: &dyn MaskRasterizer,
    ) -> FilterReturn {
        debug_assert!(rects.len() == 1 || rects.len() == 2);

        // TODO: report correct metrics for innerstyle, where we do not grow the
        // total bounds, but we do need an inset the size of our blur-radius
        if BlurStyle::Inner == self.blur_style || BlurStyle::Outer == self.blur_style {
            return FilterReturn::Unimplemented;
        }

        // TODO: take clipBounds into account to limit our coordinates up front
        // for now, just skip too-large src rects (to take the old code path).
        if rect_exceeds(&rects[0], int_to_scalar(32767)) {
            return FilterReturn::Unimplemented;
        }

        let mut margin = IPoint::new(0, 0);
        let src_m = Mask::new(&[], rects[0].round_out(), 0, MaskFormat::A8);
        let mut dst_m = MaskBuilder::default();

        let filter_result = if rects.len() == 1 {
            // special case for fast rect blur
            // don't actually do the blur the first time, just compute the correct size
            self.filter_rect_mask(
                &mut dst_m,
                &rects[0],
                matrix,
                Some(&mut margin),
                CreateMode::JustComputeBounds,
            )
        } else {
            self.filter_mask(&mut dst_m, &src_m, matrix, Some(&mut margin))
        };

        if !filter_result {
            return FilterReturn::False;
        }

        // smallR is the smallest version of 'rect' that will still guarantee that we get the
        // same blur results on all edges, plus 1 center row/col that is representative of the
        // extendible/stretchable edges of the ninepatch. Since our actual edge may be
        // fractional we inset 1 more to be sure we don't miss any interior blur.
        // x is an added pixel of blur, and { and } are the (fractional) edge pixels from the
        // original rect.
        //
        //   x x { x x .... x x } x x
        //
        // Thus, in this case, we inset by a total of 5 (on each side) beginning with our
        // outer-rect (dstM.fBounds)
        let mut small_r = [Rect::default(); 2];
        let rect_count: usize;
        let mut center = IPoint::new(0, 0);

        // +2 is from +1 for each edge (to account for possible fractional edges
        let mut small_w = dst_m.bounds.width() - src_m.bounds.width() + 2;
        let mut small_h = dst_m.bounds.height() - src_m.bounds.height() + 2;
        let inner_ir: IRect;

        if rects.len() == 1 {
            rect_count = 1;
            inner_ir = src_m.bounds;
            center.set(small_w, small_h);
        } else {
            rect_count = 2;
            inner_ir = rects[1].round_in();
            center.set(
                small_w + (inner_ir.left - src_m.bounds.left),
                small_h + (inner_ir.top - src_m.bounds.top),
            );
        }

        // +1 so we get a clean, stretchable, center row/col
        small_w += 1;
        small_h += 1;

        // we want the inset amounts to be integral, so we don't change any
        // fractional phase on the fRight or fBottom of our smallR.
        let dx = int_to_scalar(inner_ir.width() - small_w);
        let dy = int_to_scalar(inner_ir.height() - small_h);
        if dx < 0.0 || dy < 0.0 {
            // we're too small, relative to our blur, to break into nine-patch,
            // so we ask to have our normal filterMask() be called.
            return FilterReturn::Unimplemented;
        }

        small_r[0].set_ltrb(
            rects[0].left,
            rects[0].top,
            rects[0].right - dx,
            rects[0].bottom - dy,
        );
        if small_r[0].width() < 2.0 || small_r[0].height() < 2.0 {
            return FilterReturn::Unimplemented;
        }
        if rect_count == 2 {
            small_r[1].set_ltrb(
                rects[1].left,
                rects[1].top,
                rects[1].right - dx,
                rects[1].bottom - dy,
            );
            debug_assert!(!small_r[1].is_empty());
        }

        let small_rects = &small_r[..rect_count];
        let mut filter_m = MaskBuilder::default();
        if rect_count == 2 {
            let mut small_src_m = MaskBuilder::default();
            if !draw_rects_into_mask(small_rects, &mut small_src_m, rasterizer) {
                return FilterReturn::False;
            }

            if !self.filter_mask(&mut filter_m, &small_src_m.as_mask(), matrix, None) {
                return FilterReturn::False;
            }
        } else if !self.filter_rect_mask(
            &mut filter_m,
            &small_r[0],
            matrix,
            None,
            CreateMode::ComputeBoundsAndRenderImage,
        ) {
            return FilterReturn::False;
        }
        *patch = Some(make_patch(filter_m, dst_m.bounds, center));
        FilterReturn::True
    }

    // Port of: src/core/SkBlurMaskFilterImpl.cpp#L528-L539 (chrome/m156)
    fn compute_fast_bounds(&self, src: &Rect) -> Rect {
        // TODO: if we're doing kInner blur, should we return a different outset?
        //       i.e. pad == 0 ?

        let pad = 3.0 * self.sigma;

        Rect::new(
            src.left - pad,
            src.top - pad,
            src.right + pad,
            src.bottom + pad,
        )
    }
}
