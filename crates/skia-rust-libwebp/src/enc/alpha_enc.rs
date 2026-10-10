// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the alpha-plane encoder of libwebp `src/enc/alpha_enc.c` (the `ALPH` chunk payload),
//! with the spatial filters of `src/dsp/filters.c`, `WebPEstimateBestFilter` of
//! `src/utils/filters_utils.c` and `WebPDispatchAlphaToGreen` of `src/dsp/alpha_processing.c`.
//!
//! The payload is one header byte (`method | filter << 2`) and the filtered alpha, either raw
//! (`ALPHA_NO_COMPRESSION`) or as a VP8L stream of the alpha in the green channel
//! (`ALPHA_LOSSLESS_COMPRESSION`, coded at `effort_level` = `config->method`, `exact = 1`).
//!
//! Scope: `SkWebpEncoder` encodes with `alpha_quality = 100`, so `reduce_levels` is always off
//! and `QuantizeLevels` (`src/utils/quant_levels_utils.c`) is not reached; it is not ported.

// Module-level clippy allows. The C arithmetic mixes int, uint32_t and uint8_t, and the casts
// below are the width and sign conversions of the C source.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::needless_range_loop
)]

use super::bit_writer::BitWriter;
use super::vp8l_encode::{EncodeConfig, encode_stream};

/// Port of `ALPHA_NO_COMPRESSION`.
const ALPHA_NO_COMPRESSION: u8 = 0;
/// Port of `ALPHA_LOSSLESS_COMPRESSION`.
const ALPHA_LOSSLESS_COMPRESSION: u8 = 1;
/// Port of the `WEBP_FILTER_TYPE` values that `EncodeAlphaInternal` applies (`NONE` to `GRADIENT`).
const WEBP_FILTER_NONE: usize = 0;
const WEBP_FILTER_HORIZONTAL: usize = 1;
const WEBP_FILTER_VERTICAL: usize = 2;
const WEBP_FILTER_GRADIENT: usize = 3;
/// Port of `WEBP_FILTER_LAST`.
const WEBP_FILTER_LAST: usize = 4;
/// Port of `FILTER_TRY_NONE`.
const FILTER_TRY_NONE: u32 = 1 << WEBP_FILTER_NONE;
/// Port of `FILTER_TRY_ALL`.
const FILTER_TRY_ALL: u32 = (1 << WEBP_FILTER_LAST) - 1;

/// Port of `config->alpha_filtering`: 0 (none), 1 (fast: estimate one filter) and 2 (best: try
/// all). `SkWebpEncoder` keeps the default of `WebPConfigInit`, which is 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaFiltering {
    None,
    Fast,
    Best,
}

/// Port of `WebPPictureHasTransparency` for a YUV picture: whether any alpha is not 0xff.
#[must_use]
pub fn has_transparency(alpha: &[u8]) -> bool {
    alpha.iter().any(|&a| a != 0xff)
}

/// Port of `GradientPredictor_C` (`filters.c`) and of `GradientPredictor` (`filters_utils.c`):
/// the clipped gradient of three neighbours.
fn gradient_predictor(a: u8, b: u8, c: u8) -> i32 {
    let g = i32::from(a) + i32::from(b) - i32::from(c);
    if (g & !0xff) == 0 {
        g
    } else if g < 0 {
        0
    } else {
        255
    }
}

/// Port of `DoHorizontalFilter_C` with `inverse == 0` over the whole picture.
fn horizontal_filter(data: &[u8], width: usize, height: usize, stride: usize, out: &mut [u8]) {
    for row in 0..height {
        let base = row * stride;
        if row == 0 {
            out[base] = data[base];
            for x in 1..width {
                out[base + x] = data[base + x].wrapping_sub(data[base + x - 1]);
            }
        } else {
            // The first column predicts from the row above, the others from the left.
            out[base] = data[base].wrapping_sub(data[base - stride]);
            for x in 1..width {
                out[base + x] = data[base + x].wrapping_sub(data[base + x - 1]);
            }
        }
    }
}

/// Port of `DoVerticalFilter_C` with `inverse == 0` over the whole picture: the first row is
/// horizontally predicted, the others from the row above.
fn vertical_filter(data: &[u8], width: usize, height: usize, stride: usize, out: &mut [u8]) {
    for row in 0..height {
        let base = row * stride;
        if row == 0 {
            out[base] = data[base];
            for x in 1..width {
                out[base + x] = data[base + x].wrapping_sub(data[base + x - 1]);
            }
        } else {
            for x in 0..width {
                out[base + x] = data[base + x].wrapping_sub(data[base - stride + x]);
            }
        }
    }
}

/// Port of `DoGradientFilter_C` with `inverse == 0` over the whole picture.
fn gradient_filter(data: &[u8], width: usize, height: usize, stride: usize, out: &mut [u8]) {
    for row in 0..height {
        let base = row * stride;
        if row == 0 {
            out[base] = data[base];
            for x in 1..width {
                out[base + x] = data[base + x].wrapping_sub(data[base + x - 1]);
            }
        } else {
            out[base] = data[base].wrapping_sub(data[base - stride]);
            for w in 1..width {
                let pred = gradient_predictor(
                    data[base + w - 1],
                    data[base - stride + w],
                    data[base - stride + w - 1],
                );
                out[base + w] = (i32::from(data[base + w]) - pred) as u8;
            }
        }
    }
}

/// Port of `WebPFilters[filter]` applied to `data` (`stride == width`): `None` for
/// `WEBP_FILTER_NONE`, whose filter is the identity.
fn apply_filter(filter: usize, data: &[u8], width: usize, height: usize, out: &mut [u8]) {
    match filter {
        WEBP_FILTER_HORIZONTAL => horizontal_filter(data, width, height, width, out),
        WEBP_FILTER_VERTICAL => vertical_filter(data, width, height, width, out),
        WEBP_FILTER_GRADIENT => gradient_filter(data, width, height, width, out),
        _ => out.copy_from_slice(data),
    }
}

/// Port of `WebPEstimateBestFilter`: samples every other pixel of the rows and picks the filter
/// whose residuals fall in the fewest histogram bins.
#[must_use]
pub fn estimate_best_filter(data: &[u8], width: usize, height: usize, stride: usize) -> usize {
    const SMAX: usize = 16;
    let sdiff = |a: i32, b: i32| ((a - b).abs() >> 4) as usize;
    let mut bins = [[0u8; SMAX]; WEBP_FILTER_LAST];
    let mut j = 2usize;
    while j + 1 < height {
        // `p` is row j; `p[i - stride]` of the C code is the row above, reached as
        // `data[row + i - stride]` (row >= 2 * stride, so the index is never negative).
        let row = j * stride;
        let mut mean = i32::from(data[row]);
        let mut i = 2usize;
        while i + 1 < width {
            let pi = i32::from(data[row + i]);
            let diff0 = sdiff(pi, mean);
            let diff1 = sdiff(pi, i32::from(data[row + i - 1]));
            let diff2 = sdiff(pi, i32::from(data[row + i - stride]));
            let grad_pred = gradient_predictor(
                data[row + i - 1],
                data[row + i - stride],
                data[row + i - stride - 1],
            );
            let diff3 = sdiff(pi, grad_pred);
            bins[WEBP_FILTER_NONE][diff0] = 1;
            bins[WEBP_FILTER_HORIZONTAL][diff1] = 1;
            bins[WEBP_FILTER_VERTICAL][diff2] = 1;
            bins[WEBP_FILTER_GRADIENT][diff3] = 1;
            mean = (3 * mean + pi + 2) >> 2;
            i += 2;
        }
        j += 2;
    }
    let mut best_filter = WEBP_FILTER_NONE;
    let mut best_score = 0x7fff_ffffi64;
    for filter in WEBP_FILTER_NONE..WEBP_FILTER_LAST {
        let mut score: i64 = 0;
        for i in 0..SMAX {
            if bins[filter][i] > 0 {
                score += i as i64;
            }
        }
        if score < best_score {
            best_score = score;
            best_filter = filter;
        }
    }
    best_filter
}

/// Port of `GetNumColors`: the number of distinct byte values of the picture.
fn get_num_colors(data: &[u8], width: usize, height: usize, stride: usize) -> usize {
    let mut color = [false; 256];
    for j in 0..height {
        for i in 0..width {
            color[data[j * stride + i] as usize] = true;
        }
    }
    color.iter().filter(|&&c| c).count()
}

/// Port of `GetFilterMap`: the set of filters to try, as a bit map.
fn get_filter_map(
    alpha: &[u8],
    width: usize,
    height: usize,
    filtering: AlphaFiltering,
    effort_level: u32,
) -> u32 {
    match filtering {
        AlphaFiltering::Fast => {
            let try_filter_none = effort_level > 3;
            let k_min_colors_for_filter_none = 16;
            let k_max_colors_for_filter_none = 192;
            let num_colors = get_num_colors(alpha, width, height, width);
            let filter = if num_colors <= k_min_colors_for_filter_none {
                WEBP_FILTER_NONE
            } else {
                estimate_best_filter(alpha, width, height, width)
            };
            let mut bit_map = 1u32 << filter;
            if try_filter_none || num_colors > k_max_colors_for_filter_none {
                bit_map |= FILTER_TRY_NONE;
            }
            bit_map
        }
        AlphaFiltering::None => FILTER_TRY_NONE,
        AlphaFiltering::Best => FILTER_TRY_ALL,
    }
}

/// Port of `EncodeLossless` (`alpha_enc.c`): the alpha of the (filtered) picture in the green
/// channel, coded as a VP8L stream at `method = effort_level`, `exact = 1`, with the quality
/// `8 * effort_level` (`use_quality_100` is set, as `reduce_levels` is off).
fn encode_lossless(data: &[u8], width: usize, height: usize, effort_level: u32) -> Option<Vec<u8>> {
    let argb: Vec<u32> = data.iter().map(|&a| u32::from(a) << 8).collect();
    let quality = if effort_level == 6 {
        100
    } else {
        8 * effort_level as i32
    };
    let config = EncodeConfig {
        method: effort_level,
        quality,
        exact: true,
    };
    let mut bw = BitWriter::new();
    if !encode_stream(width, height, &argb, &config, &mut bw) {
        return None;
    }
    Some(bw.finish())
}

/// Port of `EncodeAlphaInternal` (no level quantisation): the `ALPH` payload for one filter.
fn encode_alpha_internal(
    data: &[u8],
    width: usize,
    height: usize,
    method: u8,
    filter: usize,
    effort_level: u32,
) -> Option<Vec<u8>> {
    let data_size = width * height;
    let mut alpha_src = vec![0u8; data_size];
    apply_filter(filter, data, width, height, &mut alpha_src);
    let mut method = method;
    let mut output: Vec<u8> = Vec::new();
    if method != ALPHA_NO_COMPRESSION {
        let coded = encode_lossless(&alpha_src, width, height, effort_level)?;
        if coded.len() > data_size {
            method = ALPHA_NO_COMPRESSION;
        } else {
            output = coded;
        }
    }
    if method == ALPHA_NO_COMPRESSION {
        output = alpha_src;
    }
    let header = method | ((filter as u8) << 2);
    let mut result = Vec::with_capacity(1 + output.len());
    result.push(header);
    result.extend_from_slice(&output);
    Some(result)
}

/// Port of `EncodeAlpha` for `quality == 100` (`reduce_levels == 0`) and `alpha_compression`
/// lossless, with the filters of `filtering` and `effort_level` = `config->method`. Returns the
/// `ALPH` chunk payload, the smallest of the tried candidates (the first one on a tie).
#[must_use]
// Mirrors the C `if (try_map != FILTER_TRY_NONE)`, which tries the filters and falls back to none.
#[allow(clippy::if_not_else)]
pub fn encode_alpha(
    alpha: &[u8],
    width: usize,
    height: usize,
    filtering: AlphaFiltering,
    effort_level: u32,
) -> Option<Vec<u8>> {
    let method = ALPHA_LOSSLESS_COMPRESSION;
    let mut try_map = get_filter_map(alpha, width, height, filtering, effort_level);
    if try_map != FILTER_TRY_NONE {
        let mut best: Option<Vec<u8>> = None;
        for filter in WEBP_FILTER_NONE..WEBP_FILTER_LAST {
            if try_map & 1 != 0 {
                let trial =
                    encode_alpha_internal(alpha, width, height, method, filter, effort_level)?;
                let better = match &best {
                    None => true,
                    Some(b) => trial.len() < b.len(),
                };
                if better {
                    best = Some(trial);
                }
            }
            try_map >>= 1;
        }
        best
    } else {
        encode_alpha_internal(alpha, width, height, method, WEBP_FILTER_NONE, effort_level)
    }
}
