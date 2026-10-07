// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkReadPixelsRec.h, src/core/SkReadPixelsRec.cpp

//! [`ReadPixelsRec`]: packages and trims the parameters passed to `readPixels()`.
//!
//! skia-rust: `fPixels` is a pointer that `trim` advances. Here the pixels are a slice that stays
//! as given, plus the byte [`offset`](ReadPixelsRec::offset) of `fPixels` from its start, so that
//! `trim` never forms an out-of-range address.

use crate::image_info::ImageInfo;
use crate::pixmap::Pixmap;
use crate::rect::IRect;
use crate::safe_math::SafeMath;

/// Helper to package and trim the parameters passed to `readPixels()`.
// Port of: src/core/SkReadPixelsRec.h#L19-L55 (chrome/m156)
#[doc(alias = "SkReadPixelsRec")]
#[derive(Debug)]
pub struct ReadPixelsRec<'a> {
    /// The destination pixels (`fPixels`, before the offset), or `None` for a null pointer.
    pub pixels: Option<&'a mut [u8]>,
    /// The byte offset of `fPixels` from the start of [`Self::pixels`]; `trim` advances it.
    pub offset: usize,
    pub row_bytes: usize,
    pub info: ImageInfo,
    pub x: i32,
    pub y: i32,
}

impl<'a> ReadPixelsRec<'a> {
    /// `x` and `y` are the offset into the source. Negative values are supported; the portion of
    /// the rectangle that is "off-screen" (negative x or y) will leave the corresponding area in
    /// the destination pixels untouched.
    // Port of: src/core/SkReadPixelsRec.h#L25-L32 (chrome/m156)
    #[must_use]
    pub fn new(
        info: &ImageInfo,
        pixels: Option<&'a mut [u8]>,
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

    /// Reads into the (writable) pixels of `pm`.
    // Port of: src/core/SkReadPixelsRec.h#L34-L40 (chrome/m156)
    #[doc(alias = "SkReadPixelsRec")]
    #[must_use]
    pub fn from_pixmap(pm: &'a mut Pixmap<'_>, x: i32, y: i32) -> Self {
        let row_bytes = pm.row_bytes();
        let info = pm.info().clone();
        Self {
            pixels: pm.bytes_mut(),
            offset: 0,
            row_bytes,
            info,
            x,
            y,
        }
    }

    /// On true, may have modified its fields (except `row_bytes`) to make it a legal subset of
    /// the specified src width/height. Negative `x` or `y` will cause the pixel offset to be
    /// incremented and `info` to be reduced to account for the portion that is "off-screen".
    ///
    /// On false, leaves self unchanged, but indicates that it does not overlap src, or is not
    /// valid (e.g. bad `info`) for `readPixels()`.
    // Port of: src/core/SkReadPixelsRec.cpp#L12-L55 (chrome/m156)
    #[must_use]
    pub fn trim(&mut self, src_width: i32, src_height: i32) -> bool {
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
        let src_r = IRect::from_xywh(x, y, self.info.width(), self.info.height());
        let Some(src_r) = IRect::intersect(&src_r, &IRect::new(0, 0, src_width, src_height)) else {
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
        self.info = self.info.with_dimensions(src_r.size());
        self.x = src_r.left;
        self.y = src_r.top;

        true
    }
}
