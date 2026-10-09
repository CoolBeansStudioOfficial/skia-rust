// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkPixmapDraw.cpp (SkPixmap::scalePixels), src/image/SkImage.cpp
// (SkImage::scalePixels)

//! The parts of `SkPixmap` and `SkImage` that draw: `scalePixels`, which draws the source through
//! an image shader into the destination. They live here because drawing needs the raster
//! backend.
//!
//! skia-rust: `SkImage::scalePixels` takes no caching hint: the only pixels it reads are the
//! image's own (`getROPixels`), so the hint would change nothing. The shader's image is a copy of
//! the source pixels (`bitmap.asImage()` in Skia shares them), which is numerically the same.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::image::Image;
use skia_rust_core::images;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shaders::ImageShader;
use skia_rust_core::tile_mode::TileMode;

use crate::surfaces;

/// `SkPixmap::scalePixels(actualDst, sampling)`: scales `src` into `actual_dst`, returning false
/// if either is empty or the scaling could not be done.
// Port of: src/core/SkPixmapDraw.cpp#L29-L81 (chrome/m156)
#[doc(alias = "scalePixels")]
pub fn scale_pixels(
    src: &Pixmap<'_>,
    actual_dst: &mut Pixmap<'_>,
    sampling: &SamplingOptions,
) -> bool {
    // Can't do anything with an empty src or dst.
    if src.width() <= 0 || src.height() <= 0 || actual_dst.width() <= 0 || actual_dst.height() <= 0
    {
        return false;
    }

    // no scaling involved?
    if src.width() == actual_dst.width() && src.height() == actual_dst.height() {
        return src.read_pixels_to_pixmap(actual_dst, (0, 0));
    }

    // If src and dst are both unpremul, we'll fake the source out to appear as if premul, and
    // mark the destination as opaque. This odd combination allows us to scale unpremul pixels
    // without ever premultiplying them (perhaps losing information in the color channels). This
    // is an idiosyncratic feature of scalePixels(), and is tested by the scalepixels_unpremul GM.
    let mut src_info = src.info().clone();
    let mut dst_info = actual_dst.info().clone();
    let mut clamp_as_if_unpremul = false;
    if src.alpha_type() == AlphaType::Unpremul && actual_dst.alpha_type() == AlphaType::Unpremul {
        src_info = src_info.with_alpha_type(AlphaType::Premul);
        dst_info = dst_info.with_alpha_type(AlphaType::Opaque);

        // We'll need to tell the image shader to clamp to [0,1] instead of the usual [0,a] when
        // using a bicubic scaling (kHigh_SkFilterQuality).
        clamp_as_if_unpremul = true;
    }

    // SkBitmap bitmap; bitmap.installPixels(src); bitmap.setImmutable(); bitmap.asImage()
    let Some(src_bytes) = src.addr() else {
        return false;
    };
    let Some(src_pm) = Pixmap::new_readonly(&src_info, src_bytes, src.row_bytes()) else {
        return false;
    };
    let Some(image) = images::raster_from_pixmap_copy(&src_pm) else {
        return false;
    };

    let scale = Matrix::rect_to_rect_or_identity(
        Rect::from_irect(src_pm.bounds()),
        Rect::from_irect(actual_dst.bounds()),
        None,
    );

    let shader = ImageShader::make(
        Some(image),
        TileMode::Clamp,
        TileMode::Clamp,
        sampling,
        Some(&scale),
        clamp_as_if_unpremul,
    );

    let row_bytes = actual_dst.row_bytes();
    let Some(dst_bytes) = actual_dst.writable_addr() else {
        return false;
    };
    let Some(mut surface) = surfaces::wrap_pixels(&dst_info, dst_bytes, row_bytes, None) else {
        return false;
    };
    let Some(shader) = shader else {
        return false;
    };

    let mut paint = Paint::default();
    paint.set_blend_mode(BlendMode::Src);
    paint.set_shader(shader);
    surface.canvas().draw_paint(&paint);
    true
}

/// `SkImage::scalePixels(dst, sampling)`: the pixels of an image, scaled into `dst`.
pub trait ImageScalePixels {
    /// See [`scale_pixels`]. Returns false if the image's pixels cannot be read.
    #[doc(alias = "scalePixels")]
    fn scale_pixels(&self, dst: &mut Pixmap<'_>, sampling: &SamplingOptions) -> bool;
}

impl ImageScalePixels for Image {
    // Port of: src/image/SkImage.cpp#L159-L180 (chrome/m156)
    fn scale_pixels(&self, dst: &mut Pixmap<'_>, sampling: &SamplingOptions) -> bool {
        if self.width() == dst.width() && self.height() == dst.height() {
            return self.read_pixels_to_pixmap(dst, (0, 0));
        }

        // Note: By calling the pixmap scaler, we never cache the final result.
        if let Some(bm) = self.get_ro_pixels()
            && let Some(pmap) = bm.peek_pixels()
        {
            return scale_pixels(&pmap, dst, sampling);
        }
        false
    }
}
