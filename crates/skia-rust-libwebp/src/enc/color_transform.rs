// Copyright 2014 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the cross-colour transform of libwebp: `VP8LColorSpaceTransform`,
//! `GetBestGreenToRed`, `GetBestGreenRedToBlue` and `VP8LTransformColor` (`predictor_enc.c`
//! and `dsp/lossless_enc.c`).

// Module-level clippy allows. The C arithmetic mixes int, uint32_t, size_t and float, and the
// casts below are the width and sign conversions of the C source. The index loops keep the C
// control flow and evaluation order, so that the code can be read against the C source.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    clippy::float_cmp,
    clippy::too_many_arguments,
    clippy::manual_range_contains
)]

use super::predictor::{combined_shannon_entropy, prediction_cost_spatial};

/// Port of `VP8LMultipliers`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Multipliers {
    pub green_to_red: u8,
    pub green_to_blue: u8,
    pub red_to_blue: u8,
}

/// Port of `ColorCodeToMultipliers`.
#[must_use]
pub fn color_code_to_multipliers(color_code: u32) -> Multipliers {
    Multipliers {
        green_to_red: (color_code & 0xff) as u8,
        green_to_blue: ((color_code >> 8) & 0xff) as u8,
        red_to_blue: ((color_code >> 16) & 0xff) as u8,
    }
}

/// Port of `MultipliersToColorCode`.
#[must_use]
pub fn multipliers_to_color_code(m: &Multipliers) -> u32 {
    0xff00_0000u32
        | (u32::from(m.red_to_blue) << 16)
        | (u32::from(m.green_to_blue) << 8)
        | u32::from(m.green_to_red)
}

/// Port of `ColorTransformDelta`: `(int8 color_pred * int8 color) >> 5`.
fn color_transform_delta(color_pred: i8, color: i8) -> i32 {
    (i32::from(color_pred) * i32::from(color)) >> 5
}

/// Port of `U32ToS8`.
fn u32_to_s8(v: u32) -> i8 {
    (v & 0xff) as u8 as i8
}

/// Port of `TransformColorRed`.
fn transform_color_red(green_to_red: u8, argb: u32) -> u8 {
    let green = u32_to_s8(argb >> 8);
    let mut new_red = (argb >> 16) as i32;
    new_red -= color_transform_delta(green_to_red as i8, green);
    (new_red & 0xff) as u8
}

/// Port of `TransformColorBlue`.
fn transform_color_blue(green_to_blue: u8, red_to_blue: u8, argb: u32) -> u8 {
    let green = u32_to_s8(argb >> 8);
    let red = u32_to_s8(argb >> 16);
    let mut new_blue = (argb & 0xff) as i32;
    new_blue -= color_transform_delta(green_to_blue as i8, green);
    new_blue -= color_transform_delta(red_to_blue as i8, red);
    (new_blue & 0xff) as u8
}

/// Port of `VP8LCollectColorRedTransforms_C`: the histogram of red residuals of a tile.
pub fn collect_color_red_transforms(
    argb: &[u32],
    stride: usize,
    tile_width: usize,
    tile_height: usize,
    green_to_red: i32,
    histo: &mut [i32; 256],
) {
    for row in 0..tile_height {
        let base = row * stride;
        for x in 0..tile_width {
            histo[transform_color_red(green_to_red as u8, argb[base + x]) as usize] += 1;
        }
    }
}

/// Port of `VP8LCollectColorBlueTransforms_C`: the histogram of blue residuals of a tile.
pub fn collect_color_blue_transforms(
    argb: &[u32],
    stride: usize,
    tile_width: usize,
    tile_height: usize,
    green_to_blue: i32,
    red_to_blue: i32,
    histo: &mut [i32; 256],
) {
    for row in 0..tile_height {
        let base = row * stride;
        for x in 0..tile_width {
            histo[transform_color_blue(green_to_blue as u8, red_to_blue as u8, argb[base + x])
                as usize] += 1;
        }
    }
}

/// Port of `VP8LTransformColor_C`: applies the cross-colour transform to `data` in place.
pub fn transform_color(m: &Multipliers, data: &mut [u32]) {
    for px in data.iter_mut() {
        let argb = *px;
        let green = u32_to_s8(argb >> 8);
        let red = u32_to_s8(argb >> 16);
        let mut new_red = (red as u8) as i32;
        let mut new_blue = (argb & 0xff) as i32;
        new_red -= color_transform_delta(m.green_to_red as i8, green);
        new_red &= 0xff;
        new_blue -= color_transform_delta(m.green_to_blue as i8, green);
        new_blue -= color_transform_delta(m.red_to_blue as i8, red);
        new_blue &= 0xff;
        *px = (argb & 0xff00_ff00) | ((new_red as u32) << 16) | (new_blue as u32);
    }
}

/// Port of `GetPredictionCostCrossColorRed`.
fn get_prediction_cost_cross_color_red(
    argb: &[u32],
    stride: usize,
    tile_width: usize,
    tile_height: usize,
    prev_x: Multipliers,
    prev_y: Multipliers,
    green_to_red: i32,
    accumulated_red_histo: &[i32; 256],
) -> f32 {
    let mut histo = [0i32; 256];
    collect_color_red_transforms(
        argb,
        stride,
        tile_width,
        tile_height,
        green_to_red,
        &mut histo,
    );
    let mut cur_diff = prediction_cost_cross_color(accumulated_red_histo, &histo);
    if (green_to_red as u8) == prev_x.green_to_red {
        cur_diff -= 3.0; // favor keeping the areas locally similar
    }
    if (green_to_red as u8) == prev_y.green_to_red {
        cur_diff -= 3.0; // favor keeping the areas locally similar
    }
    if green_to_red == 0 {
        cur_diff -= 3.0;
    }
    cur_diff
}

/// Port of `PredictionCostCrossColor`.
fn prediction_cost_cross_color(accumulated: &[i32; 256], counts: &[i32; 256]) -> f32 {
    let k_exp_value: f32 = 2.4;
    combined_shannon_entropy(counts, accumulated) + prediction_cost_spatial(counts, 3, k_exp_value)
}

/// Port of `GetBestGreenToRed`: the green-to-red multiplier of a tile.
fn get_best_green_to_red(
    argb: &[u32],
    stride: usize,
    tile_width: usize,
    tile_height: usize,
    prev_x: Multipliers,
    prev_y: Multipliers,
    quality: i32,
    accumulated_red_histo: &[i32; 256],
    best_tx: &mut Multipliers,
) {
    let k_max_iters = 4 + ((7 * quality) >> 8); // in range [4..6]
    let mut green_to_red_best: i32 = 0;
    let mut best_diff = get_prediction_cost_cross_color_red(
        argb,
        stride,
        tile_width,
        tile_height,
        prev_x,
        prev_y,
        green_to_red_best,
        accumulated_red_histo,
    );
    for iter in 0..k_max_iters {
        let delta = 32 >> iter;
        let mut offset = -delta;
        while offset <= delta {
            let green_to_red_cur = offset + green_to_red_best;
            let cur_diff = get_prediction_cost_cross_color_red(
                argb,
                stride,
                tile_width,
                tile_height,
                prev_x,
                prev_y,
                green_to_red_cur,
                accumulated_red_histo,
            );
            if cur_diff < best_diff {
                best_diff = cur_diff;
                green_to_red_best = green_to_red_cur;
            }
            offset += 2 * delta;
        }
    }
    best_tx.green_to_red = (green_to_red_best & 0xff) as u8;
}

/// Port of `GetPredictionCostCrossColorBlue`.
fn get_prediction_cost_cross_color_blue(
    argb: &[u32],
    stride: usize,
    tile_width: usize,
    tile_height: usize,
    prev_x: Multipliers,
    prev_y: Multipliers,
    green_to_blue: i32,
    red_to_blue: i32,
    accumulated_blue_histo: &[i32; 256],
) -> f32 {
    let mut histo = [0i32; 256];
    collect_color_blue_transforms(
        argb,
        stride,
        tile_width,
        tile_height,
        green_to_blue,
        red_to_blue,
        &mut histo,
    );
    let mut cur_diff = prediction_cost_cross_color(accumulated_blue_histo, &histo);
    if (green_to_blue as u8) == prev_x.green_to_blue {
        cur_diff -= 3.0; // favor keeping the areas locally similar
    }
    if (green_to_blue as u8) == prev_y.green_to_blue {
        cur_diff -= 3.0; // favor keeping the areas locally similar
    }
    if (red_to_blue as u8) == prev_x.red_to_blue {
        cur_diff -= 3.0; // favor keeping the areas locally similar
    }
    if (red_to_blue as u8) == prev_y.red_to_blue {
        cur_diff -= 3.0; // favor keeping the areas locally similar
    }
    if green_to_blue == 0 {
        cur_diff -= 3.0;
    }
    if red_to_blue == 0 {
        cur_diff -= 3.0;
    }
    cur_diff
}

/// Port of `kGreenRedToBlueNumAxis`.
const K_GREEN_RED_TO_BLUE_NUM_AXIS: usize = 8;
/// Port of `kGreenRedToBlueMaxIters`.
const K_GREEN_RED_TO_BLUE_MAX_ITERS: usize = 7;

/// Port of `GetBestGreenRedToBlue`: the green-to-blue and red-to-blue multipliers of a tile.
fn get_best_green_red_to_blue(
    argb: &[u32],
    stride: usize,
    tile_width: usize,
    tile_height: usize,
    prev_x: Multipliers,
    prev_y: Multipliers,
    quality: i32,
    accumulated_blue_histo: &[i32; 256],
    best_tx: &mut Multipliers,
) {
    let offset: [[i32; 2]; K_GREEN_RED_TO_BLUE_NUM_AXIS] = [
        [0, -1],
        [0, 1],
        [-1, 0],
        [1, 0],
        [-1, -1],
        [-1, 1],
        [1, -1],
        [1, 1],
    ];
    let delta_lut: [i32; K_GREEN_RED_TO_BLUE_MAX_ITERS] = [16, 16, 8, 4, 2, 2, 2];
    let iters: usize = if quality < 25 {
        1
    } else if quality > 50 {
        K_GREEN_RED_TO_BLUE_MAX_ITERS
    } else {
        4
    };
    let mut green_to_blue_best: i32 = 0;
    let mut red_to_blue_best: i32 = 0;
    let mut best_diff = get_prediction_cost_cross_color_blue(
        argb,
        stride,
        tile_width,
        tile_height,
        prev_x,
        prev_y,
        green_to_blue_best,
        red_to_blue_best,
        accumulated_blue_histo,
    );
    for iter in 0..iters {
        let delta = delta_lut[iter];
        for axis in 0..K_GREEN_RED_TO_BLUE_NUM_AXIS {
            let green_to_blue_cur = offset[axis][0] * delta + green_to_blue_best;
            let red_to_blue_cur = offset[axis][1] * delta + red_to_blue_best;
            let cur_diff = get_prediction_cost_cross_color_blue(
                argb,
                stride,
                tile_width,
                tile_height,
                prev_x,
                prev_y,
                green_to_blue_cur,
                red_to_blue_cur,
                accumulated_blue_histo,
            );
            if cur_diff < best_diff {
                best_diff = cur_diff;
                green_to_blue_best = green_to_blue_cur;
                red_to_blue_best = red_to_blue_cur;
            }
            if quality < 25 && iter == 4 {
                break; // next iter.
            }
        }
        if delta == 2 && green_to_blue_best == 0 && red_to_blue_best == 0 {
            break; // out of iter-loop.
        }
    }
    best_tx.green_to_blue = (green_to_blue_best & 0xff) as u8;
    best_tx.red_to_blue = (red_to_blue_best & 0xff) as u8;
}

/// Port of `GetBestColorTransformForTile`.
fn get_best_color_transform_for_tile(
    tile_x: usize,
    tile_y: usize,
    bits: u32,
    prev_x: Multipliers,
    prev_y: Multipliers,
    quality: i32,
    xsize: usize,
    ysize: usize,
    accumulated_red_histo: &[i32; 256],
    accumulated_blue_histo: &[i32; 256],
    argb: &[u32],
) -> Multipliers {
    let max_tile_size = 1usize << bits;
    let tile_y_offset = tile_y * max_tile_size;
    let tile_x_offset = tile_x * max_tile_size;
    let all_x_max = (tile_x_offset + max_tile_size).min(xsize);
    let all_y_max = (tile_y_offset + max_tile_size).min(ysize);
    let tile_width = all_x_max - tile_x_offset;
    let tile_height = all_y_max - tile_y_offset;
    let tile_start = tile_y_offset * xsize + tile_x_offset;
    let tile_argb = &argb[tile_start..];
    let mut best_tx = Multipliers::default();
    get_best_green_to_red(
        tile_argb,
        xsize,
        tile_width,
        tile_height,
        prev_x,
        prev_y,
        quality,
        accumulated_red_histo,
        &mut best_tx,
    );
    get_best_green_red_to_blue(
        tile_argb,
        xsize,
        tile_width,
        tile_height,
        prev_x,
        prev_y,
        quality,
        accumulated_blue_histo,
        &mut best_tx,
    );
    best_tx
}

/// Port of `CopyTileWithColorTransform`: transforms one tile of `argb` in place.
fn copy_tile_with_color_transform(
    xsize: usize,
    ysize: usize,
    tile_x: usize,
    tile_y: usize,
    max_tile_size: usize,
    color_transform: Multipliers,
    argb: &mut [u32],
) {
    let xscan = max_tile_size.min(xsize - tile_x);
    let yscan = max_tile_size.min(ysize - tile_y);
    for row in 0..yscan {
        let start = (tile_y + row) * xsize + tile_x;
        transform_color(&color_transform, &mut argb[start..start + xscan]);
    }
}

/// Port of `VP8LColorSpaceTransform`: chooses the multipliers of every tile (written to `image`,
/// as colour codes) and applies them to `argb` in place.
pub fn color_space_transform(
    width: usize,
    height: usize,
    bits: u32,
    quality: i32,
    argb: &mut [u32],
    image: &mut [u32],
) {
    let max_tile_size = 1usize << bits;
    let tile_xsize = (width + (1usize << bits) - 1) >> bits;
    let tile_ysize = (height + (1usize << bits) - 1) >> bits;
    let mut accumulated_red_histo = [0i32; 256];
    let mut accumulated_blue_histo = [0i32; 256];
    let mut prev_y = Multipliers::default();
    let mut prev_x = Multipliers::default();
    for tile_y in 0..tile_ysize {
        for tile_x in 0..tile_xsize {
            let tile_x_offset = tile_x * max_tile_size;
            let tile_y_offset = tile_y * max_tile_size;
            let all_x_max = (tile_x_offset + max_tile_size).min(width);
            let all_y_max = (tile_y_offset + max_tile_size).min(height);
            let offset = tile_y * tile_xsize + tile_x;
            if tile_y != 0 {
                prev_y = color_code_to_multipliers(image[offset - tile_xsize]);
            }
            prev_x = get_best_color_transform_for_tile(
                tile_x,
                tile_y,
                bits,
                prev_x,
                prev_y,
                quality,
                width,
                height,
                &accumulated_red_histo,
                &accumulated_blue_histo,
                argb,
            );
            image[offset] = multipliers_to_color_code(&prev_x);
            copy_tile_with_color_transform(
                width,
                height,
                tile_x_offset,
                tile_y_offset,
                max_tile_size,
                prev_x,
                argb,
            );
            for y in tile_y_offset..all_y_max {
                let mut ix = y * width + tile_x_offset;
                let ix_end = ix + all_x_max - tile_x_offset;
                while ix < ix_end {
                    let pix = argb[ix];
                    if ix >= 2 && pix == argb[ix - 2] && pix == argb[ix - 1] {
                        ix += 1;
                        continue; // repeated pixels are handled by backward references
                    }
                    if ix >= width + 2
                        && argb[ix - 2] == argb[ix - width - 2]
                        && argb[ix - 1] == argb[ix - width - 1]
                        && pix == argb[ix - width]
                    {
                        ix += 1;
                        continue; // repeated pixels are handled by backward references
                    }
                    accumulated_red_histo[((pix >> 16) & 0xff) as usize] += 1;
                    accumulated_blue_histo[(pix & 0xff) as usize] += 1;
                    ix += 1;
                }
            }
        }
    }
}
