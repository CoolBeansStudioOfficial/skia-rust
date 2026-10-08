// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkCropImageFilter.cpp

//! `SkCropImageFilter`: crops its input to a rectangle, tiling the crop with a tile mode. `Empty`
//! and `Tile` are crops.

#![allow(clippy::collapsible_if)] // Keeps the C++ control flow for review.
use skia_rust_core::image_filter::{
    ImageFilter, ImageFilterBase, ImageFilterCommon, round_out_layer,
};
use skia_rust_core::image_filter_result::FilterResult;
use skia_rust_core::image_filter_types::{Context, Mapping, relevant_subset, round_in};
use skia_rust_core::rect::{IRect, Rect, rect_priv};
use skia_rust_core::tile_mode::TileMode;

/// The crop filter (`SkCropImageFilter`).
// Port of: src/effects/imagefilters/SkCropImageFilter.cpp#L16-L51 (chrome/m156)
#[doc(alias = "SkCropImageFilter")]
#[derive(Debug)]
pub struct CropImageFilter {
    common: ImageFilterCommon,
    /// The parameter-space crop (`fCropRect`).
    crop_rect: Rect,
    tile_mode: TileMode,
}

impl CropImageFilter {
    /// `SkCropImageFilter(cropRect, tileMode, input)`.
    // Port of: src/effects/imagefilters/SkCropImageFilter.cpp#L16-L24 (chrome/m156)
    #[must_use]
    pub fn new(crop_rect: Rect, tile_mode: TileMode, input: Option<ImageFilter>) -> Self {
        CropImageFilter {
            common: ImageFilterCommon::new(vec![input], None),
            crop_rect,
            tile_mode,
        }
    }

    /// `cropRect(mapping)`: the layer-space crop, rounded out for decal and in otherwise.
    // Port of: src/effects/imagefilters/SkCropImageFilter.cpp#L43-L47 (chrome/m156)
    fn layer_crop_rect(&self, mapping: &Mapping) -> IRect {
        let crop = mapping.param_to_layer_rect(&self.crop_rect);
        if self.tile_mode == TileMode::Decal {
            round_out_layer(&crop)
        } else {
            round_in(&crop)
        }
    }

    /// `requiredInput(mapping, outputBounds)`.
    // Port of: src/effects/imagefilters/SkCropImageFilter.cpp#L48-L51 (chrome/m156)
    fn required_input(&self, mapping: &Mapping, output_bounds: IRect) -> IRect {
        relevant_subset(self.layer_crop_rect(mapping), output_bounds, self.tile_mode)
    }
}

impl ImageFilterBase for CropImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/effects/imagefilters/SkCropImageFilter.cpp#L33-L35 (chrome/m156)
    fn on_affects_transparent_black(&self) -> bool {
        self.tile_mode != TileMode::Decal
    }

    // Port of: src/effects/imagefilters/SkCropImageFilter.cpp#L36 (chrome/m156)
    fn ignore_inputs_affects_transparent_black(&self) -> bool {
        true
    }

    // Port of: src/effects/imagefilters/SkCropImageFilter.cpp#L124-L129 (chrome/m156)
    fn on_filter_image(&self, context: &Context<'_>) -> FilterResult {
        let crop_input = self.required_input(context.mapping(), context.desired_output());
        let child_output = self.get_child_output(0, &context.with_new_desired_output(crop_input));
        child_output.apply_crop(
            context,
            self.layer_crop_rect(context.mapping()),
            self.tile_mode,
        )
    }

    // Port of: src/effects/imagefilters/SkCropImageFilter.cpp#L131-L138 (chrome/m156)
    fn on_get_input_layer_bounds(
        &self,
        mapping: &Mapping,
        desired_output: IRect,
        content_bounds: Option<IRect>,
    ) -> IRect {
        let required_input = self.required_input(mapping, desired_output);
        self.get_child_input_layer_bounds(0, mapping, required_input, content_bounds)
    }

    // Port of: src/effects/imagefilters/SkCropImageFilter.cpp#L140-L157 (chrome/m156)
    fn on_get_output_layer_bounds(
        &self,
        mapping: &Mapping,
        content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        let child_output = self.get_child_output_layer_bounds(0, mapping, content_bounds);
        let mut crop = self.layer_crop_rect(mapping);
        if let Some(child) = child_output {
            if !skia_rust_core::image_filter_types::irect_intersect_in_place(&mut crop, &child) {
                return Some(IRect::new_empty());
            }
        }
        if self.tile_mode == TileMode::Decal {
            Some(crop)
        } else {
            None
        }
    }

    // Port of: src/effects/imagefilters/SkCropImageFilter.cpp#L159-L166 (chrome/m156)
    fn compute_fast_bounds(&self, bounds: &Rect) -> Rect {
        let mut input_bounds = match self.get_input(0) {
            Some(input) => input.compute_fast_bounds(bounds),
            None => *bounds,
        };
        if !input_bounds.intersect(self.crop_rect) {
            return Rect::new_empty();
        }
        if self.tile_mode == TileMode::Decal {
            input_bounds
        } else {
            rect_priv::make_large_s32()
        }
    }
}

/// `SkImageFilters::Crop(rect, tileMode, input)`: `None` if `rect` is not finite and sorted.
// Port of: src/effects/imagefilters/SkCropImageFilter.cpp#L53-L60 (chrome/m156)
#[doc(alias = "Crop")]
#[must_use]
pub fn crop(rect: &Rect, tile_mode: TileMode, input: Option<ImageFilter>) -> Option<ImageFilter> {
    if !(rect.is_finite() && rect.is_sorted()) {
        return None;
    }
    Some(ImageFilter::from_base(CropImageFilter::new(
        *rect, tile_mode, input,
    )))
}

/// `SkImageFilters::Empty()`: a crop to the empty rectangle, which filters everything out.
///
/// # Panics
///
/// Never: the empty rectangle is finite and sorted.
// Port of: src/effects/imagefilters/SkCropImageFilter.cpp#L62-L64 (chrome/m156)
#[doc(alias = "Empty")]
#[must_use]
pub fn empty() -> ImageFilter {
    crop(&Rect::new_empty(), TileMode::Decal, None).expect("the empty rectangle is valid")
}

/// `SkImageFilters::Tile(src, dst, input)`: repeats `src` over `dst`.
// Port of: src/effects/imagefilters/SkCropImageFilter.cpp#L66-L72 (chrome/m156)
#[doc(alias = "Tile")]
#[must_use]
pub fn tile(src: &Rect, dst: &Rect, input: Option<ImageFilter>) -> Option<ImageFilter> {
    let filter = crop(src, TileMode::Repeat, input);
    crop(dst, TileMode::Decal, filter)
}
