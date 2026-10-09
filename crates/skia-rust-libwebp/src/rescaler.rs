// Copyright 2014 Google Inc. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license
// that can be found in the COPYING file in the root of the source
// tree. An additional intellectual property rights grant can be found
// in the file PATENTS. All contributing project authors may
// be found in the AUTHORS file in the root of the source tree.
//
// Port by The skia-rust Authors.

//! Port of libwebp's rescaler: `src/utils/rescaler_utils.c` (`WebPRescalerInit`,
//! `WebPRescalerGetScaledDimensions`, `WebPRescaleNeededLines`, `WebPRescalerImport`,
//! `WebPRescalerExport`) and the portable C row functions of `src/dsp/rescaler.c`
//! (`WebPRescalerImportRowExpand_C`, `WebPRescalerImportRowShrink_C`,
//! `WebPRescalerExportRowExpand_C`, `WebPRescalerExportRowShrink_C`, and the dispatching
//! `WebPRescalerImportRow`/`WebPRescalerExportRow`).
//!
//! The SSE2 rescaler kernels are not ported: `oracle/codec-diff/libwebp` is built without SIMD
//! and the portable C path is the reference (`docs/design/codecs.md` §4).
//!
//! Arithmetic is the C arithmetic: `rescaler_t` is `uint32_t`, so the products and differences
//! wrap, and `(int)` conversions of 64-bit intermediates keep the low 32 bits.

// Module-level clippy allows. Each one mirrors the C source of this module.
// clippy::cast_possible_truncation, clippy::cast_possible_wrap, clippy::cast_sign_loss: the
// C code mixes `int`, `uint32_t` and `uint64_t` freely, and the conversions are written as the
// same casts.
// clippy::many_single_char_names: `a` and `b` are the C `A` and `B` weights of the expand export.
// clippy::manual_div_ceil: the C code writes the rounding up as `(x + y - 1) / y`, which
// `div_ceil` computes for these non-negative operands.
// clippy::needless_range_loop: the loops index the work buffer and the row at several offsets, as
// the C loops do.
// clippy::similar_names: local names mirror the C identifiers (`irow`, `frow`, `x_in`, `x_out`).
// clippy::struct_excessive_bools: `Rescaler` holds the C struct's `x_expand`/`y_expand` flags.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::manual_div_ceil,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::struct_excessive_bools
)]

/// Port of `WEBP_RESCALER_RFIX`: fixed-point precision of the multiplies.
pub const WEBP_RESCALER_RFIX: u32 = 32;
/// Port of `WEBP_RESCALER_ONE`.
const WEBP_RESCALER_ONE: u64 = 1u64 << WEBP_RESCALER_RFIX;
/// Port of `ROUNDER` (`WEBP_RESCALER_ONE >> 1`).
const ROUNDER: u64 = WEBP_RESCALER_ONE >> 1;

/// Port of `WEBP_RESCALER_FRAC(x, y)`: `((uint32_t)(((uint64_t)(x) << RFIX) / (y)))`.
#[inline]
fn frac(x: u64, y: u64) -> u32 {
    ((x << WEBP_RESCALER_RFIX) / y) as u32
}

/// Port of `MULT_FIX(x, y)`: `((uint64_t)(x) * (y) + ROUNDER) >> RFIX`.
#[inline]
fn mult_fix(x: u32, y: u32) -> u64 {
    (u64::from(x) * u64::from(y) + ROUNDER) >> WEBP_RESCALER_RFIX
}

/// Port of `MULT_FIX_FLOOR(x, y)`: `((uint64_t)(x) * (y)) >> RFIX`.
#[inline]
fn mult_fix_floor(x: u32, y: u32) -> u64 {
    (u64::from(x) * u64::from(y)) >> WEBP_RESCALER_RFIX
}

/// Port of the `(v > 255) ? 255u : (uint8_t)v` clamp of the export functions.
#[inline]
fn clip_byte(v: i32) -> u8 {
    if v > 255 { 255 } else { v as u8 }
}

/// Port of `struct WebPRescaler`. The output rows are not owned: the export functions write
/// into a row slice supplied by the caller, as the C code writes into `dst`.
#[derive(Debug, Clone)]
#[doc(alias = "WebPRescaler")]
pub struct Rescaler {
    pub x_expand: bool,
    pub y_expand: bool,
    pub num_channels: i32,
    pub fx_scale: u32,
    pub fy_scale: u32,
    pub fxy_scale: u32,
    pub y_accum: i32,
    pub y_add: i32,
    pub y_sub: i32,
    pub x_add: i32,
    pub x_sub: i32,
    pub src_width: i32,
    pub src_height: i32,
    pub dst_width: i32,
    pub dst_height: i32,
    pub src_y: i32,
    pub dst_y: i32,
    /// `irow` and `frow` are offsets into `work`; the import swaps them for expansion.
    irow: usize,
    frow: usize,
    work: Vec<u32>,
}

/// Port of `WebPRescalerInit`. Returns `None` where the C function returns 0 (size overflow).
#[doc(alias = "WebPRescalerInit")]
#[must_use]
pub fn rescaler_init(
    src_width: i32,
    src_height: i32,
    dst_width: i32,
    dst_height: i32,
    num_channels: i32,
) -> Option<Rescaler> {
    let x_add = src_width;
    let x_sub = dst_width;
    let y_add = src_height;
    let y_sub = dst_height;
    // total_size = 2 * dst_width * num_channels * sizeof(rescaler_t), checked as in
    // CheckSizeOverflow (the work buffer must be addressable).
    let total_words = 2u64 * dst_width as u64 * num_channels as u64;
    if total_words.checked_mul(4)? > (i32::MAX as u64) {
        return None;
    }
    let x_expand = src_width < dst_width;
    let y_expand = src_height < dst_height;
    let mut r = Rescaler {
        x_expand,
        y_expand,
        num_channels,
        fx_scale: 0,
        fy_scale: 0,
        fxy_scale: 0,
        y_accum: 0,
        y_add: 0,
        y_sub: 0,
        x_add: 0,
        x_sub: 0,
        src_width,
        src_height,
        dst_width,
        dst_height,
        src_y: 0,
        dst_y: 0,
        irow: 0,
        frow: 0,
        work: vec![0u32; total_words as usize],
    };
    // for 'x_expand', we use bilinear interpolation
    r.x_add = if x_expand { x_sub - 1 } else { x_add };
    r.x_sub = if x_expand { x_add - 1 } else { x_sub };
    if !x_expand {
        // fx_scale is not used otherwise
        r.fx_scale = frac(1, r.x_sub as u64);
    }
    // vertical scaling parameters
    r.y_add = if y_expand { y_add - 1 } else { y_add };
    r.y_sub = if y_expand { y_sub - 1 } else { y_sub };
    r.y_accum = if y_expand { r.y_sub } else { r.y_add };
    if y_expand {
        r.fy_scale = frac(1, r.x_add as u64);
        // rescaler->fxy_scale is unused here.
    } else {
        // This is WEBP_RESCALER_FRAC(dst_height, x_add * y_add) without the cast.
        let num = dst_height as u64 * WEBP_RESCALER_ONE;
        let den = r.x_add as u64 * r.y_add as u64;
        let ratio = num / den;
        // When the ratio does not fit 32 bits, fxy_scale is 0 and WebPRescalerExportRow takes
        // its special case.
        r.fxy_scale = if ratio > u64::from(u32::MAX) {
            0
        } else {
            ratio as u32
        };
        r.fy_scale = frac(1, r.y_sub as u64);
    }
    r.irow = 0;
    r.frow = (num_channels * dst_width) as usize;
    Some(r)
}

/// Port of `WebPRescalerGetScaledDimensions`: fills in a zero width or height proportionally.
#[doc(alias = "WebPRescalerGetScaledDimensions")]
#[must_use]
pub fn get_scaled_dimensions(
    src_width: i32,
    src_height: i32,
    scaled_width: &mut i32,
    scaled_height: &mut i32,
) -> bool {
    let mut width = *scaled_width;
    let mut height = *scaled_height;
    let max_size = i32::MAX / 2;
    // if width is unspecified, scale original proportionally to height ratio.
    if width == 0 && src_height > 0 {
        width = (src_width as u64 * height as u64).div_ceil(src_height as u64) as i32;
    }
    // if height is unspecified, scale original proportionally to width ratio.
    if height == 0 && src_width > 0 {
        height = (src_height as u64 * width as u64).div_ceil(src_width as u64) as i32;
    }
    // Check if the overall dimensions still make sense.
    if width <= 0 || height <= 0 || width > max_size || height > max_size {
        return false;
    }
    *scaled_width = width;
    *scaled_height = height;
    true
}

impl Rescaler {
    /// Port of `WebPRescalerInputDone`.
    #[must_use]
    pub fn input_done(&self) -> bool {
        self.src_y >= self.src_height
    }

    /// Port of `WebPRescalerOutputDone`.
    #[must_use]
    pub fn output_done(&self) -> bool {
        self.dst_y >= self.dst_height
    }

    /// Port of `WebPRescalerHasPendingOutput`.
    #[must_use]
    pub fn has_pending_output(&self) -> bool {
        !self.output_done() && self.y_accum <= 0
    }

    /// Port of `WebPRescaleNeededLines`.
    #[must_use]
    pub fn needed_lines(&self, max_num_lines: i32) -> i32 {
        let num_lines = (self.y_accum + self.y_sub - 1) / self.y_sub;
        if num_lines > max_num_lines {
            max_num_lines
        } else {
            num_lines
        }
    }

    /// Port of `WebPRescalerImportRowExpand_C`: bilinear expansion of one source row
    /// (`src` starts at the row's first sample).
    #[doc(alias = "WebPRescalerImportRowExpand_C")]
    fn import_row_expand(&mut self, src: &[u8]) {
        let x_stride = self.num_channels as usize;
        let x_out_max = (self.dst_width * self.num_channels) as usize;
        let x_add = self.x_add as u32;
        let x_sub = self.x_sub;
        for channel in 0..x_stride {
            let mut x_in = channel;
            let mut x_out = channel;
            // simple bilinear interpolation
            let mut accum = self.x_add;
            let mut left = u32::from(src[x_in]);
            let mut right = if self.src_width > 1 {
                u32::from(src[x_in + x_stride])
            } else {
                left
            };
            x_in += x_stride;
            loop {
                self.work[self.frow + x_out] = right
                    .wrapping_mul(x_add)
                    .wrapping_add(left.wrapping_sub(right).wrapping_mul(accum as u32));
                x_out += x_stride;
                if x_out >= x_out_max {
                    break;
                }
                accum -= x_sub;
                if accum < 0 {
                    left = right;
                    x_in += x_stride;
                    right = u32::from(src[x_in]);
                    accum += self.x_add;
                }
            }
        }
    }

    /// Port of `WebPRescalerImportRowShrink_C`: box-filter reduction of one source row.
    #[doc(alias = "WebPRescalerImportRowShrink_C")]
    fn import_row_shrink(&mut self, src: &[u8]) {
        let x_stride = self.num_channels as usize;
        let x_out_max = (self.dst_width * self.num_channels) as usize;
        for channel in 0..x_stride {
            let mut x_in = channel;
            let mut x_out = channel;
            let mut sum: u32 = 0;
            let mut accum: i32 = 0;
            while x_out < x_out_max {
                let mut base: u32 = 0;
                accum += self.x_add;
                while accum > 0 {
                    accum -= self.x_sub;
                    base = u32::from(src[x_in]);
                    sum = sum.wrapping_add(base);
                    x_in += x_stride;
                }
                {
                    // Emit next horizontal pixel.
                    let frac = base.wrapping_mul((-accum) as u32);
                    self.work[self.frow + x_out] =
                        sum.wrapping_mul(self.x_sub as u32).wrapping_sub(frac);
                    // fresh fractional start for next pixel
                    sum = mult_fix(frac, self.fx_scale) as u32;
                }
                x_out += x_stride;
            }
        }
    }

    /// Port of `WebPRescalerImportRow`: imports one source row into `frow`.
    #[doc(alias = "WebPRescalerImportRow")]
    fn import_row(&mut self, src: &[u8]) {
        if self.x_expand {
            self.import_row_expand(src);
        } else {
            self.import_row_shrink(src);
        }
    }

    /// Port of `WebPRescalerExportRowExpand_C`: writes one output row to `dst`.
    #[doc(alias = "WebPRescalerExportRowExpand_C")]
    fn export_row_expand(&mut self, dst: &mut [u8]) {
        let x_out_max = (self.dst_width * self.num_channels) as usize;
        if self.y_accum == 0 {
            for x_out in 0..x_out_max {
                let j = self.work[self.frow + x_out];
                let v = mult_fix(j, self.fy_scale) as i32;
                dst[x_out] = clip_byte(v);
            }
        } else {
            let b = frac((-self.y_accum) as u64, self.y_sub as u64);
            let a = (WEBP_RESCALER_ONE - u64::from(b)) as u32;
            for x_out in 0..x_out_max {
                let i = u64::from(a) * u64::from(self.work[self.frow + x_out])
                    + u64::from(b) * u64::from(self.work[self.irow + x_out]);
                let j = ((i + ROUNDER) >> WEBP_RESCALER_RFIX) as u32;
                let v = mult_fix(j, self.fy_scale) as i32;
                dst[x_out] = clip_byte(v);
            }
        }
    }

    /// Port of `WebPRescalerExportRowShrink_C`: writes one output row to `dst`.
    #[doc(alias = "WebPRescalerExportRowShrink_C")]
    fn export_row_shrink(&mut self, dst: &mut [u8]) {
        let x_out_max = (self.dst_width * self.num_channels) as usize;
        let yscale = self.fy_scale.wrapping_mul((-self.y_accum) as u32);
        if yscale != 0 {
            for x_out in 0..x_out_max {
                let frow_v = self.work[self.frow + x_out];
                let frac = mult_fix_floor(frow_v, yscale) as u32;
                let irow_v = self.work[self.irow + x_out];
                let v = mult_fix(irow_v.wrapping_sub(frac), self.fxy_scale) as i32;
                dst[x_out] = clip_byte(v);
                self.work[self.irow + x_out] = frac; // new fractional start
            }
        } else {
            for x_out in 0..x_out_max {
                let v = mult_fix(self.work[self.irow + x_out], self.fxy_scale) as i32;
                dst[x_out] = clip_byte(v);
                self.work[self.irow + x_out] = 0;
            }
        }
    }

    /// Port of `WebPRescalerExportRow`: emits one output row if one is due.
    #[doc(alias = "WebPRescalerExportRow")]
    pub fn export_row(&mut self, dst: &mut [u8]) {
        if self.y_accum <= 0 {
            if self.y_expand {
                self.export_row_expand(dst);
            } else if self.fxy_scale != 0 {
                self.export_row_shrink(dst);
            } else {
                // special case: src_height == dst_height and x_add == 1
                let n = (self.num_channels * self.dst_width) as usize;
                for i in 0..n {
                    dst[i] = self.work[self.irow + i] as u8;
                    self.work[self.irow + i] = 0;
                }
            }
            self.y_accum += self.y_add;
            self.dst_y += 1;
        }
    }

    /// Port of `WebPRescalerImport`: imports up to `num_lines` source rows starting at `src`,
    /// `src_stride` bytes apart, and returns how many were imported.
    #[doc(alias = "WebPRescalerImport")]
    pub fn import(&mut self, num_lines: i32, src: &[u8], src_stride: usize) -> i32 {
        let mut total_imported = 0;
        let mut off = 0usize;
        while total_imported < num_lines && !self.has_pending_output() {
            if self.y_expand {
                std::mem::swap(&mut self.irow, &mut self.frow);
            }
            self.import_row(&src[off..]);
            if !self.y_expand {
                // Accumulate the contribution of the new row.
                let n = (self.num_channels * self.dst_width) as usize;
                for x in 0..n {
                    self.work[self.irow + x] =
                        self.work[self.irow + x].wrapping_add(self.work[self.frow + x]);
                }
            }
            self.src_y += 1;
            off += src_stride;
            total_imported += 1;
            self.y_accum -= self.y_sub;
        }
        total_imported
    }

    /// Port of `WebPRescalerExport`: emits every pending output row into `dst`, which is
    /// written at `dst_stride` bytes per row (0 writes every row over the same slice, as the
    /// decoder does for its scratch rows). Returns the number of rows written.
    #[doc(alias = "WebPRescalerExport")]
    pub fn export(&mut self, dst: &mut [u8], dst_stride: usize) -> i32 {
        let mut total_exported = 0;
        let mut off = 0usize;
        while self.has_pending_output() {
            self.export_row(&mut dst[off..]);
            off += dst_stride;
            total_exported += 1;
        }
        total_exported
    }
}
