// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkImage.h (namespace SkImages), src/image/SkImage_RasterFactories.cpp

//! `SkImages`: the raster image factories (`skia_safe::images`).
//!
//! skia-rust: `RasterFromCompressedTextureData` (needs `SkDecompress`), `MakeWithFilter` (image
//! filters, Phase 3) and the lazy and GPU factories are not ported.

use crate::bitmap::Bitmap;
use crate::data::Data;
use crate::image::Image;
use crate::image_base::NEED_NEW_IMAGE_UNIQUE_ID;
use crate::image_info::ImageInfo;
use crate::image_raster::{CopyPixelsMode, ImageRaster};
use crate::pixmap::Pixmap;

/// Whether `info` and `row_bytes` make a valid raster image, and the number of bytes it takes
/// (`valid_args`).
// Port of: src/image/SkImage_RasterFactories.cpp#L35-L74 (chrome/m156)
fn valid_args(info: &ImageInfo, row_bytes: usize) -> Option<usize> {
    const MAX_DIMENSION: i32 = i32::MAX >> 2;

    // TODO(mtklein): eliminate anything here that setInfo() has already checked.
    let mut b = Bitmap::new();
    if !b.set_info(info, row_bytes) {
        return None;
    }

    if info.width() <= 0 || info.height() <= 0 {
        return None;
    }
    if info.width() > MAX_DIMENSION || info.height() > MAX_DIMENSION {
        return None;
    }

    if crate::color_type::ColorType::Unknown == info.color_type() {
        return None;
    }
    if !info.valid_row_bytes(row_bytes) {
        return None;
    }

    let size = info.compute_byte_size(row_bytes);
    if ImageInfo::byte_size_overflowed(size) {
        return None;
    }

    Some(size)
}

/// Creates a CPU-backed [`Image`] from `bitmap`, sharing or copying `bitmap` pixels. If the
/// bitmap is marked immutable, and its pixel memory is shareable, it may be shared instead of
/// copied.
///
/// [`Image`] is returned if bitmap is valid. Valid [`Bitmap`] parameters include: dimensions are
/// greater than zero; each dimension fits in 29 bits; color type and alpha type are valid, and
/// color type is not unknown; row bytes are large enough to hold one row of pixels; pixel
/// address is not `None` (`RasterFromBitmap`).
// Port of: src/image/SkImage_RasterFactories.cpp#L76-L82 (chrome/m156)
#[doc(alias = "RasterFromBitmap")]
#[must_use]
pub fn raster_from_bitmap(bitmap: &Bitmap) -> Option<Image> {
    bitmap.pixel_ref()?;

    make_image_from_raster_bitmap(bitmap, CopyPixelsMode::IfMutable)
}

/// Creates a CPU-backed [`Image`] from the pixels of `bitmap` (`SkImage_Raster::MakeFromBitmap`
/// as an [`Image`]), sharing them or copying them according to `cpm`.
// Port of: src/image/SkImage_Raster.cpp#L140-L163 (chrome/m156)
#[doc(alias = "MakeFromBitmap")]
#[must_use]
pub fn make_image_from_raster_bitmap(bitmap: &Bitmap, cpm: CopyPixelsMode) -> Option<Image> {
    ImageRaster::make_from_bitmap(bitmap, cpm, None).map(Image::from_base)
}

/// Creates a CPU-backed [`Image`] from a copy of the pixels of `pixmap` (`RasterFromPixmapCopy`).
///
/// [`Image`] is returned if pixmap is valid: the dimensions are greater than zero and fit in 29
/// bits, the color type is known and the row bytes hold a row, and there are pixels.
// Port of: src/image/SkImage_RasterFactories.cpp#L84-L94 (chrome/m156)
#[doc(alias = "RasterFromPixmapCopy")]
#[must_use]
pub fn raster_from_pixmap_copy(pixmap: &Pixmap<'_>) -> Option<Image> {
    let size = valid_args(pixmap.info(), pixmap.row_bytes())?;
    let addr = pixmap.addr()?;

    // Here we actually make a copy of the caller's pixel data
    let data = Data::new_copy(addr.get(..size)?);
    ImageRaster::from_data(
        pixmap.info(),
        data,
        pixmap.row_bytes(),
        None,
        NEED_NEW_IMAGE_UNIQUE_ID,
    )
    .map(Image::from_base)
}

/// Creates a CPU-backed [`Image`] from the pixels of `pixmap`, and calls `release_proc` when
/// the pixels are no longer needed (`RasterFromPixmap`).
///
/// skia-rust: a pixmap borrows its pixels, so they are copied (as by
/// [`raster_from_pixmap_copy`]) and `release_proc` is called right away, after the copy.
// Port of: src/image/SkImage_RasterFactories.cpp#L141-L150 (chrome/m156)
#[doc(alias = "RasterFromPixmap")]
#[must_use]
pub fn raster_from_pixmap(pixmap: &Pixmap<'_>, release_proc: impl FnOnce()) -> Option<Image> {
    let image = raster_from_pixmap_copy(pixmap);
    release_proc();
    image
}

/// Creates a CPU-backed [`Image`] from `pixels`, which are shared, not copied (`RasterFromData`).
/// `row_bytes` is the size of a pixel row or larger. Returns `None` if the info or row bytes are
/// invalid or there is not enough data.
// Port of: src/image/SkImage_RasterFactories.cpp#L96-L110 (chrome/m156)
#[doc(alias = "RasterFromData")]
#[must_use]
pub fn raster_from_data(
    info: &ImageInfo,
    pixels: impl Into<Data>,
    row_bytes: usize,
) -> Option<Image> {
    let data = pixels.into();
    let size = valid_args(info, row_bytes)?;

    // did they give us enough data?
    if data.size() < size {
        return None;
    }

    ImageRaster::from_data(info, data, row_bytes, None, NEED_NEW_IMAGE_UNIQUE_ID)
        .map(Image::from_base)
}
