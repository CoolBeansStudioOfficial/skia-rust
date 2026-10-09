// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/encode/SkImageEncoderFns.h (chrome/m156). The scanline transforms the encoders use.

/// Port of `transform_scanline_A8_to_GrayAlpha` (SkImageEncoderFns.h): each alpha byte becomes a
/// gray-alpha pixel whose gray value is zero. `dst` holds `2 * width` bytes.
// Port of: src/encode/SkImageEncoderFns.h#L20-L26 (chrome/m156)
#[doc(alias = "transform_scanline_A8_to_GrayAlpha")]
pub(crate) fn transform_scanline_a8_to_gray_alpha(dst: &mut [u8], src: &[u8], width: usize) {
    for i in 0..width {
        dst[2 * i] = 0;
        dst[2 * i + 1] = src[i];
    }
}
