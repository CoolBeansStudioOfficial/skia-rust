// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkPictureImageFilter.cpp

//! `SkImageFilters::Picture`: a leaf filter that plays a picture back into its target rectangle.

use skia_rust_core::image_filter::{ImageFilter, ImageFilterBase, ImageFilterCommon};
use skia_rust_core::image_filter_result::FilterResult;
use skia_rust_core::image_filter_types::{Context, Mapping, MatrixCapability, round_out};
use skia_rust_core::picture::Picture;
use skia_rust_core::rect::{IRect, Rect};

use crate::image_filters::crop_filter::empty;

/// The picture filter (`SkPictureImageFilter`).
// Port of: src/effects/imagefilters/SkPictureImageFilter.cpp#L28-L61 (chrome/m156)
#[doc(alias = "SkPictureImageFilter")]
#[derive(Debug)]
pub struct PictureImageFilter {
    common: ImageFilterCommon,
    picture: Picture,
    /// The parameter-space rectangle the picture is drawn into (`fCullRect`).
    cull_rect: Rect,
}

impl PictureImageFilter {
    /// `SkPictureImageFilter(picture, cullRect)`: `cull_rect` is already intersected with the
    /// picture's own cull rectangle.
    // Port of: src/effects/imagefilters/SkPictureImageFilter.cpp#L30-L36 (chrome/m156)
    #[must_use]
    pub fn new(picture: Picture, cull_rect: Rect) -> Self {
        PictureImageFilter {
            common: ImageFilterCommon::new(Vec::new(), None),
            picture,
            cull_rect,
        }
    }
}

impl ImageFilterBase for PictureImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/effects/imagefilters/SkPictureImageFilter.cpp#L38-L39 (chrome/m156)
    fn compute_fast_bounds(&self, _bounds: &Rect) -> Rect {
        self.cull_rect
    }

    // Port of: src/effects/imagefilters/SkPictureImageFilter.cpp#L52-L53 (chrome/m156)
    fn on_get_ctm_capability(&self) -> MatrixCapability {
        MatrixCapability::Complex
    }

    // Port of: src/effects/imagefilters/SkPictureImageFilter.cpp#L86-L87 (chrome/m156)
    fn on_filter_image(&self, ctx: &Context<'_>) -> FilterResult {
        FilterResult::make_from_picture(ctx, &self.picture, &self.cull_rect)
    }

    // Port of: src/effects/imagefilters/SkPictureImageFilter.cpp#L92-L98 (chrome/m156)
    fn on_get_input_layer_bounds(
        &self,
        _mapping: &Mapping,
        _desired_output: IRect,
        _content_bounds: Option<IRect>,
    ) -> IRect {
        // This is a leaf filter, it requires no input and no further recursion
        IRect::new_empty()
    }

    // Port of: src/effects/imagefilters/SkPictureImageFilter.cpp#L100-L106 (chrome/m156)
    fn on_get_output_layer_bounds(
        &self,
        mapping: &Mapping,
        _content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        // The output is the transformed bounds of the picture.
        Some(round_out(&mapping.param_to_layer_rect(&self.cull_rect)))
    }
}

/// `SkImageFilters::Picture(pic, targetRect)`: `pic` played back into `target_rect`, or the empty
/// filter if there is no picture or its cull rectangle does not meet `target_rect`. `None` for
/// `target_rect` is the picture's own cull rectangle (`SkImageFilters::Picture(pic)`).
// Port of: src/effects/imagefilters/SkPictureImageFilter.cpp#L63-L74 (chrome/m156)
// Port of: include/effects/SkImageFilters.h#L378-L381 (chrome/m156) (the one-argument overload)
#[doc(alias = "Picture")]
#[must_use]
pub fn picture(pic: Option<Picture>, target_rect: Option<&Rect>) -> Option<ImageFilter> {
    if let Some(pic) = pic {
        let target = target_rect.copied().unwrap_or_else(|| pic.cull_rect());
        let mut cull_rect = pic.cull_rect();
        if cull_rect.intersect(target) {
            return Some(ImageFilter::from_base(PictureImageFilter::new(
                pic, cull_rect,
            )));
        }
    }
    Some(empty())
}
