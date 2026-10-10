// Copyright 2014 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the prefix coding of `src/dsp/lossless_common.h`: the code (and extra bits) that
//! represents a length or distance in the VP8L alphabet.

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

use super::tables::{K_PREFIX_ENCODE_CODE, K_PREFIX_ENCODE_EXTRA_BITS_VALUE};

/// Port of `PREFIX_LOOKUP_IDX_MAX`.
const PREFIX_LOOKUP_IDX_MAX: i32 = 512;

/// Port of `BitsLog2Floor` for the prefix computations (`n` is never 0 there).
fn bits_log2_floor(n: u32) -> i32 {
    super::entropy::bits_log2_floor(n)
}

/// Port of `VP8LPrefixEncodeBitsNoLUT`, returning `(code, extra_bits)`.
fn prefix_encode_bits_no_lut(distance: i32) -> (i32, i32) {
    let distance = distance - 1;
    let highest_bit = bits_log2_floor(distance as u32);
    let second_highest_bit = (distance >> (highest_bit - 1)) & 1;
    let extra_bits = highest_bit - 1;
    let code = 2 * highest_bit + second_highest_bit;
    (code, extra_bits)
}

/// Port of `VP8LPrefixEncodeBits`: `(code, extra_bits)` for `distance`.
#[must_use]
pub fn prefix_encode_bits(distance: i32) -> (i32, i32) {
    if distance < PREFIX_LOOKUP_IDX_MAX {
        let (code, extra_bits) = K_PREFIX_ENCODE_CODE[distance as usize];
        (i32::from(code), i32::from(extra_bits))
    } else {
        prefix_encode_bits_no_lut(distance)
    }
}

/// Port of `VP8LPrefixEncode`: `(code, extra_bits, extra_bits_value)` for `distance`.
#[must_use]
pub fn prefix_encode(distance: i32) -> (i32, i32, i32) {
    if distance < PREFIX_LOOKUP_IDX_MAX {
        let (code, extra_bits) = K_PREFIX_ENCODE_CODE[distance as usize];
        let extra_bits_value = K_PREFIX_ENCODE_EXTRA_BITS_VALUE[distance as usize];
        (
            i32::from(code),
            i32::from(extra_bits),
            i32::from(extra_bits_value),
        )
    } else {
        let d = distance - 1;
        let highest_bit = bits_log2_floor(d as u32);
        let second_highest_bit = (d >> (highest_bit - 1)) & 1;
        let extra_bits = highest_bit - 1;
        let extra_bits_value = d & ((1 << extra_bits) - 1);
        let code = 2 * highest_bit + second_highest_bit;
        (code, extra_bits, extra_bits_value)
    }
}
