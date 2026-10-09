// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkColorFilterImageFilter.cpp

//! `SkColorFilterImageFilter`: applies a color filter to its input.

use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::image_filter::{ImageFilter, ImageFilterBase, ImageFilterCommon};
use skia_rust_core::image_filter_result::FilterResult;
use skia_rust_core::image_filter_types::{Context, Mapping, MatrixCapability};
use skia_rust_core::rect::{IRect, Rect, rect_priv};
use skia_rust_core::tile_mode::TileMode;

use crate::image_filters::crop_filter::crop;

/// The color filter image filter (`SkColorFilterImageFilter`).
// Port of: src/effects/imagefilters/SkColorFilterImageFilter.cpp#L16-L36 (chrome/m156)
#[doc(alias = "SkColorFilterImageFilter")]
#[derive(Debug)]
pub struct ColorFilterImageFilter {
    common: ImageFilterCommon,
    color_filter: ColorFilter,
}

impl ImageFilterBase for ColorFilterImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/effects/imagefilters/SkColorFilterImageFilter.cpp#L35 (chrome/m156)
    fn on_get_ctm_capability(&self) -> MatrixCapability {
        MatrixCapability::Complex
    }

    // Port of: src/effects/imagefilters/SkColorFilterImageFilter.cpp#L37-L39 (chrome/m156)
    fn on_affects_transparent_black(&self) -> bool {
        self.color_filter.as_base().affects_transparent_black()
    }

    // Port of: src/effects/imagefilters/SkColorFilterImageFilter.cpp#L41-L48 (chrome/m156)
    fn on_is_color_filter_node(&self) -> Option<ColorFilter> {
        Some(self.color_filter.clone())
    }

    // Port of: src/effects/imagefilters/SkColorFilterImageFilter.cpp#L50-L53 (chrome/m156)
    fn on_filter_image(&self, ctx: &Context<'_>) -> FilterResult {
        self.get_child_output(0, ctx)
            .apply_color_filter(ctx, self.color_filter.clone())
    }

    // Port of: src/effects/imagefilters/SkColorFilterImageFilter.cpp#L55-L60 (chrome/m156)
    fn on_get_input_layer_bounds(
        &self,
        mapping: &Mapping,
        desired_output: IRect,
        content_bounds: Option<IRect>,
    ) -> IRect {
        self.get_child_input_layer_bounds(0, mapping, desired_output, content_bounds)
    }

    // Port of: src/effects/imagefilters/SkColorFilterImageFilter.cpp#L62-L69 (chrome/m156)
    fn on_get_output_layer_bounds(
        &self,
        mapping: &Mapping,
        content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        if self.color_filter.as_base().affects_transparent_black() {
            None
        } else {
            self.get_child_output_layer_bounds(0, mapping, content_bounds)
        }
    }

    // Port of: src/effects/imagefilters/SkColorFilterImageFilter.cpp#L71-L79 (chrome/m156)
    fn compute_fast_bounds(&self, bounds: &Rect) -> Rect {
        if self.color_filter.as_base().affects_transparent_black() {
            rect_priv::make_large_s32()
        } else if let Some(input) = self.get_input(0) {
            input.compute_fast_bounds(bounds)
        } else {
            *bounds
        }
    }
}

/// `SkImageFilters::ColorFilter(cf, input, cropRect)`.
///
/// When `input` is a color filter node, the two color filters are composed (`makeComposed`) and
/// the new filter wraps the input's own input instead.
// Port of: src/effects/imagefilters/SkColorFilterImageFilter.cpp#L86-L103 (chrome/m156)
#[doc(alias = "ColorFilter")]
#[must_use]
pub fn color_filter(
    cf: Option<ColorFilter>,
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    let mut cf = cf;
    let mut input = input;
    if let Some(outer) = cf.take() {
        let input_color_filter = input
            .as_ref()
            .and_then(|f| f.as_base().on_is_color_filter_node());
        match input_color_filter {
            Some(inner) => {
                // `cf->makeComposed(inputCF)`, then `input = input->getInput(0)`.
                cf = Some(outer.composed(Some(inner)));
                input = input.as_ref().and_then(|f| f.get_input(0).cloned());
            }
            None => cf = Some(outer),
        }
    }
    let mut filter = input.take();
    if let Some(cf) = cf.take() {
        filter = Some(ImageFilter::from_base(ColorFilterImageFilter {
            common: ImageFilterCommon::new(vec![filter], None),
            color_filter: cf,
        }));
    }
    match crop_rect {
        Some(rect) => crop(&rect, TileMode::Decal, filter),
        None => filter,
    }
}
