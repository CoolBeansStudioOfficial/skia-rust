// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkAndroidCodec.cpp (the output rules and the constructors), and
// include/codec/SkAndroidCodec.h (the same methods) (chrome/m156)
// Port of: src/codec/SkCodecColorProfile.cpp#L23-L51 (cicp_get_android_sk_color_space) and
// #L146-L168 (ColorProfile::getAndroidOutputColorSpace), the parts the output colour space uses.
// Ported from: src/codec/SkAndroidCodec.cpp, include/codec/SkAndroidCodec.h,
// src/codec/SkCodecColorProfile.cpp
//
// Not ported yet: sampled decoding (`getAndroidPixels`, `getSampledDimensions`, `computeSampleSize`,
// `getSupportedSubset`), which needs the sampler of each decoder (`getSampler`, SkSampledCodec),
// and the adapter for WebP, GIF, AVIF and HEIF (SkAndroidCodecAdapter). The gainmap accessors are
// not ported either.

//! The Android codec: a codec with the output colour type, alpha type and colour space rules that
//! Android's image decoder uses.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_space::{ColorSpace, named_primaries, named_transfer_fn};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::stream::Stream;
use skia_rust_skcms::{IccProfile, TransferFunction};

use crate::codec::Codec;
use crate::codecs;

/// A codec with Android's output rules. Port of `SkAndroidCodec`.
#[derive(Debug)]
#[doc(alias = "SkAndroidCodec")]
pub struct AndroidCodec<'a> {
    // Port of `fInfo`: the codec's natural info, taken when the codec was made.
    info: ImageInfo,
    // Port of `fCodec`.
    codec: Codec<'a>,
}

impl<'a> AndroidCodec<'a> {
    /// Port of `SkAndroidCodec::MakeFromCodec`: wraps a codec for the formats Android decodes. The
    /// formats whose Android wrapper is not ported (GIF, WebP, AVIF, HEIF) and the ones Android does
    /// not support return `None`.
    // Port of: src/codec/SkAndroidCodec.cpp#L43-L72 (chrome/m156), the sampled arm
    #[doc(alias = "SkAndroidCodec::MakeFromCodec")]
    #[must_use]
    pub fn make_from_codec(codec: Codec<'a>) -> Option<Self> {
        match codec.encoded_format() {
            EncodedImageFormat::PNG
            | EncodedImageFormat::ICO
            | EncodedImageFormat::JPEG
            | EncodedImageFormat::BMP
            | EncodedImageFormat::WBMP => Some(Self {
                info: codec.info(),
                codec,
            }),
            _ => None,
        }
    }

    /// Port of `SkAndroidCodec::MakeFromStream` (with no PNG chunk reader): makes the codec with the
    /// registered decoders, then wraps it.
    // Port of: src/codec/SkAndroidCodec.cpp#L35-L39 (chrome/m156)
    #[doc(alias = "SkAndroidCodec::MakeFromStream")]
    #[must_use]
    pub fn make_from_stream(stream: Box<dyn Stream + Send + 'a>) -> Option<Self> {
        let codec = codecs::make_codec_from_stream(stream).ok()?;
        Self::make_from_codec(codec)
    }

    /// Port of `SkAndroidCodec::getInfo`: the codec's natural info.
    #[must_use]
    #[doc(alias = "getInfo")]
    pub fn info(&self) -> ImageInfo {
        self.info.clone()
    }

    /// The codec this wraps. Port of `SkAndroidCodec::codec`.
    #[must_use]
    pub fn codec(&self) -> &Codec<'a> {
        &self.codec
    }

    /// Port of `SkAndroidCodec::getEncodedFormat` (through the wrapped codec).
    #[must_use]
    pub fn encoded_format(&self) -> EncodedImageFormat {
        self.codec.encoded_format()
    }

    /// Port of `SkAndroidCodec::computeOutputColorType`: the colour type the codec decodes to for a
    /// requested one. High-precision files decode to F16 and 10-bit ones to 1010102 by default.
    // Port of: src/codec/SkAndroidCodec.cpp#L74-L113 (chrome/m156)
    #[must_use]
    #[doc(alias = "computeOutputColorType")]
    pub fn compute_output_color_type(&self, requested_color_type: ColorType) -> ColorType {
        let high_precision = self.codec.encoded_info().bits_per_component() > 8;
        let color_depth = self.codec.encoded_info().color_depth();
        match requested_color_type {
            ColorType::ARGB4444 => return ColorType::N32,
            // Before Gray8 existed, clients requested Alpha8 for a grayscale decode, so Alpha8 falls
            // through to Gray8.
            ColorType::Alpha8 | ColorType::Gray8 => {
                if self.info.color_type() == ColorType::Gray8 {
                    return ColorType::Gray8;
                }
            }
            ColorType::RGB565 => {
                if self.info.alpha_type() == AlphaType::Opaque {
                    return ColorType::RGB565;
                }
            }
            ColorType::RGBA1010102 => {
                if color_depth == 10 {
                    return ColorType::RGBA1010102;
                }
            }
            ColorType::RGBAF16 => return ColorType::RGBAF16,
            _ => {}
        }

        // F16 is the Android default for high-precision images.
        if high_precision {
            ColorType::RGBAF16
        } else if color_depth == 10 {
            ColorType::RGBA1010102
        } else {
            ColorType::N32
        }
    }

    /// Port of `SkAndroidCodec::computeOutputAlphaType`: opaque images stay opaque; otherwise the
    /// requested unpremultiplied or premultiplied alpha.
    // Port of: src/codec/SkAndroidCodec.cpp#L115-L120 (chrome/m156)
    #[must_use]
    #[doc(alias = "computeOutputAlphaType")]
    pub fn compute_output_alpha_type(&self, requested_unpremul: bool) -> AlphaType {
        if self.info.alpha_type() == AlphaType::Opaque {
            return AlphaType::Opaque;
        }
        if requested_unpremul {
            AlphaType::Unpremul
        } else {
            AlphaType::Premul
        }
    }

    /// Port of `SkAndroidCodec::computeOutputColorSpace`: the colour space a decode to
    /// `output_color_type` produces. A preferred space wins; then the image's own profile; then sRGB.
    /// Colour types that Android does not colour-correct have none.
    // Port of: src/codec/SkAndroidCodec.cpp#L122-L157 (chrome/m156)
    #[must_use]
    #[doc(alias = "computeOutputColorSpace")]
    pub fn compute_output_color_space(
        &self,
        output_color_type: ColorType,
        pref_color_space: Option<ColorSpace>,
    ) -> Option<ColorSpace> {
        match output_color_type {
            ColorType::RGBAF16
            | ColorType::RGB565
            | ColorType::RGBA8888
            | ColorType::BGRA8888
            | ColorType::RGBA1010102 => {
                // If a preferred space is supplied, choose it.
                if pref_color_space.is_some() {
                    return pref_color_space;
                }
                if let Some(profile) = self.codec.encoded_info().profile() {
                    return get_android_output_color_space(profile);
                }
                Some(ColorSpace::new_srgb())
            }
            // Colour correction is not supported for gray.
            _ => None,
        }
    }
}

/// Port of `ColorProfile::getAndroidOutputColorSpace`: a CICP description when the profile has one,
/// else the profile's own colour space, else its primaries with the sRGB curve, else sRGB.
// Port of: src/codec/SkCodecColorProfile.cpp#L146-L168 (chrome/m156)
#[must_use]
pub fn get_android_output_color_space(profile: &IccProfile) -> Option<ColorSpace> {
    // Prefer CICP information if it exists.
    if profile.has_cicp {
        let cicp = &profile.cicp;
        if let Some(cicp_space) = cicp_get_android_sk_color_space(
            cicp.color_primaries,
            cicp.transfer_characteristics,
            cicp.matrix_coefficients,
            cicp.video_full_range_flag,
        ) {
            return Some(cicp_space);
        }
    }
    // Leave the pixels in the encoded colour space. Colour space conversion is done after decode.
    if let Some(encoded_space) = ColorSpace::make(profile) {
        return Some(encoded_space);
    }
    if profile.has_to_xyzd50 {
        return ColorSpace::new_rgb(&named_transfer_fn::SRGB, &profile.to_xyzd50);
    }
    Some(ColorSpace::new_srgb())
}

// The constants are written at the precision Skia uses; `f32` rounds them to the same values.
/// Port of `cicp_get_android_sk_color_space`: the colour space of a CICP description, with the
/// Android adjustments to PQ and HLG that put SDR white at 203 nits. Only full-range, non-matrix
/// descriptions are handled.
// Port of: src/codec/SkCodecColorProfile.cpp#L23-L51 (chrome/m156)
#[must_use]
#[allow(clippy::excessive_precision)] // constants as written in Skia (see the note above)
fn cicp_get_android_sk_color_space(
    color_primaries: u8,
    transfer_characteristics: u8,
    matrix_coefficients: u8,
    full_range_flag: u8,
) -> Option<ColorSpace> {
    if matrix_coefficients != 0 {
        return None;
    }
    if full_range_flag != 1 {
        return None;
    }
    let primaries = named_primaries::get_cicp(named_primaries::CicpId::from_u8(color_primaries)?)?;
    let mut trfn: TransferFunction = named_transfer_fn::get_cicp(
        named_transfer_fn::CicpId::from_u8(transfer_characteristics)?,
    )?;
    match transfer_characteristics {
        16 => {
            // Android expects PQ to match 203 nits to SDR white.
            trfn = TransferFunction {
                g: -2.0,
                a: -1.555_222_978_32,
                b: 1.860_453_656_31,
                c: 32.0 / 2523.0,
                d: 2413.0 / 128.0,
                e: -2392.0 / 128.0,
                f: 8192.0 / 1305.0,
            };
        }
        18 => {
            // Android expects HLG to match 203 nits to SDR white.
            trfn = TransferFunction::make_scaled_hlgish(
                0.314_509_843,
                2.0,
                2.0,
                1.0 / 0.178_832_77,
                0.284_668_92,
                0.559_910_73,
            );
        }
        _ => {}
    }
    ColorSpace::new_rgb(&trfn, &primaries)
}
