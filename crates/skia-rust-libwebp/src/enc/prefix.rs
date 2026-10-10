// Copyright 2014 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the prefix coding of `src/dsp/lossless_common.h`: the code (and extra bits) that
//! represents a length or distance in the VP8L alphabet.

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
        (i32::from(code), i32::from(extra_bits), i32::from(extra_bits_value))
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
