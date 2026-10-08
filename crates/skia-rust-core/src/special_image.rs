// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkSpecialImage.h, src/core/SkSpecialImage.cpp (raster only)

//! `SkSpecialImage`: a subset of a device's pixels that a canvas passes from one device to
//! another (when a `saveLayer` is restored or its backdrop is snapped).
//!
//! skia-rust: only the raster flavor (`SkSpecialImage_Raster`) is ported. The GPU flavors are not
//! in scope, and `asImage`/`asShader`/`makeSubset` need `SkImage` and the image shader (Phase 3);
//! `SkSpecialImages::MakeFromRaster(subset, SkImage)` likewise. A special image holds a
//! [`Bitmap`] that shares the pixel ref of the device it was snapped from (the device's pixels are
//! never written while the image is alive: layers are immutable once restored, and a write to a
//! shared pixel ref copies first, `docs/design/pixels.md`).

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
use crate::rect::{Contains, IRect};
use crate::sampling_options::SamplingOptions;
use crate::shader::Shader;
use crate::size::ISize;
use crate::surface_props::SurfaceProps;
use crate::tile_mode::TileMode;

/// A rectangle of raster pixels with its surface properties (`SkSpecialImage`).
// Port of: src/core/SkSpecialImage.h#L33-L103 (chrome/m156)
#[doc(alias = "SkSpecialImage")]
#[derive(Clone, Debug)]
pub struct SpecialImage {
    subset: IRect,
    bitmap: Bitmap,
    props: SurfaceProps,
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
            bitmap: bm.clone(),
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
            bitmap: tmp,
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
        self.bitmap.info().color_info()
    }

    /// The image info of the whole backing store.
    #[must_use]
    pub fn backing_store_info(&self) -> &ImageInfo {
        self.bitmap.info()
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
        self.bitmap.dimensions()
    }

    /// `makeSubset(subset)`: a special image of `subset` relative to this image's subset, sharing
    /// the backing store; `None` if the backing store cannot make it.
    // Port of: src/core/SkSpecialImage.h#L91-L94 (chrome/m156)
    #[doc(alias = "makeSubset")]
    #[must_use]
    pub fn make_subset(&self, subset: &IRect) -> Option<SpecialImage> {
        let absolute = subset.with_offset(self.subset.top_left());
        // `onMakeBackingStoreSubset`: the raster flavor shares the bitmap.
        SpecialImage::make_from_raster(&absolute, &self.bitmap, &self.props)
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
        SpecialImage::make_from_raster(&outset, &self.bitmap, &self.props)
            .expect("a special image's bitmap has pixels")
    }

    /// `asImage()`: an image of the whole backing store, sharing its pixels.
    // Port of: src/core/SkSpecialImage.cpp#L60-L65 (chrome/m156)
    #[doc(alias = "asImage")]
    #[must_use]
    pub fn as_image(&self) -> Option<Image> {
        self.bitmap.as_image()
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
        self.bitmap
            .as_image()?
            .to_shader((tile_mode, tile_mode), sampling, &subset_origin)
    }

    /// The pixels of the subset as a bitmap sharing the backing store (`SkSpecialImages::AsBitmap`,
    /// `getROPixels`); `None` if the subset does not intersect the bitmap.
    // Port of: src/core/SkSpecialImage.cpp#L77-L79, L172-L178 (chrome/m156)
    #[doc(alias = "AsBitmap")]
    #[must_use]
    pub fn as_bitmap(&self) -> Option<Bitmap> {
        let mut bm = Bitmap::new();
        if self.bitmap.extract_subset(&mut bm, self.subset) {
            Some(bm)
        } else {
            None
        }
    }
}
