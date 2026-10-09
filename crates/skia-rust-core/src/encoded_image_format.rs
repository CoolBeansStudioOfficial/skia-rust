// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/codec/SkEncodedImageFormat.h#L16-L31 (chrome/m156)
// Ported from: include/codec/SkEncodedImageFormat.h

//! The formats of encoded image data.

/// The format of encoded data. Port of `SkEncodedImageFormat`; the variant names are skia-safe's
/// (the C++ `k` prefix is dropped).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[doc(alias = "SkEncodedImageFormat")]
pub enum EncodedImageFormat {
    /// Port of `kBMP`.
    #[doc(alias = "kBMP")]
    BMP,
    /// Port of `kGIF`.
    #[doc(alias = "kGIF")]
    GIF,
    /// Port of `kICO`.
    #[doc(alias = "kICO")]
    ICO,
    /// Port of `kJPEG`.
    #[doc(alias = "kJPEG")]
    JPEG,
    /// Port of `kPNG`.
    #[doc(alias = "kPNG")]
    PNG,
    /// Port of `kWBMP`.
    #[doc(alias = "kWBMP")]
    WBMP,
    /// Port of `kWEBP`.
    #[doc(alias = "kWEBP")]
    WEBP,
    /// Port of `kPKM`.
    #[doc(alias = "kPKM")]
    PKM,
    /// Port of `kKTX`.
    #[doc(alias = "kKTX")]
    KTX,
    /// Port of `kASTC`.
    #[doc(alias = "kASTC")]
    ASTC,
    /// Port of `kDNG`.
    #[doc(alias = "kDNG")]
    DNG,
    /// Port of `kHEIF`.
    #[doc(alias = "kHEIF")]
    HEIF,
    /// Port of `kAVIF`.
    #[doc(alias = "kAVIF")]
    AVIF,
    /// Port of `kJPEGXL`.
    #[doc(alias = "kJPEGXL")]
    JPEGXL,
}
