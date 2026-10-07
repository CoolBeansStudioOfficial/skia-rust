// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! The drawing seam GMs are written against: a **stub** of the raster `Canvas`/`Surface` until
//! task D6 ports `SkCanvas` and `SkSurface_Raster` (design §5, row A7: "Surface stub until D6").
//!
//! The types here have `skia-safe`'s names, paths-relative-to-the-crate and signatures
//! (`Canvas::draw_color(&self, impl Into<Color4f>, impl Into<Option<BlendMode>>) -> &Self`, …),
//! so a GM port written against them compiles unchanged against the real `Canvas`. When D6 lands,
//! this module becomes `pub use skia_rust_core::{canvas::Canvas, surface::Surface, …};` and
//! nothing else in the GM crate changes.
//!
//! The stub implements only what the harness itself needs (DM's background clear,
//! `SkAutoCanvasRestore`): `save`/`restore`/`restore_to_count`, and `draw_color`/`clear` with
//! [`BlendMode::Src`] (or an opaque color with [`BlendMode::SrcOver`], which is the same draw).
//! Those write the pixels with `Pixmap::erase_4f`, which matches `drawPaint` in `kSrc` mode for
//! every color the background uses; any other draw panics with a "needs D6" message. A stub
//! result is still compared against the goldens, so it can never produce a false pass.

use std::cell::{Cell, RefCell};

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::Color4f;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::size::ISize;

/// Port of `SkBlendMode` (names and discriminants only; D2 ports the blend modes themselves and
/// replaces this enum).
// Port of: include/core/SkBlendMode.h#L38-L74 (chrome/m156)
#[doc(alias = "SkBlendMode")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(i32)]
pub enum BlendMode {
    Clear,
    Src,
    Dst,
    #[default]
    SrcOver,
    DstOver,
    SrcIn,
    DstIn,
    SrcOut,
    DstOut,
    SrcATop,
    DstATop,
    Xor,
    Plus,
    Modulate,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Multiply,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

/// Port of `SkPixelGeometry`.
// Port of: include/core/SkSurfaceProps.h#L20-L26 (chrome/m156)
#[doc(alias = "SkPixelGeometry")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(i32)]
pub enum PixelGeometry {
    #[default]
    Unknown,
    RGBH,
    BGRH,
    RGBV,
    BGRV,
}

/// Port of `SkSurfaceProps` (the fields only; text rendering is what reads them).
#[doc(alias = "SkSurfaceProps")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct SurfaceProps {
    flags: u32,
    pixel_geometry: PixelGeometry,
}

impl SurfaceProps {
    /// `SkSurfaceProps(flags, pixelGeometry)`.
    #[must_use]
    pub const fn new(flags: u32, pixel_geometry: PixelGeometry) -> Self {
        Self {
            flags,
            pixel_geometry,
        }
    }

    /// `SkSurfaceProps::flags()`.
    #[must_use]
    pub const fn flags(&self) -> u32 {
        self.flags
    }

    /// `SkSurfaceProps::pixelGeometry()`.
    #[doc(alias = "pixelGeometry")]
    #[must_use]
    pub const fn pixel_geometry(&self) -> PixelGeometry {
        self.pixel_geometry
    }

    /// Replaces the flags (GMs' `modifySurfaceProps` overrides assign a new `SkSurfaceProps`).
    pub fn set_flags(&mut self, flags: u32) {
        self.flags = flags;
    }

    /// Replaces the pixel geometry.
    pub fn set_pixel_geometry(&mut self, pixel_geometry: PixelGeometry) {
        self.pixel_geometry = pixel_geometry;
    }
}

/// Stub of `SkCanvas` drawing into a raster [`Bitmap`] (see the module docs for what it can do).
#[doc(alias = "SkCanvas")]
#[derive(Debug)]
pub struct Canvas {
    /// The bitmap the surface wraps, moved in for the surface's lifetime (draws take `&self`, as
    /// in skia-safe; each draw borrows the cell only for its own duration).
    bitmap: RefCell<Bitmap>,
    props: SurfaceProps,
    /// `SkCanvas::fSaveCount` (starts at 1).
    save_count: Cell<usize>,
}

impl Canvas {
    fn new(bitmap: Bitmap, props: SurfaceProps) -> Self {
        Self {
            bitmap: RefCell::new(bitmap),
            props,
            save_count: Cell::new(1),
        }
    }

    /// `SkCanvas::imageInfo()`.
    #[doc(alias = "imageInfo")]
    #[must_use]
    pub fn image_info(&self) -> ImageInfo {
        self.bitmap.borrow().info().clone()
    }

    /// `SkCanvas::getBaseLayerSize()`.
    #[doc(alias = "getBaseLayerSize")]
    #[must_use]
    pub fn base_layer_size(&self) -> ISize {
        self.bitmap.borrow().dimensions()
    }

    /// `SkCanvas::getBaseProps()`.
    #[doc(alias = "getBaseProps")]
    #[must_use]
    pub fn base_props(&self) -> SurfaceProps {
        self.props
    }

    /// `SkCanvas::getSaveCount()`.
    #[doc(alias = "getSaveCount")]
    #[must_use]
    pub fn save_count(&self) -> usize {
        self.save_count.get()
    }

    /// `SkCanvas::save()`: returns the save count before the call.
    // Port of: src/core/SkCanvas.cpp#L447-L451 (chrome/m156)
    pub fn save(&self) -> usize {
        self.save_count.set(self.save_count.get() + 1);
        self.save_count.get() - 1
    }

    /// `SkCanvas::restore()`: does nothing when only the initial state is left.
    // Port of: src/core/SkCanvas.cpp#L461-L476 (chrome/m156)
    pub fn restore(&self) -> &Self {
        if self.save_count.get() > 1 {
            self.save_count.set(self.save_count.get() - 1);
        }
        self
    }

    /// `SkCanvas::restoreToCount()`.
    // Port of: src/core/SkCanvas.cpp#L478-L488 (chrome/m156)
    #[doc(alias = "restoreToCount")]
    pub fn restore_to_count(&self, count: usize) -> &Self {
        let count = count.max(1);
        let n = self.save_count().saturating_sub(count);
        for _ in 0..n {
            self.restore();
        }
        self
    }

    /// `SkCanvas::drawColor(color, mode)` (default mode [`BlendMode::SrcOver`]).
    ///
    /// # Panics
    /// Stub limitation: any mode other than `Src` (or `SrcOver` with an opaque color) needs the
    /// real canvas (task D6).
    #[doc(alias = "drawColor")]
    pub fn draw_color(
        &self,
        color: impl Into<Color4f>,
        mode: impl Into<Option<BlendMode>>,
    ) -> &Self {
        let color = color.into();
        let mode = mode.into().unwrap_or(BlendMode::SrcOver);
        let replaces = mode == BlendMode::Src || (mode == BlendMode::SrcOver && color.a >= 1.0);
        assert!(
            replaces,
            "Surface stub: draw_color with {mode:?} and alpha {} needs the real Canvas (task D6)",
            color.a
        );
        self.bitmap.borrow_mut().erase_color_4f(color);
        self
    }

    /// `SkCanvas::clear(color)`: `drawColor(color, kSrc)`.
    // Port of: include/core/SkCanvas.h#L1263-L1265 (chrome/m156)
    pub fn clear(&self, color: impl Into<Color4f>) -> &Self {
        self.draw_color(color, BlendMode::Src)
    }
}

/// Stub of a raster `SkSurface` wrapping caller-owned pixels (`SkSurfaces::WrapPixels`).
///
/// Borrows the caller's bitmap mutably for its lifetime, like skia-safe's
/// `surfaces::wrap_pixels(…) -> Borrows<'pixels, Surface>` (pixel writes need exclusive access,
/// `docs/design/pixels.md`): the bitmap is moved into the canvas and moved back when the surface
/// drops, so the caller sees every draw afterwards.
#[doc(alias = "SkSurface")]
#[derive(Debug)]
pub struct Surface<'a> {
    canvas: Canvas,
    target: &'a mut Bitmap,
}

impl<'a> Surface<'a> {
    /// `SkSurfaces::WrapPixels(bitmap.pixmap(), props)`: draws go straight into `bitmap`'s
    /// pixels. `None` if the bitmap has no pixels.
    #[doc(alias = "WrapPixels")]
    #[must_use]
    pub fn wrap_pixels(bitmap: &'a mut Bitmap, props: Option<&SurfaceProps>) -> Option<Self> {
        if bitmap.is_null() || bitmap.draws_nothing() {
            return None;
        }
        let pixels = std::mem::take(bitmap);
        Some(Surface {
            canvas: Canvas::new(pixels, props.copied().unwrap_or_default()),
            target: bitmap,
        })
    }

    /// `SkSurface::getCanvas()`.
    #[doc(alias = "getCanvas")]
    pub fn canvas(&mut self) -> &Canvas {
        &self.canvas
    }
}

impl Drop for Surface<'_> {
    fn drop(&mut self) {
        // Hand the drawn-into bitmap back to its owner.
        *self.target = std::mem::take(self.canvas.bitmap.get_mut());
    }
}
