// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/Context.cpp (the asyncRescaleAndReadPixels overloads)

//! `Context::asyncRescaleAndReadPixels`: reads back an image or surface, rescaling it first when
//! the source rect and the destination size differ.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::{Image as CoreImage, RescaleGamma, RescaleMode};
use skia_rust_core::image_info::{ImageInfo, YUVColorSpace};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{Contains, IRect};
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::size::ISize;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::yuv_math::color_matrix_rgb2yuv;

use crate::gpu::backing_fit::BackingFit;
use crate::gpu::gpu_types::{Budgeted, Mipmapped};
use crate::gpu::sk_log::skia_log_w;
use crate::graphite::async_read::{AsyncReadParams, PixelTransferResult, ReadPixelsCallback};
use crate::graphite::recorder::Recorder;
use crate::graphite::surface_graphite::Surface;
use crate::graphite::texture_format_xfer_fn::TextureFormatXferFn;
use crate::graphite::texture_utils::{as_view, rescale_image};
use crate::graphite::wgpu::context::WgpuContext;

/// What an asynchronous rescale-and-read produces from the (rescaled) source image.
#[derive(Clone, Copy, Debug)]
enum ReadKind {
    /// `asyncReadPixels`: the destination's pixels in its image info.
    Pixels,
    /// `asyncReadPixelsYUV420`: the destination's YUV planes (and alpha, when the destination's
    /// alpha type is not opaque), converted with `yuv_color_space`.
    Yuv420(YUVColorSpace),
}

/// `Context::AsyncParams<SkImage>`: the source image of an asynchronous rescale-and-read, the
/// rect of it to read, the destination image info and the callback.
// Port of: src/gpu/graphite/Context.cpp#L326-L355 (chrome/m156)
#[doc(alias = "AsyncParams")]
struct AsyncRescaleParams {
    // `fSrcImage`.
    src_image: CoreImage,
    // `fSrcRect`.
    src_rect: IRect,
    // `fDstImageInfo`.
    dst_image_info: ImageInfo,
    // `fCallback` and `fCallbackContext`.
    callback: ReadPixelsCallback,
}

impl AsyncRescaleParams {
    // Port of: src/gpu/graphite/Context.cpp#L326-L355 (chrome/m156), `validate`
    fn validate(&self) -> bool {
        // SkImage::isProtected() -> bool.
        if self.src_image.is_protected() {
            return false;
        }
        if !IRect::from_size(self.src_image.dimensions()).contains(self.src_rect) {
            return false;
        }
        skia_rust_core::image_info_priv::image_info_is_valid(&self.dst_image_info)
    }

    // Port of: src/gpu/graphite/Context.cpp#L326-L355 (chrome/m156), `fail`
    fn fail(self) {
        (self.callback)(None);
    }

    // Port of: src/gpu/graphite/Context.cpp#L326-L355 (chrome/m156), `withNewSource`
    fn with_new_source(self, src_image: CoreImage, src_rect: IRect) -> Self {
        Self {
            src_image,
            src_rect,
            ..self
        }
    }
}

impl WgpuContext {
    /// `asyncRescaleAndReadPixels(src, dstImageInfo, srcRect, rescaleGamma, rescaleMode,
    /// callback)` for an image: `callback` is called with the pixels of `src_rect`, rescaled to
    /// `dst_image_info`, or with `None` if the read failed.
    // Port of: src/gpu/graphite/Context.cpp#L357-L367 (chrome/m156)
    #[doc(alias = "asyncRescaleAndReadPixels")]
    pub fn async_rescale_and_read_pixels(
        &mut self,
        src: &CoreImage,
        dst_image_info: &ImageInfo,
        src_rect: IRect,
        rescale_gamma: RescaleGamma,
        rescale_mode: RescaleMode,
        callback: ReadPixelsCallback,
    ) {
        self.async_rescale_and_read_impl(
            AsyncRescaleParams {
                src_image: src.clone(),
                src_rect,
                dst_image_info: dst_image_info.clone(),
                callback,
            },
            rescale_gamma,
            rescale_mode,
            ReadKind::Pixels,
        );
    }

    /// `asyncRescaleAndReadPixels(src, ...)` for a surface: the surface's image is read.
    // Port of: src/gpu/graphite/Context.cpp#L369-L409 (chrome/m156)
    #[doc(alias = "asyncRescaleAndReadPixels")]
    pub fn async_rescale_and_read_pixels_surface(
        &mut self,
        src: &Surface,
        dst_image_info: &ImageInfo,
        src_rect: IRect,
        rescale_gamma: RescaleGamma,
        rescale_mode: RescaleMode,
        callback: ReadPixelsCallback,
    ) {
        // `SkSurfaces::AsImage(src)` is the surface's image view for a Graphite surface, which is
        // always made with the surface (`Surface::new` fails for a target that cannot be
        // sampled), so Skia's direct readback of a non-texturable surface has no caller here.
        let surface_image = src.as_image();
        self.async_rescale_and_read_pixels(
            &surface_image,
            dst_image_info,
            src_rect,
            rescale_gamma,
            rescale_mode,
            callback,
        );
    }

    /// `asyncRescaleAndReadImpl(asyncRead, rescaleGamma, rescaleMode, params, ...)`: reads
    /// `params.src_rect` directly when it has the destination's size, and otherwise rescales it
    /// into a scratch image first.
    // Port of: src/gpu/graphite/Context.cpp#L299-L349 (chrome/m156)
    fn async_rescale_and_read_impl(
        &mut self,
        params: AsyncRescaleParams,
        rescale_gamma: RescaleGamma,
        rescale_mode: RescaleMode,
        read: ReadKind,
    ) {
        if !params.validate() || !params.src_image.as_base().is_graphite_backed() {
            params.fail();
            return;
        }

        if params.src_rect.size() == params.dst_image_info.dimensions() {
            // No need to rescale so do a direct readback
            self.read_image(None, params, read);
            return;
        }

        // Make a recorder to collect the rescale drawing commands and the copy commands
        let recorder = self.make_internal_recorder();
        let Some(scaled_image) = rescale_image(
            &recorder,
            &params.src_image,
            params.src_rect,
            &params.dst_image_info,
            rescale_gamma,
            rescale_mode,
        ) else {
            skia_log_w!("AsyncRead failed because rescaling failed");
            params.fail();
            return;
        };
        let bounds = IRect::from_size(params.dst_image_info.dimensions());
        self.read_image(
            Some(recorder),
            params.with_new_source(scaled_image, bounds),
            read,
        );
    }

    // Calls the read of `read` on the image of `params`, with `recorder` if one was made.
    fn read_image(
        &mut self,
        recorder: Option<Recorder>,
        params: AsyncRescaleParams,
        read: ReadKind,
    ) {
        let AsyncRescaleParams {
            src_image,
            src_rect,
            dst_image_info,
            callback,
        } = params;
        match read {
            ReadKind::Pixels => {
                let src_color_info = src_image.image_info().color_info().clone();
                let view = as_view(Some(&src_image));
                self.async_read_pixels(
                    recorder,
                    AsyncReadParams {
                        src: view,
                        src_color_info,
                        src_rect,
                        dst_image_info,
                        callback,
                    },
                );
            }
            ReadKind::Yuv420(yuv_color_space) => self.async_read_pixels_yuv420(
                recorder,
                &src_image,
                src_rect,
                &dst_image_info,
                yuv_color_space,
                callback,
            ),
        }
    }

    /// `asyncRescaleAndReadPixelsYUV420(src, yuvColorSpace, dstColorSpace, srcRect, dstSize,
    /// rescaleGamma, rescaleMode, callback)` for an image: `callback` is called with the Y, U and V
    /// planes of `src_rect`, rescaled to `dst_size` and converted with `yuv_color_space`.
    // Port of: src/gpu/graphite/Context.cpp#L522-L540 (chrome/m156)
    #[doc(alias = "asyncRescaleAndReadPixelsYUV420")]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    pub fn async_rescale_and_read_pixels_yuv420(
        &mut self,
        src: &CoreImage,
        yuv_color_space: YUVColorSpace,
        dst_color_space: Option<ColorSpace>,
        src_rect: IRect,
        dst_size: ISize,
        rescale_gamma: RescaleGamma,
        rescale_mode: RescaleMode,
        callback: ReadPixelsCallback,
    ) {
        // Use kOpaque alpha type to signal that we don't read back the alpha channel.
        let dst_image_info = ImageInfo::new(
            dst_size,
            ColorType::RGBA8888,
            AlphaType::Opaque,
            dst_color_space,
        );
        self.async_rescale_and_read_impl(
            AsyncRescaleParams {
                src_image: src.clone(),
                src_rect,
                dst_image_info,
                callback,
            },
            rescale_gamma,
            rescale_mode,
            ReadKind::Yuv420(yuv_color_space),
        );
    }

    /// `asyncRescaleAndReadPixelsYUV420(src, ...)` for a surface: the surface's image is read.
    // Port of: src/gpu/graphite/Context.cpp#L542-L565 (chrome/m156)
    #[doc(alias = "asyncRescaleAndReadPixelsYUV420")]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    pub fn async_rescale_and_read_pixels_yuv420_surface(
        &mut self,
        src: &Surface,
        yuv_color_space: YUVColorSpace,
        dst_color_space: Option<ColorSpace>,
        src_rect: IRect,
        dst_size: ISize,
        rescale_gamma: RescaleGamma,
        rescale_mode: RescaleMode,
        callback: ReadPixelsCallback,
    ) {
        let surface_image = src.as_image();
        self.async_rescale_and_read_pixels_yuv420(
            &surface_image,
            yuv_color_space,
            dst_color_space,
            src_rect,
            dst_size,
            rescale_gamma,
            rescale_mode,
            callback,
        );
    }

    /// `asyncRescaleAndReadPixelsYUVA420(src, ...)` for an image: as the YUV420 read, with the
    /// alpha plane (the destination is premultiplied).
    // Port of: src/gpu/graphite/Context.cpp#L567-L584 (chrome/m156)
    #[doc(alias = "asyncRescaleAndReadPixelsYUVA420")]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    pub fn async_rescale_and_read_pixels_yuva420(
        &mut self,
        src: &CoreImage,
        yuv_color_space: YUVColorSpace,
        dst_color_space: Option<ColorSpace>,
        src_rect: IRect,
        dst_size: ISize,
        rescale_gamma: RescaleGamma,
        rescale_mode: RescaleMode,
        callback: ReadPixelsCallback,
    ) {
        let dst_image_info = ImageInfo::new(
            dst_size,
            ColorType::RGBA8888,
            AlphaType::Premul,
            dst_color_space,
        );
        self.async_rescale_and_read_impl(
            AsyncRescaleParams {
                src_image: src.clone(),
                src_rect,
                dst_image_info,
                callback,
            },
            rescale_gamma,
            rescale_mode,
            ReadKind::Yuv420(yuv_color_space),
        );
    }

    /// `asyncRescaleAndReadPixelsYUVA420(src, ...)` for a surface: the surface's image is read.
    // Port of: src/gpu/graphite/Context.cpp#L586-L605 (chrome/m156)
    #[doc(alias = "asyncRescaleAndReadPixelsYUVA420")]
    #[allow(clippy::too_many_arguments)] // mirrors the C++ signature
    pub fn async_rescale_and_read_pixels_yuva420_surface(
        &mut self,
        src: &Surface,
        yuv_color_space: YUVColorSpace,
        dst_color_space: Option<ColorSpace>,
        src_rect: IRect,
        dst_size: ISize,
        rescale_gamma: RescaleGamma,
        rescale_mode: RescaleMode,
        callback: ReadPixelsCallback,
    ) {
        let surface_image = src.as_image();
        self.async_rescale_and_read_pixels_yuva420(
            &surface_image,
            yuv_color_space,
            dst_color_space,
            src_rect,
            dst_size,
            rescale_gamma,
            rescale_mode,
            callback,
        );
    }

    /// `copyPlane` of `asyncReadPixelsYUV420`: renders `src_image` (transformed by `tex_matrix`)
    /// into a scratch surface of `plane_info`, applying `rgb2yuv` as a color matrix when given, and
    /// transfers the surface's pixels to a buffer. `None` if the plane could not be made.
    // Port of: src/gpu/graphite/Context.cpp#L618-L657 (chrome/m156), `copyPlane`
    fn copy_plane(
        &mut self,
        recorder: &mut Recorder,
        src_image: &CoreImage,
        plane_info: &ImageInfo,
        label: &str,
        rgb2yuv: Option<&[f32; 20]>,
        tex_matrix: &Matrix,
    ) -> Option<PixelTransferResult> {
        let dst_surface = Surface::make_scratch(
            recorder,
            plane_info,
            label,
            Budgeted::Yes,
            Mipmapped::No,
            BackingFit::Approx,
            /*allow_unpremul=*/ false,
        )?;

        // Render the plane defined by rgb2yuv from srcImage into dstSurface.
        let sampling = SamplingOptions::new(FilterMode::Linear, MipmapMode::None);
        let img_shader = src_image.to_shader(
            Some((TileMode::Clamp, TileMode::Clamp)),
            sampling,
            Some(tex_matrix),
        );
        let mut paint = Paint::default();
        paint.set_shader(img_shader);
        paint.set_blend_mode(BlendMode::Src);
        if let Some(rgb2yuv) = rgb2yuv {
            // NOTE: The dstSurface's color space is set to the requested RGB dstColorSpace, so the
            // rendered image is automatically converted to that RGB color space before the
            // RGB->YUV color filter is evaluated, putting the plane data into the alpha channel.
            paint.set_color_filter(color_filters::matrix_row_major(rgb2yuv, Clamp::Yes));
        }
        dst_surface.canvas().draw_paint(&paint);

        // Manually flush the surface before transferPixels() is called to ensure the rendering
        // operations run before the CopyTextureToBuffer task.
        dst_surface.flush();

        let format = dst_surface.target().proxy()?.format();
        let identity_xfer = TextureFormatXferFn::make_identity(format)?;

        // Must use planeInfo.bounds() for srcRect since dstSurface is kApprox-fit.
        let result = self.transfer_pixels(
            Some(recorder),
            dst_surface.target(),
            plane_info.color_info(),
            plane_info.bounds(),
            &identity_xfer,
        );
        result.transfer_buffer.is_some().then_some(result)
    }

    /// `asyncReadPixelsYUV420(recorder, params, yuvColorSpace)`: the Y, U and V planes (and the
    /// alpha plane when `params.dst` is not opaque) are each drawn into a scratch surface and
    /// transferred, then the transfers are finalized into `callback`.
    // Port of: src/gpu/graphite/Context.cpp#L607-L712 (chrome/m156)
    #[doc(alias = "asyncReadPixelsYUV420")]
    fn async_read_pixels_yuv420(
        &mut self,
        recorder: Option<Recorder>,
        src_image: &CoreImage,
        src_rect: IRect,
        dst_image_info: &ImageInfo,
        yuv_color_space: YUVColorSpace,
        callback: ReadPixelsCallback,
    ) {
        // The planes are always extracted via drawing, so create the Recorder if there isn't one
        // yet.
        let mut recorder = recorder.unwrap_or_else(|| self.make_internal_recorder());

        let read_alpha = dst_image_info.alpha_type() != AlphaType::Opaque;
        let ya_info = dst_image_info
            .with_color_type(ColorType::Alpha8)
            .with_alpha_type(AlphaType::Premul);
        let uv_info =
            ya_info.with_dimensions(ISize::new(ya_info.width() / 2, ya_info.height() / 2));

        let base_m = color_matrix_rgb2yuv(yuv_color_space);
        // mirrors SkMatrix::Translate(-fLeft, -fTop) of the integer rect
        #[allow(clippy::cast_precision_loss)]
        let mut tex_matrix = Matrix::translate((-src_rect.left() as f32, -src_rect.top() as f32));

        // This matrix generates (r,g,b,a) = (0, 0, 0, y).
        let mut y_m = [0.0f32; 20];
        y_m[15..20].copy_from_slice(&base_m[0..5]);
        let Some(y) = self.copy_plane(
            &mut recorder,
            src_image,
            &ya_info,
            "AsyncReadPixelsYPlane",
            Some(&y_m),
            &tex_matrix,
        ) else {
            callback(None);
            return;
        };

        // No matrix, straight copy of alpha channel.
        let a = if read_alpha {
            let Some(a) = self.copy_plane(
                &mut recorder,
                src_image,
                &ya_info,
                "AsyncReadPixelsAPlane",
                None,
                &tex_matrix,
            ) else {
                callback(None);
                return;
            };
            Some(a)
        } else {
            None
        };

        // The UV planes are at half resolution compared to Y and A in 4:2:0.
        tex_matrix.post_scale((0.5, 0.5), None);

        // This matrix generates (r,g,b,a) = (0, 0, 0, u).
        let mut u_m = [0.0f32; 20];
        u_m[15..20].copy_from_slice(&base_m[5..10]);
        let Some(u) = self.copy_plane(
            &mut recorder,
            src_image,
            &uv_info,
            "AsyncReadPixelsUPlane",
            Some(&u_m),
            &tex_matrix,
        ) else {
            callback(None);
            return;
        };

        // This matrix generates (r,g,b,a) = (0, 0, 0, v).
        let mut v_m = [0.0f32; 20];
        v_m[15..20].copy_from_slice(&base_m[10..15]);
        let Some(v) = self.copy_plane(
            &mut recorder,
            src_image,
            &uv_info,
            "AsyncReadPixelsVPlane",
            Some(&v_m),
            &tex_matrix,
        ) else {
            callback(None);
            return;
        };

        // The planes in the order of the AsyncReadResult: Y, U, V and then A.
        let mut transfers = vec![y, u, v];
        transfers.extend(a);
        self.finalize_async_read_pixels(Some(recorder), transfers, callback);
    }
}
