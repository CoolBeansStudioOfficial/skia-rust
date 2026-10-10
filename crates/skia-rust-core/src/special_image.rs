// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkSpecialImage.h, src/core/SkSpecialImage.cpp, and the
// backend-independent half of src/gpu/graphite/SpecialImage_Graphite.cpp

//! `SkSpecialImage`: a subset of a device's pixels that a canvas passes from one device to
//! another (when a `saveLayer` is restored or its backdrop is snapped).
//!
//! # Flavors (`docs/design/gpu.md` §5.5)
//!
//! Skia's `SkSpecialImage` is an abstract class with a raster subclass (`SkSpecialImage_Raster`)
//! and one per GPU backend. Graphite's (`skgpu::graphite::SpecialImage`) only wraps a
//! Graphite-backed `SkImage`. Here the flavors are the variants of a private enum:
//!
//! - raster: a [`Bitmap`] that shares the pixel ref of the device it was snapped from (the
//!   device's pixels are never written while the image is alive: layers are immutable once
//!   restored, and a write to a shared pixel ref copies first, `docs/design/pixels.md`);
//! - texture: a texture-backed [`Image`] (`SpecialImage_Graphite`). Core does not know the GPU
//!   crate: the image is a core [`Image`] whose `ImageBase` is the backend's, and the backend
//!   makes these with [`SpecialImage::make_from_texture_image`] after converting an image to its
//!   own kind (`SkSpecialImages::MakeGraphite` is `skia_rust_gpu::graphite::special_image`).
//!
//! The enum keeps the raster flavor free of dynamic dispatch and allocation.

#![allow(
    // The float casts and exact float comparisons mirror the C++ arithmetic of the Skia source
    // (`SkScalar` and `int` conversions, `==` on scalars); the control flow keeps the C++ shape
    // so the port can be reviewed line by line.
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::float_cmp,
    clippy::collapsible_if,
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::manual_let_else
)]

use crate::bitmap::Bitmap;
use crate::image::Image;
use crate::image_info::{ColorInfo, ImageInfo};
use crate::matrix::Matrix;
use crate::rect::{Contains, IRect, Rect};
use crate::sampling_options::SamplingOptions;
use crate::shader::Shader;
use crate::shaders::image_shader::ImageShader;
use crate::size::ISize;
use crate::surface_props::SurfaceProps;
use crate::tile_mode::TileMode;

/// A rectangle of a backing store's pixels with its surface properties (`SkSpecialImage`).
// Port of: src/core/SkSpecialImage.h#L33-L103 (chrome/m156)
#[doc(alias = "SkSpecialImage")]
#[derive(Clone, Debug)]
pub struct SpecialImage {
    subset: IRect,
    backing: Backing,
    props: SurfaceProps,
}

/// The backing store of a [`SpecialImage`]: the subclasses of `SkSpecialImage`.
#[derive(Clone, Debug)]
enum Backing {
    /// `SkSpecialImage_Raster::fBitmap`.
    Raster(Bitmap),
    /// `skgpu::graphite::SpecialImage::fImage`: a texture-backed image.
    Texture(Image),
}

impl SpecialImage {
    /// A special image over `subset` of `bm`, sharing its pixels; `None` if `bm` has no pixels
    /// (`SkSpecialImages::MakeFromRaster`).
    // Port of: src/core/SkSpecialImage.cpp#L120-L129 (chrome/m156)
    #[doc(alias = "MakeFromRaster")]
    #[must_use]
    pub fn make_from_raster(
        subset: &IRect,
        bm: &Bitmap,
        props: &SurfaceProps,
    ) -> Option<SpecialImage> {
        debug_assert!(bm.bounds().contains(subset));

        bm.pixel_ref()?;
        Some(SpecialImage {
            subset: *subset,
            backing: Backing::Raster(bm.clone()),
            props: *props,
        })
    }

    /// A special image over `subset` of the texture-backed `image`
    /// (`skgpu::graphite::SpecialImage(subset, image, props)`); `None` if `image` is not texture
    /// backed or `subset` is empty.
    ///
    /// The backend's `MakeGraphite` converts a non-texture image first and then calls this.
    // Port of: src/gpu/graphite/SpecialImage_Graphite.cpp#L27-L33, L60-L66 (chrome/m156)
    #[must_use]
    pub fn make_from_texture_image(
        subset: &IRect,
        image: Image,
        props: &SurfaceProps,
    ) -> Option<SpecialImage> {
        if subset.is_empty() || !image.is_texture_backed() {
            return None;
        }
        debug_assert!(image.bounds().contains(subset));
        Some(SpecialImage {
            subset: *subset,
            backing: Backing::Texture(image),
            props: *props,
        })
    }

    /// A special image holding a copy of `subset` of `bm`; `None` if `bm` has no pixels or the
    /// copy fails (`SkSpecialImages::CopyFromRaster`).
    // Port of: src/core/SkSpecialImage.cpp#L131-L152 (chrome/m156)
    #[doc(alias = "CopyFromRaster")]
    #[must_use]
    pub fn copy_from_raster(
        subset: &IRect,
        bm: &Bitmap,
        props: &SurfaceProps,
    ) -> Option<SpecialImage> {
        debug_assert!(bm.bounds().contains(subset));

        bm.pixel_ref()?;
        let mut tmp = Bitmap::new();
        let info = bm.info().with_dimensions(subset.size());
        if !tmp.try_alloc_pixels_info(&info, None) {
            return None;
        }
        let row_bytes = tmp.row_bytes();
        let tmp_info = tmp.info().clone();
        {
            let mut dst = tmp.peek_pixels_mut()?;
            let dst_pixels = dst.writable_addr()?;
            if !bm.read_pixels(&tmp_info, dst_pixels, row_bytes, subset.left, subset.top) {
                return None;
            }
        }
        // Since we're making a copy of the raster, the resulting special image is the exact size
        // of the requested subset of the original and no longer needs to be offset by subset's
        // left and top, since those were relative to the original's buffer.
        Some(SpecialImage {
            subset: IRect::from_wh(subset.width(), subset.height()),
            backing: Backing::Raster(tmp),
            props: *props,
        })
    }

    /// The bounds of the image within its backing store (`subset`).
    #[must_use]
    pub fn subset(&self) -> IRect {
        self.subset
    }

    /// The width of the image (`width`).
    #[must_use]
    pub fn width(&self) -> i32 {
        self.subset.width()
    }

    /// The height of the image (`height`).
    #[must_use]
    pub fn height(&self) -> i32 {
        self.subset.height()
    }

    /// The dimensions of the image (`dimensions`).
    #[must_use]
    pub fn dimensions(&self) -> ISize {
        self.subset.size()
    }

    /// The color info of the pixels (`colorInfo`).
    #[doc(alias = "colorInfo")]
    #[must_use]
    pub fn color_info(&self) -> &ColorInfo {
        self.backing_store_info().color_info()
    }

    /// The image info of the whole backing store.
    #[must_use]
    pub fn backing_store_info(&self) -> &ImageInfo {
        match &self.backing {
            Backing::Raster(bitmap) => bitmap.info(),
            Backing::Texture(image) => image.image_info(),
        }
    }

    /// Whether the backing store is a GPU texture (`isGraphiteBacked() || isGaneshBacked()`).
    #[doc(alias = "isGraphiteBacked")]
    #[must_use]
    pub fn is_texture_backed(&self) -> bool {
        matches!(self.backing, Backing::Texture(_))
    }

    /// `isExactFit()`: whether the subset is the whole backing store.
    // Port of: src/core/SkSpecialImage.h#L67 (chrome/m156)
    #[doc(alias = "isExactFit")]
    #[must_use]
    pub fn is_exact_fit(&self) -> bool {
        self.subset == IRect::from_size(self.backing_store_dimensions())
    }

    /// The surface properties of the device the image was snapped from (`props`).
    #[must_use]
    pub fn props(&self) -> &SurfaceProps {
        &self.props
    }

    /// The dimensions of the whole backing store (`backingStoreDimensions`).
    // Port of: src/core/SkSpecialImage.h#L63 (chrome/m156)
    #[doc(alias = "backingStoreDimensions")]
    #[must_use]
    pub fn backing_store_dimensions(&self) -> ISize {
        match &self.backing {
            // Port of: src/core/SkSpecialImage.cpp#L80 (chrome/m156)
            Backing::Raster(bitmap) => bitmap.dimensions(),
            // Port of: src/gpu/graphite/SpecialImage_Graphite.cpp#L41-L43 (chrome/m156)
            Backing::Texture(image) => image.dimensions(),
        }
    }

    /// `onMakeBackingStoreSubset(subset)`: a special image of `subset` of the same backing store.
    fn on_make_backing_store_subset(&self, subset: &IRect) -> Option<SpecialImage> {
        match &self.backing {
            // No need to extract subset, onGetROPixels handles that when needed
            // Port of: src/core/SkSpecialImage.cpp#L84-L87 (chrome/m156)
            Backing::Raster(bitmap) => SpecialImage::make_from_raster(subset, bitmap, &self.props),
            // Port of: src/gpu/graphite/SpecialImage_Graphite.cpp#L45-L48 (chrome/m156)
            Backing::Texture(image) => {
                debug_assert!(image.bounds().contains(subset));
                Some(SpecialImage {
                    subset: *subset,
                    backing: Backing::Texture(image.clone()),
                    props: self.props,
                })
            }
        }
    }

    /// `makeSubset(subset)`: a special image of `subset` relative to this image's subset, sharing
    /// the backing store; `None` if the backing store cannot make it.
    // Port of: src/core/SkSpecialImage.h#L91-L94 (chrome/m156)
    #[doc(alias = "makeSubset")]
    #[must_use]
    pub fn make_subset(&self, subset: &IRect) -> Option<SpecialImage> {
        let absolute = subset.with_offset(self.subset.top_left());
        self.on_make_backing_store_subset(&absolute)
    }

    /// `makePixelOutset()`: a special image with a 1px larger subset in the backing store. Only
    /// used when the outer pixels are known to be valid.
    // Port of: src/core/SkSpecialImage.h#L100-L102 (chrome/m156)
    #[doc(alias = "makePixelOutset")]
    #[must_use]
    /// # Panics
    ///
    /// Never: the bitmap of a special image has pixels.
    pub fn make_pixel_outset(&self) -> SpecialImage {
        let outset = self.subset.with_outset((1, 1));
        self.on_make_backing_store_subset(&outset)
            .expect("a special image's bitmap has pixels")
    }

    /// `asImage()`: an image of the whole backing store, sharing its pixels.
    // Port of: src/core/SkSpecialImage.cpp#L81 (chrome/m156), SkSpecialImage_Raster::asImage, and
    // src/gpu/graphite/SpecialImage_Graphite.cpp#L50 (chrome/m156)
    #[doc(alias = "asImage")]
    #[must_use]
    pub fn as_image(&self) -> Option<Image> {
        match &self.backing {
            Backing::Raster(bitmap) => bitmap.as_image(),
            Backing::Texture(image) => Some(image.clone()),
        }
    }

    /// `asShader(tileMode, sampling, lm, strict)` of the raster flavor: a shader over the subset
    /// (`strict`: only the subset is sampled) or over the backing store offset to the subset.
    // Port of: src/core/SkSpecialImage.cpp#L38-L52 (chrome/m156), SkSpecialImage_Raster::asShader
    #[doc(alias = "asShader")]
    #[must_use]
    pub fn as_shader(
        &self,
        tile_mode: TileMode,
        sampling: SamplingOptions,
        local_matrix: &Matrix,
        strict: bool,
    ) -> Option<Shader> {
        let bitmap = match &self.backing {
            Backing::Raster(bitmap) => bitmap,
            Backing::Texture(image) => {
                return self.as_shader_base(image, tile_mode, sampling, local_matrix, strict);
            }
        };
        if strict {
            let subset_bm = self.as_bitmap()?;
            return subset_bm
                .as_image()?
                .to_shader((tile_mode, tile_mode), sampling, local_matrix);
        }
        // SkMatrix::Translate(-subset.topLeft()) followed by postConcat(lm)
        let origin = self.subset.top_left();
        let mut subset_origin = Matrix::translate((-(origin.x as f32), -(origin.y as f32)));
        subset_origin.post_concat(local_matrix);
        bitmap
            .as_image()?
            .to_shader((tile_mode, tile_mode), sampling, &subset_origin)
    }

    /// `SkSpecialImage::asShader` (the base class's, which the GPU flavors use): a subset image
    /// shader of [`as_image`](Self::as_image) when `strict`, else a shader of the whole image.
    // Port of: src/core/SkSpecialImage.cpp#L38-L58 (chrome/m156)
    fn as_shader_base(
        &self,
        image: &Image,
        tile_mode: TileMode,
        sampling: SamplingOptions,
        local_matrix: &Matrix,
        strict: bool,
    ) -> Option<Shader> {
        // The special image's logical (0,0) is at its subset's topLeft() so we need to account for
        // that in the local matrix used when sampling.
        let origin = self.subset.top_left();
        let mut subset_origin = Matrix::translate((-(origin.x as f32), -(origin.y as f32)));
        subset_origin.post_concat(local_matrix);
        if strict {
            // However, we don't need to modify the subset itself since that is defined with
            // respect to the base image, and the local matrix is applied before any
            // tiling/clamping.
            let subset = Rect::from_irect(self.subset);
            // asImage() w/o a subset makes no copy; create the SkImageShader directly to
            // remember the subset used to access the image.
            ImageShader::make_subset(
                Some(image.clone()),
                &subset,
                tile_mode,
                tile_mode,
                &sampling,
                Some(&subset_origin),
                false,
            )
        } else {
            // Ignore 'subset' other than its origin translation applied to the local matrix.
            image.to_shader((tile_mode, tile_mode), sampling, &subset_origin)
        }
    }

    /// The pixels of the subset as a bitmap sharing the backing store (`SkSpecialImages::AsBitmap`,
    /// `getROPixels`); `None` if the subset does not intersect the bitmap, or for a texture-backed
    /// image.
    // Port of: src/core/SkSpecialImage.cpp#L77-L79, L172-L178 (chrome/m156)
    #[doc(alias = "AsBitmap")]
    #[must_use]
    pub fn as_bitmap(&self) -> Option<Bitmap> {
        let Backing::Raster(bitmap) = &self.backing else {
            return None;
        };
        let mut bm = Bitmap::new();
        if bitmap.extract_subset(&mut bm, self.subset) {
            Some(bm)
        } else {
            None
        }
    }

    /// If the special image is backed by a GPU texture, true (`isGaneshBacked`). A special image
    /// here is always raster-backed (`SkSpecialImage`'s default), so this is always false.
    // Port of: src/core/SkSpecialImage.h#L131-L132 (chrome/m156)
    #[doc(alias = "isGaneshBacked")]
    #[must_use]
    pub fn is_ganesh_backed(&self) -> bool {
        false
    }
}
