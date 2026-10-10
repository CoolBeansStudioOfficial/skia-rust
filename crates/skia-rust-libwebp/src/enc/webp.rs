// Copyright 2010 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the lossless path of libwebp `src/enc/webp_enc.c` (`WebPEncode` with
//! `config->lossless == 1`), `VP8LEncodeImage` and the RIFF container of `WriteImage`.
//!
//! `SkWebpEncoder` sets `lossless`, `method = 0`, `quality` from its options, `exact = 0` and
//! `image_hint = WEBP_HINT_DEFAULT`, with no progress hook and one thread.

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

use super::bit_writer::BitWriter;
use super::vp8l_encode::{EncodeConfig, encode_stream};

/// Port of `WEBP_MAX_DIMENSION`.
pub const WEBP_MAX_DIMENSION: usize = 16383;
/// Port of `VP8L_MAGIC_BYTE`.
const VP8L_MAGIC_BYTE: u8 = 0x2f;
/// Port of `VP8L_IMAGE_SIZE_BITS`.
const VP8L_IMAGE_SIZE_BITS: u32 = 14;
/// Port of `VP8L_VERSION_BITS`.
const VP8L_VERSION_BITS: u32 = 3;
/// Port of `VP8L_VERSION`.
const VP8L_VERSION: u32 = 0;

/// Encodes `argb` (`width` x `height`, row-major, `0xAARRGGBB` per pixel) as a lossless WebP
/// file, as `WebPEncode` does with `config->lossless = 1`, `config->method = 0`,
/// `config->quality = quality` and `config->exact = exact`.
///
/// Returns `None` where libwebp reports an error (dimensions out of range, or a size mismatch).
#[must_use]
pub fn encode_lossless(
    width: usize,
    height: usize,
    argb: &[u32],
    quality: i32,
    exact: bool,
) -> Option<Vec<u8>> {
    encode_lossless_method(width, height, argb, 0, quality, exact)
}

/// Encodes like [`encode_lossless`] at `config->method = method` (0 to 4): the VP8L stream that
/// the alpha plane of a lossy picture uses at `method` 3 is `method` 3 with `exact` set.
///
/// Returns `None` for `method` above 4 and wherever [`encode_lossless`] does.
#[must_use]
pub fn encode_lossless_method(
    width: usize,
    height: usize,
    argb: &[u32],
    method: u32,
    quality: i32,
    exact: bool,
) -> Option<Vec<u8>> {
    if method > 4 {
        return None;
    }
    if width == 0 || height == 0 || width > WEBP_MAX_DIMENSION || height > WEBP_MAX_DIMENSION {
        return None;
    }
    if argb.len() != width * height || !(0..=100).contains(&quality) {
        return None;
    }
    let mut pixels = argb.to_vec();
    if !exact {
        // WebPReplaceTransparentPixels(pic, 0x000000): fully transparent pixels become 0.
        for px in &mut pixels {
            if (*px >> 24) == 0 {
                *px = 0;
            }
        }
    }
    // VP8LEncodeImage: the header, then the stream.
    let has_alpha = pixels.iter().any(|&px| (px >> 24) != 0xff);
    let mut bw = BitWriter::new();
    bw.put_bits((width - 1) as u32, VP8L_IMAGE_SIZE_BITS);
    bw.put_bits((height - 1) as u32, VP8L_IMAGE_SIZE_BITS);
    bw.put_bits(u32::from(has_alpha), 1);
    bw.put_bits(VP8L_VERSION, VP8L_VERSION_BITS);
    let config = EncodeConfig {
        method,
        quality,
        exact,
    };
    if !encode_stream(width, height, &pixels, &config, &mut bw) {
        return None;
    }
    let webpll_data = bw.finish();
    // WriteImage: RIFF header, VP8L chunk, padding to an even size.
    let vp8l_size = 1 + webpll_data.len();
    let pad = vp8l_size & 1;
    let riff_size = 4 + 8 + vp8l_size + pad;
    let mut out = Vec::with_capacity(12 + 8 + vp8l_size + pad);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(riff_size as u32).to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(b"VP8L");
    out.extend_from_slice(&(vp8l_size as u32).to_le_bytes());
    out.push(VP8L_MAGIC_BYTE);
    out.extend_from_slice(&webpll_data);
    if pad != 0 {
        out.push(0);
    }
    Some(out)
}
