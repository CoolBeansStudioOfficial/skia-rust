// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the frame loop of libwebp 1.4.0 (`src/enc/frame_enc.c`) on the token path that
//! `SkWebpEncoder` takes (`use_tokens`, `rd_opt = RD_OPT_BASIC`, one pass, no size search):
//! `VP8EncTokenLoop` with `RecordTokens`, `FinalizeTokenProbas`, `FinalizeSkipProba`-free
//! probabilities, `SetSegmentProbas`, `SetLoopParams`, `PostLoopFinalize`.
//!
//! Not ported, because `SkWebpEncoder` does not reach them: the PSNR and size statistics
//! (`StoreSideInfo`, `ResetSideInfo`, `GetPSNR`, `ComputeNextQ`, `VP8EstimateTokenSize`), the
//! filter statistics (`VP8StoreFilterStats`), the show-compressed export, the progress reports,
//! and the non-token loop `VP8EncLoop` (used only when `use_tokens` is off).

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::too_many_lines,
    clippy::needless_range_loop
)]

use super::picture::YuvPicture;
use super::vp8_analysis::vp8_enc_analyze;
use super::vp8_cost::{
    NUM_BANDS, NUM_CTX, NUM_PROBAS, NUM_TYPES, VP8EncProba, bit_cost, calculate_level_costs,
    init_residual, set_residual_coeffs,
};
use super::vp8_encoder::{NUM_MB_SEGMENTS, VP8Encoder, VP8EncIterator};
use super::vp8_filter::adjust_filter_strength;
use super::vp8_mode::{ModeScore, vp8_decimate};
use super::vp8_quant::vp8_set_segment_params;
use super::vp8_token::VP8TBuffer;
use super::tree_tables::{VP8_COEFFS_PROBA0, VP8_COEFFS_UPDATE_PROBA};

/// Port of `MIN_COUNT`: the minimum number of macroblocks before the probabilities are updated.
const MIN_COUNT: usize = 96;
/// Port of `PARTITION0_SIZE_LIMIT`: `((VP8_MAX_PARTITION0_SIZE - 2048) << 11)`.
const PARTITION0_SIZE_LIMIT: u64 = ((1u64 << 19) - 2048) << 11;

/// Port of `Clamp` (float).
#[inline]
fn clamp(v: f32, min: f32, max: f32) -> f32 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

/// Port of `GetProba`.
fn get_proba(a: i32, b: i32) -> u8 {
    let total = a + b;
    if total == 0 {
        255 // that's the default probability.
    } else {
        ((255 * a + total / 2) / total) as u8 // rounded proba
    }
}

/// Port of `SetSegmentProbas`: the segment tree probabilities and the segment map header.
fn set_segment_probas(enc: &mut VP8Encoder) {
    let mut p = [0i32; NUM_MB_SEGMENTS];
    for n in 0..enc.mb_w * enc.mb_h {
        p[enc.mb_info[n].segment as usize] += 1;
    }
    if enc.segment_hdr.num_segments > 1 {
        let probas = &mut enc.proba.segments;
        probas[0] = get_proba(p[0] + p[1], p[2] + p[3]);
        probas[1] = get_proba(p[0], p[1]);
        probas[2] = get_proba(p[2], p[3]);
        enc.segment_hdr.update_map = i32::from(
            probas[0] != 255 || probas[1] != 255 || probas[2] != 255,
        );
        let pr = enc.proba.segments;
        if enc.segment_hdr.update_map == 0 {
            // ResetSegments
            for n in 0..enc.mb_w * enc.mb_h {
                enc.mb_info[n].segment = 0;
            }
        }
        let b = |bit: i32, prob: u8| bit_cost(bit, prob);
        enc.segment_hdr.size = p[0] * (b(0, pr[0]) + b(0, pr[1]))
            + p[1] * (b(0, pr[0]) + b(1, pr[1]))
            + p[2] * (b(1, pr[0]) + b(0, pr[2]))
            + p[3] * (b(1, pr[0]) + b(1, pr[2]));
    } else {
        enc.segment_hdr.update_map = 0;
        enc.segment_hdr.size = 0;
    }
}

/// Port of `SetLoopParams`: the segment quantizers and filters for quality `q`, the segment
/// probabilities, and the level costs (`ResetStats`).
fn set_loop_params(enc: &mut VP8Encoder, q: f32) {
    // Make sure the quality parameter is inside valid bounds
    let q = clamp(q, 0., 100.);
    vp8_set_segment_params(enc, q); // setup segment quantizations and filters
    set_segment_probas(enc); // compute segment probabilities
    // ResetStats
    calculate_level_costs(&mut enc.proba);
    enc.proba.nb_skip = 0;
}

/// Port of `ResetTokenStats`.
fn reset_token_stats(proba: &mut VP8EncProba) {
    proba.stats = [[[[0; NUM_PROBAS]; NUM_CTX]; NUM_BANDS]; NUM_TYPES];
}

/// Port of `CalcTokenProba`.
fn calc_token_proba(nb: i32, total: i32) -> i32 {
    if nb != 0 {
        255 - nb * 255 / total
    } else {
        255
    }
}

/// Port of `BranchCost`.
fn branch_cost(nb: i32, total: i32, proba: i32) -> i32 {
    nb * bit_cost(1, proba as u8) + (total - nb) * bit_cost(0, proba as u8)
}

/// Port of `FinalizeTokenProbas`: chooses the probabilities of the coefficients from their
/// statistics, and returns the estimated size in bits.
pub fn finalize_token_probas(proba: &mut VP8EncProba) -> i32 {
    let mut has_changed = false;
    let mut size = 0i32;
    for t in 0..NUM_TYPES {
        for b in 0..NUM_BANDS {
            for c in 0..NUM_CTX {
                for p in 0..NUM_PROBAS {
                    let stats = proba.stats[t][b][c][p];
                    let nb = (stats & 0xffff) as i32;
                    let total = ((stats >> 16) & 0xffff) as i32;
                    let update_proba = i32::from(VP8_COEFFS_UPDATE_PROBA[t][b][c][p]);
                    let old_p = i32::from(VP8_COEFFS_PROBA0[t][b][c][p]);
                    let new_p = calc_token_proba(nb, total);
                    let old_cost = branch_cost(nb, total, old_p) + bit_cost(0, update_proba as u8);
                    let new_cost =
                        branch_cost(nb, total, new_p) + bit_cost(1, update_proba as u8) + 8 * 256;
                    let use_new_p = old_cost > new_cost;
                    size += bit_cost(i32::from(use_new_p), update_proba as u8);
                    if use_new_p {
                        // only use proba that seem meaningful enough.
                        proba.coeffs[t][b][c][p] = new_p as u8;
                        has_changed |= new_p != old_p;
                        size += 8 * 256;
                    } else {
                        proba.coeffs[t][b][c][p] = old_p as u8;
                    }
                }
            }
        }
    }
    proba.dirty = has_changed;
    size
}

/// Port of `RecordTokens`: records the tokens of the current macroblock, with the non-zero
/// context of the iterator.
fn record_tokens(
    enc: &mut VP8Encoder,
    it: &mut VP8EncIterator,
    rd: &ModeScore,
    tokens: &mut VP8TBuffer,
) {
    it.nz_to_bytes(enc);
    let is_i16 = enc.mb_info[it.mb_index(enc)].type_ == 1;
    let mut res;
    if is_i16 {
        // i16x16
        let ctx = (it.top_nz[8] + it.left_nz[8]) as usize;
        res = init_residual(0, 1);
        set_residual_coeffs(&rd.y_dc_levels, &mut res);
        let v = tokens.record_coeff_tokens(&mut enc.proba, ctx, &res);
        it.top_nz[8] = v;
        it.left_nz[8] = v;
        res = init_residual(1, 0);
    } else {
        res = init_residual(0, 3);
    }
    for y in 0..4 {
        for x in 0..4 {
            let ctx = (it.top_nz[x] + it.left_nz[y]) as usize;
            set_residual_coeffs(&rd.y_ac_levels[x + y * 4], &mut res);
            let v = tokens.record_coeff_tokens(&mut enc.proba, ctx, &res);
            it.top_nz[x] = v;
            it.left_nz[y] = v;
        }
    }
    res = init_residual(0, 2);
    for ch in [0usize, 2] {
        for y in 0..2 {
            for x in 0..2 {
                let ctx = (it.top_nz[4 + ch + x] + it.left_nz[4 + ch + y]) as usize;
                set_residual_coeffs(&rd.uv_levels[ch * 2 + x + y * 2], &mut res);
                let v = tokens.record_coeff_tokens(&mut enc.proba, ctx, &res);
                it.top_nz[4 + ch + x] = v;
                it.left_nz[4 + ch + y] = v;
            }
        }
    }
    it.bytes_to_nz(enc);
}

/// Port of `VP8EncTokenLoop` (one pass, no size search): analyses the macroblocks, records their
/// tokens, and emits the tokens into partition 0 of the frame. The pictures' samples come from
/// `pic`. Returns `false` where the C code fails.
pub fn vp8_enc_token_loop(enc: &mut VP8Encoder, pic: &YuvPicture) -> bool {
    let mut max_count = (enc.mb_w * enc.mb_h) >> 3;
    let mut num_pass_left = enc.config.pass;
    // InitPassStats: the quality, clamped to [qmin, qmax].
    let qmin = enc.config.qmin as f32;
    let qmax = enc.config.qmax as f32;
    let stats_q = clamp(enc.config.quality, qmin, qmax);
    let stats_dq: f32 = 10.;
    let ok = true;
    if max_count < MIN_COUNT {
        max_count = MIN_COUNT;
    }
    let mut tokens = std::mem::take(&mut enc.tokens);
    while ok && num_pass_left > 0 {
        num_pass_left -= 1;
        let is_last_pass = stats_dq.abs() <= 0.4 || num_pass_left == 0 || enc.max_i4_header_bits == 0;
        let mut size_p0: u64 = 0;
        let mut cnt = max_count as i64;
        let mut it = VP8EncIterator::new(enc);
        set_loop_params(enc, stats_q);
        if is_last_pass {
            reset_token_stats(&mut enc.proba);
        }
        tokens.clear();
        loop {
            it.import(pic);
            cnt -= 1;
            if cnt < 0 {
                finalize_token_probas(&mut enc.proba);
                calculate_level_costs(&mut enc.proba); // refresh cost tables for rd-opt
                cnt = max_count as i64;
            }
            let mut info = ModeScore::default();
            vp8_decimate(enc, &mut it, &mut info);
            record_tokens(enc, &mut it, &info, &mut tokens);
            size_p0 += info.h as u64;
            it.save_boundary(enc);
            if !it.next(enc) {
                break;
            }
        }
        size_p0 += enc.segment_hdr.size as u64;
        if enc.max_i4_header_bits > 0 && size_p0 > PARTITION0_SIZE_LIMIT {
            num_pass_left += 1;
            enc.max_i4_header_bits >>= 1; // strengthen header bit limitation...
            continue; // ...and start over
        }
        if is_last_pass {
            break; // done
        }
    }
    finalize_token_probas(&mut enc.proba);
    let flat: Vec<u8> = enc
        .proba
        .coeffs
        .iter()
        .flatten()
        .flatten()
        .flatten()
        .copied()
        .collect();
    tokens.emit_tokens(&mut enc.parts[0], &flat);
    enc.tokens = tokens;
    // PostLoopFinalize: finish the partitions, then the filter strength.
    for p in enc.parts.iter_mut() {
        p.finish_in_place();
    }
    adjust_filter_strength(enc);
    ok
}

/// Port of `PreLoopInitialize`'s partition setup: the token partitions of the frame.
pub fn init_partitions(enc: &mut VP8Encoder) {
    enc.parts = (0..enc.num_parts).map(|_| super::vp8_bit_writer::VP8BitWriter::new()).collect();
}

/// Port of `VP8EncAnalyze`'s caller order in `WebPEncode`: analysis, then the token loop.
pub fn analyze_and_code(enc: &mut VP8Encoder, pic: &YuvPicture) -> bool {
    if !vp8_enc_analyze(enc, pic) {
        return false;
    }
    init_partitions(enc);
    vp8_enc_token_loop(enc, pic)
}
