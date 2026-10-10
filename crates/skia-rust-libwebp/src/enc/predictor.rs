// Copyright 2014 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the lossless transforms of libwebp `src/enc/predictor_enc.c` and
//! `src/dsp/lossless_enc.c`: the subtract-green transform, the predictor transform
//! (`VP8LResidualImage`, with the tile search `GetBestPredictorForTile`), and the prediction
//! cost model they use.
//!
//! The C code walks the image with row pointers into a scratch buffer, where `upper_row[width]`
//! holds the first pixel of the next row (the top-right neighbour of the last column). The port
//! keeps two row buffers of `width + 1` pixels and copies with the same counts as the C
//! `memcpy`s, so every read sees the same value. Near-lossless (`near_lossless < 100`) is not
//! reached by `SkWebpEncoder` and is not ported; `max_quantization` is therefore always 1.

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

use super::entropy::fast_slog2;
use crate::lossless::predict;

/// Port of `ARGB_BLACK`.
pub const ARGB_BLACK: u32 = 0xff00_0000;
/// Port of `kPredLowEffort`.
pub const K_PRED_LOW_EFFORT: u32 = 11;
/// Port of `kMaskAlpha`.
const K_MASK_ALPHA: u32 = 0xff00_0000;
/// Port of `MAX_DIFF_COST`.
const MAX_DIFF_COST: f32 = 1e30;
/// Port of `kSpatialPredictorBias`.
const K_SPATIAL_PREDICTOR_BIAS: f32 = 15.0;
/// Port of `MAX_TRANSFORM_BITS` (the cap of the tile sizes, `method < 4`).
const MAX_TRANSFORM_BITS: u32 = 6;

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

/// Port of `PredictBatch`: the residuals of `num_pixels` pixels starting at `x_start` of a row.
/// `current` and `upper` are whole rows (`upper[x]` is the pixel above `current[x]`, and
/// `upper[width]` is valid for the top-right neighbour); `out[0]` is the residual of `x_start`.
fn predict_batch(
    mode: u32,
    x_start: usize,
    y: usize,
    num_pixels: usize,
    current: &[u32],
    upper: &[u32],
    out: &mut [u32],
) {
    let mut x = x_start;
    let mut n = num_pixels;
    let mut o = 0usize;
    if x == 0 {
        // VP8LPredictorsSub[0] (black) on the first row, VP8LPredictorsSub[2] (top) below it.
        out[0] = if y == 0 {
            sub_pixels(current[0], ARGB_BLACK)
        } else {
            sub_pixels(current[0], upper[0])
        };
        x += 1;
        o += 1;
        n -= 1;
    }
    if y == 0 {
        // VP8LPredictorsSub[1] (left) for the rest of the first row.
        for i in 0..n {
            let idx = x + i;
            out[o + i] = sub_pixels(current[idx], current[idx - 1]);
        }
    } else {
        for i in 0..n {
            let idx = x + i;
            let pred = predict(
                mode,
                current[idx - 1],
                upper[idx - 1],
                upper[idx],
                upper[idx + 1],
            );
            out[o + i] = sub_pixels(current[idx], pred);
        }
    }
}

/// Port of `GetResidual`. `max_quantization` is 1 (no near-lossless), so the exact path is
/// `PredictBatch`, and the non-exact path replaces the colour of transparent pixels by their
/// prediction, writing the result back into `current` (and `upper[width]` for the first column).
fn get_residual(
    width: usize,
    upper: &mut [u32],
    current: &mut [u32],
    mode: u32,
    x_start: usize,
    x_end: usize,
    y: usize,
    exact: bool,
    out: &mut [u32],
) {
    if exact {
        predict_batch(mode, x_start, y, x_end - x_start, current, upper, out);
        return;
    }
    for x in x_start..x_end {
        let predict_value: u32 = if y == 0 {
            if x == 0 { ARGB_BLACK } else { current[x - 1] }
        } else if x == 0 {
            upper[x]
        } else {
            predict(mode, current[x - 1], upper[x - 1], upper[x], upper[x + 1])
        };
        let mut residual = sub_pixels(current[x], predict_value);
        if (current[x] & K_MASK_ALPHA) == 0 {
            residual &= K_MASK_ALPHA;
            current[x] = predict_value & !K_MASK_ALPHA;
            if x == 0 && y != 0 {
                upper[width] = current[0];
            }
        }
        out[x - x_start] = residual;
    }
}

/// Port of `PredictionCostSpatial`.
#[must_use]
pub fn prediction_cost_spatial(counts: &[i32], weight_0: i32, mut exp_val: f32) -> f32 {
    let significant_symbols = 256 >> 4;
    let exp_decay_factor: f32 = 0.6;
    let mut bits: f32 = (weight_0 as f32) * counts[0] as f32;
    for i in 1..significant_symbols {
        bits += exp_val * (counts[i] + counts[256 - i]) as f32;
        exp_val *= exp_decay_factor;
    }
    (-0.1f64 * f64::from(bits)) as f32
}

/// Port of `VP8LCombinedShannonEntropy_C`: the entropy of `x + y` less the entropies of `x` and
/// `y`, as `CombinedShannonEntropy_C` evaluates it.
#[must_use]
pub fn combined_shannon_entropy(x: &[i32], y: &[i32]) -> f32 {
    let mut retval: f32 = 0.0;
    let mut sum_x: i32 = 0;
    let mut sum_xy: i32 = 0;
    for i in 0..256 {
        let xi = x[i];
        if xi != 0 {
            let xy = xi + y[i];
            sum_x += xi;
            retval -= fast_slog2(xi as u32);
            sum_xy += xy;
            retval -= fast_slog2(xy as u32);
        } else if y[i] != 0 {
            sum_xy += y[i];
            retval -= fast_slog2(y[i] as u32);
        }
    }
    retval += fast_slog2(sum_x as u32) + fast_slog2(sum_xy as u32);
    retval
}

/// Port of `PredictionCostSpatialHistogram`.
fn prediction_cost_spatial_histogram(accumulated: &[[i32; 256]; 4], tile: &[[i32; 256]; 4]) -> f32 {
    let mut retval: f32 = 0.0;
    for i in 0..4 {
        let k_exp_value: f32 = 0.94;
        retval += prediction_cost_spatial(&tile[i], 1, k_exp_value);
        retval += combined_shannon_entropy(&tile[i], &accumulated[i]);
    }
    retval
}

/// Port of `UpdateHisto`: one count per channel of `argb`.
fn update_histo(histo: &mut [[i32; 256]; 4], argb: u32) {
    histo[0][(argb >> 24) as usize] += 1;
    histo[1][((argb >> 16) & 0xff) as usize] += 1;
    histo[2][((argb >> 8) & 0xff) as usize] += 1;
    histo[3][(argb & 0xff) as usize] += 1;
}

/// Port of `GetBestPredictorForTile`: the predictor mode of one tile that minimises the
/// estimated cost, and the tile's residual histograms added into `accumulated`.
#[allow(clippy::too_many_arguments)]
fn get_best_predictor_for_tile(
    width: usize,
    height: usize,
    tile_x: usize,
    tile_y: usize,
    bits: u32,
    accumulated: &mut [[i32; 256]; 4],
    argb: &[u32],
    exact: bool,
    modes: &[u32],
) -> u32 {
    let k_num_pred_modes = 14;
    let start_x = tile_x << bits;
    let start_y = tile_y << bits;
    let tile_size = 1usize << bits;
    let max_y = tile_size.min(height - start_y);
    let max_x = tile_size.min(width - start_x);
    let have_left = usize::from(start_x > 0);
    let context_start_x = start_x - have_left;
    let tiles_per_row = sub_sample_size(width, bits);
    let left_mode = if tile_x > 0 {
        (modes[tile_y * tiles_per_row + tile_x - 1] >> 8) & 0xff
    } else {
        0xff
    };
    let above_mode = if tile_y > 0 {
        (modes[(tile_y - 1) * tiles_per_row + tile_x] >> 8) & 0xff
    } else {
        0xff
    };
    let mut upper_row = vec![0u32; width + 1];
    let mut current_row = vec![0u32; width + 1];
    let mut best_diff = MAX_DIFF_COST;
    let mut best_mode = 0u32;
    let mut best_histo = [[0i32; 256]; 4];
    let mut residuals = vec![0u32; 1 << MAX_TRANSFORM_BITS];
    for mode in 0..k_num_pred_modes {
        let mut histo_argb = [[0i32; 256]; 4];
        if start_y > 0 {
            let src = (start_y - 1) * width + context_start_x;
            let n = max_x + have_left + 1;
            current_row[context_start_x..context_start_x + n].copy_from_slice(&argb[src..src + n]);
        }
        for relative_y in 0..max_y {
            let y = start_y + relative_y;
            std::mem::swap(&mut upper_row, &mut current_row);
            let n = max_x + have_left + usize::from(y + 1 < height);
            let src = y * width + context_start_x;
            current_row[context_start_x..context_start_x + n].copy_from_slice(&argb[src..src + n]);
            get_residual(
                width,
                &mut upper_row,
                &mut current_row,
                mode,
                start_x,
                start_x + max_x,
                y,
                exact,
                &mut residuals,
            );
            for relative_x in 0..max_x {
                update_histo(&mut histo_argb, residuals[relative_x]);
            }
        }
        let mut cur_diff = prediction_cost_spatial_histogram(accumulated, &histo_argb);
        if mode == left_mode {
            cur_diff -= K_SPATIAL_PREDICTOR_BIAS;
        }
        if mode == above_mode {
            cur_diff -= K_SPATIAL_PREDICTOR_BIAS;
        }
        if cur_diff < best_diff {
            best_histo = histo_argb;
            best_diff = cur_diff;
            best_mode = mode;
        }
    }
    for i in 0..4 {
        for j in 0..256 {
            accumulated[i][j] += best_histo[i][j];
        }
    }
    best_mode
}

/// Port of `VP8LSubSampleSize`.
fn sub_sample_size(size: usize, bits: u32) -> usize {
    (size + (1usize << bits) - 1) >> bits
}

/// Port of `CopyImageWithPrediction`: replaces `argb` by its residuals, tile by tile with the
/// modes in `modes` (or with predictor 11 everywhere when `low_effort`).
fn copy_image_with_prediction(
    width: usize,
    height: usize,
    bits: u32,
    modes: &[u32],
    argb: &mut [u32],
    low_effort: bool,
    exact: bool,
) {
    let tiles_per_row = sub_sample_size(width, bits);
    let mut upper_row = vec![0u32; width + 1];
    let mut current_row = vec![0u32; width + 1];
    for y in 0..height {
        std::mem::swap(&mut upper_row, &mut current_row);
        let n = width + usize::from(y + 1 < height);
        current_row[..n].copy_from_slice(&argb[y * width..y * width + n]);
        if low_effort {
            let mut out = vec![0u32; width];
            predict_batch(
                K_PRED_LOW_EFFORT,
                0,
                y,
                width,
                &current_row,
                &upper_row,
                &mut out,
            );
            argb[y * width..(y + 1) * width].copy_from_slice(&out);
        } else {
            let mut x = 0usize;
            while x < width {
                let mode = (modes[(y >> bits) * tiles_per_row + (x >> bits)] >> 8) & 0xff;
                let mut x_end = x + (1usize << bits);
                if x_end > width {
                    x_end = width;
                }
                let mut out = vec![0u32; x_end - x];
                get_residual(
                    width,
                    &mut upper_row,
                    &mut current_row,
                    mode,
                    x,
                    x_end,
                    y,
                    exact,
                    &mut out,
                );
                argb[y * width + x..y * width + x_end].copy_from_slice(&out);
                x = x_end;
            }
        }
    }
}

/// Port of `VP8LResidualImage` with `near_lossless == 100` (no quantisation): the predictor
/// transform of `argb` (replaced by its residuals), returning the tile modes.
///
/// `low_effort` uses predictor 11 everywhere (`method 0`); otherwise each tile gets the mode of
/// `GetBestPredictorForTile`, with the residual histograms accumulated across tiles.
#[must_use]
pub fn residual_image(
    width: usize,
    height: usize,
    bits: u32,
    low_effort: bool,
    argb: &mut [u32],
    exact: bool,
) -> Vec<u32> {
    let tiles_per_row = sub_sample_size(width, bits);
    let tiles_per_col = sub_sample_size(height, bits);
    let mut image = vec![ARGB_BLACK | (K_PRED_LOW_EFFORT << 8); tiles_per_row * tiles_per_col];
    if !low_effort {
        let mut histo = [[0i32; 256]; 4];
        for tile_y in 0..tiles_per_col {
            for tile_x in 0..tiles_per_row {
                let pred = get_best_predictor_for_tile(
                    width, height, tile_x, tile_y, bits, &mut histo, argb, exact, &image,
                );
                image[tile_y * tiles_per_row + tile_x] = ARGB_BLACK | (pred << 8);
            }
        }
    }
    copy_image_with_prediction(width, height, bits, &image, argb, low_effort, exact);
    image
}

/// Port of `VP8LResidualImage` for `low_effort` (`method 0`), with `exact == 0`.
#[must_use]
pub fn residual_image_low_effort(
    width: usize,
    height: usize,
    bits: u32,
    argb: &mut [u32],
) -> Vec<u32> {
    residual_image(width, height, bits, true, argb, false)
}
