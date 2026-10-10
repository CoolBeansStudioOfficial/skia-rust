// Copyright 2014 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of the libwebp 1.4.0 encoder that `SkWebpEncoder` uses: the VP8L (lossless) encoder and
//! its helpers. See `docs/design/codecs.md` §7 for the scope.

pub mod backward_refs;
pub mod bit_writer;
pub mod entropy;
pub mod histogram;
pub mod huffman;
pub mod prefix;
pub mod tables;
