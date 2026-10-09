// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/DitherUtils.h, src/gpu/DitherUtils.cpp

//! `skgpu::DitherRangeForConfig` and `skgpu::MakeDitherLUT`: the dither amplitude for a
//! destination format, and the 8x8 ordered-dither lookup table.
//!
//! `SK_IGNORE_GPU_DITHER` is not defined by Skia's default build, so both functions are always
//! available.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::size::ISize;

/// The dither range for a destination color type: `1 / (2^bitdepth - 1)`, or `0` for types
/// that are not dithered (half and full float, and unknown).
// Port of: src/gpu/DitherUtils.cpp#L16-L62 (chrome/m156)
#[doc(alias = "DitherRangeForConfig")]
#[must_use]
pub fn dither_range_for_config(dst_color_type: ColorType) -> f32 {
    debug_assert_ne!(dst_color_type, ColorType::Unknown);
    // We use 1 / (2^bitdepth-1) as the range since each channel can hold 2^bitdepth values
    match dst_color_type {
        // 4 bit
        ColorType::ARGB4444 => 1.0 / 15.0,
        // 6 bit
        ColorType::RGB565 => 1.0 / 63.0,
        // 8 bit
        ColorType::Alpha8
        | ColorType::Gray8
        | ColorType::R8UNorm
        | ColorType::R8G8UNorm
        | ColorType::RGB888x
        | ColorType::RGBA8888
        | ColorType::SRGBA8888
        | ColorType::BGRA8888 => 1.0 / 255.0,
        // 10 bit
        ColorType::RGBA1010102
        | ColorType::BGRA1010102
        | ColorType::RGB101010x
        | ColorType::BGR101010x
        | ColorType::BGR101010xXR
        | ColorType::BGRA10101010XR
        | ColorType::RGBA10x6 => 1.0 / 1023.0,
        // 16 bit
        ColorType::A16UNorm
        | ColorType::R16UNorm
        | ColorType::R16G16UNorm
        | ColorType::R16G16B16A16UNorm => 1.0 / 32767.0,
        // Unknown, half and float: no dithering
        ColorType::Unknown
        | ColorType::A16Float
        | ColorType::R16Float
        | ColorType::R16G16Float
        | ColorType::RGBAF16
        | ColorType::RGBF16F16F16x
        | ColorType::RGBAF16Norm
        | ColorType::RGBAF32 => 0.0,
    }
}

/// The 8x8 ordered-dither table as an immutable `A8` bitmap (`MakeDitherLUT`).
// Port of: src/gpu/DitherUtils.cpp#L64-L95 (chrome/m156)
#[doc(alias = "MakeDitherLUT")]
#[must_use]
// The casts mirror the C++ arithmetic: `m` is below 64 (exact in f32), and the table value is in
// [0, 255] before the `(uint8_t)` truncation.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn make_dither_lut() -> Bitmap {
    const IMG_SIZE: u32 = 8; // if changed, also change value in sk_dither_shader
    let mut data = [0u8; (IMG_SIZE * IMG_SIZE) as usize];
    for x in 0..IMG_SIZE {
        for y in 0..IMG_SIZE {
            // The computation of 'm' and 'value' is lifted from CPU backend.
            let m = (y & 1) << 5
                | (x & 1) << 4
                | (y & 2) << 2
                | (x & 2) << 1
                | (y & 4) >> 1
                | (x & 4) >> 2;
            let value = m as f32 * (1.0f32 / 64.0f32) - (63.0f32 / 128.0f32);
            // Bias by 0.5 to be in 0..1, mul by 255 and round to nearest int to make byte.
            data[(y * 8 + x) as usize] = ((value + 0.5f32) * 255.0f32 + 0.5f32) as u8;
        }
    }

    let mut bmp = Bitmap::new();
    let info = ImageInfo::new(ISize::new(8, 8), ColorType::Alpha8, AlphaType::Premul, None);
    let installed = bmp.install_pixels(&info, data.to_vec(), IMG_SIZE as usize);
    debug_assert!(installed);
    bmp.set_immutable();
    bmp
}
