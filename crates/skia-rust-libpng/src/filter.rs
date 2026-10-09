// Copyright (C) 1998-2025 Glenn Randers-Pehrson and the libpng contributors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
// Port of: pngrutil.c#L2267-L2402 (libpng 1.6.56, skia.googlesource.com/third_party/libpng@d5515b5b):
// `png_read_filter_row` and the Sub, Up, Avg and Paeth filters. These are the portable C paths.
// libpng's SSE2 unfilter (`intel/filter_sse2_intrinsics.c`) computes the same bytes, which the
// codec-diff harness checks.

// Clippy: each module is a line-by-line port of libpng's C, whose integer casts, long
// functions, argument lists and error returns are kept as written so they can be compared
// with the C. The Port of links name the C source for each item.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::missing_errors_doc,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::cognitive_complexity
)]

/// Port of `PNG_FILTER_VALUE_*` (pngpriv.h).
pub(crate) const PNG_FILTER_VALUE_SUB: u8 = 1;
pub(crate) const PNG_FILTER_VALUE_UP: u8 = 2;
pub(crate) const PNG_FILTER_VALUE_AVG: u8 = 3;
pub(crate) const PNG_FILTER_VALUE_PAETH: u8 = 4;

/// Port of `png_read_filter_row_sub` (pngrutil.c#L2267-L2280).
fn filter_row_sub(row: &mut [u8], rowbytes: usize, pixel_depth: u8) {
    let bpp = (usize::from(pixel_depth) + 7) >> 3;
    for i in bpp..rowbytes {
        row[i] = row[i].wrapping_add(row[i - bpp]);
    }
}

/// Port of `png_read_filter_row_up` (pngrutil.c#L2282-L2294).
fn filter_row_up(row: &mut [u8], prev_row: &[u8], rowbytes: usize) {
    for i in 0..rowbytes {
        row[i] = row[i].wrapping_add(prev_row[i]);
    }
}

/// Port of `png_read_filter_row_avg` (pngrutil.c#L2296-L2316). The integer divisions are C's, on
/// non-negative values.
fn filter_row_avg(row: &mut [u8], prev_row: &[u8], rowbytes: usize, pixel_depth: u8) {
    let bpp = (usize::from(pixel_depth) + 7) >> 3;
    let istop = rowbytes - bpp;
    let mut pp = 0usize;
    for i in 0..bpp {
        let _ = i;
        row[pp] = row[pp].wrapping_add(prev_row[pp] / 2);
        pp += 1;
    }
    // `rp` is `row + bpp` after the first loop.
    for i in 0..istop {
        let rp = bpp + i;
        let sum = u32::from(prev_row[pp]) + u32::from(row[rp - bpp]);
        row[rp] = row[rp].wrapping_add((sum / 2) as u8);
        pp += 1;
    }
}

/// Port of `png_read_filter_row_paeth_1byte_pixel` (pngrutil.c#L2318-L2345).
fn filter_row_paeth_1byte(row: &mut [u8], prev_row: &[u8], rowbytes: usize) {
    let mut c = i32::from(prev_row[0]);
    let mut a = i32::from(row[0]) + c;
    row[0] = a as u8;
    let mut pp = 1usize;
    let mut rp = 1usize;
    while rp < rowbytes {
        a &= 0xff;
        let b = i32::from(prev_row[pp]);
        pp += 1;
        let p = b - c;
        let mut pc = a - c;
        let mut pa = if p < 0 { -p } else { p };
        let pb = if pc < 0 { -pc } else { pc };
        pc = if p + pc < 0 { -(p + pc) } else { p + pc };
        if pb < pa {
            pa = pb;
            a = b;
        }
        if pc < pa {
            a = c;
        }
        c = b;
        a += i32::from(row[rp]);
        row[rp] = a as u8;
        rp += 1;
    }
}

/// Port of `png_read_filter_row_paeth_multibyte_pixel` (pngrutil.c#L2347-L2377).
fn filter_row_paeth_multibyte(row: &mut [u8], prev_row: &[u8], rowbytes: usize, pixel_depth: u8) {
    let bpp = (usize::from(pixel_depth) + 7) >> 3;
    let mut rp = 0usize;
    let mut pp = 0usize;
    while rp < bpp {
        row[rp] = row[rp].wrapping_add(prev_row[pp]);
        rp += 1;
        pp += 1;
    }
    let rp_end = bpp + (rowbytes - bpp);
    while rp < rp_end {
        // c = prev[pp - bpp], a = row[rp - bpp], b = prev[pp]
        let c = i32::from(prev_row[pp - bpp]);
        let mut a = i32::from(row[rp - bpp]);
        let b = i32::from(prev_row[pp]);
        pp += 1;
        let p = b - c;
        let mut pc = a - c;
        let mut pa = if p < 0 { -p } else { p };
        let pb = if pc < 0 { -pc } else { pc };
        pc = if p + pc < 0 { -(p + pc) } else { p + pc };
        if pb < pa {
            pa = pb;
            a = b;
        }
        if pc < pa {
            a = c;
        }
        a += i32::from(row[rp]);
        row[rp] = a as u8;
        rp += 1;
    }
}

/// Port of `png_read_filter_row` (pngrutil.c#L2393-L2402). `row` is the row without its filter
/// byte (`row_buf + 1`), and `prev_row` the previous unfiltered row (`prev_row + 1`).
pub(crate) fn read_filter_row(
    row: &mut [u8],
    prev_row: &[u8],
    rowbytes: usize,
    pixel_depth: u8,
    filter: u8,
) {
    match filter {
        PNG_FILTER_VALUE_SUB => filter_row_sub(row, rowbytes, pixel_depth),
        PNG_FILTER_VALUE_UP => filter_row_up(row, prev_row, rowbytes),
        PNG_FILTER_VALUE_AVG => filter_row_avg(row, prev_row, rowbytes, pixel_depth),
        PNG_FILTER_VALUE_PAETH => {
            // png_init_filter_functions picks the 1-byte form only when bpp == 1.
            let bpp = (usize::from(pixel_depth) + 7) >> 3;
            if bpp == 1 {
                filter_row_paeth_1byte(row, prev_row, rowbytes);
            } else {
                filter_row_paeth_multibyte(row, prev_row, rowbytes, pixel_depth);
            }
        }
        _ => {}
    }
}
