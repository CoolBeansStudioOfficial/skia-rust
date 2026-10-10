// Copyright 2014 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of libwebp `src/utils/palette.c` (the colour-table collection, the sorted palette and
//! the palette index lookup) and the colour-indexing transform of `src/enc/vp8l_enc.c`
//! (`ApplyPalette`, `VP8LBundleColorMap`).
//!
//! The palette orderings other than `kSortedDefault` (minimise deltas, the modified Zeng
//! ordering) are used only by the `method > 0` searches and are not ported here.

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

/// Port of `MAX_PALETTE_SIZE`.
pub const MAX_PALETTE_SIZE: usize = 256;
/// Port of `COLOR_HASH_SIZE` (`MAX_PALETTE_SIZE * 4`).
const COLOR_HASH_SIZE: usize = MAX_PALETTE_SIZE * 4;
/// Port of `COLOR_HASH_RIGHT_SHIFT` (`32 - log2(COLOR_HASH_SIZE)`).
const COLOR_HASH_RIGHT_SHIFT: u32 = 22;
/// Port of `kHashMul` (`color_cache_utils.c`).
const K_HASH_MUL: u32 = 0x1e35_a7bd;

/// Port of `VP8LHashPix` (`src/utils/color_cache_utils.h`).
#[must_use]
pub fn hash_pix(argb: u32, shift: u32) -> usize {
    (argb.wrapping_mul(K_HASH_MUL) >> shift) as usize
}

/// Port of `GetColorPalette`: the sorted distinct colours of the image, or `None` when there are
/// more than `MAX_PALETTE_SIZE` of them (the C code returns `MAX_PALETTE_SIZE + 1`).
#[must_use]
pub fn get_color_palette(
    argb: &[u32],
    width: usize,
    height: usize,
    stride: usize,
) -> Option<Vec<u32>> {
    let mut in_use = vec![false; COLOR_HASH_SIZE];
    let mut colors = vec![0u32; COLOR_HASH_SIZE];
    let mut num_colors = 0usize;
    let mut last_pix = !argb[0]; // so we're sure that last_pix != argb[0]
    for y in 0..height {
        let row = &argb[y * stride..];
        for &pix in &row[..width] {
            if pix == last_pix {
                continue;
            }
            last_pix = pix;
            let mut key = hash_pix(last_pix, COLOR_HASH_RIGHT_SHIFT);
            loop {
                if !in_use[key] {
                    colors[key] = last_pix;
                    in_use[key] = true;
                    num_colors += 1;
                    if num_colors > MAX_PALETTE_SIZE {
                        return None; // Exact count not needed.
                    }
                    break;
                } else if colors[key] == last_pix {
                    break; // The color is already there.
                } else {
                    key = (key + 1) & (COLOR_HASH_SIZE - 1);
                }
            }
        }
    }
    let mut palette: Vec<u32> = (0..COLOR_HASH_SIZE)
        .filter(|&i| in_use[i])
        .map(|i| colors[i])
        .collect();
    palette.sort_unstable();
    Some(palette)
}

/// Port of `SearchColorNoIdx`: the index of `color` in the sorted `sorted[..num_colors]`.
#[must_use]
pub fn search_color_no_idx(sorted: &[u32], color: u32, num_colors: usize) -> usize {
    let mut low = 0usize;
    let mut hi = num_colors;
    if sorted[low] == color {
        return low; // loop invariant: sorted[low] != color
    }
    loop {
        let mid = (low + hi) >> 1;
        match sorted[mid].cmp(&color) {
            std::cmp::Ordering::Equal => return mid,
            std::cmp::Ordering::Less => low = mid,
            std::cmp::Ordering::Greater => hi = mid,
        }
    }
}

/// Port of `PrepareMapToPalette`: the sorted palette and, for each palette index, its index in
/// the sorted order.
#[must_use]
pub fn prepare_map_to_palette(palette: &[u32], num_colors: usize) -> (Vec<u32>, Vec<u32>) {
    let mut sorted = palette[..num_colors].to_vec();
    sorted.sort_unstable();
    let mut idx_map = vec![0u32; num_colors];
    for i in 0..num_colors {
        idx_map[search_color_no_idx(&sorted, palette[i], num_colors)] = i as u32;
    }
    (sorted, idx_map)
}

/// Port of `VP8LBundleColorMap_C`: packs one row of palette indices into `dst` (`width >> xbits`
/// pixels), `1 << xbits` indices per pixel.
fn bundle_color_map(row: &[u8], width: usize, xbits: u32, dst: &mut [u32]) {
    if xbits > 0 {
        let bit_depth = 1u32 << (3 - xbits);
        let mask = (1usize << xbits) - 1;
        let mut code: u32 = 0xff00_0000;
        for x in 0..width {
            let xsub = x & mask;
            if xsub == 0 {
                code = 0xff00_0000;
            }
            code |= u32::from(row[x]) << (8 + bit_depth * xsub as u32);
            dst[x >> xbits] = code;
        }
    } else {
        for x in 0..width {
            dst[x] = 0xff00_0000 | (u32::from(row[x]) << 8);
        }
    }
}

/// Port of `APPLY_PALETTE_FOR`: maps each pixel to its index with `index_of`, using the previous
/// pixel's index when the colour repeats, then bundles each row into `dst`.
fn apply_palette_for(
    src: &[u32],
    src_stride: usize,
    dst: &mut [u32],
    dst_stride: usize,
    palette: &[u32],
    width: usize,
    height: usize,
    xbits: u32,
    index_of: &dyn Fn(u32) -> u32,
) {
    let mut tmp_row = vec![0u8; width];
    let mut prev_pix = palette[0];
    let mut prev_idx: u32 = 0;
    for y in 0..height {
        for x in 0..width {
            let pix = src[y * src_stride + x];
            if pix != prev_pix {
                prev_idx = index_of(pix);
                prev_pix = pix;
            }
            tmp_row[x] = prev_idx as u8;
        }
        bundle_color_map(&tmp_row, width, xbits, &mut dst[y * dst_stride..]);
    }
}

fn hash0(color: u32) -> u32 {
    (color >> 8) & 0xff
}

fn hash1(color: u32) -> u32 {
    ((color & 0x00ff_ffff).wrapping_mul(4_222_244_071u64 as u32)) >> (32 - 11)
}

fn hash2(color: u32) -> u32 {
    ((u64::from(color & 0x00ff_ffff) * ((1u64 << 31) - 1)) as u32) >> (32 - 11)
}

/// Port of `ApplyPalette`: the palette index image of `src` (`width` x `height`, stride
/// `src_stride`), packed `1 << xbits` indices per pixel into a `(width >> xbits)`-wide image.
#[must_use]
#[allow(clippy::too_many_arguments)] // mirrors the C signature
pub fn apply_palette(
    src: &[u32],
    src_stride: usize,
    palette: &[u32],
    width: usize,
    height: usize,
    xbits: u32,
) -> Vec<u32> {
    let palette_size = palette.len();
    let dst_width = (width + (1 << xbits) - 1) >> xbits;
    let mut dst = vec![0u32; dst_width * height];
    let dst_stride = dst_width;
    if palette_size < 4 {
        // APPLY_PALETTE_GREEDY_MAX: SearchColorGreedy.
        let index_of = |color: u32| -> u32 {
            if color == palette[0] {
                0
            } else if color == palette[1] {
                1
            } else if color == palette[2] {
                2
            } else {
                3
            }
        };
        apply_palette_for(
            src, src_stride, &mut dst, dst_stride, palette, width, height, xbits, &index_of,
        );
        return dst;
    }
    // Find a hash that maps the palette without collisions, for a direct lookup table.
    let hashes: [fn(u32) -> u32; 3] = [hash0, hash1, hash2];
    let mut chosen: Option<usize> = None;
    let mut buffer = vec![0xffffu16; 1 << 11];
    for (i, hash) in hashes.iter().enumerate() {
        let mut use_lut = true;
        buffer.iter_mut().for_each(|b| *b = 0xffff);
        for (j, &color) in palette.iter().enumerate() {
            let ind = hash(color) as usize;
            if buffer[ind] != 0xffff {
                use_lut = false;
                break;
            }
            buffer[ind] = j as u16;
        }
        if use_lut {
            chosen = Some(i);
            break;
        }
    }
    if let Some(i) = chosen {
        {
            let hash = hashes[i];
            let lut = buffer.clone();
            let index_of = move |color: u32| -> u32 { u32::from(lut[hash(color) as usize]) };
            apply_palette_for(
                src, src_stride, &mut dst, dst_stride, palette, width, height, xbits, &index_of,
            );
        }
    } else {
        {
            let (palette_sorted, idx_map) = prepare_map_to_palette(palette, palette_size);
            let index_of = move |color: u32| -> u32 {
                idx_map[search_color_no_idx(&palette_sorted, color, palette_size)]
            };
            apply_palette_for(
                src, src_stride, &mut dst, dst_stride, palette, width, height, xbits, &index_of,
            );
        }
    }
    dst
}
