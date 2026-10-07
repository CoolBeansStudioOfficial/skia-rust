// Copyright 2008 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkBitmap.h, src/core/SkBitmap.cpp

//! [`Bitmap`]: a two-dimensional raster pixel array.
//!
//! Not ported (they need drawing, `SkConvertPixels`, `SkImage`, shaders or mask filters):
//! `readPixels` (2 overloads), `writePixels`, `extractAlpha`, `asImage`, `makeShader` (4
//! overloads), `getBounds` (out-parameter forms; use [`Bitmap::bounds`]), custom
//! `SkBitmap::Allocator`s (only the default heap allocation exists), and `installPixels(const
//! SkPixmap&)` (a pixmap borrows its bytes, so a bitmap cannot share them; install an owned copy
//! with [`Bitmap::install_pixels`] instead).

use crate::alpha_type::AlphaType;
use crate::color::{Color, Color4f};
use crate::color_space::ColorSpace;
use crate::color_type::ColorType;
use crate::image_info::ImageInfo;
use crate::malloc_pixel_ref;
use crate::pixel_ref::{PixelRef, ReleaseProc};
use crate::pixel_ref_priv::make_pixel_ref_with_proc;
use crate::pixmap::Pixmap;
use crate::point::IPoint;
use crate::rect::IRect;
use crate::size::ISize;
use crate::t_fits_in::t_fits_in;
use std::mem;

/// Describes a two-dimensional raster pixel array. [`Bitmap`] is built on [`ImageInfo`],
/// containing integer width and height, [`ColorType`] and [`AlphaType`] describing the pixel
/// format, and [`ColorSpace`] describing the range of colors.
///
/// [`Bitmap`] points to [`PixelRef`], which describes the physical array of pixels.
/// [`ImageInfo`] bounds may be located anywhere fully inside the [`PixelRef`] bounds.
///
/// [`Bitmap`] can be drawn using `Canvas`. [`Bitmap`] can be a drawing destination for `Canvas`
/// draw member functions. [`Bitmap`] flexibility as a pixel container limits some optimizations
/// available to the target platform.
///
/// If the pixel array is primarily read-only, use `Image` for better performance. If the pixel
/// array is primarily written to, use `Surface` for better performance.
///
/// Cloning a [`Bitmap`] copies its [`ImageInfo`] and shares the [`PixelRef`], so both bitmaps
/// reference the same pixels. A `&Bitmap` is enough to change the pixels (`erase_color`, ...),
/// as in C++ where those methods are `const`.
///
/// # Pixel access
/// [`Bitmap::peek_pixels`] and [`Bitmap::pixmap`] borrow the pixels for reading,
/// [`Bitmap::peek_pixels_mut`] for writing. The borrows hold a lock on the shared pixels: do not
/// call another pixel-touching method of a bitmap sharing the same [`PixelRef`] (such as
/// [`Bitmap::erase_color`]) on the same thread while one is alive.
// Port of: include/core/SkBitmap.h#L51-L1200 (chrome/m156)
#[doc(alias = "SkBitmap")]
#[derive(Clone, Debug, Default)]
pub struct Bitmap {
    pixel_ref: Option<PixelRef>,
    // fPixmap: the info and row bytes, and the byte offset of the first pixel in the pixel
    // ref's storage (`fPixmap.addr() - fPixelRef->pixels()`); `None` if `fPixmap.addr()` is
    // null.
    info: ImageInfo,
    row_bytes: usize,
    offset: Option<usize>,
}

impl Bitmap {
    /// Creates an empty [`Bitmap`] without pixels, with [`ColorType::Unknown`], with
    /// [`AlphaType::Unknown`], and with a width and height of zero. The [`PixelRef`] origin is
    /// set to `(0, 0)`.
    ///
    /// Use [`Self::set_info()`] to associate color type, alpha type, width, and height after the
    /// [`Bitmap`] has been created.
    // Port of: src/core/SkBitmap.cpp#L43 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Swaps the fields of this [`Bitmap`] and `other`. Exchanges the [`PixelRef`] and origin,
    /// and the [`ImageInfo`].
    // Port of: src/core/SkBitmap.cpp#L77-L81 (chrome/m156)
    pub fn swap(&mut self, other: &mut Self) {
        mem::swap(self, other);
    }

    /// Returns the pixmap describing the pixels: [`ImageInfo`], pixel address, and row bytes
    /// (read-only; see [`Self::peek_pixels_mut`]). The pixmap has no pixels if this bitmap has
    /// none.
    // Port of: include/core/SkBitmap.h#L121 (chrome/m156)
    #[must_use]
    pub fn pixmap(&self) -> Pixmap<'_> {
        match (&self.pixel_ref, self.offset) {
            (Some(pixel_ref), Some(offset)) => Pixmap::from_read_guard(
                self.info.clone(),
                pixel_ref.pixels(),
                offset,
                self.row_bytes,
            ),
            _ => Pixmap::without_pixels(self.info.clone(), self.row_bytes),
        }
    }

    /// Returns the width, height, [`AlphaType`], [`ColorType`], and [`ColorSpace`].
    // Port of: include/core/SkBitmap.h#L127 (chrome/m156)
    #[must_use]
    pub fn info(&self) -> &ImageInfo {
        &self.info
    }

    /// Returns the pixel count in each row. Should be equal or less than
    /// `row_bytes() / info().bytes_per_pixel()`.
    ///
    /// May be less than `pixel_ref().width()`. Will not exceed `pixel_ref().width()` less
    /// `pixel_ref_origin().x`.
    // Port of: include/core/SkBitmap.h#L137 (chrome/m156)
    #[must_use]
    pub fn width(&self) -> i32 {
        self.info.width()
    }

    /// Returns the pixel row count.
    ///
    /// May be less than `pixel_ref().height()`. Will not exceed `pixel_ref().height()` less
    /// `pixel_ref_origin().y`.
    // Port of: include/core/SkBitmap.h#L146 (chrome/m156)
    #[must_use]
    pub fn height(&self) -> i32 {
        self.info.height()
    }

    /// Returns the color type.
    // Port of: include/core/SkBitmap.h#L148 (chrome/m156)
    #[doc(alias = "colorType")]
    #[must_use]
    pub fn color_type(&self) -> ColorType {
        self.info.color_type()
    }

    /// Returns the alpha type.
    // Port of: include/core/SkBitmap.h#L150 (chrome/m156)
    #[doc(alias = "alphaType")]
    #[must_use]
    pub fn alpha_type(&self) -> AlphaType {
        self.info.alpha_type()
    }

    /// Returns the color space, or `None`.
    // Port of: src/core/SkBitmap.cpp#L98-L100 (chrome/m156)
    #[doc(alias = "colorSpace")]
    #[doc(alias = "refColorSpace")]
    #[must_use]
    pub fn color_space(&self) -> Option<ColorSpace> {
        self.info.color_space()
    }

    /// Returns the number of bytes per pixel required by the color type. Returns zero if the
    /// color type is [`ColorType::Unknown`].
    // Port of: include/core/SkBitmap.h#L174 (chrome/m156)
    #[doc(alias = "bytesPerPixel")]
    #[must_use]
    pub fn bytes_per_pixel(&self) -> usize {
        self.info.bytes_per_pixel()
    }

    /// Returns the number of pixels that fit on a row: `row_bytes() / info().bytes_per_pixel()`.
    /// May be larger than `width()`.
    // Port of: include/core/SkBitmap.h#L182 (chrome/m156)
    #[doc(alias = "rowBytesAsPixels")]
    #[must_use]
    pub fn row_bytes_as_pixels(&self) -> usize {
        self.row_bytes >> self.shift_per_pixel()
    }

    /// Returns the bit shift converting row bytes to row pixels. Returns zero for
    /// [`ColorType::Unknown`].
    // Port of: include/core/SkBitmap.h#L190 (chrome/m156)
    #[doc(alias = "shiftPerPixel")]
    #[must_use]
    pub fn shift_per_pixel(&self) -> usize {
        self.info.shift_per_pixel()
    }

    /// Returns true if either `width()` or `height()` are zero.
    ///
    /// Does not check if the [`PixelRef`] is `None`; call [`Self::draws_nothing()`] to check
    /// `width()`, `height()`, and the [`PixelRef`].
    // Port of: include/core/SkBitmap.h#L204 (chrome/m156)
    #[doc(alias = "empty")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.info.is_empty()
    }

    /// Returns true if the [`PixelRef`] is `None`.
    ///
    /// Does not check if `width()` or `height()` are zero; call [`Self::draws_nothing()`] to
    /// check `width()`, `height()`, and the [`PixelRef`].
    // Port of: include/core/SkBitmap.h#L215 (chrome/m156)
    #[doc(alias = "isNull")]
    #[must_use]
    pub fn is_null(&self) -> bool {
        self.pixel_ref.is_none()
    }

    /// Returns true if `width()` or `height()` are zero, or if the [`PixelRef`] is `None`. If
    /// true, the bitmap has no effect when drawn or drawn into.
    // Port of: include/core/SkBitmap.h#L223-L225 (chrome/m156)
    #[doc(alias = "drawsNothing")]
    #[must_use]
    pub fn draws_nothing(&self) -> bool {
        self.is_empty() || self.is_null()
    }

    /// Returns the row bytes: the interval from one pixel row to the next. Row bytes is at least
    /// as large as `width() * info().bytes_per_pixel()`.
    ///
    /// Returns zero if the color type is [`ColorType::Unknown`], or if row bytes supplied to
    /// [`Self::set_info()`] is not large enough to hold a row of pixels.
    // Port of: include/core/SkBitmap.h#L235 (chrome/m156)
    #[doc(alias = "rowBytes")]
    #[must_use]
    pub fn row_bytes(&self) -> usize {
        self.row_bytes
    }

    /// Sets the alpha type, if it is compatible with the color type; returns true if it was set.
    ///
    /// Returns true unless `alpha_type` is [`AlphaType::Unknown`] and the current alpha type is
    /// not [`AlphaType::Unknown`].
    ///
    /// Returns true if the color type is [`ColorType::Unknown`]; `alpha_type` is ignored, and the
    /// alpha type remains [`AlphaType::Unknown`].
    ///
    /// Returns true if the color type is [`ColorType::RGB565`] or [`ColorType::Gray8`];
    /// `alpha_type` is ignored, and the alpha type remains [`AlphaType::Opaque`].
    ///
    /// If the color type is [`ColorType::ARGB4444`], [`ColorType::RGBA8888`],
    /// [`ColorType::BGRA8888`], or [`ColorType::RGBAF16`]: returns true unless `alpha_type` is
    /// [`AlphaType::Unknown`] and the alpha type is not [`AlphaType::Unknown`]. If the alpha type
    /// is [`AlphaType::Unknown`], `alpha_type` is ignored.
    ///
    /// If the color type is [`ColorType::Alpha8`], returns true unless `alpha_type` is
    /// [`AlphaType::Unknown`] and the alpha type is not [`AlphaType::Unknown`]. If `alpha_type`
    /// is [`AlphaType::Unpremul`], it is treated as [`AlphaType::Premul`].
    ///
    /// This changes the alpha type in the [`PixelRef`]; all bitmaps sharing it are affected.
    // Port of: src/core/SkBitmap.cpp#L138-L148 (chrome/m156)
    #[doc(alias = "setAlphaType")]
    pub fn set_alpha_type(&mut self, alpha_type: AlphaType) -> bool {
        let Some(new_alpha_type) = self.color_type().validate_alpha_type(alpha_type) else {
            return false;
        };
        if self.alpha_type() != new_alpha_type {
            self.info = self.info.with_alpha_type(new_alpha_type);
        }
        self.validate();
        true
    }

    /// Sets the color space, replacing the one in the [`ImageInfo`]. The raw pixel data is not
    /// altered by this call; no conversion is performed.
    // Port of: src/core/SkBitmap.cpp#L150-L156 (chrome/m156)
    #[doc(alias = "setColorSpace")]
    pub fn set_color_space(&mut self, color_space: impl Into<Option<ColorSpace>>) {
        let new_color_space = color_space.into();
        if self.info.color_space() != new_color_space {
            self.info = self.info.with_color_space(new_color_space);
        }
        self.validate();
    }

    /// Returns the size in bytes of the image buffer that `row_bytes()` and the image info
    /// require. Does not include unused memory on the last row when
    /// `row_bytes_as_pixels()` exceeds `width()`.
    ///
    /// Returns `usize::MAX` if the result does not fit in `usize`. Returns zero if `height()` or
    /// `width()` is 0. Returns `height()` times `row_bytes()` if the color type is
    /// [`ColorType::Unknown`].
    // Port of: include/core/SkBitmap.h#L282 (chrome/m156)
    #[doc(alias = "computeByteSize")]
    #[must_use]
    pub fn compute_byte_size(&self) -> usize {
        self.info.compute_byte_size(self.row_bytes)
    }

    /// Returns true if the pixels can not change.
    ///
    /// Most immutable bitmap checks trigger an assert only on debug builds.
    // Port of: src/core/SkBitmap.cpp#L376-L378 (chrome/m156)
    #[doc(alias = "isImmutable")]
    #[must_use]
    pub fn is_immutable(&self) -> bool {
        self.pixel_ref.as_ref().is_some_and(PixelRef::is_immutable)
    }

    /// Sets the internal flag to mark the bitmap as immutable. Once the [`PixelRef`] is marked
    /// immutable, the setting cannot be cleared. Any other bitmap sharing the same [`PixelRef`]
    /// are also marked as immutable.
    // Port of: src/core/SkBitmap.cpp#L380-L384 (chrome/m156)
    #[doc(alias = "setImmutable")]
    pub fn set_immutable(&mut self) {
        if let Some(pixel_ref) = &self.pixel_ref {
            pixel_ref.set_immutable();
        }
    }

    /// Returns true if the alpha type is [`AlphaType::Opaque`]. Does not check if the color type
    /// allows alpha, or if any pixel value has transparency.
    // Port of: include/core/SkBitmap.h#L318-L320 (chrome/m156)
    #[doc(alias = "isOpaque")]
    #[must_use]
    pub fn is_opaque(&self) -> bool {
        self.alpha_type().is_opaque()
    }

    /// Resets to its initial state; all fields are set to zero, as if the [`Bitmap`] had been
    /// initialized by [`Bitmap::new()`].
    ///
    /// Sets width, height, row bytes to zero; pixel address to `None`; color type to
    /// [`ColorType::Unknown`]; and alpha type to [`AlphaType::Unknown`]. If the [`PixelRef`] is
    /// allocated, its reference count is decreased by one, releasing its memory if the [`Bitmap`]
    /// is the sole owner.
    // Port of: src/core/SkBitmap.cpp#L83-L86 (chrome/m156)
    pub fn reset(&mut self) {
        self.pixel_ref = None; // Free pixels.
        self.info = ImageInfo::new_unknown(None);
        self.row_bytes = 0;
        self.offset = None;
    }

    /// Returns true if all pixels are opaque. [`ColorType`] determines how pixels are encoded,
    /// and whether pixel describes alpha. Returns true for color types without alpha in each
    /// pixel; for other color types, returns true if all pixels have alpha values equivalent to
    /// 1.0 or greater.
    ///
    /// For [`ColorType::RGB565`] or [`ColorType::Gray8`]: always returns true. For
    /// [`ColorType::Alpha8`], [`ColorType::BGRA8888`], [`ColorType::RGBA8888`]: returns true if
    /// all pixel alpha values are 255. For [`ColorType::ARGB4444`]: returns true if all pixel
    /// alpha values are 15. For [`ColorType::RGBAF16`]: returns true if all pixel alpha values
    /// are 1.0 or greater.
    ///
    /// Returns false for [`ColorType::Unknown`].
    // Port of: include/core/SkBitmap.h#L355-L357 (chrome/m156)
    #[doc(alias = "ComputeIsOpaque")]
    #[must_use]
    pub fn compute_is_opaque(bm: &Self) -> bool {
        bm.pixmap().compute_is_opaque()
    }

    /// Returns the integral rectangle from the origin to `width()` and `height()`.
    // Port of: include/core/SkBitmap.h#L370 (chrome/m156)
    #[must_use]
    pub fn bounds(&self) -> IRect {
        self.info.bounds()
    }

    /// Returns the integral size of `width()` and `height()`.
    // Port of: include/core/SkBitmap.h#L375 (chrome/m156)
    #[must_use]
    pub fn dimensions(&self) -> ISize {
        self.info.dimensions()
    }

    /// Returns the bounds of this bitmap within the [`PixelRef`] bounds.
    ///
    /// Multiple [`Bitmap`]s can share the same [`PixelRef`], where each has different bounds.
    /// The returned origin added to the dimensions equals or is smaller than the [`PixelRef`]
    /// dimensions.
    // Port of: include/core/SkBitmap.h#L383-L386 (chrome/m156)
    #[doc(alias = "getSubset")]
    #[must_use]
    pub fn get_subset(&self) -> IRect {
        let origin = self.pixel_ref_origin();
        IRect::from_xywh(origin.x, origin.y, self.width(), self.height())
    }

    /// Sets width, height, [`AlphaType`], [`ColorType`], [`ColorSpace`], and optional
    /// `row_bytes`. Frees pixels, and returns true if successful.
    ///
    /// The alpha type of `image_info` may be altered to a value permitted by its color type: if
    /// the color type is [`ColorType::Unknown`], the alpha type is set to
    /// [`AlphaType::Unknown`]; if it is [`ColorType::Alpha8`] and the alpha type is
    /// [`AlphaType::Unpremul`], the alpha type is replaced by [`AlphaType::Premul`]; if it is
    /// [`ColorType::RGB565`] or [`ColorType::Gray8`], the alpha type is set to
    /// [`AlphaType::Opaque`].
    ///
    /// `row_bytes` must equal or exceed `image_info.min_row_bytes()`. If the color type is
    /// [`ColorType::Unknown`], `row_bytes` is ignored and treated as zero; for all other color
    /// types, a `row_bytes` of zero (or `None`) is treated as `image_info.min_row_bytes()`.
    ///
    /// Calls [`Self::reset()`] and returns false if:
    /// - `row_bytes` exceeds 31 bits
    /// - `image_info.width()` is negative
    /// - `image_info.height()` is negative
    /// - `row_bytes` is positive and less than `image_info.width()` times
    ///   `image_info.bytes_per_pixel()`
    // Port of: src/core/SkBitmap.cpp#L104-L136 (chrome/m156)
    #[doc(alias = "setInfo")]
    #[must_use]
    pub fn set_info(
        &mut self,
        image_info: &ImageInfo,
        row_bytes: impl Into<Option<usize>>,
    ) -> bool {
        let mut row_bytes = row_bytes.into().unwrap_or(0);

        let Some(new_alpha_type) = image_info
            .color_type()
            .validate_alpha_type(image_info.alpha_type())
        else {
            return self.reset_return_false();
        };
        // don't look at info.alphaType(), since new_alpha_type is the real value...

        // require that rowBytes fit in 31bits
        let mrb = image_info.min_row_bytes64().cast_signed();
        if !t_fits_in::<i32, i64>(mrb) {
            return self.reset_return_false();
        }
        if !t_fits_in::<i32, usize>(row_bytes) {
            return self.reset_return_false();
        }

        if image_info.width() < 0 || image_info.height() < 0 {
            return self.reset_return_false();
        }

        if ColorType::Unknown == image_info.color_type() {
            row_bytes = 0;
        } else if 0 == row_bytes {
            // mrb was checked to fit in an i32 and is non-negative
            row_bytes = usize::try_from(mrb).unwrap_or(0);
        } else if !image_info.valid_row_bytes(row_bytes) {
            return self.reset_return_false();
        }

        self.pixel_ref = None; // Free pixels.
        self.info = image_info.with_alpha_type(new_alpha_type);
        self.row_bytes = row_bytes; // SkToU32(rowBytes)
        self.offset = None;
        self.validate();
        true
    }

    // Port of: src/core/SkBitmap.cpp#L38-L41 (chrome/m156)
    fn reset_return_false(&mut self) -> bool {
        self.reset();
        false
    }

    /// Sets [`ImageInfo`] to `image_info` following the rules in [`Self::set_info()`], and
    /// allocates pixel memory. Memory is zeroed.
    ///
    /// Returns false and calls [`Self::reset()`] if [`ImageInfo`] could not be set, or memory
    /// could not be allocated.
    // Port of: src/core/SkBitmap.cpp#L286-L305 (chrome/m156)
    #[doc(alias = "tryAllocPixelsFlags")]
    #[must_use]
    pub fn try_alloc_pixels_flags(&mut self, image_info: &ImageInfo) -> bool {
        if !self.set_info(image_info, None) {
            return self.reset_return_false();
        }

        // set_info may have corrected info (e.g. 565 is always opaque).
        let corrected_info = self.info.clone();

        let Some(pixel_ref) =
            malloc_pixel_ref::make_allocate(&corrected_info, corrected_info.min_row_bytes())
        else {
            return self.reset_return_false();
        };
        self.set_pixel_ref(Some(pixel_ref), (0, 0));
        if self.offset.is_none() {
            return self.reset_return_false();
        }
        self.validate();
        true
    }

    /// Like [`Self::try_alloc_pixels_flags`].
    ///
    /// # Panics
    /// If the allocation fails.
    // Port of: src/core/SkBitmap.cpp#L241-L246 (chrome/m156)
    #[doc(alias = "allocPixelsFlags")]
    pub fn alloc_pixels_flags(&mut self, image_info: &ImageInfo) {
        assert!(
            self.try_alloc_pixels_flags(image_info),
            "Bitmap::try_alloc_pixels_flags failed ColorType:{:?} AlphaType:{:?} [w:{} h:{}] rb:{}",
            image_info.color_type(),
            image_info.alpha_type(),
            image_info.width(),
            image_info.height(),
            self.row_bytes
        );
    }

    /// Sets [`ImageInfo`] to `image_info` following the rules in [`Self::set_info()`] and
    /// allocates pixel memory. `row_bytes` must equal or exceed
    /// `image_info.width()` times `image_info.bytes_per_pixel()`, or equal `None` (or zero). Pass
    /// `None` for `row_bytes` to compute the minimum valid value.
    ///
    /// Returns false and calls [`Self::reset()`] if [`ImageInfo`] could not be set, or memory
    /// could not be allocated.
    // Port of: src/core/SkBitmap.cpp#L261-L284 (chrome/m156)
    #[doc(alias = "tryAllocPixels")]
    #[must_use]
    pub fn try_alloc_pixels_info(
        &mut self,
        image_info: &ImageInfo,
        row_bytes: impl Into<Option<usize>>,
    ) -> bool {
        let row_bytes = row_bytes
            .into()
            .unwrap_or_else(|| image_info.min_row_bytes());
        if !self.set_info(image_info, row_bytes) {
            return self.reset_return_false();
        }

        // set_info may have corrected info (e.g. 565 is always opaque).
        if ColorType::Unknown == self.info.color_type() {
            return true;
        }
        // set_info may have computed a valid rowbytes if 0 were passed in
        let row_bytes = self.row_bytes;

        let Some(pixel_ref) = malloc_pixel_ref::make_allocate(&self.info, row_bytes) else {
            return self.reset_return_false();
        };
        self.set_pixel_ref(Some(pixel_ref), (0, 0));
        if self.offset.is_none() {
            return self.reset_return_false();
        }
        self.validate();
        true
    }

    /// Like [`Self::try_alloc_pixels_info`].
    ///
    /// # Panics
    /// If [`ImageInfo`] could not be set, or memory could not be allocated.
    // Port of: src/core/SkBitmap.cpp#L248-L257 (chrome/m156)
    #[doc(alias = "allocPixels")]
    pub fn alloc_pixels_info(
        &mut self,
        image_info: &ImageInfo,
        row_bytes: impl Into<Option<usize>>,
    ) {
        assert!(
            self.try_alloc_pixels_info(image_info, row_bytes),
            "Bitmap::alloc_pixels_info failed ColorType:{:?} AlphaType:{:?} [w:{} h:{}] rb:{}",
            image_info.color_type(),
            image_info.alpha_type(),
            image_info.width(),
            image_info.height(),
            self.row_bytes
        );
    }

    /// Sets [`ImageInfo`] to width, height, and native color type; and allocates pixel memory.
    /// If `is_opaque` is true, sets [`ImageInfo`] to [`AlphaType::Opaque`]; otherwise, sets to
    /// [`AlphaType::Premul`].
    ///
    /// Calls [`Self::reset()`] and returns false if width exceeds 29 bits or is negative, or
    /// height is negative. Returns false if allocation fails.
    ///
    /// Use to create a [`Bitmap`] that matches [`PMColor`](crate::color::PMColor), the native
    /// pixel arrangement on the platform.
    // Port of: src/core/SkBitmap.cpp#L216-L220 (chrome/m156)
    #[doc(alias = "tryAllocN32Pixels")]
    #[must_use]
    pub fn try_alloc_n32_pixels(
        &mut self,
        dimensions: impl Into<ISize>,
        is_opaque: impl Into<Option<bool>>,
    ) -> bool {
        let is_opaque = is_opaque.into().unwrap_or(false);
        let info = ImageInfo::new_n32(
            dimensions,
            if is_opaque {
                AlphaType::Opaque
            } else {
                AlphaType::Premul
            },
            None,
        );
        self.try_alloc_pixels_info(&info, None)
    }

    /// Like [`Self::try_alloc_n32_pixels`].
    ///
    /// # Panics
    /// If allocation fails.
    // Port of: src/core/SkBitmap.cpp#L222-L226 (chrome/m156)
    #[doc(alias = "allocN32Pixels")]
    pub fn alloc_n32_pixels(
        &mut self,
        dimensions: impl Into<ISize>,
        is_opaque: impl Into<Option<bool>>,
    ) {
        let is_opaque = is_opaque.into().unwrap_or(false);
        let info = ImageInfo::new_n32(
            dimensions,
            if is_opaque {
                AlphaType::Opaque
            } else {
                AlphaType::Premul
            },
            None,
        );
        self.alloc_pixels_info(&info, None);
    }

    /// Sets [`ImageInfo`] to `image_info` following the rules in [`Self::set_info()`], and
    /// creates a [`PixelRef`] owning `pixels`, with `row_bytes` between rows.
    ///
    /// If [`ImageInfo`] could not be set, or `row_bytes` is less than
    /// `image_info.min_row_bytes()`, or `pixels` is too small to hold the image: calls
    /// [`Self::reset()`], and returns false. Otherwise, if `pixels` is `None`: sets [`ImageInfo`]
    /// and returns true, as if [`Self::set_info()`] were called.
    ///
    /// skia-rust: the bitmap takes ownership of the bytes; the C++ borrows caller-owned memory.
    // Port of: src/core/SkBitmap.cpp#L334-L337 (chrome/m156)
    #[doc(alias = "installPixels")]
    #[must_use]
    pub fn install_pixels(
        &mut self,
        image_info: &ImageInfo,
        pixels: impl Into<Option<Vec<u8>>>,
        row_bytes: usize,
    ) -> bool {
        self.install_pixels_with_proc(image_info, pixels, row_bytes, None)
    }

    /// Like [`Self::install_pixels`], with a `release_proc` that is called with the pixels when
    /// they are no longer referenced; or immediately (with no bytes) on failure or if `pixels`
    /// is `None`.
    // Port of: src/core/SkBitmap.cpp#L307-L332 (chrome/m156)
    #[doc(alias = "installPixels")]
    #[must_use]
    pub fn install_pixels_with_proc(
        &mut self,
        image_info: &ImageInfo,
        pixels: impl Into<Option<Vec<u8>>>,
        row_bytes: usize,
        release_proc: Option<ReleaseProc>,
    ) -> bool {
        fn invoke_release_proc(proc: Option<ReleaseProc>, pixels: Vec<u8>) {
            if let Some(proc) = proc {
                proc(pixels);
            }
        }

        let pixels = pixels.into();
        if !self.set_info(image_info, row_bytes) {
            invoke_release_proc(release_proc, pixels.unwrap_or_default());
            self.reset();
            return false;
        }
        let Some(pixels) = pixels else {
            invoke_release_proc(release_proc, Vec::new());
            return true; // we behaved as if they called setInfo()
        };

        // set_info may have corrected info (e.g. 565 is always opaque).
        let corrected_info = self.info.clone();
        // skia-rust: the C++ trusts that the caller's memory is big enough; owned bytes that are
        // too short would only panic later, so reject them here.
        if pixels.len() < corrected_info.compute_byte_size(row_bytes) {
            invoke_release_proc(release_proc, pixels);
            self.reset();
            return false;
        }
        self.set_pixel_ref(
            Some(make_pixel_ref_with_proc(
                corrected_info.width(),
                corrected_info.height(),
                row_bytes,
                pixels,
                release_proc,
            )),
            (0, 0),
        );
        self.validate();
        true
    }

    /// Replaces the pixel storage with `pixels` (taking ownership), keeping the [`ImageInfo`]
    /// and row bytes. Sets the [`PixelRef`] origin to `(0, 0)`.
    ///
    /// If `pixels` is `None`, or if the color type is [`ColorType::Unknown`]: releases the
    /// reference to the [`PixelRef`], and sets it to `None`.
    // Port of: src/core/SkBitmap.cpp#L197-L205 (chrome/m156)
    #[doc(alias = "setPixels")]
    pub fn set_pixels(&mut self, pixels: impl Into<Option<Vec<u8>>>) {
        let pixels = if ColorType::Unknown == self.color_type() {
            None
        } else {
            pixels.into()
        };
        let row_bytes = self.row_bytes;
        self.offset = pixels.as_ref().map(|_| 0);
        self.pixel_ref = pixels
            .map(|pixels| PixelRef::new(self.info.width(), self.info.height(), pixels, row_bytes));
        self.validate();
    }

    /// Allocates pixel memory with the [`ImageInfo`] of the bitmap (as set by
    /// [`Self::set_info()`]) and zeroes it. Returns false if the color type is
    /// [`ColorType::Unknown`], or allocation fails.
    // Port of: src/core/SkBitmap.cpp#L207-L214 and #L357-L372 (chrome/m156)
    #[doc(alias = "tryAllocPixels")]
    #[must_use]
    pub fn try_alloc_pixels(&mut self) -> bool {
        // HeapAllocator::allocPixelRef
        if ColorType::Unknown == self.info.color_type() {
            return false;
        }

        let Some(pixel_ref) = malloc_pixel_ref::make_allocate(&self.info, self.row_bytes) else {
            return false;
        };

        self.set_pixel_ref(Some(pixel_ref), (0, 0));
        self.validate();
        true
    }

    /// Like [`Self::try_alloc_pixels`].
    ///
    /// # Panics
    /// If allocation fails.
    // Port of: src/core/SkBitmap.cpp#L228-L239 (chrome/m156)
    #[doc(alias = "allocPixels")]
    pub fn alloc_pixels(&mut self) {
        assert!(
            self.try_alloc_pixels(),
            "Bitmap::try_alloc_pixels failed ColorType:{:?} AlphaType:{:?} [w:{} h:{}] rb:{}",
            self.info.color_type(),
            self.info.alpha_type(),
            self.info.width(),
            self.info.height(),
            self.row_bytes
        );
    }

    /// Returns the [`PixelRef`], which may be shared by multiple bitmaps, or `None` if it has not
    /// been set. The [`PixelRef`] is a cheap handle: this clones it.
    // Port of: include/core/SkBitmap.h#L1004 (chrome/m156)
    #[doc(alias = "pixelRef")]
    #[must_use]
    pub fn pixel_ref(&self) -> Option<PixelRef> {
        self.pixel_ref.clone()
    }

    /// Returns the origin of the pixels within the [`PixelRef`]. The bitmap bounds are a subset
    /// of the [`PixelRef`] bounds. Multiple [`Bitmap`]s can share the same [`PixelRef`], where
    /// each has different bounds.
    ///
    /// Returns `(0, 0)` if the [`PixelRef`] is `None`.
    // Port of: src/core/SkBitmap.cpp#L158-L170 (chrome/m156)
    #[doc(alias = "pixelRefOrigin")]
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    // mirrors SkToS32 of values that fit (asserted by validate())
    pub fn pixel_ref_origin(&self) -> IPoint {
        let rb = self.row_bytes;
        let off = match (&self.pixel_ref, self.offset) {
            (Some(_), Some(off)) if 0 != rb => off,
            _ => return IPoint { x: 0, y: 0 },
        };
        debug_assert!(self.bytes_per_pixel() > 0);
        debug_assert_eq!(self.bytes_per_pixel(), 1 << self.shift_per_pixel());
        IPoint {
            x: ((off % rb) >> self.shift_per_pixel()) as i32,
            y: (off / rb) as i32,
        }
    }

    /// Sets the [`PixelRef`] containing pixels and the `offset` `(dx, dy)` within the
    /// [`PixelRef`] pixels for the top-left corner of the bitmap. Asserts in debug builds if
    /// `dx` or `dy` are out of range.
    ///
    /// The caller is responsible for ensuring that the pixels match the color type and alpha
    /// type in [`ImageInfo`].
    // Port of: src/core/SkBitmap.cpp#L172-L195 (chrome/m156)
    #[doc(alias = "setPixelRef")]
    pub fn set_pixel_ref(&mut self, pixel_ref: Option<PixelRef>, offset: impl Into<IPoint>) {
        let IPoint { x: dx, y: dy } = offset.into();
        #[cfg(debug_assertions)]
        if let Some(pr) = &pixel_ref
            && ColorType::Unknown != self.color_type()
        {
            debug_assert!(dx >= 0 && self.width() + dx <= pr.width());
            debug_assert!(dy >= 0 && self.height() + dy <= pr.height());
        }
        self.pixel_ref = if ColorType::Unknown == self.color_type() {
            None
        } else {
            pixel_ref
        };
        let mut offset = None;
        let mut row_bytes = self.row_bytes;
        // ignore dx,dy if there is no pixelref
        if let Some(pixel_ref) = &self.pixel_ref {
            row_bytes = pixel_ref.row_bytes();
            // (char*)p + dy * rowBytes + dx * this->bytesPerPixel()
            #[allow(clippy::cast_sign_loss)] // mirrors the int -> size_t conversions
            let off = (dy as usize)
                .wrapping_mul(row_bytes)
                .wrapping_add((dx as usize).wrapping_mul(self.bytes_per_pixel()));
            offset = Some(off);
        }
        self.row_bytes = row_bytes;
        self.offset = offset;
        self.validate();
    }

    /// Returns a unique value corresponding to the pixels in the [`PixelRef`]. Returns a
    /// different value after [`Self::notify_pixels_changed()`] has been called. Returns zero if
    /// the [`PixelRef`] is `None`.
    // Port of: src/core/SkBitmap.cpp#L341-L343 (chrome/m156)
    #[doc(alias = "getGenerationID")]
    #[must_use]
    pub fn generation_id(&self) -> u32 {
        self.pixel_ref.as_ref().map_or(0, PixelRef::generation_id)
    }

    /// Marks that the pixels in the [`PixelRef`] have changed. Subsequent calls to
    /// [`Self::generation_id()`] return a different value.
    // Port of: src/core/SkBitmap.cpp#L345-L350 (chrome/m156)
    #[doc(alias = "notifyPixelsChanged")]
    pub fn notify_pixels_changed(&self) {
        debug_assert!(!self.is_immutable());
        if let Some(pixel_ref) = &self.pixel_ref {
            pixel_ref.notify_pixels_changed();
        }
    }

    /// Replaces pixel values with `c`, interpreted as being in the sRGB [`ColorSpace`]. All
    /// pixels contained by [`Self::bounds()`] are affected. If the color type is
    /// [`ColorType::Gray8`] or [`ColorType::RGB565`], then alpha is ignored; RGB is treated as
    /// opaque. If the color type is [`ColorType::Alpha8`], then RGB is ignored.
    ///
    /// Input color is ultimately converted to a [`Color4f`], so [`Self::erase_color_4f`] will
    /// have higher color resolution.
    // Port of: src/core/SkBitmap.cpp#L426-L428 (chrome/m156)
    #[doc(alias = "eraseColor")]
    pub fn erase_color(&self, c: impl Into<Color>) {
        self.erase_4f(
            Color4f::from_color(c.into()),
            IRect::from_wh(self.width(), self.height()),
        );
    }

    /// Replaces pixel values with `c`, interpreted as being in the sRGB [`ColorSpace`]. All
    /// pixels contained by [`Self::bounds()`] are affected.
    // Port of: src/core/SkBitmap.cpp#L422-L424 (chrome/m156)
    #[doc(alias = "eraseColor")]
    pub fn erase_color_4f(&self, c: impl AsRef<Color4f>) {
        self.erase_4f(c, IRect::from_wh(self.width(), self.height()));
    }

    /// Replaces pixel values with the unpremultiplied color built from `a`, `r`, `g`, and `b`,
    /// interpreted as being in the sRGB [`ColorSpace`]. All pixels contained by
    /// [`Self::bounds()`] are affected.
    // Port of: include/core/SkBitmap.h#L1060-L1062 (chrome/m156)
    #[doc(alias = "eraseARGB")]
    pub fn erase_argb(&self, a: u8, r: u8, g: u8, b: u8) {
        self.erase_color(Color::from_argb(a, r, g, b));
    }

    /// Replaces pixel values inside `area` with `c`, interpreted as being in the sRGB
    /// [`ColorSpace`]. If `area` does not intersect [`Self::bounds()`], the call has no effect.
    // Port of: src/core/SkBitmap.cpp#L418-L420 (chrome/m156)
    pub fn erase(&self, c: impl Into<Color>, area: impl AsRef<IRect>) {
        self.erase_4f(Color4f::from_color(c.into()), area);
    }

    /// Replaces pixel values inside `area` with `c`, interpreted as being in the sRGB
    /// [`ColorSpace`]. If `area` does not intersect [`Self::bounds()`], the call has no effect.
    // Port of: src/core/SkBitmap.cpp#L400-L416 (chrome/m156)
    #[doc(alias = "erase")]
    pub fn erase_4f(&self, c: impl AsRef<Color4f>, area: impl AsRef<IRect>) {
        self.validate();

        if ColorType::Unknown == self.color_type() {
            // TODO: can we ASSERT that we never get here?
            return; // can't erase. Should we bzero so the memory is not uninitialized?
        }

        let Some(mut result) = self.peek_pixels_mut() else {
            return;
        };

        if result.erase_4f(c, Some(area.as_ref())) {
            self.notify_pixels_changed();
        }
    }

    /// Replaces pixel values inside `area` with `c`. Deprecated: use [`Self::erase`].
    // Port of: include/core/SkBitmap.h#L848-L850 (chrome/m156)
    #[doc(alias = "eraseArea")]
    pub fn erase_area(&self, area: impl AsRef<IRect>, c: impl Into<Color>) {
        self.erase(c, area);
    }

    /// Returns true if the bitmap can be drawn (it has pixels).
    // Port of: include/core/SkBitmap.h#L752-L754 (chrome/m156)
    #[doc(alias = "readyToDraw")]
    #[must_use]
    pub fn is_ready_to_draw(&self) -> bool {
        self.offset.is_some()
    }

    /// Returns the 8-bit pixel at `(x, y)`. The bitmap must have pixels.
    // Port of: include/core/SkBitmap.h#L1259-L1262 (chrome/m156)
    #[doc(alias = "getAddr8")]
    #[must_use]
    pub fn get_addr8(&self, x: i32, y: i32) -> u8 {
        debug_assert!(self.offset.is_some());
        self.pixmap().addr8(x, y)
    }

    /// Returns the 16-bit pixel at `(x, y)`. The bitmap must have pixels.
    // Port of: include/core/SkBitmap.h#L1254-L1257 (chrome/m156)
    #[doc(alias = "getAddr16")]
    #[must_use]
    pub fn get_addr16(&self, x: i32, y: i32) -> u16 {
        debug_assert!(self.offset.is_some());
        self.pixmap().addr16(x, y)
    }

    /// Returns the 32-bit pixel at `(x, y)`. The bitmap must have pixels.
    // Port of: include/core/SkBitmap.h#L1249-L1252 (chrome/m156)
    #[doc(alias = "getAddr32")]
    #[must_use]
    pub fn get_addr32(&self, x: i32, y: i32) -> u32 {
        debug_assert!(self.offset.is_some());
        self.pixmap().addr32(x, y)
    }

    /// Writes the 8-bit pixel at `(x, y)` (`*getAddr8(x, y) = value`). The bitmap must have
    /// writable pixels. Does not change the generation ID.
    ///
    /// # Panics
    /// If the bitmap has no pixels, or they cannot be written to.
    #[doc(alias = "getAddr8")]
    pub fn set_addr8(&self, x: i32, y: i32, value: u8) {
        let mut pixmap = self
            .peek_pixels_mut()
            .expect("the bitmap has no writable pixels");
        pixmap.set_addr8(x, y, value);
    }

    /// Writes the 16-bit pixel at `(x, y)` (`*getAddr16(x, y) = value`). The bitmap must have
    /// writable pixels. Does not change the generation ID.
    ///
    /// # Panics
    /// If the bitmap has no pixels, or they cannot be written to.
    #[doc(alias = "getAddr16")]
    pub fn set_addr16(&self, x: i32, y: i32, value: u16) {
        let mut pixmap = self
            .peek_pixels_mut()
            .expect("the bitmap has no writable pixels");
        pixmap.set_addr16(x, y, value);
    }

    /// Writes the 32-bit pixel at `(x, y)` (`*getAddr32(x, y) = value`). The bitmap must have
    /// writable pixels. Does not change the generation ID.
    ///
    /// # Panics
    /// If the bitmap has no pixels, or they cannot be written to.
    #[doc(alias = "getAddr32")]
    pub fn set_addr32(&self, x: i32, y: i32, value: u32) {
        let mut pixmap = self
            .peek_pixels_mut()
            .expect("the bitmap has no writable pixels");
        pixmap.set_addr32(x, y, value);
    }

    /// Returns the pixel at `(x, y)` as an unpremultiplied color. Returns black with alpha if
    /// the color type is [`ColorType::Alpha8`].
    ///
    /// Input is not validated: out of bounds values of `x` or `y` trigger a debug assertion.
    /// Fails if the color type is [`ColorType::Unknown`] or there are no pixels. The color space
    /// in [`ImageInfo`] is ignored.
    // Port of: include/core/SkBitmap.h#L1086-L1088 (chrome/m156)
    #[doc(alias = "getColor")]
    #[must_use]
    pub fn get_color(&self, p: impl Into<IPoint>) -> Color {
        self.pixmap().get_color(p)
    }

    /// Returns the pixel at `(x, y)` as an unpremultiplied float color.
    // Port of: include/core/SkBitmap.h#L1100-L1102 (chrome/m156)
    #[doc(alias = "getColor4f")]
    #[must_use]
    pub fn get_color_4f(&self, p: impl Into<IPoint>) -> Color4f {
        self.pixmap().get_color_4f(p)
    }

    /// Looks up the pixel at `(x, y)` and returns its alpha component, normalized to `[0..1]`.
    /// This is roughly equivalent to `get_color().a()`, but can be more efficient (and more
    /// precise if the pixels store more than 8 bits per component).
    // Port of: include/core/SkBitmap.h#L1109-L1111 (chrome/m156)
    #[doc(alias = "getAlphaf")]
    #[must_use]
    pub fn get_alpha_f(&self, p: impl Into<IPoint>) -> f32 {
        self.pixmap().get_alpha_f(p)
    }

    /// Shares the [`PixelRef`] with `dst`. Pixels are not copied; the bitmap and `dst` point to
    /// the same pixels; `dst` bounds are set to the intersection of `subset` and the original
    /// [`Self::bounds()`].
    ///
    /// `subset` may be larger than [`Self::bounds()`]. Any area outside of [`Self::bounds()`] is
    /// ignored. Any contents of `dst` are discarded.
    ///
    /// Returns false if the [`PixelRef`] is `None`, or if `subset` does not intersect
    /// [`Self::bounds()`].
    // Port of: src/core/SkBitmap.cpp#L433-L464 (chrome/m156)
    #[doc(alias = "extractSubset")]
    #[must_use]
    pub fn extract_subset(&self, dst: &mut Self, subset: impl AsRef<IRect>) -> bool {
        self.validate();

        if self.pixel_ref.is_none() {
            return false; // no src pixels
        }

        let src_rect = IRect::from_wh(self.width(), self.height());
        let Some(r) = IRect::intersect(&src_rect, subset.as_ref()) else {
            return false; // r is empty (i.e. no intersection)
        };

        // If the upper left of the rectangle was outside the bounds of this bitmap, we should
        // have exited above.
        #[allow(clippy::cast_sign_loss)] // mirrors static_cast<unsigned>
        {
            debug_assert!((r.left as u32) < (self.width() as u32));
            debug_assert!((r.top as u32) < (self.height() as u32));
        }

        let mut tmp = Bitmap::new();
        let _ = tmp.set_info(&self.info.with_dimensions(r.size()), self.row_bytes);

        if self.pixel_ref.is_some() {
            let origin = self.pixel_ref_origin();
            // share the pixelref with a custom offset
            tmp.set_pixel_ref(
                self.pixel_ref.clone(),
                (origin.x + r.left, origin.y + r.top),
            );
        }
        tmp.validate();

        // we know we're good, so commit to result
        dst.swap(&mut tmp);
        true
    }

    /// Returns a pixmap over the pixels for reading, if there are any (`peekPixels`).
    // Port of: src/core/SkBitmap.cpp#L615-L623 (chrome/m156)
    #[doc(alias = "peekPixels")]
    #[must_use]
    pub fn peek_pixels(&self) -> Option<Pixmap<'_>> {
        self.offset.map(|_| self.pixmap())
    }

    /// Returns a pixmap over the pixels that can also write to them, if there are any and they
    /// can be written to (pixels shared as immutable [`Data`](crate::data::Data) cannot).
    ///
    /// skia-rust: `SkPixmap` writes through a `const_cast`; this takes the [`PixelRef`]'s write
    /// lock for as long as the returned pixmap lives.
    #[must_use]
    pub fn peek_pixels_mut(&self) -> Option<Pixmap<'_>> {
        match (&self.pixel_ref, self.offset) {
            (Some(pixel_ref), Some(offset)) => Some(Pixmap::from_write_guard(
                self.info.clone(),
                pixel_ref.pixels_mut()?,
                offset,
                self.row_bytes,
            )),
            _ => None,
        }
    }

    /// Asserts if internal values are illegal or inconsistent (debug builds only).
    // Port of: src/core/SkBitmap.cpp#L589-L610 (chrome/m156)
    pub fn validate(&self) {
        #[cfg(debug_assertions)]
        {
            self.info.validate();

            debug_assert!(self.info.valid_row_bytes(self.row_bytes));

            // A pixel ref always has pixels in skia-rust, so "fPixelRef->pixels()" is the pixel
            // ref's existence.
            debug_assert_eq!(self.pixel_ref.is_some(), self.offset.is_some());

            if let (Some(pixel_ref), Some(_)) = (&self.pixel_ref, self.offset) {
                debug_assert_eq!(pixel_ref.row_bytes(), self.row_bytes);
                let origin = self.pixel_ref_origin();
                debug_assert!(origin.x >= 0);
                debug_assert!(origin.y >= 0);
                debug_assert!(pixel_ref.width() >= self.width() + origin.x);
                debug_assert!(pixel_ref.height() >= self.height() + origin.y);
                debug_assert!(pixel_ref.row_bytes() >= self.info.min_row_bytes());
            }
        }
    }
}
