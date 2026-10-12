// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkImage.h (namespace SkImages), src/image/SkImage_RasterFactories.cpp

//! `SkImages`: the raster image factories (`skia_safe::images`).
//!
//! skia-rust: the lazy and GPU factories are not ported. `MakeWithFilter` is `skia_rust_raster::image_filter_backend::make_with_filter`.

use std::sync::{Mutex, PoisonError};

use crate::alpha_type::AlphaType;
use crate::bitmap::Bitmap;
use crate::compressed_data_utils::{
    compressed_format_data_size, decompress, texture_compression_type_is_opaque,
};
use crate::data::Data;
use crate::image::Image;
use crate::image_base::NEED_NEW_IMAGE_UNIQUE_ID;
use crate::image_generator::ImageGenerator;
use crate::image_info::ImageInfo;
use crate::image_lazy::{ImageLazy, SharedGenerator, Validator};
use crate::image_raster::{CopyPixelsMode, ImageRaster};
use crate::pixmap::Pixmap;
use crate::size::ISize;
use crate::texture_compression_type::TextureCompressionType;

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

/// The pixels of [`raster_from_pixmap`]: the caller's memory, and the proc that releases it.
// Port of: src/core/SkData.cpp#L162-L165 (chrome/m156), `SkData::MakeWithProc`
struct SharedPixels<T> {
    pixels: T,
    // `fReleaseProc` with its context. A `Mutex` makes the closure `Sync`, as `Data` needs.
    release_proc: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl<T: AsRef<[u8]>> AsRef<[u8]> for SharedPixels<T> {
    fn as_ref(&self) -> &[u8] {
        self.pixels.as_ref()
    }
}

impl<T> Drop for SharedPixels<T> {
    // Port of: src/core/SkData.cpp#L28-L33 (chrome/m156), `~SkData`: the release proc is called
    // while the pixels are still there (the fields are dropped after this).
    fn drop(&mut self) {
        let release_proc = self
            .release_proc
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(release_proc) = release_proc {
            release_proc();
        }
    }
}

/// Creates a CPU-backed [`Image`] that shares `pixels`, and calls `release_proc` when the image
/// no longer refers to them (`RasterFromPixmap`). The image reads the pixels as they are in
/// `pixels`; they must not be changed through another handle while the image lives.
///
/// `info` and `row_bytes` describe the pixels. Returns `None`, without calling `release_proc`, if
/// they are not valid for an image, or `pixels` is shorter than the image takes.
///
/// skia-rust: `SkImages::RasterFromPixmap(pixmap, proc, context)` shares the memory a pixmap
/// points to, and the caller keeps it valid until `proc` runs. A safe `Pixmap` borrows its
/// memory, so it cannot be kept by an image; the caller hands over an owner of the memory
/// instead (a [`Data`], a `Vec<u8>`, an `Arc<[u8]>`, ...), and `release_proc` replaces the proc
/// and its context (the closure owns its state). The pixels are released when the last reference
/// to them goes away: the image, and a bitmap that shares its pixel ref (`asLegacyBitmap`),
/// whichever comes last, as in Skia. See `docs/API_MAPPING.md`.
// Port of: src/image/SkImage_RasterFactories.cpp#L161-L170 (chrome/m156)
#[doc(alias = "RasterFromPixmap")]
#[must_use]
pub fn raster_from_pixmap<T>(
    info: &ImageInfo,
    pixels: T,
    row_bytes: usize,
    release_proc: Option<impl FnOnce() + Send + 'static>,
) -> Option<Image>
where
    T: AsRef<[u8]> + Send + Sync + 'static,
{
    let size = valid_args(info, row_bytes)?;
    if pixels.as_ref().len() < size {
        return None;
    }

    // SkData::MakeWithProc(pmap.addr(), size, proc, ctx)
    let data = Data::new_with_owner(SharedPixels {
        pixels,
        release_proc: Mutex::new(
            release_proc.map(|proc| Box::new(proc) as Box<dyn FnOnce() + Send>),
        ),
    });
    ImageRaster::from_data(info, data, row_bytes, None, NEED_NEW_IMAGE_UNIQUE_ID)
        .map(Image::from_base)
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

/// Creates an N32 raster [`Image`] by decoding `data`, which holds one mip level of
/// `compression_type` at `width` x `height` (`RasterFromCompressedTextureData`). Returns `None`
/// if `data` is too small or the decode fails.
// Port of: src/image/SkImage_RasterFactories.cpp#L130-L159 (chrome/m156)
#[doc(alias = "RasterFromCompressedTextureData")]
#[must_use]
pub fn raster_from_compressed_texture_data(
    data: impl Into<Data>,
    width: i32,
    height: i32,
    compression_type: TextureCompressionType,
) -> Option<Image> {
    let data = data.into();
    let dimensions = ISize::new(width, height);
    let expected_size = compressed_format_data_size(compression_type, dimensions, false);
    if data.size() < expected_size {
        return None;
    }

    let at = if texture_compression_type_is_opaque(compression_type) {
        AlphaType::Opaque
    } else {
        AlphaType::Premul
    };

    let ii = ImageInfo::new_n32(dimensions, at, None);

    valid_args(&ii, ii.min_row_bytes())?;

    let mut bitmap = Bitmap::new();
    if !bitmap.try_alloc_pixels_info(&ii, None) {
        return None;
    }

    if !decompress(data.as_bytes(), dimensions, compression_type, &mut bitmap) {
        return None;
    }
    bitmap.set_immutable();
    raster_from_bitmap(&bitmap)
}

/// Creates a lazy [`Image`] whose pixels are made by `generator` when they are needed
/// (`DeferredFromGenerator`). Returns `None` if there is no generator or it has an empty info.
///
/// Codec generators are made by `skia_rust_codec` (`images::deferred_from_encoded_data`,
/// `codecs::deferred_image`).
// Port of: src/image/SkImage_Lazy.cpp#L326-L332 (chrome/m156)
#[doc(alias = "DeferredFromGenerator")]
#[must_use]
pub fn deferred_from_generator(generator: Option<Box<dyn ImageGenerator>>) -> Option<Image> {
    let validator = Validator::new(SharedGenerator::make(generator), None, None);
    if !validator.is_valid() {
        return None;
    }
    ImageLazy::from_validator(validator).map(Image::from_base)
}
