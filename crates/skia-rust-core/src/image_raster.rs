// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/image/SkImage_Raster.{h,cpp}

//! `SkImage_Raster`: an image whose pixels are in memory, held by an immutable [`Bitmap`].
//!
//! skia-rust: the bitmap's pixel ref is a copy-on-write handle (`docs/design/pixels.md`), so an
//! image that shares the pixels of a mutable bitmap ([`CopyPixelsMode::Never`], the snapshot of
//! a surface, the image a draw makes of a bitmap) sees the old pixels when the bitmap is written
//! afterwards, instead of the new ones. `onMakeSurface`, `notifyAddedToRasterCache` (the bitmap
//! cache is not ported) and the recorder parameters are not ported.

use core::any::Any;
use std::sync::{Arc, OnceLock};

use crate::bitmap::Bitmap;
use crate::blend_mode::BlendMode;
use crate::color_space::ColorSpace;
use crate::color_type::ColorType;
use crate::data::Data;
use crate::image::{Image, RequiredProperties};
use crate::image_base::{ImageBase, ImageType, NEED_NEW_IMAGE_UNIQUE_ID};
use crate::image_info::ImageInfo;
use crate::image_info_priv::{color_type_is_alpha_only, image_info_is_valid};
use crate::images;
use crate::malloc_pixel_ref;
use crate::matrix::Matrix;
use crate::mipmap::Mipmap;
use crate::paint::Paint;
use crate::pixel_ref::next_image_id;
use crate::pixmap::Pixmap;
use crate::rect::IRect;
use crate::sampling_options::SamplingOptions;
use crate::shader::Shader;
use crate::shaders;
use crate::shaders::image_shader::ImageShader;
use crate::tile_mode::TileMode;

/// How [`ImageRaster::make_from_bitmap`] treats the pixels of the bitmap (`SkCopyPixelsMode`).
// Port of: src/image/SkImage_Raster.h#L31-L35 (chrome/m156)
#[doc(alias = "SkCopyPixelsMode")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum CopyPixelsMode {
    /// Only copy src pixels if they are marked mutable (`kIfMutable`).
    IfMutable,
    /// Always copy src pixels (even if they are marked immutable) (`kAlways`).
    Always,
    /// Never copy src pixels (even if they are marked mutable) (`kNever`).
    Never,
}

/// An image in memory (`SkImage_Raster`).
// Port of: src/image/SkImage_Raster.h#L37-L121 (chrome/m156)
#[doc(alias = "SkImage_Raster")]
#[derive(Debug)]
pub struct ImageRaster {
    info: ImageInfo,
    unique_id: u32,
    bitmap: Bitmap,
    mips: Option<Arc<Mipmap>>,
    // `SkMipmapCache::AddAndRef(image)`: the mipmap built from the base level the first time an
    // image shader or a mipmapped draw asks for one.
    cached_mips: OnceLock<Option<Arc<Mipmap>>>,
}

// fixes skbug.com/40036261
// Port of: src/image/SkImage_Raster.cpp#L29-L34 (chrome/m156)
fn is_not_subset(bm: &Bitmap) -> bool {
    let pixel_ref = bm.pixel_ref().expect("is_not_subset needs a pixel ref");
    let dim = pixel_ref.dimensions();
    debug_assert!(
        dim != bm.dimensions() || bm.pixel_ref_origin() == crate::point::IPoint::new(0, 0)
    );
    dim == bm.dimensions()
}

impl ImageRaster {
    /// An image over the immutable bytes `data` (the `SkData` constructor).
    ///
    /// `data` must hold `info.compute_byte_size(row_bytes)` bytes; use
    /// [`images::raster_from_data`] to check.
    // Port of: src/image/SkImage_Raster.cpp#L40-L52 (chrome/m156)
    #[must_use]
    pub fn from_data(
        info: &ImageInfo,
        data: Data,
        row_bytes: usize,
        mips: Option<Arc<Mipmap>>,
        id: u32,
    ) -> Option<ImageRaster> {
        let mut bitmap = Bitmap::new();
        if !bitmap.set_info(info, row_bytes) {
            return None;
        }
        let pixel_ref = malloc_pixel_ref::make_with_data(bitmap.info(), row_bytes, data)?;
        bitmap.set_pixel_ref(Some(pixel_ref), (0, 0));
        bitmap.set_immutable();
        Some(ImageRaster {
            info: info.clone(),
            unique_id: if NEED_NEW_IMAGE_UNIQUE_ID == id {
                next_image_id()
            } else {
                id
            },
            bitmap,
            mips,
            cached_mips: OnceLock::new(),
        })
    }

    // Port of: src/image/SkImage_Raster.cpp#L54-L62 (chrome/m156)
    fn from_bitmap(
        bm: &Bitmap,
        mips: Option<Arc<Mipmap>>,
        bitmap_may_be_mutable: bool,
    ) -> ImageRaster {
        let unique_id = if is_not_subset(bm) {
            bm.generation_id()
        } else {
            NEED_NEW_IMAGE_UNIQUE_ID
        };
        debug_assert!(bitmap_may_be_mutable || bm.is_immutable());
        ImageRaster {
            info: bm.info().clone(),
            unique_id: if NEED_NEW_IMAGE_UNIQUE_ID == unique_id {
                next_image_id()
            } else {
                unique_id
            },
            bitmap: bm.clone(),
            mips,
            cached_mips: OnceLock::new(),
        }
    }

    /// Examines the bitmap to decide if it can share the existing pixel ref, or if it needs to
    /// make a deep-copy of the pixels.
    ///
    /// The bitmap's pixel ref will be shared if either the bitmap is marked as immutable, or
    /// `cpm` allows it.
    ///
    /// If the bitmap's color type cannot be converted into a corresponding [`ImageInfo`], or the
    /// bitmap's pixels cannot be accessed, this will return `None` (`MakeFromBitmap`).
    // Port of: src/image/SkImage_Raster.cpp#L140-L163 (chrome/m156)
    #[doc(alias = "MakeFromBitmap")]
    #[must_use]
    pub fn make_from_bitmap(
        bm: &Bitmap,
        cpm: CopyPixelsMode,
        mips: Option<Arc<Mipmap>>,
    ) -> Option<ImageRaster> {
        if !image_info_is_valid(bm.info()) || bm.row_bytes() < bm.info().min_row_bytes() {
            return None;
        }

        let pixmap = bm.peek_pixels()?;
        let pixels = pixmap.addr()?;

        if CopyPixelsMode::Always == cpm || (!bm.is_immutable() && CopyPixelsMode::Never != cpm) {
            let size = bm.compute_byte_size();
            if ImageInfo::byte_size_overflowed(size) {
                return None;
            }

            let data = Data::new_copy(pixels.get(..size)?);

            return ImageRaster::from_data(
                bm.info(),
                data,
                bm.row_bytes(),
                mips,
                NEED_NEW_IMAGE_UNIQUE_ID,
            );
        }
        Some(ImageRaster::from_bitmap(
            bm,
            mips,
            CopyPixelsMode::Never == cpm,
        ))
    }

    /// The bitmap holding the image's pixels (`bitmap`).
    #[must_use]
    pub fn bitmap(&self) -> &Bitmap {
        &self.bitmap
    }

    /// A shader that implements the shader+image behavior defined for drawImage/Bitmap where
    /// the paint's shader is ignored when the bitmap is a color image, but properly composed
    /// together when it is an alpha image. This allows the returned shader to be assigned to a
    /// paint clone without discarding the original behavior (`makeShaderForPaint`).
    // Port of: src/image/SkImage_Raster.cpp#L195-L211 (chrome/m156)
    #[doc(alias = "makeShaderForPaint")]
    #[must_use]
    pub fn make_shader_for_paint(
        image: &Image,
        paint: &Paint,
        tmx: TileMode,
        tmy: TileMode,
        sampling: &SamplingOptions,
        local_matrix: Option<&Matrix>,
    ) -> Option<Shader> {
        let s = ImageShader::make(Some(image.clone()), tmx, tmy, sampling, local_matrix, false)?;
        if color_type_is_alpha_only(image.color_type()) && paint.shader().is_some() {
            // Compose the image shader with the paint's shader. Alpha images+shaders should
            // output the texture's alpha multiplied by the shader's color. DstIn (d*sa) will
            // achieve this with the source image and dst shader (MakeBlend takes dst first, src
            // second).
            let dst = paint.shader()?;
            return Some(shaders::blend(BlendMode::DstIn, dst, s));
        }
        Some(s)
    }
}

// Port of: src/image/SkImage_Raster.cpp#L96-L106 (chrome/m156)
fn copy_bitmap_subset(orig: &Bitmap, subset: &IRect) -> Option<Bitmap> {
    let info = orig.info().with_dimensions(subset.size());
    let mut bitmap = Bitmap::new();
    if !bitmap.try_alloc_pixels_info(&info, None) {
        return None;
    }

    let dst_row_bytes = bitmap.row_bytes();
    let src_pixmap = orig.peek_pixels()?;
    let src_row_bytes = orig.row_bytes();
    let src = src_pixmap.addr_at((subset.left, subset.top))?;
    {
        let mut dst_pixmap = bitmap.peek_pixels_mut()?;
        let dst = dst_pixmap.writable_addr()?;
        // SkRectMemcpy(dst, bitmap.rowBytes(), src, orig.rowBytes(), bitmap.rowBytes(), height)
        for row in 0..usize::try_from(subset.height()).ok()? {
            let from = row * src_row_bytes;
            let to = row * dst_row_bytes;
            let n = dst_row_bytes.min(src.len().saturating_sub(from));
            dst[to..to + n].copy_from_slice(&src[from..from + n]);
        }
    }

    bitmap.set_immutable();
    Some(bitmap)
}

// Port of: src/image/SkImage_Raster.cpp#L108-L127 (chrome/m156)
fn copy_mipmaps(src: &Bitmap, src_mips: Option<&Arc<Mipmap>>) -> Option<Arc<Mipmap>> {
    let src_mips = src_mips?;

    let mut dst = Mipmap::build(&src.pixmap(), /* compute_contents= */ false)?;
    for i in 0..dst.count_levels() {
        let src_level = src_mips.get_level(i)?;
        let (info, row_bytes, bytes) = dst.level_bytes_mut(usize::try_from(i).ok()?)?;
        let info = info.clone();
        if !src_level
            .pixmap
            .read_pixels(&info, bytes, row_bytes, (0, 0))
        {
            return None;
        }
    }

    Some(Arc::new(dst))
}

impl ImageBase for ImageRaster {
    fn info(&self) -> &ImageInfo {
        &self.info
    }

    fn unique_id(&self) -> u32 {
        self.unique_id
    }

    // Port of: src/image/SkImage_Raster.h#L79 (chrome/m156)
    fn image_type(&self) -> ImageType {
        ImageType::Raster
    }

    // Port of: src/image/SkImage_Raster.cpp#L64-L79 (chrome/m156)
    fn on_read_pixels(
        &self,
        dst_info: &ImageInfo,
        dst_pixels: &mut [u8],
        dst_row_bytes: usize,
        src_x: i32,
        src_y: i32,
    ) -> bool {
        self.bitmap
            .read_pixels(dst_info, dst_pixels, dst_row_bytes, src_x, src_y)
    }

    // Port of: src/image/SkImage_Raster.cpp#L81-L83 (chrome/m156)
    fn on_peek_pixels(&self) -> Option<Pixmap<'_>> {
        self.bitmap.peek_pixels()
    }

    fn on_peek_bitmap(&self) -> Option<&Bitmap> {
        Some(&self.bitmap)
    }

    // Port of: src/image/SkImage_Raster.cpp#L85-L88 (chrome/m156)
    fn get_ro_pixels(&self) -> Option<Bitmap> {
        Some(self.bitmap.clone())
    }

    // Port of: src/image/SkImage_Raster.h#L88 (chrome/m156)
    fn on_has_mipmaps(&self) -> bool {
        self.mips.is_some()
    }

    fn on_is_protected(&self) -> bool {
        false
    }

    fn on_peek_mips(&self) -> Option<&Arc<Mipmap>> {
        self.mips.as_ref()
    }

    // Port of: src/core/SkMipmapCache.cpp (AddAndRef) (chrome/m156)
    fn cached_mips(&self) -> Option<Arc<Mipmap>> {
        self.cached_mips
            .get_or_init(|| {
                let src = self.get_ro_pixels()?;
                let pixmap = src.peek_pixels()?;
                Mipmap::build(&pixmap, true).map(Arc::new)
            })
            .clone()
    }

    // Port of: src/image/SkImage_Raster.cpp#L129-L170 (chrome/m156)
    fn on_make_subset(
        &self,
        subset: &IRect,
        required_properties: RequiredProperties,
    ) -> Option<Image> {
        if required_properties.mipmapped {
            let full_copy = *subset == IRect::from_wh(self.bitmap.width(), self.bitmap.height());

            let mips = if full_copy {
                copy_mipmaps(&self.bitmap, self.mips.as_ref())
            } else {
                None
            };

            // SkImage::withMipmaps will always make a copy for us so we can temporarily share
            // the pixel ref with fBitmap
            let mut tmp_subset = Bitmap::new();
            if !self.bitmap.extract_subset(&mut tmp_subset, subset) {
                return None;
            }

            let tmp = Image::from_base(ImageRaster::from_bitmap(
                &tmp_subset,
                None,
                /* bitmap_may_be_mutable= */ true,
            ));

            // withMipmaps will auto generate the mipmaps if a nullptr is passed in
            debug_assert!(
                mips.as_ref()
                    .is_none_or(|m| m.valid_for_root_level(tmp.image_info()))
            );
            Some(tmp.with_mipmaps(mips))
        } else {
            let copy = copy_bitmap_subset(&self.bitmap, subset)?;
            images::raster_from_bitmap(&copy)
        }
    }

    // Port of: src/image/SkImage_Raster.cpp#L172-L184 (chrome/m156)
    fn on_as_legacy_bitmap(&self) -> Option<Bitmap> {
        // When we're a snapshot from a surface, our bitmap may not be marked immutable
        // even though logically always we are, but in that case we can't physically share our
        // pixelref since the caller might call setImmutable() themselves
        // (thus changing our state).
        if self.bitmap.is_immutable() {
            return Some(self.bitmap.clone());
        }
        // SkImage_Base::onAsLegacyBitmap
        let info = self
            .info
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

    // Port of: src/image/SkImage_Raster.cpp#L213-L226 (chrome/m156)
    fn on_make_with_mipmaps(&self, mips: Option<Arc<Mipmap>>) -> Option<Image> {
        // It's dangerous to have two SkBitmaps that share a SkPixelRef but have different
        // SkMipmaps since various caches key on SkPixelRef's generation ID. Also, SkPixelRefs
        // that back SkSurfaces are marked "temporarily immutable" and making an image that uses
        // the same SkPixelRef can interact badly with SkSurface/SkImage copy-on-write. So we
        // just always make a copy with a new ID.
        let mips = match mips {
            Some(mips) => Some(mips),
            None => Mipmap::build(&self.bitmap.pixmap(), true).map(Arc::new),
        };
        ImageRaster::make_from_bitmap(&self.bitmap, CopyPixelsMode::Always, mips)
            .map(Image::from_base)
    }

    // Port of: src/image/SkImage_Raster.cpp#L228-L244 (chrome/m156)
    fn make_color_type_and_color_space(
        &self,
        target_color_type: ColorType,
        target_color_space: Option<ColorSpace>,
        _required_properties: RequiredProperties,
    ) -> Option<Image> {
        let src = self.bitmap.peek_pixels()?;

        let mut dst = Bitmap::new();
        if !dst.try_alloc_pixels_info(
            &self
                .bitmap
                .info()
                .with_color_type(target_color_type)
                .with_color_space(target_color_space),
            None,
        ) {
            return None;
        }

        let ok = dst.write_pixels(&src, 0, 0);
        debug_assert!(ok);
        dst.set_immutable();
        images::raster_from_bitmap(&dst)
    }

    // Port of: src/image/SkImage_Raster.cpp#L246-L252 (chrome/m156)
    fn on_reinterpret_color_space(&self, new_cs: ColorSpace) -> Option<Image> {
        // TODO: If our bitmap is immutable, then we could theoretically create another image
        // sharing our pixelRef. That doesn't work (without more invasive logic), because the
        // image gets its gen ID from the bitmap, which gets it from the pixelRef.
        let mut pixmap = self.bitmap.pixmap();
        pixmap.set_color_space(Some(new_cs));
        images::raster_from_pixmap_copy(&pixmap)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
