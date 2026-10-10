// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the loop filter strength selection of libwebp 1.4.0 (`src/enc/filter_enc.c`):
//! `VP8FilterStrengthFromDelta` and `VP8AdjustFilterStrength` (the path without statistics).
//!
//! `SkWebpEncoder` sets `autofilter = 0`, so `lf_stats_` is `NULL`: `VP8InitFilter` and
//! `VP8StoreFilterStats` (which build the SSIM statistics and run `DoFilter`) are no-ops and are
//! not ported, and `VP8AdjustFilterStrength` takes its `filter_strength > 0` branch.

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
    clippy::needless_range_loop,
    clippy::cast_possible_wrap,
    clippy::cast_lossless,
    clippy::cast_precision_loss,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::unreadable_literal,
    clippy::if_not_else,
    clippy::manual_range_contains,
    clippy::too_many_lines,
    clippy::struct_excessive_bools,
    clippy::fn_params_excessive_bools,
    clippy::needless_pass_by_value,
    clippy::items_after_statements,
    clippy::float_cmp,
    clippy::int_plus_one,
    clippy::precedence,
    clippy::unusual_byte_groupings
)]

#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use super::tables_quant::K_LEVELS_FROM_DELTA;
use super::vp8_encoder::{NUM_MB_SEGMENTS, VP8Encoder};

/// Port of `MAX_DELTA_SIZE`.
const MAX_DELTA_SIZE: i32 = 64;

/// Port of `VP8FilterStrengthFromDelta`.
#[must_use]
pub fn filter_strength_from_delta(sharpness: i32, delta: i32) -> i32 {
    let pos = if delta < MAX_DELTA_SIZE {
        delta
    } else {
        MAX_DELTA_SIZE - 1
    };
    i32::from(K_LEVELS_FROM_DELTA[sharpness as usize][pos as usize])
}

/// Port of `VP8AdjustFilterStrength` for `SkWebpEncoder` (no statistics): raises each
/// segment's filter strength to the one its maximum edge calls for, and sets the frame level
/// to the largest of them. Does nothing when `filter_strength == 0`.
pub fn adjust_filter_strength(enc: &mut VP8Encoder) {
    if enc.config.filter_strength > 0 {
        let mut max_level = 0;
        let sharpness = enc.filter_hdr.sharpness;
        for s in 0..NUM_MB_SEGMENTS {
            let dqm = &mut enc.dqm[s];
            let delta = (dqm.max_edge * i32::from(dqm.y2.q[1])) >> 3;
            let level = filter_strength_from_delta(sharpness, delta);
            if level > dqm.fstrength {
                dqm.fstrength = level;
            }
            if max_level < dqm.fstrength {
                max_level = dqm.fstrength;
            }
        }
        enc.filter_hdr.level = max_level;
    }
}
