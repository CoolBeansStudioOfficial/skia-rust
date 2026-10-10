// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/encode (chrome/m156), the encoders the crate ships: PNG, JPEG and WebP (lossless
// and lossy, with the ICC and animated paths; see `webp_encoder`).

pub mod icc;
pub mod image_encoder_fns;
pub mod jpeg_encoder;
pub mod png_encoder;
pub(crate) mod png_encoder_base;
pub(crate) mod png_encoder_impl;
pub mod webp_encoder;
