// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/image/SkImage_Base.h, src/image/SkImage_Base.cpp

//! `SkImage_Base`: the virtual interface of images ([`ImageBase`]).
//!
//! skia-rust: only the raster flavor ([`ImageRaster`](crate::image_raster::ImageRaster)) exists;
//! the lazy, picture, GPU and YUVA images, `GrDirectContext`, `SkRecorder`, async readback
//! (`onAsyncRescaleAndReadPixels*`), `onRefEncoded` and the raster cache notification
//! (`notifyAddedToRasterCache`, whose only effect is to tell the bitmap cache an ID is stale; the
//! bitmap cache is not ported) are not ported. `onMakeSurface` is declared in the raster crate
//! (`skia_rust_raster::surfaces`), where surfaces live.

use core::any::Any;
use core::fmt;
use std::sync::Arc;

use crate::bitmap::Bitmap;
use crate::color_space::ColorSpace;
use crate::color_type::ColorType;
use crate::data::Data;
use crate::image::{Image, RequiredProperties};
use crate::image_info::ImageInfo;
use crate::mipmap::Mipmap;
use crate::pixmap::Pixmap;
use crate::rect::IRect;

/// `kNeedNewImageUniqueID`: ask for a new unique ID.
// Port of: src/image/SkImage_Base.h#L35-L37 (chrome/m156)
#[doc(alias = "kNeedNewImageUniqueID")]
pub const NEED_NEW_IMAGE_UNIQUE_ID: u32 = 0;

/// The kinds of images (`SkImage_Base::Type`).
// Port of: src/image/SkImage_Base.h#L116-L126 (chrome/m156)
#[doc(alias = "SkImage_Base::Type")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ImageType {
    /// `kRaster`.
    Raster,
    /// `kRasterPinnable`.
    RasterPinnable,
    /// `kLazy`.
    Lazy,
    /// `kLazyPicture`.
    LazyPicture,
    /// `kLazyTexture`.
    LazyTexture,
    /// `kGanesh`.
    Ganesh,
    /// `kGaneshYUVA`.
    GaneshYuva,
    /// `kGraphite`.
    Graphite,
    /// `kGraphiteYUVA`.
    GraphiteYuva,
}

/// The virtual interface of an image (`SkImage_Base`, with the data members of `SkImage`).
///
/// Implementations are wrapped in an [`Image`] with [`Image::from_base`]; the non-virtual members
/// of `SkImage_Base` are inherent methods of `dyn ImageBase`.
// Port of: src/image/SkImage_Base.h#L39-L211 (chrome/m156)
#[doc(alias = "SkImage_Base")]
pub trait ImageBase: Any + fmt::Debug + Send + Sync {
    /// The info the image was created with (`SkImage::fInfo`, `imageInfo`).
    #[doc(alias = "imageInfo")]
    fn info(&self) -> &ImageInfo;

    /// The unique ID of the image's contents (`SkImage::fUniqueID`, `uniqueID`).
    #[doc(alias = "uniqueID")]
    fn unique_id(&self) -> u32;

    /// What kind of image this is (`type`).
    #[doc(alias = "type")]
    fn image_type(&self) -> ImageType;

    /// The pixels, if the image has direct access to them (`onPeekPixels`).
    #[doc(alias = "onPeekPixels")]
    fn on_peek_pixels(&self) -> Option<Pixmap<'_>> {
        None
    }

    /// The bitmap holding the image's pixels, if it has one (`onPeekBitmap`).
    #[doc(alias = "onPeekBitmap")]
    fn on_peek_bitmap(&self) -> Option<&Bitmap> {
        None
    }

    /// Copies pixels out of the image (`onReadPixels`).
    #[doc(alias = "onReadPixels")]
    fn on_read_pixels(
        &self,
        dst_info: &ImageInfo,
        dst_pixels: &mut [u8],
        dst_row_bytes: usize,
        src_x: i32,
        src_y: i32,
    ) -> bool;

    /// Whether the image has mipmap levels (`onHasMipmaps`).
    #[doc(alias = "onHasMipmaps")]
    fn on_has_mipmaps(&self) -> bool;

    /// Whether the image is protected (`onIsProtected`).
    #[doc(alias = "onIsProtected")]
    fn on_is_protected(&self) -> bool;

    /// The image's mipmaps, if it has them (`onPeekMips`, `refMips`).
    #[doc(alias = "onPeekMips")]
    #[doc(alias = "refMips")]
    fn on_peek_mips(&self) -> Option<&Arc<Mipmap>> {
        None
    }

    /// The mipmap `SkMipmapCache::AddAndRef` would find or build for this image, built the
    /// first time it is asked for (see [`ImageBase::try_load_mips`]).
    ///
    /// skia-rust: `SkMipmapCache` is a global resource cache keyed by the image's unique ID;
    /// images that can have one hold the lazily built result.
    fn cached_mips(&self) -> Option<Arc<Mipmap>> {
        None
    }

    /// A read-only copy of the pixels (`getROPixels`): we promise to not modify them, but only
    /// inspect them (or encode them).
    #[doc(alias = "getROPixels")]
    fn get_ro_pixels(&self) -> Option<Bitmap>;

    /// The subset of the image (`onMakeSubset`).
    #[doc(alias = "onMakeSubset")]
    fn on_make_subset(
        &self,
        subset: &IRect,
        required_properties: RequiredProperties,
    ) -> Option<Image>;

    /// A legacy bitmap of the image, an N32 copy in no color space unless the image can share
    /// its pixels (`onAsLegacyBitmap`).
    // Port of: src/image/SkImage_Base.cpp#L55-L70 (chrome/m156)
    #[doc(alias = "onAsLegacyBitmap")]
    fn on_as_legacy_bitmap(&self) -> Option<Bitmap> {
        // As the base-class, all we can do is make a copy (regardless of mode).
        // Subclasses that want to be more optimal should override.
        let info = self
            .info()
            .with_color_type(ColorType::N32)
            .with_color_space(None);
        let mut bitmap = Bitmap::new();
        if !bitmap.try_alloc_pixels_info(&info, None) {
            return None;
        }

        let (bm_info, row_bytes) = (bitmap.info().clone(), bitmap.row_bytes());
        let ok = {
            let mut pm = bitmap.peek_pixels_mut()?;
            let pixels = pm.writable_addr()?;
            self.on_read_pixels(&bm_info, pixels, row_bytes, 0, 0)
        };
        if !ok {
            bitmap.reset();
            return None;
        }

        bitmap.set_immutable();
        Some(bitmap)
    }

    /// The image with the pixels reinterpreted in `new_cs` (`onReinterpretColorSpace`).
    #[doc(alias = "onReinterpretColorSpace")]
    fn on_reinterpret_color_space(&self, new_cs: ColorSpace) -> Option<Image>;

    /// The image with the given mipmaps, built if `None` (`onMakeWithMipmaps`); `None` on
    /// failure.
    #[doc(alias = "onMakeWithMipmaps")]
    fn on_make_with_mipmaps(&self, _mips: Option<Arc<Mipmap>>) -> Option<Image> {
        None
    }

    /// The image converted to the given color type and color space
    /// (`makeColorTypeAndColorSpace`).
    #[doc(alias = "makeColorTypeAndColorSpace")]
    fn make_color_type_and_color_space(
        &self,
        target_color_type: ColorType,
        target_color_space: Option<ColorSpace>,
        required_properties: RequiredProperties,
    ) -> Option<Image>;

    /// The encoded data the image was made from, if it has any (`onRefEncoded`).
    // Port of: src/image/SkImage_Base.h#L70 (chrome/m156)
    #[doc(alias = "onRefEncoded")]
    fn on_ref_encoded(&self) -> Option<Data> {
        None
    }

    /// Whether the image can be drawn (`onIsValid`, with no recorder). Raster images always
    /// can; lazy images ask their generator.
    // Port of: src/image/SkImage_Base.h#L88 (chrome/m156)
    #[doc(alias = "onIsValid")]
    fn on_is_valid(&self) -> bool {
        matches!(
            self.image_type(),
            ImageType::Raster | ImageType::RasterPinnable
        )
    }

    /// The implementation as `Any`, for downcasts (`static_cast`).
    fn as_any(&self) -> &dyn Any;
}

impl dyn ImageBase {
    /// The mipmaps the image shader samples: the image's own, or the cached ones, built if they
    /// are not there yet (`try_load_mips` in `SkMipmapAccessor.cpp`).
    // Port of: src/core/SkMipmapAccessor.cpp#L22-L31 (chrome/m156)
    #[must_use]
    pub fn try_load_mips(&self) -> Option<Arc<Mipmap>> {
        if let Some(mips) = self.on_peek_mips() {
            return Some(Arc::clone(mips));
        }
        self.cached_mips()
    }

    /// True for picture-backed and codec-backed images (`isLazyGenerated`).
    #[doc(alias = "isLazyGenerated")]
    #[must_use]
    pub fn is_lazy_generated(&self) -> bool {
        matches!(
            self.image_type(),
            ImageType::Lazy | ImageType::LazyPicture | ImageType::LazyTexture
        )
    }

    /// True for images that store their pixels in memory (`isRasterBacked`).
    #[doc(alias = "isRasterBacked")]
    #[must_use]
    pub fn is_raster_backed(&self) -> bool {
        matches!(
            self.image_type(),
            ImageType::Raster | ImageType::RasterPinnable
        )
    }

    /// True for images instantiated by Ganesh in GPU memory (`isGaneshBacked`).
    #[doc(alias = "isGaneshBacked")]
    #[must_use]
    pub fn is_ganesh_backed(&self) -> bool {
        matches!(self.image_type(), ImageType::Ganesh | ImageType::GaneshYuva)
    }

    /// True for images instantiated by Graphite in GPU memory (`isGraphiteBacked`).
    #[doc(alias = "isGraphiteBacked")]
    #[must_use]
    pub fn is_graphite_backed(&self) -> bool {
        matches!(
            self.image_type(),
            ImageType::Graphite | ImageType::GraphiteYuva
        )
    }

    /// True for YUVA images (`isYUVA`).
    #[doc(alias = "isYUVA")]
    #[must_use]
    pub fn is_yuva(&self) -> bool {
        matches!(
            self.image_type(),
            ImageType::GaneshYuva | ImageType::GraphiteYuva
        )
    }

    /// True for images stored in GPU textures (`isTextureBacked`).
    #[doc(alias = "isTextureBacked")]
    #[must_use]
    pub fn is_texture_backed(&self) -> bool {
        self.is_ganesh_backed() || self.is_graphite_backed()
    }

    /// The subset of the image, if `subset` is non-empty and inside the image
    /// (`SkImage_Base::makeSubset`).
    // Port of: src/image/SkImage_Base.cpp#L72-L86 (chrome/m156)
    #[doc(alias = "makeSubset")]
    #[must_use]
    pub fn make_subset(
        &self,
        subset: &IRect,
        required_properties: RequiredProperties,
    ) -> Option<Image> {
        use crate::rect::Contains;
        if subset.is_empty() {
            return None;
        }

        let bounds = IRect::from_wh(self.info().width(), self.info().height());
        if !bounds.contains(subset) {
            return None;
        }

        self.on_make_subset(subset, required_properties)
    }

    /// The image converted to `target` color space, in its own color type
    /// (`SkImage_Base::makeColorSpace`).
    // Port of: src/image/SkImage_Base.cpp#L109-L116 (chrome/m156)
    #[doc(alias = "makeColorSpace")]
    #[must_use]
    pub fn make_color_space(
        &self,
        target: Option<ColorSpace>,
        props: RequiredProperties,
    ) -> Option<Image> {
        self.make_color_type_and_color_space(self.info().color_type(), target, props)
    }
}
