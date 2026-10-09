// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/image/SkPictureImageGenerator.{h,cpp}, src/image/SkImage_Picture.cpp,
// src/image/SkImage_LazyFactories.cpp (SkImages::DeferredFromPicture)

//! Picture-backed images: `SkImages::DeferredFromPicture` makes a lazy image whose pixels are the
//! picture drawn on demand by a [`PictureImageGenerator`] (`SkPictureImageGenerator`).
//!
//! The generator draws into a raster canvas, so it lives in this crate (core cannot name the raster
//! device). skia-rust: `SkImage_Picture`'s `replay` and `onMakeSubset` are not ported, because the
//! lazy image is made with [`skia_rust_core::images::deferred_from_generator`] and the drawing goes
//! through the generator.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_base::NEED_NEW_IMAGE_UNIQUE_ID;
use skia_rust_core::image_generator::{ImageGenerator, generator_unique_id};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images::deferred_from_generator;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::picture::Picture;
use skia_rust_core::size::ISize;
use skia_rust_core::surface_props::SurfaceProps;

use crate::surfaces::wrap_pixels;

/// `SkImages::BitDepth`: the bit depth of the pixels a picture image is made with.
// Port of: include/core/SkImage.h (SkImages::BitDepth, chrome/m156)
#[doc(alias = "SkImages::BitDepth")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum BitDepth {
    /// `kU8`: N32 pixels.
    #[default]
    #[doc(alias = "kU8")]
    U8,
    /// `kF16`: RGBA F16 pixels.
    #[doc(alias = "kF16")]
    F16,
}

/// The source of the pixels of a picture image (`SkPictureImageGenerator`): the picture drawn with
/// `matrix` and `paint` into a raster canvas with `props`.
// Port of: src/image/SkPictureImageGenerator.h#L22-L41 (chrome/m156)
#[doc(alias = "SkPictureImageGenerator")]
#[derive(Debug)]
pub struct PictureImageGenerator {
    info: ImageInfo,
    unique_id: u32,
    picture: Picture,
    matrix: Matrix,
    paint: Option<Paint>,
    props: SurfaceProps,
}

impl PictureImageGenerator {
    // Port of: src/image/SkPictureImageGenerator.cpp#L63-L79 (chrome/m156), constructor
    #[doc(alias = "SkPictureImageGenerator")]
    #[must_use]
    pub fn new(
        info: ImageInfo,
        picture: Picture,
        matrix: Option<&Matrix>,
        paint: Option<&Paint>,
        props: SurfaceProps,
    ) -> Self {
        Self {
            info,
            unique_id: generator_unique_id(NEED_NEW_IMAGE_UNIQUE_ID),
            picture,
            // `fMatrix.reset()` when there is no matrix.
            matrix: matrix.cloned().unwrap_or_else(Matrix::new_identity),
            paint: paint.cloned(),
            props,
        }
    }
}

impl ImageGenerator for PictureImageGenerator {
    fn info(&self) -> &ImageInfo {
        &self.info
    }

    fn unique_id(&self) -> u32 {
        self.unique_id
    }

    // Port of: src/image/SkPictureImageGenerator.cpp#L81-L91 (chrome/m156), onGetPixels
    fn on_get_pixels(&mut self, info: &ImageInfo, pixels: &mut [u8], row_bytes: usize) -> bool {
        // `SkCanvas::MakeRasterDirect(info, pixels, rowBytes, &fProps)`: the surface copies the
        // pixels in and back out when it drops, which the drawing below makes the result.
        let Some(mut surface) = wrap_pixels(info, pixels, row_bytes, Some(&self.props)) else {
            return false;
        };
        let canvas = surface.canvas();
        canvas.clear(Color::TRANSPARENT);
        canvas.draw_picture(&self.picture, Some(&self.matrix), self.paint.as_ref());
        true
    }
}

/// `SkImageGenerators::MakeFromPicture`: the generator for a picture image of `size`, or `None`
/// if the colour space is missing or the size is empty.
// Port of: src/image/SkPictureImageGenerator.cpp#L28-L58 (chrome/m156)
#[doc(alias = "MakeFromPicture")]
#[must_use]
pub fn make_from_picture(
    size: impl Into<ISize>,
    picture: Picture,
    matrix: Option<&Matrix>,
    paint: Option<&Paint>,
    bit_depth: BitDepth,
    color_space: Option<ColorSpace>,
    props: SurfaceProps,
) -> Option<Box<dyn ImageGenerator>> {
    let size = size.into();
    let color_space = color_space?;
    if size.is_empty() {
        return None;
    }

    let color_type = match bit_depth {
        BitDepth::U8 => ColorType::n32(),
        BitDepth::F16 => ColorType::RGBAF16,
    };
    let info = ImageInfo::new(size, color_type, AlphaType::Premul, color_space);
    Some(Box::new(PictureImageGenerator::new(
        info, picture, matrix, paint, props,
    )))
}

/// `SkImages::DeferredFromPicture`: an image of `dimensions` whose pixels are `picture` drawn with
/// `matrix` and `paint` when they are needed. `None` if the picture image cannot be made (no colour
/// space, or an empty size).
// Port of: src/image/SkImage_LazyFactories.cpp#L23-L40 (chrome/m156), with SkImage_Picture::Make
// (src/image/SkImage_Picture.cpp#L15-L24)
#[doc(alias = "DeferredFromPicture")]
#[must_use]
pub fn deferred_from_picture(
    picture: Picture,
    dimensions: impl Into<ISize>,
    matrix: Option<&Matrix>,
    paint: Option<&Paint>,
    bit_depth: BitDepth,
    color_space: Option<ColorSpace>,
    props: SurfaceProps,
) -> Option<Image> {
    let generator = make_from_picture(
        dimensions,
        picture,
        matrix,
        paint,
        bit_depth,
        color_space,
        props,
    )?;
    deferred_from_generator(Some(generator))
}
