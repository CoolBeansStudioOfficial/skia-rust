// Copyright 2010 Google Inc. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the COPYING file. Port by The skia-rust Authors.

//! Constant tables of libwebp's VP8 decoder (`src/dec/tree_dec.c`, `src/dec/quant_dec.c`,
//! `src/dec/frame_dec.c`, `src/dec/vp8_dec.c`). The large probability tables are in
//! [`crate::vp8_tables`]'s generated part, transcribed mechanically from the C source.

// Module-level clippy allows. Each one mirrors the C source of this module.
// clippy::doc_markdown: generated tables; the docs quote the C identifiers verbatim.
#![allow(clippy::doc_markdown)]

/// Port of `kZigzag`.
pub static K_ZIGZAG: [usize; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];

/// Port of `kCat3`.
pub static K_CAT3: [u8; 3] = [173, 148, 140];
/// Port of `kCat4`.
pub static K_CAT4: [u8; 4] = [176, 155, 140, 135];
/// Port of `kCat5`.
pub static K_CAT5: [u8; 5] = [180, 157, 141, 134, 130];
/// Port of `kCat6`.
pub static K_CAT6: [u8; 11] = [254, 254, 243, 230, 196, 177, 153, 140, 133, 130, 129];

/// Port of `kBands` (with the sentinel entry).
pub static K_BANDS: [usize; 17] = [0, 1, 2, 3, 6, 4, 5, 6, 6, 6, 6, 6, 6, 6, 6, 7, 0];

/// Port of `kDcTable` (quant_dec.c).
pub static K_DC_TABLE: [u16; 128] = [
    4, 5, 6, 7, 8, 9, 10, 10, 11, 12, 13, 14, 15, 16, 17, 17, //
    18, 19, 20, 20, 21, 21, 22, 22, 23, 23, 24, 25, 25, 26, 27, 28, //
    29, 30, 31, 32, 33, 34, 35, 36, 37, 37, 38, 39, 40, 41, 42, 43, //
    44, 45, 46, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, //
    59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, //
    75, 76, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, //
    91, 93, 95, 96, 98, 100, 101, 102, 104, 106, 108, 110, 112, 114, 116, 118, //
    122, 124, 126, 128, 130, 132, 134, 136, 138, 140, 143, 145, 148, 151, 154, 157, //
];

/// Port of `kAcTable` (quant_dec.c).
pub static K_AC_TABLE: [u16; 128] = [
    4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, //
    20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, //
    36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, //
    52, 53, 54, 55, 56, 57, 58, 60, 62, 64, 66, 68, 70, 72, 74, 76, //
    78, 80, 82, 84, 86, 88, 90, 92, 94, 96, 98, 100, 102, 104, 106, 108, //
    110, 112, 114, 116, 119, 122, 125, 128, 131, 134, 137, 140, 143, 146, 149, 152, //
    155, 158, 161, 164, 167, 170, 173, 177, 181, 185, 189, 193, 197, 201, 205, 209, //
    213, 217, 221, 225, 229, 234, 239, 245, 249, 254, 259, 264, 269, 274, 279, 284, //
];

/// Port of `kFilterExtraRows` (frame_dec.c), indexed by the filter type.
pub static K_FILTER_EXTRA_ROWS: [i32; 3] = [0, 2, 8];

/// Port of `kScan` (frame_dec.c): offsets of the 16 luma sub-blocks in the `BPS`-stride buffer.
pub const BPS: usize = 32;
pub static K_SCAN: [usize; 16] = [
    0,
    4,
    8,
    12, //
    4 * BPS,
    4 * BPS + 4,
    4 * BPS + 8,
    4 * BPS + 12, //
    8 * BPS,
    8 * BPS + 4,
    8 * BPS + 8,
    8 * BPS + 12, //
    12 * BPS,
    12 * BPS + 4,
    12 * BPS + 8,
    12 * BPS + 12, //
];

/// Port of `YUV_SIZE`, `Y_OFF`, `U_OFF`, `V_OFF` (vp8i_dec.h).
pub const YUV_SIZE: usize = BPS * 17 + BPS * 9;
pub const Y_OFF: usize = BPS + 8;
pub const U_OFF: usize = Y_OFF + BPS * 16 + BPS;
pub const V_OFF: usize = U_OFF + 16;

/// Port of the 4x4 and 16x16 prediction-mode enum (`common_dec.h`).
pub const B_DC_PRED: u8 = 0;
pub const B_TM_PRED: u8 = 1;
pub const B_VE_PRED: u8 = 2;
pub const B_HE_PRED: u8 = 3;
pub const B_RD_PRED: u8 = 4;
pub const B_VR_PRED: u8 = 5;
pub const B_LD_PRED: u8 = 6;
pub const B_VL_PRED: u8 = 7;
pub const B_HD_PRED: u8 = 8;
pub const B_HU_PRED: u8 = 9;
pub const DC_PRED: u8 = B_DC_PRED;
pub const V_PRED: u8 = B_VE_PRED;
pub const H_PRED: u8 = B_HE_PRED;
pub const TM_PRED: u8 = B_TM_PRED;
pub const B_DC_PRED_NOTOP: u8 = 4;
pub const B_DC_PRED_NOLEFT: u8 = 5;
pub const B_DC_PRED_NOTOPLEFT: u8 = 6;
