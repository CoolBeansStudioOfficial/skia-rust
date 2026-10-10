// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/image/SkImage_Lazy.h, src/image/SkImage_Lazy.cpp

//! `SkImage_Lazy`: an image whose pixels are made on demand by an [`ImageGenerator`].
//!
//! The generator is shared (`SharedGenerator`) by every lazy image made from it, and is only
//! used under its lock, as Skia's `ScopedGenerator` does. The decoded bitmap is kept in the
//! global [`crate::bitmap_cache`], and the image posts its entries as stale when it is dropped.
//! The mipmaps are kept per image where Skia keeps them in `SkMipmapCache`; that cache is not
//! ported, so a mipmap is not shared between images or purged on its own.
//!
//! skia-rust: GPU readback (`readPixelsProxy`), the YUV planes cache (`getPlanes`),
//! `onMakeSurface` and unique-ID listeners are not ported.

use core::any::Any;
use core::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use crate::bitmap::Bitmap;
use crate::bitmap_cache::{self, BitmapCacheDesc};
use crate::color_space::ColorSpace;
use crate::color_type::ColorType;
use crate::data::Data;
use crate::image::{Image, RequiredProperties};
use crate::image_base::{ImageBase, ImageType};
use crate::image_generator::ImageGenerator;
use crate::image_info::ImageInfo;
use crate::images;
use crate::mipmap::Mipmap;
use crate::pixel_ref::next_image_id;
use crate::rect::IRect;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic while a generator decodes leaves it usable for the next decode, as Skia's
    // SkAutoMutexExclusive does (it has no poisoning).
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A generator shared by the lazy images made from it (`SharedGenerator`).
// Port of: src/image/SkImage_Lazy.h#L23-L38 (chrome/m156)
pub struct SharedGenerator {
    generator: Mutex<Box<dyn ImageGenerator>>,
}

impl SharedGenerator {
    /// Shares `gen`, or returns `None` if there is no generator (`SharedGenerator::Make`).
    // Port of: src/image/SkImage_Lazy.cpp#L18-L20 (chrome/m156)
    #[doc(alias = "SharedGenerator::Make")]
    #[must_use]
    pub fn make(generator: Option<Box<dyn ImageGenerator>>) -> Option<Arc<SharedGenerator>> {
        generator.map(|generator| {
            Arc::new(SharedGenerator {
                generator: Mutex::new(generator),
            })
        })
    }
}

impl fmt::Debug for SharedGenerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedGenerator").finish_non_exhaustive()
    }
}

/// The info and unique ID a lazy image has, checked against its generator
/// (`SkImage_Lazy::Validator`).
// Port of: src/image/SkImage_Lazy.cpp#L30-L60 (chrome/m156)
#[doc(alias = "SkImage_Lazy::Validator")]
#[derive(Debug)]
pub struct Validator {
    shared: Option<Arc<SharedGenerator>>,
    info: ImageInfo,
    unique_id: u32,
}

impl Validator {
    /// Validates `gen`, with an optional new color type and color space for the image
    /// (`Validator(gen, colorType, colorSpace)`).
    // Port of: src/image/SkImage_Lazy.cpp#L30-L60 (chrome/m156)
    #[doc(alias = "SkImage_Lazy::Validator::Validator")]
    #[must_use]
    pub fn new(
        shared: Option<Arc<SharedGenerator>>,
        color_type: Option<ColorType>,
        color_space: Option<ColorSpace>,
    ) -> Self {
        let mut validator = Validator {
            shared: None,
            info: ImageInfo::new_unknown(None),
            unique_id: 0,
        };
        let Some(shared) = shared else {
            return validator;
        };

        // The generator's info and ID are read under its lock; Skia reads them unlocked, as
        // const getters.
        let (info, unique_id) = {
            let generator = lock(&shared.generator);
            (generator.info().clone(), generator.unique_id())
        };
        if info.is_empty() {
            return validator;
        }
        validator.info = info;
        validator.unique_id = unique_id;

        // A color type equal to the generator's is no change.
        let color_type = color_type.filter(|ct| *ct != validator.info.color_type());

        if color_type.is_some() || color_space.is_some() {
            if let Some(ct) = color_type {
                validator.info = validator.info.with_color_type(ct);
            }
            if let Some(cs) = color_space {
                validator.info = validator.info.with_color_space(Some(cs));
            }
            validator.unique_id = next_image_id();
        }

        validator.shared = Some(shared);
        validator
    }

    /// True if the generator is usable (`operator bool`).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.shared.is_some()
    }
}

/// A lazy image (`SkImage_Lazy`): the pixels are made by a shared generator when they are needed.
// Port of: src/image/SkImage_Lazy.h#L40-L105 (chrome/m156)
#[doc(alias = "SkImage_Lazy")]
pub struct ImageLazy {
    info: ImageInfo,
    unique_id: u32,
    shared: Arc<SharedGenerator>,
    /// The mipmaps the image shader samples, once built (`SkMipmapCache`).
    cached_mips: OnceLock<Option<Arc<Mipmap>>>,
    /// The result of the last `makeColorTypeAndColorSpace` (`fOnMakeColorTypeAndSpaceResult`).
    on_make_color_type_result: Mutex<Option<Image>>,
    /// Whether the image's pixels were added to the bitmap cache (`fAddedToRasterCache`).
    added_to_raster_cache: AtomicBool,
}

impl ImageLazy {
    /// The lazy image of a valid validator; `None` if it is not valid (`SkImage_Lazy(Validator*)`).
    // Port of: src/image/SkImage_Lazy.cpp#L106-L112 (chrome/m156)
    #[must_use]
    pub fn from_validator(validator: Validator) -> Option<ImageLazy> {
        let shared = validator.shared?;
        Some(ImageLazy {
            info: validator.info,
            unique_id: validator.unique_id,
            shared,
            cached_mips: OnceLock::new(),
            on_make_color_type_result: Mutex::new(None),
            added_to_raster_cache: AtomicBool::new(false),
        })
    }

    /// The pixels of the image, decoded on the first request and kept in the bitmap cache for the
    /// next ones (`SkImage_Lazy::getROPixels` with `kAllow_CachingHint`).
    // Port of: src/image/SkImage_Lazy.cpp#L114-L143 (chrome/m156)
    #[doc(alias = "getROPixels")]
    fn get_ro_pixels_cached(&self) -> Option<Bitmap> {
        let desc =
            BitmapCacheDesc::for_image(self.unique_id, self.info.width(), self.info.height());
        let mut bitmap = Bitmap::new();
        if bitmap_cache::find(&desc, &mut bitmap) {
            return Some(bitmap);
        }
        let mut rec = bitmap_cache::alloc(&desc, &self.info)?;
        // readPixelsProxy (the GPU fallback) is not ported.
        let decoded = rec.with_pixmap_mut(|pixmap| {
            let mut generator = lock(&self.shared.generator);
            generator.get_pixels_into(pixmap)
        })?;
        if !decoded {
            return None;
        }
        bitmap_cache::add(rec, &mut bitmap);
        self.added_to_raster_cache.store(true, Ordering::Relaxed);
        Some(bitmap)
    }

    /// `SkImage::makeRasterImage` for a lazy image: a raster copy of the pixels, which is the
    /// base of its subsets (`SkImage_Lazy::onMakeSubset`).
    // Port of: src/image/SkImage.cpp#L310-L338 (chrome/m156)
    fn make_raster_image(&self) -> Option<Image> {
        let info = &self.info;
        let row_bytes = info.min_row_bytes();
        let size = info.compute_byte_size(row_bytes);
        if ImageInfo::byte_size_overflowed(size) {
            return None;
        }

        let mut bytes = vec![0u8; size];
        let dst_info = info.with_color_space(None);
        if !self.on_read_pixels(&dst_info, &mut bytes, row_bytes, 0, 0) {
            return None;
        }

        images::raster_from_data(info, crate::data::Data::new_from_vec(bytes), row_bytes)
    }
}

impl Drop for ImageLazy {
    // Port of: src/image/SkImage_Base.cpp#L35-L39 (chrome/m156), `~SkImage_Base`: an image in the
    // raster cache makes its entries stale, so they are purged.
    fn drop(&mut self) {
        if self.added_to_raster_cache.load(Ordering::Relaxed) {
            bitmap_cache::notify_bitmap_gen_id_is_stale(self.unique_id);
        }
    }
}

impl fmt::Debug for ImageLazy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImageLazy")
            .field("info", &self.info)
            .field("unique_id", &self.unique_id)
            .finish_non_exhaustive()
    }
}

impl ImageBase for ImageLazy {
    fn info(&self) -> &ImageInfo {
        &self.info
    }

    fn unique_id(&self) -> u32 {
        self.unique_id
    }

    // Port of: src/image/SkImage_Lazy.h#L53 (chrome/m156)
    fn image_type(&self) -> ImageType {
        ImageType::Lazy
    }

    // Lazy images have no mipmaps of their own: the image shader builds them through
    // `cached_mips` (SkImage_Base's default onHasMipmaps).
    fn on_has_mipmaps(&self) -> bool {
        false
    }

    // Port of: src/image/SkImage_Lazy.cpp#L186-L199 (chrome/m156)
    fn on_read_pixels(
        &self,
        dst_info: &ImageInfo,
        dst_pixels: &mut [u8],
        dst_row_bytes: usize,
        src_x: i32,
        src_y: i32,
    ) -> bool {
        match self.get_ro_pixels_cached() {
            Some(bitmap) => bitmap.read_pixels(dst_info, dst_pixels, dst_row_bytes, src_x, src_y),
            None => false,
        }
    }

    // Port of: src/image/SkImage_Lazy.cpp#L40 (chrome/m156), the generator's onIsProtected
    fn on_is_protected(&self) -> bool {
        lock(&self.shared.generator).is_protected()
    }

    // Port of: src/image/SkImage_Lazy.cpp#L183-L185 (chrome/m156)
    fn on_is_valid(&self) -> bool {
        lock(&self.shared.generator).is_valid()
    }

    // Port of: src/image/SkImage_Lazy.cpp#L65-L68 (chrome/m156) and SkImage_Lazy::getROPixels
    fn get_ro_pixels(&self) -> Option<Bitmap> {
        self.get_ro_pixels_cached()
    }

    // Port of: src/image/SkImage_Lazy.cpp#L201-L212 (chrome/m156)
    fn on_ref_encoded(&self) -> Option<Data> {
        // Only the image the generator makes, not a subset or a color type change of it.
        let mut generator = lock(&self.shared.generator);
        if generator.unique_id() == self.unique_id {
            generator.ref_encoded_data()
        } else {
            None
        }
    }

    // Port of: src/core/SkMipmapCache.cpp (AddAndRef), for a lazy image's pixels
    fn cached_mips(&self) -> Option<Arc<Mipmap>> {
        self.cached_mips
            .get_or_init(|| {
                let src = self.get_ro_pixels_cached()?;
                let pixmap = src.peek_pixels()?;
                Mipmap::build(&pixmap, true).map(Arc::new)
            })
            .clone()
    }

    // Port of: src/image/SkImage_Lazy.cpp#L247-L256 (chrome/m156)
    fn on_make_subset(
        &self,
        subset: &IRect,
        required_properties: RequiredProperties,
    ) -> Option<Image> {
        // TODO in Skia: realize only the subset in the generator.
        let non_lazy = self.make_raster_image()?;
        non_lazy.make_subset(subset, required_properties)
    }

    // Port of: src/image/SkImage_Lazy.cpp#L265-L285 (chrome/m156)
    fn make_color_type_and_color_space(
        &self,
        target_color_type: ColorType,
        target_color_space: Option<ColorSpace>,
        _required_properties: RequiredProperties,
    ) -> Option<Image> {
        let mut result = lock(&self.on_make_color_type_result);
        if let Some(cached) = result.as_ref()
            && cached.color_type() == target_color_type
            && ColorSpace::equals(cached.color_space().as_ref(), target_color_space.as_ref())
        {
            return Some(cached.clone());
        }

        let validator = Validator::new(
            Some(Arc::clone(&self.shared)),
            Some(target_color_type),
            target_color_space,
        );
        let image = ImageLazy::from_validator(validator).map(Image::from_base)?;
        *result = Some(image.clone());
        Some(image)
    }

    // Port of: src/image/SkImage_Lazy.cpp#L287-L303 (chrome/m156)
    fn on_reinterpret_color_space(&self, new_cs: ColorSpace) -> Option<Image> {
        // We allocate the bitmap with the new color space, then generate the image using the
        // original one.
        let mut bitmap = Bitmap::new();
        if !bitmap.try_alloc_pixels_info(&self.info.with_color_space(Some(new_cs)), None) {
            return None;
        }
        let ok = {
            let mut pixmap = bitmap.peek_pixels_mut()?;
            pixmap.set_color_space(self.info.color_space());
            let mut generator = lock(&self.shared.generator);
            generator.get_pixels_into(&mut pixmap)
        };
        if !ok {
            return None;
        }
        bitmap.set_immutable();
        images::raster_from_bitmap(&bitmap)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
