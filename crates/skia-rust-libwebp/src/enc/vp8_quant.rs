// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the quantizer setup of libwebp 1.4.0 (`src/enc/quant_enc.c`): `ExpandMatrix`,
//! `SetupMatrices`, `SetupFilterStrength`, `QualityToCompression`, `SimplifySegments` and
//! `VP8SetSegmentParams`, with the quantizer tables of that file.
//!
//! The `pow` calls (`QualityToCompression`, `VP8SetSegmentParams`) are the host libm, marked
//! `skia-rust: libm`; see `docs/design/codecs.md` §7.

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
    clippy::cast_precision_loss,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::too_many_lines
)]

use super::tables_quant::{
    K_AC_TABLE, K_AC_TABLE2, K_BIAS_MATRICES, K_DC_TABLE, K_FREQ_SHARPENING, K_WEIGHT_Y,
};
use super::vp8_enc_dsp::VP8Matrix;
use super::vp8_encoder::{NUM_MB_SEGMENTS, SegmentInfo, VP8Encoder};
use super::vp8_filter::filter_strength_from_delta;

/// Port of `QFIX` (`vp8i_enc.h`).
const QFIX: u32 = 17;
/// Port of `BIAS(b)`: `b << (QFIX - 8)`.
#[inline]
fn bias(b: i32) -> u32 {
    (b as u32) << (QFIX - 8)
}

/// Port of `SHARPEN_BITS`.
const SHARPEN_BITS: u32 = 11;
/// Port of `MID_ALPHA`.
const MID_ALPHA: i32 = 64;
/// Port of `MIN_ALPHA`.
const MIN_ALPHA: i32 = 30;
/// Port of `MAX_ALPHA` (`quant_enc.c`; not the analysis constant).
const MAX_ALPHA: i32 = 100;
/// Port of `SNS_TO_DQ`.
const SNS_TO_DQ: f64 = 0.9;
/// Port of `FSTRENGTH_CUTOFF`.
const FSTRENGTH_CUTOFF: i32 = 2;
/// Port of `MAX_DQ_UV`.
const MAX_DQ_UV: i32 = 6;
/// Port of `MIN_DQ_UV`.
const MIN_DQ_UV: i32 = -4;

/// Port of `clip(v, m, M)` (`quant_enc.c`).
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

/// Port of `ExpandMatrix`: fills the derived fields of the matrix `m` from its first two
/// steps (`q_[0]`, `q_[1]`) and the bias of `type`. Returns the mean step, rounded.
fn expand_matrix(m: &mut VP8Matrix, ty: usize) -> i32 {
    for i in 0..2 {
        let is_ac_coeff = usize::from(i > 0);
        let b = i32::from(K_BIAS_MATRICES[ty][is_ac_coeff]);
        m.iq[i] = ((1u32 << QFIX) / u32::from(m.q[i])) as u16;
        m.bias[i] = bias(b);
        m.zthresh[i] = ((1u32 << QFIX) - 1 - m.bias[i]) / u32::from(m.iq[i]);
    }
    for i in 2..16 {
        m.q[i] = m.q[1];
        m.iq[i] = m.iq[1];
        m.bias[i] = m.bias[1];
        m.zthresh[i] = m.zthresh[1];
    }
    let mut sum: i32 = 0;
    for i in 0..16 {
        if ty == 0 {
            // we only use sharpening for AC luma coeffs
            m.sharpen[i] =
                ((i32::from(K_FREQ_SHARPENING[i]) * i32::from(m.q[i])) >> SHARPEN_BITS) as u16;
        } else {
            m.sharpen[i] = 0;
        }
        sum += i32::from(m.q[i]);
    }
    (sum + 8) >> 4
}

#[inline]
fn check_lambda_value(v: &mut i32) {
    if *v < 1 {
        *v = 1;
    }
}

/// Port of `SetupMatrices`: the quantizer matrices and the lambdas of each segment.
fn setup_matrices(enc: &mut VP8Encoder) {
    let tlambda_scale = if enc.method >= 4 {
        enc.config.sns_strength
    } else {
        0
    };
    let num_segments = enc.segment_hdr.num_segments as usize;
    for i in 0..num_segments {
        let (dq_y1_dc, dq_y2_dc, dq_y2_ac, dq_uv_dc, dq_uv_ac) = (
            enc.dq_y1_dc,
            enc.dq_y2_dc,
            enc.dq_y2_ac,
            enc.dq_uv_dc,
            enc.dq_uv_ac,
        );
        let m: &mut SegmentInfo = &mut enc.dqm[i];
        let q = m.quant;
        m.y1.q[0] = u16::from(K_DC_TABLE[clip(q + dq_y1_dc, 0, 127) as usize]);
        m.y1.q[1] = K_AC_TABLE[clip(q, 0, 127) as usize];
        m.y2.q[0] = u16::from(K_DC_TABLE[clip(q + dq_y2_dc, 0, 127) as usize]) * 2;
        m.y2.q[1] = K_AC_TABLE2[clip(q + dq_y2_ac, 0, 127) as usize];
        m.uv.q[0] = u16::from(K_DC_TABLE[clip(q + dq_uv_dc, 0, 117) as usize]);
        m.uv.q[1] = K_AC_TABLE[clip(q + dq_uv_ac, 0, 127) as usize];
        let q_i4 = expand_matrix(&mut m.y1, 0);
        let q_i16 = expand_matrix(&mut m.y2, 1);
        let q_uv = expand_matrix(&mut m.uv, 2);
        m.lambda_i4 = (3 * q_i4 * q_i4) >> 7;
        m.lambda_i16 = 3 * q_i16 * q_i16;
        m.lambda_uv = (3 * q_uv * q_uv) >> 6;
        m.lambda_mode = (q_i4 * q_i4) >> 7;
        m.lambda_trellis_i4 = (7 * q_i4 * q_i4) >> 3;
        m.lambda_trellis_i16 = (q_i16 * q_i16) >> 2;
        m.lambda_trellis_uv = (q_uv * q_uv) << 1;
        m.tlambda = (tlambda_scale * q_i4) >> 5;
        check_lambda_value(&mut m.lambda_i4);
        check_lambda_value(&mut m.lambda_i16);
        check_lambda_value(&mut m.lambda_uv);
        check_lambda_value(&mut m.lambda_mode);
        check_lambda_value(&mut m.lambda_trellis_i4);
        check_lambda_value(&mut m.lambda_trellis_i16);
        check_lambda_value(&mut m.lambda_trellis_uv);
        check_lambda_value(&mut m.tlambda);
        m.min_disto = 20 * i32::from(m.y1.q[0]); // quantization-aware min disto
        m.max_edge = 0;
        m.i4_penalty = i64::from(1000 * q_i4 * q_i4);
    }
}

/// Port of `SetupFilterStrength`: the filter level of each segment, and the frame filter header.
fn setup_filter_strength(enc: &mut VP8Encoder) {
    let level0 = 5 * enc.config.filter_strength;
    for i in 0..NUM_MB_SEGMENTS {
        let sharpness = enc.filter_hdr.sharpness;
        let m = &mut enc.dqm[i];
        let qstep = K_AC_TABLE[clip(m.quant, 0, 127) as usize] as i32 >> 2;
        let base_strength = filter_strength_from_delta(sharpness, qstep);
        let f = base_strength * level0 / (256 + m.beta);
        m.fstrength = if f < FSTRENGTH_CUTOFF {
            0
        } else if f > 63 {
            63
        } else {
            f
        };
    }
    enc.filter_hdr.level = enc.dqm[0].fstrength;
    enc.filter_hdr.simple = i32::from(enc.config.filter_type == 0);
    enc.filter_hdr.sharpness = enc.config.filter_sharpness;
}

/// Port of `QualityToCompression`.
fn quality_to_compression(c: f64) -> f64 {
    let linear_c = if c < 0.75 { c * (2. / 3.) } else { 2. * c - 1. };
    // skia-rust: libm (pow)
    linear_c.powf(1. / 3.)
}

/// Port of `SegmentsAreEquivalent`.
fn segments_are_equivalent(s1: &SegmentInfo, s2: &SegmentInfo) -> bool {
    s1.quant == s2.quant && s1.fstrength == s2.fstrength
}

/// Port of `SimplifySegments`: merges the segments that have the same quantizer and filter
/// strength, and remaps the macroblocks.
fn simplify_segments(enc: &mut VP8Encoder) {
    let mut map = [0u8, 1, 2, 3];
    let num_segments = (enc.segment_hdr.num_segments as usize).min(NUM_MB_SEGMENTS);
    let mut num_final_segments = 1usize;
    for s1 in 1..num_segments {
        // find similar segments
        let mut found = false;
        let mut s2 = 0usize;
        while s2 < num_final_segments {
            if segments_are_equivalent(&enc.dqm[s1], &enc.dqm[s2]) {
                found = true;
                break;
            }
            s2 += 1;
        }
        map[s1] = s2 as u8;
        if !found {
            if num_final_segments != s1 {
                enc.dqm[num_final_segments] = enc.dqm[s1].clone();
            }
            num_final_segments += 1;
        }
    }
    if num_final_segments < num_segments {
        // Remap
        let mut i = enc.mb_w * enc.mb_h;
        while i > 0 {
            i -= 1;
            enc.mb_info[i].segment = map[enc.mb_info[i].segment as usize];
        }
        enc.segment_hdr.num_segments = num_final_segments as i32;
        for i in num_final_segments..num_segments {
            enc.dqm[i] = enc.dqm[num_final_segments - 1].clone();
        }
    }
}

/// Port of `VP8SetSegmentParams`: the quantizer of each segment from its alpha, the chroma
/// deltas, the filter strengths, and the matrices.
pub fn vp8_set_segment_params(enc: &mut VP8Encoder, quality: f32) {
    let num_segments = enc.segment_hdr.num_segments as usize;
    let sns = enc.config.sns_strength;
    let amp = SNS_TO_DQ * f64::from(sns) / 100. / 128.;
    let q = f64::from(quality) / 100.;
    let c_base = quality_to_compression(q);
    for i in 0..num_segments {
        let expn = 1. - amp * f64::from(enc.dqm[i].alpha);
        // skia-rust: libm (pow)
        let c = c_base.powf(expn);
        let qv = (127. * (1. - c)) as i32;
        enc.dqm[i].quant = clip(qv, 0, 127);
    }
    enc.base_quant = enc.dqm[0].quant;
    for i in num_segments..NUM_MB_SEGMENTS {
        enc.dqm[i].quant = enc.base_quant;
    }
    let mut dq_uv_ac =
        (enc.uv_alpha - MID_ALPHA) * (MAX_DQ_UV - MIN_DQ_UV) / (MAX_ALPHA - MIN_ALPHA);
    dq_uv_ac = dq_uv_ac * sns / 100;
    dq_uv_ac = clip(dq_uv_ac, MIN_DQ_UV, MAX_DQ_UV);
    let mut dq_uv_dc = -4 * sns / 100;
    dq_uv_dc = clip(dq_uv_dc, -15, 15); // 4bit-signed max allowed
    enc.dq_y1_dc = 0; // TODO(skal): dq-lum
    enc.dq_y2_dc = 0;
    enc.dq_y2_ac = 0;
    enc.dq_uv_dc = dq_uv_dc;
    enc.dq_uv_ac = dq_uv_ac;
    setup_filter_strength(enc); // initialize segments' filtering, eventually
    if num_segments > 1 {
        simplify_segments(enc);
    }
    setup_matrices(enc); // finalize quantization matrices
}

/// Port of `kWeightY` (`quant_enc.c`), the luma weights of the distortion metric.
pub const WEIGHT_Y: [u16; 16] = K_WEIGHT_Y;
