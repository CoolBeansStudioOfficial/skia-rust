// Copyright (C) 1998-2026 Glenn Randers-Pehrson and the libpng contributors.
// Copyright (C) 2026 The skia-rust Authors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
// Port of: pngwtran.c#L500-L574 (`png_do_write_transformations`), and the parts of pngtrans.c the
// write path calls: `png_do_swap` (pngtrans.c#L346-L370) and `png_do_strip_channel`
// (pngtrans.c#L522-L645). libpng 1.6.56, skia.googlesource.com/third_party/libpng@d5515b5b.

// Clippy: each module is a line-by-line port of libpng's C, whose integer casts, long
// functions, argument lists and error returns are kept as written so they can be compared
// with the C. The Port of links name the C source for each item.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::too_many_lines,
    clippy::cognitive_complexity
)]

use crate::rtran::RowInfo;
use crate::structs::{PngStruct, flag};

/// Port of `PNG_SWAP_BYTES` (pngpriv.h): the transform that swaps the bytes of 16-bit samples.
pub(crate) const PNG_SWAP_BYTES: u32 = 0x0010;
/// Port of `PNG_FILLER` (pngpriv.h): the transform that adds or strips a filler channel.
pub(crate) const PNG_FILLER: u32 = 0x8000;

impl PngStruct {
    /// Port of `png_do_write_transformations` (pngwtran.c#L500-L574). Only the transforms the
    /// write API can set are reached: the filler and the byte swap.
    #[doc(alias = "png_do_write_transformations")]
    pub(crate) fn do_write_transformations(&mut self, row_info: &mut RowInfo, row: &mut [u8]) {
        if self.transformations & PNG_FILLER != 0 {
            let at_start = self.flags & flag::FILLER_AFTER == 0;
            do_strip_channel(row_info, row, at_start);
        }
        if self.transformations & PNG_SWAP_BYTES != 0 {
            do_swap(row_info, row);
        }
    }
}

/// Port of `png_do_swap` (pngtrans.c#L346-L370): swaps the two bytes of each 16-bit sample.
#[doc(alias = "png_do_swap")]
pub(crate) fn do_swap(row_info: &RowInfo, row: &mut [u8]) {
    if row_info.bit_depth == 16 {
        let istop = row_info.width as usize * usize::from(row_info.channels);
        for i in 0..istop {
            row.swap(2 * i, 2 * i + 1);
        }
    }
}

/// Port of `png_do_strip_channel` (pngtrans.c#L522-L645): removes the first channel (`at_start`
/// is false) or the last one (`at_start` is true) from each pixel, and fixes `pixel_depth`,
/// `channels`, `color_type` and `rowbytes` in `row_info`. The routine is not general: the filler
/// must be the first or the last channel.
#[doc(alias = "png_do_strip_channel")]
pub(crate) fn do_strip_channel(row_info: &mut RowInfo, row: &mut [u8], at_start: bool) {
    let ep = row_info.rowbytes; // One beyond the end of the row.
    let mut sp = 0usize; // source pointer
    let mut dp = 0usize; // destination pointer

    // At the start sp will point to the first byte to copy and dp to where it is copied to. ep
    // always points just beyond the end of the row, so the loop simply copies (channels-1)
    // channels until sp reaches ep.
    if row_info.channels == 2 {
        // GA, GX, XG cases
        if row_info.bit_depth == 8 {
            if at_start {
                // Skip initial filler
                sp += 1;
            } else {
                // Skip initial channel and, for sp, the filler
                sp += 2;
                dp += 1;
            }
            // For a 1 pixel wide image there is nothing to do
            while sp < ep {
                row[dp] = row[sp];
                dp += 1;
                sp += 2;
            }
            row_info.pixel_depth = 8;
        } else if row_info.bit_depth == 16 {
            if at_start {
                // Skip initial filler
                sp += 2;
            } else {
                // Skip initial channel and, for sp, the filler
                sp += 4;
                dp += 2;
            }
            while sp < ep {
                row[dp] = row[sp];
                row[dp + 1] = row[sp + 1];
                dp += 2;
                sp += 4;
            }
            row_info.pixel_depth = 16;
        } else {
            return; // bad bit depth
        }
        row_info.channels = 1;
        // Finally fix the color type if it records an alpha channel
        if row_info.color_type == 4 {
            row_info.color_type = 0;
        }
    } else if row_info.channels == 4 {
        // RGBA, RGBX, XRGB cases
        if row_info.bit_depth == 8 {
            if at_start {
                // Skip initial filler
                sp += 1;
            } else {
                // Skip initial channels and, for sp, the filler
                sp += 4;
                dp += 3;
            }
            // Note that the loop adds 3 to dp and 4 to sp each time.
            while sp < ep {
                row[dp] = row[sp];
                row[dp + 1] = row[sp + 1];
                row[dp + 2] = row[sp + 2];
                dp += 3;
                sp += 4;
            }
            row_info.pixel_depth = 24;
        } else if row_info.bit_depth == 16 {
            if at_start {
                // Skip initial filler
                sp += 2;
            } else {
                // Skip initial channels and, for sp, the filler
                sp += 8;
                dp += 6;
            }
            while sp < ep {
                // Copy 6 bytes, skip 2
                for k in 0..6 {
                    row[dp + k] = row[sp + k];
                }
                dp += 6;
                sp += 8;
            }
            row_info.pixel_depth = 48;
        } else {
            return; // bad bit depth
        }
        row_info.channels = 3;
        // Finally fix the color type if it records an alpha channel
        if row_info.color_type == 6 {
            row_info.color_type = 2;
        }
    } else {
        return; // The filler channel has gone already
    }

    // Fix the rowbytes value.
    row_info.rowbytes = dp;
}
