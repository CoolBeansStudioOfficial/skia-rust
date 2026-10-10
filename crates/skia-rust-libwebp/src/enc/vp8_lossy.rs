// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the lossy path of `WebPEncode` (libwebp 1.4.0 `src/enc/webp_enc.c`, the `!lossless`
//! branch) with `WebPConfigPreset(WEBP_PRESET_DEFAULT, quality)` and `method = 3`, as
//! `SkWebpEncoder` sets them: `InitVP8Encoder` (`MapConfigToTools`, the resets, the defaults),
//! `VP8EncAnalyze`, the token loop, and `VP8EncWrite`.
//!
//! Not ported: pictures with an alpha plane (`VP8EncStartAlpha` needs the lossless alpha coder at
//! method 3, which the VP8L port does not have); `WebPCleanupTransparentArea` is then not
//! reached, and `encode_lossy` returns `None` for them. The sharp YUV conversion, the dithering,
//! the progress hook and the statistics are not reached (see `picture.rs`).

#![allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap, clippy::cast_sign_loss)]

use super::picture::{YuvPicture, import_rgba};
use super::vp8_bit_writer::VP8BitWriter;
use super::vp8_cost::VP8EncProba;
use super::vp8_encoder::{LossyConfig, MbInfo, RdLevel, SegmentHeader, FilterHeader, SegmentInfo, VP8Encoder};
use super::vp8_frame::analyze_and_code;
use super::vp8_syntax::vp8_enc_write;
use super::vp8_token::VP8TBuffer;
use super::vp8_tree::{B_DC_PRED, default_probas};

/// Port of `WebPConfigPreset(WEBP_PRESET_DEFAULT, quality)` (`src/enc/config_enc.c`), with the
/// `method` that `SkWebpEncoder` sets (3 for lossy).
#[must_use]
pub fn preset_default(quality: f32, method: i32) -> LossyConfig {
    LossyConfig {
        quality,
        method,
        segments: 4,
        sns_strength: 50,
        filter_strength: 60,
        filter_sharpness: 0,
        filter_type: 1,
        partitions: 0,
        pass: 1,
        qmin: 0,
        qmax: 100,
        partition_limit: 0,
        preprocessing: 0,
    }
}

/// Port of `InitVP8Encoder` (the parts of it the lossy path reaches): the encoder state for a
/// `width x height` picture under `config`.
#[must_use]
pub fn init_vp8_encoder(config: &LossyConfig, width: usize, height: usize) -> VP8Encoder {
    let mb_w = (width + 15) >> 4;
    let mb_h = (height + 15) >> 4;
    let preds_w = 4 * mb_w + 1;
    let preds_h = 4 * mb_h + 1;
    let use_filter = config.filter_strength > 0;
    let profile = if use_filter {
        if config.filter_type == 1 { 0 } else { 1 }
    } else {
        2
    };
    // MapConfigToTools
    let method = config.method;
    let rd_opt_level = if method >= 6 {
        RdLevel::TrellisAll
    } else if method >= 5 {
        RdLevel::Trellis
    } else if method >= 3 {
        RdLevel::Basic
    } else {
        RdLevel::None
    };
    let limit = 100 - config.partition_limit;
    let max_i4_header_bits = 256 * 16 * 16 * (limit * limit) / (100 * 100);
    let mb_header_limit = (256i64 * 510 * 8 * 1024) / (mb_w * mb_h) as i64;
    let num_parts = 1usize << config.partitions;
    let use_tokens = rd_opt_level >= RdLevel::Basic;
    let mut proba = VP8EncProba::default();
    default_probas(&mut proba);
    // ResetSegmentHeader, ResetFilterHeader
    let segment_hdr = SegmentHeader {
        num_segments: config.segments,
        update_map: i32::from(config.segments > 1),
        size: 0,
    };
    let filter_hdr = FilterHeader {
        simple: 1,
        level: 0,
        sharpness: 0,
        i4x4_lf_delta: 0,
    };
    // InitTop: top_derr is allocated when the quality or the pass asks for error diffusion.
    let top_derr = (config.quality <= super::vp8_encoder::ERROR_DIFFUSION_QUALITY
        || config.pass > 1)
        .then(|| vec![[[0i8; 2]; 2]; mb_w]);
    let mut enc = VP8Encoder {
        config: config.clone(),
        filter_hdr,
        segment_hdr,
        profile,
        mb_w,
        mb_h,
        preds_w,
        num_parts,
        bw: VP8BitWriter::new(),
        parts: Vec::new(),
        tokens: VP8TBuffer::new(),
        dqm: std::array::from_fn(|_| SegmentInfo::default()),
        base_quant: 0,
        alpha: 0,
        uv_alpha: 0,
        dq_y1_dc: 0,
        dq_y2_dc: 0,
        dq_y2_ac: 0,
        dq_uv_dc: 0,
        dq_uv_ac: 0,
        proba,
        sse: [0; 4],
        sse_count: 0,
        coded_size: 0,
        residual_bytes: [[0; 4]; 3],
        block_count: [0; 3],
        method,
        rd_opt_level,
        max_i4_header_bits,
        mb_header_limit,
        use_tokens,
        mb_info: vec![MbInfo::default(); mb_w * mb_h],
        preds: vec![B_DC_PRED as u8; preds_w * preds_h + preds_w + 2],
        nz: vec![0; mb_w + 2],
        y_top: vec![0; 2 * mb_w * 16],
        top_derr,
    };
    // The C pointers: preds_ = mem + 1 + preds_w, nz_ = mem + 1 (index 1 holds nz_[0]).
    enc.preds.truncate(preds_w * preds_h + preds_w + 2);
    enc
}

/// Encodes the opaque picture `rgba` (`width x height`, 4 bytes per pixel, `RGBA` order, or
/// `RGBX` with `opaque_rgbx`) as a lossy WebP file, as `SkWebpEncoder` does with the given
/// quality. Returns `None` where the C encoder fails, or for pictures with transparency (not
/// ported; see the module comment).
#[must_use]
pub fn encode_lossy(
    rgba: &[u8],
    width: usize,
    height: usize,
    quality: f32,
    opaque_rgbx: bool,
) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || width > 16383 || height > 16383 {
        return None;
    }
    let pic: YuvPicture = import_rgba(rgba, 4 * width, width, height, false, !opaque_rgbx)?;
    if pic.a.is_some() {
        return None;
    }
    let config = preset_default(quality, 3);
    let mut enc = init_vp8_encoder(&config, width, height);
    if !analyze_and_code(&mut enc, &pic) {
        return None;
    }
    vp8_enc_write(&mut enc, width, height)
}
