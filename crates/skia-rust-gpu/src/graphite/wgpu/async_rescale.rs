// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/Context.cpp (the asyncRescaleAndReadPixels overloads)

//! `Context::asyncRescaleAndReadPixels`: reads back an image or surface, rescaling it first when
//! the source rect and the destination size differ.

use skia_rust_core::image::{Image as CoreImage, RescaleGamma, RescaleMode};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::rect::{Contains, IRect};

use crate::gpu::sk_log::skia_log_w;
use crate::graphite::async_read::{AsyncReadParams, ReadPixelsCallback};
use crate::graphite::recorder::Recorder;
use crate::graphite::surface_graphite::Surface;
use crate::graphite::texture_utils::{as_view, rescale_image};
use crate::graphite::wgpu::context::WgpuContext;

/// What an asynchronous rescale-and-read produces from the (rescaled) source image.
#[derive(Clone, Copy, Debug)]
enum ReadKind {
    /// `asyncReadPixels`: the destination's pixels in its image info.
    Pixels,
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
        match read {
            ReadKind::Pixels => {
                let src_color_info = params.src_image.image_info().color_info().clone();
                let view = as_view(Some(&params.src_image));
                self.async_read_pixels(
                    recorder,
                    AsyncReadParams {
                        src: view,
                        src_color_info,
                        src_rect: params.src_rect,
                        dst_image_info: params.dst_image_info,
                        callback: params.callback,
                    },
                );
            }
        }
    }
}
