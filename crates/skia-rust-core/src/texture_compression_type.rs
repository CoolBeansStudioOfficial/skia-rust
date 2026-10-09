// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkTextureCompressionType.h

//! `SkTextureCompressionType`.

/// The compressed texture formats Skia knows about.
///
/// | Skia | `GL_COMPRESSED_*` | `MTLPixelFormat*` | `VK_FORMAT_*_BLOCK` |
/// |---|---|---|---|
/// | `ETC2_RGB8_UNORM` | `ETC1_RGB8`, `RGB8_ETC2` | `ETC2_RGB8` (iOS-only) | `ETC2_R8G8B8_UNORM` |
/// | `BC1_RGB8_UNORM` | `RGB_S3TC_DXT1_EXT` | N/A | `BC1_RGB_UNORM` |
/// | `BC1_RGBA8_UNORM` | `RGBA_S3TC_DXT1_EXT` | `BC1_RGBA` (macOS-only) | `BC1_RGBA_UNORM` |
// Port of: include/core/SkTextureCompressionType.h#L13-L32 (chrome/m156)
#[doc(alias = "SkTextureCompressionType")]
#[allow(non_camel_case_types)] // skia-safe keeps Skia's variant names (`BC1_RGBA8_UNORM`)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum TextureCompressionType {
    /// No compression.
    #[doc(alias = "kNone")]
    #[default]
    None = 0,
    /// ETC2 RGB8 (also ETC1 RGB8).
    #[doc(alias = "kETC2_RGB8_UNORM")]
    #[doc(alias = "kETC1_RGB8")]
    ETC2_RGB8_UNORM = 1,
    /// BC1 RGB8.
    #[doc(alias = "kBC1_RGB8_UNORM")]
    BC1_RGB8_UNORM = 2,
    /// BC1 RGBA8.
    #[doc(alias = "kBC1_RGBA8_UNORM")]
    BC1_RGBA8_UNORM = 3,
}

impl TextureCompressionType {
    /// `kLast`.
    pub const LAST: Self = Self::BC1_RGBA8_UNORM;
    /// `kETC1_RGB8`, an alias of [`Self::ETC2_RGB8_UNORM`].
    pub const ETC1_RGB8: Self = Self::ETC2_RGB8_UNORM;
}
