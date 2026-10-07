// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPixmap.h, src/core/SkPixmap.cpp

//! [`Pixmap`]: [`ImageInfo`] paired with pixels and row bytes.
//!
//! # Pixel memory
//! `SkPixmap` holds a `const void*` and casts the constness away for `erase()` and
//! `writable_addr()`. In Rust a [`Pixmap<'a>`] borrows its bytes for `'a`, either shared
//! (`&[u8]`: [`Pixmap::new_readonly`], [`Bitmap::peek_pixels`](crate::bitmap::Bitmap::peek_pixels))
//! or unique (`&mut [u8]`: [`Pixmap::new`],
//! [`Bitmap::peek_pixels_mut`](crate::bitmap::Bitmap::peek_pixels_mut)); the borrow checker keeps
//! writers exclusive (`docs/design/pixels.md`). Methods that write ([`Pixmap::erase`],
//! [`Pixmap::writable_addr`], the `set_addr*` family) need the unique kind; [`Pixmap::erase`]
//! returns `false` on a read-only pixmap, the others panic.
//!
//! Pixel addresses (`addr8(x, y)` & co.) cannot be references in safe Rust (a `&[u8]` is not
//! aligned for `u16`, `u32` or `u64`), so they return the pixel value (little-endian layout, as
//! Skia assumes) and the `set_addr*` methods write one.
//!
//! Not ported: `scalePixels` (it draws through an image shader, Phase 3) and
//! `reset(const SkMask&)`.

use crate::alpha_type::AlphaType;
use crate::color::{Color, Color4f, PMColor4f};
use crate::color_data::{
    get_packed_a4444, pixel16_to_color, pixel4444_to_pixel32, swizzle_bgra_to_pm_color, swizzle_rb,
    swizzle_rgba_to_pm_color,
};
use crate::color_priv::get_packed_a32;
use crate::color_space::ColorSpace;
use crate::color_type::ColorType;
use crate::convert_pixels::convert_pixels;
use crate::half::{HALF_1, half_to_float};
use crate::image_info::ImageInfo;
use crate::image_info_priv::{color_type_shift_per_pixel, image_info_valid_conversion};
use crate::point::IPoint;
use crate::read_pixels_rec::ReadPixelsRec;
use crate::rect::IRect;
use crate::size::ISize;
use crate::t_pin::t_pin;
use crate::un_pre_multiply::pm_color_to_color;
use skia_rust_simd::vx::Float4;
use std::fmt;

// How a [`Pixmap`] borrows its pixels.
enum Storage<'a> {
    // nullptr
    None,
    Shared(&'a [u8]),
    Unique(&'a mut [u8]),
}

/// Pairs [`ImageInfo`] with pixels and row bytes.
///
/// [`Pixmap`] is a low level class which provides convenience functions to access raster
/// destinations. `Canvas` can not draw [`Pixmap`], nor does [`Pixmap`] provide a direct drawing
/// destination.
///
/// Use [`Bitmap`](crate::bitmap::Bitmap) to draw pixels referenced by [`Pixmap`]; use `Surface`
/// to draw into pixels referenced by [`Pixmap`].
///
/// [`Pixmap`] does not try to manage the lifetime of the pixel memory. Use
/// [`PixelRef`](crate::pixel_ref::PixelRef) to manage pixel memory; it is safe across threads.
// Port of: include/core/SkPixmap.h#L38-L728 (chrome/m156)
#[doc(alias = "SkPixmap")]
pub struct Pixmap<'a> {
    storage: Storage<'a>,
    row_bytes: usize,
    info: ImageInfo,
}

impl Default for Pixmap<'_> {
    /// Creates an empty [`Pixmap`] without pixels, with [`ColorType::Unknown`], with
    /// [`AlphaType::Unknown`], and with a width and height of zero.
    // Port of: include/core/SkPixmap.h#L48-L50 (chrome/m156)
    fn default() -> Self {
        Self {
            storage: Storage::None,
            row_bytes: 0,
            info: ImageInfo::new_unknown(None),
        }
    }
}

impl fmt::Debug for Pixmap<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pixmap")
            .field("row_bytes", &self.row_bytes)
            .field("info", &self.info)
            .field("has_pixels", &self.bytes().is_some())
            .finish()
    }
}

// Port of: src/core/SkPixmap.cpp#L96-L99 etc.: the debug-build preconditions of the pixel
// accessors.
fn assert_in_bounds(info: &ImageInfo, x: i32, y: i32) {
    debug_assert!(x >= 0 && x < info.width(), "x={x}; width={}", info.width());
    debug_assert!(
        y >= 0 && y < info.height(),
        "y={y}; height={}",
        info.height()
    );
}

fn get_u16(bytes: &[u8], off: usize) -> u16 {
    u16::from_ne_bytes([bytes[off], bytes[off + 1]])
}

fn get_u32(bytes: &[u8], off: usize) -> u32 {
    u32::from_ne_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
}

fn get_u64(bytes: &[u8], off: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&bytes[off..off + 8]);
    u64::from_ne_bytes(b)
}

fn get_f32(bytes: &[u8], off: usize) -> f32 {
    f32::from_bits(get_u32(bytes, off))
}

// `SkColorSetARGB(a, r, g, b)` for U8CPU arguments that may be out of range.
fn color_set_argb(a: u32, r: u32, g: u32, b: u32) -> Color {
    debug_assert!(a <= 255 && r <= 255 && g <= 255 && b <= 255);
    Color::new((a << 24) | (r << 16) | (g << 8) | b)
}

// `SkColorSetRGB(r, g, b)`
fn color_set_rgb(r: u32, g: u32, b: u32) -> Color {
    color_set_argb(0xFF, r, g, b)
}

// `SkColorSetA(c, a)`
fn color_set_a(c: Color, a: u32) -> Color {
    debug_assert!(a <= 255);
    Color::new((u32::from(c) & 0x00FF_FFFF) | (a << 24))
}

// The implicit C++ conversion of a float to `U8CPU`/`uint8_t` (a truncation).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
// mirrors the implicit float -> unsigned conversion, which is UB when out of range
fn trunc_u32(f: f32) -> u32 {
    f as u32
}

// Port of: src/core/SkSwizzlePriv.h#L53-L60 (chrome/m156), `Sk4f_toL32(swizzle_rb(p4))`'s
// `Sk4f_toL32`.
fn sk4f_to_l32(px: [f32; 4]) -> u32 {
    // For the expected positive color values, the +0.5 before the pin and cast effectively
    // rounds to the nearest int without having to call round() or lrint().
    let bytes = (Float4::load(&px) * 255.0f32 + 0.5f32)
        .pin(0.0f32, 255.0f32)
        .cast::<u8>();
    let mut l32 = [0u8; 4];
    bytes.store(&mut l32);
    u32::from_le_bytes(l32)
}

// `swizzle_rb(p4)` on a float4.
fn swizzle_rb4([r, g, b, a]: [f32; 4]) -> [f32; 4] {
    [b, g, r, a]
}

// `srgb_to_linear` of getColor() and getColor4f().
fn srgb_to_linear(x: f32) -> f32 {
    if x <= 0.04045f32 {
        x * (1.0f32 / 12.92f32)
    } else {
        // skia-rust: libm (std::pow on floats is powf)
        (x * (1.0f32 / 1.055f32) + (0.055f32 / 1.055f32)).powf(2.4f32)
    }
}

impl<'a> Pixmap<'a> {
    /// Creates a [`Pixmap`] from `info` width, height, alpha type, and color type. `pixels` are
    /// the pixels, and `row_bytes` should be `info.width()` times `info.bytes_per_pixel()`, or
    /// larger.
    ///
    /// Returns `None` if `row_bytes` is smaller than `info.min_row_bytes()` or `pixels` is too
    /// small to hold the image (`SkPixmap` performs no such checks).
    ///
    /// The pixmap may later be modified by [`Self::reset()`] to drop the pixels.
    // Port of: include/core/SkPixmap.h#L65-L67 (chrome/m156)
    #[must_use]
    pub fn new(info: &ImageInfo, pixels: &'a mut [u8], row_bytes: usize) -> Option<Self> {
        if row_bytes < info.min_row_bytes() {
            return None;
        }
        if pixels.len() < info.compute_byte_size(row_bytes) {
            return None;
        }
        Some(Self {
            storage: Storage::Unique(pixels),
            row_bytes,
            info: info.clone(),
        })
    }

    /// Like [`Self::new`], over read-only pixels: [`Self::erase`] and the writable accessors are
    /// not available.
    ///
    /// skia-rust: `SkPixmap` holds a `const void*` and `const_cast`s it for writes.
    #[must_use]
    pub fn new_readonly(info: &ImageInfo, pixels: &'a [u8], row_bytes: usize) -> Option<Self> {
        if row_bytes < info.min_row_bytes() {
            return None;
        }
        if pixels.len() < info.compute_byte_size(row_bytes) {
            return None;
        }
        Some(Self {
            storage: Storage::Shared(pixels),
            row_bytes,
            info: info.clone(),
        })
    }

    // A pixmap with `info` and `row_bytes` but no pixels.
    pub(crate) fn without_pixels(info: ImageInfo, row_bytes: usize) -> Self {
        Self {
            storage: Storage::None,
            row_bytes,
            info,
        }
    }

    // A read-only pixmap over a bitmap's pixels (`Bitmap::peek_pixels`); no size checks (the
    // bitmap validated them).
    pub(crate) fn from_shared(info: ImageInfo, pixels: &'a [u8], row_bytes: usize) -> Self {
        Self {
            storage: Storage::Shared(pixels),
            row_bytes,
            info,
        }
    }

    // A writable pixmap over a bitmap's pixels (`Bitmap::peek_pixels_mut`).
    pub(crate) fn from_unique(info: ImageInfo, pixels: &'a mut [u8], row_bytes: usize) -> Self {
        Self {
            storage: Storage::Unique(pixels),
            row_bytes,
            info,
        }
    }

    /// Sets width, height, row bytes to zero; pixel address to `None`; color type to
    /// [`ColorType::Unknown`]; and alpha type to [`AlphaType::Unknown`].
    ///
    /// The prior pixels are unaffected; it is up to the caller to release pixels memory if
    /// desired.
    // Port of: src/core/SkPixmap.cpp#L31-L35 (chrome/m156)
    pub fn reset(&mut self) -> &mut Self {
        self.storage = Storage::None;
        self.row_bytes = 0;
        self.info = ImageInfo::new_unknown(None);
        self
    }

    /// Changes the [`ColorSpace`] in the [`ImageInfo`]; preserves width, height, alpha type, and
    /// color type, and leaves the pixel address and row bytes unchanged.
    // Port of: src/core/SkPixmap.cpp#L58-L61 (chrome/m156)
    #[doc(alias = "setColorSpace")]
    pub fn set_color_space(&mut self, color_space: impl Into<Option<ColorSpace>>) -> &mut Self {
        self.info = self.info.with_color_space(color_space);
        self
    }

    // The geometry of `extractSubset`: the intersection and the byte offset of its first pixel.
    // Port of: src/core/SkPixmap.cpp#L67-L86 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // the intersection is inside the bounds, so non-negative
    fn subset_geometry(&self, subset: &IRect) -> Option<(IRect, usize)> {
        let src_rect = IRect::from_wh(self.width(), self.height());
        let r = IRect::intersect(&src_rect, subset)?; // r is empty (i.e. no intersection)

        // If the upper left of the rectangle was outside the bounds of this pixmap, we should
        // have exited above.
        debug_assert!((r.left as u32) < (self.width() as u32));
        debug_assert!((r.top as u32) < (self.height() as u32));

        let bpp = self.info.bytes_per_pixel();
        let offset = (r.top as usize)
            .wrapping_mul(self.row_bytes)
            .wrapping_add((r.left as usize).wrapping_mul(bpp));
        Some((r, offset))
    }

    /// Returns the pixmap of the intersection of this pixmap with `area`, if the intersection is
    /// not empty (a read-only view of the same pixels). Otherwise returns `None`.
    ///
    /// - `area` bounds to intersect with the pixmap
    // Port of: src/core/SkPixmap.cpp#L67-L86 (chrome/m156)
    #[doc(alias = "extractSubset")]
    #[must_use]
    pub fn extract_subset(&self, area: impl AsRef<IRect>) -> Option<Pixmap<'_>> {
        let (r, offset) = self.subset_geometry(area.as_ref())?;
        let storage = match self.bytes() {
            Some(bytes) => Storage::Shared(&bytes[offset..]),
            None => Storage::None,
        };
        Some(Pixmap {
            storage,
            row_bytes: self.row_bytes,
            info: self.info.with_dimensions(r.size()),
        })
    }

    /// Like [`Self::extract_subset`], but the returned pixmap can write to the pixels if this
    /// one can.
    #[must_use]
    pub fn extract_subset_mut(&mut self, area: impl AsRef<IRect>) -> Option<Pixmap<'_>> {
        let (r, offset) = self.subset_geometry(area.as_ref())?;
        let row_bytes = self.row_bytes;
        let info = self.info.with_dimensions(r.size());
        let storage = match &mut self.storage {
            Storage::None => Storage::None,
            Storage::Shared(bytes) => Storage::Shared(&bytes[offset..]),
            Storage::Unique(bytes) => Storage::Unique(&mut bytes[offset..]),
        };
        Some(Pixmap {
            storage,
            row_bytes,
            info,
        })
    }

    /// Returns the width, height, alpha type, color type, and color space.
    // Port of: include/core/SkPixmap.h#L160 (chrome/m156)
    #[must_use]
    pub fn info(&self) -> &ImageInfo {
        &self.info
    }

    /// Returns the row bytes: the interval from one pixel row to the next. Row bytes is at least
    /// as large as `width() * info().bytes_per_pixel()`.
    ///
    /// Returns zero if the color type is [`ColorType::Unknown`]. It is up to the pixmap creator
    /// to ensure that row bytes is a useful value.
    // Port of: include/core/SkPixmap.h#L170 (chrome/m156)
    #[doc(alias = "rowBytes")]
    #[must_use]
    pub fn row_bytes(&self) -> usize {
        self.row_bytes
    }

    /// Returns the pixels from the first pixel to the end of the borrowed bytes, or `None` if
    /// there are no pixels (`SkPixmap::addr()` returning `nullptr`).
    // Port of: include/core/SkPixmap.h#L180 (chrome/m156)
    #[must_use]
    pub fn addr(&self) -> Option<&[u8]> {
        self.bytes()
    }

    /// The bytes of the pixels, if any.
    #[must_use]
    pub fn bytes(&self) -> Option<&[u8]> {
        match &self.storage {
            Storage::None => None,
            Storage::Shared(bytes) => Some(bytes),
            Storage::Unique(bytes) => Some(bytes),
        }
    }

    /// The bytes of the pixels, if there are any and they can be written to.
    #[must_use]
    pub fn bytes_mut(&mut self) -> Option<&mut [u8]> {
        match &mut self.storage {
            Storage::None | Storage::Shared(_) => None,
            Storage::Unique(bytes) => Some(bytes),
        }
    }

    /// Returns true if the pixels can be written to.
    #[must_use]
    pub fn is_writable(&self) -> bool {
        matches!(self.storage, Storage::Unique(_))
    }

    /// Returns the pixel width in the [`ImageInfo`].
    // Port of: include/core/SkPixmap.h#L190 (chrome/m156)
    #[must_use]
    pub fn width(&self) -> i32 {
        self.info.width()
    }

    /// Returns the pixel height in the [`ImageInfo`].
    // Port of: include/core/SkPixmap.h#L196 (chrome/m156)
    #[must_use]
    pub fn height(&self) -> i32 {
        self.info.height()
    }

    /// Returns true if the width or height is zero or smaller.
    // Port of: include/core/SkPixmap.h#L199 (chrome/m156)
    #[doc(alias = "isEmpty")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.info.is_empty()
    }

    /// Returns the width and height.
    // Port of: include/core/SkPixmap.h#L201 (chrome/m156)
    #[must_use]
    pub fn dimensions(&self) -> ISize {
        self.info.dimensions()
    }

    /// Returns the color type.
    // Port of: include/core/SkPixmap.h#L203 (chrome/m156)
    #[doc(alias = "colorType")]
    #[must_use]
    pub fn color_type(&self) -> ColorType {
        self.info.color_type()
    }

    /// Returns the alpha type.
    // Port of: include/core/SkPixmap.h#L205 (chrome/m156)
    #[doc(alias = "alphaType")]
    #[must_use]
    pub fn alpha_type(&self) -> AlphaType {
        self.info.alpha_type()
    }

    /// Returns the color space in the [`ImageInfo`], or `None`.
    // Port of: src/core/SkPixmap.cpp#L63-L65 (chrome/m156)
    #[doc(alias = "colorSpace")]
    #[doc(alias = "refColorSpace")]
    #[must_use]
    pub fn color_space(&self) -> Option<ColorSpace> {
        self.info.color_space()
    }

    /// Returns true if the [`ImageInfo`] has the opaque alpha type (or an always-opaque color
    /// type). Does not check if any pixel value has transparency.
    // Port of: include/core/SkPixmap.h#L230 (chrome/m156)
    #[doc(alias = "isOpaque")]
    #[must_use]
    pub fn is_opaque(&self) -> bool {
        self.info.is_opaque()
    }

    /// Returns the integral rectangle from the origin to `width()` and `height()`.
    // Port of: include/core/SkPixmap.h#L236 (chrome/m156)
    #[must_use]
    pub fn bounds(&self) -> IRect {
        IRect::from_wh(self.width(), self.height())
    }

    /// Returns the number of pixels that fit in a row: `row_bytes() / info().bytes_per_pixel()`.
    /// May be larger than `width()`.
    // Port of: include/core/SkPixmap.h#L245 (chrome/m156)
    #[doc(alias = "rowBytesAsPixels")]
    #[must_use]
    pub fn row_bytes_as_pixels(&self) -> usize {
        self.row_bytes >> self.shift_per_pixel()
    }

    /// Returns the bit shift converting row bytes to row pixels. Returns zero for
    /// [`ColorType::Unknown`].
    // Port of: include/core/SkPixmap.h#L251 (chrome/m156)
    #[doc(alias = "shiftPerPixel")]
    #[must_use]
    pub fn shift_per_pixel(&self) -> usize {
        self.info.shift_per_pixel()
    }

    /// Returns the minimum memory required for the pixel storage. Does not include unused memory
    /// on the last row when `row_bytes_as_pixels()` exceeds `width()`.
    ///
    /// Returns `usize::MAX` if the result does not fit in `usize`. Returns zero if `height()` is
    /// 0 (or `width()` is 0 and `row_bytes` is 0). Returns `height()` times `row_bytes()` if the
    /// color type is [`ColorType::Unknown`].
    // Port of: include/core/SkPixmap.h#L264 (chrome/m156)
    #[doc(alias = "computeByteSize")]
    #[must_use]
    pub fn compute_byte_size(&self) -> usize {
        self.info.compute_byte_size(self.row_bytes)
    }

    // The byte offset of row `y` (`(size_t)y * fRowBytes`).
    #[allow(clippy::cast_sign_loss)] // y is a row index
    fn row_off(&self, y: i32) -> usize {
        (y as usize).wrapping_mul(self.row_bytes)
    }

    // The pixel bytes, which must exist (the C++ asserts `addr()`).
    fn px(&self) -> &[u8] {
        self.bytes().expect("the pixmap has no pixels")
    }

    /// Returns true if all pixels are opaque. [`ColorType`] determines how pixels are encoded,
    /// and whether pixel describes alpha. Returns true for color types without alpha in each
    /// pixel; for other color types, returns true if all pixels have alpha values equivalent to
    /// 1.0 or greater.
    ///
    /// For [`ColorType::RGB565`] or [`ColorType::Gray8`]: always returns true. For
    /// [`ColorType::Alpha8`], [`ColorType::BGRA8888`], [`ColorType::RGBA8888`]: returns true if
    /// all pixel alpha values are 255. For [`ColorType::ARGB4444`]: returns true if all pixel
    /// alpha values are 15. For [`ColorType::RGBAF16`]: returns true if all pixel alpha values
    /// are 1.0 or greater.
    ///
    /// Returns false for [`ColorType::Unknown`].
    // Port of: src/core/SkPixmap.cpp#L578-L744 (chrome/m156)
    #[doc(alias = "computeIsOpaque")]
    #[must_use]
    #[allow(clippy::too_many_lines)] // one arm per color type, as the C++
    #[allow(clippy::match_same_arms)] // one arm per color type, as the C++ switch
    #[allow(clippy::cast_sign_loss)] // mirrors the C++ int -> size_t conversions; x, y are in bounds
    pub fn compute_is_opaque(&self) -> bool {
        let height = self.height();
        let width = self.width();

        match self.color_type() {
            ColorType::Alpha8 => {
                let mut a: u32 = 0xFF;
                for y in 0..height {
                    let row = self.row_off(y);
                    for x in 0..width as usize {
                        a &= u32::from(self.px()[row + x]);
                    }
                    if 0xFF != a {
                        return false;
                    }
                }
                true
            }
            ColorType::A16UNorm => {
                let mut a: u32 = 0xFFFF;
                for y in 0..height {
                    let row = self.row_off(y);
                    for x in 0..width as usize {
                        a &= u32::from(get_u16(self.px(), row + 2 * x));
                    }
                    if 0xFFFF != a {
                        return false;
                    }
                }
                true
            }
            ColorType::A16Float => {
                for y in 0..height {
                    let row = self.row_off(y);
                    for x in 0..width as usize {
                        if get_u16(self.px(), row + 2 * x) < HALF_1 {
                            return false;
                        }
                    }
                }
                true
            }
            ColorType::RGB565
            | ColorType::Gray8
            | ColorType::R8G8UNorm
            | ColorType::R16UNorm
            | ColorType::R16Float
            | ColorType::R16G16UNorm
            | ColorType::R16G16Float
            | ColorType::RGB888x
            | ColorType::RGB101010x
            | ColorType::BGR101010x
            | ColorType::RGBF16F16F16x
            | ColorType::BGR101010xXR
            | ColorType::R8UNorm => true,
            ColorType::ARGB4444 => {
                let mut c: u32 = 0xFFFF;
                for y in 0..height {
                    let row = self.row_off(y);
                    for x in 0..width as usize {
                        c &= u32::from(get_u16(self.px(), row + 2 * x));
                    }
                    if 0xF != get_packed_a4444(c) {
                        return false;
                    }
                }
                true
            }
            ColorType::BGRA8888 | ColorType::RGBA8888 | ColorType::SRGBA8888 => {
                let mut c: u32 = !0;
                for y in 0..height {
                    let row = self.row_off(y);
                    for x in 0..width as usize {
                        c &= get_u32(self.px(), row + 4 * x);
                    }
                    if 0xFF != get_packed_a32(c) {
                        return false;
                    }
                }
                true
            }
            ColorType::RGBAF16Norm | ColorType::RGBAF16 => {
                // row += this->rowBytes() >> 1 (in SkHalfs)
                for y in 0..height {
                    let row = (y as usize).wrapping_mul(self.row_bytes >> 1) * 2;
                    for x in 0..width as usize {
                        if get_u16(self.px(), row + 2 * (4 * x + 3)) < HALF_1 {
                            return false;
                        }
                    }
                }
                true
            }
            ColorType::RGBAF32 => {
                // row += this->rowBytes() >> 2 (in floats)
                for y in 0..height {
                    let row = (y as usize).wrapping_mul(self.row_bytes >> 2) * 4;
                    for x in 0..width as usize {
                        if get_f32(self.px(), row + 4 * (4 * x + 3)) < 1.0f32 {
                            return false;
                        }
                    }
                }
                true
            }
            ColorType::RGBA1010102 | ColorType::BGRA1010102 => {
                let mut c: u32 = !0;
                for y in 0..height {
                    let row = self.row_off(y);
                    for x in 0..width as usize {
                        c &= get_u32(self.px(), row + 4 * x);
                    }
                    if 0b11 != c >> 30 {
                        return false;
                    }
                }
                true
            }
            ColorType::BGRA10101010XR => {
                const ONE: u64 = 510 + 384;
                for y in 0..height {
                    let row = self.row_off(y);
                    for x in 0..width as usize {
                        if (get_u64(self.px(), row + 8 * x) >> 54) < ONE {
                            return false;
                        }
                    }
                }
                true
            }
            ColorType::RGBA10x6 => {
                let mut acc: u16 = 0xFFC0; // Ignore bottom six bits
                for y in 0..height {
                    let row = self.row_off(y);
                    for x in 0..width as usize {
                        #[allow(clippy::cast_possible_truncation)] // (row[x] >> 48) is 16 bits
                        {
                            acc &= (get_u64(self.px(), row + 8 * x) >> 48) as u16;
                        }
                    }
                    if 0xFFC0 != acc {
                        return false;
                    }
                }
                true
            }
            ColorType::R16G16B16A16UNorm => {
                let mut acc: u16 = 0xFFFF;
                for y in 0..height {
                    let row = self.row_off(y);
                    for x in 0..width as usize {
                        #[allow(clippy::cast_possible_truncation)] // (row[x] >> 48) is 16 bits
                        {
                            acc &= (get_u64(self.px(), row + 8 * x) >> 48) as u16;
                        }
                    }
                    if 0xFFFF != acc {
                        return false;
                    }
                }
                true
            }
            ColorType::Unknown => {
                debug_assert!(false);
                false
            }
        }
    }

    // `fast_getaddr`: the byte offset of pixel (x, y) that does not go through `computeOffset`.
    // Port of: src/core/SkPixmap.cpp#L91-L94 (chrome/m156)
    #[allow(clippy::cast_sign_loss)] // x and y are asserted in bounds
    fn fast_getaddr(&self, x: i32, y: i32) -> usize {
        let x = (x as usize) << color_type_shift_per_pixel(self.color_type());
        self.row_off(y).wrapping_add(x)
    }

    /// Returns the alpha (as a normalized float) of the pixel at `(x, y)`.
    ///
    /// This is roughly equivalent to `get_color(..).a()`, but can be more efficient (and more
    /// precise if the pixels store more than 8 bits per component).
    ///
    /// Input is not validated: out of bounds values of `x` or `y` trigger a debug assertion.
    // Port of: src/core/SkPixmap.cpp#L96-L173 (chrome/m156)
    #[doc(alias = "getAlphaf")]
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // mirrors the C++ integer -> float conversions
    #[allow(clippy::too_many_lines)] // one arm per color type, as the C++
    #[allow(clippy::match_same_arms)] // one arm per color type, as the C++ switch
    #[allow(clippy::cast_sign_loss)] // mirrors the C++ int -> size_t conversions; x, y are in bounds
    pub fn get_alpha_f(&self, p: impl Into<IPoint>) -> f32 {
        let IPoint { x, y } = p.into();
        debug_assert!(self.addr().is_some());
        assert_in_bounds(&self.info, x, y);

        let src = self.fast_getaddr(x, y);
        let bytes = self.px();

        match self.color_type() {
            ColorType::Unknown => 0.0,
            ColorType::Gray8
            | ColorType::R8G8UNorm
            | ColorType::R16UNorm
            | ColorType::R16Float
            | ColorType::R16G16UNorm
            | ColorType::R16G16Float
            | ColorType::RGB565
            | ColorType::RGB888x
            | ColorType::RGB101010x
            | ColorType::BGR101010x
            | ColorType::BGR101010xXR
            | ColorType::RGBF16F16F16x
            | ColorType::R8UNorm => 1.0,
            ColorType::Alpha8 => f32::from(bytes[src]) * (1.0f32 / 255.0f32),
            ColorType::A16UNorm => f32::from(get_u16(bytes, src)) * (1.0f32 / 65535.0f32),
            ColorType::A16Float => half_to_float(get_u16(bytes, src)),
            ColorType::ARGB4444 => {
                let u16_ = get_u16(bytes, src);
                (get_packed_a4444(u32::from(u16_)) as f32) * (1.0f32 / 15.0f32)
            }
            ColorType::RGBA8888 | ColorType::BGRA8888 | ColorType::SRGBA8888 => {
                f32::from(bytes[src + 3]) * (1.0f32 / 255.0f32)
            }
            ColorType::RGBA1010102 | ColorType::BGRA1010102 => {
                let u32_ = get_u32(bytes, src);
                (u32_ >> 30) as f32 * (1.0f32 / 3.0f32)
            }
            ColorType::BGRA10101010XR => {
                let u64_ = get_u64(bytes, src);
                ((u64_ >> 54).wrapping_sub(384)) as f32 / 510.0f32
            }
            ColorType::RGBA10x6 => {
                let u64_ = get_u64(bytes, src);
                (u64_ >> 54) as f32 * (1.0f32 / 1023.0f32)
            }
            ColorType::R16G16B16A16UNorm => {
                let u64_ = get_u64(bytes, src);
                (u64_ >> 48) as f32 * (1.0f32 / 65535.0f32)
            }
            ColorType::RGBAF16Norm | ColorType::RGBAF16 => half_to_float(get_u16(bytes, src + 6)),
            ColorType::RGBAF32 => get_f32(bytes, src + 12),
        }
    }

    /// Returns the pixel at `(x, y)` as an unpremultiplied color. Returns black with alpha if the
    /// color type is [`ColorType::Alpha8`].
    ///
    /// Input is not validated: out of bounds values of `x` or `y` trigger a debug assertion, and
    /// the pixmap must have pixels. The [`ColorSpace`] in the [`ImageInfo`] is ignored. Some
    /// color precision may be lost in the conversion to unpremultiplied color; original pixel
    /// data may have additional precision.
    // Port of: src/core/SkPixmap.cpp#L192-L391 (chrome/m156)
    #[doc(alias = "getColor")]
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // mirrors the C++ integer -> float conversions
    #[allow(clippy::too_many_lines)] // one arm per color type, as the C++
    #[allow(clippy::many_single_char_names)] // the C++ locals
    #[allow(clippy::float_cmp)] // mirrors the exact `a != 0` comparisons of the C++
    #[allow(clippy::match_same_arms)] // one arm per color type, as the C++ switch
    #[allow(clippy::cast_sign_loss)] // mirrors the C++ int -> size_t conversions; x, y are in bounds
    pub fn get_color(&self, p: impl Into<IPoint>) -> Color {
        let IPoint { x, y } = p.into();
        debug_assert!(self.addr().is_some());
        assert_in_bounds(&self.info, x, y);

        let needs_unpremul = AlphaType::Premul == self.info.alpha_type();
        let to_color = |maybe_premul_color: u32| -> Color {
            if needs_unpremul {
                pm_color_to_color(maybe_premul_color)
            } else {
                Color::new(swizzle_bgra_to_pm_color(maybe_premul_color))
            }
        };

        let bytes = self.px();
        let a8 = |x: i32, y: i32| bytes[self.fast_getaddr(x, y)];
        let a16 = |x: i32, y: i32| get_u16(bytes, self.fast_getaddr(x, y));
        let a32 = |x: i32, y: i32| get_u32(bytes, self.fast_getaddr(x, y));
        let a64 = |x: i32, y: i32| get_u64(bytes, self.fast_getaddr(x, y));

        match self.color_type() {
            ColorType::Gray8 => {
                let value = u32::from(a8(x, y));
                color_set_rgb(value, value, value)
            }
            ColorType::R8UNorm => color_set_rgb(u32::from(a8(x, y)), 0, 0),
            ColorType::Alpha8 => color_set_a(Color::new(0), u32::from(a8(x, y))),
            ColorType::A16UNorm => color_set_a(
                Color::new(0),
                trunc_u32(f32::from(a16(x, y)) * (255.0f32 / 65535.0f32)),
            ),
            ColorType::A16Float => color_set_a(
                Color::new(0),
                trunc_u32(255.0f32 * half_to_float(a16(x, y))),
            ),
            ColorType::R16Float => {
                color_set_rgb(trunc_u32(255.0f32 * half_to_float(a16(x, y))), 0, 0)
            }
            ColorType::RGB565 => pixel16_to_color(u32::from(a16(x, y))),
            ColorType::ARGB4444 => {
                let c = pixel4444_to_pixel32(u32::from(a16(x, y)));
                to_color(c)
            }
            ColorType::R8G8UNorm => {
                let value = u32::from(a16(x, y));
                color_set_rgb(value & 0xff, (value >> 8) & 0xff, 0)
            }
            ColorType::R16UNorm => {
                let value = a16(x, y);
                color_set_rgb(trunc_u32(f32::from(value) * (255.0f32 / 65535.0f32)), 0, 0)
            }
            ColorType::R16G16UNorm => {
                let value = a32(x, y);
                let r = trunc_u32(((value & 0xffff) as f32) * (255.0f32 / 65535.0f32)) & 0xff;
                let g =
                    trunc_u32((((value >> 16) & 0xffff) as f32) * (255.0f32 / 65535.0f32)) & 0xff;
                color_set_rgb(r, g, 0)
            }
            ColorType::R16G16Float => {
                let value = a32(x, y);
                let r = half_to_float((value & 0xffff) as u16);
                let g = half_to_float(((value >> 16) & 0xffff) as u16);
                color_set_rgb(
                    trunc_u32(255.0f32 * r) & 0xff,
                    trunc_u32(255.0f32 * g) & 0xff,
                    0,
                )
            }
            ColorType::RGB888x => {
                let value = a32(x, y);
                Color::new(swizzle_rb(value | 0xff00_0000))
            }
            ColorType::BGRA8888 => {
                let value = a32(x, y);
                to_color(swizzle_bgra_to_pm_color(value))
            }
            ColorType::RGBA8888 => {
                let value = a32(x, y);
                to_color(swizzle_rgba_to_pm_color(value))
            }
            ColorType::SRGBA8888 => {
                let value = a32(x, y);
                let mut r = (value & 0xff) as f32 * (1.0f32 / 255.0f32);
                let mut g = ((value >> 8) & 0xff) as f32 * (1.0f32 / 255.0f32);
                let mut b = ((value >> 16) & 0xff) as f32 * (1.0f32 / 255.0f32);
                let mut a = ((value >> 24) & 0xff) as f32 * (1.0f32 / 255.0f32);

                r = srgb_to_linear(r);
                g = srgb_to_linear(g);
                b = srgb_to_linear(b);
                if a != 0.0 && needs_unpremul {
                    r = t_pin(r / a, 0.0f32, 1.0f32);
                    g = t_pin(g / a, 0.0f32, 1.0f32);
                    b = t_pin(b / a, 0.0f32, 1.0f32);
                }
                r *= 255.0f32;
                g *= 255.0f32;
                b *= 255.0f32;
                a *= 255.0f32;
                color_set_argb(trunc_u32(a), trunc_u32(r), trunc_u32(g), trunc_u32(b))
            }
            ColorType::RGB101010x => {
                let value = a32(x, y);
                // Convert 10-bit rgb to 8-bit rgb
                color_set_rgb(
                    trunc_u32((value & 0x3ff) as f32 * (255.0f32 / 1023.0f32)),
                    trunc_u32(((value >> 10) & 0x3ff) as f32 * (255.0f32 / 1023.0f32)),
                    trunc_u32(((value >> 20) & 0x3ff) as f32 * (255.0f32 / 1023.0f32)),
                )
            }
            ColorType::BGR101010xXR => {
                debug_assert!(false);
                Color::new(0)
            }
            ColorType::BGR101010x => {
                let value = a32(x, y);
                // Convert 10-bit bgr to 8-bit rgb
                color_set_rgb(
                    trunc_u32(((value >> 20) & 0x3ff) as f32 * (255.0f32 / 1023.0f32)),
                    trunc_u32(((value >> 10) & 0x3ff) as f32 * (255.0f32 / 1023.0f32)),
                    trunc_u32((value & 0x3ff) as f32 * (255.0f32 / 1023.0f32)),
                )
            }
            ColorType::BGRA1010102 | ColorType::RGBA1010102 => {
                let value = a32(x, y);
                let mut r = (value & 0x3ff) as f32 * (1.0f32 / 1023.0f32);
                let g = ((value >> 10) & 0x3ff) as f32 * (1.0f32 / 1023.0f32);
                let mut b = ((value >> 20) & 0x3ff) as f32 * (1.0f32 / 1023.0f32);
                let mut a = ((value >> 30) & 0x3) as f32 * (1.0f32 / 3.0f32);
                let mut g = g;
                if self.color_type() == ColorType::BGRA1010102 {
                    std::mem::swap(&mut r, &mut b);
                }
                if a != 0.0 && needs_unpremul {
                    r = t_pin(r / a, 0.0f32, 1.0f32);
                    g = t_pin(g / a, 0.0f32, 1.0f32);
                    b = t_pin(b / a, 0.0f32, 1.0f32);
                }
                b *= 255.0f32;
                g *= 255.0f32;
                r *= 255.0f32;
                a *= 255.0f32;
                color_set_argb(trunc_u32(a), trunc_u32(r), trunc_u32(g), trunc_u32(b))
            }
            ColorType::BGRA10101010XR => {
                debug_assert!(false);
                Color::new(0)
            }
            ColorType::RGBA10x6 => {
                let value = a64(x, y);
                color_set_argb(
                    trunc_u32(((value >> 54) & 0x3ff) as f32 * (255.0f32 / 1023.0f32)),
                    trunc_u32(((value >> 6) & 0x3ff) as f32 * (255.0f32 / 1023.0f32)),
                    trunc_u32(((value >> 22) & 0x3ff) as f32 * (255.0f32 / 1023.0f32)),
                    trunc_u32(((value >> 38) & 0x3ff) as f32 * (255.0f32 / 1023.0f32)),
                )
            }
            ColorType::R16G16B16A16UNorm => {
                let value = a64(x, y);
                let mut r = (value & 0xffff) as f32 * (1.0f32 / 65535.0f32);
                let mut g = ((value >> 16) & 0xffff) as f32 * (1.0f32 / 65535.0f32);
                let mut b = ((value >> 32) & 0xffff) as f32 * (1.0f32 / 65535.0f32);
                let mut a = ((value >> 48) & 0xffff) as f32 * (1.0f32 / 65535.0f32);
                if a != 0.0 && needs_unpremul {
                    r *= 1.0f32 / a;
                    g *= 1.0f32 / a;
                    b *= 1.0f32 / a;
                }
                r *= 255.0f32;
                g *= 255.0f32;
                b *= 255.0f32;
                a *= 255.0f32;
                color_set_argb(trunc_u32(a), trunc_u32(r), trunc_u32(g), trunc_u32(b))
            }
            ColorType::RGBF16F16F16x => {
                // (const uint64_t*)fPixels + y * (fRowBytes >> 3) + x
                let off = ((y as usize) * (self.row_bytes >> 3) + x as usize) * 8;
                let mut p4 = half4_to_float4(bytes, off);
                p4[3] = 1.0f32;
                // p4 is RGBA, but we want BGRA, so we need to swap next
                Color::new(sk4f_to_l32(swizzle_rb4(p4)))
            }
            ColorType::RGBAF16Norm | ColorType::RGBAF16 => {
                let off = ((y as usize) * (self.row_bytes >> 3) + x as usize) * 8;
                let mut p4 = half4_to_float4(bytes, off);
                if p4[3] != 0.0 && needs_unpremul {
                    let inva = 1.0f32 / p4[3];
                    p4 = mul4(p4, [inva, inva, inva, 1.0f32]);
                }
                // p4 is RGBA, but we want BGRA, so we need to swap next
                Color::new(sk4f_to_l32(swizzle_rb4(p4)))
            }
            ColorType::RGBAF32 => {
                // (const float*)fPixels + 4*y*(fRowBytes >> 4) + 4*x
                let off = (4 * (y as usize) * (self.row_bytes >> 4) + 4 * x as usize) * 4;
                let mut p4 = [
                    get_f32(bytes, off),
                    get_f32(bytes, off + 4),
                    get_f32(bytes, off + 8),
                    get_f32(bytes, off + 12),
                ];
                // From here on, just like F16:
                if p4[3] != 0.0 && needs_unpremul {
                    let inva = 1.0f32 / p4[3];
                    p4 = mul4(p4, [inva, inva, inva, 1.0f32]);
                }
                // p4 is RGBA, but we want BGRA, so we need to swap next
                Color::new(sk4f_to_l32(swizzle_rb4(p4)))
            }
            ColorType::Unknown => {
                debug_assert!(false);
                color_set_argb(0, 0, 0, 0)
            }
        }
    }

    /// Returns the pixel at `(x, y)` as an unpremultiplied float color. Returns black with alpha
    /// if the color type is [`ColorType::Alpha8`].
    ///
    /// Input is not validated: out of bounds values of `x` or `y` trigger a debug assertion, and
    /// the pixmap must have pixels. The [`ColorSpace`] in the [`ImageInfo`] is ignored. Some
    /// color precision may be lost in the conversion to unpremultiplied color; original pixel
    /// data may have additional precision, though this is less likely than for
    /// [`Self::get_color`]. Rounding errors may occur if the underlying type has lower
    /// precision.
    // Port of: src/core/SkPixmap.cpp#L395-L576 (chrome/m156)
    #[doc(alias = "getColor4f")]
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // mirrors the C++ integer -> float conversions
    #[allow(clippy::too_many_lines)] // one arm per color type, as the C++
    #[allow(clippy::many_single_char_names)] // the C++ locals
    #[allow(clippy::float_cmp)] // mirrors the exact `a != 0` comparisons of the C++
    #[allow(clippy::match_same_arms)] // one arm per color type, as the C++ switch
    #[allow(clippy::cast_sign_loss)] // mirrors the C++ int -> size_t conversions; x, y are in bounds
    pub fn get_color_4f(&self, p: impl Into<IPoint>) -> Color4f {
        let IPoint { x, y } = p.into();
        debug_assert!(self.addr().is_some());
        assert_in_bounds(&self.info, x, y);

        let needs_unpremul = AlphaType::Premul == self.info.alpha_type();
        let to_color = |maybe_premul_color: u32| -> Color {
            if needs_unpremul {
                pm_color_to_color(maybe_premul_color)
            } else {
                Color::new(swizzle_bgra_to_pm_color(maybe_premul_color))
            }
        };

        let bytes = self.px();
        let a8 = |x: i32, y: i32| bytes[self.fast_getaddr(x, y)];
        let a16 = |x: i32, y: i32| get_u16(bytes, self.fast_getaddr(x, y));
        let a32 = |x: i32, y: i32| get_u32(bytes, self.fast_getaddr(x, y));
        let a64 = |x: i32, y: i32| get_u64(bytes, self.fast_getaddr(x, y));

        match self.color_type() {
            ColorType::Gray8 => {
                let value = f32::from(a8(x, y)) / 255.0f32;
                Color4f::new(value, value, value, 1.0f32)
            }
            ColorType::R8UNorm => Color4f::new(f32::from(a8(x, y)) / 255.0f32, 0.0, 0.0, 1.0),
            ColorType::Alpha8 => Color4f::new(0.0, 0.0, 0.0, f32::from(a8(x, y)) / 255.0f32),
            ColorType::R16UNorm => Color4f::new(f32::from(a16(x, y)) / 65535.0f32, 0.0, 0.0, 1.0),
            ColorType::A16UNorm => Color4f::new(0.0, 0.0, 0.0, f32::from(a16(x, y)) / 65535.0f32),
            ColorType::R16Float => Color4f::new(half_to_float(a16(x, y)), 0.0, 0.0, 1.0),
            ColorType::A16Float => Color4f::new(0.0, 0.0, 0.0, half_to_float(a16(x, y))),
            ColorType::RGB565 => Color4f::from_color(pixel16_to_color(u32::from(a16(x, y)))),
            ColorType::ARGB4444 => {
                let c = pixel4444_to_pixel32(u32::from(a16(x, y)));
                Color4f::from_color(to_color(c))
            }
            ColorType::R8G8UNorm => {
                let value = u32::from(a16(x, y));
                Color4f::from_color(color_set_rgb(value & 0xff, (value >> 8) & 0xff, 0))
            }
            ColorType::R16G16UNorm => {
                let value = a32(x, y);
                let r = (value & 0xffff) as f32 * (1.0f32 / 65535.0f32);
                let g = ((value >> 16) & 0xffff) as f32 * (1.0f32 / 65535.0f32);
                Color4f::new(r, g, 0.0, 1.0)
            }
            ColorType::R16G16Float => {
                let value = a32(x, y);
                let r = half_to_float((value & 0xffff) as u16);
                let g = half_to_float(((value >> 16) & 0xffff) as u16);
                Color4f::new(r, g, 0.0, 1.0)
            }
            ColorType::RGB888x => {
                let c = Color::new(swizzle_rb(a32(x, y) | 0xff00_0000));
                Color4f::from_color(c)
            }
            ColorType::BGRA8888 => {
                let c = swizzle_bgra_to_pm_color(a32(x, y));
                Color4f::from_color(to_color(c))
            }
            ColorType::RGBA8888 => {
                let c = swizzle_rgba_to_pm_color(a32(x, y));
                Color4f::from_color(to_color(c))
            }
            ColorType::SRGBA8888 => {
                let value = a32(x, y);
                let mut r = (value & 0xff) as f32 * (1.0f32 / 255.0f32);
                let mut g = ((value >> 8) & 0xff) as f32 * (1.0f32 / 255.0f32);
                let mut b = ((value >> 16) & 0xff) as f32 * (1.0f32 / 255.0f32);
                let a = ((value >> 24) & 0xff) as f32 * (1.0f32 / 255.0f32);
                r = srgb_to_linear(r);
                g = srgb_to_linear(g);
                b = srgb_to_linear(b);
                if a != 0.0 && needs_unpremul {
                    r = t_pin(r / a, 0.0f32, 1.0f32);
                    g = t_pin(g / a, 0.0f32, 1.0f32);
                    b = t_pin(b / a, 0.0f32, 1.0f32);
                }
                Color4f::new(r, g, b, a)
            }
            ColorType::BGR101010xXR => {
                debug_assert!(false);
                Color4f::default()
            }
            ColorType::RGB101010x => {
                let value = a32(x, y);
                // Convert 10-bit RGB to floats
                Color4f::new(
                    (value & 0x3ff) as f32 / 1023.0f32,
                    ((value >> 10) & 0x3ff) as f32 / 1023.0f32,
                    ((value >> 20) & 0x3ff) as f32 / 1023.0f32,
                    1.0f32,
                )
            }
            ColorType::BGR101010x => {
                let value = a32(x, y);
                // Convert 10-bit BGR color values to RGBA floats
                Color4f::new(
                    ((value >> 20) & 0x3ff) as f32 / 1023.0f32,
                    ((value >> 10) & 0x3ff) as f32 / 1023.0f32,
                    (value & 0x3ff) as f32 / 1023.0f32,
                    1.0f32,
                )
            }
            ColorType::RGBA1010102 | ColorType::BGRA1010102 => {
                let value = a32(x, y);
                // Convert 10-bit color values to floats
                let b = (value & 0x3ff) as f32 * (1.0f32 / 1023.0f32);
                let mut g = ((value >> 10) & 0x3ff) as f32 * (1.0f32 / 1023.0f32);
                let mut r = ((value >> 20) & 0x3ff) as f32 * (1.0f32 / 1023.0f32);
                let a = ((value >> 30) & 0x3) as f32 * (1.0f32 / 3.0f32);
                let mut b = b;
                if a != 0.0 && needs_unpremul {
                    r = t_pin(r / a, 0.0f32, 1.0f32);
                    g = t_pin(g / a, 0.0f32, 1.0f32);
                    b = t_pin(b / a, 0.0f32, 1.0f32);
                }
                Color4f::new(r, g, b, a)
            }
            ColorType::BGRA10101010XR => {
                debug_assert!(false);
                Color4f::default()
            }
            ColorType::RGBA10x6 => {
                let value = a64(x, y);
                Color4f::new(
                    ((value >> 6) & 0x3ff) as f32 * (1.0f32 / 1023.0f32),
                    ((value >> 22) & 0x3ff) as f32 * (1.0f32 / 1023.0f32),
                    ((value >> 38) & 0x3ff) as f32 * (1.0f32 / 1023.0f32),
                    ((value >> 54) & 0x3ff) as f32 * (1.0f32 / 1023.0f32),
                )
            }
            ColorType::R16G16B16A16UNorm => {
                let value = a64(x, y);

                let mut r = (value & 0xffff) as f32 * (1.0f32 / 65535.0f32);
                let mut g = ((value >> 16) & 0xffff) as f32 * (1.0f32 / 65535.0f32);
                let mut b = ((value >> 32) & 0xffff) as f32 * (1.0f32 / 65535.0f32);
                let a = ((value >> 48) & 0xffff) as f32 * (1.0f32 / 65535.0f32);
                if a != 0.0 && needs_unpremul {
                    r *= 1.0f32 / a;
                    g *= 1.0f32 / a;
                    b *= 1.0f32 / a;
                }
                Color4f::new(r, g, b, a)
            }
            ColorType::RGBAF16Norm | ColorType::RGBAF16 => {
                let off = ((y as usize) * (self.row_bytes >> 3) + x as usize) * 8;
                let mut p4 = half4_to_float4(bytes, off);
                if p4[3] != 0.0 && needs_unpremul {
                    let inva = 1.0f32 / p4[3];
                    p4 = mul4(p4, [inva, inva, inva, 1.0f32]);
                }
                Color4f::new(p4[0], p4[1], p4[2], p4[3])
            }
            ColorType::RGBF16F16F16x => {
                let off = ((y as usize) * (self.row_bytes >> 3) + x as usize) * 8;
                let mut p4 = half4_to_float4(bytes, off);
                p4[3] = 1.0f32;
                Color4f::new(p4[0], p4[1], p4[2], p4[3])
            }
            ColorType::RGBAF32 => {
                let off = (4 * (y as usize) * (self.row_bytes >> 4) + 4 * x as usize) * 4;
                let mut p4 = [
                    get_f32(bytes, off),
                    get_f32(bytes, off + 4),
                    get_f32(bytes, off + 8),
                    get_f32(bytes, off + 12),
                ];
                // From here on, just like F16:
                if p4[3] != 0.0 && needs_unpremul {
                    let inva = 1.0f32 / p4[3];
                    p4 = mul4(p4, [inva, inva, inva, 1.0f32]);
                }
                Color4f::new(p4[0], p4[1], p4[2], p4[3])
            }
            ColorType::Unknown => {
                debug_assert!(false);
                Color4f::default() // SkColors::kTransparent
            }
        }
    }

    // `addr(x, y)`'s preconditions: `x` and `y` in bounds, and the pixels exist.
    fn pixel_offset(&self, x: i32, y: i32) -> usize {
        assert_in_bounds(&self.info, x, y);
        self.fast_getaddr(x, y)
    }

    /// Returns the readable pixels starting at pixel `(x, y)`, to the end of the borrowed bytes.
    /// Returns `None` if there are no pixels or the color type is [`ColorType::Unknown`].
    ///
    /// Input is not validated: out of bounds values of `x` or `y` trigger a debug assertion.
    // Port of: include/core/SkPixmap.h#L384-L386 (chrome/m156)
    #[doc(alias = "addr")]
    #[must_use]
    pub fn addr_at(&self, p: impl Into<IPoint>) -> Option<&[u8]> {
        let p = p.into();
        if self.color_type() == ColorType::Unknown {
            return None;
        }
        let off = self.info.compute_offset(p, self.row_bytes);
        self.bytes().map(|bytes| &bytes[off..])
    }

    /// Returns the writable pixels starting at pixel `(x, y)`. Returns `None` if the pixels are
    /// read-only, or the color type is [`ColorType::Unknown`].
    // Port of: include/core/SkPixmap.h#L499-L503 (chrome/m156)
    #[doc(alias = "writable_addr")]
    pub fn writable_addr_at(&mut self, p: impl Into<IPoint>) -> Option<&mut [u8]> {
        let p = p.into();
        if self.color_type() == ColorType::Unknown {
            return None;
        }
        let off = self.info.compute_offset(p, self.row_bytes);
        self.bytes_mut().map(|bytes| &mut bytes[off..])
    }

    /// Returns the writable pixels, if they can be written to.
    // Port of: include/core/SkPixmap.h#L488 (chrome/m156)
    pub fn writable_addr(&mut self) -> Option<&mut [u8]> {
        self.bytes_mut()
    }

    /// Returns the 8-bit pixel at `(x, y)`: one byte per pixel (asserted in debug builds).
    // Port of: include/core/SkPixmap.h#L444-L448 (chrome/m156)
    #[must_use]
    pub fn addr8(&self, x: i32, y: i32) -> u8 {
        debug_assert_eq!(1, self.info.bytes_per_pixel());
        self.px()[self.pixel_offset(x, y)]
    }

    /// Returns the 16-bit pixel at `(x, y)`: two bytes per pixel (asserted in debug builds).
    // Port of: include/core/SkPixmap.h#L458-L462 (chrome/m156)
    #[must_use]
    pub fn addr16(&self, x: i32, y: i32) -> u16 {
        debug_assert_eq!(2, self.info.bytes_per_pixel());
        get_u16(self.px(), self.pixel_offset(x, y))
    }

    /// Returns the 32-bit pixel at `(x, y)`: four bytes per pixel (asserted in debug builds).
    // Port of: include/core/SkPixmap.h#L472-L476 (chrome/m156)
    #[must_use]
    pub fn addr32(&self, x: i32, y: i32) -> u32 {
        debug_assert_eq!(4, self.info.bytes_per_pixel());
        get_u32(self.px(), self.pixel_offset(x, y))
    }

    /// Returns the 64-bit pixel at `(x, y)`: eight bytes per pixel (asserted in debug builds).
    // Port of: include/core/SkPixmap.h#L486-L490 (chrome/m156)
    #[must_use]
    pub fn addr64(&self, x: i32, y: i32) -> u64 {
        debug_assert_eq!(8, self.info.bytes_per_pixel());
        get_u64(self.px(), self.pixel_offset(x, y))
    }

    /// Returns the four half-float components of the pixel at `(x, y)` (as bit patterns).
    // Port of: include/core/SkPixmap.h#L496-L504 (chrome/m156)
    #[doc(alias = "addrF16")]
    #[must_use]
    pub fn addr_f16(&self, x: i32, y: i32) -> [u16; 4] {
        debug_assert_eq!(8, self.info.bytes_per_pixel());
        debug_assert!(matches!(
            self.color_type(),
            ColorType::RGBAF16 | ColorType::RGBAF16Norm
        ));
        let off = self.pixel_offset(x, y);
        let bytes = self.px();
        [
            get_u16(bytes, off),
            get_u16(bytes, off + 2),
            get_u16(bytes, off + 4),
            get_u16(bytes, off + 6),
        ]
    }

    // The writable bytes, which must exist (`writable_addr8()` & co. on a read-only pixmap).
    fn px_mut(&mut self) -> &mut [u8] {
        self.bytes_mut()
            .expect("the pixmap has no pixels, or they are read-only")
    }

    /// Writes the 8-bit pixel at `(x, y)`. Panics if the pixels are read-only.
    // Port of: include/core/SkPixmap.h#L513-L515 (chrome/m156)
    #[doc(alias = "writable_addr8")]
    pub fn set_addr8(&mut self, x: i32, y: i32, value: u8) {
        debug_assert_eq!(1, self.info.bytes_per_pixel());
        let off = self.pixel_offset(x, y);
        self.px_mut()[off] = value;
    }

    /// Writes the 16-bit pixel at `(x, y)`. Panics if the pixels are read-only.
    // Port of: include/core/SkPixmap.h#L527-L529 (chrome/m156)
    #[doc(alias = "writable_addr16")]
    pub fn set_addr16(&mut self, x: i32, y: i32, value: u16) {
        debug_assert_eq!(2, self.info.bytes_per_pixel());
        let off = self.pixel_offset(x, y);
        self.px_mut()[off..off + 2].copy_from_slice(&value.to_ne_bytes());
    }

    /// Writes the 32-bit pixel at `(x, y)`. Panics if the pixels are read-only.
    // Port of: include/core/SkPixmap.h#L542-L544 (chrome/m156)
    #[doc(alias = "writable_addr32")]
    pub fn set_addr32(&mut self, x: i32, y: i32, value: u32) {
        debug_assert_eq!(4, self.info.bytes_per_pixel());
        let off = self.pixel_offset(x, y);
        self.px_mut()[off..off + 4].copy_from_slice(&value.to_ne_bytes());
    }

    /// Writes the 64-bit pixel at `(x, y)`. Panics if the pixels are read-only.
    // Port of: include/core/SkPixmap.h#L556-L558 (chrome/m156)
    #[doc(alias = "writable_addr64")]
    pub fn set_addr64(&mut self, x: i32, y: i32, value: u64) {
        debug_assert_eq!(8, self.info.bytes_per_pixel());
        let off = self.pixel_offset(x, y);
        self.px_mut()[off..off + 8].copy_from_slice(&value.to_ne_bytes());
    }

    /// Copies a rectangle of pixels to `dst_pixels`. Copy starts at `src`, and does not exceed
    /// the pixmap (`width()`, `height()`). `dst_info` specifies width, height, color type, alpha
    /// type, and color space of the destination; `dst_row_bytes` the destination row length.
    /// Returns true if pixels are copied. Returns false if `dst_row_bytes` is less than
    /// `dst_info.min_row_bytes()`.
    ///
    /// Pixels are copied only if pixel conversion is possible (`SkImageInfoValidConversion`).
    /// `src.x` and `src.y` may be negative to copy only the top or left of the source. Returns
    /// false if the pixmap width or height is zero or negative, if `abs(src.x)` is greater than or
    /// equal to the pixmap width, or if `abs(src.y)` is greater than or equal to the pixmap
    /// height.
    ///
    /// skia-rust: also returns false if this pixmap has no pixels, or if `dst_pixels` is too
    /// small for `dst_info` and `dst_row_bytes` (the C++ trusts the caller).
    // Port of: src/core/SkPixmap.cpp#L175-L190 (chrome/m156)
    #[doc(alias = "readPixels")]
    pub fn read_pixels(
        &self,
        dst_info: &ImageInfo,
        dst_pixels: &mut [u8],
        dst_row_bytes: usize,
        src: impl Into<IPoint>,
    ) -> bool {
        let IPoint { x, y } = src.into();
        if !image_info_valid_conversion(dst_info, &self.info) {
            return false;
        }

        let mut rec = ReadPixelsRec::new(dst_info, Some(dst_pixels), dst_row_bytes, x, y);
        if !rec.trim(self.info.width(), self.info.height()) {
            return false;
        }

        let Some(src_pixels) = self.addr_at((rec.x, rec.y)) else {
            return false;
        };
        let src_info = self.info.with_dimensions(rec.info.dimensions());
        let Some(dst_pixels) = rec.pixels.and_then(|p| p.get_mut(rec.offset..)) else {
            return false;
        };
        convert_pixels(
            &rec.info,
            dst_pixels,
            rec.row_bytes,
            &src_info,
            src_pixels,
            self.row_bytes,
        )
    }

    /// Copies a rectangle of pixels to `dst` (`readPixels(const SkPixmap&, int, int)`): like
    /// [`Self::read_pixels`] with `dst`'s info, pixels and row bytes. Returns false if `dst` has
    /// no writable pixels.
    // Port of: include/core/SkPixmap.h#L652-L654 (chrome/m156)
    #[doc(alias = "readPixels")]
    pub fn read_pixels_to_pixmap(&self, dst: &mut Pixmap<'_>, src: impl Into<IPoint>) -> bool {
        let info = dst.info().clone();
        let row_bytes = dst.row_bytes();
        let Some(dst_pixels) = dst.writable_addr() else {
            return false;
        };
        self.read_pixels(&info, dst_pixels, row_bytes, src)
    }

    /// Writes `color` to the pixels bounded by `subset` (or the pixmap's bounds, for `None`);
    /// returns true on success. Returns false if the color type is [`ColorType::Unknown`], if
    /// `subset` does not intersect `bounds()`, or if the pixels are read-only.
    ///
    /// - `color` sRGB unpremultiplied color to write
    /// - `subset` bounding integer rectangle of the pixels to write
    // Port of: src/core/SkPixmap.cpp#L746-L748 (chrome/m156)
    pub fn erase(&mut self, color: impl Into<Color>, subset: Option<&IRect>) -> bool {
        let subset = subset.copied().unwrap_or_else(|| self.bounds());
        self.erase_4f(Color4f::from_color(color.into()), Some(&subset))
    }

    /// Writes `color` to the pixels bounded by `subset` (or the pixmap's bounds, for `None`);
    /// returns true on success. Returns false if the color type is [`ColorType::Unknown`], if
    /// `subset` is not `None` and does not intersect `bounds()`, or if the pixels are read-only.
    ///
    /// - `color` unpremultiplied color to write
    /// - `subset` bounding integer rectangle of the pixels to write; may be `None`
    // Port of: src/core/SkPixmap.cpp#L750-L811 (chrome/m156)
    #[doc(alias = "erase")]
    #[allow(clippy::cast_sign_loss)] // mirrors the C++ int -> size_t conversions; x, y are in bounds
    pub fn erase_4f(&mut self, color: impl AsRef<Color4f>, subset: Option<&IRect>) -> bool {
        if self.color_type() == ColorType::Unknown {
            return false;
        }

        let mut clip = self.bounds();
        if let Some(subset) = subset {
            match IRect::intersect(&clip, subset) {
                Some(r) => clip = r,
                None => return false, // is this check really needed (i.e. to return false in this case?)
            }
        }
        if !self.is_writable() {
            return false;
        }

        // Erase is meant to simulate drawing in kSRC mode -- which means we have to convert out
        // unpremul input into premul (which we always do when we draw).
        let c: PMColor4f = color.as_ref().premul();

        let dst = ImageInfo::new(
            (1, 1),
            self.color_type(),
            self.alpha_type(),
            self.color_space(),
        );
        let src = ImageInfo::new((1, 1), ColorType::RGBAF32, AlphaType::Premul, None);

        let mut dst_pixel = [0u8; 16]; // be large enough for our widest config (F32 x 4)
        debug_assert!(dst.bytes_per_pixel() <= dst_pixel.len());

        let mut src_pixel = [0u8; 16];
        for (i, v) in [c.r, c.g, c.b, c.a].into_iter().enumerate() {
            src_pixel[4 * i..4 * i + 4].copy_from_slice(&v.to_ne_bytes());
        }

        if !convert_pixels(&dst, &mut dst_pixel, 16, &src, &src_pixel, 16) {
            return false;
        }
        let bpp = dst.bytes_per_pixel();

        // The C++ memsets each row with the converted pixel (as the 8/16/32/64-bit value of the
        // first `bpp` bytes of `dstPixel`, or as an RGBA_F32 color); that is this byte copy.
        let row_bytes = self.row_bytes;
        let shift = self.shift_per_pixel();
        let left = clip.left as usize;
        let width = clip.width() as usize;
        let bytes = self.px_mut();
        for y in clip.top..clip.bottom {
            let start = (y as usize) * row_bytes + (left << shift);
            for x in 0..width {
                let at = start + x * bpp;
                bytes[at..at + bpp].copy_from_slice(&dst_pixel[..bpp]);
            }
        }
        true
    }
}

// `from_half(skvx::half4::Load(addr))` on the four halfs at byte offset `off`.
fn half4_to_float4(bytes: &[u8], off: usize) -> [f32; 4] {
    [
        half_to_float(get_u16(bytes, off)),
        half_to_float(get_u16(bytes, off + 2)),
        half_to_float(get_u16(bytes, off + 4)),
        half_to_float(get_u16(bytes, off + 6)),
    ]
}

// `p4 * skvx::float4(a, b, c, d)`
fn mul4(p: [f32; 4], q: [f32; 4]) -> [f32; 4] {
    [p[0] * q[0], p[1] * q[1], p[2] * q[2], p[3] * q[3]]
}
