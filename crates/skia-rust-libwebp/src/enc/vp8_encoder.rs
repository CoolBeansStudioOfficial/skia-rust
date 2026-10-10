// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the VP8 encoder state of libwebp 1.4.0: `VP8Encoder` and its records (`vp8i_enc.h`),
//! and the macroblock iterator (`src/enc/iterator_enc.c`, `VP8EncIterator`).
//!
//! The C iterator points into the encoder's arrays (`mb_info_`, `preds_`, `nz_`, `y_top_`,
//! `uv_top_`) and into its own work buffers. The port keeps the same arrays in [`VP8Encoder`] and
//! addresses them by index, so each iterator method takes the encoder it works on. The
//! `yuv_mem_` work area (`yuv_in_`, `yuv_out_`, `yuv_out2_`, `yuv_p_`) is one `Vec<u8>` addressed
//! by the offsets of the C pointers; the left samples are one `[u8; 64]` addressed the same way.
//!
//! Scope: `SkWebpEncoder` never sets `show_compressed`, `target_size`, `target_PSNR`,
//! `autofilter`, `thread_level` or a progress hook, so those paths (`VP8IteratorExport`'s body,
//! the progress reports, the autofilter statistics) are not ported; the iterator functions that
//! remain are the ones the encoder reaches.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::needless_range_loop
)]

use super::vp8_bit_writer::VP8BitWriter;
use super::vp8_cost::VP8EncProba;
use super::vp8_enc_dsp::{BPS, Edge, VP8Matrix};
use super::picture::YuvPicture;
use super::vp8_token::VP8TBuffer;

/// Port of `NUM_MB_SEGMENTS`.
pub const NUM_MB_SEGMENTS: usize = 4;
/// Port of `MAX_LF_LEVELS`.
pub const MAX_LF_LEVELS: usize = 64;
/// Port of `YUV_SIZE_ENC`: one 16x16 luma block plus the chroma, in `BPS` rows.
pub const YUV_SIZE_ENC: usize = BPS * 16;
/// Port of `PRED_SIZE_ENC`.
pub const PRED_SIZE_ENC: usize = 32 * BPS + 16 * BPS + 8 * BPS;
/// Port of `Y_OFF_ENC`.
pub const Y_OFF_ENC: usize = 0;
/// Port of `U_OFF_ENC`.
pub const U_OFF_ENC: usize = 16;
/// Port of `V_OFF_ENC`.
pub const V_OFF_ENC: usize = 16 + 8;
/// Port of `ERROR_DIFFUSION_QUALITY`.
pub const ERROR_DIFFUSION_QUALITY: f32 = 98.0;

/// Port of `RD_OPT_*` (`VP8RDLevel`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RdLevel {
    /// `RD_OPT_NONE`.
    None = 0,
    /// `RD_OPT_BASIC`: basic scoring (no trellis).
    Basic = 1,
    /// `RD_OPT_TRELLIS`.
    Trellis = 2,
    /// `RD_OPT_TRELLIS_ALL`.
    TrellisAll = 3,
}

/// The `WebPConfig` fields the lossy encoder reads (`SkWebpEncoder` sets them through
/// `WebPConfigPreset(WEBP_PRESET_DEFAULT, quality)` and `method = 3`).
#[derive(Debug, Clone, PartialEq)]
pub struct LossyConfig {
    /// `quality`: `0.0..=100.0`.
    pub quality: f32,
    /// `method`: 0..=6 (Skia sets 3).
    pub method: i32,
    /// `segments`: 1..=4.
    pub segments: i32,
    /// `sns_strength`.
    pub sns_strength: i32,
    /// `filter_strength`.
    pub filter_strength: i32,
    /// `filter_sharpness`.
    pub filter_sharpness: i32,
    /// `filter_type`: 0 = simple, 1 = strong.
    pub filter_type: i32,
    /// `partitions`: log2 of the token partition count.
    pub partitions: i32,
    /// `pass`.
    pub pass: i32,
    /// `qmin`.
    pub qmin: i32,
    /// `qmax`.
    pub qmax: i32,
    /// `partition_limit`.
    pub partition_limit: i32,
    /// `preprocessing`.
    pub preprocessing: i32,
}

/// Port of `VP8EncSegmentHeader`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SegmentHeader {
    /// `num_segments_`.
    pub num_segments: i32,
    /// `update_map_`.
    pub update_map: i32,
    /// `size_`: the bit cost of the segment map.
    pub size: i32,
}

/// Port of `VP8EncFilterHeader`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FilterHeader {
    /// `simple_`: 0 = complex, 1 = simple.
    pub simple: i32,
    /// `level_`: base filter level [0..63].
    pub level: i32,
    /// `sharpness_`: [0..7].
    pub sharpness: i32,
    /// `i4x4_lf_delta_`.
    pub i4x4_lf_delta: i32,
}

/// Port of `VP8MBInfo`: the per-macroblock mode information.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MbInfo {
    /// `type_`: 0 = i4x4, 1 = i16x16.
    pub type_: u8,
    /// `uv_mode_`.
    pub uv_mode: u8,
    /// `skip_`.
    pub skip: u8,
    /// `segment_`.
    pub segment: u8,
    /// `alpha_`: quantization susceptibility.
    pub alpha: u8,
}

/// Port of `VP8SegmentInfo`: the quantizers and filter parameters of one segment.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SegmentInfo {
    /// `y1_`.
    pub y1: VP8Matrix,
    /// `y2_`.
    pub y2: VP8Matrix,
    /// `uv_`.
    pub uv: VP8Matrix,
    /// `alpha_`: quant-susceptibility, range [-127,127].
    pub alpha: i32,
    /// `beta_`: filter-susceptibility, range [0,255].
    pub beta: i32,
    /// `quant_`: the final segment quantizer.
    pub quant: i32,
    /// `fstrength_`: the final in-loop filtering strength.
    pub fstrength: i32,
    /// `max_edge_`.
    pub max_edge: i32,
    /// `min_disto_`.
    pub min_disto: i32,
    /// `lambda_i16_`.
    pub lambda_i16: i32,
    /// `lambda_i4_`.
    pub lambda_i4: i32,
    /// `lambda_uv_`.
    pub lambda_uv: i32,
    /// `lambda_mode_`.
    pub lambda_mode: i32,
    /// `lambda_trellis_`.
    pub lambda_trellis: i32,
    /// `tlambda_`.
    pub tlambda: i32,
    /// `lambda_trellis_i16_`.
    pub lambda_trellis_i16: i32,
    /// `lambda_trellis_i4_`.
    pub lambda_trellis_i4: i32,
    /// `lambda_trellis_uv_`.
    pub lambda_trellis_uv: i32,
    /// `i4_penalty_`.
    pub i4_penalty: i64,
}

/// Port of `VP8Encoder` (the parts the lossy path of `SkWebpEncoder` uses).
#[derive(Debug, Clone)]
pub struct VP8Encoder {
    /// `config_`.
    pub config: LossyConfig,
    /// `filter_hdr_`.
    pub filter_hdr: FilterHeader,
    /// `segment_hdr_`.
    pub segment_hdr: SegmentHeader,
    /// `profile_`.
    pub profile: i32,
    /// `mb_w_`.
    pub mb_w: usize,
    /// `mb_h_`.
    pub mb_h: usize,
    /// `preds_w_`: the stride of `preds` (`4 * mb_w + 1`).
    pub preds_w: usize,
    /// `num_parts_`.
    pub num_parts: usize,
    /// `bw_`: partition 0.
    pub bw: VP8BitWriter,
    /// `parts_`: the token partitions.
    pub parts: Vec<VP8BitWriter>,
    /// `tokens_`.
    pub tokens: VP8TBuffer,
    /// `dqm_`: the four segments.
    pub dqm: [SegmentInfo; NUM_MB_SEGMENTS],
    /// `base_quant_`.
    pub base_quant: i32,
    /// `alpha_`.
    pub alpha: i32,
    /// `uv_alpha_`.
    pub uv_alpha: i32,
    /// `dq_y1_dc_`.
    pub dq_y1_dc: i32,
    /// `dq_y2_dc_`.
    pub dq_y2_dc: i32,
    /// `dq_y2_ac_`.
    pub dq_y2_ac: i32,
    /// `dq_uv_dc_`.
    pub dq_uv_dc: i32,
    /// `dq_uv_ac_`.
    pub dq_uv_ac: i32,
    /// `proba_`.
    pub proba: VP8EncProba,
    /// `sse_[4]`.
    pub sse: [u64; 4],
    /// `sse_count_`.
    pub sse_count: u64,
    /// `coded_size_`.
    pub coded_size: i32,
    /// `residual_bytes_[3][4]`.
    pub residual_bytes: [[i32; 4]; 3],
    /// `block_count_[3]`.
    pub block_count: [i32; 3],
    /// `method_`.
    pub method: i32,
    /// `rd_opt_level_`.
    pub rd_opt_level: RdLevel,
    /// `max_i4_header_bits_`.
    pub max_i4_header_bits: i32,
    /// `mb_header_limit_`.
    pub mb_header_limit: i64,
    /// `use_tokens_`.
    pub use_tokens: bool,
    /// `mb_info_`: `mb_w * mb_h`.
    pub mb_info: Vec<MbInfo>,
    /// `preds_`: `preds_w * (4 * mb_h + 1)`, with the C `preds_` pointer at index
    /// `1 + preds_w` (so `preds[-1]` and the row above are addressable).
    pub preds: Vec<u8>,
    /// `nz_`: `mb_w + 1` words, with the C `nz_` pointer at index 1 (so `nz_[-1]` is
    /// addressable).
    pub nz: Vec<u32>,
    /// `y_top_` and `uv_top_`: `2 * mb_w * 16` samples; the luma top row at 0 and the chroma top
    /// row at `mb_w * 16`.
    pub y_top: Vec<u8>,
    /// `top_derr_`: the error diffusion of the chroma DC, `[mb_w][u/v][top or left]`, or `None`.
    pub top_derr: Option<Vec<[[i8; 2]; 2]>>,
}

/// Port of `VP8EncIterator`: the current macroblock and its work buffers. The encoder's arrays
/// are addressed through [`VP8Encoder`], as the C back-pointer `enc_` does.
#[derive(Debug, Clone)]
pub struct VP8EncIterator {
    /// `x_`: the current macroblock column.
    pub x: usize,
    /// `y_`: the current macroblock row.
    pub y: usize,
    /// `yuv_in_`, `yuv_out_`, `yuv_out2_`, `yuv_p_`: the `YUV_SIZE_ENC`-spaced work areas.
    pub yuv: Vec<u8>,
    /// Offset of `yuv_in_` in `yuv`.
    pub yuv_in: usize,
    /// Offset of `yuv_out_` in `yuv`.
    pub yuv_out: usize,
    /// Offset of `yuv_out2_` in `yuv`.
    pub yuv_out2: usize,
    /// Offset of `yuv_p_` (the prediction scratch) in `yuv`.
    pub yuv_p: usize,
    /// The left samples: `y_left_[k]` is `left[1 + k]`, `u_left_[k]` is `left[32 + 1 + k]`,
    /// `v_left_[k]` is `left[48 + 1 + k]`. The corners `y_left_[-1]`, `u_left_[-1]`, `v_left_[-1]`
    /// are `left[0]`, `left[32]`, `left[48]`.
    pub left: [u8; 64],
    /// `left_nz_[9]`.
    pub left_nz: [i32; 9],
    /// `top_nz_[9]`.
    pub top_nz: [i32; 9],
    /// `left_derr_`.
    pub left_derr: [[i8; 2]; 2],
    /// `i4_boundary_[37]`.
    pub i4_boundary: [u8; 37],
    /// `i4_top_`: the index of the current top boundary sample in `i4_boundary`.
    pub i4_top: usize,
    /// `i4_`: the current intra 4x4 block.
    pub i4: usize,
    /// `count_down_`.
    pub count_down: i32,
    /// `count_down0_`.
    pub count_down0: i32,
    /// `bit_count_[4][3]`.
    pub bit_count: [[u64; 3]; 4],
    /// `luma_bits_`.
    pub luma_bits: u64,
    /// `uv_bits_`.
    pub uv_bits: u64,
    /// `do_trellis_`.
    pub do_trellis: bool,
    /// `tmp_32` of `VP8IteratorImport`: the top samples taken from the source picture (luma
    /// 0..16, then chroma 16..32), used while `use_tmp_top` is set (the analysis pass).
    pub top_tmp: [u8; 32],
    /// `true` when the top samples come from `top_tmp` (the `VP8IteratorImport(it, tmp)` mode)
    /// instead of `enc.y_top`.
    pub use_tmp_top: bool,
}

/// Port of `VP8Scan` (`src/enc/vp8i_enc.h`'s `extern`, defined in `quant_enc.c`): the offsets of
/// the 16 luma blocks in the `yuv_p` area.
pub const VP8_SCAN: [usize; 16] = [
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
];

/// Port of `VP8TopLeftI4` (`iterator_enc.c`).
const VP8_TOP_LEFT_I4: [usize; 16] = [17, 21, 25, 29, 13, 17, 21, 25, 9, 13, 17, 21, 5, 9, 13, 17];

impl VP8EncIterator {
    /// Port of `VP8IteratorInit` (with `VP8IteratorReset`): the iterator of an encoder, at the
    /// first macroblock, with `count_down = mb_w * mb_h`.
    #[must_use]
    pub fn new(enc: &mut VP8Encoder) -> Self {
        let mut it = Self {
            x: 0,
            y: 0,
            yuv: vec![0; 4 * YUV_SIZE_ENC + PRED_SIZE_ENC + 32],
            yuv_in: 0,
            yuv_out: YUV_SIZE_ENC,
            yuv_out2: 2 * YUV_SIZE_ENC,
            yuv_p: 3 * YUV_SIZE_ENC,
            left: [0; 64],
            left_nz: [0; 9],
            top_nz: [0; 9],
            left_derr: [[0; 2]; 2],
            i4_boundary: [0; 37],
            i4_top: 0,
            i4: 0,
            count_down: 0,
            count_down0: 0,
            bit_count: [[0; 3]; 4],
            luma_bits: 0,
            uv_bits: 0,
            do_trellis: false,
            top_tmp: [0; 32],
            use_tmp_top: false,
        };
        it.reset(enc);
        it
    }

    /// Port of `VP8IteratorReset`: row 0, the full count, the top context cleared.
    pub fn reset(&mut self, enc: &mut VP8Encoder) {
        self.set_row(enc, 0);
        self.set_count_down(enc.mb_w * enc.mb_h);
        // InitTop
        let top_size = enc.mb_w * 16;
        for v in &mut enc.y_top[..2 * top_size] {
            *v = 127;
        }
        for v in &mut enc.nz[1..1 + enc.mb_w] {
            *v = 0;
        }
        if let Some(derr) = enc.top_derr.as_mut() {
            for d in derr.iter_mut() {
                *d = [[0; 2]; 2];
            }
        }
        self.bit_count = [[0; 3]; 4];
        self.do_trellis = false;
    }

    /// Port of `VP8IteratorSetCountDown`.
    pub fn set_count_down(&mut self, count_down: usize) {
        self.count_down = count_down as i32;
        self.count_down0 = count_down as i32;
    }

    /// Port of `VP8IteratorIsDone`.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.count_down <= 0
    }

    /// Port of `InitLeft`: the left samples of the first column (129, with the corner 127 on the
    /// first row and 129 otherwise).
    fn init_left(&mut self, enc: &mut VP8Encoder) {
        let corner = if self.y > 0 { 129 } else { 127 };
        self.left[0] = corner;
        self.left[32] = corner;
        self.left[48] = corner;
        for k in 0..16 {
            self.left[1 + k] = 129;
        }
        for k in 0..8 {
            self.left[33 + k] = 129;
            self.left[49 + k] = 129;
        }
        self.left_nz[8] = 0;
        if enc.top_derr.is_some() {
            self.left_derr = [[0; 2]; 2];
        }
    }

    /// Port of `VP8IteratorSetRow`.
    pub fn set_row(&mut self, enc: &mut VP8Encoder, y: usize) {
        self.x = 0;
        self.y = y;
        self.init_left(enc);
    }

    /// Port of `VP8IteratorNext`: moves to the next macroblock (and the next row at the end of
    /// the row). Returns `true` while macroblocks remain.
    pub fn next(&mut self, enc: &mut VP8Encoder) -> bool {
        self.x += 1;
        if self.x == enc.mb_w {
            self.y += 1;
            self.set_row(enc, self.y);
        }
        self.count_down -= 1;
        0 < self.count_down
    }

    /// Index of the current macroblock in `enc.mb_info`.
    #[must_use]
    pub fn mb_index(&self, enc: &VP8Encoder) -> usize {
        self.y * enc.mb_w + self.x
    }

    /// Index of the current macroblock's first mode predictor in `enc.preds`.
    #[must_use]
    pub fn preds_index(&self, enc: &VP8Encoder) -> usize {
        // preds_ = enc->preds_ + y * 4 * preds_w, then += 4 per column.
        1 + enc.preds_w + self.y * 4 * enc.preds_w + 4 * self.x
    }

    /// Index of the current macroblock's non-zero context word in `enc.nz`.
    #[must_use]
    pub fn nz_index(&self) -> usize {
        1 + self.x
    }

    /// Port of `VP8SetIntra16Mode`.
    pub fn set_intra16_mode(&self, enc: &mut VP8Encoder, mode: u8) {
        let base = self.preds_index(enc);
        for y in 0..4 {
            for x in 0..4 {
                enc.preds[base + y * enc.preds_w + x] = mode;
            }
        }
        let idx = self.mb_index(enc);
        enc.mb_info[idx].type_ = 1;
    }

    /// Port of `VP8SetIntra4Mode`.
    pub fn set_intra4_mode(&self, enc: &mut VP8Encoder, modes: &[u8; 16]) {
        let base = self.preds_index(enc);
        for y in 0..4 {
            for x in 0..4 {
                enc.preds[base + y * enc.preds_w + x] = modes[4 * y + x];
            }
        }
        let idx = self.mb_index(enc);
        enc.mb_info[idx].type_ = 0;
    }

    /// Port of `VP8SetIntraUVMode`.
    pub fn set_intra_uv_mode(&self, enc: &mut VP8Encoder, mode: u8) {
        let idx = self.mb_index(enc);
        enc.mb_info[idx].uv_mode = mode;
    }

    /// Port of `VP8SetSkip`.
    pub fn set_skip(&self, enc: &mut VP8Encoder, skip: u8) {
        let idx = self.mb_index(enc);
        enc.mb_info[idx].skip = skip;
    }

    /// Port of `VP8SetSegment`.
    pub fn set_segment(&self, enc: &mut VP8Encoder, segment: u8) {
        let idx = self.mb_index(enc);
        enc.mb_info[idx].segment = segment;
    }

    /// Port of `VP8IteratorNzToBytes`: expands the packed non-zero bits of `nz_` and `nz_[-1]`
    /// into `top_nz` and `left_nz`.
    pub fn nz_to_bytes(&mut self, enc: &VP8Encoder) {
        let nzi = self.nz_index();
        let tnz = enc.nz[nzi];
        let lnz = enc.nz[nzi - 1];
        let bit = |nz: u32, n: u32| i32::from((nz >> n) & 1 != 0);
        self.top_nz[0] = bit(tnz, 12);
        self.top_nz[1] = bit(tnz, 13);
        self.top_nz[2] = bit(tnz, 14);
        self.top_nz[3] = bit(tnz, 15);
        self.top_nz[4] = bit(tnz, 18);
        self.top_nz[5] = bit(tnz, 19);
        self.top_nz[6] = bit(tnz, 22);
        self.top_nz[7] = bit(tnz, 23);
        self.top_nz[8] = bit(tnz, 24);
        self.left_nz[0] = bit(lnz, 3);
        self.left_nz[1] = bit(lnz, 7);
        self.left_nz[2] = bit(lnz, 11);
        self.left_nz[3] = bit(lnz, 15);
        self.left_nz[4] = bit(lnz, 17);
        self.left_nz[5] = bit(lnz, 19);
        self.left_nz[6] = bit(lnz, 21);
        self.left_nz[7] = bit(lnz, 23);
    }

    /// Port of `VP8IteratorBytesToNz`: packs `top_nz` and `left_nz` back into `*nz_`.
    pub fn bytes_to_nz(&self, enc: &mut VP8Encoder) {
        let t = &self.top_nz;
        let l = &self.left_nz;
        let mut nz: u32 = 0;
        nz |= ((t[0] << 12) | (t[1] << 13)) as u32;
        nz |= ((t[2] << 14) | (t[3] << 15)) as u32;
        nz |= ((t[4] << 18) | (t[5] << 19)) as u32;
        nz |= ((t[6] << 22) | (t[7] << 23)) as u32;
        nz |= (t[8] << 24) as u32; // we propagate the _top_ bit, esp. for intra4
        nz |= ((l[0] << 3) | (l[1] << 7)) as u32;
        nz |= (l[2] << 11) as u32;
        nz |= ((l[4] << 17) | (l[6] << 21)) as u32;
        let idx = self.nz_index();
        enc.nz[idx] = nz;
    }

    /// Port of `VP8IteratorSaveBoundary`: keeps the reconstructed right column as the next
    /// macroblock's left samples and the bottom row as the next row's top samples.
    pub fn save_boundary(&mut self, enc: &mut VP8Encoder) {
        let x = self.x;
        let y = self.y;
        let ysrc = self.yuv_out + Y_OFF_ENC;
        let uvsrc = self.yuv_out + U_OFF_ENC;
        let top_stride = enc.mb_w * 16;
        if x < enc.mb_w - 1 {
            // left
            for i in 0..16 {
                self.left[1 + i] = self.yuv[ysrc + 15 + i * BPS];
            }
            for i in 0..8 {
                self.left[33 + i] = self.yuv[uvsrc + 7 + i * BPS];
                self.left[49 + i] = self.yuv[uvsrc + 15 + i * BPS];
            }
            // y_left_[-1] = y_top_[15]; u_left_[-1] = uv_top_[7]; v_left_[-1] = uv_top_[15]
            self.left[0] = enc.y_top[x * 16 + 15];
            self.left[32] = enc.y_top[top_stride + x * 16 + 7];
            self.left[48] = enc.y_top[top_stride + x * 16 + 8 + 7];
        }
        if y < enc.mb_h - 1 {
            // top
            for i in 0..16 {
                enc.y_top[x * 16 + i] = self.yuv[ysrc + 15 * BPS + i];
            }
            for i in 0..16 {
                enc.y_top[top_stride + x * 16 + i] = self.yuv[uvsrc + 7 * BPS + i];
            }
        }
    }

    /// Port of `VP8IteratorStartI4`: loads the boundary samples of the first 4x4 block. The
    /// top-right samples of the last column replicate the last valid one.
    pub fn start_i4(&mut self, enc: &mut VP8Encoder) {
        self.i4 = 0; // first 4x4 sub-block
        self.i4_top = VP8_TOP_LEFT_I4[0];
        for i in 0..17 {
            // left: y_left_[15 - i], with y_left_[-1] the corner at i = 16.
            self.i4_boundary[i] = self.left[16 - i];
        }
        for i in 0..16 {
            // top
            self.i4_boundary[17 + i] = enc.y_top[enc_top_index(enc, self.x, i)];
        }
        if self.x < enc.mb_w - 1 {
            for i in 16..16 + 4 {
                self.i4_boundary[17 + i] = enc.y_top[enc_top_index(enc, self.x, i)];
            }
        } else {
            // else, replicate the last valid pixel four times
            for i in 16..16 + 4 {
                self.i4_boundary[17 + i] = self.i4_boundary[17 + 15];
            }
        }
        self.nz_to_bytes(enc); // import the non-zero context
    }

    /// Port of `VP8IteratorRotateI4`: stores the samples of the reconstructed 4x4 block `i4` of
    /// the `yuv_out` work area as the boundary of the next blocks. Returns `false` after the
    /// last block.
    pub fn rotate_i4(&mut self) -> bool {
        let blk = self.yuv_out + VP8_SCAN[self.i4];
        let top = self.i4_top;
        for i in 0..=3 {
            // store future top samples: top[-4 + i]
            self.i4_boundary[top - 4 + i] = self.yuv[blk + i + 3 * BPS];
        }
        if (self.i4 & 3) != 3 {
            // if not on the right sub-blocks #3, #7, #11, #15
            for i in 0..=2 {
                // store future left samples
                self.i4_boundary[top + i] = self.yuv[blk + 3 + (2 - i) * BPS];
            }
        } else {
            // else replicate top-right samples, as says the specs.
            for i in 0..=3 {
                self.i4_boundary[top + i] = self.i4_boundary[top + i + 4];
            }
        }
        self.i4 += 1;
        if self.i4 == 16 {
            // we're done
            return false;
        }
        self.i4_top = VP8_TOP_LEFT_I4[self.i4];
        true
    }
}

impl VP8EncIterator {
    /// Port of `ImportBlock`: copies a `w x h` block of `src` into the `size`-wide work block
    /// `dst` (`BPS` stride), replicating the last column and the last row to `size`.
    fn import_block(
        yuv: &mut [u8],
        dst: usize,
        src: &[u8],
        src_off: usize,
        src_stride: usize,
        w: usize,
        h: usize,
        size: usize,
    ) {
        let mut d = dst;
        let mut s = src_off;
        for _ in 0..h {
            yuv[d..d + w].copy_from_slice(&src[s..s + w]);
            if w < size {
                let v = yuv[d + w - 1];
                for k in w..size {
                    yuv[d + k] = v;
                }
            }
            d += BPS;
            s += src_stride;
        }
        for _ in h..size {
            // memcpy(dst, dst - BPS, size)
            yuv.copy_within(d - BPS..d - BPS + size, d);
            d += BPS;
        }
    }

    /// Port of `VP8IteratorImport(it, NULL)`: loads the source samples of the current
    /// macroblock into `yuv_in`. The left and top samples are kept by `save_boundary`, as the
    /// frame loops call it with `tmp_32 == NULL`.
    pub fn import(&mut self, pic: &YuvPicture) {
        let x = self.x;
        let y = self.y;
        let ys = (y * pic.y_stride() + x) * 16;
        let us = (y * pic.uv_stride() + x) * 8;
        let w = (pic.width - x * 16).min(16);
        let h = (pic.height - y * 16).min(16);
        let uv_w = (w + 1) >> 1;
        let uv_h = (h + 1) >> 1;
        let yin = self.yuv_in;
        let ystride = pic.y_stride();
        let uvstride = pic.uv_stride();
        Self::import_block(&mut self.yuv, yin + Y_OFF_ENC, &pic.y, ys, ystride, w, h, 16);
        Self::import_block(&mut self.yuv, yin + U_OFF_ENC, &pic.u, us, uvstride, uv_w, uv_h, 8);
        Self::import_block(&mut self.yuv, yin + V_OFF_ENC, &pic.v, us, uvstride, uv_w, uv_h, 8);
    }

    /// Port of `VP8IteratorImport(it, tmp_32)` with a scratch `tmp_32`: the left and top samples
    /// are taken from the source picture (the analysis pass). The top samples go to `top_tmp`,
    /// and `use_tmp_top` is set so that the predictors read them.
    pub fn import_tmp(&mut self, enc: &mut VP8Encoder, pic: &YuvPicture) {
        self.import(pic);
        let x = self.x;
        let y = self.y;
        let ystride = pic.y_stride();
        let uvstride = pic.uv_stride();
        let ys = (y * ystride + x) * 16;
        let us = (y * uvstride + x) * 8;
        let w = (pic.width - x * 16).min(16);
        let h = (pic.height - y * 16).min(16);
        let uv_w = (w + 1) >> 1;
        let uv_h = (h + 1) >> 1;
        if x == 0 {
            self.init_left(enc);
        } else {
            if y == 0 {
                self.left[0] = 127;
                self.left[32] = 127;
                self.left[48] = 127;
            } else {
                self.left[0] = pic.y[ys - 1 - ystride];
                self.left[32] = pic.u[us - 1 - uvstride];
                self.left[48] = pic.v[us - 1 - uvstride];
            }
            // ImportLine(ysrc - 1, y_stride, y_left_, h, 16) and the chroma equivalents.
            import_line(&pic.y, ys - 1, ystride, &mut self.left[1..17], h, 16);
            import_line(&pic.u, us - 1, uvstride, &mut self.left[33..41], uv_h, 8);
            import_line(&pic.v, us - 1, uvstride, &mut self.left[49..57], uv_h, 8);
        }
        self.use_tmp_top = true;
        if y == 0 {
            self.top_tmp = [127; 32];
        } else {
            import_line(&pic.y, ys - ystride, 1, &mut self.top_tmp[0..16], w, 16);
            import_line(&pic.u, us - uvstride, 1, &mut self.top_tmp[16..24], uv_w, 8);
            import_line(&pic.v, us - uvstride, 1, &mut self.top_tmp[24..32], uv_w, 8);
        }
    }

    /// The luma top samples of the current macroblock (`y_top_`): the scratch row of the
    /// analysis pass, or the encoder's row.
    #[must_use]
    pub fn y_top_edge<'a>(&'a self, enc: &'a VP8Encoder) -> Edge<'a> {
        if self.use_tmp_top {
            Edge {
                buf: &self.top_tmp,
                base: 0,
            }
        } else {
            Edge {
                buf: &enc.y_top,
                base: self.x * 16,
            }
        }
    }

    /// The chroma top samples of the current macroblock (`uv_top_`).
    #[must_use]
    pub fn uv_top_edge<'a>(&'a self, enc: &'a VP8Encoder) -> Edge<'a> {
        if self.use_tmp_top {
            Edge {
                buf: &self.top_tmp,
                base: 16,
            }
        } else {
            Edge {
                buf: &enc.y_top,
                base: enc.mb_w * 16 + self.x * 16,
            }
        }
    }

    /// The luma left samples (`y_left_`, with the corner at index 0 of the edge).
    #[must_use]
    pub fn y_left_edge(&self) -> Edge<'_> {
        Edge {
            buf: &self.left,
            base: 1,
        }
    }

    /// The chroma left samples (`u_left_`), and `v_left_` 16 samples after them.
    #[must_use]
    pub fn uv_left_edge(&self) -> Edge<'_> {
        Edge {
            buf: &self.left,
            base: 33,
        }
    }
}

/// Port of `ImportLine`: `dst[i] = src[i * stride]` for `i < len`, then `dst[i] = dst[len - 1]`
/// up to `total_len`.
fn import_line(src: &[u8], src_off: usize, src_stride: usize, dst: &mut [u8], len: usize, total_len: usize) {
    let mut s = src_off;
    for i in 0..len {
        dst[i] = src[s];
        s += src_stride;
    }
    for i in len..total_len {
        dst[i] = dst[len - 1];
    }
}

impl VP8EncIterator {
}

/// Index into `enc.y_top` of luma sample `i` (`0..20`) of macroblock column `x`: the top row of
/// the macroblock, plus the top-right samples in the next column.
#[inline]
#[must_use]
pub fn enc_top_index(_enc: &VP8Encoder, x: usize, i: usize) -> usize {
    x * 16 + i
}
