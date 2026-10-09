// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/ToolUtils.cpp

//! Helpers from Skia's `tools/ToolUtils` that the benches share.

use skia_rust_core::color_type::ColorType;

/// `ToolUtils::colortype_name(SkColorType)`: the name a bench puts in its result name. The
/// same function is in `tests/src/tools/tool_utils.rs`, which the bench crate cannot depend
/// on; keep the two in step.
// Port of: tools/ToolUtils.cpp#L62-L95 (chrome/m156)
#[must_use]
pub fn colortype_name(ct: ColorType) -> &'static str {
    match ct {
        ColorType::Unknown => "Unknown",
        ColorType::Alpha8 => "Alpha_8",
        ColorType::A16UNorm => "Alpha_16",
        ColorType::A16Float => "A16_float",
        ColorType::RGB565 => "RGB_565",
        ColorType::ARGB4444 => "ARGB_4444",
        ColorType::RGBA8888 => "RGBA_8888",
        ColorType::SRGBA8888 => "SRGBA_8888",
        ColorType::RGB888x => "RGB_888x",
        ColorType::BGRA8888 => "BGRA_8888",
        ColorType::RGBA1010102 => "RGBA_1010102",
        ColorType::BGRA1010102 => "BGRA_1010102",
        ColorType::RGB101010x => "RGB_101010x",
        ColorType::BGR101010x => "BGR_101010x",
        ColorType::BGR101010xXR => "BGR_101010x_XR",
        ColorType::RGBA10x6 => "RGBA_10x6",
        ColorType::Gray8 => "Gray_8",
        ColorType::RGBAF16Norm => "RGBA_F16Norm",
        ColorType::RGBF16F16F16x => "RGB_F16F16F16x",
        ColorType::RGBAF16 => "RGBA_F16",
        ColorType::RGBAF32 => "RGBA_F32",
        ColorType::R8G8UNorm => "R8G8_unorm",
        ColorType::R16UNorm => "R16_unorm",
        ColorType::R16Float => "R16_float",
        ColorType::R16G16UNorm => "R16G16_unorm",
        ColorType::R16G16Float => "R16G16_float",
        ColorType::R16G16B16A16UNorm => "R16G16B16A16_unorm",
        ColorType::R8UNorm => "R8_unorm",
        ColorType::BGRA10101010XR => "BGRA_10101010_XR",
    }
}
