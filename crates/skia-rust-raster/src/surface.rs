// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkSurface.h, src/image/SkSurface.cpp, src/image/SkSurface_Base.{h,cpp},
// src/image/SkSurface_Raster.{h,cpp}

//! `SkSurface` (raster only): pixels you draw into with a [`Canvas`].
//!
//! The factories are in [`surfaces`](crate::surfaces) (`skia-safe` has the same split). A surface
//! owns a [`Canvas`] whose root device is a [`BitmapDevice`]; the canvas notifies the surface
//! before every draw so that the generation ID changes.
//!
//! skia-rust deviations (see the "As implemented in D6" design note):
//! * `SkSurface_Raster` shares its pixel ref with the bitmap you wrapped. A Rust surface that
//!   wraps caller pixels takes them for its lifetime (`docs/design/pixels.md`):
//!   [`Surface::wrap_pixels`] moves a `&mut Bitmap` in and puts it back when the surface drops;
//!   [`surfaces::wrap_pixels`](crate::surfaces::wrap_pixels) copies a `&mut [u8]` in and out.
//! * `makeImageSnapshot` shares the surface's pixel ref with the image (a copy if the surface
//!   wraps caller pixels, as Skia); the surface's next draw detaches onto a copy
//!   (`docs/design/pixels.md`), which replaces `onCopyOnWrite` and
//!   `onRestoreBackingMutability`. The image is cached until then (`fCachedImage`).
//! * `getCanvas` is `&mut self` and the canvas is created with the surface.
//! * GPU surfaces, async readback, `characterize`, `wait`, capabilities and recorders are out of
//!   scope.

use std::rc::Rc;

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{Canvas, ContentChangeMode, PeekedPixels, SurfaceBase};
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::image_raster::CopyPixelsMode;
use skia_rust_core::images;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::{Contains, IRect};
use skia_rust_core::surface_props::SurfaceProps;

use crate::bitmap_device::BitmapDevice;

/// Where the pixels of a wrapping surface go back to when it drops.
#[derive(Debug)]
enum Target<'a> {
    /// A caller-owned bitmap that was moved into the surface.
    Bitmap(&'a mut Bitmap),
    /// A caller-owned byte slice that was copied into the surface.
    Slice(&'a mut [u8]),
}

/// A raster surface (`SkSurface`).
// Port of: include/core/SkSurface.h#L60-L80 (chrome/m156)
#[doc(alias = "SkSurface")]
#[derive(Debug)]
pub struct Surface<'a> {
    base: Rc<SurfaceBase>,
    canvas: Canvas,
    info: ImageInfo,
    props: SurfaceProps,
    target: Option<Target<'a>>,
}

/// `kMaxTotalSize` (`SK_MaxS32`).
const MAX_TOTAL_SIZE: u64 = 0x7FFF_FFFF;

/// `SkSurfaceValidateRasterInfo`: whether a raster surface can have this info and row bytes
/// (`rowBytes == 0` means "any").
// Port of: src/image/SkSurface_Raster.cpp#L26-L45 (chrome/m156)
#[doc(alias = "SkSurfaceValidateRasterInfo")]
#[must_use]
pub fn surface_validate_raster_info(info: &ImageInfo, row_bytes: usize) -> bool {
    if !skia_rust_core::image_info_priv::image_info_is_valid(info) {
        return false;
    }

    if row_bytes == 0 {
        return true; // kIgnoreRowBytesValue
    }

    if !info.valid_row_bytes(row_bytes) {
        return false;
    }

    let size = u64::try_from(info.height()).unwrap_or(0) * row_bytes as u64;
    size <= MAX_TOTAL_SIZE
}

impl<'a> Surface<'a> {
    /// Makes the surface over `bitmap`, with its canvas (`SkSurface_Raster` constructor).
    fn from_bitmap(bitmap: Bitmap, props: SurfaceProps, target: Option<Target<'a>>) -> Surface<'a> {
        let info = bitmap.info().clone();
        let device = BitmapDevice::with_props(bitmap, props);
        let canvas = Canvas::from_device(Box::new(device));
        let base = Rc::new(SurfaceBase::new());
        canvas.set_surface_base(Some(Rc::clone(&base)));
        Surface {
            base,
            canvas,
            info,
            props,
            target,
        }
    }

    /// A surface with newly allocated, zeroed pixels (`SkSurfaces::Raster`); `row_bytes` of `None`
    /// or zero means the minimum.
    // Port of: src/image/SkSurface_Raster.cpp#L200-L214 (chrome/m156)
    pub(crate) fn new_raster(
        info: &ImageInfo,
        row_bytes: Option<usize>,
        props: Option<&SurfaceProps>,
    ) -> Option<Surface<'static>> {
        if !surface_validate_raster_info(info, 0) {
            return None;
        }
        let mut bitmap = Bitmap::new();
        if !bitmap.try_alloc_pixels_info(info, row_bytes.filter(|&r| r != 0)) {
            return None;
        }
        Some(Surface::from_bitmap(
            bitmap,
            props.copied().unwrap_or_default(),
            None,
        ))
    }

    /// A surface that draws into `bitmap`'s pixels (`SkSurfaces::WrapPixels(bitmap.pixmap())`).
    /// The bitmap is moved into the surface and put back when it drops, so the caller sees
    /// every draw afterwards. `None` if the bitmap has no pixels or is not valid for a raster
    /// surface.
    // Port of: src/image/SkSurface_Raster.cpp#L152-L176 (chrome/m156)
    #[doc(alias = "WrapPixels")]
    #[must_use]
    pub fn wrap_pixels(
        bitmap: &'a mut Bitmap,
        props: Option<&SurfaceProps>,
    ) -> Option<Surface<'a>> {
        if bitmap.is_null() || bitmap.draws_nothing() {
            return None;
        }
        if !surface_validate_raster_info(bitmap.info(), bitmap.row_bytes()) {
            return None;
        }
        let pixels = std::mem::take(bitmap);
        Some(Surface::from_bitmap(
            pixels,
            props.copied().unwrap_or_default(),
            Some(Target::Bitmap(bitmap)),
        ))
    }

    /// A surface over a copy of `pixels`, copied back when the surface drops
    /// (`SkSurfaces::WrapPixels(info, pixels, rowBytes)`). `None` if the info, row bytes or
    /// slice length do not fit.
    // Port of: src/image/SkSurface_Raster.cpp#L152-L176 (chrome/m156)
    pub(crate) fn wrap_slice(
        info: &ImageInfo,
        pixels: &'a mut [u8],
        row_bytes: usize,
        props: Option<&SurfaceProps>,
    ) -> Option<Surface<'a>> {
        if !surface_validate_raster_info(info, row_bytes) {
            return None;
        }
        let mut bitmap = Bitmap::new();
        if !bitmap.install_pixels(info, pixels.to_vec(), row_bytes) {
            return None;
        }
        Some(Surface::from_bitmap(
            bitmap,
            props.copied().unwrap_or_default(),
            Some(Target::Slice(pixels)),
        ))
    }

    /// The width in pixels (`width`).
    #[must_use]
    pub fn width(&self) -> i32 {
        self.info.width()
    }

    /// The height in pixels (`height`).
    #[must_use]
    pub fn height(&self) -> i32 {
        self.info.height()
    }

    /// The image info (`imageInfo`).
    #[doc(alias = "imageInfo")]
    #[must_use]
    pub fn image_info(&self) -> ImageInfo {
        self.info.clone()
    }

    /// The surface properties (`props`).
    #[must_use]
    pub fn props(&self) -> &SurfaceProps {
        &self.props
    }

    /// The generation ID: a value that changes whenever the pixels may have changed
    /// (`generationID`).
    #[doc(alias = "generationID")]
    #[must_use]
    pub fn generation_id(&self) -> u32 {
        self.base.generation_id()
    }

    /// Tells the surface its contents are about to change by something other than its canvas
    /// (`notifyContentWillChange`).
    #[doc(alias = "notifyContentWillChange")]
    pub fn notify_content_will_change(&mut self, mode: ContentChangeMode) {
        self.base.notify_content_will_change(mode);
    }

    /// The canvas that draws into the surface (`getCanvas`).
    #[doc(alias = "getCanvas")]
    pub fn canvas(&mut self) -> &Canvas {
        &self.canvas
    }

    /// A new surface compatible with this one (`makeSurface(info)`); a raster surface with the
    /// same props.
    // Port of: src/image/SkSurface_Raster.cpp#L96-L99 (chrome/m156)
    #[doc(alias = "makeSurface")]
    #[must_use]
    pub fn new_surface(&self, info: &ImageInfo) -> Option<Surface<'static>> {
        Surface::new_raster(info, None, Some(&self.props))
    }

    /// A new surface with the dimensions `(width, height)` (`makeSurface(width, height)`).
    #[doc(alias = "makeSurface")]
    #[must_use]
    pub fn new_surface_with_dimensions(&self, dim: (i32, i32)) -> Option<Surface<'static>> {
        self.new_surface(&self.info.with_dimensions(dim))
    }

    /// An image of the surface's current pixels, which do not change when the surface is drawn
    /// to later (`makeImageSnapshot`). The same image is returned until the next draw
    /// (`refCachedImage`). `None` if the surface has no pixels.
    // Port of: src/image/SkSurface.cpp#L90-L92 (chrome/m156)
    #[doc(alias = "makeImageSnapshot")]
    pub fn image_snapshot(&mut self) -> Option<Image> {
        if let Some(cached) = self.base.cached_image() {
            return Some(cached);
        }
        let cached = self.new_image_snapshot(None)?;
        self.base.set_cached_image(cached.clone());
        Some(cached)
    }

    /// An image of the `bounds` of the surface's current pixels (`makeImageSnapshot(bounds)`);
    /// `None` if `bounds` does not intersect the surface.
    // Port of: src/image/SkSurface.cpp#L94-L106 (chrome/m156)
    #[doc(alias = "makeImageSnapshot")]
    pub fn image_snapshot_with_bounds(&mut self, bounds: impl AsRef<IRect>) -> Option<Image> {
        let surf_bounds = IRect::from_wh(self.width(), self.height());
        let bounds = IRect::intersect(bounds.as_ref(), &surf_bounds)?;
        debug_assert!(!bounds.is_empty());
        if bounds == surf_bounds {
            self.image_snapshot()
        } else {
            self.new_image_snapshot(Some(&bounds))
        }
    }

    /// `SkSurface_Raster::onNewImageSnapshot`.
    // Port of: src/image/SkSurface_Raster.cpp#L112-L136 (chrome/m156)
    fn new_image_snapshot(&mut self, subset: Option<&IRect>) -> Option<Image> {
        let we_own_the_pixels = self.target.is_none();
        self.canvas.with_root_bitmap(|bitmap| {
            if let Some(subset) = subset {
                debug_assert!(IRect::from_wh(bitmap.width(), bitmap.height()).contains(subset));
                let mut dst = Bitmap::new();
                if !dst.try_alloc_pixels_info(&bitmap.info().with_dimensions(subset.size()), None) {
                    return None;
                }
                let read = dst.peek_pixels_mut().is_some_and(|mut pm| {
                    bitmap.read_pixels_to_pixmap(&mut pm, (subset.left, subset.top))
                });
                debug_assert!(read);
                dst.set_immutable(); // key, so MakeFromBitmap doesn't make a copy of the buffer
                return dst.as_image();
            }

            // The snapshot shares the pixel ref (a clone of the handle); the next write to the
            // surface's bitmap copies it first. Pixels we do not own are always copied.
            let cpm = if we_own_the_pixels {
                CopyPixelsMode::Never
            } else {
                CopyPixelsMode::Always
            };
            images::make_image_from_raster_bitmap(bitmap, cpm)
        })?
    }

    /// The pixels, if the surface has any (`peekPixels`).
    #[doc(alias = "peekPixels")]
    #[must_use]
    pub fn peek_pixels(&mut self) -> Option<PeekedPixels<'_>> {
        self.canvas.peek_pixels()
    }

    /// Copies pixels from the surface into `dst_pixels` (`readPixels`).
    #[doc(alias = "readPixels")]
    pub fn read_pixels(
        &mut self,
        dst_info: &ImageInfo,
        dst_pixels: &mut [u8],
        dst_row_bytes: usize,
        src: impl Into<IPoint>,
    ) -> bool {
        self.canvas
            .read_pixels(dst_info, dst_pixels, dst_row_bytes, src)
    }

    /// [`read_pixels`](Self::read_pixels) into a pixmap.
    #[doc(alias = "readPixels")]
    pub fn read_pixels_to_pixmap(&mut self, dst: &mut Pixmap<'_>, src: impl Into<IPoint>) -> bool {
        self.canvas.read_pixels_to_pixmap(dst, src)
    }

    /// [`read_pixels`](Self::read_pixels) into a bitmap.
    #[doc(alias = "readPixels")]
    pub fn read_pixels_to_bitmap(&mut self, dst: &mut Bitmap, src: impl Into<IPoint>) -> bool {
        self.canvas.read_pixels_to_bitmap(dst, src)
    }

    /// Copies pixels from `src` into the surface at `dst` (`writePixels`).
    // Port of: src/image/SkSurface.cpp#L193-L211 (chrome/m156)
    #[doc(alias = "writePixels")]
    pub fn write_pixels_from_pixmap(&mut self, src: &Pixmap<'_>, dst: impl Into<IPoint>) {
        if src.addr().is_none() || src.width() <= 0 || src.height() <= 0 {
            return;
        }

        let IPoint { x, y } = dst.into();
        let src_r = IRect::from_xywh(x, y, src.width(), src.height());
        let dst_r = IRect::from_wh(self.width(), self.height());
        if IRect::intersects(&src_r, &dst_r) {
            let mut mode = ContentChangeMode::Retain;
            if src_r.contains(&dst_r) {
                mode = ContentChangeMode::Discard;
            }
            if !self.base.about_to_draw(mode) {
                return;
            }
            // SkSurface_Raster::onWritePixels: fBitmap.writePixels(src, x, y)
            self.canvas.with_root_bitmap(|bm| {
                let _ = bm.write_pixels(src, x, y);
            });
        }
    }

    /// [`write_pixels_from_pixmap`](Self::write_pixels_from_pixmap) from a bitmap.
    #[doc(alias = "writePixels")]
    pub fn write_pixels_from_bitmap(&mut self, src: &Bitmap, dst: impl Into<IPoint>) {
        if let Some(pm) = src.peek_pixels() {
            self.write_pixels_from_pixmap(&pm, dst);
        }
    }
}

impl Drop for Surface<'_> {
    fn drop(&mut self) {
        // Hand the drawn-into pixels back to their owner.
        let Some(target) = self.target.take() else {
            return;
        };
        // The canvas's pending layers are discarded, as `~SkCanvas` does.
        self.canvas.abandon_layers();
        let Some(bm) = self.canvas.take_root_bitmap() else {
            return;
        };
        match target {
            Target::Bitmap(dst) => *dst = bm,
            Target::Slice(dst) => {
                if let Some(pm) = bm.peek_pixels()
                    && let Some(bytes) = pm.addr()
                {
                    let n = dst.len().min(bytes.len());
                    dst[..n].copy_from_slice(&bytes[..n]);
                }
            }
        }
    }
}
