// Copyright 2014 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the lossless transforms of libwebp `src/enc/predictor_enc.c` and
//! `src/dsp/lossless_enc.c`: the subtract-green transform, and the predictor transform as
//! libwebp applies it at `method 0` (`low_effort`).
//!
//! At `method 0` every tile uses predictor 11 (`kPredLowEffort`, "Select"), so the tile search
//! of `GetBestPredictorForTile` is not reached and is not ported. The residuals are the C
//! `PredictBatch` results, computed from the original pixels.

// Module-level clippy allows. The C arithmetic mixes int, uint32_t, size_t and float, and the
// casts below are the width and sign conversions of the C source. The index loops, `if`/`else`
// chains and exact float comparisons keep the C control flow and evaluation order, so that the
// code can be read against the C source; they are not simplified.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::unreadable_literal,
    clippy::needless_range_loop,
    clippy::float_cmp,
    clippy::manual_midpoint,
    clippy::redundant_else,
    clippy::single_match,
    clippy::items_after_statements,
    clippy::let_and_return,
    clippy::needless_for_each,
    clippy::while_let_loop,
    clippy::approx_constant,
    clippy::too_many_arguments,
    clippy::match_same_arms,
    clippy::if_not_else,
    clippy::needless_pass_by_value,
    clippy::explicit_iter_loop,
    clippy::collapsible_else_if,
    clippy::collapsible_if,
    clippy::manual_range_contains
)]

use crate::lossless::predict;

/// Port of `ARGB_BLACK`.
pub const ARGB_BLACK: u32 = 0xff00_0000;
/// Port of `kPredLowEffort`.
pub const K_PRED_LOW_EFFORT: u32 = 11;

/// Port of `VP8LSubPixels`: per-channel `a - b` modulo 256.
#[must_use]
pub fn sub_pixels(a: u32, b: u32) -> u32 {
    let alpha_and_green = 0x00ff_00ffu32
        .wrapping_add(a & 0xff00_ff00)
        .wrapping_sub(b & 0xff00_ff00);
    let red_and_blue = 0xff00_ff00u32
        .wrapping_add(a & 0x00ff_00ff)
        .wrapping_sub(b & 0x00ff_00ff);
    (alpha_and_green & 0xff00_ff00) | (red_and_blue & 0x00ff_00ff)
}

/// Port of `VP8LSubtractGreenFromBlueAndRed_C`: red and blue minus green, per pixel.
pub fn subtract_green_from_blue_and_red(argb_data: &mut [u32]) {
    for px in argb_data.iter_mut() {
        let argb = *px;
        let green = (argb >> 8) & 0xff;
        let new_r = (((argb >> 16) & 0xff).wrapping_sub(green)) & 0xff;
        let new_b = ((argb & 0xff).wrapping_sub(green)) & 0xff;
        *px = (argb & 0xff00_ff00) | (new_r << 16) | new_b;
    }
}

/// Port of `PredictBatch` for `x_start == 0`: the residuals of `current[..num_pixels]` (`num`
/// pixels starting at column 0 of row `y`), given the original row `upper` above it.
fn predict_batch_row(mode: u32, y: usize, current: &[u32], upper: &[u32], out: &mut [u32]) {
    let num_pixels = current.len();
    let mut x_start = 0usize;
    let mut out_off = 0usize;
    if y == 0 {
        // VP8LPredictorsSub[0] (black) for the first pixel of the first row.
        out[0] = sub_pixels(current[0], ARGB_BLACK);
    } else {
        // VP8LPredictorsSub[2] (top) for the first pixel.
        out[0] = sub_pixels(current[0], upper[0]);
    }
    x_start += 1;
    out_off += 1;
    if y == 0 {
        // VP8LPredictorsSub[1] (left) for the rest of the first row.
        for i in 0..num_pixels - x_start {
            let idx = x_start + i;
            out[out_off + i] = sub_pixels(current[idx], current[idx - 1]);
        }
    } else {
        for i in 0..num_pixels - x_start {
            let idx = x_start + i;
            let t1 = upper.get(idx + 1).copied().unwrap_or(0);
            let pred = predict(mode, current[idx - 1], upper[idx - 1], upper[idx], t1);
            out[out_off + i] = sub_pixels(current[idx], pred);
        }
    }
}

/// Port of `VP8LResidualImage` for `low_effort` (`method 0`), with `exact == 0` and no
/// near-lossless step (`near_lossless == 100`, as `SkWebpEncoder` sets it).
///
/// `argb` is the `width` x `height` image (stride `width`) and is replaced by its residuals.
/// Returns the transform image: one predictor mode per tile (`bits` log2 tile size).
#[must_use]
pub fn residual_image_low_effort(
    width: usize,
    height: usize,
    bits: u32,
    argb: &mut [u32],
) -> Vec<u32> {
    let tiles_per_row = (width + (1 << bits) - 1) >> bits;
    let tiles_per_col = (height + (1 << bits) - 1) >> bits;
    let image = vec![ARGB_BLACK | (K_PRED_LOW_EFFORT << 8); tiles_per_row * tiles_per_col];
    // CopyImageWithPrediction (low effort): every row is predicted with predictor 11, from the
    // original pixels of this row and of the row above it.
    let mut upper: Vec<u32> = vec![0; width];
    for y in 0..height {
        let current: Vec<u32> = argb[y * width..(y + 1) * width].to_vec();
        let y_out = &mut argb[y * width..(y + 1) * width];
        predict_batch_row(K_PRED_LOW_EFFORT, y, &current, &upper, y_out);
        upper = current;
    }
    image
}
