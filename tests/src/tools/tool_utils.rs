// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/ToolUtils.{h,cpp} (the parts the ported tests use)

//! `ToolUtils`: helpers shared by Skia's tests.

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_type::ColorType;

/// `ToolUtils::colortype_name`.
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

/// `ToolUtils::copy_to`: copies `src` into a new bitmap of color type `dst_color_type` (same
/// dimensions, alpha type and color space), converting the pixels; on success swaps it into
/// `dst` and returns true.
// Port of: tools/ToolUtils.cpp#L395-L422 (chrome/m156)
pub fn copy_to(dst: &mut Bitmap, dst_color_type: ColorType, src: &Bitmap) -> bool {
    let Some(src_pm) = src.peek_pixels() else {
        return false;
    };

    let mut tmp_dst = Bitmap::new();
    let dst_info = src_pm.info().with_color_type(dst_color_type);
    if !tmp_dst.set_info(&dst_info, None) {
        return false;
    }

    if !tmp_dst.try_alloc_pixels() {
        return false;
    }

    let Some(mut dst_pm) = tmp_dst.peek_pixels_mut() else {
        return false;
    };

    if !src_pm.read_pixels_to_pixmap(&mut dst_pm, (0, 0)) {
        return false;
    }

    dst.swap(&mut tmp_dst);
    true
}
