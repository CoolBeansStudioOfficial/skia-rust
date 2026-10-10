// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the parse selection of libwebp `src/enc/backward_references_enc.c`
//! (`BackwardReferencesRle`, `GetBackwardReferences` and `VP8LGetBackwardReferences` with
//! `low_effort == 0`), for `lz77_types_to_try = kLZ77Standard | kLZ77RLE`, no colour cache.

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
    BackwardRefs, HashChain, MIN_LENGTH, PixOrCopy, backward_references_2d_locality,
    backward_references_lz77, find_match_length, max_find_copy_length,
};
use super::backward_refs_cost::backward_references_trace_backwards;
use super::histogram::{Histogram, histogram_estimate_bits};

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

/// The `kLZ77*` types that `GetBackwardReferences` tries, in the order of its bit loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lz77Type {
    Standard,
    Rle,
}

/// Port of `GetBackwardReferences` (reached through `VP8LGetBackwardReferences` with
/// `low_effort == 0`) for `lz77_types_to_try = kLZ77Standard | kLZ77RLE`, `cache_bits_max = 0`
/// and `do_no_cache = 0`.
///
/// The standard and RLE parses are compared by estimated bit cost. At `quality >= 25` a standard
/// parse is then replaced by its cost-model trace when the trace is cheaper. The result carries
/// the 2D locality codes.
#[must_use]
pub fn get_backward_references(
    width: usize,
    height: usize,
    argb: &[u32],
    quality: i32,
    hash_chain: &HashChain,
) -> BackwardRefs {
    let mut best = BackwardRefs::default();
    let mut best_cost = f32::MAX;
    let mut best_type: Option<Lz77Type> = None;
    for lz77_type in [Lz77Type::Standard, Lz77Type::Rle] {
        let refs_tmp = match lz77_type {
            Lz77Type::Rle => backward_references_rle(width, height, argb),
            Lz77Type::Standard => backward_references_lz77(width, height, argb, hash_chain),
        };
        let mut histo = Histogram::new(0);
        histo.create(&refs_tmp, 0);
        let bit_cost = histogram_estimate_bits(&mut histo);
        if bit_cost < best_cost {
            best = refs_tmp;
            best_cost = bit_cost;
            best_type = Some(lz77_type);
        }
    }
    if best_type == Some(Lz77Type::Standard) && quality >= 25 {
        let traced = backward_references_trace_backwards(width, height, argb, hash_chain, &best);
        let mut histo = Histogram::new(0);
        histo.create(&traced, 0);
        let bit_cost_trace = histogram_estimate_bits(&mut histo);
        if bit_cost_trace < best_cost {
            best = traced;
        }
    }
    backward_references_2d_locality(width as i32, &mut best);
    best
}
