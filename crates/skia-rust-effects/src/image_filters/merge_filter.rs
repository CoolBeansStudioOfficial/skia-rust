// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkMergeImageFilter.cpp

//! `SkMergeImageFilter`: source-over merges any number of inputs.

use skia_rust_core::image_filter::{ImageFilter, ImageFilterBase, ImageFilterCommon};
use skia_rust_core::image_filter_result::{Builder, FilterResult};
use skia_rust_core::image_filter_types::{Context, Mapping, MatrixCapability};
use skia_rust_core::rect::{IRect, Rect};

use crate::image_filters::crop_filter::crop;

/// `LayerSpace<SkIRect>::Union(count, boundsFn)`: the join of the bounds, or empty for no bounds.
// Port of: src/core/SkImageFilterTypes.h#L282-L292 (chrome/m156)
fn union(count: usize, mut bounds: impl FnMut(usize) -> IRect) -> IRect {
    if count == 0 {
        return IRect::new_empty();
    }
    let mut output = bounds(0);
    for i in 1..count {
        output = IRect::join(&output, &bounds(i));
    }
    output
}

/// The merge filter (`SkMergeImageFilter`).
// Port of: src/effects/imagefilters/SkMergeImageFilter.cpp#L15-L31 (chrome/m156)
#[doc(alias = "SkMergeImageFilter")]
#[derive(Debug)]
pub struct MergeImageFilter {
    common: ImageFilterCommon,
}

impl ImageFilterBase for MergeImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/effects/imagefilters/SkMergeImageFilter.cpp#L22 (chrome/m156)
    fn on_get_ctm_capability(&self) -> MatrixCapability {
        MatrixCapability::Complex
    }

    // Port of: src/effects/imagefilters/SkMergeImageFilter.cpp#L35-L42 (chrome/m156)
    fn on_filter_image(&self, ctx: &Context<'_>) -> FilterResult {
        let mut builder = Builder::new(ctx);
        for i in 0..self.count_inputs() {
            builder.add(
                self.get_child_output(i, ctx),
                None,
                skia_rust_core::image_filter_result::ShaderFlags::NONE,
                skia_rust_core::image_filter_result::default_sampling(),
            );
        }
        builder.merge()
    }

    // Port of: src/effects/imagefilters/SkMergeImageFilter.cpp#L44-L52 (chrome/m156)
    fn on_get_input_layer_bounds(
        &self,
        mapping: &Mapping,
        desired_output: IRect,
        content_bounds: Option<IRect>,
    ) -> IRect {
        union(self.count_inputs(), |i| {
            self.get_child_input_layer_bounds(i, mapping, desired_output, content_bounds)
        })
    }

    // Port of: src/effects/imagefilters/SkMergeImageFilter.cpp#L54-L75 (chrome/m156)
    fn on_get_output_layer_bounds(
        &self,
        mapping: &Mapping,
        content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        let mut child_is_unbounded = false;
        let child_output = union(self.count_inputs(), |i| {
            if let Some(o) = self.get_child_output_layer_bounds(i, mapping, content_bounds) {
                o
            } else {
                child_is_unbounded = true;
                IRect::new_empty()
            }
        });
        if child_is_unbounded {
            None
        } else {
            Some(child_output)
        }
    }
}

/// `SkImageFilters::Merge(filters, count, cropRect)`: `Empty()` for no inputs, and the crop of
/// the merge if `crop_rect` is given.
// Port of: src/effects/imagefilters/SkMergeImageFilter.cpp#L81-L92 (chrome/m156)
#[doc(alias = "Merge")]
#[must_use]
pub fn merge(inputs: &[Option<ImageFilter>], crop_rect: Option<Rect>) -> Option<ImageFilter> {
    if inputs.is_empty() {
        return Some(crate::image_filters::crop_filter::empty());
    }
    let filter = ImageFilter::from_base(MergeImageFilter {
        common: ImageFilterCommon::new(inputs.to_vec(), None),
    });
    match crop_rect {
        Some(rect) => crop(
            &rect,
            skia_rust_core::tile_mode::TileMode::Decal,
            Some(filter),
        ),
        None => Some(filter),
    }
}
