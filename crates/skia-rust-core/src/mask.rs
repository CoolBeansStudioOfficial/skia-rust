// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkMask.h, src/core/SkMask.cpp

//! Alpha bitmaps (`SkMask`): 1-bit, 8-bit, 3D, ARGB32, LCD16 and SDF coverage masks.
//!
//! [`Mask`] borrows its image; [`MaskBuilder`] owns it. Skia's `getAddr*` pointer accessors
//! become byte-slice accessors that start at the addressed element (indexing is bounds-checked),
//! plus value accessors for the 16- and 32-bit formats.
//!
//! `SkMask::AlphaIter` and `SkAutoMaskFreeImage` are not ported: the former is a raw-pointer
//! iterator used only by the blur code, the latter is subsumed by `Vec` ownership.

// The accessors panic exactly where Skia's debug asserts fire (wrong format, out-of-bounds
// coordinates); each says so in its summary.
#![allow(clippy::missing_panics_doc)]

use crate::point::IPoint;
use crate::rect::IRect;
use crate::safe_math::SafeMath;

/// The pixel format of a [`Mask`].
// Port of: src/core/SkMask.h#L27-L34 (chrome/m156)
#[doc(alias = "SkMask::Format")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum MaskFormat {
    /// 1 bit per pixel mask (e.g. monochrome).
    #[doc(alias = "kBW_Format")]
    BW,
    /// 8 bits per pixel mask (e.g. antialiasing).
    #[doc(alias = "kA8_Format")]
    A8,
    /// 3 8-bit per pixel planes: alpha, mul, add.
    #[doc(alias = "k3D_Format")]
    ThreeD,
    /// `SkPMColor`.
    #[doc(alias = "kARGB32_Format")]
    Argb32,
    /// 565 alpha for r/g/b.
    #[doc(alias = "kLCD16_Format")]
    Lcd16,
    /// 8 bits representing a signed distance field.
    #[doc(alias = "kSDF_Format")]
    Sdf,
}

/// `SkMask::kCountMaskFormats`.
pub const COUNT_MASK_FORMATS: u8 = MaskFormat::Sdf as u8 + 1;

impl MaskFormat {
    /// `SkMask::IsValidFormat`.
    // Port of: src/core/SkMask.h#L47 (chrome/m156)
    #[doc(alias = "IsValidFormat")]
    #[must_use]
    pub fn is_valid_format(format: u8) -> bool {
        format < COUNT_MASK_FORMATS
    }

    // Port of: src/core/SkMask.cpp#L82-L89 (chrome/m156)
    // `gMaskFormatToShift`; `None` for BW (not supported).
    fn shift(self) -> Option<u32> {
        match self {
            MaskFormat::BW => None,
            MaskFormat::A8 | MaskFormat::ThreeD | MaskFormat::Sdf => Some(0),
            MaskFormat::Argb32 => Some(2),
            MaskFormat::Lcd16 => Some(1),
        }
    }
}

/// Returns the product if it is positive and fits in 31 bits. Otherwise this returns 0.
// Port of: src/core/SkMask.cpp#L19-L25 (chrome/m156)
fn safe_mul32(a: i32, b: i32) -> i32 {
    let size = i64::from(a) * i64::from(b);
    if size > 0
        && let Ok(s) = i32::try_from(size)
    {
        return s;
    }
    0
}

// Shared geometry of `Mask` and `MaskBuilder` (the `SkMask` members).
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
fn compute_image_size(bounds: &IRect, row_bytes: u32) -> usize {
    // fRowBytes (uint32_t) is converted to the int32_t parameter of safeMul32; the result is
    // non-negative.
    safe_mul32(bounds.height(), row_bytes as i32) as usize
}

#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn compute_total_image_size(bounds: &IRect, row_bytes: u32, format: MaskFormat) -> usize {
    let size = compute_image_size(bounds, row_bytes);
    if format == MaskFormat::ThreeD {
        // SkToS32(size) is always in range: safeMul32 returns a non-negative int32.
        return safe_mul32(size as i32, 3) as usize;
    }
    size
}

fn bounds_contains(b: &IRect, x: i32, y: i32) -> bool {
    x >= b.left && x < b.right && y >= b.top && y < b.bottom
}

// Byte offset of the (x, y) element (non-BW formats). Panics (the debug asserts of
// `SkMask::getAddr`) if the format is BW or (x, y) is outside `bounds`.
fn addr_offset(bounds: &IRect, row_bytes: u32, format: MaskFormat, x: i32, y: i32) -> usize {
    assert!(
        bounds_contains(bounds, x, y),
        "({x}, {y}) is outside {bounds:?}"
    );
    let shift = format
        .shift()
        .expect("getAddr must not be called with BW format");
    let row = i64::from(y - bounds.top) * i64::from(row_bytes);
    let col = i64::from(x - bounds.left) << shift;
    usize::try_from(row + col).expect("mask offset")
}

fn addr1_offset(bounds: &IRect, row_bytes: u32, x: i32, y: i32) -> usize {
    assert!(
        bounds_contains(bounds, x, y),
        "({x}, {y}) is outside {bounds:?}"
    );
    let off = i64::from((x - bounds.left) >> 3) + i64::from(y - bounds.top) * i64::from(row_bytes);
    usize::try_from(off).expect("mask offset")
}

/// Describes alpha bitmaps, either 1-bit, 8-bit, or the 3-channel 3D format. These are passed to
/// mask filters. The image is borrowed; an empty slice stands for Skia's `nullptr` image.
// Port of: src/core/SkMask.h#L22-L141 (chrome/m156)
#[doc(alias = "SkMask")]
#[derive(Copy, Clone, Debug)]
pub struct Mask<'a> {
    /// `fImage`.
    pub image: &'a [u8],
    /// `fBounds`.
    pub bounds: IRect,
    /// `fRowBytes`.
    pub row_bytes: u32,
    /// `fFormat`.
    pub format: MaskFormat,
}

impl<'a> Mask<'a> {
    /// Constructs a mask over `image`.
    #[must_use]
    pub fn new(image: &'a [u8], bounds: IRect, row_bytes: u32, format: MaskFormat) -> Self {
        Mask {
            image,
            bounds,
            row_bytes,
            format,
        }
    }

    /// Returns true if the mask is empty: i.e. it has an empty bounds.
    #[doc(alias = "isEmpty")]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bounds.is_empty()
    }

    /// Returns the byte size of the mask, assuming only 1 plane. Does not account for
    /// [`MaskFormat::ThreeD`]; use [`Self::compute_total_image_size`]. Returns 0 on 32-bit
    /// overflow.
    // Port of: src/core/SkMask.cpp#L27-L29 (chrome/m156)
    #[doc(alias = "computeImageSize")]
    #[must_use]
    pub fn compute_image_size(&self) -> usize {
        compute_image_size(&self.bounds, self.row_bytes)
    }

    /// Returns the byte size of the mask, taking into account any extra planes (e.g.
    /// [`MaskFormat::ThreeD`]). Returns 0 on 32-bit overflow.
    // Port of: src/core/SkMask.cpp#L31-L37 (chrome/m156)
    #[doc(alias = "computeTotalImageSize")]
    #[must_use]
    pub fn compute_total_image_size(&self) -> usize {
        compute_total_image_size(&self.bounds, self.row_bytes, self.format)
    }

    /// The bytes starting at the byte that holds the bit for `(x, y)`. Panics unless the mask is
    /// [`MaskFormat::BW`] and `(x, y)` is in range. `(x, y)` is in the space of `bounds`.
    // Port of: src/core/SkMask.h#L70-L75 (chrome/m156)
    #[doc(alias = "getAddr1")]
    #[must_use]
    pub fn get_addr1(&self, x: i32, y: i32) -> &'a [u8] {
        assert_eq!(MaskFormat::BW, self.format);
        &self.image[addr1_offset(&self.bounds, self.row_bytes, x, y)..]
    }

    /// The bytes starting at the byte for `(x, y)`. Panics unless the mask is
    /// [`MaskFormat::A8`] or [`MaskFormat::Sdf`] and `(x, y)` is in range.
    // Port of: src/core/SkMask.h#L82-L88 (chrome/m156)
    #[doc(alias = "getAddr8")]
    #[must_use]
    pub fn get_addr8(&self, x: i32, y: i32) -> &'a [u8] {
        assert!(matches!(self.format, MaskFormat::A8 | MaskFormat::Sdf));
        &self.image[addr_offset(&self.bounds, self.row_bytes, self.format, x, y)..]
    }

    /// The bytes starting at the 16-bit element for `(x, y)`. Panics unless the mask is
    /// [`MaskFormat::Lcd16`] and `(x, y)` is in range.
    // Port of: src/core/SkMask.h#L96-L103 (chrome/m156)
    #[doc(alias = "getAddrLCD16")]
    #[must_use]
    pub fn get_addr_lcd16(&self, x: i32, y: i32) -> &'a [u8] {
        assert_eq!(MaskFormat::Lcd16, self.format);
        &self.image[addr_offset(&self.bounds, self.row_bytes, self.format, x, y)..]
    }

    /// The bytes starting at the 32-bit element for `(x, y)`. Panics unless the mask is
    /// [`MaskFormat::Argb32`] and `(x, y)` is in range.
    // Port of: src/core/SkMask.h#L111-L118 (chrome/m156)
    #[doc(alias = "getAddr32")]
    #[must_use]
    pub fn get_addr32(&self, x: i32, y: i32) -> &'a [u8] {
        assert_eq!(MaskFormat::Argb32, self.format);
        &self.image[addr_offset(&self.bounds, self.row_bytes, self.format, x, y)..]
    }

    /// The bytes starting at the pixel `(x, y)`, with the pixel size computed from the format.
    /// Panics for [`MaskFormat::BW`] or when `(x, y)` is out of bounds.
    // Port of: src/core/SkMask.cpp#L102-L112 (chrome/m156)
    #[doc(alias = "getAddr")]
    #[must_use]
    pub fn get_addr(&self, x: i32, y: i32) -> &'a [u8] {
        &self.image[addr_offset(&self.bounds, self.row_bytes, self.format, x, y)..]
    }

    /// The 16-bit LCD value at `(x, y)` (native endian), the value `*getAddrLCD16(x, y)`.
    #[must_use]
    pub fn lcd16(&self, x: i32, y: i32) -> u16 {
        let a = self.get_addr_lcd16(x, y);
        u16::from_ne_bytes([a[0], a[1]])
    }

    /// The 32-bit pixel at `(x, y)` (native endian), the value `*getAddr32(x, y)`.
    #[must_use]
    pub fn argb32(&self, x: i32, y: i32) -> u32 {
        let a = self.get_addr32(x, y);
        u32::from_ne_bytes([a[0], a[1], a[2], a[3]])
    }
}

/// How [`MaskBuilder::alloc_image`] initializes memory.
// Port of: src/core/SkMask.h#L232-L235 (chrome/m156)
#[doc(alias = "SkMaskBuilder::AllocType")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum AllocType {
    /// `kUninit_Alloc` (zero-filled here: safe Rust has no uninitialized memory).
    Uninit,
    /// `kZeroInit_Alloc`.
    ZeroInit,
}

/// `SkMaskBuilder::CreateMode`.
// Port of: src/core/SkMask.h#L242-L246 (chrome/m156)
#[doc(alias = "SkMaskBuilder::CreateMode")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CreateMode {
    /// Compute bounds and return.
    JustComputeBounds,
    /// Render into preallocated mask.
    JustRenderImage,
    /// Compute bounds, alloc image and render into it.
    ComputeBoundsAndRenderImage,
}

/// A [`Mask`] that owns (and can write) its image.
// Port of: src/core/SkMask.h#L192-L260 (chrome/m156)
#[doc(alias = "SkMaskBuilder")]
#[derive(Clone, Debug)]
pub struct MaskBuilder {
    /// `fImage` (empty for Skia's `nullptr`).
    pub image: Vec<u8>,
    /// `fBounds`.
    pub bounds: IRect,
    /// `fRowBytes`.
    pub row_bytes: u32,
    /// `fFormat`.
    pub format: MaskFormat,
}

impl Default for MaskBuilder {
    /// An empty BW mask with no image.
    fn default() -> Self {
        MaskBuilder {
            image: Vec::new(),
            bounds: IRect::new(0, 0, 0, 0),
            row_bytes: 0,
            format: MaskFormat::BW,
        }
    }
}

impl MaskBuilder {
    /// Constructs a builder owning `image`.
    #[must_use]
    pub fn new(image: Vec<u8>, bounds: IRect, row_bytes: u32, format: MaskFormat) -> Self {
        MaskBuilder {
            image,
            bounds,
            row_bytes,
            format,
        }
    }

    /// Borrows this builder as an immutable [`Mask`].
    #[must_use]
    pub fn as_mask(&self) -> Mask<'_> {
        Mask {
            image: &self.image,
            bounds: self.bounds,
            row_bytes: self.row_bytes,
            format: self.format,
        }
    }

    /// See [`Mask::is_empty`].
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bounds.is_empty()
    }

    /// See [`Mask::compute_image_size`].
    #[must_use]
    pub fn compute_image_size(&self) -> usize {
        compute_image_size(&self.bounds, self.row_bytes)
    }

    /// See [`Mask::compute_total_image_size`].
    #[must_use]
    pub fn compute_total_image_size(&self) -> usize {
        compute_total_image_size(&self.bounds, self.row_bytes, self.format)
    }

    /// Mutable [`Mask::get_addr1`].
    // Port of: src/core/SkMask.h#L216-L218 (chrome/m156)
    pub fn get_addr1(&mut self, x: i32, y: i32) -> &mut [u8] {
        assert_eq!(MaskFormat::BW, self.format);
        let off = addr1_offset(&self.bounds, self.row_bytes, x, y);
        &mut self.image[off..]
    }

    /// Mutable [`Mask::get_addr8`].
    // Port of: src/core/SkMask.h#L225-L227 (chrome/m156)
    pub fn get_addr8(&mut self, x: i32, y: i32) -> &mut [u8] {
        assert!(matches!(self.format, MaskFormat::A8 | MaskFormat::Sdf));
        let off = addr_offset(&self.bounds, self.row_bytes, self.format, x, y);
        &mut self.image[off..]
    }

    /// Mutable [`Mask::get_addr_lcd16`].
    // Port of: src/core/SkMask.h#L236-L238 (chrome/m156)
    pub fn get_addr_lcd16(&mut self, x: i32, y: i32) -> &mut [u8] {
        assert_eq!(MaskFormat::Lcd16, self.format);
        let off = addr_offset(&self.bounds, self.row_bytes, self.format, x, y);
        &mut self.image[off..]
    }

    /// Mutable [`Mask::get_addr32`].
    // Port of: src/core/SkMask.h#L245-L247 (chrome/m156)
    pub fn get_addr32(&mut self, x: i32, y: i32) -> &mut [u8] {
        assert_eq!(MaskFormat::Argb32, self.format);
        let off = addr_offset(&self.bounds, self.row_bytes, self.format, x, y);
        &mut self.image[off..]
    }

    /// Mutable [`Mask::get_addr`].
    // Port of: src/core/SkMask.h#L260-L262 (chrome/m156)
    pub fn get_addr(&mut self, x: i32, y: i32) -> &mut [u8] {
        let off = addr_offset(&self.bounds, self.row_bytes, self.format, x, y);
        &mut self.image[off..]
    }

    /// Allocates an image of `size` bytes rounded up to a multiple of 4.
    // Port of: src/core/SkMask.cpp#L40-L48 (chrome/m156)
    #[doc(alias = "AllocImage")]
    #[must_use]
    pub fn alloc_image(size: usize, _alloc_type: AllocType) -> Vec<u8> {
        let aligned_size = SafeMath::align4(size);
        vec![0; aligned_size]
    }

    /// Returns initial destination mask data padded by `radius_x` and `radius_y`.
    // Port of: src/core/SkMask.cpp#L57-L86 (chrome/m156)
    #[doc(alias = "PrepareDestination")]
    #[must_use]
    // The int -> size_t conversions (sign-extending) mirror the C++ implicit conversions that
    // SkSafeMath is then asked to check.
    #[allow(clippy::cast_sign_loss)]
    pub fn prepare_destination(radius_x: i32, radius_y: i32, src: &Mask<'_>) -> MaskBuilder {
        let mut safe = SafeMath::new();

        let mut dst = MaskBuilder {
            format: MaskFormat::A8,
            ..MaskBuilder::default()
        };

        // dstW = srcW + 2 * radiusX;
        let rx2 = safe.add(radius_x as isize as usize, radius_x as isize as usize);
        let dst_w = safe.add(src.bounds.width() as isize as usize, rx2);
        // dstH = srcH + 2 * radiusY;
        let ry2 = safe.add(radius_y as isize as usize, radius_y as isize as usize);
        let dst_h = safe.add(src.bounds.height() as isize as usize, ry2);

        let to_alloc = safe.mul(dst_w, dst_h);

        // We can only deal with masks that fit in INT_MAX and sides that fit in int.
        let (Ok(w), Ok(h)) = (i32::try_from(dst_w), i32::try_from(dst_h)) else {
            dst.bounds.set_empty();
            dst.row_bytes = 0;
            return dst;
        };
        if i32::try_from(to_alloc).is_err() || !safe.ok() {
            dst.bounds.set_empty();
            dst.row_bytes = 0;
            return dst;
        }

        dst.bounds.set_wh(w, h);
        dst.bounds
            .offset(IPoint::new(src.bounds.x(), src.bounds.y()));
        dst.bounds.offset(IPoint::new(-radius_x, -radius_y));
        dst.row_bytes = u32::try_from(dst_w).expect("fits in i32");

        if !src.image.is_empty() {
            dst.image = MaskBuilder::alloc_image(to_alloc, AllocType::Uninit);
        }

        dst
    }
}
