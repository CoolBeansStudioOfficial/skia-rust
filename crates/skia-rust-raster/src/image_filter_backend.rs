// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkImageFilterTypes.cpp (`RasterBackend`, `MakeRasterBackend`)

//! The raster [`Backend`] of image filters: devices are [`BitmapDevice`]s, and special images
//! share the pixels of the images they are made from.

use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::device::Device;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter_types::Backend;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;
use skia_rust_core::special_image::SpecialImage;
use skia_rust_core::surface_props::SurfaceProps;

use crate::bitmap_device::BitmapDevice;

/// The raster backend (`RasterBackend`).
// Port of: src/core/SkImageFilterTypes.cpp#L186-L223 (chrome/m156)
#[derive(Debug)]
pub struct RasterBackend {
    surface_props: SurfaceProps,
    color_type: ColorType,
}

impl Backend for RasterBackend {
    // Port of: src/core/SkImageFilterTypes.cpp#L193-L200 (chrome/m156)
    fn make_device(
        &self,
        size: ISize,
        color_space: Option<ColorSpace>,
        props: Option<&SurfaceProps>,
    ) -> Option<Box<dyn Device>> {
        let image_info = ImageInfo::new(size, self.color_type, AlphaType::Premul, color_space);
        let props = props.copied().unwrap_or(self.surface_props);
        BitmapDevice::create(&image_info, props).map(|d| Box::new(d) as Box<dyn Device>)
    }

    // Port of: src/core/SkImageFilterTypes.cpp#L201-L204 (chrome/m156), SkSpecialImages::MakeFromRaster
    fn make_image(&self, subset: &IRect, image: &Image) -> Option<SpecialImage> {
        if subset.is_empty() {
            return None;
        }
        let bm = image.get_ro_pixels()?;
        SpecialImage::make_from_raster(subset, &bm, &self.surface_props)
    }

    // Port of: src/core/SkImageFilterTypes.cpp#L205-L207 (chrome/m156)
    fn get_cached_bitmap(&self, data: &Bitmap) -> Option<Image> {
        images::raster_from_bitmap(data)
    }

    fn surface_props(&self) -> &SurfaceProps {
        &self.surface_props
    }

    fn color_type(&self) -> ColorType {
        self.color_type
    }
}

/// `MakeRasterBackend(surfaceProps, colorType)`.
// Port of: src/core/SkImageFilterTypes.cpp#L225-L227 (chrome/m156)
#[must_use]
pub fn make_raster_backend(surface_props: SurfaceProps, color_type: ColorType) -> Arc<dyn Backend> {
    Arc::new(RasterBackend {
        surface_props,
        color_type,
    })
}
