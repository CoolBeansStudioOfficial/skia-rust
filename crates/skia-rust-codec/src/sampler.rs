// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkSampler.cpp#L20-L70 (chrome/m156), the Fill half of src/codec/SkSampler.h
// Ported from: src/codec/SkSampler.cpp, src/codec/SkSampler.h

//! Samplers write rows of the destination; this module holds the part the codec core uses: filling
//! rows that a truncated or failed decode never produced.

use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;

use crate::codec::ZeroInitialized;

/// Port of `SkSampler::Fill`: writes zeros over the rows of `info` at `dst` (`rowBytes` apart).
///
/// Skia zeroes only the colour types a decoder can produce incomplete images in (N32, 565,
/// Gray8 and RGBA F16); other colour types are left alone, as Skia's default case does.
#[doc(alias = "SkSampler::Fill")]
// Image dimensions are non-negative, so the casts to usize are exact.
#[allow(clippy::cast_sign_loss)]
pub fn fill(info: &ImageInfo, dst: &mut [u8], row_bytes: usize, zero_init: ZeroInitialized) {
    // Port of: src/codec/SkSampler.cpp#L24-L26
    if zero_init == ZeroInitialized::Yes {
        return;
    }
    let bytes_per_pixel = match info.color_type() {
        ColorType::RGBA8888 | ColorType::BGRA8888 => 4,
        ColorType::RGB565 => 2,
        ColorType::Gray8 => 1,
        ColorType::RGBAF16Norm => 8,
        _ => return,
    };
    let width = info.width() as usize;
    let row_len = width * bytes_per_pixel;
    for row in 0..info.height() as usize {
        let start = row * row_bytes;
        dst[start..start + row_len].fill(0);
    }
}
