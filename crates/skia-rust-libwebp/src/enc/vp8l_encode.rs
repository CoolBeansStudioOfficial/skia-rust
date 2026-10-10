// Copyright 2013 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the lossless encoder driver of libwebp `src/enc/vp8l_enc.c` for `method` 0 to 4 with
//! `near_lossless == 100` (no quantisation), as `SkWebpEncoder` (`method 0`, lossless) and the
//! alpha plane of lossy pictures (`method 3`, `exact == 1`) reach it: `EncoderAnalyze`,
//! `AnalyzeEntropy`, the crunch configurations, `EncodeStreamHook`, `ApplyPredictFilter`,
//! `ApplyCrossColorFilter`, `EncodePalette`, `MapImageFromPalette` and `EncodeImageInternal`.
//!
//! The C code keeps a best-so-far bit writer and swaps it in; the port builds each candidate
//! from a copy of the writer state at the start of the candidate, and keeps the smallest, which
//! gives the same bytes. Methods 5 and 6 (brute-force crunch configurations, the modified Zeng
//! palette order) are not reached by `SkWebpEncoder` and are rejected.

// Module-level clippy allows. The C arithmetic mixes int, uint32_t, size_t and float, and the
// casts below are the width and sign conversions of the C source. The index loops and
// `if`/`else` chains keep the C control flow, so that the code can be read against the C.
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
    clippy::fn_params_excessive_bools,
    clippy::struct_excessive_bools
)]

use super::backward_refs::{HashChain, get_backward_references_low_effort, hash_chain_fill};
use super::backward_refs_select::{
    K_LZ77_BOX, K_LZ77_RLE, K_LZ77_STANDARD, get_backward_references_with_cache,
};
use super::bit_writer::BitWriter;
use super::color_transform::color_space_transform;
use super::entropy::bits_entropy_unrefined;
use super::entropy::{bits_log2_floor, fast_log2};
use super::histogram::bits_entropy_refine;
use super::histogram::{Histogram, HistogramSet, get_histo_image_symbols};
use super::palette::{apply_palette, get_color_palette};
use super::palette_sort::palette_sort_minimize_deltas;
use super::predictor::{residual_image, sub_pixels, subtract_green_from_blue_and_red};
use super::vp8l::{
    clear_huffman_tree_if_only_one_symbol, encode_image_no_huffman, get_histo_bits,
    get_huff_bit_lengths_and_codes, get_transform_bits, store_huffman_code,
    store_image_to_bit_mask, sub_sample_size,
};

/// Port of `TRANSFORM_PRESENT`.
const TRANSFORM_PRESENT: u32 = 1;
/// Port of `PREDICTOR_TRANSFORM`.
const PREDICTOR_TRANSFORM: u32 = 0;
/// Port of `CROSS_COLOR_TRANSFORM`.
const CROSS_COLOR_TRANSFORM: u32 = 1;
/// Port of `SUBTRACT_GREEN_TRANSFORM`.
const SUBTRACT_GREEN_TRANSFORM: u32 = 2;
/// Port of `COLOR_INDEXING_TRANSFORM`.
const COLOR_INDEXING_TRANSFORM: u32 = 3;
/// Port of `MAX_COLOR_CACHE_BITS`.
const MAX_COLOR_CACHE_BITS: i32 = 10;

/// The configuration fields of `WebPConfig` that the lossless encoder reads.
#[derive(Clone, Copy, Debug)]
pub struct EncodeConfig {
    /// `config->method`: the effort, 0 (low effort) to 4 here.
    pub method: u32,
    /// `config->quality` (as the `int` the encoder uses).
    pub quality: i32,
    /// `config->exact`: keep the colours of fully transparent pixels.
    pub exact: bool,
}

/// Port of `EntropyIx`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EntropyIx {
    Direct,
    Spatial,
    SubGreen,
    SpatialSubGreen,
    Palette,
    PaletteAndSpatial,
}

/// Port of `PaletteSorting` (the orderings reached by `method` 0 to 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaletteSorting {
    SortedDefault,
    MinimizeDelta,
    Unused,
}

/// Port of `CrunchSubConfig`.
#[derive(Clone, Copy, Debug)]
struct CrunchSubConfig {
    lz77: u32,
}

/// Port of `CrunchConfig`.
#[derive(Clone, Debug)]
struct CrunchConfig {
    entropy_idx: EntropyIx,
    palette_sorting_type: PaletteSorting,
    sub_configs: Vec<CrunchSubConfig>,
}

/// Port of `HashPix` (`AnalyzeEntropy`'s palette hash).
fn hash_pix_entropy(pix: u32) -> usize {
    let v = ((u64::from(pix) + u64::from(pix >> 19)) * 0x39c5_fba7u64) & 0xffff_ffffu64;
    (v >> 24) as usize
}

/// Port of `AnalyzeEntropy`: the entropy estimate of each of the transforms, and the choice of
/// the cheapest one. Returns `(min_entropy_ix, red_and_blue_always_zero)`.
fn analyze_entropy(
    argb: &[u32],
    width: usize,
    height: usize,
    use_palette: bool,
    palette_size: usize,
    transform_bits: u32,
) -> (EntropyIx, bool) {
    // Indices of the histograms, as the C `HistoIx` enum.
    const K_HISTO_ALPHA: usize = 0;
    const K_HISTO_ALPHA_PRED: usize = 1;
    const K_HISTO_GREEN: usize = 2;
    const K_HISTO_GREEN_PRED: usize = 3;
    const K_HISTO_RED: usize = 4;
    const K_HISTO_RED_PRED: usize = 5;
    const K_HISTO_BLUE: usize = 6;
    const K_HISTO_BLUE_PRED: usize = 7;
    const K_HISTO_RED_SUB_GREEN: usize = 8;
    const K_HISTO_RED_PRED_SUB_GREEN: usize = 9;
    const K_HISTO_BLUE_SUB_GREEN: usize = 10;
    const K_HISTO_BLUE_PRED_SUB_GREEN: usize = 11;
    const K_HISTO_PALETTE: usize = 12;
    const K_HISTO_TOTAL: usize = 13;

    if use_palette && palette_size <= 16 {
        return (EntropyIx::Palette, true);
    }
    let mut histo = vec![0u32; K_HISTO_TOTAL * 256];
    // AddSingle: alpha, red, green and blue counts of one pixel.
    let add_single = |histo: &mut Vec<u32>, p: u32, a: usize, r: usize, g: usize, b: usize| {
        histo[a * 256 + ((p >> 24) & 0xff) as usize] += 1;
        histo[r * 256 + ((p >> 16) & 0xff) as usize] += 1;
        histo[g * 256 + ((p >> 8) & 0xff) as usize] += 1;
        histo[b * 256 + (p & 0xff) as usize] += 1;
    };
    // AddSingleSubGreen: red and blue minus green.
    let add_single_sub_green = |histo: &mut Vec<u32>, p: u32, r: usize, b: usize| {
        let green = (p >> 8) & 0xff;
        histo[r * 256 + (((p >> 16) & 0xff).wrapping_sub(green) & 0xff) as usize] += 1;
        histo[b * 256 + ((p & 0xff).wrapping_sub(green) & 0xff) as usize] += 1;
    };
    let mut pix_prev = argb[0]; // Skip the first pixel.
    let mut prev_row: Option<usize> = None;
    let mut curr_row: usize = 0;
    for _y in 0..height {
        for x in 0..width {
            let pix = argb[curr_row + x];
            let pix_diff = sub_pixels(pix, pix_prev);
            pix_prev = pix;
            if pix_diff == 0 || (prev_row.is_some() && pix == argb[prev_row.unwrap() + x]) {
                continue;
            }
            add_single(
                &mut histo,
                pix,
                K_HISTO_ALPHA,
                K_HISTO_RED,
                K_HISTO_GREEN,
                K_HISTO_BLUE,
            );
            add_single(
                &mut histo,
                pix_diff,
                K_HISTO_ALPHA_PRED,
                K_HISTO_RED_PRED,
                K_HISTO_GREEN_PRED,
                K_HISTO_BLUE_PRED,
            );
            add_single_sub_green(
                &mut histo,
                pix,
                K_HISTO_RED_SUB_GREEN,
                K_HISTO_BLUE_SUB_GREEN,
            );
            add_single_sub_green(
                &mut histo,
                pix_diff,
                K_HISTO_RED_PRED_SUB_GREEN,
                K_HISTO_BLUE_PRED_SUB_GREEN,
            );
            let hash = hash_pix_entropy(pix);
            histo[K_HISTO_PALETTE * 256 + hash] += 1;
        }
        prev_row = Some(curr_row);
        curr_row += width;
    }
    let mut entropy_comp = [0f32; K_HISTO_TOTAL];
    // The "+1" counts for the zero residuals of the first pixel and of the prediction modes.
    histo[K_HISTO_RED_PRED_SUB_GREEN * 256] += 1;
    histo[K_HISTO_BLUE_PRED_SUB_GREEN * 256] += 1;
    histo[K_HISTO_RED_PRED * 256] += 1;
    histo[K_HISTO_GREEN_PRED * 256] += 1;
    histo[K_HISTO_BLUE_PRED * 256] += 1;
    histo[K_HISTO_ALPHA_PRED * 256] += 1;
    for j in 0..K_HISTO_TOTAL {
        entropy_comp[j] =
            bits_entropy_refine(&bits_entropy_unrefined(&histo[j * 256..j * 256 + 256]));
    }
    let mut entropy = [0f32; 6];
    entropy[0] = entropy_comp[K_HISTO_ALPHA]
        + entropy_comp[K_HISTO_RED]
        + entropy_comp[K_HISTO_GREEN]
        + entropy_comp[K_HISTO_BLUE];
    entropy[1] = entropy_comp[K_HISTO_ALPHA_PRED]
        + entropy_comp[K_HISTO_RED_PRED]
        + entropy_comp[K_HISTO_GREEN_PRED]
        + entropy_comp[K_HISTO_BLUE_PRED];
    entropy[2] = entropy_comp[K_HISTO_ALPHA]
        + entropy_comp[K_HISTO_RED_SUB_GREEN]
        + entropy_comp[K_HISTO_GREEN]
        + entropy_comp[K_HISTO_BLUE_SUB_GREEN];
    entropy[3] = entropy_comp[K_HISTO_ALPHA_PRED]
        + entropy_comp[K_HISTO_RED_PRED_SUB_GREEN]
        + entropy_comp[K_HISTO_GREEN_PRED]
        + entropy_comp[K_HISTO_BLUE_PRED_SUB_GREEN];
    entropy[4] = entropy_comp[K_HISTO_PALETTE];
    let tiles =
        (sub_sample_size(width, transform_bits) * sub_sample_size(height, transform_bits)) as f32;
    entropy[1] += tiles * fast_log2(14);
    entropy[3] += tiles * fast_log2(24);
    entropy[4] += (palette_size * 8) as f32;
    let last_mode_to_analyze = if use_palette { 4 } else { 3 };
    let mut min_entropy_ix: usize = 0;
    for k in 1..=last_mode_to_analyze {
        if entropy[min_entropy_ix] > entropy[k] {
            min_entropy_ix = k;
        }
    }
    let mut red_and_blue_always_zero = true;
    let pairs: [(usize, usize); 5] = [
        (K_HISTO_RED, K_HISTO_BLUE),
        (K_HISTO_RED_PRED, K_HISTO_BLUE_PRED),
        (K_HISTO_RED_SUB_GREEN, K_HISTO_BLUE_SUB_GREEN),
        (K_HISTO_RED_PRED_SUB_GREEN, K_HISTO_BLUE_PRED_SUB_GREEN),
        (K_HISTO_RED, K_HISTO_BLUE),
    ];
    let (red_ix, blue_ix) = pairs[min_entropy_ix];
    for i in 1..256 {
        if (histo[red_ix * 256 + i] | histo[blue_ix * 256 + i]) != 0 {
            red_and_blue_always_zero = false;
            break;
        }
    }
    let ix = match min_entropy_ix {
        0 => EntropyIx::Direct,
        1 => EntropyIx::Spatial,
        2 => EntropyIx::SubGreen,
        3 => EntropyIx::SpatialSubGreen,
        4 => EntropyIx::Palette,
        _ => EntropyIx::PaletteAndSpatial,
    };
    (ix, red_and_blue_always_zero)
}

/// Port of `EncoderAnalyze` for `method` 0 to 4 (`do_no_cache` is never set there). Returns the
/// crunch configurations and `red_and_blue_always_zero`.
fn encoder_analyze(
    argb: &[u32],
    width: usize,
    height: usize,
    config: &EncodeConfig,
    palette_size: usize,
    use_palette: bool,
    transform_bits: u32,
) -> Option<(Vec<CrunchConfig>, bool)> {
    let method = config.method;
    if method > 4 {
        return None;
    }
    let low_effort = method == 0;
    if low_effort {
        // AnalyzeEntropy is somewhat slow; the low-effort encoder skips it.
        let config = CrunchConfig {
            entropy_idx: if use_palette {
                EntropyIx::Palette
            } else {
                EntropyIx::SpatialSubGreen
            },
            palette_sorting_type: if use_palette {
                PaletteSorting::SortedDefault
            } else {
                PaletteSorting::Unused
            },
            sub_configs: vec![CrunchSubConfig {
                lz77: K_LZ77_STANDARD | K_LZ77_RLE,
            }],
        };
        return Some((vec![config], false));
    }
    // Try out multiple LZ77 on images with few colors.
    let n_lz77s = if palette_size > 0 && palette_size <= 16 {
        2
    } else {
        1
    };
    let (min_entropy_ix, red_and_blue_always_zero) = analyze_entropy(
        argb,
        width,
        height,
        use_palette,
        palette_size,
        transform_bits,
    );
    // Only choose the guessed best transform.
    let config = CrunchConfig {
        entropy_idx: min_entropy_ix,
        palette_sorting_type: if use_palette {
            PaletteSorting::MinimizeDelta
        } else {
            PaletteSorting::Unused
        },
        sub_configs: (0..n_lz77s)
            .map(|j| CrunchSubConfig {
                lz77: if j == 0 {
                    K_LZ77_STANDARD | K_LZ77_RLE
                } else {
                    K_LZ77_BOX
                },
            })
            .collect(),
    };
    Some((vec![config], red_and_blue_always_zero))
}

/// Port of `EncodePalette`: the palette transform header and its delta-coded palette.
fn encode_palette(bw: &mut BitWriter, low_effort: bool, palette: &[u32], quality_palette: i32) {
    let palette_size = palette.len();
    bw.put_bits(TRANSFORM_PRESENT, 1);
    bw.put_bits(COLOR_INDEXING_TRANSFORM, 2);
    bw.put_bits((palette_size - 1) as u32, 8);
    let mut tmp_palette = vec![0u32; palette_size];
    for i in (1..palette_size).rev() {
        tmp_palette[i] = sub_pixels(palette[i], palette[i - 1]);
    }
    tmp_palette[0] = palette[0];
    let mut hash_chain = HashChain::new(palette_size);
    encode_image_no_huffman(
        bw,
        &tmp_palette,
        &mut hash_chain,
        palette_size,
        1,
        quality_palette,
        low_effort,
    );
}

/// Port of `ApplyPredictFilter`: the predictor transform of `argb` (replaced by its residuals),
/// written to `bw` with its tile modes.
fn apply_predict_filter(
    bw: &mut BitWriter,
    argb: &mut [u32],
    width: usize,
    height: usize,
    transform_bits: u32,
    quality: i32,
    low_effort: bool,
    exact: bool,
) {
    let pred_bits = transform_bits;
    let transform_width = sub_sample_size(width, pred_bits);
    let transform_height = sub_sample_size(height, pred_bits);
    let transform_data = residual_image(width, height, pred_bits, low_effort, argb, exact);
    bw.put_bits(TRANSFORM_PRESENT, 1);
    bw.put_bits(PREDICTOR_TRANSFORM, 2);
    bw.put_bits(pred_bits - 2, 3);
    let mut hash_chain = HashChain::new(transform_width * transform_height);
    encode_image_no_huffman(
        bw,
        &transform_data,
        &mut hash_chain,
        transform_width,
        transform_height,
        quality,
        low_effort,
    );
}

/// Port of `ApplyCrossColorFilter`: the cross-colour transform of `argb` in place, written to
/// `bw` with its tile colour codes.
fn apply_cross_color_filter(
    bw: &mut BitWriter,
    argb: &mut [u32],
    width: usize,
    height: usize,
    transform_bits: u32,
    quality: i32,
    low_effort: bool,
) {
    let ccolor_transform_bits = transform_bits;
    let transform_width = sub_sample_size(width, ccolor_transform_bits);
    let transform_height = sub_sample_size(height, ccolor_transform_bits);
    let mut transform_data = vec![0u32; transform_width * transform_height];
    color_space_transform(
        width,
        height,
        ccolor_transform_bits,
        quality,
        argb,
        &mut transform_data,
    );
    bw.put_bits(TRANSFORM_PRESENT, 1);
    bw.put_bits(CROSS_COLOR_TRANSFORM, 2);
    bw.put_bits(ccolor_transform_bits - 2, 3);
    let mut hash_chain = HashChain::new(transform_width * transform_height);
    encode_image_no_huffman(
        bw,
        &transform_data,
        &mut hash_chain,
        transform_width,
        transform_height,
        quality,
        low_effort,
    );
}

/// Port of `EncodeImageInternal`: the entropy-coded image (`argb`, `width` x `height`) with its
/// histogram image, for each sub-configuration, keeping the smallest candidate. On return `bw`
/// holds that candidate and `cache_bits` the colour-cache size it uses.
fn encode_image_internal(
    bw: &mut BitWriter,
    argb: &[u32],
    width: usize,
    height: usize,
    quality: i32,
    low_effort: bool,
    sub_configs: &[CrunchSubConfig],
    cache_bits: &mut i32,
    histogram_bits: u32,
) {
    let histogram_image_xysize =
        sub_sample_size(width, histogram_bits) * sub_sample_size(height, histogram_bits);
    let mut hash_chain = HashChain::new(width * height);
    hash_chain_fill(&mut hash_chain, quality, argb, width, height, low_effort);
    let cache_bits_init = if *cache_bits == 0 {
        MAX_COLOR_CACHE_BITS
    } else {
        *cache_bits
    };
    let bw_init = bw.clone();
    let mut bw_best: Option<BitWriter> = None;
    let mut bw_size_best = usize::MAX;
    let mut cache_bits_out = *cache_bits;
    for sub_config in sub_configs {
        let (refs, cache_bits_tmp) = if low_effort {
            // GetBackwardReferencesLowEffort: no colour cache.
            (
                get_backward_references_low_effort(width, height, argb, &hash_chain),
                0,
            )
        } else {
            let mut cache_bits_best = 0;
            let refs = get_backward_references_with_cache(
                width,
                height,
                argb,
                quality,
                sub_config.lz77,
                cache_bits_init,
                &hash_chain,
                &mut cache_bits_best,
            );
            (refs, cache_bits_best)
        };
        let mut cand = bw_init.clone();
        let mut histogram_image = HistogramSet::new(histogram_image_xysize, cache_bits_tmp);
        let mut tmp_histo = Histogram::new(cache_bits_tmp);
        let mut histogram_symbols = vec![0u16; histogram_image_xysize];
        get_histo_image_symbols(
            width,
            height,
            &refs,
            quality,
            low_effort,
            histogram_bits as i32,
            cache_bits_tmp,
            &mut histogram_image,
            &mut tmp_histo,
            &mut histogram_symbols,
        );
        let mut histogram_image_size = histogram_image.size;
        let mut huffman_codes = get_huff_bit_lengths_and_codes(&mut histogram_image);
        if cache_bits_tmp > 0 {
            cand.put_bits(1, 1);
            cand.put_bits(cache_bits_tmp as u32, 4);
        } else {
            cand.put_bits(0, 1);
        }
        let write_histogram_image = histogram_image_size > 1;
        cand.put_bits(u32::from(write_histogram_image), 1);
        if write_histogram_image {
            let mut histogram_argb = vec![0u32; histogram_image_xysize];
            let mut max_index: usize = 0;
            for i in 0..histogram_image_xysize {
                let symbol_index = histogram_symbols[i] as usize;
                histogram_argb[i] = (symbol_index as u32) << 8;
                if symbol_index >= max_index {
                    max_index = symbol_index + 1;
                }
            }
            histogram_image_size = max_index;
            cand.put_bits(histogram_bits - 2, 3);
            let mut hash_chain_histogram = HashChain::new(histogram_image_xysize);
            encode_image_no_huffman(
                &mut cand,
                &histogram_argb,
                &mut hash_chain_histogram,
                sub_sample_size(width, histogram_bits),
                sub_sample_size(height, histogram_bits),
                quality,
                low_effort,
            );
        }
        for code in huffman_codes.iter_mut().take(5 * histogram_image_size) {
            store_huffman_code(&mut cand, code);
            clear_huffman_tree_if_only_one_symbol(code);
        }
        store_image_to_bit_mask(
            &mut cand,
            width,
            histogram_bits,
            &refs.refs,
            &histogram_symbols,
            &huffman_codes,
        );
        if cand.num_bytes() < bw_size_best {
            bw_size_best = cand.num_bytes();
            cache_bits_out = cache_bits_tmp;
            bw_best = Some(cand);
        }
    }
    if let Some(best) = bw_best {
        *bw = best;
    }
    *cache_bits = cache_bits_out;
}

/// Port of `EncodeStreamHook` and `VP8LEncodeStream` (no side worker, `thread_level == 0`):
/// tries each crunch configuration on a copy of the stream state and keeps the smallest.
/// Returns `false` for a configuration this port does not reach (`method` above 4).
#[must_use]
pub fn encode_stream(
    width: usize,
    height: usize,
    argb_in: &[u32],
    config: &EncodeConfig,
    bw: &mut BitWriter,
) -> bool {
    let palette_sorted = get_color_palette(argb_in, width, height, width);
    let use_palette = palette_sorted.is_some();
    let palette_sorted = palette_sorted.unwrap_or_default();
    let palette_size = palette_sorted.len();
    let histo_bits = get_histo_bits(config.method, use_palette, width, height);
    let transform_bits = get_transform_bits(config.method, histo_bits);
    let low_effort = config.method == 0;
    let quality = config.quality;
    let Some((crunch_configs, red_and_blue_always_zero)) = encoder_analyze(
        argb_in,
        width,
        height,
        config,
        palette_size,
        use_palette,
        transform_bits,
    ) else {
        return false;
    };
    let bw_init = bw.clone();
    let mut best: Option<BitWriter> = None;
    let mut best_size = usize::MAX;
    for crunch in &crunch_configs {
        let mut cand = bw_init.clone();
        let mut cache_bits: i32 = 0;
        let use_palette_cfg = crunch.entropy_idx == EntropyIx::Palette
            || crunch.entropy_idx == EntropyIx::PaletteAndSpatial;
        let use_subtract_green = crunch.entropy_idx == EntropyIx::SubGreen
            || crunch.entropy_idx == EntropyIx::SpatialSubGreen;
        let use_predict = crunch.entropy_idx == EntropyIx::Spatial
            || crunch.entropy_idx == EntropyIx::SpatialSubGreen
            || crunch.entropy_idx == EntropyIx::PaletteAndSpatial;
        let use_cross_color = if low_effort || use_palette_cfg {
            false
        } else {
            !red_and_blue_always_zero && use_predict
        };
        let mut current_width = width;
        let mut argb: Vec<u32>;
        if use_palette_cfg {
            let palette = match crunch.palette_sorting_type {
                PaletteSorting::MinimizeDelta => palette_sort_minimize_deltas(&palette_sorted),
                PaletteSorting::SortedDefault | PaletteSorting::Unused => palette_sorted.clone(),
            };
            encode_palette(&mut cand, low_effort, &palette, 20);
            // MapImageFromPalette.
            let xbits: u32 = if palette_size <= 4 {
                if palette_size <= 2 { 3 } else { 2 }
            } else {
                u32::from(palette_size <= 16)
            };
            argb = apply_palette(argb_in, width, &palette, width, height, xbits);
            current_width = sub_sample_size(width, xbits);
            if palette_size < (1 << MAX_COLOR_CACHE_BITS) as usize {
                cache_bits = bits_log2_floor(palette_size as u32) + 1;
            }
        } else {
            argb = argb_in.to_vec();
        }
        if use_subtract_green {
            cand.put_bits(TRANSFORM_PRESENT, 1);
            cand.put_bits(SUBTRACT_GREEN_TRANSFORM, 2);
            subtract_green_from_blue_and_red(&mut argb[..current_width * height]);
        }
        if use_predict {
            apply_predict_filter(
                &mut cand,
                &mut argb,
                current_width,
                height,
                transform_bits,
                quality,
                low_effort,
                config.exact,
            );
        }
        if use_cross_color {
            apply_cross_color_filter(
                &mut cand,
                &mut argb,
                current_width,
                height,
                transform_bits,
                quality,
                low_effort,
            );
        }
        cand.put_bits(0, 1); // No more transforms.
        encode_image_internal(
            &mut cand,
            &argb,
            current_width,
            height,
            quality,
            low_effort,
            &crunch.sub_configs,
            &mut cache_bits,
            histo_bits,
        );
        if cand.num_bytes() < best_size {
            best_size = cand.num_bytes();
            best = Some(cand);
        }
    }
    if let Some(best) = best {
        *bw = best;
    }
    true
}
