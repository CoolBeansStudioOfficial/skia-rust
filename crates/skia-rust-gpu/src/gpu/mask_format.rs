// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/MaskFormat.h

//! `skgpu::MaskFormat`: the pixel formats of the GPU mask atlases (A8, A565 LCD, ARGB).

use skia_rust_core::color_type::ColorType;

/// `skgpu::MaskFormat`.
// Port of: src/gpu/MaskFormat.h#L19-L30 (chrome/m156)
#[doc(alias = "skgpu::MaskFormat")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MaskFormat {
    /// 1-byte per pixel.
    #[doc(alias = "kA8")]
    A8,
    /// 2-bytes per pixel, RGB represent 3-channel LCD coverage.
    #[doc(alias = "kA565")]
    A565,
    /// 4-bytes per pixel, color format.
    #[doc(alias = "kARGB")]
    Argb,
}

/// `kMaskFormatCount`.
// Port of: src/gpu/MaskFormat.h#L35 (chrome/m156)
pub const MASK_FORMAT_COUNT: usize = 3;

impl MaskFormat {
    /// `MaskFormatBytesPerPixel()`.
    // Port of: src/gpu/MaskFormat.h#L40-L47 (chrome/m156)
    #[doc(alias = "MaskFormatBytesPerPixel")]
    #[must_use]
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            MaskFormat::A8 => 1,
            MaskFormat::A565 => 2,
            MaskFormat::Argb => 4,
        }
    }

    /// `MaskFormatToColorType()`.
    // Port of: src/gpu/MaskFormat.h#L50-L57 (chrome/m156)
    #[doc(alias = "MaskFormatToColorType")]
    #[must_use]
    pub const fn to_color_type(self) -> ColorType {
        match self {
            MaskFormat::A8 => ColorType::Alpha8,
            MaskFormat::A565 => ColorType::RGB565,
            MaskFormat::Argb => ColorType::RGBA8888,
        }
    }
}
