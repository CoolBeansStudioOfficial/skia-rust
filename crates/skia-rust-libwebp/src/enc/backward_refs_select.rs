// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the parse selection of libwebp `src/enc/backward_references_enc.c`:
//! `BackwardReferencesRle`, `BackwardReferencesLz77Box`, `CalculateBestCacheSize`,
//! `BackwardRefsWithLocalCache`, `GetBackwardReferences` and `VP8LGetBackwardReferences` with
//! `low_effort == 0` and `do_no_cache == 0` (the setting of `method` 0 to 4; `do_no_cache` is
//! only set by `method` 5 and 6, which `SkWebpEncoder` does not reach).

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

use super::backward_refs::{
    BackwardRefs, HashChain, MAX_LENGTH, MAX_LENGTH_BITS, MIN_LENGTH, PixOrCopy,
    backward_references_2d_locality, backward_references_lz77, find_match_length,
    max_find_copy_length,
};
use super::backward_refs_cost::backward_references_trace_backwards;
use super::color_cache::ColorCache;
use super::histogram::{Histogram, NUM_LENGTH_CODES, NUM_LITERAL_CODES, histogram_estimate_bits};
use super::prefix::prefix_encode_bits;

/// Port of `kLZ77Standard`.
pub const K_LZ77_STANDARD: u32 = 1;
/// Port of `kLZ77RLE`.
pub const K_LZ77_RLE: u32 = 2;
/// Port of `kLZ77Box`.
pub const K_LZ77_BOX: u32 = 4;
/// Port of `MAX_ENTROPY` (`1e30f`).
const MAX_ENTROPY: f32 = 1e30;
/// Port of `WINDOW_OFFSETS_SIZE_MAX`.
const WINDOW_OFFSETS_SIZE_MAX: usize = 32;

/// Port of `BackwardReferencesRle` with no colour cache: runs of the previous pixel, and copies
/// from the previous row, where they are long enough (`MIN_LENGTH`).
#[must_use]
pub fn backward_references_rle(xsize: usize, ysize: usize, argb: &[u32]) -> BackwardRefs {
    let pix_count = xsize * ysize;
    let mut refs = BackwardRefs::default();
    refs.push(PixOrCopy::literal(argb[0]));
    let min_length = MIN_LENGTH as usize;
    let mut i: usize = 1;
    while i < pix_count {
        let max_len = max_find_copy_length(pix_count - i);
        let rle_len = find_match_length(&argb[i..], &argb[i - 1..], 0, max_len);
        let prev_row_len = if i < xsize {
            0
        } else {
            find_match_length(&argb[i..], &argb[i - xsize..], 0, max_len)
        };
        if rle_len >= prev_row_len && rle_len >= min_length {
            refs.push(PixOrCopy::copy(1, rle_len as u32));
            i += rle_len;
        } else if prev_row_len >= min_length {
            refs.push(PixOrCopy::copy(xsize as u32, prev_row_len as u32));
            i += prev_row_len;
        } else {
            refs.push(PixOrCopy::literal(argb[i]));
            i += 1;
        }
    }
    refs
}

/// Port of `BackwardReferencesLz77Box`: the hash chain of the longest match among the window
/// offsets of the 2D neighbourhood (`hash_chain`, which the caller provides empty), then the
/// LZ77 parse of it.
#[must_use]
pub fn backward_references_lz77_box(
    xsize: usize,
    ysize: usize,
    argb: &[u32],
    hash_chain_best: &HashChain,
    hash_chain: &mut HashChain,
) -> BackwardRefs {
    let pix_count = xsize * ysize;
    let max_length = MAX_LENGTH as u16;
    let mut counts = vec![0u16; pix_count];
    let mut window_offsets = [0i32; WINDOW_OFFSETS_SIZE_MAX];
    let mut window_offsets_new = [0i32; WINDOW_OFFSETS_SIZE_MAX];
    let mut window_offsets_size: usize = 0;
    let mut window_offsets_new_size: usize = 0;
    let mut best_offset_prev: i32 = -1;
    let mut best_length_prev: i32 = -1;
    // counts[i] is the run of pixels equal to argb[i] starting at i (capped at MAX_LENGTH).
    if pix_count >= 1 {
        counts[pix_count - 1] = 1;
    }
    if pix_count >= 2 {
        let mut i = pix_count - 2;
        loop {
            if argb[i] == argb[i + 1] {
                counts[i] = counts[i + 1] + u16::from(counts[i + 1] != max_length);
            } else {
                counts[i] = 1;
            }
            if i == 0 {
                break;
            }
            i -= 1;
        }
    }
    {
        for y in 0..=6i32 {
            for x in -6..=6i32 {
                let offset = y * xsize as i32 + x;
                if offset <= 0 {
                    continue;
                }
                let plane_code =
                    super::backward_refs::distance_to_plane_code(xsize as i32, offset) - 1;
                if plane_code < 0 || plane_code as usize >= WINDOW_OFFSETS_SIZE_MAX {
                    continue;
                }
                window_offsets[plane_code as usize] = offset;
            }
        }
        // Compact the non-zero offsets to the front, in order.
        for i in 0..WINDOW_OFFSETS_SIZE_MAX {
            if window_offsets[i] == 0 {
                continue;
            }
            window_offsets[window_offsets_size] = window_offsets[i];
            window_offsets_size += 1;
        }
        for i in 0..window_offsets_size {
            let mut is_reachable = false;
            for j in 0..window_offsets_size {
                if is_reachable {
                    break;
                }
                is_reachable |= window_offsets[i] == window_offsets[j] + 1;
            }
            if !is_reachable {
                window_offsets_new[window_offsets_new_size] = window_offsets[i];
                window_offsets_new_size += 1;
            }
        }
    }
    if pix_count > 0 {
        hash_chain.offset_length[0] = 0;
    }
    for i in 1..pix_count {
        let mut best_length = hash_chain_best.find_length(i);
        let mut best_offset: i32 = 0;
        let mut do_compute = true;
        if best_length >= MAX_LENGTH as i32 {
            best_offset = hash_chain_best.find_offset(i);
            for ind in 0..window_offsets_size {
                if best_offset == window_offsets[ind] {
                    do_compute = false;
                    break;
                }
            }
        }
        if do_compute {
            let use_prev = best_length_prev > 1 && best_length_prev < MAX_LENGTH as i32;
            let num_ind = if use_prev {
                window_offsets_new_size
            } else {
                window_offsets_size
            };
            best_length = if use_prev { best_length_prev - 1 } else { 0 };
            best_offset = if use_prev { best_offset_prev } else { 0 };
            for ind in 0..num_ind {
                let mut curr_length: i32 = 0;
                let mut j = i;
                let j_offset_signed = if use_prev {
                    i as i64 - i64::from(window_offsets_new[ind])
                } else {
                    i as i64 - i64::from(window_offsets[ind])
                };
                if j_offset_signed < 0 || argb[j_offset_signed as usize] != argb[i] {
                    continue;
                }
                let mut j_offset = j_offset_signed as usize;
                loop {
                    let counts_j_offset = i32::from(counts[j_offset]);
                    let counts_j = i32::from(counts[j]);
                    if counts_j_offset != counts_j {
                        curr_length += counts_j_offset.min(counts_j);
                        break;
                    }
                    curr_length += counts_j_offset;
                    j_offset += counts_j_offset as usize;
                    j += counts_j_offset as usize;
                    if !(curr_length <= MAX_LENGTH as i32
                        && j < pix_count
                        && argb[j_offset] == argb[j])
                    {
                        break;
                    }
                }
                if best_length < curr_length {
                    best_offset = if use_prev {
                        window_offsets_new[ind]
                    } else {
                        window_offsets[ind]
                    };
                    if curr_length >= MAX_LENGTH as i32 {
                        best_length = MAX_LENGTH as i32;
                        break;
                    } else {
                        best_length = curr_length;
                    }
                }
            }
        }
        if best_length <= MIN_LENGTH {
            hash_chain.offset_length[i] = 0;
            best_offset_prev = 0;
            best_length_prev = 0;
        } else {
            hash_chain.offset_length[i] =
                ((best_offset as u32) << MAX_LENGTH_BITS) | (best_length as u32);
            best_offset_prev = best_offset;
            best_length_prev = best_length;
        }
    }
    if pix_count > 0 {
        hash_chain.offset_length[0] = 0;
    }
    backward_references_lz77(xsize, ysize, argb, hash_chain)
}

/// Port of `CalculateBestCacheSize`: the colour-cache size (at most `*best_cache_bits`) that
/// gives the lowest estimated entropy for the parse `refs`.
fn calculate_best_cache_size(
    argb: &[u32],
    quality: i32,
    refs: &BackwardRefs,
    best_cache_bits: &mut i32,
) {
    let cache_bits_max = if quality <= 25 { 0 } else { *best_cache_bits };
    if cache_bits_max == 0 {
        // Local colour cache is disabled.
        *best_cache_bits = 0;
        return;
    }
    let max_bits = cache_bits_max as usize;
    let mut histos: Vec<Histogram> = (0..=cache_bits_max).map(Histogram::new).collect();
    let mut hashers: Vec<Option<ColorCache>> = (0..=max_bits)
        .map(|i| (i > 0).then(|| ColorCache::new(i as u32)))
        .collect();
    let shift_max = (32 - cache_bits_max) as u32;
    let mut pos: usize = 0;
    let mut k: usize = 0;
    while k < refs.refs.len() {
        let v = refs.refs[k];
        if v.is_literal() {
            let pix = argb[pos];
            pos += 1;
            let a = (pix >> 24) & 0xff;
            let r = (pix >> 16) & 0xff;
            let g = (pix >> 8) & 0xff;
            let b = pix & 0xff;
            // The keys of the caches can be derived from the longest one.
            let mut key = super::palette::hash_pix(pix, shift_max);
            // Do not use the color cache for cache_bits = 0.
            histos[0].blue[b as usize] += 1;
            histos[0].literal[g as usize] += 1;
            histos[0].red[r as usize] += 1;
            histos[0].alpha[a as usize] += 1;
            // Deal with cache_bits > 0.
            for i in (1..=max_bits).rev() {
                let hasher = hashers[i].as_mut().expect("cache");
                if hasher.lookup(key) == pix {
                    histos[i].literal[NUM_LITERAL_CODES + NUM_LENGTH_CODES + key] += 1;
                } else {
                    hasher.set(key, pix);
                    histos[i].blue[b as usize] += 1;
                    histos[i].literal[g as usize] += 1;
                    histos[i].red[r as usize] += 1;
                    histos[i].alpha[a as usize] += 1;
                }
                key >>= 1;
            }
        } else {
            // We should compute the contribution of the (distance,length) histograms but those
            // are the same independently from the cache size, so only the length prefix counts.
            let mut len = v.length() as usize;
            let mut argb_prev = argb[pos] ^ 0xffff_ffff;
            let (code, _extra_bits) = prefix_encode_bits(len as i32);
            for i in 0..=max_bits {
                histos[i].literal[NUM_LITERAL_CODES + code as usize] += 1;
            }
            // Update the color caches.
            loop {
                if argb[pos] != argb_prev {
                    // Efficiency: insert only if the color changes.
                    let mut key = super::palette::hash_pix(argb[pos], shift_max);
                    for i in (1..=max_bits).rev() {
                        hashers[i].as_mut().expect("cache").set(key, argb[pos]);
                        key >>= 1;
                    }
                    argb_prev = argb[pos];
                }
                pos += 1;
                len -= 1;
                if len == 0 {
                    break;
                }
            }
        }
        k += 1;
    }
    let mut entropy_min = MAX_ENTROPY;
    for i in 0..=cache_bits_max {
        let entropy = histogram_estimate_bits(&mut histos[i as usize]);
        if i == 0 || entropy < entropy_min {
            entropy_min = entropy;
            *best_cache_bits = i;
        }
    }
}

/// Port of `BackwardRefsWithLocalCache`: rewrites the literals of `refs` that are in the colour
/// cache of `cache_bits` as cache indices.
fn backward_refs_with_local_cache(argb: &[u32], cache_bits: i32, refs: &mut BackwardRefs) {
    let mut pixel_index: usize = 0;
    let mut hashers = ColorCache::new(cache_bits as u32);
    for v in refs.refs.iter_mut() {
        if v.is_literal() {
            let argb_literal = v.argb_or_distance;
            match hashers.contains(argb_literal) {
                Some(ix) => {
                    // hashers contains argb_literal
                    *v = PixOrCopy::cache_idx(ix as u32);
                }
                None => hashers.insert(argb_literal),
            }
            pixel_index += 1;
        } else {
            // refs was created without local cache, so it can not have cache indexes.
            for _ in 0..v.length() {
                hashers.insert(argb[pixel_index]);
                pixel_index += 1;
            }
        }
    }
}

/// Estimated bit cost of `refs` coded with a colour cache of `cache_bits`
/// (`VP8LHistogramCreate` followed by `VP8LHistogramEstimateBits`).
fn refs_bit_cost(refs: &BackwardRefs, cache_bits: i32) -> f32 {
    let mut histo = Histogram::new(cache_bits);
    histo.create(refs, cache_bits);
    histogram_estimate_bits(&mut histo)
}

/// Port of `GetBackwardReferences` for `do_no_cache == 0`: tries each LZ77 type in
/// `lz77_types_to_try` (in the order of the C bit loop), each with the best colour cache of at
/// most `cache_bits_max` bits, keeps the cheapest, and improves a standard or box parse by its
/// cost-model trace at `quality >= 25`. Writes the chosen cache size to `cache_bits_best`.
#[must_use]
pub(super) fn get_backward_references_with_cache(
    width: usize,
    height: usize,
    argb: &[u32],
    quality: i32,
    lz77_types_to_try: u32,
    cache_bits_max: i32,
    hash_chain: &HashChain,
    cache_bits_best: &mut i32,
) -> BackwardRefs {
    let mut refs_best = BackwardRefs::default();
    let mut bit_cost_best = f32::MAX;
    let mut lz77_type_best: u32 = 0;
    let mut hash_chain_box: Option<HashChain> = None;
    let mut types = lz77_types_to_try;
    let mut lz77_type: u32 = 1;
    while types != 0 {
        if types & lz77_type != 0 {
            let mut refs_tmp = match lz77_type {
                K_LZ77_RLE => backward_references_rle(width, height, argb),
                K_LZ77_STANDARD => {
                    // Compute LZ77 with no cache (0 bits), as the ideal LZ77 with a color cache is
                    // not that different in practice.
                    backward_references_lz77(width, height, argb, hash_chain)
                }
                _ => {
                    let mut hcb = HashChain::new(width * height);
                    let refs =
                        backward_references_lz77_box(width, height, argb, hash_chain, &mut hcb);
                    hash_chain_box = Some(hcb);
                    refs
                }
            };
            // Try with a color cache.
            let mut cache_bits = cache_bits_max;
            calculate_best_cache_size(argb, quality, &refs_tmp, &mut cache_bits);
            if cache_bits > 0 {
                backward_refs_with_local_cache(argb, cache_bits, &mut refs_tmp);
            }
            let bit_cost = refs_bit_cost(&refs_tmp, cache_bits);
            if bit_cost < bit_cost_best {
                refs_best = refs_tmp;
                bit_cost_best = bit_cost;
                lz77_type_best = lz77_type;
                *cache_bits_best = cache_bits;
            }
        }
        types &= !lz77_type;
        lz77_type <<= 1;
    }
    // Improve on simple LZ77 but only for high quality (TraceBackwards is costly).
    if (lz77_type_best == K_LZ77_STANDARD || lz77_type_best == K_LZ77_BOX) && quality >= 25 {
        let hash_chain_tmp: &HashChain = if lz77_type_best == K_LZ77_STANDARD {
            hash_chain
        } else {
            hash_chain_box.as_ref().expect("box chain")
        };
        let cache_bits = *cache_bits_best;
        let traced = backward_references_trace_backwards(
            width,
            height,
            argb,
            cache_bits,
            hash_chain_tmp,
            &refs_best,
        );
        let bit_cost_trace = refs_bit_cost(&traced, cache_bits);
        if bit_cost_trace < bit_cost_best {
            refs_best = traced;
        }
    }
    backward_references_2d_locality(width as i32, &mut refs_best);
    refs_best
}

/// Port of `VP8LGetBackwardReferences` with `low_effort == 0`, `cache_bits_max == 0` and
/// `lz77_types_to_try = kLZ77Standard | kLZ77RLE`, as `EncodeImageNoHuffman` calls it.
#[must_use]
pub fn get_backward_references(
    width: usize,
    height: usize,
    argb: &[u32],
    quality: i32,
    hash_chain: &HashChain,
) -> BackwardRefs {
    let mut cache_bits = 0;
    get_backward_references_with_cache(
        width,
        height,
        argb,
        quality,
        K_LZ77_STANDARD | K_LZ77_RLE,
        0,
        hash_chain,
        &mut cache_bits,
    )
}
