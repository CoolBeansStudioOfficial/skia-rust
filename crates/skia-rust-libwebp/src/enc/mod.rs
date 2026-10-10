// Copyright 2014 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the libwebp 1.4.0 encoder that `SkWebpEncoder` uses: the VP8L (lossless) encoder and
//! its helpers. See `docs/design/codecs.md` §7 for the scope.

pub mod backward_refs;
pub mod alpha_enc;
pub mod backward_refs_cost;
pub mod backward_refs_select;
pub mod bit_writer;
pub mod color_cache;
pub mod color_transform;
pub mod cost_tables;
pub mod entropy;
pub mod histogram;
pub mod huffman;
pub mod palette;
pub mod palette_sort;
pub mod picture;
pub mod picture_cleanup;
pub mod predictor;
pub mod prefix;
pub mod tables;
pub mod tables_quant;
pub mod tree_tables;
pub mod vp8_analysis;
pub mod vp8_bit_writer;
pub mod vp8_cost;
pub mod vp8_enc_dsp;
pub mod vp8_encoder;
pub mod vp8_filter;
pub mod vp8_frame;
pub mod vp8_lossy;
pub mod vp8_mode;
pub mod vp8_quant;
pub mod vp8_syntax;
pub mod vp8_token;
pub mod vp8_tree;
pub mod vp8l;
pub mod vp8l_encode;
pub mod webp;

pub use webp::{encode_lossless, encode_lossless_method};
