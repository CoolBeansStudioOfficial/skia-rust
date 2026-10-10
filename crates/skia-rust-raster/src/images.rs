// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/image/SkImage_RasterFactories.cpp (`SkImages::MakeWithFilter`)

//! The `SkImages` factories that need the raster image filter backend.

use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;
use skia_rust_core::surface_props::SurfaceProps;

use crate::image_filter_backend::make_raster_backend;

/// `SkImages::MakeWithFilter(src, filter, subset, clipBounds, outSubset, offset)`: filters
/// `subset` of `src` on the CPU. Returns the result image, its subset that holds the result
/// (`outSubset`) and the offset of that subset relative to `src` (`offset`).
// Port of: src/image/SkImage_RasterFactories.cpp#L108-L125 (chrome/m156)
#[doc(alias = "MakeWithFilter")]
#[must_use]
pub fn make_with_filter(
    src: &Image,
    filter: &ImageFilter,
    subset: &IRect,
    clip_bounds: &IRect,
) -> Option<(Image, IRect, IPoint)> {
    let backend = make_raster_backend(SurfaceProps::default(), src.color_type());
    filter.make_image_with_filter(backend, src, *subset, *clip_bounds)
}
