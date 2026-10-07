// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkSurface.h (namespace SkSurfaces), src/image/SkSurface_Raster.cpp

//! `SkSurfaces`: the raster surface factories (`skia_safe::surfaces`).

use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::size::ISize;
use skia_rust_core::surface_props::SurfaceProps;

use crate::surface::Surface;
pub use crate::surface::surface_validate_raster_info;

/// A surface with newly allocated, zeroed pixels (`SkSurfaces::Raster`). `row_bytes` of `None`
/// means the minimum. `None` if the info is not valid for a raster surface or the allocation
/// fails.
// Port of: src/image/SkSurface_Raster.cpp#L178-L192 (chrome/m156)
#[doc(alias = "Raster")]
#[must_use]
pub fn raster(
    image_info: &ImageInfo,
    row_bytes: impl Into<Option<usize>>,
    props: Option<&SurfaceProps>,
) -> Option<Surface<'static>> {
    Surface::new_raster(image_info, row_bytes.into(), props)
}

/// [`raster`] with a premultiplied N32 info of `size` (`SkSurfaces::Raster(SkImageInfo::MakeN32Premul)`).
#[doc(alias = "Raster")]
#[must_use]
pub fn raster_n32_premul(size: impl Into<ISize>) -> Option<Surface<'static>> {
    raster(&ImageInfo::new_n32_premul(size, None), None, None)
}

/// A surface that draws into `pixels` (`SkSurfaces::WrapPixels`). The slice is copied into the
/// surface and copied back when it drops, because a canvas owns the pixels it draws into
/// (`docs/design/pixels.md`); use [`Surface::wrap_pixels`] to wrap a `Bitmap` without a copy.
/// `None` if the info, row bytes or the slice length are invalid.
// Port of: src/image/SkSurface_Raster.cpp#L152-L176 (chrome/m156)
#[doc(alias = "WrapPixels")]
#[must_use]
pub fn wrap_pixels<'a>(
    image_info: &ImageInfo,
    pixels: &'a mut [u8],
    row_bytes: impl Into<Option<usize>>,
    props: Option<&SurfaceProps>,
) -> Option<Surface<'a>> {
    let row_bytes = row_bytes
        .into()
        .filter(|&r| r != 0)
        .unwrap_or_else(|| image_info.min_row_bytes());
    Surface::wrap_slice(image_info, pixels, row_bytes, props)
}
