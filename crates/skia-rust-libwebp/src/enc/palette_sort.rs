// Copyright 2014 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the palette orderings of libwebp `src/utils/palette.c`: `PaletteSortMinimizeDeltas`
//! (`kMinimizeDelta`), which is the ordering `method 3` uses for palettes. The modified Zeng
//! ordering (`kModifiedZeng`, reached only by `method 5` and `6`) is not ported.

// Module-level clippy allows, as in the neighbouring encoder modules.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    clippy::many_single_char_names
)]

use super::predictor::sub_pixels;

/// Port of `PaletteComponentDistance`.
fn palette_component_distance(v: u32) -> u32 {
    if v <= 128 { v } else { 256 - v }
}

/// Port of `PaletteColorDistance`: a value related to the entropy of the palette entry's diff
/// from `col2`, with the RGB components weighted nine times the alpha component.
fn palette_color_distance(col1: u32, col2: u32) -> u32 {
    let diff = sub_pixels(col1, col2);
    let k_more_weight_for_rgb_than_for_alpha = 9;
    let mut score = palette_component_distance(diff & 0xff);
    score += palette_component_distance((diff >> 8) & 0xff);
    score += palette_component_distance((diff >> 16) & 0xff);
    score *= k_more_weight_for_rgb_than_for_alpha;
    score += palette_component_distance((diff >> 24) & 0xff);
    score
}

/// Port of `PaletteHasNonMonotonousDeltas`: whether two consecutive deltas of `palette` have
/// opposite signs in some channel.
fn palette_has_non_monotonous_deltas(palette: &[u32]) -> bool {
    let mut predict: u32 = 0x0000_0000;
    let mut sign_found: u8 = 0x00;
    for &color in palette {
        let diff = sub_pixels(color, predict);
        let rd = ((diff >> 16) & 0xff) as u8;
        let gd = ((diff >> 8) & 0xff) as u8;
        let bd = (diff & 0xff) as u8;
        if rd != 0x00 {
            sign_found |= if rd < 0x80 { 1 } else { 2 };
        }
        if gd != 0x00 {
            sign_found |= if gd < 0x80 { 8 } else { 16 };
        }
        if bd != 0x00 {
            sign_found |= if bd < 0x80 { 64 } else { 128 };
        }
        predict = color;
    }
    let sf = u32::from(sign_found);
    (sf & (sf << 1)) != 0 // two consequent signs.
}

/// Port of `PaletteSortMinimizeDeltas`: reorders the sorted palette greedily so that consecutive
/// entries have small deltas (`PaletteSort(kMinimizeDelta)`).
#[must_use]
pub fn palette_sort_minimize_deltas(palette_sorted: &[u32]) -> Vec<u32> {
    let num_colors = palette_sorted.len();
    let mut palette = palette_sorted.to_vec();
    if !palette_has_non_monotonous_deltas(palette_sorted) {
        return palette;
    }
    let mut predict: u32 = 0x0000_0000;
    for i in 0..num_colors {
        let mut best_ix = i;
        let mut best_score = u32::MAX;
        for (k, &color) in palette.iter().enumerate().skip(i) {
            let cur_score = palette_color_distance(color, predict);
            if best_score > cur_score {
                best_score = cur_score;
                best_ix = k;
            }
        }
        palette.swap(best_ix, i);
        predict = palette[i];
    }
    palette
}
