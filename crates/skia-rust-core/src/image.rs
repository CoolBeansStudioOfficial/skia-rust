// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkImage.h, src/image/SkImage.cpp

//! `SkImage`: a two dimensional array of pixels to draw. An [`Image`] is an immutable,
//! thread-safe container for pixel data.
//!
//! [`Image`] is the shared, cheaply clonable handle (`sk_sp<SkImage>`); the implementations
//! provide [`ImageBase`] ([`ImageRaster`](crate::image_raster::ImageRaster) is the only one).
//! The factories (`SkImages::RasterFromBitmap`, ...) are in [`crate::images`].
//!
//! skia-rust: GPU and lazy images (textures, generators, pictures, encoded data), `SkRecorder`s
//! and `GrDirectContext`s, caching hints (a hint only), asynchronous readback, `makeScaled`
//! (it draws into a surface; `skia_rust_raster::images::make_scaled` provides it), `scalePixels`
//! (likewise: `skia_rust_raster::images::scale_pixels`) and encoding are not ported.

use core::fmt;
use std::sync::Arc;

use crate::alpha_type::AlphaType;
use crate::bitmap::Bitmap;
use crate::color_space::ColorSpace;
use crate::color_type::ColorType;
use crate::image_base::ImageBase;
use crate::image_info::ImageInfo;
use crate::image_info_priv::color_type_is_alpha_only;
use crate::images;
use crate::matrix::Matrix;
use crate::mipmap::Mipmap;
use crate::pixmap::Pixmap;
use crate::point::IPoint;
use crate::rect::IRect;
use crate::sampling_options::SamplingOptions;
use crate::shader::Shader;
use crate::shaders::image_shader::ImageShader;
use crate::size::ISize;
use crate::tile_mode::TileMode;

/// Properties an image made from another must have (`SkImage::RequiredProperties`).
// Port of: include/core/SkImage.h#L101-L110 (chrome/m156)
#[doc(alias = "SkImage::RequiredProperties")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct RequiredProperties {
    /// `fMipmapped`.
    pub mipmapped: bool,
}

/// An immutable, shared array of pixels (`sk_sp<SkImage>`): a cheaply clonable handle to an
/// [`ImageBase`].
///
/// Equality is identity, as Skia compares `sk_sp`s ([`Image::ptr_eq`]).
// Port of: include/core/SkImage.h#L48-L110 (chrome/m156)
#[doc(alias = "SkImage")]
#[derive(Clone)]
pub struct Image(Arc<dyn ImageBase>);

impl Image {
    /// Wraps an image implementation.
    #[must_use]
    pub fn from_base(base: impl ImageBase) -> Image {
        Image(Arc::new(base))
    }

    /// The implementation (`as_IB`).
    #[doc(alias = "as_IB")]
    #[must_use]
    pub fn as_base(&self) -> &dyn ImageBase {
        &*self.0
    }

    /// True if `self` and `other` are the same image (Skia's `sk_sp` comparison).
    #[must_use]
    pub fn ptr_eq(&self, other: &Image) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// True if this handle is the only reference to the image (`SkRefCnt::unique`).
    #[must_use]
    pub fn is_unique(&self) -> bool {
        Arc::strong_count(&self.0) == 1
    }

    /// Returns a pixmap over the pixels, if the image has direct access to them
    /// (`peekPixels`).
    // Port of: src/image/SkImage.cpp#L37-L44 (chrome/m156)
    #[doc(alias = "peekPixels")]
    #[must_use]
    pub fn peek_pixels(&self) -> Option<Pixmap<'_>> {
        self.0.on_peek_pixels()
    }

    /// Returns the [`ImageInfo`] describing the width, height, color type, alpha type, and color
    /// space of the image (`imageInfo`).
    #[doc(alias = "imageInfo")]
    #[must_use]
    pub fn image_info(&self) -> &ImageInfo {
        self.0.info()
    }

    /// Returns the pixel count in each row (`width`).
    #[must_use]
    pub fn width(&self) -> i32 {
        self.0.info().width()
    }

    /// Returns the pixel row count (`height`).
    #[must_use]
    pub fn height(&self) -> i32 {
        self.0.info().height()
    }

    /// Returns the [`ISize`] `{ width(), height() }` (`dimensions`).
    #[must_use]
    pub fn dimensions(&self) -> ISize {
        ISize::new(self.width(), self.height())
    }

    /// Returns the [`IRect`] `{ 0, 0, width(), height() }` (`bounds`).
    #[must_use]
    pub fn bounds(&self) -> IRect {
        IRect::from_wh(self.width(), self.height())
    }

    /// Returns a value unique to the image's contents; two images with the same ID have the
    /// same pixels (`uniqueID`).
    #[doc(alias = "uniqueID")]
    #[must_use]
    pub fn unique_id(&self) -> u32 {
        self.0.unique_id()
    }

    /// Returns the [`AlphaType`] (`alphaType`).
    // Port of: src/image/SkImage.cpp#L204 (chrome/m156)
    #[doc(alias = "alphaType")]
    #[must_use]
    pub fn alpha_type(&self) -> AlphaType {
        self.0.info().alpha_type()
    }

    /// Returns the [`ColorType`] (`colorType`).
    // Port of: src/image/SkImage.cpp#L202 (chrome/m156)
    #[doc(alias = "colorType")]
    #[must_use]
    pub fn color_type(&self) -> ColorType {
        self.0.info().color_type()
    }

    /// Returns the [`ColorSpace`], the range of colors, if any (`colorSpace`, `refColorSpace`).
    // Port of: src/image/SkImage.cpp#L206-L208 (chrome/m156)
    #[doc(alias = "colorSpace")]
    #[doc(alias = "refColorSpace")]
    #[must_use]
    pub fn color_space(&self) -> Option<ColorSpace> {
        self.0.info().color_space()
    }

    /// Returns true if the image pixels represent transparency only (`isAlphaOnly`).
    // Port of: src/image/SkImage.cpp#L282 (chrome/m156)
    #[doc(alias = "isAlphaOnly")]
    #[must_use]
    pub fn is_alpha_only(&self) -> bool {
        color_type_is_alpha_only(self.0.info().color_type())
    }

    /// Returns true if pixels ignore their alpha value and are treated as fully opaque
    /// (`isOpaque`).
    #[doc(alias = "isOpaque")]
    #[must_use]
    pub fn is_opaque(&self) -> bool {
        self.alpha_type() == AlphaType::Opaque
    }

    /// Returns true if the image is backed by a GPU texture (`isTextureBacked`).
    #[doc(alias = "isTextureBacked")]
    #[must_use]
    pub fn is_texture_backed(&self) -> bool {
        self.0.is_texture_backed()
    }

    /// Returns true if the image is generated by a lazy generator (`isLazyGenerated`).
    #[doc(alias = "isLazyGenerated")]
    #[must_use]
    pub fn is_lazy_generated(&self) -> bool {
        self.0.is_lazy_generated()
    }

    /// Returns the GPU memory used by the image's texture: 0 for images in memory
    /// (`textureSize`).
    #[doc(alias = "textureSize")]
    #[must_use]
    pub fn texture_size(&self) -> usize {
        0
    }

    /// Whether the image can be drawn by the CPU: raster images always can (`isValid`, the
    /// `recorder` is for the CPU recorder in Skia).
    #[doc(alias = "isValid")]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.0.is_raster_backed()
    }

    /// Returns true if the image has mipmap levels (`hasMipmaps`).
    // Port of: src/image/SkImage.cpp#L330 (chrome/m156)
    #[doc(alias = "hasMipmaps")]
    #[must_use]
    pub fn has_mipmaps(&self) -> bool {
        self.0.on_has_mipmaps()
    }

    /// Returns true if the image is protected (`isProtected`).
    #[doc(alias = "isProtected")]
    #[must_use]
    pub fn is_protected(&self) -> bool {
        self.0.on_is_protected()
    }

    /// Returns an image with the same "base" pixels as this image, but with mipmap levels
    /// automatically generated and attached (`withDefaultMipmaps`).
    // Port of: src/image/SkImage.cpp#L334-L336 (chrome/m156)
    #[doc(alias = "withDefaultMipmaps")]
    #[must_use]
    pub fn with_default_mipmaps(&self) -> Option<Image> {
        Some(self.with_mipmaps(None))
    }

    /// An image with the base pixels of this one and the mipmaps `mips` (built if `None`); this
    /// image if they do not fit (`withMipmaps`).
    // Port of: src/image/SkImage.cpp#L322-L328 (chrome/m156)
    #[doc(alias = "withMipmaps")]
    #[must_use]
    pub fn with_mipmaps(&self, mips: Option<Arc<Mipmap>>) -> Image {
        if mips
            .as_ref()
            .is_none_or(|m| m.valid_for_root_level(self.image_info()))
            && let Some(result) = self.0.on_make_with_mipmaps(mips)
        {
            return result;
        }
        self.clone()
    }

    /// Returns a shader with the specified tiling and sampling (`makeShader`). `tile_modes` of
    /// `None` is clamp in both directions; `local_matrix` of `None` is the identity.
    ///
    /// Returns `None` if the sampling options or the matrix are invalid.
    // Port of: src/image/SkImage.cpp#L134-L164 (chrome/m156)
    #[doc(alias = "makeShader")]
    #[must_use]
    pub fn to_shader<'a>(
        &self,
        tile_modes: impl Into<Option<(TileMode, TileMode)>>,
        sampling: impl Into<SamplingOptions>,
        local_matrix: impl Into<Option<&'a Matrix>>,
    ) -> Option<Shader> {
        let (tmx, tmy) = tile_modes
            .into()
            .unwrap_or((TileMode::Clamp, TileMode::Clamp));
        ImageShader::make(
            Some(self.clone()),
            tmx,
            tmy,
            &sampling.into(),
            local_matrix.into(),
            false,
        )
    }

    /// Like [`to_shader`](Self::to_shader), for images that contain non-color data: no color
    /// space transformation is ever applied, `Unpremul` images are not premultiplied, and
    /// bicubic filtering is not supported (`None` is returned for it) (`makeRawShader`).
    // Port of: src/image/SkImage.cpp#L166-L196 (chrome/m156)
    #[doc(alias = "makeRawShader")]
    #[must_use]
    pub fn to_raw_shader<'a>(
        &self,
        tile_modes: impl Into<Option<(TileMode, TileMode)>>,
        sampling: impl Into<SamplingOptions>,
        local_matrix: impl Into<Option<&'a Matrix>>,
    ) -> Option<Shader> {
        let (tmx, tmy) = tile_modes
            .into()
            .unwrap_or((TileMode::Clamp, TileMode::Clamp));
        ImageShader::make_raw(
            Some(self.clone()),
            tmx,
            tmy,
            &sampling.into(),
            local_matrix.into(),
        )
    }

    /// Copies a rectangle of pixels from the image to `dst_pixels`. Copy starts at
    /// `src` and does not exceed the image (`readPixels`).
    ///
    /// `dst_info` specifies width, height, color type, alpha type, and color space of the
    /// destination; `dst_row_bytes` the row length. Returns true if pixels are copied.
    ///
    /// skia-rust: the destination is a byte slice, and there is no caching hint.
    // Port of: src/image/SkImage.cpp#L46-L50 (chrome/m156)
    #[doc(alias = "readPixels")]
    pub fn read_pixels(
        &self,
        dst_info: &ImageInfo,
        dst_pixels: &mut [u8],
        dst_row_bytes: usize,
        src: impl Into<IPoint>,
    ) -> bool {
        let IPoint { x, y } = src.into();
        self.0
            .on_read_pixels(dst_info, dst_pixels, dst_row_bytes, x, y)
    }

    /// [`read_pixels`](Self::read_pixels) into a pixmap (`readPixels(dContext, pmap, ...)`).
    // Port of: src/image/SkImage.cpp#L228-L232 (chrome/m156)
    #[doc(alias = "readPixels")]
    pub fn read_pixels_to_pixmap(&self, dst: &mut Pixmap<'_>, src: impl Into<IPoint>) -> bool {
        let info = dst.info().clone();
        let row_bytes = dst.row_bytes();
        let Some(pixels) = dst.writable_addr() else {
            return false;
        };
        self.read_pixels(&info, pixels, row_bytes, src)
    }

    /// Returns a read-only copy of the pixels as a bitmap sharing them when it can
    /// (`getROPixels`).
    #[doc(alias = "getROPixels")]
    #[must_use]
    pub fn get_ro_pixels(&self) -> Option<Bitmap> {
        self.0.get_ro_pixels()
    }

    /// A bitmap with the image's pixels, an N32 copy in no color space unless the pixels can be
    /// shared (`asLegacyBitmap`).
    // Port of: src/image/SkImage.cpp#L238-L241 (chrome/m156)
    #[doc(alias = "asLegacyBitmap")]
    #[must_use]
    pub fn as_legacy_bitmap(&self) -> Option<Bitmap> {
        self.0.on_as_legacy_bitmap()
    }

    /// Returns a new image containing `subset` of this image, or `None` if `subset` is empty or
    /// not inside the image (`makeSubset`).
    ///
    /// skia-rust: there is no recorder.
    #[doc(alias = "makeSubset")]
    #[must_use]
    pub fn make_subset(
        &self,
        subset: impl AsRef<IRect>,
        required_properties: RequiredProperties,
    ) -> Option<Image> {
        self.0.make_subset(subset.as_ref(), required_properties)
    }

    /// Returns an image converted to `new_color_space`, or `None` (`makeColorSpace`).
    #[doc(alias = "makeColorSpace")]
    #[must_use]
    pub fn make_color_space(
        &self,
        new_color_space: impl Into<Option<ColorSpace>>,
        required_properties: RequiredProperties,
    ) -> Option<Image> {
        self.0
            .make_color_space(new_color_space.into(), required_properties)
    }

    /// Returns an image converted to `color_type` and `color_space` (`makeColorTypeAndColorSpace`).
    #[doc(alias = "makeColorTypeAndColorSpace")]
    #[must_use]
    pub fn make_color_type_and_color_space(
        &self,
        color_type: ColorType,
        color_space: impl Into<Option<ColorSpace>>,
        required_properties: RequiredProperties,
    ) -> Option<Image> {
        self.0
            .make_color_type_and_color_space(color_type, color_space.into(), required_properties)
    }

    /// Returns an image with the same pixels interpreted in `new_color_space` (no conversion)
    /// (`reinterpretColorSpace`).
    // Port of: src/image/SkImage.cpp#L284-L301 (chrome/m156)
    #[doc(alias = "reinterpretColorSpace")]
    #[must_use]
    pub fn reinterpret_color_space(&self, new_color_space: impl Into<ColorSpace>) -> Option<Image> {
        let target = new_color_space.into();

        // No need to create a new image if:
        // (1) The color spaces are equal.
        // (2) The color type is kAlpha8.
        let color_space = self.color_space().unwrap_or_else(ColorSpace::new_srgb);
        if ColorSpace::equals(Some(&color_space), Some(&target)) || self.is_alpha_only() {
            return Some(self.clone());
        }

        self.0.on_reinterpret_color_space(target)
    }

    /// Returns this image if it is not texture backed; images made by this crate never are
    /// (`makeNonTextureImage`).
    // Port of: src/image/SkImage.cpp#L303-L308 (chrome/m156)
    #[doc(alias = "makeNonTextureImage")]
    #[must_use]
    pub fn to_non_texture_image(&self) -> Option<Image> {
        Some(self.clone())
    }

    /// An image with the same pixels, in memory (`makeRasterImage`).
    // Port of: src/image/SkImage.cpp#L310-L338 (chrome/m156)
    #[doc(alias = "makeRasterImage")]
    #[must_use]
    pub fn to_raster_image(&self) -> Option<Image> {
        if self.peek_pixels().is_some() {
            return Some(self.clone());
        }

        let info = self.image_info();
        let row_bytes = info.min_row_bytes();
        let size = info.compute_byte_size(row_bytes);
        if ImageInfo::byte_size_overflowed(size) {
            return None;
        }

        let mut bytes = vec![0u8; size];
        let dst_info = info.with_color_space(None);
        if !self.read_pixels(&dst_info, &mut bytes, row_bytes, (0, 0)) {
            return None;
        }

        images::raster_from_data(info, crate::data::Data::new_from_vec(bytes), row_bytes)
    }
}

impl AsRef<Image> for Image {
    fn as_ref(&self) -> &Image {
        self
    }
}

impl PartialEq for Image {
    /// Identity, as Skia's `sk_sp<SkImage>` `operator==`.
    fn eq(&self, other: &Image) -> bool {
        self.ptr_eq(other)
    }
}

impl fmt::Debug for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Image").field(&self.0).finish()
    }
}
