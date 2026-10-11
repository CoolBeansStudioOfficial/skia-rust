// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/image/SkRescaleAndReadPixels.cpp, the CPU paths of
// src/image/SkImage.cpp (asyncRescaleAndReadPixels), src/image/SkImage_Base.cpp
// (onAsyncRescaleAndReadPixels), src/image/SkSurface.cpp and src/image/SkSurface_Base.cpp.

//! Rescaling and reading pixels asynchronously on the CPU: the callback is always called before
//! the call returns, with the result (`AsyncReadResult`) or `None`.

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::{Image, RescaleGamma, RescaleMode};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_info_priv::image_info_is_valid;
use skia_rust_core::images;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::{Contains, IRect, Rect};
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, SamplingOptions};
use skia_rust_core::size::ISize;

use crate::surface::Surface;
use crate::surfaces;

/// `SkImage::AsyncReadResult` on the CPU: the pixels of one plane (`data(0)`, `rowBytes(0)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncReadResult {
    data: Vec<u8>,
    row_bytes: usize,
}

impl AsyncReadResult {
    /// The number of planes; always 1 on the CPU (`count`).
    #[must_use]
    pub fn count(&self) -> usize {
        1
    }

    /// The pixels of the plane (`data(0)`).
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// The row bytes of the plane (`rowBytes(0)`).
    #[must_use]
    pub fn row_bytes(&self) -> usize {
        self.row_bytes
    }

    /// The pixels and row bytes of the plane, by value.
    #[must_use]
    pub fn into_parts(self) -> (Vec<u8>, usize) {
        (self.data, self.row_bytes)
    }
}

/// `SkRescaleAndReadPixels`' sampling choice for a rescale mode (`rescaling_to_sampling`).
// Port of: src/image/SkRescaleAndReadPixels.cpp#L35-L44 (chrome/m156)
fn rescaling_to_sampling(rescale_mode: RescaleMode) -> SamplingOptions {
    match rescale_mode {
        RescaleMode::RepeatedLinear => SamplingOptions::from(FilterMode::Linear),
        RescaleMode::RepeatedCubic => SamplingOptions::from(CubicResampler {
            b: 1.0 / 3.0,
            c: 1.0 / 3.0,
        }),
        _ => SamplingOptions::default(),
    }
}

/// `static_cast<int>((s > 1.f) ? std::ceil(std::log2f(s)) : std::floor(std::log2f(s)))`.
// Port of: src/image/SkRescaleAndReadPixels.cpp#L24-L30 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ static_cast<int>; the value is integral
fn log2_steps(s: f32) -> i32 {
    if s > 1.0 {
        s.log2().ceil() as i32
    } else {
        s.log2().floor() as i32
    }
}

/// `SkRescaleAndReadPixels(bmp, resultInfo, srcRect, rescaleGamma, rescaleMode, callback)`: rescales
/// the `src` pixels in halving steps (`RepeatedLinear`/`RepeatedCubic`) or in one step, optionally
/// in linear gamma, and reads the result into `result_info`.
// Port of: src/image/SkRescaleAndReadPixels.cpp#L19-L166 (chrome/m156)
// The allows mirror the single C++ function: `sx != 1.f` is an exact test, and the offsets and sizes
// are image coordinates, far below the 2^24 where an i32 stops converting exactly to f32.
#[allow(clippy::too_many_lines, clippy::float_cmp, clippy::cast_precision_loss)]
pub fn rescale_and_read_pixels(
    src: &Pixmap<'_>,
    result_info: &ImageInfo,
    src_rect: IRect,
    rescale_gamma: RescaleGamma,
    mut rescale_mode: RescaleMode,
    callback: impl FnOnce(Option<AsyncReadResult>),
) {
    let mut src_w = src_rect.width();
    let mut src_h = src_rect.height();
    #[allow(clippy::cast_precision_loss)] // (float)resultInfo.width() / srcW in C++
    let sx = result_info.width() as f32 / src_w as f32;
    #[allow(clippy::cast_precision_loss)]
    let sy = result_info.height() as f32 / src_h as f32;

    // How many bilerp/bicubic steps to do in X and Y. + means upscaling, - means downscaling.
    let (mut steps_x, mut steps_y) = if rescale_mode == RescaleMode::Nearest {
        (i32::from(sx != 1.0), i32::from(sy != 1.0))
    } else {
        (log2_steps(sx), log2_steps(sy))
    };

    let mut paint = Paint::default();
    paint.set_blend_mode(BlendMode::Src);

    if (steps_x < 0 || steps_y < 0) && rescale_mode != RescaleMode::Nearest {
        // Don't trigger MIP generation. We don't currently have a way to trigger bicubic for
        // downscaling draws.
        rescale_mode = RescaleMode::RepeatedLinear;
    }

    let sampling = rescaling_to_sampling(rescale_mode);

    let mut src_x = src_rect.left;
    let mut src_y = src_rect.top;
    let mut constraint = SrcRectConstraint::Strict;

    // Assume we should ignore the rescale linear request if the surface has no color space since
    // it's unclear how we'd linearize from an unknown color space.
    let src_cs = src.info().color_space();
    let mut src_image: Image = match src_cs {
        Some(cs) if rescale_gamma == RescaleGamma::Linear && !cs.gamma_is_linear() => {
            let cs = cs.make_linear_gamma();
            // Promote to F16 color type to preserve precision.
            let ii = ImageInfo::new(
                ISize::new(src_w, src_h),
                ColorType::RGBAF16,
                src.info().alpha_type(),
                Some(cs),
            );
            let Some(mut linear_surf) = surfaces::raster(&ii, None, None) else {
                callback(None);
                return;
            };
            let Some(bmp_image) = images::raster_from_pixmap_copy(src) else {
                callback(None);
                return;
            };
            linear_surf.canvas().draw_image_with_sampling_options(
                &bmp_image,
                (-src_x as f32, -src_y as f32),
                sampling,
                Some(&paint),
            );
            let Some(image) = linear_surf.image_snapshot() else {
                callback(None);
                return;
            };
            src_x = 0;
            src_y = 0;
            constraint = SrcRectConstraint::Fast;
            image
        }
        _ => {
            // MakeFromBitmap would trigger a copy if bmp is mutable.
            let Some(image) = images::raster_from_pixmap_copy(src) else {
                callback(None);
                return;
            };
            image
        }
    };

    while steps_x != 0 || steps_y != 0 {
        let mut next_w = result_info.width();
        let mut next_h = result_info.height();
        if steps_x < 0 {
            next_w = result_info.width() << (-steps_x - 1);
            steps_x += 1;
        } else if steps_x != 0 {
            if steps_x > 1 {
                next_w = src_w * 2;
            }
            steps_x -= 1;
        }
        if steps_y < 0 {
            next_h = result_info.height() << (-steps_y - 1);
            steps_y += 1;
        } else if steps_y != 0 {
            if steps_y > 1 {
                next_h = src_h * 2;
            }
            steps_y -= 1;
        }

        let mut ii = src_image.image_info().with_wh(next_w, next_h);
        if steps_x == 0 && steps_y == 0 {
            // Might as well fold conversion to final info in the last step.
            ii = result_info.clone();
        }

        let Some(mut next) = surfaces::raster(&ii, None, None) else {
            callback(None);
            return;
        };
        next.canvas().draw_image_rect_with_sampling_options(
            &src_image,
            Some((
                &Rect::from_irect(IRect::from_xywh(src_x, src_y, src_w, src_h)),
                constraint,
            )),
            Rect::from_iwh(next_w, next_h),
            sampling,
            &paint,
        );
        let Some(image) = next.image_snapshot() else {
            callback(None);
            return;
        };
        src_image = image;
        src_x = 0;
        src_y = 0;
        src_w = next_w;
        src_h = next_h;
        constraint = SrcRectConstraint::Fast;
    }

    let row_bytes = result_info.min_row_bytes();
    let mut data = vec![0_u8; usize::try_from(result_info.height()).unwrap_or(0) * row_bytes];
    let read = match Pixmap::new(result_info, &mut data, row_bytes) {
        Some(mut pm) => src_image.read_pixels_to_pixmap(&mut pm, IPoint::new(src_x, src_y)),
        None => false,
    };
    if read {
        callback(Some(AsyncReadResult { data, row_bytes }));
    } else {
        callback(None);
    }
}

/// The CPU `SkImage::asyncRescaleAndReadPixels`: the pixels of an image scaled to `info`.
pub trait ImageAsyncRescaleAndReadPixels {
    /// Calls `callback` with the rescaled pixels, or `None` if they cannot be read
    /// (`SkImage::asyncRescaleAndReadPixels`, synchronous on the CPU).
    #[doc(alias = "asyncRescaleAndReadPixels")]
    fn async_rescale_and_read_pixels(
        &self,
        info: &ImageInfo,
        src_rect: IRect,
        rescale_gamma: RescaleGamma,
        rescale_mode: RescaleMode,
        callback: impl FnOnce(Option<AsyncReadResult>),
    );
}

impl ImageAsyncRescaleAndReadPixels for Image {
    // Port of: src/image/SkImage.cpp#L182-L190 (chrome/m156), asyncRescaleAndReadPixels
    fn async_rescale_and_read_pixels(
        &self,
        info: &ImageInfo,
        src_rect: IRect,
        rescale_gamma: RescaleGamma,
        rescale_mode: RescaleMode,
        callback: impl FnOnce(Option<AsyncReadResult>),
    ) {
        if !IRect::from_wh(self.width(), self.height()).contains(src_rect)
            || !image_info_is_valid(info)
        {
            callback(None);
            return;
        }
        // Port of: src/image/SkImage_Base.cpp#L41-L66 (chrome/m156), onAsyncRescaleAndReadPixels
        if let Some(peeked) = self.peek_pixels() {
            rescale_and_read_pixels(
                &peeked,
                info,
                src_rect,
                rescale_gamma,
                rescale_mode,
                callback,
            );
            return;
        }
        let mut src = Bitmap::new();
        src.alloc_pixels_flags(
            &self
                .image_info()
                .with_dimensions(ISize::new(src_rect.width(), src_rect.height())),
        );
        let read = match src.peek_pixels_mut() {
            Some(mut dst) => {
                self.read_pixels_to_pixmap(&mut dst, IPoint::new(src_rect.left, src_rect.top))
            }
            None => false,
        };
        if !read {
            callback(None);
            return;
        }
        let Some(src_pixels) = src.peek_pixels() else {
            callback(None);
            return;
        };
        rescale_and_read_pixels(
            &src_pixels,
            info,
            IRect::from_wh(src_rect.width(), src_rect.height()),
            rescale_gamma,
            rescale_mode,
            callback,
        );
    }
}

/// The CPU `SkSurface::asyncRescaleAndReadPixels` of a raster surface.
pub trait SurfaceAsyncRescaleAndReadPixels {
    /// Calls `callback` with the rescaled pixels, or `None` if they cannot be read
    /// (`SkSurface::asyncRescaleAndReadPixels`, synchronous on the CPU).
    #[doc(alias = "asyncRescaleAndReadPixels")]
    fn async_rescale_and_read_pixels(
        &mut self,
        info: &ImageInfo,
        src_rect: IRect,
        rescale_gamma: RescaleGamma,
        rescale_mode: RescaleMode,
        callback: impl FnOnce(Option<AsyncReadResult>),
    );
}

impl SurfaceAsyncRescaleAndReadPixels for Surface<'_> {
    // Port of: src/image/SkSurface.cpp#L143-L156 (chrome/m156), asyncRescaleAndReadPixels
    fn async_rescale_and_read_pixels(
        &mut self,
        info: &ImageInfo,
        src_rect: IRect,
        rescale_gamma: RescaleGamma,
        rescale_mode: RescaleMode,
        callback: impl FnOnce(Option<AsyncReadResult>),
    ) {
        let image_info = self.image_info();
        if !IRect::from_wh(image_info.width(), image_info.height()).contains(src_rect)
            || !image_info_is_valid(info)
        {
            callback(None);
            return;
        }
        // Port of: src/image/SkSurface_Base.cpp#L64-L87 (chrome/m156), onAsyncRescaleAndReadPixels
        if let Some(peeked) = self.peek_pixels() {
            let pixels = peeked.pixmap();
            rescale_and_read_pixels(
                &pixels,
                info,
                src_rect,
                rescale_gamma,
                rescale_mode,
                callback,
            );
            return;
        }
        let mut src = Bitmap::new();
        src.alloc_pixels_flags(
            &image_info.with_dimensions(ISize::new(src_rect.width(), src_rect.height())),
        );
        let read = match src.peek_pixels_mut() {
            Some(mut dst) => {
                self.read_pixels_to_pixmap(&mut dst, IPoint::new(src_rect.left, src_rect.top))
            }
            None => false,
        };
        if !read {
            callback(None);
            return;
        }
        let Some(src_pixels) = src.peek_pixels() else {
            callback(None);
            return;
        };
        rescale_and_read_pixels(
            &src_pixels,
            info,
            IRect::from_wh(src_rect.width(), src_rect.height()),
            rescale_gamma,
            rescale_mode,
            callback,
        );
    }
}
