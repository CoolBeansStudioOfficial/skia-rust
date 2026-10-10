// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the macroblock mode decision of libwebp 1.4.0 (`src/enc/quant_enc.c`) on the path
//! `SkWebpEncoder` takes: `method = 3`, so `rd_opt = RD_OPT_BASIC`, `do_trellis_ = 0`, and
//! `VP8Decimate` runs `PickBestIntra16`, `PickBestIntra4` and `PickBestUV`.
//!
//! Not ported, because `method = 3` never reaches them: `TrellisQuantizeBlock` (the trellis
//! branches of `ReconstructIntra16/4` and `ReconstructUV`, `DO_TRELLIS_UV` is 0 anyway),
//! `SimpleQuantize` (`RD_OPT_TRELLIS`), and `RefineUsingDistortion` (`RD_OPT_NONE`, `method < 3`).
//!
//! The C swaps the `yuv_out_` and `yuv_out2_` pointers and the `dst`/`tmp_dst` pointers; the port
//! swaps the offsets into the iterator's work area, which are the same pointers.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::too_many_lines,
    clippy::too_many_arguments
)]

use super::vp8_cost::{
    VP8Residual, fixed_costs_i16, fixed_costs_i4, fixed_costs_uv, get_residual_cost,
    init_residual, set_residual_coeffs,
};
use super::vp8_enc_dsp::{
    BPS, I4_MODE_OFFSETS, I4TMP, I16_MODE_OFFSETS, UV_MODE_OFFSETS, VP8Matrix, copy16x8,
    copy4x4, disto16x16, disto4x4, ftransform, ftransform2, ftransform_wht, intra4_preds,
    itransform, quantize2_blocks, quantize_block, sse16x16, sse16x8, sse4x4, Edge,
};
use super::vp8_encoder::{U_OFF_ENC, VP8EncIterator, VP8Encoder, VP8_SCAN, Y_OFF_ENC};
use super::vp8_quant::WEIGHT_Y;

/// Port of `MAX_COST`.
const MAX_COST: i64 = 0x7fff_ffff_ffff_ff;
/// Port of `RD_DISTO_MULT`.
const RD_DISTO_MULT: i64 = 256;
/// Port of `FLATNESS_LIMIT_I16`.
const FLATNESS_LIMIT_I16: i32 = 0;
/// Port of `FLATNESS_LIMIT_I4`.
const FLATNESS_LIMIT_I4: i32 = 3;
/// Port of `FLATNESS_LIMIT_UV`.
const FLATNESS_LIMIT_UV: i32 = 2;
/// Port of `FLATNESS_PENALTY`.
const FLATNESS_PENALTY: i32 = 140;
/// Port of `VP8ScanUV`.
const VP8_SCAN_UV: [usize; 8] = [
    0 + 0 * BPS,
    4 + 0 * BPS,
    0 + 4 * BPS,
    4 + 4 * BPS,
    8 + 0 * BPS,
    12 + 0 * BPS,
    8 + 4 * BPS,
    12 + 4 * BPS,
];
/// Port of `C1` (the error sent to the block below).
const C1: i32 = 7;
/// Port of `C2` (the error sent to the block on the right).
const C2: i32 = 8;
/// Port of `DSHIFT`.
const DSHIFT: i32 = 4;
/// Port of `DSCALE`.
const DSCALE: i32 = 1;

/// Port of `MULT_8B(a, b)`.
#[inline]
fn mult_8b(a: i32, b: i32) -> i32 {
    (a * b + 128) >> 8
}

/// Port of `VP8ModeScore` (`vp8i_enc.h`): the score and the levels of one mode decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeScore {
    /// `D`: distortion.
    pub d: i64,
    /// `SD`: spectral distortion.
    pub sd: i64,
    /// `H`: header bits.
    pub h: i64,
    /// `R`: rate.
    pub r: i64,
    /// `score`.
    pub score: i64,
    /// `y_dc_levels`.
    pub y_dc_levels: [i16; 16],
    /// `y_ac_levels[16][16]`.
    pub y_ac_levels: [[i16; 16]; 16],
    /// `uv_levels[8][16]`.
    pub uv_levels: [[i16; 16]; 8],
    /// `mode_i16`.
    pub mode_i16: i32,
    /// `modes_i4[16]`.
    pub modes_i4: [u8; 16],
    /// `mode_uv`.
    pub mode_uv: i32,
    /// `nz`: the non-zero blocks.
    pub nz: u32,
    /// `derr[2][3]`: the DC diffusion errors of U and V.
    pub derr: [[i8; 3]; 2],
}

impl Default for ModeScore {
    fn default() -> Self {
        Self {
            d: 0,
            sd: 0,
            h: 0,
            r: 0,
            score: 0,
            y_dc_levels: [0; 16],
            y_ac_levels: [[0; 16]; 16],
            uv_levels: [[0; 16]; 8],
            mode_i16: 0,
            modes_i4: [0; 16],
            mode_uv: 0,
            nz: 0,
            derr: [[0; 3]; 2],
        }
    }
}

/// Port of `InitScore`.
fn init_score(rd: &mut ModeScore) {
    rd.d = 0;
    rd.sd = 0;
    rd.r = 0;
    rd.h = 0;
    rd.nz = 0;
    rd.score = MAX_COST;
}

/// Port of `CopyScore`: the score fields (not the levels).
fn copy_score(dst: &mut ModeScore, src: &ModeScore) {
    dst.d = src.d;
    dst.sd = src.sd;
    dst.r = src.r;
    dst.h = src.h;
    dst.nz = src.nz; // note that nz is not accumulated, but just copied.
    dst.score = src.score;
}

/// Port of `AddScore`.
fn add_score(dst: &mut ModeScore, src: &ModeScore) {
    dst.d += src.d;
    dst.sd += src.sd;
    dst.r += src.r;
    dst.h += src.h;
    dst.nz |= src.nz; // here, new nz bits are accumulated.
    dst.score += src.score;
}

/// Port of `SetRDScore`.
fn set_rd_score(lambda: i32, rd: &mut ModeScore) {
    rd.score = (rd.r + rd.h) * i64::from(lambda) + RD_DISTO_MULT * (rd.d + rd.sd);
}

/// Port of `IsFlat` (the C path of `dsp/quant.h`): at most `thresh` non-zero AC levels in
/// `num_blocks` blocks of `levels`.
fn is_flat(levels: &[i16], num_blocks: usize, thresh: i32) -> bool {
    let mut score = 0i32;
    for b in 0..num_blocks {
        for i in 1..16 {
            // omit DC, we're only interested in AC
            score += i32::from(levels[16 * b + i] != 0);
            if score > thresh {
                return false;
            }
        }
    }
    true
}

/// Port of `IsFlatSource16`: every sample of the 16x16 block at `src` equals its first sample.
fn is_flat_source16(yuv: &[u8], src: usize) -> bool {
    let v = yuv[src];
    for i in 0..16 {
        for x in 0..16 {
            if yuv[src + i * BPS + x] != v {
                return false;
            }
        }
    }
    true
}

/// A copy of the `BPS`-stride window at `off` of `yuv` (up to 128 bytes, zero past the end), so
/// that a prediction and its output can be borrowed from the same work area.
fn window(yuv: &[u8], off: usize) -> [u8; 128] {
    let mut w = [0u8; 128];
    let n = (yuv.len().saturating_sub(off)).min(128);
    w[..n].copy_from_slice(&yuv[off..off + n]);
    w
}

/// Port of `TransformWHT_C` (`src/dsp/dec.c`): the inverse Walsh-Hadamard transform of the 16
/// DC values, written to the DC position of the 16 blocks (`out[16 * block]`).
fn transform_wht(in_: &[i16], out: &mut [i16]) {
    let mut tmp = [0i32; 16];
    for i in 0..4 {
        let a0 = i32::from(in_[i]) + i32::from(in_[12 + i]);
        let a1 = i32::from(in_[4 + i]) + i32::from(in_[8 + i]);
        let a2 = i32::from(in_[4 + i]) - i32::from(in_[8 + i]);
        let a3 = i32::from(in_[i]) - i32::from(in_[12 + i]);
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
        let base = 64 * i;
        out[base] = ((a0 + a1) >> 3) as i16;
        out[base + 16] = ((a3 + a2) >> 3) as i16;
        out[base + 32] = ((a0 - a1) >> 3) as i16;
        out[base + 48] = ((a3 - a2) >> 3) as i16;
    }
}

/// Port of `QuantizeSingle`: quantizes one DC value with error diffusion, and returns the
/// descaled error.
fn quantize_single(v: &mut i16, mtx: &VP8Matrix) -> i32 {
    let mut vv = i32::from(*v);
    let sign = vv < 0;
    if sign {
        vv = -vv;
    }
    if vv > mtx.zthresh[0] as i32 {
        let q_v = (((vv as u32).wrapping_mul(u32::from(mtx.iq[0])).wrapping_add(mtx.bias[0])) >> 17)
            as i32
            * i32::from(mtx.q[0]);
        let err = vv - q_v;
        *v = (if sign { -q_v } else { q_v }) as i16;
        return (if sign { -err } else { err }) >> DSCALE;
    }
    *v = 0;
    (if sign { -vv } else { vv }) >> DSCALE
}

/// Port of `CorrectDCValues`: adds the diffused DC errors of the neighbours to the U (`ch = 0`)
/// and V (`ch = 1`) DC values, quantizes them, and records the new errors in `rd.derr`.
fn correct_dc_values(
    enc: &VP8Encoder,
    x: usize,
    left_derr: [[i8; 2]; 2],
    mtx: &VP8Matrix,
    tmp: &mut [i16; 128],
    rd: &mut ModeScore,
) {
    let derr = enc.top_derr.as_ref().expect("top_derr");
    for ch in 0..2 {
        let top = derr[x][ch];
        let left = left_derr[ch];
        let base = ch * 4; // c = &tmp[ch * 4]: c[k][0] is tmp[16 * (ch * 4 + k)]
        let c = |k: usize| 16 * (base + k);
        let v = |k: usize| i32::from(tmp[c(k)]);
        tmp[c(0)] = (v(0) + ((C1 * i32::from(top[0]) + C2 * i32::from(left[0])) >> (DSHIFT - DSCALE))) as i16;
        let err0 = quantize_single(&mut tmp[c(0)], mtx);
        tmp[c(1)] = (i32::from(tmp[c(1)]) + ((C1 * i32::from(top[1]) + C2 * err0) >> (DSHIFT - DSCALE))) as i16;
        let err1 = quantize_single(&mut tmp[c(1)], mtx);
        tmp[c(2)] = (i32::from(tmp[c(2)]) + ((C1 * err0 + C2 * i32::from(left[1])) >> (DSHIFT - DSCALE))) as i16;
        let err2 = quantize_single(&mut tmp[c(2)], mtx);
        tmp[c(3)] = (i32::from(tmp[c(3)]) + ((C1 * err1 + C2 * err2) >> (DSHIFT - DSCALE))) as i16;
        let err3 = quantize_single(&mut tmp[c(3)], mtx);
        rd.derr[ch][0] = err1 as i8;
        rd.derr[ch][1] = err2 as i8;
        rd.derr[ch][2] = err3 as i8;
    }
}

/// Port of `StoreDiffusionErrors`.
fn store_diffusion_errors(enc: &mut VP8Encoder, it: &mut VP8EncIterator, rd: &ModeScore) {
    let x = it.x;
    let derr = enc.top_derr.as_mut().expect("top_derr");
    for ch in 0..2 {
        let mut left = [0i32; 2];
        left[0] = i32::from(rd.derr[ch][0]); // restore err1
        left[1] = 3 * i32::from(rd.derr[ch][2]) >> 2; //     ... 3/4th of err3
        let top0 = i32::from(rd.derr[ch][1]); //     ... err2
        let top1 = i32::from(rd.derr[ch][2]) - left[1]; //     ... 1/4th of err3.
        it.left_derr[ch] = [left[0] as i8, left[1] as i8];
        derr[x][ch] = [top0 as i8, top1 as i8];
    }
}

/// Port of `ReconstructIntra16`: the Intra16 prediction `mode` of the current macroblock, its
/// quantized levels, and its reconstruction into `yuv_out`. Returns the non-zero bits.
fn reconstruct_intra16(
    enc: &VP8Encoder,
    seg: usize,
    yuv_in: usize,
    yuv_p: usize,
    yuv: &mut [u8],
    rd: &mut ModeScore,
    yuv_out: usize,
    mode: usize,
) -> u32 {
    let dqm = &enc.dqm[seg];
    let ref_off = yuv_p + I16_MODE_OFFSETS[mode];
    let src = yuv_in + Y_OFF_ENC;
    let mut nz: u32 = 0;
    let mut tmp = [0i16; 256];
    let mut n = 0;
    while n < 16 {
        ftransform2(&yuv[src + VP8_SCAN[n]..], &yuv[ref_off + VP8_SCAN[n]..], &mut tmp[16 * n..]);
        n += 2;
    }
    let mut dc_tmp = [0i16; 16];
    ftransform_wht(&tmp[0..], &mut dc_tmp);
    nz |= u32::from(quantize_block(&mut dc_tmp, &mut rd.y_dc_levels, &dqm.y2)) << 24;
    let mut n = 0;
    while n < 16 {
        tmp[16 * n] = 0;
        tmp[16 * (n + 1)] = 0;
        let ac = rd.y_ac_levels.as_flattened_mut();
        nz |= quantize2_blocks(&mut tmp[16 * n..16 * n + 32], &mut ac[16 * n..16 * n + 32], &dqm.y1) << n;
        n += 2;
    }
    let mut out = [0i16; 256];
    transform_wht(&dc_tmp, &mut out);
    // The C code writes these DC values over tmp[0][...]: one per block, at 16 * block.
    for b in 0..16 {
        tmp[16 * b] = out[16 * b];
    }
    let mut n = 0;
    while n < 16 {
        let r = window(yuv, ref_off + VP8_SCAN[n]);
        itransform(&r, &tmp[16 * n..], &mut yuv[yuv_out + VP8_SCAN[n]..], true);
        n += 2;
    }
    nz
}

/// Port of `ReconstructIntra4`: the intra 4x4 prediction `mode` of the current block, its
/// quantized levels, and its reconstruction into `yuv_out`. Returns the non-zero flag.
fn reconstruct_intra4(
    enc: &VP8Encoder,
    seg: usize,
    yuv_p: usize,
    yuv: &mut [u8],
    levels: &mut [i16; 16],
    src: usize,
    yuv_out: usize,
    mode: usize,
) -> u32 {
    let dqm = &enc.dqm[seg];
    let ref_off = yuv_p + I4_MODE_OFFSETS[mode];
    let mut tmp = [0i16; 16];
    ftransform(&yuv[src..], &yuv[ref_off..], &mut tmp);
    let nz = u32::from(quantize_block(&mut tmp, levels, &dqm.y1));
    let r = window(yuv, ref_off);
    itransform(&r, &tmp, &mut yuv[yuv_out..], false);
    nz
}

/// Port of `ReconstructUV`: the chroma prediction `mode`, its quantized levels, and its
/// reconstruction into `yuv_out`. Returns the non-zero bits shifted by 16.
fn reconstruct_uv(
    enc: &VP8Encoder,
    x: usize,
    left_derr: [[i8; 2]; 2],
    seg: usize,
    yuv_in: usize,
    yuv_p: usize,
    yuv: &mut [u8],
    rd: &mut ModeScore,
    yuv_out: usize,
    mode: usize,
) -> u32 {
    let dqm = &enc.dqm[seg];
    let ref_off = yuv_p + UV_MODE_OFFSETS[mode];
    let src = yuv_in + U_OFF_ENC;
    let mut nz: u32 = 0;
    let mut tmp = [0i16; 128];
    let mut n = 0;
    while n < 8 {
        ftransform2(&yuv[src + VP8_SCAN_UV[n]..], &yuv[ref_off + VP8_SCAN_UV[n]..], &mut tmp[16 * n..]);
        n += 2;
    }
    if enc.top_derr.is_some() {
        correct_dc_values(enc, x, left_derr, &dqm.uv, &mut tmp, rd);
    }
    let mut n = 0;
    while n < 8 {
        let uv = rd.uv_levels.as_flattened_mut();
        nz |= quantize2_blocks(&mut tmp[16 * n..16 * n + 32], &mut uv[16 * n..16 * n + 32], &dqm.uv) << n;
        n += 2;
    }
    let mut n = 0;
    while n < 8 {
        let r = window(yuv, ref_off + VP8_SCAN_UV[n]);
        itransform(&r, &tmp[16 * n..], &mut yuv[yuv_out + VP8_SCAN_UV[n]..], true);
        n += 2;
    }
    nz << 16
}

/// Port of `StoreMaxDelta`.
fn store_max_delta(dqm: &mut super::vp8_encoder::SegmentInfo, dcs: &[i16; 16]) {
    let v0 = i32::from(dcs[1]).abs();
    let v1 = i32::from(dcs[2]).abs();
    let v2 = i32::from(dcs[4]).abs();
    let mut max_v = if v1 > v0 { v1 } else { v0 };
    max_v = if v2 > max_v { v2 } else { max_v };
    if max_v > dqm.max_edge {
        dqm.max_edge = max_v;
    }
}

/// Port of `SwapOut`: the reconstruction of the best mode is in `yuv_out`.
fn swap_out(it: &mut VP8EncIterator) {
    std::mem::swap(&mut it.yuv_out, &mut it.yuv_out2);
}

/// Port of `PickBestIntra16`.
fn pick_best_intra16(enc: &mut VP8Encoder, it: &mut VP8EncIterator, rd: &mut ModeScore) {
    const K_NUM_BLOCKS: usize = 16;
    let seg = enc.mb_info[it.mb_index(enc)].segment as usize;
    let lambda = enc.dqm[seg].lambda_i16;
    let tlambda = enc.dqm[seg].tlambda;
    let src = it.yuv_in + Y_OFF_ENC;
    let mut is_flat_src = is_flat_source16(&it.yuv, src);
    let mut rd_cur = ModeScore::default();
    let mut rd_best = ModeScore::default();
    rd.mode_i16 = -1;
    for mode in 0..4 {
        let tmp_dst = it.yuv_out2 + Y_OFF_ENC; // scratch buffer
        rd_cur.mode_i16 = mode as i32;
        rd_cur.nz = reconstruct_intra16(enc, seg, it.yuv_in, it.yuv_p, &mut it.yuv, &mut rd_cur, tmp_dst, mode);
        let tmp_dst = it.yuv_out2 + Y_OFF_ENC;
        rd_cur.d = i64::from(sse16x16(&it.yuv[src..], &it.yuv[tmp_dst..]));
        rd_cur.sd = if tlambda != 0 {
            i64::from(mult_8b(tlambda, disto16x16(&it.yuv[src..], &it.yuv[tmp_dst..], &WEIGHT_Y)))
        } else {
            0
        };
        rd_cur.h = i64::from(fixed_costs_i16(mode));
        rd_cur.r = i64::from(get_cost_luma16(enc, it, &rd_cur));
        if is_flat_src {
            is_flat_src = is_flat(rd_cur.y_ac_levels.as_flattened(), K_NUM_BLOCKS, FLATNESS_LIMIT_I16);
            if is_flat_src {
                rd_cur.d *= 2;
                rd_cur.sd *= 2;
            }
        }
        set_rd_score(lambda, &mut rd_cur);
        if mode == 0 || rd_cur.score < rd_best.score {
            std::mem::swap(&mut rd_cur, &mut rd_best);
            swap_out(it);
        }
    }
    *rd = rd_best;
    let dqm_mode_lambda = enc.dqm[seg].lambda_mode;
    set_rd_score(dqm_mode_lambda, rd); // finalize score for mode decision.
    it.set_intra16_mode(enc, rd.mode_i16 as u8);
    let min_disto = i64::from(enc.dqm[seg].min_disto);
    if (rd.nz & 0x100_ffff) == 0x100_0000 && rd.d > min_disto {
        let dcs = rd.y_dc_levels;
        store_max_delta(&mut enc.dqm[seg], &dcs);
    }
}

/// Port of `GetCostModeI4`: the mode costs of the current block given its top and left modes.
fn mode_costs_i4(enc: &VP8Encoder, it: &VP8EncIterator, modes: &[u8; 16]) -> [i32; 10] {
    let preds_w = enc.preds_w as isize;
    let base = it.preds_index(enc) as isize;
    let x = it.i4 & 3;
    let y = it.i4 >> 2;
    let left = if x == 0 {
        i32::from(enc.preds[(base + y as isize * preds_w - 1) as usize])
    } else {
        i32::from(modes[it.i4 - 1])
    };
    let top = if y == 0 {
        i32::from(enc.preds[(base - preds_w + x as isize) as usize])
    } else {
        i32::from(modes[it.i4 - 4])
    };
    let mut costs = [0i32; 10];
    for (mode, c) in costs.iter_mut().enumerate() {
        *c = fixed_costs_i4(top as usize, left as usize, mode);
    }
    costs
}

/// Port of `VP8GetCostLuma4`: the rate of the levels of the current 4x4 block.
fn get_cost_luma4(enc: &VP8Encoder, it: &VP8EncIterator, levels: &[i16; 16]) -> i32 {
    let x = it.i4 & 3;
    let y = it.i4 >> 2;
    let mut res = init_residual(0, 3);
    let ctx = (it.top_nz[x] + it.left_nz[y]) as usize;
    set_residual_coeffs(levels, &mut res);
    get_residual_cost(&enc.proba, ctx, &res)
}

/// Port of `VP8GetCostLuma16`: the rate of the Intra16 levels of `rd`, updating the non-zero
/// context of the macroblock.
fn get_cost_luma16(enc: &VP8Encoder, it: &mut VP8EncIterator, rd: &ModeScore) -> i32 {
    let mut r = 0;
    it.nz_to_bytes(enc); // re-import the non-zero context
    let mut res: VP8Residual<'_> = init_residual(0, 1);
    set_residual_coeffs(&rd.y_dc_levels, &mut res);
    r += get_residual_cost(&enc.proba, (it.top_nz[8] + it.left_nz[8]) as usize, &res);
    let mut res: VP8Residual<'_> = init_residual(1, 0);
    for y in 0..4 {
        for x in 0..4 {
            let ctx = (it.top_nz[x] + it.left_nz[y]) as usize;
            set_residual_coeffs(&rd.y_ac_levels[x + y * 4], &mut res);
            r += get_residual_cost(&enc.proba, ctx, &res);
            let nzv = i32::from(res.last >= 0);
            it.top_nz[x] = nzv;
            it.left_nz[y] = nzv;
        }
    }
    r
}

/// Port of `VP8GetCostUV`: the rate of the chroma levels of `rd`, updating the non-zero context.
fn get_cost_uv(enc: &VP8Encoder, it: &mut VP8EncIterator, rd: &ModeScore) -> i32 {
    let mut r = 0;
    it.nz_to_bytes(enc); // re-import the non-zero context
    let mut res: VP8Residual<'_> = init_residual(0, 2);
    for ch in [0usize, 2] {
        for y in 0..2 {
            for x in 0..2 {
                let ctx = (it.top_nz[4 + ch + x] + it.left_nz[4 + ch + y]) as usize;
                set_residual_coeffs(&rd.uv_levels[ch * 2 + x + y * 2], &mut res);
                r += get_residual_cost(&enc.proba, ctx, &res);
                let nzv = i32::from(res.last >= 0);
                it.top_nz[4 + ch + x] = nzv;
                it.left_nz[4 + ch + y] = nzv;
            }
        }
    }
    r
}

/// Port of `PickBestIntra4`: the best of the ten intra 4x4 modes for each block of the current
/// macroblock. Returns 0 when Intra16 is kept, 1 when Intra4 is selected.
fn pick_best_intra4(enc: &mut VP8Encoder, it: &mut VP8EncIterator, rd: &mut ModeScore) -> i32 {
    let seg = enc.mb_info[it.mb_index(enc)].segment as usize;
    let lambda = enc.dqm[seg].lambda_i4;
    let tlambda = enc.dqm[seg].tlambda;
    let lambda_mode = enc.dqm[seg].lambda_mode;
    let src0 = it.yuv_in + Y_OFF_ENC;
    let best_blocks = it.yuv_out2 + Y_OFF_ENC;
    let mut total_header_bits: i32 = 0;
    if enc.max_i4_header_bits == 0 {
        return 0;
    }
    let mut rd_best = ModeScore::default();
    init_score(&mut rd_best);
    rd_best.h = 211; // '211' is the value of VP8BitCost(0, 145)
    set_rd_score(lambda_mode, &mut rd_best);
    it.start_i4(enc);
    loop {
        let k_num_blocks = 1;
        let mut rd_i4 = ModeScore::default();
        init_score(&mut rd_i4);
        let mut best_mode: i32 = -1;
        let src = src0 + VP8_SCAN[it.i4];
        let mode_costs = mode_costs_i4(enc, it, &rd.modes_i4);
        let mut best_block = best_blocks + VP8_SCAN[it.i4];
        let mut tmp_dst = it.yuv_p + I4TMP; // scratch buffer.
        make_intra4_preds(it);
        for mode in 0..10 {
            let mut rd_tmp = ModeScore::default();
            let mut tmp_levels = [0i16; 16];
            rd_tmp.nz = reconstruct_intra4(enc, seg, it.yuv_p, &mut it.yuv, &mut tmp_levels, src, tmp_dst, mode)
                << it.i4;
            rd_tmp.d = i64::from(sse4x4(&it.yuv[src..], &it.yuv[tmp_dst..]));
            rd_tmp.sd = if tlambda != 0 {
                i64::from(mult_8b(tlambda, disto4x4(&it.yuv[src..], &it.yuv[tmp_dst..], &WEIGHT_Y)))
            } else {
                0
            };
            rd_tmp.h = i64::from(mode_costs[mode]);
            if mode > 0 && is_flat(&tmp_levels, k_num_blocks, FLATNESS_LIMIT_I4) {
                rd_tmp.r = i64::from(FLATNESS_PENALTY * k_num_blocks as i32);
            } else {
                rd_tmp.r = 0;
            }
            set_rd_score(lambda, &mut rd_tmp);
            if best_mode >= 0 && rd_tmp.score >= rd_i4.score {
                continue;
            }
            rd_tmp.r += i64::from(get_cost_luma4(enc, it, &tmp_levels));
            set_rd_score(lambda, &mut rd_tmp);
            if best_mode < 0 || rd_tmp.score < rd_i4.score {
                copy_score(&mut rd_i4, &rd_tmp);
                best_mode = mode as i32;
                std::mem::swap(&mut tmp_dst, &mut best_block);
                rd_best.y_ac_levels[it.i4] = tmp_levels;
            }
        }
        set_rd_score(lambda_mode, &mut rd_i4);
        add_score(&mut rd_best, &rd_i4);
        if rd_best.score >= rd.score {
            return 0;
        }
        total_header_bits += rd_i4.h as i32; // <- equal to mode_costs[best_mode];
        if total_header_bits > enc.max_i4_header_bits {
            return 0;
        }
        if best_block != best_blocks + VP8_SCAN[it.i4] {
            let (src_off, dst_off) = (best_block, best_blocks + VP8_SCAN[it.i4]);
            let block = window(&it.yuv, src_off);
            copy4x4(&block, &mut it.yuv[dst_off..]);
        }
        rd.modes_i4[it.i4] = best_mode as u8;
        let (x, y) = (it.i4 & 3, it.i4 >> 2);
        it.top_nz[x] = i32::from(rd_i4.nz != 0);
        it.left_nz[y] = i32::from(rd_i4.nz != 0);
        if !it.rotate_i4() {
            break;
        }
    }
    copy_score(rd, &rd_best);
    it.set_intra4_mode(enc, &rd.modes_i4);
    swap_out(it);
    rd.y_ac_levels = rd_best.y_ac_levels;
    1 // select intra4x4 over intra16x16
}

/// Port of `VP8MakeIntra4Preds`: the intra 4x4 predictions of the current block.
fn make_intra4_preds(it: &mut VP8EncIterator) {
    let p = it.yuv_p;
    intra4_preds(
        &mut it.yuv[p..],
        Edge {
            buf: &it.i4_boundary,
            base: it.i4_top,
        },
    );
}

/// Port of `PickBestUV`: the best chroma mode.
fn pick_best_uv(enc: &mut VP8Encoder, it: &mut VP8EncIterator, rd: &mut ModeScore) {
    const K_NUM_BLOCKS: usize = 8;
    let seg = enc.mb_info[it.mb_index(enc)].segment as usize;
    let lambda = enc.dqm[seg].lambda_uv;
    let src = it.yuv_in + U_OFF_ENC;
    let mut tmp_dst = it.yuv_out2 + U_OFF_ENC; // scratch buffer
    let dst0 = it.yuv_out + U_OFF_ENC;
    let mut dst = dst0;
    let mut rd_best = ModeScore::default();
    rd.mode_uv = -1;
    init_score(&mut rd_best);
    for mode in 0..4 {
        let mut rd_uv = ModeScore::default();
        rd_uv.nz = reconstruct_uv(enc, it.x, it.left_derr, seg, it.yuv_in, it.yuv_p, &mut it.yuv, &mut rd_uv, tmp_dst, mode);
        rd_uv.d = i64::from(sse16x8(&it.yuv[src..], &it.yuv[tmp_dst..]));
        rd_uv.sd = 0; // not calling TDisto here: it tends to flatten areas.
        rd_uv.h = i64::from(fixed_costs_uv(mode));
        rd_uv.r = i64::from(get_cost_uv(enc, it, &rd_uv));
        if mode > 0 && is_flat(rd_uv.uv_levels.as_flattened(), K_NUM_BLOCKS, FLATNESS_LIMIT_UV) {
            rd_uv.r += i64::from(FLATNESS_PENALTY * K_NUM_BLOCKS as i32);
        }
        set_rd_score(lambda, &mut rd_uv);
        if mode == 0 || rd_uv.score < rd_best.score {
            copy_score(&mut rd_best, &rd_uv);
            rd.mode_uv = mode as i32;
            rd.uv_levels = rd_uv.uv_levels;
            if enc.top_derr.is_some() {
                rd.derr = rd_uv.derr;
            }
            std::mem::swap(&mut dst, &mut tmp_dst);
        }
    }
    it.set_intra_uv_mode(enc, rd.mode_uv as u8);
    add_score(rd, &rd_best);
    if dst != dst0 {
        // copy 16x8 block if needed
        let block = window(&it.yuv, dst);
        copy16x8(&block, &mut it.yuv[dst0..]);
    }
    if enc.top_derr.is_some() {
        // store diffusion errors for next block
        store_diffusion_errors(enc, it, rd);
    }
}

/// Port of `VP8Decimate` for `rd_opt = RD_OPT_BASIC` (`method >= 3`, `do_trellis_ = 0`): the
/// best mode of the macroblock, its skip flag, and the score in `rd`. Returns `true` when the
/// macroblock is skipped.
pub fn vp8_decimate(enc: &mut VP8Encoder, it: &mut VP8EncIterator, rd: &mut ModeScore) -> bool {
    let method = enc.method;
    init_score(rd);
    super::vp8_analysis::make_luma16_preds(enc, it);
    super::vp8_analysis::make_chroma8_preds(enc, it);
    it.do_trellis = false;
    pick_best_intra16(enc, it, rd);
    if method >= 2 {
        pick_best_intra4(enc, it, rd);
    }
    pick_best_uv(enc, it, rd);
    let is_skipped = rd.nz == 0;
    it.set_skip(enc, u8::from(is_skipped));
    is_skipped
}
