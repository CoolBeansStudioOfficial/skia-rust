// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the macroblock analysis of libwebp 1.4.0 (`src/enc/analysis_enc.c`): the per-
//! macroblock susceptibility (`MBAnalyze`, with the Intra16 and chroma mode choices of the
//! `method >= 2` path), the segment assignment by k-means (`AssignSegments`), and the segment
//! alphas (`SetSegmentAlphas`).
//!
//! `SkWebpEncoder` always uses `method = 3` (so the `FastMBAnalyze` path of `method <= 1` is not
//! reached), `preprocessing = 0` (no `SmoothSegmentMap`) and `segments = 4`, so `VP8EncAnalyze`
//! always takes the segment job. The single-segment `ResetAllMBInfo` path is ported.

// Clippy allows for the C arithmetic and control flow: the C code mixes int, uint32_t
// and uint8_t, spells table offsets as `0 + 0 * BPS`, nests the mode trees as `if` chains,
// and indexes by position. The port keeps those shapes so that each line can be checked
// against the C source; the casts are the width and sign conversions of the C source.
#![allow(
    clippy::identity_op,
    clippy::erasing_op,
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::too_many_arguments,
    clippy::bool_to_int_with_if,
    clippy::cast_precision_loss,
    clippy::unreadable_literal,
    clippy::if_not_else,
    clippy::manual_range_contains,
    clippy::struct_excessive_bools,
    clippy::fn_params_excessive_bools,
    clippy::needless_pass_by_value,
    clippy::items_after_statements,
    clippy::float_cmp,
    clippy::int_plus_one,
    clippy::precedence,
    clippy::unusual_byte_groupings
)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::too_many_lines
)]

use super::picture::YuvPicture;
use super::vp8_enc_dsp::{
    Edge, I16_MODE_OFFSETS, UV_MODE_OFFSETS, VP8Histogram, collect_histogram, intra_chroma_preds,
    intra16_preds,
};
use super::vp8_encoder::{NUM_MB_SEGMENTS, U_OFF_ENC, VP8EncIterator, VP8Encoder, Y_OFF_ENC};

/// Port of `MAX_ITERS_K_MEANS`.
const MAX_ITERS_K_MEANS: usize = 6;
/// Port of `MAX_ALPHA`: 8 bits of precision for susceptibilities.
const MAX_ALPHA: usize = 255;
/// Port of `ALPHA_SCALE`.
const ALPHA_SCALE: i32 = 2 * MAX_ALPHA as i32;
/// Port of `DEFAULT_ALPHA`.
const DEFAULT_ALPHA: i32 = -1;

/// Port of `clip(v, m, M)` (`analysis_enc.c`).
#[inline]
fn clip(v: i32, m: i32, big_m: i32) -> i32 {
    if v < m {
        m
    } else if v > big_m {
        big_m
    } else {
        v
    }
}

/// Port of `SetSegmentAlphas`: the alpha and beta of each segment from its center.
fn set_segment_alphas(enc: &mut VP8Encoder, centers: &[i32; NUM_MB_SEGMENTS], mid: i32) {
    let nb = enc.segment_hdr.num_segments as usize;
    let mut min = centers[0];
    let mut max = centers[0];
    if nb > 1 {
        for n in 0..nb {
            if min > centers[n] {
                min = centers[n];
            }
            if max < centers[n] {
                max = centers[n];
            }
        }
    }
    if max == min {
        max = min + 1;
    }
    for n in 0..nb {
        let alpha = 255 * (centers[n] - mid) / (max - min);
        let beta = 255 * (centers[n] - min) / (max - min);
        enc.dqm[n].alpha = clip(alpha, -127, 127);
        enc.dqm[n].beta = clip(beta, 0, 255);
    }
}

/// Port of `FinalAlphaValue`.
fn final_alpha_value(alpha: i32) -> i32 {
    let alpha = MAX_ALPHA as i32 - alpha;
    clip(alpha, 0, MAX_ALPHA as i32)
}

/// Port of `GetAlpha`.
fn get_alpha(histo: VP8Histogram) -> i32 {
    let max_value = histo.max_value;
    let last_non_zero = histo.last_non_zero;
    if max_value > 1 {
        ALPHA_SCALE * last_non_zero / max_value
    } else {
        0
    }
}

/// Port of `AssignSegments`: clusters the alphas into the segments by k-means, sets the
/// segment of each macroblock and the segment alphas.
fn assign_segments(enc: &mut VP8Encoder, alphas: &[i32; MAX_ALPHA + 1]) {
    let nb = (enc.segment_hdr.num_segments as usize).min(NUM_MB_SEGMENTS);
    let mut centers = [0i32; NUM_MB_SEGMENTS];
    let mut weighted_average: i32 = 0;
    let mut map = [0i32; MAX_ALPHA + 1];
    let mut n = 0usize;
    while n <= MAX_ALPHA && alphas[n] == 0 {
        n += 1;
    }
    let min_a = n as i32;
    let mut n = MAX_ALPHA;
    while n as i32 > min_a && alphas[n] == 0 {
        n -= 1;
    }
    let max_a = n as i32;
    let range_a = max_a - min_a;
    for k in 0..nb {
        let nn = 2 * k as i32 + 1;
        centers[k] = min_a + (nn * range_a) / (2 * nb as i32);
    }
    for _ in 0..MAX_ITERS_K_MEANS {
        // few iters are enough
        let mut accum = [0i32; NUM_MB_SEGMENTS];
        let mut dist_accum = [0i32; NUM_MB_SEGMENTS];
        let mut n = 0usize; // track the nearest center for current 'a'
        for a in min_a..=max_a {
            let au = a as usize;
            if alphas[au] != 0 {
                while n + 1 < nb && (a - centers[n + 1]).abs() < (a - centers[n]).abs() {
                    n += 1;
                }
                map[au] = n as i32;
                dist_accum[n] += a * alphas[au];
                accum[n] += alphas[au];
            }
        }
        let mut displaced = 0i32;
        weighted_average = 0;
        let mut total_weight = 0i32;
        for n in 0..nb {
            if accum[n] != 0 {
                let new_center = (dist_accum[n] + accum[n] / 2) / accum[n];
                displaced += (centers[n] - new_center).abs();
                centers[n] = new_center;
                weighted_average += new_center * accum[n];
                total_weight += accum[n];
            }
        }
        weighted_average = (weighted_average + total_weight / 2) / total_weight;
        if displaced < 5 {
            break; // no need to keep on looping...
        }
    }
    for n in 0..enc.mb_w * enc.mb_h {
        let alpha = enc.mb_info[n].alpha as usize;
        let seg = map[alpha] as usize;
        enc.mb_info[n].segment = seg as u8;
        enc.mb_info[n].alpha = centers[seg] as u8; // for the record.
    }
    // `preprocessing & 1` (SmoothSegmentMap) is 0 for SkWebpEncoder.
    set_segment_alphas(enc, &centers, weighted_average); // pick some alphas.
}

/// Port of `VP8MakeLuma16Preds`: the four Intra16 predictions of the current macroblock, from the
/// left samples (absent in the first column) and the top samples (absent in the first row).
pub fn make_luma16_preds(enc: &VP8Encoder, it: &mut VP8EncIterator) {
    let x = it.x;
    let y = it.y;
    let left = (x > 0).then_some(Edge {
        buf: &it.left,
        base: 1,
    });
    let top = if y > 0 {
        Some(if it.use_tmp_top {
            Edge {
                buf: &it.top_tmp,
                base: 0,
            }
        } else {
            Edge {
                buf: &enc.y_top,
                base: x * 16,
            }
        })
    } else {
        None
    };
    let p = it.yuv_p;
    intra16_preds(&mut it.yuv[p..], left, top);
}

/// Port of `VP8MakeChroma8Preds`: the four chroma predictions of the current macroblock.
pub fn make_chroma8_preds(enc: &VP8Encoder, it: &mut VP8EncIterator) {
    let x = it.x;
    let y = it.y;
    let left = (x > 0).then_some(Edge {
        buf: &it.left,
        base: 33,
    });
    let top = if y > 0 {
        Some(if it.use_tmp_top {
            Edge {
                buf: &it.top_tmp,
                base: 16,
            }
        } else {
            Edge {
                buf: &enc.y_top,
                base: enc.mb_w * 16 + x * 16,
            }
        })
    } else {
        None
    };
    let p = it.yuv_p;
    intra_chroma_preds(&mut it.yuv[p..], left, top);
}

/// Port of `MBAnalyzeBestIntra16Mode`: the best Intra16 mode by the histogram alpha.
fn mb_analyze_best_intra16_mode(enc: &mut VP8Encoder, it: &mut VP8EncIterator) -> i32 {
    let max_mode = 2;
    let mut best_alpha = DEFAULT_ALPHA;
    let mut best_mode = 0usize;
    make_luma16_preds(enc, it);
    for mode in 0..max_mode {
        let histo = collect_histogram(
            &it.yuv[it.yuv_in + Y_OFF_ENC..],
            &it.yuv[it.yuv_p + I16_MODE_OFFSETS[mode]..],
            0,
            16,
        );
        let alpha = get_alpha(histo);
        if alpha > best_alpha {
            best_alpha = alpha;
            best_mode = mode;
        }
    }
    it.set_intra16_mode(enc, best_mode as u8);
    best_alpha
}

/// Port of `MBAnalyzeBestUVMode`: the chroma mode with the smallest alpha; returns the largest.
fn mb_analyze_best_uv_mode(enc: &mut VP8Encoder, it: &mut VP8EncIterator) -> i32 {
    let mut best_alpha = DEFAULT_ALPHA;
    let mut smallest_alpha = 0i32;
    let mut best_mode = 0usize;
    let max_mode = 2;
    make_chroma8_preds(enc, it);
    for mode in 0..max_mode {
        let histo = collect_histogram(
            &it.yuv[it.yuv_in + U_OFF_ENC..],
            &it.yuv[it.yuv_p + UV_MODE_OFFSETS[mode]..],
            16,
            16 + 4 + 4,
        );
        let alpha = get_alpha(histo);
        if alpha > best_alpha {
            best_alpha = alpha;
        }
        if mode == 0 || alpha < smallest_alpha {
            smallest_alpha = alpha;
            best_mode = mode;
        }
    }
    it.set_intra_uv_mode(enc, best_mode as u8);
    best_alpha
}

/// Port of `MBAnalyze`: the susceptibility of the current macroblock, added to `alphas` and to
/// the running sums `alpha` and `uv_alpha`.
fn mb_analyze(
    enc: &mut VP8Encoder,
    it: &mut VP8EncIterator,
    alphas: &mut [i32; MAX_ALPHA + 1],
    alpha_sum: &mut i32,
    uv_alpha_sum: &mut i32,
) {
    it.set_intra16_mode(enc, 0); // default: Intra16, DC_PRED
    it.set_skip(enc, 0); // not skipped
    it.set_segment(enc, 0); // default segment, spec-wise.
    // method >= 2 (SkWebpEncoder uses 3): the Intra16 mode search.
    let mut best_alpha = mb_analyze_best_intra16_mode(enc, it);
    let best_uv_alpha = mb_analyze_best_uv_mode(enc, it);
    best_alpha = (3 * best_alpha + best_uv_alpha + 2) >> 2;
    best_alpha = final_alpha_value(best_alpha);
    alphas[best_alpha as usize] += 1;
    let idx = it.mb_index(enc);
    enc.mb_info[idx].alpha = best_alpha as u8; // for later remapping.
    *alpha_sum += best_alpha; // mixed susceptibility (not just luma)
    *uv_alpha_sum += best_uv_alpha;
}

/// Port of `ResetAllMBInfo`: the single default segment, for `segments == 1` pictures.
fn reset_all_mb_info(enc: &mut VP8Encoder) {
    for mb in &mut enc.mb_info {
        mb.type_ = 1; // I16x16
        mb.uv_mode = 0;
        mb.skip = 0; // not skipped
        mb.segment = 0; // default segment
        mb.alpha = 0;
    }
    enc.dqm[0].alpha = 0;
    enc.dqm[0].beta = 0;
    enc.alpha = 0;
    enc.uv_alpha = 0;
}

/// Port of `VP8EncAnalyze`: analyses every macroblock of `pic` and assigns the segments.
///
/// The segment job (`DoSegmentsJob`) runs over all rows, with the source top and left samples
/// (`VP8IteratorImport` with a scratch buffer). The threaded split is not used (`thread_level` is
/// 0 for `SkWebpEncoder`).
pub fn vp8_enc_analyze(enc: &mut VP8Encoder, pic: &YuvPicture) -> bool {
    let do_segments = enc.segment_hdr.num_segments > 1 || enc.method <= 1;
    if do_segments {
        let total_mb = enc.mb_h * enc.mb_w;
        let mut alphas = [0i32; MAX_ALPHA + 1];
        let mut alpha_sum = 0i32;
        let mut uv_alpha_sum = 0i32;
        let mut it = VP8EncIterator::new(enc);
        it.set_count_down(enc.mb_h * enc.mb_w);
        it.use_tmp_top = true;
        // DoSegmentsJob over rows 0..mb_h.
        if !it.is_done() {
            loop {
                it.import_tmp(enc, pic);
                mb_analyze(enc, &mut it, &mut alphas, &mut alpha_sum, &mut uv_alpha_sum);
                if !it.next(enc) {
                    break;
                }
            }
        }
        enc.alpha = alpha_sum / total_mb as i32;
        enc.uv_alpha = uv_alpha_sum / total_mb as i32;
        assign_segments(enc, &alphas);
    } else {
        // Use only one default segment.
        reset_all_mb_info(enc);
    }
    true
}
