// Copyright 2011 Google Inc. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the COPYING file. Port by The skia-rust Authors.

//! Port of the C kernels of libwebp `src/dsp/dec.c` (inverse transforms, intra prediction and
//! the loop filters), with the clip tables of `src/dsp/dec_clip_tables.c` computed as the
//! clamps they are defined to be.
//!
//! Blocks are reconstructed in libwebp's `BPS`-stride work buffer. A block is addressed by the
//! index `o` of its top-left sample in the buffer. Neighbours are read at negative offsets, as in C.

// Module-level clippy allows. Each one mirrors the C source of this module.
// clippy::cast_possible_truncation: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::cast_possible_wrap: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::cast_sign_loss: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::many_single_char_names: pixel, offset and loop variables keep the single-letter names of the C source.
// clippy::too_many_arguments: the signature mirrors the parameters of the C function.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::too_many_arguments
)]

use crate::vp8_tables_small::{B_DC_PRED_NOLEFT, BPS};

/// Port of `WEBP_TRANSFORM_AC3_C1`.
const AC3_C1: i32 = 20091;
/// Port of `WEBP_TRANSFORM_AC3_C2`.
const AC3_C2: i32 = 35468;

/// Port of `WEBP_TRANSFORM_AC3_MUL1`.
#[inline]
fn ac3_mul1(a: i32) -> i32 {
    ((a * AC3_C1) >> 16) + a
}

/// Port of `WEBP_TRANSFORM_AC3_MUL2`.
#[inline]
fn ac3_mul2(a: i32) -> i32 {
    (a * AC3_C2) >> 16
}

/// Port of `clip_8b`: clamps to `0..=255`.
#[inline]
fn clip_8b(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

/// Port of `STORE(x, y, v)`: adds `v >> 3` to the sample at `(x, y)` with clipping.
#[inline]
fn store(dst: &mut [u8], o: usize, x: usize, y: usize, v: i32) {
    let idx = o + x + y * BPS;
    dst[idx] = clip_8b(i32::from(dst[idx]) + (v >> 3));
}

/// Port of `TransformOne_C`: full 4x4 inverse DCT added to the prediction at `o`.
#[doc(alias = "TransformOne")]
pub fn transform_one(input: &[i16], dst: &mut [u8], o: usize) {
    let mut c = [0i32; 4 * 4];
    // vertical pass
    for i in 0..4 {
        let a = i32::from(input[i]) + i32::from(input[8 + i]); // [-4096, 4094]
        let b = i32::from(input[i]) - i32::from(input[8 + i]); // [-4095, 4095]
        let c_ = ac3_mul2(i32::from(input[4 + i])) - ac3_mul1(i32::from(input[12 + i])); // [-3783, 3783]
        let d = ac3_mul1(i32::from(input[4 + i])) + ac3_mul2(i32::from(input[12 + i])); // [-3785, 3781]
        c[4 * i] = a + d; // [-7881, 7875]
        c[4 * i + 1] = b + c_; // [-7878, 7878]
        c[4 * i + 2] = b - c_; // [-7878, 7878]
        c[4 * i + 3] = a - d; // [-7877, 7879]
    }
    // horizontal pass
    for i in 0..4 {
        let dc = c[i] + 4;
        let a = dc + c[8 + i];
        let b = dc - c[8 + i];
        let c_ = ac3_mul2(c[4 + i]) - ac3_mul1(c[12 + i]);
        let d = ac3_mul1(c[4 + i]) + ac3_mul2(c[12 + i]);
        store(dst, o, 0, i, a + d);
        store(dst, o, 1, i, b + c_);
        store(dst, o, 2, i, b - c_);
        store(dst, o, 3, i, a - d);
    }
}

/// Port of `TransformAC3_C`: inverse DCT when only in[0], in[1] and in[4] are non-zero.
#[doc(alias = "TransformAC3")]
pub fn transform_ac3(input: &[i16], dst: &mut [u8], o: usize) {
    let a = i32::from(input[0]) + 4;
    let c4 = ac3_mul2(i32::from(input[4]));
    let d4 = ac3_mul1(i32::from(input[4]));
    let c1 = ac3_mul2(i32::from(input[1]));
    let d1 = ac3_mul1(i32::from(input[1]));
    store2(dst, o, 0, a + d4, d1, c1);
    store2(dst, o, 1, a + c4, d1, c1);
    store2(dst, o, 2, a - c4, d1, c1);
    store2(dst, o, 3, a - d4, d1, c1);
}

/// Port of `STORE2(y, dc, d, c)`.
#[inline]
fn store2(dst: &mut [u8], o: usize, y: usize, dc: i32, d: i32, c: i32) {
    store(dst, o, 0, y, dc + d);
    store(dst, o, 1, y, dc + c);
    store(dst, o, 2, y, dc - c);
    store(dst, o, 3, y, dc - d);
}

/// Port of `TransformDC_C`: DC-only inverse transform.
#[doc(alias = "TransformDC")]
pub fn transform_dc(input: &[i16], dst: &mut [u8], o: usize) {
    let dc = i32::from(input[0]) + 4;
    for j in 0..4 {
        for i in 0..4 {
            store(dst, o, i, j, dc);
        }
    }
}

/// Port of `TransformUV_C`: two pairs of 4x4 blocks of the chroma planes.
#[doc(alias = "TransformUV")]
pub fn transform_uv(input: &[i16], dst: &mut [u8], o: usize) {
    // VP8Transform(in + 0 * 16, dst, 1) and VP8Transform(in + 2 * 16, dst + 4 * BPS, 1)
    transform_one(&input[0..16], dst, o);
    transform_one(&input[16..32], dst, o + 4);
    transform_one(&input[32..48], dst, o + 4 * BPS);
    transform_one(&input[48..64], dst, o + 4 * BPS + 4);
}

/// Port of `TransformDCUV_C`.
#[doc(alias = "TransformDCUV")]
pub fn transform_dc_uv(input: &[i16], dst: &mut [u8], o: usize) {
    if input[0] != 0 {
        transform_dc(&input[0..16], dst, o);
    }
    if input[16] != 0 {
        transform_dc(&input[16..32], dst, o + 4);
    }
    if input[32] != 0 {
        transform_dc(&input[32..48], dst, o + 4 * BPS);
    }
    if input[48] != 0 {
        transform_dc(&input[48..64], dst, o + 4 * BPS + 4);
    }
}

/// Port of `TransformWHT_C`: inverse Walsh-Hadamard transform of the luma DC coefficients.
/// `out` receives the DC of each of the 16 luma blocks (every 16 entries of the coefficient
/// array, as `dst[i]` in the caller).
#[doc(alias = "TransformWHT")]
pub fn transform_wht(input: &[i16], out: &mut [i16]) {
    let mut tmp = [0i32; 16];
    for i in 0..4 {
        let a0 = i32::from(input[i]) + i32::from(input[12 + i]);
        let a1 = i32::from(input[4 + i]) + i32::from(input[8 + i]);
        let a2 = i32::from(input[4 + i]) - i32::from(input[8 + i]);
        let a3 = i32::from(input[i]) - i32::from(input[12 + i]);
        tmp[i] = a0 + a1;
        tmp[8 + i] = a0 - a1;
        tmp[4 + i] = a3 + a2;
        tmp[12 + i] = a3 - a2;
    }
    for i in 0..4 {
        let dc = tmp[i * 4] + 3; // w/ rounder
        let a0 = dc + tmp[3 + i * 4];
        let a1 = tmp[1 + i * 4] + tmp[2 + i * 4];
        let a2 = tmp[1 + i * 4] - tmp[2 + i * 4];
        let a3 = dc - tmp[3 + i * 4];
        // `out` is advanced by 64 per row in C; here `base` is the row's first output index.
        let base = i * 64;
        out[base] = ((a0 + a1) >> 3) as i16;
        out[base + 16] = ((a3 + a2) >> 3) as i16;
        out[base + 32] = ((a0 - a1) >> 3) as i16;
        out[base + 48] = ((a3 - a2) >> 3) as i16;
    }
}

/// Port of `TrueMotion` for `size` x `size` blocks.
fn true_motion(dst: &mut [u8], o: usize, size: usize) {
    let top = o - BPS;
    let top_left = i32::from(dst[top - 1]);
    for y in 0..size {
        let row = o + y * BPS;
        let left = i32::from(dst[row - 1]);
        for x in 0..size {
            let v = left + i32::from(dst[top + x]) - top_left;
            dst[row + x] = clip_8b(v);
        }
    }
}

/// Port of `Put16`.
fn put16(v: u8, dst: &mut [u8], o: usize) {
    for j in 0..16 {
        for x in 0..16 {
            dst[o + j * BPS + x] = v;
        }
    }
}

/// Port of `VE16_C`.
fn ve16(dst: &mut [u8], o: usize) {
    for j in 0..16 {
        for x in 0..16 {
            dst[o + j * BPS + x] = dst[o - BPS + x];
        }
    }
}

/// Port of `HE16_C`.
fn he16(dst: &mut [u8], o: usize) {
    for j in 0..16 {
        let v = dst[o + j * BPS - 1];
        for x in 0..16 {
            dst[o + j * BPS + x] = v;
        }
    }
}

/// Predictor `index` of `VP8PredLuma16` (`DC16_C` and friends), for a 16x16 luma block.
#[doc(alias = "VP8PredLuma16")]
pub fn pred_luma16(index: u8, dst: &mut [u8], o: usize) {
    match index {
        0 => {
            // DC16_C
            let mut dc: i32 = 16;
            for j in 0..16 {
                dc += i32::from(dst[o - 1 + j * BPS]) + i32::from(dst[o + j - BPS]);
            }
            put16((dc >> 5) as u8, dst, o);
        }
        1 => true_motion(dst, o, 16),
        2 => ve16(dst, o),
        3 => he16(dst, o),
        4 => {
            // DC16NoTop_C: sums the left column.
            let mut dc: i32 = 8;
            for j in 0..16 {
                dc += i32::from(dst[o - 1 + j * BPS]);
            }
            put16((dc >> 4) as u8, dst, o);
        }
        5 => {
            // DC16NoLeft_C: sums the top row.
            let mut dc: i32 = 8;
            for i in 0..16 {
                dc += i32::from(dst[o + i - BPS]);
            }
            put16((dc >> 4) as u8, dst, o);
        }
        _ => put16(0x80, dst, o), // DC16NoTopLeft_C
    }
}

/// Port of `VE8uv_C`, `HE8uv_C`, `DC8uv_C` and friends: predictor `index` of `VP8PredChroma8`.
#[doc(alias = "VP8PredChroma8")]
pub fn pred_chroma8(index: u8, dst: &mut [u8], o: usize) {
    match index {
        0 => {
            // DC8uv_C
            let mut dc0: i32 = 8;
            for i in 0..8 {
                dc0 += i32::from(dst[o + i - BPS]) + i32::from(dst[o - 1 + i * BPS]);
            }
            put8x8uv((dc0 >> 4) as u8, dst, o);
        }
        1 => true_motion(dst, o, 8),
        2 => {
            for j in 0..8 {
                for x in 0..8 {
                    dst[o + j * BPS + x] = dst[o - BPS + x];
                }
            }
        }
        3 => {
            for j in 0..8 {
                let v = dst[o + j * BPS - 1];
                for x in 0..8 {
                    dst[o + j * BPS + x] = v;
                }
            }
        }
        4 => {
            // DC8uvNoTop_C: sums the left column.
            let mut dc0: i32 = 4;
            for i in 0..8 {
                dc0 += i32::from(dst[o - 1 + i * BPS]);
            }
            put8x8uv((dc0 >> 3) as u8, dst, o);
        }
        5 => {
            // DC8uvNoLeft_C: sums the top row.
            debug_assert_eq!(index, B_DC_PRED_NOLEFT);
            let mut dc0: i32 = 4;
            for i in 0..8 {
                dc0 += i32::from(dst[o + i - BPS]);
            }
            put8x8uv((dc0 >> 3) as u8, dst, o);
        }
        _ => put8x8uv(0x80, dst, o), // DC8uvNoTopLeft_C
    }
}

/// Port of `Put8x8uv`.
fn put8x8uv(v: u8, dst: &mut [u8], o: usize) {
    for j in 0..8 {
        for x in 0..8 {
            dst[o + j * BPS + x] = v;
        }
    }
}

/// Port of `AVG3`.
#[inline]
fn avg3(a: u8, b: u8, c: u8) -> u8 {
    ((u32::from(a) + 2 * u32::from(b) + u32::from(c) + 2) >> 2) as u8
}

/// Port of `AVG2`.
#[inline]
fn avg2(a: u8, b: u8) -> u8 {
    ((u32::from(a) + u32::from(b) + 1) >> 1) as u8
}

/// Port of `DST(x, y)`: sample `(x, y)` of the block at `o`.
#[inline]
fn dst_idx(o: usize, x: usize, y: usize) -> usize {
    o + x + y * BPS
}

/// Predictor `index` of `VP8PredLuma4`: the 4x4 intra modes (`B_DC_PRED` ... `B_HU_PRED`).
#[doc(alias = "VP8PredLuma4")]
pub fn pred_luma4(index: u8, dst: &mut [u8], o: usize) {
    let g = |dst: &[u8], i: isize| -> u8 { dst[(o as isize + i) as usize] };
    match index {
        0 => {
            // DC4_C
            let mut dc: u32 = 4;
            for i in 0..4 {
                dc += u32::from(dst[o + i - BPS]) + u32::from(dst[o - 1 + i * BPS]);
            }
            dc >>= 3;
            for i in 0..4 {
                for x in 0..4 {
                    dst[o + i * BPS + x] = dc as u8;
                }
            }
        }
        1 => true_motion(dst, o, 4),
        2 => {
            // VE4_C
            let t = |k: isize| g(dst, k - BPS as isize);
            let vals = [
                avg3(t(-1), t(0), t(1)),
                avg3(t(0), t(1), t(2)),
                avg3(t(1), t(2), t(3)),
                avg3(t(2), t(3), t(4)),
            ];
            for i in 0..4 {
                for x in 0..4 {
                    dst[o + i * BPS + x] = vals[x];
                }
            }
        }
        3 => {
            // HE4_C
            let a = dst[o - 1 - BPS];
            let b = dst[o - 1];
            let c = dst[o - 1 + BPS];
            let d = dst[o - 1 + 2 * BPS];
            let e = dst[o - 1 + 3 * BPS];
            let rows = [avg3(a, b, c), avg3(b, c, d), avg3(c, d, e), avg3(d, e, e)];
            for (i, v) in rows.iter().enumerate() {
                for x in 0..4 {
                    dst[o + i * BPS + x] = *v;
                }
            }
        }
        4 => pred_rd4(dst, o),
        5 => pred_vr4(dst, o),
        6 => pred_ld4(dst, o),
        7 => pred_vl4(dst, o),
        8 => pred_hd4(dst, o),
        _ => pred_hu4(dst, o), // 9: HU4_C
    }
}

/// Port of `RD4_C` (down-right).
fn pred_rd4(dst: &mut [u8], o: usize) {
    let i_ = dst[o - 1];
    let j = dst[o - 1 + BPS];
    let k = dst[o - 1 + 2 * BPS];
    let l = dst[o - 1 + 3 * BPS];
    let x = dst[o - 1 - BPS];
    let a = dst[o - BPS];
    let b = dst[o + 1 - BPS];
    let c = dst[o + 2 - BPS];
    let d = dst[o + 3 - BPS];
    let set = |dst: &mut [u8], px: usize, py: usize, v: u8| dst[dst_idx(o, px, py)] = v;
    set(dst, 0, 3, avg3(j, k, l));
    let v = avg3(i_, j, k);
    set(dst, 1, 3, v);
    set(dst, 0, 2, v);
    let v = avg3(x, i_, j);
    set(dst, 2, 3, v);
    set(dst, 1, 2, v);
    set(dst, 0, 1, v);
    let v = avg3(a, x, i_);
    set(dst, 3, 3, v);
    set(dst, 2, 2, v);
    set(dst, 1, 1, v);
    set(dst, 0, 0, v);
    let v = avg3(b, a, x);
    set(dst, 3, 2, v);
    set(dst, 2, 1, v);
    set(dst, 1, 0, v);
    let v = avg3(c, b, a);
    set(dst, 3, 1, v);
    set(dst, 2, 0, v);
    set(dst, 3, 0, avg3(d, c, b));
}

/// Port of `LD4_C` (down-left).
fn pred_ld4(dst: &mut [u8], o: usize) {
    let a = dst[o - BPS];
    let b = dst[o + 1 - BPS];
    let c = dst[o + 2 - BPS];
    let d = dst[o + 3 - BPS];
    let e = dst[o + 4 - BPS];
    let f = dst[o + 5 - BPS];
    let g = dst[o + 6 - BPS];
    let h = dst[o + 7 - BPS];
    let set = |dst: &mut [u8], px: usize, py: usize, v: u8| dst[dst_idx(o, px, py)] = v;
    set(dst, 0, 0, avg3(a, b, c));
    let v = avg3(b, c, d);
    set(dst, 1, 0, v);
    set(dst, 0, 1, v);
    let v = avg3(c, d, e);
    set(dst, 2, 0, v);
    set(dst, 1, 1, v);
    set(dst, 0, 2, v);
    let v = avg3(d, e, f);
    set(dst, 3, 0, v);
    set(dst, 2, 1, v);
    set(dst, 1, 2, v);
    set(dst, 0, 3, v);
    let v = avg3(e, f, g);
    set(dst, 3, 1, v);
    set(dst, 2, 2, v);
    set(dst, 1, 3, v);
    let v = avg3(f, g, h);
    set(dst, 3, 2, v);
    set(dst, 2, 3, v);
    set(dst, 3, 3, avg3(g, h, h));
}

/// Port of `VR4_C` (vertical-right).
fn pred_vr4(dst: &mut [u8], o: usize) {
    let i_ = dst[o - 1];
    let j = dst[o - 1 + BPS];
    let k = dst[o - 1 + 2 * BPS];
    let x = dst[o - 1 - BPS];
    let a = dst[o - BPS];
    let b = dst[o + 1 - BPS];
    let c = dst[o + 2 - BPS];
    let d = dst[o + 3 - BPS];
    let set = |dst: &mut [u8], px: usize, py: usize, v: u8| dst[dst_idx(o, px, py)] = v;
    let v = avg2(x, a);
    set(dst, 0, 0, v);
    set(dst, 1, 2, v);
    let v = avg2(a, b);
    set(dst, 1, 0, v);
    set(dst, 2, 2, v);
    let v = avg2(b, c);
    set(dst, 2, 0, v);
    set(dst, 3, 2, v);
    set(dst, 3, 0, avg2(c, d));
    set(dst, 0, 3, avg3(k, j, i_));
    set(dst, 0, 2, avg3(j, i_, x));
    let v = avg3(i_, x, a);
    set(dst, 0, 1, v);
    set(dst, 1, 3, v);
    let v = avg3(x, a, b);
    set(dst, 1, 1, v);
    set(dst, 2, 3, v);
    let v = avg3(a, b, c);
    set(dst, 2, 1, v);
    set(dst, 3, 3, v);
    set(dst, 3, 1, avg3(b, c, d));
}

/// Port of `VL4_C` (vertical-left).
fn pred_vl4(dst: &mut [u8], o: usize) {
    let a = dst[o - BPS];
    let b = dst[o + 1 - BPS];
    let c = dst[o + 2 - BPS];
    let d = dst[o + 3 - BPS];
    let e = dst[o + 4 - BPS];
    let f = dst[o + 5 - BPS];
    let g = dst[o + 6 - BPS];
    let h = dst[o + 7 - BPS];
    let set = |dst: &mut [u8], px: usize, py: usize, v: u8| dst[dst_idx(o, px, py)] = v;
    set(dst, 0, 0, avg2(a, b));
    let v = avg2(b, c);
    set(dst, 1, 0, v);
    set(dst, 0, 2, v);
    let v = avg2(c, d);
    set(dst, 2, 0, v);
    set(dst, 1, 2, v);
    let v = avg2(d, e);
    set(dst, 3, 0, v);
    set(dst, 2, 2, v);
    set(dst, 0, 1, avg3(a, b, c));
    let v = avg3(b, c, d);
    set(dst, 1, 1, v);
    set(dst, 0, 3, v);
    let v = avg3(c, d, e);
    set(dst, 2, 1, v);
    set(dst, 1, 3, v);
    let v = avg3(d, e, f);
    set(dst, 3, 1, v);
    set(dst, 2, 3, v);
    set(dst, 3, 2, avg3(e, f, g));
    set(dst, 3, 3, avg3(f, g, h));
}

/// Port of `HU4_C` (horizontal-up).
fn pred_hu4(dst: &mut [u8], o: usize) {
    let i_ = dst[o - 1];
    let j = dst[o - 1 + BPS];
    let k = dst[o - 1 + 2 * BPS];
    let l = dst[o - 1 + 3 * BPS];
    let set = |dst: &mut [u8], px: usize, py: usize, v: u8| dst[dst_idx(o, px, py)] = v;
    set(dst, 0, 0, avg2(i_, j));
    let v = avg2(j, k);
    set(dst, 2, 0, v);
    set(dst, 0, 1, v);
    let v = avg2(k, l);
    set(dst, 2, 1, v);
    set(dst, 0, 2, v);
    set(dst, 1, 0, avg3(i_, j, k));
    let v = avg3(j, k, l);
    set(dst, 3, 0, v);
    set(dst, 1, 1, v);
    let v = avg3(k, l, l);
    set(dst, 3, 1, v);
    set(dst, 1, 2, v);
    for (px, py) in [(3, 2), (2, 2), (0, 3), (1, 3), (2, 3), (3, 3)] {
        set(dst, px, py, l);
    }
}

/// Port of `HD4_C` (horizontal-down).
fn pred_hd4(dst: &mut [u8], o: usize) {
    let i_ = dst[o - 1];
    let j = dst[o - 1 + BPS];
    let k = dst[o - 1 + 2 * BPS];
    let l = dst[o - 1 + 3 * BPS];
    let x = dst[o - 1 - BPS];
    let a = dst[o - BPS];
    let b = dst[o + 1 - BPS];
    let c = dst[o + 2 - BPS];
    let set = |dst: &mut [u8], px: usize, py: usize, v: u8| dst[dst_idx(o, px, py)] = v;
    let v = avg2(i_, x);
    set(dst, 0, 0, v);
    set(dst, 2, 1, v);
    let v = avg2(j, i_);
    set(dst, 0, 1, v);
    set(dst, 2, 2, v);
    let v = avg2(k, j);
    set(dst, 0, 2, v);
    set(dst, 2, 3, v);
    set(dst, 0, 3, avg2(l, k));
    set(dst, 3, 0, avg3(a, b, c));
    set(dst, 2, 0, avg3(x, a, b));
    let v = avg3(i_, x, a);
    set(dst, 1, 0, v);
    set(dst, 3, 1, v);
    let v = avg3(j, i_, x);
    set(dst, 1, 1, v);
    set(dst, 3, 2, v);
    let v = avg3(k, j, i_);
    set(dst, 1, 2, v);
    set(dst, 3, 3, v);
    set(dst, 1, 3, avg3(l, k, j));
}

/// Port of `DoFilter2_C`.
#[inline]
fn do_filter2(p: &mut [u8], o: usize, step: usize) {
    let p1 = i32::from(p[o - 2 * step]);
    let p0 = i32::from(p[o - step]);
    let q0 = i32::from(p[o]);
    let q1 = i32::from(p[o + step]);
    let a = 3 * (q0 - p0) + sclip1(p1 - q1); // in [-893,892]
    let a1 = sclip2((a + 4) >> 3); // in [-16,15]
    let a2 = sclip2((a + 3) >> 3);
    p[o - step] = clip_8b(p0 + a2);
    p[o] = clip_8b(q0 - a1);
}

/// Port of `VP8ksclip1`: clamps to `-128..=127`.
#[inline]
fn sclip1(v: i32) -> i32 {
    v.clamp(-128, 127)
}

/// Port of `VP8ksclip2`: clamps to `-16..=15`.
#[inline]
fn sclip2(v: i32) -> i32 {
    v.clamp(-16, 15)
}

/// Port of `DoFilter4_C`.
#[inline]
fn do_filter4(p: &mut [u8], o: usize, step: usize) {
    let p1 = i32::from(p[o - 2 * step]);
    let p0 = i32::from(p[o - step]);
    let q0 = i32::from(p[o]);
    let q1 = i32::from(p[o + step]);
    let a = 3 * (q0 - p0);
    let a1 = sclip2((a + 4) >> 3);
    let a2 = sclip2((a + 3) >> 3);
    let a3 = (a1 + 1) >> 1;
    p[o - 2 * step] = clip_8b(p1 + a3);
    p[o - step] = clip_8b(p0 + a2);
    p[o] = clip_8b(q0 - a1);
    p[o + step] = clip_8b(q1 - a3);
}

/// Port of `DoFilter6_C`.
#[inline]
fn do_filter6(p: &mut [u8], o: usize, step: usize) {
    let p2 = i32::from(p[o - 3 * step]);
    let p1 = i32::from(p[o - 2 * step]);
    let p0 = i32::from(p[o - step]);
    let q0 = i32::from(p[o]);
    let q1 = i32::from(p[o + step]);
    let q2 = i32::from(p[o + 2 * step]);
    let a = sclip1(3 * (q0 - p0) + sclip1(p1 - q1));
    let a1 = (27 * a + 63) >> 7; // eq. to ((3 * a + 7) * 9) >> 7
    let a2 = (18 * a + 63) >> 7; // eq. to ((2 * a + 7) * 9) >> 7
    let a3 = (9 * a + 63) >> 7; // eq. to ((1 * a + 7) * 9) >> 7
    p[o - 3 * step] = clip_8b(p2 + a3);
    p[o - 2 * step] = clip_8b(p1 + a2);
    p[o - step] = clip_8b(p0 + a1);
    p[o] = clip_8b(q0 - a1);
    p[o + step] = clip_8b(q1 - a2);
    p[o + 2 * step] = clip_8b(q2 - a3);
}

/// Port of `Hev`.
#[inline]
fn hev(p: &[u8], o: usize, step: usize, thresh: i32) -> bool {
    let p1 = i32::from(p[o - 2 * step]);
    let p0 = i32::from(p[o - step]);
    let q0 = i32::from(p[o]);
    let q1 = i32::from(p[o + step]);
    (p1 - p0).abs() > thresh || (q1 - q0).abs() > thresh
}

/// Port of `NeedsFilter_C`.
#[inline]
fn needs_filter(p: &[u8], o: usize, step: usize, t: i32) -> bool {
    let p1 = i32::from(p[o - 2 * step]);
    let p0 = i32::from(p[o - step]);
    let q0 = i32::from(p[o]);
    let q1 = i32::from(p[o + step]);
    4 * (p0 - q0).abs() + (p1 - q1).abs() <= t
}

/// Port of `NeedsFilter2_C`.
#[inline]
fn needs_filter2(p: &[u8], o: usize, step: usize, t: i32, it: i32) -> bool {
    let p3 = i32::from(p[o - 4 * step]);
    let p2 = i32::from(p[o - 3 * step]);
    let p1 = i32::from(p[o - 2 * step]);
    let p0 = i32::from(p[o - step]);
    let q0 = i32::from(p[o]);
    let q1 = i32::from(p[o + step]);
    let q2 = i32::from(p[o + 2 * step]);
    let q3 = i32::from(p[o + 3 * step]);
    if 4 * (p0 - q0).abs() + (p1 - q1).abs() > t {
        return false;
    }
    (p3 - p2).abs() <= it
        && (p2 - p1).abs() <= it
        && (p1 - p0).abs() <= it
        && (q3 - q2).abs() <= it
        && (q2 - q1).abs() <= it
        && (q1 - q0).abs() <= it
}

/// Port of `SimpleVFilter16_C`/`SimpleHFilter16_C`: the simple filter over 16 samples.
/// `along` is the step between samples, `across` the step across the edge.
fn simple_filter16(p: &mut [u8], o: usize, along: usize, across: usize, thresh: i32) {
    let thresh2 = 2 * thresh + 1;
    for i in 0..16 {
        let pos = o + i * along;
        if needs_filter(p, pos, across, thresh2) {
            do_filter2(p, pos, across);
        }
    }
}

/// Port of `SimpleVFilter16_C`.
#[doc(alias = "VP8SimpleVFilter16")]
pub fn simple_v_filter16(p: &mut [u8], o: usize, stride: usize, thresh: i32) {
    simple_filter16(p, o, 1, stride, thresh);
}

/// Port of `SimpleHFilter16_C`.
#[doc(alias = "VP8SimpleHFilter16")]
pub fn simple_h_filter16(p: &mut [u8], o: usize, stride: usize, thresh: i32) {
    simple_filter16(p, o, stride, 1, thresh);
}

/// Port of `SimpleVFilter16i_C`.
#[doc(alias = "VP8SimpleVFilter16i")]
pub fn simple_v_filter16i(p: &mut [u8], mut o: usize, stride: usize, thresh: i32) {
    for _ in 0..3 {
        o += 4 * stride;
        simple_v_filter16(p, o, stride, thresh);
    }
}

/// Port of `SimpleHFilter16i_C`.
#[doc(alias = "VP8SimpleHFilter16i")]
pub fn simple_h_filter16i(p: &mut [u8], mut o: usize, stride: usize, thresh: i32) {
    for _ in 0..3 {
        o += 4;
        simple_h_filter16(p, o, stride, thresh);
    }
}

/// Port of `FilterLoop26_C` (the MB-edge filter: `DoFilter6` or `DoFilter2`).
fn filter_loop26(
    p: &mut [u8],
    mut o: usize,
    hstride: usize,
    vstride: usize,
    size: usize,
    thresh: i32,
    ithresh: i32,
    hev_thresh: i32,
) {
    let thresh2 = 2 * thresh + 1;
    for _ in 0..size {
        if needs_filter2(p, o, hstride, thresh2, ithresh) {
            if hev(p, o, hstride, hev_thresh) {
                do_filter2(p, o, hstride);
            } else {
                do_filter6(p, o, hstride);
            }
        }
        o += vstride;
    }
}

/// Port of `FilterLoop24_C` (the inner-edge filter: `DoFilter4` or `DoFilter2`).
fn filter_loop24(
    p: &mut [u8],
    mut o: usize,
    hstride: usize,
    vstride: usize,
    size: usize,
    thresh: i32,
    ithresh: i32,
    hev_thresh: i32,
) {
    let thresh2 = 2 * thresh + 1;
    for _ in 0..size {
        if needs_filter2(p, o, hstride, thresh2, ithresh) {
            if hev(p, o, hstride, hev_thresh) {
                do_filter2(p, o, hstride);
            } else {
                do_filter4(p, o, hstride);
            }
        }
        o += vstride;
    }
}

/// Port of `VFilter16_C`.
#[doc(alias = "VP8VFilter16")]
pub fn v_filter16(
    p: &mut [u8],
    o: usize,
    stride: usize,
    thresh: i32,
    ithresh: i32,
    hev_thresh: i32,
) {
    filter_loop26(p, o, stride, 1, 16, thresh, ithresh, hev_thresh);
}

/// Port of `HFilter16_C`.
#[doc(alias = "VP8HFilter16")]
pub fn h_filter16(
    p: &mut [u8],
    o: usize,
    stride: usize,
    thresh: i32,
    ithresh: i32,
    hev_thresh: i32,
) {
    filter_loop26(p, o, 1, stride, 16, thresh, ithresh, hev_thresh);
}

/// Port of `VFilter16i_C`.
#[doc(alias = "VP8VFilter16i")]
pub fn v_filter16i(
    p: &mut [u8],
    mut o: usize,
    stride: usize,
    thresh: i32,
    ithresh: i32,
    hev_thresh: i32,
) {
    for _ in 0..3 {
        o += 4 * stride;
        filter_loop24(p, o, stride, 1, 16, thresh, ithresh, hev_thresh);
    }
}

/// Port of `HFilter16i_C`.
#[doc(alias = "VP8HFilter16i")]
pub fn h_filter16i(
    p: &mut [u8],
    mut o: usize,
    stride: usize,
    thresh: i32,
    ithresh: i32,
    hev_thresh: i32,
) {
    for _ in 0..3 {
        o += 4;
        filter_loop24(p, o, 1, stride, 16, thresh, ithresh, hev_thresh);
    }
}

/// Port of `VFilter8_C` (u and v planes at offsets `ou` and `ov`).
#[doc(alias = "VP8VFilter8")]
pub fn v_filter8(
    u: &mut [u8],
    v: &mut [u8],
    o: usize,
    stride: usize,
    thresh: i32,
    ithresh: i32,
    hev_thresh: i32,
) {
    filter_loop26(u, o, stride, 1, 8, thresh, ithresh, hev_thresh);
    filter_loop26(v, o, stride, 1, 8, thresh, ithresh, hev_thresh);
}

/// Port of `HFilter8_C`.
#[doc(alias = "VP8HFilter8")]
pub fn h_filter8(
    u: &mut [u8],
    v: &mut [u8],
    o: usize,
    stride: usize,
    thresh: i32,
    ithresh: i32,
    hev_thresh: i32,
) {
    filter_loop26(u, o, 1, stride, 8, thresh, ithresh, hev_thresh);
    filter_loop26(v, o, 1, stride, 8, thresh, ithresh, hev_thresh);
}

/// Port of `VFilter8i_C`.
#[doc(alias = "VP8VFilter8i")]
pub fn v_filter8i(
    u: &mut [u8],
    v: &mut [u8],
    o: usize,
    stride: usize,
    thresh: i32,
    ithresh: i32,
    hev_thresh: i32,
) {
    filter_loop24(u, o + 4 * stride, stride, 1, 8, thresh, ithresh, hev_thresh);
    filter_loop24(v, o + 4 * stride, stride, 1, 8, thresh, ithresh, hev_thresh);
}

/// Port of `HFilter8i_C`.
#[doc(alias = "VP8HFilter8i")]
pub fn h_filter8i(
    u: &mut [u8],
    v: &mut [u8],
    o: usize,
    stride: usize,
    thresh: i32,
    ithresh: i32,
    hev_thresh: i32,
) {
    filter_loop24(u, o + 4, 1, stride, 8, thresh, ithresh, hev_thresh);
    filter_loop24(v, o + 4, 1, stride, 8, thresh, ithresh, hev_thresh);
}
