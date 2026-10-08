// Copyright 2008 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkCanvas.h, src/core/SkCanvas.cpp (the raster constructors)

//! The `Canvas` constructors that make a raster device (`SkCanvas::MakeRasterDirect`,
//! `SkCanvas(const SkBitmap&)`). `skia_rust_core::canvas::Canvas` cannot name `BitmapDevice`, so
//! they are an extension trait with associated functions: with [`RasterCanvas`] in scope they are
//! called as in `skia-safe` (`Canvas::from_raster_direct(...)`).

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{Canvas, OwnedCanvas};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::surface_props::SurfaceProps;

use crate::bitmap_device::BitmapDevice;
use crate::surface::{Surface, surface_validate_raster_info};

/// The raster constructors of `Canvas`.
pub trait RasterCanvas {
    /// A canvas that draws into `pixels` (`SkCanvas::MakeRasterDirect`). The bytes are copied into
    /// the canvas and copied back when it drops (`docs/design/pixels.md`). `None` if the info,
    /// row bytes or the slice length are invalid.
    fn from_raster_direct<'pixels>(
        info: &ImageInfo,
        pixels: &'pixels mut [u8],
        row_bytes: impl Into<Option<usize>>,
        props: Option<&SurfaceProps>,
    ) -> Option<OwnedCanvas<'pixels>>;

    /// A canvas that draws into `bitmap` (`SkCanvas(const SkBitmap&, const SkSurfaceProps&)`). The
    /// bitmap is moved into the canvas and put back when it drops. `None` if the bitmap is not
    /// ready to draw.
    fn from_bitmap<'lt>(
        bitmap: &'lt mut Bitmap,
        props: Option<&SurfaceProps>,
    ) -> Option<OwnedCanvas<'lt>>;

    /// A new raster surface compatible with this canvas, with the canvas's surface properties if
    /// `props` is `None` (`SkCanvas::makeSurface`; `skia-safe`'s `new_surface`).
    ///
    /// skia-rust: a raster canvas's device makes raster surfaces (`SkBitmapDevice::makeSurface`).
    fn new_surface(
        &self,
        info: &ImageInfo,
        props: Option<&SurfaceProps>,
    ) -> Option<Surface<'static>>;
}

impl RasterCanvas for Canvas {
    // Port of: src/core/SkCanvas.cpp#L1047-L1052 and src/core/SkBitmapDevice.cpp#L615-L617
    // (chrome/m156)
    fn new_surface(
        &self,
        info: &ImageInfo,
        props: Option<&SurfaceProps>,
    ) -> Option<Surface<'static>> {
        let canvas_props = self.base_props();
        Surface::new_raster(info, None, Some(props.unwrap_or(&canvas_props)))
    }

    // Port of: src/core/SkCanvas.cpp#L2939-L2953 (chrome/m156)
    fn from_raster_direct<'pixels>(
        info: &ImageInfo,
        pixels: &'pixels mut [u8],
        row_bytes: impl Into<Option<usize>>,
        props: Option<&SurfaceProps>,
    ) -> Option<OwnedCanvas<'pixels>> {
        let row_bytes = row_bytes
            .into()
            .filter(|&r| r != 0)
            .unwrap_or_else(|| info.min_row_bytes());
        if !surface_validate_raster_info(info, row_bytes) {
            return None;
        }
        let mut bitmap = Bitmap::new();
        if !bitmap.install_pixels(info, pixels.to_vec(), row_bytes) {
            return None;
        }
        let device = BitmapDevice::with_props(bitmap, props.copied().unwrap_or_default());
        let canvas = Canvas::from_device(Box::new(device));
        Some(OwnedCanvas::new(
            canvas,
            Some(Box::new(move |bm: Bitmap| {
                if let Some(pm) = bm.peek_pixels()
                    && let Some(bytes) = pm.addr()
                {
                    let n = pixels.len().min(bytes.len());
                    pixels[..n].copy_from_slice(&bytes[..n]);
                }
            })),
        ))
    }

    // Port of: src/core/SkCanvas.cpp#L335-L345 (chrome/m156)
    fn from_bitmap<'lt>(
        bitmap: &'lt mut Bitmap,
        props: Option<&SurfaceProps>,
    ) -> Option<OwnedCanvas<'lt>> {
        if !bitmap.is_ready_to_draw() {
            return None;
        }
        let pixels = std::mem::take(bitmap);
        let device = BitmapDevice::with_props(pixels, props.copied().unwrap_or_default());
        let canvas = Canvas::from_device(Box::new(device));
        Some(OwnedCanvas::new(
            canvas,
            Some(Box::new(move |bm: Bitmap| *bitmap = bm)),
        ))
    }
}
