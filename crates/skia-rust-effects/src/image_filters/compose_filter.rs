// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkComposeImageFilter.cpp

//! `SkComposeImageFilter`: runs an inner filter, then an outer filter on its result.

use skia_rust_core::image_filter::{ImageFilter, ImageFilterBase, ImageFilterCommon};
use skia_rust_core::image_filter_result::FilterResult;
use skia_rust_core::image_filter_types::{Context, Mapping, MatrixCapability};
use skia_rust_core::rect::IRect;

/// Index of the outer input (`kOuter`).
const OUTER: usize = 0;
/// Index of the inner input (`kInner`).
const INNER: usize = 1;

/// The compose filter (`SkComposeImageFilter`).
// Port of: src/effects/imagefilters/SkComposeImageFilter.cpp#L14-L36 (chrome/m156)
#[doc(alias = "SkComposeImageFilter")]
#[derive(Debug)]
pub struct ComposeImageFilter {
    common: ImageFilterCommon,
}

impl ImageFilterBase for ComposeImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/effects/imagefilters/SkComposeImageFilter.cpp#L24 (chrome/m156)
    fn on_get_ctm_capability(&self) -> MatrixCapability {
        MatrixCapability::Complex
    }

    // Port of: src/effects/imagefilters/SkComposeImageFilter.cpp#L38-L50 (chrome/m156)
    fn on_filter_image(&self, context: &Context<'_>) -> FilterResult {
        let inner_output_bounds = self.get_child_output_layer_bounds(
            INNER,
            context.mapping(),
            Some(context.source().layer_bounds()),
        );
        let outer_required_input = self.get_child_input_layer_bounds(
            OUTER,
            context.mapping(),
            context.desired_output(),
            inner_output_bounds,
        );
        let inner_result = self.get_child_output(
            INNER,
            &context.with_new_desired_output(outer_required_input),
        );
        self.get_child_output(OUTER, &context.with_new_source(inner_result))
    }

    // Port of: src/effects/imagefilters/SkComposeImageFilter.cpp#L52-L64 (chrome/m156)
    fn on_get_input_layer_bounds(
        &self,
        mapping: &Mapping,
        desired_output: IRect,
        content_bounds: Option<IRect>,
    ) -> IRect {
        // else leave outer's content bounds "unbounded"
        let outer_content_bounds = match content_bounds {
            Some(_) => self.get_child_output_layer_bounds(INNER, mapping, content_bounds),
            None => None,
        };
        let inner_desired_output =
            self.get_child_input_layer_bounds(OUTER, mapping, desired_output, outer_content_bounds);
        self.get_child_input_layer_bounds(INNER, mapping, inner_desired_output, content_bounds)
    }

    // Port of: src/effects/imagefilters/SkComposeImageFilter.cpp#L66-L71 (chrome/m156)
    fn on_get_output_layer_bounds(
        &self,
        mapping: &Mapping,
        content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        let inner_bounds = self.get_child_output_layer_bounds(INNER, mapping, content_bounds);
        self.get_child_output_layer_bounds(OUTER, mapping, inner_bounds)
    }
}

/// `SkImageFilters::Compose(outer, inner)`: `inner` if `outer` is null and vice versa.
// Port of: src/effects/imagefilters/SkComposeImageFilter.cpp#L95-L106 (chrome/m156)
#[doc(alias = "Compose")]
#[must_use]
pub fn compose(outer: Option<ImageFilter>, inner: Option<ImageFilter>) -> Option<ImageFilter> {
    let Some(outer) = outer else {
        return inner;
    };
    let Some(inner) = inner else {
        return Some(outer);
    };
    let uses_src = inner.uses_source();
    Some(ImageFilter::from_base(ComposeImageFilter {
        common: ImageFilterCommon::new(vec![Some(outer), Some(inner)], Some(uses_src)),
    }))
}
