// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the C path of the libwebp 1.4.0 encoder DSP (`src/dsp/enc.c`, the functions that
//! `VP8EncDspInit` selects without SIMD): the forward and inverse transforms (`FTransform`,
//! `ITransform`, `FTransformWHT`), the intra predictors (`Intra16Preds`, `IntraChromaPreds`,
//! `Intra4Preds`), the metrics (`SSE*`, `TDisto*`, `Mean16x4`), `QuantizeBlock` and the copies,
//! and `CollectHistogram` (`src/dsp/enc.c`, with `VP8SetHistogramData`).
//!
//! Blocks are `BPS`-stride (32) work buffers. Each function takes slices that start at the block
//! origin (`dst[0]` is the C `dst`), so `dst[x + y * BPS]` is the C `DST(x, y)`. The predictors
//! read their top and left samples with the C negative indices (`top[-1]`, `left[-1]`), which are
//! addressed as `(buffer, base)` pairs: sample `k` is `buffer[base + k]`.
//!
//! The SSE2 and NEON kernels are not ported: the differential contract is the C path (see
//! `docs/design/codecs.md` §7).

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::needless_range_loop,
    clippy::identity_op,
    clippy::erasing_op
)]

/// Port of `BPS`: the common stride of the encoder and decoder work buffers.
pub const BPS: usize = 32;

/// Port of `MAX_COEFF_THRESH`: the size of the histogram of `CollectHistogram`, minus one.
pub const MAX_COEFF_THRESH: usize = 31;

/// Port of `MAX_LEVEL` (`vp8i_enc.h`).
pub const MAX_LEVEL: i32 = 2047;

/// Port of `QFIX` (`vp8i_enc.h`).
pub const QFIX: u32 = 17;

/// Port of `VP8DspScan` (`src/dsp/enc.c`): the offsets of the 16 luma and 4+4 chroma 4x4 blocks
/// in the `BPS`-stride work buffer.
pub const VP8_DSP_SCAN: [usize; 24] = [
    0 + 0 * BPS,
    4 + 0 * BPS,
    8 + 0 * BPS,
    12 + 0 * BPS,
    0 + 4 * BPS,
    4 + 4 * BPS,
    8 + 4 * BPS,
    12 + 4 * BPS,
    0 + 8 * BPS,
    4 + 8 * BPS,
    8 + 8 * BPS,
    12 + 8 * BPS,
    0 + 12 * BPS,
    4 + 12 * BPS,
    8 + 12 * BPS,
    12 + 12 * BPS,
    0 + 0 * BPS,
    4 + 0 * BPS,
    0 + 4 * BPS,
    4 + 4 * BPS, // U
    8 + 0 * BPS,
    12 + 0 * BPS,
    8 + 4 * BPS,
    12 + 4 * BPS, // V
];

/// Port of `kZigzag`.
const K_ZIGZAG: [usize; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];

/// Port of `WEBP_TRANSFORM_AC3_C1`.
const AC3_C1: i32 = 20091;
/// Port of `WEBP_TRANSFORM_AC3_C2`.
const AC3_C2: i32 = 35468;

/// Port of `WEBP_TRANSFORM_AC3_MUL1`.
#[inline]
fn transform_ac3_mul1(a: i32) -> i32 {
    ((a * AC3_C1) >> 16) + a
}

/// Port of `WEBP_TRANSFORM_AC3_MUL2`.
#[inline]
fn transform_ac3_mul2(a: i32) -> i32 {
    (a * AC3_C2) >> 16
}

/// Port of `clip_8b` (`src/dsp/enc.c`): clips to `[0, 255]`.
#[inline]
#[must_use]
pub fn clip_8b(v: i32) -> u8 {
    if (v & !0xff) == 0 {
        v as u8
    } else if v < 0 {
        0
    } else {
        255
    }
}

/// Port of `VP8Histogram` (`src/dsp/dsp.h`): only the maximum and the last non-zero bin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VP8Histogram {
    /// `max_value`.
    pub max_value: i32,
    /// `last_non_zero`.
    pub last_non_zero: i32,
}

/// Port of `VP8SetHistogramData`.
#[must_use]
pub fn set_histogram_data(distribution: &[i32; MAX_COEFF_THRESH + 1]) -> VP8Histogram {
    let mut max_value = 0;
    let mut last_non_zero = 1;
    for k in 0..=MAX_COEFF_THRESH {
        let value = distribution[k];
        if value > 0 {
            if value > max_value {
                max_value = value;
            }
            last_non_zero = k as i32;
        }
    }
    VP8Histogram {
        max_value,
        last_non_zero,
    }
}

/// Port of `CollectHistogram_C`: the histogram of the quantizable coefficient magnitudes of the
/// blocks `start_block..end_block` of `ref` against `pred`. Both slices start at the macroblock
/// origin.
#[must_use]
pub fn collect_histogram(
    ref_: &[u8],
    pred: &[u8],
    start_block: usize,
    end_block: usize,
) -> VP8Histogram {
    let mut distribution = [0i32; MAX_COEFF_THRESH + 1];
    for j in start_block..end_block {
        let mut out = [0i16; 16];
        ftransform(&ref_[VP8_DSP_SCAN[j]..], &pred[VP8_DSP_SCAN[j]..], &mut out);
        for k in 0..16 {
            let v = i32::from(out[k]).abs() >> 3;
            let clipped_value = if v > MAX_COEFF_THRESH as i32 {
                MAX_COEFF_THRESH as i32
            } else {
                v
            };
            distribution[clipped_value as usize] += 1;
        }
    }
    set_histogram_data(&distribution)
}

/// Port of `ITransformOne` (`src/dsp/enc.c`): the inverse transform of one 4x4 block, added to
/// the prediction `ref_` and written to `dst`.
fn itransform_one(ref_: &[u8], input: &[i16], dst: &mut [u8]) {
    let mut c = [0i32; 4 * 4];
    let mut tmp = 0usize;
    for i in 0..4 {
        // vertical pass
        let a = i32::from(input[i]) + i32::from(input[8 + i]);
        let b = i32::from(input[i]) - i32::from(input[8 + i]);
        let c1 = transform_ac3_mul2(i32::from(input[4 + i]))
            - transform_ac3_mul1(i32::from(input[12 + i]));
        let d = transform_ac3_mul1(i32::from(input[4 + i]))
            + transform_ac3_mul2(i32::from(input[12 + i]));
        c[tmp] = a + d;
        c[tmp + 1] = b + c1;
        c[tmp + 2] = b - c1;
        c[tmp + 3] = a - d;
        tmp += 4;
    }
    for i in 0..4 {
        // horizontal pass
        let dc = c[i] + 4;
        let a = dc + c[8 + i];
        let b = dc - c[8 + i];
        let c1 = transform_ac3_mul2(c[4 + i]) - transform_ac3_mul1(c[12 + i]);
        let d = transform_ac3_mul1(c[4 + i]) + transform_ac3_mul2(c[12 + i]);
        // STORE(x, y, v): dst[x + y * BPS] = clip_8b(ref[x + y * BPS] + (v >> 3))
        let store = |dst: &mut [u8], x: usize, v: i32| {
            let p = x + i * BPS;
            dst[p] = clip_8b(i32::from(ref_[p]) + (v >> 3));
        };
        store(dst, 0, a + d);
        store(dst, 1, b + c1);
        store(dst, 2, b - c1);
        store(dst, 3, a - d);
    }
}

/// Port of `ITransform_C`: the inverse transform of one block, and of the next one when
/// `do_two`. `ref_` and `dst` start at the block origin, `input` holds the 16 coefficients of the
/// block (and 16 more for `do_two`).
pub fn itransform(ref_: &[u8], input: &[i16], dst: &mut [u8], do_two: bool) {
    itransform_one(ref_, input, dst);
    if do_two {
        itransform_one(&ref_[4..], &input[16..], &mut dst[4..]);
    }
}

/// Port of `FTransform_C`: the forward transform of the residual `src - ref` of one 4x4 block.
pub fn ftransform(src: &[u8], ref_: &[u8], out: &mut [i16]) {
    let mut tmp = [0i32; 16];
    for i in 0..4 {
        let s = &src[i * BPS..];
        let r = &ref_[i * BPS..];
        let d0 = i32::from(s[0]) - i32::from(r[0]); // 9bit dynamic range ([-255,255])
        let d1 = i32::from(s[1]) - i32::from(r[1]);
        let d2 = i32::from(s[2]) - i32::from(r[2]);
        let d3 = i32::from(s[3]) - i32::from(r[3]);
        let a0 = d0 + d3; // 10b [-510,510]
        let a1 = d1 + d2;
        let a2 = d1 - d2;
        let a3 = d0 - d3;
        tmp[4 * i] = (a0 + a1) * 8; // 14b [-8160,8160]
        tmp[1 + 4 * i] = (a2 * 2217 + a3 * 5352 + 1812) >> 9; // [-7536,7542]
        tmp[2 + 4 * i] = (a0 - a1) * 8;
        tmp[3 + 4 * i] = (a3 * 2217 - a2 * 5352 + 937) >> 9;
    }
    for i in 0..4 {
        let a0 = tmp[i] + tmp[12 + i]; // 15b
        let a1 = tmp[4 + i] + tmp[8 + i];
        let a2 = tmp[4 + i] - tmp[8 + i];
        let a3 = tmp[i] - tmp[12 + i];
        out[i] = ((a0 + a1 + 7) >> 4) as i16; // 12b
        out[4 + i] = (((a2 * 2217 + a3 * 5352 + 12000) >> 16) + i32::from(a3 != 0)) as i16;
        out[8 + i] = ((a0 - a1 + 7) >> 4) as i16;
        out[12 + i] = ((a3 * 2217 - a2 * 5352 + 51000) >> 16) as i16;
    }
}

/// Port of `FTransform2_C`: the forward transform of two horizontally adjacent blocks.
pub fn ftransform2(src: &[u8], ref_: &[u8], out: &mut [i16]) {
    ftransform(src, ref_, out);
    ftransform(&src[4..], &ref_[4..], &mut out[16..]);
}

/// Port of `FTransformWHT_C`: the Walsh-Hadamard transform of the 16 luma DC values.
pub fn ftransform_wht(in_: &[i16], out: &mut [i16]) {
    let mut tmp = [0i32; 16];
    for i in 0..4 {
        let base = 64 * i;
        let a0 = i32::from(in_[base]) + i32::from(in_[base + 2 * 16]); // 13b
        let a1 = i32::from(in_[base + 16]) + i32::from(in_[base + 3 * 16]);
        let a2 = i32::from(in_[base + 16]) - i32::from(in_[base + 3 * 16]);
        let a3 = i32::from(in_[base]) - i32::from(in_[base + 2 * 16]);
        tmp[4 * i] = a0 + a1; // 14b
        tmp[1 + 4 * i] = a3 + a2;
        tmp[2 + 4 * i] = a3 - a2;
        tmp[3 + 4 * i] = a0 - a1;
    }
    for i in 0..4 {
        let a0 = tmp[i] + tmp[8 + i]; // 15b
        let a1 = tmp[4 + i] + tmp[12 + i];
        let a2 = tmp[4 + i] - tmp[12 + i];
        let a3 = tmp[i] - tmp[8 + i];
        let b0 = a0 + a1; // 16b
        let b1 = a3 + a2;
        let b2 = a3 - a2;
        let b3 = a0 - a1;
        out[i] = (b0 >> 1) as i16; // 15b
        out[4 + i] = (b1 >> 1) as i16;
        out[8 + i] = (b2 >> 1) as i16;
        out[12 + i] = (b3 >> 1) as i16;
    }
}

/// Port of `Fill`: `size` rows of `size` copies of `value`.
fn fill(dst: &mut [u8], value: u8, size: usize) {
    for j in 0..size {
        for x in 0..size {
            dst[j * BPS + x] = value;
        }
    }
}

/// A sample buffer with a base index: sample `k` is `buf[base + k]`, so negative C indices are
/// valid whenever `base` is large enough. The predictors take the top and left edges this way.
#[derive(Debug, Clone, Copy)]
pub struct Edge<'a> {
    /// The buffer holding the samples.
    pub buf: &'a [u8],
    /// The index of sample 0.
    pub base: usize,
}

impl Edge<'_> {
    #[inline]
    fn at(&self, k: isize) -> u8 {
        self.buf[(self.base as isize + k) as usize]
    }
}

/// Port of `VerticalPred`: `top == NULL` is `present == false`.
fn vertical_pred(dst: &mut [u8], top: Option<Edge<'_>>, size: usize) {
    match top {
        Some(t) => {
            for j in 0..size {
                for x in 0..size {
                    dst[j * BPS + x] = t.at(x as isize);
                }
            }
        }
        None => fill(dst, 127, size),
    }
}

/// Port of `HorizontalPred`.
fn horizontal_pred(dst: &mut [u8], left: Option<Edge<'_>>, size: usize) {
    match left {
        Some(l) => {
            for j in 0..size {
                let v = l.at(j as isize);
                for x in 0..size {
                    dst[j * BPS + x] = v;
                }
            }
        }
        None => fill(dst, 129, size),
    }
}

/// Port of `TrueMotion`. The C clip table `clip1 + 255 - left[-1]`, indexed by `left[y]` and then
/// `top[x]`, is `clip_8b(left[y] + top[x] - left[-1])` exactly.
fn true_motion(dst: &mut [u8], left: Option<Edge<'_>>, top: Option<Edge<'_>>, size: usize) {
    match (left, top) {
        (Some(l), Some(t)) => {
            let corner = i32::from(l.at(-1));
            for y in 0..size {
                let ly = i32::from(l.at(y as isize));
                for x in 0..size {
                    dst[y * BPS + x] = clip_8b(ly + i32::from(t.at(x as isize)) - corner);
                }
            }
        }
        (Some(l), None) => horizontal_pred(dst, Some(l), size),
        (None, Some(t)) => vertical_pred(dst, Some(t), size),
        (None, None) => fill(dst, 129, size),
    }
}

/// Port of `DCMode`.
fn dc_mode(
    dst: &mut [u8],
    left: Option<Edge<'_>>,
    top: Option<Edge<'_>>,
    size: usize,
    round: i32,
    shift: i32,
) {
    let mut dc: i32 = 0;
    if let Some(t) = top {
        for j in 0..size {
            dc += i32::from(t.at(j as isize));
        }
        if let Some(l) = left {
            // top and left present
            for j in 0..size {
                dc += i32::from(l.at(j as isize));
            }
        } else {
            // top, but no left
            dc += dc;
        }
        dc = (dc + round) >> shift;
    } else if let Some(l) = left {
        // left but no top
        for j in 0..size {
            dc += i32::from(l.at(j as isize));
        }
        dc += dc;
        dc = (dc + round) >> shift;
    } else {
        // no top, no left, nothing.
        dc = 0x80;
    }
    fill(dst, dc as u8, size);
}

/// Port of `IntraChromaPreds_C`: the four chroma predictors for the two 8x8 blocks of U and V.
/// `top` holds the U top edge followed by the V top edge (8 samples each), and `left` the U left
/// edge at `left.base` and the V left edge 16 samples later, as the C pointer arithmetic does.
pub fn intra_chroma_preds(dst: &mut [u8], left: Option<Edge<'_>>, top: Option<Edge<'_>>) {
    dc_mode(&mut dst[C8DC8..], left, top, 8, 8, 4);
    vertical_pred(&mut dst[C8VE8..], top, 8);
    horizontal_pred(&mut dst[C8HE8..], left, 8);
    true_motion(&mut dst[C8TM8..], left, top, 8);
    let dst = &mut dst[8..];
    // `if (top != NULL) top += 8; if (left != NULL) left += 16;`
    let top = top.map(|t| Edge {
        buf: t.buf,
        base: t.base + 8,
    });
    let left = left.map(|l| Edge {
        buf: l.buf,
        base: l.base + 16,
    });
    dc_mode(&mut dst[C8DC8..], left, top, 8, 8, 4);
    vertical_pred(&mut dst[C8VE8..], top, 8);
    horizontal_pred(&mut dst[C8HE8..], left, 8);
    true_motion(&mut dst[C8TM8..], left, top, 8);
}

/// Port of `Intra16Preds_C`: the four luma 16x16 predictors.
pub fn intra16_preds(dst: &mut [u8], left: Option<Edge<'_>>, top: Option<Edge<'_>>) {
    dc_mode(&mut dst[I16DC16..], left, top, 16, 16, 5);
    vertical_pred(&mut dst[I16VE16..], top, 16);
    horizontal_pred(&mut dst[I16HE16..], left, 16);
    true_motion(&mut dst[I16TM16..], left, top, 16);
}

// Offsets of the predictor outputs in the prediction area (`vp8i_enc.h`).
pub const I16DC16: usize = 0 * 16 * BPS;
pub const I16TM16: usize = I16DC16 + 16;
pub const I16VE16: usize = 1 * 16 * BPS;
pub const I16HE16: usize = I16VE16 + 16;
pub const C8DC8: usize = 2 * 16 * BPS;
pub const C8TM8: usize = C8DC8 + 1 * 16;
pub const C8VE8: usize = 2 * 16 * BPS + 8 * BPS;
pub const C8HE8: usize = C8VE8 + 1 * 16;
pub const I4DC4: usize = 3 * 16 * BPS;
pub const I4TM4: usize = I4DC4 + 4;
pub const I4VE4: usize = I4DC4 + 8;
pub const I4HE4: usize = I4DC4 + 12;
pub const I4RD4: usize = I4DC4 + 16;
pub const I4VR4: usize = I4DC4 + 20;
pub const I4LD4: usize = I4DC4 + 24;
pub const I4VL4: usize = I4DC4 + 28;
pub const I4HD4: usize = 3 * 16 * BPS + 4 * BPS;
pub const I4HU4: usize = I4HD4 + 4;
/// Port of `I4TMP`: the scratch block of the intra 4x4 search.
pub const I4TMP: usize = I4HD4 + 8;

/// Port of `VP8I16ModeOffsets` (`quant_enc.c`): the Intra16 modes DC, TM, VE, HE.
pub const I16_MODE_OFFSETS: [usize; 4] = [I16DC16, I16TM16, I16VE16, I16HE16];
/// Port of `VP8UVModeOffsets`: the chroma modes DC, TM, VE, HE.
pub const UV_MODE_OFFSETS: [usize; 4] = [C8DC8, C8TM8, C8VE8, C8HE8];
/// Port of `VP8I4ModeOffsets`: the ten intra 4x4 modes.
pub const I4_MODE_OFFSETS: [usize; 10] = [I4DC4, I4TM4, I4VE4, I4HE4, I4RD4, I4VR4, I4LD4, I4VL4, I4HD4, I4HU4];

#[inline]
fn avg3(a: u8, b: u8, c: u8) -> u8 {
    ((u32::from(a) + 2 * u32::from(b) + u32::from(c) + 2) >> 2) as u8
}

#[inline]
fn avg2(a: u8, b: u8) -> u8 {
    ((u32::from(a) + u32::from(b) + 1) >> 1) as u8
}

/// The `DST(x, y)` of the 4x4 predictors: `dst[x + y * BPS]`.
#[inline]
fn dst_set(dst: &mut [u8], x: usize, y: usize, v: u8) {
    dst[x + y * BPS] = v;
}

/// Port of `VE4`: vertical 4x4 predictor (smoothed top row).
fn ve4(dst: &mut [u8], top: Edge<'_>) {
    let vals = [
        avg3(top.at(-1), top.at(0), top.at(1)),
        avg3(top.at(0), top.at(1), top.at(2)),
        avg3(top.at(1), top.at(2), top.at(3)),
        avg3(top.at(2), top.at(3), top.at(4)),
    ];
    for i in 0..4 {
        dst[i * BPS..i * BPS + 4].copy_from_slice(&vals);
    }
}

/// Port of `HE4`: horizontal 4x4 predictor (smoothed left column).
fn he4(dst: &mut [u8], top: Edge<'_>) {
    let x = top.at(-1);
    let i = top.at(-2);
    let j = top.at(-3);
    let k = top.at(-4);
    let l = top.at(-5);
    let rows = [avg3(x, i, j), avg3(i, j, k), avg3(j, k, l), avg3(k, l, l)];
    for (r, v) in rows.iter().enumerate() {
        dst[r * BPS..r * BPS + 4].fill(*v);
    }
}

/// Port of `DC4`.
fn dc4(dst: &mut [u8], top: Edge<'_>) {
    let mut dc: u32 = 4;
    for i in 0..4 {
        dc += u32::from(top.at(i as isize)) + u32::from(top.at(-5 + i as isize));
    }
    fill(dst, (dc >> 3) as u8, 4);
}

/// Port of `RD4`.
fn rd4(dst: &mut [u8], top: Edge<'_>) {
    let x = top.at(-1);
    let i = top.at(-2);
    let j = top.at(-3);
    let k = top.at(-4);
    let l = top.at(-5);
    let a = top.at(0);
    let b = top.at(1);
    let c = top.at(2);
    let d = top.at(3);
    dst_set(dst, 0, 3, avg3(j, k, l));
    let v = avg3(i, j, k);
    dst_set(dst, 0, 2, v);
    dst_set(dst, 1, 3, v);
    let v = avg3(x, i, j);
    dst_set(dst, 0, 1, v);
    dst_set(dst, 1, 2, v);
    dst_set(dst, 2, 3, v);
    let v = avg3(a, x, i);
    dst_set(dst, 0, 0, v);
    dst_set(dst, 1, 1, v);
    dst_set(dst, 2, 2, v);
    dst_set(dst, 3, 3, v);
    let v = avg3(b, a, x);
    dst_set(dst, 1, 0, v);
    dst_set(dst, 2, 1, v);
    dst_set(dst, 3, 2, v);
    let v = avg3(c, b, a);
    dst_set(dst, 2, 0, v);
    dst_set(dst, 3, 1, v);
    dst_set(dst, 3, 0, avg3(d, c, b));
}

/// Port of `LD4`.
fn ld4(dst: &mut [u8], top: Edge<'_>) {
    let a = top.at(0);
    let b = top.at(1);
    let c = top.at(2);
    let d = top.at(3);
    let e = top.at(4);
    let f = top.at(5);
    let g = top.at(6);
    let h = top.at(7);
    dst_set(dst, 0, 0, avg3(a, b, c));
    let v = avg3(b, c, d);
    dst_set(dst, 1, 0, v);
    dst_set(dst, 0, 1, v);
    let v = avg3(c, d, e);
    dst_set(dst, 2, 0, v);
    dst_set(dst, 1, 1, v);
    dst_set(dst, 0, 2, v);
    let v = avg3(d, e, f);
    dst_set(dst, 3, 0, v);
    dst_set(dst, 2, 1, v);
    dst_set(dst, 1, 2, v);
    dst_set(dst, 0, 3, v);
    let v = avg3(e, f, g);
    dst_set(dst, 3, 1, v);
    dst_set(dst, 2, 2, v);
    dst_set(dst, 1, 3, v);
    let v = avg3(f, g, h);
    dst_set(dst, 3, 2, v);
    dst_set(dst, 2, 3, v);
    dst_set(dst, 3, 3, avg3(g, h, h));
}

/// Port of `VR4`.
fn vr4(dst: &mut [u8], top: Edge<'_>) {
    let x = top.at(-1);
    let i = top.at(-2);
    let j = top.at(-3);
    let k = top.at(-4);
    let a = top.at(0);
    let b = top.at(1);
    let c = top.at(2);
    let d = top.at(3);
    let v = avg2(x, a);
    dst_set(dst, 0, 0, v);
    dst_set(dst, 1, 2, v);
    let v = avg2(a, b);
    dst_set(dst, 1, 0, v);
    dst_set(dst, 2, 2, v);
    let v = avg2(b, c);
    dst_set(dst, 2, 0, v);
    dst_set(dst, 3, 2, v);
    dst_set(dst, 3, 0, avg2(c, d));
    dst_set(dst, 0, 3, avg3(k, j, i));
    dst_set(dst, 0, 2, avg3(j, i, x));
    let v = avg3(i, x, a);
    dst_set(dst, 0, 1, v);
    dst_set(dst, 1, 3, v);
    let v = avg3(x, a, b);
    dst_set(dst, 1, 1, v);
    dst_set(dst, 2, 3, v);
    let v = avg3(a, b, c);
    dst_set(dst, 2, 1, v);
    dst_set(dst, 3, 3, v);
    dst_set(dst, 3, 1, avg3(b, c, d));
}

/// Port of `VL4`.
fn vl4(dst: &mut [u8], top: Edge<'_>) {
    let a = top.at(0);
    let b = top.at(1);
    let c = top.at(2);
    let d = top.at(3);
    let e = top.at(4);
    let f = top.at(5);
    let g = top.at(6);
    let h = top.at(7);
    dst_set(dst, 0, 0, avg2(a, b));
    let v = avg2(b, c);
    dst_set(dst, 1, 0, v);
    dst_set(dst, 0, 2, v);
    let v = avg2(c, d);
    dst_set(dst, 2, 0, v);
    dst_set(dst, 1, 2, v);
    let v = avg2(d, e);
    dst_set(dst, 3, 0, v);
    dst_set(dst, 2, 2, v);
    dst_set(dst, 0, 1, avg3(a, b, c));
    let v = avg3(b, c, d);
    dst_set(dst, 1, 1, v);
    dst_set(dst, 0, 3, v);
    let v = avg3(c, d, e);
    dst_set(dst, 2, 1, v);
    dst_set(dst, 1, 3, v);
    let v = avg3(d, e, f);
    dst_set(dst, 3, 1, v);
    dst_set(dst, 2, 3, v);
    dst_set(dst, 3, 2, avg3(e, f, g));
    dst_set(dst, 3, 3, avg3(f, g, h));
}

/// Port of `HU4`.
fn hu4(dst: &mut [u8], top: Edge<'_>) {
    let i = top.at(-2);
    let j = top.at(-3);
    let k = top.at(-4);
    let l = top.at(-5);
    dst_set(dst, 0, 0, avg2(i, j));
    let v = avg2(j, k);
    dst_set(dst, 2, 0, v);
    dst_set(dst, 0, 1, v);
    let v = avg2(k, l);
    dst_set(dst, 2, 1, v);
    dst_set(dst, 0, 2, v);
    dst_set(dst, 1, 0, avg3(i, j, k));
    let v = avg3(j, k, l);
    dst_set(dst, 3, 0, v);
    dst_set(dst, 1, 1, v);
    let v = avg3(k, l, l);
    dst_set(dst, 3, 1, v);
    dst_set(dst, 1, 2, v);
    dst_set(dst, 3, 2, l);
    dst_set(dst, 2, 2, l);
    dst_set(dst, 0, 3, l);
    dst_set(dst, 1, 3, l);
    dst_set(dst, 2, 3, l);
    dst_set(dst, 3, 3, l);
}

/// Port of `HD4`.
fn hd4(dst: &mut [u8], top: Edge<'_>) {
    let x = top.at(-1);
    let i = top.at(-2);
    let j = top.at(-3);
    let k = top.at(-4);
    let l = top.at(-5);
    let a = top.at(0);
    let b = top.at(1);
    let c = top.at(2);
    let v = avg2(i, x);
    dst_set(dst, 0, 0, v);
    dst_set(dst, 2, 1, v);
    let v = avg2(j, i);
    dst_set(dst, 0, 1, v);
    dst_set(dst, 2, 2, v);
    let v = avg2(k, j);
    dst_set(dst, 0, 2, v);
    dst_set(dst, 2, 3, v);
    dst_set(dst, 0, 3, avg2(l, k));
    dst_set(dst, 3, 0, avg3(a, b, c));
    dst_set(dst, 2, 0, avg3(x, a, b));
    let v = avg3(i, x, a);
    dst_set(dst, 1, 0, v);
    dst_set(dst, 3, 1, v);
    let v = avg3(j, i, x);
    dst_set(dst, 1, 1, v);
    dst_set(dst, 3, 2, v);
    let v = avg3(k, j, i);
    dst_set(dst, 1, 2, v);
    dst_set(dst, 3, 3, v);
    dst_set(dst, 1, 3, avg3(l, k, j));
}

/// Port of `TM4`.
fn tm4(dst: &mut [u8], top: Edge<'_>) {
    let corner = i32::from(top.at(-1));
    for y in 0..4 {
        let t = i32::from(top.at(-2 - y as isize));
        for x in 0..4 {
            dst[y * BPS + x] = clip_8b(t + i32::from(top.at(x as isize)) - corner);
        }
    }
}

/// Port of `Intra4Preds_C`: the ten 4x4 luma predictors. `top` holds the boundary samples
/// (`top[-5..=7]`, with `top[-1]` the corner) of the block.
pub fn intra4_preds(dst: &mut [u8], top: Edge<'_>) {
    dc4(&mut dst[I4DC4..], top);
    tm4(&mut dst[I4TM4..], top);
    ve4(&mut dst[I4VE4..], top);
    he4(&mut dst[I4HE4..], top);
    rd4(&mut dst[I4RD4..], top);
    vr4(&mut dst[I4VR4..], top);
    ld4(&mut dst[I4LD4..], top);
    vl4(&mut dst[I4VL4..], top);
    hd4(&mut dst[I4HD4..], top);
    hu4(&mut dst[I4HU4..], top);
}

/// Port of `GetSSE`: the squared error of a `w x h` block.
#[must_use]
pub fn get_sse(a: &[u8], b: &[u8], w: usize, h: usize) -> i32 {
    let mut count = 0i32;
    for y in 0..h {
        for x in 0..w {
            let diff = i32::from(a[y * BPS + x]) - i32::from(b[y * BPS + x]);
            count += diff * diff;
        }
    }
    count
}

/// Port of `SSE16x16_C`.
#[must_use]
pub fn sse16x16(a: &[u8], b: &[u8]) -> i32 {
    get_sse(a, b, 16, 16)
}

/// Port of `SSE16x8_C`.
#[must_use]
pub fn sse16x8(a: &[u8], b: &[u8]) -> i32 {
    get_sse(a, b, 16, 8)
}

/// Port of `SSE8x8_C`.
#[must_use]
pub fn sse8x8(a: &[u8], b: &[u8]) -> i32 {
    get_sse(a, b, 8, 8)
}

/// Port of `SSE4x4_C`.
#[must_use]
pub fn sse4x4(a: &[u8], b: &[u8]) -> i32 {
    get_sse(a, b, 4, 4)
}

/// Port of `Mean16x4_C`: the sums of the four 4x4 blocks of a 16x4 strip.
#[must_use]
pub fn mean16x4(ref_: &[u8]) -> [u32; 4] {
    let mut dc = [0u32; 4];
    for k in 0..4 {
        let mut avg: u32 = 0;
        for y in 0..4 {
            for x in 0..4 {
                avg += u32::from(ref_[x + y * BPS + 4 * k]);
            }
        }
        dc[k] = avg;
    }
    dc
}

/// Port of `TTransform`: the weighted Hadamard sum of a 4x4 block.
fn ttransform(input: &[u8], w: &[u16]) -> i32 {
    let mut sum = 0i32;
    let mut tmp = [0i32; 16];
    for i in 0..4 {
        let inp = &input[i * BPS..];
        let a0 = i32::from(inp[0]) + i32::from(inp[2]);
        let a1 = i32::from(inp[1]) + i32::from(inp[3]);
        let a2 = i32::from(inp[1]) - i32::from(inp[3]);
        let a3 = i32::from(inp[0]) - i32::from(inp[2]);
        tmp[4 * i] = a0 + a1;
        tmp[1 + 4 * i] = a3 + a2;
        tmp[2 + 4 * i] = a3 - a2;
        tmp[3 + 4 * i] = a0 - a1;
    }
    for i in 0..4 {
        let a0 = tmp[i] + tmp[8 + i];
        let a1 = tmp[4 + i] + tmp[12 + i];
        let a2 = tmp[4 + i] - tmp[12 + i];
        let a3 = tmp[i] - tmp[8 + i];
        let b0 = a0 + a1;
        let b1 = a3 + a2;
        let b2 = a3 - a2;
        let b3 = a0 - a1;
        sum += i32::from(w[i]) * b0.abs();
        sum += i32::from(w[4 + i]) * b1.abs();
        sum += i32::from(w[8 + i]) * b2.abs();
        sum += i32::from(w[12 + i]) * b3.abs();
    }
    sum
}

/// Port of `Disto4x4_C`: the spectral distortion between two 4x4 blocks.
#[must_use]
pub fn disto4x4(a: &[u8], b: &[u8], w: &[u16]) -> i32 {
    let sum1 = ttransform(a, w);
    let sum2 = ttransform(b, w);
    (sum2 - sum1).abs() >> 5
}

/// Port of `Disto16x16_C`: the spectral distortion of a 16x16 block, as 4x4 sub-blocks.
#[must_use]
pub fn disto16x16(a: &[u8], b: &[u8], w: &[u16]) -> i32 {
    let mut d = 0i32;
    let mut y = 0;
    while y < 16 * BPS {
        let mut x = 0;
        while x < 16 {
            d += disto4x4(&a[x + y..], &b[x + y..], w);
            x += 4;
        }
        y += 4 * BPS;
    }
    d
}

/// Port of `QuantizeBlock_C` (also the Walsh-Hadamard quantizer): quantizes the 16 coefficients
/// of `input` in zigzag order into `out` (the levels), and stores the dequantized coefficients
/// back into `input`. Returns `true` when some level is non-zero.
#[must_use]
pub fn quantize_block(input: &mut [i16], out: &mut [i16], mtx: &VP8Matrix) -> bool {
    let mut last: i32 = -1;
    for n in 0..16 {
        let j = K_ZIGZAG[n];
        let sign = i32::from(input[j]) < 0;
        let coeff = (if sign { -i32::from(input[j]) } else { i32::from(input[j]) } as u32)
            .wrapping_add(u32::from(mtx.sharpen[j]));
        if coeff > mtx.zthresh[j] {
            let q = u32::from(mtx.q[j]);
            let i_q = u32::from(mtx.iq[j]);
            let b = mtx.bias[j];
            let mut level = (coeff.wrapping_mul(i_q).wrapping_add(b) >> QFIX) as i32;
            if level > MAX_LEVEL {
                level = MAX_LEVEL;
            }
            if sign {
                level = -level;
            }
            input[j] = (level * (q as i32)) as i16;
            out[n] = level as i16;
            if level != 0 {
                last = n as i32;
            }
        } else {
            out[n] = 0;
            input[j] = 0;
        }
    }
    last >= 0
}

/// Port of `Quantize2Blocks_C`: quantizes two adjacent blocks; bit `i` of the result is set when
/// block `i` has a non-zero level.
#[must_use]
pub fn quantize2_blocks(input: &mut [i16], out: &mut [i16], mtx: &VP8Matrix) -> u32 {
    let mut nz = u32::from(quantize_block(&mut input[0..16], &mut out[0..16], mtx)) << 0;
    nz |= u32::from(quantize_block(&mut input[16..32], &mut out[16..32], mtx)) << 1;
    nz
}

/// Port of `VP8Matrix` (`vp8i_enc.h`): the quantizer of one coefficient type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VP8Matrix {
    /// `q_`: quantizer steps.
    pub q: [u16; 16],
    /// `iq_`: reciprocals, fixed point.
    pub iq: [u16; 16],
    /// `bias_`: rounding bias.
    pub bias: [u32; 16],
    /// `zthresh_`: value below which a coefficient is zeroed.
    pub zthresh: [u32; 16],
    /// `sharpen_`: frequency boosters for slight sharpening.
    pub sharpen: [u16; 16],
}

/// Port of `Copy`: copies a `w x h` block.
fn copy_block(src: &[u8], dst: &mut [u8], w: usize, h: usize) {
    for y in 0..h {
        dst[y * BPS..y * BPS + w].copy_from_slice(&src[y * BPS..y * BPS + w]);
    }
}

/// Port of `Copy4x4_C`.
pub fn copy4x4(src: &[u8], dst: &mut [u8]) {
    copy_block(src, dst, 4, 4);
}

/// Port of `Copy16x8_C`.
pub fn copy16x8(src: &[u8], dst: &mut [u8]) {
    copy_block(src, dst, 16, 8);
}
