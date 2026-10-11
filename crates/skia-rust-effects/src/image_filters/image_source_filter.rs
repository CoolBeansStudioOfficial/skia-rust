// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkImageImageFilter.cpp

//! `SkImageImageFilter`: draws a part of an image into a rectangle. It is a leaf filter that
//! ignores its (absent) inputs.

use skia_rust_core::flattenable::FlattenableRegistry;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::{
    ImageFilter, ImageFilterBase, ImageFilterCommon, round_out_layer,
};
use skia_rust_core::image_filter_result::FilterResult;
use skia_rust_core::image_filter_types::{Context, Mapping, MatrixCapability};
use skia_rust_core::picture_priv;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::rect::{Contains, IRect, Rect};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::sampling_priv::{LegacyFQ, MediumAs, from_fq};
use skia_rust_core::write_buffer::BinaryWriteBuffer;

use crate::image_filters::crop_filter::empty;

/// The image source filter (`SkImageImageFilter`).
// Port of: src/effects/imagefilters/SkImageImageFilter.cpp#L17-L45 (chrome/m156)
#[doc(alias = "SkImageImageFilter")]
#[derive(Debug)]
pub struct ImageImageFilter {
    common: ImageFilterCommon,
    image: Image,
    src_rect: Rect,
    /// The parameter-space destination (`fDstRect`).
    dst_rect: Rect,
    sampling: SamplingOptions,
}

impl ImageFilterBase for ImageImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/effects/imagefilters/SkImageImageFilter.cpp#L52 (chrome/m156), SK_FLATTENABLE_HOOKS
    fn type_name(&self) -> &'static str {
        "SkImageImageFilter"
    }

    // Port of: src/effects/imagefilters/SkImageImageFilter.cpp#L131-L135 (chrome/m156), flatten
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        buffer.write_sampling(&self.sampling);
        buffer.write_rect(&self.src_rect);
        buffer.write_rect(&self.dst_rect);
        buffer.write_image(&self.image);
    }

    // Port of: src/effects/imagefilters/SkImageImageFilter.cpp#L34 (chrome/m156)
    fn on_get_ctm_capability(&self) -> MatrixCapability {
        MatrixCapability::Complex
    }

    // Port of: src/effects/imagefilters/SkImageImageFilter.cpp#L47-L49 (chrome/m156)
    fn on_filter_image(&self, ctx: &Context<'_>) -> FilterResult {
        FilterResult::make_from_image(
            ctx,
            &self.image,
            self.src_rect,
            self.dst_rect,
            self.sampling,
        )
    }

    // Port of: src/effects/imagefilters/SkImageImageFilter.cpp#L51-L55 (chrome/m156)
    fn on_get_input_layer_bounds(
        &self,
        _mapping: &Mapping,
        _desired_output: IRect,
        _content_bounds: Option<IRect>,
    ) -> IRect {
        IRect::new_empty()
    }

    // Port of: src/effects/imagefilters/SkImageImageFilter.cpp#L57-L62 (chrome/m156)
    fn on_get_output_layer_bounds(
        &self,
        mapping: &Mapping,
        _content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        Some(round_out_layer(
            &mapping.param_to_layer_rect(&self.dst_rect),
        ))
    }

    // Port of: src/effects/imagefilters/SkImageImageFilter.cpp#L24 (chrome/m156)
    fn compute_fast_bounds(&self, _src: &Rect) -> Rect {
        self.dst_rect
    }
}

/// `SkImageFilters::Image(image, srcRect, dstRect, sampling)`: `Empty()` for an empty rectangle
/// or no image.
// Port of: src/effects/imagefilters/SkImageImageFilter.cpp#L82-L108 (chrome/m156)
#[doc(alias = "Image")]
#[must_use]
pub fn image(
    image: Option<Image>,
    src_rect: Rect,
    dst_rect: Rect,
    sampling: SamplingOptions,
) -> ImageFilter {
    let Some(image) = image else {
        return empty();
    };
    if src_rect.is_empty() || dst_rect.is_empty() {
        return empty();
    }
    let image_bounds = Rect::from_isize(image.dimensions());
    if image_bounds.contains(src_rect) {
        return ImageFilter::from_base(ImageImageFilter {
            common: ImageFilterCommon::new(Vec::new(), None),
            image,
            src_rect,
            dst_rect,
            sampling,
        });
    }
    let src_to_dst = skia_rust_core::matrix::Matrix::rect_to_rect_or_identity(
        src_rect,
        dst_rect,
        skia_rust_core::matrix::ScaleToFit::Fill,
    );
    let mut image_bounds = image_bounds;
    if !image_bounds.intersect(src_rect) {
        return empty();
    }
    let mapped_bounds = src_to_dst.map_rect(image_bounds).0;
    if mapped_bounds.is_empty() {
        return empty();
    }
    ImageFilter::from_base(ImageImageFilter {
        common: ImageFilterCommon::new(Vec::new(), None),
        image,
        src_rect: image_bounds,
        dst_rect: mapped_bounds,
        sampling,
    })
}

/// `SkImageFilters::Image(image, sampling)`: the whole image, drawn at its own size.
// Port of: include/effects/SkImageFilters.h#L272-L278 (chrome/m156)
#[must_use]
pub fn image_sampled(image: Option<Image>, sampling: SamplingOptions) -> Option<ImageFilter> {
    let image = image?;
    let r = Rect::from_isize(image.dimensions());
    Some(self::image(Some(image), r, r, sampling))
}

/// `checkFilterQuality`: the legacy filter quality, which the reader checks is in range (a value
/// out of range is `kNone`).
// Port of: src/core/SkReadBuffer.cpp#L550-L562 (chrome/m156), checkInt and checkFilterQuality
fn check_filter_quality(buffer: &mut ReadBuffer<'_>) -> LegacyFQ {
    let value = buffer.read32();
    if !(0..=3).contains(&value) {
        buffer.validate(false);
        return LegacyFQ::None;
    }
    match value {
        1 => LegacyFQ::Low,
        2 => LegacyFQ::Medium,
        3 => LegacyFQ::High,
        _ => LegacyFQ::None,
    }
}

/// `SkImageImageFilter::CreateProc`.
// Port of: src/effects/imagefilters/SkImageImageFilter.cpp#L118-L135 (chrome/m156)
pub fn create_proc(
    buffer: &mut ReadBuffer<'_>,
    _registry: &FlattenableRegistry,
) -> Option<ImageFilter> {
    let sampling = if buffer.is_version_lt(picture_priv::VERSION_IMAGE_FILTER_IMAGE_SAMPLING) {
        from_fq(check_filter_quality(buffer), MediumAs::Linear)
    } else {
        buffer.read_sampling()
    };
    let src = buffer.read_rect();
    let dst = buffer.read_rect();
    let img = buffer.read_image()?;
    Some(image(Some(img), src, dst, sampling))
}
