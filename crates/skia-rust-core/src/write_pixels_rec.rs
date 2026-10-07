// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkWritePixelsRec.h, src/core/SkWritePixelsRec.cpp

//! [`WritePixelsRec`]: packages and trims the parameters passed to `writePixels()`.
//!
//! skia-rust: `fPixels` is a pointer that `trim` advances. Here the pixels are a slice that stays
//! as given, plus the byte [`offset`](WritePixelsRec::offset) of `fPixels` from its start, so
//! that `trim` never forms an out-of-range address.

use crate::image_info::ImageInfo;
use crate::pixmap::Pixmap;
use crate::rect::IRect;
use crate::safe_math::SafeMath;

/// Helper to package and trim the parameters passed to `writePixels()`.
// Port of: src/core/SkWritePixelsRec.h#L19-L55 (chrome/m156)
#[doc(alias = "SkWritePixelsRec")]
#[derive(Debug)]
pub struct WritePixelsRec<'a> {
    /// The source pixels (`fPixels`, before the offset), or `None` for a null pointer.
    pub pixels: Option<&'a [u8]>,
    /// The byte offset of `fPixels` from the start of [`Self::pixels`]; `trim` advances it.
    pub offset: usize,
    pub row_bytes: usize,
    pub info: ImageInfo,
    pub x: i32,
    pub y: i32,
}

impl<'a> WritePixelsRec<'a> {
    /// `x` and `y` are the offset into the destination. Negative values are supported; the
    /// portion of the rectangle that is "off-screen" (negative x or y) will cause the
    /// corresponding area in the source pixels to be ignored.
    // Port of: src/core/SkWritePixelsRec.h#L25-L32 (chrome/m156)
    #[must_use]
    pub fn new(
        info: &ImageInfo,
        pixels: Option<&'a [u8]>,
        row_bytes: usize,
        x: i32,
        y: i32,
    ) -> Self {
        Self {
            pixels,
            offset: 0,
            row_bytes,
            info: info.clone(),
            x,
            y,
        }
    }

    /// Writes from the pixels of `pm`.
    // Port of: src/core/SkWritePixelsRec.h#L34-L40 (chrome/m156)
    #[doc(alias = "SkWritePixelsRec")]
    #[must_use]
    pub fn from_pixmap(pm: &'a Pixmap<'_>, x: i32, y: i32) -> Self {
        Self {
            pixels: pm.addr(),
            offset: 0,
            row_bytes: pm.row_bytes(),
            info: pm.info().clone(),
            x,
            y,
        }
    }

    /// On true, may have modified its fields (except `row_bytes`) to make it a legal subset of
    /// the specified dst width/height. Negative `x` or `y` will cause the pixel offset to be
    /// incremented and `info` to be reduced to account for the portion that is "off-screen".
    ///
    /// On false, leaves self unchanged, but indicates that it does not overlap dst, or is not
    /// valid (e.g. bad `info`) for `writePixels()`.
    // Port of: src/core/SkWritePixelsRec.cpp#L12-L55 (chrome/m156)
    #[must_use]
    pub fn trim(&mut self, dst_width: i32, dst_height: i32) -> bool {
        // fInfo.minRowBytes() returns 0 if the size doesn't fit in `size_t`.
        let min_row_bytes = self.info.min_row_bytes();
        if self.pixels.is_none() || self.row_bytes < min_row_bytes || min_row_bytes == 0 {
            return false;
        }
        if 0 >= self.info.width() || 0 >= self.info.height() {
            return false;
        }
        // negating the largest negative integer (below) is UB
        if self.x == i32::MIN || self.y == i32::MIN {
            return false;
        }

        let mut x = self.x;
        let mut y = self.y;
        let dst_r = IRect::from_xywh(x, y, self.info.width(), self.info.height());
        let Some(dst_r) = IRect::intersect(&dst_r, &IRect::new(0, 0, dst_width, dst_height)) else {
            return false;
        };

        // if x or y are negative, then we have to adjust pixels
        if x > 0 {
            x = 0;
        }
        if y > 0 {
            y = 0;
        }
        // here x,y are either 0 or negative (safe to cast to size_t)
        // we negate and add them so UBSAN (pointer-overflow) doesn't get confused.
        let mut safe_math = SafeMath::new();
        #[allow(clippy::cast_sign_loss)] // -x and -y are non-negative here
        let y_offset = safe_math.mul((-y) as usize, self.row_bytes);
        #[allow(clippy::cast_sign_loss)] // -x and -y are non-negative here
        let x_offset = safe_math.mul((-x) as usize, self.info.bytes_per_pixel());
        let total = safe_math.add(y_offset, x_offset);
        if !safe_math.ok() {
            return false;
        }

        self.offset = self.offset.wrapping_add(total);
        // the intersect may have shrunk info's logical size
        self.info = self.info.with_dimensions(dst_r.size());
        self.x = dst_r.left;
        self.y = dst_r.top;

        true
    }
}
