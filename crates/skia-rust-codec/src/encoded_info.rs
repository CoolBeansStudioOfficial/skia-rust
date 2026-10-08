// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/private/SkEncodedInfo.h#L32-L247, src/codec/SkEncodedInfo.cpp#L12-L93 (chrome/m156)
// Ported from: include/private/SkEncodedInfo.h, src/codec/SkEncodedInfo.cpp

//! Describes the encoded image: its size, channel layout and bit depth.

use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_skcms::IccProfile;

/// The alpha channel of the encoded data. Port of `SkEncodedInfo::Alpha`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "SkEncodedInfo::Alpha")]
pub enum Alpha {
    /// There is no alpha channel.
    Opaque,
    /// Alpha is stored, not premultiplied.
    Unpremul,
    /// Alpha is only 0 or 255.
    Binary,
}

/// The color channels of the encoded data. Port of `SkEncodedInfo::Color`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(alias = "SkEncodedInfo::Color")]
pub enum Color {
    /// One gray channel.
    Gray,
    /// Gray followed by alpha.
    GrayAlpha,
    /// One alpha channel only.
    XAlpha,
    /// 5-6-5 packed color.
    Color565,
    /// Indices into a palette.
    Palette,
    /// Red, green, blue.
    RGB,
    /// Red, green, blue, alpha.
    RGBA,
    /// Blue, green, red.
    BGR,
    /// Blue, green, red, unused.
    BGRX,
    /// Blue, green, red, alpha.
    BGRA,
    /// Y, Cb, Cr.
    YUV,
    /// Y, Cb, Cr, alpha.
    YUVA,
    /// Inverted cyan, magenta, yellow, black.
    InvertedCMYK,
    /// Y, Cc, Cb, K (inverted CMYK with YCC).
    YCCK,
}

/// Port of `SkEncodedInfo`. The HDR metadata and the `SkCodecs::ColorProfile` wrapper are not
/// ported yet: the ICC profile is carried as the parsed [`IccProfile`] and its bytes.
#[derive(Debug, Clone)]
#[doc(alias = "SkEncodedInfo")]
pub struct EncodedInfo {
    width: i32,
    height: i32,
    color: Color,
    alpha: Alpha,
    bits_per_component: u8,
    color_depth: u8,
    profile: Option<Arc<IccProfile>>,
    profile_data: Option<Arc<[u8]>>,
}

impl EncodedInfo {
    /// Port of `SkEncodedInfo::Make(width, height, color, alpha, bitsPerComponent)`.
    // Port of: include/private/SkEncodedInfo.h#L100-L105 (chrome/m156)
    #[doc(alias = "SkEncodedInfo::Make")]
    #[must_use]
    pub fn make(
        width: i32,
        height: i32,
        color: Color,
        alpha: Alpha,
        bits_per_component: u8,
    ) -> Self {
        Self::make_with_depth(
            width,
            height,
            color,
            alpha,
            bits_per_component,
            bits_per_component,
        )
    }

    /// Port of `SkEncodedInfo::Make` with an explicit color depth (`colorDepth`).
    // Port of: src/codec/SkEncodedInfo.cpp#L49-L60 (chrome/m156)
    #[must_use]
    pub fn make_with_depth(
        width: i32,
        height: i32,
        color: Color,
        alpha: Alpha,
        bits_per_component: u8,
        color_depth: u8,
    ) -> Self {
        debug_assert!(matches!(bits_per_component, 1 | 2 | 4 | 8 | 16));
        Self::verify_color(color, alpha, bits_per_component);
        Self {
            width,
            height,
            color,
            alpha,
            bits_per_component,
            color_depth,
            profile: None,
            profile_data: None,
        }
    }

    /// Attaches an ICC profile (the `profile` argument of `SkEncodedInfo::Make`).
    #[must_use]
    pub fn with_profile(mut self, data: Arc<[u8]>, profile: IccProfile) -> Self {
        self.profile = Some(Arc::new(profile));
        self.profile_data = Some(data);
        self
    }

    // Port of: include/private/SkEncodedInfo.h#L193-L236 (VerifyColor; the asserts are
    // debug-only in Skia too)
    fn verify_color(color: Color, alpha: Alpha, bits_per_component: u8) {
        match color {
            Color::Gray => debug_assert_eq!(alpha, Alpha::Opaque),
            Color::GrayAlpha => debug_assert_ne!(alpha, Alpha::Opaque),
            Color::Palette => debug_assert_ne!(bits_per_component, 16),
            Color::RGB | Color::BGR | Color::BGRX => {
                debug_assert_eq!(alpha, Alpha::Opaque);
                debug_assert!(bits_per_component >= 8);
            }
            Color::YUV | Color::InvertedCMYK | Color::YCCK | Color::Color565 => {
                debug_assert_eq!(alpha, Alpha::Opaque);
                debug_assert_eq!(bits_per_component, 8);
            }
            Color::RGBA => debug_assert!(bits_per_component >= 8),
            Color::BGRA | Color::YUVA => debug_assert_eq!(bits_per_component, 8),
            Color::XAlpha => {
                debug_assert_eq!(alpha, Alpha::Unpremul);
                debug_assert_eq!(bits_per_component, 8);
            }
        }
    }

    /// Port of `SkEncodedInfo::makeImageInfo`: the natural image info of the encoded data.
    // Port of: src/codec/SkEncodedInfo.cpp#L12-L24 (chrome/m156)
    #[doc(alias = "makeImageInfo")]
    #[must_use]
    pub fn make_image_info(&self) -> ImageInfo {
        let ct = match self.color {
            Color::Gray => ColorType::Gray8,
            Color::XAlpha => ColorType::Alpha8,
            Color::Color565 => ColorType::RGB565,
            _ => ColorType::N32,
        };
        let alpha = if self.alpha == Alpha::Opaque {
            AlphaType::Opaque
        } else {
            AlphaType::Unpremul
        };
        // With no profile, or one with no exact colour space, the image is sRGB.
        let cs = self
            .profile
            .as_deref()
            .and_then(ColorSpace::make)
            .unwrap_or_else(ColorSpace::new_srgb);
        ImageInfo::new((self.width, self.height), ct, alpha, cs)
    }

    /// Port of `SkEncodedInfo::width`.
    #[must_use]
    pub fn width(&self) -> i32 {
        self.width
    }

    /// Port of `SkEncodedInfo::height`.
    #[must_use]
    pub fn height(&self) -> i32 {
        self.height
    }

    /// Port of `SkEncodedInfo::color`.
    #[must_use]
    pub fn color(&self) -> Color {
        self.color
    }

    /// Port of `SkEncodedInfo::alpha`.
    #[must_use]
    pub fn alpha(&self) -> Alpha {
        self.alpha
    }

    /// Port of `SkEncodedInfo::opaque`.
    #[must_use]
    pub fn opaque(&self) -> bool {
        self.alpha == Alpha::Opaque
    }

    /// Port of `SkEncodedInfo::bitsPerComponent`.
    #[must_use]
    pub fn bits_per_component(&self) -> u8 {
        self.bits_per_component
    }

    /// Port of `SkEncodedInfo::getColorDepth`.
    #[must_use]
    pub fn color_depth(&self) -> u8 {
        self.color_depth
    }

    /// Port of `SkEncodedInfo::bitsPerPixel`.
    // Port of: include/private/SkEncodedInfo.h#L139-L163 (chrome/m156)
    #[doc(alias = "bitsPerPixel")]
    #[must_use]
    pub fn bits_per_pixel(&self) -> u8 {
        let bpc = self.bits_per_component;
        match self.color {
            Color::Gray | Color::Palette => bpc,
            Color::XAlpha | Color::GrayAlpha => 2 * bpc,
            Color::RGB | Color::BGR | Color::YUV | Color::Color565 => 3 * bpc,
            Color::RGBA
            | Color::BGRA
            | Color::BGRX
            | Color::YUVA
            | Color::InvertedCMYK
            | Color::YCCK => 4 * bpc,
        }
    }

    /// Port of `SkEncodedInfo::profile`: the parsed ICC profile, if any.
    #[must_use]
    pub fn profile(&self) -> Option<&IccProfile> {
        self.profile.as_deref()
    }

    /// Port of `SkEncodedInfo::profileData`: the raw ICC bytes, if any.
    #[must_use]
    pub fn profile_data(&self) -> Option<&Arc<[u8]>> {
        self.profile_data.as_ref()
    }
}
