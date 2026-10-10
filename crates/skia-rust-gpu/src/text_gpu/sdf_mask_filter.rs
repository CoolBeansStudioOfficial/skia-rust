// Copyright 2017 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/text/gpu/SDFMaskFilter.h, src/text/gpu/SDFMaskFilter.cpp

//! The mask filter that makes the signed distance field of a glyph's mask (`SDFMaskFilter`).

use skia_rust_core::distance_field_gen::{
    DISTANCE_FIELD_PAD, generate_distance_field_from_a8_image,
    generate_distance_field_from_bw_image, generate_distance_field_from_lcd16_mask,
};
use skia_rust_core::mask::{Mask, MaskBuilder, MaskFormat};
use skia_rust_core::mask_filter::{MaskFilter, MaskFilterBase, MaskFilterType};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::Rect;

/// `sktext::gpu::SDFMaskFilterImpl`.
// Port of: src/text/gpu/SDFMaskFilter.cpp#L28-L44 (chrome/m156)
#[doc(alias = "SDFMaskFilterImpl")]
#[derive(Debug, Default)]
struct SdfMaskFilterImpl;

impl MaskFilterBase for SdfMaskFilterImpl {
    // Port of: src/text/gpu/SDFMaskFilter.cpp#L50-L52 (chrome/m156)
    fn format(&self) -> MaskFormat {
        MaskFormat::Sdf
    }

    // Port of: src/text/gpu/SDFMaskFilter.cpp#L54-L92 (chrome/m156)
    fn filter_mask(
        &self,
        dst: &mut MaskBuilder,
        src: &Mask<'_>,
        _matrix: &Matrix,
        margin: Option<&mut IPoint>,
    ) -> bool {
        if src.format != MaskFormat::A8
            && src.format != MaskFormat::BW
            && src.format != MaskFormat::Lcd16
        {
            return false;
        }

        *dst = MaskBuilder::prepare_destination(DISTANCE_FIELD_PAD, DISTANCE_FIELD_PAD, src);
        dst.format = MaskFormat::Sdf;

        if let Some(margin) = margin {
            *margin = IPoint::new(DISTANCE_FIELD_PAD, DISTANCE_FIELD_PAD);
        }

        if src.image.is_empty() {
            return true;
        }
        if dst.image.is_empty() {
            dst.bounds.set_empty();
            return false;
        }

        let width = usize::try_from(src.bounds.width()).unwrap_or(0);
        let height = usize::try_from(src.bounds.height()).unwrap_or(0);
        let row_bytes = src.row_bytes as usize;
        match src.format {
            MaskFormat::A8 => generate_distance_field_from_a8_image(
                &mut dst.image,
                src.image,
                width,
                height,
                row_bytes,
            ),
            MaskFormat::Lcd16 => generate_distance_field_from_lcd16_mask(
                &mut dst.image,
                src.image,
                width,
                height,
                row_bytes,
            ),
            _ => generate_distance_field_from_bw_image(
                &mut dst.image,
                src.image,
                width,
                height,
                row_bytes,
            ),
        }
    }

    fn filter_type(&self) -> MaskFilterType {
        MaskFilterType::Sdf
    }

    // Port of: src/text/gpu/SDFMaskFilter.cpp#L42 (chrome/m156), SK_FLATTENABLE_HOOKS
    fn type_name(&self) -> &'static str {
        "SDFMaskFilterImpl"
    }

    // Port of: src/text/gpu/SDFMaskFilter.cpp#L94-L98 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // the pad is 4
    fn compute_fast_bounds(&self, src: &Rect) -> Rect {
        let pad = DISTANCE_FIELD_PAD as f32;
        Rect::from_ltrb(
            src.left - pad,
            src.top - pad,
            src.right + pad,
            src.bottom + pad,
        )
    }
}

/// `SDFMaskFilter::Make()`: the mask filter that makes distance fields.
// Port of: src/text/gpu/SDFMaskFilter.cpp#L104-L106 (chrome/m156)
#[doc(alias = "SDFMaskFilter::Make")]
#[must_use]
pub fn make() -> MaskFilter {
    MaskFilter::from_base(SdfMaskFilterImpl)
}

#[cfg(test)]
mod tests {
    use super::*;
    use skia_rust_core::rect::IRect;

    #[test]
    fn filters_an_a8_mask_into_a_padded_distance_field() {
        let image = vec![255u8; 4 * 4];
        let src = Mask::new(&image, IRect::new(0, 0, 4, 4), 4, MaskFormat::A8);
        let mut dst = MaskBuilder::default();
        let mut margin = IPoint::new(0, 0);
        assert!(make().as_base().filter_mask(
            &mut dst,
            &src,
            Matrix::i(),
            Some(&mut margin)
        ));
        assert_eq!(dst.format, MaskFormat::Sdf);
        assert_eq!(margin, IPoint::new(4, 4));
        assert_eq!(dst.bounds, IRect::new(-4, -4, 8, 8));
        assert_eq!(dst.image.len(), 12 * 12);
    }

    #[test]
    fn rejects_other_formats() {
        let image = vec![0u8; 16];
        let src = Mask::new(&image, IRect::new(0, 0, 2, 2), 8, MaskFormat::Argb32);
        let mut dst = MaskBuilder::default();
        assert!(
            !make()
                .as_base()
                .filter_mask(&mut dst, &src, Matrix::i(), None)
        );
    }

    #[test]
    fn fast_bounds_grow_by_the_pad() {
        let b = make()
            .as_base()
            .compute_fast_bounds(&Rect::from_ltrb(1.0, 2.0, 3.0, 4.0));
        assert_eq!(b, Rect::from_ltrb(-3.0, -2.0, 7.0, 8.0));
    }
}
