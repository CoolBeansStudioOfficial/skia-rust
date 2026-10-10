// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the header-side tree coding of libwebp 1.4.0 (`src/enc/tree_enc.c`): the default
//! probabilities (`VP8DefaultProbas`), the intra mode trees (`PutI4Mode`, `PutI16Mode`,
//! `PutUVMode`, `PutSegment`), and `VP8WriteProbas`.
//!
//! `VP8CodeIntraModes` walks the macroblocks and is ported with the macroblock iterator (see the
//! encoder module), since it reads the iterator's mode predictors.

use super::tree_tables::{K_BMODES_PROBA, VP8_COEFFS_PROBA0, VP8_COEFFS_UPDATE_PROBA};
use super::vp8_bit_writer::VP8BitWriter;
use super::vp8_cost::{NUM_BANDS, NUM_CTX, NUM_PROBAS, NUM_TYPES, VP8EncProba};

/// Port of `B_DC_PRED`.
pub const B_DC_PRED: i32 = 0;
/// Port of `B_TM_PRED`.
pub const B_TM_PRED: i32 = 1;
/// Port of `B_VE_PRED`.
pub const B_VE_PRED: i32 = 2;
/// Port of `B_HE_PRED`.
pub const B_HE_PRED: i32 = 3;
/// Port of `B_RD_PRED`.
pub const B_RD_PRED: i32 = 4;
/// Port of `B_VR_PRED`.
pub const B_VR_PRED: i32 = 5;
/// Port of `B_LD_PRED`.
pub const B_LD_PRED: i32 = 6;
/// Port of `B_VL_PRED`.
pub const B_VL_PRED: i32 = 7;
/// Port of `B_HD_PRED`.
pub const B_HD_PRED: i32 = 8;
/// Port of `NUM_BMODES`.
pub const NUM_BMODES: usize = 10;
/// Port of `DC_PRED` (`= B_DC_PRED`).
pub const DC_PRED: i32 = B_DC_PRED;
/// Port of `V_PRED` (`= B_VE_PRED`).
pub const V_PRED: i32 = B_VE_PRED;
/// Port of `H_PRED` (`= B_HE_PRED`).
pub const H_PRED: i32 = B_HE_PRED;
/// Port of `TM_PRED` (`= B_TM_PRED`).
pub const TM_PRED: i32 = B_TM_PRED;

/// Port of `VP8DefaultProbas`: the default coefficient probabilities, no skip probability, and
/// the segment probabilities at 255.
pub fn default_probas(probas: &mut VP8EncProba) {
    probas.use_skip_proba = false;
    probas.segments = [255; 3];
    probas.coeffs = VP8_COEFFS_PROBA0;
    probas.dirty = true;
}

/// Port of `PutI4Mode`: codes one intra 4x4 mode with the 9 probabilities `prob`. Returns `mode`.
pub fn put_i4_mode(bw: &mut VP8BitWriter, mode: i32, prob: &[u8; NUM_BMODES - 1]) -> i32 {
    if bw.put_bit(mode != B_DC_PRED, i32::from(prob[0])) {
        if bw.put_bit(mode != B_TM_PRED, i32::from(prob[1])) {
            if bw.put_bit(mode != B_VE_PRED, i32::from(prob[2])) {
                if !bw.put_bit(mode >= B_LD_PRED, i32::from(prob[3])) {
                    if bw.put_bit(mode != B_HE_PRED, i32::from(prob[4])) {
                        bw.put_bit(mode != B_RD_PRED, i32::from(prob[5]));
                    }
                } else if bw.put_bit(mode != B_LD_PRED, i32::from(prob[6])) {
                    if bw.put_bit(mode != B_VL_PRED, i32::from(prob[7])) {
                        bw.put_bit(mode != B_HD_PRED, i32::from(prob[8]));
                    }
                }
            }
        }
    }
    mode
}

/// Port of `PutI16Mode`.
pub fn put_i16_mode(bw: &mut VP8BitWriter, mode: i32) {
    if bw.put_bit(mode == TM_PRED || mode == H_PRED, 156) {
        bw.put_bit(mode == TM_PRED, 128); // TM or HE
    } else {
        bw.put_bit(mode == V_PRED, 163); // VE or DC
    }
}

/// Port of `PutUVMode`.
pub fn put_uv_mode(bw: &mut VP8BitWriter, uv_mode: i32) {
    if bw.put_bit(uv_mode != DC_PRED, 142) {
        if bw.put_bit(uv_mode != V_PRED, 114) {
            bw.put_bit(uv_mode != H_PRED, 183); // else: TM_PRED
        }
    }
}

/// Port of `PutSegment`: the two-level segment tree with probabilities `p`.
pub fn put_segment(bw: &mut VP8BitWriter, s: i32, p: &[u8; 3]) {
    // `if (VP8PutBit(bw, s >= 2, p[0])) p += 1;`, then `VP8PutBit(bw, s & 1, p[1])`.
    let offset = usize::from(bw.put_bit(s >= 2, i32::from(p[0])));
    bw.put_bit(s & 1 != 0, i32::from(p[1 + offset]));
}

/// Port of `K_BMODES_PROBA` lookup: the probabilities of mode `mode` after the top and left
/// modes.
#[must_use]
pub fn bmode_probas(top: i32, left: i32) -> &'static [u8; NUM_BMODES - 1] {
    &K_BMODES_PROBA[top as usize][left as usize]
}

/// Port of `VP8WriteProbas`: the coefficient probability updates and the skip probability.
pub fn write_probas(bw: &mut VP8BitWriter, probas: &VP8EncProba) {
    for t in 0..NUM_TYPES {
        for b in 0..NUM_BANDS {
            for c in 0..NUM_CTX {
                for p in 0..NUM_PROBAS {
                    let p0 = probas.coeffs[t][b][c][p];
                    let update = p0 != VP8_COEFFS_PROBA0[t][b][c][p];
                    if bw.put_bit(update, i32::from(VP8_COEFFS_UPDATE_PROBA[t][b][c][p])) {
                        bw.put_bits(u32::from(p0), 8);
                    }
                }
            }
        }
    }
    if bw.put_bit_uniform(probas.use_skip_proba) {
        bw.put_bits(u32::from(probas.skip_proba), 8);
    }
}
