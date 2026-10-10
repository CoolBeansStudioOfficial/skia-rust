// Copyright 2013 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the VP8L bitstream writer of libwebp `src/enc/vp8l_enc.c` for `method 0`
//! (`low_effort`, the lossless setting of `SkWebpEncoder`), with `exact == 0`, no near-lossless
//! step, one crunch configuration and no colour cache.
//!
//! Functions keep the C names. Where the C code writes into a bit writer that it may later swap
//! with a best-so-far copy, this port writes directly, which gives the same bytes: with a single
//! candidate the best-so-far copy is the stream itself.

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

use super::backward_refs::{HashChain, PixOrCopy, PixOrCopyMode};
use super::backward_refs_select::get_backward_references;
use super::bit_writer::BitWriter;
use super::entropy::bits_log2_floor;
use super::histogram::{HistogramSet, NUM_DISTANCE_CODES, histogram_num_codes};
use super::huffman::{
    HuffmanTreeCode, HuffmanTreeToken, create_compressed_huffman_tree, create_huffman_tree,
};
use super::prefix::prefix_encode;

/// Port of `MAX_HUFF_IMAGE_SIZE`.
const MAX_HUFF_IMAGE_SIZE: usize = 2600;
/// Port of `MIN_HUFFMAN_BITS`.
const MIN_HUFFMAN_BITS: u32 = 2;
/// Port of `MAX_HUFFMAN_BITS`.
const MAX_HUFFMAN_BITS: u32 = 9;
/// Port of `CODE_LENGTH_CODES`.
const CODE_LENGTH_CODES: usize = 19;
/// Port of `NUM_LITERAL_CODES`.
const NUM_LITERAL_CODES: usize = 256;

/// Port of `VP8LSubSampleSize`.
#[must_use]
pub fn sub_sample_size(size: usize, sampling_bits: u32) -> usize {
    (size + (1usize << sampling_bits) - 1) >> sampling_bits
}

/// Port of `GetHistoBits`.
pub(super) fn get_histo_bits(method: u32, use_palette: bool, width: usize, height: usize) -> u32 {
    let mut histo_bits: i32 = (if use_palette { 9 } else { 7 }) - method as i32;
    loop {
        let huff_image_size =
            sub_sample_size(width, histo_bits as u32) * sub_sample_size(height, histo_bits as u32);
        if huff_image_size <= MAX_HUFF_IMAGE_SIZE {
            break;
        }
        histo_bits += 1;
    }
    histo_bits.clamp(MIN_HUFFMAN_BITS as i32, MAX_HUFFMAN_BITS as i32) as u32
}

/// Port of `GetTransformBits`.
pub(super) fn get_transform_bits(method: u32, histo_bits: u32) -> u32 {
    let max_transform_bits = match method {
        0..=3 => 6,
        4 => 5,
        _ => 4,
    };
    histo_bits.min(max_transform_bits)
}

/// Port of `GetHuffBitLengthsAndCodes`: the code lengths of the five alphabets of each histogram
/// (literal, red, blue, alpha, distance), in `5 * i + k` order.
pub(super) fn get_huff_bit_lengths_and_codes(
    histogram_image: &mut HistogramSet,
) -> Vec<HuffmanTreeCode> {
    let mut codes = Vec::with_capacity(5 * histogram_image.size);
    for i in 0..histogram_image.size {
        let histo = histogram_image.histograms[i]
            .as_mut()
            .expect("histogram in use");
        let literal_codes = histogram_num_codes(histo.palette_code_bits);
        let mut group = [
            HuffmanTreeCode::new(literal_codes),
            HuffmanTreeCode::new(NUM_LITERAL_CODES),
            HuffmanTreeCode::new(NUM_LITERAL_CODES),
            HuffmanTreeCode::new(NUM_LITERAL_CODES),
            HuffmanTreeCode::new(NUM_DISTANCE_CODES),
        ];
        create_huffman_tree(&mut histo.literal, 15, &mut group[0]);
        create_huffman_tree(&mut histo.red, 15, &mut group[1]);
        create_huffman_tree(&mut histo.blue, 15, &mut group[2]);
        create_huffman_tree(&mut histo.alpha, 15, &mut group[3]);
        create_huffman_tree(&mut histo.distance, 15, &mut group[4]);
        codes.extend(group);
    }
    codes
}

/// Port of `StoreHuffmanTreeOfHuffmanTreeToBitMask`.
pub(super) fn store_huffman_tree_of_huffman_tree_to_bit_mask(
    bw: &mut BitWriter,
    code_length_bitdepth: &[u8; CODE_LENGTH_CODES],
) {
    const K_STORAGE_ORDER: [usize; CODE_LENGTH_CODES] = [
        17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
    ];
    let mut codes_to_store = CODE_LENGTH_CODES;
    while codes_to_store > 4 {
        if code_length_bitdepth[K_STORAGE_ORDER[codes_to_store - 1]] != 0 {
            break;
        }
        codes_to_store -= 1;
    }
    bw.put_bits((codes_to_store - 4) as u32, 4);
    for &order in K_STORAGE_ORDER.iter().take(codes_to_store) {
        bw.put_bits(u32::from(code_length_bitdepth[order]), 3);
    }
}

/// Port of `ClearHuffmanTreeIfOnlyOneSymbol`.
pub(super) fn clear_huffman_tree_if_only_one_symbol(huffman_code: &mut HuffmanTreeCode) {
    let mut count = 0;
    for &len in &huffman_code.code_lengths {
        if len != 0 {
            count += 1;
            if count > 1 {
                return;
            }
        }
    }
    huffman_code.code_lengths.iter_mut().for_each(|l| *l = 0);
    huffman_code.codes.iter_mut().for_each(|c| *c = 0);
}

/// Port of `StoreHuffmanTreeToBitMask`.
pub(super) fn store_huffman_tree_to_bit_mask(
    bw: &mut BitWriter,
    tokens: &[HuffmanTreeToken],
    huffman_code: &HuffmanTreeCode,
) {
    for token in tokens {
        let ix = token.code as usize;
        let extra_bits = u32::from(token.extra_bits);
        bw.put_bits(
            u32::from(huffman_code.codes[ix]),
            u32::from(huffman_code.code_lengths[ix]),
        );
        match ix {
            16 => bw.put_bits(extra_bits, 2),
            17 => bw.put_bits(extra_bits, 3),
            18 => bw.put_bits(extra_bits, 7),
            _ => {}
        }
    }
}

/// Port of `StoreFullHuffmanCode`.
pub(super) fn store_full_huffman_code(bw: &mut BitWriter, tree: &HuffmanTreeCode) {
    let mut code_length_bitdepth = HuffmanTreeCode::new(CODE_LENGTH_CODES);
    bw.put_bits(0, 1);
    let tokens = create_compressed_huffman_tree(tree);
    {
        let mut histogram = [0u32; CODE_LENGTH_CODES];
        for t in &tokens {
            histogram[t.code as usize] += 1;
        }
        create_huffman_tree(&mut histogram, 7, &mut code_length_bitdepth);
    }
    let mut bitdepth_array = [0u8; CODE_LENGTH_CODES];
    bitdepth_array.copy_from_slice(&code_length_bitdepth.code_lengths);
    store_huffman_tree_of_huffman_tree_to_bit_mask(bw, &bitdepth_array);
    clear_huffman_tree_if_only_one_symbol(&mut code_length_bitdepth);
    // The trimmed length uses the code lengths after the single-symbol clear, as in C.
    let depths = code_length_bitdepth.code_lengths.clone();
    let mut trimmed_length = tokens.len();
    let mut trailing_zero_bits: u32 = 0;
    let mut i = tokens.len();
    while i > 0 {
        i -= 1;
        let ix = tokens[i].code as usize;
        if ix == 0 || ix == 17 || ix == 18 {
            trimmed_length -= 1; // discount trailing zeros
            trailing_zero_bits += u32::from(depths[ix]);
            if ix == 17 {
                trailing_zero_bits += 3;
            } else if ix == 18 {
                trailing_zero_bits += 7;
            }
        } else {
            break;
        }
    }
    let write_trimmed_length = trimmed_length > 1 && trailing_zero_bits > 12;
    let length = if write_trimmed_length {
        trimmed_length
    } else {
        tokens.len()
    };
    bw.put_bits(u32::from(write_trimmed_length), 1);
    if write_trimmed_length {
        if trimmed_length == 2 {
            bw.put_bits(0, 3 + 2); // nbitpairs=1, trimmed_length=2
        } else {
            let nbits = bits_log2_floor((trimmed_length - 2) as u32) as u32;
            let nbitpairs = nbits / 2 + 1;
            bw.put_bits(nbitpairs - 1, 3);
            bw.put_bits((trimmed_length - 2) as u32, nbitpairs * 2);
        }
    }
    store_huffman_tree_to_bit_mask(bw, &tokens[..length], &code_length_bitdepth);
}

/// Port of `StoreHuffmanCode`.
pub(super) fn store_huffman_code(bw: &mut BitWriter, huffman_code: &HuffmanTreeCode) {
    let mut count = 0;
    let mut symbols = [0usize; 2];
    const K_MAX_BITS: u32 = 8;
    const K_MAX_SYMBOL: usize = 1 << K_MAX_BITS;
    for i in 0..huffman_code.num_symbols() {
        if count >= 3 {
            break;
        }
        if huffman_code.code_lengths[i] != 0 {
            if count < 2 {
                symbols[count] = i;
            }
            count += 1;
        }
    }
    if count == 0 {
        // emit minimal tree for empty cases
        bw.put_bits(0x01, 4);
    } else if count <= 2 && symbols[0] < K_MAX_SYMBOL && symbols[1] < K_MAX_SYMBOL {
        bw.put_bits(1, 1); // Small tree marker to encode 1 or 2 symbols.
        bw.put_bits((count - 1) as u32, 1);
        if symbols[0] <= 1 {
            bw.put_bits(0, 1); // Code bit for small (1 bit) symbol value.
            bw.put_bits(symbols[0] as u32, 1);
        } else {
            bw.put_bits(1, 1);
            bw.put_bits(symbols[0] as u32, 8);
        }
        if count == 2 {
            bw.put_bits(symbols[1] as u32, 8);
        }
    } else {
        store_full_huffman_code(bw, huffman_code);
    }
}

/// Port of `StoreImageToBitMask`: the entropy-coded symbols of the image `refs`.
pub(super) fn store_image_to_bit_mask(
    bw: &mut BitWriter,
    width: usize,
    histo_bits: u32,
    refs: &[PixOrCopy],
    histogram_symbols: &[u16],
    huffman_codes: &[HuffmanTreeCode],
) {
    let histo_xsize = if histo_bits != 0 {
        sub_sample_size(width, histo_bits)
    } else {
        1
    };
    let tile_mask: usize = if histo_bits == 0 {
        0
    } else {
        !((1usize << histo_bits) - 1)
    };
    let mut x: usize = 0;
    let mut y: usize = 0;
    let mut tile_x = x & tile_mask;
    let mut tile_y = y & tile_mask;
    let mut histogram_ix = histogram_symbols[0] as usize;
    let mut codes = 5 * histogram_ix;
    for v in refs {
        if tile_x != (x & tile_mask) || tile_y != (y & tile_mask) {
            tile_x = x & tile_mask;
            tile_y = y & tile_mask;
            histogram_ix =
                histogram_symbols[(y >> histo_bits) * histo_xsize + (x >> histo_bits)] as usize;
            codes = 5 * histogram_ix;
        }
        if v.is_literal() {
            const ORDER: [u32; 4] = [1, 2, 0, 3];
            for (k, &comp) in ORDER.iter().enumerate() {
                let code = v.literal_component(comp) as usize;
                write_huffman_code(bw, &huffman_codes[codes + k], code);
            }
        } else if v.mode == PixOrCopyMode::CacheIdx {
            // The colour cache is not used on this path.
            let code = v.argb_or_distance as usize;
            let literal_ix = 256 + 24 + code;
            write_huffman_code(bw, &huffman_codes[codes], literal_ix);
        } else {
            let distance = v.argb_or_distance as i32;
            let (code, n_bits, bits) = prefix_encode(v.length() as i32);
            write_huffman_code_with_extra_bits(
                bw,
                &huffman_codes[codes],
                256 + code as usize,
                bits as u32,
                n_bits as u32,
            );
            let (code, n_bits, bits) = prefix_encode(distance);
            write_huffman_code(bw, &huffman_codes[codes + 4], code as usize);
            bw.put_bits(bits as u32, n_bits as u32);
        }
        x += v.length() as usize;
        while x >= width {
            x -= width;
            y += 1;
        }
    }
}

/// Port of `WriteHuffmanCode`.
pub(super) fn write_huffman_code(bw: &mut BitWriter, code: &HuffmanTreeCode, code_index: usize) {
    let depth = u32::from(code.code_lengths[code_index]);
    let symbol = u32::from(code.codes[code_index]);
    bw.put_bits(symbol, depth);
}

/// Port of `WriteHuffmanCodeWithExtraBits`.
pub(super) fn write_huffman_code_with_extra_bits(
    bw: &mut BitWriter,
    code: &HuffmanTreeCode,
    code_index: usize,
    bits: u32,
    n_bits: u32,
) {
    let depth = u32::from(code.code_lengths[code_index]);
    let symbol = u32::from(code.codes[code_index]);
    bw.put_bits((bits << depth) | symbol, depth + n_bits);
}

/// Port of `EncodeImageNoHuffman`: a sub-image (palette, predictor modes, histogram map) coded
/// with its own Huffman codes and no colour cache.
pub(super) fn encode_image_no_huffman(
    bw: &mut BitWriter,
    argb: &[u32],
    hash_chain: &mut HashChain,
    width: usize,
    height: usize,
    quality: i32,
    low_effort: bool,
) {
    super::backward_refs::hash_chain_fill(hash_chain, quality, argb, width, height, low_effort);
    let refs = get_backward_references(width, height, argb, quality, hash_chain);
    let mut histogram_image = HistogramSet::new(1, 0);
    histogram_image.clear();
    histogram_image.histograms[0]
        .as_mut()
        .expect("slot")
        .store_refs(&refs);
    let mut huffman_codes = get_huff_bit_lengths_and_codes(&mut histogram_image);
    bw.put_bits(0, 1);
    let histogram_symbols = [0u16];
    for code in huffman_codes.iter_mut().take(5) {
        store_huffman_code(bw, code);
        clear_huffman_tree_if_only_one_symbol(code);
    }
    store_image_to_bit_mask(bw, width, 0, &refs.refs, &histogram_symbols, &huffman_codes);
}
