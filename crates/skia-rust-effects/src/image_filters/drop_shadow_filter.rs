// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkDropShadowImageFilter.cpp

//! `SkImageFilters::DropShadow` and `DropShadowOnly`. A drop shadow is not a filter of its own:
//! it is a graph of blur, color filter, offset, merge and crop filters.

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;

use crate::image_filters::blur_filter::blur;
use crate::image_filters::color_filter_filter::color_filter;
use crate::image_filters::crop_filter::crop;
use crate::image_filters::matrix_transform_filter::matrix_transform;
use crate::image_filters::merge_filter::merge;
use skia_rust_core::color_filters;
use skia_rust_core::matrix::Matrix;

/// `make_drop_shadow_graph(offset, sigma, color, colorSpace, shadowOnly, input, crop)`: blurs the
/// input, turns it into the solid `color` with its blurred alpha, offsets it, and (unless
/// `shadow_only`) src-over merges the input on top. The result is cropped to `crop` if given.
// Port of: src/effects/imagefilters/SkDropShadowImageFilter.cpp#L35-L72 (chrome/m156)
fn make_drop_shadow_graph(
    offset: (f32, f32),
    sigma: (f32, f32),
    color: Color4f,
    color_space: Option<&ColorSpace>,
    shadow_only: bool,
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    // A drop shadow blurs the input, filters it to be the solid color + blurred alpha, and then
    // offsets it. If it's not shadow-only, the input is then src-over blended on top. Finally
    // it's cropped to the optional 'crop'.
    let mut filter = blur(sigma.0, sigma.1, TileMode::Decal, input.clone(), None);
    filter = color_filter(
        color_filters::blend(color, color_space, BlendMode::SrcIn),
        filter,
        None,
    );
    // TODO: Offset should take SkSamplingOptions too, but kLinear filtering is needed to hide
    // nearest-neighbor sampling artifacts from fractional offsets applied post-blur.
    filter = matrix_transform(
        &Matrix::translate(offset),
        SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
        filter,
    );
    if !shadow_only {
        // Merge is visually equivalent to Blend(kSrcOver) but draws each child independently,
        // whereas Blend() fills the union of the child bounds with a single shader evaluation.
        // Since we know the original and the offset blur will have somewhat disjoint bounds, a
        // Blend() shader would force evaluating tile edge conditions for each, while merge lets
        // us avoid that.
        filter = merge(&[filter, input], None);
    }
    if let Some(rect) = crop_rect {
        filter = crop(&rect, TileMode::Decal, filter);
    }
    filter
}

/// `SkImageFilters::DropShadow(dx, dy, sigmaX, sigmaY, color, colorSpace, input, cropRect)`: the
/// input, with a blurred, offset shadow of `color` drawn underneath it.
// Port of: src/effects/imagefilters/SkDropShadowImageFilter.cpp#L99-L105 (chrome/m156)
#[doc(alias = "DropShadow")]
#[must_use]
pub fn drop_shadow(
    offset: (f32, f32),
    sigma: (f32, f32),
    color: Color4f,
    color_space: Option<&ColorSpace>,
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    make_drop_shadow_graph(offset, sigma, color, color_space, false, input, crop_rect)
}

/// `SkImageFilters::DropShadowOnly(dx, dy, sigmaX, sigmaY, color, colorSpace, input, cropRect)`:
/// only the shadow, without the input.
// Port of: src/effects/imagefilters/SkDropShadowImageFilter.cpp#L107-L112 (chrome/m156)
#[doc(alias = "DropShadowOnly")]
#[must_use]
pub fn drop_shadow_only(
    offset: (f32, f32),
    sigma: (f32, f32),
    color: Color4f,
    color_space: Option<&ColorSpace>,
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    make_drop_shadow_graph(offset, sigma, color, color_space, true, input, crop_rect)
}
